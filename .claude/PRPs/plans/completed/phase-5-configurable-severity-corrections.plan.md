<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Plan: Phase 5 — Configurable Rule Severity and Corrections Map (US3)

## Summary
Implement User Story 3: configurable per-rule severity overrides, an organization-specific corrections map with rule C001, classifier-identity plumbing into audit records, and comprehensive test suites verifying the four-layer config precedence chain, hard-fail validators, and SC-006 classifier-id-in-commits guard.

## User Story
As a program office administrator,
I want to commit a `.marque.toml` overriding rule severities and adding a corrections map,
So that Marque matches our organizational policy without requiring code changes, while keeping classifier identity safely in gitignored local config.

## Problem → Solution
Rules fire at hardcoded default severities and there's no corrections map rule → Config overrides are surfaced through the engine, C001 scans tokens against the corrections map, and classifier identity flows into every audit record.

## Metadata
- **Complexity**: Medium
- **Source PRD**: `specs/001-marque-mvp/spec.md`
- **PRD Phase**: Phase 5 (T052–T060)
- **Estimated Files**: 12–15 files created/modified

---

## UX Design

### Before
```
$ cat .marque.toml
[rules]
E001 = "warn"
[corrections]
SERCET = "SECRET"

$ marque check input.txt
# E001 fires at default severity (fix), not warn
# SERCET is not caught as a typo

$ marque fix input.txt
# No audit records carry classifier_id even when configured
# SERCET passes through unfixed
```

### After
```
$ cat .marque.toml
[rules]
E001 = "warn"
[corrections]
SERCET = "SECRET"

$ marque check input.txt
# E001 fires as "warn" per config override ✓
# C001 fires on "SERCET" with source "CorrectionsMap" ✓

$ marque fix input.txt
# SERCET → SECRET via C001 (confidence 1.0) ✓
# Audit records carry classifier_id from .marque.local.toml or env var ✓
# --explain-config dumps merged config as JSON ✓
```

### Interaction Changes
| Touchpoint | Before | After | Notes |
|---|---|---|---|
| Severity in diagnostics | Always rule default | Config override wins | FR-008 |
| Corrections map | Not wired | C001 emits FixProposal | FR-009, T058 |
| Classifier ID in audit | `None` always | From config/env | T060 |
| `--explain-config` | Exits 64 (stub) | Dumps JSON, exits 0 | T056 |
| SC-006 guard | None | Automated test scan | T055 |

---

## Mandatory Reading

| Priority | File | Lines | Why |
|---|---|---|---|
| P0 | `crates/config/src/lib.rs` | all | Config loading, merging, validation — mostly done |
| P0 | `crates/engine/src/engine.rs` | 66–163 | Engine lint loop with severity override — pattern to extend |
| P0 | `crates/engine/src/engine.rs` | 205–322 | Engine fix_inner — where classifier_id is injected |
| P0 | `crates/capco/src/rules.rs` | 1–65, 890–926 | Rule pattern, CapcoRuleSet::new(), make_fix_diagnostic helper |
| P0 | `crates/rules/src/lib.rs` | 125–156, 340–357 | RuleContext struct, Rule trait — needs corrections field |
| P1 | `marque/src/main.rs` | 1–200 | CLI entry, load_config, run_check/run_fix |
| P1 | `marque/src/render.rs` | all | Audit NDJSON renderer — classifier_id already wired |
| P1 | `specs/001-marque-mvp/tasks.md` | 142–166 | Phase 5 task list (T052–T060) |
| P1 | `specs/001-marque-mvp/spec.md` | 94–126, 286–303 | US3 acceptance scenarios, FR-007..FR-011 |
| P2 | `specs/001-marque-mvp/contracts/cli.md` | 38 | `--explain-config` contract |
| P2 | `crates/capco/tests/rules_us1.rs` | all | Integration test pattern for rules |

---

## Patterns to Mirror

