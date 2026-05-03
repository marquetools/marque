<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com> -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# marque Security White Paper

> **Status tag legend**
>
> - `[LANDED]` — design is in code, tested or otherwise exercised today
> - `[PARTIAL]` — shape is committed (trait, feature flag, contract); wiring still open
> - `[PLANNED]` — not yet in code; deliverable of a named phase/task
> - `[NON-GOAL]` — deliberately out of scope; called out so it is not mistaken for an omission
>
> Each section ends with its status and the task / FR / SC IDs it is tied to.
> When a task lands or a design changes, this document is updated in the same PR.

**Document version**: 0.16 · **Last amended**: 2026-04-27
· **Authoritative companion**: [`.specify/memory/constitution.md`](../../.specify/memory/constitution.md)
· **Governing spec**: [`specs/archive/004-constraints-decoder-vocab/`](../../specs/archive/004-constraints-decoder-vocab/)

---

## 0 · Document conventions

- This paper is the **security lens** over the architecture the constitution
  already defines. Where the constitution is authoritative, this document
  cross-references rather than restates.
- Each section carries a status tag so a reviewer can tell at a glance what
  is code, what is contract, and what is still a promise.
- Evidence is cited as `path/to/file.rs:NN` against the branch this document
  lives on. Line numbers drift; the file + symbol pair is the stable
  reference. If a citation no longer matches, treat the drift as a gap and
  file a PR to this document.
- This paper does **not** define security requirements that do not already
  exist in the constitution, a plan under `docs/plans/`, or a spec under
  `specs/`. New requirements land in those sources first; the paper follows.

**Amendment procedure.** Changes to this paper follow the same PR workflow
as the constitution. A section may move between status tags in any PR; a
new section or a retired invariant requires a pointer in the change log
(§C) and, where appropriate, a corresponding constitution amendment.

---

## 1 · Executive summary

`marque` is a general-purpose rule engine for fast text processing whose MVP
application is the CAPCO/ISM classification-marking rule set. It is designed
to run in three surfaces — a CLI, an axum HTTP server, and a WebAssembly
artifact embedded in browsers and Office add-ins — and to produce byte-identical
diagnostics across all three. The security posture that follows is load-bearing
for every one of those surfaces.

Four shapes carry most of the security weight:

1. **Content-ignorance by construction.** The engine operates on `Span` byte
   offsets into caller-owned buffers, never on copies. Diagnostics, audit
   records, telemetry, logs, and cache keys carry token canonicals,
   enumerated feature labels, span offsets, digests, and posterior scalars —
   never document text. The classified content the caller hands in does not
   survive into any marque-produced stream.

2. **Audit as compliance output, not telemetry.** Every applied fix produces
   an `AppliedFix` audit record with rule ID, original text span,
   replacement text span, confidence, timestamp, classifier identity, and
   dry-run flag. Audit records are produced only by the engine; rule crates
   cannot mint them. The audit stream is the evidence a reviewer uses to
   answer "why did this fix apply?"

3. **Two-layer rule architecture with authoritative-source fidelity.** Layer 1
   predicates are generated from ODNI ISM schemas at compile time; Layer 2
   rules hand-cite CAPCO-2016 passages and are verified against the vendored
   manual. Schema version is pinned in `Cargo.toml` metadata and checked
   inside `build.rs`. A schema drift is a loud build failure, not a silent
   behavior change.

4. **WASM-safe / format-agnostic core.** The engine depends on no runtime
   I/O, no format adapters, and no platform-specific code. Document
   extraction (`marque-extract`) is a separate, non-WASM crate. Runtime
   configuration that would change recognizer posteriors is compile-time
   excluded from the WASM artifact and rejected at the server's HTTP
   boundary.

