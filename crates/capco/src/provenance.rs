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

use marque_ism::MarkingType;
use marque_rules::recognition::FeatureId;
use marque_rules::{
    Diagnostic, FeatureContribution, FixIntent, FixSource, Message, MessageArgs, MessageTemplate,
    Recognition, RuleId, Severity, SmallVec,
};
use marque_scheme::{
    Citation, ReplacementIntent, SectionLetter, Span, capco, fix_intent::RecanonScope,
};

use crate::CapcoScheme;

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
///   [`Recognition::runner_up_ratio`](marque_rules::Recognition::runner_up_ratio)
///   verbatim.
/// - `features` — the per-feature `FeatureContribution` deltas the
///   decoder recorded while canonicalizing. Threaded into
///   [`Recognition::features`](marque_rules::Recognition::features)
///   verbatim.
/// - `fix_source` — provenance discriminator for the decoder fix
///   path. [`FixSource::DecoderPosterior`] for vocab-based corrections
///   (the default decoder pipeline);
///   [`FixSource::DecoderClassificationHeuristic`] for fixes produced
///   by the position-aware short-token classification heuristic
///   (issue #133). The engine reads this to choose `Severity::Fix`
///   vs `Severity::Warn` and to cap the sole surviving `recognition`
///   axis at `HEURISTIC_RECOGNITION_CAP = 0.95` (exactly the default
///   `confidence_threshold`) for the heuristic path — a single-
///   candidate heuristic fix lands at-threshold rather than
///   saturating above it.
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
    /// confidence in `[0.0, 1.0)` for `Recognition::recognition`.
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

/// Synthetic rule identifier attached to decoder-path
/// `FixSource::DecoderPosterior` diagnostics. This identifier lets the
/// recognition-layer rewrite carry a real `RuleId` (rules and fixes
/// share that requirement) without colliding with any CAPCO rule. The
/// `"engine"` scheme is the reserved namespace for engine-minted
/// diagnostics, and the predicate id describes the rewrite in plain
/// English. (The constant and its synthesis live here, in marque-capco,
/// because the diagnostic is built from `DecoderProvenance` — a CAPCO
/// type — and is `Diagnostic<CapcoScheme>`, neither nameable by the
/// generic engine. The recognition path calls [`build_decoder_diagnostic`]
/// directly today; the
/// [`ConstraintBridge::recognition_outcome`](marque_rules::ConstraintBridge::recognition_outcome)
/// hook is the seam through which a generic pipeline reaches the same
/// synthesis. The `"engine"` scheme string keeps the audit wire-form
/// unchanged.)
const DECODER_RULE_ID: RuleId = RuleId::new("engine", "recognition.decoder-recognized");

/// Citation attached to `R001 decoder-recognition` diagnostics. Points
/// at CAPCO-2016 §A.6 — the canonical-marking-form section the decoder
/// is enforcing. Per Constitution VIII the citation is verifiable: §A.6
/// is "(U) Formatting" beginning on page 15 (table of contents,
/// `crates/capco/docs/CAPCO-2016.md` line 49) and contains the
/// canonical syntax for portion / banner / CAB markings the decoder
/// canonicalizes input toward.
const DECODER_CITATION_TYPED: Citation = capco(SectionLetter::A, 6, 15);

/// Cap applied to `recognition` for the position-aware classification
/// heuristic (issue #133) — pinned at the default
/// `confidence_threshold` (0.95) so a solo heuristic candidate lands
/// at-threshold rather than saturating above it. Pre-PR-B this cap
/// lived on the (now-retired) `Recognition::rule` axis as
/// `HEURISTIC_RULE_AXIS_CAP`; PR B collapsed the two axes into one and
/// the cap moved onto `recognition` directly. The empirical corpus
/// measurement justifying the `0.95` value (≥99.4% confidence per
/// trigger) is unchanged.
pub const HEURISTIC_RECOGNITION_CAP: f32 = 0.95;

