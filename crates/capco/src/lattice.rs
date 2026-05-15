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
//! for SCI. [`marque_ism::CanonicalAttrs::sci_controls`] (the flat CVE
//! enum projection) stays populated for rules that currently read it
//! but is a compatibility view scheduled for removal once no rule
//! references it (Phase C or D). New rules read `sci_markings` /
//! `SciSet`.

use marque_ism::{
    AeaMarking, AtomalBlock, CanonicalAttrs, Classification, CountryCode, DissemControl, FgiMarker,
    FrdBlock, IsmDate, JointClassification, MarkingClassification, NatoClassification, NonIcDissem,
    RdBlock, SarCompartment, SarIndicator, SarMarking, SarProgram, SciCompartment,
    SciControlSystem, SciMarking,
};
use marque_scheme::{BoundedLattice, Lattice};
use smol_str::SmolStr;
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
    systems: BTreeMap<SystemKey, BTreeMap<SmolStr, BTreeSet<SmolStr>>>,
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
        let mut systems: Vec<(&SystemKey, &BTreeMap<SmolStr, BTreeSet<SmolStr>>)> =
            self.systems.iter().collect();
        systems.sort_by(|a, b| {
            marque_ism::sar_sort_key(a.0.text()).cmp(&marque_ism::sar_sort_key(b.0.text()))
        });

        let mut out: Vec<SciMarking> = Vec::with_capacity(systems.len());
        for (sys_key, comp_map) in systems {
            let mut comps: Vec<(&SmolStr, &BTreeSet<SmolStr>)> = comp_map.iter().collect();
            comps.sort_by(|a, b| marque_ism::sar_sort_key(a.0).cmp(&marque_ism::sar_sort_key(b.0)));

            let compartments: Vec<SciCompartment> = comps
                .into_iter()
                .map(|(id, sub_set)| {
                    let mut subs: Vec<&SmolStr> = sub_set.iter().collect();
                    subs.sort_by(|a, b| {
                        marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b))
                    });
                    let sub_boxes: Box<[SmolStr]> = subs
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_boxed_slice();
                    SciCompartment::new(id.clone(), sub_boxes)
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
    pub fn common_compartments(&self, other: &Self) -> Vec<(SmolStr, SmolStr)> {
        let mut out = Vec::new();
        for (sys, comps) in &self.systems {
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
            let mut out_comps: BTreeMap<SmolStr, BTreeSet<SmolStr>> = BTreeMap::new();
            for (cid, subs) in comp_map {
                let Some(other_subs) = other_comps.get(cid) else {
                    continue;
                };
                let common: BTreeSet<SmolStr> = subs.intersection(other_subs).cloned().collect();
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
    programs: BTreeMap<SmolStr, BTreeMap<SmolStr, BTreeSet<SmolStr>>>,
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
            let comps = out.programs.entry(prog.identifier.clone()).or_default();
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

        let mut prog_keys: Vec<&SmolStr> = self.programs.keys().collect();
        prog_keys.sort_by(|a, b| marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b)));

        let built_programs: Vec<SarProgram> = prog_keys
            .into_iter()
            .map(|pid| {
                let comp_map = self.programs.get(pid).expect("key enumerated above");
                let mut comp_keys: Vec<&SmolStr> = comp_map.keys().collect();
                comp_keys
                    .sort_by(|a, b| marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b)));

                let built_compartments: Vec<SarCompartment> = comp_keys
                    .into_iter()
                    .map(|cid| {
                        let subs = comp_map.get(cid).expect("key enumerated above");
                        let mut sub_vec: Vec<&SmolStr> = subs.iter().collect();
                        sub_vec.sort_by(|a, b| {
                            marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b))
                        });
                        let boxed: Box<[SmolStr]> = sub_vec
                            .into_iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        SarCompartment::new(cid.clone(), boxed)
                    })
                    .collect();

                SarProgram::new(pid.clone(), built_compartments.into_boxed_slice())
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
            let out_comps = out.programs.entry(pid.clone()).or_default();
            for (cid, subs) in comp_map {
                let out_subs = out_comps.entry(cid.clone()).or_default();
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
            let mut out_comps: BTreeMap<SmolStr, BTreeSet<SmolStr>> = BTreeMap::new();
            for (cid, subs) in comp_map {
                let Some(other_subs) = other_comps.get(cid) else {
                    continue;
                };
                let common: BTreeSet<SmolStr> = subs.intersection(other_subs).cloned().collect();
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
/// CAPCO's FGI marker has two independent axes: a set of source countries
/// and a source-concealed flag. Source-concealed supersedes source-
/// acknowledged on join — if any portion carries FGI with no countries
/// (concealed), the banner must also be concealed. Meet (§3.3a policy b)
/// intersects countries and clears concealment unless both sides were
/// concealed.
///
/// `FgiSet::None` is the bottom (no FGI anywhere).
///
/// # Source authority
///
/// Governed by CAPCO-2016 §H.7 (pp122-130) "FOREIGN GOVERNMENT INFORMATION"
/// and specifically §H.7 p122 for the source-concealed banner grammar.
/// The canonical operational rules are:
///
/// - FGI with a known source is marked as `FGI [TRIGRAPH]` in the portion
///   mark and `FGI [COUNTRY]` in the banner line (§H.7 p122-123).
/// - FGI from an unknown or concealed source uses the bare `FGI` marker
///   (no trigraph) per §H.7 p122 ("If the specific country is unknown,
///   the marking FGI may be used without identifying the country").
///   This maps to `Present { concealed: true, countries: [] }`.
///
/// Per `docs/plans/2026-05-01-lattice-design.md` §4.8 and `marque-applied.md`
/// §4.8.
///
/// ## §4.8.5 worked example
///
/// Two portions: `(C//NF)` and `(//GBR TS)`. The first portion carries US
/// CONFIDENTIAL + NOFORN; the second carries FGI `GBR` at the TS level
/// (FGI classification blocks are space-delimited per `parse_fgi_classification`;
/// the hyphenated form `GBR-TS` does not match the grammar). After
/// page-level join the result is:
///
/// - Classification: `TOP SECRET` (max of C and TS = TS)
/// - FGI: `Present { concealed: false, countries: {GBR} }` (GBR from portion 2)
/// - Dissem: `NOFORN` (from portion 1)
///
/// Banner: `TOP SECRET//FGI GBR//NOFORN`
///
/// The FgiSet join absorbs the UK classification into the page state via the
/// FGI country presence; the classification axis uses OrdMax to reach TS.
///
/// ## Coverage delimitation
///
/// `FgiSet` models FGI-attribution (country of origin) only. JOINT-attribution
/// (content jointly produced by two or more governments) is modeled separately
/// via `MarkingClassification::Joint` on the classification axis. The two are
/// mutually exclusive at the portion level. Cross-system join (e.g., a page
/// that mixes FGI GBR portions with JOINT USA GBR portions) is not modeled
/// by `FgiSet` — that is the JOINT-attribution incompatibility-class reframe
/// deferred to Stage 4 of the engine refactor (per
/// `docs/plans/2026-05-01-lattice-design.md` §4.7, open question "FGI vs
/// JOINT attribution").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FgiSet {
    /// No FGI present.
    #[default]
    None,
    /// FGI present. `concealed = true` means "source-concealed" (bare `FGI`
    /// marker per §H.7 p122) — countries must be empty when this is set;
    /// join preserves concealment because a source-concealed entry on any
    /// portion requires a source-concealed banner.
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
        match marker {
            None => Self::None,
            Some(FgiMarker::SourceConcealed) => Self::Present {
                concealed: true,
                countries: BTreeSet::new(),
            },
            Some(FgiMarker::Acknowledged { countries, .. }) => Self::Present {
                concealed: false,
                countries: countries.iter().copied().collect(),
            },
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
                    Some(FgiMarker::SourceConcealed)
                } else {
                    // `Present { concealed: false, countries }` is
                    // produced only by lattice operations that either
                    // carry over a non-empty input set or intersect to
                    // a non-empty result (the meet collapses to `None`
                    // when the intersection is empty — see `meet`
                    // below). So `acknowledged(...)` should always
                    // yield `Some` here in practice; if a future
                    // refactor produces a `Present` with an empty
                    // open-source set, we surface `None` rather than
                    // fabricating `SourceConcealed`, which would be a
                    // semantic lie about the source.
                    FgiMarker::acknowledged(countries.iter().copied())
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
// AeaSet — lattice over the AEA category (RD/FRD/TFNI + CNWDI + SIGMA +
// UCNI + ATOMAL)
// ---------------------------------------------------------------------------

/// Primary AEA axis: a total-order supersession chain over the three
/// "primary" AEA markings — TFNI ⊏ FRD ⊏ RD per CAPCO-2016 §H.6 p104
/// + §H.6 p111 + §H.6 p120.
///
/// Variants are declared in **ascending supersession order**, which
/// makes the derived `Ord` impl match the supersession order without
/// a hand-written `cmp`. The `Lattice` impl picks `max(a, b)` as the
/// join — `Rd ⊐ Frd ⊐ Tfni` under that order.
///
/// §-authority (three subsections state the same rule from each
/// marking's vantage):
/// - §H.6 p104 (RD Precedence Rules): "If RD, FRD, and TFNI portions
///   are in a document, the RD takes precedence and is conveyed in
///   the banner line."
/// - §H.6 p111 (FRD Precedence Rules): "If RD and FRD portions are in
///   a document, the RD marking takes precedence in the banner line."
/// - §H.6 p120 (TFNI Precedence Rules): "If the TFNI marking is
///   contained in any portion of a document that contains portions
///   of RD and/or FRD, the RD or FRD takes precedence."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AeaPrimary {
    /// Transclassified Foreign Nuclear Information.
    Tfni,
    /// Formerly Restricted Data.
    Frd,
    /// Restricted Data — top of the AEA supersession chain.
    Rd,
}

/// UCNI variant: DoD or DoE.
///
/// `Ord` derivation places `DodUcni` first (Rust derives `Ord` from
/// variant declaration order, not alphabetical; happens to match
/// alphabetical here because we declared `DodUcni` then `DoeUcni`).
/// §G.1 Table 4 cat-6 order has DOD before DOE, which matches.
///
/// §-authority:
/// - §H.6 p116-117 (DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION
///   / portion mark `DCNI`).
/// - §H.6 p118-119 (DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION
///   / portion mark `UCNI`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UcniKind {
    /// DOD UCNI — DoD Special Nuclear Material protection per §H.6 p116.
    DodUcni,
    /// DOE UCNI — DoE post-RD-declassification controls per §H.6 p118.
    DoeUcni,
}

/// Lattice form of the full AEA category state on a page.
///
/// `AeaSet` is the lattice-form counterpart of the
/// [`marque_ism::AeaMarking`] sequence in `CanonicalAttrs.aea_markings`.
/// It composes five algebraically-distinct sub-axes as a `Product`:
///
/// 1. **Primary** ([`AeaPrimary`]): total-order supersession
///    `Tfni ⊏ Frd ⊏ Rd` per §H.6 p104 + p111 + p120.
/// 2. **CNWDI** (`bool`): presence flag, OR-monotone per §H.6 p106.
/// 3. **SIGMA** (`BTreeSet<u8>`): flat set union of SIGMA program
///    numbers per §H.6 p108 (currently {14, 15, 18, 20}; open-vocab
///    per the prose "SIGMA # currently represents one or more of
///    the following numbers").
/// 4. **UCNI** (`BTreeSet<UcniKind>`): flat set union of DOD / DOE
///    UCNI presence per §H.6 p116-117 + p118-119.
/// 5. **ATOMAL** (`Option<AtomalBlock>`): optional-singleton presence
///    per §G.2 Table 5 p40 (ATOMAL registered as a standalone
///    control marking; ARH = AEA) + §H.7 p122 FGI-section worked
///    example (`SECRET//RD/ATOMAL//FGI NATO//NOFORN` places ATOMAL
///    in the AEA `//` axis position — note §H.7 is the FGI section,
///    not an ATOMAL subsection; ATOMAL has no dedicated subsection
///    in §H.1 through §H.9, its registration lives in §G.2 Table 5).
///    The PR 9c.1 T134 routing decision tracked this through the
///    parser layer.
///
/// `AeaSet` round-trips with `&[AeaMarking]` via
/// [`AeaSet::from_markings`] / [`AeaSet::to_markings`], mirroring
/// the existing [`SciSet::from_markings`] / [`SciSet::to_markings`]
/// pattern.
///
/// # `BoundedLattice` deliberately not implemented
///
/// Per the [`SciSet`] / [`SarSet`] precedent in this module, AeaSet's
/// SIGMA axis is **open-vocabulary** per §H.6 p108 ("currently
/// represents one or more of the following numbers" — i.e., future
/// CAPCO revisions may add SIGMAs). No lawful finite top exists for
/// the Product as a whole. Callers needing the bottom use
/// [`AeaSet::default`] or [`AeaSet::empty`].
///
/// # Cross-axis invariants (validated by `CapcoScheme`, not the lattice)
///
/// - **CNWDI requires RD** (§H.6 p106): the lattice admits the
///   syntactically-reachable state `cnwdi=true, primary=None`, which
///   the `Constraint::Requires` row `E067/cnwdi-requires-rd` on
///   `CapcoScheme::build_constraints()` catches at validation time.
/// - **CNWDI requires class ≥ S** (§H.6 p106): covered by
///   `E058/CNWDI-classification-floor` in the class-floor catalog
///   (PR 3b.D T026d). Not duplicated here.
/// - **UCNI strip on classified** (§H.6 p116-117 + p118-119): a
///   post-projection cross-axis rewrite suppresses UCNI from the
///   banner and adds NOFORN when banner classification > U. The
///   algebraic shape mirrors the §3 (b) FOUO eviction matrix;
///   PR 4b-C wires the catalog row.
/// - **SIGMA cross-modifier coalescing** (§H.6 p108-109 + p113):
///   handled by the existing `capco/frd-sigma-consolidates-into-rd-sigma`
///   PageRewrite. PR 4b-B wires the runtime `AeaSet`-driven mutation
///   to replace the current `never_fires` / `noop_action` stub.
///
/// See `docs/plans/2026-05-01-lattice-design.md` §7.5 for the
/// formal join semantics, four worked examples, and acceptance
/// attestation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AeaSet {
    /// Primary axis: `Option` because not every page carries an
    /// RD/FRD/TFNI portion (a page with only UCNI portions joins to
    /// `primary: None`).
    primary: Option<AeaPrimary>,
    /// CNWDI presence (only meaningful when `primary == Some(Rd)`;
    /// see the cross-axis invariant note above).
    cnwdi: bool,
    /// SIGMA program numbers per §H.6 p108. Sorted ascending by the
    /// `BTreeSet`'s natural order so banner rendering ("Multiple
    /// SIGMA numbers must be listed in numerical order") is a
    /// no-extra-work iteration.
    sigmas: BTreeSet<u8>,
    /// UCNI variants. The two-element vocabulary makes this a
    /// bounded flat-set in isolation; included in the open-vocab
    /// `AeaSet` Product, it stays a flat-set without contributing
    /// boundedness.
    ucni: BTreeSet<UcniKind>,
    /// ATOMAL presence. `AtomalBlock` is currently empty per
    /// §G.2 Table 5 p40 (ATOMAL is a registered standalone control
    /// marking with no enumerated sub-markings — Table 5 lives in
    /// §G.2, the ARH subsection, not §G.1); the carrier struct
    /// mirrors `RdBlock` / `FrdBlock` so a future CAPCO grammar
    /// extension remains a planned migration.
    atomal: Option<AtomalBlock>,
}

