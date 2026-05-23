// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! NODIS / EXDIS rule-cluster ports from
//! `crates/capco/src/_disabled_tests.rs` per issue #722.
//!
//! # Rules covered (post-T044 wire strings)
//!
//! - **E037** `capco:portion.dissem.nodis-conflicts-exdis` — banner-
//!   line mutual exclusion. Bridge-emitted (retired as a registered
//!   `Rule` impl in PR #578; still fires via the constraint-catalog
//!   bridge — see `crates/capco/src/scheme/adapter.rs`).
//! - **E038** `capco:portion.dissem.nodis-or-exdis-requires-noforn` —
//!   NODIS / EXDIS require NOFORN. Bridge-emitted (same retirement
//!   wave as E037).
//! - **E039** `capco:page.dissem.nodis-exdis-clears-banner-rel-to` —
//!   registered rule. NODIS or EXDIS anywhere on the page → REL TO
//!   not authorized in banner.
//! - **E040** `capco:banner.banner-rollup.non-ic-dissem-roll-up` —
//!   per-row catalog ID emitted by `BannerMatchesProjectedRule`
//!   (E031 walker). NODIS supersedes EXDIS in the banner roll-up.
//! - **E041** `capco:portion.dissem.nodis-supersedes-exdis-in-portion`
//!   — registered rule. NODIS supersedes EXDIS within the same portion;
//!   emits an intent-only `FactRemove(EXDIS, Scope::Portion)` fix.
//!
//! # Source tests ported
//!
//! E037 (citation + symmetry pin):
//! - `e037_fires_when_nodis_and_exdis_coexist` — fire + citation pin.
//! - `e037_emits_no_fix_and_no_fix_intent_pending_stage4_b_reject` —
//!   conscious-defer symmetry pin (collapses post-cutover; see body
//!   for the dual-field reduction note).
//!
//! E038 (citation + fuse-into-one pin):
//! - `e038_fires_on_nodis_without_noforn` — fire + citation pin.
//! - `e038_fires_only_once_when_both_nodis_and_exdis_lack_noforn` —
//!   declarative-constraint fuse pin (one violation, not two).
//!
//! E039 (no-fix + citation pin):
//! - `e039_fires_on_banner_rel_to_with_nodis_portion` — fire +
//!   no-fix invariant + citation pin.
//! - `e039_still_fires_after_engine_gap_close` — PR 3c.B-8F regression
//!   pin.
//!
//! E040 (priority-of-NODIS-over-EXDIS pin + no-fix-when-no-block pin):
//! - `e040_nodis_has_priority_over_exdis_when_both_in_portions` —
//!   load-bearing priority invariant.
//! - `e040_emits_no_fix_when_banner_has_no_non_ic_dissem_block` —
//!   byte-safety invariant.
//!
//! E041 (intent shape + span direction + scope-guard pins):
//! - `e041_fires_on_portion_with_both_nodis_and_exdis` — fire +
//!   `Severity::Warn` + `FactRemove` intent shape.
//! - `e041_points_at_exdis_token_in_both_orderings` — span direction
//!   regardless of token order.
//! - `e041_does_not_fire_on_banner_even_when_both_present` — portion-
//!   only scope guard (companion E037-on-banner check).
//! - `e041_emits_intent_only_factremove_exdis_portion` — three
//!   intent-shape invariants in one test.
//!
//! # Authority
//!
//! CAPCO-2016 §H.9 p172 (EXDIS template — "EXCLUSIVE DISTRIBUTION");
//! §H.9 p174 (NODIS template — "NO DISTRIBUTION" + supersedes EXDIS).
//! Each citation re-verified against `crates/capco/docs/CAPCO-2016.md`
//! at authorship per Constitution VIII.

use marque_capco::scheme::TOK_EXDIS;
use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, Severity};
use marque_scheme::{FactRef, ReplacementIntent, Scope, SectionLetter, capco};

const E037_PREDICATE: &str = "portion.dissem.nodis-conflicts-exdis";
const E038_PREDICATE: &str = "portion.dissem.nodis-or-exdis-requires-noforn";
const E039_PREDICATE: &str = "page.dissem.nodis-exdis-clears-banner-rel-to";
const E040_PREDICATE: &str = "banner.banner-rollup.non-ic-dissem-roll-up";
const E041_PREDICATE: &str = "portion.dissem.nodis-supersedes-exdis-in-portion";

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

fn diags_for(source: &str, predicate: &str) -> Vec<Diagnostic<CapcoScheme>> {
    lint(source)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == predicate)
        .collect()
}

