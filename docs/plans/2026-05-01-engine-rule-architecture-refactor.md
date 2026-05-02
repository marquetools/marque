<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Engine + rule architecture refactor

**Date:** 2026-05-01
**Status:** draft — pending murder board
**Synthesizes:** three independent reviews (system architecture, QA, backend
data-flow) of the open-issue corpus impacting rule behavior, correctness,
and engine/CAPCO interaction; followed by a user-driven decision pass that
collapsed the open disagreements.
**Builds on:**
- `2026-04-19-recursive-lattice-and-decoder.md` (Phase B trait surface;
  identifies `CapcoMarking::join` as still delegating to `PageContext`)
- `2026-04-20-long-horizon-roadmap.md` (G13 invariant articulation)
- `2026-04-23-typos-evaluation-and-fuzzy-vocab-matcher.md`
**User constraints:**
- marque has no users; deprecation/aliases are not load-bearing
- issue #263 (canonicalizer/renderer split + form-routing collapse,
  49 → ~10–13 rules) is a given
- optimize for 5-year maintenance
- CAPCO-first: no second `MarkingScheme` until CAPCO is solid; CAPCO
  is the most complex grammar marque will touch and sets precedent
**Constitution gates:** I (performance), III (WASM safety), IV (two-layer
rules), V (audit-first / G13), VI (dataflow pipeline), VII (acyclic
dependencies), VIII (citation fidelity).

---

## 0. Why this doc

The issue corpus from late 2026-04 surfaced a generative pattern: a
small number of structural defects in the engine and the `CapcoScheme`
adapter were producing issues on a steady cadence. The same shape kept
appearing — a leak between layers that should not have been able to
talk to each other (input bytes into audit records, parser output into
canonical rendering, page-level state into rule input via the wrong
channel).

Three independent reviews triangulated on the same root cause from
different vantage points. This document captures their consolidated
position, the user's decision pass on top of it, and an ordered PR
plan that closes the corpus in roughly the order they were filed
without leaving structural debt behind.

The document supersedes the working notes at
`/tmp/review-{architect-1,architect-2,qa,consolidated-addendum}.md`,
which were ephemeral.

---

## 1. The two underlying problems

The corpus has *one apparent* problem (lots of issues that look
unrelated) and *two underlying* problems. Closing each underlying
problem closes a class of issue, not a single one.

### 1.1 The pivot type does too many jobs

`IsmAttributes` (`crates/ism/src/attributes.rs`) is simultaneously:

1. **Parser output** — what `marque-core::parser` produces, possibly
   carrying degraded or partial structure on malformed input.
2. **Post-canonical form** — what rules read when they emit fix
   proposals; expected to be in canonical form (uppercase, ordered,
   deduplicated).
3. **Page roll-up output** — what `PageContext::expected_*` returns
   after aggregating across all portions on a page.

These three roles are confused at the type level, and the confusion
is generative:

- **#257** (R001 leak): decoder writes uppercased input bytes into
  `provenance.canonical_bytes`, which flow through `proposal.replacement`
  and the R001 message. The pivot can't tell parser output (input
  bytes) from canonical form, so the engine can't reject the
  contamination at the boundary. G13 (Constitution V Principle V)
  enforced by code-path comment, not type.
- **#272** / **#273** / **#274** (E003 confidence + ordering):
  E003 reads `attrs.token_spans` from the original parse and cannot
  see a pending E001 `OC → ORCON` localized fix. The C-1 overlap
  guard masks the bug incidentally — symptoms surface when overlap
  doesn't quite kick in.
- **#276** (FGI banner roll-up): `page_context_to_attrs` at
  `crates/capco/src/scheme.rs:365` hardcodes `MarkingClassification::Us`,
  silently flattening FGI/NATO/JOINT during page projection. The
  page roll-up can't represent non-US classification because the
  pivot's classification slot is `MarkingClassification`, not
  `Option<MarkingClassification>`.
- **#280** (silent open-vocabulary corruption):
  `parse_fgi_marker` (`parser.rs:1011-1024`) returns `Some(FgiMarker { countries: [] })`
  when post-prefix bytes fail `CountryCode::try_new`. The pivot
  can't distinguish "authentic source-concealed FGI" (lawful per §H.7
  p126) from "we failed to parse the trigraphs" because both shapes
  collide on the same `FgiMarker`. `parse_sar_program`
  (`parser.rs:1453, 1481, 1493`) uses `is_ascii_alphanumeric()` while
  line 1458 correctly uses `is_ascii_uppercase()` — same asymmetry.