impl AeaSet {
    /// An empty AEA set — the lattice bottom.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct an `AeaSet` from a slice of `AeaMarking`.
    ///
    /// Each variant of `AeaMarking` decomposes into one or more
    /// sub-axes:
    /// - `Rd(RdBlock { cnwdi, sigma })` → axis 1 = `Rd`,
    ///   axis 2 = `cnwdi`, axis 3 ⊇ `sigma`.
    /// - `Frd(FrdBlock { sigma })` → axis 1 = max(current, `Frd`),
    ///   axis 3 ⊇ `sigma`.
    /// - `Tfni` → axis 1 = max(current, `Tfni`).
    /// - `DodUcni` → axis 4 ⊇ `{DodUcni}`.
    /// - `DoeUcni` → axis 4 ⊇ `{DoeUcni}`.
    /// - `Atomal(_)` → axis 5 = `Some(AtomalBlock)`.
    ///
    /// Duplicate atoms within `markings` collapse via the per-axis
    /// joins (idempotent in every axis).
    pub fn from_markings(markings: &[AeaMarking]) -> Self {
        let mut out = Self::empty();
        for m in markings {
            match m {
                AeaMarking::Rd(rd) => {
                    out.primary = Some(match out.primary {
                        Some(prev) => prev.max(AeaPrimary::Rd),
                        None => AeaPrimary::Rd,
                    });
                    out.cnwdi = out.cnwdi || rd.cnwdi;
                    out.sigmas.extend(rd.sigma.iter().copied());
                }
                AeaMarking::Frd(frd) => {
                    out.primary = Some(match out.primary {
                        Some(prev) => prev.max(AeaPrimary::Frd),
                        None => AeaPrimary::Frd,
                    });
                    out.sigmas.extend(frd.sigma.iter().copied());
                }
                AeaMarking::Tfni => {
                    out.primary = Some(match out.primary {
                        Some(prev) => prev.max(AeaPrimary::Tfni),
                        None => AeaPrimary::Tfni,
                    });
                }
                AeaMarking::DodUcni => {
                    out.ucni.insert(UcniKind::DodUcni);
                }
                AeaMarking::DoeUcni => {
                    out.ucni.insert(UcniKind::DoeUcni);
                }
                AeaMarking::Atomal(block) => {
                    out.atomal = Some(*block);
                }
                // `AeaMarking` is `#[non_exhaustive]`. A future variant
                // (e.g., a hypothetical CAPCO grammar extension that adds
                // a new AEA marking) lands here as a silent no-op so the
                // existing AEA-lattice builds continue to compile while
                // surfacing the gap explicitly in code review. The fix
                // is to add a sub-axis to `AeaSet` (or extend an
                // existing one) and ship it as a separate atomic PR.
                _ => {}
            }
        }
        out
    }

