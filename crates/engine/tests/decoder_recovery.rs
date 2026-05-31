// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decoder recovery tests.
//!
//! These tests exercise the three mangling classes the decoder targets:
//!
//! - Typo-to-canonical (`SERCET//NOFORN` → `SECRET//NOFORN`).
//! - Banner reordering (`NOFORN//SECRET` → `SECRET//NOFORN`).
//! - Strict-context classification floor: ambiguous `(C)` decodes to
//!   CONFIDENTIAL only when a strict CONFIDENTIAL-or-higher floor is
//!   established for the page.
//!
//! Tests hit `DecoderRecognizer` directly (not through `Engine::lint`)
//! because they cover the recognizer in isolation.

use std::sync::LazyLock;

use marque_capco::CapcoScheme;
use marque_engine::DecoderRecognizer;
use marque_ism::{Classification, DissemControl, NonIcDissem, SciControl};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};

fn deep_cx() -> ParseContext {
    // `ParseContext` is `#[non_exhaustive]` (#176 staging step 1), so
    // it is built via `default()` + field assignment.
    let mut cx = ParseContext::default();
    cx.strict_evidence = false;
    cx.preceded_by_whitespace = true;
    cx
}

/// Shared scheme instance for the test module. `CapcoScheme::new()`
/// builds non-trivial `Vec` tables; borrowing `&*TEST_SCHEME` avoids
/// repeated allocation across the many decoder recovery tests.
static TEST_SCHEME: LazyLock<CapcoScheme> = LazyLock::new(CapcoScheme::new);

fn effective_level(m: &marque_capco::CapcoMarking) -> Option<Classification> {
    m.0.classification.as_ref().map(|c| c.effective_level())
}

// ---------------------------------------------------------------------------
// Typo-to-canonical
// ---------------------------------------------------------------------------

#[test]
fn sercet_decodes_to_secret_via_edit_distance_one() {
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SERCET//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
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
    let dissem_count = marking.0.dissem_iter().count();
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
    // `RS` (delete E) AND `RSEN` (insert N), both of which are in
    // the extended correction vocab after the issue #133 long-form
    // dissem fix. The matcher returns `None` on the two-way tie,
    // the unknown token passes through to strict parse as
    // `TokenKind::Unknown`, and the decoder's step-3a Unknown-span
    // filter rejects the partial candidate.
    //
    // Honesty invariant: when any token is unresolvable, the decoder
    // surfaces zero candidates rather than fabricating a marking that
    // silently drops the unresolved token.
    //
    // The companion
    // `partial_canonicalization_with_unresolvable_token_returns_zero_candidate`
    // covers the distinct "uncorrectable / no candidate close
    // enough" path (e.g., `SECRET//WIBBLE`).
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"SECRET//RSE", 0, &*TEST_SCHEME, &deep_cx()) {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "decoder must not fabricate partial candidates when any \
             token is fuzzy-ambiguous (honesty invariant); \
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
    // Regression guard (PR #114): the decoder must NOT produce a
    // partial candidate when a token is un-correctable.
    // `SECRET//WIBBLE` is the pathological case — classification
    // fine, tail token uncorrectable. Without the Unknown-span filter the decoder
    // would have emitted a `SECRET` candidate silently dropping
    // WIBBLE. With the filter in place the candidate is dropped.
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"SECRET//WIBBLE", 0, &*TEST_SCHEME, &deep_cx()) {
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
// Banner reordering
// ---------------------------------------------------------------------------

#[test]
fn dissem_first_banner_decodes_to_canonical_order() {
    // Canonical order is classification → SCI → SAR → dissem. The
    // decoder's reorder pass should swap dissem-first input.
    let rx = DecoderRecognizer::new();
    match rx.recognize(b"NOFORN//SECRET", 0, &*TEST_SCHEME, &deep_cx()) {
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
// Strict-context classification floor
// ---------------------------------------------------------------------------

#[test]
fn unclassified_candidate_rejected_below_secret_floor() {
    // `UNCLASSIFIED` (banner form) decodes to UNCLASSIFIED when no
    // floor is set. With a SECRET floor, the candidate is below the
    // floor and must be dropped — decoder returns zero-candidate
    // Ambiguous.
    //
    // Issue #258: pre-#258 this used `(U)` (portion form), but
    // single-letter portions are now suppressed by the prose null
    // hypothesis (`U` has high prose frequency). Switch to the
    // banner form so the underlying floor predicate is what gets
    // tested, not the dispatch interaction with the null hypothesis.
    let rx = DecoderRecognizer::new();
    // `ParseContext` is `#[non_exhaustive]` (#176 staging step 1).
    let mut floored = deep_cx();
    floored.rank_floor = Some(Classification::Secret as u8);
    match rx.recognize(b"UNCLASSIFIED", 0, &*TEST_SCHEME, &floored) {
        Parsed::Ambiguous { candidates } => assert!(
            candidates.is_empty(),
            "UNCLASSIFIED below SECRET floor must zero-out candidates, got {}",
            candidates.len()
        ),
        Parsed::Unambiguous(m) => panic!("expected zero-candidate, got {:?}", effective_level(&m)),
    }
}

#[test]
fn floor_at_equal_level_accepts_candidate() {
    // `SECRET` (banner form) with a SECRET floor passes — equal
    // clears the floor. Issue #258: pre-#258 this used `(S)`
    // (portion form), but single-letter portions now lose to the
    // prose null hypothesis. The unit under test is the floor
    // predicate, not the portion-vs-banner dispatch.
    let rx = DecoderRecognizer::new();
    // `ParseContext` is `#[non_exhaustive]` (#176 staging step 1).
    let mut floored = deep_cx();
    floored.rank_floor = Some(Classification::Secret as u8);
    match rx.recognize(b"SECRET", 0, &*TEST_SCHEME, &floored) {
        Parsed::Unambiguous(marking) => {
            assert_eq!(effective_level(&marking), Some(Classification::Secret));
        }
        other => panic!("SECRET at SECRET floor should decode unambiguously, got {other:?}"),
    }
}

#[test]
fn floor_below_candidate_accepts_higher_level() {
    // `TOP SECRET` (banner form) with a CONFIDENTIAL floor passes —
    // TopSecret exceeds Confidential. Issue #258: pre-#258 this used
    // `(TS)` (portion form); the banner form is more discriminative
    // against the prose null hypothesis.
    let rx = DecoderRecognizer::new();
    // `ParseContext` is `#[non_exhaustive]` (#176 staging step 1).
    let mut floored = deep_cx();
    floored.rank_floor = Some(Classification::Confidential as u8);
    match rx.recognize(b"TOP SECRET", 0, &*TEST_SCHEME, &floored) {
        Parsed::Unambiguous(marking) => {
            assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
        }
        other => panic!("TOP SECRET above CONFIDENTIAL floor should decode, got {other:?}"),
    }
}

#[test]
fn no_floor_accepts_any_classification() {
    // With `rank_floor: None` the floor is inactive —
    // any classification passes through. Issue #258: pre-#258 this
    // used portion forms (`(U)`, `(C)`, `(S)`, `(TS)`); the banner
    // forms are more discriminative against the prose null
    // hypothesis (the full words are exceedingly rare in prose) so
    // each input still decodes unambiguously.
    let rx = DecoderRecognizer::new();
    for (input, expected) in [
        (b"UNCLASSIFIED".as_slice(), Classification::Unclassified),
        (b"CONFIDENTIAL".as_slice(), Classification::Confidential),
        (b"SECRET".as_slice(), Classification::Secret),
        (b"TOP SECRET".as_slice(), Classification::TopSecret),
    ] {
        match rx.recognize(input, 0, &*TEST_SCHEME, &deep_cx()) {
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
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"secret//noforn", 0, &*TEST_SCHEME, &deep_cx())
    else {
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
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TOP SECRET //NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
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
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TOP SECRET//COMINT//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
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
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "COMINT supersession must preserve NOFORN; attrs = {:?}",
        marking.0,
    );
}

/// **MissingDelimiter** class — recovery test (issue #133).
///
/// `SECRET//NOFORN EXDIS` (missing `//` before `EXDIS`) is the
/// canonical MissingDelimiter shape. The decoder's
/// `try_insert_delimiter` helper inserts `//` at category-transition
/// whitespace gaps before unambiguous segment-starting dissem
/// long-forms (`NOFORN`, `EXDIS`, `ORCON`, …); the result strict-
/// parses as `SECRET//NOFORN//EXDIS` with both dissems landing in
/// the right slots (`Nf` → `dissem_controls`, `Exdis` →
/// `non_ic_dissem`).
#[test]
fn missing_delimiter_secret_noforn_exdis_resolves() {
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET//NOFORN EXDIS", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "SECRET//NOFORN EXDIS must resolve unambiguously with \
             missing-delimiter insertion (issue #133)"
        );
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Secret),
        "classification must survive the delimiter insertion; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must land in dissem_controls; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis),
        "EXDIS must land in non_ic_dissem after `//` is inserted before \
         it (issue #133 missing-delimiter insertion); attrs = {:?}",
        marking.0,
    );
}

// ---------------------------------------------------------------------------
// Missing-delimiter insertion (issue #133)
// ---------------------------------------------------------------------------
//
// `try_insert_delimiter` walks the input and inserts `//` at
// category-transition whitespace gaps. Two rules drive insertion:
//
//   1. Classification → next segment: the first non-classification
//      token after the classification phrase, when no `//` has been
//      emitted yet, gets `//` prepended.
//   2. Hard-splitter dissem long-form: a small set of unambiguous
//      tokens (`NOFORN`, `EXDIS`, `ORCON`, `PROPIN`, `IMCON`,
//      `RELIDO`, `RSEN`, `EYESONLY`, `FOUO`, `FISA`, `DSEN`,
//      `NODIS`, `LIMDIS`, `ORCON-USGOV`) ALWAYS start a new segment
//      when they appear after whitespace.
//
// Exception: `SBU NOFORN` / `LES NOFORN` are non-IC dissem banner
// long forms (`SbuNf` / `LesNf`); the helper does not split between
// `SBU`/`LES` and a following `NOFORN`.
//
// The helper does NOT handle SCI starters (`SI`, `HCS`, `TK`), SAR
// prefixes (`SAR-*`), or `SPECIAL ACCESS REQUIRED-*` — those need
// classification-context lookahead. The hard-splitter +
// classification-boundary rules cover ~15 of 17 MissingDelimiter
// fixtures; the SCI/SAR/SPECIAL family covers the remaining ~2.

#[test]
fn missing_delimiter_classification_then_rel_to() {
    // `SECRET REL TO USA, AUS, GBR` — REL TO is a category-starter
    // after classification with a missing `//`. The classification-
    // boundary rule (Rule 1) inserts `//` between SECRET and REL,
    // producing `SECRET//REL TO USA, AUS, GBR` which strict-parses
    // to a SECRET marking with USA/AUS/GBR in `rel_to`.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET REL TO USA, AUS, GBR", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("SECRET REL TO USA, AUS, GBR should resolve via delimiter insertion");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Secret),
        "classification must survive; attrs = {:?}",
        marking.0,
    );
    assert!(
        !marking.0.rel_to.is_empty(),
        "REL TO list must populate after `//` is inserted; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn missing_delimiter_top_secret_classification_then_dissem() {
    // `TOP SECRET//HCS-P INTEL OPS ORCON/NOFORN` — gap is between
    // OPS (SCI sub-compartment) and ORCON (dissem long-form).
    // Hard-splitter rule (Rule 2) fires on ORCON.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"TOP SECRET//HCS-P INTEL OPS ORCON/NOFORN",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("HCS-P INTEL OPS ORCON/NOFORN should resolve");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::TopSecret),
        "TOP SECRET must survive; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Oc)
            && marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "ORCON and NOFORN must both land in dissem_controls (the \
         original `ORCON/NOFORN` block contains both); attrs = {:?}",
        marking.0,
    );
}

#[test]
fn missing_delimiter_two_dissems() {
    // `SECRET NOFORN//EXDIS` — gap between SECRET and NOFORN is
    // covered by the classification-boundary rule (Rule 1); also
    // by the hard-splitter rule on NOFORN.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET NOFORN//EXDIS", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("SECRET NOFORN//EXDIS should resolve");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret),);
    assert!(marking.0.dissem_iter().any(|d| d == &DissemControl::Nf));
    assert!(marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis));
}

