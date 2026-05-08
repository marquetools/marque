// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #234 PR-B fixup — REL TO list-grammar invariant integration tests.
//!
//! These tests exercise the **engine-level** behavior of E002, E020, and
//! E052 when they fire on overlapping REL TO spans. The unit tests in
//! `crates/capco/src/rules.rs` only inspect the diagnostic each rule
//! emits in isolation — they cannot verify the FR-016 sort or C-1 overlap
//! guard interaction. That is what this file is for.
//!
//! Background (Copilot review on PR #241):
//!
//! Under `Engine::fix`'s C-1 overlap guard, two fixes that touch the same
//! span are deterministically resolved by FR-016 sort:
//! `(span.end DESC, span.start DESC, rule_id ASC, replacement ASC)`. For
//! REL TO list-grammar rules:
//!
//! - E002 (`missing-usa-trigraph`) and E052 (`rel-to-no-duplicates`) can
//!   fire on the same span when USA is missing AND a non-USA code is
//!   duplicated. Tiebreaker: E002 < E052 → E002 wins. E002's fix is
//!   canonical (USA-first + alpha + unique), so a single fix pass is
//!   idempotent.
//! - E060 (PR 3b.F walker, REL TO row — retired E020) and E052 can
//!   fire on the same span when REL TO is misordered AND a code is
//!   duplicated. Post-rename tiebreaker: `'E052' < 'E060'` lex (since
//!   `'5' < '6'`) → **E052 wins**. (Pre-rename: E020 < E052 → E020
//!   won and produced canonical output in one pass.)
//!
//! E052's fix is dedup-only (preserves first-occurrence order); when
//! E052 wins the overlap guard, the post-fix list is set-equal to
//! the canonical but may still be misordered. A SECOND fix pass
//! catches the misorder via E060 and produces the fully canonical
//! list — so the fixed point is reached in at most 2 passes
//! (idempotent thereafter). This is the post-PR-3b.F behavior shape.
//!
//! Pre-PR-B history (preserved for reviewers chasing audit trail):
//! both E002 and E020 (now E060) compose `dedup_country_codes` +
//! `canonicalize_trigraph_list` in their fix paths, so when E020
//! (or E002) wins the overlap guard the canonical form is reached
//! in one pass. E052 retains sole responsibility for duplicate
//! detection diagnostics; its fix is intentionally dedup-only
//! (preserving user-authored order when the list is otherwise
//! canonical modulo the duplicate).
//!
//! Authority: CAPCO-2016 §H.8 p150–151 (REL TO list grammar). Per
//! Constitution VIII, the no-duplicates property is structural — §H.8
//! describes a list of country codes a release applies to, which is
//! semantically a set, so duplicates are redundant by construction.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine_default() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Apply `Engine::fix` to `source` and return `(fixed_source, applied_rule_ids)`.
fn fix_once(source: &str) -> (String, Vec<String>) {
    let engine = engine_default();
    let result = engine.fix(source.as_bytes(), FixMode::Apply);
    let fixed = String::from_utf8(result.source).expect("output is valid UTF-8");
    let applied: Vec<String> = result
        .applied
        .iter()
        .map(|f| f.proposal.rule.as_str().to_owned())
        .collect();
    (fixed, applied)
}

// ---------------------------------------------------------------------------
// E060 (REL TO row, retired E020) + E052 overlap — fixed-point convergence
// ---------------------------------------------------------------------------

#[test]
fn e060_rel_to_row_and_e052_overlap_resolves_at_fixed_point() {
    // Misordered AND duplicated: GBR appears twice and the first GBR
    // is out of alphabetical order. Both E060 (REL TO row) and E052
    // fire on the same REL TO trigraph span. Post-PR-3b.F FR-016 lex
    // tiebreaker keeps E052 (`'E052' < 'E060'` because `'5' < '6'`).
    // E052's replacement is dedup-only `USA, GBR, AUS` — set-equal
    // to the canonical but still misordered after a single pass.
    //
    // Pre-PR-3b.F history: this test was named
    // `e020_and_e052_overlap_resolves_in_single_pass` and asserted
    // single-pass canonicalization (E020 won the tiebreaker and its
    // fix was canonical). Post-PR-3b.F E052 wins and the canonical
    // fixed point is reached on the second fix pass via E060.
    let source = "SECRET//REL TO USA, GBR, AUS, GBR\n";
    let (after_first, applied_first) = fix_once(source);

    assert_eq!(
        after_first, "SECRET//REL TO USA, GBR, AUS\n",
        "single pass with E052 winning produces a dedup-only list \
         (preserves first-occurrence order); canonical comes on the \
         second pass via E060 (REL TO row)"
    );
    // Exactly one of {E060, E052} survives the C-1 overlap guard.
    // Under post-PR-3b.F FR-016 it's E052.
    let rel_to_fixes: Vec<&String> = applied_first
        .iter()
        .filter(|r| matches!(r.as_str(), "E060" | "E052"))
        .collect();
    assert_eq!(
        rel_to_fixes.len(),
        1,
        "C-1 overlap guard must keep exactly one of E060/E052: {applied_first:?}"
    );
    // The second pass produces the fully canonical form via E060.
    let (after_second, applied_second) = fix_once(&after_first);
    assert_eq!(
        after_second, "SECRET//REL TO USA, AUS, GBR\n",
        "second pass via E060 (REL TO row) must produce the fully \
         canonical REL TO list"
    );
    assert!(
        applied_second.iter().any(|r| r == "E060"),
        "second pass must fire E060 on the misordered post-dedup list: \
         {applied_second:?}"
    );
}

