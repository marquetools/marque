<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Implementation Report: Issue #208 (PR-1) — ISMCAT Tetragraph Taxonomy build-time parsing

## Summary

PR-1 of the merged plan `docs/plans/2026-04-28-tetragraph-taxonomy-and-uncertain-reduction.md` is implemented. `marque-ism/build.rs` now parses the vendored ODNI ISMCAT V2022-NOV `TetragraphTaxonomyDenormalized.xml` and emits three artifacts into `OUT_DIR/values.rs`:

- `is_decomposable(code: &str) -> Option<bool>` — three-state ODNI-authoritative discriminator (24 `Yes`, 19 `No`, 18 `NA` collapsed to `None`).
- `TETRAGRAPH_MEMBERS` / `lookup_tetragraph_members` — sourced from the 24 `decomposable="Yes"` taxonomy entries with materialized non-recursive `<Country>` lists, plus any `members`-bearing rows from `country_extensions.toml`.
- `TETRAGRAPH_PROVENANCE` (`#[doc(hidden)]`) — full audit row preserving the three-state `decomposable` flag, membership shape, deprecation date, `dateLastVerified`, and verbatim `<Description>` text for the three NA-Description entries.

The hand-curated `BUILTIN_TETRAGRAPH_MEMBERS` slice and the empty `pub const NATO: &[&str] = &[]` are retired. Schema version is pinned via `[package.metadata.marque] ismcat-tetra-version = "2022-NOV"` and verified at build time. Four `cargo:warning=` data-quality guards catch future taxonomy revisions that violate the V2022-NOV invariants the runtime API depends on.

## Assessment vs Reality

| Metric | Predicted (Plan) | Actual |
|---|---|---|
| Sequencing | Two PRs, #208 first | PR-1 (#208) implemented standalone; PR-2 (#206) deferred per plan §6 |
| Files changed | 7 (per plan §2.1 + tests) | 7 (matches) |
| Trichotomy state | 3-state | 3-state (24 Yes / 19 No / 18 NA) — exact V2022-NOV match |
| Provenance entries | 61 | 61 (XML count verified) |

## Tasks Completed

| # | Task | Status | Notes |
|---|------|--------|-------|
| 1 | Inventory existing build.rs structure | ✅ Complete | |
| 2 | Cargo.toml metadata + include allowlist | ✅ Complete | `ismcat-tetra-version = "2022-NOV"`; `Taxonomy/ISMCAT/*.xml` glob added |
| 3 | `parse_tetragraph_taxonomy` in build.rs | ✅ Complete | Uses `quick-xml` (existing build-dep); `TaxEntry`/`TaxDecomposable`/`TaxMembership` types mirror XSD |
| 4 | `verify_ismcat_tetra_version` + 4 guards | ✅ Complete | Yes-non-Members, NA-without-deprecated, No-non-Suppressed, non-NA-recursive |
| 5 | `emit_tetragraph_members` rewrite | ✅ Complete | Sources from taxonomy + extensions; shadowing impossible (existing dup guard) |
| 6 | `emit_is_decomposable` + `emit_tax_provenance` | ✅ Complete | `Description` text folded into provenance for plan §3.3 quotability |
| 7 | Wire `generate_values` + `main()` | ✅ Complete | `verify_ismcat_tetra_version()` in main; emits in `generate_values` after taxonomy parse |
| 8 | lib.rs re-exports | ✅ Complete | `ISMCAT_TETRA_VERSION`, `is_decomposable` added |
| 9 | capco/vocab `is_decomposable_tetragraph` delegate | ✅ Complete | Plus replaced obsolete NATO opacity test |
| 10 | Trichotomy + round-trip tests | ✅ Complete | 7 new tests covering all 4 plan §2.8 branches + §D Table 3 rule 23 |
| 11 | Shadowing policy + validation | ✅ Complete | Existing `load_country_extensions` duplicate guard already covers shadowing (all 61 ISMCAT codes are in `CVEnumISMCATRelTo.xsd`); no `override` opt-in needed |

## Validation Results

| Level | Status | Notes |
|---|--------|-------|
| Static Analysis | ✅ Pass | `cargo clippy --workspace --all-targets -- -D warnings` clean |
| Build (workspace) | ✅ Pass | `cargo build --workspace` clean |
| Build (WASM) | ✅ Pass | `cargo build -p marque-wasm --target wasm32-unknown-unknown` clean (Constitution Principle III preserved) |
| Unit Tests | ✅ Pass | All workspace tests pass; 0 failures |
| Format | ✅ Pass | `cargo fmt --check` clean |

## Files Changed