### RULE_IMPLEMENTATION
```rust
// SOURCE: crates/capco/src/rules.rs:70-137
struct BannerAbbreviationRule;

impl Rule for BannerAbbreviationRule {
    fn id(&self) -> RuleId { RuleId::new("E001") }
    fn name(&self) -> &'static str { "banner-abbreviation" }
    fn default_severity(&self) -> Severity { Severity::Fix }
    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        // ... iterate token_spans, emit make_fix_diagnostic
    }
}
```

### RULE_REGISTRATION
```rust
// SOURCE: crates/capco/src/rules.rs:39-54
impl CapcoRuleSet {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(BannerAbbreviationRule),
                // ... more rules ...
            ],
        }
    }
}
```

### FIX_DIAGNOSTIC_HELPER
```rust
// SOURCE: crates/capco/src/rules.rs:893-926
struct FixDiagnosticParams {
    rule: RuleId, severity: Severity, source: FixSource,
    span: Span, message: String, citation: &'static str,
    original: String, replacement: String, confidence: f32,
    migration_ref: Option<&'static str>,
}
fn make_fix_diagnostic(p: FixDiagnosticParams) -> Diagnostic { ... }
```

### ENGINE_SEVERITY_OVERRIDE
```rust
// SOURCE: crates/engine/src/engine.rs:137-158
for rule_set in &self.rule_sets {
    for rule in rule_set.rules() {
        let configured_severity = self.config.rules.overrides
            .get(rule.id().as_str())
            .and_then(|s| Severity::parse_config(s))
            .unwrap_or(rule.default_severity());
        if configured_severity == Severity::Off { continue; }
        let mut diags = rule.check(&parsed.attrs, &ctx);
        for d in &mut diags { d.severity = configured_severity; }
        diagnostics.extend(diags);
    }
}
```

### CONFIG_LOADING
```rust
// SOURCE: crates/config/src/lib.rs:211-253
pub fn load(start: &std::path::Path) -> Result<Config, ConfigError> {
    // Layer 1+2: walk upward for project + local config
    // Layer 3: environment variables
    // Hard-fail validators run after merging
}
```

### TEST_PATTERN_ENGINE
```rust
// SOURCE: crates/engine/tests/fix_pipeline.rs:19-25
fn test_engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
}
```

### TEST_PATTERN_CLI
```rust
// SOURCE: marque/tests/cli_fix.rs:15-17
fn marque() -> Command {
    Command::cargo_bin("marque").expect("marque binary")
}
```

---

## Files to Change

| File | Action | Justification |
|---|---|---|
| `crates/rules/src/lib.rs` | UPDATE | Add `corrections: Option<Arc<HashMap<String, String>>>` to RuleContext |
| `crates/engine/src/engine.rs` | UPDATE | Pass corrections from config into RuleContext; already wires classifier_id (T060 verified) |
| `crates/capco/src/rules.rs` | UPDATE | Add C001 `CorrectionsMapRule` implementation |
| `crates/capco/src/lib.rs` | UPDATE | Register C001 in CapcoRuleSet::new() (noop — rules.rs self-registers) |
| `marque/src/main.rs` | UPDATE | Wire `--explain-config` JSON output (currently exits 64 as stub) |
| `crates/config/tests/precedence.rs` | CREATE | T052+T053: precedence chain + hard-fail tests |
| `crates/capco/tests/corrections_map.rs` | CREATE | T054: corrections-map precedence over built-in rules |
| `tests/no_classifier_id_in_commits.rs` | CREATE | T055: SC-006 automated classifier-id scan |
| `tests/corpus_provenance.rs` | CREATE | T055a: SC-002a corpus provenance scan |
| `tests/corpus/invalid/corrections_typo.txt` | CREATE | Corpus fixture: `SERCET//NF` |
| `tests/corpus/invalid/corrections_typo.expected.json` | CREATE | Expected diagnostics for corrections fixture |
| `tests/corpus/invalid/corrections_typo.expected_fix.json` | CREATE | Expected fix result for corrections fixture |
| `marque/tests/cli_config.rs` | CREATE | CLI integration tests for `--explain-config` and config-driven severity |

## NOT Building

- `--explain-config` for the `fix` subcommand (mutually exclusive per contract)
- Incremental cache (Phase 7+)
- WASM config loading (Phase 6)
- Config file creation/init command
- Rule-specific config options beyond severity override

