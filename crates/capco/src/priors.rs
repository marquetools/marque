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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_pinned() {
        assert_eq!(SCHEMA_VERSION, "marque-priors-1");
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
    }

    #[test]
    fn strict_context_floors_are_valid_probabilities() {
        let p = STRICT_CONTEXT_PRIORS;
        for (name, value) in [
            ("confidential_floor", p.confidential_floor),
            ("secret_floor", p.secret_floor),
            ("top_secret_floor", p.top_secret_floor),
        ] {
            assert!(
                (0.0..=1.0).contains(&value),
                "{name} = {value} is not a valid probability"
            );
        }
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
}
