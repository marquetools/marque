// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based tests for `PageContext` roll-up monotonicity.
//!
//! Generates small vecs of `CanonicalAttrs` (1–5 portions), feeds them to
//! `PageContext::add_portion`, and asserts the structural invariants of the
//! roll-up: classification monotonicity, dissem-control union superset,
//! REL-TO intersection subset, and empty-page sentinel.

use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification, PageContext,
};
use proptest::prelude::*;
use proptest::sample::subsequence;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

fn arb_classification() -> impl Strategy<Value = Classification> {
    prop_oneof![
        Just(Classification::Unclassified),
        Just(Classification::Restricted),
        Just(Classification::Confidential),
        Just(Classification::Secret),
        Just(Classification::TopSecret),
    ]
}

fn arb_dissem_subset() -> impl Strategy<Value = Vec<DissemControl>> {
    // Use only controls that do not trigger complex PageContext filtering logic
    // (OC-USGOV is conditional on OC presence; FOUO drops in classified docs).
    // Testing the invariants with a stable subset keeps properties clean.
    let stable: Vec<DissemControl> = vec![
        DissemControl::Nf,
        DissemControl::Relido,
        DissemControl::Eyes,
        DissemControl::Pr,
        DissemControl::Fisa,
    ];
    let len = stable.len();
    subsequence(stable, 0..=len).prop_map(|v| v)
}

static VALID_COUNTRY_CODES: &[[u8; 3]] = &[
    *b"USA", *b"GBR", *b"CAN", *b"AUS", *b"NZL", *b"DEU", *b"FRA",
];

fn arb_rel_to() -> impl Strategy<Value = Vec<CountryCode>> {
    let all_codes: Vec<[u8; 3]> = VALID_COUNTRY_CODES.to_vec();
    let len = all_codes.len();
    prop_oneof![
        // Empty (no REL TO constraint)
        Just(vec![]),
        // USA only
        Just(vec![CountryCode::try_new(b"USA").unwrap()]),
        // USA + some partner nations
        subsequence(all_codes, 1..=len).prop_map(|subset| {
            // USA must be first; ensure it's present and de-duplicated.
            let mut codes: Vec<CountryCode> = std::iter::once(*b"USA")
                .chain(subset.into_iter().filter(|b| *b != *b"USA"))
                .map(|b| CountryCode::try_new(&b).unwrap())
                .collect();
            codes.dedup_by_key(|c| c.as_str().to_owned());
            codes
        }),
    ]
}

fn arb_ism_attrs() -> impl Strategy<Value = CanonicalAttrs> {
    // Per CAPCO-2016 §G.2 Table 5 (pp 40-45), pure-NATO portions
    // contribute to `dissem_nato` rather than `dissem_us`. Generate
    // both namespaces independently so the per-namespace union
    // properties below exercise both channels. The NATO subset is
    // gated to a small probability (1/4 weight) because pure-NATO
    // portions are rare in practice and we want classification +
    // dissem_us paths to remain the dominant fixture shape.
    (
        prop_oneof![
            Just(None),
            arb_classification().prop_map(|c| Some(MarkingClassification::Us(c))),
        ],
        arb_dissem_subset(),
        prop_oneof![
            3 => Just(Vec::<DissemControl>::new()),
            1 => arb_dissem_subset(),
        ],
        arb_rel_to(),
    )
        .prop_map(
            |(classification, dissem_us_subset, dissem_nato_subset, rel_to)| {
                // CanonicalAttrs is #[non_exhaustive] so use Default + field mutation.
                let mut attrs = CanonicalAttrs::default();
                attrs.classification = classification;
                attrs.dissem_us = dissem_us_subset.into_boxed_slice();
                attrs.dissem_nato = dissem_nato_subset.into_boxed_slice();
                attrs.rel_to = rel_to.into_boxed_slice();
                attrs
            },
        )
}

