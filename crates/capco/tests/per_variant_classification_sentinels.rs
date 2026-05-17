// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR #505 — per-variant classification sentinel coverage.
//!
//! Exercises the two new sentinels `TOK_NATO_CLASS` / `TOK_FGI_CLASS`
//! and the repaired `collect_present_tokens` asymmetry. After this PR
//! `collect_present_tokens` emits a concrete `TokenRef::Token(...)` for
//! each `MarkingClassification` variant (pre-PR-#505 NATO was emitted
//! as `AnyInCategory(CAT_NON_US_CLASSIFICATION)` while FGI / JOINT
//! emitted concrete tokens — see
//! `crates/capco/src/scheme/predicates/satisfies.rs`'s
//! "Per-variant classification emission" doc block).
//!
//! Coverage matrix:
//!
//! | Classification axis state | TOK_NATO_CLASS | TOK_FGI_CLASS | TOK_FGI_MARKER | TOK_JOINT |
//! |---|---|---|---|---|
//! | `Us(_)` | no | no | no | no |
//! | `Nato(_)` | yes | no | no | no |
//! | `Fgi { countries: [DEU] }` (acknowledged) | no | yes | yes (dual-axis) | no |
//! | `Fgi { countries: [] }` (concealed) | no | yes | yes (dual-axis) | no |
//! | `Joint { countries: [USA, GBR] }` | no | no | no | yes |
//! | `Conflict { .. }` | no | no | no | no |
//! | dissem-axis fgi_marker only (`Us` cls, `fgi_marker = Some(_)`) | no | no | yes (dual-axis) | no |
//!
//! Authority: CAPCO-2016 §H.7 p123 (FGI grammar — singular-owner
//! invariant for the classification axis); CAPCO-2016 §H.3 p56 (JOINT
//! classification); CAPCO-2016 §H.2 p55 (NATO classification);
//! `crates/capco/src/scheme/predicates/satisfies.rs` doc block
//! ("Per-variant classification emission" — the in-tree authority for
//! the emission contract).

use marque_capco::CapcoMarking;
use marque_capco::scheme::{CapcoScheme, TOK_FGI_CLASS, TOK_FGI_MARKER, TOK_JOINT, TOK_NATO_CLASS};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, FgiClassification, FgiMarker,
    ForeignClassification, JointClassification, MarkingClassification, NatoClassification,
};
use marque_scheme::{MarkingScheme, TokenRef};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn wrap(attrs: CanonicalAttrs) -> CapcoMarking {
    CapcoMarking::new(attrs)
}

fn us_secret() -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a
}

fn nato_secret() -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    a
}

fn fgi_acknowledged_deu() -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    let deu = CountryCode::try_new(b"DEU").expect("DEU is a valid trigraph");
    a.classification = Some(MarkingClassification::Fgi(FgiClassification {
        countries: vec![deu].into_boxed_slice(),
        level: Classification::Secret,
    }));
    a
}

fn fgi_concealed() -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Fgi(FgiClassification {
        countries: Box::new([]),
        level: Classification::Secret,
    }));
    a
}

fn joint_usa_gbr() -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    let gbr = CountryCode::try_new(b"GBR").expect("GBR is a valid trigraph");
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, gbr].into_boxed_slice(),
    }));
    a
}

fn conflict_us_nato() -> CanonicalAttrs {
    // US wins; foreign preserved for FGI fix path. Construction shape
    // mirrors `MarkingClassification::Conflict` doc-comment in
    // `crates/ism/src/attrs.rs`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Conflict {
        us: Classification::TopSecret,
        foreign: Box::new(ForeignClassification::Nato(
            NatoClassification::CosmicTopSecret,
        )),
    });
    a
}

fn us_with_fgi_marker_only() -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.fgi_marker = Some(FgiMarker::SourceConcealed);
    a
}

// ---------------------------------------------------------------------------
// satisfies — per-variant sentinel arms
// ---------------------------------------------------------------------------

#[test]
fn us_secret_matches_no_per_variant_sentinel() {
    let scheme = CapcoScheme::new();
    let m = wrap(us_secret());
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_JOINT)));
}

#[test]
fn nato_secret_matches_nato_class_only() {
    let scheme = CapcoScheme::new();
    let m = wrap(nato_secret());
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_JOINT)));
}

#[test]
fn fgi_acknowledged_matches_fgi_class_and_fgi_marker_dual_axis() {
    let scheme = CapcoScheme::new();
    let m = wrap(fgi_acknowledged_deu());
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_CLASS)));
    // FGI_MARKER is dual-axis: also matches `MarkingClassification::Fgi(_)`.
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_JOINT)));
}

#[test]
fn fgi_concealed_matches_fgi_class_and_fgi_marker_dual_axis() {
    // CAPCO-2016 §H.7 p123: source-concealed FGI has 0 countries
    // (`//FGI S`). The classification-axis sentinel matching is the
    // same as the acknowledged case — variant identity, not country
    // population, drives the per-variant emission.
    let scheme = CapcoScheme::new();
    let m = wrap(fgi_concealed());
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_CLASS)));
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_JOINT)));
}

#[test]
fn joint_matches_joint_only() {
    let scheme = CapcoScheme::new();
    let m = wrap(joint_usa_gbr());
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_JOINT)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_MARKER)));
}

#[test]
fn conflict_excluded_from_per_variant_sentinels() {
    // `Conflict { .. }` is deliberately excluded from
    // `TOK_NON_US_CLASSIFICATION` (per the fn doc in
    // `satisfies.rs`); the per-variant sentinels follow the same
    // convention. `Conflict` is E012's concern.
    let scheme = CapcoScheme::new();
    let m = wrap(conflict_us_nato());
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_JOINT)));
}

