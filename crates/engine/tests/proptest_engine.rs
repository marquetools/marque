// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based engine invariant tests.
//!
//! Covers never-panic, span bounds, idempotency, dry-run parity,
//! confidence bounds, and threshold enforcement over generated structured
//! inputs (valid and near-valid CAPCO marking strings up to 4 KB).

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use proptest::prelude::*;
use secrecy::ExposeSecret as _;
use std::sync::OnceLock;

// Build the engine once per test binary; constructing the Aho-Corasick
// automaton on every proptest case would dominate the runtime.
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        Engine::new(
            Config::default(),
            vec![Box::new(CapcoRuleSet::new())],
            marque_engine::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
    })
}

// ---------------------------------------------------------------------------
// Structured source generator
// ---------------------------------------------------------------------------

static CLASSIFICATIONS: &[&str] = &["TOP SECRET", "SECRET", "CONFIDENTIAL", "UNCLASSIFIED"];

static SCI_BLOCKS: &[&str] = &["", "//SI", "//TK", "//SI-G", "//HCS"];

static DISSEM_BLOCKS: &[&str] = &[
    "",
    "//NOFORN",
    "//REL TO USA, GBR",
    "//RELIDO",
    "//NOFORN//RELIDO",
];

fn arb_banner() -> impl Strategy<Value = String> {
    (
        0..CLASSIFICATIONS.len(),
        0..SCI_BLOCKS.len(),
        0..DISSEM_BLOCKS.len(),
    )
        .prop_map(|(ci, si, di)| {
            format!(
                "{}{}{}\n",
                CLASSIFICATIONS[ci], SCI_BLOCKS[si], DISSEM_BLOCKS[di]
            )
        })
}

fn arb_portion() -> impl Strategy<Value = String> {
    (
        prop_oneof![Just("TS"), Just("S"), Just("C"), Just("U"),],
        prop_oneof![Just(""), Just("//SI"), Just("//TK"), Just("//HCS"),],
        prop_oneof![Just(""), Just("//NF"), Just("//REL TO USA, GBR"),],
    )
        .prop_map(|(cls, sci, dissem)| format!("({}{}{})", cls, sci, dissem))
}

fn arb_source() -> impl Strategy<Value = String> {
    prop_oneof![
        // Well-formed banner only
        arb_banner(),
        // Banner followed by a portion marking
        (arb_banner(), arb_portion()).prop_map(|(b, p)| format!("{b}{p}\n")),
        // Multiple portions then a banner — deliberate violation (banner
        // at end instead of start) for near-valid input coverage.
        (arb_portion(), arb_portion(), arb_banner())
            .prop_map(|(p1, p2, b)| format!("{p1} {p2}\n{b}")),
        // Plain portion only
        arb_portion().prop_map(|p| format!("{p}\n")),
        // Multi-KB: banner + many portions to exercise multi-KB code paths
        // (100–300 portions × ~15 bytes each ≈ 1.5–4.5 KB).
        (
            arb_banner(),
            proptest::collection::vec(arb_portion(), 100..=300)
        )
            .prop_map(|(b, ps)| {
                let mut s = b;
                for p in &ps {
                    s.push_str(p);
                    s.push('\n');
                }
                s
            }),
    ]
}

// ---------------------------------------------------------------------------
// Invariants
// ---------------------------------------------------------------------------

