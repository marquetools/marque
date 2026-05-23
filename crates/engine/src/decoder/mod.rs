// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase-D probabilistic [`Recognizer`] — the "decoder".
//!
//! The deep-scan half of the strict/deep-scan recognizer split. When
//! the engine is configured for deep-scan mode and the strict
//! recognizer returns zero candidates for a marking region, the
//! engine falls back to the decoder to recover mangled markings that
//! are one of a small set of canonical-shape deviations away from a
//! real CAPCO-2016 marking:
//!
//! - Edit-distance-1/2 token typos (`SERCET` → `SECRET`).
//! - Token reordering within categories (`NOFORN//SECRET` →
//!   `SECRET//NOFORN`).
//! - CAPCO-2016-superseded tokens (`COMINT` → `SI`).
//! - Case mistakes (`secret//noforn` → `SECRET//NOFORN`).
//! - Garbled delimiters (`S ∕∕ NOFORN` → `S//NOFORN`).
//!
//! The decoder never fabricates a marking where none exists. When the
//! observed tokens fit no CAPCO grammar template, it returns
//! `Parsed::Ambiguous { candidates: vec![] }` — the zero-candidate
//! signal per the foundational-plan dispatch contract.
//!
//! ## What this module is NOT
//!
//! - **Not a full template-matching grammar engine.** Candidates are
//!   materialized by canonicalizing observed tokens and round-tripping
//!   through the strict parser; the strict parser is the arbiter of
//!   "is this a CAPCO-shape marking?".
//! - **Not a learning system.** All priors are compile-time-baked
//!   `&'static` tables from `marque_capco::priors`.
//! - **Not a fix applier.** The decoder proposes `CapcoMarking`
//!   candidates; the engine applies them through the normal
//!   `Diagnostic` / `FixProposal` path with
//!   `FixSource::DecoderPosterior`.
//!
//! ## Sub-module layout
//!
//! Per-item-cluster split (engine-internal refactor #562). See
//! `docs/refactor-006/decoder-architecture.md` for the
//! design rationale, scoring approach, crate-placement reasoning,
//! and retired-mechanism notes (`LENIENT_REL_PREFIX_PENALTY`).

mod candidates;
mod dispatcher;
mod heuristic;
mod normalize;
mod null_hypothesis;
mod recognizer;
mod recovery;
mod scoring;
mod shape;
mod types;

#[cfg(test)]
mod test_helpers;

// Public surface — byte-identical paths preserved per FR-049.
pub use dispatcher::StrictOrDecoderRecognizer;
pub use recognizer::DecoderRecognizer;
pub use shape::is_nontrivial_marking;

#[cfg(feature = "decoder-harness")]
pub use candidates::diagnostic_canonical_attempts;

// Cross-file constants — shared by multiple sub-modules.
/// K=8 candidate bound per foundational-plan §5.2 and research.md R3.
///
/// Higher K burns latency without accuracy gain (diminishing returns
/// above 6 per the primary-source corpus analysis); lower K drops
/// recall on multi-token reorderings. Tunable in-place — the bound is
/// advisory, not a correctness invariant.
pub(super) const K_MAX_CANDIDATES: usize = 8;

/// Runner-up posterior-ratio threshold for emitting `Unambiguous`.
///
/// The decoder computes `log_margin = top_posterior - runner_up_posterior`
/// in natural-log space. When `log_margin >= UNAMBIGUOUS_LOG_MARGIN`,
/// the decoder collapses to `Unambiguous(top)`; below the threshold it
/// returns `Ambiguous { candidates }` so the engine can surface a
/// diagnostic rather than auto-apply a close call.
///
/// `1.6` corresponds to a posterior odds ratio of `e^1.6 ≈ 4.95` —
/// i.e., the top candidate is roughly five times as likely as the
/// runner-up given the observed bytes. This is the **odds** ratio
/// (`P(top)/P(runner_up)`), not a probability ratio.
pub(super) const UNAMBIGUOUS_LOG_MARGIN: f32 = 1.6;

