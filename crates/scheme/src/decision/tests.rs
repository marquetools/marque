// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Unit tests for the decision-tracing instrumentation surface.

use static_assertions::const_assert_eq;

use crate::category::CategoryId;
use crate::decision::sinks::{CountingSink, NoopSink, RecordingSink};
use crate::decision::{DecisionEvent, DecisionKind, DecisionSink, DecisionSite, DecisionSource};

// Constitution I: NoopSink must be ZST so threading it through the
// engine adds nothing to the hot path.
const_assert_eq!(std::mem::size_of::<NoopSink>(), 0);

// DecisionEvent stays small enough that a 10K-event RecordingSink fits
// in roughly half a megabyte (the layout floor is set by DecisionSource,
// which carries `&'static str` fat pointers). Pin the size here so a
// layout change surfaces as a test failure.
const_assert_eq!(std::mem::size_of::<DecisionEvent>(), 56);

#[test]
fn noop_sink_is_zero_sized() {
    // Belt-and-braces: runtime check mirrors the compile-time
    // const_assert above so the test surfaces clearly if the type
    // ever grows a field.
    assert_eq!(std::mem::size_of::<NoopSink>(), 0);
    let mut sink = NoopSink::new();
    // Confirm record() accepts events without panicking.
    sink.record(make_event(0, DecisionSite::Banner, CategoryId(1), None));
}

#[test]
fn counting_sink_accumulates_totals_by_kind_category_and_portion() {
    let mut sink = CountingSink::new();

    // Inject 900 events with rotating kinds, categories, and
    // portion indices.
    let kinds = [
        DecisionKind::Evaluated,
        DecisionKind::EvaluatedSubstantive,
        DecisionKind::Mutated,
        DecisionKind::ConstraintFired,
        DecisionKind::RewriteScheduled,
        DecisionKind::RewriteApplied,
        DecisionKind::ClosureFired,
        DecisionKind::Recanonicalized,
        DecisionKind::Derived,
    ];

    let categories = [
        CategoryId::MARKING,
        CategoryId(1),
        CategoryId(2),
        CategoryId(3),
        CategoryId(7),
    ];

    // portion_count and category/kind divisors all divide
    // total_events evenly so the per-bucket assertions can use
    // exact equality. LCM(9 kinds, 5 categories, 10 portions) = 90;
    // 900 is the smallest multiple of 90 in the same magnitude as the
    // original count (900/9 = 100, 900/5 = 180, 900/10 = 90).
    let portion_count: u32 = 10;
    let total_events: u32 = 900;

    for step in 0..total_events {
        let kind = kinds[step as usize % kinds.len()];
        let category = categories[step as usize % categories.len()];
        let portion_idx = step % portion_count;
        sink.record(DecisionEvent {
            step,
            site: DecisionSite::Portion(portion_idx),
            category,
            kind,
            source: DecisionSource::Parser,
            triggered_by: None,
        });
    }

    let report = sink.into_report();

    assert_eq!(report.total, u64::from(total_events));

    // Every kind appears exactly 900 / 9 = 100 times.
    let per_kind = u64::from(total_events) / kinds.len() as u64;
    for kind in kinds {
        assert_eq!(
            report.by_kind.get(&kind).copied().unwrap_or(0),
            per_kind,
            "kind {kind:?} count"
        );
    }

    // Every category appears 900 / 5 = 180 times.
    let per_category = u64::from(total_events) / categories.len() as u64;
    for category in categories {
        assert_eq!(
            report.by_category.get(&category).copied().unwrap_or(0),
            per_category,
            "category {category:?} count"
        );
    }

    // by_portion has portion_count entries (the highest index seen
    // is portion_count - 1), each at total_events / portion_count.
    assert_eq!(report.by_portion.len(), portion_count as usize);
    let per_portion = u64::from(total_events) / u64::from(portion_count);
    for (i, count) in report.by_portion.iter().enumerate() {
        assert_eq!(*count, per_portion, "portion {i} count");
    }

    // Sum of per-category counts equals total.
    let category_sum: u64 = report.by_category.values().copied().sum();
    assert_eq!(category_sum, u64::from(total_events));

    // Sum of per-portion counts equals total.
    let portion_sum: u64 = report.by_portion.iter().copied().sum();
    assert_eq!(portion_sum, u64::from(total_events));

    // Sum of per-kind counts equals total. by_kind is maintained
    // via a different code path (dense array + KIND_ORDER mapping)
    // than total / by_category / by_portion; the cross-check catches
    // a drift between the two if one is ever changed in isolation.
    let kind_sum: u64 = report.by_kind.values().copied().sum();
    assert_eq!(kind_sum, u64::from(total_events));

    // No cascade reconstruction from a counting sink.
    assert!(report.cascade_chains.is_empty());
    assert_eq!(report.max_cascade_depth, 0);
}

