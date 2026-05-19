// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E021 — RD/FRD requires NOFORN with §123/§144 sharing-agreement
//! carve-out. #559 close-out PM decision 2026-05-19: renamed from
//! `aea-requires-noforn` to `rd-frd-requires-noforn` (the predicate
//! was always scoped to RD/FRD; the legacy "aea-" prefix was
//! misleading), severity dropped from `Fix` to `Warn`, and the
//! §123/§144 sharing-agreement carve-out is now byte-observable
//! (suppressed when REL TO or RELIDO already encodes a release
//! decision).
//!
//! Authority: CAPCO-2016 §H.6 p104 (RD entry, Relationship(s) to
//! Other Markings): "Is always used with NOFORN unless a sharing
//! agreement has been established per the Atomic Energy Act. (Ref.
//! Sections 123 and 144 of the Atomic Energy Act, and DoD
//! Instruction 5030.14.)" §H.6 p111 (FRD entry, same clause).
//! Re-verified against `crates/capco/docs/CAPCO-2016.md` at
//! authorship.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Severity;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Find the E021 diagnostic in `Engine::lint(input)` output, returning
/// `None` if E021 didn't fire on this input.
fn find_e021(input: &[u8]) -> Option<Severity> {
    engine()
        .lint(input)
        .diagnostics
        .into_iter()
        .find(|d| d.rule.as_str() == "E021")
        .map(|d| d.severity)
}

// ---------------------------------------------------------------------------
// Positive cases — predicate fires (RD/FRD without NOFORN or carve-out)
// ---------------------------------------------------------------------------

#[test]
fn fires_on_bare_rd_at_warn_severity() {
    // `SECRET//RD` — RD present, no NOFORN, no REL TO, no RELIDO. The
    // §123/§144 carve-out does not apply (no release decision on the
    // marking). #559 close-out: severity is Warn (was Fix pre-rename).
    let severity = find_e021(b"SECRET//RD");
    assert_eq!(
        severity,
        Some(Severity::Warn),
        "E021 must fire at Severity::Warn on `SECRET//RD` per #559 \
         close-out rename + severity drop",
    );
}

#[test]
fn fires_on_bare_frd_at_warn_severity() {
    // `SECRET//FRD` — same as RD case. §H.6 p111 FRD entry carries
    // the same "Is always used with NOFORN unless a sharing agreement"
    // clause.
    let severity = find_e021(b"SECRET//FRD");
    assert_eq!(
        severity,
        Some(Severity::Warn),
        "E021 must fire at Severity::Warn on `SECRET//FRD` per §H.6 p111",
    );
}

// ---------------------------------------------------------------------------
// Carve-out — REL TO or RELIDO present (byte-observable §123/§144 evidence)
// ---------------------------------------------------------------------------

#[test]
fn silent_when_rel_to_present() {
    // `SECRET//RD//REL TO USA, GBR` — REL TO indicates the author has
    // made a release decision under some sharing instrument. #559
    // close-out's pragmatic substitute for the §123/§144 documentary
    // carve-out: suppress the warning when REL TO is present on the
    // portion. Without this carve-out the warning would noisily fire
    // on every release-authorized RD marking.
    assert!(
        find_e021(b"SECRET//RD//REL TO USA, GBR").is_none(),
        "E021 must suppress on `SECRET//RD//REL TO USA, GBR` per the \
         §123/§144 byte-observable carve-out",
    );
}

#[test]
fn silent_when_relido_present() {
    // `SECRET//RD//RELIDO` — RELIDO indicates a release-by-IDO
    // decision, which is the §123/§144 sharing-agreement signal at
    // byte level.
    assert!(
        find_e021(b"SECRET//RD//RELIDO").is_none(),
        "E021 must suppress on `SECRET//RD//RELIDO` per the §123/§144 \
         byte-observable carve-out",
    );
}

#[test]
fn silent_when_noforn_present() {
    // `SECRET//RD//NF` — NOFORN already on the portion satisfies the
    // §H.6 p104 requirement directly. The pre-existing early-return
    // clause (predates #559).
    assert!(
        find_e021(b"SECRET//RD//NF").is_none(),
        "E021 must suppress when NOFORN is already present",
    );
}

// ---------------------------------------------------------------------------
// Scope guards — TFNI and UCNI excluded from the predicate
// ---------------------------------------------------------------------------

#[test]
fn silent_on_tfni_alone() {
    // `SECRET//TFNI` — TFNI is intentionally excluded from the
    // predicate scope per §H.6 p120 ("May only be used with TOP
    // SECRET, SECRET, or CONFIDENTIAL"; no "Requires NOFORN"
    // clause). E070 covers FRD>TFNI separately.
    assert!(
        find_e021(b"SECRET//TFNI").is_none(),
        "E021 must NOT fire on TFNI alone — TFNI is out of scope per \
         §H.6 p120 (silent on NOFORN); E070 covers the FRD>TFNI leg",
    );
}
