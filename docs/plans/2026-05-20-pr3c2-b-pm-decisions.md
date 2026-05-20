# PR 3c.2.B — PM Contract & Binding Decisions

**Date**: 2026-05-20
**Branch**: `refactor-006-pr-3c2-b-call-site-migration`
**Base**: `origin/staging` at HEAD `861e85e3` (PR 3c.2.A merge)
**Predecessor**: `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` (PR 3c.2.A — Citation + RenderContext + canonicalize GAT scaffolding)
**Deferred findings carryover**: `specs/006-engine-rule-refactor/followups/2026-05-19-pr-3c2-a-deferred-findings.md` (B-FOLLOWUP-1, B-FOLLOWUP-2)
**Preflight inputs**:
- `docs/plans/2026-05-20-pr3c2-b-architect-preflight.md` (system-architect lens)
- `docs/plans/2026-05-20-pr3c2-b-tactical-plan.md` (Plan agent lens)

This document is the **binding PM contract** for PR 3c.2.B. Implementation agents read this. Reviewers attest against it.

---

## §1 — Scope (what 3c.2.B does)

Migrates `from_parsed_unchecked` call sites to the `MarkingScheme::canonicalize` trait route, **and** lands the `CapcoScheme` override. Override body is `marque_ism::from_parsed_unchecked(parsed)` **verbatim** — semantic-identical, byte-identity guaranteed by T056 corpus regression matrix.

**In scope** (26 sites migrate — reconciled from preflight estimate of 25):
- 2 production engine hot-path sites (`recognizer.rs:98`, `decoder.rs:411`)
- 9 in-`src/` `#[cfg(test)]` test sites inside `crates/engine/src/decoder.rs`
- 2 WASM lint/fix sites (`wasm/src/lib.rs:1187, 1292`)
- 12 external `crates/capco/tests/` sites + 1 `crates/engine/tests/document_corpus.rs:148` = 13 external test sites
- B1 lands the `CapcoScheme::canonicalize` override body
- B-FOLLOWUP-1: HRTB compile-time smoke test

**Erratum (added in B6)**: the architect preflight's Appendix A inventory listed 11 `crates/capco/tests/` files; the implementation grep at the start of B uncovered a 12th file (`render_canonical_properties.rs:50`), correctly migrated in B4. Net total is 26 migrated + 5 carved-out = 31 sites of `from_parsed_unchecked(`, not 30. PM-B-8's "25 sites" was off by one; the corrected count is 26.

**Out of scope** (5 sites carved out):
- 1 site: `crates/capco/tests/s004_audit_content_ignorance.rs:65` — file is `#![cfg(any())]`-disabled pending Diagnostic-shape rewrite at 3c.2.C. Migration is no-op until file is re-enabled. **Decision**: leave site at adapter call; add `// TODO(3c.2.C): migrate when test rewrite per Diagnostic-shape lands` comment (PM-B-7).
- 4 sites: `crates/core/src/parser.rs:3890`, `crates/core/tests/display_only_list.rs:512`, `crates/core/tests/fgi_silent_skip_guard.rs:96, 117` — Constitution VII would be violated if `marque-capco` were added as a `marque-core` dev-dep. **Decision**: defer to 3c.2.E alongside adapter retirement (PM-B-2).

**`from_parsed_unchecked` adapter retained.** Wrapped by the override; not deleted. Deletion is 3c.2.E scope.

**Audit schema stays `marque-mvp-3`.** The audit-schema cutover lands at 3c.2.D, not here.

---

## §2 — Binding PM decisions

### PM-B-1 — Engine scheme-instance source: module-scope `LazyLock<CapcoScheme>`

**Decision**: At each of the two engine production sites (`crates/engine/src/recognizer.rs:98` and `crates/engine/src/decoder.rs:411`), introduce a **module-scope `static SCHEME: LazyLock<CapcoScheme> = LazyLock::new(CapcoScheme::new);`** and route the migration through `SCHEME.canonicalize(parsed.attrs)`.

