# Phase M issue drafts — review before posting

Five tracking issues to open against `marquetools/marque`. Each is a
roadmap-level feature, grammar-agnostic, ML-1.0 unless noted. Labels:
`enhancement, post-refactor, design-deferred, tracking`. Suggested
additional labels per issue listed below; create them if missing.

Roadmap doc: `docs/plans/2026-05-19-distribution-and-adoption.md`.

---

## Issue 1 — feat(integration): Office 365 plugin family — Office.js add-in (open-proprietary) + COM enterprise tier

**Labels:** `enhancement, post-refactor, design-deferred, tracking, javascript`
(consider adding `integration` and `office` labels)

### Summary

Live linting, autofix suggestions, and reference popovers inside
Outlook, Word, PowerPoint, and Excel. Grammar-agnostic (consumes
`marque-wasm`); ships in two tiers. Reduces adoption friction by
putting Marque where classified-content authoring already happens.

### Priority order

Outlook → Word → PowerPoint → Excel. Email is the highest-volume
marking surface; lead there.

### Two tiers

- **Tier 1 — Office.js web add-in (ML-1.0 source-viewable, license-
  gated for redistribution).** Cross-platform (Win/Mac/Web/iOS).
  Manifest-distributed via AppSource or org-managed sideloading.
  Consumes `marque-wasm` in the task-pane sandbox. Debounced
  on-edit linting + on-send block on E-severity diagnostics.
- **Tier 2 — Native COM add-in (proprietary enterprise tier; later).**
  Windows-only. Deeper API access: real-time on-keystroke
  diagnostics, ribbon controls, classification-aware document
  property writes.

### Per-app scope (Tier 1 minimum)

| App        | Marked surfaces                                       | Tier 1 minimum                                            |
| ---------- | ----------------------------------------------------- | --------------------------------------------------------- |
| Outlook    | subject, body, attachments-summary                    | diagnostics on send; pre-send block on E-severity         |
| Word       | body, headers/footers, classification block           | margin diagnostics; ribbon for full-doc fix              |
| PowerPoint | slide titles, content, speaker notes, classifications | per-slide diagnostics; classification ribbon              |
| Excel      | cell-level (formula and value), worksheet titles      | cell-decorator diagnostics; on-save lint pass             |

### Engine integration

- Thin shell over `marque-wasm` (existing crate).
- No engine code in the add-in — adding a scheme (Phase L) is a
  WASM rebuild + manifest version bump, never an add-in code change.
- Audit records flow to configured destination (local for
  individual users; server-side for org deployments).

### Acceptance criteria

- [ ] Clean sideload install on Win/Mac Office.
- [ ] Diagnostics fire on a test corpus across all four apps.
- [ ] Pre-send block honored in Outlook on E-severity.
- [ ] License-check stub present (permissive default; design follow-on).
- [ ] Audit-record content-ignorance verified (Constitution V).
- [ ] Grammar-agnostic check: dispatch against a test fixture scheme
      passes without add-in code changes.

### Dependencies

