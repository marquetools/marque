// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based tests for `PageContext` roll-up monotonicity.
//!
//! Generates small vecs of `IsmAttributes` (1–5 portions), feeds them to
//! `PageContext::add_portion`, and asserts the structural invariants of the
//! roll-up: classification monotonicity, dissem-control union superset,
//! REL-TO intersection subset, and empty-page sentinel.

use marque_ism::{
    Classification, DissemControl, IsmAttributes, MarkingClassification, PageContext, Trigraph,
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

static VALID_TRIGRAPHS: &[[u8; 3]] = &[
    *b"USA", *b"GBR", *b"CAN", *b"AUS", *b"NZL", *b"DEU", *b"FRA",
];

fn arb_rel_to() -> impl Strategy<Value = Vec<Trigraph>> {
    let all_trigraphs: Vec<[u8; 3]> = VALID_TRIGRAPHS.to_vec();
    let len = all_trigraphs.len();
    prop_oneof![
        // Empty (no REL TO constraint)
        Just(vec![]),
        // USA only
        Just(vec![Trigraph::try_new(*b"USA").unwrap()]),
        // USA + some partner nations
        subsequence(all_trigraphs, 1..=len).prop_map(|subset| {
            // USA must be first; ensure it's present and de-duplicated.
            let mut trigraphs: Vec<Trigraph> = std::iter::once(*b"USA")
                .chain(subset.into_iter().filter(|b| *b != *b"USA"))
                .map(|b| Trigraph::try_new(b).unwrap())
                .collect();
            trigraphs.dedup_by_key(|t| t.as_str().to_owned());
            trigraphs
        }),
    ]
}

fn arb_ism_attrs() -> impl Strategy<Value = IsmAttributes> {
    (
        prop_oneof![
            Just(None),
            arb_classification().prop_map(|c| Some(MarkingClassification::Us(c))),
        ],
        arb_dissem_subset(),
        arb_rel_to(),
    )
        .prop_map(|(classification, dissem_controls, rel_to)| {
            // IsmAttributes is #[non_exhaustive] so use Default + field mutation.
            let mut attrs = IsmAttributes::default();
            attrs.classification = classification;
            attrs.dissem_controls = dissem_controls.into_boxed_slice();
            attrs.rel_to = rel_to.into_boxed_slice();
            attrs
        })
}

fn arb_portions() -> impl Strategy<Value = Vec<IsmAttributes>> {
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

    // Every DissemControl on any portion must appear in expected_dissem_controls().
    #[test]
    fn dissem_controls_union_superset(portions in arb_portions()) {
        let mut ctx = PageContext::new();
        for p in &portions {
            ctx.add_portion(p.clone());
        }
        let rolled: std::collections::BTreeSet<DissemControl> =
            ctx.expected_dissem_controls().into_iter().collect();

        for portion in &portions {
            for ctrl in portion.dissem_controls.iter() {
                prop_assert!(
                    rolled.contains(&ctrl),
                    "dissem control {ctrl:?} in portion but missing from roll-up",
                );
            }
        }
    }

    // If a trigraph appears in expected_rel_to(), it must appear in every
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
                    "trigraph {:?} in roll-up but missing from portion {:?}",
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
