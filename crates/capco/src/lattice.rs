// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO structural lattice types.
//!
//! The types in this module are the lattice-form counterparts to the
//! structural types [`marque_ism::SciMarking`], [`marque_ism::SarMarking`],
//! and [`marque_ism::FgiMarker`] — newtype wrappers that implement
//! [`Lattice`] so CAPCO's structural categories compose through the
//! generic engine machinery instead of through hand-written
//! `PageContext::expected_*` functions.
//!
//! # Policy (§3.3a of the Phase B design doc)
//!
//! Tree intersection is not unique. For SCI, given `SI-G ABCD` on the
//! left and plain `SI` on the right, the meet could reasonably be (a)
//! `SI-G ABCD` (right's "SI" is the broadest ancestor and survives),
//! (b) just `SI` (drop everything the right side doesn't explicitly
//! name), or (c) empty (only identical leaves survive).
//!
//! This module picks **policy (b)**: meet keeps only elements present
//! at the same depth in both operands. That gives
//! `SI ⊓ SI-G ABCD = SI`, the interpretation closest to the plain
//! lattice definition (`x ⊓ y ≤ x` and `x ⊓ y ≤ y`).
//!
//! Callers that need a different interpretation — primarily the Phase
//! C constraint-evaluator asking "do these two portions share any SCI
//! compartment?" — use [`SciSet::overlaps`] and
//! [`SciSet::common_compartments`] rather than `Lattice::meet`.
//!
//! # SCI storage canonicalization
//!
//! Post-Phase-B, [`SciSet`] is the **canonical** page-context storage
//! for SCI. [`marque_ism::IsmAttributes::sci_controls`] (the flat CVE
//! enum projection) stays populated for rules that currently read it
//! but is a compatibility view scheduled for removal once no rule
//! references it (Phase C or D). New rules read `sci_markings` /
//! `SciSet`.

use marque_ism::{
    CountryCode, FgiMarker, SarCompartment, SarIndicator, SarMarking, SarProgram, SciCompartment,
    SciControlSystem, SciMarking,
};
use marque_scheme::{BoundedLattice, Lattice};
use std::collections::{BTreeMap, BTreeSet};

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
/// existing [`marque_ism::PageContext::expected_sci_markings`] output
/// is the Phase B verification gate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SciSet {
    /// system → compartment identifier → set of sub-compartment
    /// identifiers.
    systems: BTreeMap<SystemKey, BTreeMap<String, BTreeSet<String>>>,
}

/// Stable ordering key for `SciControlSystem`. Published variants and
/// Custom variants are interleaved on their textual forms — the final
/// emission order is re-sorted per CAPCO §A.6 p15 (numeric first) when
/// converting back to `[SciMarking]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SystemKey {
    Published(marque_ism::SciControlBare),
    Custom(String),
}

impl SystemKey {
    fn from_system(sys: &SciControlSystem) -> Self {
        match sys {
            SciControlSystem::Published(b) => SystemKey::Published(*b),
            SciControlSystem::Custom(s) => SystemKey::Custom(s.to_string()),
        }
    }

    fn text(&self) -> &str {
        match self {
            SystemKey::Published(b) => b.as_str(),
            SystemKey::Custom(s) => s.as_str(),
        }
    }

