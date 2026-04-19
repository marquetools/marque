<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Local Review: Phase 2 (Foundational) — marque MVP

**Reviewed**: 2026-04-10
**Branch**: 001-marque-mvp (uncommitted)
**Scope**: 24 modified files + 1 new (`clock.rs`), ~1262 insertions / 425 deletions
**Decision**: **REQUEST CHANGES** — ship after addressing CRITICAL and HIGH findings

## Summary

Phase 2 lands the real CVE XML→Rust codegen pipeline, the FixProposal/AppliedFix type split, the FR-016 deterministic fix ordering, and the Clock-injected engine. The architectural shape matches the spec. Validation is clean (`cargo clippy -D warnings`, `cargo test --workspace`, `cargo fmt --check` all pass). What remains are correctness and safety holes that the compiler cannot see: silent config fallbacks, panics in WASM-safe library code, a missing overlap guard that Phase 3 will activate into a data-corruption risk, and a server that bypasses the entire config system.

Three independent Rust reviewers produced converging findings across three review scopes (marque-ism codegen, rules/engine/capco, config/integration). This report consolidates them and de-duplicates overlap.

## Validation Results

| Check | Result |
|---|---|
| Type check (`cargo check`) | Pass |
| Lint (`cargo clippy -- -D warnings`) | Pass — zero warnings |
| Format (`cargo fmt --check`) | Pass |
| Tests (`cargo test --workspace`) | Pass — 3 scanner tests + 1 doctest; **zero tests in marque-ism, marque-rules, marque-engine, marque-capco, marque-config** |
| Build | Pass |

## Findings

### CRITICAL

**C-1 — Missing overlap/adjacency guard in `Engine::fix`**
`crates/engine/src/engine.rs:118–138`

FR-016 sorts fixes reverse-by-end to preserve earlier offsets, but there is no check that two fixes overlap. Phase 2 hides this because every fix proposal currently has a `Span::new(0, 0)` placeholder and is filtered by `!f.span.is_empty()` at line 113. Once Phase 3 wires real spans, two rules emitting fixes for the same byte range will silently overwrite each other via `output.splice`. This is a latent data-corruption bug.

```rust
// After sorting, drop fixes that overlap a previously kept fix:
fixes.dedup_by(|b, a| b.span.start < a.span.end);
```

Even if Phase 3 is expected to address this, a `TODO(phase-3)` with `debug_assert!` coverage is the minimum required to prevent regression.

---

**C-2 — `MARQUE_CONFIDENCE_THRESHOLD` silently discards parse failures**
`crates/config/src/lib.rs:255–258`

```rust
if let Ok(val) = std::env::var("MARQUE_CONFIDENCE_THRESHOLD") {
    if let Ok(threshold) = val.parse::<f32>() {
        config.set_confidence_threshold(threshold)?;
    }
}
```

The inner `if let Ok` drops typos like `"0.9o"` or locale values like `"0,9"` with no diagnostic. The surrounding file advertises hard-fail validators (comment: "exit 65"). This silently applies the default 0.95 threshold and is a direct contract violation. Propagate the parse error through `ConfigError`.

---

**C-3 — Generated `validators.rs` imports a type that does not exist in `generated::values`**
`crates/ism/build.rs:520–525`

```rust
use super::values::{Classification, SciControl, DissemControl};
```

`Classification` is hand-written in `attrs.rs` and is **not** emitted into `values.rs`. This will fail to compile the first time `generated::validators` is used by any downstream consumer. The fact that clippy is currently passing means the validator module is compiled but not referenced in any use site that exercises the path resolution. Either change the `use` to `crate::attrs::Classification` or rewrite the validator to a literal `matches!` so no import is needed.

---

**C-4 — `to_rust_ident` can produce empty strings and duplicate variant names**
`crates/ism/build.rs:152–179`

Two distinct CVE values can collapse to the same Rust identifier (e.g., `"RS"` and `"R-S"` both → `Rs`). Values consisting entirely of separators produce `""` which emits `pub enum X { , }` and fails to compile. The function has no dedup guard.

