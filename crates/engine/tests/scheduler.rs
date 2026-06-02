// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Scheduler tests.
//!
//! Drives `Engine::new` through a synthetic [`marque_scheme::MarkingScheme`]
//! whose `page_rewrites()` table we manipulate directly. Because
//! `marque-scheme` has no dependency on `marque-capco` (Constitution VII
//! crate graph), we define a local `StubScheme` here rather than
//! exercising `CapcoScheme` — the scheduler only inspects the
//! `reads`/`writes` axes plus the trigger/action variant shape, so no
//! real marking logic is required.

use marque_config::Config;
use marque_engine::{Engine, EngineConstructionError, ScheduledStep, SystemClock};
use marque_rules::{ConstraintBridge, RuleSet};
use marque_scheme::recognizer::{ParseContext, Recognizer};
use marque_scheme::{
    ApplyIntentError, Category, CategoryAction, CategoryId, CategoryPredicate, Citation,
    Constraint, ConstraintViolation, DerivationEdge, DerivationRelation, FactRef, FiringPredicate,
    JoinSemilattice, MarkingScheme, MeetSemilattice, PageRewrite, Parsed, RecanonScope,
    ReplacementIntent, RewriteId, Scope, SectionLetter, Template, TokenId, TokenRef,
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

// ---------------------------------------------------------------------------
// StubScheme — a minimal `MarkingScheme` whose rewrite table the test
// supplies. No parsing / validation / rendering is exercised.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct StubMarking;

impl JoinSemilattice for StubMarking {
    fn join(&self, _: &Self) -> Self {
        Self
    }
}

impl MeetSemilattice for StubMarking {
    fn meet(&self, _: &Self) -> Self {
        Self
    }
}

struct StubScheme {
    rewrites: Vec<PageRewrite<StubScheme>>,
    edges: Vec<DerivationEdge>,
}

impl StubScheme {
    fn new(rewrites: Vec<PageRewrite<StubScheme>>) -> Self {
        Self {
            rewrites,
            edges: Vec::new(),
        }
    }

    fn with_edges(rewrites: Vec<PageRewrite<StubScheme>>, edges: Vec<DerivationEdge>) -> Self {
        Self { rewrites, edges }
    }
}

impl MarkingScheme for StubScheme {
    type Token = TokenId;
    type Marking = StubMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
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
    fn category_of(&self, token: &FactRef<Self>) -> Option<CategoryId> {
        // Route TokenId(1) to CAT_X for the Intent-validation tests; any
        // other token returns None (the unroutable-token path). OpenVocab
        // refs are statically impossible here because StubScheme's
        // OpenVocabRef = Infallible.
        match token {
            FactRef::Cve(id) if id.0 == 1 => Some(CAT_X),
            _ => None,
        }
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
    fn derivation_edges(&self) -> &[DerivationEdge] {
        &self.edges
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

// `StubScheme` declares no diagnostic constraints, so it inherits every
// `ConstraintBridge` no-op default (including `bridge_emitted_rule_ids`
// → `&[]`). The empty impl is what lets the generic `Engine<StubScheme>`
// constructor satisfy its `S: ConstraintBridge` bound.
impl ConstraintBridge for StubScheme {}

/// Local [`Recognizer<StubScheme>`] — always zero-candidate `Ambiguous`
/// (the engine-safe "nothing recognized" answer). The scheduler tests
/// never lint, so the recognizer body never runs; it exists only to
/// satisfy the generic constructor's `R: Recognizer<S>` bound.
struct StubRecognizer;

impl Recognizer<StubScheme> for StubRecognizer {
    fn recognize(
        &self,
        _bytes: &[u8],
        _offset: usize,
        _scheme: &StubScheme,
        _cx: &ParseContext,
    ) -> Parsed<StubMarking> {
        Parsed::Ambiguous {
            candidates: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: construct an `Engine` over the local stub scheme through the
// generic constructor. Only the scheduler's `page_rewrites()` axis is
// exercised; the returned `Engine<StubScheme, StubRecognizer>` is
// inspected solely via `scheduled_rewrites()`.
// ---------------------------------------------------------------------------

fn try_build(
    scheme: StubScheme,
) -> Result<Engine<StubScheme, StubRecognizer>, EngineConstructionError> {
    Engine::with_clock_and_recognizer(
        Config::default(),
        Vec::<Box<dyn RuleSet<StubScheme>>>::new(),
        scheme,
        StubRecognizer,
        Box::new(SystemClock),
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
        citation: TEST_CITATION,
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
        TEST_CITATION,
        CategoryPredicate::Empty { category: CAT_X },
        CategoryAction::Clear { category: CAT_X },
        reads,
        writes,
    )
}

fn derivation_edge(
    id: &'static str,
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

/// Collect the `RewriteId`s of the page-rewrite members of a cycle, in
/// declaration order. Used by the cycle tests, which assert over the
/// rewrite participants of a cycle whose members are tagged
/// [`ScheduledStep`]s.
fn rewrite_names(members: &[ScheduledStep]) -> Vec<&'static str> {
    members
        .iter()
        .filter_map(|step| match step {
            ScheduledStep::PageRewrite(id) => Some(*id),
            ScheduledStep::DerivationEdge(_) => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Cycle rejection (pair + 3-rewrite variant).
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
            let mut names: Vec<&str> = rewrite_names(&members);
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
            let mut names: Vec<&str> = rewrite_names(&members);
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
            let mut names: Vec<&str> = rewrite_names(&members);
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
            let mut names: Vec<&str> = rewrite_names(&members);
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
// Unannotated `custom` rewrite rejected.
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
// Declaration-order independence.
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
// Real producer/consumer edge for JOINT-promotion → FGI-absorption.
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

// ---------------------------------------------------------------------------
// `CategoryAction::Intent` scheduler integration.
// ---------------------------------------------------------------------------

/// `CategoryAction::Intent` is a data-shaped action whose
/// `reads` / `writes` annotations are author-declared via
/// `PageRewrite::declarative`, just like `Clear` / `Replace` /
/// `Promote`. Empty axis slices must NOT trigger
/// `EngineConstructionError::UnannotatedCustomAxes`; only `Custom`
/// triggers/actions (opaque function pointers) require non-empty
/// annotations.
#[test]
fn intent_action_with_empty_axes_does_not_trigger_unannotated_custom_error() {
    let intent_rewrite = PageRewrite {
        id: "intent-empty-axes",
        citation: TEST_CITATION,
        trigger: CategoryPredicate::Empty { category: CAT_X },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            // TokenId(1) routes to CAT_X via StubScheme::category_of,
            // so engine-construction validation passes.
            token: FactRef::Cve(TokenId(1)),
            scope: Scope::Page,
        }),
        // Intentionally empty axes — for Intent actions this must NOT
        // be rejected as unannotated.
        reads: &[],
        writes: &[],
    };

    let scheme = StubScheme::new(vec![intent_rewrite]);
    let engine = try_build(scheme)
        .expect("Intent action with empty axes must not trip UnannotatedCustomAxes");
    let order = engine.scheduled_rewrites();
    assert_eq!(
        order,
        &["intent-empty-axes"],
        "single rewrite must schedule in its own slot",
    );
}

/// Scheduler ordering: a declarative `Clear` rewrite writes
/// CAT_X; an `Intent` rewrite reads CAT_X. The scheduler must order
/// the writer before the reader regardless of declaration order.
/// This guards the ordering between writers and `Intent`-reader
/// rewrites: when NOFORN-supremacy / FOUO-eviction rewrites land, the
/// ordering is already correct by topological sort.
#[test]
fn intent_action_orders_correctly_against_existing_rewrite_writers() {
    const READS_X: &[CategoryId] = &[CAT_X];
    const WRITES_X: &[CategoryId] = &[CAT_X];

    let writer = PageRewrite::<StubScheme>::declarative(
        "writer-clears-x",
        TEST_CITATION,
        CategoryPredicate::Empty { category: CAT_X },
        CategoryAction::Clear { category: CAT_X },
        &[],
        WRITES_X,
    );
    let reader_intent = PageRewrite {
        id: "reader-intent-on-x",
        citation: TEST_CITATION,
        trigger: CategoryPredicate::Empty { category: CAT_X },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::Cve(TokenId(1)),
            scope: Scope::Page,
        }),
        reads: READS_X,
        writes: WRITES_X,
    };

    // Declare reader first so the scheduler must reorder it.
    let scheme = StubScheme::new(vec![reader_intent, writer]);
    let engine = try_build(scheme).expect("writer/reader edge must not cycle");
    let order = engine.scheduled_rewrites();
    let writer_pos = order
        .iter()
        .position(|&r| r == "writer-clears-x")
        .expect("writer must appear");
    let reader_pos = order
        .iter()
        .position(|&r| r == "reader-intent-on-x")
        .expect("reader must appear");
    assert!(
        writer_pos < reader_pos,
        "writer-clears-x must precede reader-intent-on-x in {:?}",
        order,
    );
}

/// `Engine::new` rejects a `CategoryAction::Intent` whose `FactRef`
/// does not route to any category. `StubScheme::category_of` returns
/// `None` for any `TokenId` other than `TokenId(1)`, so a rewrite
/// using `TokenId(99)` triggers `InvalidIntentInPageRewrite`.
///
/// This mirrors the `CapcoScheme` test in `category_action_intent.rs`
/// but exercises the path through `StubScheme` to confirm the
/// validation pass calls `category_of` on the user-supplied scheme
/// (Constitution VII: the scheme's own `category_of` is the only
/// authority for token routing).
#[test]
fn engine_new_rejects_intent_with_unroutable_token_via_stub_scheme() {
    let rewrite = PageRewrite {
        id: "intent-unroutable",
        citation: TEST_CITATION,
        trigger: CategoryPredicate::Empty { category: CAT_X },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::Cve(TokenId(99)),
            scope: Scope::Page,
        }),
        reads: &[CAT_X],
        writes: &[CAT_X],
    };
    let scheme = StubScheme::new(vec![rewrite]);

    let err = match try_build(scheme) {
        Ok(_) => panic!("unroutable Intent token must fail Engine::new"),
        Err(e) => e,
    };
    match err {
        EngineConstructionError::InvalidIntentInPageRewrite {
            rewrite_id,
            fact_label,
            error,
        } => {
            assert_eq!(rewrite_id, "intent-unroutable");
            assert!(
                fact_label.contains("Cve"),
                "fact_label must Debug-format the FactRef variant: got {fact_label:?}",
            );
            assert_eq!(error, ApplyIntentError::UnknownToken);
        }
        other => panic!("expected InvalidIntentInPageRewrite, got {other:?}"),
    }
}

/// `Engine::new` accepts a `Recanonicalize` intent at any scope —
/// the intent carries no `FactRef`s, so there is nothing to validate.
#[test]
fn engine_new_accepts_recanonicalize_intent_in_page_rewrite() {
    let rewrite = PageRewrite {
        id: "intent-recanonicalize",
        citation: TEST_CITATION,
        trigger: CategoryPredicate::Empty { category: CAT_X },
        action: CategoryAction::Intent(ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Page,
            prior: None,
        }),
        reads: &[CAT_X],
        writes: &[CAT_X],
    };
    let scheme = StubScheme::new(vec![rewrite]);

    let engine = try_build(scheme).expect("Recanonicalize-only intent has no FactRefs to validate");
    let order = engine.scheduled_rewrites();
    assert_eq!(order, &["intent-recanonicalize"]);
}

#[test]
fn derivation_edges_cycle_checked_at_engine_new() {
    // A rewrite and a derivation edge form a 2-node cycle through the
    // combined graph: the rewrite writes X reads Y, the edge writes Y
    // reads X. `Engine::with_clock_and_recognizer` must reject it.
    const RW_READS: &[CategoryId] = &[CAT_Y];
    const RW_WRITES: &[CategoryId] = &[CAT_X];
    const EDGE_READS: &[CategoryId] = &[CAT_X];
    const EDGE_WRITES: &[CategoryId] = &[CAT_Y];

    let scheme = StubScheme::with_edges(
        vec![declarative_rewrite("rw", RW_READS, RW_WRITES)],
        vec![derivation_edge(
            "edge",
            EDGE_READS,
            EDGE_WRITES,
            FiringPredicate::Always,
        )],
    );

    let err = match try_build(scheme) {
        Ok(_) => panic!("rewrite/edge cycle must fail construction"),
        Err(e) => e,
    };
    match err {
        EngineConstructionError::RewriteCycle { members, .. } => {
            assert!(
                members.contains(&ScheduledStep::PageRewrite("rw")),
                "cycle must name the participating rewrite, got {members:?}"
            );
            assert!(
                members.contains(&ScheduledStep::DerivationEdge("edge")),
                "cycle must name the participating edge, got {members:?}"
            );
        }
        other => panic!("expected RewriteCycle, got {other:?}"),
    }
}

#[test]
fn engine_scheduled_steps_tags_rewrites_when_no_edges() {
    // For an edge-free scheme, scheduled_steps mirrors
    // scheduled_rewrites, tagged as PageRewrite steps.
    let scheme = StubScheme::new(vec![
        declarative_rewrite("a", &[], &[CAT_X]),
        declarative_rewrite("b", &[CAT_X], &[CAT_Y]),
    ]);
    let engine = try_build(scheme).expect("acyclic edge-free scheme builds");
    assert_eq!(engine.scheduled_rewrites(), &["a", "b"]);
    assert_eq!(
        engine.scheduled_steps(),
        &[
            ScheduledStep::PageRewrite("a"),
            ScheduledStep::PageRewrite("b"),
        ]
    );
}
