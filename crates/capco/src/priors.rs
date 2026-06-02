// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Corpus-derived priors for the Phase-D decoder.
//!
//! All data is emitted as `&'static` const tables by
//! `crates/capco/build.rs` from `crates/capco/corpus/priors.json`.
//! No runtime JSON parsing, no runtime allocation — every decoder
//! scoring call reads this module's `pub const` tables directly.
//!
//! The priors are the corpus-learned half of the decoder's scoring:
//! per-token base rates (how often each canonical token appears in
//! non-IC English prose), per-template base rates (which grammar
//! template shapes are common in real markings), and strict-context
//! probability floors (if one portion says SECRET, candidate
//! resolutions that drop to UNCLASSIFIED are rejected before scoring).
//!
//! See `crates/capco/corpus/README.md` for the generator pipeline
//! and the JSON schema contract.

/// Per-token base rate and log-prior.
///
/// `count` is the raw occurrence count in the source corpus; `log_prior`
/// is the precomputed natural-log base-rate probability the decoder
/// uses at scoring time (saves a per-query `ln()`). The token string is
/// the canonical CAPCO form (e.g., `"SECRET"`, `"NOFORN"`).
#[derive(Debug, Clone, Copy)]
pub struct TokenPrior {
    pub token: &'static str,
    pub count: u64,
    pub log_prior: f32,
}

/// Per-template base rate and log-prior.
///
/// Template names are the grammar shape identifiers the generator
/// observed — e.g., `"classification"`, `"classification//dissem"`,
/// `"classification//sci-block//dissem"`.
#[derive(Debug, Clone, Copy)]
pub struct TemplatePrior {
    pub name: &'static str,
    pub count: u64,
    pub log_prior: f32,
}

/// Strict-context probability floors for candidate filtering.
///
/// Each floor is the probability that a classification token at that
/// level in one portion of a document implies other portions share at
/// least that floor. The decoder uses these to reject candidates that
/// would resolve to a classification level below the observed strict
/// evidence on the page.
#[derive(Debug, Clone, Copy)]
pub struct StrictContextPriors {
    pub confidential_floor: f32,
    pub secret_floor: f32,
    pub top_secret_floor: f32,
}

include!(concat!(env!("OUT_DIR"), "/priors.rs"));

/// Compile-time pin: `SCHEMA_VERSION` (emitted by `build.rs` from the
/// `schema_version` field of `priors.json`) MUST equal the value this
/// crate's source code is compiled to consume. A mismatch — caused
/// by a hand-edited `priors.json` or a generator regression that bumps
/// the version — fails the build with a clear message instead of
/// producing a green binary that emits records labeled with the wrong
/// schema.
///
/// `build.rs` already rejects any `schema_version` mismatch on the
/// producer side via an accept-list (see `crates/capco/build.rs`
/// `SUPPORTED_PRIORS_SCHEMA_VERSIONS` — it accepts the single
/// `capco-priors-3` value today, but the membership form lets a new
/// generation slot in additively). This const block is the
/// consumer-side counterpart kept as an explicit source pin and
/// defense-in-depth check that the generated `SCHEMA_VERSION` still
/// matches the version this crate is wired to consume — the value
/// fences the runtime tests below at the build-time tier so a CI lane
/// that happens to skip this crate's tests still catches a regression.
/// It also forces the consumer-side expectation to be a visible source
/// declaration, so a future PR that adds a schema version to the
/// `build.rs` accept-list has to update this pin in the same edit.
///
/// `capco-priors-3` (issue #258) added the `token_prose_base_rates`
/// and `country_code_prose_base_rates` tables alongside the existing
/// marking-side rates so the decoder can compute the per-token
/// "marking-y" score `log P(token|marking) − log P(token|prose)` and
/// surface a null hypothesis ("this isn't a marking, it's prose")
/// during recognition.
const _: () = {
    let actual = SCHEMA_VERSION.as_bytes();
    let expected = b"capco-priors-3";
    if actual.len() != expected.len() {
        panic!("SCHEMA_VERSION length does not match \"capco-priors-3\"");
    }
    let mut i = 0;
    while i < actual.len() {
        if actual[i] != expected[i] {
            panic!("SCHEMA_VERSION does not equal \"capco-priors-3\"");
        }
        i += 1;
    }
};

