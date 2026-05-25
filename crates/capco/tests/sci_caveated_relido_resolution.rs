// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI / caveated / RELIDO resolution at portion scope.
//!
//! Authority: §H.4 (SI-G floor + ORCON implication), §B.3 Table 2 p21
//! (caveated classified → NOFORN), §H.8 p145 (NOFORN supersession),
//! §H.8 p154 (RELIDO ⊥ NOFORN), §H.8 p136 (ORCON supersedes RELIDO).
//!
//! These cases probe whether the page-projection resolution algebra
//! (closure + default_fill + page rewrites) is realized at PORTION
//! scope. It is not, except where a hand-written `Rule`/constraint
//! mirror exists — see RFC #799. The two `#[ignore]`d cases encode the
//! intended (spec) outcome and flip to passing once portion-level
//! realization lands.
//!
//! Bare SCI implies RELIDO (uncaveated); SCI compartments/sub-
//! compartments are the NOFORN reducers. `(S//SI)` ⇒ `(S//SI//RELIDO)`;
//! SI does NOT imply NOFORN.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use secrecy::ExposeSecret as _;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme constructs without rewrite cycles")
}

fn fix_once(source: &str) -> String {
    let result = engine().fix(source.as_bytes(), FixMode::Apply);
    String::from_utf8(result.source.expose_secret().to_vec()).expect("engine output is valid UTF-8")
}

// -------------------------------------------------------------------------
// SPEC, not yet realized at portion scope — RFC #799.
// ORCON-clears-RELIDO (§H.8 p136) and caveated⇒NOFORN exist only as
// page rewrites / default_fill; they emit no portion-scope diagnostic
// or fix. Verified: both inputs below currently produce ZERO diagnostics
// and are left unchanged. These flip to passing when portion-level
// realization lands.
// -------------------------------------------------------------------------

#[test]
#[ignore = "spec: portion-level ORCON-clears-RELIDO + caveated⇒NOFORN not yet realized — RFC #799"]
fn orcon_expels_relido_then_caveated_adds_noforn() {
    // ORCON supersedes RELIDO; with RELIDO gone the portion is caveated
    // (ORCON) with no FD&R dominator, so the caveated default adds NOFORN.
    assert_eq!(fix_once("(TS//SI-G//OC/RELIDO)"), "(TS//SI-G//OC/NF)");
}

#[test]
#[ignore = "spec: portion-level caveated⇒NOFORN not yet realized — RFC #799"]
fn si_g_compartment_orcon_drives_noforn() {
    // SI-G is excluded from the RELIDO default (its compartment template
    // drives ORCON); ORCON is caveated and drives NOFORN.
    assert_eq!(fix_once("(TS//SI-G//OC)"), "(TS//SI-G//OC/NF)");
}

// -------------------------------------------------------------------------
// Current correct behavior — live fixtures.
// -------------------------------------------------------------------------

#[test]
fn rel_to_is_retained_and_suppresses_noforn_default() {
    // REL TO is FD&R and is NOT expelled by ORCON, so it legitimately
    // suppresses the caveated NOFORN default. No change.
    let input = "(TS//SI-G//OC/REL TO USA, FVEY)";
    assert_eq!(fix_once(input), input);
}

#[test]
fn bare_sci_with_relido_is_canonical_unchanged() {
    // Bare SCI implies RELIDO; already canonical.
    let input = "(S//SI//RELIDO)";
    assert_eq!(fix_once(input), input);
}

#[test]
fn contrast_wired_edge_relido_conflicts_noforn_fixes_at_portion() {
    // The wired sibling: RELIDO ⊥ NOFORN (E054) HAS a portion-level
    // constraint mirror, unlike ORCON ⊥ RELIDO. Same §H.8 supersession
    // family; only this one was given a portion-scope realization.
    assert_eq!(fix_once("(S//SI//NF/RELIDO)"), "(S//SI//NF)");
}
