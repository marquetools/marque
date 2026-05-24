// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` in-module unit tests.
//!
//! The `#[doc(hidden)] pub` test constructors `with_rewrites` and
//! `with_extra_rewrite_for_tests` (used by integration tests under
//! `crates/capco/tests/`) live in `mod.rs` because they are not gated
//! behind `cfg(test)` and need to be reachable from non-test crates.

use super::*;

use marque_ism::{CanonicalAttrs, CountryCode, DissemControl, MarkingClassification};

fn mk_attrs() -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a
}

// capco_category_contains — all branches

#[test]
fn category_contains_detects_noforn_in_dissem() {
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Nf].into();
    let m = CapcoMarking::new(a);
    assert!(capco_category_contains(&m, CAT_DISSEM, TOK_NOFORN));
}

#[test]
fn category_contains_returns_false_on_absent_token() {
    let a = mk_attrs();
    let m = CapcoMarking::new(a);
    assert!(!capco_category_contains(&m, CAT_DISSEM, TOK_NOFORN));
}

#[test]
fn satisfies_tok_usa_reads_rel_to_for_country_code_usa() {
    // Pin the `TokenRef::Token(TOK_USA)` predicate path
    // touched by issue #183 PR-A's `Trigraph::USA` →
    // `CountryCode::USA` rename. No constraint in the current
    // catalog dispatches `TokenRef::Token(TOK_USA)` (USA-in-
    // REL-TO is read directly by the rule layer), but the
    // `satisfies_attrs` arm exists for future consumption
    // and must read `rel_to` correctly.
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.rel_to = vec![CountryCode::USA].into();
    let m = CapcoMarking::new(a);
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_USA)));

    let m_empty = CapcoMarking::new(mk_attrs());
    assert!(!scheme.satisfies(&m_empty, &TokenRef::Token(TOK_USA)));
}

#[test]
fn category_contains_returns_false_for_unhandled_pair() {
    let a = mk_attrs();
    let m = CapcoMarking::new(a);
    // An unhandled (category, token) pair — should be false.
    assert!(!capco_category_contains(&m, CAT_REL_TO, TOK_NOFORN));
    assert!(!capco_category_contains(&m, CAT_DISSEM, TOK_USA));
    assert!(!capco_category_contains(&m, CAT_SCI, TOK_NOFORN));
}

// capco_category_has_values — all branches

#[test]
fn category_has_values_rel_to_populated() {
    let mut a = mk_attrs();
    a.rel_to = vec![CountryCode::USA].into();
    let m = CapcoMarking::new(a);
    assert!(capco_category_has_values(&m, CAT_REL_TO));
}

#[test]
fn category_has_values_rel_to_empty() {
    let a = mk_attrs();
    let m = CapcoMarking::new(a);
    assert!(!capco_category_has_values(&m, CAT_REL_TO));
}

#[test]
fn category_has_values_dissem_populated() {
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Nf].into();
    let m = CapcoMarking::new(a);
    assert!(capco_category_has_values(&m, CAT_DISSEM));
}

#[test]
fn category_has_values_dissem_empty() {
    let m = CapcoMarking::new(mk_attrs());
    assert!(!capco_category_has_values(&m, CAT_DISSEM));
}

#[test]
fn category_has_values_sci_populated_via_sci_controls() {
    let mut a = mk_attrs();
    a.sci_controls = vec![marque_ism::SciControl::Si].into();
    let m = CapcoMarking::new(a);
    assert!(capco_category_has_values(&m, CAT_SCI));
}

#[test]
fn category_has_values_sci_empty() {
    let m = CapcoMarking::new(mk_attrs());
    assert!(!capco_category_has_values(&m, CAT_SCI));
}

#[test]
fn category_has_values_unhandled_returns_true() {
    // Unhandled categories default to true ("non-empty / unknown")
    // so `Empty` predicates on them stay inert.
    let m = CapcoMarking::new(mk_attrs());
    assert!(capco_category_has_values(&m, CAT_SAR));
    assert!(capco_category_has_values(&m, CAT_AEA));
    assert!(capco_category_has_values(&m, CAT_FGI_MARKER));
}

// capco_category_clear — all branches

#[test]
fn category_clear_empties_rel_to() {
    let mut a = mk_attrs();
    a.rel_to = vec![CountryCode::USA].into();
    let mut m = CapcoMarking::new(a);
    capco_category_clear(&mut m, CAT_REL_TO);
    assert!(m.0.rel_to.is_empty());
}

#[test]
fn category_clear_empties_dissem() {
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Nf].into();
    let mut m = CapcoMarking::new(a);
    capco_category_clear(&mut m, CAT_DISSEM);
    assert!(m.0.dissem_us.is_empty() && m.0.dissem_nato.is_empty());
}

#[test]
fn category_clear_unhandled_is_noop() {
    let mut a = mk_attrs();
    a.rel_to = vec![CountryCode::USA].into();
    let mut m = CapcoMarking::new(a);
    capco_category_clear(&mut m, CAT_SCI);
    // REL TO untouched — other-category clear was a no-op.
    assert_eq!(m.0.rel_to.len(), 1);
}

// capco_category_replace — all branches

