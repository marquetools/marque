# Issue #184 — Tamper-evident audit log (session Merkle root) + Gramine confidential-VM deployment

**Status:** Phase 3 — Implementation
**Branch:** claude/issue-184-workflow-w2Pvo
**Started:** 2026-05-29
**Issue:** https://github.com/marquetools/marque/issues/184

## Scope

### Restated task
Add two complementary, non-disruptive security features to `marque`:

- **Part A — Session Merkle root (audit chain integrity).** At the close of an
  `Engine::fix` session, compute a BLAKE3 Merkle root over the ordered sequence
  of serialized `AppliedFix` NDJSON records and emit a terminal `session_root`
  record in the audit stream. Any verifier can re-hash the preceding records and
  compare; deleting or mutating any record invalidates the root. Must preserve
  streaming (no per-record hash chaining → no sequentiality constraint on
  `BatchEngine`). BLAKE3 is already in-tree (zero new deps).

- **Part B — Gramine confidential-VM deployment (docs + CI).** Document and
  validate running `marque` CLI inside a Gramine SGX enclave. Doc-and-CI-first:
  add `deploy/gramine/` with a generic `marque.manifest.template`, document the
  attestation workflow and `MARQUE_CLASSIFIER_ID` injection, and add a CI smoke
  test running `cargo test -p marque` under **Gramine SIM/direct mode** (no SGX
  hardware). Document the `marque-extract` limitation (raw-text `check`/`fix`
  only inside the enclave).

### In scope
- `marque-rules`: `session_root` audit record (sentinel/terminal variant);
  audit-schema bump from `marque-3.0` → next (see decision below).
- `marque-engine`: accumulate serialized record bytes in `fix_inner`; compute
  BLAKE3 Merkle root at session close; emit terminal record. Provide a verifier
  helper + the canonical Merkle construction (documented, reproducible).
- CLI + server: surface the root in output; `--dry-run` also emits a root.
- WASM: return the root alongside the records array in `fix()`.
- Tests: corpus-level test that the root validates against records and that any
  mutation invalidates it; reconcile terminal `schema` field with the per-record
  schema constant (audit canary must stay green).
- `deploy/gramine/marque.manifest.template` (generic, `$(pwd)`-relative).
- `deploy/gramine/README.md` attestation + injection workflow docs.
- `.github/workflows/` Gramine SIM smoke-test job (no `SGX=1`).

### Out of scope (issue non-goals)
- Key management / signing / PKI (Merkle root is integrity, not authenticity).
- Per-record hash chaining (sequentiality cost for marginal gain).
- Native SGX rewrite (companion issue).
- Document-format extraction inside the enclave (host-side responsibility).
- `ZeroizeOnDrop` on `AppliedFix.input` — issue says "separate small PR";
  keep out unless trivially required.

