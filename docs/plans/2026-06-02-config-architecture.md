<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Marque: configuration architecture (decoupling, subsystems, cross-interface parity)

**Date:** 2026-06-02
**Status:** design — organizes a set of open issues (#641, #857, #645,
#244, #337, #252, #253, #826) under one architecture. No code lands
from this document directly; it defines the target shape the child
issues implement against.
**Builds on:**
- `2026-05-29-document-scope-artifacts-and-multi-scheme.md` — the
  multi-grammar engine (`ErasedEngine` / `MultiGrammarEngine`) this
  config architecture must serve.
- `2026-04-20-long-horizon-roadmap.md` — grammar expansion (Phase L)
  and operational-mode taxonomy that several config subsystems feed.
- Constitution v1.8.0 — Principles II (zero-copy / wipe-on-drop),
  III (WASM safety / runtime-config restriction), IV (two-layer
  rules), V (audit-first), VII (crate discipline), VIII (source
  fidelity).

## 0. What this document is

`marque-config` was built for one grammar. It depends on `marque-ism`,
exposes a typed `CapcoConfig` field, hard-codes a `[capco]` TOML table,
and validates a single global schema version against ODNI's
`SCHEMA_VERSION`. That was correct when CAPCO/ISM was the only scheme.
It is now the thing blocking a second grammar (#641).

Fixing the coupling surfaced a larger truth: **"config" is not one
thing.** The word currently covers at least four distinct subsystems
with different regimes, different trust boundaries, and different
homes in the crate graph. And the three I/O surfaces (WASM, server,
CLI) each marshal a bespoke, non-overlapping subset of it — so the
same capability (rule severity) is configurable on one surface and
silently absent on another.

This document separates the four subsystems, draws the regime line the
Constitution requires, and defines a single runtime config schema that
every surface accepts at a capability tier set by its trust boundary.

## 1. Two organizing axes

Everything called "config" sorts on two axes. Get these right and the
homes fall out.

### 1.1 Regime: build-time vs runtime (the Constitution III spine)

Constitution III draws a hard line:

> Severity overrides and corrections maps (data already present in the
> strict-path codepath) are permitted; anything that introduces a new
> recognizer codepath or alters recognizer posteriors (e.g., decoder
> priors) MUST be compiled in, not loaded at runtime.

The rationale is the WASM trust boundary — a config table arriving over
`postMessage` is an uninspected channel a browser embedder cannot
sandbox. So:

- **Build-time** — anything that expands the recognizer's semantic
  surface: new automaton tokens, decoder priors, grammar-default
  vocabulary tables, compiled-in signer allow-lists. These are
  compiled in, never loaded at runtime. **Not `marque-config`'s job.**
- **Runtime** — anything that gates or annotates *already-compiled*
  behavior: severity, corrections/substitutions, identity, threshold,
  historical evaluation window, carry-signature. This is
  `marque-config`'s job, and even here WASM gets a narrower slice than
  trusted callers.

The line is load-bearing. A subsystem on the wrong side of it is a
security defect, not a style choice.

### 1.2 Scope: neutral vs per-grammar vs grammar-data

- **Cross-cutting neutral** — engine/session properties that have
  nothing to do with any marking grammar (identity, threshold,
  signing, severity *machinery*). One instance, top-level.
- **Per-grammar knob** — a setting every grammar has but values
  differ per grammar (schema version, historical `as_of`). Lives in a
  registry keyed by scheme name; parsed/validated by the grammar.
- **Grammar-owned data** — content only one grammar understands
  (SCI/SAR vocabulary, REL country lists, alias tables). Supplied by
  the grammar (build-time defaults) and optionally extended by the
  operator.

## 2. The four subsystems

### 2.A Runtime config loader + `SchemeConfig` (the inversion)

**Problem.** `marque-config` depends on `marque-ism` (for
`SCHEMA_VERSION` and `UtcOffset`) and owns a typed `CapcoConfig`. A
second grammar has no home.

**Target.** Invert the dependency. A minimal trait expresses the
per-grammar config contract; the grammar implements it; `marque-config`
becomes machinery that knows no grammar.

- Trait lives in **`marque-scheme`** (the domain-neutral trait leaf),
  not in `marque-config`. This inverts the coupling *without* minting a
  `marque-capco → marque-config` edge — capco already depends on
  `marque-scheme`.

  ```rust
  // marque-scheme
  pub trait SchemeConfig: Default + Sized {
      const SCHEME_NAME: &'static str;              // "capco"
      fn compiled_schema_version() -> &'static str; // grammar supplies
      fn parse(raw: &RawConfigTable) -> Result<Self, SchemeConfigError>;
      // provided: validate parsed.version == compiled_schema_version()
  }
  ```

- `marque-config` keeps the layered reader (toml / json / env
  precedence) and produces neutral top-level fields plus a registry of
  raw per-scheme tables. The `raw → typed` resolution happens in the
  **engine**, which already knows both the scheme and capco.
- `Config` reshapes to: neutral top-level fields + `schemes:
  HashMap<String, Box<dyn SchemeConfig>>` (or a typed registry).
- **`marque-config → marque-ism` disappears**: `SCHEMA_VERSION` moves
  into capco's impl; `UtcOffset` becomes a field on capco's config
  struct, parsed inside capco.

**Do not fragment everything per-grammar.** Identity,
`confidence_threshold`, `require_signature`, and the
`rules`/`closure_rules` severity maps stay top-level. The severity maps
are already wire-string-namespaced (`"capco:closure.…"`); one flat map
works across N grammars and fragmenting it would break the key scheme.
Only genuinely per-grammar values (`version`, `as_of`,
`default_timezone`, grammar substitution namespaces) go in the
registry.

Keep `SchemeConfig` minimal — name + schema version + parse. Heavier
concerns route to subsystems B/C/D, not onto this trait.

Issues: **#641** (decoupling RFC, parent of this subsystem).

### 2.B Build-time grammar extension ("build config")

**Problem.** An org runs non-public SCI control systems / compartments,
SAR programs, and their `MarkingForm`s (ISM / banner-title / banner-
abbreviation / portion). Today: no mechanism. SCI compartments and SAR
programs *are already recognized* structurally (`parse_sci_block`,
`parse_sar_category` handle the open-ended tails), but an org token
cannot get (a) a decoder prior, (b) MarkingForm metadata for canonical
rendering, or (c) a "sanctioned custom token" validity gate. All three
are compile-in by Constitution III (the aho-corasick automaton is
compile-time per the Tech Stack; priors are explicitly compile-in).

**Target.** A first-class **build-time extension** mechanism, distinct
from the runtime loader and structurally prevented from becoming one.

- One declared per-grammar build-input manifest (e.g.
  `marque.build.toml` or an env-pointed extension directory) consumed
  by the grammar's `build.rs`, emitting additional `&'static` tables
  alongside the ODNI-generated ones — mirroring how `corpus/priors.json`
  and the ODNI XML are already consumed.
- **DX is the point of this subsystem.** Today build-time extension
  means hand-editing `corpus/priors.json` and understanding each
  grammar's bespoke `build.rs`. The target is one documented entry
  point per grammar: drop a manifest, rebuild, get compiled-in tables —
  no build-script archaeology.
- WASM receives these compiled in like any other generated table; there
  is no runtime path, by construction.

This subsystem also hosts: grammar-default corrections tables (2.C
layer 1), grammar-default alias tables (#253 canonical layer), and the
compiled-in signer allow-list (2.D, #244).

Issues: new (custom-token extension); contributes to **#253**, **#244**.

### 2.C Pre-scanner substitution (corrections + macros + aliases unified)

**Insight.** Corrections (`[corrections]`), REL macros (#252), and
country aliases (#253) are **one mechanism**: a layered pre-scanner
substitution table (aho-corasick) fed by (layer 1) grammar-shipped
build-time defaults and (layer 2) runtime user additions. They differ
only in match semantics (whole-token vs substring, case-folding) and
content. #252 and #253 both already propose reusing the corrections
machinery. Build one substitution subsystem, not three features.

- Neutral machinery: the layered substitution engine (already partly
  present as the corrections pre-scanner).
- Grammar-owned data: default tables (build-time, subsystem B) +
  the runtime user layer (subsystem A loader).
- WASM-safe: textual preprocessing flows through the normal pipeline,
  adds no recognizer codepath, alters no posteriors — the same
  reasoning that makes corrections maps a named Constitution III
  exception covers macros and aliases.
- The lint-diagnostic half of #253 (suggest the canonical trigraph) is
  a normal grammar rule, separate from the substitution table.

Issues: **#252**, **#253** (collapse into this subsystem).

### 2.D Signing / trust

**Scope.** Audit provenance and config trust — grammar-agnostic, the
most clearly neutral subsystem. Straddles both regimes.

- **Build-time:** compiled-in ed25519 signer public-key allow-list
  (#244 — deliberately "opt-out is a cargo feature, not a runtime
  flag"; signer leaves → new build, no CRL/OCSP).
- **Runtime:** detached-signature verification over config/override
  files against that allow-list; `require_signature` policy gate; the
  carry-only caller signature (already shipped via #399).
- **Full in-engine signing** of the `session_root` Merkle root (#826)
  — needs a constitution amendment for the crypto dependency; heaviest
  item, sequenced last.
- A neutral config-trust layer can wrap *any* config source with
  signature verification (#244's "signed-config generalization" open
  question), not just corpus overrides.

Issues: **#244**, **#826**; builds on shipped #399.

## 3. Cross-interface parity: one schema, two capability tiers

**Principle.** There is one canonical runtime config schema. Every
surface accepts the same field names with the same validation. Surfaces
differ only by a **capability tier set at the trust boundary**, drawn
exactly at Constitution III's "expands recognizer surface / alters
posteriors" line — never by bespoke per-surface shapes.

Two tiers:

- **Sandboxed tier (WASM).** Cannot expand the recognizer surface.
  Accepts: identity, `confidence_threshold`, corrections/substitutions,
  **rule + closure severity**, carry-`signature`, deadline/budget,
  historical `as_of`, macros/aliases. Excludes: `input_source`
  (recognizer opt-in) and `corpus_override` (posterior change) — these
  are compiled out, not merely rejected.
- **Trusted tier (server, CLI).** Everything the sandboxed tier gets,
  **plus** `input_source` and (feature-gated) `corpus_override`. The
  server adds per-request granularity for multi-tenant; the CLI is
  process-global plus flags.

### 3.1 Current vs target surface matrix

`✅` accepted · `➕` target (this architecture adds it) · `🚫` excluded
by trust boundary (Constitution III) · `—` n/a

| Field | WASM | Server | CLI | Engine (typed) |
|-------|------|--------|-----|----------------|
| identity (classifier_id / authority) | ✅ | ✅ per-req | ✅ flags | ✅ |
| confidence_threshold | ✅ | ✅ per-req | ✅ | ✅ |
| corrections / substitutions | ✅ | ➕ per-req | ✅ (toml) | ✅ |
| **rule + closure severity** | ➕ (#857) | ➕ per-req | ✅ (toml) | ✅ |
| signature (carry) / require_signature | ✅ | ✅ | ✅ | ✅ |
| deadline / budget | ✅ | ✅ (cap) | — | — |
| historical `as_of` (#337) | ➕ | ➕ | ➕ | ✅ |
| macros / aliases (#252/#253) | ➕ | ➕ | ➕ | ✅ |
| `input_source` (recognizer opt-in, #176) | 🚫 | ✅ | ✅ | ✅ |
| `corpus_override` (#244) | 🚫 | 🚫 per-req (feature) | ✅ (feature) | ✅ |

The two biggest parity gaps the matrix exposes: **severity is missing
on WASM and per-request on the server** (the strongest multi-tenant
case, #857), and **corrections is boot-only on the server** (no
per-request substitution).

### 3.2 Mechanism

- The typed `Config` struct (consumed by `Engine::new`) is the
  canonical form. WASM's `WasmConfig`, the server's `LintRequest`/
  `FixRequest` per-request overlay, and the CLI's flags all deserialize
  into the *same* field set, differing only by which fields their tier
  compiles in.
- Per-request overrides (server multi-tenant, WASM per-call) overlay a
  base `Config` — the existing `FixOptions` pattern, generalized from
  identity-only to the full sandboxed-tier field set.
- **Engine-cache cardinality** is the one real wrinkle (already live in
  the WASM engine cache keyed on config). Per-tenant severity sets
  multiply distinct cache keys; decide bounded-LRU vs accept cheap
  re-keying. Severity-only deltas are cheap to apply, so re-keying may
  be acceptable; this is a measurement question, not a blocker.

## 4. Dependency-graph impact

- `marque-config` **loses** its `marque-ism` dependency (the
  undocumented edge). It ends up depending only on `marque-rules` (for
  `Severity`) and the serde/toml stack — a near-leaf.
- `SchemeConfig` lands in `marque-scheme` (the existing trait leaf). No
  new grammar→config edge is created.
- `marque-capco` gains nothing in its normal-dep set for subsystem A
  (it already deps `marque-scheme`). Subsystem B is a `build.rs`
  concern. Subsystem C's default tables are grammar consts.
- The engine remains the convergence point that resolves raw per-scheme
  tables into typed grammar configs — consistent with Constitution VII
  and the scheme-adoption-doesn't-edit-the-engine rule (IV): adding a
  grammar's config is implementing a trait, not editing the loader.

## 5. Phasing

1. **Decouple (subsystem A).** `SchemeConfig` in `marque-scheme`;
   `Config` reshape to neutral top-level + scheme registry; drop the
   `marque-ism` edge; capco implements the trait. Unblocks #641 and a
   second grammar. *Gates everything else.*
2. **Parity (subsystem A + §3).** Generalize the per-request overlay to
   the full sandboxed-tier field set; wire severity into WASM (#857) and
   per-request severity into the server; corrections per-request on the
   server. Decide the engine-cache policy.
3. **Pre-scanner substitution (subsystem C).** Unify corrections +
   macros (#252) + aliases (#253) into one layered substitution
   subsystem.
4. **Historical evaluation (#337).** `as_of` as a per-grammar runtime
   knob consuming the already-landed deprecation-window plumbing.
5. **Build config (subsystem B).** The per-grammar build-time extension
   manifest + DX. Custom SCI/SAR vocabulary, MarkingForms, priors.
6. **Signing (subsystem D).** Override-file verification + compiled-in
   allow-list (#244); full in-engine signing (#826) last (needs a
   constitution amendment).

## 6. Constitution touchpoints

- **II** — config-borne content (corrections, substitution tables) is
  not classified content, but any owned content-bearing buffer still
  wipes on drop; the loader holds no document content.
- **III** — the regime split (§1.1) and the two-tier surface (§3) exist
  to honor this principle precisely; the sandboxed tier's exclusions
  are its enforcement.
- **IV** — adding a grammar's config is a trait impl, not an engine
  edit; severity stays engine-applied, rules stateless.
- **V** — signing/trust (2.D) extends audit provenance to config and
  override decisions (#244's "every prior override traceable to a
  person and a date").
- **VII** — the dependency inversion (§4) keeps the graph acyclic and
  `marque-scheme` the trait leaf.
- **VIII** — substitution/alias tables are *additive* over authoritative
  vocabulary, never overriding it; macros that shadow canonical tokens
  are rejected (#252/#253 design notes).

## 7. Open questions

- **`default_timezone` scope.** Per-grammar (travels with the grammar's
  `parse`) or a global document-processing default? Leaning per-grammar,
  but it is a judgment call, not mechanical.
- **Engine-cache policy under per-tenant config** (§3.2) — bounded LRU
  vs accept re-keying. Needs measurement.
- **Signed-config generalization** (#244) — wrap all config sources in
  signature verification, or keep it surgical to corpus overrides?
- **`as_of` in multi-grammar** — one global "as of" date each grammar
  maps to its nearest schema version, or an independent per-grammar
  version pin? (#337 assumes single-grammar.)
- **Server per-request granularity vs boot config** — how much of the
  trusted-tier surface is per-request vs process-global.