#[test]
fn category_replace_rel_to_copies_from_source() {
    let mut src_attrs = CanonicalAttrs::default();
    src_attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    let src = CapcoMarking::new(src_attrs);

    let mut dst = CapcoMarking::new(mk_attrs());
    capco_category_replace(&mut dst, CAT_REL_TO, &src);
    assert_eq!(dst.0.rel_to.len(), 2);
}

#[test]
fn category_replace_dissem_copies_from_source() {
    let mut src_attrs = CanonicalAttrs::default();
    src_attrs.dissem_us = vec![DissemControl::Nf].into();
    let src = CapcoMarking::new(src_attrs);

    let mut dst = CapcoMarking::new(mk_attrs());
    capco_category_replace(&mut dst, CAT_DISSEM, &src);
    assert_eq!(dst.0.dissem_us.as_ref(), &[DissemControl::Nf]);
}

#[test]
fn category_replace_unhandled_is_noop() {
    let src = CapcoMarking::new(mk_attrs());
    let mut dst = CapcoMarking::new(mk_attrs());
    let before = dst.clone();
    capco_category_replace(&mut dst, CAT_SCI, &src);
    assert_eq!(dst, before);
}

// Non-IC dissem axis — engine-prereq additions so FactRemove /
// FactAdd on EXDIS / NODIS / SBU-NF route to the right field
// instead of silently no-opping.

#[test]
fn category_has_values_non_ic_dissem_detects_presence() {
    let empty = CapcoMarking::new(mk_attrs());
    assert!(!capco_category_has_values(&empty, CAT_NON_IC_DISSEM));

    let mut a = mk_attrs();
    a.non_ic_dissem = vec![marque_ism::NonIcDissem::Exdis].into();
    let m = CapcoMarking::new(a);
    assert!(capco_category_has_values(&m, CAT_NON_IC_DISSEM));
}

#[test]
fn category_clear_empties_non_ic_dissem() {
    let mut a = mk_attrs();
    a.non_ic_dissem = vec![
        marque_ism::NonIcDissem::Nodis,
        marque_ism::NonIcDissem::Exdis,
    ]
    .into();
    let mut m = CapcoMarking::new(a);
    capco_category_clear(&mut m, CAT_NON_IC_DISSEM);
    assert!(m.0.non_ic_dissem.is_empty());
}

#[test]
fn category_replace_non_ic_dissem_copies_from_source() {
    let mut src_attrs = CanonicalAttrs::default();
    src_attrs.non_ic_dissem = vec![marque_ism::NonIcDissem::Exdis].into();
    let src = CapcoMarking::new(src_attrs);

    let mut dst = CapcoMarking::new(mk_attrs());
    capco_category_replace(&mut dst, CAT_NON_IC_DISSEM, &src);
    assert_eq!(
        dst.0.non_ic_dissem.as_ref(),
        &[marque_ism::NonIcDissem::Exdis]
    );
}

// category_of — closed-CVE sentinel → CategoryId routing
// (engine-prereq)

#[test]
fn category_of_routes_dissem_tokens() {
    let scheme = CapcoScheme::new();
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_NOFORN)),
        Some(CAT_DISSEM)
    );
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_RELIDO)),
        Some(CAT_DISSEM)
    );
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_DISPLAY_ONLY)),
        Some(CAT_DISSEM)
    );
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_ORCON)),
        Some(CAT_DISSEM)
    );
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_ORCON_USGOV)),
        Some(CAT_DISSEM)
    );
}

#[test]
fn category_of_routes_non_ic_dissem_tokens() {
    let scheme = CapcoScheme::new();
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_NODIS)),
        Some(CAT_NON_IC_DISSEM)
    );
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_EXDIS)),
        Some(CAT_NON_IC_DISSEM)
    );
}

#[test]
fn category_of_routes_rel_to_tokens() {
    let scheme = CapcoScheme::new();
    assert_eq!(scheme.category_of(&FactRef::Cve(TOK_USA)), Some(CAT_REL_TO));
    // TOK_REL_TO is the whole-axis-clear
    // sentinel for CAT_REL_TO (analog to TOK_EXDIS for
    // CAT_NON_IC_DISSEM). Pin its category routing alongside
    // TOK_USA so a future re-shuffle of capco_token_category
    // can't silently drop it.
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_REL_TO)),
        Some(CAT_REL_TO)
    );
}

#[test]
fn category_of_routes_aea_tokens() {
    let scheme = CapcoScheme::new();
    for tok in [TOK_RD, TOK_FRD, TOK_TFNI, TOK_CNWDI, TOK_UCNI] {
        assert_eq!(scheme.category_of(&FactRef::Cve(tok)), Some(CAT_AEA));
    }
}

#[test]
fn category_of_routes_open_vocab_variants() {
    let scheme = CapcoScheme::new();
    assert_eq!(
        scheme.category_of(&FactRef::OpenVocab(CapcoOpenVocabRef::Sar(Box::from(
            "PROGRAM-X"
        )))),
        Some(CAT_SAR)
    );
    assert_eq!(
        scheme.category_of(&FactRef::OpenVocab(CapcoOpenVocabRef::SciCompartment(
            Box::from("G")
        ))),
        Some(CAT_SCI)
    );
    assert_eq!(
        scheme.category_of(&FactRef::OpenVocab(CapcoOpenVocabRef::FgiTetragraph(
            Box::from("FVEY")
        ))),
        Some(CAT_FGI_MARKER)
    );
    assert_eq!(
        scheme.category_of(&FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(
            marque_ism::CountryCode::try_new(b"GBR").expect("GBR is a valid trigraph")
        ))),
        Some(CAT_REL_TO)
    );
}

