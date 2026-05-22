// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`DeclassExemptionAccumulator`] — last-observed declass exemption.

use marque_ism::CanonicalAttrs;

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
/// [`super::non_ic_dissem::NonIcDissemSet`]'s precedent: drop the
/// `JoinSemilattice` impl, keep the type as a projection accumulator
/// surfacing `from_attrs_iter` + `into_inner` / `as_inner`. The rename
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::DeclassExemption;

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
}