#[test]
fn e060_rel_to_row_and_e052_overlap_is_idempotent_at_fixed_point() {
    // Fixed-point invariant: applying `fix` until convergence produces
    // a stable output. Post-PR-3b.F this takes 2 passes (E052 dedup
    // → E060 sort), then idempotent thereafter.
    let source = "SECRET//REL TO USA, GBR, AUS, GBR\n";
    let (after_first, _) = fix_once(source);
    let (after_second, _) = fix_once(&after_first);
    let (after_third, applied_third) = fix_once(&after_second);

    assert_eq!(
        after_second, after_third,
        "fix must reach a fixed-point in at most 2 passes: a third \
         fix pass produces no further changes"
    );
    let rel_to_fixes_in_third: Vec<&String> = applied_third
        .iter()
        .filter(|r| matches!(r.as_str(), "E002" | "E060" | "E052"))
        .collect();
    assert!(
        rel_to_fixes_in_third.is_empty(),
        "third pass must not apply any REL TO fixes: {applied_third:?}"
    );
}

// ---------------------------------------------------------------------------
// E002 + E052 overlap — missing USA + duplicated non-USA code
// ---------------------------------------------------------------------------

#[test]
fn e002_and_e052_overlap_resolves_in_single_pass() {
    // USA is missing AND GBR is duplicated. E002 fires (missing USA),
    // E052 fires (GBR repeated). FR-016 lex tiebreaker keeps E002; with
    // the PR-B fixup, E002's replacement is canonical.
    let source = "SECRET//REL TO GBR, AUS, GBR\n";
    let (fixed, applied) = fix_once(source);

    assert_eq!(
        fixed, "SECRET//REL TO USA, AUS, GBR\n",
        "single pass must add USA, sort, and dedup"
    );
    let rel_to_fixes: Vec<&String> = applied
        .iter()
        .filter(|r| matches!(r.as_str(), "E002" | "E052"))
        .collect();
    assert_eq!(
        rel_to_fixes.len(),
        1,
        "C-1 overlap guard must keep exactly one of E002/E052: {applied:?}"
    );
}

#[test]
fn e002_and_e052_overlap_is_idempotent_under_re_fix() {
    let source = "SECRET//REL TO GBR, AUS, GBR\n";
    let (after_first, _) = fix_once(source);
    let (after_second, applied_second) = fix_once(&after_first);

    assert_eq!(
        after_first, after_second,
        "fix must reach a fixed point in a single pass"
    );
    let rel_to_fixes_in_second: Vec<&String> = applied_second
        .iter()
        .filter(|r| matches!(r.as_str(), "E002" | "E060" | "E052"))
        .collect();
    assert!(
        rel_to_fixes_in_second.is_empty(),
        "second pass must not apply any REL TO fixes: {applied_second:?}"
    );
}

// ---------------------------------------------------------------------------
// E052 alone — input already canonical except for duplicates
// ---------------------------------------------------------------------------

#[test]
fn e052_alone_produces_canonical_output_when_input_is_in_order() {
    // Input is alphabetically ordered with USA first, but a code
    // appears twice. E060 (REL TO row) does NOT fire (sort-only
    // check passes because USA, AUS, AUS, GBR sort-stable equals
    // itself). Only E052 fires; its dedup-only replacement is
    // canonical because the input was already canonical modulo the
    // duplicate.
    let source = "SECRET//REL TO USA, AUS, AUS, GBR\n";
    let (fixed, applied) = fix_once(source);

    assert_eq!(
        fixed, "SECRET//REL TO USA, AUS, GBR\n",
        "E052 alone must produce canonical output"
    );
    assert!(
        applied.iter().any(|r| r == "E052"),
        "E052 must be the rule that fires: {applied:?}"
    );
    assert!(
        !applied.iter().any(|r| r == "E060"),
        "E060 (REL TO row) must NOT fire on an in-order list: {applied:?}"
    );
}

// ---------------------------------------------------------------------------
// E060 (REL TO row) alone — misordered without duplicates
// ---------------------------------------------------------------------------