#[test]
fn category_of_returns_none_for_marker_sentinels() {
    let scheme = CapcoScheme::new();
    // Marker sentinels (used in categorical-presence predicates,
    // not as addressable atomic tokens) have no mapping.
    assert_eq!(scheme.category_of(&FactRef::Cve(TOK_IC_DISSEM)), None);
    assert_eq!(scheme.category_of(&FactRef::Cve(TOK_NON_IC_DISSEM)), None);
    assert_eq!(
        scheme.category_of(&FactRef::Cve(TOK_NON_US_CLASSIFICATION)),
        None
    );
    assert_eq!(scheme.category_of(&FactRef::Cve(TOK_US_CLASSIFIED)), None);
    assert_eq!(scheme.category_of(&FactRef::Cve(TOK_FGI_MARKER)), None);
    // PR #505: `TOK_NATO_CLASS` and `TOK_FGI_CLASS` are per-variant
    // classification-axis marker sentinels (no addressable category).
    // Mirrors `TOK_FGI_MARKER`'s routing.
    assert_eq!(scheme.category_of(&FactRef::Cve(TOK_NATO_CLASS)), None);
    assert_eq!(scheme.category_of(&FactRef::Cve(TOK_FGI_CLASS)), None);
}

// apply_intent — round-trip FactRemove against the wired axes.

#[test]
fn apply_intent_removes_relido_from_dissem() {
    use marque_ism::DissemControl;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Relido, DissemControl::Nf].into();
    let m = CapcoMarking::new(a);

    let intents = [ReplacementIntent::fact_remove(
        FactRef::Cve(TOK_RELIDO),
        Scope::Portion,
    )];
    let out = scheme
        .apply_intent(&m, &intents)
        .expect("RELIDO removal must succeed");
    assert_eq!(out.0.dissem_us.as_ref(), &[DissemControl::Nf]);
}

#[test]
fn apply_intent_remove_absent_token_is_inapplicable() {
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(mk_attrs());
    let intents = [ReplacementIntent::fact_remove(
        FactRef::Cve(TOK_RELIDO),
        Scope::Portion,
    )];
    assert_eq!(
        scheme.apply_intent(&m, &intents),
        Err(ApplyIntentError::IntentInapplicable)
    );
}

#[test]
fn apply_intent_remove_unknown_token_is_unknown() {
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(mk_attrs());
    // TokenId(9999) is not in the sentinel table.
    let intents = [ReplacementIntent::fact_remove(
        FactRef::Cve(TokenId(9999)),
        Scope::Portion,
    )];
    assert_eq!(
        scheme.apply_intent(&m, &intents),
        Err(ApplyIntentError::UnknownToken)
    );
}

#[test]
fn apply_intent_recanonicalize_returns_unchanged_marking() {
    use marque_scheme::RecanonScope;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![marque_ism::DissemControl::Nf].into();
    let m = CapcoMarking::new(a);

    let intents = [ReplacementIntent::Recanonicalize {
        scope: RecanonScope::Portion,
    }];
    let out = scheme
        .apply_intent(&m, &intents)
        .expect("Recanonicalize must succeed");
    // Fact set unchanged — the engine renders the marking via
    // render_canonical to produce canonical form.
    assert_eq!(
        (out.0.dissem_us.as_ref(), out.0.dissem_nato.as_ref()),
        (m.0.dissem_us.as_ref(), m.0.dissem_nato.as_ref())
    );
}

