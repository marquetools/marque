# PR 3c.2.A — Scaffolding: PM Decisions Register

**Date**: 2026-05-19
**Branch**: `refactor-006-pr-3c2-a-scaffolding` (off `origin/staging@4f87901f`)
**Base PR**: `staging`
**Status**: LOCKED — PM contract for the scaffolding sub-PR.

**Predecessor**: `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` (whole-series PM contract — D25.1–D25.7).
**Successor**: `docs/plans/2026-05-19-pr3c2-b-pm-decisions.md` (call-site migration; will be authored before 3c.2.B starts).

**Spec anchors**:
- `specs/006-engine-rule-refactor/tasks.md` T043, T048, T048a, T048b, T048c
- `specs/006-engine-rule-refactor/spec.md` FR-035a, FR-043, FR-052, FR-053
- `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` §1 "3c.2.A — Scaffolding" row
- `crates/capco/docs/CAPCO-2016.md` §G.1 Table 4 (column terms for `EmissionForm` variants)
- `crates/capco/docs/CAPCO-2016.md` §B.3 Table 2 p21 (citation form precedent — `Section.subsection Table N pNN`)

---

## 0. Scope

PR 3c.2.A lands the type and trait-surface scaffolding for the four FR-035a commitments — type definitions only, no behavioral wiring of digests, no atomic cutover. Schema stays `marque-mvp-3`; D bumps to `marque-1.0`.

**Closes** (per plan §1): T043 (groundwork), T048 (signature update), T048a (RenderContext + EmissionForm definitions), T048b (Engine wiring), T048c (EmissionForm tests).

**Implied prerequisites landing alongside**:
- `MarkingScheme::canonicalize` trait method with `unimplemented!()` default (see PM-1).
- `blake3` workspace dep (declaration only; no per-crate consumer — see PM-4).

**Out of scope** (deferred to later 3c.2 sub-PRs):
- `from_parsed_unchecked` call-site migration to `<S as MarkingScheme>::canonicalize` — 3c.2.B.
- `Diagnostic.citation: &'static str → Citation` field migration — 3c.2.C.
- `Diagnostic.message: Box<str> → Message` migration — 3c.2.C.
- Per-crate `blake3` dep, real `Blake3Hash::zero()` replacement, `AppliedFix` v2, schema bump to `marque-1.0` — 3c.2.D.
- `from_parsed_unchecked` adapter deletion — 3c.2.E.

---

## 1. PM Decisions

### PM-3c.2.A-1 — `MarkingScheme::canonicalize` signature: GAT-based associated types, `unimplemented!()` default

**Decision**: The trait method is

```rust
trait MarkingScheme {
    // existing associated types: Token, Marking, ParseError, OpenVocabRef ...

    /// Sole authorized constructor of `Self::Canonical` from `Self::Parsed`
    /// per FR-043 (post-3c.2.E sole path). PR 3c.2.B implements the
    /// CapcoScheme override; test stubs inherit the default safely.
    type Parsed<'src>;
    type Canonical;

    fn canonicalize<'src>(&self, parsed: Self::Parsed<'src>) -> Self::Canonical {
        unimplemented!(
            "MarkingScheme::canonicalize not overridden by this scheme. \
             PR 3c.2.B implements the CapcoScheme override; test stub \
             schemes that never call canonicalize() inherit the default \
             safely (the panic is unreachable from their code paths)."
        )
    }
}
```

`type Parsed<'src>` is a GAT (generic associated type). `type Canonical` is a plain associated type.

**Erratum to predecessor plan §4 R-1**: The PM contract reads "the default `canonicalize` impl in PR 3c.2.A delegates to `from_parsed_unchecked`." That delegation is **incompatible with Constitution VII directionality** — `marque-scheme` cannot reference `marque_ism::from_parsed_unchecked`. The default impl is `unimplemented!()`; the CapcoScheme override at 3c.2.B carries the `from_parsed_unchecked` body.

**Rationale**:
- Constitution VII §VII: the only permitted edge between the two crates is `marque-ism → marque-scheme`. A trait method body in `marque-scheme` cannot syntactically delegate to a function in `marque-ism`.
- Future schemes (CUI, NATO) bind their own `type Parsed<'src> = TheirParsed<'src>; type Canonical = TheirCanonical;`. The trait is grammar-neutral.
- Precedent: `MarkingScheme::apply_intent` (`scheme.rs:265-278`) uses the same `unimplemented!()` default pattern for "scheme has not migrated" semantics.
- GATs stabilized in Rust 1.65; workspace MSRV is 1.85. ✓

