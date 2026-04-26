// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 4 PR-3 — decoder recovery tests (T045, T046, T047).
//!
//! These tests exercise the three mangling classes US2 targets:
//!
//! - **T046** Typo-to-canonical (`SERCET//NOFORN` → `SECRET//NOFORN`).
//! - **T047** Banner reordering (`NOFORN//SECRET` → `SECRET//NOFORN`).
//! - **T045** Strict-context classification floor (FR-011:
//!   ambiguous `(C)` decodes to CONFIDENTIAL only when a strict
//!   CONFIDENTIAL-or-higher floor is established for the page).
//!
//! Tests hit `DecoderRecognizer` directly (not through `Engine::lint`)
//! because the PR-3 scope covers the recognizer only — Engine-side
//! wiring of per-page classification floors and audit-record
//! emission is PR-4 scope. When PR-4 lands, these tests graduate to
//! end-to-end `Engine::lint` + audit-stream checks.

use marque_capco::CapcoScheme;
use marque_engine::DecoderRecognizer;
use marque_ism::{Classification, DissemControl, NonIcDissem, SciControl};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};

fn deep_cx() -> ParseContext {
    ParseContext {
        strict_evidence: false,
        zone: None,
        position: None,
        classification_floor: None,
    }
}

fn effective_level(m: &marque_capco::CapcoMarking) -> Option<Classification> {
    m.0.classification.as_ref().map(|c| c.effective_level())
}

// ---------------------------------------------------------------------------
// T046: typo-to-canonical
// ---------------------------------------------------------------------------

#[test]
fn sercet_decodes_to_secret_via_edit_distance_one() {
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"SERCET//NOFORN", &deep_cx()) else {
        panic!("SERCET//NOFORN should resolve unambiguously to SECRET//NOFORN");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Secret),
        "typo correction must yield SECRET classification"
    );
    // Verify NOFORN landed somewhere in the marking — the strict
    // parser may categorize it as a dissem control or surface it in
    // the non-IC dissem slot. Print-debug shows the exact location
    // if the assertion fails.
    let dissem_count = marking.0.dissem_controls.len();
    let non_ic_count = marking.0.non_ic_dissem.len();
    assert!(
        dissem_count + non_ic_count > 0,
        "NOFORN must survive the typo-correction pass somewhere in the marking; \
         attrs = {:?}",
        marking.0,
    );
}

#[test]
fn fuzzy_ambiguity_yields_zero_candidate() {
    // `SECRET//RSE` — `RSE` is at fuzzy edit-distance 1 from BOTH
    // `RS` (insert E) AND `RSEN` (delete N), both of which are in
    // the extended correction vocab after the issue #133 long-form
    // dissem fix. The matcher returns `None` on the two-way tie,
    // the unknown token passes through to strict parse as
    // `TokenKind::Unknown`, and the decoder's step-3a Unknown-span
    // filter rejects the partial candidate.
    //
    // Honesty invariant FR-015: when any token is unresolvable,
    // the decoder surfaces zero candidates rather than fabricating
    // a marking that silently drops the unresolved token.
    //
    // Renamed and re-anchored from
    // `double_typo_zero_candidate_when_one_token_is_ambiguous`
    // (originally used `SERCET//NOFRN`). That example stopped
    // exercising the ambiguity path once the issue #133 long-form
    // vocab fix added `NOFORN` to the matcher's vocabulary —
    // `NOFRN → NOFORN` is now unambiguously distance-1. The
    // FR-015 invariant still warrants a regression test, just with
    // an example that holds under the extended vocab. The companion
    // `partial_canonicalization_with_unresolvable_token_returns_zero_candidate`
    // covers the distinct "uncorrectable / no candidate close
    // enough" path (e.g., `SECRET//WIBBLE`).
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"SECRET//RSE", &deep_cx()) {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "decoder must not fabricate partial candidates when any \
             token is fuzzy-ambiguous (FR-015 honesty invariant); \
             got {} candidate(s)",
            candidates.len(),
        ),
        Parsed::Unambiguous(m) => panic!(
            "expected zero-candidate; fabricated partial marking \
             would regress the r3 fix. marking = {:?}",
            m.0,
        ),
    }
}

