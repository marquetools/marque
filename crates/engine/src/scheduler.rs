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
    CategoryAction, CategoryId, CategoryPredicate, MarkingScheme, PageRewrite, RewriteId,
};
use std::collections::{BTreeMap, BTreeSet};

use crate::errors::EngineConstructionError;

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
    let mut frontier: std::collections::VecDeque<usize> = (0..n)
        .filter(|i| in_degree[*i] == 0)
        .collect();
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
        // Cycle detected. Collect the members still in-degree > 0 and
        // any rewrite reachable only from them. The cycle may span
        // multiple components; `cycle_members` below returns every
        // participating rewrite.
        let cycle_indexes: Vec<usize> = (0..n).filter(|i| in_degree[*i] > 0).collect();
        let axis = cycle_axis(rewrites, &cycle_indexes);
        let members: Box<[RewriteId]> = cycle_indexes
            .into_iter()
            .map(|i| rewrites[i].id)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        return Err(EngineConstructionError::RewriteCycle { axis, members });
    }

    Ok(scheduled.into_boxed_slice())
}

/// Does the rewrite's trigger or action contain a `Custom` variant?
///
/// `Custom` variants carry function pointers opaque to the scheduler,
/// so their dataflow cannot be derived from the variant itself —
/// callers must annotate `reads` / `writes` explicitly. Declarative
/// variants can safely elide annotations because the category is
/// carried in the variant payload.
fn rewrite_is_custom<S: MarkingScheme + ?Sized>(rw: &PageRewrite<S>) -> bool {
    matches!(rw.trigger, CategoryPredicate::Custom(_))
        || matches!(rw.action, CategoryAction::Custom(_))
}

/// Pick a category from the rewrite cycle to name in the error.
///
/// Any category that appears on both sides of the cycle is a valid
/// answer. We pick the lowest category id that's both read and written
/// by some member of the cycle — gives deterministic output for
/// reporting. Falls back to `CategoryId(0)` when nothing matches
/// (should not happen if `schedule_rewrites` correctly identified a
/// cycle, but the fallback keeps the return type total).
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
    reads
        .intersection(&writes)
        .next()
        .copied()
        .unwrap_or(CategoryId(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use marque_scheme::{
        Category, Constraint, ConstraintViolation, Lattice, Parsed, Scope, Template, TokenId,
        TokenRef,
    };

    // Minimal scheme used to exercise the scheduler without pulling in
    // marque-capco (unit tests within `marque-engine` should not force
    // a dependency on a specific rule crate). Because schedule_rewrites
    // only touches `reads` / `writes` / `id` / the trigger+action
    // variant shape, none of the other trait methods need real
    // behavior here.
    #[derive(Clone, Debug, PartialEq, Eq, Default)]
    struct StubMarking;

    impl Lattice for StubMarking {
        fn join(&self, _other: &Self) -> Self {
            Self
        }
        fn meet(&self, _other: &Self) -> Self {
            Self
        }
    }

    struct StubScheme;

    impl MarkingScheme for StubScheme {
        type Token = TokenId;
        type Marking = StubMarking;
        type ParseError = ();
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
            "test",
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
