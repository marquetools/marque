// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use smol_str::SmolStr;

// ===========================================================================
// SAR (Special Access Required) structural types
// ===========================================================================
//
// See CAPCO Register §H.5 (pp 99–102) and §A.6 (pp 15–17) for the source
// grammar. SAR identifiers are agency-assigned codewords and cannot be
// enumerated — this type hierarchy validates shape and roll-up rather than
// membership.

/// Complete SAR category block parsed from a marking.
///
/// Produced by `marque-core::parser::parse_sar_category` (P2) and stored on
/// [`CanonicalAttrs::sar_markings`]. Only one SAR block is permitted per
/// marking per §A.6; multiple `//SAR-…//` blocks in the same marking yield
/// an `E030 sar-indicator-repeat` diagnostic.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SarMarking {
    /// The form of SAR indicator used in the source marking.
    pub indicator: SarIndicator,
    /// Programs in the order they appeared. Sort-order validation is
    /// performed by rule E028, not at parse time.
    pub programs: Box<[SarProgram]>,
}

/// Which SAR indicator form a marking uses. Banner lines may use either;
/// portion marks may only use `Abbrev` (rule E026 enforces this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SarIndicator {
    /// `SAR-` (portion and banner).
    Abbrev,
    /// `SPECIAL ACCESS REQUIRED-` (banner only).
    Full,
}

/// A single Special Access Program with optional compartments.
///
/// Identifier forms (§A.6 grammar):
/// - Abbreviated: 2–3 alphanumeric characters (`BP`, `CD`, `XR`).
/// - Full (nickname): uppercase letters with optional spaces
///   (`BUTTER POPCORN`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SarProgram {
    /// Program identifier as it appeared in the source.
    ///
    /// `SmolStr` inline storage is capped at 23 bytes. Abbreviated form
    /// (2–3 chars) and most nicknames stay inline; nicknames above the
    /// threshold (e.g., `SPECIAL ACCESS REQUIRED` is exactly 23 bytes,
    /// anything longer overflows) fall back to `Arc<str>`. The fallback
    /// is still better than `Box<str>`'s always-heap path and keeps
    /// `Clone` cheap (refcount bump), so the field stays `SmolStr` even
    /// for the full-form case.
    pub identifier: SmolStr,
    /// Compartments in source order. May be empty.
    pub compartments: Box<[SarCompartment]>,
}

/// A compartment within a SAR program, optionally carrying sub-compartments.
///
/// §H.5 p100 explicitly forbids depicting hierarchy below the sub-compartment
/// level.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SarCompartment {
    /// Compartment identifier (alphanumeric).
    pub identifier: SmolStr,
    /// Sub-compartments in source order. May be empty.
    pub sub_compartments: Box<[SmolStr]>,
}

impl SarMarking {
    /// Construct a [`SarMarking`] from an indicator form and a list of
    /// programs. `programs` SHOULD be in source order — sort validation is
    /// performed by rule E028, not here.
    pub fn new(indicator: SarIndicator, programs: Box<[SarProgram]>) -> Self {
        Self {
            indicator,
            programs,
        }
    }
}

impl SarProgram {
    /// Construct a [`SarProgram`] with an optional compartment list.
    pub fn new(identifier: impl Into<SmolStr>, compartments: Box<[SarCompartment]>) -> Self {
        Self {
            identifier: identifier.into(),
            compartments,
        }
    }

