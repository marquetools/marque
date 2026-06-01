// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Foreign-banner correctness corpus.
//!
//! Loads `tests/corpus/foreign/*.txt` fixtures and asserts the
//! engine-level behavioral invariants tracked under #276:
//!
//! - **`pure_foreign_banner.txt`**: #276 Case 1 — pure-foreign
//!   page with `(//DEU C//REL TO USA, DEU)` portions (FGI
//!   classification system, no US axis); banner
//!   `//DEU CONFIDENTIAL//REL TO USA, DEU` matches the projected
//!   `Fgi(Confidential, [DEU])` state per §H.7 p126. Zero
//!   diagnostics expected.
//! - **`joint_us_uk.txt`**: JOINT US+GBR page per CAPCO-2016
//!   §H.3 p56; banner matches projected page state. Zero diagnostics.
//! - **`nato_only_page.txt`**: solely-NATO page per §H.7
//!   pp123-125; banner preserves the NATO classification variant.
//!   Zero diagnostics.
//! - **`mixed_us_foreign_rollup.txt`**: the load-bearing #276 fixture.
//!   `(S//NF) + (//DEU TS//REL TO USA, DEU)` with observed banner
//!   `SECRET//NOFORN`. Per §H.7 p129 the correct banner is
//!   `TOP SECRET//FGI DEU//NOFORN`. E068 + E069 fire (classification
//!   mismatch + missing FGI marker). NOFORN-supremacy composition:
//!   NOFORN survives, REL TO is cleared via the §H.8 p145
//!   `capco/noforn-clears-rel-to` PageRewrite, and the FGI marker
//!   propagates through the cross-axis composition.
//! - **`fgi_concealed.txt`**: §H.7 p128 worked
//!   example. `(S//RELIDO) + (//DEU S//NF) + (//FGI S//NF)` projects to
//!   bare `FGI` (no trigraph) per the source-concealed-dominates rule
//!   on §H.7 p124. Banner observes `SECRET//FGI//NOFORN`; matches.
//!   Zero diagnostics.
//!
//! ## Why this lives in `marque-capco/tests/`
//!
//! The corpus accuracy harness in
//! `crates/engine/tests/corpus_accuracy.rs` scans only the `valid/`,
//! `invalid/`, and `prose/` subdirectories. The `foreign/` directory
//! is loaded here so the foreign-banner correctness invariants travel
//! with the CAPCO domain crate.
//!
//! ## Assertion shape
//!
//! Rule-ID presence/absence rather than exact-span matching. Banner
//! candidates fire at different byte offsets depending on where the
//! engine emits the candidate, and the `mixed_us_foreign_rollup`
//! fixture has two banner candidates (top + bottom). The
//! presence-only check is robust to that variation while still
//! catching the load-bearing #276 regression (no E068/E069 firing
//! at all is the bug).

