// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Bare HCS / bare RSV class-specific rules (E061 / E062 / E063).
//!
//! Authority: CAPCO-2016 §H.4 pp 62, 70.
//!
//! - **E061** (`hcs-bare-at-confidential-legacy-remark`): bare HCS at
//!   CONFIDENTIAL — legacy guidance per §H.4 p62: "When legacy
//!   information at the CONFIDENTIAL//HCS level is discovered, contact
//!   the originator for guidance prior to reusing the information."
//!   Warn severity, no fix.
//!
//! - **E062** (`hcs-bare-suggest-subcompartment`): bare HCS at
//!   SECRET / TOP SECRET — per §H.4 p62 re-mark guidance. Emits 3
//!   per-candidate Suggest diagnostics (HCS-O / HCS-P / HCS-O-P) so
//!   editors can offer one-click substitution. The classifier picks
//!   the right one based on content (Operations vs Product). Warn
//!   severity at the rule level.
//!
//! - **E063** (`rsv-bare-requires-compartment`): bare RSV — per §H.4
//!   p70 "the RSV marking may not be used alone and requires the
//!   associated compartment". Warn severity, no fix (the compartment
//!   identifier is org-private content beyond Marque's vocabulary).
//!   Bare RSV is a structurally-incomplete marking (RESERVE is itself
//!   canonical; just missing the compartment), not invalid — Warn
//!   surfaces the gap without claiming the marking is structurally
//!   broken. Contrast with E065's deprecated-control-system rows where
//!   the source control system itself is retired.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{CapcoEngine, FixedClock};
use marque_rules::Severity;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine() -> CapcoEngine {
    CapcoEngine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme constructs without rewrite cycles")
}

fn lint(rule: &str, source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    let result = engine().lint(source);
    result
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.predicate_id() == rule)
        .collect()
}

// =========================================================================
// E061 — bare HCS at CONFIDENTIAL
// =========================================================================

#[test]
fn e061_fires_on_bare_hcs_at_confidential() {
    let source = b"(CONFIDENTIAL//HCS//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-at-confidential-legacy-remark", source);
    assert_eq!(diags.len(), 1, "exactly one E061 diagnostic expected");
    assert_eq!(diags[0].severity, Severity::Warn);
    assert!(
        format!("{}", diags[0].citation).contains("§H.4 p62"),
        "citation must cite §H.4 p62; got {:?}",
        diags[0].citation
    );
    // Closed `Message` shape — free-form "contact the originator" prose
    // is absent from the message body (Constitution V Principle V). The
    // SupersededToken template captures the deprecation class; the §H.4
    // p62 citation pins the authority. The user-facing prose is renderer
    // responsibility.
    use marque_rules::MessageTemplate;
    assert_eq!(
        diags[0].message.template(),
        MessageTemplate::SupersededToken,
        "E061 fires under the SupersededToken template; got {:?}",
        diags[0].message.template(),
    );
}

#[test]
fn e061_does_not_fire_at_secret() {
    // E061 is class-specific to CONFIDENTIAL; bare HCS at SECRET is
    // E062's domain.
    let source = b"(SECRET//HCS//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-at-confidential-legacy-remark", source);
    assert!(diags.is_empty(), "E061 must not fire outside CONFIDENTIAL");
}

#[test]
fn e061_does_not_fire_at_top_secret() {
    let source = b"(TOP SECRET//HCS//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-at-confidential-legacy-remark", source);
    assert!(diags.is_empty());
}

#[test]
fn e061_does_not_fire_when_hcs_has_compartment() {
    // Bare HCS only — compound HCS-O / HCS-P / HCS-O-P forms are not
    // legacy.
    let source = b"(CONFIDENTIAL//HCS-O//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-at-confidential-legacy-remark", source);
    assert!(
        diags.is_empty(),
        "E061 must not fire when HCS carries a compartment"
    );
}

// =========================================================================
// E062 — bare HCS at SECRET / TOP SECRET
// =========================================================================

#[test]
fn e062_emits_three_candidates_at_secret() {
    // §H.4 p62 — re-mark to HCS-O / HCS-P / HCS-O-P templates.
    let source = b"(SECRET//HCS//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-suggest-subcompartment", source);
    assert_eq!(
        diags.len(),
        3,
        "exactly 3 per-candidate diagnostics expected (HCS-O, HCS-P, HCS-O-P)"
    );

    let replacements: Vec<String> = diags
        .iter()
        .filter_map(|d| {
            d.text_correction
                .as_ref()
                .map(|t| t.replacement.to_string())
        })
        .collect();
    assert!(replacements.contains(&"HCS-O".to_owned()));
    assert!(replacements.contains(&"HCS-P".to_owned()));
    assert!(replacements.contains(&"HCS-O-P".to_owned()));

    // Per-diagnostic severity is the rule's emitted Suggest — the
    // engine only overwrites severity from `config.rules.overrides`
    // (the user's `.marque.toml`), not from `default_severity()`.
    // The rule emits Severity::Suggest per-candidate so the engine's
    // auto-apply gate never promotes them; the user picks the right
    // one via UI.
    for d in &diags {
        assert_eq!(
            d.severity,
            Severity::Suggest,
            "per-candidate diagnostics emit at Suggest severity so engine never auto-applies"
        );
    }
}

#[test]
fn e062_emits_three_candidates_at_top_secret() {
    let source = b"(TOP SECRET//HCS//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-suggest-subcompartment", source);
    assert_eq!(diags.len(), 3);
}

