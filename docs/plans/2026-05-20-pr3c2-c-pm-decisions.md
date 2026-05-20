# PR 3c.2.C — Diagnostic Reshape: PM Decisions

**Date**: 2026-05-20
**Branch**: `refactor-006-pr-3c2-c-diagnostic-reshape` (off `origin/staging@50d1e281`)
**Base PR**: `staging`
**Status**: LOCKED 2026-05-20 — PM contract; implementation agent proceeds against this scope.

**Predecessor preflights**:
- `docs/plans/2026-05-20-pr3c2-c-architect-preflight.md` (architect)
- `docs/plans/2026-05-20-pr3c2-c-rust-preflight.md` (rust-specialist)

**Master contract**: `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` §1 (D25.3, R-2)
**Deferred-findings register**: `specs/006-engine-rule-refactor/followups/2026-05-19-pr-3c2-a-deferred-findings.md`

**Spec anchors**: FR-003 (no `format!` of input bytes), FR-016 (audit ordering preserved), FR-035a (4 commitments — `Diagnostic` reshape covered by C), G13 (content-ignorance invariant).

---

## 0. Scope (binding)

PR 3c.2.C closes T042 (`Diagnostic` reshape), T046 (rule-emission migration to `Message::new`), and T050 (engine.rs decoder `format!` retirement). Schema stays on `marque-mvp-3` (no audit-schema bump in C — that's atomic at 3c.2.D).

**In scope** (atomic field-type changes):
1. `Diagnostic.message: Box<str>` → `Diagnostic.message: Message` (closed `MessageTemplate` + `MessageArgs` from PR 3c.1)
2. `Diagnostic.citation: &'static str` → `Diagnostic.citation: Citation` (typed struct from PR 3c.2.A)
3. 7 `format!`-built `message:` sites in `crates/capco/src/` migrate to `Message::new(template, args)`
4. 1 engine synthetic R001 site at `engine.rs:4026` migrates to `Message::new(MessageTemplate::DecoderRecognized, ...)`
5. 1 engine R002 site at `engine.rs:4109` migrates to `Message::new(MessageTemplate::ReparseFailed, ...)`
6. Engine citation constants `DECODER_CITATION`, `R002_CITATION`, `CORRECTIONS_MAP_CITATION` migrate to typed `Citation` constants
7. C-FOLLOWUP-3 stale forward-pointer comments updated (5 sites)
8. C-FOLLOWUP-5 pre-existing `clippy::question_mark` at `core/parser.rs:2199` resolved (opening housekeeping commit)
9. C-FOLLOWUP-4 cfg-gate lift on `s004_audit_content_ignorance.rs` + `rules_us1.rs` — **scoped INTO C** per PM-C-3 below
10. Test additions per PM-C-9 below

**Explicitly NOT in scope**:
- `ConstraintViolation.message: String → Message` — Constitution VII graph-leaf rule (PM-C-1)
- `ConstraintViolation.citation: &'static str → Citation` — same (PM-C-1)
- `PageRewrite.citation: &'static str → Citation` — same (PM-C-1)
- `ClassFloorRow.citation`, `SciPerSystemRow.citation`, `NonCanonicalRow.citation`, `CompanionAction.citation`, `JointHcsHelper.citation` — internal catalog rows in `marque-capco` stay `&'static str` and convert at the bridge (these don't violate VII because they're in `marque-capco`, but per PM-C-1 the bridge approach is uniform across all of them to keep the conversion in one place)
- `AuditNote.citation: &'static str → Citation` — out of scope per OQ-C1 (file GH-FOLLOWUP-C-1)
- `citation!()` declarative macro — REJECTED per PM-C-2; use `capco()` const-fn helper instead
- Audit-schema bump to `marque-1.0` — defers to 3c.2.D
- WASM JSON wire-format breaking change documentation — see PM-C-7

---

## 1. PM Decisions

### PM-C-1 — `ConstraintViolation` / `PageRewrite` / catalog-row citations: bridge conversion, not in-place migration

**Decision**: `ConstraintViolation.message: String` and `ConstraintViolation.citation: &'static str` (defined in `marque-scheme`) STAY unchanged in C. Same for `PageRewrite.citation: &'static str`. All internal `marque-capco` catalog row citations (`ClassFloorRow.citation`, `SciPerSystemRow.citation`, etc.) STAY `&'static str`. The bridge layer converts to typed `Citation` and `Message` at the `ConstraintViolation → Diagnostic` boundary, which lives in `crates/capco/src/rules_declarative.rs` (a `marque-capco` site — `marque-capco` already depends on `marque-rules`, so the conversion is graph-legal there).

**Why**: The architect preflight's OQ-C7 "migrate together" proposal would require `marque-scheme` to depend on `marque-rules` (to carry `Message` and `Citation` types as struct fields). `marque-scheme` is the leaf of the workspace dependency graph (Constitution VII canonical ordering); this is not an authorization question, it is a graph reversal — structurally impossible without re-architecting the trait surface.

The bridge-conversion approach is the only Constitution-VII-compliant path. It:
- Keeps `marque-scheme` as the graph leaf (Constitution VII §IV preserved)
- Moves type-conversion logic to `marque-capco` where it belongs (this is where rules construct their emission, after all)
- Avoids the engine-crate touch authorization concern entirely (no `marque-scheme` edits in C)

**Rejected alternatives**:
- (a) Architect's "migrate together" with PM authorization — structurally impossible due to graph leaf.
- (b) Move `Citation` / `Message` to `marque-scheme` — wrong layer; these are rule-emission types, not scheme types.
- (c) Synthetic `Message::new(MessageTemplate::ConstraintViolation, args{ free_form_string })` — would require a `String`-typed field on `MessageArgs`, violating the Constitution V closed-args invariant.

**Implementation note**: `crates/capco/src/scheme/adapter.rs::message_by_name` returns `Option<String>` today; under this decision it MAY return `Option<Message>` because `adapter.rs` is in `marque-capco` (which depends on `marque-rules`). The bridge gets a fully-typed `Message` from the dyadic catalog rows; for `Constraint::Custom` rows (where `message_by_name` returns `None`), the bridge falls back to a `template + args` mapping keyed on the `constraint_label`. Add `bridge_message(constraint_label: &'static str) -> Message` helper to `rules_declarative.rs` per rust-preflight R-C4.

### PM-C-2 — `citation!()` macro: REJECTED; use `capco()` const-fn helper

**Decision**: Do NOT ship a `citation!(§H.4 p61)` declarative macro. Ship a `capco(letter: SectionLetter, sub: u8, page: u16) -> Citation` private const-fn helper in `crates/rules/src/citation.rs` (re-exported through the prelude). For the ~57 `Diagnostic.citation` migration sites + the engine citation constants, the helper reduces ~120 chars to ~28 chars (`capco(SectionLetter::H, 4, 61)`).

**Why**: The rust-specialist preflight correctly identified that `§` is NOT a valid Rust token. `macro_rules!` cannot tokenize `§` in its match arms — the architect's `citation!(§H.4 p61)` proposal would fail to parse. The macro could accept `citation!(H.4 p61)` without the sigil, but the visual distinctiveness gain is small and the parsing surface is harder to extend cleanly.

The `capco()` const-fn helper:
- Compiles cleanly under Rust 1.85+ (we're on the 1.85 workspace floor) — `NonZeroU8::new(N).expect()` is const-fn since 1.83
- Const-compatible (usable in catalog row constants, `static` constants, `const fn` bodies)
- Citation-lint AST scanner already handles typed `Citation` values — no tooling update needed
- Future grammar extensions (`cui()`, `nato()`, etc.) land as parallel const-fn helpers; no macro-rules expansion problem

**Helper definition**:

```rust
// In crates/rules/src/citation.rs (re-exported via lib.rs prelude)

/// Const-fn ergonomic constructor for CAPCO-2016 citations.
///
/// Use this in catalog rows, `static` constants, and `const fn` bodies
/// to construct `Citation` values without the boilerplate of
/// `Citation::new(AuthoritativeSource::Capco2016, ...)`.
///
/// `page` and `subsection` must be non-zero; a `0` argument panics at
/// const evaluation (compile error).
///
/// # Examples
///
/// ```
/// use marque_rules::{capco, SectionLetter, Citation};
/// const SCI_GRAMMAR: Citation = capco(SectionLetter::H, 4, 61);
/// const CAVEATED_FDR: Citation = capco_table(SectionLetter::B, 3, 2, 21);
/// ```
pub const fn capco(letter: SectionLetter, subsection: u8, page: u16) -> Citation {
    let subsection = match core::num::NonZeroU8::new(subsection) {
        Some(n) => n,
        None => panic!("subsection must be non-zero"),
    };
    let page = match core::num::NonZeroU16::new(page) {
        Some(n) => n,
        None => panic!("page must be non-zero"),
    };
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(letter).with_subsection(subsection),
        page,
    )
}

/// Const-fn helper for CAPCO citations with a Table reference.
pub const fn capco_table(
    letter: SectionLetter,
    subsection: u8,
    table: u8,
    page: u16,
) -> Citation {
    let subsection = match core::num::NonZeroU8::new(subsection) {
        Some(n) => n,
        None => panic!("subsection must be non-zero"),
    };
    let table = match core::num::NonZeroU8::new(table) {
        Some(n) => n,
        None => panic!("table must be non-zero"),
    };
    let page = match core::num::NonZeroU16::new(page) {
        Some(n) => n,
        None => panic!("page must be non-zero"),
    };
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(letter)
            .with_subsection(subsection)
            .with_table(table),
        page,
    )
}
```

**No `unsafe`**: `NonZeroU8::new_unchecked` would shave a few nanoseconds at compile time at the cost of an unsafe block per construction. The const-fn `match`-based approach is safe, panics at compile time on invalid input, and is just as ergonomic at the call site.

**Bare `§<L>` shape** (no subsection, e.g., a hypothetical `§H pNN`): not currently used in CAPCO citations — every cited site has a subsection. No `capco_bare()` helper in C; add later if a future site needs it.

### PM-C-3 — Lift cfg-gates on `s004_audit_content_ignorance.rs` and `rules_us1.rs` in C

**Decision**: Both files lose their `#![cfg(any())]` gate in C5 (the atomic Diagnostic field-type migration commit). Test bodies are rewritten to validate the new `Message` + `Citation` shape.

