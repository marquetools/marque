// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Page-rewrite scheduler.
//!
//! `schedule_rewrites` runs Kahn's algorithm over the declared
//! `PageRewrite::reads` / `writes` axes to produce a deterministic
//! topological order — a rewrite that writes category `X` runs before
//! any rewrite that reads `X`. Cycles and unannotated `Custom`
//! rewrites abort construction with
//! [`EngineConstructionError`](crate::errors::EngineConstructionError).
//!
//! This runs once at [`Engine::new`] and its output is cached on the
//! engine instance; per-document rewrite evaluation walks the
//! pre-computed order without re-sorting.

use marque_scheme::{
    ApplyIntentError, CategoryAction, CategoryId, CategoryPredicate, FactRef, MarkingScheme,
    PageRewrite, ReplacementIntent, RewriteId,
};
use std::collections::{BTreeMap, BTreeSet};

use crate::errors::EngineConstructionError;

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
/// `writes` axes.
///
/// Returns the ordered list of `RewriteId`s. Rewrites that have no
/// predecessor (neither read nor write a category another rewrite
/// writes) retain their declaration order relative to each other
/// (FR-007: declaration-order independence for *cycle-free*
/// inputs — but for rewrites with no edge between them, the only
/// stable answer is declaration order).
///
/// # Errors
///
/// - [`EngineConstructionError::UnannotatedCustomAxes`] if any
///   rewrite with a `Custom` trigger or action has empty `reads` or
///   `writes`. Declarative rewrites (`Contains` / `Empty` triggers
///   with `Clear` / `Replace` / `Promote` actions) are permitted to
///   have empty annotations — the scheduler treats them as "no
///   dataflow dependency" rather than as an authoring bug.
/// - [`EngineConstructionError::RewriteCycle`] if the axis graph
///   contains a cycle. All members participating in the cycle are
///   reported, not just the entry point.
pub fn schedule_rewrites<S>(
    rewrites: &[PageRewrite<S>],
) -> Result<Box<[RewriteId]>, EngineConstructionError>
where
    S: MarkingScheme + ?Sized,
{
    // 1. Enforce custom-annotation invariant.
    for rw in rewrites {
        let has_custom = rewrite_is_custom(rw);
        if has_custom && (rw.reads.is_empty() || rw.writes.is_empty()) {
            return Err(EngineConstructionError::UnannotatedCustomAxes { rewrite: rw.id });
        }
    }

    // 2. Build the dependency graph: `edge(a, b)` iff `a` writes a
    //    category `b` reads. `a` must run before `b`.
    //
    //    Keep the per-rewrite adjacency list as an ordered set so the
    //    traversal is deterministic across runs.
    let n = rewrites.len();
    let mut in_degree: Vec<usize> = vec![0; n];
    let mut successors: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];

    // Map each category to the rewrites that write it. `BTreeMap` for
    // stable iteration order.
    let mut writers: BTreeMap<CategoryId, Vec<usize>> = BTreeMap::new();
    for (idx, rw) in rewrites.iter().enumerate() {
        for w in rw.writes {
            writers.entry(*w).or_default().push(idx);
        }
    }

    for (idx, rw) in rewrites.iter().enumerate() {
        for read_cat in rw.reads {
            let Some(producers) = writers.get(read_cat) else {
                continue;
            };
            for &producer_idx in producers {
                if producer_idx == idx {
                    // A rewrite that reads and writes the same
                    // category is a self-edge; don't count it.
                    continue;
                }
                // Producer must run before consumer: producer_idx → idx.
                if successors[producer_idx].insert(idx) {
                    in_degree[idx] += 1;
                }
            }
        }
    }

    // 3. Kahn's algorithm. Seed the frontier with indexes that have
    //    in-degree 0, preserving declaration order.
    let mut frontier: std::collections::VecDeque<usize> =
        (0..n).filter(|i| in_degree[*i] == 0).collect();
    let mut scheduled: Vec<RewriteId> = Vec::with_capacity(n);
    while let Some(idx) = frontier.pop_front() {
        scheduled.push(rewrites[idx].id);
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
        // Tarjan's SCC so downstream-blocked rewrites (nodes that
        // have in-degree > 0 only because a cycle upstream never
        // resolves) are excluded from the reported `members` list.
        // Self-edges (`a` reads and writes category `X`) are
        // explicitly NOT counted as cycles above; a size-1 SCC
        // without a self-successor is a downstream-blocked node,
        // not a cycle member.
        //
        // When multiple disjoint cycles exist, report **one** of
        // them — the one containing the lowest-index rewrite in
        // declaration order. Flattening all cycles into a single
        // `members` list would produce an error whose `axis` names
        // one cycle and whose member set mixes unrelated rewrites,
        // which is worse than naming just one cycle authoritatively.
        // The author fixes this cycle, re-runs, and the next cycle
        // (if any) surfaces on the next attempt.
        let sccs = tarjan_sccs(n, &successors);
        let mut cycle_sccs: Vec<Vec<usize>> = sccs
            .into_iter()
            .filter(|scc| {
                // Size-1 SCCs reach this code only through
                // downstream blocking — the self-loop case (a
                // rewrite reading and writing the same category)
                // is skipped earlier when building `successors`, so
                // a singleton SCC never contains a true self-edge.
                scc.len() > 1
            })
            .collect();
        debug_assert!(
            !cycle_sccs.is_empty(),
            "scheduled.len() != n but Tarjan found no non-trivial SCC; \
             this indicates a logic error in `schedule_rewrites` or in \
             `tarjan_sccs`, because Kahn's algorithm only leaves nodes \
             unscheduled when the residual graph contains a cycle."
        );
        // Pick the SCC that contains the rewrite with the lowest
        // declaration index — deterministic across runs.
        let picked = cycle_sccs
            .iter_mut()
            .min_by_key(|scc| scc.iter().min().copied().unwrap_or(usize::MAX))
            .expect("debug_assert above guards the empty-Vec case");
        picked.sort_unstable();
        let axis = cycle_axis(rewrites, picked);
        let members: Box<[RewriteId]> = picked
            .iter()
            .map(|&i| rewrites[i].id)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        return Err(EngineConstructionError::RewriteCycle { axis, members });
    }

    Ok(scheduled.into_boxed_slice())
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

