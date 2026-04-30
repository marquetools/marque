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
        as_of: None,
        preceded_by_whitespace: true,
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
    // `RS` (delete E) AND `RSEN` (insert N), both of which are in
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

/// **MissingDelimiter** class — recovery test (issue #133 PR 3).
///
/// `SECRET//NOFORN EXDIS` (missing `//` before `EXDIS`) is the
/// canonical MissingDelimiter shape. The decoder's
/// `try_insert_delimiter` helper inserts `//` at category-transition
/// whitespace gaps before unambiguous segment-starting dissem
/// long-forms (`NOFORN`, `EXDIS`, `ORCON`, …); the result strict-
/// parses as `SECRET//NOFORN//EXDIS` with both dissems landing in
/// the right slots (`Nf` → `dissem_controls`, `Exdis` →
/// `non_ic_dissem`).
///
/// Renamed from `missing_delimiter_secret_noforn_exdis_currently_unrecovered`
/// — the "currently unrecovered" framing was the temporary state
/// pinned in PR 1's predecessor and explicitly invited replacement
/// when the helper lands. Recovery is now the regression-guarded
/// shape.
#[test]
fn missing_delimiter_secret_noforn_exdis_resolves() {
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//NOFORN EXDIS", &deep_cx()) else {
        panic!(
            "SECRET//NOFORN EXDIS must resolve unambiguously after issue #133 \
             PR 3 missing-delimiter insertion lands"
        );
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Secret),
        "classification must survive the delimiter insertion; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "NOFORN must land in dissem_controls; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis),
        "EXDIS must land in non_ic_dissem after `//` is inserted before \
         it (issue #133 PR 3 missing-delimiter insertion); attrs = {:?}",
        marking.0,
    );
}

// ---------------------------------------------------------------------------
// Issue #133 PR 3: missing-delimiter insertion
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
// The PR 3 helper does NOT yet handle SCI starters (`SI`, `HCS`,
// `TK`), SAR prefixes (`SAR-*`), or `SPECIAL ACCESS REQUIRED-*` —
// those need classification-context lookahead and are deferred to
// the planned PR 4 / corpus-confidence work. The hard-splitter +
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET REL TO USA, AUS, GBR", &deep_cx())
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
    let Parsed::Unambiguous(marking) =
        rx.recognize(b"TOP SECRET//HCS-P INTEL OPS ORCON/NOFORN", &deep_cx())
    else {
        panic!("HCS-P INTEL OPS ORCON/NOFORN should resolve");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::TopSecret),
        "TOP SECRET must survive; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Oc)
            && marking.0.dissem_controls.contains(&DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET NOFORN//EXDIS", &deep_cx()) else {
        panic!("SECRET NOFORN//EXDIS should resolve");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret),);
    assert!(marking.0.dissem_controls.contains(&DissemControl::Nf));
    assert!(marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis));
}

#[test]
fn missing_delimiter_hard_splitter_inside_segment() {
    // `TOP SECRET//SI/TK NOFORN` — NOFORN follows whitespace inside
    // an SCI segment. Hard-splitter rule fires on NOFORN.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"TOP SECRET//SI/TK NOFORN", &deep_cx()) else {
        panic!("TOP SECRET//SI/TK NOFORN should resolve");
    };
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//SBU NOFORN", &deep_cx()) else {
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
    // (issue #133 PR 5). The SAR grammar accepts any alphanumeric
    // identifier, so the strict parser cleanly absorbs `NOFORN` as
    // the trailing sub-compartment of the `XR-XRA` compartment when
    // no `//` separator precedes it. The competing delim-inserted
    // candidate puts `NOFORN` into `dissem_controls` instead — that's
    // the canonical interpretation. Before PR 5 the bag-of-tokens
    // scorer rewarded the absorbing parse (fewer scored tokens →
    // higher posterior because log-priors are negative);
    // `HARD_SPLITTER_ABSORPTION_PENALTY` flips the contest.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB NOFORN",
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
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "NOFORN must land in dissem_controls (delim-inserted candidate \
         beats the absorbing one via HARD_SPLITTER_ABSORPTION_PENALTY); \
         attrs = {:?}",
        marking.0,
    );
}

