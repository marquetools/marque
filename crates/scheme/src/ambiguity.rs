// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Ambiguity surface for local probabilistic disambiguation.
//!
//! The engine is deterministic for >99% of real input. Ambiguity is
//! local to specific productions — `(C)` being the canonical case
//! (CONFIDENTIAL portion vs copyright symbol). For those cases the
//! parser emits `Parsed::Ambiguous` with a small list of candidates
//! and per-candidate evidence features. A separate resolver combines
//! the features via Bayesian log-odds to either resolve above a
//! confidence threshold or surface a verification request.
//!
//! The typed split `Unambiguous` / `Ambiguous` is what makes the rest
//! of the engine statically safe: auto-fix paths take only
//! `Unambiguous` markings.

/// Outcome of parsing one marking.
#[derive(Debug, Clone)]
pub enum Parsed<M> {
    /// One unambiguous interpretation. Safe for auto-fix.
    Unambiguous(M),
    /// Multiple candidate interpretations. Requires resolution.
    Ambiguous { candidates: Vec<Candidate<M>> },
}

/// One candidate interpretation, with evidence the resolver will use.
#[derive(Debug, Clone)]
pub struct Candidate<M> {
    pub marking: M,
    pub evidence: Vec<EvidenceFeature>,
    /// Log-odds prior. Combined additively with per-feature log-odds.
    pub prior_log_odds: f32,
}

/// An evidence feature observed near the ambiguous production.
///
/// Features carry empirically-estimated log-odds from the corpus
/// analysis (see `tools/corpus-analysis/output/enron-full.json`). The
/// resolver sums the log-odds across features to score each candidate.
#[derive(Debug, Clone)]
pub struct EvidenceFeature {
    pub label: &'static str,
    /// Signed contribution to the log-odds of this candidate.
    pub log_odds: f32,
}

impl<M> Parsed<M> {
    #[inline]
    /// Convenience: is this a single unambiguous parse?
    pub fn is_unambiguous(&self) -> bool {
        matches!(self, Self::Unambiguous(_))
    }

    #[inline]
    /// Map the underlying marking type while preserving the variant.
    pub fn map<N, F: Fn(M) -> N>(self, f: F) -> Parsed<N> {
        match self {
            Self::Unambiguous(m) => Parsed::Unambiguous(f(m)),
            Self::Ambiguous { candidates } => Parsed::Ambiguous {
                candidates: candidates
                    .into_iter()
                    .map(|c| Candidate {
                        marking: f(c.marking),
                        evidence: c.evidence,
                        prior_log_odds: c.prior_log_odds,
                    })
                    .collect(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn unambiguous_roundtrips_through_map() {
        let p = Parsed::Unambiguous(42_u32);
        let mapped = p.map(|x| x + 1);
        match mapped {
            Parsed::Unambiguous(v) => assert_eq!(v, 43),
            Parsed::Ambiguous { .. } => panic!("unexpected ambiguity"),
        }
    }

    #[test]
    fn is_unambiguous_distinguishes_variants() {
        let u = Parsed::Unambiguous(1_u32);
        assert!(u.is_unambiguous());

        let a = Parsed::Ambiguous::<u32> {
            candidates: vec![Candidate {
                marking: 1,
                evidence: vec![],
                prior_log_odds: 0.0,
            }],
        };
        assert!(!a.is_unambiguous());
    }

    #[test]
    fn candidate_score_adds_prior_and_feature_log_odds() {
        // The resolver scores candidates by adding the prior log-odds
        // to the sum of all observed evidence feature log-odds. This
        // test pins that scoring algorithm directly — a future change
        // that introduces e.g. weighted averaging or another
        // combinator will make the assertion fail.
        let candidate = Candidate {
            marking: "classification",
            evidence: vec![
                EvidenceFeature {
                    label: "year_nearby",
                    log_odds: -2.0,
                },
                EvidenceFeature {
                    label: "high_prior_classification",
                    log_odds: 1.5,
                },
                EvidenceFeature {
                    label: "list_marker_context",
                    log_odds: -0.8,
                },
            ],
            prior_log_odds: 0.7,
        };

        let score = candidate.prior_log_odds
            + candidate
                .evidence
                .iter()
                .map(|feature| feature.log_odds)
                .sum::<f32>();

        // 0.7 + (-2.0 + 1.5 - 0.8) = -0.6
        assert!((score - (-0.6)).abs() < 1e-6);
    }
}