#[test]
fn e062_does_not_fire_at_confidential() {
    // E062 is class-specific to S/TS; bare HCS at C is E061's domain.
    let source = b"(CONFIDENTIAL//HCS//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-suggest-subcompartment", source);
    assert!(diags.is_empty());
}

#[test]
fn e062_does_not_fire_when_hcs_has_compartment() {
    let source = b"(SECRET//HCS-O//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-suggest-subcompartment", source);
    assert!(diags.is_empty());
}

// =========================================================================
// E063 — bare RSV requires compartment
// =========================================================================

#[test]
fn e063_fires_on_bare_rsv() {
    // §H.4 p70: "the RSV marking may not be used alone and requires
    // the associated compartment".
    //
    // Severity::Warn (not Error): bare RSV is a structurally-incomplete
    // marking — RESERVE is canonical per §H.4 p70; the user just hasn't
    // specified the required compartment. The rule surfaces the gap;
    // the marking will be valid once the user adds the compartment.
    // Contrast with E065's deprecated-control-system rows (bare KDK /
    // KLONDIKE / EL / ENDSEAL / ECI) which fire at Error because the
    // source control system itself is gone.
    let source = b"(TOP SECRET//RSV//NOFORN)";
    let diags = lint("portion.sci.rsv-bare-requires-compartment", source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Warn);
    assert!(
        format!("{}", diags[0].citation).contains("§H.4 p70"),
        "citation must cite §H.4 p70; got {:?}",
        diags[0].citation
    );
    // Closed `Message` shape — free-form "may not be used alone" prose
    // is absent (Constitution V Principle V). The RequiredByPresence
    // template captures the missing-companion class; the §H.4 p70
    // citation pins the authority.
    use marque_rules::MessageTemplate;
    assert_eq!(
        diags[0].message.template(),
        MessageTemplate::RequiredByPresence,
        "E063 fires under the RequiredByPresence template; got {:?}",
        diags[0].message.template(),
    );
    // No fix proposed (compartment identifier is org-private).
    assert!(diags[0].text_correction.is_none());
}

#[test]
fn e063_does_not_fire_when_rsv_has_compartment() {
    // `RSV-XYZ` (compartment present) — E063 must not fire.
    let source = b"(TOP SECRET//RSV-XYZ//NOFORN)";
    let diags = lint("portion.sci.rsv-bare-requires-compartment", source);
    assert!(
        diags.is_empty(),
        "E063 must not fire when RSV carries a compartment"
    );
}

#[test]
fn e063_does_not_fire_on_non_rsv_sci() {
    let source = b"(TOP SECRET//SI//NOFORN)";
    let diags = lint("portion.sci.rsv-bare-requires-compartment", source);
    assert!(diags.is_empty());
}

// =========================================================================
// Span anchoring through the deprecated SCI long-form parser path.
//
// E062 (and E061 / E063) locate byte anchors by filtering
// `attrs.token_spans` for `TokenKind::SciSystem` and indexing by
// `sci_markings` position. Before the fix, the long-form parser path
// (HUMINT, COMINT, ECI, EL, ENDSEAL, KDK, KLONDIKE) emitted only a
// `TokenKind::SciControl` span and no `TokenKind::SciSystem` span; the
// rule's span lookup silently fell through to `Span::new(0, 0)` and
// the resulting diagnostic anchored at byte 0..0 of the input — silent
// audit corruption per Constitution Principle V. The fix adds a
// coincident SciSystem span at the long-form recognizer site
// (parser.rs ~line 441), restoring the invariant the rule layer
// already depended on.
// =========================================================================

#[test]
fn e062_humint_long_form_diagnostics_have_non_zero_spans() {
    // Pre-fix regression test: `(SECRET//HUMINT//NOFORN)` triggered E062
    // because HUMINT is the deprecated long form of HCS (bare HCS at
    // S/TS). The walker emitted three Suggest candidates anchored at
    // `Span::new(0, 0)`. Post-fix the parser emits a SciSystem span
    // covering bytes 9..15 (`HUMINT`), and every E062 diagnostic
    // anchors at that span.
    let source = b"(SECRET//HUMINT//NOFORN)";
    let diags = lint("portion.sci.hcs-bare-suggest-subcompartment", source);
    assert_eq!(diags.len(), 3, "E062 emits three Suggest candidates");
    for d in &diags {
        assert_ne!(
            d.span,
            marque_ism::span::Span::new(0, 0),
            "E062 diagnostic must not anchor at Span::new(0, 0); long-form parser \
             path must emit a SciSystem span (Constitution V audit invariant)"
        );
        // `(SECRET//HUMINT//NOFORN)`: `HUMINT` at bytes 9..15.
        assert_eq!(
            d.span.start, 9,
            "E062 diagnostic span must cover the HUMINT bytes"
        );
        assert_eq!(d.span.end, 15);
    }
}

#[test]
fn e062_comint_long_form_diagnostics_have_non_zero_spans() {
    // COMINT is the deprecated long-form for SI (not HCS). Bare HCS
    // class-specific rules (E062) should not fire on COMINT — the
    // parser maps COMINT to SI, which is a different SCI control. We
    // assert that whatever SCI-related rule fires (or doesn't), no
    // diagnostic anchors at `Span::new(0, 0)` because the long-form
    // parser path now emits a SciSystem span.
    let source = b"(SECRET//COMINT//NOFORN)";
    let engine = engine();
    let result = engine.lint(source);
    for d in &result.diagnostics {
        assert_ne!(
            d.span,
            marque_ism::span::Span::new(0, 0),
            "no diagnostic on COMINT long-form input may anchor at Span::new(0, 0); \
             got rule {:?}",
            d.rule.predicate_id()
        );
    }
}