**Rationale (override of architect's leaning toward thread-through)**: The architecturally-cleanest path is to thread `&CapcoScheme` through the `Recognizer<S>::recognize` trait signature — but that expands B's scope into a trait-surface refactor touching every recognizer impl. PR 3c.2.B is a **call-site migration**, not a trait-surface redesign. The `LazyLock` is a deliberate transitional smell:

- It matches the existing `bridge_scheme: CapcoScheme::new()` precedent at `crates/engine/src/engine.rs:503` (engine constructor already builds a scheme by value at startup) — the static merely moves that construction one layer up.
- `LazyLock` is stdlib since Rust 1.80; workspace MSRV is 1.85.
- `CapcoScheme` is NOT zero-sized (4 `Vec` fields populated by `build_categories()` / `build_constraints()` / `build_page_rewrites()` at `crates/capco/src/scheme/adapter.rs:67-76`). Per-call construction at the hot path would be a measurable allocation regression — violates Constitution I (Uncompromising Performance).
- Inherent helper (`CapcoScheme::canonicalize_inner`) was rejected as an anti-pattern: it makes the trait route a fig leaf while real dispatch happens elsewhere.

**Follow-up commitment**: open a tracking issue `engine-S-generic-recognizer-cleanup` after PR 3c.2.B merges. The issue scope: thread `&S` through `Recognizer<S>::recognize(&self, scheme: &S, ...)`, retire both `LazyLock<CapcoScheme>` statics, retire `engine.bridge_scheme` to a directly-passed `&self.scheme`. Targets post-1.0 cleanup, not in the 3c.2 series. The static carries an explicit `// TODO(engine-S-generic-recognizer-cleanup)` comment naming this issue.

### PM-B-2 — Core 4-site carve-out: defer to 3c.2.E

**Decision**: Leave the 4 sites in `crates/core/{src,tests}/` on `marque_ism::from_parsed_unchecked(...)` for PR 3c.2.B. Migrate (or rewrite to delete) at PR 3c.2.E.

Sites:
- `crates/core/src/parser.rs:3890` (`#[cfg(test)] mod tests::CanonicalParsed`)
- `crates/core/tests/display_only_list.rs:512`
- `crates/core/tests/fgi_silent_skip_guard.rs:96, 117`

**Rationale**: Adding `marque-capco` as a `marque-core` dev-dep would create a Cargo-legal but Constitution-VII-violating dep cycle (`marque-capco` already deps on `marque-core`; the reverse edge inverts the dep graph). A locally-defined `TestScheme` stub in `crates/core/tests/` is a maintenance liability. Deferral is cleanest: PR 3c.2.E will face this same decision when it deletes the adapter, and at that point a `TestScheme` stub (or rewriting the tests to not need canonicalization) becomes the natural answer — but 3c.2.B is not the right vehicle.

**Each of the 4 sites gets an explicit comment**: `// TODO(3c.2.E): migrate or rewrite when `marque_ism::from_parsed_unchecked` adapter retires; Constitution VII forbids `marque-core ←── marque-capco` dev-dep edge.`

### PM-B-3 — External `tests/` migrations: inline scheme construction per helper

**Decision**: Each external test helper that needs canonicalization constructs a fresh `let scheme = CapcoScheme::new();` inline (or `let scheme = CapcoScheme::new();` once per test where the helper is per-test). **No `static LazyLock` in tests.**

**Rationale**: Test hermeticity > microsecond cost. CapcoScheme construction is dominated by 4 `Vec` allocations — measurable in microseconds, irrelevant for CI runtime. Hermetic test state is more valuable than the perf shortcut.

**Per-file pattern**:
- Where the test already constructs `CapcoScheme::new()` for other purposes (e.g., `render_canonical_axis_fixtures.rs:789, 829`; both `wasm/src/lib.rs` sites; `relido_clears_page_rewrites.rs:47`), **reuse that scheme**. Zero new allocation cost.
- Where the test helper is module-level and called from multiple `#[test]` functions, the helper takes `&CapcoScheme` as a parameter; each `#[test]` constructs the scheme inline.
- Where the helper is per-test, construct the scheme at the helper's top.

### PM-B-4 — Doc-comment sweep: 5 updates in B; 8 deferrals to 3c.2.E

**Decision**: B5 (closeout commit) sweeps 5 doc-comments that describe code state changing in this PR. 8 doc-comments that describe the adapter while it still exists are left for 3c.2.E.

**Update now (5)**:
1. `crates/scheme/src/scheme.rs:140-151` — retense "PR 3c.2.B implements the CapcoScheme override" → "PR 3c.2.B (landed) implements the CapcoScheme override; adapter retained until 3c.2.E".
2. `crates/capco/src/scheme/marking_scheme_impl.rs:299` — retense the "override lands at PR 3c.2.B" comment to past tense, co-located with the override body.
3. `crates/rules/src/confidence.rs:49` — strike `from_parsed_unchecked` from the "what changes at PR 3c.2" pending-list.
4. `crates/wasm/src/lib.rs:1184` — delete the "PR 3c retires `from_parsed_unchecked` in favor of `MarkingScheme::canonicalize`; this call migrates then" comment block; replace with concise post-migration note.
5. `crates/wasm/src/lib.rs:1289` — same as 1184.

**Defer to 3c.2.E (8)**: `crates/core/src/lib.rs:13`, `crates/core/src/parser.rs:9, 52, 3871`, `crates/ism/src/attrs.rs:31`, `crates/ism/src/dissem_attribution.rs:52`, `crates/ism/src/lib.rs:18`, `crates/ism/src/parsed.rs:11`. These describe the adapter itself; they remain factually correct at the end of 3c.2.B.

`crates/ism/src/lib.rs:48` (the `pub use canonical::{CanonicalAttrs, from_parsed_unchecked};` re-export) is **code**, not a comment — do not touch in B. Re-export removal is the literal deletion at 3c.2.E.

### PM-B-5 — HRTB smoke test location: `crates/scheme/tests/hrtb_smoke.rs`

**Decision**: B-FOLLOWUP-1's compile-time HRTB smoke test lands at `/home/knitli/marque/crates/scheme/tests/hrtb_smoke.rs`.

**Rationale (override of architect's `crates/engine/tests/` recommendation)**: Placing the test in the crate that DECLARES the GAT (`marque-scheme`) gives minimum bisect distance. A future scheme implementor (CUI, NATO) whose binding destabilizes HRTB inference will see the test break in the same crate that introduced the regression. Engine-test placement is downstream; the trait-test placement catches it earlier.

Exact content per planner §4.

### PM-B-6 — Commit sequence: 5 commits

**Decision**: 5 atomic commits per planner §3. Each compiles standalone; each must pass T056 byte-identity at boundary.

- **B1**: `CapcoScheme::canonicalize` override implementation (body = `marque_ism::from_parsed_unchecked(parsed)` verbatim) + B-FOLLOWUP-1 HRTB smoke test at `crates/scheme/tests/hrtb_smoke.rs`. No call sites migrate yet. Trivially byte-identical (no production caller routes through the new path).
- **B2**: Engine production migration — `recognizer.rs:98` + `decoder.rs:411`. Each adds a module-scope `static SCHEME: LazyLock<CapcoScheme>` per PM-B-1 with the `TODO(engine-S-generic-recognizer-cleanup)` comment.
- **B3**: WASM (`lib.rs:1187, 1292`) + 9 engine in-`src/` test sites (`decoder.rs:6337, 6393, 6402, 6445, 6454, 6568, 6592, 6615, 6875`). WASM sites reuse the already-constructed `let scheme = CapcoScheme::new()` at lines 1167 and 1263 — zero new allocation.
- **B4**: 13 external test migrations across `crates/capco/tests/` (12 files) and `crates/engine/tests/document_corpus.rs` per PM-B-3.
- **B5**: Doc-comment sweep (5 updates per PM-B-4) + `// TODO(3c.2.E)` annotations on the 4 core sites per PM-B-2 + `// TODO(3c.2.C)` annotation on `s004_audit_content_ignorance.rs:65` per PM-B-7 + closeout attestation.

Each commit body cites the PM-B-N decisions it actuates. Each commit message has a "T056 byte-identity: verified" line.

### PM-B-7 — s004 deferral

**Decision**: Leave `crates/capco/tests/s004_audit_content_ignorance.rs:65` at adapter call. The file is `#![cfg(any())]`-disabled per its header (line 2: "legacy FixProposal-shape test disabled pending rewrite"). Migration is a no-op for byte-identity but the file produces no behavioral signal until the rewrite lands.

Add inline comment `// TODO(3c.2.C): migrate when test rewrite per Diagnostic-shape lands` adjacent to line 65. Mention deferral in B5 commit message.

### PM-B-8 — Total migration count

**Decision baseline (corrected in B6 reviewer-pass closure)**: 26 sites migrated in 3c.2.B; 5 sites carved out.

- MIGRATE: 4 production + 9 in-src tests + 13 external tests (12 capco + 1 engine) = 26
- CARVE-OUT: 1 (s004) + 4 (core sites) = 5

The architect preflight's Appendix A listed 11 `crates/capco/tests/` files; a 12th (`render_canonical_properties.rs:50`) surfaced at the implementation grep and was correctly migrated in B4 — see the erratum in §1 above. PM-B-2 + PM-B-7 explicitly document the carve-outs. Reviewer attestation must not flag "incomplete migration"; the 5-site carve-out is the contract.

### PM-B-9 — T056 byte-identity gate

**Decision**: T056 corpus regression matrix gates every commit. The override body is literal `marque_ism::from_parsed_unchecked(parsed)` — semantic equivalence is by construction.

Each commit's PR body must include the T056 result attestation: `T056 corpus regression: byte-identical NDJSON before/after`. The reviewer must confirm the attestation matches a run on the commit SHA in CI.

### PM-B-10 — Tests > coverage gate

**Decision**: Per user's directive, all code added in B must be covered by behavior-focused tests. The HRTB smoke test (B1) is a compile-only artifact — it doesn't need behavioral assertions. The override body in B1 is `marque_ism::from_parsed_unchecked(parsed)` verbatim — every existing test that runs through the canonicalize path now exercises the override transitively; this satisfies coverage of the new method without new tests duplicating existing assertions.

If CodeCov denies on B's PR, the implementation agent expands the test suite at the affected sites. Focus on behavior (does the trait route produce the same canonical output as the adapter for representative ParsedAttrs inputs?) rather than implementation specifics. A new behavior-focused test at the override site (in `crates/capco/tests/`) is recommended: assert byte-equivalence between `scheme.canonicalize(parsed.attrs)` and `marque_ism::from_parsed_unchecked(parsed.attrs)` for a representative set of ParsedAttrs values. This makes the byte-identity guarantee explicit at the unit-test level (not just at the T056 corpus level).

---

## §3 — Resolved open questions

**Architect OQ-B1** (engine scheme-instance source) → resolved by PM-B-1 (module-scope `LazyLock`).
**Architect OQ-B2 / Planner OQ-1** (core 4-site carve-out) → resolved by PM-B-2 (defer to 3c.2.E).
**Planner OQ-2** (production-site scheme construction strategy) → resolved by PM-B-1 (same).
**Planner OQ-3** (test-site scheme construction strategy) → resolved by PM-B-3 (inline per helper / per test).
**Planner OQ-4** (doc-comment sweep timing) → resolved by PM-B-4 (5 in B; 8 deferred).

---

## §4 — Risk register (inherited from preflight, PM-acknowledged)

- **R-B1** (LazyLock scheme construction): Addressed via PM-B-1. The smell is documented and tracked under `engine-S-generic-recognizer-cleanup`.
- **R-B2** (Constitution VII directionality): Not a violation. `marque-engine → marque-capco` edge already exists; `marque-engine → marque-scheme` edge already exists. No new edges.
- **R-B3** (WASM scheme construction): Zero new cost — schemes already in scope at both WASM sites.
- **R-B4** (Test scheme construction × 13 files): Acceptable. ~50 extra `CapcoScheme::new()` calls per `cargo test --workspace` run, microseconds each, dominated by Vec allocations.
- **R-B5** (Engine borrow lifetime): Not an issue. `CapcoScheme` held by value in Engine; `&self` borrow for canonicalize call is per-call only.
- **R-B6** (Constitution VII inversion at core sites): Addressed via PM-B-2 (defer to 3c.2.E).
- **R-B7** (s004 `cfg(any())` deferral): Addressed via PM-B-7 (defer to 3c.2.C).

---

## §5 — Reviewer attestation checklist

Three reviewers (rust-specialist, code-reviewer, system-architect) attest each item:

- [ ] CapcoScheme `canonicalize` override body is byte-identical to `marque_ism::from_parsed_unchecked(parsed)` — diff between override body and adapter body shows zero divergence.
- [ ] B-FOLLOWUP-1 HRTB smoke test landed at `crates/scheme/tests/hrtb_smoke.rs` per PM-B-5.
- [ ] PM-B-1 actuated: module-scope `static SCHEME: LazyLock<CapcoScheme>` at `recognizer.rs` and `decoder.rs`, each with `// TODO(engine-S-generic-recognizer-cleanup)` comment.
- [ ] PM-B-2 actuated: 4 core sites carry `// TODO(3c.2.E)` annotations; **none of the 4 sites migrated in B**.
- [ ] PM-B-3 actuated: external test migrations construct CapcoScheme inline per helper / per test; **no `LazyLock` in test code**.
- [ ] PM-B-4 actuated: 5 doc-comments updated; 8 deferred to 3c.2.E.
- [ ] PM-B-7 actuated: s004 site carries `// TODO(3c.2.C)` annotation; **site not migrated in B**.
- [ ] PM-B-8 actuated: 25 sites migrated, 5 carved out; reviewer can recount and confirm.
- [ ] PM-B-9 actuated: every commit's PR body has T056 byte-identity attestation matched to CI.
- [ ] PM-B-10 actuated: behavior-focused tests cover new code; CodeCov ≥80% on changed lines. Recommended explicit byte-equivalence test landed.
- [ ] Constitution VII directionality preserved: no new dep edges introduced; verified via `cargo metadata` graph diff before/after.
- [ ] Constitution V Principle V test-fixture carve-out preserved at every external `tests/` site (engine-only-promotion of `AppliedFix` not violated; this PR does not touch promotion).
- [ ] Constitution VIII citation discipline: this PR introduces no new `§<Letter>.<sub> p<N>` citations; if any incidental ones appear in code comments, each re-verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship.
- [ ] `cargo nextest run --workspace` green at every commit.
- [ ] `cargo fmt --all -- --check` clean (per CI rustfmt-stable).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] WASM CI matrix passes (`wasm-pack build crates/wasm`) at B3 and B5.
- [ ] tools/citation-lint runs clean (no new §-pattern false-positives introduced).
- [ ] T056 corpus regression matrix passes at every commit boundary; PR description aggregates results.
- [ ] Adjacent paths walked: each migration site reviewed against patterns 5-10 lines above and below to confirm no missed call (e.g., a second `from_parsed_unchecked` call in the same function).
- [ ] "Will we maintain this for 5 years?" — the `LazyLock<CapcoScheme>` static is explicitly transitional and tracked under `engine-S-generic-recognizer-cleanup`. Each carve-out has a `TODO(3c.2.E)` or `TODO(3c.2.C)` annotation naming the retirement PR.

---

## §6 — Carryovers to 3c.2.C

These items land at PR 3c.2.C preflight, recorded here for traceability:

- **C-carryover-1**: 4 core sites in `crates/core/{src,tests}/` may migrate at 3c.2.E (whichever sub-PR retires the adapter) — but if 3c.2.C touches `crates/core/src/parser.rs:3890` for any other reason (e.g., test helper rewrite), revisit per PM-B-2.
- **C-carryover-2**: `s004_audit_content_ignorance.rs` rewrite scope. The `#![cfg(any())]` gate is in place pending Diagnostic-shape rewrite per 3c.2.C scope. PM-B-7 commits to migrating the `from_parsed_unchecked` site as part of that rewrite, not separately.
- **C-carryover-3**: `engine-S-generic-recognizer-cleanup` follow-up issue must be filed and linked from the 3c.2.B PR body (so the `LazyLock` debt is visible in the post-merge tracker).
- **C-carryover-4**: 8 doc-comments deferred per PM-B-4 list explicitly into the 3c.2.E PM brief so the closeout sweep doesn't miss them.

---

**Contract status**: BINDING for PR 3c.2.B implementation.
