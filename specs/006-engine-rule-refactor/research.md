<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Phase 0 Research: Engine + Rule Architecture Refactor

**Branch**: `006-engine-rule-refactor` | **Date**: 2026-05-03
**Input**: [plan.md](./plan.md), [spec.md](./spec.md)

The spec emitted no `[NEEDS CLARIFICATION]` markers — the source plans
are post-murder-board and post-user-decision-pass — but seven tactical
implementation decisions warrant Phase 0 resolution before PR 0 begins.
Each is decided here so the implementing PRs don't relitigate.

---

## R-1 — AST tooling stack for the three CI lints

**Decision**: All three lints (`tools/citation-lint/`,
`tools/masking-pin-lint/`, `tools/promote-callsite-lint/`) are Rust
binary crates that consume the workspace via `syn` 2.x for AST parsing
plus `proc-macro2` for span/token handling. The masking-pin lint adds
`octocrab` (or `gh-api-client` equivalent) for the mandatory GitHub-API
issue-state check (FR-039 rule 4).

**Rationale**:
- `syn` is the canonical Rust AST library, used by every macro crate
  and most workspace tooling. It handles full Rust syntax including
  attributes, doc comments (`/// ...` is parsed as `#[doc = "..."]`),
  and string-literal interpolation patterns. The citation-lint needs
  to inspect doc-comment `§X.Y` references and string-literal
  interpolations across `citation:` / `message:` / `constraint_label:`
  fields — `syn` covers all four positions uniformly.
- `proc-macro2` provides `Span` access for emitting accurate error
  locations (file:line:col). CI failures must point at the offending
  source line; regex-based scans cannot.
- `octocrab` is the maintained Rust GitHub API client; the masking-pin
  lint queries `repos/{owner}/{repo}/issues/{n}` and follows
  `state_reason: "completed"` + closing-PR `closed_as_duplicate_of`
  chains until it hits a final close (FR-039 rule 4 — mandatory, not
  optional, with `closed_as_duplicate_of` chain-following).

**Alternatives considered**:
- **`tree-sitter` + `tree-sitter-rust`**: Cross-language parser,
  better for polyglot tooling. Rejected because the lints are Rust-only
  and `syn` is the workspace-native choice — adding a non-Cargo build
  dependency for tooling that targets only Rust source is unnecessary
  weight.
- **regex-only**: What the spec explicitly rejects (FR-018, FR-039,
  FR-040 all say "AST-based, not regex"). The murder board found that
  regex pattern-matching missed the `format!("decoder-recognized
  canonical form: {replacement:?}")` interpolation at `engine.rs:1389`
  because `{replacement:?}` is not a syntactic pattern that grep
  reliably catches across formatter rewrites. AST inspection finds
  every `format!`/`format_args!`/`write!`/`writeln!` call uniformly.
- **`rust-analyzer` IDE library**: Heavier dependency; designed for
  semantic queries rather than lint-style structural checks.
  Rejected as overkill.

**Implementation note**: The three lint crates ship in PR 0 (masking-pin
+ promote-callsite) and PR 0.5 (citation). They live in `tools/` —
**not** as workspace members of the product graph — to avoid
contaminating WASM-safe crate dependency closure (Constitution III) or
the audit-record-producing dep chain (Constitution VII). They are
invoked from CI via `cargo run --manifest-path tools/<lint-name>/Cargo.toml -- <args>`
against the workspace source tree.

---

## R-2 — `MessageTemplate` starter enum

**Decision**: Generate the initial `MessageTemplate` variant set by
mechanical extraction from the existing rule catalog *before* PR 3c
lands. The discovery script lives at `tools/message-template-extract/`
(workspace-internal), parses every `Diagnostic::message` construction
site in `crates/capco/src/rules.rs` and `crates/engine/src/engine.rs`,
clusters the `format!`/`format_args!` first-arg literals, and emits a
skeleton `enum MessageTemplate { ... }` plus per-variant `MessageArgs`
type signatures. The skeleton is reviewed and pruned by hand into the
PR 3c source.

**Rationale**:
- The current message catalog is the source of truth for what templates
  the post-cutover surface needs. Inventing a "clean" enum without
  grounding it in existing messages risks shipping a surface that
  doesn't cover real diagnostics.
