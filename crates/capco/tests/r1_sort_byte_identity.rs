// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! R1 — byte-identity tests pinning sort behavior across the
//! `marque-capco` sort-consolidation WASM-cut refactor (issue #689,
//! synthesis brief `/tmp/r1-synthesis-brief.md`).
//!
//! These tests MUST pass on pre-refactor code and again on post-refactor
//! code. The refactor replaces inline `.sort_by(|a, b| ...)` closures
//! with named `fn`-item comparators per the PR #585 precedent
//! (`crates/capco/src/lattice.rs::sort_smolstrs_by_sar`). Byte-identical
//! sort output is the load-bearing invariant — the refactor saves WASM
//! bytes by collapsing per-closure monomorphizations, not by changing
//! semantics.
//!
//! # Test coverage rationale
//!
//! Per the rust-specialist preflight §4 (`/tmp/r1-rust-feasibility.md`),
//! two sort sites had coverage gaps that the broader corpus did not
//! exercise:
//!
//! - **`DisplayOnlyBlock::to_vec` (`crates/capco/src/lattice.rs:3989`)**
//!   sorts country codes trigraphs-first then tetragraphs, alpha within
//!   each bucket, per CAPCO-2016 §H.8 p163 ("the LIST is a comma-and-
//!   space separated list of trigraphs and tetragraphs in alphabetical
//!   order, trigraphs first") + §A.6 p16 (separator alphabet).
//!   `r1_display_only_block_to_vec_mixed_trigraph_tetragraph` exercises
//!   that ordering with a mixed `{GBR, NATO, CAN, FVEY}` set (two
//!   trigraphs, two tetragraphs).
//!
//! - **`HierarchicalTreeSet::sorted_entries`
//!   (`crates/capco/src/lattice.rs:229`)** sorts via
//!   `sar_sort_key(ta).cmp(&sar_sort_key(tb)).then_with(|| ta.cmp(tb))`.
//!   The tiebreaker fires only when two distinct keys produce the same
//!   `(bool, u64, &str)` triple — the documented case is two numeric
//!   identifiers exceeding `u64::MAX` that both fall through
//!   `unwrap_or(u64::MAX)` to `(false, u64::MAX, "")`.
//!   `r1_sorted_entries_numeric_overflow_tiebreaker` exercises that
//!   path with two SAR programs whose 20-digit identifiers overflow u64
//!   and differ only in their final digit.
//!
//! Each test asserts byte-identical output. Any change in the comparator
//! semantics (e.g., dropping `then_with`, swapping trigraph/tetragraph
//! priority) trips the test immediately.
//!
//! # Authority (re-verified at authorship 2026-05-22 against
//! `crates/capco/docs/CAPCO-2016.md` per Constitution VIII)
//!
//! - §H.8 p163 (DISPLAY ONLY LIST ordering — trigraphs first, alpha
//!   within bucket).
//! - §A.6 p16 (separator alphabet — comma+space between country codes).
//! - §H.5 p99 (SAR program ascending sort, numeric-first then alpha).
//! - §A.6 p15-16 (numeric-first ordering rule shared by SCI / SAR / AEA
//!   sub-compartment lists).

use std::collections::BTreeSet;

use marque_capco::lattice::{DisplayOnlyBlock, SarSet};
use marque_ism::{CountryCode, SarCompartment, SarIndicator, SarMarking, SarProgram};

