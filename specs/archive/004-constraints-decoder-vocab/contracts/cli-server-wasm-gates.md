# Contract: CLI / Server / WASM Runtime-Config Gates

**Crates:** `marque` (CLI), `marque-server`, `marque-wasm`, `marque-config`
**Phase:** D
**Spec refs:** FR-013 (third clause), US2.5, US2.6, Constitution III amendment

## Intent

Two orthogonal gates, one per concern:

1. **Corpus-override gate** — runtime configuration that modifies decoder posteriors. Permitted on CLI (trusted self-invocation); rejected at the HTTP handler on server (untrusted wire caller); compile-excluded on WASM (untrusted embedder).
2. **Decoder-dispatch gate** — which API call invokes the probabilistic decoder at all. Default is strict-only on every target; explicit opt-in per target unlocks the decoder with baked-in priors.

### Gate 1: corpus-override

| Target | Corpus override accepted? | Enforcement |
|---|---|---|
| CLI (`marque`) | Yes — `--corpus-override <file>` | Operator trusts their own invocation |
| Server (`marque-server`) | No — rejected at HTTP handler | Untrusted caller over the wire |
| WASM (`marque-wasm`) | No — no codepath exists | Untrusted embedder; compile-time exclusion via Cargo feature |

### Gate 2: decoder dispatch

| Target | Strict default | Decoder opt-in | Notes |
|---|---|---|---|
| CLI (`marque`) | `marque check`, `marque fix` | `marque fix --deep-scan` | Per-invocation flag |
| Server (`marque-server`) | `POST /v1/lint`, `POST /v1/fix` | `POST /v1/lint?deep_scan=1`, `POST /v1/fix?deep_scan=1` (or `/v1/batch` endpoint) | Per-request query param |
| WASM (`marque-wasm`) | `lint(bytes)`, `fix(bytes)` | `lint_deep_scan(bytes)`, `fix_deep_scan(bytes)` | Separate exports — no runtime flag parameter |

Both gates are enforced independently: a WASM embedder can call `lint_deep_scan` (Gate 2 opt-in) but CANNOT override the corpus (Gate 1 — no such API exists in WASM).

## Surface

### CLI

```bash
marque fix --deep-scan --corpus-override ./my-corpus.toml document.txt
```

- `--corpus-override` is a valid CLI flag; the file is parsed at invocation start; the parsed priors replace the built-in corpus priors for the duration of the run.
- Audit records produced with an override active carry a feature `CorpusOverrideInEffect` so downstream consumers can version-gate their trust.

### Server

```http
POST /v1/fix HTTP/1.1
Content-Type: application/json
X-Marque-Corpus-Override: ...    # REJECTED
```

- Any request header, query parameter, or body field claiming to override the corpus is rejected with `400 Bad Request` and audit-logged.
- The server binary is compiled WITH the override-parsing code (shared with CLI), but the HTTP handler does not expose it.

### WASM

- `marque-wasm` is compiled WITHOUT the override-parsing code. A Cargo feature `corpus-override` gates the code; the WASM target does not enable the feature.
- A compile-fail test in the WASM build verifies that introducing a corpus-override codepath into the WASM artifact fails compilation.
- WASM exposes four exports:
  - `lint(bytes) -> DiagnosticStream` — strict recognizer only (default, low-latency).
  - `fix(bytes) -> FixResult` — strict recognizer only.
  - `lint_deep_scan(bytes) -> DiagnosticStream` — decoder enabled with build-time-baked priors.
  - `fix_deep_scan(bytes) -> FixResult` — decoder enabled with build-time-baked priors.
- Neither deep-scan export accepts runtime prior-modifying parameters. Audit records from `fix_deep_scan` carry the same `Confidence` payload as the CLI `--deep-scan` path.

## Contract

- **CLI (permitted):** Local operators may supply a corpus override. They are trusted (self-invocation). Audit record carries `CorpusOverrideInEffect` feature.
- **Server (rejected at handler):** HTTP callers MUST NOT be able to supply a corpus override. Any such field is rejected with `400`. Rejection is audit-logged but does not expose the attempted override contents to downstream logs.
- **WASM (compile-time excluded):** The WASM artifact cannot contain a corpus-override codepath. Enforcement is the `corpus-override` Cargo feature, not disabled at runtime.

### Why compile-time for WASM and runtime for server

- The server binary exists as one artifact; operators running `marque-server` have legitimate reasons to sometimes use the override code (e.g., CLI mode of the same binary, if added). Runtime rejection at the handler is precise.
- The WASM artifact is a single-purpose sandboxed embedding. It has no legitimate need for the override code, and a runtime rejection in WASM can be patched by a hostile embedder. Compile-time exclusion is unbypassable.

## Failure modes

| Error | Trigger | Expected behavior |
|---|---|---|
| Server receives corpus override | Any field, header, or param claiming to override corpus | `400 Bad Request`; audit log entry; no engine change |
| WASM build contains override code | Future commit adds corpus-override path to WASM | `cargo build --target wasm32-unknown-unknown -p marque-wasm` fails |
| CLI with malformed override file | Operator supplies invalid override | `2` exit code; clear error message naming the malformed entry |

## Test scenarios

1. **CLI override accepted:** `marque fix --deep-scan --corpus-override ./test-corpus.toml sample.txt` succeeds. Audit records carry `CorpusOverrideInEffect`.
2. **Server rejects corpus-override body field:** `POST /v1/fix` with a JSON body containing `"corpus_override": {...}` returns `400`.
3. **Server rejects corpus-override header:** `POST /v1/fix` with `X-Marque-Corpus-Override: ...` returns `400`.
4. **WASM compile-fail:** A test module in `marque-wasm/tests/` that attempts to reach the corpus-override code under `cfg(target_arch = "wasm32")` fails to compile. This is verified by `cargo build --target wasm32-unknown-unknown` passing only when the codepath is absent.
5. **Interactive (no opt-in) skips decoder:** `marque check sample.txt` (no `--deep-scan`) does not invoke the decoder. Latency is bounded by the strict path (SC-001 ≤16 ms p95). Acceptance Scenario US2.4.
6. **WASM strict default:** `lint(bytes)` in the WASM artifact does not invoke the decoder. Acceptance Scenario US2.7.
7. **WASM deep-scan opt-in:** `lint_deep_scan(bytes)` invokes the decoder with baked-in priors and emits audit records matching the CLI path. Acceptance Scenario US2.8.
8. **WASM has no runtime prior override:** Neither `lint_deep_scan` nor `fix_deep_scan` accepts a parameter that changes decoder posteriors at runtime (type-system enforcement — the export signatures take only a byte buffer). Acceptance Scenario US2.6.