---

## Step-by-Step Tasks

### Task 1: Add corrections map to RuleContext (T056 partial, T058 prereq)
- **ACTION**: Add a `corrections` field to `RuleContext` in `marque-rules` so rules can access the config corrections map
- **IMPLEMENT**: Add `pub corrections: Option<std::sync::Arc<std::collections::HashMap<String, String>>>` to `RuleContext`. This field is `Option` so existing code constructing `RuleContext` without corrections still compiles. The `Arc` avoids cloning the map for every candidate.
- **MIRROR**: Same pattern as `page_context: Option<Arc<PageContext>>` already on RuleContext
- **IMPORTS**: `use std::collections::HashMap;` (already imported via `std::sync::Arc`)
- **GOTCHA**: `RuleContext` derives `Clone` — `Arc<HashMap>` is cheap to clone, but must add `Clone` bound if not already present (it is — HashMap implements Clone). Also update all existing `RuleContext { ... }` construction sites to include `corrections: None` (or the real map).
- **VALIDATE**: `cargo check --workspace` — all existing RuleContext constructors must compile

### Task 2: Wire corrections map from Config through Engine (T056 partial)
- **ACTION**: In `Engine::lint()`, wrap `self.config.corrections` in an `Arc` once and pass it into every `RuleContext`
- **IMPLEMENT**: At the top of `Engine::lint()`, create `let corrections_arc = if self.config.corrections.is_empty() { None } else { Some(Arc::new(self.config.corrections.clone())) };`. Then when constructing `RuleContext`, set `corrections: corrections_arc.clone()`.
- **MIRROR**: ENGINE_SEVERITY_OVERRIDE pattern — context is built per-candidate but the Arc is shared
- **IMPORTS**: None new — `Arc` is already imported in engine.rs
- **GOTCHA**: The clone is of the Arc pointer, not the HashMap. Only one HashMap allocation per lint() call.
- **VALIDATE**: `cargo test --workspace` — all existing tests pass (corrections is None by default)

### Task 3: Implement C001 corrections-map rule (T058)
- **ACTION**: Add `CorrectionsMapRule` to `crates/capco/src/rules.rs`
- **IMPLEMENT**: 
  - Zero-size struct `CorrectionsMapRule`
  - `id()` → `RuleId::new("C001")`
  - `name()` → `"corrections-map"`
  - `default_severity()` → `Severity::Fix`
  - `check()`: if `ctx.corrections` is None, return empty. Otherwise iterate `attrs.token_spans` and for each token whose `text` (case-sensitive) matches a key in the corrections map, emit a `Diagnostic` with `FixProposal { source: FixSource::CorrectionsMap, confidence: 1.0, migration_ref: Some("corrections-map") }`. The span is `token_span.span`, original is `token_span.text`, replacement is the map value.
- **MIRROR**: RULE_IMPLEMENTATION + FIX_DIAGNOSTIC_HELPER patterns
- **IMPORTS**: Same as existing rules — `use marque_ism::{IsmAttributes, Span, TokenKind, TokenSpan};`
- **GOTCHA**: FR-009 requires corrections to win over built-in rules on same span. This is automatic: FR-016 sorts `(span.end DESC, span.start DESC, rule_id ASC, replacement ASC)` and `"C001" < "E001"` lexicographically, so C001 wins under the C-1 overlap guard. No special code needed.
- **GOTCHA**: The corrections map keys may match tokens that are already valid (e.g., user adds `"SECRET" = "TOP SECRET"`). This is intentional — the user's corrections map is authoritative.
- **GOTCHA**: Only match on the token's source text (`token_span.text`), not the parsed enum value. A typo like "SERCET" won't parse to a valid enum — it will appear in token_spans as `TokenKind::Unknown` or `TokenKind::Classification` depending on scanner behavior.
- **VALIDATE**: `cargo check -p marque-capco` compiles; add unit test in rules.rs `mod tests`

