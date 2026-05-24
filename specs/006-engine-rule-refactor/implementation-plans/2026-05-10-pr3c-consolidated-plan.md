---
date: 2026-05-10
status: planning — pre-implementation
supersedes: (worktree-only) 2026-05-09-pr3c-migration-plan.md
parent-architecture: specs/006-engine-rule-refactor/architecture.md (2026-05-09)
parent-audit: specs/006-engine-rule-refactor/rule-body-audit.md (2026-05-10)
parent-decisions: specs/006-engine-rule-refactor/decisions/{01..04}*.md (2026-05-10)
followups: specs/006-engine-rule-refactor/followups/{no-fix-auto-apply-calibration, admonition-channel}.md
authors: PM (synthesized from 4-analyst decision round + pre-flight verification)
---

# PR 3c — Consolidated Plan

## What this plan grounds

PR 3c is the engine + rule-architecture-refactor keystone. PR 3c.1 (foundation
types, T030–T040) landed earlier; this plan covers PR 3c.A and PR 3c.B (a 2-PR
split, per Decision 11) which together replace `FixProposal` with the
bag-of-tokens emission vocabulary, land the `render_canonical` trait surface
and body, migrate the 47 registered rules per their §3.0.b purpose-row, and
preserve audit-record G13 closure throughout.

## Pre-flight findings (2026-05-10)

1. **PR 3c.A bases on `006-engine-rule-refactor` (workspace GREEN).**
   `cargo check --workspace` produces zero errors on this branch. The
   "5 errors in `marque/src/render.rs` baseline" referenced in earlier
   planning lived on the unmerged `refactor-006-pr-3c-foundations`
   branch, which contained pre-architectural-restatement foundation
   work for what was originally scoped as PR 3c.2. **That branch is
   discarded** — its directive-enum direction (extending
   `ReplacementIntent::Render` with `S::RenderDirective` bound to a
   concrete `CapcoRenderDirective` enum) is exactly the abstraction
   architecture.md §"What was lost during PR 3c.1" rejects. Any
   architecturally-neutral content from those 11 commits (docs,
   formatting, seal-hole fix) is easier to redo on the new shape than
   to rebase across the type rewrite. Commit 1 documents the
   green-at-branching baseline of `006-engine-rule-refactor`;
   subsequent commits assert no regression.

2. **`tools/citation-lint/` EXISTS** alongside six sibling lint tools
   (`masking-pin-lint`, `promote-callsite-lint`, `message-template-extract`,
   `regression-grep`, `corpus-analysis`, `flake-watch`). The Constitution VIII
   enforcement harness is more mature than Agent 1 knew at decision time.
   Commit 1's scope is **calibration + extension**, not from-scratch build.

3. **PR 3c.1 foundation types LANDED** at:
   - `crates/rules/src/lib.rs` (987 LoC)
   - `crates/rules/src/fix_intent.rs` (312 LoC)
   - `crates/rules/src/message.rs` (637 LoC)
   - `crates/rules/src/confidence.rs` (434 LoC)
   - `crates/rules/tests/fix_intent_smoke.rs`

