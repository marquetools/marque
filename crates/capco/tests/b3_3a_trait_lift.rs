// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! B3.3a — canonical-space + `ConstraintBridge` trait-lift coverage.
//!
//! B3.3a exposes the engine's CAPCO-inherent canonical-space operations
//! (`join_via_lattice`, `project_from_attrs_slice`, the
//! `From<CanonicalAttrs>` conversion) and the constraint-bridge methods
//! through trait surfaces so B3.3b can drive them generically. The engine
//! is untouched in B3.3a (it still calls the inherent methods), so the
//! contract here is twofold:
//!
//! 1. **Delegation identity.** Each new `CapcoScheme` trait method must
//!    produce exactly what the inherent method it delegates to produces.
//!    This guards against future drift in the one-line delegation bodies.
//! 2. **Generic reachability.** The bounds B3.3b will use
//!    (`S: MarkingScheme` for canonical ops, `S: ConstraintBridge` for the
//!    bridge) must be callable for *both* `CapcoScheme` and the second
//!    scheme (`StubScheme`) — otherwise a generic `Engine<S>` could not
//!    instantiate them.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_core::Parser;
use marque_ism::CanonicalAttrs;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_rules::ConstraintBridge;
use marque_scheme::{MarkingScheme, Scope, Span};
use marque_test_utils::stub_scheme::StubScheme;

/// Parse a portion string into `CanonicalAttrs` via the strict parser —
/// the same path the engine's per-candidate dispatch uses, without
/// pulling in `marque-engine`.
fn parse_portion(scheme: &CapcoScheme, text: &str) -> CanonicalAttrs {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, text.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&candidate, text.as_bytes())
        .expect("valid portion must parse");
    scheme.canonicalize(parsed.attrs)
}

fn sample_portions(scheme: &CapcoScheme) -> Vec<CanonicalAttrs> {
    vec![
        parse_portion(scheme, "(S//NOFORN)"),
        parse_portion(scheme, "(TS//SI-G//NOFORN)"),
        parse_portion(scheme, "(C//REL TO USA, GBR)"),
    ]
}

// -------------------------------------------------------------------------
// 1. Delegation identity — CapcoScheme MarkingScheme canonical-space ops.
// -------------------------------------------------------------------------

#[test]
fn canonical_page_join_matches_inherent_join_via_lattice() {
    let scheme = CapcoScheme::new();
    let portions = sample_portions(&scheme);
    for n in 0..=portions.len() {
        let slice = &portions[..n];
        assert_eq!(
            MarkingScheme::canonical_page_join(&scheme, slice),
            CapcoMarking::join_via_lattice(slice),
            "canonical_page_join must equal join_via_lattice for {n} portion(s)",
        );
    }
}

#[test]
fn project_canonical_matches_inherent_from_attrs_slice() {
    let scheme = CapcoScheme::new();
    for attrs in sample_portions(&scheme) {
        let slice = std::slice::from_ref(&attrs);
        let via_trait = MarkingScheme::project_canonical(&scheme, slice);
        let via_inherent =
            marque_ism::ProjectedMarking::from_canonical(scheme.project_from_attrs_slice(slice));
        assert_eq!(via_trait, via_inherent);
    }
}

#[test]
fn marking_from_canonical_matches_inherent_from() {
    let scheme = CapcoScheme::new();
    for attrs in sample_portions(&scheme) {
        let via_trait = MarkingScheme::marking_from_canonical(&scheme, attrs.clone());
        let via_inherent = CapcoMarking::from(attrs);
        // `CapcoMarking: PartialEq` compares the parsed attrs only
        // (decoder provenance is excluded from `Eq`); both sides here are
        // `CapcoMarking(attrs, None)`, so the comparison is exact.
        assert_eq!(via_trait, via_inherent);
    }
}

// -------------------------------------------------------------------------
// 2. Delegation identity — CapcoScheme ConstraintBridge methods.
// -------------------------------------------------------------------------

#[test]
fn constraint_bridge_has_diagnostic_constraints_matches_inherent() {
    let scheme = CapcoScheme::new();
    assert_eq!(
        ConstraintBridge::has_diagnostic_constraints(&scheme),
        scheme.has_diagnostic_constraints(),
    );
    // CAPCO declares diagnostic constraints; the trait must surface that.
    assert!(ConstraintBridge::has_diagnostic_constraints(&scheme));
}

#[test]
fn constraint_bridge_fix_intent_by_name_matches_inherent() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(TS//RD//NOFORN)");
    // A real AEA constraint label and a label that yields `None`, so we
    // exercise both arms.
    for label in ["portion.aea.rd-frd-requires-noforn", "no.such.label"] {
        let via_trait =
            ConstraintBridge::fix_intent_by_name(&scheme, label, &attrs, MarkingType::Portion);
        let via_inherent = scheme.fix_intent_by_name(label, &attrs, MarkingType::Portion);
        // `FixIntent` derives `Debug` but not `PartialEq`; compare the
        // `is_some()` discriminant plus the debug projection.
        assert_eq!(via_trait.is_some(), via_inherent.is_some());
        assert_eq!(format!("{via_trait:?}"), format!("{via_inherent:?}"));
    }
}

#[test]
fn constraint_bridge_message_by_name_matches_inherent() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(TS//RD//NOFORN)");
    for label in ["portion.aea.rd-frd-requires-noforn", "no.such.label"] {
        let via_trait =
            ConstraintBridge::message_by_name(&scheme, label, &attrs, MarkingType::Portion);
        let via_inherent = scheme.message_by_name(label, &attrs, MarkingType::Portion);
        assert_eq!(via_trait, via_inherent);
    }
}

