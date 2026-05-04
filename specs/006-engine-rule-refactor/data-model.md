<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Phase 1 Data Model: Engine + Rule Architecture Refactor

**Branch**: `006-engine-rule-refactor` | **Date**: 2026-05-03
**Source**: [spec.md](./spec.md) FR-001..FR-041; [plan.md](./plan.md) Project Structure; [research.md](./research.md) R-1..R-8

This document specifies the type-system shapes for the new and reshaped
entities introduced by the refactor. Each entity links to the FRs it
satisfies and the PR(s) that lands it. Rust signatures are illustrative
sketches; PR-time implementation may refine field names while preserving
the contract.

---

## Pivot type split (PR 3a, FR-006 supporting infrastructure)

The single `IsmAttributes` type currently doing three jobs (parser
output / post-canonical form / page roll-up output, per consolidated
plan §1.1) splits into three distinct types living in `marque-ism`.

### `ParsedAttrs<'src>`

Parser output. Possibly carries degraded / partial structure on
malformed input. Borrows from the input byte buffer via `'src`.

```rust
pub struct ParsedAttrs<'src> {
    pub classification: Option<ParsedClassification<'src>>,
    pub sci_markings: Box<[ParsedSciMarking<'src>]>,
    pub sar_markings: Box<[ParsedSarMarking<'src>]>,
    pub fgi_markers: Box<[ParsedFgiMarker<'src>]>,
    pub dissem_us: Box<[ParsedDissem<'src>]>,
    pub dissem_nato: Box<[ParsedDissem<'src>]>,
    pub aea: Option<ParsedAea<'src>>,
    pub declassify_on: Option<ParsedDeclassifyOn<'src>>,
    pub token_spans: Box<[Span]>,
    pub source_bytes_origin: SourceOrigin,
}
```

Each `Parsed*<'src>` is a thin wrapper that retains the source-bytes
slice for the parsed token. This is what comes out of
`marque-core::parser`.

**Lifecycle**: short-lived. Constructed by the parser; consumed
immediately by canonicalization.

### `CanonicalAttrs`

Post-canonical, owned form. Fields are validated against
`Vocabulary<S>::shape_admits` and rendered through scheme-specific
canonicalization. This is what rules consume.

```rust
pub struct CanonicalAttrs {
    pub classification: Option<MarkingClassification>,   // FR-007: Option, not hardcoded Us
    pub sci_set: SciSet,                                  // existing lattice type
    pub sar_set: SarSet,                                  // existing lattice type
    pub fgi_set: FgiSet,                                  // FR-017 discriminant via FgiMarker variants
    pub dissem_us: Box<[DissemControl]>,
    pub dissem_nato: Box<[DissemControl]>,                // PR 9 (FR-046, #271 / 7B)
    pub aea: Option<AeaControl>,
    pub declassify_on: Option<DeclassifyOn>,
    pub token_spans: Box<[Span]>,
}
```

**Lifecycle**: lives for the duration of a single rule-evaluation pass.
PR 3a's `from_parsed_unchecked(ParsedAttrs<'src>) -> CanonicalAttrs`
adapter (`#[doc(hidden)]`) bridges the gap between PR 3a (pivot split
lands; rules still consume `&CanonicalAttrs` via the adapter) and PR 3c
(adapter deletes; canonicalization is mandatory and explicit).

### `ProjectedMarking`

Output of `MarkingScheme::project(scope, ...)`. For `Scope::Page` this
is what banner-validation rules consume; for `Scope::Portion` it is the
single-portion canonical form.

```rust
pub struct ProjectedMarking {
    pub scope: Scope,
    pub classification: Option<MarkingClassification>,     // FR-007/FR-008: foreign markers preserved
    pub sci_set: SciSet,
    pub sar_set: SarSet,
    pub fgi_set: FgiSet,
    pub dissem_us: Box<[DissemControl]>,
    pub dissem_nato: Box<[DissemControl]>,
    pub aea: Option<AeaControl>,
    pub declassify_on: Option<DeclassifyOn>,
    pub provenance: ProjectionProvenance,                  // which portions contributed; lattice trace
}
```

**Lifecycle**: produced by the scheme's `project` impl; consumed by
banner-validation rules (PR 9 migrates them).

### Adapter: `from_parsed_unchecked` (PR 3a only; deleted at PR 3c)

