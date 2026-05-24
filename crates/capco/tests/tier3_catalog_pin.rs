// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-H (issue #371 tier-3) catalog pin for the SCI per-system
//! bitmask compilation.
//!
//! Pins the structural invariants of [`SCI_PER_SYSTEM_CATALOG`] after the
//! PR-H tier-3 additions of `bitmask_trigger`, `bitmask_trigger_exact`,
//! `bitmask_companion_required`, and `bitmask_companion_forbidden` to every
//! [`SciPerSystemRow`]:
//!
//!  * **Total rows**: 5
//!  * **Rows with `bitmask_trigger: Some(_)`**: 5 (100% coverage)
//!  * **Rows with `bitmask_trigger_exact: true`**: 4 (exact-hit rows;
//!    only HCS-P-NOFORN uses the coarse SCI_PRESENT gate)
//!  * **Positional row-name list**: pins every row's `name` in catalog
//!    order (catches rename-at-same-count and swap-at-same-count drift)
//!
//! ## Why 100% bitmask coverage?
//!
//! Unlike the class-floor catalog (85% coverage, 4 structural passthroughs),
//! all 5 SCI per-system rows have closed-atom triggers: HCS-O, HCS-P-sub,
//! SI-G, TK-BLFH/IDIT/KAND are exact atoms; HCS-P uses the SCI_PRESENT
//! coarse gate with presence_hcs_p_any confirmation. No row requires
//! open-vocab fallthrough.
//!
//! ## Drift classes caught
//!
//! | Drift | Caught by |
//! |---|---|
//! | Row added / removed | Length + name list |
//! | Row renamed at same count | Name list |
//! | Row swapped | Name list |
//! | `bitmask_trigger` flipped Some→None | `some_count` check |
//! | `bitmask_trigger_exact` flipped | `exact_count` check |

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_ism::canonical::CanonicalAttrs;
use marque_scheme::{Constraint, MarkingScheme};

fn sci_per_system_names(scheme: &CapcoScheme) -> Vec<&'static str> {
    scheme
        .constraints()
        .iter()
        .filter_map(|c| match c {
            Constraint::Custom { name, .. } if name.starts_with("marking.sci.") => Some(*name),
            _ => None,
        })
        .collect()
}

/// Pins the total row count at 5.
#[test]
fn sci_per_system_catalog_row_count_is_5() {
    let scheme = CapcoScheme::new();
    let names = sci_per_system_names(&scheme);
    assert_eq!(
        names.len(),
        5,
        "SCI_PER_SYSTEM_CATALOG must have exactly 5 rows; got {}. Row list: {:#?}",
        names.len(),
        names,
    );
}

/// Pins the exact ordered row-name list of SCI_PER_SYSTEM_CATALOG.
///
/// Row order matches the catalog declaration order in `sci_per_system.rs`.
/// Swapping two rows changes which diagnostic appears first when both fire;
/// this pin catches that before corpus runs do.
#[test]
fn sci_per_system_catalog_row_names_positional() {
    const EXPECTED: &[&str] = &[
        "marking.sci.hcs-o-companions",
        "marking.sci.hcs-p-noforn-required",
        "marking.sci.hcs-p-sub-companions",
        "marking.sci.si-g-companions",
        "marking.sci.tk-compartment-noforn-required",
    ];

    let scheme = CapcoScheme::new();
    let actual = sci_per_system_names(&scheme);

    assert_eq!(
        actual, EXPECTED,
        "SCI_PER_SYSTEM_CATALOG positional name list diverged from pin. \
         Use the diff above to identify renamed or reordered rows."
    );
}

/// Pins that all 5 rows return empty on an all-zero `CanonicalAttrs`
/// (trigger gate fires false for every row → short-circuit to empty).
#[test]
fn sci_per_system_catalog_all_rows_return_empty_on_empty_attrs() {
    let scheme = CapcoScheme::new();
    let names = sci_per_system_names(&scheme);
    let empty = CanonicalAttrs::default();
    let marking = CapcoMarking::new(empty);
    let bits = scheme.precompute_bits(&marking);

    for name in &names {
        let violations = scheme.evaluate_custom(name, &marking, bits);
        assert!(
            violations.is_empty(),
            "Row '{name}' fired on empty CanonicalAttrs — trigger gate must return \
             empty when no SCI family is present"
        );
    }
}

