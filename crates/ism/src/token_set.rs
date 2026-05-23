// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time Aho-Corasick automaton over CVE token vocabulary.
//!
//! The automaton is built from all known CVE tokens at startup (via LazyLock)
//! and injected into the parser as a `TokenSet` implementation.

use aho_corasick::AhoCorasick;
use std::sync::LazyLock;

use crate::generated::values;
use crate::marking_forms::MARKING_FORMS;

/// Minimal interface the parser needs from the token set.
/// Implemented by `CapcoTokenSet`; injected at engine init.
pub trait TokenSet: Send + Sync {
    /// Returns the canonical token string if `token` is a known CVE value.
    fn canonicalize(&self, token: &str) -> Option<&'static str>;

    /// Returns true if `token` is a known country trigraph.
    fn is_trigraph(&self, token: &str) -> bool;

    /// Returns the vocabulary slice used for fuzzy correction lookups.
    ///
    /// This is the token vocabulary against which unknown tokens are compared
    /// by the `marque_core::fuzzy` module. Must be sorted and deduplicated
    /// (binary search is used for the "is already valid" check).
    ///
    /// The returned slice is borrowed from the implementor, which allows
    /// implementations to hold the vocabulary on `self` (e.g., in a `Vec`
    /// built at construction time) rather than in a global static. Each
    /// entry is `&'static str` because the fuzzy matcher returns canonical
    /// tokens with `'static` lifetime in `FuzzyCorrection::token`.
    ///
    /// The default implementation returns an empty slice, disabling fuzzy
    /// correction for external `TokenSet` implementors that do not override it.
    fn correction_vocab(&self) -> &[&'static str] {
        &[]
    }
}

/// Aho-Corasick automaton over all CVE tokens — built once from generated data.
static AUTOMATON: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::builder()
        .ascii_case_insensitive(false) // markings are case-sensitive
        .build(values::ALL_CVE_TOKENS)
        .expect("CVE token automaton construction failed")
});

/// Classification structural keywords not present as standalone
/// entries in `ALL_CVE_TOKENS`. Issue #133 PR 8.
///
/// `TOP SECRET` is in `ALL_CVE_TOKENS` as a single multi-word entry,
/// but the bare `TOP` is not — and the decoder's `scan_token`
/// tokenizer splits on whitespace, so an input like `TPP SECRET`
/// arrives at the fuzzy matcher as the standalone token `TPP` with
/// no `TOP` correction target available. Adding `TOP` to the fuzzy
/// vocab lets the standard edit-distance path recover the
/// `TPP→TOP`, `UOP→TOP`, `TDOP→TOP`, `QTOP→TOP`, `TOPW→TOP` family
/// of typos seen in the mangled corpus. The strict parser
/// then re-joins `TOP SECRET` into the canonical multi-word
/// classification.
///
/// Round-trip safety: a fuzzy-correction returning `TOP` for an
/// input typo lands as the bare token `TOP`, which the strict
/// parser combines with the following `SECRET` token into
/// `MarkingClassification::Us(Classification::TopSecret)` via the
/// usual two-word classification path. Round-trip pinned by the
/// PR-8 integration tests in `decoder_recovery.rs`.
const CLASSIFICATION_STRUCTURAL_KEYWORDS: &[&str] = &["TOP"];

