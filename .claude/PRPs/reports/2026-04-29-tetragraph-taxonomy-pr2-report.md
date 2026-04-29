<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Implementation Report: Issue #206 (PR-2) — S005/S006 `rel-to-opaque-uncertain-reduction`

## Summary

PR-2 of the merged plan `docs/plans/2026-04-28-tetragraph-taxonomy-and-uncertain-reduction.md` is implemented as **two** registered rules sharing one analysis helper:

- **S005** (`rel-to-opaque-uncertain-reduction`, `Severity::Suggest`) — fires when a REL TO tetragraph with uncertain ISMCAT membership drops out of the page-level atom-semantics intersection AND the banner has no REL TO (active validation) OR the banner's REL TO is missing a code atom-semantics says should survive (`expected ⊄ banner_atomic`).
- **S006** (`rel-to-opaque-uncertain-reduction-info`, `Severity::Info`) — same trigger as S005, but fires when the banner is consistent with atom-semantics (`expected ⊆ banner_atomic`). Audit-only signal for the case where the producer plausibly drew on membership data we don't have.

The conceptual design is plan §3.1's "split severity" rule. The two-rule split is forced by `marque_engine::Engine::lint`'s severity-override behavior (`engine.rs:641` unconditionally overwrites every emitted diagnostic's severity with the rule's configured/default severity), which was caught by Copilot review on the initial single-rule implementation. Two rules sharing an `analyze_uncertain_reduction` helper preserve the plan's intent without changing engine semantics.

Together the pair is the second consumer of the suggest-don't-fix channel (after S004, PR #242) and the first consumer of the new `marque_ism::is_decomposable` discriminator landed in PR-1 (#208). They surface verbatim ODNI taxonomy `<Description>` text via `lookup_tetragraph_provenance`, closing PR-1's open use-case for the description payload.

A small public-API addition supports this: `marque_ism::PageContext::portions(&self) -> &[IsmAttributes]`. Most banner rules want one of the rolled-up `expected_*` accessors; S005 is the first rule that needs the pre-rollup view because the diagnostic depends on which portion contributed which code, not on the rolled-up answer. The accessor doc comment makes this explicit so future rules don't reach for raw portions when an aggregator would do.

## Open Question Resolution

Plan §8 Q3 (banner-consistency primitive) — **resolved**. Verified against `crates/ism/schemas/ISM-v2022-DEC/Schematron/ISM_XML-ROLLUP-phase.xsl` (`util:expandDecomposableTetras`, lines 387–410) that ODNI's rollup compares post-expansion atom sets, not raw token strings. S005's `expected ⊆ banner_atomic` check matches that contract: both portions and banner are expanded via `expand_tetragraph` before comparison.

## Assessment vs Reality

| Metric | Predicted (Plan) | Actual |
|---|---|---|
| Sequencing | Two PRs, #206 second | PR-2 implemented after PR-1 merged |
| Files changed | 2 (per plan §3.4) | 3 (added `page_context.rs` for the `portions()` accessor) |
| Severity model | Split (Info / Suggest) | Split (Info / Suggest) — implemented per §3.2 |
| Test cases | 7 fire / no-fire (§3.5) | 17 unit tests (7 fire/no-fire + state-text variants + audit-content-ignorance) |
| Rule count | 57 | 57 (56 + 1) |

## Tasks Completed

| # | Task | Status | Notes |
|---|------|--------|-------|
| 1 | Resolve plan §8 Q3 banner-consistency primitive | ✅ Complete | XSL inspection: `util:expandDecomposableTetras` confirms post-expansion comparison |
| 2 | Add `PageContext::portions()` accessor | ✅ Complete | Doc comment explains it's the first per-portion-needing rule; future rules should prefer `expected_*` |
| 3 | Implement `RelToOpaqueUncertainReductionRule` | ✅ Complete | Trichotomy split (Info / Suggest), trigraph filter, `s005_state_text` covers all four `is_decomposable=None` shapes |
| 4 | Register S005 in `CapcoRuleSet::new()` after E052 | ✅ Complete | Per plan §3.6 |
| 5 | S005 fire / no-fire / Info-vs-Suggest tests | ✅ Complete | Inline unit tests in `rules.rs::tests` (the natural home for diagnostic-level tests; rel_to_invariants.rs is engine-fix-level) |
| 6 | Update `capco_rule_set_registers_all_rules` | ✅ Complete | Count 56 → 57; added explicit S004 + S005 `contains` checks |
| 7 | Update `rule_count_reflects_registration_changes` | ✅ Complete | Count 56 → 57 in `corpus_parity.rs` integration test |

