// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme::closure()` micro-bench across representative CAPCO input
//! profiles.
//!
//! # Purpose
//!
//! PR-F of the FactBitmask refactor (issue #371). Measures the bitmask
//! Kleene fixpoint path landed in PR-D against six representative input
//! profiles that exercise distinct code paths through the 10-row
//! [`CLOSURE_TABLE`](marque_capco::closure_table::CLOSURE_TABLE):
//!
//! 1. **`hot1_exit`** — UNCLASSIFIED, no special markings. The HOT-1
//!    early-exit guard `(derive_bits(attrs).bits() & ALL_TRIGGER_MASK) == 0`
//!    returns immediately; zero table rows evaluated.
//! 2. **`row9_us_classified`** — SECRET, no SCI/SAR/AEA/dissem. Only Row 9
//!    (`capco/relido-if-us-collateral-class`) fires; one-iteration fixpoint.
//! 3. **`rows_1_0_hcs_o`** — TOP SECRET + HCS-O compartment. Row 1
//!    (`capco/hcs-o-implies-noforn-orcon`) fires, then Row 0
//!    (`capco/noforn-if-caveated`) fires transitively via the ORCON cone;
//!    two-iteration fixpoint.
//! 4. **`row7_nato_class`** — NATO Secret classification. Row 7
//!    (`capco/rel-to-usa-nato-if-nato-classification`) fires; the
//!    closed-vocab cone adds `REL_TO_USA` and the open-vocab NATO
//!    tetragraph cone is applied by `closure()` after the Kleene loop.
//! 5. **`worst_case_all_rows`** — TOP SECRET + all 6 SCI compartment
//!    sentinels + SAR present + AEA/RD. All applicable rows fire across
//!    multiple Kleene iterations; maximum dispatch cost.
//! 6. **`batch_closure`** — applies closure to 50 identical TS+HCS-O markings
//!    in sequence; measures throughput across a document batch.
//!
//! # Relationship to other benches
//!
//! `lint_latency.rs` / `linear_scaling.rs` measure end-to-end `Engine::lint`
//! cost (scanner + parser + rules + `project`). This bench isolates `closure()`
//! alone — the projection step that runs once per page after `join_via_lattice`.
//! `profile_project.rs::phase_b_closure` covers the same call but only for the
//! `(S//NF) + (TS//SI)` synthesis pair. This bench adds the HOT-1 exit,
//! per-marking-row, NATO, and worst-case profiles that `phase_b_closure` omits.
//!
//! # Maintenance contract
//!
//! If the `CLOSURE_TABLE` row count or atom inventory changes, verify that
//! the worst-case scenario still exercises the intended rows (the intent is
//! "fire every row at least once per Kleene iteration"). The individual
//! scenarios are deliberately simple so their cost directly reflects the
//! bitmask dispatch plus the Kleene loop overhead, not extraneous parsing.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_capco::CapcoMarking;
use marque_capco::scheme::CapcoScheme;
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, MarkingClassification, NatoClassification,
    SarIndicator, SarMarking, SarProgram, SciCompartment, SciControlBare, SciControlSystem,
    SciMarking,
};
use marque_scheme::MarkingScheme;
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

/// SECRET, no SCI/SAR/AEA/dissem/NATO. Only Row 9
/// (`capco/relido-if-us-collateral-class`) fires; one-iteration fixpoint.
fn secret_no_special() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    CapcoMarking::new(a)
}

/// TOP SECRET + HCS-O compartment. Row 1 (`capco/hcs-o-implies-noforn-orcon`)
/// fires; Row 0 (`capco/noforn-if-caveated`) follows transitively via the ORCON
/// cone; two-iteration fixpoint on first call.
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

/// NATO Secret classification. Row 7 fires; closed-vocab `REL_TO_USA` cone plus
/// the open-vocab NATO tetragraph cone applied by `closure()` outside the loop.
fn nato_secret() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    CapcoMarking::new(a)
}

/// TOP SECRET + all 6 SCI compartment sentinels + SAR present + AEA/RD.
/// All applicable rows fire across multiple Kleene iterations; maximum dispatch.
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

    // SAR present — lights `SAR_PRESENT` (bit 36); triggers Row 0 (Trio 1).
    let sar_prog = SarProgram::new(SmolStr::new("BP"), Box::new([]));
    a.sar_markings = Some(SarMarking::new(SarIndicator::Abbrev, Box::new([sar_prog])));

    // AEA/RD — lights `AEA_RD` (bit 22); triggers Row 0 (Trio 1) via the
    // `AEA_RD` entry in `ROW0_NOFORN_IF_CAVEATED_TRIGGERS`.
    a.aea_markings = Box::new([AeaMarking::Rd(Default::default())]);

    // RELIDO suppressor absent: rows 8-9 should fire for SCI + US class.
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

    // 1. HOT-1 early exit — UNCLASSIFIED, zero rows evaluated.
    let hot1 = unclassified_no_markings();
    c.bench_function("closure_hot1_exit", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(hot1.clone()));
            black_box(out)
        });
    });

    // 2. Single-row (Row 9): US collateral classified → RELIDO.
    let row9 = secret_no_special();
    c.bench_function("closure_row9_us_classified", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(row9.clone()));
            black_box(out)
        });
    });

    // 3. Two-step (Rows 1 then 0): HCS-O → NOFORN+ORCON; ORCON triggers Trio 1.
    let hcs_o = top_secret_hcs_o();
    c.bench_function("closure_rows_1_0_hcs_o", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(hcs_o.clone()));
            black_box(out)
        });
    });

    // 4. Trio 3: NATO classification → REL_TO_USA + open-vocab NATO cone.
    let nato = nato_secret();
    c.bench_function("closure_row7_nato_class", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(nato.clone()));
            black_box(out)
        });
    });

    // 5. Worst case: all 6 SCI sentinels + SAR + AEA/RD → all non-NATO rows fire
    //    across multiple Kleene iterations (Row 7 requires NATO_CLASS, absent here).
    let worst = worst_case_all_rows();
    c.bench_function("closure_worst_case_all_rows", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(worst.clone()));
            black_box(out)
        });
    });

    // 6. Batch throughput: 50 sequential closures on identical TS+HCS-O input.
    //    Measures per-call amortized cost under realistic document-batch load.
    //    The marking is pre-joined outside the iter loop; each clone + closure
    //    call pair is timed (clone overhead is negligible for this input size).
    let batch_input = joined_hcs_o_n(1);
    c.bench_function("closure_batch_50", |b| {
        b.iter(|| {
            for _ in 0..50 {
                black_box(scheme.closure(black_box(batch_input.clone())));
            }
        });
    });
}

criterion_group!(benches, closure_profiles);
criterion_main!(benches);
