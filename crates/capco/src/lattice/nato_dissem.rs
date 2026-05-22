// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`NatoDissemSet`] — trivial union over the NATO-attributed dissem axis.

use marque_ism::{CanonicalAttrs, DissemControl};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// NatoDissemSet — trivial union over the NATO-attributed dissem axis
// ---------------------------------------------------------------------------

/// Lattice form of the NATO-attributed IC dissem axis: a `BTreeSet`
/// of `DissemControl` tokens with **no overlays**.
///
/// Per CAPCO-2016 p41 (Table — Authority-Reciprocity-Holdback by
/// Registered Marking — for the NATO-reciprocity case), NATO
/// contributes only `ORCON-NATO` and `REL TO` to the IC dissem axis,
/// both of which compose by simple BTreeSet union at the banner
/// level. None of the US-context exceptions (OC-USGOV drop, FOUO
/// drop, DSEN override, NF injection, RELIDO unanimity) apply —
/// those are §H.8 US-attributed behaviors, and the NATO reciprocity
/// boundary at p41 explicitly carves them out.
///
/// **`BoundedLattice` deliberately not implemented.** The NATO
/// dissem vocabulary is closed at two elements today, but the
/// underlying `DissemControl` enum is shared with US dissem so the
/// namespace bound is loose; bottom = empty set, top is unsafe to
/// claim. The SciSet/SarSet/AeaSet precedent for open-vocab applies
/// (M-25 PR 4b-B 7th-pass — `FgiSet` was previously listed in this
/// precedent; B-1 PR 4b-B 8th-pass retired `FgiSet`'s
/// `BoundedLattice` impl — `FgiSet` does NOT implement
/// `BoundedLattice`. See [`super::dissem::DissemSet`] for rationale.)
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - p41 (NATO reciprocity table — NATO dissem set is the
///   intersection of NATO-permitted-and-IC-compatible markings).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NatoDissemSet {
    set: BTreeSet<DissemControl>,
}

impl NatoDissemSet {
    /// An empty NATO dissem set — the lattice bottom.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct from a slice of `CanonicalAttrs` — plain BTreeSet
    /// union over per-portion `dissem_nato`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        let mut set = BTreeSet::new();
        for p in portions {
            for t in p.dissem_nato.iter() {
                set.insert(*t);
            }
        }
        Self { set }
    }

    /// Borrow the underlying BTreeSet.
    pub fn as_set(&self) -> &BTreeSet<DissemControl> {
        &self.set
    }

    /// Render to a `Box<[DissemControl]>` in BTreeSet natural order.
    pub fn into_boxed_slice(self) -> Box<[DissemControl]> {
        self.set.into_iter().collect::<Vec<_>>().into_boxed_slice()
    }

    /// Borrow as a `Vec` for compatibility with existing
    /// `PageContext::expected_dissem_nato`-shaped APIs.
    pub fn to_vec(&self) -> Vec<DissemControl> {
        self.set.iter().copied().collect()
    }
}

impl JoinSemilattice for NatoDissemSet {
    fn join(&self, other: &Self) -> Self {
        let mut set = self.set.clone();
        set.extend(other.set.iter().copied());
        Self { set }
    }
}

impl MeetSemilattice for NatoDissemSet {
    fn meet(&self, other: &Self) -> Self {
        let set: BTreeSet<DissemControl> = self.set.intersection(&other.set).copied().collect();
        Self { set }
    }
}
