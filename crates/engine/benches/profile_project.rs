// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-D.2 commit 7 — one-shot phase-attribution probe for
//! `CapcoScheme::project`.
//!
//! Mirrors `build_representative_input` from `lint_latency.rs`, parses
//! the document into per-portion `CanonicalAttrs` values via the
//! engine's strict pipeline, then measures isolated calls to
//! (a) `join_via_lattice`, (b) `closure`, (c) `page_rewrites` loop,
//! (d) `from_canonical`, and (e) the whole `scheme.project +
//! from_canonical` pipeline.
//!
//! Used to attribute the lint_10kb regression; the file ships with
//! commit 7 but is hand-run via `cargo bench --bench profile_project`
//! when needed.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_capco::CapcoMarking;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::{Engine, StrictRecognizer};
use marque_ism::{CanonicalAttrs, PageContext};
use marque_scheme::{MarkingScheme, Scope};
use std::hint::black_box;
use std::sync::Arc;

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

/// Extract the set of per-portion `CanonicalAttrs` produced by the
/// engine's strict pipeline on the bench input. We lint once and grab
/// the page-context portions; replay them under the per-phase
/// micro-benches below.
fn collect_portions() -> Vec<CanonicalAttrs> {
    let input = build_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default scheme")
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    // Engine::lint doesn't expose the per-portion list, but for
    // measurement purposes we can replay the parse path. The
    // attribution probe only needs *some* representative
    // CanonicalAttrs slice to time `scheme.project` against; the
    // bench's actual lint_10kb regression comes from invoking
    // scheme.project ~20× per call (cache miss per portion). One
    // miss is sufficient to attribute the per-call cost.
    //
    // Simulate the cache-miss state: parse the input once, grab
    // the post-lint portions via a fresh `PageContext::add_portion`
    // walk of the engine's first-pass parsed markings. Rather than
    // wiring through internal accessors, just synthesize a
    // representative portion mix matching the bench input.
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

    // Phase A: join_via_lattice in isolation.
    c.bench_function("phase_a_join_via_lattice", |b| {
        b.iter(|| {
            let attrs = CapcoMarking::join_via_lattice(black_box(&portions));
            black_box(attrs);
        });
    });

    // Phase B: closure in isolation, applied to a pre-joined marking.
    let joined_attrs = CapcoMarking::join_via_lattice(&portions);
    let joined = CapcoMarking::new(joined_attrs);
    c.bench_function("phase_b_closure", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(joined.clone()));
            black_box(out);
        });
    });

    // Phase C: whole scheme.project(Scope::Page, ...) call.
    let markings: Vec<CapcoMarking> = portions
        .iter()
        .cloned()
        .map(CapcoMarking::new)
        .collect();
    c.bench_function("phase_c_scheme_project", |b| {
        b.iter(|| {
            let out = scheme.project(Scope::Page, black_box(&markings));
            black_box(out);
        });
    });

    // Phase D: from_canonical bridge.
    let projected = scheme.project(Scope::Page, &markings);
    c.bench_function("phase_d_from_canonical", |b| {
        b.iter(|| {
            let pm = marque_ism::ProjectedMarking::from_canonical(black_box(projected.0.clone()));
            black_box(pm);
        });
    });

    // Phase E: end-to-end engine-side replay through the new fast-path
    // `project_from_page_context`.
    let mut page_context = PageContext::new();
    for p in &portions {
        page_context.add_portion(p.clone());
    }
    c.bench_function("phase_e_engine_project_path", |b| {
        b.iter(|| {
            let projected = scheme.project_from_page_context(&page_context);
            let pm = marque_ism::ProjectedMarking::from_canonical(projected);
            black_box(pm);
        });
    });

    // Phase F: lint_10kb-style replay — full Engine::lint call. This
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
        .with_recognizer(Arc::new(StrictRecognizer::new()));
        c.bench_function("phase_f_engine_lint_full", |b| {
            b.iter(|| engine.lint(black_box(&input)));
        });
    }

    // Phase G: scaling — project_from_attrs_slice at portion counts
    // matching the lint_10kb call sequence (1, 5, 10, 25, 50). The
    // bench profiling discovered that ~50 cache-miss calls happen
    // with portions growing monotonically; the per-call O(n) work
    // dominates the regression.
    for &n in &[1usize, 5, 10, 25, 50] {
        let mut large_page = PageContext::new();
        for _ in 0..n {
            large_page.add_portion(portions[0].clone());
        }
        c.bench_function(&format!("phase_g_project_n{}", n), |b| {
            b.iter(|| {
                let projected = scheme.project_from_page_context(&large_page);
                let pm = marque_ism::ProjectedMarking::from_canonical(projected);
                black_box(pm);
            });
        });
    }

    // Phase H: isolate the tmp_ctx rebuild cost. Mirrors
    // `join_via_lattice`'s per-call tmp_ctx construction.
    for &n in &[10usize, 25, 50] {
        let portions_slice: Vec<CanonicalAttrs> = (0..n).map(|_| portions[0].clone()).collect();
        c.bench_function(&format!("phase_h_tmp_ctx_rebuild_n{}", n), |b| {
            b.iter(|| {
                let mut ctx = PageContext::new();
                for p in black_box(&portions_slice) {
                    ctx.add_portion(p.clone());
                }
                black_box(ctx);
            });
        });
    }

    // Phase I: isolate the lattice-axis composition cost (join_via_lattice
    // alone, no tmp_ctx — fails because join_via_lattice DOES build
    // tmp_ctx; this phase measures the WHOLE join_via_lattice).
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
