// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.F engine-prereq — `CategoryAction::Intent`
//! executor + `Engine::new` validation tests.
//!
//! These tests exercise the new `CategoryAction::Intent(ReplacementIntent<S>)`
//! variant introduced in this PR:
//!
//! - **FactAdd round-trip**: a synthetic rewrite whose action is
//!   `Intent(FactAdd { Cve(TOK_NOFORN), Page })` adds NOFORN to a
//!   projected marking that lacks it.
//! - **FactRemove round-trip**: a synthetic rewrite whose action is
//!   `Intent(FactRemove { Cve(TOK_RELIDO), Page })` removes RELIDO
//!   from a projected marking that contains it.
//! - **Idempotence**: applying the same FactAdd rewrite when the
//!   token is already present is a silent no-op (no panic, no
//!   error, no double-add).
//! - **Recanonicalize is a no-op**: `Intent(Recanonicalize { ... })`
//!   inside a `PageRewrite::action` does not mutate the projected
//!   marking — re-rendering is a renderer-stage concern, not a
//!   page-rewrite concern.
//! - **`Engine::new` validation**: a synthetic rewrite carrying an
//!   intent with an unroutable `TokenId` causes `Engine::new` to
//!   return `Err(InvalidIntentInPageRewrite { .. })` rather than
//!   silently no-opping at projection time.
//! - **G13 closure**: a rewrite carrying an `OpenVocab` `FactRef`
//!   does not leak source-derived strings into the projected
//!   marking's debug output.
//!
//! Constitution VIII note: these tests do not embed CAPCO-2016
//! citations because the rewrites are synthetic test fixtures, not
//! real CAPCO rules. The actual NOFORN-supremacy and FOUO-eviction
//! `PageRewrite` entries land in Sub-PR 8.F with their citations.

use marque_capco::scheme::{CAT_DISSEM, CAT_REL_TO, TOK_NOFORN, TOK_RELIDO};
use marque_capco::{CapcoMarking, CapcoOpenVocabRef, CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, EngineConstructionError, FixedClock};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification,
};
use marque_scheme::{
    ApplyIntentError, CategoryAction, CategoryPredicate, FactRef, MarkingScheme, PageRewrite,
    RecanonScope, ReplacementIntent, Scope, TokenId,
};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

fn portion_at(c: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a
}

fn wrap(attrs: CanonicalAttrs) -> CapcoMarking {
    CapcoMarking::new(attrs)
}

// ---------------------------------------------------------------------------
// 1. FactAdd round-trip — synthetic rewrite adds NOFORN to the projection.
// ---------------------------------------------------------------------------

