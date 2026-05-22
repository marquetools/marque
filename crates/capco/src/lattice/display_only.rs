// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`DisplayOnlyBlock`] — lattice over the DISPLAY ONLY axis
//! (cross-axis intersection over (REL TO ∪ DO), with banner-REL-TO
//! and USA subtraction).

use marque_ism::{CanonicalAttrs, CountryCode, DissemControl};
use marque_scheme::JoinSemilattice;
use smallvec::SmallVec;
use std::collections::BTreeSet;

use super::helpers::cmp_country_code_trigraph_first;
use super::rel_to::RelToBlock;

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
///    opaque codes; alphabetical within each bucket per §H.8 p163.
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
    /// Sorted trigraphs-first then alphabetical per §H.8 p163.
    Lattice {
        /// BTreeSet for deterministic ordering; render via
        /// `into_boxed_slice` for the §H.8 p163 sort.
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
    /// each bucket per §H.8 p163.
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
                // Named `fn`-item comparator
                // (`super::helpers::cmp_country_code_trigraph_first`) for
                // reviewability + pattern consistency with the other R1
                // helpers (issue #689 / PR #585 precedent at
                // `super::helpers::sort_smolstrs_by_sar`) — see the
                // function-level doc-comment for why this single-callsite
                // extraction is 1 → 1 on monomorphizations (no WASM saving)
                // and what the actual justification is. §H.8 p163 +
                // §A.6 p16 ordering (trigraphs first, tetragraphs after,
                // alpha within bucket).
                codes.sort_by(cmp_country_code_trigraph_first);
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
mod tests {
    use super::*;
    use crate::lattice::test_support::*;
    use marque_ism::Classification;

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
        assert!(
            !codes.contains(&CountryCode::USA),
            "USA must NOT appear in DO"
        );
    }

    #[test]
    fn display_only_block_trigraphs_sort_before_tetragraphs() {
        // §H.8 p163: trigraphs before tetragraphs, alphabetical within
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
