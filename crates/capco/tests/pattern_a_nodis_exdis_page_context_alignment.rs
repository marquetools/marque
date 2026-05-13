// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.F-engine-gap — PageContext alignment with the just-
//! merged §H.9 NODIS/EXDIS Pattern A PageRewrites.
//!
//! These tests pin a dual-route alignment invariant: the
//! `PageContext::expected_rel_to()` read API (engine-side, reads raw
//! portion attrs and applies supersession inline) and the
//! `scheme.project(Scope::Page, ...)` route (scheme-side, applies the
//! declared PageRewrites) MUST agree on the projected REL TO state when a
//! portion carries NODIS or EXDIS.
//!
//! # The gap (before this PR)
//!
//! 8.F (PR #393) and 8.F.2 (PR #394) declared four PageRewrites:
//! `capco/{nodis,exdis,sbu-nf,les-nf}-implies-noforn`. Each injects NOFORN
//! into the page CAT_DISSEM at `scheme.project(Scope::Page, ...)`; the
//! existing `capco/noforn-clears-rel-to` rewrite then clears CAT_REL_TO.
//! The final-projection layer was already correct.
//!
//! But `PageContext::expected_rel_to()` is a parallel read API — it walks
//! raw portion data and applies supersession inline rather than routing
//! through the scheme. Prior to this PR, that inline logic only
//! short-circuited on (a) any portion carrying NOFORN directly, and
//! (b) the SBU-NF/LES-NF classified-context split. NODIS/EXDIS in a
//! portion did NOT short-circuit, despite §H.9 p172/p174 being explicit:
//!
//! - §H.9 p172 (EXDIS), `crates/capco/docs/CAPCO-2016.md:4241`:
//!   "REL TO is not authorized in the banner line if any portion contains
//!   EXDIS information. In this case, NOFORN would convey in the banner
//!   line."
//! - §H.9 p174 (NODIS), `crates/capco/docs/CAPCO-2016.md:4301`:
//!   "REL TO is not authorized in the banner line if any portion contains
//!   NODIS information. In this case, NOFORN would convey in the banner
//!   line."
//!
//! Net effect: any caller reading `PageContext::expected_rel_to()` saw a
//! non-empty intersection (the raw portion data) when a NODIS/EXDIS
//! portion was present, while the scheme's final projection saw it
//! cleared. The S005/S006 atom-semantics rule (`crates/capco/src/rules.rs`)
//! and `render_expected_banner` (`crates/ism/src/page_context.rs`) both
//! depend on the read API matching the final-projection state.
//!
//! # What this PR closed
//!
//! `expected_non_ic_dissem` now sets `needs_nf = true` when any portion
//! carries NODIS or EXDIS (classification-independent per the §H.9
//! passages above). `expected_rel_to` already short-circuits when
//! `needs_nf` is true (lines 405-409), so the read API now agrees with
//! the scheme's PageRewrite-driven projection.
//!
//! # Test inventory
//!
//! 1. `nodis_portion_clears_rel_to_via_page_rewrite_AND_page_context_agrees`
//! 2. `exdis_portion_clears_rel_to_via_page_rewrite_AND_page_context_agrees`
//! 3. `rel_to_intersection_preserved_when_no_nodis_or_exdis_present`
//!
//! Tests 1-2 exercise positive alignment (both routes empty REL TO).
//! Test 3 is the inverse / negative case (rust-reviewer MEDIUM finding
//! during preflight): pin that the new `seen.contains` clause in
//! `expected_non_ic_dissem` does NOT over-fire when neither NODIS nor
//! EXDIS is in the portion set.

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, MarkingClassification, NonIcDissem, PageContext,
};
use marque_scheme::{MarkingScheme, Scope};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

fn gbr() -> CountryCode {
    CountryCode::try_new(b"GBR")
        .expect("GBR is a valid 3-char CAPCO trigraph per CVEnumISMCATRelTo.xsd")
}

fn fra() -> CountryCode {
    CountryCode::try_new(b"FRA")
        .expect("FRA is a valid 3-char CAPCO trigraph per CVEnumISMCATRelTo.xsd")
}

fn portion_with_non_ic(c: Classification, non_ic: &[NonIcDissem]) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.non_ic_dissem = non_ic.to_vec().into_boxed_slice();
    a
}

fn portion_with_rel_to(c: Classification, countries: &[CountryCode]) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.rel_to = countries.to_vec().into_boxed_slice();
    a
}

/// Build a `PageContext` accumulator from a slice of `CanonicalAttrs`.
/// Mirrors what `Engine::lint` does internally per page.
fn page_context_from(portions: &[CanonicalAttrs]) -> PageContext {
    let mut ctx = PageContext::new();
    for attrs in portions {
        ctx.add_portion(attrs.clone());
    }
    ctx
}

/// Lift an `CanonicalAttrs` to a `CapcoMarking` for scheme.project.
fn lift(a: CanonicalAttrs) -> CapcoMarking {
    CapcoMarking::new(a)
}

// ---------------------------------------------------------------------------
// Test 1 — NODIS dual-route alignment
// ---------------------------------------------------------------------------