#[test]
fn dissem_axis_fgi_marker_without_fgi_classification_matches_fgi_marker_only() {
    // Pre-PR-#505 the symmetric concern was that NATO classification
    // emitted only an umbrella `AnyInCategory(...)` while FGI emitted
    // a concrete token. The reverse direction also matters: dissem-
    // axis `fgi_marker.is_some()` with `Us(_)` classification must
    // match `TOK_FGI_MARKER` (dual-axis) but NOT `TOK_FGI_CLASS`
    // (strict classification-axis). This pins the strict / dual-axis
    // split.
    let scheme = CapcoScheme::new();
    let m = wrap(us_with_fgi_marker_only());
    assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!scheme.satisfies(&m, &TokenRef::Token(TOK_JOINT)));
}

// ---------------------------------------------------------------------------
// iter_present_tokens — emission contract (the asymmetry repair)
// ---------------------------------------------------------------------------

fn collect_emitted(scheme: &CapcoScheme, marking: &CapcoMarking) -> Vec<TokenRef> {
    scheme.iter_present_tokens(marking).collect()
}

#[test]
fn us_emission_excludes_classification_sentinels() {
    let scheme = CapcoScheme::new();
    let m = wrap(us_secret());
    let tokens = collect_emitted(&scheme, &m);
    assert!(!tokens.contains(&TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_JOINT)));
}

#[test]
fn nato_emission_includes_nato_class_concrete_token() {
    // Pre-#505: NATO emitted `AnyInCategory(CAT_NON_US_CLASSIFICATION)`
    // (umbrella). Post-#505: NATO emits `Token(TOK_NATO_CLASS)`
    // (concrete per-variant).
    let scheme = CapcoScheme::new();
    let m = wrap(nato_secret());
    let tokens = collect_emitted(&scheme, &m);
    assert!(
        tokens.contains(&TokenRef::Token(TOK_NATO_CLASS)),
        "NATO emission must include `Token(TOK_NATO_CLASS)`; got {tokens:?}",
    );
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_JOINT)));
}

#[test]
fn fgi_emission_includes_both_fgi_marker_and_fgi_class() {
    // Post-#505: FGI emits BOTH `TOK_FGI_MARKER` (dual-axis legacy
    // emission) AND `TOK_FGI_CLASS` (new strict classification-axis
    // sentinel). Family predicates that read either shape see FGI.
    let scheme = CapcoScheme::new();
    let m = wrap(fgi_acknowledged_deu());
    let tokens = collect_emitted(&scheme, &m);
    assert!(
        tokens.contains(&TokenRef::Token(TOK_FGI_MARKER)),
        "FGI emission must include `Token(TOK_FGI_MARKER)` (dual-axis); got {tokens:?}",
    );
    assert!(
        tokens.contains(&TokenRef::Token(TOK_FGI_CLASS)),
        "FGI emission must include `Token(TOK_FGI_CLASS)` (strict); got {tokens:?}",
    );
    assert!(!tokens.contains(&TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_JOINT)));
}

#[test]
fn fgi_concealed_emission_matches_acknowledged() {
    // Variant identity drives emission, not country count.
    let scheme = CapcoScheme::new();
    let m = wrap(fgi_concealed());
    let tokens = collect_emitted(&scheme, &m);
    assert!(tokens.contains(&TokenRef::Token(TOK_FGI_MARKER)));
    assert!(tokens.contains(&TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_JOINT)));
}

#[test]
fn joint_emission_includes_joint_only() {
    let scheme = CapcoScheme::new();
    let m = wrap(joint_usa_gbr());
    let tokens = collect_emitted(&scheme, &m);
    assert!(
        tokens.contains(&TokenRef::Token(TOK_JOINT)),
        "JOINT emission must include `Token(TOK_JOINT)`; got {tokens:?}",
    );
    assert!(!tokens.contains(&TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_MARKER)));
}

#[test]
fn conflict_emission_excludes_all_classification_sentinels() {
    // The pre-existing `Us(_) | Conflict { .. }` arm in
    // `collect_present_tokens` emits nothing for the classification
    // axis. PR #505 does not alter that behavior — the per-variant
    // sentinels track the lawful axis variants only.
    let scheme = CapcoScheme::new();
    let m = wrap(conflict_us_nato());
    let tokens = collect_emitted(&scheme, &m);
    assert!(!tokens.contains(&TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_FGI_MARKER)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_JOINT)));
}

#[test]
fn dissem_axis_fgi_marker_emits_fgi_marker_without_fgi_class() {
    // PR #505 invariant on the dissem-axis side: `attrs.fgi_marker.is_some()`
    // with `Us(_)` classification emits `TOK_FGI_MARKER` (via the
    // separate per-`attrs.fgi_marker` block in
    // `collect_present_tokens`) but does NOT emit `TOK_FGI_CLASS`.
    // The new strict sentinel reflects classification-axis variant
    // identity only.
    let scheme = CapcoScheme::new();
    let m = wrap(us_with_fgi_marker_only());
    let tokens = collect_emitted(&scheme, &m);
    assert!(
        tokens.contains(&TokenRef::Token(TOK_FGI_MARKER)),
        "dissem-axis fgi_marker must emit `Token(TOK_FGI_MARKER)`; got {tokens:?}",
    );
    assert!(
        !tokens.contains(&TokenRef::Token(TOK_FGI_CLASS)),
        "dissem-axis fgi_marker must NOT emit `Token(TOK_FGI_CLASS)`; got {tokens:?}",
    );
    assert!(!tokens.contains(&TokenRef::Token(TOK_NATO_CLASS)));
    assert!(!tokens.contains(&TokenRef::Token(TOK_JOINT)));
}