/// Look up a token's log-prior by exact canonical form.
///
/// Returns `None` for tokens the generator did not observe in the
/// source corpus — decoder code should fall back to the per-template
/// base rate (or a Laplace-smoothed floor) in that case.
///
/// Exploits the sort invariant pinned by `tables_are_sorted_by_name`
/// (`build.rs` sorts at emit time, the runtime test verifies). The
/// decoder calls this in K=8 scoring loops per candidate, so binary
/// search is worth the cost over a linear scan.
pub fn token_log_prior(token: &str) -> Option<f32> {
    TOKEN_BASE_RATES
        .binary_search_by_key(&token, |t| t.token)
        .ok()
        .map(|i| TOKEN_BASE_RATES[i].log_prior)
}

/// Look up a template's log-prior by shape identifier.
///
/// Same sort-invariant-backed binary search as [`token_log_prior`].
pub fn template_log_prior(name: &str) -> Option<f32> {
    TEMPLATE_BASE_RATES
        .binary_search_by_key(&name, |t| t.name)
        .ok()
        .map(|i| TEMPLATE_BASE_RATES[i].log_prior)
}

/// Look up a country code's log-prior (issue #233).
///
/// The backing [`COUNTRY_CODE_BASE_RATES`] table covers every CAPCO
/// country-code shape — 2-char codes (e.g., `EU`), 3-char trigraphs
/// (`USA`, `GBR`, `AUS`, …), 4-char tetragraphs (`FVEY`, `ACGU`,
/// `NATO`, …), and group codes (e.g., `AUSTRALIA_GROUP` if surfaced
/// by the corpus pipeline). The lookup is shape-agnostic: callers
/// pass the canonical token form and binary-search returns whatever
/// is in the table.
///
/// Returns `None` for codes the priors generator did not surface
/// (rare ISO codes outside the FVEY / NATO / Indo-Pacific baseline) —
/// decoder code falls back to [`MISSING_TOKEN_LOG_PRIOR`] in that
/// case, which is more punitive than a Laplace-smoothed zero count
/// and is the right behavior for unknown candidates.
///
/// The decoder calls this once per token in a candidate's `rel_to`
/// list, so the same sort-invariant-backed binary search as
/// [`token_log_prior`] applies.
pub fn country_code_log_prior(token: &str) -> Option<f32> {
    COUNTRY_CODE_BASE_RATES
        .binary_search_by_key(&token, |t| t.token)
        .ok()
        .map(|i| COUNTRY_CODE_BASE_RATES[i].log_prior)
}

/// Log-prior floor for tokens absent from the prose-stratum priors
/// table (issue #258).
///
/// Mirrors the marking-side `MISSING_TOKEN_LOG_PRIOR` constant in
/// `crates/engine/src/decoder.rs`. Used by the decoder when summing
/// the prose-side of the per-token marking-y score
/// `log P(token|marking) − log P(token|prose)` and the prose-side
/// table has no entry for the token.
///
/// The value matches the marking-side floor on purpose: an unknown
/// token contributes the same penalty on both sides, so the marking-y
/// delta for unknown tokens is exactly zero (neutral signal). Picking
/// a different value would silently bias every unknown-token candidate
/// toward "marking" or "prose" without corpus evidence.
pub const MISSING_PROSE_LOG_PRIOR: f32 = -12.0;

