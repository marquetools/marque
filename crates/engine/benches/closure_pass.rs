// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Benches related to the closure-pass subsystem.
//!
//! Mix of two measurement shapes:
//!
//! 1. **`close()`-isolated micro-measurements.** Call
//!    `CapcoScheme::closure()` directly to measure the bitmask Kleene
//!    fixpoint path landed in PR-D (issue #371) against representative
//!    CAPCO input profiles. Post-#704 the close() catalog walks the
//!    6-row [`CLOSURE_TABLE`](marque_capco::closure_table::CLOSURE_TABLE)
//!    (Rows 1-6); the four "default if absent" rules pre-#704
//!    Rows 0/7/8/9 retired to `marque_capco::scheme::default_fill`,
//!    which runs in `scheme.project()` between close() and the
//!    supersession overlay.
//!
//! 2. **`close()`-driven-via-`project()` full-pipeline measurements.**
//!    Call `scheme.project(Scope::Page, &[marking])` to measure the
//!    user-observable cost of the post-#704 lattice pass: per-axis
//!    join → close() → default_fill → supersession overlay →
//!    page_rewrites. These benches
//!    inherit cost from every pipeline stage, not just the closure
//!    table walk. Bench names start with `project_*` so future
//!    triage doesn't read the cost as close()-only.
//!
//! # Post-#704 reconciliation (issue #714)
//!
//! Pre-#704, the close()-isolated benches measured the full lattice
//! pass because all 10 rows (transitive + default-fill) lived in
//! `CLOSURE_TABLE`. Post-#704 they measure a strictly smaller path
//! than their names imply (Rows 0/7/8/9 retired). The `closure_row*`
//! benches were renamed to `project_*` and rewritten to call
//! `scheme.project()` so the measured cost matches the user-facing
//! shape. `closure_hot1_exit` is kept as a close()-only bench
//! because its framing (HOT-1 early-exit guard) stays honest:
//! HOT-1 returns before any default-fill triggers can fire.
//! `closure_worst_case_all_rows` is kept as a close()-only bench
//! and a parallel `project_worst_case_all_rows` is added so
//! stage decomposition (close() vs full pipeline) stays visible.
//!
//! # Relationship to other benches
//!
//! `lint_latency.rs` / `linear_scaling.rs` measure end-to-end
//! `Engine::lint` cost (scanner + parser + rules + `project`).
//! This file isolates two layers of that stack: the close() Kleene
//! fixpoint alone (closure_* benches) and the close()-driven-via-
//! project() lattice pass (project_* benches). `profile_project.rs`
//! adds per-phase attribution on a single synthesized portion mix.
//!
//! # Maintenance contract
//!
//! If the `CLOSURE_TABLE` row count or atom inventory changes,
//! verify that `closure_worst_case_all_rows` / `project_worst_case_all_rows`
//! still exercise the intended rows (the intent is "fire each row
//! in the post-#704 6-row catalog at least once per Kleene
//! iteration"). The individual
//! scenarios are deliberately simple so their cost directly
//! reflects the bitmask dispatch plus the Kleene loop overhead
//! (close()-isolated benches) or the lattice + close + default-fill
//! + page-rewrites stack (project_* benches), not extraneous parsing.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_capco::CapcoMarking;
use marque_capco::scheme::CapcoScheme;
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, MarkingClassification, NatoClassification,
    SarIndicator, SarMarking, SarProgram, SciCompartment, SciControlBare, SciControlSystem,
    SciMarking,
};
use marque_scheme::{MarkingScheme, Scope};
use smol_str::SmolStr;
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Input constructors
// ---------------------------------------------------------------------------

/// UNCLASSIFIED, no special markings. No closure-trigger bits are set
/// (`derive_bits(..) & ALL_TRIGGER_MASK == 0`); HOT-1 guard exits without
/// evaluating any table row.
fn unclassified_no_markings() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Unclassified));
    CapcoMarking::new(a)
}

/// SECRET, no SCI/SAR/AEA/dissem/NATO. Post-#704: bare US-classification
/// no longer triggers a closure row; the RELIDO default-fill moved to
/// `default_fill::row9_should_fill`. Used by `project_us_classified` to
/// measure the full pipeline cost on the smallest US-classified fixture.
fn secret_no_special() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    CapcoMarking::new(a)
}

/// TOP SECRET + HCS-O compartment. Post-#704: Row 1
/// (`capco:closure.dissem.hcs-o-implies-noforn-orcon`) fires producing
/// NOFORN + ORCON via its cone in close(). In the full project()
/// pipeline the SI-G → ORCON → NOFORN chain still works end-to-end
/// (close() + default_fill); the close()-only measurement is one
/// Kleene iteration on HCS-O.
fn top_secret_hcs_o() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    let comp = SciCompartment::new(SmolStr::new("O"), Box::new([]));
    let sci = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        Box::new([comp]),
        None,
    );
    a.sci_markings = Box::new([sci]);
    CapcoMarking::new(a)
}