#[test]
fn constraint_bridge_sci_per_system_matches_inherent() {
    let scheme = CapcoScheme::new();
    let overrides = std::collections::HashMap::new();
    // SCI-bearing portion so the per-system catalog walk has input.
    let attrs = parse_portion(&scheme, "(TS//SI-G//NOFORN)");
    let span = Span::new(0, 16);
    let via_trait = ConstraintBridge::bridge_sci_per_system_diagnostics(
        &scheme,
        &attrs,
        span,
        Scope::Portion,
        &overrides,
    );
    let via_inherent =
        scheme.bridge_sci_per_system_diagnostics(&attrs, span, Scope::Portion, &overrides);
    // `Diagnostic` does not derive `PartialEq`; compare count + the
    // ordered rule-id projection (a stable, content-ignorant signature).
    assert_eq!(via_trait.len(), via_inherent.len());
    let ids = |ds: &[marque_rules::Diagnostic<CapcoScheme>]| {
        ds.iter().map(|d| d.rule.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(ids(&via_trait), ids(&via_inherent));
}

/// Sink-aware projection delegation-identity. Gated, mirroring the
/// `#[cfg(feature = "decision-tracing")]` gate on the override itself and
/// the existing `project_with_sink` unit test — without the feature the
/// override (and this test) are not compiled. A `NoopSink` is sufficient:
/// the projected *result* must match the non-sink inherent path
/// regardless of observer.
#[cfg(feature = "decision-tracing")]
#[test]
fn project_canonical_with_sink_matches_inherent() {
    use marque_scheme::NoopSink;
    let scheme = CapcoScheme::new();
    for attrs in sample_portions(&scheme) {
        let slice = std::slice::from_ref(&attrs);
        let mut sink = NoopSink;
        let via_trait = MarkingScheme::project_canonical_with_sink(&scheme, slice, &mut sink);
        let mut sink2 = NoopSink;
        let via_inherent = marque_ism::ProjectedMarking::from_canonical(
            scheme.project_from_attrs_slice_with_sink(slice, &mut sink2),
        );
        assert_eq!(via_trait, via_inherent);
        // And the sink-aware projection must equal the non-sink one.
        assert_eq!(via_trait, MarkingScheme::project_canonical(&scheme, slice));
    }
}

// -------------------------------------------------------------------------
// 3. Generic reachability — the bounds B3.3b will use, on both schemes.
// -------------------------------------------------------------------------

/// Exercises `canonical_page_join` through the bare `MarkingScheme` bound
/// (the only canonical-space op with a non-`unimplemented!()` default, so
/// the only one callable on a scheme that does not canonicalize).
fn join_through_bound<S: MarkingScheme>(scheme: &S, portions: &[S::Canonical]) -> S::Canonical {
    scheme.canonical_page_join(portions)
}

/// Exercises every `ConstraintBridge` method through the generic bound —
/// this is the exact bound B3.3b's pipeline will require.
fn bridge_through_bound<S: ConstraintBridge>(scheme: &S, canonical: &S::Canonical) -> bool {
    let overrides = std::collections::HashMap::new();
    let _ = scheme.fix_intent_by_name("probe", canonical, MarkingType::Portion);
    let _ = scheme.message_by_name("probe", canonical, MarkingType::Portion);
    let _ = scheme.bridge_sci_per_system_diagnostics(
        canonical,
        Span::new(0, 1),
        Scope::Portion,
        &overrides,
    );
    scheme.has_diagnostic_constraints()
}

#[test]
fn generic_bounds_callable_for_capco_scheme() {
    let scheme = CapcoScheme::new();
    let portions = sample_portions(&scheme);
    let joined = join_through_bound(&scheme, &portions);
    assert_eq!(joined, CapcoMarking::join_via_lattice(&portions));
    assert!(bridge_through_bound(&scheme, &portions[0]));
}

#[test]
fn generic_bounds_callable_for_stub_scheme() {
    let scheme = StubScheme::new();
    // StubScheme: Canonical = (). Calling through the bare `MarkingScheme`
    // bound is what proves reachability; the page-join default folds to the
    // last element (or bottom for an empty page), and the return is unit, so
    // there is nothing meaningful to assert on the value itself.
    join_through_bound(&scheme, &[(), (), ()]);
    join_through_bound(&scheme, &[]);
    // StubScheme inherits every ConstraintBridge no-op default.
    assert!(!bridge_through_bound(&scheme, &()));
}

// -------------------------------------------------------------------------
// 4. StubScheme inherits the no-op ConstraintBridge defaults.
// -------------------------------------------------------------------------

#[test]
fn stub_scheme_inherits_constraint_bridge_defaults() {
    let scheme = StubScheme::new();
    let overrides = std::collections::HashMap::new();
    assert!(!ConstraintBridge::has_diagnostic_constraints(&scheme));
    assert!(
        ConstraintBridge::fix_intent_by_name(&scheme, "x", &(), MarkingType::Portion).is_none()
    );
    assert!(ConstraintBridge::message_by_name(&scheme, "x", &(), MarkingType::Portion).is_none());
    assert!(
        ConstraintBridge::bridge_sci_per_system_diagnostics(
            &scheme,
            &(),
            Span::new(0, 1),
            Scope::Portion,
            &overrides,
        )
        .is_empty()
    );
}
