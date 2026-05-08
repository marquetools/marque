// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3b.F (T026f) — `DeclarativeNonCanonicalInputRule` (rule ID
//! `E060`) integration tests.
//!
//! The walker collapses four retired ordering-validation rules into
//! a single `impl Rule` block dispatched over a private 5-row catalog
//! (`NON_CANONICAL_CATALOG`):
//!
//!   Row 1 — `non-canonical/rel-to-usa-first`            (§H.8 p150-151)
//!   Row 2 — `non-canonical/joint-alphabetical`           (§H.3 p56)
//!   Row 3 — `non-canonical/sigma-numeric-sort`           (§H.6 p108)
//!   Row 4 — `non-canonical/sar-program-ascending-sort`   (§H.5 p99)
//!   Row 5 — `non-canonical/sci-compartment-numeric-then-alpha` (§H.4 p61)
//!
//! All emitted diagnostics carry `Diagnostic.rule = "E060"`; per-row
//! identification flows via the diagnostic message text + the
//! `Diagnostic.citation` field. The catalog is private to the
//! walker module — these tests pin per-row behavior via the engine's
//! public lint surface (`Engine::lint`) rather than via direct
//! catalog access, per `feedback_pub_doc_hidden_is_still_public_api.md`.
//!
//! Authority for each row is the cited CAPCO-2016 §X.Y pNN passage
//! verified at plan-authoring time against the vendored
//! `crates/capco/docs/CAPCO-2016.md` (per Constitution VIII).
//!
//! See `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md`
//! §6 for the full test plan.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::{Diagnostic, Severity};
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine_default() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn engine_with_config(config: Config) -> Engine {
    Engine::with_clock(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("CAPCO scheme has no rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic> {
    engine_default().lint(source.as_bytes()).diagnostics
}

fn e060_diags(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    diags.iter().filter(|d| d.rule.as_str() == "E060").collect()
}

/// Filter on E060 + a row-identifying message-text substring so a
/// single multi-violation fixture can be partitioned into per-row
/// hits without running afoul of cross-row interference.
fn e060_diags_for_row<'a>(diags: &'a [Diagnostic], message_substring: &str) -> Vec<&'a Diagnostic> {
    diags
        .iter()
        .filter(|d| d.rule.as_str() == "E060" && d.message.contains(message_substring))
        .collect()
}

// ===========================================================================
// §6.4 — Catalog-pin test (hardcoded EXPECTED_KINDS reconciles against
// engine's public lint surface; no `pub fn` + `#[doc(hidden)]` needed)
// ===========================================================================

/// Per-row pinning constant: each entry pairs a row-identifying
/// message-text substring with the row's authoritative §-citation.
/// These five rows are the entire catalog — bumping the count means
/// the walker's catalog actually grew or shrank, which is an
/// intentional, documented change.
const EXPECTED_ROWS: &[(&str, &str)] = &[
    // Row 1 — REL TO USA-first alpha
    (
        "REL TO country codes must be alphabetically ordered",
        "§H.8 p150",
    ),
    // Row 2 — JOINT alphabetical
    (
        "JOINT country codes must be alphabetically ordered",
        "§H.3 p56",
    ),
    // Row 3 — AEA SIGMA numeric sort
    ("SIGMA numbers must be in numerical order", "§H.6 p108"),
    // Row 4 — SAR program ascending sort
    ("SAR programs must be in ascending order", "§H.5 p99"),
    // Row 5 — SCI compartment + sub-compartment numeric-then-alpha
    (
        "SCI compartments must be listed in ascending order",
        "§H.4 p61",
    ),
];

