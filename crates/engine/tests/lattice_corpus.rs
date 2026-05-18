// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-D.2 Pattern-D corpus fixtures.
//!
//! Companion to [`closure_hotpath.rs`]. Where `closure_hotpath.rs`
//! pins the seven `CLOSURE_NOFORN_*` rows + idempotence/monotonicity
//! laws + the `apply_fact_add` NOFORN-supersession routing, this
//! file covers three additional Pattern-D scenarios called out in
//! the PR 4b-D.2 spec:
//!
//! - `closure_nato_rel_to_solely_nato`: closure injects USA/NATO
//!   into REL TO silently (Severity::Info per D20) on a solely-NATO
//!   page. S007 (the text-layer Severity::Suggest sibling) is
//!   suppressed because there is no US-document context to emit a
//!   text-layer suggestion against. Asserts via `scheme.project()`.
//! - `closure_nato_rel_to_us_plus_nato`: US doc with a NATO portion.
//!   The §H.7 pp123-125 reciprocal-raise flattens the NATO
//!   classification to `Us(_)` at join time, so `TOK_NATO_CLASS` is
//!   absent in the joined marking by the time the closure operator
//!   sees it. Closure therefore does NOT inject USA/NATO into REL TO
//!   on this path — that's the load-bearing assertion the test
//!   makes (closure is suppressed by the reciprocal-raise). The
//!   NATO source provenance survives on the FGI axis (`fgi_marker`);
//!   S007 owns the text-layer Severity::Suggest behavior for this
//!   scenario, exercised end-to-end in
//!   `crates/capco/tests/dissem_nato_*.rs`.
//!
//!   Copilot R1 review #6 fixed this doc — pre-fix the summary
//!   claimed "Closure injects USA/NATO into REL TO" which is the
//!   opposite of what the test actually verifies.
//! - `closure_relido_unanimity`: confirms the PR 4b-B
//!   RELIDO-observed-unanimity overlay survives the PR 4b-D.2 hot-path
//!   flip. When every portion on a page carries RELIDO, the projected
//!   marking keeps RELIDO; when some portion doesn't, RELIDO is
//!   dropped at banner roll-up.
//!
//! Authority (re-verified 2026-05-17 against
//! `crates/capco/docs/CAPCO-2016.md`):
//! - §H.7 p127 + §G.2 Table 5 p40 — NATO REL TO closure cone.
//! - §H.8 pp155-156 — RELIDO observed-unanimity at banner roll-up.
//! - §H.7 pp123-125 — solely-NATO classification preservation (the
//!   `pure_nato_lattice_vs_pagecontext_diverges` divergence row
//!   established in PR 4b-B Commit 8).
//!
//! ### Why not JSON-file fixtures
//!
//! PR 4b-D.2's spec originally asked for `input.json` + `expected.ndjson`
//! file siblings. `CanonicalAttrs` does not derive `serde::Serialize`
//! (Constitution VII would require an out-of-scope cross-crate
//! type-system change to add it); building a JSON-driven fixture format
//! would require either inventing a Marque-source-to-attrs encoder or
//! sidestepping the type system. Inline-typed Rust tests with §-cited
//! comments capture the same Pattern-D coverage with stronger
//! type-checking and zero risk of fixture-file format drift.
//! The four NOFORN-implication scenarios from the spec
//! (`closure-noforn-implies-sar/fgi/orcon/aea-rd`) live in
//! `closure_hotpath.rs::closure_noforn_*` — adding equivalent
//! lattice-corpus tests here would duplicate that coverage.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification,
    NatoClassification,
};
use marque_scheme::{MarkingScheme, Scope};

fn classified_us(level: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(level));
    a
}

fn project_page(portions: &[CanonicalAttrs]) -> CapcoMarking {
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> = portions.iter().cloned().map(CapcoMarking::new).collect();
    scheme.project(Scope::Page, &markings)
}

fn rel_to_contains(m: &CapcoMarking, target: CountryCode) -> bool {
    m.0.rel_to.iter().any(|c| c == &target)
}

fn dissem_contains(m: &CapcoMarking, target: DissemControl) -> bool {
    m.0.dissem_us.iter().any(|d| d == &target)
}

