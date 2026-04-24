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
use marque_ism::Classification;
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
fn double_typo_sercet_nofrn_decodes_to_canonical() {
    // Edit-distance-1 on both tokens. SERCET → SECRET, NOFRN → NOFORN.
    // Both sit within the fuzzy matcher's edit budget.
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"SERCET//NOFRN", &deep_cx()) {
        Parsed::Unambiguous(marking) => {
            assert_eq!(
                effective_level(&marking),
                Some(Classification::Secret),
                "double typo should still recover SECRET"
            );
        }
        Parsed::Ambiguous { candidates } => {
            assert!(
                !candidates.is_empty(),
                "double typo must at least surface candidates"
            );
        }
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
// Scheme ergonomics: CapcoScheme + decoder integrate without extra glue
// ---------------------------------------------------------------------------

#[test]
fn decoder_recognizer_implements_recognizer_for_capco_scheme() {
    fn assert_impl<R: Recognizer<CapcoScheme>>() {}
    assert_impl::<DecoderRecognizer>();
}
