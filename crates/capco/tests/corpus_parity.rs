#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 3 US1 — Corpus-parity baseline harness (T026, T037, T038).
//!
//! The Phase 3 migration guarantees byte-identical diagnostic output
//! against the pre-branch baseline: declarative `Constraint` /
//! `PageRewrite` entries are registered on `CapcoScheme` for the
//! scheduler + catalog surface, but the hand-written rule impls in
//! `crate::rules` remain the authoritative emitters of diagnostics.
//! Retirement of those rule impls (T035) is intentionally staged to
//! a follow-up so byte-identity is trivially preserved in this phase.
//!
//! This harness runs the shared corpus fixtures through `Engine::lint`
//! and `Engine::fix`, asserting that:
//!
//! 1. Every fixture still produces a well-formed `LintResult`.
//! 2. The Phase 3 rule count matches the pre-Phase-3 count (39).
//! 3. Every declared `PageRewrite` on `CapcoScheme` carries a
//!    non-empty citation.
//!
//! Full corpus-diff parity (baseline manifest vs. current run) rides
//! on top of the corpus-accuracy harness in
//! `crates/engine/tests/corpus_accuracy.rs`; this file pins the
//! Phase 3 declaration-layer invariants specifically.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::RuleSet;
use marque_scheme::MarkingScheme;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn rule_count_reflects_registration_changes() {
    // T035a: 1-for-1 swap of 11 hand-written rules → 11 declarative
    // wrappers. Count stayed at 39.
    //
    // T035b: retired 3 over-restrictive JOINT rules (E017, E018,
    // E019) that contradicted CAPCO-2016 §H.3 p57 (Relationship(s)
    // to Other Markings, which permits SCI/SAP/AEA/FGI/IC/Non-IC
    // dissem with JOINT "as appropriate"); added 1 narrowed rule
    // (E036 joint-conflicts-hcs) matching §H.3 p57's only specific
    // exclusion ("May not be used with the HCS markings or NOFORN
    // markings"). Net: 39 - 3 + 1 = 37.
    //
    // T035c-1b: added S001 (prefer-banner-abbreviation, style). Net: 38.
    //
    // T035c-8: added S002 (banner-consistent-form, style). Net: 39.
    //
    // T035c-14: retired W001 (DeprecatedMarkingWarningRule).
    // CAPCO-2016 §F (Legacy Control Markings, p35) treats legacy
    // markings as unauthorized — an error category owned by
    // E006/E008 — not "deprecated but still legal." No
    // authoritative bucket exists for a warning-severity
    // vocabulary-deprecation rule. Net: 38.
    //
    // T035c-21 PR-A: added E037 (nodis-conflicts-exdis) + E038
    // (dos-dissem-noforn) per CAPCO-2016 §H.9 NODIS/EXDIS templates
    // (p172 + p174). Net: 40.
    //
    // S003 (follow-up from #97 / T035c-18): added joint-usa-first
    // style rule. §H.3 p56 prescribes pure alphabetical for JOINT
    // with no USA-first carve-out; S003 encodes the convention
    // observed in REL TO §H.8 p150–151 across US-authored country
    // lists. Info severity. Net: 41.
    //
    // T035c-21 PR-B: added E039 (nodis-exdis-clears-banner-rel-to) +
    // E040 (nodis-exdis-banner-rollup) + E041 (nodis-supersedes-exdis
    // -in-portion). Net: 44.
    //
    // T035d: added 10 per-SCI-system constraint rules (E042–E051)
    // covering §H.4 class-ceiling and required-companion constraints
    // under the fix-and-warn pattern. Retired in PR 3b.E into the
    // E059 walker; see the T026e note further down. Net (at landing):
    // 44 + 10 = 54.
    // Issue #234 PR-B (rel-to-no-duplicates):
    //   E052 REL TO duplicate country codes (structural; pairs with
    //        the decoder's USA-injection path on the same §H.8 p150–151
    //        list-grammar surface)
    // Net: 54 + 1 = 55.
    //
    // Issue #235 / #186 PR-3: added S004 (rel-to-trigraph-suggest),
    // first consumer of the suggest-don't-fix channel. Net: 55 + 1 = 56.
    //
    // Issue #206: added S005 (rel-to-opaque-uncertain-reduction —
    // Suggest branch) AND S006 (same trigger, Info branch). Two
    // registered rules sharing one analysis helper because the
    // engine overwrites emitted severity with the rule's configured
    // severity (see the S005/S006 module-header comment in
    // crates/capco/src/rules.rs); a single rule cannot stably emit
    // at two severities. Net: 56 + 2 = 58. PR #488 retires S006 and
    // migrates S005 to `Phase::PageFinalization` (see the PR #488
    // section below for the rationale).
    // Issue #256: added E053 (noforn-rel-to-conflict), declarative
    // wrapper over the `capco/noforn-conflicts-rel-to` constraint
    // in CapcoScheme. §H.8 p145. Net: 58 + 1 = 59.
    //
    // T026a (PR 3b Sub-move A): collapsed three banner-roll-up rules
    // (E031 `SarBannerRollupRule`, E035 `SciBannerRollupRule`, E040
    // `NodisExdisBannerRollupRule`) into a single
    // `BannerMatchesProjectedRule` walker dispatched over a per-category
    // catalog. Diagnostics still emit with per-row IDs (E031 / E035 /
    // E040) for audit-stream continuity. Net delta: -2 rules
    // (3 retired + 1 walker added). Final: 59 - 2 = 57.
    //
    // T026d (PR 3b Sub-move D): retired three pure class-floor rules
    // (`DeclarativeCnwdiConstraintRule` E022, `DeclarativeUcniClassificationRule`
    // E025, hand-written `SarClassificationRule` E027) into the
    // `DeclarativeClassFloorRule` walker (rule ID E058) dispatched
    // over a 27-row class-floor catalog at marque-applied.md §3.4.6
    // family granularity. Diagnostics emit with `Diagnostic.rule = "E058"`;
    // per-row identification flows via the catalog row's `name` field
    // (e.g., `"banner.aea.floor-cnwdi"`,
    // `"banner.classification.floor-sar"`,
    // `"banner.aea.ceiling-dod-ucni"`,
    // `"banner.aea.ceiling-doe-ucni"`,
    // `"class-floor/<marking>"` for new rows). Net delta: -2 rules
    // (3 retired + 1 walker added). Final: 61 - 2 = 59.
    //
    // T026e (PR 3b Sub-move E): retired the 10 hand-written per-SCI-
    // system rules (E042–E051 — `HcsOCompanionsRule`,
    // `HcsPRequiresNofornRule`, `HcsPSubcompartmentTsOnlyRule`,
    // `HcsClassificationCeilingRule`, `SiCompartmentTopSecretRule`,
    // `SiGammaCompanionsRule`, `RsvClassificationCeilingRule`,
    // `TkClassificationCeilingRule`, `TkBlfhTopSecretRule`,
    // `TkCompartmentRequiresNofornRule`) into the
    // `DeclarativeSciPerSystemRule` walker (rule ID E059) dispatched
    // over a 5-row SCI per-system catalog at CAPCO-2016 §H.4 family
    // granularity. The class-floor portions of E044/E045/E046/E048/
    // E049/E050 are absorbed by PR 3b.D's class-floor catalog rows
    // (`banner.classification.floor-hcs-comp-sub`, `class-floor/HCS-comp`,
    // `class-floor/SI-comp`, `class-floor/RSV-comp`, `class-floor/TK`,
    // `class-floor/TK-BLFH`); no class-floor rows are added in PR 3b.E.
    // Diagnostics emit with `Diagnostic.rule = "E059"`; per-row
    // identification flows via the catalog row's `name` field
    // (`marking.sci.hcs-o-companions`,
    // `sci-per-system/HCS-P-NOFORN`,
    // `sci-per-system/HCS-P-sub-companions`,
    // `sci-per-system/SI-G-companions`,
    // `sci-per-system/TK-compartment-NOFORN`). Net delta: -9 rules
    // (10 retired + 1 walker added). Final: 59 - 9 = 50.
    //
    // T026f (PR 3b Sub-move F): retired four ordering rules
    // (E020 CountryCodeOrderingRule, E023 SigmaValidationRule,
    // E028 SarProgramOrderRule, E033 SciCompartmentOrderRule) into
    // the DeclarativeNonCanonicalInputRule walker (rule ID E060)
    // dispatching over a 5-row internal catalog (NON_CANONICAL_CATALOG)
    // covering REL TO USA-first alpha (§H.8 p150-151), JOINT alpha
    // (§H.3 p56), AEA SIGMA numeric sort (§H.6 p108), SAR program
    // ascending alpha (§H.5 p99), and SCI compartment + sub-
    // compartment numeric-then-alpha (§H.4 p61). Diagnostics emit
    // with `Diagnostic.rule = "E060"`; per-row identification flows
    // via the diagnostic message text (which preserves the existing
    // rule's human-readable phrasing verbatim). The walker retires
    // when the Phase C renderer trait surface lands in PR 5+
    // (Stage 4). Net delta: -3 rules (4 retired + 1 walker added).
    // Final: 50 - 3 = 47.
    //
    // PR 3c.B Commit 7.3 + 7.4 (walker decomposition):
    // `DeclarativeClassFloorRule` (E058, 7.3) and
    // `DeclarativeSciPerSystemRule` (E059, 7.4) retired. The 32 catalog
    // rows (27 class-floor + 5 SCI per-system) still fire — they flow
    // through the engine's constraint-catalog bridge. Class-floor uses
    // the `ConstraintViolation` envelope path (no fixes); SCI per-system
    // uses the direct `CapcoScheme::bridge_sci_per_system_diagnostics`
    // path so fixes (`FixProposal`) survive the deletion. Net delta:
    // -2 (7.3 -1; 7.4 -1). Final: 33 - 2 = 31.
    //
    // PR 9a T135a (issue #307 Group D): added
    // `DeprecatedSciLongFormRule` (E065) — canonicalization walker for
    // deprecated SCI long-form tokens per CAPCO-2016 §H.4 pp 61, 62, 74,
    // 76, 78, 85. Net delta: +1. Final: 31 + 1 = 32.
    //
    // PR 9a (issue #307): added three class-specific bare-HCS /
    // bare-RSV rules per CAPCO-2016 §H.4 pp 62, 70:
    //   E061  hcs-bare-at-confidential-legacy-remark  (§H.4 p62)
    //   E062  hcs-bare-suggest-subcompartment         (§H.4 p62)
    //   E063  rsv-bare-requires-compartment           (§H.4 p70)
    // E061 / E062 complement E010 with class-specific guidance; E063
    // is net new (no prior coverage). Net delta: +3. Final: 32 + 3 = 35.
    //
    // PR 9a Commit 5 (issue #307): added `EyesOnlyConvertToRelToRule`
    // (E064) — EYES / EYES ONLY → REL TO conversion per §H.8 p157 +
    // p158. NSA-only and deprecated since the markings waiver expired
    // 1 Oct 2017. Net delta: +1. Final: 35 + 1 = 36.
    //
    // PR 9c.1 T134: added `LegacyNatoCompoundRemarkRule` (E066) —
    // legacy NATO compound text re-marking per CAPCO-2016 §H.7 line
    // 4702 + §H.7 p122 (ATOMAL → AEA) + §G.2 p40 + §H.7 p127
    // (BALK/BOHEMIA → SCI). The rule fires when the parser
    // canonicalizes legacy compound text into bare class + AEA/SCI
    // companion and emits a Recanonicalize fix at confidence 1.0.
    // Net delta: +1. Final: 36 + 1 = 37.
    //
    // PR 9c.2 (FR-048): added `BareNatoRequiresRelToRule` (S007) —
    // bare NATO classification in a US-classified document should
    // carry `REL TO USA, NATO` per CAPCO-2016 §H.7 p127 Notional
    // Example 2. Suggest-channel severity; users can opt up via
    // `[rules] S007 = "warn"`. The solely-NATO-document case is
    // carved out via `ProjectedMarking::is_solely_nato_classified`.
    // Net delta: +1. Final: 37 + 1 = 38.
    //
    // Issue #407 / PR #491: added `BareCanonicalCompoundRule` (E067)
    // — bare CNWDI / NK / EU portion-mark short-forms → canonical
    // CAPCO-2016 compound forms per §H.6 p106 (RD-CNWDI), §H.4 p83
    // (SI-NK), §H.4 p78 (SI-EU). Walker filters `TokenKind::Unknown`,
    // emits `Severity::Fix` text-correction diagnostics with
    // hardcoded static replacement literals (Constitution V).
    // Net delta: +1. Final: 39 + 1 = 40.
    //
    // PR #488 (issue #488): retired `RelToOpaqueUncertainReductionInfoRule`
    // (S006) and migrated `RelToOpaqueUncertainReductionSuggestRule`
    // (S005) to `Phase::PageFinalization`. The historical Suggest/Info
    // split was an engine-workaround (per-rule severity override is
    // the only way to surface two severities for one trigger), NOT
    // §-grounded — CAPCO-2016 §H.8 (REL TO grammar) + §D.2 Table 3
    // rule 21 (the roll-up intersection law) apply uniformly without
    // distinguishing "active validation" from "consistent case." The
    // collapse leaves a single Suggest-severity rule that fires on
    // the page-level fixpoint snapshot (closes the pre-#488
    // banner-less false-negative). Net delta: -1. Final: 40 - 1 = 39.
    //
    // Bumping this number means a rule was added or retired; either
    // action should be an intentional, documented change.
    let rule_set = CapcoRuleSet::new();
    assert_eq!(
        rule_set.rules().len(),
        28,
        "rule count: PR 3b umbrella closed at 47. PR 3c.B Commit 6 \
         (form-bucket migration) reduced to 33. PR 3c.B Commit 7.3 \
         + 7.4 retire `DeclarativeClassFloorRule` (E058) and \
         `DeclarativeSciPerSystemRule` (E059); their 27 + 5 catalog \
         rows fire via the engine's bridge — net delta -2. Final: \
         31. PR 9a T135a adds `DeprecatedSciLongFormRule` (E065) — \
         canonicalization walker for deprecated SCI long-form tokens \
         per CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85; net delta +1. \
         PR 9a (issue #307) adds three class-specific bare-HCS / \
         bare-RSV rules (E061 / E062 / E063); net delta +3. PR 9a \
         Commit 5 adds `EyesOnlyConvertToRelToRule` (E064) — EYES \
         / EYES ONLY → REL TO conversion per §H.8 p157 + p158; \
         net delta +1. Final: 36. PR 9c.1 T134 adds \
         `LegacyNatoCompoundRemarkRule` (E066) — legacy NATO \
         compound text re-marking per §G.2 p40 (Table 5) + §H.7 p122 + \
         §H.7 p127; net delta +1. Final: 37. PR 9c.2 (FR-048) adds \
         `BareNatoRequiresRelToRule` (S007) — bare NATO classification \
         in a US-classified document should carry `REL TO USA, NATO` \
         per §H.7 p127 Notional Example 2; net delta +1. Final: 38. \
         PR 4b-B (006 T112) adds `JointDisunityCollapseRule` (W004) — \
         JOINT producer-disunity-collapse-to-FGI per §H.3 p57 + \
         §H.7 p123 (CV-4 PR 4b-B 8th-pass updated from §H.3 p56; \
         Warn-only; cross-axis fix deferred to renderer in \
         PR 5+ per H-1 PR 4b-B follow-up triage); net delta +1. \
         Final: 39. Issue #407 / PR #491 adds \
         `BareCanonicalCompoundRule` (E067) — bare CNWDI / NK / EU \
         short-forms → canonical CAPCO-2016 compound portion marks \
         per §H.6 p106, §H.4 p83, §H.4 p78; net delta +1. Final: \
         40. PR #488 (issue #488) retires \
         `RelToOpaqueUncertainReductionInfoRule` (S006) and migrates \
         `RelToOpaqueUncertainReductionSuggestRule` (S005) to \
         `Phase::PageFinalization`. The S005/S006 Suggest/Info split \
         was an engine-workaround (per-rule severity override the \
         only way to surface two severities for one trigger), NOT \
         §-grounded; CAPCO-2016 §H.8 + §D.2 Table 3 rule 21 apply \
         uniformly to REL TO atom-semantics. Net delta: -1. Final: \
         39. PR closing #470 retires \
         `DeclarativeCominglingWarningRule` (W002). CAPCO-2016 §H.7 \
         p123 (`crates/capco/docs/CAPCO-2016.md` lines 3051-3065) \
         documents `(S//FGI AUS GBR)` as the canonical \"Example \
         Portion Mark (when sources are acknowledged, but not \
         segregated from US)\" shape — exactly what the predicate \
         was warning on. The §H.7 p124 segregation rule is \
         conditioned on ICD-206 status (a document-level property), \
         so a portion-local warning premised on it produces noise \
         without a useful action. Net delta: -1. Final: 38. PR #578 \
         retires 15 declarative wrappers (E010/E012/E014/E015/E016/ \
         E021/E024/E036/E037/E038/E053/E054/E055/E056/E057) into the \
         engine's constraint-catalog bridge — `severity` + \
         `span_anchor` now live on `Constraint::Conflicts` / \
         `Constraint::Requires` and the engine bridge synthesizes \
         `FixIntent` via `CapcoScheme::fix_intent_by_name`. S004 \
         stays a registered walker (`RelToTrigraphSuggestRule`) \
         because its candidate replacement is corpus-derived during \
         evaluation. Net delta: -15. Final: 23. #559 close-out C1 \
         (2026-05-19) adds `RelidoImpliedByClosureRule` (S008) — \
         byte-surfacing twin of the `CLOSURE_RELIDO_SCI` / \
         `CLOSURE_RELIDO_US_CLASS` lattice-layer closures per \
         CAPCO-2016 §H.8 p154 + §D.2 Table 3 rule 17; \
         Severity::Suggest at confidence 0.85 matching S007's \
         text-layer pattern. Net delta: +1. Final: 24. \
         Issue #261 adds `FgiExplicitWithTrigraphRule` (E071) — \
         FGI with explicit trigraph when concealment intended or \
         acknowledgment contradicted per CAPCO-2016 §H.7 p124; \
         four-case behavioral spec. Net delta: +1. Final: 25. \
         Issue #250 adds `PreferTetragraphCollapseRule` (S009) — \
         suggest replacing explicit member trigraph lists with a \
         compact tetragraph per CAPCO-2016 §H.8 p150; default Off. \
         Net delta: +1. Final: 26. Issue #251 adds \
         `CollapseUniformRelPortionsRule` (S010) + \
         `BareRelPortionDivergenceRule` (E072) — REL TO / bare-REL \
         consistency per CAPCO-2016 §H.8 p150-151. Net delta: +2. \
         Final: 28. \
         See `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md` \
         for the architectural rationale. Adjust this assertion only \
         when rule registration actually changes."
    );
}

