// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tests for `decoder/recognizer.rs`. Carved into a parallel
//! file because the combined production + test surface would push
//! the file over the 800-line gate. Reached from the source file
//! via `#[path = "tests/recognizer_tests.rs"] #[cfg(test)] mod tests;`.

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
use crate::decoder::recovery::reorder::marking_classification;
use crate::decoder::scoring::absorbs_hard_splitter_in_sar_or_sci;
use crate::decoder::test_helpers::{TEST_SCHEME, deep_cx};

#[test]
fn decoder_defers_to_strict_when_strict_evidence_is_set() {
    let rx = DecoderRecognizer::new();
    let cx = ParseContext::default(); // strict_evidence = true
    match rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) {
        Parsed::Ambiguous { candidates } => assert!(candidates.is_empty()),
        other => panic!("expected zero-candidate Ambiguous, got {other:?}"),
    }
}

#[test]
fn decoder_zero_candidate_on_no_template_fit() {
    let rx = DecoderRecognizer::new();
    // Neither token is in the vocabulary and no fuzzy match.
    match rx.recognize(b"FROBNITZ//WIBBLE", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "unrecognized input must be zero-candidate, got {} candidate(s)",
            candidates.len()
        ),
        Parsed::Unambiguous(m) => panic!("unexpected strict match: {m:?}"),
    }
}

#[test]
fn decoder_resolves_sar_with_trailing_noforn_via_absorption_penalty() {
    // The SC-004 fixtures `SAR-BP-J12 …` and
    // `SPECIAL ACCESS REQUIRED-BUTTER POPCORN …` with a trailing
    // NOFORN have always produced the right candidate bytes from
    // `try_insert_delimiter`, but lost the scoring contest before
    // PR-5 because the absorbing strict parse contributed only the
    // classification's prior while the delim-inserted parse paid
    // the additional log-prior of NF. The
    // `HARD_SPLITTER_ABSORPTION_PENALTY` flips the contest; this
    // test pins both fixture shapes.
    let rx = DecoderRecognizer::new();
    for input in &[
        "TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN",
        "SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB NOFORN",
    ] {
        let parsed = rx.recognize(input.as_bytes(), 0, &*TEST_SCHEME, &deep_cx());
        match parsed {
            Parsed::Unambiguous(m) => {
                assert!(
                    m.0.sar_markings.is_some(),
                    "input {input:?}: expected SAR present in winning candidate"
                );
                // PR #178 review (Copilot, decoder.rs:2841): assert
                // the SPECIFIC dissem control we expect — `Nf`.
                // The previous `!is_empty()` check would silently
                // accept a future regression that emitted a
                // different dissem token (e.g., a misclassified
                // `Oc`/`Pr`) and still call the test green.
                assert!(
                    m.0.dissem_iter()
                        .any(|d| matches!(d, marque_ism::DissemControl::Nf)),
                    "input {input:?}: expected NOFORN (DissemControl::Nf) to land \
                     as a dissem control (winning candidate must be the delim-\
                     inserted form, not the absorbing one); got dissem_us = \
                     {:?}, dissem_nato = {:?}",
                    m.0.dissem_us,
                    m.0.dissem_nato,
                );
                assert!(
                    !absorbs_hard_splitter_in_sar_or_sci(&m),
                    "input {input:?}: winning marking must not bury a hard \
                     splitter inside SAR/SCI"
                );
            }
            other => panic!("input {input:?}: expected Unambiguous, got {other:?}"),
        }
    }
}

#[test]
fn decoder_rejects_trivial_strict_parse() {
    // The strict parser is lenient: it accepts `FROBNITZ//WIBBLE`
    // and emits an CanonicalAttrs with classification=None,
    // dissem_controls=[], sci_controls=[]. The decoder must treat
    // that as "no real parse" and drop the candidate — otherwise
    // it would fabricate an empty marking for arbitrary prose.
    // Inline scheme per test for hermeticity.
    let scheme = CapcoScheme::new();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, 16),
        kind: MarkingType::Banner,
    };
    let parsed = parser
        .parse(&candidate, b"FROBNITZ//WIBBLE")
        .expect("strict parser should accept arbitrary bytes");
    let marking = CapcoMarking::new(scheme.canonicalize(parsed.attrs));
    assert!(
        !is_nontrivial_marking(&marking),
        "empty marking must be filtered"
    );
}