4. **PR 3c.1's emission types are pre-architectural-restatement.** Current
   shape: `ReplacementIntent::{Cve, Render, Delete}` with `RenderDirective<S>`
   phantom alias and `target_span: Span` on `FixIntent`. Architecture.md
   (2026-05-09) §"What was lost during PR 3c.1" names this design as the
   wrong abstraction layer. PR 3c.A commit 2 **replaces** the types with
   `FactAdd` / `FactRemove` / `Recanonicalize` (architecture.md's commitment),
   not extends them. This is the load-bearing structural change of PR 3c.

5. **`MarkingScheme` trait surface** (in `crates/scheme/src/scheme.rs`) has
   `validate`, `project`, `project_banner`, `render_portion`, `render_banner`
   — but no `render_canonical` and no `canonicalize` GAT. Decision 5's
   `render_canonical` addition is genuinely new in PR 3c.A commit 4.

6. **No local 2026-05-09 plan to supersede.** The worktree migration plan
   from PR 3c.2 was not merged. This plan is the canonical reference going
   forward.

## Architectural commitment

The four-stage pipeline (`bytes → ParsedAttrs → CanonicalAttrs →
ProjectedMarking → bytes`) is the architectural backbone. Rules are
divergence detectors that compare `Vec<CanonicalAttrs>` against
`ProjectedMarking` and emit fact-set deltas (`FactAdd` / `FactRemove`) or
re-render requests (`Recanonicalize`). The renderer (`render_canonical`)
is the single source of canonical form. Every type, trait, and crate
boundary in this plan respects that pipeline. Full statement at
`specs/006-engine-rule-refactor/architecture.md` (2026-05-09); type sketch
at architecture.md §"What fixes are / Type sketch" (2026-05-10).

## The 11 locked decisions

| # | Decision | Resolution | Artifact |
|---|---|---|---|
| 1 | Renderer-first or buckets-first? | Renderer-first; beachhead = E054 + E057 + E021 (2 `FactRemove` + 1 `FactAdd`) | `decisions/01-migration-sequencing.md` |
| 2 | Walker decomposition | **Hybrid** — keep E060 as walker (retires when renderer absorbs it); inline E058 (27 rows) and E059 (5 rows) as per-row `Constraint` entries | `decisions/02-catalog-shape.md` |
| 3 | Six no-fix rules: feature or refactor? | **Middle-path** — land vocabulary in PR 3c; auto-apply + calibration deferred to follow-up | `followups/no-fix-auto-apply-calibration.md` |
| 4 | Custom-rule residue (E005, S005, S006) | **Provisional `Constraint::Custom`** in PR 3c with retirement-target comments | `decisions/02-catalog-shape.md` D4 |
| 5 | Renderer interface design | Per-axis canonicalization parameterized by `Scope`; `&'static [(CategoryId, fn pointer)]` dispatch table on `CapcoScheme`; writer-passing (`&mut dyn fmt::Write`) | `decisions/01-migration-sequencing.md` D5 |
| 6 | Citation hygiene: opportunistic or systematic? | **Systematic** — ~30 edits + lint-harness extension in commit 1 | `decisions/04-citation-hygiene.md` |
| 7 | Renderer test strategy | Re-enable existing `#[ignore]`'d round-trip at `crates/capco/tests/parse_render_roundtrip.rs:420` + add lattice-equal-byte-identical property test (~250–400 LoC) | `decisions/03-empirical-concerns.md` D7 |
| 8 | `ProjectedMarking` materialization cost | Existing benches show 19× headroom on SC-001 (824 µs / 16 000 µs budget); per-page cache mirroring `PageContext`; rely on `bench-check.sh` for regression detection | `decisions/03-empirical-concerns.md` D8 |
| 9 | Audit-schema cutover | **AMENDED twice — Path C (defer bump to commit 10) adopted post-pass-2 review (2026-05-10).** Path A (bump at commit 2) failed second-pass review: between commits 2 and 10 the binary would emit two structurally-distinct `AppliedFix.proposal` shapes under one schema name, violating FR-014's single-shape-per-binary invariant. Path C: schema bump to `marque-mvp-3` lands atomically with `FixProposal` cleanup at commit 10. | `decisions/03-empirical-concerns.md` D9 + amendment below |
| 10 | Recognizer diagnostic surface | Distinct R-prefix rule IDs (R001 + future R002+); admonition channel out-of-scope for PR 3c | `decisions/03-empirical-concerns.md` D10 |
| 11 | PR commit shape | **Split into 2 PRs**: 3c.A (5 commits, foundations) + 3c.B (5 commits, migration execution + cleanup) | `decisions/01-migration-sequencing.md` D11 |

### Decision 9 amendment — RESOLVED (2026-05-10, twice)

**First amendment (Path A):** post-pass-1 plan review (rust-reviewer
item 4, architect item 5) identified that Decision 9's "no schema
bump needed" assertion was structurally wrong. The concrete defect:
`AppliedFix.proposal` changes from `FixProposal` (fields: `span`,
`original`, `replacement`, `confidence`, `source`, `migration_ref`)
to `FixIntent<S>` (a tagged enum with three variants, each carrying
different fields). NDJSON output for the `proposal` sub-object is
structurally replaced, not extended. Initial resolution: bump to
`marque-mvp-3` at commit 2.

**Second amendment (Path C, ADOPTED):** post-pass-2 plan review
(architect "stealth-mvp-3a/3b" finding, rust-reviewer "structurally
distinguishable JSON shapes under one schema name" finding) showed
Path A still violates FR-014. Between commit 2 and commit 10, the
binary emits TWO structurally-distinct `AppliedFix.proposal`
shapes — `FixIntent`-shape JSON for migrated rules, legacy
`FixProposal`-shape JSON for non-migrated rules — both labeled
`marque-mvp-3`. A consumer pinned to "marque-mvp-3" reads
different schemas at commit 7 vs commit 11. That is dual-versioning
under one schema name; FR-014's "a single binary emits exactly one
schema" invariant requires structural equivalence at the schema
level, not just label equivalence.

**Path C resolution: defer the schema bump to commit 10.** Commits
2 through 9 still emit `marque-mvp-2`-shape audit records for ALL
rules (migrated and non-migrated). The new `FixIntent<S>` flows
through the engine internally but is converted at the audit-emit
boundary into the legacy `FixProposal`-shape JSON for migrated
rules during the transition. Commit 10 atomically (a) removes
`FixProposal` from `marque-rules`, (b) flips
`MARQUE_AUDIT_SCHEMA` default to `"marque-mvp-3"`, (c) extends the
accept-list, (d) removes the engine-side legacy-emit conversion,
and (e) lands the `FixIntent`-shape JSON as the `marque-mvp-3`
shape. The cutover is atomic; FR-014's invariant holds throughout.

**Implementation.**

- `MARQUE_AUDIT_SCHEMA` build-time env-pin's closed accept-list in
  `marque-engine` extends to `["marque-mvp-1", "marque-mvp-2",
  "marque-mvp-3"]` **at commit 10**, not commit 2. Default flips to
  `"marque-mvp-3"` at commit 10. Commits 2–9 keep
  `"marque-mvp-2"` as the default.
- Engine-side legacy-emit conversion (commit 2 scope addition):
  `Engine::fix_inner` carries a small private function
  `fix_intent_to_legacy_proposal(intent: &FixIntent<S>, scheme: &S,
  ctx: &PromotionContext) -> FixProposal` that maps a
  `FactRemove`/`FactAdd`/`Recanonicalize` to the equivalent
  `FixProposal` (span + replacement bytes computed via
  `render_canonical`) for audit emission. This conversion exists
  ONLY in `marque-engine`, never in rule crates. Commit 10 deletes
  this function alongside the legacy `FixProposal` type.
- WASM/server serializers (`crates/wasm/src/lib.rs`,
  `crates/server/...`) — UNCHANGED in commit 2. The serializers
  continue reading `fix.proposal.span` / `.original` / `.replacement`
  / `.source` because every audit record in commits 2–9 carries the
  legacy-compat shape (synthesized from the `FixIntent` for migrated
  rules, native for non-migrated). Commit 10 rewrites them to read
  the `FixIntent` variants directly.
- CHANGELOG entry lands at commit 10, documenting the
  `marque-mvp-2 → marque-mvp-3` shape change with the rationale
  (PR 3c bag-of-tokens architectural restatement).

**Strengthened `g13_closure` content-ignorance test (commit 3
acceptance criteria) stays scoped to `FixIntent`-shape internal
records only**, not the audit-emitted JSON during transition.
The audit-emitted JSON inherits the pre-existing G13 channel
(`FixProposal.original`/`.replacement` carry document bytes — a
known pre-existing channel that PR 3c was always going to close at
commit 10). The test pins the Constitution V Principle V invariant
on the new emission path; the legacy emission path retires at
commit 10 with `FixProposal` itself. Per commit 3's acceptance
criteria amendment below: the test enumerates the structural
envelope of the `FixIntent` data structure (Rust-side, not
JSON-side) and fails if any byte outside that envelope appears in
the in-memory `AppliedFix.proposal` payload at promotion time.

## PR 3c.A — Foundations (5 commits)

Goal: land the citation-hygiene baseline, the new emission-type vocabulary,
the `render_canonical` trait surface + body, and a 3-rule beachhead that
validates the vocabulary end-to-end. After 3c.A merges to staging, no
rule has retired yet; the migration begins in 3c.B.

**Commit ordering within 3c.A (post-pass-2 review): linear
`1 → 2 → 4 → 5 → 3`.** Commit 3's beachhead must run against a
working `render_canonical` body so the byte-identity gate (User
RESOLVED Q2) can verify the multi-trigraph case at merge time.
Commit 5 imports types declared in commit 4
(`AxisRenderRow`, `RENDER_TABLE`, `MarkingScheme::render_canonical`),
so commit 5 cannot compile against PR 3c.A's base without commit
4's branch as its base. Wall-clock parallel authoring is fine
(commit 5 body developed against a working copy with both
branches), but merge sequence is linear.

### Commit 1 — Citation hygiene sweep + lint-harness extension

**Goal.** Land Constitution VIII's correctness baseline so the migration
inherits clean §-citations and a CI check that prevents drift.

**Scope.**
- Citation corrections (~30 edits across `crates/capco/src/rules.rs`,
  `crates/capco/src/rules_declarative.rs`, `crates/capco/src/scheme.rs`):
  - **E012 wrapper**: `§B.1` → `§H.3 p55` (per `decisions/04-citation-hygiene.md`)
  - **E015 wrapper**: `§B.3` → `§H.7 + §B.3.d`
  - **E001, E009, E011, E013**: §-only → page-precise (E011 has a semantic
    error per the audit: cites `§H.3 p163` (DISPLAY ONLY); correct is `§H.3 p55`)
  - ~22 additional §-only tightenable citations per Agent 4's wider sweep
- `tools/citation-lint/` extension to assert every rule's citation
  passes verbatim Grep against `crates/capco/docs/CAPCO-2016.md`
- CI workflow add to invoke `tools/citation-lint/` on every PR

**Acceptance criteria.**
- `cargo check --workspace` green (no regression from baseline)
- `tools/citation-lint/` exits non-zero on any rule whose `Diagnostic.citation`
  cannot be Grep-located in `CAPCO-2016.md`
- All ~30 corrected citations Grep-verified by the harness
- CI workflow blocks merge on citation-lint failure

**Tests.** Lint-harness self-test (a known-good rule passes; a known-bad
deliberate-defect fixture fails).

**Constitution.** Principle VIII (Authoritative Source Fidelity) — every
citation in commit 1 re-verified at point of edit, not just at point of
audit. The lint harness is the mechanical enforcement going forward.

**LoC estimate.** ~30 citation edits (~60 LoC) + ~150–250 LoC harness
extension (the existing `tools/citation-lint/` already has parser, scanner,
catalog, resolver, and integration tests; commit 1 needs to wire a
passage-grep stage and a `pub(crate)` accessor for per-rule citation walk —
the deferred `crates/capco/tests/citation_fidelity.rs` skeleton is the
landing pad) + ~50 LoC CI config. **Total: ~260–360 LoC.** User to
confirm scope by inspecting `tools/citation-lint/` before commit 1 lands;
the citation-edit sub-commit can land independently of the harness extension
if the harness needs more work than expected.

**Risks.** A citation correction may surface a rule whose predicate is also
wrong against the corrected source — that's a Constitution VIII defect, not
a hygiene fix. If discovered, escalate as a separate commit (out of 3c.A
scope) and proceed without that rule's correction.

### Commit 2 — Replace emission types on `marque-rules`

**Goal.** Replace PR 3c.1's pre-restatement emission types
(`ReplacementIntent::{Cve, Render, Delete}` with `RenderDirective<S>` phantom
and `target_span: Span`) with the bag-of-tokens vocabulary
(`FactAdd` / `FactRemove` / `Recanonicalize`) per architecture.md §"What
fixes are."

**Scope.**
- `crates/rules/src/fix_intent.rs` (312 LoC) — rewrite. New shapes per
  architecture.md type sketch (post-amendment, both variants use
  `FactRef<S>`):
  - `ReplacementIntent::FactAdd { token: FactRef<S>, scope: Scope }`
  - `ReplacementIntent::FactRemove { token_ref: FactRef<S>, scope: Scope }`
  - `ReplacementIntent::Recanonicalize { scope: RecanonScope }`
  - New `FactRef<S>` enum: `Cve(TokenId)` | `OpenVocab(S::OpenVocabRef)`
  - New `RecanonScope` enum: `Portion` | `Page` | `Document`
- `FixIntent<S>` struct: retain `confidence`, `feature_ids`, `message`;
  REMOVE `target_span: Span` (spans are diagnostic-only per architecture.md).
- Retire `RenderDirective<S>` phantom-type alias. The planned
  `CapcoRenderDirective` enum (referenced in fix_intent.rs doc comment) is
  NOT built; it was the directive-enum design architecture.md explicitly
  rejects.
- `MarkingScheme` trait: add `type OpenVocabRef: Debug + Clone + Eq + Hash
  + Send + Sync + 'static;` associated type (no default — every scheme
  must declare its open-vocab carrier). The full bound set is required
  because `FactRef<S>` and therefore `FixIntent<S>` derive `Debug` /
  `Clone` and flow through `BatchEngine` (Send + Sync per Constitution
  VI), and downstream consumers may key on it (`Eq + Hash`). `'static`
  excludes lifetime-borrowing carriers — open-vocab refs must own
  their data (e.g., a SAR program identifier as a `Box<str>` or an
  enum, not `&'src str`).
- All **12** existing `MarkingScheme` impls bind `type OpenVocabRef`
  atomically in this commit (intermediate state won't compile
  otherwise — `OpenVocabRef` has no associated-type default).
  Pre-pass-2 estimate of "four" was wrong by 8; full enumeration
  (per `rg 'impl MarkingScheme for' --type rust`):

  Production:
  - `crates/capco/src/scheme.rs:2054` — `CapcoScheme`: `type OpenVocabRef = CapcoOpenVocabRef;`

  In-tree test stubs:
  - `crates/engine/src/scheduler.rs:352` — `StubScheme`: `Infallible`
  - `crates/scheme/src/canonical.rs:507` — `TestScheme`: `Infallible`
  - `crates/scheme/src/page_rewrite.rs:386` — `FakeScheme`: `Infallible`
  - `crates/rules/src/fix_intent.rs:204` — `TestScheme`: `Infallible`

  Integration test stubs:
  - `crates/engine/tests/scheduler.rs:50` — `StubScheme`: `Infallible`
  - `crates/scheme/tests/send_sync.rs:50` — `StubScheme`: `Infallible`
  - `crates/scheme/tests/adoption_readiness.rs:167` — `StubScheme`: `Infallible`
  - `crates/scheme/tests/evaluator.rs:47` — `StubScheme`: `Infallible`
  - `crates/scheme/tests/canonical_unconstructable.rs:66` — `StubScheme`: `Infallible`
  - `crates/scheme/tests/codec_surface.rs:70` — `MockScheme`: `Infallible`
  - `crates/rules/tests/fix_intent_smoke.rs:48` — `StubScheme`: `Infallible`

  `CapcoOpenVocabRef` is a new owned enum unifying SAR program
  identifiers, SCI compartment / sub-compartment paths, and FGI
  tetragraphs. Concrete shape lands in commit 2 with
  `#[derive(Debug, Clone, PartialEq, Eq, Hash)]` (matches the
  trait's `OpenVocabRef` bound set). Canonicalize-side construction
  sites land in commit 6. **No `serde::Serialize`/`Deserialize`
  derives** — current audit serialization is hand-written in WASM
  and server crates, so derive-bounds are not load-bearing. If a
  future commit adds derive-based serialization (e.g., for an MCP
  audit-stream consumer), `Serialize`/`Deserialize` are added to
  the `OpenVocabRef` bound set THEN, not pre-emptively.
- `crates/rules/src/applied_fix.rs` (or wherever `AppliedFix` lives) —
  add a new `AppliedFixProposal<S: MarkingScheme>` enum and convert
  `AppliedFix` to `AppliedFix<S>`:
  ```rust
  /// Engine-promoted proposal payload. Carries either the legacy
  /// FixProposal (for non-migrated rules during the PR 3c
  /// transition) or the new FixIntent<S> (for migrated rules).
  /// The engine's promotion path (`__engine_promote`) selects the
  /// variant; the audit-emit path converts FixIntent → FixProposal
  /// during transition (per Decision 9 Path C). Commit 10 retires
  /// the Legacy variant atomically with the schema bump.
  pub enum AppliedFixProposal<S: MarkingScheme> {
      Legacy(FixProposal),
      New(FixIntent<S>),
  }

  pub struct AppliedFix<S: MarkingScheme> {
      pub rule_id:       RuleId,
      pub proposal:      AppliedFixProposal<S>,
      pub confidence:    f32,
      pub timestamp:     SystemTime,
      pub classifier_id: Option<ClassifierId>,
      pub dry_run:       bool,
      pub input:         Box<str>,  // existing field
  }
  ```
- `AppliedFix::__engine_promote` signature changes to accept either
  variant explicitly. The cleanest shape is two constructors with
  shared body:
  ```rust
  #[doc(hidden)]
  pub fn __engine_promote_legacy(
      rule_id: RuleId,
      proposal: FixProposal,
      ctx: PromotionContext,
  ) -> AppliedFix<S> { ... }

  #[doc(hidden)]
  pub fn __engine_promote(
      rule_id: RuleId,
      intent: FixIntent<S>,
      ctx: PromotionContext,
  ) -> AppliedFix<S> { ... }
  ```
  Both wrap the proposal in the matching `AppliedFixProposal` variant
  before snapshotting `ctx` runtime state. Constitution V's
  test-fixture carve-out applies to both constructors. Commit 10
  retires `__engine_promote_legacy` and renames `__engine_promote`
  to take `FixIntent<S>` directly (single constructor again).
- `Engine::fix_inner` (`crates/engine/src/engine.rs`) — branch on
  `Diagnostic.fix` (legacy) vs `Diagnostic.fix_intent` (new) and
  call the matching constructor. Both paths share confidence-
  threshold, FR-016 sort, and C-1 overlap-guard logic. The
  `fix_intent_to_legacy_proposal` conversion runs at audit-emit
  time (NDJSON serialization), not at promotion time — the
  in-memory `AppliedFix.proposal: AppliedFixProposal::New(intent)`
  preserves the structural form for the strengthened
  `g13_closure` test (commit 3) to enumerate.
- `crates/rules/src/diagnostic.rs` (or wherever `Diagnostic` lives) —
  add `fix_intent: Option<FixIntent<S>>` alongside existing
  `fix: Option<FixProposal>`. **This makes `Diagnostic` generic over
  `S`** — trace and update consumers atomically:
  - `LintResult` and `FixResult` gain `<S>`
  - `Rule::check` return type: `Vec<Diagnostic<S>>`
  - `RuleSet`, `Engine`, `BatchEngine` gain `<S>` where they don't
    already have it
  - `crates/wasm/src/lib.rs::diagnostic_to_json` and
    `crates/wasm/src/lib.rs::applied_fix_to_audit_json_v2` —
    UNCHANGED in commit 2 (per Decision 9 Path C amendment). These
    serializers continue to read `fix.proposal.span` / `.original` /
    `.replacement` / `.source` because every audit record in commits
    2–9 carries the legacy-compat `FixProposal`-shape JSON. The
    engine performs an internal conversion (`fix_intent_to_legacy_proposal`
    helper inside `marque-engine`) that synthesizes a `FixProposal`
    from a `FixIntent<S>` at promotion time for migrated rules.
    Commit 10 rewrites the serializers to read `FixIntent` variants
    directly when `MARQUE_AUDIT_SCHEMA` flips to `"marque-mvp-3"`.
  - `crates/server/...` — any axum response types parameterized by
    `Diagnostic` or `LintResult` gain `<S>`
  - `crates/engine/.../pipeline::accept_diagnostic` and similar
    engine-emit paths
- `crates/rules/tests/fix_intent_smoke.rs` — rewrite to construct new
  variants.
- `crates/rules/src/lib.rs` — update re-exports.
- Doc comments in `fix_intent.rs` — rewrite the "Lifecycle (post-PR-3c.2)"
  section to reflect the new emission shape; remove references to
  `EngineConstructor::build_open_vocab` (which existed to dispatch the
  retired `Render` directive).

**Acceptance criteria.**
- `cargo check --workspace` green
- `cargo test -p marque-rules` green (smoke tests pass under new shape)
- `cargo test --workspace` no new failures (existing rules still emit
  `FixProposal` — they're not migrated yet)
- `FixProposal` and the new `FixIntent` coexist on `marque-rules` (D11
  transitional-state acceptance; bounded to one PR-roundtrip per
  Constitution VII §IV)

**Tests.** Smoke tests for each of the three new `ReplacementIntent`
variants. Property test: any `FixIntent<S>` round-trips through
`Debug` / `Clone` cleanly.

**Constitution.** Principle V (G13 closure) — neither `FactAdd` nor
`FactRemove` carries document bytes. `FactRef::OpenVocab(S::OpenVocabRef)`
carries scheme-side typed references (e.g., a SAR program identifier from
canonicalize), never raw input bytes.

**Crate-graph note.** Constitution VII Principle VII commits to
`marque-rules` gaining a `marque-scheme` dep "so `FixIntent<S>` can
reference scheme-defined types." That dep already exists per PR 3c.1; no
new graph edge.

**LoC estimate.** ~312 LoC `fix_intent.rs` rewritten + ~80 LoC smoke
tests rewritten + ~40 LoC `lib.rs` re-exports + ~120 LoC
`Diagnostic<S>` generic spread (consumer signature updates +
re-exports) + ~30 LoC across **12** `MarkingScheme` impls binding
`OpenVocabRef` (the 11 stubs all bind `Infallible`, ~2 LoC each;
`CapcoScheme` binds `CapcoOpenVocabRef`, ~3 LoC) + ~30 LoC
`CapcoOpenVocabRef` enum declaration + ~60 LoC `AppliedFixProposal<S>`
enum + dual `__engine_promote` constructors + ~60 LoC
`fix_intent_to_legacy_proposal` engine-side conversion helper +
~30 LoC `Engine::fix_inner` branching = **~770 LoC.** Path C
adds engine-side conversion code (~120 LoC net) but eliminates the
WASM/server serializer rewrite that Path A required (~80 LoC).
Net delta from pre-pass-2 estimate: +70 LoC.

**Risks.**
- `EngineConstructor::build_open_vocab` may have consumers in
  `marque-engine` not yet audited. Pre-flight: Grep
  `EngineConstructor::build_open_vocab` across the workspace; any
  consumers retire alongside.
- The `S::OpenVocabRef` associated type addition is a `MarkingScheme`
  trait edit. Per Constitution VII §IV, scheme-trait edits must precede
  scheme-side consumers. Order within commit 2: trait edit (with
  bounds) → all four scheme impls bind `OpenVocabRef` (three to
  `Infallible`, one to `CapcoOpenVocabRef`) → `FixIntent` rewrite →
  `Diagnostic<S>` generic spread → WASM/server serializer migration.
- `Diagnostic<S>` generic spread is invasive. Pre-flight Grep:
  `rg 'Diagnostic[<( ]|fn check\b'` to enumerate every `Diagnostic`
  consumer and `Rule::check` impl across the workspace. Estimate the
  blast radius before commit 2 begins; if the count exceeds ~80
  call sites, consider a `dyn Trait`-erasing alternative (e.g.,
  `Box<dyn AnyFixIntent>` with downcast at promotion time) before
  proceeding. The expected count (per ~50 rules + ~20 test fixtures
  + ~10 engine sites) is well under that threshold.
- Test-fixture call sites for `AppliedFix::__engine_promote` migrate
  alongside production call sites. Pre-flight Grep:
  `rg '__engine_promote' --type rust` — every call site must update
  to the new `FixIntent<S>` signature, and per Constitution V's
  test-fixture carve-out each `cfg(test)` site carries a comment
  naming the carve-out (re-verify those comments survive the
  rewrite).
- **Audit-record JSON shape is UNCHANGED in commit 2** (per Decision 9
  Path C). The `AppliedFix.proposal: AppliedFixProposal<S>` enum is a
  Rust-level structural change — the engine wraps the proposal in
  `AppliedFixProposal::New(intent)` for migrated rules and
  `AppliedFixProposal::Legacy(proposal)` for non-migrated. The
  audit-emit path (in `marque-engine`'s NDJSON serializer)
  unwraps both variants to the same `marque-mvp-2`-shape JSON
  via the `fix_intent_to_legacy_proposal` helper. Schema version
  stays `"marque-mvp-2"` through commits 2–9. The schema flip + JSON
  shape change land atomically at commit 10. FR-014's single-shape-
  per-binary invariant holds throughout.
- This is **not** a Constitution VII §IV scheme-adoption PR — PR 3c is
  the engine-refactor PR that scheme-adoption PRs (e.g., PR 3b
  walker collapses) build on. §IV's "scheme-adoption PR MUST NOT edit
  engine crates" restriction does not apply here. Commit 2 (and
  commit 4) explicitly edit `marque-rules` and `marque-scheme` because
  PR 3c is exactly the engine-evolution work §IV anticipates.

### Commit 3 — Beachhead: E054 + E057 + E021

**Goal.** Migrate three representative rules to the new emission types,
proving the engine's promotion path round-trips both `FactRemove` and
`FactAdd` correctly under realistic CAPCO inputs. **Commit 3 lands
AFTER commit 5** (renderer body) — see "Commit ordering" amendment
below.

**Scope.**
- **E054** (NOFORN ⊥ RELIDO) — `crates/capco/src/rules_declarative.rs`.
  Migrate from `FixProposal` (span splice with empty replacement +
  separator-eating helper) to
  `FactRemove { token_ref: FactRef::Cve(RELIDO), scope: portion.dissem }`.
  Citation: CAPCO-2016 §H.8 p145 (verified by Agent 1).
- **E057** (ORCON-USGOV ⊥ RELIDO) — same shape as E054 with different
  binding partner. Citation: §H.8 p140 (verified).
- **E021** (AEA → NOFORN) — Migrate from `Severity::Error` no-fix to
  `FactAdd { token: FactRef::Cve(NOFORN), scope: portion.dissem }`.
  Citation: §H.6.

**Acceptance criteria.**
- Each rule emits a `FixIntent<CapcoScheme>` of the named shape on a
  test fixture
- Engine's `fix_inner` consumes the `FixIntent` and produces an
  `AppliedFix` with G13-clean payload (token canonicals + scope IDs only)
- `cargo test -p marque-capco` green
- **Byte-identity gate (User RESOLVED Q2):** snapshot the audit-record
  output from a representative E054/E057 fixture set on the pre-PR-3c
  baseline; rerun on the beachhead branch; diff MUST be empty.
  Both single-trigraph (degenerate, post-removal yields empty REL TO)
  AND multi-trigraph (load-bearing, post-removal yields canonical
  comma-space REL TO) fixtures are required. The multi-trigraph case
  is the load-bearing test; the single-trigraph case alone does not
  exercise `render_canonical`'s sort or delimiter logic. Because
  commit 3 lands after commit 5 in the new ordering, the renderer
  body is available when this gate runs.
- E055 / E056 still emit `FixProposal` — unchanged — and serve as the
  byte-identity comparison baseline for non-migrated rules.

**Tests.**
- Per-rule: `e0XX_emits_correct_shape` — assert the rule fires on a
  known-bad input and produces the expected variant + scope + token
- Engine round-trip: `e0XX_fix_promotes_through_engine` — assert
  `Engine::fix(input)` produces an `AppliedFix` whose `proposal` matches
  expectation
- Audit: `e0XX_audit_g13_closure` — assert no document bytes in the
  promoted record. **Strengthened gate (per architect's review,
  scoped per pass-2 review):** the test enumerates the structural
  envelope of the **in-memory** `AppliedFix.proposal:
  AppliedFixProposal::New(intent)` payload (Rust-side, not JSON-side)
  and fails if any byte outside the structural set (token
  canonicals via `FactRef::Cve`, structural references via
  `FactRef::OpenVocab(CapcoOpenVocabRef)`, `Scope` discriminants,
  `RecanonScope` discriminants) appears in the in-memory payload at
  promotion time. **Scope: `FixIntent`-shape `AppliedFixProposal::New`
  records ONLY**, NOT the legacy-compat NDJSON output and NOT
  `AppliedFixProposal::Legacy(FixProposal)` records (which carry
  document bytes by design — the pre-existing G13 channel that
  closes at commit 10 with `FixProposal` itself). Without this
  scope, the test would fire on every non-migrated rule's audit
  record during commits 3–9. The test is the empirical pin for
  Constitution V Principle V's content-ignorance invariant on the
  new emission path.

**Constitution.** Principle V (audit-first) — the promotion path's
`__engine_promote` constructor is the only legal way to construct an
`AppliedFix`. The new emission shapes flow through it unchanged. The
strengthened g13_closure test pins Constitution V Principle V's
content-ignorance invariant against any future regression.

**LoC estimate.** ~30 LoC per rule × 3 rules (rule body migration) +
~150 LoC test scaffolding + ~50 LoC strengthened g13_closure
content-ignorance assertion = **~290 LoC.**

**Risks.**
- ~~E054's existing span-splice + separator-eating helper~~ —
  RESOLVED by commit ordering (commit 3 now lands after commit 5).
  `render_canonical` is available; the multi-trigraph fixture is
  the load-bearing test, not deferred to commit 6.
- Beachhead rules consume the new `FixIntent` shape while the other 44
  rules still emit `FixProposal`. The `Engine` must dispatch both during
  the transition. Verify the existing engine code path handles
  `Option<FixIntent<S>>` alongside `Option<FixProposal>` on `Diagnostic`;
  if not, commit 3 must extend it. (Per User RESOLVED Q1: yes, both
  fields coexist; commit 2 added the new field; commit 3 wires
  dispatch.)
- ~~E024's atomic multi-remove verification~~ — MOVED to commit 8.
  Commit 3's beachhead does not exercise atomic-cluster promotion;
  E054 / E057 are single-`FactRemove` shapes, E021 is single-`FactAdd`.
  Commit 8 (where E024 actually migrates) is the right place to
  verify multi-remove atomicity. Removed from commit 3 to avoid
  testing infrastructure that has no consumer until ~5 commits
  later.
- **`Recanonicalize` is not exercised by the beachhead.** E054 / E057 /
  E021 are `FactRemove` / `FactRemove` / `FactAdd` only. Rules emitting
  `Recanonicalize` (E002, S003) land in commit 6 without prior
  engine-side validation. Mitigation: the engine's `fix_inner`
  dispatch path for `Recanonicalize` is documented in commit 4's
  trait-surface scope (the engine retrieves the `ProjectedMarking`
  for the named `RecanonScope` from the per-page projection it
  computed during `lint`; no rule provides the projection — the
  engine already has it in scope at fix-application time). Commit 6
  carries an integration test for the first `Recanonicalize`
  consumer; if the dispatch path is missing, commit 6 extends it
  (and surfaces during commit 6's pre-flight, not at integration
  time).

**Commit ordering (amendment).** Per the plan-review finding that
commit 3's byte-identity gate cannot satisfy at its merge time without
commit 5's renderer body present, **the new commit-graph ordering is
1 → 2 → 4 → 5 → 3 within PR 3c.A**. Commits 4 and 5 still depend on
commit 2 (emission types) but no longer depend on commit 3, and
commit 3 now depends on 5 (so the renderer body is available when
the byte-identity gate runs). Commits 4 and 5 may parallelize
internally but commit 5 must merge before commit 3.

### Commit 4 — `render_canonical` trait surface

**Goal.** Add the per-axis canonicalization dispatch surface to
`MarkingScheme` per Decision 5.

**Scope.**
- `crates/scheme/src/scheme.rs` — add:
  ```rust
  fn render_canonical(
      &self,
      m: &Self::Marking,
      scope: Scope,
      out: &mut dyn fmt::Write,
  ) -> fmt::Result;
  ```
  Default impls for `render_portion` / `render_banner` that call
  `render_canonical(m, Scope::Portion, &mut s)` / `Scope::Page`
  respectively, collecting into `String`.
- `crates/scheme/src/scheme.rs` doc updates — describe the writer-passing
  contract and the lattice-equal-byte-identical property.
- `crates/capco/src/scheme.rs` — degenerate `render_canonical` impl that
  delegates to existing `render_portion` / `render_banner` for the scope
  variants. Body lands in commit 5; commit 4 is interface only.
- `AxisRenderRow` data struct (private to `crates/capco/src/`):
  ```rust
  pub(crate) struct AxisRenderRow {
      pub category: CategoryId,
      pub render: fn(&CapcoMarking, Scope, &mut dyn fmt::Write) -> fmt::Result,
  }
  pub(crate) const RENDER_TABLE: &[AxisRenderRow] = &[ /* placeholder rows */ ];
  ```
  Commit 4 declares the type and an empty/placeholder `RENDER_TABLE`;
  commit 5 populates it.

**Acceptance criteria.**
- `cargo check --workspace` green
- `cargo test --workspace` green (all schemes implement the new trait via
  the default-delegating impl)
- `render_portion(m)` and `render_banner(m)` produce byte-identical output
  to the pre-commit baseline (the default impls are pass-throughs)

**Tests.** Trait-existence smoke test: `MarkingScheme::render_canonical`
is callable on `CapcoScheme` with each `Scope` variant.

**Engine-side `Recanonicalize` dispatch contract.** Commit 4 also
documents how `Engine::fix_inner` materializes the `ProjectedMarking`
that `Recanonicalize { scope }` names. The contract: the engine
already computes per-page projections during `lint` (per Constitution
VI's dataflow pipeline). At fix-application time, the engine consults
its in-scope projection for the named `RecanonScope`, then calls
`render_canonical(&projection.marking, scope.into(), &mut writer)`.
Rules never carry the `ProjectedMarking` — the engine is the
authority. This contract is documented inline on `MarkingScheme`'s
`render_canonical` doc comment and re-stated in the `Recanonicalize`
variant's doc comment in `marque-rules`. Commit 4 does NOT exercise
the contract end-to-end (no `Recanonicalize`-emitting rule in PR
3c.A); commit 6 is the first integration test.

**Engine-side per-page scratch buffer.** Per the architect's review,
the `&mut dyn fmt::Write` writer-passing benefit only materializes
when the engine pre-allocates and reuses a scratch buffer across
portions on a page. Commit 4 commits to: the engine's lint loop
holds a per-page reusable `String` scratch buffer (or `Vec<u8>` if
non-UTF-8 carriers land later), threaded through every
`render_canonical` call within a page. Without this, writer-passing
is a marginal improvement, not the SC-001 win Decision 5 cites.

**Constitution.** Principle VII §IV — this is a scheme-trait edit.
Lands inside PR 3c.A (an engine-refactor PR, NOT a scheme-adoption
PR — §IV's restriction does not apply; see commit 2's risks). PR
3b's umbrella explicitly cited the §IV restriction because 3b sub-PRs
were scheme-consumption work; PR 3c is the preceding engine
evolution those sub-PRs depend on.

**LoC estimate.** ~50 LoC trait edit + ~80 LoC delegate impl + ~30 LoC
`AxisRenderRow` declarations + ~30 LoC engine scratch-buffer wiring +
~30 LoC `Recanonicalize` dispatch documentation = **~220 LoC.**

**Commit ordering within 3c.A (clarified per pass-2 review).** Per
architect pass-2: commit 5 imports commit 4's types
(`AxisRenderRow`, `RENDER_TABLE`, `MarkingScheme::render_canonical`)
to populate them with bodies. Commit 5's source code depends on
commit 4's types being declared — they cannot truly compile in
parallel against the same base. **The honest critical path is
linear: `1 → 2 → 4 → 5 → 3`** (4 serial steps, not 5 — commit 3 is
last instead of in the middle). Wall-clock work CAN proceed on
sub-branches: a developer can author commit 5's body in a working
copy that imports a not-yet-merged commit 4 branch, but the merge
sequence into PR 3c.A's linear history is 4 then 5 then 3. The
prior framing ("{4 ∥ 5} parallel") was technically inaccurate;
the new framing names the dependency at the type level while
preserving the work-parallelism observation.

**Risks.** The `&mut dyn fmt::Write` choice (vs `io::Write`) restricts to
ASCII / UTF-8 output. CAPCO is ASCII-only per §A.6; partner-scheme markings
(NATO, partner-national) may need byte-level output in the future. The
risk is YAGNI for PR 3c; a future scheme would add a per-scheme
writer-trait associated type if needed.

### Commit 5 — `render_canonical` body + test harness

**Goal.** Populate the per-axis dispatch table with real canonicalization
functions, one per CAPCO category. Land the renderer test harness.

**Scope.**
- `crates/capco/src/render/` (new module directory) — one file per axis:
  - `render_classification.rs` (US, JOINT, FGI, NATO class)
  - `render_dissem.rs` (banner/portion form choice per Scope)
  - `render_rel_to.rs` (USA-first alpha sort, comma-space delimiter,
    trigraph dedup)
  - `render_sci.rs` (numeric-then-alpha sort, hyphen + space delimiters,
    SCI compositional grammar)
  - `render_sar.rs` (single indicator, slash-separated programs,
    per-program compartment sort)
  - `render_aea.rs` (RD/FRD/TFNI precedence, SIGMA numeric sort)
  - `render_fgi.rs` (concealed vs explicit list; tetragraph expansion)
  - `render_non_ic_dissem.rs` (banner forms; LIMDIS/SBU/EXDIS/NODIS)
  - `render_declassify.rs` (CAB position; date-max canonicalization)
  - additional axes as `Category` enumerates them (verify against
    `crates/capco/src/scheme.rs:36-49`)
- `crates/capco/src/scheme.rs` — populate `RENDER_TABLE` with one
  `AxisRenderRow` per category, sorted by `Category::ordering_rank`
  for §A.6 Figure 2 sequence
- `crates/capco/src/scheme.rs` — wire `CapcoScheme::render_canonical` to
  iterate `RENDER_TABLE` and dispatch
- `crates/capco/tests/parse_render_roundtrip.rs` — re-enable the
  `#[ignore]`'d full-attribute round-trip test at line 420 (per
  decisions/03-empirical-concerns.md D7)
- `crates/capco/tests/render_canonical_properties.rs` (new) — two
  property tests:
  - `round_trip_idempotent`: `render(parse(render(parse(x)))) ==
    render(parse(x))` for a corpus of canonical-form fixtures
  - `lattice_equal_renders_byte_identical`: pairs of inputs differing only
    by form (delimiter, sort, abbreviation) render to the same bytes

**Acceptance criteria.**
- `cargo check --workspace` green
- `cargo test -p marque-capco` green; both new property tests pass
- Re-enabled round-trip test passes
- `render_portion(m)` and `render_banner(m)` outputs change to canonical
  form (now driven by `render_canonical` instead of delegating); existing
  tests asserting specific portion/banner output strings may need
  fixture updates if their inputs were non-canonical
- `bench-check.sh` — no regression beyond noise threshold (current
  19× headroom on SC-001 absorbs any reasonable cost)

**Tests.** Two property tests (above) + per-axis golden-output tests
(~10 fixtures per axis, ~10 axes = ~100 fixtures total).

**Constitution.** Principle I — `render_canonical` is on the hot path.
Verify per-axis bench post-merge; if any axis adds >5% to SC-001 latency,
flag for optimization (probably zero — per-axis dispatch is the same
operations the current code performs, just relocated).

**LoC estimate.** ~80 LoC per axis × ~10 axes (render body) + ~200 LoC
property-test scaffolding + ~150 LoC fixture data + ~50 LoC scheme.rs
wiring = **~1200 LoC.** This is the largest commit in 3c.A.

**Risks.**
- Some form rules in the audit (E001, E009, E013, E029, E030, E032,
  E052) currently encode canonicalization logic in their `Rule::check`
  bodies. Per the audit (`rule-body-audit.md`) and Decision 1, ~22
  of 47 rules are mis-housed precisely because they encode
  canonicalization wrongly or at the wrong granularity. **Verification
  oracle is CAPCO-2016 §H, NOT retiring rule code** (Constitution
  VIII): commit 5's per-axis fns reproduce the canonical forms
  defined by the manual, verified by per-axis golden-output fixtures
  hand-written from §H sections (REL TO §H.8 p150-151 USA-first alpha,
  comma-space; SCI §H.4 p61 numeric-then-alpha; SAR §H.5 p99 program
  ascending; AEA §H.6 p108 SIGMA numeric sort; etc.). Per-rule pre/post
  byte-identity diffs against retiring rules are EXPECTED to differ
  (that's why the rules retire) and are documentation, not a
  pass/fail gate. The pass/fail gate is "does the renderer match
  the manual."
- The per-axis golden-output fixtures are derived from CAPCO-2016 §H
  directly (independent oracle), with cross-checks against the ODNI
  ISM XML schema where the manual is silent. Each axis's fixture
  file carries the §-citation in a header comment (Constitution VIII
  citation discipline applies to fixtures too).
- E060's walker rules (5 form sub-rules) encode delimiter / sort logic
  that must be in the renderer post-commit-5. Verify that
  `render_canonical` produces output that would make E060's walker
  bodies a no-op on canonical input. Walker itself retires in commit 6
  of 3c.B. The retirement is not contingent on byte-identity to the
  walker — it's contingent on the renderer producing canonical output
  per §H.

## PR 3c.B — Migration execution (5 commits)

Goal: retire form rules into the renderer; decompose walkers; complete
the conflicts / requires bucket migration; provision E005 / S005 / S006
as `Constraint::Custom`. After 3c.B merges, the rule catalog is the
post-architectural-restatement steady state minus the deferred follow-ups.

### Commit 6 — Form-bucket migration

**Goal.** Retire 16 form rules + walker E060 (5 rows) into the renderer.
Each rule's body either deletes entirely or contracts to a
`Recanonicalize { scope }` divergence emit.

**Scope.**
- **Rules that retire entirely** (renderer absorbs by construction; no
  residual emit needed per `rule-body-audit.md:128-143`):
  - E001 (portion-mark-in-banner; banner abbrev choice)
  - E003 (misordered-blocks; §A.6 ordinal)
  - E004 (separator-count; `////+` collapse)
  - E009 (portion abbreviation; mirror of E001)
  - S001 (prefer banner abbreviation)
  - S002 (banner consistent form; falls out by construction)
  - E011 (missing non-US prefix; `//` is structural)
  - E013 (delimiter mismatch; per-axis delimiter choice)
  - E026 (SAR portion form; `SAR-` is canonical)
  - E029 (SAR compartment order)
  - E030 (SAR indicator repeat)
  - E032 (SCI system order; sub-compartment order)
  - E052 (REL TO no duplicates; set-canonicalization)
  - **E060 walker** (5 form rows: REL TO, JOINT, SIGMA, SAR, SCI sort)
- **Rules that contract to `Recanonicalize { scope }` emit** (rule still
  fires on input-form divergence; renderer re-renders):
  - E002 (missing USA trigraph) — `Recanonicalize { rel_to }` for
    USA-not-first; **also** emits `FactAdd { USA, scope: rel_to }` for
    USA-missing (the multi-purpose split per audit row E002). One rule,
    two emit shapes depending on which branch fires.
  - S003 (JOINT USA-first) — optional convention layered above renderer;
    stays as `Recanonicalize` emit gated by config
- **Rule that splits** (audit row E032): SCI system-order retires; the
  sub-compartment-order sub-shape consolidates into E060's retirement
  in this commit

**Acceptance criteria.**
- `cargo check --workspace` green
- `cargo test -p marque-capco` green
- 14 form rules deleted from `crates/capco/src/rules.rs` /
  `rules_declarative.rs`; their tests deleted or contracted to renderer
  property tests
- E002 and S003 emit new shape; their tests updated
- E060 walker deleted; the `NonCanonicalInputCatalog` data structure
  retired
- Registered rule count drops from 47 to ~32 (14 deleted + walker
  retirement = -15; +0 added = 47 - 15 = 32). The `corpus_parity.rs:170`
  count pin updated.
- Bench-check no regression

**Tests.** Renderer property tests from commit 5 absorb most of the
retiring rules' coverage. Per-rule retirement requires deleting the
rule's existing tests OR contracting them to assert the renderer
produces canonical form on the test's input.

**Constitution.** Principle IV — Layer 2 (hand-written rules) contracts
significantly. Per-axis canonicalization in the renderer is still
Layer 2 (in `crates/capco/src/render/`), just decoupled from the
divergence-detection layer.

**LoC estimate.** ~14 rule-body deletes (~50 LoC each = 700 LoC removed)
+ walker delete (~200 LoC removed) + E002/S003 rewrites (~80 LoC) +
test updates (~300 LoC) = **net -600 to -800 LoC removed.**

**Risks.**
- Some form rules emit non-form fixes as a side effect (E002's
  `FactAdd { USA }` is the named multi-purpose split). Audit each
  retiring rule for hidden side-channels before delete. Specifically
  re-verify rule-body-audit rows for E001 / E003 / E004 / E009 / S001 /
  S002 / E011 / E013 / E026 / E029 / E030 / E032 / E052 — any column
  marked "multi-purpose split required" or with a non-form
  ProjectedMarking axis listed.
- Audit-record byte-identity (SC-008) may shift on these rules because
  their `FixIntent` payload changes from byte-splice to `Recanonicalize`
  or no-emit. Re-snapshot SC-008 baseline at commit 6's end; if any
  consumer relied on the old shape, surface as a separate finding.

### Commit 7 — Walker decomposition + `ConstraintViolation` extension

> **AMENDED 2026-05-11** — the preflight architect review on branch
> `pr3c-c-commit7` surfaced five plan-gaps that materially restructure
> this commit. The original 3-subcommit shape (7.1 / 7.2 / 7.3) is
> replaced by a 4-subcommit shape (7.1 / 7.2 / 7.3 / 7.4) split across
> two PRs. The end-state goals (~1700 LoC net, two walkers retired, 32
> catalog rows fire through the constraint catalog) are unchanged.
> See `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`
> for the five amendments and the re-numbered subcommit sequence.
>
> Headline corrections to the section below: (a) the `corpus_parity.rs:170`
> delta is **-2** (33 → 31), not +30 — the catalog rows aren't
> registered Rule impls; (b) extension is `Option<Span>` / `Option<Severity>`,
> not bare; (c) `FixIntent<S>` cannot live on `ConstraintViolation`
> (Constitution VII cycle), uses scheme-side
> `CapcoScheme::fix_intent_by_name(...)` helper instead; (d) the
> engine bridge is a new subcommit (7.2) — the production lint path
> does not call `scheme.validate(...)` today; (e) walker deletion is
> staged behind a `scheme_equivalence.rs` byte-identity test.

**Goal.** Decompose E058 (27 class-floor rows) and E059 (5 SCI per-system
rows) into per-row `Constraint` entries on `CapcoScheme`. Per user's
locked answer to question 3: the `ConstraintViolation` extension is the
**first subcommit** (it's the engine-side trait edit that scheme-side
consumers depend on).

**Scope.**

**Subcommit 7.1 (first):** `ConstraintViolation` extension on
`marque-scheme`.
- `crates/scheme/src/constraint.rs:155-160` — extend
  `ConstraintViolation` from its current minimal shape to carry:
  - `span: Span` (so the inlined constraints produce diagnostic location)
  - `severity: Severity` (so per-row severity can differ from the
    walker's `default_severity`)
- `crates/rules/src/lib.rs` — re-export the extended `ConstraintViolation`
  shape if needed
- Trait-edit lands in isolation; no scheme-side consumer yet

**Subcommit 7.2:** E058 decomposition.
- Inline the 27 class-floor rows from `CLASS_FLOOR_CATALOG` (currently in
  `crates/capco/src/scheme.rs:~3641-3893` per the prior PM brief) as
  individual `Constraint::Custom("class-floor/<marking>", …)` entries
  with their per-row §-citations and per-row severity
- Delete `DeclarativeClassFloorRule` from `crates/capco/src/rules_declarative.rs`
- Each row gets a derived rule ID (e.g., `E058/<marking>`) so severity
  config can target individual rows; alternative numeric range is a
  policy choice for commit 7

**Subcommit 7.3:** E059 decomposition.
- Inline the 5 SCI per-system rows as individual `Constraint` entries.
  Per audit row E059, these mix Requires (companion-required) and
  Conflicts (forbid-companion); split into two catalogs:
  - `requires/<marking>-companion` (4 rows, FactAdd shape)
  - `conflicts/<marking>-forbid` (1 row, FactRemove shape — HCS-P sub
    vs ORCON-USGOV)
- Delete `DeclarativeSciPerSystemRule`

**Acceptance criteria.**
- `cargo check --workspace` green
- `cargo test --workspace` green; all 32 retiring rows now fire as
  per-row constraint violations with correct span + severity
- Registered rule-ID count drops on rule-set, increases on
  per-row-IDs: net effect on `corpus_parity.rs:170` is +30 (each row
  is its own rule ID per `decisions/02-catalog-shape.md` D2 lock)
- `exact_rule_id_set` pin at `crates/capco/tests/post_3b_registration_pin.rs`
  updated

**Tests.** Each row's existing test (currently asserting walker fires
on input X) becomes a constraint-violation test asserting the inlined
constraint fires on the same input with the same severity + citation.
~32 tests migrated; no new test coverage added beyond the migration.

**Constitution.** Principle IV — declarative `Constraint` entries are
the natural Layer 2 shape for these rules. The walker pattern was
transitional; inlining ratifies the architectural commitment.

**LoC estimate.** ~50 LoC trait edit + ~30 LoC per row × 32 rows + ~80
LoC walker deletes + ~100 LoC test migration = **~1700 LoC net** (mostly
relocation, not addition).

**Risks.**
- The `ConstraintViolation` extension may require updates to every
  scheme that currently implements `MarkingScheme::validate` (the
  default impl that produces `ConstraintViolation`s). Pre-flight: Grep
  every `impl MarkingScheme` in the workspace for `validate` overrides.
- Per-row severity preservation: E058's 27 rows have heterogeneous
  severity per the audit. Verify the per-row severity values are
  carried through the inlining; sample 5 rows for byte-identity check
  against the walker's pre-decomposition diagnostic output.

### Commit 8 — Conflicts / Requires bucket completion

**Goal.** Migrate the remaining conflicts and requires rules
(non-beachhead, non-walker). Per `decisions/01-migration-sequencing.md`
D1, this is the bulk of the structural migration after foundations.

**Scope.** ~10 rules per Agent 1's count:
- **Conflicts** (FactRemove shape): E016, E024 (multi-remove), E036,
  E037, E055, E056, E041
- **Requires** (FactAdd shape): E010, E012, E014, E015, E038, E053
  (subsumed by `noforn-clears-rel-to` page rewrite — verify before
  migration)
- **Page-rewrite shape** (declared as `PageRewrite` on `CapcoScheme`,
  not rule emission): E039 (NODIS/EXDIS clears banner REL TO), W003
  (classification ≥ C clears non-IC dissem subset). These rules retire
  to declarations on `CapcoScheme::page_rewrites()`; the divergence
  diagnostic falls out of input-vs-projected on the relevant axis.

**Acceptance criteria.**
- Each migrated rule emits the audit-named shape on a fixture
- Engine round-trip produces correct `AppliedFix`
- E024's atomic multi-remove (per `followups/no-fix-auto-apply-calibration.md`
  Verification gate step 6) round-trips: both `FactRemove { FRD }` and
  `FactRemove { TFNI }` land or neither lands
- `cargo test -p marque-capco` green
- Audit-record byte-identity preserved against pre-PR-3c snapshot for
  rules whose fix shape doesn't change (E024 multi-remove may shift —
  document)

**Tests.** Per-rule emit-shape test (~10 rules × ~30 LoC = 300 LoC).
Engine round-trip already covered by commit 3's beachhead infrastructure.

**Constitution.** Principle V — all promotions go through
`__engine_promote`; G13 closure preserved.

**LoC estimate.** ~50 LoC rule migration × ~10 rules + ~50 LoC PageRewrite
declarations × 2 + ~300 LoC tests = **~850 LoC.**

**Risks.**
- E024's multi-remove is the structural novelty. The engine's
  promotion path may not currently support "atomic cluster" promotion
  (both `FactRemove`s land or neither). Verify in commit 3's
  beachhead-test infrastructure (or extend in commit 8 if needed).
  **Update (2026-05-11):** the engine-side fix lands as GitHub issue
  #348 (SmallVec extension to
  `ReplacementIntent::FactRemove`), a standalone PR scheduled to land
  *before* Sub-PR 8.C drafts E024's migration. See
  `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`
  for the Stage-4 umbrella this prerequisite eventually feeds into.
- E053's subsumption-into-`noforn-clears-rel-to` page rewrite assumes
  that page rewrite is declared on `CapcoScheme` today. Verify; if not
  declared, commit 8's scope expands to include declaring it. (If
  E053 retires, end-state rule count drops from 63 to 62; rule-count
  arithmetic table reflects the assumption that it does retire.)

**CLI ↔ WASM byte-identity harness scoping (commit 8 sub-commit, per
pass-2 review).** The PR 3c.B verification gate "CLI and WASM produce
identical NDJSON output for shared fixture set" requires a
fixture-capture harness that compares both outputs as byte strings.
Pre-flight on commit 6: `ls crates/wasm/tests/ crates/marque/tests/`
and confirm whether such a harness exists (likely not — current
WASM tests run via `wasm-bindgen-test` against a JS callback;
current CLI tests capture stdout via `assert_cmd`). If absent,
commit 8 adds it as a sub-commit before the conflicts/requires
migration begins:
- New `tests/cli_wasm_parity/` directory at workspace root (or
  `crates/marque/tests/parity_with_wasm.rs` if simpler) running both
  paths against shared fixtures and diffing the captured NDJSON.
- ~150 LoC harness scaffolding (fixture loader, NDJSON normalizer,
  diff reporter) added to commit 8's LoC estimate.
- If the harness already exists in some other form, commit 8 wires
  the shared fixture set into it instead of re-creating.
"Inspect both manually" is not an acceptance criterion; without an
automated harness the gate is unverifiable.

### Commit 9 — Provisional `Constraint::Custom` for E005 / S005 / S006

**Goal.** Per `decisions/02-catalog-shape.md` D4, the three no-clean-fit
rules get provisional `Constraint::Custom` entries with retirement-
target comments in source. E005 targets `Recanonicalize { Document }`
once renderer can position declass in CAB; S005/S006 target the
admonition channel (deferred per `followups/admonition-channel.md`).

**Scope.**
- E005 (`crates/capco/src/rules.rs` declassify-misplaced) — migrate to a
  `Constraint::Custom("declassify-misplaced", …)` entry on `CapcoScheme`
  with the multi-span detection logic preserved. Source comment names
  retirement target: "Retires to `Recanonicalize { scope: Document }`
  once `render_canonical` can position declass in CAB by construction
  (per CAPCO §E.1 + §E.2). Tracker:
  `followups/[no spec yet — name the issue]`."
- S005 (`crates/capco/src/rules.rs` rel-to-opaque-uncertain-reduction) —
  migrate to `Constraint::Custom("rel-to-uncertain", …)`. Source
  comment names admonition channel as retirement target;
  cross-reference `followups/admonition-channel.md`.
- S006 (sister rule) — same shape as S005

**Acceptance criteria.**
- Three rules retire from `rules.rs` proper, land as `Constraint::Custom`
  entries on `CapcoScheme`
- Each constraint emits the same diagnostic shape as the retired rule
- Source comments explicit about retirement target
- Registered rule count after commit 9: see arithmetic table below.
  Verify with `corpus_parity.rs:170` count pin AND
  `post_3b_registration_pin.rs` exact-rule-ID-set pin per commit.

**Rule-count arithmetic (per commit, post-pass-2 review).** Starting
count post-PR-3b is **47**. Numbers below traced line-by-line to
`rule-body-audit.md`, not synthesized.

| Commit | Action | Net Δ | Running count |
|---|---|---|---|
| 6 (form bucket) | Retire 13 form rules entirely (E001, E003, E004, E009, E011, E013, E026, E029, E030, E032, E052, S001, S002) + retire E060 walker (5-row form walker, the renderer absorbs all 5 rows). E002 contracts to `Recanonicalize` and stays registered; S003 stays as configurable convention layered above §H.3. | −14 | 33 |
| 7.1 (`ConstraintViolation` trait edit) | Engine-side trait edit on `marque-scheme`; no registered-rule change. | 0 | 33 |
| 7.2 (E058 catalog inline) | Retire walker E058 (−1); add 27 per-row IDs as `E058/<row-name>` (+27). | +26 | 59 |
| 7.3 (E059 catalog inline) | Retire walker E059 (−1); split into requires/conflicts catalogs, 4 + 1 = 5 per-row IDs (+5). | +4 | 63 |
| 7.4 (banner walker decomp) | Retire walker (top-level Rule::id() = E031, the carrier walker for E031/E035/E040 per PR 3b.A) (−1); add 3 per-row IDs (E031, E035, E040) (+3). | +2 | 65 |
| 8 (conflicts/requires migration) | Rules migrate emission types but stay registered (no count change). E039 and W003 retire to `PageRewrite` declarations on `CapcoScheme` (−2). E053 conditional: if subsumption into `noforn-clears-rel-to` PageRewrite verifies, additional −1 (final count then 62). Plan assumes E053 retires; if it doesn't, end state is 63. | −3 | 62 |
| 9 (provisional `Constraint::Custom` for E005 / S005 / S006) | Rules retire from `rules.rs` proper; `Constraint::Custom` entries count as registered rules per PR 3b.D/E precedent. | 0 | 62 |
| 10 (`FixProposal` cleanup + schema bump) | No registered-rule change. | 0 | 62 |

**End-state count after PR 3c.B is 62** (or 63 if E053 doesn't
retire). The previous estimates (53 in v1, 59 from earlier draft
arithmetic) were both wrong. Architect's pass-2 recompute (63) and
code-reviewer's pass-2 finding (count doesn't reconcile with audit)
both flagged the v1 number; this rebuild traces every delta to a
specific `rule-body-audit.md` row.