/// NATO classification structural keywords not present in `ALL_CVE_TOKENS`.
///
/// NATO-specific classification words appear in multi-word forms that the
/// strict parser recognises: `COSMIC TOP SECRET`, `COSMIC TOP SECRET-BOHEMIA`,
/// `COSMIC TOP SECRET-BALK`, `COSMIC TOP SECRET ATOMAL`. Like `TOP` above,
/// the decoder's whitespace tokenizer splits these multi-word forms so each
/// word arrives individually at the fuzzy matcher. Without these entries in
/// the correction vocab, OCR/transcription typos (`COSMID`, `BOHFMIA`,
/// `ATOAML`, `BBLE`) produce `TokenKind::Unknown` spans and the decoder
/// discards the candidate.
///
/// Round-trip safety: the strict parser in `marque-core` recognises each
/// multi-word NATO form and maps it to the corresponding
/// `MarkingClassification::NonUs(NatoClassification::*)` variant, so a
/// fuzzy-corrected `COSMIC` / `BOHEMIA` / `ATOMAL` / `BALK` token lands
/// on the correct classification after strict parsing.
///
/// Authority: CAPCO-2016 §H.2 p55 (Non-US Protective Markings,
/// referring to Manual Appendix B for NATO Protective Markings).
/// Vocabulary anchor: §G.1 Table 4 p37, the COSMIC TOP SECRET / ATOMAL /
/// BALK / BOHEMIA rows under category "2. Non-US Protective Markings".
const NATO_CLASSIFICATION_KEYWORDS: &[&str] = &["ATOMAL", "BALK", "BOHEMIA", "COSMIC"];

/// SAR structural keywords (CAPCO-2016 §H.5 p100, "SAR-" indicator and
/// "SPECIAL ACCESS REQUIRED-" full form), included in the fuzzy
/// correction vocabulary so OCR/transcription typos in the indicator
/// keywords (`SPCIAL`, `CCESS`, `SEPCIAL`, etc.) get corrected to the
/// canonical form before the strict SAR parser's literal `starts_with`
/// matches in `crates/core/src/parser.rs::parse_sar_category` run.
///
/// These keywords are NOT in `ALL_CVE_TOKENS` because the ODNI
/// `CVEnumISMSAR.xml` is empty — SAR program identifiers are
/// agency-assigned codewords not centrally registered. The structural
/// SAR parser handles `SAR-`/`SPECIAL ACCESS REQUIRED-` as fixed
/// literal indicator strings, but the fuzzy matcher had no way to
/// recover a typo in those keywords because they weren't in any
/// vocabulary it consulted. Issue #133 PR 6.
///
/// `REQUIRED` is intentionally excluded: in real corpus inputs it is
/// always followed immediately by `-<program-nickname>` (e.g.,
/// `REQUIRED-BUTTER`), and the decoder's `scan_token` includes
/// internal hyphens in a single token, so `REQUIRED-BUTTER` is one
/// 14-character token that no fuzzy correction targeting `REQUIRED`
/// (8 chars) can reach within `MAX_EDIT_DISTANCE = 2`. Adding
/// `REQUIRED` would be a no-op for this hot path; if a future
/// fixture surfaces with `REQUIRED` as an isolated token (e.g.,
/// `SPECIAL ACCESS REQUIRED -BUTTER`), revisit. `SAR` is similarly
/// excluded because it is always glued to a program identifier
/// (`SAR-BP-J12`) — see `try_sar_indicator_repair` in
/// `crates/engine/src/decoder.rs` for the structural prefix-strip /
/// missing-hyphen path that handles `USAR-BP` / `SARBP`.
const SAR_STRUCTURAL_KEYWORDS: &[&str] = &["ACCESS", "SPECIAL"];

