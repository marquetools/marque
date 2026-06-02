// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Page-rewrite and derivation-edge scheduler.
//!
//! `schedule_steps` runs a single Kahn's-algorithm pass over the union
//! of the scheme's declared `PageRewrite`s and `DerivationEdge`s, using
//! each node's `reads` / `writes` axes to produce a deterministic
//! topological order — a node that writes category `X` runs before any
//! node that reads `X`. Cycles, unannotated `Custom` rewrites, and
//! ambiguous co-writers abort construction with
//! [`EngineConstructionError`](crate::errors::EngineConstructionError).
//!
//! `schedule_rewrites` is a rewrites-only convenience that delegates to
//! `schedule_steps` with no edges and projects the rewrite steps back
//! out — the order it returns is the rewrite-step subsequence of the
//! combined order.
//!
//! This runs once at [`Engine::new`] and its output is cached on the
//! engine instance; per-document rewrite evaluation walks the
//! pre-computed order without re-sorting.

use marque_scheme::{
    ApplyIntentError, CategoryAction, CategoryId, CategoryPredicate, DerivationEdge, EdgeId,
    FactRef, MarkingScheme, PageRewrite, ReplacementIntent, RewriteId,
};
use std::collections::{BTreeMap, BTreeSet};

use crate::errors::EngineConstructionError;

/// A node in the combined rewrite + derivation-edge schedule.
///
/// The discriminant is load-bearing: [`RewriteId`] and [`EdgeId`] are
/// both `&'static str`, so an untagged order could conflate a rewrite
/// and an edge that happen to share an id string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledStep {
    /// A page-rewrite step, identified by its [`RewriteId`].
    PageRewrite(RewriteId),
    /// A derivation-edge step, identified by its [`EdgeId`].
    DerivationEdge(EdgeId),
}