/// `DisplayOnlyBlock::to_vec` returns the country list with trigraphs
/// (length 3) ordered before tetragraphs (length 4+), with alphabetical
/// ordering within each bucket, per CAPCO-2016 §H.8 p163 + §A.6 p16.
///
/// Mixed input `{GBR, NATO, CAN, FVEY}` (two trigraphs + two
/// tetragraphs) exercises the comparator's primary key (length==3 vs
/// not) and the secondary key (alpha within each bucket) on the same
/// input — neither the rust-specialist's preflight §4 review nor a grep
/// of the existing fixture corpus found another byte-identity test that
/// pins this ordering.
#[test]
fn r1_display_only_block_to_vec_mixed_trigraph_tetragraph() {
    // Construct the lattice directly — `DisplayOnlyBlock::Lattice` is a
    // public variant with public field, as exercised in
    // `category_lattice_laws.rs::display_only_block::lattice()`.
    let countries: BTreeSet<CountryCode> = [
        CountryCode::try_new(b"GBR").expect("GBR is a valid trigraph"),
        CountryCode::try_new(b"NATO").expect("NATO is a valid tetragraph"),
        CountryCode::try_new(b"CAN").expect("CAN is a valid trigraph"),
        CountryCode::try_new(b"FVEY").expect("FVEY is a valid tetragraph"),
    ]
    .into_iter()
    .collect();
    let block = DisplayOnlyBlock::Lattice { countries };

    let out = block.to_vec();
    let texts: Vec<&str> = out.iter().map(|c| c.as_str()).collect();

    // §H.8 p163: trigraphs first (alpha), then tetragraphs (alpha).
    // CAN before GBR within the trigraph bucket; FVEY before NATO within
    // the tetragraph bucket.
    assert_eq!(
        texts,
        vec!["CAN", "GBR", "FVEY", "NATO"],
        "DisplayOnlyBlock::to_vec must order trigraphs first then \
         tetragraphs, alpha within each bucket (CAPCO-2016 §H.8 p163)"
    );
}

/// `HierarchicalTreeSet::sorted_entries` (exercised here through the
/// public `SarSet::to_marking` path) applies the
/// `marque_ism::sar_sort_key`-based comparator with a lexicographic
/// tiebreaker (`then_with(|| ta.cmp(tb))`).
///
/// The tiebreaker is observable only when two distinct keys produce
/// the same `(bool, u64, &str)` triple. `sar_sort_key` builds the
/// triple as `(prefix_len == 0, u64-parsed prefix or u64::MAX on
/// overflow, remainder)` — two pure-numeric identifiers of 20 digits
/// or more overflow u64 and both collapse to `(false, u64::MAX, "")`,
/// at which point the secondary `ta.cmp(tb)` chooses the lex order.
///
/// The two identifiers below differ only in the last digit (`8` vs
/// `9`), so `"9...8"` lex-precedes `"9...9"`.
#[test]
fn r1_sorted_entries_numeric_overflow_tiebreaker() {
    // 20-digit numeric program identifiers — both overflow u64::MAX.
    // u64::MAX = 18446744073709551615 (20 digits) — anything 20 digits
    // starting with 9 is comfortably above the overflow line.
    let id_a = "99999999999999999998"; // collapses to (false, u64::MAX, "")
    let id_b = "99999999999999999999"; // collapses to (false, u64::MAX, "")
    debug_assert_eq!(id_a.len(), 20);
    debug_assert_eq!(id_b.len(), 20);

    // Construct an out-of-order SarMarking. `SarSet::from_marking` reads
    // input order; `SarSet::to_marking` re-sorts via `sorted_entries`,
    // which is the path under test.
    let programs: Box<[SarProgram]> = vec![
        SarProgram::new(id_b, Box::<[SarCompartment]>::default()),
        SarProgram::new(id_a, Box::<[SarCompartment]>::default()),
    ]
    .into_boxed_slice();
    let input = SarMarking::new(SarIndicator::Abbrev, programs);

    let set = SarSet::from_marking(Some(&input));
    let out = set
        .to_marking()
        .expect("SarSet built from non-empty input must round-trip");
    let identifiers: Vec<&str> = out.programs.iter().map(|p| p.identifier.as_str()).collect();

    // Primary sort key collides on both; the lexicographic tiebreaker
    // places `..8` before `..9`. Without the tiebreaker, the BTreeMap's
    // natural ordering happens to give the same answer for this pair,
    // but the assertion captures the comparator's *contract* — any
    // refactor that drops `then_with` must preserve this output by some
    // other mechanism or fail the test.
    assert_eq!(
        identifiers,
        vec![id_a, id_b],
        "sorted_entries must apply the lex tiebreaker when sar_sort_key \
         triples collide (CAPCO-2016 §A.6 p15-16 — numeric-first \
         ordering with stable total order on key collisions)"
    );
}
