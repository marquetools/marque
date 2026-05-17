// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Engine::fix` latency benchmarks. Three functions live here:
//!
//! - **`fix_single_e054_apply`** — `engine.fix((S//NF/RELIDO), FixMode::Apply)`
//!   on a single-portion input that produces exactly one E054 fix at
//!   confidence 0.95. End-to-end per-fix latency: scanner + parser + rule
//!   evaluation + intent synthesis + promotion + apply + audit. Criterion
//!   amortizes over thousands of iterations, so the reported time IS the
//!   per-fix cost for an interactive caller fixing one marking at a time.
//! - **`fix_single_e054_dry_run`** — same input, `FixMode::DryRun`. Drops
//!   the source-rewrite cost; isolates the cost of generating the audit
//!   record without applying the rewrite to a fresh `Vec<u8>`.
//! - **`lint_single_e054_baseline`** — `engine.lint` on the same input.
//!   The delta `fix_single_e054_apply - lint_single_e054_baseline` is the
//!   marginal cost of intent synthesis + promotion + apply + audit on top
//!   of detection.
//!
//! **Rule under test**: E054 — RELIDO conflicts with NOFORN (§H.8 p154).
//! The subtractive `FactRemove(RELIDO, Scope::Portion)` intent is the
//! canonical example of the intent-synthesis fix path: the engine parses
//! the candidate span, applies the intent via `CapcoScheme::apply_intent`,
//! and re-renders the result via `render_portion`. The rewrite removes
//! RELIDO from the portion, leaving `(S//NF)`.
//!
//! **Previous bench**: before PR 3c.B Commit 6 (form-bucket migration)
//! retired E001, this bench used `SECRET//NF\n` with the E001
//! `NF → NOFORN` expansion as the signal. Post-retirement the renderer
//! absorbs that expansion by construction and no `AppliedFix` is produced,
//! so the old invariant always panicked. E054 is the direct replacement:
//! it still fires a deterministic confidence-0.95 fix on the strict path.
//!
//! Marketing-facing number is `fix_single_e054_apply`: total wall-clock
//! time to detect, promote, apply, and audit one fix on a one-portion
//! input. The lint baseline contextualizes how much of that is detection
//! vs the fix-specific work.
//!
//! Not gated by `scripts/bench-check.sh` — there is no SC-target for
//! per-fix latency yet. Wired in advisory mode so the numbers print
//! alongside the gated benches.

use std::hint::black_box;
use std::sync::Arc;

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{Engine, FixMode, StrictRecognizer};

/// Single-portion input that produces exactly one E054 fix at confidence
/// 0.95. RELIDO conflicts with NOFORN (NF) per CAPCO-2016 §H.8 p154;
/// the fix removes RELIDO and re-renders the portion as `(S//NF)`.
const SINGLE_FIX_INPUT: &[u8] = b"(S//NF/RELIDO)\n";

/// Expected source bytes after applying the single E054 fix. The portion
/// is re-rendered with RELIDO removed; NF is the canonical portion-scope
/// abbreviation for NOFORN (§A.6 p16 / CAPCO-2016 Table 4 row 8 p36).
const EXPECTED_FIXED_SOURCE: &[u8] = b"(S//NF)\n";

/// Expected combined confidence for the E054 fix. E054 uses
/// `Confidence::strict(0.95)` (recognition=1.0, rule=0.95); combined
/// equals 0.95, which is exactly the default threshold — the gate is
/// `>= threshold`, so the fix auto-applies.
const EXPECTED_CONFIDENCE: f32 = 0.95;

fn build_engine() -> Engine {
    Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_recognizer(Arc::new(StrictRecognizer::new()))
}

