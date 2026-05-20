# PR 3c.2.A — Deferred Findings Register

**Date**: 2026-05-19
**Source**: 3-reviewer pass on PR 3c.2.A (`refactor-006-pr-3c2-a-scaffolding`)
**Status**: Tracking — items deferred from PR 3c.2.A merge gate; each targets a specific successor PR or follow-up issue.

This document captures all reviewer-identified findings that were NOT addressed in PR 3c.2.A (either because they belong in a later sub-PR or because they should become tracked GitHub issues). The cleanup commit landed in PR 3c.2.A covers (a) the rustdoc broken-link fix at `canonical.rs:217`, (b) the PM-9 site-count erratum, (c) explicit `Send + Sync + Copy` compile-time pins on `Citation` / `RenderContext` / `EmissionForm` / `SchemaVersionId` / `SectionRef` / `SectionLetter` / `AuthoritativeSource`.

---

## Items tracked for **PR 3c.2.B preflight**

### B-FOLLOWUP-1: HRTB smoke test for the new GAT

**Severity**: LOW (forward-defense).
**Source**: System-architect reviewer F-4.

**Issue**: `MarkingScheme::type Parsed<'src>` is the first GAT on the trait. Zero generic consumers of `S::Parsed<'_>` exist today; B's body lift uses `CapcoScheme` directly. But once a generic helper consuming `S::Parsed<'_>` lands (likely in 3c.2.B or 3c.2.D), HRTB inference (`for<'a>`) can surface "implementation not general enough" errors that are notoriously fragile to debug.

**Action for 3c.2.B**: After the `from_parsed_unchecked` body-lift lands, add a 5-line compile-time smoke test in the engine test surface:

```rust
fn _hrtb_smoke<S: MarkingScheme>(_scheme: &S)
where
    for<'a> S::Parsed<'a>: Sized,
{}
```

This catches HRTB inference issues at compile time, before they bite at a generic helper site.