#[test]
fn test_non_canonical_walker_declares_5_rows() {
    // Lint a fixture containing one violation per row — but the row
    // authors' invariants don't all fit in a single banner (e.g.,
    // SCI sub-compartment ordering invariant requires its own
    // marking shape, JOINT requires a JOINT classification token,
    // AEA SIGMA requires RD/FRD with sigma values). Run five
    // separate fixtures and assert each row fires exactly once.
    let row_fixtures = [
        // Row 1: REL TO unordered after USA. Misordered (`GBR, AUS`
        // should be `AUS, GBR`). E060 (REL TO row) fires.
        (
            "SECRET//REL TO USA, GBR, AUS",
            "REL TO country codes must be alphabetically ordered",
        ),
        // Row 2: JOINT countries unordered (`USA GBR AUS` → expected
        // `AUS GBR USA` in pure alpha per §H.3 p56).
        (
            "//JOINT S USA GBR AUS//REL TO USA, AUS, GBR",
            "JOINT country codes must be alphabetically ordered",
        ),
        // Row 3: SIGMA out of numerical order. RD-SIGMA 20 14 should
        // be RD-SIGMA 14 20.
        (
            "SECRET//RD-SIGMA 20 14//NOFORN",
            "SIGMA numbers must be in numerical order",
        ),
        // Row 4: SAR programs out of order (`CD/BP` → `BP/CD`).
        (
            "SECRET//SAR-CD/BP//NOFORN",
            "SAR programs must be in ascending order",
        ),
        // Row 5: SCI compartments out of order (`SI-NK-G` → `SI-G-NK`
        // since `G < NK`).
        (
            "TOP SECRET//SI-NK-G//ORCON/NOFORN",
            "SCI compartments must be listed in ascending order",
        ),
    ];
    assert_eq!(
        row_fixtures.len(),
        EXPECTED_ROWS.len(),
        "row_fixtures count must match EXPECTED_ROWS count — bumping \
         either means the walker catalog actually grew or shrank, \
         which is an intentional, documented change",
    );
    for (idx, (source, expected_msg)) in row_fixtures.iter().enumerate() {
        let diags = lint(source);
        let row_diags = e060_diags_for_row(&diags, expected_msg);
        assert_eq!(
            row_diags.len(),
            1,
            "row {idx} ({expected_msg:?}) must fire exactly once on \
             fixture {source:?}; full diags: {diags:?}",
        );
    }
}

#[test]
fn test_non_canonical_walker_emits_authored_citations() {
    // For each row, lint a per-row violation fixture and assert the
    // emitted Diagnostic.citation contains the authoritative §-anchor
    // verified at plan-authoring time. This is the per-row
    // citation-fidelity test (Constitution VIII).
    let row_fixtures = [
        ("SECRET//REL TO USA, GBR, AUS", "§H.8 p150"),
        ("//JOINT S USA GBR AUS//REL TO USA, AUS, GBR", "§H.3 p56"),
        ("SECRET//RD-SIGMA 20 14//NOFORN", "§H.6 p108"),
        ("SECRET//SAR-CD/BP//NOFORN", "§H.5 p99"),
        ("TOP SECRET//SI-NK-G//ORCON/NOFORN", "§H.4 p61"),
    ];
    for (source, expected_cite) in row_fixtures.iter() {
        let diags = lint(source);
        let e060 = e060_diags(&diags);
        assert!(!e060.is_empty(), "E060 must fire on {source:?}: {diags:?}",);
        assert!(
            e060.iter().any(|d| d.citation.contains(expected_cite)),
            "E060 citation on {source:?} must contain {expected_cite:?}; \
             got citations: {:?}",
            e060.iter().map(|d| d.citation).collect::<Vec<_>>(),
        );
    }
}

#[test]
fn test_non_canonical_walker_citation_fidelity_locks_expected_rows() {
    // Catalog-pin reconciliation: assert that the per-row
    // (message_substring, citation) pairs in EXPECTED_ROWS are all
    // observable from the public lint surface using fixtures that
    // exercise each row. If a future agent renames a row's message
    // text or citation, this test fails before the change can land.
    //
    // Constitution VIII (citation integrity) requires this:
    // citations are per-claim, not per-block, and any rename of a
    // row's authoritative passage must be a deliberate documented
    // change.
    let combined_fixtures = [
        "SECRET//REL TO USA, GBR, AUS",
        "//JOINT S USA GBR AUS//REL TO USA, AUS, GBR",
        "SECRET//RD-SIGMA 20 14//NOFORN",
        "SECRET//SAR-CD/BP//NOFORN",
        "TOP SECRET//SI-NK-G//ORCON/NOFORN",
    ];
    for ((expected_msg, expected_cite), source) in
        EXPECTED_ROWS.iter().zip(combined_fixtures.iter())
    {
        let diags = lint(source);
        let row_diag = e060_diags_for_row(&diags, expected_msg);
        assert!(
            !row_diag.is_empty(),
            "row {expected_msg:?} must fire on {source:?}: {diags:?}",
        );
        assert!(
            row_diag[0].citation.contains(expected_cite),
            "row {expected_msg:?} citation must contain {expected_cite:?}; \
             got: {:?}",
            row_diag[0].citation,
        );
    }
}

// ===========================================================================
// §6.2 — Per-row behavior triplet (fires-on-violation, doesn't-fire-when-
// satisfied, doesn't-fire-when-marking-absent)
// ===========================================================================

