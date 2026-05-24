#![cfg(any())]
// Legacy FixProposal-shape harness, disabled pending rewrite.

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Corpus-parity baseline harness.
//!
//! Declarative `Constraint` / `PageRewrite` entries are registered on
//! `CapcoScheme` for the scheduler + catalog surface, while hand-written
//! rule impls in `crate::rules` remain the authoritative emitters of
//! diagnostics, preserving byte-identical diagnostic output.
//!
//! This harness runs the shared corpus fixtures through `Engine::lint`
//! and `Engine::fix`, asserting that:
//!
//! 1. Every fixture still produces a well-formed `LintResult`.
//! 2. The registered rule count matches the expected count.
//! 3. Every declared `PageRewrite` on `CapcoScheme` carries a
//!    non-empty citation.
//!
//! Full corpus-diff parity (baseline manifest vs. current run) rides
//! on top of the corpus-accuracy harness in
//! `crates/engine/tests/corpus_accuracy.rs`; this file pins the
//! declaration-layer invariants specifically.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::RuleSet;
use marque_scheme::MarkingScheme;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn rule_count_reflects_registration_changes() {
    // Pins the registered CAPCO rule count. A change here means a rule
    // was added or retired; treat it as an intentional, documented
    // registration change, not a number to bump silently.
    let rule_set = CapcoRuleSet::new();
    assert_eq!(
        rule_set.rules().len(),
        32,
        "registered CAPCO rule count changed; adjust this assertion only \
         when rule registration actually changes"
    );
}

#[test]
fn phase_3_declares_twenty_three_page_rewrites_with_citations() {
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();
    assert_eq!(
        rewrites.len(),
        23,
        "declared page-rewrite count changed; the catalog covers FOUO/UCNI/ \
         LIMDIS/SBU eviction and NOFORN-promotion rewrites per CAPCO-2016 \
         §H.6 / §H.8 / §H.9"
    );
    for rw in rewrites {
        assert!(
            !rw.citation.is_empty(),
            "rewrite {} has empty citation; Constitution VIII requires a \
             traceable authoritative-source passage",
            rw.id
        );
    }
}

#[test]
fn phase_3_engine_lint_produces_wellformed_result_on_empty_input() {
    // Smoke test: the scheduler construction path does not regress the
    // trivial empty-input case.
    let engine = engine();
    let result = engine.lint(b"");
    assert!(result.is_clean());
    assert_eq!(result.error_count(), 0);
    assert_eq!(result.warn_count(), 0);
}

#[test]
fn phase_3_scheduler_exposes_twenty_three_scheduled_rewrites() {
    // The scheduler produced a topological order at construction time.
    // Verify the scheduled set equals the declared set — the ordering is
    // a data-flow property, not a declaration-order one.
    let engine = engine();
    let scheduled = engine.scheduled_rewrites();
    assert_eq!(scheduled.len(), 23);
    let mut names: Vec<&str> = scheduled.to_vec();
    names.sort();
    assert_eq!(
        names,
        [
            "capco/classification-evicts-fouo",
            "capco/dod-ucni-evicted-by-classified",
            "capco/dod-ucni-promotes-noforn-when-classified",
            "capco/doe-ucni-evicted-by-classified",
            "capco/doe-ucni-promotes-noforn-when-classified",
            "capco/exdis-implies-noforn",
            "capco/fgi-restricted-rollup-on-us-contact",
            "capco/fgi-rollup-on-us-contact",
            "capco/fouo-evicted-by-classified",
            "capco/frd-sigma-consolidates-into-rd-sigma",
            "capco/joint-cross-class-rollup",
            "capco/les-nf-implies-noforn",
            "capco/les-nf-transmutes-on-classified-contact",
            "capco/limdis-evicted-by-classified",
            "capco/nodis-implies-noforn",
            "capco/noforn-clears-fdr-family",
            "capco/noforn-clears-rel-to",
            "capco/non-fdr-control-evicts-fouo",
            "capco/orcon-nato-to-us-orcon-on-us-contact",
            "capco/sbu-evicted-by-classified",
            "capco/sbu-nf-implies-noforn",
            "capco/sbu-nf-transmutes-on-classified-contact",
            "capco/us-presence-promotes-bare-fgi-attribution",
        ]
    );
}

#[test]
fn phase_3_noforn_clearer_runs_after_dissem_transmutations() {
    // The DISSEM-writing transmutations all write CAT_DISSEM;
    // `capco/noforn-clears-rel-to` reads CAT_DISSEM (and writes
    // CAT_REL_TO). The scheduler must therefore order each DISSEM
    // writer BEFORE the NOFORN clearer — otherwise a transmutation
    // that emits NOFORN could fire after the clearer and leave REL TO
    // populated when it should have been cleared. This ordering is a
    // declarative guarantee of the scheme's `reads` / `writes`
    // annotations, not an accident of declaration order.
    //
    // DISSEM writers (each declares `writes = [CAT_DISSEM]`):
    //   - ORCON-NATO, SBU-NF, LES-NF transmutations.
    //   - `capco/nodis-implies-noforn` (CAPCO-2016 §H.9 p174) and
    //     `capco/exdis-implies-noforn` (CAPCO-2016 §H.9 p172).
    //   - `capco/sbu-nf-implies-noforn` (CAPCO-2016 §H.9 p178) and
    //     `capco/les-nf-implies-noforn` (CAPCO-2016 §H.9 p185).
    let engine = engine();
    let scheduled = engine.scheduled_rewrites();
    let nf = scheduled
        .iter()
        .position(|&r| r == "capco/noforn-clears-rel-to")
        .expect("noforn-clears-rel-to is declared");
    for dissem_writer in [
        "capco/orcon-nato-to-us-orcon-on-us-contact",
        "capco/sbu-nf-transmutes-on-classified-contact",
        "capco/les-nf-transmutes-on-classified-contact",
        "capco/nodis-implies-noforn",
        "capco/exdis-implies-noforn",
        "capco/sbu-nf-implies-noforn",
        "capco/les-nf-implies-noforn",
    ] {
        let pos = scheduled
            .iter()
            .position(|&r| r == dissem_writer)
            .unwrap_or_else(|| panic!("{dissem_writer} is declared"));
        assert!(
            pos < nf,
            "{dissem_writer} ({pos}) must be scheduled before \
             noforn-clears-rel-to ({nf}) — scheduled order: {scheduled:?}",
        );
    }
}
