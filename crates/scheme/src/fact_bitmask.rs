// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Packed Boolean characteristic-vector over a closed-vocab atom set.
//!
//! Lives in `marque-scheme` (the foundation leaf) as a domain-neutral
//! primitive. A marking system's closed-vocab fact axes form an FCA-
//! shaped lattice (Birkhoff representation, `pure-lattice.md` §6 + §8);
//! a bitmask over the atom set is the natural compact storage for
//! join, meet, and closure operations against that lattice fragment.
//!
//! [`FactBitmask`] itself is content-free — it carries no atom names
//! or semantics. Concrete schemes (CAPCO today; a future CUI / NATO /
//! partner-national crate) own their atom inventory and bit
//! assignments in their domain crate. This crate publishes only the
//! storage primitive and the bit-manipulation API.
//!
//! Constitution VII positions this primitive alongside
//! [`crate::lattice`], [`crate::constraint`], and [`crate::builtins`]
//! — all domain-neutral trait surfaces and lattice constructors.
//! `marque-ism` and `marque-capco` consume this primitive; neither
//! defines it.
//!
//! See `docs/plans/2026-05-20-371-factbitmask-refactor.md` §2 for the
//! placement rationale (PM disposition OQ-1, 2026-05-20).
//!
//! # Capacity
//!
//! [`WIDTH`] is 128 bits. CAPCO's atom inventory uses ~51 bits today;
//! the remaining 77 bits split between CAPCO future growth (~45 bits
//! at 51–95) and foreign-grammar future use (32 bits at 96–127).
//! Schemes whose atom inventory exceeds 128 bits will need a wider
//! primitive (e.g., `[u64; 4]`); the bitmask abstraction generalizes
//! cleanly to that shape, but landing it requires a new module.
//!
//! # Lattice semantics
//!
//! Bit subset is the partial order: `a ⊑ b` iff every set bit in `a`
//! is also set in `b`. See [`FactBitmask::is_subset_of`]. The derived
//! [`PartialOrd`] / [`Ord`] traits implement *lexicographic* `u128`
//! ordering, **not** the lattice partial order — they're present for
//! `BTreeMap`/`Hash` ergonomics only. Always use `is_subset_of` for
//! lattice comparisons.

use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// Number of bits in a [`FactBitmask`].
pub const WIDTH: u32 = 128;

/// Packed Boolean characteristic-vector over a closed-vocab atom set.
///
/// A newtype wrapper around `u128`. The wrapping is `#[repr(transparent)]`
/// so the runtime representation is identical to `u128`; the wrapping
/// exists only to prevent accidental cross-domain mixing at the type
/// level (an arbitrary `u128` cannot be passed where a `FactBitmask`
/// is expected without an explicit construction).
///
/// All inherent methods are `const fn`, allowing the type to appear in
/// `const`-evaluated table initializers (e.g., a scheme's static
/// closure-table row entries).
#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FactBitmask(u128);

impl FactBitmask {
    /// The all-zeros bitmask — the lattice bottom.
    pub const EMPTY: Self = Self(0);

    /// Construct a [`FactBitmask`] from an explicit `u128`.
    ///
    /// This is the inverse of [`FactBitmask::bits`]. Domain crates use
    /// it sparingly (typically only in tests and bench drivers); the
    /// preferred construction is `EMPTY.with_bit(...)` chained.
    #[inline]
    #[must_use]
    pub const fn from_bits(bits: u128) -> Self {
        Self(bits)
    }

    /// Extract the underlying `u128`.
    ///
    /// Returned for diagnostic display, serialization, and proptest
    /// shrinking. Domain crates that need to compose multiple masks
    /// (e.g., `MASK_X | MASK_Y`) should use the [`BitOr`] impl on
    /// `FactBitmask` itself rather than unwrapping.
    #[inline]
    #[must_use]
    pub const fn bits(self) -> u128 {
        self.0
    }