    fn into_system(self) -> SciControlSystem {
        match self {
            SystemKey::Published(b) => SciControlSystem::Published(b),
            SystemKey::Custom(s) => SciControlSystem::Custom(s.into_boxed_str()),
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
        let mut out = Self::empty();
        for m in markings {
            let key = SystemKey::from_system(&m.system);
            let comp_map = out.systems.entry(key).or_default();
            if m.compartments.is_empty() {
                // Bare system — ensure the entry exists so a subsequent
                // rollup preserves the bare form.
                continue;
            }
            for comp in m.compartments.iter() {
                if !comp_map.contains_key(comp.identifier.as_ref()) {
                    comp_map.insert(comp.identifier.to_string(), Default::default());
                }
                let sub_set = comp_map.get_mut(comp.identifier.as_ref()).unwrap();
                sub_set.extend(comp.sub_compartments.iter().map(ToString::to_string));
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
        let mut systems: Vec<(&SystemKey, &BTreeMap<String, BTreeSet<String>>)> =
            self.systems.iter().collect();
        systems.sort_by(|a, b| {
            marque_ism::sar_sort_key(a.0.text()).cmp(&marque_ism::sar_sort_key(b.0.text()))
        });

        let mut out: Vec<SciMarking> = Vec::with_capacity(systems.len());
        for (sys_key, comp_map) in systems {
            let mut comps: Vec<(&String, &BTreeSet<String>)> = comp_map.iter().collect();
            comps.sort_by(|a, b| marque_ism::sar_sort_key(a.0).cmp(&marque_ism::sar_sort_key(b.0)));

            let compartments: Vec<SciCompartment> = comps
                .into_iter()
                .map(|(id, sub_set)| {
                    let mut subs: Vec<&String> = sub_set.iter().collect();
                    subs.sort_by(|a, b| {
                        marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b))
                    });
                    let sub_boxes: Box<[Box<str>]> = subs
                        .into_iter()
                        .map(|s| s.clone().into_boxed_str())
                        .collect::<Vec<_>>()
                        .into_boxed_slice();
                    SciCompartment::new(id.clone().into_boxed_str(), sub_boxes)
                })
                .collect();

            out.push(SciMarking::new(
                sys_key.clone().into_system(),
                compartments.into_boxed_slice(),
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
    pub fn common_compartments(&self, other: &Self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (sys, comps) in &self.systems {
            let Some(other_comps) = other.systems.get(sys) else {
                continue;
            };
            for cid in comps.keys() {
                if other_comps.contains_key(cid) {
                    out.push((sys.text().to_owned(), cid.clone()));
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

impl Lattice for SciSet {
    /// Component-wise union. For each control system present in either
    /// operand, include it; within a system, union its compartments;
    /// within a compartment, union its sub-compartments.
    fn join(&self, other: &Self) -> Self {
        let mut out = self.clone();
        for (sys, comp_map) in &other.systems {
            let out_comps = match out.systems.get_mut(sys) {
                Some(c) => c,
                None => out.systems.entry(sys.clone()).or_default(),
            };

            for (cid, subs) in comp_map {
                let out_subs = match out_comps.get_mut(cid) {
                    Some(s) => s,
                    None => out_comps.entry(cid.clone()).or_default(),
                };
                out_subs.extend(subs.iter().cloned());
            }
        }
        out
    }

    /// Component-wise **equal-depth** intersection per §3.3a policy
    /// (b). A system survives only if it appears on both sides; within
    /// a surviving system, a compartment survives only if present on
    /// both; within a surviving compartment, sub-compartments are
    /// intersected.
    ///
    /// Note: this is not the only reasonable meet on a compartment
    /// tree. See the module-level docs and [`SciSet::overlaps`] /
    /// [`SciSet::common_compartments`] for alternatives.
    fn meet(&self, other: &Self) -> Self {
        let mut out = Self::empty();
        for (sys, comp_map) in &self.systems {
            let Some(other_comps) = other.systems.get(sys) else {
                // System absent from other operand: drop (§3.3a policy (b) —
                // system must appear at the shared depth 0).
                continue;
            };
            // Intersect compartments. Missing compartments on either side
            // are dropped; the system itself survives because it's at the
            // shared depth (both operands contain it). That gives:
            //   - `SI ⊓ SI-G = SI`     (non-shared compartments drop)
            //   - `SI-G ⊓ SI-H = SI`   (both have compartments but they
            //                           disagree — compartments drop,
            //                           bare system survives)
            //   - `SI-G A ⊓ SI-G B = SI-G` (compartment survives, subs drop)
            let mut out_comps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for (cid, subs) in comp_map {
                let Some(other_subs) = other_comps.get(cid) else {
                    continue;
                };
                let common: BTreeSet<String> = subs.intersection(other_subs).cloned().collect();
                out_comps.insert(cid.clone(), common);
            }
            out.systems.insert(sys.clone(), out_comps);
        }
        out
    }
}

// `SciSet` intentionally does **not** implement `BoundedLattice`: SCI
// has no lawful finite top because agency-custom control systems are
// an open set (any new `[A-Z0-9]{2,5}` identifier extends the
// universe). An "empty" top would violate the
// `BoundedLattice::top ⊔ a = top` contract on any non-empty `a`. Use
// [`SciSet::empty`] / [`SciSet::default`] when you need the bottom,
// and [`Lattice::join`] / [`Lattice::meet`] for composition. Schemes
// that want a bounded variant should wrap `SciSet` with an explicit
// sentinel top.

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
    programs: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
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
            if !out.programs.contains_key(prog.identifier.as_ref()) {
                out.programs
                    .insert(prog.identifier.to_string(), Default::default());
            }
            let comps = out.programs.get_mut(prog.identifier.as_ref()).unwrap();
            for comp in prog.compartments.iter() {
                if !comps.contains_key(comp.identifier.as_ref()) {
                    comps.insert(comp.identifier.to_string(), Default::default());
                }
                let subs = comps.get_mut(comp.identifier.as_ref()).unwrap();
                subs.extend(comp.sub_compartments.iter().map(ToString::to_string));
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

        let mut prog_keys: Vec<&String> = self.programs.keys().collect();
        prog_keys.sort_by(|a, b| marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b)));

        let built_programs: Vec<SarProgram> = prog_keys
            .into_iter()
            .map(|pid| {
                let comp_map = self.programs.get(pid).expect("key enumerated above");
                let mut comp_keys: Vec<&String> = comp_map.keys().collect();
                comp_keys
                    .sort_by(|a, b| marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b)));

                let built_compartments: Vec<SarCompartment> = comp_keys
                    .into_iter()
                    .map(|cid| {
                        let subs = comp_map.get(cid).expect("key enumerated above");
                        let mut sub_vec: Vec<&String> = subs.iter().collect();
                        sub_vec.sort_by(|a, b| {
                            marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b))
                        });
                        let boxed: Box<[Box<str>]> = sub_vec
                            .into_iter()
                            .map(|s| s.clone().into_boxed_str())
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        SarCompartment::new(cid.clone().into_boxed_str(), boxed)
                    })
                    .collect();

                SarProgram::new(
                    pid.clone().into_boxed_str(),
                    built_compartments.into_boxed_slice(),
                )
            })
            .collect();

        Some(SarMarking::new(
            SarIndicator::Abbrev,
            built_programs.into_boxed_slice(),
        ))
    }

    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }
}

impl Lattice for SarSet {
    fn join(&self, other: &Self) -> Self {
        let mut out = self.clone();
        for (pid, comp_map) in &other.programs {
            let out_comps = match out.programs.get_mut(pid) {
                Some(c) => c,
                None => out.programs.entry(pid.clone()).or_default(),
            };

            for (cid, subs) in comp_map {
                let out_subs = match out_comps.get_mut(cid) {
                    Some(s) => s,
                    None => out_comps.entry(cid.clone()).or_default(),
                };
                out_subs.extend(subs.iter().cloned());
            }
        }
        out
    }

    fn meet(&self, other: &Self) -> Self {
        let mut out = Self::empty();
        for (pid, comp_map) in &self.programs {
            let Some(other_comps) = other.programs.get(pid) else {
                continue;
            };
            let mut out_comps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for (cid, subs) in comp_map {
                let Some(other_subs) = other_comps.get(cid) else {
                    continue;
                };
                let common: BTreeSet<String> = subs.intersection(other_subs).cloned().collect();
                out_comps.insert(cid.clone(), common);
            }
            out.programs.insert(pid.clone(), out_comps);
        }
        out
    }
}

// `SarSet` intentionally does **not** implement `BoundedLattice`: SAR
// program identifiers are agency-assigned codewords, an open set. An
// "empty" top would violate the `BoundedLattice::top ⊔ a = top`
// contract on any non-empty `a`. Use [`SarSet::empty`] /
// [`SarSet::default`] when you need the bottom, and [`Lattice::join`]
// / [`Lattice::meet`] for composition.

// ---------------------------------------------------------------------------
// FgiSet — lattice over the FGI marker
// ---------------------------------------------------------------------------

/// FGI marker in lattice form.
///
/// CAPCO's FGI marker has two independent axes: a set of source
/// countries and a source-concealed flag. Source-concealed supersedes
/// source-acknowledged on join — if any portion carries FGI with no
/// countries (concealed), the banner must also be concealed. Meet
/// (§3.3a policy b) intersects countries and clears concealment unless
/// both sides were concealed.
///
/// `FgiSet::None` is the bottom (no FGI anywhere).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FgiSet {
    /// No FGI present.
    #[default]
    None,
    /// FGI present. `concealed = true` means "source-concealed" —
    /// countries must be empty when this is set; join preserves
    /// concealment.
    Present {
        concealed: bool,
        countries: BTreeSet<CountryCode>,
    },
}

impl FgiSet {
    pub fn empty() -> Self {
        Self::None
    }

