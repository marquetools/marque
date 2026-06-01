// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E007 (`capco:portion.metadata.x-shorthand-date-pattern`) coverage
//! pins ported from `crates/capco/src/_disabled_tests.rs` per issue
//! #722.
//!
//! # Source tests ported
//!
//! - `e007_fires_on_pattern_matched_x_shorthand_not_in_migration_table`
//!   — pattern-fallback path coverage: `25X2-` is not in the seed
//!   `MIGRATIONS` table but matches the X-shorthand fallback regex
//!   E007 owns.
//! - `e007_still_fires_on_migration_table_entries` — table-backed
//!   path coverage: `25X1-` IS in the seed `MIGRATIONS` table.
//!
//! Both paths emit at `Recognition::strict()` — strict-path fixes
//! collapse to 1.0; severity controls auto-apply, not confidence.
//!
//! Both paths produce a `text_correction` (E007 is a text-correction
//! rule per `Phase::Localized` plus the `Diagnostic::text_correction`
//! field — the new C001-shaped emission channel for byte-precise
//! known replacements, per `legacy-rule-id-map.md` §1).
//!
//! # Authority
//!
//! CAPCO-2016 §E.6 pp 33-34 (X-shorthand date-pattern migration to
//! canonical YYYYMMDD form). Re-verified against
//! `crates/capco/docs/CAPCO-2016.md` at authorship per Constitution
//! VIII.

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
// Pattern fallback (strict 1.0 — collapsed in PR A)
// ---------------------------------------------------------------------------

/// `25X2-` is NOT in the seed `MIGRATIONS` table. Before the pattern
/// fallback landed, this would have fallen through to E008. Now E007
/// owns it via the X-shorthand regex; the canonical replacement is
/// `25X2` (trailing `-` stripped). Strict-path fix proposals emit at
/// `Recognition::strict()`; severity controls auto-apply, not
/// confidence.
///
/// Authority: CAPCO-2016 §E.6 pp 33-34 (X-shorthand date-pattern
/// migration). Re-verified against `crates/capco/docs/CAPCO-2016.md`
/// per Constitution VIII.
#[test]
fn e007_fires_on_pattern_matched_x_shorthand_not_in_migration_table() {
    let diags = lint("SECRET//25X2-//NOFORN");
    let e007: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == E007_PREDICATE)
        .collect();
    assert_eq!(e007.len(), 1, "E007 must fire on `25X2-`: {diags:?}");
    let tc = e007[0]
        .text_correction
        .as_ref()
        .expect("E007 must carry a text_correction");
    assert_eq!(
        tc.replacement.as_str(),
        "25X2",
        "E007 pattern-fallback replacement must strip the trailing `-`; \
         got: {:?}",
        tc.replacement,
    );
    let conf = tc.confidence.combined();
    assert!(
        (conf - 1.0).abs() < f32::EPSILON,
        "E007 pattern-fallback confidence must be 1.0 (strict-path \
         collapse); got: {conf}",
    );
    // E008 must NOT also fire on the same span — the suppression
    // path in `text_handling.rs::is_x_shorthand_for_suppression` is
    // the load-bearing belt-and-suspenders here.
    let e008: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == E008_PREDICATE)
        .collect();
    assert!(
        e008.is_empty(),
        "E008 must be suppressed when E007 owns the pattern-matched \
         span: {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Migration-table-backed path (strict 1.0 — collapsed in PR A)
// ---------------------------------------------------------------------------

/// `25X1-` IS in the seed `MIGRATIONS` table (per
/// `crates/ism/build.rs`). E007 owns the table-backed path; the
/// replacement is `25X1` (table-anchored canonical form). Strict-path
/// fix proposals emit at `Recognition::strict()`.
///
/// Authority: CAPCO-2016 §E.6 pp 33-34. Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e007_fires_on_migration_table_entry() {
    let diags = diags_for("SECRET//25X1-//NOFORN", E007_PREDICATE);
    assert_eq!(
        diags.len(),
        1,
        "E007 must fire on table-backed `25X1-`: {diags:?}",
    );
    let tc = diags[0]
        .text_correction
        .as_ref()
        .expect("E007 must carry a text_correction");
    assert_eq!(
        tc.replacement.as_str(),
        "25X1",
        "E007 table-backed replacement must be `25X1`; got: {:?}",
        tc.replacement,
    );
    let conf = tc.confidence.combined();
    assert!(
        (conf - 1.0).abs() < f32::EPSILON,
        "E007 table-backed confidence must be 1.0 (strict-path \
         collapse); got: {conf}",
    );
}