#[test]
fn missing_delimiter_hard_splitter_inside_segment() {
    // `TOP SECRET//SI/TK NOFORN` — NOFORN follows whitespace inside
    // an SCI segment. Hard-splitter rule fires on NOFORN.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TOP SECRET//SI/TK NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("TOP SECRET//SI/TK NOFORN should resolve");
    };
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must land in dissem_controls; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.sci_controls.contains(&SciControl::Si)
            && marking.0.sci_controls.contains(&SciControl::Tk),
        "SI/TK must land in sci_controls as both SI and TK (the \
         original `SI/TK` block contains both); attrs = {:?}",
        marking.0,
    );
}

#[test]
fn missing_delimiter_does_not_split_sbu_noforn() {
    // `SECRET//SBU NOFORN` — SBU NOFORN is the non-IC dissem
    // **banner long form** for `NonIcDissem::SbuNf`. The helper
    // must NOT split between SBU and NOFORN. The strict parser
    // accepts `SBU NOFORN` as a single non-IC dissem entry via
    // `parse_non_ic_full_form`.
    //
    // The assertion requires `Parsed::Unambiguous` with `SbuNf` in
    // `non_ic_dissem`. An earlier `if let Parsed::Unambiguous(..)`
    // shape silently passed when the recognizer returned
    // `Parsed::Ambiguous` — defeating the regression guard. If the
    // helper ever incorrectly splits SBU and NOFORN, SBU on its
    // own doesn't resolve, the candidate is discarded by step 3a,
    // and the recognizer returns zero-candidate Ambiguous —
    // exactly the case the previous shape silently allowed.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET//SBU NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("SECRET//SBU NOFORN must resolve unambiguously");
    };
    assert!(
        marking.0.non_ic_dissem.contains(&NonIcDissem::SbuNf),
        "SBU NOFORN must land as a single non-IC dissem entry \
         (`NonIcDissem::SbuNf`), not split into separate tokens; \
         attrs = {:?}",
        marking.0,
    );
}

#[test]
fn missing_delimiter_sar_block_with_trailing_noforn_resolves() {
    // `SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB NOFORN`
    // (issue #133). The SAR grammar accepts any alphanumeric
    // identifier, so the strict parser cleanly absorbs `NOFORN` as
    // the trailing sub-compartment of the `XR-XRA` compartment when
    // no `//` separator precedes it. The competing delim-inserted
    // candidate puts `NOFORN` into `dissem_controls` instead — that's
    // the canonical interpretation. A naive bag-of-tokens scorer
    // would reward the absorbing parse (fewer scored tokens → higher
    // posterior because log-priors are negative);
    // `HARD_SPLITTER_ABSORPTION_PENALTY` flips the contest.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB NOFORN",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("SAR with trailing NOFORN must resolve unambiguously");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(
        marking.0.sar_markings.is_some(),
        "SAR block must be present; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must land in dissem_controls (delim-inserted candidate \
         beats the absorbing one via HARD_SPLITTER_ABSORPTION_PENALTY); \
         attrs = {:?}",
        marking.0,
    );
}

