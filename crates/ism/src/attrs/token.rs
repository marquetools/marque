// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use smol_str::SmolStr;

use crate::span::Span;

/// One parser-recognized token plus its byte span in the original source.
///
/// Used by Phase 3 rules to surface byte-precise diagnostic spans without
/// re-parsing the source. The `text` field carries the literal token bytes
/// so rules that need the source content (E006, E007, E008 against migration
/// keys) can look up entries without threading `&[u8] source` through every
/// `Rule::check` signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSpan {
    pub kind: TokenKind,
    pub span: Span,
    pub text: SmolStr,
}

/// Discriminant for `TokenSpan`. Phase 3 rules read these to filter
/// token-span lookups by category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Classification level token (S, SECRET, TS, TOP SECRET, ...).
    Classification,
    /// SCI control token (SI, TK, HCS, ...).
    ///
    /// Emitted by the existing CVE exact-match path. For new structural
    /// parsing (spec 003-sci-compartments) see [`TokenKind::SciSystem`],
    /// [`TokenKind::SciCompartment`], and [`TokenKind::SciSubCompartment`].
    SciControl,
    /// Structural SCI control-system anchor (e.g., `SI`, `TK`, `123`).
    ///
    /// Emitted by the structural SCI parser introduced in spec
    /// 003-sci-compartments alongside the existing [`TokenKind::SciControl`]
    /// token for exact-CVE matches.
    SciSystem,
    /// Structural SCI compartment identifier (e.g., `G` in `SI-G`).
    SciCompartment,
    /// Structural SCI sub-compartment identifier (e.g., `ABCD` in `SI-G ABCD`).
    SciSubCompartment,
    /// Legacy SAR identifier token. Superseded by `SarIndicator` +
    /// `SarProgram` + `SarCompartment` + `SarSubCompartment` after the
    /// structural SAR model landed. No longer emitted by the parser.
    #[deprecated(note = "use SarIndicator/SarProgram/SarCompartment/SarSubCompartment")]
    SarIdentifier,
    /// SAR category indicator — `SAR-` or `SPECIAL ACCESS REQUIRED-`.
    /// One per SAR block; serves as the anchor for block-ordering rules.
    SarIndicator,
    /// SAR program identifier (e.g., `BP`, `BUTTER POPCORN`).
    SarProgram,
    /// SAR compartment identifier (e.g., `J12`).
    SarCompartment,
    /// SAR sub-compartment identifier (e.g., `J54`).
    SarSubCompartment,
    /// Atomic Energy Act marking token (RD, FRD, CNWDI, TFNI, SIGMA ##, etc.).
    AeaMarking,
    /// FGI marker token (`FGI`, `FGI DEU`, `FGI DEU GBR`).
    FgiMarker,
    /// One country trigraph/tetragraph appearing inside an FGI ownership
    /// list. The parser emits one of these spans per shape-admitted
    /// country token in `parse_fgi_marker_with_spans` alongside the
    /// block-level [`Self::FgiMarker`] span (the same dual-emission
    /// pattern [`Self::RelToTrigraph`] uses alongside
    /// [`Self::RelToBlock`]).
    ///
    /// Distinct from [`Self::RelToTrigraph`] so rules can read a single
    /// ownership-axis span without cross-axis contamination — issue
    /// #545's `FgiOwnershipTrigraphSuggestRule` reads only these spans.
    /// Distinct from [`Self::Unknown`] so E008 ("unrecognized token")
    /// does not co-fire on shape-admitted-but-unregistered ownership
    /// tokens like `XX` / `ZZZ`.
    ///
    /// Authority: CAPCO-2016 §H.7 p122 (FGI ownership-list grammar)
    /// + §A.6 p16 (single-space FGI list separator).
    FgiOwnershipTrigraph,
    /// Dissemination control token (NOFORN, NF, ORCON, OC, RELIDO, ...).
    DissemControl,
    /// Non-IC dissemination control token (LIMDIS, DS, SBU, LES, SSI, ...).
    NonIcDissem,
    /// REL TO country trigraph (USA, GBR, AUS, ...). One per token, not the
    /// whole REL TO list.
    RelToTrigraph,
    /// The full `REL TO ...` block text. Recorded so E013 can inspect the
    /// raw source for delimiter errors (spaces instead of commas).
    RelToBlock,
    /// DISPLAY ONLY country trigraph (AFG, IRQ, NATO, ...) — one per
    /// token, not the whole DISPLAY ONLY list. Parallel to
    /// [`Self::RelToTrigraph`]; the per-trigraph span is distinct
    /// from REL TO so future rules that operate on a single axis
    /// (e.g., per-§D.2 Table 3 row 25/26 banner roll-up) can locate
    /// only the relevant entries.
    DisplayOnlyTrigraph,
    /// The full `DISPLAY ONLY ...` block text, including the
    /// `DISPLAY ONLY` keyword and the comma-separated country list.
    /// Parallel to [`Self::RelToBlock`].
    DisplayOnlyBlock,
    /// Declassification exemption code in CAB or banner (25X1, 50X1-HUM).
    DeclassExemption,
    /// Declassification date in CAB or banner (YYYYMMDD or YYYY).
    DeclassDate,
    /// `//` separator between blocks. Recorded so E004 can detect extra/
    /// missing separator runs.
    Separator,
    /// A non-empty block that did not match any known token kind. E008 fires
    /// one diagnostic per `Unknown` entry.
    Unknown,
}