#[test]
fn e060_rel_to_row_alone_still_canonicalizes_when_input_has_no_duplicates() {
    // Pure misorder, no duplicates. E060 (REL TO row) fires alone.
    // The dedup-then-canonicalize path is a no-op for the dedup
    // step; output is the sorted list. This pins that the dedup
    // compose doesn't accidentally reshape inputs that have no
    // duplicates.
    let source = "SECRET//REL TO USA, GBR, AUS\n";
    let (fixed, applied) = fix_once(source);

    assert_eq!(
        fixed, "SECRET//REL TO USA, AUS, GBR\n",
        "E060 (REL TO row) alone must produce sorted output (dedup is a \
         no-op when input has no duplicates)"
    );
    assert!(
        applied.iter().any(|r| r == "E060"),
        "E060 (REL TO row) must fire on misordered input: {applied:?}"
    );
    assert!(
        !applied.iter().any(|r| r == "E052"),
        "E052 must NOT fire when no duplicates are present: {applied:?}"
    );
}

// ---------------------------------------------------------------------------
// Stress: misorder + duplicates spanning across multiple REL TO blocks
// ---------------------------------------------------------------------------

#[test]
fn multi_block_dedup_reaches_fixed_point_in_single_pass() {
    // Two REL TO blocks, each independently misordered AND
    // duplicated. E060 (REL TO row) detects multiple REL TO blocks
    // (`rel_to_blocks.len() > 1`) and takes the suppression branch
    // — it emits a diagnostic but no fix, because a flat first→last
    // splice across blocks would delete the intervening `//...//`
    // content. E052 is the only fix-emitting rule on this input; it
    // scopes each diagnostic per block, so each block's duplicates
    // are collapsed independently and the intervening `//NF//`
    // (which E001 expands to `//NOFORN//`) is preserved verbatim.
    //
    // The idempotency contract this test pins: the multi-block
    // dedup-only path reaches a fixed point in one fix pass. Per-
    // block ordering is NOT canonicalized here — that would require
    // lifting E060 (REL TO row)'s multi-block suppression, which is
    // out of scope for the PR-B / PR-3b.F refactors. The block-
    // level orderings stay as the user wrote them, minus duplicates.
    let source = "SECRET//REL TO USA, GBR, AUS, GBR//NF//REL TO USA, JPN, AUS, JPN\n";
    let (fixed, _) = fix_once(source);
    assert_eq!(
        fixed, "SECRET//REL TO USA, GBR, AUS//NOFORN//REL TO USA, JPN, AUS\n",
        "each block must dedup independently; E001 expands NF→NOFORN; \
         E060 (REL TO row) suppresses on multi-block input so \
         per-block ordering is preserved"
    );

    // Idempotency: re-running fix produces no further changes. E060
    // (REL TO row) still suppresses (still multi-block); E052 has
    // nothing to dedup.
    let (after_second, applied_second) = fix_once(&fixed);
    assert_eq!(
        after_second, fixed,
        "single-pass fixed-point invariant under multi-block dedup"
    );
    let rel_to_fixes_in_second: Vec<&String> = applied_second
        .iter()
        .filter(|r| matches!(r.as_str(), "E002" | "E052"))
        .collect();
    assert!(
        rel_to_fixes_in_second.is_empty(),
        "no fix-emitting REL TO rule may fire on the deduped output: {applied_second:?}"
    );
}

// ---------------------------------------------------------------------------
// Audit-record content: applied fix carries E060 (REL TO row) or E052
// (whichever wins; post-PR-3b.F it's E052 per FR-016 lex tiebreaker)
// with confidence 1.0 and source = BuiltinRule
// ---------------------------------------------------------------------------

#[test]
fn applied_fix_for_overlap_carries_audit_provenance() {
    let source = "SECRET//REL TO USA, GBR, AUS, GBR\n";
    let engine = engine_default();
    let result = engine.fix(source.as_bytes(), FixMode::Apply);

    let rel_to_applied: Vec<_> = result
        .applied
        .iter()
        .filter(|f| matches!(f.proposal.rule.as_str(), "E060" | "E052"))
        .collect();
    assert_eq!(
        rel_to_applied.len(),
        1,
        "exactly one rule in {{E060, E052}} survives the overlap guard"
    );
    let applied = rel_to_applied[0];
    assert!(
        (applied.proposal.confidence.combined() - 1.0).abs() < f32::EPSILON,
        "REL TO list-grammar fixes are deterministic — confidence must be 1.0"
    );
    assert_eq!(
        applied.proposal.source,
        marque_rules::FixSource::BuiltinRule,
        "REL TO list-grammar fixes originate in built-in rules, not the corrections map"
    );
    // Post-PR-3b.F: E052 wins the FR-016 lex tiebreaker
    // (`'E052' < 'E060'`), and E052's fix is dedup-only (preserves
    // first-occurrence order). Set-equal to the canonical, but the
    // alphabetical order is reached on the second fix pass via
    // E060 (REL TO row) — see
    // `e060_rel_to_row_and_e052_overlap_resolves_at_fixed_point`.
    let replacement = applied.proposal.replacement.as_ref();
    assert!(
        replacement == "USA, GBR, AUS" || replacement == "USA, AUS, GBR",
        "the surviving fix's replacement must be set-equal to the \
         canonical REL TO list (E052 wins post-rename and produces \
         `USA, GBR, AUS`; if a future change reverses the tiebreaker \
         the replacement becomes the fully canonical \
         `USA, AUS, GBR`); got: {replacement}"
    );
}
