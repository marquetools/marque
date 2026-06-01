// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! B3.3a.2 — recognition-accessor trait-lift coverage.
//!
//! B3.3a landed the canonical-space methods (`canonical_page_join`,
//! `project_canonical`, `marking_from_canonical`) and the constraint
//! bridge. B3.3a.2 closes the recognition/reparse boundary the engine
//! pipeline reaches across in B3.3b, lifting three more CAPCO-inherent
//! reads onto trait surface:
//!
//! - [`MarkingScheme::canonical_from_marking`] — the inverse of
//!   `marking_from_canonical`; `recognize_marking_candidate` and the fix
//!   reparse cache cross from marking space back into canonical space here
//!   (`CapcoMarking.0`).
//! - [`MarkingScheme::canonical_rank`] — the page rank-floor read
//!   (CAPCO's effective classification level).
//! - [`ConstraintBridge::recognition_outcome`] — the decoder
//!   provenance side-channel (`CapcoMarking.1`): decoder-path flag,
//!   posterior, and the synthetic `R001` diagnostic built by the
//!   relocated [`build_decoder_diagnostic`].
//!
//! The engine is untouched in B3.3a.2 (it still calls the relocated
//! `build_decoder_diagnostic` directly), so the contract here is the
//! same two-fold one as b3_3a:
//!
//! 1. **Delegation identity.** Each new `CapcoScheme` method produces
//!    exactly what the read it replaces produces.
//! 2. **Generic reachability.** The bounds B3.3b will use
//!    (`S: MarkingScheme` for `canonical_*`, `S: ConstraintBridge` for
//!    `recognition_outcome`) are callable for both `CapcoScheme` and the
//!    second scheme (`StubScheme`).

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_capco::{DecoderProvenance, build_decoder_diagnostic};
use marque_core::Parser;
use marque_ism::CanonicalAttrs;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_rules::{ConstraintBridge, FeatureContribution, FeatureId, FixSource};
use marque_scheme::{MarkingScheme, Span};
use marque_test_utils::stub_scheme::{StubMarking, StubScheme};

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

/// A decoder provenance trace whose canonical bytes are `canonical`.
/// `runner_up_ratio = Some(3.0)` gives a deterministic, sub-1.0
/// recognition score (`0.75`); the feature list is empty.
fn sample_provenance(canonical: &[u8]) -> DecoderProvenance {
    let features: Box<[FeatureContribution]> = Box::default();
    DecoderProvenance::new(
        Box::from(canonical),
        -0.5,
        Some(3.0),
        features,
        FixSource::DecoderPosterior,
    )
}

// -------------------------------------------------------------------------
// 1. canonical_from_marking — delegation identity (CapcoMarking.0).
// -------------------------------------------------------------------------

#[test]
fn canonical_from_marking_matches_tuple_field() {
    let scheme = CapcoScheme::new();
    for txt in ["(S//NOFORN)", "(TS//SI-G//NOFORN)", "(C//REL TO USA, GBR)"] {
        let attrs = parse_portion(&scheme, txt);
        let marking = CapcoMarking::new(attrs.clone());
        assert_eq!(
            MarkingScheme::canonical_from_marking(&scheme, &marking),
            attrs,
            "canonical_from_marking must equal CapcoMarking.0 for {txt}",
        );
        // Round-trips with the B3.3a `marking_from_canonical` lift.
        let lifted = MarkingScheme::marking_from_canonical(&scheme, attrs.clone());
        assert_eq!(
            MarkingScheme::canonical_from_marking(&scheme, &lifted),
            attrs,
            "marking_from_canonical → canonical_from_marking must round-trip for {txt}",
        );
    }
}

#[test]
fn canonical_from_marking_drops_provenance() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//NOFORN)");
    // A decoder-path marking projects to the same canonical as a
    // strict-path one: provenance is recognition metadata, not state.
    let marking = CapcoMarking(attrs.clone(), Some(sample_provenance(b"(S//NOFORN)")));
    assert_eq!(
        MarkingScheme::canonical_from_marking(&scheme, &marking),
        attrs
    );
}

// -------------------------------------------------------------------------
// 2. canonical_rank — delegation identity (effective classification level).
// -------------------------------------------------------------------------

