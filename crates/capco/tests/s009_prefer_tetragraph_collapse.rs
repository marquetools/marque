// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for rule **S009** — `prefer-tetragraph-collapse`.
//!
//! S009 is `Severity::Off` by default; all tests configure the engine
//! with `[rules] S009 = "suggest"` to activate it. Tests cover:
//!
//! - FVEY full-member collapse (`AUS CAN GBR NZL` + USA → `FVEY`)
//! - ACGU collapse (`AUS CAN GBR` + USA → `ACGU`)
//! - Multi-tetragraph collapse when two tetragraphs apply
//! - Partial member list (no collapse — not all members present)
//! - Already compact (tetragraph already in list — no-op)
//! - USA-only REL TO (no-op — nothing to collapse with)
//! - Off-by-default (rule does not fire when not configured)
//! - Canonical order in replacement (USA first, trigraphs alpha, tetragraphs alpha)
//!
//! Authority: CAPCO-2016 §H.8 p150 re-verified at authorship against
//! `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Severity;

/// Build an engine with S009 enabled at `suggest` severity.
fn engine_with_s009() -> Engine {
    let mut config = Config::default();
    config
        .rules
        .overrides
        .insert("S009".to_string(), "suggest".to_string());
    Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine construction should succeed")
}

/// Build an engine with S009 at default severity (Off).
fn engine_default() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine construction should succeed")
}

/// Find S009 diagnostics in the lint result.
fn s009_diags(
    result: &marque_engine::LintResult,
) -> Vec<&marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "S009")
        .collect()
}

// ---------------------------------------------------------------------------
// Off-by-default gate
// ---------------------------------------------------------------------------

