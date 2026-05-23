// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use smol_str::SmolStr;

use crate::generated::values::{SciControl, SciControlBare};

// ===========================================================================
// SCI structural types (spec 003-sci-compartments)
// ===========================================================================

/// A fully-parsed SCI category-block entry.
///
/// A banner or portion may carry multiple `SciMarking` entries separated by
/// `/` within one SCI category block (e.g., `//SI-G/TK-BLFH//`).
///
/// Construction is restricted to [`SciMarking::new`] (the struct is
/// `#[non_exhaustive]`) so new fields can be added without breaking the
/// parser.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SciMarking {
    /// The control-system anchor. One of the published bare control
    /// systems (see [`SciControlBare`]) or a structurally-parsed custom
    /// value.
    pub system: SciControlSystem,

    /// Compartments in source order. Sort-order validation is the concern
    /// of CAPCO rule E033 (not the parser).
    pub compartments: Box<[SciCompartment]>,

    /// If the `{system}-{first_compartment}` composite exactly matches an
    /// ODNI CVE value (e.g., `SI-G`, `HCS-P`, `TK-BLFH`), this records the
    /// matching [`SciControl`] variant. Only populated when the matching
    /// compartment has NO sub-compartments — sub-compartments imply the
    /// compound is a structural anchor rather than a CVE atom. `None`
    /// otherwise.
    pub canonical_enum: Option<SciControl>,
}

impl SciMarking {
    /// Construct a new `SciMarking`. Used by the parser (`marque-core`) to
    /// populate [`CanonicalAttrs::sci_markings`].
    pub fn new(
        system: SciControlSystem,
        compartments: Box<[SciCompartment]>,
        canonical_enum: Option<SciControl>,
    ) -> Self {
        Self {
            system,
            compartments,
            canonical_enum,
        }
    }
}

/// Which kind of SCI control system a [`SciMarking`] anchors on.
///
/// One of three variants: a published bare system drawn from the live
/// ODNI CVE, an agency-allocated custom identifier (per CAPCO-2016
/// §A.6 p15), or a NATO Special Access Program identifier (CAPCO-2016
/// §G.2 p40 + §H.7 p127).
///
/// NATO SAPs (`BOHEMIA`, `BALK`) are not in the ODNI ISM CVE — they're
/// CAPCO-only registered NATO control markings. They render standalone
/// in their own `//`-separated category (in the SCI block position),
/// e.g. `//CTS//BOHEMIA` (banner) / `(//CTS//BOHEMIA)` (portion). See
/// the §H.7 p127 worked example for ordering: US class // US-SCI /
/// BALK / BOHEMIA // ... .
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SciControlSystem {
    /// One of the published bare control systems.
    Published(SciControlBare),
    /// An agency-allocated system matching `[A-Z0-9]{2,5}` (per CAPCO-2016
    /// §A.6 p15 `123` example). Stores the raw text exactly as it appeared
    /// in the source.
    Custom(SmolStr),
    /// One of the two NATO Special Access Programs registered in
    /// CAPCO-2016 §G.2 p40 (`BOHEMIA`, `BALK`). NATO SAPs travel in
    /// the SCI category position but are CAPCO-only (no ODNI CVE
    /// entry).
    NatoSap(NatoSap),
}

/// Registered NATO Special Access Programs per CAPCO-2016 §G.2 p40 +
/// §H.7 Appendix B.
///
/// Both `Bohemia` and `Balk` render standalone (no `SAR-` prefix) and
/// are typically CTS-only. The §H.7 p127 worked example renders BALK
/// before BOHEMIA when both are present (numeric-then-alpha ordering
/// per §A.6 p15-16; `BALK < BOHEMIA` lexicographically).
/// Derive `Ord` so [`SciControlSystem::NatoSap`] participates in the
/// numeric-then-alpha ordering used by the lattice-side
/// `SciSet::from_markings` SCI roll-up via `SystemKey`. The variants
/// are declared `Balk` then
/// `Bohemia` (alphabetical text), so the derived `Ord` orders `Balk <
/// Bohemia` — which matches `as_str()`-based lexicographic ordering.
/// `as_str()` text remains the sort key consulted by the renderer and
/// roll-up; the derived `Ord` is a co-monotone alignment, not a
/// separate canonical order.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NatoSap {
    /// BALK — NATO SAP per CAPCO-2016 §G.2 p40 (exercise replacement
    /// for BOHEMIA per legacy Combined Communications-Electronics
    /// Board guidance referenced in §G.2). BALK sorts before BOHEMIA
    /// alphabetically — see the §H.7 p127 worked example.
    Balk,
    /// BOHEMIA — NATO SAP per CAPCO-2016 §G.2 p40.
    Bohemia,
}

impl NatoSap {
    /// Canonical name as used in both banner and portion forms
    /// (CAPCO-2016 §G.1 Table 4 p38 row "ATOMAL/BALK/BOHEMIA" — same-
    /// form across title, banner-abbrev, and portion columns).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bohemia => "BOHEMIA",
            Self::Balk => "BALK",
        }
    }
}

/// A single compartment under an SCI control system.
///
/// Compartments carry an identifier plus zero or more sub-compartments in
/// source order. Construction is restricted to [`SciCompartment::new`]
/// (the struct is `#[non_exhaustive]`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SciCompartment {
    /// Compartment identifier (alphanumeric). Example: `G` in `SI-G`.
    pub identifier: SmolStr,
    /// Sub-compartments in source order. Example: `ABCD`, `DEFG` in
    /// `SI-G ABCD DEFG`.
    pub sub_compartments: Box<[SmolStr]>,
}

impl SciCompartment {
    /// Construct a new `SciCompartment`. Used by the parser to populate
    /// [`SciMarking::compartments`].
    pub fn new(identifier: impl Into<SmolStr>, sub_compartments: Box<[SmolStr]>) -> Self {
        Self {
            identifier: identifier.into(),
            sub_compartments,
        }
    }
}
