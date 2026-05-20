// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based tests for the bitmask Kleene closure operator.
//!
//! Covers the four algebraic properties from the PR-C plan section 6:
//!
//! - **P1 Idempotence** — `close(close(b)) == close(b)`.
//! - **P2 Extensivity** — `(close(b).bits() & b.bits()) == b.bits()` —
//!   every input bit survives.
//! - **P3 Monotonicity** — `a ⊑ b ⟹ close(a) ⊑ close(b)`. Equivalent
//!   form: `close(a | b) ⊒ close(a) | close(b)` for bitmasks under
//!   bitwise subset ordering.
//! - **P4 Convergence bound** — Kleene iteration count is bounded.
//!   The `close` function caps at [`MAX_CLOSURE_ITERATIONS`] = 16 in
//!   `crates/capco/src/scheme/closure_table.rs`; this proptest also
//!   asserts the loop converges in ≤ that bound by re-running the
//!   fixpoint outside `close` and counting iterations explicitly.
//!
//! Two generator strategies are used:
//!
//! - **`arb_full_u128`** — uniform over the full `u128` range. Robust:
//!   stresses every bit including the reserved-future-growth range
//!   (bits 51..128) which `close` should pass through unchanged
//!   because no row's trigger covers them.
//! - **`arb_realistic_bitmask`** — masked to `0..CAPCO_ATOM_COUNT`
//!   (51 atom bits). Domain-focused: every set bit corresponds to an
//!   actual CAPCO atom, so shrunk counterexamples are interpretable
//!   in terms of CAPCO markings.
//!
//! The PR-C plan section 6 reserves P5 (cross-path parity vs
//! `CapcoScheme::closure`) for PR-D, where it becomes the load-bearing
//! parity gate alongside the corpus regression harness.

use marque_capco::closure_table::{ALL_TRIGGER_MASK, CLOSURE_TABLE, MAX_CLOSURE_ITERATIONS, close};
use marque_capco::fact_bitmask::CAPCO_ATOM_COUNT;
use marque_scheme::FactBitmask;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Generator strategies
// ---------------------------------------------------------------------------

/// Uniform over the full `u128` range. Stresses reserved-future-growth
/// bits + every combination of trigger / suppressor / cone atoms.
fn arb_full_u128() -> impl Strategy<Value = FactBitmask> {
    any::<u128>().prop_map(FactBitmask::from_bits)
}

/// Masked to the active CAPCO atom inventory (bits `0..CAPCO_ATOM_COUNT`).
/// The mask is derived from [`CAPCO_ATOM_COUNT`] directly so a future
/// atom-inventory bump (51 → 52, etc.) automatically widens the mask
/// — closes the staleness gap flagged in PR-C review.
fn arb_realistic_bitmask() -> impl Strategy<Value = FactBitmask> {
    // Inventory mask = bits `0..CAPCO_ATOM_COUNT` set. Every set bit
    // corresponds to a real atom on `CanonicalAttrs`. The static-assert
    // below mirrors the `fact_bitmask::CAPCO_ATOM_COUNT` source-of-
    // truth guard (`<=`, since `CAPCO_ATOM_COUNT == FACT_BITMASK_WIDTH`
    // = 128 is a valid maximally-utilized inventory). The `if`
    // special-cases that ceiling so the `1u128 << 128` shift doesn't
    // overflow when the inventory fully saturates the primitive.
    const INVENTORY_MASK: u128 = if CAPCO_ATOM_COUNT == marque_scheme::FACT_BITMASK_WIDTH {
        u128::MAX
    } else {
        (1u128 << CAPCO_ATOM_COUNT) - 1
    };
    const _: () = assert!(
        CAPCO_ATOM_COUNT <= marque_scheme::FACT_BITMASK_WIDTH,
        "CAPCO_ATOM_COUNT must fit in FactBitmask::WIDTH",
    );
    any::<u128>().prop_map(|raw| FactBitmask::from_bits(raw & INVENTORY_MASK))
}

// ---------------------------------------------------------------------------
// Manual Kleene-iteration counter (P4 convergence-bound assertion)
// ---------------------------------------------------------------------------

/// Standalone Kleene-loop driver used by P4. Returns
/// `(fixpoint, iteration_count)` so the proptest can assert the
/// iteration count is bounded. Mirrors `close`'s body exactly so a
/// drift would surface as a divergence.
fn close_and_count(input: FactBitmask) -> (FactBitmask, usize) {
    let mut bits = input.bits();
    for iter in 0..MAX_CLOSURE_ITERATIONS {
        let mut next = bits;
        for row in CLOSURE_TABLE {
            let trigger_hit = (next & row.trigger_mask) != 0;
            let suppressed = row.suppressor_mask != 0 && (next & row.suppressor_mask) != 0;
            if trigger_hit && !suppressed {
                next |= row.cone_mask;
            }
        }
        if next == bits {
            return (FactBitmask::from_bits(bits), iter + 1);
        }
        bits = next;
    }
    panic!(
        "close_and_count: did not converge within {MAX_CLOSURE_ITERATIONS} \
         iterations on input {input:?} — bound is too low or close() \
         has a non-monotone row.",
    );
}

