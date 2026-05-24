// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared lattice infrastructure: the `HierarchicalTreeSet<K>` storage
//! primitive used by [`super::sci::SciSet`] and [`super::sar::SarSet`],
//! plus the [`sort_smolstrs_by_sar`] /
//! [`cmp_country_code_trigraph_first`] / [`sorted_compartment_items`]
//! comparators called from per-type submodules and (in
//! `render/*`'s doc-comments only) from the renderer family.
//!
//! Visibility is `pub(super)` throughout — these are crate-internal
//! lattice infrastructure, never part of the `marque_capco::lattice::*`
//! public surface.

use marque_ism::CountryCode;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet};

/// Sort a slice of `&SmolStr` by `marque_ism::sar_sort_key` (CAPCO §H.5 p99
/// numeric-first, alphabetic-after; also serves SCI §A.6 p15 which shares the
/// same numeric-then-alpha rule).
///
/// Single named site so the compiler emits exactly one
/// `slice::sort::stable::quicksort::quicksort<&SmolStr, _>` instantiation
/// regardless of how many call sites use it (issue #585). Each separate
/// `.sort_by(|a, b| sar_sort_key(a)…)` call site would otherwise emit a
/// distinct ~15.6 KiB monomorphization; consolidating the `&SmolStr` sites
/// + the tuple site (sorting keys-first) collapses them to one.
///
/// Deliberately NOT `#[inline]`: the closure here has a single anonymous type
/// (`sort_smolstrs_by_sar::{closure#0}`) regardless of inlining decisions, so
/// the mono guarantee holds either way. Omitting the hint avoids contradicting
/// the doc comment's "single named site" framing — `lto = "fat"` (workspace
/// `Cargo.toml`) handles whole-program inlining naturally if profitable.
pub(super) fn sort_smolstrs_by_sar(slice: &mut [&SmolStr]) {
    slice.sort_by(|a, b| marque_ism::sar_sort_key(a).cmp(&marque_ism::sar_sort_key(b)));
}

/// Compare two `&CountryCode` references with trigraphs (length 3)
/// before tetragraphs and any opaque longer codes, alphabetical within
/// each bucket — the CAPCO-2016 §H.8 p163 DISPLAY ONLY LIST ordering
/// (also §A.6 p16 separator alphabet).
///
/// Single callsite at present (`DisplayOnlyBlock::to_vec`): the
/// closure-axis mono "collapse" here is from 1 → 1, **so this
/// extraction does NOT save WASM bytes** — it is justified by
/// reviewability (sort semantic reviewable separately from the
/// cross-axis lattice machinery) and pattern consistency with the
/// other comparator helpers (issue #689 / PR #585 precedent).
pub(super) fn cmp_country_code_trigraph_first(
    a: &CountryCode,
    b: &CountryCode,
) -> std::cmp::Ordering {
    let a_is_trigraph = a.as_str().len() == 3;
    let b_is_trigraph = b.as_str().len() == 3;
    a_is_trigraph
        .cmp(&b_is_trigraph)
        .reverse()
        .then_with(|| a.as_str().cmp(b.as_str()))
}

// ---------------------------------------------------------------------------
// HierarchicalTreeSet — shared 3-level tree storage primitive
// ---------------------------------------------------------------------------

/// A 3-level hierarchical tree: `K → SmolStr → SmolStr`.
///
/// Backed by `BTreeMap<K, BTreeMap<SmolStr, BTreeSet<SmolStr>>>`, this
/// type captures the repeated "outer key → compartments → sub-compartments"
/// pattern shared by [`super::sci::SciSet`] (outer key = `SystemKey`)
/// and [`super::sar::SarSet`] (outer key = [`SmolStr`]). Both types
/// differ only in their outer key type; this generic struct lets them
/// share [`join_with`][Self::join_with], [`meet_with`][Self::meet_with],
/// and the sorted-traversal helper
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
pub(super) struct HierarchicalTreeSet<K: Clone + Ord> {
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
    pub(super) fn empty() -> Self {
        Self::default()
    }

    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a mutable reference to the compartment map for `key`,
    /// inserting an empty entry if absent. Used by `from_markings*`
    /// constructors to record bare (no-compartment) entries without a
    /// separate "ensure entry" step.
    #[inline]
    pub(super) fn entry_outer(&mut self, key: K) -> &mut BTreeMap<SmolStr, BTreeSet<SmolStr>> {
        self.inner.entry(key).or_default()
    }

    /// Returns the compartment map for `key`, or `None` if absent.
    #[inline]
    pub(super) fn get(&self, key: &K) -> Option<&BTreeMap<SmolStr, BTreeSet<SmolStr>>> {
        self.inner.get(key)
    }