    /// SAR program identifier abbreviation shape gate: 2 or 3 ASCII
    /// uppercase alphanumeric bytes.
    ///
    /// This is the byte-class admission predicate for the `SAR-`
    /// indicator form (the abbreviated, portion-mark-and-banner form).
    /// Parser admission for the `SPECIAL ACCESS REQUIRED-` indicator
    /// form goes through [`SarProgram::admits_program_id_full`]
    /// instead — the full form admits a different shape (uppercase
    /// letters with optional spaces, no length cap).
    ///
    /// # Single source of truth
    ///
    /// This predicate is the canonical SAR abbreviation shape gate.
    /// The `Vocabulary<CapcoScheme>` adapter at
    /// `crates/capco/src/vocabulary.rs` calls this from
    /// `shape_admits(CAT_SAR, _)` (Phase 5 PR-2 gating); the strict
    /// parser at `crates/core/src/parser.rs::parse_sar_program` calls
    /// it directly to gate the `SarIndicator::Abbrev` branch
    /// (FR-015 / FR-016, T089). Both call sites MUST go through this
    /// function rather than inline a length-and-class check —
    /// keeping the predicate single-sited prevents drift between
    /// admission and parser surfaces (CHK030).
    ///
    /// # Authority
    ///
    /// CAPCO-2016 §H.5 p101 ("Additional Marking Instructions: A
    /// program identifier abbreviation is the two or three-character
    /// designator for the program") fixes the length bound.
    /// CAPCO-2016 §H.5 p99 ("SAR program identifiers are
    /// alphanumeric values") fixes the character class.
    ///
    /// SAR is the only marking category with no CVE registry —
    /// `CVEnumISMSAR.xml` is intentionally empty per ODNI policy.
    /// With no registry to validate against, the shape gate IS the
    /// validation. Per CAPCO-2016 §A.6 p15 and §G.1 p36, all
    /// banner-line and portion-mark Register entries are uppercase,
    /// so SAR identifiers must conform. Lenient acceptance would let
    /// lowercase identifiers through the strict path silently,
    /// bypassing the `DecoderRecognizer` that handles demangling
    /// (e.g., mixed-case repair, missing-hyphen suggestions). The
    /// parser/decoder split is intentional: the parser is strict so
    /// the decoder can be lenient and informative.
    ///
    /// Case-mismatch demangling (lowercase → uppercase) is covered by
    /// `crates/engine/tests/decoder_dispatch_post_280.rs` via issue #699.
    /// Missing-hyphen demangling at the program/compartment boundary
    /// (e.g., `SAR-BP XA5` → `SAR-BP-XA5`) remains future decoder work
    /// tracked under issue #710.
    ///
    /// # Examples
    ///
    /// ```
    /// use marque_ism::SarProgram;
    /// // §H.5 p101 examples (Register convention: uppercase).
    /// assert!(SarProgram::admits_program_id_abbrev(b"BP"));
    /// assert!(SarProgram::admits_program_id_abbrev(b"SDA"));
    /// assert!(SarProgram::admits_program_id_abbrev(b"XR"));
    /// // §H.5 p99 — alphanumeric values (digits permitted).
    /// assert!(SarProgram::admits_program_id_abbrev(b"99"));
    /// assert!(SarProgram::admits_program_id_abbrev(b"A1"));
    /// // Rejected: lowercase fails the Register-uppercase rule
    /// // (§A.6 p15 + §G.1 p36); decoder handles demangling.
    /// assert!(!SarProgram::admits_program_id_abbrev(b"bp"));
    /// assert!(!SarProgram::admits_program_id_abbrev(b"Bp"));
    /// // Rejected: too short.
    /// assert!(!SarProgram::admits_program_id_abbrev(b"B"));
    /// // Rejected: too long.
    /// assert!(!SarProgram::admits_program_id_abbrev(b"BPCD"));
    /// // Rejected: empty.
    /// assert!(!SarProgram::admits_program_id_abbrev(b""));
    /// // Rejected: punctuation.
    /// assert!(!SarProgram::admits_program_id_abbrev(b"B-"));
    /// assert!(!SarProgram::admits_program_id_abbrev(b"B P"));
    /// ```
    #[inline]
    pub const fn admits_program_id_abbrev(bytes: &[u8]) -> bool {
        let len = bytes.len();
        if len < 2 || len > 3 {
            return false;
        }
        let mut i = 0;
        while i < len {
            if !(bytes[i].is_ascii_uppercase() || bytes[i].is_ascii_digit()) {
                return false;
            }
            i += 1;
        }
        true
    }

