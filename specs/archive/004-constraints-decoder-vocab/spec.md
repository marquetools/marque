# Feature Specification: Declarative Rule Expression, Probabilistic Recovery, and Full Vocabulary Metadata (Phases C–E)

**Feature Branch**: `004-constraints-decoder-vocab`
**Created**: 2026-04-20
**Status**: archived — Phases C, D, and E all shipped. Phase C (declarative constraints + topological page-rewrite scheduler) landed via #69 and the rewrite series; Phase D (probabilistic recognition + audit v2) landed via PRs #111, #112, #114, #122, #127, #131, #135; Phase E (vocabulary surface + `Codec<S>` + `Vocabulary<S>` impl + Phase-E readiness stub) landed via PRs #141 → #146. See CLAUDE.md "Recent Changes" → "Phase 5", "Phase 4". Kept for historical context.
**Input**: User description: "'docs/plans/2026-04-19-recursive-lattice-and-decoder.md' phases C, D, and E"

---

## Context

This specification reverse-covers phases C, D, and E of the 2026-04-19 recursive-lattice-and-decoder plan. That plan is treated as the primary source for technical detail; this specification captures the *why*, user value, acceptance boundaries, and measurable outcomes so the work can clear Spec Kit's gates before planning resumes.

The three phases form one coherent capability story:

- **Phase C** turns roughly a third of CAPCO's rules from hand-written code into declarative data, with a shared evaluator every future scheme inherits.
- **Phase D** gives marque a probabilistic fallback recognizer so mangled markings (OCR artifacts, hand-typed errors, historical drift) resolve automatically with full provenance instead of silent failure.
- **Phase E** surfaces the full vocabulary metadata (authority, owner/producer, deprecation, point of contact) that ODNI publishes but marque currently flattens away, and pins the serialization-codec trait so downstream phases can round-trip structured documents.

Together the three phases move marque from "one tool with 39 hardcoded CAPCO rules and a token-id lookup table" to "an engine where any grammar is declarative data flowing from its authoritative source, with a probabilistic fallback for real-world messy inputs."

Phase B (recursive category lattices) is assumed shipped; Phase F (CUI as second scheme) and later depend on these three phases completing.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Scheme authors express rules as data, not code (Priority: P1)

A contributor adding a new CAPCO-adjacent constraint, or authoring a new grammar entirely (e.g., a future NATO or CUI adapter), expresses the common cases — "these two tokens conflict", "this token requires that one", "this token implies another", "NOFORN supersedes REL TO" — as declarative entries against a shared vocabulary and evaluator, rather than writing a bespoke rule implementation per constraint.

**Why this priority**: Declarative constraint expression is the foundation of marque's "any grammar, shallow adapter" promise. Without it every new scheme is hundreds of lines of bespoke rule code. With it, the common 80% of rules become data entries verifiable line-by-line against the governing manual. This is the phase whose delivery unlocks everything downstream (Phase F CUI, Phase L NATO/partner nationals).

**Independent Test**: Migrate approximately one-third of CAPCO's hand-written rules to the declarative surface. Run the full CAPCO corpus before and after migration and verify the diagnostic output is byte-identical (equivalence test). The migration delivers value on its own — rule maintenance cost drops and new-scheme authors have a pattern to follow — even if phases D and E never ship.

**Acceptance Scenarios**:

1. **Given** a grammar declares a conflict between tokens A and B as a declarative constraint entry, **When** the engine evaluates a marking containing both A and B, **Then** the same diagnostic is emitted as the pre-migration hand-written rule produced, with no change in severity, citation, or span.
2. **Given** a grammar declares a page-level rewrite whose action or trigger is custom (not one of the built-in enum variants), **When** the grammar is loaded without explicit read/write axis annotations for that rewrite, **Then** engine construction fails with an error that names the offending rewrite.
3. **Given** two page-level rewrites are declared such that rewrite A writes an axis that rewrite B reads, and rewrite B writes an axis that rewrite A reads, **When** engine construction runs the schedule, **Then** construction fails with a cycle error naming both rewrites.
4. **Given** multiple page-level rewrites exist without cyclic dependencies, **When** the engine constructs its scheduler, **Then** the scheduled execution order depends only on read/write axis dependencies and not on the declaration order in the grammar source.

---

### User Story 2 - Compliance staff can clean up historical corpora without manual re-marking (Priority: P2)

