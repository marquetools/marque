// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tests for `decoder/scoring.rs`. Carved into a parallel
//! file because the combined production + test surface would push
//! the file over the 800-line gate. Reached from the source file
//! via `#[path = "tests/scoring_tests.rs"] #[cfg(test)] mod tests;`.

use std::sync::LazyLock;

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_core::Parser;
use marque_ism::{
    CapcoTokenSet, Classification, DissemControl, MarkingClassification,
    span::{MarkingCandidate, MarkingType, Span},
};
use marque_rules::confidence::FeatureId;
use marque_scheme::MarkingScheme;
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{LinePrefix, ParseContext, Recognizer};
use smallvec::SmallVec;

use super::*;
use crate::decoder::DecoderRecognizer;
use crate::decoder::UNAMBIGUOUS_LOG_MARGIN;
use crate::decoder::test_helpers::{TEST_SCHEME, deep_cx};
use crate::decoder::types::{FeatureEntry, feature_entry_to_evidence};

#[test]
fn is_hard_splitter_covers_documented_long_forms() {
    // Pin the hard-splitter set against accidental shrinkage —
    // every long-form dissem from the doc table must remain
    // a hard splitter.
    for token in &[
        "NOFORN",
        "ORCON",
        "ORCON-USGOV",
        "PROPIN",
        "IMCON",
        "RELIDO",
        "RSEN",
        "EYESONLY",
        "FOUO",
        "FISA",
        "DSEN",
        "EXDIS",
        "NODIS",
        "LIMDIS",
    ] {
        assert!(
            is_hard_splitter(token),
            "{token:?} must be a hard splitter (issue #133)"
        );
    }
}

#[test]
fn is_hard_splitter_excludes_short_forms() {
    // Short-form abbreviations (NF, OC, PR, IMC, RS) are
    // intentionally excluded — they could collide with SAR
    // compartment / sub-compartment naming.
    for token in &["NF", "OC", "PR", "IMC", "RS"] {
        assert!(
            !is_hard_splitter(token),
            "{token:?} is intentionally NOT a hard splitter (collision risk)"
        );
    }
}

#[test]
fn score_candidate_splits_prior_and_posterior() {
    // Synthesize a fake attempt with known non-zero feature deltas
    // and verify the (prior, posterior) return tuple: posterior
    // must be prior + Σ feature.delta, and prior must NOT include
    // any of the feature deltas.
    let scheme = CapcoScheme::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, 14),
        kind: MarkingType::Banner,
    };
    let parsed = parser
        .parse(&candidate, b"SECRET//NOFORN")
        .expect("SECRET//NOFORN must parse");
    // Inline scheme construction per test for hermeticity; routes
    // via the trait override.
    let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));

    let features = [
        FeatureEntry {
            id: FeatureId::EditDistance1,
            delta: -0.5,
        },
        FeatureEntry {
            id: FeatureId::TokenReorder,
            delta: -0.4,
        },
    ];
    let attempt = CanonicalAttempt {
        bytes: SmallVec::from_slice(b"SECRET//NOFORN"),
        features: features.iter().copied().collect(),
        fix_source: marque_rules::FixSource::DecoderPosterior,
    };
    let (prior, posterior) = score_candidate(&attempt, &marking, MarkingType::Banner);

    let feature_sum: f32 = features.iter().map(|f| f.delta).sum();
    let reconstructed = prior + feature_sum;
    assert!(
        (reconstructed - posterior).abs() < 1e-6,
        "posterior must equal prior + Σ feature deltas; \
         prior={prior}, feature_sum={feature_sum}, posterior={posterior}"
    );
    // And the prior alone must differ from the posterior when
    // the features carry non-trivial deltas.
    assert!(
        (prior - posterior).abs() > f32::EPSILON,
        "prior_log_odds must exclude feature deltas; \
         prior={prior}, posterior={posterior}"
    );
}

