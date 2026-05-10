// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 3 US1 — Scheduler (T019–T022).
//!
//! Drives `Engine::new` through a synthetic [`marque_scheme::MarkingScheme`]
//! whose `page_rewrites()` table we manipulate directly. Because
//! `marque-scheme` has no dependency on `marque-capco` (Constitution VII
//! crate graph), we define a local `StubScheme` here rather than
//! exercising `CapcoScheme` — the scheduler only inspects the
//! `reads`/`writes` axes plus the trigger/action variant shape, so no
//! real marking logic is required.

use marque_config::Config;
use marque_engine::{Engine, EngineConstructionError};
use marque_rules::RuleSet;
use marque_scheme::{
    Category, CategoryAction, CategoryId, CategoryPredicate, Constraint, ConstraintViolation,
    Lattice, MarkingScheme, PageRewrite, Parsed, RewriteId, Scope, Template, TokenId, TokenRef,
};

// ---------------------------------------------------------------------------
// StubScheme — a minimal `MarkingScheme` whose rewrite table the test
// supplies. No parsing / validation / rendering is exercised.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct StubMarking;

impl Lattice for StubMarking {
    fn join(&self, _: &Self) -> Self {
        Self
    }
    fn meet(&self, _: &Self) -> Self {
        Self
    }
}

struct StubScheme {
    rewrites: Vec<PageRewrite<StubScheme>>,
}

impl StubScheme {
    fn new(rewrites: Vec<PageRewrite<StubScheme>>) -> Self {
        Self { rewrites }
    }
}

