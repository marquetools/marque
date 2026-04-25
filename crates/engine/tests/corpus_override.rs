// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T069 — engine emits `CorpusOverrideInEffect` feature contribution
//! when a corpus override is installed via
//! [`Engine::with_corpus_override`] (Phase 4 PR-5, FR-013).
//!
//! Scope: PR-5 minimal. The contribution is an audit-trail marker
//! (`delta = 0.0`) — the engine does **not** yet substitute override
//! priors into decoder scoring. Wiring substitution is a follow-up.
//!
//! Why behind `corpus-override`: the test exercises the gated builder
//! `Engine::with_corpus_override(...)`, which only exists when the
//! Cargo feature is on. The whole file is `#![cfg(...)]`-guarded so
//! the default `cargo test --workspace` build (no features) doesn't
//! drag the test in.

#![cfg(feature = "corpus-override")]

use std::sync::Arc;

use marque_capco::capco_rules;
use marque_config::Config;
use marque_config::corpus_override::CorpusOverride;
use marque_engine::{Engine, FixMode};
use marque_rules::{FeatureId, FixSource};

fn deep_scan_engine_without_override() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_deep_scan()
}

fn deep_scan_engine_with_override() -> Engine {
    deep_scan_engine_without_override()
        // Audit-marker-only override: every scoring section empty,
        // schema_version implicit. The flag is what matters for T069.
        .with_corpus_override(Arc::new(CorpusOverride::default()))
}

/// `(SERCET//NF)` — mangled portion the decoder canonicalizes to
/// `(SECRET//NF)`. Lifted from `decoder_path_record_shape` in
/// `tests/audit.rs`; if the decoder ever stops recovering this input
/// the corpus-override audit assertion is vacuous, so we track the
/// same fixture deliberately.
const MANGLED_PORTION: &[u8] = b"(SERCET//NF)";

/// With corpus override active, every decoder-path fix MUST carry
/// exactly one `CorpusOverrideInEffect` feature contribution
/// appended to its `Confidence.features` list.
#[test]
fn decoder_fix_carries_corpus_override_feature_when_active() {
    let engine = deep_scan_engine_with_override();
    assert!(
        engine.corpus_override_active(),
        "with_corpus_override must flip corpus_override_active() to true"
    );

    let result = engine.fix(MANGLED_PORTION, FixMode::DryRun);

    let mut decoder_fixes_examined = 0usize;
    for fix in &result.applied {
        if fix.source != FixSource::DecoderPosterior {
            continue;
        }
        decoder_fixes_examined += 1;

        let override_features: Vec<_> = fix
            .confidence
            .features
            .iter()
            .filter(|f| f.id == FeatureId::CorpusOverrideInEffect)
            .collect();
        assert_eq!(
            override_features.len(),
            1,
            "expected exactly one CorpusOverrideInEffect contribution on \
             decoder-path fix at {}..{}, got {} (full features: {:?})",
            fix.proposal.span.start,
            fix.proposal.span.end,
            override_features.len(),
            fix.confidence.features,
        );
        assert_eq!(
            override_features[0].delta, 0.0,
            "PR-5 minimal scope: CorpusOverrideInEffect must carry \
             delta=0.0 (audit-marker only — override priors do not \
             yet shift scoring). delta {} indicates the prior-\
             substitution wiring landed without an audit-schema bump.",
            override_features[0].delta,
        );
    }

    // Vacuity guard: a pass with zero decoder fixes would silently
    // weaken the assertion (every loop iteration above is skipped).
    // Mirror the guard in `decoder_path_record_shape`.
    assert!(
        decoder_fixes_examined >= 1,
        "expected ≥1 decoder fix on the mangled-portion fixture; \
         got 0. The dispatcher likely never invoked the decoder."
    );
}

/// Without corpus override, no decoder-path fix may carry
/// `CorpusOverrideInEffect`. This is the negative half of T069 —
/// the audit-marker is silent unless override is actually in
/// effect, so an auditor reading the audit stream can trust
/// "this fix has no override marker" as positive evidence the
/// fix came out of stock priors.
#[test]
fn decoder_fix_omits_corpus_override_feature_without_override() {
    let engine = deep_scan_engine_without_override();
    assert!(
        !engine.corpus_override_active(),
        "default deep-scan engine must report corpus_override_active() = false"
    );

    let result = engine.fix(MANGLED_PORTION, FixMode::DryRun);

    let mut decoder_fixes_examined = 0usize;
    for fix in &result.applied {
        if fix.source != FixSource::DecoderPosterior {
            continue;
        }
        decoder_fixes_examined += 1;

        let has_override_marker = fix
            .confidence
            .features
            .iter()
            .any(|f| f.id == FeatureId::CorpusOverrideInEffect);
        assert!(
            !has_override_marker,
            "decoder-path fix at {}..{} carries CorpusOverrideInEffect \
             without an override installed — audit stream would be \
             misleading. Full features: {:?}",
            fix.proposal.span.start, fix.proposal.span.end, fix.confidence.features,
        );
    }

    assert!(
        decoder_fixes_examined >= 1,
        "expected ≥1 decoder fix on the mangled-portion fixture; \
         got 0. The dispatcher likely never invoked the decoder."
    );
}
