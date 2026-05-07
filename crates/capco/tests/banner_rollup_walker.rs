// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Behavior tests for the banner-roll-up walker (T026a, PR 3b Sub-move A).
//!
//! These tests pin the *behavior* of `BannerMatchesProjectedRule` — the
//! walker that subsumed `SarBannerRollupRule` (E031),
//! `SciBannerRollupRule` (E035), and `NodisExdisBannerRollupRule` (E040).
//! The contract under test:
//!
//! 1. Each catalog row emits its diagnostic under the historical per-row
//!    rule ID (E031 / E035 / E040), preserving audit-stream continuity
//!    and the FR-016 / C-1 overlap-guard interaction with E028 / E029.
//! 2. Each row carries its own §-citation, severity, and fix shape.
//! 3. The walker fires only on Banner / CAB candidates and only when a
//!    `PageContext` is available.
//! 4. Multiple categories can fire simultaneously without interference.
//!
//! These are end-to-end tests through `Engine::lint`, not unit tests on
//! the walker's internals — the goal is to assert the *user-visible*
//! diagnostic stream, which is what audit consumers depend on. The
//! tests deliberately do NOT assert the walker's struct name, the
//! catalog's row count, or any other implementation detail that could
//! change without a behavior change.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, Severity};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic> {
    engine().lint(source.as_bytes()).diagnostics
}

fn diags_for_rule<'a>(diags: &'a [Diagnostic], rule_id: &str) -> Vec<&'a Diagnostic> {
    diags
        .iter()
        .filter(|d| d.rule.as_str() == rule_id)
        .collect()
}

// ---------------------------------------------------------------------------
// SAR row (E031) — banner has block, missing program
// ---------------------------------------------------------------------------

#[test]
fn sar_row_fires_when_banner_has_block_but_omits_program() {
    // Portions: SAR-BP and SAR-CD. Banner: SAR-BP only. The walker's
    // SAR row must emit one E031 diagnostic with a zero-width
    // insertion fix appending "/CD" at the end of the SAR block.
    // Confidence 0.9 (per §H.5 p101 — fixable with high but not
    // perfect confidence; sub-threshold by default config so the
    // engine demotes Severity::Fix to Severity::Suggest).
    let source = "(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
    let diags = lint(source);
    let e031 = diags_for_rule(&diags, "E031");
    assert_eq!(
        e031.len(),
        1,
        "exactly one E031 diagnostic; got {} from full diag list: {diags:?}",
        e031.len()
    );

    let d = e031[0];
    assert!(
        d.message.contains("CD"),
        "diagnostic message must name the missing program; got: {:?}",
        d.message
    );
    assert!(
        d.citation.contains("§H.5 p101"),
        "E031 row citation must reference §H.5 p101; got: {:?}",
        d.citation
    );

    let fix = d.fix.as_ref().expect(
        "E031 SAR row must carry a fix when banner has SAR block — \
         the zero-width-insertion path is the FR-016 / C-1 contract \
         that lets E031 coexist with E028 / E029",
    );
    assert_eq!(
        fix.span.start, fix.span.end,
        "E031 fix must be a zero-width insertion (start == end)",
    );
    assert_eq!(
        fix.original.as_ref(),
        "",
        "zero-width insertion has empty `original`",
    );
    assert_eq!(
        fix.replacement.as_ref(),
        "/CD",
        "insertion replacement must be `/<missing-program>`",
    );
    assert!(
        (fix.confidence.combined() - 0.9).abs() < f32::EPSILON,
        "E031 SAR-row confidence is 0.9; got {}",
        fix.confidence.combined(),
    );
}

// ---------------------------------------------------------------------------
// SAR row — banner has no block at all
// ---------------------------------------------------------------------------

#[test]
fn sar_row_emits_error_no_fix_when_banner_lacks_sar_block_entirely() {
    // Portion has SAR-BP; banner has no SAR block. Inserting a new
    // category block requires byte-positioning the block between SCI
    // and AEA, which the walker cannot safely supply from rule
    // context. Severity escalates to Error and the diagnostic carries
    // no fix.
    let source = "(S//SAR-BP//NF)\nSECRET//NOFORN";
    let diags = lint(source);
    let e031 = diags_for_rule(&diags, "E031");
    assert_eq!(e031.len(), 1, "expected one E031: {diags:?}");

    let d = e031[0];
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.fix.is_none(),
        "no-block path must not propose a fix; got: {:?}",
        d.fix,
    );
    assert!(
        d.message.contains("missing an SAR block"),
        "message wording must describe a whole missing block, not a \
         partial one (PR #101 review pin); got: {:?}",
        d.message,
    );
}