/// First consumer of FactAdd lands NOFORN
/// add semantics on `CAT_DISSEM`. Replaces the pre-migration
/// "FactAdd is always inapplicable" pin; the three cases below
/// (a/b/c) cover the wired-axis success path, the
/// idempotence-on-already-present path, and the unwired-axis
/// regression guard that confirms the stub-removal did not
/// over-reach into axes whose migration is still queued.
///
/// Case (a): bare classification marking → FactAdd(NOFORN, Portion)
/// places NOFORN into `attrs.dissem_us` (the dissem_us-only write
/// target). The lone Secret classification on `mk_attrs()` has an empty
/// dissem axis pre-call; post-call `dissem_us` contains exactly
/// `[Nf]`.
///
/// Case (b): marking already containing NOFORN — FactAdd(NOFORN)
/// is a per-intent no-op and `apply_fact_add` returns
/// `Err(IntentInapplicable)`. The lone intent in this batch
/// produces no mutation, so `apply_intent` aggregates the
/// whole-batch result as `Err(IntentInapplicable)` (the engine
/// silently drops the synthesized fix). Symmetric with
/// FactRemove's "absent token is inapplicable" policy: both axes
/// report per-intent inapplicability when the requested mutation
/// is a no-op, per the `MarkingScheme::apply_intent` trait
/// contract (`MarkingScheme::apply_intent` in crates/scheme/src/scheme.rs).
///
/// Case (c): FactAdd against an unwired axis (CAT_SCI via
/// `TOK_HCS`) returns `Err(IntentInapplicable)`. The routing
/// table maps `TOK_HCS → CAT_SCI`, so the call reaches
/// `apply_fact_add` with the SCI category and falls through to
/// the unwired-axis arm. Regression-guards the stub-removal did
/// not over-reach: only CAT_DISSEM is wired in this sub-PR, and
/// other axes return `IntentInapplicable` until their own
/// migration sub-PRs land.
#[test]
fn apply_fact_add_noforn_adds_to_dissem_us_idempotent() {
    use marque_ism::DissemControl;
    let scheme = CapcoScheme::new();

    // Case (a): bare classification → NOFORN added to dissem.
    let m_bare = CapcoMarking::new(mk_attrs());
    let intents = [ReplacementIntent::FactAdd {
        token: FactRef::Cve(TOK_NOFORN),
        scope: Scope::Portion,
    }];
    let out_a = scheme
        .apply_intent(&m_bare, &intents)
        .expect("FactAdd(NOFORN, Portion) must succeed on bare marking");
    assert_eq!(
        out_a.0.dissem_us.as_ref(),
        &[DissemControl::Nf],
        "after FactAdd(NOFORN) the dissem axis must contain exactly [Nf]"
    );

    // Case (b): marking already containing NOFORN — the whole
    // batch is a per-intent no-op. Per `MarkingScheme::apply_intent`
    // contract (in scheme/src/scheme.rs), this aggregates to
    // `Err(IntentInapplicable)` so the engine drops the synthesized
    // fix. A FactAdd of an already-present token returns per-intent
    // `IntentInapplicable` from `apply_fact_add`; the lone intent
    // in `intents` produces no mutation, so the batch result is
    // `Err`.
    let err_b = scheme
        .apply_intent(&out_a, &intents)
        .expect_err("redundant FactAdd(NOFORN) must aggregate to Err(IntentInapplicable)");
    assert_eq!(
        err_b,
        ApplyIntentError::IntentInapplicable,
        "redundant single-intent FactAdd batch must be IntentInapplicable, not a successful no-op",
    );

    // Case (c): unwired axis (CAT_SCI via TOK_HCS) → IntentInapplicable.
    // Regression guard that the stub-removal did not over-reach
    // into axes whose migration is still queued. `TOK_HCS` routes
    // to `CAT_SCI` via `capco_token_category`; `apply_fact_add`
    // sees a category that is not yet wired and returns
    // `IntentInapplicable`, which propagates through the
    // whole-batch no-op detection (the lone intent in `intents`
    // did not apply, so the batch returns Err).
    let m_unwired = CapcoMarking::new(mk_attrs());
    let unwired_intents = [ReplacementIntent::FactAdd {
        token: FactRef::Cve(TOK_HCS),
        scope: Scope::Portion,
    }];
    assert_eq!(
        scheme.apply_intent(&m_unwired, &unwired_intents),
        Err(ApplyIntentError::IntentInapplicable),
        "FactAdd against the unwired CAT_SCI axis must return \
         IntentInapplicable (only CAT_DISSEM is wired for FactAdd)"
    );
}

#[test]
fn apply_intent_multi_intent_batch_applies_atomically() {
    use marque_ism::DissemControl;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![
        DissemControl::Relido,
        DissemControl::Displayonly,
        DissemControl::Nf,
    ]
    .into();
    let m = CapcoMarking::new(a);

    // Two removals targeting the same axis.
    let intents = [
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_DISPLAY_ONLY), Scope::Portion),
    ];
    let out = scheme
        .apply_intent(&m, &intents)
        .expect("multi-intent batch must succeed");
    // Both tokens removed; NF retained.
    assert_eq!(out.0.dissem_us.as_ref(), &[DissemControl::Nf]);
}

/// Idempotence/commutativity invariant pin — Copilot review on PR #369.
///
/// A redundant intent within a batch (e.g., two rules emit the
/// same `FactRemove`, or one intent in the batch removes a token
/// a prior intent already removed) MUST be treated as a per-intent
/// no-op and MUST NOT abort the rest of the batch. The earlier
/// implementation used `?` to propagate per-intent
/// `IntentInapplicable` errors, which broke the trait-level
/// invariant — fixed in the same commit as this test.
#[test]
fn apply_intent_redundant_intent_within_batch_does_not_abort() {
    use marque_ism::DissemControl;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Relido, DissemControl::Nf].into();
    let m = CapcoMarking::new(a);

    // First intent removes RELIDO (succeeds). Second intent is a
    // redundant FactRemove of the same token — RELIDO is already
    // gone after the first removal. The redundant intent MUST be
    // silently skipped; the batch as a whole MUST succeed because
    // at least one intent had effect.
    let intents = [
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
    ];
    let out = scheme
        .apply_intent(&m, &intents)
        .expect("redundant intent within batch must not abort");
    // RELIDO removed exactly once; NF retained.
    assert_eq!(out.0.dissem_us.as_ref(), &[DissemControl::Nf]);
}

/// Mixed-applicability batch: some intents apply, others are
/// no-ops because their target token is already absent. The batch
/// MUST succeed and apply the applicable subset.
#[test]
fn apply_intent_mixed_applicability_batch_applies_applicable_subset() {
    use marque_ism::DissemControl;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Relido, DissemControl::Nf].into();
    let m = CapcoMarking::new(a);

    // First intent removes DISPLAY ONLY (already absent — no-op
    // per-intent). Second intent removes RELIDO (succeeds). Batch
    // succeeds because RELIDO removal had effect.
    let intents = [
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_DISPLAY_ONLY), Scope::Portion),
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
    ];
    let out = scheme
        .apply_intent(&m, &intents)
        .expect("mixed-applicability batch must apply the applicable subset");
    assert_eq!(out.0.dissem_us.as_ref(), &[DissemControl::Nf]);
}