- The mechanical extraction runs once at PR 3c implementation, not on
  every CI run. It produces a starter file; the human curation step
  decides which messages collapse into one template (parameter-only
  variants), which split (semantically distinct), and which drop
  altogether (rule collapse from #263 may eliminate ~30 rules' messages
  outright).
- The extraction script's output is checked in as
  `specs/006-engine-rule-refactor/contracts/message-template-starter.md`
  to make the "what messages exist today" snapshot reviewable as a
  diff against the eventual PR 3c source.

**Alternatives considered**:
- **Catalog by hand**: Prone to omission; the rule collapse PR 3b
  changes which rules survive, so a hand-catalog before 3b lands is
  stale before 3c lands. Mechanical extraction at 3c time avoids that.
- **Synthesize from rule semantics, not existing messages**: Risks
  shipping messages that don't match what users see today. Migration
  cost on consumers (CLI users, IDE plugins) is real even though there
  are no external consumers — it would discard a maintenance signal.

**Carry-forward**: The mechanical starter extraction remains limited
to rule-emitted messages; it does not auto-discover engine-synthetic
diagnostics. However, the curated `MessageTemplate` set for this spec
still reserves the engine diagnostics `MessageTemplate::DecoderRecognized`
and `MessageTemplate::ReparseFailed`, which correspond to the current
R001 / R002 channel and are serialized by the audit-record contract.
The plan §9.4 centralization of engine-synthetic IDs into
`marque-rules` remains a separate refactor not in scope here.

---

## R-3 — `(scheme, predicate-id)` naming convention

**Decision**: Rule IDs are `(scheme: &'static str, predicate_id:
&'static str)` tuples where `scheme` is the lowercase scheme short name
(`"capco"` for the only in-tree scheme today) and `predicate_id` is
**dot-separated, lowercase, structural-not-numeric**, of the form
`<surface>.<category>.<predicate>` with `<predicate>` further
subdivided by hyphens when needed for readability.

Examples:
- `("capco", "banner.classification.usa-trigraph")` — the example from the consolidated plan §10.2.1 audit-record sketch
- `("capco", "portion.dissem.noforn-supersedes-relto")`
- `("capco", "banner.sci.system-canonicalization")`
- `("capco", "page.fouo.evicted-by-non-fdr-dissem")`
- `("engine", "r001.decoder-recognized")` (RESERVED for engine-minted sentinel scheme)
- `("engine", "r002.reparse-failed")` (RESERVED)

**Rationale**:
- Dot-separated nested form scales: as the rule catalog adds
  `marque-cui` or other schemes, the structural form composes
  (`("cui", "marking.fouo.confidential-eviction")`).
- `<surface>` (`banner` / `portion` / `page` / `engine`) tells the
  reader where in the document the rule fires — useful at audit-log
  triage time.
- `<category>` matches the lattice category where applicable
  (`classification`, `sci`, `sar`, `dissem`, `fgi`, `nato`, `fouo`,
  `aea`, `declassification`).
- `<predicate>` is descriptive English-with-hyphens. The murder-board
  citation defects (HCS-P fabrication, `p150–151 p151` doubling) showed
  that obscure rule IDs (`E028` etc.) hide the underlying predicate;
  switching to descriptive predicate names makes citation review
  meaningful at the audit-record level.
- The `(scheme, predicate_id)` tuple is `Copy + Eq + Hash` since both
  fields are `&'static str` — usable as a `HashMap` key without
  allocation.

**Alternatives considered**:
- **Keep `E###` / `W###` / `S###` / `C###`**: Rejected per consolidated
  plan §3 (resolved dissent) — clean break, retire at PR 3c.
- **UUID-keyed**: Stable across renames but loses the descriptive
  property that makes audit triage useful. Rejected.
- **Hash of predicate code**: Stable but opaque; auditors cannot read
  it. Rejected.

**Migration mapping**: PR 3c emits a one-time
`docs/refactor-006/legacy-rule-id-map.md` listing every retired
`E###`/`W###`/`S###`/`C###` ID with its `(scheme, predicate-id)`
successor. The map exists for archaeological purposes
(historical commit messages, prior-art docs) only — there is no
runtime translation table since there are no pre-cutover audit
records (clean break per Constitution-V principle and FR-037).

---

## R-4 — `pre_pass_1_attrs` cache strategy

