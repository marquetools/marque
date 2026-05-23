// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 9c.1 T134 — E066 (legacy NATO compound text re-marking) end-to-end
//! firing + rewrite-text coverage.
//!
//! E066 is the autofix rule introduced in Commit 6 of PR 9c.1. It
//! fires whenever the parser canonicalized a legacy NATO compound text
//! (eight portion forms + five banner forms) into bare class + AEA/SCI
//! companion, and emits a `ReplacementIntent::Recanonicalize` fix at
//! `Confidence::strict(1.0)` so the engine auto-applies it.
//!
//! This file covers all thirteen legacy patterns plus two negative
//! cases (bare canonical NATO + bare US classification). Per pattern
//! the assertions pin three properties:
//!
//! 1. E066 fires in `Engine::lint` output for the legacy input.
//! 2. The emitted `FixIntent`'s `replacement` is a
//!    `ReplacementIntent::Recanonicalize` payload with
//!    `Confidence::strict(1.0)`.
//! 3. `Engine::fix(..., FixMode::Apply)` produces the canonical
//!    multi-block form as a byte-identical output.
//!
//! The rewrite outputs were verified against the live engine before
//! being pinned here (PR 9c.1 R0 probe). Any future change to the
//! renderer that breaks one of the canonical forms surfaces here
//! immediately.
//!
//! # Authority
//!
//! - CAPCO-2016 §G.1 Table 4 p37 (portion-form + banner-title columns
//!   for all thirteen legacy compounds).
//! - CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking — registers
//!   ATOMAL / BOHEMIA / BALK as standalone control markings).
//! - CAPCO-2016 §H.7 p122 (ATOMAL worked example, AEA-block placement).
//! - CAPCO-2016 §H.7 p127 (BOHEMIA worked example, SCI-block placement).
//! - CAPCO-2016 §A.6 p15-17 (multi-block portion / banner grammar — the
//!   canonical target shape).
//!
//! # Spec linkage
//!
//! Reviewer fix-up under PR 9c.1 R0 (Commit 10) — replaces the empty
//! file fabricated in Commit 9 with substantive assertions.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::Confidence;
use marque_scheme::{RecanonScope, ReplacementIntent};
use secrecy::ExposeSecret as _;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Asserts the three E066 invariants for a single legacy input:
/// 1. E066 fires (lint).
/// 2. The diagnostic's `fix_intent` is a `Recanonicalize` at
///    confidence 1.0 (the strict fix-emit shape).
/// 3. `Engine::fix` produces the expected canonical byte string.
///
/// `scope` is the expected `RecanonScope` (Portion for `(..)` inputs,
/// Page for banner inputs).
fn assert_e066_fires_and_rewrites_to(
    input: &str,
    expected_output: &str,
    expected_scope: RecanonScope,
) {
    let eng = engine();

    // Property 1: E066 fires.
    let lint = eng.lint(input.as_bytes());
    let e066_diag = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "marking.recanonicalize.legacy-nato-compound")
        .unwrap_or_else(|| {
            panic!(
                "E066 must fire on {input:?}; diagnostics: {:?}",
                lint.diagnostics
                    .iter()
                    .map(|d| d.rule.predicate_id())
                    .collect::<Vec<_>>(),
            )
        });

    // Property 2: fix is Recanonicalize @ confidence 1.0.
    let fix_intent = e066_diag
        .fix
        .as_ref()
        .expect("E066 must carry a FixIntent (FixIntent shape, not legacy FixProposal)");
    match fix_intent.replacement {
        ReplacementIntent::Recanonicalize { scope } => {
            assert_eq!(
                scope, expected_scope,
                "E066 Recanonicalize scope mismatch on {input:?}: \
                 expected {expected_scope:?}, got {scope:?}",
            );
        }
        ref other => panic!("E066 FixIntent must be Recanonicalize on {input:?}; got: {other:?}"),
    }
    let expected_conf = Confidence::strict(1.0);
    assert_eq!(
        fix_intent.confidence, expected_conf,
        "E066 confidence must be strict(1.0) on {input:?}; got: {:?}",
        fix_intent.confidence,
    );

    // Property 3: fix produces the canonical multi-block form.
    let result = eng.fix(input.as_bytes(), FixMode::Apply);
    let actual = String::from_utf8(result.source.expose_secret().to_vec())
        .unwrap_or_else(|e| panic!("Engine::fix produced non-UTF8 output on {input:?}: {e}"));
    assert_eq!(
        actual,
        expected_output,
        "E066 rewrite mismatch on {input:?}: expected {expected_output:?}, \
         got {actual:?}; applied: {:?}",
        result
            .applied_fixes()
            .map(|af| af.rule.predicate_id())
            .collect::<Vec<_>>(),
    );

    // E066 must appear in the applied set (sanity check on the
    // promotion path).
    assert!(
        result
            .applied_fixes()
            .any(|af| af.rule.predicate_id() == "marking.recanonicalize.legacy-nato-compound"),
        "E066 must appear in applied fixes on {input:?}; applied: {:?}",
        result
            .applied_fixes()
            .map(|af| af.rule.predicate_id())
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// Portion-form patterns (8 of 13)
// ---------------------------------------------------------------------------

/// `(//CTSA)` → `(//CTS//ATOMAL)`.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p37 (COSMIC TOP SECRET ATOMAL
/// portion); §G.2 p40 (ATOMAL ARH); §H.7 p122 (AEA placement);
/// §A.6 p15-17 (canonical multi-block target).
#[test]
fn e066_ctsa_portion_to_cts_atomal() {
    assert_e066_fires_and_rewrites_to("(//CTSA)", "(//CTS//ATOMAL)", RecanonScope::Portion);
}

/// `(//CTS-A)` → `(//CTS//ATOMAL)`.
///
/// Authority: §G.1 Table 4 p37; §G.2 p40; §H.7 p122; §A.6 p15-17.
#[test]
fn e066_cts_a_portion_to_cts_atomal() {
    assert_e066_fires_and_rewrites_to("(//CTS-A)", "(//CTS//ATOMAL)", RecanonScope::Portion);
}

/// `(//NSAT)` → `(//NS//ATOMAL)`.
///
/// Authority: §G.1 Table 4 p37 (NATO SECRET ATOMAL portion); §G.2 p40;
/// §H.7 p122; §A.6 p15-17.
#[test]
fn e066_nsat_portion_to_ns_atomal() {
    assert_e066_fires_and_rewrites_to("(//NSAT)", "(//NS//ATOMAL)", RecanonScope::Portion);
}

/// `(//NS-A)` → `(//NS//ATOMAL)`.
///
/// Authority: §G.1 Table 4 p37; §G.2 p40; §H.7 p122; §A.6 p15-17.
#[test]
fn e066_ns_a_portion_to_ns_atomal() {
    assert_e066_fires_and_rewrites_to("(//NS-A)", "(//NS//ATOMAL)", RecanonScope::Portion);
}

/// `(//NCA)` → `(//NC//ATOMAL)`.
///
/// Authority: §G.1 Table 4 p37 (NATO CONFIDENTIAL ATOMAL portion);
/// §G.2 p40; §H.7 p122; §A.6 p15-17.
#[test]
fn e066_nca_portion_to_nc_atomal() {
    assert_e066_fires_and_rewrites_to("(//NCA)", "(//NC//ATOMAL)", RecanonScope::Portion);
}

/// `(//NC-A)` → `(//NC//ATOMAL)`.
///
/// Authority: §G.1 Table 4 p37; §G.2 p40; §H.7 p122; §A.6 p15-17.
#[test]
fn e066_nc_a_portion_to_nc_atomal() {
    assert_e066_fires_and_rewrites_to("(//NC-A)", "(//NC//ATOMAL)", RecanonScope::Portion);
}

/// `(//CTS-B)` → `(//CTS//BOHEMIA)`.
///
/// Authority: §G.1 Table 4 p37 (COSMIC TOP SECRET BOHEMIA portion);
/// §G.2 p40 + §H.7 p127 (BOHEMIA in SCI block); §A.6 p15-17.
#[test]
fn e066_cts_b_portion_to_cts_bohemia() {
    assert_e066_fires_and_rewrites_to("(//CTS-B)", "(//CTS//BOHEMIA)", RecanonScope::Portion);
}

/// `(//CTS-BALK)` → `(//CTS//BALK)`.
///
/// Authority: §G.1 Table 4 p37 (COSMIC TOP SECRET BALK portion);
/// §G.2 p40; §A.6 p15-17.
#[test]
fn e066_cts_balk_portion_to_cts_balk() {
    assert_e066_fires_and_rewrites_to("(//CTS-BALK)", "(//CTS//BALK)", RecanonScope::Portion);
}

// ---------------------------------------------------------------------------
// Banner-form patterns (5 of 13)
// ---------------------------------------------------------------------------

/// `//COSMIC TOP SECRET ATOMAL` → `//COSMIC TOP SECRET//ATOMAL`.
///
/// Authority: §G.1 Table 4 p37 (banner-title column); §G.2 p40;
/// §H.7 p122; §A.6 p15-17.
#[test]
fn e066_banner_cosmic_top_secret_atomal() {
    assert_e066_fires_and_rewrites_to(
        "//COSMIC TOP SECRET ATOMAL",
        "//COSMIC TOP SECRET//ATOMAL",
        RecanonScope::Page,
    );
}

/// `//COSMIC TOP SECRET-BOHEMIA` → `//COSMIC TOP SECRET//BOHEMIA`.
///
/// Authority: §G.1 Table 4 p37; §G.2 p40 + §H.7 p127; §A.6 p15-17.
#[test]
fn e066_banner_cosmic_top_secret_bohemia() {
    assert_e066_fires_and_rewrites_to(
        "//COSMIC TOP SECRET-BOHEMIA",
        "//COSMIC TOP SECRET//BOHEMIA",
        RecanonScope::Page,
    );
}

/// `//COSMIC TOP SECRET-BALK` → `//COSMIC TOP SECRET//BALK`.
///
/// Authority: §G.1 Table 4 p37; §G.2 p40; §A.6 p15-17.
#[test]
fn e066_banner_cosmic_top_secret_balk() {
    assert_e066_fires_and_rewrites_to(
        "//COSMIC TOP SECRET-BALK",
        "//COSMIC TOP SECRET//BALK",
        RecanonScope::Page,
    );
}

/// `//NATO SECRET ATOMAL` → `//NATO SECRET//ATOMAL`.
///
/// Authority: §G.1 Table 4 p37; §G.2 p40; §H.7 p122; §A.6 p15-17.
#[test]
fn e066_banner_nato_secret_atomal() {
    assert_e066_fires_and_rewrites_to(
        "//NATO SECRET ATOMAL",
        "//NATO SECRET//ATOMAL",
        RecanonScope::Page,
    );
}

/// `//NATO CONFIDENTIAL ATOMAL` → `//NATO CONFIDENTIAL//ATOMAL`.
///
/// Authority: §G.1 Table 4 p37; §G.2 p40; §H.7 p122; §A.6 p15-17.
#[test]
fn e066_banner_nato_confidential_atomal() {
    assert_e066_fires_and_rewrites_to(
        "//NATO CONFIDENTIAL ATOMAL",
        "//NATO CONFIDENTIAL//ATOMAL",
        RecanonScope::Page,
    );
}

// ---------------------------------------------------------------------------
// Negative cases — E066 must NOT fire on canonical / unrelated inputs.
// ---------------------------------------------------------------------------

/// Bare NATO classification with no compound suffix — E066 must NOT
/// fire. The rule predicate gates on the canonicalized companion
/// being non-empty, which only happens for the thirteen legacy patterns.
///
/// Authority: §A.6 p15-17 — bare canonical forms are the destination
/// state E066 re-marks toward, not a trigger condition.
#[test]
fn e066_does_not_fire_on_bare_cts_portion() {
    let eng = engine();
    let source = b"(//CTS)";
    let lint = eng.lint(source);

    assert!(
        lint.diagnostics
            .iter()
            .all(|d| d.rule.predicate_id() != "marking.recanonicalize.legacy-nato-compound"),
        "E066 must NOT fire on bare canonical (//CTS); diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>(),
    );

    // And the fix output is byte-identical (modulo unrelated rules).
    let result = eng.fix(source, FixMode::Apply);
    assert!(
        result
            .applied_fixes()
            .all(|af| af.rule.predicate_id() != "marking.recanonicalize.legacy-nato-compound"),
        "E066 must NOT appear in applied fixes for bare (//CTS); applied: {:?}",
        result
            .applied_fixes()
            .map(|af| af.rule.predicate_id())
            .collect::<Vec<_>>(),
    );
}

/// Bare US classification — E066 must NOT fire. The rule's NATO-axis
/// gate (`matches!(classification, Some(MarkingClassification::Nato(_)))`)
/// short-circuits before any further checks.
///
/// Authority: §A.6 p15-17 — US classification is structurally distinct
/// from NATO and is the routing the NATO-axis gate excludes.
#[test]
fn e066_does_not_fire_on_bare_us_portion() {
    let eng = engine();
    let source = b"(S)";
    let lint = eng.lint(source);

    assert!(
        lint.diagnostics
            .iter()
            .all(|d| d.rule.predicate_id() != "marking.recanonicalize.legacy-nato-compound"),
        "E066 must NOT fire on US-class (S); diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// PR 9c.1 R1 Copilot #3 — companion-write dedup.
//
// When the parser's legacy NATO compound canonicalization
// (`crates/core/src/parser.rs` idx == 1 non-US classification block)
// and its canonical-form AEA / SCI block recognition both write the
// same axis value, the parser used to push the value twice and the
// canonical renderer emitted `ATOMAL/ATOMAL` / `BOHEMIA/BOHEMIA` —
// breaking E066's byte-identical `Recanonicalize` fix-text contract.
//
// `marque_ism::dedup_companions` (called from `Parser::parse` after
// `attribute_dissems`) collapses the duplicate axis entries. These
// four tests pin the post-dedup behavior at the engine boundary:
// the canonical render carries each NATO SAP / ATOMAL exactly once,
// regardless of how many source-text paths claimed it.
//
// All four inputs were verified to produce duplicated output
// (`ATOMAL/ATOMAL`, `BOHEMIA/BOHEMIA`, `BALK/BALK`,
// `ATOMAL/ATOMAL`) when the dedup pass was temporarily disabled
// during R1 — the tests would fail without `dedup_companions`.
//
// Authority:
// - CAPCO-2016 §G.1 Table 4 p37 (legacy compound text — the source
//   side of the duplicate write).
// - CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking —
//   ATOMAL / BOHEMIA / BALK are standalone control markings; one
//   axis entry per marking).
// - CAPCO-2016 §H.7 p122 (ATOMAL placement in the AEA block).
// - CAPCO-2016 §H.7 p127 (BOHEMIA placement in the SCI block).
// ---------------------------------------------------------------------------

/// `(//NSAT//ATOMAL)` — legacy `NSAT` + explicit canonical `ATOMAL`
/// block. Both paths write [`AeaMarking::Atomal`]; the dedup pass
/// must collapse to one entry. The canonical render carries
/// `ATOMAL` exactly once.
///
/// Without `dedup_companions` the engine emits `(//NS//ATOMAL/ATOMAL)`
/// — duplicate token, broken `Recanonicalize` contract. With the
/// pass the output is `(//NS//ATOMAL)` and `aea_markings.len() == 1`.
///
/// Authority: §G.1 Table 4 p37 + §G.2 p40 + §H.7 p122.
#[test]
fn dedup_nsat_atomal_collapses_duplicate_aea() {
    assert_e066_fires_and_rewrites_to("(//NSAT//ATOMAL)", "(//NS//ATOMAL)", RecanonScope::Portion);
}

/// `(//CTS-B//BOHEMIA)` — legacy `CTS-B` + explicit canonical
/// `BOHEMIA` block. Both paths write
/// [`SciControlSystem::NatoSap`]`(NatoSap::Bohemia)`; dedup must
/// collapse to one entry. Canonical render carries `BOHEMIA` once.
///
/// Without `dedup_companions` the engine emits
/// `(//CTS//BOHEMIA/BOHEMIA)`. With the pass the output is
/// `(//CTS//BOHEMIA)`.
///
/// Authority: §G.1 Table 4 p37 + §G.2 p40 + §H.7 p127.
#[test]
fn dedup_ctsb_bohemia_collapses_duplicate_sci() {
    assert_e066_fires_and_rewrites_to(
        "(//CTS-B//BOHEMIA)",
        "(//CTS//BOHEMIA)",
        RecanonScope::Portion,
    );
}

/// `(//CTS-BALK//BALK)` — legacy `CTS-BALK` + explicit canonical
/// `BALK` block. Both paths write `NatoSap::Balk`; dedup must
/// collapse to one entry. Canonical render carries `BALK` once.
///
/// Without `dedup_companions` the engine emits
/// `(//CTS//BALK/BALK)`. With the pass the output is
/// `(//CTS//BALK)`.
///
/// Authority: §G.1 Table 4 p37 + §G.2 p40.
#[test]
fn dedup_ctsbalk_balk_collapses_duplicate_sci() {
    assert_e066_fires_and_rewrites_to("(//CTS-BALK//BALK)", "(//CTS//BALK)", RecanonScope::Portion);
}

/// `//NATO SECRET ATOMAL//ATOMAL//` — banner-form legacy compound
/// `NATO SECRET ATOMAL` + explicit canonical `ATOMAL` block.
/// Confirms the dedup invariant holds for banner-form inputs as
/// well as portion-form.
///
/// Without `dedup_companions` the engine emits
/// `//NATO SECRET//ATOMAL/ATOMAL`. With the pass the output is
/// `//NATO SECRET//ATOMAL`.
///
/// Authority: §G.1 Table 4 p37 (banner-title column) + §G.2 p40 +
/// §H.7 p122.
#[test]
fn dedup_banner_nato_secret_atomal_collapses_duplicate_aea() {
    assert_e066_fires_and_rewrites_to(
        "//NATO SECRET ATOMAL//ATOMAL//",
        "//NATO SECRET//ATOMAL",
        RecanonScope::Page,
    );
}
