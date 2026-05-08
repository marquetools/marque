// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3b.E (T026e) — SCI per-system catalog walker (E059) behavior tests.
//!
//! The 5 catalog rows declared in `crate::scheme::SCI_PER_SYSTEM_CATALOG`
//! are exercised by:
//!
//! 1. **Per-row triplet** (15 tests, 3 per row) — fires on violation,
//!    does not fire when satisfied, does not fire when marking absent.
//! 2. **Multi-branch fan-out** (rows #1 / #3 / #4 — verify each emit
//!    branch independently).
//! 3. **Severity escalation** — no-IC-dissem-block portions escalate to
//!    `Severity::Error` no-fix.
//! 4. **Scope guard** — pure foreign classifications (NATO/JOINT/FGI)
//!    do not fire E059 since §H.4 is US-only-scoped.
//! 5. **PR-D × PR-E overlap** — class-floor + companion violations fire
//!    side-by-side without overlap-guard interference.
//! 6. **Audit traceability** — each emitted message carries the row's
//!    marking label.
//! 7. **Naming convention** — every catalog row name starts with
//!    `sci-per-system/`.
//! 8. **Severity::Off** — `[rules] E059 = "off"` suppresses all PR-E
//!    diagnostics (FR-008 invariant).
//! 9. **Citation fidelity** — every row's citation matches the
//!    verified §H.4 page anchors from the planning doc §2.

use marque_capco::scheme::CapcoScheme;
use marque_capco::{CapcoRuleSet, capco_rules};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, RuleSet, Severity};
use marque_scheme::MarkingScheme;

// ---------------------------------------------------------------------------
// Engine setup helpers
// ---------------------------------------------------------------------------

/// Build a default-configured `Engine` for SCI per-system lint tests.
fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Run `source` through the engine and return its diagnostics.
fn lint(source: &str) -> Vec<Diagnostic> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Filter the diagnostic stream to E059 emissions whose message contains
/// `marker_text` (substring match). The catalog uses one rule ID `E059`
/// for all 5 rows; per-row identification flows via the diagnostic
/// message text. The marker_text is typically the marking label
/// (e.g., `"HCS-O"`, `"SI-G"`, `"TK-{BLFH"`).
fn e059_diags_for<'a>(diags: &'a [Diagnostic], marker_text: &str) -> Vec<&'a Diagnostic> {
    diags
        .iter()
        .filter(|d| d.rule.as_str() == "E059" && d.message.contains(marker_text))
        .collect()
}

/// All E059 diagnostics in `diags`, regardless of message content.
fn e059_diags(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    diags.iter().filter(|d| d.rule.as_str() == "E059").collect()
}

// ===========================================================================
// Authoring-contract tests
// ===========================================================================

#[test]
fn catalog_declares_five_sci_per_system_rows() {
    let scheme = CapcoScheme::new();
    let rows: Vec<&str> = scheme
        .constraints()
        .iter()
        .filter(|c| c.name().starts_with("sci-per-system/"))
        .map(|c| c.name())
        .collect();
    assert_eq!(
        rows.len(),
        5,
        "expected 5 SCI per-system catalog rows; got {}: {:?}",
        rows.len(),
        rows
    );
}

#[test]
fn sci_per_system_catalog_naming_convention() {
    // Every row's `name` MUST start with `sci-per-system/` per the
    // PR 3b.E plan §4.2 naming-prefix invariant. Iterate the catalog
    // directly via `sci_per_system_catalog_row_names()` (a test-only
    // accessor that walks `SCI_PER_SYSTEM_CATALOG`) rather than
    // `scheme.constraints()` filtered by `contains("sci-per-system")`
    // — the latter would silently skip a typo'd-prefix row like
    // `sai-per-system/...`. Direct catalog iteration catches the typo
    // at the row's authoring site.
    let names = marque_capco::scheme::sci_per_system_catalog_row_names();
    assert!(
        !names.is_empty(),
        "catalog must have at least one row; got 0"
    );
    for n in &names {
        assert!(
            n.starts_with("sci-per-system/"),
            "PR-E catalog row {n:?} must start with `sci-per-system/` \
             (no `contains` filter — typo-tolerant)"
        );
    }
}