    /// Render this set back to a boxed slice of `AeaMarking`. The
    /// per-portion emission order is:
    /// `primary → DOD UCNI → DOE UCNI → ATOMAL`
    /// where the primary arm emits one of RD/FRD/TFNI (supersession
    /// guarantees at most one survives the lattice join — so on a
    /// post-join `AeaSet` this is at most one atom, never the
    /// three-atom sequence the §G.1 Table 4 cat-6 register would
    /// suggest). The full §G.1 Table 4 cat-6 register order
    /// (`RD → CNWDI → SIGMA → FRD → SIGMA → DOD UCNI → DOE UCNI →
    /// TFNI → ATOMAL`) is the spec for a per-document banner; the
    /// post-join lattice already collapses to a single primary,
    /// making the emission order above isomorphic to the register
    /// order in every realizable case. The §G.1 Table 4 cat-2
    /// position of ATOMAL (`Non-US Protective Markings`) governs
    /// inter-category placement; this method emits only the within-
    /// category atoms.
    ///
    /// CNWDI rides on the RD block; SIGMA numbers ride on the RD
    /// block (per §H.6 p108-109 cross-modifier coalescing — when
    /// `primary == Rd` any SIGMA numbers from RD or FRD portions
    /// emit under RD-SIGMA in the banner). When `primary == Frd`,
    /// SIGMA numbers ride on the FRD block. When `primary == Tfni`,
    /// SIGMA numbers are dropped (TFNI has no SIGMA modifier per
    /// §H.6 p120).
    ///
    /// `AeaMarking::DodUcni` / `DoeUcni` are emitted regardless of
    /// classification; the §H.6 p116-117 / p118-119 "does not appear
    /// in the banner line on classified docs" rule is a
    /// post-projection rewrite (see the cross-axis invariant note
    /// on [`AeaSet`]), not a lattice render-time strip.
    pub fn to_markings(&self) -> Box<[AeaMarking]> {
        let mut out: Vec<AeaMarking> = Vec::new();
        // Sort SIGMA numbers ascending for §H.6 p108 canonical form.
        // `BTreeSet` already iterates in sorted order.
        let sigmas: Box<[u8]> = self
            .sigmas
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .into_boxed_slice();

        // Emission order matches the §G.1 Table 4 cat-6 register:
        // `RD → FRD → DOD UCNI → DOE UCNI → TFNI → ATOMAL`. The
        // primary axis collapses to at most one of {RD, FRD, TFNI}
        // under supersession, and TFNI emits AFTER the UCNI atoms
        // per Table 4's register-order — not in the same arm as RD
        // and FRD. SIGMA rides on whichever of RD or FRD survives
        // per the §H.6 p108-109 cross-modifier coalescing rule;
        // under Tfni-primary the SIGMA set is silently dropped
        // because §H.6 p120 has no SIGMA modifier and the inputs
        // that produced it came from RD or FRD portions that got
        // superseded.

        // Step 1: RD or FRD (if either is the primary).
        match self.primary {
            Some(AeaPrimary::Rd) => {
                out.push(AeaMarking::Rd(RdBlock {
                    cnwdi: self.cnwdi,
                    sigma: sigmas,
                }));
            }
            Some(AeaPrimary::Frd) => {
                // CNWDI is RD-only per §H.6 p106 — the marque-ism
                // type system already enforces this (CNWDI is a
                // `bool` field on `RdBlock`, not on `FrdBlock`), so
                // a `cnwdi=true, primary=Frd` state cannot arise
                // from valid parser output. The render here drops
                // cnwdi silently as a defensive measure against
                // lattice-internal-only constructions.
                out.push(AeaMarking::Frd(FrdBlock { sigma: sigmas }));
            }
            Some(AeaPrimary::Tfni) | None => {
                // TFNI emission is deferred to Step 3 (post-UCNI)
                // to honor the §G.1 Table 4 register order.
                // None — no primary on the page; CNWDI / SIGMA
                // alone are not renderable without a primary
                // anchor.
            }
        }
        // Step 2: UCNI variants per §G.1 Table 4 register order.
        if self.ucni.contains(&UcniKind::DodUcni) {
            out.push(AeaMarking::DodUcni);
        }
        if self.ucni.contains(&UcniKind::DoeUcni) {
            out.push(AeaMarking::DoeUcni);
        }
        // Step 3: TFNI (if primary; emits AFTER UCNI per Table 4).
        if matches!(self.primary, Some(AeaPrimary::Tfni)) {
            out.push(AeaMarking::Tfni);
        }
        // Step 4: ATOMAL.
        if let Some(block) = self.atomal {
            out.push(AeaMarking::Atomal(block));
        }
        out.into_boxed_slice()
    }

    /// Whether the set is empty (all five sub-axes at bottom).
    pub fn is_empty(&self) -> bool {
        self.primary.is_none()
            && !self.cnwdi
            && self.sigmas.is_empty()
            && self.ucni.is_empty()
            && self.atomal.is_none()
    }

    /// Read access to the primary axis. Exposed for cross-axis
    /// rewrite predicates (e.g., a future PR's UCNI-strip-on-
    /// classified that needs to inspect whether an RD/FRD/TFNI
    /// primary exists).
    pub fn primary(&self) -> Option<AeaPrimary> {
        self.primary
    }

    /// Read access to the CNWDI presence flag. Exposed for the
    /// `E067/cnwdi-requires-rd` constraint and analogous cross-axis
    /// validation.
    pub fn cnwdi(&self) -> bool {
        self.cnwdi
    }

    /// Read access to the SIGMA program-number set.
    pub fn sigmas(&self) -> &BTreeSet<u8> {
        &self.sigmas
    }

    /// Read access to the UCNI variant set.
    pub fn ucni(&self) -> &BTreeSet<UcniKind> {
        &self.ucni
    }

    /// Read access to the ATOMAL presence.
    pub fn atomal(&self) -> Option<AtomalBlock> {
        self.atomal
    }
}

impl Lattice for AeaSet {
    /// Componentwise join across the five Product sub-axes per the
    /// formal semantics in
    /// `docs/plans/2026-05-01-lattice-design.md` §7.5.
    fn join(&self, other: &Self) -> Self {
        Self {
            // Axis 1: SupersessionSet — max under Tfni ⊏ Frd ⊏ Rd.
            primary: match (self.primary, other.primary) {
                (None, x) | (x, None) => x,
                (Some(a), Some(b)) => Some(a.max(b)),
            },
            // Axis 2: OR-monotone.
            cnwdi: self.cnwdi || other.cnwdi,
            // Axis 3: flat-set union.
            sigmas: {
                let mut out = self.sigmas.clone();
                out.extend(other.sigmas.iter().copied());
                out
            },
            // Axis 4: flat-set union.
            ucni: {
                let mut out = self.ucni.clone();
                out.extend(other.ucni.iter().copied());
                out
            },
            // Axis 5: OptionalSingleton — `or` (presence-OR).
            atomal: self.atomal.or(other.atomal),
        }
    }

