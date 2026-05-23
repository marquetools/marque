// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tests for `decoder/recovery/rel_to.rs`. Carved into a parallel
//! file because `rel_to.rs` already sits at 781 lines and any
//! co-located tests block would push the combined file over the
//! 800-line gate. Reached from `recovery/rel_to.rs` via
//! `#[path = "../tests/rel_to_recovery_tests.rs"]`.

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
fn rel_to_header_normalize_fixes_rel_ot_transposition() {
    // Pattern 1: `REL OT ` (TO → OT) → `REL TO `.
    let result = try_rel_to_header_normalize("SECRET//REL OT USA, AUS, GBR");
    assert_eq!(
        result.as_deref(),
        Some("SECRET//REL TO USA, AUS, GBR"),
        "REL OT must rewrite to REL TO at //-boundary",
    );
}

#[test]
fn rel_to_header_normalize_fixes_relt_o_token_boundary() {
    // Pattern 2: `RELT O ` (T migrated from REL to start of next
    // token) → `REL TO `. The fuzzy pass would otherwise rewrite
    // `RELT` (4 chars) → `REL` (in-vocab DissemControl, distance
    // 1) and silently drop USA from the strict parse.
    let result = try_rel_to_header_normalize("SECRET//RELT O USA, AUS, GBR");
    assert_eq!(
        result.as_deref(),
        Some("SECRET//REL TO USA, AUS, GBR"),
        "RELT O must rewrite to REL TO at //-boundary",
    );
}

#[test]
fn rel_to_header_normalize_returns_none_on_canonical() {
    // Canonical `REL TO ` (and texts without REL at all) round-
    // trip unchanged.
    assert!(try_rel_to_header_normalize("SECRET//REL TO USA, AUS, GBR").is_none());
    assert!(try_rel_to_header_normalize("SECRET//NOFORN").is_none());
    assert!(try_rel_to_header_normalize("").is_none());
}

#[test]
fn rel_to_header_normalize_requires_token_boundary() {
    // The pattern must not fire when embedded inside a longer
    // alphanumeric run. Without the boundary check, `XREL OT Y`
    // would match the substring `REL OT` even though the leading
    // `X` makes the whole thing a single 6-char token.
    assert!(try_rel_to_header_normalize("XREL OT Y").is_none());
    assert!(try_rel_to_header_normalize("SOMETHINGRELT O Y").is_none());
}

#[test]
fn rel_to_entry_normalize_joins_a_us_to_aus() {
    // Pattern 3: 4-char entry `A US` joins to AUS only when the
    // joined 3-letter string is a known trigraph. AUS is a
    // trigraph; A alone is not.
    let result = try_rel_to_entry_normalize("SECRET//REL TO USA,A US, GBR");
    // The replacement preserves the entry's leading whitespace
    // (none here), so the rewritten block is `USA,AUS, GBR`.
    assert_eq!(
        result.as_deref(),
        Some("SECRET//REL TO USA,AUS, GBR"),
        "A US should join to AUS when is_trigraph(AUS) holds",
    );
}

#[test]
fn rel_to_entry_normalize_swaps_au_comma_s_to_aus_comma() {
    // Pattern 4: `<2-upper>,<1-upper><space>` swaps to
    // `<3-upper joined>,` only when the joined trigraph is
    // valid AND the 2-letter prefix alone is not a trigraph.
    let result = try_rel_to_entry_normalize("SECRET//REL TO USA, AU,S GBR");
    assert_eq!(
        result.as_deref(),
        Some("SECRET//REL TO USA, AUS, GBR"),
        "AU,S should swap to AUS, when is_trigraph(AUS) holds and AU is not a trigraph",
    );
}

#[test]
fn rel_to_entry_normalize_does_not_corrupt_eu_comma_pattern() {
    // EU is itself a valid 2-char trigraph entry. Pattern 4 must
    // not fire on `EU,X ` because `is_trigraph(EU)` is true —
    // this guards the rule "only fix when the prefix alone is
    // invalid". (Even though `EUX` may not be a trigraph and
    // wouldn't pass the join-is-trigraph guard either, the
    // prefix-is-trigraph check is the cleaner discriminator.)
    let result = try_rel_to_entry_normalize("SECRET//REL TO USA, EU, GBR");
    assert!(
        result.is_none(),
        "canonical EU entry must round-trip unchanged",
    );
}

#[test]
fn rel_to_entry_normalize_returns_none_outside_rel_to() {
    // No REL TO header → no entry-pass fixes. The patterns are
    // scoped to inside REL TO blocks specifically.
    assert!(try_rel_to_entry_normalize("SECRET//SI/TK//NOFORN").is_none());
    assert!(try_rel_to_entry_normalize("").is_none());
}

#[test]
fn rel_to_structural_repair_short_circuits_without_rel() {
    // Pre-check: text without `REL` returns None immediately,
    // skipping the byte walks.
    assert!(try_rel_to_structural_repair("SECRET//NOFORN").is_none());
    assert!(try_rel_to_structural_repair("(C)").is_none());
    assert!(try_rel_to_structural_repair("").is_none());
}