#[test]
fn sci_per_system_catalog_citations() {
    // Every row's citation must match one of the verified §H.4 page
    // anchors from PR 3b.E plan §2.1.
    let expected: &[(&str, &str)] = &[
        ("sci-per-system/HCS-O-companions", "CAPCO-2016 §H.4 p64"),
        ("sci-per-system/HCS-P-NOFORN", "CAPCO-2016 §H.4 p66"),
        ("sci-per-system/HCS-P-sub-companions", "CAPCO-2016 §H.4 p68"),
        ("sci-per-system/SI-G-companions", "CAPCO-2016 §H.4 p80"),
        (
            "sci-per-system/TK-compartment-NOFORN",
            "CAPCO-2016 §H.4 p87 + p91 + p95",
        ),
    ];
    let scheme = CapcoScheme::new();
    for (name, citation) in expected {
        let row = scheme
            .constraints()
            .iter()
            .find(|c| c.name() == *name)
            .unwrap_or_else(|| panic!("expected catalog row {name:?} present"));
        assert_eq!(
            row.label(),
            *citation,
            "row {name:?} citation drift; expected {citation:?}, got {:?}",
            row.label()
        );
    }
}

#[test]
fn capco_rules_set_includes_sci_per_system_walker() {
    let set = capco_rules();
    let ids: Vec<&str> = set.rules().iter().map(|r| r.id().as_str()).collect();
    assert!(
        ids.contains(&"E059"),
        "rule set must register `DeclarativeSciPerSystemRule` (E059); registered IDs: {ids:?}"
    );
    for retired in [
        "E042", "E043", "E044", "E045", "E046", "E047", "E048", "E049", "E050", "E051",
    ] {
        assert!(
            !ids.contains(&retired),
            "{retired} retired in PR 3b.E; rule set must not register the legacy per-system rule"
        );
    }
}

// ===========================================================================
// Row #1 — HCS-O companions (§H.4 p64)
// ===========================================================================

#[test]
fn hcs_o_companions_fires_when_orcon_missing() {
    // (S//HCS-O//NF) — NOFORN present, ORCON missing. Expect one
    // ORCON-insertion diagnostic.
    let diags = lint("(S//HCS-O//NF)");
    let hits = e059_diags_for(&diags, "HCS-O requires ORCON");
    assert_eq!(
        hits.len(),
        1,
        "exactly one ORCON-missing diagnostic: {diags:?}"
    );
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/OC");
}

#[test]
fn hcs_o_companions_fires_when_noforn_missing() {
    // (S//HCS-O//OC) — ORCON present, NOFORN missing.
    let diags = lint("(S//HCS-O//OC)");
    let hits = e059_diags_for(&diags, "HCS-O requires NOFORN");
    assert_eq!(
        hits.len(),
        1,
        "exactly one NOFORN-missing diagnostic: {diags:?}"
    );
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/NF");
}

