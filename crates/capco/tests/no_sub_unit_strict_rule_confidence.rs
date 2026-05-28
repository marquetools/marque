// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR A invariant: every strict-path `rule` confidence is `1.0`.
//!
//! PR A collapsed every CAPCO rule's `Confidence::strict(...)`
//! emission to `1.0`. The intent is that only the decoder path
//! produces sub-1.0 `rule` confidence; hand-written strict-path rules
//! emit at `Confidence::strict(1.0)` everywhere. PR B will retire the
//! `rule` axis entirely (coordinated with an audit-schema bump).
//!
//! This test is the regression gate that prevents the invariant from
//! drifting between now and PR B. It runs the full default
//! `CapcoRuleSet` against a small set of fixtures chosen to trigger
//! each migrated rule and asserts every emitted
//! `confidence.rule` equals `1.0` within `f32::EPSILON`.
//!
//! Fixtures are inlined rather than loaded from `tests/fixtures/`
//! because the discrimination point is per-rule confidence emission,
//! not per-fixture corpus parity — the inlined bytes carry the rule
//! trigger and nothing else, so a future fixture-rename does not
//! affect this gate.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Inlined inputs covering each strict-path rule the PR-A migration
/// touched. Adding a rule whose default behavior triggers on one of
/// these inputs requires no test change; adding a rule whose trigger
/// is not covered by these inputs requires extending this list AND
/// re-verifying the invariant gate against the new trigger.
fn pr_a_triggering_inputs() -> &'static [&'static [u8]] {
    &[
        // E002 — REL TO missing USA (FactAdd intent; strict 1.0).
        b"SECRET//REL TO GBR",
        // E002 — USA not first (Recanonicalize intent; strict 1.0).
        b"SECRET//REL TO GBR, USA",
        // E007 — table-backed X-shorthand migration (text_correction).
        b"SECRET//25X1-//NOFORN",
        // E007 — pattern-fallback X-shorthand (text_correction).
        b"SECRET//25X2-//NOFORN",
        // E021 — RD requires NOFORN (constraint-driven FactAdd).
        b"SECRET//RD",
        // E066 — legacy NATO compound recanonicalization.
        b"(//CTSA)",
        b"(//NS-A)",
        // E071 — FGI explicit trigraph (Case A: full ⊆ REL TO).
        b"(//FGI DEU R//REL TO USA, DEU)",
        // E071 — FGI explicit trigraph (Case C: empty REL TO overlap).
        b"(//FGI DEU R)",
        // E071 — FGI explicit trigraph (Case D: partial overlap).
        b"(//FGI DEU GBR R//REL TO USA, DEU)",
        // S004 — REL TO trigraph suggest (AUT → AUS).
        b"SECRET//REL TO USA, AUT",
        // S007 — bare NATO requires REL TO USA, NATO (sibling US portion).
        b"(//NS)\n(S//REL TO USA, FVEY)",
        // S008 — RELIDO implied by closure (US classified + bare SCI
        // collateral form). Crafted to fire S008's projection-adds-
        // RELIDO branch.
        b"(S//SI)",
        // S009 — prefer-tetragraph-collapse (default Off; the rule's
        // emission path stays gated, so this input is benign for the
        // invariant scan).
        b"SECRET//REL TO USA, AUS, CAN, GBR, NZL",
        // C001 / corrections-map — no triggering input here (the
        // corrections map is config-driven; the rule's default-empty
        // map yields no diagnostics in this default-config test).
        // Coverage of the corrections-map path is owned by
        // `corrections_map.rs`.
        // E062 — HCS supersession (bare HCS — Suggest with three
        // text-correction candidates per §H.4 p62).
        b"(TS//HCS)",
        // Companion-insertion path (`scheme/actions/companions.rs`)
        // — fires on HCS-P sub-compartment companions; bare HCS-P
        // triggers the ORCON companion-required path.
        b"(TS//HCS-P)",
        // Dissem-closure / non-IC dissem evaluator paths.
        b"SECRET//LIMDIS",
        // SCI per-system / SAR roll-up evaluators (banner-form).
        b"SECRET//SAR-FOO\n(S//SAR-FOO)",
    ]
}

#[test]
fn every_strict_path_rule_emits_confidence_strict_1_0() {
    let engine = engine();

    let mut violations: Vec<String> = Vec::new();

    for source in pr_a_triggering_inputs() {
        let lint_result = engine.lint(source);
        for diag in &lint_result.diagnostics {
            // FixIntent payload — every emitted strict-path FixIntent
            // must carry `confidence.rule == 1.0`.
            if let Some(intent) = diag.fix.as_ref() {
                let rule_axis = intent.confidence.rule;
                if (rule_axis - 1.0).abs() >= f32::EPSILON {
                    violations.push(format!(
                        "rule={} severity={:?} input={:?}: FixIntent \
                         confidence.rule = {rule_axis} (expected 1.0)",
                        diag.rule,
                        diag.severity,
                        String::from_utf8_lossy(source),
                    ));
                }
            }
            // TextCorrection payload — same invariant on the
            // text-correction channel.
            if let Some(tc) = diag.text_correction.as_ref() {
                let rule_axis = tc.confidence.rule;
                if (rule_axis - 1.0).abs() >= f32::EPSILON {
                    violations.push(format!(
                        "rule={} severity={:?} input={:?}: TextCorrection \
                         confidence.rule = {rule_axis} (expected 1.0)",
                        diag.rule,
                        diag.severity,
                        String::from_utf8_lossy(source),
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "strict-path `rule` confidence must equal 1.0 for every \
         emitted FixIntent / TextCorrection (PR A invariant); \
         {} violation(s):\n{}",
        violations.len(),
        violations.join("\n"),
    );
}
