<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Feature Specification: Marque MVP — CAPCO Marking Linter and Fixer

**Feature Branch**: `001-marque-mvp`
**Created**: 2026-04-08
**Status**: Draft
**Input**: User description: "Shoehorn the existing design doc (docs/plans/2026-03-11-marque-design.md) into the speckit process to validate scope and requirements for the Marque MVP."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Lint a document for marking errors (Priority: P1)

A classifier or reviewer runs Marque against a document containing portion markings,
banner markings, and a Classification Authority Block. Marque reports each marking
violation with the offending span, a plain-language explanation of why it is wrong,
the relevant CAPCO citation, and (when applicable) a suggested fix with a confidence
score. The reviewer never has to leave their workflow to look up the spec.

**Why this priority**: Lint-only is the smallest slice that delivers real value —
it surfaces violations the existing tooling cannot explain, and it is the foundation
every other slice (fix, batch, integrations) depends on.

**Independent Test**: Feed a known-bad raw text snippet (e.g. a banner with a
portion-style abbreviation) to the linter and verify the reported diagnostic includes
the correct rule ID, span, message, and CAPCO reference. No fix application required.

**Acceptance Scenarios**:

1. **Given** the literal input bytes `S//NF\n` (6 bytes; a banner using portion
   abbreviations at byte offset 0),
   **When** the user lints the document with default configuration,
   **Then** the system reports exactly one diagnostic
   `{ rule: "E001", severity: "error", span: { start: 0, end: 5 }, citation:
   "CAPCO-2016 §A.6", fix: { replacement: "SECRET//NOFORN",
   confidence: 1.0 } }`, and the exit code is `1`.
2. **Given** the literal input bytes `SECRET//REL TO FVEY\n` (a REL TO list
   missing the `USA` trigraph at the leading ordinal position),
   **When** the user lints the document,
   **Then** the system reports exactly one diagnostic with `rule: "E002"`
   spanning the `REL TO FVEY` substring, with a suggested fix inserting `USA, `
   immediately after `REL TO ` (confidence `0.97`).
3. **Given** the literal input bytes `UNCLASSIFIED\n`,
   **When** the user lints the document,
   **Then** the system reports zero diagnostics and exits `0`.
4. **Given** the body-prose input `The answer is (S) of course.\n` (a
   parenthesized single-letter token mid-sentence, not in a marking zone),
   **When** the user lints the document,
   **Then** the system reports zero diagnostics (disambiguation heuristic
   treats the `(S)` as prose, not a portion marking — see the Edge Cases
   section), exercising SC-003a.

---

### User Story 2 - Auto-fix high-confidence violations with audit trail (Priority: P2)

After reviewing diagnostics, the user asks Marque to apply fixes. Only fixes at or
above a configured confidence threshold are applied automatically; lower-confidence
fixes remain as suggestions. Every applied change is recorded in an audit log that
captures the rule, original text, replacement, confidence, timestamp, and (when
configured) the classifier identity. A dry-run mode shows what would change without
writing.

**Why this priority**: Fixing is the productivity multiplier, but only after lint is
trustworthy. The audit log is a hard prerequisite for any IC/DoD adoption — without
it, automatic remediation is a non-starter.

**Independent Test**: Run fix on a document with mixed high- and low-confidence
violations and verify (a) only high-confidence fixes are applied, (b) the resulting
document is valid, (c) every applied fix appears in the audit log with all required
fields, and (d) dry-run produces the same audit output without modifying the file.

**Acceptance Scenarios**:

1. **Given** the literal input bytes `SERCET//NF\nThe block order is wrong\n`
   with a `[corrections] SERCET = "SECRET"` entry (confidence `1.0`, rule
   `C001`) and a latent `E003` misordered-blocks diagnostic on line 2
   (confidence `0.6`), and a configured threshold of `0.9`,
   **When** the user runs `marque fix`,
   **Then** stdout begins `SECRET//NOFORN\n` (the C001 correction applied),
   line 2 is unchanged, the stderr NDJSON audit stream contains exactly one
   `AppliedFix` record for rule `C001`, the `E003` proposal remains only as a
   `fix` field on the stdout diagnostic stream (never in the audit stream),
   and all audit fields (`rule`, `span`, `original`, `replacement`,
   `confidence`, `timestamp`, `dry_run`, `schema`) are present.
