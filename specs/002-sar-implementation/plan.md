# SAR Implementation Plan

## Phase sequence

1. **P1 — Data model** (`marque-ism`)
   Remove the empty `SarIdentifier` enum from `build.rs`. Add `SarMarking`, `SarIndicator`, `SarProgram`, `SarCompartment` to `attrs.rs`. Replace `IsmAttributes.sar_identifiers` with `sar_markings: Option<SarMarking>`. Add new `TokenKind` variants. Mark `TokenKind::SarIdentifier` `#[deprecated]`. `cargo check --workspace` must pass — downstream crates will have placeholder matches for the new types until later phases fill them in.

2. **P2 — Subparser** (`marque-core`)
   Add `parse_sar_category(bytes: &[u8], base: usize) -> Option<(SarMarking, Vec<TokenSpan>)>` in `parser.rs`. Dispatch from the existing category-block loop when a block starts with `SAR-` or `SPECIAL ACCESS REQUIRED-`. Grammar exactly per `spec.md` §R2. Unit tests cover: the canonical §H.5 Table 7 example, the single-program cases (`SAR-BP`, `SPECIAL ACCESS REQUIRED-BUTTER POPCORN`), and multi-program (`SAR-BP/CD/XR`).

3. **P3 — Rules** (`marque-capco`)
   Six new rule structs in `rules.rs`, registered in `CapcoRuleSet::new()`. Each has `default_severity`, `check`, and an optional fix proposal. Unit tests per rule with positive and negative cases. All diagnostics cite `CAPCO-2016 §H.5` (or `§A.6` for E026, E030 where §A.6 is the stronger cite).

4. **P4 — Page roll-up** (`marque-ism` + `marque-capco`)
   Extend `PageContext` with `expected_sar_marking()`. Implement E031 (`sar-banner-rollup`) against the `PageContext` an engine supplies to banner/CAB candidates. Tests: single-portion, multi-portion, roll-up missing from banner.

5. **P5 — Corpus + harness**
   Add eight corpus fixtures (four valid, four invalid) per spec §SC-SAR. Extend the accuracy harness in `tests/corpus_accuracy.rs` (or the equivalent location — check before assuming) so SAR rules gate at ≥95%. Snapshot NDJSON output for the canonical banner.

6. **P6 — Documentation**
   Update `CLAUDE.md` with a SAR subsection under Architecture. Update `README.md` rule-count badge from 29 → 35 (or whatever the final count is). Add a migration note to `crates/marque-ism/README.md` explaining the `SarIdentifier` → `SarMarking` move.

## Side-fix: E004 should flag `//` between same-category tokens, not `/` within one

Discovered during scouting. `SeparatorCountRule::check` currently flags any single `/` inside a Classification or Unknown token as a missing category separator — that's the wrong direction. Per §A.6 Figure 2, `/` is the **correct** within-category separator; `//` is the category separator. The actionable bug is the inverse: when a user writes `SECRET//SI//TK//NOFORN`, the `//` between `SI` and `TK` is wrong because both are SCI controls and belong in the same category block (`SECRET//SI/TK//NOFORN`).

The rewritten rule walks adjacent category blocks, classifies the lead token of each side against the known CAPCO category parsers, and when both sides resolve to the same category emits a high-confidence (0.95) fix to replace `//` with `/`. When either side is unknown or the categories differ, the rule stays silent. Details in task #7. Keep the fix in the same branch but commit separately so `git log` stays legible.

## Risks

- **Back-compat break for `IsmAttributes`.** `sar_identifiers` is public. Downstream code (if any) that iterates it breaks. Mitigation: current surveys show only internal uses; we bump `marque-ism` to 0.3.0 as part of this branch.
- **Parser complexity.** The SAR grammar has three levels (program → compartment → sub-compartment) with two separators at each level (`-`, space, `/`). Handwritten recursive-descent is clearest; keep the subparser under 100 LOC and document each transition.
- **Ambiguity between full-form nickname and misspelled abbrev.** `SPECIAL ACCESS REQUIRED-BP` is technically legal (full indicator, abbrev nickname). Treat any identifier that matches the 2–3 char abbrev shape as `Abbrev` semantics regardless of indicator, but record the indicator as-parsed for E026.
- **Sort-order definition.** "Numeric first, then alpha, ascending" needs a total order. Use: split identifier at first non-digit; compare numeric prefix as u64 (missing = treat as 0 AND flag as alpha-only); fall back to bytewise ASCII for the alpha tail. Lock this in a shared helper.

## Acceptance

- All 6 new rules pass unit tests.
- Corpus harness green at ≥95% per rule.
- `cargo check --workspace` and `cargo test --workspace` green.
- WASM build produces byte-identical NDJSON for all SAR fixtures (SC-008 parity).
- No regressions in existing E001–E025, W001–W003, C001 tests.
- E004 side-fix landed with its own test additions.

## Branch hygiene

- Branch: `feat/sar-implementation`
- Commits: one per phase (P1..P6) plus one for the E004 side-fix plus one docs commit. Prefer real commits over squash so the phase boundaries stay readable.
- PR title: `feat(sar): structural SAR marking support per CAPCO-2016 §H.5`