#[test]
fn partial_canonicalization_with_unresolvable_token_returns_zero_candidate() {
    // Regression guard for PR #114 round-3 review: the decoder must
    // NOT produce a partial candidate when a token is
    // un-correctable. `SECRET//WIBBLE` was the reviewer's
    // pathological case — classification fine, tail token
    // uncorrectable. Without the Unknown-span filter the decoder
    // would have emitted a `SECRET` candidate silently dropping
    // WIBBLE. With the filter in place the candidate is dropped.
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"SECRET//WIBBLE", &deep_cx()) {
        Parsed::Ambiguous { candidates } => {
            assert!(
                candidates.is_empty(),
                "decoder must not silently drop uncorrectable tokens; \
                 got {} candidate(s): {:?}",
                candidates.len(),
                candidates,
            );
        }
        Parsed::Unambiguous(m) => panic!(
            "expected zero-candidate (WIBBLE is uncorrectable); got \
             Unambiguous({:?}) — partial-marking fabrication has \
             regressed",
            m.0,
        ),
    }
}

// ---------------------------------------------------------------------------
// T047: banner reordering
// ---------------------------------------------------------------------------

#[test]
fn dissem_first_banner_decodes_to_canonical_order() {
    // Canonical order is classification → SCI → SAR → dissem. The
    // decoder's reorder pass should swap dissem-first input.
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"NOFORN//SECRET", &deep_cx()) {
        Parsed::Unambiguous(marking) => {
            assert_eq!(
                effective_level(&marking),
                Some(Classification::Secret),
                "reorder should put SECRET in classification slot; \
                 attrs = {:?}",
                marking.0,
            );
        }
        Parsed::Ambiguous { candidates } => {
            // Ambiguous with at least one viable candidate is also
            // acceptable — the reorder succeeds even if a second
            // interpretation ties for top posterior.
            assert!(!candidates.is_empty());
            let has_secret = candidates
                .iter()
                .any(|c| effective_level(&c.marking) == Some(Classification::Secret));
            assert!(
                has_secret,
                "reorder candidate set must contain a SECRET option"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// T045: strict-context classification floor (FR-011)
// ---------------------------------------------------------------------------

#[test]
fn unclassified_candidate_rejected_below_secret_floor() {
    // `(U)` decodes to UNCLASSIFIED at 1.0 when no floor is set. With
    // a SECRET floor, the candidate is below the floor and must be
    // dropped — decoder returns zero-candidate Ambiguous.
    let rx = DecoderRecognizer::new();
    let floored = ParseContext {
        classification_floor: Some(Classification::Secret as u8),
        ..deep_cx()
    };
    match rx.recognize(b"(U)", &floored) {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "UNCLASSIFIED below SECRET floor must zero-out candidates, got {}",
            candidates.len()
        ),
        Parsed::Unambiguous(m) => panic!(
            "expected zero-candidate (FR-011), got {:?}",
            effective_level(&m)
        ),
    }
}

#[test]
fn floor_at_equal_level_accepts_candidate() {
    // `(S)` with a SECRET floor passes — equal clears the floor.
    let rx = DecoderRecognizer::new();
    let floored = ParseContext {
        classification_floor: Some(Classification::Secret as u8),
        ..deep_cx()
    };
    match rx.recognize(b"(S)", &floored) {
        Parsed::Unambiguous(marking) => {
            assert_eq!(effective_level(&marking), Some(Classification::Secret));
        }
        other => panic!("(S) at SECRET floor should decode unambiguously, got {other:?}"),
    }
}

#[test]
fn floor_below_candidate_accepts_higher_level() {
    // `(TS)` with a CONFIDENTIAL floor passes — TopSecret exceeds
    // Confidential.
    let rx = DecoderRecognizer::new();
    let floored = ParseContext {
        classification_floor: Some(Classification::Confidential as u8),
        ..deep_cx()
    };
    match rx.recognize(b"(TS)", &floored) {
        Parsed::Unambiguous(marking) => {
            assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
        }
        other => panic!("(TS) above CONFIDENTIAL floor should decode, got {other:?}"),
    }
}

#[test]
fn no_floor_accepts_any_classification() {
    // With `classification_floor: None` the floor is inactive —
    // any classification passes through.
    let rx = DecoderRecognizer::new();
    for (input, expected) in [
        (b"(U)".as_slice(), Classification::Unclassified),
        (b"(C)".as_slice(), Classification::Confidential),
        (b"(S)".as_slice(), Classification::Secret),
        (b"(TS)".as_slice(), Classification::TopSecret),
    ] {
        match rx.recognize(input, &deep_cx()) {
            Parsed::Unambiguous(marking) => {
                assert_eq!(
                    effective_level(&marking),
                    Some(expected),
                    "no-floor should pass {} → {:?}",
                    std::str::from_utf8(input).unwrap(),
                    expected,
                );
            }
            other => panic!(
                "{} should decode unambiguously without a floor, got {other:?}",
                std::str::from_utf8(input).unwrap()
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Per-class smoke coverage (Phase 4 review M11)
//
// One named test per mangling class so a regression on any single
// class fails with a specific test name, rather than only surfacing
// as a delta in the aggregate decoder_accuracy harness rate. The
// aggregate harness in `decoder_accuracy.rs` still owns the
// statistical floor; these are the regression-name-per-class
// counterpart.
// ---------------------------------------------------------------------------

/// **WrongCase** class — `secret//noforn` → `SECRET//NOFORN`.
///
/// The decoder's case-normalization pass routes through fuzzy match
/// with case-insensitive comparison; this is the simplest two-token
/// case-mangled input that the class targets. The decoder_accuracy
/// harness reports WrongCase at 100% resolution; this pins the
/// canonical example behind a specific test name.
#[test]
fn wrong_case_lowercase_marking_decodes_to_canonical() {
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"secret//noforn", &deep_cx()) else {
        panic!("lowercase secret//noforn should case-normalize to SECRET//NOFORN");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Secret),
        "case normalization must yield SECRET classification"
    );
    // Pin NOFORN preservation: a regression that case-normalized SECRET
    // correctly but dropped the trailing `noforn` token would still
    // pass an Unambiguous-with-Secret check. Assert NOFORN survives in
    // the resolved marking. (NOFORN is `DissemControl::Nf`, not a
    // `NonIcDissem` variant.)
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "case normalization must preserve NOFORN; attrs = {:?}",
        marking.0,
    );
}

/// **GarbledDelimiter** class — extra spaces around `//`.
///
/// `TOP SECRET //NOFORN` (whitespace before the delimiter) is one of
/// the canonical garbled-delimiter shapes the decoder targets;
/// `normalize_delimiters_and_case` collapses interior whitespace
/// around `//` before strict-parsing. The decoder_accuracy harness
/// reports GarbledDelimiter at 100%; this is the named case.
#[test]
fn garbled_delimiter_extra_space_decodes_to_canonical() {
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"TOP SECRET //NOFORN", &deep_cx()) else {
        panic!("TOP SECRET //NOFORN (extra space) should normalize to TOP SECRET//NOFORN");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::TopSecret),
        "garbled-delimiter normalization must preserve TOP SECRET classification"
    );
    // Pin NOFORN preservation: a delimiter-normalization regression
    // that handled the leading classification but dropped the trailing
    // dissem control would still pass classification-only checks.
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "delimiter normalization must preserve NOFORN; attrs = {:?}",
        marking.0,
    );
}

