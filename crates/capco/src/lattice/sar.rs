// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`SarSet`] — lattice over the full SAR category state.

use marque_ism::{SarCompartment, SarIndicator, SarMarking, SarProgram};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
use smol_str::SmolStr;

use super::helpers::{HierarchicalTreeSet, sorted_compartment_items};

// ---------------------------------------------------------------------------
// SarSet — lattice over the full SAR category state
// ---------------------------------------------------------------------------

/// The full SAR state on a document / portion, in lattice form.
///
/// CAPCO caps SAR cardinality at one block per marking, but across
/// portions on a page the programs / compartments / sub-compartments
/// compose. This type joins by unioning at every hierarchical level;
/// meet follows the §3.3a policy (b) equal-depth intersection.
///
/// Round-trips with `Option<SarMarking>` via [`SarSet::from_marking`]
/// and [`SarSet::to_marking`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SarSet {
    /// program id → compartment id → set of sub-compartment ids.
    programs: HierarchicalTreeSet<SmolStr>,
}

impl SarSet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_marking(marking: Option<&SarMarking>) -> Self {
        let mut out = Self::empty();
        let Some(sar) = marking else {
            return out;
        };
        for prog in sar.programs.iter() {
            let comps = out.programs.entry_outer(prog.identifier.clone());
            for comp in prog.compartments.iter() {
                let subs = comps.entry(comp.identifier.clone()).or_default();
                subs.extend(comp.sub_compartments.iter().cloned());
            }
        }
        out
    }

    /// Render this set back to an `Option<SarMarking>` with programs /
    /// compartments / sub-compartments sorted per §H.5 numeric-first
    /// order. Indicator defaults to `Abbrev` (the banner roll-up
    /// convention).
    pub fn to_marking(&self) -> Option<SarMarking> {
        if self.programs.is_empty() {
            return None;
        }
        // Helpers carry the LA-4 inline-capacity sizing (see
        // `SciSet::to_markings` for the rationale): programs at
        // inline-4, compartments at inline-8, sub-compartments at
        // inline-4 — all heap-free for ordinary documents.
        let entries = self.programs.sorted_entries(|k| k.as_str());
        let built_programs: Box<[SarProgram]> = entries
            .into_iter()
            .map(|(pid, comp_map)| {
                let compartments: Box<[SarCompartment]> = sorted_compartment_items(comp_map)
                    .into_iter()
                    .map(|(cid, subs)| SarCompartment::new(cid.clone(), subs))
                    .collect();
                SarProgram::new(pid.clone(), compartments)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Some(SarMarking::new(SarIndicator::Abbrev, built_programs))
    }

    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }
}

impl JoinSemilattice for SarSet {
    /// Component-wise union: merge programs, compartments, and
    /// sub-compartments. Delegates to the internal
    /// `HierarchicalTreeSet::join_with` method in `super::helpers`.
    fn join(&self, other: &Self) -> Self {
        Self {
            programs: self.programs.join_with(&other.programs),
        }
    }
}

impl MeetSemilattice for SarSet {
    /// Component-wise equal-depth intersection per §3.3a policy (b).
    /// Delegates to the internal `HierarchicalTreeSet::meet_with`
    /// method in `super::helpers`.
    fn meet(&self, other: &Self) -> Self {
        Self {
            programs: self.programs.meet_with(&other.programs),
        }
    }
}

// `SarSet` intentionally does **not** implement `BoundedLattice`: SAR
// program identifiers are agency-assigned codewords, an open set. An
// "empty" top would violate the `BoundedLattice::top ⊔ a = top`
// contract on any non-empty `a`. Use [`SarSet::empty`] /
// [`SarSet::default`] when you need the bottom, and
// [`JoinSemilattice::join`] / [`MeetSemilattice::meet`] for
// composition.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::type_complexity)] // Test-fixture DSL; explicit shape is clearer than a newtype.
mod tests {
    use super::*;
    use crate::lattice::test_support::*;

    #[test]
    fn sar_set_join_unions_programs() {
        let a = SarSet::from_marking(Some(&mk_sar_portion(vec![("BP", vec![])])));
        let b = SarSet::from_marking(Some(&mk_sar_portion(vec![("CD", vec![])])));
        let j = a.join(&b);
        let out = j.to_marking().expect("nonempty");
        let ids: Vec<&str> = out.programs.iter().map(|p| p.identifier.as_str()).collect();
        assert_eq!(ids, vec!["BP", "CD"]);
    }

    #[test]
    fn sar_set_meet_intersects_compartments() {
        let a = SarSet::from_marking(Some(&mk_sar_portion(vec![(
            "BP",
            vec![("J12", vec!["J54"])],
        )])));
        let b = SarSet::from_marking(Some(&mk_sar_portion(vec![(
            "BP",
            vec![("J12", vec!["J54", "K15"])],
        )])));
        let m = a.meet(&b);
        let out = m.to_marking().expect("nonempty");
        assert_eq!(out.programs[0].compartments[0].sub_compartments.len(), 1);
    }

    // SarSet — round-trip, accessors, meet edge cases

    #[test]
    fn sar_set_empty_roundtrip_returns_none() {
        let set = SarSet::from_marking(None);
        assert!(set.is_empty());
        assert!(set.to_marking().is_none());
    }

    #[test]
    fn sar_set_is_empty_false_on_populated() {
        let set = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new("BP", Box::new([]))].into_boxed_slice(),
        )));
        assert!(!set.is_empty());
    }

    #[test]
    fn sar_set_round_trip_with_nested_hierarchy() {
        let sar = SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new(
                "BP",
                vec![SarCompartment::new(
                    "J12",
                    Box::new([SmolStr::from("K20"), SmolStr::from("K15")]),
                )]
                .into_boxed_slice(),
            )]
            .into_boxed_slice(),
        );
        let set = SarSet::from_marking(Some(&sar));
        let out = set.to_marking().expect("nonempty");
        // Indicator normalizes to Abbrev on roundtrip.
        assert_eq!(out.indicator, SarIndicator::Abbrev);
        // Sub-compartments come out in numeric-first sort order.
        let subs: Vec<&str> = out.programs[0].compartments[0]
            .sub_compartments
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(subs, vec!["K15", "K20"]);
    }

    #[test]
    fn sar_set_meet_drops_programs_not_on_both_sides() {
        let a = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new("BP", Box::new([]))].into_boxed_slice(),
        )));
        let b = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new("CD", Box::new([]))].into_boxed_slice(),
        )));
        assert!(a.meet(&b).is_empty());
    }

    #[test]
    fn sar_set_meet_common_program_keeps_entry() {
        let a = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new("BP", Box::new([]))].into_boxed_slice(),
        )));
        let b = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new("BP", Box::new([]))].into_boxed_slice(),
        )));
        let m = a.meet(&b);
        assert!(!m.is_empty());
    }
}