#[test]
fn decoder_recovers_typo_sercet_to_secret() {
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"SERCET//NOFORN", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Unambiguous(m) => {
            assert_eq!(
                marking_classification(&m),
                Some(Classification::Secret),
                "expected SECRET classification from SERCET fuzzy-correction"
            );
        }
        other => panic!("expected Unambiguous(SECRET//NOFORN), got {other:?}"),
    }
}

#[test]
fn decoder_recovers_case_mangled_input() {
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"secret//noforn", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Unambiguous(m) => {
            assert_eq!(marking_classification(&m), Some(Classification::Secret));
        }
        other => panic!("expected Unambiguous, got {other:?}"),
    }
}

#[test]
fn decoder_suppresses_prose_glue_single_letter_portion() {
    // Prose-glue heuristic: when the byte preceding the candidate
    // is NOT whitespace, a single-letter `(s)` / `(c)` is
    // overwhelmingly a plural-suffix (`letter(s)`) or function-
    // call glyph (`function(c)`). The decoder must produce zero
    // candidates so the engine doesn't synthesize a spurious R001
    // diagnostic.
    let rx = DecoderRecognizer::new();
    let glued = ParseContext {
        preceded_by_whitespace: false,
        ..deep_cx()
    };
    for input in &[b"(s)", b"(c)", b"(u)", b"(S)", b"(C)"] {
        match rx.recognize(*input, 0, &*TEST_SCHEME, &glued) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "{:?} glued to a word must produce zero candidates, got {}",
                std::str::from_utf8(*input).unwrap_or("<bytes>"),
                candidates.len(),
            ),
            Parsed::Unambiguous(_) => panic!(
                "{:?} glued to a word must not resolve",
                std::str::from_utf8(*input).unwrap_or("<bytes>"),
            ),
        }
    }
}

#[test]
fn decoder_prose_glue_suppresses_u_that_null_gate_would_admit() {
    // HIGH 1 (review) — pins the independence of the prose-glue
    // early-return from the post-#472 null gate.
    //
    // The `U`-token marking-y delta is `+2.86`, which exceeds the
    // [`NULL_HYPOTHESIS_LOG_MARGIN`] (`+2.5`) — an isolated `(u)`
    // with `preceded_by_whitespace = true` clears the null gate
    // and recovers to UNCLASSIFIED (the
    // `decoder_residual_gap_isolated_u_recovers_to_unclassified`
    // test pins that recovery).
    //
    // This test pins the symmetric case: the SAME `(u)` with
    // `preceded_by_whitespace = false` (e.g., `function(u)`,
    // `sec(u)rity`) must be suppressed. Because the null gate
    // alone would admit it, the prose-glue early-return is
    // independently load-bearing here. Removing the early-return
    // (e.g., on the assumption that the null gate now subsumes
    // it) would silently regress this case.
    let rx = DecoderRecognizer::new();

    // Baseline: not glued, null gate admits, recovers to
    // UNCLASSIFIED.
    let standalone = rx.recognize(b"(u)", 0, &*TEST_SCHEME, &deep_cx());
    assert!(
        matches!(
            &standalone,
            Parsed::Unambiguous(m)
                if m.0.classification
                    == Some(MarkingClassification::Us(Classification::Unclassified))
        ),
        "standalone `(u)` must recover to UNCLASSIFIED via the \
         null gate's +2.86 marking-y delta exceeding the +2.5 \
         margin; got {standalone:?}",
    );

    // Glued: same input, `preceded_by_whitespace = false`. The
    // prose-glue early-return suppresses BEFORE the null gate.
    let glued_cx = ParseContext {
        preceded_by_whitespace: false,
        ..deep_cx()
    };
    let glued = rx.recognize(b"(u)", 0, &*TEST_SCHEME, &glued_cx);
    match glued {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "glued `(u)` (preceded_by_whitespace=false) must be \
             zero-candidate via the prose-glue early-return; got \
             {} candidate(s)",
            candidates.len(),
        ),
        Parsed::Unambiguous(m) => panic!(
            "glued `(u)` must be suppressed by the prose-glue \
             early-return — the post-#472 null gate alone admits \
             this case ({:+.2} delta exceeds {:+.2} margin), so \
             prose-glue removal would silently regress it. Got \
             Unambiguous({:?})",
            2.86_f32, NULL_HYPOTHESIS_LOG_MARGIN, m.0.classification,
        ),
    }
}