#[test]
fn missing_delimiter_full_sar_with_trailing_noforn_resolves() {
    // `TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN`
    // (issue #133). Same scoring problem as the abbreviated SAR
    // shape above, but here `NOFORN` gets absorbed as the trailing
    // word of the multi-word `Full`-indicator program nickname
    // (`identifier: "BUTTER POPCORN NOFORN"`). The `Full` shape needs
    // the per-word check inside `contains_hard_splitter_word` to fire.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`Full`-indicator SAR with trailing NOFORN must resolve");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
    let sar = marking
        .0
        .sar_markings
        .as_ref()
        .expect("SAR block must be present");
    assert_eq!(sar.programs.len(), 1, "exactly one program; got {sar:?}");
    assert_eq!(
        &*sar.programs[0].identifier, "BUTTER POPCORN",
        "program identifier must be the clean nickname (no NOFORN absorbed); got {sar:?}",
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must land in dissem_controls; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn missing_delimiter_no_change_on_already_canonical() {
    // `SECRET//NOFORN//EXDIS` is fully canonical — the helper
    // must not insert anything (would produce a no-op candidate
    // that would dedup against the fuzzy candidate but is still
    // wasted work). Because the helper returns `Option<String>`
    // (None when no insertions), this test exercises the early-
    // return path through the decoder's normal recovery on a
    // clean input.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET//NOFORN//EXDIS", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("SECRET//NOFORN//EXDIS should resolve directly");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(marking.0.dissem_iter().any(|d| d == &DissemControl::Nf));
    assert!(marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis));
}

// ---------------------------------------------------------------------------
// SAR indicator-keyword structural repair (issue #133)
// ---------------------------------------------------------------------------
//
// Three structural recovery paths:
//   1. `[A-Z]{1,3}SAR-` → `SAR-` prefix strip (USAR-BP, ABSAR-BP, …).
//   2. `SAR[A-Z0-9]{2,3}<delim>` → `SAR-<rest><delim>` missing-hyphen
//      insertion (SARBP, SARABC).
//   3. `SPECIAL`, `ACCESS` added to `EXTENDED_CORRECTION_VOCAB` so
//      the fuzzy matcher can recover SPCIAL/SEPCIAL/SPECAL/CCESS-
//      style keyword typos via the existing per-token correction
//      path.
//
// The structural penalty `CUSTOM_SCI_MARKING_PENALTY` in
// `score_candidate` is a companion: without it, a raw `USAR-BP-J12`
// segment is interpreted by the lenient strict-parser as 3 custom-
// system SCI markings (USAR/CD/XR with `canonical_enum: None`), and
// the bag-of-tokens scorer can't distinguish that interpretation
// from the SAR-repaired candidate. The penalty demotes the
// custom-only-SCI parse so the SAR-repaired candidate wins by a
// margin clearing `UNAMBIGUOUS_LOG_MARGIN`.
//
// All three integration tests below pin a named fixture from the
// mangled corpus so the recovery shapes are anchored to specific
// inputs rather than an opaque aggregate.

#[test]
fn typo_usar_prefix_resolves_via_indicator_repair() {
    // Pinned fixture: `tests/fixtures/mangled/typo/d04f45f7a4f5a8b4.json`
    // (`SECRET//USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN`).
    // The full Enron-corpus SAR shape with a stray `U` prefix on the
    // SAR indicator. Without recovery this resolves as 3 custom-system
    // SCI markings (lenient strict parse) competing with the
    // SAR-repaired candidate at a tied posterior; both
    // `try_sar_indicator_repair` (which adds the SAR candidate) and
    // `CUSTOM_SCI_MARKING_PENALTY` (which demotes the custom-SCI
    // candidate) are needed to clear the `UNAMBIGUOUS_LOG_MARGIN`
    // threshold.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("USAR-BP-... must resolve via SAR indicator repair");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    let sar = marking
        .0
        .sar_markings
        .as_ref()
        .expect("SAR block must be present after USAR→SAR repair");
    assert_eq!(
        sar.programs.len(),
        3,
        "expected 3 programs (BP, CD, XR); got {sar:?}"
    );
    assert_eq!(&*sar.programs[0].identifier, "BP");
    assert_eq!(&*sar.programs[1].identifier, "CD");
    assert_eq!(&*sar.programs[2].identifier, "XR");
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_sarbp_missing_hyphen_resolves_via_indicator_repair() {
    // Pinned fixture: `tests/fixtures/mangled/typo/fbf5ed813c109c14.json`
    // (`TOP SECRET//SARBP//NOFORN`). The minimal missing-hyphen
    // case — `SARBP` is 5 alnum chars (SAR + 2-char identifier)
    // with no separator. `try_sar_indicator_repair`'s Pattern B
    // (alnum run 2-3 chars before delim) fires and inserts the
    // hyphen.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TOP SECRET//SARBP//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("SARBP must resolve via SAR indicator repair");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
    let sar = marking
        .0
        .sar_markings
        .as_ref()
        .expect("SAR block must be present after SARBP→SAR-BP repair");
    assert_eq!(sar.programs.len(), 1);
    assert_eq!(&*sar.programs[0].identifier, "BP");
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_spcial_keyword_resolves_via_extended_correction_vocab() {
    // Pinned fixture: `tests/fixtures/mangled/typo/1f75ddd89b432949.json`
    // (`TOP SECRET//SPCIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN`).
    // `SPCIAL` is a missing-`E` typo on `SPECIAL` (edit distance 1).
    // `SPECIAL` lives in `SAR_STRUCTURAL_KEYWORDS`, which lets the
    // per-token fuzzy matcher recover it; the strict SAR parser then matches the
    // canonical `SPECIAL ACCESS REQUIRED-BUTTER POPCORN` indicator
    // literally. No structural-repair pass is involved — this case
    // exercises the vocab path only.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"TOP SECRET//SPCIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("SPCIAL must fuzzy-correct to SPECIAL via extended vocab");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
    let sar = marking
        .0
        .sar_markings
        .as_ref()
        .expect("SAR block must be present after SPCIAL→SPECIAL fuzzy");
    assert_eq!(sar.programs.len(), 1);
    assert_eq!(
        &*sar.programs[0].identifier, "BUTTER POPCORN",
        "Full-form program identifier must round-trip; got {sar:?}",
    );
    assert!(marking.0.dissem_iter().any(|d| d == &DissemControl::Nf));
}

// ---------------------------------------------------------------------------
// Issue #133: stray-character `/X/` recovery
// ---------------------------------------------------------------------------
//
// The `try_collapse_stray_char_slash` pass walks the fuzzy-corrected
// text looking for the `<alnum>/<single_alnum_char>/<alnum>` pattern
// and emits three candidate transforms (drop X, right-attach X to
// next token, left-attach X to previous token). Step 3a's
// `TokenKind::Unknown` filter is the natural disambiguator: only the
// transform that produces fully-recognized tokens survives.
//
// `MIN_FUZZY_LEN` deliberately stays at 3 rather than 2: lowering it
// to recover `UK→TK`-style 2-char tail typos corrupts SAR
// sub-compartments, because the canonical Enron-corpus SAR fixture
// has `RB` as a standalone 2-char sub-compartment token and `RB` is
// at edit distance 1 from `RS` (the RSEN portion form) — so 2-char
// fuzzy silently turns SAR sub-compartments into dissem controls.
// The `MIN_FUZZY_LEN` doc in `crates/core/src/fuzzy.rs` carries the
// full rationale.
//
// Three integration tests below pin a named fixture from the mangled
// corpus per recovery branch (drop / right-attach / left-attach), so
// the recovery shapes are anchored to specific inputs rather than an
// opaque aggregate.

#[test]
fn typo_drop_stray_r_resolves_via_collapse_stray_char_slash() {
    // Pinned fixture: `tests/fixtures/mangled/typo/7885156a2c2c125f.json`
    // (`SECRET//NOFORN/R/EXDIS`). The drop-X transform produces
    // canonical `SECRET//NOFORN//EXDIS`; the right-attach
    // (`...//REXDIS`) and left-attach (`...//NOFORNR//EXDIS`)
    // candidates contain Unknown tokens and are filtered by step 3a.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET//NOFORN/R/EXDIS", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("`/R/` between NOFORN and EXDIS must resolve via drop-X");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis),
        "EXDIS must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_right_attach_n_resolves_via_collapse_stray_char_slash() {
    // Pinned fixture: `tests/fixtures/mangled/typo/2cb13fe4682ff31c.json`
    // (`TOP SECRET//SI/N/OFORN`). The right-attach transform
    // re-attaches the stray `N` to the leading position of `OFORN`,
    // producing canonical `TOP SECRET//SI//NOFORN`. The drop
    // (`...//SI//OFORN` — OFORN unknown) and left-attach
    // (`...//SIN//OFORN` — both unknown) candidates are filtered
    // by step 3a.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TOP SECRET//SI/N/OFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("`/N/` before OFORN must resolve via right-attach");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
    assert!(
        marking
            .0
            .sci_controls
            .iter()
            .any(|c| matches!(c, SciControl::Si)),
        "SI must survive; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must be reconstructed from N + OFORN; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_left_attach_t_resolves_via_collapse_stray_char_slash() {
    // Pinned fixture: `tests/fixtures/mangled/typo/cff1d0ac74e901c3.json`
    // (`SECRE/T/REL TO USA, AUS, GBR`). The left-attach transform
    // re-attaches the stray `T` to the trailing position of `SECRE`,
    // producing canonical `SECRET//REL TO USA, AUS, GBR`. The drop
    // (`SECRE//REL TO ...` — SECRE unknown classification) and
    // right-attach (`SECRE//TREL TO ...` — both unknown) candidates
    // are filtered by step 3a / 3e (Portion/Banner without
    // classification).
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRE/T/REL TO USA, AUS, GBR",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`/T/` after SECRE must resolve via left-attach");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert_eq!(
        marking.0.rel_to.len(),
        3,
        "REL TO must carry 3 trigraphs (USA, AUS, GBR); attrs = {:?}",
        marking.0,
    );
}

// ---------------------------------------------------------------------------
// 3-char classification typo recovery (issue #133)
// ---------------------------------------------------------------------------
//
// Two complementary recovery paths:
//
// 1. **Bare `TOP` added to `EXTENDED_CORRECTION_VOCAB`.** The CVE
//    schema only lists the multi-word `TOP SECRET` entry, so without
//    bare `TOP` the fuzzy matcher had no target for `TPP→TOP`-style
//    typos at the leading classification slot. With `TOP` in vocab
//    the standard distance-1 fuzzy path recovers `TPP`, `UOP`, plus
//    4-char one-extra-letter cases (`TDOP`, `QTOP`, `TOPW`) and the
//    `TOPS ECRET` token-boundary case (`TOPS`→`TOP` at distance 1
//    via the length-diff filter, then strict parser re-joins
//    `TOP SECRET`).
//
// 2. **3-char heuristic for `OTP`→`TOP`** plus 2-char heuristic
//    extension for `TP`/`TO`→`TOP`. `OTP` is dist 2 from `TOP`
//    under standard Levenshtein (T↔O transposition counts as 2
//    substitutions), and the fuzzy matcher's
//    `MIN_USEFUL_CONFIDENCE` floor (0.45) blocks distance-2
//    corrections for 3-char inputs (confidence 0.40). `TP`/`TO`
//    are 2-char and below `MIN_FUZZY_LEN`. Both paths take a
//    targeted heuristic at the leading classification slot.
//
// `is_canonical_short_classification` was widened to recognize bare
// `TOP` so the heuristic doesn't fire on already-canonical
// `TOP SECRET//...` input.
//
// Five named integration tests below pin the canonical fixture for
// each recovery branch.

#[test]
fn typo_tpp_resolves_via_top_vocab_addition() {
    // Pinned fixture family: `tests/fixtures/mangled/typo/ed06b49d58c3c389.json`
    // (`TPP SECRET//SI//NOFORN`). Bare `TOP` is in the fuzzy
    // correction vocab; standard dist-1 fuzzy then handles
    // `TPP→TOP`. Strict parser re-joins `TOP SECRET` into the
    // canonical multi-word classification.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TPP SECRET//SI//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("`TPP SECRET//SI//NOFORN` must resolve via TOP-vocab fuzzy path");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
    assert!(
        marking
            .0
            .sci_controls
            .iter()
            .any(|c| matches!(c, SciControl::Si)),
        "SI must survive; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_4char_one_extra_letter_resolves_via_top_vocab() {
    // Pinned fixture family: `TDOP`/`QTOP`/`TOPW SECRET//...` —
    // 4-char inputs at edit distance 1 from `TOP` via the
    // Levenshtein length-diff filter. The vocab path covers all
    // three with no extra heuristic.
    let rx = DecoderRecognizer::new();
    for input in &[
        b"TDOP SECRET//SI//NOFORN".as_slice(),
        b"QTOP SECRET//SI//NOFORN".as_slice(),
        b"TOPW SECRET//SI//NOFORN".as_slice(),
    ] {
        let Parsed::Unambiguous(marking) = rx.recognize(input, 0, &*TEST_SCHEME, &deep_cx()) else {
            panic!(
                "{:?} must resolve via TOP-vocab fuzzy path",
                std::str::from_utf8(input).unwrap_or("<non-utf8>")
            );
        };
        assert_eq!(
            effective_level(&marking),
            Some(Classification::TopSecret),
            "4-char one-extra-letter `TOP`-typo must resolve to TopSecret"
        );
    }
}

#[test]
fn typo_otp_resolves_via_3char_heuristic() {
    // Pinned fixture: `OTP SECRET//...` shape. T↔O transposition
    // is dist 2 under standard Levenshtein, so the vocab fuzzy
    // path's confidence floor blocks it; the 3-char classification
    // heuristic is the recovery path. `FixSource` for this path is
    // `DecoderClassificationHeuristic` (Severity::Warn,
    // `recognition` capped at `HEURISTIC_RECOGNITION_CAP = 0.95`
    // — exactly the default `confidence_threshold`, so the
    // heuristic fix lands at-threshold by design).
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"OTP SECRET//SI//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("`OTP SECRET//...` must resolve via 3-char heuristic");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
    assert!(
        marking
            .0
            .sci_controls
            .iter()
            .any(|c| matches!(c, SciControl::Si)),
        "SI must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_tp_and_to_resolve_via_2char_heuristic_extension() {
    // 2-char heuristic extension. `TP`/`TO` at the leading
    // classification slot map to `TOP` (the elided-middle-O and
    // elided-trailing-P cases respectively). Bare `TP`/`TO` have
    // no other canonical CAPCO meaning, so the heuristic isn't
    // ambiguous in practice.
    let rx = DecoderRecognizer::new();
    for input in &[
        b"TP SECRET//SI//NOFORN".as_slice(),
        b"TO SECRET//SI//NOFORN".as_slice(),
    ] {
        let Parsed::Unambiguous(marking) = rx.recognize(input, 0, &*TEST_SCHEME, &deep_cx()) else {
            panic!(
                "{:?} must resolve via 2-char TP/TO heuristic",
                std::str::from_utf8(input).unwrap_or("<non-utf8>")
            );
        };
        assert_eq!(
            effective_level(&marking),
            Some(Classification::TopSecret),
            "2-char `TP`/`TO` classification typo must resolve to TopSecret"
        );
    }
}

#[test]
fn typo_tops_ecret_resolves_via_top_vocab_token_boundary() {
    // Pinned fixture: `TOPS ECRET//...` — token-boundary issue
    // where the `S` of `SECRET` migrated to the end of `TOP`. The
    // decoder recovers because `TOPS` (4 chars) fuzzy-matches `TOP` (3 chars)
    // at edit distance 1 (delete trailing `S`), and `ECRET` (5 chars)
    // fuzzy-matches `SECRET` (6 chars) at edit distance 1 (insert
    // leading `S`). The strict parser then re-joins them as
    // `TOP SECRET`.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TOPS ECRET//SI//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("`TOPS ECRET//...` must resolve via TOP+SECRET vocab fuzzy");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
}

// ---------------------------------------------------------------------------
// Issue #133: REL TO structural repair (preprocessing)
// ---------------------------------------------------------------------------
//
// Four structural patterns recovered as preprocessing in
// `generate_candidate_bytes` (before fuzzy correction). Preprocessing
// rather than separate-candidate emission because (a) fuzzy correction
// would silently rewrite `RELT` → `REL` before pattern 2's header
// normalize could fire, and (b) REL TO trigraphs do NOT contribute to
// the prior in `canonical_tokens_for`, so a separate fix candidate
// would tie with the raw on prior and lose on emit-order.
//
// Patterns 1 and 2 are literal-shape transforms — `REL OT ` and
// `RELT O ` are not valid CAPCO anywhere. Patterns 3 and 4 are
// trigraph-guarded: the fix only fires when `is_country_code(joined)`
// returns true AND the shorter prefix alone is not a trigraph.
//
// The riskier per-trigraph fuzzy cluster (`USB → USA`, `AUT → AUS`,
// `ASU → AUS`) is deferred to issue #186 because it requires
// corpus-weighted priors plus block-level CAPCO §H.8 invariants
// (originator-first, alphabetical sort, no duplicates) to safely
// disambiguate against valid trigraphs like AUT (Austria) and UZB
// (Uzbekistan).

#[test]
fn typo_rel_ot_resolves_via_header_normalize() {
    // Pinned fixture: `tests/fixtures/mangled/typo/b5c53e55302e8c5f.json`
    // (`SECRET//REL OT USA, AUS, GBR`). Header transposition: TO
    // appears as OT. Preprocessing rewrites `REL OT ` → `REL TO `
    // before fuzzy/strict run.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//REL OT USA, AUS, GBR",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`REL OT` must resolve via header normalize");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert_eq!(
        marking.0.rel_to.len(),
        3,
        "REL TO must carry 3 trigraphs (USA, AUS, GBR); attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_relt_o_resolves_via_header_normalize() {
    // Pinned fixture: `tests/fixtures/mangled/typo/a41b81bc72978bc5.json`
    // (`SECRET//RELT O USA, AUS, GBR`). Header token-boundary slip:
    // the trailing T migrated from REL to the start of the gap before
    // O. Preprocessing rewrites `RELT O ` → `REL TO ` before the
    // fuzzy pass would otherwise silently rewrite `RELT` → `REL`
    // (distance 1 deletion of T against the in-vocab DissemControl
    // `REL` token), which would make the strict parser land at
    // [AUS, GBR] with USA dropped.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//RELT O USA, AUS, GBR",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`RELT O` must resolve via header normalize");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert_eq!(
        marking.0.rel_to.len(),
        3,
        "REL TO must carry 3 trigraphs (USA, AUS, GBR); attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_a_us_resolves_via_entry_token_boundary() {
    // Pinned fixture: `tests/fixtures/mangled/typo/1b4875ece8a0a396.json`
    // (`SECRET//REL TO USA,A US, GBR`). Entry token-boundary: AUS
    // appears as `A US` with a stray space. Preprocessing rewrites
    // the 4-character entry `A US` → `AUS` only when `is_country_code`
    // confirms the joined 3-letter string is a valid country code.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//REL TO USA,A US, GBR",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`A US` inside REL TO must resolve via entry token-boundary");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert_eq!(
        marking.0.rel_to.len(),
        3,
        "REL TO must carry 3 trigraphs (USA, AUS, GBR); attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_au_comma_s_resolves_via_entry_comma_misplacement() {
    // Pinned fixture: `tests/fixtures/mangled/typo/83e3ea8d68711138.json`
    // (`SECRET//REL TO USA, AU,S GBR`). Entry comma misplacement:
    // the comma in `AUS, GBR` slipped left one position to produce
    // `AU,S GBR`. Preprocessing rewrites `AU,S ` → `AUS, ` only
    // when `is_country_code(AU + S)` (i.e., AUS) is true AND
    // `is_country_code(AU)` is false — the trigraph guard excludes
    // false-positive shapes like `EU,S USA` where the comma is
    // between the valid 2-char EU and a separate entry.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//REL TO USA, AU,S GBR",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`AU,S GBR` inside REL TO must resolve via entry comma misplacement");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert_eq!(
        marking.0.rel_to.len(),
        3,
        "REL TO must carry 3 trigraphs (USA, AUS, GBR); attrs = {:?}",
        marking.0,
    );
}

#[test]
fn rel_to_structural_repair_does_not_corrupt_aut_austria() {
    // Defensive regression test: AUT is a valid country trigraph
    // (Austria, ISO 3166-1 alpha-3). The riskier per-trigraph fuzzy
    // recovery deferred to issue #186 would have to disambiguate
    // AUT-as-typo-for-AUS from AUT-as-Austria using corpus priors
    // and block-level invariants. The structural repair is
    // intentionally narrower: it only touches literal-shape
    // patterns and trigraph-joinable tokens. AUT in a valid
    // position must round-trip unchanged.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET//REL TO USA, AUT", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("`REL TO USA, AUT` (AUT = Austria) must round-trip unchanged");
    };
    assert_eq!(
        marking.0.rel_to.len(),
        2,
        "REL TO must keep both USA and AUT (Austria); attrs = {:?}",
        marking.0,
    );
}

// ---------------------------------------------------------------------------
// Issue #233: corpus-weighted trigraph priors for REL TO recovery
// ---------------------------------------------------------------------------
//
// Phase 4 PR-1 baked per-token log-priors but explicitly deferred REL TO
// trigraphs (see prior comment block above and CLAUDE.md "Phase 4 PR-1").
// Issue #233 adds a parallel ``COUNTRY_CODE_BASE_RATES`` table so the decoder
// can break fuzzy ties between popular trigraphs (USA, GBR, AUS, FVEY)
// and rare lookalikes (UZB, ASM, AUT-as-Austria) by log-prior delta
// rather than edit distance alone. The decoder's existing
// ``UNAMBIGUOUS_LOG_MARGIN`` (~1.6 nats ≈ 5× odds ratio) realizes the
// "only correct if there's nothing else it can be" rule once the prior
// gap is wide enough.

#[test]
fn typo_usb_resolves_to_usa_via_trigraph_priors() {
    // Pinned fixture: `tests/fixtures/mangled/typo/ba3fed4ec87384d3.json`
    // (`SECRET//REL TO USB, AUS, GBR`). USB is not a country trigraph;
    // both USA (1 substitution: B→A) and UZB (1 substitution: S→Z) are
    // edit-distance-1 candidates. Without trigraph priors the decoder
    // cannot break the tie. With ``COUNTRY_CODE_BASE_RATES`` USA's
    // log-prior dominates UZB's by ~7 nats — far above the
    // ``UNAMBIGUOUS_LOG_MARGIN``.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//REL TO USB, AUS, GBR",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`USB → USA` recovery must produce an unambiguous decode (issue #233)");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    let trigraphs: Vec<&str> = marking.0.rel_to.iter().map(|c| c.as_str()).collect();
    assert!(
        trigraphs.contains(&"USA"),
        "USB must be corrected to USA; got rel_to={:?}",
        trigraphs,
    );
    assert!(
        !trigraphs.contains(&"UZB"),
        "USB must NOT be corrected to UZB (corpus-weighted prior loses); got rel_to={:?}",
        trigraphs,
    );
}

#[test]
fn typo_asu_resolves_to_aus_via_trigraph_priors() {
    // Pinned fixture: `tests/fixtures/mangled/typo/401856cea23a70f4.json`
    // (`SECRET//REL TO USA, ASU, GBR`). ASU is not a country trigraph;
    // ASM (American Samoa) is edit-distance-1 (substitute U→M), AUS
    // is edit-distance-2 (transpose S/U). Pure fuzzy picks ASM. With
    // trigraph priors AUS's log-prior dominates ASM's by ~7 nats and
    // overwhelms the 1-edit advantage.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//REL TO USA, ASU, GBR",
        0,
        &*TEST_SCHEME,
        &deep_cx(),
    ) else {
        panic!("`ASU → AUS` recovery must produce an unambiguous decode (issue #233)");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    let trigraphs: Vec<&str> = marking.0.rel_to.iter().map(|c| c.as_str()).collect();
    assert!(
        trigraphs.contains(&"AUS"),
        "ASU must be corrected to AUS; got rel_to={:?}",
        trigraphs,
    );
    assert!(
        !trigraphs.contains(&"ASM"),
        "ASU must NOT be corrected to ASM (corpus-weighted prior loses); got rel_to={:?}",
        trigraphs,
    );
}

// ---------------------------------------------------------------------------
// Issue #234 PR-B: REL TO USA-injection for short first entries
// ---------------------------------------------------------------------------
//
// `try_rel_to_fuzzy_trigraph_candidates` (PR-A, issue #233) handles
// 3-char REL TO entry typos by fuzzy-matching against the trigraph
// vocabulary. PR-B closes the complementary case: the FIRST entry of a
// REL TO block, which §H.8 p151 requires to be USA, sometimes appears
// as a 1- or 2-character token below `MIN_FUZZY_LEN = 3`. Without this
// path the decoder produces zero candidates because the strict
// parser drops the unknown short entry and PR-A's 3-char filter
// excludes it from fuzzy matching.

#[test]
fn recovers_ad2bcfe3ac0b0765_short_first_entry_resolves_to_usa() {
    // Pinned fixture: `tests/fixtures/mangled/typo/ad2bcfe3ac0b0765.json`
    // (`SECRET//REL TO SA, AUS, GBR` → `SECRET//REL TO USA, AUS, GBR`).
    // `SA` is 2 chars — below `MIN_FUZZY_LEN = 3`, so PR-A's fuzzy
    // path skips it. The §H.8 p151 USA-first invariant gives PR-B
    // the structural signal: the first entry of a REL TO block is
    // canonically USA, so a short first entry is most plausibly a
    // truncated USA. The injection candidate replaces `SA` with
    // `USA`; corpus-weighted log-priors (PR-A's scoring contribution)
    // carry it past the no-recovery baseline at score time.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"SECRET//REL TO SA, AUS, GBR", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`SA → USA` recovery must produce an unambiguous decode \
             (issue #234 PR-B fixture ad2bcfe3ac0b0765)"
        );
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    let trigraphs: Vec<&str> = marking.0.rel_to.iter().map(|c| c.as_str()).collect();
    assert!(
        trigraphs.contains(&"USA"),
        "first entry must be corrected to USA; got rel_to={:?}",
        trigraphs,
    );
    assert!(
        trigraphs.contains(&"AUS"),
        "AUS must survive the recovery; got rel_to={:?}",
        trigraphs,
    );
    assert!(
        trigraphs.contains(&"GBR"),
        "GBR must survive the recovery; got rel_to={:?}",
        trigraphs,
    );
}

// ---------------------------------------------------------------------------
// Issue #133: position-aware short-token classification heuristic
// ---------------------------------------------------------------------------
//
// The keyboard-proximity heuristic resolves 1- and 2-character typos
// in the leading classification slot of portion or banner markings —
// shapes the vocab-based fuzzy matcher cannot touch (`MIN_FUZZY_LEN
// = 3`). The recognizer flags these with
// `FixSource::DecoderClassificationHeuristic` so the engine emits
// `Severity::Warn` (always-visible in `--check`) and caps the sole
// surviving `recognition` axis at `HEURISTIC_RECOGNITION_CAP = 0.95`
// — exactly the default `confidence_threshold`, so a single-
// candidate heuristic fix lands at-threshold by design.
//
// The Enron-corpus-derived mangled-fixture tree at
// `tests/fixtures/mangled/typo/` contains very few short-leading-
// classification typos (mostly 3+ char tail-token typos like UK→TK,
// USAR→SAR), so the harness's per-class rate doesn't move from this
// heuristic alone. These integration tests pin the heuristic's
// behavior on synthetic inputs that exercise the keyboard-proximity
// table directly.

#[test]
fn heuristic_2char_ts_decodes_portion() {
    // (YS//NF) — `YS` is the keyboard-proximity-typo for `TS`
    // (Y is one key left of T on QWERTY, S is unchanged). Outside
    // the heuristic's scope it would be left as-is and the strict
    // parse would land with classification=None (YS doesn't
    // resolve), making the candidate fail the engine's expected-
    // attrs equality.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(YS//NF)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(YS//NF) should resolve to (TS//NF) via the heuristic");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::TopSecret),
        "YS leading position should heuristic-fix to TS; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NF must survive the heuristic-corrected canonicalization"
    );
}

#[test]
fn heuristic_1char_s_decodes_portion() {
    // (W//NF) — `W` is QWERTY-adjacent to S (one key above).
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(W//NF)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(W//NF) should resolve to (S//NF) via the heuristic");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Secret),
        "W leading position should heuristic-fix to S; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NF must survive"
    );
}

#[test]
fn heuristic_1char_c_decodes_portion() {
    // (V//NF) — V is QWERTY-adjacent to C (one key right).
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(V//NF)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(V//NF) should resolve to (C//NF) via the heuristic");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Confidential),
        "V leading position should heuristic-fix to C; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn heuristic_decodes_banner_form() {
    // Banner form (no parens) — same heuristic applies.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"RS//NOFORN", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("RS//NOFORN should heuristic-resolve to TS//NOFORN");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::TopSecret),
        "RS heuristic-fixes to TS in banner shape; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn heuristic_emits_classification_heuristic_provenance() {
    // The decoder must tag the canonicalization attempt with
    // `FixSource::DecoderClassificationHeuristic` so the engine
    // can downgrade severity and cap rule confidence. The check
    // here is on the marking's `provenance.fix_source` field
    // — without it the engine would treat the
    // fix the same as a vocab-based decoder fix, which would
    // (a) auto-apply at default threshold and (b) show as
    // `Severity::Fix` instead of `Severity::Warn`, defeating the
    // fix-and-warn intent.
    use marque_rules::FixSource;
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(YS//NF)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(YS//NF) must resolve unambiguously");
    };
    let provenance = marking
        .1
        .as_ref()
        .expect("decoder marking must carry provenance");
    assert_eq!(
        provenance.fix_source,
        FixSource::DecoderClassificationHeuristic,
        "heuristic-corrected candidate must tag its provenance source"
    );
}

#[test]
fn heuristic_does_not_fire_on_canonical_classification() {
    // (S//NF) is fully canonical — the heuristic must not fire
    // (would emit a no-op rewrite). The decoder should produce
    // an Unambiguous marking with `provenance.fix_source ==
    // DecoderPosterior` (the standard vocab path), or nothing at
    // all if the strict path picked up the marking.
    use marque_rules::FixSource;
    let rx = DecoderRecognizer::new();
    if let Parsed::Unambiguous(marking) = rx.recognize(b"(S//NF)", 0, &*TEST_SCHEME, &deep_cx())
        && let Some(provenance) = marking.1.as_ref()
    {
        assert_ne!(
            provenance.fix_source,
            FixSource::DecoderClassificationHeuristic,
            "heuristic must not fire on already-canonical (S//NF); \
             attrs = {:?}",
            marking.0,
        );
    }
}

// ---------------------------------------------------------------------------
// SCI delimiter recovery (issue #198 — #133)
// ---------------------------------------------------------------------------
//
// End-to-end verification that the preprocessing in
// `try_sci_delimiter_repair` flows through the recognizer and produces
// a parsed CapcoMarking with the expected SCI controls. Per-pattern
// unit tests live in `decoder.rs::tests`; these guard the
// preprocessing→recognizer integration.

#[test]
fn sci_delimiter_repair_recovers_concatenated_compound_hcsp() {
    // `(S//HCSP)` — concatenated `HCS-P` (Pattern A). Preprocessing
    // rewrites HCSP → HCS-P; the strict parser then accepts HCS-P as
    // a registered control-compartment compound.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//HCSP)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(S//HCSP) should resolve via SCI delimiter repair");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(
        marking.0.sci_controls.contains(&SciControl::HcsP),
        "HcsP must land in sci_controls; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn sci_delimiter_repair_recovers_missing_slash_sitk() {
    // `(S//SITK)` — concatenated `SI` + `TK` (Pattern B).
    // Preprocessing rewrites SITK → SI/TK; both bare control
    // systems must land in sci_controls.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//SITK)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(S//SITK) should resolve via SCI delimiter repair");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(
        marking.0.sci_controls.contains(&SciControl::Si)
            && marking.0.sci_controls.contains(&SciControl::Tk),
        "SI and TK must both land in sci_controls; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn sci_delimiter_repair_recovers_wrong_delimiter_si_dash_tk() {
    // `(S//SI-TK)` — `-` between two bare CS is wrong (Pattern C).
    // Preprocessing rewrites SI-TK → SI/TK. SI-TK is NOT a registered
    // CVE compound, so the rewrite is unambiguous.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//SI-TK)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(S//SI-TK) should resolve via SCI delimiter repair");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(
        marking.0.sci_controls.contains(&SciControl::Si)
            && marking.0.sci_controls.contains(&SciControl::Tk),
        "SI and TK must both land in sci_controls; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn sci_delimiter_repair_leaves_canonical_compound_alone() {
    // `(S//SI-G)` — SI-G is a registered CVE compound. Preprocessing
    // must NOT rewrite it (Pattern C short-circuits on registered
    // compounds). Resolves via the normal strict path.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//SI-G)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!("(S//SI-G) must resolve as canonical SI-G");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    // SI-G is a registered CVE compound; the parser emits the
    // `SiG` variant directly into `sci_controls` (not the bare `Si`
    // plus a separate compartment) per the structural-or-CVE
    // dispatch in `parse_sci_block`. This assertion is what makes
    // the test load-bearing as a regression guard against Pattern C
    // erroneously firing on the canonical form.
    assert!(
        marking.0.sci_controls.contains(&SciControl::SiG),
        "SI-G must land in sci_controls as the SiG compound (no Pattern C \
         rewrite); attrs = {:?}",
        marking.0,
    );
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

// ---------------------------------------------------------------------------
// NATO longhand fold
// ---------------------------------------------------------------------------
//
// These tests verify that the decoder's `try_nato_fold` preprocessing helper
// recovers NATO longhand classification levels from mangled portion markings.
//
// Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (canonical Register — NATO
// portion abbreviations NU/NR/NC/NS/CTS for the five base levels).

use marque_ism::NatoClassification;
use marque_ism::attrs::MarkingClassification;
use marque_rules::recognition::FeatureId;

/// Return the `NatoClassification` from a marking, or panic.
fn nato_class(m: &marque_capco::CapcoMarking) -> NatoClassification {
    match m
        .0
        .classification
        .as_ref()
        .expect("marking has no classification")
    {
        MarkingClassification::Nato(n) => *n,
        other => panic!("expected Nato classification, got {other:?}"),
    }
}

#[test]
fn nato_u_portion_folds_to_nu() {
    // `(NATO U)` — NATO UNCLASSIFIED longhand abbrev → NU. The strict
    // parser doesn't recognize `NATO U` without the `//` prefix, so
    // the decoder folds it to `(//NU)` → strict-parses to
    // NatoUnclassified.
    //
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(NATO U)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO U)` must fold to `(//NU)` and decode \
             to NatoUnclassified (decoder NATO longhand fold)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoUnclassified,
        "NATO U must fold to NU (NatoUnclassified)"
    );
}

#[test]
fn nato_r_portion_folds_to_nr() {
    // `(NATO R)` — NATO RESTRICTED longhand abbrev → NR
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(NATO R)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO R)` must fold to `(//NR)` and decode \
             to NatoRestricted (decoder NATO longhand fold)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoRestricted,
        "NATO R must fold to NR (NatoRestricted)"
    );
}

#[test]
fn nato_c_portion_folds_to_nc() {
    // `(NATO C)` — NATO CONFIDENTIAL longhand abbrev → NC
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(NATO C)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO C)` must fold to `(//NC)` and decode \
             to NatoConfidential (decoder NATO longhand fold)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoConfidential,
        "NATO C must fold to NC (NatoConfidential)"
    );
}

#[test]
fn nato_s_portion_folds_to_ns() {
    // `(NATO S)` — NATO SECRET longhand abbrev → NS
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(NATO S)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO S)` must fold to `(//NS)` and decode \
             to NatoSecret (decoder NATO longhand fold)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoSecret,
        "NATO S must fold to NS (NatoSecret)"
    );
}

