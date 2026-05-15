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
    AeaMarking, AtomalBlock, CountryCode, FgiMarker, FrdBlock, RdBlock, SarCompartment,
    SarIndicator, SarMarking, SarProgram, SciCompartment, SciControlSystem, SciMarking,
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
///    not an ATOMAL subsection; ATOMAL has no dedicated §H entry,
///    its registration lives in §G.2 Table 5). The PR 9c.1 T134
///    routing decision tracked this through the parser layer.
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

        // Axis 1 + cross-axis SIGMA coalescing per §H.6 p108-109.
        // SIGMA rides on the primary axis output (RD or FRD); under
        // Tfni-primary the SIGMA set is silently dropped because §H.6
        // p120 has no SIGMA modifier and the inputs that produced it
        // would have come from RD or FRD portions that got superseded.
        match self.primary {
            Some(AeaPrimary::Rd) => {
                out.push(AeaMarking::Rd(RdBlock {
                    cnwdi: self.cnwdi,
                    sigma: sigmas,
                }));
            }
            Some(AeaPrimary::Frd) => {
                // CNWDI is RD-only per §H.6 p106 — if the lattice
                // joined to `cnwdi=true, primary=Frd`, the input was
                // malformed (caught by the E067/cnwdi-requires-rd
                // Constraint). The render here drops cnwdi silently
                // because the FRD block has no CNWDI modifier.
                out.push(AeaMarking::Frd(FrdBlock { sigma: sigmas }));
            }
            Some(AeaPrimary::Tfni) => {
                out.push(AeaMarking::Tfni);
            }
            None => {
                // No primary AEA marking on the page; CNWDI / SIGMA
                // alone are not renderable without a primary anchor.
                // The E067 Constraint catches CNWDI-without-RD; SIGMA
                // without a primary is similarly invalid input but
                // not currently constrained (§H.6 p108 says SIGMA
                // "Requires RD" but Marque does not yet emit a
                // `SIGMA-requires-RD` constraint — tracked as future
                // work, not regression here).
            }
        }
        // Axis 4 — UCNI variants in §G.1 Table 4 order.
        if self.ucni.contains(&UcniKind::DodUcni) {
            out.push(AeaMarking::DodUcni);
        }
        if self.ucni.contains(&UcniKind::DoeUcni) {
            out.push(AeaMarking::DoeUcni);
        }
        // Axis 5 — ATOMAL.
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