- `marque-wasm` parity (✅ shipped).
- M4 UI kit (#new) — diagnostic tooltip + reference popover components.
- #255 (per-token authoritative help text) — for reference popover.

### Sub-issues to file once this lands

- M1.1 Outlook add-in (lead)
- M1.2 Word add-in
- M1.3 PowerPoint add-in
- M1.4 Excel add-in
- M1.5 License-check design
- M1.6 COM add-in (Tier 2)

### References

- `docs/plans/2026-05-19-distribution-and-adoption.md` §2
- Constitution III (WASM safety), Constitution V (audit content-
  ignorance)

---

## Issue 2 — feat(deploy): Kubernetes deployment — Helm chart + Operator for batch/API processing

**Labels:** `enhancement, post-refactor, design-deferred, tracking, security`
(consider adding `deploy` and `kubernetes` labels)

### Summary

Turnkey K8s deployment of `marque-server` for batch and API
workloads. `helm install` brings up a working cluster; scales on
QPS/latency. License: Marque License 1.0.

### Two stages

- **Helm chart (initial).** `marque-server` Deployment, optional
  `marque-extract` sidecar for format conversion, optional Redis
  for rate-limit state, ConfigMap-driven scheme enablement, HPA on
  QPS/latency, NetworkPolicy templates, PodSecurityStandard
  `restricted` profile.
- **Operator (follow-on).** Custom `MarqueCluster` CRD with multi-
  tenant config, per-scheme version pinning, automated scheme
  rollouts, canary deploys, audit-log retention policies.

### Hardening built-in

- Cosign-signed images and SBOM attestation (consolidates #197).
- ReadOnlyRootFilesystem, dropped capabilities, no-new-privileges.
- Helm test suite — smoke + integration golden corpus from
  `tests/corpus/documents/`.
- Prometheus metrics endpoint; default Grafana dashboard.

### Acceptance criteria

- [ ] `helm install` brings up working cluster with LoadBalancer/Ingress.
- [ ] `helm test` passes against golden corpus.
- [ ] Images cosign-verified by `helm install` pre-hook.
- [ ] HPA scales under synthetic load; metrics scrapable.
- [ ] `helm uninstall` cleanly removes all resources.
- [ ] Grammar-agnostic check: enabling a fixture scheme via ConfigMap
      works without chart changes.

### Dependencies

- `marque-server` stability.
- #197 (Docker + cosign + SBOM) — image build pipeline.
- Audit-log persistence story (bundled Postgres vs BYO).

### Sub-issues to file once this lands

- M2.1 Helm chart skeleton + smoke tests
- M2.2 Cosign + SBOM verification gate
- M2.3 NetworkPolicy + PodSecurityStandard templates
- M2.4 Prometheus metrics + Grafana dashboard
- M2.5 Operator design (CRD shape + reconcile loop)
- M2.6 Operator implementation

### References

- `docs/plans/2026-05-19-distribution-and-adoption.md` §3
- #197 (consolidates), #184 (audit log integrity)

---

## Issue 3 — feat(deploy): AWS Lambda deployment-in-a-box — two reference Step Functions templates

**Labels:** `enhancement, post-refactor, design-deferred, tracking`
(consider adding `deploy` and `aws` labels)

### Summary

Turnkey AWS deployment as a proprietary deployment-in-a-box.
Customer picks one of two reference architectures by workload
shape; both ship as separate CDK templates.

### Two reference architectures

- **Per-crate pipeline.** Step Functions orchestrates
  `marque-extract` → `marque-engine` → audit-emit, each as its own
  Lambda. Lower latency per stage, finer billing granularity,
  deeper per-stage observability. For low-latency interactive APIs.
- **Monolithic + fan-out.** Single Lambda runs the full
  extract→engine→emit pipeline; Step Functions handles batch
  enumeration, retry, and fan-out parallelism over S3 corpora.
  Simpler ops, lower cold-start cost per document. For large-
  corpus batch sweeps.

### What ships in the box

- CDK (primary) + Terraform (secondary) templates for both
  architectures.
- Least-privilege IAM roles per Lambda; KMS keys for at-rest
  encryption.
- S3 buckets for input, output, audit records (object-lock
  optional).
- DynamoDB or S3 for audit-record index.
- X-Ray tracing wired across Step Functions stages.
- CloudWatch dashboards (default) and alarms (configurable).
- Lambda layer pre-built with WASM artifact + Linux x86_64 musl
  native binary.

### Acceptance criteria

- [ ] `cdk deploy` brings up both architectures from a fresh AWS account.
- [ ] Example invocation succeeds end-to-end with audit records written.
- [ ] X-Ray trace shows expected stage boundaries.
- [ ] CloudWatch dashboard populated by load test.
- [ ] Cold-start measured + documented for both architectures.
- [ ] Grammar-agnostic check: enabling a fixture scheme works via
      environment variables without template changes.

### Dependencies

- Native Linux x86_64 musl build of the engine.
- Audit-emit as a discrete crate (already shipped).
- Decision: WASM-on-Lambda first vs native — design questions in
  §4.3 of the Phase M doc.

### Sub-issues to file once this lands

- M3.1 Lambda layer build pipeline (WASM + musl native)
- M3.2 Per-crate CDK template
- M3.3 Monolithic + fan-out CDK template
- M3.4 Terraform parity
- M3.5 Pricing / billing model
- M3.6 ARM64 (Graviton) variants (design-deferred)

### References

- `docs/plans/2026-05-19-distribution-and-adoption.md` §4

---

## Issue 4 — feat(integration): Marque live-editing UI kit — Web Components core + React adapter

**Labels:** `enhancement, post-refactor, design-deferred, tracking, javascript`
(consider adding `integration` and `ui-kit` labels)

### Summary

Drop-in components for embedding Marque in web apps, Electron
apps, internal tools, and the M1/M5 surfaces themselves.
Frameworkless core (Web Components / Lit) + thin React adapter.
TypeScript reference implementation. Published to npm. License:
Marque License 1.0.

### Stack

- **Core:** Web Components (Lit-based). Works in vanilla JS, Vue,
  Svelte, Electron, any framework that consumes custom elements.
- **React adapter:** thin wrappers over each Web Component,
  sibling npm package.
- **TypeScript reference implementation.** Strong types over the
  diagnostic / suggestion / audit-record surface.

### Initial component set

- `<marque-diagnostic-marker>` — inline squiggly underline
  (info / warn / error).
- `<marque-diagnostic-tooltip>` — hover/click tooltip with rule
  ID, citation, message, suggested fix.
- `<marque-suggestion-menu>` — apply / ignore-once / ignore-rule /
  explain dropdown.
- `<marque-reference-popover>` — primary-source excerpt viewer
  (consumes per-token help text, #255).
- `<marque-info-banner>` — top/bottom dynamic banner
  (classification rollup, marking count, status).
- `<marque-editor>` — composed full editor (textarea + overlay +
  diagnostic panel) for one-line integrations.

### Companion: web-editor-in-a-box

Pre-composed app shell — `<marque-editor>` plus document load /
save / share controls. The fastest path from "I want Marque in my
web app" to a working integration.

### Accessibility + i18n

- WCAG AA conformance from day one.
- Reduced-motion respected (no required motion for diagnostics).
- Reference popover text localizable; severity labels translatable.
- Storybook + CodeSandbox examples in launch.

### Acceptance criteria

- [ ] All initial components published to npm under a Marque scope.
- [ ] Storybook live with every component documented.
- [ ] CodeSandbox examples for vanilla, React, and one alt framework.
- [ ] WCAG AA verified (axe-core in CI).
- [ ] Reduced-motion compliance verified.
- [ ] Web-editor-in-a-box renders against a test corpus.
- [ ] Grammar-agnostic check: works against a fixture scheme without
      kit changes.

### Dependencies

- `marque-wasm` (existing).
- #254 (vocabulary lookup API surface) — reference popover content.
- #255 (per-token authoritative help text) — reference popover content.
- Audit-record shape stability (post-PR-3c.2).

### Sub-issues to file once this lands

- M4.1 Diagnostic marker + tooltip
- M4.2 Suggestion menu
- M4.3 Reference popover
- M4.4 Info banner
- M4.5 Composed `<marque-editor>`
- M4.6 React adapter package
- M4.7 Web-editor-in-a-box shell
- M4.8 Storybook + accessibility CI

### References

- `docs/plans/2026-05-19-distribution-and-adoption.md` §5
- #254, #255, #189

---

## Issue 5 — feat(integration): Multi-level admin UI + dashboard — policy + auditing + reporting

**Labels:** `enhancement, post-refactor, design-deferred, tracking, javascript`
(consider adding `integration` and `admin-ui` labels)

### Summary

Web UI for policy management, audit oversight, and aggregated
auditing reports. Built from M4 UI kit components. Three audience
levels: individual user (counts), team lead (policy), security/
compliance (aggregated stats + reports). License: Marque License 1.0.

### Three levels, three audiences

| Level | Audience              | Surface                                                                                      |
| ----- | --------------------- | -------------------------------------------------------------------------------------------- |
| 1     | Individual user       | "my marking activity" — counts, fix-acceptance rate, recent diagnostics, personal trend     |
| 2     | Team / program lead   | rule severity overrides, corrections map editor, scheme enablement, per-team rollups        |
| 3     | Security / compliance | audit-record browser, aggregated stats, anomaly flags, exportable reports (PDF, CSV)        |

### Functional scope

- **Policy management:** UI editor for `.marque.toml` settings with
  policy preview, dry-run against a corpus, then commit; per-org
  policy storage; scheme version pinning.
- **Scheme registration:** enable/disable installed schemes (CAPCO
  today, Phase L additions later); per-tenant overrides.
- **Auditing & reporting:**
  - User-level stats and counts (from the audit records the user
    classified — *not* document content).
  - Aggregated dashboards for compliance: per-rule fire rate, per-
    classifier acceptance rate, ingest volume by surface.
  - Report templates with PDF / CSV export.
  - Anomaly flags (e.g., unusually high override rate by
    classifier).

### Auth + multi-tenancy

- OIDC and SAML out of the box.
- RBAC: per-tenant roles (user / lead / compliance) map to
  Level 1/2/3 surfaces.
- Tenant isolation at the data layer.

### Compliance invariants

Downstream of the engine's audit stream. Every display, export,
and aggregation MUST preserve Constitution V audit-content-
ignorance: no document content, no metadata field values, no
subject-claim text. Permitted in display: rule IDs, severities,
span offsets, digests, posterior scalars, counts, classifier IDs,
scheme IDs.

### Acceptance criteria

- [ ] Multi-tenant install with two demo tenants.
- [ ] RBAC enforced — Level 1 user cannot see Level 3 aggregated stats.
- [ ] OIDC and SAML login both work against test IdPs.
- [ ] Policy editor produces valid `.marque.toml` and dry-run preview
      matches an offline run.
- [ ] Audit-record browser indexes a 10k-record corpus and queries
      sub-second.
- [ ] PDF + CSV reports generate against fixture data.
- [ ] Content-ignorance audit: scrape the admin UI for any document
      text emission against the test corpus.
- [ ] Grammar-agnostic check: enabling a fixture scheme surfaces in
      scheme-enablement UI without admin-UI changes.

### Dependencies

- M4 (UI kit).
- M2 or M3 for server-side audit storage.
- Stable audit-record schema (`marque-1.0`, post-PR-3c.2).

### Sub-issues to file once this lands

- M5.1 Policy editor + dry-run preview
- M5.2 Scheme registration UI
- M5.3 Audit-record browser
- M5.4 Aggregated dashboards (Level 3)
- M5.5 Personal stats (Level 1)
- M5.6 Report generation (PDF / CSV)
- M5.7 Auth + RBAC + multi-tenancy
- M5.8 Anomaly detection design

### References

- `docs/plans/2026-05-19-distribution-and-adoption.md` §6
- Constitution V (audit content-ignorance), #184 (audit integrity)
