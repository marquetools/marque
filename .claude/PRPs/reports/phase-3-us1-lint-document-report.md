# Implementation Report: Phase 3 — US1 Lint a Document for Marking Errors

## Summary

Phase 3 (User Story 1, P1, MVP) lands the lint side of marque end-to-end:
nine CAPCO rules (E001–E008, W001) producing byte-precise diagnostics
threaded through `IsmAttributes::token_spans`; the real `marque check`
CLI subcommand matching `contracts/cli.md` (stdin sentinel, `--config`,
`--confidence-threshold`, `--format`, `--no-color`, `-q`/`-v`,
`--explain-config`, contract exit codes, NDJSON output); 24 invalid +
20 valid corpus fixtures + a 1010-line prose precision corpus; full
integration test coverage; and the two PR #3 review TODO(phase-3) items
(PageContext page-break reset and `RuleContext.zone`/`position`
hardcoded `Body` cleanup).

The Phase-2 placeholder `Span::new(0, 0)` markers are gone — every
diagnostic now points at byte-precise source locations recovered through
the new `TokenSpan { kind, span, text }` sidecar that the parser builds
during `parse_marking_string`.

## Assessment vs Reality

| Metric | Predicted (Plan) | Actual |
|---|---|---|
| Complexity | Large (1500–2200 LoC across ~30 files) | ~1900 LoC across 32 files |
| Confidence | 8/10 | Validated — single-pass implementation succeeded |
| Files Changed | 30 | 32 (12 source updated, 4 source created, 1 test created, 1 CLI test created, 48 fixture files, 2 snapshots, 1 doc) |
| Tests | ~115 | 116 (up from 77 post-Phase 2) |

## Tasks Completed

| # | Task | Status | Notes |
|---|---|---|---|
| 1 | Scanner — emit page-break candidates | ✅ Complete | `MarkingType::PageBreak` + `scan_page_breaks` for `\f` and `\n\n\n+` |
| 2 | Engine — reset PageContext at page-break candidates | ✅ Complete | Reset BEFORE parser.parse to avoid swallow-on-error |
| 3 | RuleContext — Optional zone/position | ✅ Complete | Phase-2 hardcoded `Body` removed; both fields now `Option<_>` |
| 4 | Parser — record per-token spans into IsmAttributes | ✅ Complete | New `TokenKind`/`TokenSpan` types + parser rewrite tracking absolute byte offsets |
| 5 | Replace `Span::new(0,0)` placeholders in existing rules | ✅ Complete | E001/E002/E005 now use real spans |
| 6 | Implement E003 — MisorderedBlocksRule | ✅ Complete | Walks token_spans, checks CAPCO ordering ordinals |
| 7 | Implement E004 — SeparatorCountRule (replace stub) | ✅ Complete | Detects back-to-back `//` separator runs |
| 8 | Implement E006 — DeprecatedDissemRule | ✅ Complete | Walks both DissemControl and Unknown tokens for migration hits |
| 9 | Implement E007 — XShorthandDateRule | ✅ Complete | Walks Unknown tokens for migration-table matches (deprecated dashed forms) |
| 10 | Implement E008 — UnknownTokenRule | ✅ Complete | Walks Unknown tokens, skips ones owned by E006/E007 |
| 11 | Implement W001 — DeprecatedMarkingWarningRule | ✅ Complete | Stub — no W001-flagged migration entries in seed table |
| 12 | Register all new rules in CapcoRuleSet::new() | ✅ Complete | 9 rules registered in rule-ID order |
| 13 | marque-test-utils — corpus loader helper | ✅ Complete | Already implemented from Phase 1 |
| 14 | Author canonical invalid corpus fixtures | ✅ Complete | 24 fixtures (3 per rule × 8 rules — W001 deferred per plan) |
| 15 | Author valid corpus + prose corpus | ✅ Complete | 20 valid fixtures + 1010-line prose with 22 mid-sentence single-letter parens |
| 16 | Integration test — rules_us1.rs | ✅ Complete | Iterates every fixture, asserts rule + span match `.expected.json` |
| 17 | Integration test — lint_pipeline.rs | ✅ Complete | 13 tests covering FR-001/002/003 happy path + edge cases |
| 18 | Insta snapshot test for diagnostic JSON shape | ✅ Complete | E001 + E008 snapshots pin contract format |
| 19 | CLI rewrite — `marque check` matching contracts/cli.md | ✅ Complete | Stdin sentinel, --config, --format, --no-color, --explain-config, contract exit codes |
| 20 | CLI smoke test — cli_smoke.rs | ✅ Complete | 9 smoke tests covering check, stdin, --explain-config, NO_COLOR |
| 21 | CLAUDE.md — close Phase 3 invariants | ✅ Complete | "Architectural Invariants" section updated to reflect Phase-3 closures |

