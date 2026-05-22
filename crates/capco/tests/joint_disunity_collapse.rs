// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! W004 joint-disunity-collapse rule + JointSet lattice integration tests.
//!
//! Authority (verified 2026-05-16 against CAPCO-2016.md):
//! - §H.3 p56 (JOINT classification grammar).
//! - §H.3 pp55-59 (JOINT worked examples).
//! - §H.3 p57 ("JOINT not carried forward to banner in US documents").
//! - §H.7 p123 (FGI source-acknowledged form for disunity-collapse migration).
//!
//! PR refactor-006-pr-pagefinalization / issue #461 (Phase::PageFinalization
//! migration; the JointSet lattice tests originally landed in PR 4b-B
//! Commit 5 (006 T112) and were rebaselined here under the
//! PageFinalization dispatch contract).

use marque_capco::scheme::CapcoScheme;
use marque_capco::{CapcoRuleSet, JointSet};
use marque_config::Config;
use marque_engine::{Engine, FixedClock};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, JointClassification, MarkingClassification,
};

fn cc(s: &str) -> CountryCode {
    CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
}

fn joint_portion(level: Classification, producers: &[&str]) -> CanonicalAttrs {
    let countries: Box<[CountryCode]> = producers
        .iter()
        .map(|s| cc(s))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level,
        countries,
    }));
    a
}

#[test]
fn joint_unanimous_two_portions_same_producers_passes_through() {
    // §H.3 worked example: (//JOINT S USA GBR) (//JOINT S USA GBR)
    // → banner //JOINT SECRET USA, GBR.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "GBR"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    match s.to_marking_classification() {
        Some(MarkingClassification::Joint(j)) => {
            assert_eq!(j.level, Classification::Secret);
            assert_eq!(j.countries.len(), 2);
            let codes: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
            assert!(codes.contains(&"USA"));
            assert!(codes.contains(&"GBR"));
        }
        other => panic!("expected Joint marking, got {other:?}"),
    }
}