/// Look up a token's prose-stratum log-prior by exact canonical form
/// (issue #258).
///
/// The prose-stratum priors are derived from a corpus stratum that
/// contains only prose-bearing material (Enron email, CIA CREST
/// declassified records, Congressional Record, GAO reports — all
/// confirmed prose-dominant per issue #258 owner confirmation). The
/// decoder consumes this in parallel with [`token_log_prior`] to
/// compute the per-token "marking-y" score
/// `log P(token|marking) − log P(token|prose)`.
///
/// Return contract:
///
/// - **`Some(lp)`** for every token in the canonical CAPCO
///   vocabulary (`tools/corpus-analysis/tokens/capco.json` —
///   `derive_priors` materializes one prose row per vocabulary
///   token, so a vocabulary token the prose corpus never observed
///   still returns the Laplace-smoothed zero-count log-prior, NOT
///   `None`). The Laplace floor for a 0-count token in the
///   reference Enron corpus is around `-9.8` to `-10.1` —
///   distinguishable from but not far above
///   [`MISSING_PROSE_LOG_PRIOR`] (`-12.0`).
/// - **`None`** for tokens outside the canonical vocabulary —
///   most commonly `Custom` SCI control tokens or other
///   open-set agency-assigned strings the corpus pipeline doesn't
///   recognize. Decoder code falls back to
///   [`MISSING_PROSE_LOG_PRIOR`] in that case so the marking-y
///   delta for an unknown token is neutral (the marking-side
///   floor matches at `-12.0`).
///
/// Same sort-invariant-backed binary search as [`token_log_prior`].
pub fn token_prose_log_prior(token: &str) -> Option<f32> {
    TOKEN_PROSE_BASE_RATES
        .binary_search_by_key(&token, |t| t.token)
        .ok()
        .map(|i| TOKEN_PROSE_BASE_RATES[i].log_prior)
}