use marque_capco::{CapcoRuleSet, scheme::CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_test_utils::{corpus_root, load_expected, load_fixture};
use std::collections::HashSet;
use std::path::PathBuf;

fn engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn foreign_fixture(name: &str) -> PathBuf {
    corpus_root().join("foreign").join(name)
}

/// Run the engine against a foreign-corpus fixture and return the set
/// of emitted rule IDs.
///
/// AAA-pattern: caller arranges the fixture name and expected set;
/// this helper acts (loads + lints) and returns the observable for
/// the caller's assertion.
fn observed_rule_ids(fixture_name: &str) -> HashSet<String> {
    let path = foreign_fixture(fixture_name);
    let source = load_fixture(&path);
    let result = engine().lint(&source);
    result
        .diagnostics
        .iter()
        .map(|d| d.rule.predicate_id().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// Pure-foreign banner: zero diagnostics expected.
// ---------------------------------------------------------------------------

/// #276 Case 1 — pure-foreign banner page. Portions
/// `(//DEU C//REL TO USA, DEU)` use the FGI classification system
/// (no US axis at all), so the per-portion classification parser
/// produces `MarkingClassification::Fgi { level: Confidential,
/// countries: [DEU] }`. The page projection preserves the `Fgi(_)`
/// variant per §H.7 pp123-125 (solely-foreign preservation). The
/// banner `//DEU CONFIDENTIAL//REL TO USA, DEU` matches the
/// projected state on both axes: classification (`Fgi(_)` variant,
/// effective level Confidential) and FGI marker (`None` on both
/// observed and projected — the pure-foreign-classification-system
/// form populates `classification` only, not `fgi_marker`).
///
/// Authority: CAPCO-2016 §H.7 p126 — pure-foreign worked example
/// `(//GBR S)` portion form rolls up to `//GBR SECRET` banner; this
/// fixture mirrors the structure with DEU + REL TO USA, DEU.
/// Distinct from #276 Case 2 (commingled US + FGI), which would
/// produce `(C//FGI DEU)` portions, `Us(Confidential)`
/// classification, and an FGI marker on the dissem axis.
#[test]
fn t062_pure_foreign_banner_zero_diagnostics() {
    // Arrange: the fixture documents zero expected diagnostics.
    let expected = load_expected(&foreign_fixture("pure_foreign_banner.txt"));
    assert!(
        expected.diagnostics.is_empty(),
        "pure_foreign_banner.expected.json must list zero diagnostics; \
         the fixture documents the §H.7 p124 happy path"
    );

    // Act: lint with the default engine.
    let observed = observed_rule_ids("pure_foreign_banner.txt");

    // Assert: rule loop did not emit E068 or E069 — the banner +
    // page state are in agreement. Other rules that happen to fire
    // on this fixture (none expected by inspection, but the assertion
    // is scoped to E068/E069 to keep the test orthogonal to unrelated
    // walker registrations) are tolerated.
    assert!(
        !observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 fired on pure-foreign banner — banner classification \
         matches projected state; rule should not fire. observed = {observed:?}"
    );
    assert!(
        !observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 fired on pure-foreign banner — banner FGI marker \
         matches projected state; rule should not fire. observed = {observed:?}"
    );
}

// ---------------------------------------------------------------------------
// JOINT US+GBR: zero diagnostics expected.
// ---------------------------------------------------------------------------

/// JOINT US+GBR page per CAPCO-2016 §H.3 p56. Banner matches the
/// projected page state on classification (Joint variant preserved
/// per §H.3 p56) and REL TO (USA, GBR per §H.3 — JOINT requires the
/// REL TO companion). FGI marker is None on both observed and
/// projected sides because JOINT is its own classification axis,
/// distinct from FGI (the FgiSet builder reads
/// `Joint(_).countries` only into FGI when projecting from a
/// JOINT-disunity collapse — not here, where producer lists agree).
#[test]
fn t063_joint_us_uk_zero_diagnostics() {
    let expected = load_expected(&foreign_fixture("joint_us_uk.txt"));
    assert!(
        expected.diagnostics.is_empty(),
        "joint_us_uk.expected.json must list zero diagnostics"
    );

    let observed = observed_rule_ids("joint_us_uk.txt");
    assert!(
        !observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 fired on JOINT US+GBR — banner classification matches \
         projected Joint variant; rule should not fire. observed = {observed:?}"
    );
    assert!(
        !observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 fired on JOINT US+GBR — FGI marker absent on both \
         observed and projected sides; rule should not fire. observed = {observed:?}"
    );
}

// ---------------------------------------------------------------------------
// Pure-NATO page: zero diagnostics expected.
// ---------------------------------------------------------------------------

/// Pure-NATO page per CAPCO-2016 §H.7 pp123-125 (solely-NATO
/// classification preservation). The closure operator injects USA +
/// NATO into REL TO per §H.7 p127 + §G.2 Table 5 p40
/// (alliance-reciprocity). E068 + E069 do not fire because the
/// banner matches the projected page state.
#[test]
fn t063_nato_only_page_zero_diagnostics() {
    let expected = load_expected(&foreign_fixture("nato_only_page.txt"));
    assert!(
        expected.diagnostics.is_empty(),
        "nato_only_page.expected.json must list zero diagnostics"
    );

    let observed = observed_rule_ids("nato_only_page.txt");
    assert!(
        !observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 fired on pure-NATO page — banner classification matches \
         projected Nato variant per §H.7 pp123-125; rule should not fire. \
         observed = {observed:?}"
    );
    assert!(
        !observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 fired on pure-NATO page — pure-NATO portions do not \
         populate fgi_marker; rule should not fire. observed = {observed:?}"
    );
}

// ---------------------------------------------------------------------------
// Mixed US+foreign roll-up: E068 + E069 fire.
// ---------------------------------------------------------------------------

/// The load-bearing #276 fixture. `(S//NF) + (//DEU TS//REL TO USA,
/// DEU)` with observed banner `SECRET//NOFORN`. Per CAPCO-2016 §H.7
/// p129 line 3168 worked example the correct banner is
/// `TOP SECRET//FGI DEU//NOFORN`.
///
/// **E068 fires** — observed `Secret`, projected `TopSecret` via the
/// §H.7 pp123-125 reciprocal raise + max-across-systems rule.
///
/// **E069 fires** — observed FGI marker absent, projected `DEU`
/// (carried via the `FgiSet::from_attrs_iter` cross-axis fold that
/// unions per-portion `fgi_marker` with classification-derived
/// producers).
///
/// **Composition assertion** — NOFORN supremacy preserves the FGI
/// marker through the `capco/noforn-clears-rel-to` PageRewrite
/// (§H.8 p145). The portion-level `REL TO USA, DEU` is cleared at
/// the banner; NOFORN survives; the FGI marker is carried through
/// the cross-axis composition. Verified indirectly by E069 firing —
/// if the FGI marker were lost during the PageRewrite cascade, E069
/// would NOT fire on the missing-marker arm.
#[test]
fn t063a_t059b_mixed_us_foreign_rollup_emits_e068_and_e069() {
    // Arrange: the fixture documents that E068 + E069 are the
    // expected diagnostics.
    let expected = load_expected(&foreign_fixture("mixed_us_foreign_rollup.txt"));
    // `ExpectedRuleId` is a struct of (scheme, predicate_id) String
    // fields, not a `RuleId`. The expected.json fixture serializes the
    // 2-tuple form; the test checks predicate-ID strings directly.
    let expected_rules: HashSet<&str> = expected
        .diagnostics
        .iter()
        .map(|d| d.rule.predicate_id.as_str())
        .collect();
    assert!(
        expected_rules.contains("banner.classification.mismatch-vs-projected")
            && expected_rules.contains("banner.fgi.marker-mismatch-vs-projected"),
        "mixed_us_foreign_rollup.expected.json must list \
         banner.classification.mismatch-vs-projected + \
         banner.fgi.marker-mismatch-vs-projected; got {expected_rules:?}"
    );

    // Act: lint with the default engine.
    let observed = observed_rule_ids("mixed_us_foreign_rollup.txt");

    // Assert: both rules fired.
    assert!(
        observed.contains("banner.classification.mismatch-vs-projected"),
        "banner.classification.mismatch-vs-projected did NOT fire on \
         mixed US+foreign roll-up — this is the #276 regression. \
         Observed banner SECRET//NOFORN classifies as Secret; projection \
         across `(S//NF) + (//DEU TS)` is TopSecret per §H.7 pp123-125 \
         reciprocal raise. observed = {observed:?}"
    );
    assert!(
        observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "banner.fgi.marker-mismatch-vs-projected did NOT fire on mixed \
         US+foreign roll-up — this is the #276 regression. Observed \
         banner has no FGI marker; projection across `(S//NF) + \
         (//DEU TS)` populates FGI DEU per the FgiSet cross-axis fold. \
         observed = {observed:?}"
    );
}

// ---------------------------------------------------------------------------
// Source-concealed + source-acknowledged FGI: zero diagnostics.
// ---------------------------------------------------------------------------

/// CAPCO-2016 §H.7 p128 line 3153 Notional Example Page 3:
/// `(S//RELIDO) + (//DEU S//NF) + (//FGI S//NF)` projects to
/// `SECRET//FGI//NOFORN` (bare `FGI` per §H.7 p124 source-concealed
/// dominates rule). The observed banner matches; E068 + E069 do not
/// fire.
#[test]
fn t063_fgi_concealed_zero_diagnostics() {
    let expected = load_expected(&foreign_fixture("fgi_concealed.txt"));
    assert!(
        expected.diagnostics.is_empty(),
        "fgi_concealed.expected.json must list zero diagnostics"
    );

    let observed = observed_rule_ids("fgi_concealed.txt");
    assert!(
        !observed.contains("banner.classification.mismatch-vs-projected"),
        "E068 fired on §H.7 p128 concealed+acknowledged worked example \
         — banner classification matches projected Secret; rule should \
         not fire. observed = {observed:?}"
    );
    assert!(
        !observed.contains("banner.fgi.marker-mismatch-vs-projected"),
        "E069 fired on §H.7 p128 concealed+acknowledged worked example \
         — banner `FGI` (bare) matches projected source-concealed FGI \
         marker per the §H.7 p124 dominance rule; rule should not fire. \
         observed = {observed:?}"
    );
}
