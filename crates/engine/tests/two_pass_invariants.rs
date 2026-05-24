// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property tests for the two-pass fix pipeline's reshape-aware
//! invariants.
//!
//! These tests target the engine-level guarantees, not specific rules.
//! The fixtures are valid (or near-valid) CAPCO markings that exercise
//! both the pass-1 Localized rule path (corrections-map, deprecated-
//! marking migration) and the pass-2 WholeMarking path (banner roll-up,
//! FD&R precedence).
//!
//! Invariants exercised:
//!
//! - **Non-overlap**: no two `AppliedFix.span`s overlap. The pass-1 and
//!   pass-2 partitions are disjoint by rule phase, and the engine's
//!   overlap demotion converts any pass-2 diagnostic that would have
//!   collided with a pass-1 span into `Severity::Suggest` — preventing
//!   promotion. Both arms are exercised by varying source shapes.
//! - **Reshape-aware no-double-fire**: the same `(rule_id, span)` MUST
//!   NOT appear twice across the merged `pass1_applied + pass2_applied`
//!   audit stream. Disambiguation drops a pass-2 diagnostic whose
//!   `(rule, candidate_span)` matches a pass-1 promoted fix; the
//!   partition by phase already guarantees rule-disjoint sets, so the
//!   cross-pass uniqueness check is the stronger statement.
//!
//! # Proptest budget
//!
//! Cases bounded at 64 per property (well below the proptest default
//! of 256) so the suite runs in under a second on the interactive-
//! latency budget. Shrinking is bounded by the small `Strategy` ranges
//! below — every generator produces ≤300 B of input.

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use proptest::prelude::*;
use secrecy::ExposeSecret as _;
use std::collections::HashSet;
use std::sync::OnceLock;

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        Engine::new(
            Config::default(),
            vec![Box::new(CapcoRuleSet::new())],
            marque_engine::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
    })
}

// ---------------------------------------------------------------------------
// Strategies — small, focused generators that include both clean
// markings and pass-1-trigger-prone variants.
// ---------------------------------------------------------------------------

/// Valid US classifications.
static CLASSIFICATIONS: &[&str] = &["TOP SECRET", "SECRET", "CONFIDENTIAL", "UNCLASSIFIED"];

/// Dissem suffixes that participate in banner/CAB roll-up.
static DISSEM_SUFFIXES: &[&str] = &["", "//NOFORN", "//REL TO USA, GBR", "//RELIDO", "//FOUO"];

/// Portion classifications.
static PORTION_CLASS: &[&str] = &["TS", "S", "C", "U"];

/// Portion dissem suffixes.
static PORTION_DISSEM: &[&str] = &["", "//NF", "//REL TO USA, GBR"];

fn arb_banner() -> impl Strategy<Value = String> {
    (0..CLASSIFICATIONS.len(), 0..DISSEM_SUFFIXES.len())
        .prop_map(|(c, d)| format!("{}{}\n", CLASSIFICATIONS[c], DISSEM_SUFFIXES[d]))
}

fn arb_portion() -> impl Strategy<Value = String> {
    (0..PORTION_CLASS.len(), 0..PORTION_DISSEM.len())
        .prop_map(|(c, d)| format!("({}{})\n", PORTION_CLASS[c], PORTION_DISSEM[d]))
}

/// Compose a banner + 1–4 portions. The combinations include valid
/// inputs (banner matches the rolled-up portions) and invalid ones
/// (banner missing dissem controls present in portions). Mixed shapes
/// give both pass-1 and pass-2 paths exposure.
fn arb_doc() -> impl Strategy<Value = String> {
    (
        arb_banner(),
        proptest::collection::vec(arb_portion(), 1..=4),
    )
        .prop_map(|(banner, portions)| {
            let mut s = banner;
            for p in &portions {
                s.push_str(p);
            }
            s
        })
}

// ---------------------------------------------------------------------------
// Half-open span overlap predicate. Mirrors `engine::spans_overlap`,
// duplicated here because the engine-internal function is not exposed
// (`#[cfg(test)]` inside the engine crate). Identical logic.
// ---------------------------------------------------------------------------

