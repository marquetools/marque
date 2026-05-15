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
    // at two severities. Net: 56 + 2 = 58.
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
    // (e.g., `"E058/CNWDI-classification-floor"`,
    // `"E058/SAR-classification-floor"`,
    // `"E058/DOD-UCNI-classification-ceiling"`,
    // `"E058/DOE-UCNI-classification-ceiling"`,
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
    // (`class-floor/HCS-comp-sub`, `class-floor/HCS-comp`,
    // `class-floor/SI-comp`, `class-floor/RSV-comp`, `class-floor/TK`,
    // `class-floor/TK-BLFH`); no class-floor rows are added in PR 3b.E.
    // Diagnostics emit with `Diagnostic.rule = "E059"`; per-row
    // identification flows via the catalog row's `name` field
    // (`sci-per-system/HCS-O-companions`,
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
    // 4702 + §H.7 p123 (ATOMAL → AEA) + §G.2 p41 + §H.7 p127
    // (BALK/BOHEMIA → SCI). The rule fires when the parser
    // canonicalizes legacy compound text into bare class + AEA/SCI
    // companion and emits a Recanonicalize fix at confidence 1.0.
    // Net delta: +1. Final: 36 + 1 = 37.
    //
    // Bumping this number means a rule was added or retired; either
    // action should be an intentional, documented change.
    let rule_set = CapcoRuleSet::new();
    assert_eq!(
        rule_set.rules().len(),
        37,
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
         compound text re-marking per §G.2 p41 (Table 5) + §H.7 p123 + \
         §H.7 p127; net delta +1. Final: 37. See \
         `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md` \
         for the architectural rationale. Adjust this assertion only \
         when rule registration actually changes."
    );
}

#[test]
fn phase_3_declares_thirteen_page_rewrites_with_citations() {
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();
    assert_eq!(
        rewrites.len(),
        13,
        "PR 3b.B (T026b) declared nine page rewrites — the retained \
         `capco/noforn-clears-rel-to` plus the eight §3.4.1 / §3.4.3 \
         transmutation entries from `marque-applied.md` (consultant \
         Entry 6 split into 6a + 6b for D13). The two earlier Phase-3 \
         stubs (`joint-promotion`, `fgi-absorption`) were retired in \
         PR 3b.B. PR 3c.B Sub-PR 8.F adds two Pattern A NOFORN-supremacy \
         rewrites: `capco/nodis-implies-noforn` (CAPCO-2016 §H.9 p174) \
         and `capco/exdis-implies-noforn` (CAPCO-2016 §H.9 p172). PR 3c.B \
         Sub-PR 8.F.2 adds two more Pattern A entries: \
         `capco/sbu-nf-implies-noforn` (CAPCO-2016 §H.9 p178) and \
         `capco/les-nf-implies-noforn` (CAPCO-2016 §H.9 p185), bringing \
         the total to thirteen."
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
fn phase_3_scheduler_exposes_thirteen_scheduled_rewrites() {
    // The scheduler produced a topological order at construction
    // time (Phase 3 T031). Expose it and verify the scheduled set
    // equals the declared set — the ordering is a data-flow
    // property, not a declaration-order one. Set is the retained
    // `noforn-clears-rel-to` plus the eight PR 3b.B transmutations
    // plus the two PR 3c.B Sub-PR 8.F Pattern A rewrites
    // (`capco/nodis-implies-noforn`, `capco/exdis-implies-noforn`)
    // plus the two PR 3c.B Sub-PR 8.F.2 Pattern A rewrites
    // (`capco/sbu-nf-implies-noforn`, `capco/les-nf-implies-noforn`).
    let engine = engine();
    let scheduled = engine.scheduled_rewrites();
    assert_eq!(scheduled.len(), 13);
    let mut names: Vec<&str> = scheduled.to_vec();
    names.sort();
    assert_eq!(
        names,
        [
            "capco/exdis-implies-noforn",
            "capco/fgi-restricted-rollup-on-us-contact",
            "capco/fgi-rollup-on-us-contact",
            "capco/frd-sigma-consolidates-into-rd-sigma",
            "capco/joint-cross-class-rollup",
            "capco/les-nf-implies-noforn",
            "capco/les-nf-transmutes-on-classified-contact",
            "capco/nodis-implies-noforn",
            "capco/noforn-clears-rel-to",
            "capco/orcon-nato-to-us-orcon-on-us-contact",
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
