// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Stray-character `/X/` recovery.
//!
//! Collapse `(S/X/NF)` / `(TS / X / NF)` style inputs where a stray
//! single character was wedged between slashes. Returns the set of
//! candidate strings produced by attempting the collapse, deduplicated
//! upstream.

// ---------------------------------------------------------------------------
// Stray-character `/X/` recovery
// ---------------------------------------------------------------------------

/// Walk `text` looking for the `<alnum>/<single_alnum_char>/<alnum>`
/// pattern. For each match (currently only the first match is
/// processed — see "scope" below) emit three candidate transforms:
///
/// 1. **Drop X** — `A/X/B` → `A//B`. Recovers stray characters
///    inserted between two valid tokens. Example:
///    `SECRET//NOFORN/R/EXDIS` → `SECRET//NOFORN//EXDIS` (the stray
///    `/R/` between NOFORN and EXDIS is removed).
///
/// 2. **Right-attach X** — `A/X/B` → `A//XB`. Recovers a single
///    character that got separated from the start of the right
///    token by a `/`. Example: `TOP SECRET//SI/N/OFORN` →
///    `TOP SECRET//SI//NOFORN` (the `N` was the leading character
///    of `NOFORN`).
///
/// 3. **Left-attach X** — `A/X/B` → `AX//B`. Recovers a single
///    character that got separated from the end of the left token
///    by a `/`. Example: `SECRE/T/REL TO USA, AUS, GBR` →
///    `SECRET//REL TO USA, AUS, GBR` (the `T` was the trailing
///    character of `SECRET`).
///
/// All three transforms are emitted as candidates; the recognizer's
/// step-3a [`TokenKind::Unknown`](marque_ism::TokenKind::Unknown)
/// filter is the natural disambiguator. For each input only one of
/// the three transforms produces fully-recognized tokens — the
/// other two leave broken-token fragments (`OFORN`, `NOFORNR`,
/// `SECRER`, …) that survive strict parsing as `TokenKind::Unknown`
/// and get dropped before scoring. The decoder doesn't need a
/// per-pattern lookup table to choose the right transform; the
/// vocab does the choosing implicitly.
///
/// # Scope
///
/// Only the FIRST `/X/` match in the input is processed; an input
/// with multiple stray-character patterns (e.g., `S/I/T/K`) is not
/// fully recovered by a single pass. The current corpus has very
/// few multi-pattern inputs (1–2 in the unresolved Typo set), and
/// adding a multi-pass loop here would complicate the candidate cap
/// in [`generate_candidate_bytes`] without proportional benefit. The
/// pass can iterate later if multi-pattern recovery becomes
/// load-bearing for SC-004 movement.
///
/// # Pattern boundary requirements
///
/// The `/X/` match requires alphanumeric context on both sides
/// (`<alnum>/<X>/<alnum>`). Without those guards the pattern would
/// fire on edge cases like `(/X/)` (start of portion form) where
/// the surrounding context is structural punctuation, not a token —
/// the recovery would be semantically meaningless there because
/// there's no token to attach `X` to.
pub(in crate::decoder) fn try_collapse_stray_char_slash(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        // `/X/` shape: bytes[i] = `/`, bytes[i+1] = single ASCII
        // alnum, bytes[i+2] = `/`. The single-alnum requirement
        // prevents matching on `/AB/` (which would be a 2-char
        // token between slashes, not a stray character).
        if bytes[i] != b'/' || !bytes[i + 1].is_ascii_alphanumeric() || bytes[i + 2] != b'/' {
            i += 1;
            continue;
        }
        // Boundary check: the slashes must be sandwiched between
        // alphanumeric tokens on both sides. Without this guard
        // `(/X/)` (start-of-portion-form) would trip the match.
        let prev_alnum = i > 0 && bytes[i - 1].is_ascii_alphanumeric();
        let next_alnum = i + 3 < bytes.len() && bytes[i + 3].is_ascii_alphanumeric();
        if !prev_alnum || !next_alnum {
            i += 1;
            continue;
        }

        let x = bytes[i + 1];
        let prefix = &bytes[..i];
        let suffix = &bytes[i + 3..];

        // The unwraps are safe: `text` is valid UTF-8, `prefix` /
        // `suffix` are slices on byte boundaries (the pattern only
        // matched on ASCII bytes), and we only insert ASCII bytes
        // (`/`, `x` which is ASCII alnum) between them.
        let mut out = Vec::with_capacity(3);

        // 1. Drop X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.extend_from_slice(b"//");
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        // 2. Right-attach X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.extend_from_slice(b"//");
        buf.push(x);
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        // 3. Left-attach X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.push(x);
        buf.extend_from_slice(b"//");
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        return out;
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unused_imports)]
mod tests {
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
    use crate::decoder::test_helpers::{TEST_SCHEME, deep_cx};

    #[test]
    fn try_collapse_stray_char_slash_emits_three_transforms() {
        // Each `/X/` match emits exactly three candidate bytes
        // (drop, right-attach, left-attach). This pins the contract
        // and makes any future scope expansion (multi-pass, extra
        // transforms) a deliberate, reviewable change.
        let result = try_collapse_stray_char_slash("AB/X/CD");
        assert_eq!(result.len(), 3, "expected 3 candidates; got {result:?}");
        assert_eq!(result[0], "AB//CD"); // drop X
        assert_eq!(result[1], "AB//XCD"); // right-attach X to CD
        assert_eq!(result[2], "ABX//CD"); // left-attach X to AB
    }