**Decision**: Pre-pass-1 attributes are cached **per-marking** in a
slot-allocated `SmallVec<[CanonicalAttrs<'src>; 4]>` owned by
`Engine::fix_inner`'s stack frame for the duration of the two-pass
apply. `RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>`
is filled by the engine when dispatching `Phase::WholeMarking` rules
whose span overlaps a pass-1 fix; populated as `None` for
`Phase::Localized` rules (pass-2 doesn't dispatch them) and for
`Phase::WholeMarking` rules whose span does not overlap any pass-1 fix.

**Rationale**:
- The cache lives only for the duration of one document's
  `fix_inner` call — it does not survive across documents in a
  `BatchEngine` workload, so no Arc/synchronization overhead is
  warranted.
- `SmallVec<[T; 4]>` covers the typical case (a 10 KB document has
  O(10s) markings; 4-slot inline avoids heap on most documents) without
  capping the worst case.
- The `'src` lifetime threads through naturally: `CanonicalAttrs<'src>`
  borrows from the input buffer (after PR 3a's pivot split makes
  `ParsedAttrs<'src>` a borrowed type); the cache borrows the same
  `'src` so no clone is needed for the pre-pass-1 snapshot.
- Filling `pre_pass_1_attrs` only when relevant (overlap with pass-1
  fix) avoids paying the cache cost on documents with no pass-1 /
  pass-2 interaction — most documents.

**Alternatives considered**:
- **Re-parse from pre-pass-1 buffer per pass-2 rule call**: Wasteful;
  the same marking would re-parse N times for N pass-2 rules. Plan
  §9.3 calls out "implementation cost is bounded — most pass-2 rules
  don't touch pass-1 spans", which assumes caching, not re-parse.
- **Cache the raw bytes, re-parse on access**: Halfway measure that
  loses the type-safety of `&CanonicalAttrs<'src>`; re-parse can
  fail (and would need its own R002-style handling). Rejected.
- **`HashMap<MarkingSpan, CanonicalAttrs<'src>>`**: Allocates a hash
  map per document; overkill for small N. Rejected.

**Implementation note**: PR 7 (pass split) lands this. The
`RuleContext` field signature is part of the rule-API contract — it
appears in the `Rule` trait's `evaluate(&self, attrs: &CanonicalAttrs,
ctx: &RuleContext)` method. External rule crates see this in the
`marque-rules` API surface; documented in `contracts/fix-intent.md`.

---

## R-5 — Baseline measurement capture

**Decision**: Capture pre-refactor baselines for the four bench gates
(FR-029 fix_throughput already landed; FR-030 interactive p99; FR-031
multi-page projection; FR-032 fix_10kb two-pass) as a **single PR 0
deliverable** that runs the existing benches against `main` and emits
`benches/baselines/2026-05-pre-refactor.json`. The post-refactor PRs
read this JSON and assert against it.

**Rationale**:
- The "preserve performance" requirement (FR-030..FR-033) needs a
  measured baseline, not a pinned absolute number. p95 ≤ 16 ms is
  the SC-001 target (from CLAUDE.md / Constitution I); the *additional*
  ±5% / ±10% envelopes only make sense relative to a captured
  baseline.
- Capturing once at PR 0 fixes the comparison point for the entire
  refactor sequence. Without this, every PR could individually claim
  "no regression vs. last PR" while the cumulative slope drifts up
  invisibly.
- The baseline JSON becomes a checked-in artifact, comparable across
  Git history; future regression investigations can replay against it.

**Alternatives considered**:
- **Re-baseline at every PR**: Hides cumulative regression; rejected.
- **No baseline, only absolute thresholds**: SC-001 has p95 ≤ 16 ms
  but the p99 / multipage / fix_10kb numbers are *relative*. Without a
  captured baseline, "baseline + 5%" is undefined. Rejected.

**Format**: NDJSON or single JSON object — implementer's call at PR 0;
suggest single JSON with fields `{ bench: "name", p50, p95, p99, mean,
samples, git_sha, captured_at }`. The `bench-check.sh` regression gate
reads it.

---

## R-6 — F.1 corpus discovery scope

**Decision**: PR 0.5 runs the F.1 corpus-fidelity gate against the
existing rule catalog as a **discovery exercise**, emits a list of
failures (citations whose canonical example is absent from the shared
`tests/corpus/` tree used by rule/corpus tests), and PR 0.6 fixes
those failures. The set of failures is unknown until PR 0.5 runs; the
consolidated plan §4 row PR 0.5 acknowledges this ("F.1 runs against
existing catalog as discovery — failures get fixed in PR 0.6").

**Rationale**:
- Predicting the exact set of citation defects without running the
  lint is exactly the failure mode that produced the HCS-P / `p150–151
  p151` cluster — assuming the catalog is clean. The mechanical
  discovery at PR 0.5 is what catches the unknowns.
- PR 0.6's scope is reactive to PR 0.5's output. The four
  pre-identified defect classes (`§4` fabrications, `p150–151 p151`
  doublings, SIGMA archaeology, HCS-P over-strict predicate) are the
  floor; PR 0.5 may add to this list.

**Process**:
1. PR 0.5 lands citation-lint + F.1 skeleton.
2. CI runs against the existing catalog; failures are captured into a
   PR 0.5 artifact `docs/refactor-006/citation-defect-catalog.md`
   (auto-generated at lint run time).
3. PR 0.6 reads the catalog, addresses every entry, lands corpus
   fixtures for any newly-cited authority lacking one, and updates
   the citation per the rules in FR-018 (in-normative-range, page
   number valid).
4. PR 0.6 cannot land while the catalog is non-empty; the merge gate is
   the catalog-empty check.

**Out of scope**: The PR 10 maturation extends F.1 from sparse (one
canonical example per existing rule) to full per-cited-authority
coverage. PR 10 is a pure expansion of the same lint shape; no new
mechanism.

---

## R-7 — Sealed-trait pattern for cross-crate `Canonical<S>` construction

**Decision**: Use the standard Rust **sealed-trait pattern**. A
private module `marque_scheme::canonical::sealed` declares a `Sealed`
trait; the public `CanonicalConstructor<S>` trait extends `Sealed`;
implementations of `Sealed` are crate-private to `marque-scheme`
(specifically, an `EngineConstructor<S>` impl that the engine
instantiates and holds). External rule crates can name the
`CanonicalConstructor<S>` trait but cannot implement it because they
cannot implement `Sealed`.

```rust
// In marque-scheme:
mod sealed { pub trait Sealed {} }
pub trait CanonicalConstructor<S: MarkingScheme>: sealed::Sealed {
    fn build_open_vocab(category: CategoryId, bytes: Box<str>, scope: Scope) -> Canonical<S>;
}

