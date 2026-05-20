// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO structural lattice types.
//!
//! The types in this module are the lattice-form counterparts to the
//! structural types [`marque_ism::SciMarking`], [`marque_ism::SarMarking`],
//! and [`marque_ism::FgiMarker`] — newtype wrappers that implement
//! [`Lattice`] so CAPCO's structural categories compose through the
//! generic engine machinery. Post-PR-4b-E (this module's
//! `*::from_attrs_iter` constructors + free helpers like
//! [`sci_controls_from_markings`]) these helpers ARE the production
//! page-roll-up path — the retired `PageContext::expected_*` accessor
//! surface was the pre-PR-4b-E shape.
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
use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet};

/// Sort a slice of `&SmolStr` by `marque_ism::sar_sort_key` (CAPCO §H.5 p99
/// numeric-first, alphabetic-after; also serves SCI §A.6 p15 which shares the
/// same numeric-then-alpha rule).
///
/// Single named site so the compiler emits exactly one
/// `slice::sort::stable::quicksort::quicksort<&SmolStr, _>` instantiation
/// regardless of how many call sites use it (per PR 4b-perf LA-1 follow-up
/// — issue #585). Previously each `.sort_by(|a, b| sar_sort_key(a)…)` call
/// emitted a distinct ~15.6 KiB monomorphization; consolidating the 4
/// `&SmolStr` sites + 1 tuple site (refactored to sort keys-first) collapses
/// 5 monos to 1.
///
/// Deliberately NOT `#[inline]`: the closure here has a single anonymous type
/// (`sort_smolstrs_by_sar::{closure#0}`) regardless of inlining decisions, so
/// the mono guarantee holds either way. Omitting the hint avoids contradicting
/// the doc comment's "single named site" framing — `lto = "fat"` (workspace
/// `Cargo.toml`) handles whole-program inlining naturally if profitable.
fn sort_smolstrs_by_sar(slice: &mut [&SmolStr]) {
    slice.sort_by(|a, b| marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b)));
}

// ---------------------------------------------------------------------------
// HierarchicalTreeSet — shared 3-level tree storage primitive
// ---------------------------------------------------------------------------

/// A 3-level hierarchical tree: `K → SmolStr → SmolStr`.
///
/// Backed by `BTreeMap<K, BTreeMap<SmolStr, BTreeSet<SmolStr>>>`, this
/// type captures the repeated "outer key → compartments → sub-compartments"
/// pattern shared by [`SciSet`] (outer key = [`SystemKey`]) and [`SarSet`]
/// (outer key = [`SmolStr`]). Both types differ only in their outer key
/// type; this generic struct lets them share [`join_with`][Self::join_with],
/// [`meet_with`][Self::meet_with], and the sorted-traversal helper
/// [`sorted_entries`][Self::sorted_entries] without duplicating the
/// iteration logic.
///
/// Hot-path methods (`join_with`, `meet_with`, and the simple accessors)
/// are marked `#[inline]`. `sorted_entries` is deliberately not inlined
/// — it takes an `impl Fn` closure argument whose type is generic at each
/// call site, so marking it `#[inline]` would produce a distinct
/// monomorphization per call site (mirroring the `sort_smolstrs_by_sar`
/// design note above). The single non-inlined definition plus LTO handles
/// whole-program optimization when profitable.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HierarchicalTreeSet<K: Clone + Ord> {
    inner: BTreeMap<K, BTreeMap<SmolStr, BTreeSet<SmolStr>>>,
}

/// Manual `Default` impl so that `HierarchicalTreeSet<K>` is `Default`
/// even when `K: !Default` (matching `BTreeMap`'s own `Default` bound).
impl<K: Clone + Ord> Default for HierarchicalTreeSet<K> {
    fn default() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }
}