// ---------------------------------------------------------------------------
// P1 — Idempotence
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn p1_idempotence_full_range(bits in arb_full_u128()) {
        let once = close(bits);
        let twice = close(once);
        prop_assert_eq!(once, twice, "close is not idempotent on input {:?}", bits);
    }

    #[test]
    fn p1_idempotence_realistic(bits in arb_realistic_bitmask()) {
        let once = close(bits);
        let twice = close(once);
        prop_assert_eq!(once, twice);
    }
}

// ---------------------------------------------------------------------------
// P2 — Extensivity
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn p2_extensivity_full_range(bits in arb_full_u128()) {
        let closed = close(bits);
        prop_assert_eq!(
            closed.bits() & bits.bits(),
            bits.bits(),
            "close stripped a bit from input {:?}: closed = {:?}",
            bits,
            closed,
        );
    }

    #[test]
    fn p2_extensivity_realistic(bits in arb_realistic_bitmask()) {
        let closed = close(bits);
        prop_assert_eq!(closed.bits() & bits.bits(), bits.bits());
    }
}

// ---------------------------------------------------------------------------
// P3 — Monotonicity
// ---------------------------------------------------------------------------

proptest! {
    /// `a ⊑ b ⟹ close(a) ⊑ close(b)`. Generates two bitmasks `a` and
    /// `b_extra` and forms `b = a | b_extra` so `a ⊑ b` holds by
    /// construction. Asserts `close(a) ⊑ close(b)` in the bitwise
    /// subset order.
    #[test]
    fn p3_monotonicity_full_range(a in arb_full_u128(), b_extra in arb_full_u128()) {
        let b = FactBitmask::from_bits(a.bits() | b_extra.bits());
        prop_assert!(a.is_subset_of(b.bits()));
        let closed_a = close(a);
        let closed_b = close(b);
        prop_assert!(
            closed_a.is_subset_of(closed_b.bits()),
            "close is not monotone: a={a:?} ⊑ b={b:?} but \
             close(a)={closed_a:?} is NOT ⊑ close(b)={closed_b:?}",
        );
    }

    #[test]
    fn p3_monotonicity_realistic(a in arb_realistic_bitmask(), b_extra in arb_realistic_bitmask()) {
        let b = FactBitmask::from_bits(a.bits() | b_extra.bits());
        let closed_a = close(a);
        let closed_b = close(b);
        prop_assert!(closed_a.is_subset_of(closed_b.bits()));
    }
}

// ---------------------------------------------------------------------------
// P4 — Convergence bound
// ---------------------------------------------------------------------------

proptest! {
    /// Kleene iteration count ≤ [`MAX_CLOSURE_ITERATIONS`] (= 16) on
    /// every input. The CAPCO catalog's longest causal chain is depth
    /// 2; we expect typical inputs to converge in 1-3 iterations. The
    /// 16-bound is a generous ceiling that the loop never approaches
    /// for realistic inputs.
    ///
    /// Drift guard: the proptest also asserts
    /// `close_and_count` returns the same fixpoint as `close`. The
    /// local driver mirrors `close`'s body — silently diverging would
    /// invalidate P4 — and this cross-check catches that drift.
    #[test]
    fn p4_convergence_bound_full_range(bits in arb_full_u128()) {
        let (fixpoint, iter_count) = close_and_count(bits);
        prop_assert!(
            iter_count <= MAX_CLOSURE_ITERATIONS,
            "convergence took {} iterations (bound = {}) on input {:?}",
            iter_count,
            MAX_CLOSURE_ITERATIONS,
            bits,
        );
        prop_assert_eq!(
            fixpoint,
            close(bits),
            "close_and_count diverged from close — local driver \
             drifted out of sync with `close()`'s body",
        );
    }

    #[test]
    fn p4_convergence_bound_realistic(bits in arb_realistic_bitmask()) {
        let (fixpoint, iter_count) = close_and_count(bits);
        prop_assert!(iter_count <= MAX_CLOSURE_ITERATIONS);
        prop_assert_eq!(fixpoint, close(bits));
    }
}

// ---------------------------------------------------------------------------
// Auxiliary properties
// ---------------------------------------------------------------------------

proptest! {
    /// HOT-1 invariant — if no trigger bit is set, the fixpoint MUST
    /// equal the input. Gates PR-D's early-exit short-circuit at the
    /// production call site.
    #[test]
    fn hot1_no_trigger_means_no_change(bits in arb_full_u128()) {
        if (bits.bits() & ALL_TRIGGER_MASK) == 0 {
            prop_assert_eq!(close(bits), bits);
        }
    }

    /// `close` only flips bits inside the table's union of cone masks.
    /// The reserved-future-growth bits (51..128) MUST pass through
    /// unchanged.
    #[test]
    fn close_only_touches_cone_atoms(bits in arb_full_u128()) {
        let all_cones: u128 = CLOSURE_TABLE.iter().fold(0, |acc, r| acc | r.cone_mask);
        let closed = close(bits);
        let delta = closed.bits() ^ bits.bits();
        // Every bit in the delta must be a cone bit. (The fixpoint can
        // only ADD bits per P2, so `delta` is the set of cone bits
        // added during the Kleene loop.)
        prop_assert_eq!(
            delta & !all_cones,
            0,
            "close introduced a bit outside the union of cone masks: \
             input = {:?}, closed = {:?}, delta = {:#034x}",
            bits, closed, delta,
        );
    }
}