```rust
#[doc(hidden)]
pub fn from_parsed_unchecked(parsed: ParsedAttrs<'_>) -> CanonicalAttrs { ... }
```

The `#[doc(hidden)]` + `_unchecked` suffix makes it clear this is a
transitional shim. PR 3c deletes it; rules then consume `CanonicalAttrs`
constructed only via the explicit canonicalization path
(`MarkingScheme::canonicalize(ParsedAttrs<'_>) -> CanonicalAttrs` or
equivalent).

---

## `Canonical<S>` (PR 3c, FR-001)

Provenance-tagged canonical replacement. The keystone type for G13
type-invariant closure.

```rust
pub struct Canonical<S: MarkingScheme> {
    bytes: Box<str>,
    source: TokenSource,
    _scheme: PhantomData<S>,
}

enum TokenSource {
    Cve(TokenId),
    OpenVocab {
        category: CategoryId,
        render_call_site: &'static Location<'static>,
    },
}

impl<S: MarkingScheme> Canonical<S> {
    /// CLOSED-CVE PATH (sealed via TokenId, which can only come from
    /// Vocabulary<S>::lookup). The only public closed-vocab constructor.
    pub fn from_cve(token: TokenId, scope: Scope) -> Self { ... }

    /// OPEN-VOCAB PATH. Crate-private to marque-scheme; reachable only
    /// from `MarkingScheme::render_canonical` impls via the sealed
    /// `CanonicalConstructor<S>` trait (R-7).
    pub(crate) fn from_render(
        category: CategoryId,
        bytes: Box<str>,
        scope: Scope,
        site: &'static Location<'static>,
    ) -> Self { ... }

    pub fn bytes(&self) -> &str { &self.bytes }
    pub fn source(&self) -> &TokenSource { &self.source }
    pub fn scope(&self) -> Scope { /* derived from constructor */ }
    pub fn digest(&self) -> Blake3Hash { /* BLAKE3 of bytes; for audit */ }
}
```

**Validation rules (FR-001)**:
- No public `Box<str> → Canonical<S>` constructor MAY exist. (Compile-fail tests at `crates/scheme/tests/canonical_unconstructable.rs` demonstrate this property.)
- The `_scheme: PhantomData<S>` enforces that a `Canonical<CapcoScheme>` cannot be passed where `Canonical<OtherScheme>` is expected.
- `from_cve(TokenId, Scope)` is callable from any crate; `TokenId` itself is constructed only by `Vocabulary<S>::lookup`.
- `from_render(...)` is `pub(crate)` to `marque-scheme` and only invoked from `MarkingScheme::render_canonical` impls (which live in `marque-capco` for now). The sealed `CanonicalConstructor<S>` trait (R-7) is what bridges the crate boundary so external rule crates can request open-vocab rendering without naming `from_render` directly.

**Used by**: `FixIntent<S>` (rule emission), `AppliedFix` v2 (audit
record), `MarkingScheme::render_canonical` (the only path that
constructs open-vocab canonicals).

---

## `FixIntent<S>` (PR 3c, FR-025)

Rule-emission API. Rules emit `FixIntent<S>` values; the engine renders
them through `MarkingScheme::render_canonical` to produce `Canonical<S>`
and promotes to `AppliedFix` in `Engine::fix_inner`.

```rust
pub struct FixIntent<S: MarkingScheme> {
    pub target_span: Span,
    pub replacement: ReplacementIntent<S>,
    pub confidence: Confidence,
    pub feature_ids: SmallVec<[FeatureId; 4]>,
    pub message: Message,
}

pub enum ReplacementIntent<S: MarkingScheme> {
    /// Closed-CVE replacement; engine renders via Canonical::from_cve.
    Cve { token: TokenId, scope: Scope },

    /// Open-vocab replacement; engine renders via the scheme's
    /// render_canonical, which calls Canonical::from_render under the
    /// sealed CanonicalConstructor<S>.
    Render {
        category: CategoryId,
        directive: RenderDirective<S>,    // scheme-specific; e.g., SciMarking { control, comps, sub_comps } for CapcoScheme
        scope: Scope,
    },

    /// Delete the token entirely (used by some rules); audit records
    /// "Canonical: <empty>" with provenance Engine.
    Delete,
}
```