#[test]
fn missing_delimiter_full_sar_with_trailing_noforn_resolves() {
    // `TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN`
    // (issue #133 PR 5). Same scoring problem as the abbreviated SAR
    // shape above, but here `NOFORN` gets absorbed as the trailing
    // word of the multi-word `Full`-indicator program nickname
    // (`identifier: "BUTTER POPCORN NOFORN"`). The `Full` shape needs
    // the per-word check inside `contains_hard_splitter_word` to fire.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN NOFORN",
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
        marking.0.dissem_controls.contains(&DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//NOFORN//EXDIS", &deep_cx()) else {
        panic!("SECRET//NOFORN//EXDIS should resolve directly");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(marking.0.dissem_controls.contains(&DissemControl::Nf));
    assert!(marking.0.non_ic_dissem.contains(&NonIcDissem::Exdis));
}

// ---------------------------------------------------------------------------
// Issue #133 PR 6: SAR indicator-keyword structural repair
// ---------------------------------------------------------------------------
//
// Three structural recovery paths added in PR 6:
//   1. `[A-Z]{1,3}SAR-` → `SAR-` prefix strip (USAR-BP, ABSAR-BP, …).
//   2. `SAR[A-Z0-9]{2,3}<delim>` → `SAR-<rest><delim>` missing-hyphen
//      insertion (SARBP, SARABC).
//   3. `SPECIAL`, `ACCESS` added to `EXTENDED_CORRECTION_VOCAB` so
//      the fuzzy matcher can recover SPCIAL/SEPCIAL/SPECAL/CCESS-
//      style keyword typos via the existing per-token correction
//      path.
//
// The structural penalty `CUSTOM_SCI_MARKING_PENALTY` in
// `score_candidate` is a peer change: without it, a raw `USAR-BP-J12`
// segment is interpreted by the lenient strict-parser as 3 custom-
// system SCI markings (USAR/CD/XR with `canonical_enum: None`), and
// the bag-of-tokens scorer can't distinguish that interpretation
// from the SAR-repaired candidate. The penalty demotes the
// custom-only-SCI parse so the SAR-repaired candidate wins by a
// margin clearing `UNAMBIGUOUS_LOG_MARGIN`.
//
// All three integration tests below pin a named fixture from the
// SC-004 mangled corpus so the harness's `Typo`-class rate (`50.0%`
// post-PR-6) is anchored to specific recovery shapes rather than an
// opaque aggregate.

#[test]
fn typo_usar_prefix_resolves_via_indicator_repair() {
    // Pinned fixture: `tests/fixtures/mangled/typo/d04f45f7a4f5a8b4.json`
    // (`SECRET//USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN`).
    // The full Enron-corpus SAR shape with a stray `U` prefix on the
    // SAR indicator. Pre-PR-6 this resolved as 3 custom-system SCI
    // markings (lenient strict parse) competing with the SAR-repaired
    // candidate at a tied posterior; both `try_sar_indicator_repair`
    // (added the SAR candidate) and `CUSTOM_SCI_MARKING_PENALTY`
    // (demoted the custom-SCI candidate) had to land together to
    // clear the `UNAMBIGUOUS_LOG_MARGIN` threshold.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"SECRET//USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN",
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
        marking.0.dissem_controls.contains(&DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"TOP SECRET//SARBP//NOFORN", &deep_cx())
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
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "NOFORN must survive; attrs = {:?}",
        marking.0,
    );
}

#[test]
fn typo_spcial_keyword_resolves_via_extended_correction_vocab() {
    // Pinned fixture: `tests/fixtures/mangled/typo/1f75ddd89b432949.json`
    // (`TOP SECRET//SPCIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN`).
    // `SPCIAL` is a missing-`E` typo on `SPECIAL` (edit distance 1).
    // PR 6's vocab addition of `SPECIAL` to
    // `SAR_STRUCTURAL_KEYWORDS` lets the existing per-token fuzzy
    // matcher recover it; the strict SAR parser then matches the
    // canonical `SPECIAL ACCESS REQUIRED-BUTTER POPCORN` indicator
    // literally. No structural-repair pass is involved — this case
    // exercises the vocab path only.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(
        b"TOP SECRET//SPCIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN",
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
    assert!(marking.0.dissem_controls.contains(&DissemControl::Nf));
}