**Why**: The architect preflight recommended NOT lifting because "FixProposal is gone; tests need full rewrite." But:
- `rules_us1.rs` already runs cleanly under PR 3c.2.B's `scheme.canonicalize(parsed.attrs)` migration (per the PM-B-7 note already in the file at line 78). The cfg-gate is the only thing keeping it from compiling; the body needs only the `Diagnostic.message`/`citation` field-type adjustments.
- `s004`'s test purpose is EXACTLY what C delivers: it asserts that `Diagnostic.message` carries no document-content bytes. The closed-template `Message` shape IS the assertion target. The test naturally adapts: `assert_eq!(diag.message.template(), expected_template)` and `assert!(diag.message.args().token.is_some())` (a `TokenId`, not a raw byte string).
- Coverage matters. Both tests are high-signal. Re-gating to 3c.2.D defers them past the point where they're needed.

**Test body changes required in C5**:
- `s004`: replace `diag.message.contains("AUT")` style assertions (which validated bytes-in-message) with `assert!(matches!(diag.message.template(), MessageTemplate::AmbiguousCountryCode))` + `assert_eq!(diag.message.args().token, Some(TOK_AUT))` style. The test purpose strengthens — the closed-set check is stricter than the substring check.
- `rules_us1`: `Vec<(String, usize, usize)>` return is rule-id-tuple format; doesn't inspect message content. Should compile as-is once cfg-gate lifts (modulo any `Diagnostic` field-access changes the body might have today).

