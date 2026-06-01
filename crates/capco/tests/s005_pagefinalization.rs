// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR #488 (issue #488) S005 `Phase::PageFinalization` migration tests.
//!
//! Authority (re-verified 2026-05-17 against
//! `crates/capco/docs/CAPCO-2016.md`):
//! - §H.8 (REL TO list grammar — syntax + tetragraph definition).
//! - §D.2 Table 3 rule 21 (the REL TO atom-semantics intersection law
//!   that applies uniformly without distinguishing "active validation"
//!   from "consistent case" — the §-grounding for collapsing the
//!   historical S005/S006 split into one rule).
//! - ODNI ISMCAT V[`marque_ism::ISMCAT_TETRA_VERSION`] Tetragraph
//!   Taxonomy (member-country expansion for atom intersection).
//!
//! Behavioral contract pinned by this suite (mirroring
//! `joint_disunity_collapse.rs` for W004):
//!
//! 1. S005 fires on the page-level fixpoint snapshot, regardless of
//!    whether the page closes with a banner / CAB candidate or just
//!    runs off the end of the document.
//! 2. S005 fires exactly once per page that has an uncertain code
//!    surviving the intersection-with-other-codes guard.
//! 3. The historical S005/S006 split is gone — banner-consistent pages
//!    that pre-#488 emitted S006 (Info) now emit S005 (Suggest).
//! 4. The four supersession bails (NF-direct, SBU-NF/LES-NF needs-NF,
//!    NODIS, EXDIS) still suppress emission under PageFinalization
//!    dispatch.
//! 5. Severity-config override silences via `[rules] S005 = "off"`.
//! 6. The Banner candidate's main-loop dispatch does NOT double-fire
//!    S005 on pages that close with a banner (the engine's
//!    `pass_finalization_rule_indices` skip in the main loop covers
//!    S005 the same way it covers W004).
//! 7. Audit content-ignorance (Constitution V Principle V): diagnostic
//!    messages embed only
//!    canonical CAPCO REL TO codes + ODNI taxonomy `<Description>`
//!    text — never document bytes.
//! 8. The boundary-anchor span is zero-length and lands at a
//!    well-defined offset (page-break or `source.len()`).
//! 9. Empty pages produce no S005 (the dispatch helper's
//!    `!page_context.is_empty()` guard).
//! 10. The trigraph filter (`len == 3`) still suppresses pure-trigraph
//!     REL TO portions.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::{Config, RuleConfig};
use marque_engine::{CapcoEngine, FixedClock};
use marque_rules::Severity;
use std::collections::HashMap;

