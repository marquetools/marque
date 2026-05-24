// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based tests for page roll-up monotonicity.
//!
//! Generates small vecs of `CanonicalAttrs` (1–5 portions) and asserts the
//! structural invariants of the per-axis lattice roll-up: classification
//! monotonicity, dissem-control union superset, REL-TO intersection subset,
//! and empty-page sentinel.
//!
//! These proptests exercise the page-rollup invariants via the
//! lattice-native helpers in `marque-capco::lattice`:
//! `ClassificationLattice::from_attrs_iter`, `DissemSet::from_attrs_iter`,
//! `NatoDissemSet::from_attrs_iter`, `RelToBlock::from_attrs_iter`. The
//! file lives in `crates/capco/tests/` because those lattice helpers
//! live in `marque-capco` (and `marque-ism` cannot dev-depend on
//! `marque-capco` without creating a dev-cycle).

use marque_capco::lattice::{ClassificationLattice, DissemSet, NatoDissemSet, RelToBlock};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, DissemControl, FgiClassification,
    JointClassification, MarkingClassification, NatoClassification,
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

/// Strategy producing any of the four `MarkingClassification` variants
/// (Us / Fgi / Nato / Joint).
///
/// PR 5 (006 T063a + #276): Pre-PR-5 `arb_ism_attrs` generated only
/// `Some(MarkingClassification::Us(c))`, so foreign-banner roll-up
/// regressions were unreachable from proptest. Adding the foreign
/// variants closes that coverage gap.
///
/// Approximate weighting:
/// - 60% `Us(_)` — preserves the dominant fixture shape.
/// - 15% `Fgi(_)` — exercises §H.7 reciprocal-classification and the
///   FgiSet cross-axis fold.
/// - 10% `Nato(_)` — exercises §G.2 reciprocity + the §H.7 pp123-125
///   solely-NATO preservation path.
/// - 15% `Joint(_)` — exercises §H.3 p56 JOINT producer-list grammar.
///
/// Each foreign variant uses a small deterministic country-list
/// shape so the strategy's failure cases are reproducible. Tetragraph
/// admission (FVEY / ACGU / TEYE) and arbitrary trigraph fuzz are
/// out of scope for the basic page-rollup invariants this file
/// exercises.
fn arb_marking_classification_any_variant() -> impl Strategy<Value = MarkingClassification> {
    let usa = CountryCode::try_new(b"USA").expect("USA trigraph");
    let gbr = CountryCode::try_new(b"GBR").expect("GBR trigraph");
    let deu = CountryCode::try_new(b"DEU").expect("DEU trigraph");
    prop_oneof![
        // 60% — Us(_): dominant case.
        60 => arb_classification().prop_map(MarkingClassification::Us),
        // 15% — Fgi(_): source-acknowledged FGI with a single
        // trigraph. Source-concealed (empty country list) is a
        // separate path covered by the foreign corpus fixtures.
        15 => arb_classification().prop_map(move |level| {
            MarkingClassification::Fgi(FgiClassification {
                countries: Box::new([deu]),
                level,
            })
        }),
        // 10% — Nato(_): NATO classification ladder.
        10 => prop_oneof![
            Just(NatoClassification::NatoUnclassified),
            Just(NatoClassification::NatoRestricted),
            Just(NatoClassification::NatoConfidential),
            Just(NatoClassification::NatoSecret),
            Just(NatoClassification::CosmicTopSecret),
        ].prop_map(MarkingClassification::Nato),
        // 15% — Joint(_): JOINT US + GBR co-ownership. USA-first
        // alphabetical per §H.3 p56.
        15 => arb_classification().prop_map(move |level| {
            MarkingClassification::Joint(JointClassification {
                level,
                countries: Box::new([usa, gbr]),
            })
        }),
    ]
}