If `s004`'s test bodies use `MessageTemplate` variants that don't exist yet (e.g., `AmbiguousCountryCode`), the implementation agent has authority to add the variant in C2 since C stays on `marque-mvp-3`. Per the closed-set invariant, variant additions during `marque-mvp-3` are permitted (only locked at `marque-1.0` transition in 3c.2.D).

### PM-C-4 — Add `AuthoritativeSource::Config` and `AuthoritativeSource::EngineInternal` variants in C2

**Decision**: Add both variants to `crates/rules/src/citation.rs::AuthoritativeSource` in C2 (the second commit). Both are sentinel sources for non-CAPCO citations:
- `Config` — for `CORRECTIONS_MAP_CITATION` (user's `.marque.toml` corrections map)
- `EngineInternal` — for `R002_CITATION` (engine-synthesized re-parse-failure diagnostic)

**Why**: `AuthoritativeSource` is `#[non_exhaustive]` per its declaration at `citation.rs:243`; adding variants is additive and non-breaking. The rust-specialist preflight correctly identifies that `DECODER_CITATION` is a CAPCO citation (`§A.6 p15`) but `CORRECTIONS_MAP_CITATION` and `R002_CITATION` are NOT — they should not use `AuthoritativeSource::Capco2016`.

Adding both at once avoids a follow-up commit. `#[non_exhaustive]` on the enum means downstream consumers can't write exhaustive matches on `AuthoritativeSource` anyway, so the additions cause zero break.

**Display rendering**: The `Citation::Display` impl currently does NOT render `document`. For non-CAPCO sources, the Display output `§A p1` is meaningless. Two options:
- (a) Add a document-prefix to Display when source ≠ CAPCO (e.g., `[Config] CORRECTIONS_MAP_CITATION` for `Config`-sourced)
- (b) Override Display for these specific sentinels (e.g., `CORRECTIONS_MAP_CITATION.fmt()` returns `"[config: corrections]"`)

**Resolution**: option (a). Update `Citation::Display` in C2 to prepend `[<source>] ` when source ≠ `Capco2016`. This keeps Display deterministic and machine-parseable. For `Config` source render as `[config]`; for `EngineInternal` source render as `[engine-internal]`. CAPCO sources continue to render bare (`§H.4 p61`).

**Constraint**: Update `tools/citation-lint` to recognize and skip the `[<source>] ` prefix for non-CAPCO citations (i.e., don't try to enforce CAPCO §-citations on sentinel non-CAPCO citations). This is part of C2's housekeeping.

### PM-C-5 — Drop runtime byte text from C001 and E014 messages (Constitution V Principle V)

**Decision**: ACCEPT the loss of runtime byte detail from `Diagnostic.message` for:
- C001 corrections-map: drop `text` (original typo bytes); keep `expected_token: Option<TokenId>` (replacement only when registered)
- E014 JOINT REL TO coverage: drop the runtime country list `[GBR, DEU]` from the message; use `MessageArgs { token: Some(TOK_JOINT), category: Some(CategoryId::RelTo) }`
- C001 alternative: per rust-preflight §2.4, `MessageArgs.token: None` is permitted when no `TokenId` projection exists for the matched substring; the audit consumer reconstructs from `(source, span)`

**Why**: This is the constitutional G13 closure for `Diagnostic.message`. Per Constitution V Principle V: "Audit records MUST be content-ignorant. No document content, document metadata field values, or subject-claim free-form text MAY appear in an `AppliedFix` or any future audit-adjacent record."

The renderer at the CLI/WASM emit layer CAN re-derive runtime bytes from `(source, span)` for human display — that's renderer responsibility, not engine responsibility. The CLI's `render_message(template, args, source, span) -> String` can produce human-friendly output like `"corrections-map fix: 'SERCET' → 'SECRET' at byte 142..148"` by looking up `source[span]`. The engine never produces that string itself.

**Renderer responsibility (downstream of C, but document now)**: when the WASM JSON emit at `crates/wasm/src/lib.rs:380` ships under the new structured shape, it can include a `rendered_message` companion field that is a human-readable string derived from `(template, args, source, span)`. That field is OUTSIDE the audit record contract — it's a presentation-layer convenience. The audit record itself stays content-ignorant.

### PM-C-6 — Drop SCI rollup "missing systems" list from E035 message

**Decision**: `MessageTemplate::BannerRollupMismatch` with `category: Some(CategoryId::Sci)` is the audit shape for E035. The per-system list is dropped from the audit. Renderers re-derive from `(source, span, live marking)` if needed for user display.

**Why**: Same constitutional reasoning as PM-C-5. Per rust-preflight §2.5, adding `feature_ids: SmallVec<[FeatureId; 8]>` to carry per-system identification would require coordinated `MARQUE_AUDIT_SCHEMA` bump — contradicting the no-schema-bump-in-C stance. The detail loss is acceptable; the audit retains category-level identification.

If a future user-feedback signal demands per-system identification, the path is: (a) add `MessageArgs.feature_ids` field, (b) bump `MARQUE_AUDIT_SCHEMA`, (c) add `FeatureId::SciSystem(Hcs|Si|Rsv|Tk)` variants. All deferred; file as GH-FOLLOWUP-C-2 if needed.

### PM-C-7 — WASM JSON wire-format change is ACCEPTED

**Decision**: The WASM lint output JSON shape changes in C from `{ "message": "free-form string", "citation": "CAPCO-2016 §H.4 p61" }` to `{ "message": { "template": "...", "args": { ... } }, "citation": "§H.4 p61" }`. This is a breaking change at the JSON wire layer.

**Why**: Marque is pre-users per project memory `feedback_pre_users_no_deprecation_phasing.md`. No external consumers exist; no migration phase needed. The structured-message shape is the load-bearing C delivery — it's what gives downstream consumers a parseable, machine-readable diagnostic format instead of a free-form string they have to grep.

**Document in PR description**: explicitly call out the JSON shape change with before/after examples. Add a comment to the WASM JSON emit code referencing this PM decision. Add a `MARQUE_LINT_JSON_SCHEMA` companion constant to the audit schema (pinned to "marque-mvp-3-lint" or similar) so a future consumer can detect the shape via a version handshake.

**NO `Display` on `Message`** per architectural lockdown — adding `Display` would create a covert free-form channel that defeats the closed-template invariant. The compile-fail doctests at `message.rs:506` block this.

### PM-C-8 — Commit sequence (build-green at each boundary)

**Decision**: 6-commit sequence based on rust-preflight §6, with C4 and C5 folded into one atomic commit to avoid a broken-compilation intermediate.

```
C1 — Housekeeping: clippy::question_mark fix at core/parser.rs:2199
C2 — Infrastructure: AuthoritativeSource variants + Citation constants + capco() / capco_table() const-fn helpers + citation-lint update for [<source>] prefix + compile-fail doctest for From<&str> for Citation
C3 — C-FOLLOWUP-3: 5 stale forward-pointer comment updates (mechanical)
C4 — FixDiagnosticParams.message: String → Message migration (capco-internal; updates 5 of 7 format! sites)
C5 — Atomic Diagnostic field-type migration (THE BIG ONE):
     * Diagnostic.message: Box<str> → Message
     * Diagnostic.citation: &'static str → Citation
     * All 5 Diagnostic constructors update signatures
     * ~57 Diagnostic.message + ~57 Diagnostic.citation call sites
     * Bridge layer: ConstraintViolation → Diagnostic conversion in rules_declarative.rs (bridge_message helper)
     * 2 helpers.rs ConstraintViolation.message format! sites stay String (per PM-C-1; bridge converts)
     * Engine R001 + R002 format! sites migrate
     * WASM JSON emit updates to structured shape
     * Lift cfg-gates on s004 + rules_us1; rewrite test bodies for new shape
     * Update existing tests in engine.rs that use diag.message.as_ref() (lines 6745, 6760)
C6 — Test additions + reviewer-pass closeout:
     * Compile-fail doctest: impl From<&str> for Citation (verify still doesn't exist)
     * Positive control: crates/rules/tests/citation_no_from_str.rs
     * Citation-lint real-parser round-trip (C-FOLLOWUP-1) at tools/citation-lint/tests/
     * Documentation updates in PR description
     * Final cargo clippy --workspace --all-targets -- -D warnings green
```

**Build-green gate at each commit**: `cargo check --workspace` exits 0. C5 is the only commit that touches the field type itself; all preceding commits add infrastructure or migrate internal helpers. C5 is unavoidably large (~350 lines of mechanical call-site changes); it must land as one commit per master plan D25.3 ("no transitional dual-field").

### PM-C-9 — Test coverage requirements (>80% per standing brief)

**Decision**: C ships with the following test surface:

1. **Existing test suite preservation**: `cargo test --workspace` must pass with zero regressions. The corpus harness (`ExpectedDiagnostic`) doesn't match on message text per OQ-C3 — it adapts transparently.

2. **Compile-fail doctest** added to `crates/rules/src/citation.rs`:
   ```rust
   /// ```compile_fail
   /// use marque_rules::Citation;
   /// let _: Citation = "CAPCO-2016 §H.4 p61".into();
   /// ```
   ```
   Proves `impl From<&str> for Citation` does not exist.

3. **Positive control** at `crates/rules/tests/citation_no_from_str.rs` (new file):
   ```rust
   #[test]
   fn citation_new_accepts_struct_construction() {
       use marque_rules::*;
       let _ = capco(SectionLetter::H, 4, 61);
   }
   ```

4. **Citation-lint real-parser round-trip** at `tools/citation-lint/tests/citation_display_roundtrip.rs` (new file, located in citation-lint to avoid dep inversion — see rust-preflight §4.2):
   - Round-trips `format!("{citation}")` through `citation_lint::find_in_fragment`
   - Asserts the parsed result matches the original `Citation` fields

5. **cfg-gate lifts** (PM-C-3) restore `s004_audit_content_ignorance.rs` and `rules_us1.rs` coverage. The s004 test's purpose strengthens under the closed-template shape — that's net coverage gain.

6. **No new free-form `format!`-in-`Diagnostic.message` paths**: post-C, `grep -rn 'message:\s*format!' crates/capco/src/` must return 0 hits. `grep -rn 'format!.*decoder-recognized\|format!.*post-pass-1' crates/engine/src/` must return 0 hits.

7. **WASM lint JSON wire-shape**: existing WASM parity test at `crates/wasm/tests/` may need updates to match the new structured-message shape. Implementation agent verifies parity is preserved after the wire-format update.

**CodeCov posture**: If CodeCov flags >5% coverage drop, expand the test suite per standing brief. The most likely test additions are: (a) explicit `Message::template()` + `args()` accessor tests for each migrated MessageTemplate variant, (b) `Citation::Display` rendering tests for each `AuthoritativeSource` variant (including the new `Config` / `EngineInternal` prefix behavior).

### PM-C-10 — Constitution check + reviewer attestation

**Decision**: PR 3c.2.C constitution-check posture:

| Principle | Verdict | Notes |
|---|---|---|
| **I (Performance)** | PASS | SC-001 16ms ceiling preserved; bench drift reported but not blocking per master D25.6. |
| **II (Zero-Copy)** | PASS | `Message` is stack-bound (no new heap allocation on hot path); `Citation: Copy`. |
| **III (WASM-Safe)** | PASS | `capco()` const-fn expands at compile time; no runtime validation code ships. |
| **IV (Two-Layer Rule)** | PASS | No Layer 1 / Layer 2 boundary change; emission surface only. |
| **V (Audit-First / G13)** | **PASS — LOAD-BEARING** | This IS the G13 closure for `Diagnostic.message`. PM-C-5 / PM-C-6 detail-loss decisions are constitutional. |
| **VI (Dataflow Pipeline)** | PASS | No phase change. |
| **VII (Crate Discipline)** | **PASS** | PM-C-1 keeps `marque-scheme` as graph leaf — bridge conversion preserves the canonical ordering. C touches `crates/{capco,rules,engine,wasm,core}/` only. NO `marque-scheme` edits. |
| **VIII (Citation Fidelity)** | **PASS — LOAD-BEARING** | Typed `Citation` IS the VIII closure. Every migrated citation re-verified against `crates/capco/docs/CAPCO-2016.md` per propagation rule. |

**Reviewer attestation checklist** (carried into 3-reviewer pass):

- [ ] **C1 housekeeping**: `cargo clippy --workspace --all-targets -- -D warnings` exits 0 at C1 boundary
- [ ] **No `marque-scheme` edits** (Constitution VII): `git diff origin/staging -- crates/scheme/src/` shows zero changes (stale forward-pointers at scheme/src/scheme.rs:604,699,734 are LOC changes only — the file does change for comment updates; reviewer verifies they're comment-only)
- [ ] **Closed-template invariant**: no new `Diagnostic.message: format!(...)` calls; compile-fail doctests still passing
- [ ] **Closed-args invariant**: no new `String` / `Vec<u8>` / `&str` fields added to `MessageArgs`
- [ ] **No `Display` for `Message`**: still doesn't exist; compile-fail doctests at `message.rs:506` still passing
- [ ] **No `From<&str> for Citation`**: new compile-fail doctest enforcing this
- [ ] **Constitution V Principle V (G13)**: no runtime byte text flows into `Diagnostic.message`; PM-C-5 / PM-C-6 narrowings documented in PR description
- [ ] **Constitution VIII propagation**: every migrated `Citation` re-verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship (sampled subset ≥10 sites in reviewer pass)
- [ ] **57-site `Diagnostic.message` migration complete**: `grep -rn 'message:\s*format!' crates/capco/src/ crates/engine/src/` returns 0 hits
- [ ] **57-site `Diagnostic.citation` migration complete**: `grep -rn 'citation:\s*"CAPCO' crates/capco/src/rules.rs crates/engine/src/engine.rs` returns 0 hits in `Diagnostic::new` / `Diagnostic::with_fix*` contexts (catalog rows in `crates/scheme/` and internal capco helpers stay `&'static str` per PM-C-1)
- [ ] **Engine R001 + R002 migrated**: `grep -rn 'format!.*decoder-recognized\|format!.*post-pass-1' crates/engine/src/` returns 0 hits
- [ ] **WASM JSON wire-format documented**: PR description explicitly notes the shape change
- [ ] **cfg-gates on s004 + rules_us1 lifted**: both files compile under `cargo test --workspace`; test bodies migrated to new Diagnostic shape
- [ ] **No `__engine_promote` calls outside Constitution V Principle V carve-out**: `grep -rn '__engine_promote' crates/` returns only cfg-gated test sites + the carve-out comment at each
- [ ] **`citation-lint` round-trip test exists** (C-FOLLOWUP-1): integration test at `tools/citation-lint/tests/`
- [ ] **C-FOLLOWUP-3 stale comments updated**: 5 sites at scheme.rs:604,699,734 + marking_scheme_impl.rs:587,674; new text references "a future PR will land the §G.1 Table 4 dispatch body"
- [ ] **Bench-check non-blocking** per master D25.6
- [ ] **"Will we maintain this for 5 years?"** durability standard

---

## 2. Risk register

### R-C1 — `bridge_message` mapping completeness

**Likelihood**: MEDIUM. The bridge converting `ConstraintViolation.message: String → Diagnostic.message: Message` must cover all ~25 active constraint labels. A missed label produces a runtime panic or a fallback that loses information.

**Impact**: Per-label diagnostic disappears or carries wrong template.

**Mitigation**: Implementation agent builds the `constraint_label → (MessageTemplate, MessageArgs)` mapping by exhaustively enumerating registered `Constraint::Custom("...", ...)` sites in `crates/capco/src/`. Reviewer attestation requires the map cover every active constraint label. Add a `#[test] fn bridge_message_covers_every_constraint_label()` smoke test that iterates over `CapcoScheme::constraints()` and asserts each returns `Some(_)` from `bridge_message`.

### R-C2 — `Citation::Display` non-CAPCO prefix breaking existing tools

**Likelihood**: LOW. The Display impl currently doesn't emit a prefix; adding `[config] ` for non-CAPCO sources is additive for new variants. CAPCO citations (the only ones existing before C2) continue to render bare.

**Impact**: If `citation-lint`'s parser doesn't handle the `[<source>] ` prefix, it will reject the new sentinel citations.

**Mitigation**: Update `tools/citation-lint/src/scanner.rs` in C2 atomically with the `AuthoritativeSource::Config` / `EngineInternal` additions. Test that the scanner skips (rather than rejects) the prefix.

### R-C3 — Lifted cfg-gated tests need MessageTemplate variants that don't exist

**Likelihood**: LOW. The s004 test asserts that `Diagnostic.message` doesn't carry document bytes. Under the migration, every variant in `MessageTemplate` is by construction document-byte-free (the closed set is the invariant).

**Impact**: If a test references a hypothetical template like `AmbiguousCountryCode` for S004's assertion, and that variant doesn't exist in the current closed set, the cfg-gate lift will fail to compile.

**Mitigation**: When lifting the cfg-gate, the implementation agent checks each `assert!()` against the closed `MessageTemplate` variants. If a variant is missing, it MAY be added in C2 (per PM-C-3 — additions during `marque-mvp-3` are permitted). Variant additions are listed in the PR description.

### R-C4 — `render_message` consumer-side absence

**Likelihood**: HIGH. The CLI, the WASM, and the audit emit layer all currently use `format!("{}", d.message)` style rendering against the `Box<str>` shape. Post-C, `Message` has no `Display`. Any call site missed in C5 fails to compile.

**Impact**: Compilation failures cascade across the workspace.

**Mitigation**: Pre-C5 grep — `grep -rn '\.message' crates/engine/src/ crates/wasm/src/ --include='*.rs' | grep -v '//\|push\|set\|field'` to enumerate all consumer sites. The implementation agent verifies each is updated to use `template()` + `args()` accessors (or to construct a `render_message(template, args, source, span)` helper in the appropriate consumer crate).

The 3 known render call sites today (per PM-measured grep):
- `crates/wasm/src/lib.rs:380` — WASM JSON emit; updates to structured shape per PM-C-7
- `crates/engine/src/engine.rs:6745` — test site; assertion updates to `args()` accessors
- `crates/engine/src/engine.rs:6760` — test site; same

### R-C5 — 57-site citation re-verification scale

**Likelihood**: HIGH (per architect preflight R-5 — citation drift is empirically the dominant marque failure mode).

**Impact**: ~3-5 sites statistically may not match CAPCO-2016.md exactly.

**Mitigation**: Implementation agent runs every migrated `capco(L, sub, page)` / `capco_table(L, sub, table, page)` call against the `crates/capco/docs/CAPCO-2016_citation_index.yml` lookup. Reviewer-pass spot-checks ≥10 sites against the manual. Citation discipline holds per Constitution VIII.

### R-C6 — clippy fan-out from new code

**Likelihood**: MEDIUM. C5's ~57 new `capco(SectionLetter::H, 4, 61)` constructions plus ~57 `Message::new(MessageTemplate::X, MessageArgs { ... })` constructions trip lints like `clippy::redundant_field_names`, `clippy::needless_update`, etc.

**Impact**: CI clippy gate fails on the C5 commit.

**Mitigation**: Run `cargo +stable clippy --workspace --all-targets -- -D warnings` after C5 locally before pushing. Address per-commit per `feedback_clippy_nightly_vs_stable_drift` memory.

### R-C7 — Hidden `ConstraintViolation.message: String` consumers

**Likelihood**: LOW. The bridge in `rules_declarative.rs` is the main consumer. Other consumers (if any) would also need updating.

**Impact**: Build break if a consumer expects `String` but bridge changes return type.

**Mitigation**: Per PM-C-1, `ConstraintViolation.message: String` STAYS unchanged. Bridge constructs `Diagnostic.message: Message` from the `String` at conversion time but doesn't change the source struct. No consumers are affected.

---

## 3. Implementation agent brief

The implementation agent receives:

1. This PM contract (binding scope; PM-C-1 through PM-C-10)
2. The architect preflight (architectural framing; note the architect's OQ-C7 / `citation!()` macro recommendations were superseded by PM-C-1 / PM-C-2 per rust-preflight findings)
3. The rust-specialist preflight (tactical mechanics; load-bearing for the migration shape)
4. Full content of `crates/capco/CAPCO-CONTEXT.md` (not linked — embedded in brief)
5. Standing constraints:
   - All PRs against `staging`
   - No `--no-gpg-sign` / `--no-verify` / `git push --force` without explicit user authorization
   - Citations cite `CAPCO-2016 §X.Y pNN` form only; Constitution VIII propagation rule
   - Constitution V Principle V: `AppliedFix::__engine_promote` is engine-only
   - Constitution VII: PR 3c.2.C touches `crates/{capco,rules,engine,wasm,core}/`; no `marque-scheme` edits; no scheme adoption
   - >80% test coverage per standing brief; if CodeCov denies, expand suite
   - "Will we maintain this for 5 years?" durability standard
   - Walk adjacent code paths — if a fix surfaces in one place, check related callsites for the same issue (this is the failure mode that bit B7 — Copilot caught 3 of 12 PM-B-3 second-clause violations that the original 3-reviewer pass missed)

6. Specific must-do items:
   - C1 first; clippy gate must clear before C2 starts
   - PM-C-1 bridge approach is binding (no `marque-scheme` edits)
   - PM-C-2 `capco()` helper, NOT a macro
   - PM-C-3 cfg-gate lift on s004 + rules_us1
   - PM-C-4 add both Config and EngineInternal AuthoritativeSource variants
   - PM-C-5 + PM-C-6 — accept the byte-content drop; renderer responsibility shifts
   - PM-C-7 — WASM JSON wire-format change documented
   - PM-C-8 — 6-commit sequence
   - PM-C-9 — tests added per the list
   - PM-C-10 — reviewer attestation checklist must pass at PR submission

Once C5 lands, run a comprehensive `cargo test --workspace` AND `cargo clippy --workspace --all-targets -- -D warnings` before opening the PR.

---

## 4. Sub-PR cadence

Standard 5-stage cycle:
1. ✅ Preflight (architect + rust-specialist) — COMPLETE
2. ✅ Resolve decision points — COMPLETE (this document)
3. Implementation agent — NEXT
4. 3-reviewer pass (rust-reviewer + code-reviewer + architect or lattice-consultant)
5. Submit PR + monitor Copilot feedback (2-5 rounds typical)

---

**Locked**: 2026-05-20.
**Implementation agent unblocked**: YES (PM contract complete).