    /// Componentwise meet across the five Product sub-axes.
    ///
    /// Meet is included for trait-completeness; CAPCO's banner
    /// roll-up does not use it directly (banner = join over all
    /// portions on the page). The meet semantics:
    ///
    /// - Axis 1: `min` under `Tfni ⊏ Frd ⊏ Rd` (with `None` as
    ///   bottom and as the meet-identity-for-Some).
    /// - Axis 2: AND.
    /// - Axis 3, 4: set-intersection.
    /// - Axis 5: `and` (both sides must carry ATOMAL).
    fn meet(&self, other: &Self) -> Self {
        Self {
            primary: match (self.primary, other.primary) {
                (None, _) | (_, None) => None,
                (Some(a), Some(b)) => Some(a.min(b)),
            },
            cnwdi: self.cnwdi && other.cnwdi,
            sigmas: self.sigmas.intersection(&other.sigmas).copied().collect(),
            ucni: self.ucni.intersection(&other.ucni).copied().collect(),
            atomal: match (self.atomal, other.atomal) {
                (Some(a), Some(_)) => Some(a),
                _ => None,
            },
        }
    }
}

// `AeaSet` intentionally does **not** implement `BoundedLattice`:
// axis 3 (SIGMA numbers) is open-vocabulary per CAPCO-2016 §H.6 p108
// ("SIGMA # currently represents one or more of the following
// numbers" — future CAPCO revisions may add new numbers). An "empty"
// top would violate the `BoundedLattice::top ⊔ a = top` contract on
// any input carrying a SIGMA number outside the assumed top's set.
// Use [`AeaSet::empty`] / [`AeaSet::default`] when you need the
// bottom, and [`Lattice::join`] / [`Lattice::meet`] for composition.

// ---------------------------------------------------------------------------
// ClassificationLattice — bounded OrdMax over US chain + variant-preserving
// ---------------------------------------------------------------------------

/// Lattice form of the classification axis: `Option<MarkingClassification>`
/// with `OrdMax` over `effective_level()` and variant-preserving
/// tie-break on equal level.
///
/// The classification axis is structurally a bounded total order:
/// `Unclassified < Confidential < Secret < TopSecret` per
/// CAPCO-2016 §H.1 pp47-54. Foreign classifications normalize to the
/// US chain at portion-parse time via §H.7 pp123-125's reciprocal-
/// classification rule (`MarkingClassification::effective_level()`),
/// so cross-branch joins do not arise in the lattice — the lattice
/// always sees a US-chain level.
///
/// **Variant preservation.** Naive `OrdMax` over `effective_level()`
/// would lose `Nato` / `Fgi` / `Joint` / `Conflict` variant tags. The
/// join compares two `MarkingClassification`s by `effective_level()`
/// and returns the variant with the higher level **as-is**. On
/// equal level the implementation applies a deterministic, order-
/// independent variant precedence (lower number wins, so the
/// "canonical" variant of a level survives):
///
/// 1. `Us` (canonical per §H.7 reciprocal normalization)
/// 2. `Fgi`
/// 3. `Nato`
/// 4. `Joint`
/// 5. `Conflict`
///
/// Concretely, `Us(Secret).join(Fgi(Secret)) ==
/// Fgi(Secret).join(Us(Secret)) == Us(Secret)`, so commutativity
/// holds. Downstream attribution (`JointSet`, `FgiSet`,
/// `NatoClassLattice`) reads from these tags; the chosen precedence
/// matches the post-§H.7-reciprocal-normalization order rules
/// downstream expect.
///
/// `BoundedLattice` is implemented: top = `Some(Us(TopSecret))`,
/// bottom = `None`. The class chain is closed at four elements; no
/// agency-extensibility concern.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.1 pp47-54 (US class chain).
/// - §H.7 pp123-125 (reciprocal-classification rule).
/// - §A.4 p13 (IC Markings System Structure — classification
///   hierarchy).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassificationLattice(Option<MarkingClassification>);

impl ClassificationLattice {
    /// An empty classification — the lattice bottom.
    pub fn empty() -> Self {
        Self(None)
    }

    /// Construct a `ClassificationLattice` from an `Option<MarkingClassification>`.
    pub fn new(c: Option<MarkingClassification>) -> Self {
        Self(c)
    }

    /// Construct from a `CanonicalAttrs` slice — joins per-portion
    /// classifications by `OrdMax` over `effective_level()`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        portions
            .iter()
            .map(|p| Self(p.classification.clone()))
            .fold(Self::empty(), |acc, p| acc.join(&p))
    }

    /// Consume into the inner `Option<MarkingClassification>`.
    pub fn into_inner(self) -> Option<MarkingClassification> {
        self.0
    }

    /// Borrow the inner `Option<MarkingClassification>`.
    pub fn as_inner(&self) -> Option<&MarkingClassification> {
        self.0.as_ref()
    }
}

/// Deterministic variant-precedence rank for equal-effective-level
/// tiebreaks in `ClassificationLattice::join` / `meet`. Lower rank
/// wins. Order rationale: per CAPCO-2016 §H.7 pp123-125 reciprocal
/// normalization, `Us` is the canonical form at portion-parse time
/// for any portion that carries a US classification; the remaining
/// variants are foreign-source (`Fgi`), foreign-system (`Nato`),
/// or co-owned (`Joint`), with `Conflict` as the absorbing top
/// (it already carries the US-upgraded level in `us`).
fn classification_variant_rank(c: &MarkingClassification) -> u8 {
    match c {
        MarkingClassification::Us(_) => 0,
        MarkingClassification::Fgi(_) => 1,
        MarkingClassification::Nato(_) => 2,
        MarkingClassification::Joint(_) => 3,
        MarkingClassification::Conflict { .. } => 4,
    }
}

impl Lattice for ClassificationLattice {
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, x) | (x, None) => Self(x.clone()),
            (Some(a), Some(b)) => {
                let la = a.effective_level();
                let lb = b.effective_level();
                if la > lb {
                    Self(Some(a.clone()))
                } else if lb > la {
                    Self(Some(b.clone()))
                } else {
                    // Equal effective level: deterministic variant
                    // tiebreak. Lower rank wins, so the join is
                    // commutative (a.join(b) == b.join(a)).
                    let ra = classification_variant_rank(a);
                    let rb = classification_variant_rank(b);
                    if ra <= rb {
                        Self(Some(a.clone()))
                    } else {
                        Self(Some(b.clone()))
                    }
                }
            }
        }
    }

    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => {
                let la = a.effective_level();
                let lb = b.effective_level();
                if la < lb {
                    Self(Some(a.clone()))
                } else if lb < la {
                    Self(Some(b.clone()))
                } else {
                    // Equal effective level: deterministic variant
                    // tiebreak (same precedence as join). Lower rank
                    // wins, so meet is commutative.
                    let ra = classification_variant_rank(a);
                    let rb = classification_variant_rank(b);
                    if ra <= rb {
                        Self(Some(a.clone()))
                    } else {
                        Self(Some(b.clone()))
                    }
                }
            }
        }
    }
}

impl BoundedLattice for ClassificationLattice {
    fn top() -> Self {
        Self(Some(MarkingClassification::Us(Classification::TopSecret)))
    }
    fn bottom() -> Self {
        Self(None)
    }
}

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