| File | Action | Notes |
|---|--------|-------|
| `crates/ism/Cargo.toml` | UPDATED | Added `ismcat-tetra-version = "2022-NOV"`; added `Taxonomy/ISMCAT/*.xml` to `include` |
| `crates/ism/build.rs` | UPDATED | +~600 lines — parser, emitters, version verifier, 4 guards |
| `crates/ism/src/lib.rs` | UPDATED | Re-export `ISMCAT_TETRA_VERSION`, `is_decomposable` |
| `crates/ism/src/page_context.rs` | UPDATED | Replaced 3 NATO-as-opaque tests with KFOR-based equivalents (NATO is now decomposable=Yes per #208) |
| `crates/ism/tests/tetragraph_membership.rs` | UPDATED | Updated baseline count (2→24), FVEY order (alphabetical→taxonomy), added No / NA categories |
| `crates/capco/src/vocab.rs` | UPDATED | Added `is_decomposable_tetragraph` delegate; removed obsolete `pub const NATO: &[&str] = &[]`; updated FVEY order to taxonomy publication order; replaced NATO-opacity test |
| `crates/capco/tests/tetragraph_consolidation.rs` | UPDATED | Updated NATO from negative to positive list; added 7 trichotomy tests + §D Table 3 rule 23 round-trip |

## Deviations from Plan

1. **`pub const NATO: &[&str] = &[]` removed entirely** rather than updated. The constant had zero consumers (verified via grep) and an empty value that was now misleading (NATO is `decomposable=Yes` with 30 trigraph members in V2022-NOV). Cleaner removal than misleading retention.

2. **`Description` text folded into `TetragraphProvenance.description`**. Plan §3.3 says "the diagnostic should quote the taxonomy's `<Description>` text verbatim"; emitting it through the provenance table now (rather than rediscovering it in PR-2) closes the dead-code warning the build script hit on the `TaxMembership::Description(String)` variant and makes PR-2's S005 emitter trivial.

3. **`TaxMembership::Members.organizations` retained with `#[allow(dead_code)]`** rather than dropped. Reason in the field comment: kept for future organization-aware diagnostics; `recursive: bool` already encodes the only fact PR-1 reads but the names may matter later.

4. **FVEY publication order is `AUS, CAN, NZL, GBR, USA`** (taxonomy verbatim), not alphabetical. Plan §2.8 didn't specify which order to assert; the existing `crates/ism/tests/tetragraph_membership.rs::fvey_canonical_membership` test was hardcoded alphabetical. Updated to match ODNI's publication order per Constitution Principle VIII (authoritative source fidelity), with a separate `fvey_set_membership` test that compares as set so a future ODNI reorder doesn't trip the test even if the ordered assertion does.

5. **Shadowing policy resolved without code change** (plan §8 Q2). Investigation showed all 61 ISMCAT taxonomy codes appear in `CVEnumISMCATRelTo.xsd`, and `load_country_extensions` already rejects any extension whose `code` duplicates a CVE entry. Therefore extensions can never shadow taxonomy codes; the question is moot. No `override = true` opt-in implemented.

## Issues Encountered

1. **Stale rust-analyzer diagnostics on initial build** — rust-analyzer flagged a `description` field as missing on existing `TetragraphProvenance` initializers in `target/debug/build/.../values.rs`. Resolved by `cargo build` regenerating the file. Not a real issue, just the IDE caching the previous schema.

2. **Three NATO-as-opaque tests broke** in `crates/ism/src/page_context.rs` and one in `crates/ism/tests/tetragraph_membership.rs`. Expected — the whole point of #208 is that NATO is no longer opaque. Replaced with KFOR (decomposable="No" — atom by authority) for the equivalent intersection semantics, and updated the membership count from 2 to 24 with rationale.

3. **Two test ordering mismatches** with the new taxonomy-sourced FVEY (`AUS, CAN, NZL, GBR, USA` vs alphabetical `AUS, CAN, GBR, NZL, USA`). Updated tests to assert taxonomy order with set-equivalence cross-checks where the order isn't load-bearing.

4. **Three clippy lints fired**: doc-list overindentation in two long doc comments (`build.rs` and the generated `is_decomposable` doc string), one collapsible-match in the parser's `Empty` branch, and one `iter_cloned_collect` in the FVEY-vs-ACGU set test. Fixed all four.

## Tests Written

| Test File | Tests | Coverage |
|---|------:|----------|
| `crates/capco/tests/tetragraph_consolidation.rs` | +7 | Trichotomy: Yes-with-members, No-atom, NA-Suppressed, NA-Description, NA-recursive (BHTF), unknown-or-extension. Plus §D Table 3 rule 23 round-trip. |
| `crates/capco/src/vocab.rs` (unit) | +5 | NATO-decomposable, EU-no, FVEY-yes, deprecated-none, unknown-none. Plus a set-equivalence `fvey_set_membership` test. |
| `crates/ism/tests/tetragraph_membership.rs` | +1 (renamed/restructured) | NATO has taxonomy membership (replacing `nato_is_opaque_not_in_table`); split former opacity test into NA-deprecated and No-atom-by-authority categories. |
| `crates/ism/src/page_context.rs` (unit) | 3 modified | NATO→KFOR fixture swap in opaque-tetragraph intersection tests, plus updated `expand_tetragraph_returns_none_for_opaque_and_unknown`. |

## Generated-Output Spot Check

```
$ grep -E '^\s+"[A-Z_]+" => Some\(true\)'  values.rs | wc -l   # 24
$ grep -E '^\s+"[A-Z_]+" => Some\(false\)' values.rs | wc -l   # 19
$ grep -c 'TetragraphProvenance { code:' values.rs              # 61
```

Matches plan §1.2 empirical V2022-NOV distribution exactly. The three NA-Description entries (EUDA, MPFL, PGMF) carry verbatim ODNI deferral text in `TETRAGRAPH_PROVENANCE.description` for PR-2's S005 emitter.

## Next Steps

- [ ] **PR-2 (#206)**: implement `RelToOpaqueUncertainReductionRule` (S005) per plan §3. Banner-aware severity selection (Info vs Suggest), `is_decomposable_tetragraph` discriminator, `{state}` text from `TETRAGRAPH_PROVENANCE`. Open question §8 Q3 still pending: confirm banner-consistency primitive against rollup XSL behavior before PR-2 merges.
- [ ] Code review via `/code-review` (or PR-creation flow).
