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
Per PM-D-7 / PM-D-11 the v2 shape is flat `AppliedReplacement<S>`
(no enum discriminator on the struct — the discriminant is derived
at audit-emit time from `FixSource`), `SystemTime` (not
`DateTime<Utc>`), `Option<Arc<str>>` for classifier identity (not
a `ClassifierId` newtype), and includes `source: FixSource` at the
top level.

```rust
pub struct AppliedFix<S: MarkingScheme> {
    pub rule: RuleId,
    pub severity: Severity,                         // top-level snapshot from Diagnostic.severity (PM-D-11)
    pub span: Span,
    pub fix: AppliedFixDetail<S>,
    pub source: FixSource,                          // recognizer provenance (PM-D-7 emit-time discriminant input)
    pub message: Message,                           // template + args; FR-003
    pub timestamp: SystemTime,                      // not DateTime<Utc>
    pub classifier_id: Option<Arc<str>>,            // not a ClassifierId newtype
    pub dry_run: bool,
    pub input: Option<Arc<str>>,                    // caller-supplied input identifier
}

pub struct AppliedFixDetail<S: MarkingScheme> {
    pub replacement: AppliedReplacement<S>,         // flat struct; no Strict/Decoder enum
    pub original_span: Span,                        // FR-004: span only, no bytes
    pub original_digest: Blake3Hash,                // BLAKE3 of pre-fix bytes
}

pub struct AppliedReplacement<S: MarkingScheme> {
    pub canonical: Canonical<S>,                    // engine-rendered canonical payload
    pub confidence: Confidence,                     // confidence snapshot
    pub bytes_digest: Blake3Hash,                   // BLAKE3 of canonical.bytes() (PM-D-6, precomputed at promotion)
}

/// Derived at audit-emit time from `AppliedFix.source` via the 5-to-2 collapse
/// in `discriminant_from_source` (PM-D-7). NOT stored on `AppliedReplacement`.
pub enum Discriminant { Strict, Decoder }

pub struct RuleId(pub &'static str /* predicate_id */);
```

**Validation rules**:
- `AppliedFix::__engine_promote(...)` is `pub #[doc(hidden)]` and reachable only from `Engine::fix_inner` in production code (FR-005). Test-fixture carve-out per Constitution V Principle V remains in effect; promote-callsite lint (FR-040, AST-based) enforces.
- The FR-040 lint's reserved-name list at HEAD: `__engine_promote`, `__engine_promote_text_correction` (PM-D-4 text-correction split; PR 3c.2.D fixup F-1 added the lint coverage), `__engine_construct`. Exact-equality match on the last path segment — back-compat names like `__engine_promote_legacy` are deliberately NOT covered.
- `AppliedFix.fix.replacement` carries `Canonical<S>` post-PR-3c.2.D (per FR-035a). The `Canonical<S>` already encodes provenance (CVE-typed vs. open-vocab-typed); `Discriminant::Strict | Decoder` (derived at emit time from `AppliedFix.source` via PM-D-7's 5-to-2 collapse) tracks the recognizer that produced the fix. The discriminant is NOT a struct field on `AppliedReplacement<S>` — it's projected at JSON emit time.
- `AppliedFix.message` is `Message` (template + args) — FR-003 fully closed at PR 3c.2.D. The template carries the `MessageTemplate` variant name verbatim via `as_str()`; the args object holds the closed-set permitted-identifier types (`token`, `expected_token`, `actual_token`, `category`, `span`, `digest`, `confidence`, `feature_ids`).
- `RuleId` is the 1-tuple `(&'static str)` form through `marque-1.0`; the 2-tuple `(scheme, predicate-id)` migration is post-PR-10 per FR-049 (the stability freeze begins at PR 10 merge; the 2-tuple change requires the freeze to be unfrozen).
- Pre-cutover (`marque-mvp-3`) records are not interoperable with `marque-1.0` binaries (FR-037 — clean break, no `marque-audit-reader` crate scheduled); single-value `MARQUE_AUDIT_SCHEMA` validation at build time (FR-034). (`marque-mvp-1` / `marque-mvp-2` retired in earlier PRs.)
- Non-marking text corrections (C001 corrections-map matches, the E006-shaped deprecation path) flow through a separate `AppliedTextCorrection` type carrying a corpus-derived `SmolStr` replacement (PM-D-4). The two types are disjoint by construction; G13 boundary is type-level checkable.

**Serialization**: NDJSON. Schema field is `"schema": "marque-1.0"` per FR-035 / FR-035a. The exact JSON shape is documented in `contracts/audit-record.md` §"NDJSON record shape".

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
    /// `ctx.emission_form` selects the form (Auto / Portion / Banner /
    /// BannerAbbreviated / LongTitle) — see FR-052 and the
    /// `RenderContext` section below.
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

## `RenderContext` and `EmissionForm` (PR 3c.2, FR-052)

Engine-supplied context to `MarkingScheme::render_canonical`. The `RenderContext`
shape lands alongside `render_canonical`'s implementation in PR 3c.2 (T048 +
T048a). Locking the shape now is the only marking-form-handling change worth
pulling into the keystone window: `render_canonical` is the single open-vocab
emission path, and any future recognize-A-emit-B work (ISM-XML output,
schema-compliance round-trip, grammar-bridge adapters) flows through this
context. Adding `emission_form` later means re-signing `render_canonical`
and propagating into every `FixIntent` emission site across the rule
catalog — a much larger blast radius than locking it now.

