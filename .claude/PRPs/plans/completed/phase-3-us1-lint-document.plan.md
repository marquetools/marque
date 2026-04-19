<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Plan: Phase 3 — US1 Lint a Document for Marking Errors

## Summary

Phase 3 (User Story 1, P1, MVP) lands the lint side of marque end-to-end:
ten CAPCO rules (E001–E008, W001) producing byte-precise diagnostics, the
real `marque check` CLI subcommand with stdin/file input and human/JSON
output, the `--explain-config` diagnostic surface, and the corpus +
pipeline tests that pin every rule against `.expected.json` golden files.
The hardcoded Phase-2 placeholder spans (`Span::new(0, 0)`) are replaced
with real spans threaded through `RuleContext` from the parser, and the
PageContext page-break reset is wired so banner/CAB rules are correct on
multi-page documents — both items the PR #3 review flagged as Phase-3
work.

## User Story

As a **classification reviewer / CSO at a defense contractor**,
I want to run `marque check <file>` (or `cat file | marque check -`) and
receive precise diagnostics — rule ID, byte span, human message, CAPCO
citation, optional fix suggestion — for every marking violation in the
document,
so that I can hand a clean, citation-backed punch list back to the
classifying author before the document leaves my queue.

## Problem → Solution

**Current state**: Phase 2 wired the engine, parser, and codegen, but the
CAPCO rules in `marque-capco/src/rules.rs` emit `Span::new(0, 0)`
placeholders, only four of the ten target rules exist (E001/E002/E004/
E005, with E004 a stub that returns `vec![]`), and the CLI is a
hand-written `clap` skeleton with no `--config`, no `--format json` per
contract, no `--explain-config`, no stdin support, and exit codes that
don't match `contracts/cli.md`. The corpus directories (`tests/corpus/
{invalid,valid,prose}/`) exist but contain zero fixtures, and no
integration test drives the rule set against fixtures.

**Desired state**: ten rules emit byte-precise diagnostics rooted in the
real source span; `marque check` matches the contract verbatim (stdin,
file, mixed input; `--config`, `--confidence-threshold`, `--format
human|json`, `--no-color`, `--explain-config`, `-q`, `-v`); ≥30
known-bad fixtures + ≥20 known-good fixtures + the prose precision
corpus exist with `.expected.json` siblings; integration tests fail loud
on any rule-ID or span drift; FR-016 reverse-byte ordering is exercised
end-to-end; `RuleContext.page_context` is reset at scanner-emitted page
breaks so banner/CAB rules are correct on multi-page input; and the
US1 acceptance test from `spec.md` passes byte-for-byte.

## Metadata

- **Complexity**: **Large** — 10 rules + CLI rewrite + ~50 fixtures + 4
  integration test files + scanner span plumbing + page-break wiring.
  Estimated 25–35 source files touched, 1500–2200 LoC.
- **Source PRD**: `specs/001-marque-mvp/tasks.md`
- **PRD Phase**: Phase 3 — User Story 1 (P1, MVP) — tasks T025–T042a
- **Estimated Files**: 30 files (~12 created, ~18 modified)

---

## UX Design

### Before

```
$ marque check banner.txt
banner.txt:0:0 [E001] banner uses abbreviated dissem control "NF"; use "NOFORN"
  → fix (confidence 100%): "NOFORN"
banner.txt:0:0 [E002] REL TO list missing required USA trigraph
  → fix (confidence 97%): "USA, GBR, AUS"
```

Spans always print as `0:0`. JSON format works but doesn't conform to
`contracts/diagnostic.json` (missing `severity`, `citation`, wrong
`fix` shape). Stdin (`-`) is unsupported. `--config`, `--explain-config`,
`--no-color`, `-q`, `-v` are missing. Exit codes don't distinguish
"warnings only" from "errors present".

### After

```
$ marque check banner.txt
banner.txt:1:1 error[E001] banner uses abbreviated dissem control "NF"; use "NOFORN"
  --> banner.txt:1:18-20
   |
 1 | TOP SECRET//SI//NF
   |                  ^^ replace with NOFORN  (CAPCO-2016 §A.6)
   |                  ^^ replace with NOFORN  (CAPCO-2016 §A.6)
   |

banner.txt:1:1 error[E002] REL TO list missing required USA trigraph
  --> banner.txt:1:24-32
   |
 1 | TOP SECRET//REL TO GBR, AUS
   |                    ^^^^^^^^ insert USA at start  (CAPCO-2016 §A.6)
   |                    ^^^^^^^^ insert USA at start  (CAPCO-2016 §A.6)
   |

2 diagnostics (2 errors, 0 warnings)
```

```
$ cat banner.txt | marque check - --format json
{"rule":"E001","severity":"error","span":{"start":17,"end":19},"message":"banner uses abbreviated dissem control \"NF\"; use \"NOFORN\"","citation":"CAPCO-2016 §A.6","fix":{"source":"BuiltinRule","replacement":"NOFORN","confidence":1.0,"migration_ref":"CAPCO-2016 §A.6"}}
{"rule":"E002","severity":"error","span":{"start":23,"end":31},"message":"REL TO list missing required USA trigraph","citation":"CAPCO-2016 §A.6","fix":{"source":"BuiltinRule","replacement":"USA, GBR, AUS","confidence":0.97,"migration_ref":"CAPCO-2016 §A.6"}}
```

```
$ marque check --explain-config | jq .
{
  "rules": {
    "E001": "fix",
    "E002": "fix",
    "E003": "warn",
    ...
  },
  "corrections": ["SERCET", "TOPSECRET"],
  "confidence_threshold": 0.95,
  "schema_version": "ISM-v2022-DEC",
  "classifier_id_present": true
}
```

### Interaction Changes

| Touchpoint | Before | After | Notes |
|---|---|---|---|
| `marque check` arguments | `[FILE...]` (required, no stdin) | `[PATH...]` with `-` sentinel; reads stdin when no path given | FR-014a |
| `--format` | `text\|json`, no contract conformance | `human\|json` (NDJSON, one record per line, conforming to `contracts/diagnostic.json`) | R-4 |
| `--config` | absent | accepts an explicit config-file path, short-circuits the upward walk | contracts/cli.md §Configuration discovery |
| `--explain-config` | absent | dumps merged Configuration as JSON, exits 0; mutually exclusive with paths and `fix` | contracts/cli.md §Common options |
| `--no-color` | absent | suppresses ANSI; honors `NO_COLOR` and `TERM=dumb` env vars | contracts/cli.md §Common options |
| `-q` / `-v` | absent | quiet / verbose narration toggles | contracts/cli.md §Common options |
| Diagnostic span | always `0:0` | byte-precise span into the original source | Scanner / parser span plumbing |
| Diagnostic format | `start:end [rule] message` | rustc-style with caret + citation in human mode; NDJSON in json mode | R-4 |
| Exit code | `0` clean / `1` else | `0` clean, `1` errors, `2` warnings only, `64` usage, `65` config, `74` IO | contracts/cli.md §Exit codes |
| PageContext lifecycle | accumulated for whole document | reset at scanner-emitted page-break candidates | PR-3 review TODO(phase-3) at engine.rs:80 |
| `RuleContext.zone` / `position` | hardcoded `Body` | populated from scanner-derived document position | PR-3 review TODO(phase-3) at engine.rs:103 |

---

## Mandatory Reading

Files that MUST be read in full before implementing this plan. Do not
search the codebase during implementation — every reference is captured
here.

| Priority | File | Lines | Why |
|---|---|---|---|
| **P0** | `crates/marque-capco/src/rules.rs` | all (282) | Existing rule shape — every new rule must mirror the four already there. The `Span::new(0,0)` placeholders mark the hot spots Phase 3 must replace. |
| **P0** | `crates/marque-engine/src/engine.rs` | 60–151 | The lint loop. Phase 3 plumbs real spans through `RuleContext` and resets `PageContext` at page breaks here. The two `TODO(phase-3)` comments at lines 80 and 103 are the entry points. |
| **P0** | `crates/marque-rules/src/lib.rs` | 125–143 | `RuleContext` definition — Phase 3 either grows this struct (e.g., `source: &[u8]`, `marking_span: Span`) or threads spans via the existing `marking_type` discriminant. |
| **P0** | `crates/marque-core/src/parser.rs` | all (348) | Parser. Phase 3 must extend `ParsedMarking` (or create a parallel `ParsedMarkingWithSpans` shape) so each parsed token carries its `Span` back into `IsmAttributes` rather than discarding offsets. The current `parse_marking_string` calls split + parse but never records inner spans. |
| **P0** | `crates/marque-core/src/scanner.rs` | all (153) | Scanner. Phase 3 adds a `MarkingType::PageBreak` variant (or equivalent) so the engine can reset `PageContext`. The candidate ordering already sorts by `span.start`. |
| **P0** | `marque/src/main.rs` | all (229) | The CLI to be replaced. Reuse the wiring (`marque_config::load`, `Engine::new`, `engine.lint(&source)`, `engine.fix_with_threshold`) but rebuild the `Cli` enum to match `contracts/cli.md`. |
| **P0** | `specs/001-marque-mvp/contracts/cli.md` | all (186) | The CLI contract. Every flag, every exit code, every output stream rule lives here. |
| **P0** | `specs/001-marque-mvp/contracts/diagnostic.json` | all (70) | The diagnostic JSON Schema. The NDJSON renderer must produce records that round-trip through this schema with `additionalProperties: false`. |
| **P1** | `crates/marque-engine/src/output.rs` | all (57) | `LintResult` / `FixResult`. `error_count`, `warn_count`, `fix_count` already exist and are what the CLI exit-code logic needs. |
| **P1** | `crates/marque-ism/src/page_context.rs` | all (333) | `PageContext` aggregation rules (max for classification, union for SCI/dissem, intersection-with-NOFORN-supersession for REL TO, max-date for declassify). Phase 3 calls `PageContext::new()` at every page break. |
| **P1** | `crates/marque-ism/src/attrs.rs` | 1–80 | `IsmAttributes` shape and `Classification` (hand-written, both portion and banner forms). |
| **P1** | `crates/marque-ism/src/span.rs` | all (140) | `Span::new` panics on inverted bounds; `as_str` returns `Result`. The new rules MUST construct spans with valid `start <= end` and not call `unwrap()` on `as_str` for non-ASCII paths. |
| **P1** | `crates/marque-config/src/lib.rs` | all (442) | `Config`, `Configuration` shape, `confidence_threshold`, `corrections` map, `user.classifier_id`. The `--explain-config` JSON dump reads from here. |
| **P1** | `tests/corpus/CORPUS_CONTRACT.md` | all (60) | Per-rule fixture minimums (≥3 invalid + ≥20 valid + 1000 lines prose). Phase 3 produces the fixtures, T069 in Phase 7 measures them. |
| **P2** | `crates/marque-ism/build.rs` | 650–720 | The `MIGRATIONS` table. Rules E006/E007/W001 consume this via `marque_ism::generated::values::find_migration`. |
| **P2** | `crates/marque-rules/src/lib.rs` | 145–225 | `FixSource`, `FixProposal`, `Diagnostic` shapes — what every new rule emits. `make_fix_diagnostic` in rules.rs:264 is the helper to mirror. |
| **P2** | `.claude/PRPs/reviews/pr-3-review.md` | all (164) | PR #3 review findings. Phase 3 inherits the H-2 documentation note (already landed in CLAUDE.md), the engine.rs:80 / engine.rs:103 TODO(phase-3) markers, and the page_context.rs Phase-3 TODOs. The H-1/H-3/M-/L- findings are already resolved in commit 304a15f. |
| **P2** | `CLAUDE.md` | "Architectural Invariants" section | Lists the convention-only invariants Phase 3 must keep intact (`AppliedFix::__engine_promote` engine-only, `FixProposal` purity, `Severity::Off` non-firing, `RuleContext.zone/position` Phase-2 hardcoded, `PageContext` no-reset). Phase 3 closes the last two of these. |

## External Documentation

| Topic | Source | Key Takeaway |
|---|---|---|
| ODNI ISM CAPCO Implementation Guidance | `crates/marque-ism/schemas/ISM-v2022-DEC/` (already in repo) | Authoritative source for E001 (banner abbreviation), E002 (REL TO USA placement), E003 (block ordering), E005 (declass in CAB only), E006/W001 (deprecated dissem). Already parsed by `build.rs` into `MIGRATIONS` and the generated enums. |
| `clap` derive API for stdin sentinel | `clap` 4.x docs (Context7) | `value_parser = clap::value_parser!(PathBuf)` accepts `-`; the CLI then matches on `path.as_os_str() == "-"` to switch to stdin. The existing CLI uses `Vec<PathBuf>` already; only the dispatch logic changes. |
| `insta` snapshot review | `insta` 1.x docs | `cargo insta review` for golden file updates. Use `insta::assert_json_snapshot!` for diagnostic JSON snapshots so a structural drift in the contract is loud. |
| `serde_json` NDJSON | stdlib + serde | Each diagnostic is `serde_json::to_string(&diag)?` followed by `\n`; flush per-record, not per-buffer, so a panic mid-stream still produces complete-line output. |
| `is-terminal` for TTY detection | `is-terminal` 0.4 (already a transitive dep via tracing-subscriber) | `IsTerminal::is_terminal(&std::io::stdout())` to choose `human` vs `json` default per contract. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly. Every
snippet below is verbatim from the current source — no inventions.

### NAMING_CONVENTION
```rust
// SOURCE: crates/marque-capco/src/rules.rs:62-74
/// Banners must use full words: SECRET not S, NOFORN not NF, TOP SECRET not TS.
struct BannerAbbreviationRule;