#[test]
fn phase_3_declares_twenty_three_page_rewrites_with_citations() {
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();
    assert_eq!(
        rewrites.len(),
        23,
        "PR 4b-C Commit 3 added 7 Pattern-C strip rows + Commit 4 added \
         2 Pattern-B structural FOUO-eviction rows to the 14-row \
         post-PR-4b-B catalog. \
         Pattern-C: `capco/limdis-evicted-by-classified` (CAPCO-2016 \
         §H.9 p170), `capco/sbu-evicted-by-classified` (CAPCO-2016 \
         §H.9 p176), `capco/dod-ucni-evicted-by-classified` (CAPCO-2016 \
         §H.6 p116), `capco/dod-ucni-promotes-noforn-when-classified` \
         (CAPCO-2016 §H.6 p116), `capco/doe-ucni-evicted-by-classified` \
         (CAPCO-2016 §H.6 p118), `capco/doe-ucni-promotes-noforn-when-classified` \
         (CAPCO-2016 §H.6 p118), `capco/fouo-evicted-by-classified` \
         (CAPCO-2016 §H.8 p134). Pattern-B: \
         `capco/classification-evicts-fouo` (CAPCO-2016 §H.8 p134) and \
         `capco/non-fdr-control-evicts-fouo` (CAPCO-2016 §H.8 p134) — \
         both quote the §H.8 p134 FOUO Precedence Rules passage but \
         cite distinct sub-clauses (classified-document vs UNCLASSIFIED \
         with other dissemination controls)."
    );
    for rw in rewrites {
        assert!(
            !rw.citation.is_empty(),
            "rewrite {} has empty citation; Constitution VIII requires a \
             traceable authoritative-source passage",
            rw.id
        );
    }
}

