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
fn lint(source: &str) -> Vec<Diagnostic> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Filter the diagnostic stream to the E058 emissions whose message
/// contains `marker_text` (substring match). The catalog uses one rule
/// ID `E058` for all 27 rows; per-row identification flows via the
/// diagnostic message text. The marker_text is typically the
/// `marking_label` (e.g., `"CNWDI"`, `"SI compartments"`).
fn e058_diags_for<'a>(diags: &'a [Diagnostic], marker_text: &str) -> Vec<&'a Diagnostic> {
    diags
        .iter()
        .filter(|d| d.rule.as_str() == "E058" && d.message.contains(marker_text))
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
            n.starts_with("class-floor/") || n.starts_with("E058/")
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
    let scheme = CapcoScheme::new();
    for c in scheme.constraints() {
        let n = c.name();
        if !(n.starts_with("class-floor/") || n.starts_with("E058/")) {
            continue;
        }
        let label = c.label();
        let valid =
            label.starts_with("CAPCO-2016 §") || label.starts_with("marque-applied.md §3.7");
        assert!(
            valid,
            "class-floor row {n:?} citation must start with `CAPCO-2016 §` or \
             `marque-applied.md §3.7` (passthrough); got: {label:?}"
        );
    }
}

#[test]
fn capco_rules_set_includes_class_floor_walker() {
    let set = capco_rules();
    let ids: Vec<&str> = set.rules().iter().map(|r| r.id().as_str()).collect();
    assert!(
        ids.contains(&"E058"),
        "rule set must register `DeclarativeClassFloorRule` (E058); registered IDs: {ids:?}"
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
    assert_eq!(hcs_sub[0].citation, "CAPCO-2016 §H.4");
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

// NATO BALK / BOHEMIA: presence predicate fires only when the document's
// NATO classification is exactly `CosmicTopSecretBalk` or
// `CosmicTopSecretBohemia` (already at TS via reciprocal-raise per
// §3.4.1 Note (i)). The floor is satisfied by the presence itself in
// the well-formed case; the row exists to catch data-corruption /
// mangled input where a BALK/BOHEMIA portion is incorrectly carried
// with a sub-CTS NATO classification (which the parser cannot
// construct via the well-formed path because the variant binds them
// together). These rows are exercised by direct attrs construction in
// the unit tests in `crates/capco/src/scheme.rs` — the engine path
// can't construct the divergent state.

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
    assert_eq!(cnwdi[0].citation, "CAPCO-2016 §H.6 p104");
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
    assert_eq!(rd_sigma[0].citation, "CAPCO-2016 §H.6 p113");
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
    assert_eq!(sar[0].citation, "CAPCO-2016 §H.5");
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
    assert_eq!(ucni[0].citation, "CAPCO-2016 §H.6 p116");
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
    assert_eq!(ucni[0].citation, "CAPCO-2016 §H.6 p118");
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
    assert!(
        bur[0]
            .message
            .contains("ISM but not enumerated in CAPCO-2016"),
        "passthrough diagnostic must quote §3.7 policy framing; got: {:?}",
        bur[0].message
    );
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
    let mut config = Config::default();
    config
        .rules
        .overrides
        .insert("E058".to_string(), "off".to_string());
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
    let e058: Vec<&Diagnostic> = diags.iter().filter(|d| d.rule.as_str() == "E058").collect();
    assert!(
        e058.is_empty(),
        "with `[rules] E058 = \"off\"`, no E058 diagnostics may emit (FR-008): {diags:?}"
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
