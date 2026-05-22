// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 5 (006 T059a) — per-evaluator unit tests for E068 + E069.
//!
//! Drives small synthetic page shapes through `Engine::lint` and
//! asserts the rule-emission behavior of the two new banner-rollup
//! catalog rows:
//!
//! - `evaluate_classification_banner_rollup` (E068)
//! - `evaluate_fgi_marker_banner_rollup` (E069)
//!
//! ## Test architecture
//!
//! The evaluators are crate-private `fn` items; they're exercised
//! through the `Engine` dispatch path (the only production path that
//! invokes them). Each test arranges a small input string, lints, and
//! asserts the presence (or absence) of the target rule ID across the
//! diagnostic stream. Per-row severity Error is implicit (the engine
//! emits at the row's `severity` field; no fix is attached because
//! cross-axis byte-positioning a classification/FGI block from rule
//! context alone is unsafe — see the row doc-comments in `rules.rs`).
//!
//! ## AAA structure
//!
//! Every test follows Arrange → Act → Assert.
//!
//! ## Branch coverage (>80% on the two new evaluators)
//!
//! E068 has 6 match arms in the `(observed, projected)` evaluator:
//!   1. `(None, None)` — agreement (identity case; not directly
//!      tested because no banner candidate emits in this shape under
//!      current scanner output).
//!   2. `(None, Some(_))` — banner missing classification.
//!      Architecturally unreachable today (scanner always emits a
//!      classification token for banner candidates); retained as a
//!      defensive guard for future scanner changes.
//!   3. `(Some(_), None)` — banner over-claims classification.
//!   4. `(Some(a), Some(b))` with `effective_level()` disagreement.
//!   5. `(Some(a), Some(b))` with same level but different variant
//!      kind.
//!   6. `(Some(a), Some(b))` agreeing — no diagnostic.
//!
//! E069 has 5 reachable branches:
//!   1. `(None, None)` — agreement.
//!   2. `(None, Some(_))` — banner missing FGI marker.
//!   3. `(Some(_), None)` — banner over-claims FGI.
//!   4. `(Some(SourceConcealed), Some(Acknowledged{..}))` or vice
//!      versa — variant mismatch.
//!   5. `(Some(Acknowledged{a}), Some(Acknowledged{b}))` with
//!      different country lists.

