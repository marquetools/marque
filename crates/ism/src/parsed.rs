// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `ParsedAttrs<'src>` — parser output that retains a borrow into the
//! original source bytes for every parsed token.
//!
//! `marque-core::parser` produces a `ParsedAttrs<'src>` per scanner
//! candidate. The engine (via the recognizer) immediately canonicalizes
//! via `MarkingScheme::canonicalize` — the trait route, sole production
//! path per FR-043. Rules consume the resulting `CanonicalAttrs`,
//! never `ParsedAttrs`.
//!
//! # Lifecycle
//!
//! Short-lived. `ParsedAttrs<'src>` exists only between Phase 2 (parser
//! emits) and the immediate canonicalization step. It MUST NOT outlive
//! the input byte buffer it borrows from. Storing one in `RuleContext`,
//! the engine's per-page accumulator, or any cross-document structure
//! is a misuse — those consumers want `CanonicalAttrs` (owned).
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
    AeaMarking, CountryCode, DeclassExemption, DissemControl, FgiMarker, MarkingClassification,
    NonIcDissem, SarMarking, SciControl, SciMarking, TokenSpan,
};
use crate::date::IsmDate;
use crate::dissem_attribution::DefaultOrigin;
use crate::span::Span;

/// Where in the document the parser ran.
///
/// Threaded onto `ParsedAttrs<'src>` so the canonicalizer (PR 3c) and
/// the engine can route per-origin rule subsets. Today the parser sets
/// this from `MarkingType` (`Banner` / `Portion` / `Cab`); the
/// `PageBreak` variant is unrepresentable here because page-break
/// candidates do not produce `ParsedAttrs` (the parser short-circuits).
#[non_exhaustive]
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
/// **Borrow / own split.** Most fields wrap their typed value in a
/// `Parsed*<'src>` struct that carries a `&'src str` source-bytes slice
/// and a `Span` (e.g., `ParsedDissem`, `ParsedSciMarking`). The CAB
/// free-text fields `classified_by` and `derived_from` are
/// `Option<&'src str>` directly — they are simple borrows that do not
/// need a typed value alongside. `declass_exemption` is owned
/// (`Option<DeclassExemption>`, a closed CVE enum, no byte slice
/// needed). `non_ic_dissem` and `rel_to` are wrapped in
/// `ParsedNonIcDissem<'src>` / `ParsedRelToEntry<'src>` like the other
/// dissem categories.
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
///
/// **Exhaustive**: the struct intentionally exposes every field for
/// brace construction and destructure outside `marque-ism`. PR 3c.2.E
/// lifted the structural rename body (formerly
/// `marque_ism::from_parsed_unchecked`) into
/// `CapcoScheme::canonicalize` in `marque-capco`, and into the four
/// `marque-core` test helpers that Constitution VII forbids from
/// reaching `MarkingScheme::canonicalize` (the trait route). Those
/// inline lifts require destructure, so `#[non_exhaustive]` is gone.
/// Field additions become explicit migrations of the inlined sites;
/// FR-043 keeps `MarkingScheme::canonicalize` the sole production
/// `ParsedAttrs → CanonicalAttrs` constructor.
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
    ///
    /// Field name preserves the existing `IsmAttributes::sar_markings`
    /// (plural) form so the rename layer is purely structural —
    /// renaming to singular is deferred to PR 3c when shape narrowing
    /// happens.
    pub sar_markings: Option<ParsedSarMarking<'src>>,

    /// AEA markings (RD/FRD/CNWDI/SIGMA/UCNI/TFNI) per CAPCO §H.6.
    /// Multiple permitted in one block per §H.6.
    pub aea_markings: Box<[ParsedAea<'src>]>,

    /// FGI marker in a US-classified marking (`FGI` or `FGI [LIST]`)
    /// per CAPCO §H.7. Distinct from `MarkingClassification::Fgi`,
    /// which means the marking IS foreign-classified.
    pub fgi_marker: Option<ParsedFgiMarker<'src>>,

    /// US-attributed IC dissemination controls (NOFORN, ORCON, RELIDO,
    /// FOUO, ...). Holds the dissem tokens whose parent portion has any
    /// US classification axis (`MarkingClassification::Us` or
    /// `Conflict { us, .. }`), OR whose parent portion has no
    /// classification axis at all and the configured
    /// [`DefaultOrigin`] is [`DefaultOrigin::Us`] (CAPCO's default).
    ///
    /// **CAPCO-2016 §G.2 Table 5 (pp 40-45) NATO-dissem ARH rule.**
    /// Table 5 enumerates two NATO dissemination control markings —
    /// "ORCON (NATO dissemination control marking)" and "RELEASEABLE
    /// TO or [LIST] ONLY" — and directs the ARH for both to "See US X
    /// ARH requirements." No NATO-specific dissem form (e.g.,
    /// `ORCON-NATO`) exists in the Register. Operational consequence:
    /// when OC or REL TO appears in a US-classified marking, the
    /// resolved namespace is US (the NATO-origin form shares the US
    /// ARH machinery). The split exists so non-CAPCO consumers (future
    /// cross-system translation, audit-trail provenance) can
    /// distinguish the two — `dissem_nato` populates only when the
    /// parent portion has no US classification axis (i.e., a
    /// pure-NATO portion).
    pub dissem_us: Box<[ParsedDissem<'src>]>,

    /// NATO-attributed IC dissemination controls. Populated only for
    /// pure-NATO portions — i.e., the parent portion's
    /// `classification` is `MarkingClassification::Nato(_)` with no US
    /// axis. Mixed or US-classified portions route every dissem to
    /// [`Self::dissem_us`] per the §G.2 Table 5 ARH rule documented
    /// there.
    ///
    /// Tokens that are NATO-only by spec (ATOMAL, BALK, BOHEMIA) are
    /// NOT dissems and route to the AEA / SCI axes per FR-047 — they
    /// never appear here.
    pub dissem_nato: Box<[ParsedDissem<'src>]>,

    /// Non-IC dissemination controls (LIMDIS/LES/SBU/SSI/...).
    /// Separate authority framework per CAPCO §H.9 (pp 169–191).
    pub non_ic_dissem: Box<[ParsedNonIcDissem<'src>]>,

    /// REL TO country / country-group codes. USA must be present and
    /// first when the marking targets a US release (E002 enforces).
    /// Each entry retains its source byte slice.
    pub rel_to: Box<[ParsedRelToEntry<'src>]>,

    /// DISPLAY ONLY country / country-group codes per CAPCO-2016 §H.8
    /// p163. Grammar parallels REL TO (comma-separated trigraphs +
    /// tetragraphs); semantics differ — DISPLAY ONLY is a foreign
    /// **disclosure** decision (recipient may view, may not retain a
    /// copy) while REL TO is a **release** decision (recipient may
    /// retain). USA is NOT required in the DISPLAY ONLY list (release
    /// to US recipients is implicit; the list names the foreign
    /// audience that may view).
    pub display_only_to: Box<[ParsedDisplayOnlyEntry<'src>]>,

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

impl<'src> ParsedAttrs<'src> {
    /// Construct a [`ParsedAttrs`] with every field provided.
    ///
    /// Required because `ParsedAttrs` is `#[non_exhaustive]` to keep
    /// consumers (rules) from pattern-matching exhaustively, but the
    /// parser in `marque-core` is the sole constructor and lives in a
    /// different crate. The constructor accepts every field by value
    /// so the parser does not have to thread `..Default::default()` —
    /// the type deliberately does not derive `Default` so every
    /// construction site has to name `source_bytes_origin` explicitly
    /// (the parser would otherwise lose the dispatch signal silently
    /// on a typo).
    ///
    /// Argument order mirrors the field declaration order so a future
    /// field addition can be slotted in deterministically (and the
    /// parser site updated atomically).
    #[allow(clippy::too_many_arguments)] // mirrors field count by design
    pub fn new(
        classification: Option<ParsedClassification<'src>>,
        sci_markings: Box<[ParsedSciMarking<'src>]>,
        sci_controls: Box<[SciControl]>,
        sar_markings: Option<ParsedSarMarking<'src>>,
        aea_markings: Box<[ParsedAea<'src>]>,
        fgi_marker: Option<ParsedFgiMarker<'src>>,
        dissem_us: Box<[ParsedDissem<'src>]>,
        dissem_nato: Box<[ParsedDissem<'src>]>,
        non_ic_dissem: Box<[ParsedNonIcDissem<'src>]>,
        rel_to: Box<[ParsedRelToEntry<'src>]>,
        display_only_to: Box<[ParsedDisplayOnlyEntry<'src>]>,
        declassify_on: Option<ParsedDeclassifyOn<'src>>,
        classified_by: Option<&'src str>,
        derived_from: Option<&'src str>,
        declass_exemption: Option<DeclassExemption>,
        token_spans: Box<[TokenSpan]>,
        source_bytes_origin: SourceOrigin,
    ) -> Self {
        Self {
            classification,
            sci_markings,
            sci_controls,
            sar_markings,
            aea_markings,
            fgi_marker,
            dissem_us,
            dissem_nato,
            non_ic_dissem,
            rel_to,
            display_only_to,
            declassify_on,
            classified_by,
            derived_from,
            declass_exemption,
            token_spans,
            source_bytes_origin,
        }
    }

    /// Iterate every IC dissem control on this marking across both
    /// namespace fields ([`Self::dissem_us`] then [`Self::dissem_nato`]).
    ///
    /// Use this when the consumer cares about "any IC dissem regardless
    /// of namespace" (e.g., the renderer, the
    /// `is_nontrivial_marking` decoder check). When the consumer cares
    /// specifically about US-attributed or NATO-attributed dissems
    /// (e.g., a future cross-system translator), read the underlying
    /// fields directly.
    ///
    /// The returned iterator is `Clone` so multi-pass consumers (e.g.,
    /// a renderer that walks twice for line-length accounting) do not
    /// need to re-construct it.
    pub fn dissem_iter(&self) -> impl Iterator<Item = &ParsedDissem<'src>> + Clone {
        self.dissem_us.iter().chain(self.dissem_nato.iter())
    }

    /// Default origin to use when [`Self::classification`] is `None`.
    /// Mirrors the [`crate::dissem_attribution::attribute_dissems`]
    /// rule; exposed as an associated constant so call sites can pin
    /// the CAPCO default without re-importing the enum.
    pub const DEFAULT_ORIGIN_CAPCO: DefaultOrigin = DefaultOrigin::Us;
}

// ---------------------------------------------------------------------
// Parsed*<'src> thin wrappers
//
// Each wrapper pairs the parser-produced typed value with the
// source-bytes slice the parser identified as that token. The slice is
// stored as `&'src str` rather than `&'src [u8]` because the parser
// already validated UTF-8 at candidate ingest (per `Span::as_str` +
// `MarkingType::Portion` strip-paren path); deferring re-validation
// here would be wasted work.
//
// All wrappers derive `Debug + Clone + PartialEq + Eq` because each
// inner field already does.
// ---------------------------------------------------------------------

/// Classification with its source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedClassification<'src> {
    pub value: MarkingClassification,
    /// Source bytes the parser interpreted as this classification.
    /// E.g., `"TOP SECRET"`, `"S"`, `"COSMIC TOP SECRET-BOHEMIA"`.
    pub bytes: &'src str,
    /// Span of `bytes` within the original source buffer.
    pub span: Span,
}

impl<'src> ParsedClassification<'src> {
    /// Constructor — required because `#[non_exhaustive]` blocks
    /// brace-construction across crate boundaries (the parser lives
    /// in `marque-core`, not `marque-ism`).
    pub fn new(value: MarkingClassification, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// Structural SCI marking + source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSciMarking<'src> {
    pub value: SciMarking,
    /// Source bytes for the full SCI sub-block (e.g., `"SI-G ABCD"`).
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedSciMarking<'src> {
    pub fn new(value: SciMarking, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// SAR block + source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSarMarking<'src> {
    pub value: SarMarking,
    /// Full SAR block source (e.g., `"SAR-BP-J12 J54"`).
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedSarMarking<'src> {
    pub fn new(value: SarMarking, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// FGI marker + source bytes.
///
/// `FgiMarker` is the post-PR-2 enum (`SourceConcealed` |
/// `Acknowledged { countries: SmallVec<[CountryCode; N]> }`, see
/// `crate::attrs::FgiMarker`). Lawful source-concealed FGI per
/// CAPCO-2016 §H.7 p122 is the `SourceConcealed` variant; an
/// acknowledged country list is `Acknowledged { countries }` with the
/// constructor (`FgiMarker::acknowledged`) rejecting the empty list.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFgiMarker<'src> {
    pub value: FgiMarker,
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedFgiMarker<'src> {
    pub fn new(value: FgiMarker, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// One IC dissem control + source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDissem<'src> {
    pub value: DissemControl,
    /// E.g., `"NF"`, `"NOFORN"`, `"OC"`, `"ORCON"`.
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedDissem<'src> {
    pub fn new(value: DissemControl, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// One non-IC dissem control + source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNonIcDissem<'src> {
    pub value: NonIcDissem,
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedNonIcDissem<'src> {
    pub fn new(value: NonIcDissem, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// One REL TO country / country-group entry + source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRelToEntry<'src> {
    pub value: CountryCode,
    /// E.g., `"USA"`, `"FVEY"`, `"AUSTRALIA_GROUP"`.
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedRelToEntry<'src> {
    pub fn new(value: CountryCode, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// One DISPLAY ONLY country / country-group entry + source bytes.
///
/// Mirrors [`ParsedRelToEntry`] structurally because the comma-separated
/// country-list grammar is identical for REL TO and DISPLAY ONLY per
/// CAPCO-2016 §H.8 p150-151 (REL TO) and §H.8 p163 (DISPLAY ONLY). The
/// semantics differ — DISPLAY ONLY is a *disclosure* decision (foreign
/// recipient may view without retaining a copy) while REL TO is a
/// *release* decision (recipient may retain) per §H.8 p163 Definition —
/// but the per-entry shape (CountryCode + source bytes + span) is the
/// same.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDisplayOnlyEntry<'src> {
    pub value: CountryCode,
    /// E.g., `"AFG"`, `"IRQ"`, `"NATO"`.
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedDisplayOnlyEntry<'src> {
    pub fn new(value: CountryCode, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// Declassification date + source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDeclassifyOn<'src> {
    pub value: IsmDate,
    /// Source representation, e.g., `"20351231"` or `"2035-12-31"`.
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedDeclassifyOn<'src> {
    pub fn new(value: IsmDate, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}

/// One AEA block + source bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAea<'src> {
    pub value: AeaMarking,
    /// Full AEA block (e.g., `"RD-CNWDI-SIGMA 18 20"`).
    pub bytes: &'src str,
    pub span: Span,
}

impl<'src> ParsedAea<'src> {
    pub fn new(value: AeaMarking, bytes: &'src str, span: Span) -> Self {
        Self { value, bytes, span }
    }
}