#[test]
fn decoder_suppresses_single_letter_portion_via_null_hypothesis() {
    // Issue #258 + PR1 (documents-corpus marking stratum): an
    // isolated `(s)` (preceded by whitespace, so the prose-glue
    // heuristic is bypassed) is the prose null-hypothesis case.
    // The decoder must produce zero candidates so the engine
    // doesn't synthesize a spurious R001 diagnostic.
    //
    // Before PR1: the marking-side prior for `S` was the Laplace
    // floor (zero hits in `tests/corpus/valid/`) so the per-token
    // marking-y delta `log P("S"|marking) − log P("S"|prose)` was
    // negative — the null hypothesis won under the original
    // `posterior >= null_posterior` filter.
    //
    // After PR1: `tests/corpus/documents/marked/` contributes 173
    // hits for `S`, pushing the marking-side delta to `+2.21`
    // (`S`: marking `-3.28`, prose `-5.49`). A zero-margin
    // filter would let the marking hypothesis win and
    // re-introduce the SC-003a Federalist `(s)` regression. The
    // `NULL_HYPOTHESIS_LOG_MARGIN = 2.5` floor (see constant
    // doc) was tuned to keep `(s)` suppressed at +2.21 while
    // still admitting multi-token candidates whose delta is
    // many times larger.
    //
    // This is the exact behavior that closes the SC-003a
    // regression on `Notwithstanding (s) the early prevalence` —
    // the decoder doesn't auto-fix prose-shaped single-letter
    // portions to a SECRET portion.
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"(s)", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "isolated lowercase (s) must be zero-candidate (null wins), \
             got {} candidate(s)",
            candidates.len()
        ),
        Parsed::Unambiguous(m) => panic!(
            "isolated lowercase (s) must be suppressed by the prose null \
             hypothesis, got Unambiguous({:?})",
            m.0.classification,
        ),
    }
}

