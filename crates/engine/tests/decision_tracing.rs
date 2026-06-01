// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase F integration test for decision-tracing.
//!
//! Pins three properties at scale on a 10 KB representative input:
//!
//! 1. **Event volume** — the instrumented engine produces a meaningful
//!    stream (> 100 events on a 10 KB mixed-marking + prose input), not
//!    just a single emission per portion.
//! 2. **Wiring coverage** — every "important" [`DecisionKind`] reaches
//!    the sink at least once across the run: [`DecisionKind::Evaluated`]
//!    (per-axis dispatch), [`DecisionKind::ConstraintFired`]
//!    (constraint-bridge emission), and
//!    [`DecisionKind::RewriteScheduled`] (page-rewrite fan-out).
//! 3. **Content ignorance (Constitution V Principle V)** — a unique
//!    sentinel substring injected into the fixture's prose lines does
//!    NOT appear in the debug projection of any emitted event. The
//!    same guarantee the G13 audit canary pins for NDJSON audit
//!    output, extended to the decision-event stream.
//!
//! Cascade-chain reconstruction is exercised by the focused
//! `decision_tracing_cascade_chains_resolve_within_document` test
//! below, which feeds the captured events through
//! [`RecordingSink::into_report_from_events`] and asserts at least one
//! [`CascadeChain`] reaches `depth >= 1`.
//!
//! The Phase C/D smoke tests at `decision_tracing_smoke.rs` pin the
//! minimum integration property (sink receives at least one event);
//! this file is the at-scale companion the smoke tests deferred.

#![cfg(feature = "decision-tracing")]

use std::sync::{Arc, Mutex};

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_scheme::{DecisionEvent, DecisionKind, DecisionSink, RecordingSink};

/// Sentinel substring embedded into the fixture's prose lines. Chosen
/// to be lexically distinctive — neither a CAPCO token nor a closed-
/// enum identifier nor a plausible BLAKE3 hex digit — so any appearance
/// in event debug-format output is unambiguously a content leak.
const WIDGET_SENTINEL: &str = "WIDGETIPHRASESENTINEL";