CLAUDE.md "end-state target ~10 surviving rules across all four
stages remains binding" — the 62-count after PR 3c is the running-
down target for PRs 4 and 5+ (Stages 3 and 4 of the engine
refactor), where (a) the renderer absorbs more form rows, (b) the
admonition channel absorbs S004/S005/S006/W002/W034, and (c) the
class-floor and SCI-per-system catalogs flatten further as the
constraint evaluator gains expressive power. The 62-rule
intermediate state preserves the diagnostic-surface granularity
that downstream consumers (severity overrides, audit consumers)
depend on; the further reduction is a multi-PR roadmap, not
this PR's scope.

The per-commit pin update instruction (Verification Gates section
below) covers BOTH pins (count + exact ID set) per commit; a wrong
pin breaks CI on the commit that introduced the drift, not later.

**Tests.** Existing rule tests migrate to constraint-violation tests
(same shape as commit 7 row migrations).

**Constitution.** Principle V — `Custom` is the principled exception,
not a junk drawer. Each has a written retirement target with a named
trigger condition.

**LoC estimate.** ~100 LoC rule migrations (three rules, modest
complexity per E005's multi-span logic) + ~30 LoC retirement-target
comments + ~80 LoC test migration = **~210 LoC.**

**Risks.**
- E005's multi-span logic doesn't map cleanly to `Constraint::Custom`'s
  detection contract (likely a single-fact predicate). Verify the
  constraint can express "declass token appears in
  banner.declassify_on OR portion.declassify_on instead of
  cab.declassify_on." If the predicate shape is too restrictive,
  commit 9 expands to allow multi-fact `Constraint::Custom` (which is
  itself a `marque-scheme` trait edit that should land alongside
  commit 7's `ConstraintViolation` extension — surface during
  pre-flight on commit 7).

### Commit 10 — `FixProposal` cleanup + audit-schema bump (atomic cutover)

**Goal.** Per Decision 9 Path C amendment + User RESOLVED Q1, retire
the transitional both-fields shape on `Diagnostic`, retire the
`AppliedFixProposal::Legacy` enum variant, retire the engine-side
`fix_intent_to_legacy_proposal` conversion, AND atomically flip the
`MARQUE_AUDIT_SCHEMA` default to `"marque-mvp-3"` with the new
`FixIntent`-shape JSON. The cutover is atomic — every audit record
emitted post-commit-10 is `marque-mvp-3` shape, every record
pre-commit-10 was `marque-mvp-2` shape. FR-014's single-shape-per-
binary invariant holds across the boundary.

**Scope.**

*Type-level cleanup:*
- `crates/rules/src/fix_intent.rs` — delete the `FixProposal` struct
  and its impl block.
- `crates/rules/src/applied_fix.rs` (or wherever `AppliedFixProposal`
  lives) — collapse the `AppliedFixProposal<S>` enum back to a plain
  `pub proposal: FixIntent<S>` field on `AppliedFix<S>`. The enum
  was a transitional shape; with `FixProposal` gone, the
  `AppliedFixProposal::New(intent)` wrapping is unnecessary.
- `crates/rules/src/diagnostic.rs` (or wherever `Diagnostic` lives) —
  remove `fix: Option<FixProposal>`; rename `fix_intent:
  Option<FixIntent<S>>` to `fix: Option<FixIntent<S>>` (the
  ergonomic name is reclaimed once the legacy type is gone).
- `crates/engine/src/lib.rs` (or wherever `AppliedFix` lives) —
  `AppliedFix::__engine_promote` signature: drop the `FixProposal`
  parameter; takes `FixIntent<S>` only. Retire
  `__engine_promote_legacy`. Retire the engine-side
  `fix_intent_to_legacy_proposal` conversion helper.

*Audit-schema bump (NEW in commit 10):*
- `crates/engine/src/...` — extend `MARQUE_AUDIT_SCHEMA` build-time
  env-pin's closed accept-list to `["marque-mvp-1", "marque-mvp-2",
  "marque-mvp-3"]`. Default flips to `"marque-mvp-3"`.
- `crates/engine/src/AUDIT_SCHEMA_VERSION` constant updates.
- `crates/wasm/src/lib.rs::diagnostic_to_json` and
  `crates/wasm/src/lib.rs::applied_fix_to_audit_json_v2` — rewrite
  to read the `FixIntent` variants directly. The legacy field-by-
  field reads (`fix.proposal.span`, `.original`, `.replacement`,
  `.source`) are deleted.
- `crates/server/...` — same rewrite of the audit-emit path.
- CHANGELOG entry: `marque-mvp-2 → marque-mvp-3` shape change with
  the rationale (PR 3c bag-of-tokens architectural restatement),
  the field set on each `FixIntent` variant, and a migration note
  for any future external consumer.

*Test fixture migration:*
- `crates/rules/tests/`, `crates/capco/tests/`, all integration tests
  using `__engine_promote` for fixture construction — migrate to the
  `FixIntent<S>`-only signature. Constitution V test-fixture carve-
  out comments preserved.

**Acceptance criteria.**
- `grep -rn 'FixProposal' crates/ --include='*.rs'` returns empty.
- `grep -rn 'AppliedFixProposal' crates/ --include='*.rs'` returns
  empty (the transitional enum is also gone post-commit-10).
- `grep -rn 'fix_intent_to_legacy_proposal' crates/ --include='*.rs'`
  returns empty.
- `MARQUE_AUDIT_SCHEMA=marque-mvp-3 cargo build` produces a binary
  whose audit output is `FixIntent`-shape JSON.
- `cargo +stable check --workspace` green.
- `cargo +stable test --workspace` green.
- `cargo +stable clippy --workspace -- -D warnings` green.
- `bench-check.sh` within thresholds.
- The strengthened `g13_closure` test from commit 3 now applies
  system-wide (no scope restriction needed — every `AppliedFix`
  carries `FixIntent<S>` directly, the structural envelope is
  uniform).
- CHANGELOG entry committed.

**Tests.** Existing tests pass; the `g13_closure` test scope widens
to system-wide (was: `FixIntent`-shape only during transition).

**Constitution.** Principle V — `__engine_promote` signature change
preserves the engine-only invariant (signature still
`pub #[doc(hidden)]`, only callable from `Engine::fix_inner` and the
`cfg(test)` carve-out call sites). G13 closure now structurally
enforced for every audit record (no legacy bytes channel).