2. **Given** any fix invocation,
   **When** the `--dry-run` flag is set,
   **Then** no file content changes, but the full audit output is produced.
3. **Given** a fix run with no classifier identity configured,
   **When** the user runs fix,
   **Then** the audit log still records all other fields and clearly marks the
   classifier identity as absent.

---

### User Story 3 - Configure rule severity and corrections per project (Priority: P2)

A program office defines its policy in a committed project config: which rules are
errors, which are warnings, which are silently fixed, and a corrections map of
known organization-specific typos. Individual users layer their own identity
(classifier ID, declassification authority) in a local, gitignored config. CLI flags
and environment variables override both for one-off runs.

**Why this priority**: Without configurable severity and a corrections map, Marque
cannot match real organizational policies and will be rejected on first contact with
a program office. User identity must be separable from committed config to avoid
leaking PII into version control.

**Independent Test**: Create a project config that downgrades one rule to warning
and adds a custom correction; create a local config with a classifier ID; verify
that lint output reflects the new severity, the correction is applied during fix,
and the classifier ID appears only in audit records (never in any committed file).

**Acceptance Scenarios**:

1. **Given** a project config that sets a rule severity to `warn`,
   **When** the user lints a document violating that rule,
   **Then** the diagnostic is reported with `warn` severity, not the rule default.
2. **Given** a `.marque.toml` with `[corrections] SERCET = "SECRET"` and the
   literal input bytes `SERCET//NF\n`,
   **When** the user runs `marque fix`,
   **Then** stdout is `SECRET//NOFORN\n` and the stderr audit record for the
   typo carries `rule: "C001"`, `source: "CorrectionsMap"`, and
   `migration_ref: "corrections-map"`.
3. **Given** an environment variable that overrides the configured classifier ID,
   **When** the user runs fix,
   **Then** the audit records use the environment value, not the local config value.

---

### User Story 4 - Lint and fix raw text from a web context (Priority: P3)

An IC analyst types a classification banner into a web form field in an
internal application. On each keystroke the form revalidates via an embedded
Marque web worker; the analyst sees red squiggles under malformed spans within
one display frame of their last keystroke and can accept suggested fixes
inline without leaving the form. The primary actor is the analyst; success is
"I caught my own marking error before submitting, without opening a separate
tool or spec." The web worker accepts a raw string and returns diagnostics
and suggested fixes within a single keystroke's budget. No file format
support is required in this context.

**Why this priority**: The web worker target is what unlocks the high-volume
distribution channels (browser extensions, Office add-ins, internal web forms),
but it depends on the lint and fix slices being stable first.

**Independent Test**: Load the web worker build, send a marking string, and verify
diagnostics return for the same input that the native linter would flag, with
matching rule IDs and spans.

**Acceptance Scenarios**:

1. **Given** the web worker is loaded and a malformed banner is supplied,
   **When** the caller invokes lint,
   **Then** the diagnostics returned match those produced by the native linter for
   the same input, byte for byte on spans.
2. **Given** the web worker build,
   **When** the caller invokes any function,
   **Then** no file system or network access is attempted.

---

### User Story 5 - Surface unrecognized markings gracefully (Priority: P3)

A reviewer runs Marque against a document containing a token inside a marking
candidate that the parser does not recognize at all (a typo'd control, a
retired code word, or a garbled span). Marque reports an unknown-token
diagnostic with a precise span and a message distinguishing *"this is not a
valid token"* from *"this is a valid token in the wrong place"*; no fix is
offered. The reviewer can then correct the source manually without fearing
the tool silently dropped a violation.

**Why this priority**: FR-012 requires unknown-token reporting; without an
explicit story it has no independent test and no acceptance bar. Real IC
documents contain garbled tokens; silently dropping them is worse than a
false positive.

**Independent Test**: Feed a fixture containing the literal bytes
`SECRET//XYZZY\n` where `XYZZY` is not a valid control; verify exactly one
diagnostic `{ rule: "E008", severity: "error", span: { start: 9, end: 14 },
fix: null }`.

