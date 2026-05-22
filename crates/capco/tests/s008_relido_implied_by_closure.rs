// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! S008 — RELIDO implied by closure. #559 close-out PM decision
//! 2026-05-19: byte-surfacing twin of the lattice-layer
//! `CLOSURE_RELIDO_SCI` / `CLOSURE_RELIDO_US_CLASS` closures, mirroring
//! S007's text-layer pattern for `CLOSURE_REL_TO_USA_NATO`.
//!
//! End-to-end pinning through `Engine::lint`. The rule fires when:
//!
//! - Portion has a CAT_SCI marking (triggers `CLOSURE_RELIDO_SCI`), OR
//! - Portion is US collateral classified (triggers
//!   `CLOSURE_RELIDO_US_CLASS`), AND no FD&R suppressor blocks the
//!   closure, AND no RELIDO-incompatible marking (FGI / JOINT / NATO /
//!   SI-G / HCS-O / HCS-P / TK-{BLFH,IDIT,KAND}) is present, AND RELIDO
//!   is not already in the portion's dissem_us.
//!
//! Authority: CAPCO-2016 §H.8 p154 (RELIDO template) + §D.2 Table 3
//! rule 17 (FD&R defaults for caveated content). Verified against
//! `crates/capco/docs/CAPCO-2016.md` at the time of authorship.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Did `Engine::lint(input)` produce a `Diagnostic` whose `rule`
/// matches the S008 predicate-ID 2-tuple?
fn fires_s008(input: &[u8]) -> bool {
    let result = engine().lint(input);
    let s008 = marque_rules::RuleId::new("capco", "portion.dissem.relido-implied-by-closure");
    result.diagnostics.iter().any(|d| d.rule == s008)
}

// ---------------------------------------------------------------------------
// Positive cases — closure injects RELIDO; S008 should fire.
// ---------------------------------------------------------------------------

#[test]
fn fires_on_us_collateral_classified_portion_without_fdr() {
    // `(S)` — bare US Secret. `CLOSURE_RELIDO_US_CLASS` triggers on
    // US collateral classification; no FD&R suppressor present.
    assert!(
        fires_s008(b"(S)\n"),
        "S008 must fire on bare US-classified portion absent FD&R",
    );
}

#[test]
fn fires_on_sci_portion_without_suppressor() {
    // `(TS//SI)` — US Top Secret with SI control. `CLOSURE_RELIDO_SCI`
    // triggers on CAT_SCI presence; SI without compartment is not in
    // `FDR_OR_RELIDO_INCOMPAT` (SI-G specifically is suppressed, bare
    // SI is not).
    assert!(
        fires_s008(b"(TS//SI)\n"),
        "S008 must fire on US-classified portion with bare SI \
         (CLOSURE_RELIDO_SCI trigger)",
    );
}

// ---------------------------------------------------------------------------
// Negative cases — closure does NOT inject RELIDO; S008 must NOT fire.
// ---------------------------------------------------------------------------

#[test]
fn silent_when_relido_already_present() {
    // `(S//RELIDO)` — RELIDO already on the portion. No suggestion
    // needed. Early-return clause 2.
    assert!(
        !fires_s008(b"(S//RELIDO)\n"),
        "S008 must not fire when RELIDO is already present",
    );
}

#[test]
fn silent_when_noforn_present() {
    // `(S//NF)` — NOFORN suppresses RELIDO (FD&R dominator per §H.8
    // p145 supersession overlay).
    assert!(
        !fires_s008(b"(S//NF)\n"),
        "S008 must not fire when NOFORN is present (§H.8 p145 \
         supersession overlay strips RELIDO)",
    );
}

#[test]
fn silent_when_rel_to_present() {
    // `(S//REL TO USA, GBR)` — REL TO is an FD&R dominator that
    // suppresses the implicit-RELIDO closure default.
    assert!(
        !fires_s008(b"(S//REL TO USA, GBR)\n"),
        "S008 must not fire when REL TO is present (FD&R dominator \
         suppresses CLOSURE_RELIDO_US_CLASS)",
    );
}

#[test]
fn silent_when_display_only_present() {
    // `(S//DISPLAY ONLY GBR)` — DISPLAY ONLY is an FD&R dominator at
    // the lattice layer. Post-#618 the closure's
    // `satisfies(TOK_DISPLAY_ONLY)` predicate scans BOTH
    // `attrs.dissem_iter()` for `DissemControl::Displayonly` AND
    // `attrs.display_only_to` (the country-list axis the parser
    // routes the canonical wire form into). Pre-#618 the predicate
    // only checked `dissem_iter()`, missing the canonical wire
    // form, which forced a workaround at S008 clause 2b
    // (`!attrs.display_only_to.is_empty()` early-return). #618
    // widened the predicate, the workaround was retired, and S008
    // now reaches the canonical suppressor path through clause 3.
    // This test stays load-bearing as a regression guard — if the
    // underlying predicate ever drifts back, the test trips.
    assert!(
        !fires_s008(b"(S//DISPLAY ONLY GBR)\n"),
        "S008 must not fire when DISPLAY ONLY is present — \
         post-#618 the closure suppressor correctly recognizes \
         the `display_only_to` axis, so no RELIDO injection \
         occurs and S008 has nothing to suggest",
    );
}

#[test]
fn silent_when_rel_to_present_with_relido_already() {
    // `(S//REL TO USA, GBR/RELIDO)` — RELIDO is already present, so
    // S008 must not fire (clause 2: RELIDO already present). The
    // REL TO + RELIDO pairing is also a valid §H.8 p154 combination
    // ("OK alone or with REL TO"); this test pins the early-return
    // behavior, not the suppressor interaction.
    assert!(
        !fires_s008(b"(S//REL TO USA, GBR/RELIDO)\n"),
        "S008 must not fire when RELIDO is already present alongside \
         REL TO (§H.8 p154 permits the pairing)",
    );
}

#[test]
fn silent_on_unclassified_portion() {
    // `(U)` — Unclassified is not a CLOSURE_RELIDO_US_CLASS trigger
    // (the closure is gated on `TOK_US_COLLATERAL_CLASSIFIED` which
    // matches TS/S/C only).
    assert!(
        !fires_s008(b"(U)\n"),
        "S008 must not fire on Unclassified portions (closure trigger \
         is US collateral classified — TS/S/C only)",
    );
}

#[test]
fn silent_on_banner_marking() {
    // S008 is portion-only. A banner that lacks RELIDO does not get
    // an S008 suggestion — banner roll-up is the right surface and
    // flows automatically once portions carry RELIDO.
    //
    // `SECRET\n(S)` is a minimal classified-banner document; the
    // banner doesn't carry RELIDO but S008 must not propose adding
    // it to the banner.
    let result = engine().lint(b"SECRET\n(S)\n");
    let s008 = marque_rules::RuleId::new("capco", "portion.dissem.relido-implied-by-closure");
    let banner_s008 = result.diagnostics.iter().filter(|d| d.rule == s008).count();
    // The portion (S) may trigger S008, but the banner must not.
    // We check that S008 doesn't fire MORE than once (the portion's),
    // and if multiple match-fits exist this would catch the
    // banner-firing regression.
    assert!(
        banner_s008 <= 1,
        "S008 must fire at most once per document — once on the \
         portion if applicable, never on the banner. Got {banner_s008} \
         S008 diagnostics on `SECRET\\n(S)\\n`",
    );
}