A compliance team holds a backlog of historical documents whose markings are inconsistently formatted — OCR'd scans, hand-typed banners with ordering errors, superseded token names, missing delimiters. Today marque flags each as an error and stops. With the probabilistic recognizer, the team runs a batch reconciliation pass and marque auto-fixes markings it is highly confident about, surfacing only the genuinely ambiguous ones for human review. Every auto-fix carries an auditable posterior score and feature trace so a reviewer can spot-check a sample and decide whether to accept the remediation batch.

**Why this priority**: The backlog-reconciliation workload is the revenue-relevant commercial use case after authoring lint. Without the probabilistic recognizer, marque's answer to mangled historical input is "your problem, not mine." With it, marque becomes the tool that actually processes the accumulated corpus, not just the next document typed.

**Independent Test**: Feed a curated fixture of at least 200 mangled markings (typo, reordering, missing delimiter, superseded token, wrong case, garbled delimiter) labeled with their expected canonical form into the engine with the batch/deep-scan opt-in enabled. Measure the resolution rate at a confidence-threshold cutoff. Value is delivered once the recognizer resolves most of the fixture correctly with explainable provenance on each fix, independent of whether any other phase has shipped.

**Acceptance Scenarios**:

1. **Given** a document contains a marking with a single-character typo (e.g., `SERCET` for `SECRET`), **When** the engine runs with the probabilistic recognizer enabled, **Then** the marking is auto-corrected and the resulting audit record carries the recognition confidence, the rule confidence, and the feature contributions that drove the decision.
2. **Given** a document contains `(C)` in a paragraph of copyright-flavored prose *and* also contains a strict-path-recognized CONFIDENTIAL portion elsewhere in the document, **When** the engine processes the document, **Then** the `(C)` resolves to CONFIDENTIAL and is not downgraded to copyright by the probabilistic recognizer.
3. **Given** a document carries a banner with tokens in an order the grammar does not allow (e.g., dissem controls before SCI), **When** the engine runs with the probabilistic recognizer enabled, **Then** the banner is auto-corrected to canonical order with an audit record indicating a reordering-class mangling was detected.
4. **Given** an author is typing in an interactive session, **When** the probabilistic recognizer opt-in has not been set, **Then** the engine's per-keystroke latency is bounded by the strict path alone and the recognizer never fires.
5. **Given** an HTTP request to the server includes a caller-supplied corpus override, **When** the server processes the request, **Then** the override is rejected and no change to recognizer posteriors takes effect.
6. **Given** a WASM embedder, **When** the embedder attempts to inject a runtime corpus override, **Then** no such injection surface exists — the corpus tables are compiled into the WASM artifact and are not reachable through the WASM API.
7. **Given** a WASM embedder running an interactive authoring surface (browser extension, Office add-in), **When** it calls the default lint export `lint(bytes)`, **Then** only the strict recognizer fires and latency stays in the strict-path envelope (SC-001).
8. **Given** the same WASM embedder running a batch-cleanup pass over a backlog, **When** it calls the explicit decoder export `lint_deep_scan(bytes)`, **Then** the decoder fires with the build-time-baked corpus priors and the audit records produced carry the same confidence + runner-up + features trace as the CLI path (FR-009, SC-007). No WASM API permits changing the priors at runtime (FR-013 third clause).

---

### User Story 3 - Rules and audit records reference the full authoritative vocabulary, not opaque identifiers (Priority: P3)

A compliance reviewer reading an audit record for an applied fix sees the originator of the token, the authoritative source (URN + version), the effective deprecation state (if any), and a point of contact — not just an opaque internal identifier. A rule author implementing a rule that depends on who published a token (e.g., "CIA-originated SCI requires extra validation") queries this metadata directly through the vocabulary surface, without round-tripping to the source XML at build time or hand-maintaining a parallel table.

**Why this priority**: Marque's legitimacy claim — "you can trace every fix to its authority" — is only half-true today. The source XML has the authority data; marque discards it at build time. Restoring it closes the trust gap without changing the shipping pipeline. P3 because neither of the immediate-value stories (P1, P2) blocks on it; however, without it, Phase F (CUI) cannot express the CUI-specific authority checks its manual requires.

**Independent Test**: Ingest the ODNI ISM-v2022-DEC vocabulary through the dual JSON + XML codepath. Verify that a rule can query authority, owner/producer, deprecation replacement, portion form, banner form, and banner abbreviation for every active token through a single trait surface. Verify that the `FOUO → CUI` migration entry — never correct CAPCO policy — is removed from the migration table.

**Acceptance Scenarios**:

1. **Given** a rule needs to emit a diagnostic that cites the originating authority for a token, **When** the rule queries the vocabulary surface for that token's metadata, **Then** the authority's source name, URN, schema version, and point of contact are returned as static data requiring no allocation.
2. **Given** a grammar migrates an older token name to its current replacement, **When** the engine applies the corresponding fix, **Then** the audit record records both the source token's URN and the replacement token's URN, and the migration table entry governs the mapping.
3. **Given** a document contains the FOUO marking in a CAPCO-governed context, **When** the engine processes the document, **Then** FOUO is treated as an active and valid dissemination control (no CUI migration is suggested), and the corpus regression on CAPCO remains byte-identical after the stale migration entry is removed.
4. **Given** a later phase (Phase G or beyond) needs to serialize a marking to a structured format (XML or JSON), **When** it implements the codec surface, **Then** no new trait-surface evolution is required; the codec trait was already pinned in Phase E.

---

### Edge Cases

- The probabilistic recognizer encounters a token bag that fits no grammar template (e.g., half-NATO, half-CAPCO tokens). Expected: an "ambiguous with zero candidates" result — explicit signal, never a silent fallthrough to a strict-path error.
- A grammar declares a page-level rewrite with both a custom action and a custom trigger. Expected: the grammar author supplies explicit read/write axis annotations; engine construction fails if they are absent.
- A document carries a mangled region where the two top candidates have nearly equal posteriors. Expected: the engine returns `Ambiguous` with the candidate set, the fix is not auto-applied, and the diagnostic surfaces the candidates for human resolution.
- An operator supplies a corpus override file when running the CLI locally. Expected: accepted on the CLI target only; rejected by the server binary; unreachable from WASM.
- The ODNI publishes a new ISM schema version. Expected: the schema version pin is bumped as a deliberate migration, with re-verified citations across affected rules before the migration lands.
- A rule implementer cites a passage from the CAPCO manual in a doc comment. Expected (Constitution Principle VIII): the citation is verified against the vendored manual before commit; unverified citations are removed, not left pending.
- An engine build processes documents and emits audit records, then the audit schema bumps in a later build. Expected: older records remain parseable by downstream consumers; each engine build emits exactly one schema version.

## Requirements *(mandatory)*

### Functional Requirements

#### Phase C — Declarative constraints and page-level rewrites

- **FR-001**: Grammar authors MUST be able to express binary relationships between tokens or categories (conflict, requirement, implication, supersession) as declarative data rather than hand-written code.
- **FR-002**: A shared constraint evaluator MUST operate across all grammars. Adding a new grammar MUST NOT require reimplementing the evaluator.
- **FR-003**: Page-level rewrites (cross-category transformations such as "NOFORN supersedes REL TO") MUST be expressible as declarative rewrites with explicit annotations of which axes the rewrite reads and which it writes.
- **FR-004**: The engine MUST refuse to construct a grammar whose declared page-level rewrites form a read/write dependency cycle. The error MUST name both members of the cycle.
- **FR-005**: The engine MUST refuse to construct a grammar that declares a page-level rewrite with a custom action or a custom trigger unless the rewrite also declares its read and write axes explicitly.
- **FR-006**: Approximately one-third of the CAPCO ruleset that fits the declarative-constraint or declarative-rewrite shape MUST be migrated to the declarative surface in this phase, with byte-identical corpus diagnostic output before and after the migration.
- **FR-007**: The scheduled execution order of page-level rewrites MUST depend only on their read/write axis dependencies; reordering their declaration in the grammar source MUST NOT change the scheduled order or the diagnostic output.

#### Phase D — Probabilistic recognizer

- **FR-008**: The engine MUST provide a probabilistic recognizer that resolves mangled marking regions against a grammar's marking space. Mangling classes supported include: typo, token reordering, missing delimiter, superseded token, wrong case, and garbled delimiter.
- **FR-009**: A fix proposal produced by the probabilistic recognizer MUST carry a confidence composed of recognition confidence, rule confidence, and optionally region confidence. The runner-up posterior ratio and the list of evidence features that contributed to the decision MUST be retained in the audit record.
- **FR-010**: Interactive authoring sessions MUST NOT invoke the probabilistic recognizer by default. Invocation MUST require explicit opt-in — a CLI flag for single-operator runs, a batch-mode option for server deployments.
- **FR-011**: The probabilistic recognizer MUST NOT downgrade the classification of a region below what other strict-path evidence in the same document supports. "Same document" = one `Engine::lint` invocation on a single input byte buffer (for the CLI, one file; for the server, one request body; for WASM, one `lint_*` call). When any strict-path-recognized portion in that document is at CONFIDENTIAL or higher, an ambiguous `(C)` elsewhere in that same document MUST resolve to CONFIDENTIAL without consulting the copyright-vs-classification prior.
- **FR-012**: Evidence feature labels in audit records MUST come from an enumerated list fixed at grammar build time. Free-form label text MUST be rejected by the type system, not accepted and later length-capped.
- **FR-013**: Runtime configuration that modifies recognizer posteriors (corpus overrides) MUST be available only on the interactive CLI target. The server target MUST reject such configuration from HTTP requests. The WASM target MUST reject this at build time — no override codepath may exist in the WASM artifact.
- **FR-013a**: The WASM artifact MUST expose the decoder behind an explicit opt-in export (`lint_deep_scan` / `fix_deep_scan`) distinct from the default strict-path exports (`lint` / `fix`). The decoder's corpus priors are baked into the artifact at build time (Constitution III: "compiled in, not loaded at runtime"); no WASM export MAY accept prior-modifying configuration at runtime.
- **FR-014**: A single engine build MUST emit audit records at exactly one schema version. Audit records from prior schema versions MUST remain parseable by downstream consumers.
- **FR-015**: When the probabilistic recognizer receives a token bag that fits no grammar template, it MUST return an explicit "ambiguous with zero candidates" result; it MUST NOT silently fall through to the strict-path error.

