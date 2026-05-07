<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Engine + rule architecture refactor (consolidated, post-murder-board)

**Date:** 2026-05-02
**Status:** ready for implementation
**Amended:** 2026-05-07 — PR 3b staging re-sequenced per
`docs/plans/2026-05-07-pr3b-consultation-verdict.md`. **Source rule
count re-baselined to 59** (ground-truth `grep -c '^impl Rule for'`
across `rules.rs` / `rules_declarative.rs` / `rules_sci_per_system.rs`).
The "49 → ~10–13" count target appearing throughout this document
(user constraints, §3 PR 3b row, §4 PR 3b row, §11 PR 3b row,
mapping table) is **superseded** by the staged target — and the "49"
itself was a historical approximation, not a verified count. Operative
target: **PR 3b proper lands 13–18 surviving rules (Stage 1)**;
cumulative collapse to 9–11 lands across PR 3.7 / PR 4 / PR 5+. The
8–18 D13 band remains the end-state acceptance gate. The
authoritative staging table lives in
`specs/006-engine-rule-refactor/plan.md` D13 addendum and
`.claude/skills/marque-lattice-consultant/references/marque-applied.md`
§3.11. Two new primitives (`Constraint::Conflicts::RhsFamily(predicate)`
+ `MarkingScheme::closure(...)` per `marque-applied.md` §4.7) fold
into PR 3.7 — see `tasks.md` T108b / T108c. The body of this plan is
otherwise unaffected; PR sequencing (3a → 3b → 3c → 3.7 → 4 → 5+) is
unchanged.
**Supersedes:** `2026-05-01-engine-rule-architecture-refactor.md` (deleted in the PR that landed this plan)
**Gates:** `2026-05-01-lattice-design.md` (filled in by PR 3.7 — see §11)
**Synthesizes:**
- Three independent reviews (system architecture, QA, backend data-flow) of the
  open-issue corpus impacting rule behavior, correctness, and engine/CAPCO interaction.
- A user-driven decision pass that collapsed the open disagreements.
- A seven-reviewer adversarial murder board (system-architect, backend-architect,
  root-cause-analyst, quality-engineer, security-engineer, refactoring-expert,
  performance-engineer) and the user decisions resolving its findings.
**Builds on:**
- `2026-04-19-recursive-lattice-and-decoder.md` (Phase B trait surface)
- `2026-04-20-long-horizon-roadmap.md` (G13 invariant articulation)
- `2026-04-23-typos-evaluation-and-fuzzy-vocab-matcher.md`
**User constraints:**
- marque has no users, no downstream consumers, no API expectations. **Clean break
  is the operating philosophy.** This plan is the last clean-break window; the
  window closes when external consumers attach.
- Issue #263 (canonicalizer/renderer split + form-routing collapse,
  49 → ~10–13 rules) is a given. **(Amended 2026-05-07: target re-
  sequenced to 13–18 at PR 3b proper + 9–11 end-state across PR 3.7 /
  PR 4 / PR 5+; see top-of-file amendment banner.)**
- Optimize for 5-year maintenance.
- CAPCO-first: no second `MarkingScheme` until CAPCO is solid. `Vocabulary<S>`,
  `MarkingScheme`, `Codec<S>` ship `#[doc(hidden)] pub` semver-unstable. They
  will change on contact with scheme #2; that is the accepted cost.
**Constitution gates:** I (performance), III (WASM safety), IV (two-layer
rules), V (audit-first / G13), VI (dataflow pipeline), VII (acyclic
dependencies), VIII (citation fidelity).

---

## 0. Why this doc

The issue corpus from late 2026-04 surfaced a generative pattern: a small number
of structural defects in the engine and the `CapcoScheme` adapter were producing
issues on a steady cadence. The same shape kept appearing — a leak between
layers that should not have been able to talk to each other (input bytes into
audit records, parser output into canonical rendering, page-level state into
rule input via the wrong channel).

Three independent reviews triangulated on the same root cause from different
vantage points. The user's decision pass collapsed the substantive
disagreements. A seven-reviewer adversarial murder board stress-tested the
consolidated plan and surfaced six structural weaknesses (G13 closure
shallow, lattice-doc gate gameable, F.1 timing too late, PR 3 monolith too
large, performance benches don't measure the claims, audit-schema accept-list
shape questionable). The user accepted the corrections and chose the
clean-break path on the audit-schema and adapter-removal questions.

This document is the **complete consolidated plan**. It supersedes the prior
2026-05-01 draft (delete after this merges) and the working notes at
`/tmp/review-*.md` and `/tmp/issue-*.md`. There is one plan, not multiple
drafts.

---

## 1. The two underlying problems

The corpus has *one apparent* problem (lots of issues that look unrelated)
and *two underlying* problems. Closing each underlying problem closes a
class of issue, not a single one.

A **third problem class** — recognizer scoring quality — was identified by
the murder board (root-cause-analyst F4) as outside this frame. Issues
#258 (decoder prose null hypothesis) and #260 (decoder folds bare NATO
levels) belong to this third class. They are addressed by PR 8 as
standalone work; **this plan does not claim to close them.**

### 1.1 The pivot type does too many jobs

`IsmAttributes` (`crates/ism/src/attrs.rs`) is simultaneously:

1. **Parser output** — what `marque-core::parser` produces, possibly carrying
   degraded or partial structure on malformed input.
2. **Post-canonical form** — what rules read when they emit fix proposals;
   expected to be in canonical form (uppercase, ordered, deduplicated).
3. **Page roll-up output** — what `PageContext::expected_*` returns after
   aggregating across all portions on a page.

These three roles are confused at the type level, and the confusion is
generative:

- **#257** (R001 leak): decoder writes uppercased input bytes into
  `provenance.canonical_bytes`, which flow through `proposal.replacement`
  and the R001 message. The pivot can't tell parser output (input bytes)
  from canonical form, so the engine can't reject the contamination at
  the boundary. G13 (Constitution V Principle V) enforced by code-path
  comment, not type.
- **#272** / **#273** / **#274** (E003 confidence + ordering):
  E003 reads `attrs.token_spans` from the original parse and cannot see a
  pending E001 `OC → ORCON` localized fix.
- **#276** (FGI banner roll-up): `page_context_to_attrs` at
  `crates/capco/src/scheme.rs:365` hardcodes `MarkingClassification::Us`,
  silently flattening FGI/NATO/JOINT during page projection.
- **#280** (silent open-vocabulary corruption):
  `parse_fgi_marker` (`parser.rs:1011-1024`) returns
  `Some(FgiMarker { countries: [] })` when post-prefix bytes fail
  `CountryCode::try_new`. The pivot can't distinguish "authentic
  source-concealed FGI" (lawful per §H.7 p123) from "we failed to parse the
  trigraphs" because both shapes collide on the same `FgiMarker`.

### 1.2 The Phase-B trait surface is built but not load-bearing

`marque-scheme` shipped `MarkingScheme`, `Lattice`, `BoundedLattice`,
`Constraint`, `Scope`, `PageRewrite`, the built-in lattice constructors,
and the topological scheduler. The engine doesn't drive load-bearing
roll-up through any of it. `CapcoMarking::join` at
`crates/capco/src/scheme.rs:188-247` *currently violates* the lattice laws
by delegating to `PageContext`. The `Scope::Page` projection isn't wired
into `Engine::lint`.

The previous attempt at lattice unification skimmed the complexity and
ended up bandaged until it was almost unused. PR 4 is therefore gated by
PR 3.7 (lattice §-resolution spike) — the lattice design doc must be
filled in with the actual math before any lattice impl lands.

---

## 2. Triple-confirmed convergences

Eight points on which all three original reviewers independently arrived
at the same conclusion. Murder board did not contradict any.

**2.1 #263 is the keystone refactor.** Canonicalizer + renderer split
+ form-routing collapse. 49 → ~10–13 rules. Co-lands in PR 3b. _(Amended 2026-05-07: Stage-1 target 13–18; see top-of-file banner.)_

**2.2 The pivot type does too many jobs** (§1.1).

**2.3 Page-level aggregation should drive through `Scope::Page`
projection** (§1.2). Wire projection in, kill the
`MarkingClassification::Us` hardcode at `scheme.rs:365`, declare the
missing rewrites, retire `PageContext`.