### Task 4: Register C001 in CapcoRuleSet (T058a)
- **ACTION**: Add `CorrectionsMapRule` to the rules vec in `CapcoRuleSet::new()`
- **IMPLEMENT**: Add `Box::new(CorrectionsMapRule),` after the W001 entry
- **MIRROR**: RULE_REGISTRATION pattern
- **GOTCHA**: The existing test `capco_rule_set_registers_all_phase3_rules` checks specific rule IDs — add `assert!(ids.contains(&"C001"));`
- **VALIDATE**: `cargo test -p marque-capco -- capco_rule_set` passes

### Task 5: Wire `--explain-config` JSON output (T056 partial)
- **ACTION**: Implement the `--explain-config` flag per `contracts/cli.md`: dump merged config as JSON to stdout and exit 0
- **IMPLEMENT**: In `run_check()` (and `run_fix()` mutual exclusion is already enforced), when `common.explain_config` is true: load config, build a JSON object with `{ rules: { "E001": "fix", ... }, corrections: ["SERCET", ...], confidence_threshold: 0.95, schema_version: "ISM-v2022-DEC", classifier_id_present: true/false }`, write to stdout, return `EX_OK`. The classifier_id *value* must never be exposed — only a boolean.
- **MIRROR**: CONFIG_LOADING pattern for load_config
- **IMPORTS**: `serde_json`
- **GOTCHA**: `--explain-config` is already defined on `CommonOptions` and the mutual exclusion with paths is handled. Currently it's stubbed to exit 64 in `run_check`. Need to replace the stub with actual implementation. For `fix`, the mutual exclusion with `--explain-config` is already checked.
- **VALIDATE**: `cargo run -p marque -- check --explain-config` outputs valid JSON and exits 0

### Task 6: Verify classifier_id flows into audit records (T060)
- **ACTION**: Verify that `classifier_id` from config already flows through `Engine::fix` into `AppliedFix` and then into the NDJSON audit renderer
- **IMPLEMENT**: This is already done — `engine.rs:255-260` reads `self.config.user.classifier_id` and passes it as `Option<Arc<str>>` to `AppliedFix::__engine_promote`. The NDJSON renderer in `render.rs` already emits `classifier_id: null` or the string value. Write a CLI integration test to confirm end-to-end: set `MARQUE_CLASSIFIER_ID=TEST-42`, run `marque fix`, verify audit NDJSON contains `"classifier_id":"TEST-42"`.
- **MIRROR**: TEST_PATTERN_CLI
- **VALIDATE**: CLI integration test passes

### Task 7: Write config precedence tests (T052)
- **ACTION**: Create `crates/config/tests/precedence.rs` testing the four-layer precedence chain
- **IMPLEMENT**: 
  - Test: `.marque.toml` sets E001=warn, no local/env/CLI → E001 severity is warn
  - Test: `.marque.toml` sets E001=warn, `.marque.local.toml` sets nothing (no [rules] in local) → E001 is warn
  - Test: `MARQUE_CONFIDENCE_THRESHOLD=0.5` overrides `.marque.toml` threshold
  - Test: `MARQUE_CLASSIFIER_ID=env-id` overrides `.marque.local.toml` classifier_id
  - Each test creates a tempdir with config files and calls `marque_config::load()`
- **MIRROR**: CONFIG_LOADING pattern + existing discover_* tests in lib.rs
- **IMPORTS**: `tempfile`, `std::fs`, `marque_config`
- **GOTCHA**: Env var tests must use unique var names or serialize (env vars are process-global). Use `std::env::set_var` carefully with cleanup, or scope tests to avoid conflicts. Consider using `temp_env` crate or manual cleanup.
- **GOTCHA**: Must write a `.marque.toml` that passes schema version validation — include `[capco]\nversion = "ISM-v2022-DEC"` or omit [capco] to get the default.
- **VALIDATE**: `cargo test -p marque-config --test precedence`

### Task 8: Write hard-fail scenario tests (T053)
- **ACTION**: Add hard-fail tests to `crates/config/tests/precedence.rs`
- **IMPLEMENT**:
  - Test: `.marque.toml` with `[user]` section → `ConfigError::UserSectionInCommitted`, exit code 65
  - Test: `.marque.toml` with `[capco] version = "WRONG"` → `ConfigError::SchemaVersionMismatch`, exit code 65
  - Test: `.marque.toml` with `confidence_threshold = 2.0` → `ConfigError::ThresholdOutOfRange`, exit code 65