/// AEA and SCI structural keywords not present in `ALL_CVE_TOKENS`.
///
/// These individual words appear as components of multi-word Marking Titles
/// that the strict parser recognises. The decoder's whitespace tokeniser
/// splits them, so each word arrives at the fuzzy matcher independently. Without
/// these entries, OCR/transcription typos (`TAELNT`, `TALNET`, `FRMERLY`,
/// `KEYOLE`) produce `TokenKind::Unknown` spans that cause the decoder to
/// discard the candidate.
///
/// **TALENT / KEYHOLE** (§H.4 p73): The full Marking Title for TK is "TALENT
/// KEYHOLE". OCR commonly mangles individual words of long titles; having both
/// bare words in the vocab lets `TAELNT KEYHOLE` → `TALENT KEYHOLE` → `TK`.
///
/// **FORMERLY** (§H.6 p116): The full Marking Title for FRD is "FORMERLY
/// RESTRICTED DATA". A typo like `FRMERLY RESTRICTED DATA` arrives at the
/// fuzzy matcher as the token `FRMERLY`; without `FORMERLY` in the vocab the
/// decoder cannot recover it.
///
/// Round-trip safety: the strict parser already handles `TALENT KEYHOLE` →
/// `TK` and `FORMERLY RESTRICTED DATA` → `FRD` via `parse_sci_block` /
/// `title_to_portion` paths respectively, so fuzzy-corrected tokens land at
/// the expected parsed values without further changes.
///
/// Note: `NUCLEAR` (appears in CNWDI/TFNI/DOD-UCNI/DOE-UCNI titles) is
/// intentionally excluded — it is a very common English word and would produce
/// excessive false-positive fuzzy corrections on unrelated text.
const AEA_SCI_STRUCTURAL_KEYWORDS: &[&str] = &["FORMERLY", "KEYHOLE", "TALENT"];

/// NATO portion-form abbreviations not present in `ALL_CVE_TOKENS`.
///
/// Covers the five base-level [`crate::NatoClassification`] variants
/// (`NU`, `NR`, `NC`, `NS`, `CTS`) plus the five **legacy compound**
/// portion forms (`NCA`, `NSAT`, `CTSA`, `CTS-B`, `CTS-BALK`).
///
/// # Legacy compound forms
///
/// Per CAPCO-2016 §G.2 p40 + §H.7 p122, fused
/// classification+sub-marking forms are **structurally wrong** —
/// ATOMAL is an AEA-axis marking and BOHEMIA/BALK are NATO SAPs in the
/// SCI position. The parser canonicalizes the legacy text into bare
/// class + AEA/SCI companion at parse time. The legacy forms remain in
/// this list because:
///
/// 1. The fuzzy-correction pass operates on raw tokens before strict
///    parsing. `CTSA` is at edit-distance 1 from `CTS` — without it
///    in the no-fuzz vocabulary, `(//CTSA//NF)` would be silently
///    corrupted to `(//CTS//NF)` (losing the ATOMAL signal). Same
///    risk for `NCA` (vs `NC`) and `NSAT` (vs `NS`).
/// 2. The E066 autofix rule (in `marque-capco`) needs the strict
///    parser to ACCEPT these forms so it can detect them via raw
///    token spans and emit the canonical-text fix.
///
/// The five base-level forms are NOT emitted by ODNI CVE XML (which
/// records only vocabulary enum values, not derived portion forms).
/// Without them in the fuzzy-correction vocabulary,
/// `fuzzy_correct_tokens` cannot distinguish `CTS` from `TS`
/// (edit-distance 1) and silently rewrites — destroying the
/// NATO-longhand fold's output.
///
/// Round-trip safety: the strict parser in `marque-core` accepts all
/// 10 forms as valid non-US portion markings. The five base-level
/// forms map to a corresponding [`crate::NatoClassification`]
/// variant; the five legacy compound forms map to a `NatoClassification`
/// bare class + AEA/SCI companion write. Adding
/// these tokens to the correction vocab makes `FuzzyVocabMatcher::correct`
/// return `None` for exact-match inputs (binary-search fast path),
/// causing `fuzzy_correct_tokens` to pass them through unchanged
/// (Case 4 verbatim).
///
/// Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (portion-form column);
/// §G.2 p40 + §H.7 p122 (the normative anchors for the canonical
/// AEA-axis / SCI-axis placement of ATOMAL / BOHEMIA / BALK).
const NATO_PORTION_FORMS: &[&str] = &[
    // Five base-level portion forms (CAPCO-2016 §G.1 Table 4)
    "CTS", "NC", "NR", "NS", "NU",
    // Five legacy compound forms — post-PR-9c.1, the strict parser
    // canonicalizes these to bare class + AEA/SCI companion. They
    // stay in the no-fuzz vocabulary so fuzzy correction can't
    // silently rewrite them to the bare-class form before the strict
    // parser sees them, and so E066 autofix can detect them.
    "CTS-B", "CTS-BALK", "CTSA", "NCA", "NSAT",
];