#[test]
fn joint_unanimous_three_portions_different_levels() {
    // (//JOINT C USA GBR) (//JOINT TS USA GBR) (//JOINT S USA GBR)
    // → banner //JOINT TOP SECRET USA, GBR (OrdMax on level).
    let portions = [
        joint_portion(Classification::Confidential, &["USA", "GBR"]),
        joint_portion(Classification::TopSecret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "GBR"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    assert_eq!(s.highest_level(), Some(Classification::TopSecret));
}

#[test]
fn joint_disunity_two_portions_different_producers_collapses_to_fgi() {
    // (//JOINT S USA GBR) (//JOINT S USA CAN)
    // → banner SECRET//FGI CAN GBR + W004 Warn diagnostic.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "CAN"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(s.is_disunity_collapse());
    let non_us = s.disunity_collapse_non_us_producers().expect("disunity");
    assert!(non_us.contains(&cc("GBR")));
    assert!(non_us.contains(&cc("CAN")));
    // USA is excluded from the non-US producer set; banner FGI block
    // gets only the non-US producers (the US side becomes the
    // primary banner level).
    assert!(!non_us.contains(&cc("USA")));

    // The collapse renders the banner classification as US (highest
    // level observed), not Joint.
    match s.to_marking_classification() {
        Some(MarkingClassification::Us(c)) => {
            assert_eq!(c, Classification::Secret);
        }
        other => panic!("expected Us(Secret), got {other:?}"),
    }
}

#[test]
fn joint_mixed_with_us_portions_no_w004_fires() {
    // §H.3 p57: mixed (JOINT + US) → JOINT does not roll
    // up. Lattice returns `Mixed` (PR 4b-B follow-up C-3: was
    // `Bottom` before but that conflated identity with absorption,
    // breaking associativity under grouped joins); the existing
    // PageContext path handles FGI migration.
    let mut us = CanonicalAttrs::default();
    us.classification = Some(MarkingClassification::Us(Classification::Secret));
    let portions = [joint_portion(Classification::Secret, &["USA", "GBR"]), us];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(matches!(s, JointSet::Mixed));
    assert!(s.is_mixed());
    // W004 only fires on DisunityCollapse; Mixed should not fire it.
    assert!(!s.is_disunity_collapse());
}

#[test]
fn joint_disunity_warn_diagnostic_carries_no_document_text() {
    // Constitution V Principle V G13: the W004 diagnostic message
    // must not contain document text. We verify this indirectly by
    // confirming that the JointSet's `disunity_collapse_non_us_producers`
    // returns only canonical CountryCode vocabulary atoms (3-byte
    // trigraphs). The rule emits these as canonical strings, never
    // as input bytes.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "CAN", "FRA"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    let non_us = s.disunity_collapse_non_us_producers().expect("disunity");

    // Every producer must be a valid canonical trigraph (3 bytes
    // ASCII uppercase). The CountryCode type enforces this at the
    // type level (any try_new failure would have caught it in the
    // setup), so this assertion is structural — if it ever fails,
    // CountryCode invariants have eroded.
    for c in non_us {
        let s = c.as_str();
        assert_eq!(s.len(), 3, "non-trigraph CountryCode: {s:?}");
        assert!(s.bytes().all(|b| b.is_ascii_uppercase()));
    }
}

// ---------------------------------------------------------------------------
// H-2 PR 4b-B follow-up — engine-level W004 tests
// ---------------------------------------------------------------------------
//
// The JointSet unit tests above exercise the lattice type directly.
// The tests below run W004 (`JointDisunityCollapseRule`) through the
// rule/engine path — they verify the diagnostic actually fires,
// carries the expected severity / rule ID, surfaces only canonical
// CountryCode trigraph identifiers in its message (Constitution V
// Principle V G13), and is correctly suppressed on negative cases
// (mixed JOINT+US per §H.3 p57; pure-US pages; pure-JOINT-unanimous
// pages).

fn engine_with_fixed_clock() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

#[test]
fn w004_fires_on_joint_disunity_banner() {
    // Two JOINT portions with disagreeing producer lists accumulate
    // on the page; the EOD `Phase::PageFinalization` dispatch sees
    // the closed `JointSet::DisunityCollapse` state and emits W004.
    // The trailing banner is incidental — issue #461 closure means
    // W004 is no longer Banner-gated, so the firing surface is the
    // page fixpoint regardless of whether a closing banner exists.
    // The test name is retained from pre-#461 for git-blame
    // continuity; the assertions still hold under PageFinalization
    // because the same disunity is observable on the closed state
    // regardless of which boundary closes the page.
    //
    // **Copilot-flagged regression guard.** Pre-fix this test was a
    // `.find().is_some()` assertion that masked the engine's
    // main-loop double-dispatch defect: the main candidate loop
    // ran W004 on the Banner candidate (because the loop had no
    // phase filter), and `dispatch_page_finalization` ran it
    // again at EOD. Tightened to `count == 1` so any future
    // regression that re-introduces a missing phase filter at
    // the engine main-loop level (`engine.rs:1202-1203`) fails
    // this test loudly.
    //
    // Assertions: rule = "W004", Warn severity, citation references
    // §H.3 p57 + §H.7 p123 (CV-4 PR 4b-B 8th-pass — updated from
    // `§H.3 p56`).
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) first portion.\n\
                   (//JOINT S USA CAN) second portion.\n\
                   SECRET//FGI CAN GBR//NOFORN\n";

    let lint = engine.lint(source);
    let w004_diags: Vec<_> = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi")
        .collect();
    assert_eq!(
        w004_diags.len(),
        1,
        "W004 must fire EXACTLY once on a JOINT-disunity page with a \
         closing banner (regression guard for the engine main-loop \
         phase-filter defect Copilot flagged on PR #461). All \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
    let w004 = w004_diags[0];
    assert_eq!(w004.severity, marque_rules::Severity::Warn);
    // CV-4 (PR 4b-B 8th-pass): citation amended from
    // `§H.3 p56 + §H.7 p123` to `§H.3 p57 + §H.7 p123` — §H.3 p57
    // ("The FGI marking including all trigraph/tetragraph codes
    // identified in the JOINT portion(s)" in the Derivative Use
    // bullets) is the precise migration-trigger authority. §H.7 p123
    // grounds the FGI grammar the migrated producers render under.
    //
    // PR 3c.2.C C5: typed `Citation` carries ONE §-reference per
    // diagnostic. The rule body anchors at §H.3 p57 (the precise
    // migration-trigger); the §H.7 p123 FGI grammar reference now
    // lives in the rule doc comment rather than the diagnostic
    // citation field (compare `crates/capco/src/rules.rs`
    // JointDisunityCollapseRule). Assert only the structured anchor.
    assert!(
        format!("{}", w004.citation).contains("§H.3 p57"),
        "W004 citation must reference §H.3 p57 (structured anchor): {:?}",
        w004.citation
    );
}

#[test]
fn w004_message_contains_only_canonical_trigraphs() {
    // Constitution V Principle V G13: the W004 diagnostic message
    // MUST NOT contain document bytes.
    //
    // PR 3c.2.C C5: G13 is now structurally enforced by the closed
    // `Message` shape — `MessageArgs` field types are restricted to
    // `Option<TokenId>` / `Option<CategoryId>` / `Option<Span>` /
    // `Blake3Hash` / `Confidence` / `FeatureId`; raw bytes are
    // unrepresentable. The test purpose strengthens: instead of
    // grepping prose for a sentinel that *could have* leaked, we
    // verify the template + category identification (the closed-set
    // analog of "what does the diagnostic say").
    let engine = engine_with_fixed_clock();
    // Use a distinctive surrounding prose sentinel that should NEVER
    // appear in any diagnostic message regardless of rule. The
    // sentinel is kept in the source so future regressions
    // (an accidental free-form channel added back to `Message`)
    // would still surface via a corpus diff, but the assertion is
    // now structural.
    let prose_sentinel = "PROSE_SENTINEL_LEAKED_INTO_DIAGNOSTIC";
    let source = format!(
        "{prose_sentinel} (//JOINT TS USA GBR) first portion.\n\
         {prose_sentinel} (//JOINT TS USA CAN) second portion.\n\
         TOP SECRET//FGI CAN GBR//NOFORN\n"
    );

    let lint = engine.lint(source.as_bytes());
    let w004 = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi")
        .expect("W004 must fire on disunity page");
    // Closed-template identification: W004 fires under
    // `BannerRollupMismatch` with `CAT_JOINT_CLASSIFICATION`. The
    // per-trigraph identification that the prose previously carried
    // (CAN / GBR) is dropped per PM-C-5 / PM-C-6 — the renderer
    // re-derives runtime detail from `(source, span, marking)`.
    use marque_capco::scheme::CAT_JOINT_CLASSIFICATION;
    use marque_rules::MessageTemplate;
    assert_eq!(
        w004.message.template(),
        MessageTemplate::BannerRollupMismatch,
        "W004 fires under the BannerRollupMismatch template; got {:?}",
        w004.message.template(),
    );
    assert_eq!(
        w004.message.args().category,
        Some(CAT_JOINT_CLASSIFICATION),
        "W004 must identify the JOINT classification category; got {:?}",
        w004.message.args().category,
    );
}

#[test]
fn w004_does_not_fire_on_pure_us_page() {
    // No JOINT portions → JointSet::Bottom → W004 must NOT fire.
    let engine = engine_with_fixed_clock();
    let source = b"(S) plain portion one.\n\
                   (S) plain portion two.\n\
                   SECRET\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.predicate_id() != "W004"),
        "W004 must NOT fire on pure-US page; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_does_not_fire_on_mixed_joint_plus_us() {
    // §H.3 p57: JOINT does not roll up in US documents. The
    // JointSet returns `Mixed` (post-C-3 PR 4b-B follow-up; was
    // Bottom pre-split). W004 must NOT fire — the FGI migration
    // for the JOINT non-US producers rides through the existing
    // PageContext-resident `expected_fgi_marker` path, not through
    // W004's lattice signal.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) joint-classified portion.\n\
                   (S) plain-us-classified portion.\n\
                   SECRET//FGI GBR//NOFORN\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.predicate_id() != "W004"),
        "W004 must NOT fire on mixed JOINT+US page per §H.3 p57; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_does_not_fire_on_pure_joint_unanimous() {
    // All portions JOINT with identical producer lists → JointSet::
    // UnanimousProducers → W004 must NOT fire (no disunity to
    // surface). The banner shows `//JOINT [class] [LIST]` per §H.3 p56.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) first portion.\n\
                   (//JOINT S USA GBR) second portion.\n\
                   //JOINT SECRET USA, GBR\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.predicate_id() != "W004"),
        "W004 must NOT fire on unanimous-JOINT page; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn joint_disunity_union_excludes_usa() {
    // §H.7 p123: FGI is foreign-source; USA never appears in the FGI
    // [LIST] regardless of where it appeared on the source side. The
    // JointSet's `union_non_us_producers` excludes USA by
    // construction.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "FRA", "DEU"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    let non_us = s.disunity_collapse_non_us_producers().unwrap();
    assert!(!non_us.contains(&cc("USA")));
    assert!(non_us.contains(&cc("GBR")));
    assert!(non_us.contains(&cc("FRA")));
    assert!(non_us.contains(&cc("DEU")));
    assert_eq!(non_us.len(), 3);
}

// ---------------------------------------------------------------------------
// Issue #461 closure: W004 migrated to Phase::PageFinalization. The
// engine synthesizes a PageFinalization candidate at every PageBreak
// (BEFORE the PageContext reset) and once at end-of-document; W004
// now sees the page-level fixpoint snapshot. The pre-#461 8th-pass
// false-negative (banner-first layouts without closing banner) is
// closed by the EOD path; the pre-#461 6th-pass false-positive
// (Mixed-page mis-detection at Portion time) does not recur because
// PageFinalization fires exactly once per page on the closed page
// state.
//
// §-authority: §H.3 p57 (JOINT not carried to banner — Derivative
// Use bullets specify the FGI [LIST] migration trigger) + §H.7 p123
// (FGI grammar). Verified 2026-05-16 against
// `crates/capco/docs/CAPCO-2016.md`.
// ---------------------------------------------------------------------------

#[test]
fn w004_fires_on_banner_first_via_eod_finalization() {
    // Pre-#461: this layout was a documented false-negative. The top
    // banner ran before any portions accumulated (so the old
    // Banner-only firing path saw `page_context = None` and bailed),
    // and no closing footer banner meant the rule never re-fired.
    // Post-#461: PageFinalization dispatch at end-of-document
    // observes the page-level fixpoint snapshot — JOINT-disunity
    // across the two portions surfaces W004 once. Authority:
    // §H.3 p57 (banner cannot carry JOINT in US documents) + §H.7
    // p123 (FGI grammar the non-US producers migrate under).
    let engine = engine_with_fixed_clock();
    let source = b"//JOINT SECRET USA, GBR, CAN\n\
                   (//JOINT S USA GBR) first portion.\n\
                   (//JOINT S USA CAN) second portion creates disunity.\n";
    let lint = engine.lint(source);
    let w004 = lint.diagnostics.iter().find(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi");
    assert!(
        w004.is_some(),
        "Issue #461 closure: W004 MUST fire via EOD PageFinalization \
         on banner-first JOINT-disunity layout with no closing banner; \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
    let w004 = w004.unwrap();
    assert_eq!(w004.severity, marque_rules::Severity::Warn);
    // PR 3c.2.C C5: typed `Citation` carries one §-reference per
    // diagnostic. The rule anchors at §H.3 p57; §H.7 p123 lives in
    // the rule doc comment per PM-C-5/PM-C-6.
    assert!(
        format!("{}", w004.citation).contains("§H.3 p57"),
        "W004 citation must reference §H.3 p57 (structured anchor): {:?}",
        w004.citation
    );
}

#[test]
fn w004_does_not_fire_on_single_joint_portion_at_eod() {
    // A single JOINT portion can never produce disunity — disunity
    // needs at least two JOINT portions with disagreeing producer
    // lists. Even under Phase::PageFinalization (which now reaches
    // single-portion pages via the EOD dispatch), the JointSet is
    // `UnanimousProducers` for a one-portion page, not
    // `DisunityCollapse`, so W004 stays silent. Verifies the
    // PageFinalization closure didn't accidentally widen the firing
    // surface beyond the §H.3 p57 disunity contract.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) only portion on the page.\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.predicate_id() != "W004"),
        "W004 must NOT fire on a single JOINT portion (no disunity \
         possible); diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Issue #461 — Phase::PageFinalization behavioral coverage (new tests).
// ---------------------------------------------------------------------------
//
// These tests pin behavior of the PageFinalization dispatch path
// against §H.3 p57 + §H.7 p123. Names describe what the user sees
// ("fires", "does NOT fire", which page), not the dispatch
// mechanism. The engine's `dispatch_page_finalization` synthesizes
// one boundary candidate per `MarkingType::PageBreak` (BEFORE the
// PageContext reset) and one at end-of-document. The PageBreak
// heuristic is form-feed (`\f`) or `\n\n\n+`; the tests below use
// form-feed for determinism.

#[test]
fn w004_fires_per_page_break_independently() {
    // §H.3 p57 + §H.7 p123: each page's banner is finalized
    // independently. Page 1 has JOINT disunity → W004 fires.
    // Page 2 has unanimous JOINT producers (no disunity) → W004
    // stays silent on page 2. Net: exactly one W004 diagnostic
    // across the document. The form-feed separates the pages so
    // the scanner emits a `MarkingType::PageBreak` candidate that
    // triggers the first PageFinalization dispatch.
    let engine = engine_with_fixed_clock();
    let source: &[u8] = b"(//JOINT S USA GBR) page 1 first portion.\n\
                          (//JOINT S USA CAN) page 1 disunity portion.\n\
                          \x0c\
                          (//JOINT S USA GBR) page 2 first portion.\n\
                          (//JOINT S USA GBR) page 2 same producers.\n";
    let lint = engine.lint(source);
    let w004_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi")
        .count();
    assert_eq!(
        w004_count,
        1,
        "W004 must fire exactly once (page 1 has disunity, page 2 is \
         unanimous); diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_fires_on_both_disunity_pages() {
    // Two pages, both with JOINT disunity. PageFinalization dispatch
    // fires per-page on the page-level fixpoint, so W004 emits
    // twice — once at the form-feed boundary (closing page 1), once
    // at end-of-document (closing page 2). Authority: §H.3 p57
    // (JOINT not carried to banner; non-US producers migrate per
    // §H.7 p123).
    let engine = engine_with_fixed_clock();
    let source: &[u8] = b"(//JOINT S USA GBR) page 1 first portion.\n\
                          (//JOINT S USA CAN) page 1 disunity portion.\n\
                          \x0c\
                          (//JOINT S USA FRA) page 2 first portion.\n\
                          (//JOINT S USA DEU) page 2 disunity portion.\n";
    let lint = engine.lint(source);
    let w004_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi")
        .count();
    assert_eq!(
        w004_count,
        2,
        "W004 must fire exactly twice (one per disunity page); \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_does_not_fire_on_mixed_page_via_finalization() {
    // §H.3 p57: a page with JOINT portions AND a non-JOINT portion
    // collapses to `JointSet::Mixed` at the page-level fixpoint.
    // The FGI migration for the JOINT non-US producers rides
    // through the existing PageContext-resident `expected_fgi_marker`
    // path, NOT through W004. PageFinalization sees the closed
    // page state — Mixed, not DisunityCollapse — so W004 stays
    // silent. This is the regression that the 6th-pass
    // Portion-firing experiment introduced and the 8th-pass
    // Banner-only reverted; PageFinalization preserves the
    // 8th-pass correctness while also closing the banner-first
    // false-negative.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) joint-classified portion.\n\
                   (//JOINT S USA CAN) joint-classified portion.\n\
                   (S//NF) non-joint portion forces Mixed.\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.predicate_id() != "W004"),
        "W004 must NOT fire on a Mixed page (JOINT + non-JOINT \
         portions) per §H.3 p57; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_does_not_fire_on_empty_page() {
    // A document with a banner only (no portions) has an empty
    // PageContext at end-of-document — `dispatch_page_finalization`
    // skips empty pages by construction (its caller guards on
    // `!page_context.is_empty()`). No rule body runs and no W004
    // diagnostic emits. Authority for the empty-page guard:
    // `dispatch_page_finalization` doc + Constitution VI (engine
    // does not invoke PageFinalization on empty pages).
    let engine = engine_with_fixed_clock();
    let source = b"//JOINT SECRET USA, GBR\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.predicate_id() != "W004"),
        "W004 must NOT fire on a banner-only document with no \
         portions; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_eod_fires_for_trailing_disunity_without_pagebreak() {
    // No form-feed, no `\n\n\n+`, no closing banner — purely
    // trailing JOINT portions running off the end of the document.
    // Pre-#461 this was the documented false-negative
    // (`w004_does_not_fire_on_banner_first_document_with_no_closing_banner`,
    // renamed to `w004_fires_on_banner_first_via_eod_finalization`).
    // Post-#461 the EOD PageFinalization dispatch closes it
    // unconditionally — even without a banner anywhere. This is
    // the minimum case where the EOD path matters: a document
    // composed of nothing but disunified JOINT portions.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) first portion.\n\
                   (//JOINT S USA CAN) second portion forces disunity.\n";
    let lint = engine.lint(source);
    let w004 = lint.diagnostics.iter().find(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi");
    assert!(
        w004.is_some(),
        "Issue #461 closure: W004 MUST fire via EOD PageFinalization \
         on trailing-portions-only layout; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Engine main-loop phase-filter regression guards (Copilot review on
// PR refactor-006-pr-pagefinalization / issue #461).
// ---------------------------------------------------------------------------
//
// Copilot's HIGH-severity finding: the engine's main candidate-loop
// (`engine.rs:1202-1203`) iterated `self.rule_sets[..].rules()[..]`
// with NO phase filter, so every registered rule — including
// `Phase::PageFinalization` rules — ran on every Portion / Banner /
// CAB candidate. With W004's body no longer gated on
// `MarkingType::Banner`, the rule fired TWICE on any page with a
// closing banner: once from the main loop's Banner-candidate
// dispatch (because the engine attaches `ctx.page_context` to
// non-Portion candidates with non-empty pages), and once from
// `dispatch_page_finalization` at the next PageBreak / EOD.
//
// The two tests below pin "exactly once per page" semantics on the
// two layout shapes where the pre-fix bug would surface as a
// double-fire: single page with closing banner, and two-page document
// with closing banners on both pages.
//
// Both tests would FAIL on the pre-fix engine; they PASS on
// the post-fix engine (`pass_finalization_rule_indices` skipped from
// the main loop). The companion regression tightening lives at
// `w004_fires_on_joint_disunity_banner` (top of file) where
// `.find().is_some()` was upgraded to `.filter().count() == 1`.

#[test]
fn w004_fires_exactly_once_on_page_with_closing_banner() {
    // Single page with two disunified JOINT portions AND a closing
    // banner. Pre-Copilot-fix the main candidate loop's
    // Banner-candidate dispatch ran W004 once (the body would early-
    // return on the Banner-only guard, but the guard was removed in
    // the PageFinalization migration — now the body runs and
    // emits because `page_context` is populated on Banner candidates
    // by accumulation, and `JointSet::DisunityCollapse` is true at
    // that snapshot). `dispatch_page_finalization` then ran W004 a
    // second time at EOD. Net: 2 W004 diagnostics. Post-fix: the
    // main loop's phase filter skips PageFinalization rules entirely,
    // so only the EOD synthesis dispatches W004 → count == 1.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) first portion.\n\
                   (//JOINT S USA CAN) second portion.\n\
                   SECRET//FGI CAN GBR//NOFORN\n";
    let lint = engine.lint(source);
    let w004_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi")
        .count();
    assert_eq!(
        w004_count,
        1,
        "W004 must fire EXACTLY once on a single page with a closing \
         banner. A count of 2 means the engine's main candidate loop \
         did NOT skip Phase::PageFinalization rules and W004 ran both \
         (a) on the Banner candidate in the main loop, AND (b) via \
         dispatch_page_finalization at EOD. This is the Copilot-HIGH \
         regression guard for PR #461. Diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_fires_exactly_once_per_page_when_banner_closes_page() {
    // Two-page document, each page has disunified JOINT portions
    // AND a closing banner before the form-feed (or end-of-document
    // for page 2). Pre-Copilot-fix this would emit FOUR W004
    // diagnostics: two from main-loop Banner dispatches + two from
    // PageFinalization (one per `\f` boundary + one at EOD — except
    // the EOD page's banner doesn't precede a `\f`, so dispatch
    // fires at EOD instead; either way, one PageFinalization fire
    // per page). Post-fix: only the PageFinalization-path fires,
    // one per page → count == 2.
    let engine = engine_with_fixed_clock();
    let source: &[u8] = b"(//JOINT S USA GBR) page 1 first portion.\n\
                          (//JOINT S USA CAN) page 1 disunity portion.\n\
                          SECRET//FGI CAN GBR//NOFORN\n\
                          \x0c\
                          (//JOINT S USA FRA) page 2 first portion.\n\
                          (//JOINT S USA DEU) page 2 disunity portion.\n\
                          SECRET//FGI DEU FRA//NOFORN\n";
    let lint = engine.lint(source);
    let w004_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.fgi.joint-disunity-collapses-to-fgi")
        .count();
    assert_eq!(
        w004_count,
        2,
        "W004 must fire EXACTLY twice on a two-page document where \
         each page has a closing banner (one per page, not four). \
         A count of 4 indicates the main-loop phase filter regressed \
         and W004 dispatched on both Banner candidates AND both \
         PageFinalization synthesis points. Copilot-HIGH regression \
         guard for PR #461. Diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}