/// Whole-batch no-op: every intent is inapplicable. The batch
/// returns `Err(IntentInapplicable)` so the engine drops the fix
/// silently. This is the only case where `IntentInapplicable`
/// propagates from `apply_intent`.
#[test]
fn apply_intent_whole_batch_inapplicable_returns_err() {
    use marque_ism::DissemControl;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Nf].into();
    let m = CapcoMarking::new(a);

    // Both intents target tokens not present on this marking.
    let intents = [
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
        ReplacementIntent::fact_remove(FactRef::Cve(TOK_DISPLAY_ONLY), Scope::Portion),
    ];
    assert_eq!(
        scheme.apply_intent(&m, &intents),
        Err(ApplyIntentError::IntentInapplicable)
    );
    // NF retained — the marking is unchanged because no intent
    // applied.
}

// CAT_REL_TO whole-axis-clear sentinel
// (TOK_REL_TO) extends the FactRemove routing E053 uses to clear
// REL TO when NOFORN is present per §H.8 p145. The three cases
// below cover: (a) wired-axis success path on a populated REL TO
// axis; (b) per-intent inapplicability on an empty axis (trait
// contract `MarkingScheme::apply_intent` in crates/scheme/src/scheme.rs); (c) regression
// guard that the pre-existing TOK_USA single-country removal
// path still works post-extension.

#[test]
fn apply_intent_removes_rel_to_whole_axis_sentinel() {
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.rel_to = vec![
        CountryCode::USA,
        CountryCode::try_new(b"GBR").unwrap(),
        CountryCode::try_new(b"AUS").unwrap(),
    ]
    .into();
    let m = CapcoMarking::new(a);

    let intents = [ReplacementIntent::fact_remove(
        FactRef::Cve(TOK_REL_TO),
        Scope::Portion,
    )];
    let out = scheme
        .apply_intent(&m, &intents)
        .expect("TOK_REL_TO whole-axis clear must succeed on populated axis");
    assert!(
        out.0.rel_to.is_empty(),
        "REL TO axis must be empty after whole-axis-clear sentinel"
    );
}

#[test]
fn apply_intent_rel_to_whole_axis_clear_on_empty_is_inapplicable() {
    // Empty REL TO axis: whole-axis-clear sentinel is per-intent
    // inapplicable (trait contract — already-empty axis is a
    // no-op). With a single intent in the batch, the whole-batch
    // result aggregates to `Err(IntentInapplicable)`.
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(mk_attrs());
    assert!(m.0.rel_to.is_empty(), "fixture precondition");

    let intents = [ReplacementIntent::fact_remove(
        FactRef::Cve(TOK_REL_TO),
        Scope::Portion,
    )];
    assert_eq!(
        scheme.apply_intent(&m, &intents),
        Err(ApplyIntentError::IntentInapplicable)
    );
}

#[test]
fn apply_intent_removes_usa_only_regression_guard() {
    // Regression guard: TOK_USA single-country removal still
    // works after the TOK_REL_TO whole-axis-clear sentinel
    // landed alongside it. USA is removed; GBR remains.
    let scheme = CapcoScheme::new();
    let gbr = CountryCode::try_new(b"GBR").unwrap();
    let mut a = mk_attrs();
    a.rel_to = vec![CountryCode::USA, gbr].into();
    let m = CapcoMarking::new(a);

    let intents = [ReplacementIntent::fact_remove(
        FactRef::Cve(TOK_USA),
        Scope::Portion,
    )];
    let out = scheme
        .apply_intent(&m, &intents)
        .expect("TOK_USA single-country removal must succeed");
    assert_eq!(out.0.rel_to.as_ref(), &[gbr], "USA removed, GBR retained");
}

// Declarative rewrite dispatch — exercise the Contains / Empty /
// Clear / Replace match arms inside `project`.

#[test]
fn project_applies_declarative_contains_then_clear() {
    // Construct a scheme with a declarative Contains-trigger +
    // Clear-action rewrite (instead of the default Custom
    // closures). That way the engine hits the Contains and Clear
    // match arms in the project() dispatch.
    let rewrites = vec![PageRewrite {
        id: "test/nf-clears-rel-to",
        citation: marque_scheme::Citation::new(
            marque_scheme::AuthoritativeSource::EngineInternal,
            marque_scheme::SectionRef::new(marque_scheme::SectionLetter::A),
            core::num::NonZeroU16::new(1).unwrap(),
        ),
        trigger: CategoryPredicate::Contains {
            category: CAT_DISSEM,
            token: TOK_NOFORN,
        },
        action: CategoryAction::Clear {
            category: CAT_REL_TO,
        },
        reads: &[CAT_DISSEM],
        writes: &[CAT_REL_TO],
    }];
    let scheme = CapcoScheme::with_rewrites(rewrites);

    // Two portions: one with NOFORN, one with REL TO.
    let mut p1 = mk_attrs();
    p1.dissem_us = vec![DissemControl::Nf].into();
    let mut p2 = mk_attrs();
    p2.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

    let out = marque_scheme::MarkingScheme::project(
        &scheme,
        marque_scheme::Scope::Page,
        &[CapcoMarking::new(p1), CapcoMarking::new(p2)],
    );
    // Rewrite should have fired — REL TO cleared.
    assert!(out.0.rel_to.is_empty());
}