**Validation rules**:
- A rule's `evaluate` method returns `Vec<Diagnostic>`; each `Diagnostic` MAY carry an `Option<FixIntent<S>>`.
- The engine's promotion path (`Engine::fix_inner`) calls `S::render_canonical(intent.replacement, ctx)` to get a `Canonical<S>`, then constructs the `AppliedFix` carrying both the rendered `Canonical<S>` and the original `target_span` + BLAKE3 digest of the pre-fix bytes.
- External rule crates depend on `marque-rules` (which re-exports `FixIntent<S>` from `marque-scheme`) and never construct `Canonical<S>` directly. This is what closes the cross-crate emission story (consolidated plan §8.1, "Cross-crate rule emission").

**Used by**: every rule's emission. Replaces direct `FixProposal::new(...)` construction.

---

## `Phase` (PR 7, FR-021)

Rule-registration tag.

```rust
pub enum Phase {
    Localized,      // FixIntent::target_span MUST be sub-token-only
    WholeMarking,   // FixIntent::target_span MUST cover a full marking
}

pub trait Rule {
    fn id(&self) -> RuleId;
    fn phase(&self) -> Phase;            // FR-021
    fn evaluate(
        &self,
        attrs: &CanonicalAttrs,
        ctx: &RuleContext,
    ) -> Vec<Diagnostic>;
}
```

**Validation rules**:
- Engine enforces at registration: `Phase::Localized` rule's `FixIntent::target_span` is sub-token-only; `Phase::WholeMarking` rule's span covers a full marking. Violation rejects the rule at `Engine::new`.
- A rule needing both phases registers two entries (one per phase) sharing a backend module — no `Phase::Both` escape hatch (FR-021, plan §9.1).
- `Phase::WholeMarking` rules receive `ctx.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>` populated when their span overlaps a pass-1 fix (FR-023, R-4).

---

## `MessageTemplate` / `MessageArgs` (PR 3c, FR-003)

Closed enumeration of stable message templates plus closed-set
permitted arg types. The mechanism that makes audit-record
content-ignorance a *type invariant* rather than a grep firewall.

```rust
pub struct Message {
    template: MessageTemplate,
    args: MessageArgs,
}

pub enum MessageTemplate {
    DecoderRecognized,
    BannerMissingClassification,
    PortionUnknownDissem,
    SciCompartmentInvalid,
    SarProgramOrdering,
    FgiTrigraphInvalid,
    FouoEvictedByClassification,
    FouoEvictedByNonFdrDissem,
    NofornSupersedesRelTo,
    /* ... closed set; PR 3c bakes in the starter set extracted per R-2 */
}

pub struct MessageArgs {
    pub token: Option<TokenId>,
    pub category: Option<CategoryId>,
    pub span: Option<Span>,
    pub digest: Option<Blake3Hash>,
    pub confidence: Option<Confidence>,
    pub feature_ids: SmallVec<[FeatureId; 4]>,
    pub expected_token: Option<TokenId>,
    pub actual_token: Option<TokenId>,
    /* CLOSED SET — adding a field requires audit-schema bump */
}
```

**Validation rules (FR-003)**:
- `Message::new(template: MessageTemplate, args: MessageArgs)` is the only public constructor. There is no `Message::from_string`, no `Message::format(...)`, no `impl From<&str> for Message`.
- `MessageArgs` field set is closed: only the listed permitted types (TokenId, CategoryId, Span, Blake3Hash, Confidence, FeatureId — per Constitution V Principle V) appear. No `String`, no `&str`, no `Vec<u8>`, no input-byte-derived types.
- Rendering a `Message` to display text reads the template and substitutes args via a stable lookup table (no `format!` over input bytes). The display-rendering is a separate concern from audit-record serialization.
- The `engine.rs:1389` `format!("decoder-recognized canonical form: {replacement:?}")` interpolation deletes (FR-003, plan §8.3); replaced by `Message::new(MessageTemplate::DecoderRecognized, MessageArgs { token: Some(token_id), ..MessageArgs::default() })`.

**Used by**: every `Diagnostic`. Replaces direct `format!`-built message strings.

---

## `AppliedFix` v2 (PR 3c, FR-002 / FR-004 / FR-005 / FR-026)

Audit record. Reshaped to carry only content-ignorant identifiers.