impl Rule for BannerAbbreviationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E001")
    }
    fn name(&self) -> &'static str {
        "banner-abbreviation"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
```

Rules are zero-size structs named `<Concept>Rule`, registered in
`CapcoRuleSet::new()` via `Box::new(<Name>Rule)`. Rule ID strings are
`E\d{3}` / `W\d{3}` / `C\d{3}`. The `name()` is kebab-case matching the
config-key form (`banner-abbreviation`).

### ERROR_HANDLING — rule check loop
```rust
// SOURCE: crates/marque-capco/src/rules.rs:76-101
fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
    use marque_ism::MarkingType;
    if ctx.marking_type != MarkingType::Banner {
        return vec![];
    }
    let mut diagnostics = Vec::new();
    for control in attrs.dissem_controls.iter() {
        if let Some(full) = expand_dissem_abbreviation(control) {
            let abbrev = control.as_str().to_owned();
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                span: Span::new(0, 0), // TODO: wire actual span (Phase 3)
                ...
            }));
        }
    }
    diagnostics
}
```

Rules return `Vec<Diagnostic>` (never `Result`). Failure modes are
"no diagnostics" or "a diagnostic with `severity=Error` and no fix".
Rules NEVER panic — `Span::new` is the only thing that could, and Phase
3 must hand it a real `start <= end`.

### LOGGING_PATTERN
```rust
// SOURCE: marque/src/main.rs:65-68
tracing_subscriber::fmt()
    .with_env_filter(std::env::var("MARQUE_LOG").unwrap_or_else(|_| "marque=info".to_owned()))
    .init();
```

`tracing` is initialized once in `main`. Rule code never logs — diagnostics
are the output channel. CLI narration uses `eprintln!` at `info!`/`warn!`
level via `tracing::info!`, gated by `-q`.

### REPOSITORY/SERVICE PATTERN — engine wiring
```rust
// SOURCE: marque/src/main.rs:85-94
let config = match marque_config::load(&cwd) {
    Ok(c) => c,
    Err(e) => {
        eprintln!("error: {e}");
        process::exit(e.exit_code());
    }
};

let engine = Engine::new(config, vec![Box::new(capco_rules())]);
```

`Engine::new(config, rule_sets)` is the construction site. Phase 3
preserves this exactly — only the dispatch around `engine.lint(&source)`
changes.

### FIX HELPER PATTERN
```rust
// SOURCE: crates/marque-capco/src/rules.rs:250-282
struct FixDiagnosticParams {
    rule: RuleId,
    severity: Severity,
    span: Span,
    message: String,
    citation: &'static str,
    original: String,
    replacement: String,
    confidence: f32,
    migration_ref: Option<&'static str>,
}

fn make_fix_diagnostic(p: FixDiagnosticParams) -> Diagnostic {
    let proposal = FixProposal::new(
        p.rule.clone(),
        FixSource::BuiltinRule,
        p.span,
        p.original,
        p.replacement,
        p.confidence,
        p.migration_ref,
    );
    Diagnostic::new(p.rule, p.severity, p.span, p.message, p.citation, Some(proposal))
}
```

Every rule that produces a fix goes through `make_fix_diagnostic`.
Phase 3 may extend this with a `source: FixSource` field if a rule needs
`MigrationTable` or `CorrectionsMap` instead of the default `BuiltinRule`
— E006/E007/W001 will need `MigrationTable`.

### TEST_STRUCTURE — unit test pattern
```rust
// SOURCE: crates/marque-core/src/parser.rs:241-262
#[cfg(test)]
mod tests {
    use super::*;
    use marque_ism::span::{MarkingCandidate, MarkingType, Span};
    use marque_ism::token_set::CapcoTokenSet;

    fn make_candidate(text: &[u8], kind: MarkingType, offset: usize) -> MarkingCandidate {
        MarkingCandidate {
            span: Span::new(offset, offset + text.len()),
            kind,
        }
    }

