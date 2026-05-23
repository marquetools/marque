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

fn parse_portion(scheme: &CapcoScheme, text: &str) -> CanonicalAttrs {
    // PR 3c.2.B (PM-B-3 second clause): the helper takes `&CapcoScheme`
    // so `project_page` (which already constructs one for
    // `scheme.project`) reuses that instance instead of allocating a
    // fresh scheme per parse.
    let tokens = CapcoTokenSet;
    let parser = marque_core::Parser::new(&tokens);
    let cand = MarkingCandidate {
        span: Span::new(0, text.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&cand, text.as_bytes())
        .expect("test input must parse cleanly");
    scheme.canonicalize(parsed.attrs)
}

fn project_page(scheme: &CapcoScheme, portions: &[&str]) -> CanonicalAttrs {
    let markings: Vec<CapcoMarking> = portions
        .iter()
        .map(|p| CapcoMarking::new(parse_portion(scheme, p)))
        .collect();
    let projected = scheme.project(Scope::Page, &markings);
    projected.0
}

fn banner_carries_relido(scheme: &CapcoScheme, portions: &[&str]) -> bool {
    let attrs = project_page(scheme, portions);
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
    let scheme = CapcoScheme::new();
    assert!(
        !banner_carries_relido(&scheme, &["(S//DISPLAY ONLY GBR/RELIDO)"]),
        "DISPLAY ONLY on the same portion as RELIDO must evict \
         RELIDO at page projection (§H.8 p154); same-portion case",
    );
}

#[test]
fn display_only_clears_relido_cross_portion() {
    // Cross-portion: portion A has REL TO + RELIDO (so it carries
    // release permission via REL TO and survives the §D.2 row-19
    // all-or-nothing gate), portion B has DISPLAY ONLY GBR.
    //
    // Post-#704 (refined per redirect brief): pre-#704 the
    // `MASK_RELIDO_US_CLASS_SUPPRESSORS` mask (which includes
    // REL_TO_PRESENT) suppressed Row 9 inside `close()`, so closure
    // didn't add the RELIDO that the join overlay had stripped.
    // Post-#704 the SAME mask gates
    // `default_fill::row9_should_fill`: input portion A carries
    // REL TO USA, GBR → REL_TO_PRESENT is set on the post-close
    // bitmask → default-fill Row 9's gate fails → no implicit
    // RELIDO added. End-to-end identical to pre-#704.
    //
    // The §B.3 paragraph b p19 "NOT MARKED PREVIOUSLY" conservatism
    // the pre-#704 suppressor encoded ("don't add implicit RELIDO
    // when explicit REL TO present") is preserved post-#704 because
    // the same mask (now on the default-fill gate) implements the
    // same gate.
    //
    // Authority: §B.3 Table 2 p21 (trigger authority — the
    // implicit-RELIDO default obligation); §B.3 paragraph b p19
    // (FD&R-absent gate); §B.3.a p19 (REL TO is canonical FD&R, in
    // MASK_FDR_DOMINATORS and therefore in
    // MASK_RELIDO_US_CLASS_SUPPRESSORS); §H.8 p154 (RELIDO marking
    // template — what RELIDO means once triggered).
    let scheme = CapcoScheme::new();
    assert!(
        !banner_carries_relido(
            &scheme,
            &["(S//REL TO USA, GBR/RELIDO)", "(S//DISPLAY ONLY GBR)"]
        ),
        "DISPLAY ONLY on one portion must evict RELIDO from another \
         portion at page projection (§H.8 p154 + §B.3.a p19 FD&R \
         dominator gate on default-fill Row 9); end-to-end behavior \
         identical to pre-#704.",
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
    let scheme = CapcoScheme::new();
    assert!(
        !banner_carries_relido(&scheme, &["(S//RELIDO)", "(S//OC)"]),
        "ORCON on one portion must evict RELIDO from another portion \
         at page projection (§H.8 p136); cross-portion case",
    );
}

#[test]
fn orcon_usgov_clears_relido_cross_portion() {
    // Cross-portion: portion A has RELIDO, portion B has ORCON-USGOV.
    // §H.8 p140 ORCON-USGOV entry: same exclusion as ORCON.
    let scheme = CapcoScheme::new();
    assert!(
        !banner_carries_relido(&scheme, &["(S//RELIDO)", "(S//OC-USGOV)"]),
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
    let scheme = CapcoScheme::new();
    let attrs = project_page(&scheme, &["(S//OC)"]);
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

    // Post-T044 predicate IDs for the retired E055/E056/E057 family
    // (per `legacy-rule-id-map.md` §2 — retired declarative-wrapper IDs).
    // These now route through the PageRewrite path silently; a
    // re-introduced Conflicts row would emit one of these predicate
    // IDs on the diagnostic stream. The pre-Copilot version compared
    // against the legacy "E055"/"E056"/"E057" string literals, which
    // were vacuous post-T044 (no rule's `predicate_id()` is those
    // strings anymore — the comparisons always passed and would have
    // hidden a regression).
    const RETIRED_PREDICATE_IDS: &[&str] = &[
        "portion.dissem.display-only-clears-relido", // E055 (§H.8 p154 + p163)
        "portion.dissem.orcon-clears-relido",        // E056 (§H.8 p136 + p154)
        "portion.dissem.orcon-usgov-clears-relido",  // E057 (§H.8 p140 + p154)
    ];

    for input in cases {
        let result = engine.lint(input);
        for d in &result.diagnostics {
            let id = d.rule.predicate_id();
            assert!(
                !RETIRED_PREDICATE_IDS.contains(&id),
                "Retired rule (post-T044 predicate {id}) fired on input \
                 {:?} — the Constraint::Conflicts row should be gone \
                 post-#559; the PageRewrite path \
                 (capco/*-clears-relido) is silent at the diagnostic \
                 surface",
                std::str::from_utf8(input).unwrap_or("<bytes>"),
            );
        }
    }
}
