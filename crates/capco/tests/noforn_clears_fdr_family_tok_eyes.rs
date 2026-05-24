// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Regression test for issue #546 — `noforn-clears-fdr-family`
//! `PageRewrite`'s `FactRemove` batch (`[TOK_RELIDO, TOK_DISPLAY_ONLY,
//! TOK_EYES]`) must dispatch all three sub-intents through
//! `apply_fact_remove(CAT_DISSEM)` without returning
//! `ApplyIntentError::UnknownToken`.
//!
//! Pre-fix the `CAT_DISSEM` arm in
//! `crates/capco/src/scheme/actions/intent.rs` had no `TOK_EYES =>
//! DissemControl::Eyes` match, so the `TOK_EYES` sub-intent fell
//! through to `_ => Err(ApplyIntentError::UnknownToken)`. The batch
//! dispatcher in [`<CapcoScheme as MarkingScheme>::apply_intent`]
//! propagates that error up via early-return, aborting the batch
//! AFTER any preceding sub-intent (RELIDO / DISPLAY_ONLY) had already
//! mutated the marking — silent partial mutation that the engine then
//! reported as a no-op'd rewrite. On the engine projection path the
//! same error surfaced as an `ERROR`-level tracing event on every
//! NOFORN-bearing page (a routine state), drowning out real catalog
//! regressions on that high-signal channel.
//!
//! These tests pin both the positive-mutation case (EYES present →
//! removed) and the absent-token case (EYES absent → silent per-fact
//! no-op).
//!
//! # Authority
//!
//! `noforn-clears-fdr-family`'s citation chain (declared on the
//! rewrite in `rewrites::noforn_clears::noforn_clears_rows`):
//! `§D.2 Table 3 row 2 + §H.8 p154 + §H.8 p157`. §H.8 p145 is the
//! NOFORN-dominates-FD&R-family rule the rewrite enforces; §H.8 p157
//! is the EYES ONLY entry that retains `DissemControl::Eyes` through
//! lint (the parser preserves it; E064 handles the EYES → REL TO
//! migration at fix time). Both citations re-verified 2026-05-18
//! against `crates/capco/docs/CAPCO-2016.md`.

use marque_capco::scheme::{TOK_DISPLAY_ONLY, TOK_EYES, TOK_RELIDO};
use marque_capco::{CapcoMarking, CapcoScheme};
use marque_ism::{CanonicalAttrs, Classification, DissemControl, MarkingClassification};
use marque_scheme::{FactRef, MarkingScheme, ReplacementIntent, Scope};

fn marking_with(dissem_us: Vec<DissemControl>) -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = dissem_us.into_boxed_slice();
    CapcoMarking::new(a)
}

/// Build the exact `FactRemove` batch the `noforn-clears-fdr-family`
/// rewrite emits (declared in
/// `rewrites::noforn_clears::noforn_clears_rows`).
fn fdr_family_batch() -> [ReplacementIntent<CapcoScheme>; 1] {
    [ReplacementIntent::FactRemove {
        facts: smallvec::smallvec![
            FactRef::Cve(TOK_RELIDO),
            FactRef::Cve(TOK_DISPLAY_ONLY),
            FactRef::Cve(TOK_EYES),
        ],
        scope: Scope::Page,
    }]
}

/// Positive-mutation case: a marking carrying NOFORN + EYES must have
/// EYES removed by the rewrite's `FactRemove` batch. Pre-fix this
/// returned `Err(UnknownToken)` and EYES stayed in the marking.
#[test]
fn fdr_family_batch_removes_eyes_when_present() {
    let scheme = CapcoScheme::new();
    let marking = marking_with(vec![DissemControl::Nf, DissemControl::Eyes]);

    let intents = fdr_family_batch();
    let out = scheme
        .apply_intent(&marking, &intents)
        .expect("FactRemove[RELIDO,DISPLAY_ONLY,EYES] must succeed when EYES is present (#546)");

    let dissem: Vec<DissemControl> = out.0.dissem_iter().copied().collect();
    assert!(
        !dissem.contains(&DissemControl::Eyes),
        "EYES must be removed by the FactRemove batch; got dissem = {dissem:?}",
    );
    assert!(
        dissem.contains(&DissemControl::Nf),
        "NOFORN must remain (it is not in the FactRemove batch); got dissem = {dissem:?}",
    );
}

/// All-three-present case: pins the partial-mutation guard. With
/// RELIDO + DISPLAY_ONLY + EYES + NOFORN in the marking, the batch
/// must mutate all three FD&R-family tokens atomically. Pre-fix the
/// EYES sub-intent aborted the batch AFTER RELIDO and DISPLAY_ONLY
/// had already been removed — silent disagreement between the
/// rendered marking and the engine's "rewrite was a no-op" log.
#[test]
fn fdr_family_batch_removes_all_three_atomically() {
    let scheme = CapcoScheme::new();
    let marking = marking_with(vec![
        DissemControl::Nf,
        DissemControl::Relido,
        DissemControl::Displayonly,
        DissemControl::Eyes,
    ]);

    let intents = fdr_family_batch();
    let out = scheme.apply_intent(&marking, &intents).expect(
        "FactRemove batch must succeed atomically with all three FD&R tokens present (#546)",
    );

    let dissem: Vec<DissemControl> = out.0.dissem_iter().copied().collect();
    assert!(
        !dissem.contains(&DissemControl::Relido),
        "RELIDO must be removed; got dissem = {dissem:?}",
    );
    assert!(
        !dissem.contains(&DissemControl::Displayonly),
        "DISPLAY ONLY must be removed; got dissem = {dissem:?}",
    );
    assert!(
        !dissem.contains(&DissemControl::Eyes),
        "EYES must be removed (#546 — pre-fix the EYES sub-intent returned UnknownToken \
         after the preceding RELIDO/DISPLAY_ONLY removes had already landed); \
         got dissem = {dissem:?}",
    );
    assert!(
        dissem.contains(&DissemControl::Nf),
        "NOFORN must remain (not in the FactRemove batch); got dissem = {dissem:?}",
    );
}

/// Absent-token case: a marking carrying NOFORN but none of the
/// dominated FD&R tokens must surface the whole batch as
/// `Err(IntentInapplicable)` — NOT `Err(UnknownToken)`. Pre-fix the
/// TOK_EYES sub-intent returned `UnknownToken` regardless of whether
/// EYES was present in the marking (the match arm dispatched purely
/// by token id, not by presence), so on every NOFORN-bearing page the
/// engine logged the `ERROR`-level "PageRewrite Intent failed at
/// runtime" event — the routine-state bug that masked real catalog
/// regressions on that channel.
#[test]
fn fdr_family_batch_is_inapplicable_when_no_fdr_tokens_present() {
    let scheme = CapcoScheme::new();
    let marking = marking_with(vec![DissemControl::Nf]);

    let intents = fdr_family_batch();
    let result = scheme.apply_intent(&marking, &intents);

    use marque_scheme::ApplyIntentError;
    assert!(
        matches!(result, Err(ApplyIntentError::IntentInapplicable)),
        "FactRemove batch on a NOFORN-only marking must aggregate to \
         Err(IntentInapplicable) (whole-batch no-op), NOT Err(UnknownToken) \
         (#546 — pre-fix the TOK_EYES arm dispatched to `_ => UnknownToken` \
         regardless of presence, turning every NOFORN-bearing projection into \
         a routine ERROR log); got {result:?}",
    );
}