- **MIRROR**: Existing `exit_code_matches_contract` test in config/src/lib.rs
- **VALIDATE**: `cargo test -p marque-config --test precedence`

### Task 9: Write corrections-map precedence tests (T054)
- **ACTION**: Create `crates/capco/tests/corrections_map.rs`
- **IMPLEMENT**:
  - Test: Built-in E001 fires on "NF" AND corrections map has "NF"="NOFORN" → C001 wins (its FixProposal has rule C001 and source CorrectionsMap). Verify via `engine.fix()` that the applied fix has `rule: "C001"` and `source: CorrectionsMap`.
  - Test: Corrections map entry with no built-in rule match → C001 fires independently
  - Test: Empty corrections map → C001 produces no diagnostics
  - Use `Engine::with_clock()` and a config with corrections populated
- **MIRROR**: TEST_PATTERN_ENGINE
- **IMPORTS**: `marque_capco::capco_rules`, `marque_config::Config`, `marque_engine::{Engine, FixMode, FixedClock}`
- **GOTCHA**: For the precedence test, both E001 and C001 will emit proposals for the same span. FR-016 sort + C-1 overlap guard determines which one wins. C001 < E001 lexicographically, so C001 is kept. Verify the `applied[0].proposal.source` is `FixSource::CorrectionsMap`.
- **VALIDATE**: `cargo test -p marque-capco --test corrections_map`

### Task 10: Create corpus fixtures for corrections (T054 support)
- **ACTION**: Add test corpus fixtures for corrections-map testing
- **IMPLEMENT**:
  - `tests/corpus/invalid/corrections_typo.txt`: `SERCET//NF\n`
  - `tests/corpus/invalid/corrections_typo.expected.json`: diagnostics for C001 (SERCET→SECRET) and E001 (NF→NOFORN). The C001 diagnostic has `source: "CorrectionsMap"`.
  - `tests/corpus/invalid/corrections_typo.expected_fix.json`: `{ "text": "SECRET//NOFORN\n", "applied": [C001, E001], "remaining": [] }`
- **MIRROR**: Existing `mixed_confidence.txt` fixture pattern
- **GOTCHA**: The corpus harness in `rules_us1.rs` won't automatically test corrections fixtures because the default config has no corrections. These fixtures are for the dedicated `corrections_map.rs` integration tests that construct a config with corrections.
- **VALIDATE**: Files exist and are valid JSON

### Task 11: Write SC-006 classifier-id scan (T055)
- **ACTION**: Create `tests/no_classifier_id_in_commits.rs`
- **IMPLEMENT**: 
  - Scan all files under `tests/corpus/`, `crates/*/tests/`, `crates/*/examples/` for strings matching classifier-id patterns: numeric IDs (5+ digits), "classifier_id" literal with a non-test value, common PII-shaped strings.
  - Pattern: regex like `\b\d{5,}\b` (but exclude line numbers, timestamps, byte offsets which are common in test fixtures). More targeted: look for `classifier_id.*=.*"[^"]*\d{5}"` patterns, or anything matching `"classifier_id":"[^"]+"` in JSON where the value isn't "null" or a test sentinel.
  - Simpler approach per spec: just scan for any string that looks like a real classifier ID. Use an allowlist for known test sentinels (e.g., "TEST-CLASSIFIER-42" used in fix_pipeline.rs).
- **MIRROR**: Standard Rust integration test with `walkdir` or `glob` + `std::fs::read_to_string`
- **IMPORTS**: `glob` or manual directory walking
- **GOTCHA**: Must not flag test sentinels like "TEST-CLASSIFIER-42" — these are clearly not real IDs. The scan should look for patterns like `\d{5,}` that appear in `classifier_id` context, or just verify no file contains `classifier_id` followed by a value that isn't a known test sentinel.
- **VALIDATE**: `cargo test --test no_classifier_id_in_commits`