// ---------------------------------------------------------------------------
// SCI row (E035) — banner missing system
// ---------------------------------------------------------------------------

#[test]
fn sci_row_fires_when_banner_missing_compartment_present_in_portion() {
    // Portion: TS//SI-G//NF. Banner: TS//SI//NF. The portion carries
    // compartment G; the banner shows bare SI. §H.4 contains no
    // hierarchy-optional carve-out (unlike SAR §H.5 p101), so the
    // SCI row must fire on the missing compartment.
    let source = "(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";
    let diags = lint(source);
    let e035 = diags_for_rule(&diags, "E035");
    assert_eq!(e035.len(), 1, "expected one E035: {diags:?}");

    let d = e035[0];
    assert_eq!(
        d.severity,
        Severity::Error,
        "E035 row severity is Error (no Fix → Suggest demotion)",
    );
    assert!(
        d.citation.contains("§H.4"),
        "E035 row citation must reference §H.4 per-system precedence \
         rules; got: {:?}",
        d.citation,
    );
    assert!(
        d.message.contains("G") && d.message.contains("compartment"),
        "diagnostic must name the missing compartment kind; got: {:?}",
        d.message,
    );
}

// ---------------------------------------------------------------------------
// Non-IC dissem row (E040) — banner missing NODIS
// ---------------------------------------------------------------------------

#[test]
fn non_ic_dissem_row_fires_when_banner_missing_nodis() {
    // Portion: NODIS. Banner: classification + IC dissem (NOFORN) +
    // Non-IC dissem (LIMDIS) — has a Non-IC block but missing NODIS.
    // §H.9 p174: NODIS in any portion must appear in the banner.
    // Banner has a Non-IC block, so the fix is a zero-width insertion
    // appending "/NODIS".
    let source = "(S//NF//ND)\nSECRET//NOFORN//LIMDIS";
    let diags = lint(source);
    let e040 = diags_for_rule(&diags, "E040");
    assert_eq!(e040.len(), 1, "expected one E040: {diags:?}");

    let d = e040[0];
    assert!(
        d.citation.contains("§H.9 p172") || d.citation.contains("§H.9 p174"),
        "E040 row citation must reference §H.9 p172 (EXDIS) and/or \
         §H.9 p174 (NODIS); got: {:?}",
        d.citation,
    );
    assert!(
        d.message.contains("NODIS"),
        "diagnostic must name NODIS as the required token; got: {:?}",
        d.message,
    );

    let fix = d
        .fix
        .as_ref()
        .expect("Non-IC row with block must carry a fix");
    assert_eq!(fix.span.start, fix.span.end, "zero-width insertion");
    assert_eq!(fix.replacement.as_ref(), "/NODIS");
}

#[test]
fn non_ic_dissem_row_emits_error_no_fix_when_banner_lacks_non_ic_block_entirely() {
    // Portion: NODIS. Banner: classification + IC dissem only — no
    // Non-IC dissem block at all. The Non-IC evaluator's `None` arm
    // applies here: byte-positioning a new Non-IC category block from
    // rule context alone requires separator offsets the rule cannot
    // safely supply, so the row escalates severity to Error and emits
    // no fix.
    //
    // Isolated from `walker_fires_per_row_when_multiple_categories_mismatch`
    // (which exercises all three `None` arms simultaneously) so a
    // regression on the Non-IC `None` arm specifically is named in
    // CI output instead of being entangled with SAR / SCI failures.
    let source = "(S//NF//ND)\nSECRET//NOFORN";
    let diags = lint(source);
    let e040 = diags_for_rule(&diags, "E040");
    assert_eq!(e040.len(), 1, "expected one E040: {diags:?}");

    let d = e040[0];
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.fix.is_none(),
        "Non-IC dissem `None` arm must not propose a fix; got: {:?}",
        d.fix,
    );
    assert!(
        d.message.contains("NODIS"),
        "diagnostic must name NODIS as the required token; got: {:?}",
        d.message,
    );
}

// ---------------------------------------------------------------------------
// Banner matches projection — silence
// ---------------------------------------------------------------------------

