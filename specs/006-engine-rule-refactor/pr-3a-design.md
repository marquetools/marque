<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3a Design: Pivot Type Split (KEYSTONE-1)

**Branch**: `006-engine-rule-refactor`
**Source**: `tasks.md` T020–T025; `data-model.md` § Pivot type split; `spec.md` FR-006/FR-007/FR-046; consolidated plan §3.1
**Status**: Approved-for-implementation
**Audience**: implementer-of-record for PR 3a

> **Post-implementation amendments.**
>
> - **T022 dep-graph clarification.** §3.4's `ProjectedMarking::scope`
>   field forces a `marque-ism → marque-scheme` dep edge. The
>   consolidated plan's Appendix D anticipates an
>   `marque-ism` ↔ `marque-scheme` edge as part of the keystone-window
>   dep-graph evolution; both crates remain WASM-safe; the graph stays
>   acyclic because `marque-scheme` does NOT depend on `marque-ism`
>   (it remains a domain-neutral trait surface). An earlier deferral of
>   T022 was reverted: the edge was always anticipated, and shipping
>   `ProjectedMarking` at PR 3a gives PR 5/6 the stable target the
>   design intended. Constitution VII's literal "peer leaf" wording
>   describes the pre-refactor shape; the plan supersedes for the
>   refactor itself.
> - **§3.1 wrapper count corrected: 9, not 8.** A
>   `ParsedRelToEntry<'src>` wrapper for REL TO country trigraphs is
>   required and was implemented; it was omitted from the §3.1 listing.
>   The full set is `ParsedClassification`, `ParsedSciMarking`,
>   `ParsedSarMarking`, `ParsedFgiMarker`, `ParsedDissem`,
>   `ParsedNonIcDissem`, `ParsedRelToEntry`, `ParsedDeclassifyOn`, and
>   `ParsedAea` — all `#[non_exhaustive]` with `pub fn new(value, bytes,
>   span)` constructors so the parser (in `marque-core`) can construct
>   them across the crate boundary.
> - **§3.1/§3.2 `sar_markings` plural retained.** The design said
>   `sar_marking` (singular) but the existing `IsmAttributes` field is
>   `sar_markings` (plural). Per Decision #5 ("retain existing field
>   names"), implementation kept plural. Shape narrowing to singular,
>   if desired, is PR 3c's job.
> - **WASM call sites of `from_parsed_unchecked` retained.** Two
>   production WASM helpers (`compute_banner_native`,
>   `generate_cab_native` in `crates/wasm/src/lib.rs`) call
>   `from_parsed_unchecked` directly. They are documented as "Does NOT
>   run the rules engine" — banner/CAB generation outside the rule
>   pipeline. The "engine-owned adapter" principle is a guideline, not
>   an architectural requirement; the WASM sites are explicit known
>   exceptions and carry inline carve-out comments naming PR 3c as the
>   migration target. FR-040 lint whitelists the call sites.

PR 3a is structural-rename + transitional-adapter. **No rule semantics
change. No discriminant change. No schema bump. No new fields.** Output
is byte-identical to `main` on every fixture in every corpus. PR 3a is
independently revertable; CI matrix runs `corpus-regression × {3a-only}`
to enforce that.

The keystone three-PR sequence:
- **PR 3a (this)** — pivot split lands; rules consume `&CanonicalAttrs`
  via the `from_parsed_unchecked` adapter; behavior unchanged.
- **PR 3b** — rule-collapse refactor (#263).
- **PR 3c** — `Canonical<S>` sealing, `MessageTemplate`, audit cutover,
  `from_parsed_unchecked` deletes, `MarkingScheme::canonicalize` becomes
  the only `ParsedAttrs → CanonicalAttrs` path.

---

## 1. Executive summary

Three new types land in `marque-ism`:

- **`ParsedAttrs<'src>`** — borrowed parser output. Carries the
  source-byte slice of every parsed token through eight thin
  `Parsed*<'src>` wrappers, plus existing CAB free-text fields.
- **`CanonicalAttrs`** — the owned, post-canonical form. What rules
  consume. Field shape preserves every distinction `IsmAttributes`
  currently makes; differences are name-only at this PR.
- **`ProjectedMarking`** — the `Scope::Page` projection target. Defined
  but **not wired** at PR 3a; PR 6 cuts over.

A `#[doc(hidden)] pub fn from_parsed_unchecked(ParsedAttrs<'_>) ->
CanonicalAttrs` bridges the gap so `marque-core::parser` can produce
`ParsedAttrs<'src>`, the engine immediately runs the adapter, and rules
consume `&CanonicalAttrs` with no semantic change.

**Most consequential decision**: `CanonicalAttrs` and `ParsedAttrs<'src>`
both keep the entire `IsmAttributes` field surface — the parallel
`sci_controls` + `sci_markings` fields, `Option<SarMarking>` (singular),
`Option<FgiMarker>` (singular), `non_ic_dissem`, `rel_to`, `classified_by`,
`derived_from`, `declass_exemption`, `aea_markings: Box<[AeaMarking]>`,
and `token_spans`. The data-model.md sketch shows a tighter shape
(`sci_set`, `sar_set`, `fgi_set`, no `non_ic_dissem`, etc.); that tighter
shape is **PR 3c's job**, not PR 3a's. PR 3a is a rename+borrow exercise;
shape-narrowing happens after the rule collapse simplifies the consumer
side. Holding the line on "behavior identical" requires keeping the
existing fields. The `dissem_us`/`dissem_nato` split lands at PR 9 per
FR-046 — PR 3a uses a single `dissem_controls` field, matching today.

`IsmAttributes` is **deleted entirely** (clean break, no users). The
`PageContext` field type changes from `Vec<IsmAttributes>` to
`Vec<CanonicalAttrs>`; aggregation logic is unchanged.

---

## 2. Module placement

| New file | Purpose |
|---|---|
| `crates/ism/src/parsed.rs` | `ParsedAttrs<'src>` + 8 thin `Parsed*<'src>` wrappers + `SourceOrigin` |
| `crates/ism/src/canonical.rs` | `CanonicalAttrs` + `from_parsed_unchecked` adapter |
| `crates/ism/src/projected.rs` | `ProjectedMarking` + `ProjectionProvenance` |

The existing `crates/ism/src/attrs.rs` retains all the leaf types
(`MarkingClassification`, `Classification`, `NatoClassification`,
`FgiClassification`, `JointClassification`, `ForeignClassification`,
`AeaMarking`, `RdBlock`, `FrdBlock`, `FgiMarker`, `NonIcDissem`,
`CountryCode`, `SarMarking`, `SarIndicator`, `SarProgram`,
`SarCompartment`, `SciMarking`, `SciControlSystem`, `SciCompartment`,
`TokenSpan`, `TokenKind`, plus generated CVE re-exports). The
`IsmAttributes` struct + its `impl IsmAttributes` block delete from this
file.

`crates/ism/src/lib.rs` re-exports update:

```rust
pub mod canonical;
pub mod parsed;
pub mod projected;

// Replaces the IsmAttributes re-export in the existing public surface:
pub use canonical::{CanonicalAttrs, from_parsed_unchecked};
pub use parsed::{
    ParsedAea, ParsedAttrs, ParsedClassification, ParsedDeclassifyOn,
    ParsedDissem, ParsedFgiMarker, ParsedNonIcDissem, ParsedSarMarking,
    ParsedSciMarking, SourceOrigin,
};
pub use projected::{ProjectedMarking, ProjectionProvenance};
```

`PageContext` stays in `page_context.rs` and switches its inner storage
from `Vec<IsmAttributes>` to `Vec<CanonicalAttrs>`. The `expected_*`
accessors keep their signatures and bodies unchanged (every field they
read has the same name on `CanonicalAttrs` as on `IsmAttributes`).

**Rationale for splitting into three new files** rather than packing
everything into `attrs.rs`: each of the three types is the keystone of
a different downstream PR (3c, 6, 9). Letting them grow in their own
files avoids the "everything-in-attrs.rs" anti-pattern that produced
the 1500-line `attrs.rs` we are unwinding. Per Constitution VII +
common/coding-style.md "many small files > few large files."

---

## 3. Final type signatures

### 3.1 `ParsedAttrs<'src>` and the eight `Parsed*<'src>` wrappers

```rust
// crates/ism/src/parsed.rs
//
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `ParsedAttrs<'src>` — parser output that retains a borrow into the
//! original source bytes for every parsed token.
//!
//! `marque-core::parser` produces a `ParsedAttrs<'src>` per scanner
//! candidate. The engine immediately canonicalizes via
//! `MarkingScheme::canonicalize` (post-PR-3c) or
//! `marque_ism::from_parsed_unchecked` (PR 3a transitional path).
//! Rules consume the resulting `CanonicalAttrs`, never `ParsedAttrs`.
//!
//! # Lifecycle
//!
//! Short-lived. `ParsedAttrs<'src>` exists only between Phase 2 (parser
//! emits) and the immediate canonicalization step. It MUST NOT outlive
//! the input byte buffer it borrows from. Storing one in `RuleContext`,
//! `PageContext`, or any cross-document structure is a misuse — those
//! consumers want `CanonicalAttrs` (owned).
//!
//! # Why a borrowed type at this layer
//!
//! Constitution II ("Zero-Copy, Streaming Core") makes the parser
//! responsible for not duplicating input. `ParsedAttrs<'src>` is the
//! type-level enforcement: every parsed token retains a `&'src str`
//! pointer into the source, so a developer cannot accidentally allocate
//! a `Box<str>` on the parser hot path. The owning `CanonicalAttrs`
//! materializes only when canonicalization is explicitly invoked.

use crate::attrs::{
    AeaMarking, CountryCode, DeclassExemption, DissemControl, FgiMarker,
    MarkingClassification, NonIcDissem, SarMarking, SciControl, SciMarking,
    TokenSpan,
};
use crate::date::IsmDate;
use crate::span::Span;

/// Where in the document the parser ran.
///
/// Threaded onto `ParsedAttrs<'src>` so the canonicalizer (PR 3c) and
/// the engine can route per-origin rule subsets. Today the parser sets
/// this from `MarkingType` (`Banner` / `Portion` / `Cab`); the
/// `PageBreak` variant is unrepresentable here because page-break
/// candidates do not produce `ParsedAttrs` (the parser short-circuits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceOrigin {
    /// `(TS//SI//NF)` style — parenthesized inline marking.
    Portion,
    /// `TOP SECRET//SI//NOFORN` standalone line.
    Banner,
    /// Multi-line CAB block.
    Cab,
}