```rust
pub struct RenderContext {
    /// Existing — Page vs Portion (already in PR 3.7 / Phase B surface).
    pub scope: Scope,

    /// NEW (FR-052) — explicit emission-form selector. Engine populates
    /// from rule context (defaulting to Auto); scheme honors it in the
    /// closed-CVE branch by routing to the matching FormSet field.
    pub emission_form: EmissionForm,

    /// NEW — schema version this render targets. Reserved for future
    /// codec / grammar-bridge routing. Defaults to the active scheme's
    /// pinned ism-schema-version.
    pub schema_version: SchemaVersionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmissionForm {
    /// Derive from Scope: Page → BannerAbbreviation if present else
    /// BannerTitle; Portion → Portion. Preserves pre-3c.2 emission
    /// behavior — matches the existing `Vocabulary::banner_form()`
    /// abbreviation-when-distinct semantics. Every existing FixIntent
    /// emission site uses Auto and is unaffected by 3c.2.
    Auto,
    /// Force portion form — CAPCO §G.1 Table 4 column 3 "Authorized
    /// Portion Mark". Always present per token. Examples: "S" for
    /// SECRET, "NF" for NOFORN.
    Portion,
    /// Force banner-title form — CAPCO §G.1 Table 4 column 1
    /// "Authorized Banner Line Marking Title". Always present per token.
    /// Examples: "SECRET" for SECRET (no distinct abbreviation exists),
    /// "NOT RELEASABLE TO FOREIGN NATIONALS" for NOFORN.
    BannerTitle,
    /// Force banner-abbreviation form — CAPCO §G.1 Table 4 column 2
    /// "Authorized Banner Line Abbreviation". Distinct from BannerTitle
    /// only when one exists. Examples: "NOFORN" for NOFORN (the
    /// abbreviation is distinct from the title); SECRET has no distinct
    /// abbreviation, so this falls back to BannerTitle ("SECRET") per
    /// the validation rule below.
    BannerAbbreviation,
}
```

**Validation rules (FR-052)**:
- `EmissionForm` is `#[non_exhaustive]` — adding a future variant
  (`IsmDescriptionTitle` for ISM-XML output; `XmlAttribute`; scheme-
  specific forms) MUST NOT require touching `FixIntent` emission sites
  that pass `EmissionForm::Auto`. Existing rules that don't care about
  form pass `Auto` and inherit the scope-derived default forever.
- The closed-CVE branch of `render_canonical` MUST honor every variant
  by routing to the matching `FormSet` field via `Vocabulary<S>::forms()`.
  When `EmissionForm::BannerAbbreviation` is requested but
  `forms().banner_abbreviation == None` (the token has no distinct
  abbreviation, e.g., classifications, FISA, RELIDO), the branch MUST
  return `forms().banner_title` rather than panicking. `BannerTitle` and
  `Portion` are total — every token has one — so no fallback rule is
  needed for those variants.
- The open-vocab branch (rendering an SCI compartment, an FGI trigraph
  list, etc.) ignores `emission_form` for now — open-vocab tokens carry
  one canonical per scope. Future scheme-specific variants of
  `EmissionForm` may extend this.
- `schema_version` is reserved data plumbing — no consumer in 3c.2.
  Wired by the engine to the active scheme's pinned schema version
  (`marque-ism/Cargo.toml [package.metadata.marque] ism-schema-version`).

**Used by**: `MarkingScheme::render_canonical` (the only path that reads
`ctx`); `Engine::fix_inner` (the only constructor of `RenderContext`).

---

## `Vocabulary<S>` extensions (PR 2 / PR 3d / PR 4)

Existing `Vocabulary<S>` (Phase 5 metadata surface) gains methods across three
PRs — all additive, no signature changes to existing methods:

```rust
pub trait Vocabulary<S: MarkingScheme>: Send + Sync + 'static {
    /* existing Phase-5 methods: lookup, authority, owner, deprecation,
       urn, schema_version */

    /// PR 3d — aggregate per-token form data. The three canonical CAPCO
    /// forms (Portion Mark, Banner Line Marking Title, Banner Line
    /// Abbreviation per §G.1 Table 4) plus a recognize-only alias slice
    /// for forms accepted on input but not emitted by default. Replaces
    /// the per-form-method triple as the authoritative accessor; existing
    /// per-form methods become default methods over `forms()`.
    fn forms(&self, token: &S::Token) -> &'static FormSet;

    /// PR 3d — default-method accessors over `forms()`. Existing call
    /// sites are unaffected. Schemes that already provide bespoke
    /// `forms()` impls inherit these for free.
    fn portion_form(&self, token: &S::Token) -> &'static str {
        self.forms(token).portion
    }
    /// Returns the banner-abbreviation form when one is distinct, else
    /// the banner-title form. Preserves the pre-3d semantics of
    /// `banner_form()` byte-for-byte (matches the existing
    /// `MarkingForm.banner` field's "abbreviation when distinct, else
    /// title" convention in `crates/ism/src/marking_forms.rs`).
    fn banner_form(&self, token: &S::Token) -> &'static str {
        let f = self.forms(token);
        f.banner_abbreviation.unwrap_or(f.banner_title)
    }
    fn banner_abbreviation(&self, token: &S::Token) -> Option<&'static str> {
        self.forms(token).banner_abbreviation
    }

    /// PR 2 / FR-015 — admit input bytes against a category's structural
    /// shape. Used by parser (replacing inline byte-class checks at the
    /// four open-vocabulary admission sites — three `is_ascii_alphanumeric`
    /// checks plus the FGI trigraph silent-skip) and by open-vocab
    /// Canonical construction.
    fn shape_admits(category: CategoryId, bytes: &[u8]) -> bool;

    /// PR 4 / FR-010 — per-token classification of dissem markings as
    /// foreign-disclosure-and-release (FD&R) or non-FD&R (CAPCO §H.8).
    /// Returns false for non-dissem categories.
    fn is_fdr_dissem(token: TokenId) -> bool;
}
```

**Validation rules**:
- `shape_admits` is total over `(CategoryId, &[u8])`; categories with no structural shape rule (closed-CVE-only) return `lookup(bytes).is_some()`.
- `is_fdr_dissem` baked from `crates/capco/docs/CAPCO-2016.md` §H.8 at build time (per the Phase 5 metadata surface mechanism). PR 4 wires `SupersessionSet` over the dissem axis to the predicate.
- `forms()` MUST return `&'static FormSet` data (no runtime allocation; SC-008 invariant). The CAPCO impl reuses the existing `marking_forms.rs` static table plus build-time-derived per-token records.
- The default per-form accessor methods MUST NOT be overridden when a scheme implements `forms()` — the trait shape relies on round-trip equality between `forms(token).portion` and `portion_form(token)` for back-compat.

---

## `FormSet` and `FormKind` (PR 3d, FR-053)

Per-token aggregation of every form a scheme recognizes (input) or emits
(output). Replaces the closed-world "exactly three forms per token"
assumption baked into the Phase 5 per-form methods. Lands in
`crates/scheme/src/vocabulary.rs`; CAPCO build-time impl lands in
`crates/capco/src/vocabulary.rs` and `crates/ism/src/marking_forms.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormSet {
    /// CAPCO §G.1 Table 4 column 3 — "Authorized Portion Mark".
    /// Always present per token. Examples: "S" for SECRET, "NF" for
    /// NOFORN, "FOUO" for FOR OFFICIAL USE ONLY.
    pub portion: &'static str,
    /// CAPCO §G.1 Table 4 column 1 — "Authorized Banner Line Marking
    /// Title". Always present per token. Examples: "SECRET" for SECRET
    /// (no distinct abbreviation), "NOT RELEASABLE TO FOREIGN NATIONALS"
    /// for NOFORN (the long descriptive title; abbreviation lives in the
    /// next field), "FOR OFFICIAL USE ONLY" for FOUO.
    pub banner_title: &'static str,
    /// CAPCO §G.1 Table 4 column 2 — "Authorized Banner Line
    /// Abbreviation". `Some` only when the abbreviation is distinct from
    /// `banner_title`. Examples: `Some("NOFORN")` for NOFORN,
    /// `Some("FOUO")` for FOR OFFICIAL USE ONLY, `None` for SECRET (no
    /// abbreviation form exists for any classification marking).
    pub banner_abbreviation: Option<&'static str>,
    /// Forms recognized on input but NOT emitted by default.
    /// Populated for ISM `Description.title` when it differs from
    /// CAPCO `banner_title` (per build-time ODNI XML harvest); also
    /// historical aliases pre-dating the current schema. Engine policy
    /// — not data shape — decides whether any alias may be promoted to
    /// emission.
    pub recognized_aliases: &'static [(FormKind, &'static str)],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormKind {
    /// ODNI ISM CVE `Description.title` form, when the published title
    /// disagrees with CAPCO's `banner_title` (typically the ISM form is
    /// anachronistic). Recognize-only by default.
    IsmDescriptionTitle,
    /// Pre-dated alias from a prior CAPCO revision. Recognize-only.
    HistoricalAlias,
    /* future: NatoTitle, FgiTrigraphFullName, scheme-specific kinds */
}
```

