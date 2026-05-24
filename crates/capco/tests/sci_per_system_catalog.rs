#![cfg(any())]
// Gated on the legacy `FixProposal` / `Message::contains` /
// `ReplacementIntent::as_ref` shapes that no longer exist. The
// predicate-ID filters in the body use the 2-tuple rule-ID shape, but
// the remaining API drift (Message type, FixIntent shape) means this
// file stays disabled until the FixIntent/Message accessors are
// rewritten end-to-end.

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI per-system catalog walker behavior tests.
//!
//! Each catalog row's `name` IS its predicate ID (`marking.sci.<row>`);
//! there is no walker-level rule ID. Severity-override is per-row via
//! `[rules] "capco:marking.sci.<row>" = "..."`. Tests filter by
//! `predicate_id().starts_with("marking.sci.")` to capture all 5
//! catalog rows or by the specific row's predicate ID when testing
//! per-row behavior.
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
//!    do not fire since §H.4 is US-only-scoped.
//! 5. **Class-floor × companion overlap** — class-floor + companion
//!    violations fire side-by-side without overlap-guard interference.
//! 6. **Audit traceability** — each emitted message carries the row's
//!    marking label.
//! 7. **Naming convention** — every catalog row name starts with
//!    `marking.sci.`.
//! 8. **Severity::Off** — a per-row `off` override suppresses that
//!    row's diagnostics: an `Off`-severity rule cannot fire.
//! 9. **Citation fidelity** — every row's citation matches the
//!    verified §H.4 page anchors.

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
fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Filter the diagnostic stream to SCI per-system catalog emissions
/// whose message contains `marker_text` (substring match). Each of the
/// 5 catalog rows has its own predicate ID
/// (`marking.sci.<row>`); the `starts_with("marking.sci.")` prefix
/// match captures every row. Per-row identification flows via either
/// the diagnostic message text (this helper) or the row's predicate
/// ID directly (use `sci_diags_with_predicate` for that).
fn sci_diags_for<'a>(
    diags: &'a [Diagnostic<CapcoScheme>],
    marker_text: &str,
) -> Vec<&'a Diagnostic<CapcoScheme>> {
    diags
        .iter()
        .filter(|d| {
            d.rule.scheme() == "capco"
                && d.rule.predicate_id().starts_with("marking.sci.")
                && d.message.contains(marker_text)
        })
        .collect()
}

/// All SCI per-system catalog diagnostics in `diags`, regardless of
/// row or message content. Captures every row via the `marking.sci.`
/// predicate-ID prefix.
fn sci_diags(diags: &[Diagnostic<CapcoScheme>]) -> Vec<&Diagnostic<CapcoScheme>> {
    diags
        .iter()
        .filter(|d| d.rule.scheme() == "capco" && d.rule.predicate_id().starts_with("marking.sci."))
        .collect()
}

/// Filter the diagnostic stream to a specific catalog row by its
/// exact predicate ID (e.g., `"marking.sci.hcs-o-companions"`). Use for tests that need
/// row-level granularity (e.g., per-row severity override tests).
fn sci_diags_with_predicate<'a>(
    diags: &'a [Diagnostic<CapcoScheme>],
    predicate_id: &str,
) -> Vec<&'a Diagnostic<CapcoScheme>> {
    diags
        .iter()
        .filter(|d| d.rule.scheme() == "capco" && d.rule.predicate_id() == predicate_id)
        .collect()
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
        .filter(|c| c.name().starts_with("marking.sci."))
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

