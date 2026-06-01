// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E008 (`capco:marking.metadata.unrecognized-token`) suppression
//! matrix pins ported from `crates/capco/src/_disabled_tests.rs` per
//! issue #722.
//!
//! # Source tests ported
//!
//! - `e008_no_fix_offered` — E008 never carries a fix payload.
//! - `e008_suppressed_on_migration_backed_unknown` — `25X1-` is a
//!   table-backed X-shorthand entry: E007 owns + E008 suppresses,
//!   belt-and-suspenders (the paired E007 firing check prevents a
//!   silent drop if the migration lookup ever breaks).
//! - `e008_suppressed_on_pattern_matched_x_shorthand` — `25X9-` is
//!   the pattern-fallback X-shorthand case: same suppression
//!   discipline + paired E007 firing check.
//! - `e008_fires_on_malformed_first_sar_with_empty_program` — `SAR-`
//!   alone (no program identifier) fails SAR grammar; previous SAR
//!   rules can't run without a successful first SAR parse, so E008
//!   is the only rule that can surface the malformation.
//! - `e008_fires_on_malformed_first_spelled_sar_with_empty_program`
//!   — Spelled-out variant: `SPECIAL ACCESS REQUIRED-` with no
//!   program must not be silently dropped either.
//! - `e008_fires_on_malformed_sci_shape` — `SI-` (dangling hyphen)
//!   is SCI-shaped but invalid; the structural subparser rejects
//!   it, so it falls through as Unknown and E008 correctly fires.
//!
//! # Authority
//!
//! CAPCO-2016 §G.1 p36 (Register-closed-set token authority); §E.6
//! pp 33-34 (X-shorthand migration patterns owned by E007); §H.5 pp
//! 99-101 (SAR grammar); §H.4 p61 (SCI grammar). Each citation
//! re-verified against `crates/capco/docs/CAPCO-2016.md` at
//! authorship per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_rules::Diagnostic;

const E007_PREDICATE: &str = "portion.metadata.x-shorthand-date-pattern";
const E008_PREDICATE: &str = "marking.metadata.unrecognized-token";

fn engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

fn diags_for(source: &str, predicate: &str) -> Vec<Diagnostic<CapcoScheme>> {
    lint(source)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == predicate)
        .collect()
}

// ---------------------------------------------------------------------------
// E008 never carries a fix
// ---------------------------------------------------------------------------

/// E008 is a pure-Error rule with no remediation channel — neither a
/// `fix` (FixIntent) nor a `text_correction`
/// payload can be attached. The unrecognized-token surface is
/// classifier-judgment territory; the engine refuses to guess.
#[test]
fn e008_carries_no_fix_or_text_correction() {
    let diags = diags_for("SECRET//XYZZY//NOFORN", E008_PREDICATE);
    let e008 = diags.first().expect("E008 must fire on unknown token");
    assert!(
        e008.fix.is_none() && e008.text_correction.is_none(),
        "E008 must propose no fix and no text_correction; \
         got fix={:?}, text_correction={:?}",
        e008.fix,
        e008.text_correction,
    );
}

// ---------------------------------------------------------------------------
// Suppression matrix — E007 owns X-shorthand; E008 steps aside
// ---------------------------------------------------------------------------

/// `25X1-` is an Unknown token the seed `MIGRATIONS` table captures.
/// E007 owns the X-shorthand path; E008 MUST step aside. The paired
/// E007 firing check is belt-and-suspenders — without it a future
/// change that breaks E007's migration lookup could produce a silent
/// suppression where E008 also stays quiet.
///
/// Authority: CAPCO-2016 §E.6 pp 33-34 (E007 X-shorthand ownership).
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` per
/// Constitution VIII.
#[test]
fn e008_suppressed_on_migration_backed_x_shorthand() {
    let diags = lint("SECRET//25X1-//NOFORN");
    let e008: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == E008_PREDICATE)
        .collect();
    let e007: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == E007_PREDICATE)
        .collect();
    assert!(
        e008.is_empty(),
        "E008 must be suppressed for migration-backed X-shorthand \
         (E007 owns this path): {diags:?}",
    );
    assert!(
        !e007.is_empty(),
        "E007 must fire for migration-backed X-shorthand — otherwise \
         the suppression is a silent drop: {diags:?}",
    );
}

/// `25X9-` is NOT in the seed `MIGRATIONS` table but matches the
/// pattern-fallback X-shorthand regex. E008 MUST still step aside
/// (suppression path 2 in the rule doc); paired E007 firing check
/// prevents a silent drop.
#[test]
fn e008_suppressed_on_pattern_matched_x_shorthand() {
    let diags = lint("SECRET//25X9-//NOFORN");
    let e008: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == E008_PREDICATE)
        .collect();
    let e007: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == E007_PREDICATE)
        .collect();
    assert!(
        e008.is_empty(),
        "E008 must be suppressed for pattern-matched X-shorthand even \
         when not in seed MIGRATIONS (E007 owns): {diags:?}",
    );
    assert!(
        !e007.is_empty(),
        "E007 must fire for pattern-matched X-shorthand — otherwise \
         suppression is a silent drop: {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// E008 fires on malformed SAR / SCI shapes (no silent drops)
// ---------------------------------------------------------------------------

/// `SAR-` alone (no program identifier) fails SAR grammar — the
/// parser does not produce a `SarMarking`, so `attrs.sar_markings`
/// stays `None` and SAR-specific rules return early at their
/// `attrs.sar_markings.is_none()` guard. E008 MUST fire to surface
/// the malformation; an earlier version of E008's suppression matched
/// on prefix only, silently dropping `SAR-`. Tightening the
/// suppression to require `attrs.sar_markings.is_some()` AND a
/// non-empty suffix restores the E008 error.
///
/// Authority: CAPCO-2016 §H.5 pp 99-101 (SAR grammar). Re-verified
/// against `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e008_fires_on_malformed_first_sar_with_empty_program() {
    let diags = diags_for("SECRET//SAR-//NOFORN", E008_PREDICATE);
    assert!(
        !diags.is_empty(),
        "E008 must fire on malformed first SAR (empty program): {diags:?}",
    );
}

/// Spelled-out variant of the SAR malformation: `SPECIAL ACCESS
/// REQUIRED-` with no program must not be silently dropped either.
#[test]
fn e008_fires_on_malformed_first_spelled_sar_with_empty_program() {
    let diags = diags_for("SECRET//SPECIAL ACCESS REQUIRED-//NOFORN", E008_PREDICATE);
    assert!(
        !diags.is_empty(),
        "E008 must fire on malformed first `SPECIAL ACCESS REQUIRED-` \
         (empty program): {diags:?}",
    );
}

/// `SI-` is SCI-shaped but invalid (dangling hyphen). The structural
/// subparser rejects it, so it falls through as `Unknown` and E008
/// correctly fires — no silent suppression.
///
/// Authority: CAPCO-2016 §H.4 p61 + §A.6 pp 15-17 (SCI grammar).
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` per
/// Constitution VIII.
#[test]
fn e008_fires_on_malformed_sci_shape() {
    let diags = diags_for("SECRET//SI-//NOFORN", E008_PREDICATE);
    assert!(
        !diags.is_empty(),
        "E008 must fire on malformed SCI-shaped token (`SI-` with \
         dangling hyphen): {diags:?}",
    );
}