### Task 12: Write corpus provenance scan (T055a)
- **ACTION**: Create `tests/corpus_provenance.rs`
- **IMPLEMENT**:
  - (a) Every file under `tests/corpus/` matches registered patterns: `invalid/*.txt`, `invalid/*.expected.json`, `invalid/*.expected_fix.json`, `valid/*.txt`, `valid/*.expected.json`, `CORPUS_CONTRACT.md`, `CORPUS_PROVENANCE.md`, `README.md`
  - (b) `CORPUS_PROVENANCE.md` exists and contains "Reviewer:" or similar attestation line
  - (c) No fixture contains classifier-id-shaped strings (reuse T055 scanner logic — extract a shared helper)
  - (d) No fixture contains token strings outside generated CVE enumerations (use `marque_ism::generated::values` to build allowlist)
- **MIRROR**: Standard workspace-level integration test
- **GOTCHA**: Criterion (d) requires importing `marque_ism::generated::values` — need to check what's exported. If not all CVE token strings are accessible, may need to add a public accessor.
- **VALIDATE**: `cargo test --test corpus_provenance`

### Task 13: Write CLI integration tests for config (T056 support)
- **ACTION**: Create `marque/tests/cli_config.rs` with CLI integration tests for configuration behavior
- **IMPLEMENT**:
  - Test: `--explain-config` outputs valid JSON with expected fields and exits 0
  - Test: severity override in `.marque.toml` changes diagnostic severity in output
  - Test: `MARQUE_CLASSIFIER_ID=TEST-42` appears in audit NDJSON during `fix`
  - Test: corrections map entry produces C001 diagnostic
  - Use tempdir with `.marque.toml` files, pass `--config` flag to point at them
- **MIRROR**: TEST_PATTERN_CLI from cli_fix.rs
- **VALIDATE**: `cargo test -p marque --test cli_config`

---

## Testing Strategy

### Unit Tests

| Test | Input | Expected Output | Edge Case? |
|---|---|---|---|
| C001 empty corrections | `SECRET//NF`, no corrections | No C001 diagnostics | Yes |
| C001 single match | `SERCET//NF`, corrections: SERCET→SECRET | C001 diagnostic, CorrectionsMap source | No |
| C001 no match | `SECRET//NF`, corrections: SERCET→SECRET | No C001 diagnostics | Yes |
| C001 multiple matches | `SERCET//NF SERCET`, corrections: SERCET→SECRET | Multiple C001 diagnostics | No |
| C001 case sensitive | `sercet//NF`, corrections: SERCET→SECRET | No C001 match (case mismatch) | Yes |
| FR-009 precedence | Same span, E001 and C001 both fire | C001 wins in applied fixes | No |
| Severity override warn | Config E001=warn | Diagnostic.severity == Warn | No |
| Severity override off | Config E001=off | No E001 diagnostics | No |
| Precedence: env > local | Both set classifier_id | Env value wins | No |
| Hard-fail: [user] in committed | .marque.toml with [user] | ConfigError::UserSectionInCommitted | No |

### Edge Cases Checklist
- [x] Empty corrections map → C001 produces nothing
- [x] Corrections key matches multiple tokens → multiple diagnostics
- [x] Corrections key that matches a valid token (e.g., "SECRET" → "TOP SECRET") → fires anyway
- [x] Both C001 and built-in rule match same span → C001 wins per FR-009/FR-016
- [x] All rules set to "off" → empty diagnostic stream
- [x] classifier_id as empty string in local config → treated as not set (L-2 invariant)
- [x] Env var `MARQUE_CONFIDENCE_THRESHOLD` with invalid float → exit 65
- [x] `--explain-config` → JSON output, exit 0, classifier_id value never exposed

---

## Validation Commands

### Static Analysis
```bash
cargo clippy --workspace -- -D warnings
```
EXPECT: Zero warnings

### Unit Tests
```bash
cargo test --workspace
```
EXPECT: All tests pass, count rises from 181 to ~210+

### Config Tests
```bash
cargo test -p marque-config --test precedence
```
EXPECT: All precedence and hard-fail tests pass

