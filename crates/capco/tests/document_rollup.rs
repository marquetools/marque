// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase C1 (T030) — page→document lattice-fold algebra over CAPCO axes.
//!
//! These tests pin the correctness property that `canonical_document_join`
//! is `canonical_page_join` one scope up (pages, not portions), so the
//! page→document fold routes through `CapcoMarking::join_via_lattice` and
//! preserves the observational state the per-axis lattices encode:
//! DissemSet RELIDO-unanimity, NOFORN supersession, JointSet disunity
//! collapse, and classification OrdMax.
//!
//! The construction path mirrors `b3_3a_trait_lift.rs`: a *page* rollup is
//! itself `canonical_page_join` over that page's portions; the *document*
//! rollup is `canonical_document_join` over the page rollups. The file
//! lives in `crates/capco/tests/` (not `marque-scheme`) because the real
//! lattice axes live in `marque-capco`, and `marque-scheme` cannot
//! dev-depend on `marque-capco` without inverting the dependency graph
//! (Constitution VII).
//!
//! Citation note (Constitution VIII): the synthesis plan's table cited
//! `§G.1 Table 4 p37` for the max-classification test. That is a
//! misattribution — Table 4 (p36) is the *Register of Authorized
//! Classification and Control Markings* (the marking-ordering register),
//! not the banner roll-up rule. The authority for "the banner takes the
//! highest classification level of all portions" is §D.2 p28 ("Banner
//! Line 'Roll-Up' Rules": *"Taking the highest classification level of
//! all the portions and using that as the banner line classification
//! marking"*), which is also what the in-tree `ClassificationLattice`
//! cites for the roll-up rule (`classification.rs`). §H.1 pp47-54
//! governs only how each level's banner is *formed*, not the roll-up
//! aggregation. Corrected below; verified against CAPCO-2016.md.

use marque_capco::scheme::CapcoScheme;
use marque_ism::CanonicalAttrs;
use marque_ism::attrs::*;
use marque_scheme::MarkingScheme as _;

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

/// A US-classified portion at `level` with the given `dissem_us` controls.
fn us_portion(level: Classification, dissem_us: &[DissemControl]) -> CanonicalAttrs {
    // CanonicalAttrs is #[non_exhaustive]; build via Default + field set.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(level));
    a.dissem_us = dissem_us.to_vec().into_boxed_slice();
    a
}

/// Fold a page's portions into a single page-rollup `CanonicalAttrs` — the
/// same operation the engine applies per page.
fn page_rollup(scheme: &CapcoScheme, portions: &[CanonicalAttrs]) -> CanonicalAttrs {
    scheme.canonical_page_join(portions)
}

/// Fold per-page rollups into the document rollup — the C1 deliverable.
fn document_rollup(scheme: &CapcoScheme, pages: &[CanonicalAttrs]) -> CanonicalAttrs {
    scheme.canonical_document_join(pages)
}

fn effective_level(a: &CanonicalAttrs) -> Option<Classification> {
    a.classification.as_ref().map(|c| c.effective_level())
}

fn has_dissem(a: &CanonicalAttrs, ctrl: DissemControl) -> bool {
    a.dissem_us.contains(&ctrl)
}

// ---------------------------------------------------------------------------
// Algebra tests
// ---------------------------------------------------------------------------

/// The document banner takes the highest classification level of all
/// portions across all pages (OrdMax over the level ladder).
///
/// Authority: CAPCO-2016 §D.2 p28 ("Banner Line 'Roll-Up' Rules":
/// "Taking the highest classification level of all the portions and using
/// that as the banner line classification marking"). Verified against
/// `crates/capco/docs/CAPCO-2016.md`.
#[test]
fn document_rollup_max_classification_across_pages() {
    let scheme = CapcoScheme::new();
    let page_secret = page_rollup(&scheme, &[us_portion(Classification::Secret, &[])]);
    let page_top_secret = page_rollup(&scheme, &[us_portion(Classification::TopSecret, &[])]);
    let page_confidential = page_rollup(&scheme, &[us_portion(Classification::Confidential, &[])]);

    let doc = document_rollup(&scheme, &[page_secret, page_top_secret, page_confidential]);
    assert_eq!(
        effective_level(&doc),
        Some(Classification::TopSecret),
        "document rollup must equal the max classification across pages",
    );
}