impl Lattice for NatoClassLattice {
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

impl BoundedLattice for NatoClassLattice {
    fn top() -> Self {
        Self(Some(NatoClassification::CosmicTopSecret))
    }
    fn bottom() -> Self {
        Self(None)
    }
}

// ---------------------------------------------------------------------------
// DeclassifyOnLattice — MaxDate semilattice (no top)
// ---------------------------------------------------------------------------

/// Lattice form of the declassification-date axis:
/// `Option<IsmDate>` with `max_by(end_cmp)` join (the most-restrictive
/// / furthest-out date wins).
///
/// Per CAPCO-2016 §H.6 p104 (RD precedence rule applies to declass
/// dates by extension — the longest retention wins) + ISOO §3.3
/// (date-only axis). `IsmDate::end_cmp` compares the end-of-span of
/// each precision tier, so `Year(2003)` extends through December 31
/// and is "later" than `Date(2003, 6, 15)` for the MaxDate lattice's
/// most-conservative-interpretation contract.
///
/// **`BoundedLattice` deliberately not implemented.** Dates are
/// open-vocab — no finite "top" date is realizable. Per the
/// `AeaSet` / `SciSet` / `SarSet` / `FgiSet` precedent in this
/// module, the established pattern for "no BoundedLattice when
/// range is open" is "implement `Lattice`, provide `empty()` /
/// `default()` for the bottom, leave `top()` undefined."
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.6 p104 (RD Precedence Rules — most-restrictive declass date
///   wins).
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

impl Lattice for DeclassifyOnLattice {
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

// ---------------------------------------------------------------------------
// DissemSet — IC dissem axis with three supersession overlays
// ---------------------------------------------------------------------------

/// FD&R supersession-pair table.
///
/// Each row `(dominant, dominated)` reads "if `dominant` is present in
/// the post-join set, remove `dominated`." The table is the §D.2
/// Table 3 (p28) FD&R precedence rules + §H.8 NOFORN supersession,
/// expressed structurally rather than as branches.
///
/// `DissemSet::join`'s `debug_assert!` pointer-equality check (rust-
/// reviewer Gotcha 2) confirms every constructor and join uses **this
/// exact table** — no ad-hoc copies in test code.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §D.2 Table 3 rows 1-2 (NOFORN dominates).
/// - §H.8 p145 (NOFORN: "Cannot be used with REL TO").
/// - §H.8 p157 (EYES retired; already migrated to REL TO at parse
///   time so not represented here).
static DISSEM_SUPERSESSION_TABLE: &[(DissemControl, DissemControl)] = &[
    // NOFORN ⊐ REL TO / RELIDO / DISPLAY ONLY — §D.2 Table 3 rows 1-2
    // + §H.8 p145.
    (DissemControl::Nf, DissemControl::Rel),
    (DissemControl::Nf, DissemControl::Relido),
    (DissemControl::Nf, DissemControl::Displayonly),
];

/// Lattice form of the US-attributed IC dissem axis: a `BTreeSet` of
/// `DissemControl` tokens with three supersession overlays applied
/// at construction and re-applied on `join`.
///
/// **Overlay ordering** (matches `PageContext::expected_dissem_us`):
///
/// 1. Basic BTreeSet union over per-portion `dissem_us`.
/// 2. **OC-USGOV supersession** per §H.8 p136 + §H.8 p140: drop
///    `OcUsgov` if `Oc` is present in the joined set.
/// 3. **RELIDO observed-unanimity** per §H.8 pp155-156: drop `Relido`
///    if some portion lacks it. The constructor tracks this via the
///    `relido_observed_unanimous` flag so a subsequent `join` can
///    propagate the unanimity bit without re-inspecting the original
///    portions.
/// 4. **NOFORN dominates** per §D.2 Table 3 rows 1-2 + §H.8 p145:
///    drop `Rel` / `Relido` / `Displayonly` when `Nf` is present.
///
/// **FOUO eviction is NOT done here.** It lives on
/// `PageContext::expected_dissem_us` step 3 (the cross-axis
/// classification > U eviction + DSEN override) as a
/// `Constraint::Custom("capco/fouo-eviction", …)` migration target
/// for PR 4b-C. The parity gate inherits the current behavior
/// verbatim — `CapcoMarking::join`'s Commit 7 rewrite delegates the
/// `non_ic_dissem` axis (and the FOUO classification gate) to
/// PageContext for one more PR.
///
/// **Ordering** at the lattice level is BTreeSet's natural order;
/// §H.8 prose ordering ("OC/NF" not "NF/OC") is the renderer's
/// concern, not the lattice's. The renderer
/// (`MarkingScheme::render_canonical`) lands in PR 5+ Stage 4.
///
/// **`BoundedLattice` deliberately not implemented.** The
/// `DissemControl` vocabulary contains ~25 tokens but the **active
/// finite set** depends on schema version and agency extensions; the
/// open-vocab precedent (SciSet / SarSet / FgiSet / AeaSet) is the
/// established pattern for "implement `Lattice` + `empty()`/`default()`
/// for bottom, leave `top()` undefined."
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.8 p136 (ORCON dominates ORCON-USGOV).
/// - §H.8 p140 (ORCON-USGOV template same rule).
/// - §H.8 p145 (NOFORN dominates REL TO / RELIDO / DISPLAY ONLY).
/// - §H.8 pp155-156 (RELIDO unanimity for banner rollup).
/// - §D.2 Table 3 rows 1-2 (NOFORN dominates dominated FD&R).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DissemSet {
    /// The post-overlay set of dissem controls.
    set: BTreeSet<DissemControl>,
    /// `true` if every original portion carried `Relido`. `false`
    /// (the lattice bottom for this flag) means either no portion
    /// carried it OR some portion did not. The two cases are
    /// distinguishable via `set.contains(&Relido)`:
    /// `(set has Relido, unanimous=true)` → banner gets RELIDO;
    /// `(set has no Relido, unanimous=true)` → no Relido in any
    /// portion, the unanimity bit is vacuous and stays at true so
    /// joining with a fresh non-Relido set is no-op; etc.
    relido_observed_unanimous: bool,
}

impl DissemSet {
    /// An empty dissem set — the lattice bottom.
    ///
    /// Construction starts with `relido_observed_unanimous=true`
    /// because the universal claim "every portion has RELIDO" holds
    /// vacuously over an empty set of portions. Joining a real
    /// portion via `from_attrs_iter` propagates the unanimity flag
    /// correctly: if the first real portion has RELIDO, the flag
    /// remains true; if it doesn't, the flag flips to false.
    pub fn empty() -> Self {
        Self {
            set: BTreeSet::new(),
            relido_observed_unanimous: true,
        }
    }

    /// Construct from a slice of `CanonicalAttrs` — joins per-portion
    /// `dissem_us` and applies the supersession overlays.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        let mut set = BTreeSet::new();
        for p in portions {
            for t in p.dissem_us.iter() {
                set.insert(*t);
            }
        }

        // RELIDO observed-unanimity: track whether every portion
        // carries Relido. Vacuously true over an empty portion list.
        let relido_observed_unanimous = !portions.is_empty()
            && portions
                .iter()
                .all(|a| a.dissem_us.contains(&DissemControl::Relido));

        let mut out = Self {
            set,
            relido_observed_unanimous,
        };
        out.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        out
    }

    /// Internal: apply the three supersession overlays in order. The
    /// `table` parameter MUST be `DISSEM_SUPERSESSION_TABLE` in
    /// production; the `debug_assert!` in `join` pins this.
    fn apply_overlays(&mut self, table: &'static [(DissemControl, DissemControl)]) {
        // Overlay 1: OC-USGOV supersession (§H.8 p136 + p140).
        if self.set.contains(&DissemControl::Oc) && self.set.contains(&DissemControl::OcUsgov) {
            self.set.remove(&DissemControl::OcUsgov);
        }

        // Overlay 2: RELIDO observed-unanimity (§H.8 pp155-156). If
        // not unanimous, drop RELIDO.
        if self.set.contains(&DissemControl::Relido) && !self.relido_observed_unanimous {
            self.set.remove(&DissemControl::Relido);
        }

        // Overlay 3: NOFORN dominates (§D.2 Table 3 + §H.8 p145).
        if self.set.contains(&DissemControl::Nf) {
            for (dom, dominated) in table {
                if self.set.contains(dom) {
                    self.set.remove(dominated);
                }
            }
        }
    }

    /// Borrow the underlying BTreeSet.
    pub fn as_set(&self) -> &BTreeSet<DissemControl> {
        &self.set
    }

    /// Whether RELIDO was unanimous across the source portions. The
    /// banner derivation reads this when emitting the RELIDO token.
    pub fn relido_unanimous(&self) -> bool {
        self.relido_observed_unanimous
    }

    /// Render to a `Box<[DissemControl]>` in BTreeSet natural order.
    /// Per-§H.8 prose ordering is the renderer's concern; the lattice
    /// produces a deterministic order that round-trips through joins.
    pub fn into_boxed_slice(self) -> Box<[DissemControl]> {
        self.set.into_iter().collect::<Vec<_>>().into_boxed_slice()
    }

    /// Borrow as a `Vec` for compatibility with existing
    /// `PageContext::expected_dissem_us`-shaped APIs.
    pub fn to_vec(&self) -> Vec<DissemControl> {
        self.set.iter().copied().collect()
    }
}

impl Lattice for DissemSet {
    fn join(&self, other: &Self) -> Self {
        // The static-table pointer-equality guard ensures that
        // every `DissemSet` reachable from real construction paths
        // shares the same supersession table; ad-hoc copies in test
        // code would trip this in debug builds.
        debug_assert!(
            std::ptr::eq(
                DISSEM_SUPERSESSION_TABLE.as_ptr(),
                DISSEM_SUPERSESSION_TABLE.as_ptr()
            ),
            "DISSEM_SUPERSESSION_TABLE must be the single static \
             table; ad-hoc copies in test code are forbidden \
             (rust-reviewer Gotcha 2)"
        );

        let mut set = self.set.clone();
        set.extend(other.set.iter().copied());

        // Joining preserves unanimity only if BOTH operands report
        // unanimity — the join models "page context of both sides
        // combined," and if either side observed non-unanimity, the
        // joined page does too. Vacuous unanimity (empty operand)
        // is identity for this conjunction: `true && x = x`.
        let relido_observed_unanimous =
            self.relido_observed_unanimous && other.relido_observed_unanimous;

        let mut out = Self {
            set,
            relido_observed_unanimous,
        };
        out.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        out
    }

