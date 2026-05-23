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
//! profiles that exercise distinct code paths through the 6-row
//! [`CLOSURE_TABLE`](marque_capco::closure_table::CLOSURE_TABLE)
//! (post-#704; Rows 0/7/8/9 retired to
//! `marque_capco::scheme::default_fill`):
//!
//! 1. **`hot1_exit`** — UNCLASSIFIED, no special markings. The HOT-1
//!    early-exit guard `(derive_bits(attrs).bits() & ALL_TRIGGER_MASK) == 0`
//!    returns immediately; zero table rows evaluated.
//! 2. **`row9_us_classified`** — SECRET, no SCI/SAR/AEA/dissem. Post-#704
//!    this bench measures the HOT-1 exit cost on US-classified input (Row 9
//!    retired; no closure rows fire). Rewriting to call `scheme.project()`
//!    would more accurately measure the default-fill stage; tracked as a
//!    follow-up issue at #704 merge.
//! 3. **`rows_1_0_hcs_o`** — TOP SECRET + HCS-O compartment. Post-#704
//!    Row 1 (`capco:closure.dissem.hcs-o-implies-noforn-orcon`) fires
//!    producing ORCON + NOFORN. The pre-#704 Row 0 transitive NOFORN
//!    chain retired with Row 0 to `default_fill::row0_should_fill`; HCS-O
//!    via close() now produces NOFORN+ORCON directly (Row 1's cone),
//!    not via the transitive chain.
//! 4. **`row7_nato_class`** — NATO Secret classification. Post-#704 Row 7
//!    retired; NATO classification no longer triggers any closure row.
//!    The REL TO USA, NATO injection moved to
//!    `default_fill::row7_should_fill`. This bench now measures the
//!    HOT-1 exit cost on NATO input (NATO_CLASS is not in the post-#704
//!    `ALL_TRIGGER_MASK`).
//! 5. **`worst_case_all_rows`** — TOP SECRET + all 6 SCI compartment
//!    sentinels + SAR present + AEA/RD. Post-#704 close() walks 6 rows;
//!    SCI sentinel rows fire (HCS-O / HCS-P[sub] / SI-G / TK-{BLFH,IDIT,KAND}).
//!    SAR / AEA / US-classification no longer trigger any closure row
//!    (Rows 0/9 retired); they're default-fill triggers in the full
//!    pipeline.
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

/// SECRET, no SCI/SAR/AEA/dissem/NATO. Post-#704: Row 9 retired to
/// `default_fill::row9_should_fill`; this bench now measures the
/// HOT-1 exit cost on US-classified input (no closure rows fire on
/// bare US classification post-#704; `US_COLLATERAL_CLASSIFIED` is
/// not in the post-#704 `ALL_TRIGGER_MASK`). Rewriting to call
/// `scheme.project()` would measure the default-fill stage; tracked
/// as a follow-up issue at #704 merge.
fn secret_no_special() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    CapcoMarking::new(a)
}

/// TOP SECRET + HCS-O compartment. Post-#704: Row 1
/// (`capco:closure.dissem.hcs-o-implies-noforn-orcon`) fires producing
/// NOFORN + ORCON directly via its cone. The pre-#704 Row 0 transitive
/// chain (ORCON triggers caveated → NOFORN) retired with Row 0 to
/// `default_fill::row0_should_fill`; the SI-G → ORCON → NOFORN chain
/// still works end-to-end but crosses the close()/default_fill boundary
/// in the full project() pipeline. close() alone produces NOFORN +
/// ORCON in one Kleene iteration on HCS-O.
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
/// triggers any closure row. `NATO_CLASS` is not in the post-#704
/// `ALL_TRIGGER_MASK`, so this bench measures the HOT-1 exit cost on
/// NATO input. The REL TO USA, NATO injection moved to default-fill.
fn nato_secret() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    CapcoMarking::new(a)
}

/// TOP SECRET + all 6 SCI compartment sentinels + SAR present + AEA/RD.
/// Post-#704: close() walks 6 rows; the SCI sentinel rows fire (HCS-O /
/// HCS-P[sub] / SI-G / TK-{BLFH,IDIT,KAND}). SAR / AEA / US-classification
/// retired to default-fill so they no longer trigger any closure row;
/// this bench now measures dispatch cost across the 6 surviving rows
/// rather than the pre-#704 10-row catalog. The "worst case" framing
/// is still correct against the post-#704 catalog (every surviving row
/// can fire in one Kleene iteration on this input).
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
    // applies to the surviving 6-row catalog.
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

    // 2. Single-row (post-#704: was Row 9, retired to default-fill):
    //    US collateral classified. Post-#704 this measures the HOT-1
    //    exit cost on US-classified input; the Row 9 RELIDO injection
    //    moved to `default_fill::row9_should_fill`. Bench name kept
    //    for criterion-history continuity; follow-up issue tracks
    //    rewriting to `scheme.project()` for accurate measurement.
    let row9 = secret_no_special();
    c.bench_function("closure_row9_us_classified", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(row9.clone()));
            black_box(out)
        });
    });

    // 3. Single-row (post-#704: was rows 1 then 0 transitively):
    //    HCS-O via close() now produces NOFORN+ORCON directly via
    //    Row 1's cone in one Kleene iteration. The pre-#704
    //    transitive ORCON → Trio 1 → NOFORN chain retired with Row
    //    0 to `default_fill::row0_should_fill`; the full project()
    //    pipeline still produces the same end-to-end state via
    //    close() + default-fill.
    let hcs_o = top_secret_hcs_o();
    c.bench_function("closure_rows_1_0_hcs_o", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(hcs_o.clone()));
            black_box(out)
        });
    });

    // 4. Post-#704: Row 7 retired to `default_fill::row7_should_fill`;
    //    NATO classification no longer triggers any closure row. This
    //    bench now measures the HOT-1 exit cost on NATO input. Bench
    //    name kept for criterion-history continuity; follow-up issue
    //    tracks rewriting to `scheme.project()` for accurate measurement.
    let nato = nato_secret();
    c.bench_function("closure_row7_nato_class", |b| {
        b.iter(|| {
            let out = scheme.closure(black_box(nato.clone()));
            black_box(out)
        });
    });

    // 5. Worst case across the post-#704 6-row catalog: TOP SECRET +
    //    all 6 SCI compartment sentinels fires all 6 surviving rows
    //    (HCS-O / HCS-P[sub] / SI-G / TK-{BLFH,IDIT,KAND}) in one
    //    Kleene iteration. SAR / AEA / US-classification on the
    //    fixture do NOT trigger any close() row post-#704 (Rows 0/9
    //    retired to default-fill); they're carried for fixture
    //    completeness but don't contribute to close() dispatch cost.
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