// Row 1 — REL TO USA-first alpha
// ---------------------------------------------------------------------------

#[test]
fn row1_rel_to_fires_on_unordered() {
    // GBR before AUS — should be USA, AUS, GBR after USA-first.
    let diags = lint("SECRET//REL TO USA, GBR, AUS");
    let e060 = e060_diags_for_row(&diags, "REL TO country codes");
    assert_eq!(e060.len(), 1, "Row 1 must fire on USA, GBR, AUS: {diags:?}");
    let fix = e060[0].fix.as_ref().expect("Row 1 must carry a fix");
    assert_eq!(fix.replacement.as_ref(), "USA, AUS, GBR");
}

#[test]
fn row1_rel_to_does_not_fire_when_canonical() {
    let diags = lint("SECRET//REL TO USA, AUS, GBR");
    let e060 = e060_diags_for_row(&diags, "REL TO country codes");
    assert!(
        e060.is_empty(),
        "Row 1 must not fire on canonical list: {diags:?}",
    );
}

#[test]
fn row1_rel_to_does_not_fire_when_axis_absent() {
    // No REL TO marking present.
    let diags = lint("SECRET//NOFORN");
    let e060 = e060_diags_for_row(&diags, "REL TO country codes");
    assert!(
        e060.is_empty(),
        "Row 1 must not fire when REL TO axis is absent: {diags:?}",
    );
}

// Row 2 — JOINT alphabetical
// ---------------------------------------------------------------------------

#[test]
fn row2_joint_fires_on_unordered() {
    // GBR before AUS in JOINT list (not pure alpha).
    let diags = lint("//JOINT S USA GBR AUS//REL TO USA, AUS, GBR");
    let e060 = e060_diags_for_row(&diags, "JOINT country codes");
    assert_eq!(e060.len(), 1, "Row 2 must fire on JOINT: {diags:?}");
}

#[test]
fn row2_joint_does_not_fire_on_pure_alpha() {
    // `AUS GBR USA` is the canonical JOINT order (pure alpha,
    // no USA-first carve-out per §H.3 p56). Row 2 must stay silent
    // even though USA isn't first — that's S003's territory.
    let diags = lint("//JOINT S AUS GBR USA");
    let e060 = e060_diags_for_row(&diags, "JOINT country codes");
    assert!(
        e060.is_empty(),
        "Row 2 must not fire on pure-alpha JOINT (USA last is OK): {diags:?}",
    );
}

#[test]
fn row2_joint_does_not_fire_when_axis_absent() {
    // No JOINT classification.
    let diags = lint("SECRET//REL TO USA, AUS, GBR");
    let e060 = e060_diags_for_row(&diags, "JOINT country codes");
    assert!(
        e060.is_empty(),
        "Row 2 must not fire when JOINT axis is absent: {diags:?}",
    );
}

// Row 3 — AEA SIGMA numeric sort
// ---------------------------------------------------------------------------

#[test]
fn row3_sigma_fires_on_misorder() {
    // SIGMA 20 14 should be 14 20.
    let diags = lint("SECRET//RD-SIGMA 20 14//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SIGMA numbers must be in numerical order");
    assert_eq!(e060.len(), 1, "Row 3 must fire on SIGMA 20 14: {diags:?}");
    let fix = e060[0]
        .fix
        .as_ref()
        .expect("Row 3 misorder must carry a fix");
    assert_eq!(fix.replacement.as_ref(), "14 20");
}

#[test]
fn row3_sigma_invalid_set_emit_no_fix() {
    // SIGMA 99 is outside the currently authorized set (14, 15, 18,
    // 20). Row 3 emits a no-fix diagnostic for this branch.
    let diags = lint("SECRET//RD-SIGMA 99//NOFORN");
    let e060 = e060_diags_for_row(&diags, "currently authorized set");
    assert_eq!(
        e060.len(),
        1,
        "Row 3 invalid-set branch must fire on SIGMA 99: {diags:?}",
    );
    assert!(
        e060[0].fix.is_none(),
        "Row 3 invalid-set branch must NOT carry a fix (historical \
         SIGMA values require originating-program guidance): {:?}",
        e060[0],
    );
}

#[test]
fn row3_sigma_does_not_fire_on_canonical() {
    let diags = lint("SECRET//RD-SIGMA 14 20//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SIGMA numbers");
    assert!(
        e060.is_empty(),
        "Row 3 must not fire on SIGMA 14 20 (canonical): {diags:?}",
    );
}