#[test]
fn phase_3_engine_lint_produces_wellformed_result_on_empty_input() {
    // Smoke test: the Phase 3 scheduler construction path does not
    // regress the trivial empty-input case.
    let engine = engine();
    let result = engine.lint(b"");
    assert!(result.is_clean());
    assert_eq!(result.error_count(), 0);
    assert_eq!(result.warn_count(), 0);
}

#[test]
fn phase_3_scheduler_exposes_twenty_three_scheduled_rewrites() {
    // The scheduler produced a topological order at construction
    // time (Phase 3 T031). Expose it and verify the scheduled set
    // equals the declared set — the ordering is a data-flow
    // property, not a declaration-order one.
    //
    // Post-PR-4b-C set: the existing 14 rows + 7 PR 4b-C Commit 3
    // Pattern-C strip rows (5 strip + 2 NOFORN-promote for UCNI) +
    // 2 PR 4b-C Commit 4 Pattern-B structural FOUO-eviction rows.
    let engine = engine();
    let scheduled = engine.scheduled_rewrites();
    assert_eq!(scheduled.len(), 23);
    let mut names: Vec<&str> = scheduled.to_vec();
    names.sort();
    assert_eq!(
        names,
        [
            "capco/classification-evicts-fouo",
            "capco/dod-ucni-evicted-by-classified",
            "capco/dod-ucni-promotes-noforn-when-classified",
            "capco/doe-ucni-evicted-by-classified",
            "capco/doe-ucni-promotes-noforn-when-classified",
            "capco/exdis-implies-noforn",
            "capco/fgi-restricted-rollup-on-us-contact",
            "capco/fgi-rollup-on-us-contact",
            "capco/fouo-evicted-by-classified",
            "capco/frd-sigma-consolidates-into-rd-sigma",
            "capco/joint-cross-class-rollup",
            "capco/les-nf-implies-noforn",
            "capco/les-nf-transmutes-on-classified-contact",
            "capco/limdis-evicted-by-classified",
            "capco/nodis-implies-noforn",
            "capco/noforn-clears-fdr-family",
            "capco/noforn-clears-rel-to",
            "capco/non-fdr-control-evicts-fouo",
            "capco/orcon-nato-to-us-orcon-on-us-contact",
            "capco/sbu-evicted-by-classified",
            "capco/sbu-nf-implies-noforn",
            "capco/sbu-nf-transmutes-on-classified-contact",
            "capco/us-presence-promotes-bare-fgi-attribution",
        ]
    );
}