// Crate-private impl, only marque-scheme can construct one:
pub(crate) struct EngineConstructor<S: MarkingScheme>(PhantomData<S>);
impl<S: MarkingScheme> sealed::Sealed for EngineConstructor<S> {}
impl<S: MarkingScheme> CanonicalConstructor<S> for EngineConstructor<S> { /* ... */ }
```

The engine holds an `EngineConstructor<CapcoScheme>` (or for whichever
`S` is active) and uses it to render `FixIntent<S>` values into
`Canonical<S>` during `Engine::fix_inner` promotion. External rule
crates emit `FixIntent<S>` and the engine renders them — the
external crate never names `EngineConstructor` and cannot construct
one.

**Rationale**:
- Sealed-trait is the established Rust pattern for "extensible by us,
  closed to outside callers" (used by `serde::de::Visitor`, `ZeroMap`,
  many internal AST crates). It's the right shape for the closure
  property in §8.1 of the consolidated plan.
- The closure holds even when a future `marque-cui` adds a new
  `CapcoScheme`-equivalent: the new scheme adapter declares its own
  `MarkingScheme` impl; the engine's `EngineConstructor<NewScheme>`
  is still the only `CanonicalConstructor<NewScheme>` impl because
  `marque-cui` cannot implement `Sealed`.
- Compile-time enforcement: if a downstream rule crate writes
  `impl CanonicalConstructor<MyScheme> for MyConstructor { ... }`,
  the build fails with a clear error (`Sealed` is not in scope).

**Alternatives considered**:
- **Visibility-only (`pub(crate)` on the constructor)**: Rejected
  because `Canonical::from_render` would have to live in
  `marque-scheme`, but the *render function* (`MarkingScheme::render_canonical`)
  is implemented per-scheme in `marque-capco` (a different crate).
  `pub(crate)` doesn't reach across crate boundaries; sealed trait
  does.
- **Capability token pattern (`CanonicalConstructor::token() ->
  CanonicalToken`)**: More machinery; `EnginePromotionToken` already
  uses this for `__engine_construct`, but the audit-promotion token
  is a runtime check; here we want compile-time impossibility.
  Sealed trait gives the stronger property.
- **Macro-emitted constructor**: Hides the contract. Rejected.

**Implementation note**: PR 3c lands this pattern alongside `FixIntent<S>`.
The contract is documented in `contracts/fix-intent.md` for external
rule-crate authors. The sealed pattern is *also* applied to
`AppliedFix::__engine_promote` and `EnginePromotionToken::__engine_construct`
via the AST-based promote-callsite lint (FR-040) — different
enforcement mechanism (lint vs. type system) for the same invariant
(only the engine constructs these), because `__engine_promote`'s
constructor signature is dictated by `marque-rules` (which depends on
neither `marque-scheme` nor the engine), so type-level sealing is not
available there.

---

## R-8 — SC-010 baseline re-anchoring decision tree

**Decision**: The mangled-corpus accuracy baseline (SC-010, equivalent
to CLAUDE.md's prior SC-004) re-anchors at PR 3c implementation per
the following decision tree:

1. **Run the post-PR-3c mangled corpus accuracy bench** against the
   pre-refactor baseline.
2. **If the new accuracy is ≥ 0.85**: keep the existing 0.85 floor;
   no re-anchoring needed; this becomes the new SC-010 outcome.
3. **If the new accuracy is < 0.85 but ≥ 0.80**: investigate which
   corpus inputs lost coverage. If they were inputs whose mangled
   tokens were decoder-canonicalized to *open-vocabulary* tokens
   (the case the K-Option 2 lockout intentionally removes), **re-curate
   the mangled corpus** at `tests/corpus/mangled/`: split into
   `mangled-closed-vocab/` (inputs whose canonicals are CVE tokens,
   should still be ≥ 0.85) and `mangled-open-vocab/` (inputs whose
   canonicals require open-vocab, expected to be diagnostic-only after
   PR 3c). The 0.85 floor applies to `mangled-closed-vocab/` only.
4. **If the new accuracy is < 0.80** OR the loss is not attributable
   to the K-Option 2 lockout (e.g., a real regression): **back out the
   PR 3c change at the merge gate** per the §3.6 measurement-gating
   discipline. This is the same backout discipline as performance
   benches (FR-033) but applied to accuracy.

**Rationale**:
- The decoder open-vocab lockout (FR-027) intentionally narrows the
  fix-recall surface; SC-010's baseline was set when the decoder *was*
  producing open-vocab fixes. A blanket 0.85 floor without re-anchoring
  conflates two different things: regressions on closed-vocab (real
  regression, must back out) and regressions on open-vocab (intended
  scope reduction).
- The decision tree distinguishes the two cases mechanically. The
  re-curated corpus is the cleaner long-term artifact: future
  regressions on closed-vocab fixes are detectable at their own
  sharper threshold.

**Binding** (per **decision D5** in `decisions.md`): this decision
tree is **binding**, not deferred to PR 3c review notes. The chosen
branch and threshold value MUST be encoded in
`tests/corpus/mangled/threshold.toml` (per D7); `scripts/bench-check.sh`
reads that file. Reviewer judgment at PR 3c merge time is limited to
verifying the artifact reflects the measured outcome — not to
overriding the tree. If accuracy lands <0.80 and the loss is not
K-Option-2-attributable, PR 3a / 3b / 3c revert as a unit.

**Alternatives considered**:
- **Hold the 0.85 floor regardless**: would reject PR 3c even though
  the underlying lockout is correct. Rejected.
- **Lower the floor without splitting the corpus**: loses the future
  regression signal on closed-vocab. Rejected.
- **Split immediately at PR 3c without measuring first**: skips the
  measurement step that distinguishes regression from intended scope
  change. Rejected.
- **Defer the decision to reviewer judgment in PR 3c review notes**
  (the prior posture): rejected per D5 — leaves the rollback policy
  in reviewer judgment under merge-pressure; pre-commit makes the
  policy mechanical.

---

---

## R-9 — PR 9 sub-divide into 9a / 9b / 9c

**Decision** (per **D9** in `decisions.md`): PR 9 splits into three
sub-PRs, each scoped to a single correctness property and
independently revertable per US8 discipline:

- **PR 9a** — parser separator spans (#106). Includes an internal
  acceptance test asserting the parser correctly identifies separator
  positions (`/`, `//`, whitespace boundaries) at the parser-output
  layer. **Closes nothing in the issue tracker** by itself
  (infrastructure for 9b / 9c).