    pub fn from_marker(marker: Option<&FgiMarker>) -> Self {
        let Some(m) = marker else {
            return Self::None;
        };
        if m.countries.is_empty() {
            Self::Present {
                concealed: true,
                countries: BTreeSet::new(),
            }
        } else {
            Self::Present {
                concealed: false,
                countries: m.countries.iter().copied().collect(),
            }
        }
    }

    pub fn to_marker(&self) -> Option<FgiMarker> {
        match self {
            Self::None => None,
            Self::Present {
                concealed,
                countries,
            } => {
                if *concealed {
                    Some(FgiMarker {
                        countries: Box::new([]),
                    })
                } else {
                    Some(FgiMarker {
                        countries: countries
                            .iter()
                            .copied()
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    })
                }
            }
        }
    }
}

impl Lattice for FgiSet {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::None, o) | (o, Self::None) => o.clone(),
            (
                Self::Present {
                    concealed: a_c,
                    countries: a_cs,
                },
                Self::Present {
                    concealed: b_c,
                    countries: b_cs,
                },
            ) => {
                let concealed = *a_c || *b_c;
                if concealed {
                    Self::Present {
                        concealed: true,
                        countries: BTreeSet::new(),
                    }
                } else {
                    let mut countries = a_cs.clone();
                    countries.extend(b_cs.iter().copied());
                    Self::Present {
                        concealed: false,
                        countries,
                    }
                }
            }
        }
    }

    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::None, _) | (_, Self::None) => Self::None,
            (
                Self::Present {
                    concealed: a_c,
                    countries: a_cs,
                },
                Self::Present {
                    concealed: b_c,
                    countries: b_cs,
                },
            ) => {
                let concealed = *a_c && *b_c;
                if concealed {
                    Self::Present {
                        concealed: true,
                        countries: BTreeSet::new(),
                    }
                } else {
                    let countries: BTreeSet<CountryCode> =
                        a_cs.intersection(b_cs).copied().collect();
                    if countries.is_empty() && !concealed {
                        // Both present but no common countries — the
                        // meet collapses to the empty FGI marker, but
                        // that's not representable as `Present` with no
                        // countries without claiming concealment. Fall
                        // back to None as the "no shared FGI" answer.
                        Self::None
                    } else {
                        Self::Present {
                            concealed: false,
                            countries,
                        }
                    }
                }
            }
        }
    }
}