**Acceptance Scenarios**:

1. **Given** the literal input bytes `SECRET//XYZZY\n`,
   **When** the user lints the document,
   **Then** the system reports exactly one diagnostic with `rule: "E008"`,
   the span covers `XYZZY`, and no fix is attached.

---

### Stakeholder: Compliance Reviewer (non-interactive)

A compliance reviewer or auditor never runs `marque` themselves. They consume
the `AppliedFix` NDJSON stream after the fact — days, weeks, or months later —
to answer three questions about a historical document processing session:

1. **What was changed?** Every `AppliedFix` record identifies a byte span, the
   original bytes at that span, and the replacement, so the reviewer can
   reconstruct the diff without having the original document.
2. **Why was it changed?** Every record carries a stable `rule` identifier and
   (when the fix came from a migration or corrections table) a `migration_ref`,
   so the reviewer can trace the change back to a CAPCO citation or an
   organizational corrections-map entry.
3. **Who is accountable?** Every record carries an optional `classifier_id`
   and a timestamp, so the reviewer can attribute the change to the operator
   who ran the tool.

The reviewer's needs are the reason the audit record's required fields are
what they are. This stakeholder is not a user story because they do not
invoke the tool, but their acceptance criterion is load-bearing: *a reviewer
given only the NDJSON stream and the rule-ID → citation mapping can
reconstruct and justify every applied change*. SC-004 verifies audit
completeness against this criterion.

---

## Clarifications

### Session 2026-04-08

- Q: What is the default confidence threshold for auto-applied fixes? → A: 0.95, with the threshold user-adjustable via config and CLI flag. Most deprecated-marking conversions (including X-shorthand date markings like `25X1-` → current form) are deterministic table lookups and should sit at or above 0.95; the threshold exists so organizations can tune *how aggressive* auto-fix is.
- Q: What is the interactive latency target for a single typical document? → A: ≤16ms p95 for inputs up to ~10KB of raw text, matching constitution Principle I (one display frame budget). Hyper-tuning beyond this is post-MVP.
- Q: What input surface does the MVP CLI expose? → A: File path arguments and stdin (with `-` as the conventional stdin sentinel). Directory globbing/recursion is deferred. Note: the target demo experience is a document-style web UI driving lint/fix as the user types; for the MVP that demo is expected to be wired through stdin (or the web-worker build), not through native file watching.

### Edge Cases

- A `(S)` token appears mid-sentence in body prose where it is unlikely to be a
  portion marking — the system must use document context to avoid false positives.
  Disambiguation heuristic: a single-letter parenthesized token is classified as
  a portion marking only when its `RuleContext.zone` is a marking zone (start of
  paragraph/line, header/footer, or immediately preceding a sentence); otherwise
  it is ignored. The heuristic lives in the parser, not in a rule.
- A document mixes valid markings with markings using a deprecated CAPCO version —
  the system must report deprecation against the configured schema version.
- A fix would overlap another fix in the same span — the system must apply fixes
  in a deterministic order (reverse byte order) and never produce a corrupted output.
- A document is empty, contains no markings, or contains only whitespace — the
  system must return an empty diagnostic set and exit successfully.
- A document contains a marking the parser does not recognize at all — the system
  must report it as an unknown-token diagnostic rather than silently dropping it.
- The corrections map and a built-in rule both apply to the same span — the user
  correction must take precedence.
- The confidence threshold is set to 1.0 — only fixes the system is certain about
  must be applied; everything else remains a suggestion.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST detect portion markings, banner markings, and
  Classification Authority Blocks in raw text input and report each as a diagnostic
  with a precise byte span.
- **FR-002**: The system MUST validate detected markings against the pinned CAPCO
  schema version and report each violation with a stable rule identifier, severity,
  and human-readable explanation.
- **FR-003**: The system MUST cite the relevant CAPCO section for every violation it
  reports.
- **FR-004**: The system MUST be able to apply fixes for violations whose proposed
  remediation meets or exceeds a configured confidence threshold, and MUST leave all
  other proposed fixes as suggestions only. The default threshold is `0.95`. The
  threshold MUST be adjustable via project configuration, environment variable, and
  command-line flag, following the standard precedence chain.
