// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.2.C C6 — positive control proving [`marque_rules::Citation`] is
//! constructible only through structured construction.
//!
//! The compile-fail proofs that no free-form constructor exists
//! (`From<&str>`, `From<String>`, `Citation::from_str`) live as
//! `compile_fail` doctests on the [`marque_rules::Citation`] type
//! itself in `crates/rules/src/citation.rs` (lines 65–84). They run
//! via `cargo test --doc -p marque-rules`. This integration file pins
//! the complementary positive case: structured construction via
//! [`Citation::new`] and the three const-fn ergonomic helpers
//! ([`capco`], [`capco_section`], [`capco_table`]) works from outside
//! the `marque-rules` crate.
//!
//! Companion to `crates/rules/tests/message_no_freeform_ctor.rs`
//! (PR 3c.1 T033 — the [`Message`] equivalent). Together the two
//! files exercise the closed-construction surface of the diagnostic
//! emission types, which is the load-bearing G13 invariant for
//! `Diagnostic.message` / `Diagnostic.citation` per Constitution V
//! Principle V.
//!
//! # CAPCO §-citation verification
//!
//! Every literal §-reference below was re-verified against
//! `crates/capco/docs/CAPCO-2016.md` at PR 3c.2.C C6 authorship per
//! Constitution Principle VIII propagation rule:
//!
//! - §H.4 p61 — SCI grammar reminder per CAPCO-2016 §H.4 p61.
//! - §B.3 Table 2 p21 — caveated FD&R rule per CAPCO-2016 §B.3 Table
//!   2 p21 (project memory `project_capco_p20_caveated_definition`).
//! - §F p35 — Legacy Control Markings per CAPCO-2016 §F p35 (bare
//!   section letter; §F is the unnumbered legacy-markings section).

use core::num::{NonZeroU8, NonZeroU16};
use marque_rules::{
    AuthoritativeSource, Citation, SectionLetter, SectionRef, capco, capco_section, capco_table,
};

#[test]
fn citation_new_via_explicit_struct_construction() {
    // The base path: Citation::new + SectionRef builder. Verifies
    // the const-fn constructor is reachable from outside the crate.
    let c = Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()),
        NonZeroU16::new(61).unwrap(),
    );
    assert_eq!(format!("{c}"), "§H.4 p61");
}

#[test]
fn citation_new_via_capco_helper() {
    // The ergonomic path: capco() helper. ~28 chars vs ~120 for the
    // explicit struct form. Used at ~57 migrated call sites in
    // crates/capco/src/ and crates/engine/src/.
    let c = capco(SectionLetter::H, 4, 61);
    assert_eq!(format!("{c}"), "§H.4 p61");
}

#[test]
fn citation_new_via_capco_table_helper() {
    // The table-variant ergonomic path. Used for the §B.3 Table 2
    // p21 caveated FD&R rule.
    let c = capco_table(SectionLetter::B, 3, 2, 21);
    assert_eq!(format!("{c}"), "§B.3 Table 2 p21");
}

#[test]
fn citation_new_via_capco_section_helper() {
    // The bare-section ergonomic path. §F (Legacy Control Markings)
    // carries no numbered subsections per the citation-index, so
    // capco_section() is the canonical constructor for that family.
    let c = capco_section(SectionLetter::F, 35);
    assert_eq!(format!("{c}"), "§F p35");
}

#[test]
fn citation_is_copy_through_function_call() {
    // Citation: Copy is load-bearing — citations flow by value
    // through diagnostic emission, audit tracing, and lookup tables.
    // This test pins the Copy semantics by forcing a move-and-still-
    // usable pattern that would fail if Copy regressed to Clone.
    fn takes_citation(c: Citation) -> Citation {
        c
    }
    let c = capco(SectionLetter::H, 4, 61);
    let c2 = takes_citation(c);
    // `c` is still usable because Citation: Copy.
    assert_eq!(format!("{c}"), format!("{c2}"));
}

#[test]
fn citation_const_fn_evaluation_from_external_crate() {
    // All three helpers must be `const fn` so catalog rows in
    // crates/capco/src/scheme.rs can use them in `const` contexts.
    // This test pins const-fn evaluation from outside marque-rules.
    const SCI_GRAMMAR: Citation = capco(SectionLetter::H, 4, 61);
    const CAVEATED_FDR: Citation = capco_table(SectionLetter::B, 3, 2, 21);
    const LEGACY: Citation = capco_section(SectionLetter::F, 35);
    assert_eq!(format!("{SCI_GRAMMAR}"), "§H.4 p61");
    assert_eq!(format!("{CAVEATED_FDR}"), "§B.3 Table 2 p21");
    assert_eq!(format!("{LEGACY}"), "§F p35");
}