#[test]
fn decoder_admits_mangled_marking_under_observed_null_gate() {
    // Copilot #3 follow-up: must-not-over-suppress stress test
    // for the [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`] = -7.0 floor.
    //
    // The constant's doc-comment names `(CMS)` / SC-003a as the
    // must-suppress side: prose acronyms with no marking
    // vocabulary support should fall below
    // [`NULL_HYPOTHESIS_LOG_MARGIN`] and be suppressed.
    // This test pins the symmetric must-NOT-over-suppress side:
    // a genuinely mangled-but-recoverable marking
    // (`(SERCET//NF)`, edit-distance-1 typo of `SECRET`) must
    // clear the same gate and reach `Parsed::Unambiguous`. If a
    // future calibration change tightens the floor too far —
    // raising it to where multi-token mangled markings get
    // swept up — this test fails.
    //
    // `(SERCET//NF)` chosen because:
    // 1. Already named in the [`OBSERVED_UNKNOWN_PROSE_LOG_PRIOR`]
    //    constant doc as the example case the floor was sized to
    //    admit ("legitimate single-token mangled-marking
    //    recoveries (`(SERCET//NF)`) stay above it").
    // 2. Has `//` so [`has_double_slash`] bypasses the null gate
    //    entirely — this test pins the bypass + scoring path,
    //    not just the gate threshold. A regression that broke
    //    `has_double_slash` would surface as this candidate
    //    failing recovery, with the gate threshold a secondary
    //    suspect.
    // 3. Strong marking-side prior (SECRET + NOFORN both in
    //    `token_base_rates`) producing a high posterior so the
    //    runner-up ratio and resulting confidence sit well
    //    above the default `confidence_threshold = 0.95`.
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"(SERCET//NF)", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Unambiguous(m) => {
            // The strict parse on the canonicalized bytes must
            // yield `Us(Secret)`.
            assert_eq!(
                m.0.classification,
                Some(MarkingClassification::Us(Classification::Secret)),
                "(SERCET//NF) must recover to Us(Secret); got {:?}",
                m.0.classification,
            );
            // Provenance must carry an EditDistance feature —
            // confirms the fuzzy-correction path was exercised
            // (SERCET → SECRET, Levenshtein 2: R↔C transpose
            // requires two substitutions). EditDistance1 OR
            // EditDistance2 both indicate the fuzzy path
            // produced the canonical form.
            let prov =
                m.1.as_ref()
                    .expect("decoder-path recovery must carry DecoderProvenance");
            let has_edit_distance = prov
                .features
                .iter()
                .any(|f| matches!(f.id, FeatureId::EditDistance1 | FeatureId::EditDistance2));
            assert!(
                has_edit_distance,
                "(SERCET//NF) recovery must record an EditDistance \
                 feature in provenance (SERCET → SECRET); got {:?}",
                prov.features,
            );
        }
        Parsed::Ambiguous { candidates } => panic!(
            "(SERCET//NF) is the canonical must-not-over-suppress \
             case named in the OBSERVED_UNKNOWN_PROSE_LOG_PRIOR \
             constant doc — recovery must succeed. If this fails, \
             audit (a) whether `has_double_slash` still bypasses \
             the null gate for `//`-bearing inputs, (b) whether \
             the `-7.0` floor or `+2.5` margin was tightened, or \
             (c) whether SECRET / NOFORN dropped out of \
             `token_base_rates`. Got Ambiguous with {} candidate(s).",
            candidates.len(),
        ),
    }
}

#[test]
fn decoder_residual_gap_isolated_u_recovers_to_unclassified() {
    // KNOWN RESIDUAL GAP — pinning current behavior.
    //
    // `(u)` has a `+2.86` marking-vs-prose delta on the `U`
    // token (see the NULL_HYPOTHESIS_LOG_MARGIN constant doc on
    // line ~152). That delta exceeds the `+2.5` null filter
    // margin, so an isolated `(u)` recovers to UNCLASSIFIED
    // when no context features fire (test-default
    // `ParseContext` carries `line_offset: None`,
    // `line_prefix: None`, `surrounding_is_lowercase: false`).
    //
    // The Task 10 `LowercaseSurroundingContext` feature
    // (`-2.0`) suppresses the common mid-prose `(u)` case in
    // lowercase-dominant context (decoder.rs::
    // decoder_applies_lowercase_context_penalty_in_lowercase_prose
    // pins that). The residual surface is `(u)` at column 0
    // in mixed-case or uppercase context — vanishingly rare in
    // real IC text, but not zero.
    //
    // This test pins the current behavior so a future
    // regression (drift in token priors, threshold tuning, a
    // new feature) is loud. Closing the gap further likely
    // requires a third signal (document-level archival mode,
    // page zone, etc.) and is deferred — see PR description
    // "Deferred (separate work)".
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"(u)", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Unambiguous(m) => {
            assert_eq!(
                m.0.classification,
                Some(MarkingClassification::Us(Classification::Unclassified)),
                "isolated `(u)` at default ParseContext currently \
                 resolves to UNCLASSIFIED (documented residual gap, \
                 +2.86 marking-vs-prose delta exceeds +2.5 margin)",
            );
        }
        Parsed::Ambiguous { candidates } => {
            panic!(
                "isolated `(u)` was expected to recover to UNCLASSIFIED \
                 under the pinned residual gap; got Ambiguous with {} \
                 candidate(s). If the decoder behavior tightened, this \
                 test should be inverted to assert zero candidates and \
                 the residual-gap doc rationale on \
                 NULL_HYPOTHESIS_LOG_MARGIN updated to reflect the new \
                 behavior.",
                candidates.len(),
            );
        }
    }
}