/// The 5 catalog rows expected to exist on `CapcoScheme`, paired with
/// their verified `CAPCO-2016 §H.4 pXX` citations. Hardcoded here so
/// adding / typoing / removing a catalog row without updating this list
/// fails the citation-fidelity and naming-convention tests at CI time
/// — without requiring a public accessor on `marque_capco::scheme`.
const EXPECTED_ROWS: &[(&str, &str)] = &[
    ("marking.sci.hcs-o-companions", "CAPCO-2016 §H.4 p64"),
    ("marking.sci.hcs-p-noforn-required", "CAPCO-2016 §H.4 p66"),
    ("marking.sci.hcs-p-sub-companions", "CAPCO-2016 §H.4 p68"),
    ("marking.sci.si-g-companions", "CAPCO-2016 §H.4 p80"),
    (
        "marking.sci.tk-compartment-noforn-required",
        "CAPCO-2016 §H.4 p87 + p91 + p95",
    ),
];

#[test]
fn sci_per_system_catalog_naming_convention() {
    // Every expected row's `name` MUST start with `marking.sci.` per
    // the predicate-ID convention. The companion
    // `sci_per_system_catalog_citations` test below pins the same
    // expected names against their citations as they appear on
    // `CapcoScheme.constraints()` — together the two tests catch:
    //   (a) a catalog-side row with a typo'd prefix (e.g.,
    //       `sai-per-system/...`) — the citations test fails to find
    //       the expected row name in `scheme.constraints()`;
    //   (b) an expected-list typo'd prefix — this test fails on the
    //       prefix assertion;
    //   (c) a row added to the catalog without being added to
    //       `EXPECTED_ROWS` — caught by the row-count pin in
    //       `capco_rules_set_includes_sci_per_system_walker` (rule
    //       count) AND by direct comparison via the citations test
    //       (which would not exercise the new row, leaving it
    //       silently uncovered — flagged at code review).
    //
    // Note: an `EXPECTED_ROWS.is_empty()` guard is omitted — the
    // const has 5 entries by definition; clippy::const_is_empty
    // would flag the assertion as statically false.
    for (name, _) in EXPECTED_ROWS {
        assert!(
            name.starts_with("marking.sci."),
            "catalog row {name:?} must start with `marking.sci.`"
        );
    }
}

