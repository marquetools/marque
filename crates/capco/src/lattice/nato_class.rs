// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`NatoClassLattice`] — bounded OrdMax over the NATO classification
//! chain (`NU < NR < NC < NS < CTS`).

use marque_ism::{CanonicalAttrs, MarkingClassification, NatoClassification};
use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};

// ---------------------------------------------------------------------------
// NatoClassLattice — bounded OrdMax over the NATO chain
// ---------------------------------------------------------------------------

/// Lattice form of the NATO classification axis:
/// `Option<NatoClassification>` with `OrdMax` over
/// `NU < NR < NC < NS < CTS` per CAPCO-2016 §H.2 p55.
///
/// **Pure-NATO documents only.** This lattice shadows
/// `ClassificationLattice` for documents with no US portions.
/// Mixed US+NATO documents reciprocally-raise at portion-parse time
/// via the existing §H.7 pp123-125 rule; `non_us_classification` is
/// `None` at banner for such pages.
///
/// `BoundedLattice` is implemented: top = `Some(CosmicTopSecret)`,
/// bottom = `None`. The NATO chain is a closed five-element ladder
/// (no agency-extensibility, unlike US classifications which can
/// theoretically receive new tiers).
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.2 p55 (Non-US Protective Markings — refers to NATO chain).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NatoClassLattice(Option<NatoClassification>);

impl NatoClassLattice {
    /// An empty NATO classification — the lattice bottom.
    pub fn empty() -> Self {
        Self(None)
    }

    /// Construct a `NatoClassLattice` from an `Option<NatoClassification>`.
    pub fn new(c: Option<NatoClassification>) -> Self {
        Self(c)
    }

    /// Construct from a `CanonicalAttrs` slice — picks `Nato(_)`
    /// portions and joins by `OrdMax` over the NATO chain. Returns
    /// `empty()` if no portion carries a NATO classification.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        let max = portions
            .iter()
            .filter_map(|p| match &p.classification {
                Some(MarkingClassification::Nato(n)) => Some(*n),
                _ => None,
            })
            .max_by_key(|n| n.us_equivalent());
        Self(max)
    }

    /// Consume into the inner `Option<NatoClassification>`.
    pub fn into_inner(self) -> Option<NatoClassification> {
        self.0
    }

    /// Borrow the inner `Option<NatoClassification>`.
    pub fn as_inner(&self) -> Option<NatoClassification> {
        self.0
    }
}

impl JoinSemilattice for NatoClassLattice {
    fn join(&self, other: &Self) -> Self {
        match (self.0, other.0) {
            (None, x) | (x, None) => Self(x),
            (Some(a), Some(b)) => {
                if a.us_equivalent() >= b.us_equivalent() {
                    Self(Some(a))
                } else {
                    Self(Some(b))
                }
            }
        }
    }
}

impl MeetSemilattice for NatoClassLattice {
    fn meet(&self, other: &Self) -> Self {
        match (self.0, other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => {
                if a.us_equivalent() <= b.us_equivalent() {
                    Self(Some(a))
                } else {
                    Self(Some(b))
                }
            }
        }
    }
}

impl BoundedJoinSemilattice for NatoClassLattice {
    fn bottom() -> Self {
        Self(None)
    }
}

impl BoundedMeetSemilattice for NatoClassLattice {
    fn top() -> Self {
        Self(Some(NatoClassification::CosmicTopSecret))
    }
}
