<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Marque: Phase M — Distribution & Adoption Surfaces

**Date:** 2026-05-19
**Status:** strategic — defines the distribution / adoption track. Sits
alongside Phases J/K/L (`2026-04-20-long-horizon-roadmap.md`) on a
separate axis: those phases evolve the engine's *capabilities*; Phase M
evolves the engine's *reach*. Both are post-Phase-H.
**Builds on:**
- `2026-04-20-long-horizon-roadmap.md` — strategic horizon (J/K/L).
- `2026-04-19-recursive-lattice-and-decoder.md` — engine maturity
  (Phases B–H) Phase M consumes as a stable foundation.

## 0. What this is

By the time Phases B–H ship, Marque has a stable engine that ingests
anything `marque-extract` reads, recognizes any registered scheme,
validates via declarative constraints, composes via lattice joins, and
diffs two markings. That is *capability*. **Distribution and adoption
are a separate axis** — a perfect engine with no editor integration is
a CLI nobody runs.

Phase M defines five adoption surfaces that share three properties:

1. **Grammar-agnostic by construction.** Every surface consumes the
   engine via existing API boundaries (CLI, WASM, server, audit
   stream). None bakes in CAPCO/ISM assumptions. Adding CUI / NATO /
   future schemes (Phase L) lights up every surface without surface
   changes.
2. **Reduce adoption friction.** The MVP today requires a user to
   invoke a CLI on a file. Each surface in Phase M moves Marque to
   where work already happens: inside Office, inside web editors,
   inside cloud pipelines, inside org dashboards.
3. **Sustain the project.** Licensing is mixed: the engine and
   WASM-safe core stay under the Marque License (commercial-use-
   restricted source-available) per Constitution Tech Stack;
   adoption surfaces split into Marque License (UI kit, K8s,
   admin) and proprietary tiers (Office COM, AWS Step Functions
   deployment-in-a-box) where the work justifies it.

Each surface is internally phaseable; this doc sketches scope and the
key design questions, not implementation. Tracking issues land per
surface; PR-level decomposition is per-issue.

## 1. Surface map

| Tag | Surface                                    | License                                  | Status               |
| --- | ------------------------------------------ | ---------------------------------------- | -------------------- |
| M1  | Office 365 plugin family                   | Office.js: ML-1.0 source-viewable; COM: proprietary | this doc §2 |
| M2  | Kubernetes deployment (Helm + Operator)    | ML-1.0                                   | this doc §3          |
| M3  | AWS Lambda deployment-in-a-box             | proprietary                              | this doc §4          |
| M4  | Live-editing UI kit (web components + React) | ML-1.0                                 | this doc §5          |
| M5  | Multi-level admin UI + dashboard           | ML-1.0                                   | this doc §6          |

Cross-surface dependency: M4 (UI kit) underpins M1 (Office add-ins'
side panels) and M5 (admin UI). M5 consumes M2 or M3 for its server
backplane. M3 is standalone but reuses the audit-emit crate that
already feeds M5's audit-record browser.

## 2. M1 — Office 365 plugin family

**Goal:** live linting, autofix suggestions, and reference popovers
inside Outlook, Word, PowerPoint, and Excel — in the application where
classified-content authoring already happens.

**Priority order:** Outlook → Word → PowerPoint → Excel. Outlook leads
because email is the highest-volume marking surface and where
mismarkings most often escape review.

### 2.1 Two tiers

- **Tier 1 — Office.js web add-in (ML-1.0 source-viewable, license-
  gated for redistribution).** Cross-platform (Win/Mac/Web/iOS),
  manifest-distributed via AppSource or sideloaded for org-managed
  deployments. Consumes `marque-wasm` via a JS bridge; the engine
  runs in the add-in's task pane sandbox.
- **Tier 2 — Native COM add-in (proprietary enterprise tier).**
  Windows-only. Deeper API access — real-time on-keystroke
  diagnostics (Office.js debounces at ~500ms), ribbon controls,
  classification-aware document property writes. Ships later, after
  Tier 1 proves the value proposition.

