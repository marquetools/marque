// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T048c — `render_canonical` `EmissionForm` fixtures.
//!
//! Ships in PR 3c.2.A per `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md`
//! PM-10 with two patterns of tests:
//!
//! 1. **Enabled** — `EmissionForm::Auto` + `Scope::{Page, Portion}`
//!    assertions for NOFORN / SECRET / FOUO. These pin byte-identity
//!    with the pre-3c.2 `render_banner` / `render_portion` outputs;
//!    they are the load-bearing byte-identity gate for PR 3c.2.A.
//!
//! 2. **`#[ignore]`-gated** — `EmissionForm::Portion` /
//!    `EmissionForm::BannerTitle` / `EmissionForm::BannerAbbreviation`
//!    assertions for the same three tokens. These carry the FR-052
//!    acceptance criteria for PR 3c.2.B: when the §G.1 Table 4
//!    dispatch body lands in `CapcoScheme::render_canonical`, the
//!    `#[ignore]` removal flips each to enabled and the assertion
//!    holds.
//!
//! # Why three tokens
//!
//! - **NOFORN** — distinct portion abbreviation (`NF`), distinct
//!   banner abbreviation (`NOFORN`), distinct title (`NOT RELEASABLE
//!   TO FOREIGN NATIONALS`). All three §G.1 Table 4 columns differ
//!   per `crates/ism/src/marking_forms.rs` row at title="NOT
//!   RELEASABLE TO FOREIGN NATIONALS" — the canonical exercise of the
//!   four-form ambiguity.
//! - **SECRET** — US classification level; portion="S",
//!   banner="SECRET", title="SECRET". No distinct abbreviation form —
//!   `BannerAbbreviation` should fall back to title per project memory
//!   `project_formset_banner_abbreviation_semantic`.
//! - **FOUO** — distinct title (`FOR OFFICIAL USE ONLY`), banner =
//!   portion = `FOUO`. Exercises the `portion = banner != title` case
//!   per `crates/ism/src/marking_forms.rs` row.
//!
//! # CAPCO §-citations verified at authorship
//!
//! - NOFORN per CAPCO-2016 §H.8 p145 (Banner Title `NOT RELEASABLE TO
//!   FOREIGN NATIONALS`, Banner Abbreviation `NOFORN`, Portion `NF`).
//!   Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 3c.2.A
//!   authorship.
//! - SECRET per CAPCO-2016 §H.1 p48 (US Classification SECRET, Portion
//!   `S`).
//! - FOUO per CAPCO-2016 §H.8 p134 (Banner Title `FOR OFFICIAL USE
//!   ONLY`, Banner Abbreviation `FOUO`, Portion `FOUO`).

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{CanonicalAttrs, Classification, DissemControl, MarkingClassification};
use marque_scheme::{EmissionForm, MarkingScheme, RenderContext, SchemaVersionId, Scope};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a SECRET-only marking (no dissem). Canonical banner =
/// `"SECRET"`; canonical portion = `"S"`.
fn make_secret() -> CapcoMarking {
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
    CapcoMarking::new(attrs)
}

/// Build a SECRET + NOFORN marking. Canonical banner =
/// `"SECRET//NOFORN"`; canonical portion = `"S//NF"`. Per CAPCO-2016
/// §H.8 p145 NOFORN axis.
fn make_secret_noforn() -> CapcoMarking {
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
    attrs.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
    CapcoMarking::new(attrs)
}

/// Build an UNCLASSIFIED + FOUO marking. Canonical banner =
/// `"UNCLASSIFIED//FOUO"`; canonical portion = `"U//FOUO"`. Per
/// CAPCO-2016 §H.8 p134 (FOUO is U-only).
fn make_unclassified_fouo() -> CapcoMarking {
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Unclassified));
    attrs.dissem_us = vec![DissemControl::Fouo].into_boxed_slice();
    CapcoMarking::new(attrs)
}

fn ctx(scope: Scope, form: EmissionForm) -> RenderContext {
    RenderContext::new(scope, form, SchemaVersionId::MarqueMvp3)
}

/// Render via `render_canonical(_, &RenderContext, _)` and return the
/// resulting bytes.
fn render(scheme: &CapcoScheme, marking: &CapcoMarking, ctx: &RenderContext) -> String {
    let mut out = String::new();
    scheme
        .render_canonical(marking, ctx, &mut out)
        .expect("render_canonical must succeed for Portion / Page / Document");
    out
}

