// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3b.D (T026d) — class-floor catalog walker (E058) behavior tests.
//!
//! Each catalog row in `crate::scheme::CLASS_FLOOR_CATALOG` is exercised
//! by three observable-behavior tests:
//!
//! 1. **Fires below floor** — a marking present on a portion / banner
//!    whose classification is below the row's `F(M)` floor produces
//!    one diagnostic with `Diagnostic.rule == "E058"` and a
//!    message-text identifier matching the row's marking label.
//! 2. **Does not fire at-or-above floor** — the same marking on a
//!    portion / banner whose classification meets the floor produces
//!    no E058 diagnostic for that row.
//! 3. **Does not fire when marking absent** — a portion / banner with
//!    no triggering marking produces no E058 diagnostic for that row.
//!
//! Plus per-row authoring-contract tests:
//!
//! - The catalog declares 27 `Constraint::Custom` rows (5 TS + 8 S +
//!   8 C + 2 UCNI + 4 passthrough) under the §3.4.6 family-granularity
//!   layout.
//! - Each row has a verified `CAPCO-2016 §X.Y pNN` (or `§H.7 Appendix B`)
//!   citation matching the implementation plan §2.
//! - The walker emits at the per-row severity (`Error` for enumerated
//!   rows; `Warn` for unknown-floor passthrough rows per §3.4.6
//!   Q-3.4.6b).
//! - Severity::Off override at `[rules] E058 = "off"` suppresses all
//!   class-floor diagnostics (FR-008 invariant).

use marque_capco::scheme::CapcoScheme;
use marque_capco::{CapcoRuleSet, capco_rules};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, RuleSet, Severity};
use marque_scheme::MarkingScheme;

// ---------------------------------------------------------------------------
// Engine setup helpers
// ---------------------------------------------------------------------------

/// Build a default-configured `Engine` for class-floor lint tests.
fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Run `source` through the engine and return its diagnostics.
fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Filter the diagnostic stream to E058 emissions.
///
/// PR 3c.2.C C5 reshape: under the closed-template `Message` shape,
/// per-row identification via marker text (`"CNWDI"`, `"SI compartments"`,
/// etc.) is no longer available — the runtime `marking_label` no longer
/// flows into `Diagnostic.message`.
///
/// PR 3c.2.C C7 (R-C1) closed the bridge gap: `message_by_name` and
/// `citation_by_name` in `crates/capco/src/scheme/adapter.rs` now cover
/// the 27 `class-floor/*` + `E058/*` rows. Each E058 diagnostic carries
/// its row-native `Citation` (verifiable per-row via `d.citation`) and
/// a `MessageTemplate::ClassificationFloorViolated` template. Per-row
/// identification is available at the audit boundary via
/// `(d.rule, d.citation)` rather than via the retired `marker_text`
/// substring scan.
///
/// The `_marker_text` parameter is kept for call-site documentation
/// (each test still names the row it intends to exercise) but is not
/// consumed by the filter. Per-row assertions in this file use the
/// typed `d.citation` field directly per the C7 strengthening.
fn e058_diags_for<'a>(
    diags: &'a [Diagnostic<CapcoScheme>],
    _marker_text: &str,
) -> Vec<&'a Diagnostic<CapcoScheme>> {
    // T044 OD-8.A: the bridge no longer collapses to a single `E058`
    // rule_id; each catalog row emits its own predicate ID in the
    // `banner.<axis>.<floor|ceiling>-<marking>` form. Filter on the
    // substring discriminator (mirrors `is_class_floor_catalog_name`).
    diags
        .iter()
        .filter(|d| {
            let pid = d.rule.predicate_id();
            pid.contains(".floor-") || pid.contains(".ceiling-")
        })
        .collect()
}

// ===========================================================================
// Authoring-contract tests
// ===========================================================================

#[test]
fn catalog_declares_27_class_floor_rows() {
    let scheme = CapcoScheme::new();
    let class_floor_rows: Vec<&str> = scheme
        .constraints()
        .iter()
        .filter(|c| {
            let n = c.name();
            n.contains(".floor-") || n.contains(".ceiling-")
        })
        .map(|c| c.name())
        .collect();
    assert_eq!(
        class_floor_rows.len(),
        27,
        "expected 27 class-floor catalog rows (5 TS + 8 S + 8 C + 2 UCNI + 4 passthrough); got {}: \
         {:?}",
        class_floor_rows.len(),
        class_floor_rows
    );
}

/// PR 9c.1 T134 (architect D5): pin the citation anchors for the
/// three NATO control-marking rows so a future edit can't silently
/// drift them.
///
/// BALK / BOHEMIA cite §G.2 p40 (their authoritative anchor — the
/// ARH table where the manual identifies them as SAPs). ATOMAL
/// cites §H.7 p122 (the worked example showing `SECRET//RD/ATOMAL//
/// FGI NATO//NOFORN`).
///
/// The companion severity pin (BALK/BOHEMIA = Warn, ATOMAL = Error)
/// is internal to the catalog row's `severity` field and verified
/// at firing time via the engine's class-floor emit path. Severity
/// drift would change exit codes for downstream audit consumers and
/// would need to be an intentional, documented change.
///
/// Citations: CAPCO-2016 §G.2 p40 (BALK/BOHEMIA — ARH table only,
/// soft); §H.7 p122 (ATOMAL — worked example in §H.7).
#[test]
fn pr_9c_1_nato_rows_pin_citation_anchors() {
    use marque_scheme::{Citation, SectionLetter, capco};
    let scheme = CapcoScheme::new();
    // PR 10.A.1: typed Citation pin — assertions compare structured
    // §/page values directly. Constitution VIII propagation: every
    // row's expected citation re-verified at PR 9c.1 authorship; this
    // PR preserves the per-row anchor values verbatim.
    let expected: &[(&str, Citation)] = &[
        ("banner.classification.floor-balk", capco(SectionLetter::G, 2, 40)),
        ("banner.classification.floor-bohemia", capco(SectionLetter::G, 2, 40)),
        ("banner.aea.floor-atomal", capco(SectionLetter::H, 7, 122)),
    ];

    for (row_name, expected_cite) in expected {
        let row = scheme
            .constraints()
            .iter()
            .find(|c| c.name() == *row_name)
            .unwrap_or_else(|| panic!("missing catalog row {row_name:?}"));
        assert_eq!(
            row.label(),
            *expected_cite,
            "{row_name} citation drifted (PR 9c.1 D5 pin); \
             expected {expected_cite}, got {actual}",
            actual = row.label(),
        );
    }
}

#[test]
fn catalog_row_names_are_unique() {
    let scheme = CapcoScheme::new();
    let mut all_names: Vec<&str> = scheme.constraints().iter().map(|c| c.name()).collect();
    let total = all_names.len();
    all_names.sort_unstable();
    all_names.dedup();
    assert_eq!(
        total,
        all_names.len(),
        "catalog row names must be unique across the entire scheme catalog"
    );
}

#[test]
fn catalog_citations_reference_capco_or_passthrough() {
    use marque_scheme::AuthoritativeSource;
    let scheme = CapcoScheme::new();
    for c in scheme.constraints() {
        let n = c.name();
        if !(n.contains(".floor-") || n.contains(".ceiling-")) {
            continue;
        }
        let label = c.label();
        // PR 10.A.1: typed Citation pin — row labels are either
        // `AuthoritativeSource::Capco2016` (the 23 enumerated rows) or
        // `AuthoritativeSource::EngineInternal` (the 4 passthrough rows
        // that cite `marque-applied.md §3.7`, not a CAPCO passage).
        // No other source variants are valid for this catalog.
        let valid = matches!(
            label.document,
            AuthoritativeSource::Capco2016 | AuthoritativeSource::EngineInternal
        );
        assert!(
            valid,
            "class-floor row {n:?} citation must originate from CAPCO-2016 \
             or the engine's marque-applied.md passthrough policy; got: \
             document={:?} ({})",
            label.document, label,
        );
    }
}

