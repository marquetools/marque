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
/// touched, paired with a minimum-payload count so a future rule
/// retirement that hollows out an input fails this test loudly
/// rather than silently dropping coverage.
///
/// Each entry is `(input_bytes, min_payload_count, label)`. The
/// `min_payload_count` is the minimum number of `(FixIntent +
/// TextCorrection)` payloads the input must produce. Set to `0` for
/// inputs that are intentionally benign (covered elsewhere or gated
/// off by default). Adding a rule whose default behavior triggers on
/// one of these inputs requires no test change beyond bumping the
/// floor if appropriate; adding a rule whose trigger is not covered
/// by these inputs requires extending this list AND re-verifying the
/// invariant gate against the new trigger.
fn pr_a_triggering_inputs() -> &'static [(&'static [u8], usize, &'static str)] {
    &[
        // E002 — REL TO missing USA (FactAdd intent; strict 1.0).
        (b"SECRET//REL TO GBR", 1, "E002 missing-USA"),
        // E002 — USA not first (Recanonicalize intent; strict 1.0).
        (b"SECRET//REL TO GBR, USA", 1, "E002 USA-not-first"),
        // E007 — table-backed X-shorthand migration (text_correction).
        (b"SECRET//25X1-//NOFORN", 1, "E007 table-backed X-shorthand"),
        // E007 — pattern-fallback X-shorthand (text_correction).
        (b"SECRET//25X2-//NOFORN", 1, "E007 pattern-fallback X-shorthand"),
        // E021 — RD requires NOFORN (constraint-driven FactAdd).
        (b"SECRET//RD", 1, "E021 RD-requires-NOFORN"),
        // E066 — legacy NATO compound recanonicalization.
        (b"(//CTSA)", 1, "E066 legacy CTSA"),
        (b"(//NS-A)", 1, "E066 legacy NS-A"),
        // E071 — FGI explicit trigraph (Case A: full ⊆ REL TO).
        (b"(//FGI DEU R//REL TO USA, DEU)", 1, "E071 FGI Case A"),
        // E071 — FGI explicit trigraph (Case C: empty REL TO overlap).
        (b"(//FGI DEU R)", 1, "E071 FGI Case C"),
        // E071 — FGI explicit trigraph (Case D: partial overlap).
        (b"(//FGI DEU GBR R//REL TO USA, DEU)", 1, "E071 FGI Case D"),
        // S004 — REL TO trigraph suggest. The current corpus-calibrated
        // driver pair is `ASM → USA` (per `s004_coverage_exclusion.rs`);
        // the historical `AUT → AUS` example no longer clears
        // `SUGGEST_LOG_MARGIN` after prior re-stratification (#258).
        // DEU is a sibling that contributes no USA coverage so S004
        // fires on ASM.
        (b"(C//REL TO ASM, DEU)\n", 1, "S004 trigraph suggest"),
        // S007 — bare NATO requires REL TO USA, NATO (sibling US portion).
        (b"(//NS)\n(S//REL TO USA, FVEY)", 1, "S007 bare-NATO REL TO"),
        // S008 — RELIDO implied by closure (US classified + bare SCI
        // collateral form). Crafted to fire S008's projection-adds-
        // RELIDO branch.
        (b"(S//SI)", 1, "S008 RELIDO-implied-by-closure"),
        // S009 — prefer-tetragraph-collapse — default Off; emission path
        // stays gated by severity, so this input is benign for the
        // invariant scan. Coverage of the S009 emission path is owned
        // by `rel_to.rs::tests`.
        (b"SECRET//REL TO USA, AUS, CAN, GBR, NZL", 0, "S009 (default Off)"),
        // E062 — HCS supersession (bare HCS — Suggest with three
        // text-correction candidates per §H.4 p62).
        (b"(TS//HCS)", 1, "E062 HCS supersession"),
        // Companion-insertion path (`scheme/actions/companions.rs`)
        // — fires on HCS-P sub-compartment companions; bare HCS-P
        // triggers the ORCON companion-required path.
        (b"(TS//HCS-P)", 1, "companions HCS-P → ORCON"),
        // Non-IC dissem / classified-strip page-rewrite (Pattern C). A
        // banner-only `SECRET//LIMDIS` does not emit a payload-bearing
        // diagnostic on its own — the LIMDIS-evicted-by-classified
        // rewrite needs both an unclassified LIMDIS portion and a
        // classified portion to fire. Coverage of the Pattern-C
        // emission path is owned by `lattice_vs_scheme_parity.rs`.
        (b"(U//LIMDIS)\n(S)\n", 0, "non-IC dissem (LIMDIS, dedicated suite)"),
        // SCI per-system / SAR roll-up evaluator (banner-form). Banner
        // SAR-BP + portion SAR-CD = mismatch → rollup fires per the
        // §H.5 evaluator. (A matching banner/portion pair would not
        // exercise the rollup path.)
        (b"(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN\n", 1, "SAR rollup mismatch"),
        // C001 / corrections-map — no triggering input here. The
        // corrections map is config-driven; the rule's default-empty
        // map yields no diagnostics in this default-config test.
        // Coverage of the corrections-map path is owned by
        // `corrections_map.rs`.
    ]
}

#[test]
fn every_strict_path_rule_emits_confidence_strict_1_0() {
    let engine = engine();

    let mut violations: Vec<String> = Vec::new();
    let mut coverage_failures: Vec<String> = Vec::new();

    for (source, min_payload_count, label) in pr_a_triggering_inputs() {
        let lint_result = engine.lint(source);
        let mut input_payloads: usize = 0;
        for diag in &lint_result.diagnostics {
            // FixIntent payload — every emitted strict-path FixIntent
            // must carry `confidence.rule == 1.0`.
            if let Some(intent) = diag.fix.as_ref() {
                input_payloads += 1;
                let rule_axis = intent.confidence.rule;
                if (rule_axis - 1.0).abs() >= f32::EPSILON {
                    violations.push(format!(
                        "rule={} severity={:?} input={:?} ({label}): FixIntent \
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
                input_payloads += 1;
                let rule_axis = tc.confidence.rule;
                if (rule_axis - 1.0).abs() >= f32::EPSILON {
                    violations.push(format!(
                        "rule={} severity={:?} input={:?} ({label}): TextCorrection \
                         confidence.rule = {rule_axis} (expected 1.0)",
                        diag.rule,
                        diag.severity,
                        String::from_utf8_lossy(source),
                    ));
                }
            }
        }

        // Coverage gate: an input declared to produce N payloads must
        // actually produce at least N. A future rule retirement that
        // hollows out an input fails this assertion loudly so the
        // invariant gate does not silently lose discrimination.
        if input_payloads < *min_payload_count {
            coverage_failures.push(format!(
                "input={:?} ({label}): expected at least {} payload(s), got {}",
                String::from_utf8_lossy(source),
                min_payload_count,
                input_payloads,
            ));
        }
    }

    assert!(
        coverage_failures.is_empty(),
        "PR A invariant test lost discrimination on {} input(s):\n{}",
        coverage_failures.len(),
        coverage_failures.join("\n"),
    );
    assert!(
        violations.is_empty(),
        "strict-path `rule` confidence must equal 1.0 for every \
         emitted FixIntent / TextCorrection (PR A invariant); \
         {} violation(s):\n{}",
        violations.len(),
        violations.join("\n"),
    );
}