    /// SAR full-form program identifier shape gate: one or more
    /// ASCII bytes, each either an uppercase letter `[A-Z]` or
    /// space, with at least one non-space byte. Hyphens and digits
    /// are NOT permitted inside the full nickname.
    ///
    /// This is the byte-class admission predicate for the
    /// `SPECIAL ACCESS REQUIRED-` indicator form (the spelled-out,
    /// banner-only form). Parser admission for the abbreviated form
    /// (`SAR-`) goes through
    /// [`SarProgram::admits_program_id_abbrev`] instead.
    ///
    /// The hyphen exclusion is load-bearing: in the parser, the
    /// first `-` after the indicator literal always marks the
    /// program/compartment boundary
    /// (CAPCO-2016 §H.5 p100). If the predicate admitted hyphens, a
    /// nickname like `"BUTTER-POPCORN"` would parse as a program
    /// `"BUTTER"` with compartment `"POPCORN"` — silently
    /// reinterpreting the marking. The digit exclusion follows the
    /// Register convention that nicknames spell out program names
    /// (Table 7 §H.5 p100 example: `BUTTER POPCORN`); the digits
    /// belong with the abbreviation form, not the full form.
    ///
    /// # Single source of truth
    ///
    /// The strict parser at
    /// `crates/core/src/parser.rs::parse_sar_program` calls this to
    /// gate the `SarIndicator::Full` branch (FR-015 closure
    /// alongside the abbreviation predicate). The
    /// `Vocabulary<CapcoScheme>::shape_admits(CAT_SAR, _)` arm
    /// gates the *abbreviation* shape (the most common surface
    /// form, and the one a category-level admission predicate is
    /// most useful for); the full form has no `CategoryId` of its
    /// own in the current scheme, so the vocabulary surface does
    /// not call this directly. Keeping the predicate symbolic in
    /// `marque-ism` rather than inline at the parser site preserves
    /// the FR-015 invariant ("admission via documented vocabulary
    /// surface") and matches the FGI / abbreviation precedent.
    ///
    /// # Authority
    ///
    /// CAPCO-2016 §H.5 p101 ("Authorized Banner Line Marking
    /// Title: SPECIAL ACCESS REQUIRED-[program identifier]";
    /// "Example Banner Line: TOP SECRET//SAR-BUTTER POPCORN") +
    /// §H.5 p100 Table 7 (canonical example uses `BUTTER POPCORN`,
    /// uppercase letters with a single interior space).
    ///
    /// CAPCO-2016 does not explicitly bound the full-form character
    /// class beyond "the program's assigned nickname, codeword, or
    /// abbreviation" (§H.5 p99). The "uppercase + spaces, no
    /// hyphens, no digits" interpretation is a marque parser
    /// convention rooted in Table 7's canonical example and the
    /// hyphen-as-compartment-separator rule (§H.5 p100). This is
    /// noted at the call site so a future revision of CAPCO can
    /// widen or narrow the predicate intentionally rather than by
    /// drift.
    ///
    /// # Examples
    ///
    /// ```
    /// use marque_ism::SarProgram;
    /// // §H.5 p101 canonical example.
    /// assert!(SarProgram::admits_program_id_full(b"BUTTER POPCORN"));
    /// // Single uppercase token also lawful.
    /// assert!(SarProgram::admits_program_id_full(b"SODA"));
    /// // Multiple interior spaces are accepted by the predicate;
    /// // a downstream style rule may flag double-spacing.
    /// assert!(SarProgram::admits_program_id_full(b"BUTTER  POPCORN"));
    /// // Rejected: lowercase.
    /// assert!(!SarProgram::admits_program_id_full(b"butter popcorn"));
    /// // Rejected: digits — abbreviation territory.
    /// assert!(!SarProgram::admits_program_id_full(b"123"));
    /// // Rejected: hyphen — would silently re-parse as
    /// // program/compartment boundary.
    /// assert!(!SarProgram::admits_program_id_full(b"BUTTER-POPCORN"));
    /// // Rejected: empty.
    /// assert!(!SarProgram::admits_program_id_full(b""));
    /// // Rejected: only spaces.
    /// assert!(!SarProgram::admits_program_id_full(b"   "));
    /// ```
    #[inline]
    pub const fn admits_program_id_full(bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let mut i = 0;
        let mut has_non_space = false;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b' ' {
                // space — permitted.
            } else if b.is_ascii_uppercase() {
                has_non_space = true;
            } else {
                return false;
            }
            i += 1;
        }
        has_non_space
    }
}