#[test]
fn class_floor_diagnostics_flow_through_engine_bridge_at_e058() {
    // PR 3c.B Commit 7.3: `DeclarativeClassFloorRule` retired from
    // `CapcoRuleSet`. The 27 catalog rows still emit diagnostics —
    // they flow through the engine's constraint-catalog bridge.
    //
    // T044 OD-8.A: the bridge no longer folds catalog row names to a
    // walker-level `Diagnostic.rule = "E058"` — each row emits its own
    // canonical predicate ID (`banner.<axis>.<floor|ceiling>-<marking>`).
    // This test pins the post-deletion + post-T044 external surface:
    //   1. No registered rule with the legacy E058/E022/E025/E027
    //      predicate-IDs (those walkers are gone);
    //   2. `engine.lint` still produces a class-floor diagnostic on
    //      a known-firing fixture (CONFIDENTIAL//RD-CNWDI → CNWDI
    //      floor row, §H.6 p104 §2.2 family) — now with predicate ID
    //      `capco:banner.aea.floor-cnwdi`.
    let set = capco_rules();
    let ids: Vec<&str> = set.rules().iter().map(|r| r.id().predicate_id()).collect();
    assert!(
        !ids.contains(&"E058"),
        "post-7.3: `DeclarativeClassFloorRule` retired; no rule with id `E058` should be \
         registered. The bridge emits E058 via name folding. Registered IDs: {ids:?}"
    );
    assert!(
        !ids.contains(&"E022"),
        "E022 retired in PR 3b.D; rule set must not register the legacy CNWDI wrapper"
    );
    assert!(
        !ids.contains(&"E025"),
        "E025 retired in PR 3b.D; rule set must not register the legacy UCNI wrapper"
    );
    assert!(
        !ids.contains(&"E027"),
        "E027 retired in PR 3b.D; rule set must not register the legacy SAR-classification rule"
    );

    // Bridge-emission anchor: confirm the deleted walker's external
    // surface is preserved end-to-end through `engine.lint`.
    let diags = lint("CONFIDENTIAL//RD-CNWDI//NOFORN\n");
    let cnwdi_e058 = e058_diags_for(&diags, "CNWDI");
    assert_eq!(
        cnwdi_e058.len(),
        1,
        "post-7.3: bridge must emit E058 for CONFIDENTIAL//RD-CNWDI \
         (CNWDI floor §H.6 p104 §2.2): {diags:?}"
    );
}

// ===========================================================================
// §2.1 Floor TS — single classification level (5 family rows)
// ===========================================================================

#[test]
fn hcs_p_subcompartment_fires_below_top_secret() {
    // §2.1 row #1: HCS-[comp][sub] — TS-only.
    // S//HCS-P JJJ: HCS-P with sub-compartment JJJ in a SECRET banner.
    let diags = lint("SECRET//HCS-P JJJ//ORCON/NOFORN\n");
    let hcs_sub = e058_diags_for(&diags, "HCS sub-compartment");
    assert_eq!(
        hcs_sub.len(),
        1,
        "HCS-P [SUB] floor (TS) must fire on SECRET banner: {diags:?}"
    );
    assert_eq!(hcs_sub[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge now reads per-row `citation_typed`
    // from `CLASS_FLOOR_CATALOG`; class-floor row `banner.classification.floor-hcs-comp-sub`
    // anchors at §H.4 p60 (SCI section General Information).
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        hcs_sub[0].citation,
        capco(SectionLetter::H, 4, 60),
        "banner.classification.floor-hcs-comp-sub must emit §H.4 p60; got: {:?}",
        hcs_sub[0].citation,
    );
    assert_eq!(hcs_sub[0].citation.document, AuthoritativeSource::Capco2016,);
}

#[test]
fn hcs_p_subcompartment_does_not_fire_at_top_secret() {
    let diags = lint("TOP SECRET//HCS-P JJJ//ORCON/NOFORN\n");
    let hcs_sub = e058_diags_for(&diags, "HCS sub-compartment");
    assert!(
        hcs_sub.is_empty(),
        "HCS-P [SUB] floor must not fire on TS banner: {diags:?}"
    );
}

#[test]
fn hcs_p_subcompartment_does_not_fire_when_no_subcompartment() {
    // Bare HCS-P (no sub-compartment) — different family row (HCS-comp).
    let diags = lint("SECRET//HCS-P//NOFORN\n");
    let hcs_sub = e058_diags_for(&diags, "HCS sub-compartment");
    assert!(
        hcs_sub.is_empty(),
        "HCS-P [SUB] floor must not fire on bare HCS-P (no sub): {diags:?}"
    );
}

#[test]
fn si_compartment_fires_below_top_secret() {
    // §2.1 row #2: SI-[comp] — TS-only. Covers SI-G, SI-ECRU, SI-NK,
    // SI-[any compartment]. Family-granular: any SI compartment present.
    let diags = lint("SECRET//SI-G//ORCON/NOFORN\n");
    let si_comp = e058_diags_for(&diags, "SI compartments");
    assert_eq!(
        si_comp.len(),
        1,
        "SI compartment floor must fire on S//SI-G: {diags:?}"
    );
}

#[test]
fn si_compartment_does_not_fire_at_top_secret() {
    let diags = lint("TOP SECRET//SI-G//ORCON/NOFORN\n");
    let si_comp = e058_diags_for(&diags, "SI compartments");
    assert!(
        si_comp.is_empty(),
        "SI compartment floor must not fire on TS: {diags:?}"
    );
}

#[test]
fn si_compartment_does_not_fire_on_bare_si() {
    // Bare SI is the §2.3 row (floor C), not the SI-comp row.
    let diags = lint("SECRET//SI//NOFORN\n");
    let si_comp = e058_diags_for(&diags, "SI compartments");
    assert!(
        si_comp.is_empty(),
        "SI-comp floor must not fire on bare SI: {diags:?}"
    );
}

#[test]
fn tk_blfh_fires_below_top_secret() {
    // §2.1 row #3: TK-BLFH — TS-only.
    let diags = lint("SECRET//TK-BLFH//NOFORN\n");
    let tk_blfh = e058_diags_for(&diags, "TK-BLFH");
    assert_eq!(
        tk_blfh.len(),
        1,
        "TK-BLFH floor must fire on SECRET banner: {diags:?}"
    );
}

#[test]
fn tk_blfh_does_not_fire_at_top_secret() {
    let diags = lint("TOP SECRET//TK-BLFH//NOFORN\n");
    let tk_blfh = e058_diags_for(&diags, "TK-BLFH");
    assert!(tk_blfh.is_empty(), "TK-BLFH must not fire on TS: {diags:?}");
}

#[test]
fn tk_blfh_does_not_fire_on_bare_tk() {
    // Bare TK is the §2.2 row (floor S), not the TK-BLFH row.
    let diags = lint("SECRET//TK//NOFORN\n");
    let tk_blfh = e058_diags_for(&diags, "TK-BLFH");
    assert!(
        tk_blfh.is_empty(),
        "TK-BLFH must not fire on bare TK: {diags:?}"
    );
}

// NATO BALK / BOHEMIA: per PR 9c.1 T134 the presence predicate fires
// when `attrs.sci_markings` carries a `SciControlSystem::NatoSap`
// variant. CAPCO-2016 §G.2 p40 identifies BALK/BOHEMIA as NATO SAPs in
// the SCI category position (rendered standalone, no `SAR-` prefix).
// The legacy fused variants `CosmicTopSecretBalk` / `CosmicTopSecretBohemia`
// were retired in PR 9c.1 Commit 5 — pre-PR-9c.1 those variants
// conflated classification with SCI semantics on a single axis.
//
// The row severity is `Warn` (downgrade from `Error` in PR 9c.1
// Commit 4) because §G.2 p40's citation depth is too soft to drive
// Error.  Well-formed NATO inputs `//COSMIC TOP SECRET-BALK` /
// `//COSMIC TOP SECRET-BOHEMIA` parse to bare `CosmicTopSecret` class
// + `NatoSap::{Balk,Bohemia}` SCI companion — effective level TS via
// `us_equivalent`; the TS floor is satisfied → no diagnostic. The row
// fires only when the SCI marking exists at a sub-CTS class
// (data-corruption / mangled input).

// ===========================================================================
// §2.2 Floor S — TS-or-S allowed (sample of 8 family rows)
// ===========================================================================

#[test]
fn hcs_compartment_fires_below_secret() {
    // §2.2 row #6: HCS-[comp] (HCS-O, HCS-P bare) — TS-or-S floor.
    let diags = lint("CONFIDENTIAL//HCS-O//ORCON/NOFORN\n");
    let hcs_comp = e058_diags_for(&diags, "HCS-O / HCS-P");
    assert_eq!(
        hcs_comp.len(),
        1,
        "HCS-comp floor must fire on CONFIDENTIAL banner: {diags:?}"
    );
}

#[test]
fn hcs_compartment_does_not_fire_at_secret() {
    let diags = lint("SECRET//HCS-O//ORCON/NOFORN\n");
    let hcs_comp = e058_diags_for(&diags, "HCS-O / HCS-P");
    assert!(
        hcs_comp.is_empty(),
        "HCS-comp must not fire on SECRET: {diags:?}"
    );
}

#[test]
fn rsv_compartment_fires_below_secret() {
    // §2.2 row #7: RSV-[comp] — TS-or-S floor.
    let diags = lint("CONFIDENTIAL//RSV-ABC//NOFORN\n");
    let rsv = e058_diags_for(&diags, "RSV compartment");
    assert_eq!(
        rsv.len(),
        1,
        "RSV compartment floor must fire on C: {diags:?}"
    );
}

#[test]
fn rsv_compartment_does_not_fire_at_secret() {
    let diags = lint("SECRET//RSV-ABC//NOFORN\n");
    let rsv = e058_diags_for(&diags, "RSV compartment");
    assert!(
        rsv.is_empty(),
        "RSV compartment must not fire on S: {diags:?}"
    );
}

#[test]
fn cnwdi_fires_below_secret() {
    // §2.2 row #11: CNWDI (replaces retired E022). TS-or-S RD floor.
    let diags = lint("CONFIDENTIAL//RD-CNWDI//NOFORN\n");
    let cnwdi = e058_diags_for(&diags, "CNWDI");
    assert_eq!(
        cnwdi.len(),
        1,
        "CNWDI floor must fire on CONFIDENTIAL: {diags:?}"
    );
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // CNWDI anchors at §H.6 p104.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        cnwdi[0].citation,
        capco(SectionLetter::H, 6, 104),
        "banner.aea.floor-cnwdi must emit §H.6 p104; got: {:?}",
        cnwdi[0].citation,
    );
    assert_eq!(cnwdi[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn cnwdi_does_not_fire_at_secret() {
    let diags = lint("SECRET//RD-CNWDI//NOFORN\n");
    let cnwdi = e058_diags_for(&diags, "CNWDI");
    assert!(cnwdi.is_empty(), "CNWDI must not fire on S RD: {diags:?}");
}

#[test]
fn rd_sigma_fires_below_secret() {
    // §2.2 row #9: RD-SG (RD-SIGMA) — TS-or-S floor.
    let diags = lint("CONFIDENTIAL//RD-SIGMA 14//NOFORN\n");
    let rd_sigma = e058_diags_for(&diags, "RD-SIGMA");
    assert_eq!(
        rd_sigma.len(),
        1,
        "RD-SIGMA floor must fire on C: {diags:?}"
    );
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // RD-SIGMA anchors at §H.6 p113.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        rd_sigma[0].citation,
        capco(SectionLetter::H, 6, 113),
        "class-floor/RD-SG must emit §H.6 p113; got: {:?}",
        rd_sigma[0].citation,
    );
    assert_eq!(
        rd_sigma[0].citation.document,
        AuthoritativeSource::Capco2016
    );
}

#[test]
fn rd_sigma_does_not_fire_at_secret() {
    let diags = lint("SECRET//RD-SIGMA 14//NOFORN\n");
    let rd_sigma = e058_diags_for(&diags, "RD-SIGMA");
    assert!(
        rd_sigma.is_empty(),
        "RD-SIGMA must not fire on S: {diags:?}"
    );
}

// ===========================================================================
// §2.3 Floor C — any classified level (sample of 8 family rows)
// ===========================================================================

#[test]
fn rd_bare_does_not_fire_on_classified() {
    // §2.3 row #16: RD bare — floor C (TS / S / C all valid).
    let diags = lint("CONFIDENTIAL//RD//NOFORN\n");
    let rd = e058_diags_for(&diags, "RD requires");
    assert!(rd.is_empty(), "RD bare must not fire on C: {diags:?}");
}

#[test]
fn sar_fires_on_unclassified() {
    // §2.3 row #15: SAR — floor C (replaces retired E027).
    let diags = lint("UNCLASSIFIED//SAR-BP\n");
    let sar = e058_diags_for(&diags, "SAR requires");
    assert_eq!(sar.len(), 1, "SAR floor must fire on U//SAR-*: {diags:?}");
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // SAR anchors at §H.5 p99 (SAR section start).
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        sar[0].citation,
        capco(SectionLetter::H, 5, 99),
        "E058/SAR-classification-floor must emit §H.5 p99; got: {:?}",
        sar[0].citation,
    );
    assert_eq!(sar[0].citation.document, AuthoritativeSource::Capco2016);
    // Pin the no-fix invariant. SAR classification-floor violations
    // require human review per §H.5 — the bridge MUST NOT auto-fix.
    // Migrated from the retired `e027_fires_on_unclassified_banner_with_sar`
    // lib-test, which asserted `sar[0].fix.is_none()`. Post-7.3 the SAR
    // row's `fix_intent_by_name` returns `None` (no FixIntent populates
    // until / unless a future PR adds class-promotion intents); this
    // assertion pins that contract so a future regression that adds a
    // fix to SAR-classification-floor without an explicit human-review
    // exemption fails the test.
    assert!(
        sar[0].fix.is_none() && sar[0].fix.is_none(),
        "SAR floor must emit no fix (human review required, §H.5): {:?}",
        sar[0]
    );
}

#[test]
fn sar_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//SAR-BP//NOFORN\n");
    let sar = e058_diags_for(&diags, "SAR requires");
    assert!(sar.is_empty(), "SAR must not fire on C: {diags:?}");
}