## Validation Results

| Level | Status | Notes |
|---|--------|-------|
| Static Analysis | ✅ Pass | `cargo clippy --workspace --all-targets -- -D warnings` clean |
| Build (workspace) | ✅ Pass | `cargo build --workspace` clean |
| Build (WASM) | ✅ Pass | `cargo build -p marque-wasm --target wasm32-unknown-unknown` clean (Constitution Principle III preserved — `PageContext::portions()` is `marque-ism` and ships on the WASM-safe path) |
| Unit Tests | ✅ Pass | 409 unit tests pass (17 new S005 tests); full workspace: 0 failures |
| Format | ✅ Pass | `cargo fmt --check` clean |
| Integration (REL TO invariants) | ✅ Pass | 8 existing E002/E020/E052 overlap tests still pass; S005 doesn't introduce new C-1 overlap interactions because it never emits a fix |

## Files Changed

| File | Action | Notes |
|---|--------|-------|
| `crates/ism/src/page_context.rs` | UPDATED | +22 lines — added `pub fn portions(&self) -> &[IsmAttributes]` with doc comment explaining why most rules should prefer `expected_*` |
| `crates/capco/src/rules.rs` | UPDATED | +681 / -4 lines — `RelToOpaqueUncertainReductionRule`, three module-private helpers (`s005_state_text`, `s005_expand_atomic`, `s005_render_set`), registration in `CapcoRuleSet::new()`, manifest comment for S005, 17 unit tests, updated `capco_rule_set_registers_all_rules` count to 57 |
| `crates/capco/tests/corpus_parity.rs` | UPDATED | +14 / -3 lines — bumped `rule_count_reflects_registration_changes` to 57 with S005 changelog entry |

## Deviations from Plan

1. **Test fixtures use NA-deprecated tetragraphs (RSMA, EUDA, BHTF), not the plan's `MNFI` org-fork example.** Reason: `country_extensions.toml` ships empty by default, so a fixture using `MNFI` would require populating extensions just for the test, polluting build-time data with test-only entries. NA-deprecated codes are in the CVE recognition surface (so the parser keeps them in `attrs.rel_to`) AND `is_decomposable` returns `None` for them — identical runtime behavior from S005's POV. Only the `{state}` text in the diagnostic differs, and that's covered by the four `s005_state_text_for_*` tests. Documented in a header comment on the S005 test block.

2. **Tests landed in `crates/capco/src/rules.rs::tests`, not `crates/capco/tests/rel_to_invariants.rs`.** Plan §3.4 lists `rel_to_invariants.rs`, but that file specifically tests engine-level fix-application invariants (FR-016 sort, C-1 overlap guard) using `Engine::fix`. S005 emits no fix, so those interactions don't apply. The closest existing pattern is the E039/E040 inline tests in `rules.rs::tests` (banner+portion fixtures driven through the test-support `lint_banner` helper) — same shape as S005. Putting S005 tests there keeps similar tests adjacent.

3. **Added a trigraph filter (`s.len() == 3 → continue`) inside the rule that the plan §3.2 pseudocode doesn't mention.** Reason: `is_decomposable` is a *tetragraph* discriminator, not a country-code discriminator. ISO 3166-1 alpha-3 trigraphs (USA, GBR, AUS, …) aren't in the ISMCAT taxonomy at all, so `is_decomposable("USA") == None` for the same reason `is_decomposable("XYZW") == None`. Without the filter the rule would incorrectly fire on every multi-portion REL TO with non-overlapping trigraphs. Documented in an extended inline comment with the empirical CVE-recognition-surface length distribution (1 length-2 + 280 length-3 + 58 length-4 + 1 length-15) as evidence the filter selects exactly the right population.

4. **Three small helpers split out from the rule body** (`s005_state_text`, `s005_expand_atomic`, `s005_render_set`) instead of inline logic. Reason: each is independently testable, and `s005_state_text` exercises four `(decomposable, membership_shape)` arm shapes that would be brittle to drive through full `IsmAttributes` fixtures. Mirrors the S004 pattern (`s004_message`, `s004_edit_distance` extracted for the same reason).

5. **`Diagnostic` message wording differs from plan §3.3's example.** Plan example included `If {X} includes {GBR}, the banner should be: {classification}//REL TO USA, GBR / If {X} excludes {GBR}, the banner is: {classification}//{NOFORN or atom-result}`. The implemented message keeps the substantive content (uncertain-code name, state, atom-intersection result, other-codes, resolution paths) but drops the speculative "if-then" example because (a) the operator may not act on this diagnostic at the banner-line level — they may add `MNFI` to extensions instead; (b) the message length was already getting unwieldy. Plan said "the issue's example message is the right shape" — shape, not literal text — so this stays inside the latitude.