// ---------------------------------------------------------------------------
// Pattern D fixture 5: closure_nato_rel_to_solely_nato
// ---------------------------------------------------------------------------

/// Solely-NATO page: a single bare NATO classification portion with
/// no US contribution. The closure operator fires
/// `CLOSURE_REL_TO_USA_NATO` at `Severity::Info` (silent lattice-layer
/// fact propagation per decisions.md D20); the projection's REL TO axis
/// gains USA + NATO; the page-classification axis preserves the
/// `Nato(_)` variant per §H.7 pp123-125 (the `pure_nato_lattice_vs_pagecontext_diverges`
/// divergence documented in PR 4b-B Commit 8).
///
/// S007 (the text-layer Severity::Suggest companion) is suppressed
/// on solely-NATO documents because there is no US-document context
/// to surface the byte-level "add REL TO USA, NATO" suggestion
/// against. This fixture asserts the lattice state directly via
/// `scheme.project`; the engine-output assertion (S007 NDJSON
/// diagnostic) lives in the dissem-NATO rule tests in
/// `crates/capco/tests/dissem_nato_pure_nato_portion.rs`.
///
/// Authority: §H.7 p127 (Notional Example Page 2 worked example —
/// `(//CTS//BOHEMIA//REL TO USA, NATO)`) + §G.2 Table 5 p40
/// (alliance-reciprocity ARH grounding) + §H.7 pp123-125 (NATO
/// classification preservation on solely-NATO pages).
#[test]
fn closure_nato_rel_to_solely_nato() {
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let projected = project_page(&[nato_portion]);

    // Classification axis: Nato(_) is preserved per §H.7 pp123-125.
    assert!(
        matches!(
            projected.0.classification,
            Some(MarkingClassification::Nato(_))
        ),
        "solely-NATO page must preserve the Nato classification variant \
         per §H.7 pp123-125; classification = {:?}",
        projected.0.classification,
    );

    // REL TO axis: closure injects USA + NATO silently.
    let usa = CountryCode::USA;
    let nato = CountryCode::try_new(b"NATO").expect("tetragraph");
    assert!(
        rel_to_contains(&projected, usa),
        "closure should inject USA into rel_to on solely-NATO page \
         (§H.7 p127 + §G.2 Table 5 p40); rel_to = {:?}",
        projected.0.rel_to,
    );
    assert!(
        rel_to_contains(&projected, nato),
        "closure should inject NATO into rel_to on solely-NATO page \
         (§H.7 p127 + §G.2 Table 5 p40); rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Pattern D fixture 6: closure_nato_rel_to_us_plus_nato
// ---------------------------------------------------------------------------

/// US document with a NATO portion: a US-classified portion alongside
/// a NATO-classified portion. The §H.7 reciprocal-raise rule
/// (pp123-125) flattens the NATO variant to `Us(_)` at the
/// page-classification layer when ANY US portion is in scope.
///
/// The closure operator's `CLOSURE_REL_TO_USA_NATO` trigger is
/// `TOK_NATO_CLASS`, which `satisfies_attrs` emits ONLY when the
/// joined marking's classification is `Nato(_)`. After the §H.7
/// reciprocal-raise flattens to `Us(_)`, the trigger no longer
/// fires — the closure does NOT inject USA/NATO at the lattice
/// layer on US+NATO mixed pages.
///
/// The NATO source provenance does survive on the FGI axis (via
/// the `fgi_marker` field from the per-portion `Nato(_)`
/// classification's foreign source), but the REL TO cone injection
/// is gated on the structural NATO classification, not on the FGI
/// signal. S007 owns the text-layer Severity::Suggest behavior for
/// this scenario — it fires through `Engine::lint`'s strict-
/// recognizer path and surfaces the byte-level "add REL TO USA,
/// NATO" suggestion. S007's engine-side fixture lives in
/// `crates/capco/tests/dissem_nato_*.rs`; this Pattern-D fixture
/// pins the lattice-layer non-injection observable.
///
/// Authority: §H.7 pp123-125 (reciprocal raise — US-bearing page →
/// US classification at banner; the classification-axis flatten
/// makes `TOK_NATO_CLASS` absent in the joined marking) +
/// §H.7 p127 + §G.2 Table 5 p40 (closure NATO REL TO cone, which
/// requires the NATO classification trigger to fire).
#[test]
fn closure_nato_rel_to_us_plus_nato() {
    let us_portion = classified_us(Classification::Secret);
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let projected = project_page(&[us_portion, nato_portion]);

    // Classification axis: reciprocal-raise flattens NATO to Us(Secret).
    assert!(
        matches!(
            projected.0.classification,
            Some(MarkingClassification::Us(Classification::Secret))
        ),
        "US+NATO mixed page must flatten classification to Us(Secret) \
         per §H.7 pp123-125 reciprocal-raise; classification = {:?}",
        projected.0.classification,
    );

    // REL TO axis: closure does NOT inject USA/NATO because the
    // NATO classification trigger is absent after the reciprocal-
    // raise flatten. S007 owns the text-layer suggestion path for
    // this case.
    let usa = CountryCode::USA;
    let nato = CountryCode::try_new(b"NATO").expect("tetragraph");
    assert!(
        !rel_to_contains(&projected, usa),
        "closure must NOT inject USA into rel_to on US+NATO mixed page \
         — the NATO classification was flattened to Us(_) by §H.7 \
         reciprocal-raise, so the CLOSURE_REL_TO_USA_NATO trigger \
         (TOK_NATO_CLASS) is absent; rel_to = {:?}",
        projected.0.rel_to,
    );
    assert!(
        !rel_to_contains(&projected, nato),
        "closure must NOT inject NATO into rel_to on US+NATO mixed \
         page — see USA assertion above; rel_to = {:?}",
        projected.0.rel_to,
    );

    // FGI axis: the NATO source provenance survives via the
    // `fgi_marker` field (the FgiSet axis carries NATO as a
    // foreign source even after the classification reciprocal-
    // raise). The S007 text-layer rule reads this and emits the
    // Severity::Suggest diagnostic.
    assert!(
        projected.0.fgi_marker.is_some(),
        "US+NATO mixed page must preserve NATO source provenance on \
         the FGI axis after classification flatten; fgi_marker = {:?}",
        projected.0.fgi_marker,
    );
}

// ---------------------------------------------------------------------------
// Pattern D fixture 7: closure_relido_unanimity
// ---------------------------------------------------------------------------

/// PR 4b-B installed the RELIDO observed-unanimity overlay on
/// `DissemSet`: at banner roll-up, RELIDO survives only if every
/// portion on the page carries RELIDO (§H.8 pp155-156). This fixture
/// confirms the overlay survives the PR 4b-D.2 hot-path flip — the
/// production page-projection path (`scheme.project(Scope::Page, ...)`)
/// must preserve the same supersession semantic the parity-gate
/// fixtures (`crates/capco/tests/page_context_lattice_parity.rs`)
/// already pin.
///
/// Scenario A: every portion has RELIDO → projection keeps RELIDO.
/// Scenario B: one portion has RELIDO, another doesn't → projection
/// drops RELIDO from the banner.
///
/// Authority: §H.8 pp155-156 (RELIDO observed-unanimity: "The
/// RELIDO marking is conveyed in the banner line if all of the
/// portions ... in the document contain the RELIDO marking").
#[test]
fn closure_relido_unanimity_all_portions() {
    let mut p1 = classified_us(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    let mut p2 = classified_us(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    let projected = project_page(&[p1, p2]);

    assert!(
        dissem_contains(&projected, DissemControl::Relido),
        "RELIDO unanimous across all portions must survive banner \
         roll-up (§H.8 pp155-156); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

#[test]
fn closure_relido_unanimity_drops_on_disagreement() {
    let mut p1 = classified_us(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    let p2 = classified_us(Classification::Secret);
    let projected = project_page(&[p1, p2]);

    assert!(
        !dissem_contains(&projected, DissemControl::Relido),
        "RELIDO non-unanimous (some portion lacks it) must be dropped \
         at banner roll-up (§H.8 pp155-156); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}
