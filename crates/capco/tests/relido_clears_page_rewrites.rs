// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! #559 close-out (PM decision 2026-05-19) + #618 — RELIDO-eviction
//! PageRewrites that converted the retired E055 / E056 / E057
//! `Constraint::Conflicts` rows into subtractive page-scope
//! supersession rewrites. Exercises the cross-portion behavior
//! (dominator on one portion, RELIDO on another) that the per-portion
//! Conflicts rows missed pre-conversion. The E055 DISPLAY ONLY row
//! was deferred behind #618 until `satisfies(TOK_DISPLAY_ONLY)` was
//! widened to recognize the canonical `display_only_to` parser axis.
//!
//! Each row's `action` is `FactRemove(RELIDO, Scope::Page)`, so the
//! end-to-end signal is: presence of the dominator on any portion +
//! presence of RELIDO on any portion → projected banner drops
//! RELIDO. The lint surface no longer emits E055 / E056 / E057
//! diagnostics; the eviction is silent (canonical-output rather than
//! diagnostic-surface — per Marque's "guide, don't just flag"
//! convention for dissem conflicts).
//!
//! Authority:
//! - E055 (DISPLAY ONLY > RELIDO): CAPCO-2016 §H.8 p154
//! - E056 (ORCON > RELIDO): CAPCO-2016 §H.8 p136
//! - E057 (ORCON-USGOV > RELIDO): CAPCO-2016 §H.8 p140
//!
//! Re-verified against `crates/capco/docs/CAPCO-2016.md` at authorship.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{CanonicalAttrs, CapcoTokenSet, MarkingCandidate, MarkingType, Span};
use marque_scheme::{MarkingScheme, Scope};

fn parse_portion(text: &str) -> CanonicalAttrs {
    let tokens = CapcoTokenSet;
    let parser = marque_core::Parser::new(&tokens);
    let cand = MarkingCandidate {
        span: Span::new(0, text.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&cand, text.as_bytes())
        .expect("test input must parse cleanly");
    marque_ism::from_parsed_unchecked(parsed.attrs)
}

fn project_page(portions: &[&str]) -> CanonicalAttrs {
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> = portions
        .iter()
        .map(|p| CapcoMarking::new(parse_portion(p)))
        .collect();
    let projected = scheme.project(Scope::Page, &markings);
    projected.0
}

fn banner_carries_relido(portions: &[&str]) -> bool {
    let attrs = project_page(portions);
    attrs
        .dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Relido))
}

// ---------------------------------------------------------------------------
// E.4.4 — DISPLAY ONLY > RELIDO at page scope (#618).
// ---------------------------------------------------------------------------

#[test]
fn display_only_clears_relido_within_one_portion() {
    // Single portion carries both DISPLAY ONLY GBR and RELIDO
    // (canonical token order — DISPLAY ONLY precedes RELIDO in the
    // IC dissem category per CAPCO §H.8 sort order). §H.8 p154
    // marks RELIDO incompatible with DISPLAY ONLY. The canonical
    // wire form routes the country list to `attrs.display_only_to`;
    // #618 widened `satisfies(TOK_DISPLAY_ONLY)` and the
    // PageRewrite category predicate so the
    // `capco/display-only-clears-relido` trigger fires on that axis.
    assert!(
        !banner_carries_relido(&["(S//DISPLAY ONLY GBR/RELIDO)"]),
        "DISPLAY ONLY on the same portion as RELIDO must evict \
         RELIDO at page projection (§H.8 p154); same-portion case",
    );
}