// ===========================================================================
// ENABLED — `EmissionForm::Auto + Scope::Page` byte-identity
// ===========================================================================
//
// These tests are the load-bearing byte-identity gate for PR 3c.2.A:
// `Auto + Page` MUST produce the same bytes as `render_banner`, which
// is what the pre-3c.2 engine emitted.

#[test]
fn auto_page_secret_matches_render_banner() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();
    let auto_page = render(&scheme, &marking, &ctx(Scope::Page, EmissionForm::Auto));
    let banner = scheme.render_banner(&marking);
    assert_eq!(
        auto_page, banner,
        "Auto + Page MUST byte-match render_banner output"
    );
    assert_eq!(banner, "SECRET");
}

#[test]
fn auto_page_noforn_matches_render_banner() {
    let scheme = CapcoScheme::new();
    let marking = make_secret_noforn();
    let auto_page = render(&scheme, &marking, &ctx(Scope::Page, EmissionForm::Auto));
    let banner = scheme.render_banner(&marking);
    assert_eq!(
        auto_page, banner,
        "Auto + Page MUST byte-match render_banner output"
    );
    assert_eq!(banner, "SECRET//NOFORN");
}

#[test]
fn auto_page_fouo_matches_render_banner() {
    let scheme = CapcoScheme::new();
    let marking = make_unclassified_fouo();
    let auto_page = render(&scheme, &marking, &ctx(Scope::Page, EmissionForm::Auto));
    let banner = scheme.render_banner(&marking);
    assert_eq!(
        auto_page, banner,
        "Auto + Page MUST byte-match render_banner output"
    );
    assert_eq!(banner, "UNCLASSIFIED//FOUO");
}

// ===========================================================================
// ENABLED — `EmissionForm::Auto + Scope::Portion` byte-identity
// ===========================================================================

#[test]
fn auto_portion_secret_matches_render_portion() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();
    let auto_portion = render(&scheme, &marking, &ctx(Scope::Portion, EmissionForm::Auto));
    let portion = scheme.render_portion(&marking);
    assert_eq!(
        auto_portion, portion,
        "Auto + Portion MUST byte-match render_portion output"
    );
    assert_eq!(portion, "S");
}

#[test]
fn auto_portion_noforn_matches_render_portion() {
    let scheme = CapcoScheme::new();
    let marking = make_secret_noforn();
    let auto_portion = render(&scheme, &marking, &ctx(Scope::Portion, EmissionForm::Auto));
    let portion = scheme.render_portion(&marking);
    assert_eq!(
        auto_portion, portion,
        "Auto + Portion MUST byte-match render_portion output"
    );
    assert_eq!(portion, "S//NF");
}

#[test]
fn auto_portion_fouo_matches_render_portion() {
    let scheme = CapcoScheme::new();
    let marking = make_unclassified_fouo();
    let auto_portion = render(&scheme, &marking, &ctx(Scope::Portion, EmissionForm::Auto));
    let portion = scheme.render_portion(&marking);
    assert_eq!(
        auto_portion, portion,
        "Auto + Portion MUST byte-match render_portion output"
    );
    assert_eq!(portion, "U//FOUO");
}

// ===========================================================================
// `#[ignore]`-gated — `EmissionForm::Portion` (force portion form)
// ===========================================================================
//
// FR-052 acceptance criteria for PR 3c.2.B: when the §G.1 Table 4
// dispatch body lands in `CapcoScheme::render_canonical`, force-Portion
// emits the portion-mark form regardless of `Scope`. The `#[ignore]`
// flips to enabled at 3c.2.B; the assertion shape pins the contract
// today so the migration is mechanical (just remove the attribute).

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_portion_secret_returns_portion_mark() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();
    let out = render(&scheme, &marking, &ctx(Scope::Page, EmissionForm::Portion));
    // Per CAPCO-2016 §H.1 p48: US Secret portion = "S". Force-Portion
    // emits portion form even though Scope::Page would default to
    // banner under `Auto`.
    assert_eq!(out, "S");
}

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_portion_noforn_returns_portion_mark() {
    let scheme = CapcoScheme::new();
    let marking = make_secret_noforn();
    let out = render(&scheme, &marking, &ctx(Scope::Page, EmissionForm::Portion));
    // Per CAPCO-2016 §H.8 p145: NOFORN portion = "NF". On a
    // SECRET+NOFORN marking, force-Portion yields "S//NF".
    assert_eq!(out, "S//NF");
}

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_portion_fouo_returns_portion_mark() {
    let scheme = CapcoScheme::new();
    let marking = make_unclassified_fouo();
    let out = render(&scheme, &marking, &ctx(Scope::Page, EmissionForm::Portion));
    // Per CAPCO-2016 §H.8 p134: FOUO portion = "FOUO" (banner = portion
    // for FOUO). force-Portion on U+FOUO yields "U//FOUO".
    assert_eq!(out, "U//FOUO");
}

