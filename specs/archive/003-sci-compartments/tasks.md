<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# SCI Compartments Tasks

Phase layout mirrors `specs/002-sar-implementation/tasks.md`. Each phase is one commit; agents claim a phase via the task harness.

## P1 — Data model (`marque-ism`)

- [ ] Add `SciMarking`, `SciControlSystem`, `SciControlBare`, `SciCompartment` to `crates/ism/src/attrs.rs` with `#[non_exhaustive]` and `::new()` constructors (lesson from SAR P2 — non_exhaustive blocks struct-literal construction from `marque-core`).
- [ ] Add `IsmAttributes.sci_markings: Box<[SciMarking]>` alongside the existing `sci_controls: Box<[SciControl]>`. Do NOT remove or deprecate `sci_controls`.
- [ ] Add `TokenKind::SciSystem` and `TokenKind::SciSubCompartment`. Audit whether `TokenKind::SciCompartment` exists already; if so, reuse; if not, add it.
- [ ] Re-export new types from `crates/ism/src/lib.rs`.
- [ ] Derive `SciControlBare` from the CVE at build time (per QA review: the 2016 manual publishes only 4 systems, the 2022 CVE has 7 — the recognition set must follow the live CVE, not a hardcoded list). `build.rs` emits a `SciControlBare` enum containing every CVE value whose text contains no `-`. Provides a `fn is_bare_cve_value(s: &str) -> bool` helper for the structural anchor check.
- [ ] `cargo check --workspace` green with no changes to `marque-core` or `marque-capco` consumers yet (P2 wires them).

## P2 — Subparser (`marque-core`)

- [ ] Add `parse_sci_block(text, base, tokens) -> Option<(Vec<SciMarking>, Vec<TokenSpan>)>`.
- [ ] Dispatch from `IsmAttributes::from_marking_bytes` category-block loop BEFORE `SciControl::parse(trimmed)`. If `parse_sci_block` returns `Some`, use it; else fall back to the existing exact-match path.
- [ ] Grammar per spec §R2. Recursive descent:
  1. Split block on `/` → per-system chunks.
  2. For each chunk, split on first `-` → `(control, rest)`.
  3. Validate `control`: bare CVE match OR custom shape `[A-Z0-9]{2,5}`.
  4. For the rest, iterate: `-COMP (SPACE SUB)*`.
- [ ] After structural parse, populate `canonical_enum` by checking `format!("{control}-{first_comp}").parse::<SciControl>()` — ONLY if that compartment has no sub-compartments (subs imply the compound is an anchor, not atomic).
- [ ] Populate `IsmAttributes.sci_markings` AND maintain the existing `sci_controls` population for back-compat (projection from `canonical_enum` per-marking).
- [ ] Unit tests:
  - Existing happy paths continue to parse identically (no regression).
  - `SI-G ABCD` → SciMarking{ Si, [{G, [ABCD]}] }, canonical_enum = None (subs present).
  - `HCS-P` → SciMarking{ Hcs, [{P, []}] }, canonical_enum = Some(HcsP).
  - `123/SI-G ABCD DEFG-MMM AACD` → two markings with correct nesting.
  - `99` → SciMarking{ Custom("99"), [] }, canonical_enum = None.
  - Rejection: `SI-` (dangling hyphen), `-SI` (leading hyphen), empty string, lowercase letters.
- [ ] Regression: run every existing SCI-related test in `marque-core` and `marque-capco` — all must pass unchanged.

## P3 — Rules (`marque-capco`)

