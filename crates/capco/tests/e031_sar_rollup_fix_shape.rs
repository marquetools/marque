// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E031 (`capco:banner.banner-rollup.sar-portions-roll-up`) — SAR
//! banner-rollup walker fix-shape + hierarchy-optional pins ported
//! from `crates/capco/src/_disabled_tests.rs` per issue #722.
//!
//! E031 is the walker registration for `BannerMatchesProjectedRule`;
//! the SAR row IS the walker's `Rule::id()` tuple per `legacy-rule-id-
//! map.md` §5. The walker emits 4 additional rule IDs (E035 / E040 /
//! E068 / E069) per `additional_emitted_ids`; this file covers the SAR
//! row only.
//!
//! # Source tests ported
//!
//! - `e031_fires_when_banner_missing_program_from_portion` — fire +
//!   canonical-fix output (load-bearing: the rewritten banner must
//!   contain `SECRET//SAR-BP/CD//NOFORN`).
//! - `e031_does_not_fire_when_banner_omits_portion_compartment` —
//!   §H.5 p101 hierarchy-optional carve-out (compartment).
//! - `e031_does_not_fire_when_banner_omits_portion_sub_compartment`
//!   — §H.5 p101 hierarchy-optional carve-out (sub-compartment).
//! - `e031_fires_when_banner_has_no_sar_block_but_portion_does` —
//!   no-fix invariant when the entire SAR block is missing.
//! - `e031_fix_preserves_observed_hierarchy_when_adding_missing_program`
//!   — hierarchy-preservation invariant.
//! - `e031_cites_h5_p101` — citation lockdown pin.
//!
//! # Architecture note
//!
//! The walker emits a structural
//! `FixIntent` (`FactAdd` for missing-program insertion or a
//! `Recanonicalize` per the row dispatch); the engine synthesizes
//! the byte-precise zero-width-insert at promotion time. The load-
//! bearing invariant — "applied output produces
//! `SECRET//SAR-BP/CD//NOFORN`" — is preserved by reading
//! `Engine::fix(...).source` instead of inspecting fix-internal
//! fields.
//!
//! # Authority
//!
//! CAPCO-2016 §H.5 p101 (SAR roll-up rule + the hierarchy-optional
//! carve-out — both rules live at the same page anchor under the
//! single-citation discipline). Re-verified against
//! `crates/capco/docs/CAPCO-2016.md` at authorship per Constitution
//! VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_rules::{Diagnostic, Severity};
use marque_scheme::{SectionLetter, capco};

const E031_PREDICATE: &str = "banner.banner-rollup.sar-portions-roll-up";

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

fn e031_diags(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    lint(source)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == E031_PREDICATE)
        .collect()
}

/// The SAR banner-rollup evaluator emits the fix as a
/// `text_correction` carrying a zero-width insertion at the end of
/// the SAR block. The fix's `confidence` is 0.9 (per
/// `crates/capco/src/rules/banner/eval_sar.rs`); the default
/// `confidence_threshold` is 0.95, so the engine demotes the
/// diagnostic to `Severity::Suggest` rather than auto-applying it.
/// The walker's "≤2-pass convergence" comment documents the
/// sub-threshold posture deliberately (a second `marque fix` pass
/// finishes canonicalization via E028).
///
/// These tests inspect the `text_correction.replacement` payload
/// directly (the modern equivalent of the legacy
/// `fix.replacement.as_ref()` reads) so the load-bearing invariants
/// — the inserted bytes, the SAR-block-end span anchor, the
/// hierarchy-preservation property — are pinned without depending
/// on threshold-gating policy.
fn assert_text_correction_replacement(diag: &Diagnostic<CapcoScheme>, expected: &str) {
    let tc = diag
        .text_correction
        .as_ref()
        .expect("E031 SAR-row must carry a text_correction");
    assert_eq!(
        tc.replacement.as_str(),
        expected,
        "E031 SAR-row text_correction replacement must be {expected:?}; \
         got: {:?}",
        tc.replacement,
    );
}

// ---------------------------------------------------------------------------
// Fire + canonical-fix output
// ---------------------------------------------------------------------------

/// Portions introduce SAR-BP and SAR-CD; banner only mentions BP.
/// E031 fires; the emitted `text_correction` carries `/CD` as the
/// zero-width insertion at the end of the SAR block. Splicing the
/// replacement at that anchor produces `SECRET//SAR-BP/CD//NOFORN`.
///
/// Pre-cutover the legacy `FixProposal` carried `span.start ==
/// span.end` (zero-width insertion), `original == ""`, and
/// `replacement == "/CD"`; post-cutover the walker emits via
/// `Diagnostic.text_correction` with the same replacement bytes
/// and a span that the engine's synthesis path treats as a
/// zero-width insertion at the SAR-block-end anchor. The default
/// confidence threshold (0.95) gates the 0.9-confidence fix, so the
/// rule's `Severity::Fix` is demoted to `Suggest` and the diagnostic
/// stays unapplied at default config — see the helper doc-comment
/// above. The load-bearing invariant — the replacement bytes the
/// engine would splice — is preserved.
///
/// Authority: CAPCO-2016 §H.5 p101 (SAR roll-up rule). Re-verified
/// against `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e031_fires_and_emits_canonical_text_correction() {
    let src = "(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
    let diags = e031_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E031 must fire when banner omits CD: {diags:?}"
    );
    assert_text_correction_replacement(&diags[0], "/CD");
}