#[test]
fn canonical_rank_matches_effective_level() {
    let scheme = CapcoScheme::new();
    for txt in ["(S//NOFORN)", "(TS//SI-G//NOFORN)", "(C//REL TO USA, GBR)"] {
        let attrs = parse_portion(&scheme, txt);
        let via_trait = MarkingScheme::canonical_rank(&scheme, &attrs);
        let via_inherent = attrs
            .classification
            .as_ref()
            .map(|c| c.effective_level() as u8);
        assert_eq!(via_trait, via_inherent, "rank mismatch for {txt}");
        assert!(via_trait.is_some(), "classified portion must carry a rank");
    }
}

#[test]
fn canonical_rank_is_none_without_classification() {
    let scheme = CapcoScheme::new();
    // The lattice bottom carries no classification.
    assert_eq!(
        MarkingScheme::canonical_rank(&scheme, &CanonicalAttrs::default()),
        None
    );
}

// -------------------------------------------------------------------------
// 3. recognition_outcome — delegation identity (CapcoMarking.1).
// -------------------------------------------------------------------------

#[test]
fn recognition_outcome_strict_path_is_inert() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//NOFORN)");
    // Strict path: provenance is None.
    let marking = CapcoMarking::new(attrs);
    let outcome = ConstraintBridge::recognition_outcome(
        &scheme,
        &marking,
        Span::new(0, 11),
        b"(S//NOFORN)",
        MarkingType::Portion,
        false,
    );
    assert!(!outcome.is_decoder_path);
    assert!(outcome.recognition_score.is_none());
    assert!(outcome.diagnostic.is_none());
}

#[test]
fn recognition_outcome_decoder_path_matches_build_decoder_diagnostic() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//NOFORN)");
    // Canonical bytes differ from the original, so build_decoder_diagnostic
    // emits an R001 (the no-op-rewrite filter does not trip).
    let original: &[u8] = b"(SERCET//NF)";
    let prov = sample_provenance(b"(SECRET//NF)");
    let marking = CapcoMarking(attrs, Some(prov.clone()));
    let span = Span::new(0, original.len());

    let outcome = ConstraintBridge::recognition_outcome(
        &scheme,
        &marking,
        span,
        original,
        MarkingType::Portion,
        false,
    );
    assert!(outcome.is_decoder_path);
    assert_eq!(outcome.recognition_score, Some(prov.recognition_score()));

    // Delegation identity: the bridge's diagnostic IS what the relocated
    // free function produces for the same inputs. `Diagnostic` derives
    // neither `PartialEq` nor `Debug`-compare, so check the observable
    // projection (presence + rule id + severity + span), as b3_3a does.
    let direct = build_decoder_diagnostic(span, original, &prov, MarkingType::Portion, false);
    assert_eq!(outcome.diagnostic.is_some(), direct.is_some());
    let via_trait = outcome.diagnostic.expect("decoder rewrite must emit R001");
    let via_direct = direct.expect("decoder rewrite must emit R001");
    assert_eq!(via_trait.rule, via_direct.rule);
    assert_eq!(via_trait.severity, via_direct.severity);
    assert_eq!(via_trait.span, via_direct.span);
    assert_eq!(
        via_trait.rule.to_string(),
        "engine:recognition.decoder-recognized"
    );
}

#[test]
fn recognition_outcome_threads_corpus_override_into_diagnostic_features() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//NOFORN)");
    // Canonical bytes differ from the original ⇒ an R001 is emitted, so the
    // `corpus_override_active` flag has a diagnostic to ride along on.
    let original: &[u8] = b"(SERCET//NF)";
    let prov = sample_provenance(b"(SECRET//NF)");
    let marking = CapcoMarking(attrs, Some(prov));
    let span = Span::new(0, original.len());

    let has_override_feature = |corpus_override_active: bool| {
        let outcome = ConstraintBridge::recognition_outcome(
            &scheme,
            &marking,
            span,
            original,
            MarkingType::Portion,
            corpus_override_active,
        );
        let diag = outcome
            .diagnostic
            .expect("decoder rewrite must emit R001 regardless of override flag");
        diag.fix
            .as_ref()
            .expect("R001 carries a recanonicalize FixIntent")
            .confidence
            .features
            .iter()
            .any(|f| f.id == FeatureId::CorpusOverrideInEffect)
    };

    assert!(
        has_override_feature(true),
        "corpus_override_active = true must record FeatureId::CorpusOverrideInEffect"
    );
    assert!(
        !has_override_feature(false),
        "corpus_override_active = false must not record the override feature"
    );
}

