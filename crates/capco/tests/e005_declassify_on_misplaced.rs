// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E005 (`capco:portion.declassification.declassify-on-misplaced`)
//! coverage pins ported from `crates/capco/src/_disabled_tests.rs`
//! per issue #722.
//!
//! # Source tests ported
//!
//! - `e005_fires_on_declass_exemption_in_portion` — portion-scope
//!   coverage. Banner-scope coverage is already exercised by
//!   `tests/corpus/capco/invalid/` fixtures; the portion-scope path
//!   (CAPCO-2016 §C.1 p26 mirrors §D.1 p27 for portions) was originally
//!   tested via the inline `mod tests` block. No corpus fixture covers
//!   the portion-scope path, so this
//!   port preserves the coverage.
//! - `e005_citation_points_at_specific_sections` — citation lockdown
//!   pin: the diagnostic's typed `Citation` MUST anchor at §E.1 p31.
//! - `e005_emits_no_fix_and_no_fix_intent_pending_stage4_*` —
//!   conscious-defer symmetry pin. Pre-cutover the test asserted both
//!   `fix.is_none()` AND `fix_intent.is_none()`. Post-PR-3c.B-Commit-10
//!   `Diagnostic` has a single `fix: Option<FixIntent<S>>` field plus
//!   `text_correction: Option<TextCorrection>`; the symmetry pin
//!   collapses to "neither fix nor text_correction is populated".
//!
//! # Authority
//!
//! CAPCO-2016 §E.1 p31 (Declassify On is a CAB line — the primary
//! anchor) + §D.1 p27 (banner-line categories explicitly exclude
//! declassification markings; §C.1 p26 mirrors for portions). Each
//! citation re-verified against `crates/capco/docs/CAPCO-2016.md`
//! at authorship per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_rules::Diagnostic;
use marque_scheme::{SectionLetter, capco};

const E005_PREDICATE: &str = "portion.declassification.declassify-on-misplaced";

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

fn e005_diags(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    lint(source)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == E005_PREDICATE)
        .collect()
}

// ---------------------------------------------------------------------------
// Portion-scope coverage
// ---------------------------------------------------------------------------

/// CAPCO-2016 §C.1 p26 closed category list for portions mirrors §D.1
/// p27's closed category list for banners, so `25X1` between `//`
/// separators in a portion is just as misplaced as in a banner. The
/// span pin (`"25X1"`) catches a regression that drifts the span
/// boundary into the surrounding `//` separators.
///
/// Authority: CAPCO-2016 §E.1 p31 (Declassify On is a CAB line —
/// primary anchor) + §C.1 p26 (portion-line category exclusion).
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` per
/// Constitution VIII.
#[test]
fn e005_fires_on_declass_exemption_in_portion() {
    let src = "(S//25X1//NF)";
    let diags = e005_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E005 must fire on declass exemption inside a portion: {diags:?}",
    );
    let span_text = diags[0]
        .span
        .as_str(src.as_bytes())
        .expect("E005 span must point at valid UTF-8 bytes");
    assert_eq!(
        span_text, "25X1",
        "E005 span must point precisely at the declass token, not the \
         surrounding separators; got: {span_text:?}",
    );
}

// ---------------------------------------------------------------------------
// Citation lockdown
// ---------------------------------------------------------------------------

/// Lock down the citation. The typed `Citation` pins the primary
/// anchor (§E.1 p31 — "Declassify On is a CAB line"). The
/// cross-reference to §D.1 p27 lives on the rule's
/// `DECLASSIFY_MISPLACED_CROSS_REFS` constant; the companion assertion
/// (via `DECLASSIFY_MISPLACED_CROSS_REFS.contains(...)`)
/// is exercised by `crates/capco/tests/citation_fidelity.rs` +
/// `crates/capco/src/rules/citation_cross_refs_tests.rs`. This test
/// pins the diagnostic-side primary anchor only.
///
/// Authority: CAPCO-2016 §E.1 p31. Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e005_citation_anchors_at_e1_p31() {
    let src = "SECRET//25X1//NOFORN";
    let diags = e005_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E005 must fire on banner declass: {diags:?}"
    );
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::E, 1, 31),
        "E005 citation must anchor at §E.1 p31 (Declassify On is a CAB \
         line); got: {:?}",
        diags[0].citation,
    );
}

// ---------------------------------------------------------------------------
// Conscious-defer symmetry pin
// ---------------------------------------------------------------------------

/// E005 is a stage-4-pending Recanonicalize-document rule
/// (Path A fallback). The structural blocker — `MarkingScheme::
/// evaluate_custom` having no access to `RuleContext.marking_type` —
/// means the rule MUST consciously decline to emit a fix today.
///
/// The symmetry pin formerly asserted both `fix.is_none()`
/// AND `fix_intent.is_none()` (two separate fields). Now
/// `Diagnostic` has a single `fix: Option<FixIntent<S>>` plus
/// `text_correction: Option<TextCorrection>`; the dual-field
/// assertion collapses to "neither fix nor text_correction is
/// populated".
///
/// The content-ignorance closure walker at
/// `crates/capco/tests/g13_closure_fix_intent.rs` only inspects rules
/// whose `fix.is_some()`; E005 with neither populated is unreached
/// by that walker, so this symmetry pin is the only guard against a
/// future commit silently flipping E005 to emit a fix without the
/// Stage-4 design landing first.
#[test]
fn e005_emits_neither_fix_nor_text_correction_pending_stage4() {
    let src = "SECRET//25X1//NOFORN";
    let diags = e005_diags(src);
    let e005 = diags
        .first()
        .expect("E005 must fire on `SECRET//25X1//NOFORN`");
    assert!(
        e005.fix.is_none() && e005.text_correction.is_none(),
        "E005 must consciously decline to emit a fix or text_correction \
         until Stage-4 Recanonicalize-document lands. \
         Got fix={:?}, text_correction={:?}",
        e005.fix,
        e005.text_correction,
    );
}
