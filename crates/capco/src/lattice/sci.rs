// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`SciSet`] — lattice over the full SCI category state, plus the
//! [`sci_controls_from_markings`] free helper that produces the flat
//! CVE-enum projection consumed by the back-compat
//! `CanonicalAttrs.sci_controls` field.

use marque_ism::{CanonicalAttrs, SciCompartment, SciControlSystem, SciMarking};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
use smol_str::SmolStr;
use std::collections::BTreeSet;

use super::helpers::{HierarchicalTreeSet, sorted_compartment_items};

// ---------------------------------------------------------------------------
// SciSet — lattice over the full SCI category state
// ---------------------------------------------------------------------------

/// The set of SCI markings on a document / portion, in lattice form.
///
/// Internally a map from control-system to compartment → sub-compartment
/// tree; `join` is component-wise union and `meet` is component-wise
/// equal-depth intersection per the module-level §3.3a policy. Ordering
/// of output is deterministic (BTreeMap / BTreeSet backing storage).
///
/// `SciSet` round-trips with `[SciMarking]` via [`SciSet::from_markings`]
/// and [`SciSet::to_markings`]. The byte-level equivalence with the
/// retired `PageContext::expected_sci_markings` output was the Phase B
/// verification gate; post-PR-4b-E `SciSet::to_markings()` is the
/// production roll-up path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SciSet {
    /// system → compartment identifier → set of sub-compartment
    /// identifiers.
    systems: HierarchicalTreeSet<SystemKey>,
}

/// Stable ordering key for `SciControlSystem`. Published variants and
/// Custom variants are interleaved on their textual forms — the final
/// emission order is re-sorted per CAPCO §A.6 p15 (numeric first) when
/// converting back to `[SciMarking]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SystemKey {
    Published(marque_ism::SciControlBare),
    Custom(SmolStr),
    NatoSap(marque_ism::NatoSap),
}

impl SystemKey {
    fn from_system(sys: &SciControlSystem) -> Self {
        match sys {
            SciControlSystem::Published(b) => SystemKey::Published(*b),
            SciControlSystem::Custom(s) => SystemKey::Custom(s.clone()),
            SciControlSystem::NatoSap(sap) => SystemKey::NatoSap(*sap),
        }
    }

    fn text(&self) -> &str {
        match self {
            SystemKey::Published(b) => b.as_str(),
            SystemKey::Custom(s) => s.as_str(),
            SystemKey::NatoSap(sap) => sap.as_str(),
        }
    }

    fn into_system(self) -> SciControlSystem {
        match self {
            SystemKey::Published(b) => SciControlSystem::Published(b),
            SystemKey::Custom(s) => SciControlSystem::Custom(s),
            SystemKey::NatoSap(sap) => SciControlSystem::NatoSap(sap),
        }
    }
}

impl SciSet {
    /// An empty SCI set — the lattice bottom.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct an `SciSet` from a slice of `SciMarking`. Compartment
    /// identifiers collide on equal-string basis (the same identifier
    /// on different systems stays distinct because it's keyed under
    /// the system).
    pub fn from_markings(markings: &[SciMarking]) -> Self {
        Self::from_markings_iter(markings.iter())
    }