## Issues Encountered

1. **Trigraphs would have triggered the rule incorrectly.** `is_decomposable` returns `None` for any code not in the V2022-NOV tetragraph taxonomy, including 3-letter ISO trigraphs. Without a length filter, S005 would fire on every multi-portion REL TO where a trigraph appears in some portions but not others (e.g., `(S//REL TO USA, GBR)` + `(S//REL TO USA, AUS)` would flag both GBR and AUS as "uncertain"). Resolved with `s.len() == 3 → skip` per deviation #3 above. Caught during test design when the "no fire — pure trigraphs" case initially failed.

2. **Initial test fixture for the Suggest-inconsistent-banner case actually produced Info.** First-draft fixture had banner = `REL TO USA, FRA` against atom-intersection {USA}. `{USA} ⊆ {USA, FRA}` is true, so the consistency check picked Info. Realized I needed atom-intersection to have ≥2 codes so the banner could *miss* one — switched portion 2 from `(S//REL TO USA, AUS)` to `(S//REL TO USA, AUS, GBR)` so atom-intersection becomes `{USA, GBR}`, banner `REL TO USA` then misses GBR ⇒ Suggest. Plan §3.5 case 3 wording was vague enough that this took test-design iteration to nail down; updated the test comments to make the math explicit.

3. **Two existing rule-count tests drifted past 56 → 57.** `capco_rule_set_registers_all_rules` (unit) and `rule_count_reflects_registration_changes` (integration). Both updated together with a S005 entry in the changelog comment so the next rule addition has the same prompt to bump.

## Tests Written

| Test File | Tests | Coverage |
|---|------:|----------|
| `crates/capco/src/rules.rs::tests` | +17 | S005 firing: Suggest (active-validation, banner drops a preserved code); Info (banner equals atom-intersection, banner is proper superset). S005 no-fire: uncertain in every portion, KFOR atom-by-authority, EU atom-by-authority, FVEY decomposable-known, single REL TO portion, pure-trigraph fixtures, empty other_codes set. State text: NA-Suppressed (RSMA), NA-Description (EUDA, with verbatim quote), NA-Members(recursive) (BHTF), absent (XYZW). Constitution V audit-content-ignorance: surrounding document text doesn't leak into the message. |
| `crates/capco/src/rules.rs::tests` | 1 modified | `capco_rule_set_registers_all_rules` count 56 → 57 with explicit `S004` and `S005` containment checks |
| `crates/capco/tests/corpus_parity.rs` | 1 modified | `rule_count_reflects_registration_changes` count 56 → 57 with S005 changelog entry |

## Generated-Output Spot Check

Sample diagnostic for the canonical fixture
`(S//REL TO USA, GBR, RSMA)\n(S//REL TO USA, AUS, GBR)\nSECRET//NOFORN`:

```
S005 [Suggest]
Span: <banner-region>
Message: REL TO code `RSMA` has uncertain membership (deprecated,
membership suppressed (NA-Suppressed in V2022-NOV)). Atom-semantics
intersection produced REL TO USA, GBR, but `RSMA`'s hypothetical
membership may include AUS from other portions. Resolution: (a) add
`RSMA` membership to country_extensions.toml with an authoritative
source citation, or (b) revise the marking to use codes with known
membership.
Citation: CAPCO-2016 §H.8 + ODNI ISMCAT V2022-NOV Tetragraph Taxonomy
Fix: None
```

EUDA fixture additionally surfaces the verbatim ODNI Description text:
`"As of 15 March 2016, disclosure request should be referred to the
original classification authority for a disclosure decision."`

## Next Steps

- [ ] **Code review** via PR description / `/code-review` then push branch `feat/206-s005-uncertain-reduction` and open the PR against `main`.
- [ ] **Future**: deprecated-tetragraph remarking aid (W### — "deprecated tetragraph in active marking"). Plan §2.3 / §2.10 mentions this as a follow-on; all 18 NA codes carry a `deprecated="YYYY-MM-DD"` date that's already in `TETRAGRAPH_PROVENANCE`. Out of scope here.
- [ ] **Future**: org-fork extension `kind = "membership-shorthand" | "organization-atom"` discriminator (plan §2.3 footnote). Allows operators to opt in to `is_decomposable("MNFI") == Some(true)` when they have verifiable membership data outside ODNI's taxonomy. Out of scope here.
- [ ] **Future**: per-document temporal resolution (`as_of` on `ParseContext`). Plan §2.10 — versioning the taxonomy by marking date.