// ===========================================================================
// E037 — NODIS conflicts with EXDIS in banner (§H.9 p172 + p174)
// ===========================================================================

/// Banner with both NODIS and EXDIS triggers E037. NOFORN is added
/// to silence E038 (which would also fire on either NODIS or EXDIS
/// without NOFORN); the residual is one E037 diagnostic with the
/// citation pinned at §H.9 p172.
///
/// Authority: CAPCO-2016 §H.9 p172 (EXDIS — "EXCLUSIVE DISTRIBUTION";
/// the primary authority anchor; the cross-reference to p174 NODIS
/// lives in the rule doc comment per PR 10.A.1 / D13 single-citation
/// discipline). Re-verified against `crates/capco/docs/CAPCO-2016.md`
/// per Constitution VIII.
#[test]
fn e037_fires_when_nodis_and_exdis_coexist_and_pins_h9_p172() {
    let diags = diags_for("SECRET//NOFORN//NODIS/EXDIS", E037_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E037 must fire when both NODIS and EXDIS are present: {diags:?}",
    );
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::H, 9, 172),
        "E037 citation must pin §H.9 p172; got: {:?}",
        diags[0].citation,
    );
}

/// E037 conscious-defer symmetry pin. The Stage-4 target is `Reject {
/// suggest: None }` — error diagnostic with no auto-applied fix —
/// because CAPCO-2016 §H.9 prescribes mutual exclusion at the banner
/// without specifying which token to keep. The G13 closure walker
/// (`crates/capco/tests/g13_closure_fix_intent.rs`) only inspects
/// rules whose fix path is populated; this symmetry pin is the only
/// guard against asymmetric drift toward `fix.is_some()` /
/// `text_correction.is_some()` on this rule.
///
/// Pre-PR-3c.B-Commit-10 the assertion was dual-field
/// (`fix.is_none() && fix_intent.is_none()`). Post-cutover
/// `Diagnostic` has a single `fix: Option<FixIntent<S>>` plus
/// `text_correction: Option<TextCorrection>`; the assertion collapses
/// to "neither fix nor text_correction is populated".
#[test]
fn e037_emits_neither_fix_nor_text_correction_pending_stage4_b_reject() {
    let diags = diags_for("SECRET//NOFORN//NODIS/EXDIS", E037_PREDICATE);
    let e037 = diags.first().expect("E037 must fire on banner NODIS+EXDIS");
    assert!(
        e037.fix.is_none() && e037.text_correction.is_none(),
        "E037 must consciously decline to emit a fix or text_correction \
         until Stage-4 B-Reject consolidation lands; see followups/\
         incompatibility-primitive-consolidation.md. Got fix={:?}, \
         text_correction={:?}",
        e037.fix,
        e037.text_correction,
    );
}

// ===========================================================================
// E038 — NODIS / EXDIS require NOFORN (§H.9 p172 + p174)
// ===========================================================================

/// E038 fires on NODIS-without-NOFORN. The citation pins §H.9 p172
/// (the EXDIS authority — single-citation discipline per D13; the
/// p174 NODIS cross-reference lives in the rule doc comment).
///
/// Authority: CAPCO-2016 §H.9 p172. Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e038_fires_on_nodis_without_noforn_and_pins_h9_p172() {
    let diags = diags_for("SECRET//NODIS", E038_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E038 must fire on NODIS without NOFORN: {diags:?}",
    );
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::H, 9, 172),
        "E038 citation must pin §H.9 p172; got: {:?}",
        diags[0].citation,
    );
}

/// E038 must fire EXACTLY ONCE even when both NODIS and EXDIS lack
/// NOFORN. The declarative constraint fuses the disjunction
/// (`NODIS ∪ EXDIS requires NOFORN`) into a single violation — a
/// regression that splits it into two per-token violations would
/// double-count the same logical requirement.
#[test]
fn e038_fires_only_once_when_both_nodis_and_exdis_lack_noforn() {
    let diags = diags_for("SECRET//NODIS/EXDIS", E038_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E038 must fire exactly once even when both NODIS and EXDIS \
         are present: {diags:?}",
    );
}

// ===========================================================================
// E039 — REL TO cleared from banner when portion has NODIS/EXDIS (§H.9)
// ===========================================================================