**Closure (PR 3c.2.B B1, 2026-05-20)**: ~~CLOSED~~. Landed at `crates/scheme/tests/hrtb_smoke.rs` per PM-B-5 (the PM override of architect's `crates/engine/tests/` recommendation — placement in the GAT-declaring crate minimizes bisect distance when a future scheme implementor destabilizes HRTB inference). Compile-only test; `cargo check -p marque-scheme --tests` is the gate. The smoke test body matches the action snippet above plus a no-op `#[test] fn hrtb_smoke_compiles()` so the file appears in nextest output. System-architect reviewer attestation at PR 3c.2.B confirms the placement decision is defensible against the 5-year bar.

### B-FOLLOWUP-2: Site count baseline = 23 (not 26)

**Severity**: LOW (documentation accuracy).
**Source**: Code-reviewer F-2; cleanup committed at 3c.2.A close.

**Issue**: The PM-9 contract was authored citing "26 verified via grep at 2026-05-19" but the actual count of `impl MarkingScheme for X` blocks is **23** (1 production CapcoScheme + 22 stubs across 19 files; 4 stubs in `proptest_closure_rejects_non_monotone.rs` and 2 in `closure_derived_path.rs`). The PR 3c.2.A cleanup commit updated PM-9 to clarify this. 3c.2.B's inventory should start from 23.

**Action for 3c.2.B preflight**: When inventorying `from_parsed_unchecked` migration sites, use grep-direct counts (`grep -rn "impl MarkingScheme for " /home/knitli/marque/crates/`) and not the predecessor PM doc count.

**Closure (PR 3c.2.B preflight, 2026-05-20)**: ~~CLOSED~~. Architect preflight Appendix A re-verified the site inventory at 2026-05-20 via the prescribed grep: **30 caller sites** (14 production + 16 external test) — the correct baseline for 3c.2.B's migration accounting. PM contract PM-B-8 records the resulting 25-migrated + 5-carved-out split. Reviewer reconfirmed post-implementation count via `grep -rn "from_parsed_unchecked" crates/ --include='*.rs'`: 5 carve-outs + 2 byte-equivalence-test sites + 1 trait-override-body site + 1 adapter declaration + 1 re-export. Math holds.

---

## Items tracked for **PR 3c.2.C preflight**

### C-FOLLOWUP-1: citation-lint real-parser round-trip

**Severity**: LOW (test discipline).
**Source**: Architect reviewer F-1; rust-idiom reviewer R-3 (Display test scanner gap).

**Issue**: `citation_display_roundtrip.rs::matches_citation_lint_form` is a hand-rolled byte scanner that codifies the **expected** citation-lint shape, NOT a programmatic invocation of `tools/citation-lint/src/citation.rs::find_in_fragment`. If the citation-lint parser ever diverges from the hand-rolled scanner, the round-trip test passes while citation-lint rejects (or vice versa). The hand-rolled scanner also accepts `§H Table 2 p21` (no subsection, table-present), which the type system permits but `tools/citation-lint` likely rejects.

**Action for 3c.2.C**: When `Diagnostic.citation: &'static str → Citation` migrates, add one more test that round-trips a `format!("{citation}")` string THROUGH `tools/citation-lint`'s actual parser, asserting the parsed result matches the original `Citation` fields. This converts the unit-test gate to an integration-test gate against the real consumer.

### C-FOLLOWUP-2: `citation!()` macro for construction verbosity (opportunistic)

**Severity**: LOW (ergonomic).
**Source**: Architect reviewer F-6.

**Issue**: `Citation::new(AuthoritativeSource::Capco2016, SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()), NonZeroU16::new(61).unwrap())` is ~120 chars for what was previously a 9-char `&'static str`. The PM intentionally rejected a `citation!()` macro per D25.2 (const-fn surface is the chosen ergonomic floor), but ~41 sites in `crates/capco/src/` will migrate in 3c.2.C.

**Action for 3c.2.C**: If C's diff becomes noisy with verbose `Citation::new(...)` calls, add a declarative `citation!(§H.4 p61)` macro under `marque-rules`. The PM doesn't preclude this — the rejection was specifically of a compile-time-validation macro, not a sugar macro.

### C-FOLLOWUP-3: Stale forward-pointer comments referencing PR 3c.2.B

**Severity**: LOW (documentation drift).
**Source**: Code-reviewer pass on PR 3c.2.B (2026-05-20).

**Issue**: 5 doc-comments authored at PR 3c.2.A state "the §G.1 Table 4 dispatch body lands at PR 3c.2.B" or equivalent wording. PR 3c.2.B's scope was call-site migration, NOT EmissionForm dispatch (the A-stage authoring agent mis-predicted B's scope). These comments are now stale forward-pointers to a PR identifier that has merged with different content.

**Locations**:
- `/home/knitli/marque/crates/scheme/src/scheme.rs:604`
- `/home/knitli/marque/crates/scheme/src/scheme.rs:699`
- `/home/knitli/marque/crates/scheme/src/scheme.rs:734`
- `/home/knitli/marque/crates/capco/src/scheme/marking_scheme_impl.rs:587`
- `/home/knitli/marque/crates/capco/src/scheme/marking_scheme_impl.rs:674`

**Action for 3c.2.C**: Treat as MIGRATE-NOW in C's PM-B-4 analog. Update each to "a future PR will land the §G.1 Table 4 dispatch body" (or to whichever sub-PR actually lands EmissionForm dispatch, if that scope is now scheduled). Confirm at 3c.2.C preflight whether EmissionForm dispatch is scoped into C, D, E, or post-1.0; the comments must reflect the actual landing target.

### C-FOLLOWUP-4: rules_us1.rs migration parallel to s004 cfg-gate lift

**Severity**: LOW (inventory drift).
**Source**: Architect reviewer pass on PR 3c.2.B (2026-05-20), A8.

**Issue**: `crates/capco/tests/rules_us1.rs` is `#![cfg(any())]`-disabled at line 1 — same gate as `s004_audit_content_ignorance.rs` (both disabled per "PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite"). PM-B-7 explicitly carved out s004 but did not name rules_us1.rs alongside it; PM-B-3 listed rules_us1.rs in the 12-file external test migration inventory, and the implementation agent migrated it accordingly. The migration is benign (file is cfg-excluded; modified line never compiles), but it is a real inventory drift relative to the carve-out boundary.