- **PR 9b** — `dissem_us` / `dissem_nato` position-attributed split
  (#271). Depends on PR 9a (separator positions delimit US-vs-NATO
  dissem regions). Banner-validation rules migrate to consume
  `&ProjectedMarking` here. Closes #271 (and #270 / #264 if their
  banner-validation paths land here).
- **PR 9c** — ATOMAL / BOHEMIA recognition via the existing
  `Vocabulary<S>` build-time generation pipeline (#246) +
  NATO-portion-in-US-doc declarative `Constraint` requiring
  `REL TO USA, NATO` derivation in the banner (#265). Closes #246,
  #251, #265.

**Rationale**:
- PR 9 currently bundles parser infrastructure (FR-045) + data-model
  position-attribution (FR-046) + vocabulary additions (FR-047, FR-048)
  across three distinct correctness properties. US8 (independent
  revertability) requires sub-division when a single PR touches
  multiple properties — the same discipline applied to PR 3 (3a/3b/3c)
  and PR 6 (6a/6b/6c).
- PR 9a is pure infrastructure with no rule consumer; the internal
  acceptance test (parser-level separator-position correctness)
  prevents 9a from shipping as "infrastructure with no consumer," a
  smell that would invite later refactor of 9a's internals without
  a regression-catch mechanism.
- Sub-PR ordering is fixed by data-flow dependency (9a → 9b → 9c).
  Bundling 9b's banner-validation migration with 9c is permissible
  if implementer finds it cleaner (the migration depends on the data
  model existing, not on the NATO tokens being recognized).

**Alternatives considered**:
- **Keep PR 9 monolithic**: rejected per US8 revertability discipline.
- **Two-way split (infra+data-model / vocab+constraint)**: rejected;
  the parser-infra → data-model dependency is real and warrants its
  own revert point.

---

## R-10 — Masking-pin lint cache strategy: API-first with cache fallback

**Decision** (per **D11** in `decisions.md`): the masking-pin lint
(FR-039) calls the GitHub API at PR-time with a **5-second timeout**.
On API failure (timeout, rate-limit, network error), the lint falls
back to a **daily-refreshed cache** at `tools/masking-pin-lint/cache/`
and emits a CI **warning** (not error). A scheduled CI job populates
the cache once per day.

**Rationale**:
- **Cache-only** (the alternative) weakens the lint at exactly the
  moment correctness most depends on it: when an issue closes, the
  PR that should remove the pin opens, and the cache has not yet
  refreshed — the lint sees stale "open" state and fails to flag the
  stale pin. The lint becomes weaker precisely when it most needs to
  be strong.
- **API-only** (the original posture) gates every PR build on GitHub
  availability. GitHub outages or rate-limit excursions block the
  refactor's PR throughput entirely.
- **Cache-with-fallback** is the standard pattern: prefer fresh,
  accept stale on outage with a visible warning. The 24-hour
  staleness window on cache is acceptable because the failure mode
  (stale "open" state on a closed issue) is detected at the next
  fresh API call.
- Cache schema: keyed by `(repo, issue_number)`, value is
  `{ state: "open" | "closed", closed_at, closed_as_duplicate_of:
  Option<u64>, refreshed_at }`. Daily refresh job follows
  `closed_as_duplicate_of` chains until terminal-close (FR-039 rule
  4).

**Alternatives considered**:
- **Cache-only with daily refresh**: rejected per the staleness
  failure mode above.
- **No caching, PR-time API only**: rejected per the GitHub
  availability dependency.
- **In-process cache with TTL** (per CI run): no persistence across
  runs; degenerates to API-only. Rejected.

---

## R-11 — `_unchecked` lint by signature shape, not name

**Decision** (per **D12** in `decisions.md`): the
`tools/promote-callsite-lint/` lint is extended (FR-040 amendment) to
flag any function whose **signature shape** matches
`fn(...ParsedAttrs<'_>...) -> CanonicalAttrs` outside
`MarkingScheme::canonicalize`. The lint targets shape, not name.

**Whitelist**:
- `unsafe fn` blocks (Rust stdlib uses `_unchecked` for `unsafe` APIs:
  `get_unchecked`, `from_utf8_unchecked`, etc.).
- The transitional `pub(crate) fn from_parsed_unchecked` adapter in
  `marque-ism` during the PR 3a → 3c keystone window — exempted
  via path-based carve-out keyed on
  `crates/ism/src/attrs.rs::from_parsed_unchecked`. The carve-out
  auto-removes when 3c lands (the function is deleted; the lint then
  has nothing to whitelist).

**Rationale**:
- Naming-only lint (e.g., flagging `fn` whose name ends in
  `_unchecked`) is brittle: a future contributor renaming the helper
  to `from_parsed_raw` evades the lint without changing the failure
  pattern.
- Targeting signature shape catches **intent**: any
  `ParsedAttrs → CanonicalAttrs` conversion outside the trait method
  is the actual failure pattern the lint is meant to prevent.
- `syn`'s AST exposes function signatures uniformly; the shape match
  is straightforward (parse return type, parse argument types, match
  against the prohibited shape).

**Alternatives considered**:
- **Name-suffix lint** (`*_unchecked`): rejected per the renaming
  evasion mode.
- **No lint, convention only**: rejected per the spec's invariant
  philosophy — invariants enforced by convention rot under
  contributor turnover.
- **Macro-based check**: rejected; `syn` AST inspection is cheaper
  and uniform across the workspace.

---

## Summary

| ID | Decision | Lands at |
|---|---|---|
| R-1 | `syn` + `proc-macro2` (+ `octocrab` for masking-pin GitHub API) for the three AST-based lints; `tools/<lint>/` not workspace product crates | PR 0 (mask, promote), PR 0.5 (citation) |
| R-2 | Mechanically extract `MessageTemplate` starter set from existing diagnostic catalog at PR 3c implementation; check in starter as `contracts/message-template-starter.md` | PR 3c |
| R-3 | `(scheme, predicate-id)` IDs are dot-separated `<surface>.<category>.<predicate>` lowercase strings; one-time legacy-mapping doc in `docs/refactor-006/legacy-rule-id-map.md` | PR 3c |
| R-4 | `pre_pass_1_attrs` is a `SmallVec<[CanonicalAttrs<'src>; 4]>` owned by `Engine::fix_inner` stack frame, populated only on overlap | PR 7 |
| R-5 | Capture pre-refactor baselines as `benches/baselines/2026-05-pre-refactor.json` once at PR 0; subsequent PRs assert against this | PR 0 |
| R-6 | F.1 lints existing catalog at PR 0.5 to discover defects; PR 0.6 fixes everything PR 0.5 surfaced; PR 0.6 merge-gated on catalog-empty | PR 0.5, PR 0.6 |
| R-7 | Sealed-trait pattern for `CanonicalConstructor<S>`; engine holds the only impl; external rule crates emit `FixIntent<S>` | PR 3c |
| R-8 | SC-010 re-anchor decision tree (**binding** per D5): measure → ≥0.85 keep / 0.80–0.85 re-curate corpus / <0.80 or non-lockout regression back out 3a/3b/3c as a unit; chosen branch encoded in `tests/corpus/mangled/threshold.toml` | PR 3c |
| R-9 | PR 9 → 9a (separator spans + internal acceptance test) → 9b (dissem_us / dissem_nato split) → 9c (ATOMAL/BOHEMIA + NATO Constraint) per D9 | PR 9 |
| R-10 | Masking-pin lint: API-first with 5s timeout, daily-cache fallback, CI warning on fallback (per D11) | PR 0 |
| R-11 | `_unchecked` lint targets signature shape (`fn(...ParsedAttrs<'_>...) -> CanonicalAttrs` outside `MarkingScheme::canonicalize`); `unsafe fn` whitelisted; transitional adapter exempted during 3a–3c (per D12) | PR 0 |

All `[NEEDS CLARIFICATION]` markers from spec.md are resolved (none
were emitted; no resolution required). The 16 process / contract
decisions surfaced in panel review are captured in
[`decisions.md`](./decisions.md) as D1–D16; their cross-references
into research.md are R-8 (binding amendment, D5), R-9 (D9), R-10
(D11), and R-11 (D12). Phase 1 design proceeds.