```rust
let mut seen = std::collections::HashSet::new();
for (value, desc) in entries {
    let ident = to_rust_ident(value);
    assert!(!ident.is_empty(), "empty ident for CVE value {value:?}");
    assert!(seen.insert(ident.clone()), "duplicate ident {ident:?}");
    // emit variant
}
```

Today's CVE schema may happen to avoid collisions, but the next ODNI package update could silently break the build or, worse, emit a silently wrong mapping.

---

### HIGH

**H-1 — Server bypasses the entire config system**
`crates/server/src/main.rs:160`

```rust
let config = Config::default();
```

No `.marque.toml` loaded, no `MARQUE_CONFIDENCE_THRESHOLD`, no `MARQUE_CLASSIFIER_ID`, and — most important — **no schema-version hard-fail check (FR-011)**. The server never runs the validator the spec treats as mandatory. Call `marque_config::load()` and fail startup with the documented exit code on error.

---

**H-2 — Server `FixRequest::confidence_threshold` field is dead**
`crates/server/src/main.rs:71–75, 136–149`

The field is serialized in the API struct (with `#[allow(dead_code)]` as a tell) but `fix_handler` passes the engine's compiled-in config threshold unconditionally. Callers setting `"confidence_threshold": 0.7` see no effect. Either plumb it (engine needs a per-call threshold), or remove the field and document that the server is configured statically.

---

**H-3 — CLI `--confidence` flag is dead**
`marque/src/main.rs:118, 131`

```rust
fn run_fix(engine: &Engine, files: &[PathBuf], dry_run: bool, _confidence: f32) -> i32 {
```

The `_` prefix signals deliberate non-use. The flag is advertised to operators but has no effect. Same category as H-2 — advertised control that is not delivered. Either plumb the value into config before engine construction, add a per-call threshold override method on `Engine`, or remove the flag.

---

**H-4 — `current_dir().unwrap()` in the CLI main path**
`marque/src/main.rs:70`

```rust
let config = marque_config::load(std::env::current_dir().unwrap().as_path()).unwrap_or_else(...);
```

`current_dir()` fails in chroots, sandboxes, and when the cwd has been deleted. The outer `unwrap_or_else` only catches `ConfigError` — this panics with a Rust backtrace instead of the documented exit 74 (EX_IOERR). Handle explicitly.

---

**H-5 — `Trigraph::as_str()` panics in WASM-safe library code**
`crates/ism/src/attrs.rs:94–97`

```rust
pub fn as_str(&self) -> &str {
    std::str::from_utf8(&self.0).expect("Trigraph bytes must be valid ASCII")
}
```

`Trigraph(pub [u8; 3])` exposes its inner field publicly. Any caller can construct `Trigraph([0xFF, 0xFF, 0xFF])` and the next `as_str` or `Display` call panics. `marque-ism` is consumed by the WASM target where panic recovery is unavailable. Make the field private and provide `Trigraph::try_new(b: [u8; 3]) -> Option<Self>` that validates ASCII alphabetic, then mark `as_str` as `unsafe { from_utf8_unchecked }` with a `// SAFETY:` invariant comment.

---

**H-6 — Unrecognized rule severity strings in config silently fall back to the default**
`crates/engine/src/engine.rs:72–78`

```rust
.and_then(|s| Severity::parse_config(s))
.unwrap_or(rule.default_severity())
```

`.marque.toml` entries like `banner-abbreviation = "err"` (typo) or `"disable"` (wrong spelling of `off`) silently apply the rule's default severity. The user gets no feedback that their override was ignored. Validate severity strings in `marque-config` at load time — iterate `file.rules` and call `Severity::parse_config`, returning `ConfigError::UnknownSeverity { rule, value }`.

---

**H-7 — O(n²) `applied_fixes.contains` in `remaining_diagnostics`**
`crates/engine/src/engine.rs:107, 159–166`