**Validation rules (FR-053)**:
- `FormSet` is constructed only at build time; runtime code holds
  `&'static FormSet`. No public `FormSet::new` — the only construction
  path is the macro / generator that the build script emits.
- `FormKind` is `#[non_exhaustive]`. Promoting a recognize-only
  `FormKind` variant to first-class emission requires:
  (a) adding the matching `EmissionForm` variant (FR-052 — additive);
  (b) extending `render_canonical`'s closed-CVE branch to dispatch on it.
  No `FormSet` shape change is required.
- `recognized_aliases` MUST contain only forms that genuinely *do* differ
  from the three canonical fields (`portion`, `banner_title`,
  `banner_abbreviation`). The build-time generator MUST elide duplicates
  (an ISM `Description.title` that exactly matches the CAPCO
  `banner_title` does not appear).
- Build-time generation rule: when ODNI ISM XML's `<Description>` text
  carries a parseable title that differs from the CAPCO `banner_title`
  (per `crates/capco/docs/CAPCO-2016.md` §G.1 Table 4 column 1), it
  lands in `recognized_aliases` with `FormKind::IsmDescriptionTitle`.
  When the texts agree, the entry is omitted (no information added).

**Used by**: `Vocabulary<S>::forms()` (returns `&'static FormSet`);
`MarkingScheme::canonicalize` (consults `recognized_aliases` during
input normalization — recognizes the alias, normalizes to canonical);
`MarkingScheme::render_canonical` (consults the canonical fields via
`EmissionForm`).

---

## `Deprecation<Token>` validity windows (PR 3d, FR-054)

Extension of the existing `Deprecation<Token>` struct. Adds
schema-version range fields so the data plumbing for "evaluate as
valid for the time" exists before any consumer needs it.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation<Token> {
    /// Existing — schema version at deprecation time.
    pub since: &'static str,

    /// NEW (FR-054) — schema version of first publication. Defaults
    /// to None when build.rs cannot derive it from ODNI XSD annotations.
    pub valid_from: Option<&'static str>,

    /// NEW (FR-054) — schema version after which the token is no
    /// longer valid in newly-authored documents. Defaults to None when
    /// the token has no successor in the migration table (rare —
    /// FOUO-style "no replacement" cases per FR-017).
    pub valid_until: Option<&'static str>,

    /// Existing — replacement token id when one is defined.
    pub replacement: Option<Token>,
}
```

**Validation rules (FR-054)**:
- Both `valid_from` and `valid_until` are `Option<&'static str>` —
  build-time generation populates them when source annotations exist;
  defaults to `None` otherwise. No runtime panic if absent.
- `valid_from <= since` MUST hold when both are populated (a token
  cannot be deprecated before it was published). Build-time test
  asserts the invariant over the generated migration table.
- No PR 3d consumer is required. The validity-window data is the
  prerequisite for a later `RuleContext.evaluation_as_of:
  Option<SchemaVersionId>` flag (post-refactor; tracked separately as
  a follow-on issue).

**Used by**: build-time generation only in PR 3d. Post-refactor:
historical-as-valid evaluation mode in rules; ISM-XML codec
(`Codec<CapcoScheme>`) when it needs to round-trip a document
authored under a prior schema version.

---

## `FgiMarker` discriminant (PR 2, FR-017)

Replaces the current `FgiMarker { countries: Box<[CountryCode]> }` shape that collides
on `countries: []` between lawful source-concealed FGI and parser-failure corruption.

```rust
pub enum FgiMarker {
    /// Lawful per CAPCO §H.7 p123 — FGI without disclosed source country.
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
pub const R002_RULE_ID: RuleId = RuleId("engine", "r002.reparse-failed");

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
Vec<AuditLine>        ◀── FR-002: content-ignorant
   │                     ◀── FR-026: rule-id surface (string form through marque-1.0;
   │                     │           2-tuple post-PR-10 per FR-049)
   │                     ◀── FR-035 / FR-035a: schema "marque-1.0" (active post PR 3c.2.D)
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