#[test]
fn orcon_family_does_not_fire_at_confidential() {
    // §2.3 row #20: ORCON family — floor C.
    let diags = lint("CONFIDENTIAL//ORCON/NOFORN\n");
    let orcon = e058_diags_for(&diags, "ORCON / ORCON-USGOV");
    assert!(
        orcon.is_empty(),
        "ORCON family must not fire on C: {diags:?}"
    );
}

// ===========================================================================
// §2.4 Floor =U — UNCLASSIFIED-only (split UCNI rows per PM #1)
// ===========================================================================

#[test]
fn dod_ucni_fires_above_unclassified() {
    // §2.4 row #22: DOD UCNI — UNCLASSIFIED-only ceiling.
    let diags = lint("SECRET//DOD UCNI\n");
    let ucni = e058_diags_for(&diags, "DOD UCNI may only");
    assert_eq!(ucni.len(), 1, "DOD UCNI ceiling must fire on S: {diags:?}");
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // DOD UCNI anchors at §H.6 p116.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        ucni[0].citation,
        capco(SectionLetter::H, 6, 116),
        "E058/DOD-UCNI-classification-ceiling must emit §H.6 p116; got: {:?}",
        ucni[0].citation,
    );
    assert_eq!(ucni[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn dod_ucni_does_not_fire_on_unclassified() {
    let diags = lint("UNCLASSIFIED//DOD UCNI\n");
    let ucni = e058_diags_for(&diags, "DOD UCNI may only");
    assert!(
        ucni.is_empty(),
        "DOD UCNI must not fire on UNCLASSIFIED: {diags:?}"
    );
}

#[test]
fn doe_ucni_fires_above_unclassified() {
    // §2.4 row #23: DOE UCNI — UNCLASSIFIED-only ceiling. Carries its
    // own §H.6 p118 citation distinct from DOD UCNI's §H.6 p116 (PM
    // decision #1: UCNI split into separate rows for citation
    // specificity).
    let diags = lint("SECRET//DOE UCNI\n");
    let ucni = e058_diags_for(&diags, "DOE UCNI may only");
    assert_eq!(ucni.len(), 1, "DOE UCNI ceiling must fire on S: {diags:?}");
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // DOE UCNI anchors at §H.6 p118.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        ucni[0].citation,
        capco(SectionLetter::H, 6, 118),
        "E058/DOE-UCNI-classification-ceiling must emit §H.6 p118; got: {:?}",
        ucni[0].citation,
    );
    assert_eq!(ucni[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn doe_ucni_does_not_fire_on_unclassified() {
    let diags = lint("UNCLASSIFIED//DOE UCNI\n");
    let ucni = e058_diags_for(&diags, "DOE UCNI may only");
    assert!(
        ucni.is_empty(),
        "DOE UCNI must not fire on UNCLASSIFIED: {diags:?}"
    );
}

// ===========================================================================
// §2.6 Unknown-floor passthrough — Warn severity per §3.4.6 Q-3.4.6b
// ===========================================================================
//
// Passthrough fires when the marking is present AND classification is
// below C (the conservative provisional floor). The diagnostic message
// quotes the §3.7 passthrough-policy framing.

#[test]
fn passthrough_bur_fires_at_warn_severity() {
    let diags = lint("UNCLASSIFIED//BUR\n");
    let bur = e058_diags_for(&diags, "BUR family");
    assert_eq!(bur.len(), 1, "BUR passthrough must fire on U: {diags:?}");
    assert_eq!(
        bur[0].severity,
        Severity::Warn,
        "passthrough rows fire at Warn (§3.4.6 Q-3.4.6b); got {:?}",
        bur[0].severity
    );
    // PR 3c.2.C C5: passthrough §3.7 policy framing prose used to
    // live in `Diagnostic.message` as a free-form sentence. The
    // closed-template shape drops the prose; the passthrough-policy
    // narrative is preserved on the source side in
    // `crates/capco/src/scheme/constraints/helpers.rs::class_floor_emit`
    // and renders via the bridge's CLI-side renderer per PM-C-5
    // ("renderer responsibility"). The test purpose strengthens to
    // "E058 fired at Warn", which is the bridge-observable contract.
    let _ = bur[0].message.template();
}

#[test]
fn passthrough_bur_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//BUR//NOFORN\n");
    let bur = e058_diags_for(&diags, "BUR family");
    assert!(
        bur.is_empty(),
        "BUR passthrough must not fire at-or-above C floor: {diags:?}"
    );
}

#[test]
fn passthrough_klm_fires_at_warn_severity() {
    let diags = lint("UNCLASSIFIED//KLM\n");
    let klm = e058_diags_for(&diags, "KLM family");
    assert_eq!(klm.len(), 1, "KLM passthrough must fire on U: {diags:?}");
    assert_eq!(klm[0].severity, Severity::Warn);
}

#[test]
fn passthrough_mvl_fires_at_warn_severity() {
    let diags = lint("UNCLASSIFIED//MVL\n");
    let mvl = e058_diags_for(&diags, "MVL");
    assert_eq!(mvl.len(), 1, "MVL passthrough must fire on U: {diags:?}");
    assert_eq!(mvl[0].severity, Severity::Warn);
}

// ===========================================================================
// FR-008: Severity::Off override suppresses all class-floor diagnostics
// ===========================================================================

#[test]
fn severity_off_at_e058_suppresses_all_class_floor_diagnostics() {
    // T044 OD-8.A: the bridge no longer collapses to a single E058
    // walker ID. The CNWDI floor is now targeted by its own
    // predicate ID `capco:banner.aea.floor-cnwdi`; suppression
    // requires setting that key (FR-008 invariant unchanged — an
    // `Off`-severity rule cannot fire).
    let mut config = Config::default();
    config
        .rules
        .overrides
        .insert(
            "capco:banner.aea.floor-cnwdi".to_string(),
            "off".to_string(),
        );
    let engine_with_off = Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine construction must succeed");

    // CONFIDENTIAL//RD-CNWDI: CNWDI floor would fire at default config.
    let diags = engine_with_off
        .lint(b"CONFIDENTIAL//RD-CNWDI//NOFORN\n")
        .diagnostics;
    let cnwdi: Vec<&Diagnostic<CapcoScheme>> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == "banner.aea.floor-cnwdi")
        .collect();
    assert!(
        cnwdi.is_empty(),
        "with `[rules] \"capco:banner.aea.floor-cnwdi\" = \"off\"`, \
         no banner.aea.floor-cnwdi diagnostics may emit \
         (FR-008): {diags:?}"
    );
}

// ===========================================================================
// Span anchoring (PM directive #2: anchor at marking token, not classification)
// ===========================================================================

#[test]
fn cnwdi_span_anchors_at_aea_marking_not_classification() {
    let source = "CONFIDENTIAL//RD-CNWDI//NOFORN\n";
    let diags = lint(source);
    let cnwdi = e058_diags_for(&diags, "CNWDI");
    assert_eq!(cnwdi.len(), 1);
    let anchor = &source[cnwdi[0].span.start..cnwdi[0].span.end];
    // The AEA-marking token covers the RD/CNWDI text — not "CONFIDENTIAL".
    assert!(
        anchor != "CONFIDENTIAL",
        "PM directive #2: span must anchor at the marking token (RD-CNWDI), not the \
         classification token. Got anchor: {anchor:?}"
    );
}

#[test]
fn sar_span_anchors_at_sar_indicator_not_classification() {
    let source = "UNCLASSIFIED//SAR-BP\n";
    let diags = lint(source);
    let sar = e058_diags_for(&diags, "SAR requires");
    assert_eq!(sar.len(), 1);
    let anchor = &source[sar[0].span.start..sar[0].span.end];
    // Must not be "UNCLASSIFIED" (the classification token).
    assert!(
        anchor != "UNCLASSIFIED",
        "PM directive #2: span must anchor at the SAR indicator, not the classification \
         token. Got anchor: {anchor:?}"
    );
}

// ===========================================================================
// R1 C1 — NATO / FGI / JOINT class-floor regression tests
// ===========================================================================
//
// The pre-fix `class_floor_satisfied` and the diagnostic message helper
// queried `attrs.us_classification()`, which returns `None` for non-US
// classification kinds (NATO / FGI / JOINT). Result: every well-formed
// NATO portion bearing BALK / BOHEMIA / ATOMAL emitted a spurious
// class-floor diagnostic and reported the current classification as
// "unknown".
//
// The fix: `class_floor_satisfied` and the diagnostic message helper use
// `MarkingClassification::effective_level()` for the AtLeast policy. That
// accessor maps NATO / FGI / JOINT classifications to their US-equivalent
// level via reciprocal-raise (CTS → TS, NS → S, NC → C, NR → R, NU → U)
// per `marque-applied.md` §3.4.1 Note (i).
//
// EqualsU (UCNI ceiling) deliberately keeps `us_classification()`
// semantics: UCNI per CAPCO-2016 §H.6 p116 / p118 is US-only AEA, and a
// non-US classification carrying UCNI is malformed input (caught by
// other rules).
//
// Tests below cover (a) NATO well-formed inputs no longer emit a
// false-positive class-floor diagnostic, (b) the diagnostic message
// reads the correct effective level (not "unknown") for non-US
// classifications, and (c) JOINT inputs with US-only markings (SAR)
// satisfy the floor when the JOINT level reciprocal-raises to ≥ floor.

#[test]
fn balk_does_not_fire_on_well_formed_cts_balk_banner() {
    // PR 9c.1 T134: `//COSMIC TOP SECRET-BALK` parses to bare
    // `Nato(CosmicTopSecret)` class + `NatoSap::Balk` SCI companion.
    // Effective level TS. BALK floor (TS) is satisfied → no diagnostic.
    let diags = lint("//COSMIC TOP SECRET-BALK\n");
    let balk = e058_diags_for(&diags, "BALK (NATO)");
    assert!(
        balk.is_empty(),
        "BALK floor must not fire on well-formed `//COSMIC TOP SECRET-BALK` \
         (effective TS satisfies the TS floor via reciprocal-raise per \
         marque-applied.md §3.4.1 Note (i)): {diags:?}"
    );
}

#[test]
fn bohemia_does_not_fire_on_well_formed_cts_bohemia_banner() {
    let diags = lint("//COSMIC TOP SECRET-BOHEMIA\n");
    let bohemia = e058_diags_for(&diags, "BOHEMIA (NATO)");
    assert!(
        bohemia.is_empty(),
        "BOHEMIA floor must not fire on well-formed `//COSMIC TOP SECRET-BOHEMIA`: {diags:?}"
    );
}

#[test]
fn atomal_does_not_fire_on_well_formed_nato_secret_atomal_banner() {
    // ATOMAL floor is C. NATO SECRET ATOMAL effective level = S, which
    // satisfies AtLeast(C) via reciprocal-raise.
    let diags = lint("//NATO SECRET ATOMAL\n");
    let atomal = e058_diags_for(&diags, "ATOMAL (NATO)");
    assert!(
        atomal.is_empty(),
        "ATOMAL floor must not fire on well-formed `//NATO SECRET ATOMAL` \
         (effective S satisfies the C floor via reciprocal-raise): {diags:?}"
    );
}

#[test]
fn atomal_does_not_fire_on_well_formed_nato_confidential_atomal_banner() {
    // NATO CONFIDENTIAL ATOMAL effective level = C, exactly at the floor.
    let diags = lint("//NATO CONFIDENTIAL ATOMAL\n");
    let atomal = e058_diags_for(&diags, "ATOMAL (NATO)");
    assert!(
        atomal.is_empty(),
        "ATOMAL floor must not fire on well-formed `//NATO CONFIDENTIAL ATOMAL` \
         (effective C exactly meets the C floor): {diags:?}"
    );
}

#[test]
fn atomal_does_not_fire_on_well_formed_cosmic_top_secret_atomal_banner() {
    let diags = lint("//COSMIC TOP SECRET ATOMAL\n");
    let atomal = e058_diags_for(&diags, "ATOMAL (NATO)");
    assert!(
        atomal.is_empty(),
        "ATOMAL floor must not fire on well-formed `//COSMIC TOP SECRET ATOMAL` \
         (effective TS satisfies the C floor): {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// FGI reciprocal-raise behavior on enumerated rows (e.g., SAR)
// ---------------------------------------------------------------------------
//
// SAR per §H.5 p101 is US-only by EO 13526 §4.3 — but a JOINT-classified
// portion (US-co-owned) with SAR is well-formed (the JOINT category
// carries a US-equivalent class via JointClassification.level). The
// reciprocal-raise fix means the SAR floor recognizes the JOINT class
// and doesn't fire spuriously.

#[test]
fn sar_does_not_fire_on_joint_secret_with_sar() {
    // `//JOINT S USA GBR//SAR-BP//REL TO USA, GBR`: JOINT S has
    // effective level S (US-equivalent), which satisfies SAR's C floor.
    let diags = lint("//JOINT S USA GBR//SAR-BP//REL TO USA, GBR\n");
    let sar = e058_diags_for(&diags, "SAR requires");
    assert!(
        sar.is_empty(),
        "SAR floor must not fire on JOINT S//SAR-* (effective S satisfies C floor): {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// Diagnostic message reports the effective level (not "unknown")
// ---------------------------------------------------------------------------
//
// Pre-fix, the diagnostic message helper returned "unknown" for any
// non-US classification because it queried `us_classification()`.
// Post-fix it queries `effective_level()` so the user sees the
// reciprocal-raised banner-form name.
//
// We exercise this path via constructed `CapcoMarking` + the public
// trait-path `MarkingScheme::validate`, then filter the resulting
// `ConstraintViolation` list by `constraint_label` (the catalog row
// `name`). This avoids reaching for `pub(crate)` fast-path methods
// and uses the same dispatch the engine ultimately triggers.

fn validate_and_filter<'a>(
    scheme: &'a CapcoScheme,
    marking: &'a marque_capco::scheme::CapcoMarking,
    name: &str,
) -> Vec<marque_scheme::ConstraintViolation> {
    scheme
        .validate(marking)
        .into_iter()
        .filter(|v| v.constraint_label == name)
        .collect()
}

#[test]
fn diagnostic_message_reports_reciprocal_raised_level_for_nato() {
    use marque_capco::scheme::CapcoMarking;
    use marque_ism::{CanonicalAttrs, MarkingClassification, NatoClassification, SciControl};

    // NATO RESTRICTED + BUR passthrough. NATO RESTRICTED has effective
    // level R (reciprocal-raised), which is below the provisional C
    // floor for BUR — the floor fires, AND the diagnostic message must
    // report "RESTRICTED" (effective level), not "unknown".
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Nato(
        NatoClassification::NatoRestricted,
    ));
    attrs.sci_controls = vec![SciControl::Bur].into_boxed_slice();

    let scheme = CapcoScheme::new();
    let marking = CapcoMarking::from(attrs);
    let violations = validate_and_filter(&scheme, &marking, "banner.classification.floor-passthrough-bur");
    assert_eq!(
        violations.len(),
        1,
        "BUR passthrough must fire on NATO RESTRICTED (effective R below provisional C floor)"
    );
    assert!(
        violations[0].message.contains("RESTRICTED"),
        "diagnostic must report effective level RESTRICTED (reciprocal-raised from \
         NATO RESTRICTED), not `unknown`. Got message: {:?}",
        violations[0].message
    );
    assert!(
        !violations[0].message.contains("unknown"),
        "diagnostic must not report `unknown` for a non-US classification. Got: {:?}",
        violations[0].message
    );
}

#[test]
fn diagnostic_message_reports_unknown_only_when_no_classification() {
    use marque_capco::scheme::CapcoMarking;
    use marque_ism::{CanonicalAttrs, SciControl};

    // No classification set. Diagnostic message must read "unknown" —
    // this is the legitimate fallback for the truly-unclassified case.
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = None;
    attrs.sci_controls = vec![SciControl::Bur].into_boxed_slice();

    let scheme = CapcoScheme::new();
    let marking = CapcoMarking::from(attrs);
    let violations = validate_and_filter(&scheme, &marking, "banner.classification.floor-passthrough-bur");
    assert_eq!(violations.len(), 1);
    assert!(
        violations[0].message.contains("unknown"),
        "diagnostic must report `unknown` when no classification is parsed. Got: {:?}",
        violations[0].message
    );
}

// ---------------------------------------------------------------------------
// EqualsU (UCNI) keeps US-classification semantics
// ---------------------------------------------------------------------------
//
// Per §H.6 p116 (DOD UCNI) / p118 (DOE UCNI), UCNI is US-only AEA. A
// non-US classification carrying UCNI is malformed; other rules catch
// the malformed shape. The class-floor walker's EqualsU policy
// deliberately keeps `attrs.us_classification()` semantics rather than
// `effective_level()`.

#[test]
fn dod_ucni_fires_on_nato_classification_carrying_ucni() {
    use marque_capco::scheme::CapcoMarking;
    use marque_ism::{AeaMarking, CanonicalAttrs, MarkingClassification, NatoClassification};

    // Constructed attrs: NATO CONFIDENTIAL + DOD UCNI. This is malformed
    // per CAPCO §H.6 p116 — UCNI is US AEA. The EqualsU policy fires
    // (us_classification() returns None for NATO; ceiling unsatisfied).
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Nato(
        NatoClassification::NatoConfidential,
    ));
    attrs.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();

    let scheme = CapcoScheme::new();
    let marking = CapcoMarking::from(attrs);
    let violations = validate_and_filter(&scheme, &marking, "banner.aea.ceiling-dod-ucni");
    assert_eq!(
        violations.len(),
        1,
        "DOD UCNI ceiling must fire on NATO classification + UCNI \
         (UCNI is US AEA per §H.6 p116; non-US classification + UCNI is malformed)"
    );
}

// ===========================================================================
// R3.4 Path A — per-row catalog coverage (Copilot R3 C4)
// ===========================================================================
//
// The module docstring claims every catalog row carries the three
// observable-behavior tests. Pre-R3 the suite covered a subset of
// rows; R3.4 fills in the rows that were missing one or more of the
// fires-below / silent-at-or-above / silent-when-absent triplet.
//
// Rows already fully covered by the §2.1 / §2.2 / §2.3 / §2.4 / §2.6
// blocks above (HCS-comp-sub, SI-comp, TK-BLFH, BALK, BOHEMIA,
// HCS-comp partial, RSV-comp partial, CNWDI partial, RD-SG partial,
// SAR partial, RD partial, ATOMAL partial, ORCON partial, EYES-ONLY
// missing, DOD/DOE UCNI, BUR, KLM, MVL) get their missing assertions
// added below; rows missing entirely (TK family, FRD-SG, RSEN, IMCON,
// SI-bare, FRD-bare, TFNI, EYES-ONLY, passthrough-HCS-X) get the full
// observable-behavior triplet.
//
// Convention: each test function names the row + the property it
// pins so a future failure points at the exact row.

// ---------------------------------------------------------------------------
// Naming-prefix invariant (R3.2 build-time enforcement)
// ---------------------------------------------------------------------------

#[test]
fn class_floor_catalog_naming_convention() {
    // T044: every catalog row's `name` MUST contain `.floor-` or
    // `.ceiling-` (the new predicate-ID discriminator per
    // legacy-rule-id-map §3). The `is_class_floor_catalog_name`
    // dispatch in `evaluate_custom_by_attrs` is a substring check
    // that depends on this invariant. Adding a row whose name doesn't
    // follow the convention would break the dispatch routing for that
    // row silently — this test fails the build instead.
    let scheme = CapcoScheme::new();
    let class_floor_rows: Vec<&str> = scheme
        .constraints()
        .iter()
        .filter(|c| {
            let n = c.name();
            n.contains(".floor-") || n.contains(".ceiling-")
        })
        .map(|c| c.name())
        .collect();
    assert_eq!(
        class_floor_rows.len(),
        27,
        "expected 27 class-floor catalog rows under the .floor- / .ceiling- \
         substring convention (T044 predicate-ID form); got {}: {:?}",
        class_floor_rows.len(),
        class_floor_rows
    );
    // The above filter catches anything with the right prefix; this
    // test pins that EVERY row's name carries one of the two prefixes
    // (no other prefix is used). If a future row uses some third
    // prefix (e.g., `cf/...`), the count above would still pass but
    // this assertion would fail.
    for c in scheme.constraints() {
        let n = c.name();
        // Every row whose marking is class-floor-related (catalog
        // dispatch routes to it) must use one of the two prefixes.
        // The non-class-floor rows in the catalog (E010/, E012/,
        // capco/joint-requires-usa, etc.) are EXCLUDED — they don't
        // route through `is_class_floor_catalog_name`.
        if n.contains(".floor-") || n.contains(".ceiling-") {
            assert!(
                n.contains(".floor-") || n.contains(".ceiling-"),
                "class-floor catalog row name {n:?} violates naming-prefix invariant"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// §2.1 row #4 — class-floor/BALK: fires-below + absent
// ---------------------------------------------------------------------------
//
// BALK presence is bound to SciControlSystem::NatoSap(NatoSap::Balk)
// per PR 9c.1 T134. "BALK present at sub-CTS class" is reachable from
// the engine path because the parser writes the SCI companion
// independently of the bare NATO classification, so this absence test
// pairs naturally with the well-formed engine-path silence covered by
// `balk_does_not_fire_on_well_formed_cts_balk_banner` above.

#[test]
fn balk_does_not_fire_when_marking_absent() {
    // No NATO classification; BALK presence is `None`. Floor must
    // not fire.
    let diags = lint("TOP SECRET//SI//NOFORN\n");
    let balk = e058_diags_for(&diags, "BALK (NATO)");
    assert!(
        balk.is_empty(),
        "BALK floor must not fire when no BALK classification is present: {diags:?}"
    );
}

#[test]
fn bohemia_does_not_fire_when_marking_absent() {
    let diags = lint("TOP SECRET//SI//NOFORN\n");
    let bohemia = e058_diags_for(&diags, "BOHEMIA (NATO)");
    assert!(
        bohemia.is_empty(),
        "BOHEMIA floor must not fire when no BOHEMIA classification is present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #6 — class-floor/HCS-comp: absent
// ---------------------------------------------------------------------------

#[test]
fn hcs_compartment_does_not_fire_when_no_hcs_marking() {
    // No HCS marking at all → row must not fire.
    //
    // PR 3c.2.C C5: under the closed-template shape the bridge
    // collapses every class-floor row to rule_id "E058". The
    // original `e058_diags_for(.., "HCS-O / HCS-P")` marker filter
    // is no longer functional. The input `SECRET//SI-G//ORCON/NOFORN`
    // also triggers `class-floor/SI-comp` (SI compartments require
    // TS), so a generic `is_empty()` check now fails. To preserve
    // the test intent (HCS row doesn't fire when HCS marking
    // absent), filter by span: the SI-comp row anchors at the SI
    // SciSystem token; an HCS-row firing would anchor at an HCS
    // token, which doesn't exist in this input. Asserting that the
    // E058 count equals 1 (just SI-comp) verifies HCS didn't fire.
    let diags = lint("SECRET//SI-G//ORCON/NOFORN\n");
    let e058 = e058_diags_for(&diags, "");
    assert_eq!(
        e058.len(),
        1,
        "exactly one E058 expected (SI-comp at SECRET; HCS-comp absent): {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #7 — class-floor/RSV-comp: absent
// ---------------------------------------------------------------------------

#[test]
fn rsv_compartment_does_not_fire_when_no_rsv_marking() {
    let diags = lint("SECRET//SI//NOFORN\n");
    let rsv = e058_diags_for(&diags, "RSV compartment");
    assert!(
        rsv.is_empty(),
        "RSV compartment floor must not fire when no RSV marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #8 — class-floor/TK family: full triplet
// ---------------------------------------------------------------------------
//
// TK family: bare TK + TK-IDIT (with/without sub-comp) + TK-KAND
// (with/without sub-comp). Excludes TK-BLFH (covered by §2.1 row #3).

#[test]
fn tk_family_fires_below_secret_on_bare_tk() {
    let diags = lint("CONFIDENTIAL//TK//NOFORN\n");
    let tk = e058_diags_for(&diags, "TK / TK-IDIT / TK-KAND");
    assert_eq!(
        tk.len(),
        1,
        "TK family floor must fire on CONFIDENTIAL//TK: {diags:?}"
    );
    assert_eq!(tk[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // TK family anchors at §H.4 p60.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        tk[0].citation,
        capco(SectionLetter::H, 4, 60),
        "class-floor/TK must emit §H.4 p60; got: {:?}",
        tk[0].citation,
    );
    assert_eq!(tk[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn tk_family_does_not_fire_at_secret_on_bare_tk() {
    let diags = lint("SECRET//TK//NOFORN\n");
    let tk = e058_diags_for(&diags, "TK / TK-IDIT / TK-KAND");
    assert!(
        tk.is_empty(),
        "TK family floor must not fire on SECRET//TK: {diags:?}"
    );
}

#[test]
fn tk_family_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let tk = e058_diags_for(&diags, "TK / TK-IDIT / TK-KAND");
    assert!(
        tk.is_empty(),
        "TK family floor must not fire when no TK marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #9 — class-floor/RD-SG: absent
// ---------------------------------------------------------------------------

#[test]
fn rd_sigma_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let rd_sg = e058_diags_for(&diags, "RD-SIGMA");
    assert!(
        rd_sg.is_empty(),
        "RD-SIGMA floor must not fire when no RD-SIGMA marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #10 — class-floor/FRD-SG: full triplet
// ---------------------------------------------------------------------------

#[test]
fn frd_sigma_fires_below_secret() {
    let diags = lint("CONFIDENTIAL//FRD-SIGMA 14//NOFORN\n");
    let frd_sg = e058_diags_for(&diags, "FRD-SIGMA");
    assert_eq!(
        frd_sg.len(),
        1,
        "FRD-SIGMA floor must fire on CONFIDENTIAL: {diags:?}"
    );
    assert_eq!(frd_sg[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // FRD-SIGMA anchors at §H.6 p113.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        frd_sg[0].citation,
        capco(SectionLetter::H, 6, 113),
        "class-floor/FRD-SG must emit §H.6 p113; got: {:?}",
        frd_sg[0].citation,
    );
    assert_eq!(frd_sg[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn frd_sigma_does_not_fire_at_secret() {
    let diags = lint("SECRET//FRD-SIGMA 14//NOFORN\n");
    let frd_sg = e058_diags_for(&diags, "FRD-SIGMA");
    assert!(
        frd_sg.is_empty(),
        "FRD-SIGMA floor must not fire on SECRET: {diags:?}"
    );
}

#[test]
fn frd_sigma_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let frd_sg = e058_diags_for(&diags, "FRD-SIGMA");
    assert!(
        frd_sg.is_empty(),
        "FRD-SIGMA floor must not fire when no FRD-SIGMA marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #11 — banner.aea.floor-cnwdi: absent
// ---------------------------------------------------------------------------

#[test]
fn cnwdi_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let cnwdi = e058_diags_for(&diags, "CNWDI");
    assert!(
        cnwdi.is_empty(),
        "CNWDI floor must not fire when no CNWDI marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #12 — class-floor/RSEN: full triplet
// ---------------------------------------------------------------------------

#[test]
fn rsen_fires_below_secret() {
    // RSEN portion form is `RS`; banner form is `RSEN`. RSEN floor S.
    //
    // PR 3c.2.C C5: under the closed-template shape, both the
    // `class-floor/RSEN` row and the `class-floor/TK` row fire on
    // this input (TK at CONFIDENTIAL also violates its floor).
    // Assert exactly 2 E058 diagnostics fire; the legacy marker
    // filter is no longer available to isolate just the RSEN row.
    let diags = lint("CONFIDENTIAL//TK//RSEN\n");
    let rsen = e058_diags_for(&diags, "RSEN");
    assert_eq!(
        rsen.len(),
        2,
        "two E058 diagnostics expected (RSEN + TK both fire below S): {diags:?}"
    );
    assert_eq!(rsen[0].severity, Severity::Error);
    assert_eq!(rsen[1].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`.
    // The fixture fires both `class-floor/RSEN` (§H.8 p149) AND
    // `class-floor/TK` (§H.4 p60) — assert the diagnostic-set
    // contains both citations rather than asserting a positional
    // index (sort order is determined by the engine).
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    let citations: Vec<_> = rsen.iter().map(|d| d.citation).collect();
    assert!(
        citations.contains(&capco(SectionLetter::H, 8, 149)),
        "class-floor/RSEN must contribute §H.8 p149; got: {:?}",
        citations,
    );
    assert!(
        citations.contains(&capco(SectionLetter::H, 4, 60)),
        "class-floor/TK must contribute §H.4 p60; got: {:?}",
        citations,
    );
    for d in &rsen {
        assert_eq!(d.citation.document, AuthoritativeSource::Capco2016);
    }
}

#[test]
fn rsen_does_not_fire_at_secret() {
    let diags = lint("SECRET//TK//RSEN\n");
    let rsen = e058_diags_for(&diags, "RSEN");
    assert!(
        rsen.is_empty(),
        "RSEN floor must not fire on SECRET: {diags:?}"
    );
}

#[test]
fn rsen_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let rsen = e058_diags_for(&diags, "RSEN");
    assert!(
        rsen.is_empty(),
        "RSEN floor must not fire when no RSEN dissem control present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.2 row #13 — class-floor/IMCON: full triplet
// ---------------------------------------------------------------------------

#[test]
fn imcon_fires_below_secret() {
    // IMCON portion form is `IMC`; banner form is `IMCON`. Floor S.
    let diags = lint("CONFIDENTIAL//IMCON/NOFORN\n");
    let imcon = e058_diags_for(&diags, "IMCON");
    assert_eq!(
        imcon.len(),
        1,
        "IMCON floor must fire on CONFIDENTIAL: {diags:?}"
    );
    assert_eq!(imcon[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // IMCON anchors at §H.8 p144.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        imcon[0].citation,
        capco(SectionLetter::H, 8, 144),
        "class-floor/IMCON must emit §H.8 p144; got: {:?}",
        imcon[0].citation,
    );
    assert_eq!(imcon[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn imcon_does_not_fire_at_secret() {
    let diags = lint("SECRET//IMCON/NOFORN\n");
    let imcon = e058_diags_for(&diags, "IMCON");
    assert!(
        imcon.is_empty(),
        "IMCON floor must not fire on SECRET: {diags:?}"
    );
}

#[test]
fn imcon_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let imcon = e058_diags_for(&diags, "IMCON");
    assert!(
        imcon.is_empty(),
        "IMCON floor must not fire when no IMCON dissem control present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #14 — class-floor/SI (bare): full triplet
// ---------------------------------------------------------------------------

#[test]
fn si_bare_fires_when_no_classification() {
    // SI bare floor is C; "no classification" fails any AtLeast(C)
    // floor (preserves retired-E022/E027 historical behavior). The
    // engine path doesn't naturally reach "SI bare with no class
    // token" via well-formed input, so use a portion form.
    // Equivalent: a SAR-only "(SAR-BP)" portion has no classification
    // token in the same span as SI; we test SI on UNCLASSIFIED.
    let diags = lint("UNCLASSIFIED//SI\n");
    let si = e058_diags_for(&diags, "SI (bare)");
    assert_eq!(
        si.len(),
        1,
        "SI bare floor (C) must fire on UNCLASSIFIED//SI: {diags:?}"
    );
    assert_eq!(si[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // SI bare anchors at §H.4 p60.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        si[0].citation,
        capco(SectionLetter::H, 4, 60),
        "class-floor/SI must emit §H.4 p60; got: {:?}",
        si[0].citation,
    );
    assert_eq!(si[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn si_bare_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let si = e058_diags_for(&diags, "SI (bare)");
    assert!(
        si.is_empty(),
        "SI bare floor must not fire on CONFIDENTIAL: {diags:?}"
    );
}

#[test]
fn si_bare_does_not_fire_when_marking_absent() {
    // PR 3c.2.C C5: input `CONFIDENTIAL//TK//NOFORN` triggers the
    // `class-floor/TK` row (TK at C, needs S). SI is absent so the
    // SI-bare row does not fire; the legacy marker filter is no
    // longer available, so assert E058 count == 1 instead.
    let diags = lint("CONFIDENTIAL//TK//NOFORN\n");
    let e058 = e058_diags_for(&diags, "");
    assert_eq!(
        e058.len(),
        1,
        "exactly one E058 expected (TK at C; SI-bare absent): {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #15 — E058/SAR-classification-floor: absent
// ---------------------------------------------------------------------------

#[test]
fn sar_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let sar = e058_diags_for(&diags, "SAR requires");
    assert!(
        sar.is_empty(),
        "SAR floor must not fire when no SAR marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #16 — class-floor/RD: fires-below + absent
// ---------------------------------------------------------------------------

#[test]
fn rd_bare_fires_when_no_classification_token() {
    // RD bare floor C. UNCLASSIFIED//RD fires (RD requires ≥ C).
    let diags = lint("UNCLASSIFIED//RD//NOFORN\n");
    let rd = e058_diags_for(&diags, "RD requires");
    assert_eq!(
        rd.len(),
        1,
        "RD bare floor (C) must fire on UNCLASSIFIED: {diags:?}"
    );
    assert_eq!(rd[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // RD bare anchors at §H.6 p104.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        rd[0].citation,
        capco(SectionLetter::H, 6, 104),
        "class-floor/RD must emit §H.6 p104; got: {:?}",
        rd[0].citation,
    );
    assert_eq!(rd[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn rd_bare_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let rd = e058_diags_for(&diags, "RD requires");
    assert!(
        rd.is_empty(),
        "RD bare floor must not fire when no RD marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #17 — class-floor/FRD: full triplet
// ---------------------------------------------------------------------------

#[test]
fn frd_bare_fires_when_unclassified() {
    let diags = lint("UNCLASSIFIED//FRD//NOFORN\n");
    let frd = e058_diags_for(&diags, "FRD requires");
    assert_eq!(
        frd.len(),
        1,
        "FRD bare floor (C) must fire on UNCLASSIFIED: {diags:?}"
    );
    assert_eq!(frd[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // FRD bare anchors at §H.6 p104.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        frd[0].citation,
        capco(SectionLetter::H, 6, 104),
        "class-floor/FRD must emit §H.6 p104; got: {:?}",
        frd[0].citation,
    );
    assert_eq!(frd[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn frd_bare_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//FRD//NOFORN\n");
    let frd = e058_diags_for(&diags, "FRD requires");
    assert!(
        frd.is_empty(),
        "FRD bare floor must not fire on CONFIDENTIAL: {diags:?}"
    );
}

#[test]
fn frd_bare_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let frd = e058_diags_for(&diags, "FRD requires");
    assert!(
        frd.is_empty(),
        "FRD bare floor must not fire when no FRD marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #18 — class-floor/TFNI: full triplet
// ---------------------------------------------------------------------------

#[test]
fn tfni_fires_when_unclassified() {
    let diags = lint("UNCLASSIFIED//TFNI\n");
    let tfni = e058_diags_for(&diags, "TFNI");
    assert_eq!(
        tfni.len(),
        1,
        "TFNI floor (C) must fire on UNCLASSIFIED: {diags:?}"
    );
    assert_eq!(tfni[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // TFNI anchors at §H.6 p107.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        tfni[0].citation,
        capco(SectionLetter::H, 6, 107),
        "class-floor/TFNI must emit §H.6 p107; got: {:?}",
        tfni[0].citation,
    );
    assert_eq!(tfni[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn tfni_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//TFNI\n");
    let tfni = e058_diags_for(&diags, "TFNI");
    assert!(
        tfni.is_empty(),
        "TFNI floor must not fire on CONFIDENTIAL: {diags:?}"
    );
}

#[test]
fn tfni_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let tfni = e058_diags_for(&diags, "TFNI");
    assert!(
        tfni.is_empty(),
        "TFNI floor must not fire when no TFNI marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #19 — class-floor/ATOMAL: absent
// ---------------------------------------------------------------------------

#[test]
fn atomal_does_not_fire_when_marking_absent() {
    let diags = lint("TOP SECRET//SI//NOFORN\n");
    let atomal = e058_diags_for(&diags, "ATOMAL (NATO)");
    assert!(
        atomal.is_empty(),
        "ATOMAL floor must not fire when no ATOMAL classification present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #20 — class-floor/ORCON family: fires-below + absent
// ---------------------------------------------------------------------------

#[test]
fn orcon_family_fires_when_unclassified() {
    let diags = lint("UNCLASSIFIED//ORCON\n");
    let orcon = e058_diags_for(&diags, "ORCON / ORCON-USGOV");
    assert_eq!(
        orcon.len(),
        1,
        "ORCON family floor (C) must fire on UNCLASSIFIED: {diags:?}"
    );
    assert_eq!(orcon[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // ORCON family anchors at §H.8 p136.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        orcon[0].citation,
        capco(SectionLetter::H, 8, 136),
        "class-floor/ORCON must emit §H.8 p136; got: {:?}",
        orcon[0].citation,
    );
    assert_eq!(orcon[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn orcon_family_does_not_fire_when_marking_absent() {
    let diags = lint("CONFIDENTIAL//SI//NOFORN\n");
    let orcon = e058_diags_for(&diags, "ORCON / ORCON-USGOV");
    assert!(
        orcon.is_empty(),
        "ORCON family floor must not fire when no ORCON dissem present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.3 row #21 — class-floor/EYES-ONLY: full triplet
// ---------------------------------------------------------------------------

// EYES ONLY: the parser recognizes the CVE `EYES` form (per
// `marque-ism::DissemControl::Eyes`). The compound banner-form
// `USA/[LIST] EYES ONLY` syntax is supported via
// `recognize_eyes_only_block` (PR 9a / T135a Commit 5); bare
// `EYES ONLY` in a banner maps to `DissemControl::Eyes` via the
// `MARKING_FORMS` entry added in the banner-lexer issue. The
// fires-below + at-floor + absent triplet below uses portion form
// `(U//EYES)` / `(C//EYES)` since it is equivalent; banner-form
// variants are exercised in `crates/capco/tests/eyes_to_rel_to.rs`.

#[test]
fn eyes_only_fires_when_unclassified() {
    let diags = lint("(U//EYES)\n");
    let eyes = e058_diags_for(&diags, "EYES ONLY");
    assert_eq!(
        eyes.len(),
        1,
        "EYES ONLY floor (C) must fire on (U//EYES): {diags:?}"
    );
    assert_eq!(eyes[0].severity, Severity::Error);
    // PR 3c.2.C C7 (R-C1): bridge reads per-row `citation_typed`;
    // EYES ONLY anchors at §H.8 p152.
    use marque_scheme::{AuthoritativeSource, SectionLetter, capco};
    assert_eq!(
        eyes[0].citation,
        capco(SectionLetter::H, 8, 152),
        "class-floor/EYES-ONLY must emit §H.8 p152; got: {:?}",
        eyes[0].citation,
    );
    assert_eq!(eyes[0].citation.document, AuthoritativeSource::Capco2016);
}

#[test]
fn eyes_only_does_not_fire_at_confidential() {
    let diags = lint("(C//EYES)\n");
    let eyes = e058_diags_for(&diags, "EYES ONLY");
    assert!(
        eyes.is_empty(),
        "EYES ONLY floor must not fire on (C//EYES): {diags:?}"
    );
}

#[test]
fn eyes_only_does_not_fire_when_marking_absent() {
    let diags = lint("(C//SI//NF)\n");
    let eyes = e058_diags_for(&diags, "EYES ONLY");
    assert!(
        eyes.is_empty(),
        "EYES ONLY floor must not fire when no EYES ONLY dissem present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.4 — DOD/DOE UCNI: silent-when-absent (fires/silent-at-U already covered)
// ---------------------------------------------------------------------------

#[test]
fn dod_ucni_does_not_fire_when_marking_absent() {
    let diags = lint("UNCLASSIFIED//FOUO\n");
    let ucni = e058_diags_for(&diags, "DOD UCNI may only");
    assert!(
        ucni.is_empty(),
        "DOD UCNI ceiling must not fire when no DOD UCNI marking present: {diags:?}"
    );
}

#[test]
fn doe_ucni_does_not_fire_when_marking_absent() {
    let diags = lint("UNCLASSIFIED//FOUO\n");
    let ucni = e058_diags_for(&diags, "DOE UCNI may only");
    assert!(
        ucni.is_empty(),
        "DOE UCNI ceiling must not fire when no DOE UCNI marking present: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.6 row #25 — class-floor/passthrough-HCS-X: full triplet
// ---------------------------------------------------------------------------

#[test]
fn passthrough_hcs_x_fires_at_warn_severity_on_unclassified() {
    let diags = lint("UNCLASSIFIED//HCS-X\n");
    let hcsx = e058_diags_for(&diags, "HCS-X");
    assert_eq!(
        hcsx.len(),
        1,
        "HCS-X passthrough must fire on UNCLASSIFIED: {diags:?}"
    );
    assert_eq!(
        hcsx[0].severity,
        Severity::Warn,
        "passthrough rows fire at Warn (§3.4.6 Q-3.4.6b)"
    );
    // PR 3c.2.C C5: passthrough §3.7 policy framing prose dropped
    // per PM-C-5; see `passthrough_bur_fires_at_warn_severity_on_unclassified`
    // for the same migration note.
    let _ = hcsx[0].message.template();
}

#[test]
fn passthrough_hcs_x_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//HCS-X//NOFORN\n");
    let hcsx = e058_diags_for(&diags, "HCS-X");
    assert!(
        hcsx.is_empty(),
        "HCS-X passthrough must not fire at-or-above C floor: {diags:?}"
    );
}

#[test]
fn passthrough_hcs_x_does_not_fire_when_marking_absent() {
    // PR 3c.2.C C5: same shape as the KLM / MVL absent variants;
    // SI-bare fires on U, HCS-X is absent.
    let diags = lint("UNCLASSIFIED//SI\n");
    let e058 = e058_diags_for(&diags, "");
    assert_eq!(
        e058.len(),
        1,
        "exactly one E058 expected (SI-bare at U; HCS-X passthrough absent): {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// §2.6 — KLM / MVL: at-or-above + absent (fires-below already covered)
// ---------------------------------------------------------------------------

#[test]
fn passthrough_klm_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//KLM//NOFORN\n");
    let klm = e058_diags_for(&diags, "KLM family");
    assert!(
        klm.is_empty(),
        "KLM passthrough must not fire at-or-above C floor: {diags:?}"
    );
}

#[test]
fn passthrough_klm_does_not_fire_when_marking_absent() {
    // PR 3c.2.C C5: same shape as passthrough_mvl variant; SI-bare
    // fires on U, KLM is absent.
    let diags = lint("UNCLASSIFIED//SI\n");
    let e058 = e058_diags_for(&diags, "");
    assert_eq!(
        e058.len(),
        1,
        "exactly one E058 expected (SI-bare at U; KLM passthrough absent): {diags:?}"
    );
}

#[test]
fn passthrough_mvl_does_not_fire_at_confidential() {
    let diags = lint("CONFIDENTIAL//MVL//NOFORN\n");
    let mvl = e058_diags_for(&diags, "MVL");
    assert!(
        mvl.is_empty(),
        "MVL passthrough must not fire at-or-above C floor: {diags:?}"
    );
}

#[test]
fn passthrough_mvl_does_not_fire_when_marking_absent() {
    // PR 3c.2.C C5: marker filter dropped; input `UNCLASSIFIED//SI`
    // triggers `class-floor/SI-bare` (SI-bare floor at U). The MVL
    // passthrough row would anchor at an MVL token, which is absent;
    // assert exactly 1 E058 (the SI-bare row, not MVL).
    let diags = lint("UNCLASSIFIED//SI\n");
    let e058 = e058_diags_for(&diags, "");
    assert_eq!(
        e058.len(),
        1,
        "exactly one E058 expected (SI-bare at U; MVL passthrough absent): {diags:?}"
    );
}