#[test]
fn display_only_clears_relido_cross_portion() {
    // Cross-portion: portion A has REL TO + RELIDO (so it carries
    // release permission via REL TO and survives the §D.2 row-19
    // all-or-nothing gate), portion B has DISPLAY ONLY GBR.
    // Both portions carry release permission, so DisplayOnlyBlock
    // rolls up to {GBR} (intersection of {USA, GBR} from REL TO
    // expansion ∩ {GBR} from DISPLAY ONLY). With display_only_to
    // populated at page scope, the `capco/display-only-clears-relido`
    // PageRewrite fires and removes RELIDO. §H.8 p154 —
    // DISPLAY ONLY supersedes RELIDO at page roll-up.
    assert!(
        !banner_carries_relido(&["(S//REL TO USA, GBR/RELIDO)", "(S//DISPLAY ONLY GBR)"]),
        "DISPLAY ONLY on one portion must evict RELIDO from another \
         portion at page projection (§H.8 p154); cross-portion case \
         requires both portions to carry release permission so the \
         §D.2 row-19 gate passes",
    );
}

// ---------------------------------------------------------------------------
// E.4.3 — ORCON > RELIDO at page scope.
// ---------------------------------------------------------------------------

#[test]
fn orcon_clears_relido_cross_portion() {
    // Cross-portion: portion A has RELIDO, portion B has ORCON.
    // §H.8 p136 ORCON entry: "May not be used with RELIDO." Page
    // projection must drop RELIDO.
    assert!(
        !banner_carries_relido(&["(S//RELIDO)", "(S//OC)"]),
        "ORCON on one portion must evict RELIDO from another portion \
         at page projection (§H.8 p136); cross-portion case",
    );
}

#[test]
fn orcon_usgov_clears_relido_cross_portion() {
    // Cross-portion: portion A has RELIDO, portion B has ORCON-USGOV.
    // §H.8 p140 ORCON-USGOV entry: same exclusion as ORCON.
    assert!(
        !banner_carries_relido(&["(S//RELIDO)", "(S//OC-USGOV)"]),
        "ORCON-USGOV on one portion must evict RELIDO from another \
         portion at page projection (§H.8 p140); cross-portion case",
    );
}

// ---------------------------------------------------------------------------
// Idempotence — pages without RELIDO are unchanged.
// ---------------------------------------------------------------------------

#[test]
fn no_op_when_relido_absent() {
    // Trigger present (ORCON), but RELIDO absent. FactRemove of an
    // absent token is the per-intent no-op (`IntentInapplicable`,
    // silent), so the page projection is unchanged.
    let attrs = project_page(&["(S//OC)"]);
    assert!(
        attrs
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Oc)),
        "ORCON must survive page projection when RELIDO is absent — \
         the FactRemove(RELIDO) intent is no-op when RELIDO isn't \
         on the page",
    );
}

// ---------------------------------------------------------------------------
// Retirement guard — E055/E056/E057 must not fire as diagnostics
// anymore. The retired Conflicts rows have been removed from the
// constraint catalog; if a regression re-adds them, this test
// catches the resurfacing diagnostic.
// ---------------------------------------------------------------------------

#[test]
fn retired_e055_e056_e057_no_longer_fire_as_diagnostics() {
    use marque_capco::CapcoRuleSet;
    use marque_config::Config;
    use marque_engine::Engine;

    let engine = Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default scheme constructs cleanly");

    // The pre-#559 trigger inputs that would have fired E055/E056/E057
    // as Conflicts diagnostics. Post-#559 + #618 these inputs route
    // through the PageRewrite path silently — no per-portion
    // diagnostic surface, just canonical output.
    let cases: &[&[u8]] = &[
        b"(S//RELIDO/DISPLAY ONLY GBR)", // E055 trigger
        b"(S//OC/RELIDO)",               // E056 trigger
        b"(S//OC-USGOV/RELIDO)",         // E057 trigger
    ];

    for input in cases {
        let result = engine.lint(input);
        for d in &result.diagnostics {
            let id = d.rule.as_str();
            assert!(
                id != "E055" && id != "E056" && id != "E057",
                "Retired rule {id} fired on input {:?} — the \
                 Constraint::Conflicts row should be gone post-#559; \
                 the PageRewrite path (capco/*-clears-relido) is \
                 silent at the diagnostic surface",
                std::str::from_utf8(input).unwrap_or("<bytes>"),
            );
        }
    }
}