**Rejected alternatives**:
- Free function only (no trait method): defeats FR-043's "sole post-keystone path" invariant. Future schemes would either re-import CAPCO's function or invent their own — no trait enforcement.
- Separate `Canonicalize<P, C>` trait alongside `MarkingScheme`: introduces a permanent double-bound (`S: MarkingScheme + Canonicalize<...>`) at every engine call site. Adds cross-cutting cognitive load forever to support a single-PR-of-lifetime adapter.
- Default impl delegating to `from_parsed_unchecked`: blocked by Constitution VII (the predecessor R-1 erratum).

### PM-3c.2.A-2 — Module placement

**Decision**:
- `RenderContext`, `EmissionForm`, `SchemaVersionId` live in a new file `crates/scheme/src/render_context.rs`, re-exported from `crates/scheme/src/lib.rs`.
- `Citation`, `SectionRef`, `SectionLetter`, `PageNumber`, `AuthoritativeSource` live in a new file `crates/rules/src/citation.rs`, re-exported from `crates/rules/src/lib.rs`.
- The `MarkingScheme::canonicalize` trait method, plus `type Parsed<'src>` and `type Canonical` declarations, live in `crates/scheme/src/scheme.rs` alongside the existing `apply_intent` declaration.

**Rationale**:
- `scheme.rs` is already large; single-responsibility module split is correct for the render-context surface (it lives in `marque-scheme` but is not itself the trait).
- `Citation` is a field of `Diagnostic` (which lives in `marque-rules`); `marque-rules` already depends on `marque-scheme`. Placing `Citation` in `marque-scheme` would either invert the dep edge or force a new edge — both are worse than the leaf-respecting placement.
- The trait method declaration belongs adjacent to the other trait methods in `scheme.rs`. Module split for the parameter type is fine; trait method split would obscure the trait surface.

**Rejected alternatives**:
- All five types in one new `marque-citation` crate: over-engineered for the surface size. Reconsider if a future scheme needs the types without `marque-rules`.
- `Citation` in `marque-scheme`: violates the leaf property — `Citation`'s field types (`SectionRef`, `PageNumber`, `AuthoritativeSource`) are domain-concrete and have no business in the scheme trait surface.

### PM-3c.2.A-3 — `SchemaVersionId`: closed enum at A, `as_str()` bridges to wire format

**Decision**:

```rust
// crates/scheme/src/render_context.rs
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaVersionId {
    /// `marque-mvp-3` — active at PR 3c.2.A landing. The atomic cutover
    /// to `marque-1.0` lands at PR 3c.2.D per FR-035a.
    MarqueMvp3,
}

impl SchemaVersionId {
    /// Bridge to the wire-format string. `marque-engine`'s
    /// `AUDIT_SCHEMA_VERSION: &'static str` stays parallel; this is the
    /// `RenderContext`-side accessor.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarqueMvp3 => "marque-mvp-3",
        }
    }
}
```

`marque-engine`'s `AUDIT_SCHEMA_VERSION: &'static str = env!("MARQUE_AUDIT_SCHEMA")` (at `crates/engine/src/lib.rs:87`) stays unchanged at A. 3c.2.D adds the `V1_0` variant and flips the default.

**Rationale**:
- Const-evaluability composes with `Citation::new` (const fn per D25.2) and future const-fn renderer paths.
- Closed enum is Constitution VIII shape — schema versions are an enumerable authority surface, not an open string namespace.
- `#[non_exhaustive]` reserves `V1_0` for 3c.2.D's bump; every `match` becomes a compile error at D pointing at the site that needs to decide.
- The wire-format string stays `&'static str` (the audit NDJSON serializer emits `"schema": "marque-mvp-3"`; the wire is string-typed by FR-014).

**Rejected alternatives**:
- `&'static str` field on `RenderContext`: defers the compile-time discipline to D and forces D to do two migrations (`&'static str` → `SchemaVersionId` AND `MarqueMvp3` → `V1_0`). Closed enum at A means D's diff is purely additive.
- Migrate `AUDIT_SCHEMA_VERSION` to `SchemaVersionId` in A: the wire format is string-typed; the const stays. Bridging via `as_str()` is the right shape.

### PM-3c.2.A-4 — `blake3` workspace dep with WASM-safe feature set

**Decision**:

```toml
# Cargo.toml, [workspace.dependencies]
blake3 = { version = "1", default-features = false, features = ["pure"] }
```

