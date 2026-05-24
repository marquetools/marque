// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use smallvec::SmallVec;

use super::{CountryCode, NatoClassification};

// ===========================================================================
// Classification types
// ===========================================================================

/// The classification system and level for a marking.
///
/// A marking has exactly one classification system. When the parser finds
/// two (e.g., `SECRET//NATO SECRET//...`), it resolves to [`Conflict`](Self::Conflict).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkingClassification {
    /// US IC classification.
    Us(Classification),
    /// Non-US (FGI) classification: `//GBR S//...`
    Fgi(FgiClassification),
    /// NATO classification: `//NS//...`
    Nato(NatoClassification),
    /// JOINT classification (US co-owned): `//JOINT S USA GBR//...`
    Joint(JointClassification),
    /// Parser found two classification systems in one marking.
    ///
    /// US wins, upgraded to the greater of the two levels.
    /// The foreign part is preserved so rules can suggest the FGI fix.
    ///
    /// Example: `SECRET//COSMIC TOP SECRET//REL TO USA, NATO`
    /// → `us: TopSecret`, `foreign: Nato(CosmicTopSecret)`
    /// → fix: `TOP SECRET//FGI NATO//REL TO USA, NATO`
    Conflict {
        /// Resolved US classification (max of both levels).
        us: Classification,
        /// The foreign classification that should become an FGI marker.
        foreign: Box<ForeignClassification>,
    },
}

impl MarkingClassification {
    /// The effective classification level for ordering purposes, regardless of
    /// classification system.
    ///
    /// NATO levels are mapped to their US equivalents via
    /// [`NatoClassification::us_equivalent`]. All systems use the
    /// [`Classification`] ladder for comparison so that `Iterator::max()` on
    /// a mixed set of portions returns the most restrictive level overall.
    pub fn effective_level(&self) -> Classification {
        match self {
            Self::Us(c) => *c,
            Self::Fgi(f) => f.level,
            Self::Nato(n) => n.us_equivalent(),
            Self::Joint(j) => j.level,
            Self::Conflict { us, .. } => *us,
        }
    }
}

impl Default for MarkingClassification {
    fn default() -> Self {
        Self::Us(Classification::Unclassified)
    }
}

/// The non-US classification in a [`MarkingClassification::Conflict`].
///
/// Preserves enough information for rules to generate the FGI fix:
/// the foreign system, its level, and any associated countries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeignClassification {
    Fgi(FgiClassification),
    Nato(NatoClassification),
    Joint(JointClassification),
}

// ---------------------------------------------------------------------------
// Classification level (US ladder + RESTRICTED for foreign interop)
// ---------------------------------------------------------------------------

/// Classification level. Ordered by restrictiveness: U < R < C < S < TS.
///
/// Includes `Restricted` for foreign-origin markings — many non-US
/// classification systems (and NATO) have a RESTRICTED level between
/// UNCLASSIFIED and CONFIDENTIAL.
///
/// The derived `Ord` reflects restrictiveness ordering so that
/// `Iterator::max()` returns the most restrictive level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Classification {
    Unclassified,
    Restricted,
    Confidential,
    Secret,
    TopSecret,
}

impl Classification {
    /// Banner form (full words, no abbreviations).
    pub fn banner_str(self) -> &'static str {
        match self {
            Self::Unclassified => "UNCLASSIFIED",
            Self::Restricted => "RESTRICTED",
            Self::Confidential => "CONFIDENTIAL",
            Self::Secret => "SECRET",
            Self::TopSecret => "TOP SECRET",
        }
    }

    /// Portion form (abbreviation used in portion markings).
    pub fn portion_str(self) -> &'static str {
        match self {
            Self::Unclassified => "U",
            Self::Restricted => "R",
            Self::Confidential => "C",
            Self::Secret => "S",
            Self::TopSecret => "TS",
        }
    }
}

// ---------------------------------------------------------------------------
// FGI classification (non-US, country-prefixed)
// ---------------------------------------------------------------------------

/// Non-US (FGI) classification.
///
/// Two forms exist:
///
/// - **Source-acknowledged**: country trigraph(s) identify the originator.
///   `//GBR S//REL TO USA, GBR` (single owner) or `//CAN GBR S` (multiple
///   producers per the §H.7 p123 worked example `(//CAN GBR S)` and §H.7
///   p124 prose authorizing multi-country FGI alphabetically space-
///   separated; ICD 206 commingling clause).
/// - **Source-concealed**: `FGI` replaces the country trigraph(s) when
///   the originating country is sensitive. `//FGI S//REL TO USA, GBR`
///   An empty `countries` list indicates source-concealed FGI.
///
/// Countries are space-delimited in the source marking.
///
/// # Disambiguation from sibling axes
///
/// - [`FgiMarker`] is a **dissem-axis** marker for commingled US+FGI
///   banner forms (`SECRET//FGI GBR DEU//...`), separate from the
///   classification axis modeled here. The two can coexist.
/// - [`JointClassification`] models US-inclusive co-ownership where
///   the US is itself one of the producers. `FgiClassification` is the
///   non-US classification axis and does not include the US.
///
/// # Banner aggregation
///
/// If a document contains **any** source-concealed FGI portions alongside
/// source-acknowledged FGI portions, the banner must use `FGI` without
/// country codes — revealing the country list would compromise the
/// concealed source. This rule is enforced at the `PageContext` level
/// during banner validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FgiClassification {
    /// Originating countries (space-delimited in source).
    /// Empty for source-concealed FGI (`//FGI S//...`).
    pub countries: Box<[CountryCode]>,
    /// Classification level (includes RESTRICTED).
    pub level: Classification,
}

// ---------------------------------------------------------------------------
// JOINT classification
// ---------------------------------------------------------------------------