This paper documents each of those shapes, enumerates the threat model
that motivates them, and lists the gaps where intended design does not yet
match reality. The [gap register](#17--known-gaps--roadmap) at the end is
the action list.

**Status**: `[LANDED]` for the shape; `[PARTIAL]` for enforcement detail —
see individual sections.

---

## 2 · Assets, adversaries, and trust surfaces

### 2.1 Assets protected

| Asset | Lifetime in marque | Protected by |
|---|---|---|
| Classified content in transit through the engine | Lives in caller buffer only; reachable via `Span` | Zero-copy invariant (§3.2); WASM-safe format-agnostic core (§3.3) |
| Audit records | Emitted to stderr / WASM return channel per fix | Engine-only promotion boundary (§3.4, §6.2); schema versioning (§6.4) |
| Classifier identity | Read from env or `.marque.local.toml`; stamped into audit records; never logged | Config segregation (§11); deliberate non-goal: forgery resistance (§2.5) |
| Corpus-derived priors (Phase D+) | `build.rs` input only; compiled into `&'static` tables | Build-time pipeline (§7.4); reproducible generation from `tools/corpus-analysis/` |
| ODNI ISM schema fidelity | Vendored under `crates/ism/schemas/`; parsed at build | Schema version pin (§7.2); Authoritative Source Fidelity principle (§3.7) |
| CAPCO-2016 citation integrity | Vendored under `crates/capco/docs/`; cited in rule code | Principle VIII (§3.7); citation verification task T089 |

### 2.2 Adversaries considered

- **Hostile document author** — drafts input designed to mislead the
  recognizer (T1), exhaust the engine, or smuggle content into marque's
  output stream.
- **Hostile API caller** — sends crafted HTTP requests to `marque-server`
  aiming to exhaust resources, bias the recognizer via runtime configuration
  (T3), or inject into audit logs.
- **Hostile WASM embedder** — hosts the marque artifact in a browser page
  and tries to bias behavior via postMessage-style channels, or to read
  state belonging to other tenants sharing the same runtime.
- **Hostile corpus / prior contributor** — submits a PR to
  `tools/corpus-analysis/` or a regenerated `priors.json` that biases the
  decoder (T1 amplified to the build layer).
- **Supply-chain attacker** — ships a malicious version of a direct or
  transitive dependency, or subverts the GitHub Actions pipeline.
- **Curious insider** — runs marque in an authorized context but tries to
  use diagnostic, audit, or telemetry channels to retain or redistribute
  content the tool was supposed to pass through without accreting.

### 2.3 Trust surfaces

| Surface | Runs as | Trusts | Does not trust |
|---|---|---|---|
| CLI (`marque`) | User's own process | The user invoking it, including `--corpus-override` | — |
| `marque-server` | Networked service | Its operator's configuration file | HTTP callers, including fully-authenticated ones, for runtime-config items |
| `marque-wasm` | Browser / extension worker | The artifact's compiled priors | All runtime callers for config that would alter recognizer posteriors |
| Build environment | CI or developer workstation | Vendored schemas + manual + corpus fingerprints | Network fetches at build time |
| `.marque.toml` (committed) | Repo policy | Project maintainers | Operator / user identity data |
| `.marque.local.toml` (gitignored) | Operator machine | Operator / user | Never committed; enforced by gitignore + schema |

### 2.4 Threat model

The primary threat model for the decoder (Phase D, 004 spec) lives in
[`docs/plans/2026-04-19-recursive-lattice-and-decoder.md`](../plans/2026-04-19-recursive-lattice-and-decoder.md) §6a.
This paper adopts T1–T3 from that model and extends it to the full surface:

- **T1 — Prior-manipulation on local disambiguation.** An adversary drafts
  prose whose statistics bias the decoder on local calls like `(C)` → C
  (confidential vs. copyright). Mitigation: strict-context floor — if any
  CONFIDENTIAL+ marking exists anywhere in the document, ambiguous `(C)`
  resolves to CONFIDENTIAL without consulting the decoder.
  See §9.3 and `ParseContext.strict_evidence` (FR-011, T045, T062).

- **T2 — Content leakage via decoder feature traces.** The audit-v2 record
  adds `features: Vec<FeatureContribution>`. If `FeatureId` were a free
  string, a sufficiently creative feature label could exfiltrate document
  text. Mitigation: `FeatureId` is an enum, enforced at compile time; a
  corpus-level CI test (T056) greps audit output for non-token content.
  See §5.2 and §6.4.

- **T3 — Runtime corpus override as a trust boundary.** An attacker who
  can inject a table of prior overrides into a running engine can bias
  fixes toward a target outcome. Mitigation: `--corpus-override` is
  CLI-only (the CLI's principal *is* the operator); the server rejects
  any HTTP request carrying override payload with a clean 400; the WASM
  artifact compiles the override codepath out entirely via
  `default-features = false` on `marque-engine`.
  See §10 and `specs/004-constraints-decoder-vocab/contracts/cli-server-wasm-gates.md`.

- **T4 — Adversarial input triggers panic, OOM, or pathological latency.**
  Mitigation: bounded span lengths in the scanner, stateless rules, fuzz
  target over `Engine::lint`, p95 latency gates (SC-001 / SC-002).
  See §9.1.

- **T5 — Hostile HTTP caller exhausts server resources.** Mitigation:
  intended Tower layers for auth, rate limiting, body-size cap, and
  concurrency backpressure. *Not all wired today; see gap register.*
  See §10.2 and §13.

- **T6 — Hostile WASM embedder reads across tenants.** Mitigation: the
  WASM artifact returns only to its direct caller; marque holds no
  cross-invocation state beyond the compiled priors; embedder isolation
  is the browser's job.
  See §10.3.

- **T7 — Hostile corpus contributor biases decoder priors at build time.**
  Mitigation: corpus regeneration is gated on `MARQUE_ENRON_CORPUS`;
  `priors.json` will land with a corpus fingerprint that is a hash of
  metadata (not content) per Constitution V; the corpus accuracy
  regression harness (SC-003) catches aggregate drift.
  See §7.4 and §14.

- **T8 — Supply-chain attack on a direct or transitive dependency.**
  Mitigation: `cargo-deny` (advisories + license allow-list + source
  allow-list) on every PR; `Cargo.lock` committed; all GitHub Actions
  pinned to commit SHAs; CodeQL across Rust / Python / Actions.
  See §8 and §13.

- **T9 — Classifier identity forgery in audit records.** *Deliberate
  non-goal.* The audit stream records *claimed* identity; cryptographic
  proof is a deployment concern (TLS client certs, SSO attestations,
  signed log aggregation).
  See §6.5 and §2.5.

### 2.5 Explicit non-goals

These are frequently-requested properties marque deliberately does **not**
provide. Each is listed so an evaluator does not mistake its absence for
an oversight:

- **Not a Data Loss Prevention system.** marque lints markings; it does
  not classify content, detect exfiltration, or enforce egress policy.
- **Not a classification-review authority.** Diagnostics and fixes are
  tools for human reviewers, not substitutes for one.
- **Not a cryptographic commitment.** The audit stream is NDJSON on
  stderr; tamper-evidence (signed logs, transparency trees) is a
  deployment concern.
- **No memory zeroization on drop.** Constitution II notes sensitive
  content should be footprint-minimized and hints at future
  SGX/TrustZone integration. Today, buffers are not zeroed; `zeroize`
  is not a dependency. Callers who need this should wrap marque in a
  process that exits between documents.
- **No classifier-identity forgery resistance.** See T9 above.
- **No cross-tenant isolation at the engine layer.** `BatchEngine`
  documents are processed concurrently but independently. Tenant
  isolation is the deployment's responsibility.
- **No input zeroization.** Callers retain the buffer; marque neither
  copies it nor zeroes it.

**Status**: `[LANDED]` for T1/T2/T3 shapes; `[PARTIAL]` for enforcement
(see gap register). All non-goals are `[NON-GOAL]` by design.

---

## 3 · Architectural security invariants

Each invariant below is first-class in [`.specify/memory/constitution.md`](../../.specify/memory/constitution.md);
this section is a security-lens index.

### 3.1 Content-ignorance (G13 / I-J2 / I-K2)

Engine output streams — diagnostics, audit records, feature traces,
cache keys, logs — contain token canonicals, enumerated feature labels,
span offsets, digests, posterior scalars, and category IDs. They do
**not** contain document content, metadata field values, or subject-claim
free-form text.

G13 (Constitution V; object-side audit) is the current invariant.
I-J2 (`DecisionRecord` in the Phase J access-decision work) and I-K2
(metadata extraction in Phase K) are the forward-compatible extensions.

The corpus-level CI enforcement (T056) is the load-bearing check that
converts content-ignorance from convention into a gate.

**Status**: `[LANDED]`. T056 shipped as `crates/engine/tests/audit.rs`:
a sentinel-grep sweep over every fixture in
`tests/corpus/{invalid,valid,prose}/`, a marking-in-prose composite
test that wraps each invalid fixture in ~4 KB of article prose, a
companion diagnostic-stream check, and a `#[should_panic]` self-test
that proves the sentinel-check is load-bearing. FR-012 (`FeatureId`
as enum) landed earlier in `crates/rules/src/confidence.rs`.

### 3.2 Zero-copy & `Span` discipline

The scanner produces `Span` values (byte offsets into the caller's
buffer) with no heap allocation on the hot path. `IsmAttributes` fields
use `Box<[T]>` to preclude over-allocation after parse.

Cited: Constitution II. The hot-path allocation regression gate
(`crates/core/tests/alloc_budget.rs`, behind the `count-allocs`
feature) installs a counting global allocator and asserts
`Scanner::scan(...)` does not exceed a small allocation budget. The
canonical claim — "alloc count for a buffer with one banner is the
same whether the buffer is 23 bytes or 4 KB" — is encoded as
`scanner_alloc_count_is_buffer_size_independent`. The gate runs as
a dedicated `count-allocs` CI job:

```
cargo test -p marque-core --features count-allocs \
    --test alloc_budget -- --test-threads=1
```

`--test-threads=1` is mandatory because `ALLOCATIONS` is a
process-wide atomic counter; parallel tests inflate each other's
deltas. The on-test `MEASURE_LOCK` mutex narrows the contention
surface but cannot eliminate test-runner-side allocations between
acquire/release cycles. The header comment in `alloc_budget.rs`
covers the full reasoning.

**Status**: `[LANDED]`.

### 3.3 Format-agnostic core & WASM-safe set

The WASM-safe crate set is `marque-ism`, `marque-core`, `marque-rules`,
`marque-scheme`, `marque-capco`. Each has zero runtime I/O dependencies,
no format adapters, no platform-specific code. `marque-extract`
(document formats via Kreuzberg) is not in this set and does not ship
in WASM builds.

The runtime-config restriction on the WASM target (no caller-supplied
data that would alter recognizer posteriors) is the sharp edge of this
invariant. See §10.3.

**Status**: `[LANDED]` for dependency hygiene; `[PARTIAL]` for
compile-fail test that would catch future drift.

### 3.4 Engine-promotion boundary for `AppliedFix`

Rule crates produce `FixProposal` values, which are pure data with no
runtime context. Only `Engine::fix_inner` may promote a proposal to an
`AppliedFix` by snapshotting timestamp, classifier ID, dry-run flag,
and input. The constructor `AppliedFix::__engine_promote` is
`pub #[doc(hidden)]` because visibility cannot seal it across the
`marque-rules` ← `marque-engine` dependency direction; the seal is by
convention.

Cited: `crates/engine/src/engine.rs:504, 517, 612` (production call
sites) and Constitution V "Architectural Invariants".

**Status**: `[LANDED]`. Type-level seal landed in v0.12 via
`EnginePromotionToken` (private `_seal: ()` ZST, `crates/rules/src/lib.rs`)
threaded as the sixth argument of `__engine_promote`. See gap register
row 5 (struck through) and §6.2 for the full call-site analysis.

### 3.5 Rule and recognizer statelessness

Rule impls and `Recognizer` impls are `Send + Sync` and hold no mutable
global state. Interior mutability via `OnceCell<Mutex<_>>`-as-hidden-cache
and similar patterns is prohibited. `LazyLock<CapcoScheme>`
(`crates/capco/src/rules_declarative.rs:102`) is init-once immutable,
which is compliant; it is the only static in the rule crates.

Cited: Constitution VI; FR-023.

**Status**: `[LANDED]`.

### 3.6 Acyclic crate graph

The canonical dependency graph in Constitution VII is one-directional.
`marque-rules` does not depend on `marque-core`. `marque-capco` does
not depend on `marque-core`. `marque-engine` is the convergence point.
`cargo check --workspace` passing on every commit is the gate.

**Status**: `[LANDED]`.

### 3.7 Authoritative source fidelity

Every CAPCO rule cites a verified passage in `crates/capco/docs/CAPCO-2016.md`.
Every ODNI predicate flows from `crates/ism/schemas/ISM-v2022-DEC/`.
Citations are verified at PR time and re-verified at propagation.
Principle VIII (Constitution) is the full contract; SC-009 and FR-021
are the spec-side enforcement.

**Status**: `[LANDED]`. T089 citation-verification pass landed in
PR #154 (commit `cdc0866`); every `Constraint`, `PageRewrite`, and
`TokenMetadataFull` citation re-verified against
`crates/capco/docs/CAPCO-2016.md` and `crates/ism/schemas/ISM-v2022-DEC/`.
Future propagations re-verify per Constitution VIII; FR-021 + SC-009
remain the standing enforcement.

---

## 4 · Memory safety

### 4.1 Edition 2024, `forbid(unsafe_code)` inventory

Every rust crate in the workspace declares either
`#![forbid(unsafe_code)]` or `#![deny(unsafe_code)]`:

| Crate | Directive | File |
|---|---|---|
| `marque-capco` | forbid | `crates/capco/src/lib.rs:5` |
| `marque-scheme` | forbid | `crates/scheme/src/lib.rs:5` |
| `marque-test-utils` | forbid | `crates/test-utils/src/lib.rs:5` |
| `marque-extract` | forbid | `crates/extract/src/lib.rs:5` |
| `marque-ism` | deny | `crates/ism/src/lib.rs:5` |
| `marque-engine` | forbid | `crates/engine/src/lib.rs:5` |
| `marque-config` | forbid | `crates/config/src/lib.rs:5` |
| `marque-core` | forbid | `crates/core/src/lib.rs:5` |
| `marque-wasm` | forbid at module attribute | `crates/wasm/src/lib.rs:31` |
| `marque-rules` | forbid | `crates/rules/src/lib.rs:5` |

`deny` on `marque-ism` is the narrowest opening: a single
`#[allow(unsafe_code)]` is permitted only for `Trigraph::as_str` (§4.2).

### 4.2 Unsafe-block audit

Two justified unsafe blocks ship today, each carrying a `// SAFETY:`
doc comment that documents the precondition and how the surrounding
code satisfies it:

- **`Trigraph::as_str`** (`crates/ism/src/attrs.rs:1085`, allow-local):
  wraps `std::str::from_utf8_unchecked` over a `Trigraph` whose only
  constructors are `try_new` (ASCII uppercase predicate) and the `USA`
  constant. ASCII is valid UTF-8. The SAFETY comment names both
  constructor paths and the ASCII⊂UTF-8 chain that discharges the
  `from_utf8_unchecked` precondition.
- **WASM talc allocator bootstrap** (`crates/wasm/src/lib.rs:101`):
  one-time initialization of the linear-memory heap using
  `&raw mut INITIAL_HEAP`, a Rust 2024 syntax that avoids creating a
  reference that could alias. The SAFETY comment names the
  alias-freedom invariant (no Rust reference is created), the
  one-time-init guarantee, and the module-locality of `INITIAL_HEAP`
  (no other access path exists after the claim).

Test-only unsafe (environment-variable setters in
`crates/config/tests/precedence.rs`) does not ship.

**Status**: `[LANDED]` for the SAFETY-comment discipline; the
audit-on-introduction obligation rolls forward to any future
`unsafe { ... }` block — landing one without a SAFETY comment is a
review-blocking defect.

### 4.3 Panic-free hot-path policy

The hot path (scanner + parser) uses safe indexing and `memchr_iter`;
no `unwrap()` is reachable from adversarial input.

Three deliberate `unwrap()` calls exist in the parser at
`crates/core/src/parser.rs` where the preceding parsed-token-span
predicate guarantees the option is `Some`. Two deliberate `.expect()`
calls exist in `crates/engine/src/batch.rs:196, 226` for the
`ConcurrencyController` semaphore; the panic surfaces via
`BatchError::is_panic()` rather than silently. See gap register (P1-8)
for the graceful-shutdown improvement.

### 4.4 Buffer lifetimes & zeroization posture

**`[NON-GOAL]`**. Buffers are caller-owned; marque does not copy,
extend, or zero them. A process-memory-dump adversary can recover
previously-processed content. Callers needing zeroization should
wrap marque in a process that exits between documents, or wait for a
future SGX/TrustZone-aware build (Constitution II).

---

## 5 · Content handling & data minimization

### 5.1 Spans-not-copies

`Span` is `{ start: usize, end: usize }` (`crates/ism/src/span.rs`).
Scanner output is `MarkingCandidate { span, kind, .. }`. Parser output
is `IsmAttributes` with `Box<[TokenSpan]>` fields. No step in the hot
path materializes a string copy of the marking.

A `to_vec()` call previously appeared on the recognizer dispatch
path and was flagged as gap register #16. Subsequent refactoring
removed it; the only `.to_vec()` in `crates/core/src/parser.rs` today
is at line 2072 inside a `#[test]` block, where it builds a Vec from
a `Box<[DissemControl]>` so a `.contains(&...)` assertion can run.
Tests are not on the hot path, so the zero-copy invariant holds.

### 5.2 Diagnostic message policy

Diagnostic messages interpolate **token canonicals** — the value of the
enumerated CVE token, the rule ID, the authoritative-source citation —
and **span offsets**. They do not interpolate surrounding document
text. Rule authors relying on `format!("{:?}", token)` in a diagnostic
message are producing a token-canonical string, not content.

A written policy formalizing this distinction — human-visible
diagnostics vs machine-ingested audit feeds — is open (gap register
P1 follow-up).

### 5.3 Error-path policy

`CoreError` variants in `crates/core/src/error.rs` currently embed
token strings in `Display` via `{token:?}`. `CoreError` is an internal
type and must not cross into audit records or server responses. The
type system permits `marque_core::CoreError` to be `.to_string()`'d
by a downstream consumer (the type is `pub use`'d from `crates/core/src/lib.rs`),
so the no-leak property is asserted at runtime rather than at the
visibility level.

`crates/engine/tests/core_error_isolation.rs` (gap register #20,
closed in v0.7) embeds a high-entropy ASCII canary in adversarial
input bytes designed to trip every `CoreError` construction site
(`MalformedMarking` over a free-form portion / banner that survives
the scanner but fails the recognizer; `InvalidUtf8` carries a `Span`
only and has no leak vector). It then walks every text-bearing field
of `LintResult` and `FixResult` — `Diagnostic.message`,
`FixProposal.original`, `FixProposal.replacement`,
`AppliedFix.proposal.{original,replacement}`,
`RemainingDiagnostic.message` — and asserts the canary appears in
none of them. A self-test sanity-asserts that
`CoreError::MalformedMarking(canary).to_string()` does carry the
canary; if a future refactor redacts the Display output, that test
fires and the file is retired alongside this section.

The companion property — `CoreError` being literally unreachable
from any non-`marque-core` `.rs` file — is true today by code-review
discipline (workspace grep finds only doc-comment references in
`crates/engine/tests/audit.rs` and `crates/capco/src/scheme.rs`,
plus the declaring module). Tightening this to a `pub(crate)`
visibility is a P3 follow-up tracked alongside the engine API
review.

### 5.4 Logging & telemetry

`MARQUE_LOG=marque=debug|trace` raises verbosity via `tracing`. The
current engine emits only one `tracing::warn!` and one server-startup
`tracing::info!`; neither interpolates document content.

The CLI's `--help` `ENVIRONMENT VARIABLES:` block now carries the
production-safety caveat for `MARQUE_LOG=trace`: trace level is
reserved for future diagnostic output that may interpolate input
fragments and must not be enabled when processing classified content.
The warning is surfaced through clap's `after_help` so an operator
who has never read this whitepaper still sees it. The matching
runtime guard — converting the resolved filter to a stderr notice
when `trace` appears — is deferred until a `tracing::trace!` site
that touches input bytes actually lands; until then, the documentation
is the contract.

### 5.5 Test-fixture provenance

Test fixtures under `tests/corpus/` and
`tests/fixtures/mangled/` are synthetic or derived from the public
Enron corpus (see `tests/fixtures/mangled/README.md`). No real
classified content is committed. The regeneration pipeline requires
`MARQUE_ENRON_CORPUS` and writes a content-free metadata fingerprint.

A corpus-provenance CI test (`marque/tests/corpus_provenance.rs`)
enforces that fixtures contain only CVE-vocabulary tokens and no
classifier identities. The `mangled_fixtures_observed_expected_token_only`
test (gap register #19, closed in v0.7) extends the same discipline
to `tests/fixtures/mangled/**/*.json`: each `observed` / `expected`
field is checked against the prose-sentinel list shared with
`crates/engine/tests/audit.rs::PROSE_SENTINELS`, against a
classifier-id-shaped digit-run heuristic, and against a 256-byte
length cap that forecloses prose leakage independently of the
sentinel list.

---

## 6 · Audit & compliance

### 6.1 `FixProposal` purity

`FixProposal` (`crates/rules/src/lib.rs`) contains `rule`, `source`,
`span`, `original`, `replacement`, `confidence`, `migration_ref`. No
timestamps, no classifier identity, no process ID, no hostname. Rule
crates construct these; snapshots happen only on promotion.

**Status**: `[LANDED]`.

### 6.2 `__engine_promote` visibility invariant

`AppliedFix::__engine_promote` accepts an `EnginePromotionToken` whose
sole field is private to `marque-rules`, so external crates cannot
brace-construct one. The single bypass surface is
`EnginePromotionToken::__engine_construct()`, which is
`#[doc(hidden)]`, named to make bypass intent unmistakable, and called
from exactly one place in production: the private
`engine_promotion_token()` helper in `crates/engine/src/engine.rs`.
That helper feeds the three production promotion sites
(`Engine::fix_inner`'s Apply and DryRun arms, and
`apply_text_corrections`).

Two test-only exceptions exist under the Constitution V Principle V
test-fixture carve-out: `crates/engine/tests/audit.rs` (G13 sentinel
sweep input) and `marque/src/render.rs` (audit-emitter unit test).
Each call site carries an inline carve-out comment.

The seal is enforced at the type level for brace construction
(rejected by Rust's privacy rules) and by convention for the
`__engine_construct()` and `__engine_promote()` doors (the
`#[doc(hidden)]` engine-only contract documented on each). A grep
for `EnginePromotionToken` outside `marque-engine` (and outside
test code covered by the carve-out) flags every Constitution V
violation in one pass — the bypass surface used to be a single
generic-named function (`__engine_promote`); now it is a single
named type.

Tests pin both halves:

- `crates/rules/src/lib.rs` carries a `compile_fail` doctest on
  `EnginePromotionToken` proving brace construction is rejected by
  the privacy gate at the doctest's separate-crate compile.
- `crates/rules/tests/engine_promotion_seal.rs::documented_door_can_mint_token_from_outside_marque_rules`
  exercises the documented door from outside `marque-rules`,
  proving the bypass surface is callable when needed.

**Status**: `[LANDED]` call-site discipline; `[LANDED]` type-level
seal (gap register #5 closed).

### 6.3 Confidence gate, FR-016 sort, C-1 overlap guard

- Threshold filter: `Engine::fix_inner` rejects proposals whose combined
  confidence is below the configured threshold.
- FR-016 sort: fixes sort by reverse span order (end DESC, start DESC,
  rule_id ASC, replacement ASC), so application does not shift
  later spans.
- C-1 overlap guard: a fix whose span overlaps an already-accepted fix
  is dropped with no panic.
- Construction-time validation: `Confidence::validate` enforces each
  axis is finite and in `[0, 1]` where constrained.

A malformed rule that constructs an out-of-range `Confidence` would
otherwise panic at `FixProposal::new`. `Engine::lint` wraps every
`Rule::check` call in `std::panic::catch_unwind` (gap register #10,
closed in v0.10) — a buggy rule's panic is now logged via
`tracing::warn!` at target `marque_engine::rule_panic` (naming the
rule's `RuleId` and the panic's payload string) and the rule is
skipped for that candidate; sibling rules and remaining candidates
keep running. The fix path uses the same loop, so it gets the same
guarantee transitively. `crates/engine/tests/rule_panic_isolation.rs`
pins the contract end-to-end against three failure modes: a bare
`panic!()`, the canonical out-of-range `Confidence` failure (calls
real `FixProposal::new`), and a sibling-rules-continue assertion;
plus a smoke test that the production CAPCO rule set still emits
diagnostics after the wrapper landed.

The wrapper requires `panic = "unwind"` in the release profile —
`Cargo.toml` `[profile.release]` had `panic = "abort"`, which would
have aborted the process before the catch could fire. Switched to
unwind; the binary-size and runtime cost (~5-10% / ~1-3%) is
acceptable for a compliance tool where one rule's defect must not
trip a complete service outage. The change also makes
`BatchError::is_panic()` (`crates/engine/src/batch.rs`) actually
fire in release builds — `tokio::task::spawn_blocking` reports a
`JoinError::Panic` only on unwound worker threads, not aborted ones.

**Status**: `[LANDED]`.

### 6.4 Audit record schema versioning

Audit records are NDJSON on stderr. Two schemas coexist:
`marque-mvp-1` (`specs/001-marque-mvp/contracts/audit-record.json`,
12-field shape) and `marque-mvp-2`
(`specs/004-constraints-decoder-vocab/contracts/audit-record-v2.md`,
strict superset adding `recognition`, `runner_up_ratio`, and
`features: Vec<FeatureContribution>` with `FeatureId` enum values).

FR-014 requires **single-schema-version-per-build**:
`MARQUE_AUDIT_SCHEMA` is a build-time flag, not a runtime negotiation.
Mixed-schema streams are impossible because the codepath is resolved
at compile time. The flag is read by `crates/engine/build.rs`,
validated against `["marque-mvp-1", "marque-mvp-2"]` (panic on
anything else), and surfaced as `pub const
marque_engine::AUDIT_SCHEMA_VERSION` plus the const-folded selector
`AUDIT_SCHEMA_IS_V2`. The CLI emitter (`marque/src/render.rs`) and
the WASM emitter (`crates/wasm/src/lib.rs`) dispatch on the selector
to emit either `AuditRecordJsonV1` or `AuditRecordJsonV2`. v2 ⊃ v1
back-compat is pinned by
`crates/engine/tests/audit.rs::v1_records_parse_in_v2_consumer`
(T054); the stream-level single-schema invariant is pinned by
`marque/tests/cli_fix.rs::audit_stream_uses_only_one_schema_version`
(T055).

**Status**: `[CLOSED]` — both schemas wired through PR-4 (gap
register P0-1 closed). PR-4b adds decoder-sourced records
(`FixSource::DecoderPosterior`) with non-empty `features` and a
`runner_up_ratio` once `Engine::fix_inner` learns to consume the
decoder dispatcher's `Parsed::Unambiguous` candidates.

### 6.5 Classifier identity handling

Identity is read from `MARQUE_CLASSIFIER_ID` env var or
`.marque.local.toml[user].classifier_id` (`crates/config/src/lib.rs`),
never from `.marque.toml`. Identity is stamped into `AppliedFix` at
promotion; the only emission path is the NDJSON audit record on stderr.
`--explain-config` emits `"classifier_id_present": bool`, never the
value.

Forgery resistance is a **`[NON-GOAL]`** — see §2.5 / T9. The audit
stream records claimed identity; cryptographic proof is a deployment
concern.

**Status**: `[LANDED]` for sourcing, storage, and emission;
`[NON-GOAL]` for forgery resistance.

### 6.6 Dry-run correctness

`--dry-run` produces full audit output via the same `__engine_promote`
call with `dry_run = true` and returns the source unchanged. No file
write, no stdout write in this path. Tested in
`crates/engine/src/engine.rs` (`dry_run_returns_original_source_but_records_applied`).

**Status**: `[LANDED]`.

### 6.7 `Severity::Off` as non-firing

The rule loop checks `configured_severity == Severity::Off` before
invoking `rule.check()` (`crates/engine/src/engine.rs:295`). No
`Off`-severity diagnostic can be produced. Pre-scanner C001 corrections
are gated identically.

**Status**: `[LANDED]`.

### 6.8 `FixResult`-never-cached

Constitution V prohibits caching `FixResult`; only `LintResult` may be
cached (v0.2 feature). The cache is not yet implemented; the
constraint is enforced prospectively in design.

**Status**: `[PLANNED]` v0.2; rule stands.

---

## 7 · Build-time security

### 7.1 `build.rs` integrity

`crates/ism/build.rs` parses ODNI ISM schema files with `quick-xml`
(event-driven). No DTD resolution, no network I/O, no parent-directory
writes. All generated files land under `OUT_DIR`.

`crates/engine/build.rs` reads the `MARQUE_AUDIT_SCHEMA` environment
variable and emits a const; no parsing, no I/O.

### 7.2 Schema version pinning

`crates/ism/Cargo.toml`'s `[package.metadata.marque] ism-schema-version`
is verified inside `build.rs` against the constant
`SCHEMA_VERSION = "ISM-v2022-DEC"`. Drift between Cargo metadata, the
build-script constant, and the on-disk schema directory name panics
the build.

### 7.3 Vendored authoritative sources

- `crates/capco/docs/CAPCO-2016.md` / `.pdf` — public-domain; used for
  human reference and rule citations.
- `crates/ism/schemas/ISM-v2022-DEC/` — public-domain ODNI schemas;
  consumed by `build.rs`.

SHA256 checksums cover both trees:

- `crates/capco/docs/original-refs/SHA256SUMS` — CAPCO PDFs, PPTs, and
  historical reference notes.
- `crates/ism/schemas/ISM-v2022-DEC/SHA256SUMS` — all 756 ODNI schema
  files (XSD, RNG, SCH, XSL, CSV, JSON, XML).

The `refs-integrity` CI job runs `sha256sum -c` against both files on
every PR. A silent replacement of any vendored reference fails CI with
a named mismatched file. Intentional updates require a matching edit
to the SHA256SUMS file in the same PR — which is visible in diff review
and tied to Principle VIII re-verification of affected citations.

### 7.4 Corpus / prior pipeline

`tools/corpus-analysis/analyze.py` regenerates `crates/capco/corpus/priors.json`
from the Enron corpus gated on `MARQUE_ENRON_CORPUS`. The fingerprint
written with `priors.json` is a hash of file metadata (path, size,
mtime) — not content — so regenerated priors never accrete document
bytes into the repo.

Phase D's `build.rs` will consume `priors.json` if present; corpus
override at runtime (§10) is the only caller-controlled prior channel,
and it is gated per surface.

`tools/corpus-analysis/requirements.txt` pins `requests==2.33.1`
exactly (gap register #17, closed in v0.7) so a `pip install -r
requirements.txt` in CI or a contributor sandbox is reproducible —
an upstream PyPI release cannot silently change the bytes that get
installed and re-shape `priors.json` or the mangled fixtures. The
shebang on `analyze.py` is `#!/usr/bin/env -S uv run --script`; uv
inline-metadata pinning (PEP 723) and `pip-tools`-style hash pinning
across the transitive set (`requests` pulls in `charset-normalizer`,
`idna`, `urllib3`, `certifi`) are tracked as follow-ups.

### 7.5 Generated-code determinism

No timestamps, no RNG, no external calls in any `build.rs`. Output
files are written via formatted strings. `Cargo.lock` is committed.

**Status**: `[LANDED]` for §§7.1–7.5.

---

## 8 · Supply chain & dependency hygiene

### 8.1 `cargo-deny` policy and CI gate

`deny.toml` at repo root enforces:

- **Advisories**: RustSec `advisory-db`.
- **Licenses**: allow-list (Apache-2.0, MIT, BSD-2/3-Clause, ISC,
  Zlib, CC0-1.0, CC-BY-4.0, LicenseRef-MarqueLicense-1.0).
- **Sources**: only `https://github.com/rust-lang/crates.io-index`.
- **Git deps**: disallowed in primary crates.

`.github/workflows/ci.yml` runs `cargo-deny-action` (pinned SHA) on
every PR.

### 8.2 Lockfile & registry discipline

`Cargo.lock` is committed. No git dependencies in the workspace. No
non-crates.io registries in use.

### 8.3 Action pinning

All GitHub Actions are pinned to commit SHAs (not `@vN` tags).

### 8.4 REUSE & licensing

`REUSE.toml` annotates every file's SPDX license and copyright. Public-
domain schemas/docs are tagged as such. The `reuse` CI job runs
`reuse lint` on every PR; drift in SPDX headers, missing license text,
or mis-annotated files fails the build with a named diagnostic.

### 8.5 Workspace licensing posture

All marque source is under the **Marque License 1.0**
(`LicenseRef-MarqueLicense-1.0`) — including the WASM-safe set
(`marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`,
`marque-capco`), the engine, the integration surfaces, and shared
infrastructure. See `LICENSE.md` at the workspace root for terms. The
prior permissive-core / commercial-integrations split (Apache-2.0 on
the WASM-safe set, Elastic/commercial on integrations) was retired in
Constitution v1.2.0 — permissive licensing on the engine core exposed
marque to hyperscaler commoditization. ML-1.0 is source-available with
commercial-use restrictions that preclude a managed-API competitor
while preserving every legitimate integration path (self-hosted,
browser extension, CLI, IDE plugin, enterprise on-prem, WASM embed).

**Dependency hygiene under ML-1.0**: marque crates may depend on
permissively-licensed crates (the `deny.toml` allow-list in §8.1
covers the acceptable SPDX set); they must not depend on copyleft
(GPL/LGPL/AGPL/MPL) or competing source-available licenses (Elastic
License 2.0, BSL, SSPL). Two CI gates enforce this:

1. **Workspace scope** (`deny` job + `deny.toml`) — applies to every
   crate in the workspace, gating advisories, licenses, and sources.
2. **WASM-safe subgraph scope** (`deny-wasm-safe` job +
   `deny.wasm-safe.toml`) — applies a stricter allow-list to the
   WASM-safe set (`marque-ism`, `marque-core`, `marque-rules`,
   `marque-scheme`, `marque-capco`) and their transitive closure.
   The exclusion list in the job explicitly names every non-WASM-safe
   workspace member; `--exclude-dev` drops test-only deps. A new
   workspace member landing MUST be classified at the same PR —
   either added to the exclude list or left in the WASM-safe check.
   A copyleft dep that sneaks in only through `marque-server` or the
   CLI will pass the workspace-wide gate (if the workspace gate ever
   relaxes) but still trips this narrower gate for WASM.

### 8.6 NPM demo & site

- `demo/package.json` has zero lifecycle hooks
  (`postinstall`, `preinstall`, `prepare`, `prepublish` all absent).
- `site/` (Astro 6) vendors the three site fonts locally — OCR-B
  (display, brand mark), Fira Code (monospace, code blocks), and IBM
  Plex Sans (body text) — under `site/src/assets/{OCR-B,Fira-Code,IBM-Plex-Sans}/`,
  each with a `LICENSE` (SIL OFL 1.1 for Fira Code and IBM Plex Sans;
  per-package terms for OCR-B) and a `README.md` documenting the
  exact upstream package and version that produced the bytes. The
  `astro.config.mjs` font block uses `fontProviders.local()`
  exclusively; `fontProviders.fontsource()` (which fetches from
  `api.fontsource.org` + `cdn.jsdelivr.net` at build time) is not
  used. The build is reproducible offline and the bytes that ship
  to the browser are the bytes committed to this repo.

### 8.7 Release provenance

Release artifacts publish to crates.io via OIDC token exchange
(`rust-lang/crates-io-auth-action`). Every non-dry-run release also
produces a signed source archive and three SBOM files via
`actions/attest-build-provenance`:

1. `git archive` produces a workspace-scoped `marque-<version>.tar.gz`
   from the release tag. This is **not** equivalent to any crates.io
   per-crate tarball — crates.io packages are produced per-crate by
   `cargo package` with per-crate `include`/`exclude` rules and will
   not match byte-for-byte. The GitHub-released archive is a
   separately verifiable workspace-state provenance artifact, not a
   mirror.
2. `reuse spdx` generates a `marque-<version>.spdx` (SPDX tag-value
   format) from the existing REUSE annotations that `reuse lint`
   already enforces on every PR (§8.4). This SBOM covers every file's
   license and copyright and is NTIA minimum-elements compliant.
   The `reuse` version is pinned to the same `5.0.2` used by the `ci.yml`
   `reuse` job so behavior is consistent across lint and generation.
3. `cargo cyclonedx` generates `marque-<version>.cyclonedx.json` and
   `marque-<version>.cyclonedx.xml` by walking `Cargo.lock` and emitting
   the full transitive dependency graph with versions, licenses, and
   checksums. CycloneDX is the format most SBOM consumers and
   vulnerability scanners (Grype, Trivy, Dependency-Track) expect for
   supply-chain analysis.
4. `attest-build-provenance` signs the source archive and each SBOM file
   keylessly via Sigstore using the GitHub OIDC token and records the
   attestations in GitHub's transparency log.
5. All four artifacts are attached to the GitHub release.

Consumers verify with:

```
gh attestation verify marque-<version>.tar.gz --owner marquetools
gh attestation verify marque-<version>.spdx --owner marquetools
gh attestation verify marque-<version>.cyclonedx.json --owner marquetools
gh attestation verify marque-<version>.cyclonedx.xml --owner marquetools
```

The release workflow holds `id-token: write` + `attestations: write`
permissions so it can obtain the OIDC token and record the
attestation. Cosign is not installed — GitHub's Sigstore-backed
attestation path is narrower and avoids adding a separate CLI
to the release surface.

The archive / SBOM / attest / release steps gate on `dry-run == false`
only (not on tag-creation freshness), so a re-triggered release run for
an existing tag still completes the attestation + GitHub-release work.
`softprops/action-gh-release` upserts; re-attesting an already-attested
artifact records a fresh signature bound to the same subject digest.

All actions in the release workflow are SHA-pinned per §8.3.
`cargo-cyclonedx` is pinned to `0.5.7` via `cargo install --version 0.5.7 --locked`.

**Status**: `[LANDED]` for §§8.1–8.7 including SBOM generation (issue #191).
(§8.5 landed via Constitution v1.2.0; §8.4 `reuse lint` and §8.7
release-archive attestation landed alongside the initial whitepaper; SBOM
generation landed in v0.16. The narrower WASM-safe-subgraph CI gate
remains a gap — see gap register 14.)

---

## 9 · Runtime security & robustness

### 9.1 Input-size & span bounds

- Portion-candidate span length is clamped to `[3, 256]` bytes in the
  scanner.
- Banner / CAB candidate length is not clamped; pathological
  full-buffer candidates are possible but are linear in cost.
- `Engine::lint` has no per-document max-size parameter. Per-document
  limits are a deployment concern for the server surface (§10.2).

### 9.2 Aho-Corasick DoS posture

The pre-scanner `CachedAhoCorasick` is built once from a size-bounded
corrections map. Standard Aho-Corasick is immune to overlap-triggered
exponential blowup; no pathological corrections pattern is shippable
without PR review.

### 9.3 Decoder bound K=8 per template

Phase D's `DecoderRecognizer` enumerates at most K=8 candidates per
grammar template (`specs/004-constraints-decoder-vocab/research.md`
§Decoder scope; plan §5.2). The strict-context floor runs first: if
any CONFIDENTIAL+ marking already exists, ambiguous local markings
resolve deterministically and the decoder is not consulted.

**Wiring (T1, FR-011, T045/T062, landed in PR #114).** Two
mechanisms enforce the floor:

- *Strict-evidence gate.* `DecoderRecognizer::recognize`
  (`crates/engine/src/decoder.rs:148-159`) checks
  `cx.strict_evidence` before any prior consultation and returns
  `Parsed::Ambiguous { candidates: vec![] }` when set. The engine
  drives this from the deep-scan opt-in
  (`crates/engine/src/engine.rs:369-374`,
  `strict_evidence: !self.deep_scan`), so by default — i.e.,
  without `--deep-scan` — the decoder is never invoked.
- *Per-page classification floor.* When deep-scan is active, the
  engine accumulates the highest strict-path classification rank
  on the current page in `classification_floor`
  (`crates/engine/src/engine.rs:338-419`), threads it through
  `ParseContext`, and the decoder drops any candidate below the
  floor at `decoder.rs:251-257`. Decoder-path recognitions never
  raise the floor — only strict-path recognitions do — so a
  misrecognition cannot self-justify by lifting the threshold it
  then clears. The floor resets on `MarkingType::PageBreak` per
  Constitution VI's reset-before-parse invariant.

Tests: `decoder_defers_to_strict_when_strict_evidence_is_set`
(`crates/engine/src/decoder.rs:1320`) pins the gate;
`unclassified_candidate_rejected_below_secret_floor`,
`floor_at_equal_level_accepts_candidate`,
`floor_below_candidate_accepts_higher_level`, and
`no_floor_accepts_any_classification`
(`crates/engine/tests/decoder_recovery.rs:167-249`) pin the
classification-floor behavior.

### 9.4 `BatchEngine` concurrency

`recoco-utils::ConcurrencyController` provides row + byte semaphores
(`crates/engine/src/batch.rs`). Defaults: 32 in-flight documents, no
byte cap unless set. CPU-bound work runs on
`tokio::task::spawn_blocking`. Results stream in completion order;
callers correlate via echoed `id`.

A closed `ConcurrencyController` (runtime shutdown, explicit semaphore
close) surfaces as `BatchError::ShutdownInProgress` per document
(gap register #8, closed in v0.9). The previous `.expect("…")`
panicked, propagating through `JoinError::is_panic()` and triggering
spurious supervisor alerts on a routine end-of-life signal. The new
variant is distinct from `is_panic()` and `is_cancelled()`, has a
matching `is_shutdown()` predicate, and `Display` names the state
explicitly so log greps on operator dashboards pick it up.

### 9.5 Page-context reset-before-parse

The engine resets `PageContext` **before** attempting to parse a
page-break candidate (`crates/engine/src/engine.rs:225`). A malformed
page-break candidate cannot block the reset; banner rules never see
prior-page state.

### 9.6 Fix-loop termination

`apply_text_corrections` sorts fixes in reverse span order and applies
an overlap guard. The loop is O(n log n) over diagnostic count.
Unbounded fix loops are impossible.

### 9.7 Timeouts & cancellation

**`[LANDED]` (spec 005, v0.13).** The engine accepts a per-call
deadline and cooperatively aborts at well-defined boundaries. The
prior framing (deployment concern; engine synchronously blocks) was
correct as a worst-case but left every surface — CLI, server, WASM,
batch — to invent its own timeout shim, none of which could see
inside the lint loop. Spec 005 lands a uniform deadline parameter at
the engine and threads it through every surface.

**Per-call options surface.** `LintOptions { deadline: Option<Instant> }`
and `FixOptions { deadline: Option<Instant>, threshold_override:
Option<f32> }` (`crates/engine/src/options.rs`) are both
`#[non_exhaustive]` and reach the engine via two new methods:
`Engine::lint_with_options(&[u8], &LintOptions) -> LintResult` and
`Engine::fix_with_options(&[u8], FixMode, &FixOptions) -> Result<FixResult, EngineError>`
(`crates/engine/src/engine.rs`). The pre-existing `Engine::lint` /
`Engine::fix` / `Engine::fix_with_threshold` shims now delegate to
the `_with_options` paths with `Default::default()` options — zero
behavior change for callers that don't set a deadline.

The deadline is an absolute `Instant` (not a `Duration`), so a slow
preceding stage (concurrency-controller wait, HTTP middleware, batch
permit acquisition) cannot consume the budget — the caller stamps
`Instant::now() + duration` at the boundary it cares about, and the
engine carries no implicit clock. `web_time::Instant` is re-exported
as `marque_engine::Instant` so the same type works on native and
`wasm32-unknown-unknown` (where `std::time::Instant::now()` panics).

**Cooperative cancellation.** The engine checks the deadline at
three boundaries:

1. Pre-pass, before the scanner runs (`Engine::lint_with_options`
   top of body). An already-expired deadline returns
   `LintResult { truncated: true, candidates_processed: 0,
   candidates_total: 0, diagnostics: vec![] }` immediately for
   lint, or `Err(EngineError::DeadlineExceeded { partial_lint })`
   for fix.
2. Per-candidate, at the top of the candidate iteration loop
   (`Engine::lint_with_options`). On expiry the loop breaks with
   `truncated: true` and `candidates_processed` set to the count
   completed so far; `candidates_total` is the scanner's full
   candidate count.
3. Pre-fix-loop and per-fix-application, in `Engine::fix_inner`.
   `fix_inner` calls `lint_with_options` with the same deadline used
   for fix application — the budget is shared across both passes, not
   per-pass. So the `partial_lint` carried in
   `EngineError::DeadlineExceeded` distinguishes two trip points via
   `partial_lint.truncated`:
   - **Lint-time trip** (`partial_lint.truncated == true`) — the
     deadline expired during lint; `partial_lint.candidates_processed
     < candidates_total` shows how far the lint pass got. The server
     surfaces this as `truncated_by: "lint"` in the 504 body.
   - **Fix-time trip** (`partial_lint.truncated == false`) — lint
     completed and the deadline tripped at the pre-fix check or
     during fix application. `partial_lint` is the complete
     `LintResult`; only the fix application is partial. The server
     surfaces this as `truncated_by: "fix"` in the 504 body.

   The asymmetric-response invariant (no partial `FixResult` ever
   constructed) holds in both cases — lint is observation, fix is
   commitment, and the deadline trip never produces a half-applied
   fix regardless of which boundary fires.

There is no preemptive cancellation. A pathological rule that
spins for minutes inside a single candidate would still complete
that candidate before the next deadline check. The spec records
this as an explicit non-goal — preemption would require either
panic-as-control-flow (rejected under panic-free hot-path policy,
§4.3) or a separate worker thread per document (rejected under
zero-copy / streaming-core, §3.2). The boundary granularity is
calibrated to the fix-loop and candidate-loop scale (microseconds
to single-digit milliseconds per iteration on the SC-001 corpus),
which is fine enough that an operator-set 30 s deadline trips
within tens of microseconds of expiry on hardware where
`Instant::now()` is a vDSO read.

**Asymmetric response shape (Constitution V Principle V).** Lint
returns a partial `LintResult` with `truncated: true`; fix returns
`Err(EngineError::DeadlineExceeded { partial_lint })`. The split is
load-bearing: a partial `FixResult` would commit half a fix to the
audit stream (mid-document text rewrite, audit record emitted, second
half of the rewrite never executed), violating audit-record integrity.
A truncated lint commits nothing — diagnostics are pure observation
— so the partial result is safe to return.

`EngineError` (`crates/engine/src/errors.rs`) is the new runtime-error
enum, distinct from the existing `EngineConstructionError` (build-time
errors). Variants today: `DeadlineExceeded { partial_lint: LintResult }`,
`InvalidThreshold(InvalidThreshold)`. The enum is `#[non_exhaustive]`
so future runtime-error variants land without a semver break.

**Per-surface wiring summary.** Each surface stamps its own deadline
at the boundary it owns; the engine sees only `Option<Instant>`.

| Surface | Stamping site | Surface error |
|---|---|---|
| CLI (`marque`) | `--deadline <humantime>` accepted by `clap` as `Option<String>`, then parsed via `humantime::parse_duration` in `validate_deadline()`; `Instant::now() + d` per invocation | `EX_TEMPFAIL` (75) on fix expiry; stderr warning + truncated render on lint expiry |
| Server (`marque-server`) | `X-Marque-Deadline: <u64-ms>` header (or per-endpoint default 30 s); `Instant::now() + d` per request, **after** body deserialization | `400` on bad header; `504` on fix expiry; `200 + Marque-Truncated: true` on lint expiry |
| WASM (`marque-wasm`) | `deadline_ms: f64` on the JS-side options object; `Instant::now() + d` per call, validated `is_finite() && >= 0.0` | JS error string on overflow / invalid; mirrors server `504` body shape on fix expiry |
| Batch (`BatchEngine`) | `BatchOptions::per_doc_deadline: Option<Duration>`; **`Instant::now() + d` AFTER permit acquisition** so concurrency-controller wait does not consume the budget | `BatchError::DocumentDeadlineExceeded { partial_lint }` per-document; `is_deadline_exceeded()` predicate distinct from `is_panic()` / `is_shutdown()` / `is_cancelled()` |

The batch case is the subtle one: stamping `Instant::now() + d` at
permit acquisition (rather than at `lint_many` entry) means an
earlier slow document holding the semaphore does not eat into a
later fast document's budget. Each document gets its full
configured deadline starting from the moment it begins work.

**Constitution III analysis.** The deadline parameter does not
introduce a new recognizer codepath, does not alter recognizer
posteriors, and does not change the vocabulary surface. It is a
runtime budget cap on existing behavior, not a semantic surface.
This is explicitly permitted under Principle III's WASM
runtime-config restriction — only changes that expand the
engine's semantic surface (e.g., loading decoder priors at runtime)
are forbidden through the WASM boundary.

**Not on the deadline path.** Build-time work
(`marque-ism/build.rs`, corpus bake), the `Engine::new` constructor
(rule-set wiring, scheduler validation), and the WASM `Engine`
cache hit (`crates/wasm/src/lib.rs::with_engine`) all run without
a deadline check. These are bounded by code size, not input size.

**Tests**: `crates/engine/tests/deadline.rs` (Phase 1+2 — types,
shim equivalence, pre-pass / per-candidate / per-fix trips,
generous-deadline-runs-to-completion sanity);
`crates/engine/benches/deadline_overhead.rs` (overhead bench;
permissive 10 % gate today, target 2 %, gated by host-clock
variance — see T018 in `specs/005-engine-deadlines/tasks.md`);
`marque/tests/cli_deadline.rs` (CLI surface, baseline-derived
truncation, JSON-vs-human output discipline);
`crates/server/tests/http_deadline.rs` (header parsing, default,
truncation, 504 / 400 / 504 vs config-issue 500, duplicate-header
rejection, ordering invariant);
`crates/wasm/tests/deadline_parity.rs` (WASM↔native byte-identical
NDJSON parity at generous and zero deadlines);
`crates/engine/tests/batch_deadline.rs` (per-doc isolation, lint
truncation as `Ok`, no-deadline sanity, overflow does not silently
disable the budget, variant matchability / predicates / Display /
source).

**Status**: `[LANDED]` for §§9.1 (partial, see gap)–9.7.

---

## 10 · Trust boundaries by surface

### 10.1 CLI (`marque`)

- Accepts `--corpus-override <file>` (T3 CLI-side; principal is the
  operator).
- Accepts `--deadline <humantime>` (e.g., `30s`, `2m`) on `check` and
  `fix`; per-invocation `Instant::now() + d` stamping. `--deadline 0`
  / `--deadline 0s` / unparseable values surface as `EX_USAGE` (64).
  Truncated `LintResult` from `check` renders normally with a final
  stderr `"⚠ deadline exceeded: covered N/M candidates"` line (Human
  format only — JSON format suppresses trailing narration to keep
  the NDJSON stream pipeable). `EngineError::DeadlineExceeded` from
  `fix` exits `EX_TEMPFAIL` (75) with stderr explanation. The
  `--dry-run` re-lint reuses the same `FixOptions` so the deadline
  applies across primary + replay (a single budget covers both).
- Emits audit records to stderr.
- No shell invocation; paths come from `clap` as `PathBuf` values.

**Status**: `[LANDED]`.

### 10.2 `marque-server`

- Endpoints today: `POST /v1/lint`, `POST /v1/fix`, `GET /v1/health`,
  `GET /v1/schema/version`.
- **No authentication middleware wired.** Tower layers for auth,
  rate limiting, and structured logging are specified and un-wired;
  this is called out explicitly in `CLAUDE.md`.
- **Body-size cap: landed (gap register #6, closed in v0.9).** axum's
  `DefaultBodyLimit::max(N)` Tower layer is applied via
  `marque_server::build_app_with_limit` (the default
  `marque_server::build_app` calls it with
  `DEFAULT_BODY_LIMIT_BYTES = 10 * 1024 * 1024`, i.e., 10 MiB). The
  `marque-server` binary entry point reads `MARQUE_MAX_BODY_BYTES`
  via `marque_server::resolve_body_limit` and aborts with
  `EX_USAGE` (64) on a value that fails to parse or is below the
  1 KiB floor. The resolved cap is logged on the `listening` line.
  Oversize requests surface as `413 Payload Too Large` before
  reaching the handler;
  `crates/server/tests/http.rs::body_above_explicit_limit_is_rejected_with_413`
  + `fix_endpoint_honors_body_limit` exercise the gate at a
  4 KiB test cap, and `default_limit_admits_realistic_traffic`
  asserts a 256 KiB body fits under the production default
  (catches a regression that lowered the constant).
- **T3 corpus-override rejection: landed.** Every HTTP request is
  inspected at the handler for a corpus-override payload across three
  channels — JSON body field `corpus_override`, header
  `X-Marque-Corpus-Override`, and query-string parameter
  `corpus_override=...` / `corpus-override=...` (all case-insensitive).
  **Presence on any channel returns `400 Bad Request`.** Each channel
  observes presence without materializing caller-supplied contents:
  the body-field guard uses a custom `PresenceMarker` deserializer so
  any value shape (`null`, `{}`, `[]`, strings, numbers, booleans) is
  rejected on key presence alone; the header guard uses
  `HeaderMap::contains_key`; the query guard decodes param names via
  `form_urlencoded::parse` so percent-encoded variants like
  `?corpus%5Foverride=1` cannot bypass the match. Rejection emits a
  `tracing::warn!` entry at target `marque_server::t3` naming the
  channel only — the payload is never materialized, stored, or
  logged.
  Implementation in `crates/server/src/lib.rs::reject_if_corpus_override`;
  tests in `crates/server/tests/http.rs` (T049, T050, plus the
  percent-encoded, case-insensitive, multiple-param, and empty-value-
  shape variants).
- **Per-request deadline: landed (gap register #7, closed in v0.13).**
  `X-Marque-Deadline` header carries an unsigned-integer count of
  milliseconds (e.g., `X-Marque-Deadline: 30000` for 30 s). Parsed via
  `str::parse::<u64>()` — no `humantime` dep on the server graph (the
  WASM-safe allow-list cannot drag it in transitively). `MARQUE_MAX_DEADLINE`
  env var sets the cap (default `60000` = 60 s, accept range
  `[MIN_DEADLINE_MS=1, MAX_DEADLINE_CAP_MS=600000]` = 1 ms to 10 min);
  values that fail to parse or fall outside the range fail fast at
  startup with `EX_USAGE` (64) — same shape as the body-size resolver.
  Per-endpoint default deadline 30 s when the header is absent
  (`DEFAULT_ENDPOINT_DEADLINE_MS=30000`). Resolved cap logged on the
  startup `listening` line as `deadline_cap_ms`.
  Header validation (`resolve_request_deadline` in
  `crates/server/src/lib.rs`) rejects negative / non-numeric / overflow
  / below 1 ms / above the configured cap with `400 Bad Request`, runs
  **before** JSON body deserialization so a bad header takes
  precedence over body validation (`422`), and uses
  `headers.get_all().iter()` to reject duplicate `X-Marque-Deadline`
  headers (a single client setting two values is a configuration bug
  the server surfaces explicitly rather than silently picking one).
  `stamp_request_deadline` maps `Instant::now().checked_add(d)` overflow
  to `500 Internal Server Error` (the configured cap allows a duration
  the host's monotonic clock can't represent — a server misconfiguration,
  not a client error). Truncated lint surfaces as HTTP 200 with the
  partial body and a `Marque-Truncated: true` response header for
  clients that don't deserialize the full body.
  `EngineError::DeadlineExceeded` from fix surfaces as HTTP 504
  Gateway Timeout with `DeadlineExceededBody { truncated_by,
  diagnostics, error_count, warn_count, fix_count, candidates_processed,
  candidates_total }`. The `fix_handler` catch-all maps any future
  unknown `EngineError` variant to `500` so an enum addition cannot
  accidentally surface as `422`. Tests in
  `crates/server/tests/http_deadline.rs` cover header parsing
  (zero / non-numeric / negative / above-cap / just-above-configured-cap
  / overflow / duplicate-header), default-deadline behavior (no
  `Marque-Truncated` on the happy path), truncation, the 504 fix path,
  and the validate-header-before-body ordering invariant.

**Status**: `[LANDED]` for T3 corpus-override enforcement (T049 / T050
/ T066), the body-size cap, and the per-request deadline (gap #7
closed in v0.13); `[PARTIAL]` for the broader surface (auth
middleware still un-wired).

### 10.3 `marque-wasm`

- `default-features = false` on the `marque-engine` dependency in
  `crates/wasm/Cargo.toml` excludes the `corpus-override` codepath at
  compile time.
- JS-facing API accepts only UTF-8 text and a serde-typed config
  object; no raw pointers cross the bridge.
- Talc allocator with 100 KB initial heap; linear-memory growth is
  bounded by the browser/runtime, not by marque.
- **T3 compile-fail guard: landed.** Three independent layers:
  (1) `corpus-override` is not declared in `crates/wasm/Cargo.toml
  [features]`, so `--features corpus-override` on this crate fails
  with cargo's "package does not have feature" error before any
  Rust compilation runs; (2) a `#[cfg(all(target_arch = "wasm32",
  feature = "corpus-override"))] compile_error!(...)` in
  `crates/wasm/src/lib.rs` fires if the feature reaches the crate
  via a dependency-edge change; (3)
  `crates/wasm/tests/no_corpus_override.rs` carries a sibling
  `compile_error!` under `#[cfg(feature = "corpus-override")]` and a
  trivial `#[test]` so `cargo test -p marque-wasm` fails loudly on
  any future drift (T051 / T067). The `corpus-override` cfg name is
  declared at the workspace level in `Cargo.toml
  workspace.lints.rust` check-cfg so rustc does not warn about the
  deliberately-undeclared feature probe.
- **Per-call deadline: landed (gap register #7 partial closure,
  v0.13).** `WasmConfig.deadline_ms: Option<f64>` accepted on the
  JS-side options object passed to `lint` / `fix`. Validation in
  `parse_deadline_ms` requires `is_finite() && >= 0.0` — `serde_json`
  rejects most pathological inputs at JSON-parse time; the explicit
  `is_finite()` check is the second line of defense against `NaN` /
  `±Inf` from a hand-rolled JSON producer. Per-call stamping at
  `Instant::now().checked_add(Duration::from_millis(...))`; overflow
  surfaces as a JS error string (the host's `Performance.now()`
  origin doesn't permit the requested deadline). The engine sees
  `Option<Instant>` only; `web_time::Instant` is the
  `Performance.now()` polyfill on `wasm32-unknown-unknown` and a
  literal `pub use` of `std::time::Instant` on native (so the
  shared-engine path stays one codepath). `EngineError::DeadlineExceeded`
  from fix returns `Err(...)` carrying a JSON-serialized
  `DeadlineExceededBodyJson` that mirrors the server's 504 response
  body shape, so a JS-side caller can dispatch on the same fields
  whether it talks to the WASM module or the HTTP endpoint.
  Constitution III analysis (recorded in the crate-level doc comment):
  `deadline_ms` does not introduce a new recognizer codepath, does
  not alter posteriors, and does not change the vocabulary surface.
  The deep-scan entry points (`lint_deep_scan` / `fix_deep_scan`,
  Gate 2 / FR-019) deliberately remain byte-only — runtime-tunable
  decoder priors would expand the recognizer surface and are
  forbidden through the WASM boundary; deadline is a budget cap, not
  a recognizer surface, so it is permitted. `WasmConfigCacheKey` (the
  cache-key projection used by `with_engine`) **excludes**
  `deadline_ms`: the cache stores a configured `Engine`, and a
  per-call deadline does not change what the engine *is*. Cache hits
  also avoid building a fresh `Config` on the hot path —
  `with_engine` accepts `FnOnce() -> Result<Config, String>` and only
  calls it on cache miss. SC-008 byte-identical NDJSON parity is
  preserved at both deadline shapes (generous deadline = full lint;
  zero deadline = empty NDJSON), validated by
  `crates/wasm/tests/deadline_parity.rs`. Mid-pass truncation parity
  is intentionally not tested across the WASM↔native boundary —
  `Instant::now()` is sampled independently per call and the trip
  point is a hardware-clock race.

**Status**: `[LANDED]`.

### 10.4 `marque-extract`

- Non-WASM only. Wraps Kreuzberg for 75+ document formats.
- Trust boundary: this crate sees full document content; callers are
  responsible for buffer lifetime.
- Excluded from every WASM-safe invariant section.

**Status**: `[LANDED]`.

---

## 11 · Configuration security

### 11.1 `.marque.toml` vs `.marque.local.toml`

- `.marque.toml` (committed): `[capco]`, `[rules]`, `[corrections]`
  allowed. A `[user]` section is a hard-fail at load time
  (`crates/config/src/lib.rs`; exit code 65; FR-010 / SC-006).
- `.marque.local.toml` (gitignored via `*.local.toml` in `.gitignore`):
  `[user]` section with `classifier_id`, `classification_authority`.

### 11.2 Precedence

CLI flags > environment variables > `.marque.local.toml` >
`.marque.toml`.

### 11.3 Identity never committed

The schema enforces classifier identity never enters a committed file.
CI pipelines inject `MARQUE_CLASSIFIER_ID` via environment.

### 11.4 Environment surface

| Variable | Purpose |
|---|---|
| `MARQUE_LOG` | `tracing-subscriber` filter. Trace-level is not production-safe for classified content. |
| `MARQUE_CLASSIFIER_ID` | Identity stamped into audit records |
| `MARQUE_CLASSIFICATION_AUTHORITY` | Authority string stamped into audit records |
| `MARQUE_AUDIT_SCHEMA` | Build-time audit schema selector (Phase D; FR-014) |
| `MARQUE_ENRON_CORPUS` | Required for corpus regeneration in `tools/corpus-analysis/` |

**Status**: `[LANDED]`. `MARQUE_AUDIT_SCHEMA` wired in v0.5 via
PR #122 — `crates/engine/build.rs` reads the env var, validates against
the closed accept-list `["marque-mvp-1", "marque-mvp-2"]`, and emits
`pub const AUDIT_SCHEMA_VERSION` consumed by `marque/src/render.rs` and
`crates/wasm/src/lib.rs`. See gap register row 1 (struck through).

---

## 12 · Cryptographic primitives

### 12.1 Document fingerprinting (v0.2)

The planned v0.2 cache key is
`blake3(content) ++ schema_version ++ config_hash`. BLAKE3 is used
unmodified (no truncation). The cache stores only `LintResult`, never
`FixResult` (§6.8).

### 12.2 Signing & attestation

Release source archives are signed with Sigstore via
`actions/attest-build-provenance` (§8.7 / §13.6). The crates.io
publish path itself still relies on OIDC token exchange
(`rust-lang/crates-io-auth-action`) — crates.io does not accept
Sigstore attestations on the upload. Consumers that need provenance
verify against the GitHub-released source archive, not the crates.io
tarball.

### 12.3 Audit-record integrity

NDJSON on stderr. Best-effort per-line flush. No signing, no hash
chain. Tamper-evidence is a deployment concern.

**Status**: `[PLANNED]` for §12.1; `[LANDED]` for §12.2 (release
archive only — crates.io upload is out of scope); `[NON-GOAL]` (at
engine layer) for §12.3 tamper-evidence.

---

## 13 · CI/CD security

### 13.1 Branch protection expectations

Main branch requires passing CI, `cargo-deny`, `cargo clippy`,
corpus-accuracy regression, and reviewer approval. Exact settings
are a repo admin concern; the expectation lives here.

### 13.2 Workflow permissions

`.github/workflows/ci.yml` declares `permissions: contents: read`
only. Secrets are read only in release / coverage jobs.

### 13.3 Secret exposure

CODECOV_TOKEN is scoped to the coverage job. Release-publish secrets
are scoped to `release.yml` (`id-token: write`, `contents: write`).

### 13.4 CodeQL

CodeQL runs on Rust, Python, and Actions languages with default query
packs. No custom queries today.

### 13.5 ISM schema monitor workflow

`.github/workflows/check-ism-schema.yml` monitors the ODNI page for
schema revisions and files idempotent issues on mismatch. Version
bumps are human-reviewed migrations, never silent refreshes
(Constitution VIII).

### 13.6 Release posture

`.github/workflows/release.yml` publishes to crates.io via OIDC
(`rust-lang/crates-io-auth-action`) and attaches a Sigstore-signed
source archive to each GitHub release (§8.7). All actions in the
release workflow are SHA-pinned per §8.3.

**Status**: `[LANDED]` for §§13.1–13.6.

---

## 14 · Adversarial testing

- **Fuzz harness**: `cargo-fuzz` target over `Engine::lint` accepts
  arbitrary `&[u8]`. Not CI-gated (nightly-only; run periodically).
- **Corpus accuracy regression** (SC-002 / SC-003): ≥95% per-rule
  accuracy floor, byte-identical CAPCO corpus diagnostics before/after
  any refactor. Gates every Phase C/D/E merge.
- **Mangled fixtures** (≥200 labeled cases; 004 Phase D): six mangling
  classes (typo, reordering, missing-delimiter, superseded-token,
  wrong-case, garbled-delimiter). Regenerable from Enron corpus.
- **WASM parity** (SC-008): byte-identical NDJSON diagnostics across
  CLI and WASM on the same corpus subset.
- **Content-ignorance CI grep** (T056): sentinel grep over every
  fixture's audit-stream output, plus a composite marking-in-prose
  test and a `#[should_panic]` self-test proving the check is
  load-bearing. Lives at `crates/engine/tests/audit.rs`.
- **Property-based testing** (`proptest`): 41 tests across three
  integration suites, exercising algebraic laws and engine invariants
  over generated inputs rather than fixed samples.
  - *Lattice laws* (`crates/capco/tests/proptest_lattice.rs`): full
    `Lattice` contract — join/meet idempotency, commutativity,
    associativity, bottom-identity/bottom-absorbs-meet, and both
    absorption directions — for `SciSet` and `SarSet` over generated
    compartment trees. `FgiSet` additionally covers `BoundedLattice`
    laws (bottom absorbs meet, top propagates join) and concealment
    monotonicity; join-over-meet absorption holds unconditionally. Note:
    meet-over-join absorption and `top`-is-meet-identity are
    intentionally not tested for `FgiSet` — CAPCO's concealment
    supersession rule causes `join` with a concealed element to produce
    a result whose `meet` collapses country sets to empty, a documented
    deviation from the standard lattice law driven by §3.3a policy.
  - *Engine invariants* (`crates/engine/tests/proptest_engine.rs`):
    never-panic, span-bounds (`start ≤ end ≤ source.len()`), fix
    idempotency (`fix(fix(x)) == fix(x)`), dry-run/apply parity,
    dry-run source-unchanged, threshold enforcement, and confidence
    bounds — over generated structured CAPCO banner and portion strings
    up to ~4 KB (small structured inputs plus a multi-KB variant with
    100–300 portions). Engine is constructed once via `OnceLock` to
    avoid re-building the Aho-Corasick automaton per test case.
  - *PageContext roll-up* (`crates/ism/tests/proptest_page_context.rs`):
    classification exact-equality roll-up (asserts `rolled ==
    portion_max`, catching both regressions and phantom over-restriction),
    dissem-control union superset, and REL TO intersection property over
    generated `IsmAttributes` vectors.
  Complements the fuzz target (raw-byte chaos) and the fixed-sample
  lattice-laws suite (exhaustive 7-sample cross-product) by covering
  the much larger space of generated compartment-tree and marking
  combinations that neither approach reaches.

**Status**: `[LANDED]` for fuzz, corpus, mangled, parity, T056, proptest.

---

## 15 · Authoritative source fidelity & citation integrity

Constitution Principle VIII is the full contract. Key security
implications:

- Every rule, constraint, rewrite, and vocabulary entry cites a
  verified passage in its primary source.
- Fabricated citations are a correctness defect of the same severity
  as a wrong predicate, not a stylistic choice.
- Propagation re-verifies (moving a citation into a new file is a new
  verification).
- Source revisions are planned migrations, not silent refreshes.

SC-009 (corpus-wide citation verification) and FR-021 (citation
verified at commit) are the spec-side enforcement. T089 was the
one-shot systematic audit pass and landed in PR #154 (commit
`cdc0866`); the standing FR-021 commit-time verification is the
ongoing guard.

**Status**: `[LANDED]`.

---

## 16 · Vulnerability disclosure & response

See [`SECURITY.md`](../../SECURITY.md) at the repo root for the full
policy. Summary:

- **Preferred channel**: GitHub Private Vulnerability Reporting.
- **Alternate**: encrypted email to `adam@knitli.com` (PGP key at
  `https://knitli.com/.well-known/pgp-key.txt`).
- **Acknowledgment SLA**: 24 hours.
- **Initial triage SLA**: 5 business days.
- **Scope includes**: rule engine correctness, supply chain, WASM
  sandbox escape, server auth/injection/DoS, NPM demo.
- **Scope excludes**: grammar/spec interpretation disagreements,
  non-DoS performance issues, upstream dependency issues (report
  upstream and notify).

---

## 17 · Known gaps & roadmap

Each gap carries a **severity**, **owner** (task / FR / SC ID where
possible), and a **remediation plan**. Severities:

- **P0** — must land before Phase D decoder + audit-v2 are merged
- **P1** — should land during Phase D
- **P2** — paper the decision now, fix opportunistically
- **P3** — explicit non-goal or deferred-phase item; listed for clarity

| # | Gap | Severity | Owner | Remediation |
|---|---|---|---|---|
| 1 | ~~`MARQUE_AUDIT_SCHEMA` not wired; `render.rs` hard-codes `"marque-mvp-1"`~~ **Resolved (PR-4).** Engine exposes `pub const AUDIT_SCHEMA_VERSION` from `env!("MARQUE_AUDIT_SCHEMA")`; `marque/src/render.rs` and `crates/wasm/src/lib.rs` dispatch v1/v2 emitter struct from the const-folded `AUDIT_SCHEMA_IS_V2` selector. Build-time validation against `["marque-mvp-1", "marque-mvp-2"]` panics on unknown values. T054 (back-compat parse) and T055 (single-schema stream invariant) ride on top of the wiring. | ~~P0~~ closed | FR-014, T005, T054, T055 | Done |
| 5 | ~~`__engine_promote` seal is convention-only~~ | ~~P1~~ | ~~Constitution V invariant~~ | **Resolved.** `EnginePromotionToken` (private `_seal: ()` field, `crates/rules/src/lib.rs`) seals `AppliedFix::__engine_promote` at the type level — external crates cannot brace-construct the token. The single bypass surface is `EnginePromotionToken::__engine_construct()` (`#[doc(hidden)]`, engine-only by convention), called from one place in production: the `engine_promotion_token()` helper in `crates/engine/src/engine.rs`. Two test-only exceptions (`crates/engine/tests/audit.rs`, `marque/src/render.rs::tests`) carry inline carve-out comments per Constitution V Principle V. Tests: `compile_fail` doctest on `EnginePromotionToken` proves brace construction is rejected; `crates/rules/tests/engine_promotion_seal.rs` proves the documented door works across the crate boundary. §6.2 rewritten |
| 6 | ~~Server has no explicit `DefaultBodyLimit`~~ | ~~P1~~ | ~~§10.2~~ | **Resolved.** `marque_server::DEFAULT_BODY_LIMIT_BYTES = 10 * 1024 * 1024`; applied via `axum::extract::DefaultBodyLimit::max(N)` in `build_app` / `build_app_with_limit`. Override at runtime via `MARQUE_MAX_BODY_BYTES` (resolved by `marque_server::resolve_body_limit`; values below 1 KiB rejected at startup with `EX_USAGE`). Tests in `crates/server/tests/http.rs` exercise both the `413` path and a `default_limit_admits_realistic_traffic` regression guard against a future drop-the-constant change |
| 7 | ~~No per-document timeout at the engine or server layer~~ | ~~P1~~ | ~~§9.7~~ | **Resolved (spec 005, PRs #161 / #162 / #163 / #164 / #165 / #166).** `LintOptions { deadline: Option<Instant> }` and `FixOptions { deadline, threshold_override }` (`crates/engine/src/options.rs`, both `#[non_exhaustive]`) feed `Engine::lint_with_options` / `Engine::fix_with_options`. Cooperative-cancellation checks at three boundaries: pre-pass, per-candidate, per-fix-application (`crates/engine/src/engine.rs`). Asymmetric response shape per Constitution V Principle V — lint returns `LintResult { truncated: true, candidates_processed, candidates_total }`, fix returns `Err(EngineError::DeadlineExceeded { partial_lint })` (no partial `FixResult` ever constructed). Per-surface wiring: CLI `--deadline <humantime>` (`marque/src/main.rs`); server `X-Marque-Deadline: <u64-ms>` header + `MARQUE_MAX_DEADLINE` env-var cap (`crates/server/src/lib.rs`); WASM `WasmConfig.deadline_ms: Option<f64>` validated `is_finite() && >= 0.0` (`crates/wasm/src/lib.rs`); batch `BatchOptions.per_doc_deadline: Option<Duration>` stamped after permit acquisition with `BatchError::DocumentDeadlineExceeded { partial_lint }` (`crates/engine/src/batch.rs`). `web_time::Instant` re-exported as `marque_engine::Instant` carries the type across native and WASM. Tests: `crates/engine/tests/deadline.rs`, `crates/engine/tests/batch_deadline.rs`, `marque/tests/cli_deadline.rs`, `crates/server/tests/http_deadline.rs`, `crates/wasm/tests/deadline_parity.rs`. Bench: `crates/engine/benches/deadline_overhead.rs` (10 % gate today, 2 % target). §9.7 rewritten in v0.13 |
| 8 | ~~`BatchEngine` `.expect()` panics on semaphore close~~ | ~~P1~~ | ~~§9.4, `batch.rs:196, 226`~~ | **Resolved.** New `BatchError::ShutdownInProgress` variant with matching `is_shutdown()` predicate; `From<tokio::sync::AcquireError>` impl maps the (only possible) error. Both `lint_many` and `fix_many` propagate the variant per-document instead of panicking. Unit tests cover `is_*` discrimination, `Display`, `Error::source`, and the `From` conversion driven through a closed `Semaphore` |
| 9 | ~~Strict-context floor (T1) not wired in decoder~~ | ~~P1~~ | ~~T045, T062, FR-011~~ | **Resolved (PR #114, commit `bc57bfc`).** `DecoderRecognizer::recognize` (`crates/engine/src/decoder.rs:148-159`) reads `ParseContext.strict_evidence` before any prior consultation and returns zero-candidate `Parsed::Ambiguous` when set; the engine drives this from the deep-scan opt-in at `crates/engine/src/engine.rs:369-374`. The related FR-011 per-page classification floor accumulates strict-path classifications at `engine.rs:338-419`, threads via `ParseContext.classification_floor`, and the decoder drops sub-floor candidates at `decoder.rs:251-257`. Floor resets on `MarkingType::PageBreak` per Constitution VI. Tests pin the gate (`decoder_defers_to_strict_when_strict_evidence_is_set`) and the floor (`unclassified_candidate_rejected_below_secret_floor` and three siblings in `decoder_recovery.rs`). §9.3 updated with the wiring citations |
| 10 | ~~`Confidence::validate` panic on bad rule halts the document~~ | ~~P1~~ | ~~§6.3~~ | **Resolved.** `Engine::lint` wraps every `Rule::check` call in `std::panic::catch_unwind(AssertUnwindSafe(...))`. Caught panics emit `tracing::warn!` at `marque_engine::rule_panic` and the rule is skipped for that candidate; sibling rules + remaining candidates keep running. `[profile.release]` switched from `panic = "abort"` to `panic = "unwind"` so the catch fires in release. Tests in `crates/engine/tests/rule_panic_isolation.rs` cover bare panic, real `FixProposal::new` invalid-`Confidence` panic, sibling-rules-continue, and a CAPCO smoke test |
| 11 | ~~No integrity hash for vendored CAPCO PDF / ODNI schemas~~ | ~~P2~~ | ~~§7.3~~ | **Resolved.** `crates/capco/docs/original-refs/SHA256SUMS` + `crates/ism/schemas/ISM-v2022-DEC/SHA256SUMS` verified by the `refs-integrity` CI job on every PR |
| 12 | ~~`reuse lint` not in CI~~ | ~~P2~~ | ~~§8.4~~ | **Resolved.** `reuse` job in `ci.yml` installs `reuse` via `pipx` and runs `reuse lint` on every PR |
| 13 | ~~No Sigstore/Cosign signing of release artifacts~~ | ~~P2~~ | ~~§8.7, §13.6~~ | **Resolved (release archive).** `actions/attest-build-provenance` signs a `git archive` workspace-state source tarball per release; attestation recorded in GitHub's transparency log. The archive is a separately verifiable provenance artifact, not a mirror of the crates.io per-crate tarballs. crates.io upload itself is out of scope — crates.io does not accept Sigstore attestations |
| 14 | ~~No CI gate enforcing the WASM-safe-subgraph dependency-license allow-list~~ | ~~P2~~ | ~~§8.5, Constitution Tech Stack~~ | **Resolved.** `deny.wasm-safe.toml` + `deny-wasm-safe` CI job invoke `cargo deny` against the WASM-safe subgraph (every non-WASM-safe workspace member excluded, dev-deps pruned) with a stricter allow-list. Original gap framing was "Apache-2.0 purity" under the retired Apache-core posture; landed form enforces the Constitution v1.2.0 "no copyleft / no competing source-available" dep-hygiene rule |
| 15 | ~~`--features count-allocs` hot-path alloc gate not in CI~~ | ~~P2~~ | ~~Constitution II~~ | **Resolved.** `crates/core/tests/alloc_budget.rs` (behind `count-allocs` feature) installs a counting global allocator and asserts `Scanner::scan(...)` does not exceed a small allocation budget across four cases (empty / single-banner / multi-marking / 4-KB-buffer-vs-small parity). `.github/workflows/ci.yml` `count-allocs` job runs `cargo test -p marque-core --features count-allocs --test alloc_budget -- --test-threads=1` on every PR |
| 16 | ~~`crates/core/src/parser.rs` `to_vec()` is undocumented~~ | ~~P2~~ | ~~§5.1~~ | **Resolved (refactor).** Workspace grep confirms the only `.to_vec()` call in `crates/core/src/parser.rs` today is at line 2072 inside a `#[test]` block (`Box<[DissemControl]> → Vec` for a `.contains()` assertion). Tests are not on the hot path; the zero-copy invariant holds. Whitepaper §5.1 pointed at code that has since been refactored away |
| 17 | ~~`tools/corpus-analysis/` has unpinned Python deps~~ | ~~P2~~ | ~~§7.4~~ | **Resolved.** `tools/corpus-analysis/requirements.txt` pins `requests==2.33.1` exactly. Transitive-set hash pinning via `pip-tools` and PEP 723 inline-metadata in `analyze.py` are tracked as follow-ups |
| 18 | ~~`site` fetches `fontsource` fonts at build~~ | ~~P2~~ | ~~§8.6~~ | **Resolved.** Fira Code (5 weights, Latin) and IBM Plex Sans (5 weights × normal/italic, Latin) are vendored under `site/src/assets/{Fira-Code,IBM-Plex-Sans}/font/` (SIL OFL 1.1) alongside per-font `LICENSE` and `README.md`. `astro.config.mjs` uses `fontProviders.local()` for all three site fonts — no `api.fontsource.org` / `cdn.jsdelivr.net` fetch at build time |
| 19 | ~~Mangled fixture `observed`/`expected` fields lack a token-only invariant test~~ | ~~P2~~ | ~~§5.5~~ | **Resolved.** `marque/tests/corpus_provenance.rs::mangled_fixtures_observed_expected_token_only` walks `tests/fixtures/mangled/**/*.json`, asserts each `observed` / `expected` field is free of prose sentinels (shared list with `crates/engine/tests/audit.rs`), free of classifier-id-shaped 5+-digit runs, and within a 256-byte length cap |
| 20 | ~~`CoreError::Display` leaks token text if surfaced~~ | ~~P2~~ | ~~§5.3~~ | **Resolved (runtime).** `crates/engine/tests/core_error_isolation.rs` embeds a high-entropy canary in adversarial input designed to trip every `CoreError` construction site, then asserts the canary never appears in any text-bearing field of `LintResult` / `FixResult`. Self-test sanity-asserts that `CoreError::MalformedMarking(canary).to_string()` does carry the canary, so a future Display redaction surfaces explicitly. Visibility-level tightening (`pub(crate)` on `CoreError`) is a P3 follow-up |
| 21 | ~~Shipped unsafe blocks lack SAFETY doc comments~~ | ~~P2~~ | ~~§4.2~~ | **Resolved.** Audit confirmed both shipped unsafe blocks already carry `// SAFETY:` doc comments — `crates/wasm/src/lib.rs:101` (Talc heap-claim alias-freedom + one-time-init invariants) and `crates/ism/src/attrs.rs:1085` (Trigraph constructor → ASCII⊂UTF-8 chain discharging the `from_utf8_unchecked` precondition). Whitepaper §4.2 was stale; updated in v0.6 to describe the present state |
| 22 | ~~`MARQUE_LOG` trace level is not flagged as production-unsafe~~ | ~~P2~~ | ~~§5.4, §11.4~~ | **Resolved.** `marque --help` carries an `ENVIRONMENT VARIABLES:` block (clap `after_help`) naming `MARQUE_LOG` and warning that `marque=trace` is not production-safe for classified content. Whitepaper §5.4 documents the route. The matching runtime stderr-notice guard is deferred until a `tracing::trace!` site that touches input bytes actually lands |
| 23 | Memory zeroization on drop | P3 | Constitution II future SGX/TrustZone | Explicit non-goal; wait for the right platform |
| 24 | Tamper-evident audit log at engine layer | P3 | §12.3 | Explicit non-goal; deployment concern |
| 25 | Cache-poisoning analysis | P3 | §12.1 v0.2 | Defer to v0.2 cache design |
| 26 | Phase J `DecisionRecord` content-ignorance (I-J2) | P3 | Phase J plan | Repeat §5 analysis for `DecisionRecord` when Phase J begins |
| 27 | Phase K metadata-extraction content-ignorance (I-K2) | P3 | Phase K plan | Repeat §5 analysis for extractors when Phase K begins |

---

## Appendix A · Extended threat model

Appendix reserved for per-threat detail (attack trees, preconditions,
detection signal, response). Not populated in this draft; entries will
be added alongside Phase D decoder work (T1 strict-context wiring,
T2 feature-trace audit, T3 surface enforcement).

## Appendix B · Invariant reference card

One-page tear-off for PR reviewers. Each invariant maps to the rule a
PR must not break. Not populated in this draft; target is a printable
two-column card keyed to §3 of this paper and Constitution II–VIII.

## Appendix C · Change log

| Version | Date | Change | Author |
|---|---|---|---|
| 0.1 | 2026-04-24 | Initial skeleton: §§0–17, Appendices A–C stubs. Sourced from parallel security audits of current implementation + open items in `specs/004-constraints-decoder-vocab/`. | Adam Poulemanos (with Claude Code) |
| 0.2 | 2026-04-24 | T056 (P0-4) landed as `crates/engine/tests/audit.rs`. §3.1 and §14 flipped from `[PARTIAL]` to `[LANDED]`. Gap register row 4 removed. | Adam Poulemanos (with Claude Code) |
| 0.3 | 2026-04-24 | T3 enforcement (P0-2 + P0-3) landed. Server rejects corpus-override across body, header, and query-string channels (T049/T050/T066 — `crates/server/src/lib.rs` + `crates/server/tests/http.rs`). WASM compile-fail guard landed (T051/T067 — `crates/wasm/src/lib.rs` + `crates/wasm/tests/no_corpus_override.rs`). §10.2 (corpus-override portion) and §10.3 flipped from `[PARTIAL]` to `[LANDED]`. Gap register rows 2 and 3 removed. | Adam Poulemanos (with Claude Code) |
| 0.5 | 2026-04-24 | P0-1 closed retroactively (gap register row 1 already struck through by the time §6.4 was last touched). PR #122 wired `MARQUE_AUDIT_SCHEMA` through `crates/engine/build.rs` and switched the CLI + WASM emitters to `marque_engine::AUDIT_SCHEMA_VERSION`; default schema bumped to `marque-mvp-2`; v1 downgrade kept green via suffixed snapshots and the T054 / T055 invariants. The header version had been pre-bumped to 0.5 ahead of this row. | Adam Poulemanos (with Claude Code) |
| 0.6 | 2026-04-24 | Three P2 hygiene gaps closed. Gap #18: site fonts vendored locally — Fira Code + IBM Plex Sans (SIL OFL 1.1) added under `site/src/assets/{Fira-Code,IBM-Plex-Sans}/`; `astro.config.mjs` flipped to `fontProviders.local()`; build no longer fetches from `api.fontsource.org` / `cdn.jsdelivr.net`. Gap #21: §4.2 corrected — both shipped unsafe blocks already carry `// SAFETY:` doc comments; the whitepaper's previous claim was stale. Gap #22: `marque --help` carries an `ENVIRONMENT VARIABLES:` block via clap `after_help` warning that `MARQUE_LOG=trace` is not production-safe for classified content; §5.4 documents the route. Other admin: gap register rows 18, 21, 22 struck through; §4.2 / §5.4 / §8.6 updated. | Adam Poulemanos (with Claude Code) |
| 0.7 | 2026-04-25 | Four more P2 hygiene gaps closed. Gap #16: §5.1 corrected — only `.to_vec()` call in `crates/core/src/parser.rs` is in a `#[test]` block, not on the hot path. Gap #17: `tools/corpus-analysis/requirements.txt` pins `requests==2.33.1` so a non-uv install is reproducible; §7.4 documents the route. Gap #19: `marque/tests/corpus_provenance.rs::mangled_fixtures_observed_expected_token_only` walks `tests/fixtures/mangled/**/*.json` and asserts `observed` / `expected` fields are free of prose sentinels, classifier-id-shaped digit runs, and exceed-length leaks; §5.5 documents the route. Gap #20: `crates/engine/tests/core_error_isolation.rs` embeds a high-entropy canary in adversarial input designed to trip every `CoreError` construction site and asserts the canary appears in no text-bearing field of `LintResult` / `FixResult`; §5.3 rewritten. Gap register rows 16, 17, 19, 20 struck through. | Adam Poulemanos (with Claude Code) |
| 0.8 | 2026-04-25 | Last P2 hygiene gap closed. Gap #15: `crates/core/tests/alloc_budget.rs` (behind `count-allocs` feature) installs a counting global allocator and gates `Scanner::scan(...)` allocation count across four cases (empty / single-banner / multi-marking / buffer-size-independence). `.github/workflows/ci.yml` `count-allocs` job runs the gate under `--test-threads=1` on every PR. §3.2 flipped from `[PARTIAL]` to `[LANDED]`. Gap register row 15 struck through. | Adam Poulemanos (with Claude Code) |
| 0.9 | 2026-04-25 | Two narrow P1 gaps closed. Gap #6: server body-size cap landed — `marque_server::DEFAULT_BODY_LIMIT_BYTES = 10 MiB`, applied via `axum::DefaultBodyLimit::max(N)` Tower layer in `build_app` / `build_app_with_limit`; runtime override via `MARQUE_MAX_BODY_BYTES` resolved by `resolve_body_limit` (rejects values below 1 KiB with `EX_USAGE`); tests cover `413` rejection on both `/v1/lint` and `/v1/fix` plus a 256 KiB realistic-traffic regression guard. §10.2 rewritten. Gap #8: `BatchEngine` `.expect()` replaced with new `BatchError::ShutdownInProgress` variant + `From<AcquireError>` impl + `is_shutdown()` predicate; both `lint_many` and `fix_many` now propagate the error per-document. §9.4 rewritten. Gap register rows 6 and 8 struck through. | Adam Poulemanos (with Claude Code) |
| 0.10 | 2026-04-25 | One more P1 gap closed. Gap #10: `Engine::lint` wraps every `Rule::check` in `std::panic::catch_unwind(AssertUnwindSafe(...))`; caught panics emit `tracing::warn!` at `marque_engine::rule_panic` and the rule is skipped for the candidate without aborting the document. `Cargo.toml` `[profile.release]` switched from `panic = "abort"` to `panic = "unwind"` so the catch fires in release. Tests in `crates/engine/tests/rule_panic_isolation.rs` cover bare panic, the real `FixProposal::new` invalid-`Confidence` panic, sibling-rules-continue, and a CAPCO smoke test. §6.3 rewritten. Gap register row 10 struck through. | Adam Poulemanos (with Claude Code) |
| 0.11 | 2026-04-25 | Documentation drift fix — Gap #9 (P1) closed retroactively. The strict-context floor was wired by Phase 4 PR #114 (commit `bc57bfc`, T045/T062/FR-011): `DecoderRecognizer::recognize` returns zero-candidate `Parsed::Ambiguous` when `cx.strict_evidence` is set, the engine drives `strict_evidence: !self.deep_scan` per-candidate, the FR-011 per-page classification floor accumulates from strict-path recognitions only, and four tests in `crates/engine/tests/decoder_recovery.rs` plus the inline `decoder_defers_to_strict_when_strict_evidence_is_set` test pin the behavior. §9.3 expanded with the wiring citations. Gap register row 9 struck through. | Adam Poulemanos (with Claude Code) |
| 0.12 | 2026-04-25 | Gap #5 (P1) closed — type-level seal for `AppliedFix::__engine_promote`. New `EnginePromotionToken` ZST in `marque-rules` carries a private `_seal: ()` field; brace construction from outside `marque-rules` is rejected by Rust's privacy rules. The token threads as the sixth argument of `__engine_promote`. Single bypass surface (`EnginePromotionToken::__engine_construct()`) is `#[doc(hidden)]` and engine-only by convention; production code mints it from exactly one place — the private `engine_promotion_token()` helper in `crates/engine/src/engine.rs` — feeding the three production promotion sites. Two test-fixture carve-outs (`crates/engine/tests/audit.rs`, `marque/src/render.rs::tests`) updated with inline carve-out comments. Acceptance: `compile_fail` doctest on `EnginePromotionToken` pins brace-construction rejection at the doctest's separate-crate compile; `crates/rules/tests/engine_promotion_seal.rs::documented_door_can_mint_token_from_outside_marque_rules` proves the documented door works. §6.2 rewritten. Gap register row 5 struck through. | Adam Poulemanos (with Claude Code) |
| 0.13 | 2026-04-26 | Last P1 gap closed — gap #7, per-document timeout. Spec 005 landed in six PRs over four phases. Header date bumped 2026-04-25 → 2026-04-26 to match this entry; the 0.10 → 0.12 entries date-stamped 2026-04-25 are unchanged. **Phase 1** (PR #161): foundational types — `LintOptions { deadline: Option<Instant> }` and `FixOptions { deadline, threshold_override }` (both `#[non_exhaustive]`), `Engine::lint_with_options` / `fix_with_options`, `EngineError` runtime-error enum, back-compat shims preserved. Zero behavior change. **Phase 2** (PR #162): cooperative cancellation wired in `Engine` at three boundaries — pre-pass, per-candidate, per-fix-application — producing truncated `LintResult` for lint and `Err(EngineError::DeadlineExceeded { partial_lint })` for fix per Constitution V Principle V. `crates/engine/benches/deadline_overhead.rs` bench gated at 10 % (target 2 %; tightening blocked on host-clock variance). **Phase 3a** (PR #163): CLI `--deadline <humantime>` flag, `EX_TEMPFAIL` (75) on fix expiry, `EX_USAGE` (64) on `--deadline 0`, JSON-mode trailing-narration suppressed to keep the NDJSON pipe-clean, `--dry-run` re-lint reuses the same `FixOptions`. **Phase 3b** (PR #164): server `X-Marque-Deadline: <u64-ms>` header, `MARQUE_MAX_DEADLINE` env-var cap mirroring `resolve_body_limit`, per-endpoint default 30 s, header-validation-before-body-deserialization ordering invariant, duplicate-header rejection, `400` / `504` / `500`-on-config-overflow response codes, `Marque-Truncated` response header on truncated lint. **Phase 3c** (PR #165): WASM `WasmConfig.deadline_ms: Option<f64>` validated `is_finite() && >= 0.0`, Constitution III runtime-config-restriction analysis recorded in crate-level docs, `WasmConfigCacheKey` projection excludes `deadline_ms`, `with_engine` accepts `FnOnce -> Result<Config, String>` to avoid building `Config` on cache hit, SC-008 byte-identical NDJSON parity preserved at generous and zero deadlines. **Phase 3d** (PR #166): `BatchOptions` gains `#[non_exhaustive]` + `per_doc_deadline: Option<Duration>`, `BatchEngine::lint_many_with_options` / `fix_many_with_options`, `BatchError::DocumentDeadlineExceeded { partial_lint }` per-document with `is_deadline_exceeded()` predicate distinct from `is_panic()` / `is_shutdown()` / `is_cancelled()`. Per-doc `Instant::now() + d` stamping happens AFTER permit acquisition so concurrency-controller wait does not consume the budget; `checked_add` overflow maps to `deadline = now` (engine treats as expired) rather than silently disabling the operator-configured budget. Required infrastructure: `web-time` workspace dep + engine dep; `web_time::Instant` re-exported as `marque_engine::Instant` so the engine's per-candidate `Instant::now()` works on `wasm32-unknown-unknown` (where `std::time::Instant::now()` panics). §9.7 rewritten from `[NON-GOAL]` to `[LANDED]`; §10.1 (CLI), §10.2 (server), §10.3 (WASM) each updated with deadline-handling block; gap register row 7 struck through. | Adam Poulemanos (with Claude Code) |
| 0.16 | 2026-04-27 | SBOM generation landed (issue #191). `release.yml` gains two new steps gated on `dry-run == false`: **Generate SBOMs** installs `reuse==5.0.2` (same pin as `ci.yml` `reuse` job) and runs `reuse spdx --add-license-concluded --creator-organization "Knitli Inc."` to produce `marque-<version>.spdx` (SPDX tag-value, NTIA compliant), then installs `cargo-cyclonedx` and runs it in JSON + XML modes to produce `marque-<version>.cyclonedx.json` and `marque-<version>.cyclonedx.xml` (full transitive dep graph from Cargo.lock); **Attest SBOMs** passes all three SBOM files to `actions/attest-build-provenance` (same action, same SHA-pin used by the existing archive attestation — no new permissions required). All four release artifacts (archive + three SBOMs) are attached to the GitHub release via `softprops/action-gh-release`; the release body documents all four `gh attestation verify` invocations. `SECURITY.md` updated: planned SBOM note → landed. `cargo-cyclonedx` SHA-pin deferred to issue #191 follow-up per §8.3. §8.7 rewritten. | Adam Poulemanos (with Claude Code) |
| 0.15 | 2026-04-26 | Property-based testing layer added (`proptest` v1). Three new integration suites (41 tests): `crates/capco/tests/proptest_lattice.rs` (29 tests — full `Lattice` contract for `SciSet`/`SarSet` including meet associativity, bottom-absorbs-meet, and both absorption directions; `FgiSet` adds `BoundedLattice` laws, meet associativity, join-over-meet absorption, and concealment monotonicity — meet-over-join absorption and top-is-meet-identity intentionally omitted due to CAPCO §3.3a concealment-supersession deviation documented in the test file); `crates/engine/tests/proptest_engine.rs` (8 tests — never-panic, span bounds, fix idempotency, dry-run/apply parity, dry-run source-unchanged, threshold enforcement, confidence bounds; generator extended with a multi-KB variant producing 100–300-portion inputs ≈ 1.5–4.5 KB); `crates/ism/tests/proptest_page_context.rs` (4 tests — classification exact-equality roll-up, dissem-control union superset, REL TO intersection property). `proptest = { version = "1", default-features = false, features = ["std"] }` added to workspace dependencies; per-crate dev-dependency added for `marque-capco`, `marque-engine`, `marque-ism`. §14 updated. | Adam Poulemanos (with Claude Code) |
| 0.14 | 2026-04-26 | Stale `[PARTIAL]` status notes flipped to `[LANDED]` after a doc-vs-state audit caught four section footers still claiming work-in-progress for items the gap register and Appendix C already record as resolved. **§3.4** (engine-promotion boundary): seal-by-convention claim updated to reflect the v0.12 `EnginePromotionToken` ZST landing (gap row 5 struck through). **§3.7** (authoritative source fidelity): "T089 is open" replaced with the PR #154 / commit `cdc0866` landing reference; FR-021 commit-time verification cited as the standing guard. **§11.4** (environment surface): `MARQUE_AUDIT_SCHEMA` wiring claim replaced with the v0.5 / PR #122 landing — env-var read in `crates/engine/build.rs`, accept-list `["marque-mvp-1", "marque-mvp-2"]`, `pub const AUDIT_SCHEMA_VERSION` consumed by CLI + WASM emitters. **§15** (citation integrity): same T089 update as §3.7 plus the FR-021 standing-guard framing. No code changes — pure doc-state reconciliation. The two genuinely-open `[PARTIAL]` notes (§3.3 WASM-safe-set drift compile-fail test absent; §10.2 server auth middleware un-wired) and the two `[PLANNED]` v0.2 cache notes (§6.8, §12.1) stand. | Adam Poulemanos (with Claude Code) |