use marque_capco::{CapcoRuleSet, scheme::CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use std::collections::HashSet;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Lint the given source and return the set of emitted rule IDs.
fn observed_rule_ids(source: &str) -> HashSet<String> {
    let result = engine().lint(source.as_bytes());
    result
        .diagnostics
        .iter()
        .map(|d| d.rule.predicate_id().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// E068 branch coverage
// ---------------------------------------------------------------------------

/// E068 fires on a Some-Some `effective_level` mismatch where the
/// page projection is TopSecret (driven by a US TS portion + a
/// foreign TS portion) and the banner observes Secret.
///
/// **Naming note**: this test was originally named
/// `e068_fires_when_banner_missing_classification_and_page_has_one`,
/// implying it exercised the `(None, Some(_))` arm. It does not —
/// the source string carries a classified banner
/// (`SECRET//FGI DEU//NOFORN`), so `attrs.classification` is
/// `Some(Us(Secret))`. The arrangement actually drives the
/// Some-Some level-mismatch branch (arm 4 of the E068 match), which
/// uses the same evaluator path as the architecturally unreachable
/// `(None, Some(_))` arm. The `(None, Some(_))` arm is retained in
/// the evaluator as a defensive guard for future scanner changes
/// (today every banner candidate the scanner emits carries a
/// classification token); there is no reachable test shape for it
/// via current scanner output.
///
/// Authority: CAPCO-2016 §H.7 pp123-125 banner roll-up grammar +
/// §H.7 p129 worked example.
#[test]
fn e068_fires_on_topsecret_projection_with_secret_banner() {
    // Arrange: TS US portion + TS FGI DEU portion → projection
    // TopSecret; banner observes Secret.
    let source = "SECRET//FGI DEU//NOFORN
(TS//NF)
(//DEU TS//REL TO USA, DEU)
SECRET//FGI DEU//NOFORN
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert: E068 fires because portions roll up to TopSecret but
    // banner observes Secret (Some-Some effective_level mismatch
    // branch — arm 4 of the E068 match).
    assert!(
        observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 must fire on TopSecret-projected page with Secret \
         banner; observed = {observed:?}"
    );
}

/// E068 fires when banner classification level disagrees with the
/// projected page state. The load-bearing branch:
/// `(Some(observed), Some(projected))` with
/// `observed.effective_level() != projected.effective_level()`.
///
/// Authority: CAPCO-2016 §H.7 p129 line 3168 worked example.
#[test]
fn e068_fires_on_classification_level_mismatch() {
    // Arrange: portions are TopSecret + Secret → max TopSecret.
    // Banner observes Secret.
    let source = "SECRET//NOFORN
(TS//NF)
(S//NF)
SECRET//NOFORN
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert
    assert!(
        observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 must fire when banner is Secret but portions project \
         TopSecret (§H.7 reciprocal raise); observed = {observed:?}"
    );
}

/// E068 does NOT fire when banner and projection agree on
/// classification.
#[test]
fn e068_does_not_fire_on_agreement() {
    // Arrange: portions are both Secret; banner is Secret.
    let source = "SECRET//NOFORN
(S//NF)
(S//NF)
SECRET//NOFORN
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert
    assert!(
        !observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 must not fire when banner and projection agree; \
         observed = {observed:?}"
    );
}

// ---------------------------------------------------------------------------
// E069 branch coverage
// ---------------------------------------------------------------------------

/// E069 fires when banner has no FGI marker but page projects one.
/// Load-bearing #276 reproduction matching the §H.7 p129 worked
/// example.
///
/// Authority: CAPCO-2016 §H.7 p124 banner-line FGI roll-up rule +
/// §H.7 p129 line 3168 worked example
/// (`TOP SECRET//FGI CAN DEU//NOFORN` produced by `(S//REL TO USA,
/// AUS) + (//CAN S) + (//DEU TS//NF)`).
#[test]
fn e069_fires_when_banner_missing_fgi_marker_and_page_projects_one() {
    // Arrange: US portion + DEU FGI portion. Banner observes no FGI
    // marker.
    let source = "TOP SECRET//NOFORN
(S//NF)
(//DEU TS//REL TO USA, DEU)
TOP SECRET//NOFORN
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert: E069 fires for the missing FGI marker. (E068 may also
    // fire because of dependent classification mismatch; this test
    // is orthogonal to that.)
    assert!(
        observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 must fire when banner lacks FGI marker but portions \
         contribute FGI DEU (§H.7 p124 banner roll-up); observed = \
         {observed:?}"
    );
}

/// E069 does NOT fire when banner and projection agree on the FGI
/// marker (acknowledged, same country list).
///
/// Authority: CAPCO-2016 §H.7 p124.
#[test]
fn e069_does_not_fire_on_fgi_marker_agreement() {
    // Arrange: pure-foreign FGI DEU page; banner retains FGI DEU.
    let source = "CONFIDENTIAL//FGI DEU
(C//FGI DEU)
(C//FGI DEU)
CONFIDENTIAL//FGI DEU
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert
    assert!(
        !observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 must not fire when banner FGI marker matches projection; \
         observed = {observed:?}"
    );
}

/// E069 does NOT fire on a pure-NATO page — NATO is its own axis
/// (not FGI), so neither observed nor projected sides populate
/// `fgi_marker`.
///
/// Authority: CAPCO-2016 §H.7 pp123-125 + §G.2 p40 Table 5.
#[test]
fn e069_does_not_fire_on_pure_nato_page() {
    // Arrange: pure-NATO portions; banner is bare NATO.
    let source = "//NATO SECRET//REL TO USA, NATO
(//NS//REL TO USA, NATO)
(//NS//REL TO USA, NATO)
//NATO SECRET//REL TO USA, NATO
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert
    assert!(
        !observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 must not fire on pure-NATO page (fgi_marker is None on \
         both sides; NATO is its own axis per §G.2 p40); observed = \
         {observed:?}"
    );
}

// ---------------------------------------------------------------------------
// Combined: E068 + E069 both fire on the load-bearing #276 fixture
// ---------------------------------------------------------------------------

/// E069 does NOT fire when banner FGI country list is identical to
/// the projected page state as a SET, but listed in non-canonical
/// (non-alphabetical) order. The post-fix-up evaluator compares
/// country lists as `BTreeSet`s, not slices — ordering is the
/// renderer's concern, not E069's. Pre-fix-up this case would have
/// false-positive-fired because the parser preserves textual order
/// (`NZL GBR`) while `FgiSet::to_marker()` iterates a sorted
/// `BTreeSet` (`GBR, NZL`).
///
/// Shape: commingled US + FGI portions (`(S//FGI <CC>//NF)`) so the
/// page projection populates `fgi_marker` on the dissem axis (the
/// §H.7 p123 commingled signal). Pure-foreign portions `(//<CC> S)`
/// would populate `classification = Fgi(...)` instead and leave
/// `fgi_marker = None` — that exercises the `(Some, None)` branch,
/// not the country-set branch we're testing here.
///
/// Authority: CAPCO-2016 §H.7 p124 banner-line FGI roll-up rule
/// describes the *required* country set; ordering is governed by
/// `render_canonical` (post-PR-3b.F E060 retirement, the renderer
/// owns canonical-form discipline).
#[test]
fn e069_does_not_fire_on_non_canonical_country_order() {
    // Arrange: commingled US + FGI portions contribute FGI GBR and
    // FGI NZL; banner lists them in non-alphabetical order
    // (`NZL GBR`). Country sets match; only the order differs.
    let source = "SECRET//FGI NZL GBR//NOFORN
(S//FGI GBR//NF)
(S//FGI NZL//NF)
SECRET//FGI NZL GBR//NOFORN
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert: E069 must NOT fire — the country sets are identical.
    // Pre-fix-up this assertion would have failed because slice
    // equality saw `[NZL, GBR] != [GBR, NZL]`.
    assert!(
        !observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 must not fire on non-canonical country order (sets are \
         identical); observed = {observed:?}"
    );
}

/// The §H.7 p129 line 3168 worked example fires BOTH E068 and E069
/// in one banner check. Verifies that the two rows are independent —
/// the engine's dispatch loop visits every catalog row.
#[test]
fn e068_and_e069_both_fire_on_mixed_us_foreign_with_wrong_banner() {
    // Arrange: §H.7 p129 line 3168 worked example — but the banner
    // observes the wrong shape (SECRET//NOFORN instead of TOP
    // SECRET//FGI DEU//NOFORN). Both classification (S vs TS) and
    // FGI marker (none vs DEU) disagree.
    let source = "SECRET//NOFORN
(S//NF)
(//DEU TS//REL TO USA, DEU)
SECRET//NOFORN
";

    // Act
    let observed = observed_rule_ids(source);

    // Assert
    assert!(
        observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 must fire on classification-level mismatch (§H.7 \
         reciprocal raise: Secret observed, TopSecret projected); \
         observed = {observed:?}"
    );
    assert!(
        observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 must fire on missing FGI marker (§H.7 p124 + p129 line \
         3168); observed = {observed:?}"
    );
}