**No per-crate consumer line in A.** The dep stays in the workspace lockfile but no crate's build closure pulls blake3 until 3c.2.D adds `marque-engine.dependencies.blake3`.

**Rationale**:
- `default-features = false` matches the WASM-size discipline established at `Cargo.toml:32-155` for other size-sensitive deps.
- `features = ["pure"]` selects pure-Rust SIMD-free path on `wasm32-unknown-unknown`. SIMD/intrinsics features bloat the WASM binary above the D25.7 ≤5% size budget.
- Workspace-only declaration at A keeps the dep in the lockfile (allows `cargo deny` license check) without committing any crate to ship blake3 at A. D adds the per-crate consumer.

**OQ-6 resolution**: WASM CI matrix at `.github/workflows/ci.yml:498+` runs `wasm-pack build crates/wasm`. blake3-on-wasm32 is exercised at 3c.2.D when `marque-engine` adds the per-crate dep. A does NOT need an additional WASM job.

### PM-3c.2.A-5 — `Citation` types: rich `SectionRef`, closed `SectionLetter`, `NonZeroU16` page

**Decision**:

```rust
// crates/rules/src/citation.rs
use core::num::{NonZeroU8, NonZeroU16};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    pub document: AuthoritativeSource,
    pub section: SectionRef,
    pub page: PageNumber,
}

impl Citation {
    /// Const-fn constructor — no runtime validation per
    /// `2026-05-19-pr3c2-plan-and-decisions.md` D25.2. Citation-lint at
    /// `tools/citation-lint/` catches drift at CI time.
    pub const fn new(document: AuthoritativeSource, section: SectionRef, page: PageNumber) -> Self {
        Self { document, section, page }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionRef {
    pub letter: SectionLetter,
    /// Subsection number (`§H.5` → subsection = Some(5)).
    pub subsection: Option<NonZeroU8>,
    /// Sub-subsection (`§H.5.4` → sub_subsection = Some(4)).
    pub sub_subsection: Option<NonZeroU8>,
    /// Table number when the citation targets a specific table
    /// (`§B.3 Table 2 p21` → table = Some(2)).
    pub table: Option<NonZeroU8>,
}

impl SectionRef {
    pub const fn new(letter: SectionLetter) -> Self {
        Self { letter, subsection: None, sub_subsection: None, table: None }
    }

    pub const fn with_subsection(self, subsection: NonZeroU8) -> Self {
        Self { subsection: Some(subsection), ..self }
    }

    pub const fn with_sub_subsection(self, sub_subsection: NonZeroU8) -> Self {
        Self { sub_subsection: Some(sub_subsection), ..self }
    }

    pub const fn with_table(self, table: NonZeroU8) -> Self {
        Self { table: Some(table), ..self }
    }
}

/// CAPCO-2016 normative section letters per
/// `project_capco_doc_structure.md` (§A–H normative; §I–K excluded).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionLetter {
    A, B, C, D, E, F, G, H,
}

pub type PageNumber = NonZeroU16;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthoritativeSource {
    /// CAPCO-2016 Implementation Guide (the marque-vendored manual at
    /// `crates/capco/docs/CAPCO-2016.md`).
    Capco2016,
}
```

**Rationale**:
- `§B.3 Table 2 p21` is a real CAPCO citation form per project memory `project_capco_p20_caveated_definition.md`. The `table` field accommodates it losslessly. Bare 2-level `SectionRef { section, subsection }` would force "Table 2" to be encoded out-of-band — drift risk.
- `NonZeroU8` for subsection/sub_subsection/table niche-saves the `Option<u8>` tail and statically rejects sentinel-zero.
- `NonZeroU16` for page: CAPCO-2016 has ≤200 pages; the niche saves a byte via `Option<Citation>` and statically rejects page-zero.
- `SectionLetter` closed to `{A, B, C, D, E, F, G, H}` per Constitution VIII normative range. `#[non_exhaustive]` reserves grow-path for future grammars whose section vocabulary differs (CUI, NATO).
- `AuthoritativeSource::Capco2016` is the single variant; `#[non_exhaustive]` reserves variants like `NaroNistSp800_171` (future CUI), `NatoSecManual` (future NATO).
- Builder-style `with_*` const-fn methods keep top-level citation construction readable: `Citation::new(AuthoritativeSource::Capco2016, SectionRef::new(SectionLetter::B).with_subsection(NonZeroU8::new(3).unwrap()).with_table(NonZeroU8::new(2).unwrap()), NonZeroU16::new(21).unwrap())`. Verbose at site of declaration; data-only at runtime.

