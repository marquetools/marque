// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_rules::{Diagnostic, Severity};

fn engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Under the closed-template `Message` shape, per-row identification
/// via marker text (`"HCS-O"`, `"SI-G"`, etc.) is not available — the
/// runtime `marking_label` / `token_name` strings do not flow into
/// `Diagnostic.message`. The diagnostics carry the
/// `MessageTemplate::RequiredByPresence` template.
///
/// The `marker_text` parameter is kept for call-site documentation;
/// the filter only matches on `rule_id`.
fn e059_diags_for<'a>(
    diags: &'a [Diagnostic<CapcoScheme>],
    _marker_text: &str,
) -> Vec<&'a Diagnostic<CapcoScheme>> {
    // The bridge does not collapse to a single rule_id; each catalog row
    // emits its own predicate ID in the `marking.sci.<predicate>` form.
    // Filter on the substring prefix (mirrors
    // `is_sci_per_system_catalog_name`).
    diags
        .iter()
        .filter(|d| d.rule.predicate_id().starts_with("marking.sci."))
        .collect()
}

#[test]
fn hcs_o_no_dissem_still_emits_fixes() {
    let diags = lint("(S//HCS-O)");
    let hits = e059_diags_for(&diags, "HCS-O");
    assert_eq!(hits.len(), 2, "HCS-O must emit one per missing companion");
    for diag in hits {
        assert!(
            diag.fix.is_some(),
            "missing dissem block should still be fixable"
        );
        assert_eq!(diag.severity, Severity::Warn);
    }
}

#[test]
fn hcs_p_no_dissem_still_emits_fix() {
    let diags = lint("(S//HCS-P)");
    let hits = e059_diags_for(&diags, "HCS-P requires NOFORN");
    assert_eq!(hits.len(), 1, "expected one HCS-P NOFORN diagnostic");
    assert!(
        hits[0].fix.is_some(),
        "missing dissem block should still be fixable"
    );
    assert_eq!(hits[0].severity, Severity::Warn);
}

#[test]
fn si_g_no_dissem_still_emits_fix() {
    let diags = lint("(TS//SI-G)");
    let hits = e059_diags_for(&diags, "SI-G requires ORCON");
    assert_eq!(hits.len(), 1, "expected one SI-G ORCON diagnostic");
    assert!(
        hits[0].fix.is_some(),
        "missing dissem block should still be fixable"
    );
    assert_eq!(hits[0].severity, Severity::Warn);
}