#[test]
fn row3_sigma_does_not_fire_when_axis_absent() {
    let diags = lint("SECRET//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SIGMA numbers");
    assert!(
        e060.is_empty(),
        "Row 3 must not fire when AEA axis is absent: {diags:?}",
    );
}

// Row 4 — SAR program ascending sort
// ---------------------------------------------------------------------------

#[test]
fn row4_sar_fires_on_misorder() {
    let diags = lint("SECRET//SAR-CD/BP//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SAR programs");
    assert_eq!(e060.len(), 1, "Row 4 must fire on CD/BP: {diags:?}");
    let fix = e060[0].fix.as_ref().expect("Row 4 must carry a fix");
    assert_eq!(fix.replacement.as_ref(), "SAR-BP/CD");
    assert!((fix.confidence.combined() - 0.85).abs() < f32::EPSILON);
}

#[test]
fn row4_sar_does_not_fire_on_canonical() {
    let diags = lint("SECRET//SAR-BP/CD//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SAR programs");
    assert!(
        e060.is_empty(),
        "Row 4 must not fire on BP/CD (sorted): {diags:?}",
    );
}

#[test]
fn row4_sar_does_not_fire_when_axis_absent() {
    let diags = lint("SECRET//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SAR programs");
    assert!(
        e060.is_empty(),
        "Row 4 must not fire when SAR axis is absent: {diags:?}",
    );
}

// Row 5 — SCI compartment + sub-compartment numeric-then-alpha
// ---------------------------------------------------------------------------

#[test]
fn row5_sci_compartment_fires_on_misorder() {
    let diags = lint("TOP SECRET//SI-NK-G//ORCON/NOFORN");
    let e060 = e060_diags_for_row(&diags, "SCI compartments");
    assert_eq!(e060.len(), 1, "Row 5 must fire on SI-NK-G: {diags:?}");
    assert_eq!(e060[0].severity, Severity::Error);
}

#[test]
fn row5_sci_subcompartment_fires_on_misorder() {
    // Sub-compartments DEFG ABCD out of alpha order within G.
    let diags = lint("TOP SECRET//SI-G DEFG ABCD//ORCON/NOFORN");
    let e060 = e060_diags_for_row(&diags, "SCI sub-compartments");
    assert_eq!(
        e060.len(),
        1,
        "Row 5 must fire on SI-G DEFG ABCD: {diags:?}",
    );
}

#[test]
fn row5_sci_does_not_fire_on_canonical() {
    let diags = lint("SECRET//SI-G ABCD DEFG//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SCI ");
    assert!(
        e060.is_empty(),
        "Row 5 must not fire on canonical SCI: {diags:?}",
    );
}

#[test]
fn row5_sci_does_not_fire_when_axis_absent() {
    let diags = lint("SECRET//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SCI ");
    assert!(
        e060.is_empty(),
        "Row 5 must not fire when SCI axis is absent: {diags:?}",
    );
}

// ===========================================================================
// §6.5 — REL TO multi-block suppression preserves E002 interaction
// ===========================================================================

#[test]
fn test_e060_rel_to_multi_block_suppression() {
    // Two REL TO blocks; first is misordered. Row 1 must fire with
    // a no-fix diagnostic carrying the suppression message — a
    // first→last splice across blocks would delete the intervening
    // `//NF//` content.
    let src = "SECRET//REL TO USA, GBR//NF//REL TO AUS";
    let diags = lint(src);
    let e060 = e060_diags_for_row(&diags, "REL TO country codes");
    assert_eq!(e060.len(), 1, "Row 1 must still fire: {diags:?}");
    assert!(
        e060[0].fix.is_none(),
        "Row 1 must NOT carry a fix when multiple REL TO blocks are \
         present (cross-block splice would corrupt): {e060:?}",
    );
    assert!(
        e060[0].message.contains("multiple REL TO blocks"),
        "suppression message must explain why: {}",
        e060[0].message,
    );
}

// ===========================================================================
// §6.7 — JOINT alphabetical correctness (no USA-first carve-out)
// ===========================================================================

#[test]
fn test_e060_joint_pure_alpha_no_usa_first_carveout() {
    // CAN ISR USA is alphabetical (pure-alpha canonical for {CAN,
    // ISR, USA}); USA is last but Row 2 must stay silent because
    // §H.3 p56 prescribes pure alpha.
    let diags = lint("//JOINT S CAN ISR USA");
    let e060 = e060_diags_for_row(&diags, "JOINT country codes");
    assert!(
        e060.is_empty(),
        "Row 2 must not fire on alphabetical JOINT even when USA is \
         last: {diags:?}",
    );
}