    /// Returns `true` if the bit at position `bit` is set.
    ///
    /// In debug builds, panics if `bit >= WIDTH`. In release builds,
    /// Rust masks the shift amount to `bit % WIDTH` — so an
    /// out-of-range index silently wraps (e.g., `is_set(128)`
    /// returns the state of bit 0). Out-of-range indices are a
    /// caller-contract violation, not a library invariant: domain
    /// crates MUST enforce `bit < WIDTH` at their atom-layout
    /// boundary via a `static_assert!` over their atom count. The
    /// `debug_assert!` here is the development-time guard; the
    /// static-assert in the consumer is the production guard.
    #[inline]
    pub const fn is_set(self, bit: u32) -> bool {
        debug_assert!(bit < WIDTH, "FactBitmask::is_set: bit index out of range");
        ((self.0 >> bit) & 1) != 0
    }

    /// Returns a new [`FactBitmask`] with the bit at position `bit` set.
    ///
    /// Existing bits are preserved. In debug builds, panics if `bit
    /// >= WIDTH`. Release-build semantics for out-of-range `bit` are
    /// the same as [`is_set`](Self::is_set) — Rust masks the shift
    /// amount to `bit % WIDTH`. Same caller-contract: domain crates
    /// MUST `static_assert!` their atom count fits.
    #[inline]
    #[must_use]
    pub const fn with_bit(self, bit: u32) -> Self {
        debug_assert!(bit < WIDTH, "FactBitmask::with_bit: bit index out of range");
        Self(self.0 | (1u128 << bit))
    }

    /// Returns a new [`FactBitmask`] with all bits in `mask` set in
    /// addition to whatever was already set in `self`.
    #[inline]
    #[must_use]
    pub const fn with_bits(self, mask: u128) -> Self {
        Self(self.0 | mask)
    }

    /// Returns `true` if any bit in `mask` is also set in `self`.
    ///
    /// Equivalent to `(self & mask) != EMPTY`. Used for "any-of"
    /// trigger / suppressor checks in closure-table dispatch.
    #[inline]
    pub const fn intersects(self, mask: u128) -> bool {
        (self.0 & mask) != 0
    }

    /// Returns `true` if every set bit in `self` is also set in `mask`
    /// — i.e., `self ⊑ mask` in lattice terms.
    ///
    /// Equivalent to `(self & mask) == self`. Use this for lattice
    /// partial-order comparisons; the derived [`PartialOrd`] is
    /// `u128`-lexicographic, not subset.
    #[inline]
    pub const fn is_subset_of(self, mask: u128) -> bool {
        (self.0 & mask) == self.0
    }

    /// Returns `true` if no bits are set.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the number of bits set.
    ///
    /// Useful for diagnostic output and saturation checks against
    /// per-scheme atom-count budgets.
    #[inline]
    pub const fn count_ones(self) -> u32 {
        self.0.count_ones()
    }
}

impl core::fmt::Debug for FactBitmask {
    /// Hex form (16-byte u128 as two 64-bit halves).
    ///
    /// `marque-scheme` does not know the per-scheme atom names; the
    /// symbolic `Debug` form is the responsibility of the consuming
    /// domain crate, which can wrap [`FactBitmask`] in its own
    /// newtype or implement a separate display helper that walks a
    /// `FACT_BIT_NAMES` table. The hex form here is what shows up in
    /// proptest shrinking and panic messages by default.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FactBitmask(0x{:016x}_{:016x})",
            (self.0 >> 64) as u64,
            self.0 as u64
        )
    }
}

impl BitOr for FactBitmask {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for FactBitmask {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for FactBitmask {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for FactBitmask {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for FactBitmask {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for FactBitmask {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for FactBitmask {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_bits_set() {
        assert!(FactBitmask::EMPTY.is_empty());
        assert_eq!(FactBitmask::EMPTY.bits(), 0);
        assert_eq!(FactBitmask::EMPTY.count_ones(), 0);
    }

    #[test]
    fn default_is_empty() {
        assert_eq!(FactBitmask::default(), FactBitmask::EMPTY);
    }

    #[test]
    fn with_bit_then_is_set() {
        let m = FactBitmask::EMPTY.with_bit(0).with_bit(5).with_bit(127);
        assert!(m.is_set(0));
        assert!(m.is_set(5));
        assert!(m.is_set(127));
        assert!(!m.is_set(1));
        assert!(!m.is_set(64));
        assert_eq!(m.count_ones(), 3);
    }

    #[test]
    fn with_bit_is_idempotent() {
        let m1 = FactBitmask::EMPTY.with_bit(7);
        let m2 = m1.with_bit(7);
        assert_eq!(m1, m2);
    }