#[test]
fn walker_silent_when_banner_matches_all_categories() {
    // Three portions covering SAR, SCI, and NODIS; banner reflects
    // all three correctly. Walker must not emit any of E031 / E035 /
    // E040.
    let source = "(TS//SI-G//SAR-BP//ND)\n\
                  (TS//SI-G//SAR-BP//ND)\n\
                  TOP SECRET//SI-G//SAR-BP//NOFORN//NODIS";
    let diags = lint(source);
    for rule_id in ["E031", "E035", "E040"] {
        let row_diags = diags_for_rule(&diags, rule_id);
        assert!(
            row_diags.is_empty(),
            "walker emitted unexpected {rule_id} diagnostic on \
             matching banner; full diag list: {diags:?}",
        );
    }
}

// ---------------------------------------------------------------------------
// Multiple categories fire simultaneously
// ---------------------------------------------------------------------------

#[test]
fn walker_fires_per_row_when_multiple_categories_mismatch() {
    // Portion has SAR + SCI compartment + NODIS; banner is bare class.
    // All three category rows must fire, each with its own per-row
    // ID, severity, and citation.
    //
    // Note: when the banner has NO SAR block, NO SCI block, and NO
    // Non-IC dissem block, the walker emits no-fix Error diagnostics
    // for each — the SAR / Non-IC code paths emit Error severity
    // directly (overriding the row's `severity` field) because
    // byte-positioning a new category block from rule context alone
    // is unsafe. SCI similarly emits Error.
    let source = "(TS//SI-G//SAR-BP//ND)\nTOP SECRET//NOFORN";
    let diags = lint(source);

    let e031 = diags_for_rule(&diags, "E031");
    let e035 = diags_for_rule(&diags, "E035");
    let e040 = diags_for_rule(&diags, "E040");

    assert_eq!(e031.len(), 1, "expected one E031 (SAR row): {diags:?}");
    assert_eq!(e035.len(), 1, "expected one E035 (SCI row): {diags:?}");
    assert_eq!(e040.len(), 1, "expected one E040 (Non-IC row): {diags:?}");

    // Distinct citations per row — D13 single-citation discipline.
    assert!(e031[0].citation.contains("§H.5 p101"));
    assert!(e035[0].citation.contains("§H.4"));
    assert!(e040[0].citation.contains("§H.9 p172") || e040[0].citation.contains("§H.9 p174"));

    // Distinct rule IDs reach the audit stream — the load-bearing
    // invariant from the T026a risk register.
    let mut ids: Vec<&str> = vec![
        e031[0].rule.as_str(),
        e035[0].rule.as_str(),
        e040[0].rule.as_str(),
    ];
    ids.sort();
    assert_eq!(ids, vec!["E031", "E035", "E040"]);
}

// ---------------------------------------------------------------------------
// Marking-type guard — portion candidates do not trigger the walker
// ---------------------------------------------------------------------------

#[test]
fn walker_silent_on_portion_candidates() {
    // A bare portion with no banner. The walker's marking-type guard
    // must short-circuit; no E031 / E035 / E040 diagnostic should
    // appear. (Other rules may still fire on the portion — we
    // assert only that the walker stays silent.)
    let source = "(TS//SI-G//SAR-BP//ND)";
    let diags = lint(source);
    for rule_id in ["E031", "E035", "E040"] {
        let row_diags = diags_for_rule(&diags, rule_id);
        assert!(
            row_diags.is_empty(),
            "walker fired on a portion-only document — marking-type \
             guard regression. {rule_id} diags: {row_diags:?}",
        );
    }
}

// ---------------------------------------------------------------------------
// PageContext guard — banner with no preceding portions
// ---------------------------------------------------------------------------

#[test]
fn walker_silent_when_banner_has_no_preceding_portions() {
    // A document with a banner but no portion candidates anywhere.
    // The engine never builds a PageContext (no portions to
    // accumulate), so `ctx.page_context` stays `None` and the walker
    // returns early. No E031 / E035 / E040 emitted.
    let source = "TOP SECRET//SI-G//SAR-BP//NOFORN//NODIS";
    let diags = lint(source);
    for rule_id in ["E031", "E035", "E040"] {
        let row_diags = diags_for_rule(&diags, rule_id);
        assert!(
            row_diags.is_empty(),
            "walker fired on a banner with no PageContext — \
             page-context guard regression. {rule_id} diags: \
             {row_diags:?}",
        );
    }
}