#[test]
fn nato_ts_portion_folds_to_cts() {
    // `(NATO TS)` — NATO TOP SECRET longhand abbrev → CTS
    // Per CAPCO-2016 §G.1 Table 4 pp 36-38, NATO TOP SECRET maps to
    // COSMIC TOP SECRET (CTS) in the canonical Register.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(NATO TS)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO TS)` must fold to `(//CTS)` and decode \
             to CosmicTopSecret (decoder NATO longhand fold)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::CosmicTopSecret,
        "NATO TS must fold to CTS (CosmicTopSecret)"
    );
}

#[test]
fn nato_secret_long_form_folds_to_ns() {
    // `(NATO SECRET//NF)` — NATO SECRET full-word longhand with NOFORN
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"(NATO SECRET//NF)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO SECRET//NF)` must fold to `(//NS//NF)` \
             and decode to NatoSecret (decoder NATO longhand fold)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoSecret,
        "NATO SECRET must fold to NS (NatoSecret)"
    );
    // NOFORN must survive the fold
    let nf_present = marking
        .0
        .dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    assert!(nf_present, "NOFORN must survive the NATO SECRET fold");
}

#[test]
fn nato_top_secret_long_form_folds_to_cts() {
    // `(NATO TOP SECRET//NF)` — NATO TOP SECRET full-word longhand → CTS
    // Two-token level requires treating "TOP SECRET" as a compound in the fold.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"(NATO TOP SECRET//NF)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO TOP SECRET//NF)` must fold to `(//CTS//NF)` \
             and decode to CosmicTopSecret (decoder NATO longhand fold)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::CosmicTopSecret,
        "NATO TOP SECRET must fold to CTS (CosmicTopSecret)"
    );
    let nf_present = marking
        .0
        .dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    assert!(nf_present, "NOFORN must survive the NATO TOP SECRET fold");
}