/// Parser output for one marking candidate.
///
/// Each `Parsed*<'src>` field retains a `&'src str` slice over the
/// source bytes the parser interpreted as that token, so the
/// canonicalizer (PR 3c) can compute round-trip properties (FR-019)
/// without re-borrowing the input. `token_spans` carries the
/// pre-existing per-token span array unchanged.
///
/// `non_ic_dissem`, `rel_to`, `classified_by`, `derived_from`, and
/// `declass_exemption` are owned because their parser output is not a
/// 1:1 byte-slice — `parse_fgi_classification` expands country codes,
/// non-IC parsers normalize abbreviations to enum variants, etc. PR 3c
/// can refine this if a use case appears; PR 3a does not introduce
/// borrows where the parser doesn't already preserve them.
///
/// # Invariants
///
/// - Every populated `Parsed*<'src>` borrows from the same `'src` —
///   the byte buffer the candidate was extracted from. This is a
///   discipline contract, not a type-system bound; the parser is the
///   sole constructor and must enforce it.
/// - `source_bytes_origin` reflects which scanner-emitted candidate
///   produced this `ParsedAttrs`. Page-break candidates do not produce
///   one; the engine short-circuits before reaching the parser.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAttrs<'src> {
    /// US/FGI/NATO/JOINT classification. `None` when the parser failed
    /// to identify a classification (e.g., empty marking content).
    pub classification: Option<ParsedClassification<'src>>,

    /// Structural SCI markings (the source of truth for compartments
    /// + sub-compartments per CAPCO §A.6).
    pub sci_markings: Box<[ParsedSciMarking<'src>]>,

    /// CVE-projection of `sci_markings` when the bare control or
    /// `{ctrl}-{first_comp}` matches a CVE atom and no
    /// sub-compartments are present. Retained verbatim from
    /// `IsmAttributes::sci_controls` because rules currently read it
    /// (CLAUDE.md: "compatibility view scheduled for removal in Phase
    /// C or D"). PR 3a does not remove it.
    pub sci_controls: Box<[SciControl]>,

    /// SAR block, if present. CAPCO §A.6 caps SAR at one block per
    /// marking, so cardinality is `Option`, not `Box<[]>`.
    pub sar_marking: Option<ParsedSarMarking<'src>>,

    /// AEA markings (RD/FRD/CNWDI/SIGMA/UCNI/TFNI) per CAPCO §H.6.
    /// Multiple permitted in one block per §H.6.
    pub aea_markings: Box<[ParsedAea<'src>]>,

    /// FGI marker in a US-classified marking (`FGI` or `FGI [LIST]`)
    /// per CAPCO §H.7. Distinct from `MarkingClassification::Fgi`,
    /// which means the marking IS foreign-classified.
    pub fgi_marker: Option<ParsedFgiMarker<'src>>,

    /// IC dissemination controls (NOFORN, ORCON, RELIDO, FOUO, ...).
    /// Single field at PR 3a; PR 9 (FR-046) splits into `dissem_us`
    /// and `dissem_nato` once the parser tracks separator spans
    /// (#106). Until then, every dissem token lands here.
    pub dissem_controls: Box<[ParsedDissem<'src>]>,

    /// Non-IC dissemination controls (LIMDIS/LES/SBU/SSI/...).
    /// Separate authority framework per CAPCO §H.9 (pp 169–191).
    pub non_ic_dissem: Box<[ParsedNonIcDissem<'src>]>,

    /// REL TO country / country-group codes. USA must be present and
    /// first when the marking targets a US release (E002 enforces).
    /// Each entry retains its source byte slice.
    pub rel_to: Box<[ParsedRelToEntry<'src>]>,

    /// Declassification date (YYYY, YYYYMMDD, or ISO 8601). Holds an
    /// `IsmDate` (typed precision tier) plus the source-bytes slice.
    pub declassify_on: Option<ParsedDeclassifyOn<'src>>,

    /// Free-text "Classified By" identifier from CAB. Borrows from
    /// the source line.
    pub classified_by: Option<&'src str>,

    /// Free-text "Derived From" source from CAB.
    pub derived_from: Option<&'src str>,

    /// Declassification exemption code from CAB (25X1, 50X1-HUM, ...).
    /// CVE enum, no source-byte borrow needed.
    pub declass_exemption: Option<DeclassExemption>,

    /// Per-token byte spans into the original source buffer. Reused
    /// verbatim from `IsmAttributes::token_spans`.
    pub token_spans: Box<[TokenSpan]>,

    /// Which candidate-shape produced this parse. Set by the parser at
    /// `parse_portion` / `parse_banner` / `parse_cab` dispatch; never
    /// `PageBreak` (page-break candidates short-circuit before parsing).
    pub source_bytes_origin: SourceOrigin,
}

// ---------------------------------------------------------------------
// Parsed*<'src> thin wrappers
//
// Each wrapper pairs the parser-produced typed value with the
// source-bytes slice the parser identified as that token. The slice is
// stored as `&'src str` rather than `&'src [u8]` because the parser
// already validated UTF-8 at candidate ingest (per `Span::as_str` +
// `MarkingType::Portion` strip-paren path); deferring re-validation
// here would be wasted work. Slices over byte-buffer-borne CountryCode
// values likewise use `&'src str`.
//
// All wrappers derive `Debug + Clone + PartialEq + Eq` because each
// inner field already does.
// ---------------------------------------------------------------------

/// Classification with its source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedClassification<'src> {
    pub value: MarkingClassification,
    /// Source bytes the parser interpreted as this classification.
    /// E.g., `"TOP SECRET"`, `"S"`, `"COSMIC TOP SECRET-BOHEMIA"`.
    pub bytes: &'src str,
    /// Span of `bytes` within the original source buffer.
    pub span: Span,
}

/// Structural SCI marking + source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSciMarking<'src> {
    pub value: SciMarking,
    /// Source bytes for the full SCI sub-block (e.g., `"SI-G ABCD"`).
    pub bytes: &'src str,
    pub span: Span,
}

/// SAR block + source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSarMarking<'src> {
    pub value: SarMarking,
    /// Full SAR block source (e.g., `"SAR-BP-J12 J54"`).
    pub bytes: &'src str,
    pub span: Span,
}

/// FGI marker + source bytes.
///
/// PR 3a does NOT introduce the `FgiMarker::SourceConcealed |
/// Acknowledged` discriminant — that's PR 2 (FR-017). The current
/// `FgiMarker { countries: Box<[CountryCode]> }` shape is preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFgiMarker<'src> {
    pub value: FgiMarker,
    pub bytes: &'src str,
    pub span: Span,
}