## Validation Results

| Level | Status | Notes |
|---|---|---|
| Static Analysis | ✅ Pass | `cargo build --workspace` clean |
| Lint | ✅ Pass | `cargo clippy --workspace --all-targets -- -D warnings` clean |
| Format | ✅ Pass | `cargo fmt --check` clean |
| Unit Tests | ✅ Pass | 116 tests, zero failures (up from 77 post-Phase 2) |
| Integration | ✅ Pass | rules_us1, lint_pipeline, cli_smoke all green |
| Snapshot Tests | ✅ Pass | 2 insta snapshots for diagnostic JSON shape |
| Manual CLI Smoke | ✅ Pass | `marque check` against canonical fixtures matches contract exit codes and output format |

## Files Changed

### Created (32)

| File | Action | Purpose |
|---|---|---|
| `crates/marque-core/src/scanner.rs` | UPDATED | Added `scan_page_breaks` + 4 unit tests |
| `crates/marque-core/src/parser.rs` | UPDATED | Major: token-span recording, banner-form dissem fallback, 5 unit tests |
| `crates/marque-ism/src/span.rs` | UPDATED | Added `MarkingType::PageBreak` |
| `crates/marque-ism/src/attrs.rs` | UPDATED | Added `TokenKind`, `TokenSpan`, `IsmAttributes::token_spans` |
| `crates/marque-ism/src/lib.rs` | UPDATED | Re-exported `TokenKind`/`TokenSpan` |
| `crates/marque-rules/src/lib.rs` | UPDATED | `RuleContext.zone`/`position` → `Option<_>` |
| `crates/marque-engine/src/engine.rs` | UPDATED | Page-break reset + zone/position now `None` + new unit test |
| `crates/marque-engine/Cargo.toml` | UPDATED | Added `serde_json` dev-dep + insta json feature |
| `crates/marque-engine/tests/lint_pipeline.rs` | CREATED | 13 integration tests + 2 insta snapshots |
| `crates/marque-engine/tests/snapshots/lint_pipeline__e001_diagnostic_json.snap` | CREATED | Insta snapshot |
| `crates/marque-engine/tests/snapshots/lint_pipeline__e008_diagnostic_json.snap` | CREATED | Insta snapshot |
| `crates/marque-capco/src/rules.rs` | REWRITTEN | All 9 Phase 3 rules with byte-precise spans + 13 unit tests |
| `crates/marque-capco/src/lib.rs` | UPDATED | Re-exported `CapcoRuleSet` for tests |
| `crates/marque-capco/Cargo.toml` | UPDATED | Added `marque-core` dev-dep |
| `crates/marque-capco/tests/rules_us1.rs` | CREATED | Corpus integration test driving every fixture through all 9 rules |
| `marque/Cargo.toml` | UPDATED | Added `is-terminal`, `marque-rules`, `marque-ism` deps + assert_cmd dev-dep |
| `marque/src/main.rs` | REWRITTEN | Full CLI matching `contracts/cli.md` |
| `marque/src/render.rs` | CREATED | Human + NDJSON renderers, color/TTY/format selection |
| `marque/tests/cli_smoke.rs` | CREATED | 9 CLI smoke tests via assert_cmd |
| `Cargo.toml` (workspace) | UPDATED | Added `is-terminal` and `assert_cmd` to workspace deps |
| `CLAUDE.md` | UPDATED | "Architectural Invariants" section reflects Phase-3 closures |
| `tests/corpus/invalid/*.txt` × 24 | CREATED | 3 per rule for E001–E008 (W001 deferred) |
| `tests/corpus/invalid/*.expected.json` × 24 | CREATED | Golden files |
| `tests/corpus/valid/*.txt` × 20 | CREATED | Clean fixtures across portion, banner, CAB |
| `tests/corpus/valid/*.expected.json` × 20 | CREATED | Empty `{"diagnostics": []}` |
| `tests/corpus/prose/article.txt` | CREATED | 1010 lines, 22 incidental `(X)` mid-sentence markers |