impl BoundedLattice for FgiSet {
    fn bottom() -> Self {
        Self::None
    }
    /// Top: source-concealed with no countries — dominates every other
    /// non-concealed state under the supersession rule.
    fn top() -> Self {
        Self::Present {
            concealed: true,
            countries: BTreeSet::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::type_complexity)] // Test-fixture DSL; explicit shape is clearer than a newtype.
mod tests {
    use super::*;
    use marque_ism::{SciControlBare, SciControlSystem};

    // SciSet

    fn mk_sci(system: SciControlSystem, comps: Vec<(&str, Vec<&str>)>) -> SciMarking {
        let compartments: Vec<SciCompartment> = comps
            .into_iter()
            .map(|(cid, subs)| {
                let sub_boxes: Box<[Box<str>]> = subs
                    .into_iter()
                    .map(|s| s.to_string().into_boxed_str())
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                SciCompartment::new(cid.to_string().into_boxed_str(), sub_boxes)
            })
            .collect();
        SciMarking::new(system, compartments.into_boxed_slice(), None)
    }

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
        assert_eq!(c.identifier.as_ref(), "G");
        let subs: Vec<&str> = c.sub_compartments.iter().map(|s| s.as_ref()).collect();
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
        assert_eq!(common, vec![("SI".to_owned(), "G".to_owned())]);
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

