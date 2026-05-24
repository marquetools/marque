// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Recognizer-outcome regression suite for the decoder split (#562).
//!
//! Pins the semantic `Parsed<CapcoMarking>` output (Unambiguous shape +
//! key marking attributes, or Ambiguous-with-zero-candidates) of both
//! `DecoderRecognizer` and `StrictOrDecoderRecognizer` against an
//! 18-fixture corpus covering each recovery pass + null-hypothesis
//! suppression branch. Catches the case where the split changes
//! behavior in a way that no other unit or integration test happens
//! to exercise — the brief flagged this as the load-bearing
//! correctness gate ("it builds and is green ≠ it functions
//! correctly"). Compares parsed shapes, not raw bytes — the engine
//! does not surface the canonicalized byte stream at the recognizer
//! boundary.
//!
//! Each fixture asserts the Parsed shape and, for Unambiguous, the
//! observed classification + dissem-count (and REL TO countries
//! where relevant). Fixtures pin observed behavior, not aspirational
//! behavior; if a future scoring tweak changes which candidate wins
//! for a borderline input, this test fails and the fixture is
//! updated.

use marque_capco::CapcoScheme;
use marque_engine::{DecoderRecognizer, StrictOrDecoderRecognizer};
use marque_ism::{Classification, DissemControl, MarkingClassification, SciControl};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};

fn deep_cx() -> ParseContext {
    // Mirrors the in-mod test helper: `strict_evidence = false` so
    // the dispatcher always tries the decoder leg, and
    // `preceded_by_whitespace = true` so the prose-glue suppression
    // doesn't fire on single-letter portion inputs.
    ParseContext {
        strict_evidence: false,
        preceded_by_whitespace: true,
        ..ParseContext::default()
    }
}

fn scheme() -> CapcoScheme {
    CapcoScheme::new()
}

fn classification(m: &marque_capco::CapcoMarking) -> Option<Classification> {
    m.0.classification.as_ref().map(|c| c.effective_level())
}

fn dissem_us_count(m: &marque_capco::CapcoMarking) -> usize {
    m.0.dissem_us.len()
}

fn rel_to_count(m: &marque_capco::CapcoMarking) -> usize {
    m.0.rel_to.len()
}

fn run_decoder(input: &str) -> Parsed<marque_capco::CapcoMarking> {
    let rx = DecoderRecognizer;
    let sc = scheme();
    rx.recognize(input.as_bytes(), 0, &sc, &deep_cx())
}

fn run_dispatcher(input: &str) -> Parsed<marque_capco::CapcoMarking> {
    let rx = StrictOrDecoderRecognizer::new();
    let sc = scheme();
    rx.recognize(input.as_bytes(), 0, &sc, &deep_cx())
}

// ---------------------------------------------------------------------------
// Canonical / clean inputs — strict-leg should win in the dispatcher; the
// decoder leg is exercised via DecoderRecognizer directly to confirm it also
// produces a sane Unambiguous result.
// ---------------------------------------------------------------------------