`Vec<(RuleId, Span)>` with linear `contains` per diagnostic. `RuleId` is `&'static str`; `Span` is `Copy + Eq`. Use a `HashSet` (requires `#[derive(Hash)]` on `Span` — verify it's there). For large documents with many rules this is a real scaling cost.

Additionally the filter clones `f.rule` on every iteration even when `applied_fixes` is empty.

---

**H-8 — `ALL_CVE_TOKENS` contains hardcoded duplicates**
`crates/ism/build.rs:438–445`

After emitting all CVE dissem control tokens from XML, `build.rs` unconditionally appends `NOFORN`, `ORCON`, `PROPIN`, `IMCON` as literal strings — values that are already present from the XML. `canonicalize` still returns correct answers by first-hit, but the duplicates inflate the slice and slow both the linear scan and the `AUTOMATON` construction. Deduplicate before emission.

---

**H-9 — `parse_classification` is hand-coded while the file comment claims the migration is complete**
`crates/core/src/parser.rs:175–208`

```rust
fn parse_classification(s: &str) -> Option<Classification> {
    match s { "TS" | "TOP SECRET" => ..., _ => None }
}
```

Line 208 asserts "Token classification now uses generated CVE enum from_str() methods instead of hard-coded heuristics." That is true for `SciControl`, `DissemControl`, `SarIdentifier`, `DeclassExemption` — but not for `Classification`. When ODNI publishes a schema update, the other types auto-track; this one won't. Either route classification through the generated type (adding abbreviation mapping to codegen) or correct the comment.

---

**H-10 — Zero tests in the core crates**
marque-ism, marque-rules, marque-engine, marque-capco, marque-config all have **no** `#[cfg(test)]` modules.

The build.rs codegen pipeline, `Engine::fix` ordering, `Severity::parse_config`, `FixProposal::new` confidence validation, and `Config::set_confidence_threshold` validators are all entirely untested. A minimum bar before Phase 3 lands real spans:

- Round-trip `parse(as_str(x)) == Some(x)` for every generated enum variant
- `Engine::fix` ordering with synthetic overlapping spans (once the overlap guard is in)
- `Severity::parse_config` for valid, invalid, and case-sensitivity
- `Config::set_confidence_threshold` boundary cases (0.0, 1.0, NaN, -0.1, 1.1)
- `FixedClock` determinism verification

---

### MEDIUM

**M-1 — `FixProposal::confidence` validator is `debug_assert!` only**
`crates/rules/src/lib.rs:134–137`

```rust
debug_assert!(
    (0.0..=1.0).contains(&confidence) && !confidence.is_nan(),
    ...
);
```

Stripped in release. A rule emitting `confidence: f32::INFINITY` or `NaN` passes in production. `NaN` then compares as `>= threshold` always false and the fix silently disappears; `INFINITY` bypasses any threshold. Change to `assert!` or a `Result`-returning constructor. Current callers pass literal constants so the panic path will not fire unexpectedly.

---

**M-2 — `Span::as_slice` panics on out-of-bounds in release**
`crates/ism/src/span.rs:29–31`

`Span::new` only `debug_assert`s `start <= end`. `as_slice` does `&source[self.start..self.end]` which panics on any bound violation in release. Add a checked variant:

```rust
pub fn try_as_slice<'a>(&self, source: &'a [u8]) -> Option<&'a [u8]> {
    source.get(self.start..self.end)
}
```

And upgrade `Span::new` from `debug_assert` to `assert`.

---

**M-3 — `RuleContext` always passes `Zone::Body` / `DocumentPosition::Body`**
`crates/engine/src/engine.rs:64–67`

Hardcoded. Current rules only key off `marking_type`, so this is not a live bug — but it guarantees any future rule that reads `zone` or `position` will silently get wrong answers. Add a `// TODO(phase-3): plumb document structure from scanner` comment at minimum.

---

**M-4 — `classifier_id` is `Box<str>`-cloned per fix in the engine loop**
`crates/engine/src/engine.rs:130–144`

Switch to `Arc<str>` in `AppliedFix` (or share via `Arc` from `Config::user`) to make the per-fix clone O(1). For a large document with many fixes this is avoidable allocation.

---

**M-5 — `batch.rs` `.expect("lint task panicked")` aborts the entire process**
`crates/engine/src/batch.rs:116, 146`

`spawn_blocking` `JoinError` covers both panics and cancellation. Using `.expect` turns any single-document failure into a process-wide abort that loses partial batch results. Propagate `JoinError` through the stream item.

---

**M-6 — `format!("{:?}", severity)` leaks Debug format into JSON**
`crates/server/src/main.rs:118`, `crates/wasm/src/lib.rs:44`

Debug formatting is not a stable API. Derive `Serialize` on `Severity` with `#[serde(rename_all = "lowercase")]` (or implement `Display`) and use that. Better: introduce a shared `DiagnosticJson` type in `marque-engine` so server and WASM do not duplicate.

---

**M-7 — Unused runtime dependencies in `marque-ism`**
`crates/ism/Cargo.toml:11–16`

`thiserror`, `phf`, and `anyhow` are listed as runtime deps but `grep` confirms they are not used in `src/`. They bloat every downstream consumer including WASM (where binary size matters). Remove until actually used. `phf` in particular should be reserved until trigraph lookups are migrated to `phf::Set` (see recommended fix for H-8-adjacent trigraph O(n) scan).

---

**M-8 — `CapcoTokenSet::canonicalize` is O(n)**
`crates/ism/src/token_set.rs:32–34`

Linear scan over `ALL_CVE_TOKENS` per call. The neighboring `AUTOMATON` static is dead-coded with `#[allow(dead_code)]`. Emit `ALL_CVE_TOKENS` sorted from `build.rs` and use `binary_search` for O(log n), or switch to `phf::Set` for O(1). The trigraph `contains` at line 40 has the same issue.

---

**M-9 — `make_fix_diagnostic` has 9 parameters with `#[allow(too_many_arguments)]`**
`crates/capco/src/rules.rs:248`

Extract a `FixProposalParams` struct. Phase 3 will multiply the call sites.

---

**M-10 — Silent XML unescape via `unwrap_or_default` in `parse_cve_xml`**
`crates/ism/build.rs:128`

```rust
current_value.push_str(&e.unescape().unwrap_or_default());
```

Any unescape error silently truncates the value and can feed an empty string into `to_rust_ident` (→ C-4). Panic with a clear message instead — this is a build script, a panic is the correct failure mode.

---

### LOW

**L-1 — `run_metadata` returns exit 0 with a TODO message** — `marque/src/main.rs:158–161`. Should return exit 69 (EX_UNAVAILABLE) so scripts do not trust the stub.
**L-2 — Duplicate doc comment on `Severity::parse_config`** — `crates/rules/src/lib.rs:54–55`.
**L-3 — `FixedClock(pub SystemTime)` exposes inner field** — `crates/engine/src/clock.rs:24`. Prefer `FixedClock::new(t)`.
**L-4 — `RuleId(pub &'static str)` allows arbitrary construction** — `crates/rules/src/lib.rs:25`. Make inner field `pub(crate)` and add `pub const fn new`.
**L-5 — `LintResult::fix_count()` counts `Severity::Fix` diagnostics, not diagnostics with proposals** — `crates/engine/src/output.rs:32–38`. Either rename to `fixable_diagnostic_count` or add `d.fix.is_some()` guard.
**L-6 — `SCHEMA_VERSION` not re-exported at crate root** — `crates/ism/src/lib.rs`. Add `pub use generated::values::SCHEMA_VERSION;`.
**L-7 — `Severity::Off` ordering under `max` merge semantics** — `crates/rules/src/lib.rs:40–51`. `Off < Warn < Error < Fix` means a local config cannot suppress a rule if the project config set it to `Error`. This may be intentional for a security tool; document explicitly.
**L-8 — `writeln!(out, ...).unwrap()` throughout `build.rs`** — Infallible on `String`, but non-idiomatic; return `fmt::Result` and propagate with `?`.
**L-9 — `parse_xsd_trigraphs` does not unescape attribute values** — `crates/ism/build.rs:481`. Safe today because trigraphs are pure ASCII, but inconsistent with `parse_cve_xml` which does unescape.

## Files Reviewed

### Modified (24)
- `Cargo.lock`
- `crates/capco/src/rules.rs`
- `crates/config/Cargo.toml`
- `crates/config/src/lib.rs`
- `crates/core/src/attrs.rs`
- `crates/core/src/parser.rs`
- `crates/core/src/scanner.rs`
- `crates/core/src/span.rs`
- `crates/engine/src/batch.rs`
- `crates/engine/src/engine.rs`
- `crates/engine/src/lib.rs`
- `crates/engine/src/output.rs`
- `crates/engine/src/pipeline.rs`
- `crates/extract/src/lib.rs`
- `crates/ism/Cargo.toml`
- `crates/ism/build.rs`
- `crates/ism/src/attrs.rs`
- `crates/ism/src/lib.rs`
- `crates/ism/src/span.rs`
- `crates/ism/src/token_set.rs`
- `crates/rules/src/lib.rs`
- `crates/server/src/main.rs`
- `crates/wasm/src/lib.rs`
- `marque/src/main.rs`

### Added (1)
- `crates/engine/src/clock.rs`

## Blocking Issues Summary

| # | Severity | File:Line | Issue |
|---|---|---|---|
| C-1 | CRITICAL | engine.rs:118 | Missing overlap guard — Phase 3 data-corruption risk |
| C-2 | CRITICAL | config/lib.rs:255 | `MARQUE_CONFIDENCE_THRESHOLD` parse failure silently ignored |
| C-3 | CRITICAL | build.rs:520 | `validators.rs` imports `Classification` from wrong path |
| C-4 | CRITICAL | build.rs:152 | `to_rust_ident` can produce empty/duplicate variants |
| H-1 | HIGH | server/main.rs:160 | Server bypasses config load (no FR-011 schema check) |
| H-2 | HIGH | server/main.rs:71 | `confidence_threshold` field is dead |
| H-3 | HIGH | marque/main.rs:118 | CLI `--confidence` flag is dead |
| H-4 | HIGH | marque/main.rs:70 | `current_dir().unwrap()` panics instead of exit 74 |
| H-5 | HIGH | attrs.rs:94 | `Trigraph::as_str` panics in WASM-safe library |
| H-6 | HIGH | engine.rs:72 | Unrecognized severity strings silently ignored |
| H-7 | HIGH | engine.rs:159 | O(n²) `remaining_diagnostics` filter |
| H-8 | HIGH | build.rs:438 | Duplicate tokens in `ALL_CVE_TOKENS` |
| H-9 | HIGH | parser.rs:175 | `parse_classification` hand-coded despite comment |
| H-10 | HIGH | all src/ | Zero tests in core crates |

**4 CRITICAL + 10 HIGH = 14 blocking findings.**

## Recommended Remediation Order

1. **C-3** — compile-time ticking bomb; a single `use` fix
2. **H-1** — single-line server config load; satisfies FR-011
3. **C-2, H-6** — config validator gaps; 1 file, ~20 lines
4. **H-4** — cli safety; 1 file, ~10 lines
5. **H-5, L-4** — make `Trigraph` and `RuleId` inner fields private; small blast radius
6. **H-2, H-3** — confidence plumbing or flag removal; design decision first
7. **C-1** — overlap guard with `dedup_by`; 3 lines
8. **C-4, H-8, M-10** — build.rs hardening cluster
9. **H-7** — Vec → HashSet migration
10. **H-9, M-3** — comment corrections and TODO markers
11. **H-10** — test backfill; should run in parallel with everything above

## Decision

**REQUEST CHANGES.** Validation is clean but the correctness and safety holes above should be addressed before Phase 3 begins. Several of the issues (C-1, H-6, C-2) are the kind of silent-fallback bugs that become very hard to diagnose once they ship into real usage of a security-sensitive linter.

Not a BLOCK because every finding is localized and mechanical — there is no architectural rework required, and the bulk of these can be fixed in a single focused pass.
