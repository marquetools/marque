// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decoder provenance — side channel that carries the
//! probabilistic recognizer's canonical-bytes attempt and feature trace
//! out of [`Recognizer::recognize`](marque_scheme::recognizer::Recognizer)
//! so the engine can emit a `FixSource::DecoderPosterior` fix without
//! re-running the decoder.
//!
//! ## Why this lives on `CapcoMarking`, not on `Parsed::Unambiguous`
//!
//! The `Parsed<M>::Unambiguous(M)` shape is fixed at `marque-scheme`
//! (domain-neutral) and changing it would require a coordinated
//! breaking change across every potential domain crate (CAPCO today,
//! CUI / NATO / JOINT later). The `CapcoMarking` type is already the
//! domain-specific seam between the recognizer and the engine, so
//! attaching optional provenance there keeps the trait surface
//! stable while still letting the engine see "this recognition went
//! through the decoder fallback, here is the canonical form it
//! resolved to."
//!
//! Strict-path recognizers leave [`CapcoMarking::provenance`] as
//! `None`; the engine treats `None` as "strict path, no decoder
//! evidence to record."
//!
//! ## What the engine does with this
//!
//! When `CapcoMarking::provenance` is `Some(p)` after a recognition,
//! `Engine::lint` emits a synthetic [`R001 decoder-recognition`]
//! diagnostic whose [`FixProposal`](marque_rules::FixProposal)
//! rewrites the original byte span to `p.canonical_bytes`. The fix's
//! `confidence` is built from `p.posterior` (mapped to a `[0.0, 1.0]`
//! recognition score via softmax over top vs. runner-up) and carries
//! `p.runner_up_ratio` and `p.features` verbatim. The `source` is
//! `FixSource::DecoderPosterior`, locking the audit-record provenance
//! per the data-model spec.

use marque_rules::{FeatureContribution, FixSource};

/// Provenance trace recorded when a probabilistic recognizer (the
/// probabilistic decoder) produces a marking. Strict-path recognizers leave
/// the corresponding `CapcoMarking::provenance` field as `None`.
///
/// Fields:
///
/// - `canonical_bytes` — the canonicalized byte string the decoder
///   accepted. Used by the engine as the replacement text in the
///   synthetic decoder fix.
/// - `posterior` — the natural-log posterior of the top candidate
///   (`prior + Σ feature.delta`, in nats). Negative; closer to zero
///   means more probable.
/// - `runner_up_ratio` — `exp(top.posterior - runner_up.posterior)`
///   i.e. the odds ratio between the top candidate and the second-
///   best. `None` when the decoder's K-truncated set had only one
///   surviving candidate. Threaded into
///   [`Confidence::runner_up_ratio`](marque_rules::Confidence::runner_up_ratio)
///   verbatim.
/// - `features` — the per-feature `FeatureContribution` deltas the
///   decoder recorded while canonicalizing. Threaded into
///   [`Confidence::features`](marque_rules::Confidence::features)
///   verbatim.
/// - `fix_source` — provenance discriminator for the decoder fix
///   path. [`FixSource::DecoderPosterior`] for vocab-based corrections
///   (the default decoder pipeline);
///   [`FixSource::DecoderClassificationHeuristic`] for fixes produced
///   by the position-aware short-token classification heuristic
///   (issue #133). The engine reads this to choose
///   `Severity::Fix` vs `Severity::Warn` and to cap
///   `Confidence::rule` for the heuristic path so the combined
///   confidence stays below the default `confidence_threshold` of
///   `0.95` — the heuristic is always-visible but only auto-applies
///   when the user explicitly opts into a lower threshold.
///
/// Held as `Box<[T]>` (not `Vec<T>`) so the in-memory size after
/// recognition is the smallest legal representation — markings flow
/// through `Engine::lint` in tight loops, and a 24-byte `Vec` header
/// per non-decoder-path marking would inflate the strict-path hot
/// path for no benefit.
///
/// Marked `#[non_exhaustive]` so additional discriminators / sidecar
/// fields can land in future PRs without breaking external
/// constructors. Internal construction is in
/// `marque_engine::decoder` (the decoder pipeline) and
/// `marque_capco::provenance::tests`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DecoderProvenance {
    /// Canonicalized bytes that strict-parsed under the decoder.
    pub canonical_bytes: Box<[u8]>,
    /// Natural-log posterior of the top candidate (in nats).
    pub posterior: f32,
    /// `exp(top.posterior - runner_up.posterior)` when a runner-up
    /// exists; `None` when the K-truncated set collapsed to one.
    pub runner_up_ratio: Option<f32>,
    /// Per-feature contributions recorded during canonicalization.
    pub features: Box<[FeatureContribution]>,
    /// Provenance discriminator for the decoder fix path. See the
    /// type-level doc above for the engine's interpretation.
    pub fix_source: FixSource,
}