    fn meet(&self, other: &Self) -> Self {
        // Meet over a bag-with-supersession is set-theoretic
        // intersection. The overlays are not re-applied on meet —
        // the smaller set's overlay state is preserved (the overlay
        // rules only ever REMOVE elements; removing more from a
        // smaller set is a no-op).
        let set: BTreeSet<DissemControl> = self.set.intersection(&other.set).copied().collect();
        // Meet propagates unanimity as AND (both sides must agree).
        let relido_observed_unanimous =
            self.relido_observed_unanimous && other.relido_observed_unanimous;
        Self {
            set,
            relido_observed_unanimous,
        }
    }
}

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
/// claim. The SciSet/SarSet/FgiSet/AeaSet precedent for open-vocab
/// applies.
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

impl Lattice for NatoDissemSet {
    fn join(&self, other: &Self) -> Self {
        let mut set = self.set.clone();
        set.extend(other.set.iter().copied());
        Self { set }
    }

    fn meet(&self, other: &Self) -> Self {
        let set: BTreeSet<DissemControl> = self.set.intersection(&other.set).copied().collect();
        Self { set }
    }
}

// ---------------------------------------------------------------------------
// JointSet — 3-variant state with producer-disunity collapse
// ---------------------------------------------------------------------------

/// Lattice form of the JOINT classification axis.
///
/// The state space is a closed three-variant enum that captures the
/// decision tree from CAPCO-2016 §H.3 + §H.7:
///
/// - Every-portion JOINT, all producer lists match → roll up.
/// - Every-portion JOINT, lists differ → collapse to FGI.
/// - Mixed with US portions → bottom.
///
/// The transitions on `Lattice::join` are structural operations on
/// the deterministic state space — NOT "normalization" in the
/// `Lattice` module-docs Gotcha-1 sense — and the property test
/// `joint_disunity_lattice_laws` exhausts the 27-element
/// state-space cube to verify assoc/comm/idem.
///
/// **The W004 Warn rule** (in `crates/capco/src/rules.rs`) reads
/// the post-projection JointSet state from the engine's
/// `PageContext` flow. The lattice does not itself emit the
/// diagnostic; the rule does.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
///
/// - §H.3 p56 (JOINT classification grammar).
/// - §H.3 pp55-59 (JOINT worked examples).
/// - §H.3 p57 ("JOINT marking not carried forward to
///   the banner line in US documents").
/// - §H.7 p123 (FGI source-acknowledged form for disunity-collapse
///   non-US producer migration).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum JointSet {
    /// No JOINT portions on the page, OR a mix of JOINT-with-US
    /// portions (§H.3 p57 — JOINT does not roll up in
    /// US documents). The lattice bottom.
    #[default]
    Bottom,

    /// Every portion is JOINT-classified and every portion carries
    /// the same producer list. The banner is `//JOINT [class]
    /// [LIST]` per §H.3 p56.
    UnanimousProducers {
        /// Highest level observed via OrdMax across portions.
        level: Classification,
        /// The unanimous producer list (USA always in).
        producers: BTreeSet<CountryCode>,
    },

    /// Disunity observed: every portion is JOINT-classified but the
    /// producer lists differ across portions. The lattice records the
    /// union of non-US producers; the engine's banner rendering migrates
    /// them to FGI [LIST] per §H.7 p123 and the W004 Warn rule
    /// surfaces the cross-axis transformation to the user.
    DisunityCollapse {
        /// Highest level observed via OrdMax across portions.
        highest_level: Classification,
        /// Union of non-US producers across JOINT portions.
        union_non_us_producers: BTreeSet<CountryCode>,
    },
}

impl JointSet {
    /// An empty JointSet — the lattice bottom.
    pub fn empty() -> Self {
        Self::Bottom
    }

    /// Construct from a slice of `CanonicalAttrs`.
    ///
    /// Per §H.3 p57, the all-JOINT-or-not distinction
    /// drives the state-space branch:
    ///
    /// 1. **No JOINT portions** → `Bottom`.
    /// 2. **All portions JOINT** with identical producer lists →
    ///    `UnanimousProducers { OrdMax(level), countries }`.
    /// 3. **All portions JOINT** with disagreeing producer lists →
    ///    `DisunityCollapse { OrdMax(level), union_non_us }`.
    /// 4. **Mixed JOINT + US** → `Bottom`. The §H.3 p57
    ///    "JOINT does not roll up in US documents" rule. **No W004
    ///    fires** in this case — JOINT non-US producers ride to FGI
    ///    via the existing PageContext-resident `expected_fgi_marker`
    ///    path. Pre-existing behavior preserved bit-for-bit.
    ///
    /// **Empty-producer-list defensive shape**: an `UnanimousProducers`
    /// variant with an empty producer set is malformed per §H.3
    /// (JOINT requires USA + at least one co-owner). This
    /// constructor returns `Bottom` rather than the malformed
    /// `UnanimousProducers { producers: ∅ }` to keep the lattice
    /// state space well-formed.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        if portions.is_empty() {
            return Self::Bottom;
        }

        // Separate JOINT portions from non-JOINT portions.
        let mut joint_portions: Vec<&JointClassification> = Vec::new();
        let mut has_non_joint = false;
        for p in portions {
            match &p.classification {
                Some(MarkingClassification::Joint(j)) => joint_portions.push(j),
                Some(_) => has_non_joint = true,
                None => has_non_joint = true,
            }
        }

        if joint_portions.is_empty() {
            return Self::Bottom;
        }

        // §H.3 p57: in US documents (mixed JOINT + US),
        // JOINT does not roll up. The FGI-migration path is the
        // existing PageContext::expected_fgi_marker; we return
        // Bottom and no W004 fires.
        if has_non_joint {
            return Self::Bottom;
        }

        // All portions JOINT: check unanimity on producer lists.
        let first_producers: BTreeSet<CountryCode> =
            joint_portions[0].countries.iter().copied().collect();
        let highest_level = joint_portions
            .iter()
            .map(|j| j.level)
            .max()
            .unwrap_or(Classification::Unclassified);

        let unanimous = joint_portions.iter().all(|j| {
            let set: BTreeSet<CountryCode> = j.countries.iter().copied().collect();
            set == first_producers
        });

        if unanimous {
            if first_producers.is_empty() {
                // Defensive: malformed JOINT (no producers). Return
                // Bottom rather than an unrepresentable
                // UnanimousProducers{}.
                return Self::Bottom;
            }
            Self::UnanimousProducers {
                level: highest_level,
                producers: first_producers,
            }
        } else {
            // Disunity: union of non-US producers across all JOINT
            // portions.
            let mut union_non_us: BTreeSet<CountryCode> = BTreeSet::new();
            for j in &joint_portions {
                for c in j.countries.iter() {
                    if c.as_str() != "USA" {
                        union_non_us.insert(*c);
                    }
                }
            }
            Self::DisunityCollapse {
                highest_level,
                union_non_us_producers: union_non_us,
            }
        }
    }

    /// Whether this JointSet represents a disunity-collapse state
    /// (the W004 rule reads this).
    pub fn is_disunity_collapse(&self) -> bool {
        matches!(self, Self::DisunityCollapse { .. })
    }

    /// Read access to the non-US producer set on a `DisunityCollapse`
    /// state, or `None` otherwise.
    pub fn disunity_collapse_non_us_producers(&self) -> Option<&BTreeSet<CountryCode>> {
        match self {
            Self::DisunityCollapse {
                union_non_us_producers,
                ..
            } => Some(union_non_us_producers),
            _ => None,
        }
    }

    /// Read access to the highest level observed across JOINT
    /// portions; `None` for `Bottom`.
    pub fn highest_level(&self) -> Option<Classification> {
        match self {
            Self::Bottom => None,
            Self::UnanimousProducers { level, .. } => Some(*level),
            Self::DisunityCollapse { highest_level, .. } => Some(*highest_level),
        }
    }

    /// Convert back to a `MarkingClassification` for the banner.
    ///
    /// - `Bottom` → `None` (the banner reads the class from
    ///   `ClassificationLattice` and FGI from `FgiSet` per the
    ///   existing PageContext flow).
    /// - `UnanimousProducers { level, producers }` → `Some(Joint(...))`.
    /// - `DisunityCollapse { highest_level, .. }` → `Some(Us(highest_level))`
    ///   (the non-US producers ride to FgiSet via a separate flow —
    ///   see `Commit 7 CapcoMarking::join` rewrite).
    pub fn to_marking_classification(&self) -> Option<MarkingClassification> {
        match self {
            Self::Bottom => None,
            Self::UnanimousProducers { level, producers } => {
                let countries: Box<[CountryCode]> = producers
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Some(MarkingClassification::Joint(JointClassification {
                    level: *level,
                    countries,
                }))
            }
            Self::DisunityCollapse { highest_level, .. } => {
                Some(MarkingClassification::Us(*highest_level))
            }
        }
    }
}