#[test]
fn nato_in_rel_to_list_is_not_folded() {
    // `(S//REL TO USA, NATO)` — `NATO` is a country tetragraph inside
    // REL TO, not a classification keyword. The fold's segment-leading
    // guard must not fire here because the `S` segment (US classification)
    // comes first and the `REL TO USA, NATO` segment does not start with
    // `NATO`.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (fold guard invariant).
    let rx = DecoderRecognizer::new();
    let result = rx.recognize(b"(S//REL TO USA, NATO)", 0, &*TEST_SCHEME, &deep_cx());
    match result {
        Parsed::Unambiguous(marking) => {
            // Must parse as US Secret, not Nato(NatoSecret)
            match marking.0.classification.as_ref() {
                Some(MarkingClassification::Nato(_)) => {
                    panic!(
                        "fold must NOT fire on NATO in REL TO list; got Nato classification \
                         instead of Us(Secret)"
                    );
                }
                Some(MarkingClassification::Us(lvl)) => {
                    assert_eq!(
                        *lvl,
                        marque_ism::Classification::Secret,
                        "REL TO USA, NATO should parse as US Secret"
                    );
                }
                other => panic!("unexpected classification: {other:?}"),
            }
        }
        Parsed::Ambiguous { .. } => {
            // Also acceptable — the key invariant is that Nato classification
            // was NOT injected by the fold. If the decoder doesn't recognize
            // this input at all, that's fine; what matters is that a false
            // Nato fold didn't fire.
        }
    }
}

