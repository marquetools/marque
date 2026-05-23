// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// Quarantined dead test block from `rules.rs` pre-#561 split.
// `#[cfg(any())]` makes this permanently unreachable; preserved
// for disposition decision in issue #722.
// DO NOT add new tests here. New tests go in
// `crates/capco/tests/` integration files or `mod tests` blocks
// inside their rule's submodule.

#[cfg(any())] // PR 3c.B Commit 10: inline tests reading legacy FixProposal fields disabled pending rewrite.
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_capco_test_support::{lint_banner, lint_portion};

    #[test]
    fn capco_rule_set_registers_all_rules() {
        let set = CapcoRuleSet::new();
        let ids: Vec<&str> = set.rules().iter().map(|r| r.id().as_str()).collect();
        // Kept rules.
        assert!(ids.contains(&"E002"));
        assert!(ids.contains(&"E005"));
        assert!(ids.contains(&"E006"));
        assert!(ids.contains(&"E007"));
        assert!(ids.contains(&"E008"));
        assert!(ids.contains(&"E010"));
        assert!(ids.contains(&"E012"));
        assert!(ids.contains(&"E014"));
        assert!(ids.contains(&"E015"));
        // W002 retired in the PR closing #470 — CAPCO §H.7 p123
        // authorizes the canonical commingled shape this rule was
        // firing on; the §H.7 p124 segregation rule is doc-level
        // (ICD-206 status) and unenforceable portion-local.
        assert!(!ids.contains(&"W002"));
        assert!(ids.contains(&"E016"));
        assert!(ids.contains(&"E021"));
        assert!(ids.contains(&"E024"));
        assert!(ids.contains(&"W003"));
        assert!(ids.contains(&"C001"));
        assert!(ids.contains(&"E031")); // BannerMatchesProjectedRule walker
        assert!(ids.contains(&"W034"));
        assert!(ids.contains(&"E036"));
        assert!(ids.contains(&"E037"));
        assert!(ids.contains(&"E038"));
        assert!(ids.contains(&"E039"));
        assert!(ids.contains(&"E041"));
        assert!(ids.contains(&"S003"));
        assert!(ids.contains(&"S005"));
        assert!(ids.contains(&"S006"));
        assert!(ids.contains(&"E053"));
        assert!(ids.contains(&"E054"));
        assert!(ids.contains(&"E055"));
        assert!(ids.contains(&"E056"));
        assert!(ids.contains(&"E057"));
        // PR 3c.B Commit 7.3: `DeclarativeClassFloorRule` (E058) retired.
        // The 27 catalog rows fire through the engine's constraint-
        // catalog bridge with `Diagnostic.rule = "E058"` (audit-stream
        // continuity); no registered `Rule::id() == "E058"` post-7.3.
        assert!(
            !ids.contains(&"E058"),
            "E058 walker retired in PR 3c.B Commit 7.3; the catalog rows \
             emit via the engine bridge."
        );
        // PR 3c.B Commit 7.4: `DeclarativeSciPerSystemRule` (E059) retired.
        // The 5 catalog rows fire through the bridge's direct path
        // (`CapcoScheme::bridge_sci_per_system_diagnostics`) with
        // `Diagnostic.rule = "E059"` and full `FixProposal` payloads;
        // no registered `Rule::id() == "E059"` post-7.4.
        assert!(
            !ids.contains(&"E059"),
            "E059 walker retired in PR 3c.B Commit 7.4; the catalog rows \
             emit via the engine bridge with fixes intact."
        );

        // Retired-rule guards. PR 3c.B Commit 6 retires 13 rules + the
        // E060 walker into `MarkingScheme::render_canonical` (form-bucket
        // migration). See `docs/plans/2026-05-10-pr3c-consolidated-plan.md`
        // lines 788–862 for the architectural commitment.
        assert!(
            !ids.contains(&"E001"),
            "E001 retired in PR 3c.B Commit 6 — portion-mark-in-banner \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E003"),
            "E003 retired in PR 3c.B Commit 6 — block ordering absorbed \
             by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E004"),
            "E004 retired in PR 3c.B Commit 6 — separator normalization \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E009"),
            "E009 retired in PR 3c.B Commit 6 — banner→portion abbrev \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"S001"),
            "S001 retired in PR 3c.B Commit 6 — banner-abbrev preference \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"S002"),
            "S002 retired in PR 3c.B Commit 6 — banner-form consistency \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E011"),
            "E011 retired in PR 3c.B Commit 6 — //-prefix normalization \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E013"),
            "E013 retired in PR 3c.B Commit 6 — list-delimiter \
             normalization absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E026"),
            "E026 retired in PR 3c.B Commit 6 — SAR portion form \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E029"),
            "E029 retired in PR 3c.B Commit 6 — SAR compartment order \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E030"),
            "E030 retired in PR 3c.B Commit 6 — SAR indicator repetition \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E032"),
            "E032 retired in PR 3c.B Commit 6 — SCI sort order absorbed \
             by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E052"),
            "E052 retired in PR 3c.B Commit 6 — REL TO duplicates \
             absorbed by MarkingScheme::render_canonical"
        );
        assert!(
            !ids.contains(&"E060"),
            "E060 (non-canonical input walker) retired in PR 3c.B Commit \
             6 — its 5 ordering rows are absorbed by \
             MarkingScheme::render_canonical"
        );

        // Pre-existing retirement guards (still valid).
        assert!(!ids.contains(&"W001"), "W001 retired in T035c-14");
        assert!(!ids.contains(&"E017"), "E017 retired in T035b");
        assert!(!ids.contains(&"E018"), "E018 retired in T035b");
        assert!(!ids.contains(&"E019"), "E019 retired in T035b");
        assert!(
            !ids.contains(&"E020"),
            "E020 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E022"),
            "E022 retired in PR 3b.D into E058 catalog"
        );
        assert!(
            !ids.contains(&"E023"),
            "E023 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E025"),
            "E025 retired in PR 3b.D into E058 catalog"
        );
        assert!(
            !ids.contains(&"E027"),
            "E027 retired in PR 3b.D into E058 catalog"
        );
        assert!(
            !ids.contains(&"E028"),
            "E028 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E033"),
            "E033 retired in PR 3b.F (and now via E060 in PR 3c.B Commit 6)"
        );
        assert!(
            !ids.contains(&"E035"),
            "E035 retired as a registered rule ID by T026a; emitted as a \
             per-row catalog ID by BannerMatchesProjectedRule"
        );
        assert!(
            !ids.contains(&"E040"),
            "E040 retired as a registered rule ID by T026a; emitted as a \
             per-row catalog ID by BannerMatchesProjectedRule"
        );
        for retired_e042_to_e051 in [
            "E042", "E043", "E044", "E045", "E046", "E047", "E048", "E049", "E050", "E051",
        ] {
            assert!(
                !ids.contains(&retired_e042_to_e051),
                "{retired_e042_to_e051} retired in PR 3b.E into the E059 SCI per-system walker"
            );
        }

        // Post-PR-3c.B-Commit-7.4 registered count: 31 rules.
        // History: PR 3b umbrella closed at 47. PR 3c.B Commit 6 retires
        // 13 form rules + 1 walker (E060) into the renderer (form-bucket
        // migration); 47 - 14 = 33. PR 3c.B Commit 7.3 retires
        // `DeclarativeClassFloorRule` (E058) into the constraint-catalog
        // bridge; 33 - 1 = 32. PR 3c.B Commit 7.4 retires
        // `DeclarativeSciPerSystemRule` (E059) into the bridge's direct
        // path (with fixes preserved); 32 - 1 = 31.
        assert_eq!(set.rules().len(), 31);
    }

    #[test]
    fn e002_fires_when_usa_missing_with_real_span() {
        let src_str = "SECRET//REL TO GBR, AUS";
        let diags = lint_banner(src_str);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        // Span covers the full REL TO trigraph list (first → last), not
        // just the first trigraph — required so `Engine::fix` can splice
        // the full list with the canonical replacement in one step.
        assert_eq!(e002[0].span.as_str(src_str.as_bytes()).unwrap(), "GBR, AUS");
    }

    // T035c-10: fix canonicalization — E002's replacement must produce
    // the fully canonical REL TO list (USA first + non-USA entries
    // alphabetical per CAPCO-2016 §H.8 p151) in a single pass. This
    // is required because E020 gates on `rel_to[0] == USA` and so is
    // silent whenever E002 fires; if E002's fix preserved input order,
    // the output would still carry a latent alphabetical-ordering
    // violation that only a second pass would catch.

    #[test]
    fn e002_fix_sorts_non_usa_trigraphs_when_usa_missing() {
        // USA absent and non-USA entries in non-alphabetical order.
        // Canonical form: USA, AUS, GBR.
        let diags = lint_banner("SECRET//REL TO GBR, AUS");
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a FixProposal");
        assert_eq!(
            fix.replacement.as_ref(),
            "USA, AUS, GBR",
            "E002 must produce canonical REL TO (USA first + alphabetical rest)"
        );
    }

    #[test]
    fn e002_fix_sorts_non_usa_trigraphs_when_usa_misplaced() {
        // USA present but not first, and non-USA entries unsorted.
        // Canonical form: USA, AUS, GBR.
        let diags = lint_banner("SECRET//REL TO GBR, USA, AUS");
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a FixProposal");
        assert_eq!(
            fix.replacement.as_ref(),
            "USA, AUS, GBR",
            "E002 must produce canonical REL TO in one pass: {}",
            fix.replacement.as_ref()
        );
    }

    // T035c-10 second-round review fixes: trailing-delimiter tail
    // consumption and multi-block suppression.

    #[test]
    fn e002_fix_consumes_trailing_comma_in_rel_to_block() {
        // `REL TO GBR, AUS,` has a trailing `,` inside the RelToBlock.
        // Splicing only `GBR, AUS` (first→last trigraph) would leave
        // the trailing `,` behind: `REL TO USA, AUS, GBR,` — still
        // malformed. The fix span must extend through the delimiter
        // tail so the rewritten banner is clean.
        let src = "SECRET//REL TO GBR, AUS,";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a fix");
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "GBR, AUS,",
            "fix span must cover the delimiter-only tail so splicing \
             leaves no stale `,`/whitespace behind"
        );
    }

    #[test]
    fn e002_fix_span_includes_recognized_tetragraph_tail() {
        // Issue #183 PR-A: tetragraphs (FVEY, ACGU, NATO, …) are now
        // first-class `CountryCode` values, recognized by
        // `is_trigraph` and stored in `rel_to`. The E002 fix span
        // (first→last `RelToTrigraph` token within the block) must
        // therefore extend through FVEY in the tail — splicing
        // `GBR, AUS` only would leave a stale `, FVEY` behind. Pre-
        // PR-A this test asserted the inverse (FVEY was silently
        // dropped at the parser, so the splice intentionally stopped
        // at AUS); the inverse is now wrong.
        let src = "SECRET//REL TO GBR, AUS, FVEY";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a fix");
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "GBR, AUS, FVEY",
            "tetragraph FVEY is now a recognized country code (issue #183) \
             — the fix span must include it",
        );
    }

    #[test]
    fn e002_fix_span_stops_at_unrecognized_tail_token() {
        // Companion to `e002_fix_span_includes_recognized_tetragraph_tail`
        // — the defensive invariant that a non-recognized tail token
        // is NOT swallowed by the splice still holds. Issue #183 PR-A
        // widened recognition from trigraphs to all CVE country codes
        // (incl. tetragraphs and the longer registered codes), but
        // anything outside that vocabulary still fails the
        // `is_trigraph` gate at the parser, never gets a
        // `RelToTrigraph` token span, and so the fix span stops at
        // the last recognized code. `XYZQ` here is a 4-char string
        // outside the CVE TRIGRAPHS list.
        let src = "SECRET//REL TO GBR, AUS, XYZQ";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(e002.len(), 1);
        let fix = e002[0].fix.as_ref().expect("E002 must carry a fix");
        assert_eq!(
            fix.span.as_str(src.as_bytes()).unwrap(),
            "GBR, AUS",
            "unrecognized tail token must not be swallowed by the splice"
        );
    }

    #[test]
    fn e002_suppresses_fix_on_multiple_rel_to_blocks() {
        // If the parser sees more than one REL TO block in a marking,
        // a single first→last splice would delete intervening `//...//`
        // content (here `//NF//`). The rule must emit a diagnostic
        // without a FixProposal so the engine cannot corrupt the
        // source.
        let src = "SECRET//REL TO GBR//NF//REL TO AUS";
        let diags = lint_banner(src);
        let e002: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E002").collect();
        assert_eq!(
            e002.len(),
            1,
            "E002 must still fire (diagnostic present): {diags:?}"
        );
        assert!(
            e002[0].fix.is_none(),
            "E002 must NOT carry a fix when multiple REL TO blocks \
             are present (cross-block splice would delete intervening \
             `//NF//`): {e002:?}"
        );
    }

    #[test]
    fn e002_fix_output_does_not_trigger_e020() {
        // Apply E002's fix as the new input and confirm E020 stays silent —
        // this is the invariant that lets E020 gate on `rel_to[0] == USA`.
        let diags_round1 = lint_banner("CONFIDENTIAL//REL TO FRA, DEU");
        let e002: Vec<_> = diags_round1
            .iter()
            .filter(|d| d.rule.as_str() == "E002")
            .collect();
        assert_eq!(e002.len(), 1);
        let fixed = e002[0].fix.as_ref().unwrap().replacement.as_ref();
        assert_eq!(fixed, "USA, DEU, FRA");

        // Round 2: feed the canonicalized REL TO back through the linter;
        // neither E002 nor the E060 walker (REL TO row) should fire on
        // the rewritten banner.
        let round2_banner = format!("CONFIDENTIAL//REL TO {fixed}");
        let diags_round2 = lint_banner(&round2_banner);
        assert!(
            diags_round2
                .iter()
                .all(|d| d.rule.as_str() != "E002" && d.rule.as_str() != "E060"),
            "E002's canonical output must not fire E002 or E060: {diags_round2:?}"
        );
    }

    #[test]
    fn e005_fires_on_declass_exemption_in_banner() {
        let diags = lint_banner("SECRET//25X1//NOFORN");
        let e005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E005").collect();
        assert_eq!(e005.len(), 1);
        let src = b"SECRET//25X1//NOFORN";
        assert_eq!(e005[0].span.as_str(src).unwrap(), "25X1");
    }

    // T035c-16: E005 audit — scope expansion and citation lockdown.

    #[test]
    fn e005_fires_on_declass_exemption_in_portion() {
        // Portion-scope coverage: CAPCO §D.1 p27's closed category list
        // for banners is mirrored for portions (§C.1 p26 lines 525ff),
        // so `25X1` between `//` separators in a portion is just as
        // misplaced as in a banner. Before T035c-16 this fired nothing
        // (the rule was banner-only); the audit extended scope to cover
        // portions.
        let diags = lint_portion("(S//25X1//NF)");
        let e005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E005").collect();
        assert_eq!(
            e005.len(),
            1,
            "E005 must fire on declass exemption inside a portion: {diags:?}"
        );
        let src = b"(S//25X1//NF)";
        assert_eq!(e005[0].span.as_str(src).unwrap(), "25X1");
    }

    #[test]
    fn e005_citation_points_at_specific_sections() {
        // Lock down the T035c-16 citation retargeting — `§E.1 p31` and
        // `§D.1 p27` are the specific passages that jointly establish
        // the invariant. A future regression that drifts to a bare
        // `§E` would pass Constitution VIII's surface check but fail
        // re-verifiability, which is the whole point.
        let diags = lint_banner("SECRET//25X1//NOFORN");
        let e005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E005").collect();
        assert_eq!(e005.len(), 1);
        // PR 10.A.1: typed Citation pins the primary anchor (§E.1 p31 —
        // "Declassify On is a CAB line"). The cross-reference to §D.1
        // p27 (banner categories exclude declassification) is documented
        // in the rule's doc-comment but is not representable as a
        // second § on a single typed Citation; the brief's "Multi-page
        // citation decision" applies.
        assert_eq!(
            e005[0].citation,
            capco(SectionLetter::E, 1, 31),
            "E005 citation must anchor at §E.1 p31 (Declassify On is a CAB line); \
             got: {:?}",
            e005[0].citation
        );
        // PR 10.A.1 Commit 4: secondary-passage cross-reference to
        // §D.1 p27 lives on `super::DECLASSIFY_MISPLACED_CROSS_REFS`. The companion
        // assertion (`DECLASSIFY_MISPLACED_CROSS_REFS.contains(...)`) is in the
        // dedicated `citation_cross_refs_tests` (bottom of this file) integration test
        // — this inline `mod tests` block is `#[cfg(any())]`-gated
        // dead code, so adding the guard here would not run. See
        // the cross-refs test file for the pin.
    }

    #[test]
    fn e008_fires_on_unknown_token() {
        let diags = lint_banner("SECRET//XYZZY//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        assert_eq!(e008.len(), 1);
        let src = b"SECRET//XYZZY//NOFORN";
        assert_eq!(e008[0].span.as_str(src).unwrap(), "XYZZY");
    }

    #[test]
    fn looks_like_deprecated_x_shorthand_matches_expected_patterns() {
        use super::looks_like_deprecated_x_shorthand as m;
        // Deprecated forms (must match)
        assert!(m("25X1-"));
        assert!(m("25X2-"));
        assert!(m("25X9-"));
        assert!(m("50X1-"));
        assert!(m("50X1-HUM-"));
        assert!(m("25X3-WMD-"));
        // Canonical forms (must NOT match — no trailing dash)
        assert!(!m("25X1"));
        assert!(!m("50X1-HUM"));
        // Malformed / unrelated
        assert!(!m(""));
        assert!(!m("-"));
        assert!(!m("X1-"));
        assert!(!m("25-X1-"));
        assert!(!m("25X-"));
        assert!(!m("ABCX1-"));
        assert!(!m("25X1-hum-"), "lowercase suffix should not match");
        assert!(!m("NOFORN"));
    }

    #[test]
    fn e007_fires_on_pattern_matched_x_shorthand_not_in_migration_table() {
        // `25X2-` is NOT in the seed MIGRATIONS table. Before the pattern
        // fallback, this would have fallen through to E008. Now E007
        // should fire with a confidence of 0.95 and a replacement of
        // `25X2` (trailing `-` stripped).
        let diags = lint_banner("SECRET//25X2-//NOFORN");
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert_eq!(e007.len(), 1);
        let fix = e007[0].fix.as_ref().expect("E007 must carry a fix");
        assert_eq!(fix.replacement.as_ref(), "25X2");
        assert!((fix.confidence.combined() - 0.95).abs() < f32::EPSILON);
        // E008 must NOT also fire on the same span.
        assert!(diags.iter().all(|d| d.rule.as_str() != "E008"));
    }

    #[test]
    fn e007_still_fires_on_migration_table_entries() {
        // The existing 25X1- path (table-backed) must still work.
        let diags = lint_banner("SECRET//25X1-//NOFORN");
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert_eq!(e007.len(), 1);
        let fix = e007[0].fix.as_ref().unwrap();
        assert_eq!(fix.replacement.as_ref(), "25X1");
        // Table confidence from the seed MIGRATIONS entry (0.97).
        assert!((fix.confidence.combined() - 0.97).abs() < f32::EPSILON);
    }

    #[test]
    fn migrations_table_contains_no_fouo_entry() {
        // FOUO remains a valid CAPCO dissem control per CVEnumISMDissem.xml
        // and CAPCO-2016 §F. CUI is a separate (NARA) marking system, not a
        // CAPCO dissem control. A prior `FOUO → CUI` migration entry was
        // removed as factually incorrect; this regression guard prevents
        // re-introduction. Any future "suggest CUI on non-IC documents"
        // behavior must live in a CUI adapter gated by opt-in config.
        use marque_ism::generated::migrations::find_migration;
        assert!(
            find_migration("FOUO").is_none(),
            "FOUO must not appear in MIGRATIONS (see crates/ism/build.rs doc block)"
        );
    }

    #[test]
    fn migrations_table_contains_no_limdis_entry() {
        // LIMDIS is a current non-IC dissem control (CAPCO-2016 §H.9).
        // A prior `LIMDIS → RELIDO` migration entry was removed as
        // factually incorrect; this regression guard prevents
        // re-introduction.
        use marque_ism::generated::migrations::find_migration;
        assert!(
            find_migration("LIMDIS").is_none(),
            "LIMDIS must not appear in MIGRATIONS (see crates/ism/build.rs doc block)"
        );
    }

    #[test]
    fn e006_does_not_fire_on_fouo_in_banner() {
        // Full-pipeline regression: the absence of a FOUO migration entry
        // must produce no E006 diagnostic in a banner containing FOUO.
        // The policy question "FOUO in a classified banner" is handled at
        // the PageContext roll-up (FOUO drops from classified banners) and
        // in Phase C as a declarative `Constraint::Conflicts(FOUO, Classified)`.
        let diags = lint_banner("UNCLASSIFIED//FOUO");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E006"),
            "E006 must not fire on FOUO: {diags:?}"
        );
    }

    #[test]
    fn e008_no_fix_offered() {
        let diags = lint_banner("SECRET//XYZZY//NOFORN");
        let e008 = diags.iter().find(|d| d.rule.as_str() == "E008").unwrap();
        assert!(e008.fix.is_none(), "FR-012: E008 must not propose a fix");
    }

    // T035c-12: pin-down tests for E008's four suppression paths,
    // plus regression guards that confirm E008 still fires when expected.

    #[test]
    fn e008_suppressed_on_migration_backed_unknown() {
        // `25X1-` is an Unknown token that the seed MIGRATIONS table
        // captures. E007 owns X-shorthand; E008 must step aside AND
        // E007 must actually fire — otherwise a future change that
        // breaks E007's migration lookup could produce a silent
        // suppression with no diagnostic at all.
        let diags = lint_banner("SECRET//25X1-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert!(
            e008.is_empty(),
            "E008 must be suppressed for migration-backed X-shorthand \
             (E007 owns this path): {diags:?}"
        );
        assert!(
            !e007.is_empty(),
            "E007 must fire for migration-backed X-shorthand — \
             otherwise suppression is a silent drop: {diags:?}"
        );
    }

    #[test]
    fn e008_suppressed_on_pattern_matched_x_shorthand() {
        // `25X9-` is not in the seed MIGRATIONS table but matches the
        // X-shorthand pattern E007 catches via fallback. E008 must
        // still step aside — see the suppression path 2 in the rule
        // doc comment. Also assert that E007 actually fires so this
        // cannot regress into a silent drop where E008 is suppressed
        // but no owning diagnostic is emitted.
        let diags = lint_banner("SECRET//25X9-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        let e007: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E007").collect();
        assert!(
            e008.is_empty(),
            "E008 must be suppressed for pattern-matched X-shorthand \
             even when not in seed MIGRATIONS (E007 owns): {diags:?}"
        );
        assert!(
            !e007.is_empty(),
            "E007 must fire for pattern-matched X-shorthand — \
             otherwise suppression is a silent drop: {diags:?}"
        );
    }

    #[test]
    fn e008_fires_on_malformed_first_sar_with_empty_program() {
        // `SAR-` alone (no program identifier) fails SAR grammar. The
        // parser does not produce a `SarMarking`, so `attrs.sar_markings`
        // stays `None` and `SarIndicatorRepeatRule::check` returns early
        // at its `attrs.sar_markings.is_none()` guard. An earlier
        // version of E008's suppression matched on prefix only, so this
        // malformed token was silently dropped. Tightening the
        // suppression to require `attrs.sar_markings.is_some()` AND a
        // non-empty suffix restores the E008 error.
        let diags = lint_banner("SECRET//SAR-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        assert!(
            !e008.is_empty(),
            "E008 must fire on malformed first SAR (empty program) — \
             E030 cannot run without a successful first SAR, so E008 \
             is the only rule that can surface this: {diags:?}"
        );
    }

    #[test]
    fn e008_fires_on_malformed_first_spelled_sar_with_empty_program() {
        // Same regression as above for the `SPECIAL ACCESS REQUIRED-`
        // prefix. `SPECIAL ACCESS REQUIRED-` with no program must not
        // be silently dropped.
        let diags = lint_banner("SECRET//SPECIAL ACCESS REQUIRED-//NOFORN");
        let e008: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E008").collect();
        assert!(
            !e008.is_empty(),
            "E008 must fire on malformed first `SPECIAL ACCESS \
             REQUIRED-` (empty program): {diags:?}"
        );
    }

    #[test]
    fn no_diagnostics_on_clean_banner() {
        let diags = lint_banner("TOP SECRET//SI//NOFORN");
        assert!(
            diags.is_empty(),
            "clean banner should produce no diagnostics, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostics_on_clean_portion() {
        let diags = lint_portion("(S//NF)");
        // Both "S" and "NF" are correct portion-form abbreviations.
        // E001 must not fire (not a banner), and E009 must not fire
        // (already using abbreviated forms).
        assert!(
            diags.is_empty(),
            "clean portion should produce no diagnostics, got: {diags:?}"
        );
    }

    // --- S003: joint-usa-first (style, follow-up from #97) ---
    // Rule S003 detects JOINT lists that don't lead with USA.
    #[test]
    fn s003_fires_on_joint_list_without_usa_first() {
        let attrs = CanonicalAttrs {
            classification: Some(MarkingClassification::Joint(
                vec![
                    CountryCode::try_new("GBR").unwrap(),
                    CountryCode::try_new("USA").unwrap(),
                ]
                .into_boxed_slice(),
            )),
            ..CanonicalAttrs::default()
        };
        let ctx = RuleContext::default();
        let rule = super::JointUsaFirstRule;
        let diags = <super::JointUsaFirstRule as Rule<CapcoScheme>>::check(&rule, &attrs, &ctx);
        assert_eq!(diags.len(), 1, "S003 must fire: {diags:?}");
    }

    #[test]
    fn s003_does_not_fire_when_usa_already_first() {
        let diags = lint_banner("//JOINT S USA GBR AUS//REL TO USA, AUS, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire when USA is already first: {diags:?}"
        );
    }

    #[test]
    fn s003_does_not_fire_without_usa_in_joint_list() {
        // Anomalous per §H.3 p163 (USA always in JOINT), but
        // S003 only fires when USA IS present but not first. Other
        // rules flag the missing-USA case.
        let diags = lint_banner("//JOINT S GBR AUS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire when USA is absent: {diags:?}"
        );
    }

    #[test]
    fn s003_does_not_fire_on_single_country_joint() {
        // Single-country JOINT (just USA) — nothing to reorder.
        let diags = lint_banner("//JOINT S USA");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire on single-country JOINT: {diags:?}"
        );
    }

    #[test]
    fn s003_does_not_fire_in_portion() {
        // S003 is banner-only, matching S001/S002's scope. Portion-
        // form JOINT is rarely used; convention-based style rules
        // are banner-focused.
        let diags = lint_portion("(//JOINT S AUS GBR USA)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "S003"),
            "S003 must not fire in portion context: {diags:?}"
        );
    }

    #[test]
    fn s003_citation_frames_as_convention_not_mandate() {
        // Constitution VIII: the citation MUST make clear that S003
        // encodes convention, not a CAPCO mandate. §H.3 is explicitly
        // silent on USA-first. Lock the "IC convention" framing so a
        // regression that fabricates a §H.3 carve-out fails here.
        let diags = lint_banner("//JOINT S AUS GBR USA");
        let s003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S003").collect();
        assert_eq!(s003.len(), 1);
        let citation = s003[0].citation;
        // PR 10.A.1: typed `Citation` pins the primary anchor (§H.3 p56,
        // which prescribes pure-alpha JOINT ordering — the IC convention
        // S003 encodes is layered above this default). The cross-
        // reference to §H.8 pp 150-151 (REL TO USA-first convention)
        // and the "IC convention" framing both live in the rule's
        // doc-comment, not in the typed Citation; the brief's
        // "Multi-page citation decision" applies.
        assert_eq!(
            citation,
            capco(SectionLetter::H, 3, 56),
            "S003 citation must anchor at §H.3 p56 (pure-alpha JOINT \
             ordering); got: {citation:?}"
        );
        // PR 10.A.1 Commit 4: secondary-passage cross-reference to
        // §H.8 p150 lives on `super::JOINT_USA_FIRST_CROSS_REFS`. The companion
        // assertion is in `citation_cross_refs_tests` (bottom of this file) — see the
        // E005 site above for why this inline `mod tests` block
        // can't host the guard.
    }

    // --- S004: rel-to-trigraph-suggest (issue #235 / #186 PR-3) ---
    //
    // S004 surfaces a `Severity::Suggest` diagnostic when a REL TO
    // entry has a corpus-rare prior and a corpus-common 1- or 2-edit
    // neighbor. The fix is informational; the engine never auto-
    // applies a Suggest-severity diagnostic regardless of confidence.

    #[test]
    // --- S005: REL TO opaque-uncertain reduction (issue #206) ---
    //
    // Test fixtures use NA-deprecated tetragraphs from the V2022-NOV
    // taxonomy (RSMA, EUDA, BHTF) rather than the org-fork extension
    // example (`MNFI`) the plan §3.5 cites. Reason: org-fork
    // extensions live in `country_extensions.toml`, which ships
    // empty by default — a fixture using `MNFI` would require
    // populating extensions just for the test, polluting the
    // build-time data. NA-deprecated codes are in the CVE recognition
    // surface so the parser keeps them in `attrs.rel_to`, AND
    // `is_decomposable` returns `None` for them, which is exactly
    // the trigger condition S005 cares about. Both categories
    // produce identical runtime semantics; only the `{state}` text
    // in the diagnostic differs (covered by `s005_state_text_for_*`).
    #[test]
    fn s005_suggests_when_uncertain_drops_and_banner_has_no_rel_to() {
        // Two portions; RSMA appears in only one. Atom-semantics
        // intersection is {USA, GBR}; RSMA dropped. Banner has no
        // REL TO at all (NOFORN supersedes) — active validation
        // context per plan §3.1 → Suggest.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        assert_eq!(s005.len(), 1, "S005 must fire once on RSMA: {diags:?}");
        assert_eq!(
            s005[0].severity,
            marque_rules::Severity::Suggest,
            "banner has no REL TO ⇒ active validation ⇒ Suggest, got {:?}",
            s005[0].severity,
        );
        assert!(s005[0].fix.is_none(), "S005 emits no fix");
        assert!(
            s005[0].message.contains("RSMA"),
            "S005 message must name the uncertain code: {:?}",
            s005[0].message
        );
        assert!(
            s005[0].message.contains("AUS"),
            "S005 message must list 'other codes' that AUS could have entered \
             through RSMA's hypothetical membership: {:?}",
            s005[0].message
        );
    }

    #[test]
    fn s006_info_when_banner_equals_atom_intersection() {
        // Banner carries exactly the atom-semantics intersection.
        // expected = {USA, GBR}; banner_atomic = {USA, GBR}.
        // expected ⊆ banner ⇒ Info branch ⇒ S006 (not S005). S005
        // stays silent on this fixture; the engine-level severity
        // override flattening means S005 cannot also emit at Info.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA, GBR";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        let s006: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S006").collect();
        assert!(
            s005.is_empty(),
            "S005 must NOT fire when banner is consistent (S006 covers Info): {s005:?}"
        );
        assert_eq!(s006.len(), 1, "S006 must fire once: {diags:?}");
        assert_eq!(
            s006[0].severity,
            marque_rules::Severity::Info,
            "expected ⊆ banner ⇒ Info, got {:?}",
            s006[0].severity,
        );
        assert!(s006[0].fix.is_none(), "S006 emits no fix");
    }

    #[test]
    fn s006_info_when_banner_is_proper_superset_of_atom_intersection() {
        // Banner extends atom-semantics with FRA. The plan's
        // consistency check is `expected ⊆ banner`, not equality —
        // the operator may legitimately have membership data we
        // don't (Constitution VIII forbids invention of facts). FRA
        // pulled from outside is honored as Info (S006), not S005.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA, FRA, GBR";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        let s006: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S006").collect();
        assert!(s005.is_empty(), "S005 must NOT fire: {s005:?}");
        assert_eq!(s006.len(), 1, "S006 must fire: {diags:?}");
        assert_eq!(
            s006[0].severity,
            marque_rules::Severity::Info,
            "banner ⊇ expected (extras allowed) ⇒ Info"
        );
    }

    #[test]
    fn s005_suggests_when_banner_drops_a_code_atom_semantics_preserves() {
        // Banner is missing GBR which atom-semantics says must
        // survive. expected = {USA, GBR}; banner_atomic = {USA}.
        // expected ⊄ banner ⇒ Suggest — the safe default isn't met.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        let s005: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S005").collect();
        assert_eq!(s005.len(), 1, "S005 must fire: {diags:?}");
        assert_eq!(
            s005[0].severity,
            marque_rules::Severity::Suggest,
            "banner drops GBR ⇒ inconsistent ⇒ Suggest"
        );
    }

    /// Helper: count diagnostics for either rule of the
    /// S005/S006 pair (they share the trigger condition; only one
    /// of the two emits per banner candidate).
    fn count_s005_or_s006(diags: &[Diagnostic<CapcoScheme>]) -> usize {
        diags
            .iter()
            .filter(|d| matches!(d.rule.as_str(), "S005" | "S006"))
            .count()
    }

    #[test]
    fn s005_does_not_fire_when_uncertain_code_in_every_portion() {
        // RSMA in BOTH portions ⇒ survives atom-semantics
        // intersection. The atom result reflects RSMA's presence;
        // neither S005 nor S006 has anything to surface.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA, RSMA)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when uncertain code survives intersection: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_atom_by_authority_kfor() {
        // KFOR is `decomposable=No` — atom by authority.
        // `is_decomposable("KFOR") == Some(false)`, so the rule's
        // `is_none()` filter excludes it. Atom-semantics is the
        // correct answer: the code IS the recipient.
        let source = "(S//REL TO USA, GBR, KFOR)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on KFOR (decomposable=No): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_atom_by_authority_eu() {
        // EU is the 2-letter atom-by-authority special case. Same
        // logic as KFOR — `is_decomposable("EU") == Some(false)`,
        // filtered by the `is_none()` gate.
        let source = "(S//REL TO USA, GBR, EU)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on EU (decomposable=No): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_decomposable_known_fvey() {
        // FVEY is `decomposable=Yes` — atom-semantics expands to
        // {AUS, CAN, GBR, NZL, USA} before intersection. Both
        // portions get the same expanded set; intersection is
        // precise; no uncertainty to surface.
        let source = "(S//REL TO USA, FVEY)\n\
                      (S//REL TO USA, FVEY)\n\
                      SECRET//REL TO USA, FVEY";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on FVEY (decomposable=Yes): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_for_single_rel_to_portion() {
        // Only one portion has a non-empty REL TO list. No
        // intersection to compute; rule bails out at the
        // `portions_with_rel_to.len() < 2` guard.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//FOUO)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire with fewer than 2 REL TO portions: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_only_trigraphs_appear() {
        // Pure trigraph portions. The trigraph filter (`s.len() ==
        // 3`) excludes every code; uncertain_codes is empty;
        // diagnostic suppressed. ISO 3166-1 alpha-3 codes are atomic
        // by convention, not uncertain.
        let source = "(S//REL TO USA, GBR)\n\
                      (S//REL TO USA, AUS)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire on pure-trigraph fixtures \
             (trigraph filter): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_other_codes_set_is_empty() {
        // RSMA dropped, but every surviving atom IS in expected.
        // `other_codes` is empty — there's nothing the operator
        // might have intended to release to through RSMA's
        // hypothetical membership. Suppress.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must suppress when no 'other codes' to surface: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_non_ic_split_injects_nf() {
        // The non-IC SBU-NF/LES-NF split forces NF injection at
        // banner roll-up in classified documents (CAPCO-2016
        // §H.9). When that split fires,
        // `PageContext::expected_rel_to` returns empty even though
        // no portion carries `DissemControl::Nf` directly — REL TO
        // is superseded at the page level. Pin the second NOFORN
        // bail in `analyze_uncertain_reduction` (the `needs_nf`
        // branch — also covers NODIS/EXDIS portions per the
        // §H.9 p172 / p174 imply-NF extension landed in PR
        // 3c.B-8F-engine-gap; this test stays scoped to SBU-NF,
        // with separate tests below for the NODIS/EXDIS paths).
        //
        // Fixture: portion 1 has SBU-NF (the split trigger);
        // portions 2 and 3 have classified REL TO with an uncertain
        // code (RSMA). Without the bail, the rule would compute
        // `portions_with_rel_to.len() == 2`, `expected_set = {}`
        // (NF-injection supersession), and fire a misleading
        // "intersection produced REL TO (empty…)" diagnostic.
        let source = "(S//SBU-NF)\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN//SBU";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must bail when non-IC SBU-NF split forces NF \
             injection at banner roll-up: {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_a_portion_carries_noforn() {
        // Regression for Copilot review on PR #249: NOFORN supersedes
        // REL TO at the page level. `PageContext::expected_rel_to`
        // returns empty because the marking is superseded, not
        // because the atom intersection is empty — firing S005 in
        // that case produces a misleading "intersection produced
        // REL TO (empty…)" diagnostic. Pin the bail.
        //
        // Fixture: portion 1 has NOFORN, portions 2+3 have REL TO
        // with an uncertain code (RSMA). Pre-fix, the rule would
        // have computed `portions_with_rel_to.len() == 2`,
        // `expected_set = {}` (NOFORN supersession), and fired
        // S005 with empty-intersection wording. Post-fix, the
        // NOFORN check bails before any of that runs.
        let source = "(S//NF)\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when any portion carries NOFORN \
             (REL TO is superseded at the page level): {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_portion_has_nodis() {
        // PR 3c.B-8F-engine-gap regression: NODIS in any portion implies
        // NOFORN in the banner per CAPCO-2016 §H.9 p174 verbatim — "REL TO
        // is not authorized in the banner line if any portion contains
        // NODIS information. In this case, NOFORN would convey in the
        // banner line." `PageContext::expected_rel_to` now short-circuits
        // to empty when NODIS is present in any portion, and the
        // `needs_nf` bail in `analyze_uncertain_reduction` (lines
        // 2311-2314 after the rename) propagates this. Pin the bail.
        //
        // Fixture: portion 1 has NODIS only (NOT explicit `//NOFORN` — the
        // §H.9 p174 imply-NF semantics IS what we are testing; including
        // explicit NF in the portion would route the bail through the
        // pre-existing `any_portion_noforn` short-circuit at line 2303-
        // 2310 instead of the new `needs_nf` path, defeating the
        // regression purpose. Caught by Copilot review on this PR.).
        // Portions 2 and 3 have classified REL TO with an uncertain code
        // (RSMA). Pre-PR the rule would have computed
        // `portions_with_rel_to.len() == 2`, `expected_set = {}` (NODIS
        // supersession via `needs_nf`), and fired a misleading
        // "intersection produced REL TO (empty…)" diagnostic. Post-PR
        // the `needs_nf` bail stops it.
        let source = "(S//NODIS)\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NODIS//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when any portion carries NODIS \
             (REL TO is superseded at the page level per §H.9 p174): \
             {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_portion_has_exdis() {
        // PR 3c.B-8F-engine-gap regression: EXDIS analogue of the NODIS
        // test above. CAPCO-2016 §H.9 p172 verbatim — "REL TO is not
        // authorized in the banner line if any portion contains EXDIS
        // information. In this case, NOFORN would convey in the banner
        // line."
        //
        // Portion 1 carries EXDIS only — see the NODIS test above for
        // why explicit `//NOFORN` is intentionally omitted from the
        // portion (route the bail through the new `needs_nf` path, not
        // the pre-existing `any_portion_noforn` short-circuit).
        let source = "(S//EXDIS)\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//EXDIS//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when any portion carries EXDIS \
             (REL TO is superseded at the page level per §H.9 p172): \
             {diags:?}"
        );
    }

    #[test]
    fn s005_does_not_fire_when_other_codes_only_appear_alongside_x() {
        // Regression for Copilot review on PR #249: the previous
        // `union − expected − {X}` definition included atoms that
        // appeared only in the same portion as X. Such atoms can't
        // be hypothetically pulled in via X's membership — they're
        // already explicitly listed in the X-containing portion, so
        // their intersection survival depends on whether they also
        // appear in the OTHER portions, not on X's membership.
        //
        // Here GBR appears only alongside RSMA (in portion 1).
        // Portion 2 has only USA. atom-semantics intersection =
        // {USA}. RSMA dropped, but no atom in portions-without-X
        // (= {USA} only) is missing from expected. The rule must
        // stay silent. The pre-fix implementation would have
        // computed `other_codes = {USA, GBR, RSMA} − {USA} − {RSMA}
        // = {GBR}` and fired a false-positive Info diagnostic.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA)\n\
                      SECRET//REL TO USA";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when 'other codes' only appear \
             alongside X (post-Copilot-review fix): {diags:?}"
        );
    }

    #[test]
    fn s005_quotes_verbatim_taxonomy_description_for_na_description_codes() {
        // EUDA is `decomposable=NA` with `<Membership><Description>`
        // in V2022-NOV. The taxonomy carries verbatim ODNI text
        // ("As of 15 March 2016, disclosure request should be
        // referred to the original classification authority...").
        // Plan §3.3 requires that text to surface verbatim in the
        // diagnostic — Constitution V audit-content-ignorance is
        // satisfied because the text is ODNI taxonomy data, not
        // user-document content.
        let source = "(S//REL TO USA, GBR, EUDA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .unwrap_or_else(|| panic!("S005 must fire on EUDA: {diags:?}"));
        assert!(
            s005.message.contains("disclosure request"),
            "S005 must quote verbatim Description text for NA-Description codes; got: {:?}",
            s005.message
        );
        assert!(
            s005.message.contains("original classification authority"),
            "S005 must include the OCA-deferral phrase ODNI published: {:?}",
            s005.message
        );
    }

    #[test]
    fn s005_state_text_for_na_suppressed_code() {
        let text = super::s005_state_text("RSMA");
        assert!(
            text.contains("deprecated") && text.contains("suppressed"),
            "RSMA is NA-Suppressed; state text must say so: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_for_na_description_code() {
        let text = super::s005_state_text("EUDA");
        assert!(
            text.contains("deprecated"),
            "EUDA is NA; state text must mark it deprecated: {text:?}"
        );
        assert!(
            text.contains("original classification authority"),
            "EUDA Description text must reach state output: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_for_recursive_code() {
        let text = super::s005_state_text("BHTF");
        assert!(
            text.contains("recursive") || text.contains("out of scope"),
            "BHTF is NA-Members(recursive): {text:?}"
        );
    }

    #[test]
    fn s005_state_text_for_unknown_code() {
        // Code absent from V2022-NOV taxonomy entirely — represents
        // org-fork extensions or genuinely unknown codes.
        let text = super::s005_state_text("XYZW");
        assert!(
            text.contains("absent"),
            "unknown-code state text must mention absence: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_decomposable_yes_hits_defensive_fallback() {
        // FVEY is `decomposable="Yes"` / `membership_shape="Members"`
        // in V2022-NOV. The rule's outer `is_decomposable == None`
        // guard means the state-text helper is never called with
        // FVEY in production (S005's loop filters such codes out
        // before formatting), but the function is callable
        // directly and its catch-all arm `(decomp, shape) =>
        // format!(…)` is the defensive fallback if a future
        // taxonomy revision introduces a `(non-NA, *)` reachable
        // shape. Pin the fallback's format so the contract is
        // documented behavior.
        let text = super::s005_state_text("FVEY");
        assert!(
            text.contains("decomposable=\"Yes\""),
            "fallback must surface decomposable verbatim: {text:?}"
        );
        assert!(
            text.contains("membership_shape=\"Members\""),
            "fallback must surface membership_shape verbatim: {text:?}"
        );
        assert!(
            text.contains("ISMCAT V"),
            "fallback includes the ISMCAT_TETRA_VERSION preamble: {text:?}"
        );
    }

    #[test]
    fn s005_state_text_decomposable_no_hits_defensive_fallback() {
        // EU is `decomposable="No"` (atom by authority) in V2022-NOV.
        // Same defensive-fallback contract as the Yes case.
        let text = super::s005_state_text("EU");
        assert!(
            text.contains("decomposable=\"No\""),
            "fallback for No: {text:?}"
        );
        assert!(
            text.contains("membership_shape=\"Suppressed\""),
            "fallback for No (Suppressed shape): {text:?}"
        );
    }

    #[test]
    fn s005_handles_empty_atom_intersection() {
        // Disjoint REL TO portions ⇒ atom-semantics intersection
        // is empty (no shared codes), but the rule should still
        // surface the silent-loss case if uncertain codes drop and
        // there are other-portion atoms that would have been
        // pulled in by hypothetical membership. Pins the
        // empty-set arm of `expected_str` rendering
        // (`"(empty — atom intersection produced no shared codes)"`).
        //
        // Fixture is intentionally malformed (REL TO without USA
        // per §H.8) — that's the only way to land an empty atom
        // intersection in well-formed input. E002
        // (missing-USA-trigraph) will also fire on both portions;
        // its diagnostic is independent of S005's.
        let source = "(S//REL TO GBR, RSMA)\n\
                      (S//REL TO AUS)\n\
                      SECRET//NF";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .unwrap_or_else(|| {
                panic!("S005 must fire on empty-intersection RSMA fixture: {diags:?}")
            });
        assert!(
            s005.message.contains("(empty"),
            "expected empty-intersection wording in S005 message: {:?}",
            s005.message
        );
    }

    #[test]
    fn s005_multi_portion_uses_intersection_across_portions_without_x() {
        // Three portions: portion 1 carries X=RSMA; portions 2 and
        // 3 don't. `atoms_in_every_without_x` is the intersection of
        // p2's expansion = {USA, GBR} and p3's expansion = {USA, GBR}
        // = {USA, GBR}. After subtracting expected={USA} and {RSMA},
        // `other_codes = {GBR}` — non-empty, S005 fires. This
        // exercises the `for p in &portions_without_x[1..]` loop
        // body that the two-portion fixtures don't reach.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA, GBR)\n\
                      (S//REL TO USA, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .unwrap_or_else(|| panic!("S005 must fire on 3-portion RSMA fixture: {diags:?}"));
        assert!(
            s005.message.contains("GBR"),
            "S005 must surface GBR (intersect({{USA, GBR}}, {{USA, GBR}}) \
             − {{USA}} − {{RSMA}} = {{GBR}}): {:?}",
            s005.message
        );
        assert!(
            !s005.message.contains("RSMA, GBR") && !s005.message.contains("AUS"),
            "the two non-X portions are identical; only GBR should \
             reach other_codes: {:?}",
            s005.message
        );
    }

    #[test]
    fn s005_does_not_fire_when_portions_without_x_have_disjoint_atoms() {
        // Three portions: p1 has X=RSMA, p2 has GBR but not AUS,
        // p3 has AUS but not GBR. atoms_in_every_without_x =
        // intersect({USA, GBR}, {USA, AUS}) = {USA}. After
        // subtracting expected={USA} and {RSMA}, other_codes = {}.
        // The rule must stay silent — even hypothetically including
        // GBR or AUS in RSMA's membership wouldn't make either
        // survive intersection (the OTHER non-X portion lacks them).
        // This pins the intersection-vs-union semantics: a union
        // implementation would have produced other_codes={GBR, AUS}
        // and fired a false positive.
        let source = "(S//REL TO USA, RSMA)\n\
                      (S//REL TO USA, GBR)\n\
                      (S//REL TO USA, AUS)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        assert_eq!(
            count_s005_or_s006(&diags),
            0,
            "S005/S006 must not fire when portions-without-X have \
             disjoint atoms outside expected (intersection wipes \
             them): {diags:?}"
        );
    }

    #[test]
    fn s005_rule_trait_getters() {
        // Cover the `id` / `name` / `default_severity` accessors that
        // the inline-test harness's direct `rule.check()` calls
        // bypass. Engine-level tests exercise these too, but pinning
        // the contract here keeps the regression closer to the
        // implementation.
        let rule = super::RelToOpaqueUncertainReductionSuggestRule;
        assert_eq!(<_ as Rule<CapcoScheme>>::id(&rule).as_str(), "S005");
        assert_eq!(
            <_ as Rule<CapcoScheme>>::name(&rule),
            "rel-to-opaque-uncertain-reduction"
        );
        assert_eq!(
            <_ as Rule<CapcoScheme>>::default_severity(&rule),
            marque_rules::Severity::Suggest
        );
    }

    #[test]
    fn s006_rule_trait_getters() {
        let rule = super::RelToOpaqueUncertainReductionInfoRule;
        assert_eq!(<_ as Rule<CapcoScheme>>::id(&rule).as_str(), "S006");
        assert_eq!(
            <_ as Rule<CapcoScheme>>::name(&rule),
            "rel-to-opaque-uncertain-reduction-info"
        );
        assert_eq!(
            <_ as Rule<CapcoScheme>>::default_severity(&rule),
            marque_rules::Severity::Info
        );
    }

    #[test]
    fn s005_helpers_render_set_promotes_usa_and_alphabetizes_rest() {
        // `s005_render_set` produces the comma-separated string
        // S005/S006 messages embed for `expected_str` and
        // `other_str`. USA goes first; the rest alpha. Pin the
        // contract directly because the integration tests only
        // observe it through the diagnostic message wording.
        use std::collections::BTreeSet;
        let set: BTreeSet<&str> = ["GBR", "AUS", "USA", "FRA"].into_iter().collect();
        let rendered = super::s005_render_set(&set);
        assert_eq!(rendered, "USA, AUS, FRA, GBR");

        // No USA — pure alphabetical (BTreeSet already sorts the
        // input, so the join order matches insertion order).
        let no_usa: BTreeSet<&str> = ["GBR", "AUS", "FRA"].into_iter().collect();
        assert_eq!(super::s005_render_set(&no_usa), "AUS, FRA, GBR");

        // Empty set → empty string. The rule guards against this
        // path via the `expected_set.is_empty()` branch but pinning
        // the helper's behavior keeps the contract honest.
        let empty: BTreeSet<&str> = BTreeSet::new();
        assert_eq!(super::s005_render_set(&empty), "");
    }

    #[test]
    fn s005_helpers_expand_atomic_round_trips_through_tetragraph() {
        // `s005_expand_atomic` is the rule's view of "what trigraphs
        // does this REL TO list cover after tetragraph expansion?"
        // FVEY decomposes; opaque codes (RSMA, KFOR) and trigraphs
        // pass through unchanged. Direct unit test because the
        // integration tests don't observe the function's output
        // shape, only the downstream diagnostic.
        use marque_ism::CountryCode;
        use std::collections::BTreeSet;

        let rel_to: Vec<CountryCode> = ["USA", "FVEY"]
            .into_iter()
            .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
            .collect();
        let expanded = super::s005_expand_atomic(&rel_to);
        let expected: BTreeSet<&str> = ["USA", "AUS", "CAN", "GBR", "NZL"].into_iter().collect();
        assert_eq!(
            expanded, expected,
            "FVEY must expand to its 5 trigraph members + USA passthrough"
        );

        // Opaque tetragraph (RSMA NA-Suppressed) and trigraphs pass
        // through.
        let opaque: Vec<CountryCode> = ["USA", "RSMA"]
            .into_iter()
            .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
            .collect();
        let expanded_opaque = super::s005_expand_atomic(&opaque);
        let expected_opaque: BTreeSet<&str> = ["USA", "RSMA"].into_iter().collect();
        assert_eq!(expanded_opaque, expected_opaque);
    }

    #[test]
    fn s005_audit_content_ignorance_no_user_content_in_message() {
        // Constitution V: the diagnostic message must reference only
        // canonical token strings (the tetragraph, the trigraphs in
        // expected/other_codes, and verbatim taxonomy data) — never
        // surrounding source bytes. Pin the contract by feeding a
        // fixture whose surrounding text would be obviously visible
        // if leaked. Banner has no REL TO so this is the active-
        // validation Suggest case ⇒ S005 fires (not S006).
        let source = "Document subject: \"Operation Confidential\"\n\
                      (S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| matches!(d.rule.as_str(), "S005" | "S006"))
            .expect("S005 or S006 must fire on RSMA fixture");
        assert!(
            !s005.message.contains("Operation Confidential"),
            "S005/S006 message must not leak surrounding document text: {:?}",
            s005.message
        );
        assert!(
            !s005.message.contains("Document subject"),
            "S005/S006 message must not leak surrounding document text: {:?}",
            s005.message
        );
    }

    // --- E010: Bare HCS rule ---

    #[test]
    fn e010_fires_on_bare_hcs_in_banner() {
        // PR 3c.B Sub-PR 8.D.3 — E010 migrated to `fix_intent: None`
        // (conscious-defer per CAPCO-2016 §H.4 lines 1369–1395; the
        // classifier must read the HCS-O / HCS-P marking templates).
        // The diagnostic still fires; only the auto-fix is dropped.
        let diags = lint_banner("TOP SECRET//HCS//NOFORN");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1);
        let src = b"TOP SECRET//HCS//NOFORN";
        assert_eq!(e010[0].span.as_str(src).unwrap(), "HCS");
        assert!(
            e010[0].fix.is_none(),
            "E010 must not carry a legacy FixProposal post-migration; got: {:?}",
            e010[0].fix
        );
        assert!(
            e010[0].fix_intent.is_none(),
            "E010 must consciously decline to emit a FixIntent \
             (HCS-O vs HCS-P is a classifier decision per §H.4); \
             got: {:?}",
            e010[0].fix_intent
        );
    }

    #[test]
    fn e010_fires_on_bare_hcs_in_portion() {
        // PR 3c.B Sub-PR 8.D.3 — same conscious-defer shape as the
        // banner variant. The diagnostic still fires; no auto-fix.
        let diags = lint_portion("(TS//HCS//NF)");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1);
        assert!(
            e010[0].fix.is_none(),
            "E010 must not carry a legacy FixProposal post-migration; got: {:?}",
            e010[0].fix
        );
        assert!(
            e010[0].fix_intent.is_none(),
            "E010 must consciously decline to emit a FixIntent \
             (HCS-O vs HCS-P is a classifier decision per §H.4); \
             got: {:?}",
            e010[0].fix_intent
        );
    }

    #[test]
    fn e010_does_not_fire_on_hcs_p() {
        let diags = lint_banner("TOP SECRET//HCS-P//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E010"),
            "E010 must not fire on HCS-P, got: {diags:?}"
        );
    }

    #[test]
    fn e010_does_not_fire_on_hcs_o() {
        let diags = lint_banner("TOP SECRET//HCS-O//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E010"),
            "E010 must not fire on HCS-O, got: {diags:?}"
        );
    }

    #[test]
    fn e010_does_not_emit_fix_when_hcs_o_present() {
        // PR 3c.B Sub-PR 8.D.3 — the pre-migration behavior lowered
        // fix confidence to 0.5 when HCS-O appeared alongside bare HCS
        // (ambiguous suggestion). Post-migration the entire fix path is
        // dropped; only the diagnostic fires.
        let diags = lint_banner("TOP SECRET//HCS//HCS-O//NOFORN");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1);
        assert!(
            e010[0].fix.is_none(),
            "E010 must not carry a legacy FixProposal post-migration; got: {:?}",
            e010[0].fix
        );
        assert!(
            e010[0].fix_intent.is_none(),
            "E010 must consciously decline to emit a FixIntent \
             (HCS-O vs HCS-P is a classifier decision per §H.4); \
             got: {:?}",
            e010[0].fix_intent
        );
    }

    // --- E012: Dual classification ---

    #[test]
    fn e012_fires_on_us_plus_nato() {
        let diags = lint_banner("SECRET//NATO SECRET//NOFORN");
        let e012: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E012").collect();
        assert_eq!(e012.len(), 1);
        assert!(e012[0].message.contains("US") && e012[0].message.contains("NATO"));
        // Pin the citation field to the catalog-matched authoritative
        // passage. Drift back to the legacy `§B.1` umbrella reference
        // would be caught here; structural citation-lint (which
        // accepts both `§B.1` and `§H.3 p55` as well-formed)
        // would not flag the regression.
        assert_eq!(e012[0].citation, capco(SectionLetter::H, 3, 55));
        // PR 3c.B Sub-PR 8.D.5: conscious-defer migration. E012
        // emits neither a legacy `FixProposal` nor a structural
        // `FixIntent`. See `crates/capco/src/rules_declarative.rs`
        // module-level comment on `DeclarativeDualClassificationRule`
        // for the cross-axis-renormalization rationale.
        assert!(e012[0].fix.is_none());
        assert!(e012[0].fix_intent.is_none());
    }

    #[test]
    fn e012_does_not_fire_on_us_only() {
        let diags = lint_banner("SECRET//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E012"));
    }

    #[test]
    fn e012_does_not_fire_on_nato_only() {
        let diags = lint_banner("//NATO SECRET//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E012"),
            "E012 should not fire on pure NATO, got: {:?}",
            diags
                .iter()
                .filter(|d| d.rule.as_str() == "E012")
                .collect::<Vec<_>>()
        );
    }

    // W002 retired (closes #470). Live regression coverage lives in
    // `crates/capco/tests/w002_retired.rs`; the dormant inline tests
    // (this block is `#[cfg(any())]`-gated at the module level) are
    // not re-added here.

    // --- E014: JOINT participants missing from REL TO ---

    #[test]
    fn e014_fires_when_joint_country_missing_from_rel_to() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA");
        let e014: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E014").collect();
        assert_eq!(e014.len(), 1);
        assert!(e014[0].message.contains("GBR"));
    }

    #[test]
    fn e014_does_not_fire_when_all_present() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E014"),
            "E014 should not fire when all JOINT countries in REL TO, got: {:?}",
            diags
                .iter()
                .filter(|d| d.rule.as_str() == "E014")
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn e014_does_not_fire_when_joint_country_covered_by_fvey_tetragraph() {
        // GBR is a FVEY member; REL TO USA, FVEY implicitly covers GBR.
        // §H.8 p145 defines tetragraphs as collective references to their
        // constituent trigraphs.
        let diags = lint_banner("//JOINT S GBR USA//REL TO USA, FVEY");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E014"),
            "E014 must not fire when JOINT country is covered by FVEY: {diags:?}"
        );
    }

    #[test]
    fn e014_does_not_fire_when_all_five_eyes_in_joint_covered_by_fvey() {
        // All five FVEY members in JOINT; FVEY alone covers them all.
        let diags = lint_banner("//JOINT S AUS CAN GBR NZL USA//REL TO USA, FVEY");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E014"),
            "E014 must not fire when all JOINT countries covered by FVEY: {diags:?}"
        );
    }

    #[test]
    fn e014_still_fires_when_joint_country_not_covered_by_tetragraph() {
        // DEU is not a FVEY member; REL TO USA, FVEY does not cover DEU.
        let diags = lint_banner("//JOINT S DEU USA//REL TO USA, FVEY");
        let e014: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E014").collect();
        assert_eq!(
            e014.len(),
            1,
            "E014 must still fire when a JOINT country is not in any REL TO tetragraph: {diags:?}"
        );
        assert!(e014[0].message.contains("DEU"));
    }

    // --- E015: Non-US without dissem ---

    #[test]
    fn e015_fires_on_nato_without_dissem() {
        let diags = lint_banner("//NATO SECRET");
        let e015: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E015").collect();
        assert_eq!(e015.len(), 1);
        // Pin the citation to the catalog-matched authoritative pair
        // (§H.7 p122 + §B.3 p20). Regression guard against drift back
        // to the legacy `§B.3`-only umbrella reference; structural
        // citation-lint accepts both forms and would not catch it.
        // Multi-passage citation `§H.7 p122 + §B.3 p20` — primary anchor
        // is the leading passage (§H.7 p122). Cross-reference to §B.3 p20
        // documented at the core_catalog row.
        assert_eq!(e015[0].citation, capco(SectionLetter::H, 7, 122));
    }

    #[test]
    fn e015_does_not_fire_with_rel_to() {
        let diags = lint_banner("//NATO SECRET//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E015"),
            "E015 should not fire when dissem present, got: {:?}",
            diags
                .iter()
                .filter(|d| d.rule.as_str() == "E015")
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn e015_does_not_fire_on_us_classification() {
        let diags = lint_banner("SECRET");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E015"));
    }

    // --- Non-US clean markings produce no unexpected diagnostics ---

    #[test]
    fn clean_nato_portion_no_diagnostics() {
        let diags = lint_portion("(//NS//REL TO USA, GBR)");
        let unexpected: Vec<_> = diags
            .iter()
            .filter(|d| !matches!(d.rule.as_str(), "E002")) // E002 may fire on USA ordering
            .collect();
        assert!(
            unexpected.is_empty(),
            "clean NATO portion should have no unexpected diagnostics, got: {unexpected:?}"
        );
    }

    // --- Non-IC dissem controls ---

    #[test]
    fn non_ic_dissem_parses_in_portion() {
        let diags = lint_portion("(U//DS)");
        // DS = LIMDIS portion form. Should parse without E008 (unknown token).
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E008"),
            "DS should be recognized as non-IC dissem, not unknown: {diags:?}"
        );
    }

    #[test]
    fn non_ic_dissem_les_nf_parses() {
        let diags = lint_portion("(U//LES-NF)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E008"),
            "LES-NF should be recognized: {diags:?}"
        );
    }

    // --- W003: Non-IC dissem in classified banner ---

    #[test]
    fn w003_fires_on_sbu_in_classified_banner() {
        let diags = lint_banner("CONFIDENTIAL//SBU");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(w003.len(), 1);
        assert!(w003[0].message.contains("SBU"));
    }

    #[test]
    fn w003_does_not_fire_on_unclassified_banner() {
        let diags = lint_banner("UNCLASSIFIED//SBU");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "W003 should not fire on UNCLASSIFIED banner: {diags:?}"
        );
    }

    #[test]
    fn w003_fires_on_limdis_in_classified_banner() {
        // CAPCO-2016 §H.9 p170: "When a document contains LIMDIS
        // and classified portions, LIMDIS is not used in the banner
        // line." Prior impl incorrectly placed LIMDIS in the
        // propagating set on a paraphrased "NGA Title 10" justification;
        // §H.9 is explicit that LIMDIS is stripped from classified
        // banners.
        let diags = lint_banner("SECRET//LIMDIS");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(
            w003.len(),
            1,
            "W003 must fire on LIMDIS in classified banner (§H.9 p170): {diags:?}"
        );
        assert!(w003[0].message.contains("LIMDIS"));
    }

    #[test]
    fn w003_does_not_fire_on_exdis_in_classified_banner() {
        // CAPCO-2016 §H.9 p172: "If EXDIS is contained in any
        // portion of a document that does not contain one or more NODIS
        // portions, EXDIS must appear in the banner line." Example
        // banner on p173: SECRET//NOFORN//EXDIS. Prior impl excluded
        // EXDIS from the propagating set; the §H.9 rule is the
        // opposite.
        let diags = lint_banner("SECRET//NOFORN//EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "EXDIS propagates to classified banners per §H.9 p172: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_nodis_in_classified_banner() {
        // CAPCO-2016 §H.9 p174: "If NODIS is contained in any
        // portion of a document, it must appear in the banner line."
        // Example banner on p174: SECRET//NOFORN//NODIS. Prior impl
        // excluded NODIS from the propagating set; the §H.9 rule is
        // the opposite.
        let diags = lint_banner("SECRET//NOFORN//NODIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "NODIS propagates to classified banners per §H.9 p174: {diags:?}"
        );
    }

    #[test]
    fn w003_fires_on_sbu_nf_in_classified_banner() {
        // CAPCO-2016 §H.9 p178: SBU NOFORN "Applicable only to
        // unclassified information." p179 example 2 shows a
        // `SECRET//NOFORN` banner with a `(U//SBU-NF)` portion — SBU-NF
        // absent from banner. The NOFORN half of SBU-NF *does*
        // propagate via `PageContext::expected_non_ic_dissem` (it
        // splits portion-level SBU-NF into SBU + NF-flag, emitting
        // NOFORN into the classified banner's dissem block). What
        // W003 catches is the literal `SBU NOFORN` *banner* form in a
        // classified document — that surface form is non-canonical
        // per §H.9, independent of whether NOFORN itself propagates.
        let diags = lint_banner("SECRET//SBU NOFORN");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(
            w003.len(),
            1,
            "W003 must fire on literal SBU-NF in classified banner (§H.9 p178): {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_les_in_classified_banner() {
        // CAPCO-2016 §H.9 p181: "The LES marking always appears in
        // the banner line if contained in any portion, regardless of
        // classification level." Example banners on p183: SECRET//REL
        // TO USA, FVEY//LES, SECRET//NOFORN//LES.
        let diags = lint_banner("SECRET//LES");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LES propagates to classified banners per §H.9 p181: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_les_nf_in_classified_banner() {
        // CAPCO-2016 §H.9 p185: "The LES marking always appears
        // in the banner line if LES information (either LES or LES
        // NOFORN) is contained in the document, regardless of the
        // document's classification level." The §H.9 canonical form
        // in classified docs is "LES" at banner with NOFORN split into
        // the dissem block (§H.9 p185), but `LES NOFORN` in a
        // classified banner is not a W003 concern — the canonicalization
        // is a separate page-rewrite concern.
        let diags = lint_banner("SECRET//LES NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "LES-NF propagates to classified banners per §H.9 p185: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_ssi_in_classified_banner() {
        // CAPCO-2016 §H.9 p189: "If the SSI marking is contained
        // in any portion of a document it must appear in the banner
        // line, regardless of the document's overall classification
        // level." Example banner on p191: SECRET//REL TO USA,
        // ACGU//SSI.
        let diags = lint_banner("SECRET//SSI");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "SSI propagates to classified banners per §H.9 p189: {diags:?}"
        );
    }

    #[test]
    fn w003_fires_on_sbu_in_nato_classified_banner() {
        // Non-US (NATO) classified banners are still classified — W003 should fire.
        let diags = lint_banner("//NS//SBU");
        let w003: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W003").collect();
        assert_eq!(
            w003.len(),
            1,
            "W003 must fire on SBU in NATO classified banner: {diags:?}"
        );
    }

    #[test]
    fn w003_does_not_fire_on_portion() {
        let diags = lint_portion("(C//DS)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "W003 is banner-only: {diags:?}"
        );
    }

    #[test]
    fn non_ic_dissem_correct_classified_doc() {
        let diags = lint_banner("CONFIDENTIAL//NOFORN");
        assert!(
            diags.is_empty(),
            "clean classified banner should have no diagnostics: {diags:?}"
        );
        let diags = lint_portion("(U//DS)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W003"),
            "non-IC dissem in portion should not fire W003: {diags:?}"
        );
    }

    // --- E016: RESTRICTED not allowed with JOINT ---

    #[test]
    fn e016_fires_on_joint_restricted() {
        let diags = lint_banner("//JOINT R USA GBR//REL TO USA, GBR");
        let e016: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E016").collect();
        assert_eq!(e016.len(), 1);
        assert!(e016[0].message.contains("RESTRICTED"));
        // PR 3c.B Sub-PR 8.B — message must surface the operational Five
        // Eyes equivalence hint so users know how to re-mark the violating
        // text manually. Wording stays context-neutral because the rule's
        // `check` does not consult `RuleContext` and can fire on either a
        // portion or a banner (the test input here is a banner). The hint
        // is framed as "per Five Eyes practice" — NOT as a §H.3 claim —
        // because the equivalence lives in CAPCO-2016 Appendix A §4 (Five
        // Eyes Marking Comparisons), not in §H.3. See the module-level
        // comment on `DeclarativeJointRestrictedRule` in
        // `rules_declarative.rs` and the followup at
        // `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`.
        assert!(
            e016[0].message.contains("CONFIDENTIAL"),
            "E016 message must surface the operational equivalence hint \
             (RESTRICTED → CONFIDENTIAL per Five Eyes practice) so the \
             user knows how to re-mark; got: {:?}",
            e016[0].message
        );
        assert!(
            e016[0].message.contains("Five Eyes"),
            "E016 message must frame the equivalence as Five Eyes practice \
             (NOT as a §H.3 claim — Constitution VIII citation fidelity); \
             got: {:?}",
            e016[0].message
        );
        // PR 3c.B Sub-PR 8.B — citation pin (D13 single-citation discipline).
        assert_eq!(e016[0].citation, capco(SectionLetter::H, 3, 56));
    }

    /// PR 3c.B Sub-PR 8.B — pin the consciously-decided-no-fix-intent
    /// migration state for E016.
    ///
    /// Per the 2026-05-11 lattice-consultant session captured in
    /// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`,
    /// E016 is **Category A.3 — Transmute via foreign-equivalence map**:
    /// the eventual Stage-4 target is `Remove(RESTRICTED) ⊕ Add(CONFIDENTIAL)`
    /// emitted as one atomic audit repair, driven by a foreign-equivalence
    /// vocabulary table. That vocabulary table does not exist in
    /// `marque-capco::vocab` today and its source is open (see the
    /// followup file's Open Question 1 — candidates include CAPCO-2016
    /// Appendix A §4 / Five Eyes Marking Comparisons, currently not
    /// vendored). Until the source is resolved, the rule emits a
    /// diagnostic with both `fix.is_none()` AND `fix_intent.is_none()`.
    ///
    /// **Do not** dual-populate this rule with a single-fact
    /// `FactRemove(RESTRICTED, Portion)` intent in the interim — that
    /// would land a half-fix (leaving the marking without a
    /// classification level) and corrupt the audit log under
    /// Constitution V.
    ///
    /// **Coverage note:** the G13 closure walker at
    /// `crates/capco/tests/g13_closure_fix_intent.rs::all_migrated_rule_intents_pass_g13_envelope_walker`
    /// only inspects rules that auto-apply through the engine (those
    /// emitting `AppliedFixProposal::New` records, which require
    /// `fix_intent.is_some()`). E016 with `fix_intent: None` is never
    /// reached by that walker — so this symmetry pin is the **only**
    /// guard against a future commit accidentally producing an
    /// asymmetric `(fix, fix_intent)` pair on E016. Without it, a drift
    /// toward `fix.is_some() && fix_intent.is_none()` (or the inverse)
    /// would slip through CI silently.
    #[test]
    fn e016_emits_no_fix_and_no_fix_intent_pending_stage4_a3_transmute() {
        let diags = lint_banner("//JOINT R USA GBR//REL TO USA, GBR");
        let e016 = diags
            .iter()
            .find(|d| d.rule.as_str() == "E016")
            .expect("E016 must fire on `//JOINT R USA GBR//REL TO USA, GBR`");
        assert!(
            e016.fix.is_none(),
            "E016 fix must be None until Stage-4 A.3 consolidation lands; \
             see incompatibility-primitive-consolidation.md followup"
        );
        assert!(
            e016.fix_intent.is_none(),
            "E016 fix_intent must be None (symmetric with fix.is_none(). \
             The G13 walker does NOT see (None, None) rules; this test is \
             the only guard against asymmetric drift"
        );
    }

    #[test]
    fn e016_does_not_fire_on_joint_secret() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E016"));
    }

    // --- E017/E018/E019 retirement regressions (T035b) ---
    //
    // These tests pin the retirement: markings that the legacy
    // rules wrongly flagged must NOT emit those rule IDs after
    // T035b. CAPCO §H.3 p57 permits JOINT with IC and non-IC
    // dissem (excluding only NOFORN and HCS per §H.3 p57) and with
    // FGI (cross-ref §H.7). Any reintroduction of E017/E018/E019
    // diagnostics would regress CAPCO-2016 fidelity.

    #[test]
    fn e017_does_not_fire_on_joint_rel_to_banner() {
        // Generic retirement check: E017 (JOINT + FGI marker) is
        // retired — the rule ID must never appear on the diagnostic
        // stream regardless of input. This test uses a plain
        // JOINT+REL TO banner, which does NOT exercise an FGI-marker
        // path (the parser's banner grammar does not surface
        // `fgi_marker` on a JOINT classification). True FGI-marker
        // coverage requires constructing `CanonicalAttrs` directly;
        // that's covered at the scheme level in
        // `scheme_equivalence.rs::no_legacy_e017_e018_e019_constraints_in_catalog`.
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E017"),
            "E017 retired; must never fire: {diags:?}"
        );
    }

    #[test]
    fn e018_does_not_fire_on_joint_with_noforn() {
        // Pre-T035b: E018 flagged JOINT + NOFORN as "IC dissem other
        // than REL TO". CAPCO §H.3 p57 does exclude NOFORN
        // from JOINT, but that's caught indirectly via
        // `capco/noforn-conflicts-rel-to` + E014 (REL TO required).
        // E018 itself must not fire.
        let diags = lint_banner("//JOINT S USA GBR//NF");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E018"),
            "E018 retired; must never fire: {diags:?}"
        );
    }

    #[test]
    fn e018_does_not_fire_on_joint_with_rel_to_only() {
        // Still holds post-retirement — plain `//JOINT S USA GBR//
        // REL TO USA, GBR` is the canonical valid JOINT form.
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E018"),
            "E018 retired; must never fire: {diags:?}"
        );
    }

    #[test]
    fn e019_does_not_fire_on_joint_with_limdis() {
        // Pre-T035b: E019 flagged JOINT + LIMDIS as "JOINT + non-IC
        // dissem". CAPCO §H.3 p57 explicitly permits non-IC
        // dissem with JOINT "as appropriate". Retired entirely.
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR//LIMDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E019"),
            "E019 retired; must never fire: {diags:?}"
        );
    }

    // --- E036: JOINT + HCS markings (T035b replacement) ---

    #[test]
    fn legacy_joint_hcs_rules_do_not_fire_on_parser_path() {
        // §H.3 p57: "May not be used with the HCS markings".
        // This parser-driven test does not reliably provide positive
        // E036 coverage because the grammar may not surface HCS in
        // a JOINT banner at this point. What it *does* verify is
        // that the retired legacy JOINT rules (E017/E018/E019)
        // never appear on this input path. Positive E036 coverage
        // lives in scheme-level tests
        // (`scheme_equivalence::e036_fires_on_joint_with_bare_hcs` /
        // `_with_hcs_p`) where attrs can be constructed directly.
        let diags = lint_banner("//JOINT S USA GBR//HCS-P//REL TO USA, GBR");
        assert!(
            diags
                .iter()
                .all(|d| !matches!(d.rule.as_str(), "E017" | "E018" | "E019")),
            "legacy E017/E018/E019 must not fire post-T035b: {diags:?}"
        );
    }

    /// PR 3c.B Sub-PR 8.B — pin the consciously-decided-no-fix-intent
    /// migration state for E036.
    ///
    /// Per the 2026-05-11 lattice-consultant session captured in
    /// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`,
    /// E036 is **Category B — genuine mutual exclusion without policy
    /// decision**: the eventual Stage-4 target is `Reject { suggest:
    /// Some(...) }` — error diagnostic with an optional
    /// `Severity::Suggest` companion ("did you mean
    /// `SECRET//HCS-P//REL TO [LIST]`?"). No auto-applied fix exists for
    /// this combination — JOINT changes attribution semantics; HCS is
    /// CIA-owned and US-only; the marking shape is contradictory in a
    /// way no removal can resolve.
    ///
    /// JOINT+HCS is academic in practice (JOINT classifications are
    /// largely DOD-only; HCS is CIA-only; the agencies' marking
    /// vocabularies don't overlap on this axis), so the diagnostic-only
    /// landing is functionally sufficient.
    ///
    /// **Parser-gap note:** the existing test
    /// `legacy_joint_hcs_rules_do_not_fire_on_parser_path` above
    /// documents that the engine pipeline (`lint_banner`) does not
    /// reliably surface E036 because the parser may not emit `TOK_HCS`
    /// inside a JOINT banner. This symmetry pin therefore constructs
    /// `CanonicalAttrs` programmatically and calls
    /// `DeclarativeJointHcsRule.check()` directly — at the Rule-emission
    /// layer (Diagnostic), one layer above the scheme-validation
    /// (ConstraintViolation) layer covered by
    /// `tests/scheme_equivalence.rs::e036_fires_on_joint_with_bare_hcs`.
    /// The `(fix, fix_intent)` symmetry is a Diagnostic-shape invariant
    /// that scheme-level tests cannot pin.
    ///
    /// **Coverage note:** the G13 closure walker at
    /// `crates/capco/tests/g13_closure_fix_intent.rs::all_migrated_rule_intents_pass_g13_envelope_walker`
    /// only inspects rules that auto-apply through the engine (those
    /// emitting `AppliedFixProposal::New` records, which require
    /// `fix_intent.is_some()`). E036 with `fix_intent: None` is never
    /// reached by that walker — so this symmetry pin is the **only**
    /// guard against a future commit accidentally producing an
    /// asymmetric `(fix, fix_intent)` pair on E036. Without it, a drift
    /// toward `fix.is_some() && fix_intent.is_none()` (or the inverse)
    /// would slip through CI silently.
    #[test]
    fn e036_emits_no_fix_and_no_fix_intent_pending_stage4_b_reject() {
        use crate::rules_declarative::DeclarativeJointHcsRule;
        use marque_ism::{
            CanonicalAttrs, Classification, CountryCode, JointClassification,
            MarkingClassification, MarkingType, SciCompartment, SciControlBare, SciControlSystem,
            SciMarking,
        };
        use marque_rules::{Rule, RuleContext};

        let mut attrs = CanonicalAttrs::default();
        attrs.classification = Some(MarkingClassification::Joint(JointClassification {
            level: Classification::Secret,
            countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
        }));
        attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
        attrs.sci_markings = vec![SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Vec::<SciCompartment>::new().into_boxed_slice(),
            None,
        )]
        .into();

        // Test-fixture carve-out per Constitution V Principle V:
        // synthetic empty span — these tests construct attrs
        // directly and do not exercise intent-only synthesis.
        // Unit test for the declarative-rule layer; no engine
        // two-pass pipeline. PR 4b-B 9th-pass follow-up:
        // `RuleContext` is `#[non_exhaustive]`; use the `new`
        // minimal-context constructor.
        let ctx = RuleContext::new(MarkingType::Banner, marque_scheme::Span::new(0, 0));

        let rule = DeclarativeJointHcsRule;
        let diags = rule.check(&attrs, &ctx);

        assert_eq!(
            diags.len(),
            1,
            "E036 must emit exactly one Diagnostic on JOINT+HCS attrs; got: {diags:?}"
        );
        let d = &diags[0];
        assert_eq!(d.rule.as_str(), "E036");
        assert_eq!(d.citation, capco(SectionLetter::H, 3, 57));
        assert!(
            d.fix.is_none(),
            "E036 fix must be None until Stage-4 B reject lands; \
             see incompatibility-primitive-consolidation.md followup"
        );
        assert!(
            d.fix_intent.is_none(),
            "E036 fix_intent must be None (symmetric with fix.is_none(). \
             The G13 walker does NOT see (None, None) rules; this test is \
             the only guard against asymmetric drift"
        );
    }

    /// PR 3c.B Sub-PR 8.B — programmatic negative case complementing
    /// `e036_emits_no_fix_and_no_fix_intent_pending_stage4_b_reject`.
    ///
    /// Closes the layer-symmetry gap raised by the code-reviewer:
    /// the positive case above tests `DeclarativeJointHcsRule.check()`
    /// directly with programmatic `CanonicalAttrs`. The engine-path
    /// negative case at `e036_does_not_fire_on_joint_without_hcs`
    /// covers a different layer (engine pipeline) and inherits the
    /// parser-gap caveat documented on
    /// `legacy_joint_hcs_rules_do_not_fire_on_parser_path`. This test
    /// closes that gap: it confirms `DeclarativeJointHcsRule.check()`
    /// returns an empty `Vec` when given JOINT+non-HCS-SCI attrs, at
    /// the same Rule-emission layer as the positive case.
    #[test]
    fn e036_does_not_fire_on_joint_with_non_hcs_sci_at_rule_layer() {
        use crate::rules_declarative::DeclarativeJointHcsRule;
        use marque_ism::{
            CanonicalAttrs, Classification, CountryCode, JointClassification,
            MarkingClassification, MarkingType, SciControl,
        };
        use marque_rules::{Rule, RuleContext};

        let mut attrs = CanonicalAttrs::default();
        attrs.classification = Some(MarkingClassification::Joint(JointClassification {
            level: Classification::Secret,
            countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
        }));
        attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
        // SI is permitted with JOINT (§H.3 p57: SCI excluding HCS is
        // permitted with JOINT). The rule must NOT fire.
        attrs.sci_controls = vec![SciControl::Si].into();

        // Test-fixture carve-out per Constitution V Principle V:
        // synthetic empty span — these tests construct attrs
        // directly and do not exercise intent-only synthesis.
        // Unit test for the declarative-rule layer; no engine
        // two-pass pipeline. PR 4b-B 9th-pass follow-up:
        // `RuleContext` is `#[non_exhaustive]`; use the `new`
        // minimal-context constructor.
        let ctx = RuleContext::new(MarkingType::Banner, marque_scheme::Span::new(0, 0));

        let rule = DeclarativeJointHcsRule;
        let diags = rule.check(&attrs, &ctx);

        assert!(
            diags.is_empty(),
            "E036 must NOT fire on JOINT+SI (SCI sans HCS is permitted per \
             §H.3 p57); got: {diags:?}"
        );
    }

    #[test]
    fn e036_does_not_fire_on_joint_without_hcs() {
        let diags = lint_banner("//JOINT S USA GBR//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E036"),
            "E036 must not fire without HCS present: {diags:?}"
        );
    }

    // --- E037: NODIS ⊥ EXDIS (T035c-21 PR-A, §H.9 p172 + p174) ---

    #[test]
    fn e037_fires_when_nodis_and_exdis_coexist() {
        // Banner carries both NODIS and EXDIS — mutually exclusive per
        // §H.9 p172 + p174. NOFORN is also
        // required (E038), so include it so we only see E037.
        let diags = lint_banner("SECRET//NOFORN//NODIS/EXDIS");
        let e037: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E037").collect();
        assert_eq!(
            e037.len(),
            1,
            "E037 must fire when both NODIS and EXDIS are present: {diags:?}"
        );
        // PR 10.A.1: typed Citation pins the primary anchor (§H.9 p172 —
        // EXDIS authority). The cross-reference to p174 (NODIS) lives
        // in the rule's doc-comment per the brief's "Multi-page
        // citation decision".
        assert_eq!(
            e037[0].citation,
            capco(SectionLetter::H, 9, 172),
            "E037 citation must pin §H.9 p172 (EXDIS authority); got: {:?}",
            e037[0].citation
        );
        // PR 10.A.1 Commit 4: secondary-passage cross-reference to
        // §H.9 p174 lives on `super::NODIS_EXDIS_MUTEX_CROSS_REFS`. Companion
        // assertion in `citation_cross_refs_tests` (bottom of this file) (see the E005
        // site above for the cfg-gating rationale).
    }

    #[test]
    fn e037_does_not_fire_with_only_nodis() {
        let diags = lint_banner("SECRET//NOFORN//NODIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E037"),
            "E037 must not fire when only NODIS present: {diags:?}"
        );
    }

    #[test]
    fn e037_does_not_fire_with_only_exdis() {
        let diags = lint_banner("SECRET//NOFORN//EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E037"),
            "E037 must not fire when only EXDIS present: {diags:?}"
        );
    }

    // --- E038: NODIS / EXDIS require NOFORN (T035c-21 PR-A, §H.9) ---

    #[test]
    fn e038_fires_on_nodis_without_noforn() {
        // §H.9 p174: NODIS "May be used only with NOFORN
        // information." Banner with NODIS and no NOFORN is a
        // violation.
        let diags = lint_banner("SECRET//NODIS");
        let e038: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E038").collect();
        assert_eq!(
            e038.len(),
            1,
            "E038 must fire on NODIS without NOFORN: {diags:?}"
        );
        // PR 10.A.1: typed Citation pins §H.9 p172; cross-reference to
        // p174 lives in the rule doc-comment.
        assert_eq!(
            e038[0].citation,
            capco(SectionLetter::H, 9, 172),
            "E038 citation must pin §H.9 p172 (EXDIS authority); got: {:?}",
            e038[0].citation
        );
        // PR 10.A.1 Commit 4: secondary-passage cross-reference to
        // §H.9 p174 lives on `super::NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS`. Companion
        // assertion in `citation_cross_refs_tests` (bottom of this file).
    }

    #[test]
    fn e038_fires_on_exdis_without_noforn() {
        let diags = lint_banner("SECRET//EXDIS");
        let e038: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E038").collect();
        assert_eq!(
            e038.len(),
            1,
            "E038 must fire on EXDIS without NOFORN: {diags:?}"
        );
    }

    #[test]
    fn e038_does_not_fire_when_nodis_has_noforn() {
        let diags = lint_banner("SECRET//NOFORN//NODIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E038"),
            "E038 must not fire when NOFORN is present: {diags:?}"
        );
    }

    #[test]
    fn e038_does_not_fire_when_exdis_has_noforn() {
        let diags = lint_banner("SECRET//NOFORN//EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E038"),
            "E038 must not fire when NOFORN is present: {diags:?}"
        );
    }

    #[test]
    fn e038_fires_only_once_when_both_nodis_and_exdis_lack_noforn() {
        // A single marking with both NODIS and EXDIS (invalid per
        // E037) AND no NOFORN should fire E037 once + E038 once —
        // not E038 twice. The declarative Custom constraint fuses
        // the NODIS/EXDIS disjunction into a single violation.
        let diags = lint_banner("SECRET//NODIS/EXDIS");
        let e038: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E038").collect();
        assert_eq!(
            e038.len(),
            1,
            "E038 must fire exactly once even when both NODIS and EXDIS \
             are present: {diags:?}"
        );
    }

    // --- E039: REL TO cleared from banner when portion has NODIS/EXDIS ---

    #[test]
    fn e039_fires_on_banner_rel_to_with_nodis_portion() {
        // Portion carries NODIS; banner carries REL TO. §H.9 p174
        // line 4301: REL TO not authorized in banner when any portion
        // has NODIS.
        let source = "(S//NF//ND)\nSECRET//NOFORN//NODIS//REL TO USA, GBR";
        let diags = lint_banner(source);
        let e039: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E039").collect();
        assert_eq!(
            e039.len(),
            1,
            "E039 must fire when banner has REL TO and portion has NODIS: {diags:?}"
        );
        assert!(
            e039[0].fix.is_none(),
            "E039 emits no fix (removing REL TO is multi-span and \
             requires human judgment): {:?}",
            e039[0].fix
        );
        // PR 10.A.1: typed Citation pins §H.9 p172; cross-reference to
        // p174 (NODIS) documented at the rule.
        assert_eq!(
            e039[0].citation,
            capco(SectionLetter::H, 9, 172),
            "E039 citation must pin §H.9 p172 (EXDIS); got: {:?}",
            e039[0].citation
        );
        // PR 10.A.1 Commit 4: secondary-passage cross-reference to
        // §H.9 p174 lives on `super::NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS`. Companion
        // assertion in `citation_cross_refs_tests` (bottom of this file).
    }

    #[test]
    fn e039_fires_on_banner_rel_to_with_exdis_portion() {
        let source = "(S//NF//XD)\nSECRET//NOFORN//EXDIS//REL TO USA, GBR";
        let diags = lint_banner(source);
        let e039: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E039").collect();
        assert_eq!(
            e039.len(),
            1,
            "E039 must fire when banner has REL TO and portion has EXDIS: {diags:?}"
        );
    }

    #[test]
    fn e039_does_not_fire_without_nodis_or_exdis_in_portions() {
        // Banner has REL TO, portion has no NODIS/EXDIS — E039 must
        // stay silent (this is a normal REL TO banner).
        let source = "(S//NF)\nSECRET//NOFORN//REL TO USA, GBR";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E039"),
            "E039 must not fire without NODIS/EXDIS in any portion: {diags:?}"
        );
    }

    #[test]
    fn e039_does_not_fire_when_banner_has_no_rel_to() {
        let source = "(S//NF//ND)\nSECRET//NOFORN//NODIS";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E039"),
            "E039 must not fire when banner has no REL TO: {diags:?}"
        );
    }

    #[test]
    fn e039_still_fires_after_engine_gap_close() {
        // PR 3c.B-8F-engine-gap regression pin: E039 reads
        // `attrs.rel_to` (the literal banner REL TO list) AND
        // `page.expected_non_ic_dissem()` first element (the NODIS/EXDIS
        // set) — neither of which is affected by the engine-gap close.
        // The gap-close adjusts `expected_non_ic_dissem`'s SECOND tuple
        // element (`needs_nf`), and `expected_rel_to`'s short-circuit
        // behavior. E039's check path does not consume either signal.
        //
        // This test pins the load-bearing assertion that E039 stays in
        // place after the gap-close lands. Re-running the existing
        // `e039_fires_on_banner_rel_to_with_nodis_portion` test post-PR
        // would catch a regression, but THIS test exists to document
        // why E039 is preserved (not retired) by this PR: the engine
        // gap closes a parallel read-API inconsistency; E039 is the
        // dedicated rule for "banner has REL TO + portion has
        // NODIS/EXDIS" and retains its check path verbatim.
        //
        // E039 retirement is a follow-on PR that requires a
        // BannerMatchesProjectedRule REL TO row to become the natural
        // detector. Not in scope here.
        let source = "(S//NODIS)\nSECRET//NODIS//REL TO USA";
        let diags = lint_banner(source);
        let e039: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E039").collect();
        assert_eq!(
            e039.len(),
            1,
            "E039 must continue firing after PR 3c.B-8F-engine-gap (banner \
             has REL TO + portion has NODIS): {diags:?}"
        );
        // PR 10.A.1: typed Citation pins the primary anchor §H.9 p172.
        assert_eq!(
            e039[0].citation,
            capco(SectionLetter::H, 9, 172),
            "E039 citation must continue to pin §H.9 p172: {:?}",
            e039[0].citation
        );
        // PR 10.A.1 Commit 4: secondary-passage cross-reference to
        // §H.9 p174 lives on `super::NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS`. Companion
        // assertion in `citation_cross_refs_tests` (bottom of this file).
    }

    // --- E040: Banner must roll up NODIS (or EXDIS if no NODIS) ---

    #[test]
    fn e040_fires_when_banner_missing_nodis_from_portion() {
        // Portion has NODIS; banner has no NODIS. §H.9 p174 line
        // 4300: NODIS in any portion must appear in the banner.
        // Banner already has a non-IC dissem block (LIMDIS), so fix
        // is an insertion at the end of that block.
        let source = "(S//NF//ND)\nSECRET//NOFORN//LIMDIS";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(
            e040.len(),
            1,
            "E040 must fire when banner omits NODIS: {diags:?}"
        );
        assert!(
            e040[0].message.contains("NODIS"),
            "E040 message must name the missing token; got: {:?}",
            e040[0].message
        );
        let fix = e040[0].fix.as_ref().expect("E040 must carry a fix");
        assert_eq!(
            fix.span.start, fix.span.end,
            "E040 fix must be a zero-width insertion"
        );
        assert_eq!(fix.replacement.as_ref(), "/NODIS");
    }

    #[test]
    fn e040_fires_when_banner_missing_exdis_and_no_nodis_anywhere() {
        // Portion has EXDIS; no NODIS anywhere; banner has no EXDIS.
        // §H.9 p172.
        let source = "(S//NF//XD)\nSECRET//NOFORN//LIMDIS";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(
            e040.len(),
            1,
            "E040 must fire when banner omits EXDIS with no NODIS: {diags:?}"
        );
        let fix = e040[0].fix.as_ref().expect("fix expected");
        assert_eq!(fix.replacement.as_ref(), "/EXDIS");
    }

    #[test]
    fn e040_nodis_has_priority_over_exdis_when_both_in_portions() {
        // Portions have both NODIS and EXDIS; banner has neither.
        // §H.9 p172 / p174: NODIS has priority
        // over EXDIS in the banner. Banner must carry NODIS (not
        // EXDIS).
        let source = "(S//NF//ND)\n(S//NF//XD)\nSECRET//NOFORN//LIMDIS";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(e040.len(), 1);
        assert!(
            e040[0].message.contains("NODIS"),
            "E040 must require NODIS (not EXDIS) when both are in portions; \
             got message: {:?}",
            e040[0].message
        );
        let fix = e040[0].fix.as_ref().expect("fix expected");
        assert_eq!(
            fix.replacement.as_ref(),
            "/NODIS",
            "fix must add NODIS, not EXDIS"
        );
    }

    #[test]
    fn e040_does_not_fire_when_banner_already_has_required_token() {
        let source = "(S//NF//ND)\nSECRET//NOFORN//NODIS";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E040"),
            "E040 must not fire when banner already has NODIS: {diags:?}"
        );
    }

    #[test]
    fn e040_emits_no_fix_when_banner_has_no_non_ic_dissem_block() {
        // Banner has classification + IC dissem only, but NO
        // Non-IC dissem block at all. Inserting a new category block
        // is unsafe (needs separator-positioning), so E040 emits a
        // no-fix Error.
        let source = "(S//NF//ND)\nSECRET//NOFORN";
        let diags = lint_banner(source);
        let e040: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E040").collect();
        assert_eq!(e040.len(), 1);
        assert!(
            e040[0].fix.is_none(),
            "E040 must not carry a fix when banner has no Non-IC dissem \
             block (byte-positioning a new block is unsafe): {:?}",
            e040[0].fix
        );
    }

    // --- E041: NODIS supersedes EXDIS in a portion ---

    #[test]
    fn e041_fires_on_portion_with_both_nodis_and_exdis() {
        // §H.9 p172 / p174: when a portion has both, NODIS supersedes
        // EXDIS. E041 surfaces the diagnostic at Warn severity and
        // emits an intent-only `FactRemove(EXDIS, Scope::Portion)`
        // fix that the engine auto-applies via the synthesis path
        // (PR 3c.B Sub-PR 8.E.2 — unblocks E041 in #106). The legacy `fix`
        // field stays `None`; the new emission is on `fix_intent`.
        let diags = lint_portion("(S//NF//ND/XD)");
        let e041: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E041").collect();
        assert_eq!(
            e041.len(),
            1,
            "E041 must fire on portion with both NODIS and EXDIS: {diags:?}"
        );
        assert_eq!(e041[0].severity, Severity::Warn);
        assert!(
            e041[0].fix.is_none(),
            "E041 emits no legacy FixProposal (intent-only emission); \
             the engine synthesizes the byte-precise fix via \
             `synthesize_intent_only_fixes` at fix time; got: {:?}",
            e041[0].fix
        );
        assert!(
            e041[0].fix_intent.is_some(),
            "E041 must emit `fix_intent: Some(FactRemove(EXDIS, Portion))` \
             post-PR-3c.B-Sub-PR-8.E.2; got: {:?}",
            e041[0].fix_intent
        );
        assert!(
            e041[0].message.contains("NODIS") && e041[0].message.contains("EXDIS"),
            "E041 message must name both tokens; got: {:?}",
            e041[0].message
        );
    }

    #[test]
    fn e041_points_at_exdis_token_in_both_orderings() {
        // E041's diagnostic span should point at the EXDIS token
        // regardless of whether it appears before or after NODIS in
        // the portion. Exercise both orderings.
        for src in ["(S//NF//ND/XD)", "(S//NF//XD/ND)"] {
            let diags = lint_portion(src);
            let e041: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E041").collect();
            assert_eq!(e041.len(), 1, "E041 must fire on {src:?}: {diags:?}");
            let span_text = e041[0].span.as_str(src.as_bytes()).unwrap();
            assert_eq!(
                span_text, "XD",
                "E041 span must point at the EXDIS token in {src:?}; \
                 got: {span_text:?}"
            );
        }
    }

    #[test]
    fn e041_does_not_fire_on_portion_with_only_nodis() {
        let diags = lint_portion("(S//NF//ND)");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E041"),
            "E041 must not fire on portion with only NODIS: {diags:?}"
        );
    }

    #[test]
    fn e041_does_not_fire_on_banner_even_when_both_present() {
        // E041 is portion-only per §H.9 p172 + p174 ("in the portion
        // mark"). The banner case is owned by E037 (mutual exclusion,
        // Error).
        let diags = lint_banner("SECRET//NOFORN//NODIS/EXDIS");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E041"),
            "E041 must not fire on banner context: {diags:?}"
        );
        // But E037 must still fire.
        assert!(
            diags.iter().any(|d| d.rule.as_str() == "E037"),
            "E037 must still fire on banner NODIS+EXDIS: {diags:?}"
        );
    }

    /// PR 3c.B Sub-PR 8.E — pin the consciously-decided-no-fix-intent
    /// migration state for E037.
    ///
    /// Per the 2026-05-11 lattice-consultant session captured in
    /// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`,
    /// E037 is **Category B — genuine mutual exclusion without policy
    /// decision**: the eventual Stage-4 target is `Reject { suggest: None }`
    /// — error diagnostic with no auto-applied fix. CAPCO-2016 §H.9 does
    /// not specify a banner-level supersession; only that NODIS and EXDIS
    /// MUST NOT coexist (p172 + p174). Portion-level supersession is
    /// E041's territory and is itself blocked on the parser within-category
    /// separator gap (Category A.1 Remove(EXDIS, Scope::Portion)).
    ///
    /// **Coverage note:** the G13 closure walker at
    /// `crates/capco/tests/g13_closure_fix_intent.rs::all_migrated_rule_intents_pass_g13_envelope_walker`
    /// only inspects rules that auto-apply through the engine (those
    /// emitting `AppliedFixProposal::New` records, which require
    /// `fix_intent.is_some()`). E037 with `fix_intent: None` is never
    /// reached by that walker — so this symmetry pin is the **only**
    /// guard against a future commit accidentally producing an
    /// asymmetric `(fix, fix_intent)` pair on E037. Without it, a drift
    /// toward `fix.is_some() && fix_intent.is_none()` (or the inverse)
    /// would slip through CI silently.
    #[test]
    fn e037_emits_no_fix_and_no_fix_intent_pending_stage4_b_reject() {
        let diags = lint_banner("SECRET//NOFORN//NODIS/EXDIS");
        let e037 = diags
            .iter()
            .find(|d| d.rule.as_str() == "E037")
            .expect("E037 must fire on `SECRET//NOFORN//NODIS/EXDIS`");
        assert!(
            e037.fix.is_none(),
            "E037 fix must be None until Stage-4 B-Reject consolidation lands; \
             see incompatibility-primitive-consolidation.md followup"
        );
        assert!(
            e037.fix_intent.is_none(),
            "E037 fix_intent must be None (symmetric with fix.is_none()). \
             The G13 walker does NOT see (None, None) rules; this test is \
             the only guard against asymmetric drift"
        );
    }

    /// PR 3c.B Sub-PR 8.E.2 — pin E041's intent-only emission shape
    /// (unblocks E041, the primary rule named in #106).
    ///
    /// E041 emits `fix: None, fix_intent: Some(FactRemove(EXDIS,
    /// Scope::Portion))`. The engine's
    /// `synthesize_intent_only_fixes` consumes the intent + the
    /// diagnostic's `candidate_span` to produce a byte-precise
    /// FixProposal that covers the full portion span; the
    /// within-category `/` separator is replaced as part of the
    /// re-rendered portion, sidestepping the parser gap tracked in
    /// issue #106.
    ///
    /// This test pins three load-bearing invariants of the
    /// intent-only emission:
    ///
    /// 1. `fix.is_none()` — the legacy `FixProposal` field stays
    ///    empty. The engine synthesizes the byte-precise fix
    ///    downstream; the rule does not duplicate it on the
    ///    diagnostic. (Dual-population is reserved for Path C
    ///    migrations under Commits 3/8; E041 is an intent-only
    ///    rule, never dual-populated.)
    ///
    /// 2. `fix_intent.is_some()` and the intent variant is
    ///    `ReplacementIntent::FactRemove` with `token_ref =
    ///    FactRef::Cve(TOK_EXDIS)` and `scope = Scope::Portion`.
    ///    Any drift (FactAdd, wrong token, wrong scope) would
    ///    silently change which token gets removed.
    ///
    /// 3. `candidate_span.is_some()` — load-bearing for the
    ///    synthesis path. `synthesize_intent_only_fixes` skips any
    ///    intent-only diagnostic whose `candidate_span` is `None`
    ///    (see `crates/engine/src/engine.rs:2141-2143`), so an E041
    ///    that emits `fix_intent` without `candidate_span` would
    ///    silently fail to auto-apply.
    #[test]
    fn e041_emits_intent_only_factremove_exdis_portion() {
        use marque_scheme::{FactRef, ReplacementIntent, Scope};

        let diags = lint_portion("(S//NF//ND/XD)");
        let e041 = diags
            .iter()
            .find(|d| d.rule.as_str() == "E041")
            .expect("E041 must fire on portion `(S//NF//ND/XD)` carrying both NODIS and EXDIS");

        assert!(
            e041.fix.is_none(),
            "E041 must emit `fix: None` (intent-only); got: {:?}",
            e041.fix
        );

        let intent = e041
            .fix_intent
            .as_ref()
            .expect("E041 must emit `fix_intent: Some(FactRemove(EXDIS, Portion))`");
        match &intent.replacement {
            ReplacementIntent::FactRemove { facts, scope } => {
                assert_eq!(
                    facts.len(),
                    1,
                    "E041 FactRemove must have exactly one fact (EXDIS); got: {facts:?}"
                );
                assert_eq!(
                    facts[0],
                    FactRef::Cve(crate::scheme::TOK_EXDIS),
                    "E041 intent must target EXDIS (§H.9 names EXDIS as \
                     the loser); got: {:?}",
                    facts[0]
                );
                assert_eq!(
                    *scope,
                    Scope::Portion,
                    "E041 intent scope must be Portion per §H.9 p172 + \
                     p174 (\"in the portion mark\"); got: {scope:?}"
                );
            }
            other => panic!("E041 intent must be ReplacementIntent::FactRemove; got: {other:?}"),
        }

        assert!(
            e041.candidate_span.is_some(),
            "E041 must populate `candidate_span` so the engine's \
             `synthesize_intent_only_fixes` knows which scope-bytes to \
             re-render; got: {:?}",
            e041.candidate_span
        );
    }

    // Engine-level round-trip / idempotence / FR-016 tests for E041
    // live in `crates/capco/tests/e041_intent_only_engine.rs` (PR 3c.B
    // Sub-PR 8.E.2 — unblocks E041 in #106). They can't live inside this
    // `#[cfg(test)]` module because the `marque_engine` dependency
    // pulls in `marque_capco` as published, giving two non-equal
    // crate identities for `CapcoScheme` (the inline module's
    // `crate::scheme::CapcoScheme` vs `marque_capco::scheme::CapcoScheme`)
    // and breaking the `RuleSet<CapcoScheme>` trait bound.

    // -----------------------------------------------------------------------
    // E053 — NOFORN conflicts with REL TO (§H.8 p145)
    // -----------------------------------------------------------------------

    #[test]
    fn e053_fires_when_noforn_and_rel_to_coexist_in_banner() {
        // §H.8 p145: NOFORN "Cannot be used with REL TO."
        let diags = lint_banner("SECRET//REL TO USA, GBR//NOFORN");
        let e053: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E053").collect();
        assert_eq!(
            e053.len(),
            1,
            "E053 must fire once when NOFORN and REL TO coexist: {diags:?}"
        );
    }

    #[test]
    fn e053_fires_on_portion_with_nf_and_rel_to() {
        // Portion-mark form: `NF` is the portion abbreviation for NOFORN.
        let diags = lint_portion("(S//REL TO USA, GBR/NF)");
        let e053: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E053").collect();
        assert_eq!(
            e053.len(),
            1,
            "E053 must fire on portion with NF and REL TO: {diags:?}"
        );
    }

    #[test]
    fn e053_silent_when_only_noforn_no_rel_to() {
        let diags = lint_banner("SECRET//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E053"),
            "E053 must not fire when REL TO is absent: {diags:?}"
        );
    }

    #[test]
    fn e053_silent_when_only_rel_to_no_noforn() {
        let diags = lint_banner("SECRET//REL TO USA, GBR");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E053"),
            "E053 must not fire when NOFORN is absent: {diags:?}"
        );
    }

    #[test]
    fn e002_fix_output_dedups_when_input_has_duplicates() {
        // Issue #234 PR-B fixup (Copilot review): E002's fix path
        // also composes dedup + canonicalize so its replacement stays
        // single-pass idempotent against E052 on overlapping spans.
        // Input: USA missing AND non-USA codes duplicated → E002
        // fires (missing USA), E052 fires (GBR repeated). FR-016
        // tiebreaker keeps E002 (lex), so E002's replacement must be
        // canonical or re-lint would still fire E052.
        let src = "SECRET//REL TO GBR, AUS, GBR";
        let diags = lint_banner(src);
        let e002_fix = diags
            .iter()
            .find(|d| d.rule.as_str() == "E002")
            .and_then(|d| d.fix.as_ref())
            .expect("E002 must fire and carry a fix when USA is missing from REL TO");
        assert_eq!(
            e002_fix.replacement.as_ref(),
            "USA, AUS, GBR",
            "E002 fix must dedup before sorting (canonical form, no duplicates)"
        );
    }

    #[test]
    fn dedup_country_codes_preserves_first_occurrence_order() {
        use marque_ism::CountryCode;
        let codes = vec![
            CountryCode::USA,
            CountryCode::try_new(b"GBR").unwrap(),
            CountryCode::USA,
            CountryCode::try_new(b"AUS").unwrap(),
            CountryCode::try_new(b"GBR").unwrap(),
        ];
        let deduped = dedup_country_codes(&codes);
        let expected = vec![
            CountryCode::USA,
            CountryCode::try_new(b"GBR").unwrap(),
            CountryCode::try_new(b"AUS").unwrap(),
        ];
        assert_eq!(deduped, expected);
    }

    // --- E021: RD/FRD requires NOFORN ---

    #[test]
    fn e021_fires_on_rd_without_noforn() {
        let diags = lint_banner("SECRET//RD//REL TO USA, GBR");
        let e021: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E021").collect();
        assert_eq!(e021.len(), 1);
    }

    #[test]
    fn e021_does_not_fire_on_rd_with_noforn() {
        let diags = lint_banner("SECRET//RD//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E021"),
            "E021 should not fire with NOFORN present: {diags:?}"
        );
    }

    #[test]
    fn e021_fires_on_frd_without_noforn() {
        let diags = lint_banner("SECRET//FRD//REL TO USA, GBR");
        let e021: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E021").collect();
        assert_eq!(e021.len(), 1);
    }

    // --- CNWDI floor (formerly E022, now bridge-emitted E058 via
    // catalog row `E058/CNWDI-classification-floor`) ---
    //
    // PR 3b.D (T026d): the CNWDI floor invariant moved into the
    // class-floor catalog. PR 3c.B Commit 7.3: the walker
    // (`DeclarativeClassFloorRule`) retired; the engine's constraint-
    // catalog bridge is the sole emitter. The lib-level tests
    // (`e022_fires_on_cnwdi_with_confidential` and friends) that
    // exercised `lint_banner` retired alongside the walker — the
    // bridge fires through `engine.lint`, which the lib-level harness
    // bypasses. The 27 class-floor catalog rows are covered
    // comprehensively (fires-below / silent-at-floor / silent-when-
    // absent triplet per row, plus span-anchor + severity-override
    // tests) by the engine-level test suite in
    // `crates/capco/tests/class_floor_catalog.rs`. The CNWDI-specific
    // entry points are `cnwdi_fires_below_secret` and
    // `cnwdi_does_not_fire_when_marking_absent` in that file.

    // --- E024: RD precedence ---

    #[test]
    fn e024_fires_on_rd_plus_frd() {
        // Both RD and FRD in same marking — FRD should be removed.
        let diags = lint_banner("SECRET//RD//FRD//NOFORN");
        let e024: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E024").collect();
        assert_eq!(e024.len(), 1);
        assert!(e024[0].message.contains("FRD"));
    }

    #[test]
    fn e024_does_not_fire_on_rd_alone() {
        let diags = lint_banner("SECRET//RD//NOFORN");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E024"));
    }

    // --- UCNI ceiling (formerly E025, now bridge-emitted E058 via
    // catalog rows `E058/DOD-UCNI-classification-ceiling` +
    // `E058/DOE-UCNI-classification-ceiling`) ---
    //
    // PR 3b.D (T026d): the UCNI ceiling invariant moved into the
    // class-floor catalog as TWO rows (DOD UCNI + DOE UCNI; split per
    // PM decision so each carries its own §H.6 sub-page citation).
    // PR 3c.B Commit 7.3: lib-level `lint_banner` tests retired
    // alongside the walker — the engine-level UCNI coverage lives in
    // `crates/capco/tests/class_floor_catalog.rs::dod_ucni_*` and
    // `doe_ucni_*`.

    // --- Spec 003 SCI compartments: E010 structural regression ---

    #[test]
    fn e010_still_fires_when_hcs_reaches_rule_through_structural_path() {
        // Bare `HCS` is dispatched to the structural subparser (is_bare_cve_value
        // matches) and surfaces as SciMarking { Published(Hcs), compartments: [] }.
        // The canonical_enum projection also populates sci_controls, so both
        // detection predicates in E010 see the bare HCS. This test pins that
        // the combined predicate still fires once (not twice) for regression.
        let diags = lint_banner("TOP SECRET//HCS//NOFORN");
        let e010: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E010").collect();
        assert_eq!(e010.len(), 1, "E010 must fire exactly once for bare HCS");
    }

    // --- Shared sort key ---

    #[test]
    fn sar_sort_key_numeric_before_alpha() {
        // Numeric-prefixed sorts before pure alpha.
        assert!(sar_sort_key("12") < sar_sort_key("BP"));
        assert!(sar_sort_key("7ALPHA") < sar_sort_key("BP"));
    }

    #[test]
    fn sar_sort_key_numeric_by_value() {
        // Numeric prefixes compare as integers, not bytewise.
        assert!(sar_sort_key("9") < sar_sort_key("12"));
        assert!(sar_sort_key("J12") < sar_sort_key("J54"));
    }

    #[test]
    fn sar_sort_key_alpha_by_bytelex() {
        assert!(sar_sort_key("BP") < sar_sort_key("CD"));
        assert!(sar_sort_key("CD") < sar_sort_key("XR"));
    }

    // --- SAR floor (formerly E027, now bridge-emitted E058 via
    // catalog row `E058/SAR-classification-floor`) ---
    //
    // PR 3b.D (T026d): the SAR floor invariant moved into the class-floor
    // catalog. PR 3c.B Commit 7.3: lib-level `lint_banner` tests retired
    // alongside the walker. Engine-level coverage:
    // `crates/capco/tests/class_floor_catalog.rs::sar_fires_on_unclassified`,
    // `sar_does_not_fire_at_confidential`, and
    // `sar_does_not_fire_when_marking_absent`.

    // --- W034: SCI custom control info ---

    #[test]
    fn e034_fires_on_custom_control_via_structural_path() {
        // `123/SI-G` routes through the structural subparser; the `123` head
        // creates a Custom-system SciMarking. W034 surfaces that for audit
        // visibility (severity Off by default, so the engine gates it).
        let diags = lint_banner("TOP SECRET//123/SI-G//NOFORN");
        let w034: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "W034").collect();
        assert_eq!(
            w034.len(),
            1,
            "W034 must fire on custom control 123: {diags:?}"
        );
        assert!(w034[0].fix.is_none(), "W034 must not propose a fix");
        // T035c-2: W034 now defaults to Warn (was Off with a harness
        // workaround). Info is available as a config-opt-in.
        assert_eq!(w034[0].severity, marque_rules::Severity::Warn);
        assert!(w034[0].message.contains("unpublished SCI control system"));
    }

    #[test]
    fn e034_does_not_fire_on_published_only() {
        let diags = lint_banner("TOP SECRET//SI-G//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "W034"),
            "W034 must not fire on SI-G alone: {diags:?}"
        );
    }

    // --- E035: SCI banner rollup ---

    #[test]
    fn e035_no_ops_without_page_marking() {
        // E035 is dispatched by `BannerMatchesProjectedRule::check`,
        // whose first-line guard is `ctx.page_marking.as_ref()` (PR 9b
        // T133 / FR-006). On a banner with no preceding portions the
        // engine never produces a page-marking projection — there is
        // nothing to project from — so the walker returns early and
        // E035 must stay silent. This is the stable empty-page guard,
        // not a temporary harness gap.
        let diags = lint_banner("TOP SECRET//SI-G//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E035"),
            "E035 must no-op without per-page marking: {diags:?}"
        );
    }

    #[test]
    fn e035_fires_on_missing_compartment_sci_asymmetry_with_sar() {
        // SCI/SAR asymmetry lockdown: portion has `SI-G` (system SI,
        // compartment G); banner has bare `SI` (no compartment shown).
        // E035 MUST fire. This is the exact shape that E031 (SAR)
        // deliberately does NOT fire on after T035c-19 PR-C — §H.5
        // p101 makes SAR hierarchy optional. §H.4 contains
        // no equivalent carve-out for SCI, so E035 enforces full
        // hierarchy roll-up. Flipping this test would break the
        // source-level semantic distinction.
        let source = "(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(
            e035.len(),
            1,
            "E035 MUST fire when banner omits compartment G that appears in \
             a portion — SCI has no hierarchy-optional carve-out: {diags:?}"
        );
        assert!(
            e035[0].message.contains("G"),
            "message must name the missing compartment; got: {:?}",
            e035[0].message
        );
    }

    #[test]
    fn e035_fires_on_missing_sub_compartment_sci_asymmetry_with_sar() {
        // Sibling asymmetry test: portion has `SI-G ABCD` (sub-comp
        // ABCD under compartment G); banner has `SI-G` (no
        // sub-compartment). E035 MUST fire; E031 would not for the
        // SAR-equivalent shape.
        let source = "(TS//SI-G ABCD//NF)\nTOP SECRET//SI-G//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(
            e035.len(),
            1,
            "E035 MUST fire when banner omits sub-compartment ABCD present \
             in a portion: {diags:?}"
        );
        assert!(
            e035[0].message.contains("ABCD"),
            "message must name the missing sub-compartment; got: {:?}",
            e035[0].message
        );
    }

    #[test]
    fn e035_does_not_fire_when_banner_covers_full_hierarchy() {
        // Happy path: banner's hierarchy matches the portion's. E035
        // must stay silent.
        let source = "(TS//SI-G ABCD//NF)\nTOP SECRET//SI-G ABCD//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E035"),
            "E035 must not fire when banner already covers portion hierarchy: \
             {diags:?}"
        );
    }

    #[test]
    fn e035_message_wording_covers_all_hierarchy_levels() {
        // PR #102 review: the rule's `missing` list can contain
        // three shapes — system-missing, compartment-missing, and
        // sub-compartment-missing. The earlier diagnostic message
        // said only "missing compartments", which was inaccurate
        // for the system-missing case (entire SCI control system
        // absent from banner). This test locks the corrected
        // wording.
        //
        // Scenario: portion carries `TK` (entire system); banner
        // carries only `SI`. So TK is missing as an ENTIRE SYSTEM,
        // not just a compartment. The message must reflect that.
        let source = "(TS//SI/TK//NF)\nTOP SECRET//SI//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(e035.len(), 1);
        let msg = &e035[0].message;
        assert!(
            msg.contains("systems, compartments, and/or sub-compartments")
                || msg.contains("markings"),
            "E035 message must describe the hierarchy-level breadth \
             accurately (not only 'compartments'); got: {msg:?}"
        );
        assert!(
            msg.contains("TK"),
            "E035 message must name the missing TK system; got: {msg:?}"
        );
        // The per-entry format still specifies the level for each
        // missing item, so `TK` carries "(system missing from banner)".
        assert!(
            msg.contains("system missing from banner"),
            "E035 per-entry annotation must mark TK as an entirely \
             missing system; got: {msg:?}"
        );
    }

    #[test]
    fn e035_cites_per_system_precedence_rules() {
        let source = "(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";
        let diags = lint_banner(source);
        let e035: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E035").collect();
        assert_eq!(e035.len(), 1);
        // T026a D13 single-citation discipline: the citation string
        // carries §H.4 per-system "Precedence Rules for Banner Line
        // Guidance" only — that's the operative banner-roll-up rule
        // for SCI per `specs/006-engine-rule-refactor/tasks.md`
        // T026a. §D.2 p28 (CAPCO-2016 lines 577–579) restates the
        // same invariant in general-algorithm prose; it lives as a
        // background reference in `evaluate_sci_banner_rollup`'s doc
        // comment, NOT in the citation string.
        // PR 10.A.1: typed Citation pins §H.4 p61 (SCI per-system
        // "Precedence Rules for Banner Line Guidance" anchor). The
        // string framing "Precedence Rules for Banner Line" lived in
        // the pre-migration string citation and is dropped here —
        // the typed Citation is structurally `§H.4 p61` only. §D.2's
        // general-algorithm prose stays in `evaluate_sci_banner_rollup`'s
        // doc comment as a background reference (D13 single-citation
        // discipline preserved by construction).
        assert_eq!(
            e035[0].citation,
            capco(SectionLetter::H, 4, 61),
            "E035 citation must pin §H.4 p61; got: {:?}",
            e035[0].citation
        );
    }

    // --- E008 skip filter: structural SCI tokens ---

    #[test]
    fn e008_does_not_fire_on_structurally_formed_sci_tokens() {
        // `SI-G ABCD DEFG` is a structurally-formed SCI token. When the
        // parser accepts it, no Unknown span is produced and E008 stays
        // silent for that reason. This test pins the structural happy path.
        let diags = lint_banner("SECRET//SI-G ABCD DEFG//NOFORN");
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E008"),
            "E008 must not fire on structurally-parsed SI-G block: {diags:?}"
        );
    }

    // PR 3b.D (T026d): retired E027 → E058 catalog row
    // `E058/SAR-classification-floor`. PR 3c.B Commit 7.3: the walker
    // retired; SAR-floor citation coverage moved to the engine-level
    // test `crates/capco/tests/class_floor_catalog.rs::sar_fires_on_unclassified`
    // (asserts `sar[0].citation == "CAPCO-2016 §H.5"`).

    // --- E031: sar-banner-rollup ---

    #[test]
    fn e031_fires_when_banner_missing_program_from_portion() {
        // Portions introduce SAR-BP and SAR-CD; banner only mentions BP.
        // E031's fix is a zero-width INSERTION at the end of the SAR
        // block — so the fix span is (block_end, block_end) and the
        // replacement is `/CD`. This shape lets E031 coexist with E028
        // / E029 fixes on the same marking under the engine's overlap
        // guard (see rule doc for the full FR-016 argument).
        let source = "(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(
            e031.len(),
            1,
            "E031 must fire when banner omits CD: {diags:?}"
        );
        let d = e031[0];
        assert!(
            d.message.contains("CD"),
            "message must name the missing program: {}",
            d.message
        );
        let fix = d
            .fix
            .as_ref()
            .expect("E031 must carry a fix when banner has SAR block");

        // Zero-width insertion span: start == end == end-of-block byte.
        assert_eq!(
            fix.span.start, fix.span.end,
            "E031 fix must be a zero-width insertion"
        );
        assert_eq!(
            fix.original.as_ref(),
            "",
            "zero-width insertion must have empty `original`"
        );
        assert_eq!(
            fix.replacement.as_ref(),
            "/CD",
            "insertion replacement must be `/<missing>`"
        );
        assert!((fix.confidence.combined() - 0.9).abs() < f32::EPSILON);

        // Applied output: simulate the splice and confirm the banner now
        // contains `SAR-BP/CD`.
        let mut buf = source.as_bytes().to_vec();
        buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
        let applied = std::str::from_utf8(&buf).unwrap();
        assert!(
            applied.contains("SECRET//SAR-BP/CD//NOFORN"),
            "applied fix must produce `SECRET//SAR-BP/CD//NOFORN`; \
             got: {applied:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_banner_omits_portion_compartment() {
        // T035c-19 PR-C: narrowed predicate. §H.5 p101 and
        // §H.5 p99 make banner hierarchy depth (below the
        // program identifier) optional. A portion with `SAR-BP-J12`
        // rolling up to a banner with `SAR-BP` (no compartment shown)
        // is compliant — the author deliberately omitted hierarchy,
        // which §H.5 permits. The prior behavior treated this as an
        // E031 violation; that was over-restriction relative to
        // source.
        let source = "(S//SAR-BP-J12//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must NOT fire on optional-hierarchy banner \
             (portion has BP-J12, banner has bare BP — §H.5 p101 \
             permits): {diags:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_banner_omits_portion_sub_compartment() {
        // Sibling case: portion has `SAR-BP-J12 K15` (J12 is a
        // compartment, K15 is a sub-compartment of J12); banner has
        // `SAR-BP-J12` (omits the sub-compartment). §H.5 p101 line
        // 2460 covers sub-compartments too ("hierarchy ... below the
        // program identifier is optional"). Must not fire.
        let source = "(S//SAR-BP-J12 K15//NF)\nSECRET//SAR-BP-J12//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must NOT fire when banner omits sub-compartment \
             present in portion (hierarchy is optional): {diags:?}"
        );
    }

    #[test]
    fn e031_fires_when_banner_has_no_sar_block_but_portion_does() {
        // Portion has SAR-BP; banner has no SAR block at all.
        let source = "(S//SAR-BP//NF)\nSECRET//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(
            e031.len(),
            1,
            "E031 must fire when banner lacks any SAR block: {diags:?}"
        );
        // No fix when banner has no SAR block (byte-positioning is unsafe).
        assert!(
            e031[0].fix.is_none(),
            "E031 must not propose a fix when no SAR block exists"
        );
        // And severity escalates to Error for this variant.
        assert_eq!(e031[0].severity, Severity::Error);

        // PR #101 review: the no-block message must describe a whole
        // missing block, NOT read like the block exists but is
        // missing internal programs. Pin the distinct wording so a
        // regression that re-merges the two branches' messages
        // fails here.
        let msg = &e031[0].message;
        assert!(
            msg.contains("missing an SAR block"),
            "no-block message must state that the SAR block itself is \
             missing; got: {msg:?}"
        );
        assert!(
            !msg.contains("SAR block is missing programs"),
            "no-block message must NOT reuse the with-block \
             'block is missing programs' wording; got: {msg:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_banner_matches_portions() {
        let source = "(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP/CD//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must not fire when banner SAR block covers all portions: {diags:?}"
        );
    }

    #[test]
    fn e031_does_not_fire_when_no_portions_have_sar() {
        // Banner has a SAR block but no portions carry SAR — the rollup
        // produces None and nothing is missing.
        let source = "(S//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        assert!(
            diags.iter().all(|d| d.rule.as_str() != "E031"),
            "E031 must not fire without any SAR portions: {diags:?}"
        );
    }

    #[test]
    fn e031_fix_preserves_observed_hierarchy_when_adding_missing_program() {
        // T035c-19 PR-C: the zero-width insertion at end-of-block
        // preserves the observed banner's hierarchy verbatim (because
        // it doesn't touch the observed bytes at all) and adds only
        // the missing programs as bare identifiers. §H.5 p101 line
        // 2460 makes hierarchy depiction the author's choice — the
        // fix honors that for existing programs by construction.
        //
        // Portion: SAR-BP-J12 (BP with compartment J12) and SAR-CD.
        // Banner observed: SAR-BP-J12 (BP with compartment shown, CD
        // missing). Applied output: SAR-BP-J12/CD (J12 preserved,
        // bare CD appended — NO invented hierarchy on CD).
        let source = "(S//SAR-BP-J12//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP-J12//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(
            e031.len(),
            1,
            "E031 must fire on missing program CD: {diags:?}"
        );
        let fix = e031[0].fix.as_ref().expect("E031 must have fix");

        assert_eq!(fix.replacement.as_ref(), "/CD");
        assert_eq!(fix.span.start, fix.span.end);

        let mut buf = source.as_bytes().to_vec();
        buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
        let applied = std::str::from_utf8(&buf).unwrap();
        assert!(
            applied.contains("SECRET//SAR-BP-J12/CD//NOFORN"),
            "applied fix must preserve BP-J12 and append bare CD; \
             got: {applied:?}"
        );
    }

    #[test]
    fn e031_cites_line_2458_and_hierarchy_optional_note() {
        // T035c-19 PR-C citation lockdown. E031's authority is:
        //   §H.5 p101  — programs MUST roll up
        //   §H.5 p101  — hierarchy MAY be omitted
        // The citation string must reference both so reviewers land
        // on the two passages that together define the narrowed
        // predicate.
        let source = "(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";
        let diags = lint_banner(source);
        let e031: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();
        assert_eq!(e031.len(), 1);
        // PR 10.A.1: typed Citation pins §H.5 p101 — both the SAR
        // roll-up rule and the hierarchy-optional carve-out live at
        // the same passage.
        assert_eq!(
            e031[0].citation,
            capco(SectionLetter::H, 5, 101),
            "E031 citation must pin §H.5 p101 (roll-up rule + \
             hierarchy-optional carve-out); got: {:?}",
            e031[0].citation
        );
    }

    #[test]
    fn e008_fires_on_malformed_sci_shape() {
        // `SI-` is SCI-shaped but invalid (dangling hyphen). The structural
        // subparser rejects it, so it falls through as Unknown and E008
        // correctly fires — no silent suppression.
        let diags = lint_banner("SECRET//SI-//NOFORN");
        assert!(
            diags.iter().any(|d| d.rule.as_str() == "E008"),
            "E008 must fire on malformed SCI-shaped token: {diags:?}"
        );
    }

    /// PR 3c.B Sub-PR 9 — pin the consciously-decided-no-fix-intent
    /// migration state for E005.
    ///
    /// Per `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md`
    /// D4 (Path A fallback), E005 stays as a registered hand-written
    /// `Rule` impl in `rules.rs` until `render_canonical` lands on the
    /// `MarkingScheme` trait surface (the
    /// `Recanonicalize { scope: Document }` retirement target). The
    /// structural blocker — `MarkingScheme::evaluate_custom` having no
    /// access to `RuleContext.marking_type` — is tracked in
    /// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`.
    /// Until the retirement vehicle lands, the rule emits a diagnostic
    /// with both `fix.is_none()` AND `fix_intent.is_none()`.
    ///
    /// **Coverage note:** the G13 closure walker at
    /// `crates/capco/tests/g13_closure_fix_intent.rs::all_migrated_rule_intents_pass_g13_envelope_walker`
    /// only inspects rules that auto-apply through the engine (those
    /// emitting `AppliedFixProposal::New` records, which require
    /// `fix_intent.is_some()`). E005 with `fix_intent: None` is never
    /// reached by that walker — so this symmetry pin is the **only**
    /// guard against a future commit accidentally producing an
    /// asymmetric `(fix, fix_intent)` pair on E005.
    #[test]
    fn e005_emits_no_fix_and_no_fix_intent_pending_stage4_recanonicalize_document() {
        let diags = lint_banner("SECRET//25X1//NOFORN");
        let e005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "E005")
            .expect("E005 must fire on `SECRET//25X1//NOFORN` (declass exemption in banner)");
        assert!(
            e005.fix.is_none(),
            "E005 fix must be None until Stage-4 `Recanonicalize {{ scope: Document }}` \
             lands; see constraint-context-extension.md followup"
        );
        assert!(
            e005.fix_intent.is_none(),
            "E005 fix_intent must be None (symmetric with fix.is_none()). \
             The G13 walker does NOT see (None, None) rules; this test is \
             the only guard against asymmetric drift"
        );
    }

    /// PR 3c.B Sub-PR 9 — pin the consciously-decided-no-fix-intent
    /// migration state for S005.
    ///
    /// Per `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md`
    /// D4 (Path A fallback), S005 stays as a registered hand-written
    /// `Rule` impl in `rules.rs` until the admonition emitter channel
    /// is specced and built per
    /// `specs/006-engine-rule-refactor/followups/admonition-channel.md`.
    /// The structural blocker — `MarkingScheme::evaluate_custom` having
    /// no access to `RuleContext.page_portions` (the entire body of
    /// `analyze_uncertain_reduction` is page-portions-dependent) — is
    /// tracked in
    /// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`.
    /// Until either retirement vehicle lands, the rule emits a diagnostic
    /// with both `fix.is_none()` AND `fix_intent.is_none()`.
    ///
    /// **Coverage note:** same as the E005 pin — the G13 walker doesn't
    /// see `(None, None)` rules; this symmetry pin is the only guard.
    #[test]
    fn s005_emits_no_fix_and_no_fix_intent_pending_stage4_admonition_channel() {
        // RSMA is an NA-deprecated tetragraph from the V2022-NOV ISMCAT
        // taxonomy (per the existing test-module note at L~4585);
        // `is_decomposable("RSMA")` returns `None`, so it qualifies as
        // an uncertain code. Two portions list it differently; the
        // page-level atom intersection drops RSMA. Banner has no REL TO
        // (NOFORN supersedes) — active-validation / Suggest branch.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//NOFORN";
        let diags = lint_banner(source);
        let s005 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S005")
            .expect("S005 must fire on RSMA uncertain-reduction (Suggest branch)");
        assert!(
            s005.fix.is_none(),
            "S005 fix must be None until Stage-4 admonition channel lands; \
             see admonition-channel.md and constraint-context-extension.md followups"
        );
        assert!(
            s005.fix_intent.is_none(),
            "S005 fix_intent must be None (symmetric with fix.is_none())"
        );
    }

    /// PR 3c.B Sub-PR 9 — pin the consciously-decided-no-fix-intent
    /// migration state for S006. Same shape as S005's pin; the Info
    /// branch fires when the banner's REL TO is consistent with the
    /// atom-semantics intersection.
    #[test]
    fn s006_emits_no_fix_and_no_fix_intent_pending_stage4_admonition_channel() {
        // Banner REL TO equals the atom-semantics intersection
        // ({USA, GBR}); `expected ⊆ banner` ⇒ Info branch ⇒ S006 fires.
        let source = "(S//REL TO USA, GBR, RSMA)\n\
                      (S//REL TO USA, AUS, GBR)\n\
                      SECRET//REL TO USA, GBR";
        let diags = lint_banner(source);
        let s006 = diags
            .iter()
            .find(|d| d.rule.as_str() == "S006")
            .expect("S006 must fire on RSMA uncertain-reduction (Info branch)");
        assert!(
            s006.fix.is_none(),
            "S006 fix must be None until Stage-4 admonition channel lands"
        );
        assert!(
            s006.fix_intent.is_none(),
            "S006 fix_intent must be None (symmetric with fix.is_none())"
        );
    }
}