#[test]
fn phase_3_noforn_clearer_runs_after_dissem_transmutations() {
    // The DISSEM-writing transmutations all write CAT_DISSEM;
    // `capco/noforn-clears-rel-to` reads CAT_DISSEM (and writes
    // CAT_REL_TO). The scheduler must therefore order each DISSEM
    // writer BEFORE the NOFORN clearer — otherwise a transmutation
    // that emits NOFORN could fire after the clearer and leave REL TO
    // populated when it should have been cleared. This ordering is a
    // declarative guarantee of the scheme's `reads` / `writes`
    // annotations, not an accident of declaration order.
    //
    // DISSEM writers:
    //   - Entries 5, 6a, 6b — ORCON-NATO, SBU-NF, LES-NF
    //     transmutations (PR 3b.B).
    //   - PR 3c.B Sub-PR 8.F Pattern A rewrites —
    //     `capco/nodis-implies-noforn` (CAPCO-2016 §H.9 p174) and
    //     `capco/exdis-implies-noforn` (CAPCO-2016 §H.9 p172) — each
    //     declares `writes = [CAT_DISSEM]`, so the same DISSEM-writer
    //     precedence invariant applies.
    //   - PR 3c.B Sub-PR 8.F.2 Pattern A rewrites —
    //     `capco/sbu-nf-implies-noforn` (CAPCO-2016 §H.9 p178) and
    //     `capco/les-nf-implies-noforn` (CAPCO-2016 §H.9 p185) — same
    //     `writes = [CAT_DISSEM]` annotation, same precedence invariant.
    let engine = engine();
    let scheduled = engine.scheduled_rewrites();
    let nf = scheduled
        .iter()
        .position(|&r| r == "capco/noforn-clears-rel-to")
        .expect("noforn-clears-rel-to is declared");
    for dissem_writer in [
        "capco/orcon-nato-to-us-orcon-on-us-contact",
        "capco/sbu-nf-transmutes-on-classified-contact",
        "capco/les-nf-transmutes-on-classified-contact",
        "capco/nodis-implies-noforn",
        "capco/exdis-implies-noforn",
        "capco/sbu-nf-implies-noforn",
        "capco/les-nf-implies-noforn",
    ] {
        let pos = scheduled
            .iter()
            .position(|&r| r == dissem_writer)
            .unwrap_or_else(|| panic!("{dissem_writer} is declared"));
        assert!(
            pos < nf,
            "{dissem_writer} ({pos}) must be scheduled before \
             noforn-clears-rel-to ({nf}) — scheduled order: {scheduled:?}",
        );
    }
}