#[test]
fn nato_in_fgi_list_is_not_folded() {
    // `(//FGI USA NATO C)` — `NATO` is a tetragraph in the FGI country list,
    // not the first token of the segment. The fold's segment-leading guard
    // must not substitute `NATO C` as if it were a classification.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (fold guard invariant).
    let rx = DecoderRecognizer::new();
    let result = rx.recognize(b"(//FGI USA NATO C)", 0, &*TEST_SCHEME, &deep_cx());
    // The invariant: if any candidate is returned, none should have
    // MarkingClassification::Nato(_) as the primary classification from the fold.
    match result {
        Parsed::Unambiguous(marking) => {
            // A valid FGI marking parsed; confirm fold didn't inject Nato class
            if let Some(MarkingClassification::Nato(_)) = marking.0.classification.as_ref() {
                panic!(
                    "fold must NOT fire on NATO inside FGI country list; \
                     got Nato classification from fold"
                );
            }
        }
        Parsed::Ambiguous { candidates } => {
            for c in &candidates {
                if let Some(MarkingClassification::Nato(_)) = c.marking.0.classification.as_ref() {
                    panic!("fold must NOT inject Nato classification into FGI-list candidate");
                }
            }
        }
    }
}

#[test]
fn already_canonical_ns_is_idempotent() {
    // `(//NS//NF)` is the canonical NATO SECRET NOFORN portion.
    // The strict recognizer handles it directly; the decoder is not
    // invoked at all (strict parse succeeds). The fold's None-return
    // on already-canonical input means no SupersededToken feature is
    // added.
    //
    // This test exercises the engine path (via DecoderRecognizer) to
    // confirm that canonical input doesn't trigger the fold path.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let result = rx.recognize(b"(//NS//NF)", 0, &*TEST_SCHEME, &deep_cx());
    // Canonical input should decode correctly (strict recognizer handles it,
    // but even via decoder the result must be NatoSecret + Noforn).
    match result {
        Parsed::Unambiguous(marking) => {
            assert_eq!(
                nato_class(&marking),
                NatoClassification::NatoSecret,
                "canonical (//NS//NF) must decode to NatoSecret"
            );
            // If provenance is present (decoder path), the features must NOT
            // include SupersededToken (no fold was needed).
            if let Some(prov) = marking.1.as_ref() {
                let has_superseded = prov
                    .features
                    .iter()
                    .any(|f| f.id == FeatureId::SupersededToken);
                assert!(
                    !has_superseded,
                    "canonical input must NOT emit SupersededToken feature; \
                     fold only fires on longhand input"
                );
            }
        }
        Parsed::Ambiguous { .. } => {
            // Also acceptable — the key invariant is absence of the fold feature.
        }
    }
}