/// Asserts the bench invariant: `SINGLE_FIX_INPUT` must produce exactly one
/// E054 fix at combined confidence 0.95, and the rewritten source must equal
/// `EXPECTED_FIXED_SOURCE`. If rule behavior changes (E054 is retired,
/// confidence drops below threshold, a new rule fires on this input, etc.)
/// this panics with a descriptive message so the breakage is visible
/// immediately rather than silently measuring a different code path.
///
/// Call once per benchmark function, **outside** the Criterion `b.iter`
/// loop, before handing control to Criterion.
fn assert_bench_invariants(engine: &Engine) {
    let fix_result = engine.fix(SINGLE_FIX_INPUT, FixMode::Apply);

    // Exactly one fix applied total — extra fixes would change what the bench
    // is measuring (multiple rewrites, different pipeline branch).
    assert_eq!(
        fix_result.applied.len(),
        1,
        "fix_latency invariant: expected exactly 1 applied fix on input {:?}; \
         got {}. Applied rules: {:?}",
        std::str::from_utf8(SINGLE_FIX_INPUT).unwrap_or("<non-utf8>"),
        fix_result.applied.len(),
        fix_result
            .applied
            .iter()
            .map(|f| f.rule.as_str())
            .collect::<Vec<_>>(),
    );

    let e054_fix = &fix_result.applied[0];
    assert_eq!(
        e054_fix.rule.as_str(),
        "E054",
        "fix_latency invariant: expected the sole applied fix to be E054, got {:?}",
        e054_fix.rule.as_str(),
    );

    // Combined confidence must be exactly 0.95 (Confidence::strict(0.95):
    // recognition=1.0 × rule=0.95). A deviation here means the bench is
    // measuring a different code path than the deterministic strict-path
    // FactRemove this benchmark documents.
    let combined = e054_fix.confidence.combined();
    assert!(
        (combined - EXPECTED_CONFIDENCE).abs() < 1e-6_f32,
        "fix_latency invariant: expected E054 fix confidence {EXPECTED_CONFIDENCE}, got {combined}",
    );

    assert_eq!(
        fix_result.source.as_slice(),
        EXPECTED_FIXED_SOURCE,
        "fix_latency invariant: rewritten source mismatch. \
         expected {:?}, got {:?}",
        std::str::from_utf8(EXPECTED_FIXED_SOURCE).unwrap_or("<non-utf8>"),
        std::str::from_utf8(&fix_result.source).unwrap_or("<non-utf8>"),
    );

    let lint_result = engine.lint(SINGLE_FIX_INPUT);
    let has_e054_diag = lint_result
        .diagnostics
        .iter()
        .any(|d| d.rule.as_str() == "E054");
    assert!(
        has_e054_diag,
        "fix_latency invariant: E054 diagnostic not found in lint output for input {:?}. \
         Diagnostics: {:?}",
        std::str::from_utf8(SINGLE_FIX_INPUT).unwrap_or("<non-utf8>"),
        lint_result
            .diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>(),
    );
}

fn fix_apply_benchmark(c: &mut Criterion) {
    let engine = build_engine();
    assert_bench_invariants(&engine);
    c.bench_function("fix_single_e054_apply", |b| {
        b.iter(|| engine.fix(black_box(SINGLE_FIX_INPUT), FixMode::Apply));
    });
}

fn fix_dry_run_benchmark(c: &mut Criterion) {
    let engine = build_engine();
    assert_bench_invariants(&engine);
    c.bench_function("fix_single_e054_dry_run", |b| {
        b.iter(|| engine.fix(black_box(SINGLE_FIX_INPUT), FixMode::DryRun));
    });
}

fn lint_baseline_benchmark(c: &mut Criterion) {
    let engine = build_engine();
    assert_bench_invariants(&engine);
    c.bench_function("lint_single_e054_baseline", |b| {
        b.iter(|| engine.lint(black_box(SINGLE_FIX_INPUT)));
    });
}

criterion_group!(
    benches,
    fix_apply_benchmark,
    fix_dry_run_benchmark,
    lint_baseline_benchmark
);
criterion_main!(benches);