    /// Returns `true` if `key` is present in the outer map.
    #[inline]
    pub(super) fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// Iterates over outer keys in `BTreeMap` order.
    #[inline]
    pub(super) fn keys(&self) -> impl Iterator<Item = &K> {
        self.inner.keys()
    }

    /// Iterates over `(outer_key, compartment_map)` pairs in `BTreeMap`
    /// order.
    #[inline]
    pub(super) fn iter(&self) -> impl Iterator<Item = (&K, &BTreeMap<SmolStr, BTreeSet<SmolStr>>)> {
        self.inner.iter()
    }

    /// Component-wise union: for each outer key in either operand, union
    /// its compartment map; within each compartment, union its
    /// sub-compartment set. Implements the `join` for both
    /// [`super::sci::SciSet`] and [`super::sar::SarSet`].
    #[inline]
    pub(super) fn join_with(&self, other: &Self) -> Self {
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
    /// the `meet` for both [`super::sci::SciSet`] and
    /// [`super::sar::SarSet`].
    #[inline]
    pub(super) fn meet_with(&self, other: &Self) -> Self {
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
    /// A final tie-breaker on the outer key `K` itself is applied after
    /// the text comparison. Distinct outer keys can project to the same
    /// `key_text` string — e.g. `SystemKey::Published(SciControlBare::Si)`
    /// and a user-constructed `SystemKey::Custom("SI")` both render as
    /// `"SI"`. Without the outer-key tie-breaker, the comparator returns
    /// `Equal` for two distinct entries and `slice::sort_by` (unstable)
    /// would order them non-deterministically across Rust versions. With
    /// it, the comparator is a strict total order on the entry-pair domain.
    ///
    /// Inline-4 covers the typical outer-key count (SCI: SI/TK/HCS/G;
    /// SAR: ≤4 programs per portion in ordinary documents) so the sorted
    /// scratch buffer stays on the stack on the hot path.
    #[allow(clippy::type_complexity)] // Inline-4 SmallVec is load-bearing; a type alias would hide the capacity.
    pub(super) fn sorted_entries(
        &self,
        key_text: impl Fn(&K) -> &str,
    ) -> SmallVec<[(&K, &BTreeMap<SmolStr, BTreeSet<SmolStr>>); 4]> {
        let mut entries: SmallVec<[_; 4]> = self.inner.iter().collect();
        // Keep this comparator closure inline (issue #689). Extracting
        // its body to a named `fn`-item regresses the WASM bundle by
        // ~6.2 KB on the `release-web` profile: LTO + ICF fold the two
        // `K`-instantiations of the inline closure (`K = SystemKey` for
        // `SciSet::to_markings`, `K = SmolStr` for `SarSet::to_marking`)
        // into one code-gen via byte-identity merging, but a named fn
        // introduces a call boundary that LLVM ICF cannot fold across,
        // leaving the `SystemKey` instance fully resident. Mono-collapse
        // works for the closure-axis where the slice element type repeats
        // across callsites (the `&str` collapse in `render/mod.rs`), but
        // per-`K` generic sites are best left to LTO.
        entries.sort_by(|a, b| {
            let ta = key_text(a.0);
            let tb = key_text(b.0);
            marque_ism::sar_sort_key(ta)
                .cmp(&marque_ism::sar_sort_key(tb))
                .then_with(|| ta.cmp(tb))
                .then_with(|| a.0.cmp(b.0))
        });
        entries
    }
}

/// Sort and render a compartment map into a list of
/// `(identifier, sorted-sub-compartments)` pairs. Shared rendering
/// helper for [`super::sci::SciSet::to_markings`] and
/// [`super::sar::SarSet::to_marking`].
///
/// Inline capacities:
/// - `comp_keys` scratch: inline-8 covers the typical SCI compartment
///   count (the `NF/PR/OC/REL/IMCON/RS` shape and similar) plus headroom;
///   SAR compartment counts are typically ≤4 per program but inline-8 is
///   a stack-only ceiling, no waste on the heap path.
/// - per-compartment `subs` scratch: inline-4 covers the typical
///   sub-compartment count for both SCI and SAR.
/// - returned vec: inline-8 matches the SCI `compartments` capacity for
///   the rendering path; callers consume via `into_iter` so the
///   inline storage is dropped at the end of the iteration without an
///   additional Box round-trip.
#[allow(clippy::type_complexity)] // Inline-8 SmallVec is load-bearing; a type alias would hide the capacity.
pub(super) fn sorted_compartment_items(
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