    #[test]
    fn try_collapse_stray_char_slash_returns_empty_when_no_pattern() {
        // Inputs without a `/X/` pattern produce no candidates.
        let cases: &[&str] = &[
            "SECRET",
            "SECRET//NOFORN",
            "SECRET//NOFORN//EXDIS",
            "(C)",
            "",
            // A `/` followed by 2+ alnum chars is NOT the pattern —
            // `/AB/` is a regular 2-char token between slashes.
            "SECRET/AB/CD",
            // `//` (canonical separator) doesn't match because the
            // single-char-between-slashes shape requires alnum at
            // bytes[i+1].
            "SECRET////NOFORN",
        ];
        for input in cases {
            assert!(
                try_collapse_stray_char_slash(input).is_empty(),
                "input {input:?} should not match /X/ pattern",
            );
        }
    }

    #[test]
    fn try_collapse_stray_char_slash_requires_alnum_boundary() {
        // The pattern requires alnum on both sides of `/X/`. Without
        // both, the recovery is semantically meaningless (no token
        // to attach X to / no token next to the strip).
        // Leading boundary missing: `/X/Y` at position 0 has no
        // alnum at i-1.
        assert!(try_collapse_stray_char_slash("/X/Y").is_empty());
        // Trailing boundary missing: `Y/X/` has no alnum at i+3.
        assert!(try_collapse_stray_char_slash("Y/X/").is_empty());
        // Both alnum: matches.
        assert_eq!(
            try_collapse_stray_char_slash("Y/X/Z").len(),
            3,
            "alnum on both sides should match"
        );
    }

    #[test]
    fn try_collapse_stray_char_slash_processes_only_first_match() {
        // Scope: only the first `/X/` is processed. Multi-pattern
        // inputs need a future multi-pass extension.
        let result = try_collapse_stray_char_slash("A/X/B/Y/C");
        assert_eq!(result.len(), 3);
        // Each candidate carries only the first transform — the
        // second `/Y/` pattern is left in place verbatim.
        assert_eq!(result[0], "A//B/Y/C"); // drop first X
        assert_eq!(result[1], "A//XB/Y/C"); // right-attach first X
        assert_eq!(result[2], "AX//B/Y/C"); // left-attach first X
    }

    #[test]
    fn decoder_recovers_drop_stray_char() {
        // End-to-end: `SECRET//NOFORN/R/EXDIS` resolves to the
        // canonical `SECRET//NOFORN//EXDIS` via the drop-X transform.
        // The right-attach (`SECRET//NOFORN//REXDIS` — REXDIS unknown)
        // and left-attach (`SECRET//NOFORNR//EXDIS` — NOFORNR unknown)
        // candidates are dropped by step 3a's Unknown-token filter.
        // Pinned per `tests/fixtures/mangled/typo/7885156a2c2c125f.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) =
            rx.recognize(b"SECRET//NOFORN/R/EXDIS", 0, &*TEST_SCHEME, &deep_cx())
        else {
            panic!("`/R/` between NOFORN and EXDIS must resolve via drop-X");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::Secret),
        );
        assert!(
            marking
                .0
                .dissem_iter()
                .any(|d| matches!(d, marque_ism::DissemControl::Nf)),
            "NOFORN must survive; attrs = {:?}",
            marking.0,
        );
        assert!(
            marking
                .0
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Exdis)),
            "EXDIS must survive; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn decoder_recovers_right_attach_stray_char() {
        // End-to-end: `TOP SECRET//SI/N/OFORN` resolves to the
        // canonical `TOP SECRET//SI//NOFORN` via right-attach (the
        // `N` was the leading char of NOFORN). The drop candidate
        // (`TOP SECRET//SI//OFORN` — OFORN unknown) and left-attach
        // (`TOP SECRET//SIN//OFORN` — both unknown) are dropped by
        // step 3a's Unknown-token filter. Pinned per
        // `tests/fixtures/mangled/typo/2cb13fe4682ff31c.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) =
            rx.recognize(b"TOP SECRET//SI/N/OFORN", 0, &*TEST_SCHEME, &deep_cx())
        else {
            panic!("`/N/` before OFORN must resolve via right-attach");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::TopSecret),
        );
        assert!(
            marking
                .0
                .sci_controls
                .iter()
                .any(|c| matches!(c, marque_ism::SciControl::Si)),
            "SI must survive; attrs = {:?}",
            marking.0,
        );
        assert!(
            marking
                .0
                .dissem_iter()
                .any(|d| matches!(d, marque_ism::DissemControl::Nf)),
            "NOFORN must be reconstructed; attrs = {:?}",
            marking.0,
        );
    }

    #[test]
    fn decoder_recovers_left_attach_stray_char() {
        // End-to-end: `SECRE/T/REL TO USA, AUS, GBR` resolves to the
        // canonical `SECRET//REL TO USA, AUS, GBR` via left-attach
        // (the `T` was the trailing char of SECRET). The drop
        // (`SECRE//REL TO ...` — SECRE unknown) and right-attach
        // (`SECRE//TREL TO ...` — both unknown) are dropped by
        // step 3a. Pinned per
        // `tests/fixtures/mangled/typo/cff1d0ac74e901c3.json`.
        let rx = DecoderRecognizer::new();
        let Parsed::Unambiguous(marking) = rx.recognize(
            b"SECRE/T/REL TO USA, AUS, GBR",
            0,
            &*TEST_SCHEME,
            &deep_cx(),
        ) else {
            panic!("`/T/` after SECRE must resolve via left-attach");
        };
        assert_eq!(
            marking
                .0
                .classification
                .as_ref()
                .map(|c| c.effective_level()),
            Some(Classification::Secret),
        );
        assert_eq!(
            marking.0.rel_to.len(),
            3,
            "REL TO must carry 3 trigraphs (USA, AUS, GBR); attrs = {:?}",
            marking.0,
        );
    }
}