**LoC estimate.**
- Source deletions: ~−650 LoC (`FixProposal` struct + impls,
  `AppliedFixProposal` enum, `__engine_promote_legacy` constructor,
  `fix_intent_to_legacy_proposal` helper, legacy serializer branches,
  transitional re-exports).
- Source additions: ~+80 LoC (rewritten WASM/server serializers
  reading `FixIntent` variants — they were structurally unable to
  do this in commit 2 because `FixIntent`'s variants didn't carry
  `span` directly, and Path C deferred the rewrite to here).
- Test migrations: ~+150 LoC (fixture sites that constructed
  `FixProposal` for tests now construct `FixIntent` variants — the
  ones that used `__engine_promote_legacy` migrate to
  `__engine_promote`).
- CHANGELOG entry: ~+30 LoC.
- **Net ~−390 LoC.**

**Risks.**
- A `FixProposal` consumer slipped through earlier commits (e.g., a
  test fixture in a crate not in the workspace member list, or a
  `dev-dependencies`-gated utility). Mitigation: pre-flight
  `rg 'FixProposal' --type rust --hidden` (search hidden
  directories) before the deletion sub-commit; resolve before
  proceeding.

## Out-of-scope follow-ups (post-PR-3c)

Tracked at:
- `specs/006-engine-rule-refactor/followups/no-fix-auto-apply-calibration.md`
  — D3's middle-path: 6 rules' auto-apply thresholds + 24 calibration
  tests (~210–360 LoC, single-commit follow-up after PR 3c.B merges)