impl<K: Clone + Ord> HierarchicalTreeSet<K> {
    #[inline]
    fn empty() -> Self {
        Self::default()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a mutable reference to the compartment map for `key`,
    /// inserting an empty entry if absent. Used by `from_markings*`
    /// constructors to record bare (no-compartment) entries without a
    /// separate "ensure entry" step.
    #[inline]
    fn entry_outer(&mut self, key: K) -> &mut BTreeMap<SmolStr, BTreeSet<SmolStr>> {
        self.inner.entry(key).or_default()
    }

    /// Returns the compartment map for `key`, or `None` if absent.
    #[inline]
    fn get(&self, key: &K) -> Option<&BTreeMap<SmolStr, BTreeSet<SmolStr>>> {
        self.inner.get(key)
    }

    /// Returns `true` if `key` is present in the outer map.
    #[inline]
    fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// Iterates over outer keys in `BTreeMap` order.
    #[inline]
    fn keys(&self) -> impl Iterator<Item = &K> {
        self.inner.keys()
    }

    /// Iterates over `(outer_key, compartment_map)` pairs in `BTreeMap`
    /// order.
    #[inline]
    fn iter(&self) -> impl Iterator<Item = (&K, &BTreeMap<SmolStr, BTreeSet<SmolStr>>)> {
        self.inner.iter()
    }

    /// Component-wise union: for each outer key in either operand, union
    /// its compartment map; within each compartment, union its
    /// sub-compartment set. Implements the `join` for both [`SciSet`] and
    /// [`SarSet`].
    #[inline]
    fn join_with(&self, other: &Self) -> Self {
        let mut out = self.clone();
        for (outer_key, comp_map) in &other.inner {
            let out_comps = out.inner.entry(outer_key.clone()).or_default();
            for (cid, subs) in comp_map {
                let out_subs = out_comps.entry(cid.clone()).or_default();
                out_subs.extend(subs.iter().cloned());
            }
        }
        out
    }

    /// Component-wise equal-depth intersection per §3.3a policy (b): an
    /// outer key survives only if present in both operands; within a
    /// surviving key a compartment survives only if present in both; within
    /// a surviving compartment sub-compartments are intersected. Implements
    /// the `meet` for both [`SciSet`] and [`SarSet`].
    #[inline]
    fn meet_with(&self, other: &Self) -> Self {
        let mut out = Self::empty();
        for (outer_key, comp_map) in &self.inner {
            let Some(other_comps) = other.inner.get(outer_key) else {
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
            out.inner.insert(outer_key.clone(), out_comps);
        }
        out
    }

    /// Returns entries sorted by `sar_sort_key` applied to the text form of
    /// the outer key. Used by `to_markings` / `to_marking` to produce
    /// deterministic §A.6 / §H.5 ordering (numeric-prefixed identifiers
    /// first, then alphabetic).
    ///
    /// The `key_text` closure extracts a `&str` from `K` because `K`'s
    /// natural `Ord` may not match CAPCO sort order — e.g. `SystemKey`
    /// sorts by an internal discriminant while §A.6 p15 requires
    /// numeric-first ordering over the textual form. Passing the text
    /// projection decouples the storage key ordering from the rendering
    /// order without introducing a separate `OrdText` bound on `K`.
    ///
    /// A lexicographic tie-breaker on the raw `key_text` string is applied
    /// after the `sar_sort_key` comparison so that two distinct keys whose
    /// sort keys collide (e.g., two numeric identifiers that both map to
    /// `u64::MAX` under overflow) still produce a stable, total order.
    ///
    /// Inline-4 covers the typical outer-key count (SCI: SI/TK/HCS/G;
    /// SAR: ≤4 programs per portion in ordinary documents) so the sorted
    /// scratch buffer stays on the stack on the hot path (LA-4 fix per
    /// PR 614 — inline capacity tuning in `to_markings` / `to_marking`).
    #[allow(clippy::type_complexity)] // Inline-4 SmallVec is load-bearing (LA-4); a type alias would hide the capacity.
    fn sorted_entries(
        &self,
        key_text: impl Fn(&K) -> &str,
    ) -> SmallVec<[(&K, &BTreeMap<SmolStr, BTreeSet<SmolStr>>); 4]> {
        let mut entries: SmallVec<[_; 4]> = self.inner.iter().collect();
        entries.sort_by(|a, b| {
            let ta = key_text(a.0);
            let tb = key_text(b.0);
            marque_ism::sar_sort_key(ta)
                .cmp(&marque_ism::sar_sort_key(tb))
                .then_with(|| ta.cmp(tb))
        });
        entries
    }
}

/// Sort and render a compartment map into a list of
/// `(identifier, sorted-sub-compartments)` pairs. Shared rendering
/// helper for [`SciSet::to_markings`] and [`SarSet::to_marking`].
///
/// Inline capacities (LA-4 fix per PR 614):
/// - `comp_keys` scratch: inline-8 covers the typical SCI compartment
///   count (the `NF/PR/OC/REL/IMCON/RS` shape and similar) plus headroom;
///   SAR compartment counts are typically ≤4 per program but inline-8 is
///   a stack-only ceiling, no waste on the heap path.
/// - per-compartment `subs` scratch: inline-4 covers the typical
///   sub-compartment count for both SCI and SAR.
/// - returned vec: inline-8 matches PR 614's `compartments` capacity for
///   the SCI rendering path; callers consume via `into_iter` so the
///   inline storage is dropped at the end of the iteration without an
///   additional Box round-trip.
#[allow(clippy::type_complexity)] // Inline-8 SmallVec is load-bearing (LA-4); a type alias would hide the capacity.
fn sorted_compartment_items(
    comp_map: &BTreeMap<SmolStr, BTreeSet<SmolStr>>,
) -> SmallVec<[(&SmolStr, Box<[SmolStr]>); 8]> {
    let mut comp_keys: SmallVec<[&SmolStr; 8]> = comp_map.keys().collect();
    sort_smolstrs_by_sar(&mut comp_keys);
    comp_keys
        .into_iter()
        .map(|id| {
            let sub_set = comp_map
                .get(id)
                .expect("compartment key must exist in map (internal invariant violated)");
            let mut subs: SmallVec<[&SmolStr; 4]> = sub_set.iter().collect();
            sort_smolstrs_by_sar(&mut subs);
            let sub_boxes: Box<[SmolStr]> = subs.into_iter().cloned().collect();
            (id, sub_boxes)
        })
        .collect()
}

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
    /// sub-compartments. Delegates to [`HierarchicalTreeSet::join_with`].
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
    /// Delegates to [`HierarchicalTreeSet::meet_with`].
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
    /// sub-compartments. Delegates to [`HierarchicalTreeSet::join_with`].
    fn join(&self, other: &Self) -> Self {
        Self {
            programs: self.programs.join_with(&other.programs),
        }
    }
}

impl MeetSemilattice for SarSet {
    /// Component-wise equal-depth intersection per §3.3a policy (b).
    /// Delegates to [`HierarchicalTreeSet::meet_with`].
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
/// (concealed), the banner must also be concealed. Meet is dual: the
/// source-concealed form acts as the lattice top for the FGI
/// source-disclosure dimension, so meet with a concealed operand returns
/// the OTHER operand (the acknowledged side), and meet of two concealed
/// operands returns concealed. Meet of two acknowledged operands
/// intersects their country sets; an empty intersection collapses to
/// `None` (no shared FGI).
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
///
/// **`#[non_exhaustive]`** (B-4, PR 4b-B 8th-pass follow-up): the
/// state space is closed today (`None` and `Present { concealed, countries }`
/// over an open `CountryCode` axis), but future CAPCO grammar
/// extensions or decoder-confidence partial states may add a
/// `Partial` / `Concealed { partial_countries: ... }` variant
/// without breaking the closed-set contract for the existing two
/// — declaring `#[non_exhaustive]` keeps downstream matchers honest
/// (they MUST handle the unknown case with a wildcard arm).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
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

impl JoinSemilattice for FgiSet {
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
}

impl MeetSemilattice for FgiSet {
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
                // P-9-1 (9th-pass): source-concealed acts as lattice TOP
                // in the FGI source-disclosure dimension.  The join already
                // makes concealed dominate (P-1, 8th-pass), so the dual
                // absorption law `a ⊓ (a ⊔ b) = a` requires meet to treat
                // the concealed form as top — meet(x, top) = x.
                //
                // Three cases:
                //   (a) both concealed  → concealed (idempotent top)
                //   (b) one concealed, one acknowledged → acknowledged side
                //       (meet with top returns the other operand)
                //   (c) both acknowledged → intersect country sets
                //
                // Authority: §H.7 p128 ("A document containing portions of
                // both source-concealed FGI and source-acknowledged FGI must
                // have only the 'FGI' marking without source
                // trigraph(s)/tetragraph(s) in the banner line, as it is the
                // most restrictive form of the marking") — concealed is the
                // strictest / highest element. Verified 2026-05-16 against
                // crates/capco/docs/CAPCO-2016.md.
                match (*a_c, *b_c) {
                    (true, true) => {
                        // (a) both concealed — top ⊓ top = top.
                        Self::Present {
                            concealed: true,
                            countries: BTreeSet::new(),
                        }
                    }
                    (true, false) => {
                        // (b) self is concealed (top) → return other.
                        Self::Present {
                            concealed: false,
                            countries: b_cs.clone(),
                        }
                    }
                    (false, true) => {
                        // (b) other is concealed (top) → return self.
                        Self::Present {
                            concealed: false,
                            countries: a_cs.clone(),
                        }
                    }
                    (false, false) => {
                        // (c) both acknowledged — intersect country sets.
                        let countries: BTreeSet<CountryCode> =
                            a_cs.intersection(b_cs).copied().collect();
                        if countries.is_empty() {
                            // No common countries — collapse to bottom
                            // (no shared FGI on this page).
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
}

// `FgiSet` deliberately does NOT implement `BoundedLattice` (B-1, PR 4b-B
// 8th-pass follow-up). Although `SourceConcealed` is a valid syntactic
// supersession-top for the `Lattice::join` operation (it dominates every
// non-concealed state), the `CountryCode` axis underneath
// `Present { concealed: false, countries: BTreeSet<CountryCode> }` is
// **open-vocabulary** — new trigraphs and tetragraphs land per ISMCAT
// schema updates without an FgiSet code change. There is no lawful
// finite "top" over the full `(concealed, countries)` Cartesian
// product, so the `SciSet` / `SarSet` / `AeaSet` open-vocab precedent
// applies. Use `FgiSet::empty()` / `FgiSet::default()` (== `Self::None`)
// for the bottom; callers that need the source-concealed supersession
// sentinel construct it explicitly via
// `FgiSet::from_marker(Some(&FgiMarker::SourceConcealed))`.

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
///    per §G.2 Table 5 p40 (ATOMAL registered as a standalone control
///    marking) + §H.7 p122 worked example (`SECRET//RD/ATOMAL//FGI
///    NATO//NOFORN` places ATOMAL in the AEA category position
///    alongside RD — confirming AEA-axis routing). Note §H.7 is the
///    FGI section, not an ATOMAL subsection; ATOMAL has no dedicated
///    subsection in §H.1 through §H.9, its registration lives in
///    §G.2 Table 5 and its AEA-axis routing is established by the
///    §H.7 p122 worked example, not by Table 5 itself. The PR 9c.1
///    T134 routing decision tracked this through the parser layer.
///
///    **CV-2 (PR 4b-B 8th-pass follow-up).** Pre-CV-2 wording said
///    `§G.2 Table 5 p40 (ATOMAL registered as a standalone control
///    marking; ARH = AEA)`. Verified 2026-05-16 against
///    `crates/capco/docs/CAPCO-2016.md`: Table 5 places ATOMAL under
///    its own row (no group header in the markdown rendering between
///    the NATO classification rows and the BOHEMIA/BALK rows), with
///    the ARH column reading "Requires ATOMAL read-in" — it does NOT
///    say "ARH = AEA". The "AEA category position" routing claim
///    derives from the §H.7 p122 worked example placement, not from
///    Table 5. The "ARH = AEA" parenthetical was a Constitution VIII
///    misattribution; the corrected citation pair (§G.2 Table 5 p40
///    for registration + §H.7 p122 worked example for AEA-axis
///    placement) preserves the routing-decision rationale without
///    over-claiming what Table 5 says.
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
        Self::from_markings_iter(markings.iter())
    }

    /// Construct an `AeaSet` from an iterator over `AeaMarking` references.
    ///
    /// Prefer this over [`Self::from_markings`] when the caller already has an
    /// iterator, such as a flattened per-portion slice — avoids the intermediate
    /// `Vec<AeaMarking>` allocation. CLONE-1 performance fix (issue #606).
    pub fn from_markings_iter<'a>(markings: impl Iterator<Item = &'a AeaMarking>) -> Self {
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
        // LA-2 empty-axis fast-path: skip SmallVec / sigmas-box
        // construction when no AEA markings were accumulated (the
        // common case on documents with no RD/FRD/TFNI/UCNI/ATOMAL
        // portions).
        if self.is_empty() {
            return Box::default();
        }
        // Inline-5 covers all AEA variants (Rd/Frd, DodUcni, DoeUcni,
        // Tfni, Atomal); the output stays heap-free for typical
        // documents (LA-4).
        let mut out: SmallVec<[AeaMarking; 5]> = SmallVec::with_capacity(5);
        // Sort SIGMA numbers ascending for §H.6 p108 canonical form.
        // `BTreeSet` already iterates in sorted order. Inline-8 covers
        // the observed SIGMA range (1–99; in practice 1–5); (LA-4).
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

impl JoinSemilattice for AeaSet {
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
}

impl MeetSemilattice for AeaSet {
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
/// `Unclassified < Restricted < Confidential < Secret < TopSecret`
/// per CAPCO-2016 §H.1 pp47-54 (US-domestic levels) and §H.2 p55 /
/// `NatoClassification::us_equivalent()` (NATO `NR` maps to
/// `Restricted` in the foreign-interop tier between U and C). M-7
/// (PR 4b-B follow-up): the chain is five elements, not four —
/// `Restricted` survives as a foreign-interop tier for portions
/// that carry NATO `NR` or an FGI source whose foreign system has
/// a RESTRICTED level (`FgiClassification.level = Restricted`).
/// Foreign classifications normalize to the US chain at portion-
/// parse time via §H.7 pp123-125's reciprocal-classification rule
/// (`MarkingClassification::effective_level()`), so cross-branch
/// joins do not arise in the lattice — the lattice always sees a
/// US-chain level.
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
/// **Same-variant payload tiebreak** (C-7 PR 4b-B follow-up). At
/// same level + same variant, country-bearing payloads (`Fgi`,
/// `Joint`) are **unioned** rather than picking one operand by
/// pointer order — `Fgi(S, [GBR]).join(Fgi(S, [CAN])) =
/// Fgi(S, [CAN, GBR])`. Union is commutative and idempotent, which
/// is what makes the lattice law hold. The union semantic also
/// matches the §H.7 p123 / §D.2 p28 banner-rollup rule that the
/// banner FGI list is the union of every observed foreign source.
/// `Conflict` payloads (`foreign: Box<ForeignClassification>`)
/// recurse into the same union rule when both sides carry the same
/// foreign variant; cross-variant payloads fall back to a
/// foreign-variant rank (Fgi < Nato < Joint).
///
/// `BoundedLattice` is implemented: top = `Some(Us(TopSecret))`,
/// bottom = `None`. The class chain is closed at five elements
/// (`Unclassified < Restricted < Confidential < Secret < TopSecret`,
/// M-7 PR 4b-B follow-up); no agency-extensibility concern.
///
/// §-authority (verified 2026-05-16 against CAPCO-2016.md):
/// - §H.1 pp47-54 (US class chain).
/// - §H.2 p55 (Non-US Protective Markings — refers to NATO chain
///   and to Manual Appendix A for FVEY equivalence).
/// - §H.7 pp123-125 (FGI grammar — supports the reciprocal-
///   classification convention applied at portion-parse time).
///
/// Manual Appendix A "Non-US Protective Markings (includes the
/// Five Eyes Marking Comparisons)" is referenced from §A.4 Table 1
/// p14 and §H.2 p55. It is the equivalence table that grounds the
/// `us_equivalent()` mapping from NATO levels to US levels, but
/// Appendix A is not vendored in `crates/capco/docs/CAPCO-2016.md`
/// (the markdown extract covers the lettered sections of the
/// Manual body — A through K — only, not the Appendices); the
/// appendix is an out-of-tree cross-reference, parallel to ISOO
/// section 3.3 in the `DeclassifyOnLattice` doc-comment.
///
/// **CV-3 (PR 4b-B 8th-pass follow-up).** Pre-CV-3 wording listed
/// `§A.4 p13 (IC Markings System Structure — classification hierarchy)`.
/// Verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`:
/// §A.4 p13 is a one-paragraph framing of "IC Markings System
/// Structure"; the §A.4 Table 1 IC Markings System Artifacts (which
/// names Appendix A as the FVEY equivalence reference) lands on
/// p14, not p13. Neither sub-page enumerates the classification
/// hierarchy itself. The §H.1 + §H.2 + Manual Appendix A citations
/// above carry the hierarchy + reciprocal-mapping authority that
/// the lattice actually relies on; §A.4 p13 was decorative.
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

    /// Construct from an iterator of pre-computed `Option<MarkingClassification>`
    /// values — joins them by `OrdMax` over `effective_level()`.
    ///
    /// Prefer this over [`Self::from_attrs_iter`] when the caller has already
    /// mapped or transformed each portion's classification, because it avoids
    /// the need to clone a full `CanonicalAttrs` slice just to modify the
    /// `classification` field. CLONE-1 performance fix (issue #606): eliminates
    /// the `filtered: Vec<CanonicalAttrs>` allocation in `join_via_lattice_body`.
    pub fn from_classification_iter(
        iter: impl Iterator<Item = Option<MarkingClassification>>,
    ) -> Self {
        iter.map(Self).fold(Self::empty(), |acc, p| acc.join(&p))
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

/// Same-variant / same-level payload tiebreaker for
/// `ClassificationLattice::join` (UNION semantic).
///
/// C-7 (PR 4b-B follow-up): the variant-rank tiebreaker alone is not
/// sufficient — two `Fgi` (or two `Joint`) values at the same level
/// with different country payloads previously fell through `ra <= rb`
/// returning the left operand, which broke commutativity. This helper
/// produces a join-result whose country payload is the **union** of
/// both operands' country lists, matching the §H.7 p123 banner-rollup
/// rule that the banner FGI list unions every observed foreign source
/// ("the one or more unique country trigraph(s) and/or tetragraph(s)
/// used in the portions"). Union is commutative and idempotent, so
/// commutativity + idempotence + associativity all hold without
/// further branching.
///
/// `Us`, `Nato`, and `Conflict` have no list payload at this level
/// (Nato carries only a tag; Us carries only the level; Conflict's
/// `foreign` is `Box<ForeignClassification>` which would need a
/// dedicated tiebreaker — for now we union the `foreign` payload
/// via the same rule when both sides are the same `ForeignClassification`
/// shape, else fall back to picking the canonically-smaller operand
/// by `effective_level()` + variant-rank tiebreak applied to
/// `foreign`'s inner variant).
///
/// **Companion**: see [`classification_meet_same_variant`] for the
/// dual-side semantic (INTERSECTION; bottom on disjoint payloads).
/// C-9 (PR 4b-B follow-up) split the two operations because using
/// union for both broke the absorption laws — `a ⊔ (a ⊓ b) = a` and
/// `a ⊓ (a ⊔ b) = a` cannot hold if `meet` and `join` are the same
/// op.
fn classification_join_same_variant(
    a: &MarkingClassification,
    b: &MarkingClassification,
) -> MarkingClassification {
    use std::collections::BTreeSet;
    // Idempotency short-circuit: if a == b, return a unchanged so
    // input order is preserved through the round-trip (avoids the
    // BTreeSet canonical-ordering side effect when a caller is
    // joining a payload with itself).
    if a == b {
        return a.clone();
    }
    match (a, b) {
        (MarkingClassification::Us(_), MarkingClassification::Us(_)) => a.clone(),
        (MarkingClassification::Fgi(fa), MarkingClassification::Fgi(fb)) => {
            // P-1 (8th-pass): source-concealed-dominates — if either side
            // has an empty countries list (the `//FGI [level]` form per
            // CAPCO-2016 §H.7 p124), the joined result MUST also be
            // source-concealed (empty countries). Chaining two lists when
            // one is empty returns the non-empty side and silently loses
            // the concealed signal — the banner incorrectly becomes
            // acknowledged `FGI [LIST]` instead of bare `FGI`.
            //
            // §-authority: §H.7 p124 (precedence rules for banner line
            // guidance: "if any of the portions have concealed FGI source
            // information... only the 'FGI' marking without the source
            // trigraph(s)/tetragraph(s) must appear in the banner line").
            // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
            let countries = if fa.countries.is_empty() || fb.countries.is_empty() {
                // Concealed dominates: produce the source-concealed form.
                Box::new([]) as Box<[marque_ism::CountryCode]>
            } else {
                let merged: BTreeSet<marque_ism::CountryCode> = fa
                    .countries
                    .iter()
                    .copied()
                    .chain(fb.countries.iter().copied())
                    .collect();
                merged.into_iter().collect::<Vec<_>>().into_boxed_slice()
            };
            MarkingClassification::Fgi(marque_ism::FgiClassification {
                level: fa.level, // same level — invariant of the tiebreaker
                countries,
            })
        }
        (MarkingClassification::Nato(_), MarkingClassification::Nato(_)) => a.clone(),
        (MarkingClassification::Joint(ja), MarkingClassification::Joint(jb)) => {
            let merged: BTreeSet<marque_ism::CountryCode> = ja
                .countries
                .iter()
                .copied()
                .chain(jb.countries.iter().copied())
                .collect();
            MarkingClassification::Joint(marque_ism::JointClassification {
                level: ja.level,
                countries: merged.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            })
        }
        (
            MarkingClassification::Conflict {
                us: ua,
                foreign: fa,
            },
            MarkingClassification::Conflict {
                us: ub,
                foreign: fb,
            },
        ) => {
            // us level matches by invariant (effective_level equality).
            // foreign payloads may differ; union the country-bearing
            // shapes when both sides carry the same ForeignClassification
            // variant; otherwise the variant-rank precedence on the
            // foreign payload picks the canonically-smaller side.
            let _ = (ua, ub);
            let foreign = merge_foreign_classification(fa, fb);
            MarkingClassification::Conflict {
                us: *ua,
                foreign: Box::new(foreign),
            }
        }
        // Different variants reach here only through a programming
        // error in `join`; defensively return `a`.
        _ => a.clone(),
    }
}

/// Merge two `ForeignClassification` payloads from same-level
/// `Conflict` variants. Same-variant union; cross-variant falls
/// back to the variant-rank precedence (lower rank wins).
fn merge_foreign_classification(
    a: &marque_ism::ForeignClassification,
    b: &marque_ism::ForeignClassification,
) -> marque_ism::ForeignClassification {
    use marque_ism::ForeignClassification;
    use std::collections::BTreeSet;
    match (a, b) {
        (ForeignClassification::Fgi(fa), ForeignClassification::Fgi(fb)) => {
            // P-1 (8th-pass): source-concealed-dominates — same fix as
            // `classification_join_same_variant`. Empty countries = the
            // source-concealed `//FGI [level]` form (§H.7 p124). If either
            // side is concealed, the joined result must be concealed.
            //
            // §-authority: §H.7 p124 (precedence rules for banner line
            // guidance: concealed dominates acknowledged in any mixed page).
            // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
            let countries = if fa.countries.is_empty() || fb.countries.is_empty() {
                Box::new([]) as Box<[marque_ism::CountryCode]>
            } else {
                let merged: BTreeSet<marque_ism::CountryCode> = fa
                    .countries
                    .iter()
                    .copied()
                    .chain(fb.countries.iter().copied())
                    .collect();
                merged.into_iter().collect::<Vec<_>>().into_boxed_slice()
            };
            ForeignClassification::Fgi(marque_ism::FgiClassification {
                level: fa.level,
                countries,
            })
        }
        (ForeignClassification::Nato(_), ForeignClassification::Nato(_)) => a.clone(),
        (ForeignClassification::Joint(ja), ForeignClassification::Joint(jb)) => {
            let merged: BTreeSet<marque_ism::CountryCode> = ja
                .countries
                .iter()
                .copied()
                .chain(jb.countries.iter().copied())
                .collect();
            ForeignClassification::Joint(marque_ism::JointClassification {
                level: ja.level,
                countries: merged.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            })
        }
        _ => {
            // Cross-variant: pick the canonically-smaller variant
            // (Fgi < Nato < Joint, mirroring `classification_variant_rank`
            // for the top-level shapes).
            let rank = |fc: &ForeignClassification| -> u8 {
                match fc {
                    ForeignClassification::Fgi(_) => 1,
                    ForeignClassification::Nato(_) => 2,
                    ForeignClassification::Joint(_) => 3,
                }
            };
            if rank(a) <= rank(b) {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}

/// Same-variant / same-level payload tiebreaker for
/// `ClassificationLattice::meet` (INTERSECTION semantic).
///
/// C-9 (PR 4b-B follow-up): the dual of [`classification_join_same_variant`].
/// `meet` is GLB on the country-list partial order:
///
/// - Equal payloads → that value (idempotence).
/// - One payload ⊆ the other → the smaller payload (it IS the GLB).
/// - Disjoint payloads → `None` (no common lower bound; meet falls
///   to the lattice bottom).
///
/// Returning `None` on disjoint payloads is what keeps the absorption
/// laws `a ⊔ (a ⊓ b) = a` and `a ⊓ (a ⊔ b) = a` holding: joining
/// `a` with `None` gives `a`, and meeting `a` with anything `≥ a`
/// gives `a`. Using `union` (the join semantic) on the meet side
/// broke both absorption laws.
///
/// `Us` and `Nato` carry no country payload at same level → meet
/// returns the value directly. `Conflict` is the absorbing top: at
/// same level + same shape (both Conflict, same foreign), meet is
/// that value; otherwise meet is `None`.
fn classification_meet_same_variant(
    a: &MarkingClassification,
    b: &MarkingClassification,
) -> Option<MarkingClassification> {
    use std::collections::BTreeSet;
    // Idempotency short-circuit: if a == b, return a unchanged so
    // input order is preserved through the round-trip.
    if a == b {
        return Some(a.clone());
    }
    match (a, b) {
        (MarkingClassification::Us(_), MarkingClassification::Us(_)) => Some(a.clone()),
        (MarkingClassification::Fgi(fa), MarkingClassification::Fgi(fb)) => {
            // P-9-1 (9th-pass): source-concealed (empty countries) is TOP in the
            // FGI source-disclosure dimension.  Meet with top returns the other
            // operand; dual of the join's concealed-dominates rule (P-1, 8th-pass).
            // Authority: §H.7 p128 (concealed is most restrictive form).
            // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
            let a_concealed = fa.countries.is_empty();
            let b_concealed = fb.countries.is_empty();
            match (a_concealed, b_concealed) {
                (true, true) => {
                    // Both concealed → top ⊓ top = top.
                    Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                        level: fa.level,
                        countries: Box::new([]),
                    }))
                }
                (true, false) => {
                    // self is concealed (top) → return other.
                    Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                        level: fb.level,
                        countries: fb.countries.clone(),
                    }))
                }
                (false, true) => {
                    // other is concealed (top) → return self.
                    Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                        level: fa.level,
                        countries: fa.countries.clone(),
                    }))
                }
                (false, false) => {
                    let sa: BTreeSet<marque_ism::CountryCode> =
                        fa.countries.iter().copied().collect();
                    let sb: BTreeSet<marque_ism::CountryCode> =
                        fb.countries.iter().copied().collect();
                    let inter: BTreeSet<marque_ism::CountryCode> =
                        sa.intersection(&sb).copied().collect();
                    if inter.is_empty() {
                        None
                    } else {
                        Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                            level: fa.level,
                            countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                        }))
                    }
                }
            }
        }
        (MarkingClassification::Nato(_), MarkingClassification::Nato(_)) => Some(a.clone()),
        (MarkingClassification::Joint(ja), MarkingClassification::Joint(jb)) => {
            let sa: BTreeSet<marque_ism::CountryCode> = ja.countries.iter().copied().collect();
            let sb: BTreeSet<marque_ism::CountryCode> = jb.countries.iter().copied().collect();
            let inter: BTreeSet<marque_ism::CountryCode> = sa.intersection(&sb).copied().collect();
            if inter.is_empty() {
                None
            } else {
                Some(MarkingClassification::Joint(
                    marque_ism::JointClassification {
                        level: ja.level,
                        countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                    },
                ))
            }
        }
        (
            MarkingClassification::Conflict {
                us: ua,
                foreign: fa,
            },
            MarkingClassification::Conflict {
                us: ub,
                foreign: fb,
            },
        ) => {
            // us level matches by invariant. Conflict carries an
            // implicit US + a single foreign payload; meet is the
            // foreign-intersection lifted back into Conflict, or
            // None if the foreign payloads are incomparable.
            let _ = (ua, ub);
            meet_foreign_classification(fa, fb).map(|foreign| MarkingClassification::Conflict {
                us: *ua,
                foreign: Box::new(foreign),
            })
        }
        _ => None,
    }
}