### 1.2 The Phase-B trait surface is built but not load-bearing

`marque-scheme` shipped `MarkingScheme`, `Lattice`, `BoundedLattice`,
`Constraint`, `Scope`, `PageRewrite`, the built-in lattice constructors,
and the topological scheduler. The engine doesn't drive load-bearing
roll-up through any of it. `CapcoMarking::join` at
`crates/capco/src/scheme.rs:188-247` *currently violates* the lattice
laws by delegating to `PageContext` — the Phase B doc is honest about
the caveat but the per-category lattice math was never done. The
`Scope::Page` projection isn't wired into `Engine::lint`.

This is the second underlying problem and is the larger maintenance
risk. The previous attempt at lattice unification skimmed the
complexity and ended up bandaged until it was almost unused. Repeating
that pattern is the failure mode this plan is designed to avoid; PR 4
is therefore gated by a separate lattice design document
(`2026-05-01-lattice-design.md`) that must be reviewed before any code
lands.

---

## 2. Triple-confirmed convergences

Eight points on which all three reviewers independently arrived at the
same conclusion.

**2.1 #263 is the keystone refactor.** Canonicalizer + renderer split
+ form-routing collapse. 49 → ~10–13 rules. PR 3 below co-lands #263.

**2.2 The pivot type does too many jobs** (§1.1).

**2.3 Page-level aggregation should drive through `Scope::Page`
projection** (§1.2). Wire projection in, kill the `MarkingClassification::Us`
hardcode at `scheme.rs:365`, declare the missing rewrites, retire
`PageContext`.