    // SarSet

    fn mk_sar_portion(programs: Vec<(&str, Vec<(&str, Vec<&str>)>)>) -> SarMarking {
        let built: Vec<SarProgram> = programs
            .into_iter()
            .map(|(pid, comps)| {
                let comp_boxes: Vec<SarCompartment> = comps
                    .into_iter()
                    .map(|(cid, subs)| {
                        let sub_boxes: Box<[Box<str>]> = subs
                            .into_iter()
                            .map(|s| s.to_string().into_boxed_str())
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        SarCompartment::new(cid.to_string().into_boxed_str(), sub_boxes)
                    })
                    .collect();
                SarProgram::new(
                    pid.to_string().into_boxed_str(),
                    comp_boxes.into_boxed_slice(),
                )
            })
            .collect();
        SarMarking::new(SarIndicator::Abbrev, built.into_boxed_slice())
    }

    #[test]
    fn sar_set_join_unions_programs() {
        let a = SarSet::from_marking(Some(&mk_sar_portion(vec![("BP", vec![])])));
        let b = SarSet::from_marking(Some(&mk_sar_portion(vec![("CD", vec![])])));
        let j = a.join(&b);
        let out = j.to_marking().expect("nonempty");
        let ids: Vec<&str> = out.programs.iter().map(|p| p.identifier.as_ref()).collect();
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

    // FgiSet

    #[test]
    fn fgi_set_concealed_supersedes_acknowledged() {
        let conc = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let ack = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let j = conc.join(&ack);
        match j {
            FgiSet::Present {
                concealed,
                countries,
            } => {
                assert!(concealed);
                assert!(countries.is_empty());
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_set_join_unions_acknowledged_countries() {
        let a = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let b = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"DEU").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let j = a.join(&b);
        match j {
            FgiSet::Present { countries, .. } => {
                assert_eq!(countries.len(), 2);
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_set_none_is_bottom() {
        assert_eq!(FgiSet::bottom(), FgiSet::None);
    }

    // -----------------------------------------------------------------
    // Additional coverage: constructors, accessors, Custom paths,
    // round-trip, meet edge cases
    // -----------------------------------------------------------------

    // SciSet — Custom system, accessors, round-trip

    #[test]
    fn sci_set_custom_system_round_trip() {
        // Custom systems (agency-allocated `[A-Z0-9]{2,5}`) should
        // round-trip via from_markings / to_markings.
        let custom = SciMarking::new(
            SciControlSystem::Custom("99".to_string().into_boxed_str()),
            vec![].into_boxed_slice(),
            None,
        );
        let set = SciSet::from_markings(&[custom]);
        let out = set.to_markings();
        assert_eq!(out.len(), 1);
        match &out[0].system {
            SciControlSystem::Custom(s) => assert_eq!(s.as_ref(), "99"),
            SciControlSystem::Published(_) => panic!("expected Custom"),
        }
    }

    #[test]
    fn sci_set_custom_system_text_used_for_ordering() {
        // Two customs; ordering in output uses SAR-style sort keys
        // (numeric first, then alpha).
        let custom_alpha = SciMarking::new(
            SciControlSystem::Custom("AAA".to_string().into_boxed_str()),
            Box::new([]),
            None,
        );
        let custom_num = SciMarking::new(
            SciControlSystem::Custom("99".to_string().into_boxed_str()),
            Box::new([]),
            None,
        );
        let set = SciSet::from_markings(&[custom_alpha, custom_num]);
        let out = set.to_markings();
        assert_eq!(out.len(), 2);
        // Numeric `99` sorts before alphabetic `AAA`.
        match &out[0].system {
            SciControlSystem::Custom(s) => assert_eq!(s.as_ref(), "99"),
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn sci_set_round_trip_with_subcompartments_preserves_order() {
        let m = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new(
                "G".to_string().into_boxed_str(),
                vec![
                    "DEFG".to_string().into_boxed_str(),
                    "ABCD".to_string().into_boxed_str(),
                ]
                .into_boxed_slice(),
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
            .map(|s| s.as_ref())
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
            vec![SciCompartment::new(
                "G".to_string().into_boxed_str(),
                Box::new([]),
            )]
            .into_boxed_slice(),
            None,
        )]);
        let b = SciSet::from_markings(&[SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new(
                "H".to_string().into_boxed_str(),
                Box::new([]),
            )]
            .into_boxed_slice(),
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
            vec![SciCompartment::new(
                "G".to_string().into_boxed_str(),
                vec!["A".to_string().into_boxed_str()].into_boxed_slice(),
            )]
            .into_boxed_slice(),
            None,
        );
        let m2 = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            vec![SciCompartment::new(
                "G".to_string().into_boxed_str(),
                vec!["B".to_string().into_boxed_str()].into_boxed_slice(),
            )]
            .into_boxed_slice(),
            None,
        );
        let set = SciSet::from_markings(&[m1, m2]);
        let out = set.to_markings();
        assert_eq!(out.len(), 1);
        let subs: Vec<&str> = out[0].compartments[0]
            .sub_compartments
            .iter()
            .map(|s| s.as_ref())
            .collect();
        assert_eq!(subs, vec!["A", "B"]);
    }

    #[test]
    fn sci_set_to_markings_empty_returns_empty() {
        let set = SciSet::empty();
        let out = set.to_markings();
        assert_eq!(out.len(), 0);
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
            vec![SarProgram::new(
                "BP".to_string().into_boxed_str(),
                Box::new([]),
            )]
            .into_boxed_slice(),
        )));
        assert!(!set.is_empty());
    }

    #[test]
    fn sar_set_round_trip_with_nested_hierarchy() {
        let sar = SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new(
                "BP".to_string().into_boxed_str(),
                vec![SarCompartment::new(
                    "J12".to_string().into_boxed_str(),
                    vec![
                        "K20".to_string().into_boxed_str(),
                        "K15".to_string().into_boxed_str(),
                    ]
                    .into_boxed_slice(),
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
            .map(|s| s.as_ref())
            .collect();
        assert_eq!(subs, vec!["K15", "K20"]);
    }

    #[test]
    fn sar_set_meet_drops_programs_not_on_both_sides() {
        let a = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new(
                "BP".to_string().into_boxed_str(),
                Box::new([]),
            )]
            .into_boxed_slice(),
        )));
        let b = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new(
                "CD".to_string().into_boxed_str(),
                Box::new([]),
            )]
            .into_boxed_slice(),
        )));
        assert!(a.meet(&b).is_empty());
    }

    #[test]
    fn sar_set_meet_common_program_keeps_entry() {
        let a = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new(
                "BP".to_string().into_boxed_str(),
                Box::new([]),
            )]
            .into_boxed_slice(),
        )));
        let b = SarSet::from_marking(Some(&SarMarking::new(
            SarIndicator::Abbrev,
            vec![SarProgram::new(
                "BP".to_string().into_boxed_str(),
                Box::new([]),
            )]
            .into_boxed_slice(),
        )));
        let m = a.meet(&b);
        assert!(!m.is_empty());
    }

    // FgiSet — from_marker/to_marker round-trip + concealed branches

    #[test]
    fn fgi_set_from_marker_none_returns_none() {
        assert_eq!(FgiSet::from_marker(None), FgiSet::None);
    }

    #[test]
    fn fgi_set_from_marker_empty_countries_is_concealed() {
        let m = FgiMarker {
            countries: Box::new([]),
        };
        let set = FgiSet::from_marker(Some(&m));
        assert!(matches!(
            set,
            FgiSet::Present {
                concealed: true,
                ..
            }
        ));
    }

    #[test]
    fn fgi_set_from_marker_populated_countries_is_open() {
        let m = FgiMarker {
            countries: vec![CountryCode::try_new(b"GBR").unwrap()].into_boxed_slice(),
        };
        let set = FgiSet::from_marker(Some(&m));
        match set {
            FgiSet::Present {
                concealed,
                countries,
            } => {
                assert!(!concealed);
                assert_eq!(countries.len(), 1);
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_set_to_marker_none_for_none() {
        assert!(FgiSet::None.to_marker().is_none());
    }

    #[test]
    fn fgi_set_to_marker_concealed_emits_empty_countries() {
        let set = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let marker = set.to_marker().expect("Some");
        assert!(marker.countries.is_empty());
    }

    #[test]
    fn fgi_set_to_marker_open_round_trips_countries() {
        let mut countries = BTreeSet::new();
        countries.insert(CountryCode::try_new(b"GBR").unwrap());
        countries.insert(CountryCode::try_new(b"DEU").unwrap());
        let set = FgiSet::Present {
            concealed: false,
            countries,
        };
        let marker = set.to_marker().expect("Some");
        assert_eq!(marker.countries.len(), 2);
    }

    #[test]
    fn fgi_set_empty_is_none() {
        assert_eq!(FgiSet::empty(), FgiSet::None);
    }

    #[test]
    fn fgi_set_default_is_none() {
        let d: FgiSet = FgiSet::default();
        assert_eq!(d, FgiSet::None);
    }

    #[test]
    fn fgi_set_top_is_concealed_empty() {
        let t = FgiSet::top();
        assert!(matches!(
            t,
            FgiSet::Present {
                concealed: true,
                ..
            }
        ));
    }

    #[test]
    fn fgi_set_join_none_right_preserves_left() {
        let left = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        assert_eq!(left.join(&FgiSet::None), left);
    }

    #[test]
    fn fgi_set_join_none_left_preserves_right() {
        let right = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        assert_eq!(FgiSet::None.join(&right), right);
    }

    #[test]
    fn fgi_set_meet_both_concealed_preserved() {
        let a = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let b = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let m = a.meet(&b);
        assert!(matches!(
            m,
            FgiSet::Present {
                concealed: true,
                ..
            }
        ));
    }

    #[test]
    fn fgi_set_meet_disjoint_countries_collapses_to_none() {
        let a = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let b = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"DEU").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        assert_eq!(a.meet(&b), FgiSet::None);
    }

    #[test]
    fn fgi_set_meet_common_country_preserved() {
        let a = FgiSet::Present {
            concealed: false,
            countries: [
                CountryCode::try_new(b"GBR").unwrap(),
                CountryCode::try_new(b"DEU").unwrap(),
            ]
            .iter()
            .copied()
            .collect(),
        };
        let b = FgiSet::Present {
            concealed: false,
            countries: [
                CountryCode::try_new(b"GBR").unwrap(),
                CountryCode::try_new(b"FRA").unwrap(),
            ]
            .iter()
            .copied()
            .collect(),
        };
        let m = a.meet(&b);
        match m {
            FgiSet::Present {
                concealed,
                countries,
            } => {
                assert!(!concealed);
                assert_eq!(countries.len(), 1);
                assert!(countries.contains(&CountryCode::try_new(b"GBR").unwrap()));
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_set_meet_none_collapses_to_none() {
        let a = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        assert_eq!(FgiSet::None.meet(&a), FgiSet::None);
        assert_eq!(a.meet(&FgiSet::None), FgiSet::None);
        assert_eq!(FgiSet::None.meet(&FgiSet::None), FgiSet::None);
    }
}
