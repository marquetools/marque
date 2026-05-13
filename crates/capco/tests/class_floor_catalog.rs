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

/// Filter the diagnostic stream to the E058 emissions whose message
/// contains `marker_text` (substring match). The catalog uses one rule
/// ID `E058` for all 27 rows; per-row identification flows via the
/// diagnostic message text. The marker_text is typically the
/// `marking_label` (e.g., `"CNWDI"`, `"SI compartments"`).
fn e058_diags_for<'a>(
    diags: &'a [Diagnostic<CapcoScheme>],
    marker_text: &str,
) -> Vec<&'a Diagnostic<CapcoScheme>> {
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
fn class_floor_diagnostics_flow_through_engine_bridge_at_e058() {
    // PR 3c.B Commit 7.3: `DeclarativeClassFloorRule` retired from
    // `CapcoRuleSet`. The 27 catalog rows still emit diagnostics —
    // they flow through the engine's constraint-catalog bridge with
    // `Diagnostic.rule = "E058"` (the bridge folds catalog row names
    // `E058/...` and `class-floor/...` to the walker-level ID per
    // engine.rs comment). This test pins the post-deletion external
    // surface:
    //   1. No registered `Rule::id() == "E058"` (walker is gone);
    //   2. `engine.lint` still produces an E058-tagged diagnostic on
    //      a known-firing fixture (CONFIDENTIAL//RD-CNWDI → CNWDI
    //      floor row, §H.6 p104 §2.2 family).
    let set = capco_rules();
    let ids: Vec<&str> = set.rules().iter().map(|r| r.id().as_str()).collect();
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
    let e058: Vec<&Diagnostic<CapcoScheme>> =
        diags.iter().filter(|d| d.rule.as_str() == "E058").collect();
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
    // `//COSMIC TOP SECRET-BALK` parses to `Nato(CosmicTopSecretBalk)`,
    // effective level TS. BALK floor (TS) is satisfied → no diagnostic.
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
    let violations = validate_and_filter(&scheme, &marking, "class-floor/passthrough-BUR");
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
    let violations = validate_and_filter(&scheme, &marking, "class-floor/passthrough-BUR");
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
    let violations = validate_and_filter(&scheme, &marking, "E058/DOD-UCNI-classification-ceiling");
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
    // R3.2: every catalog row's `name` MUST start with one of the two
    // prefixes (`E058/` or `class-floor/`). The `is_class_floor_catalog_name`
    // dispatch in `evaluate_custom_by_attrs` is an O(1) prefix check
    // that depends on this invariant. Adding a row whose name doesn't
    // follow the convention would break the dispatch routing for that
    // row silently — this test fails the build instead.
    let scheme = CapcoScheme::new();
    let class_floor_rows: Vec<&str> = scheme
        .constraints()
        .iter()
        .filter(|c| {
            let n = c.name();
            n.starts_with("E058/") || n.starts_with("class-floor/")
        })
        .map(|c| c.name())
        .collect();
    assert_eq!(
        class_floor_rows.len(),
        27,
        "expected 27 class-floor catalog rows under the E058/ + class-floor/ prefix \
         convention; got {}: {:?}",
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
        if n.starts_with("E058/") || n.starts_with("class-floor/") {
            assert!(
                n.starts_with("E058/") || n.starts_with("class-floor/"),
                "class-floor catalog row name {n:?} violates naming-prefix invariant"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// §2.1 row #4 — class-floor/BALK: fires-below + absent
// ---------------------------------------------------------------------------
//
// BALK presence is bound to NatoClassification::CosmicTopSecretBalk
// (the parser binds class+marking together), so "BALK present at
// sub-CTS class" is a constructed-attrs case — only reachable via
// the trait/validate path. The well-formed engine-path silence is
// covered by `balk_does_not_fire_on_well_formed_cts_balk_banner`
// above.

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
    let diags = lint("SECRET//SI-G//ORCON/NOFORN\n");
    let hcs = e058_diags_for(&diags, "HCS-O / HCS-P");
    assert!(
        hcs.is_empty(),
        "HCS-comp floor must not fire when no HCS marking present: {diags:?}"
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
    assert_eq!(tk[0].citation, "CAPCO-2016 §H.4");
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
    assert_eq!(frd_sg[0].citation, "CAPCO-2016 §H.6 p113");
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
// §2.2 row #11 — E058/CNWDI-classification-floor: absent
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
    let diags = lint("CONFIDENTIAL//TK//RSEN\n");
    let rsen = e058_diags_for(&diags, "RSEN");
    assert_eq!(
        rsen.len(),
        1,
        "RSEN floor must fire on CONFIDENTIAL: {diags:?}"
    );
    assert_eq!(rsen[0].severity, Severity::Error);
    assert_eq!(rsen[0].citation, "CAPCO-2016 §H.8 p149");
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
    assert_eq!(imcon[0].citation, "CAPCO-2016 §H.8 p144");
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
    assert_eq!(si[0].citation, "CAPCO-2016 §H.4");
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
    let diags = lint("CONFIDENTIAL//TK//NOFORN\n");
    let si = e058_diags_for(&diags, "SI (bare)");
    assert!(
        si.is_empty(),
        "SI bare floor must not fire when no bare SI marking present: {diags:?}"
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
    assert_eq!(rd[0].citation, "CAPCO-2016 §H.6 p104");
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
    assert_eq!(frd[0].citation, "CAPCO-2016 §H.6 p104");
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
    assert_eq!(tfni[0].citation, "CAPCO-2016 §H.6 p107");
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
    assert_eq!(orcon[0].citation, "CAPCO-2016 §H.8 p136");
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
// `marque-ism::DissemControl::Eyes`). The banner-form
// `USA/[LIST] EYES ONLY` syntax requires lexer support for
// trigraph-EYES coupling that doesn't yet exist; portion form
// `(U//EYES)` / `(C//EYES)` is what the parser recognizes via the
// CVE projection. The fires-below + at-floor + absent triplet uses
// portion form so the engine path actually exercises the row.

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
    assert_eq!(eyes[0].citation, "CAPCO-2016 §H.8 p152");
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
    assert!(
        hcsx[0]
            .message
            .contains("ISM but not enumerated in CAPCO-2016"),
        "passthrough diagnostic must quote §3.7 policy framing; got: {:?}",
        hcsx[0].message
    );
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
    let diags = lint("UNCLASSIFIED//SI\n");
    let hcsx = e058_diags_for(&diags, "HCS-X");
    assert!(
        hcsx.is_empty(),
        "HCS-X passthrough must not fire when no HCS-X marking present: {diags:?}"
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
    let diags = lint("UNCLASSIFIED//SI\n");
    let klm = e058_diags_for(&diags, "KLM family");
    assert!(
        klm.is_empty(),
        "KLM passthrough must not fire when no KLM marking present: {diags:?}"
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
    let diags = lint("UNCLASSIFIED//SI\n");
    let mvl = e058_diags_for(&diags, "MVL");
    assert!(
        mvl.is_empty(),
        "MVL passthrough must not fire when no MVL marking present: {diags:?}"
    );
}