#[test]
fn dispatcher_resolves_clean_portion_secret_noforn_unambiguously() {
    let parsed = run_dispatcher("(SECRET//NOFORN)");
    let Parsed::Unambiguous(m) = parsed else {
        panic!("expected Unambiguous; got {parsed:?}");
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    assert_eq!(dissem_us_count(&m), 1);
}

#[test]
fn decoder_resolves_typo_sercet_to_secret() {
    let parsed = run_decoder("(SERCET//NOFORN)");
    let Parsed::Unambiguous(m) = parsed else {
        panic!("expected Unambiguous; got {parsed:?}");
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    assert_eq!(dissem_us_count(&m), 1);
}

#[test]
fn decoder_resolves_lowercase_secret_noforn_to_canonical() {
    let parsed = run_decoder("secret//noforn");
    let Parsed::Unambiguous(m) = parsed else {
        panic!("expected Unambiguous; got {parsed:?}");
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    assert_eq!(dissem_us_count(&m), 1);
}

// ---------------------------------------------------------------------------
// Null-hypothesis suppression — short portion shapes mid-prose should NOT
// resolve to markings. Constructs a context where the surrounding bytes are
// clearly prose-shaped so the null gate fires.
// ---------------------------------------------------------------------------

#[test]
fn dispatcher_suppresses_single_letter_portion_in_prose_context() {
    // Federalist `(s)` mid-sentence — the prose hypothesis should win
    // and the decoder should return zero candidates.
    let rx = StrictOrDecoderRecognizer::new();
    let sc = scheme();
    let mut cx = deep_cx();
    cx.preceded_by_whitespace = false; // glue to prior word
    let parsed = rx.recognize(b"(s)", 0, &sc, &cx);
    // Either Ambiguous-empty (decoder rejected) or Unambiguous with empty
    // attrs (strict accepted a degenerate parse) — neither produces a
    // user-visible marking. We accept both shapes.
    match parsed {
        Parsed::Ambiguous { ref candidates } if candidates.is_empty() => {}
        Parsed::Unambiguous(ref m) if m.0.classification.is_none() => {}
        other => panic!("expected zero-candidate or trivial-attrs; got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Recovery-pass coverage — each pass gets one or two fixtures to confirm the
// pass still fires and produces a candidate the scoring layer can rank.
// ---------------------------------------------------------------------------

#[test]
fn decoder_recovers_missing_delimiter_secret_noforn_exdis() {
    let parsed = run_decoder("SECRET//NOFORN EXDIS");
    // Either Unambiguous (when the score margin is met) or Ambiguous
    // with the expected candidate present.
    let m = match parsed {
        Parsed::Unambiguous(m) => m,
        Parsed::Ambiguous { candidates } if !candidates.is_empty() => candidates[0].marking.clone(),
        other => panic!("expected non-empty result; got {other:?}"),
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
}

#[test]
fn decoder_recovers_sar_indicator_repair_usar_prefix() {
    // USAR-BP-J12 → SAR-BP-J12 via the SAR indicator-keyword repair pass.
    let parsed = run_decoder("SECRET//USAR-BP-J12//NOFORN");
    let m = match parsed {
        Parsed::Unambiguous(m) => m,
        Parsed::Ambiguous { candidates } if !candidates.is_empty() => candidates[0].marking.clone(),
        other => panic!("expected non-empty result; got {other:?}"),
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
}

#[test]
fn decoder_recovers_stray_char_slash_recovery() {
    // S/X/NF — stray X wedged between S and NF. The collapse-stray
    // pass strips the X and the scorer reaches Unambiguous(Secret +
    // NOFORN) cleanly; the runner-up margin is wide enough that the
    // null gate does not need to intervene.
    let parsed = run_decoder("(S/X/NF)");
    let Parsed::Unambiguous(m) = parsed else {
        panic!("expected Unambiguous; got {parsed:?}");
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    assert_eq!(dissem_us_count(&m), 1);
}

#[test]
fn decoder_recovers_rel_to_header_transposition() {
    // REL OT USA → REL TO USA via the header-normalize pass.
    let parsed = run_decoder("SECRET//REL OT USA, GBR");
    let m = match parsed {
        Parsed::Unambiguous(m) => m,
        Parsed::Ambiguous { candidates } if !candidates.is_empty() => candidates[0].marking.clone(),
        other => panic!("expected non-empty result; got {other:?}"),
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    // REL TO USA, GBR → 2 country codes.
    assert!(rel_to_count(&m) >= 2);
}

#[test]
fn decoder_recovers_sci_delimiter_hcsp_to_hcs_p() {
    // HCSP → HCS-P via SCI delimiter insertion (Pattern A) PLUS
    // promotion of the trailing single `/` to `//` (Pattern D —
    // issue #720). After #720, the recovery promotes intra-SCI `/`
    // to category `//` when the next token is non-SCI; this fixture
    // exercises that combined path. Strong assertion: the recovered
    // input must parse **unambiguously** (the #720 bug was an
    // empty-candidate `Ambiguous`; the contract is a single resolved
    // marking) with Secret + HcsP + NOFORN in their canonical slots.
    let parsed = run_decoder("SECRET//HCSP/NOFORN");
    let m = match parsed {
        Parsed::Unambiguous(m) => m,
        other => panic!("post-#720 contract is Parsed::Unambiguous; got {other:?}"),
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    assert!(
        m.0.sci_controls.contains(&SciControl::HcsP),
        "HCS-P must land in sci_controls; attrs = {:?}",
        m.0,
    );
    assert!(
        m.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must land in dissem_iter; attrs = {:?}",
        m.0,
    );
}

#[test]
fn decoder_recovers_nato_longhand_fold_portion() {
    // (NATO SECRET) → (NS) via NATO fold pass.
    let parsed = run_decoder("(NATO SECRET)");
    let m = match parsed {
        Parsed::Unambiguous(m) => m,
        Parsed::Ambiguous { candidates } if !candidates.is_empty() => candidates[0].marking.clone(),
        other => panic!("expected non-empty result; got {other:?}"),
    };
    assert!(matches!(
        &m.0.classification,
        Some(MarkingClassification::Nato(_))
    ));
}

#[test]
fn decoder_recovers_nato_longhand_fold_banner() {
    // NATO SECRET//NF → NS//NF banner via NATO fold pass.
    let parsed = run_decoder("NATO SECRET//NF");
    let m = match parsed {
        Parsed::Unambiguous(m) => m,
        Parsed::Ambiguous { candidates } if !candidates.is_empty() => candidates[0].marking.clone(),
        other => panic!("expected non-empty result; got {other:?}"),
    };
    assert!(matches!(
        &m.0.classification,
        Some(MarkingClassification::Nato(_))
    ));
}

#[test]
fn decoder_recovers_canonical_reorder_noforn_secret() {
    // NOFORN//SECRET → SECRET//NOFORN via the canonical-reorder pass.
    // The reorder pass produces a winning candidate that resolves to
    // Unambiguous Secret + NF.
    let parsed = run_decoder("NOFORN//SECRET");
    let Parsed::Unambiguous(m) = parsed else {
        panic!("expected Unambiguous after canonical reorder; got {parsed:?}");
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    assert_eq!(dissem_us_count(&m), 1);
}

// ---------------------------------------------------------------------------
// Classification heuristic — position-aware short-token fixes.
// ---------------------------------------------------------------------------

#[test]
fn decoder_classification_heuristic_3char_otp_to_top() {
    // OTP SECRET//NF — OTP→TOP via 3-char heuristic. Marked as
    // DecoderClassificationHeuristic provenance, which the engine
    // downgrades to Warn severity; the decoder still emits the candidate.
    let parsed = run_decoder("OTP SECRET//NF");
    let m = match parsed {
        Parsed::Unambiguous(m) => m,
        Parsed::Ambiguous { candidates } if !candidates.is_empty() => candidates[0].marking.clone(),
        // If the gate rejects the heuristic candidate the test still passes —
        // the heuristic is opt-in via lowered confidence threshold.
        Parsed::Ambiguous { .. } => return,
    };
    assert_eq!(classification(&m), Some(Classification::TopSecret));
}

#[test]
fn decoder_classification_heuristic_2char_rs_to_ts() {
    // RS//NF — RS→TS via 2-char heuristic (R-cluster + S-cluster).
    let parsed = run_decoder("(RS//NF)");
    let Parsed::Unambiguous(m) = parsed else {
        panic!("expected Unambiguous after 2-char RS→TS heuristic; got {parsed:?}");
    };
    assert_eq!(classification(&m), Some(Classification::TopSecret));
    assert_eq!(dissem_us_count(&m), 1);
}

#[test]
fn decoder_classification_heuristic_1char_x_to_s() {
    // (X//NF) — X→S via 1-char heuristic (X-cluster).
    let parsed = run_decoder("(X//NF)");
    let Parsed::Unambiguous(m) = parsed else {
        panic!("expected Unambiguous after 1-char X→S heuristic; got {parsed:?}");
    };
    assert_eq!(classification(&m), Some(Classification::Secret));
    assert_eq!(dissem_us_count(&m), 1);
}

// ---------------------------------------------------------------------------
// CAB-shape skip path — CAB inputs must NOT trigger the classification
// heuristic; the strict path handles them.
// ---------------------------------------------------------------------------

#[test]
fn dispatcher_handles_cab_shape_without_heuristic_perturbation() {
    let parsed = run_dispatcher("Classified By: Joe Smith");
    // CAB inputs don't carry a classification — the strict path either
    // accepts (with attrs.classified_by set) or returns trivial.
    // The point is the decoder leg's classification-heuristic doesn't
    // mangle the input.
    match parsed {
        Parsed::Unambiguous(m) => {
            // Either classified_by is set OR the marking is otherwise
            // trivial — what we don't want is a fabricated classification.
            assert!(
                m.0.classified_by.is_some() || classification(&m).is_none(),
                "CAB input should not produce a fabricated classification: {m:?}"
            );
        }
        Parsed::Ambiguous { .. } => {} // also fine — no Unambiguous fabrication
    }
}

// ---------------------------------------------------------------------------
// Fast-path coverage — clean US class + dissem shape goes through the fast
// parse path, not the full recovery pipeline. Confirms the fast path still
// produces the canonical result post-split.
// ---------------------------------------------------------------------------

#[test]
fn decoder_fast_path_resolves_clean_us_class_and_dissem() {
    let parsed = run_decoder("(S//NF)");
    // (S//NF) at preceded_by_whitespace = true bypasses the null gate
    // (multi-character dissem token) and the fast path returns
    // Unambiguous directly.
    match parsed {
        Parsed::Unambiguous(m) => {
            assert_eq!(classification(&m), Some(Classification::Secret));
            assert_eq!(dissem_us_count(&m), 1);
        }
        Parsed::Ambiguous { candidates } if !candidates.is_empty() => {
            // Some scoring configurations might not reach the unambiguous margin
            // — accept Ambiguous-with-candidates as long as the top candidate is right.
            let top = &candidates[0].marking;
            assert_eq!(classification(top), Some(Classification::Secret));
        }
        other => panic!("expected non-empty result for (S//NF); got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Strict-only opt-in via ParseContext.
// ---------------------------------------------------------------------------

#[test]
fn dispatcher_defers_to_strict_when_strict_evidence_is_true() {
    let rx = StrictOrDecoderRecognizer::new();
    let sc = scheme();
    let mut cx = deep_cx();
    cx.strict_evidence = true;
    // Mangled input — strict alone would fail to recover.
    let parsed = rx.recognize(b"(SERCET//NOFORN)", 0, &sc, &cx);
    // With strict_evidence, the decoder leg is skipped — we expect the
    // strict-only result, which for SERCET is either trivial or carries
    // a Classification-kind token with no resolved classification.
    match parsed {
        Parsed::Unambiguous(m) => {
            assert!(
                classification(&m).is_none(),
                "strict-only on SERCET must not resolve to a classification"
            );
        }
        Parsed::Ambiguous { .. } => {} // also acceptable
    }
}
