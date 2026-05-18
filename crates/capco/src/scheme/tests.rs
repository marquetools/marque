// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` in-module unit tests. Lifted from the monolithic
//! `scheme.rs` per the issue #466 split plan
//! (`claudedocs/refactor-466/split_proposal.md`).
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
    // `satisfies_attrs` arm exists for future T035b consumption
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
// (PR 3c.B engine-prereq Commit 3)

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
    // PR 3c.B Sub-PR 8.D.2: TOK_REL_TO is the whole-axis-clear
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

/// PR 3c.B Sub-PR 8.D.1 — first consumer of FactAdd lands NOFORN
/// add semantics on `CAT_DISSEM`. Replaces the pre-migration
/// "FactAdd is always inapplicable" pin; the three cases below
/// (a/b/c) cover the wired-axis success path, the
/// idempotence-on-already-present path, and the unwired-axis
/// regression guard that confirms the stub-removal did not
/// over-reach into axes whose migration is still queued.
///
/// Case (a): bare classification marking → FactAdd(NOFORN, Portion)
/// places NOFORN into `attrs.dissem_us` (post PR 9b / FR-046 split;
/// see D9b-1 in decisions.md re the dissem_us-only write target).
/// The lone Secret classification on `mk_attrs()` has an empty
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
/// contract (crates/scheme/src/scheme.rs:185-194).
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
    // contract (scheme/src/scheme.rs:185-194), this aggregates to
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
         IntentInapplicable (only CAT_DISSEM is wired in Sub-PR 8.D.1)"
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

// PR 3c.B Sub-PR 8.D.2 — CAT_REL_TO whole-axis-clear sentinel
// (TOK_REL_TO) extends the FactRemove routing E053 uses to clear
// REL TO when NOFORN is present per §H.8 p145. The three cases
// below cover: (a) wired-axis success path on a populated REL TO
// axis; (b) per-intent inapplicability on an empty axis (trait
// contract `crates/scheme/src/scheme.rs:185-194`); (c) regression
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
        citation: "test",
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
        citation: "test",
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
// PR 4b-D.2 Commit 6 — any_closure_trigger_fires predicate (in-crate
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

/// The short-circuit predicate returns `true` on `(S)` — uncaveated
/// US-classified markings became a Trio 2 closure trigger in Issue
/// #524 Phase 3 (`CLOSURE_RELIDO_US_CLASS`, `marque-applied.md`
/// Section 4.7.5). Pre-Phase-3 this case returned `false`; the
/// behavior flipped intentionally with the implicit-RELIDO row.
#[test]
fn any_closure_trigger_fires_true_on_uncaveated_us_classified() {
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(mk_attrs());
    assert!(
        scheme.any_closure_trigger_fires(&m),
        "uncaveated US-classified should trip the Trio 2 US_CLASS trigger \
         (`CLOSURE_RELIDO_US_CLASS` per marque-applied Section 4.7.5)"
    );
}

/// The short-circuit predicate returns `true` when ORCON is present
/// (Trio-1 NOFORN trigger).
#[test]
fn any_closure_trigger_fires_true_on_orcon() {
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Oc].into();
    let m = CapcoMarking::new(a);
    assert!(scheme.any_closure_trigger_fires(&m));
}

/// The short-circuit predicate returns `true` on bare NATO classification
/// (CLOSURE_REL_TO_USA_NATO trigger via TOK_NATO_CLASS).
#[test]
fn any_closure_trigger_fires_true_on_bare_nato() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(
        marque_ism::NatoClassification::NatoSecret,
    ));
    let m = CapcoMarking::new(a);
    assert!(scheme.any_closure_trigger_fires(&m));
}

/// The short-circuit predicate consults `trigger_fires` only — NOT
/// `should_fire` (which also checks suppression). When ORCON + NOFORN
/// coexist (NOFORN is in FDR_DOMINATORS so it suppresses the closure),
/// the predicate must return `true`. The fixpoint runs and finds
/// nothing to add (suppressed), but the short-circuit must not skip.
#[test]
fn any_closure_trigger_fires_returns_true_even_when_suppressed() {
    let scheme = CapcoScheme::new();
    let mut a = mk_attrs();
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Nf].into();
    let m = CapcoMarking::new(a);
    assert!(
        scheme.any_closure_trigger_fires(&m),
        "predicate must consult triggers only, not suppressors — \
         ORCON trigger is present even though NOFORN suppresses"
    );
}