#[test]
fn recording_sink_reconstructs_three_level_cascade() {
    // Topology:
    //   step 0 (root)
    //     ├── step 1
    //     │     └── step 3
    //     └── step 2
    //           └── step 4
    //
    // Root at depth 0, children at depth 1, grandchildren at
    // depth 2. Per the plan: max_cascade_depth == 2.
    let mut sink = RecordingSink::with_capacity(5);

    sink.record(make_event(
        0,
        DecisionSite::Banner,
        CategoryId::MARKING,
        None,
    ));
    sink.record(make_event(
        1,
        DecisionSite::Portion(0),
        CategoryId(1),
        Some(0),
    ));
    sink.record(make_event(
        2,
        DecisionSite::Portion(1),
        CategoryId(2),
        Some(0),
    ));
    sink.record(make_event(
        3,
        DecisionSite::Portion(0),
        CategoryId(1),
        Some(1),
    ));
    sink.record(make_event(
        4,
        DecisionSite::Portion(1),
        CategoryId(2),
        Some(2),
    ));

    let report = sink.into_report();

    assert_eq!(report.total, 5);
    assert_eq!(report.cascade_chains.len(), 1);
    assert_eq!(report.max_cascade_depth, 2);

    let chain = &report.cascade_chains[0];
    assert_eq!(chain.root_event, 0);
    assert_eq!(chain.root_site, DecisionSite::Banner);
    assert_eq!(chain.depth, 2);
    assert_eq!(chain.events.len(), 5);

    // Every step ID is present in the chain.
    let mut events_sorted = chain.events.clone();
    events_sorted.sort_unstable();
    assert_eq!(events_sorted, vec![0, 1, 2, 3, 4]);

    // Root is first in DFS pre-order.
    assert_eq!(chain.events[0], 0);
}

#[test]
fn recording_sink_treats_dangling_parent_as_root() {
    // An event whose triggered_by points to a missing step is
    // treated as a cascade root. Defensive against partial captures.
    let mut sink = RecordingSink::new();
    sink.record(make_event(0, DecisionSite::Banner, CategoryId(1), None));
    sink.record(make_event(
        5,
        DecisionSite::Portion(0),
        CategoryId(1),
        Some(99), // Parent step 99 was never recorded.
    ));

    let report = sink.into_report();
    assert_eq!(report.cascade_chains.len(), 2);
    assert_eq!(report.max_cascade_depth, 0);
}

#[test]
fn recording_sink_handles_self_referential_event() {
    // An event whose `triggered_by` points to its own step is treated
    // as having no parent (the parent-step lookup finds the event itself
    // and the edge is dropped). The DFS visited set additionally
    // guarantees no infinite loop if the dropping logic ever regresses.
    let mut sink = RecordingSink::new();
    sink.record(make_event(7, DecisionSite::Banner, CategoryId(1), Some(7)));
    let report = sink.into_report();
    assert_eq!(report.cascade_chains.len(), 1);
    assert_eq!(report.cascade_chains[0].root_event, 7);
    assert_eq!(report.cascade_chains[0].depth, 0);
    assert_eq!(report.max_cascade_depth, 0);
}

#[test]
fn recording_sink_records_in_order() {
    let mut sink = RecordingSink::new();
    for step in 0..10 {
        sink.record(make_event(
            step,
            DecisionSite::Document,
            CategoryId(1),
            None,
        ));
    }
    let events = sink.events();
    assert_eq!(events.len(), 10);
    for (i, event) in events.iter().enumerate() {
        assert_eq!(event.step, i as u32);
    }
}

#[test]
fn project_with_sink_default_delegates_and_emits_nothing() {
    // Minimal MarkingScheme impl: every required method is a stub.
    // The Phase B contract under test is the default-delegating
    // body of `project_with_sink` — it MUST call `project` and MUST
    // NOT touch the sink.
    use crate::ambiguity::Parsed;
    use crate::scheme::MarkingScheme;
    use crate::scope::Scope;

    #[derive(Debug)]
    struct StubErr;
    impl core::fmt::Display for StubErr {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("StubErr")
        }
    }
    impl std::error::Error for StubErr {}

    struct MinimalScheme;
    impl MarkingScheme for MinimalScheme {
        type Token = u32;
        type Marking = u8;
        type ParseError = StubErr;
        type OpenVocabRef = core::convert::Infallible;
        type Parsed<'src> = ();
        type Canonical = ();
        type Projected = ();

        fn name(&self) -> &str {
            "minimal"
        }
        fn schema_version(&self) -> &str {
            "0"
        }
        fn categories(&self) -> &[crate::category::Category] {
            &[]
        }
        fn constraints(&self) -> &[crate::constraint::Constraint] {
            &[]
        }
        fn templates(&self) -> &[crate::template::Template] {
            &[]
        }
        fn parse(&self, _: &str) -> Result<Parsed<u8>, StubErr> {
            Ok(Parsed::Ambiguous {
                candidates: Vec::new(),
            })
        }
        fn project(&self, _scope: Scope, markings: &[u8]) -> u8 {
            markings.iter().copied().fold(0, u8::saturating_add)
        }
        fn render_canonical(
            &self,
            _m: &u8,
            _ctx: &crate::RenderContext,
            _out: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
        }
    }

    let scheme = MinimalScheme;
    let markings = [3u8, 4u8, 5u8];
    let expected = scheme.project(Scope::Page, &markings);

    let mut sink = RecordingSink::new();
    let actual = scheme.project_with_sink(Scope::Page, &markings, &mut sink);

    assert_eq!(actual, expected);
    assert_eq!(sink.events().len(), 0);

    // Symmetric contract: `closure_with_sink` must also default-delegate
    // to `closure` and emit no events. Same MinimalScheme; the default
    // `closure` is a no-op so the assertion is that the marking
    // round-trips and the sink stays empty.
    let mut closure_sink = RecordingSink::new();
    let closed = scheme.closure_with_sink(42u8, &mut closure_sink);
    assert_eq!(closed, scheme.closure(42u8));
    assert_eq!(closure_sink.events().len(), 0);
}

fn make_event(
    step: u32,
    site: DecisionSite,
    category: CategoryId,
    triggered_by: Option<u32>,
) -> DecisionEvent {
    DecisionEvent {
        step,
        site,
        category,
        kind: DecisionKind::Evaluated,
        source: DecisionSource::Parser,
        triggered_by,
    }
}