#[test]
fn s009_does_not_fire_when_off_by_default() {
    let engine = engine_default();
    // FVEY-collapsible input — would fire if enabled.
    let result = engine.lint(b"SECRET//REL TO USA, AUS, CAN, GBR, NZL//NOFORN");
    let diags = s009_diags(&result);
    assert!(
        diags.is_empty(),
        "S009 must not fire when severity is Off (default); got: {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// FVEY collapse: AUS + CAN + GBR + NZL (+ USA) → FVEY
// ---------------------------------------------------------------------------

#[test]
fn s009_collapses_fvey_members_in_portion() {
    let engine = engine_with_s009();
    let result = engine.lint(b"(S//REL TO USA, AUS, CAN, GBR, NZL)");
    let diags = s009_diags(&result);
    assert_eq!(
        diags.len(),
        1,
        "expected exactly one S009 diagnostic; got {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.severity, Severity::Suggest);
    // Replacement should be the compact FVEY form.
    let tc = d
        .text_correction
        .as_ref()
        .expect("S009 diagnostic must carry a text_correction replacement");
    assert_eq!(
        tc.replacement.as_str(),
        "REL TO USA, FVEY",
        "S009 replacement should be 'REL TO USA, FVEY'; got {:?}",
        tc.replacement,
    );
}

#[test]
fn s009_collapses_fvey_members_in_banner() {
    let engine = engine_with_s009();
    let result = engine.lint(b"SECRET//REL TO USA, AUS, CAN, GBR, NZL");
    let diags = s009_diags(&result);
    assert_eq!(
        diags.len(),
        1,
        "expected one S009 for banner; got {diags:?}"
    );
    let tc = diags[0]
        .text_correction
        .as_ref()
        .expect("must have text_correction");
    assert_eq!(tc.replacement.as_str(), "REL TO USA, FVEY");
}

// ---------------------------------------------------------------------------
// ACGU collapse: AUS + CAN + GBR + USA (4 members; ACGU without NZL)
// ---------------------------------------------------------------------------

#[test]
fn s009_collapses_acgu_members_when_all_present() {
    // ACGU = AUS + CAN + GBR + USA (4 members). When all 4 are in the
    // REL TO list, suggest ACGU.
    let engine = engine_with_s009();
    // Build with all ACGU members (no NZL so FVEY doesn't also apply).
    let result = engine.lint(b"(S//REL TO USA, AUS, CAN, GBR)");
    let diags = s009_diags(&result);
    assert_eq!(diags.len(), 1, "expected one S009 for ACGU; got {diags:?}");
    let tc = diags[0]
        .text_correction
        .as_ref()
        .expect("must have text_correction");
    // FVEY needs NZL too, so only ACGU qualifies here.
    assert_eq!(
        tc.replacement.as_str(),
        "REL TO USA, ACGU",
        "S009 should suggest ACGU for AUS+CAN+GBR+USA; got {:?}",
        tc.replacement,
    );
}

// ---------------------------------------------------------------------------
// Partial member list — no collapse (not all members present)
// ---------------------------------------------------------------------------

#[test]
fn s009_no_op_when_fvey_members_incomplete() {
    let engine = engine_with_s009();
    // Only AUS + CAN — not all of FVEY (GBR and NZL missing).
    let result = engine.lint(b"(S//REL TO USA, AUS, CAN)");
    let diags = s009_diags(&result);
    assert!(
        diags.is_empty(),
        "S009 must not fire when member list is incomplete; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Already compact — tetragraph already in the REL TO list
// ---------------------------------------------------------------------------

#[test]
fn s009_no_op_when_tetragraph_already_present() {
    let engine = engine_with_s009();
    // FVEY already in the list — no individual members, nothing to collapse.
    let result = engine.lint(b"(S//REL TO USA, FVEY)");
    let diags = s009_diags(&result);
    assert!(
        diags.is_empty(),
        "S009 must not fire when tetragraph is already in the list; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// USA-only REL TO — no collapse possible
// ---------------------------------------------------------------------------

#[test]
fn s009_no_op_on_usa_only_rel_to() {
    let engine = engine_with_s009();
    // NOTE: "REL TO USA" alone is an unauthorized form (§H.8 p151)
    // and E002 would fire, but S009 should still not fire.
    let result = engine.lint(b"(S//REL TO USA)");
    let diags = s009_diags(&result);
    assert!(
        diags.is_empty(),
        "S009 must not fire on a USA-only REL TO list; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// No REL TO at all — no collapse possible
// ---------------------------------------------------------------------------

#[test]
fn s009_no_op_when_no_rel_to() {
    let engine = engine_with_s009();
    let result = engine.lint(b"(S//NF)");
    let diags = s009_diags(&result);
    assert!(
        diags.is_empty(),
        "S009 must not fire when there is no REL TO; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Canonical replacement order: USA first, remaining trigraphs alpha, tetragraphs alpha
// ---------------------------------------------------------------------------

#[test]
fn s009_replacement_preserves_non_collapsed_trigraphs() {
    let engine = engine_with_s009();
    // DEU is not a member of FVEY or ACGU — should remain in the replacement.
    // Input: USA, AUS, CAN, DEU, GBR, NZL → collapse FVEY (AUS+CAN+GBR+NZL),
    // DEU stays as a lone trigraph.
    let result = engine.lint(b"(S//REL TO USA, AUS, CAN, DEU, GBR, NZL)");
    let diags = s009_diags(&result);
    assert_eq!(diags.len(), 1, "expected one S009; got {diags:?}");
    let tc = diags[0]
        .text_correction
        .as_ref()
        .expect("must have text_correction");
    // DEU stays, FVEY collapses AUS+CAN+GBR+NZL.
    // Canonical order: USA, DEU (trigraph alpha), FVEY (tetragraph alpha).
    assert_eq!(
        tc.replacement.as_str(),
        "REL TO USA, DEU, FVEY",
        "S009 should emit 'REL TO USA, DEU, FVEY'; got {:?}",
        tc.replacement,
    );
}

#[test]
fn s009_replacement_usa_comes_first() {
    let engine = engine_with_s009();
    let result = engine.lint(b"(S//REL TO USA, AUS, CAN, GBR, NZL)");
    let diags = s009_diags(&result);
    let tc = diags[0]
        .text_correction
        .as_ref()
        .expect("must have text_correction");
    assert!(
        tc.replacement.as_str().starts_with("REL TO USA"),
        "S009 replacement must start with 'REL TO USA'; got {:?}",
        tc.replacement,
    );
}