- **FR-004a**: Deprecated-marking conversions defined by deterministic CAPCO
  migration tables (including X-shorthand declassification date markings) MUST be
  assigned a confidence score consistent with their determinism, so that
  organizations relying on the default threshold receive these fixes automatically.
- **FR-005**: The system MUST produce an audit record for every applied fix
  containing rule identifier, original text, replacement text, confidence score,
  timestamp, and (when present) classifier identity. For the MVP, the audit
  stream is delivered on the process's standard error channel as NDJSON;
  durable file-backed sinks (e.g. fsync-on-close rotation to an
  `--audit-log <PATH>` target) are a **known limitation** of the MVP and are
  deferred to the server slice. Operators requiring durable audit capture must
  redirect stderr to a file and accept the risk of stdio buffer loss on
  process crash. This deferral is explicit, not an oversight.
- **FR-005a**: The audit NDJSON writer MUST emit each record as a single
  atomic write (serialize-to-buffer, then one `write_all`), so a
  partially-serialized record can never appear in the stream. If
  serialization of a record fails, the engine MUST emit a single
  `{"schema":"marque-mvp-1","error":"<code>","rule":"<rule-id>"}` error
  frame on the audit stream and return a nonzero exit code. Every audit
  record (including error frames) MUST carry a top-level `"schema"` field
  identifying the contract version (`"marque-mvp-1"` for this slice) so
  future consumers can detect version drift without out-of-band metadata.
- **FR-006**: The system MUST support a dry-run mode that produces the full audit
  output without modifying any input.
- **FR-007**: The system MUST load configuration from a committed project file, an
  optional gitignored local file, environment variables, and command-line flags,
  applying them in that order of increasing precedence.
- **FR-008**: The system MUST allow project configuration to override the default
  severity of any rule (including disabling it). A rule configured to severity
  `off` MUST NOT execute and MUST NOT produce any diagnostic; the `off` value
  is valid only in configuration input and MUST NEVER appear on an emitted
  diagnostic record. Every `Diagnostic` in the output stream therefore has
  an effective severity of `warn`, `error`, or `fix` — `off` is unrepresentable
  at emission time.
- **FR-009**: The system MUST allow project configuration to define a corrections
  map of organization-specific replacements, and MUST apply user corrections in
  preference to built-in defaults when both match.
- **FR-010**: The system MUST keep classifier identity out of any committed
  configuration file, accepting it only via the gitignored local config or
  environment variables.
- **FR-011**: The system MUST pin the active CAPCO schema version explicitly so
  that schema upgrades are intentional, never silent.
- **FR-012**: The system MUST report unrecognized tokens within marking candidate
  boundaries as diagnostics rather than dropping them silently.
- **FR-013**: After `Engine::lint` or `Engine::fix` returns, no engine-,
  rule-, or configuration-owned allocation may hold a reference to or a copy
  of the caller's input buffer. The lint/fix API surface is lifetime-bound so
  that this invariant is enforced at compile time where possible; a
  drop-check/Miri test (see tasks T021a) verifies the runtime case. The
  caller always owns the input buffer; the engine borrows it for the duration
  of a single call and retains nothing afterward.
- **FR-014**: The system MUST expose lint and fix capabilities through a
  format-agnostic interface that accepts raw text input, suitable for embedding in
  web worker, command-line, and service contexts without re-implementation.
- **FR-014a**: The MVP command-line interface MUST accept input from both file
  path arguments and standard input (with `-` as the conventional stdin sentinel).
  Directory globbing and recursive directory traversal are out of scope for the
  MVP slice.
- **FR-015**: The system MUST emit diagnostics and audit records in a structured,
  machine-readable form suitable for downstream consumption, in addition to any
  human-readable output.