/// Extended fuzzy-correction vocabulary: `ALL_CVE_TOKENS` ∪ banner long forms
/// from [`MARKING_FORMS`] ∪ [`SAR_STRUCTURAL_KEYWORDS`] ∪
/// [`CLASSIFICATION_STRUCTURAL_KEYWORDS`] ∪ [`NATO_CLASSIFICATION_KEYWORDS`] ∪
/// [`AEA_SCI_STRUCTURAL_KEYWORDS`] ∪ [`NATO_PORTION_FORMS`],
/// sorted and deduplicated.
///
/// `ALL_CVE_TOKENS` carries only the **portion-form** abbreviations
/// (`NF`, `OC`, `PR`, `XD`, `ND`) and a handful of single-form tokens
/// (`RELIDO`, `FISA`, `FOUO`). The banner long forms — which are valid
/// inputs the strict parser handles via
/// [`crate::marking_forms::banner_to_portion`] — were missing from the
/// vocabulary the fuzzy matcher consults, so an OCR/transcription typo
/// like `NOFORON` (distance 1 from `NOFORN`) found no correction target
/// and the decoder discarded it as `TokenKind::Unknown`. See issue #133.
///
/// Round-trip safety: the strict parser's `parse_dissem_full_form` and
/// `parse_non_ic_full_form` already accept the banner forms here and
/// translate them to the canonical portion enum, so a fuzzy correction
/// returning `NOFORN` (rather than `NF`) lands on the same final
/// [`crate::DissemControl::Nf`] after strict parsing. The SAR
/// structural keywords (`SAR_STRUCTURAL_KEYWORDS`) are similarly
/// round-trip safe: `parse_sar_category` accepts the canonical
/// `SPECIAL ACCESS REQUIRED-` indicator literally, so a correction
/// returning `SPECIAL` for `SPCIAL` lands at the same `SarMarking`
/// after strict parsing.
///
/// Multi-word banner forms (`DEA SENSITIVE`, `SBU NOFORN`,
/// `LES NOFORN`, `DOD UCNI`, `DOE UCNI`) are retained intentionally.
/// The decoder's per-token fuzzy tokenizer (`scan_token` in
/// `crates/engine/src/decoder.rs`) splits raw input on whitespace, so
/// these never appear as a single *input* token to the matcher — but
/// fuzzy correction can still emit one of them as the canonical
/// *output* for a whitespace-free typo (e.g., `SBUNOFORN` →
/// `SBU NOFORN`, distance 1, single-character insertion of the
/// space). The strict parser then accepts the corrected multi-word
/// form via `parse_non_ic_full_form` / `parse_dissem_full_form` and
/// translates it to the canonical portion enum, so the round-trip
/// lands at the expected `NonIcDissem::SbuNf` (or peer). Pinned by
/// `marque-core::fuzzy::tests::real_vocab_emits_multi_word_banner_for_whitespace_free_typo`.
static EXTENDED_CORRECTION_VOCAB: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut v: Vec<&'static str> = values::ALL_CVE_TOKENS.to_vec();
    for f in MARKING_FORMS {
        v.push(f.banner);
    }
    v.extend_from_slice(SAR_STRUCTURAL_KEYWORDS);
    v.extend_from_slice(CLASSIFICATION_STRUCTURAL_KEYWORDS);
    v.extend_from_slice(NATO_CLASSIFICATION_KEYWORDS);
    v.extend_from_slice(AEA_SCI_STRUCTURAL_KEYWORDS);
    v.extend_from_slice(NATO_PORTION_FORMS);
    v.sort();
    v.dedup();
    v
});