```rust
pub struct AppliedFix {
    pub rule: RuleId,                              // (scheme, predicate-id) form per FR-026 / R-3
    pub severity: Severity,
    pub span: Span,
    pub fix: AppliedFixDetail,
    pub message: Message,                           // template + args; FR-003
    pub timestamp: DateTime<Utc>,
    pub classifier_id: Option<ClassifierId>,
    pub dry_run: bool,
}

pub struct AppliedFixDetail {
    pub replacement: FixReplacement,                // discriminant Strict | Decoder
    pub original_span: Span,                        // FR-004: span only, no bytes
    pub original_digest: Blake3Hash,                // BLAKE3 of pre-fix bytes
}

pub enum FixReplacement {
    Strict { canonical: Canonical<CapcoScheme>, confidence: Confidence },
    Decoder { canonical: Canonical<CapcoScheme>, confidence: Confidence },
}

pub struct RuleId(pub &'static str /* scheme */, pub &'static str /* predicate_id */);
```

**Validation rules**:
- `AppliedFix::__engine_promote(...)` is `pub #[doc(hidden)]` and reachable only from `Engine::fix_inner` in production code (FR-005). Test-fixture carve-out per Constitution V Principle V remains in effect; promote-callsite lint (FR-040, AST-based) enforces.
- `AppliedFix.fix.replacement` carries `Canonical<S>`; the `Canonical<S>` already encodes provenance (CVE-typed vs. open-vocab-typed). `FixReplacement::Strict | Decoder` discriminant tracks the recognizer that produced the fix.
- `AppliedFix.message` is `Message` (template + args), never a free-form string (FR-003).
- `RuleId` is `(scheme, predicate-id)` form; legacy `E###`/`W###`/`S###`/`C###` IDs do not appear in v2 records (FR-026, R-3).
- Pre-cutover (`marque-mvp-2`) records are unreadable by post-cutover binaries (FR-037); single-value `MARQUE_AUDIT_SCHEMA` validation at build time (FR-034).

**Serialization**: NDJSON. Schema field is `"schema": "marque-1.0"` (FR-035). The exact JSON shape is documented in `contracts/audit-record.md`.

---

## `Diagnostic` v2 (PR 3c, FR-003)

Surface output for the lint phase.

```rust
pub struct Diagnostic {
    pub rule: RuleId,
    pub severity: Severity,
    pub span: Span,
    pub message: Message,                           // template + args
    pub citation: Citation,                         // §X.Y pNN form, lint-validated (FR-018)
    pub fix: Option<FixIntent<CapcoScheme>>,        // rules emit FixIntent (FR-025)
}

pub struct Citation {
    pub section: SectionRef,                        // e.g., A.6, H.8 (parsed structure, not raw string)
    pub page: PageNumber,                           // verified against vendored source (FR-018)
    pub document: AuthoritativeSource,              // identifies which vendored source
}
```

**Validation rules**:
- `Citation::new(section, page, document)` checks at construction that `section.is_normative()` (CAPCO §A–H only per FR-018), `page` falls within the vendored source's page range, and `(section, page)` resolves to a real passage. Violations are compile-time errors when the citation is `const`-constructed; runtime panics when dynamic. The citation lint (FR-018, AST-based) catches both.
- `fix: Option<FixIntent<S>>` — rules emit `FixIntent`, not `FixProposal`. The engine renders + promotes (FR-025).

---

## `MarkingScheme` trait extensions (PR 3c)

Existing `MarkingScheme` trait gains:

```rust
pub trait MarkingScheme: Send + Sync + 'static {
    /* existing items */
    type Marking: /* ... */;

    /// Render an open-vocab fix replacement to Canonical<Self>. The
    /// only call path that produces an open-vocab Canonical<Self>.
    fn render_canonical<C: CanonicalConstructor<Self>>(
        intent: &FixIntent<Self>,
        ctx: &RenderContext,
    ) -> Canonical<Self>;

    /// Existing in Phase B; Scope::Page projection becomes load-bearing
    /// at PR 6 cutover (FR-006).
    fn project(scope: Scope, parts: &[Self::Marking]) -> ProjectedMarking;

    /// FR-043: the single explicit trait-method canonicalization path.
    /// PR 3a's `from_parsed_unchecked` adapter is `#[doc(hidden)]` and
    /// transitional; PR 3c deletes the adapter, leaving this trait
    /// method as the sole `ParsedAttrs<'_> → CanonicalAttrs` constructor.
    /// No public `ParsedAttrs → CanonicalAttrs` path may exist outside
    /// the trait. Canonicalization is a scheme decision; rule crates do
    /// not own it.
    fn canonicalize(parsed: ParsedAttrs<'_>) -> CanonicalAttrs;
}

