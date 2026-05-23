// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`RelToBlock`] — IntersectSet with NOFORN supersession.

use marque_ism::{CanonicalAttrs, CountryCode, DissemControl, NonIcDissem};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
use smallvec::SmallVec;
use std::collections::BTreeSet;

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

    /// Strip the REL TO block when an external NOFORN dominator
    /// is observed on the dissem axis (§H.8 p145).
    ///
    /// Issue #704 (closure-monotonicity-via-supersession): the
    /// `CapcoScheme::closure` operator is purely additive
    /// post-#704 — Trio 3 (`capco:closure.nato.rel-to-usa-nato-if-nato-classification`)
    /// fires whenever the trigger atom (`NATO_CLASS`) is present
    /// even when NOFORN is also present in the input. The §H.8
    /// p145 NOFORN-dominates semantic that the previous
    /// `MASK_FDR_DOMINATORS` suppressor encoded moves here: when
    /// the caller observes NOFORN in the post-closure dissem
    /// axis, this overlay transitions the REL TO block to
    /// [`Self::NofornSuperseded`] so the writeback in
    /// `CapcoScheme::project` strips the closure-added USA / NATO
    /// (and any input-provided trigraphs) from `attrs.rel_to`.
    ///
    /// Behavior: if `noforn_present` is true, return
    /// `NofornSuperseded` unconditionally. Otherwise return
    /// `self` unchanged. The overlay is **idempotent**
    /// (`f(f(x)) == f(x)` because the function ignores `self`
    /// when `noforn_present` is true and is identity otherwise).
    /// It is **join-monotone** with the caller's `noforn_present`
    /// monotonically growing on `a ⊑ b`: if `noforn_present(a)`
    /// implies `noforn_present(b)` (which holds in the closure
    /// pipeline because NOFORN can only be added, never stripped,
    /// by `close`), then `f(a) ⊑ f(b)` because (a) if both true,
    /// both reach `NofornSuperseded` (equal, ⊑ holds); (b) if
    /// only b is true, f(a) is identity and f(b) is the
    /// `NofornSuperseded` absorbing top, and `x ⊑ NofornSuperseded`
    /// holds for every x via the join lattice's absorbing-element
    /// semantics.
    ///
    /// **Pure function.** Takes ownership and returns a new
    /// `RelToBlock`; no `&mut self`. Composes with `join` and
    /// `from_attrs_iter` without re-entrancy concerns.
    ///
    /// **Visibility (`pub(crate)`) + `#[allow(dead_code)]`.** Issue
    /// #704 review-cycle resolution Fix 3 downgraded from `pub` to
    /// `pub(crate)`. The production strip in
    /// `CapcoScheme::apply_supersession_overlays` clears
    /// `attrs.rel_to` directly without going through this typed
    /// method (operating at the `CanonicalAttrs` boundary where the
    /// lattice round-trip would be wasted work). The method stays
    /// as the typed surface for in-crate lattice-test fixtures +
    /// future refactors that operate on `RelToBlock` values
    /// directly; `pub(crate)` keeps the FR-049 stability freeze
    /// surface honest by not exporting an untested-in-production
    /// API. `#[allow(dead_code)]` is required because the
    /// `#[cfg(test)]` inline test module's calls are the only
    /// callers today — without the allow, the stable-clippy
    /// `dead_code` lint fires under `-D warnings`. The
    /// `#[cfg(test)]` alternative would forbid the future-refactor
    /// use case the method exists for.
    ///
    /// Authority: §H.8 p145 (NOFORN: "Cannot be used with REL TO,
    /// RELIDO, EYES ONLY, or DISPLAY ONLY"); §H.7 p127 (the
    /// `capco:closure.nato.rel-to-usa-nato-if-nato-classification`
    /// row's primary authority; this overlay resolves the §H.8
    /// p145 conflict between the §H.7 p127 implicit `REL TO USA,
    /// NATO` default and an explicit NOFORN).
    #[allow(dead_code)]
    pub(crate) fn with_nato_implicit_stripped(self, noforn_present: bool) -> Self {
        if noforn_present {
            Self::NofornSuperseded
        } else {
            self
        }
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
// `with_nato_implicit_stripped` unit tests (issue #704)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod with_nato_implicit_stripped_tests {
    use super::*;

    fn lattice_usa_gbr() -> RelToBlock {
        let mut countries = BTreeSet::new();
        countries.insert(CountryCode::USA);
        countries.insert(CountryCode::GBR);
        RelToBlock::Lattice { countries }
    }

    #[test]
    fn empty_input_with_no_noforn_returns_empty() {
        let stripped = RelToBlock::Bottom.with_nato_implicit_stripped(false);
        assert_eq!(stripped, RelToBlock::Bottom);
    }

    #[test]
    fn empty_input_with_noforn_returns_noforn_superseded() {
        let stripped = RelToBlock::Bottom.with_nato_implicit_stripped(true);
        assert_eq!(stripped, RelToBlock::NofornSuperseded);
    }

    #[test]
    fn lattice_with_noforn_present_strips_to_noforn_superseded() {
        // The §H.8 p145 conflict: REL TO USA, GBR cannot coexist
        // with NOFORN. Overlay forces NofornSuperseded.
        let stripped = lattice_usa_gbr().with_nato_implicit_stripped(true);
        assert_eq!(stripped, RelToBlock::NofornSuperseded);
    }

    #[test]
    fn lattice_with_no_noforn_is_kept() {
        // No external NOFORN observation → overlay is identity.
        let block = lattice_usa_gbr();
        let stripped = block.clone().with_nato_implicit_stripped(false);
        assert_eq!(stripped, block);
    }

    #[test]
    fn idempotent_no_noforn() {
        let block = lattice_usa_gbr();
        let once = block.clone().with_nato_implicit_stripped(false);
        let twice = once.clone().with_nato_implicit_stripped(false);
        assert_eq!(once, twice);
    }

    #[test]
    fn idempotent_with_noforn() {
        let once = lattice_usa_gbr().with_nato_implicit_stripped(true);
        let twice = once.clone().with_nato_implicit_stripped(true);
        assert_eq!(once, twice);
        assert_eq!(once, RelToBlock::NofornSuperseded);
    }

    /// Join-monotone given a monotone `noforn_present` indicator
    /// — if NOFORN-presence grows along the lattice order,
    /// `f(a) ⊑ f(b)` in `RelToBlock`'s join order
    /// (`Bottom < Lattice{..} < Empty < NofornSuperseded`).
    #[test]
    fn join_monotone_under_growing_noforn() {
        // a ⊑ b in input states; noforn(a)=false, noforn(b)=true.
        // f(a) = a (identity); f(b) = NofornSuperseded. The
        // join lattice satisfies `x ⊑ NofornSuperseded` for any
        // x, so monotonicity holds.
        let a = lattice_usa_gbr();
        let b = lattice_usa_gbr();
        let fa = a.with_nato_implicit_stripped(false);
        let fb = b.with_nato_implicit_stripped(true);
        // Witness: fa.join(fb) == fb (NofornSuperseded absorbs).
        assert_eq!(fa.join(&fb), RelToBlock::NofornSuperseded);
        // Also witness: fa.join(fb) == fb (fb is the joined
        // upper bound, so fa ⊑ fb).
        assert_eq!(fa.join(&fb), fb);
    }
}