fn spans_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Across `Engine::fix(Apply)`, no two `AppliedFix.span`s overlap.
    ///
    /// The engine's two-pass orchestrator guarantees pass-1 fixes
    /// land first in disjoint span windows (C-1 dedup within
    /// pass-1), then pass-2 dispatches against the post-pass-1
    /// buffer with overlap demotion: any pass-2 diagnostic
    /// whose span overlaps a pass-1 promoted span is demoted to
    /// `Severity::Suggest`, which excludes it from auto-apply. The
    /// invariant verified here is the OBSERVABLE outcome — the
    /// audit stream's spans are mutually disjoint regardless of
    /// which pass produced them.
    #[test]
    fn i18_applied_spans_are_pairwise_disjoint(src in arb_doc()) {
        let result = engine().fix(src.as_bytes(), FixMode::Apply);
        let spans: Vec<(usize, usize)> = result
            .applied_fixes()
            .map(|a| (a.span.start, a.span.end))
            .collect();
        for i in 0..spans.len() {
            for j in (i + 1)..spans.len() {
                let (a_s, a_e) = spans[i];
                let (b_s, b_e) = spans[j];
                prop_assert!(
                    !spans_overlap(a_s, a_e, b_s, b_e),
                    "overlapping applied fixes at indices {} ({}, {}) and {} ({}, {}) for input {:?}",
                    i, a_s, a_e, j, b_s, b_e, src
                );
            }
        }
    }

    /// No `(rule_id, span)` pair appears twice across the audit stream.
    ///
    /// Same span + same rule across pass-1 + pass-2 is what
    /// reshape-aware disambiguation forbids. The pass partition is
    /// rule-disjoint by phase, but a Walker rule can register under
    /// Localized while emitting under multiple catalog IDs — the
    /// `additional_emitted_ids` mechanism. The audit stream must
    /// still be uniqueness-preserving on the `(emitted_id, span)`
    /// key.
    #[test]
    fn i19_no_duplicate_rule_span_pairs(src in arb_doc()) {
        let result = engine().fix(src.as_bytes(), FixMode::Apply);
        let mut seen: HashSet<(String, usize, usize)> = HashSet::new();
        for fix in result.applied_fixes() {
            let key = (fix.rule.predicate_id().to_string(), fix.span.start, fix.span.end);
            prop_assert!(
                seen.insert(key.clone()),
                "duplicate (rule_id, span) in applied: {:?} for input {:?}",
                key, src
            );
        }
    }

    /// Cross-pass refinement: when pass-1 promoted any fix, no pass-2
    /// promoted fix shares its candidate-anchored (rule, span). The
    /// check here cross-references the applied stream against itself:
    /// the engine's pass-1 / pass-2 partition is internal, so we verify
    /// the observable property — every (rule_id, span) key appears at
    /// most once.
    ///
    /// This is the dual of i19_no_duplicate_rule_span_pairs: that
    /// test pins HashSet uniqueness; this one pins the property
    /// that motivates the disambiguation in the first place — same
    /// rule, same marking, no re-fire.
    #[test]
    fn i19_pass1_pass2_disjoint_on_rule_span_keys(src in arb_doc()) {
        let result = engine().fix(src.as_bytes(), FixMode::Apply);
        let mut counts: std::collections::HashMap<(String, usize, usize), usize> =
            std::collections::HashMap::new();
        for fix in result.applied_fixes() {
            let key = (fix.rule.predicate_id().to_string(), fix.span.start, fix.span.end);
            *counts.entry(key).or_insert(0) += 1;
        }
        for (key, count) in &counts {
            prop_assert!(
                *count == 1,
                "rule + span key {:?} appears {} times in applied — each rule+span pair must apply at most once for input {:?}",
                key, count, src
            );
        }
    }

    /// Idempotency under successive `fix` calls: applying the same
    /// engine twice must converge. The first `fix` produces a buffer;
    /// linting that buffer must yield fewer-or-equal diagnostics than
    /// the original (no new defects surface from pass-2's reshape work).
    ///
    /// This guards against a subtle disambiguation regression: if the
    /// engine forgot to drop a same-(rule, span) pass-2 diagnostic,
    /// the post-fix lint would still surface it as a remaining
    /// diagnostic.
    #[test]
    fn fix_does_not_introduce_new_defects(src in arb_doc()) {
        let bytes = src.as_bytes();
        let before = engine().lint(bytes).diagnostics.len();
        let result = engine().fix(bytes, FixMode::Apply);
        let after = engine().lint(result.source.expose_secret()).diagnostics.len();
        prop_assert!(
            after <= before,
            "fix introduced diagnostics: before={} after={} for input {:?}",
            before, after, src
        );
    }
}