fn engine_with_fixed_clock() -> CapcoEngine {
    CapcoEngine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

// ---------------------------------------------------------------------------
// (a) Single-page banner-less layout fires at EOD.
// ---------------------------------------------------------------------------

#[test]
fn s005_fires_at_eod_on_banner_less_layout() {
    // Pre-#488: zero diagnostics — Banner/CAB gate at the head of
    // `analyze_uncertain_reduction` blocked firing because there was
    // no banner candidate to attach `page_context` to.
    // Post-#488: PageFinalization dispatch fires at end-of-document
    // on the closed page state; the disjoint REL TO lists trigger
    // S005 via the RSMA uncertain-code path.
    //
    // spellchecker:off
    // RSMA is a `decomposable="NA"` (deprecated, `<MembershipSupressed/>`
    // sentinel — single `p`, ODNI's spelling, preserved verbatim
    // from the source XSD; see `crates/ism/src/build.rs`) tetragraph
    // in ISMCAT V2022-NOV per `crates/capco/tests/tetragraph_consolidation.rs::
    // trichotomy_decomposable_na_deprecated_suppressed`. It satisfies
    // `is_decomposable(_).is_none() && len != 3`, the S005 trigger.
    // spellchecker:on
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//REL TO USA, GBR, RSMA) portion one.\n\
                   (S//REL TO USA, AUS, GBR) portion two.\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        1,
        "S005 must fire EXACTLY once on a banner-less page with \
         disjoint REL TO portions (the EOD PageFinalization path \
         closes the pre-#488 banner-less false-negative). \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// (b) Multi-page firing independence.
// ---------------------------------------------------------------------------

#[test]
fn s005_fires_per_page_break_independently() {
    // Page 1 has disjoint REL TO portions with the uncertain RSMA
    // tetragraph → S005 fires at the form-feed boundary. Page 2 has
    // only one REL TO portion (the `portions_with_rel_to.len() < 2`
    // bail suppresses S005 there) → S005 stays silent on page 2.
    // Net: exactly one S005 across the document.
    //
    // Mirrors `w004_fires_per_page_break_independently` from
    // `joint_disunity_collapse.rs`. Form-feed (`\x0c`) is the
    // deterministic page-break trigger; the `\n\n\n+` heuristic is
    // conservative and avoided here for stability.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source: &[u8] = b"(S//REL TO USA, GBR, RSMA) page 1 portion 1.\n\
                          (S//REL TO USA, AUS, GBR) page 1 portion 2.\n\
                          \x0c\
                          (S//REL TO USA, GBR) page 2 single portion.\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        1,
        "S005 must fire exactly once (page 1 has disjoint REL TO \
         portions with RSMA; page 2 has a single REL TO portion so \
         the two-portion bail suppresses S005). diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// (c) Banner-present case still fires — the e2 collapse contract.
// ---------------------------------------------------------------------------

#[test]
fn s005_fires_at_suggest_on_banner_consistent_page_post_collapse() {
    // Pre-#488: this fixture emitted S006 (Info) because the banner's
    // REL TO {USA, GBR} was consistent with the atom-semantics
    // intersection {USA, GBR}. Post-#488 the Info branch is gone; the
    // collapse means the same trigger now emits S005 at Suggest.
    //
    // This is the load-bearing contract of the e2 collapse: the
    // diagnostic identity changes (S006 → S005) and the severity
    // changes (Info → Suggest), and that change is intentional per
    // the PR brief.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//REL TO USA, GBR, RSMA) portion one.\n\
                   (S//REL TO USA, AUS, GBR) portion two.\n\
                   SECRET//REL TO USA, GBR\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s006_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "S006")
        .count();
    assert_eq!(
        s006_count,
        0,
        "S006 must NOT fire post-#488 — the rule is retired. \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );

    let s005: Vec<_> = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .collect();
    assert_eq!(
        s005.len(),
        1,
        "S005 must fire EXACTLY once on banner-consistent page \
         post-#488 collapse (pre-#488 this was S006 at Info; the \
         collapse intentionally re-routes to S005 at Suggest). \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        s005[0].severity,
        Severity::Suggest,
        "S005 severity must be Suggest (the collapse retired the \
         Info severity that S006 carried). got: {:?}",
        s005[0].severity
    );
}

// ---------------------------------------------------------------------------
// (d) All four supersession gates suppress S005.
// ---------------------------------------------------------------------------

#[test]
fn s005_supersession_bail_noforn_direct() {
    // Any portion carrying DissemControl::Nf supersedes REL TO at the
    // page level (CAPCO-2016 §H.8 + §H.9 mutual exclusion).
    // `PageContext::expected_rel_to` returns empty because of
    // supersession, not because of disjoint atom intersection;
    // firing S005 would surface a misleading "intersection produced
    // REL TO (empty)" diagnostic. The bail at the top of
    // `analyze_uncertain_reduction` prevents this.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//NF) portion one.\n\
                   (S//REL TO USA, GBR, RSMA) portion two.\n\
                   (S//REL TO USA, AUS, GBR) portion three.\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        0,
        "S005 must NOT fire when any portion carries NOFORN directly \
         (REL TO is superseded at the page level per §H.8 / §H.9). \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn s005_supersession_bail_sbu_nf_split() {
    // The non-IC SBU-NF/LES-NF classified-context split forces NF
    // injection at banner roll-up (CAPCO-2016 §H.9 p178 — SBU-NF;
    // §H.9 p185 — LES-NF). When that split fires `needs_nf` is true
    // and `analyze_uncertain_reduction` bails because REL TO is
    // superseded.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//SBU-NF) portion one.\n\
                   (S//REL TO USA, GBR, RSMA) portion two.\n\
                   (S//REL TO USA, AUS, GBR) portion three.\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        0,
        "S005 must NOT fire when SBU-NF classified-context split \
         forces NF injection at banner roll-up (§H.9 p178). \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn s005_supersession_bail_nodis() {
    // NODIS in any portion implies NOFORN in the banner per
    // CAPCO-2016 §H.9 p174 verbatim — "REL TO is not authorized in
    // the banner line if any portion contains NODIS information. In
    // this case, NOFORN would convey in the banner line."
    // `expected_non_ic_dissem.needs_nf` reflects this; the bail
    // suppresses S005.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//NODIS) portion one.\n\
                   (S//REL TO USA, GBR, RSMA) portion two.\n\
                   (S//REL TO USA, AUS, GBR) portion three.\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        0,
        "S005 must NOT fire when any portion carries NODIS \
         (§H.9 p174 — NOFORN supersedes REL TO at banner roll-up). \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

#[test]
fn s005_supersession_bail_exdis() {
    // EXDIS in any portion implies NOFORN in the banner per
    // CAPCO-2016 §H.9 p172 verbatim — "REL TO is not authorized in
    // the banner line if any portion contains EXDIS information. In
    // this case, NOFORN would convey in the banner line." Same shape
    // as NODIS; the bail suppresses S005.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//EXDIS) portion one.\n\
                   (S//REL TO USA, GBR, RSMA) portion two.\n\
                   (S//REL TO USA, AUS, GBR) portion three.\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        0,
        "S005 must NOT fire when any portion carries EXDIS \
         (§H.9 p172 — NOFORN supersedes REL TO at banner roll-up). \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// (e) Severity-config override silences via `[rules] S005 = "off"`.
// ---------------------------------------------------------------------------

#[test]
fn s005_severity_config_off_silences_rule() {
    // The engine's per-rule severity override is the production
    // surface for opting out: `[rules] S005 = "off"` in
    // `.marque.toml`. Verify the override reaches the
    // PageFinalization dispatch — the all-Off short-circuit in
    // `dispatch_page_finalization` returns Ok(()) before invoking
    // any rule when every PageFinalization rule's resolved severity
    // is `Off`, but a single S005-off override still leaves W004 to
    // fire on a JOINT-disunity page, so we need to verify
    // *specifically* that S005 stops firing (rather than checking
    // total diagnostics drops to zero).
    //
    // Arrange.
    let mut overrides = HashMap::new();
    // Rule-override key uses the wire-string form.
    overrides.insert(
        "capco:page.dissem.rel-to-uncertain-reduction".to_owned(),
        "off".to_owned(),
    );
    let mut config = Config::default();
    config.rules = RuleConfig { overrides };
    let engine = CapcoEngine::with_clock(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("engine with S005 override must construct");
    let source = b"(S//REL TO USA, GBR, RSMA) portion one.\n\
                   (S//REL TO USA, AUS, GBR) portion two.\n\
                   SECRET//REL TO USA, GBR\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        0,
        "[rules] S005 = \"off\" must silence S005 entirely. \
         diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// (f) No duplicate emission with closing banner.
// ---------------------------------------------------------------------------

#[test]
fn s005_fires_exactly_once_per_page_when_banner_closes_page() {
    // Page closes with a Banner candidate AND has a triggering REL
    // TO disjunction. If the engine main loop's Banner-candidate
    // dispatch ran every registered rule including PageFinalization
    // rules, S005 (a PageFinalization rule) would emit twice (once at
    // the Banner candidate via the main loop, once via
    // `dispatch_page_finalization` at the next PageBreak / EOD). The
    // engine skips PageFinalization rules in the main loop via
    // `pass_finalization_rule_indices` — exactly one S005 per page.
    //
    // Mirrors `w004_fires_exactly_once_per_page_when_banner_closes_page`.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source: &[u8] = b"(S//REL TO USA, GBR, RSMA) portion one.\n\
                          (S//REL TO USA, AUS, GBR) portion two.\n\
                          SECRET//REL TO USA, GBR\n\
                          \x0c\
                          (S//REL TO USA, FRA, RSMA) page 2 portion 1.\n\
                          (S//REL TO USA, DEU, FRA) page 2 portion 2.\n\
                          SECRET//REL TO USA, FRA\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        2,
        "S005 must fire EXACTLY twice on a two-page document where \
         each page has a closing banner (one per page, not four). \
         A count of 4 indicates the main-loop phase filter regressed \
         and S005 dispatched on both Banner candidates AND both \
         PageFinalization synthesis points. diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// (g) Audit content-ignorance (Constitution V Principle V).
// ---------------------------------------------------------------------------

#[test]
fn s005_diagnostic_carries_no_document_text() {
    // Audit content-ignorance (Constitution V Principle V): no document
    // text in any audit surface. This is STRUCTURALLY enforced by the
    // closed-template / closed-args invariants. `Diagnostic.message`
    // is a `Message` (template + args); the args are constrained to
    // `Option<TokenId>` / `Option<CategoryId>` / `Option<Span>` and a
    // handful of other typed-identifier fields — raw bytes are
    // unrepresentable. `Diagnostic.citation` is a typed `Citation`
    // (struct), no longer a free-form string.
    //
    // The original prose-substring check ("RSMA must appear in
    // message") no longer applies — the canonical-vocabulary token
    // identification now flows via `args.token: Option<TokenId>`
    // (the closed-set analog), not a substring. The "prose-sentinel
    // must not appear" check is preserved as a defense-in-depth
    // measure but is now structurally trivial: there is no way for a
    // `Message` to contain the prose sentinel under the closed-set
    // shape. The test purpose strengthens.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let prose_sentinel = "PROSE_SENTINEL_LEAKED_INTO_S005";
    let source = format!(
        "{prose_sentinel} (S//REL TO USA, GBR, RSMA) portion one.\n\
         {prose_sentinel} (S//REL TO USA, AUS, GBR) portion two.\n\
         SECRET//REL TO USA, GBR\n"
    );

    // Act.
    let lint = engine.lint(source.as_bytes());

    // Assert.
    let s005 = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .expect("S005 must fire on RSMA-uncertain page");
    // Closed-template identification: S005 fires under
    // `NonCanonicalOrder` with `CAT_REL_TO` category (see
    // `crates/capco/src/rules.rs` `S005_CITATION` + emitter).
    use marque_capco::scheme::CAT_REL_TO;
    use marque_rules::MessageTemplate;
    assert_eq!(
        s005.message.template(),
        MessageTemplate::NonCanonicalOrder,
        "S005 fires under the NonCanonicalOrder template; got {:?}",
        s005.message.template(),
    );
    assert_eq!(
        s005.message.args().category,
        Some(CAT_REL_TO),
        "S005 must identify the REL TO axis; got {:?}",
        s005.message.args().category,
    );
    // Defense-in-depth: assert no prose sentinel leaks into the
    // rendered template label or citation Display. With the closed-
    // set shape both are structurally byte-bounded, but the check
    // costs nothing and would surface a regression if the closure
    // ever relaxed.
    let template_label = s005.message.template().as_str();
    assert!(
        !template_label.contains(prose_sentinel),
        "content-ignorance violation: template label {template_label:?} leaked prose sentinel"
    );
    let citation_render = format!("{}", s005.citation);
    assert!(
        !citation_render.contains(prose_sentinel),
        "content-ignorance violation: citation render {citation_render:?} leaked prose sentinel"
    );
}

// ---------------------------------------------------------------------------
// (h) Boundary-anchor span is well-defined.
// ---------------------------------------------------------------------------

#[test]
fn s005_span_is_zero_length_boundary_anchor() {
    // PageFinalization rules receive `ctx.candidate_span` as a
    // zero-length `Span(boundary_offset, boundary_offset)` — either
    // at a `MarkingType::PageBreak` byte offset or at `source.len()`
    // for the EOD dispatch. PageContext does not store per-portion
    // spans, so refining to a specific offending portion isn't
    // possible without an engine-side extension. Pin the contract.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//REL TO USA, GBR, RSMA) portion one.\n\
                   (S//REL TO USA, AUS, GBR) portion two.\n";
    let source_len = source.len();

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005 = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .expect("S005 must fire on the disjoint REL TO portions");
    assert_eq!(
        s005.span.start, s005.span.end,
        "S005 span must be zero-length (PageFinalization boundary \
         anchor). got: start={}, end={}",
        s005.span.start, s005.span.end
    );
    // The only PageBreak in this fixture is the implicit EOD;
    // expect the span to land at source.len() (the EOD anchor).
    assert_eq!(
        s005.span.start, source_len,
        "S005 span must land at source.len() (EOD boundary) for a \
         document without form-feeds. got: start={}, source.len()={}",
        s005.span.start, source_len
    );
}

// ---------------------------------------------------------------------------
// (i) Empty-page short-circuit — no S005 on banner-only document.
// ---------------------------------------------------------------------------

#[test]
fn s005_does_not_fire_on_empty_page() {
    // A document with only a banner (no portions) has an empty
    // PageContext at EOD — `dispatch_page_finalization`'s caller
    // guards on `!page_context.is_empty()` and skips the dispatch
    // entirely. No rule body runs and no S005 emits. Same invariant
    // as `w004_does_not_fire_on_empty_page` in
    // `joint_disunity_collapse.rs`.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"SECRET//REL TO USA, GBR\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        0,
        "S005 must NOT fire on a banner-only document with no \
         portions (empty-page short-circuit in \
         `dispatch_page_finalization`'s caller). diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// (j) Trigraph filter — pure-trigraph REL TO portions don't fire.
// ---------------------------------------------------------------------------

#[test]
fn s005_does_not_fire_on_pure_trigraph_portions() {
    // ISO 3166-1 alpha-3 trigraphs (USA, GBR, FRA, …) aren't in the
    // ISMCAT tetragraph taxonomy by definition, so
    // `is_decomposable(trigraph)` returns `None` — but trigraphs are
    // atomic by ISO convention, not uncertain. The `s.len() != 3`
    // filter in `analyze_uncertain_reduction` excludes them
    // explicitly so the `is_none()` gate doesn't mis-classify them
    // as S005 triggers.
    //
    // Arrange.
    let engine = engine_with_fixed_clock();
    let source = b"(S//REL TO USA, GBR) portion one.\n\
                   (S//REL TO USA, FRA) portion two.\n\
                   SECRET//REL TO USA\n";

    // Act.
    let lint = engine.lint(source);

    // Assert.
    let s005_count = lint
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "page.dissem.rel-to-uncertain-reduction")
        .count();
    assert_eq!(
        s005_count,
        0,
        "S005 must NOT fire on pure-trigraph REL TO portions \
         (trigraph filter: trigraphs are atomic by ISO convention, \
         not uncertain). diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

// Issue #722 — additional S005 behavioral pins ported from the
// quarantined `_disabled_tests.rs` live in a sibling file
// (`s005_pagefinalization_ports_722.rs`) to keep this file under
// the 800-line coding-style gate. The sibling carries the
// not-ported / structurally-subsumed dispositions in its header
// for the message-content-assertion group obsoleted by the
// audit content-ignorance closure.