/// JOINT classification: US is co-owner with other nations.
///
/// `//JOINT S USA GBR//REL TO USA, GBR`
///
/// Country list is space-delimited (NOT comma-delimited like REL TO).
/// Must include USA. All JOINT participants must also appear in REL TO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JointClassification {
    /// Classification level (US ladder, includes RESTRICTED).
    pub level: Classification,
    /// Co-owning countries (space-delimited in source). Must include USA.
    pub countries: Box<[CountryCode]>,
}

// ---------------------------------------------------------------------------
// FGI marker (in US-classified markings)
// ---------------------------------------------------------------------------

/// FGI marker in a US-classified marking: `FGI` (source-concealed) or
/// `FGI [LIST]` (source-acknowledged).
///
/// Appears in the FGI block (after SAR, before dissem controls) when a
/// US-classified document references foreign government information.
///
/// This is NOT the same as [`FgiClassification`] — that represents a
/// marking where the classification itself IS foreign. This marker says
/// "this US-classified marking contains foreign government information."
///
/// # Authoritative source
///
/// CAPCO-2016 §H.7 p122 defines two banner forms:
///
/// | Variant | Banner | Portion |
/// |---|---|---|
/// | Source-acknowledged | `FOREIGN GOVERNMENT INFORMATION [LIST]` (abbr `FGI [LIST]`) | with country trigraphs |
/// | Source-concealed    | `FOREIGN GOVERNMENT INFORMATION` (abbr `FGI`)              | without country list |
///
/// Concealment is used when revealing the country list would compromise
/// the foreign source. If a page mixes concealed + acknowledged portions,
/// the banner must use the concealed form (bare `FGI`).
///
/// # Why an enum, not a struct with `Box<[CountryCode]>`
///
/// The previous shape `FgiMarker { countries: Box<[CountryCode]> }`
/// made `countries: []` ambiguous between two meanings:
///
///   1. Lawful source-concealed FGI (the `FGI` banner form).
///   2. A parser failure that silently dropped a country list — the
///      classic open-vocabulary corruption case (issue #280).
///
/// The explicit discriminant retires that collision. `Acknowledged`
/// is constructed only via [`acknowledged`],
/// which rejects an empty country list, so the corrupt shape is
/// type-system-unrepresentable.
///
/// [`acknowledged`]: FgiMarker::acknowledged
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FgiMarker {
    /// Source-concealed FGI per CAPCO-2016 §H.7 p122.
    ///
    /// Banner: `FOREIGN GOVERNMENT INFORMATION` (abbr `FGI`) with no
    /// country list. Used when revealing the country list would
    /// compromise the foreign source.
    SourceConcealed,

    /// Source-acknowledged FGI per CAPCO-2016 §H.7 p123.
    ///
    /// Banner: `FOREIGN GOVERNMENT INFORMATION [LIST]` (abbr
    /// `FGI [LIST]`). The country list is non-empty by construction —
    /// see [`FgiMarker::acknowledged`] for the public constructor that
    /// enforces the invariant.
    ///
    /// Marked `#[non_exhaustive]` so external crates **cannot**
    /// construct this variant via struct-literal syntax. This is
    /// load-bearing for the discriminant invariant: it forces external
    /// callers through [`FgiMarker::acknowledged`], which rejects the empty
    /// country list. Pattern matching from outside the crate still
    /// works with the `..` rest pattern:
    /// `FgiMarker::Acknowledged { countries, .. }`. Internal
    /// (in-crate) construction is unrestricted, but only the
    /// `acknowledged` constructor reaches the variant in this crate's
    /// codepaths — see callsites for the audit.
    #[non_exhaustive]
    Acknowledged {
        /// One or more country trigraphs/tetragraphs. Non-empty by
        /// construction (enforced by [`FgiMarker::acknowledged`]).
        ///
        /// `SmallVec<[CountryCode; 4]>` keeps the typical FGI list
        /// (≤4 codes) inline — no heap allocation on the parsing
        /// hot path (Constitution Principle II).
        countries: SmallVec<[CountryCode; 4]>,
    },
}

impl FgiMarker {
    /// Construct an acknowledged FGI marker from a non-empty list of
    /// country codes. Returns `None` if the list is empty — at that
    /// point the caller has either parser-failure data (return `None`
    /// to the caller and let it surface as a diagnostic) or the source
    /// is genuinely concealed (use [`FgiMarker::SourceConcealed`]
    /// directly).
    ///
    /// Authority: CAPCO-2016 §H.7 p122 (the `FGI [LIST]` banner form
    /// requires a non-empty `[LIST]`).
    pub fn acknowledged<I>(countries: I) -> Option<Self>
    where
        I: IntoIterator<Item = CountryCode>,
    {
        let countries: SmallVec<[CountryCode; 4]> = countries.into_iter().collect();
        if countries.is_empty() {
            None
        } else {
            Some(Self::Acknowledged { countries })
        }
    }

    /// Country trigraphs for this marker.
    ///
    /// - `SourceConcealed` → empty slice (no countries by definition;
    ///   distinguishable from a parse failure because the variant
    ///   itself is the disambiguator).
    /// - `Acknowledged { countries }` → `&countries[..]`, guaranteed
    ///   non-empty by the [`FgiMarker::acknowledged`] constructor.
    pub fn countries(&self) -> &[CountryCode] {
        match self {
            Self::SourceConcealed => &[],
            Self::Acknowledged { countries } => countries.as_slice(),
        }
    }

    /// `true` iff this is a source-concealed marker (the bare `FGI`
    /// banner form, CAPCO-2016 §H.7 p122).
    pub fn is_concealed(&self) -> bool {
        matches!(self, Self::SourceConcealed)
    }
}