/// Portion carries NODIS; banner carries REL TO. §H.9 p174:
/// REL TO not authorized in banner when any portion has NODIS. E039
/// fires + carries NO fix (removing REL TO is multi-span and
/// requires human judgment; the engine refuses to guess).
///
/// Authority: CAPCO-2016 §H.9 p172 (EXDIS primary anchor; cross-
/// reference to p174 NODIS in rule doc). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e039_fires_on_banner_rel_to_with_nodis_portion_no_fix() {
    let src = "(S//NF//ND)\nSECRET//NOFORN//NODIS//REL TO USA, GBR";
    let diags = diags_for(src, E039_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E039 must fire on banner REL TO + portion NODIS: {diags:?}",
    );
    assert!(
        diags[0].fix.is_none() && diags[0].text_correction.is_none(),
        "E039 must NOT carry a fix (removing REL TO is multi-span and \
         requires human judgment): {:?}",
        diags[0],
    );
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::H, 9, 172),
        "E039 citation must pin §H.9 p172; got: {:?}",
        diags[0].citation,
    );
}

/// PR 3c.B-8F-engine-gap regression pin. E039 must continue firing
/// after the engine gap close — the rule's check path consumes
/// `attrs.rel_to` (literal banner REL TO list) AND the page-level
/// NODIS/EXDIS projection; neither is affected by the gap close.
///
/// This test exists to document why E039 is preserved (not retired)
/// by the gap-close PR: E039's check path is independent of the
/// engine-side read-API consistency adjustment.
#[test]
fn e039_still_fires_after_pr_3cb_8f_engine_gap_close() {
    let src = "(S//NODIS)\nSECRET//NODIS//REL TO USA";
    let diags = diags_for(src, E039_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E039 must continue firing post-PR-3c.B-8F gap close: {diags:?}",
    );
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::H, 9, 172),
        "E039 citation must continue to pin §H.9 p172: {:?}",
        diags[0].citation,
    );
}

// ===========================================================================
// E040 — Banner roll-up: NODIS supersedes EXDIS (E031 walker per-row)
// ===========================================================================

/// Portions have both NODIS and EXDIS; banner has neither. Per §H.9
/// p172 and §H.9 p174, NODIS has priority over EXDIS in the banner
/// roll-up. The walker (`BannerMatchesProjectedRule`) MUST emit a
/// single E040 diagnostic naming NODIS as the missing token (not
/// EXDIS).
///
/// Pre-cutover the test asserted on `fix.replacement.as_ref() ==
/// "/NODIS"` (FixProposal narrow-splice shape). Post-cutover the
/// walker emits a structural intent + the engine synthesizes the
/// fix at promotion time; the load-bearing invariant — "NODIS wins
/// over EXDIS in the banner" — is observed via the diagnostic's
/// presence (it's the E040 catalog row that names the NODIS-vs-EXDIS
/// priority; the alternative — E040 firing for EXDIS — would be a
/// regression).
///
/// Authority: CAPCO-2016 §H.9 p174 (NODIS supersedes EXDIS) +
/// §H.9 p172 (EXDIS authority). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e040_nodis_has_priority_over_exdis_when_both_in_portions() {
    let src = "(S//NF//ND)\n(S//NF//XD)\nSECRET//NOFORN//LIMDIS";
    let diags = diags_for(src, E040_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E040 must fire when banner omits NODIS (which wins over \
         EXDIS in the banner roll-up): {diags:?}",
    );
}

/// E040 must NOT propose a fix when the banner has no Non-IC dissem
/// block at all. Inserting a new category block is byte-unsafe (needs
/// separator-positioning); the walker emits a no-fix Error in that
/// shape.
#[test]
fn e040_emits_no_fix_when_banner_has_no_non_ic_dissem_block() {
    let src = "(S//NF//ND)\nSECRET//NOFORN";
    let diags = diags_for(src, E040_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E040 must fire on missing Non-IC block: {diags:?}"
    );
    assert!(
        diags[0].fix.is_none() && diags[0].text_correction.is_none(),
        "E040 must NOT carry a fix when banner has no Non-IC dissem \
         block (byte-positioning a new block is unsafe): {:?}",
        diags[0],
    );
}

// ===========================================================================
// E041 — NODIS supersedes EXDIS within a portion (§H.9)
// ===========================================================================