- `specs/006-engine-rule-refactor/followups/admonition-channel.md`
  — D10's deferral: structural diagnostic channel for advisory markings.
  Trigger condition: first concrete consumer or second admonition-shape
  rule.
- `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`
  — Stage-4 (PR 5+) consolidation: `Constraint::Incompatible` umbrella
  primitive unifying conflict-family rules (E016/E024/E036/E037/E039/
  E041/E054/E055/E056/E057). Three-category taxonomy (A.1 single-fact
  removal, A.2 chain-removal cluster, A.3 transmute via foreign-
  equivalence map, B genuine mutual exclusion). Approx −10 rules net
  toward the 8–18 target band. **Near-term prerequisite — SmallVec
  extension to `ReplacementIntent::FactRemove` for Sub-PR 8.C — is
  separated out as GitHub issue #348** and lands before 8.C
  drafts E024's catalog row.

Open queue (from 2026-05-10 review):
- **Items 4–7 (short-term, during PR 3c.B):** `Verification` subsection
  in architecture.md; one working test against `Engine::fix`; iteration
  cap + failure mode for closure operator; row-count column in audit
  table.
- **Items 8–10 (long-term, post-PR-3c):** Glossary / decision-index;
  inline §3.0.b definitions; confidence-calibration policy doc.