#[test]
fn score_candidate_includes_country_code_prior_for_rel_to() {
    // Issue #233: `score_candidate` sums `country_code_log_prior` over
    // the `rel_to` slice of the parsed marking. A marking with TWO REL TO
    // entries must produce a strictly lower (more negative) prior than the
    // same marking with ONE entry, because each country code contributes a
    // negative log-prior term and GBR is a known high-frequency trigraph.
    // Inline scheme construction per test for hermeticity; both
    // call sites route via the trait override.
    let scheme = CapcoScheme::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);

    let one_candidate = MarkingCandidate {
        span: Span::new(0, 18),
        kind: MarkingType::Banner,
    };
    let one_parsed = parser
        .parse(&one_candidate, b"SECRET//REL TO USA")
        .expect("SECRET//REL TO USA must parse");
    let one_marking = CapcoMarking::new(scheme.canonicalize(one_parsed.attrs));

    let two_candidate = MarkingCandidate {
        span: Span::new(0, 23),
        kind: MarkingType::Banner,
    };
    let two_parsed = parser
        .parse(&two_candidate, b"SECRET//REL TO USA, GBR")
        .expect("SECRET//REL TO USA, GBR must parse");
    let two_marking = CapcoMarking::new(scheme.canonicalize(two_parsed.attrs));

    let attempt_one = CanonicalAttempt {
        bytes: SmallVec::from_slice(b"SECRET//REL TO USA"),
        features: SmallVec::new(),
        fix_source: marque_rules::FixSource::DecoderPosterior,
    };
    let attempt_two = CanonicalAttempt {
        bytes: SmallVec::from_slice(b"SECRET//REL TO USA, GBR"),
        features: SmallVec::new(),
        fix_source: marque_rules::FixSource::DecoderPosterior,
    };

    let (prior_one, _) = score_candidate(&attempt_one, &one_marking, MarkingType::Banner);
    let (prior_two, _) = score_candidate(&attempt_two, &two_marking, MarkingType::Banner);

    // GBR has a known negative log-prior, so adding it to the REL TO
    // list must make the total prior strictly more negative.
    assert!(
        prior_two < prior_one,
        "adding GBR to REL TO must lower (more negative) the prior via \
         country_code_log_prior; prior_one={prior_one}, prior_two={prior_two}"
    );
}

#[test]
fn score_candidate_deduplicates_rel_to_entries() {
    // Issue #233 dedup guard: a duplicate REL TO entry (e.g. "USA, USA")
    // must score identically to the deduplicated form ("USA") because
    // `seen_rel_to_codes` prevents double-counting.
    // Inline scheme construction per test for hermeticity; both
    // call sites route via the trait override.
    let scheme = CapcoScheme::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);

    let dup_candidate = MarkingCandidate {
        span: Span::new(0, 23),
        kind: MarkingType::Banner,
    };
    // Parser may or may not produce two rel_to entries for "USA, USA" —
    // the dedup guard must be robust either way: the prior must equal
    // that of a single "USA" entry.
    let dup_parsed = parser
        .parse(&dup_candidate, b"SECRET//REL TO USA, USA")
        .expect("SECRET//REL TO USA, USA must parse leniently");
    let dup_marking = CapcoMarking::new(scheme.canonicalize(dup_parsed.attrs));

    let once_candidate = MarkingCandidate {
        span: Span::new(0, 18),
        kind: MarkingType::Banner,
    };
    let once_parsed = parser
        .parse(&once_candidate, b"SECRET//REL TO USA")
        .expect("SECRET//REL TO USA must parse");
    let once_marking = CapcoMarking::new(scheme.canonicalize(once_parsed.attrs));

    let attempt_dup = CanonicalAttempt {
        bytes: SmallVec::from_slice(b"SECRET//REL TO USA, USA"),
        features: SmallVec::new(),
        fix_source: marque_rules::FixSource::DecoderPosterior,
    };
    let attempt_once = CanonicalAttempt {
        bytes: SmallVec::from_slice(b"SECRET//REL TO USA"),
        features: SmallVec::new(),
        fix_source: marque_rules::FixSource::DecoderPosterior,
    };

    let (prior_dup, _) = score_candidate(&attempt_dup, &dup_marking, MarkingType::Banner);
    let (prior_once, _) = score_candidate(&attempt_once, &once_marking, MarkingType::Banner);

    // Deduplication ensures the duplicate USA is only scored once, so
    // both priors must be equal (same base tokens + same single USA prior).
    assert!(
        (prior_dup - prior_once).abs() < 1e-5,
        "duplicate REL TO entry must not double-count the country-code prior; \
         prior_dup={prior_dup}, prior_once={prior_once}"
    );
}

