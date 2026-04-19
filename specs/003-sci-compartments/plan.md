<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# SCI Compartments Implementation Plan

## Phase sequence

Mirrors the SAR phase structure (see `feat/sar-implementation` / PR #18 for working reference patterns) but diverges in two places:

1. The CVE is **partially populated**, not empty — parser keeps both a structural view (`sci_markings`) and the enum projection (`sci_controls`).
2. Existing corpus fixtures must keep passing unchanged — this is a strict non-regression constraint.

### P1 — Data model (`marque-ism`)

- Add `SciMarking`, `SciControlSystem`, `SciControlBare`, `SciCompartment` to `attrs.rs`.
- Add `IsmAttributes.sci_markings: Box<[SciMarking]>`. **Keep** `IsmAttributes.sci_controls: Box<[SciControl]>` as a back-compat projection.
- Add `TokenKind::SciSystem`, `::SciSubCompartment`. Reuse the existing `::SciCompartment` token kind where it already exists (audit first — may only be notional).
- `build.rs` stays unchanged — `CVEnumISMSCIControls.xml` continues to drive `SciControl`. Add a const array of the 7 bare controls for the structural anchor list; generate it from the same XML.
- `cargo check --workspace` green.

### P2 — Subparser (`marque-core`)

- Add `parse_sci_block(text: &str, base: usize, tokens: &dyn TokenSet) -> Option<(Vec<SciMarking>, Vec<TokenSpan>)>` dispatched from the category-block loop BEFORE the `SciControl::parse` exact-match path.
- Grammar per `spec.md` §R2. Hand-written recursive-descent, no regex.
- For each parsed `SciMarking`, attempt `SciControl::parse(full_composite)` where `full_composite` is the bare-system + first compartment (e.g., `SI-G`). If it hits, store the enum in `canonical_enum`. This preserves the pre-registered compound behavior.
- The `SciControl::parse` exact-match fallback is retained for inputs the structural parser doesn't claim (belt-and-suspenders).
- Unit tests cover:
  - All existing happy paths (`SI`, `SI-G`, `HCS-P`, `TK`, `(TS//SI/TK//NF)`)
  - Sub-compartments: `SI-G ABCD`, `HCS-P SOMECOMP`
  - Multi-compartment: `SI-G ABCD DEFG-MMM AACD`
  - Custom control: `123`, `99/SI-G` (custom mixed with published)
  - §A.6 p15 canonical full example
  - Rejection: `SI-` (dangling hyphen), empty compartment id

### P3 — Rule updates + new rules (`marque-capco`)

- **E010 `bare-hcs`**: audit — when the structural parser recognizes `HCS` with compartments, the rule should NOT fire (HCS with a compartment is fine). Update the fire predicate to check `sci_markings` for any `Hcs` system with empty `compartments`, rather than just checking the enum for bare `Hcs`.
- **E011 `missing-non-us-prefix`**: audit — continues to consume `sci_controls`. Structural changes shouldn't affect it, but verify with a regression test.
- **E032 `sci-system-order`** (new): within one SCI block, control systems must be ascending (numeric first, alpha after). Reuse the `sar_sort_key` helper from SAR (or factor to a shared utility).
- **E033 `sci-compartment-order`** (new): within a system, compartments ascending; within a compartment, sub-compartments ascending.
- **E034 `sci-custom-control-warning`** (new): when `SciControlSystem::Custom(...)` appears, emit a warning (severity `Warn`, no fix) recommending verification that the control is agency-allocated.
- **E035 `sci-banner-rollup`** (new): mirror of SAR's E031. Banner must contain all compartments/sub-compartments present in portions for the same control system.
- **E008 skip filter**: extend to skip Unknown tokens that start with a bare SCI control followed by `-` or space (the new parser owns these).
- Register E032–E035 in `CapcoRuleSet::new()`.

### P4 — Page roll-up (`marque-ism` + `marque-capco`)

- `PageContext::expected_sci_markings() -> Box<[SciMarking]>` returning rolled-up structural markings. Union semantics by `(control_system, compartment_identifier)`; sub-compartments unioned per compartment. Each level sorted per §A.6 ordering.
- Extend `render_expected_banner()` to emit the rolled-up SCI block (replacing or supplementing the current `expected_sci_controls()` enum-based path).
- E035 rule consumes `expected_sci_markings()` to compare against observed banner.

### P5 — Corpus + harness

New fixtures:
- `valid/sci_canonical_subcompartments.txt` — `TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN` (§A.6 p15 manual example)
- `valid/sci_bare_single_sub.txt` — `SECRET//SI-G ABCD//NOFORN`
- `valid/sci_multi_subs.txt` — `SECRET//HCS-P OPS INTEL//NOFORN`
- `valid/sci_custom_control.txt` — `TOP SECRET//99/SI//NOFORN`
- `invalid/sci_system_order.txt` — `TOP SECRET//SI/123//NOFORN` (numeric should come first) → E032
- `invalid/sci_compartment_order.txt` — `SECRET//SI-NK-G//NOFORN` (letters out of order) → E033
- `invalid/sci_subcompartment_order.txt` — `SECRET//SI-G DEFG ABCD//NOFORN` → E033
- `invalid/sci_banner_missing_compartment.txt` — multi-line, portion has `SI-G ABCD`, banner has only `SI-G` → E035

Existing SCI fixtures (every `SECRET//SI//NOFORN`, `TOP SECRET//HCS-P//NOFORN`, `(TS//SI/TK//NF)`, etc.) MUST continue to pass unchanged. Add a regression-check sub-section to the harness.

### P6 — Documentation

- `CLAUDE.md`: new SCI subsection under Architecture / Two-Layer Rule Architecture explaining the hybrid CVE + structural approach. Bump rule count.
- `README.md`: update feature/rule count mention.
- `crates/ism/README.md`: migration note for the added `sci_markings` field (non-breaking; `sci_controls` retained).

## Shared code with SAR

The `sar_sort_key` helper duplicated across `marque-ism::page_context` and `marque-capco::rules` is a candidate for promotion to a shared utility once the SAR PR merges. Acceptable workaround for this branch: third private copy under SCI's namespace that returns structurally-identical tuples. If the SAR PR lands first, refactor in this branch to consume the promoted helper.

## Risks

- **Back-compat break**: `IsmAttributes.sci_controls` is public and widely read. Keeping it as an enum projection avoids breaking consumers but doubles the storage. Alternative: deprecate `sci_controls` in 0.4.0, remove in 0.5.0. Decision: keep for 0.4.0, evaluate deprecation separately.
- **E010 regression**: if the structural parser inadvertently changes `sci_controls` population (e.g., produces `SciControl::SiG` where old parser produced an error), existing E010 tests may change semantics. Mitigation: run existing E010 test file against P2 output before starting P3; gate P2 merge on no E010 behavior change.
- **Parser ambiguity**: `SI-ABCD` could be (a) an enumerated `SI-EU`-like value (no, `ABCD` isn't in the CVE), (b) a custom compound control system, or (c) `SI` with an unpublished compartment `ABCD`. Decision: always try CVE exact match first; if miss, decompose as `SI` + compartment `ABCD`.
- **Canonical-enum preservation**: `SI-G ABCD` in the structural path produces marking with `system=Si, compartments=[G{subs=[ABCD]}]`, which doesn't obviously match `SciControl::SiG`. Decision: the `canonical_enum` field is populated by checking `"{system}-{first_compartment}".parse::<SciControl>()` after parsing; if it hits, record the enum — but only when no sub-compartments are present on that compartment (subs imply the G is an anchor, not a compound atom).

## Acceptance

- All 4 new rules + 2 audited rules pass unit tests.
- All existing SCI tests green (regression-free).
- Corpus harness green at ≥95% per rule (including new ones).
- `cargo check --workspace` and `cargo test --workspace` green.
- WASM parity (SC-008): byte-identical NDJSON for new fixtures.
- CAPCO-2016 §A.6 p15 canonical example parses correctly end-to-end.

## Branch hygiene

- Branch: `feat/sci-compartments` (based on current `main` — independent of SAR PR #18 for now; rebase if SAR lands first and introduces shared helpers).
- Commits: one per phase + one for each audited rule's regression check + side-commits as needed.
- PR title: `feat(sci): structural compartment/sub-compartment support per CAPCO-2016 §A.6`.
- Opens against `main` once SAR PR #18 has merged OR on a clearly documented assumption that neither PR's changes conflict (the two touch different data structures and parser paths).