    /// Construct an `SciSet` from an iterator over `SciMarking` references.
    ///
    /// Prefer this over [`Self::from_markings`] when the caller already has an
    /// iterator, such as a flattened per-portion slice — avoids the intermediate
    /// `Vec<SciMarking>` allocation. CLONE-1 performance fix (issue #606).
    pub fn from_markings_iter<'a>(markings: impl Iterator<Item = &'a SciMarking>) -> Self {
        let mut out = Self::empty();
        for m in markings {
            let key = SystemKey::from_system(&m.system);
            let comp_map = out.systems.entry_outer(key);
            if m.compartments.is_empty() {
                // Bare system — ensure the entry exists so a subsequent
                // rollup preserves the bare form.
                continue;
            }
            for comp in m.compartments.iter() {
                let sub_set = comp_map.entry(comp.identifier.clone()).or_default();
                sub_set.extend(comp.sub_compartments.iter().cloned());
            }
        }
        out
    }

    /// Render this set back to a boxed slice of `SciMarking` in §A.6 p15
    /// ordering (numeric-prefixed first, then alphabetic). Output
    /// `canonical_enum` is always `None` — the CVE compound form is
    /// per-portion only; a rolled-up structural projection has no
    /// single corresponding enum variant.
    pub fn to_markings(&self) -> Box<[SciMarking]> {
        // LA-2 empty-axis fast-path: skip all sorting / allocation
        // when no SCI markings were accumulated (the common case on
        // documents with no SCI portions).
        if self.is_empty() {
            return Box::default();
        }
        // The shared `sorted_entries` / `sorted_compartment_items`
        // helpers (HierarchicalTreeSet, PR 613) carry the SmallVec
        // inline-capacity sizing from PR 614 — systems/programs at
        // inline-4, compartments at inline-8, sub-compartments at
        // inline-4 — so scratch buffers stay on the stack for ordinary
        // classified documents.
        let entries = self.systems.sorted_entries(|k| k.text());
        let mut out: Vec<SciMarking> = Vec::with_capacity(entries.len());
        for (sys_key, comp_map) in entries {
            let compartments: Box<[SciCompartment]> = sorted_compartment_items(comp_map)
                .into_iter()
                .map(|(id, subs)| SciCompartment::new(id.clone(), subs))
                .collect();
            out.push(SciMarking::new(
                sys_key.clone().into_system(),
                compartments,
                None,
            ));
        }
        out.into_boxed_slice()
    }

    /// Whether this set and `other` share at least one control system.
    /// Exposed for Phase-C constraint work that needs overlap
    /// semantics distinct from the equal-depth meet.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.systems.keys().any(|k| other.systems.contains_key(k))
    }

    /// Compartments (as `(system-text, compartment-id)` pairs) present
    /// on both sides. Exposed for Phase-C constraint work.
    pub fn common_compartments(&self, other: &Self) -> Vec<(SmolStr, SmolStr)> {
        let mut out = Vec::new();
        for (sys, comps) in self.systems.iter() {
            let Some(other_comps) = other.systems.get(sys) else {
                continue;
            };
            for cid in comps.keys() {
                if other_comps.contains_key(cid) {
                    out.push((SmolStr::from(sys.text()), cid.clone()));
                }
            }
        }
        out
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }
}

impl JoinSemilattice for SciSet {
    /// Component-wise union: merge control systems, compartments, and
    /// sub-compartments. Delegates to the internal
    /// `HierarchicalTreeSet::join_with` method in `super::helpers`.
    fn join(&self, other: &Self) -> Self {
        Self {
            systems: self.systems.join_with(&other.systems),
        }
    }
}

impl MeetSemilattice for SciSet {
    /// Component-wise **equal-depth** intersection per §3.3a policy (b).
    /// A system survives only if it appears on both sides; within a
    /// surviving system, a compartment survives only if present on both;
    /// within a surviving compartment, sub-compartments are intersected.
    ///
    /// Note: this is not the only reasonable meet on a compartment
    /// tree. See the module-level docs and [`SciSet::overlaps`] /
    /// [`SciSet::common_compartments`] for alternatives.
    ///
    /// Delegates to the internal `HierarchicalTreeSet::meet_with`
    /// method in `super::helpers`.
    fn meet(&self, other: &Self) -> Self {
        Self {
            systems: self.systems.meet_with(&other.systems),
        }
    }
}

// `SciSet` intentionally does **not** implement `BoundedLattice`: SCI
// has no lawful finite top because agency-custom control systems are
// an open set (any new `[A-Z0-9]{2,5}` identifier extends the
// universe). An "empty" top would violate the
// `BoundedLattice::top ⊔ a = top` contract on any non-empty `a`. Use
// [`SciSet::empty`] / [`SciSet::default`] when you need the bottom,
// and [`JoinSemilattice::join`] / [`MeetSemilattice::meet`] for
// composition. Schemes
// that want a bounded variant should wrap `SciSet` with an explicit
// sentinel top.

// ---------------------------------------------------------------------------
// sci_controls_from_markings — flat CVE-enum projection helper
// ---------------------------------------------------------------------------

/// Project a slice of `CanonicalAttrs` to the flat CVE-enum
/// projection `Box<[SciControl]>` consumed by the back-compat
/// `CanonicalAttrs.sci_controls` field.
///
/// Mirrors `PageContext::expected_sci_controls`: unions the
/// per-portion `sci_controls` field (the flat CVE projection populated
/// at parse time) across portions. Sorted via `BTreeSet` natural
/// order; dedup'd by `Ord` on `SciControl`.
///
/// Returns a sorted, dedup'd `Box<[SciControl]>` in BTreeSet natural order.
///
/// **Why not project from `SciSet::to_markings()` output?** The
/// structural roll-up at `SciSet::to_markings` sets `canonical_enum:
/// None` on every output entry (per the `expected_sci_markings`
/// doc-comment — "`canonical_enum` is always `None` on roll-up
/// output"); the flat CVE projection MUST come from the per-portion
/// `sci_controls` field (where parsers populated it at parse time
/// for portion entries that matched a compound CVE).
///
/// §-authority: CAPCO-2016 §H.4 p61 (SCI compartment grammar) — the
/// flat CVE-enum projection is the back-compat view; the structural
/// form `SciSet::to_markings` is the authoritative roll-up.
/// Verified 2026-05-18 against `crates/capco/docs/CAPCO-2016.md`.
pub fn sci_controls_from_markings(portions: &[CanonicalAttrs]) -> Box<[marque_ism::SciControl]> {
    let mut seen: BTreeSet<marque_ism::SciControl> = BTreeSet::new();
    for p in portions {
        for c in p.sci_controls.iter().copied() {
            seen.insert(c);
        }
    }
    seen.into_iter().collect::<Vec<_>>().into_boxed_slice()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::type_complexity)] // Test-fixture DSL; explicit shape is clearer than a newtype.