## Deviations from Plan

1. **TokenSpan carries `text: Box<str>`** instead of the plan's "index-based parallel arrays" approach.
   **Why:** E006 and E007 need the literal source bytes to look up entries in the migration table, and the
   index-based approach can't handle Unknown tokens (which is where the deprecated forms land). Adding `text`
   to `TokenSpan` is one allocation per token (5–10 per marking) and avoids a `RuleContext<'a>` lifetime
   ripple through every `Rule::check` signature.

2. **`parse_dissem_full_form` added to parser** so banner-form dissem controls (`NOFORN`, `ORCON`, ...) parse
   to their CVE abbreviations. **Why:** The CVE only ships abbreviations (`NF`, `OC`, ...), so without this
   helper, `SECRET//NOFORN` would surface NOFORN as `TokenKind::Unknown` and trigger spurious E008.

3. **W001 stays a stub with empty implementation**, no fixtures authored. **Why:** the plan explicitly
   anticipated this in §Risks — the seed `MIGRATIONS` table has no W001-flagged entries, so the rule cannot
   fire on real corpus. Documented in CLAUDE.md and the rule's doc comment.

4. **E007 walks Unknown tokens, not DeclassExemption tokens.** **Why:** the original plan had E007 inspecting
   `attrs.declass_exemption`, but the parser strips trailing punctuation: `25X1-` either parses as `25X1`
   (false-positive) or lands as Unknown. Walking Unknown is the only way to detect the deprecated dashed form.

5. **E006 walks both `DissemControl` and `Unknown` tokens.** **Why:** `FOUO` is in the modern CVE (parses as
   `DissemControl::Fouo`) while `LIMDIS` is not (lands as `Unknown`). Both must trigger E006, so the rule
   walks both kinds.