pub trait CanonicalConstructor<S: MarkingScheme>: sealed::Sealed {
    fn build_open_vocab(category: CategoryId, bytes: Box<str>, scope: Scope) -> Canonical<S>;
}
```

**Validation rules**:
- `Send + Sync + 'static` bound on `MarkingScheme` is required for `BatchEngine` correctness (Constitution VI). PR 0's `static_assertions::assert_impl_all!` catches violation.
- `render_canonical` is generic over the sealed `CanonicalConstructor<S>` so the engine can pass its private `EngineConstructor<S>` impl. External rule crates can call `render_canonical` (the engine does, on their behalf during promotion) but cannot supply their own `CanonicalConstructor<S>`.

---

## `Vocabulary<S>` extensions (PR 2 / PR 4)

Existing `Vocabulary<S>` (Phase 5 metadata surface) gains:

```rust
pub trait Vocabulary<S: MarkingScheme>: Send + Sync + 'static {
    /* existing: lookup, authority, owner, deprecation, urn, schema_version, portion_form, banner_form */

    /// FR-015 — admit input bytes against a category's structural shape.
    /// Used by parser (replacing inline byte-class checks at the four
    /// open-vocabulary admission sites — three `is_ascii_alphanumeric`
    /// checks plus the FGI trigraph silent-skip) and by open-vocab
    /// Canonical construction.
    fn shape_admits(category: CategoryId, bytes: &[u8]) -> bool;

    /// FR-010 — per-token classification of dissem markings as
    /// foreign-disclosure-and-release (FD&R) or non-FD&R (CAPCO §H.8).
    /// Returns false for non-dissem categories.
    fn is_fdr_dissem(token: TokenId) -> bool;
}
```

**Validation rules**:
- `shape_admits` is total over `(CategoryId, &[u8])`; categories with no structural shape rule (closed-CVE-only) return `lookup(bytes).is_some()`.
- `is_fdr_dissem` baked from `crates/capco/docs/CAPCO-2016.md` §H.8 at build time (per the Phase 5 metadata surface mechanism). PR 4 wires `SupersessionSet` over the dissem axis to the predicate.

---

## `FgiMarker` discriminant (PR 2, FR-017)

Replaces the current `FgiMarker { countries: Box<[CountryCode]> }` shape that collides
on `countries: []` between lawful source-concealed FGI and parser-failure corruption.

```rust
pub enum FgiMarker {
    /// Lawful per CAPCO §H.7 p126 — FGI without disclosed source country.
    SourceConcealed,

    /// One or more validated country trigraphs (CAPCO §H.7).
    Acknowledged { countries: SmallVec<[CountryCode; 4]> },
}
```

**Validation rules**:
- Constructor for `Acknowledged` requires `countries.len() >= 1`; the `[]` case is unrepresentable.
- `parse_fgi_marker` returns `None` when post-prefix bytes fail `Vocabulary<S>::shape_admits` (FR-016) — does NOT return `Some(SourceConcealed)` as a fallback. SourceConcealed is only constructed when the input genuinely carries no trigraphs in the lawful position.
- Rules previously matching `countries.is_empty()` audit-migrate to matching the variant explicitly: `FgiMarker::SourceConcealed => ...` vs. `FgiMarker::Acknowledged { countries } => ...`.

---

## `R002` synthetic engine diagnostic (PR 7, FR-024)

Engine-minted diagnostic class for two-pass re-parse failure.

```rust
// In marque-engine (FR-041 — engine mints, not rule crate)
pub const R002_RULE_ID: RuleId = RuleId("capco", "engine.r002.reparse-failed");

pub struct R002Diagnostic {
    pub contributing_pass1_fix_ids: SmallVec<[RuleId; 4]>,
    pub failure_span: Option<Span>,                   // where parse failed in post-pass-1 buffer
    pub message: Message,                              // MessageTemplate::ReparseFailed { ... }
}
```

