# Phase 0 Research: Marque MVP

**Branch**: `001-marque-mvp` | **Date**: 2026-04-08

This document resolves the plan-level unknowns that gate Phase 1 design. Each
item is recorded as **Decision / Rationale / Alternatives considered**.

---

## R-1: ODNI ISM CVE XML → Rust enum generation in `build.rs`

**Decision**: Use `quick-xml` in pull-parser mode inside `marque-ism/build.rs`
to walk every file under `schemas/ISM-v2022-DEC/CVE_ISM/*.xml` and emit one Rust
enum per CVE file into `OUT_DIR/values.rs` (consumed via `marque-ism/src/generated.rs`). Identifiers are derived from
the CVE element `Term` content with a deterministic sanitizer
(`UPPER_SNAKE_CASE`, non-alphanumeric → `_`, leading digits prefixed with `_`).
Each enum derives `Copy, Clone, Eq, PartialEq, Hash, Debug` and gets a
`as_canonical(&self) -> &'static str` method returning the original token text
for round-tripping.

**Rationale**: `quick-xml` is already in the constitution Tech Stack table and
its pull-parser API has minimal allocation overhead at build time. Generating one
enum per CVE file (rather than one mega-enum) keeps the type system honest about
which token classes are valid in which positions, mirroring the schema structure
that downstream rules reason about. Sanitizing identifiers deterministically
makes diff review of the generated code possible across schema versions.

**Alternatives considered**:
- *`xml-rs`*: also serde-friendly, but slower build-time and a larger dep
  surface than `quick-xml`. Rejected.
- *`roxmltree`*: pleasant DOM, but loads the whole tree into memory; no benefit
  here over a streaming pull-parser.
- *Hand-curated enums*: zero build-time cost but defeats the entire two-layer
  model (constitution Principle IV) and guarantees drift on the next schema
  bump.
- *One mega-enum across all CVEs*: simpler but loses the structural type
  information that lets rules statically prove a token belongs to the right
  category.

**Schema rollback strategy**: The active schema version is pinned in
`marque-ism/Cargo.toml [package.metadata.marque] ism-schema-version` and
asserted at build time (T010). A new ODNI schema package lands as a sibling
directory under `crates/marque-ism/schemas/` (e.g. `ISM-v2024-JUN/`) in a
dedicated branch; the pin bumps in that same branch, Layer 1 regenerates, and
the Layer 2 rule set is audited for behavioral drift against the MVP corpus
before the branch merges. If a schema bump is later found to have regressed
production, rollback is a single-commit revert of the pin + schema directory —
no database migration, no on-disk state, no deploy coordination. The previous
schema directory is retained in `git` history and can be restored with
`git checkout`. This makes schema changes cheap to attempt and cheap to
reverse, which is the only sustainable policy for tracking an upstream we do
not control.

---

## R-2: ODNI Schematron → predicate function generation

**Decision**: Parse `schemas/ISM-v2022-DEC/Schematron/ISM_XML.sch` and the
`Lib/*.sch` includes with `quick-xml`, extract every `<sch:rule>` and its
`<sch:assert>` / `<sch:report>` children, normalize the embedded XPath
expressions to a small fixed vocabulary (attribute presence, attribute equality,
set membership, set cardinality), and emit one boolean predicate function per
assertion into `OUT_DIR/validators.rs` (consumed via `marque-ism/src/generated.rs`). Each generated predicate has the
shape `fn ${rule_id}_${assertion_idx}(attrs: &IsmAttributes) -> bool` and
contains *only* the binary check — no message, no remediation, no severity.
The Layer 2 hand-written rules in `marque-capco/src/rules.rs` import these
predicates and decide *what to do* when one returns `false`.

For the MVP slice, we generate predicates only for assertions whose XPath fits
the fixed vocabulary above (estimated ~70% of `ISM_XML.sch`). Assertions that
require a richer XPath subset are surfaced at build time as a warning and
skipped; they become explicit hand-written rules in Layer 2 instead. This is
acceptable for the MVP because the rule set committed by the spec is finite and
each rule will have a hand-written Layer 2 implementation regardless.

**Rationale**: Compiling Schematron predicates into Rust functions preserves
constitution Principle IV (binary predicates only at Layer 1) and Principle I
(no runtime XPath interpreter on the hot path). The fixed vocabulary
intentionally excludes the long tail of Schematron's XPath surface area —
support for that tail can be added schema-version by schema-version as needed,
without retroactively breaking Layer 2.

**Alternatives considered**:
- *Embed an XPath interpreter at runtime*: rejected. Pulls in a multi-MB dep,
  blows the WASM binary, and adds a per-rule interpretation cost on the hot
  path.
- *Use an existing Schematron-to-XSLT converter and embed an XSLT runtime*:
  same objections, more so.
- *Generate predicates for every assertion blindly*: would force us to support
  the full XPath subset Schematron uses, which is out of scope for the MVP.
  Skip-with-warning is the safer compromise.

---

