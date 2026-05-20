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

### B-FOLLOWUP-2: Site count baseline = 23 (not 26)

**Severity**: LOW (documentation accuracy).
**Source**: Code-reviewer F-2; cleanup committed at 3c.2.A close.

**Issue**: The PM-9 contract was authored citing "26 verified via grep at 2026-05-19" but the actual count of `impl MarkingScheme for X` blocks is **23** (1 production CapcoScheme + 22 stubs across 19 files; 4 stubs in `proptest_closure_rejects_non_monotone.rs` and 2 in `closure_derived_path.rs`). The PR 3c.2.A cleanup commit updated PM-9 to clarify this. 3c.2.B's inventory should start from 23.

**Action for 3c.2.B preflight**: When inventorying `from_parsed_unchecked` migration sites, use grep-direct counts (`grep -rn "impl MarkingScheme for " /home/knitli/marque/crates/`) and not the predecessor PM doc count.

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

### GH-FOLLOWUP-2: `sub_subsection`-dead-capability

**Severity**: LOW (YAGNI cleanup).
**Source**: Architect reviewer F-5.

**Issue**: `SectionRef::sub_subsection: Option<NonZeroU8>` is dead capability at PR 3c.2.A. CAPCO-2016 has zero subsections deeper than `§H.5` per the citation index — no `§H.5.4`-style reference exists in the source manual. The const fixture `_SAR_SUB_SUB` and the test `display_subsection_plus_sub_subsection_h5_4_p99` use a synthetic shape acknowledged by the test doc comment as a representative round-trip target.

**Action**: File a GitHub issue titled `sub_subsection-dead-capability` linking to:
- `crates/rules/src/citation.rs:158` (the field declaration).
- The synthetic test at `crates/rules/tests/citation_display_roundtrip.rs:97-105`.

**Resolution path**: After 3c.2.C lands (the `&'static str → Citation` migration), grep `with_sub_subsection` across `crates/`. If zero non-test consumers exist, the field is provable dead capability and can be retired in a follow-up PR. If a future CAPCO revision introduces 3-level subsections, the field's grow-path is justified.

### GH-FOLLOWUP-3: Test name cosmetic rename

**Severity**: LOW (cosmetic).
**Source**: Code-reviewer F-3.

**Issue**: The test name `display_subsection_plus_sub_subsection_h5_4_p99` at `citation_display_roundtrip.rs:97-105` references a synthetic citation form (`§H.5.4 p99`) that doesn't exist in CAPCO-2016. The test doc-comment acknowledges this honestly, but the test name itself implies a real citation.

**Action**: Either fold into GH-FOLLOWUP-2 (the same test goes away when sub_subsection is retired) OR rename to `display_three_level_form_round_trips` in a future cleanup. Not worth a dedicated PR; opportunistic.

---

## Items already addressed in PR 3c.2.A cleanup commit

### Addressed-1: Rustdoc broken link `[RenderContext]`

Fixed at `crates/scheme/src/canonical.rs:217` — changed `[`RenderContext`]` to `[`crate::RenderContext`]` so rustdoc resolves through the re-export.

### Addressed-2: PM-9 site count erratum

`docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` PM-9 updated with the post-implementation erratum noting the actual count is 23, not 26.

### Addressed-3: Send + Sync compile-time pins

`assert_impl_all!` pins added to test modules in `render_context.rs` (for `RenderContext`, `EmissionForm`, `SchemaVersionId`) and `citation.rs` (for `Citation`, `SectionRef`, `SectionLetter`, `AuthoritativeSource`). `static_assertions` added to `crates/scheme/Cargo.toml` `[dev-dependencies]`. Forward-defense: future field additions that break Send/Sync/Copy fail compilation.

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