## Verification gates

### Per-commit (every commit in 3c.A and 3c.B)
- `cargo +stable check --workspace` green (or no regression from baseline)
- `cargo +stable test --workspace` green (or documented exemption with
  fixture-update rationale)
- `cargo +stable clippy --workspace -- -D warnings` green
- `bench-check.sh` within thresholds
- GPG-signed commit (per project standing requirement)
- Commit message conforms to project style

### Per-PR (3c.A and 3c.B before merge)
- Pre-flight rust-reviewer + code-reviewer agents dispatched and addressed
- All CRITICAL and HIGH findings resolved
- PR description includes test plan + Constitution VIII citation
  re-verification statement
- Branch up-to-date with `staging`
- CI green (including `tools/citation-lint/`)

### PR 3c.B-specific
- Audit-record byte-identity (SC-008) snapshot preserved or
  documented-shifted for each migrated rule. **Important:** the
  SC-008 baseline is re-snapshotted at end of commit 6 so commits 7,
  8, 9, 10 measure against the post-commit-6 baseline. To prevent
  unintended drift baking in, **each commit in 3c.B carries a
  per-commit byte-identity check against the immediately-prior
  commit's baseline** (not just the post-commit-6 baseline). A drift
  is only acceptable if it matches the commit's documented intentional
  changes; otherwise it's a regression.