**Action for 3c.2.C**: When the `#![cfg(any())]` gate on s004 lifts as part of the Diagnostic-shape rewrite, walk `rules_us1.rs` in lockstep — the pre-migrated `scheme.canonicalize(parsed.attrs)` shape at `rules_us1.rs:73` is already in place, so C's work for that file is purely the Diagnostic-shape rewrite, not the canonicalize migration.

### C-FOLLOWUP-5: Pre-existing `clippy::question_mark` at `crates/core/src/parser.rs:2199`

**Severity**: MEDIUM (CI gate; blocks workspace clippy strict mode).
**Source**: Rust-reviewer + code-reviewer pass on PR 3c.2.B (2026-05-20).

**Issue**: A pre-existing `clippy::question_mark` warning at `/home/knitli/marque/crates/core/src/parser.rs:2199` causes `cargo clippy --workspace --all-targets -- -D warnings` to fail. The warning was present in base commit `861e85e3` (PR 3c.2.A merge to staging); not introduced by 3c.2.B. The fix is mechanical: replace `else if let Some(p) = trimmed.strip_suffix(" EYES") { (p, false) } else { return None; }` with `else { let p = trimmed.strip_suffix(" EYES")?; (p, false) };` — a one-line change.

**Action for 3c.2.C**: Resolve as opening housekeeping commit of 3c.2.C (or a standalone chore PR before C lands). Required before workspace clippy strict mode can re-enable.

### C-FOLLOWUP-6: Byte-equivalence test §-citation re-verification at 3c.2.E

**Severity**: LOW (citation discipline at lifecycle boundary).
**Source**: Architect reviewer pass on PR 3c.2.B (2026-05-20), A6.

**Issue**: `crates/capco/tests/canonicalize_byte_equivalence.rs` carries a `§H.4 p80` citation in a doc-comment for the `portion_sci_si_g_with_orcon_noforn` fixture. The citation is correct at PR 3c.2.B authorship; the file's lifetime ends at PR 3c.2.E (the test header documents this at lines 18-23). When 3c.2.E retires the adapter, the file either deletes or refactors to a second oracle. If refactored, the §H.4 p80 citation re-verifies per Constitution VIII (propagation rule: every citation move requires re-verification at point of propagation).

**Action for 3c.2.E**: Treat the §H.4 p80 citation as a propagation event under Constitution VIII when sweeping the byte-equivalence test. Re-verify the citation against `crates/capco/docs/CAPCO-2016.md` at point of propagation; do not let it accrete unchecked.

---

## Items addressed in PR 3c.2.B B6 reviewer-pass closeout (2026-05-20)

### B6-Addressed-1: TODO issue-number citation per architect LOW-1

**Source**: System-architect reviewer pass, LOW-1.
**Resolution**: 5 TODO references to `engine-S-generic-recognizer-cleanup` updated to also cite GitHub issue `#634`:
- `crates/engine/src/recognizer.rs:64` — narrative reference updated to include `(#634)`
- `crates/engine/src/recognizer.rs:75-79` — TODO block updated to `TODO(engine-S-generic-recognizer-cleanup, #634)` with `#634` in body
- `crates/engine/src/recognizer.rs:131` (was :130 pre-edit) — in-function reference updated to include `(#634)`
- `crates/engine/src/decoder.rs:119-123` — TODO block updated to `TODO(engine-S-generic-recognizer-cleanup, #634)` with `#634` in body
- `crates/engine/src/decoder.rs:434` (was :433 pre-edit) — in-function reference updated to include `(#634)`

Both symbolic name (`engine-S-generic-recognizer-cleanup`) AND issue number (`#634`) now present at every TODO site. `git grep "#634" crates/` returns 5 hits; `git grep "engine-S-generic-recognizer-cleanup" crates/` returns 5 hits.

### B6-Addressed-2: PM-B-8 site count reconciliation 25 → 26