/// NATO Secret classification. Post-#704: Row 7 retired to
/// `default_fill::row7_should_fill`; NATO classification no longer
/// triggers any closure row. Used by `project_nato_default_fill`
/// to measure the full pipeline cost (close() HOT-1 exit + the
/// NATO REL TO USA, NATO injection in default_fill + page_rewrites).
fn nato_secret() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    CapcoMarking::new(a)
}

/// TOP SECRET + all 6 SCI compartment sentinels + SAR present + AEA/RD.
/// Post-#704: close() walks 6 rows; the SCI sentinel rows fire (HCS-O /
/// HCS-P[sub] / SI-G / TK-{BLFH,IDIT,KAND}). SAR / AEA / US-classification
/// retired to default-fill so they no longer trigger any closure row;
/// in `closure_worst_case_all_rows` this fixture measures dispatch
/// across the 6 surviving rows. In `project_worst_case_all_rows` the
/// full project() pipeline runs and the default-fill triggers (Rows
/// 0/9 equivalents) also contribute.
fn worst_case_all_rows() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));

    // All 6 SCI compartment sentinels: SI-G, HCS-O, HCS-P[sub], TK-BLFH,
    // TK-IDIT, TK-KAND — one SciMarking each so `derive_bits` lights all
    // six per-compartment sentinel bits (40–45).
    let si_g = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        Box::new([SciCompartment::new(SmolStr::new("G"), Box::new([]))]),
        None,
    );
    let hcs_o = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        Box::new([SciCompartment::new(SmolStr::new("O"), Box::new([]))]),
        None,
    );
    let hcs_p_sub = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        Box::new([SciCompartment::new(
            SmolStr::new("P"),
            Box::new([SmolStr::new("ABCD")]),
        )]),
        None,
    );
    let tk_blfh = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Tk),
        Box::new([SciCompartment::new(SmolStr::new("BLFH"), Box::new([]))]),
        None,
    );
    let tk_idit = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Tk),
        Box::new([SciCompartment::new(SmolStr::new("IDIT"), Box::new([]))]),
        None,
    );
    let tk_kand = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Tk),
        Box::new([SciCompartment::new(SmolStr::new("KAND"), Box::new([]))]),
        None,
    );
    a.sci_markings = Box::new([si_g, hcs_o, hcs_p_sub, tk_blfh, tk_idit, tk_kand]);

    // SAR present — lights `SAR_PRESENT` (bit 36). Post-#704 this
    // bit is in `default_fill::ROW0_CAVEATED_TRIGGERS` (Row 0 retired
    // from close() to default-fill); close() ignores it.
    let sar_prog = SarProgram::new(SmolStr::new("BP"), Box::new([]));
    a.sar_markings = Some(SarMarking::new(SarIndicator::Abbrev, Box::new([sar_prog])));

    // AEA/RD — lights `AEA_RD` (bit 22). Post-#704 this bit is also
    // in `default_fill::ROW0_CAVEATED_TRIGGERS`; close() ignores it.
    a.aea_markings = Box::new([AeaMarking::Rd(Default::default())]);

    // Post-#704: Rows 8-9 retired to default-fill; close() walks
    // the 6 SCI per-marking rows only. The "all rows fire" framing
    // applies to the surviving 6-row catalog for `closure_worst_case_all_rows`,
    // and to the full lattice pass for `project_worst_case_all_rows`.
    CapcoMarking::new(a)
}

