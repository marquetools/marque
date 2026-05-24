// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! S005 (`capco:page.dissem.rel-to-uncertain-reduction`) issue-#722
//! behavioral ports — split-sibling of `s005_pagefinalization.rs` to
//! keep both files under the 800-line coding-style gate.
//!
//! # Source tests disposition
//!
//! The disabled `s005_*` tests in `_disabled_tests.rs` fell into
//! three groups:
//!
//! 1. **Helper-fn unit tests** (`s005_state_text`, `s005_expand_atomic`,
//!    `s005_render_set`) — ported to a colocated `#[cfg(test)] mod
//!    tests` block in `crates/capco/src/rules/rel_to_uncertainty.rs`
//!    (helpers are private `fn`; widening visibility for test reach is
//!    forbidden per `feedback_pub_doc_hidden_is_still_public_api`).
//!
//! 2. **Message-content assertions** (`s005_quotes_verbatim_taxonomy_*`,
//!    `s005_handles_empty_atom_intersection`, parts of
//!    `s005_multi_portion_uses_intersection_*`) — obsoleted by the
//!    audit content-ignorance closure. S005 now emits
//!    `MessageTemplate::NonCanonicalOrder` with
//!    `category=Some(CAT_REL_TO)` and no runtime values; the
//!    per-message taxonomy / per-token prose those tests asserted
//!    on is no longer reachable from `Message` (which has no
//!    `.contains()` method and no free-form String field). The
//!    structural property they guarded is preserved at the type
//!    level. NOT PORTED.
//!
//! 3. **Behavioral assertions on diagnostic presence / absence** —
//!    ported below. These don't depend on message content; the
//!    legacy `count_s005_or_s006` helper collapses to a simple
//!    S005 count under the post-#488 single-Suggest model.
//!
//! # Source tests ported in this file
//!
//! - `s005_does_not_fire_when_portions_without_x_have_disjoint_atoms`
//!   (port-map #58) — load-bearing intersection-vs-union semantics
//!   pin.
//! - `s005_emits_no_fix_and_no_fix_intent_pending_stage4_admonition_channel`
//!   (port-map #164) — conscious-defer symmetry pin (collapses
//!   post-cutover dual-field assertion to single-field
//!   `fix.is_none() && text_correction.is_none()`).
//!
//! # Authority
//!
//! CAPCO-2016 §H.8 + §D.2 Table 3 rule 21 (REL TO atom-semantics).
//! Re-verified against `crates/capco/docs/CAPCO-2016.md` at
//! authorship per Constitution VIII.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::{Engine, FixedClock};
use marque_rules::Diagnostic;

const S005_PREDICATE_ID: &str = "page.dissem.rel-to-uncertain-reduction";

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn count_s005(diags: &[Diagnostic<CapcoScheme>]) -> usize {
    diags
        .iter()
        .filter(|d| d.rule.predicate_id() == S005_PREDICATE_ID)
        .count()
}

/// Three portions: p1 has X=RSMA, p2 has GBR but not AUS, p3 has
/// AUS but not GBR. `atoms_in_every_without_x = intersect({USA,
/// GBR}, {USA, AUS}) = {USA}`. After subtracting `expected={USA}`
/// and `{RSMA}`, `other_codes = {}`. The rule MUST stay silent —
/// even hypothetically including GBR or AUS in RSMA's membership
/// wouldn't make either survive intersection (the OTHER non-X
/// portion lacks them). This pins the intersection-vs-union
/// semantics: a union implementation would have produced
/// `other_codes={GBR, AUS}` and fired a false positive.
///
/// Authority: CAPCO-2016 §H.8 + §D.2 Table 3 rule 21 (REL TO
/// atom-semantics intersection). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn s005_does_not_fire_when_portions_without_x_have_disjoint_atoms() {
    let source = b"(S//REL TO USA, RSMA)\n\
                   (S//REL TO USA, GBR)\n\
                   (S//REL TO USA, AUS)\n\
                   SECRET//NOFORN";
    let diags = engine().lint(source).diagnostics;
    assert_eq!(
        count_s005(&diags),
        0,
        "S005 must not fire when portions-without-X have disjoint atoms \
         outside expected (intersection wipes them — union would \
         false-positive): {diags:?}",
    );
}

/// S005 conscious-defer symmetry pin. The Stage-4 target is the
/// "admonition channel" (the rule surfaces an unresolvable REL TO
/// uncertainty; the engine declines to guess at a fix). That
/// channel does not yet exist; S005 emits neither `fix` nor
/// `text_correction`.
///
/// The assertion was formerly dual-field
/// (`fix.is_none() && fix_intent.is_none()`). The
/// `Diagnostic` now has a single `fix: Option<FixIntent<S>>` plus
/// `text_correction`; the dual-field assertion collapses to
/// "neither `fix` nor `text_correction` is populated".
#[test]
fn s005_emits_neither_fix_nor_text_correction_pending_stage4_admonition() {
    // Three-portion fixture that fires S005 (RSMA is opaque-uncertain;
    // intersection with the two GBR-bearing non-X portions includes
    // GBR, which falls outside `expected={USA}` and the trigger
    // `{RSMA}`).
    let source = b"(S//REL TO USA, RSMA)\n\
                   (S//REL TO USA, GBR)\n\
                   (S//REL TO USA, GBR)\n\
                   SECRET//NOFORN";
    let diags = engine().lint(source).diagnostics;
    let s005 = diags
        .iter()
        .find(|d| d.rule.predicate_id() == S005_PREDICATE_ID)
        .unwrap_or_else(|| panic!("S005 must fire on RSMA 3-portion fixture: {diags:?}"));
    assert!(
        s005.fix.is_none() && s005.text_correction.is_none(),
        "S005 must consciously decline to emit a fix or text_correction \
         until the Stage-4 admonition channel lands; got fix={:?}, \
         text_correction={:?}",
        s005.fix,
        s005.text_correction,
    );
}