proptest! {
    // Never panic on any valid UTF-8 input.
    #[test]
    fn never_panic(src in arb_source()) {
        let _ = engine().lint(src.as_bytes());
    }

    // Every diagnostic span satisfies start <= end && end <= source.len().
    #[test]
    fn span_bounds(src in arb_source()) {
        let bytes = src.as_bytes();
        let result = engine().lint(bytes);
        for diag in &result.diagnostics {
            let s = diag.span.start;
            let e = diag.span.end;
            prop_assert!(s <= e, "span start {s} > end {e} in {src:?}");
            prop_assert!(e <= bytes.len(), "span end {e} > source len {} in {src:?}", bytes.len());
        }
    }

    // fix is idempotent: applying fixes twice yields the same source as applying once.
    #[test]
    fn fix_idempotent(src in arb_source()) {
        let e = engine();
        let first = e.fix(src.as_bytes(), FixMode::Apply);
        let second = e.fix(first.source.expose_secret(), FixMode::Apply);
        prop_assert!(
            second.source.expose_secret() == first.source.expose_secret(),
            "fix not idempotent on: {:?}", src,
        );
    }

    // DryRun and Apply produce identical rule IDs and confidence values in applied.
    #[test]
    fn dry_run_matches_apply_applied(src in arb_source()) {
        let e = engine();
        let dry = e.fix(src.as_bytes(), FixMode::DryRun);
        let apply = e.fix(src.as_bytes(), FixMode::Apply);
        prop_assert!(
            dry.applied_fixes().count() == apply.applied_fixes().count(),
            "dry-run and apply applied counts differ for {:?}", src,
        );
        for (d, a) in dry.applied_fixes().zip(apply.applied_fixes()) {
            prop_assert!(
                d.rule == a.rule,
                "dry-run/apply rule ID mismatch for {:?}", src,
            );
            // confidence combined() values must be identical (same rule, same input)
            let dc = d.fix.replacement.confidence.combined();
            let ac = a.fix.replacement.confidence.combined();
            prop_assert!(
                (dc - ac).abs() < f32::EPSILON,
                "dry-run confidence {} != apply confidence {} for {:?}", dc, ac, src,
            );
        }
    }

    // DryRun must not modify the source bytes.
    #[test]
    fn dry_run_source_unchanged(src in arb_source()) {
        let bytes = src.as_bytes().to_vec();
        let result = engine().fix(&bytes, FixMode::DryRun);
        prop_assert!(
            result.source.expose_secret() == bytes,
            "dry-run modified source for {:?}", src,
        );
    }

    // Every AppliedFix confidence satisfies the default threshold.
    #[test]
    fn threshold_respected(src in arb_source()) {
        let threshold = Config::default().confidence_threshold();
        let result = engine().fix(src.as_bytes(), FixMode::Apply);
        for fix in result.applied_fixes() {
            let combined = fix.fix.replacement.confidence.combined();
            prop_assert!(
                combined >= threshold,
                "applied fix confidence {combined} < threshold {threshold} for {src:?}",
            );
        }
        for tc in result.applied_text_corrections() {
            let combined = tc.confidence.combined();
            prop_assert!(
                combined >= threshold,
                "applied text correction confidence {combined} < threshold {threshold} for {src:?}",
            );
        }
    }

    // Every Confidence value in an AppliedFix has recognition and
    // combined() in [0.0, 1.0]. Post-PR-B there is no `rule` axis.
    #[test]
    fn confidence_bounds(src in arb_source()) {
        let result = engine().fix(src.as_bytes(), FixMode::Apply);
        for fix in result.applied_fixes() {
            let c = &fix.fix.replacement.confidence;
            prop_assert!(
                (0.0..=1.0).contains(&c.recognition),
                "recognition {} out of range for {src:?}",
                c.recognition,
            );
            let combined = c.combined();
            prop_assert!(
                (0.0..=1.0).contains(&combined),
                "combined confidence {combined} out of range for {src:?}",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Clean-input property: a well-formed banner produces no diagnostics.
//
// Parameterised over the small set of clean banners the engine definitely
// accepts, not generated, because "what is a valid clean banner" is
// schema-governed and generating only valid ones is out of scope here.
// ---------------------------------------------------------------------------

#[test]
fn clean_banners_produce_no_diagnostics() {
    // Only banners that pass all active rules — keep this list conservative.
    // The proptest properties above cover near-valid inputs; this test
    // specifically guards the happy-path guarantee.
    let clean_inputs: &[&[u8]] = &[
        b"TOP SECRET//SI//NOFORN\n",
        b"SECRET//TK\n",
        b"CONFIDENTIAL\n",
        b"UNCLASSIFIED\n",
    ];
    let e = engine();
    for input in clean_inputs {
        let result = e.lint(input);
        assert!(
            result.is_clean(),
            "clean banner produced diagnostics: {:?}\ninput: {:?}",
            result.diagnostics,
            std::str::from_utf8(input).unwrap_or("<invalid utf8>"),
        );
    }
}
