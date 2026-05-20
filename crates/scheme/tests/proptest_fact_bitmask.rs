// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property tests for [`marque_scheme::FactBitmask`].
//!
//! These verify the type-level invariants of the bitmask primitive:
//! [`is_subset_of`] is a partial order, [`intersects`] is any-of,
//! [`with_bit`] composes correctly with [`is_set`], and the bitwise
//! operator impls (`|`, `&`, `^`, `!`) compose to Boolean-algebra
//! identities. These are the contracts CAPCO's `derive_bits` /
//! `apply_closed_bits_to` projection (and the future `close()`
//! Kleene loop) rely on.
//!
//! Tests for closure-table laws (idempotence, extensivity,
//! monotonicity, convergence-bound) live in `marque-capco` — they
//! depend on the CAPCO atom inventory and `CLOSURE_TABLE`, which
//! are not exposed at the `marque-scheme` layer.

use marque_scheme::FactBitmask;
use proptest::prelude::*;

/// Generate an arbitrary `FactBitmask` (full `u128` value range).
fn arb_bitmask() -> impl Strategy<Value = FactBitmask> {
    any::<u128>().prop_map(FactBitmask::from_bits)
}

/// Generate an arbitrary bit index in `0..128`.
fn arb_bit_index() -> impl Strategy<Value = u32> {
    0u32..128u32
}

