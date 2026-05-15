// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Diagnostic harness for the SC-004 accuracy gap (issue #133).
//!
//! Not a gate. Walks a hand-picked set of unresolved fixtures from each
//! mangling class and prints, per fixture, what the decoder pipeline
//! actually did:
//!
//! - the canonicalized byte-string the strict parser saw,
//! - whether strict-parse succeeded,
//! - whether the parsed attrs carry any `TokenKind::Unknown` spans
//!   (which step 3a in `decoder.rs` discards),
//! - whether the parsed attrs match the expected form's attrs.
//!
//! Run with:
//!   cargo test -p marque-engine --features decoder-harness \
//!     --test decoder_diagnostic -- --ignored --nocapture
//!
//! Output is meant to be pasted into issue #133 to localize where in
//! the pipeline each unresolved class fails.

#![cfg(feature = "decoder-harness")]

use marque_capco::CapcoMarking;
use marque_engine::{DecoderRecognizer, StrictRecognizer};
use marque_ism::{CapcoTokenSet, TokenKind, token_set::TokenSet as _};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};

fn deep_cx() -> ParseContext {
    ParseContext {
        strict_evidence: false,
        zone: None,
        position: None,
        classification_floor: None,
        as_of: None,
        preceded_by_whitespace: true,
    }
}

fn same_meaning(a: &marque_ism::CanonicalAttrs, b: &marque_ism::CanonicalAttrs) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.token_spans = Box::new([]);
    b.token_spans = Box::new([]);
    a == b
}

/// Strict-parse `input` bytes via the strict recognizer. Returns
/// `None` when the recognizer collapses to `Parsed::Ambiguous` (the
/// strict path's parse-failure signal). Operates on `&[u8]` so the
/// diagnostic can pass decoder canonical-attempt bytes through
/// without an intermediate UTF-8 round-trip — matches what the
/// decoder itself does internally.
fn parse_strict_attrs(input: &[u8]) -> Option<CapcoMarking> {
    let strict = StrictRecognizer::new();
    match strict.recognize(input, &deep_cx()) {
        Parsed::Unambiguous(m) => Some(m),
        _ => None,
    }
}