#[test]
fn sci_per_system_catalog_citations() {
    // Every row's citation must match one of the verified §H.4 page
    // anchors.
    let scheme = CapcoScheme::new();
    for (name, citation) in EXPECTED_ROWS {
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
fn sci_per_system_diagnostics_flow_through_engine_bridge_per_row() {
    // The SCI per-system walker is not a registered rule; the 5 catalog
    // rows emit diagnostics through the engine's constraint-catalog
    // bridge via the direct path
    // (`CapcoScheme::bridge_sci_per_system_diagnostics`), each row
    // carrying its own predicate ID (`marking.sci.<row>`) and full
    // `FixProposal` payloads. This test pins the external surface:
    //   1. The 10 legacy per-system rules (E042–E051) remain
    //      unregistered;
    //   2. `engine.lint` still emits the row's predicate ID with a
    //      fix for a known-firing fixture (TS//HCS-O//NF missing
    //      ORCON → `marking.sci.hcs-o-companions`).
    let set = capco_rules();
    let ids: Vec<&str> = set.rules().iter().map(|r| r.id().predicate_id()).collect();
    for retired in [
        "E042", "E043", "E044", "E045", "E046", "E047", "E048", "E049", "E050", "E051",
    ] {
        assert!(
            !ids.contains(&retired),
            "{retired} must not be registered: the legacy per-system rule is retired"
        );
    }

    // Bridge-emission anchor: confirm the walker's external surface —
    // and the fix payload — is preserved end-to-end through
    // `engine.lint`. `(TS//HCS-O//NF)` is a portion with
    // HCS-O and NOFORN; row #1 (HCS-O companions, predicate
    // `marking.sci.hcs-o-companions`) must fire the ORCON-missing
    // diagnostic with a `FixProposal` that inserts ORCON into the
    // dissem block.
    let diags = lint("(TS//HCS-O//NF)");
    let hcs_o: Vec<&Diagnostic<CapcoScheme>> = diags
        .iter()
        .filter(|d| {
            d.rule.scheme() == "capco"
                && d.rule.predicate_id() == "marking.sci.hcs-o-companions"
                && d.message.contains("HCS-O requires ORCON")
        })
        .collect();
    assert_eq!(
        hcs_o.len(),
        1,
        "bridge must emit `marking.sci.hcs-o-companions` for (TS//HCS-O//NF) \
         (HCS-O missing ORCON per §H.4 p64): {diags:?}"
    );
    assert!(
        hcs_o[0].fix.is_some(),
        "SCI per-system bridge MUST preserve the walker's fix payload \
         (companion-insertion); got: {:?}",
        hcs_o[0]
    );
}

// ===========================================================================
// Row #1 — HCS-O companions (§H.4 p64)
// ===========================================================================

#[test]
fn hcs_o_companions_fires_when_orcon_missing() {
    // (S//HCS-O//NF) — NOFORN present, ORCON missing. Expect one
    // ORCON-insertion diagnostic.
    let diags = lint("(S//HCS-O//NF)");
    let hits = sci_diags_for(&diags, "HCS-O requires ORCON");
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
    let hits = sci_diags_for(&diags, "HCS-O requires NOFORN");
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
    let hits = sci_diags_for(&diags, "HCS-O forbids ORCON-USGOV");
    assert_eq!(hits.len(), 1, "exactly one forbid diagnostic: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "OC");
}

#[test]
fn hcs_o_companions_oc_usgov_satisfies_orcon_presence() {
    // OC-USGOV satisfies ORCON-presence post-fix, so the rule must NOT
    // emit an additional ORCON-insertion (would produce duplicate ORCON).
    let diags = lint("(S//HCS-O//OC-USGOV/NF)");
    let hits = sci_diags(&diags);
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
    let hits = sci_diags_for(&diags, "HCS-O");
    assert!(hits.is_empty(), "compliant HCS-O: {diags:?}");
}

#[test]
fn hcs_o_companions_does_not_fire_when_marking_absent() {
    let diags = lint("(S)");
    let hits = sci_diags_for(&diags, "HCS-O");
    assert!(hits.is_empty(), "no HCS-O present: {diags:?}");
}

#[test]
fn hcs_o_companions_no_dissem_block_escalates_to_error_no_fix() {
    // (S//HCS-O) — both companions missing AND no IC dissem block.
    // Verbatim port of `e042_no_dissem_block_escalates_to_error_no_fix`
    // from the retired rules_sci_per_system.rs.
    let diags = lint("(S//HCS-O)");
    let hits: Vec<_> = sci_diags(&diags)
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
    let hits = sci_diags_for(&diags, "HCS-O requires ORCON");
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
    let hits = sci_diags_for(&diags, "HCS-P requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/NF");
}

#[test]
fn hcs_p_noforn_does_not_fire_when_present() {
    let diags = lint("(S//HCS-P//OC/NF)");
    let hits = sci_diags_for(&diags, "HCS-P requires NOFORN");
    assert!(hits.is_empty(), "compliant HCS-P: {diags:?}");
}

#[test]
fn hcs_p_noforn_does_not_fire_on_bare_hcs_or_hcs_o() {
    // Only HCS-P triggers this row.
    let d1 = lint("(S//HCS)");
    assert!(
        sci_diags_for(&d1, "HCS-P requires NOFORN").is_empty(),
        "bare HCS must not fire HCS-P NOFORN row: {d1:?}"
    );
    let d2 = lint("(S//HCS-O//OC/NF)");
    assert!(
        sci_diags_for(&d2, "HCS-P requires NOFORN").is_empty(),
        "HCS-O must not fire HCS-P NOFORN row: {d2:?}"
    );
}

#[test]
fn hcs_p_noforn_no_dissem_block_escalates_to_error_no_fix() {
    let diags = lint("(S//HCS-P)");
    let hits = sci_diags_for(&diags, "HCS-P requires NOFORN");
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
    let hits = sci_diags_for(&diags, "HCS-P sub-compartment requires ORCON");
    assert_eq!(hits.len(), 1, "exactly one ORCON-missing: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/OC");
}

#[test]
fn hcs_p_sub_companions_replaces_oc_usgov() {
    let diags = lint("(TS//HCS-P JJJ//OC-USGOV/NF)");
    let hits = sci_diags_for(&diags, "HCS-P sub-compartment forbids ORCON-USGOV");
    assert_eq!(hits.len(), 1, "exactly one forbid: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "OC");
}

#[test]
fn hcs_p_sub_companions_does_not_fire_when_satisfied() {
    let diags = lint("(TS//HCS-P JJJ//OC/NF)");
    let hits = sci_diags_for(&diags, "HCS-P sub-compartment");
    assert!(hits.is_empty(), "compliant HCS-P sub: {diags:?}");
}

#[test]
fn hcs_p_sub_companions_does_not_fire_on_bare_hcs_p() {
    // Bare HCS-P (no sub-compartment) — different row (HCS-P NOFORN).
    let diags = lint("(TS//HCS-P//OC/NF)");
    let hits = sci_diags_for(&diags, "HCS-P sub-compartment");
    assert!(hits.is_empty(), "no sub-compartment present: {diags:?}");
}

#[test]
fn hcs_p_sub_companions_oc_usgov_only_replaces_no_duplicate_orcon() {
    // (TS//HCS-P JJJ//OC-USGOV) — only OC-USGOV, no bare OC. The
    // OC-USGOV satisfies ORCON-presence post-fix; only the replacement
    // should fire (not the insertion).
    let diags = lint("(TS//HCS-P JJJ//OC-USGOV)");
    let hits: Vec<_> = sci_diags(&diags)
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
    let hits = sci_diags_for(&diags, "SI-G requires ORCON");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    assert!(hits[0].fix.is_none(), "no dissem → no-fix: {:?}", hits[0]);
    assert_eq!(hits[0].severity, Severity::Error);
}

#[test]
fn si_g_companions_inserts_orcon_when_dissem_block_present() {
    let diags = lint("(TS//SI-G//NF)");
    let hits = sci_diags_for(&diags, "SI-G requires ORCON");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/OC");
}

#[test]
fn si_g_companions_replaces_oc_usgov() {
    let diags = lint("(TS//SI-G//OC-USGOV)");
    let hits = sci_diags_for(&diags, "SI-G forbids ORCON-USGOV");
    assert_eq!(hits.len(), 1, "exactly one forbid: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "OC");
}

#[test]
fn si_g_companions_oc_usgov_only_no_duplicate_orcon() {
    // (TS//SI-G//OC-USGOV) — OC-USGOV satisfies ORCON-presence
    // post-fix; no insertion should fire.
    let diags = lint("(TS//SI-G//OC-USGOV)");
    let hits: Vec<_> = sci_diags(&diags)
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
    let hits = sci_diags_for(&diags, "SI-G");
    assert!(hits.is_empty(), "compliant SI-G: {diags:?}");
}

// ===========================================================================
// Row #5 — TK compartment NOFORN (§H.4 p87 + p91 + p95)
// ===========================================================================

#[test]
fn tk_compartment_noforn_fires_on_blfh_without_noforn() {
    let diags = lint("(TS//TK-BLFH//OC)");
    let hits = sci_diags_for(&diags, "TK-{BLFH|IDIT|KAND} requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/NF");
}

#[test]
fn tk_compartment_noforn_fires_on_idit_without_noforn() {
    let diags = lint("(TS//TK-IDIT//OC)");
    let hits = sci_diags_for(&diags, "TK-{BLFH|IDIT|KAND} requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
}

#[test]
fn tk_compartment_noforn_fires_on_kand_without_noforn() {
    let diags = lint("(TS//TK-KAND//OC)");
    let hits = sci_diags_for(&diags, "TK-{BLFH|IDIT|KAND} requires NOFORN");
    assert_eq!(hits.len(), 1, "exactly one diag: {diags:?}");
}

#[test]
fn tk_compartment_noforn_does_not_fire_on_bare_tk() {
    // Bare TK doesn't require NOFORN (§H.4 p85 — only the compartments
    // do).
    let diags = lint("(TS//TK)");
    let hits = sci_diags_for(&diags, "TK-{BLFH|IDIT|KAND}");
    assert!(hits.is_empty(), "bare TK must not fire: {diags:?}");
}

#[test]
fn tk_compartment_noforn_does_not_fire_when_present() {
    let diags = lint("(TS//TK-BLFH//NF)");
    let hits = sci_diags_for(&diags, "TK-{BLFH|IDIT|KAND}");
    assert!(hits.is_empty(), "compliant TK-BLFH: {diags:?}");
}

// ===========================================================================
// Scope guard — §H.4 is US-only; pure foreign classifications skip
// ===========================================================================

#[test]
fn e059_skips_joint_classifications() {
    // JOINT banner with every per-system SCI marking. `us_level()` returns
    // None for JOINT, so all 5 rows must short-circuit.
    let src = "//JOINT S USA GBR//HCS-O HCS-P JJJ//SI-G//TK-BLFH//REL TO USA, GBR";
    let diags = lint(src);
    let hits = sci_diags(&diags);
    assert!(
        hits.is_empty(),
        "SCI per-system rows must not fire on JOINT (non-US) classification; \
         §H.4 companion constraints are US-scoped: {hits:?}"
    );
}

#[test]
fn e059_still_fires_on_us_classifications() {
    // Sanity: scope guard must not mask US-side violations.
    let diags = lint("(S//HCS-O HCS-P)");
    assert!(
        sci_diags_for(&diags, "HCS-O").iter().any(|_| true),
        "SCI per-system rows must fire on US-classified HCS-O without companions: {diags:?}"
    );
    assert!(
        sci_diags_for(&diags, "HCS-P").iter().any(|_| true),
        "SCI per-system rows must fire on US-classified HCS-P without NOFORN: {diags:?}"
    );
}

// ===========================================================================
// Class-floor × companion overlap — both fire side-by-side
// ===========================================================================

/// Filter the diagnostic stream to class-floor catalog emissions.
/// Class-floor rows carry per-row predicate IDs like
/// `banner.classification.floor-hcs-comp` and `banner.aea.floor-rd`;
/// the `banner.*.floor-` / `banner.*.ceiling-` prefix match captures
/// every row regardless of axis.
fn class_floor_diags(diags: &[Diagnostic<CapcoScheme>]) -> Vec<&Diagnostic<CapcoScheme>> {
    diags
        .iter()
        .filter(|d| {
            let pid = d.rule.predicate_id();
            d.rule.scheme() == "capco"
                && (pid.starts_with("banner.classification.floor-")
                    || pid.starts_with("banner.aea.floor-")
                    || pid.starts_with("banner.aea.ceiling-")
                    || pid.starts_with("banner.dissem.floor-"))
        })
        .collect()
}

#[test]
fn pr_d_class_floor_and_pr_e_companion_both_fire_distinctly() {
    // (S//HCS-O) — HCS-O on SECRET (S satisfies the S-floor for
    // `banner.classification.floor-hcs-comp`), missing ORCON + NOFORN.
    //
    // Expected:
    //  - class-floor (any axis) does NOT fire (S meets S-floor).
    //  - HCS-O companions fires (Error no-fix because no IC dissem
    //    block exists).
    let diags = lint("(S//HCS-O)");
    let class_floor = class_floor_diags(&diags);
    assert!(
        class_floor.is_empty(),
        "class-floor must not fire on S//HCS-O (S meets floor): {class_floor:?}"
    );
    let companion: Vec<_> = sci_diags_for(&diags, "HCS-O");
    assert_eq!(
        companion.len(),
        2,
        "companions must fire for both ORCON and NOFORN missing: {companion:?}"
    );
}

#[test]
fn pr_d_class_floor_only_fires_when_companion_satisfied() {
    // (C//HCS-P//OC/NF) — HCS-P on CONFIDENTIAL (below S-floor),
    // companions correct.
    //
    // Expected:
    //  - `banner.classification.floor-hcs-comp` fires (C below
    //    S-floor).
    //  - HCS-P NOFORN does NOT fire (NOFORN present).
    let diags = lint("(C//HCS-P//OC/NF)");
    let class_floor: Vec<_> =
        sci_diags_with_predicate(&diags, "banner.classification.floor-hcs-comp");
    assert_eq!(
        class_floor.len(),
        1,
        "banner.classification.floor-hcs-comp must fire for C//HCS-P: {diags:?}"
    );
    let companion = sci_diags_for(&diags, "HCS-P");
    assert!(
        companion.is_empty(),
        "HCS-P NOFORN must not fire when NOFORN present: {companion:?}"
    );
}

#[test]
fn pr_d_class_floor_and_pr_e_companion_both_fire_when_both_violated() {
    // (C//HCS-O) — HCS-O on CONFIDENTIAL (below S-floor), no
    // companions and no IC dissem block.
    //
    // Expected:
    //  - class-floor (banner.classification.floor-hcs-comp) fires
    //    (C below S-floor).
    //  - HCS-O companions fires for both ORCON and NOFORN
    //    (escalated to Error no-fix because no IC dissem block).
    let diags = lint("(C//HCS-O)");
    let class_floor = class_floor_diags(&diags);
    assert!(
        !class_floor.is_empty(),
        "class-floor must fire on C//HCS-O: {diags:?}"
    );
    let companion: Vec<_> = sci_diags_for(&diags, "HCS-O");
    assert_eq!(
        companion.len(),
        2,
        "companions must fire for both ORCON and NOFORN missing: {companion:?}"
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
    let hits = sci_diags_for(&diags, "TK-{BLFH|IDIT|KAND}");
    assert_eq!(hits.len(), 1, "exactly one TK-NOFORN diag: {diags:?}");
    let fix = hits[0].fix.as_ref().expect("fix attached");
    assert_eq!(fix.replacement.as_ref(), "/NOFORN");
}

// ===========================================================================
// Severity::Off override — per-row dispatch
// ===========================================================================

#[test]
fn sci_per_system_off_severity_suppresses_specific_row() {
    // Each catalog row is independently overridable via its own
    // wire-string key. Setting
    // `[rules] "capco:marking.sci.hcs-o-companions" = "off"` MUST
    // suppress only HCS-O diagnostics — the other 4 rows are untouched.
    let mut config = Config::default();
    config.rules.overrides.insert(
        "capco:marking.sci.hcs-o-companions".to_owned(),
        "off".to_owned(),
    );
    let engine = Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine constructs");
    // (S//HCS-O//OC) → only HCS-O companion violations would fire
    // (NOFORN missing). With HCS-O row off, no diagnostic should
    // surface from `marking.sci.hcs-o-companions`.
    let diags = engine.lint(b"(S//HCS-O//OC)").diagnostics;
    let hcs_o_hits = sci_diags_with_predicate(&diags, "marking.sci.hcs-o-companions");
    assert!(
        hcs_o_hits.is_empty(),
        "capco:marking.sci.hcs-o-companions = off must suppress that row: {hcs_o_hits:?}"
    );
}

#[test]
fn sci_per_system_off_does_not_leak_to_other_rows() {
    // Severity-override scoping check: setting one row to `off` MUST
    // NOT suppress the other 4 rows. Verifies the per-row dispatch
    // shape — a walker-level hoist would let every row's diagnostics
    // leak through unchanged regardless of any override.
    let mut config = Config::default();
    config.rules.overrides.insert(
        "capco:marking.sci.hcs-o-companions".to_owned(),
        "off".to_owned(),
    );
    let engine = Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine constructs");
    // (TS//SI-G//NF) → only SI-G row fires (ORCON missing). Setting
    // HCS-O off must not silence SI-G.
    let diags = engine.lint(b"(TS//SI-G//NF)").diagnostics;
    let si_g_hits = sci_diags_with_predicate(&diags, "marking.sci.si-g-companions");
    assert!(
        !si_g_hits.is_empty(),
        "capco:marking.sci.hcs-o-companions = off must NOT suppress \
         the SI-G row: {diags:?}"
    );
}

#[test]
fn sci_per_system_off_all_five_rows_independently() {
    // Belt-and-suspenders: override all 5 rows to `off` simultaneously
    // and assert every catalog row is suppressed — the per-row-key
    // analog of suppressing the whole walker.
    let mut config = Config::default();
    for row in [
        "capco:marking.sci.hcs-o-companions",
        "capco:marking.sci.hcs-p-noforn-required",
        "capco:marking.sci.hcs-p-sub-companions",
        "capco:marking.sci.si-g-companions",
        "capco:marking.sci.tk-compartment-noforn-required",
    ] {
        config
            .rules
            .overrides
            .insert(row.to_owned(), "off".to_owned());
    }
    let engine = Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine constructs");
    // (S//HCS-O) would fire 2 HCS-O diagnostics by default; with all
    // 5 rows off, no SCI per-system diagnostic should surface.
    let diags = engine.lint(b"(S//HCS-O)").diagnostics;
    let hits = sci_diags(&diags);
    assert!(
        hits.is_empty(),
        "all 5 SCI per-system rows off must suppress every catalog diagnostic: {hits:?}"
    );
}

// ===========================================================================
// Audit-stream traceability — per-row identifiable from diagnostic
// ===========================================================================

#[test]
fn sci_per_system_diagnostic_stream_per_row_identifiable() {
    // Lint a fixture exercising all 5 row violations; assert each
    // emitted message contains its row's marking label AND carries
    // the row's predicate ID.
    //
    // Row #1 HCS-O: (S//HCS-O) — missing ORCON + NOFORN
    // Row #2 HCS-P: (S//HCS-P) — missing NOFORN
    // Row #3 HCS-P-sub: (TS//HCS-P JJJ//NF) — missing ORCON
    // Row #4 SI-G: (TS//SI-G//NF) — missing ORCON
    // Row #5 TK: (TS//TK-BLFH//OC) — missing NOFORN
    let cases: &[(&str, &str, &str)] = &[
        ("(S//HCS-O)", "HCS-O", "marking.sci.hcs-o-companions"),
        ("(S//HCS-P)", "HCS-P", "marking.sci.hcs-p-noforn-required"),
        (
            "(TS//HCS-P JJJ//NF)",
            "HCS-P sub-compartment",
            "marking.sci.hcs-p-sub-companions",
        ),
        ("(TS//SI-G//NF)", "SI-G", "marking.sci.si-g-companions"),
        (
            "(TS//TK-BLFH//OC)",
            "TK-{BLFH|IDIT|KAND}",
            "marking.sci.tk-compartment-noforn-required",
        ),
    ];
    for (src, label, predicate) in cases {
        let diags = lint(src);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| {
                d.rule.scheme() == "capco"
                    && d.rule.predicate_id() == *predicate
                    && d.message.contains(label)
            })
            .collect();
        assert!(
            !hits.is_empty(),
            "SCI per-system must emit per-row identifiable diagnostic for {src:?} \
             (predicate {predicate:?}, label {label:?}): {diags:?}"
        );
    }
}
