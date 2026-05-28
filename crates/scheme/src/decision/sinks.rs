// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Concrete [`DecisionSink`] implementations.
//!
//! - [`NoopSink`] — zero-sized, zero-cost; the engine's default.
//! - [`CountingSink`] — running tallies, no per-event allocation.
//! - [`RecordingSink`] — full event stream, cascade-chain reconstruction.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::category::CategoryId;
use crate::decision::report::{CascadeChain, DecisionReport};
use crate::decision::{DecisionEvent, DecisionKind, DecisionSink};

/// Number of [`DecisionKind`] variants, used to size [`CountingSink::by_kind`].
///
/// MUST stay in sync with the [`DecisionKind`] enum definition. Adding a
/// variant requires bumping this constant and extending
/// [`discriminant_index`].
const DECISION_KIND_COUNT: usize = 8;

/// Map a [`DecisionKind`] to a dense `usize` index in `0..DECISION_KIND_COUNT`.
///
/// Used as the fixed-size index into [`CountingSink::by_kind`]. The map
/// MUST be total over the enum (every variant has a distinct index in
/// range) and MUST agree with `DECISION_KIND_COUNT`. The compiler checks
/// totality via the exhaustive match.
#[inline]
const fn discriminant_index(k: DecisionKind) -> usize {
    match k {
        DecisionKind::Evaluated => 0,
        DecisionKind::EvaluatedSubstantive => 1,
        DecisionKind::Mutated => 2,
        DecisionKind::ConstraintFired => 3,
        DecisionKind::RewriteScheduled => 4,
        DecisionKind::RewriteApplied => 5,
        DecisionKind::ClosureFired => 6,
        DecisionKind::Recanonicalized => 7,
    }
}

/// Inverse of [`discriminant_index`]: ordered list of every variant in
/// index order. Used by [`CountingSink::into_report`] to convert the
/// dense `by_kind` array into a `BTreeMap` keyed by variant.
const KIND_ORDER: [DecisionKind; DECISION_KIND_COUNT] = [
    DecisionKind::Evaluated,
    DecisionKind::EvaluatedSubstantive,
    DecisionKind::Mutated,
    DecisionKind::ConstraintFired,
    DecisionKind::RewriteScheduled,
    DecisionKind::RewriteApplied,
    DecisionKind::ClosureFired,
    DecisionKind::Recanonicalized,
];

/// Zero-sized [`DecisionSink`] that discards every event.
///
/// The engine's default. `NoopSink::record` is `#[inline(always)]` and
/// has an empty body — the optimizer collapses every call site to no
/// instructions, so threading `&mut NoopSink` through the engine adds
/// no measurable cost on the hot path. Constitution I (uncompromising
/// performance) is what makes this the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NoopSink;

impl NoopSink {
    /// Construct a fresh [`NoopSink`]. The constructor exists for
    /// symmetry with the other sinks; the type is ZST so any value
    /// is equivalent.
    #[inline(always)]
    pub const fn new() -> Self {
        Self
    }
}

impl DecisionSink for NoopSink {
    #[inline(always)]
    fn record(&mut self, _event: DecisionEvent) {}
}

/// [`DecisionSink`] that accumulates per-kind, per-category, and
/// per-portion counts without retaining the individual events.
///
/// Intended for "how busy was the engine?" reporting: total decisions,
/// breakdown by kind (parser dispatches vs constraint firings vs page
/// rewrites), breakdown by category (classification axis vs SCI vs
/// dissem), and breakdown by portion index. No allocation per
/// recorded event after the first time a new category or higher
/// portion index is seen.
#[derive(Debug, Clone)]
pub struct CountingSink {
    /// Running total across all recorded events.
    total: u64,
    /// Dense per-[`DecisionKind`] counts. Indexed via
    /// [`discriminant_index`].
    by_kind: [u64; DECISION_KIND_COUNT],
    /// Per-[`CategoryId`] counts. Sparse; only categories that
    /// appeared have entries.
    by_category: BTreeMap<CategoryId, u64>,
    /// Per-portion counts indexed by portion number. Grows by
    /// extension when a higher portion index is observed; positions
    /// for portions never seen carry zero.
    by_portion: Vec<u64>,
}