/// Build a ~10 KB representative input mirroring `build_representative_input`
/// from `benches/deadline_overhead.rs`, but with the sentinel string
/// woven into every prose line. The marking shape stays valid so the
/// engine actually exercises the page-rewrite + closure paths (a fully
/// invalid corpus would skew the event-kind distribution).
fn build_representative_input_with_sentinel(target_bytes: usize) -> Vec<u8> {
    // Sentinel embedded ONLY in prose lines (Lorem-ipsum text), never
    // inside a marking. Markings stay valid so the engine's
    // page-rewrite and closure stages fire end-to-end.
    let block = format!(
        "TOP SECRET//SCI//NOFORN\n\
         \n\
         Lorem {sentinel} dolor sit amet, consectetur adipiscing elit. Sed do\n\
         eiusmod tempor incididunt ut {sentinel} et dolore magna aliqua.\n\
         \n\
         (S//NF) This portion contains abbreviated dissemination controls.\n\
         \n\
         SECRET//NOFORN//REL TO USA, GBR\n\
         \n\
         Ut {sentinel} ad minim veniam, quis nostrud exercitation {sentinel} laboris.\n\
         \n\
         (TS//SI) Another portion with SCI controls and valid formatting.\n\
         \n",
        sentinel = WIDGET_SENTINEL,
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

/// Shared-mutable capture sink mirroring the smoke-test `Inspectable`
/// pattern — pushes every event into an `Arc<Mutex<Vec<_>>>` the test
/// retains a clone of, so the event stream is reachable after the
/// sink moves into [`Engine::with_decision_sink`].
#[derive(Clone)]
struct Inspectable {
    events: Arc<Mutex<Vec<DecisionEvent>>>,
}

impl DecisionSink for Inspectable {
    fn record(&mut self, event: DecisionEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

/// Build a default-config engine with the CAPCO scheme + capture sink
/// installed.
fn build_engine_with_capture() -> (CapcoEngine, Arc<Mutex<Vec<DecisionEvent>>>) {
    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Inspectable {
        events: events.clone(),
    };
    let engine = CapcoEngine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme constructs cleanly")
    .with_decision_sink(sink);
    (engine, events)
}

#[test]
fn decision_tracing_emits_at_scale_with_content_ignorance() {
    let input = build_representative_input_with_sentinel(10_000);
    let (engine, events) = build_engine_with_capture();

    let _ = engine.lint(&input);

    let observed = events.lock().expect("mutex not poisoned");

    // ---------------------------------------------------------------
    // Property 1 — event volume.
    //
    // Phase D's local-counter smoke produced ~88 events on a single
    // tiny portion. A 10 KB mixed-marking + prose input (4 valid
    // markings per ~400-byte block, ~25 blocks at 10 KB) should
    // easily clear 100. A floor below that indicates the engine
    // collapsed an emission path (most likely a feature-gate
    // mismatch on a per-axis emit site).
    // ---------------------------------------------------------------
    assert!(
        observed.len() > 100,
        "decision-tracing should emit > 100 events on a 10 KB \
         representative input; observed {}. Most likely cause: an \
         instrumented emission site compiled out (cfg gate mismatched \
         the test's required-features), or the per-axis dispatch path \
         stopped emitting Evaluated events.",
        observed.len(),
    );

    // ---------------------------------------------------------------
    // Property 2 — wiring coverage across the three load-bearing kinds.
    //
    // Each `DecisionKind` listed below is produced by a distinct
    // pipeline stage; an absence indicates that stage's emission
    // wiring regressed.
    //  - `Evaluated` comes from per-portion rule dispatch (engine).
    //  - `ConstraintFired` comes from the constraint bridge (engine).
    //  - `RewriteScheduled` comes from page-rewrite execution (scheme).
    // ---------------------------------------------------------------
    let has_evaluated = observed
        .iter()
        .any(|e| matches!(e.kind, DecisionKind::Evaluated));
    let has_constraint_fired = observed
        .iter()
        .any(|e| matches!(e.kind, DecisionKind::ConstraintFired));
    let has_rewrite_scheduled = observed
        .iter()
        .any(|e| matches!(e.kind, DecisionKind::RewriteScheduled));

    assert!(
        has_evaluated,
        "no DecisionKind::Evaluated event observed across {} events. \
         The per-portion rule-dispatch emission path appears to have \
         regressed; check `dispatch_rules_for_marking` in \
         `crates/engine/src/engine/lint_helpers.rs`.",
        observed.len(),
    );
    assert!(
        has_constraint_fired,
        "no DecisionKind::ConstraintFired event observed across {} \
         events. The constraint-bridge emission path appears to have \
         regressed; check `crates/engine/src/engine/bridge.rs`. The \
         fixture's mixed-control markings (TS//SCI//NOFORN, REL TO USA, \
         GBR) should produce at least one constraint hit.",
        observed.len(),
    );
    assert!(
        has_rewrite_scheduled,
        "no DecisionKind::RewriteScheduled event observed across {} \
         events. The page-rewrite execution emission path appears to \
         have regressed; check the page-rewrite loop in \
         `crates/capco/src/scheme/marking_scheme_impl.rs`. The fixture's \
         NOFORN + REL TO co-occurrence should trigger \
         `noforn-clears-rel-to`.",
        observed.len(),
    );

    // ---------------------------------------------------------------
    // Property 3 — cascade depth reachable.
    //
    // Phase D's local-counter approach gives at least
    // RewriteScheduled → RewriteApplied edges, so depth 1 is the
    // achievable floor on a fixture that fires any rewrite at all.
    // ---------------------------------------------------------------
    let report =
        RecordingSink::into_report_from_events(observed.iter().copied().collect::<Vec<_>>());
    assert!(
        report.max_cascade_depth >= 1,
        "max_cascade_depth = {} (< 1) on a 10 KB representative input. \
         Cascade reconstruction failed even though the input fires \
         page-rewrites — verify `triggered_by` is populated on the \
         RewriteApplied children emitted alongside RewriteScheduled \
         parents.",
        report.max_cascade_depth,
    );

    // ---------------------------------------------------------------
    // Property 4 — content ignorance.
    //
    // The G13 canary at `audit_g13_canary.rs` pins this property for
    // NDJSON audit-record output via a JSON-aware sweep. The decision-
    // event stream uses the same invariant: `DecisionEvent` carries
    // only IDs, indices, enum tags, and `&'static str` source labels
    // (per the type definition in `crates/scheme/src/decision.rs`).
    // The simplest empirical check is to debug-format every event and
    // assert the sentinel substring is absent — `Debug` projects every
    // string-bearing field by design, so a leak through any future
    // field addition would surface here.
    // ---------------------------------------------------------------
    for (idx, event) in observed.iter().enumerate() {
        let debug_repr = format!("{event:?}");
        assert!(
            !debug_repr.contains(WIDGET_SENTINEL),
            "content-ignorance violation at event #{idx} (step={}): the \
             debug projection contains the sentinel substring \
             {WIDGET_SENTINEL:?}. DecisionEvent must carry only IDs, \
             indices, enum tags, and `&'static str` labels (Constitution \
             V Principle V). A future field that interpolates input \
             content would land here.\n\n\
             Event debug: {debug_repr}",
            event.step,
        );
    }
}

#[test]
fn decision_tracing_cascade_chains_resolve_within_document() {
    // Multi-portion banner-rollup fixture. Pairs explicit NOFORN with
    // explicit REL TO so the page projection fires
    // `noforn-clears-rel-to` — the same fixture shape Phase D's smoke
    // tests pin for the page-rewrite emission path. The added
    // expectation here is that the captured event stream, fed through
    // `RecordingSink::into_report_from_events`, reconstructs a cascade
    // with `depth >= 1`. Depth 1 is the floor — the page-rewrite
    // fan-out alone produces a RewriteScheduled parent + at least one
    // RewriteApplied child edge.
    let (engine, events) = build_engine_with_capture();
    let input = b"SECRET//NOFORN\n\n(S//NF) text (S//REL TO USA, FVEY)";

    let _ = engine.lint(input);

    let captured: Vec<DecisionEvent> = events
        .lock()
        .expect("mutex not poisoned")
        .iter()
        .copied()
        .collect();
    let report = RecordingSink::into_report_from_events(captured);

    assert!(
        !report.cascade_chains.is_empty(),
        "RecordingSink::into_report_from_events should reconstruct at \
         least one cascade chain on a multi-portion NOFORN+REL TO \
         fixture; got 0 chains. Cascade reconstruction reads \
         `triggered_by` edges — verify the engine populates that field \
         on derived events."
    );

    let chains_with_depth: Vec<u32> = report
        .cascade_chains
        .iter()
        .map(|c| c.depth)
        .filter(|d| *d >= 1)
        .collect();
    assert!(
        !chains_with_depth.is_empty(),
        "no cascade chain reached depth >= 1 across {} chains. The \
         max-depth across all chains was {}. The fixture should \
         produce at least one RewriteScheduled → RewriteApplied edge \
         on `noforn-clears-rel-to`; if no chain exceeds depth 0, the \
         `triggered_by` edge is missing on the child event.",
        report.cascade_chains.len(),
        report.max_cascade_depth,
    );
}