impl MarkingScheme for StubScheme {
    type Token = TokenId;
    type Marking = StubMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;

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
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &self.rewrites
    }
    fn render_portion(&self, _: &Self::Marking) -> String {
        String::new()
    }
    fn render_banner(&self, _: &Self::Marking) -> String {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Helper: construct an `Engine` from a scheme without pulling in rules.
// ---------------------------------------------------------------------------

fn try_build(scheme: StubScheme) -> Result<Engine, EngineConstructionError> {
    // The engine is hardcoded to `CapcoScheme` for its internal rule
    // dispatch (decoder, recognizer) post-PR 3c.B; the scheduler test
    // exercises only the `MarkingScheme::page_rewrites` axis of a
    // *separate* `StubScheme` value, so the rule-set type parameter
    // here is `CapcoScheme` (matching the engine's bound), not the
    // local stub.
    Engine::new(
        Config::default(),
        Vec::<Box<dyn RuleSet<marque_capco::CapcoScheme>>>::new(),
        scheme,
    )
}

const CAT_X: CategoryId = CategoryId(1);
const CAT_Y: CategoryId = CategoryId(2);
const CAT_Z: CategoryId = CategoryId(3);
const CAT_W: CategoryId = CategoryId(4);

// Stubs for `Custom` trigger/action — their bodies never run (the
// scheduler only inspects `reads`/`writes`), but they let us construct
// a rewrite whose variant is `Custom` so the `UnannotatedCustomAxes`
// guard can fire when annotations are empty. We sidestep
// `PageRewrite::custom`'s `const-fn` guard (which panics at
// compile/runtime for empty slices) by constructing the struct
// literally — that is the only way to reach the engine-level
// `UnannotatedCustomAxes` branch.

#[allow(dead_code)]
fn never_triggers(_: &StubMarking) -> bool {
    false
}
#[allow(dead_code)]
fn never_acts(_: &mut StubMarking) {}

fn custom_rewrite_with(
    id: RewriteId,
    reads: &'static [CategoryId],
    writes: &'static [CategoryId],
) -> PageRewrite<StubScheme> {
    // Direct struct construction — the `PageRewrite::custom`
    // constructor asserts non-empty axes, but the engine-level
    // `UnannotatedCustomAxes` path is what we're exercising here, so
    // we bypass the constructor-level guard.
    PageRewrite {
        id,
        citation: "test",
        trigger: CategoryPredicate::Custom(never_triggers),
        action: CategoryAction::Custom(never_acts),
        reads,
        writes,
    }
}

fn declarative_rewrite(
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

// ---------------------------------------------------------------------------
// T019 — Cycle rejection (pair + 3-rewrite variant).
// ---------------------------------------------------------------------------

#[test]
fn cyclic_rewrite_pair_fails_construction() {
    // `a` writes X, reads Y. `b` writes Y, reads X. Cycle.
    const A_READS: &[CategoryId] = &[CAT_Y];
    const A_WRITES: &[CategoryId] = &[CAT_X];
    const B_READS: &[CategoryId] = &[CAT_X];
    const B_WRITES: &[CategoryId] = &[CAT_Y];

    let scheme = StubScheme::new(vec![
        declarative_rewrite("a", A_READS, A_WRITES),
        declarative_rewrite("b", B_READS, B_WRITES),
    ]);

    let err = match try_build(scheme) {
        Ok(_) => panic!("rewrite pair cycle must fail"),
        Err(e) => e,
    };
    match err {
        EngineConstructionError::RewriteCycle { axis: _, members } => {
            let mut names: Vec<&str> = members.to_vec();
            names.sort();
            assert_eq!(
                names,
                ["a", "b"],
                "cycle must name every participating rewrite, got {names:?}"
            );
        }
        other => panic!("expected RewriteCycle, got {other:?}"),
    }
}

#[test]
fn disjoint_cycles_report_one_scc_only() {
    // Two disjoint cycles:
    //   * Cycle-1: a ↔ b (categories X, Y)
    //   * Cycle-2: c ↔ d (categories Z, W)
    // The scheduler must report exactly one of them, not a mixed
    // member list that names nodes from both. We pick the cycle
    // containing the lowest-index rewrite in declaration order —
    // `a` here — so the reported set is deterministically {a, b}.
    const A_READS: &[CategoryId] = &[CAT_Y];
    const A_WRITES: &[CategoryId] = &[CAT_X];
    const B_READS: &[CategoryId] = &[CAT_X];
    const B_WRITES: &[CategoryId] = &[CAT_Y];
    const C_READS: &[CategoryId] = &[CAT_W];
    const C_WRITES: &[CategoryId] = &[CAT_Z];
    const D_READS: &[CategoryId] = &[CAT_Z];
    const D_WRITES: &[CategoryId] = &[CAT_W];

    let scheme = StubScheme::new(vec![
        declarative_rewrite("a", A_READS, A_WRITES),
        declarative_rewrite("b", B_READS, B_WRITES),
        declarative_rewrite("c", C_READS, C_WRITES),
        declarative_rewrite("d", D_READS, D_WRITES),
    ]);

    let err = match try_build(scheme) {
        Ok(_) => panic!("cycles must fail"),
        Err(e) => e,
    };
    match err {
        EngineConstructionError::RewriteCycle { members, .. } => {
            let mut names: Vec<&str> = members.to_vec();
            names.sort();
            assert_eq!(
                names,
                ["a", "b"],
                "disjoint cycles should surface as a single SCC; the \
                 cycle containing the lowest-index rewrite wins. Got {names:?}",
            );
        }
        other => panic!("expected RewriteCycle, got {other:?}"),
    }
}

#[test]
fn downstream_blocked_rewrite_not_reported_as_cycle_member() {
    // `a` ↔ `b` form a cycle. `d` reads `Y` (which `b` writes) and
    // writes `Z` — `d` is blocked by the cycle but is NOT a cycle
    // member itself. Kahn's residual `in_degree > 0` set would
    // include `d`; Tarjan's SCC must exclude it.
    const A_READS: &[CategoryId] = &[CAT_Y];
    const A_WRITES: &[CategoryId] = &[CAT_X];
    const B_READS: &[CategoryId] = &[CAT_X];
    const B_WRITES: &[CategoryId] = &[CAT_Y];
    const D_READS: &[CategoryId] = &[CAT_Y];
    const D_WRITES: &[CategoryId] = &[CAT_Z];

    let scheme = StubScheme::new(vec![
        declarative_rewrite("a", A_READS, A_WRITES),
        declarative_rewrite("b", B_READS, B_WRITES),
        declarative_rewrite("d", D_READS, D_WRITES),
    ]);

    let err = match try_build(scheme) {
        Ok(_) => panic!("cycle must fail"),
        Err(e) => e,
    };
    match err {
        EngineConstructionError::RewriteCycle { members, .. } => {
            let mut names: Vec<&str> = members.to_vec();
            names.sort();
            assert_eq!(
                names,
                ["a", "b"],
                "downstream-blocked `d` must NOT appear in cycle members; got {names:?}",
            );
        }
        other => panic!("expected RewriteCycle, got {other:?}"),
    }
}

#[test]
fn cyclic_three_rewrite_cycle_reports_all_members() {
    // a writes X reads Z, b writes Y reads X, c writes Z reads Y.
    // Cycle: a → b → c → a (via category edges).
    const A_READS: &[CategoryId] = &[CAT_Z];
    const A_WRITES: &[CategoryId] = &[CAT_X];
    const B_READS: &[CategoryId] = &[CAT_X];
    const B_WRITES: &[CategoryId] = &[CAT_Y];
    const C_READS: &[CategoryId] = &[CAT_Y];
    const C_WRITES: &[CategoryId] = &[CAT_Z];

    let scheme = StubScheme::new(vec![
        declarative_rewrite("a", A_READS, A_WRITES),
        declarative_rewrite("b", B_READS, B_WRITES),
        declarative_rewrite("c", C_READS, C_WRITES),
    ]);

    let err = match try_build(scheme) {
        Ok(_) => panic!("3-rewrite cycle must fail"),
        Err(e) => e,
    };
    match err {
        EngineConstructionError::RewriteCycle { axis: _, members } => {
            let mut names: Vec<&str> = members.to_vec();
            names.sort();
            assert_eq!(
                names,
                ["a", "b", "c"],
                "cycle must name every participating rewrite, got {names:?}"
            );
            assert!(
                members.len() > 2,
                "this fixture is specifically > 2 to exercise variable-length reporting"
            );
        }
        other => panic!("expected RewriteCycle, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// T020 — Unannotated `custom` rewrite rejected.
// ---------------------------------------------------------------------------

#[test]
fn unannotated_custom_rewrite_fails_construction() {
    // Empty `reads` with a `Custom` trigger — engine refuses the scheme.
    let scheme = StubScheme::new(vec![custom_rewrite_with("bad", &[], &[CAT_X])]);
    let err = match try_build(scheme) {
        Ok(_) => panic!("empty-reads custom rewrite must fail"),
        Err(e) => e,
    };
    match err {
        EngineConstructionError::UnannotatedCustomAxes { rewrite } => {
            assert_eq!(rewrite, "bad");
        }
        other => panic!("expected UnannotatedCustomAxes, got {other:?}"),
    }
}

#[test]
fn unannotated_custom_rewrite_with_empty_writes_fails_construction() {
    // Empty `writes` is equally unacceptable.
    let scheme = StubScheme::new(vec![custom_rewrite_with("bad", &[CAT_X], &[])]);
    let err = match try_build(scheme) {
        Ok(_) => panic!("empty-writes custom rewrite must fail"),
        Err(e) => e,
    };
    assert!(matches!(
        err,
        EngineConstructionError::UnannotatedCustomAxes { rewrite } if rewrite == "bad"
    ));
}

// ---------------------------------------------------------------------------
// T021 — Declaration-order independence.
// ---------------------------------------------------------------------------

#[test]
fn scheduled_order_independent_of_declaration() {
    // Three rewrites with real producer/consumer edges:
    //   - "prod-y": writes Y (no reads)
    //   - "cons-y": reads Y, writes Z
    //   - "cons-z": reads Z, writes W
    // Scheduled order must always be prod-y → cons-y → cons-z.
    const PROD_Y_WRITES: &[CategoryId] = &[CAT_Y];
    const CONS_Y_READS: &[CategoryId] = &[CAT_Y];
    const CONS_Y_WRITES: &[CategoryId] = &[CAT_Z];
    const CONS_Z_READS: &[CategoryId] = &[CAT_Z];
    const CONS_Z_WRITES: &[CategoryId] = &[CAT_W];

    fn mk_prod() -> PageRewrite<StubScheme> {
        declarative_rewrite("prod-y", &[], PROD_Y_WRITES)
    }
    fn mk_cy() -> PageRewrite<StubScheme> {
        declarative_rewrite("cons-y", CONS_Y_READS, CONS_Y_WRITES)
    }
    fn mk_cz() -> PageRewrite<StubScheme> {
        declarative_rewrite("cons-z", CONS_Z_READS, CONS_Z_WRITES)
    }

    type Mk = fn() -> PageRewrite<StubScheme>;
    let permutations: &[[Mk; 3]] = &[
        [mk_prod, mk_cy, mk_cz],
        [mk_prod, mk_cz, mk_cy],
        [mk_cy, mk_prod, mk_cz],
        [mk_cy, mk_cz, mk_prod],
        [mk_cz, mk_prod, mk_cy],
        [mk_cz, mk_cy, mk_prod],
    ];

    let mut scheduled_orders: Vec<Vec<&'static str>> = Vec::new();
    for perm in permutations {
        let rewrites = perm.iter().map(|f| f()).collect();
        let engine = try_build(StubScheme::new(rewrites)).expect("cycle-free rewrite set");
        let order: Vec<&'static str> = engine.scheduled_rewrites().to_vec();
        scheduled_orders.push(order);
    }

    let first = &scheduled_orders[0];
    for (i, order) in scheduled_orders.iter().enumerate() {
        assert_eq!(
            order, first,
            "declaration permutation #{i} produced a different schedule: {order:?} vs {first:?}",
        );
    }
    assert_eq!(first.as_slice(), &["prod-y", "cons-y", "cons-z"]);
}

// ---------------------------------------------------------------------------
// T022 — Real producer/consumer edge for JOINT-promotion → FGI-absorption.
// ---------------------------------------------------------------------------

#[test]
fn joint_promotion_before_fgi_absorption() {
    // JOINT-promotion writes `fgi`; FGI-absorption reads `fgi`. Modeled
    // abstractly via CAT_X as the `fgi` stand-in. Regardless of
    // declaration order, JOINT-promotion must be scheduled first.
    const JP_WRITES: &[CategoryId] = &[CAT_X];
    const FA_READS: &[CategoryId] = &[CAT_X];
    const FA_WRITES: &[CategoryId] = &[CAT_X];

    // Declare FGI-absorption first, then JOINT-promotion — the reverse
    // of the final scheduled order. The scheduler must correct it.
    let scheme = StubScheme::new(vec![
        declarative_rewrite("fgi-absorption", FA_READS, FA_WRITES),
        declarative_rewrite("joint-promotion", &[], JP_WRITES),
    ]);
    let engine = try_build(scheme).expect("producer-consumer edge must not cycle");
    let order = engine.scheduled_rewrites();
    let jp = order.iter().position(|&r| r == "joint-promotion").unwrap();
    let fa = order.iter().position(|&r| r == "fgi-absorption").unwrap();
    assert!(
        jp < fa,
        "joint-promotion must precede fgi-absorption in {:?}",
        order,
    );
}