#### Phase E — Vocabulary metadata and codec scaffolding

- **FR-016**: Every active vocabulary token MUST expose its authority (source + URN + schema version), owner/producer, point of contact, deprecation status, canonical portion form, banner form, and (when defined) banner abbreviation through a shared trait surface.
- **FR-017**: When a token is deprecated and a replacement mapping exists in the build-time migration table, the vocabulary surface MUST make that replacement queryable. When no replacement is known, the deprecation status is still exposed but no replacement is invented.
- **FR-018**: The ODNI ISM schema MUST be consumed through both a JSON codepath (for per-term vocabulary data) and an XML codepath (for XSD and Schematron constraint predicates). Neither is a fallback for the other; both exist because ODNI publishes both.
- **FR-019**: A codec trait surface for grammar serialization MUST be published in this phase, with no concrete implementations yet. The surface MUST be sufficient for a later phase to implement XML and JSON round-trip without further trait evolution.
- **FR-020**: The `FOUO → CUI` migration table entry MUST be removed. FOUO remains a valid active dissemination control in CAPCO ISM; any future per-agency FOUO-to-CUI suggestion MUST be gated on explicit configuration, not baked into the migration table.

#### Cross-cutting (applies across all three phases)

- **FR-021**: Every rule, constraint, rewrite, or vocabulary reference added or modified in these phases MUST cite a verified passage in the primary source (the CAPCO-2016 manual at `crates/capco/docs/CAPCO-2016.md` or the ODNI ISM schema at `crates/ism/schemas/ISM-v2022-DEC/`). Citations MUST be verified at the point of commit per Constitution Principle VIII. Unverifiable citations MUST be removed, not left in place pending follow-up.
- **FR-022**: A *scheme-adoption contribution* — a PR that lands a new marking scheme (e.g., Phase F CUI, a future NATO or JOINT adapter) — MUST NOT edit grammar-independent crates (`marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`). Engine gaps discovered by a scheme-adoption PR MUST be closed in a separate predecessor PR that lands first, per Constitution Principle IV. This branch is *not* a scheme-adoption contribution: it is the engine-infrastructure PR (trait surfaces, scheduler, audit v2, vocabulary tables) plus an exercising migration of ~15 existing CAPCO rules into the new declarative surface. The infrastructure edits to engine-side crates in this branch are precisely what FR-022 will require of future scheme-adoption work — they land here so Phase F onward can honor FR-022 cleanly.
- **FR-023**: All rule implementations and recognizer implementations MUST be `Send + Sync` and hold no mutable global state, preserving the concurrency guarantees the batch engine depends on.

### Key Entities