/// Pick a category from the rewrite cycle to name in the error.
///
/// Any category that appears on both sides of the cycle is a valid
/// answer. We pick the lowest category id that's both read and
/// written by some member of the cycle for deterministic reporting.
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
    indexes: &[usize],
) -> CategoryId {
    let mut reads: BTreeSet<CategoryId> = BTreeSet::new();
    let mut writes: BTreeSet<CategoryId> = BTreeSet::new();
    for &i in indexes {
        for r in rewrites[i].reads {
            reads.insert(*r);
        }
        for w in rewrites[i].writes {
            writes.insert(*w);
        }
    }
    let picked = reads.intersection(&writes).next().copied();
    debug_assert!(
        picked.is_some(),
        "cycle_axis called with no shared read/write axis; this should \
         be unreachable when `indexes` names a real cycle in the scheduler \
         graph. The release-mode fallback is CategoryId(0), but reaching \
         it means `schedule_rewrites` classified a non-cycle as a cycle.",
    );
    picked.unwrap_or(CategoryId(0))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_scheme::{
        Category, Citation, Constraint, ConstraintViolation, JoinSemilattice, MeetSemilattice,
        Parsed, Scope, SectionLetter, Template, TokenId, TokenRef,
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
        // PR 3c.2.A — see evaluator.rs for the binding rationale.
        type Parsed<'src> = ();
        type Canonical = ();
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
        fn render_portion(&self, _: &Self::Marking) -> String {
            String::new()
        }
        fn render_banner(&self, _: &Self::Marking) -> String {
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
}