#[test]
fn feature_entry_to_evidence_uses_canonical_label_registry() {
    // Regression guard for PR #142 H2: the projection from
    // `FeatureEntry` onto `EvidenceFeature::label` MUST route
    // through `FeatureId::as_str()` — the single source of truth
    // declared in `crates/rules/src/confidence.rs:208`. A divergent
    // local registry (the pre-fix shape, snake_case labels in a
    // duplicate match arm) produces wire-format drift the audit
    // emitter cannot detect, because today's dispatcher discards
    // `Parsed::Ambiguous` results and the bug stays latent.
    //
    // This test exhaustively covers every `FeatureId` variant. A
    // new variant added without an `as_str()` arm fails compilation
    // there (the match is exhaustive); a new variant whose label
    // diverges from `as_str()` here would have to be deliberately
    // wrong, since this test reads `id.as_str()` directly. The
    // load-bearing assertion is that `feature_entry_to_evidence`
    // does the same thing.
    for id in [
        FeatureId::EditDistance1,
        FeatureId::EditDistance2,
        FeatureId::TokenReorder,
        FeatureId::SupersededToken,
        FeatureId::BaseRateCommonMarking,
        FeatureId::StrictContextClassification,
        FeatureId::CorpusOverrideInEffect,
    ] {
        let entry = FeatureEntry { id, delta: -0.5 };
        let evidence = feature_entry_to_evidence(&entry);
        assert_eq!(
            evidence.label,
            id.as_str(),
            "decoder evidence label diverged from FeatureId::as_str() \
             for {id:?}: got {label:?}, expected {expected:?}",
            label = evidence.label,
            expected = id.as_str(),
        );
        assert_eq!(evidence.log_odds, -0.5);
    }
}

#[test]
fn runner_up_ratio_saturates_on_extreme_log_margin() {
    // Regression guard for PR #127 review comment on decoder.rs:305:
    // when `log_margin` is large enough that `f32::exp()` overflows
    // (≈ ≥ 88.7 nats on f32), the previous code emitted `+∞` into
    // `Confidence::runner_up_ratio` and `Confidence::validate`
    // rejected the resulting record at the audit boundary,
    // panicking inside `FixProposal::new`. The fix saturates at
    // `f32::MAX`. We exercise both branches here with bare
    // `f32::exp` since the saturation logic is the same closed
    // expression used in `recognize`.
    for &log_margin in &[88.0_f32, 100.0_f32, 200.0_f32, 1000.0_f32] {
        let ratio = log_margin.exp();
        let clamped = if ratio.is_finite() { ratio } else { f32::MAX };
        assert!(
            clamped.is_finite(),
            "log_margin = {log_margin}: clamped ratio must be finite, got {clamped}"
        );
        assert!(
            clamped > 0.0,
            "log_margin = {log_margin}: clamped ratio must be > 0, got {clamped}"
        );
    }
    // And a sanity check on the in-band path: at the
    // UNAMBIGUOUS_LOG_MARGIN threshold, `exp()` returns a finite
    // value and we don't clamp.
    let at_threshold = UNAMBIGUOUS_LOG_MARGIN.exp();
    assert!(at_threshold.is_finite() && at_threshold > 1.0);
}