fn arb_portions() -> impl Strategy<Value = Vec<CanonicalAttrs>> {
    proptest::collection::vec(arb_ism_attrs(), 1..=5)
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

proptest! {
    // expected_classification() must equal the exact max over portions.
    #[test]
    fn classification_monotone(portions in arb_portions()) {
        let mut ctx = PageContext::new();
        for p in &portions {
            ctx.add_portion(p.clone());
        }

        let rolled = ctx.expected_classification();
        let portion_max = portions
            .iter()
            .filter_map(|a| a.us_classification())
            .max();

        prop_assert_eq!(
            rolled,
            portion_max,
            "expected_classification roll-up does not equal portion max for portions: {:?}",
            portions,
        );
    }

    // Every dissem token in any portion's `dissem_us` must appear in
    // the rolled-up `expected_dissem_us()`. Pins the US-namespace
    // union direction post PR 9b / FR-046 split — the prior
    // `dissem_controls_union_superset` name referred to the retired
    // unified field.
    //
    // Exception: RELIDO is evicted by `expected_dissem_us` Step 5
    // when the page has FD&R intent (some portion has REL TO or
    // DISPLAY ONLY) but both rolled-up foreign-audience axes come
    // back empty — Step 5 injects NF and removes RELIDO so the
    // banner doesn't render the §H.8 p154 / §D.2 row 2 conflict.
    // The proptest reproduces that condition exactly so it doesn't
    // misclassify a pre-existing portion-level NF+RELIDO conflict
    // (which E054 catches at the rule layer) as a Step-5 eviction.
    #[test]
    fn dissem_us_union_superset(portions in arb_portions()) {
        let mut ctx = PageContext::new();
        for p in &portions {
            ctx.add_portion(p.clone());
        }
        let rolled: std::collections::BTreeSet<DissemControl> =
            ctx.expected_dissem_us().into_iter().collect();

        // Replicate Step 5's eviction predicate so we know when the
        // union-superset invariant is intentionally relaxed.
        let has_fdr_intent = portions
            .iter()
            .any(|a| !a.rel_to.is_empty() || !a.display_only_to.is_empty());
        let step5_fires = has_fdr_intent
            && ctx.expected_rel_to().is_empty()
            && ctx.expected_display_only().is_empty();

        for portion in &portions {
            for ctrl in portion.dissem_us.iter() {
                if *ctrl == DissemControl::Relido && step5_fires {
                    // Eviction is the intended behavior; skip the
                    // superset check for this token.
                    continue;
                }
                prop_assert!(
                    rolled.contains(ctrl),
                    "dissem_us control {ctrl:?} in portion but missing from US roll-up",
                );
            }
        }
    }

    // Every dissem token in any portion's `dissem_nato` must appear
    // in the rolled-up `expected_dissem_nato()`. Companion to the
    // dissem_us property above — exercises the parallel NATO channel
    // wired by PR 9b T132 / FR-046 (CAPCO-2016 §G.2 Table 5 (pp 40-45):
    // pure-NATO portions contribute here, not to dissem_us).
    #[test]
    fn dissem_nato_union_superset(portions in arb_portions()) {
        let mut ctx = PageContext::new();
        for p in &portions {
            ctx.add_portion(p.clone());
        }
        let rolled: std::collections::BTreeSet<DissemControl> =
            ctx.expected_dissem_nato().into_iter().collect();

        for portion in &portions {
            for ctrl in portion.dissem_nato.iter() {
                prop_assert!(
                    rolled.contains(ctrl),
                    "dissem_nato control {ctrl:?} in portion but missing from NATO roll-up",
                );
            }
        }
    }

    // If a country code appears in expected_rel_to(), it must appear in every
    // portion that carries a non-empty REL TO list (intersection property).
    #[test]
    fn rel_to_intersection_property(portions in arb_portions()) {
        let mut ctx = PageContext::new();
        for p in &portions {
            ctx.add_portion(p.clone());
        }
        let rolled_set: std::collections::BTreeSet<String> = ctx
            .expected_rel_to()
            .into_iter()
            .map(|t| t.as_str().to_owned())
            .collect();

        let rel_to_portions: Vec<_> = portions
            .iter()
            .filter(|a| !a.rel_to.is_empty())
            .collect();

        if rel_to_portions.is_empty() {
            return Ok(());
        }

        for t_str in &rolled_set {
            for portion in &rel_to_portions {
                let portion_strs: std::collections::BTreeSet<String> =
                    portion.rel_to.iter().map(|t| t.as_str().to_owned()).collect();
                prop_assert!(
                    portion_strs.contains(t_str),
                    "country code {:?} in roll-up but missing from portion {:?}",
                    t_str,
                    portion.rel_to,
                );
            }
        }
    }
}

// Empty page sentinel: not a proptest, just a deterministic guard.
#[test]
fn empty_page_context_returns_none_classification() {
    let ctx = PageContext::new();
    assert_eq!(ctx.expected_classification(), None);
}