/// One IC dissem control + source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDissem<'src> {
    pub value: DissemControl,
    /// E.g., `"NF"`, `"NOFORN"`, `"OC"`, `"ORCON"`.
    pub bytes: &'src str,
    pub span: Span,
}

/// One non-IC dissem control + source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNonIcDissem<'src> {
    pub value: NonIcDissem,
    pub bytes: &'src str,
    pub span: Span,
}

/// One REL TO country / country-group entry + source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRelToEntry<'src> {
    pub value: CountryCode,
    /// E.g., `"USA"`, `"FVEY"`, `"AUSTRALIA_GROUP"`.
    pub bytes: &'src str,
    pub span: Span,
}

/// Declassification date + source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDeclassifyOn<'src> {
    pub value: IsmDate,
    /// Source representation, e.g., `"20351231"` or `"2035-12-31"`.
    pub bytes: &'src str,
    pub span: Span,
}

/// One AEA block + source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAea<'src> {
    pub value: AeaMarking,
    /// Full AEA block (e.g., `"RD-CNWDI-SIGMA 18 20"`).
    pub bytes: &'src str,
    pub span: Span,
}
```

### 3.2 `CanonicalAttrs`

```rust
// crates/ism/src/canonical.rs
//
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CanonicalAttrs` — the owned, post-canonical marking representation
//! that rules consume.
//!
//! Constructed from `ParsedAttrs<'_>` exactly two ways:
//!
//! 1. **PR 3a transitional**: `from_parsed_unchecked` — a `pub
//!    #[doc(hidden)]` adapter that performs the structural
//!    rename without applying any canonicalization rules. PR 3a's
//!    invariant is byte-identical behavior on every fixture; the
//!    adapter exists to thread the new types through the engine
//!    without churning rule semantics. PR 3c deletes this function.
//!
//! 2. **Post-PR-3c canonical path**: `MarkingScheme::canonicalize`,
//!    the only authorized public route. A scheme decides what
//!    canonicalization means (case folding, deprecated-token
//!    migration, etc.) and rule crates do not own the choice.
//!
//! # Why owned
//!
//! Rules need attrs that outlive the source byte buffer (e.g.,
//! `PageContext` accumulates per-portion attrs across the whole page
//! before banner-validation rules consume the aggregate; the source
//! buffer of an early portion may have been freed by then). Having
//! `CanonicalAttrs` own its data simplifies the lifetimes that flow
//! through the engine without forcing every rule signature to carry
//! an `'src` parameter.
//!
//! # Field shape
//!
//! Mirrors `IsmAttributes` exactly at PR 3a — same field names, same
//! types, same semantics. Subsequent PRs reshape:
//!
//! - **PR 9 (FR-046)** splits `dissem_controls` into `dissem_us` +
//!   `dissem_nato` once the parser tracks separator spans (#106).
//! - **PR 3c** may migrate `sci_controls` (the CVE projection) to a
//!   `SciSet`-only shape if no rule reads `sci_controls` post-collapse
//!   (CLAUDE.md "compatibility view scheduled for removal").
//! - **PR 2 (FR-017)** introduces `FgiMarker::SourceConcealed |
//!   Acknowledged`. PR 3a uses the existing flat `FgiMarker`.
//!
//! Holding the existing field shape at PR 3a is what keeps the change
//! byte-identical and independently revertable.

use crate::attrs::{
    AeaMarking, CountryCode, DeclassExemption, DissemControl, FgiMarker,
    MarkingClassification, NonIcDissem, SarMarking, SciControl, SciMarking,
    TokenSpan,
};
use crate::date::IsmDate;
use crate::parsed::{
    ParsedAea, ParsedAttrs, ParsedClassification, ParsedDeclassifyOn,
    ParsedDissem, ParsedFgiMarker, ParsedNonIcDissem, ParsedRelToEntry,
    ParsedSarMarking, ParsedSciMarking,
};