/// **SupersededToken** class — `COMINT` → `SI` substitution.
///
/// CAPCO-2016 §H.4 p74 retired the `COMINT` title for the Special
/// Intelligence (SI) control system; the decoder's
/// `SUPERSEDED_TOKEN_MAP` substitutes the live form. This test pins
/// the canonical example.
#[test]
fn superseded_comint_decodes_to_si() {
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"TOP SECRET//COMINT//NOFORN", &deep_cx())
    else {
        panic!("TOP SECRET//COMINT//NOFORN should supersede COMINT to SI");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::TopSecret),
        "superseded-token substitution must preserve TOP SECRET classification"
    );
    // Pin the actual supersession: `COMINT` must rewrite to
    // `SciControl::Si` in the resolved attrs. A "some SCI presence"
    // soft check would pass for any SCI-bearing marking and miss the
    // case where the substitution dropped to the wrong control system.
    // Mirrors the inline `decoder_recovers_superseded_comint_to_si`
    // unit test in `crates/engine/src/decoder.rs`.
    let has_si = marking
        .0
        .sci_controls
        .iter()
        .any(|c| matches!(c, SciControl::Si));
    assert!(
        has_si,
        "expected SI in sci_controls after COMINT supersession; \
         attrs = {:?}",
        marking.0,
    );
    // The trailing `//NOFORN` must also survive the supersession pass.
    // A regression that handled COMINT → SI but dropped the dissem
    // control tail would pass the SI check above and the
    // classification check.
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "COMINT supersession must preserve NOFORN; attrs = {:?}",
        marking.0,
    );
}