/// E041 fires on a portion carrying both NODIS and EXDIS. Severity
/// is `Warn`; the rule emits an intent-only `FactRemove(EXDIS,
/// Scope::Portion)` fix that the engine auto-applies via the
/// synthesis path (PR 3c.B Sub-PR 8.E.2). The legacy `fix` field
/// IS `Option<FixIntent<S>>` post-cutover — it's the intent slot.
///
/// Authority: CAPCO-2016 §H.9 p174 (NODIS supersedes EXDIS) +
/// §H.9 p172 (EXDIS authority). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e041_fires_warn_with_factremove_intent() {
    let diags = diags_for("(S//NF//ND/XD)", E041_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E041 must fire on portion with both NODIS and EXDIS: {diags:?}"
    );
    assert_eq!(diags[0].severity, Severity::Warn);
    assert!(
        diags[0].fix.is_some(),
        "E041 must emit a FixIntent (FactRemove EXDIS at Portion scope); \
         got fix={:?}",
        diags[0].fix,
    );
}

/// E041's diagnostic span MUST point at the EXDIS token regardless
/// of whether it appears before or after NODIS in the portion.
/// Exercise both orderings.
#[test]
fn e041_points_at_exdis_token_in_both_orderings() {
    for src in ["(S//NF//ND/XD)", "(S//NF//XD/ND)"] {
        let diags = diags_for(src, E041_PREDICATE);
        assert_eq!(diags.len(), 1, "E041 must fire on {src:?}: {diags:?}");
        let span_text = diags[0]
            .span
            .as_str(src.as_bytes())
            .expect("E041 span must be valid UTF-8");
        assert_eq!(
            span_text, "XD",
            "E041 span must point at the EXDIS token in {src:?}; \
             got: {span_text:?}",
        );
    }
}

/// E041 is portion-only per CAPCO-2016 §H.9 p172 + p174 ("in the
/// portion mark"). The banner case is owned by E037 (mutual
/// exclusion at banner-level, Error severity). Pin both: E041 stays
/// silent on banner context AND E037 still fires on the banner
/// NODIS+EXDIS combination.
#[test]
fn e041_does_not_fire_on_banner_but_e037_does() {
    let diags = lint("SECRET//NOFORN//NODIS/EXDIS");
    assert!(
        diags
            .iter()
            .all(|d| d.rule.predicate_id() != E041_PREDICATE),
        "E041 must not fire on banner context: {diags:?}",
    );
    assert!(
        diags
            .iter()
            .any(|d| d.rule.predicate_id() == E037_PREDICATE),
        "E037 must still fire on banner NODIS+EXDIS: {diags:?}",
    );
}

/// E041 emits an intent-only `ReplacementIntent::FactRemove` payload.
/// Three load-bearing invariants the synthesis path consumes:
///
/// 1. The replacement variant is `FactRemove`.
/// 2. The single fact targets `TOK_EXDIS` (§H.9 names EXDIS as the
///    loser when both NODIS and EXDIS are present).
/// 3. The scope is `Scope::Portion` (§H.9 p172 + p174 "in the
///    portion mark").
///
/// All three are needed: a drift toward `FactAdd`, wrong token, or
/// wrong scope would silently change which token gets removed.
///
/// (`candidate_span.is_some()` was a fourth invariant pre-cutover
/// — `synthesize_intent_only_fixes` skipped any intent diagnostic
/// whose `candidate_span` was `None`. Post-cutover the engine's
/// synthesis path is generalized; the test still asserts the candidate
/// span is populated as a defensive guard.)
#[test]
fn e041_emits_intent_only_factremove_exdis_portion() {
    let diags = diags_for("(S//NF//ND/XD)", E041_PREDICATE);
    let e041 = diags.first().expect("E041 must fire on `(S//NF//ND/XD)`");

    let intent = e041.fix.as_ref().expect(
        "E041 must emit `fix: Some(FixIntent { replacement: FactRemove(EXDIS, Portion), .. })`",
    );
    match &intent.replacement {
        ReplacementIntent::FactRemove { facts, scope } => {
            assert_eq!(
                facts.len(),
                1,
                "E041 FactRemove must have exactly one fact (EXDIS); got: {facts:?}",
            );
            assert_eq!(
                facts[0],
                FactRef::Cve(TOK_EXDIS),
                "E041 intent must target EXDIS (§H.9 names EXDIS as the \
                 loser); got: {:?}",
                facts[0],
            );
            assert_eq!(
                *scope,
                Scope::Portion,
                "E041 intent scope must be Portion per §H.9 p172 + p174 \
                 (\"in the portion mark\"); got: {scope:?}",
            );
        }
        other => panic!("E041 intent must be ReplacementIntent::FactRemove; got: {other:?}"),
    }

    assert!(
        e041.candidate_span.is_some(),
        "E041 must populate `candidate_span` so the engine's synthesis \
         path knows which scope-bytes to re-render; got: {:?}",
        e041.candidate_span,
    );
}