**Rejected alternatives**:
- `SectionRef` as a closed enum `{A, B, ..., H}` with subsection out-of-band: blocked by `§B.3 Table 2` shape.
- `PageNumber` as bare `u16`: defeats the type-level guarantee that page-zero is invalid.
- `SectionLetter` with `Annex(u8)`: speculative; defer to a future grammar's adoption PR per Constitution VIII.

### PM-3c.2.A-6 — No `Default` derive on `RenderContext` (explicit construction enforces audit trail)

**Decision**: `RenderContext` does NOT derive or implement `Default`. Construction is always explicit via `RenderContext::new(scope, emission_form, schema_version)`.

**Rationale**:
- T048b says "every existing rule emission site uses `EmissionForm::Auto`" — that's an explicit audit-trail requirement. A silent `Default` impl would hide the construction site from code review.
- A code-reviewer can grep `RenderContext::new(` and see every emission site; they cannot grep `RenderContext::default()` for "I forgot to pass Auto explicitly."
- The ergonomic cost (more verbose construction) is the right trade for auditability of the keystone migration.

**Rejected alternatives**:
- Derive `Default`: defaults silently to `Scope::Portion + EmissionForm::Auto + SchemaVersionId::MarqueMvp3`. Convenient but loses the audit-trail discipline T048b requires.

### PM-3c.2.A-7 — `Citation::Display` lands in A with round-trip property test

**Decision**: `impl Display for Citation` lands in A and emits the canonical citation-lint regex form (`§<Letter>.<subsection>[.<sub_subsection>] [Table <table>] p<page>` — verbatim CAPCO citation strings). Sibling `crates/rules/tests/citation_display_roundtrip.rs` tests Display output against representative CAPCO citation forms.

Concrete Display contract (binding for A):
- `Citation { document: Capco2016, section: SectionRef { letter: H, subsection: Some(4), sub_subsection: None, table: None }, page: 61 }` → `"§H.4 p61"`.
- `Citation { document: Capco2016, section: SectionRef { letter: B, subsection: Some(3), sub_subsection: None, table: Some(2) }, page: 21 }` → `"§B.3 Table 2 p21"`.
- `Citation { document: Capco2016, section: SectionRef { letter: H, subsection: Some(5), sub_subsection: Some(4), table: None }, page: 99 }` → `"§H.5.4 p99"`.

`document` is NOT rendered in Display today (CAPCO-2016 is the only authority); add `[CAPCO-2016]`-style prefix when a second `AuthoritativeSource` variant lands.

**Rationale**:
- 3c.2.C migrates `Diagnostic.citation: &'static str → Citation`. If `Display` lands in C, the format-spec might drift from the citation-lint regex; landing in A with golden tests catches the issue before C.
- Round-trip with citation-lint's regex form (the same one `tools/citation-lint/` parses) is the load-bearing correctness invariant. Sub-PR 3c.2.C then has zero format-spec work — `Display` is already shape-correct.

**Rejected alternatives**:
- Defer `Display` to 3c.2.C: bakes a format-spec drift risk into C. Architect preflight flagged this as a correctness concern outweighing the speculative "specing in advance" risk.

### PM-3c.2.A-8 — `Engine::synthesize_fixes` keeps `render_portion`/`render_banner` (default-impl delegation chain)

**Decision**: `crates/engine/src/engine.rs::synthesize_fixes` continues to call `scheme.render_portion(&modified)` and `scheme.render_banner(&modified)`. The trait-default impls of those methods construct `RenderContext` internally and delegate to `render_canonical`. CapcoScheme's overrides of `render_portion`/`render_banner` use the same pattern.

Plan §1 item 7 ("`Engine::fix_inner` wires `RenderContext` per fix") is satisfied via the default-impl delegation chain — `synthesize_fixes` doesn't need to construct `RenderContext` itself. Direct migration of `synthesize_fixes` to `render_canonical(.., &RenderContext::new(..), ..)` is a 3c.2.B+ option, NOT 3c.2.A scope.

**Rationale**:
- Byte-identity: the default-impl chain is the existing emission path, just with `RenderContext` threaded through internally. T056 corpus regression is the gate.
- Scope discipline: A's brief is scaffolding, not call-site migration. Keep the engine surface narrow.

### PM-3c.2.A-9 — Test stub migration: 26 sites get `type Parsed<'src> = (); type Canonical = ();`