fn token_summary(attrs: &marque_ism::CanonicalAttrs) -> String {
    let mut counts = std::collections::BTreeMap::new();
    for span in attrs.token_spans.iter() {
        *counts.entry(format!("{:?}", span.kind)).or_insert(0) += 1;
    }
    counts
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn unknown_token_text(input: &[u8], attrs: &marque_ism::CanonicalAttrs) -> Vec<String> {
    attrs
        .token_spans
        .iter()
        .filter(|s| matches!(s.kind, TokenKind::Unknown))
        .filter_map(|s| {
            s.span
                .try_as_slice(input)
                .and_then(|b| std::str::from_utf8(b).ok())
                .map(|t| t.to_string())
        })
        .collect()
}

fn trace_one(label: &str, observed: &str, expected: &str) {
    println!("──────────────────────────────────────────────────────────────────");
    println!("[{label}]");
    println!("  observed: {observed:?}");
    println!("  expected: {expected:?}");

    // 1. Ask the decoder for the canonicalized byte attempts it would
    //    hand to the strict parser. Calling the real generator (rather
    //    than re-implementing token-walking + fuzzy correction here)
    //    means the trace can never drift from the decoder's actual
    //    behavior — every transform `generate_candidate_bytes` applies
    //    (delimiter normalization, fuzzy correction, superseded-token
    //    replacement, reorder) shows up below.
    let attempts = marque_engine::decoder::diagnostic_canonical_attempts(observed.as_bytes());
    if attempts.is_empty() {
        println!("  decoder canonical attempts: (none)");
    } else {
        println!("  decoder canonical attempts ({}):", attempts.len());
        for (i, attempt) in attempts.iter().enumerate() {
            // Attempts come from `generate_candidate_bytes` →
            // `String::into_bytes()`, so they are valid UTF-8 by
            // construction. Asserting the invariant via `expect`
            // surfaces a Phase D contract break loudly instead of
            // silently substituting a placeholder string and feeding
            // it to the strict parser later.
            let attempt_str = std::str::from_utf8(attempt)
                .expect("decoder canonical attempt must be valid UTF-8 (built from String)");
            println!("    [{i}] {attempt_str:?}");
        }
    }

    // 2. Strict-parse each attempt — this is the same path
    //    `DecoderRecognizer::recognize` step 2 takes, including the
    //    step-3a Unknown-token-span check that discards partial
    //    canonicalizations.
    for (i, attempt) in attempts.iter().enumerate() {
        match parse_strict_attrs(attempt) {
            Some(m) => {
                let unknown_tokens = unknown_token_text(attempt, &m.0);
                println!(
                    "    [{i}] strict-parse: OK — token_kinds={{{}}}",
                    token_summary(&m.0)
                );
                if !unknown_tokens.is_empty() {
                    println!("        Unknown spans: {unknown_tokens:?}");
                    println!(
                        "        → REJECTED by decoder step 3a \
                         (`has_unknown_token`)"
                    );
                }
            }
            None => println!("    [{i}] strict-parse: FAILED"),
        }
    }

    // 3. Strict-parse the expected form for ground-truth attrs.
    let expected_marking = parse_strict_attrs(expected.as_bytes());
    let expected_attrs = match &expected_marking {
        Some(m) => Some(&m.0),
        None => {
            println!("  strict-parse(expected): FAILED");
            None
        }
    };

    // 4. Run the actual decoder to see what it produces end-to-end.
    let decoder = DecoderRecognizer::new();
    let result = decoder.recognize(observed.as_bytes(), &deep_cx());
    match result {
        Parsed::Unambiguous(m) => {
            let r =
                m.1.as_ref()
                    .expect("decoder returned Parsed::Unambiguous without provenance")
                    .recognition_score();
            println!("  decoder verdict: Unambiguous (recognition={r:.3})");
            if let Some(exp) = expected_attrs {
                let attrs_match = same_meaning(&m.0, exp);
                if attrs_match {
                    println!("    attrs match expected: ✓ resolved");
                } else {
                    println!(
                        "    attrs differ from expected:\n      decoded={:?}\n      expected={:?}",
                        attrs_summary(&m.0),
                        attrs_summary(exp),
                    );
                }
            }
        }
        Parsed::Ambiguous { candidates } => {
            println!(
                "  decoder verdict: Ambiguous (candidates={})",
                candidates.len()
            );
            for (i, c) in candidates.iter().take(3).enumerate() {
                println!(
                    "    [{i}] prior={:.3}, evidence={:?}, attrs_summary={:?}",
                    c.prior_log_odds,
                    c.evidence
                        .iter()
                        .map(|e| (e.label, e.log_odds))
                        .collect::<Vec<_>>(),
                    attrs_summary(&c.marking.0)
                );
            }
        }
    }
}

fn attrs_summary(attrs: &marque_ism::CanonicalAttrs) -> String {
    format!(
        "cls={:?} sci={} sar={} dissem={} rel_to={} declass={:?}",
        attrs.classification,
        attrs.sci_markings.len(),
        attrs.sar_markings.is_some() as usize,
        attrs.dissem_iter().count(),
        attrs.rel_to.len(),
        attrs.declassify_on,
    )
}

/// Hand-picked unresolved samples from each mangling class. One per
/// observed pipeline failure mode so issue #133 can localize each
/// independently.
const SAMPLES: &[(&str, &str, &str)] = &[
    // Typo class — NOFORON: fuzzy-correct edit-distance-1 insertion.
    // Should produce a candidate. Harness reported "zero-candidate".
    (
        "Typo / NOFORON",
        "SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORON",
        "SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN",
    ),
    // Typo class — UK vs TK. Both 2-char tokens; UK is a country
    // trigraph; TK is an SCI control. Decoder's MIN_FUZZY_LEN=3 means
    // 2-char tokens are never fuzzy-corrected.
    (
        "Typo / UK→TK",
        "TOP SECRET//SI/UK//NOFORN",
        "TOP SECRET//SI/TK//NOFORN",
    ),
    // Typo class — TPP→TOP. The mangled form is the first word of a
    // two-word classification. Fuzzy-correctable in isolation but the
    // strict parser sees `TPP SECRET` not as classification.
    (
        "Typo / TPP",
        "TPP SECRET//SI//NOFORN",
        "TOP SECRET//SI//NOFORN",
    ),
    // Typo class — USAR→SAR (extra leading char on a SAR program
    // prefix). `scan_token` keeps `USAR-BP-J12` as a single compound
    // token; fuzzy can't see the SAR prefix in isolation.
    (
        "Typo / USAR",
        "SECRET//USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN",
        "SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN",
    ),
    // MissingDelimiter / dissem run-together. Whole class fails (0/17)
    // because generate_candidate_bytes never inserts `//`.
    (
        "MissingDelimiter / NOFORN EXDIS",
        "SECRET//NOFORN EXDIS",
        "SECRET//NOFORN//EXDIS",
    ),
    // MissingDelimiter — boundary between SCI block and dissem.
    (
        "MissingDelimiter / SI/TK NOFORN",
        "TOP SECRET//SI/TK NOFORN",
        "TOP SECRET//SI/TK//NOFORN",
    ),
    // MissingDelimiter — header-style classification + SCI without `//`.
    (
        "MissingDelimiter / TS HCS-P",
        "TOP SECRET HCS-P INTEL OPS//ORCON/NOFORN",
        "TOP SECRET//HCS-P INTEL OPS//ORCON/NOFORN",
    ),
];

#[test]
#[ignore]
fn trace_unresolved_samples() {
    println!();
    println!("Decoder diagnostic — issue #133 follow-up data");
    println!("==============================================");
    for (label, observed, expected) in SAMPLES {
        trace_one(label, observed, expected);
    }
    println!("──────────────────────────────────────────────────────────────────");
}

#[test]
#[ignore]
fn probe_fuzzy_for_specific_tokens() {
    let token_set = CapcoTokenSet;
    let vocab = token_set.correction_vocab();
    let matcher = marque_core::fuzzy::FuzzyVocabMatcher::new(vocab);
    println!();
    println!("Direct fuzzy probes (issue #133)");
    println!("================================");
    for tok in [
        "NOFORON",
        "NOFRON",
        "TPP",
        "USAR",
        "SERCET",
        "CONFIDETIAL",
        "ORCON",
    ] {
        let result = matcher.correct(tok);
        match result {
            Some(c) => println!(
                "  {tok:?} → {:?} (distance={}, confidence={:.3})",
                c.token, c.distance, c.confidence
            ),
            None => {
                // `FuzzyVocabMatcher::correct` returns None for four
                // documented reasons. Branch on the structural ones
                // (already-canonical, below MIN_FUZZY_LEN) so the
                // probe can't mislead by treating a known-vocab
                // entry as "no candidate." For the remaining cases
                // dump the closest 5 vocab entries with their
                // Levenshtein distances and label whether None means
                // "no candidate within MAX_EDIT_DISTANCE" vs "two or
                // more vocab entries tied at the closest distance
                // (ambiguous)."
                if vocab.binary_search(&tok).is_ok() {
                    println!("  {tok:?} → None — already canonical (no correction needed)");
                    continue;
                }
                if tok.len() < marque_core::fuzzy::MIN_FUZZY_LEN {
                    println!(
                        "  {tok:?} → None — below MIN_FUZZY_LEN={} \
                         (single-/double-char tokens are too noisy for \
                         edit-distance correction)",
                        marque_core::fuzzy::MIN_FUZZY_LEN
                    );
                    continue;
                }
                let mut dists: Vec<(u8, &str)> = vocab
                    .iter()
                    .map(|&v| {
                        let d = naive_distance(tok, v);
                        (d, v)
                    })
                    .collect();
                dists.sort_by_key(|(d, _)| *d);
                let best = dists.first().map(|(d, _)| *d).unwrap_or(u8::MAX);
                let tied_at_best = dists.iter().filter(|(d, _)| *d == best).count();
                let max_dist = marque_core::fuzzy::MAX_EDIT_DISTANCE;
                let reason = if best > max_dist {
                    format!("no candidate within MAX_EDIT_DISTANCE={max_dist}")
                } else if tied_at_best > 1 {
                    format!("ambiguous — {tied_at_best} candidates tied at distance {best}")
                } else {
                    format!(
                        "candidate at distance {best} but matcher rejected \
                         (likely below MIN_USEFUL_CONFIDENCE)"
                    )
                };
                println!(
                    "  {tok:?} → None — {reason}; closest 5: {:?}",
                    &dists.iter().take(5).collect::<Vec<_>>()
                );
            }
        }
    }
}

fn naive_distance(a: &str, b: &str) -> u8 {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let m = a.len();
    let n = b.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n].min(255) as u8
}