- **CLI ↔ WASM byte-identity cross-check at end of PR 3c.B.** The
  `applied_fix_to_audit_json_v2` paths in `crates/marque/src/main.rs`
  (CLI) and `crates/wasm/src/lib.rs` (WASM) must produce identical
  NDJSON output for a shared E054 / E057 / E021 / E024 / E058-row /
  E060 fixture set (covering FactRemove, FactAdd, multi-FactRemove,
  Constraint::Custom, walker-row variants). This is the SC-008 parity
  promise the project already makes; PR 3c.B must not silently
  bifurcate it.
- `corpus_parity.rs:170` rule-count pin updated correctly per commit
- `post_3b_registration_pin.rs` exact-rule-ID-set pin updated per
  commit (catches "rule X renamed to rule Y at the same count" — the
  count pin alone misses this drift class).
- **`FixProposal` fully removed from `marque-rules` (commit 10).**
  `grep -rn 'FixProposal' crates/ --include='*.rs'` returns empty.
  This is the closing gate of PR 3c.B and the empirical confirmation
  of User RESOLVED Q1's "removed in final cleanup commit" promise.

## Risk register

| Risk | Severity | Mitigation |
|---|---|---|
| PR 3c.1's `EngineConstructor::build_open_vocab` has consumers we haven't audited | High | Pre-flight Grep before commit 2; retire consumers alongside |
| Multi-remove (E024) needs engine-side atomic-cluster support | High | Verify in commit 8 (where E024 actually migrates); extend if absent. **Moved from commit 3 (no consumer there).** |
| `Diagnostic<S>` generic spread reaches more call sites than expected | High | Commit 2 pre-flight Grep `Diagnostic[<( ]\|fn check\b` to enumerate; if >80 sites, consider `dyn Trait`-erasing alternative |
| WASM/server JSON-shape change in `AppliedFix.proposal` (Decision 9) | RESOLVED via Path C | Schema bump deferred to commit 10 (atomic with `FixProposal` cleanup). Commits 2–9 keep `marque-mvp-2` shape; engine-side `fix_intent_to_legacy_proposal` helper synthesizes legacy JSON from `FixIntent` for migrated rules. FR-014 single-shape-per-binary invariant holds throughout. |
| Engine-side `fix_intent_to_legacy_proposal` conversion has gaps (Path C addition) | Medium | Conversion is total: `FactRemove` → `FixProposal { span: <removed-token-span>, replacement: "", ... }`; `FactAdd` → `FixProposal { span: <insertion-point>, replacement: "<rendered-canonical-bytes>", ... }`; `Recanonicalize` → `FixProposal { span: <scope-span>, replacement: <render_canonical(scope)>, ... }`. All three paths exist today (the retiring rules' bodies do exactly this). Pre-flight on commit 2: confirm conversion produces byte-identical legacy JSON for the beachhead rules' fixtures. |
| `AppliedFixProposal<S>` enum collapses asymmetrically at commit 10 | Low | Commit 10's collapse from `AppliedFixProposal::New(intent)` back to plain `FixIntent<S>` is mechanical (a sed-equivalent). Risk only if any consumer pattern-matches on the enum variant explicitly; pre-flight grep `AppliedFixProposal::` before commit 10. |
| Form rules have hidden non-form side effects (multi-purpose splits) | Medium | Pre-flight audit row re-read for each form rule in commit 6 |
| `ConstraintViolation` extension breaks every existing scheme impl | Medium | Pre-flight Grep `validate` overrides; coordinate with subcommit 7.1 |
| Renderer regressions slip past property tests | Medium | Per-axis golden-output fixtures derived from CAPCO-2016 §H (NOT from retiring rule code); each fixture file headers carry §-citation |
| Audit-record byte-identity (SC-008) regression on retired form rules | Medium | Re-snapshot SC-008 baseline at end of commit 6; **per-commit byte-identity check against immediately-prior commit's baseline** (not just post-commit-6) catches mid-flight drift |
| `Recanonicalize` dispatch contract breaks at first integration test | Medium | Commit 4 documents the engine-side `ProjectedMarking` retrieval; commit 6 is first end-to-end test; pre-flight on commit 6 catches gaps |
| SC-001 latency regression at commit 5's renderer body | Medium | `bench-check.sh` per commit; if any axis adds >10% (not 5% — the prior threshold was advisory and toothless), commit 5 halts pending optimization. 19× headroom absorbs reasonable cost; >10% is unreasonable. |
| Decoder-path `ParsedAttrs` cannot source `ProjectedMarking` for `FactRef::Cve` materialization | Medium | Commit 6 first decoder-recognized rule consuming `FactRemove` exercises this; if decoder-path projection isn't present at fix_inner time, commit 6 extends or scope expands |
| Walker decomp commit 7 mid-flight stall (subcommit 7.2 discovers row needs `ConstraintViolation` axis not in 7.1) | Medium | Subcommit 7.2/7.3/7.4 author pauses, extends 7.1 trait edit, then resumes row inlining. Partial-inlining commits MUST NOT land. |
| E005's `Constraint::Custom` predicate shape too restrictive | Low | Pre-flight on commit 7 surfaces the multi-fact-Custom requirement if it exists |
| `S::OpenVocabRef` bounds (`Debug + Clone + Eq + Hash + Send + Sync + 'static`) too restrictive for a future scheme's open-vocab carrier | Low | Bounds match the deriveable property set on `FactRef<S>` and the project's Send+Sync invariant; relaxing is a forward-compatible change |

## Open questions — RESOLVED

User's answers (2026-05-10):

1. **Engine's `Diagnostic.fix` shape during the transition** — RESOLVED: **YES,
   carry `Option<FixIntent<S>>` alongside `Option<FixProposal>` during the
   transition.** Commit 2 adds the new field; commits 6/8 retire the old
   field per-rule as each rule migrates. `FixProposal` is removed in the
   final cleanup commit of PR 3c.B once no rule constructs it. This is
   pre-users (per `feedback_pre_users_no_deprecation_phasing.md`) — the
   transitional both-fields shape is ergonomic-only, not a back-compat
   commitment.

2. **Beachhead engine-side rendering for E054/E057** — RESOLVED: **YES, do
   it.** Commit 3's beachhead PR MUST verify byte-identical output between
   the pre-PR-3c `compute_relido_removal_span` path and the post-PR-3c
   `render_canonical(post-FactRemove projection)` path before merging.
   Concretely: snapshot the audit-record output from a representative
   E054/E057 fixture set on the pre-PR-3c baseline; rerun on the
   beachhead branch; diff MUST be empty. If it's not, commit 5
   (`render_canonical` body) has a defect — fix it before commit 3
   merges, do not ship a "known-shifted" audit baseline.

3. **`tools/citation-lint/`'s current scope** — User will inspect the
   existing tool's source directly to determine what calibration commit
   1 entails vs. what's net-new. Plan-level guidance: commit 1's
   citation-hygiene sweep is independent of the lint-harness extension
   scope; the sweep can land first, the harness extension can be a
   sub-commit or a follow-up depending on what user finds in the
   existing tool.

## References

- `specs/006-engine-rule-refactor/architecture.md` — architectural commitment
- `specs/006-engine-rule-refactor/rule-body-audit.md` — 47-row audit table
- `specs/006-engine-rule-refactor/decisions/01-migration-sequencing.md` —
  Decisions 1, 5, 11
- `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md` — Decisions
  2, 3, 4
- `specs/006-engine-rule-refactor/decisions/03-empirical-concerns.md` —
  Decisions 7, 8, 9, 10
- `specs/006-engine-rule-refactor/decisions/04-citation-hygiene.md` —
  Decision 6
- `specs/006-engine-rule-refactor/followups/no-fix-auto-apply-calibration.md`
  — D3 follow-up
- `specs/006-engine-rule-refactor/followups/admonition-channel.md` — D10
  deferral
- `.specify/memory/constitution.md` — Principles I, IV, V, VI, VII, VIII
- `crates/capco/docs/CAPCO-2016.md` — primary authoritative source
- `crates/capco/CAPCO-CONTEXT.md` — full domain context