### Corrections Tests
```bash
cargo test -p marque-capco --test corrections_map
```
EXPECT: FR-009 precedence and corrections behavior verified

### CLI Integration Tests
```bash
cargo test -p marque --test cli_config
```
EXPECT: --explain-config, severity override, classifier_id, corrections all work end-to-end

### Guard Tests
```bash
cargo test --test no_classifier_id_in_commits
cargo test --test corpus_provenance
```
EXPECT: No classifier IDs in committed files, corpus provenance valid

### Full Suite
```bash
cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --check
```
EXPECT: All green

---

## Acceptance Criteria
- [ ] Severity override in `.marque.toml` changes diagnostic severity (FR-008, SC-007)
- [ ] `[corrections]` map entries fire C001 with `source: CorrectionsMap` (FR-009)
- [ ] C001 wins over built-in rules on same span (FR-009 precedence)
- [ ] Classifier ID from env/local config appears in audit NDJSON (T060)
- [ ] `--explain-config` dumps merged config JSON, never exposes classifier_id value
- [ ] `[user]` in `.marque.toml` exits 65 (FR-010, SC-006)
- [ ] Schema version mismatch exits 65 (FR-011)
- [ ] SC-006 scan passes (no classifier IDs in committed test files)
- [ ] SC-002a corpus provenance scan passes
- [ ] All 4-layer precedence chain behavior tested (T052)
- [ ] All validation commands pass with zero errors

## Completion Checklist
- [ ] Code follows discovered patterns (rule impl, diagnostic helper, test patterns)
- [ ] Error handling matches codebase style (ConfigError variants, exit codes)
- [ ] Tests follow test patterns (tempdir setup, assert_cmd for CLI)
- [ ] No hardcoded values (corrections come from config, not code)
- [ ] No unnecessary scope additions
- [ ] Self-contained — no questions needed during implementation

## Risks
| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Env var test isolation (process-global vars) | Medium | Test flakiness | Use unique env var names or serialize tests; document cleanup pattern |
| C001 token matching on unknown/partially-parsed tokens | Low | Missed corrections | The scanner emits candidates; parser may fail. C001 operates on token_spans which are populated by the parser — if parser fails, no token_spans exist. Corrections that target fully unparseable text won't fire. Document this limitation. |
| `--explain-config` exposes classifier_id accidentally | Low | SC-006 violation | Emit `classifier_id_present: bool`, never the value. Add test asserting the value is absent. |
| Existing tests break from RuleContext field addition | Low | Build failure | Use `corrections: None` default at all existing construction sites |

## Notes

### What's Already Done (verified in codebase)
- Config loading with 4-layer precedence (T022): ✅ complete
- Hard-fail validators (T023): ✅ complete (but need integration test coverage — T053)
- Severity override in engine lint loop: ✅ complete (engine.rs:137-158)
- Classifier ID injection in fix_inner: ✅ complete (engine.rs:255-260)
- Audit NDJSON renderer handles classifier_id: ✅ complete (render.rs)
- Corrections HashMap in Config struct: ✅ exists (config.rs:89), parsed from TOML (config.rs:339)
- Env var loading (MARQUE_CLASSIFIER_ID, MARQUE_CONFIDENCE_THRESHOLD): ✅ complete (config.rs:375-391)

### What's New
- `RuleContext.corrections` field
- `CorrectionsMapRule` (C001) implementation
- `--explain-config` JSON output (replace stub)
- 4 new test files (precedence.rs, corrections_map.rs, no_classifier_id_in_commits.rs, corpus_provenance.rs)
- CLI config integration tests
- 3 corpus fixtures

### Key Architectural Decision
C001 accesses the corrections map via `RuleContext.corrections: Option<Arc<HashMap<String, String>>>`. This preserves the rule-is-stateless invariant (the map comes from context, not rule state) and is consistent with how `page_context` is already passed via `Option<Arc<T>>`. The alternative of making C001 hold a reference at construction time was rejected because rules are constructed once in `CapcoRuleSet::new()` and don't have access to config at that point.