// ---------------------------------------------------------------------------
// Hierarchy-optional carve-outs (§H.5 p101)
// ---------------------------------------------------------------------------

/// Narrowed predicate: §H.5 p101 makes banner hierarchy depth (below
/// the program identifier) optional. A
/// portion with `SAR-BP-J12` rolling up to a banner with `SAR-BP`
/// (no compartment shown) is compliant — the author deliberately
/// omitted hierarchy. The prior behavior treated this as an E031
/// violation; that was over-restriction relative to source.
///
/// Authority: CAPCO-2016 §H.5 p101. Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e031_does_not_fire_when_banner_omits_portion_compartment() {
    let src = "(S//SAR-BP-J12//NF)\nSECRET//SAR-BP//NOFORN";
    let diags = e031_diags(src);
    assert!(
        diags.is_empty(),
        "E031 must NOT fire on optional-hierarchy banner (portion has \
         BP-J12, banner has bare BP — §H.5 p101 permits): {diags:?}",
    );
}

/// Sibling case: §H.5 p101 covers sub-compartments too ("hierarchy
/// ... below the program identifier is optional"). Portion has
/// `SAR-BP-J12 K15` (J12 compartment, K15 sub-compartment); banner
/// has `SAR-BP-J12` (omits the sub-compartment). E031 must stay
/// silent.
#[test]
fn e031_does_not_fire_when_banner_omits_portion_sub_compartment() {
    let src = "(S//SAR-BP-J12 K15//NF)\nSECRET//SAR-BP-J12//NOFORN";
    let diags = e031_diags(src);
    assert!(
        diags.is_empty(),
        "E031 must NOT fire when banner omits sub-compartment present \
         in portion (hierarchy is optional): {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// No-fix invariant: missing SAR block
// ---------------------------------------------------------------------------

/// Portion has SAR-BP; banner has no SAR block at all. The walker
/// fires + escalates severity to Error (the whole block is missing,
/// not just a program within it). No fix because byte-positioning a
/// new SAR block in the banner is unsafe.
///
/// Pre-cutover the test asserted on two message-content invariants
/// (`"missing an SAR block"` vs `"SAR block is missing programs"` —
/// distinct-wording pin from PR #101 review). The message
/// template is now shared; the distinction is structural (severity
/// + fix.is_none()) rather than prose-substring.
#[test]
fn e031_fires_error_no_fix_when_banner_has_no_sar_block() {
    let src = "(S//SAR-BP//NF)\nSECRET//NOFORN";
    let diags = e031_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E031 must fire when banner lacks any SAR block: {diags:?}",
    );
    assert!(
        diags[0].fix.is_none() && diags[0].text_correction.is_none(),
        "E031 must not propose a fix when no SAR block exists \
         (byte-positioning a new block is unsafe): {:?}",
        diags[0],
    );
    assert_eq!(
        diags[0].severity,
        Severity::Error,
        "E031 escalates to Error when the entire SAR block is missing; \
         got: {:?}",
        diags[0].severity,
    );
}

// ---------------------------------------------------------------------------
// Hierarchy preservation in fix output
// ---------------------------------------------------------------------------

/// The zero-width insertion at end-of-block must preserve the observed
/// banner's hierarchy verbatim and add only
/// the missing programs as bare identifiers. §H.5 p101 makes
/// hierarchy depiction the author's choice; the fix honors that by
/// construction.
///
/// Portion: SAR-BP-J12 (BP with compartment J12) AND SAR-CD. Banner
/// observed: SAR-BP-J12 (BP with compartment shown, CD missing).
/// Applied output: SAR-BP-J12/CD (J12 preserved, bare CD appended
/// — NO invented hierarchy on CD).
///
/// Authority: CAPCO-2016 §H.5 p101 (hierarchy-optional carve-out).
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` per
/// Constitution VIII.
#[test]
fn e031_fix_appends_bare_program_id_no_invented_hierarchy() {
    let src = "(S//SAR-BP-J12//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP-J12//NOFORN";
    let diags = e031_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E031 must fire on missing program CD: {diags:?}"
    );
    // The walker matches by program identifier only (§H.5 p101
    // hierarchy-optional); the replacement appends bare `CD` —
    // no invented hierarchy. Splicing `/CD` at the SAR-block-end
    // anchor preserves the observed `BP-J12` hierarchy verbatim and
    // adds only the missing program.
    assert_text_correction_replacement(&diags[0], "/CD");
}

// ---------------------------------------------------------------------------
// Citation lockdown (renamed from legacy line-NNNN form)
// ---------------------------------------------------------------------------

/// Citation lockdown. E031's authority is §H.5 p101 — both the SAR
/// roll-up rule AND the hierarchy-optional carve-out live at the same
/// passage. The typed `Citation` pins the passage; the cross-reference
/// text framing lives in the rule doc comment.
///
/// Test fn name renamed from `e031_cites_line_2458_and_hierarchy_optional_note`
/// per `feedback_avoid_line_number_anchoring` +
/// `feedback_citations_use_page_numbers` (CAPCO-2016 citations cite
/// §X.Y pNN; line-NNNN form is retired).
///
/// Authority: CAPCO-2016 §H.5 p101. Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e031_cites_h5_p101() {
    let src = "(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
    let diags = e031_diags(src);
    assert_eq!(diags.len(), 1, "E031 must fire: {diags:?}");
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::H, 5, 101),
        "E031 citation must pin §H.5 p101 (SAR roll-up rule + \
         hierarchy-optional carve-out); got: {:?}",
        diags[0].citation,
    );
}