/// RELIDO survives to the document banner only when EVERY portion across
/// every page carries RELIDO (observed-unanimity). All-pages-unanimous
/// keeps it.
///
/// Authority: CAPCO-2016 §H.8 pp155-156 (RELIDO appears on the banner only
/// when every portion carries RELIDO). Verified against CAPCO-2016.md.
#[test]
fn relido_unanimity_survives_all_pages_relido() {
    let scheme = CapcoScheme::new();
    // Every portion on every page carries RELIDO.
    let page_a = page_rollup(
        &scheme,
        &[
            us_portion(Classification::Secret, &[DissemControl::Relido]),
            us_portion(Classification::Confidential, &[DissemControl::Relido]),
        ],
    );
    let page_b = page_rollup(
        &scheme,
        &[us_portion(Classification::Secret, &[DissemControl::Relido])],
    );
    assert!(
        has_dissem(&page_a, DissemControl::Relido) && has_dissem(&page_b, DissemControl::Relido),
        "precondition: each unanimous page rollup keeps RELIDO",
    );

    let doc = document_rollup(&scheme, &[page_a, page_b]);
    assert!(
        has_dissem(&doc, DissemControl::Relido),
        "all-pages-unanimous RELIDO must survive to the document banner; \
         dissem_us = {:?}",
        doc.dissem_us,
    );
}

/// RELIDO is dropped at the document banner when at least one page is not
/// RELIDO-unanimous. A naive token re-union would wrongly keep RELIDO —
/// this is the LV3 guardrail.
///
/// Authority: CAPCO-2016 §H.8 pp155-156. Verified against CAPCO-2016.md.
#[test]
fn relido_dropped_when_one_page_not_unanimous() {
    let scheme = CapcoScheme::new();
    // Page A is unanimous RELIDO.
    let page_a = page_rollup(
        &scheme,
        &[us_portion(Classification::Secret, &[DissemControl::Relido])],
    );
    // Page B has one RELIDO portion and one bare portion → not unanimous,
    // so the page rollup already drops RELIDO.
    let page_b = page_rollup(
        &scheme,
        &[
            us_portion(Classification::Secret, &[DissemControl::Relido]),
            us_portion(Classification::Confidential, &[]),
        ],
    );
    assert!(
        !has_dissem(&page_b, DissemControl::Relido),
        "precondition: non-unanimous page rollup drops RELIDO",
    );

    let doc = document_rollup(&scheme, &[page_a, page_b]);
    assert!(
        !has_dissem(&doc, DissemControl::Relido),
        "RELIDO must drop at the document banner when one page is not \
         unanimous; dissem_us = {:?}",
        doc.dissem_us,
    );
}

/// NOFORN supersession survives the page→document fold: once any page
/// contributes NOFORN, the dominated FD&R controls (REL TO / RELIDO /
/// EYES / DISPLAY ONLY) are stripped at the document banner. A naive
/// re-union would re-admit the dominated tokens from non-NOFORN pages.
///
/// Authority: CAPCO-2016 §H.8 p145 (NOFORN cannot be used with REL TO,
/// RELIDO, EYES ONLY, DISPLAY ONLY) + §D.2 Table 3 rows 1-2 (NOFORN
/// dominates FD&R controls). Verified against CAPCO-2016.md.
#[test]
fn noforn_supersession_survives_page_to_doc_fold() {
    let scheme = CapcoScheme::new();
    // Page A carries NOFORN.
    let page_noforn = page_rollup(
        &scheme,
        &[us_portion(Classification::Secret, &[DissemControl::Nf])],
    );
    // Page B carries RELIDO + EYES + DISPLAY ONLY (all dominated FD&R
    // controls named in the §H.8 p145 list), so the assertion covers
    // every token the citation enumerates.
    let page_fdr = page_rollup(
        &scheme,
        &[us_portion(
            Classification::Secret,
            &[
                DissemControl::Relido,
                DissemControl::Eyes,
                DissemControl::Displayonly,
            ],
        )],
    );

    let doc = document_rollup(&scheme, &[page_noforn, page_fdr]);
    assert!(
        has_dissem(&doc, DissemControl::Nf),
        "NOFORN must reach the document banner; dissem_us = {:?}",
        doc.dissem_us,
    );
    assert!(
        !has_dissem(&doc, DissemControl::Relido)
            && !has_dissem(&doc, DissemControl::Eyes)
            && !has_dissem(&doc, DissemControl::Displayonly)
            && !has_dissem(&doc, DissemControl::Rel),
        "NOFORN must strip dominated FD&R controls at the document banner; \
         dissem_us = {:?}",
        doc.dissem_us,
    );
}

/// The fold is order-independent: permuting a page slice yields a
/// byte-identical document rollup. This is the join semilattice law
/// (commutativity + associativity + idempotence) the C1 fold relies on.
///
/// Authority: join-semilattice algebra (research D12 / LV3). The CAPCO
/// per-axis lattices are each commutative + associative semilattices, so
/// the cross-axis fold is permutation-invariant.
#[test]
fn fold_is_order_independent() {
    let scheme = CapcoScheme::new();
    let page_relido = page_rollup(
        &scheme,
        &[us_portion(Classification::Secret, &[DissemControl::Relido])],
    );
    let page_noforn = page_rollup(
        &scheme,
        &[us_portion(Classification::TopSecret, &[DissemControl::Nf])],
    );
    let page_eyes = page_rollup(
        &scheme,
        &[us_portion(
            Classification::Confidential,
            &[DissemControl::Eyes],
        )],
    );

    let pages = [page_relido, page_noforn, page_eyes];
    let baseline = document_rollup(&scheme, &pages);

    // All six permutations of three pages must agree with the baseline.
    let perms = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for perm in perms {
        let permuted: Vec<CanonicalAttrs> = perm.iter().map(|&i| pages[i].clone()).collect();
        let rolled = document_rollup(&scheme, &permuted);
        assert_eq!(
            rolled, baseline,
            "document fold must be order-independent; permutation {perm:?} diverged",
        );
    }
}