- **FR-016**: When multiple fixes apply to overlapping or adjacent spans, the
  system MUST apply them in a deterministic, non-corrupting order:
  (1) fixes are applied in reverse byte order (largest `span.end` first) so
  earlier spans' byte offsets remain valid during application;
  (2) for two fixes with identical `(span.start, span.end)`, the tiebreaker is
  rule-ID lexicographic ascending order (e.g. `C001` before `E001`);
  (3) for two fixes with identical span *and* rule ID (e.g. two corrections-map
  entries colliding), the tiebreaker is the replacement text in
  lexicographic ascending order. These rules make fix application a total
  order — two runs on the same input always produce byte-identical output
  and byte-identical audit streams.

### Key Entities

- **Marking Candidate**: A region of source text that the scanner has identified as
  a possible portion, banner, or CAB marking, represented as a byte span with a
  candidate type. Never copies the underlying content.
- **ISM Attributes**: The normalized representation of a parsed marking, capturing
  classification level, SCI controls, SAR identifiers, dissemination controls,
  REL TO trigraphs, declassification information, and authority fields.
- **Diagnostic**: A reported violation, including rule identifier, severity, span,
  message, CAPCO citation, and an optional proposed fix.
- **Fix Proposal**: A proposed replacement for a span, including the rule
  identifier, the original and replacement text, a confidence score in
  0.0–1.0, and an optional migration reference. Pure data; no timestamp, no
  classifier identity. A proposal remains a *suggestion* until it is applied.
- **Applied Fix (Audit Record)**: An immutable record of a fix that was
  actually applied (or would have been applied, under `--dry-run`), wrapping
  the originating Fix Proposal and adding the timestamp, classifier identity
  when configured, and a dry-run flag. Only Applied Fixes appear in the audit
  stream; sub-threshold suggestions never do.
- **Configuration**: The merged view of project, local, environment, and CLI
  settings, including rule severities, corrections map, classifier identity, and
  the pinned CAPCO schema version.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user linting a single typical document (raw text input up to
  approximately 10KB) receives diagnostics within **16ms at the 95th percentile**
  on commodity developer hardware, fitting inside one display frame budget so the
  result feels instantaneous during interactive use. **Scope**: SC-001 applies
  to the **native** build only (CLI / library, measured via the
  `benches/lint_latency.rs` criterion bench on the reference machine
  documented in plan.md §Performance Goals). The WASM build is covered by
  SC-001b.
- **SC-001b** (**WASM interactive latency, advisory**): The WASM build, driven
  from the harness at `crates/wasm/examples/harness.html` in a current
  Chromium-family browser on the reference machine, SHOULD complete
  `lint(text)` in **≤32ms p95** on the same ≤10KB inputs used by SC-001 — two
  display frames, accounting for the JS↔WASM boundary and the `daachorse`
  Phase-2 matcher. This is an advisory gate for the MVP (logged, not
  blocking CI) because the browser environment is variable; it becomes a
  hard gate in the browser-extension slice. Measurement method and recorded
  numbers live in `benches/wasm_latency.md`.
- **SC-001a** (**performance regression gate**): The `benches/lint_latency.rs`
  criterion benchmark commits a baseline p50/p95/p99 to `benches/baseline.json`
  for the reference machine documented in the plan. CI fails any pull request
  that regresses p95 by more than **10%** versus the committed baseline. The
  baseline is re-captured only by an explicit, reviewed commit that updates
  `benches/baseline.json` — never by an automated "update-on-green" job. This
  prevents silent drift past the SC-001 16ms budget.
- **SC-002**: For the **frozen MVP corpus** (see SC-002a), the system reports the
  expected violation with the correct rule identifier and byte span on at least
  95% of cases measured both **per rule** and **overall**. Per-rule measurement
  prevents the aggregate bar from being met by stuffing the corpus with
  easy-to-detect violations at the expense of the harder structural rules.