    fn parse_banner(text: &str) -> ParsedMarking {
        let source = text.as_bytes();
        let tokens = CapcoTokenSet;
        let parser = Parser::new(&tokens);
        let candidate = make_candidate(source, MarkingType::Banner, 0);
        parser.parse(&candidate, source).expect("parse should succeed")
    }
```

Unit tests live in `#[cfg(test)] mod tests` at the bottom of each source
file. Helpers are private fns inside the test module. Snake_case test
names describe behavior (`banner_with_declass_date_populates_attrs`).

### TEST_STRUCTURE — integration test pattern
```rust
// SOURCE: crates/marque-engine/src/engine.rs (the engine_with_config helper)
fn engine_with_config(config: Config, proposals: Vec<FixProposal>) -> Engine {
    let mut rules = Vec::new();
    for proposal in proposals {
        rules.push(Box::new(SingleProposalRule { proposal }) as Box<dyn Rule>);
    }
    Engine::new(config, vec![Box::new(StaticRuleSet { rules })])
}
```

Integration tests under `crates/<crate>/tests/<topic>.rs` build an
`Engine` directly with their own rule set, then drive it through `lint`
or `fix`. They do NOT spin up a CLI process — that lives in
`crates/marque/tests/cli_smoke.rs` (T042).

---

## Files to Change

| File | Action | Justification |
|---|---|---|
| `tests/corpus/invalid/banner_abbrev.txt` | CREATE | T025 — canonical E001 fixture per acceptance test |
| `tests/corpus/invalid/banner_abbrev.expected.json` | CREATE | T025 — golden file with rule + span |
| `tests/corpus/invalid/missing_usa_trigraph.txt` | CREATE | T025 — canonical E002 fixture |
| `tests/corpus/invalid/missing_usa_trigraph.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/misordered_blocks.txt` | CREATE | T025 — canonical E003 fixture |
| `tests/corpus/invalid/misordered_blocks.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/separator_count.txt` | CREATE | T025 — canonical E004 fixture |
| `tests/corpus/invalid/separator_count.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/declass_in_banner.txt` | CREATE | T025 — canonical E005 fixture |
| `tests/corpus/invalid/declass_in_banner.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/deprecated_dissem.txt` | CREATE | T025 — canonical E006 fixture |
| `tests/corpus/invalid/deprecated_dissem.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/x_shorthand_date.txt` | CREATE | T025 — canonical E007 fixture |
| `tests/corpus/invalid/x_shorthand_date.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/unknown_token.txt` | CREATE | T025 — canonical E008 fixture |
| `tests/corpus/invalid/unknown_token.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/deprecated_warning.txt` | CREATE | T025 — canonical W001 fixture |
| `tests/corpus/invalid/deprecated_warning.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/invalid/<rule>_2.txt` × 9 rules × 2 extras each | CREATE | T025 — ≥3 fixtures per rule (canonical + 2 variants), 18 additional files |
| `tests/corpus/invalid/<rule>_2.expected.json` × 18 | CREATE | T025 — golden files for the variants |
| `tests/corpus/invalid/mixed_confidence.txt` | CREATE | T025 — canonical mixed-confidence fixture (used by Phase 4 too) |
| `tests/corpus/invalid/mixed_confidence.expected.json` | CREATE | T025 — golden file |
| `tests/corpus/valid/clean_portion.txt` | CREATE | T026 — known-good portion fixture |
| `tests/corpus/valid/clean_portion.expected.json` | CREATE | T026 — `{"diagnostics": []}` |
| `tests/corpus/valid/clean_banner.txt` | CREATE | T026 — known-good banner |
| `tests/corpus/valid/<19 more>.{txt,expected.json}` | CREATE | T026 — ≥20 known-good fixtures total spanning portion, banner, and CAB types |
| `tests/corpus/prose/article.txt` | CREATE | T026a — ≥1000 lines body prose, 20+ incidental `(S)` / `(a)` mid-sentence |
| `crates/marque-capco/tests/rules_us1.rs` | CREATE | T027 — drives every fixture from T025/T026 through `Engine::lint`, asserts rule ID + span |
| `crates/marque-engine/tests/lint_pipeline.rs` | CREATE | T028 — happy path FR-001/002/003 + edge cases (empty, whitespace, mid-sentence `(S)`, unknown token) |
| `crates/marque-engine/tests/snapshots/` | CREATE (directory) | T029 — `insta` snapshot files for human + JSON diagnostic formats |
| `crates/marque/tests/cli_smoke.rs` | CREATE | T042 — CLI integration test against ≥3 corpus fixtures |
| `crates/marque-test-utils/src/lib.rs` | UPDATE | Add `load_fixture(path) -> (source, expected)` helper used by every integration test |
| `crates/marque-capco/src/rules.rs` | UPDATE | T030 — replace `Span::new(0, 0)` placeholders, finish E004 (currently a stub returning `vec![]`), add `MisorderedBlocksRule` (E003), `DeprecatedDissemRule` (E006), `XShorthandDateRule` (E007), `UnknownTokenRule` (E008), `DeprecatedMarkingWarningRule` (W001) |
| `crates/marque-capco/src/lib.rs` | UPDATE | T039 — register E003, E006, E007, E008, W001 in `CapcoRuleSet::new()` |
| `crates/marque-rules/src/lib.rs` | UPDATE | Extend `RuleContext` with `marking_span: Span` and `source: &'a [u8]` (or use a borrowed lifetime parameter); the field replaces the convention that `attrs` has been parsed from a known source span |
| `crates/marque-core/src/parser.rs` | UPDATE | Extend `parse_marking_string` to record per-token spans into a new `IsmAttributes::token_spans: Box<[(TokenKind, Span)]>` (or pass a `&mut Vec<(TokenKind, Span)>` sink); the rules use these to point at the *exact* offending byte range, not the whole marking. |
| `crates/marque-ism/src/attrs.rs` | UPDATE | Add `IsmAttributes::token_spans` (or a parallel `IsmTokenSpans` sidecar struct held alongside `IsmAttributes` in `ParsedMarking`) so spans are reachable from every rule without re-parsing. |
| `crates/marque-core/src/scanner.rs` | UPDATE | Add `MarkingType::PageBreak` and a `scan_page_breaks` pass that emits a candidate at every `\f` (form feed) and at every `\n\n\n+` (3+ blank lines, US-letter heuristic). |
| `crates/marque-ism/src/span.rs` | UPDATE | Add `MarkingType::PageBreak` variant. |
| `crates/marque-engine/src/engine.rs` | UPDATE | Reset `page_context = PageContext::new()` and clear `page_context_arc` when `candidate.kind == MarkingType::PageBreak`. Replace the `TODO(phase-3)` at line 80 with the real reset call. Wire `parsed.token_spans` (or sidecar) into `RuleContext`. |
| `marque/src/main.rs` | UPDATE (rewrite) | T040, T041, T042a — match `contracts/cli.md` exactly: stdin sentinel, `--config`, `--confidence-threshold`, `--format`, `--no-color`, `-q`, `-v`, `--explain-config`, contract exit codes, NDJSON output |
| `marque/src/render.rs` | CREATE | T041 — split rendering into `human_render` (rustc-style with caret + citation) and `json_render` (NDJSON conforming to `contracts/diagnostic.json`); honors `NO_COLOR`, `TERM=dumb`, `--no-color` |
| `marque/Cargo.toml` | UPDATE | Add `is-terminal` (TTY detect for default `--format`), `serde` + `serde_json` (already transitive but make explicit), `colored` or `owo-colors` for ANSI rendering |
| `crates/marque-engine/src/output.rs` | UPDATE | Add `LintResult::has_errors()` / `has_warnings()` so the CLI exit-code logic doesn't recompute via `error_count() > 0`. Preserves the existing `error_count`/`warn_count` API. |
| `tests/snapshots/<n>.snap` | CREATE | `insta` snapshot files for the JSON diagnostic shape across the canonical fixtures |
| `CLAUDE.md` | UPDATE | Strike the Phase-3 TODO bullets in "Architectural Invariants" once `RuleContext.zone/position` and `PageContext` reset are wired. Replace with a one-line note that they are now Phase-3-correct. |

## NOT Building

These are explicitly OUT OF SCOPE for Phase 3 — do not let any of them
creep in. They belong to later phases or future work.

- **`marque fix` subcommand body** — Phase 4 / US2 (T047–T051a). Phase 3
  only adds the `--confidence-threshold` flag so the shared option block
  exists. The `fix` subcommand keeps its current Phase-2 minimal
  implementation; Phase 4 rewrites it.
- **`[corrections]` map and rule C001** — Phase 5 / US3 (T056–T060).
  Phase 3 must NOT consume `Config::corrections` from any rule.
- **`MARQUE_CLASSIFIER_ID` plumbing into rules** — Phase 5 / US3 (T060).
  Phase 3 leaves `Config::user.classifier_id` untouched at the rule
  layer; the engine still passes it through to `AppliedFix` (already
  Phase-2 work).
- **WASM parity tests** — Phase 6 / US4 (T061–T066a).
- **Atomic temp-file rename for fix output** — Phase 4 / US2 (T048).
- **NDJSON audit-record stream from `fix`** — Phase 4 / US2 (T049). The
  `check` NDJSON is the diagnostic stream; `fix`'s audit stream is a
  separate codepath under Phase 4.
- **Performance benchmarks (`benches/lint_latency.rs` body)** — Phase 7
  (T067/T067a/T068). Phase 3 only needs the rules to *compile* fast
  enough for the test suite — measurement comes later.
- **Corpus accuracy harness `tests/corpus_accuracy.rs`** — Phase 7
  (T069). Phase 3 produces the fixtures; Phase 7 runs the ≥95% gate
  against them.
- **`tests/no_classifier_id_in_commits.rs`** — Phase 5 / US3 (T055). Do
  not add classifier-id-shaped strings to any new fixture; the canonical
  forms (`12345`, `00001`) are reserved for Phase 5's `.marque.local.toml`
  setup. Phase 3 fixtures must contain ZERO `classifier_id` values.
- **`cargo-fuzz` target** — Phase 7 (T072a).
- **CVE deprecation table expansion** — the existing 5-entry `MIGRATIONS`
  table in `build.rs` is enough for Phase 3. E006/E007/W001 read from
  what's there. Real ODNI deprecation list parsing is a separate Phase 3
  follow-up if `find_migration` returns no hits — explicit `TODO` allowed.

---

## Step-by-Step Tasks

### Task 1: Scanner — emit page-break candidates (PR-3 review TODO)
- **ACTION**: Add `MarkingType::PageBreak` and a `scan_page_breaks` pass.
- **IMPLEMENT**:
  - In `crates/marque-ism/src/span.rs`, add `PageBreak` to `MarkingType`.
  - In `crates/marque-core/src/scanner.rs`, add `scan_page_breaks(source, &mut out)`:
    - Use `memchr_iter(b'\x0c', source)` (form feed) — emit a zero-length
      `Span::new(pos, pos)` at every `\f`.
    - Use a regex-free walk for `\n\n\n+` (≥3 consecutive `\n`): track
      a run length, emit a candidate at the third newline.
    - Span is zero-length so it sorts to the right place via the existing
      `candidates.sort_unstable_by_key(|c| c.span.start)` line.
- **MIRROR**: `scan_portions` and `scan_banners` in scanner.rs:36–69 —
  same shape, push into `out: &mut Vec<MarkingCandidate>`.
- **IMPORTS**: `use memchr::memchr_iter;` (already present).
- **GOTCHA**: A zero-length span passes `Span::new`'s `start <= end`
  invariant. Do NOT use `Span::new(0, 0)` — use the *actual* offset.
  The existing `Span::is_empty()` check at engine.rs:163 in `fix_inner`
  will filter these from any fix dispatch (they aren't fixable), but
  `lint`'s rule loop must NOT skip page breaks.
- **VALIDATE**:
  - `cargo test -p marque-core scanner::tests::detects_page_break`
  - New unit test: feed `b"page1\n\f\npage2\n"` and assert one
    `PageBreak` candidate at offset 6.

### Task 2: Engine — reset PageContext at page breaks
- **ACTION**: In `engine.rs::lint`, when `candidate.kind ==
  MarkingType::PageBreak`, call `page_context = PageContext::new();
  page_context_arc = None;` and `continue`. Do not run any rule against
  a `PageBreak` candidate.
- **IMPLEMENT**:
  ```rust
  // Inside the for-candidate loop in lint(), before parser.parse()
  if candidate.kind == MarkingType::PageBreak {
      page_context = PageContext::new();
      page_context_arc = None;
      continue;
  }
  ```
- **MIRROR**: The `if parsed.kind == MarkingType::Portion` block at
  engine.rs:96–101 — same `page_context_arc = None;` pattern.
- **IMPORTS**: None (PageContext already imported at line 68).
- **GOTCHA**: Place this check BEFORE `parser.parse(candidate, source)`.
  The parser does not understand `PageBreak` and will return `Err`,
  which the existing `let Ok(parsed) = ... else { continue; }` would
  swallow — so the reset would never happen.
- **VALIDATE**:
  - New unit test in `crates/marque-engine/src/engine.rs::tests`:
    ```rust
    #[test]
    fn page_context_resets_at_page_break() {
        // Page 1: TOP SECRET portion
        // Page break: \f
        // Page 2: SECRET banner
        // Banner expected_classification on page 2 must be Secret, not TopSecret.
    }
    ```

### Task 3: RuleContext — drop Phase-2 hardcoded `Body`
- **ACTION**: Either populate `zone`/`position` from scanner-derived
  document position OR remove the fields entirely until they have a real
  source. **Decision**: keep the fields but make them `Option<Zone>`
  and `Option<DocumentPosition>`, populated only when the scanner can
  prove a value. For Phase 3, scanner-derived position remains `None`
  (banner-line detection alone doesn't determine header vs footer
  reliably) — but the field type now correctly says "we don't know."
- **IMPLEMENT**: In `crates/marque-rules/src/lib.rs`, change
  `RuleContext.zone: Zone` to `Option<Zone>` and `position: DocumentPosition`
  to `Option<DocumentPosition>`. Update the engine call site at
  `engine.rs:121–122` to pass `None` for both. Delete the Phase-2 `TODO`
  comment block at engine.rs:103–106.
- **MIRROR**: `RuleContext.page_context: Option<Arc<PageContext>>` at
  rules/lib.rs:142 — same Optional shape and same "lazy fill" semantics.
- **IMPORTS**: None.
- **GOTCHA**: This is a public API break for any rule that reads
  `ctx.zone`. Today no rule does (verified by `grep -rn 'ctx\.zone\|ctx\.position'
  crates/`). If the grep returns a hit, that rule must be updated in
  the same commit.
- **VALIDATE**:
  - `cargo build --workspace` clean.
  - `cargo test --workspace` clean — no rule tests should reference
    `zone` or `position`.

### Task 4: Parser — record per-token spans into IsmAttributes
- **ACTION**: Extend `IsmAttributes` with `token_spans:
  Box<[TokenSpan]>`, where `TokenSpan { kind: TokenKind, span: Span }`.
  `TokenKind` is a new enum with variants `Classification`,
  `SciControl`, `DissemControl`, `RelToTrigraph`, `SarIdentifier`,
  `DeclassExemption`, `DeclassDate`, `Separator`, `Unknown`.
- **IMPLEMENT**:
  - In `crates/marque-ism/src/attrs.rs`, add a new module
    `attrs::token_span` defining `TokenKind` (derive `Debug, Clone, Copy,
    PartialEq, Eq`) and `TokenSpan { pub kind: TokenKind, pub span: Span }`.
  - Add `IsmAttributes::token_spans: Box<[TokenSpan]>` (default `Box::new([])`).
  - In `crates/marque-core/src/parser.rs::parse_marking_string`, replace
    the `for block in &blocks[1..]` loop with a span-tracking variant
    that walks the original string and records `(start, end)` for every
    token recognized. The current `s.split("//")` discards offsets — use
    `s.match_indices("//")` to track separator positions, then walk each
    block with offset arithmetic to land each recognized token's span
    relative to the marking-string start, then add the marking's
    `candidate.span.start` to get absolute offsets.
- **MIRROR**: The existing `parse_marking_string` flow at parser.rs:127–181
  — same dispatch on `SciControl::parse / DissemControl::parse / ...`,
  but each branch also pushes a `TokenSpan` to a sink.
- **IMPORTS**: `use marque_ism::attrs::{TokenKind, TokenSpan};` in
  parser.rs.
- **GOTCHA 1**: `s.split("//")` does not give offsets. Use
  `s.match_indices("//").map(|(i, _)| i)` to get separator offsets, then
  manually slice between them.
- **GOTCHA 2**: For portion markings, the marking-string offset is
  `candidate.span.start + 1` (skip the `(`). For banner markings it is
  `candidate.span.start` (no leading paren). The existing
  `parse_portion` strips the parens with `strip_prefix('(').and_then(|s|
  s.strip_suffix(')'))` — do that math once and pass it to
  `parse_marking_string`.
- **GOTCHA 3**: Whitespace inside a block (e.g., `"REL TO USA, GBR"`)
  must be normalized in the recognized-token span — point the span at
  `USA`, not at the leading space. `block.trim_start()` returns a
  subslice; use `subslice.as_ptr() as usize - original.as_ptr() as
  usize` to recover the offset within `original`. This trick is already
  used in scanner.rs:62.
- **VALIDATE**:
  - `cargo test -p marque-core parser::tests::token_spans_track_offsets`
  - New unit test:
    ```rust
    #[test]
    fn token_spans_track_offsets() {
        let parsed = parse_banner("TOP SECRET//SI//NF");
        // Expect: Classification span 0..10, SciControl span 12..14,
        //         DissemControl span 16..18.
    }
    ```

### Task 5: Replace Span::new(0, 0) placeholders in existing rules
- **ACTION**: In `crates/marque-capco/src/rules.rs`, replace every
  `Span::new(0, 0)` with the corresponding `TokenSpan` from `attrs.token_spans`.
- **IMPLEMENT**:
  - **E001 BannerAbbreviationRule** (rules.rs:88): for each abbreviated
    `DissemControl`, find the `TokenSpan` whose `kind == DissemControl`
    and whose `span.as_str(source)` equals the abbreviation. Use that
    span. To do that, the rule needs `source` — which means
    `RuleContext` must carry `source: &[u8]`. Add `RuleContext.source:
    &'a [u8]` (lifetime-parameterized).
  - **E002 MissingUsaTrigraphRule** (rules.rs:172): the span is the
    union of all `RelToTrigraph` token spans, OR the first
    `RelToTrigraph` span if `has_usa && !usa_first`.
  - **E005 DeclassifyInBannerRule** (rules.rs:236): the span is the
    `DeclassExemption` or `DeclassDate` token span — whichever is
    populated.
- **MIRROR**: `make_fix_diagnostic` shape unchanged (rules.rs:264–282).
- **IMPORTS**: None new in rules.rs.
- **GOTCHA**: `RuleContext` becoming generic over a source lifetime is a
  workspace-wide ripple. Either:
  (a) Use `RuleContext<'a>` with `source: &'a [u8]` and add the lifetime
      to every `Rule::check` signature — clean but invasive.
  (b) Don't add `source` to `RuleContext` at all; instead match the
      token spans by *position* (the rule already knows which abbreviated
      `DissemControl` it found by index into `attrs.dissem_controls`,
      so it can look up `attrs.token_spans` by the same index). This is
      the simpler path; **prefer (b)**.
  - Decision: **(b)**. Use parallel-indexing into `attrs.token_spans`
    filtered by `kind`. The rules never need raw source bytes in Phase 3.
- **VALIDATE**:
  - `cargo test -p marque-capco` — existing E001/E002/E005 unit tests
    should now produce non-zero spans. Update the asserted spans.
  - Run the canonical fixtures from Task 14 through `Engine::lint`
    and verify the diagnostic spans match the `.expected.json` files.

### Task 6: Implement E003 — MisorderedBlocksRule
- **ACTION**: New struct `MisorderedBlocksRule` that fires when a banner's
  blocks are not in CAPCO order: `Classification → SCI → SAR → Dissem
  (incl. REL TO)`. Suggest a reordered banner with `confidence = 0.6`
  (kept as suggestion under the default 0.95 threshold).
- **IMPLEMENT**:
  - The rule reads `attrs.token_spans` and checks the order of token
    `kind` values. The expected order is: one `Classification`, then any
    number of `SciControl`, then any number of `SarIdentifier`, then any
    number of `DissemControl` / `RelToTrigraph` (REL TO is part of the
    dissem block).
  - When out of order, build the reordered string by emitting
    `attrs.classification.unwrap().banner_str()`, then sorted `sci`,
    `sar`, `dissem`, `REL TO ...`.
  - Span is the marking's whole `source_span` (already on `ParsedMarking`
    — needs to be threaded through `RuleContext` as a new field
    `marking_span: Span`).
- **MIRROR**: `BannerAbbreviationRule` shape (rules.rs:62–102).
- **IMPORTS**: `use marque_ism::attrs::TokenKind;`
- **GOTCHA**: Banners may legitimately omit blocks (e.g., `SECRET`
  alone). An empty banner is in order by definition — return `vec![]`.
  Also: portion markings have their own ordering (same rules), so this
  rule fires for both `Banner` and `Portion`.
- **VALIDATE**:
  - Unit test: `(SECRET//NF//SI)` → fires E003, suggests `(SECRET//SI//NF)`.
  - Unit test: `(SECRET//SI//NF)` → no diagnostic.
  - Run against `tests/corpus/invalid/misordered_blocks.txt`.

### Task 7: Implement E004 — SeparatorCountRule (replace stub)
- **ACTION**: Replace the current `vec![]` stub at rules.rs:200 with a
  real implementation that detects extra or missing `//` separators
  inside markings. Confidence `0.99`.
- **IMPLEMENT**:
  - Scan `attrs.token_spans` for adjacent `Separator` tokens (Task 4
    must record separators) — `// //` is `SeparatorSeparator`. Or
    detect `///` (3 slashes) by checking the byte offset between two
    consecutive `Separator` spans is 0.
  - Suggested fix replaces the offending separator run with exactly `//`.
  - Span is the offending separator run.
- **MIRROR**: `BannerAbbreviationRule` (rules.rs:62–102) for the
  diagnostic shape.
- **IMPORTS**: None new.
- **GOTCHA**: The Phase-2 placeholder reason for E004 being a stub is
  documented in the comment: `Requires raw source text in rule context
  — not available until Phase 3`. Now it IS available via
  `attrs.token_spans` (which is built from raw source by Task 4) — that
  comment must be deleted.
- **VALIDATE**:
  - Unit test: `TOP SECRET////NOFORN` (4 slashes) → fires E004,
    suggests `TOP SECRET//NOFORN`.
  - Unit test: `TOP SECRET/NOFORN` (1 slash) → fires E004, suggests
    `TOP SECRET//NOFORN`.
  - Run against `tests/corpus/invalid/separator_count.txt`.

### Task 8: Implement E006 — DeprecatedDissemRule
- **ACTION**: New rule that fires when a `DissemControl` matches an
  entry in `marque_ism::generated::values::find_migration` (i.e., the
  `MIGRATIONS` table). Suggested fix is `entry.replacement` with
  `confidence = entry.confidence`. Source is `FixSource::MigrationTable`.
- **IMPLEMENT**:
  - For each `DissemControl` in `attrs.dissem_controls`, call
    `marque_ism::generated::values::find_migration(control.as_str())`.
    If `Some`, emit a diagnostic with `migration_ref =
    Some(entry.reference)`.
- **MIRROR**: `BannerAbbreviationRule` (rules.rs:62–102) but use
  `FixSource::MigrationTable` in `make_fix_diagnostic`.
- **IMPORTS**: `use marque_ism::generated::values::find_migration;`
- **GOTCHA**: The current `MIGRATIONS` table only has 5 entries
  (`LIMDIS`, `FOUO`, `NF`, `25X1-`, `50X1-`). E006 fires only on
  `LIMDIS` and `FOUO` (the dissem entries). `NF` is handled by E001
  (abbreviation), not E006 — so the rule must NOT fire on `NF` even
  though it's in the migration table. Filter by checking the entry's
  `replacement` is itself a valid `DissemControl` (not just an
  abbreviation expansion).
- **VALIDATE**:
  - Unit test: `(SECRET//LIMDIS)` → fires E006, suggests `RELIDO`.
  - Unit test: `(SECRET//NF)` → no E006 (E001 owns this).
  - Run against `tests/corpus/invalid/deprecated_dissem.txt`.

### Task 9: Implement E007 — XShorthandDateRule
- **ACTION**: New rule that fires when `attrs.declass_exemption` matches
  one of the X-shorthand patterns (`25X1-`, `50X1-`) in `MIGRATIONS`.
  Source is `FixSource::MigrationTable`. Confidence `0.97` per
  `MIGRATIONS` table.
- **IMPLEMENT**:
  - Check `attrs.declass_exemption.as_str()` against `find_migration`.
  - If `Some(entry)`, suggest `entry.replacement`.
- **MIRROR**: E006 (Task 8).
- **IMPORTS**: Same as E006.
- **GOTCHA**: `DeclassExemption` is a generated enum. `as_str` returns
  the canonical CVE form, which may differ from the migration table key.
  E.g., the table key is `"25X1-"` (with trailing dash) but the CVE
  enum form is `"25X1"`. The rule must handle this by also checking
  the ParsedMarking's *raw bytes* via `marking_span.as_str(source)` —
  which means E007 needs `source` access. **Decision**: pass `source` to
  the rule via the `RuleContext.source: &[u8]` field added in Task 5
  Option (a) — the simpler index-based path doesn't help here because
  the migration key is a *byte pattern*, not a parsed token.
  - Alternative: Add a parallel `MIGRATIONS_BY_PATTERN` table in
    `build.rs` keyed by exact ASCII pattern, looked up via
    `attrs.token_spans` against the original source. This avoids the
    `RuleContext.source` ripple. **Prefer this alternative**.
- **VALIDATE**:
  - Unit test: `Declassify On: 25X1-` → fires E007, suggests `25X1`.
  - Run against `tests/corpus/invalid/x_shorthand_date.txt`.

### Task 10: Implement E008 — UnknownTokenRule
- **ACTION**: New rule that fires when the parser encountered tokens
  inside a marking candidate boundary that did NOT match any known
  CVE token. Severity `Error`, no fix offered (FR-012).
- **IMPLEMENT**:
  - The parser currently silently drops unrecognized tokens at
    parser.rs:169–170 (`Other unrecognized tokens are silently dropped
    here. The rules layer (E008) detects and reports them.`). But the
    parser doesn't currently retain them anywhere. This must change:
    - In Task 4's `IsmAttributes::token_spans`, also record `TokenKind::Unknown`
      for any non-empty block that didn't match any known parse path.
    - E008 reads `attrs.token_spans.iter().filter(|t| t.kind ==
      TokenKind::Unknown)` and emits one diagnostic per unknown token.
  - Diagnostic span is the unknown token's span.
- **MIRROR**: E001 shape, but use `Diagnostic::new(..., None)` for the
  fix (no fix proposal).
- **IMPORTS**: `use marque_ism::attrs::TokenKind;`
- **GOTCHA**: A marking like `(S//FOO//NF)` would emit `TokenKind::Unknown`
  for `FOO`, but `S` and `NF` still parse correctly. The rule's per-
  diagnostic granularity must be PER unknown token, not "the whole
  marking is unknown."
- **VALIDATE**:
  - Unit test: `(SECRET//XYZZY//NOFORN)` → fires E008 on `XYZZY`.
  - Unit test: `(SECRET//NOFORN)` → no E008.
  - Run against `tests/corpus/invalid/unknown_token.txt`.

### Task 11: Implement W001 — DeprecatedMarkingWarningRule
- **ACTION**: New rule, severity `Warn`, that fires when an older but
  still-valid marking appears (e.g., a marking that was renamed in a
  later CAPCO revision but not removed). Confidence `0.97`. Source
  `FixSource::MigrationTable`.
- **IMPLEMENT**:
  - Distinct from E006/E007: the latter are *errors* because the
    marking is no longer valid. W001 is a *warning* because the marking
    is still legal but a newer canonical form exists.
  - For Phase 3, the only W001-eligible entries in `MIGRATIONS` are
    those marked with a "still valid" annotation. **The current 5-entry
    `MIGRATIONS` table has no such marker** — Task 11 must add a
    `MigrationEntry.deprecated_severity: Severity` field (or a parallel
    `bool is_warning_only`) and update the 5 entries: `LIMDIS`,
    `FOUO`, `NF`, `25X1-`, `50X1-` are all `Severity::Error` (no W001
    triggers from the seed set). So W001 in Phase 3 effectively reads:
    "if migration table is empty of W001-flagged entries, the rule
    fires zero diagnostics — that is acceptable." Plan a minimum-viable
    W001 fixture that triggers via a *test-only* migration entry
    injected through a custom `RuleSet` in the integration test.
- **MIRROR**: E006 (Task 8) shape.
- **IMPORTS**: Same as E006.
- **GOTCHA**: Without W001-flagged entries in `MIGRATIONS`, the
  fixture-based test is the only way to validate the rule. Use the
  `crates/marque-capco/tests/rules_us1.rs` integration test to
  construct a custom `MigrationEntry` (test-only constant), feed it
  through a wrapper rule, and assert the warning fires.
- **VALIDATE**:
  - Integration test in `rules_us1.rs` uses a synthetic migration entry
    to drive W001.
  - Real corpus fixture `deprecated_warning.txt` is added but its
    `.expected.json` lists `[]` until Phase 3 follow-up adds a
    W001-flagged migration entry. Document this in the fixture
    `.expected.json` comment field (or sibling README).

### Task 12: Register all new rules
- **ACTION**: Update `CapcoRuleSet::new()` at
  `crates/marque-capco/src/rules.rs:35–46` to register E003, E006,
  E007, E008, W001.
- **IMPLEMENT**:
  ```rust
  rules: vec![
      Box::new(BannerAbbreviationRule),
      Box::new(MissingUsaTrigraphRule),
      Box::new(MisorderedBlocksRule),
      Box::new(SeparatorCountRule),
      Box::new(DeclassifyInBannerRule),
      Box::new(DeprecatedDissemRule),
      Box::new(XShorthandDateRule),
      Box::new(UnknownTokenRule),
      Box::new(DeprecatedMarkingWarningRule),
  ],
  ```
- **MIRROR**: The existing 4-rule registration block.
- **IMPORTS**: Each new rule struct already imported via the `mod` they
  live in; no new imports.
- **GOTCHA**: Order matters for diagnostic ordering — rules are run in
  registration order, but the FR-016 sort in `Engine::fix_inner` re-sorts
  fixes by span. For `lint`, diagnostic ordering follows scan candidate
  order × rule registration order. Keep registration in rule-ID order
  (E001, E002, E003, E004, E005, E006, E007, E008, W001) for
  predictability.
- **VALIDATE**:
  - `cargo test -p marque-capco` — registration test asserts 9 rules
    in `CapcoRuleSet::new().rules().len()`.

### Task 13: marque-test-utils — corpus loader helper
- **ACTION**: Add a `load_fixture(path: &str) -> (Vec<u8>, ExpectedDiagnostics)`
  helper that reads a fixture and its sibling `.expected.json`.
- **IMPLEMENT**:
  - `pub struct ExpectedDiagnostics { pub diagnostics: Vec<ExpectedDiagnostic> }`
  - `pub struct ExpectedDiagnostic { pub rule: String, pub start: usize, pub end: usize }`
  - Parse with `serde_json` (already in dev-deps).
- **MIRROR**: Existing helpers in `crates/marque-test-utils/src/lib.rs`
  (currently a one-function stub).
- **IMPORTS**: `use serde::Deserialize;`
- **GOTCHA**: `serde_json` may not be in `marque-test-utils` deps yet —
  add it under `[dependencies]` (NOT `[dev-dependencies]` since
  `marque-test-utils` is itself a dev-dep crate consumed by other crates'
  tests).
- **VALIDATE**:
  - `cargo test -p marque-test-utils` — round-trip a fixture.

### Task 14: Author the canonical corpus fixtures
- **ACTION**: Create the 11 canonical fixtures listed in the task list
  (T025 minimum + the 2 SC-003a-named fixtures `unknown_token.txt` and
  `mixed_confidence.txt`). Each fixture is a small text file with the
  exact violation, plus a `.expected.json` listing the expected rule(s)
  and exact byte spans.
- **IMPLEMENT**: Use the byte counts that the new span-aware parser
  produces (Task 4). For each fixture, run `cargo run -p marque -- check
  --format json <fixture>` once after Task 5 lands and copy the
  resulting span values into the `.expected.json` — this is the only
  way to be sure the spans are correct, because hand-counting bytes is
  error-prone with multibyte UTF-8 (none of the canonical fixtures use
  non-ASCII, but enforce the rule anyway).
- **MIRROR**: The schema in `tests/corpus/CORPUS_CONTRACT.md` and the
  example shape:
  ```json
  {
    "diagnostics": [
      {"rule": "E001", "start": 17, "end": 19}
    ]
  }
  ```
- **IMPORTS**: N/A (text fixtures).
- **GOTCHA 1**: Each fixture must contain ZERO classifier-id-shaped
  strings. The Phase-5 audit test (T055) will fail if any 5-digit number
  resembling a classifier ID slips in. Use citation strings like
  `"CAPCO-2016 §A.6"` and avoid lone 5-digit numbers anywhere in the
  `"CAPCO-2016 §A.6"` and avoid lone 5-digit numbers anywhere in the
  fixture text.
- **GOTCHA 2**: Each `.expected.json` file is self-contained and
  uses `additionalProperties: false` semantics — no comment field, no
  metadata. The fixture's purpose lives in the filename and a sibling
  `.txt` header comment if needed.
- **GOTCHA 3**: Generate ≥3 invalid fixtures per rule (T025 mandate).
  The 2 extras can be variants of the canonical (different abbreviation,
  different misorder pattern, etc.).
- **VALIDATE**:
  - Manual: Run `marque check tests/corpus/invalid/<fixture>.txt
    --format json` after Task 5 lands.
  - Automated: Task 16's `rules_us1.rs` integration test asserts every
    fixture matches its `.expected.json`.

### Task 15: Author the valid corpus fixtures + prose corpus
- **ACTION**: Create ≥20 known-good fixtures under `tests/corpus/valid/`
  plus the ≥1000-line prose corpus at `tests/corpus/prose/article.txt`.
- **IMPLEMENT**:
  - Valid fixtures: clean portion (`(TS//SI//NOFORN)`), clean banner
    (`TOP SECRET//SENSITIVE INTELLIGENCE//NOFORN`), clean CAB
    (`Classified By: ...\nDerived From: ...\nDeclassify On: 20330101`),
    etc. Spread across all three marking types. Each gets an empty
    `.expected.json`: `{"diagnostics": []}`.
  - Prose corpus: ≥1000 lines of Public Domain text (e.g., paragraphs
    from the EO 13526 Federal Register text, or Project Gutenberg
    public-domain prose). Insert ≥20 incidental parenthesized
    single-letter tokens (`(S)`, `(a)`, `(i)`, etc.) in mid-sentence
    positions. The expectation file does not exist for prose — the
    accuracy harness in T069 (Phase 7) reads them all and asserts
    zero diagnostics.
- **MIRROR**: `tests/corpus/CORPUS_CONTRACT.md` minima.
- **IMPORTS**: N/A.
- **GOTCHA**: The mid-sentence `(S)` heuristic relies on the
  scanner+parser disambiguation: a `(S)` followed by a lowercase letter
  or in mid-sentence position should NOT produce a `Classification::Secret`
  diagnostic. The current parser at parser.rs:192–200 unconditionally
  parses `"S"` as `Some(Classification::Secret)`. Phase 3 must add a
  context-aware filter at the scanner OR parser level — likely:
  in `scanner.rs::scan_portions`, reject candidates whose enclosing
  context has lowercase letters within ±5 bytes of the parens. Add
  this as an explicit subtask under Task 1 if the prose corpus surfaces
  false positives.
- **VALIDATE**:
  - `tests/corpus_provenance.rs` (T055a, lands in Phase 5) will scan
    every file under `tests/corpus/` for shape compliance. Phase 3
    must produce files that pass this scan once it lands.

### Task 16: Integration test — rules_us1.rs
- **ACTION**: Create `crates/marque-capco/tests/rules_us1.rs` that
  iterates every fixture in `tests/corpus/invalid/` AND
  `tests/corpus/valid/`, runs `Engine::lint`, and asserts the diagnostics
  match `.expected.json` exactly (rule ID + span).
- **IMPLEMENT**:
  ```rust
  use marque_capco::capco_rules;
  use marque_config::Config;
  use marque_engine::Engine;
  use marque_test_utils::load_fixture;
  use std::path::PathBuf;

  fn fixtures(dir: &str) -> Vec<PathBuf> {
      let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
          .parent().unwrap().parent().unwrap()
          .join("tests").join("corpus").join(dir);
      std::fs::read_dir(root).unwrap()
          .filter_map(|e| e.ok())
          .map(|e| e.path())
          .filter(|p| p.extension().is_some_and(|e| e == "txt"))
          .collect()
  }

  #[test]
  fn invalid_corpus_matches_expected_diagnostics() {
      let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);
      for path in fixtures("invalid") {
          let (source, expected) = load_fixture(path.to_str().unwrap());
          let result = engine.lint(&source);
          assert_eq!(
              result.diagnostics.len(),
              expected.diagnostics.len(),
              "fixture {path:?}: diagnostic count mismatch"
          );
          for (got, want) in result.diagnostics.iter().zip(expected.diagnostics.iter()) {
              assert_eq!(got.rule.as_str(), want.rule, "fixture {path:?}");
              assert_eq!(got.span.start, want.start, "fixture {path:?}");
              assert_eq!(got.span.end, want.end, "fixture {path:?}");
          }
      }
  }
  ```
- **MIRROR**: The tests/structure pattern in `crates/marque-engine/src/engine.rs::tests::engine_with_config`.
- **IMPORTS**: `marque-test-utils` as a dev-dependency in
  `crates/marque-capco/Cargo.toml`.
- **GOTCHA**: The default `Config` registers all rules, so any corpus
  fixture firing an unexpected diagnostic will fail loud. This is the
  design — corpus drift must be a CI failure.
- **VALIDATE**:
  - `cargo test -p marque-capco --test rules_us1`

### Task 17: Integration test — lint_pipeline.rs
- **ACTION**: Create `crates/marque-engine/tests/lint_pipeline.rs`
  covering FR-001/FR-002/FR-003 happy path and the spec edge cases:
  empty document, whitespace-only, mid-sentence `(S)`, unknown token.
- **IMPLEMENT**: 6+ tests, each constructing a small input string and
  asserting the resulting `LintResult` shape.
- **MIRROR**: `crates/marque-core/src/parser.rs::tests` shape.
- **IMPORTS**: `marque_engine::Engine`, `marque_config::Config`,
  `marque_capco::capco_rules`.
- **GOTCHA**: Empty document MUST produce `LintResult::is_clean() ==
  true` and `error_count() == 0`. Whitespace-only same. Mid-sentence
  `(S) ` (lowercase letter following) MUST NOT fire any rule.
- **VALIDATE**:
  - `cargo test -p marque-engine --test lint_pipeline`

### Task 18: Snapshot test for diagnostic JSON shape
- **ACTION**: Add `insta::assert_json_snapshot!` calls in
  `crates/marque-engine/tests/lint_pipeline.rs` for the human and JSON
  output of one canonical fixture (`banner_abbrev.txt`).
- **IMPLEMENT**:
  ```rust
  #[test]
  fn diagnostic_json_shape_is_stable() {
      let source = std::fs::read("../../tests/corpus/invalid/banner_abbrev.txt").unwrap();
      let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);
      let result = engine.lint(&source);
      let json: Vec<_> = result.diagnostics.iter().map(diagnostic_to_json).collect();
      insta::assert_json_snapshot!("banner_abbrev_diagnostics", json);
  }
  ```
- **MIRROR**: None — `insta` is new for this crate. Pattern follows the
  `insta` README.
- **IMPORTS**: Add `insta = "1"` to `[dev-dependencies]` of
  `crates/marque-engine/Cargo.toml`. Already in workspace dev-deps from
  Phase 1 T004.
- **GOTCHA**: Snapshots live under `crates/marque-engine/tests/snapshots/`.
  First run produces `*.snap.new` — rename via `cargo insta review`
  before committing.
- **VALIDATE**:
  - `cargo insta test --crate marque-engine`
  - `cargo insta accept` to land the initial snapshots.

### Task 19: CLI rewrite — `marque check` with full contract surface
- **ACTION**: Rewrite `marque/src/main.rs` (and add `marque/src/render.rs`)
  to match `contracts/cli.md` for the `check` subcommand. Leave `fix`
  and `metadata` minimally functional but extended with the common
  options.
- **IMPLEMENT**:
  - New `Cli` shape with shared `CommonOptions { config, confidence_threshold,
    format, no_color, quiet, verbose, explain_config }` flattened into
    each subcommand.
  - `Format` enum: `Human, Json` (NOT `text`; matches contract).
    Default to `Human` if stdout is a TTY, else `Json`. Honor
    `NO_COLOR` env var and `TERM=dumb`.
  - `[PATH...]` accepts the `-` sentinel for stdin. When no path is
    given, read stdin. Mixed paths + `-` allowed.
  - Exit codes: `0` clean, `1` errors, `2` warnings only, `64` usage,
    `65` config, `74` IO. Use `LintResult::error_count() > 0` /
    `warn_count() > 0` for the `0/1/2` decision.
  - `--explain-config` is mutually exclusive with input paths and the
    `fix` subcommand. Return `64` if combined.
  - `--config <PATH>` short-circuits the upward walk in
    `marque_config::load`.
  - `marque/src/render.rs` exposes:
    - `render_human(diagnostics: &[Diagnostic], source: &[u8], path: &Path,
      use_color: bool) -> String` — rustc-style with caret + citation.
    - `render_json(diagnostics: &[Diagnostic]) -> String` — NDJSON, one
      record per line, conforming to `contracts/diagnostic.json`.
- **MIRROR**: The existing `print_text_diagnostics` /
  `print_json_diagnostics` (marque/src/main.rs:194–229) for the
  rendering shape, but expand to match the contract.
- **IMPORTS**: `is-terminal = "0.4"`, `serde = { version = "1", features
  = ["derive"] }`, `serde_json = "1"`, `owo-colors = "4"`. Already in
  workspace dev-deps.
- **GOTCHA 1**: NDJSON is one record per line, ending with `\n`, NOT
  pretty-printed. The current `serde_json::to_string_pretty` at
  marque/src/main.rs:225 is wrong for the contract. Use
  `serde_json::to_string` and append `\n`.
- **GOTCHA 2**: `additionalProperties: false` in
  `contracts/diagnostic.json` means the JSON output MUST omit any
  field not in the schema. Build a `#[derive(Serialize)]` struct that
  matches the schema exactly, do NOT use `serde_json::json!{}` macros
  that may include extras.
- **GOTCHA 3**: `--explain-config` MUST emit `classifier_id_present:
  bool` — never the value itself. The contract wording is explicit:
  "classifier-id presence as a boolean, *not* the value". Test that
  the literal string `12345` (or any classifier-id-shaped value) never
  appears in the `--explain-config` output even when set in env or
  local config.
- **GOTCHA 4**: For stdin input (`-`), the CLI must NOT decode the
  input as UTF-8 before passing to `engine.lint` — the engine takes
  `&[u8]` and the parser handles UTF-8 validation per-span. Just call
  `std::io::stdin().read_to_end(&mut buf)`.
- **VALIDATE**:
  - `cargo build -p marque` clean.
  - `marque check tests/corpus/invalid/banner_abbrev.txt` → exit 1,
    diagnostic on stdout in human format.
  - `cat tests/corpus/invalid/banner_abbrev.txt | marque check -
    --format json` → exit 1, NDJSON on stdout matching
    `contracts/diagnostic.json`.
  - `marque check tests/corpus/valid/clean_banner.txt` → exit 0.
  - `marque check --explain-config` → exit 0, JSON config dump,
    no `classifier_id` value present.
  - `marque check --explain-config tests/corpus/invalid/banner_abbrev.txt`
    → exit 64 (mutually exclusive with paths).

### Task 20: CLI smoke test — cli_smoke.rs
- **ACTION**: Create `crates/marque/tests/cli_smoke.rs` that spawns
  `marque check` against ≥3 corpus fixtures and asserts stdout, stderr,
  exit code.
- **IMPLEMENT**:
  - Use `assert_cmd` (or `std::process::Command` directly) to spawn
    `env!("CARGO_BIN_EXE_marque")`.
  - Test cases:
    1. `marque check valid/clean_banner.txt` → exit 0, stdout empty.
    2. `marque check invalid/banner_abbrev.txt` → exit 1, stdout
       contains `E001`.
    3. `marque check invalid/banner_abbrev.txt --format json` → exit 1,
       stdout is valid NDJSON, first line parses as JSON object with
       `rule == "E001"`.
    4. `cat invalid/banner_abbrev.txt | marque check -` → exit 1,
       same content as #2.
    5. `marque check --explain-config` → exit 0, stdout is valid JSON
       containing `confidence_threshold` and `schema_version` keys.
- **MIRROR**: None — `crates/marque/tests/` does not exist yet. Create it.
- **IMPORTS**: `assert_cmd = "2"` in `marque/Cargo.toml`
  `[dev-dependencies]`. Already in workspace dev-deps from Phase 1 T004.
- **GOTCHA**: `CARGO_BIN_EXE_marque` is set by Cargo when compiling
  integration tests of binary crates. Use it directly, not
  `env::current_exe`.
- **VALIDATE**:
  - `cargo test -p marque --test cli_smoke`

### Task 21: Update CLAUDE.md — close the Phase-3 invariants
- **ACTION**: Update the "Architectural Invariants (do not bypass)"
  section in `CLAUDE.md`:
  - Strike the bullet `RuleContext.zone and RuleContext.position are not
    trustworthy in Phase 2.` — replace with: `RuleContext.zone and
    RuleContext.position are now Option-typed; both are None until the
    scanner can prove otherwise.`
  - Strike the bullet `PageContext never resets between pages today.`
    — replace with: `PageContext resets at scanner-emitted page-break
    candidates (Phase 3). The reset happens before parser.parse() runs
    on the page-break candidate, so a corrupted candidate cannot block
    the reset.`
- **MIRROR**: The existing bullet shape in CLAUDE.md.
- **IMPORTS**: N/A.
- **GOTCHA**: Do NOT delete the bullets — the surrounding text explains
  WHY they exist and the historical context is load-bearing for future
  contributors.
- **VALIDATE**: `git diff CLAUDE.md` shows the bullet rewrites.

---

## Testing Strategy

### Unit Tests

| Test | Input | Expected Output | Edge Case? |
|---|---|---|---|
| `scanner::page_break_form_feed` | `b"a\fb"` | one `PageBreak` candidate at offset 1 | yes |
| `scanner::page_break_blank_lines` | `b"a\n\n\nb"` | one `PageBreak` candidate | yes |
| `parser::token_spans_track_offsets` | `"TOP SECRET//SI//NF"` | spans 0..10, 12..14, 16..18 | no |
| `parser::token_spans_strip_paren` | `"(SECRET//NF)"` | classification span 1..7 (not 0..6) | yes |
| `engine::page_context_resets_at_page_break` | portion-then-pagebreak-then-banner input | banner sees only second-page portions | yes |
| `engine::page_break_does_not_run_rules` | `b"\f"` only | empty `LintResult` | yes |
| `rules::e001_uses_real_span` | `b"TOP SECRET//NF"` | E001 fires with span 12..14 | no |
| `rules::e002_uses_first_trigraph_span` | `b"S//REL TO GBR, AUS"` | E002 fires with span pointing at `GBR` | no |
| `rules::e003_misordered_blocks_fires` | `b"(SECRET//NF//SI)"` | E003 fires with confidence 0.6 | no |
| `rules::e003_correct_order_no_fire` | `b"(SECRET//SI//NF)"` | no diagnostic | no |
| `rules::e004_missing_separator` | `b"SECRET/NOFORN"` | E004 fires, suggests `//` | no |
| `rules::e004_extra_separators` | `b"SECRET////NOFORN"` | E004 fires, suggests `//` | no |
| `rules::e006_limdis_to_relido` | `(SECRET//LIMDIS)` | E006 fires, suggests RELIDO | no |
| `rules::e006_does_not_fire_on_nf` | `(SECRET//NF)` | no E006 (E001 owns this) | yes |
| `rules::e007_x_shorthand` | `Declassify On: 25X1-` | E007 fires, suggests 25X1 | no |
| `rules::e008_unknown_token` | `(SECRET//XYZZY//NOFORN)` | E008 fires on XYZZY | no |
| `rules::e008_no_fix_offered` | `(SECRET//XYZZY)` | diagnostic.fix is None | yes |
| `rules::w001_with_synthetic_migration` | injected migration + matching marking | W001 fires at Warn severity | yes |
| `cli::stdin_dash_sentinel` | `cat fixture | marque check -` | exit 1, NDJSON on stdout | no |
| `cli::no_color_env_var` | `NO_COLOR=1 marque check fixture` | no ANSI in stdout | yes |
| `cli::explain_config_no_classifier_id` | env `MARQUE_CLASSIFIER_ID=12345`, then `marque check --explain-config` | stdout contains `classifier_id_present: true` and never `12345` | yes |

### Edge Cases Checklist
- [ ] Empty document — `Engine::lint(b"")` returns clean result
- [ ] Whitespace-only document — `Engine::lint(b"   \n\n   ")` returns clean
- [ ] Mid-sentence `(S)` (lowercase context) — does NOT fire any rule
- [ ] Mid-sentence `(a)`, `(i)` enumeration markers — do NOT fire
- [ ] Single-line file with no trailing newline
- [ ] CRLF line endings — handled by `trim_ascii` in scanner
- [ ] Multi-page document — PageContext resets at every `\f`
- [ ] Unknown token within otherwise-valid marking — E008 fires only on the unknown token
- [ ] Empty REL TO list — E002 does NOT fire (only fires when REL TO present)
- [ ] Banner with classification only (e.g., `SECRET\n`) — no rule fires
- [ ] Stdin with no terminal newline
- [ ] Stdin with binary content (non-UTF-8) — `74 EX_IOERR`
- [ ] Mixed `-` and file path inputs
- [ ] `--format json` with TTY stdout (explicit override)
- [ ] `--no-color` overrides `NO_COLOR=` (empty)
- [ ] `--explain-config` with `fix` subcommand → exit 64

---

## Validation Commands

### Static Analysis
```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```
**EXPECT**: Zero errors, zero warnings.

### Unit Tests (per crate)
```bash
cargo test -p marque-core
cargo test -p marque-ism
cargo test -p marque-rules
cargo test -p marque-engine
cargo test -p marque-capco
cargo test -p marque-config
cargo test -p marque
```
**EXPECT**: Each crate's unit + integration tests pass. Total test
count rises from 67 (post-Phase-2) to ~115–130 (Phase 3 adds 9 rule
tests + 6 pipeline + 5 CLI smoke + 6 scanner/parser + ~20 fixtures
through `rules_us1.rs`).

### Full Test Suite via cargo nextest
```bash
cargo nextest run --workspace
```
**EXPECT**: All tests pass. nextest's per-test isolation catches any
Phase-3 test that mutates shared state (e.g., env vars used by the CLI
smoke tests must be set per-process via `Command::env`, not via
`std::env::set_var`).

### Insta Snapshot Review
```bash
cargo insta test --workspace
cargo insta review        # interactive — accept new snapshots
```
**EXPECT**: All snapshots match. New snapshots accepted on first land.

### Manual CLI Validation (against canonical fixtures)
```bash
# Build the CLI
cargo build -p marque

# Each of these is a smoke test for one contract surface
target/debug/marque check tests/corpus/invalid/banner_abbrev.txt           # exit 1
target/debug/marque check tests/corpus/valid/clean_banner.txt              # exit 0
target/debug/marque check tests/corpus/invalid/banner_abbrev.txt --format json | jq .rule
cat tests/corpus/invalid/banner_abbrev.txt | target/debug/marque check -   # exit 1
NO_COLOR=1 target/debug/marque check tests/corpus/invalid/banner_abbrev.txt
target/debug/marque check --explain-config | jq .
target/debug/marque check --explain-config tests/corpus/invalid/banner_abbrev.txt  # exit 64
```
**EXPECT**: Each invocation matches the contract behavior (exit code,
stream content, format).

### Corpus Provenance Sanity (manual, ahead of T055a in Phase 5)
```bash
# Every fixture file under tests/corpus/ must follow the path pattern
ls tests/corpus/invalid/*.txt tests/corpus/invalid/*.expected.json
ls tests/corpus/valid/*.txt tests/corpus/valid/*.expected.json
ls tests/corpus/prose/*.txt
# No classifier-id-shaped strings in any fixture
! grep -rE '\b[0-9]{5}\b' tests/corpus/invalid/ tests/corpus/valid/ tests/corpus/prose/
```
**EXPECT**: All fixtures present, no 5-digit numbers in any fixture.

---

## Acceptance Criteria

- [ ] All 21 tasks completed
- [ ] All validation commands pass
- [ ] `tests/corpus/invalid/` contains ≥30 fixtures (≥3 per rule × 10 rules)
- [ ] `tests/corpus/valid/` contains ≥20 fixtures
- [ ] `tests/corpus/prose/article.txt` contains ≥1000 lines + ≥20 mid-sentence single-letter parens
- [ ] Every rule (E001–E008, W001) emits byte-precise spans (no `Span::new(0, 0)`)
- [ ] `marque check` matches `contracts/cli.md` exit codes, flags, and output streams
- [ ] `marque check --explain-config` never emits a classifier-id value
- [ ] PageContext resets at scanner-emitted page-break candidates
- [ ] CLAUDE.md "Architectural Invariants" updated to reflect Phase-3 closures
- [ ] No new `unsafe` blocks (Trigraph's `from_utf8_unchecked` is the only one and was Phase 2's)
- [ ] Total test count ≥115
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --check` clean

## Completion Checklist

- [ ] Code follows discovered patterns (zero-size rule structs, kebab-case rule names, `make_fix_diagnostic` helper)
- [ ] Error handling matches codebase style (`Result` returns from parser/config; rules return `Vec<Diagnostic>` only)
- [ ] No `unwrap()` in CLI paths — every IO error maps to a contract exit code
- [ ] Tests follow `#[cfg(test)] mod tests` pattern with snake_case behavior names
- [ ] No hardcoded classifier-id-shaped values in any fixture
- [ ] No new `pub` API surface beyond what tasks specify
- [ ] CLAUDE.md updated for Phase-3 invariant closures
- [ ] Self-contained — no questions needed during implementation
- [ ] PR review TODO(phase-3) markers at engine.rs:80 and engine.rs:103 are removed
- [ ] PageContext Phase-3 TODO at page_context.rs:50 is removed (page-context wiring is now real)
- [ ] DeclassExemption Phase-3 TODO at page_context.rs:216–225 is left in place — it is duration-aware exemption ordering, which is independent and out of scope for Phase 3

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Span plumbing through `parse_marking_string` is more invasive than estimated, breaking existing parser tests | Medium | High | Land Tasks 4–5 in a single commit, run the existing parser test suite at every step. Revert and re-think before adding rules. |
| Adding `RuleContext.source: &[u8]` ripples through every `Rule::check` signature | High (if Option (a) chosen) | High | Choose Option (b) — index-based parallel arrays — for E001/E002/E005. Use the build-time `MIGRATIONS_BY_PATTERN` table for E007. |
| The scanner page-break heuristic produces false positives on prose paragraphs | Medium | Medium | Use only the form-feed `\f` byte initially; document the `\n\n\n+` heuristic as a follow-up. The prose corpus does NOT contain `\f`. |
| `(S)` mid-sentence in prose triggers `Classification::Secret` and the prose corpus produces unwanted diagnostics | High | High | Add a parser-level lookahead in Task 1: reject portion candidates whose preceding/following byte is a lowercase ASCII letter. Validate against `tests/corpus/prose/article.txt` before merge. |
| `additionalProperties: false` in `contracts/diagnostic.json` makes the JSON renderer brittle to future field additions | Low | Low | Use a typed `#[derive(Serialize)]` struct mirroring the schema. Add a doc-comment linking to the schema file path so future contributors update both. |
| `insta` snapshots drift on every minor span change | Medium | Low | Use `insta::assert_json_snapshot!` with `redactions` for non-stable fields (line numbers, file paths). Snapshots pin shape, not values. |
| `cargo insta` is not installed in CI | Medium | Low | Add `cargo insta test` to `scripts/check.sh`; document the installation step in `CONTRIBUTING.md`. The `insta` crate works with plain `cargo test` and just produces `*.snap.new` files; CI fails on those via the `insta` env var. |
| Per-rule fixture authoring is error-prone for byte spans | High | Medium | Author fixtures *after* Tasks 4–5 land (real spans available), use `marque check --format json` to generate the `.expected.json` files automatically, then commit both the fixture and the generated golden file in the same commit. |
| W001 has no real migration entries to fire on | High | Low | Use a synthetic migration entry in the integration test only. Document the gap in the W001 fixture's README. Real W001 entries land when ODNI deprecates a marking that remains valid — out of scope for Phase 3. |

## Notes

- **PR #3 review items inherited into Phase 3**: All HIGH/MEDIUM/LOW
  findings (H-1, H-2, H-3, M-1 through M-5, L-1 through L-4, O-1) are
  already resolved in commit `304a15f` and are NOT in Phase 3 scope.
  The only PR-3 review items Phase 3 inherits are the **TODO(phase-3)
  markers in code** that the review explicitly noted as known
  Phase-3 obligations:
  1. `crates/marque-engine/src/engine.rs:80` — reset PageContext at
     page boundaries (Tasks 1, 2)
  2. `crates/marque-engine/src/engine.rs:103` — populate `RuleContext.zone`
     and `position` (Task 3)
  3. `crates/marque-rules/src/lib.rs:132–143` — `page_context: None`
     is now populated (already done in Phase 2; Phase 3 closes the
     reset gap)
  4. `crates/marque-capco/src/rules.rs:88,172,201,236` — `Span::new(0, 0)`
     placeholders (Tasks 4, 5, 7)
  5. `crates/marque-ism/src/page_context.rs:50` — TODO Phase 3
     wiring (Task 2)
  6. `crates/marque-ism/src/page_context.rs:216–225` — duration-aware
     `DeclassExemption` ordering — **deferred**, NOT Phase 3 scope.
     Document this in Task 21's CLAUDE.md update.

- **The "should not block Phase 3" findings from PR #3 review are
  CLOSED**. H-1 (BatchError forwarding), H-3 (INFINITY test), all
  MEDIUM/LOW findings landed in commit `304a15f`. This plan does NOT
  re-address them.

- **Why E007's pattern matching needs extra care**: The current
  `MIGRATIONS` table key for the X-shorthand patterns is `"25X1-"`
  (with trailing dash), but the parser strips trailing punctuation when
  it parses `DeclassExemption`. The Task 9 alternative — emitting a
  parallel `MIGRATIONS_BY_PATTERN` table from `build.rs` keyed by exact
  ASCII pattern — sidesteps this by working on the original source
  bytes via `attrs.token_spans`, not the parsed enum value. This is
  the recommended path.

- **Why W001 needs a synthetic migration entry**: The seed
  `MIGRATIONS` table contains only error-severity deprecations
  (`LIMDIS`, `FOUO`, `NF`, `25X1-`, `50X1-`). A real W001 trigger
  requires a marking that ODNI has flagged as "still valid but
  superseded", which the Phase-2 `build.rs` does not yet emit. The
  rule must exist for the test suite, and the test must use a
  synthetic entry to drive it. When Phase 4 or Phase 5 expands the
  migration table, W001 starts firing on real corpus fixtures
  automatically.

- **Why the prose corpus must come from a public-domain source**:
  The corpus repository is committed to source control. Any fixture
  derived from copyrighted material is a license risk. Project
  Gutenberg or Federal Register text are safe defaults. Document the
  source URL in `tests/corpus/CORPUS_PROVENANCE.md`.

- **Why `--explain-config` exists in Phase 3 and not Phase 4**: Phase
  3 is the first phase where the merged `Configuration` actually
  affects rule behavior at the user level (severity overrides). The
  diagnostic surface — "why is rule X firing as error?" — is needed
  by Phase 3's first real users. Phase 4 inherits the flag for `fix`
  but doesn't add to it.

- **Anti-scope-creep reminder**: Do NOT touch `marque-extract`,
  `marque-server`, `marque-wasm`, or the `fix` subcommand body during
  Phase 3. Each one is a separate phase. The shared `Engine` API surface
  is finalized in Phase 2 and Phase 3 only consumes it.