/// Companion to [`merge_foreign_classification`] for the meet side.
/// Same-variant payloads intersect; cross-variant returns the
/// HIGHER-rank operand (the dominated, lower-≤ side; the GLB dual of
/// the join's "lower variant rank wins" tiebreak).
///
/// **C-9b (PR 4b-B 7th-pass follow-up).** Pre-fix, this function
/// returned `None` on cross-variant inputs while
/// `merge_foreign_classification` returned the lower-rank operand.
/// That asymmetry broke the dual absorption law `a ⊓ (a ⊔ b) = a` for
/// `Conflict` values whose inner foreign payloads had different
/// variants — the join would settle on the lower-rank inner, but the
/// meet would collapse the entire outer Conflict to bottom. C-9b
/// aligns the cross-variant meet with the join's tiebreak (return the
/// higher-rank operand, the GLB dual), mirroring how C-9 fixed the
/// same asymmetry at the outer `ClassificationLattice::meet` level.
///
/// §-authority: §H.7 pp123-125 reciprocal-normalization grounds the
/// variant-rank ordering (Fgi=1 < Nato=2 < Joint=3). Verified
/// 2026-05-15 against CAPCO-2016.md.
fn meet_foreign_classification(
    a: &marque_ism::ForeignClassification,
    b: &marque_ism::ForeignClassification,
) -> Option<marque_ism::ForeignClassification> {
    use marque_ism::ForeignClassification;
    use std::collections::BTreeSet;
    match (a, b) {
        (ForeignClassification::Fgi(fa), ForeignClassification::Fgi(fb)) => {
            // P-9-1 (9th-pass): source-concealed (empty countries) is TOP in
            // the FGI source-disclosure dimension — dual of the join's
            // concealed-dominates rule (P-1, 8th-pass). Meet(top, x) = x.
            // Authority: §H.7 p128 (concealed is most restrictive form).
            // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
            let a_concealed = fa.countries.is_empty();
            let b_concealed = fb.countries.is_empty();
            match (a_concealed, b_concealed) {
                (true, true) => Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                    level: fa.level,
                    countries: Box::new([]),
                })),
                (true, false) => Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                    level: fb.level,
                    countries: fb.countries.clone(),
                })),
                (false, true) => Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                    level: fa.level,
                    countries: fa.countries.clone(),
                })),
                (false, false) => {
                    let sa: BTreeSet<marque_ism::CountryCode> =
                        fa.countries.iter().copied().collect();
                    let sb: BTreeSet<marque_ism::CountryCode> =
                        fb.countries.iter().copied().collect();
                    let inter: BTreeSet<marque_ism::CountryCode> =
                        sa.intersection(&sb).copied().collect();
                    if inter.is_empty() {
                        None
                    } else {
                        Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                            level: fa.level,
                            countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                        }))
                    }
                }
            }
        }
        (ForeignClassification::Nato(_), ForeignClassification::Nato(_)) => Some(a.clone()),
        (ForeignClassification::Joint(ja), ForeignClassification::Joint(jb)) => {
            let sa: BTreeSet<marque_ism::CountryCode> = ja.countries.iter().copied().collect();
            let sb: BTreeSet<marque_ism::CountryCode> = jb.countries.iter().copied().collect();
            let inter: BTreeSet<marque_ism::CountryCode> = sa.intersection(&sb).copied().collect();
            if inter.is_empty() {
                None
            } else {
                Some(ForeignClassification::Joint(
                    marque_ism::JointClassification {
                        level: ja.level,
                        countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                    },
                ))
            }
        }
        // C-9b: cross-variant → return the HIGHER-rank operand (the
        // dominated, lower-≤ side; GLB dual of `merge_foreign_classification`'s
        // tiebreak). The rank function below MUST agree with the one
        // in `merge_foreign_classification` (Fgi=1 < Nato=2 < Joint=3).
        _ => {
            let rank = |fc: &ForeignClassification| -> u8 {
                match fc {
                    ForeignClassification::Fgi(_) => 1,
                    ForeignClassification::Nato(_) => 2,
                    ForeignClassification::Joint(_) => 3,
                }
            };
            // Dual of merge: merge returns the LOWER-rank operand
            // (the GREATER element under ≤); meet returns the
            // HIGHER-rank operand (the LESSER element under ≤).
            if rank(a) >= rank(b) {
                Some(a.clone())
            } else {
                Some(b.clone())
            }
        }
    }
}

impl JoinSemilattice for ClassificationLattice {
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
                    //
                    // C-7 (PR 4b-B follow-up): when both operands
                    // share the same variant AND the same level, the
                    // payloads may still differ — e.g.
                    // `Fgi(S, [GBR]).join(Fgi(S, [CAN]))`. The
                    // variant-rank tiebreak alone fell through
                    // `ra <= rb` returning the left operand, which
                    // broke commutativity on same-variant payload
                    // diffs. We union the country payloads per the
                    // §H.7 p123 / §D.2 p28 banner-rollup rule that
                    // the banner FGI list is the union of every
                    // observed foreign source.
                    let ra = classification_variant_rank(a);
                    let rb = classification_variant_rank(b);
                    if ra == rb {
                        Self(Some(classification_join_same_variant(a, b)))
                    } else if ra < rb {
                        Self(Some(a.clone()))
                    } else {
                        Self(Some(b.clone()))
                    }
                }
            }
        }
    }
}

impl MeetSemilattice for ClassificationLattice {
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
                    // Equal effective level: meet must be the GLB
                    // dual of `join`. The join policy is:
                    //   - lower variant-rank wins at same level
                    //     (Us < Fgi < Nato < Joint < Conflict),
                    //     so the lower-rank variant is the GREATER
                    //     element in the lattice ≤ order;
                    //   - same variant + same level, payloads union.
                    //
                    // GLB (meet) is therefore the dual:
                    //   - cross-variant: return the HIGHER variant-
                    //     rank operand (the dominated, lower-≤ side).
                    //     §H.7 pp123-125 reciprocal-normalization.
                    //   - same variant + same level, payloads
                    //     INTERSECT (country-list GLB). Empty
                    //     intersection drops to the lattice bottom.
                    //
                    // C-9 (PR 4b-B follow-up): pre-fix, meet mirrored
                    // join's tiebreaker (lower rank wins) AND used
                    // the UNION helper for same-variant payloads.
                    // Both branches broke the absorption laws
                    // `a ⊔ (a ⊓ b) = a` / `a ⊓ (a ⊔ b) = a`.
                    let ra = classification_variant_rank(a);
                    let rb = classification_variant_rank(b);
                    if ra == rb {
                        match classification_meet_same_variant(a, b) {
                            Some(m) => Self(Some(m)),
                            None => Self(None),
                        }
                    } else if ra < rb {
                        // a has lower rank → a is GREATER in ≤ →
                        // b is the meet (the dominated, lower-≤).
                        Self(Some(b.clone()))
                    } else {
                        // a has higher rank → a is LESSER in ≤ →
                        // a is the meet.
                        Self(Some(a.clone()))
                    }
                }
            }
        }
    }
}

impl BoundedJoinSemilattice for ClassificationLattice {
    fn bottom() -> Self {
        Self(None)
    }
}

impl BoundedMeetSemilattice for ClassificationLattice {
    fn top() -> Self {
        Self(Some(MarkingClassification::Us(Classification::TopSecret)))
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
/// **Note** (CV-1, PR 4b-B 8th-pass follow-up): pre-CV-1 this doc
/// comment cited §H.6 p104 ("RD precedence rule applies to declass
/// dates by extension"). §H.6 p104 is about RD/FRD/TFNI banner
/// roll-up — its actual relevant rule for declass dates is the
/// opposite ("Automatic declassification of documents containing RD
/// information is prohibited") which forbids a declass-date on RD
/// documents entirely. The pre-CV-1 citation was a Constitution VIII
/// stretch; §E.3 p32 is the proper authority for date aggregation.
///
/// **`BoundedLattice` deliberately not implemented.** Dates are
/// open-vocab — no finite "top" date is realizable. Per the
/// `AeaSet` / `SciSet` / `SarSet` precedent in this module, the
/// established pattern for "no BoundedLattice when range is open"
/// is "implement `Lattice`, provide `empty()` / `default()` for
/// the bottom, leave `top()` undefined." (M-25 PR 4b-B 7th-pass —
/// `FgiSet` was previously listed in this precedent; B-1 PR 4b-B
/// 8th-pass retired `FgiSet`'s `BoundedLattice` impl — `FgiSet`
/// does NOT implement `BoundedLattice`. Removed from precedent list
/// to avoid misattribution.)
///
/// §-authority (verified 2026-05-16 against CAPCO-2016.md):
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
/// The single-static-table convention (M-14 PR 4b-B follow-up) is
/// enforced by the crate-private `apply_overlays` API taking
/// `DISSEM_SUPERSESSION_TABLE` directly — the only call site is
/// inside `marque-capco`, code-review enforces no ad-hoc copies.
/// An earlier `debug_assert!` pointer-equality check (rust-reviewer
/// Gotcha 2) was removed in H-4 because it compared the table
/// pointer to itself (always true, false protection); the `&'static`
/// reference passed everywhere in this module is the actual
/// invariant.
///
/// §-authority (verified 2026-05-16 against CAPCO-2016.md):
/// - §D.2 Table 3 rows 1-2 (NOFORN dominates FD&R controls).
/// - §H.8 p145 (NOFORN: "Cannot be used with REL TO, RELIDO, EYES ONLY,
///   or DISPLAY ONLY").
/// - §H.8 p157 (EYES ONLY: NSA-only marking — E064 emits a fix to migrate
///   EYES ONLY → REL TO at engine fix-time, but the parser preserves
///   `DissemControl::Eyes` during lint runs. P-4 (8th-pass): corrected
///   prior docstring that falsely claimed "EYES retired... already migrated
///   to REL TO at parse time so not represented here" — the parser does NOT
///   migrate at parse time; `scheme.rs:190` and `scheme.rs:3677` confirm
///   `DissemControl::Eyes` survives parse and appears in `dissem_us` during
///   intermediate lattice composition. NOFORN must dominate EYES ONLY in
///   the supersession table for the lattice path to be correct per §H.8 p145.
///   E064 handles the EYES → REL TO migration as a separate rule at fix time.)
static DISSEM_SUPERSESSION_TABLE: &[(DissemControl, DissemControl)] = &[
    // NOFORN ⊐ REL TO / RELIDO / DISPLAY ONLY / EYES ONLY — §D.2 Table 3
    // rows 1-2 + §H.8 p145 ("Cannot be used with REL TO, RELIDO, EYES ONLY,
    // or DISPLAY ONLY").
    //
    // P-4 (8th-pass): added EYES ONLY. Pre-fix the table omitted it based on
    // a false assumption that the parser migrated EYES → REL TO at parse time.
    // The parser preserves DissemControl::Eyes (see scheme.rs:190); E064 is
    // the engine-time migration rule. During lint runs and intermediate lattice
    // composition, EYES can appear and must be stripped when NOFORN is present.
    (DissemControl::Nf, DissemControl::Rel),
    (DissemControl::Nf, DissemControl::Relido),
    (DissemControl::Nf, DissemControl::Displayonly),
    (DissemControl::Nf, DissemControl::Eyes),
];

/// Lattice form of the US-attributed IC dissem axis: a `BTreeSet` of
/// `DissemControl` tokens with three supersession overlays applied
/// at construction and re-applied on `join`.
///
/// **Overlay set** (applied at `from_attrs_iter` / `join` time):
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
///    Applied via `with_noforn_injected` at the cross-axis NOFORN
///    rendezvous in `CapcoMarking::join_via_lattice` (G-8 PR 4b-B
///    follow-up).
///
/// **Post-PR-4b-E divergence inventory** (matching parity-gate
/// fixtures in `crates/capco/tests/lattice_vs_scheme_parity.rs`).
/// The PR 4b-E `PageContext::expected_*` deletion retired the
/// PageContext side of the original parity comparison; the surviving
/// comparison is between the per-axis lattice path
/// (`project_via_lattice`) and the full scheme pipeline
/// (`project_via_scheme = scheme.project(Scope::Page, ...)`, which
/// runs the declarative PageRewrite catalog over the per-axis
/// composition). The four overlays this `DissemSet` doc-comment
/// previously inventoried as "PageContext-only" all land on the
/// scheme path now:
///
/// - **Overlay 4 (NOFORN dominates)** lives on the lattice path
///   itself via `DissemSet::with_noforn_injected`. Per §H.8 p145
///   plus §D.2 Table 3 rows 1-2 the overlay strips `Rel` / `Relido`
///   / `Displayonly` when `Nf` is present.
/// - **FOUO classification-gate eviction** lives on
///   `scheme.project(Scope::Page, ...)` via the
///   `capco/classification-evicts-fouo` (Pattern B) and
///   `capco/fouo-evicted-by-classified` (Pattern C) PageRewrites
///   declared on `CapcoScheme` (CAPCO-2016 §H.8 p134
///   classified-document sub-clause).
/// - **UCNI classification-gate strip** lives on
///   `scheme.project(Scope::Page, ...)` via the
///   `capco/{dod,doe}-ucni-evicted-by-classified` and
///   `capco/{dod,doe}-ucni-promotes-noforn-when-classified`
///   PageRewrites (CAPCO-2016 §H.6 p116 DOD UCNI / §H.6 p118 DOE
///   UCNI; the NOFORN-promotion clause fires before the strip so
///   the §H.6 NOFORN-promotion semantic on classified pages is
///   preserved).
/// - **Cross-axis NOFORN injection from `non_ic_dissem`** mirrors on
///   the lattice path via `DissemSet::with_noforn_injected` (G-8
///   PR 4b-B). `NonIcDissemSet::from_attrs_iter`'s `needs_nf` flag
///   drives the injection on classified SBU-NF / LES-NF pages
///   (§H.9 p178 SBU-NF / §H.9 p185 LES-NF), and the supersession
///   overlay then re-runs Overlay 4 to strip dominated controls.
///
/// **Ordering** at the lattice level is BTreeSet's natural order;
/// §H.8 prose ordering ("OC/NF" not "NF/OC") is the renderer's
/// concern, not the lattice's. The renderer
/// (`MarkingScheme::render_canonical`) lands in PR 5+ Stage 4.
///
/// **`BoundedLattice` deliberately not implemented.** The
/// `DissemControl` vocabulary contains ~25 tokens but the **active
/// finite set** depends on schema version and agency extensions; the
/// open-vocab precedent (SciSet / SarSet / AeaSet) is the
/// established pattern for "implement `Lattice` + `empty()`/`default()`
/// for bottom, leave `top()` undefined." (M-25 PR 4b-B 7th-pass —
/// `FgiSet` was previously listed in this precedent; B-1 PR 4b-B
/// 8th-pass retired `FgiSet`'s `BoundedLattice` impl — `FgiSet`
/// does NOT implement `BoundedLattice`. Removed from precedent list
/// to avoid misattribution.)
///
/// **Partial-lattice note (C-4 PR 4b-B follow-up).** The
/// `relido_observed_unanimous` flag is a **join-side aggregation
/// property** — it tracks whether every portion contributing to the
/// page's dissem state has RELIDO. `meet` has no natural reading for
/// this flag, so its result carries the vacuous-true value (the
/// identity under subsequent AND-joins). This is what makes the
/// load-bearing absorption law `a ⊔ (a ⊓ b) = a` hold algebraically.
/// The dual law `a ⊓ (a ⊔ b) = a` does NOT hold over the full
/// `(set, flag)` pair — `DissemSet` is a join-semilattice with a
/// structural `meet` provided for completeness on the `set` axis.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.8 p136 (ORCON dominates ORCON-USGOV).
/// - §H.8 p140 (ORCON-USGOV template same rule).
/// - §H.8 p145 (NOFORN dominates REL TO / RELIDO / DISPLAY ONLY).
/// - §H.8 pp155-156 (RELIDO unanimity for banner rollup).
/// - §D.2 Table 3 rows 1-2 (NOFORN dominates dominated FD&R).
///
/// **`Default`** (C-8 PR 4b-B follow-up). `Default` MUST agree with
/// `empty()` — both are the lattice bottom with the vacuous-truth
/// `relido_observed_unanimous = true` flag. A derived `Default` would
/// produce `relido_observed_unanimous = false` (bool's Default), which
/// would break `Default == empty()` and silently drop RELIDO when a
/// `Default::default()` value was joined into a unanimous-RELIDO set.
/// The manual `Default` impl delegates to `empty()` so the two
/// constructors agree.
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Default for DissemSet {
    /// `Default` MUST agree with `DissemSet::empty()` (C-8 PR 4b-B
    /// follow-up). See the struct doc comment for the rationale —
    /// the derived `Default` set `relido_observed_unanimous = false`
    /// (bool's Default) and broke C-5's `from_attrs_iter(&[]) ==
    /// empty()` agreement on a third constructor.
    fn default() -> Self {
        Self::empty()
    }
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
    ///
    /// Empty input returns `Self::empty()` (the lattice bottom)
    /// exactly — `from_attrs_iter(&[]) == DissemSet::empty()`.
    /// The vacuous-truth treatment of "every portion carries
    /// RELIDO over an empty portion list" matches the universal-
    /// quantifier convention and the `empty()` constructor's
    /// `relido_observed_unanimous = true`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        if portions.is_empty() {
            return Self::empty();
        }

        let mut set = BTreeSet::new();
        for p in portions {
            for t in p.dissem_us.iter() {
                set.insert(*t);
            }
        }

        // RELIDO observed-unanimity: track whether every portion
        // carries Relido. Vacuously true over an empty portion list
        // (universal quantifier convention); since we early-returned
        // on `portions.is_empty()`, this expression is now strictly
        // `every observed portion has RELIDO`.
        let relido_observed_unanimous = portions
            .iter()
            .all(|a| a.dissem_us.contains(&DissemControl::Relido));

        let mut out = Self {
            set,
            relido_observed_unanimous,
        };
        out.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        out
    }