proptest! {
    // ------------------------------------------------------------------
    // `with_bit` + `is_set` composition
    // ------------------------------------------------------------------

    /// After `with_bit(b)`, `is_set(b)` returns true.
    #[test]
    fn with_bit_then_is_set_true(bits in arb_bitmask(), b in arb_bit_index()) {
        let m = bits.with_bit(b);
        prop_assert!(m.is_set(b));
    }

    /// `with_bit(b)` does not clear other bits.
    #[test]
    fn with_bit_preserves_other_bits(
        bits in arb_bitmask(),
        b in arb_bit_index(),
    ) {
        let m = bits.with_bit(b);
        // every bit originally set in `bits` must still be set in `m`
        prop_assert!(bits.is_subset_of(m.bits()));
    }

    /// `with_bit` is idempotent.
    #[test]
    fn with_bit_idempotent(bits in arb_bitmask(), b in arb_bit_index()) {
        prop_assert_eq!(bits.with_bit(b).with_bit(b), bits.with_bit(b));
    }

    // ------------------------------------------------------------------
    // `intersects` is any-of
    // ------------------------------------------------------------------

    /// `m.intersects(mask)` ↔ `(m & mask).bits() != 0`.
    #[test]
    fn intersects_matches_bitwise_and(m in arb_bitmask(), mask in any::<u128>()) {
        let intersects = m.intersects(mask);
        let via_and = (m & FactBitmask::from_bits(mask)).bits() != 0;
        prop_assert_eq!(intersects, via_and);
    }

    /// No bitmask intersects EMPTY.
    #[test]
    fn nothing_intersects_empty(m in arb_bitmask()) {
        prop_assert!(!m.intersects(0));
    }

    /// A bitmask always intersects itself unless empty.
    #[test]
    fn nonempty_intersects_self(m in arb_bitmask()) {
        prop_assert_eq!(m.intersects(m.bits()), !m.is_empty());
    }

    // ------------------------------------------------------------------
    // `is_subset_of` is a partial order
    // ------------------------------------------------------------------

    /// Reflexivity: `m ⊑ m`.
    #[test]
    fn subset_reflexive(m in arb_bitmask()) {
        prop_assert!(m.is_subset_of(m.bits()));
    }

    /// EMPTY is the bottom: `EMPTY ⊑ m`.
    #[test]
    fn empty_is_bottom(m in arb_bitmask()) {
        prop_assert!(FactBitmask::EMPTY.is_subset_of(m.bits()));
    }

    /// Antisymmetry: `a ⊑ b ∧ b ⊑ a ⟹ a == b`.
    #[test]
    fn subset_antisymmetric(a in arb_bitmask(), b in arb_bitmask()) {
        if a.is_subset_of(b.bits()) && b.is_subset_of(a.bits()) {
            prop_assert_eq!(a, b);
        }
    }

    /// Transitivity: `a ⊑ b ∧ b ⊑ c ⟹ a ⊑ c`.
    #[test]
    fn subset_transitive(
        a in arb_bitmask(),
        b in arb_bitmask(),
        c in arb_bitmask(),
    ) {
        if a.is_subset_of(b.bits()) && b.is_subset_of(c.bits()) {
            prop_assert!(a.is_subset_of(c.bits()));
        }
    }

    // ------------------------------------------------------------------
    // Bitwise op identities (Boolean algebra)
    // ------------------------------------------------------------------

    /// `(a | b) ⊒ a` — bitor is upper bound.
    #[test]
    fn bitor_is_upper_bound(a in arb_bitmask(), b in arb_bitmask()) {
        let joined = a | b;
        prop_assert!(a.is_subset_of(joined.bits()));
        prop_assert!(b.is_subset_of(joined.bits()));
    }

    /// `(a & b) ⊑ a` — bitand is lower bound.
    #[test]
    fn bitand_is_lower_bound(a in arb_bitmask(), b in arb_bitmask()) {
        let met = a & b;
        prop_assert!(met.is_subset_of(a.bits()));
        prop_assert!(met.is_subset_of(b.bits()));
    }

    /// Commutativity of `|`.
    #[test]
    fn bitor_commutative(a in arb_bitmask(), b in arb_bitmask()) {
        prop_assert_eq!(a | b, b | a);
    }

    /// Commutativity of `&`.
    #[test]
    fn bitand_commutative(a in arb_bitmask(), b in arb_bitmask()) {
        prop_assert_eq!(a & b, b & a);
    }

    /// Associativity of `|`.
    #[test]
    fn bitor_associative(
        a in arb_bitmask(),
        b in arb_bitmask(),
        c in arb_bitmask(),
    ) {
        prop_assert_eq!((a | b) | c, a | (b | c));
    }

    /// Associativity of `&`.
    #[test]
    fn bitand_associative(
        a in arb_bitmask(),
        b in arb_bitmask(),
        c in arb_bitmask(),
    ) {
        prop_assert_eq!((a & b) & c, a & (b & c));
    }

    /// Identity: `m | EMPTY == m`.
    #[test]
    fn bitor_identity(m in arb_bitmask()) {
        prop_assert_eq!(m | FactBitmask::EMPTY, m);
    }

    /// Idempotence: `m | m == m`.
    #[test]
    fn bitor_idempotent(m in arb_bitmask()) {
        prop_assert_eq!(m | m, m);
    }

    /// Idempotence: `m & m == m`.
    #[test]
    fn bitand_idempotent(m in arb_bitmask()) {
        prop_assert_eq!(m & m, m);
    }

    /// De Morgan: `!(a | b) == !a & !b`.
    #[test]
    fn de_morgan_or(a in arb_bitmask(), b in arb_bitmask()) {
        prop_assert_eq!(!(a | b), !a & !b);
    }

    /// De Morgan: `!(a & b) == !a | !b`.
    #[test]
    fn de_morgan_and(a in arb_bitmask(), b in arb_bitmask()) {
        prop_assert_eq!(!(a & b), !a | !b);
    }

    /// Double negation: `!!m == m`.
    #[test]
    fn not_involution(m in arb_bitmask()) {
        prop_assert_eq!(!!m, m);
    }

    /// `m ^ m == EMPTY`.
    #[test]
    fn bitxor_self_is_empty(m in arb_bitmask()) {
        prop_assert_eq!(m ^ m, FactBitmask::EMPTY);
    }

    /// `m ^ EMPTY == m`.
    #[test]
    fn bitxor_empty_identity(m in arb_bitmask()) {
        prop_assert_eq!(m ^ FactBitmask::EMPTY, m);
    }

    /// `^=` agrees with `^`.
    #[test]
    fn bitxor_assign_matches_bitxor(a in arb_bitmask(), b in arb_bitmask()) {
        let mut working = a;
        working ^= b;
        prop_assert_eq!(working, a ^ b);
    }

    /// `|=` agrees with `|`.
    #[test]
    fn bitor_assign_matches_bitor(a in arb_bitmask(), b in arb_bitmask()) {
        let mut working = a;
        working |= b;
        prop_assert_eq!(working, a | b);
    }

    /// `&=` agrees with `&`.
    #[test]
    fn bitand_assign_matches_bitand(a in arb_bitmask(), b in arb_bitmask()) {
        let mut working = a;
        working &= b;
        prop_assert_eq!(working, a & b);
    }

    // ------------------------------------------------------------------
    // Round-trip
    // ------------------------------------------------------------------

    /// `from_bits` and `bits` round-trip.
    #[test]
    fn from_bits_roundtrip(raw in any::<u128>()) {
        prop_assert_eq!(FactBitmask::from_bits(raw).bits(), raw);
    }

    /// `count_ones` matches `u128::count_ones`.
    #[test]
    fn count_ones_matches_u128(raw in any::<u128>()) {
        prop_assert_eq!(FactBitmask::from_bits(raw).count_ones(), raw.count_ones());
    }
}
