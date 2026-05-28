//! Decoder-internal types: scored candidates, feature entries, canonical attempts.
//!
//! These are pure data carriers consumed by the scoring and recognizer pipelines.
//! Field visibility is `pub(super)` (parent-module-visible: the surrounding
//! `decoder/` module) so sibling sub-modules can construct and read them. The
//! items are not re-exported from `decoder/mod.rs` and never leak past it.
//! `pub(super)` is narrower than `pub(crate)` — engine code outside `decoder/`
//! cannot reach these types.

use marque_rules::recognition::FeatureId;
use marque_scheme::ambiguity::EvidenceFeature;
use smallvec::SmallVec;

use marque_capco::CapcoMarking;

/// One scored candidate kept in the decoder's working set.
///
/// `prior` and `posterior` are tracked separately so
/// `Candidate::prior_log_odds` can carry the prior alone (per the
/// trait-level contract in `crates/scheme/src/ambiguity.rs`) while
/// internal sort / threshold decisions use the posterior.
pub(super) struct ScoredCandidate {
    pub(super) marking: CapcoMarking,
    /// Sum of baked corpus log-priors over the marking's canonical
    /// tokens. No feature deltas included.
    pub(super) prior: f32,
    /// `prior + Σ feature.delta`. Used for sorting and threshold
    /// comparisons inside the decoder; not stored in the emitted
    /// `Candidate` record.
    pub(super) posterior: f32,
    /// Prose-side null posterior.
    ///
    /// Sum of [`marque_capco::priors::token_prose_log_prior`] over
    /// the same canonical tokens used for [`Self::prior`], plus
    /// [`marque_capco::priors::country_code_prose_log_prior`] over
    /// the same `rel_to` codes. Carries the prose hypothesis for the
    /// candidate's token set — `log P(tokens | prose)` evaluated
    /// against the prose-stratum corpus.
    ///
    /// The dispatch logic in `DecoderRecognizer::recognize` §6
    /// compares this against [`Self::posterior`] for the top
    /// candidate. When `null_posterior > posterior` the decoder
    /// returns zero candidates (prose wins the null
    /// hypothesis competition, no fix is emitted). When
    /// `null_posterior <= posterior` the null hypothesis becomes a
    /// virtual runner-up that flows into `recognition_score`.
    pub(super) null_posterior: f32,
    /// Canonical byte string the strict parser accepted for this
    /// candidate. Threaded into `DecoderProvenance::canonical_bytes`
    /// when this candidate wins the Unambiguous collapse, so the
    /// engine can emit the decoder fix from the original mangled
    /// bytes to this canonical form.
    pub(super) canonical_bytes: Box<[u8]>,
    /// Per-candidate feature contributions. `SmallVec<[…; 4]>` matches
    /// `Recognition::features` so the inline-4 case stays heap-free
    /// from canonicalization through audit emission.
    pub(super) features: SmallVec<[FeatureEntry; 4]>,
    /// Provenance discriminator carried from the originating
    /// [`CanonicalAttempt`]. The engine maps this to
    /// [`Severity::Fix`](marque_rules::Severity::Fix) for
    /// `DecoderPosterior` and
    /// [`Severity::Warn`](marque_rules::Severity::Warn) for
    /// `DecoderClassificationHeuristic`.
    pub(super) fix_source: marque_rules::FixSource,
}

/// One feature recorded during candidate generation, paired with its
/// log-odds contribution. The decoder accumulates these to reconstruct
/// `Recognition::features` at audit-emit time.
#[derive(Debug, Clone, Copy)]
pub(super) struct FeatureEntry {
    pub(super) id: FeatureId,
    pub(super) delta: f32,
}

/// Project a `FeatureEntry` onto the wire-shape [`EvidenceFeature`].
///
/// Routes the label through [`FeatureId::as_str`] — the single source
/// of truth for the FeatureId → audit-record-string registry declared
/// in `crates/rules/src/confidence.rs`. Lifted out of the inline
/// closure in `DecoderRecognizer::recognize` so the projection is
/// directly testable: a divergent local label registry would now fail
/// `tests::feature_entry_to_evidence_uses_canonical_label_registry`
/// rather than going unnoticed because the dispatcher discards
/// `Parsed::Ambiguous` results today.
pub(super) fn feature_entry_to_evidence(f: &FeatureEntry) -> EvidenceFeature {
    EvidenceFeature {
        label: f.id.as_str(),
        log_odds: f.delta,
    }
}

/// A canonicalization attempt: the byte string the decoder will hand
/// to the strict parser, plus the features that transformation
/// represents. Zero or more attempts are generated per observed input.
///
/// `bytes` and `features` use `SmallVec` inline storage tuned to the
/// empirical distribution of CAPCO markings: most candidate byte
/// strings fit in 64 bytes (portion shapes are tiny; banners can
/// spill to the heap when REL TO grows long) and most attempts record
/// 1–4 features. The inline path saves a per-attempt heap allocation
/// on the deep-scan latency budget.
pub(super) struct CanonicalAttempt {
    pub(super) bytes: SmallVec<[u8; 64]>,
    pub(super) features: SmallVec<[FeatureEntry; 4]>,
    /// Which decoder path produced this attempt. Defaults to
    /// [`marque_rules::FixSource::DecoderPosterior`] for the standard
    /// vocab-based pipeline (delimiter normalization, fuzzy
    /// correction, token reorder, superseded-token replacement).
    /// The position-aware classification heuristic emits attempts
    /// with [`marque_rules::FixSource::DecoderClassificationHeuristic`]
    /// so the engine can downgrade to
    /// [`marque_rules::Severity::Warn`] and cap
    /// [`marque_rules::Recognition::rule`].
    pub(super) fix_source: marque_rules::FixSource,
}