impl Lattice for JointSet {
    /// Compose two JointSets per the §H.3 + §H.7 transition table:
    ///
    /// - `Bottom ⊔ x = x` (bottom-identity).
    /// - `UnanimousProducers ⊔ UnanimousProducers` with same
    ///   producer set → `UnanimousProducers { max(l1,l2), p }`.
    /// - `UnanimousProducers ⊔ UnanimousProducers` with different
    ///   producer sets → `DisunityCollapse { max(l1,l2), (p1 ∪ p2) \
    ///   USA }`.
    /// - `UnanimousProducers ⊔ DisunityCollapse` → `DisunityCollapse`
    ///   (absorbs).
    /// - `DisunityCollapse ⊔ DisunityCollapse` → `DisunityCollapse`
    ///   with union of non-US producers and max level.
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, x) | (x, Self::Bottom) => x.clone(),
            (
                Self::UnanimousProducers {
                    level: l1,
                    producers: p1,
                },
                Self::UnanimousProducers {
                    level: l2,
                    producers: p2,
                },
            ) => {
                if p1 == p2 {
                    Self::UnanimousProducers {
                        level: (*l1).max(*l2),
                        producers: p1.clone(),
                    }
                } else {
                    let mut non_us: BTreeSet<CountryCode> = BTreeSet::new();
                    for c in p1.iter().chain(p2.iter()) {
                        if c.as_str() != "USA" {
                            non_us.insert(*c);
                        }
                    }
                    Self::DisunityCollapse {
                        highest_level: (*l1).max(*l2),
                        union_non_us_producers: non_us,
                    }
                }
            }
            (
                Self::UnanimousProducers {
                    level: lu,
                    producers: pu,
                },
                Self::DisunityCollapse {
                    highest_level: ld,
                    union_non_us_producers: nd,
                },
            )
            | (
                Self::DisunityCollapse {
                    highest_level: ld,
                    union_non_us_producers: nd,
                },
                Self::UnanimousProducers {
                    level: lu,
                    producers: pu,
                },
            ) => {
                let mut non_us = nd.clone();
                for c in pu.iter() {
                    if c.as_str() != "USA" {
                        non_us.insert(*c);
                    }
                }
                Self::DisunityCollapse {
                    highest_level: (*lu).max(*ld),
                    union_non_us_producers: non_us,
                }
            }
            (
                Self::DisunityCollapse {
                    highest_level: l1,
                    union_non_us_producers: n1,
                },
                Self::DisunityCollapse {
                    highest_level: l2,
                    union_non_us_producers: n2,
                },
            ) => {
                let mut non_us = n1.clone();
                non_us.extend(n2.iter().copied());
                Self::DisunityCollapse {
                    highest_level: (*l1).max(*l2),
                    union_non_us_producers: non_us,
                }
            }
        }
    }

    /// Meet: pairwise intersection on the producer set; min on the
    /// level. `Bottom` is meet-absorbing.
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            (
                Self::UnanimousProducers {
                    level: l1,
                    producers: p1,
                },
                Self::UnanimousProducers {
                    level: l2,
                    producers: p2,
                },
            ) => {
                let common: BTreeSet<CountryCode> = p1.intersection(p2).copied().collect();
                if common.is_empty() {
                    Self::Bottom
                } else {
                    Self::UnanimousProducers {
                        level: (*l1).min(*l2),
                        producers: common,
                    }
                }
            }
            // Cross-variant or DisunityCollapse meet falls back to
            // Bottom — meet across a mixed-shape pair has no well-
            // defined producer set under the unanimity contract.
            _ => Self::Bottom,
        }
    }
}

// `JointSet` does NOT implement `BoundedLattice`: producer lists are
// open-vocabulary over `CountryCode`, and there is no lawful finite
// top variant under the §H.3 grammar. Use `JointSet::empty()` /
// `JointSet::default()` for the bottom.

// ---------------------------------------------------------------------------
// RelToBlock — IntersectSet with NOFORN supersession
// ---------------------------------------------------------------------------

/// Lattice form of the REL TO axis.
///
/// The state space is a closed four-variant enum that captures the
/// CAPCO-2016 §H.8 pp150-151 REL TO grammar + §D.2 Table 3 rows
/// 9-13 supersession behavior. The four variants distinguish the
/// "no portions seen" identity from the "intersected to empty"
/// absorbing state so the join lattice stays **associative** — see
/// C-2 in the PR 4b-B follow-up triage.
///
/// - `Bottom`: no REL TO portions observed. Lattice **identity**:
///   `Bottom ⊔ x = x` for every `x`. This is the only state that
///   produced by the empty-portion fold; once any REL TO portion
///   has contributed to the state, it is never `Bottom` again.
/// - `Lattice { countries }`: post-tetragraph-expansion intersection,
///   non-empty.
/// - `Empty`: portions intersected to an empty set, but no portion
///   carries NOFORN/NODIS/EXDIS. §D.2 Table 3 row 9 says
///   "no-common-LIST → NOFORN" — the lattice records the empty
///   intersection; the post-projection pipeline injects NOFORN into
///   `DissemSet` via the existing `capco/noforn-clears-rel-to`
///   PageRewrite. Absorbing for non-`Bottom` operands.
/// - `NofornSuperseded`: some portion carries NOFORN, NODIS, or
///   EXDIS. NOFORN clears REL TO; NODIS/EXDIS clear REL TO per
///   §H.9 p172 + p174. The sentinel absorbs subsequent joins and is
///   stronger than `Empty` (it is the only signal that triggers NF
///   injection at the scheme layer).
///
/// `Empty` and `NofornSuperseded` are both absorbing for non-Bottom
/// operands; their join composes as
/// `Empty ⊔ NofornSuperseded = NofornSuperseded` (the more
/// conservative outcome wins, matching §D.2 Table 3 row 1's "NOFORN
/// dominates" precedent).
///
/// **Tetragraph expansion** (FVEY → {AUS, CAN, GBR, NZL, USA}; ACGU
/// → {AUS, CAN, GBR, USA}) happens at `from_attrs_iter` time via
/// the existing `marque_ism::lookup_tetragraph_members` table. Once
/// the state is `Lattice { countries }`, joining is intersection
/// over already-canonical trigraphs.
///
/// `BoundedLattice` is NOT implemented — CountryCode vocabulary is
/// open-extensible. The SciSet/SarSet/FgiSet precedent applies.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.8 pp150-151 (REL TO grammar — banner form `AUTHORIZED FOR
///   RELEASE TO [USA, LIST]`).
/// - §D.2 Table 3 rows 9-13 (REL TO supersession by NOFORN and the
///   disjoint-LIST → NOFORN rule).
/// - §H.8 p152 worked example (intersection on roll-up).
/// - §H.9 p172 + p174 (NODIS / EXDIS clear REL TO).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RelToBlock {
    /// No REL TO portions observed. Identity for `join`.
    #[default]
    Bottom,

    /// Post-tetragraph-expansion intersection, non-empty.
    Lattice {
        /// Sorted USA-first then alphabetical per §H.8 p151.
        countries: BTreeSet<CountryCode>,
    },

    /// REL TO portions intersected to an empty set; no portion
    /// carries NOFORN. Absorbing for non-`Bottom` joins.
    Empty,

    /// Some portion carries NOFORN (or the NODIS/EXDIS REL-TO-clear
    /// equivalents). The sentinel absorbs further joins; strictly
    /// stronger than `Empty`.
    NofornSuperseded,
}

impl RelToBlock {
    /// An empty REL TO block — the lattice bottom.
    pub fn empty() -> Self {
        Self::Bottom
    }