#[test]
fn nato_fold_emits_superseded_token_feature() {
    // `(NATO S)` → fold fires → FeatureId::SupersededToken present exactly once
    // in the decoder provenance. This test validates the audit-trail requirement
    // from the brief: the fold records `SupersededToken` (reusing the existing
    // variant per the brief's explicit instruction, delta 0.0)
    // (see `decoder.rs:855-862` wire-site comment for the equivalence-transform-not-supersession rationale).
    //
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(NATO S)", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`(NATO S)` must decode unambiguously after the NATO longhand fold \
             (audit-feature check)"
        );
    };
    let provenance = marking
        .1
        .as_ref()
        .expect("decoder path must carry DecoderProvenance for (NATO S)");
    let superseded_count = provenance
        .features
        .iter()
        .filter(|f| f.id == FeatureId::SupersededToken)
        .count();
    assert_eq!(
        superseded_count, 1,
        "fold must emit SupersededToken exactly once in provenance features; \
         got {superseded_count}. features = {:?}",
        provenance.features
    );
}
#[test]
fn nato_in_second_segment_yields_decode_miss() {
    // `(S//NATO C)` — NATO C appears in the SCI/dissem slot (second
    // segment), NOT the classification slot (first segment). The
    // fold is restricted to the first non-empty `//`-separated segment only.
    // The first segment is `S` (doesn't start with "NATO "), so fold_nato_segment
    // returns None for it, `any_changed = false`, and `try_nato_fold` returns None.
    // The decoder feeds the original `(S//NATO C)` to the strict parser, which
    // cannot parse it, and the decoder returns a zero-candidate Ambiguous
    // (decode-miss).
    //
    // Domain rationale (§H.7): NATO commingled with US info should transmute to
    // FGI (`(S//FGI NATO)`) — not produce a NATO-axis canonical or a Conflict
    // intermediate. That transmutation is not yet implemented; the decoder
    // produces a decode-miss to avoid wrong intermediates in the meantime.
    //
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38; §A.6 pp 15-17; §H.7 p122.
    let rx = DecoderRecognizer::new();
    let parsed = rx.recognize(b"(S//NATO C)", 0, &*TEST_SCHEME, &deep_cx());
    match parsed {
        Parsed::Ambiguous { ref candidates } if candidates.is_empty() => {
            // Expected: decode-miss. The fold doesn't fire on the second segment,
            // the strict parser can't handle `(S//NATO C)`, and the decoder
            // correctly surfaces zero candidates (§H.7 FGI transmutation domain).
        }
        Parsed::Ambiguous { ref candidates } => {
            // Non-zero candidates: decoder manufactured a partial or Conflict result.
            panic!(
                "`(S//NATO C)` must return zero-candidate decode-miss \
                 (§H.7 FGI transmutation domain); got {} candidate(s): {:?}",
                candidates.len(),
                candidates,
            );
        }
        Parsed::Unambiguous(marking) => {
            // Any Unambiguous result means the fold or decoder fabricated a
            // marking from a cross-segment NATO input — wrong.
            panic!(
                "`(S//NATO C)` must not produce an Unambiguous result; \
                 got marking = {:?}",
                marking.0,
            );
        }
    }
}