mod tests {
    use super::*;
    use crate::lattice::test_support::*;
    use marque_ism::{SciControlBare, SciControlSystem};

    #[test]
    fn sci_set_join_unions_compartments() {
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["ABCD"])],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["DEFG"])],
        )]);
        let j = a.join(&b);
        let out = j.to_markings();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].compartments.len(), 1);
        let c = &out[0].compartments[0];
        assert_eq!(c.identifier.as_str(), "G");
        let subs: Vec<&str> = c.sub_compartments.iter().map(|s| s.as_str()).collect();
        assert_eq!(subs, vec!["ABCD", "DEFG"]);
    }

    #[test]
    fn sci_set_meet_equal_depth_intersection() {
        // §3.3a policy (b): `SI ⊓ SI-G ABCD = SI` (bare system survives;
        // compartments don't).
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["ABCD"])],
        )]);
        let m = a.meet(&b);
        let out = m.to_markings();
        assert_eq!(out.len(), 1);
        assert!(out[0].compartments.is_empty());
    }

    #[test]
    fn sci_set_meet_disagreeing_compartments_preserves_bare_system() {
        // §3.3a policy (b): `SI-G ABCD ⊓ SI-H DEFG = SI` (bare system
        // survives because both sides contain SI at depth 0; the
        // compartments disagree so they drop). Regression test against a
        // prior implementation that silently dropped the system entirely
        // when both sides had disagreeing non-empty compartment maps.
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["ABCD"])],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("H", vec!["DEFG"])],
        )]);
        let m = a.meet(&b);
        let out = m.to_markings();
        assert_eq!(out.len(), 1);
        assert!(matches!(
            &out[0].system,
            SciControlSystem::Published(SciControlBare::Si)
        ));
        assert!(out[0].compartments.is_empty());
    }

    #[test]
    fn sci_set_meet_drops_disjoint_systems() {
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Hcs),
            vec![],
        )]);
        let m = a.meet(&b);
        assert!(m.is_empty());
    }

    #[test]
    fn sci_set_overlaps_true_on_shared_system() {
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["ABCD"])],
        )]);
        assert!(a.overlaps(&b));
    }

    #[test]
    fn sci_set_common_compartments() {
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["ABCD"])],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["DEFG"])],
        )]);
        let common = a.common_compartments(&b);
        assert_eq!(common, vec![(SmolStr::from("SI"), SmolStr::from("G"))]);
    }

    #[test]
    fn sci_set_join_associative() {
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["A"])],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["B"])],
        )]);
        let c = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Tk),
            vec![],
        )]);
        assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn sci_set_join_commutative() {
        let a = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Si),
            vec![("G", vec!["A"])],
        )]);
        let b = SciSet::from_markings(&[mk_sci(
            SciControlSystem::Published(SciControlBare::Tk),
            vec![],
        )]);
        assert_eq!(a.join(&b), b.join(&a));
    }

    // -----------------------------------------------------------------
    // Additional coverage: constructors, accessors, Custom paths,
    // round-trip
    // -----------------------------------------------------------------

    #[test]
    fn sci_set_custom_system_round_trip() {
        // Custom systems (agency-allocated `[A-Z0-9]{2,5}`) should
        // round-trip via from_markings / to_markings.
        let custom = SciMarking::new(
            SciControlSystem::Custom("99".into()),
            vec![].into_boxed_slice(),
            None,
        );
        let set = SciSet::from_markings(&[custom]);
        let out = set.to_markings();
        assert_eq!(out.len(), 1);
        match &out[0].system {
            SciControlSystem::Custom(s) => assert_eq!(s.as_str(), "99"),
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn sci_set_custom_system_text_used_for_ordering() {
        // Two customs; ordering in output uses SAR-style sort keys
        // (numeric first, then alpha).
        let custom_alpha =
            SciMarking::new(SciControlSystem::Custom("AAA".into()), Box::new([]), None);
        let custom_num = SciMarking::new(SciControlSystem::Custom("99".into()), Box::new([]), None);
        let set = SciSet::from_markings(&[custom_alpha, custom_num]);
        let out = set.to_markings();
        assert_eq!(out.len(), 2);
        // Numeric `99` sorts before alphabetic `AAA`.
        match &out[0].system {
            SciControlSystem::Custom(s) => assert_eq!(s.as_str(), "99"),
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn sci_set_round_trip_with_subcompartments_preserves_order() {
        let m = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new(
                "G",
                Box::new([SmolStr::from("DEFG"), SmolStr::from("ABCD")]),
            )]
            .into_boxed_slice(),
            None,
        );
        let set = SciSet::from_markings(&[m]);
        let out = set.to_markings();
        // Sub-compartments come out alpha-sorted per §A.6 p15.
        let subs: Vec<&str> = out[0].compartments[0]
            .sub_compartments
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(subs, vec!["ABCD", "DEFG"]);
    }

    #[test]
    fn sci_set_is_empty_and_accessors() {
        let empty = SciSet::empty();
        assert!(empty.is_empty());
        let populated = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([]),
            None,
        )]);
        assert!(!populated.is_empty());
    }

    #[test]
    fn sci_set_overlaps_false_on_disjoint_systems() {
        let a = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([]),
            None,
        )]);
        let b = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Tk),
            Box::new([]),
            None,
        )]);
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn sci_set_common_compartments_empty_on_disjoint() {
        let a = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([]),
            None,
        )]);
        let b = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Tk),
            Box::new([]),
            None,
        )]);
        assert!(a.common_compartments(&b).is_empty());
    }

    #[test]
    fn sci_set_common_compartments_shared_system_disjoint_compartments() {
        // Same system (SI), disagreeing compartment IDs — common_compartments
        // should be empty.
        let a = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new("G", Box::new([]))].into_boxed_slice(),
            None,
        )]);
        let b = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new("H", Box::new([]))].into_boxed_slice(),
            None,
        )]);
        assert!(a.common_compartments(&b).is_empty());
    }

    #[test]
    fn sci_set_from_markings_merges_duplicate_entries() {
        // Two markings for the same system with different
        // sub-compartments should merge.
        let m1 = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new("G", Box::new([SmolStr::from("A")]))].into_boxed_slice(),
            None,
        );
        let m2 = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new("G", Box::new([SmolStr::from("B")]))].into_boxed_slice(),
            None,
        );
        let set = SciSet::from_markings(&[m1, m2]);
        let out = set.to_markings();
        assert_eq!(out.len(), 1);
        let subs: Vec<&str> = out[0].compartments[0]
            .sub_compartments
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(subs, vec!["A", "B"]);
    }

    #[test]
    fn sci_set_to_markings_empty_returns_empty() {
        let set = SciSet::empty();
        let out = set.to_markings();
        assert_eq!(out.len(), 0);
    }

    // sci_controls_from_markings — happy-path and lattice-relevant edge cases.

    #[test]
    fn sci_controls_from_markings_empty_input_returns_empty_slice() {
        let out = sci_controls_from_markings(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn sci_controls_from_markings_deduplicates_repeated_controls() {
        // Two portions with the same SI control should
        // project to a single SciControl entry — set-union semantics.
        let mut p1 = marque_ism::CanonicalAttrs::default();
        p1.sci_controls = Box::new([marque_ism::SciControl::Si]);
        let mut p2 = marque_ism::CanonicalAttrs::default();
        p2.sci_controls = Box::new([marque_ism::SciControl::Si]);
        let controls = sci_controls_from_markings(&[p1, p2]);
        let si_count = controls
            .iter()
            .filter(|c| **c == marque_ism::SciControl::Si)
            .count();
        assert_eq!(si_count, 1, "expected dedup; got {si_count} SI entries");
    }

    #[test]
    fn sci_controls_from_markings_unions_distinct_controls() {
        // Union of distinct SciControls across portions.
        let mut p1 = marque_ism::CanonicalAttrs::default();
        p1.sci_controls = Box::new([marque_ism::SciControl::Si]);
        let mut p2 = marque_ism::CanonicalAttrs::default();
        p2.sci_controls = Box::new([marque_ism::SciControl::Tk]);
        let controls = sci_controls_from_markings(&[p1, p2]);
        assert!(controls.contains(&marque_ism::SciControl::Si));
        assert!(controls.contains(&marque_ism::SciControl::Tk));
    }
}