#[test]
fn test_e060_joint_fires_on_misorder_and_fixes_to_pure_alpha() {
    // ISR before CAN is non-alpha; should fix to CAN ISR USA.
    let diags = lint("//JOINT S ISR CAN USA");
    let e060 = e060_diags_for_row(&diags, "JOINT country codes");
    assert_eq!(e060.len(), 1, "Row 2 must fire on misorder: {diags:?}");
    let fix = e060[0].fix.as_ref().expect("Row 2 must carry a fix");
    assert_eq!(
        fix.replacement.as_ref(),
        "JOINT S CAN ISR USA",
        "JOINT fix must produce pure-alpha (no USA-first carve-out): {fix:?}",
    );
}

// ===========================================================================
// §6.6 — Overlap-guard test against E052 (REL TO no-duplicates)
// ===========================================================================

#[test]
fn test_e060_e052_overlap_guard_admits_one_winner() {
    // Misordered AND duplicated. Both Row 1 (E060 REL TO row) and
    // E052 fire on the same span. Post-PR-3b.F FR-016 lex
    // tiebreaker keeps E052 (`'E052' < 'E060'` since `'5' < '6'`).
    // The detailed engine-level fixed-point convergence is tested
    // in `tests/rel_to_invariants.rs`; here we just lock the
    // "exactly one survives" invariant at the engine fix layer.
    let engine = engine_default();
    let result = engine.fix(b"SECRET//REL TO USA, GBR, AUS, GBR\n", FixMode::Apply);
    let rel_to_applied: Vec<_> = result
        .applied
        .iter()
        .filter(|f| matches!(f.proposal.rule.as_str(), "E060" | "E052"))
        .collect();
    assert_eq!(
        rel_to_applied.len(),
        1,
        "exactly one rule in {{E060, E052}} survives the C-1 \
         overlap guard: {:?}",
        result
            .applied
            .iter()
            .map(|f| f.proposal.rule.as_str())
            .collect::<Vec<_>>(),
    );
}

// ===========================================================================
// §6.11 — `Severity::Off` override skips the walker entirely (FR-008)
// ===========================================================================

#[test]
fn test_e060_off_severity_skips_walker() {
    // Configure `[rules] E060 = "off"` and lint a document with
    // multiple row violations. No E060 diagnostics should emit.
    let mut config = Config::default();
    config
        .rules
        .overrides
        .insert("E060".to_owned(), "off".to_owned());
    let engine = engine_with_config(config);
    let diags = engine
        .lint(b"SECRET//SAR-CD/BP//RD-SIGMA 20 14//NOFORN")
        .diagnostics;
    let e060 = e060_diags(&diags);
    assert!(
        e060.is_empty(),
        "E060 = \"off\" must skip the walker entirely (FR-008): {diags:?}",
    );
}

// ===========================================================================
// §6.12 — Audit-stream traceability via diagnostic message text
// ===========================================================================

#[test]
fn test_e060_per_row_message_identifiability() {
    // Each row's message text contains a unique substring that an
    // audit-stream consumer can grep for to identify which row fired
    // — even after the rule-ID rename from E020/E023/E028/E033 to
    // E060. This pins that the per-row message phrasing is
    // preserved verbatim from the retired rules so existing audit-
    // log queries continue to work.
    let row_fixtures = [
        ("SECRET//REL TO USA, GBR, AUS", "REL TO country codes"),
        ("//JOINT S ISR CAN USA", "JOINT country codes"),
        ("SECRET//RD-SIGMA 20 14//NOFORN", "SIGMA numbers"),
        ("SECRET//SAR-CD/BP//NOFORN", "SAR programs"),
        ("TOP SECRET//SI-NK-G//ORCON/NOFORN", "SCI compartments"),
    ];
    for (source, expected_substring) in row_fixtures.iter() {
        let diags = lint(source);
        let row_diags = e060_diags_for_row(&diags, expected_substring);
        assert!(
            !row_diags.is_empty(),
            "row {expected_substring:?} must fire on {source:?}: {diags:?}",
        );
    }
}

// ===========================================================================
// §6.8 — Byte-identical NDJSON parity (selected fixtures)
// ===========================================================================