### 2.2 Scope per surface

| App         | Surfaces marked                                   | Tier-1 minimum                                    |
| ----------- | ------------------------------------------------- | ------------------------------------------------- |
| Outlook     | subject line, body, attachments-list summary      | diagnostics on send; pre-send block on E-severity |
| Word        | document body, headers/footers, classification block | diagnostics live in margin; ribbon for full-doc fix |
| PowerPoint  | slide titles, content, speaker notes, classifications | per-slide diagnostics; classification ribbon       |
| Excel       | cell-level (formula and value), worksheet titles  | cell-decorator diagnostics; on-save lint pass     |

### 2.3 Engine integration

- The add-in is a thin shell over `marque-wasm` (existing crate).
- No engine code lives in the add-in — adding a new scheme (Phase L)
  is a WASM rebuild + manifest version bump, never an add-in code
  change.
- Audit records flow to the configured audit destination (local
  storage for individual users; server-side for org deployments).

### 2.4 Open questions

- License-check mechanism for Tier 1 (offline-tolerant vs always-
  online vs license-server pull). Reserve the design; ship Tier 1
  with a permissive default and tighten in M5 admin UI.
- AppSource publication path vs sideloading-only for early access.

## 3. M2 — Kubernetes deployment

**Goal:** turnkey K8s deployment for batch and API workloads. Drop
into any cluster with `helm install`; scale on QPS.

### 3.1 Two stages

- **Helm chart (initial).** Wraps `marque-server` as a Deployment,
  with optional `marque-extract` sidecar for format conversion,
  optional Redis for rate-limit state, ConfigMap-driven scheme
  enablement, HPA on QPS/latency, NetworkPolicy templates,
  PodSecurityStandard `restricted` profile.
- **Operator (follow-on).** Custom `MarqueCluster` CRD with multi-
  tenant config, per-scheme version pinning, automated scheme
  rollouts, canary deploys, audit-log retention policies.

### 3.2 Hardening built-in