impl SarCompartment {
    /// Construct a [`SarCompartment`] with an optional sub-compartment list.
    pub fn new(identifier: impl Into<SmolStr>, sub_compartments: Box<[SmolStr]>) -> Self {
        Self {
            identifier: identifier.into(),
            sub_compartments,
        }
    }

    /// SAR compartment / sub-compartment identifier shape gate:
    /// one or more ASCII uppercase alphanumeric bytes.
    ///
    /// CAPCO-2016 §H.5 pp 99-100 places compartments and
    /// sub-compartments under one rule: both are alphanumeric values
    /// listed in ascending sort order. The grammar does not
    /// distinguish their character class or length bounds, so a
    /// single predicate admits both grammar positions. The parser
    /// (`crates/core/src/parser.rs::parse_sar_program`) calls this
    /// for both the compartment slot and the sub-compartment slot.
    ///
    /// # Single source of truth
    ///
    /// The strict parser at
    /// `crates/core/src/parser.rs::parse_sar_program` calls this to
    /// gate compartments (segments after the first `-`) and
    /// sub-compartments (space-separated tokens within a
    /// compartment segment) (FR-015, T090, T091). Both call sites
    /// MUST go through this function rather than inline a
    /// length-and-class check — keeping the predicate single-sited
    /// prevents drift between admission and parser surfaces
    /// (CHK030).
    ///
    /// The `Vocabulary<CapcoScheme>::shape_admits(CAT_SAR, _)` arm
    /// uses [`SarProgram::admits_program_id_abbrev`] (the
    /// program-id-abbreviation shape) because `CAT_SAR` represents
    /// the SAR program identifier slot in the marking grammar, not
    /// the compartment slot. There is no `CategoryId` for SAR
    /// compartments today; if one is added, this predicate is the
    /// natural target for its `shape_admits` arm.
    ///
    /// # Authority
    ///
    /// CAPCO-2016 §H.5 p99 ("SAR program identifiers are
    /// alphanumeric values"; the surrounding prose at p99-100
    /// applies the same rule to compartments and sub-compartments).
    /// CAPCO-2016 §H.5 p100 Table 7 examples confirm the character
    /// class spans alpha (`J12`, `K15`, `YYY`, `XRA`, `J54`, `RB`)
    /// and digit (`456`, `689`).
    ///
    /// CAPCO-2016 prose does NOT specify an upper length bound for
    /// compartments or sub-compartments — Table 7 examples are 2-3
    /// characters but no rule pins that. We admit length ≥ 1 with
    /// no upper bound, matching the parser's existing behavior. A
    /// future revision of CAPCO that pins a length cap would be a
    /// planned migration; this predicate is the lift point.
    ///
    /// SAR is the only marking category with no CVE registry —
    /// `CVEnumISMSAR.xml` is intentionally empty per ODNI policy.
    /// With no registry to validate against, the shape gate IS the
    /// validation. Per CAPCO-2016 §A.6 p15 and §G.1 p36, all
    /// banner-line and portion-mark Register entries are uppercase,
    /// so SAR identifiers (programs, compartments, sub-compartments)
    /// must conform. Lenient acceptance would let lowercase
    /// identifiers through the strict path silently, bypassing the
    /// `DecoderRecognizer` that handles demangling (e.g.,
    /// mixed-case repair, missing-hyphen suggestions). The
    /// parser/decoder split is intentional: the parser is strict so
    /// the decoder can be lenient and informative.
    ///
    /// SAR demangling (case-mismatch, program/compartment/sub-compartment)
    /// is covered by `crates/engine/tests/decoder_dispatch_post_280.rs`
    /// and `crates/engine/tests/recognized_canonical_field_scoping.rs`.
    ///
    /// # Examples
    ///
    /// ```
    /// use marque_ism::SarCompartment;
    /// // §H.5 p100 Table 7 examples.
    /// assert!(SarCompartment::admits_identifier(b"J12"));
    /// assert!(SarCompartment::admits_identifier(b"K15"));
    /// assert!(SarCompartment::admits_identifier(b"YYY"));
    /// assert!(SarCompartment::admits_identifier(b"RB"));
    /// // Sub-compartments use the same shape.
    /// assert!(SarCompartment::admits_identifier(b"J54"));
    /// assert!(SarCompartment::admits_identifier(b"456"));
    /// assert!(SarCompartment::admits_identifier(b"689"));
    /// // Single character is admitted (manual silent on lower bound
    /// // beyond ≥1; marque admits length 1+).
    /// assert!(SarCompartment::admits_identifier(b"1"));
    /// // Rejected: lowercase fails the Register-uppercase rule
    /// // (§A.6 p15 + §G.1 p36); decoder handles demangling.
    /// assert!(!SarCompartment::admits_identifier(b"j12"));
    /// assert!(!SarCompartment::admits_identifier(b"yYy"));
    /// // Rejected: empty.
    /// assert!(!SarCompartment::admits_identifier(b""));
    /// // Rejected: punctuation.
    /// assert!(!SarCompartment::admits_identifier(b"J-12"));
    /// assert!(!SarCompartment::admits_identifier(b"J 12"));
    /// // Rejected: non-ASCII.
    /// assert!(!SarCompartment::admits_identifier("Ĵ12".as_bytes()));
    /// ```
    #[inline]
    pub const fn admits_identifier(bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let mut i = 0;
        while i < bytes.len() {
            if !(bytes[i].is_ascii_uppercase() || bytes[i].is_ascii_digit()) {
                return false;
            }
            i += 1;
        }
        true
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod sar_shape_tests {
    //! Shape-admission predicates for SAR program identifiers and
    //! compartments / sub-compartments. These are the canonical
    //! single-source-of-truth predicates that
    //! `crates/core/src/parser.rs::parse_sar_program` calls; the
    //! parser-side tests in `crates/core/src/parser.rs::sar_parse_tests`
    //! exercise the full grammar through the dispatch path. These
    //! tests focus on the predicate's accept/reject set in isolation.
    //!
    //! Authority: CAPCO-2016 §H.5 pp 99–101 (verified against
    //! `crates/capco/docs/CAPCO-2016.md`).

    use super::{SarCompartment, SarProgram};

    // ----- SarProgram::admits_program_id_abbrev -------------------------

    #[test]
    fn abbrev_accepts_two_or_three_alnum() {
        // §H.5 p101 register-convention examples.
        assert!(SarProgram::admits_program_id_abbrev(b"BP"));
        assert!(SarProgram::admits_program_id_abbrev(b"SDA"));
        assert!(SarProgram::admits_program_id_abbrev(b"XR"));
        // §H.5 p99 — alphanumeric values (digits permitted).
        assert!(SarProgram::admits_program_id_abbrev(b"A1"));
        assert!(SarProgram::admits_program_id_abbrev(b"99"));
        assert!(SarProgram::admits_program_id_abbrev(b"1A2"));
    }

    #[test]
    fn abbrev_rejects_wrong_length() {
        // Length < 2: empty or 1 char.
        assert!(!SarProgram::admits_program_id_abbrev(b""));
        assert!(!SarProgram::admits_program_id_abbrev(b"B"));
        // Length > 3: 4 chars or more.
        assert!(!SarProgram::admits_program_id_abbrev(b"BPCD"));
        assert!(!SarProgram::admits_program_id_abbrev(b"ABCDE"));
    }

    #[test]
    fn abbrev_rejects_lowercase_open_vocab_shape_is_validation() {
        // Issue #280: SAR has no CVE registry (`CVEnumISMSAR.xml`
        // intentionally empty per ODNI policy). With no registry to
        // validate against, the shape gate IS the validation. Per
        // CAPCO-2016 §A.6 p15 + §G.1 p36, all banner-line and
        // portion-mark Register entries are uppercase; SAR
        // identifiers must conform. Lowercase / mixed-case falls
        // through to the decoder, which handles demangling.
        assert!(!SarProgram::admits_program_id_abbrev(b"bp"));
        assert!(!SarProgram::admits_program_id_abbrev(b"sDa"));
        assert!(!SarProgram::admits_program_id_abbrev(b"Bp"));
    }

    #[test]
    fn abbrev_rejects_punctuation_and_whitespace() {
        // The hyphen is the program/compartment separator
        // (§H.5 p100); admitting it inside the abbreviation would
        // silently re-parse the marking.
        assert!(!SarProgram::admits_program_id_abbrev(b"B-"));
        assert!(!SarProgram::admits_program_id_abbrev(b"-B"));
        // Space is the sub-compartment separator (§H.5 p100); same
        // re-parse risk.
        assert!(!SarProgram::admits_program_id_abbrev(b"B P"));
        // Other punctuation has no role in the §H.5 grammar.
        assert!(!SarProgram::admits_program_id_abbrev(b"B."));
        assert!(!SarProgram::admits_program_id_abbrev(b"B/"));
    }

    #[test]
    fn abbrev_rejects_non_ascii() {
        // Non-ASCII multi-byte sequences make `bytes.len()` exceed
        // the 2-3 char gate immediately; the alnum test would also
        // reject them. Both gates fail loud rather than admitting
        // a multi-byte token.
        let two_byte_e_acute: &[u8] = "é".as_bytes();
        assert_eq!(two_byte_e_acute.len(), 2);
        assert!(!SarProgram::admits_program_id_abbrev(two_byte_e_acute));
    }

    // ----- SarProgram::admits_program_id_full ---------------------------

    #[test]
    fn full_accepts_uppercase_with_optional_spaces() {
        // §H.5 p101 canonical example.
        assert!(SarProgram::admits_program_id_full(b"BUTTER POPCORN"));
        // Single uppercase token also lawful.
        assert!(SarProgram::admits_program_id_full(b"SODA"));
        assert!(SarProgram::admits_program_id_full(b"A"));
        // Multiple-word nicknames.
        assert!(SarProgram::admits_program_id_full(b"ALPHA BETA GAMMA"));
    }

    #[test]
    fn full_rejects_lowercase() {
        assert!(!SarProgram::admits_program_id_full(b"butter popcorn"));
        assert!(!SarProgram::admits_program_id_full(b"BUTTER popcorn"));
        assert!(!SarProgram::admits_program_id_full(b"Butter Popcorn"));
    }

    #[test]
    fn full_rejects_digits() {
        // Digits are abbreviation territory — admitting them in the
        // full form blurs the §H.5 p101 distinction between the
        // two indicator forms.
        assert!(!SarProgram::admits_program_id_full(b"123"));
        assert!(!SarProgram::admits_program_id_full(b"BUTTER1"));
        assert!(!SarProgram::admits_program_id_full(b"BP1"));
    }

    #[test]
    fn full_rejects_hyphen() {
        // The first hyphen after the indicator literal always marks
        // the program/compartment boundary (§H.5 p100). Admitting
        // hyphens inside the nickname would silently re-parse a
        // marking like `BUTTER-POPCORN` as
        // `program=BUTTER, compartment=POPCORN`.
        assert!(!SarProgram::admits_program_id_full(b"BUTTER-POPCORN"));
        assert!(!SarProgram::admits_program_id_full(b"-BUTTER"));
        assert!(!SarProgram::admits_program_id_full(b"BUTTER-"));
    }

    #[test]
    fn full_rejects_empty_or_only_spaces() {
        assert!(!SarProgram::admits_program_id_full(b""));
        assert!(!SarProgram::admits_program_id_full(b" "));
        assert!(!SarProgram::admits_program_id_full(b"   "));
    }

    #[test]
    fn full_rejects_other_punctuation() {
        assert!(!SarProgram::admits_program_id_full(b"BUTTER.POPCORN"));
        assert!(!SarProgram::admits_program_id_full(b"BUTTER/POPCORN"));
        assert!(!SarProgram::admits_program_id_full(b"BUTTER_POPCORN"));
    }

    // ----- SarCompartment::admits_identifier ----------------------------

    #[test]
    fn compartment_accepts_alnum_any_length_at_least_one() {
        // §H.5 p100 Table 7 examples (compartments).
        assert!(SarCompartment::admits_identifier(b"J12"));
        assert!(SarCompartment::admits_identifier(b"K15"));
        assert!(SarCompartment::admits_identifier(b"YYY"));
        assert!(SarCompartment::admits_identifier(b"XRA"));
        // §H.5 p100 Table 7 examples (sub-compartments — same shape).
        assert!(SarCompartment::admits_identifier(b"J54"));
        assert!(SarCompartment::admits_identifier(b"456"));
        assert!(SarCompartment::admits_identifier(b"689"));
        assert!(SarCompartment::admits_identifier(b"RB"));
    }

    #[test]
    fn compartment_accepts_length_one() {
        // Manual is silent on a lower bound beyond ≥1; marque admits
        // single-character identifiers.
        assert!(SarCompartment::admits_identifier(b"A"));
        assert!(SarCompartment::admits_identifier(b"1"));
    }

    #[test]
    fn compartment_accepts_long_alnum() {
        // Manual is silent on an upper length bound; marque admits
        // length 1+. A future revision pinning a cap is a planned
        // migration at this predicate.
        assert!(SarCompartment::admits_identifier(b"ABCDEFGHIJ"));
        assert!(SarCompartment::admits_identifier(b"VERYLONGCOMPARTMENT123"));
    }

    #[test]
    fn compartment_rejects_lowercase_open_vocab_shape_is_validation() {
        // Issue #280: SAR open-vocab tightening (see
        // `abbrev_rejects_lowercase_open_vocab_shape_is_validation`
        // for the full rationale). The compartment / sub-compartment
        // predicate enforces the same Register-uppercase rule per
        // §A.6 p15 + §G.1 p36.
        assert!(!SarCompartment::admits_identifier(b"j12"));
        assert!(!SarCompartment::admits_identifier(b"yYy"));
        assert!(!SarCompartment::admits_identifier(b"blue42"));
    }

    #[test]
    fn compartment_rejects_empty() {
        assert!(!SarCompartment::admits_identifier(b""));
    }

    #[test]
    fn compartment_rejects_punctuation_and_whitespace() {
        // Hyphen is compartment separator (§H.5 p100), space is
        // sub-compartment separator. Admitting either would re-parse
        // the marking.
        assert!(!SarCompartment::admits_identifier(b"J-12"));
        assert!(!SarCompartment::admits_identifier(b"J 12"));
        // Other punctuation has no role in §H.5.
        assert!(!SarCompartment::admits_identifier(b"J.12"));
        assert!(!SarCompartment::admits_identifier(b"J_12"));
        assert!(!SarCompartment::admits_identifier(b"J/12"));
    }

    #[test]
    fn compartment_rejects_non_ascii() {
        let two_byte_e_acute: &[u8] = "é".as_bytes();
        assert!(!SarCompartment::admits_identifier(two_byte_e_acute));
        let three_byte_e_acute: &[u8] = "Ĵ12".as_bytes();
        assert!(!SarCompartment::admits_identifier(three_byte_e_acute));
    }

    // ----- Cross-predicate sanity ---------------------------------------

    #[test]
    fn abbrev_implies_compartment() {
        // Property: anything `admits_program_id_abbrev` admits is
        // also admitted by `admits_identifier` — the abbreviation
        // shape (2-3 uppercase-or-digit) is strictly within the
        // compartment shape (1+ uppercase-or-digit). Useful as a
        // sanity check that the two predicates don't drift apart on
        // character class. Issue #280 tightened both predicates to
        // reject lowercase; the strict-subset property still holds
        // over the new (uppercase + digit) character class.
        for input in [
            b"BP".as_slice(),
            b"SDA".as_slice(),
            b"99".as_slice(),
            b"A1".as_slice(),
            b"XR".as_slice(),
        ] {
            assert!(SarProgram::admits_program_id_abbrev(input));
            assert!(
                SarCompartment::admits_identifier(input),
                "compartment predicate must admit anything the \
                 abbreviation predicate admits (input={:?})",
                std::str::from_utf8(input).unwrap_or("<non-utf8>"),
            );
        }
    }
}