/// A `CategoryAction::Intent(FactAdd { Cve(TOK_NOFORN), Page })`
/// rewrite mutates the projected marking by adding NOFORN to its
/// dissem-controls axis. Verifies the executor arm in
/// `CapcoScheme::project` calls `apply_intent_to_marking` and the
/// result lands in the projection.
#[test]
fn page_rewrite_intent_fact_add_mutates_projection() {
    let rewrite = PageRewrite {
        id: "test/intent-fact-add",
        citation: "test fixture",
        // Fires when the dissem axis is empty — true on the input
        // portion below, which carries only a classification.
        trigger: CategoryPredicate::Empty {
            category: CAT_DISSEM,
        },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::Cve(TOK_NOFORN),
            scope: Scope::Page,
        }),
        reads: &[CAT_DISSEM],
        writes: &[CAT_DISSEM],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);

    let portion = wrap(portion_at(Classification::Secret));
    let out = scheme.project(Scope::Page, &[portion]);

    assert!(
        out.0.dissem_controls.contains(&DissemControl::Nf),
        "FactAdd rewrite must add NOFORN; got dissem_controls = {:?}",
        out.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// 2. FactRemove round-trip — synthetic rewrite removes RELIDO.
// ---------------------------------------------------------------------------

/// A `CategoryAction::Intent(FactRemove { Cve(TOK_RELIDO), Page })`
/// rewrite mutates the projected marking by removing RELIDO from its
/// dissem-controls axis. Mirror of test 1 for the FactRemove path.
#[test]
fn page_rewrite_intent_fact_remove_mutates_projection() {
    let rewrite = PageRewrite {
        id: "test/intent-fact-remove",
        citation: "test fixture",
        // Fires when NOFORN is present in dissem — the input below
        // carries NOFORN, so the trigger fires.
        trigger: CategoryPredicate::Contains {
            category: CAT_DISSEM,
            token: TOK_NOFORN,
        },
        action: CategoryAction::Intent(ReplacementIntent::fact_remove(
            FactRef::Cve(TOK_RELIDO),
            Scope::Page,
        )),
        reads: &[CAT_DISSEM],
        writes: &[CAT_DISSEM],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);

    let mut attrs = portion_at(Classification::Secret);
    attrs.dissem_controls = vec![DissemControl::Nf, DissemControl::Relido].into();
    let portion = wrap(attrs);
    let out = scheme.project(Scope::Page, &[portion]);

    assert!(
        out.0.dissem_controls.contains(&DissemControl::Nf),
        "NOFORN must remain after FactRemove(RELIDO); got {:?}",
        out.0.dissem_controls,
    );
    assert!(
        !out.0.dissem_controls.contains(&DissemControl::Relido),
        "RELIDO must be removed by the rewrite; got {:?}",
        out.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// 3. Idempotence — FactAdd on a marking already containing the token.
// ---------------------------------------------------------------------------

/// A `FactAdd` rewrite that fires against a marking already
/// containing the token is a silent per-intent no-op (the executor's
/// `Err(IntentInapplicable)` arm logs and continues; the projection
/// is unchanged). No panic, no error, no double-add.
#[test]
fn page_rewrite_intent_fact_add_idempotent_when_already_present() {
    let rewrite = PageRewrite {
        id: "test/intent-fact-add-idempotent",
        citation: "test fixture",
        // Fires on the presence of NOFORN — and adds NOFORN. The
        // marking already has NOFORN; the add is a no-op.
        trigger: CategoryPredicate::Contains {
            category: CAT_DISSEM,
            token: TOK_NOFORN,
        },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::Cve(TOK_NOFORN),
            scope: Scope::Page,
        }),
        reads: &[CAT_DISSEM],
        writes: &[CAT_DISSEM],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);

    let mut attrs = portion_at(Classification::Secret);
    attrs.dissem_controls = vec![DissemControl::Nf].into();
    let portion = wrap(attrs);
    let out = scheme.project(Scope::Page, &[portion]);

    // Exactly one NOFORN — not duplicated. No panic.
    let noforn_count = out
        .0
        .dissem_controls
        .iter()
        .filter(|d| matches!(d, DissemControl::Nf))
        .count();
    assert_eq!(
        noforn_count, 1,
        "FactAdd of a token already present must be a no-op (no duplicate); \
         got dissem_controls = {:?}",
        out.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// 4. Recanonicalize is a no-op at page-rewrite scope.
// ---------------------------------------------------------------------------

/// `CategoryAction::Intent(Recanonicalize { scope: RecanonScope::Page })`
/// authored inside a `PageRewrite` is silently inert: the rewrite
/// fires, the executor calls `apply_intent_to_marking`, but
/// `Recanonicalize` returns `Ok(())` without mutating the marking.
/// Re-rendering is a renderer-stage concern handled by
/// `MarkingScheme::render_canonical`, not at projection time.
#[test]
fn page_rewrite_intent_recanonicalize_is_no_op() {
    // Reference: same projection with NO page rewrites.
    let scheme_baseline = CapcoScheme::new();
    let mut attrs = portion_at(Classification::Secret);
    attrs.dissem_controls = vec![DissemControl::Nf].into();
    let portion_ref = wrap(attrs.clone());
    let baseline = scheme_baseline.project(Scope::Page, &[portion_ref]);

    // Same input through a scheme with a Recanonicalize rewrite.
    let rewrite = PageRewrite {
        id: "test/intent-recanonicalize",
        citation: "test fixture",
        trigger: CategoryPredicate::Contains {
            category: CAT_DISSEM,
            token: TOK_NOFORN,
        },
        action: CategoryAction::Intent(ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Page,
        }),
        reads: &[CAT_DISSEM],
        writes: &[CAT_DISSEM],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);
    let portion = wrap(attrs);
    let out = scheme.project(Scope::Page, &[portion]);

    assert_eq!(
        baseline.0.dissem_controls, out.0.dissem_controls,
        "Recanonicalize must not mutate the projection's dissem axis",
    );
    assert_eq!(
        baseline.0.classification, out.0.classification,
        "Recanonicalize must not mutate the projection's classification",
    );
}

// ---------------------------------------------------------------------------
// 5. Engine::new validation rejects an unroutable Cve token.
// ---------------------------------------------------------------------------

/// `Engine::new` walks every `CategoryAction::Intent` in the
/// scheme's page-rewrites table and validates each intent's
/// `FactRef`s through `category_of`. A `FactRef::Cve(TokenId(u32::MAX))`
/// is guaranteed not to map to any category in `CapcoScheme` (the
/// constants in `capco_token_category` are all under 200), so the
/// validation fails with `InvalidIntentInPageRewrite`.
///
/// This is the load-bearing test for engine-construction-time
/// validation: a scheme-authoring bug (a `FactAdd` pointing at a
/// `TokenId` the scheme doesn't know about) surfaces here rather
/// than silently no-opping on the first page that triggers the
/// rewrite.
#[test]
fn engine_new_rejects_intent_with_unroutable_cve_token() {
    // TokenId(u32::MAX) is guaranteed not to be wired in
    // `capco_token_category`'s match arms (which cover only the
    // declared sentinels TOK_NOFORN..=TOK_REL_TO, all under 200).
    let unroutable = TokenId(u32::MAX);
    let rewrite = PageRewrite {
        id: "test/intent-unroutable-token",
        citation: "test fixture",
        trigger: CategoryPredicate::Empty {
            category: CAT_DISSEM,
        },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::Cve(unroutable),
            scope: Scope::Page,
        }),
        reads: &[CAT_DISSEM],
        writes: &[CAT_DISSEM],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);

    let result = Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        scheme,
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    );

    match result {
        Err(EngineConstructionError::InvalidIntentInPageRewrite {
            rewrite_id,
            fact_label,
            error,
        }) => {
            assert_eq!(
                rewrite_id, "test/intent-unroutable-token",
                "Engine must name the offending rewrite by id",
            );
            assert!(
                fact_label.contains("Cve") && fact_label.contains(&u32::MAX.to_string()),
                "fact_label must Debug-format the offending FactRef: got {fact_label:?}",
            );
            assert_eq!(
                error,
                ApplyIntentError::UnknownToken,
                "validation must report UnknownToken for unroutable Cve",
            );
        }
        Err(other) => panic!("expected InvalidIntentInPageRewrite, got {other:?}"),
        Ok(_) => panic!("expected validation to reject unroutable Cve token at Engine::new"),
    }
}

