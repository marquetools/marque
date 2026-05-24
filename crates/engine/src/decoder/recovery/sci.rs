// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI delimiter recovery.
//!
//! Insert the `-` separator between an SCI control system and its
//! first compartment when the input wrote them as a glued token
//! (`HCSP` → `HCS-P`) and similar shape recoveries on SCI tokens.

use marque_ism::{DissemControl, NonIcDissem, SciControl, SciControlBare, marking_forms};

// ---------------------------------------------------------------------------
// SCI delimiter recovery
// ---------------------------------------------------------------------------

/// SCI delimiter recovery preprocessing — issues #198 and #133.
///
/// Repairs three classes of SCI delimiter typos against the closed
/// CVE vocabulary in `CVEnumISMSCIControls.xml`. Vocabulary checks
/// dispatch through the build-time-generated [`SciControlBare::parse`]
/// (bare control systems) and [`SciControl::parse`] (the full CVE set
/// including all registered control-compartment compounds), so the
/// repair surface tracks ODNI schema updates automatically — no
/// hand-maintained vocabulary slice to drift out of sync per
/// Constitution IV (Layer 1 generated predicates):
///
/// - **Pattern A (concatenated compound)**: a token equal to a compound
///   with the hyphen removed → canonical hyphenated form. `HCSP →
///   HCS-P`, `SIG → SI-G`, `TKKAND → TK-KAND`, etc.
/// - **Pattern B (concatenated bare control systems)**: a token of
///   length 4–6 that splits cleanly into two bare control systems →
///   slash-joined form (`SITK → SI/TK`, `HCSSI → HCS/SI`) per §A.6
///   p16 and the `TOP SECRET//ANB/SI/TK/XNB//NOFORN` example on p194.
///   Ambiguous splits bail out — see [`repair_sci_token`] for the
///   guard.
/// - **Pattern C (wrong delimiter)**: a token of the form
///   `<bare_cs>-<bare_cs>` that is NOT itself a registered compound →
///   slash-joined form. `SI-TK → SI/TK` (because `SI-TK` is not
///   registered), but `SI-G` is left alone (it IS registered — `-` is
///   the correct control-compartment separator per §A.6 p16).
/// - **Pattern D (intra-SCI `/` promotion at category boundary)** —
///   issue #720. When the current token is SCI-shaped (parses as
///   `SciControl`, `SciControlBare`, or — post-Pattern-B — splits
///   cleanly into two bare control systems) AND the following
///   delimiter is a single `/` (not `//`) AND the NEXT token is a
///   known non-SCI hard splitter (`DissemControl::parse(...)`,
///   `NonIcDissem::parse(...)`, or their single-word banner-abbreviation
///   equivalents via `marking_forms::banner_to_portion` — the next token
///   is one whitespace-delimited word, so multi-word long-form titles
///   are out of scope; see `is_non_sci_hard_splitter_token`), the single
///   `/` is promoted to
///   `//` — the canonical category separator per CAPCO-2016 §A.6 p16.
///   Fires standalone: covers both inputs where Pattern A/B/C already
///   rewrote the SCI token (`HCSP/NOFORN → HCS-P//NOFORN`) and
///   inputs where the SCI control is already canonical and only the
///   delimiter is wrong (`HCS-P/NOFORN → HCS-P//NOFORN`,
///   `SI/NOFORN → SI//NOFORN`). The gate is "SCI + `/` + non-SCI",
///   NOT "any control + `/` + any other category" — the dissem/dissem
///   chain `ORCON/NOFORN` is left alone because ORCON is not SCI-
///   shaped (regression-guarded by
///   `missing_delimiter_top_secret_classification_then_dissem` in
///   `crates/engine/tests/decoder_recovery.rs`). Strict-parser path
///   is unchanged — E004's `SECRET//SI/NF` stray-slash detection,
///   covered by the `sci_mixed_category_slash_block_falls_through` test
///   in `crates/core/src/parser/tests/controls_tests.rs`, runs against
///   the strict parser and is unaffected: this recovery executes only
///   when the strict path could not recognize the input.
///
/// **Out of scope** — sub-compartment fuzzy recovery (`ABCE → ABCD`),
/// unregistered-compartment recovery, and any rewrite that would
/// require fuzz-correcting against agency-assigned codewords. Those
/// require operator-supplied vocab (issue #180) — the engine cannot
/// invent identifiers it doesn't know are valid (Constitution VIII).
///
/// **Architectural shape** mirrors `try_rel_to_structural_repair`
/// (issue #190): runs as preprocessing on the input string before
/// per-token fuzzy correction, returns `Some(repaired)` only when at
/// least one repair fired. The caller pushes a `BaseRateCommonMarking`
/// feature onto `delim_features` so every candidate derived from the
/// repaired text inherits the audit trace.
///
/// **Allocation behavior**: short-circuits without allocation when the
/// pre-check finds no SCI control system root in the text. The
/// per-token walk borrows the input until a fix actually fires.
pub(in crate::decoder) fn try_sci_delimiter_repair(text: &str) -> Option<String> {
    if !contains_any_sci_root(text) {
        return None;
    }

    // ASCII-only guard. The SCI control-system vocabulary
    // (`SciControlBare::ALL`) and the registered compound names
    // (`SciControl::ALL`) are pure ASCII, as are the delimiters this
    // function recognizes (`-`, `/`, `(`, `)`, space, tab, newline,
    // CR, comma). So any non-ASCII input cannot match any pattern;
    // bailing early avoids the byte-vs-char-boundary hazard that
    // would otherwise arise from indexing `text` with byte offsets.
    if !text.is_ascii() {
        return None;
    }

    let bytes = text.as_bytes();
    let mut result: Option<String> = None;
    let mut last_copied = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let at_boundary = i == 0
            || matches!(
                bytes[i - 1],
                b'/' | b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r' | b','
            );
        if !at_boundary {
            i += 1;
            continue;
        }

        let token_start = i;
        let token_end = bytes[token_start..]
            .iter()
            .position(|&b| matches!(b, b'/' | b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r' | b','))
            .map(|n| token_start + n)
            .unwrap_or(bytes.len());

        // Track whether the resolved token (original or Pattern A/B/C
        // repaired) is SCI-shaped, so Pattern D can decide whether to
        // promote a trailing single `/` to `//`. `repaired_text` is
        // `None` when no Pattern A/B/C repair fired; the original
        // bytes are then used both for the SCI-shape probe and for
        // (no-op) emission.
        let mut sci_shaped = false;
        let mut repaired_text: Option<String> = None;
        if token_start < token_end {
            let token = &text[token_start..token_end];
            repaired_text = repair_sci_token(token);
            let resolved = repaired_text.as_deref().unwrap_or(token);
            sci_shaped = is_sci_shaped(resolved);
        }

        // Pattern D — intra-SCI `/` promotion when the current token
        // is SCI-shaped, the following delimiter is exactly one `/`
        // (i.e., not already `//`), and the next token is a known
        // non-SCI hard splitter (DissemControl / NonIcDissem, in
        // either abbreviated or long form). Standalone — fires
        // whether or not Pattern A/B/C also rewrote the current
        // token. See issue #720.
        let mut promote_delim = false;
        if sci_shaped && token_end < bytes.len() && bytes[token_end] == b'/' {
            // Reject `//` — that's already the canonical category
            // separator; nothing to promote.
            let next_is_slash = bytes.get(token_end + 1).is_some_and(|&b| b == b'/');
            if !next_is_slash {
                let next_start = token_end + 1;
                let next_end = bytes[next_start..]
                    .iter()
                    .position(|&b| {
                        matches!(b, b'/' | b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r' | b',')
                    })
                    .map(|n| next_start + n)
                    .unwrap_or(bytes.len());
                if next_start < next_end {
                    let next_tok = &text[next_start..next_end];
                    // Predicate gate: the next token must be a known
                    // non-SCI hard splitter — a published abbreviated
                    // or long-form dissem / non-IC dissem control.
                    // Anything SCI-shaped, unknown, or country-list-
                    // shaped (e.g., trigraphs in a REL TO tail) MUST
                    // NOT trigger promotion: the same `/` is the
                    // legitimate intra-category separator there
                    // (`SI/TK`, `REL TO USA, FVEY/NF`).
                    if is_non_sci_hard_splitter_token(next_tok) {
                        promote_delim = true;
                    }
                }
            }
        }

        let token_repaired = repaired_text.is_some();
        if token_repaired || promote_delim {
            let r = result.get_or_insert_with(|| String::with_capacity(text.len() + 1));
            r.push_str(&text[last_copied..token_start]);
            if let Some(repaired) = repaired_text.as_deref() {
                r.push_str(repaired);
            } else {
                r.push_str(&text[token_start..token_end]);
            }
            last_copied = token_end;
            if promote_delim {
                // Inject only the SECOND `/` here. The original single
                // `/` is NOT copied at this point — `last_copied` was
                // just set to `token_end` (the index of that original
                // `/`), so it is emitted by the next copied source
                // segment: either the leading `r.push_str(&text[last_copied
                // ..token_start])` slice of a later rewrite iteration, or
                // — if no further rewrite fires — the final
                // `r.push_str(&text[last_copied..])` tail copy. Either
                // way the injected `/` lands immediately before that
                // original `/`, rendering the canonical `//`.
                r.push('/');
            }
        }

        // Advance past the token; the next iteration will re-check the
        // boundary before the byte after the delimiter (or terminate at
        // end-of-input).
        i = token_end + 1;
    }

    result.map(|mut r| {
        r.push_str(&text[last_copied..]);
        r
    })
}

/// Predicate for Pattern D's left side: returns `true` when `token`
/// names an SCI control system or SCI compound. Accepts:
///
/// 1. The full CVE compound set (`SciControl::parse`) — e.g.,
///    `HCS-P`, `SI-G`, `TK-KAND`, `BUR-BLG`.
/// 2. The bare control-system set (`SciControlBare::parse`) — e.g.,
///    `HCS`, `SI`, `TK`, `BUR`.
/// 3. Slash-joined chains of bare control systems (the canonical
///    Pattern B output shape) — e.g., `SI/TK`, `HCS/SI`. Recognized
///    by splitting on `/` and requiring every part to parse as a
///    bare control system.
///
/// Used only by the Pattern D promotion gate; deliberately narrow so
/// the SCI/non-SCI distinction Pattern D depends on cannot collapse.
fn is_sci_shaped(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    if SciControl::parse(token).is_some() || SciControlBare::parse(token).is_some() {
        return true;
    }
    if token.contains('/') {
        return token
            .split('/')
            .all(|part| SciControlBare::parse(part).is_some());
    }
    false
}

/// Predicate for Pattern D's right side: returns `true` when `token`
/// is a known non-SCI hard splitter — a published dissem control or
/// non-IC dissem in either the abbreviated portion form (`NF`, `OC`,
/// `XD`, `SBU`, ...) or its single-word banner abbreviation (`NOFORN`,
/// `ORCON`, `EXDIS`, ...). Single-word banner recognition routes through
/// `marque_ism::marking_forms::banner_to_portion` so the surface tracks
/// the canonical `MARKING_FORMS` table — matching the strict parser's
/// `parse_dissem_full_form` / `parse_non_ic_full_form` shape (those live
/// as `pub(super)` in `marque-core` so we re-derive them locally per
/// issue #720 preflight decision point #3).
///
/// **Scope — single word.** `token` is the one whitespace-delimited word
/// following the `/` (see the caller's scan in `try_sci_delimiter_repair`,
/// which stops at the first space/structural delimiter). Multi-word
/// long-form *titles* (e.g. NOFORN's "NOT RELEASABLE TO FOREIGN
/// NATIONALS") are therefore not recognized here — and a title lookup is
/// not needed: every dissem / non-IC control whose `title` differs from
/// its `banner` abbreviation is multi-word, so `title_to_portion` could
/// never resolve a single-word `token` to a dissem/non-IC portion (the
/// only single-word `title != banner` rows in `MARKING_FORMS` are SCI
/// compartments, which fail the dissem/non-IC gate below). Recovering a
/// misplaced `/` before a multi-word banner would require widening the
/// caller's scan; that is out of scope for issue #720.
fn is_non_sci_hard_splitter_token(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    if DissemControl::parse(token).is_some() || NonIcDissem::parse(token).is_some() {
        return true;
    }
    let portion = marking_forms::banner_to_portion(token);
    portion.is_some_and(|p| DissemControl::parse(p).is_some() || NonIcDissem::parse(p).is_some())
}

/// Cheap pre-check for [`try_sci_delimiter_repair`]: returns true when
/// the input contains at least one bare SCI control system identifier
/// as a substring. False positives just mean we walk the bytes and
/// return `None` — no correctness impact, only a performance
/// optimization for the overwhelmingly common case where the input has
/// no SCI category at all.
fn contains_any_sci_root(text: &str) -> bool {
    text.contains("HCS")
        || text.contains("KLM")
        || text.contains("MVL")
        || text.contains("RSV")
        || text.contains("BUR")
        || text.contains("SI")
        || text.contains("TK")
}

/// Per-token classifier for SCI delimiter repair. Returns the repaired
/// token if one of patterns A/B/C matches; otherwise `None`.
///
/// All vocabulary checks dispatch through the build-time-generated
/// [`SciControlBare::parse`] and [`SciControl::parse`] (from
/// `marque-ism`'s generated `values.rs`), so the repair surface tracks
/// `CVEnumISMSCIControls.xml` automatically. New CVE compounds added
/// in a future ODNI schema bump (e.g., a hypothetical `SI-XYZ`) are
/// auto-discovered by Pattern A without any code change here.
///
/// Pattern dispatch order:
/// 1. Pattern A (split into bare-CS prefix + suffix; if
///    `{prefix}-{suffix}` is a registered CVE value, return it)
/// 2. Pattern C (token contains `-`, neither side is a registered
///    compound's compartment, both halves are bare CS)
/// 3. Pattern B (no `-`, splits into two bare CS, unambiguous)
fn repair_sci_token(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }

    // ASCII-only guard. The CVE vocabulary is pure ASCII, so a non-
    // ASCII token cannot match any pattern; bailing early ensures
    // the byte-offset slicing below (`token[..split]`,
    // `token[split..]`, `token[..dash_pos]`, `token[dash_pos + 1..]`)
    // never lands in the middle of a multi-byte UTF-8 sequence. This
    // is a defense-in-depth check — the only production caller
    // (`try_sci_delimiter_repair`) already gates on ASCII — but
    // keeping it here makes the function's invariant local and
    // self-evident for any future caller (e.g., a unit test).
    if !token.is_ascii() {
        return None;
    }

    let len = token.len();

    // Pattern A — concatenated registered compound. Walk every split
    // where the prefix is a bare control system; if `{prefix}-{suffix}`
    // is in the CVE vocabulary, return the canonical hyphenated form.
    // Bare CS lengths are 2 or 3; suffix length range comes from CVE
    // (max compartment-form suffix is 4 chars, e.g. TK-BLFH).
    if !token.contains('-') && (3..=8).contains(&len) {
        for &split in &[2usize, 3] {
            if split >= len {
                continue;
            }
            let prefix = &token[..split];
            let suffix = &token[split..];
            if SciControlBare::parse(prefix).is_some() {
                let canonical = format!("{prefix}-{suffix}");
                if SciControl::parse(&canonical).is_some() {
                    return Some(canonical);
                }
            }
        }
    }

    // Pattern C — wrong delimiter (`-` between two bare CS). Skip if
    // the whole token is itself a registered CVE compound.
    if let Some(dash_pos) = token.find('-') {
        if SciControl::parse(token).is_some() {
            return None;
        }
        let prefix = &token[..dash_pos];
        let suffix = &token[dash_pos + 1..];
        if SciControlBare::parse(prefix).is_some() && SciControlBare::parse(suffix).is_some() {
            return Some(format!("{prefix}/{suffix}"));
        }
        return None;
    }

    // Pattern B — concatenated bare control systems (no delimiter).
    // Bare CS lengths are 2 or 3; the concatenation is therefore in
    // [4..=6]. Try splits at positions 2 and 3 (the only split points
    // that can yield two valid bare-CS halves) and require an
    // unambiguous match.
    if !(4..=6).contains(&len) {
        return None;
    }
    let mut found: Option<(&str, &str)> = None;
    for &split in &[2usize, 3] {
        if split >= len {
            continue;
        }
        let suffix_len = len - split;
        if !(2..=3).contains(&suffix_len) {
            continue;
        }
        let prefix = &token[..split];
        let suffix = &token[split..];
        if SciControlBare::parse(prefix).is_some() && SciControlBare::parse(suffix).is_some() {
            if found.is_some() {
                return None;
            }
            found = Some((prefix, suffix));
        }
    }
    found.map(|(p, s)| format!("{p}/{s}"))
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
    fn sci_delimiter_repair_concatenated_compound_hcsp() {
        // Pattern A: `HCSP` (registered compound `HCS-P` with hyphen
        // missing) → `HCS-P`.
        let result = try_sci_delimiter_repair("SECRET//HCSP//NOFORN");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//HCS-P//NOFORN"),
            "HCSP must rewrite to HCS-P (CVE-registered compound)",
        );
    }

    #[test]
    fn sci_delimiter_repair_concatenated_compound_hcso() {
        // Pattern A: HCSO → HCS-O.
        let result = try_sci_delimiter_repair("SECRET//HCSO//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//HCS-O//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_concatenated_compound_sig() {
        // Pattern A: SIG → SI-G. The CVE list has SI-G; G is a
        // compartment of SI per §A.6 p16.
        let result = try_sci_delimiter_repair("SECRET//SIG//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//SI-G//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_concatenated_compound_tkkand() {
        // Pattern A: TKKAND → TK-KAND. Tests that the longer
        // concatenated forms (6 chars) are matched correctly.
        let result = try_sci_delimiter_repair("SECRET//TKKAND//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//TK-KAND//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_schema_coverage_bur_compounds() {
        // Pattern A is schema-driven via `SciControl::parse`, so it
        // covers every CVE compound automatically — including the
        // BUR-* family that an earlier hand-maintained list omitted.
        // Locks in the schema-derived contract: any future ODNI
        // schema bump that adds new compounds is auto-covered without
        // changes to this file.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//BURBLG//NOFORN").as_deref(),
            Some("SECRET//BUR-BLG//NOFORN"),
        );
        assert_eq!(
            try_sci_delimiter_repair("SECRET//BURDTP//NOFORN").as_deref(),
            Some("SECRET//BUR-DTP//NOFORN"),
        );
        assert_eq!(
            try_sci_delimiter_repair("SECRET//BURWRG//NOFORN").as_deref(),
            Some("SECRET//BUR-WRG//NOFORN"),
        );
    }

    #[test]
    fn sci_delimiter_repair_missing_slash_sitk() {
        // Pattern B: SITK → SI/TK. Per §A.6 p16 + p194 example,
        // multiple control systems within an SCI category use `/`.
        let result = try_sci_delimiter_repair("SECRET//SITK//NOFORN");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//SI/TK//NOFORN"),
            "SITK must rewrite to SI/TK (two bare control systems concatenated)",
        );
    }

    #[test]
    fn sci_delimiter_repair_missing_slash_hcssi() {
        // Pattern B: HCSSI → HCS/SI. Tests 3+2 split (HCS is len 3,
        // SI is len 2).
        let result = try_sci_delimiter_repair("SECRET//HCSSI//NOFORN");
        assert_eq!(result.as_deref(), Some("SECRET//HCS/SI//NOFORN"));
    }

    #[test]
    fn sci_delimiter_repair_wrong_delimiter_si_dash_tk() {
        // Pattern C: SI-TK → SI/TK. The whole token is not a CVE
        // compound, both halves are bare CS, so `-` is wrong.
        let result = try_sci_delimiter_repair("SECRET//SI-TK//NOFORN");
        assert_eq!(
            result.as_deref(),
            Some("SECRET//SI/TK//NOFORN"),
            "SI-TK must rewrite to SI/TK (two bare CS, `-` is for control-compartment)",
        );
    }

    #[test]
    fn sci_delimiter_repair_leaves_registered_compound_alone() {
        // Pattern C must NOT fire on registered compounds. SI-G is in
        // CVEnumISMSCIControls.xml — `-` is the correct separator.
        assert!(try_sci_delimiter_repair("SECRET//SI-G//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//HCS-P//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//TK-KAND//NOFORN").is_none());
    }

    #[test]
    fn sci_delimiter_repair_returns_none_on_canonical() {
        // Already-canonical inputs round-trip unchanged.
        assert!(try_sci_delimiter_repair("SECRET//SI/TK//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//SI//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("SECRET//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("").is_none());
    }

    #[test]
    fn sci_delimiter_repair_does_not_fire_on_word_substring() {
        // SIGMA contains "SIG" as a substring but is a single token
        // — Pattern A requires whole-token equality, not contains.
        assert!(try_sci_delimiter_repair("SIGMA").is_none());
        // SITE, SITS — same protection.
        assert!(try_sci_delimiter_repair("SITE").is_none());
        // SIGNAL — contains SIG; whole token is not in Pattern A.
        assert!(try_sci_delimiter_repair("SIGNAL").is_none());
    }

    #[test]
    fn sci_delimiter_repair_short_circuits_without_sci_root() {
        // Pre-check: no SCI control system substring → no repair.
        assert!(try_sci_delimiter_repair("CONFIDENTIAL//NOFORN").is_none());
        assert!(try_sci_delimiter_repair("(C)").is_none());
        assert!(try_sci_delimiter_repair("").is_none());
    }

    #[test]
    fn sci_delimiter_repair_does_not_panic_on_non_ascii() {
        // The function must not panic on multi-byte UTF-8 input. The
        // SCI vocabulary is pure ASCII, so any non-ASCII input is
        // unmatchable — bail early rather than risk a byte-offset
        // slice landing mid-codepoint. Inputs intentionally chosen
        // to exercise both the outer scanner (`try_sci_delimiter_repair`)
        // and the inner per-token classifier (`repair_sci_token`).
        assert!(try_sci_delimiter_repair("SECRET//SI/TK//日本語").is_none());
        assert!(try_sci_delimiter_repair("Ω SI TK").is_none());
        assert!(try_sci_delimiter_repair("こんにちは").is_none());
        // Direct call to the per-token helper with non-ASCII content.
        assert!(repair_sci_token("SI日").is_none());
        assert!(repair_sci_token("日本").is_none());
    }

    #[test]
    fn repair_sci_token_rejects_partial_decompositions() {
        // HCSI = HCS+I (I not bare) or H+CSI (neither bare) — no
        // valid Pattern B decomposition.
        assert!(repair_sci_token("HCSI").is_none());
        // ABCDE — random, no valid CS decomposition.
        assert!(repair_sci_token("ABCDE").is_none());
        // BUR alone — bare CS by itself, len 3, fails Pattern B's
        // 4..=6 length check, no `-`, not in Pattern A. Returns None.
        assert!(repair_sci_token("BUR").is_none());
    }

    // ---------------------------------------------------------------
    // Pattern D — intra-SCI `/` promotion (issue #720)
    // ---------------------------------------------------------------

    #[test]
    fn pattern_d_promotes_canonical_sci_slash_dissem() {
        // (b) standalone: SCI already canonical, only the delimiter
        // is wrong. `HCS-P/NOFORN` → `HCS-P//NOFORN`.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//HCS-P/NOFORN").as_deref(),
            Some("SECRET//HCS-P//NOFORN"),
            "canonical HCS-P followed by single `/` then NOFORN must \
             promote to category `//`",
        );
        // Bare SCI control system + single-slash + abbrev dissem.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//SI/NOFORN").as_deref(),
            Some("SECRET//SI//NOFORN"),
        );
        // Bare SCI + single-slash + non-IC dissem (long form).
        assert_eq!(
            try_sci_delimiter_repair("SECRET//TK/EXDIS").as_deref(),
            Some("SECRET//TK//EXDIS"),
        );
    }

    #[test]
    fn pattern_d_promotes_after_pattern_a_repair() {
        // (a) composed with Pattern A: HCSP → HCS-P AND the trailing
        // `/NOFORN` → `//NOFORN`. The single output covers both
        // repairs — the canonical decoder-recovery shape for the bug
        // class that motivated issue #720.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//HCSP/NOFORN").as_deref(),
            Some("SECRET//HCS-P//NOFORN"),
            "HCSP → HCS-P AND trailing single `/` → `//` (Pattern A + D)",
        );
    }

    #[test]
    fn pattern_d_promotes_after_pattern_b_repair() {
        // Pattern B output (SI/TK) is itself SCI-shaped — the last
        // bare CS in the chain (TK) is what abuts the trailing
        // delimiter; the predicate accepts the slash-joined chain
        // wholesale via `is_sci_shaped`.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//SITK/NOFORN").as_deref(),
            Some("SECRET//SI/TK//NOFORN"),
            "SITK → SI/TK AND trailing single `/` → `//` (Pattern B + D)",
        );
    }

    #[test]
    fn pattern_d_promotes_after_pattern_c_repair() {
        // Pattern C output (SI/TK from SI-TK) — same as B output
        // shape, plus a trailing single `/` to NOFORN.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//SI-TK/NOFORN").as_deref(),
            Some("SECRET//SI/TK//NOFORN"),
            "SI-TK → SI/TK AND trailing single `/` → `//` (Pattern C + D)",
        );
    }

    #[test]
    fn pattern_d_does_not_promote_sci_then_sci() {
        // SCI followed by SCI uses the SAME `/` separator as Pattern
        // B output (intra-category delimiter, §A.6 p16). The gate
        // requires the right side to be a non-SCI hard splitter; SI
        // followed by TK must leave the `/` alone.
        // (Canonical input — no rewrite at all.)
        assert!(try_sci_delimiter_repair("SECRET//SI/TK//NOFORN").is_none());
        // And the only rewrite for `SI/TK/NOFORN` is to promote the
        // `/` before NOFORN (after the second SCI token TK), not the
        // one between SI and TK.
        assert_eq!(
            try_sci_delimiter_repair("SECRET//SI/TK/NOFORN").as_deref(),
            Some("SECRET//SI/TK//NOFORN"),
            "only the SCI→non-SCI boundary promotes; SCI→SCI stays \
             single-slash",
        );
    }

    #[test]
    fn pattern_d_does_not_promote_dissem_then_dissem() {
        // ORCON is a dissem long form; NOFORN is a dissem long form.
        // The gate (left side requires SCI-shape) must not fire.
        // This is the regression guard against breaking
        // `decoder_recovery::missing_delimiter_top_secret_classification_then_dissem`.
        assert!(try_sci_delimiter_repair("TOP SECRET//ORCON/NOFORN").is_none());
        // Even when the SCI substring `HCS` appears earlier in the
        // text (so the cheap pre-check passes), the boundary between
        // ORCON and NOFORN must not be touched.
        let out = try_sci_delimiter_repair("TOP SECRET//HCS-P INTEL OPS ORCON/NOFORN");
        assert!(
            out.is_none(),
            "ORCON/NOFORN is a dissem/dissem chain — Pattern D must \
             not promote it; got: {out:?}",
        );
    }

    #[test]
    fn pattern_d_leaves_canonical_double_slash_alone() {
        // The standard `(S//HCS-P)` / `(S//HCSP)` no-trailing-dissem
        // shapes that the existing recovery suite covers must not
        // gain a Pattern D promotion — the right boundary is `)`,
        // not `/`.
        assert!(try_sci_delimiter_repair("(S//HCS-P)").is_none());
        assert!(try_sci_delimiter_repair("(S//SI/TK)").is_none());
    }

    #[test]
    fn pattern_d_leaves_rel_to_country_chain_alone() {
        // `REL TO USA, FVEY/NF` — the `/` in `FVEY/NF` is the REL TO
        // trailing-dissem separator, NOT an SCI boundary. SCI doesn't
        // appear in this input at all; the cheap pre-check should
        // bail out before any walking happens. Belt-and-suspenders
        // assertion that no false promotion sneaks through.
        assert!(try_sci_delimiter_repair("SECRET//REL TO USA, FVEY/NF").is_none());
    }

    #[test]
    fn pattern_d_does_not_promote_sci_then_unknown() {
        // `SI/GARBAGE` — GARBAGE is not a known non-SCI hard
        // splitter (DissemControl/NonIcDissem don't accept it).
        // Leave the `/` alone; the decoder's other passes can take
        // a second crack at it.
        assert!(try_sci_delimiter_repair("SECRET//SI/GARBAGE").is_none());
    }
}
