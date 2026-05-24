// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase-attribution probe for `CapcoScheme::project`.
//!
//! Measures isolated calls to (a) `join_via_lattice`, (b) `closure`,
//! (c) the trait-path `scheme.project`, (d) `from_canonical`,
//! (e) the engine fast-path `project_from_attrs_slice + from_canonical`,
//! (f) the whole `Engine::lint(10KB input)`, (g) project scaling at
//! several portion counts (1, 5, 10, 25, 50), (h) the per-page
//! accumulator rebuild in isolation, and (i) `join_via_lattice`
//! scaling.
//!
//! Attributes per-stage projection cost. Ships in-tree so future
//! perf work has a baseline to compare against.
//!
//! ## Post-close stage cost (#704, #714)
//!
//! `scheme.closure()` runs only the 6-row `CLOSURE_TABLE`
//! Kleene fixpoint. The full `scheme.project()` pipeline (see
//! `crates/capco/src/scheme/marking_scheme_impl.rs::project_attrs_pipeline`)
//! runs FIVE stages in order:
//!
//! 1. `join_via_lattice` (per-axis composition over the input slice)
//! 2. `closure` (Kleene fixpoint over the 6-row catalog)
//! 3. `apply_default_fill` (Rows 0/7/8/9 — non-monotone "default if
//!    absent" rules; `pub(crate)`, not on the bench-public surface)
//! 4. `apply_supersession_overlays` (FD&R supersession, OC > OC-USGOV,
//!    NOFORN-dominates contradictions; private associated fn)
//! 5. `page_rewrites` (declarative catalog application)
//!
//! Neither `apply_default_fill` nor `apply_supersession_overlays` is
//! reachable from outside `marque_capco::scheme`. Exposing either as
//! a bench-only entry point would violate the bench-only scope of
//! #714 (Constitution VII §IV — no production-surface changes).
//!
//! The per-phase benches below give two useful stage-decomposition
//! deltas on the representative `(S//NF) + (TS//SI)` synthesis pair:
//!
//! ```text
//! cost(closure-only) ≈ phase_b_closure
//!     // join is hoisted out of the iter loop; this measures
//!     // the Kleene fixpoint alone on a pre-joined marking.
//!
//! cost(bridge + join + closure) ≈ phase_b_prime_closure_on_unjoined
//!     // the `&[CapcoMarking]` → `Vec<CanonicalAttrs>` clone bridge +
//!     // join + closure, measured together on the input shape that
//!     // `scheme.project()` takes.
//!
//! cost(default_fill + supersession + page_rewrites)
//!     ≈ phase_c_scheme_project − phase_b_prime_closure_on_unjoined
//!     // phase_c and phase_b_prime share the same bridge+join-included
//!     // prefix, so both cancel; the delta isolates exactly the three
//!     // post-close stages.
//!
//! cost(bridge + join + default_fill + supersession + page_rewrites)
//!     ≈ phase_c_scheme_project − phase_b_closure
//!     // phase_b_closure is closure-only (both the bridge and the join
//!     // are hoisted out of the iter loop), so the delta picks up the
//!     // bridge + join cost on top of the three post-close stages. Use
//!     // this when triaging bridge/join overhead; use the phase_b_prime
//!     // delta above when triaging the post-close stages alone.
//! ```
//!
//! Isolating `apply_default_fill` ALONE (i.e. excluding supersession
//! and page_rewrites) is not possible without making it public; the
//! four-stage attribution above is the best honest decomposition the
//! current public surface supports. Future regression triage on the
//! default-fill stage should look at these deltas first; if a finer
//! split becomes necessary, the path is a separate `pub(crate)` bench
//! crate inside `marque_capco` rather than widening the public API.
//!
//! ## Full-lint bench vs isolated micro-benches — synthesis caveat
//!
//! The `phase_f_engine_lint_full` bench runs the full `Engine::lint`
//! on the same 10KB input `lint_latency.rs` uses; it's an
//! authoritative measurement of the end-to-end cost.
//!
//! Every other bench here measures isolated calls against
//! **synthesized** `CanonicalAttrs` portions (`collect_portions`
//! returns a hand-built `(S//NF)` + `(TS//SI)` pair rather than
//! lifting the parser's actual output). The synthesis is a
//! phase-attribution probe, not a regression gate: it lets us
//! attribute "what is the cost of one `join_via_lattice` call?"
//! independent of the parser's contribution to lint latency.
//!
//! ### Maintenance contract
//!
//! If the bench corpus's representative axis mix drifts away from
//! the `(S//NF)` + `(TS//SI)` pair the synthesizer mimics — e.g. the
//! lint_10kb input gains heavy SCI / FGI / AEA portions — the
//! per-phase numbers here may silently understate the production
//! cost on those axes. When refactoring per-axis lattice code,
//! regenerate the synthesis input from the actual `lint_10kb`
//! parse trace or extend it to cover the new axes.
//!
//! This is a **maintenance item, not a bug**. The synthesis is
//! correct for the bench input it mirrors today; the contract is
//! "keep it in sync if the bench input shape changes."