// ---------------------------------------------------------------------------
// Issue #133 PR 7: stray-character `/X/` recovery
// ---------------------------------------------------------------------------
//
// The `try_collapse_stray_char_slash` pass walks the fuzzy-corrected
// text looking for the `<alnum>/<single_alnum_char>/<alnum>` pattern
// and emits three candidate transforms (drop X, right-attach X to
// next token, left-attach X to previous token). Step 3a's
// `TokenKind::Unknown` filter is the natural disambiguator: only the
// transform that produces fully-recognized tokens survives.
//
// PR 7 also briefly experimented with lowering `MIN_FUZZY_LEN` from
// 3 to 2 to recover `UK→TK`-style 2-char tail typos, but reverted
// because the canonical Enron-corpus SAR fixture has `RB` as a
// standalone 2-char sub-compartment token and `RB` is at edit
// distance 1 from `RS` (the RSEN portion form) — so 2-char fuzzy
// silently corrupted SAR sub-compartments into dissem controls.
// Net 4 SAR-shape regressions vs 1 UK→TK win. The `MIN_FUZZY_LEN`
// doc in `crates/core/src/fuzzy.rs` carries the full rationale.
//
// Three integration tests below pin a named fixture from the SC-004
// mangled corpus per recovery branch (drop / right-attach /
// left-attach), so the harness's `Typo`-class rate movement
// (50.0% → 56.9% post-PR-7; 65→74/130, +9 fixtures) is anchored to
// specific recovery shapes rather than an opaque aggregate.