/// Owned, canonical-form attributes. The pivot type rules consume.
///
/// # Block ordering (CAPCO)
///
/// Field order mirrors CAPCO block sequence: classification → SCI →
/// SAR → AEA → FGI → IC dissem → non-IC dissem → REL TO → CAB. This
/// is documentation-only; rules dispatch on field name, not order.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalAttrs {
    /// US/FGI/NATO/JOINT classification, or `None` when the parser
    /// found no classification. **FR-007**: must remain `Option<_>` —
    /// the `MarkingClassification::Us` hardcode at
    /// `crates/capco/src/scheme.rs:365` is PR 5's deletion target,
    /// not PR 3a's.
    pub classification: Option<MarkingClassification>,

    /// SCI controls (CVE projection). Compatibility view per CLAUDE.md;
    /// new rules SHOULD read `sci_markings` instead.
    pub sci_controls: Box<[SciControl]>,

    /// Structural SCI markings — authoritative for compartments and
    /// sub-compartments per CAPCO §A.6.
    pub sci_markings: Box<[SciMarking]>,

    /// SAR block, at most one per marking per §A.6. Cardinality is
    /// `Option`, not `Box<[]>`.
    pub sar_marking: Option<SarMarking>,

    /// AEA markings (RD/FRD/CNWDI/SIGMA/UCNI/TFNI) per §H.6.
    pub aea_markings: Box<[AeaMarking]>,

    /// FGI marker in a US-classified marking. Flat shape at PR 3a;
    /// PR 2 introduces the `SourceConcealed | Acknowledged`
    /// discriminant (FR-017).
    pub fgi_marker: Option<FgiMarker>,

    /// IC dissemination controls. Single field at PR 3a; PR 9
    /// (FR-046) splits into `dissem_us` + `dissem_nato`.
    pub dissem_controls: Box<[DissemControl]>,

    /// Non-IC dissemination controls (CAPCO §H.9).
    pub non_ic_dissem: Box<[NonIcDissem]>,

    /// REL TO country / country-group codes.
    pub rel_to: Box<[CountryCode]>,

    /// Declassification date from CAB (typed precision tier).
    pub declassify_on: Option<IsmDate>,

    /// Free-text "Classified By" identifier from CAB.
    pub classified_by: Option<Box<str>>,

    /// Free-text "Derived From" source from CAB.
    pub derived_from: Option<Box<str>>,

    /// Declassification exemption code from CAB.
    pub declass_exemption: Option<DeclassExemption>,

    /// Per-token byte spans into the original source buffer. Reused
    /// verbatim from `IsmAttributes::token_spans`. Used by rules that
    /// need byte-precise diagnostic spans (E001, E002, E003, ...).
    pub token_spans: Box<[TokenSpan]>,
}

impl CanonicalAttrs {
    /// Convenience accessor: returns the US classification level if
    /// this marking uses the US or Conflict classification system.
    /// Pure-FGI / NATO / JOINT markings return `None`.
    ///
    /// Mirrors `IsmAttributes::us_classification` exactly so existing
    /// rule call sites compile unchanged after the type rename.
    pub fn us_classification(&self) -> Option<crate::attrs::Classification> {
        match self.classification {
            Some(MarkingClassification::Us(c)) => Some(c),
            Some(MarkingClassification::Conflict { us, .. }) => Some(us),
            _ => None,
        }
    }
}
```

### 3.3 `from_parsed_unchecked` adapter

```rust
// crates/ism/src/canonical.rs (continued)

/// Transitional adapter — converts `ParsedAttrs<'_>` into
/// `CanonicalAttrs` by structural rename only.
///
/// **`#[doc(hidden)] pub`** because the data-model.md spec (and FR-043)
/// require it to be cross-crate-callable but visibly project-internal.
/// The `_unchecked` suffix follows the Rust-stdlib convention: a path
/// that *exists* but is not the public-API path you should reach for.
///
/// # PR-3c lifecycle
///
/// This function deletes at PR 3c, when `MarkingScheme::canonicalize`
/// becomes the sole `ParsedAttrs → CanonicalAttrs` constructor (FR-043).
/// FR-040's `_unchecked`-shape signature lint (R-11 in `research.md`)
/// flags any function matching `fn(...ParsedAttrs<'_>...) ->
/// CanonicalAttrs` outside `MarkingScheme::canonicalize`; the adapter
/// here is whitelisted via path-based carve-out
/// (`crates/ism/src/canonical.rs::from_parsed_unchecked`) for the
/// duration of the keystone window. The carve-out auto-removes when 3c
/// lands and the function is deleted.
///
/// # Semantics
///
/// **Byte-identical to PR-3a-pre behavior.** Every field is moved
/// across without transformation — no case folding, no deprecated-token
/// migration, no canonicalization. The function name's `_unchecked`
/// suffix names this exact gap: a real `canonicalize` impl would do
/// more work; this adapter does none.
///
/// # Why it isn't `From<ParsedAttrs<'_>> for CanonicalAttrs`
///
/// FR-040's lint targets `fn(...ParsedAttrs<'_>...) -> CanonicalAttrs`
/// signatures regardless of name. Implementing `From` would generate a
/// lint-flagging `fn from(_: ParsedAttrs<'_>) -> Self` synthesized
/// signature; whitelisting it would dilute the lint. A free function
/// with a deliberately-unwieldy name is the right shape for "yes,
/// this exists; no, you should not reach for it casually."
#[doc(hidden)]
pub fn from_parsed_unchecked(parsed: ParsedAttrs<'_>) -> CanonicalAttrs {
    let ParsedAttrs {
        classification,
        sci_markings,
        sci_controls,
        sar_marking,
        aea_markings,
        fgi_marker,
        dissem_controls,
        non_ic_dissem,
        rel_to,
        declassify_on,
        classified_by,
        derived_from,
        declass_exemption,
        token_spans,
        source_bytes_origin: _, // discarded; not on CanonicalAttrs
    } = parsed;

    CanonicalAttrs {
        classification: classification.map(|c| c.value),
        sci_controls,
        sci_markings: sci_markings
            .into_vec()
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        sar_marking: sar_marking.map(|p| p.value),
        aea_markings: aea_markings
            .into_vec()
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        fgi_marker: fgi_marker.map(|p| p.value),
        dissem_controls: dissem_controls
            .into_vec()
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        non_ic_dissem: non_ic_dissem
            .into_vec()
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        rel_to: rel_to
            .into_vec()
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        declassify_on: declassify_on.map(|p| p.value),
        classified_by: classified_by.map(|s| s.into()),
        derived_from: derived_from.map(|s| s.into()),
        declass_exemption,
        token_spans,
    }
}
```

**Implementer note**: the `into_vec().into_iter().map().collect()` round
trip is unavoidable because `Box<[T]>` is not iter-by-value-friendly in
stable Rust. A future cleanup could use `Vec::from(boxed_slice).into_iter()`
form (more readable) — implementer's choice; both compile to identical
code on opt builds. Avoid `.iter().cloned()` here: that defeats the
move and would force `T: Clone` bounds we don't need.

### 3.4 `ProjectedMarking` and `ProjectionProvenance`

```rust
// crates/ism/src/projected.rs
//
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `ProjectedMarking` — the output of `MarkingScheme::project(scope,
//! ...)`. Defined at PR 3a, **not wired** at PR 3a; PR 6 cuts over.
//!
//! At PR 3a the type exists so PR 5's `expected_classification` widening
//! and PR 6's `Scope::Page` cutover have a stable target. No engine call
//! site reads or writes `ProjectedMarking` here — `PageContext::expected_*`
//! continues to drive page roll-up.
//!
//! # Field shape
//!
//! Mirrors `CanonicalAttrs` for the fields that participate in page
//! roll-up plus a `scope` discriminant and a `provenance` trace.
//! Fields not relevant to projection (`classified_by`, `derived_from`,
//! `declass_exemption`, `token_spans`) are absent — a projected marking
//! is a banner / page aggregate, not a CAB.