use criterion::{Criterion, criterion_group, criterion_main};
use marque_capco::CapcoMarking;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::Engine;
use marque_ism::CanonicalAttrs;
use marque_scheme::{MarkingScheme, Scope};
use std::hint::black_box;

fn build_input(target_bytes: usize) -> Vec<u8> {
    let block = concat!(
        "TOP SECRET//SCI//NOFORN\n",
        "\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n",
        "eiusmod tempor incididunt ut labore et dolore magna aliqua.\n",
        "\n",
        "(S//NF) This portion contains abbreviated dissemination controls.\n",
        "\n",
        "SECRET//NOFORN//REL TO USA, GBR\n",
        "\n",
        "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.\n",
        "\n",
        "(TS//SI) Another portion with SCI controls and valid formatting.\n",
        "\n",
    );
    let block_bytes = block.as_bytes();
    let mut input = Vec::with_capacity(target_bytes + block_bytes.len());
    while input.len() < target_bytes {
        input.extend_from_slice(block_bytes);
    }
    let complete_blocks = target_bytes / block_bytes.len();
    input.truncate(complete_blocks.max(1) * block_bytes.len());
    input.resize(target_bytes, b' ');
    input
}

/// Build the representative `CanonicalAttrs` slice used by the
/// per-phase micro-benches. PR 4b-D.2 Copilot R1 #10 corrected this
/// doc — the prior "extract portions from the strict pipeline" was
/// misleading: `Engine::lint` doesn't expose per-portion `CanonicalAttrs`,
/// and the body below synthesizes a representative `(S//NF)` +
/// `(TS//SI)` pair AFTER an Engine::lint warmup call.
///
/// The synthesis is intentional — see the file-level "Full-lint
/// bench vs isolated micro-benches — synthesis caveat" doc. The
/// Engine::lint call kept
/// here is a warmup so the per-phase benches see a stable runtime
/// state (criterion caching, instruction cache, etc.); the returned
/// portions are NOT extracted from the lint result.
fn collect_portions() -> Vec<CanonicalAttrs> {
    let input = build_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default scheme")
    .with_strict_recognizer();

    // Engine::lint doesn't expose the per-portion list, but for
    // measurement purposes we can replay the parse path. The
    // attribution probe only needs *some* representative
    // CanonicalAttrs slice to time `scheme.project` against; the
    // bench's actual lint_10kb regression comes from invoking
    // scheme.project ~20× per call (cache miss per portion). One
    // miss is sufficient to attribute the per-call cost.
    //
    // Simulate the cache-miss state: parse the input once, then
    // synthesize a representative portion mix matching the bench
    // input. Rather than wiring through internal accessors, build
    // a Vec<CanonicalAttrs> directly — that's the same shape the
    // engine accumulator carries internally.
    let _ = engine.lint(black_box(&input));

    let mut p1 = CanonicalAttrs::default();
    p1.classification = Some(marque_ism::MarkingClassification::Us(
        marque_ism::Classification::Secret,
    ));
    p1.dissem_us = vec![marque_ism::DissemControl::Nf].into_boxed_slice();

    let mut p2 = CanonicalAttrs::default();
    p2.classification = Some(marque_ism::MarkingClassification::Us(
        marque_ism::Classification::TopSecret,
    ));
    p2.sci_controls = vec![marque_ism::SciControl::Si].into_boxed_slice();

    vec![p1, p2]
}