// ===========================================================================
// `#[ignore]`-gated — `EmissionForm::BannerTitle` (force long form)
// ===========================================================================

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_banner_title_secret_returns_title() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();
    let out = render(
        &scheme,
        &marking,
        &ctx(Scope::Page, EmissionForm::BannerTitle),
    );
    // Per CAPCO-2016 §H.1 p48: US Secret title == banner == "SECRET"
    // (US classifications have no distinct long form).
    assert_eq!(out, "SECRET");
}

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_banner_title_noforn_returns_title() {
    let scheme = CapcoScheme::new();
    let marking = make_secret_noforn();
    let out = render(
        &scheme,
        &marking,
        &ctx(Scope::Page, EmissionForm::BannerTitle),
    );
    // Per CAPCO-2016 §H.8 p145: NOFORN title = "NOT RELEASABLE TO
    // FOREIGN NATIONALS" (distinct from banner abbreviation).
    assert_eq!(out, "SECRET//NOT RELEASABLE TO FOREIGN NATIONALS");
}

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_banner_title_fouo_returns_title() {
    let scheme = CapcoScheme::new();
    let marking = make_unclassified_fouo();
    let out = render(
        &scheme,
        &marking,
        &ctx(Scope::Page, EmissionForm::BannerTitle),
    );
    // Per CAPCO-2016 §H.8 p134: FOUO title = "FOR OFFICIAL USE ONLY"
    // (distinct from banner/portion abbreviation `FOUO`).
    assert_eq!(out, "UNCLASSIFIED//FOR OFFICIAL USE ONLY");
}

// ===========================================================================
// `#[ignore]`-gated — `EmissionForm::BannerAbbreviation` (force short form)
// ===========================================================================

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_banner_abbreviation_secret_falls_back_to_title() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();
    let out = render(
        &scheme,
        &marking,
        &ctx(Scope::Page, EmissionForm::BannerAbbreviation),
    );
    // Per CAPCO-2016 §H.1 p48: US Secret has no distinct banner
    // abbreviation. Per project memory
    // `project_formset_banner_abbreviation_semantic`,
    // BannerAbbreviation falls back to BannerTitle when no distinct
    // short form exists. Output equals `BannerTitle` for SECRET.
    assert_eq!(out, "SECRET");
}

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_banner_abbreviation_noforn_returns_abbreviation() {
    let scheme = CapcoScheme::new();
    let marking = make_secret_noforn();
    let out = render(
        &scheme,
        &marking,
        &ctx(Scope::Page, EmissionForm::BannerAbbreviation),
    );
    // Per CAPCO-2016 §H.8 p145: NOFORN banner abbreviation = "NOFORN"
    // (distinct from title "NOT RELEASABLE TO FOREIGN NATIONALS").
    assert_eq!(out, "SECRET//NOFORN");
}

#[test]
#[ignore = "blocked on T048b: forced-mode EmissionForm dispatch awaits engine-side RenderContext construction at the fix-emit boundary"]
fn explicit_banner_abbreviation_fouo_returns_abbreviation() {
    let scheme = CapcoScheme::new();
    let marking = make_unclassified_fouo();
    let out = render(
        &scheme,
        &marking,
        &ctx(Scope::Page, EmissionForm::BannerAbbreviation),
    );
    // Per CAPCO-2016 §H.8 p134: FOUO banner abbreviation = "FOUO"
    // (equals portion form; distinct from title).
    assert_eq!(out, "UNCLASSIFIED//FOUO");
}