#[test]
fn lowercase_nato_secret_atomal_recovers_via_case_normalization() {
    // Regression guard: lowercase `(//nato secret atomal//nf)`
    // case-normalizes, fuzzy-corrects, and reaches the strict parser
    // intact (the NATO fold MUST NOT mangle the `ATOMAL` suffix —
    // verified at the helper level by
    // `fold_nato_segment_returns_none_for_atomal_compound` in
    // decoder.rs unit tests). This test verifies the end-to-end engine
    // recovery path produces the canonical result.
    //
    // Pipeline: lowercase input → normalize_delimiters_and_case → uppercase →
    // try_nato_fold("NATO SECRET ATOMAL") returns None →
    // fuzzy_correct_tokens passes ATOMAL through (in NATO_CLASSIFICATION_KEYWORDS) →
    // generate_candidate_bytes emits `(//NATO SECRET ATOMAL//NF)` →
    // strict parser's parse_nato_classification("NATO SECRET ATOMAL") →
    // canonical form: bare class `NatoSecret` + AEA
    // `Atomal` companion (`(//NS//ATOMAL//NF)` semantic).
    //
    // Citations: CAPCO-2016 §G.1 Table 4 pp 36-38 (legacy text
    // recognition); §H.7 p122 (ATOMAL as AEA, canonical structural
    // model); §G.2 p40 (Table 5: registers ATOMAL as standalone
    // control marking, the autofix target per project memory
    // `remark-on-derivative-use-is-marque-autofix`).
    let rx = DecoderRecognizer::new();
    let parsed = rx.recognize(b"(//nato secret atomal//nf)", 0, &*TEST_SCHEME, &deep_cx());
    match parsed {
        Parsed::Unambiguous(ref marking) => {
            // Legacy `NATO SECRET ATOMAL` text canonicalizes to bare
            // `NatoClassification::NatoSecret` plus an AEA-axis
            // `Atomal` companion (CAPCO-2016 §H.7 p122 worked example
            // + §G.2 p40 Table 5 registration of ATOMAL as a standalone
            // control marking). The classification and AEA semantics
            // are kept on separate axes rather than fused.
            match marking.0.classification.as_ref() {
                Some(MarkingClassification::Nato(NatoClassification::NatoSecret)) => {
                    // Expected canonical bare-class outcome.
                }
                other => panic!(
                    "`(//nato secret atomal//nf)` must recover with \
                     canonical bare class NatoSecret, got {other:?}"
                ),
            }
            let has_atomal = marking
                .0
                .aea_markings
                .iter()
                .any(|a| matches!(a, marque_ism::AeaMarking::Atomal(_)));
            assert!(
                has_atomal,
                "ATOMAL companion must be written into the AEA \
                 axis when the legacy `NATO SECRET ATOMAL` form is recovered \
                 (CAPCO-2016 §H.7 p122)"
            );
        }
        other => panic!(
            "`(//nato secret atomal//nf)` must decode unambiguously, \
             got {other:?}"
        ),
    }
}

// ---------------------------------------------------------------------------
// Banner-form NATO fold (#260)
// ---------------------------------------------------------------------------
//
// Previously `try_nato_fold` returned None for Banner kind, so inputs
// like `NATO S//NOFORN` (banner abbreviation) failed because the strict parser
// only accepts the full banner forms (`NATO SECRET`, `COSMIC TOP SECRET`).
// The fold now handles Banner kind, expanding abbreviations to
// their canonical banner long forms.
//
// Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (canonical Register); §A.6 p15.

#[test]
fn nato_u_banner_folds_to_nato_unclassified() {
    // `NATO U//NF\n` — banner abbreviation for NATO UNCLASSIFIED + NOFORN.
    // The strict parser rejects `NATO U`; the fold expands to `NATO UNCLASSIFIED`,
    // prepends `//` (§A.6 p15), giving `//NATO UNCLASSIFIED//NF`.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"NATO U//NF\n", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`NATO U//NF` must fold to `//NATO UNCLASSIFIED//NF` \
             and decode to NatoUnclassified (banner NATO fold, #260)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoUnclassified,
        "NATO U banner must fold to NatoUnclassified"
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive the banner fold; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn nato_r_banner_folds_to_nato_restricted() {
    // `NATO R//NF` — banner abbreviation for NATO RESTRICTED + NOFORN.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"NATO R//NF", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`NATO R//NF` must fold to `//NATO RESTRICTED//NF` \
             and decode to NatoRestricted (banner NATO fold, #260)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoRestricted,
        "NATO R banner must fold to NatoRestricted"
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn nato_c_banner_folds_to_nato_confidential() {
    // `NATO C//NF` — banner abbreviation for NATO CONFIDENTIAL + NOFORN.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"NATO C//NF", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`NATO C//NF` must fold to `//NATO CONFIDENTIAL//NF` \
             and decode to NatoConfidential (banner NATO fold, #260)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoConfidential,
        "NATO C banner must fold to NatoConfidential"
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn nato_s_banner_folds_to_nato_secret() {
    // `NATO S//NF` — banner abbreviation for NATO SECRET + NOFORN.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"NATO S//NF", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`NATO S//NF` must fold to `//NATO SECRET//NF` \
             and decode to NatoSecret (banner NATO fold, #260)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::NatoSecret,
        "NATO S banner must fold to NatoSecret"
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn nato_ts_banner_folds_to_cosmic_top_secret() {
    // `NATO TS//NF` — banner abbreviation for COSMIC TOP SECRET + NOFORN.
    // Per CAPCO-2016 §G.1 Table 4 pp 36-38, NATO TOP SECRET maps to
    // COSMIC TOP SECRET in the canonical Register.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"NATO TS//NF", 0, &*TEST_SCHEME, &deep_cx())
    else {
        panic!(
            "`NATO TS//NF` must fold to `//COSMIC TOP SECRET//NF` \
             and decode to CosmicTopSecret (banner NATO fold, #260)"
        );
    };
    assert_eq!(
        nato_class(&marking),
        NatoClassification::CosmicTopSecret,
        "NATO TS banner must fold to CosmicTopSecret"
    );
    assert!(
        marking.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn nato_secret_banner_already_canonical_no_fold() {
    // `NATO SECRET//NF` is already the canonical banner long form for NATO SECRET.
    // The strict recognizer handles it directly (no fold needed). The decoder
    // either routes through strict-first or the fold returns None (idempotent).
    // Key invariant: canonical input must NOT emit SupersededToken — the fold
    // did not fire.
    // Citation: CAPCO-2016 §G.1 Table 4 pp 36-38.
    let rx = DecoderRecognizer::new();
    let result = rx.recognize(b"NATO SECRET//NF", 0, &*TEST_SCHEME, &deep_cx());
    match result {
        Parsed::Unambiguous(ref marking) => {
            assert_eq!(
                nato_class(marking),
                NatoClassification::NatoSecret,
                "canonical NATO SECRET must decode as NatoSecret"
            );
            // If provenance is present (decoder path), confirm NO SupersededToken.
            if let Some(prov) = marking.1.as_ref() {
                let has_superseded = prov
                    .features
                    .iter()
                    .any(|f| f.id == FeatureId::SupersededToken);
                assert!(
                    !has_superseded,
                    "canonical banner form must NOT emit SupersededToken (fold must be idempotent); \
                     features = {:?}",
                    prov.features
                );
            }
        }
        Parsed::Ambiguous { .. } => {
            // Also acceptable — canonical input should always decode;
            // an Ambiguous result would indicate a regression elsewhere.
            panic!("canonical `NATO SECRET//NF` should decode unambiguously");
        }
    }
}