fn phase_attribution(c: &mut Criterion) {
    let portions = collect_portions();
    let scheme = CapcoScheme::new();

    // phase_a_join_via_lattice: join_via_lattice in isolation.
    c.bench_function("phase_a_join_via_lattice", |b| {
        b.iter(|| {
            let attrs = CapcoMarking::join_via_lattice(black_box(&portions));
            black_box(attrs);
        });
    });

    // phase_b_closure: closure in isolation, applied to a pre-joined marking.
    //
    // Post-#704 scope note: `scheme.closure()` runs the 6-row
    // `CLOSURE_TABLE` Kleene fixpoint only (Rows 1-6, per-marking
    // unconditional implications from §H.4 marking templates). The
    // four "default if absent" rules (Rows 0/7/8/9 pre-#704) retired
    // to `marque_capco::scheme::default_fill`, which runs in
    // `scheme.project()` between close() and the supersession
    // overlay. `phase_b_closure` measures the additive Kleene
    // fixpoint ONLY — the `join_via_lattice` step is hoisted out of
    // the iter loop, so the per-call cost reflects closure dispatch
    // alone.
    //
    // See the file-level "Post-close stage cost" doc for the full
    // stage-decomposition algebra. `phase_b_prime_closure_on_unjoined`
    // below pairs with this bench: phase_b_closure isolates the
    // Kleene fixpoint, phase_b_prime measures join+closure, and
    // phase_c measures the full pipeline. The two deltas
    // (phase_c − phase_b vs phase_c − phase_b_prime) give different
    // attribution slices — see the file-level doc before using
    // either for regression triage.
    let joined_attrs = CapcoMarking::join_via_lattice(&portions);
    let joined = CapcoMarking::new(joined_attrs);
    c.bench_function("phase_b_closure", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(joined.clone()));
            black_box(out);
        });
    });

    // phase_c input, shared with phase_b_prime below so both pay the same
    // `&[CapcoMarking]` → `Vec<CanonicalAttrs>` clone bridge that
    // `MarkingScheme::project` performs internally.
    let markings: Vec<CapcoMarking> = portions.iter().cloned().map(CapcoMarking::new).collect();

    // phase_b_prime: mirror `scheme.project()`'s prefix — the
    // `&[CapcoMarking]` → `Vec<CanonicalAttrs>` clone bridge, then
    // `join_via_lattice` + `closure` — on the same `(S//NF) + (TS//SI)`
    // synthesis pair. Pairs with phase_c so the delta
    //
    //     phase_c_scheme_project − phase_b_prime_closure_on_unjoined
    //
    // isolates exactly the three post-close stages
    // (`apply_default_fill` + `apply_supersession_overlays` +
    // `page_rewrites`): both phases pay the bridge clone AND the join,
    // so both cancel in the delta.
    //
    // Why this is necessary: `phase_b_closure` hoists both the bridge
    // and `join_via_lattice` out of the iter loop, so `phase_c −
    // phase_b` recovers the bridge + join PLUS the three post-close
    // stages, not just the post-close stages. An earlier framing (#714)
    // claimed the delta was `default_fill + page_rewrites` only; this
    // bench closes that gap. (Review on #754 caught that an
    // earlier `phase_b_prime` joined `&portions` directly and never
    // paid the bridge, so `phase_c − phase_b_prime` still leaked the
    // bridge-clone cost — fixed here by paying it inside the loop.)
    //
    // Constitution VII §IV bench-only scope: `apply_default_fill`
    // is `pub(crate)` and `apply_supersession_overlays` is a private
    // associated fn; isolating either alone would require exposing
    // a production-surface entry point. The post-close-stages delta
    // above is the best honest attribution this scope allows.
    c.bench_function("phase_b_prime_closure_on_unjoined", |b| {
        b.iter(|| {
            // Mirror MarkingScheme::project's prefix verbatim:
            // `markings.iter().map(|m| m.0.clone()).collect()`.
            let raw: Vec<CanonicalAttrs> =
                black_box(&markings).iter().map(|m| m.0.clone()).collect();
            let attrs = CapcoMarking::join_via_lattice(&raw);
            let out = scheme.closure(CapcoMarking::new(attrs));
            black_box(out);
        });
    });

    // phase_c_scheme_project: whole scheme.project(Scope::Page, ...) call.
    c.bench_function("phase_c_scheme_project", |b| {
        b.iter(|| {
            let out = scheme.project(Scope::Page, black_box(&markings));
            black_box(out);
        });
    });

    // phase_d_from_canonical: from_canonical bridge.
    let projected = scheme.project(Scope::Page, &markings);
    c.bench_function("phase_d_from_canonical", |b| {
        b.iter(|| {
            let pm = marque_ism::ProjectedMarking::from_canonical(black_box(projected.0.clone()));
            black_box(pm);
        });
    });

    // phase_e_engine_project_path: end-to-end engine-side replay through the
    // `project_from_attrs_slice` fast-path (successor to
    // `project_from_page_context`).
    let page_portions: Vec<CanonicalAttrs> = portions.to_vec();
    c.bench_function("phase_e_engine_project_path", |b| {
        b.iter(|| {
            let projected = scheme.project_from_attrs_slice(&page_portions);
            let pm = marque_ism::ProjectedMarking::from_canonical(projected);
            black_box(pm);
        });
    });

    // phase_f_engine_lint_full: lint_10kb-style replay — full Engine::lint call. This
    // gives us the bench's actual call shape so we can compare the
    // per-phase costs against the total to find the missing time.
    {
        let input = build_input(10_000);
        let engine = Engine::new(
            Config::default(),
            marque_engine::default_ruleset(),
            marque_engine::default_scheme(),
        )
        .expect("default scheme")
        .with_strict_recognizer();
        c.bench_function("phase_f_engine_lint_full", |b| {
            b.iter(|| engine.lint(black_box(&input)));
        });
    }

    // phase_g_project: scaling — project_from_attrs_slice at portion counts
    // matching the lint_10kb call sequence (1, 5, 10, 25, 50). The
    // bench profiling discovered that ~50 cache-miss calls happen
    // with portions growing monotonically; the per-call O(n) work
    // dominates the regression.
    for &n in &[1usize, 5, 10, 25, 50] {
        let large_page: Vec<CanonicalAttrs> = (0..n).map(|_| portions[0].clone()).collect();
        c.bench_function(&format!("phase_g_project_n{}", n), |b| {
            b.iter(|| {
                let projected = scheme.project_from_attrs_slice(&large_page);
                let pm = marque_ism::ProjectedMarking::from_canonical(projected);
                black_box(pm);
            });
        });
    }

    // phase_h_tmp_ctx_rebuild: isolate the per-page accumulator rebuild cost.
    // Mirrors the engine's per-PageBreak `page_portions =
    // Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY)` + per-portion
    // `push` sequence.
    for &n in &[10usize, 25, 50] {
        let portions_slice: Vec<CanonicalAttrs> = (0..n).map(|_| portions[0].clone()).collect();
        c.bench_function(&format!("phase_h_tmp_ctx_rebuild_n{}", n), |b| {
            b.iter(|| {
                let mut ctx: Vec<CanonicalAttrs> = Vec::with_capacity(8);
                for p in black_box(&portions_slice) {
                    ctx.push(p.clone());
                }
                black_box(ctx);
            });
        });
    }

    // phase_i_join: measures the full `join_via_lattice` call, including
    // its internal tmp_ctx build. There is no clean isolation of the
    // tmp_ctx step alone from the rest of `join_via_lattice` on the
    // current public surface, so this bench reports the whole-function
    // cost. Pair with `phase_h_tmp_ctx_rebuild_n*` for a rough
    // attribution: phase_h ≈ tmp_ctx alone, phase_i ≈ tmp_ctx +
    // per-axis composition.
    for &n in &[10usize, 25, 50] {
        let portions_slice: Vec<CanonicalAttrs> = (0..n).map(|_| portions[0].clone()).collect();
        c.bench_function(&format!("phase_i_join_n{}", n), |b| {
            b.iter(|| {
                let attrs = CapcoMarking::join_via_lattice(black_box(&portions_slice));
                black_box(attrs);
            });
        });
    }
}

criterion_group!(benches, phase_attribution);
criterion_main!(benches);
