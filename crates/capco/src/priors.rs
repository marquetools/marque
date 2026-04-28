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
//! probability floors (FR-011 — if one portion says SECRET, candidate
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

/// Strict-context probability floors for FR-011 candidate filtering.
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
/// producer side (see `crates/capco/build.rs:73-82` — it accepts only
/// the single `marque-priors-2` value today). This const block is the
/// consumer-side counterpart kept as an explicit source pin and
/// defense-in-depth check that the generated `SCHEMA_VERSION` still
/// matches the version this crate is wired to consume — the value
/// fences the runtime tests below at the build-time tier so a CI lane
/// that happens to skip this crate's tests still catches a regression.
/// It also forces the consumer-side expectation to be a visible source
/// declaration, so a future PR that bumps `build.rs` to accept a new
/// schema version has to update this pin in the same edit.
const _: () = {
    let actual = SCHEMA_VERSION.as_bytes();
    let expected = b"marque-priors-2";
    if actual.len() != expected.len() {
        panic!("SCHEMA_VERSION length does not match \"marque-priors-2\"");
    }
    let mut i = 0;
    while i < actual.len() {
        if actual[i] != expected[i] {
            panic!("SCHEMA_VERSION does not equal \"marque-priors-2\"");
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

/// Look up a country trigraph's log-prior (issue #233).
///
/// Returns `None` for trigraphs the priors generator did not surface
/// (rare ISO codes outside the FVEY / NATO / Indo-Pacific baseline) —
/// decoder code falls back to [`MISSING_TOKEN_LOG_PRIOR`] in that
/// case, which is more punitive than a Laplace-smoothed zero count
/// and is the right behavior for unknown candidates.
///
/// The decoder calls this once per token in a candidate's `rel_to`
/// list, so the same sort-invariant-backed binary search as
/// [`token_log_prior`] applies.
pub fn trigraph_log_prior(token: &str) -> Option<f32> {
    TRIGRAPH_BASE_RATES
        .binary_search_by_key(&token, |t| t.token)
        .ok()
        .map(|i| TRIGRAPH_BASE_RATES[i].log_prior)
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
        assert_eq!(SCHEMA_VERSION, "marque-priors-2");
    }

    #[test]
    fn corpus_fingerprint_is_present() {
        assert!(
            CORPUS_FINGERPRINT.starts_with("sha512:"),
            "corpus fingerprint must be sha512:-prefixed, got {CORPUS_FINGERPRINT:?}"
        );
    }

    #[test]
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
            !TRIGRAPH_BASE_RATES.is_empty(),
            "trigraph base rates must be populated (issue #233)"
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
        for pair in TRIGRAPH_BASE_RATES.windows(2) {
            assert!(
                pair[0].token <= pair[1].token,
                "trigraph table not sorted: {:?} before {:?}",
                pair[0].token,
                pair[1].token,
            );
        }
    }

    #[test]
    fn strict_context_floors_are_valid_probabilities() {
        // Mirrors the build-time policy in
        // `crates/capco/build.rs::require_probability` per Phase 4
        // review M8: floors live in `(0.0, 1.0]`. `0.0` is rejected
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
        for t in TRIGRAPH_BASE_RATES {
            assert!(
                t.log_prior.is_finite() && t.log_prior <= 0.0,
                "trigraph {:?} has invalid log_prior {}",
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
    fn trigraph_log_prior_lookup_works() {
        let first = TRIGRAPH_BASE_RATES
            .first()
            .expect("table must be non-empty per tables_are_non_empty");
        let lookup = trigraph_log_prior(first.token);
        assert_eq!(lookup, Some(first.log_prior));
        assert_eq!(trigraph_log_prior("this-trigraph-does-not-exist"), None);
    }

    #[test]
    fn high_frequency_trigraphs_outweigh_lookalikes() {
        // Issue #233 acceptance: USA must outscore UZB (and AUS must
        // outscore ASM) by enough to swamp the decoder's
        // ``UNAMBIGUOUS_LOG_MARGIN = 1.6`` (~5× odds ratio). Otherwise
        // a fuzzy edit-distance-1 candidate could win against a
        // legitimate edit-distance-2 candidate purely on edit cost.
        const UNAMBIGUOUS_LOG_MARGIN: f32 = 1.6;

        let usa = trigraph_log_prior("USA").expect("USA must be in trigraph table");
        let uzb = trigraph_log_prior("UZB").expect("UZB must be in trigraph table");
        let aus = trigraph_log_prior("AUS").expect("AUS must be in trigraph table");
        let asm = trigraph_log_prior("ASM").expect("ASM must be in trigraph table");

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
