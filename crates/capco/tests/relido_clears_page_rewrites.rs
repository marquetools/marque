// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! #559 close-out (PM decision 2026-05-19) — RELIDO-eviction
//! PageRewrites that converted the retired E056 / E057
//! `Constraint::Conflicts` rows into subtractive page-scope
//! supersession rewrites. Exercises the cross-portion behavior
//! (ORCON on one portion, RELIDO on another) that the per-portion
//! Conflicts rows missed pre-conversion. The third row (E055
//! DISPLAY ONLY > RELIDO) is deferred — see
//! `rewrites/relido_clears.rs` module header for the parser-axis /
//! scheduler-cycle rationale.
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

    // The pre-#559 trigger inputs that would have fired E056/E057
    // as Conflicts diagnostics (E055 the DISPLAY ONLY case is
    // deferred — see module header). Post-#559 these inputs route
    // through the PageRewrite path silently — no per-portion
    // diagnostic surface, just canonical output.
    let cases: &[&[u8]] = &[
        b"(S//OC/RELIDO)",       // E056 trigger
        b"(S//OC-USGOV/RELIDO)", // E057 trigger
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