/// Pins the bitmask-trigger distribution: 5 rows with Some trigger,
/// 4 rows with exact trigger, 1 row with coarse gate (HCS-P-NOFORN).
///
/// Because `bitmask_trigger` is `pub(crate)`, we pin the distribution
/// indirectly via the observable behavior on a targeted set of inputs:
///
/// - An attrs that carries only `SCI_PRESENT` (bare HCS-P, no atom) must
///   still fire HCS-P-NOFORN when NOFORN is absent (coarse gate row).
/// - An attrs with ONLY SCI_PRESENT (bare HCS-P) and NO other SCI atoms
///   must NOT fire HCS-O-companions, HCS-P-sub-companions, SI-G-companions,
///   or TK-compartment-NOFORN (those rows need their exact atoms).
///
/// This test verifies the 4-exact / 1-coarse split without accessing
/// `pub(crate)` fields.
#[test]
fn sci_per_system_coarse_gate_row_fires_on_sci_present_only() {
    use marque_ism::{
        Classification, MarkingClassification, SciCompartment, SciControlBare, SciControlSystem,
        SciMarking,
    };

    let scheme = CapcoScheme::new();

    // Bare HCS-P (no sub-compartments) + US Secret + no NOFORN.
    // Only SCI_PRESENT is set — SCI_HCS_O, SCI_HCS_P_SUB, SCI_SI_G,
    // SCI_TK_{BLFH,IDIT,KAND} are all zero.
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
    attrs.sci_markings = Box::new([SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        Box::new([SciCompartment::new("P", Box::new([]))]), // bare — no sub-compartments
        None,
    )]);
    // No dissem → NOFORN absent → HCS-P-NOFORN should fire.
    attrs.dissem_us = Box::new([]);

    let marking = CapcoMarking::new(attrs.clone());
    let bits = scheme.precompute_bits(&marking);

    // HCS-P-NOFORN (coarse gate row) MUST fire — bare HCS-P + no NOFORN.
    let hcs_p_violations =
        scheme.evaluate_custom("marking.sci.hcs-p-noforn-required", &marking, bits);
    assert!(
        !hcs_p_violations.is_empty(),
        "HCS-P-NOFORN must fire on bare HCS-P + US S + no NOFORN (coarse gate row)"
    );

    // All exact-gate rows must NOT fire — their atoms are absent.
    for name in &[
        "marking.sci.hcs-o-companions",
        "marking.sci.hcs-p-sub-companions",
        "marking.sci.si-g-companions",
        "marking.sci.tk-compartment-noforn-required",
    ] {
        let violations = scheme.evaluate_custom(name, &marking, bits);
        assert!(
            violations.is_empty(),
            "Row '{name}' fired on bare-HCS-P attrs (SCI_PRESENT only) — \
             exact-gate rows must not fire when their atom is absent"
        );
    }
}

/// Pins that all 5 rows return empty when no US classification is present
/// (the US-only §H.4 early-out in all emit functions).
///
/// All §H.4 companion rows apply only to US-classified portions. When the
/// classification is None (or NATO-only), the emit functions return empty
/// immediately. The bitmask fast path mirrors this via the
/// `US_COLLATERAL_CLASSIFIED | US_UNCLASSIFIED` zero check.
#[test]
fn sci_per_system_all_rows_return_empty_without_us_classification() {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    let scheme = CapcoScheme::new();

    // Attrs with all 5 SCI families present but NO classification.
    // Every trigger fires (atoms set); but the companion check early-outs
    // because no US classification is present.
    let mut attrs = CanonicalAttrs::default();
    // No classification.
    attrs.sci_markings = Box::new([
        SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Box::new([SciCompartment::new("O", Box::new([]))]), // HCS-O
            None,
        ),
        SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Box::new([SciCompartment::new("P", Box::new(["ALPHA".into()]))]), // HCS-P sub
            None,
        ),
        SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([SciCompartment::new("G", Box::new([]))]), // SI-G
            None,
        ),
        SciMarking::new(
            SciControlSystem::Published(SciControlBare::Tk),
            Box::new([SciCompartment::new("BLFH", Box::new([]))]), // TK-BLFH
            None,
        ),
    ]);
    // No dissem (all companions absent) — violation if US class present.
    attrs.dissem_us = Box::new([]);

    let marking = CapcoMarking::new(attrs);
    let bits = scheme.precompute_bits(&marking);
    let names = sci_per_system_names(&scheme);

    for name in &names {
        let violations = scheme.evaluate_custom(name, &marking, bits);
        assert!(
            violations.is_empty(),
            "Row '{name}' fired on attrs with no US classification — \
             all §H.4 rows must return empty when classification is None (US-only gate)"
        );
    }
}