**2.4 Open-vocabulary parser asymmetry (#280) is small and urgent.**
The fix is mechanical (replace `is_ascii_alphanumeric()` with
`Vocabulary<S>::shape_admits` at the four sites; replace silent-skip with
`None` return); the cost of leaving it is silent semantic corruption.
Murder board added: introduce `FgiMarker::SourceConcealed | Acknowledged`
discriminant in PR 2 (W2 / backend-architect #8) so lawful
source-concealed FGI doesn't collide with the now-unreachable corruption
shape.

**2.5 #277 single-pass forward splice goes first. [LANDED in PR #278.]**
`Vec::splice` per fix is O(N·M); single forward walk is linear.
`fix_throughput` Criterion bench wired into `bench-check.sh` (R² ≥ 0.9).

**2.6 Two-pass apply (#272) is the right shape.** Pass 1 (localized
token rewrites) → splice → re-parse → pass 2 (whole-marking rules
including E003). Murder board added: phase-tagged rules at registration,
non-overlap invariant (I-18), reshape-aware pass-2 (I-19), R002
diagnostic for re-parse failure (§9).

**2.7 Decoder leak (#257) needs canonical-bytes validation against
shape-admission.** Murder board added: provenance-tagged `Canonical`
with sealed closed-CVE constructor (§8.1), decoder locked out of
open-vocabulary canonicalization (§8.2). #258 prose null hypothesis is
a third problem class (PR 8, acknowledged not closed).

**2.8 Citation fidelity needs CI enforcement.** Constitution VIII makes
citation accuracy a correctness requirement. Murder board added:
8A scope widened to `message:` and `constraint_label:` strings and
doc-comment `§X.Y` form (not just `citation:` fields); F.1 corpus
skeleton lands at PR 0.5 alongside 8A (not deferred to the end);
preemptive PR 0.6 fixes the four pre-existing citation defects the
murder board surfaced (see PR 0.5 / 0.6 rows in §4).

---

## 3. Resolved disagreements

Six substantive disagreements during the original conferral, plus four
murder-board findings the user resolved. All resolved.

**3.1 Theme 1 (rule/engine boundary): three layers.** `ParsedAttrs<'src>`
/ `CanonicalAttrs` / `ProjectedMarking` (data-flow types) +
`FixReplacement::Strict | Decoder` (audit discriminant) + `FixIntent<S>`
(rule-emission API) ship as one stack. **Murder board override: FixIntent
is no longer deferred.** Three reviewers agreed the deferral triggers are
imminent or already firing (#257's message-channel leak, decoder/strict
provenance erasure). Under clean-break, FixIntent lands in PR 3c — the
rule-API surface is reshaping anyway, doing it now is one diff vs. one
diff against a freshly-touched surface later.

The `ParsedAttrs<'src> → CanonicalAttrs` conversion is an explicit trait
method, **`MarkingScheme::canonicalize(parsed: ParsedAttrs<'_>) -> CanonicalAttrs`**.
PR 3a's `from_parsed_unchecked` adapter is `#[doc(hidden)]` and exists
only during the keystone window (3a → 3c) so existing rule code keeps
compiling against `&CanonicalAttrs`. PR 3c deletes the adapter; rules
post-keystone consume `CanonicalAttrs` produced *only* via
`MarkingScheme::canonicalize`. The trait method is the type-system seal
that prevents rule crates (current `marque-capco`, future `marque-cui`,
etc.) from constructing `CanonicalAttrs` from arbitrary parser output —
canonicalization is a scheme decision, and the scheme owns it.

**3.2 Theme 2 (lattice laws first).** `CapcoMarking::join` currently
violates the laws. PR 4 (lattice-law foundation) lands before PR 5
(`expected_classification()` widening). Cost: #276 partial close delays
one PR. Worth it. **Murder board override: PR 4 is gated by PR 3.7
(lattice §-resolution spike), which fills the lattice design doc with
actual math, including the eight §10 open-question resolutions and
cross-axis dominance fixtures (FOUO eviction, FGI banner roll-up,
SCI cross-system canonicalization).** The previous lattice attempt's
"bandaged until almost unused" failure mode was the gate-as-stub
problem; the spike PR closes it.

**3.3 Theme 4 (vocabulary scope).** `Vocabulary<S>::shape_admits` covers
both closed CVE tokens (admitted by membership) AND structurally-permitted
slots (admitted by generative rules: SAR program ID shape,
trigraph/tetragraph form, SCI compartment grammar). **Murder board
override: open-vocabulary slots get a *narrower* admission via
provenance-tagging on the strict path, and the decoder is locked out of
open-vocabulary canonicalization entirely.** See §8 for the type-system
details. Closed-CVE majority gets compile-enforced sealing; open-vocab
residual is honest about its limits.

**3.4 Theme 7 (dissem position attribution).** PR 9 ships 7B (separate
`dissem_us` / `dissem_nato` parser fields, position-attributed) and
closes #271. **7C (`Vocabulary<S>::TokenId` distinguishing US ORCON from
NATO ORCON despite same surface) is deferred indefinitely** — speculative
until a second `MarkingScheme` exists in-tree, and per the user's
CAPCO-first constraint that won't happen until CAPCO is solid.

**3.5 Theme 8 (citation gates compose).** Three layers, three failure
modes. **Murder board override: scope and timing both shift forward.**
- 8A (citation-string lint) catches *fabrication* mechanically — at PR
  time, every `§X.Y pNN` reference parses and exists in the vendored
  source. **Scope widened**: the lint inspects `citation:` struct fields,
  `message:` strings, `constraint_label:` strings, and doc-comment
  `§X.Y` references. The HCS-P fabrication-style at `scheme.rs:1787` and
  `:1883` (the literal `§4` is fabricated; CAPCO has only §A–H normative)
  passes today's narrower 8A; the widened lint catches it.
- F.1 (corpus fixture per cited authority) catches *predicate-vs-example
  drift*. **Lands at PR 0.5 with sparse fixtures, matures to full coverage
  at PR 10.** Running F.1 against the existing catalog at PR 0.5 is the
  scope-discovery exercise; whatever F.1 surfaces (HCS-P-shape drift,
  `p150–151 p151` doubling, cross-revision SIGMA archaeology) gets fixed
  in PR 0.6 before the keystone begins.
- 8C (vendored-source registry) — declarative-only at PR 10. Lower
  priority given CAPCO-first; not load-bearing until scheme #2.

8A → standalone PR 0.5. F.1 skeleton → PR 0.5. F.1 maturation → PR 10.
8B (passage extraction) demoted to nice-to-have.

**3.6 Theme 5 (sequencing on `shape_admits`): measurement-gated.** PR 2
lands `Vocabulary<S>::shape_admits` AND an SC-001 latency probe
simultaneously. Threshold: >5% mean regression OR >5% p99 regression →
back out to E.1 (case-strict at the four cited parser sites only —
`parser.rs:1011-1024`, `1453`, `1481`, `1493` — no extraction).
**Murder board override: p99 tail-percentile assertion added** because
mean-on-mean cannot detect a per-token vtable miss that adds bounded
microseconds in the worst case but averages near zero. `Arc<dyn
Vocabulary<S>>` already precludes cross-crate devirtualization; the bench
must measure tail latency, not just average.

**3.7 Murder board: PR 3 monolith too large.** Resolved by splitting into
three independent PRs: 3a (pivot split + adapter), 3b (#263 rule
collapse), 3c (`FixReplacement` discriminant + `Canonical` sealing +
adapter delete + FixIntent + rule-ID retirement + audit cutover). Each
PR has independent revert. See §4.

**3.8 Murder board: audit-schema accept-list.** Resolved by clean break.
No accept-list. `MARQUE_AUDIT_SCHEMA` validates against a single value at
build. Old records are not readable; there are no old records to read.
The `mvp-N` naming retires. Post-keystone schema is `marque-1.0`. See §10.

**3.9 Murder board: pass-1 fix reshapes pass-2 input.** Resolved by
phase-tagged rules + I-18 (non-overlap) + I-19 (reshape-aware
whole-marking) + R002 diagnostic. See §9.

**3.10 Murder board: trait surface hardening risk.** Resolved by
accepting the cost. `Vocabulary<S>`, `MarkingScheme`, `Codec<S>` ship
`#[doc(hidden)] pub` semver-unstable. They will change on contact with
scheme #2; that is the accepted cost of CAPCO-first.

---

## 4. PR sequence

Single ordered sequence. Each PR independently shippable; each maps to
specific issues; each carries a Constitution Check (Appendix D). Order
respects WASM-safety (Principle III) and the acyclic dependency graph
(Principle VII).

| PR | Description | Closes | Constitution check |
|----|-------------|--------|--------------------|
| 0 | `static_assertions` on rule + recognizer trait bounds (`Rule: Send + Sync`, `Recognizer<S>: Send + Sync`); masking-pin lint at `tools/masking-pin-lint/` (**AST-based**, not regex); **promote-callsite lint at `tools/promote-callsite-lint/`** (also AST-based — enforces I-15: `AppliedFix::__engine_promote` and `EnginePromotionToken::__engine_construct` only callable from `Engine::fix_inner` in production code, with `#[cfg(test)]`/`tests/` carve-out per Constitution V Principle V) | (preemptive) | V, VI |
| 0.5 | Citation-string lint (8A) at `tools/citation-lint/` — **scope: `citation:` fields + `message:` strings + `constraint_label:` strings + doc-comment `§X.Y` references**. Lint rejects bare `§NN` (no subsection), out-of-normative-range sections (CAPCO normative is §A–H only), pages outside the vendored source's range, **AND legacy `line NNNN` citation forms** (the project retired `line NNNN` citations in commit b340bec — page numbers only). F.1 corpus-fidelity skeleton with one canonical example per existing rule. F.1 runs against existing catalog as discovery exercise; catalogues failures for PR 0.6. | (preemptive) | VIII |
| 0.6 | Preemptive citation-defect fix. Closes the four murder-board findings: (a) `§4` fabrication at `scheme.rs` lines 1734/1783/1787/1796/1814/1822/1830/1841/1850/1883 and similar — corrected target is **`§H.4`** for HCS / HCS-O / HCS-P sites (per CAPCO-2016 §H.4 pp 62–66); (b) doubled `p150–151 p151` at five sites in `rules.rs` lines 2022, 2148, 2609, 2919, 10142; (c) cross-revision SIGMA archaeology at `rules.rs:4053`; (d) HCS-P predicate at `scheme.rs:1839-1849` is **two-sided** per CAPCO-2016 §H.4 p66 — over-strict on optional `ORCON`/`ORCON-USGOV` ("may be used") AND under-strict on the missing `NOFORN` requirement ("requires NOFORN"); both sides MUST be corrected together. Plus whatever else PR 0.5's F.1 run catches. **Implementer re-greps line numbers at PR 0.6 time — file edits since the murder board may have shifted offsets; defect classes are stable, line numbers are not.** Constitution VIII satisfied across the catalog before refactor begins. | (preemptive) | VIII |
| 1 | Single-pass forward splice; `fix_throughput` Criterion bench wired into `bench-check.sh` (R² ≥ 0.9) | #277 | I, VI |
| 2 | `Vocabulary<S>::shape_admits` + parser case-strict (measurement-gated; **p99 tail-percentile assertion** added to >5% threshold); FGI silent-skip → `None`; **`FgiMarker::SourceConcealed \| Acknowledged { countries }` discriminant introduced**; rules using `countries.is_empty()` audited and migrated; `is_ascii_alphanumeric()` → `shape_admits` at the four parser sites | #280 | I, III, IV, VIII |
| 3a | **Keystone-1**: pivot split (`ParsedAttrs<'src>`/`CanonicalAttrs`/`ProjectedMarking`) + `from_parsed_unchecked` transitional adapter (`#[doc(hidden)]`). All rules consume `&CanonicalAttrs` via the adapter. No rule collapse, no discriminant change, no schema bump. Independently revertable. | (structural prerequisite) | III, V, VI, VII |
| 3b | **Keystone-2**: #263 rule collapse — Stage-1 target 13–18 (amended 2026-05-07) using the pivot from 3a. Touches `crates/capco/src/rules.rs` + `crates/capco/src/rules_declarative.rs` + `crates/capco/src/rules_sci_per_system.rs` + `crates/capco/src/scheme.rs` rule-set construction (T026e collapses `rules_sci_per_system.rs` to a `Constraint::Custom` walker; T026b/c/d add declarative `PageRewrite` / `Conflicts` / `Requires` rows on `CapcoScheme`). No schema bump. Independently revertable. Six sub-moves T026a–T026f per `tasks.md`; see top-of-file amendment banner. | #263 | IV, VI |
| 3c | **Keystone-3**: `FixReplacement::Strict \| Decoder` discriminant + provenance-tagged `Canonical` with sealed closed-CVE constructor (G-Option 3, §8.1) + decoder locked out of open-vocabulary canonicalization (K-Option 2, §8.2) + `engine.rs::build_decoder_diagnostic` carve-out delete (the `proposal.original = ""` branch around the `FixProposal::new(..., "", replacement, ...)` call — currently `engine.rs:1369-1384` but **implementer re-greps at PR 3c time** since this anchor has already shifted once and the function body is in active flux) + `from_parsed_unchecked` adapter delete + **`FixIntent<S>` rule-API surface lands** + **rule-ID retirement to `(scheme, predicate-id)` keys** + audit schema cutover (single bump `marque-mvp-2 → marque-1.0`, no accept-list, see §10). Independently revertable. | #257, #267 Gap A, #267 Gap B (fix-emission becomes mechanical via `render_canonical`) | III, V (G13 → type invariant), VI |
| 3.7 | **Lattice §-resolution spike**. Fill `2026-05-01-lattice-design.md` §§2–8 with §-citations, formal join semantics, worked examples, property fixtures. Resolve all eight §10 open items; **no "explicitly deferred to a tracked issue" escape valve**. Patch §3 Q3 (`noforn-clears-rel-to` is already a declared `PageRewrite` per CLAUDE.md "Phase B"; reframe as confirm-and-document). Add cross-axis dominance fixtures to §9 (FOUO eviction, FGI banner roll-up #276, SCI cross-system canonicalization). Named owner + deadline before merge. | (gate for PR 4) | VI, VIII |
| 4 | Lattice-law foundation: per-category `Lattice` impls + property tests (now including cross-axis fixtures from PR 3.7). **`CapcoMarking::join`'s `PageContext` delegation deleted with no equivalence shim** (clean break). | (regression gate) | VI |
| 5 | Widen `expected_classification()` → `Option<MarkingClassification>`; kill `MarkingClassification::Us` hardcode at `scheme.rs:365`; render-canonical drops redundant `FGI` token when trigraph present (#261 falls out) | #276 (partial), #261 | VI, VIII |
| 6 | Drive `scheme.project(Scope::Page, ...)` from `Engine::lint`. **`PageContext` deleted at PR 6 merge** (was PR 10, collapsed here under clean break). PR 6 is structured as a three-commit sub-sequence: **commit 6a** wires `Scope::Page` projection behind a feature flag with `PageContext` still default; **commit 6b** runs `lint_100kb_multipage` Criterion bench against both paths and asserts projection ≤ baseline + 10%; **commit 6c** flips default to projection and deletes `PageContext`. The bench thus measures both during 6b and projection-only post-merge. | (cutover) | I, VI |
| 7 | **Phase-tagged pass split**: rules declare `Phase::Localized \| WholeMarking` at registration (rules needing both phases register twice — see §9.1). Engine enforces I-18 (non-overlap), I-19 (reshape-aware whole-marking). **R002 diagnostic** for re-parse-failure (pass-1 fixes ship + R002 emits + pass-2 doesn't run; document state coherent). Computed E003 confidence with `FeatureId::PrecedingFixPenalty`; suggested-reorder in E003 message. **`fix_10kb` Criterion bench gates this PR**. Audit schema unchanged from PR 3c (`marque-1.0` already covers `FeatureId::PrecedingFixPenalty`). | #272, #273, #274 | I, V, VI |
| 8 | Decoder prose null hypothesis priors (`marque-priors-3` schema bump); fold bare `NATO {level}` to canonical NATO marking. **Note: third problem class (recognizer scoring quality) — outside the two underlying problems frame; this plan does not claim closure of #258/#260, only delivery of priors and folding logic.** | #258, #260 | III |
| 9 | Parser separator spans (#106); 7B `dissem_us` / `dissem_nato` position-attributed fields; banner-validation rules migrate to `&ProjectedMarking`; declare missing `PageRewrite`s; ATOMAL/BOHEMIA recognition; NATO-portion-in-US-doc → REL TO USA, NATO derivation as declarative `Constraint` | #106, #270, #271, #265, #246, #264, #251 | IV, VI, VIII |
| 10 | F.1 corpus gate maturation (full per-cited-authority coverage; was PR 11). 8C vendored-source registry declared (declarative-only). | #267 Gap C if not closed by 0.6 | VIII |

**PRs folded under clean break:**
- Original PR 10 (retire PageContext) → folded into PR 6.
- Original PR 11 (F.1 maturation) → renumbered to PR 10 after the fold.
- Original PR 11.5 (rule-ID retirement) → folded into PR 3c.
- Original PR 3.5 (gate removal) → folded into PR 3c.

**PR 12+ priority** (deferred until later phases, per user direction):

1. **#261** — drop redundant `FGI` when trigraph present *(falls out of PR 5)*.
2. **#265** — NATO portion in US doc requires REL TO USA, NATO *(declarative `Constraint`; lands in PR 9)*.
3. **#267 Gap A + Gap B** — companion-insert + E038 fix-emission *(mechanical once `Canonical` exists; lands in PR 3c via FixIntent)*.
4. **#260** — decoder folds bare NATO {level} *(lands in PR 8)*.
5. **#246** — ATOMAL/BOHEMIA + US dissems in non-US markings *(lands in PR 9)*.

Several "fall out" of structural changes earlier in the sequence; each
is placed where its underlying mechanism lands, rather than queued as a
separate PR.

### Audit-schema cutover

Under clean break, **one cutover, no accept-list**:

| PR | Schema | Trigger | Compat |
|----|--------|---------|--------|
| 3c | `marque-mvp-2` → `marque-1.0` | `FixReplacement::Strict\|Decoder` discriminant + `Canonical` sealed constructor + `FixIntent` audit fields + `(scheme, predicate-id)` rule-ID form + `FeatureId::PrecedingFixPenalty` reserved | None. Pre-cutover records unreadable by the post-cutover binary. No reader crate. There are no records. |

PR 7 does NOT bump the schema — `FeatureId::PrecedingFixPenalty` is
reserved in `marque-1.0` at PR 3c so that PR 7 ships data into a slot that
already exists.

PR 8's `marque-priors-3` is a *priors-bake* schema, not the audit schema.
Bumped independently per Phase D conventions.

---

## 5. Invariants register

Nineteen invariants the post-migration engine must satisfy. Each carries
an enforcement mechanism and a regression catch. The register is the
acceptance criterion for "we're done" — at the end of PR 10 every
invariant holds; until then, masking-pin discipline (I-16) tracks gaps.

| # | Invariant | Enforcement | Regression catch |
|---|-----------|-------------|------------------|
| I-1 | Every byte in `FixProposal::replacement` came from `MarkingScheme::render_canonical(token, scope)` against a `Vocabulary<S>::shape_admits`-passing canonical | **Type system**: `Canonical`'s sealed closed-CVE constructor (`Canonical::from_cve(TokenId)`) is the only public path for closed-vocabulary tokens. Open-vocab path goes through `render_canonical` with provenance tag (§8.1). Decoder cannot construct an open-vocab `Canonical` (§8.2). | Property test: every emitted `FixProposal::replacement` decomposes back into `(TokenId, Scope)` for closed-vocab; open-vocab decomposes into `(Category, RenderCallSite)`. Compile-fail tests demonstrating `Box<str> → Canonical` paths don't exist. |
| I-2 | `FixProposal.original` and `Diagnostic.message` carry no document content bytes — only category IDs, span offsets, BLAKE3 digests, posterior scalars, enumerated `FeatureId` labels | **Type system**: `Diagnostic::message` constructor takes `MessageTemplate` (an enum of stable strings) + `MessageArgs` (a closed set of permitted scalar/ID types). `FixProposal::original` becomes `Span` only — caller resolves bytes if needed; audit emitter resolves to BLAKE3. | `core_error_isolation.rs`; corpus canary scan for verbatim input. The `engine.rs:1389` `format!("decoder-recognized canonical form: {replacement:?}")` interpolation deletes (becomes `MessageTemplate::DecoderRecognized { token: TokenId }`). |
| I-3 | `kept_fixes` non-overlapping in span order regardless of iteration direction | C-1 overlap guard (existing) | Property: shuffle, splice ascending vs. descending, byte-identical |
| I-4 | Pass 2 reads only post-pass-1 buffer + `&CanonicalAttrs<'src>` (re-parsed) | Engine re-parses between passes (PR 7) | Property: pass-1 token change feeds pass-2 rule input |
| I-5 | `Vec<AppliedFix>` monotonically appended; never reordered post-promotion | `Engine::fix_inner` (existing) | Snapshot test at `crates/engine/tests/audit_sequence_snapshot.rs` (PR 3c): apply a fixed input through `Engine::fix_inner`, snapshot the emitted `Vec<AppliedFix>` order, assert byte-identical across re-runs |
| I-6 | `Confidence::combined()` is the only threshold-comparison operator | `engine.rs:930` filter (existing) | Mutation test at `crates/engine/tests/confidence_threshold_mutation.rs` (PR 3c): a `cfg(test)`-gated build-flag swaps `Confidence::combined()` for `Confidence::recognition()` in the engine filter, asserts the mangled-corpus accuracy gate (SC-004) regresses below baseline |
| I-7 | Decoder candidates always include the prose null hypothesis when one applies | `decoder.rs::recognize` (PR 8) | SC-003a precision gate vs. `tests/corpus/prose/article.txt` |
| I-8 | Open-vocabulary identifier shape checks route through `Vocabulary<S>::shape_admits`; no inline `is_ascii_*` for category-typed tokens in `marque-core/parser.rs` | Refactor PR 2; CI grep flags drift | Per-fixture: `(TS//SAR-fk)`, `(TS//FGI deu)`; CI grep |
| I-9 | `parse_fgi_marker` returns `None` (not `Some` with degraded structure) when post-prefix bytes fail `shape_admits`; `FgiMarker` discriminates `SourceConcealed` from `Acknowledged { countries }` | PR 2; silent-skip path at `parser.rs:1011-1024` deleted; discriminant introduced | `tests/parser/fgi_silent_skip_guard.rs`; rule audit confirms no `countries.is_empty()` pattern matches in `marque-capco` |
| I-10 | Every `Constraint`/`PageRewrite`/`Rule` cited authority has ≥1 corpus fixture exercising the predicate against the canonical example | `crates/capco/tests/citation_fidelity.rs` (PR 0.5 skeleton, PR 10 maturation — F.1) | The CI gate; HCS-P would have failed it |
| I-11 | Every `Rule` and `Recognizer<S>` impl is `Send + Sync` | `static_assertions::assert_impl_all!` from `RuleSet::new()` (PR 0) | Compile-fail at construction |
| I-12 | `Scope::Page` projection is the source of truth for banner-validation rule input; no `PageContext`-only paths exist. **Depends on I-17 holding** (lattice impls satisfying laws are the substrate the projection runs on) | Lattice foundation (PR 4); `scheme.project(Scope::Page, ...)` enabled in PR 6; `PageContext` deleted in PR 6 (clean break, no equivalence shim window) | `tests/corpus/lattice/` regression suite (cross-axis fixtures from PR 3.7); multi-page `lint_100kb_multipage` bench |
| I-13 | `MarkingClassification::Us` never hardcoded in any projection function; `expected_classification()` returns `Option<MarkingClassification>` | Removal of `scheme.rs:365` (PR 5) | `tests/corpus/foreign/pure_foreign_banner.json` (#276 reproduction) |
| I-14 | `MARQUE_AUDIT_SCHEMA` build-time-pinned; one schema per binary; **no accept-list — single value validation** (FR-014, clean break) | Build-time validation (existing, tightened) | `audit_schema_consistency.rs` |
| I-15 | `AppliedFix::__engine_promote` and `EnginePromotionToken::__engine_construct` called only from `Engine::fix_inner` in production code (test-fixture carve-out per Constitution V Principle V; carve-out enumerated for both constructors) | Convention; type-level seal via `EnginePromotionToken`'s private field (existing); `from_parsed_unchecked` adapter does NOT exist post-PR-3c | **AST-based** CI lint at `tools/promote-callsite-lint/`; `#[cfg(test)]`/`tests/` carve-out enumerated per call site |
| I-16 | Every `with_recognizer(StrictRecognizer)` test pin carries `// MASKING-PIN: tracks #NNN` (with `#NNN` open) or `// INTENTIONAL-STRICT: <reason>`; masking pins removed in the issue-closing PR; **GitHub-API closure check is mandatory, not optional, and follows `closed_as_duplicate_of` chains** | `tools/masking-pin-lint/` (AST-based, not regex). Backfill: 2 masking pins (`core_error_isolation.rs` → #257 closes at 3c; `corpus_accuracy.rs` → #258 closes at PR 8), 5 intentional pins | The lint; both masking pins die after PR 3c + PR 8 |
| I-17 | Every category in `CapcoScheme::categories()` has a `Lattice` impl satisfying assoc/comm/idem/identity-with-bottom AND cross-axis dominance (FOUO eviction, FGI banner roll-up, SCI cross-system canonicalization) | Property tests in `crates/capco/tests/category_lattice_laws.rs` + cross-axis fixtures in `crates/capco/tests/cross_axis_dominance.rs` (PR 4); contents of `2026-05-01-lattice-design.md` after PR 3.7 fill-in | Property tests + cross-axis corpus regression |
| **I-18** | **For any pass-1 `AppliedFix` with span S₁ and any pass-2 `AppliedFix` with span S₂, S₁ ∩ S₂ = ∅** (pass-2 fixes overlapping pass-1 spans demote to suggestions, not auto-applied) | Engine pass-2 dispatch filters by overlap; `Phase::Localized \| WholeMarking` declared at rule registration (one phase per rule; defect classes needing both register two entries) | Property test in `crates/engine/tests/two_pass_invariants.rs` shuffling fix orderings |
| **I-19** | **`Phase::WholeMarking` rules whose span overlaps a pass-1 fix re-validate against pre-pass-1 attrs (cached from pass-0) before firing** (avoids retroactive-satisfaction false positives where pass-1 reshape coincidentally matches a pass-2 predicate) | `RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>` for `Phase::WholeMarking` rules; `None` for `Phase::Localized`; engine populates from pass-0 cache | Property test + reshape-targeted fixtures (E001 `OC → ORCON` followed by E003 ordering check) |

### Decoder/strict drift as a type-system gap

The R001 leak (#257) and saturation channel (#258) are the same defect
from two angles: `StrictRecognizer` and `DecoderRecognizer` produce
identical `Parsed<S::Marking>` shape but operate under different
correctness properties. Strict-path invariants hold by construction;
decoder-path invariants today hold by carve-out
(`provenance.canonical_bytes` may include uppercased unrecognized input
segments; `recognition_score()` saturates at `0.999999` for solo
candidates regardless of evidence). The `proposal.original = ""` carve-out
at `engine.rs::build_decoder_diagnostic` (the `proposal.original = ""` branch, currently `engine.rs:1369-1384` — re-grep at edit time) is the tell — invariants enforced by
comment-propagation across code paths are the failure mode that produces
this class of bug.

After PR 3c: a decoder canonical carrying unrecognized bytes is
**unconstructable** as a `Canonical` (§8.2 — decoder is locked out of
open-vocab canonicalization; closed-CVE construction is sealed). The
decoder's contract becomes "produce `Parsed::Unambiguous` or
`Parsed::Ambiguous` over closed-CVE shapes only; refer open-vocab to
diagnostic-only output." #258 closes in PR 8 independently as recognizer
scoring work, not as G13 closure work. The carve-out at
`engine.rs::build_decoder_diagnostic` (the `proposal.original = ""` branch, currently `engine.rs:1369-1384` — re-grep at edit time) is deleted in PR 3c.

---

## 6. Test strategy

Five-layer property-test architecture, strengthened post-murder-board.

**Layer 1 — Lattice law tests per category** (gates I-17; PR 4). For
every category in `CapcoScheme::categories()`:
- assoc/comm/idem/identity-with-bottom on `Lattice::join` (in-category)
- **cross-axis dominance fixtures** (FOUO eviction by `MarkingClassification > U` AND by non-FD&R dissem; FGI banner roll-up #276; SCI cross-system canonicalization)

Lives at `crates/capco/tests/category_lattice_laws.rs` and
`crates/capco/tests/cross_axis_dominance.rs`. Cross-axis fixtures are
**load-bearing**: in-category laws alone cannot detect a `SupersessionSet`
that always returns the larger set (set union is associative even when
supersession is broken) and cannot detect FOUO surviving a join it
shouldn't have. The fixture set comes from PR 3.7's lattice-doc fill-in.

**Layer 2 — Parse–render round-trip** (PR 2).
`prop_assert_eq!(parse(bytes), parse(render(parse(bytes))))` with explicit
equivalence relation: structural equality on `Parsed` ignoring
`provenance.source_bytes` and confidence floats, with an allowlist of
canonicalizations (whitespace collapse, casing, ordering). Lives at
`crates/capco/tests/parse_render_roundtrip.rs`. Catches silent semantic
degradation (#280's `(TS//FGI deu) → FgiMarker { countries: [] }`).
Strict-path only — decoder paths are not byte-stable and live in
`Parsed::Ambiguous`-shaped tests.

**Layer 3 — Per-pass fix invariants** (gates I-1, I-2, I-4, I-18, I-19;
PRs 3c + 7). At `crates/engine/tests/fix_invariants.rs`:

1. `applied_fix.span ⊆ marking_span`.
2. Every byte in `replacement` decomposes back to `(TokenId, Scope)` for closed-vocab; `(Category, RenderCallSite)` for open-vocab. Unconstructable paths fail to compile.
3. Pass-2 fixes consume the post-pass-1 buffer (re-parse between passes).
4. **I-18 non-overlap**: shuffle pass-1 / pass-2 fix orderings, assert no overlap in promoted output.
5. **I-19 reshape-aware**: pass-1 reshape (e.g., `OC → ORCON`) feeds pass-2; rule re-validates against pre-pass-1 attrs.
6. Audit-record canary: **deterministic** scan of NDJSON output asserts no input bytes appear except inside span-offset identifiers. Replaces `core_error_isolation.rs`'s masking pin once PR 3c lands. The deterministic scan reads `Engine::fix_inner`'s emitted `Vec<AppliedFix>` only; test-fabricated records under Constitution V's carve-out are explicitly excluded.

**Layer 4 — Corpus regression sweeps** (PR 4 onward). **Five corpora** ×
two recognizers = ten CI runs:

| Corpus | Target | Catches |
|---|---|---|
| `tests/corpus/valid/` | zero auto-applied fixes (info/suggest permitted) | rule-layer false positives |
| `tests/corpus/mangled/` | ≥0.85 fix accuracy (SC-004) | recall regressions |
| `tests/corpus/prose/` (Gutenberg + Federalist + Wikipedia) | zero diagnostics | decoder null-hypothesis regressions on pure prose |
| **`tests/corpus/prose-positive/`** | true-positive markings in prose context MUST fire diagnostics | recall on prose-shaped CAPCO mentions; closes the "zero diagnostics oracle is too strong" failure mode |
| **`tests/corpus/lattice/`** | hand-crafted page-context fixtures (#276 reproduction, FOUO eviction, FGI banner roll-up, SCI compartment union) | lattice-projection regressions Layer 1 cannot catch via property alone |

Prose corpus verifies PR 8 and lifts `corpus_accuracy.rs`'s masking pin.
`prose-positive` and `lattice` corpora are the murder-board additions —
the original 3-corpus oracle was insufficient.

**Layer 5 — Citation lint** (PR 0.5 skeleton + PR 10 maturation).
`tools/citation-lint/` parses every `citation:` field, **`message:` string,
`constraint_label:` string, and doc-comment `§X.Y` reference**; asserts
§X.Y exists in `crates/capco/docs/CAPCO-2016.md`; page falls within
markdown offsets; §X.Y in normative range §A–H; rejects bare `§NN`;
**rejects legacy `line NNNN` citation forms** (retired in commit b340bec
— citations carry page numbers, not line numbers, because line numbers
in the vendored markdown drift on every edit). F.1 corpus fixture per
cited authority lands at PR 0.5 with sparse fixtures (one canonical
example per existing rule), matures to full coverage at PR 10. **PR 0.5
runs F.1 against the existing catalog as discovery** — failures
(including any surviving `line NNNN` forms) get fixed in PR 0.6.

### Bench coverage

Murder-board F gap closure:

| Bench | Gates | Lands |
|-------|-------|-------|
| `fix_throughput` (R² ≥ 0.9) | PR 1 splice rewrite | Already landed in PR #278 |
| **SC-001 with p99 tail-percentile assertion** | PR 2 `shape_admits` indirect-call | PR 2 |
| **`fix_10kb`** | PR 7 two-pass re-parse cost | PR 7 |
| **`lint_100kb_multipage`** | PR 6 `Scope::Page` projection cutover | PR 6 |

Each bench is gated in `bench-check.sh` against the relevant baseline.
The §3.6 measurement-gating discipline (>5% mean OR p99 regression
backs out the change) applies uniformly.

### Masking-pin discipline (I-16)

Verified `with_recognizer(StrictRecognizer)` inventory:

| Site | Category |
|---|---|
| `corpus_accuracy.rs:49` | **MASKING-PIN — tracks #258** (closes at PR 8) |
| `core_error_isolation.rs:92` | **MASKING-PIN — tracks #257** (closes at PR 3c) |
| `decoder_dispatch.rs:29`, `audit.rs:120`, `decoder_diagnostic.rs:58`, `decoder_accuracy.rs:451 / :758` | INTENTIONAL-STRICT (5 sites) |

Two masking pins is the pragmatic ceiling. Lint rules (CI-enforced at
`tools/masking-pin-lint/`, **AST-based**):

1. Every masking pin: `// MASKING-PIN: tracks #NNN — remove when issue closes.` within 5 lines.
2. Every intentional pin: `// INTENTIONAL-STRICT: <reason>` within 5 lines.
3. Unmarked pins fail CI.
4. **Mandatory** GitHub-API check: tracked issue is open; follows `closed_as_duplicate_of` chains until it hits a final close. Cascade-close-via-meta-issue is flagged at lint time.
5. **Closure protocol**: when an issue closes, the pin is removed in the same PR; the pin-removal PR includes a regression test that demonstrates fix necessity (must fail on pre-fix HEAD).
6. A third masking pin requires a team-review approval comment.

I-16 lands at PR 0 alongside `static_assertions`.

### PR 3a/3b/3c fixture continuity

Several hundred `IsmAttributes { ... }` literals across the test corpus
reshape across the keystone subsequence:

- **PR 3a**: pivot split lands. Existing literals migrate to
  `CanonicalAttrs::from_parsed_unchecked(...)` via the transitional
  adapter (`#[doc(hidden)]`). Touches every fixture but the migration is
  mechanical (sed-replaceable). Three revert points: revert PR 3a / no-op.
- **PR 3b**: rule collapse — Stage-1 target 13–18 (amended 2026-05-07; see top-of-file banner). Touches rule registration; some
  fixtures consolidate as their rules consolidate. Independently
  revertable.
- **PR 3c**: adapter delete + FixIntent + rule-ID retire + schema cutover.
  Fixtures that still call `from_parsed_unchecked` migrate to consuming
  `CanonicalAttrs` produced via the explicit
  `MarkingScheme::canonicalize(parsed)` trait path (§3.1) — and via the
  `FixIntent` rule API where applicable. **Adapter deletes in this PR**
  — no removal-PR scheduled because there is none needed; clean break.
  Independently revertable.

A CI matrix during the keystone window: corpus regression × {3a-only,
3a+3b, 3a+3b+3c} = 3 runs to verify each subsequence is independently
correct.

---

## 7. Surviving dissent

Two original dissents resolved by user decision:

- **Rule IDs** (`E###`/`W###`/`S###`/`C###` retirement): **resolved** in
  favor of retirement at PR 3c (was PR 11.5; folded under clean break).
- **Designing for second `MarkingScheme`**: **resolved** in favor of
  CAPCO-first. 7C deferred indefinitely; 8C declarative-only at PR 10.
  `Vocabulary<S>`, `MarkingScheme`, `Codec<S>` ship `#[doc(hidden)] pub`
  semver-unstable; **they will change on contact with scheme #2 and that
  is the accepted cost.**

One dissent survives:

**Decoder needs more independent test surface.** Two existing test pins
(`core_error_isolation.rs`, `corpus_accuracy.rs`) mask decoder defects;
both close in PRs 3c + 8. The meta-pattern — a CI test pinned to mask a
known defect — should not be normalized. Masking-pin discipline in §6 is
binding, not aspirational.

### Murder-board overrides incorporated

- **FixIntent no longer deferred** — lands in PR 3c (§3.1). Three
  reviewers agreed deferral triggers were imminent; clean-break makes
  landing now cost-equivalent to landing later.
- **Audit-schema accept-list eliminated** — single-value validation,
  clean break (§10).
- **PR 3 split into 3a/3b/3c** — independent revert (§4).
- **PR 0.5 F.1 skeleton + PR 0.6 preemptive citation fix** — closes the
  HCS-P precedent exposure window (§2.8; PR 0.5 / 0.6 rows in §4).
- **PR 3.7 lattice §-resolution spike** — closes the gate-as-stub
  failure mode (§4, §11).
- **Bench gap closure** — `fix_10kb` + multi-page projection + p99 tail
  (§6).
- **`Canonical` provenance + decoder open-vocab lockout** — G13 becomes
  a type invariant for closed-vocab; honest about open-vocab residual
  (§8).
- **Pass-split phase tags + I-18/I-19 + R002** — closes the pass-1
  reshape failure modes (§9).
- **`FgiMarker::SourceConcealed | Acknowledged`** — closes the
  shape-collision failure mode in PR 2 (§2.4, §5 I-9).
- **AST-based lints** — masking-pin lint and citation-lint use AST
  detection, not regex (§6).

---

## 8. Type system: `Canonical` provenance + decoder open-vocab lockout

The murder board's load-bearing finding (W1) was that the original plan's
"G13 becomes a type invariant" claim leaked at three points: the
`format!` message channel, the `from_parsed_unchecked` adapter, and the
open-vocabulary slot where `shape_admits` admits all grammar-valid bytes
regardless of provenance. This section specifies the type-system
mechanism that closes all three.

### 8.1 `Canonical` is provenance-tagged with sealed closed-CVE constructor

```rust
pub struct Canonical<S: MarkingScheme> {
    bytes: Box<str>,
    source: TokenSource,
}

enum TokenSource {
    Cve(TokenId),
    OpenVocab { category: CategoryId, render_call_site: &'static Location<'static> },
}

impl<S: MarkingScheme> Canonical<S> {
    /// Closed-CVE: only constructor in module scope.
    /// Type narrows: the caller must hold a TokenId, which can only
    /// come from Vocabulary<S>::lookup. There is no Box<str> → Canonical path.
    pub fn from_cve(token: TokenId, scope: Scope) -> Self { ... }

    /// Open-vocabulary: private to render_canonical implementations.
    /// Records the call site as provenance.
    pub(crate) fn from_render(
        category: CategoryId,
        bytes: Box<str>,
        scope: Scope,
        site: &'static Location<'static>,
    ) -> Self { ... }
}
```

**Closed-CVE majority (most rules)** gets compile-enforced sealing.
Rules cannot construct `Canonical` from `Box<str>` because the
constructor accepts `TokenId`, not bytes. `format!`/`concat!`/byte-level
construction paths fail to compile.

**Open-vocabulary residual** (SCI sub-comps, SAR program IDs, country
trigraphs in some contexts) carries `OpenVocab { render_call_site }`.
Audit consumer can distinguish CVE-typed canonicals (high trust) from
open-vocab canonicals (trust-on-render-site). This is the same shape
Constitution V Principle V's "enumerated `FeatureId` labels" set already
uses for confidence; we extend the pattern.

The provenance tag **lands the cheap part of `FixIntent` now** (the
type-level closure of the leak channel) and **defers nothing**. PR 3c
ships the rule-API surface for `FixIntent<S>` — rules emit
`FixIntent<S>` values; the engine renders them through
`MarkingScheme::render_canonical` to produce `Canonical<S>` with correct
provenance.

**Cross-crate rule emission.** Rule crates other than `marque-capco`
(e.g., a future `marque-cui`, a partner-national scheme adapter) need to
emit fixes too. The sealed-constructor design supports this via a
sealed-trait pattern:

```rust
mod private {
    pub trait Sealed {}
}

pub trait CanonicalConstructor<S: MarkingScheme>: private::Sealed {
    fn build_open_vocab(category: CategoryId, bytes: Box<str>, scope: Scope) -> Canonical<S>;
}

// Implementations of `Sealed` are crate-private to the engine.
// External rule crates emit FixIntent<S>; the engine, holding a
// CanonicalConstructor<S> impl, renders to Canonical<S> on the rule's behalf.
```

External rule crates **never construct `Canonical<S>` directly**. They
emit `FixIntent<S>` (the rule-API surface; PR 3c). The engine holds the
sealed `CanonicalConstructor<S>` and is the only path that can call
`build_open_vocab`. This preserves the closed-construction property
across the workspace boundary that Constitution VII opens up for new
rule crate families (`marque-cui`, etc.) without requiring rule crates
to depend on engine internals.

### 8.2 Decoder is locked out of open-vocabulary canonicalization

Decoder-recognized shapes for **closed-CVE tokens** produce
`Canonical<S>` via `Canonical::from_cve(TokenId, ...)`. The TokenId comes
from the same `Vocabulary<S>::lookup` that strict path uses; no path
allows decoder-canonicalized open-vocab bytes through `Canonical`.

Decoder-recognized shapes for **open-vocabulary tokens** (unknown SCI
sub-comp, novel SAR program ID, etc.) produce `Parsed::Ambiguous` with
*diagnostic-only* output. No `FixProposal` is emitted; the engine
surfaces the diagnostic without an auto-apply candidate.

This is the **K-Option 2** choice: decoder loses no real-world capability
(it never had reliable open-vocab canonicalization — that's the bug that
produced #257), strict-path keeps its structural-only canonical form
(per CLAUDE.md "SCI Compartments" — `parse_sci_block` is structural,
not vocabulary-bound), and the closure becomes structural rather than
tag-based.

Trade-off, made explicit: a strict-fail input `(TS//SI-G xyzz)` where
`xyzz` is a legitimate but agency-private sub-comp gets a diagnostic
instead of a fix proposal. That is correct — auto-fixing what we cannot
validate is the bug, not a feature.

**SC-004 baseline note**: the existing SC-004 mangled-corpus accuracy
gate (≥0.85 fix accuracy) was measured with the decoder allowed to
produce open-vocab fixes. Locking the decoder out reduces fix recall on
inputs whose mangled tokens were previously decoder-canonicalized.
SC-004 baseline re-anchors at PR 3c; the threshold may need adjustment
downward to reflect intentional lockout, OR the corpus may need
re-curation to exclude open-vocab cases that were never legitimately
fixable. Decision deferred to PR 3c implementation; flag in PR 3c review.

### 8.3 Message channel closure

`Diagnostic::message` becomes:

```rust
pub struct Message {
    template: MessageTemplate,
    args: MessageArgs,
}

enum MessageTemplate {
    DecoderRecognized,                      // was: format!("decoder-recognized canonical form: {replacement:?}")
    BannerMissingClassification,
    PortionUnknownDissem,
    /* ... closed set of stable string templates */
}

struct MessageArgs {
    /* closed set of permitted argument types: TokenId, Span, BLAKE3, Confidence, FeatureId */
    /* never: raw bytes, &str slices of input, format!-interpolated content */
}
```

The `engine.rs:1389` interpolation `format!("decoder-recognized canonical
form: {replacement:?}")` deletes; replaced by
`Message::new(MessageTemplate::DecoderRecognized, MessageArgs { token: ... })`.
The template renders to a stable string with no input-byte interpolation;
the audit consumer reads the template enum + args, not a free-form string.

This is what makes I-2 a type invariant rather than a grep firewall.

---

## 9. Pass-split semantics

Pass 1 (localized token rewrites) → splice → re-parse → pass 2
(whole-marking rules including E003) needs more than the original plan
specified. The murder board (backend-architect #4) identified three
failure modes; this section specifies the resolution.

### 9.1 Phase-tagged rules at registration

Each rule declares its phase at construction:

```rust
enum Phase {
    Localized,      // span MUST be strictly inside a single token boundary
    WholeMarking,   // span MUST cover a full marking span
}

trait Rule {
    fn phase(&self) -> Phase;
    /* ... */
}
```

Engine enforces at registration:
- `Phase::Localized` rule's `FixProposal::span` is sub-token-only.
- `Phase::WholeMarking` rule's span covers a full marking.

Each rule belongs to exactly one phase. If a defect class genuinely
needs detection in both phases (rare), register two rule entries
sharing a backend module — one `Phase::Localized`, one
`Phase::WholeMarking` — each with its own `RuleId`. This keeps the
dispatch contract single-valued at registration and surfaces the
"this rule is doing two distinct jobs" cost at the rule-set level
where it can be reviewed, rather than hiding it behind a `Both`
escape hatch.

### 9.2 I-18 — span non-overlap between passes

For any pass-1 `AppliedFix` with span S₁ and any pass-2 `AppliedFix`
with span S₂: **S₁ ∩ S₂ = ∅**.

Enforcement: engine's pass-2 dispatch filters out diagnostics whose span
overlaps any pass-1 span; the diagnostic demotes to a *suggestion* in
the audit log (`severity: suggest`), not auto-applied. The user sees
the suggestion in CLI/IDE output but the document is not auto-modified
twice on the same span.

Regression catch: property test in
`crates/engine/tests/two_pass_invariants.rs` shuffling pass-1 / pass-2
fix orderings and asserting non-overlap in promoted `AppliedFix`
records.

### 9.3 I-19 — reshape-aware whole-marking rules

Pass-1 fix that reshapes a token (e.g., `OC → ORCON` lengthens the
span; pass-2 E003 ordering predicate reads the lengthened form) must
not fire pass-2 rules that retroactively satisfy on the reshaped bytes
when the original predicate held against the pre-reshape attrs.

`RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>` is
populated from the pass-0 (original) parse cache for `Phase::WholeMarking`
rules; `None` for `Phase::Localized` (pass-2 doesn't dispatch them).

`Phase::WholeMarking` rules whose span overlaps a pass-1 fix re-validate
against `pre_pass_1_attrs` before firing. If the predicate held against
the pre-reshape attrs, it was a real defect that pass-1 incidentally
fixed; pass-2 does not re-fire.

If the predicate did NOT hold against `pre_pass_1_attrs` but DOES hold
against post-pass-1 attrs, pass-1 introduced the predicate condition;
pass-2 fires (that's a real new defect introduced by the pass-1 fix —
but in the I-18 non-overlap model, that case shouldn't arise; it's a
double-check).

**Disambiguation — predicate held against both pre-pass-1 AND
post-pass-1 attrs:** treat as the same defect (do not re-fire) only
when the pass-2 rule's `RuleId` matches the pass-1 fix's `RuleId` or
their `(scheme, predicate-id)` keys point to the same constraint
catalog entry. If the rule IDs differ, fire — the post-pass-1 marking
violates a different rule that pass-1 didn't address. This handles the
"E001 fix happens to satisfy E003 retroactively while leaving E007
intact" case correctly: E001 doesn't re-fire (pass-1 did its job), E003
doesn't re-fire (incidental satisfaction), E007 fires (it's a different
predicate that pass-1 didn't touch).

Implementation cost is bounded — most pass-2 rules don't touch pass-1
spans.

### 9.4 R002 — re-parse-failure rollback

If `parse(post_pass_1_buffer)` fails:

- Pass-1 `AppliedFix` records remain in the audit log (they happened;
  the audit is honest about what was applied).
- Pass-2 does not run.
- Engine emits a new diagnostic class **`R002 — pass-1 fix produced
  unparseable buffer`** carrying the pass-1 fix IDs that contributed.
- Document state: pass-1 buffer is returned as the corrected document.
  The user sees the pass-1 fixes plus the R002 diagnostic.

This is **honest about partial progress**. Atomic rollback would lie
about what happened (the audit ledger would say "no fixes" while the
intermediate state was real); this approach keeps the audit ledger
coherent with document state.

R002 is minted by `marque-engine` alongside R001 (currently
`crates/engine/src/engine.rs:49`, `DECODER_RULE_ID`); lands in PR 7.
Centralizing the synthetic-engine-diagnostic IDs (R001, R002, …) into
`marque-rules` is a separate refactor not in scope for this plan.

**Sentinel `"engine"` scheme for synthetic engine diagnostics.** Under
the PR 3c rule-ID retirement to `(scheme, predicate-id)` form, R001 and
R002 carry the sentinel scheme `"engine"`:

- `("engine", "r001.decoder-recognized")` (R001, lands today via PR 3c rule-ID retirement)
- `("engine", "r002.reparse-failed")` (R002, lands at PR 7)

Rationale: R001/R002 are minted by the engine, not by a `MarkingScheme`
implementation. Inheriting the active scheme's namespace
(`("capco", "engine.r001....")`) would lie about provenance — the
diagnostic is *about* a CAPCO marking but isn't *from* CAPCO. Using a
sentinel scheme keeps `("capco", ...)` cleanly meaning "from a CAPCO
rule" and leaves room for future schemes (`("cui", ...)`,
`("nato", ...)`, etc.) to follow the same convention. The sentinel
namespace is also forward-compatible with the deferred refactor that
centralizes engine-synthetic IDs into `marque-rules` — they all share
one scheme already.

`"engine"` is reserved at PR 3c rule-ID retirement and is not a valid
`MarkingScheme` registration target. The `(scheme, predicate-id)` form
allows it because `scheme` is a string, not a typed reference; the
audit-record contract documents `"engine"` as a reserved sentinel.

---

## 10. Audit clean break

### 10.1 Single-value schema validation

Pre-clean-break, the plan specified a monotonically-growing accept-list
(`["marque-mvp-1", "marque-mvp-2", ...]`) so older audit records would
remain readable by newer binaries. The murder board's W6 finding —
combined with the user's confirmation that no downstream consumers exist
— makes this scaffolding not just unnecessary but actively harmful (it
encodes a contract that doesn't apply, which makes it a load-bearing
fiction the next refactor inherits).

Post-clean-break:

- `MARQUE_AUDIT_SCHEMA` env var pinned at build time, validated against a
  **single value**, not an accept-list.
- The `mvp-N` naming retires. Post-keystone schema is `marque-1.0`.
- No `marque-audit-reader` crate. No reader-only feature flag. No
  forward-readability commitment.
- Pre-cutover records are unreadable by post-cutover binaries. There are
  no pre-cutover records (no users, no deployment); the property is a
  type-level guarantee, not a runtime concern.
- **CI absence-check** (FR-037 verification): a polish-phase script (e.g.,
  `tools/audit-cleanup-check.sh`, or folded into an existing CI step)
  asserts (a) no `crates/audit-reader/` directory exists; (b) no
  `audit-reader`, `marque-audit-reader`, or analogous reader feature
  appears in any workspace `Cargo.toml`; (c) no public re-export under
  `marque_engine::reader::*` exists. Negative requirements need a
  positive enforcement; comment-propagated absence is the failure mode
  the murder board (W6) called out.

### 10.2 Cutover composition

PR 3c bumps `marque-mvp-2 → marque-1.0` and bakes in:

- `FixReplacement::Strict | Decoder` discriminant on `FixProposal::replacement`.
- `Canonical<S>` provenance-tagged shape (`source: TokenSource::Cve(_) | OpenVocab { ... }`).
- `FixIntent<S>` rule-emission audit fields.
- `(scheme, predicate-id)` rule-ID form replacing `E###`/`W###`/`S###`/`C###`.
- Reserved slot for `FeatureId::PrecedingFixPenalty` (PR 7 fills it; no schema bump needed at PR 7).
- Reserved slot for R002 diagnostic class (PR 7 fills it).

**One audit-schema bump for the entire refactor sequence.** PR 8 bumps
the *priors* schema (`marque-priors-3`) — that's a separate
build-time-baked artifact, not the audit schema.

### 10.2.1 Audit-record JSON shape sketch (post-cutover)

Implementer guidance for PR 3c. Exact field names finalized at
implementation; the shape is what matters.

```jsonc
{
  "schema": "marque-1.0",
  "rule": { "scheme": "capco", "predicate_id": "banner.classification.usa-trigraph" },
  // Engine-minted synthetic diagnostics use the sentinel "engine" scheme:
  //   "rule": { "scheme": "engine", "predicate_id": "r001.decoder-recognized" }
  //   "rule": { "scheme": "engine", "predicate_id": "r002.reparse-failed" }
  // See §9.4 for the convention.
  "severity": "error",
  "span": { "start": 1024, "end": 1037 },
  "fix": {
    "replacement": {
      "discriminant": "strict",          // "strict" | "decoder"
      "canonical": {
        "source": "cve",                  // "cve" | "open_vocab"
        "token_id": "Classification.Secret",   // when source = "cve"
        // OR: "category": "SciCompartment", "render_call_site": "marque-capco/src/render.rs:142"
        "bytes_digest": "blake3:..."      // BLAKE3 of rendered bytes; bytes themselves not in record
      },
      "confidence": { "recognition": 0.95, "rule": 1.0, "combined": 0.95, "features": ["PrecedingFixPenalty"] }
    },
    "original_span": { "start": 1024, "end": 1037 },   // span only, no bytes
    "original_digest": "blake3:..."                     // BLAKE3 of pre-fix bytes
  },
  "message": {
    "template": "BannerMissingClassification",          // closed enum
    "args": { "expected": "Classification.Secret" }     // closed-set scalar/ID types only
  },
  "timestamp": "2026-05-02T14:32:11Z",
  "classifier_id": "12345",                             // when present
  "dry_run": false
}
```

Constitution V Principle V's content-ignorance constraint is preserved
structurally: no document content fields, no `original` byte slice (only
span + digest), `message` is a template-args pair not a free-form string.
The `bytes_digest` field is mandatory for both strict and decoder
canonicals so corpus regression can verify content-ignorance by
construction (canary scan asserts the literal bytes never appear in the
NDJSON output).

### 10.3 What's preserved across the cutover

Constitution V Principle V's audit-record content-ignorance constraint:
no document content, no document metadata field values, no subject-claim
free-form text. Permitted identifiers: token canonicals, category IDs,
span offsets, BLAKE3 digests, posterior scalars, enumerated `FeatureId`
labels, **and now: enumerated `MessageTemplate` labels** (§8.3).

Post-cutover audit records are smaller (no `proposal.original` byte
slices; just `Span` references), more uniform (no carve-out for decoder
path; no `proposal.original = ""` discriminant), and structurally
content-ignorant.

---

## 11. Lattice §-resolution spike (PR 3.7)

`2026-05-01-lattice-design.md` is currently a stub. The murder board
(W2) identified the gate-as-stub failure mode — a tired reviewer can
sign off on a fillable acceptance checklist without resolving the eight
§10 open questions, reproducing the previous lattice attempt's "skimmed
complexity, ended up bandaged" outcome.

PR 3.7 is the explicit gate-resolution work, scheduled before PR 4
implementation begins.

### 11.1 Required deliverables

For each category in `2026-05-01-lattice-design.md` §§2–8:

1. §-citations to `crates/capco/docs/CAPCO-2016.md` covering the marking
   grammar and any commingling / dominance / supersession rules.
2. **Formal join semantics** — stated as a function with preconditions
   and postconditions, not prose.
3. **Worked examples** showing two non-trivial values joining and the
   result, including any edge cases the §-citation calls out.
4. **Property-test fixtures** named by file and test name covering
   assoc/comm/idem/identity-with-bottom for the category.
5. **Cross-axis fixtures** (new) where the category interacts with
   another category's dominance: FOUO eviction by classification > U
   AND by non-FD&R dissem; FGI banner roll-up #276; SCI cross-system
   canonicalization; AEA exemption commingling with classification.

### 11.2 Open question resolution

All eight items in lattice doc §10 must resolve to a §-citation +
explicit decision. The "explicitly deferred to a tracked issue" escape
valve in §9 is removed. If a question genuinely cannot resolve, it
blocks PR 4 — no soft punt.

§3 Q3 (`NF` clears `REL TO`: lattice op vs. `PageRewrite`) is **not an
open question** per CLAUDE.md "Phase B": `capco/noforn-clears-rel-to` is
already a declared `PageRewrite`. The dispatch shape is **projection
first, then `page_rewrites` apply within `project(Scope::Page, ...)`**
(see `crates/capco/src/scheme.rs::project` body). The topological
scheduler in `crates/engine/src/scheduler.rs` orders the rewrites among
themselves (writers before readers); it does not run them before
projection. Reframe Q3 as "confirm and document," not "decide."

### 11.3 Acceptance

PR 3.7 lands when:

- All §§2–8 sections satisfy items (1)–(5) above.
- All §10 items resolved (no deferrals).
- §3 Q3 reframed as confirm-and-document.
- A named reviewer (in the PR description) has confirmed each category's
  worked examples by hand against the §-citation.

**PR 3.7 also amends `2026-05-01-lattice-design.md` itself**, since the
lattice doc was written before the cross-axis fixture and escape-valve
decisions:

- §9 acceptance checklist: add a new item for cross-axis dominance
  fixtures (item (5) above).
- §9 acceptance checklist: delete the "or explicitly deferred to a
  tracked issue" clause — the escape valve is removed.
- §10 item #3 (`NF` clears `REL TO`: lattice op vs. `PageRewrite`):
  reframe as "confirm and document" per CLAUDE.md "Phase B"
  (`capco/noforn-clears-rel-to` is already a declared `PageRewrite`).

**Default owner**: the consolidated-plan author (or named successor in
the PR description). **Default deadline**: 2 weeks from PR 3c merge. If
the deadline slips, PRs 4–10 stall; reschedule with explicit team
review.

PR 4 cannot land before PR 3.7. If PR 3.7 stalls, PRs 4–10 stall; this
is the cost of taking the gate seriously.

---

## Appendix A: FD&R semantics for FOUO

CAPCO-2016 §H.8 identifies dissemination-control markings as either
foreign-disclosure-and-release (FD&R) or non-FD&R. The distinction
governs FOUO commingling.

### FD&R dissem markings (per user direction)

- `REL TO`
- `RELIDO`
- `NOFORN` (`NF`)
- `DISPLAY ONLY`
- `EYES` (deprecated; maps to `REL TO`)

### Non-FD&R dissem markings

Everything else: `RSEN`, `IMCON` (`IMC`), `PROPIN` (`PR`), `ORCON` (`OC`),
`ORCON-USGOV`, etc.

### FOUO commingling rules

FOUO has **two dominance axes**:

1. **Classification dominates FOUO.** FOUO cannot appear in any marking
   with classification > U (unclassified).
2. **Non-FD&R dissem dominates FOUO.** Any non-FD&R dissem token in the
   dissem set evicts FOUO.

Coexistence with FD&R-class dissems is preserved.

### Worked-example fixtures

For PR 4 / Layer 1 lattice law tests, and Layer 4 corpus regression
(`tests/corpus/lattice/`):

| Marking | Valid? | Reason |
|---------|--------|--------|
| `(U//REL TO USA, FVEY/DISPLAY ONLY UKR/FOUO)` | ✅ | FD&R-only with FOUO |
| `(U//NF/FOUO)` | ✅ | NF is FD&R; FOUO survives |
| `(C//FOUO)` | ❌ | Classification > U evicts FOUO |
| `(U//PR/FOUO)` | ❌ | PROPIN (non-FD&R) evicts FOUO |
| `(U//NF/IMC/FOUO)` | ❌ | IMCON (non-FD&R) evicts FOUO; NF being FD&R doesn't save it |

Separator note (CAPCO §A.5 p17 + Figure 2): `//` separates *categories*
and `/` separates *values within one category*. FOUO, NOFORN (NF),
PROPIN (PR), IMCON (IMC), REL TO, and DISPLAY ONLY are all IC
dissemination controls (group 8 of the Register), so they are
`/`-separated within the dissem category. Compare the canonical
CAPCO example `(S//NF/PR)` at §H.8 p148 and `(U//FOUO/REL TO USA, JPN)`
in CAPCO-2016 source.

Lattice-modeling note: §H.8 p134 prose ("FOUO does not convey in the
banner line if the document is UNCLASSIFIED with FOUO and other
dissemination control markings, excluding any FD&R markings") and
the §H.8 p134 "Commingling Rule(s) Within a Portion" describe FOUO
**display behavior** ("does not convey"). The lattice projection
treats "does not convey" as "drop the token from the canonical
form" — the canonical-form perspective makes display-eviction and
lattice-eviction equivalent. The fixtures' `❌` outcome is therefore
"the projected canonical form drops FOUO," not "the source portion
is illegal CAPCO."

Intra-category Register-order is a separate canonicalization concern
handled by `render_canonical`, not by the eviction lattice. The
fixtures above are intentionally written in user-input order
(reflecting common drafter error patterns), not Register-canonical
order.

The last fixture is deliberately constructed to be invalid for *one*
reason only (IMC eviction), not two — `IMC` is the portion-form
abbreviation, matching the surrounding portion-form dissems, so the test
isolates the FOUO+non-FD&R interaction without conflating form-mixing
as a second invalidity.

### Lattice expression

FOUO is a single-bit category in the dissem axis with two eviction
rules:

- Cross-category `Constraint`: `MarkingClassification > Unclassified`
  evicts FOUO (PR 9 declarative `Constraint`).
- In-category `SupersessionSet`: any dissem-set element with
  `is_fdr_dissem == false` supersedes FOUO (PR 4 lattice impl, detailed
  in `2026-05-01-lattice-design.md` §3 after PR 3.7 fill-in).

`is_fdr_dissem` is a per-token `Vocabulary<S>` metadata field; Phase 5
already added authority/owner/schema-version/portion-form metadata, so
this is a one-field extension.

---

## Appendix B: Issue → PR mapping

| Issue | PR | Mechanism |
|---|---|---|
| #106 | 9 | Parser separator spans |
| #246 | 9 | NATO control vocabulary + 7B `dissem_us`/`dissem_nato` |
| #251 | 9 | Banner-validation migration to `&ProjectedMarking` |
| #257 | 3c | `Canonical<S>` provenance-tagged + decoder open-vocab lockout (§8) |
| #258 | 8 | Decoder prose null hypothesis priors (third problem class — acknowledged not closed by this plan) |
| #260 | 8 | Decoder folds bare NATO {level} (third problem class — same) |
| #261 | 5 | `FgiSet` render-canonical drops redundant `FGI` |
| #263 | 3b | Rule collapse — Stage-1 13–18 (amended 2026-05-07; end-state 9–11 across PR 3.7 / PR 4 / PR 5+) |
| #264 | 9 | Banner-validation migration |
| #265 | 9 | NATO-portion-in-US-doc → REL TO USA, NATO declarative `Constraint` |
| #266 | — | Deferred (CAB out of immediate scope) |
| #267 Gap A | 3c | Fix-emission via `render_canonical` becomes mechanical |
| #267 Gap B | 3c | Same mechanism as Gap A |
| #267 Gap C | 0.6 (if F.1 surfaces) or 10 | F.1 corpus gate catches predicate-vs-canonical-example drift |
| #270 | 9 | Banner-validation migration |
| #271 | 9 | 7B position-attributed dissem |
| #272 | 7 | Phase-tagged pass split |
| #273 | 7 | Computed E003 confidence (pre-pass-1 attrs via I-19) |
| #274 | 7 | Suggested-reorder in E003 message |
| #276 partial | 5 | `expected_classification()` widening + `Us` hardcode kill |
| #276 commingled | 9 | FGI banner roll-up via `Scope::Page` projection |
| #277 | 1 | Single-pass forward splice (landed in PR #278) |
| #280 | 2 | `shape_admits` + FGI silent-skip → `None` + `FgiMarker::SourceConcealed \| Acknowledged` |

---

## Appendix C: Filed enhancements (deferred indefinitely)

- **7C `Vocabulary<S>::TokenId`** distinguishing US ORCON from NATO
  ORCON despite same surface — trigger: second `MarkingScheme` lands
  in-tree.
- **Multi-scheme interaction** (CUI ↔ CAPCO): expected first interaction
  for marque. Lattice supersession across schemes (e.g., declassified
  CAPCO → CUI transitions) will require trait-surface work; this is
  acceptable since `Vocabulary<S>`, `MarkingScheme`, `Codec<S>` ship
  semver-unstable.
- **8B citation passage extraction** — nice-to-have; demoted from
  citation-fidelity gate composition.
- **`marque-audit-reader`** — explicitly NOT scheduled. No downstream
  consumers; clean break.
- **#266** CAB Declassify On canned strings for AEA / NATO commingling
  (§E.4 AEA, §E.5 NATO) — out of immediate scope per user direction.

**`FixIntent<S>` is no longer in this list** — landed in PR 3c per
murder-board override (§3.1).

---

## Appendix D: Constitution check by PR

| PR | Primary Constitution exposure |
|----|-------------------------------|
| 0 | VI — preemptive infrastructure (AST-based masking-pin lint) |
| 0.5 | VIII — citation-string lint widened scope; F.1 skeleton |
| 0.6 | VIII — preemptive citation-defect fix |
| 1 | I, VI — splice rewrite + bench gate (already landed in PR #278) |
| 2 | I, III, IV, VIII — `Vocabulary<S>::shape_admits`, FGI discriminant, p99 latency probe |
| 3a | III, V, VI, VII — pivot split (`ParsedAttrs`/`CanonicalAttrs`/`ProjectedMarking`); no dep-graph change yet |
| 3b | IV, VI — rule collapse |
| 3c | III, V (G13 → type invariant via §8), VI, VII — discriminant, sealing, FixIntent, ID retire, schema cutover; **dep-graph: `marque-rules` gains a `marque-scheme` dep when `FixIntent<S>` lands here (graph depth +1, still acyclic)** |
| 3.7 | VI, VIII — lattice §-resolution + cross-axis fixtures |
| 4 | VI — lattice impls; `CapcoMarking::join` delegation deletion (clean break) |
| 5 | VI, VIII — `expected_classification` widening |
| 6 | I, VI — `Scope::Page` projection cutover; `lint_100kb_multipage` bench |
| 7 | I (latency budget within SC-001 with phase split, verified by `fix_10kb` bench), V (reserved schema slots filled), VI |
| 8 | III — decoder priors (third problem class) |
| 9 | IV, VI, VIII — banner-validation migration, declarative `Constraint`s, ATOMAL/BOHEMIA |
| 10 | VIII — F.1 maturation, 8C registry declared |

### Note on PR 3c dependency-graph shift

PR 3c is where `marque-rules` gains a `marque-scheme` dependency. PR 3a
adds the pivot types (`ParsedAttrs`/`CanonicalAttrs`/`ProjectedMarking`)
inside `marque-ism` (replacing `IsmAttributes`'s overloaded role).
Rules' input-type *signature* changes (`&IsmAttributes` → `&CanonicalAttrs`),
but both types live in `marque-ism` — a crate `marque-rules` already
depends on — so **no new crate-level dep-graph edge forms at 3a**.

The shift lands in PR 3c with `FixIntent<S>`: the rule-API value rules
emit instead of constructing `Canonical<S>` directly. `FixIntent<S>`
references scheme-defined types (`Scope`, `CategoryId`, `TokenId`),
which forces the `marque-rules → marque-scheme` edge. Per §8.1,
external rule crates **never construct `Canonical<S>` directly** — they
emit `FixIntent<S>` and the engine renders to `Canonical<S>` via
`MarkingScheme::render_canonical` against the sealed
`CanonicalConstructor<S>` impl.

Updated dependency graph:

```text
marque-ism    ←── marque-core ──────────────────────┐
marque-ism    ←── marque-scheme ←── marque-rules ←── marque-capco ──┤
                                                                    ↓
                                                              marque-engine ←── marque-config
                                                                    ↑
                                                              marque-wasm
                                                                    ↑
                                              marque-extract (non-WASM only)
                                                                    ↑
                                                              marque-server
                                                                    ↑
                                                               marque (CLI)
```

Read `A ←── B` as "B depends on A". `marque-rules` now depends on
`marque-scheme` (it always implicitly depended on the scheme abstraction;
PR 3a makes the dependency explicit). Graph remains acyclic.
`cargo check --workspace` passes; CLAUDE.md and the constitution graph
update in PR 3a.

---

## Appendix E: Murder-board findings cross-reference

For traceability — each murder-board finding (W1–W6 plus single-reviewer
items) and where this plan addresses it.

| Finding | Source | Resolution |
|---------|--------|------------|
| W1 — G13 closure leaky (six reviewers) | All | §8.1 (sealed `Canonical` constructor), §8.2 (decoder lockout), §8.3 (message channel closure), §10 (audit clean break) |
| W2 — PR 4 lattice gate gameable (five reviewers) | All | §11 (PR 3.7 spike), §6 Layer 1 (cross-axis fixtures) |
| W3 — F.1 too late (three reviewers + three concrete defects) | system-architect, security-engineer, quality-engineer | PR 0.5 + PR 0.6 (§4) |
| W4 — PR 3 monolith (three reviewers) | refactoring-expert, system-architect, backend-architect | PR 3a/3b/3c split (§4, §6) |
| W5 — Performance benches don't measure claims | performance-engineer | `fix_10kb`, `lint_100kb_multipage`, p99 tail (§6) |
| W6 — Audit-schema accept-list (three reviewers) | backend-architect, root-cause-analyst, system-architect | §10 (single-value validation) |
| FgiMarker shape collision | backend-architect #8 | PR 2 + I-9 + §2.4 |
| `format!("decoder-recognized canonical form: {replacement:?}")` at `engine.rs:1389` | root-cause-analyst F1 | §8.3 (`MessageTemplate`) |
| Pass-1 reshape | backend-architect #4 | §9 (P7-1 through P7-4, I-18, I-19, R002) |
| CAPCO-first hardening | system-architect #4 | §3.10 (semver-unstable, accept the cost) |
| `marque-rules` gains `marque-scheme` dep | system-architect #1 | Appendix D note |
| Citation defects in production | security-engineer F4 | PR 0.6 (preemptive) |
| Masking-pin cascade-close | security-engineer F5 | I-16 rule 4 (mandatory GitHub-API + dup chain follow) |

---

**End of plan.** This document supersedes
`docs/plans/2026-05-01-engine-rule-architecture-refactor.md`, which was
deleted in the PR that landed this consolidated plan.