**Validation rules**:
- Emitted only when `parse(post_pass_1_buffer)` fails inside `Engine::fix_inner`. Plan §9.4.
- Pass-1 `AppliedFix` records remain in the audit log (the fixes happened); R002 carries the contributing IDs so an auditor can see which pass-1 fix(es) led to the unparseable state.
- Pass-2 does not run after R002 emits; document state is the pass-1 buffer.
- A `MessageTemplate::ReparseFailed` variant lands in the PR 3c starter set (R-2) with reserved space for PR 7's filling — alongside `FeatureId::PrecedingFixPenalty` reserved for PR 7's E003 confidence work (FR-035).

---

## Relationships & lifecycle

```text
Input bytes (&[u8])
   │
   ▼ marque-core::scanner (memchr SIMD)
SpanStream (zero-alloc Span values)
   │
   ▼ marque-core::parser (aho-corasick)
ParsedAttrs<'src>     ◀── FR-015: shape_admits at four sites
   │                     ◀── FR-016: returns None on shape failure (no degraded Some)
   │                     ◀── FR-017: FgiMarker::SourceConcealed | Acknowledged
   ▼ MarkingScheme::canonicalize  (PR 3a; explicit at PR 3c)
CanonicalAttrs        ◀── FR-007: classification: Option<MarkingClassification>
   │
   ▼ Rule::evaluate(&CanonicalAttrs, &RuleContext)  ◀── FR-021: phase-tagged
Vec<Diagnostic { fix: Option<FixIntent<S>> }>       ◀── FR-025: rules emit Intent
   │
   ▼ Engine::fix_inner (only path that promotes)
   │   - filter by Confidence::combined() ≥ threshold
   │   - sort + non-overlap (C-1, I-3)
   │   - render FixIntent → Canonical<S> via S::render_canonical
   │   - construct AppliedFix via __engine_promote
Vec<AppliedFix>       ◀── FR-002: content-ignorant
   │                     ◀── FR-026: (scheme, predicate-id) RuleId
   │                     ◀── FR-035: schema "marque-1.0"
   ▼ NDJSON serializer
Audit log                                      ◀── SC-001: canary scan finds zero input bytes

# Page-level rollup (FR-006):
[CanonicalAttrs from each portion on a page]
   │
   ▼ MarkingScheme::project(Scope::Page, ...)   ◀── FR-006: replaces PageContext
ProjectedMarking      ◀── FR-008: FGI marker preserved
   │
   ▼ Banner-validation rules (PR 9 migration)
Vec<Diagnostic>
```

---

## Validation rule coverage matrix

| Entity | FRs satisfied |
|---|---|
| `ParsedAttrs<'src>` | FR-015, FR-016, FR-017, FR-045, FR-046 |
| `CanonicalAttrs` | FR-007, FR-046 |
| `ProjectedMarking` | FR-006, FR-008, FR-009, FR-046 |
| `Canonical<S>` | FR-001, FR-027 (closure point) |
| `FixIntent<S>` | FR-025 |
| `Phase` | FR-021, FR-022, FR-023 |
| `Severity::Suggest` | FR-022, FR-042 |
| `Message` / `MessageTemplate` / `MessageArgs` | FR-003 |
| `AppliedFix` v2 | FR-002, FR-004, FR-005, FR-026, FR-034, FR-035 |
| `Diagnostic` v2 | FR-003, FR-018 (via `Citation`) |
| `MarkingScheme` extensions | FR-006, FR-014, FR-043 (`canonicalize`) |
| `Vocabulary<S>` extensions | FR-010, FR-015, FR-047 (NATO tokens via build-time gen) |
| `FgiMarker` discriminant | FR-017 |
| `R002Diagnostic` | FR-024, FR-041, FR-044 (sentinel scheme) |
| Declarative `Constraint` (NATO-portion-in-US-doc) | FR-048 |

All FR-001..FR-048 from the spec map to a type-system or
trait-surface element above except those that are pure CI / process
discipline (FR-018 lint, FR-019 fixtures, FR-020 preemptive fix,
FR-029..FR-033 perf gates, FR-036 priors-bake, FR-037 absence-check,
FR-038 static-assert, FR-039 masking lint, FR-040 promote lint).
Those are addressed in `contracts/audit-record.md` and
`contracts/engine-pipeline.md` plus the plan's Project Structure.