## R-3: Confidence calibration for deprecated-marking conversions

**Decision**: Build a static `phf` map at compile time, keyed by deprecated
token (canonicalized to upper case, separators stripped), valued by
`(replacement, confidence, migration_ref)` triples. The CAPCO migration tables —
including the X-shorthand date markings (`25X1`, `25X1-human`, etc.) — are
deterministic 1:1 conversions and are assigned `confidence = 0.98`. Conversions
that require additional context (e.g., reformatting a CAB declassification
field) are *not* table-driven; they live as hand-written Layer 2 rules with
their own (lower) confidence. The `0.98` value sits above the default `0.95`
threshold from the spec's Clarifications section, so users on defaults receive
these fixes automatically — matching the user's stated expectation during
clarification.

**Rationale**: A `phf` table is the cheapest possible runtime lookup for a
known-static key set, has zero collisions by construction, and adds nothing to
the WASM binary that wouldn't be there anyway. Pinning the migration confidence
at `0.98` (rather than `1.0`) leaves head-room for organizations who choose to
raise their threshold to `1.0` and want migration conversions to become
suggestions instead of auto-applies — that's the kind of policy lever the spec
explicitly asked us to preserve.

**Alternatives considered**:
- *`HashMap` built at startup*: rejected. Adds runtime cost for no gain over
  `phf` for a static key set.
- *Embed migrations in the matching rule itself*: rejected. Couples Layer 1
  data to Layer 2 rule code and makes schema-version diffs harder to audit.
- *Confidence `1.00`*: rejected. Removes the threshold lever the user
  explicitly asked for — at `1.00` there is no value of `--confidence-threshold`
  short of `> 1.0` that makes migrations suggestions.

---

## R-4: Structured diagnostic and audit-record JSON shape

**Decision**: Adopt a flat JSON object per diagnostic with stable field names,
emitted one-per-line (NDJSON) for stream friendliness. Fields:
`{ "rule": "E001", "severity": "error", "span": { "start": 12, "end": 18 },
"message": "...", "citation": "CAPCO-2016 §A.6", "fix": null | { ... } }` (citation refers to the CAPCO Register and Manual, 2016). The
`fix` sub-object uses: `{ "replacement": "...", "confidence": 0.98,
"migration_ref": "...", "audit": { ... } }`. The audit sub-object uses:
`{ "rule": "E001", "original": "...", "replacement": "...", "confidence": 0.98,
"timestamp": "2026-04-08T12:34:56Z", "classifier_id": null | "..." }`.

A JSON Schema for both shapes is committed under `contracts/`. The schema is
versioned via a top-level `$schema` URL pointing at the schema file in this
feature directory; the actual contract version is `marque-mvp-1`.

**Rationale**: NDJSON is the standard CLI streaming format and plays well with
`jq`, log shippers, and downstream pipelines. Flat structure keeps consumers
simple. Committing a JSON Schema turns SC-008 (native/WASM parity) into a
mechanically checkable property — both targets must serialize through the same
serde derives and a shared `assert_diagnostic_matches_schema` test.

**Alternatives considered**:
- *Single JSON array as the entire output*: rejected. Cannot stream; consumers
  must wait for end-of-input.
- *Protobuf*: rejected. Adds tooling burden without benefit at MVP scale; can
  be revisited if a server-to-server use case emerges.
- *Bare TOML*: rejected. Lossier for nested structures and tooling support is
  weaker outside config files.

---

## R-5: Audit log destination strategy