/// Minimum log-margin a candidate's marking-side posterior must hold
/// over its prose null posterior to clear the per-candidate prose
/// filter (issue #258, expanded after the documents-corpus marking
/// stratum landed in PR1).
///
/// Originally the filter required only `posterior >= null_posterior`
/// (a zero-margin gate). That was sufficient when the marking
/// stratum was just `tests/corpus/valid/` (~34 short fixtures); the
/// per-token marking-vs-prose delta `log P(token|marking) − log
/// P(token|prose)` for short tokens like `S` and `U` stayed near
/// zero and the Federalist `(s)` mid-prose case suppressed cleanly.
///
/// After PR1 added `tests/corpus/documents/marked/` (40 multi-page
/// synthetic-positive documents with hundreds of `(S//*)` portion
/// marks) the marking-side prior for `S` strengthened from
/// `log_prior ≈ -3.97` to `-3.28`, while the prose-side prior
/// (Enron + Congressional Record + GAO + CIA CREST) sat at `-5.11` —
/// a `+1.83` delta. The zero-margin filter let the marking
/// hypothesis win for isolated `(s)` candidates and re-introduced
/// the SC-003a regression on `Notwithstanding (s) the early`.
///
/// `2.5` (e^2.5 ≈ 12.2×) is the smallest margin that suppresses the
/// Federalist `(s)` regression at its actual marking-vs-null delta of
/// `+2.21` (`S`: marking `-3.28`, prose `-5.49`). `(c)` at `+1.08`
/// and most other single-letter portions are rejected at the same
/// threshold by construction. `(u)` at `+2.86` survives this margin
/// — a lowercase `(u)` mid-prose canonicalizing to UNCLASSIFIED is
/// the residual false-positive surface; it has not been observed in
/// the test corpus, and the prose-glue heuristic
/// (`preceded_by_whitespace = false`) suppresses the much more
/// common `letter(s)` / `function(c)` cases independently.
///
/// **Where the gate applies.** The recognizer applies this margin to
/// every `MarkingType::Portion` input EXCEPT:
///
/// - portions containing `//` (`has_double_slash`): the category
///   separator is a marking-grammar signal English prose does not
///   produce, so the marking interpretation is the only plausible
///   reading.
/// - portions whose inner content is exactly a canonical
///   classification token (`is_bare_classification_shape`): the
///   whitelist is `(U)`, `(C)`, `(S)`, `(TS)`, `(R)` plus the NATO
///   abbreviations `(NU)`, `(NR)`, `(NC)`, `(NS)`, `(CTS)`. Pinning
///   a positive margin on these would reject legitimate NATO/IC
///   abbreviation recovery (NU at marking `-8.43`, prose `-8.34`,
///   delta `-0.09`; NC at marking `-8.43`, prose `-5.89`, delta
///   `-2.54`) where the marking stratum has zero examples but the
///   strict grammar still recognizes the token.
///
/// Banner and CAB shapes bypass the filter entirely — their forms
/// are long enough that English prose doesn't fabricate them by
/// glyph coincidence. Multi-letter Portion candidates outside the
/// classification whitelist (e.g., `(SI)`, `(HCS)`) ARE subject to
/// the margin; the structural discrimination the strict parser
/// provides is not by itself sufficient when prose can produce the
/// same token by glyph coincidence.
pub(super) const NULL_HYPOTHESIS_LOG_MARGIN: f32 = 2.5;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Per-sub-file test split landed in a follow-up commit; each sub-file
// hosts a `#[cfg(test)] mod tests { use super::*; }` block exercising
// its own private surface. The block below retains only cross-cutting
// assertions (Send + Sync trait-object guarantees) that aren't owned
// by any single sub-module.

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use marque_capco::CapcoScheme;
    use marque_scheme::recognizer::Recognizer;

    use super::*;

    #[test]
    fn decoder_is_send_sync_as_trait_object() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<DecoderRecognizer>();
        assert_send_sync::<StrictOrDecoderRecognizer>();
        assert_send_sync::<std::sync::Arc<dyn Recognizer<CapcoScheme>>>();
    }
}
