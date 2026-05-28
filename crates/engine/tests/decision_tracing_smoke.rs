// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase C smoke test for engine-side decision-tracing.
//!
//! Pins the minimum integration property: when a non-Noop
//! [`marque_scheme::DecisionSink`] is installed via
//! [`Engine::with_decision_sink`] and the engine lints a real
//! fixture, the sink receives at least one
//! [`marque_scheme::DecisionEvent`].
//!
//! The deeper integration test (event count > 100, cascade depth ≥ 2,
//! content-ignorance over the full event stream) is the Phase F
//! `tests/decision_tracing.rs` test, not this smoke. This file only
//! pins the engine wiring — sink construction, builder method,
//! emit-path execution.

#![cfg(feature = "decision-tracing")]

use std::sync::{Arc, Mutex};

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::Engine;
use marque_scheme::{DecisionEvent, DecisionSink};
use static_assertions::assert_impl_all;

// Constitution VI: `Engine` must remain `Send + Sync` under the
// `decision-tracing` feature so `BatchEngine` can keep sharing it
// across Tokio workers. Pinning this here (rather than at the
// definition site) because `static_assertions` is a `dev-dependency`.
assert_impl_all!(Engine: Send, Sync);

/// Test-only sink that retains a clone-of-events surface so the test
/// can inspect what the engine emitted after the sink moved into
/// `Engine`. The engine's owned sink is behind
/// `Mutex<Box<dyn SyncDecisionSink>>`; an `Arc<Mutex<Vec<_>>>` shared
/// with the test is the simplest hermetic accessor.
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

#[test]
fn engine_emits_through_shared_sink() {
    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Inspectable {
        events: events.clone(),
    };

    let engine = Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme constructs cleanly")
    .with_decision_sink(sink);

    let _ = engine.lint(b"(S//NF)");

    let observed = events.lock().expect("mutex not poisoned");
    assert!(
        !observed.is_empty(),
        "Phase C wiring should have emitted at least one DecisionEvent \
         when linting `(S//NF)`; observed 0 events. The most likely \
         cause is an emission site that compiled out (cfg gate \
         mismatched against the test's required-features), or the \
         emit helper short-circuited because the boxed sink resolved \
         to NoopSink (the builder method `with_decision_sink` must \
         have replaced the default before lint runs)."
    );
}

#[test]
fn step_counter_resets_between_lint_calls() {
    // Per Constitution review HIGH finding: the step counter is
    // documented as "per-document" — it MUST reset between `lint`
    // calls so `triggered_by` references resolve into the current
    // document's event stream only. The simplest pin: emit on doc
    // A, snapshot the max step, clear the buffer, emit on doc B,
    // assert doc B's events start back at step 0.
    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Inspectable {
        events: events.clone(),
    };

    let engine = Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme constructs cleanly")
    .with_decision_sink(sink);

    let _ = engine.lint(b"(S//NF)");
    let doc_a_max_step = events
        .lock()
        .expect("mutex not poisoned")
        .iter()
        .map(|e| e.step)
        .max()
        .expect("doc A produced events");

    events.lock().expect("mutex not poisoned").clear();

    let _ = engine.lint(b"(S//NF)");
    let doc_b_min_step = events
        .lock()
        .expect("mutex not poisoned")
        .iter()
        .map(|e| e.step)
        .min()
        .expect("doc B produced events");

    assert_eq!(
        doc_b_min_step, 0,
        "Doc B's first step ID should be 0; got {doc_b_min_step} (doc A's max was {doc_a_max_step}). \
         If the counter persisted across documents, doc B would start at doc_a_max_step + 1, \
         which breaks RecordingSink::into_report cascade-chain reconstruction."
    );
}