/// Pre-join N identical TS+HCS-O markings for throughput scaling.
fn joined_hcs_o_n(n: usize) -> CapcoMarking {
    let portions: Vec<CanonicalAttrs> = (0..n).map(|_| top_secret_hcs_o().0).collect();
    let joined = CapcoMarking::join_via_lattice(&portions);
    CapcoMarking::new(joined)
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn closure_profiles(c: &mut Criterion) {
    let scheme = CapcoScheme::new();

    // ---- close()-isolated micro-measurements ----

    // HOT-1 early exit — UNCLASSIFIED, zero rows evaluated. Bench
    // framing post-#704 stays honest because HOT-1 returns before any
    // default-fill triggers could fire; close()-only measurement is
    // representative of the user-observable cost on this input.
    let hot1 = unclassified_no_markings();
    c.bench_function("closure_hot1_exit", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(hot1.clone()));
            black_box(out)
        });
    });

    // Post-#704 worst-case close()-isolated dispatch across the
    // surviving 6-row catalog. Measures bitmask Kleene fixpoint
    // dispatch cost only; the SAR / AEA / US-classification bits
    // in the fixture are carried for fixture completeness but
    // no longer contribute to close() cost (Rows 0/9 retired to
    // default_fill). Paired with `project_worst_case_all_rows`
    // for stage-decomposition visibility (close() vs full pipeline).
    // Framing-vs-name note: bench inherits ONLY the close() cost,
    // not the default_fill / page_rewrites stages.
    let worst = worst_case_all_rows();
    c.bench_function("closure_worst_case_all_rows", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(worst.clone()));
            black_box(out)
        });
    });

    // Batch throughput: 50 sequential closures on identical TS+HCS-O input.
    // Measures per-call amortized cost under realistic document-batch load.
    // The marking is pre-joined outside the iter loop; each clone + closure
    // call pair is timed (clone overhead is negligible for this input size).
    let batch_input = joined_hcs_o_n(1);
    c.bench_function("closure_batch_50", |b| {
        b.iter(|| {
            for _ in 0..50 {
                black_box(scheme.closure(black_box(batch_input.clone())));
            }
        });
    });

    // ---- close()-driven-via-project() full-pipeline measurements ----

    // Renamed from `closure_row9_us_classified` (#714).
    //
    // Post-#704: Row 9 (US_COLLATERAL_CLASSIFIED → RELIDO) retired
    // from `CLOSURE_TABLE` to `default_fill::row9_should_fill`. The
    // old bench called `scheme.closure()` on bare US-classified
    // input and measured the HOT-1 exit cost (not the Row 9 effect).
    // Rewritten to call `scheme.project(Scope::Page, &[marking])`
    // so the measurement reflects the full user-observable pipeline:
    // per-axis join → close() (HOT-1 exit on this input) →
    // default_fill (RELIDO injection fires) → page_rewrites.
    //
    // Framing-vs-name note: this bench inherits cost from lattice
    // + close + default_fill + page_rewrites — not just one stage.
    // Future regression triage on `project_us_classified` should
    // run `profile_project.rs::phase_b_closure` / `phase_c_scheme_project`
    // to attribute deltas to individual stages.
    let us_classified = secret_no_special();
    let us_classified_slice = [us_classified];
    c.bench_function("project_us_classified", |b| {
        b.iter(|| {
            let out = scheme.project(Scope::Page, black_box(&us_classified_slice));
            black_box(out)
        });
    });

    // Renamed from `closure_rows_1_0_hcs_o` (#714).
    //
    // Post-#704: Row 0 (transitive ORCON → NOFORN chain) retired
    // to `default_fill::row0_should_fill`. The old bench called
    // `scheme.closure()` and measured one Kleene iteration of
    // Row 1's cone (HCS-O → NOFORN + ORCON); the transitive
    // chain that produced the bench's original name no longer
    // runs inside close(). Rewritten to call
    // `scheme.project(Scope::Page, &[marking])` so the full HCS-O
    // → ORCON → NOFORN chain runs end-to-end (close() + default_fill).
    //
    // Framing-vs-name note: this bench inherits cost from lattice
    // + close + default_fill + page_rewrites. The `hcs_o_chain`
    // suffix refers to the closure+default-fill chain, not a
    // single CLOSURE_TABLE row.
    let hcs_o = top_secret_hcs_o();
    let hcs_o_slice = [hcs_o];
    c.bench_function("project_hcs_o_chain", |b| {
        b.iter(|| {
            let out = scheme.project(Scope::Page, black_box(&hcs_o_slice));
            black_box(out)
        });
    });

    // Renamed from `closure_row7_nato_class` (#714).
    //
    // Post-#704: Row 7 (NATO classification → REL TO USA, NATO
    // injection) retired to `default_fill::row7_should_fill`. The
    // old bench called `scheme.closure()` on NATO-classified input
    // and measured the HOT-1 exit cost (not the Row 7 effect).
    // Rewritten to call `scheme.project(Scope::Page, &[marking])`
    // so the NATO default-fill injection actually runs.
    //
    // Framing-vs-name note: this bench inherits cost from lattice
    // + close + default_fill + page_rewrites — the `default_fill`
    // suffix marks the stage that's the dominant cost driver on
    // this input post-#704.
    let nato = nato_secret();
    let nato_slice = [nato];
    c.bench_function("project_nato_default_fill", |b| {
        b.iter(|| {
            let out = scheme.project(Scope::Page, black_box(&nato_slice));
            black_box(out)
        });
    });

    // Parallel to `closure_worst_case_all_rows` (#714). Measures the
    // full project() pipeline on the same TOP SECRET + 6 SCI sentinels
    // + SAR + AEA/RD fixture. Both benches share the fixture; since
    // `closure_worst_case_all_rows` calls `scheme.closure()` directly
    // (closure only — no bridge, no join), the delta
    // (`project_worst_case_all_rows` − `closure_worst_case_all_rows`)
    // approximates everything `project()` does that bare `closure()`
    // does not: the `&[CapcoMarking]` → `Vec<CanonicalAttrs>` clone
    // bridge + join + default_fill + supersession overlay +
    // page_rewrites on the worst-case input. (For a post-close-stages-
    // only attribution that cancels the bridge + join, see the
    // `phase_b_prime` delta in `profile_project.rs`.)
    //
    // Framing-vs-name note: this bench inherits cost from the bridge +
    // lattice join + close + default_fill + supersession overlay +
    // page_rewrites; "all rows" applies to both the close() catalog AND
    // the default-fill triggers (SAR / AEA / US-classification) on this
    // input.
    let worst_proj = worst_case_all_rows();
    let worst_slice = [worst_proj];
    c.bench_function("project_worst_case_all_rows", |b| {
        b.iter(|| {
            let out = scheme.project(Scope::Page, black_box(&worst_slice));
            black_box(out)
        });
    });
}

criterion_group!(benches, closure_profiles);
criterion_main!(benches);