    /// Internal: apply the three supersession overlays in order.
    /// The `table` parameter MUST be `DISSEM_SUPERSESSION_TABLE`
    /// in production (M-14 PR 4b-B follow-up — the `debug_assert!`
    /// pointer-equality "Gotcha 2" check from H-4 was removed
    /// because it compared the table to itself; the single-static-
    /// table convention is enforced by `apply_overlays` being
    /// crate-private with `DISSEM_SUPERSESSION_TABLE` as the only
    /// in-tree caller).
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

    /// Borrow as a `Vec` for callers that need the post-overlay set
    /// in `Vec`-shaped form (parity-gate fixtures and similar
    /// inspection sites; `into_boxed_slice` is the production
    /// renderer-facing API).
    pub fn to_vec(&self) -> Vec<DissemControl> {
        self.set.iter().copied().collect()
    }

    /// Inject `Nf` into the set and re-apply the supersession
    /// overlay. G-8 (PR 4b-B follow-up) — callers that need to
    /// inject NOFORN from a cross-axis source (non-IC SBU-NF /
    /// LES-NF on a classified page, NODIS / EXDIS supersession,
    /// or the `capco/noforn-clears-rel-to` PageRewrite) MUST route
    /// through here so the §H.8 p145 NOFORN-dominates rule strips
    /// `Rel` / `Relido` / `Displayonly` from the set.
    ///
    /// Pre-G-8 the cross-axis injection at the NOFORN rendezvous
    /// in the `join_via_lattice` body added `Nf` directly into
    /// `out.dissem_us` after `DissemSet::into_boxed_slice` ran,
    /// which left dominated controls in place — invalid per
    /// §H.8 p145.
    ///
    /// Authority: §H.8 p145 (NOFORN: "Cannot be used with REL TO /
    /// RELIDO / EYES ONLY / DISPLAY ONLY") + §D.2 Table 3 rows 1-2.
    pub fn with_noforn_injected(mut self) -> Self {
        self.set.insert(DissemControl::Nf);
        // Re-run the supersession overlay so the NOFORN-dominates
        // step strips any `Rel` / `Relido` / `Displayonly` left in
        // the bag.
        self.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        self
    }
}