### Key decisions / constraints (from issue comments)
1. **Schema reconciliation (comment #1):** The issue body text (`marque-mvp-2` /
   `marque-mvp-3`) is stale. The *actual* in-tree constant is `MARQUE_AUDIT_SCHEMA
   = "marque-3.0"` (`crates/engine/src/audit.rs`, closed accept-list
   `["marque-3.0"]`). The terminal `session_root` record's `schema` field MUST
   match the per-record schema string or the audit canary fails. Adding the
   `session_root` record type is an additive audit-surface change → bump to
   **`marque-3.1`** per the Stable API Surface rule ("`marque-3.1` for additive
   changes"). Update the accept-list, the `const_assert`, the default, and every
   reference in lock-step.
2. **CI must not need SGX hardware (comment #2):** smoke test runs Gramine in
   SIM/direct mode (`SGX=0`). Manifest template must be generic — no hardcoded
   `/home/runner`; use Gramine `$(pwd)`-relative manifest variables.
3. **Audit content-ignorance (Constitution V):** the `session_root` record may
   carry only digests, counts, schema string, timestamp — NO document content.
   The G13 canary (`crates/engine/tests/audit_g13_canary.rs`) must stay green.
4. **Streaming / batch (Constitution VI):** no per-record chaining; root computed
   over the already-produced ordered sequence at session close only.

### Success criteria
- `cargo build`, `cargo test`, `cargo clippy --workspace` all green.
- Audit canary + registration pins green; schema bumped consistently to the
  reconciled value across all references.
- Round-trip test: verifier recomputes root from records → matches; any mutated/
  deleted record → mismatch.
- CLI/server/WASM all surface the root; `--dry-run` emits a root.
- `deploy/gramine/` manifest template + docs present; CI SIM-mode smoke job added
  and self-consistent (no SGX hardware requirement).
- WASM-safety preserved (no new runtime deps in the WASM-safe set; BLAKE3 already
  WASM-configured).

## Plan

### Research findings (synthesized)

- **Fix flow** (`crates/engine/src/engine.rs::fix_inner`): lints → filters proposals
  ≥ threshold → applies right-to-left into a `Zeroizing<String>` scratch buffer →
  promotes each to `AppliedFix` via `__engine_promote` → reverses to document order
  → returns `FixResult::new(source, fixes, dry_run)`. `fix()` and `fix_dry_run()`
  both call `fix_inner`. `FixResult` (`fix_result.rs`) holds `source: SecretBox<str>`,
  `fixes: Vec<AppliedFix>`, `dry_run: bool` with accessors `source()/fixes()/dry_run()`.
- **NDJSON serialization**: one `AppliedFix` → one line via
  `AuditRecord::new(fix).to_string()` (`audit.rs`), which delegates to
  `AppliedFix::to_audit_json(MARQUE_AUDIT_SCHEMA)` in `marque-rules`. Records are
  content-ignorant (BLAKE3 `digest` of input, never raw content). There is **no
  `type` discriminator** on applied-fix records today.
- **Schema machinery** (`crates/engine/src/audit.rs`): `validate_schema()` const-fn
  closed accept-list `b"marque-3.0"`; `MARQUE_AUDIT_SCHEMA` default `"marque-3.0"`
  (override via `option_env!`); `const _: () = assert!(validate_schema(...))`;
  `is_accepted_schema()` runtime mirror. Re-exported from engine `lib.rs`.
- **Three output surfaces** all share the identical idiom — iterate `result.fixes()`,
  map each through `AuditRecord::new(fix).to_string()`, emit:
  - CLI `marque/src/main.rs` (`Commands::Fix`): joins records → audit-log file or
    stderr; fixed text → stdout/file. `--dry-run` handled (skips write). 
  - Server `crates/server/src/handlers.rs::fix_handler`: `FixResponse { source, records: Vec<String> }`.
  - WASM `crates/wasm/src/lib.rs::fix`: JS object `{ source, records: [...] }`.
- **blake3** is already a dep of `marque-rules` and `marque-engine` (`blake3.workspace
  = true`), workspace-configured `default-features=false, features=["pure"]` (WASM-safe).
  No new dependency anywhere.
- **Tests asserting the literal `"marque-3.0"`**: `audit_schema_pin.rs` (×2),
  `server/tests/fix_endpoint.rs` (×1), `CLAUDE.md` (×2 docs). `audit_g13_canary.rs`
  and `wasm/tests/fix_roundtrip.rs` use the **constant** → auto-track the bump.
- **No existing** `deploy/`, gramine/SGX prior art, or `docs/security/WHITEPAPER.md`
  in tree (CLAUDE.md references the whitepaper but it's not present). New files follow
  SPDX headers; `REUSE.toml` already blanket-annotates `**/*.rs|toml|md|yml|yaml|json`,
  so new files in those extensions need no `.license` sidecar but SHOULD carry the
  inline SPDX header to match house style. Non-standard extensions (`.template`,
  `.manifest.template`) are NOT covered by REUSE.toml globs → add a REUSE.toml
  annotation entry OR a `.license` sidecar for them.
- **CI** `.github/workflows/build.yml`: single `build-and-test` job on
  `ubuntu-latest`, `dtolnay/rust-toolchain@stable` pinned `1.85.0` + clippy/rustfmt,
  cargo cache, then build/test/clippy(`-D warnings`)/fmt. `rust-toolchain.toml` pins
  `1.85.0`, profile minimal, target `wasm32-unknown-unknown`.

### Merkle construction (canonical, documented, RFC 6962-style domain separation)

Computed over the ordered applied-fix NDJSON line bytes **only** (NOT the terminal
record — a record cannot embed its own hash). Domain separation prevents
second-preimage attacks:

- leaf:  `H(0x00 ++ line_bytes)`        where `line_bytes` = `AppliedFix::to_audit_json(schema)` (no trailing newline)
- node:  `H(0x01 ++ left_32 ++ right_32)`
- odd count at a level: the lone node is **promoted unchanged** to the next level.
- empty (record_count == 0): root = `H(0x02)` (domain-tagged empty marker), so a
  zero-fix session still emits a well-defined, verifiable root.
- `H` = BLAKE3-256 (`blake3::hash`).

This matches the producer and verifier through a single shared function so they can
never drift.

### Sections (ordered; each independently testable)

**S1 — Schema bump `marque-3.0` → `marque-3.1` (engine + tests + docs).**
- `crates/engine/src/audit.rs`: `validate_schema` accept-list → `b"marque-3.1"`;
  `MARQUE_AUDIT_SCHEMA` default → `"marque-3.1"`; verify any doc-comment literal.
- `crates/engine/tests/audit_schema_pin.rs`: pin → `"marque-3.1"`.
- `crates/server/tests/fix_endpoint.rs`: literal assert → `"marque-3.1"`.
- `CLAUDE.md` lines 50, 184: update current-schema references to `marque-3.1`,
  noting the additive `session_root` terminal record (the file's own rule:
  "marque-3.1 for additive changes").
- *Verify*: `cargo test -p marque-engine audit_schema` green; full `grep` shows no
  stale `marque-3.0` literal except historical/changelog narration.
- Rationale: reconciles issue's stale `marque-mvp-2/3` text against the real in-tree
  constant per maintainer comment #1; one binary = one schema (replace, not append).

**S2 — `SessionRoot` type + Merkle + terminal-record serializer (engine).**
- New module `crates/engine/src/merkle.rs` (or extend `audit.rs`): 
  - `pub fn merkle_root_lines<B: AsRef<[u8]>>(lines: &[B]) -> [u8; 32]` (shared
    producer/verifier; implements the construction above).
  - `pub struct SessionRoot { record_count: usize, root: [u8; 32] }` with
    `compute(fixes: &[AppliedFix]) -> Self` (serializes each via
    `to_audit_json(MARQUE_AUDIT_SCHEMA)` — byte-identical to `AuditRecord` Display),
    `root_hex()`, `record_count()`.
  - `SessionRoot::to_ndjson(&self) -> String`: terminal line
    `{"type":"session_root","schema":"<MARQUE_AUDIT_SCHEMA>","record_count":N,"root":"<hex>","ts":"<rfc3339>"}`.
    `ts` from `SystemTime::now()` at call time; serialize with the same RFC3339 path
    `AppliedFix` already uses (reuse that helper to avoid a new time-format dep).
  - `pub fn verify_session_root(applied_lines: &[&str], expected_root_hex: &str) -> bool`
    for verifiers/tests: recompute over the applied-fix lines, hex-compare.
- Content-ignorance: terminal record carries only `type`, `schema`, integer
  `record_count`, BLAKE3 hex `root`, RFC3339 `ts` — no content, no filenames.
- Re-export `SessionRoot` (+ `verify_session_root`, `merkle_root_lines`) from engine
  `lib.rs`.
- *Verify*: unit tests in `merkle.rs` — known-vector root for 0/1/2/3 records;
  odd-promotion; mutating a leaf changes the root; reordering changes the root.

**S3 — Surface the root in CLI / server / WASM (+ dry-run).**
- Each surface, after building the applied-fix `records`, appends
  `SessionRoot::compute(result.fixes()).to_ndjson()` as the final NDJSON line.
  Because `compute` re-serializes with the same `to_audit_json`, the root is over
  exactly the emitted applied-fix bytes.
  - CLI: append terminal line to the joined audit NDJSON (both fix + dry-run paths;
    `fix_dry_run` already returns a normal `FixResult` so it works uniformly).
  - Server: add `root: String` (hex) to `FixResponse` AND append the terminal record
    to `records` so the NDJSON stream is self-contained and the convenience field is
    present.
  - WASM: append terminal record to the `records` JS array AND set a top-level
    `root` string on the return object (issue: "emit root ... alongside the records
    array"). No new runtime config → Constitution III preserved.
- *Verify*: existing CLI/server/WASM tests still pass; add assertions that the
  terminal `session_root` record is present and its `root` validates via
  `verify_session_root` over the applied-fix lines.

**S4 — Corpus/round-trip integration test (engine).**
- New `crates/engine/tests/session_root_integrity.rs`:
  - Fix a multi-fix document; collect applied-fix lines via `AuditRecord`; compute
    `SessionRoot`; assert `verify_session_root(lines, root_hex)` is true.
  - Mutate one record byte → assert verification fails.
  - Delete one record → assert verification fails.
  - Reorder two records → assert verification fails.
  - Zero-fix input → terminal record emitted with `record_count: 0` and the
    documented empty-root; verifies.
- Extend `audit_g13_canary.rs`: include the terminal `session_root` line in the
  scanned stream and assert the sentinel still never appears (content-ignorance of
  the new record).
- *Verify*: `cargo test -p marque-engine` green.

**S5 — Gramine deploy assets (`deploy/gramine/`).**
- `deploy/gramine/marque.manifest.template`: generic Gramine manifest using
  `{{ entrypoint }}`, `{{ gramine.runtimedir() }}`, `{{ arch_libdir }}`,
  `{{ env.HOME }}`-free, NO hardcoded `/home/runner` (maintainer comment #2). Allowed
  env: `MARQUE_CLASSIFIER_ID`, `MARQUE_LOG`, `MARQUE_AUDIT_SCHEMA`. Trusted files:
  the marque binary + gramine runtime + arch libdir; `sgx.debug = true` and
  `sgx.enclave_size` sized for raw-text `check`/`fix`; `sys.enable_extra_runtime_domain_names`.
  `loader.argv` passthrough. Templated, `$(pwd)`-relative.
- `deploy/gramine/README.md`: build/run in **direct (non-SGX) mode**
  (`gramine-manifest` → `gramine-direct`); the SGX-mode path (`gramine-sgx`,
  `gramine-sgx-sign`); the attestation workflow (what a verifier checks —
  MRENCLAVE/MRSIGNER, DCAP/EPID quote surface); how `MARQUE_CLASSIFIER_ID` is
  injected via the enclave env; documented limitations (`marque-extract`/Kreuzberg
  not in-enclave → raw-text `check`/`fix` only; format extraction stays host-side).
  Cite Constitution III (enclave use case was a design goal).
- REUSE: the `.template` extension is NOT covered by `REUSE.toml` globs → add a
  `[[annotations]]` entry for `deploy/gramine/*.template` (and any non-glob files)
  so the REUSE/license stays complete.
- *Verify*: `gramine-manifest` produces a valid `.manifest` from the template in S6
  CI; manual read-through for generic paths.

**S6 — CI Gramine SIM-mode smoke test (`.github/workflows/gramine-smoke.yml`).**
- Separate workflow (isolates the external `gramine` apt install from the main
  build gate) on `ubuntu-latest`, PR + push to main.
- Steps: checkout → install Rust `1.85.0` → install Gramine from the official apt
  repo → `cargo build -p marque --locked` (debug; raw-text only, no extract) →
  `gramine-manifest` from `deploy/gramine/marque.manifest.template` with
  `-Dentrypoint=$(pwd)/target/debug/marque` etc. → `gramine-direct marque check
  <fixture>` and `gramine-direct marque fix --dry-run <fixture>` → assert the output
  includes diagnostics and a `session_root` terminal record (ties Part A + B
  together). **Direct mode only — no `SGX=1`, no hardware** (maintainer comment #2).
- Document in the workflow comments why `gramine-direct` (not `gramine-sgx`) and why
  a separate workflow. Run `cargo test -p marque` natively as a fallback assertion in
  the same job so the "cargo test -p marque" intent from the issue is satisfied even
  where running the whole harness under gramine is impractical.
- *Verify*: cannot fully run gramine locally in this container; ensure YAML is valid
  and self-consistent; rely on the SIM-mode-only constraint + generic manifest.

### Sequencing
- S1 → S2 → (S3, S4 depend on S2) → S5 → S6 (S6 depends on S5 manifest + S1/S2 output).
- S5 and S1/S2 are independent; S5 can be authored in parallel during implementation.

## Critique notes — hardened decisions (5+ critics: correctness, simplicity×2, integration×2, robustness, completeness)

**D1 — Merkle vs flat (simplicity).** KEEP a real RFC 6962-style Merkle tree. The
issue title + body say "Merkle root" explicitly; Constitution VIII (authoritative-
source fidelity) makes honoring the named primitive correct. Trade-off (no
inclusion-proof use case today) noted and accepted; the tree leaves that door open
at ~40 LOC cost.

**D2 — Verifier/byte-identity (BLOCKER: correctness, robustness×2).** The root is
computed over the EXACT emitted record-line bytes (the JSON object, NO trailing
newline), in emission order, EXCLUDING the terminal record. Each surface hashes the
bytes IT emits → self-consistent per channel. Verifier: recompute
`merkle_root_lines(emitted_record_lines)` and compare to the terminal `root`.
Reproducibility: under a fixed clock the AppliedFix timestamps are fixed → records +
root are byte-reproducible; the terminal record's `ts` is wall-clock and is NOT part
of the hash input (a record can't embed its own hash anyway). Documented in code +
README.

**D3 — Crate placement (BLOCKER: integration×2, completeness).** `SessionRoot` +
`merkle_root_lines` live in `marque-rules` (audit-type home; WASM-safe; `blake3`
already a dep). Kept as a STANDALONE sentinel type (issue: "or as a sentinel
variant"), NOT an `AuditLine` variant and NOT inside `FixResult.audit_lines`. Reason:
the audit-line→JSON projection is scheme-aware and lives per-surface (render.rs /
wasm); making it an enum variant would force a parallel-update of every scheme-aware
arm AND require the engine to own canonical serialization. Standalone keeps the
change shallow and lets each surface emit the terminal record over its own bytes.

**D4 — Content-ignorance / zeroize (BLOCKER robustness #1 → resolved non-issue).**
Audit lines are digest-only by the G13 invariant (no document content), so holding
serialized lines as `Vec<String>` to hash does NOT violate Constitution II (which
governs content-BEARING buffers). The fixed-source buffer stays `SecretBox` as today.
Terminal-record fields: `type` (permitted), `schema` (= `MARQUE_AUDIT_SCHEMA`
constant, never a literal), `record_count` (numeric, not scanned), `root` rendered as
`"blake3:<hex>"` (auto-permitted by the canary prefix rule), `ts` (RFC3339 — add to
canary `PERMITTED_STRING_KEYS`). Add a shape-pin test so accidental content-bearing
fields fail loudly.

**D5 — Schema-bump locations (correction: integration).** No `crates/engine/src/audit.rs`
exists. Real sites: `crates/engine/build.rs` (ACCEPTED + DEFAULT), `crates/engine/src/lib.rs`
(`AUDIT_SCHEMA_IS_V3_0` → rename `_V3_1`; doc comments), `crates/engine/tests/audit_schema_accept_list.rs`
(4 assertions + const name), `crates/engine/tests/audit_g13_canary.rs` (2 synthetic
literals), CLI `main.rs` help text + `marque/tests/cli_version.rs`, render.rs/wasm
`AUDIT_SCHEMA_IS_V3_0` references, docs (CLAUDE.md — AGENTS.md/GEMINI.md are symlinks).
`GET /v1/schema/version` returns `marque_capco::SCHEMA_VERSION` (CAPCO priors axis) —
INDEPENDENT of the audit schema; no change.

**D6 — Root scope = ALL audit_lines (both AppliedFix + TextCorrection arms), in
order. Verified against the `AuditLine` enum (two arms today).**

**D7 — Batch (robustness): root is PER-DOCUMENT.** Each `Engine::fix` call = one
session = one `FixResult` = one root over that document's audit_lines. `BatchEngine`
runs independent `fix_with_options` calls; no cross-document root, no sequentiality.
Add a two-document independence test.

**D8 — Empty/odd/edge (robustness, correctness).** `merkle_root_lines(&[])` → `H(0x02)`
(domain-tagged empty marker); 1 record → `H(0x00‖line)`; odd node promoted unchanged.
Zero-fix sessions STILL emit a terminal record (`record_count: 0`). Hex lowercase,
64 chars (blake3 `to_hex` default). Tests pin 0/1/2/3-record vectors + mutation +
deletion + reorder + reproducibility + empty.

**D9 — Server (integration, completeness).** Current `FixResponse` carries no records.
Add `session_root: String` (`blake3:<hex>`) AND `audit_log: Vec<String>` (the NDJSON
record lines) so the server response is self-verifiable. Server serializes audit_lines
via a single shared canonical serializer added to `marque-engine`
(`marque_engine::audit::audit_line_ndjson(scheme, line)`); server computes the root
over exactly those strings. CLI/WASM keep their existing serializers (no regression
risk) and compute the root over their own emitted bytes — each channel internally
consistent. (Cross-surface byte-identity remains the CLI↔WASM parity-test concern,
untouched.)

**D10 — Gramine (robustness, integration, simplicity).** PIN the gramine apt version;
add a `gramine-manifest` compile-validation step before running; use Gramine native
template vars (`{{ entrypoint }}`, `{{ gramine.runtimedir() }}`, `{{ arch_libdir }}`)
with CI `-D` overrides — no hardcoded `/home/runner`. `sgx.enclave_size = "512M"`
(documented: raw-text check/fix headroom). Direct (non-SGX) mode only. Separate
`.github/workflows/gramine-smoke.yml` (isolates flaky external apt from the 966-line
ci.yml; documented rationale) + native `cargo test -p marque` in the same job. Fixture:
a `tests/corpus/invalid/*` case that triggers ≥1 fix; assert a `session_root` line
appears (ties Part A+B).

**D11 — Verifier surface (completeness).** Document the verification recipe in
`deploy/gramine/README.md` is wrong home → put it in CLAUDE.md Stable API Surface +
the `SessionRoot` rustdoc (re-hash preceding record lines, compare `root`). A CLI
`marque verify` subcommand is OUT of scope (separate feature); the programmatic
`SessionRoot::verify` + documented recipe satisfy "any verifier can re-hash."

**D12 — Acknowledged non-goals (completeness).** `ZeroizeOnDrop` on `AppliedFix.input`
stays out (issue says separate PR; not touched here). No `CHANGELOG.md` at repo root
(only `crates/utils/CHANGELOG.md`); schema-bump note goes in CLAUDE.md.

**D13 — CLAUDE.md Stable API Surface (BLOCKER integration).** Bump "current schema"
to `marque-3.1`; add a bullet documenting the `session_root` terminal record (shape,
position = last, Merkle construction, verifier recipe).

## Implementation log

## Implementation log
_TBD (Phase 3)_

## Review findings
_TBD (Phase 4–5)_