#[test]
fn decoder_rejects_bare_restricted_via_recognizer_predicate() {
    // `(R)` parses cleanly under the strict path's lenient
    // grammar but fails `is_us_restricted` at
    // both the strict recognizer and inside the decoder's
    // candidate loop (step 3c-bis). The decoder must produce
    // zero candidates regardless of preceded-by-whitespace.
    let rx = DecoderRecognizer::new();
    for cx in &[
        deep_cx(),
        ParseContext {
            preceded_by_whitespace: false,
            ..deep_cx()
        },
    ] {
        match rx.recognize(b"(r)", 0, &*TEST_SCHEME, cx) {
            Parsed::Ambiguous { candidates } => assert!(
                candidates.is_empty(),
                "bare (r) must be zero-candidate (preceded_by_whitespace={}), got {}",
                cx.preceded_by_whitespace,
                candidates.len()
            ),
            Parsed::Unambiguous(m) => panic!(
                "bare (r) must be rejected, got Unambiguous({:?})",
                m.0.classification
            ),
        }
    }
}

#[test]
fn decoder_recovers_superseded_comint_to_si() {
    let rx = DecoderRecognizer::new();
    // SECRET//COMINT//NOFORN — COMINT is CAPCO-2016 §A.6 p16-superseded to SI.
    match rx.recognize(b"SECRET//COMINT//NOFORN", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Unambiguous(m) => {
            assert_eq!(marking_classification(&m), Some(Classification::Secret));
            // Verify SI is in the SCI controls list after correction.
            let has_si =
                m.0.sci_controls
                    .iter()
                    .any(|c| matches!(c, marque_ism::SciControl::Si));
            assert!(
                has_si,
                "expected SI in sci_controls after COMINT supersession"
            );
        }
        other => panic!("expected Unambiguous, got {other:?}"),
    }
}

#[test]
fn decoder_recovers_reordered_banner() {
    let rx = DecoderRecognizer::new();
    // Dissem-first mangled; canonical is classification-first.
    match rx.recognize(b"NOFORN//SECRET", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Unambiguous(m) => {
            assert_eq!(marking_classification(&m), Some(Classification::Secret));
        }
        Parsed::Ambiguous { candidates } => {
            assert!(
                !candidates.is_empty(),
                "reordering should at least surface candidates"
            );
        }
    }
}

#[test]
fn decoder_honors_classification_floor_fr011() {
    let rx = DecoderRecognizer::new();
    // Input is "(U)" which canonicalizes to an UNCLASSIFIED
    // portion. With a Secret floor, the candidate must be
    // dropped.
    let cx = ParseContext {
        strict_evidence: false,
        classification_floor: Some(Classification::Secret as u8),
        preceded_by_whitespace: true,
        ..ParseContext::default()
    };
    match rx.recognize(b"(U)", 0, &*TEST_SCHEME, &cx) {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "UNCLASSIFIED below SECRET floor must produce zero candidates, got {}",
            candidates.len()
        ),
        Parsed::Unambiguous(m) => panic!(
            "expected zero-candidate, got Unambiguous({:?})",
            marking_classification(&m)
        ),
    }
}

#[test]
fn decoder_classification_floor_allows_equal_or_above() {
    let rx = DecoderRecognizer::new();
    // (S//NF) with Confidential floor — SECRET exceeds floor.
    let cx = ParseContext {
        strict_evidence: false,
        classification_floor: Some(Classification::Confidential as u8),
        preceded_by_whitespace: true,
        ..ParseContext::default()
    };
    match rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &cx) {
        Parsed::Unambiguous(m) => {
            assert_eq!(marking_classification(&m), Some(Classification::Secret));
        }
        other => panic!("expected Unambiguous, got {other:?}"),
    }
}