// P-9-3 (9th-pass) — Partial-lattice divergence note for `DissemSet`.
//
// `DissemSet` implements only `JoinSemilattice`, NOT `MeetSemilattice`.
// The `relido_observed_unanimous` flag is a join-side aggregation property
// (a record of observed page composition); `meet` has no natural reading
// for this flag — the dual absorption law `a ⊓ (a ⊔ b) = a` cannot hold
// over the full `(set, relido_observed_unanimous)` pair. PR #456 resolved
// this by splitting the `Lattice` trait into `JoinSemilattice` and
// `MeetSemilattice` halves; `DissemSet` implements only the join half,
// so the type system now rejects any attempt to call `.meet()` on it at
// compile time.
//
// See the `DissemSet` doc comment above (§ "Partial-lattice note C-4")
// for full rationale.
impl JoinSemilattice for DissemSet {
    fn join(&self, other: &Self) -> Self {
        // The single-static-table convention is enforced by the
        // crate-private `apply_overlays` API taking
        // `DISSEM_SUPERSESSION_TABLE` directly (it has no other call
        // sites). H-4 PR 4b-B follow-up removed a tautological
        // `debug_assert!` that compared the table pointer to itself
        // — always true, false protection.
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
/// claim. The SciSet/SarSet/AeaSet precedent for open-vocab applies
/// (M-25 PR 4b-B 7th-pass — `FgiSet` was previously listed in this
/// precedent; B-1 PR 4b-B 8th-pass retired `FgiSet`'s
/// `BoundedLattice` impl — `FgiSet` does NOT implement
/// `BoundedLattice`. See DissemSet doc above for rationale.)
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

// ---------------------------------------------------------------------------
// JointSet — 4-variant state with producer-disunity collapse
// ---------------------------------------------------------------------------

/// Lattice form of the JOINT classification axis.
///
/// The state space is a closed four-variant enum that captures the
/// decision tree from CAPCO-2016 §H.3 + §H.7. The `Mixed` variant
/// (added in PR 4b-B follow-up C-3) distinguishes "no JOINT seen"
/// (the lattice identity `Bottom`) from "JOINT and non-JOINT both
/// observed" (an absorbing state) so `join` stays **associative**.
///
/// - `Bottom`: no JOINT-bearing portion observed. Lattice identity.
/// - `UnanimousProducers`: every observed portion is JOINT with the
///   same producer set. The banner is `//JOINT [class] [LIST]` per
///   §H.3 p56.
/// - `DisunityCollapse`: every observed portion is JOINT but the
///   producer lists differ. Non-US producers migrate to FGI per
///   §H.7 p123.
/// - `Mixed`: at least one JOINT portion AND at least one
///   non-JOINT portion observed. Absorbing for the JOINT axis —
///   §H.3 p57 "JOINT marking is not carried forward to the banner
///   line in US documents." Once `Mixed`, the JOINT axis cannot
///   resurrect to `UnanimousProducers` regardless of subsequent
///   joins.
///
/// The transitions on `Lattice::join` are structural operations on
/// the deterministic state space — NOT "normalization" in the
/// `Lattice` module-docs Gotcha-1 sense — and the property test
/// `joint_disunity_lattice_laws` exhausts the state-space cube to
/// verify assoc/comm/idem.
///
/// **The W004 Warn rule** (in `crates/capco/src/rules.rs`) reads
/// the post-projection JointSet state from the engine's
/// `PageContext` flow. W004 fires only on `DisunityCollapse`;
/// `Mixed` is the §H.3 p57 case where FGI migration rides through
/// `expected_fgi_marker` and no W004 fires. The lattice does not
/// itself emit the diagnostic; the rule does.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
///
/// - §H.3 p56 (JOINT classification grammar).
/// - §H.3 pp55-59 (JOINT worked examples).
/// - §H.3 p57 ("JOINT marking not carried forward to
///   the banner line in US documents").
/// - §H.7 p123 (FGI source-acknowledged form for disunity-collapse
///   non-US producer migration).
///
/// **`#[non_exhaustive]`** (B-4, PR 4b-B 8th-pass follow-up): the
/// four-variant decision tree is the lawful closed set per §H.3 p57
/// today, but future CAPCO revisions or partial-decoder states may
/// add a `PartialDisunity` / `Inferred` variant — declaring
/// `#[non_exhaustive]` requires downstream matchers to handle the
/// unknown case with a wildcard arm so a future variant addition
/// is a non-breaking change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum JointSet {
    /// No JOINT-bearing portion observed. Lattice identity for `join`.
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

    /// At least one JOINT portion AND at least one non-JOINT
    /// portion observed. §H.3 p57: JOINT does not roll up to the
    /// banner in US documents. Absorbing for the JOINT axis — once
    /// `Mixed`, subsequent joins cannot resurrect a JOINT roll-up
    /// state. Non-US producers ride to `FgiSet` via
    /// `expected_fgi_marker`; no W004 fires on `Mixed`.
    Mixed,
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
    /// 1. **No portions / no JOINT portion** → `Bottom` (identity).
    /// 2. **All portions JOINT** with identical producer lists →
    ///    `UnanimousProducers { OrdMax(level), countries }`.
    /// 3. **All portions JOINT** with disagreeing producer lists →
    ///    `DisunityCollapse { OrdMax(level), union_non_us }`.
    /// 4. **Mixed JOINT + non-JOINT** → `Mixed`. The §H.3 p57
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
        //
        // **Malformed JOINT portions are dropped at this point.** A
        // JOINT portion is malformed when it fails either of the two
        // §H.3 p56 grammar invariants:
        //
        // 1. Producer list must be non-empty (`!j.countries.is_empty()`).
        // 2. **USA must appear in the producer list** ("USA always
        //    appears as the OWNER/PRODUCER" per §H.3 p56). Pre-fix
        //    (PR 4b-B 9th-pass), only invariant #1 was enforced; a
        //    `JointClassification { countries: [GBR] }` (one country,
        //    no USA) was pushed to `joint_portions`, treated as
        //    well-formed unanimous, and emitted a JOINT banner
        //    without USA — unrepresentable in the §H.3 grammar.
        //
        // Per the existing empty-producer rationale: dropping
        // malformed portions at scan time keeps the remaining
        // (well-formed) portions in the correct shape to drive the
        // lattice state per the standard rules: zero remaining →
        // `Bottom`; well-formed unanimous → `UnanimousProducers`;
        // well-formed disagreement → `DisunityCollapse`.
        //
        // The malformed portion is **invisible to the JOINT axis**
        // (does not count as "non-JOINT" either). The classification
        // axis still consumes the malformed portion's
        // `effective_level()` for the level-chain max via
        // `ClassificationLattice`; this normalization is
        // JOINT-axis-only.
        //
        // Authority: §H.3 p56 (JOINT grammar requires non-empty
        // `[LIST]` AND USA in the producer list). Verified
        // 2026-05-16 against CAPCO-2016.md.
        let has_usa = |j: &JointClassification| j.countries.iter().any(|c| c.as_str() == "USA");
        // Inline-4 covers the typical JOINT portion count per page;
        // deeply collaborative documents with 5+ JOINT portions spill
        // to heap cleanly (LA-4).
        let mut joint_portions: SmallVec<[&JointClassification; 4]> =
            SmallVec::with_capacity(portions.len().min(4));
        let mut has_non_joint = false;
        for p in portions {
            match &p.classification {
                // Well-formed: non-empty AND contains USA.
                Some(MarkingClassification::Joint(j)) if !j.countries.is_empty() && has_usa(j) => {
                    joint_portions.push(j)
                }
                // Malformed JOINT (empty producer list OR no USA):
                // drop, treat as invisible to the JOINT axis. The
                // portion is still a CanonicalAttrs entry on the
                // page, so it doesn't count as "non-JOINT" either —
                // the malformed shape contributes nothing.
                Some(MarkingClassification::Joint(_)) => {}
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
        // `Mixed` (absorbing) and no W004 fires.
        if has_non_joint {
            return Self::Mixed;
        }

        // All (well-formed) portions JOINT: check unanimity on
        // producer lists.
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
            // Note: `first_producers` is guaranteed non-empty here
            // because empty-producer portions were dropped above.
            // The defensive `is_empty()` check at this site is
            // therefore redundant post-fix; we keep an assertion-
            // shaped early return for belt-and-braces (any future
            // refactor that re-introduces empty-producer portions
            // before this point will fail loud rather than producing
            // a malformed `UnanimousProducers { producers: ∅ }`).
            if first_producers.is_empty() {
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
    /// portions; `None` for `Bottom` and `Mixed` (the latter does
    /// not carry a per-axis level since JOINT doesn't roll up).
    pub fn highest_level(&self) -> Option<Classification> {
        match self {
            Self::Bottom | Self::Mixed => None,
            Self::UnanimousProducers { level, .. } => Some(*level),
            Self::DisunityCollapse { highest_level, .. } => Some(*highest_level),
        }
    }

    /// Whether the page is in the `Mixed` state — JOINT and non-JOINT
    /// portions both observed. JOINT does not roll up to the banner
    /// in this case (§H.3 p57).
    pub fn is_mixed(&self) -> bool {
        matches!(self, Self::Mixed)
    }

    /// Convert back to a `MarkingClassification` for the banner.
    ///
    /// - `Bottom` → `None` (no JOINT portion observed; the banner
    ///   reads the class from `ClassificationLattice` and FGI from
    ///   `FgiSet` per the existing PageContext flow).
    /// - `Mixed` → `None` (§H.3 p57: JOINT does not roll up in US
    ///   documents; the banner reads the class from `Us(_)` and FGI
    ///   from the cross-axis fold).
    /// - `UnanimousProducers { level, producers }` → `Some(Joint(...))`.
    /// - `DisunityCollapse { highest_level, .. }` → `Some(Us(highest_level))`
    ///   (the non-US producers ride to FgiSet via a separate flow —
    ///   see `Commit 7 CapcoMarking::join` rewrite).
    pub fn to_marking_classification(&self) -> Option<MarkingClassification> {
        match self {
            Self::Bottom | Self::Mixed => None,
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

// P-9-3 (9th-pass) — Partial-lattice divergence note for `JointSet`.
//
// `JointSet` implements only `JoinSemilattice`, NOT `MeetSemilattice`.
// The `Mixed` / `DisunityCollapse` distinction is a record of observed
// page composition (join-side aggregation), not an algebraic element;
// `meet` has no natural reading for non-identical producer sets — the
// dual absorption law `a ⊓ (a ⊔ b) = a` cannot hold over the full state
// space. Independently, the pre-split `meet` was non-idempotent on
// `DisunityCollapse` self-pairs (`a ⊓ a = Bottom ≠ a`) because the
// fallback arm collapsed every non-identical-payload pair to `Bottom`
// — the partial behavior was stronger than dual-absorption failure alone.
// PR #456 resolved this by splitting the `Lattice` trait into
// `JoinSemilattice` and `MeetSemilattice` halves; `JointSet` implements
// only the join half, so the type system now rejects any attempt to call
// `.meet()` on it at compile time.
impl JoinSemilattice for JointSet {
    ///   with union of non-US producers and max level.
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            // Mixed is absorbing for non-Bottom operands. §H.3 p57.
            // We deliberately let Bottom ⊔ Mixed = Mixed propagate
            // (Bottom is the identity, Mixed is the new state).
            (Self::Mixed, _) | (_, Self::Mixed) => Self::Mixed,
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
///   stronger than `Empty`. Note: both `NofornSuperseded` AND `Empty`
///   trigger NF injection at the scheme layer
///   (`CapcoMarking::join_via_lattice`) — `NofornSuperseded` via
///   NODIS/EXDIS supersession (§H.9 p172/p174) and `Empty` via
///   §D.2 Table 3 row 9 (no-common-LIST → NOFORN). See
///   [`RelToBlock::is_noforn_superseded`] and
///   [`RelToBlock::is_empty_intersection`].
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
/// open-extensible. The SciSet/SarSet/FgiSet/AeaSet precedent applies
/// (`FgiSet` retired its `BoundedLattice` impl in B-1, PR 4b-B 8th-pass —
/// see the §6 "Note on `BoundedLattice`" block in `FgiSet` for the
/// open-vocab rationale; both FgiSet and RelToBlock share the same
/// `CountryCode` open-vocab axis).
///
/// **`#[non_exhaustive]`** (B-4, PR 4b-B 8th-pass follow-up): the
/// four-variant state space is closed today, but future CAPCO
/// extensions (e.g., a `PartialIntersection` variant for partial-
/// decoder REL TO recovery) may add states without breaking the
/// closed-set contract for the existing four — declaring
/// `#[non_exhaustive]` requires downstream matchers to handle the
/// unknown case with a wildcard arm.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.8 pp150-151 (REL TO grammar — banner form `AUTHORIZED FOR
///   RELEASE TO [USA, LIST]`).
/// - §D.2 Table 3 rows 9-13 (REL TO supersession by NOFORN and the
///   disjoint-LIST → NOFORN rule).
/// - §H.8 p152 worked example (intersection on roll-up).
/// - §H.9 p172 + p174 (NODIS / EXDIS clear REL TO).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
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

        // Gather only portions with a non-empty REL TO list. Inline-8
        // covers the typical per-page REL-TO portion count; pages with
        // 9+ REL TO portions spill to heap cleanly (LA-4).
        let rel_to_portions: SmallVec<[&CanonicalAttrs; 8]> =
            portions.iter().filter(|a| !a.rel_to.is_empty()).collect();

        if rel_to_portions.is_empty() {
            return Self::Bottom;
        }

        // Expand each portion's REL TO into a set of trigraph
        // strings, resolving tetragraphs to constituents. Inline-8
        // mirrors `rel_to_portions` capacity (LA-4).
        let expanded: SmallVec<[BTreeSet<&str>; 8]> = rel_to_portions
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

    /// Whether the block is the `NofornSuperseded` sentinel.
    ///
    /// NF injection at the scheme layer (`CapcoMarking::join_via_lattice`)
    /// is triggered by EITHER `NofornSuperseded` (NODIS/EXDIS supersession
    /// per §H.9 p172/p174) OR `Empty` (REL TO intersection has no common
    /// LIST per §D.2 Table 3 row 9). See `CapcoMarking::join_via_lattice`
    /// for the injection rendezvous. This accessor is a convenience check
    /// for the `NofornSuperseded` arm only; callers that need both arms
    /// should also call [`Self::is_empty_intersection`].
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

impl JoinSemilattice for RelToBlock {
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
}

impl MeetSemilattice for RelToBlock {
    fn meet(&self, other: &Self) -> Self {
        // Meet over REL TO — union of country lists, semantically
        // "the broader release that BOTH sides could have authored."
        //
        // `NofornSuperseded` is the **join-top** of `RelToBlock`:
        // every state joins to `NofornSuperseded` (the absorbing element
        // on the join side, modeling "any NOFORN-injecting supersession
        // on the page forces banner NOFORN per §H.8 p145 + §D.2 Table 3
        // row 9"). Symmetrically, `meet(NofornSuperseded, x) = x` —
        // `NofornSuperseded` as join-top means the GLB with any state x
        // is x itself. The prior arm `(N, _) | (_, N) => N` treated N
        // as meet-bottom, which violated dual absorption: for any
        // `a ≠ N`, `a ⊓ (a ⊔ N) = a ⊓ N` should equal `a` but
        // returned `N` instead (11th-pass lattice-consultant HIGH defect,
        // fixed here; isomorphic to C-9 on `ClassificationLattice`).
        //
        // `Bottom` is the meet-absorbing element (bottom of the meet
        // semilattice). `Empty` (intersected-to-empty REL TO) meets
        // like a normal element — joining to a real LIST under union
        // there is nothing to forbid.
        match (self, other) {
            (Self::NofornSuperseded, x) | (x, Self::NofornSuperseded) => x.clone(),
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
// Open-vocab helpers and lattice helpers added in PR 4b-E for the residue-axis
// migration off `PageContext::expected_*`.
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
// FgiSet::from_attrs_iter — unions per-portion FgiMarker with
// classification-derived producers (NATO / JOINT / FGI variants).
// ---------------------------------------------------------------------------

impl FgiSet {
    /// Construct an `FgiSet` from a slice of `CanonicalAttrs` —
    /// unions per-portion `fgi_marker` with the producers implied by
    /// the per-portion classification axis:
    ///
    /// - `MarkingClassification::Fgi(_)` contributes its trigraph list
    ///   (or `SourceConcealed` if the list is empty).
    /// - `MarkingClassification::Nato(_)` contributes the `NATO` code.
    /// - `MarkingClassification::Joint(_)` contributes the non-US
    ///   producers from its country list.
    /// - Other classification variants contribute nothing.
    /// - An explicit `FgiMarker::SourceConcealed` on any portion makes
    ///   the result source-concealed (`Present { concealed: true, .. }`)
    ///   regardless of other contributions — concealed is the dominating
    ///   element per §H.7 p128.
    ///
    /// §-authority (verified 2026-05-18 against
    /// `crates/capco/docs/CAPCO-2016.md`):
    /// - §H.7 p122 (FGI source-concealed grammar).
    /// - §H.7 p123 (FGI acknowledged + classification-derived producers).
    /// - §H.7 p128 (concealed-dominates-acknowledged when mixed).
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        let mut has_any_fgi = false;
        let mut has_source_concealed = false;
        let mut countries: BTreeSet<CountryCode> = BTreeSet::new();

        for attrs in portions {
            // Explicit FGI marker on the portion.
            if let Some(marker) = &attrs.fgi_marker {
                has_any_fgi = true;
                match marker {
                    FgiMarker::SourceConcealed => {
                        has_source_concealed = true;
                    }
                    FgiMarker::Acknowledged {
                        countries: marker_countries,
                        ..
                    } => {
                        countries.extend(marker_countries.iter().copied());
                    }
                }
            }

            // Classification-derived producers (NATO / JOINT / FGI variants).
            match &attrs.classification {
                Some(MarkingClassification::Fgi(fgi)) => {
                    has_any_fgi = true;
                    if fgi.countries.is_empty() {
                        has_source_concealed = true;
                    } else {
                        countries.extend(fgi.countries.iter().copied());
                    }
                }
                Some(MarkingClassification::Nato(_)) => {
                    has_any_fgi = true;
                    if let Some(nato) = CountryCode::try_new(b"NATO") {
                        countries.insert(nato);
                    }
                }
                Some(MarkingClassification::Joint(j)) => {
                    has_any_fgi = true;
                    let usa = CountryCode::try_new(b"USA");
                    for c in j.countries.iter() {
                        if Some(*c) != usa {
                            countries.insert(*c);
                        }
                    }
                }
                _ => {}
            }
        }

        if !has_any_fgi {
            return Self::None;
        }

        // §H.7 p128: source-concealed dominates open sources.
        if has_source_concealed {
            return Self::Present {
                concealed: true,
                countries: BTreeSet::new(),
            };
        }

        if countries.is_empty() {
            // Defensive: an explicit `Acknowledged{}` marker with an
            // empty country list (which the type-system should
            // currently prevent — `acknowledged()` returns `None`)
            // collapses to `None` rather than fabricating an
            // acknowledged-but-empty `Present`.
            Self::None
        } else {
            Self::Present {
                concealed: false,
                countries,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NonIcDissemSet — lattice over the non-IC dissem axis with
// classification-gated SBU-NF / LES-NF split + NODIS / EXDIS NF-injection.
// ---------------------------------------------------------------------------

/// Lattice form of the non-IC dissem axis.
///
/// Carries the union of per-portion `non_ic_dissem` tokens after
/// the classification-independent compound-supersedes-bare overlay
/// (§H.9 p178 / p185 — see #552 below), the classification-gated
/// SBU-NF / LES-NF transformations (§H.9 p178 / p185 — see #541),
/// and the NODIS / EXDIS NF-injection flag (§H.9 p172 / p174).
///
/// # Same-axis compound supersession (#552)
///
/// Before any classification gate fires, two co-presence rules apply
/// regardless of classification level:
///
/// - `{Sbu, SbuNf} ⊆ set` → drop `Sbu`. **§H.9 p178** (SBU NOFORN
///   Precedence Rules for Banner Line Guidance): *"When a document
///   contains both SBU-NF and SBU portions, SBU NOFORN supersedes
///   SBU in the banner line."*
/// - `{Les, LesNf} ⊆ set` → drop `Les`. **§H.9 p185** + canonical
///   banner-form examples: portion `(U//LES-NF)` rolls up to banner
///   `UNCLASSIFIED//LES NOFORN`; the LES-NF compound carries the LES
///   family marker in unclassified banner form, so bare LES is
///   redundant when LES-NF is also present.
///
/// The supersession runs BEFORE the classified gate so the
/// post-supersession set is what the gate sees. Net result for the
/// four U/S × SBU/LES quadrants:
///
/// | Input | Unclassified | Classified |
/// |---|---|---|
/// | `{Sbu, SbuNf}` | `{SbuNf}` | `{}` + `needs_nf` |
/// | `{Les, LesNf}` | `{LesNf}` | `{Les}` + `needs_nf` |
///
/// `needs_nf` is set when:
/// - SBU-NF appears on a classified page (§H.9 p178 — SBU vanishes
///   from the set; only `needs_nf` is asserted — see asymmetry note
///   below), OR
/// - LES-NF appears on a classified page (§H.9 p185 — `Les` is
///   inserted into the set AND `needs_nf` is asserted), OR
/// - Any portion carries NODIS or EXDIS (classification-independent
///   per §H.9 p172 / p174 — the manual does not gate the NF injection
///   on classification level for these tokens).
///
/// # SBU-NF / LES-NF classified-context asymmetry (the §H.9 p178 vs
/// §H.9 p185 difference)
///
/// On classified pages the two compound-NF non-IC dissem tokens
/// behave OPPOSITELY:
///
/// - **SBU-NF**: the bare SBU vanishes entirely from the output set;
///   only NOFORN is injected via `needs_nf`. **§H.9 p178** (SBU NOFORN
///   Commingling Rule(s) Within a Portion): *"If the portion is
///   classified, the classification level of the portion adequately
///   protects the SBU information, so SBU is not reflected in the
///   portion mark; however a NOFORN marking must be added to the
///   portion mark, e.g., (C//NF)."* The classification level subsumes
///   SBU's role as administrative-protection marker.
///
/// - **LES-NF**: the bare LES is RETAINED in the output set; NOFORN
///   is injected via `needs_nf` in parallel. **§H.9 p185** (LES NOFORN
///   Precedence Rules for Banner Line Guidance): *"The LES marking
///   always appears in the banner line if LES information (either LES
///   or LES NOFORN) is contained in the document, regardless of the
///   document's classification level. When a classified document
///   contains portions of U//LES-NF, the 'LES' marking is used in the
///   banner line and the NOFORN marking is applied as a Dissemination
///   Control Marking. For example: SECRET//NOFORN//LES."* LES carries
///   independent regulatory discipline (law-enforcement legal-process
///   restrictions per §H.9 p182 LES Warning Statement, originator-
///   control discipline per §H.9 p186 Notes — and the
///   `SECRET//NOFORN//LES` worked example at §H.9 p184 Notional Example
///   Page 4) that classification does NOT subsume — hence the
///   asymmetry with SBU.
///
/// **`Default`** is the bottom: empty set, `needs_nf = false`.
///
/// **Projection helper, NOT a `JoinSemilattice`.** Earlier review
/// passes flagged the missing trait impl (rust-reviewer H-3); the
/// lattice-consultant verdict was that the missing impl is the
/// architecturally correct shape, not a gap. The classified-context
/// SBU-NF / LES-NF transformations are gated on the page-level
/// `is_classified` predicate, which depends on the OUTER
/// classification axis being known. A pure per-axis `join` cannot
/// read the classification axis; implementing the trait would
/// silently produce wrong output on any cross-axis composition path.
/// Production consumers use [`Self::from_attrs_iter`] directly. See
/// [`DeclassExemptionAccumulator`] (which retired its
/// `JoinSemilattice` impl in PR 4b-E review for the dual reason: a
/// commutativity violation) for the same precedent. The structural
/// template is **"don't claim a trait when the laws can't hold."**
///
/// §-authority (verified 2026-05-18 against
/// `crates/capco/docs/CAPCO-2016.md`):
/// - §H.9 p172 (EXDIS — REL TO not authorized in banner; NOFORN
///   conveys).
/// - §H.9 p174 (NODIS — same).
/// - §H.9 p178 (SBU-NF — SBU vanishes on classified; NOFORN added).
/// - §H.9 p185 (LES-NF — LES retained on classified; NOFORN added).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NonIcDissemSet {
    set: BTreeSet<NonIcDissem>,
    needs_nf: bool,
}

impl NonIcDissemSet {
    /// An empty non-IC dissem set — the lattice bottom.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct from a slice of `CanonicalAttrs`. Applies the
    /// classification-gated SBU-NF / LES-NF split (the page is
    /// considered classified if any portion's classification is above
    /// `Unclassified`) and the unconditional NODIS / EXDIS
    /// NF-injection flag.
    ///
    /// Mirrors `PageContext::expected_non_ic_dissem`'s shape exactly,
    /// returning `(set, needs_nf)` via `into_inner_with_needs_nf`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        // Classification gate: any portion above Unclassified makes
        // the page "classified" for the SBU-NF / LES-NF split.
        let classified = portions.iter().any(|a| {
            a.classification
                .as_ref()
                .is_some_and(|c| c.effective_level() > Classification::Unclassified)
        });

        let mut set: BTreeSet<NonIcDissem> = BTreeSet::new();
        for attrs in portions {
            for d in attrs.non_ic_dissem.iter() {
                set.insert(*d);
            }
        }

        // #552 — Classification-independent same-axis supersession:
        // compound NOFORN-bearing token dominates its bare sibling.
        // Applied BEFORE the classification-gated #541 transformations
        // so the post-supersession set is what feeds the classified
        // strip/split below.
        //
        // §H.9 p178 (SBU NOFORN Precedence Rules for Banner Line
        // Guidance): "When a document contains both SBU-NF and SBU
        // portions, SBU NOFORN supersedes SBU in the banner line."
        // Drop bare SBU; keep SBU-NF. At classified the existing #541
        // strip then removes SBU-NF entirely, leaving `{}` + needs_nf
        // (banner `SECRET//NOFORN`). At unclassified the SBU-NF
        // survives (banner `UNCLASSIFIED//SBU NOFORN`).
        if set.contains(&NonIcDissem::SbuNf) {
            set.remove(&NonIcDissem::Sbu);
        }
        // §H.9 p185 (LES NOFORN — banner-form heading + Notional
        // Example Page 1): the banner for `(U//LES-NF)` portions is
        // `UNCLASSIFIED//LES NOFORN`, i.e. the LES-NF compound carries
        // the LES family marker in unclassified banner form. With both
        // `Les` and `LesNf` portions present, LES-NF dominates bare
        // LES on the unclassified banner. The existing #541 classified
        // split then transforms `{LesNf}` → `{Les}` + needs_nf at
        // classified, yielding `SECRET//NOFORN//LES` per §H.9 p185
        // (LES NOFORN Precedence Rules for Banner Line Guidance).
        if set.contains(&NonIcDissem::LesNf) {
            set.remove(&NonIcDissem::Les);
        }

        let mut needs_nf = false;
        if classified {
            // §H.9 p178 (SBU NOFORN Commingling Rule(s) Within a
            // Portion): "If the portion is classified, the
            // classification level of the portion adequately protects
            // the SBU information, so SBU is not reflected in the
            // portion mark; however a NOFORN marking must be added to
            // the portion mark, e.g., (C//NF)." SBU vanishes entirely;
            // NOFORN injection happens via `needs_nf`. Asymmetric with
            // the LES-NF branch immediately below (LES survives) —
            // see the type-level doc-comment for the regulatory
            // rationale. #541. Re-verified 2026-05-18 against
            // `crates/capco/docs/CAPCO-2016.md`.
            if set.remove(&NonIcDissem::SbuNf) {
                needs_nf = true;
            }
            // §H.9 p185 (LES NOFORN Precedence Rules for Banner Line
            // Guidance): "The LES marking always
            // appears in the banner line if LES information (either
            // LES or LES NOFORN) is contained in the document,
            // regardless of the document's classification level. When
            // a classified document contains portions of U//LES-NF,
            // the 'LES' marking is used in the banner line and the
            // NOFORN marking is applied as a Dissemination Control
            // Marking. For example: SECRET//NOFORN//LES." LES is
            // RETAINED in the output set (asymmetric with SBU above);
            // NOFORN injection happens via `needs_nf` in parallel.
            // Re-verified 2026-05-18 against
            // `crates/capco/docs/CAPCO-2016.md`.
            if set.remove(&NonIcDissem::LesNf) {
                set.insert(NonIcDissem::Les);
                needs_nf = true;
            }
        }

        // §H.9 p172 (EXDIS) / p174 (NODIS): NF must be injected into
        // the dissem block regardless of classification level. NODIS
        // / EXDIS themselves stay in the non-IC set.
        if set.contains(&NonIcDissem::Nodis) || set.contains(&NonIcDissem::Exdis) {
            needs_nf = true;
        }

        Self { set, needs_nf }
    }

    /// Whether NOFORN must be injected into the dissem block at
    /// banner roll-up.
    pub fn needs_nf(&self) -> bool {
        self.needs_nf
    }

    /// Borrow the underlying set.
    pub fn as_set(&self) -> &BTreeSet<NonIcDissem> {
        &self.set
    }

    /// Render to a `Box<[NonIcDissem]>` in BTreeSet natural order.
    pub fn into_boxed_slice(self) -> Box<[NonIcDissem]> {
        self.set.into_iter().collect::<Vec<_>>().into_boxed_slice()
    }

    /// Consume into `(set, needs_nf)` to match
    /// `PageContext::expected_non_ic_dissem`'s tuple shape.
    pub fn into_inner_with_needs_nf(self) -> (Vec<NonIcDissem>, bool) {
        (self.set.into_iter().collect(), self.needs_nf)
    }

    /// Render to a `Vec<NonIcDissem>` for compatibility.
    pub fn to_vec(&self) -> Vec<NonIcDissem> {
        self.set.iter().copied().collect()
    }
}

// ---------------------------------------------------------------------------
// DeclassExemptionAccumulator — last-observed declass exemption.
// ---------------------------------------------------------------------------

/// Last-observed accumulator for the declass-exemption axis.
///
/// **Projection helper, NOT a lattice.** Earlier drafts of this type
/// implemented `JoinSemilattice` with a "right-operand-wins" join body
/// that was admittedly non-commutative. The trait contract at
/// `crates/scheme/src/lattice.rs:55-64` requires commutativity, so the
/// review chain (rust-reviewer H-1 + lattice-consultant L-1) called
/// the impl what it was — a contract violation. The fix follows
/// [`NonIcDissemSet`]'s precedent: drop the `JoinSemilattice` impl,
/// keep the type as a projection accumulator surfacing
/// `from_attrs_iter` + `into_inner` / `as_inner`. The rename
/// `DeclassExemptionLattice -> DeclassExemptionAccumulator` makes the
/// non-lattice nature explicit at the type-name level.
///
/// Conservative "last-observed" semantics: `from_attrs_iter` walks
/// portions in document order and keeps the last portion's
/// `declass_exemption`, or `None` if no portion carries one. The CAB
/// generator in `crates/wasm/src/lib.rs` uses the same shape via an
/// inline accumulator (see `generate_cab_native`).
///
/// **Phase 3 TODO** (carried over from
/// `PageContext::expected_declass_exemption`): a correct implementation
/// would return the exemption providing the longest period of protection
/// per CAPCO-2016 §E.3 pp 32-33 (Multiple Sources hierarchy:
/// 50X1 - HUM > 50X2 - WMD > ... > 25X# > derived calculation). The
/// current implementation is the conservative last-observed placeholder;
/// Phase 3 should add a duration-aware comparator.
///
/// §-authority (verified 2026-05-18 against
/// `crates/capco/docs/CAPCO-2016.md`):
/// - §E.1 p31 (exemption-category catalog: 25X#/50X#/75X# values).
/// - §E.3 pp 32-33 (Multiple Sources hierarchy — the "longest period
///   of protection" rule the Phase 3 TODO targets; the §E.3 prose at
///   lines 665+ of the markdown spells out the 50X > 25X precedence and
///   the same-date-tiebreaker rule).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclassExemptionAccumulator(Option<marque_ism::DeclassExemption>);

impl DeclassExemptionAccumulator {
    /// An empty exemption — the accumulator's identity / bottom value.
    pub fn empty() -> Self {
        Self(None)
    }

    /// Construct from a slice of `CanonicalAttrs` — last-observed
    /// exemption across portions in document order, or `None` if no
    /// portion carries one.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        Self(
            portions
                .iter()
                .filter_map(|a| a.declass_exemption)
                .next_back(),
        )
    }

    /// Consume into the inner `Option<DeclassExemption>`.
    pub fn into_inner(self) -> Option<marque_ism::DeclassExemption> {
        self.0
    }

    /// Borrow the inner `Option<DeclassExemption>`.
    pub fn as_inner(&self) -> Option<marque_ism::DeclassExemption> {
        self.0
    }
}

// ---------------------------------------------------------------------------
// DisplayOnlyBlock — lattice over the DISPLAY ONLY axis (cross-axis
// intersection over (REL TO ∪ DO), with banner-REL-TO and USA subtraction).
// ---------------------------------------------------------------------------

/// Lattice form of the DISPLAY ONLY axis on a page.
///
/// Carries the post-intersection set of country codes that should appear in
/// the banner's `DISPLAY ONLY [LIST]` block per CAPCO-2016 §H.8 p163 +
/// §D.2 Table 3 rows 18-20 + 25-27.
///
/// # Semantics
///
/// 1. **NOFORN supersedes.** Any portion carrying `Nf` in `dissem_us` → `Empty`.
///    (§D.2 Table 3 rows 1-2 + §H.8 p145.)
/// 2. **NODIS / EXDIS short-circuit.** Any portion carrying NODIS or EXDIS
///    in `non_ic_dissem` → `Empty`. The `needs_nf` flag from
///    `NonIcDissemSet::from_attrs_iter` injects NOFORN at the dissem layer,
///    and per §D.2 Table 3 row 2 NOFORN + DISPLAY ONLY cannot coexist on
///    the banner. (§H.9 p172 / p174.)
/// 3. **Row-19 all-or-nothing gate.** Every portion MUST have a non-empty
///    display-permission set (REL TO ∪ DISPLAY ONLY). A portion with
///    neither axis collapses the result to `Empty` per §D.2 Table 3 row 19
///    (DISPLAY ONLY + portion without FD&R → NOFORN banner).
/// 4. **Per-portion display permission = expand(REL TO) ∪ expand(DISPLAY ONLY).**
///    Tetragraph expansion uses `marque_ism::lookup_tetragraph_members`
///    (FVEY/ACGU/… → constituent trigraphs); opaque codes pass through.
///    Per §D.2 Table 3 row 26 Note ("if information is approved for
///    release to a given audience it has automatically been approved for
///    disclosure to that audience"), each portion's display-permission
///    set is the union of REL TO and DO axes — release subsumes disclosure.
/// 5. **Cross-portion intersection.** The banner DO list is the intersection
///    of per-portion display-permission sets across all portions.
/// 6. **Banner-REL-TO subtraction (row 27).** Countries that appear in the
///    banner's REL TO axis do NOT also appear in DO — REL TO is the
///    stricter axis. The constructor takes a pre-computed `RelToBlock`
///    to subtract.
/// 7. **USA subtraction.** USA is the implicit originator (per §H.8 p163
///    worked examples, USA never appears in the DO axis).
/// 8. **Ordering.** Trigraphs (length 3) first, then tetragraphs and other
///    opaque codes; alphabetical within each bucket per §H.8 p164.
///
/// # Variants
///
/// Mirrors `RelToBlock`'s 4-variant shape so `join` has an absorbing
/// element on each branch and stays associative:
///
/// - `Bottom`: no DISPLAY ONLY portions observed. Identity for `join`.
/// - `Lattice { countries }`: post-intersection non-empty set.
/// - `Empty`: DISPLAY ONLY portions exist but intersection / row-19 gate
///   collapsed the result (no NOFORN). Distinguishable from `Bottom` to
///   keep `join` associative.
/// - `NofornSuperseded`: some portion carries NOFORN (or NODIS/EXDIS).
///   Absorbs further joins; strictly stronger than `Empty`.
///
/// # §-authority (verified 2026-05-18 against `crates/capco/docs/CAPCO-2016.md`):
///
/// - §H.8 p163 (DISPLAY ONLY template + banner grammar).
/// - §D.2 Table 3 rows 18-20 (DISPLAY ONLY + RELIDO / no-FD&R / disjoint-DO).
/// - §D.2 Table 3 rows 25-27 (DISPLAY ONLY common-LIST + REL TO + dual-channel).
/// - §H.9 p172 + p174 (NODIS / EXDIS clear DISPLAY ONLY via NF injection).
/// - §H.8 p145 (NOFORN dominates DISPLAY ONLY).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum DisplayOnlyBlock {
    /// No DISPLAY ONLY portions observed. Identity for `join`.
    #[default]
    Bottom,

    /// Post-intersection set of country codes for the banner DO block.
    /// Sorted trigraphs-first then alphabetical per §H.8 p164.
    Lattice {
        /// BTreeSet for deterministic ordering; render via
        /// `into_boxed_slice` for the §H.8 p164 sort.
        countries: BTreeSet<CountryCode>,
    },

    /// DISPLAY ONLY portions observed but the row-19 gate, the empty
    /// intersection, or the USA / banner-REL-TO subtraction collapsed
    /// the result to empty. Distinguishable from `Bottom` so `join`
    /// keeps an absorbing element separate from the identity.
    Empty,

    /// Some portion carries NOFORN (or the NODIS / EXDIS equivalents
    /// that inject NOFORN at the dissem layer). The sentinel absorbs
    /// further joins; strictly stronger than `Empty`.
    NofornSuperseded,
}

impl DisplayOnlyBlock {
    /// An empty DISPLAY ONLY block — the lattice bottom.
    pub fn empty() -> Self {
        Self::Bottom
    }

    /// Construct a `DisplayOnlyBlock` from a slice of `CanonicalAttrs`,
    /// the pre-computed banner `RelToBlock` (for row-27 subtraction),
    /// and the pre-computed `needs_nf` flag from
    /// `NonIcDissemSet::from_attrs_iter` (for the NODIS/EXDIS
    /// short-circuit).
    ///
    /// Splitting the inputs lets callers share work — `marking.rs`'s
    /// page-aggregation path already computes both `RelToBlock` and
    /// `NonIcDissemSet` for other axes; passing them in avoids
    /// recomputation.
    pub fn from_attrs_iter(
        portions: &[CanonicalAttrs],
        rel_to_block: &RelToBlock,
        needs_nf: bool,
    ) -> Self {
        if portions.is_empty() {
            return Self::Bottom;
        }

        // (1) NOFORN supersession — §D.2 Table 3 rows 1-2.
        let any_noforn = portions
            .iter()
            .any(|a| a.dissem_us.iter().any(|d| matches!(d, DissemControl::Nf)));
        if any_noforn {
            return Self::NofornSuperseded;
        }

        // (2) NODIS / EXDIS short-circuit via the NonIcDissemSet
        // `needs_nf` signal (which also fires on SBU-NF / LES-NF
        // classified-context splits at §H.9 p178 / p185). Per §D.2
        // Table 3 row 2 NOFORN + DISPLAY ONLY cannot coexist on the
        // banner.
        if needs_nf {
            return Self::NofornSuperseded;
        }

        // (3) Row-19 all-or-nothing gate: every portion must have a
        // non-empty (REL TO ∪ DISPLAY ONLY) set. A portion with
        // neither makes the page fall into NOFORN by row 19. We
        // surface this as `Empty` (no display-permission countries
        // survive) — the caller's NOFORN-injection logic is at the
        // dissem layer, not here.
        let any_empty = portions
            .iter()
            .any(|a| a.rel_to.is_empty() && a.display_only_to.is_empty());
        if any_empty {
            return Self::Empty;
        }

        // (4) Per-portion display permission = expand(REL TO) ∪
        // expand(DISPLAY ONLY) — release subsumes disclosure (§D.2
        // Table 3 row 26 Note). Inline-8 covers the typical per-page
        // portion count; 9+ portions spill to heap cleanly (LA-4).
        let expanded: SmallVec<[BTreeSet<&str>; 8]> = portions
            .iter()
            .map(|a| {
                let mut set = BTreeSet::new();
                for t in a.rel_to.iter().chain(a.display_only_to.iter()) {
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

        // (5) Cross-portion intersection.
        let mut result: BTreeSet<&str> = expanded[0].clone();
        for set in &expanded[1..] {
            result = result.intersection(set).copied().collect();
        }

        // (6) Subtract banner REL TO countries — §D.2 Table 3 row 27.
        // (7) Subtract USA — implicit originator per §H.8 p163 worked
        //     examples.
        let rel_to_codes = rel_to_block.to_vec();
        let rel_set: BTreeSet<&str> = rel_to_codes.iter().map(|c| c.as_str()).collect();
        result.remove("USA");
        let result: BTreeSet<&str> = result.difference(&rel_set).copied().collect();

        if result.is_empty() {
            return Self::Empty;
        }

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

    /// Render to a `Box<[CountryCode]>` with trigraphs first (length 3)
    /// then tetragraphs and other opaque codes, alphabetical within
    /// each bucket per §H.8 p164.
    pub fn into_boxed_slice(self) -> Box<[CountryCode]> {
        self.to_vec().into_boxed_slice()
    }

    /// Render to a `Vec<CountryCode>` mirroring
    /// `PageContext::expected_display_only`'s shape.
    pub fn to_vec(&self) -> Vec<CountryCode> {
        match self {
            Self::Bottom | Self::Empty | Self::NofornSuperseded => Vec::new(),
            Self::Lattice { countries } => {
                let mut codes: Vec<CountryCode> = countries.iter().copied().collect();
                codes.sort_by(|a, b| {
                    let a_is_trigraph = a.as_str().len() == 3;
                    let b_is_trigraph = b.as_str().len() == 3;
                    a_is_trigraph
                        .cmp(&b_is_trigraph)
                        .reverse()
                        .then_with(|| a.as_str().cmp(b.as_str()))
                });
                codes
            }
        }
    }

    /// Whether the block is the `NofornSuperseded` sentinel.
    pub fn is_noforn_superseded(&self) -> bool {
        matches!(self, Self::NofornSuperseded)
    }

    /// Whether the block is the `Empty` absorbing state.
    pub fn is_empty_intersection(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl JoinSemilattice for DisplayOnlyBlock {
    fn join(&self, other: &Self) -> Self {
        // NofornSuperseded > Empty > Lattice{·} > Bottom.
        // Mirrors `RelToBlock::join` structurally — same 4-variant
        // absorbing-element pattern so `join` stays associative.
        match (self, other) {
            (Self::NofornSuperseded, _) | (_, Self::NofornSuperseded) => Self::NofornSuperseded,
            (Self::Empty, _) | (_, Self::Empty) => Self::Empty,
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
    fn fgi_set_none_is_empty() {
        // B-1 (PR 4b-B 8th-pass): `FgiSet::bottom()` retired alongside
        // the `BoundedLattice` impl. `FgiSet::empty()` is the public
        // bottom constructor; `FgiSet::None` is the variant it maps to.
        assert_eq!(FgiSet::empty(), FgiSet::None);
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

    // `fgi_set_top_is_concealed_empty` retired in B-1 (PR 4b-B 8th-pass
    // follow-up). `FgiSet` no longer implements `BoundedLattice`; the
    // `SourceConcealed` supersession sentinel is still reachable via
    // `FgiSet::from_marker(Some(&FgiMarker::SourceConcealed))`, exercised
    // by `fgi_set_meet_both_concealed_preserved` below.

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

    // -----------------------------------------------------------------------
    // PR 4b-E: new lattice helpers — happy-path + lattice-law coverage
    // -----------------------------------------------------------------------

    use marque_ism::{
        CanonicalAttrs, Classification, DeclassExemption, MarkingClassification,
        NatoClassification, NonIcDissem,
    };

    fn portion_us(level: Classification) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(level));
        a
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
        let mut p1 = CanonicalAttrs::default();
        p1.sci_controls = Box::new([marque_ism::SciControl::Si]);
        let mut p2 = CanonicalAttrs::default();
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
        let mut p1 = CanonicalAttrs::default();
        p1.sci_controls = Box::new([marque_ism::SciControl::Si]);
        let mut p2 = CanonicalAttrs::default();
        p2.sci_controls = Box::new([marque_ism::SciControl::Tk]);
        let controls = sci_controls_from_markings(&[p1, p2]);
        assert!(controls.contains(&marque_ism::SciControl::Si));
        assert!(controls.contains(&marque_ism::SciControl::Tk));
    }

    // FgiSet::from_attrs_iter — happy-path + concealed-dominates + JOINT
    // producer extraction + associativity.

    #[test]
    fn fgi_set_from_attrs_iter_empty_returns_none() {
        let portions: [CanonicalAttrs; 0] = [];
        assert_eq!(FgiSet::from_attrs_iter(&portions), FgiSet::None);
    }

    #[test]
    fn fgi_set_from_attrs_iter_nato_classification_yields_nato_producer() {
        let mut p = CanonicalAttrs::default();
        p.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
        let result = FgiSet::from_attrs_iter(&[p]);
        match result {
            FgiSet::Present {
                concealed: false,
                countries,
            } => {
                let nato = CountryCode::try_new(b"NATO").unwrap();
                assert!(countries.contains(&nato), "expected NATO producer");
            }
            other => panic!("expected Present {{concealed: false}}, got {other:?}"),
        }
    }

    #[test]
    fn fgi_set_from_attrs_iter_concealed_dominates_acknowledged() {
        // §H.7 p128: mixed concealed + acknowledged → concealed wins.
        let mut concealed_portion = CanonicalAttrs::default();
        concealed_portion.fgi_marker = Some(FgiMarker::SourceConcealed);
        let mut acknowledged_portion = CanonicalAttrs::default();
        acknowledged_portion.fgi_marker =
            FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]);
        let result = FgiSet::from_attrs_iter(&[concealed_portion, acknowledged_portion]);
        assert!(
            matches!(
                result,
                FgiSet::Present {
                    concealed: true,
                    ..
                }
            ),
            "concealed must dominate; got {result:?}"
        );
    }

    #[test]
    fn fgi_set_from_attrs_iter_joint_excludes_usa_producer() {
        // JOINT producers contribute to FGI minus USA (USA is implicit
        // owner, not a foreign source).
        let mut joint_portion = CanonicalAttrs::default();
        joint_portion.classification = Some(MarkingClassification::Joint(
            marque_ism::JointClassification {
                level: Classification::Secret,
                countries: Box::new([
                    CountryCode::try_new(b"USA").unwrap(),
                    CountryCode::try_new(b"GBR").unwrap(),
                ]),
            },
        ));
        let result = FgiSet::from_attrs_iter(&[joint_portion]);
        match result {
            FgiSet::Present {
                concealed: false,
                countries,
            } => {
                let usa = CountryCode::try_new(b"USA").unwrap();
                let gbr = CountryCode::try_new(b"GBR").unwrap();
                assert!(!countries.contains(&usa), "USA must NOT appear");
                assert!(countries.contains(&gbr), "GBR must appear");
            }
            other => panic!("expected Present {{concealed: false}}, got {other:?}"),
        }
    }

    #[test]
    fn fgi_set_from_attrs_iter_associative_with_join() {
        // Lattice law: from_attrs_iter(&a ++ b ++ c) == from_attrs_iter(&a).join(&...)
        // The construction path is union-based; assembling per-portion
        // via repeated join must agree with bulk construction.
        let mut p1 = CanonicalAttrs::default();
        p1.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]);
        let mut p2 = CanonicalAttrs::default();
        p2.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"DEU").unwrap()]);
        let mut p3 = CanonicalAttrs::default();
        p3.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"FRA").unwrap()]);

        let bulk = FgiSet::from_attrs_iter(&[p1.clone(), p2.clone(), p3.clone()]);
        let step = FgiSet::from_attrs_iter(&[p1])
            .join(&FgiSet::from_attrs_iter(&[p2]))
            .join(&FgiSet::from_attrs_iter(&[p3]));
        assert_eq!(
            bulk, step,
            "bulk construction must agree with iterated join"
        );
    }

    // NonIcDissemSet — classification gate, NF injection, lattice
    // bottom invariant.

    #[test]
    fn non_ic_dissem_set_default_is_empty_bottom() {
        let s = NonIcDissemSet::default();
        assert!(s.as_set().is_empty());
        assert!(!s.needs_nf());
    }

    #[test]
    fn non_ic_dissem_set_empty_equals_default() {
        assert_eq!(NonIcDissemSet::empty(), NonIcDissemSet::default());
    }

    #[test]
    fn non_ic_dissem_set_sbu_nf_drops_sbu_on_classified() {
        // §H.9 p178 (Commingling Rule(s) Within a Portion): "If the
        // portion is classified, the classification level of the
        // portion adequately protects the SBU information, so SBU is
        // not reflected in the portion mark; however a NOFORN marking
        // must be added to the portion mark, e.g., (C//NF)." SBU
        // vanishes entirely; only NOFORN survives via `needs_nf`.
        // #541.
        let mut p = portion_us(Classification::Secret);
        p.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            !s.as_set().contains(&NonIcDissem::Sbu),
            "§H.9 p178: SBU is not reflected on classified portion; \
             set must NOT contain Sbu after SBU-NF strip. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.as_set().contains(&NonIcDissem::SbuNf),
            "SBU-NF must be removed from the set (it's transformed \
             into NOFORN-via-needs_nf). set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p178: NOFORN must be added to the portion mark for \
             classified-context SBU-NF. needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_sbu_nf_kept_on_unclassified() {
        // §H.9 p178 (canonical unclassified form): SBU-NF on
        // unclassified pages survives verbatim — banner
        // `UNCLASSIFIED//SBU NOFORN`, portion `(U//SBU-NF)`. No
        // transformation. Symmetric with the LES-NF unclassified
        // case immediately below.
        let mut p = portion_us(Classification::Unclassified);
        p.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            s.as_set().contains(&NonIcDissem::SbuNf),
            "§H.9 p178 (canonical unclassified form): SBU-NF must \
             survive verbatim on unclassified pages. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p178: unclassified SBU-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_les_nf_splits_on_classified() {
        // §H.9 p185 (LES NOFORN Precedence Rules for Banner Line
        // Guidance): "The LES marking always appears in the banner
        // line if LES information (either LES or LES NOFORN) is
        // contained in the document, regardless of the document's
        // classification level. When a classified document contains
        // portions of U//LES-NF, the 'LES' marking is used in the
        // banner line and the NOFORN marking is applied as a
        // Dissemination Control Marking. For example:
        // SECRET//NOFORN//LES."
        //
        // This is the negative-regression gate for #541's asymmetry:
        // LES MUST survive classification (unlike SBU). LES carries
        // independent regulatory authority (law-enforcement
        // legal-process restrictions, originator-control discipline)
        // that classification does NOT subsume; SBU is purely
        // admin-protection that classification DOES subsume. A
        // future "make it symmetric" change-of-mind must trip this
        // test before it can land.
        let mut p = portion_us(Classification::Secret);
        p.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            s.as_set().contains(&NonIcDissem::Les),
            "§H.9 p185: LES survives on classified pages; set must \
             contain Les after LES-NF split. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.as_set().contains(&NonIcDissem::LesNf),
            "LES-NF must be removed (transformed into Les + NOFORN). \
             set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p185: NOFORN must be added at banner roll-up. \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_les_nf_kept_on_unclassified() {
        // §H.9 p185 (canonical unclassified form): portion form
        // `(U//LES-NF)` retained as-is on unclassified pages.
        // Symmetric with the SBU-NF unclassified case.
        let mut p = portion_us(Classification::Unclassified);
        p.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(
            s.as_set().contains(&NonIcDissem::LesNf),
            "§H.9 p185 (canonical unclassified form): LES-NF must \
             survive verbatim on unclassified pages. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p185: unclassified LES-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    // -----------------------------------------------------------
    // #552 — same-axis compound-supersedes-bare overlay tests.
    // -----------------------------------------------------------

    #[test]
    fn non_ic_dissem_set_sbu_nf_supersedes_sbu_on_unclassified() {
        // §H.9 p178 (SBU NOFORN Precedence Rules for Banner Line
        // Guidance): "When a document contains both SBU-NF and SBU
        // portions, SBU NOFORN supersedes SBU in the banner line."
        // Net unclassified output: `{SbuNf}` only; banner
        // `UNCLASSIFIED//SBU NOFORN`. #552.
        let mut p_sbu = portion_us(Classification::Unclassified);
        p_sbu.non_ic_dissem = Box::new([NonIcDissem::Sbu]);
        let mut p_sbu_nf = portion_us(Classification::Unclassified);
        p_sbu_nf.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_sbu, p_sbu_nf]);
        assert!(
            !s.as_set().contains(&NonIcDissem::Sbu),
            "§H.9 p178: SBU-NF supersedes SBU; bare Sbu must be \
             dropped on co-presence. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.as_set().contains(&NonIcDissem::SbuNf),
            "§H.9 p178: compound SBU-NF survives the supersession. \
             set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p178: unclassified SBU-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_les_nf_supersedes_les_on_unclassified() {
        // §H.9 p185 (LES NOFORN — banner-form heading + Notional
        // Example Page 1): banner for `(U//LES-NF)` portions is
        // `UNCLASSIFIED//LES NOFORN`; LES-NF compound carries the
        // LES family marker, so bare LES is redundant on
        // co-presence. Net unclassified output: `{LesNf}` only;
        // banner `UNCLASSIFIED//LES NOFORN`. #552.
        let mut p_les = portion_us(Classification::Unclassified);
        p_les.non_ic_dissem = Box::new([NonIcDissem::Les]);
        let mut p_les_nf = portion_us(Classification::Unclassified);
        p_les_nf.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_les, p_les_nf]);
        assert!(
            !s.as_set().contains(&NonIcDissem::Les),
            "§H.9 p185: LES-NF supersedes LES on co-presence; bare \
             Les must be dropped. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.as_set().contains(&NonIcDissem::LesNf),
            "§H.9 p185: compound LES-NF survives the supersession. \
             set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.needs_nf(),
            "§H.9 p185: unclassified LES-NF does not trigger NOFORN \
             injection (NF is encoded in the compound token itself). \
             needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_classified_sbu_and_sbu_nf_strip_to_needs_nf() {
        // #552 + #541 interaction: both bare SBU and compound SBU-NF
        // present on a classified page. Step 1 (#552 supersession)
        // drops bare SBU. Step 2 (#541 classified gate) strips
        // SBU-NF and asserts `needs_nf`. Net: empty set + `needs_nf`
        // → banner `SECRET//NOFORN`. §H.9 p178.
        let mut p_sbu = portion_us(Classification::Secret);
        p_sbu.non_ic_dissem = Box::new([NonIcDissem::Sbu]);
        let mut p_sbu_nf = portion_us(Classification::Secret);
        p_sbu_nf.non_ic_dissem = Box::new([NonIcDissem::SbuNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_sbu, p_sbu_nf]);
        assert!(
            s.as_set().is_empty(),
            "§H.9 p178: classified strip after #552 supersession \
             must leave the non-IC set empty. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p178: NOFORN must be injected on classified \
             SBU-NF strip. needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_classified_les_and_les_nf_split_to_les() {
        // #552 + #541 interaction: both bare LES and compound LES-NF
        // present on a classified page. Step 1 (#552 supersession)
        // drops bare LES. Step 2 (#541 classified gate) splits
        // LES-NF → re-inserts bare Les and asserts `needs_nf`. Net:
        // `{Les}` + `needs_nf` → banner `SECRET//NOFORN//LES` per
        // §H.9 p185.
        let mut p_les = portion_us(Classification::Secret);
        p_les.non_ic_dissem = Box::new([NonIcDissem::Les]);
        let mut p_les_nf = portion_us(Classification::Secret);
        p_les_nf.non_ic_dissem = Box::new([NonIcDissem::LesNf]);
        let s = NonIcDissemSet::from_attrs_iter(&[p_les, p_les_nf]);
        assert!(
            s.as_set().contains(&NonIcDissem::Les),
            "§H.9 p185: classified split after #552 supersession \
             must leave bare Les in the set. set = {:?}",
            s.as_set(),
        );
        assert!(
            !s.as_set().contains(&NonIcDissem::LesNf),
            "§H.9 p185: LES-NF must be transformed into Les + \
             NOFORN on classified pages. set = {:?}",
            s.as_set(),
        );
        assert!(
            s.needs_nf(),
            "§H.9 p185: NOFORN must be injected on classified \
             LES-NF split. needs_nf = {}",
            s.needs_nf(),
        );
    }

    #[test]
    fn non_ic_dissem_set_nodis_injects_nf_regardless_of_classification() {
        // §H.9 p174: NODIS → NF in banner, classification-independent.
        let mut p = portion_us(Classification::Unclassified);
        p.non_ic_dissem = Box::new([NonIcDissem::Nodis]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(s.as_set().contains(&NonIcDissem::Nodis));
        assert!(s.needs_nf());
    }

    #[test]
    fn non_ic_dissem_set_exdis_injects_nf() {
        // §H.9 p172: EXDIS → NF in banner.
        let mut p = portion_us(Classification::Secret);
        p.non_ic_dissem = Box::new([NonIcDissem::Exdis]);
        let s = NonIcDissemSet::from_attrs_iter(&[p]);
        assert!(s.as_set().contains(&NonIcDissem::Exdis));
        assert!(s.needs_nf());
    }

    #[test]
    fn non_ic_dissem_set_from_empty_input_is_bottom() {
        let s = NonIcDissemSet::from_attrs_iter(&[]);
        assert_eq!(s, NonIcDissemSet::empty());
    }

    // DeclassExemptionAccumulator — last-observed projection helper.
    //
    // Renamed from `DeclassExemptionLattice` in PR 4b-E review fix-up
    // (rust-reviewer H-1 + lattice-consultant L-1): the type is a
    // projection accumulator, not a lattice — the prior
    // `JoinSemilattice` impl was non-commutative by construction and
    // violated the trait contract. The `idempotent_on_join` and
    // `identity_with_bottom` tests retired with the impl; only the
    // `from_attrs_iter` invariants remain.

    #[test]
    fn declass_exemption_accumulator_default_is_bottom() {
        let l = DeclassExemptionAccumulator::default();
        assert_eq!(l.as_inner(), None);
    }

    #[test]
    fn declass_exemption_accumulator_empty_equals_default() {
        assert_eq!(
            DeclassExemptionAccumulator::empty(),
            DeclassExemptionAccumulator::default()
        );
    }

    #[test]
    fn declass_exemption_accumulator_from_attrs_iter_picks_last_observed() {
        let mut p1 = CanonicalAttrs::default();
        p1.declass_exemption = Some(DeclassExemption::X25x1);
        let mut p2 = CanonicalAttrs::default();
        p2.declass_exemption = Some(DeclassExemption::X25x2);
        let l = DeclassExemptionAccumulator::from_attrs_iter(&[p1, p2]);
        assert_eq!(l.as_inner(), Some(DeclassExemption::X25x2));
    }

    #[test]
    fn declass_exemption_accumulator_from_attrs_iter_empty_is_bottom() {
        let l = DeclassExemptionAccumulator::from_attrs_iter(&[]);
        assert_eq!(l, DeclassExemptionAccumulator::empty());
    }

    // ----------------------------------------------------------------
    // DisplayOnlyBlock — happy-path + §D.2 Table 3 rows 18-20 / 25-27
    // ----------------------------------------------------------------

    fn portion_with_rel_to(level: Classification, rel: &[&str]) -> CanonicalAttrs {
        let mut a = portion_us(level);
        a.rel_to = rel
            .iter()
            .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        a
    }

    fn portion_with_display_only(level: Classification, display: &[&str]) -> CanonicalAttrs {
        let mut a = portion_us(level);
        a.display_only_to = display
            .iter()
            .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        a
    }

    fn portion_with_dissem_us(level: Classification, dissem: &[DissemControl]) -> CanonicalAttrs {
        let mut a = portion_us(level);
        a.dissem_us = dissem.to_vec().into_boxed_slice();
        a
    }

    #[test]
    fn display_only_block_default_is_bottom() {
        let b = DisplayOnlyBlock::default();
        assert_eq!(b, DisplayOnlyBlock::Bottom);
    }

    #[test]
    fn display_only_block_empty_returns_bottom() {
        // Empty portions → Bottom.
        let b = DisplayOnlyBlock::from_attrs_iter(&[], &RelToBlock::empty(), false);
        assert_eq!(b, DisplayOnlyBlock::Bottom);
    }

    #[test]
    fn display_only_block_noforn_superseded() {
        // §D.2 Table 3 rows 1-2 + §H.8 p145: NOFORN dominates DO.
        let portions = [portion_with_dissem_us(
            Classification::Secret,
            &[DissemControl::Nf],
        )];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), false);
        assert!(b.is_noforn_superseded());
    }

    #[test]
    fn display_only_block_needs_nf_short_circuits_to_noforn() {
        // §H.9 p172 (EXDIS) / p174 (NODIS) inject NF at dissem layer;
        // per §D.2 Table 3 row 2 NF + DO cannot coexist.
        let portions = [portion_with_display_only(Classification::Secret, &["GBR"])];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), true);
        assert!(b.is_noforn_superseded());
    }

    #[test]
    fn display_only_block_row_19_empty_portion_collapses() {
        // §D.2 Table 3 row 19: DO + portion with no FD&R → NOFORN.
        // We surface as Empty; caller injects NF at dissem layer.
        let portions = [
            portion_with_display_only(Classification::Secret, &["GBR"]),
            portion_us(Classification::Secret),
        ];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), false);
        assert!(b.is_empty_intersection());
    }

    #[test]
    fn display_only_block_simple_intersection() {
        // §D.2 Table 3 row 25: DO + DO with common LIST → DO [common].
        let portions = [
            portion_with_display_only(Classification::Secret, &["GBR", "CAN"]),
            portion_with_display_only(Classification::Secret, &["GBR", "AUS"]),
        ];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), false);
        let codes = b.to_vec();
        let gbr = CountryCode::try_new(b"GBR").unwrap();
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0], gbr);
    }

    #[test]
    fn display_only_block_disjoint_intersection_is_empty() {
        // §D.2 Table 3 row 20: DO + DO with no common LIST → NOFORN.
        let portions = [
            portion_with_display_only(Classification::Secret, &["GBR"]),
            portion_with_display_only(Classification::Secret, &["AUS"]),
        ];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), false);
        assert!(b.is_empty_intersection());
    }

    #[test]
    fn display_only_block_cross_axis_with_empty_rel_to_keeps_gbr() {
        // §D.2 Table 3 row 26 Note (no banner REL TO branch):
        // when the input `rel_to_block` is empty/`Bottom`, the
        // row-27 subtraction has nothing to subtract — the DO
        // intersection survives intact.
        //
        // Copilot R1 fix: the previous combined test computed
        // `RelToBlock::from_attrs_iter(portions)` and admitted
        // an ambiguous outcome ("Lattice{GBR} or Empty are both
        // acceptable"). That admitted-ambiguity passes even if
        // a future change silently swaps the variants. Splitting
        // into two tests with deterministic `rel_to_block` inputs
        // (`empty()` and the `Lattice {USA,GBR}` construction
        // below) pins each row-27 branch independently.
        let portions = [
            portion_with_rel_to(Classification::Secret, &["USA", "GBR"]),
            portion_with_display_only(Classification::Secret, &["GBR"]),
        ];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), false);
        // With `rel_to_block = Bottom`, row-27 subtraction is a
        // no-op. The DO intersection is {GBR} (REL TO portion
        // contributes display-permission {USA,GBR}; DO portion
        // contributes {GBR}; intersection {GBR}; USA stripped per
        // §H.8 p163 USA-subtraction). Result: `Lattice {GBR}`.
        let codes = b.to_vec();
        assert_eq!(
            codes,
            vec![CountryCode::GBR],
            "empty rel_to_block leaves DO intersection {{GBR}} intact, \
             got {b:?}"
        );
    }

    #[test]
    fn display_only_block_cross_axis_with_banner_rel_to_empties_gbr() {
        // §D.2 Table 3 row 27: when banner REL TO covers the same
        // countries as the DO intersection, row-27 subtraction
        // empties the DO list — the explicit REL TO authorization
        // makes the explicit DISPLAY ONLY redundant.
        //
        // Copilot R1 fix: companion to
        // `display_only_block_cross_axis_with_empty_rel_to_keeps_gbr`
        // pinning the non-empty banner REL TO branch. Construct
        // `RelToBlock::Lattice {USA,GBR}` directly inside the crate
        // (the variant is `#[non_exhaustive]` for external callers
        // only) so the row-27 subtraction has a deterministic input.
        let portions = [
            portion_with_rel_to(Classification::Secret, &["USA", "GBR"]),
            portion_with_display_only(Classification::Secret, &["GBR"]),
        ];
        let banner_rel_to = RelToBlock::Lattice {
            countries: [CountryCode::USA, CountryCode::GBR].into_iter().collect(),
        };
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &banner_rel_to, false);
        // DO intersection {GBR} minus banner REL TO {USA,GBR} = {}
        // → `Empty` (row 9-ish absorbing, distinct from `Bottom`).
        assert!(
            matches!(b, DisplayOnlyBlock::Empty),
            "row-27 subtraction over {{USA,GBR}} empties the DO list, \
             expected Empty, got {b:?}"
        );
    }

    #[test]
    fn display_only_block_usa_is_subtracted() {
        // §H.8 p163: USA is implicit originator and never appears in
        // DO axis.
        let portions = [
            portion_with_display_only(Classification::Secret, &["USA", "GBR"]),
            portion_with_display_only(Classification::Secret, &["USA", "GBR"]),
        ];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), false);
        let codes = b.to_vec();
        let usa = CountryCode::try_new(b"USA").unwrap();
        assert!(!codes.contains(&usa), "USA must NOT appear in DO");
    }

    #[test]
    fn display_only_block_trigraphs_sort_before_tetragraphs() {
        // §H.8 p164: trigraphs before tetragraphs, alphabetical within
        // each bucket.
        let portions = [
            portion_with_display_only(Classification::Secret, &["GBR", "NATO"]),
            portion_with_display_only(Classification::Secret, &["GBR", "NATO"]),
        ];
        let b = DisplayOnlyBlock::from_attrs_iter(&portions, &RelToBlock::empty(), false);
        let codes = b.to_vec();
        // GBR (trigraph, 3 chars) must come before NATO (tetragraph, 4 chars).
        assert!(codes.len() >= 2);
        assert_eq!(codes[0].as_str().len(), 3, "trigraph must sort first");
    }

    // Lattice-law tests for DisplayOnlyBlock::join

    #[test]
    fn display_only_block_join_associative() {
        let a = DisplayOnlyBlock::Lattice {
            countries: [
                CountryCode::try_new(b"GBR").unwrap(),
                CountryCode::try_new(b"CAN").unwrap(),
            ]
            .iter()
            .copied()
            .collect(),
        };
        let b = DisplayOnlyBlock::Lattice {
            countries: [
                CountryCode::try_new(b"GBR").unwrap(),
                CountryCode::try_new(b"AUS").unwrap(),
            ]
            .iter()
            .copied()
            .collect(),
        };
        let c = DisplayOnlyBlock::Lattice {
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        // (a.join(b)).join(c) == a.join(b.join(c))
        let left = a.join(&b).join(&c);
        let right = a.join(&b.join(&c));
        assert_eq!(left, right, "join must be associative");
    }

    #[test]
    fn display_only_block_join_identity_with_bottom() {
        let lat = DisplayOnlyBlock::Lattice {
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let bot = DisplayOnlyBlock::Bottom;
        assert_eq!(lat.join(&bot), lat);
        assert_eq!(bot.join(&lat), lat);
    }

    #[test]
    fn display_only_block_join_empty_absorbs() {
        // Empty absorbs Lattice and Bottom (but not NofornSuperseded).
        let lat = DisplayOnlyBlock::Lattice {
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let empty = DisplayOnlyBlock::Empty;
        assert_eq!(empty.join(&lat), DisplayOnlyBlock::Empty);
        assert_eq!(lat.join(&empty), DisplayOnlyBlock::Empty);
    }

    #[test]
    fn display_only_block_join_noforn_supersedes_all() {
        let lat = DisplayOnlyBlock::Lattice {
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let nofn = DisplayOnlyBlock::NofornSuperseded;
        assert_eq!(nofn.join(&lat), DisplayOnlyBlock::NofornSuperseded);
        assert_eq!(lat.join(&nofn), DisplayOnlyBlock::NofornSuperseded);
        assert_eq!(
            DisplayOnlyBlock::Empty.join(&nofn),
            DisplayOnlyBlock::NofornSuperseded
        );
    }
}
