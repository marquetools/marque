// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Confidence — Phase D audit-provenance payload.
//!
//! Every [`FixProposal`](crate::FixProposal) carries a `Confidence`
//! record describing how the engine arrived at the proposal. The
//! record stores two primary scalar confidence axes —
//! `recognition` and `rule` — plus optional auxiliary fields
//! (`region` and `runner_up_ratio`) and a list of named feature
//! contributions. Together they reconstruct the decoder's scoring
//! path so an auditor can verify *why* a given fix was promoted.
//!
//! The engine's current threshold-facing combined score is
//! `recognition * rule` as exposed by [`Confidence::combined`].
//! `region` is recorded as additional audit/context metadata when
//! available, but it does not currently participate in that
//! combined score. `runner_up_ratio` likewise provides decoder
//! provenance rather than a direct multiplicative/additive input to
//! `combined()`.
//!
//! ## Precision: `f32` throughout
//!
//! All scores are `f32`. The decoder scores in `f64` internally
//! (log-priors and posteriors accumulate across many features), but
//! the emitted `Confidence` downcasts once at the boundary so the
//! audit record stays compact and byte-stable. This matches the
//! foundational-plan invariant line 739-757.
//!
//! ## `features` is closed
//!
//! [`FeatureId`] is a non-`#[non_exhaustive]` closed enum. A new
//! feature means a new variant and a coordinated bump of the audit
//! schema version (`MARQUE_AUDIT_SCHEMA`) — silent additions would
//! break the auditability contract on already-emitted records.

/// Multi-axis confidence attached to every [`FixProposal`](crate::FixProposal).
///
/// Fields:
///
/// - `recognition` — posterior from the [`Recognizer`](marque_scheme::Recognizer)
///   that surfaced this candidate (0.0–1.0).
/// - `rule` — confidence the emitting rule has in its own fix
///   (0.0–1.0). Strict-path rules report 1.0 when the invariant is
///   unambiguous.
/// - `region` — optional region-level confidence (a page-context
///   prior, for example).
/// - `runner_up_ratio` — optional ratio of top candidate to runner-up
///   posterior. Decoder-sourced fixes carry this; strict-path fixes
///   leave it `None` because the strict grammar has no runner-up by
///   construction.
/// - `features` — the concrete evidence features that contributed to
///   `recognition`. Used by the corpus-accuracy harness to break down
///   where posterior mass came from.
///
/// Construction happens via [`Confidence::strict`] (for rules that
/// bypass the decoder) or the decoder's scoring path (Phase 4 / task
/// T061).
#[derive(Debug, Clone, PartialEq)]
pub struct Confidence {
    /// Recognizer posterior in `[0.0, 1.0]`.
    pub recognition: f32,
    /// Rule-level confidence in `[0.0, 1.0]`.
    pub rule: f32,
    /// Region / page-context confidence, when a rule computes one.
    pub region: Option<f32>,
    /// Posterior ratio between top candidate and runner-up
    /// (`None` for strict-path fixes; set by decoder-sourced fixes).
    pub runner_up_ratio: Option<f32>,
    /// Per-feature contributions to `recognition`.
    pub features: Vec<FeatureContribution>,
}

impl Confidence {
    /// Confidence record for a strict-path fix where recognition was
    /// unambiguous.
    ///
    /// `rule_confidence` is the rule's own confidence in its proposed
    /// fix (typically 1.0 for migrations, lower for heuristics). The
    /// recognition axis is pinned at 1.0 because the strict grammar
    /// has one unambiguous match by definition, and no feature
    /// contributions are recorded — strict-path fixes do not traverse
    /// the decoder's feature graph.
    #[inline]
    pub fn strict(rule_confidence: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&rule_confidence) && !rule_confidence.is_nan(),
            "Confidence::strict rule confidence must be in [0.0, 1.0] and not NaN, got {rule_confidence}"
        );
        Self {
            recognition: 1.0,
            rule: rule_confidence,
            region: None,
            runner_up_ratio: None,
            features: Vec::new(),
        }
    }

    /// Product of `recognition` and `rule`. The engine's
    /// confidence-threshold gate compares this combined score against
    /// the configured threshold (FR-016).
    #[inline]
    pub fn combined(&self) -> f32 {
        self.recognition * self.rule
    }
}

/// One named contribution to [`Confidence::recognition`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureContribution {
    /// Which feature.
    pub id: FeatureId,
    /// Signed delta added to the log-posterior by this feature.
    pub delta: f32,
}

/// Closed enumeration of features the decoder can record.
///
/// New variants MUST bump the audit schema version (see
/// `MARQUE_AUDIT_SCHEMA` in `marque-engine/build.rs`). Treat this
/// enum as part of the on-the-wire audit contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureId {
    /// Observed form is edit-distance 1 from a canonical token.
    EditDistance1,
    /// Observed form is edit-distance 2 from a canonical token.
    EditDistance2,
    /// Observed form is a token-order permutation of a canonical
    /// banner/portion shape.
    TokenReorder,
    /// Observed form is a known CAPCO-2016-superseded token whose
    /// replacement is unambiguous (e.g., `COMINT → SI`).
    SupersededToken,
    /// The candidate's base rate in the target corpus dominates the
    /// posterior (common-marking prior).
    BaseRateCommonMarking,
    /// Strict-context classification floor (FR-011) applied — e.g.,
    /// banner at TOP SECRET forces a strict posterior for
    /// classification tokens at ≥ that level on the same page.
    StrictContextClassification,
    /// Corpus-override data (opt-in, non-WASM, non-server) shifted
    /// the posterior. Recorded so an auditor can identify fixes
    /// produced under organizational overrides vs. stock priors.
    CorpusOverrideInEffect,
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn strict_pins_recognition_at_one() {
        let c = Confidence::strict(0.85);
        assert_eq!(c.recognition, 1.0);
        assert_eq!(c.rule, 0.85);
        assert!(c.region.is_none());
        assert!(c.runner_up_ratio.is_none());
        assert!(c.features.is_empty());
    }

    #[test]
    fn combined_is_product_of_axes() {
        let c = Confidence::strict(0.9);
        assert!((c.combined() - 0.9).abs() < 1e-6);

        let c2 = Confidence {
            recognition: 0.8,
            rule: 0.5,
            region: None,
            runner_up_ratio: None,
            features: Vec::new(),
        };
        assert!((c2.combined() - 0.4).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "Confidence::strict rule confidence")]
    fn strict_panics_on_nan() {
        let _ = Confidence::strict(f32::NAN);
    }

    #[test]
    #[should_panic(expected = "Confidence::strict rule confidence")]
    fn strict_panics_above_one() {
        let _ = Confidence::strict(1.01);
    }

    #[test]
    fn feature_contribution_roundtrip() {
        let fc = FeatureContribution {
            id: FeatureId::EditDistance1,
            delta: -0.3,
        };
        assert_eq!(fc.id, FeatureId::EditDistance1);
        assert!((fc.delta - (-0.3)).abs() < 1e-6);
    }
}