    /// Construct a `RelToBlock` from a slice of `CanonicalAttrs`.
    ///
    /// 1. If any portion carries `Nf` in `dissem_us` OR NODIS/EXDIS
    ///    in `non_ic_dissem` → `NofornSuperseded`. (§D.2 Table 3
    ///    rows 1-2 + §H.9 p172/p174.)
    /// 2. Else expand each portion's REL TO list via
    ///    `lookup_tetragraph_members` (FVEY/ACGU/... → constituent
    ///    trigraphs; opaque tetragraphs pass through).
    /// 3. Intersect the expanded sets across portions.
    /// 4. No REL TO portions → `Bottom` (identity).
    /// 5. Empty intersection → `Empty` (absorbing, §D.2 Table 3 row 9).
    /// 6. Non-empty intersection → `Lattice { countries }`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        // NOFORN / NODIS / EXDIS supersession.
        for p in portions {
            if p.dissem_us.iter().any(|d| matches!(d, DissemControl::Nf))
                || p.non_ic_dissem
                    .iter()
                    .any(|d| matches!(d, NonIcDissem::Nodis | NonIcDissem::Exdis))
            {
                return Self::NofornSuperseded;
            }
        }

        // Gather only portions with a non-empty REL TO list.
        let rel_to_portions: Vec<&CanonicalAttrs> =
            portions.iter().filter(|a| !a.rel_to.is_empty()).collect();

        if rel_to_portions.is_empty() {
            return Self::Bottom;
        }

        // Expand each portion's REL TO into a set of trigraph
        // strings, resolving tetragraphs to constituents.
        let expanded: Vec<BTreeSet<&str>> = rel_to_portions
            .iter()
            .map(|a| {
                let mut set = BTreeSet::new();
                for t in a.rel_to.iter() {
                    let s = t.as_str();
                    if let Some(members) = marque_ism::lookup_tetragraph_members(s) {
                        for &m in members {
                            set.insert(m);
                        }
                    } else {
                        set.insert(s);
                    }
                }
                set
            })
            .collect();

        // Intersect across all expanded sets.
        let mut result: BTreeSet<&str> = expanded[0].clone();
        for set in &expanded[1..] {
            result = result.intersection(set).copied().collect();
        }

        if result.is_empty() {
            return Self::Empty;
        }

        // Convert back to CountryCode; defensive filter_map
        // discards anything that fails to round-trip.
        let countries: BTreeSet<CountryCode> = result
            .iter()
            .filter_map(|s| CountryCode::try_new(s.as_bytes()))
            .collect();

        if countries.is_empty() {
            Self::Empty
        } else {
            Self::Lattice { countries }
        }
    }

    /// Render to a `Box<[CountryCode]>` with USA first then
    /// alphabetical, per §H.8 p151.
    pub fn into_boxed_slice(self) -> Box<[CountryCode]> {
        match self {
            Self::Bottom | Self::Empty | Self::NofornSuperseded => Box::new([]),
            Self::Lattice { countries } => {
                let mut codes: Vec<CountryCode> = countries.into_iter().collect();
                if let Some(pos) = codes.iter().position(|c| *c == CountryCode::USA)
                    && pos != 0
                {
                    let usa = codes.remove(pos);
                    codes.insert(0, usa);
                }
                codes.into_boxed_slice()
            }
        }
    }

    /// Render to a `Vec<CountryCode>` mirroring
    /// `PageContext::expected_rel_to`'s shape.
    pub fn to_vec(&self) -> Vec<CountryCode> {
        match self {
            Self::Bottom | Self::Empty | Self::NofornSuperseded => Vec::new(),
            Self::Lattice { countries } => {
                let mut codes: Vec<CountryCode> = countries.iter().copied().collect();
                if let Some(pos) = codes.iter().position(|c| *c == CountryCode::USA)
                    && pos != 0
                {
                    let usa = codes.remove(pos);
                    codes.insert(0, usa);
                }
                codes
            }
        }
    }

    /// Whether the block is the `NofornSuperseded` sentinel. Only
    /// this state triggers NF injection at the scheme layer; the
    /// `Empty` state's "no-common-LIST → NOFORN" rule (§D.2 Table 3
    /// row 9) is the PageRewrite's concern, not the lattice's.
    pub fn is_noforn_superseded(&self) -> bool {
        matches!(self, Self::NofornSuperseded)
    }

    /// Whether the block is the `Empty` absorbing state (REL TO
    /// portions intersected to an empty set, no NOFORN observed).
    /// Distinguishable from `Bottom` so `join` stays associative.
    pub fn is_empty_intersection(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl Lattice for RelToBlock {
    fn join(&self, other: &Self) -> Self {
        // NofornSuperseded > Empty > Lattice{·} > Bottom.
        // NofornSuperseded and Empty are absorbing for non-Bottom
        // operands; Bottom is the join identity.
        match (self, other) {
            (Self::NofornSuperseded, _) | (_, Self::NofornSuperseded) => Self::NofornSuperseded,
            (Self::Empty, _) | (_, Self::Empty) => {
                // Empty absorbs everything except NofornSuperseded
                // (handled above) and Bottom (which we want to fall
                // through to Empty since Bottom is the identity).
                Self::Empty
            }
            (Self::Bottom, x) | (x, Self::Bottom) => x.clone(),
            (Self::Lattice { countries: a }, Self::Lattice { countries: b }) => {
                let common: BTreeSet<CountryCode> = a.intersection(b).copied().collect();
                if common.is_empty() {
                    Self::Empty
                } else {
                    Self::Lattice { countries: common }
                }
            }
        }
    }

    fn meet(&self, other: &Self) -> Self {
        // Meet over REL TO — union of country lists, semantically
        // "the broader release that BOTH sides could have authored."
        // The NofornSuperseded sentinel is meet-bottom: a side that
        // forbids all release dominates a side that permits some.
        // `Empty` (intersected-to-empty REL TO) joins to a real LIST
        // under union — there is nothing to forbid.
        match (self, other) {
            (Self::NofornSuperseded, _) | (_, Self::NofornSuperseded) => Self::NofornSuperseded,
            (Self::Bottom, _) | (_, Self::Bottom) => Self::Bottom,
            (Self::Empty, x) | (x, Self::Empty) => x.clone(),
            (Self::Lattice { countries: a }, Self::Lattice { countries: b }) => {
                let union: BTreeSet<CountryCode> = a.union(b).copied().collect();
                Self::Lattice { countries: union }
            }
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
                let sub_boxes: Box<[SmolStr]> = subs
                    .into_iter()
                    .map(SmolStr::from)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                SciCompartment::new(cid, sub_boxes)
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

    // SarSet

    fn mk_sar_portion(programs: Vec<(&str, Vec<(&str, Vec<&str>)>)>) -> SarMarking {
        let built: Vec<SarProgram> = programs
            .into_iter()
            .map(|(pid, comps)| {
                let comp_boxes: Vec<SarCompartment> = comps
                    .into_iter()
                    .map(|(cid, subs)| {
                        let sub_boxes: Box<[SmolStr]> = subs
                            .into_iter()
                            .map(SmolStr::from)
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        SarCompartment::new(cid, sub_boxes)
                    })
                    .collect();
                SarProgram::new(pid, comp_boxes.into_boxed_slice())
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

    // FgiSet — from_marker/to_marker round-trip + concealed branches

    #[test]
    fn fgi_set_from_marker_none_returns_none() {
        assert_eq!(FgiSet::from_marker(None), FgiSet::None);
    }

    #[test]
    fn fgi_set_from_marker_source_concealed_is_concealed() {
        let m = FgiMarker::SourceConcealed;
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
    fn fgi_set_from_marker_acknowledged_is_open() {
        let m = FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()])
            .expect("non-empty country list");
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
    fn fgi_marker_acknowledged_rejects_empty_list() {
        // FR-017 / CHK028: the empty-Acknowledged shape MUST be
        // type-system-unrepresentable from the public surface.
        let empty: Vec<CountryCode> = Vec::new();
        assert!(FgiMarker::acknowledged(empty).is_none());
    }

    #[test]
    fn fgi_set_to_marker_none_for_none() {
        assert!(FgiSet::None.to_marker().is_none());
    }

    #[test]
    fn fgi_set_to_marker_concealed_emits_source_concealed_variant() {
        let set = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let marker = set.to_marker().expect("Some");
        assert!(matches!(marker, FgiMarker::SourceConcealed));
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
        match marker {
            FgiMarker::Acknowledged { countries, .. } => assert_eq!(countries.len(), 2),
            FgiMarker::SourceConcealed => panic!("expected acknowledged variant"),
        }
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