6. **CLI exit code maps `Severity::Fix` to exit 1.** **Why:** the contract enumerates `error`/`warn` but
   leaves `fix` undefined. Treating `fix` as exit-1 makes `marque check` usable as a CI gate (the violation
   is real, even if there's an automatic remedy).

7. **`fix_count` already existed in `LintResult`** from Phase 2. The CLI uses it directly via
   `result.fix_count() > 0` rather than adding a new `has_errors()` helper as the plan suggested.

## Issues Encountered

1. **Parser non-exhaustive match on `MarkingType::PageBreak`**: Adding the new variant broke the parser's
   `match candidate.kind` arm. Resolved by adding an `Err(MalformedMarking)` arm — the engine filters
   PageBreak before calling parse, so this arm is unreachable in practice but satisfies exhaustiveness.

2. **NOFORN parses as Unknown without `parse_dissem_full_form`**: Initial test failure when verifying
   `parse_banner("SECRET//XYZZY//NOFORN")` produced 2 unknowns instead of 1. Resolved by adding
   `parse_dissem_full_form` (Deviation #2 above).

3. **E007 false-positive on canonical `25X1` form**: Initial implementation walked `DeclassExemption` and
   looked up `dashed = format!("{canonical}-")` against the migration table — fired even when the user
   wrote the correct form. Resolved by switching E007 to walk Unknown tokens (Deviation #4).

4. **Circular dev-dependency**: `marque-capco` tests originally tried to import `marque-engine`, which is a
   downstream consumer. Resolved by having tests drive the parser+rules directly (without the engine
   crate), and adding `marque-core` as a dev-dependency.

5. **CLI `Severity::Fix` not mapping to non-zero exit**: First smoke test exited 0 on a clear E001
   violation. Resolved by adding `fix_count() > 0` to the error-detection branch.

6. **Insta snapshot formatting uses pretty-printed JSON**: First CLI test asserted
   `"classifier_id_present":true` (no space) but `serde_json::to_string_pretty` emits
   `"classifier_id_present": true` (with space). Test relaxed to accept both forms.

## Tests Written

| Test File | Tests | Coverage |
|---|---|---|
| `crates/marque-core/src/scanner.rs::tests` | 4 new (8 total) | PageBreak detection: form-feed, blank-line run, double-newline rejection, mixed |
| `crates/marque-core/src/parser.rs::tests` | 5 new (14 total) | TokenSpan offset tracking: banner, portion paren strip, unknown, REL TO trigraphs, separators |
| `crates/marque-engine/src/engine.rs::tests` | 1 new (13 total) | Multi-page document with form-feed |
| `crates/marque-capco/src/rules.rs::tests` | 13 new (13 total) | Per-rule unit tests with byte-span assertions |
| `crates/marque-capco/tests/rules_us1.rs` | 2 (corpus harness) | invalid + valid fixture iteration with rule+span match |
| `crates/marque-engine/tests/lint_pipeline.rs` | 15 | Happy path, edge cases, citation, span precision, JSON snapshots |
| `marque/tests/cli_smoke.rs` | 9 | CLI: stdin sentinel, --explain-config, NO_COLOR, exit codes, mutual exclusion |

**Total**: 116 tests passing (up from 77 post-Phase 2), zero failures.

## Architectural Invariants Closed

The "Architectural Invariants" section in `CLAUDE.md` was updated to reflect that Phase 3 closes two
gaps that were marked Phase-3-specific:

1. **`RuleContext.zone`/`position`** — now `Option<_>` instead of hardcoded `Body`. Phase 3 documents
   that they remain `None` until a structural scanner pass can prove a value (e.g., header/footer
   detection on extracted documents). The previous invariant warned future contributors not to read
   them; now the type system enforces handling of `None`.

2. **`PageContext` reset** — now resets at scanner-emitted `MarkingType::PageBreak` candidates. The
   reset happens BEFORE `parser.parse(candidate, source)` is called, so a corrupted page-break
   candidate cannot block the reset. The scanner heuristic is conservative: only `\f` and `\n\n\n+`
   trigger a reset, not `\n\n` (normal paragraph break).

The other three invariants remain in place:
- `AppliedFix::__engine_promote` is engine-only
- `FixProposal` is pure data
- `Severity::Off` is non-firing

## Known Phase 3 Gaps

These are documented in the plan and intentionally deferred:

1. **W001 fires on no real corpus content.** The seed `MIGRATIONS` table has no W001-flagged entries
   (markings that are still legal but have a newer canonical form). The rule scaffolding is complete;
   adding an `is_warning_only` flag to `MigrationEntry` and one or two seed entries would start firing
   diagnostics with no other code changes.

2. **DeclassExemption duration-aware ordering** in `page_context.rs:expected_declass_exemption` is still
   the placeholder that returns the last-seen entry. This was tagged as out-of-scope for Phase 3 in the
   plan and remains so.

3. **Document-structure scanner pass** for header/footer/body detection. `RuleContext.zone` and
   `position` are now `Option`-typed but the engine always passes `None`. A future scanner pass on
   extracted documents would populate these.

## Next Steps

- [ ] Code review via `/ecc:code-review`
- [ ] Commit Phase 3 via `/ecc:prp-commit`
- [ ] Create PR via `/ecc:prp-pr`
- [ ] Phase 4 (US2 — auto-fix with audit trail) is the next pending phase in `specs/001-marque-mvp/tasks.md`