/// Build the synthetic `R001 decoder-recognition` diagnostic emitted
/// when a recognizer returned a marking carrying [`DecoderProvenance`].
/// Returns `None` when the original or canonical bytes are not valid
/// UTF-8 — `FixProposal` carries `Box<str>` for both `original` and
/// `replacement`, so we cannot construct the proposal without UTF-8
/// validity. CAPCO markings are ASCII by spec (CAPCO-2016 §A.6); a
/// non-UTF-8 result here would mean the canonicalization pass produced
/// something the strict parser shouldn't have accepted, which is a
/// separate bug to surface — silently dropping the synthetic diagnostic
/// is the conservative move. Also returns `None` for a no-op rewrite
/// (canonicalization preserved bytes byte-for-byte) — a degenerate
/// audit record that carries no information.
///
/// Lives in marque-capco (rather than the engine) because it reads
/// scheme-private [`DecoderProvenance`] and produces a
/// `Diagnostic<CapcoScheme>` — neither nameable by the generic engine.
/// The recognition path calls this function directly; the
/// [`ConstraintBridge::recognition_outcome`](marque_rules::ConstraintBridge::recognition_outcome)
/// hook is the seam through which a generic pipeline reaches the same
/// synthesis.
///
/// # Audit-shape contract (Constitution V Principle V)
///
/// The diagnostic's `message` MUST NOT carry verbatim input bytes —
/// only token canonicals, span offsets, and digests/posterior scalars
/// are permitted in audit output. The "before" form is omitted from
/// the message; the span tells the audit consumer *where* the fix
/// landed and the structural `FixIntent` carries *what* shape the
/// recognition became (a `Recanonicalize { scope: RecanonScope::Portion }`
/// emission for R001).
///
/// The audit record's `AppliedFix.proposal` carries no document bytes
/// for the decoder path: the `AppliedFixProposal::FixIntent(_)` variant
/// carries the structural intent only. The lint-side
/// `Diagnostic.recognized_canonical` field DOES carry the canonical
/// bytes (as a `SecretBox<[u8]>` that wipes on drop, Constitution II) so
/// user-facing renderers can show the recognized form in `check` output
/// without running `fix`; the asymmetry is intentional and pinned by
/// `lint_carries_recognized_canonical_fix_audit_does_not`.
///
/// The fix's [`Recognition`] is populated entirely from the decoder's
/// provenance trace: `recognition` derives from `runner_up_ratio` via
/// [`DecoderProvenance::recognition_score`] (strictly `< 1.0` so audit
/// consumers distinguish strict from decoder provenance by a single
/// field comparison; the position-aware classification heuristic caps it
/// at [`HEURISTIC_RECOGNITION_CAP`]); `runner_up_ratio` and `features`
/// thread through verbatim. When `corpus_override_active`, an extra
/// [`FeatureId::CorpusOverrideInEffect`] contribution with `delta = 0.0`
/// is appended as an audit-trail marker (the zero delta is load-bearing:
/// the override surface is wired end-to-end without yet substituting
/// override priors into decoder scoring).
pub fn build_decoder_diagnostic(
    span: Span,
    original_bytes: &[u8],
    provenance: &DecoderProvenance,
    _kind: MarkingType,
    corpus_override_active: bool,
) -> Option<Diagnostic<CapcoScheme>> {
    let original = std::str::from_utf8(original_bytes).ok()?;
    let replacement = std::str::from_utf8(&provenance.canonical_bytes).ok()?;

    // No-op rewrite (canonicalization preserved bytes byte-for-byte) is
    // not informative and would produce a degenerate audit record; skip.
    if original == replacement {
        return None;
    }

    // `provenance.features` is a `Box<[FeatureContribution]>`; copy into
    // a `SmallVec<[…; 4]>` matching `Recognition::features` so the inline-4
    // case stays heap-free even after the optional override-marker push.
    let mut features: SmallVec<[FeatureContribution; 4]> =
        SmallVec::from_slice(&provenance.features);
    if corpus_override_active {
        features.push(FeatureContribution {
            id: FeatureId::CorpusOverrideInEffect,
            delta: 0.0,
        });
    }

    // Dispatch on the decoder's `fix_source`. Standard vocab-based
    // recognition emits at `Severity::Fix` with the decoder's full
    // posterior on `recognition` (engine applies whenever
    // `recognition >= confidence_threshold`). The position-aware
    // classification heuristic (issue #133) emits at `Severity::Warn`
    // (always-visible in `--check`, non-zero exit code) with
    // `recognition` capped at [`HEURISTIC_RECOGNITION_CAP = 0.95`] —
    // matching the default `confidence_threshold` so a single-candidate
    // heuristic fix lands at-threshold rather than saturating above it.
    let raw_recognition = provenance.recognition_score();
    let (severity, recognition, fix_source) = match provenance.fix_source {
        FixSource::DecoderClassificationHeuristic => (
            Severity::Warn,
            raw_recognition.min(HEURISTIC_RECOGNITION_CAP),
            FixSource::DecoderClassificationHeuristic,
        ),
        // All non-heuristic decoder paths use the existing posterior
        // shape. Strict-source variants (BuiltinRule, CorrectionsMap,
        // MigrationTable) do not flow through this builder — they come
        // from rule-pipeline emissions, not the decoder — so routing
        // them to `DecoderPosterior` here is a defensive default that
        // preserves the existing strict-decoder shape for any future
        // fix-source variant.
        _ => (Severity::Fix, raw_recognition, FixSource::DecoderPosterior),
    };

    let confidence = Recognition {
        recognition,
        runner_up_ratio: provenance.runner_up_ratio,
        features,
    };
    let rule = DECODER_RULE_ID;
    // Audit-shape contract: the decoder-path engine-minted record
    // carries no document bytes (Constitution V Principle V). The
    // `original` / `replacement` bindings above served the UTF-8-validity
    // and no-op-rewrite gates only — both have already run. The canonical
    // bytes feeding `recognized_canonical` come directly from
    // `provenance.canonical_bytes`, wrapped in `SecretBox<[u8]>` — the
    // secret wipes on drop and every readout goes through
    // `expose_secret()` (Constitution II).
    let _ = (original, replacement);
    let recognized_canonical = Some(secrecy::SecretBox::new(Box::from(
        provenance.canonical_bytes.as_ref(),
    )));
    let intent = FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
            prior: None,
        },
        confidence,
        feature_ids: SmallVec::new(),
        message: Message::new(MessageTemplate::DecoderRecognized, MessageArgs::default()),
        source: fix_source,
        migration_ref: None,
    };
    Some(
        Diagnostic::with_fix_at_span(
            rule,
            severity,
            span,
            span,
            Message::new(
                MessageTemplate::DecoderRecognized,
                MessageArgs {
                    span: Some(span),
                    ..MessageArgs::default()
                },
            ),
            DECODER_CITATION_TYPED,
            intent,
        )
        .with_recognized_canonical(recognized_canonical),
    )
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