#[test]
fn project_applies_declarative_empty_then_replace() {
    // An Empty trigger on an unhandled category (returns false, so
    // rewrite does NOT fire). Verify a Replace action is reachable
    // via a trigger that DOES fire.
    let mut replacement = CanonicalAttrs::default();
    replacement.dissem_us = vec![DissemControl::Nf].into();

    let rewrites = vec![PageRewrite {
        id: "test/empty-rel-to-triggers-replace-dissem",
        citation: marque_scheme::Citation::new(
            marque_scheme::AuthoritativeSource::EngineInternal,
            marque_scheme::SectionRef::new(marque_scheme::SectionLetter::A),
            core::num::NonZeroU16::new(1).unwrap(),
        ),
        trigger: CategoryPredicate::Empty {
            category: CAT_REL_TO,
        },
        action: CategoryAction::Replace {
            category: CAT_DISSEM,
            with: CapcoMarking::new(replacement),
        },
        reads: &[CAT_REL_TO],
        writes: &[CAT_DISSEM],
    }];
    let scheme = CapcoScheme::with_rewrites(rewrites);

    // Portion with no REL TO — trigger fires → dissem replaced.
    let p = mk_attrs();
    let out = marque_scheme::MarkingScheme::project(
        &scheme,
        marque_scheme::Scope::Page,
        &[CapcoMarking::new(p)],
    );
    assert!(out.0.dissem_us.contains(&DissemControl::Nf));
}

// ---------------------------------------------------------------------------
// any_closure_trigger_fires predicate (in-crate
// pub(crate) reach)
// ---------------------------------------------------------------------------

/// The short-circuit predicate returns `false` on a bottom marking.
/// No catalog rule's trigger fires.
#[test]
fn any_closure_trigger_fires_false_on_bottom() {
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(CanonicalAttrs::default());
    assert!(!scheme.any_closure_trigger_fires(&m));
}

/// Post-#704: the short-circuit predicate returns `false` on bare
/// `(S)` — the pre-#704 Row 9 (`CLOSURE_RELIDO_US_CLASS`) retired to
/// `default_fill::row9_should_fill` because it's a non-monotone
/// "default if absent" rule. `close()` no longer triggers on
/// `US_COLLATERAL_CLASSIFIED`; the default-fill stage in
/// `project_attrs_pipeline` picks up the RELIDO injection.
#[test]
fn any_closure_trigger_fires_false_on_uncaveated_us_classified_post_704() {
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(mk_attrs());
    assert!(
        !scheme.any_closure_trigger_fires(&m),
        "post-#704: uncaveated US-classified is a `default_fill` trigger, \
         not a `close()` trigger. close()'s ALL_TRIGGER_MASK covers only \
         the six SCI per-marking sentinels (Rows 1-6); US_COLLATERAL_CLASSIFIED \
         retired with Row 9 to default-fill."
    );
}

/// Post-#704: the short-circuit predicate returns `false` when only
/// ORCON is present. Pre-#704 Row 0 (`CLOSURE_NOFORN_CAVEATED`) had
/// ORCON in its trigger mask; post-#704 that row retired to
/// `default_fill::row0_should_fill`, so ORCON-alone no longer trips
/// `close()`. The default-fill stage in project_attrs_pipeline reads
/// the post-close bitmask (which still has ORCON from the input) and
/// fires Row 0's NOFORN cone.
#[test]
fn any_closure_trigger_fires_false_on_orcon_alone_post_704() {
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Oc].into();
    let m = CapcoMarking::new(a);
    assert!(
        !scheme.any_closure_trigger_fires(&m),
        "post-#704: bare ORCON is a `default_fill` Row 0 trigger, not a \
         `close()` trigger."
    );
}

/// Post-#704: bare NATO classification retired from close()'s trigger
/// mask (Row 7 → default_fill).
#[test]
fn any_closure_trigger_fires_false_on_bare_nato_post_704() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(
        marque_ism::NatoClassification::NatoSecret,
    ));
    let m = CapcoMarking::new(a);
    assert!(
        !scheme.any_closure_trigger_fires(&m),
        "post-#704: bare NATO classification is a `default_fill` Row 7 \
         trigger, not a `close()` trigger."
    );
}

/// Post-#704: ORCON + NOFORN no longer trips close()'s mask (Row 0
/// retired). The default-fill stage observes the input has NOFORN
/// and skips Row 0 (FD&R already present per §B.3 paragraph b p19).
#[test]
fn any_closure_trigger_fires_false_on_orcon_plus_noforn_post_704() {
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Nf].into();
    let m = CapcoMarking::new(a);
    assert!(
        !scheme.any_closure_trigger_fires(&m),
        "post-#704: ORCON is a default_fill trigger, not a close() trigger. \
         close()'s ALL_TRIGGER_MASK covers six SCI per-marking sentinels."
    );
}