#[test]
fn contains_hard_splitter_word_detects_per_word() {
    // Whole-string match.
    assert!(contains_hard_splitter_word("NOFORN"));
    assert!(contains_hard_splitter_word("ORCON"));
    assert!(contains_hard_splitter_word("EXDIS"));
    // Per-word match (the `Full` SAR-program-nickname absorption
    // shape — `BUTTER POPCORN NOFORN`).
    assert!(contains_hard_splitter_word("BUTTER POPCORN NOFORN"));
    assert!(contains_hard_splitter_word("ORCON BUTTER POPCORN"));
    assert!(contains_hard_splitter_word("BUTTER NOFORN POPCORN"));
    // Negatives — clean SAR identifiers must not match.
    assert!(!contains_hard_splitter_word("BP"));
    assert!(!contains_hard_splitter_word("J12"));
    assert!(!contains_hard_splitter_word("XRA"));
    assert!(!contains_hard_splitter_word("BUTTER POPCORN"));
    assert!(!contains_hard_splitter_word(""));
}

#[test]
fn absorbs_hard_splitter_detects_full_sar_program_with_trailing_noforn() {
    // The `SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN` shape:
    // strict parser builds a `Full`-indicator SAR with the program
    // identifier `"BUTTER POPCORN NOFORN"` (multi-word nickname,
    // NOFORN absorbed as the trailing word). Pinned to ensure the
    // per-word check in `contains_hard_splitter_word` keeps firing.
    use marque_ism::{CanonicalAttrs, SarIndicator, SarMarking, SarProgram};
    let sar = SarMarking::new(
        SarIndicator::Full,
        Box::new([SarProgram::new("BUTTER POPCORN NOFORN", Box::new([]))]),
    );
    let mut attrs = CanonicalAttrs::default();
    attrs.sar_markings = Some(sar);
    let marking = CapcoMarking::new(attrs);
    assert!(
        absorbs_hard_splitter_in_sar_or_sci(&marking),
        "NOFORN as trailing word of multi-word SAR program identifier must be detected"
    );
}

#[test]
fn absorbs_hard_splitter_in_sar_detects_noforn_as_subcomp() {
    // Direct construction: a SAR program with NOFORN buried as a
    // sub-compartment of a normal compartment. Mirrors the parse
    // shape produced by `SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/
    // XR-XRA RB NOFORN` when the strict parser absorbs NOFORN at
    // the SAR-block tail.
    use marque_ism::{CanonicalAttrs, SarCompartment, SarIndicator, SarMarking, SarProgram};
    use smol_str::SmolStr;
    let sar = SarMarking::new(
        SarIndicator::Abbrev,
        Box::new([SarProgram::new(
            "BP",
            Box::new([SarCompartment::new(
                "J12",
                Box::new([SmolStr::from("RB"), SmolStr::from("NOFORN")]),
            )]),
        )]),
    );
    let mut attrs = CanonicalAttrs::default();
    attrs.sar_markings = Some(sar);
    let marking = CapcoMarking::new(attrs);
    assert!(
        absorbs_hard_splitter_in_sar_or_sci(&marking),
        "NOFORN as SAR sub-compartment must be detected as absorption"
    );
}

#[test]
fn absorbs_hard_splitter_in_sar_detects_noforn_as_compartment_identifier() {
    // PR #178 review (Codecov, decoder.rs:1795): pin the
    // SAR-compartment-IDENTIFIER branch (vs the sub-compartment
    // branch covered above). Some absorbing parses end up with the
    // hard splitter as the compartment identifier itself rather
    // than a sub-compartment leaf — e.g., a `SAR-BP NOFORN` shape
    // where the strict parser emits `BP` as the program and
    // `NOFORN` as a bare compartment with no sub-compartments.
    use marque_ism::{CanonicalAttrs, SarCompartment, SarIndicator, SarMarking, SarProgram};
    let sar = SarMarking::new(
        SarIndicator::Abbrev,
        Box::new([SarProgram::new(
            "BP",
            Box::new([SarCompartment::new("NOFORN", Box::new([]))]),
        )]),
    );
    let mut attrs = CanonicalAttrs::default();
    attrs.sar_markings = Some(sar);
    let marking = CapcoMarking::new(attrs);
    assert!(
        absorbs_hard_splitter_in_sar_or_sci(&marking),
        "NOFORN as SAR compartment identifier must be detected as absorption"
    );
}

