// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.2.B PM-B-10 — Byte-equivalence test for the
//! `CapcoScheme::canonicalize` trait override against the
//! `marque_ism::from_parsed_unchecked` adapter.
//!
//! The override body is literal `marque_ism::from_parsed_unchecked(parsed)`
//! verbatim (PM-B-1) — semantic equivalence is by construction. This
//! file pins that property at the unit-test level for a representative
//! set of CAPCO markings drawn from the strict-path corpus, so any
//! future drift between the two paths (e.g., a well-meaning refactor
//! that splits a code path in the override but forgets to mirror it in
//! the adapter, or vice versa) surfaces as a test failure immediately
//! rather than waiting for T056 corpus regression to catch it.
//!
//! **Lifetime of this test**: it remains valid through PR 3c.2.D
//! (audit-schema cutover). At PR 3c.2.E the adapter
//! `marque_ism::from_parsed_unchecked` is deleted; this test's
//! comparison side either retires alongside the adapter or is
//! refactored to invoke a second canonicalization-by-construction
//! oracle.
//!
//! **Authority**:
//! - `docs/plans/2026-05-20-pr3c2-b-pm-decisions.md` PM-B-10
//!   (behavior-focused byte-equivalence test requirement).
//! - `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` PM-1 (the trait
//!   method signature this exercises).
//! - `crates/ism/src/canonical.rs:216-303` (the adapter body — same
//!   semantics as the override).

use marque_capco::scheme::CapcoScheme;
use marque_core::Parser;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::{MarkingScheme, Span};

/// Drive the parser twice on the same source so both call paths
/// receive byte-identical `ParsedAttrs` inputs. (Re-parsing rather
/// than cloning sidesteps `ParsedAttrs<'src>`'s lifetime constraint
/// — the type carries borrowed token spans.)
fn assert_canonical_equivalence(source: &[u8], kind: MarkingType) {
    let scheme = CapcoScheme::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind,
    };

    // Adapter path — sole oracle until PR 3c.2.E retires the adapter.
    let parsed_adapter = parser
        .parse(&candidate, source)
        .expect("strict-path corpus fixture must parse");
    // Test-fixture carve-out per Constitution V Principle V.
    let via_adapter = marque_ism::from_parsed_unchecked(parsed_adapter.attrs);

    // Trait override path — the migration target.
    let parsed_override = parser
        .parse(&candidate, source)
        .expect("strict-path corpus fixture must parse");
    let via_override = scheme.canonicalize(parsed_override.attrs);

    assert_eq!(
        via_adapter,
        via_override,
        "CapcoScheme::canonicalize must produce CanonicalAttrs \
         byte-identical to marque_ism::from_parsed_unchecked for input \
         {input:?}",
        input = String::from_utf8_lossy(source),
    );
}

// =============================================================================
// Portion-form fixtures — representative slice across every CAPCO axis
// =============================================================================

#[test]
fn portion_us_classification_only() {
    assert_canonical_equivalence(b"(S)", MarkingType::Portion);
}

#[test]
fn portion_us_dissem_noforn() {
    assert_canonical_equivalence(b"(S//NF)", MarkingType::Portion);
}

#[test]
fn portion_us_rel_to_usa_gbr() {
    assert_canonical_equivalence(b"(S//REL TO USA, GBR)", MarkingType::Portion);
}

#[test]
fn portion_sci_si_g_with_orcon_noforn() {
    // SCI compartment, classification ascends to TS per §H.4 p80.
    assert_canonical_equivalence(b"(TS//SI-G//OC/NF)", MarkingType::Portion);
}

#[test]
fn portion_sar_marking() {
    assert_canonical_equivalence(b"(S//SAR-EXAMPLE//NF)", MarkingType::Portion);
}

#[test]
fn portion_aea_rd() {
    assert_canonical_equivalence(b"(S//RD//NF)", MarkingType::Portion);
}

#[test]
fn portion_fgi_nato() {
    assert_canonical_equivalence(b"(S//FGI NATO)", MarkingType::Portion);
}

// =============================================================================
// Banner-form fixtures — different parser path; same canonicalization
// =============================================================================

#[test]
fn banner_unclassified_fouo() {
    assert_canonical_equivalence(b"UNCLASSIFIED//FOUO", MarkingType::Banner);
}

#[test]
fn banner_secret_noforn() {
    assert_canonical_equivalence(b"SECRET//NOFORN", MarkingType::Banner);
}

#[test]
fn banner_top_secret_rel_to_usa_nato() {
    assert_canonical_equivalence(b"TOP SECRET//REL TO USA, NATO", MarkingType::Banner);
}

#[test]
fn banner_secret_rd_noforn() {
    assert_canonical_equivalence(b"SECRET//RD//NOFORN", MarkingType::Banner);
}