- **Constraint**: A declarative relationship between tokens or categories. Variants cover conflict, requirement, implication, and supersession. Evaluated by the shared constraint evaluator, not per-grammar code.
- **Page Rewrite**: A declarative transformation applied during page-level roll-up. Carries explicit read/write axis annotations, a trigger condition, and an action. Scheduled by the engine via topological sort over axis dependencies.
- **Strict Recognizer**: Today's structural parser, wrapped behind the recognizer trait surface. Fast, zero-allocation, succeeds only when input matches the grammar exactly.
- **Probabilistic Recognizer**: A bag-of-tokens Bayesian recognizer that takes over when the strict recognizer fails (batch-mode) or when a rule escalates a region. Produces candidate rankings with explicit posterior scores.
- **Fix Confidence**: The composite confidence attached to every fix proposal — recognition confidence times rule confidence times optional region confidence, plus a runner-up posterior ratio and feature-contribution list when the probabilistic recognizer produced the proposal.
- **Vocabulary Token Metadata**: The full per-term record exposed by the vocabulary surface — authority, owner/producer, point of contact, deprecation, URN, schema version, portion/banner forms, and scheme-specific extensions.
- **Codec (trait surface only)**: The serialization-interchange surface through which later phases round-trip markings to structured formats (XML, JSON). Pinned in this phase; no implementations yet.
- **Mangled-Marking Fixture**: A labeled corpus of at least 200 real or synthetic mangled markings used to measure the probabilistic recognizer's accuracy and calibrate its thresholds.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Single-document lint completes within 16 milliseconds at the 95th percentile for 10 KB of clean strict-path input, measured on the existing benchmark corpus. No regression from the pre-Phase-C baseline.
- **SC-002**: Single-document lint completes within 18 milliseconds at the 95th percentile for 10 KB of input containing one mangled region requiring probabilistic recovery. "One mangled region" = one marking span (portion or banner) with 1–2 mangled tokens that the strict recognizer rejects and the decoder resolves within the K=8 candidate bound.
- **SC-003**: Per-rule accuracy on the existing CAPCO corpus stays at or above 95 percent across all three phases; no rule's accuracy regresses below the pre-Phase-C baseline.
- **SC-004**: Of a mangled-marking fixture of at least 200 labeled cases, at least 85 percent are resolved to the expected canonical marking when the probabilistic recognizer's aggregate confidence threshold is set to 0.85 or higher.
- **SC-005**: Approximately one-third of CAPCO's hand-written rule implementations retire — the rules that fit the declarative-constraint or declarative-rewrite shape are expressed as data rather than code, with byte-identical corpus diagnostics before and after the migration.
- **SC-006**: `marque-mvp-1` audit records emitted by pre-Phase-D engine builds continue to parse without modification in `marque-mvp-2`-aware downstream consumers (back-compat is one-directional: v2 parsers read v1; v1 parsers do not read v2 without upgrade). A single engine build emits exactly one audit schema version.
- **SC-007**: Every automatically-applied fix originating from the probabilistic recognizer carries enough provenance for a compliance reviewer to answer "why was this chosen over the runner-up?" without consulting the engine's source code.
- **SC-008**: Rule code can query vocabulary metadata (authority, owner/producer, deprecation, URN, schema version, portion/banner forms) for every active token through a single trait surface, with zero runtime allocation.
- **SC-009**: Every rule, constraint, rewrite, or vocabulary entry added or modified in these phases carries a citation that a reviewer can trace to a passage in the primary source; citations that cannot be traced are removed rather than retained.
- **SC-010**: A grammar adoption PR (e.g., Phase F's CUI) can be prepared against the post-Phase-E trait surfaces without requiring changes to the grammar-independent crates. *Verification note*: this criterion is **deferred-verifiable** — it can only be confirmed when Phase F lands and attempts adoption. Phase E exits with a compile-only check (T078) that the codec trait is stable and a readiness stub (T089b) that exercises every Phase-E trait surface as if building a minimal second scheme, catching surface gaps without waiting for Phase F.

## Assumptions

- Phase B (recursive category lattices, `PageContext` driven by scheme projection, structural CAPCO types ported to lattice form) has shipped. Phases C, D, and E depend on that foundation.
- The CAPCO-2016 manual (`crates/capco/docs/CAPCO-2016.md`, with the PDF original at `crates/capco/docs/original-refs/CAPCO-2016.pdf`) is the authoritative source for ISM/CAPCO during the scope of this work.
- The ODNI ISM XML schema pin stays at `ISM-v2022-DEC`; any bump is out of scope and handled per Constitution Principle IV's schema-version discipline.
- The mangled-marking fixture is generated from a corpus artifact (the Enron corpus high-confidence markings subjected to known mangling transforms); the fixture source is vendored or generated at build time and the corpus artifact is author-supplied, not shipped in the repository.
- The existing ≥95% per-rule accuracy gate in the corpus harness remains in force and is the non-negotiable floor for all three phases.
- The engine already supports bidirectional operation (authoring and batch reconciliation) as established in prior phases. This work extends, rather than introduces, that bidirectionality.
- Phases F (CUI), G (ControlBlock generalization), and H (diff rules) are out of scope here but are the immediate consumers of the trait surfaces pinned in Phase E; their needs are not re-litigated in this specification.
- Audit-record content-ignorance (Constitution Principle V) already holds today and is preserved, not newly established, by this work. No document content crosses the engine boundary into audit output in any of the three phases.