impl CountingSink {
    /// Construct an empty [`CountingSink`] with all counters at zero.
    #[inline]
    pub fn new() -> Self {
        Self {
            total: 0,
            by_kind: [0; DECISION_KIND_COUNT],
            by_category: BTreeMap::new(),
            by_portion: Vec::new(),
        }
    }

    /// Convert the accumulated counts into a [`DecisionReport`].
    ///
    /// The returned report's `cascade_chains` is empty and
    /// `max_cascade_depth` is zero — chain reconstruction requires the
    /// full event stream that a counting sink doesn't retain. Use
    /// [`RecordingSink::into_report`] when chains are needed.
    pub fn into_report(self) -> DecisionReport {
        let by_kind: BTreeMap<DecisionKind, u64> = KIND_ORDER
            .iter()
            .copied()
            .zip(self.by_kind)
            .filter(|(_, count)| *count != 0)
            .collect();
        DecisionReport {
            total: self.total,
            by_category: self.by_category,
            by_kind,
            by_portion: self.by_portion,
            cascade_chains: Vec::new(),
            max_cascade_depth: 0,
        }
    }
}

impl Default for CountingSink {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionSink for CountingSink {
    fn record(&mut self, event: DecisionEvent) {
        self.total = self.total.saturating_add(1);
        self.by_kind[discriminant_index(event.kind)] =
            self.by_kind[discriminant_index(event.kind)].saturating_add(1);
        self.by_category
            .entry(event.category)
            .and_modify(|c| *c = c.saturating_add(1))
            .or_insert(1);
        if let crate::decision::DecisionSite::Portion(idx) = event.site {
            let needed = idx as usize + 1;
            if self.by_portion.len() < needed {
                self.by_portion.resize(needed, 0);
            }
            self.by_portion[idx as usize] = self.by_portion[idx as usize].saturating_add(1);
        }
    }
}

/// [`DecisionSink`] that retains every event verbatim.
///
/// Use for the full-trace path: rule-by-rule replay, cascade-chain
/// reconstruction, debugging the engine's decision sequence on a
/// single document. Each event is 56 bytes (pinned by
/// `const_assert_eq!` in the decision tests); ten thousand events fit
/// in roughly half a megabyte.
#[derive(Debug, Clone)]
pub struct RecordingSink {
    events: Vec<DecisionEvent>,
}

impl RecordingSink {
    /// Construct an empty [`RecordingSink`].
    #[inline]
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Construct an empty [`RecordingSink`] with pre-allocated
    /// capacity for `n` events.
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        Self {
            events: Vec::with_capacity(n),
        }
    }

    /// Borrow the recorded event stream in record order.
    #[inline]
    pub fn events(&self) -> &[DecisionEvent] {
        &self.events
    }

    /// Build a [`DecisionReport`] from a caller-supplied event vector
    /// without going through a [`RecordingSink`] instance.
    ///
    /// Phase E `marque trace` uses this to bridge the
    /// `Arc<Mutex<Vec<DecisionEvent>>>` capture pattern (the same one
    /// the Phase C smoke tests use) into the report-with-cascade-chains
    /// surface that the CLI's `summary` and `narrate` formats consume.
    /// Identical semantics to [`RecordingSink::into_report`] —
    /// implementation is a 1-line forward into a temporary
    /// [`RecordingSink`].
    pub fn into_report_from_events(events: Vec<DecisionEvent>) -> DecisionReport {
        Self { events }.into_report()
    }