#[test]
fn typo_drop_stray_r_resolves_via_collapse_stray_char_slash() {
    // Pinned fixture: `tests/fixtures/mangled/typo/7885156a2c2c125f.json`
    // (`SECRET//NOFORN/R/EXDIS`). The drop-X transform produces
    // canonical `SECRET//NOFORN//EXDIS`; the right-attach
    // (`...//REXDIS`) and left-attach (`...//NOFORNR//EXDIS`)
    // candidates contain Unknown tokens and are filtered by step 3a.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//NOFORN/R/EXDIS", &deep_cx()) else {
        panic!("`/R/` between NOFORN and EXDIS must resolve via drop-X");
    };
    assert_eq!(effective_level(&marking), Some(Classification::Secret));
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"TOP SECRET//SI/N/OFORN", &deep_cx()) else {
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
        marking.0.dissem_controls.contains(&DissemControl::Nf),
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRE/T/REL TO USA, AUS, GBR", &deep_cx())
    else {
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
// Issue #133 PR 8: 3-char classification typo recovery
// ---------------------------------------------------------------------------
//
// Two complementary recovery paths added in PR 8:
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
// SC-004 movement: Typo class 56.9% → 69.2% (+12.3 pp, +16
// fixtures); aggregate 78.1% → 84.2% (+6.1 pp). Five named
// integration tests below pin the canonical fixture for each
// recovery branch.

#[test]
fn typo_tpp_resolves_via_top_vocab_addition() {
    // Pinned fixture family: `tests/fixtures/mangled/typo/ed06b49d58c3c389.json`
    // (`TPP SECRET//SI//NOFORN`). PR 8 adds bare `TOP` to the fuzzy
    // correction vocab; standard dist-1 fuzzy then handles
    // `TPP→TOP`. Strict parser re-joins `TOP SECRET` into the
    // canonical multi-word classification.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"TPP SECRET//SI//NOFORN", &deep_cx()) else {
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
        marking.0.dissem_controls.contains(&DissemControl::Nf),
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
        let Parsed::Unambiguous(marking) = rx.recognize(input, &deep_cx()) else {
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
    // Confidence::rule capped at 0.80).
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"OTP SECRET//SI//NOFORN", &deep_cx()) else {
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
    // PR 8's 2-char heuristic extension. `TP`/`TO` at the leading
    // classification slot map to `TOP` (the elided-middle-O and
    // elided-trailing-P cases respectively). Bare `TP`/`TO` have
    // no other canonical CAPCO meaning, so the heuristic isn't
    // ambiguous in practice.
    let rx = DecoderRecognizer::new();
    for input in &[
        b"TP SECRET//SI//NOFORN".as_slice(),
        b"TO SECRET//SI//NOFORN".as_slice(),
    ] {
        let Parsed::Unambiguous(marking) = rx.recognize(input, &deep_cx()) else {
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
    // where the `S` of `SECRET` migrated to the end of `TOP`. PR 8
    // recovers because `TOPS` (4 chars) fuzzy-matches `TOP` (3 chars)
    // at edit distance 1 (delete trailing `S`), and `ECRET` (5 chars)
    // fuzzy-matches `SECRET` (6 chars) at edit distance 1 (insert
    // leading `S`). The strict parser then re-joins them as
    // `TOP SECRET`.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"TOPS ECRET//SI//NOFORN", &deep_cx()) else {
        panic!("`TOPS ECRET//...` must resolve via TOP+SECRET vocab fuzzy");
    };
    assert_eq!(effective_level(&marking), Some(Classification::TopSecret));
}

// ---------------------------------------------------------------------------
// Issue #133 PR 9: REL TO structural repair (preprocessing)
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
// trigraph-guarded: the fix only fires when `is_trigraph(joined)`
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//REL OT USA, AUS, GBR", &deep_cx())
    else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//RELT O USA, AUS, GBR", &deep_cx())
    else {
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
    // the 4-character entry `A US` → `AUS` only when `is_trigraph`
    // confirms the joined 3-letter string is a valid country code.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//REL TO USA,A US, GBR", &deep_cx())
    else {
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
    // when `is_trigraph(AU + S)` (i.e., AUS) is true AND
    // `is_trigraph(AU)` is false — the trigraph guard excludes
    // false-positive shapes like `EU,S USA` where the comma is
    // between the valid 2-char EU and a separate entry.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//REL TO USA, AU,S GBR", &deep_cx())
    else {
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
    // and block-level invariants. PR 9's structural repair is
    // intentionally narrower: it only touches literal-shape
    // patterns and trigraph-joinable tokens. AUT in a valid
    // position must round-trip unchanged.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//REL TO USA, AUT", &deep_cx()) else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//REL TO USB, AUS, GBR", &deep_cx())
    else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//REL TO USA, ASU, GBR", &deep_cx())
    else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"SECRET//REL TO SA, AUS, GBR", &deep_cx())
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
// Issue #133 PR 2: position-aware short-token classification heuristic
// ---------------------------------------------------------------------------
//
// The keyboard-proximity heuristic resolves 1- and 2-character typos
// in the leading classification slot of portion or banner markings —
// shapes the vocab-based fuzzy matcher cannot touch (`MIN_FUZZY_LEN
// = 3`). The recognizer flags these with
// `FixSource::DecoderClassificationHeuristic` so the engine emits
// `Severity::Warn` (always-visible in `--check`) and caps
// `Confidence::rule` at 0.80 (below the default 0.95 threshold —
// auto-applies only with explicit user opt-in via lower threshold).
//
// The Enron-corpus-derived mangled-fixture tree at
// `tests/fixtures/mangled/typo/` contains very few short-leading-
// classification typos (mostly 3+ char tail-token typos like UK→TK,
// USAR→SAR), so the SC-004 harness's per-class rate doesn't move
// from this heuristic alone. These integration tests pin the
// heuristic's behavior on synthetic inputs that exercise the
// keyboard-proximity table directly. A follow-up PR adding fixtures
// for this typo class will make the SC-004 movement measurable.

#[test]
fn heuristic_2char_ts_decodes_portion() {
    // (YS//NF) — `YS` is the keyboard-proximity-typo for `TS`
    // (Y is one key left of T on QWERTY, S is unchanged). Outside
    // the heuristic's scope it would be left as-is and the strict
    // parse would land with classification=None (YS doesn't
    // resolve), making the candidate fail the engine's expected-
    // attrs equality.
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(YS//NF)", &deep_cx()) else {
        panic!("(YS//NF) should resolve to (TS//NF) via the heuristic");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::TopSecret),
        "YS leading position should heuristic-fix to TS; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "NF must survive the heuristic-corrected canonicalization"
    );
}

#[test]
fn heuristic_1char_s_decodes_portion() {
    // (W//NF) — `W` is QWERTY-adjacent to S (one key above).
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(W//NF)", &deep_cx()) else {
        panic!("(W//NF) should resolve to (S//NF) via the heuristic");
    };
    assert_eq!(
        effective_level(&marking),
        Some(Classification::Secret),
        "W leading position should heuristic-fix to S; attrs = {:?}",
        marking.0,
    );
    assert!(
        marking.0.dissem_controls.contains(&DissemControl::Nf),
        "NF must survive"
    );
}

#[test]
fn heuristic_1char_c_decodes_portion() {
    // (V//NF) — V is QWERTY-adjacent to C (one key right).
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(V//NF)", &deep_cx()) else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"RS//NOFORN", &deep_cx()) else {
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
    // (PR 2 plumbing) — without it the engine would treat the
    // fix the same as a vocab-based decoder fix, which would
    // (a) auto-apply at default threshold and (b) show as
    // `Severity::Fix` instead of `Severity::Warn`, defeating the
    // fix-and-warn intent.
    use marque_rules::FixSource;
    let rx = DecoderRecognizer::new();
    let Parsed::Unambiguous(marking) = rx.recognize(b"(YS//NF)", &deep_cx()) else {
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
    if let Parsed::Unambiguous(marking) = rx.recognize(b"(S//NF)", &deep_cx())
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
// SCI delimiter recovery (issue #198 — #133 PR 10)
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//HCSP)", &deep_cx()) else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//SITK)", &deep_cx()) else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//SI-TK)", &deep_cx()) else {
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
    let Parsed::Unambiguous(marking) = rx.recognize(b"(S//SI-G)", &deep_cx()) else {
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