**Decision**: Every `impl MarkingScheme` in `crates/scheme/src/`, `crates/scheme/tests/`, `crates/engine/tests/`, `crates/rules/src/`, and `crates/rules/tests/` (count: 26 verified via grep at 2026-05-19) gets two new associated-type declarations:

```rust
type Parsed<'src> = ();
type Canonical = ();
```

The `unimplemented!()` default on `canonicalize` means these stubs never trigger the canonicalize panic (they never call it). The associated-type declarations are mechanical syntactic requirements.

**Rationale**:
- GATs require associated type declaration on every impl, no default.
- `()` is the lowest-information binding; stubs that never canonicalize are honest about the absence.

### PM-3c.2.A-10 — `render_canonical_emission_form.rs` test scope: ship `Auto` cases enabled; ignore explicit-form cases pending 3c.2.B

**Decision**: `crates/capco/tests/render_canonical_emission_form.rs` ships in 3c.2.A with two patterns of tests:

1. **Enabled** (assert byte-identity with pre-3c.2 emission):
   - `EmissionForm::Auto + Scope::Page` for NOFORN, SECRET, FOUO → matches the pre-A `render_banner` output.
   - `EmissionForm::Auto + Scope::Portion` for NOFORN, SECRET, FOUO → matches the pre-A `render_portion` output.

2. **`#[ignore]`-gated** with `TODO(3c.2.B): unblock when §G.1 Table 4 dispatch lands`:
   - `EmissionForm::Portion` for NOFORN ("NF"), SECRET ("S"), FOUO ("FOUO").
   - `EmissionForm::BannerTitle` for NOFORN ("NOT RELEASABLE TO FOREIGN NATIONALS"), SECRET ("SECRET"), FOUO ("FOR OFFICIAL USE ONLY").
   - `EmissionForm::BannerAbbreviation` for NOFORN ("NOFORN"), SECRET ("SECRET" — abbreviation falls back to BannerTitle when no distinct abbreviation), FOUO ("FOUO").

The ignored tests are the FR-052 acceptance criteria. They flip to enabled in 3c.2.B when `CapcoScheme::render_canonical` dispatches on `EmissionForm` per §G.1 Table 4 column terms. Each ignored test carries the explicit comment naming 3c.2.B as the unblocking PR.

**Rationale**:
- A's body is scaffolding — `EmissionForm::Auto` is the only emission code path; the explicit forms are reserved for B.
- Shipping the test file with the eventual acceptance criteria in `#[ignore]` form keeps B's surface clean (just `#[ignore]` removal) and gives the test file its T048c shape now.

---

## 2. Commit sequence inside PR 3c.2.A

Five logical commits, each compiles standalone, each preserves T056 byte-identity:

| Commit | Touches | Surface added |
|---|---|---|
| **A1 — blake3 workspace dep** | `Cargo.toml` (workspace) | `blake3 = { version = "1", default-features = false, features = ["pure"] }` |
| **A2 — type defs in `marque-scheme`** | `crates/scheme/src/render_context.rs` (new), `crates/scheme/src/lib.rs` | `RenderContext`, `EmissionForm`, `SchemaVersionId` |
| **A3 — `Citation` types in `marque-rules`** | `crates/rules/src/citation.rs` (new), `crates/rules/src/lib.rs`, `crates/rules/tests/citation_display_roundtrip.rs` (new) | `Citation`, `SectionRef`, `SectionLetter`, `PageNumber`, `AuthoritativeSource` + Display + round-trip tests |
| **A4 — trait surface: `canonicalize` + `render_canonical` signature** | `crates/scheme/src/scheme.rs` + 26 `impl MarkingScheme` sites + 13+ test call sites + CapcoScheme override | `type Parsed<'src>` GAT, `type Canonical`, `canonicalize` method (default `unimplemented!()`), `render_canonical(.., &RenderContext, ..)` signature |
| **A5 — EmissionForm tests** | `crates/capco/tests/render_canonical_emission_form.rs` (new) | T048c test file per PM-10 (Auto cases enabled, explicit-form cases `#[ignore]`-gated) |

Each commit ends with `cargo check --workspace && cargo nextest run --workspace`. Final A5 also runs the WASM job locally if available (`wasm-pack build crates/wasm --target web --profiling`).