/// Post-#704: the short-circuit predicate fires on SCI per-marking
/// triggers (the six rows that survived close()). HCS-O is the
/// canonical example — Row 1 in the post-#704 CLOSURE_TABLE.
#[test]
fn any_closure_trigger_fires_true_on_hcs_o_post_704() {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};
    use smol_str::SmolStr;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    let comp = SciCompartment::new(SmolStr::new("O"), Box::new([]));
    a.sci_markings = Box::new([SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        Box::new([comp]),
        None,
    )]);
    let m = CapcoMarking::new(a);
    assert!(
        scheme.any_closure_trigger_fires(&m),
        "post-#704: HCS-O is a Row 1 close() trigger (per-marking \
         unconditional implication per §H.4 p64)."
    );
}

// capco_axis_mask — per-page bitmask for eligibility gate (CO-2)

#[test]
fn axis_mask_empty_marking_has_no_bits_set() {
    let m = CapcoMarking::new(CanonicalAttrs::default());
    assert_eq!(capco_axis_mask(&m), 0, "default attrs → zero mask");
}

#[test]
fn axis_mask_us_classification_sets_cat_classification_bit() {
    let m = CapcoMarking::new(mk_attrs());
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_CLASSIFICATION.0) != 0,
        "US classification must set CAT_CLASSIFICATION bit"
    );
    assert!(
        mask & (1 << CAT_NON_US_CLASSIFICATION.0) == 0,
        "US classification must NOT set CAT_NON_US_CLASSIFICATION bit"
    );
    assert!(
        mask & (1 << CAT_JOINT_CLASSIFICATION.0) == 0,
        "US classification must NOT set CAT_JOINT_CLASSIFICATION bit"
    );
}

#[test]
fn axis_mask_nato_classification_sets_non_us_bit() {
    use marque_ism::NatoClassification;
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_CLASSIFICATION.0) != 0,
        "NATO classification must set CAT_CLASSIFICATION bit"
    );
    assert!(
        mask & (1 << CAT_NON_US_CLASSIFICATION.0) != 0,
        "NATO classification must set CAT_NON_US_CLASSIFICATION bit"
    );
    assert!(
        mask & (1 << CAT_JOINT_CLASSIFICATION.0) == 0,
        "NATO classification must NOT set CAT_JOINT_CLASSIFICATION bit"
    );
}

#[test]
fn axis_mask_joint_classification_sets_joint_bit() {
    use marque_ism::{Classification, JointClassification};
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![].into(),
    }));
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_CLASSIFICATION.0) != 0,
        "JOINT classification must set CAT_CLASSIFICATION bit"
    );
    assert!(
        mask & (1 << CAT_JOINT_CLASSIFICATION.0) != 0,
        "JOINT classification must set CAT_JOINT_CLASSIFICATION bit"
    );
    assert!(
        mask & (1 << CAT_NON_US_CLASSIFICATION.0) == 0,
        "JOINT classification must NOT set CAT_NON_US_CLASSIFICATION bit"
    );
}

#[test]
fn axis_mask_dissem_sets_dissem_bit() {
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Nf].into();
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_DISSEM.0) != 0,
        "NOFORN must set CAT_DISSEM bit"
    );
}

#[test]
fn axis_mask_no_dissem_leaves_dissem_bit_clear() {
    let m = CapcoMarking::new(mk_attrs());
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_DISSEM.0) == 0,
        "no dissem controls → CAT_DISSEM bit must be clear"
    );
}

#[test]
fn axis_mask_non_ic_dissem_sets_bit() {
    let mut a = mk_attrs();
    a.non_ic_dissem = vec![marque_ism::NonIcDissem::Nodis].into();
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_NON_IC_DISSEM.0) != 0,
        "NODIS must set CAT_NON_IC_DISSEM bit"
    );
}

#[test]
fn axis_mask_rel_to_sets_bit() {
    let mut a = mk_attrs();
    a.rel_to = vec![CountryCode::USA].into();
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_REL_TO.0) != 0,
        "REL TO non-empty → CAT_REL_TO bit set"
    );
}

#[test]
fn axis_mask_sar_sets_bit() {
    use marque_ism::{SarIndicator, SarMarking, SarProgram};
    let mut a = mk_attrs();
    a.sar_markings = Some(SarMarking::new(
        SarIndicator::Abbrev,
        Box::new([SarProgram::new("BP", Box::new([]))]),
    ));
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_SAR.0) != 0,
        "SAR present → CAT_SAR bit set"
    );
}

#[test]
fn axis_mask_aea_sets_bit() {
    use marque_ism::{AeaMarking, RdBlock};
    let mut a = mk_attrs();
    a.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into();
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_AEA.0) != 0,
        "AEA present → CAT_AEA bit set"
    );
}

#[test]
fn axis_mask_sci_sets_bit_via_controls() {
    let mut a = mk_attrs();
    a.sci_controls = vec![marque_ism::SciControl::Si].into();
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_SCI.0) != 0,
        "SCI controls non-empty → CAT_SCI bit set"
    );
}