pub struct CapcoTokenSet;

impl TokenSet for CapcoTokenSet {
    fn canonicalize(&self, token: &str) -> Option<&'static str> {
        // `ALL_CVE_TOKENS` is emitted sorted and deduplicated by build.rs,
        // so an O(log n) binary search is correct and faster than the
        // previous O(n) linear scan.
        values::ALL_CVE_TOKENS
            .binary_search(&token)
            .ok()
            .map(|i| values::ALL_CVE_TOKENS[i])
    }

    fn is_trigraph(&self, token: &str) -> bool {
        // TRIGRAPHS is emitted sorted and deduplicated by build.rs, so
        // binary_search is O(log n) over ~340 entries instead of the old
        // O(n) `.contains()` linear scan. Hot path for every REL TO parse.
        values::TRIGRAPHS.binary_search(&token).is_ok()
    }

    fn correction_vocab(&self) -> &[&'static str] {
        EXTENDED_CORRECTION_VOCAB.as_slice()
    }
}

impl CapcoTokenSet {
    /// Returns a reference to the Aho-Corasick automaton built from all CVE tokens.
    /// Reserved for Phase 2 multi-pattern matching when per-token spans are wired.
    #[allow(dead_code)]
    pub(crate) fn automaton() -> &'static AhoCorasick {
        &AUTOMATON
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn all_cve_tokens_are_sorted_and_unique() {
        let tokens = values::ALL_CVE_TOKENS;
        for window in tokens.windows(2) {
            assert!(
                window[0] < window[1],
                "ALL_CVE_TOKENS is not strictly sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn trigraphs_are_sorted_and_unique() {
        // `is_trigraph` relies on binary_search, so the slice must be
        // strictly-sorted. If a future ODNI XSD update shuffles the order,
        // build.rs collects into a BTreeSet and this test catches any
        // regression of that contract.
        let trigraphs = values::TRIGRAPHS;
        for window in trigraphs.windows(2) {
            assert!(
                window[0] < window[1],
                "TRIGRAPHS is not strictly sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn canonicalize_returns_known_token() {
        let set = CapcoTokenSet;
        // SECRET is in the banner-words we always emit.
        assert_eq!(set.canonicalize("SECRET"), Some("SECRET"));
    }

    #[test]
    fn canonicalize_returns_none_for_unknown() {
        let set = CapcoTokenSet;
        assert_eq!(set.canonicalize("BANANAPHONE"), None);
    }

    #[test]
    fn usa_is_a_known_trigraph() {
        let set = CapcoTokenSet;
        assert!(set.is_trigraph("USA"));
    }

    #[test]
    fn unknown_string_is_not_a_trigraph() {
        let set = CapcoTokenSet;
        assert!(!set.is_trigraph("XYZ_NOT_A_COUNTRY"));
    }

    #[test]
    fn correction_vocab_returns_sorted_nonempty_slice() {
        let vocab = CapcoTokenSet.correction_vocab();
        assert!(!vocab.is_empty(), "correction vocab must not be empty");
        for window in vocab.windows(2) {
            assert!(
                window[0] < window[1],
                "correction_vocab must be strictly sorted: {:?} >= {:?}",
                window[0],
                window[1],
            );
        }
    }

    #[test]
    fn correction_vocab_contains_core_classification_tokens() {
        let vocab = CapcoTokenSet.correction_vocab();
        for expected in &["SECRET", "CONFIDENTIAL", "UNCLASSIFIED"] {
            assert!(
                vocab.binary_search(expected).is_ok(),
                "correction_vocab must contain {expected:?}"
            );
        }
    }

    #[test]
    fn correction_vocab_excludes_non_ic_dissem_caveats() {
        // Regression guard for the non-IC dissem deny-list invariant.
        // ODNI's `CVEnumISMDissem.xml` is a UNION enum bundling IC
        // dissem controls (CAPCO source 1) with the ISOO CUI Registry
        // caveat tail (AC, AWP, DL_ONLY, FED_ONLY, FEDCON, NOCON) and
        // the DOD-SAP `WAIVED` entry. CAPCO-2016 §A.3 p13 explicitly
        // disclaims caveats from its scope. The `build.rs` of
        // `marque-ism` deny-lists those seven tokens so they never
        // enter the IC `DissemControl` enum or `ALL_CVE_TOKENS`. This
        // test pins that invariant — a future schema-update bump that
        // re-introduces them, or a deny-list typo, fails here loudly
        // rather than silently broadening the CAPCO grammar to accept
        // caveats as IC dissem controls.
        //
        // Tracking issue for the broader caveat / second-banner-line
        // data model: github.com/marquetools/marque#128.
        let vocab = CapcoTokenSet.correction_vocab();
        for forbidden in &[
            "WAIVED", "AC", "AWP", "DL_ONLY", "FED_ONLY", "FEDCON", "NOCON",
        ] {
            assert!(
                vocab.binary_search(forbidden).is_err(),
                "correction_vocab MUST NOT contain {forbidden:?} — \
                 it is a non-IC caveat (CAPCO-2016 §A.3 p13 \
                 disclaimer) that should be filtered by build.rs's \
                 NON_IC_DISSEM_DENY_LIST"
            );
        }
    }

    #[test]
    fn correction_vocab_contains_dissem_banner_long_forms() {
        // Issue #133 root cause #1: the fuzzy matcher saw only
        // `ALL_CVE_TOKENS`, which carries the dissem **portion**
        // abbreviations (NF, OC, PR) plus `RELIDO`/`FISA`/`FOUO`,
        // but not the banner long forms (NOFORN, ORCON, PROPIN,
        // EXDIS, NODIS, …). So `NOFORON` had no edit-distance
        // candidate and the decoder discarded it. The extended
        // vocab pulls every entry's banner form from
        // `marking_forms::MARKING_FORMS`, with the strict parser's
        // `parse_dissem_full_form` then normalizing the matched
        // long form to the canonical portion enum.
        let vocab = CapcoTokenSet.correction_vocab();
        for expected in &[
            "NOFORN",
            "ORCON",
            "ORCON-USGOV",
            "IMCON",
            "PROPIN",
            "RSEN",
            "LIMDIS",
            "EXDIS",
            "NODIS",
        ] {
            assert!(
                vocab.binary_search(expected).is_ok(),
                "correction_vocab MUST contain {expected:?} — \
                 banner long form per CAPCO-2016 §G.1 Table 4 \
                 (issue #133 root cause #1)"
            );
        }
    }

    #[test]
    fn correction_vocab_keeps_ic_dissem_controls() {
        // Companion to `correction_vocab_excludes_non_ic_dissem_caveats`:
        // make sure the deny-list didn't take a real IC dissem control
        // with it. Every entry below appears in CAPCO-2016 §A.5 page 38
        // as an IC dissem (or §H.8 for the per-marking detail page);
        // RAWFISA + EXEMPT_FROM_ICD501_DISCOVERY are post-CAPCO-2016
        // additions in the live ICRM XML, kept by the deny-list-rather-
        // than-allowlist approach so future IC additions flow through
        // automatically.
        let vocab = CapcoTokenSet.correction_vocab();
        for expected in &[
            "RS",
            "FOUO",
            "OC",
            "OC-USGOV",
            "IMC",
            "NF",
            "PR",
            "REL",
            "RELIDO",
            "EYES",
            "DSEN",
            "RAWFISA",
            "FISA",
            "DISPLAYONLY",
            "EXEMPT_FROM_ICD501_DISCOVERY",
        ] {
            assert!(
                vocab.binary_search(expected).is_ok(),
                "correction_vocab MUST contain {expected:?} — \
                 IC dissem control per CAPCO-2016 §A.5 / §H.8 or \
                 a post-2016 ICRM addition"
            );
        }
    }

    #[test]
    fn correction_vocab_contains_top_classification_keyword() {
        // Issue #133 PR 8: bare `TOP` lives outside `ALL_CVE_TOKENS`
        // because the CVE schema only lists the full multi-word
        // `TOP SECRET` classification entry. The decoder's
        // `scan_token` whitespace tokenizer arrives at the fuzzy
        // matcher with `TPP` (or other 3/4-char typos) as a
        // standalone token, so without `TOP` in the correction vocab
        // there's no fuzzy target and the candidate gets dropped.
        // Adding `TOP` here lets the standard edit-distance fuzzy
        // path recover `TPP→TOP` (dist 1), `UOP→TOP` (dist 1),
        // `TDOP→TOP` (dist 1, 4-char input via length-diff filter),
        // `QTOP→TOP` (dist 1), and `TOPW→TOP` (dist 1). Strict
        // parser then re-joins `TOP SECRET` into the canonical
        // multi-word classification.
        let vocab = CapcoTokenSet.correction_vocab();
        assert!(
            vocab.binary_search(&"TOP").is_ok(),
            "correction_vocab MUST contain bare \"TOP\" — issue #133 PR 8 \
             classification typo recovery target",
        );
    }

    #[test]
    fn correction_vocab_contains_sar_structural_keywords() {
        // Issue #133 PR 6: the SAR indicator keywords (`SPECIAL`,
        // `ACCESS`) live outside `ALL_CVE_TOKENS` because the ODNI
        // `CVEnumISMSAR.xml` is empty (SAR program identifiers are
        // agency-assigned and not centrally registered). The structural
        // SAR parser handles the `SPECIAL ACCESS REQUIRED-` indicator
        // as a literal string match, but the fuzzy matcher had no
        // vocabulary entry for `SPECIAL` or `ACCESS` — so an OCR typo
        // like `SPCIAL` (distance 1 from `SPECIAL`) produced no
        // correction, the token survived as `TokenKind::Unknown`,
        // and the decoder discarded the candidate via step 3a's
        // Unknown-span filter. This test pins the fix.
        //
        // `REQUIRED` and `SAR` are deliberately NOT in this list —
        // they are always glued to a program nickname / identifier
        // (`REQUIRED-BUTTER`, `SAR-BP-J12`) inside one `scan_token`
        // chunk, so adding them to the vocab is a no-op for the hot
        // path. See `SAR_STRUCTURAL_KEYWORDS` doc comment.
        let vocab = CapcoTokenSet.correction_vocab();
        for expected in &["ACCESS", "SPECIAL"] {
            assert!(
                vocab.binary_search(expected).is_ok(),
                "correction_vocab MUST contain {expected:?} — \
                 SAR structural keyword per CAPCO-2016 §H.5 p100 \
                 (issue #133 PR 6)"
            );
        }
    }

    #[test]
    fn correction_vocab_contains_aea_sci_structural_keywords() {
        // PR #256: AEA/SCI long-title structural keywords added so the fuzzy
        // matcher can recover OCR typos in "FORMERLY RESTRICTED DATA" (FRD,
        // §H.6) and "TALENT KEYHOLE" (TK, §H.4 p71). `NUCLEAR` is
        // intentionally excluded — see `AEA_SCI_STRUCTURAL_KEYWORDS` doc
        // comment.
        let vocab = CapcoTokenSet.correction_vocab();
        for expected in &["FORMERLY", "KEYHOLE", "TALENT"] {
            assert!(
                vocab.binary_search(expected).is_ok(),
                "correction_vocab MUST contain {expected:?} — \
                 AEA/SCI structural keyword per CAPCO-2016 §H.6 / §H.4 p71 \
                 (PR #256)"
            );
        }
    }
}