fn arb_dissem_subset() -> impl Strategy<Value = Vec<DissemControl>> {
    // Use only controls that do not trigger complex page-rollup filtering
    // logic (OC-USGOV is conditional on OC presence; FOUO drops in
    // classified docs). Testing the invariants with a stable subset
    // keeps properties clean.
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
            // PR 5 (006 T063a + #276): generate Us/Fgi/Nato/Joint
            // variants so foreign-banner roll-up regressions are
            // reachable from proptest. The dominant US case is
            // weighted at 60% inside the strategy; foreign variants
            // sum to 40%.
            arb_marking_classification_any_variant().prop_map(Some),
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
    // ClassificationLattice::from_attrs_iter must produce the exact max
    // over portions on the effective-level ladder. Pre-PR-5 the
    // assertion read `us_classification()` (returns None for
    // FGI/NATO/JOINT), which was equivalent for the prior US-only
    // proptest strategy. Post-PR-5 the strategy generates foreign
    // variants too, so the property compares against the cross-system
    // `effective_level()` max — which is what `OrdMax` over the
    // classification ladder actually computes.
    //
    // The §H.7 pp123-125 reciprocal-raise normalization happens at
    // `CapcoMarking::join_via_lattice_body`, NOT at the per-axis
    // `ClassificationLattice`. The lattice composes variants
    // structurally (variant-rank tiebreaker for equal effective
    // levels); the reciprocal flatten to `Us(_)` is a
    // `CapcoMarking`-level projection that runs after the per-axis
    // joins. So the per-axis lattice's effective-level max is the
    // load-bearing property here.
    #[test]
    fn classification_monotone(portions in arb_portions()) {
        let rolled = ClassificationLattice::from_attrs_iter(&portions)
            .into_inner()
            .map(|c| c.effective_level());
        let portion_max = portions
            .iter()
            .filter_map(|a| a.classification.as_ref().map(|c| c.effective_level()))
            .max();

        prop_assert_eq!(
            rolled,
            portion_max,
            "classification roll-up does not equal portion effective-level max for portions: {:?}",
            portions,
        );
    }

    // Every dissem token in any portion's `dissem_us` must appear in
    // the rolled-up `DissemSet`. Pins the US-namespace union direction.
    //
    // Two exception classes are NOT covered by this pure-union claim:
    //
    // 1. **Supersession-overlay-managed tokens** — `OcUsgov`, `Relido`,
    //    and `Fouo` are excluded because their
    //    banner presence is governed by §H.8 supersession rules, not
    //    by union:
    //    - `OcUsgov` per §H.8 p136 / p140: ORCON ⊐ ORCON-USGOV;
    //      USGOV drops when ORCON is present anywhere on the page.
    //    - `Relido` per §H.8 pp155-156: RELIDO appears on the banner
    //      only when every portion carries RELIDO (Layer 1
    //      observed-unanimity).
    //    - `Fouo` per §H.8 p134: drops in classified documents and
    //      when DSEN is present.
    //    Per-overlay behavior is pinned by dedicated tests in
    //    the per-type `#[cfg(test)] mod tests` blocks under
    //    `crates/capco/src/lattice/` and the parity gate
    //    at `crates/capco/tests/page_context_lattice_parity.rs`.
    //
    // 2. **FD&R-family eviction under NOFORN dominance** — the
    //    FD&R-family tokens (REL, RELIDO, EYES, DISPLAY ONLY marker)
    //    are evicted by the §H.8 p145 NOFORN-dominates overlay
    //    whenever NF reaches the rolled-up banner. Per §D.2 Table 3
    //    rows 1+2 and §H.8 p154 / p157, NOFORN supersedes every
    //    other FD&R-class marking at banner scope.
    //
    // The remaining DissemControl values pass through by plain union.
    #[test]
    fn dissem_us_union_superset(portions in arb_portions()) {
        let rolled: std::collections::BTreeSet<DissemControl> =
            DissemSet::from_attrs_iter(&portions)
                .into_boxed_slice()
                .iter()
                .copied()
                .collect();
        let banner_has_noforn = rolled.contains(&DissemControl::Nf);

        let fdr_family = [
            DissemControl::Rel,
            DissemControl::Relido,
            DissemControl::Eyes,
            DissemControl::Displayonly,
        ];

        for portion in &portions {
            for ctrl in portion.dissem_us.iter() {
                // Exception class 1 — supersession-overlay-managed
                // tokens (§H.8 p136/p140, pp155-156, p134).
                if matches!(
                    ctrl,
                    DissemControl::OcUsgov
                        | DissemControl::Relido
                        | DissemControl::Fouo
                ) {
                    continue;
                }
                // Exception class 2 — FD&R-family eviction when
                // NOFORN dominates at banner scope (§D.2 Table 3
                // rows 1+2, §H.8 p145 + p154 + p157).
                if banner_has_noforn && fdr_family.contains(ctrl) {
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
    // in the rolled-up `NatoDissemSet`. Companion to the dissem_us
    // property above — exercises the parallel NATO channel (CAPCO-2016
    // §G.2 Table 5 (pp 40-45): pure-NATO portions contribute here).
    #[test]
    fn dissem_nato_union_superset(portions in arb_portions()) {
        let rolled: std::collections::BTreeSet<DissemControl> =
            NatoDissemSet::from_attrs_iter(&portions)
                .into_boxed_slice()
                .iter()
                .copied()
                .collect();

        for portion in &portions {
            for ctrl in portion.dissem_nato.iter() {
                prop_assert!(
                    rolled.contains(ctrl),
                    "dissem_nato control {ctrl:?} in portion but missing from NATO roll-up",
                );
            }
        }
    }

    // If a country code appears in the rolled-up REL TO, it must
    // appear in every portion that carries a non-empty REL TO list
    // (intersection property).
    #[test]
    fn rel_to_intersection_property(portions in arb_portions()) {
        let rolled_set: std::collections::BTreeSet<String> = RelToBlock::from_attrs_iter(&portions)
            .into_boxed_slice()
            .iter()
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
fn empty_page_rollup_returns_none_classification() {
    let rolled = ClassificationLattice::from_attrs_iter(&[]).into_inner();
    assert!(rolled.is_none());
}