#[test]
fn axis_mask_display_only_sets_bit() {
    let mut a = mk_attrs();
    a.display_only_to = vec![CountryCode::try_new(b"GBR").unwrap()].into();
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_DISPLAY_ONLY_TO.0) != 0,
        "DISPLAY ONLY non-empty → CAT_DISPLAY_ONLY_TO bit set"
    );
    // `capco_category_contains(CAT_DISSEM, TOK_DISPLAY_ONLY)` returns true
    // when `display_only_to` is non-empty (canonical parsed form), so the
    // CAT_DISSEM bit must also be set to keep `Contains(CAT_DISSEM,
    // TOK_DISPLAY_ONLY)` triggers (e.g. `capco/display-only-clears-relido`)
    // reachable through the eligibility gate.
    assert!(
        mask & (1 << CAT_DISSEM.0) != 0,
        "DISPLAY ONLY non-empty → CAT_DISSEM bit set (eligibility for Contains(CAT_DISSEM, TOK_DISPLAY_ONLY) triggers)"
    );
}

#[test]
fn axis_mask_declassify_on_sets_bit() {
    use marque_ism::IsmDate;
    let mut a = mk_attrs();
    a.declassify_on = Some(IsmDate::Year(2030));
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_DECLASSIFY_ON.0) != 0,
        "declassify_on populated → CAT_DECLASSIFY_ON bit set"
    );
}

#[test]
fn axis_mask_fgi_marker_sets_bit() {
    use marque_ism::FgiMarker;
    let mut a = mk_attrs();
    a.fgi_marker = Some(FgiMarker::SourceConcealed);
    let m = CapcoMarking::new(a);
    let mask = capco_axis_mask(&m);
    assert!(
        mask & (1 << CAT_FGI_MARKER.0) != 0,
        "FGI marker present → CAT_FGI_MARKER bit set"
    );
}
// ---------------------------------------------------------------------------
// HOT-1 (issue #595) — `ClosureAxisFlags` axis-guard correctness
// ---------------------------------------------------------------------------
//
// These tests verify that the HOT-1 per-rule axis guard never
// incorrectly short-circuits a rule that should fire, and that the
// pre-loop fast path correctly rejects markings with no triggerable
// axis.  Semantics must be byte-identical to the structural closure.

/// HOT-1: post-#704 `project()` semantics inject NOFORN on a marking
/// carrying ORCON via the `default_fill::row0_should_fill` path.
/// Pre-#704 the injection happened inside `close()`'s Kleene fixpoint
/// (Row 0); post-#704 close() is purely additive over Rows 1-6, and
/// the NOFORN injection happens in `apply_default_fill` after close()
/// converges. End-to-end behavior preserved.
#[test]
fn hot1_pipeline_semantics_preserved_for_orcon_trigger() {
    use marque_scheme::Scope;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Oc].into();
    let m = CapcoMarking::new(a);
    let out = scheme.project(Scope::Page, std::slice::from_ref(&m));
    assert!(
        out.0.dissem_us.iter().any(|d| d == &DissemControl::Nf),
        "post-#704 project() must inject NOFORN on ORCON via default-fill \
         Row 0 (the pre-#704 Row 0 in close()'s Kleene fixpoint retired \
         to default_fill::row0_should_fill); dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Post-#704 FD&R supersession at the project() boundary:
/// `project(Page)` on a bare US classified marking WITH NOFORN
/// already present is a no-op on the dissem axis. Post-#704
/// `default_fill::row9_should_fill`'s gate `(post_close ∩
/// US_COLLATERAL_CLASSIFIED != 0) ∧ (post_close ∩
/// MASK_RELIDO_US_CLASS_SUPPRESSORS == 0)` SKIPS when NOFORN is
/// in the input — NOFORN is in `MASK_RELIDO_US_CLASS_SUPPRESSORS`
/// per §B.3.a p19. RELIDO is never injected; the supersession
/// overlay's NOFORN-dominates-RELIDO strip is a no-op (nothing
/// to strip). End state: input `{NOFORN}` → output `{NOFORN}`.
///
/// Authority: §B.3 paragraph b p19 ("not marked previously" gate
/// applies to default_fill Row 9); §B.3.a p19 (NOFORN is canonical
/// FD&R, in MASK_FDR_DOMINATORS and therefore in
/// MASK_RELIDO_US_CLASS_SUPPRESSORS); §H.8 p145 (NOFORN dominates
/// RELIDO, irrelevant on this input because no RELIDO is ever
/// added).
#[test]
fn project_noop_on_classified_with_noforn_via_default_fill_gate() {
    use marque_scheme::Scope;
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs(); // US(Secret)
    a.dissem_us = vec![DissemControl::Nf].into();
    let before = CapcoMarking::new(a);
    let after = scheme.project(Scope::Page, std::slice::from_ref(&before));
    assert_eq!(
        before.0.dissem_us, after.0.dissem_us,
        "project must converge to `{{NOFORN}}` when NOFORN is already \
         present (default_fill::row9_should_fill SKIPS because NOFORN \
         is in MASK_RELIDO_US_CLASS_SUPPRESSORS per §B.3.a p19 — \
         RELIDO is never added so the §H.8 p145 supersession overlay \
         has nothing to strip)"
    );
}

/// HOT-1: `closure()` is a no-op on a bottom marking (all axes empty).
/// The pre-loop short-circuit from `ClosureAxisFlags` must return the
/// input without entering the fixpoint loop.
#[test]
fn hot1_closure_noop_on_bottom_marking() {
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(CanonicalAttrs::default());
    let closed = scheme.closure(m.clone());
    assert_eq!(
        m.0, closed.0,
        "closure on bottom marking must return input unchanged"
    );
}
