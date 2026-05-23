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

// Public surface — byte-identical paths preserved (frozen API).
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
pub(in crate::decoder) const K_MAX_CANDIDATES: usize = 8;

/// Runner-up posterior-margin threshold gating `Unambiguous` vs.
/// `Ambiguous` emission: top wins outright only when its posterior
/// beats the runner-up by at least this log-odds delta.
///
/// `1.6` ≈ 5× odds ratio (`e^1.6 ≈ 4.95`) — large enough that the top
/// candidate is meaningfully more likely than the runner-up, small
/// enough that close calls surface as diagnostics rather than
/// auto-fixes.
///
/// See `docs/refactor-006/decoder-architecture.md` § "Scoring approach"
/// for the corpus-derived derivation.
pub(in crate::decoder) const UNAMBIGUOUS_LOG_MARGIN: f32 = 1.6;

/// Per-candidate prose-null gate: a `MarkingType::Portion` candidate's
/// marking-side posterior must beat its observed prose-side posterior
/// by at least this log-margin before the decoder dispatches it.
///
/// `2.5` ≈ 12× odds ratio (`e^2.5 ≈ 12.2×`) — sized to suppress short
/// single-letter prose false-positives (Federalist `(s)`, `(c)`) while
/// leaving canonical bare classification tokens and `//`-bearing
/// shapes (banner/CAB and bare-classification whitelist) on the
/// bypass path.
///
/// See `docs/refactor-006/decoder-architecture.md` §
/// "Null-hypothesis dispatch" for the corpus-derived derivation,
/// per-token measurements, and the full bypass / whitelist enumeration.
pub(in crate::decoder) const NULL_HYPOTHESIS_LOG_MARGIN: f32 = 2.5;

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