    /// Convert the recorded event stream into a [`DecisionReport`].
    ///
    /// Walks [`DecisionEvent::triggered_by`] edges to reconstruct
    /// cascade chains: every event with `triggered_by == None` is a
    /// root, and the chain rooted at that event contains every
    /// transitively-triggered descendant. `depth` is the longest
    /// path from the root to a leaf in the chain (root edge = 0;
    /// root + one child = 1; etc.).
    ///
    /// Events whose `triggered_by` points to an unknown step (no
    /// event with that step number is present in the stream) are
    /// treated as roots — this is defensive against partial
    /// captures where the parent event was dropped or never
    /// emitted.
    pub fn into_report(self) -> DecisionReport {
        let events = self.events;

        // Aggregate the same counts CountingSink produces.
        let mut total: u64 = 0;
        let mut by_kind_dense = [0u64; DECISION_KIND_COUNT];
        let mut by_category: BTreeMap<CategoryId, u64> = BTreeMap::new();
        let mut by_portion: Vec<u64> = Vec::new();
        for event in &events {
            total = total.saturating_add(1);
            by_kind_dense[discriminant_index(event.kind)] =
                by_kind_dense[discriminant_index(event.kind)].saturating_add(1);
            by_category
                .entry(event.category)
                .and_modify(|c| *c = c.saturating_add(1))
                .or_insert(1);
            if let crate::decision::DecisionSite::Portion(idx) = event.site {
                let needed = idx as usize + 1;
                if by_portion.len() < needed {
                    by_portion.resize(needed, 0);
                }
                by_portion[idx as usize] = by_portion[idx as usize].saturating_add(1);
            }
        }
        let by_kind: BTreeMap<DecisionKind, u64> = KIND_ORDER
            .iter()
            .copied()
            .zip(by_kind_dense)
            .filter(|(_, count)| *count != 0)
            .collect();

        // Index events by step so children can find their parent's
        // site without a linear scan.
        let mut by_step: HashMap<u32, usize> = HashMap::with_capacity(events.len());
        for (idx, event) in events.iter().enumerate() {
            by_step.insert(event.step, idx);
        }

        // Build the parent → children adjacency map. An event whose
        // `triggered_by` points to a step we don't have, or to its own
        // step, is treated as a root (root_candidates picks it up
        // below). Dropping self-edges here keeps the adjacency map
        // acyclic at the source — the DFS visited set below is a
        // belt-and-braces guard against any other cycle a future
        // refactor might introduce.
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for event in &events {
            if let Some(parent) = event.triggered_by
                && parent != event.step
                && by_step.contains_key(&parent)
            {
                children.entry(parent).or_default().push(event.step);
            }
        }

        // Sort each child list so DFS traversal is deterministic.
        for child_list in children.values_mut() {
            child_list.sort_unstable();
        }

        // Identify roots: events with no parent, whose parent is not
        // present in the recorded stream, or whose parent is themselves
        // (the self-edge was dropped above; the event still needs to
        // surface in a chain).
        let mut roots: Vec<u32> = events
            .iter()
            .filter(|e| match e.triggered_by {
                None => true,
                Some(parent) => parent == e.step || !by_step.contains_key(&parent),
            })
            .map(|e| e.step)
            .collect();
        roots.sort_unstable();

        // DFS each root, collecting steps in pre-order traversal
        // and tracking max depth. A visited set guards against
        // self-referential events (`triggered_by == Some(own_step)`)
        // and any other cycle that would otherwise loop forever.
        let mut cascade_chains: Vec<CascadeChain> = Vec::with_capacity(roots.len());
        let mut max_cascade_depth: u32 = 0;
        let mut visited: HashSet<u32> = HashSet::with_capacity(events.len());
        for root in roots {
            let root_idx = by_step[&root];
            let root_site = events[root_idx].site;
            let mut chain_events: Vec<u32> = Vec::new();
            let mut depth: u32 = 0;
            // Stack carries (step, depth_from_root).
            let mut stack: Vec<(u32, u32)> = vec![(root, 0)];
            while let Some((step, d)) = stack.pop() {
                if !visited.insert(step) {
                    continue;
                }
                chain_events.push(step);
                if d > depth {
                    depth = d;
                }
                if let Some(child_steps) = children.get(&step) {
                    // Reverse-push so the smallest step is processed
                    // first (matches the sorted child order on pop).
                    for child in child_steps.iter().rev() {
                        stack.push((*child, d + 1));
                    }
                }
            }
            if depth > max_cascade_depth {
                max_cascade_depth = depth;
            }
            cascade_chains.push(CascadeChain {
                root_event: root,
                root_site,
                events: chain_events,
                depth,
            });
        }

        DecisionReport {
            total,
            by_category,
            by_kind,
            by_portion,
            cascade_chains,
            max_cascade_depth,
        }
    }
}

impl Default for RecordingSink {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionSink for RecordingSink {
    fn record(&mut self, event: DecisionEvent) {
        self.events.push(event);
    }
}