use crate::attrs::{
    AeaMarking, CountryCode, DissemControl, FgiMarker, MarkingClassification,
    NonIcDissem, SarMarking, SciControl, SciMarking,
};
use crate::date::IsmDate;
use crate::span::Span;
use marque_scheme::Scope;

/// Output of a `MarkingScheme::project(scope, ...)` call.
///
/// PR 3a defines the shape; PR 6 wires the engine to consume it.
/// Banner-validation rules migrate to `&ProjectedMarking` at PR 9.
///
/// **FR-007 + FR-008**: `classification: Option<MarkingClassification>`
/// preserves foreign provenance; `fgi_marker` survives the projection
/// alongside classification rather than being collapsed into it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedMarking {
    /// Which scope produced this projection. The engine reads this to
    /// dispatch banner-validation vs. document-level rules.
    pub scope: Scope,

    /// Aggregated classification. `None` when no portion contributed a
    /// US classification — pure-foreign pages produce this case
    /// post-PR-5.
    pub classification: Option<MarkingClassification>,

    /// SCI controls (CVE projection of `sci_markings`).
    pub sci_controls: Box<[SciControl]>,

    /// Structural SCI markings (compartments + sub-compartments).
    pub sci_markings: Box<[SciMarking]>,

    /// SAR block, at most one per banner per §A.6.
    pub sar_marking: Option<SarMarking>,

    /// AEA markings.
    pub aea_markings: Box<[AeaMarking]>,

    /// FGI marker. Survives projection so banner roll-up retains
    /// foreign provenance (FR-008, #261).
    pub fgi_marker: Option<FgiMarker>,

    /// IC dissemination controls. Single field at PR 3a/PR 6; PR 9
    /// splits into `dissem_us` + `dissem_nato`.
    pub dissem_controls: Box<[DissemControl]>,

    /// Non-IC dissemination controls.
    pub non_ic_dissem: Box<[NonIcDissem]>,

    /// REL TO list (intersection across portions, NOFORN-superseded).
    pub rel_to: Box<[CountryCode]>,

    /// Most-conservative declassification date (max-end across
    /// portions).
    pub declassify_on: Option<IsmDate>,

    /// Trace of which portions and lattice operations contributed.
    /// Used by banner-validation rules (E035 SCI roll-up, E031 SAR
    /// roll-up, etc.) to point a diagnostic at the offending
    /// per-portion span.
    pub provenance: ProjectionProvenance,
}

/// Lattice trace + per-portion contribution record for a
/// `ProjectedMarking`.
///
/// Defined at PR 3a as an empty-default placeholder. PR 6 fills in the
/// fields consumed by banner-validation rules. The shape is reserved so
/// PR 6 doesn't require a separate type-system change.
///
/// # Why a struct, not a typedef
///
/// Banner rules need both the source-portion spans (for diagnostic
/// pointers) and a lattice-operation summary ("which join produced this
/// SCI compartment set?"). Splitting them into a struct now avoids a
/// later breaking-change PR.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectionProvenance {
    /// Source-portion spans that contributed to this projection.
    /// Used by E035 (SCI banner roll-up) to point diagnostics at the
    /// offending portion when the banner is missing a compartment.
    pub contributing_portion_spans: Box<[Span]>,
}
```

---

## 4. Migration plan

### 4.1 Parser (`marque-core::parser`)

The parser today builds an `IsmAttributes` and returns
`ParsedMarking { attrs, source_span, kind }`. Post-PR-3a:

```rust
// crates/core/src/parser.rs

use marque_ism::{ParsedAttrs, ProjectedMarking, /* leaf types */};

#[derive(Debug)]
pub struct ParsedMarking<'src> {
    pub attrs: ParsedAttrs<'src>,
    pub source_span: Span,
    pub kind: MarkingType,
}

impl<'t> Parser<'t> {
    pub fn parse<'src>(
        &self,
        candidate: &MarkingCandidate,
        source: &'src [u8],
    ) -> Result<ParsedMarking<'src>, CoreError> { /* ... */ }
}
```

**Required changes inside the parser body**:

1. Replace every `IsmAttributes::default()` construction with a private
   builder `ParsedAttrsBuilder<'src>` that accumulates `Box<Vec<...>>`s
   and finalizes to `ParsedAttrs<'src>`. The current parser's
   incremental builds already use `Vec<T>` collected at the end; rename
   the locals (`sci`, `sci_markings`, `aea`, `dissem`, `non_ic`,
   `rel_to`) to the `Parsed*<'src>`-typed equivalents and wrap each
   parsed value with the source-bytes slice that produced it.

2. At every `attrs.field = ...` site that today writes a
   non-`Parsed*<'src>` value, wrap the value with the corresponding
   `Parsed*<'src>` constructor using the same `&'src str` the parser
   already extracted (`trimmed`, `s`-via-`&s[rel_start..rel_end]`,
   tetragraph slices, etc.).

3. The CAB path (`parse_cab`) builds `classified_by: Some(val.trim())`
   today; switch to `Some(val.trim())` returning a `&'src str` (already
   the type). The engine consumes via `.into()` in
   `from_parsed_unchecked`.

4. Set `source_bytes_origin: SourceOrigin::Portion | Banner | Cab`
   from the dispatch arm of `parse`.

5. `parse_cab` does not produce per-token spans today (`token_spans`
   left empty); preserve that. CAB markings get
   `source_bytes_origin: Cab` and `token_spans: Box::new([])`.

**Estimated scope**: ~80 line-edits in `crates/core/src/parser.rs`,
all mechanical (name changes + wrapper construction). The parser body
is 1500+ lines but the touched surface is concentrated in
`parse_marking_string` and `parse_cab`.

### 4.2 Scheme (`marque-capco::CapcoScheme::project`)

`CapcoScheme::project` at PR 3a is a `MarkingScheme::project_banner`
implementer that today returns `CapcoMarking` (which wraps
`IsmAttributes`). PR 3a's job: rename `IsmAttributes` to
`CanonicalAttrs` inside `CapcoMarking` and update
`page_context_to_attrs` to return `CanonicalAttrs`.

Required edits:

1. `page_context_to_attrs(&PageContext) -> CanonicalAttrs` — the body
   stays identical except it constructs `CanonicalAttrs { ... }`
   instead of `IsmAttributes { ... }`. The `MarkingClassification::Us`
   hardcode at `:365` stays untouched at PR 3a (PR 5 deletes it).

2. Anywhere `CapcoMarking` wraps an `IsmAttributes`, rename to
   `CanonicalAttrs`. The lattice impls on `CapcoMarking` (`Lattice for
   CapcoMarking` etc.) keep their bodies; only the inner type name
   changes.

**Do NOT** change `project_banner` semantics, do NOT change which
fields participate in roll-up, do NOT introduce `Scope::Page`
dispatch. PR 6 owns that work.

### 4.3 Rule crate (`marque-capco::rules*`)

The trait change in `marque-rules`:

```rust
// crates/rules/src/lib.rs

pub trait Rule: Send + Sync {
    fn id(&self) -> RuleId;
    fn name(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic>;
    //                       ^^^^^^^^^^^^^^^^
    //                       was: &IsmAttributes
}

// And the import change at the crate top:
use marque_ism::{CanonicalAttrs, Span};  // was: IsmAttributes, Span
```

**Cascade across the rule crate**:

| File | Change | Quantified |
|---|---|---|
| `crates/rules/src/lib.rs` | Trait sig + 1 use-line | ~3 lines |
| `crates/capco/src/rules.rs` | `&IsmAttributes` → `&CanonicalAttrs` everywhere | ~100 sites (~56 rules × ~2 sites/rule incl. helpers) |
| `crates/capco/src/rules_declarative.rs` | Same rename | ~30 sites |
| `crates/capco/src/rules_sci_per_system.rs` | Same rename | ~15 sites |
| `crates/capco/src/scheme.rs` | `evaluate_named_constraint(&IsmAttributes, ...)` → `(&CanonicalAttrs, ...)`; `page_context_to_attrs` return type | ~10 sites |

`sed -i 's/&IsmAttributes\b/\&CanonicalAttrs/g'` over the rule crate
plus the `marque_ism::IsmAttributes` import → `marque_ism::CanonicalAttrs`
import will catch most. Hand-verify in `scheme.rs` (the inherent
`evaluate_named_constraint` fast path) and `lattice.rs` (`SciSet`,
`SarSet`, `FgiSet` constructors that take `&[SciMarking]` etc., not
attrs — those don't touch the rename).

**Rule-body invariant**: not a single rule body changes. Every field
read (`attrs.classification`, `attrs.rel_to`, `attrs.token_spans`,
etc.) resolves identically against `CanonicalAttrs`. Rule unit tests
in `#[cfg(test)] mod tests { ... }` — about 30 inline modules with
~67 `IsmAttributes { ... }` literals total — get the rename via the
same sed pass.

### 4.4 Engine + `RuleContext`

`RuleContext.page_context: Option<Arc<PageContext>>` keeps its type;
`PageContext` switches its inner `Vec<IsmAttributes>` to
`Vec<CanonicalAttrs>` and its `add_portion(&mut self, attrs:
IsmAttributes)` parameter becomes `CanonicalAttrs`.

The engine's `lint_with_options` body builds `PageContext` like:

```rust
// crates/engine/src/engine.rs (current, illustrative)

let parsed = parser.parse(&candidate, source)?;     // ParsedMarking<'src>
let attrs = from_parsed_unchecked(parsed.attrs);    // CanonicalAttrs

// page_context.add_portion(attrs.clone()) for portions
// rule.check(&attrs, &ctx) for rules
```

The `from_parsed_unchecked` call site is the **single** transitional
adapter site. The engine owns it. After PR 3c the line becomes
`scheme.canonicalize(parsed.attrs)`; the rest of the body is
unchanged.

**Recognizer plumbing**: `Recognizer<S>` is Phase D / probabilistic
machinery. The `StrictRecognizer` and `DecoderRecognizer` (in
`marque-engine`) currently produce `IsmAttributes` via a slightly
different path. Per the consolidated plan §3.1, post-PR-3a the
recognizers produce `Parsed<S::Marking>` where `S::Marking =
CapcoMarking` (which wraps `CanonicalAttrs`). PR 3a wires this by:

1. Swap the `CapcoMarking::new(IsmAttributes)` constructor signature to
   take `CanonicalAttrs`.
2. The recognizer adapters in `marque-engine::decoder` /
   `marque-engine::recognizer` build their `Parsed<S::Marking>` via
   the same `from_parsed_unchecked` adapter.

`RecognizerError` and `ParseContext` types in `marque-scheme` stay
unchanged.

### 4.5 Tests

Three classes of test files touch:

**A. Unit tests inside source files** (~30 `#[cfg(test)] mod tests`
modules). Each constructs `IsmAttributes { ... }` literals and passes
to a rule's `check`. Sed-replaceable form:

```bash
# In each test module:
sed -i \
  -e 's/IsmAttributes\b/CanonicalAttrs/g' \
  -e 's/marque_ism::IsmAttributes/marque_ism::CanonicalAttrs/g' \
  -e 's/use marque_ism::IsmAttributes;/use marque_ism::CanonicalAttrs;/g' \
  $file
```

**B. Integration tests in `crates/{capco,engine,ism}/tests/`**. Same
sed pass. Approximate site counts (re-grep at edit time, defect
classes are stable, line numbers are not):

- `crates/capco/tests/` — ~40 `IsmAttributes { ... }` literals
- `crates/engine/tests/` — ~15 sites
- `crates/ism/tests/` — ~12 sites (page_context aggregation tests)

**Total ~67 sites** matches the task description.

**C. Tests that build a `ParsedAttrs<'src>` directly** (none exist
today; all current tests build the post-canonical form). PR 3a does
not introduce any.

**Special cases**:

1. Tests that build `IsmAttributes` and immediately wrap in a
   `CapcoMarking` for projection — same rename applies.
2. The `crates/ism/src/page_context.rs` inline tests (the long
   `mod tests { ... }` block, ~600 lines of test code in the file
   already read above) construct `IsmAttributes` literals densely;
   these are the bulk of the 67 sites and migrate cleanly under sed.
3. `crates/capco/tests/scheme_equivalence.rs` (Phase B equivalence
   test). The test compares `scheme.project(Scope::Page, ...)` output
   against `PageContext::expected_*` output. Both sides change type;
   the comparison stays semantically identical. Equivalence is
   preserved.

**Migration recipe (mechanical)**:

```bash
# From repo root:
git grep -l -E 'IsmAttributes' | grep -v specs/ | grep -v docs/ | \
  xargs sed -i \
    -e 's/\bIsmAttributes\b/CanonicalAttrs/g' \
    -e 's/use marque_ism::IsmAttributes/use marque_ism::CanonicalAttrs/g'

cargo check --workspace --all-targets
# → expect ~3-5 hand-fix sites: page_context.rs's storage type,
#   parser.rs's return type, recognizer plumbing.
```

After the sed pass, hand-fix:
- `crates/ism/src/page_context.rs` — `Vec<IsmAttributes>` →
  `Vec<CanonicalAttrs>`, `add_portion` parameter, `portions()` return
- `crates/core/src/parser.rs` — return type + `Parsed*<'src>`
  wrapping (this is the substantive engineering work; see §4.1)
- `crates/engine/src/engine.rs` — single `from_parsed_unchecked` call
  site
- `crates/capco/src/scheme.rs` — `page_context_to_attrs` return type
- `crates/ism/src/lib.rs` — re-exports
- `crates/ism/src/attrs.rs` — delete `IsmAttributes` + `impl`

### 4.6 Documentation updates

- `crates/ism/README.md` (if present) — update to mention the new
  three-type pivot.
- Crate-level `//!` doc in `crates/ism/src/lib.rs` — replace mentions
  of "the canonical `IsmAttributes`" with the three-type description.
- `CLAUDE.md` "Key Types" section — update post-PR-3a (separate
  follow-up commit; not in PR 3a scope per task list).

---

## 5. Decisions register

| # | Question | Decision | Rationale |
|---|---|---|---|
| 1 | `dissem_us` vs single `dissem` field shape on `CanonicalAttrs` | **Single `dissem_controls`** at PR 3a. PR 9 splits per FR-046 | PR 3a is rename+borrow only. Splitting now requires teaching the parser separator-span tracking (#106 / FR-045) — that is PR 9's job. Splitting blindly with all dissem in `dissem_us` and an always-empty `dissem_nato` ships an explicit lie about field semantics; better to leave the existing single field and let PR 9 do the real split with parser support. |
| 2 | `sci_set` vs parallel `sci_controls` + `sci_markings` | **Keep both fields** verbatim from `IsmAttributes` | CLAUDE.md schedules `sci_controls` removal "in Phase C or D when no rule references it." Several rules (E010, E011) read `sci_controls` today. Removing it requires migrating those rules — that's PR 3b's collapse, not PR 3a's rename. PR 3a keeps both. PR 3c can convert to `SciSet`/`SarSet`/`FgiSet` after the rule-collapse simplifies the consumer side. |
| 3 | `SarSet` vs `Option<SarMarking>` | **`Option<SarMarking>`** | CAPCO §A.6 caps SAR at one block per marking. The lattice form (`SarSet`) is meaningful at projection (page roll-up), not at portion-attrs. PR 3a stays singular; `SarSet` already exists in `marque-capco::lattice` and PR 6 wires it in. |
| 4 | `FgiSet` vs `Option<FgiMarker>` | **`Option<FgiMarker>`** | Same as SAR: per-portion cardinality is at most one. `FgiSet` is the page-projection lattice form. PR 3a preserves the existing shape. |
| 5 | Fields not in data-model.md spec (`non_ic_dissem`, `rel_to`, `classified_by`, `derived_from`, `declass_exemption`, `aea_markings`) | **All retained on both `ParsedAttrs<'src>` and `CanonicalAttrs`** with their existing names | Spec data-model.md was sketching the future shape post-narrowing; it is not the authoritative PR 3a contract. PR 3a's invariant is byte-identical behavior — every field that current rules read must remain available. |
| 6 | `aea: Option<AeaControl>` (singular) vs `aea_markings: Box<[AeaMarking]>` (plural) | **`aea_markings: Box<[AeaMarking]>`** plural; matches today's parser output | Multiple AEA blocks per marking are valid (`SECRET//RD//FRD-SIGMA 14//NOFORN`). data-model.md's `Option<AeaControl>` shape is incorrect for current CAPCO grammar; defer to PR 3c if a real narrowing is wanted. |
| 7 | `Parsed*<'src>` field types: byte-slice vs `&str` vs Span-only | **`&'src str` + `Span`** on every wrapper | Parser already validated UTF-8 at scanner-candidate ingest. `&'src str` is what FR-019 round-trip property needs (compares byte-by-byte). Span is kept for diagnostic positioning. The pair is small (16+16 bytes on 64-bit) and inline in `Box<[Parsed*<'src>]>`. Pure-Span (no `&'src str`) would force re-borrowing at canonicalization sites. |
| 8 | `from_parsed_unchecked` semantics — exact byte-identical or canonicalize-while-bridging? | **Exact byte-identical (no canonicalization)** | PR 3a's revertability gate is `corpus-regression × {3a-only}` byte-identical. Any normalization in the adapter would be a behavior change PR 3c would inherit. The adapter is intentionally a structural rename. |
| 9 | `ProjectedMarking` field shape — fully filled at PR 3a or stub? | **Fully filled type, empty `ProjectionProvenance`** | The type is defined now so PR 5 (FR-007 widening) and PR 6 (cutover) don't need a separate type-system change. `ProjectionProvenance` lands as an empty-default struct with one `contributing_portion_spans: Box<[Span]>` field reserved; PR 6 fills it. |
| 10 | Migration shape for ~67 test fixtures | **Sed-replace `IsmAttributes` → `CanonicalAttrs`** | Field shapes are identical at PR 3a; mechanical rename catches every site. Hand-review unit tests in `parser.rs` if any (none today). |
| 11 | Does `IsmAttributes` survive as deprecated re-export? | **No — delete entirely** | Per "no users, no API expectations." No `pub use IsmAttributes = CanonicalAttrs` shim. Constitution VII's clean-break philosophy applies. Anything that compiles today against `IsmAttributes` either renames or is in a corpus regression test that already gets the sed pass. |
| 12 | `Rule` trait signature change — single-step or with shim? | **Single-step at PR 3a** | The shim cost is greater than the rename cost. `Rule::check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext)` lands at PR 3a; rule crates compile under the new signature in the same PR. |
| 13 | Module placement: one file or split? | **Three files** (`parsed.rs`, `canonical.rs`, `projected.rs`) | Each is the keystone for a different downstream PR (3c / 6 / 9). Each will accrete logic; co-locating in `attrs.rs` reproduces the 1500-line file we're unwinding. |
| 14 | `RuleContext` — does it carry `'src`? | **No** — PR 3a context stays owned (`Arc<PageContext>` of `CanonicalAttrs`) | Adding `'src` to `RuleContext` bleeds the lifetime through every rule signature. Rules consume owned `&CanonicalAttrs`; the `'src` only lives in the parser → adapter step that the engine owns. |
| 15 | `ParsedAttrs<'src>` derives — `PartialEq + Eq`? | **Yes**, plus `Debug + Clone` | All inner fields (typed values + `&'src str`) implement `PartialEq + Eq` already. Useful for tests asserting parser output. |

---

## 6. Out-of-scope but worth flagging (for PR 3b/3c/5/6/9)

- **PR 3c — adapter deletion + `MarkingScheme::canonicalize`**: the
  `from_parsed_unchecked` adapter goes; rules consume only via the
  trait method. PR 3a's `_unchecked` is the disposable bridge. The
  FR-040 lint must be wired with the path-based carve-out
  whitelist mentioned in research R-11.

- **PR 3c — narrow `CanonicalAttrs` field shape**: post-rule-collapse,
  `sci_controls` may delete (no consumers); `Box<[]>` fields may
  consolidate into the `Set` types (`SciSet`, `SarSet`). PR 3a holds
  the old shape; do not pre-narrow.

- **PR 5 — `expected_classification: Option<MarkingClassification>` +
  delete `MarkingClassification::Us` hardcode**: PR 3a leaves that
  hardcode at `crates/capco/src/scheme.rs:365` untouched. Adding a
  `// TODO PR 5` comment would couple PRs; skip the comment, just
  don't introduce new dependencies.

- **PR 6 — `ProjectedMarking` consumer wiring**: PR 3a defines the
  type; banner-validation rules continue reading `PageContext`. PR 6
  flips the default.

- **PR 9 — `dissem_us` / `dissem_nato` split**: PR 3a uses single
  `dissem_controls`; PR 9 splits.

- **PR 2 — `FgiMarker::SourceConcealed | Acknowledged`**: PR 3a uses
  the existing flat `FgiMarker { countries: Box<[CountryCode]> }`. PR
  2 introduces the discriminant + parser route changes.

- **`MARQUE_AUDIT_SCHEMA` cutover**: schema stays at `marque-mvp-2`
  through PR 3a/3b; PR 3c bumps to `marque-1.0`. PR 3a does not
  touch `crates/engine/src/lib.rs`'s `AUDIT_SCHEMA_VERSION`.

- **Recognizer scoring (R001 message)**: PR 3a does not change
  `crates/engine/src/engine.rs:1389`'s `format!("decoder-recognized
  canonical form: {replacement:?}")` — that's PR 3c's job per
  FR-003.

- **`AppliedFix.proposal.original`** and `proposal.replacement` leak
  channels (#257) — PR 3c's responsibility; PR 3a leaves them as-is
  but the masking-pin in `core_error_isolation.rs` stays.

- **`marque-wasm` + `marque-server` + benchmark code paths**: each
  consumes the engine's `lint`/`fix` API. After the rename, these
  build clean. Verify under `wasm-pack build crates/wasm` and
  `cargo build -p marque-server` as part of the PR check.

---

## 7. Risk register

| # | Risk | Probability | Mitigation |
|---|---|---|---|
| 1 | **Lifetime threading regression in the parser**: introducing `'src` may force unrelated function signatures to grow. The parser body has helper functions (`parse_classification`, `parse_nato_classification`, `parse_sar_category`, etc.) that today take `&str`. Lifetime inference will Just Work for these (the `&str` they take is borrowed from the same `'src`), but a few `Box<str>::into` sites in error paths may need `&'src str` adjustments. | Medium | Run `cargo check --workspace --all-targets` after every parser-file edit, not in one big bang. The compiler will surface lifetime mismatches with line-precise errors. Implementer should not `#[allow]` any lifetime warning that surfaces — each is a real call site that now needs to declare its borrow source. |
| 2 | **Hidden `IsmAttributes` references in path-qualified imports** (e.g., `marque_ism::attrs::IsmAttributes`) that the sed pass over `\bIsmAttributes\b` catches but module-rename doesn't. | Low-Medium | Run `git grep -E 'attrs::IsmAttributes\|attrs::CanonicalAttrs'` after the rename. The `IsmAttributes` struct lives in `attrs.rs` today; PR 3a moves `CanonicalAttrs` to `canonical.rs`, so callers reaching for `marque_ism::attrs::IsmAttributes` need updating to `marque_ism::CanonicalAttrs` (the re-export). Alternatively, keep `pub use canonical::CanonicalAttrs` available in `attrs.rs` as a transparent alias for the duration of PR 3a, removed at PR 3b/3c — implementer's call. Recommend the cleaner path: just update the imports. |
| 3 | **`PageContext` storage type change breaks tests that pattern-match on `IsmAttributes`**. The `portions()` accessor returns `&[IsmAttributes]` today and is consumed by S005/#206 paths in the rule crate. | Low | The accessor renames to `&[CanonicalAttrs]`; sed catches it. Verify under `cargo test -p marque-capco` after the rename. The S005 / S006 rules (REL TO membership-uncertain reduction) read `portions()` directly — re-verify them in the rule-crate test sweep before merging. |
| 4 | **Implementer narrows `CanonicalAttrs` fields against the design** ("data-model.md says `dissem_us` and `dissem_nato`") and corrupts revertability. | Low | This document explicitly forbids field-shape changes at PR 3a. Reviewer should reject any PR 3a diff that adds a new field, removes a field, or splits an existing field. The CI matrix `corpus-regression × {3a-only}` is the mechanical guard — any narrowing produces semantic-different output and the matrix fails. |
| 5 | **`from_parsed_unchecked` accidentally allocates** through one of the `into_vec().into_iter().map().collect()` round-trips, regressing parser hot-path latency. | Low | The `into_vec()` form is a compile-time `unsafe` no-op (it reuses the boxed slice's allocation as a Vec). The `.map()` chain is a single-pass move; no per-element allocation. SC-001 latency bench should show no change. If it does, switch to `Vec::from(boxed).into_iter()` form (same machine code, cleaner source). |
| 6 | **`ProjectedMarking` is defined but never constructed at PR 3a**, leading to `dead_code` warnings. | Low | The type is `pub`-exported from `marque-ism`; `dead_code` doesn't fire for `pub` items. If clippy's `unused_imports` / `dead_code` lints in CI flag it (workspace-wide `#[deny(...)]`), suppress with a single targeted `#[allow(dead_code)]` on the type with a `// PR 6 wires consumer` comment. Better: `marque-engine` adds a no-op `pub fn _project_marking_unused() -> ProjectedMarking { ... }` test-only constructor — implementer's call. Recommend the targeted allow. |

---

## 8. Acceptance checklist for the implementer

Before opening PR 3a:

- [ ] Three new files exist: `parsed.rs`, `canonical.rs`, `projected.rs`
- [ ] `IsmAttributes` and its `impl` are deleted from `attrs.rs`
- [ ] `marque-ism/src/lib.rs` re-exports updated (no `IsmAttributes`)
- [ ] Parser produces `ParsedMarking<'src> { attrs: ParsedAttrs<'src>, ... }`
- [ ] Engine's single `from_parsed_unchecked(parsed.attrs)` call site lands
- [ ] `Rule::check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext)`
- [ ] `PageContext::Vec<CanonicalAttrs>` storage; accessor signatures renamed
- [ ] All rule files (`rules.rs`, `rules_declarative.rs`, `rules_sci_per_system.rs`) renamed
- [ ] All test fixtures (~67 sites) renamed
- [ ] `cargo check --workspace --all-targets` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo test --workspace` passes (corpus regression)
- [ ] `wasm-pack build crates/wasm --target web --profile release-web` succeeds
- [ ] CI matrix entry `corpus-regression × {3a-only}` added (T025) and green
- [ ] No new `unsafe` blocks, no new `#[allow]` attributes on production code
- [ ] No edits to `crates/capco/src/scheme.rs:365` `Us` hardcode
- [ ] No edits to `crates/engine/src/engine.rs`'s decoder diagnostic format string
- [ ] No edits to `MARQUE_AUDIT_SCHEMA`
- [ ] `from_parsed_unchecked` is `#[doc(hidden)] pub fn`, lives in `crates/ism/src/canonical.rs`, contains zero canonicalization logic

PR description template:

```
PR 3a: Pivot type split (KEYSTONE-1)

Splits IsmAttributes into ParsedAttrs<'src>, CanonicalAttrs, and
ProjectedMarking. Adds from_parsed_unchecked transitional adapter.
Migrates parser, engine, scheme, and rule crate to consume
&CanonicalAttrs. Test fixtures renamed via mechanical sed pass.

Behavior is byte-identical to the prior commit: corpus-regression sweep
× {3a-only} passes. PR 3a is independently revertable.

Constitution check:
- I (perf): no hot-path allocation introduced; from_parsed_unchecked
  performs structural moves only
- II (zero-copy): ParsedAttrs<'src> retains source-byte borrows via 8
  thin Parsed*<'src> wrappers
- III (WASM safety): all new types in marque-ism, no I/O, no platform
  deps; wasm-pack build clean
- IV (two-layer rules): rule signatures rename only; no rule semantics
  change
- V (audit-first): no audit-record changes; AppliedFix unchanged
- VI (dataflow): pipeline shape unchanged
- VII (acyclic deps): no new dep edges
- VIII (citations): no rule citations touched
```