**Decision**: For the MVP, the CLI writes diagnostics to **stdout** (so they
compose with shell pipelines) and writes audit records to **stderr** by default
(so they don't pollute downstream consumers of `marque check --format json`).
Both can be redirected to files via standard shell redirection. A future
`--audit-log <PATH>` flag is reserved but not implemented in this slice.
Diagnostics and audit records use distinct JSON Schemas, so a consumer that
captures stderr can mix-and-match streams safely.

The classifier identity in audit records comes exclusively from
`MARQUE_CLASSIFIER_ID` env var or `.marque.local.toml`; the CLI MUST refuse to
load a `.marque.toml` (committed config) that contains a `[user]` table. This is
checked at config load time, with a hard-fail exit (`65 EX_DATAERR`).

**Rationale**: Splitting streams is the standard Unix posture and aligns with
the way `cargo`, `git`, and `ruff` separate machine-consumable output from
operator narration. The hard-fail on classifier identity in committed config
turns SC-006 into an enforceable invariant rather than a wishful test, and
catches the most common mistake (copy-paste between `.toml` files) at the moment
of harm rather than after a `git push`.

**Alternatives considered**:
- *Audit to stdout, diagnostics to stderr*: rejected. Inverts the standard Unix
  convention and breaks `marque check --format json | jq ...`.
- *Audit to a fixed `.marque/audit.log` file*: rejected for MVP. Adds a
  filesystem dependency to a slice that the constitution says should retain
  zero document content. Reserve for v0.2 with explicit opt-in.
- *Soft-warn on classifier identity in committed config*: rejected. Soft warns
  get ignored; hard-fail forces correction.

---

## R-6: Test corpus sourcing strategy

**Decision**: Hand-craft the MVP corpus from synthetic markings only. Every
fixture is a marking string surrounded by Lorem Ipsum or other manifestly
fictional prose. No real-world classified or controlled documents are
checked into the repo, ever. The corpus lives at `tests/corpus/{valid,invalid}/`
and is loaded by a `dev-dependency` helper crate (`marque-corpus-loader`,
internal-only) so individual tests can request "all invalid fixtures matching
rule E001" without duplicating fixture lists. Each invalid fixture has a
sibling `.expected.json` file describing the expected diagnostic stream; the
test harness asserts deep equality.

The corpus targets ≥40 invalid + ≥20 valid fixtures for the MVP, distributed
across the rule set proportionally to real-world frequency (banner abbreviation
and missing-USA-trigraph dominate; structural moves get one or two each).
Fixtures use the canonical CAPCO marking syntax exactly as it appears in the
public ODNI documentation — no novel markings, no FOUO content.

**Rationale**: Synthetic-only is the only sourcing strategy that doesn't invite
a CI/repo review by a counter-intel office. Every fixture is provably non-real
because it's wrapped in Lorem Ipsum. The expected-output sidecar files turn
SC-002 and SC-003 into trivially verifiable properties (just diff against the
sidecar) and make diff review of rule changes mechanically auditable.

**Alternatives considered**:
- *Pull from FOIA-released documents*: rejected. Provenance is hard to
  verify, redactions vary, and the legal posture of derivative works is
  unclear enough to be a distraction.
- *Generate fixtures programmatically from a marking grammar*: a great idea
  for v0.2 once the rule set stabilizes, but introduces a chicken-and-egg
  dependency for the MVP (the generator needs the parser, the parser needs
  fixtures to test against).
- *Reuse the ODNI sample documents*: rejected. They're not licensed for
  redistribution and we don't want to find out what happens if someone
  republishes them via crates.io.

---

## R-7: WASM Phase 2 token-matching engine

**Decision**: Use `aho-corasick` for both native and WASM builds in the MVP and
revisit `daachorse` once we have real binary-size measurements from the WASM
artifact. The constitution names both as acceptable; the design doc recommended
benchmarking. For the MVP we prioritize having a single code path until we have
data showing `daachorse` is materially smaller or faster on the WASM target.
The benchmark harness in `benches/` will produce that data as a side effect of
the existing perf workstreams.

**Rationale**: Two implementations means two integration paths, two test
matrices, and two failure modes. The cost of running `aho-corasick` in WASM is
not yet measured; until it is, "single implementation" is the simpler default.
This decision is reversible inside one PR if measurements show otherwise.

**Alternatives considered**:
- *`daachorse` from day one*: not justified without measurement; introduces a
  divergence between native and WASM code paths that we'd then have to test
  twice.
- *Pluggable matcher behind a trait*: over-engineered. Both libraries have the
  same interface for our purposes; we can swap implementations without an
  abstraction layer when the time comes.

---

## R-8: `recoco-utils::ConcurrencyController` integration scope

**Decision**: `ConcurrencyController` is a `BatchEngine`-only concern and is
**not** wired into the MVP. The MVP `Engine` exposes only the single-document
lint/fix path. `BatchEngine` and the row+byte semaphore live in
`marque-engine` but behind a `batch` feature flag that is not enabled by
default and is not built by the MVP CLI or the WASM target.

**Rationale**: The spec's user stories cover single-document interactive use
(US1, US2, US4) and per-project configuration (US3). None require a batch path.
Linear-scaling SC-005 can be verified at the single-document loop level without
pulling in a concurrency controller. Deferring the batch path keeps the
constitution Principle VII dependency graph minimal for the MVP and leaves the
LMDB cache + batch surface as a clean v0.2 feature slice.

**Alternatives considered**:
- *Wire `BatchEngine` in but leave it untested*: rejected. Untested code on
  the dependency graph violates constitution Principle V's spirit (auditability)
  and complicates the cargo feature matrix for no MVP user benefit.
- *Drop `BatchEngine` from the workspace entirely*: rejected. It's
  scaffolding for v0.2 and the design doc explicitly references it; deleting
  and re-adding is more churn than feature-flagging it off.

---

## Open items deferred to Phase 2 / future slices

- **Concrete benchmark hardware target**: SC-001 says "commodity developer
  hardware" — the benchmark harness will record CPU model and CI runner type
  but a hard pass/fail bar across machine classes is a v0.2 concern.
- **Internationalization of diagnostic messages**: not in scope for MVP; all
  messages are en-US. The diagnostic JSON shape leaves room for a future
  `locale` field without breaking changes.
- **Streaming over very large inputs**: the spec's interactive latency target
  applies to ≤10KB inputs. Streaming chunked processing for multi-megabyte
  documents is part of the format-extraction slice (because that's where
  multi-megabyte inputs originate).