**Byte-identity invariant**: T056 corpus regression matrix MUST be green at every commit (the matrix runs on PR-3a/3b corpus subsets per T025/T029 precedent; PR 3c.2.A's branch filter `refactor-006-pr-3c2*` triggers the same matrix shape — verify in T145 mirror at A5).

---

## 3. Risk register additions

The predecessor plan §4 R-1 erratum is the headline. New A-specific risks:

### R-A1: PM plan §4 R-1 erratum

The PM contract reads "default `canonicalize` impl delegates to `from_parsed_unchecked`" — incompatible with Constitution VII directionality. **Mitigation**: PM-1 above records the erratum; the default impl is `unimplemented!()`. CapcoScheme override at 3c.2.B carries the body. Document this in the 3c.2.B preflight brief so the implementing agent doesn't re-derive the mistake.

### R-A2: First GAT on `MarkingScheme`

`type Parsed<'src>` is the first GAT on this trait. GAT inference rules differ from non-generic associated types in HRTB scenarios; future helpers bounded on `S: MarkingScheme` that consume `S::Parsed<'_>` may need `for<'a>` bounds. **Mitigation**: `cargo check --workspace` at A's CI pass catches HRTB-induced "implementation not general enough" errors at the boundary. If 3c.2.B helpers surface a real HRTB issue, escalate to PM — the fallback is a Q1(b)-style standalone `Canonicalize` trait.

### R-A3: 26 test stubs need synchronous migration

Adding `type Parsed<'src>` and `type Canonical` to `MarkingScheme` requires all 26 `impl MarkingScheme` sites to declare both associated types. **Mitigation**: Commit A4 batches all 26 sites in one diff; the additions are 2-line `type Parsed<'src> = (); type Canonical = ();` per stub.

### R-A4: `render_canonical` migration scope

13+ test call sites + production paths use the old `(marking, Scope, &mut Write)` signature. **Mitigation**: Commit A4 batches all sites in one diff. Migration is mechanical (`scope` → `&RenderContext::new(scope, EmissionForm::Auto, SchemaVersionId::MarqueMvp3)`). The CAPCO override's body reads `ctx.scope` exactly where it used to read `scope`; `emission_form` is unread until 3c.2.B.

### R-A5: `Citation::Display` round-trip with citation-lint regex

Per PM-7, Display ships in A with golden tests. **Mitigation**: golden tests at `crates/rules/tests/citation_display_roundtrip.rs` lock the format spec. If 3c.2.C surfaces a citation-lint regex mismatch, the bug is in C's migration of `&'static str` literal citations to `Citation::new(...)` construction — not in A's Display impl.

---

## 4. Reviewer attestation checklist (PR 3c.2.A)

For each of the three reviewers (rust-reviewer, code-reviewer, system-architect):

- [ ] Constitution VII directionality preserved: `marque-scheme` references no `marque-ism` types in trait method bodies or signatures.
- [ ] `MarkingScheme::canonicalize` default is `unimplemented!()`; no delegation to `from_parsed_unchecked`.
- [ ] `RenderContext`, `EmissionForm`, `SchemaVersionId` placement: `crates/scheme/src/render_context.rs` (NOT `scheme.rs`).
- [ ] `Citation`, `SectionRef`, `SectionLetter`, `PageNumber`, `AuthoritativeSource` placement: `crates/rules/src/citation.rs`.
- [ ] `Citation::new` is `const fn`. No runtime validation.
- [ ] No `Default` impl on `RenderContext`.
- [ ] `Citation::Display` round-trips through citation-lint regex form (golden tests passing).
- [ ] All 26 `impl MarkingScheme` sites declare `type Parsed<'src>` + `type Canonical`.
- [ ] T056 corpus regression matrix passes (byte-identity preserved).
- [ ] WASM CI matrix passes (`wasm-pack build crates/wasm`).
- [ ] `EmissionForm::Auto + Scope::{Page,Portion}` tests in `render_canonical_emission_form.rs` are ENABLED and pass; explicit-form tests are `#[ignore]`-gated with TODO referencing 3c.2.B.
- [ ] `blake3` is workspace dep only; no per-crate consumer line in A.
- [ ] No new `Diagnostic.citation` migration in A (stays `&'static str`).
- [ ] No new `Diagnostic.message` migration in A (stays `Box<str>`).
- [ ] No `__engine_promote` body change in A.
- [ ] Schema stays `marque-mvp-3`.
- [ ] All §-citations re-verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship (Constitution VIII propagation rule).
- [ ] Adjacent code paths walked: every consumer of the migrated signatures (trait impls, callers, test fixtures) verified.
- [ ] "Will we maintain this for 5 years?" — the trait surface decisions in A are load-bearing for CUI/NATO/future-scheme adoption. Any reviewer concern about scheme-neutrality MUST be raised before merge.