- **SC-002a** (**corpus contract**): The "representative set" referenced by
  SC-002, SC-003, and SC-008 is the fixture set committed under `tests/corpus/`
  and tagged `mvp-corpus-v1` before Phase 7 begins. The corpus MUST contain at
  least 3 known-bad fixtures per rule under test (E001–E008, W001, C001) and at
  least 20 known-good fixtures. Each known-bad fixture has a sibling
  `.expected.json` pinning the expected diagnostic rule IDs and byte spans. The
  corpus is the load-bearing artifact for SC-002/SC-003/SC-008 — the criteria
  are not falsifiable against any other input set. **Provenance contract**:
  every fixture MUST be synthetic (wrapped in Lorem Ipsum or manifestly
  fictional prose, using only public CAPCO marking syntax from ODNI
  documentation); the corpus root MUST contain a `CORPUS_PROVENANCE.md`
  declaring this invariant and naming the reviewer who spot-checked the
  corpus before the `mvp-corpus-v1` tag; and CI MUST run a provenance scan
  (see SC-006's companion check at task T055a) asserting that every file
  under `tests/corpus/` matches a registered fixture-path pattern and
  contains no classifier-id-shaped strings and no token lists beyond the
  generated CVE enumerations. A fixture that fails the provenance scan
  blocks merge.
- **SC-003**: For the same frozen MVP corpus, automatic fixes applied at the
  default confidence threshold produce a document that passes a re-lint pass
  with zero remaining violations on at least 95% of cases, measured both
  per-rule and overall.
- **SC-003a** (**precision on clean prose**): On a body-prose corpus of at least
  1000 lines containing no markings (including incidental parenthesized
  single-letter tokens like `(S)` appearing mid-sentence), the linter produces
  zero diagnostics. This exercises the `(S)`-disambiguation heuristic and bounds
  the false-positive rate on non-marking text.
- **SC-004**: Every applied fix in any successful run is accompanied by a complete
  audit record — no missing fields, no orphaned changes — verified by an audit
  completeness check on the output.
- **SC-005**: Throughput on a batch of representative documents scales linearly
  with input size across at least one order of magnitude, with no super-linear
  slowdown or unbounded memory growth.
- **SC-006**: No committed configuration file in any test fixture or example
  contains a classifier identity, verified by an automated check.
- **SC-007**: A user who changes a single rule's severity in the project config
  sees the change reflected on the next lint run without any other configuration
  edits.
- **SC-008**: The same input string produces **byte-identical** diagnostic JSON
  (rule IDs, byte spans, messages, citations) whether linted via the native
  interface or the web-worker interface, even though the two builds use
  different Phase-2 token-matching engines (`aho-corasick` on native,
  `daachorse` on WASM). Byte-identical output across differing matchers is the
  contract; the parity harness (tasks T061/T070) is its verification.

## Assumptions

- The MVP scope covers raw text input only. File-format extraction (DOCX, PDF, etc.)
  is explicitly out of scope for this slice and will be addressed in a follow-up
  feature once the core pipeline is stable.
- The MVP targets the currently pinned ODNI ISM schema version
  (ISM-v2022-DEC, per the existing `marque-capco` package metadata). Other schema
  versions are out of scope for this slice.
- The default confidence threshold is `0.95`, chosen to auto-apply unambiguous
  fixes (typos, abbreviation expansions, trigraph normalization, separator
  normalization, and deterministic deprecated-marking conversions including
  X-shorthand date markings) while leaving judgment-call structural moves and CAB
  rewrites as suggestions until explicitly opted into. Organizations tune the
  threshold to their own risk appetite.
- The incremental batch cache, server REST surface, browser extensions, Office
  add-ins, and metadata sanitization are all out of scope for this MVP slice and
  are tracked separately on the roadmap.
- Document context (where in the document a candidate appears) is available to
  the parser at the precision needed to disambiguate body prose from markings;
  ambiguity resolution heuristics are part of the rule logic, not a separate user
  feature.
- The system runs in a trusted local environment for the MVP — multi-tenant
  isolation, authentication, and rate limiting are server concerns deferred to the
  server feature slice.

## Out of Scope

- File format extraction (DOCX, PDF, HTML, etc.) and OCR of scanned documents.
- The REST microservice and its auth/logging/rate-limiting middleware.
- The incremental LMDB-backed batch cache.
- Office add-ins (Word, Outlook, PowerPoint, Excel) and the browser extension.
- Document metadata extraction and sanitization (EXIF, XMP, document properties).
- CUI marking validation, NTK metadata, and TDF validation rule families.
- Managed rule update subscription service and enterprise dashboard.
