// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`DeclassifyOnLattice`] — MaxDate semilattice (no top) over the
//! declassification-date axis.

use marque_ism::{CanonicalAttrs, IsmDate};
use marque_scheme::{JoinSemilattice, MeetSemilattice};

// ---------------------------------------------------------------------------
// DeclassifyOnLattice — MaxDate semilattice (no top)
// ---------------------------------------------------------------------------

/// Lattice form of the declassification-date axis:
/// `Option<IsmDate>` with `max_by(end_cmp)` join (the most-restrictive
/// / furthest-out date wins).
///
/// Per CAPCO-2016 §E.3 p32 "Multiple Sources and the Declassify On
/// Line Hierarchy" — the load-bearing rule is verbatim: *"The
/// 'Declassify On' line must reflect the single declassification
/// value that provides the longest classification duration of any
/// of the sources."* This is the explicit max-date aggregation rule
/// that grounds the lattice's `max_by(end_cmp)` semantic. ISOO §3.3
/// is the out-of-tree primary source CAPCO §E.3 derives from;
/// included as a cross-reference, not as primary authority per
/// Constitution VIII.
///
/// `IsmDate::end_cmp` compares the end-of-span of each precision tier,
/// so `Year(2003)` extends through December 31 and is "later" than
/// `Date(2003, 6, 15)` for the MaxDate lattice's most-conservative-
/// interpretation contract.
///
/// **Authority note**: §E.3 p32 is the proper authority for declass-date
/// aggregation, NOT §H.6 p104. §H.6 p104 is about RD/FRD/TFNI banner
/// roll-up; its rule for declass dates is the opposite ("Automatic
/// declassification of documents containing RD information is prohibited"),
/// which forbids a declass-date on RD documents entirely.
///
/// **`BoundedLattice` deliberately not implemented.** Dates are
/// open-vocab — no finite "top" date is realizable. Per the
/// `AeaSet` / `SciSet` / `SarSet` precedent in this module, the
/// established pattern for "no BoundedLattice when range is open"
/// is "implement `Lattice`, provide `empty()` / `default()` for
/// the bottom, leave `top()` undefined." `FgiSet` is NOT part of this
/// precedent — it does not implement `BoundedLattice` at all.
///
/// §-authority (CAPCO-2016.md):
/// - §E.3 p32 (Multiple Sources and the Declassify On Line Hierarchy
///   — "single declassification value that provides the longest
///   classification duration of any of the sources").
/// - ISOO §3.3 (out-of-tree primary; included for cross-reference,
///   not as primary source per Constitution VIII).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclassifyOnLattice(Option<IsmDate>);

impl DeclassifyOnLattice {
    /// An empty declassify-on — the lattice bottom.
    pub fn empty() -> Self {
        Self(None)
    }

    /// Construct a `DeclassifyOnLattice` from an `Option<IsmDate>`.
    pub fn new(d: Option<IsmDate>) -> Self {
        Self(d)
    }

    /// Construct from a `CanonicalAttrs` slice — picks the maximum
    /// declassify-on date across portions per `IsmDate::end_cmp`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        let max = portions
            .iter()
            .filter_map(|p| p.declassify_on.clone())
            .max_by(|a, b| a.end_cmp(b));
        Self(max)
    }

    /// Consume into the inner `Option<IsmDate>`.
    pub fn into_inner(self) -> Option<IsmDate> {
        self.0
    }

    /// Borrow the inner `Option<IsmDate>`.
    pub fn as_inner(&self) -> Option<&IsmDate> {
        self.0.as_ref()
    }
}

impl JoinSemilattice for DeclassifyOnLattice {
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, x) | (x, None) => Self(x.clone()),
            (Some(a), Some(b)) => {
                if a.end_cmp(b).is_ge() {
                    Self(Some(a.clone()))
                } else {
                    Self(Some(b.clone()))
                }
            }
        }
    }
}

impl MeetSemilattice for DeclassifyOnLattice {
    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => {
                if a.end_cmp(b).is_le() {
                    Self(Some(a.clone()))
                } else {
                    Self(Some(b.clone()))
                }
            }
        }
    }
}