**Source**: Code-reviewer pass, LOW finding.
**Resolution**: `docs/plans/2026-05-20-pr3c2-b-pm-decisions.md` PM-B-8 and §1 updated to reflect actual migration count of 26 (4 production + 9 in-src tests + 13 external sites across 12 files). The architect preflight Appendix A had listed 11 external `crates/capco/tests/` files; `render_canonical_properties.rs:50` was uncovered at implementation-grep time and correctly migrated in B4. The B5 commit message already acknowledged the discrepancy.

### B6-Addressed-3: Missing tactical-plan.md file

**Source**: Rust-reviewer pass, LOW finding.
**Resolution**: `docs/plans/2026-05-20-pr3c2-b-tactical-plan.md` created as a redirect to the binding PM contract. The Plan-agent preflight pass returned its tactical content inline rather than writing to file; the redirect file documents this drift and lists the load-bearing tactical findings that were folded into the PM contract. Future preflight briefs should explicitly require Write tool calls before agent return.

---

## Items tracked as **follow-up GitHub issues** (file before 3c.2.B starts)

### GH-FOLLOWUP-1: `citation-scheme-genericity`

**Severity**: MEDIUM (load-bearing 5-year question).
**Source**: Architect reviewer F-2 + F-3 (overlap), code-reviewer §5.

**Issue**: At scheme #2 adoption (CUI / NATO / partner-national), two surfaces will need refactoring:

1. **`SectionLetter`** is closed to `{A..=H}` (CAPCO-only). CUI uses category codes (`AGRI`, `BANK`, `OPSEC`, etc.); NATO uses different section vocabulary. Options: either `Citation` becomes generic-over-scheme (`Citation<S>` with `S::SectionToken` associated type) OR `SectionLetter` extended to a closed-enum-per-scheme.
2. **`EmissionForm`** variants `BannerTitle` / `BannerAbbreviation` reflect CAPCO §G.1 Table 4 column terms. CUI lacks this title/abbreviation duality at the token level; CUI's `render_canonical` will collapse both via the FR-052 fallback (`banner_abbreviation: None → banner_title`). The CAPCO-coupling is documented under the constitutional spec-lines-472-481 trait-surface-instability carve-out, but if it ossifies, scheme #2 hits a much larger refactor.

**Action**: File a GitHub issue titled `citation-scheme-genericity` linking to:
- `crates/rules/src/citation.rs` `SectionLetter` definition (lines 212-223).
- `crates/scheme/src/render_context.rs` `EmissionForm` variant declarations (lines 100-152).
- This deferred-findings document.

The issue's resolution lands in a PR titled `Citation::Scheme-genericity` at scheme #2 adoption time (≥18 months out per estimate). Tracking it makes the cost visible so it doesn't ossify into a silent 5-year commitment.

### ~~GH-FOLLOWUP-2: `sub_subsection`-dead-capability~~ — **CLOSED in A7**

**Severity**: LOW (YAGNI cleanup).
**Source**: Architect reviewer F-5 + Copilot inline review on PR #627.

**Original issue**: `SectionRef::sub_subsection: Option<NonZeroU8>` was dead capability at PR 3c.2.A. CAPCO-2016 has zero subsections deeper than `§H.5` per the citation index. Architect F-5 flagged it for YAGNI retirement; Copilot's PR #627 inline review found the related type-safety hole (`with_sub_subsection` allowed constructing an invalid `subsection: None + sub_subsection: Some` state).