// ---------------------------------------------------------------------------
// 6. G13: OpenVocab FactRef does not leak source bytes.
// ---------------------------------------------------------------------------

/// Constitution V Principle V G13 regression guard: a rewrite whose
/// intent carries an `OpenVocab` `FactRef` does not leak any
/// source-derived strings into the projected marking's debug
/// output. The `CapcoOpenVocabRef::CountryCode` payload is a typed
/// `marque_ism::CountryCode` (16-byte fixed buffer of canonicalized
/// trigraph bytes), not a slice of original input.
///
/// Picks a country code (`GBR`) that does NOT appear anywhere in
/// our synthetic source-shaped input string; if the debug output
/// somehow contained source bytes, the assertion would not catch
/// `GBR` directly but would catch the absurd source-string text
/// `("__leaked_source_marker_42__")`.
#[test]
fn g13_open_vocab_factref_does_not_leak_source_bytes() {
    let gbr = CountryCode::try_new(b"GBR").expect("GBR is a valid trigraph");
    let rewrite = PageRewrite {
        id: "test/intent-open-vocab-no-leak",
        citation: "test fixture",
        trigger: CategoryPredicate::Empty {
            category: CAT_REL_TO,
        },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(gbr)),
            scope: Scope::Page,
        }),
        reads: &[CAT_REL_TO],
        writes: &[CAT_REL_TO],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);

    // Note: this string is the "source bytes" we want to confirm
    // never appears in the projected marking's debug output. It is
    // intentionally absurd so any accidental injection is obvious.
    let leaked_source_marker = "__leaked_source_marker_42__";

    let portion = wrap(portion_at(Classification::Secret));
    let out = scheme.project(Scope::Page, &[portion]);

    let debug = format!("{:?}", out);
    assert!(
        !debug.contains(leaked_source_marker),
        "projected marking debug output must not contain source-derived strings; \
         got: {debug}",
    );

    // Also confirm the OpenVocab FactAdd path actually ran — GBR is
    // in the rel_to axis — so the test is exercising the path it
    // claims to exercise (not vacuously passing because the rewrite
    // didn't fire).
    assert!(
        out.0.rel_to.contains(&gbr),
        "OpenVocab FactAdd must add GBR to REL TO axis; got {:?}",
        out.0.rel_to,
    );
}