- Cosign-signed images and SBOM attestation (consolidates #197).
- ReadOnlyRootFilesystem, dropped capabilities, no-new-privileges.
- Helm test suite — smoke + integration golden corpus from
  `tests/corpus/documents/`.
- Prometheus metrics endpoint; default Grafana dashboard.

### 3.3 Open questions

- Audit-record persistence: bundled Postgres (Helm dependency) vs
  bring-your-own (most realistic for compliance environments)?
- Multi-tenancy story — namespace-per-tenant vs in-pod sub-tenant
  isolation. Defers to Operator phase.

## 4. M3 — AWS Lambda deployment-in-a-box

**Goal:** proprietary, turnkey AWS deployment. Customer picks one of
two reference architectures by workload shape; both are billed as
deployment-in-a-box (recurring or fixed).

### 4.1 Two reference architectures

- **Per-crate pipeline:** Step Functions orchestrates
  `marque-extract` → `marque-engine` → audit-emit, each as its own
  Lambda. Lower latency per stage, finer billing granularity, deeper
  per-stage observability. Choice for low-latency interactive APIs.
- **Monolithic + fan-out:** single Lambda runs the full
  extract→engine→emit pipeline; Step Functions handles batch
  enumeration, retry, and fan-out parallelism over S3 corpora.
  Simpler ops, lower cold-start cost per document. Choice for
  large-corpus batch sweeps.

Customer picks per workload; both ship as separate CDK templates.

### 4.2 What ships in the box

- CDK (primary) + Terraform (secondary) templates for both
  architectures.
- Least-privilege IAM roles per Lambda; KMS keys for at-rest
  encryption.
- S3 buckets for input, output, and audit records (object-lock
  optional for compliance).
- DynamoDB or S3 for audit-record index.
- X-Ray tracing wired across Step Functions stages.
- CloudWatch dashboards (default) and alarms (configurable).
- Lambda layer pre-built with the WASM artifact and any native
  binary for the Linux x86_64 musl target.

### 4.3 Open questions

- Native vs WASM in Lambda runtime — native is faster but doubles
  the build matrix. Reserve: ship WASM-on-Lambda first; native as
  follow-on optimization with measured win.
- ARM64 (Graviton) variants — defer until corpus measurements
  justify the matrix expansion.

## 5. M4 — Live-editing UI kit

**Goal:** drop-in components for embedding Marque in web apps,
Electron apps, internal tools, and the M1/M5 surfaces themselves.

### 5.1 Stack

- **Core:** Web Components (Lit-based). Frameworkless. Works in
  vanilla JS, Vue, Svelte, Electron, any framework that consumes
  custom elements.
- **React adapter:** thin wrappers over each Web Component
  (`react-wrap-balancer` style). Published as a sibling npm package.
- **TypeScript reference implementation.** Strong types over the
  diagnostic / suggestion / audit-record surface.
- Published to npm under `@marque-tools/` (or equivalent).

### 5.2 Initial component set

- `<marque-diagnostic-marker>` — inline squiggly underline (info /
  warn / error severity).
- `<marque-diagnostic-tooltip>` — hover/click tooltip with rule ID,
  citation, message, and suggested fix.
- `<marque-suggestion-menu>` — apply / ignore-once / ignore-rule /
  explain dropdown.
- `<marque-reference-popover>` — primary-source excerpt viewer
  (consumes per-token help text, #255).
- `<marque-info-banner>` — top/bottom dynamic banner (classification
  rollup, marking count, status). Configurable position + density.
- `<marque-editor>` — composed full editor (textarea + overlay +
  diagnostic panel) for one-line integrations.

### 5.3 Companion: web-editor-in-a-box

A pre-composed app shell — `<marque-editor>` plus document-load /
save / share controls. The fastest path from "I want Marque in my
web app" to a working integration.

### 5.4 Accessibility + i18n

- WCAG AA conformance from day one.
- Reduced-motion respected (no required motion for diagnostics).
- Reference popover text localizable; severity labels translatable.
- Storybook + CodeSandbox examples in launch.

### 5.5 Dependencies

- `marque-wasm` (existing).
- Vocabulary lookup surface (#254) — needed for reference popover.
- Per-token help text (#255) — needed for reference popover.
- Audit-record shape stability (post-PR-3c.2 schema bump).

## 6. M5 — Multi-level admin UI + dashboard

**Goal:** web UI for policy management, audit oversight, and
adoption insight. Built from M4 components.

### 6.1 Three levels, three audiences

| Level | Audience           | Surface                                                                                          |
| ----- | ------------------ | ------------------------------------------------------------------------------------------------ |
| 1     | Individual user    | "my marking activity" — counts, fix-acceptance rate, recent diagnostics, personal trend          |
| 2     | Team / program lead | rule severity overrides, corrections map editor, scheme enablement, per-team rollups            |
| 3     | Security / compliance | audit-record browser, aggregated stats, anomaly flags, exportable reports (PDF, CSV)         |

### 6.2 Functional scope

- **Policy management:** edit `.marque.toml` settings through a UI
  with policy preview, dry-run against a corpus, then commit; per-
  org policy storage; scheme version pinning.
- **Scheme registration:** enable/disable installed schemes (CAPCO
  today, CUI / NATO when Phase L lands); per-tenant overrides.
- **Auditing & reporting:**
  - Stats and counts for users (consumes audit records the user
    classified — *not* document content; G13 / I-J2 still binding).
  - Aggregated dashboards for security/compliance (per-rule fire
    rate, per-classifier acceptance rate, ingest volume by
    surface).
  - Report templates with PDF / CSV export.
  - Anomaly flags (e.g., unusually high override rate by
    classifier).

### 6.3 Auth + multi-tenancy

- OIDC and SAML support out of the box.
- RBAC: per-tenant roles (user / lead / compliance) map to Level
  1/2/3 surfaces.
- Tenant isolation at the data layer.

### 6.4 Compliance invariants

The admin UI is downstream of the engine's audit stream. Every
display, export, and aggregation MUST preserve the Constitution V
audit-content-ignorance invariant: no document content, no
metadata field values, no subject-claim text. Permitted: rule
IDs, severities, span offsets, digests, posterior scalars, counts,
classifier IDs, scheme IDs.

### 6.5 Dependencies

- M4 (UI kit).
- M2 or M3 for server-side audit storage.
- Stable audit-record schema (`marque-1.0`, post-PR-3c.2).

## 7. Architectural invariants that carry

Every surface in Phase M is bound by these from the engine constitution:

- **Grammar-agnostic shape.** No surface bakes in CAPCO/ISM
  assumptions. Adding a scheme is a config change, never a surface
  change. Verified by: dispatching M1 / M4 / M5 against a test
  fixture scheme (Phase L F2 / F3) before declaring done.
- **Audit content-ignorance (Constitution V).** No surface — not the
  Office add-in, not the UI kit's diagnostic tooltip, not the admin
  UI's aggregated report — may emit document content into the
  audit stream or to any downstream sink that reaches the audit
  store.
- **WASM-safe core stays WASM-safe (Constitution III).** M1 / M4
  consume `marque-wasm`; the engine and its dependencies stay
  free of format adapters and runtime-loaded recognizers.
- **Engine has one promotion path (Constitution V).** No surface
  fabricates `AppliedFix` records or bypasses the engine's
  confidence-threshold gate.

## 8. Open questions

- **License-check mechanism.** Office.js Tier 1 ("open proprietary,
  source viewable, license required") needs a license-check shape
  that doesn't break offline editing. Defer the design; ship
  permissive default; tighten via M5 admin UI when ready.
- **Cost model.** M3 deployment-in-a-box and M1 Tier 2 COM add-in
  are proprietary; the cost / billing surface is out of scope for
  this doc and will land separately.
- **Adoption metric definition.** M5 "adoption metrics" needs a
  precise definition before instrumentation. Defer to M5 sub-phase.
- **Cross-surface SSO.** M1, M4-consumers, and M5 each authenticate
  separately today. Eventual unification (OIDC issuer shared across
  surfaces) is a follow-on.

## 9. Non-goals

- **Replacing the CLI.** The CLI stays the integration-testable,
  scriptable baseline. Adoption surfaces add reach without
  removing the surface that grounds testing.
- **Replacing `marque-server`.** M2 and M3 are deployment
  conveniences around the existing server; they do not fork the
  server's API surface.
- **In-tree mobile clients.** Office mobile add-ins (Tier 1) work
  via the Office.js cross-platform manifest; a native iOS / Android
  Marque app is out of scope.
- **Replacing existing #254 / #255 / #197.** Those issues are
  prerequisites for surfaces in this phase, not subsumed by them.

## 10. Revision policy

Phase M depends only on Phase H of the long-horizon roadmap (engine
stability after diff rules ship). Reordering of M1–M5 is allowed and
expected; the priority above reflects current product judgment, not
build dependency.

If a near-term engine phase (B–H) changes shape in a way that breaks
an assumption in this doc (e.g., the audit-record schema shifts in
a way that invalidates M5 §6.4), this doc updates in response, not
the other way around.

## 11. Mapping to existing issues

- `#254` (vocabulary lookup surface) — prerequisite for M4 reference
  popover.
- `#255` (per-token authoritative help text) — prerequisite for M4
  reference popover + M1 hover tooltips.
- `#197` (Docker with cosign + SBOM) — M2 consumes; M3 reuses image
  build pipeline.
- `#189` (suggest-while-you-type) — M1 Tier 2 (COM) and M4 editor
  composition consume this as a behavior.
- `#338` (cross-grammar interconversion) — Phase L work; M1 / M4 /
  M5 must remain grammar-agnostic regardless of which schemes are
  installed.
- `#184` (tamper-evident audit log) — M2 / M3 server-side audit
  consumes; M5 audit browser displays integrity status.