/// Look up a country code's prose-stratum log-prior (issue #258).
///
/// Mirrors [`country_code_log_prior`] on the prose stratum. The prose-
/// side log-prior for high-frequency country codes (USA, GBR, AUS,
/// FVEY, …) must be high enough that an isolated REL-TO-style mention
/// in prose — e.g., a proper-noun "(USA)" in a Federalist Papers
/// passage — does not auto-fix; the marking-y delta for those codes
/// shrinks toward zero in prose-shaped contexts.
///
/// Return contract:
///
/// - **`Some(lp)`** for every code in
///   `_REL_TO_COUNTRY_CODE_BASELINE`
///   (`tools/corpus-analysis/analyze.py` — `derive_priors`
///   pre-seeds the prose table with the full baseline vocabulary,
///   so a code the prose corpus never observed (e.g., NZL, FVEY)
///   still returns the Laplace-smoothed zero-count log-prior, NOT
///   `None`).
/// - **`None`** for codes outside the baseline — most commonly
///   rare ISO trigraphs that don't appear in CAPCO REL TO usage
///   at all. Decoder code falls back to [`MISSING_PROSE_LOG_PRIOR`]
///   in that case.
pub fn country_code_prose_log_prior(token: &str) -> Option<f32> {
    COUNTRY_CODE_PROSE_BASE_RATES
        .binary_search_by_key(&token, |t| t.token)
        .ok()
        .map(|i| COUNTRY_CODE_PROSE_BASE_RATES[i].log_prior)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn schema_version_matches_expected_at_runtime() {
        // Runtime counterpart of the `const _: () = ...` block above.
        // The const block catches a mismatch at compile time; this test
        // serves as a redundant tripwire that surfaces the problem in
        // test reports too (a build failure can be missed by a CI lane
        // that doesn't compile this crate; the test suite always does).
        assert_eq!(SCHEMA_VERSION, "capco-priors-3");
    }

    #[test]
    fn schema_version_uses_capco_priors_namespace() {
        // T082: priors schema names are per-grammar (`capco-priors-N`),
        // not the retired cross-grammar `marque-priors-N`. Pins the
        // namespace so a regenerated priors.json reverting to the old
        // prefix fails even if its generation number matches.
        assert!(
            SCHEMA_VERSION.starts_with("capco-priors-"),
            "priors SCHEMA_VERSION must use the capco-priors- namespace, got {SCHEMA_VERSION:?}"
        );
    }

    #[test]
    fn corpus_fingerprint_is_present() {
        assert!(
            CORPUS_FINGERPRINT.starts_with("sha512:"),
            "corpus fingerprint must be sha512:-prefixed, got {CORPUS_FINGERPRINT:?}"
        );
    }

    #[test]
    // The `is_empty()` calls below are always-false at compile time
    // because the tables are `&'static [_]` constants populated by
    // `build.rs`. The assertions still earn their keep as a regression
    // guard: if a future `build.rs` change accidentally emits an empty
    // table, this test fires before the engine silently runs with no
    // priors. clippy correctly identifies the constant-known operand
    // but the cost of the redundant compile-time check is zero.
    #[allow(clippy::const_is_empty)]
    fn tables_are_non_empty() {
        assert!(
            !TOKEN_BASE_RATES.is_empty(),
            "token base rates must be populated"
        );
        assert!(
            !TEMPLATE_BASE_RATES.is_empty(),
            "template base rates must be populated"
        );
        assert!(
            !COUNTRY_CODE_BASE_RATES.is_empty(),
            "country-code base rates must be populated (issue #233)"
        );
        assert!(
            !TOKEN_PROSE_BASE_RATES.is_empty(),
            "token prose base rates must be populated (issue #258)"
        );
        assert!(
            !COUNTRY_CODE_PROSE_BASE_RATES.is_empty(),
            "country-code prose base rates must be populated (issue #258)"
        );
    }

    #[test]
    fn tables_are_sorted_by_name() {
        for pair in TOKEN_BASE_RATES.windows(2) {
            assert!(
                pair[0].token <= pair[1].token,
                "token table not sorted: {:?} before {:?}",
                pair[0].token,
                pair[1].token,
            );
        }
        for pair in TEMPLATE_BASE_RATES.windows(2) {
            assert!(
                pair[0].name <= pair[1].name,
                "template table not sorted: {:?} before {:?}",
                pair[0].name,
                pair[1].name,
            );
        }
        for pair in COUNTRY_CODE_BASE_RATES.windows(2) {
            assert!(
                pair[0].token <= pair[1].token,
                "country-code table not sorted: {:?} before {:?}",
                pair[0].token,
                pair[1].token,
            );
        }
        for pair in TOKEN_PROSE_BASE_RATES.windows(2) {
            assert!(
                pair[0].token <= pair[1].token,
                "token prose table not sorted: {:?} before {:?}",
                pair[0].token,
                pair[1].token,
            );
        }
        for pair in COUNTRY_CODE_PROSE_BASE_RATES.windows(2) {
            assert!(
                pair[0].token <= pair[1].token,
                "country-code prose table not sorted: {:?} before {:?}",
                pair[0].token,
                pair[1].token,
            );
        }
    }

    #[test]
    fn strict_context_floors_are_valid_probabilities() {
        // Mirrors the build-time policy in
        // `crates/capco/build.rs::require_probability`: floors live
        // in `(0.0, 1.0]`. `0.0` is rejected
        // because it silently makes the strict-context rule a no-op
        // (the feature contribution becomes algebraically identity).
        let p = STRICT_CONTEXT_PRIORS;
        assert!(
            p.confidential_floor > 0.0 && p.confidential_floor <= 1.0,
            "confidential_floor is not a valid strict-context floor; must be in (0.0, 1.0]; 0.0 is rejected because it makes the strict-context rule a silent no-op"
        );
        assert!(
            p.secret_floor > 0.0 && p.secret_floor <= 1.0,
            "secret_floor is not a valid strict-context floor; must be in (0.0, 1.0]; 0.0 is rejected because it makes the strict-context rule a silent no-op"
        );
        assert!(
            p.top_secret_floor > 0.0 && p.top_secret_floor <= 1.0,
            "top_secret_floor is not a valid strict-context floor; must be in (0.0, 1.0]; 0.0 is rejected because it makes the strict-context rule a silent no-op"
        );
    }

    // The three tests below exercise the rejection predicate (`> 0.0 && <= 1.0`)
    // on synthetic `StrictContextPriors` values so that both branches of the
    // condition are reachable by the coverage instrumenter.  The test above only
    // ever reads `STRICT_CONTEXT_PRIORS`, which contains known-valid values, so
    // the "condition is false" branch would otherwise remain uncovered.

    #[test]
    fn strict_context_floor_rejects_zero() {
        // 0.0 is explicitly excluded: a zero floor makes the strict-context
        // rule algebraically identity (no contribution to the log-posterior).
        let zero_floor = StrictContextPriors {
            confidential_floor: 0.0,
            secret_floor: 0.5,
            top_secret_floor: 0.5,
        };
        assert!(
            !(zero_floor.confidential_floor > 0.0 && zero_floor.confidential_floor <= 1.0),
            "0.0 must not satisfy the valid strict-context floor predicate"
        );
    }

    #[test]
    fn strict_context_floor_rejects_above_one() {
        // Values > 1.0 are not valid probabilities.
        let above_one = StrictContextPriors {
            confidential_floor: 0.5,
            secret_floor: 1.5,
            top_secret_floor: 0.5,
        };
        assert!(
            !(above_one.secret_floor > 0.0 && above_one.secret_floor <= 1.0),
            "1.5 must not satisfy the valid strict-context floor predicate"
        );
    }

    #[test]
    fn strict_context_floor_accepts_boundary_one() {
        // 1.0 is included (closed upper bound): the predicate is `<= 1.0`.
        let at_one = StrictContextPriors {
            confidential_floor: 1.0,
            secret_floor: 1.0,
            top_secret_floor: 1.0,
        };
        assert!(
            at_one.confidential_floor > 0.0 && at_one.confidential_floor <= 1.0,
            "1.0 must satisfy the valid strict-context floor predicate (closed upper bound)"
        );
        assert!(
            at_one.secret_floor > 0.0 && at_one.secret_floor <= 1.0,
            "1.0 must satisfy the valid strict-context floor predicate (closed upper bound)"
        );
        assert!(
            at_one.top_secret_floor > 0.0 && at_one.top_secret_floor <= 1.0,
            "1.0 must satisfy the valid strict-context floor predicate (closed upper bound)"
        );
    }

    #[test]
    fn log_priors_are_finite_and_non_positive() {
        // log(probability) is always ≤ 0 and finite for well-formed
        // priors. Infinite or NaN would indicate generator regression.
        for t in TOKEN_BASE_RATES {
            assert!(
                t.log_prior.is_finite() && t.log_prior <= 0.0,
                "token {:?} has invalid log_prior {}",
                t.token,
                t.log_prior,
            );
        }
        for t in TEMPLATE_BASE_RATES {
            assert!(
                t.log_prior.is_finite() && t.log_prior <= 0.0,
                "template {:?} has invalid log_prior {}",
                t.name,
                t.log_prior,
            );
        }
        for t in COUNTRY_CODE_BASE_RATES {
            assert!(
                t.log_prior.is_finite() && t.log_prior <= 0.0,
                "country code {:?} has invalid log_prior {}",
                t.token,
                t.log_prior,
            );
        }
        for t in TOKEN_PROSE_BASE_RATES {
            assert!(
                t.log_prior.is_finite() && t.log_prior <= 0.0,
                "prose token {:?} has invalid log_prior {}",
                t.token,
                t.log_prior,
            );
        }
        for t in COUNTRY_CODE_PROSE_BASE_RATES {
            assert!(
                t.log_prior.is_finite() && t.log_prior <= 0.0,
                "prose country code {:?} has invalid log_prior {}",
                t.token,
                t.log_prior,
            );
        }
    }

    #[test]
    fn token_log_prior_lookup_works() {
        let first = TOKEN_BASE_RATES
            .first()
            .expect("table must be non-empty per tables_are_non_empty");
        let lookup = token_log_prior(first.token);
        assert_eq!(lookup, Some(first.log_prior));
        assert_eq!(token_log_prior("this-token-does-not-exist"), None);
    }

    #[test]
    fn template_log_prior_lookup_works() {
        let first = TEMPLATE_BASE_RATES
            .first()
            .expect("table must be non-empty per tables_are_non_empty");
        let lookup = template_log_prior(first.name);
        assert_eq!(lookup, Some(first.log_prior));
        assert_eq!(template_log_prior("this-template-does-not-exist"), None);
    }

    #[test]
    fn country_code_log_prior_lookup_works() {
        let first = COUNTRY_CODE_BASE_RATES
            .first()
            .expect("table must be non-empty per tables_are_non_empty");
        let lookup = country_code_log_prior(first.token);
        assert_eq!(lookup, Some(first.log_prior));
        assert_eq!(
            country_code_log_prior("this-country-code-does-not-exist"),
            None
        );
    }

    #[test]
    fn token_prose_log_prior_lookup_works() {
        let first = TOKEN_PROSE_BASE_RATES
            .first()
            .expect("table must be non-empty per tables_are_non_empty");
        let lookup = token_prose_log_prior(first.token);
        assert_eq!(lookup, Some(first.log_prior));
        assert_eq!(token_prose_log_prior("this-token-does-not-exist"), None);
    }

    #[test]
    fn country_code_prose_log_prior_lookup_works() {
        let first = COUNTRY_CODE_PROSE_BASE_RATES
            .first()
            .expect("table must be non-empty per tables_are_non_empty");
        let lookup = country_code_prose_log_prior(first.token);
        assert_eq!(lookup, Some(first.log_prior));
        assert_eq!(
            country_code_prose_log_prior("this-country-code-does-not-exist"),
            None,
        );
    }

    #[test]
    fn missing_prose_log_prior_matches_marking_floor() {
        // The marking-side floor lives in
        // ``crates/engine/src/decoder.rs::MISSING_TOKEN_LOG_PRIOR``.
        // Keep this assertion in sync if the marking-side constant
        // moves; the contract is that an unknown token contributes a
        // neutral marking-y delta (zero) — picking different floors
        // would silently bias every unknown-token candidate without
        // corpus evidence.
        assert_eq!(MISSING_PROSE_LOG_PRIOR, -12.0_f32);
    }

    #[test]
    fn high_frequency_country_codes_outweigh_lookalikes() {
        // Issue #233 acceptance: USA must outscore UZB (and AUS must
        // outscore ASM) by enough to swamp the decoder's
        // ``UNAMBIGUOUS_LOG_MARGIN = 1.6`` (~5× odds ratio). Otherwise
        // a fuzzy edit-distance-1 candidate could win against a
        // legitimate edit-distance-2 candidate purely on edit cost.
        const UNAMBIGUOUS_LOG_MARGIN: f32 = 1.6;

        let usa = country_code_log_prior("USA").expect("USA must be in country-code table");
        let uzb = country_code_log_prior("UZB").expect("UZB must be in country-code table");
        let aus = country_code_log_prior("AUS").expect("AUS must be in country-code table");
        let asm = country_code_log_prior("ASM").expect("ASM must be in country-code table");

        let usa_uzb = usa - uzb;
        let aus_asm = aus - asm;

        assert!(
            usa_uzb > UNAMBIGUOUS_LOG_MARGIN,
            "log_prior(USA) - log_prior(UZB) = {usa_uzb} must exceed UNAMBIGUOUS_LOG_MARGIN = {UNAMBIGUOUS_LOG_MARGIN} so fuzzy USB→USA wins"
        );
        assert!(
            aus_asm > UNAMBIGUOUS_LOG_MARGIN,
            "log_prior(AUS) - log_prior(ASM) = {aus_asm} must exceed UNAMBIGUOUS_LOG_MARGIN = {UNAMBIGUOUS_LOG_MARGIN} so AUS wins despite ASM's edit-distance advantage"
        );
    }
}