/// Validates every [`CategoryAction::Intent`] in the scheme's
/// page-rewrites table by walking each intent's [`FactRef`]s and
/// confirming the scheme can route each one via
/// [`MarkingScheme::category_of`].
///
/// Returns
/// [`EngineConstructionError::InvalidIntentInPageRewrite`](crate::errors::EngineConstructionError::InvalidIntentInPageRewrite)
/// on the first unroutable `FactRef` found. Downstream `project()`
/// calls can then trust that every `CategoryAction::Intent` they
/// encounter is well-formed at engine-construction time, even though
/// the runtime executor still handles per-intent errors defensively
/// (Constitution VI: `Engine::lint`'s hot path must not unwind into
/// Tower middleware).
///
/// Both `FactRef::Cve` and `FactRef::OpenVocab` references are checked
/// uniformly: every scheme that implements
/// [`MarkingScheme::category_of`] handles both variants, so the
/// validation pass is symmetric.
///
/// Per-intent walk:
///
/// - `FactAdd { token, .. }` validates `token`.
/// - `FactRemove { facts, .. }` validates every `FactRef` in `facts`.
/// - `Recanonicalize { .. }` carries no `FactRef`; nothing to validate.
pub(crate) fn validate_intent_rewrites<S>(
    scheme: &S,
    rewrites: &[PageRewrite<S>],
) -> Result<(), EngineConstructionError>
where
    S: MarkingScheme,
{
    for rw in rewrites {
        if let CategoryAction::Intent(intent) = &rw.action {
            for fact in intent_fact_refs(intent) {
                if scheme.category_of(fact).is_none() {
                    return Err(EngineConstructionError::InvalidIntentInPageRewrite {
                        rewrite_id: rw.id,
                        fact_label: format!("{fact:?}"),
                        error: ApplyIntentError::UnknownToken,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Walk every [`FactRef`] inside a [`ReplacementIntent`].
///
/// Returned as a `Vec<&FactRef<S>>` rather than an `impl Iterator` so
/// the implementation stays readable without pulling in the `either`
/// crate. `FactAdd` contributes one fact; `FactRemove` contributes
/// one or more (SmallVec `[_; 2]` inline capacity covers the
/// single-fact common case + the atomic-cluster pair); `Recanonicalize`
/// contributes none.
fn intent_fact_refs<S>(intent: &ReplacementIntent<S>) -> Vec<&FactRef<S>>
where
    S: MarkingScheme + ?Sized,
{
    match intent {
        ReplacementIntent::FactAdd { token, .. } => vec![token],
        ReplacementIntent::FactRemove { facts, .. } => facts.iter().collect(),
        // No FactRefs to validate; the renderer handles
        // recanonicalization at fix-application time.
        ReplacementIntent::Recanonicalize { .. } => Vec::new(),
        // `ReplacementIntent` is `#[non_exhaustive]`. A future variant
        // that introduces new `FactRef`s MUST be handled explicitly
        // here so engine-construction-time validation covers it.
        // `unreachable!()` is safe at this site — `intent_fact_refs`
        // is called from `Engine::new`'s validation pass, not from
        // the `lint`/`fix` hot path, so a panic surfaces at startup
        // (the correct loud-failure surface) rather than mid-request.
        _ => unreachable!(
            "intent_fact_refs: new ReplacementIntent variant not handled — \
             add an explicit match arm and update validate_intent_rewrites \
             coverage before shipping the variant",
        ),
    }
}

/// Compute the topological order of `rewrites` by their `reads` /
/// `writes` axes — a rewrites-only convenience over [`schedule_steps`].
///
/// Delegates to `schedule_steps` with no derivation edges and projects
/// the rewrite steps back out. The returned order is byte-identical to
/// the rewrite-step subsequence of the combined order; for a scheme
/// with no edges that is the whole order.
///
/// Returns the ordered list of `RewriteId`s. Rewrites that have no
/// predecessor (neither read nor write a category another rewrite
/// writes) retain their declaration order relative to each other.
/// Dataflow edges fully determine the order for cycle-free inputs; for
/// rewrites with no edge between them, the only stable answer is
/// declaration order.
///
/// # Errors
///
/// Propagates every error [`schedule_steps`] can raise — see its docs.
pub fn schedule_rewrites<S>(
    rewrites: &[PageRewrite<S>],
) -> Result<Box<[RewriteId]>, EngineConstructionError>
where
    S: MarkingScheme + ?Sized,
{
    schedule_steps(rewrites, &[]).map(|order| project_rewrites(&order))
}

/// The rewrites-only projection of a combined schedule.
///
/// Keeps every [`ScheduledStep::PageRewrite`] step, in combined-pass
/// order, dropping derivation-edge steps. This is the single source of
/// truth for the cached rewrite order: deriving it from the union pass
/// (rather than a separate edge-free sort) means an edge-bearing
/// scheme's rewrite order honestly reflects edge-induced transitive
/// ordering instead of two orders that could disagree.
pub(crate) fn project_rewrites(order: &[ScheduledStep]) -> Box<[RewriteId]> {
    order
        .iter()
        .filter_map(|step| match step {
            ScheduledStep::PageRewrite(id) => Some(*id),
            ScheduledStep::DerivationEdge(_) => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

/// Compute the topological order of the union of `rewrites` and
/// `edges` by their `reads` / `writes` axes.
///
/// The combined graph treats each page rewrite and each derivation edge
/// as a node. A node that writes category `X` runs before any node that
/// reads `X` (writers-before-readers). Node identity is tagged via
/// [`ScheduledStep`] so a rewrite and an edge that share an id string
/// stay distinct.
///
/// The edge topology is static: every declared edge is scheduled and
/// validated here regardless of its [`marque_scheme::FiringPredicate`].
/// Firing only gates whether an edge runs at evaluation time, never
/// whether it participates in scheduling — so a mode-gated edge is
/// cycle-checked and co-writer-checked identically to an always-firing
/// one.
///
/// # Errors
///
/// - [`EngineConstructionError::UnannotatedCustomAxes`] if any
///   rewrite with a `Custom` trigger or action has empty `reads` or
///   `writes`. Declarative rewrites (`Contains` / `Empty` triggers
///   with `Clear` / `Replace` / `Promote` actions) are permitted to
///   have empty annotations — the scheduler treats them as "no
///   dataflow dependency" rather than as an authoring bug. Derivation
///   edges carry no `Custom` variant, so this check never applies to
///   them.
/// - [`EngineConstructionError::AmbiguousCoWriter`] if a derivation
///   edge co-writes a category with another node but no read forces a
///   deterministic order between them (a stale-value read hazard).
/// - [`EngineConstructionError::RewriteCycle`] if the axis graph
///   contains a cycle. All members participating in the cycle are
///   reported, not just the entry point.
pub fn schedule_steps<S>(
    rewrites: &[PageRewrite<S>],
    edges: &[DerivationEdge],
) -> Result<Box<[ScheduledStep]>, EngineConstructionError>
where
    S: MarkingScheme + ?Sized,
{
    // 1. Enforce the custom-annotation invariant on rewrites. Edges
    //    have no `Custom` variant, so they are exempt by construction.
    for rw in rewrites {
        let has_custom = rewrite_is_custom(rw);
        if has_custom && (rw.reads.is_empty() || rw.writes.is_empty()) {
            return Err(EngineConstructionError::UnannotatedCustomAxes { rewrite: rw.id });
        }
    }

    // Unified index space: `0..r` are rewrites (declaration order),
    // `r..r+e` are edges (declaration order).
    let r = rewrites.len();
    let n = r + edges.len();

    // 2. Reject ambiguous co-writers before building the graph, so the
    //    actionable per-axis error surfaces ahead of a graph-shaped
    //    cycle error (mirrors the validate-first construction ordering).
    reject_ambiguous_cowriters(rewrites, edges, r, n)?;

    // 3. Build the dependency graph: `edge(a, b)` iff `a` writes a
    //    category `b` reads. `a` must run before `b`. Keep the
    //    per-node adjacency list as an ordered set so the traversal is
    //    deterministic across runs.
    let mut in_degree: Vec<usize> = vec![0; n];
    let mut successors: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];

    // Map each category to the nodes that write it. `BTreeMap` for
    // stable iteration order.
    let mut writers: BTreeMap<CategoryId, Vec<usize>> = BTreeMap::new();
    for idx in 0..n {
        for w in node_writes(rewrites, edges, r, idx) {
            writers.entry(*w).or_default().push(idx);
        }
    }

    // `idx` spans the unified index space (rewrites then edges) and is
    // used both as a slice index and as a `node_reads` accessor key, so
    // a single-slice `enumerate` does not apply here.
    #[allow(clippy::needless_range_loop)]
    for idx in 0..n {
        for read_cat in node_reads(rewrites, edges, r, idx) {
            let Some(producers) = writers.get(read_cat) else {
                continue;
            };
            for &producer_idx in producers {
                if producer_idx == idx {
                    // A node that reads and writes the same category is
                    // a self-edge; don't count it.
                    continue;
                }
                // Producer must run before consumer: producer_idx → idx.
                if successors[producer_idx].insert(idx) {
                    in_degree[idx] += 1;
                }
            }
        }
    }

    // 4. Kahn's algorithm. Seed the frontier with indexes that have
    //    in-degree 0, preserving declaration order (rewrites ahead of
    //    edges on ties, since rewrites occupy the lower index range).
    let mut frontier: std::collections::VecDeque<usize> =
        (0..n).filter(|i| in_degree[*i] == 0).collect();
    let mut scheduled: Vec<ScheduledStep> = Vec::with_capacity(n);
    while let Some(idx) = frontier.pop_front() {
        scheduled.push(node_step(rewrites, edges, r, idx));
        // Iterate successors in declaration order (BTreeSet iterates in
        // sorted order; indexes are declaration-ordered).
        for &succ in &successors[idx] {
            in_degree[succ] -= 1;
            if in_degree[succ] == 0 {
                frontier.push_back(succ);
            }
        }
    }

    if scheduled.len() != n {
        // Cycle detected. Extract the actual cycle participants via
        // Tarjan's SCC so downstream-blocked nodes (nodes that have
        // in-degree > 0 only because a cycle upstream never resolves)
        // are excluded from the reported `members` list. Self-edges (a
        // node that reads and writes category `X`) are explicitly NOT
        // counted as cycles above; a size-1 SCC without a self-successor
        // is a downstream-blocked node, not a cycle member.
        //
        // When multiple disjoint cycles exist, report **one** of them —
        // the one containing the lowest-index node in declaration order.
        // Flattening all cycles into a single `members` list would
        // produce an error whose `axis` names one cycle and whose member
        // set mixes unrelated nodes, which is worse than naming just one
        // cycle authoritatively. The author fixes this cycle, re-runs,
        // and the next cycle (if any) surfaces on the next attempt.
        let sccs = tarjan_sccs(n, &successors);
        let mut cycle_sccs: Vec<Vec<usize>> = sccs
            .into_iter()
            .filter(|scc| {
                // Size-1 SCCs reach this code only through downstream
                // blocking — the self-loop case (a node reading and
                // writing the same category) is skipped earlier when
                // building `successors`, so a singleton SCC never
                // contains a true self-edge.
                scc.len() > 1
            })
            .collect();
        debug_assert!(
            !cycle_sccs.is_empty(),
            "scheduled.len() != n but Tarjan found no non-trivial SCC; \
             this indicates a logic error in `schedule_steps` or in \
             `tarjan_sccs`, because Kahn's algorithm only leaves nodes \
             unscheduled when the residual graph contains a cycle."
        );
        // Pick the SCC that contains the node with the lowest
        // declaration index — deterministic across runs.
        let picked = cycle_sccs
            .iter_mut()
            .min_by_key(|scc| scc.iter().min().copied().unwrap_or(usize::MAX))
            .expect("debug_assert above guards the empty-Vec case");
        picked.sort_unstable();
        let axis = cycle_axis(rewrites, edges, r, picked);
        let members: Box<[ScheduledStep]> = picked
            .iter()
            .map(|&i| node_step(rewrites, edges, r, i))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        return Err(EngineConstructionError::RewriteCycle { axis, members });
    }

    Ok(scheduled.into_boxed_slice())
}

/// The `reads` axes of node `i` in the unified index space.
///
/// Node `i` is rewrite `i` when `i < r`, otherwise edge `i - r`. Both
/// payload types are `&'static [CategoryId]`.
fn node_reads<'a, S>(
    rewrites: &'a [PageRewrite<S>],
    edges: &'a [DerivationEdge],
    r: usize,
    i: usize,
) -> &'static [CategoryId]
where
    S: MarkingScheme + ?Sized,
{
    if i < r {
        rewrites[i].reads
    } else {
        edges[i - r].reads
    }
}

/// The `writes` axes of node `i` in the unified index space.
fn node_writes<'a, S>(
    rewrites: &'a [PageRewrite<S>],
    edges: &'a [DerivationEdge],
    r: usize,
    i: usize,
) -> &'static [CategoryId]
where
    S: MarkingScheme + ?Sized,
{
    if i < r {
        rewrites[i].writes
    } else {
        edges[i - r].writes
    }
}

/// The tagged [`ScheduledStep`] for node `i` in the unified index space.
fn node_step<S>(
    rewrites: &[PageRewrite<S>],
    edges: &[DerivationEdge],
    r: usize,
    i: usize,
) -> ScheduledStep
where
    S: MarkingScheme + ?Sized,
{
    if i < r {
        ScheduledStep::PageRewrite(rewrites[i].id)
    } else {
        ScheduledStep::DerivationEdge(edges[i - r].id)
    }
}

/// Reject any derivation edge that co-writes a category with another
/// node when no read forces a deterministic order between them.
///
/// Edge-scoped: the guard fires only for an unordered co-writing pair
/// in which at least one node is a derivation edge. Two page rewrites
/// that co-write the same category with no read between them are
/// tolerated (declaration order wins) exactly as before — a rewrite↔
/// rewrite hazard would otherwise regress every multi-rewrite scheme.
///
/// For distinct nodes `A`, `B` with `Shared = A.writes ∩ B.writes`:
///
/// - `Shared = ∅` → not a hazard.
/// - `Shared ∩ B.reads ≠ ∅` or `Shared ∩ A.reads ≠ ∅` → an explicit
///   read forces a deterministic order, so no stale read.
/// - otherwise → two producers of a shared category with no read
///   forcing order; the final value depends on an arbitrary
///   declaration-order tiebreak. Reject with
///   [`EngineConstructionError::AmbiguousCoWriter`].
///
/// Only the annotation-inconsistency form is detectable here: the guard
/// cannot detect an edge whose body consumes a category absent from
/// both its `reads` and `writes`, because the scheduler does not
/// introspect edge bodies.
fn reject_ambiguous_cowriters<S>(
    rewrites: &[PageRewrite<S>],
    edges: &[DerivationEdge],
    r: usize,
    n: usize,
) -> Result<(), EngineConstructionError>
where
    S: MarkingScheme + ?Sized,
{
    for a in 0..n {
        for b in (a + 1)..n {
            // Edge-scoped: at least one node of the pair must be an edge.
            let one_is_edge = a >= r || b >= r;
            if !one_is_edge {
                continue;
            }
            let a_writes: BTreeSet<CategoryId> =
                node_writes(rewrites, edges, r, a).iter().copied().collect();
            if a_writes.is_empty() {
                continue;
            }
            let b_writes: BTreeSet<CategoryId> =
                node_writes(rewrites, edges, r, b).iter().copied().collect();
            let shared: BTreeSet<CategoryId> = a_writes.intersection(&b_writes).copied().collect();
            if shared.is_empty() {
                continue;
            }
            let a_reads: BTreeSet<CategoryId> =
                node_reads(rewrites, edges, r, a).iter().copied().collect();
            let b_reads: BTreeSet<CategoryId> =
                node_reads(rewrites, edges, r, b).iter().copied().collect();
            let ordered = shared.intersection(&b_reads).next().is_some()
                || shared.intersection(&a_reads).next().is_some();
            if ordered {
                continue;
            }
            // Pick the lowest shared category for deterministic
            // reporting; both nodes are reported as the co-writing pair.
            let axis = shared
                .iter()
                .next()
                .copied()
                .expect("shared is non-empty by the guard above");
            let nodes: Box<[ScheduledStep]> = Box::new([
                node_step(rewrites, edges, r, a),
                node_step(rewrites, edges, r, b),
            ]);
            return Err(EngineConstructionError::AmbiguousCoWriter { axis, nodes });
        }
    }
    Ok(())
}

/// Tarjan's strongly-connected-components algorithm.
///
/// Returns one `Vec<usize>` per SCC. A graph edge `u → v` is encoded
/// as `successors[u].contains(&v)`. The algorithm is deterministic:
/// it walks `successors` in `BTreeSet` order (sorted) so SCCs with
/// identical node sets are grouped identically across runs.
///
/// Size-1 SCCs without a self-edge are not cycles — they are nodes
/// the caller may choose to filter out based on the graph's semantics
/// (the scheduler above filters on `scc.len() > 1` because `successors`
/// has already had self-edges stripped).
///
/// The implementation is iterative to avoid stack-overflow on
/// pathological inputs — `CapcoScheme` has ≤10 rewrites today, but a
/// future CUI / NATO scheme may declare more.
fn tarjan_sccs(n: usize, successors: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    // Per-node state.
    let mut index: Vec<Option<usize>> = vec![None; n];
    let mut lowlink: Vec<usize> = vec![0; n];
    let mut on_stack: Vec<bool> = vec![false; n];
    let mut scc_stack: Vec<usize> = Vec::new();
    let mut next_index: usize = 0;
    let mut sccs: Vec<Vec<usize>> = Vec::new();

    // DFS-frame stack: (node, iterator-position into that node's
    // successor list). We iterate successors as a Vec snapshot so we
    // can pause and resume at a given index without borrowing the
    // source BTreeSet across frames.
    struct Frame {
        node: usize,
        successors: Vec<usize>,
        pos: usize,
    }
    let mut dfs: Vec<Frame> = Vec::new();

    for start in 0..n {
        if index[start].is_some() {
            continue;
        }

        // Seed.
        index[start] = Some(next_index);
        lowlink[start] = next_index;
        next_index += 1;
        scc_stack.push(start);
        on_stack[start] = true;
        dfs.push(Frame {
            node: start,
            successors: successors[start].iter().copied().collect(),
            pos: 0,
        });

        while let Some(frame) = dfs.last_mut() {
            if frame.pos < frame.successors.len() {
                let w = frame.successors[frame.pos];
                frame.pos += 1;
                if index[w].is_none() {
                    // Descend into w.
                    index[w] = Some(next_index);
                    lowlink[w] = next_index;
                    next_index += 1;
                    scc_stack.push(w);
                    on_stack[w] = true;
                    dfs.push(Frame {
                        node: w,
                        successors: successors[w].iter().copied().collect(),
                        pos: 0,
                    });
                } else if on_stack[w] {
                    let v = frame.node;
                    let w_idx = index[w].expect("index[w] was set when w was pushed");
                    lowlink[v] = lowlink[v].min(w_idx);
                }
            } else {
                // Frame exhausted — pop and emit SCC if v is the root.
                let v = frame.node;
                dfs.pop();
                if let Some(parent) = dfs.last_mut() {
                    lowlink[parent.node] = lowlink[parent.node].min(lowlink[v]);
                }
                let v_index = index[v].expect("index[v] was set at seed");
                if lowlink[v] == v_index {
                    let mut component = Vec::new();
                    while let Some(w) = scc_stack.pop() {
                        on_stack[w] = false;
                        component.push(w);
                        if w == v {
                            break;
                        }
                    }
                    sccs.push(component);
                }
            }
        }
    }

    sccs
}

/// Does the rewrite's trigger or action contain a `Custom` variant?
///
/// `Custom` variants carry function pointers opaque to the scheduler,
/// so their dataflow cannot be derived from the variant itself —
/// callers must annotate `reads` / `writes` explicitly. Declarative
/// variants can safely elide annotations because the category is
/// carried in the variant payload.
fn rewrite_is_custom<S: MarkingScheme + ?Sized>(rw: &PageRewrite<S>) -> bool {
    // Match on explicit references so the predicate stays correct
    // even if a future variant introduces non-`Copy` payloads that
    // default-binding-mode ergonomics won't cover.
    let trigger_is_custom = matches!(&rw.trigger, CategoryPredicate::Custom(_));
    let action_is_custom = match &rw.action {
        CategoryAction::Custom(_) => true,
        // `Intent` carries a data-shaped `ReplacementIntent` whose
        // category routing is statically validated at `Engine::new`;
        // its `reads` / `writes` annotations are author-declared via
        // `PageRewrite::declarative`, just like other declarative
        // actions. Treat it as non-custom so empty axes are tolerated.
        CategoryAction::Intent(_) => false,
        CategoryAction::Clear { .. }
        | CategoryAction::Replace { .. }
        | CategoryAction::Promote { .. } => false,
    };
    trigger_is_custom || action_is_custom
}

/// Pick a category from the cycle to name in the error.
///
/// Any category that appears on both sides of the cycle is a valid
/// answer. We pick the lowest category id that's both read and
/// written by some member of the cycle for deterministic reporting.
/// `indexes` are positions in the unified index space (rewrites then
/// edges), read via the `node_reads` / `node_writes` accessors.
///
/// The intersection is guaranteed non-empty when `indexes` names a
/// real cycle — every edge `a → b` in the scheduler's graph exists
/// because some `c ∈ a.writes ∩ b.reads`, and a cycle requires at
/// least one such shared category. The `debug_assert!` below catches
/// a broken invariant loudly in tests and debug builds; the release
/// fallback returns `CategoryId(0)` only as a last-resort defense
/// against a future refactor that calls this helper with a
/// non-cyclic index set.
fn cycle_axis<S: MarkingScheme + ?Sized>(
    rewrites: &[PageRewrite<S>],
    edges: &[DerivationEdge],
    r: usize,
    indexes: &[usize],
) -> CategoryId {
    let mut reads: BTreeSet<CategoryId> = BTreeSet::new();
    let mut writes: BTreeSet<CategoryId> = BTreeSet::new();
    for &i in indexes {
        for cat in node_reads(rewrites, edges, r, i) {
            reads.insert(*cat);
        }
        for cat in node_writes(rewrites, edges, r, i) {
            writes.insert(*cat);
        }
    }
    let picked = reads.intersection(&writes).next().copied();
    debug_assert!(
        picked.is_some(),
        "cycle_axis called with no shared read/write axis; this should \
         be unreachable when `indexes` names a real cycle in the scheduler \
         graph. The release-mode fallback is CategoryId(0), but reaching \
         it means `schedule_steps` classified a non-cycle as a cycle.",
    );
    picked.unwrap_or(CategoryId(0))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_scheme::{
        Category, Citation, Constraint, ConstraintViolation, DerivationRelation, FiringPredicate,
        JoinSemilattice, MeetSemilattice, Parsed, Scope, SectionLetter, Template, TokenId,
        TokenRef,
    };

    // Test-fixture sentinel Citation (Constitution V Principle V test
    // carve-out). Routes through `AuthoritativeSource::EngineInternal`
    // so Display renders `[engine-internal]` and the value carries no
    // false CAPCO §-claim.
    const TEST_CITATION: Citation = Citation::new(
        marque_scheme::AuthoritativeSource::EngineInternal,
        marque_scheme::SectionRef::new(SectionLetter::A),
        match core::num::NonZeroU16::new(1) {
            Some(n) => n,
            None => unreachable!(),
        },
    );

    // Minimal scheme used to exercise the scheduler without pulling in
    // marque-capco (unit tests within `marque-engine` should not force
    // a dependency on a specific rule crate). Because schedule_rewrites
    // only touches `reads` / `writes` / `id` / the trigger+action
    // variant shape, none of the other trait methods need real
    // behavior here.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    struct StubMarking;

    impl JoinSemilattice for StubMarking {
        fn join(&self, _other: &Self) -> Self {
            Self
        }
    }

    impl MeetSemilattice for StubMarking {
        fn meet(&self, _other: &Self) -> Self {
            Self
        }
    }

    struct StubScheme;

    impl MarkingScheme for StubScheme {
        type Token = TokenId;
        type Marking = StubMarking;
        type ParseError = ();
        type OpenVocabRef = core::convert::Infallible;
        // See evaluator.rs for the binding rationale.
        type Parsed<'src> = ();
        type Canonical = ();
        type Projected = ();
        fn name(&self) -> &str {
            "stub"
        }
        fn schema_version(&self) -> &str {
            "v0"
        }
        fn categories(&self) -> &[Category] {
            &[]
        }
        fn constraints(&self) -> &[Constraint] {
            &[]
        }
        fn templates(&self) -> &[Template] {
            &[]
        }
        fn parse(&self, _: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
            Err(())
        }
        fn satisfies(&self, _: &Self::Marking, _: &TokenRef) -> bool {
            false
        }
        fn validate(&self, _: &Self::Marking) -> Vec<ConstraintViolation> {
            vec![]
        }
        fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
            StubMarking
        }
        fn render_item(&self, _: &Self::Marking) -> String {
            String::new()
        }
        fn render_summary(&self, _: &Self::Marking) -> String {
            String::new()
        }
        fn render_canonical(
            &self,
            _: &Self::Marking,
            _: &marque_scheme::RenderContext,
            _: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
        }
    }

    const CAT_X: CategoryId = CategoryId(1);
    const CAT_Y: CategoryId = CategoryId(2);
    const CAT_Z: CategoryId = CategoryId(3);

    fn declarative(
        id: RewriteId,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
    ) -> PageRewrite<StubScheme> {
        PageRewrite::declarative(
            id,
            TEST_CITATION,
            CategoryPredicate::Empty { category: CAT_X },
            CategoryAction::Clear { category: CAT_X },
            reads,
            writes,
        )
    }

    fn derivation_edge(
        id: EdgeId,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
        firing: FiringPredicate,
    ) -> DerivationEdge {
        DerivationEdge::new(
            id,
            DerivationRelation::Rollup,
            TEST_CITATION,
            reads,
            writes,
            firing,
        )
    }

    // -- existing `schedule_rewrites` projection regression tests -----------
    // The four tests below stay byte-identical: they are the signal that
    // the rewrites-only projection of the combined order is exact.

    #[test]
    fn empty_input_is_empty_output() {
        let scheduled = schedule_rewrites::<StubScheme>(&[]).unwrap();
        assert!(scheduled.is_empty());
    }

    #[test]
    fn no_dependencies_preserves_declaration_order() {
        let rewrites = vec![
            declarative("a", &[], &[CAT_X]),
            declarative("b", &[], &[CAT_Y]),
            declarative("c", &[], &[CAT_Z]),
        ];
        let scheduled = schedule_rewrites(&rewrites).unwrap();
        assert_eq!(scheduled.as_ref(), ["a", "b", "c"]);
    }

    #[test]
    fn writer_before_reader() {
        // b reads X, a writes X ⇒ a must precede b in the schedule
        // regardless of declaration order.
        let rewrites = vec![
            declarative("b", &[CAT_X], &[CAT_Y]),
            declarative("a", &[], &[CAT_X]),
        ];
        let scheduled = schedule_rewrites(&rewrites).unwrap();
        assert_eq!(scheduled.as_ref(), ["a", "b"]);
    }

    #[test]
    fn self_edge_is_permitted() {
        // A rewrite that reads and writes the same category has no
        // in-edge from itself — it's a no-op dependency.
        let rewrites = vec![declarative("a", &[CAT_X], &[CAT_X])];
        let scheduled = schedule_rewrites(&rewrites).unwrap();
        assert_eq!(scheduled.as_ref(), ["a"]);
    }

    // -- combined rewrite + derivation-edge scheduling ---------------------

    #[test]
    fn edges_only_preserves_declaration_order() {
        let edges = vec![
            derivation_edge("e1", &[], &[CAT_X], FiringPredicate::Always),
            derivation_edge("e2", &[], &[CAT_Y], FiringPredicate::Always),
            derivation_edge("e3", &[], &[CAT_Z], FiringPredicate::Always),
        ];
        let order = schedule_steps::<StubScheme>(&[], &edges).unwrap();
        assert_eq!(
            order.as_ref(),
            [
                ScheduledStep::DerivationEdge("e1"),
                ScheduledStep::DerivationEdge("e2"),
                ScheduledStep::DerivationEdge("e3"),
            ]
        );
    }

    #[test]
    fn rewrite_writer_before_edge_reader() {
        // Rewrite writes X, edge reads X ⇒ rewrite precedes edge.
        let rewrites = vec![declarative("r", &[], &[CAT_X])];
        let edges = vec![derivation_edge(
            "e",
            &[CAT_X],
            &[CAT_Y],
            FiringPredicate::Always,
        )];
        let order = schedule_steps(&rewrites, &edges).unwrap();
        assert_eq!(
            order.as_ref(),
            [
                ScheduledStep::PageRewrite("r"),
                ScheduledStep::DerivationEdge("e"),
            ]
        );
    }

    #[test]
    fn edge_writer_before_rewrite_reader() {
        // Edge writes X, rewrite reads X ⇒ edge precedes rewrite.
        let rewrites = vec![declarative("r", &[CAT_X], &[CAT_Y])];
        let edges = vec![derivation_edge("e", &[], &[CAT_X], FiringPredicate::Always)];
        let order = schedule_steps(&rewrites, &edges).unwrap();
        assert_eq!(
            order.as_ref(),
            [
                ScheduledStep::DerivationEdge("e"),
                ScheduledStep::PageRewrite("r"),
            ]
        );
    }

    #[test]
    fn edge_to_edge_ordering() {
        // Edge A writes X, edge B reads X ⇒ A before B.
        let edges = vec![
            derivation_edge("b", &[CAT_X], &[CAT_Y], FiringPredicate::Always),
            derivation_edge("a", &[], &[CAT_X], FiringPredicate::Always),
        ];
        let order = schedule_steps::<StubScheme>(&[], &edges).unwrap();
        assert_eq!(
            order.as_ref(),
            [
                ScheduledStep::DerivationEdge("a"),
                ScheduledStep::DerivationEdge("b"),
            ]
        );
    }

    #[test]
    fn union_cycle_with_edge_member_is_rewrite_cycle() {
        // Rewrite writes X reads Y; edge writes Y reads X ⇒ cycle whose
        // members include both kinds.
        let rewrites = vec![declarative("r", &[CAT_Y], &[CAT_X])];
        let edges = vec![derivation_edge(
            "e",
            &[CAT_X],
            &[CAT_Y],
            FiringPredicate::Always,
        )];
        let err = schedule_steps(&rewrites, &edges).unwrap_err();
        match err {
            EngineConstructionError::RewriteCycle { axis, members } => {
                assert!(members.contains(&ScheduledStep::PageRewrite("r")));
                assert!(members.contains(&ScheduledStep::DerivationEdge("e")));
                assert!(axis == CAT_X || axis == CAT_Y, "axis was {axis:?}");
            }
            other => panic!("expected RewriteCycle, got {other:?}"),
        }
    }

    #[test]
    fn when_mode_edge_still_participates_in_cycle_check() {
        // Same cycle, but the edge is mode-gated — firing is
        // scheduling-irrelevant, so the cycle is still rejected.
        let rewrites = vec![declarative("r", &[CAT_Y], &[CAT_X])];
        let edges = vec![derivation_edge(
            "e",
            &[CAT_X],
            &[CAT_Y],
            FiringPredicate::WhenMode("strict"),
        )];
        let err = schedule_steps(&rewrites, &edges).unwrap_err();
        assert!(matches!(err, EngineConstructionError::RewriteCycle { .. }));
    }

    #[test]
    fn cowriter_without_explicit_read_is_ambiguous() {
        // Rewrite writes Y; edge writes Y but reads nothing ⇒ two
        // producers of Y with no read forcing order. Stale-value anchor.
        let rewrites = vec![declarative("r", &[], &[CAT_Y])];
        let edges = vec![derivation_edge("e", &[], &[CAT_Y], FiringPredicate::Always)];
        let err = schedule_steps(&rewrites, &edges).unwrap_err();
        match err {
            EngineConstructionError::AmbiguousCoWriter { axis, nodes } => {
                assert_eq!(axis, CAT_Y);
                assert!(nodes.contains(&ScheduledStep::PageRewrite("r")));
                assert!(nodes.contains(&ScheduledStep::DerivationEdge("e")));
            }
            other => panic!("expected AmbiguousCoWriter, got {other:?}"),
        }
    }

    #[test]
    fn cowriter_with_explicit_read_is_ok() {
        // Edge also reads Y ⇒ the explicit read forces edge-after-rewrite.
        let rewrites = vec![declarative("r", &[], &[CAT_Y])];
        let edges = vec![derivation_edge(
            "e",
            &[CAT_Y],
            &[CAT_Y],
            FiringPredicate::Always,
        )];
        let order = schedule_steps(&rewrites, &edges).unwrap();
        assert_eq!(
            order.as_ref(),
            [
                ScheduledStep::PageRewrite("r"),
                ScheduledStep::DerivationEdge("e"),
            ]
        );
    }

    #[test]
    fn rewrite_rewrite_cowriter_is_not_guarded() {
        // Two rewrites co-write Y, neither reads it ⇒ tolerated
        // (edge-scoping protects multi-rewrite schemes from regression).
        let rewrites = vec![
            declarative("a", &[], &[CAT_Y]),
            declarative("b", &[], &[CAT_Y]),
        ];
        let order = schedule_steps::<StubScheme>(&rewrites, &[]).unwrap();
        assert_eq!(
            order.as_ref(),
            [
                ScheduledStep::PageRewrite("a"),
                ScheduledStep::PageRewrite("b"),
            ]
        );
    }

    #[test]
    fn independent_edge_and_rewrite_dont_false_positive() {
        // Rewrite writes X, edge writes Y (disjoint) ⇒ no hazard.
        let rewrites = vec![declarative("r", &[], &[CAT_X])];
        let edges = vec![derivation_edge("e", &[], &[CAT_Y], FiringPredicate::Always)];
        let order = schedule_steps(&rewrites, &edges).unwrap();
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn empty_axis_edge_is_tolerated() {
        // An edge with no reads and no writes is a dataflow no-op: it
        // schedules, trips no guard, and joins no cycle.
        let edges = vec![derivation_edge("e", &[], &[], FiringPredicate::Always)];
        let order = schedule_steps::<StubScheme>(&[], &edges).unwrap();
        assert_eq!(order.as_ref(), [ScheduledStep::DerivationEdge("e")]);
    }

    #[test]
    fn rewrites_only_projection_matches_schedule_rewrites() {
        // The projection of the combined order for an edge-free input is
        // byte-identical to schedule_rewrites.
        let rewrites = vec![
            declarative("b", &[CAT_X], &[CAT_Y]),
            declarative("a", &[], &[CAT_X]),
        ];
        let combined = schedule_steps(&rewrites, &[]).unwrap();
        let projected = project_rewrites(&combined);
        let direct = schedule_rewrites(&rewrites).unwrap();
        assert_eq!(projected, direct);
    }
}