/// **MissingDelimiter** class — current-state regression guard.
///
/// `SECRET//NOFORN EXDIS` (missing `//` before `EXDIS`) is the
/// canonical MissingDelimiter shape. The decoder_accuracy harness
/// reports MissingDelimiter at 0% resolution today (tracked under GH
/// issue #133); the recognizer either returns zero-candidate
/// `Ambiguous` or an `Unambiguous` marking that does not include
/// `EXDIS` in the dissem-control set.
///
/// This test pins the **current** behavior so a future improvement
/// to the MissingDelimiter codepath (which would either resolve the
/// input correctly or change which dissem-controls survive) fails
/// here loudly, prompting a review-and-update of the assertion shape
/// rather than silently shifting the harness's per-class rate. When
/// MissingDelimiter recovery lands, replace this assertion with a
/// successful-resolution check (mirroring the WrongCase / Garbled
/// shapes above).
#[test]
fn missing_delimiter_secret_noforn_exdis_currently_unrecovered() {
    let rx = DecoderRecognizer::new();
    let result = rx.recognize(b"SECRET//NOFORN EXDIS", &deep_cx());
    match result {
        Parsed::Unambiguous(marking) => {
            // Decoder may resolve to SECRET//NOFORN with EXDIS dropped
            // (the trailing run after `NOFORN ` doesn't strict-parse
            // as a separate dissem control without the `//`).
            // EXDIS is a non-IC dissem control — confirm it did NOT
            // land in the resolved marking via a direct variant check
            // (avoids the Debug-format brittleness and per-iter
            // allocation that the closure-with-format! shape would
            // introduce). If a future fix carries EXDIS through, this
            // assertion fails and the test should be rewritten to
            // celebrate the recovery.
            assert!(
                !marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis),
                "MissingDelimiter recovery (issue #133) appears to have improved — \
                 EXDIS is now surviving in `SECRET//NOFORN EXDIS`. Update the test \
                 to assert successful recovery rather than current-state limitation. \
                 attrs = {:?}",
                marking.0,
            );
        }
        Parsed::Ambiguous { candidates } => {
            // Zero-candidate Ambiguous is the current-state shape —
            // the decoder doesn't find a confident canonicalization
            // for `SECRET//NOFORN EXDIS` today (issue #133). Asserting
            // an empty candidate set is the strict regression guard:
            // if a future improvement starts producing non-empty
            // ambiguous candidates here, the assertion fails and the
            // test should be rewritten to assert the new recovery
            // shape rather than relying on `<= K_MAX_CANDIDATES`
            // (which is always true and would mask any partial
            // improvement).
            assert!(
                candidates.is_empty(),
                "MissingDelimiter recovery (issue #133) appears to have improved — \
                 ambiguous candidates are now being produced for `SECRET//NOFORN EXDIS`. \
                 Update the test to assert the new recovery behavior rather than the \
                 current zero-candidate limitation. candidates = {candidates:?}",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Scheme ergonomics: CapcoScheme + decoder integrate without extra glue
// ---------------------------------------------------------------------------

#[test]
fn decoder_recognizer_implements_recognizer_for_capco_scheme() {
    fn assert_impl<R: Recognizer<CapcoScheme>>() {}
    assert_impl::<DecoderRecognizer>();
}

// The `prior_log_odds` split invariant (Candidate::prior_log_odds
// carries the prior alone, not the full posterior) is intentionally
// NOT tested here. An integration-level assertion through the
// recognizer surface is fragile: the decoder may legitimately resolve
// unambiguously (no `Candidate` record emitted) or return zero-
// candidate on any given input, which makes the check vacuous
// whenever the implementation gets more accurate. The invariant is
// pinned by the deterministic decoder-local unit test
// `score_candidate_splits_prior_and_posterior` in `decoder.rs`,
// which constructs a known `CanonicalAttempt` + marking and asserts
// the split exactly.