- [ ] **E010 `bare-hcs`** audit: current logic fires on `SciControl::Hcs` presence. New logic: ALSO check `sci_markings` — if any `SciMarking { system: Published(Hcs), compartments: [] }` appears, fire. Do NOT fire when HCS has any compartments (those are `HCS-P/O/X/...` compounds). Preserve exact existing semantics for enum-path input; add structural-path detection.
- [ ] **E011 `missing-non-us-prefix`** audit: no behavioral change expected. Add a regression test exercising the structural path to confirm.
- [ ] **E032 `sci-system-order`** (new): walk `sci_markings`, detect out-of-order adjacent pairs (reuse SAR's `sar_sort_key` semantics — numeric first, alpha after — either via shared helper if SAR landed, or a local copy). Fix: reorder, confidence 0.85. Cite `CAPCO-2016 §A.6`.
- [ ] **E033 `sci-compartment-order`** (new): per marking, check compartments ascending; per compartment, check sub-compartments ascending. Fix: reorder, confidence 0.85. Cite `CAPCO-2016 §A.6`.
- [ ] **E034 `sci-custom-control-info`** (new): emit Info (no fix) for each `SciMarking` with `system: Custom(_)`. Per QA review, the 2016 manual treats unpublished systems as legitimate; this rule is **informational** for audit visibility, not a suggestion that the marking is incorrect. Cite `CAPCO-2016 §A.6 p16; §H.4 p61`. Default severity: `Info` (or `Off` if the severity enum doesn't distinguish Info from Warn — in that case, ship it `Off` by default and require `--include-info` to enable).
- [ ] **E035 `sci-banner-rollup`** (new): consume `ctx.page_context.expected_sci_markings()`, compare against `attrs.sci_markings`, fire on missing compartments/sub-compartments per system. Fix: rebuild the SCI block. Confidence 0.9. Cite `CAPCO-2016 §H.4` per-system Precedence Rules (e.g., p62, p64, p66, p68) as primary; `§D.2 p28` as supporting.
- [ ] **E008 skip filter** extension: tokens starting with a bare SCI system name followed by `-` or space are claimed by the structural parser; if the parser didn't claim them (returned None), they remain Unknown and E008 fires as before.
- [ ] Register E032–E035 in `CapcoRuleSet::new()`. Rule count: 35 (with SAR) + 4 = 39, or 29 + 4 = 33 (without SAR) — pick whichever the base branch has at merge time.
- [ ] Unit tests per new rule with positive and negative cases. Positive for E033 sub-compartment: `SECRET//SI-G DEFG ABCD//NOFORN` (should reorder DEFG, ABCD → ABCD, DEFG).

## P4 — Page roll-up (`marque-ism`)

- [ ] `PageContext::expected_sci_markings() -> Box<[SciMarking]>`. Union semantics: `BTreeMap<SciControlSystem, BTreeMap<String, BTreeSet<String>>>` keyed system → compartment → sub-compartments.
- [ ] Integrate into `render_expected_banner()`: when both `expected_sci_controls()` (old enum path) and `expected_sci_markings()` are populated, prefer the structural rendering for the SCI block. When a control system has no compartments, render as bare (matches existing output). When compartments are present, render `CTRL-COMP1 SUB-COMP2 SUB` form.
- [ ] Tests for rollup: single portion; two portions merging compartments under same system; two portions with different systems.

## P5 — Corpus + harness

- [ ] `valid/sci_canonical_subcompartments.txt` — `TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN` (§A.6 p15 canonical)
- [ ] `valid/sci_bare_single_sub.txt` — `SECRET//SI-G ABCD//NOFORN`
- [ ] `valid/sci_multi_subs.txt` — `SECRET//HCS-P OPS INTEL//NOFORN`
- [ ] `valid/sci_custom_control.txt` — `TOP SECRET//99/SI//NOFORN`
- [ ] `invalid/sci_system_order.txt` — `TOP SECRET//SI/123//NOFORN` → E032
- [ ] `invalid/sci_compartment_order.txt` — `SECRET//SI-NK-G//NOFORN` → E033 (NK then G — alpha out of order)
- [ ] `invalid/sci_subcompartment_order.txt` — `SECRET//SI-G DEFG ABCD//NOFORN` → E033
- [ ] `invalid/sci_custom_control_info.txt` — `SECRET//999//NOFORN` → E034 (Info severity; only fires if info-level diagnostics are enabled in the harness config). Rename fixture to match the rule's final name.
- [ ] `invalid/sci_banner_missing_compartment.txt` — multi-line; portion has SI-G ABCD, banner has SI-G only → E035
- [ ] Harness: if SC-003 harness already auto-discovers, no changes needed (cf. SAR P5 report). Confirm per-rule accuracy gating catches E032–E035.
- [ ] **Regression check**: enumerate all existing SCI fixtures and confirm they still produce the same diagnostics (or absence thereof) after this branch.

## P6 — Documentation

- [ ] `CLAUDE.md`: SCI subsection under Architecture mentioning the hybrid CVE + structural approach and the `sci_markings` field. Bump rule count.
- [ ] `README.md`: update rule count + add SCI mention to features.
- [ ] `crates/ism/README.md`: migration note (non-breaking; `sci_markings` is additive).

## Coordination with SAR branch

- If `feat/sar-implementation` (#18) merges before this branch starts implementation, rebase onto the new main. `sar_sort_key` may have been promoted to a shared helper; reuse it.
- If SAR is still open when this branch begins P2+, the shared helper is duplicated locally (document in the commit message so reviewers know it's intentional).

## Exit criteria

- [ ] All items in P1–P6 checked.
- [ ] `cargo check --workspace` green.
- [ ] `cargo test --workspace` green except pre-existing SC-006 failure (known unrelated).
- [ ] §A.6 p15 canonical example parses correctly.
- [ ] Zero regressions in existing SCI tests.
- [ ] PR opened against main.