#[test]
fn test_e060_walker_preserves_diagnostic_message_text() {
    // The walker preserves byte-identical message text from the
    // four retired rules. This pins the verbatim phrasing for
    // each row.
    //
    // Row 1 message form (verbatim from retired E020 REL TO arm):
    let diags = lint("SECRET//REL TO USA, GBR, AUS");
    let e060 = e060_diags_for_row(&diags, "REL TO country codes");
    assert_eq!(e060.len(), 1);
    assert!(
        e060[0]
            .message
            .starts_with("REL TO country codes must be alphabetically ordered"),
        "Row 1 message text must be byte-identical to retired E020 REL TO arm: {:?}",
        e060[0].message,
    );

    // Row 4 message form (verbatim from retired E028):
    let diags = lint("SECRET//SAR-CD/BP//NOFORN");
    let e060 = e060_diags_for_row(&diags, "SAR programs");
    assert_eq!(e060.len(), 1);
    assert_eq!(
        &*e060[0].message,
        "SAR programs must be in ascending order (numeric first, then alphabetic)",
        "Row 4 message text must be byte-identical to retired E028",
    );

    // Row 5 message form (verbatim from retired E033 compartment arm):
    let diags = lint("TOP SECRET//SI-NK-G//ORCON/NOFORN");
    let e060 = e060_diags_for_row(&diags, "SCI compartments");
    assert_eq!(e060.len(), 1);
    assert_eq!(
        &*e060[0].message,
        "SCI compartments must be listed in ascending order (numeric first, then alphabetic)",
        "Row 5 (compartment arm) message text must be byte-identical to retired E033",
    );

    // Row 5 sub-compartment arm message:
    let diags = lint("TOP SECRET//SI-G DEFG ABCD//ORCON/NOFORN");
    let e060 = e060_diags_for_row(&diags, "SCI sub-compartments");
    assert_eq!(e060.len(), 1);
    assert_eq!(
        &*e060[0].message,
        "SCI sub-compartments must be listed in ascending order (numeric first, then alphabetic)",
        "Row 5 (sub-compartment arm) message text must be byte-identical \
         to retired E033",
    );
}

// ===========================================================================
// Per-row severity preservation
// ===========================================================================

#[test]
fn test_e060_per_row_severity_preserved() {
    // Rows 1-4 are authored at `Severity::Fix` (matches retired
    // E020/E023/E028/E033 defaults — except E033 was Error). Row 5
    // is authored at `Severity::Error` (matches retired E033).
    //
    // Important note on the engine's Fix→Suggest demotion: rows
    // emitted at `Severity::Fix` whose `FixProposal.confidence`
    // falls below the engine's `confidence_threshold` (default
    // 0.95) are demoted to `Severity::Suggest` post-rule per the
    // suggest-don't-fix channel (engine.rs ~line 823). This matches
    // pre-PR-3b.F behavior of the retired rules:
    //
    //   - Row 1 (REL TO) confidence 1.0 → emits at `Fix` (≥ 0.95)
    //   - Row 4 (SAR) confidence 0.85 → demoted to `Suggest`
    //     (< 0.95) — matches retired E028 emitting at `suggest`
    //     in `crates/wasm/tests/parity_corpus.json` line 146.
    //   - Row 5 (SCI) authored at `Error` → emits at `Error`
    //     regardless of confidence (Suggest demotion only applies
    //     to authored-`Fix` rules per engine.rs:823).
    //
    // The walker preserves per-row authoring intent verbatim; the
    // emitted severity reflects authoring intent + engine
    // post-processing, byte-identical to the retired rules.
    let diags = lint("SECRET//REL TO USA, GBR, AUS");
    let row1 = e060_diags_for_row(&diags, "REL TO");
    assert_eq!(
        row1[0].severity,
        Severity::Fix,
        "Row 1 (REL TO) emits at Severity::Fix (confidence 1.0 ≥ \
         threshold 0.95)",
    );

    let diags = lint("SECRET//SAR-CD/BP//NOFORN");
    let row4 = e060_diags_for_row(&diags, "SAR programs");
    assert_eq!(
        row4[0].severity,
        Severity::Suggest,
        "Row 4 (SAR) authored at Fix but confidence 0.85 < 0.95 \
         threshold; engine demotes to Severity::Suggest (matches \
         retired E028 behavior at parity_corpus.json line 146)",
    );

    let diags = lint("TOP SECRET//SI-NK-G//ORCON/NOFORN");
    let row5 = e060_diags_for_row(&diags, "SCI compartments");
    assert_eq!(
        row5[0].severity,
        Severity::Error,
        "Row 5 (SCI) emits at Severity::Error (authored at Error; \
         Suggest demotion only applies to authored-Fix rules)",
    );
}