/// An empty document (no pages) folds to the lattice bottom
/// (`CanonicalAttrs::default()`) — the join identity.
///
/// Authority: join identity law (`join` over the empty set is bottom).
#[test]
fn empty_document_is_lattice_bottom() {
    let scheme = CapcoScheme::new();
    let doc = document_rollup(&scheme, &[]);
    assert_eq!(
        doc,
        CanonicalAttrs::default(),
        "an empty document must fold to the canonical bottom",
    );
}

/// A single-page document folds to exactly that page (join identity
/// `join(x) = x`). The second case starts from a `Conflict` portion: the
/// *page* rollup trips the `join_via_lattice` body path (the single-portion
/// fast-path bails on `Conflict`, normalizing it to `Us(..)`), and the
/// *document* fold of that already-normalized page rollup must still be
/// identity. (The document fold itself receives the normalized value, not
/// a `Conflict` — the body-path coverage is on the page rollup.)
///
/// Authority: join identity law.
#[test]
fn single_page_fold_is_identity() {
    let scheme = CapcoScheme::new();

    // Plain single page → identity.
    let page = page_rollup(
        &scheme,
        &[us_portion(Classification::Secret, &[DissemControl::Nf])],
    );
    let doc = document_rollup(&scheme, std::slice::from_ref(&page));
    assert_eq!(
        doc, page,
        "a single-page document must equal that page (join identity)",
    );

    // Body-path case: a Conflict classification makes the PAGE rollup take
    // the join_via_lattice body path (the single-portion fast-path bails on
    // Conflict, normalizing it to Us(..)). The document fold then receives
    // the already-normalized page rollup; identity must hold on it.
    let mut conflict = CanonicalAttrs::default();
    conflict.classification = Some(MarkingClassification::Conflict {
        us: Classification::Secret,
        foreign: Box::new(ForeignClassification::Nato(
            NatoClassification::NatoConfidential,
        )),
    });
    let conflict_page = page_rollup(&scheme, std::slice::from_ref(&conflict));
    let conflict_doc = document_rollup(&scheme, std::slice::from_ref(&conflict_page));
    assert_eq!(
        conflict_doc, conflict_page,
        "single-page identity must hold on the normalized post-Conflict page rollup",
    );
}

/// A document mixing a JOINT page (UnanimousProducers) with a non-JOINT
/// page collapses the JOINT axis to `Mixed`: the document banner is
/// flattened to `Us(level)` and does NOT carry the JOINT producer shape.
///
/// Authority: CAPCO-2016 §H.3 p57 ("a JOINT marking cannot be rolled up
/// to the banner line in US documents" — once JOINT and non-JOINT both
/// appear, the JOINT shape collapses). Verified against CAPCO-2016.md.
#[test]
fn joint_mixed_absorbing_across_pages() {
    let scheme = CapcoScheme::new();
    let usa = CountryCode::try_new(b"USA").expect("USA trigraph");
    let gbr = CountryCode::try_new(b"GBR").expect("GBR trigraph");

    // Page A: a JOINT portion → UnanimousProducers → Joint(_) classification.
    let mut joint_portion = CanonicalAttrs::default();
    joint_portion.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: Box::new([usa, gbr]),
    }));
    let page_joint = page_rollup(&scheme, std::slice::from_ref(&joint_portion));
    assert!(
        matches!(
            page_joint.classification,
            Some(MarkingClassification::Joint(_))
        ),
        "precondition: a unanimous-JOINT page rolls up to a Joint(_) banner",
    );

    // Page B: a plain US portion (non-JOINT).
    let page_us = page_rollup(&scheme, &[us_portion(Classification::Secret, &[])]);

    // Document fold of JOINT + non-JOINT → Mixed → flattened to Us(_).
    let doc = document_rollup(&scheme, &[page_joint, page_us]);
    assert!(
        !matches!(doc.classification, Some(MarkingClassification::Joint(_))),
        "JOINT + non-JOINT across pages must collapse to Mixed (no JOINT \
         banner shape); classification = {:?}",
        doc.classification,
    );
    assert_eq!(
        effective_level(&doc),
        Some(Classification::Secret),
        "the collapsed banner must still carry the max effective level",
    );
}