**Resolution (A7 commit on PR #627)**: `sub_subsection` field + `with_sub_subsection` builder + `_SAR_SUB_SUB` const fixture + `display_subsection_plus_sub_subsection_h5_4_p99` test + scanner regex sub_subsection branch + proptest generator sub_subsection branch all removed. Display now emits `§<L>[.<sub>] [Table <N>] p<page>` only.

If a future revision of CAPCO-2016 or a different authoritative source introduces 3-level subsections, the field re-extends additively via `#[non_exhaustive]` on `SectionRef`.

### ~~GH-FOLLOWUP-3: Test name cosmetic rename~~ — **CLOSED in A7**

**Severity**: LOW (cosmetic).
**Source**: Code-reviewer F-3.

**Original issue**: The test name `display_subsection_plus_sub_subsection_h5_4_p99` at `citation_display_roundtrip.rs:97-105` referenced a synthetic citation form not in CAPCO-2016.

**Resolution (A7)**: Test retired alongside `sub_subsection` field removal — closed by removal, not rename.

---

## Items already addressed in PR 3c.2.A cleanup commits

### Addressed-1: Rustdoc broken link `[RenderContext]` (A6)

Fixed at `crates/scheme/src/canonical.rs:217` — changed `[`RenderContext`]` to `[`crate::RenderContext`]` so rustdoc resolves through the re-export.

### Addressed-2: PM-9 site count erratum (A6 + A7)

`docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` PM-9 updated with the post-implementation erratum noting the actual count is 23, not 26. A7 swept the remaining "26" references in R-A3 + the A4 commit table row + the reviewer attestation checklist after Copilot's PR #627 inline review surfaced the leftover instances.

### Addressed-3: Send + Sync compile-time pins (A6)

`assert_impl_all!` pins added to test modules in `render_context.rs` (for `RenderContext`, `EmissionForm`, `SchemaVersionId`) and `citation.rs` (for `Citation`, `SectionRef`, `SectionLetter`, `AuthoritativeSource`). `static_assertions` added to `crates/scheme/Cargo.toml` `[dev-dependencies]`. Forward-defense: future field additions that break Send/Sync/Copy fail compilation.

### Addressed-4: `sub_subsection` retirement (A7)

`sub_subsection` field + `with_sub_subsection` builder + dead test fixtures retired from `Citation` / `SectionRef` per Copilot inline review on PR #627 + architect F-5 / code-reviewer F-3. See GH-FOLLOWUP-2 and GH-FOLLOWUP-3 above (both closed).

### Addressed-5: `cargo fmt --check` failure (A7)

Stable rustfmt wanted multi-line const declarations in `crates/scheme/src/render_context.rs` test module that local nightly rustfmt rendered single-line. `cargo fmt --all` applied to reformat. CI `Format + Lint` job passes post-A7.

### Addressed-6: PM doc commit-count accuracy (A7)

PM doc §2 "Commit sequence inside PR 3c.2.A" extended from 5 commits to 7 to include A6 (reviewer-pass cleanup) and A7 (Copilot-review cleanup) per Copilot inline review on PR #627. The "Five logical commits" framing now reads "Seven commits land in PR 3c.2.A; A1–A5 are the logical scaffolding commits, A6 is the reviewer-pass cleanup, A7 is the Copilot-review cleanup."

---

## Reviewer pass summary

Three parallel reviewers ran against 5 commits on `refactor-006-pr-3c2-a-scaffolding`:

| Reviewer | Verdict | Critical findings |
|---|---|---|
| Rust-idiom | PASS | 0 critical, 0 high, 4 low (commit-message off-by-one; missing Send+Sync pins → addressed; Display scanner gap → C-FOLLOWUP-1; intentional Display non-rendering of `document`) |
| Code-reviewer (Constitution VII / adjacent paths) | PASS-WITH-MINOR-FIXES | 0 critical, 0 high, 1 medium (citation-scheme-genericity → GH-FOLLOWUP-1), 3 low (rustdoc broken link → addressed; PM-9 count → addressed; test name → GH-FOLLOWUP-3) |
| System-architect (5-year / sub-PR enablement) | PASS-WITH-MINOR-FIXES | 0 critical, 0 high, 1 medium (citation-scheme-genericity → GH-FOLLOWUP-1), 5 low (citation-lint round-trip → C-FOLLOWUP-1; EmissionForm CAPCO-coupling → GH-FOLLOWUP-1; HRTB smoke test → B-FOLLOWUP-1; sub_subsection dead capability → GH-FOLLOWUP-2; Citation construction verbosity → C-FOLLOWUP-2) |

**Consolidated verdict**: PASS for merge after cleanup commit lands.

5-year-maintenance-bar standard: the trait-surface decisions in PR 3c.2.A are sound for CAPCO today and forward-compatible with CUI/NATO future adoption under the spec lines 472-481 trait-surface-instability carve-out. The two CAPCO-coupled surfaces (`SectionLetter`, `EmissionForm`) are explicitly tracked in GH-FOLLOWUP-1 so the cost is visible.