#[test]
fn recognition_outcome_decoder_noop_rewrite_emits_no_diagnostic() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//NOFORN)");
    // Canonical bytes equal the original ⇒ the no-op-rewrite filter in
    // build_decoder_diagnostic returns None, but the recognition is still
    // decoder-path and carries a score.
    let bytes: &[u8] = b"(S//NOFORN)";
    let prov = sample_provenance(bytes);
    let marking = CapcoMarking(attrs, Some(prov.clone()));
    let outcome = ConstraintBridge::recognition_outcome(
        &scheme,
        &marking,
        Span::new(0, bytes.len()),
        bytes,
        MarkingType::Portion,
        false,
    );
    assert!(outcome.is_decoder_path);
    assert_eq!(outcome.recognition_score, Some(prov.recognition_score()));
    assert!(
        outcome.diagnostic.is_none(),
        "no-op rewrite must not synthesize an R001"
    );
}

// -------------------------------------------------------------------------
// 4. Generic reachability — the bounds B3.3b will use, on both schemes.
// -------------------------------------------------------------------------

/// `canonical_rank` through the bare `MarkingScheme` bound (default
/// `None`, so callable on a scheme that does not rank canonicals).
fn rank_through_bound<S: MarkingScheme>(scheme: &S, canonical: &S::Canonical) -> Option<u8> {
    scheme.canonical_rank(canonical)
}

/// `recognition_outcome` through the `ConstraintBridge` bound — the exact
/// bound B3.3b's recognition path requires.
fn decoder_path_through_bound<S: ConstraintBridge>(scheme: &S, marking: &S::Marking) -> bool {
    scheme
        .recognition_outcome(marking, Span::new(0, 1), b"", MarkingType::Portion, false)
        .is_decoder_path
}

#[test]
fn generic_bounds_callable_for_capco_scheme() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//NOFORN)");
    assert!(rank_through_bound(&scheme, &attrs).is_some());
    // A strict-path marking is not decoder-path.
    assert!(!decoder_path_through_bound(
        &scheme,
        &CapcoMarking::new(attrs)
    ));
}

#[test]
fn generic_bounds_callable_for_stub_scheme() {
    let scheme = StubScheme::new();
    // StubScheme: Canonical = (), no rank axis ⇒ default None.
    assert_eq!(rank_through_bound(&scheme, &()), None);
    // StubScheme inherits the inert recognition_outcome default.
    assert!(!decoder_path_through_bound(
        &scheme,
        &StubMarking::default()
    ));
}

/// `canonical_from_marking` has an `unimplemented!()` default (it cannot
/// be synthesized generically), so reachability is proven on CapcoScheme
/// — the same treatment b3_3a gives `marking_from_canonical`.
fn canonical_through_bound<S: MarkingScheme>(scheme: &S, marking: &S::Marking) -> S::Canonical {
    scheme.canonical_from_marking(marking)
}

#[test]
fn canonical_from_marking_callable_through_bound_for_capco() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//NOFORN)");
    let got = canonical_through_bound(&scheme, &CapcoMarking::new(attrs.clone()));
    assert_eq!(got, attrs);
}

// -------------------------------------------------------------------------
// 5. StubScheme inherits the inert recognition_outcome default.
// -------------------------------------------------------------------------

#[test]
fn stub_scheme_recognition_outcome_default_is_inert() {
    let scheme = StubScheme::new();
    let outcome = ConstraintBridge::recognition_outcome(
        &scheme,
        &StubMarking::default(),
        Span::new(0, 1),
        b"x",
        MarkingType::Portion,
        false,
    );
    assert!(!outcome.is_decoder_path);
    assert!(outcome.recognition_score.is_none());
    assert!(outcome.diagnostic.is_none());
}