impl DecoderProvenance {
    /// Construct a `DecoderProvenance` record. Required because the
    /// struct is `#[non_exhaustive]` and cannot be built with
    /// struct-literal syntax from other crates (the decoder lives in
    /// `marque-engine`, this type lives in `marque-capco`).
    ///
    /// All fields documented on the struct itself.
    pub fn new(
        canonical_bytes: Box<[u8]>,
        posterior: f32,
        runner_up_ratio: Option<f32>,
        features: Box<[FeatureContribution]>,
        fix_source: FixSource,
    ) -> Self {
        Self {
            canonical_bytes,
            posterior,
            runner_up_ratio,
            features,
            fix_source,
        }
    }

    /// Convert the decoder's natural-log posterior into a recognition
    /// confidence in `[0.0, 1.0)` for `Confidence::recognition`.
    ///
    /// Formula: softmax over top vs. runner-up using the stored
    /// `runner_up_ratio` (= `exp(top - runner_up)`). When no
    /// runner-up exists, returns the asymptote `1.0 - epsilon` so
    /// the strict-path invariant `recognition == 1.0` (Constitution
    /// V audit-record contract) still distinguishes strict from
    /// decoder.
    ///
    /// `epsilon = 1e-6` is well below any realistic decoder
    /// posterior precision (`f32` has ~7 decimal digits) so callers
    /// comparing `recognition < 1.0` see the decoder-path branch
    /// reliably without worrying about exact-equality artifacts.
    pub fn recognition_score(&self) -> f32 {
        const SOLO_RECOGNITION: f32 = 1.0 - 1e-6;
        match self.runner_up_ratio {
            Some(ratio) if ratio.is_finite() && ratio > 0.0 => {
                // softmax(top, runner_up) = exp(top) / (exp(top) +
                // exp(runner_up)) = ratio / (ratio + 1.0).
                let score = ratio / (ratio + 1.0);
                // Clamp into [0.0, SOLO_RECOGNITION] — never collide
                // with the strict 1.0 sentinel even if numerical
                // round-up pushes a near-asymptotic ratio over.
                score.clamp(0.0, SOLO_RECOGNITION)
            }
            _ => SOLO_RECOGNITION,
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_rules::{FeatureContribution, FeatureId};

    fn provenance(ratio: Option<f32>) -> DecoderProvenance {
        DecoderProvenance {
            canonical_bytes: Box::from(b"SECRET//NOFORN".as_slice()),
            posterior: -2.0,
            runner_up_ratio: ratio,
            features: Box::from([FeatureContribution {
                id: FeatureId::EditDistance1,
                delta: -0.5,
            }]),
            fix_source: FixSource::DecoderPosterior,
        }
    }

    #[test]
    fn recognition_for_solo_candidate_is_just_below_one() {
        // No runner-up — recognition saturates just below 1.0 so a
        // `recognition < 1.0` check still distinguishes decoder path
        // from strict path.
        let p = provenance(None);
        let score = p.recognition_score();
        assert!(score < 1.0, "decoder-path recognition must be < 1.0");
        assert!(
            score > 0.99,
            "solo candidate should saturate near 1.0, got {score}"
        );
    }

    #[test]
    fn recognition_softmax_at_unambiguous_threshold() {
        // The decoder's UNAMBIGUOUS_LOG_MARGIN is 1.6 nats, so
        // ratio = exp(1.6) ≈ 4.953. softmax = 4.953 / 5.953 ≈ 0.832.
        let p = provenance(Some(1.6_f32.exp()));
        let score = p.recognition_score();
        assert!(
            (score - (4.953 / 5.953)).abs() < 0.01,
            "expected ~0.832 at the 1.6-nat threshold, got {score}"
        );
    }

    #[test]
    fn recognition_clamps_invalid_ratio_to_solo() {
        // A non-finite or non-positive ratio (defensive — the decoder
        // should never produce this) collapses to the solo asymptote
        // rather than letting NaN poison the audit record.
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0] {
            let p = provenance(Some(bad));
            let score = p.recognition_score();
            assert!(
                score.is_finite() && score < 1.0,
                "ratio = {bad} should produce a finite, sub-1.0 \
                 recognition; got {score}"
            );
        }
    }
}