/// `(S//NODIS)` portion paired with `(S//REL TO USA, GBR)`: both routes
/// must agree that the projected page REL TO is empty.
///
/// Route A (`scheme.project(Scope::Page, ...)`): the
/// `capco/nodis-implies-noforn` PageRewrite (from 8.F #393) fires, adding
/// NOFORN to page CAT_DISSEM; `capco/noforn-clears-rel-to` then clears
/// CAT_REL_TO.
///
/// Route B (`PageContext::expected_rel_to()`): the engine-gap close in
/// THIS PR makes `expected_non_ic_dissem` set `needs_nf = true` when
/// NODIS is present in any portion. `expected_rel_to` short-circuits on
/// `needs_nf == true` and returns empty.
///
/// Authority: CAPCO-2016 §H.9 p174 verbatim — "REL TO is not authorized
/// in the banner line if any portion contains NODIS information. In this
/// case, NOFORN would convey in the banner line."
/// (`crates/capco/docs/CAPCO-2016.md:4301`).
#[test]
fn nodis_portion_clears_rel_to_via_page_rewrite_and_page_context_agrees() {
    let scheme = CapcoScheme::new();

    let nodis_portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Nodis]);
    let rel_to_portion = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);

    // Route A — scheme.project (PageRewrite-driven)
    let projected = scheme.project(
        Scope::Page,
        &[lift(nodis_portion.clone()), lift(rel_to_portion.clone())],
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "scheme.project(Scope::Page) must clear REL TO when any portion \
         has NODIS (capco/nodis-implies-noforn → capco/noforn-clears-rel-to \
         per §H.9 p174); got rel_to = {:?}",
        projected.0.rel_to,
    );

    // Route B — PageContext.expected_rel_to (engine-side read API)
    let ctx = page_context_from(&[nodis_portion, rel_to_portion]);

    assert!(
        ctx.expected_rel_to().is_empty(),
        "PageContext::expected_rel_to() must short-circuit to empty when \
         any portion has NODIS, via the needs_nf flag from \
         expected_non_ic_dissem per §H.9 p174; got rel_to = {:?}",
        ctx.expected_rel_to(),
    );
}

// ---------------------------------------------------------------------------
// Test 2 — EXDIS dual-route alignment
// ---------------------------------------------------------------------------

/// Symmetric to test 1 for EXDIS.
///
/// Authority: CAPCO-2016 §H.9 p172 verbatim — "REL TO is not authorized
/// in the banner line if any portion contains EXDIS information. In this
/// case, NOFORN would convey in the banner line."
/// (`crates/capco/docs/CAPCO-2016.md:4241`).
#[test]
fn exdis_portion_clears_rel_to_via_page_rewrite_and_page_context_agrees() {
    let scheme = CapcoScheme::new();

    let exdis_portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Exdis]);
    let rel_to_portion = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);

    // Route A — scheme.project
    let projected = scheme.project(
        Scope::Page,
        &[lift(exdis_portion.clone()), lift(rel_to_portion.clone())],
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "scheme.project(Scope::Page) must clear REL TO when any portion \
         has EXDIS (capco/exdis-implies-noforn → capco/noforn-clears-rel-to \
         per §H.9 p172); got rel_to = {:?}",
        projected.0.rel_to,
    );

    // Route B — PageContext.expected_rel_to
    let ctx = page_context_from(&[exdis_portion, rel_to_portion]);

    assert!(
        ctx.expected_rel_to().is_empty(),
        "PageContext::expected_rel_to() must short-circuit to empty when \
         any portion has EXDIS, via the needs_nf flag from \
         expected_non_ic_dissem per §H.9 p172; got rel_to = {:?}",
        ctx.expected_rel_to(),
    );
}

// ---------------------------------------------------------------------------
// Test 3 — Inverse / negative case (rust-reviewer MEDIUM finding)
// ---------------------------------------------------------------------------

/// Pin that the new `seen.contains(&NonIcDissem::Nodis) || ... Exdis ...`
/// clause in `expected_non_ic_dissem` does NOT over-fire when neither
/// trigger is present.
///
/// Fixture: two REL TO portions with overlapping country sets (USA, GBR)
/// and one with USA, GBR, FRA. No NODIS, no EXDIS, no NOFORN, no SBU-NF,
/// no LES-NF — none of the four `needs_nf` triggers are in play.
///
/// Expected: both routes return the intersection [USA, GBR].
///
/// Why this matters: a too-broad `seen.contains` predicate (e.g., one
/// that fires on ANY non-empty `non_ic_dissem` regardless of content)
/// would silently break country-rollup for documents containing
/// unrelated non-IC tokens (FOUO, SBU without NOFORN, LES without
/// NOFORN). This test guards against that regression.
#[test]
fn rel_to_intersection_preserved_when_no_nodis_or_exdis_present() {
    let scheme = CapcoScheme::new();

    let p1 = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);
    let p2 = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr(), fra()]);

    // Route A — scheme.project
    let projected = scheme.project(Scope::Page, &[lift(p1.clone()), lift(p2.clone())]);

    let projected_set: std::collections::BTreeSet<&str> =
        projected.0.rel_to.iter().map(|c| c.as_str()).collect();
    let expected_set: std::collections::BTreeSet<&str> = ["USA", "GBR"].into_iter().collect();
    assert_eq!(
        projected_set, expected_set,
        "scheme.project(Scope::Page) must compute the REL TO intersection \
         normally when no NODIS/EXDIS/NOFORN/{{SBU,LES}}-NF triggers are \
         present; got rel_to = {:?}",
        projected.0.rel_to,
    );

    // Route B — PageContext.expected_rel_to
    let ctx = page_context_from(&[p1, p2]);
    let pc_rel_to = ctx.expected_rel_to();
    let pc_set: std::collections::BTreeSet<&str> = pc_rel_to.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        pc_set, expected_set,
        "PageContext::expected_rel_to() must compute the intersection \
         normally when no needs_nf triggers are present; got rel_to = {pc_rel_to:?}",
    );
}