**2.4 Open-vocabulary parser asymmetry (#280) is small and urgent.**
The fix is mechanical (replace `is_ascii_alphanumeric()` with
`Vocabulary<S>::shape_admits` at the four sites; replace silent-skip
with `None` return); the cost of leaving it is silent semantic
corruption.

**2.5 #277 single-pass forward splice goes first.** `Vec::splice` per
fix is O(N·M); single forward walk is linear. No schema change. Add
`fix_throughput/100mb` to SC-005.

**2.6 Two-pass apply (#272) is the right shape.** Pass 1 (localized
token rewrites) → splice → re-parse → pass 2 (whole-marking rules
including E003). Depends on PR 1 being in place.

**2.7 Decoder leak (#257) needs canonical-bytes validation against
shape-admission; #258 prose null hypothesis is independent.** The
decoder's `provenance.canonical_bytes → proposal.replacement → R001
message` channel is closed structurally by validating against
`Vocabulary<S>::shape_admits` (which covers both closed CVE tokens
AND structurally-permitted slots: SCI compartment grammar, SAR shape,
country trigraphs, tetragraphs). #258 lands separately at the
candidate-construction layer.

**2.8 Citation fidelity needs CI enforcement.** Constitution VIII
makes citation accuracy a correctness requirement; #267 Gap C (HCS-P
over-strict at `scheme.rs:1846`, contradicting the canonical example
in same cited §H.4 p66) shows convention has already failed once.
Combined gate: 8A (citation-string lint) + F.1 (corpus fixture per
cited authority) + 8C (vendored-source registry).

---

## 3. Resolved disagreements

Six substantive disagreements during the conferral. All resolved.

**3.1 Theme 1 (rule/engine boundary): three layers, not three
competitors.** `ParsedAttrs<'src>` / `CanonicalAttrs` / `ProjectedMarking`
(data-flow types) + `FixReplacement::Strict | Decoder` (audit
discriminant) + `FixIntent<S>` (rule-emission API) are layers of one
stack. Keystone PR 3 ships the first two. **`FixIntent` is filed as
an enhancement** (see Appendix C) — not deferred indefinitely.
PR 3's `Canonical` constructor is designed forward-compatible:
single rendering site (`MarkingScheme::render_canonical`) so the
later `FixIntent` migration is a rule-API change, not an engine
change. Trigger to land `FixIntent`: a third "rules-emitting-bytes"
leak channel files OR multi-scheme interaction (CUI ↔ CAPCO is the
expected first interaction) reaches design.

**3.2 Theme 2 (lattice laws first).** `CapcoMarking::join` currently
violates the laws. PR 4 (lattice-law foundation) lands before PR 5
(`expected_classification()` widening). Cost: #276 partial close
delays one PR. Worth it — widening on a known-broken lattice ships a
known law violation. PR 4 is gated by `2026-05-01-lattice-design.md`.

**3.3 Theme 4 (vocabulary scope).** `Vocabulary<S>::shape_admits`
covers both closed CVE tokens (admitted by membership) AND
structurally-permitted slots (admitted by generative rules: SAR
program ID shape, trigraph/tetragraph form, SCI compartment grammar).
Reconciles "no input bytes flow as canonicals" with "open-vocabulary
isn't a literal closed set."

**3.4 Theme 7 (dissem position attribution).** PR 9 ships 7B
(separate `dissem_us` / `dissem_nato` parser fields, position-
attributed) and closes #271. **7C (`Vocabulary<S>`-driven `TokenId`
distinguishing US ORCON from NATO ORCON despite same surface) is
deferred indefinitely** — speculative until a second `MarkingScheme`
exists in-tree, and per the user's CAPCO-first constraint that won't
happen until CAPCO is solid.

**3.5 Theme 8 (citation gates compose).** Three layers, three failure
modes:
- 8A (citation-string lint) catches *fabrication* mechanically — at
  PR time, every cited `§X.Y pNN` parses and exists in the vendored
  source. HCS-P (#267 Gap C) passes 8A — citation parses fine.
- F.1 (corpus fixture per cited authority) catches *predicate-vs-
  example drift* — runs canonical CAPCO-2016 examples through the
  engine. HCS-P fails F.1 — canonical example fires the over-strict
  predicate.
- 8C (vendored-source registry) catches *primary-source identity* —
  declares which file is the authority for each scheme. Lower
  priority given CAPCO-first; declarative-only.

8A → standalone PR 0.5. F.1 → PR 11. 8C → declared in PR 11; not
load-bearing until scheme #2. 8B (passage extraction) demoted to
nice-to-have.

**3.6 Theme 5 (sequencing on `shape_admits`): measurement-gated.**
PR 2 lands `Vocabulary<S>::shape_admits` AND an SC-001 latency probe
simultaneously. Threshold: >5% regression → back out to E.1
(case-strict at the four cited parser sites only — `parser.rs:1011-1024`,
`1453`, `1481`, `1493` — no extraction). The bench is what proves
indirect-call inlining holds; assumption alone doesn't.

---

## 4. PR sequence

Single ordered sequence. Each PR independently shippable; each maps
to specific issues; each carries a Constitution Check. Order respects
WASM-safety (Principle III) and the acyclic dependency graph
(Principle VII).

| PR | Description | Closes | Constitution check |
|----|-------------|--------|--------------------|
| 0 | `static_assertions` on rule + recognizer trait bounds (`Rule: Send + Sync`, `Recognizer<S>: Send + Sync`); masking-pin lint at `tools/masking-pin-lint/` | (preemptive) | VI |
| 0.5 | Citation-string lint (8A) at `tools/citation-lint/` | (preemptive) | VIII |
| 1 | Single-pass forward splice; add `fix_throughput/100mb` to SC-005 | #277 | I, VI |
| 2 | `Vocabulary<S>::shape_admits` + parser case-strict (measurement-gated, see §3.6); FGI silent-skip → `None`; `is_ascii_alphanumeric()` → `shape_admits` at the four parser sites | #280 | I, III, IV, VIII |
| 3 | **Keystone**: pivot split (`ParsedAttrs<'src>`/`CanonicalAttrs`/`ProjectedMarking`) + `FixReplacement::Strict\|Decoder` audit discriminant + #263 canonicalizer/renderer split. **No back-compat shim** (no users; clean break). `MARQUE_AUDIT_SCHEMA` bumps `mvp-2 → mvp-3`. `Canonical` constructor designed forward-compatible to `FixIntent` migration | #257, #263, #267 Gap A, #267 Gap B (fix-emission becomes mechanical via `render_canonical`) | III, V (G13 → type invariant), VI, VII |
| 4 | Lattice-law foundation: per-category `Lattice` impls + property tests. **Gated by `2026-05-01-lattice-design.md`** — design doc must be reviewed before code lands. Replaces `CapcoMarking::join`'s `PageContext` delegation with component-wise per-category joins | (regression gate) | VI |
| 5 | Widen `expected_classification()` → `Option<MarkingClassification>`; kill `MarkingClassification::Us` hardcode at `scheme.rs:365`; render-canonical drops redundant `FGI` token when trigraph present (#261 falls out) | #276 (partial), #261 | VI, VIII |
| 6 | Drive `scheme.project(Scope::Page, ...)` from `Engine::lint` behind feature flag; equivalence gate vs `PageContext` on full corpus | (cutover enabler) | VI |
| 7 | Phase split (pass 1 / re-parse / pass 2); computed E003 confidence with `FeatureId::PrecedingFixPenalty`; suggested-reorder in E003 message. `MARQUE_AUDIT_SCHEMA` bumps `mvp-3 → mvp-4` (per user direction; even if `FeatureId` is conditional, bump for clarity) | #272, #273, #274 | I, V, VI |
| 8 | Decoder prose null hypothesis priors (`marque-priors-3` schema bump); fold bare `NATO {level}` to canonical NATO marking | #258, #260 | III |
| 9 | Parser separator spans (#106); 7B `dissem_us` / `dissem_nato` position-attributed fields; banner-validation rules migrate to `&ProjectedMarking`; declare missing `PageRewrite`s; ATOMAL/BOHEMIA recognition; NATO-portion-in-US-doc → REL TO USA, NATO derivation as declarative `Constraint` | #106, #270, #271, #265, #246, #264, #251 | IV, VI, VIII |
| 10 | Retire `PageContext`; constraint-catalog migration. `Scope::Page` projection becomes the only roll-up path | (cleanup) | VI |
| 11 | F.1 corpus gate (canonical example per cited authority); 8C vendored-source registry declared; HCS-P over-strict predicate corrected | #267 Gap C | VIII |
| 11.5 | Retire `E###`/`W###`/`S###`/`C###` rule IDs in favor of `(scheme, predicate-id)` keyed on the constraint catalog. Audit records carry constraint identifiers, not artificial sequence numbers | (cleanup) | V |
| 12+ | Domain entries riding the new boundary, in user priority order: deferred until later phases. Note: #266 (CAB Declassify On canned strings for AEA / NATO commingling) deferred — out of immediate scope | (per priority below) | IV, VIII per declaration |

### PR 12+ priority

User-prioritized by frequency of real-world impact:

1. **#261** — drop redundant `FGI` when trigraph present *(falls out of PR 5 render-canonical change; track for confirmation)*
2. **#265** — NATO portion in US doc requires REL TO USA, NATO *(declarative `Constraint`; lands in PR 9)*
3. **#267 Gap A + Gap B** — companion-insert + E038 fix-emission *(mechanical once `Canonical` exists; lands in PR 3)*
4. **#260** — decoder folds bare NATO {level} *(decoder priors; lands in PR 8)*
5. **#246** — ATOMAL/BOHEMIA + US dissems in non-US markings *(vocabulary + 7B; lands in PR 9)*

Several of these "fall out" of the structural changes earlier in the
sequence — e.g., once `FgiSet`'s render-canonical drops the redundant
`FGI` literal when any trigraph is present, #261 closes without a
dedicated rule. The PR plan above places each in the slot where its
underlying mechanism lands, rather than queuing them as separate PRs.

### Audit-schema bumps

`MARQUE_AUDIT_SCHEMA` is build-time-pinned; the accept-list grows
monotonically; one binary emits one schema (FR-014).

| PR | Bump | Trigger | Compat note |
|----|------|---------|-------------|
| PR 3 | `marque-mvp-2` → `marque-mvp-3` | `FixProposal::replacement` becomes `FixReplacement::Strict(Canonical) \| Decoder(Canonical, Confidence)` | No back-compat shim (no users). Old records unsupported; clean break |
| PR 7 | `marque-mvp-3` → `marque-mvp-4` | Phase split adds `FeatureId::PrecedingFixPenalty` | Old `mvp-3` records unsupported |

PRs 5 / 6 / 9 / 10 / 11 / 11.5 grow the rule-ID accept-list (a *value*
of the schema, not its *shape*) — no bump required, even though
PR 11.5 retires the artificial-sequence form (the audit field becomes
a constraint identifier; that's a value range change, not a shape
change).

---

## 5. Invariants register

Seventeen invariants the post-migration engine must satisfy. Each
carries an enforcement mechanism and a regression catch. The register
is the acceptance criterion for "we're done" — at the end of PR 11.5
every invariant holds; until then, masking-pin discipline (I-16)
tracks gaps.

| # | Invariant | Enforcement | Regression catch |
|---|-----------|-------------|------------------|
| I-1 | Every byte in `FixProposal::replacement` came from `MarkingScheme::render_canonical(token, scope)` against a `Vocabulary<S>::shape_admits`-passing canonical | Convention at the rule level (PR 3); CI grep ensures rules don't construct `Box<str>` via `format!`/literal in production code | Property test on every rule's emitted replacement: tokenize-and-`shape_admits` covers every byte |
| I-2 | `FixProposal.original` and `Diagnostic.message` carry no document content bytes — only category IDs, span offsets, BLAKE3 digests, posterior scalars, enumerated `FeatureId` labels | Type system + constructor (PR 3); audit-shape carve-out at `engine.rs:1317-1323` deleted because `Canonical` cannot carry input bytes | `core_error_isolation.rs`; corpus canary scan for verbatim input |
| I-3 | `kept_fixes` non-overlapping in span order regardless of iteration direction | C-1 overlap guard (existing) | Property: shuffle, splice ascending vs descending, byte-identical |
| I-4 | Pass 2 reads only post-pass-1 buffer + `&CanonicalAttrs<'src>` | Engine re-parses between passes (PR 7) | Property: pass-1 token change feeds pass-2 rule input |
| I-5 | `Vec<AppliedFix>` monotonically appended; never reordered post-promotion | `Engine::fix_inner` (existing) | Snapshot test on audit-record sequence |
| I-6 | `Confidence::combined()` is the only threshold-comparison operator | `engine.rs:930` filter (existing) | Mutation: replace `combined()` with `recognition` only, assert SC-003 regression |
| I-7 | Decoder candidates always include the prose null hypothesis when one applies | `decoder.rs::recognize` (PR 8) | SC-003a precision gate vs `tests/corpus/prose/article.txt` |
| I-8 | Open-vocabulary identifier shape checks route through `Vocabulary<S>::shape_admits`; no inline `is_ascii_*` for category-typed tokens in `marque-core/parser.rs` | Refactor PR 2; CI grep flags drift | Per-fixture: `(TS//SAR-fk)`, `(TS//FGI deu)`; CI grep |
| I-9 | `parse_fgi_marker` returns `None` (not `Some` with degraded structure) when post-prefix bytes fail `shape_admits` | PR 2; silent-skip path at `parser.rs:1011-1024` deleted | `tests/parser/fgi_silent_skip_guard.rs` |
| I-10 | Every `Constraint`/`PageRewrite`/`Rule` cited authority has ≥1 corpus fixture exercising the predicate against the canonical example | `crates/capco/tests/citation_fidelity.rs` (PR 11 — F.1) | The CI gate; HCS-P would have failed it |
| I-11 | Every `Rule` and `Recognizer<S>` impl is `Send + Sync` | `static_assertions::assert_impl_all!` from `RuleSet::new()` (PR 0) | Compile-fail at construction |
| I-12 | `Scope::Page` projection is the source of truth for banner-validation rule input; `PageContext`-only paths fail the equivalence test | Lattice foundation (PR 4) makes `scheme.project(Scope::Page, ...)` lawful; corpus equivalence test introduced in PR 4; projection enabled behind flag in PR 6 (cutover gate); `PageContext` deleted in PR 10. **Depends on I-17 holding** | The equivalence test (PR 4 introduction; PR 6 cutover) |
| I-13 | `MarkingClassification::Us` never hardcoded in any projection function; `expected_classification()` returns `Option<MarkingClassification>` | Removal of `scheme.rs:365` (PR 5) | `tests/corpus/foreign/pure_foreign_banner.json` (#276 reproduction) |
| I-14 | `MARQUE_AUDIT_SCHEMA` build-time-pinned; one schema per binary (FR-014) | Build-time validation (existing) | `audit_schema_consistency.rs` |
| I-15 | `AppliedFix::__engine_promote` and `EnginePromotionToken::__engine_construct` called only from `Engine::fix_inner` in production code (test-fixture carve-out per Constitution V) | Convention; type-level seal via `EnginePromotionToken`'s private field (existing) | CI grep gate; `#[cfg(test)]`/`tests/` carve-out |
| I-16 | Every `with_recognizer(StrictRecognizer)` test pin carries `// MASKING-PIN: tracks #NNN` (with `#NNN` open) or `// INTENTIONAL-STRICT: <reason>`; masking pins removed in the issue-closing PR | `tools/masking-pin-lint/`. Backfill: 2 masking pins (`core_error_isolation.rs` → #257, `corpus_accuracy.rs` → #258), 5 intentional pins | The lint; both masking pins die after PR 3 + PR 8 |
| I-17 | Every category in `CapcoScheme::categories()` has a `Lattice` impl satisfying assoc/comm/idem/identity-with-bottom | Property tests in `crates/capco/tests/category_lattice_laws.rs` (PR 4); component-wise impls replace `CapcoMarking::join`'s `PageContext` delegation. Required design content in `2026-05-01-lattice-design.md` | Property tests + corpus equivalence (PR 6 cutover gate) |

### Decoder/strict drift as a type-system gap

The R001 leak (#257) and saturation channel (#258) are the same defect
from two angles: `StrictRecognizer` and `DecoderRecognizer` produce
identical `Parsed<S::Marking>` shape but operate under different
correctness properties. Strict-path invariants hold by construction
(every byte in `attrs.token_spans` is a CVE canonical or structurally-
validated open-vocabulary token). Decoder-path invariants today hold
by carve-out (`provenance.canonical_bytes` may include uppercased
unrecognized input segments; `recognition_score()` saturates at
`0.999999` for solo candidates regardless of evidence). The
`proposal.original = ""` carve-out at `engine.rs:1317-1323` is the
tell — invariants enforced by comment-propagation across code paths
are the failure mode that produces this class of bug.

After PR 3: a decoder canonical carrying unrecognized bytes is
unconstructable as a `Canonical` (`shape_admits` rejects). The
decoder's contract becomes "produce `Parsed::Unambiguous` or
`Parsed::Ambiguous`" — same shape as strict, structurally validated.
#258 closes in PR 8 independently. The carve-out at
`engine.rs:1317-1323` is deleted in PR 3.

---

## 6. Test strategy

Five-layer property-test architecture. Each layer catches a class of
bug.

**Layer 1 — Lattice law tests per category** (gates I-17; PR 4). For
every category in `CapcoScheme::categories()`: associativity,
commutativity, idempotency, identity-with-bottom on `Lattice::join`.
Lives at `crates/capco/tests/category_lattice_laws.rs`, extending the
existing `lattice_laws.rs` and `proptest_lattice.rs` (which today
cover only `SciSet` / `SarSet` / `FgiSet`). Widening (PR 5) or
projecting (PR 6) before laws-pass is correct logic on a broken
substrate (§3.2).

**Layer 2 — Parse–render round-trip** (PR 2).
`prop_assert_eq!(parse(bytes), parse(render(parse(bytes))))`. Lives
at `crates/capco/tests/parse_render_roundtrip.rs`. Catches silent
semantic degradation (#280's `(TS//FGI deu) → FgiMarker { countries: [] }`).
Enforces "fail loud or canonicalize."

**Layer 3 — Per-pass fix invariants** (gates I-1, I-2, I-4; PRs 3 + 7).
At `crates/engine/tests/fix_invariants.rs`:

1. `applied_fix.span ⊆ marking_span`.
2. `Vocabulary<S>::shape_admits(category, replacement)` covers every byte.
3. Pass-2 fixes consume the post-pass-1 buffer (re-parse between passes).
4. Audit-record canary: randomized canary-marker scan of NDJSON
   output asserts no input bytes appear except inside span-offset
   identifiers (Constitution V Principle V / G13). Replaces
   `core_error_isolation.rs`'s masking pin once PR 3 lands.

**Layer 4 — Corpus regression sweeps** (PR 4 onward). Three corpora
× two recognizers = six CI runs:

| Corpus | Target | Catches |
|---|---|---|
| `tests/corpus/valid/` | zero auto-applied fixes (info/suggest permitted) | rule-layer false positives |
| `tests/corpus/mangled/` | ≥0.85 fix accuracy (SC-004) | recall regressions |
| `tests/corpus/prose/` (Gutenberg + Federalist + Wikipedia) | zero diagnostics | decoder null-hypothesis regressions |

Prose corpus verifies PR 8 and lifts `corpus_accuracy.rs`'s masking
pin.

**Layer 5 — Citation lint** (PR 0.5). `tools/citation-lint/` parses
every `citation:` field; asserts §X.Y exists in
`crates/capco/docs/CAPCO-2016.md`; page falls within markdown offsets;
§X.Y in normative range §A–H (project memory: §I–K not valid targets);
rejects bare `§NN` (the imprecise form at `scheme.rs:1796/1815`,
#267 Gap C). Complementary to F.1 (PR 11). Both land; do not bundle.

### Masking-pin discipline (I-16)

Verified `with_recognizer(StrictRecognizer)` inventory:

| Site | Category |
|---|---|
| `corpus_accuracy.rs:49` | **MASKING-PIN — tracks #258** (closes at PR 8) |
| `core_error_isolation.rs:92` | **MASKING-PIN — tracks #257** (closes at PR 3) |
| `decoder_dispatch.rs:29`, `audit.rs:120`, `decoder_diagnostic.rs:58`, `decoder_accuracy.rs:451 / :758` | INTENTIONAL-STRICT (5 sites) |

Two masking pins is the pragmatic ceiling. Lint rules (CI-enforced
at `tools/masking-pin-lint/`):

1. Every masking pin: `// MASKING-PIN: tracks #NNN — remove when issue closes.` within 5 lines.
2. Every intentional pin: `// INTENTIONAL-STRICT: <reason>` within 5 lines.
3. Unmarked pins fail CI. Optional GitHub-API check: tracked issue is open.
4. **Closure protocol**: when an issue closes, the pin is removed in the same PR. A closing PR that doesn't remove its pin is rejected.
5. A third masking pin requires a team-review approval comment.

I-16 lands at PR 0 alongside `static_assertions`. Backfill: 7 inline
comments.

### PR 3 fixture continuity

PR 3 reshapes several hundred `IsmAttributes { ... }` literals across
the test corpus. Mitigation: feature-gated keystone (`--canonicalizer`,
default off) with `CanonicalAttrs::from_parsed_unchecked` as a
`#[doc(hidden)]` transitional adapter. Rule-side migration opts in
per-rule-class. Corpus equivalence test gates the cutover. Three
revert points (flag off / per-rule revert / full PR revert) instead
of a single monolithic rollback.

---

## 7. Surviving dissent

Two §6 dissents from the original conferral are resolved by user
decision:

- **Rule IDs** (`E###`/`W###`/`S###`/`C###` retirement): **resolved**
  in favor of retirement at PR 11.5.
- **Designing for second `MarkingScheme`**: **resolved** in favor of
  CAPCO-first. 7C deferred indefinitely; 8C declarative-only.
  `Vocabulary<S>` and `MarkingScheme` trait surfaces still ship as
  planned because they're needed for `shape_admits` and `Scope::Page`
  projection regardless, but trait surfaces designed *purely* for
  cross-scheme abstraction stay minimal until CAPCO is solid and a
  second scheme actually arrives.

One dissent survives:

**Decoder needs more independent test surface.** Two existing test
pins (`core_error_isolation.rs`, `corpus_accuracy.rs`) mask decoder
defects; both close in PRs 3 + 8. The meta-pattern — a CI test pinned
to mask a known defect — should not be normalized. Masking-pin
discipline in §6 is binding, not aspirational.

### Reserved position (filed as enhancement, not deferred)

`FixIntent<S>` (per §3.1) is filed as an enhancement to land when
either trigger fires:

1. A third "rules-emitting-bytes" leak channel files (suggesting
   `Canonical` constructor + `shape_admits` discipline isn't catching
   the class).
2. CUI ↔ CAPCO scheme interaction reaches design (rules-portability
   becomes load-bearing instead of speculative).

PR 3's `Canonical` constructor is forward-compatible: a single
rendering site (`MarkingScheme::render_canonical`) so the later
migration is a rule-API change, not an engine change. The
multi-scheme triggers are real — CUI and CAPCO will interact
(CUI markings appearing in former-CAPCO-classified contexts;
declass-driven transitions) — but the keystone PR closes the
immediate corpus without it.

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

1. **Classification dominates FOUO.** FOUO cannot appear in any
   marking with classification > U (unclassified).
2. **Non-FD&R dissem dominates FOUO.** Any non-FD&R dissem token in
   the dissem set evicts FOUO.

Coexistence with FD&R-class dissems is preserved.

### Worked-example fixtures

For PR 4 / Layer 1 lattice law tests, and Layer 4 corpus regression:

| Marking | Valid? | Reason |
|---------|--------|--------|
| `(U//REL TO USA, FVEY/DISPLAY ONLY UKR//FOUO)` | ✅ | FD&R-only with FOUO |
| `(U//NF//FOUO)` | ✅ | NF is FD&R; FOUO survives |
| `(C//FOUO)` | ❌ | Classification > U evicts FOUO |
| `(U//PR//FOUO)` | ❌ | PROPIN (non-FD&R) evicts FOUO |
| `(U//NF/IMC//FOUO)` | ❌ | IMCON (non-FD&R) evicts FOUO; NF being FD&R doesn't save it |

The last fixture is deliberately constructed to be invalid for *one*
reason only (IMC eviction), not two — `IMC` is the portion-form
abbreviation, matching the surrounding portion-form dissems, so the
test isolates the FOUO+non-FD&R interaction without conflating
form-mixing as a second invalidity.

### Lattice expression

FOUO is a single-bit category in the dissem axis with two eviction
rules:

- Cross-category `Constraint`: `MarkingClassification > Unclassified`
  evicts FOUO (PR 9 declarative `Constraint`).
- In-category `SupersessionSet`: any dissem-set element with
  `is_fdr_dissem == false` supersedes FOUO (PR 4 lattice impl,
  detailed in `2026-05-01-lattice-design.md` §3).

`is_fdr_dissem` is a per-token `Vocabulary<S>` metadata field; Phase 5
already added authority/owner/schema-version/portion-form metadata,
so this is a one-field extension.

---

## Appendix B: Issue → PR mapping

| Issue | PR | Mechanism |
|---|---|---|
| #106 | 9 | Parser separator spans |
| #246 | 9 | NATO control vocabulary + 7B `dissem_us`/`dissem_nato` |
| #251 | 9 | Banner-validation migration to `&ProjectedMarking` |
| #257 | 3 | `Canonical` constructor structurally rejects unrecognized bytes |
| #258 | 8 | Decoder prose null hypothesis priors |
| #260 | 8 | Decoder folds bare NATO {level} |
| #261 | 5 | `FgiSet` render-canonical drops redundant `FGI` |
| #263 | 3 | Co-lands as keystone refactor |
| #264 | 9 | Banner-validation migration |
| #265 | 9 | NATO-portion-in-US-doc → REL TO USA, NATO declarative `Constraint` |
| #266 | — | Deferred (CAB out of immediate scope) |
| #267 Gap A | 3 | Fix-emission via `render_canonical` becomes mechanical |
| #267 Gap B | 3 | Same mechanism as Gap A |
| #267 Gap C | 11 | F.1 corpus gate catches predicate-vs-canonical-example drift |
| #270 | 9 | Banner-validation migration |
| #271 | 9 | 7B position-attributed dissem |
| #272 | 7 | Pass split |
| #273 | 7 | Computed E003 confidence |
| #274 | 7 | Suggested-reorder in E003 message |
| #276 partial | 5 | `expected_classification()` widening + `Us` hardcode kill |
| #276 commingled | 9 | FGI banner roll-up via `Scope::Page` projection |
| #277 | 1 | Single-pass forward splice |
| #280 | 2 | `shape_admits` + FGI silent-skip → `None` |

---

## Appendix C: Filed enhancements (not in this plan)

- **`FixIntent<S>` rule-emission API** — triggers: third leak channel
  OR multi-scheme interaction reaches design. PR 3 keeps the
  rendering site singular so this migration is rule-API-only.
- **7C `Vocabulary<S>::TokenId`** distinguishing US ORCON from NATO
  ORCON despite same surface — trigger: second `MarkingScheme` lands
  in-tree.
- **Multi-scheme interaction** (CUI ↔ CAPCO): expected first interaction
  for marque. Lattice supersession across schemes (e.g., declassified
  CAPCO → CUI transitions) will require trait-surface work.
- **8B citation passage extraction** — nice-to-have; demoted from
  citation-fidelity gate composition.
- **#266** CAB Declassify On canned strings for AEA / NATO commingling
  (§C.4, §C.5) — out of immediate scope per user direction.

---

## Appendix D: Constitution check by PR

Each PR's primary Constitution exposure:

- **PR 0, 0.5, 1**: Principles I, VI, VIII — preemptive infrastructure.
- **PR 2**: Principles III (WASM-safety of `Vocabulary` extension),
  IV (two-layer respected — generated `shape_admits` from Layer 1
  metadata; rules consume), VIII (citations).
- **PR 3 (keystone)**: Principles III (`Canonical` is WASM-safe;
  no I/O), V (G13 elevated from convention to type invariant), VI
  (phased pipeline preserved), VII (acyclic — `marque-rules`
  unchanged depth).
- **PR 4**: Principle VI (lattice cleanly within `marque-scheme` /
  `marque-capco`; engine doesn't change).
- **PR 5, 6**: Principles VI, VIII.
- **PR 7**: Principles I (latency budget within SC-001 with phase
  split — verified by bench), V (audit schema bumped intentionally),
  VI.
- **PR 8**: Principle III (decoder priors compile-time; no runtime
  config in WASM, per Principle III's WASM runtime-config restriction).
- **PR 9**: Principles IV (declarative `Constraint` shape; no rule
  bodies for dyadic invariants), VI, VIII.
- **PR 10**: Principle VI (cleanup).
- **PR 11**: Principle VIII (citation-fidelity gate).
- **PR 11.5**: Principle V (audit value range change; not a shape
  change so no schema bump; `(scheme, predicate-id)` form aligns
  audit records with constraint catalog).