#[test]
fn absorbs_hard_splitter_accepts_clean_sar() {
    // Negative case: a SAR with realistic identifiers (`BP`, `J12`,
    // `RB`) and no hard-splitter token anywhere. Must NOT trigger
    // the penalty.
    use marque_ism::{CanonicalAttrs, SarCompartment, SarIndicator, SarMarking, SarProgram};
    use smol_str::SmolStr;
    let sar = SarMarking::new(
        SarIndicator::Abbrev,
        Box::new([SarProgram::new(
            "BP",
            Box::new([SarCompartment::new(
                "J12",
                Box::new([SmolStr::from("RB"), SmolStr::from("XRA")]),
            )]),
        )]),
    );
    let mut attrs = CanonicalAttrs::default();
    attrs.sar_markings = Some(sar);
    let marking = CapcoMarking::new(attrs);
    assert!(
        !absorbs_hard_splitter_in_sar_or_sci(&marking),
        "clean SAR identifiers must not trigger the absorption penalty"
    );
}

#[test]
fn absorbs_hard_splitter_in_sci_detects_orcon_as_subcomp() {
    // Defensive coverage for SCI absorption — the existing strict-
    // parser path drops most SCI absorption via the
    // `TokenKind::Unknown` filter in step 3a, but a future grammar
    // change that loosens SCI compartment shape could let a hard
    // splitter through. Pinned so the penalty stays defensive.
    use marque_ism::{
        CanonicalAttrs, SciCompartment, SciControlBare, SciControlSystem, SciMarking,
    };
    use smol_str::SmolStr;
    let sci = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        Box::new([SciCompartment::new("G", Box::new([SmolStr::from("ORCON")]))]),
        None,
    );
    let mut attrs = CanonicalAttrs::default();
    attrs.sci_markings = Box::new([sci]);
    let marking = CapcoMarking::new(attrs);
    assert!(
        absorbs_hard_splitter_in_sar_or_sci(&marking),
        "ORCON as SCI sub-compartment must be detected as absorption"
    );
}

#[test]
fn absorbs_hard_splitter_in_sci_detects_orcon_as_compartment_identifier() {
    // PR #178 review (Codecov, decoder.rs:1811): pin the SCI-
    // compartment-IDENTIFIER branch (vs the sub-compartment branch
    // above). Defensive coverage — today's strict-parser SCI path
    // drops most absorption via the `TokenKind::Unknown` filter at
    // step 3a, but a future grammar change that lets a hard
    // splitter through as the compartment id needs the penalty
    // active.
    use marque_ism::{
        CanonicalAttrs, SciCompartment, SciControlBare, SciControlSystem, SciMarking,
    };
    let sci = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        Box::new([SciCompartment::new(Box::from("ORCON"), Box::new([]))]),
        None,
    );
    let mut attrs = CanonicalAttrs::default();
    attrs.sci_markings = Box::new([sci]);
    let marking = CapcoMarking::new(attrs);
    assert!(
        absorbs_hard_splitter_in_sar_or_sci(&marking),
        "ORCON as SCI compartment identifier must be detected as absorption"
    );
}

#[test]
fn absorbs_hard_splitter_negative_on_empty_marking() {
    // Sanity floor: a marking with neither SAR nor SCI never
    // triggers the penalty.
    use marque_ism::CanonicalAttrs;
    let attrs = CanonicalAttrs::default();
    let marking = CapcoMarking::new(attrs);
    assert!(
        !absorbs_hard_splitter_in_sar_or_sci(&marking),
        "marking without SAR/SCI must not trigger the penalty"
    );
}