    #[test]
    fn with_bits_unions() {
        let m = FactBitmask::EMPTY.with_bits(0b1010).with_bits(0b0101);
        assert_eq!(m.bits(), 0b1111);
    }

    #[test]
    fn intersects_is_any_of() {
        let m = FactBitmask::EMPTY.with_bit(3).with_bit(10);
        assert!(m.intersects(1u128 << 3));
        assert!(m.intersects((1u128 << 3) | (1u128 << 10)));
        assert!(m.intersects((1u128 << 3) | (1u128 << 99)));
        assert!(!m.intersects(1u128 << 5));
        assert!(!m.intersects(0));
    }

    #[test]
    fn is_subset_of_lattice_order() {
        let a = FactBitmask::EMPTY.with_bit(3);
        let b = FactBitmask::EMPTY.with_bit(3).with_bit(10);
        assert!(a.is_subset_of(b.bits()));
        assert!(!b.is_subset_of(a.bits()));
        assert!(a.is_subset_of(a.bits())); // reflexive
        assert!(FactBitmask::EMPTY.is_subset_of(b.bits())); // bottom
    }

    #[test]
    fn bitor_unions() {
        let a = FactBitmask::EMPTY.with_bit(1);
        let b = FactBitmask::EMPTY.with_bit(2);
        let c = a | b;
        assert!(c.is_set(1));
        assert!(c.is_set(2));
        assert_eq!(c.count_ones(), 2);
    }

    #[test]
    fn bitor_assign() {
        let mut a = FactBitmask::EMPTY.with_bit(1);
        let b = FactBitmask::EMPTY.with_bit(2);
        a |= b;
        assert_eq!(a.count_ones(), 2);
    }

    #[test]
    fn bitand_intersects() {
        let a = FactBitmask::EMPTY.with_bit(1).with_bit(2);
        let b = FactBitmask::EMPTY.with_bit(2).with_bit(3);
        let c = a & b;
        assert_eq!(c, FactBitmask::EMPTY.with_bit(2));
    }

    #[test]
    fn bitxor_symmetric_difference() {
        let a = FactBitmask::EMPTY.with_bit(1).with_bit(2);
        let b = FactBitmask::EMPTY.with_bit(2).with_bit(3);
        let c = a ^ b;
        assert_eq!(c, FactBitmask::EMPTY.with_bit(1).with_bit(3));
    }

    #[test]
    fn bitxor_assign() {
        let mut a = FactBitmask::EMPTY.with_bit(1).with_bit(2);
        let b = FactBitmask::EMPTY.with_bit(2).with_bit(3);
        a ^= b;
        assert_eq!(a, FactBitmask::EMPTY.with_bit(1).with_bit(3));
    }

    #[test]
    fn not_inverts() {
        let a = FactBitmask::EMPTY.with_bit(0);
        let n = !a;
        assert!(!n.is_set(0));
        assert!(n.is_set(1));
        assert_eq!(n.count_ones(), 127);
    }

    #[test]
    fn from_bits_roundtrip() {
        let raw: u128 = 0xDEAD_BEEF_CAFE_BABE_0123_4567_89AB_CDEF;
        assert_eq!(FactBitmask::from_bits(raw).bits(), raw);
    }

    #[test]
    fn debug_format_is_hex() {
        let m = FactBitmask::from_bits(0x0000_0000_0000_0001_0000_0000_0000_0002);
        let s = format!("{m:?}");
        assert_eq!(s, "FactBitmask(0x0000000000000001_0000000000000002)");
    }

    #[test]
    fn const_fn_in_static() {
        // Verify that the API is genuinely const-fn — these `const`
        // initializers fail to compile if any inherent method
        // accidentally requires runtime evaluation. The `const { ... }`
        // assert blocks force the constant evaluation at compile time;
        // the runtime body just confirms the type-system gate ran.
        const M: FactBitmask = FactBitmask::EMPTY.with_bit(3).with_bit(7);
        const _: () = assert!(M.is_set(3));
        const _: () = assert!(!M.is_set(5));
        assert_eq!(M.count_ones(), 2);
    }

    #[test]
    fn width_constant() {
        assert_eq!(WIDTH, 128);
    }
}