#[test]
fn hcs_o_companions_replaces_oc_usgov() {
    // (S//HCS-O//OC-USGOV/NF) — OC-USGOV present, must be replaced.
    let diags = lint("(S//HCS-O//OC-USGOV/NF)");
    let hits = e059_diags_for(&diags, "HCS-O forbids ORCON-USGOV");
    assert_eq!(hits.len(), 1, "exactly one forbid diagnostic: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "OC");
}

#[test]
fn hcs_o_companions_oc_usgov_satisfies_orcon_presence() {
    // OC-USGOV satisfies ORCON-presence post-fix, so the rule must NOT
    // emit an additional ORCON-insertion (would produce duplicate ORCON).
    let diags = lint("(S//HCS-O//OC-USGOV/NF)");
    let hits = e059_diags(&diags);
    let hcs_o_hits: Vec<_> = hits
        .iter()
        .filter(|d| d.message.contains("HCS-O"))
        .collect();
    assert_eq!(
        hcs_o_hits.len(),
        1,
        "only the OC-USGOV replacement should fire; OC-USGOV satisfies \
         ORCON-presence post-fix: {hits:?}"
    );
    assert!(
        hcs_o_hits[0].message.contains("forbids ORCON-USGOV"),
        "sole HCS-O diag must be the forbid diagnostic: {:?}",
        hcs_o_hits[0].message
    );
}

#[test]
fn hcs_o_companions_does_not_fire_when_satisfied() {
    let diags = lint("(S//HCS-O//OC/NF)");
    let hits = e059_diags_for(&diags, "HCS-O");
    assert!(hits.is_empty(), "compliant HCS-O: {diags:?}");
}

#[test]
fn hcs_o_companions_does_not_fire_when_marking_absent() {
    let diags = lint("(S)");
    let hits = e059_diags_for(&diags, "HCS-O");
    assert!(hits.is_empty(), "no HCS-O present: {diags:?}");
}

#[test]
fn hcs_o_companions_no_dissem_block_escalates_to_error_no_fix() {
    // (S//HCS-O) — both companions missing AND no IC dissem block.
    // Verbatim port of `e042_no_dissem_block_escalates_to_error_no_fix`
    // from the retired rules_sci_per_system.rs.
    let diags = lint("(S//HCS-O)");
    let hits: Vec<_> = e059_diags(&diags)
        .into_iter()
        .filter(|d| d.message.contains("HCS-O"))
        .collect();
    assert_eq!(
        hits.len(),
        2,
        "HCS-O must emit one diagnostic per missing companion: {diags:?}"
    );
    for d in &hits {
        assert!(d.fix.is_none(), "no dissem block → no-fix Error: {d:?}");
        assert_eq!(d.severity, Severity::Error);
    }
}

#[test]
fn hcs_o_companions_diagnostic_points_at_sci_not_dissem() {
    // (S//HCS-O//NF) — diagnostic caret on HCS-O SCI token, fix at end
    // of NF. Verbatim port of
    // `e042_companion_insert_diagnostic_points_at_sci_not_dissem`.
    let src = "(S//HCS-O//NF)";
    let diags = lint(src);
    let hits = e059_diags_for(&diags, "HCS-O requires ORCON");
    assert_eq!(hits.len(), 1, "only missing-ORCON diagnostic: {hits:?}");
    let diag = hits[0];
    let fix = diag.fix.as_ref().expect("fix attached");

    // Fix span is the zero-width insertion at the end of `NF`.
    assert_eq!(
        fix.span.start, fix.span.end,
        "insertion fix must be zero-width: {fix:?}"
    );
    let inserted_at = &src.as_bytes()[..fix.span.start];
    assert!(
        inserted_at.ends_with(b"NF"),
        "fix must apply at end of the NF dissem token: prefix={}",
        String::from_utf8_lossy(inserted_at)
    );

    // Diagnostic caret on the HCS-O SCI token.
    assert_ne!(
        diag.span, fix.span,
        "diagnostic caret and fix span must differ: {diag:?}"
    );
    let caret = &src.as_bytes()[diag.span.start..diag.span.end];
    assert_eq!(
        caret, b"HCS-O",
        "diagnostic caret must point at the HCS-O compound SCI token"
    );
}

// ===========================================================================
// Row #2 — HCS-P NOFORN (§H.4 p66)
// ===========================================================================

#[test]
fn hcs_p_noforn_fires_when_missing() {
    let diags = lint("(S//HCS-P//OC)");
    let hits = e059_diags_for(&diags, "HCS-P requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/NF");
}

#[test]
fn hcs_p_noforn_does_not_fire_when_present() {
    let diags = lint("(S//HCS-P//OC/NF)");
    let hits = e059_diags_for(&diags, "HCS-P requires NOFORN");
    assert!(hits.is_empty(), "compliant HCS-P: {diags:?}");
}

#[test]
fn hcs_p_noforn_does_not_fire_on_bare_hcs_or_hcs_o() {
    // Only HCS-P triggers this row.
    let d1 = lint("(S//HCS)");
    assert!(
        e059_diags_for(&d1, "HCS-P requires NOFORN").is_empty(),
        "bare HCS must not fire HCS-P NOFORN row: {d1:?}"
    );
    let d2 = lint("(S//HCS-O//OC/NF)");
    assert!(
        e059_diags_for(&d2, "HCS-P requires NOFORN").is_empty(),
        "HCS-O must not fire HCS-P NOFORN row: {d2:?}"
    );
}

#[test]
fn hcs_p_noforn_no_dissem_block_escalates_to_error_no_fix() {
    let diags = lint("(S//HCS-P)");
    let hits = e059_diags_for(&diags, "HCS-P requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    assert!(hits[0].fix.is_none(), "no dissem → no-fix: {:?}", hits[0]);
    assert_eq!(hits[0].severity, Severity::Error);
}

// ===========================================================================
// Row #3 — HCS-P sub-compartment companions (§H.4 p68)
// ===========================================================================

#[test]
fn hcs_p_sub_companions_fires_when_orcon_missing() {
    // (TS//HCS-P JJJ//NF) — already at TS, NF present, ORCON missing.
    let diags = lint("(TS//HCS-P JJJ//NF)");
    let hits = e059_diags_for(&diags, "HCS-P sub-compartment requires ORCON");
    assert_eq!(hits.len(), 1, "exactly one ORCON-missing: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/OC");
}

#[test]
fn hcs_p_sub_companions_replaces_oc_usgov() {
    let diags = lint("(TS//HCS-P JJJ//OC-USGOV/NF)");
    let hits = e059_diags_for(&diags, "HCS-P sub-compartment forbids ORCON-USGOV");
    assert_eq!(hits.len(), 1, "exactly one forbid: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "OC");
}

#[test]
fn hcs_p_sub_companions_does_not_fire_when_satisfied() {
    let diags = lint("(TS//HCS-P JJJ//OC/NF)");
    let hits = e059_diags_for(&diags, "HCS-P sub-compartment");
    assert!(hits.is_empty(), "compliant HCS-P sub: {diags:?}");
}

#[test]
fn hcs_p_sub_companions_does_not_fire_on_bare_hcs_p() {
    // Bare HCS-P (no sub-compartment) — different row (HCS-P NOFORN).
    let diags = lint("(TS//HCS-P//OC/NF)");
    let hits = e059_diags_for(&diags, "HCS-P sub-compartment");
    assert!(hits.is_empty(), "no sub-compartment present: {diags:?}");
}

#[test]
fn hcs_p_sub_companions_oc_usgov_only_replaces_no_duplicate_orcon() {
    // (TS//HCS-P JJJ//OC-USGOV) — only OC-USGOV, no bare OC. The
    // OC-USGOV satisfies ORCON-presence post-fix; only the replacement
    // should fire (not the insertion).
    let diags = lint("(TS//HCS-P JJJ//OC-USGOV)");
    let hits: Vec<_> = e059_diags(&diags)
        .into_iter()
        .filter(|d| d.message.contains("HCS-P sub-compartment"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "only the OC-USGOV replacement should fire: {hits:?}"
    );
    assert!(hits[0].message.contains("forbids ORCON-USGOV"));
}

// ===========================================================================
// Row #4 — SI-G companions (§H.4 p80)
// ===========================================================================

#[test]
fn si_g_companions_fires_on_missing_orcon() {
    // (TS//SI-G) — no dissem block; ORCON-missing escalates to Error.
    let diags = lint("(TS//SI-G)");
    let hits = e059_diags_for(&diags, "SI-G requires ORCON");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    assert!(hits[0].fix.is_none(), "no dissem → no-fix: {:?}", hits[0]);
    assert_eq!(hits[0].severity, Severity::Error);
}

#[test]
fn si_g_companions_inserts_orcon_when_dissem_block_present() {
    let diags = lint("(TS//SI-G//NF)");
    let hits = e059_diags_for(&diags, "SI-G requires ORCON");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/OC");
}

#[test]
fn si_g_companions_replaces_oc_usgov() {
    let diags = lint("(TS//SI-G//OC-USGOV)");
    let hits = e059_diags_for(&diags, "SI-G forbids ORCON-USGOV");
    assert_eq!(hits.len(), 1, "exactly one forbid: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "OC");
}

#[test]
fn si_g_companions_oc_usgov_only_no_duplicate_orcon() {
    // (TS//SI-G//OC-USGOV) — OC-USGOV satisfies ORCON-presence
    // post-fix; no insertion should fire.
    let diags = lint("(TS//SI-G//OC-USGOV)");
    let hits: Vec<_> = e059_diags(&diags)
        .into_iter()
        .filter(|d| d.message.contains("SI-G"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "only the OC-USGOV replacement should fire: {hits:?}"
    );
    assert!(hits[0].message.contains("forbids ORCON-USGOV"));
}

#[test]
fn si_g_companions_does_not_fire_when_satisfied() {
    let diags = lint("(TS//SI-G//OC)");
    let hits = e059_diags_for(&diags, "SI-G");
    assert!(hits.is_empty(), "compliant SI-G: {diags:?}");
}

// ===========================================================================
// Row #5 — TK compartment NOFORN (§H.4 p87 + p91 + p95)
// ===========================================================================

#[test]
fn tk_compartment_noforn_fires_on_blfh_without_noforn() {
    let diags = lint("(TS//TK-BLFH//OC)");
    let hits = e059_diags_for(&diags, "TK-{BLFH|IDIT|KAND} requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/NF");
}

#[test]
fn tk_compartment_noforn_fires_on_idit_without_noforn() {
    let diags = lint("(TS//TK-IDIT//OC)");
    let hits = e059_diags_for(&diags, "TK-{BLFH|IDIT|KAND} requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
}

#[test]
fn tk_compartment_noforn_fires_on_kand_without_noforn() {
    let diags = lint("(TS//TK-KAND//OC)");
    let hits = e059_diags_for(&diags, "TK-{BLFH|IDIT|KAND} requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
}

#[test]
fn tk_compartment_noforn_does_not_fire_on_bare_tk() {
    // Bare TK doesn't require NOFORN (§H.4 p85 — only the compartments
    // do).
    let diags = lint("(TS//TK)");
    let hits = e059_diags_for(&diags, "TK-{BLFH|IDIT|KAND}");
    assert!(hits.is_empty(), "bare TK must not fire: {diags:?}");
}

#[test]
fn tk_compartment_noforn_does_not_fire_when_present() {
    let diags = lint("(TS//TK-BLFH//NF)");
    let hits = e059_diags_for(&diags, "TK-{BLFH|IDIT|KAND}");
    assert!(hits.is_empty(), "compliant TK-BLFH: {diags:?}");
}

// ===========================================================================
// Scope guard — §H.4 is US-only; pure foreign classifications skip
// ===========================================================================

#[test]
fn e059_skips_joint_classifications() {
    // JOINT banner with every PR-E SCI marking. `us_level()` returns
    // None for JOINT, so all 5 rows must short-circuit.
    let src = "//JOINT S USA GBR//HCS-O HCS-P JJJ//SI-G//TK-BLFH//REL TO USA, GBR";
    let diags = lint(src);
    let hits = e059_diags(&diags);
    assert!(
        hits.is_empty(),
        "E059 must not fire on JOINT (non-US) classification; \
         §H.4 companion constraints are US-scoped: {hits:?}"
    );
}

#[test]
fn e059_still_fires_on_us_classifications() {
    // Sanity: scope guard must not mask US-side violations.
    let diags = lint("(S//HCS-O HCS-P)");
    assert!(
        e059_diags_for(&diags, "HCS-O").iter().any(|_| true),
        "E059 must fire on US-classified HCS-O without companions: {diags:?}"
    );
    assert!(
        e059_diags_for(&diags, "HCS-P").iter().any(|_| true),
        "E059 must fire on US-classified HCS-P without NOFORN: {diags:?}"
    );
}

// ===========================================================================
// PR-D × PR-E overlap — class-floor + companion fire side-by-side
// ===========================================================================

#[test]
fn pr_d_class_floor_and_pr_e_companion_both_fire_distinctly() {
    // (S//HCS-O) — HCS-O on SECRET (S satisfies the S-floor for
    // class-floor/HCS-comp), missing ORCON + NOFORN.
    //
    // Expected:
    //  - PR D class-floor/HCS-comp does NOT fire (S meets the S-floor).
    //  - PR E HCS-O companions fires (Error no-fix because no IC
    //    dissem block exists).
    let diags = lint("(S//HCS-O)");
    let class_floor: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E058").collect();
    assert!(
        class_floor.is_empty(),
        "class-floor must not fire on S//HCS-O (S meets floor): {class_floor:?}"
    );
    let companion: Vec<_> = e059_diags_for(&diags, "HCS-O");
    assert_eq!(
        companion.len(),
        2,
        "PR E must fire for both ORCON and NOFORN missing: {companion:?}"
    );
}

#[test]
fn pr_d_class_floor_only_fires_when_companion_satisfied() {
    // (C//HCS-P//OC/NF) — HCS-P on CONFIDENTIAL (below S-floor),
    // companions correct.
    //
    // Expected:
    //  - PR D class-floor/HCS-comp fires (C below S-floor).
    //  - PR E HCS-P NOFORN does NOT fire (NOFORN present).
    let diags = lint("(C//HCS-P//OC/NF)");
    let class_floor: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.as_str() == "E058" && d.message.contains("HCS-O / HCS-P"))
        .collect();
    assert_eq!(
        class_floor.len(),
        1,
        "class-floor/HCS-comp must fire for C//HCS-P: {diags:?}"
    );
    let companion = e059_diags_for(&diags, "HCS-P");
    assert!(
        companion.is_empty(),
        "PR E HCS-P NOFORN must not fire when NOFORN present: {companion:?}"
    );
}

#[test]
fn pr_d_class_floor_and_pr_e_companion_both_fire_when_both_violated() {
    // (C//HCS-O) — HCS-O on CONFIDENTIAL (below S-floor), no
    // companions and no IC dissem block.
    //
    // Expected:
    //  - PR D class-floor/HCS-comp fires (C below S-floor).
    //  - PR E HCS-O companions fires for both ORCON and NOFORN
    //    (escalated to Error no-fix because no IC dissem block).
    let diags = lint("(C//HCS-O)");
    let class_floor: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E058").collect();
    assert!(
        !class_floor.is_empty(),
        "class-floor must fire on C//HCS-O: {diags:?}"
    );
    let companion: Vec<_> = e059_diags_for(&diags, "HCS-O");
    assert_eq!(
        companion.len(),
        2,
        "PR E must fire for both ORCON and NOFORN missing: {companion:?}"
    );
    for d in &companion {
        assert_eq!(d.severity, Severity::Error);
        assert!(d.fix.is_none());
    }
}

// ===========================================================================
// Banner-form rendering — full forms used when banner uses long form
// ===========================================================================

#[test]
fn rules_use_full_form_in_banner_when_dissem_is_full() {
    // Banner with full-form ORCON; missing NOFORN should insert
    // /NOFORN, not /NF.
    let diags = lint("SECRET//TK-BLFH//ORCON");
    let hits = e059_diags_for(&diags, "TK-{BLFH|IDIT|KAND}");
    assert_eq!(hits.len(), 1, "exactly one TK-NOFORN diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/NOFORN");
}

// ===========================================================================
// Severity::Off override — FR-008 invariant
// ===========================================================================

#[test]
fn e059_off_severity_skips_walker() {
    let mut config = Config::default();
    config
        .rules
        .overrides
        .insert("E059".to_owned(), "off".to_owned());
    let engine = Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine constructs");
    let diags = engine.lint(b"(S//HCS-O)").diagnostics;
    let hits: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E059").collect();
    assert!(
        hits.is_empty(),
        "E059 = off must suppress all PR-E diagnostics (FR-008): {hits:?}"
    );
}

// ===========================================================================
// Audit-stream traceability — per-row identifiable from diagnostic
// ===========================================================================

#[test]
fn e059_diagnostic_stream_per_row_identifiable() {
    // Lint a fixture exercising all 5 row violations; assert each
    // emitted message contains its row's marking label.
    //
    // Row #1 HCS-O: (S//HCS-O) — missing ORCON + NOFORN
    // Row #2 HCS-P: (S//HCS-P) — missing NOFORN
    // Row #3 HCS-P-sub: (TS//HCS-P JJJ//NF) — missing ORCON
    // Row #4 SI-G: (TS//SI-G//NF) — missing ORCON
    // Row #5 TK: (TS//TK-BLFH//OC) — missing NOFORN
    let cases: &[(&str, &str)] = &[
        ("(S//HCS-O)", "HCS-O"),
        ("(S//HCS-P)", "HCS-P"),
        ("(TS//HCS-P JJJ//NF)", "HCS-P sub-compartment"),
        ("(TS//SI-G//NF)", "SI-G"),
        ("(TS//TK-BLFH//OC)", "TK-{BLFH|IDIT|KAND}"),
    ];
    for (src, label) in cases {
        let diags = lint(src);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.rule.as_str() == "E059" && d.message.contains(label))
            .collect();
        assert!(
            !hits.is_empty(),
            "E059 must emit per-row identifiable diagnostic for {src:?} \
             (label {label:?}): {diags:?}"
        );
    }
}
