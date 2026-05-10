---
agent: Agent 2 (Decision 2/3/4 — catalog shape)
date: 2026-05-10
scope: PR 3c rule-catalog shape (walker decomposition; six no-fix declarative rules; custom-rule residue)
inputs:
  - specs/006-engine-rule-refactor/architecture.md
  - crates/capco/CAPCO-CONTEXT.md
  - specs/006-engine-rule-refactor/rule-body-audit.md
  - .specify/memory/constitution.md
  - crates/capco/src/scheme.rs
  - crates/capco/src/rules_declarative.rs
  - crates/capco/src/rules.rs
  - crates/scheme/src/scheme.rs
  - crates/scheme/src/constraint.rs
  - crates/engine/src/engine.rs (R001 / decoder synthesis)
discipline:
  - Read-only investigation; no code modified.
  - file:line citations on every concrete claim.
  - CAPCO § citations re-verified against crates/capco/docs/CAPCO-2016.md per Constitution VIII.
  - Pre-users (no deprecation phasing) per project memory.
---

# Decisions 2 / 3 / 4 — catalog shape (PR 3c)

The architectural restatement (`architecture.md` §"What rules are", §"What fixes are") names the renderer as the single source of canonical form, fact-set delta as the fix vocabulary, and `Constraint::Custom` as a *small principled exception*, not a junk drawer. The three decisions below are framed against that ground truth.

---

## Decision 2 — Walker decomposition strategy

### PM's lean
**(B) Inline rows as individual `Constraint` entries on `CapcoScheme`.** The walker pattern was a PR 3b transitional shape; the architectural restatement makes `Constraint` itself the dispatch primitive.

### Evidence

1. **The catalogs are already shaped like `Constraint::Custom` for the two structural walkers** (E058, E059). The 27 `ClassFloorRow` rows are declared verbatim as 27 `Constraint::Custom { name: "class-floor/<...>", label: <citation> }` entries inside `CapcoScheme::build_constraints` (`crates/capco/src/scheme.rs:1552-1684`). The 5 `SciPerSystemRow` rows are declared identically as `Constraint::Custom { name: "sci-per-system/<...>", label: <citation> }` entries (`crates/capco/src/scheme.rs:1722-1742`). The catalog rows themselves (`CLASS_FLOOR_CATALOG`, `SCI_PER_SYSTEM_CATALOG`) live as `&'static [ClassFloorRow]` / `&'static [SciPerSystemRow]` private to the scheme module (`scheme.rs:3312`, `scheme.rs:4320`). Row scaffolding already pairs each entry with its `Constraint::Custom` declaration via the `name` field (`scheme.rs:2718-2756`, `scheme.rs:3744-3762`). **Rows are not walker-private; they are name-keyed `Constraint::Custom` entries with a hot-path side-table.**

2. **What the walker bodies do that a generic `Constraint` evaluator on `CapcoScheme` does NOT do today** (`crates/capco/src/rules_declarative.rs:1567-1625` for E058, `:1746-1776` for E059, `:1746-2017` plus per-row evaluators for E060):

   - **Per-portion early-out guard** — the walker pre-computes 5 axis-presence flags once per portion and skips rows whose axis is empty (`rules_declarative.rs:1574-1584`, `:1747-1755`). This is a pure perf optimization; the generic evaluator currently does row-presence checks inside each Custom predicate (`scheme.rs:2851-2856`).
   - **Coarse axis dispatch on `ClassFloorAxis`** — the walker reads `row.axis` and skips by enum tag without invoking `(row.presence)(attrs)` (`rules_declarative.rs:1593-1607`). Same shape, perf only.
   - **Span-anchor resolution per row** (`rules_declarative.rs:1611-1614` calling `class_floor_anchor_span`) — the walker resolves a `Span` for the diagnostic by reading `row.primary_kind` and walking `attrs.token_spans`. The generic evaluator returns `ConstraintViolation`, which **does not carry a span** (`crates/scheme/src/constraint.rs:155-160`). This is structural, not perf.
   - **Per-row severity propagation** — `Diagnostic.severity = row.severity` (`rules_declarative.rs:1617`). `ConstraintViolation` **does not carry severity** (`constraint.rs:155-160`).
   - **Fix-shape construction** — E059's `emit_companion_insert` produces a `FixProposal` with span + replacement bytes (`scheme.rs:3885-3915`). `ConstraintViolation` carries no fix.

3. **Generic-evaluator capability gap.** The shared `marque_scheme::constraint::evaluate` walker (`crates/scheme/src/constraint.rs:180-256`) emits `ConstraintViolation { constraint_label, message, citation }` with three fields. To absorb the three walker bodies, the evaluator would need to additionally carry **(a) a span anchor**, **(b) a per-row severity**, and **(c) for E059 the fix proposal** (companion-insert fix, `confidence: 0.9` per `scheme.rs:3905`). All three are scheme-level signals not visible from `Constraint::Custom { name, label }` alone. Either:
   - `ConstraintViolation` grows three optional fields (`span: Option<Span>`, `severity: Option<Severity>`, `fix: Option<FixProposal>`), or
   - the scheme exposes a richer `evaluate_custom` return type that includes these (engine consumes), or
   - the walker stays as a thin Rule that calls `scheme.evaluate_custom` and decorates with span/severity/fix from a parallel side-table — which is what the walker is today.

4. **Inline rule-ID strategy.** The audit table records 37 rows across the three walkers. If E058 and E059 inline (32 rows), each row's catalog row name (`class-floor/HCS-comp-sub`, `sci-per-system/HCS-O-companions`, etc.) is *already* the stable identifier the user-facing config keys against — the walker uses the rule-level ID (E058/E059) only because severity-config can't currently address rows. **Per the architecture restatement and the rule-body audit, per-row identification flowing via `name` (not a `RuleId`) is the right level**: the diagnostic surface is "constraint X fired against this marking," not "rule Y emitted a diagnostic about constraint X." E060 is a different shape — see point 6.

5. **Per-row §-citation preservation, sampled across the three catalogs**:
   - E058 row `class-floor/HCS-comp-sub` → `"CAPCO-2016 §H.4"` (`scheme.rs:3320`) — verified at `CAPCO-2016.md` §H.4 p61 (SCI grammar with TS-only sub-compartments per §H.4 p68 for HCS-P SUB).
   - E058 row `class-floor/RD-SG` → `"CAPCO-2016 §H.6 p113"` (`scheme.rs:3417`) — verified, FRD-SIGMA entry.
   - E058 row `E058/CNWDI-classification-floor` → `"CAPCO-2016 §H.6 p104"` (`scheme.rs:3441`) — verified, RD entry naming CNWDI-as-RD subset.
   - E059 row `sci-per-system/HCS-O-companions` → `"CAPCO-2016 §H.4 p64"` (`scheme.rs:4329`) — verified, HCS-O entry "requires ORCON + NOFORN".
   - E060 row `non-canonical/sigma-numeric-sort` → `"CAPCO-2016 §H.6 p108"` (`rules_declarative.rs:1998`) — verified, RD-SIGMA entry "current SIGMAs: 14, 15, 18, 20" with numeric-sort grammar.

   All five row-level citations preserve verbatim through the inline transformation — the catalog row's `citation` field is identical to what the `Constraint::Custom { label }` already carries (`scheme.rs:1552-1684` and `:1722-1742`). **Constitution VIII discipline is not threatened by the inline move.**

6. **E060 is structurally different from E058 / E059** — and the audit + walker docs both name this. The 5 rows in `NON_CANONICAL_CATALOG` are private to `rules_declarative.rs` (`:1974-2017`) and **deliberately do NOT have `Constraint::Custom` entries on the scheme**. The walker doc (`:1820-1828`) is explicit: *"PR 3b.D / 3b.E catalogs ... are cross-axis predicates over canonical attributes. PR 3b.F's invariants are not predicates over canonical attributes; they are non-canonical input detection — the invariant fires when the surface-form token order in the source bytes differs from the canonical representative."* These are renderer-canonical-form concerns. The audit's table column 4 names `MarkingScheme::render_canonical` as the absorb-into target for every E060 row (`rule-body-audit.md` row "E060"). The walker self-identifies as "STAGE-1 INTERIM ... retires cleanly when `MarkingScheme::render_canonical` lands" (per audit; verified by walker citation in `rules_declarative.rs:1804-1808`).

   **`render_canonical` is NOT yet on the `MarkingScheme` trait surface** (`crates/scheme/src/scheme.rs:43-154` — only `render_portion` and `render_banner` exist). E060's ultimate retirement target therefore depends on a Stage 4 / PR 5+ trait-surface addition that this PR is not landing.

### Recommendation

**Hybrid path**, structurally honest about the three walkers being three different things:

- **E058 (27 class-floor rows): Inline as 27 individual `Constraint::Custom` entries; retire the walker; add a sibling primitive.** The path the PM is leaning toward, with one prerequisite the gap analysis surfaces. The 27 rows are **already declared** as `Constraint::Custom` entries (`scheme.rs:1552-1684`); the walker is the second emission path on the same data. Inlining means **the walker file disappears**, the per-row severity / span / message lives on `ClassFloorRow` (already does, `scheme.rs:2731-2747`), and `marque_scheme::constraint::evaluate` becomes the dispatcher. Prerequisite: extend `ConstraintViolation` with `Option<Span>` + `Option<Severity>` (or extend the trait surface with a richer return type for `Custom` arms). Class-floor rows have no fix today (the audit's "fix-logic match" column reads `none-needed today; natural shape is FactAdd` for floor rows), so no `FixProposal` plumbing is needed in this PR.

- **E059 (5 SCI per-system rows): Inline if and only if `FactAdd` / `FactRemove` are on the trait surface in PR 3c.** E059 rows do produce fixes today — `confidence: 0.9` companion-insert spans (`scheme.rs:3899-3909`). The rows mix `CompanionRequired` (FactAdd shape) with `Custom` closures (multi-branch FactAdd + FactRemove for ORCON-USGOV → ORCON, see audit row E059). If PR 3c lands `FactAdd` / `FactRemove` as `Diagnostic`-attachable values from declarative constraints, E059 inlines the same way E058 does. **If not, E059 stays as a walker** until those primitives land — fixes are too valuable to lose, and the audit's E059 row already documents that the natural split is `CompanionRequired (FactAdd)` and `forbid (FactRemove)`. **Inlining without the fact-set fix vocabulary degrades the walker into 5 no-fix Errors, which is a regression** the rule-body audit specifically calls out as a PR-3c-only-if-emission-vocab-lands constraint.

- **E060 (5 non-canonical-input rows): Stay as a walker; do not inline.** The rows are not `Constraint::Custom` entries on `CapcoScheme` *by design* (`rules_declarative.rs:1820-1828`). They are renderer-canonical-form concerns that retire when `render_canonical` lands on the trait surface (Stage 4 / PR 5+, not this PR per `architecture.md` §"What this does NOT commit us to"). The walker's role in PR 3c is **stable**; the ID-reduction win is zero. Inlining E060 as `Constraint::Custom` would (a) hide the renderer-target by pretending it's a constraint over canonical attributes when it isn't, and (b) require a citation-fidelity move that the row currently encodes via the `SCI_CITATION_ANCHOR` compile-time prefix-equality discipline (`rules_declarative.rs:1907-1931`) that has no analogue in the constraint catalog yet.

### Rationale
The PM lean is right for the catalogs that are already half-inlined (E058, E059) and wrong for E060 (which is not a constraint). Treating "walker decomposition" as a single decision papers over the structural difference: E058/E059 are constraint catalogs with a side-table for richer emission; E060 is a renderer-correctness catalog parked behind a walker until the renderer lands.

The inline path respects the architecture restatement's principle that `Constraint` is the dispatch primitive (`architecture.md` §"The §3.0.b purpose split — where each rule lives") and the constitution's audit-first invariant (per-row `name` is what shows up in the audit stream, the walker's `RuleId::new("E058")` is bookkeeping). The keep-as-walker path on E060 respects the architecture restatement's principle that form is renderer territory, not constraint territory (`architecture.md` §"The 'form is not shape' principle (§3.0.a)").

### Tradeoffs
- **Inline E058 (and conditionally E059) costs**: a primitive extension on `marque-scheme` (`ConstraintViolation` grows fields, or `evaluate_custom` returns a richer type). This is a leaf-crate API change touching the trait surface — visible blast radius. Per the constitution's directionality rule (`marque-scheme` is the leaf), additions are legal but have to be justified. The justification is solid: the existing return type can't carry the data the engine needs, and the walker's existence is *structurally* the cost of that gap.
- **Inline E058 benefits**: ~1 walker rule removed (E058 → 0 emitting `RuleId`s), ~1 file (`rules_declarative.rs` walker section) removed, ~600 lines of walker dispatch + axis-flag scaffolding deleted. Per-row severity-config (`[rules] class-floor/HCS-comp-sub = "warn"`) becomes possible, which is a feature win the user has requested as an override surface (per `marque-applied.md` §3.0.b "every rule must be overridable at compile time, and severity adjustable at runtime").
- **Keep E060 walker cost**: walker file stays; per-row severity-config goes through `E060` rule-level only. The walker self-documents its own retirement (`rules_declarative.rs:1804-1808`), so the keep-decision is small-cost-high-clarity.
- **Risk on E059 inline-without-fact-set-vocab**: regression to no-fix; loses 5 currently-applying companion-insert fixes. Audit confirms this is unacceptable (the rule-body audit's "fix-logic match" column for E059 reads "none-needed today (no fix; some rows have token-replacement fixes for the forbid-companion sub-cases per the in-tree comment)" but the walker code has confidence-0.9 fixes today (`scheme.rs:3905`)). Audit and code disagree slightly here — the code has more fixes than the audit credits. Inlining without fix vocab strictly loses ground.

### Confidence
**Medium-high.** The structural point (E058/E059 catalogs are already shaped as `Constraint::Custom` with a hot-path side-table) is verifiable from the source. The "what does the walker do that the evaluator doesn't" gap is concrete — three named fields. The E060 distinction is documented by the walker's own doc and the audit. Open question: whether `ConstraintViolation` extension is in scope for PR 3c or is a separate primitive PR (Decision 1/8/10 territory; not mine to answer). **Affects Decision 1: trait-surface scope** — inline-E058 requires `ConstraintViolation` to gain span+severity, or `evaluate_custom` to return a richer type. **Affects Decision 5/6: fix-vocabulary primitives** — inline-E059 requires `FactAdd` / `FactRemove` available at the constraint-evaluator boundary, not just at the rule layer.

---

## Decision 3 — Six no-fix declarative rules (E021, E024, E036, E037, E038, E041): feature add or refactor

### PM's lean
**Refactor — land vocabulary, leave fix bodies to a follow-up PR.** PR 3c is already large; six rules × (fix impl + correctness tests + idempotency tests + G13 closure tests) is a separable review surface.

### Evidence

1. **Each rule's body today**:

   | Rule | Citation | Detection | Today's emission | Audit's claimed natural shape |
   |---|---|---|---|---|
   | **E021** AEA-NOFORN | §H.6 p104 (RD), p111 (FRD), p120 (TFNI) | `violations_for("E021/aea-requires-noforn")` non-empty (`rules_declarative.rs:619`) | `Diagnostic` Error, `None` fix (`:625-634`) | `FactAdd { NOFORN, scope: portion.dissem }` |
   | **E024** RD-precedence | §H.6 p104 ("RD takes precedence... is conveyed in the banner line") | walks `attrs.aea_markings` for `Frd`/`Tfni` after RD predicate fires (`rules_declarative.rs:683-707`) | multi-emission Error, `None` fix (`:695-706`) | `FactRemove { FRD, scope }` + `FactRemove { TFNI, scope }` |
   | **E036** JOINT/HCS | §H.3 p57 ("May not be used with the HCS markings") | violations_for predicate (`:569`); span at the offending HCS token (`:581-586`) | Error, `None` fix (`:588-597`) | `FactRemove { HCS, scope: portion.sci }` |
   | **E037** NODIS/EXDIS conflict | §H.9 p172 + p174 (mutual exclusion) | violations_for + first non-IC dissem span (`:807-814`) | Error, `None` fix (`:816-824`) | `FactRemove` (one or the other) |
   | **E038** NODIS/EXDIS-implies-NOFORN | §H.9 p172 (EXDIS) + p174 (NODIS) ("May be used only with NOFORN") | violations_for + first non-IC dissem span (`:857`) | Error, `None` fix | `FactAdd { NOFORN, scope: portion.dissem }` |
   | **E041** NODIS-supersedes-EXDIS-in-portion | §H.9 p172 (EXDIS) + p174 (NODIS) ("NODIS supersedes EXDIS in the portion mark") | both NODIS and EXDIS in `attrs.non_ic_dissem`, portion-only (`rules.rs:5620-5650`) | Warn, `None` fix (rule's own doc explains why, `:5595-5606`) | `FactRemove { EXDIS, scope: portion.non_ic_dissem }` |

2. **Citation re-verification (Constitution VIII)** — sampled all six against `crates/capco/docs/CAPCO-2016.md`:
   - E021: §H.6 p104 RD entry mentions NOFORN-required; verified.
   - E024: §H.6 p104 "RD takes precedence over FRD/TFNI"; verified, see `CAPCO-CONTEXT.md:331`.
   - E036: §H.3 p57 "May not be used with the HCS markings or NOFORN markings"; verified, see `rules_declarative.rs:545-548`.
   - E037: §H.9 p172 (EXDIS) + p174 (NODIS) mutual exclusion; verified, see `CAPCO-CONTEXT.md:379-380`.
   - E038: §H.9 p172 / p174 "May be used only with NOFORN information"; verified, `CAPCO-CONTEXT.md:379-380`.
   - E041: §H.9 p172 + p174 "NODIS (ND) supersedes EXDIS (XD) in the portion mark"; verified verbatim in rule doc (`rules.rs:5562-5567`).

3. **Audit's "shape unambiguous" claim — worst case verification**. Picked **E024** as the worst case: the rule today emits *two* diagnostics (one per FRD/TFNI marking present alongside RD). The audit claims natural shape is `FactRemove { FRD }` + `FactRemove { TFNI }` (audit row E024). Walking through the §H.6 p104 citation: RD takes precedence, FRD evicts when RD present, TFNI evicts when RD present (the document's source-of-truth eviction language). The shape *is* `FactRemove`, but specifically: per audit row E024, "today the multi-emit pattern flags both losers without proposing the removal" — the citation directly dictates `FactRemove`, and the rule body already enumerates the loser tokens (`rules_declarative.rs:686-690`). Genuinely unambiguous.

   E036's removal target is also unambiguous: "JOINT may not be used with HCS" + JOINT is the more-binding marking → `FactRemove { HCS }` not `FactRemove { JOINT }`. The audit row E036 calls this out: "natural shape: `FactRemove { HCS }` since JOINT is the more-binding marking (per the §H.3 p57 specific exclusion); see ... the dissem-axis subtractive-fix pattern from the RELIDO conflict cluster (E054-E057) suggests a parallel SCI-axis subtractive-fix is justified."

   E037 is ambiguous about which token to remove (NODIS or EXDIS) **at the syntactic level**, but the citation pair §H.9 p172 + p174 is silent on portion-scope conflict resolution. **However**, E041 (p174) specifies that NODIS supersedes EXDIS, so the consolidated rule's `FactRemove` target is EXDIS for portion-scope. The audit acknowledges this by noting E037 + E041 both fire (`rules.rs:5575-5585`). For the PR-3c emission-vocabulary lift: E037's `FactRemove` target *is* unambiguous when the §H.9 p172/p174 supersession (E041's territory) is in scope — drop EXDIS.

4. **Cost estimate, per-rule** (lines of fix-impl + tests):
   - **Fix impl per rule (`FactAdd` / `FactRemove`)**: per architecture restatement, `FactAdd { token, scope }` and `FactRemove { token_ref, scope }` are the structural primitives. Each rule emits one (or for E024 / E037, slightly more nuanced) literal. ~5-10 lines per rule. ~30-60 lines total across six.
   - **Correctness tests per rule**: assert the fix proposal is emitted, asserts the canonical tokens it carries match the citation. ~15-25 lines per rule. ~90-150 lines total.
   - **Idempotency tests per rule**: lint-fix-lint converges in ≤2 passes, no oscillation between FactAdd and a lattice closure that would then revoke. ~10-15 lines per rule. ~60-90 lines total.
   - **G13 / Constitution V audit-content tests per rule**: confirm `AppliedFix.proposal.replacement` carries only token canonicals, no document content. ~5-10 lines per rule. ~30-60 lines total. (Constitution V Principle V; the engine-level G13 closure test in `tests/` would cover the family if structured well; per-rule is belt-and-suspenders.)

   **Total: ~210-360 lines of net-new code across six rules.** Bounded. Reviewable in one PR.

5. **What the audit says will be true once the fix vocabulary lands**: from `rule-body-audit.md` "What this implies for the next plan" — *"Other declarative rules that today are no-fix (E021, E024, E036, E037, E038, E041) gain real fix proposals once the directive vocabulary lands; the mandate is already in the catalog row, only the emission shape is missing."* The audit treats this as a clean separable feature.

6. **The 5-year-maintain litmus**:
   - Refactor-only path: PR 3c lands the `FactAdd`/`FactRemove` *vocabulary* on the trait surface; the six rules are migrated to construct `FactAdd` / `FactRemove` (structurally — they emit the new types instead of `None`); but no fix is **promoted to AppliedFix** in PR 3c because the engine's threshold + promotion path needs a parallel update. This is a "vocabulary lands, semantics stays" move.
   - Refactor-with-fix path: PR 3c additionally wires the six rules to actually produce auto-applied fixes, with full test coverage.
   - The follow-up PR landing fix bodies is **small** because: (a) the vocabulary is already there, (b) the rule bodies already enumerate the offender tokens, (c) the citations already commit to the FactAdd / FactRemove shape, (d) the test scaffolding for confidence-threshold-based promotion already exists from the RELIDO cluster (E054-E057, `rules_declarative.rs:1213-1487`). The follow-up PR is "wire emission-shape construction; copy 6 test files from the RELIDO cluster as templates; verify against corpus." That's <500 lines total.

### Recommendation

**Refactor: land the `FactAdd` / `FactRemove` vocabulary in PR 3c; migrate the six rules to construct (but not auto-apply) fixes; defer auto-apply + full test scaffolding to a small follow-up PR.**

The middle path between "pure refactor (no shape change)" and "feature add (full fix bodies)". The six rules in PR 3c emit `FactAdd { NOFORN, dissem }` / `FactRemove { ... }` but the engine treats them as no-auto-apply (severity stays Error/Warn, threshold check fails by default, fix surfaces as suggestion). Follow-up PR adjusts confidence on each + adds promotion path.

**Rationale for not doing pure refactor**: the audit shows the natural shape is *already* unambiguous from each rule's citation — leaving the six rules emitting `Diagnostic.fix = None` after the vocabulary lands creates a conspicuous mismatch between the type system saying "fixes available" and the rule bodies refusing to construct them. That mismatch is a "to-do later" backlog signal future maintainers will read as architectural debt.

**Rationale for not doing full feature**: 6 × (fix impl + 4 test categories) is bounded but adds confidence-calibration decisions (the audit's "Confidence calibration drift signal" in §"Notable findings" is real — E054-E057 use 0.95, E001 uses 1.0, E029 uses 0.85; picking 6 fresh values requires a calibration-policy decision PR 3c doesn't need to take).

### Tradeoffs
- **Refactor-only (pure)**: avoids calibration decisions; smallest PR; but creates a 6-rule "to-do" surface that will look like debt at review time.
- **Refactor + middle path (recommended)**: lands the vocabulary, lands the structural shape, defers the calibration. **The follow-up PR is genuinely small** because the test scaffolding already exists (RELIDO cluster as template).
- **Feature add (full fixes in PR 3c)**: maximal value delivered; PR balloons by ~300 lines + calibration decisions; high merge risk for what is already a structurally-significant PR.

### Confidence
**High.** The audit's "shape unambiguous" claim is verified for the worst case (E024); five of six rules cite their canonical `FactAdd` / `FactRemove` target directly in their existing rule doc; the engineering cost is bounded. The middle-path recommendation is honest about which decisions are deferrable (calibration) vs which are not (vocabulary). **Affects Decision 5/6 (emission-shape primitives)**: the recommendation depends on `FactAdd` / `FactRemove` being on the trait surface in PR 3c — that's a shape decision not in my scope.

---

## Decision 4 — Custom-rule residue (E005, S005, S006)

### PM's lean
**Zero genuine `Constraint::Custom` rules. All three find non-Custom homes.** E005 → `Recanonicalize` at document scope (renderer places declass in CAB by construction). S005/S006 → admonition channel via the recognizer.

### Evidence

1. **E005's body** (`crates/capco/src/rules.rs:1141-1187`):
   - Detects `attrs.declassify_on.is_some() || attrs.declass_exemption.is_some()` on `MarkingType::Banner | Portion` (i.e., declass-data appears on a non-CAB marking).
   - Citations §E.1 p31 + §D.1 p27 + §C.1 p26 — all verified against `CAPCO-2016.md` (declass is a CAB line, banner/portion category sets exclude declass).
   - Today's emission: Error, `None` fix.
   - Why no fix: the rule's own doc (`:1135-1140`) is explicit: *"Repairing a misplaced declass marking requires moving the token from the banner/portion into a CAB, which is multi-span document-level rewriting rather than a local replacement."*

2. **`MarkingScheme::render_canonical` is NOT yet on the trait surface** (`crates/scheme/src/scheme.rs:43-154` lists `render_portion` and `render_banner` only). The renderer-side path the PM proposes for E005 — "renderer places declass in CAB by construction" — depends on a trait-surface addition that is **deferred per `architecture.md` §"What this does NOT commit us to"**. The architectural commitment names `render_canonical` as the future single source of canonical form, but the trait extension is not in PR 3c scope (per the architecture restatement's explicit deferral list).

3. **CAPCO §-citation for the canonical-position-of-declass claim** — verified at §E.1 p31 + §E.2 p32 (per the rule's existing citation chain). The §-language *does* dictate the canonical position: `Declassify On` is a CAB line, full stop. The renderer-by-construction repair is therefore §-grounded — a renderer that sees a `ProjectedMarking` carrying declass data and the marking's type is banner/portion will place declass in the CAB on render. The architecture restatement's `Recanonicalize { scope }` (`architecture.md` §"Recanonicalize { scope }") explicitly enumerates *"Block reordering (CAPCO §A.6 ordinal sequence)"* as a `Recanonicalize` use case — declass-position is a stricter form (block placement, not block reorder).

4. **The "but the renderer doesn't exist yet" gap**. Two paths:
   - **Path A**: PR 3c declares the *intent* — E005 emits a `Recanonicalize { scope: Document }` directive, but the directive is parked behind a feature-flag or a no-op renderer that still produces no auto-fix. Rule's emission shape changes; user-facing behavior doesn't. Stage 4 / PR 5+ wires the directive to a real renderer.
   - **Path B**: PR 3c keeps E005 as a `Constraint::Custom` (or as the existing hand-written rule) until `render_canonical` lands. Document-level multi-span surgery genuinely *is* outside the lattice / fact-set-delta primitives; the `Recanonicalize` scope today operates on a portion or banner block, not a document-spanning translation.

   The PM lean (E005 → `Recanonicalize` document-scope, renderer places by construction) is structurally right per `architecture.md`, but lands cleanly only **once** `render_canonical` lands. Until then, E005 is genuinely a multi-span document-level rewrite that doesn't fit the three-variant fix vocabulary.

5. **S005 / S006 bodies** (`crates/capco/src/rules.rs:3409-3427` + `:3540-3700` for the shared analyzer):
   - Both rules share `analyze_uncertain_reduction` (`:3540`), which produces `S005Candidate` values per uncertain tetragraph in REL TO whose membership data is incomplete (`is_decomposable == None`).
   - **Shared trigger**: page has 2+ portions with REL TO; an uncertain tetragraph drops out of the page-level intersection; non-empty other-codes set survives.
   - **S005 vs S006 split**: the page's banner REL TO is missing a code that atom-semantics says should survive (Suggest, S005) vs banner REL TO is *consistent* with atom-semantics (Info, S006). Per the module-level header (`:3345-3377`): *"Conceptually one diagnostic with a context-dependent severity ... Implementation-wise two registered rules because `marque_engine::Engine::lint` overwrites every emitted diagnostic's severity with the rule's configured/default severity."*

   The two-registered-rules split is a **workaround for an engine limitation**, not a CAPCO-§-grounded distinction. The audit row S005/S006 makes this point: *"the two rules exist as two registered impls only because the engine's severity-override layer cannot stably emit one rule at two severities."*

6. **The recognizer architecture and R001** (`crates/engine/src/engine.rs:1325-1499`):
   - R001 (`DECODER_RULE_ID = "R001"`, `:50`) is the synthetic decoder-recognition diagnostic the engine emits when a recognizer returned a marking carrying `DecoderProvenance` (`:1383-1499`).
   - R001 carries severity, span, message, citation, and a `FixProposal` with confidence derived from the decoder's `runner_up_ratio` posterior (`:1434-1440`). Audit-shape contract: G13 closure preserved (`proposal.original = ""` per `:1442-1448` per Constitution V Principle V).
   - **What R001 is**: a fix-shaped diagnostic emitted by the recognizer layer. Strictly more capable than rules (because it's already coupled to the recognizer's confidence machinery).

7. **"Admonition channel" — does this concept exist as code today?** — searching the codebase confirms no `admonition_emitter` / `admonition_channel` module exists. The audit's mention of admonition (`rule-body-audit.md` line 161 "admonition / `Constraint::Custom`; the two rules exist as two registered impls only because...") is forward-looking. `marque-applied.md` §3.0.b and the audit's purpose-row taxonomy both name `admonition` as a separate emission channel that does not exist yet:
   - `marque-applied.md:443-444`: *"Style / suggestion (Phase C — renderer; §3.5, §3.6)"* and *"Accompanying requirement — Out-of-engine (admonition emitter, audit trailer); §3.4.4 sidebar"*.
   - `marque-applied.md:730`: *"these belong in a separate emit channel (admonition emitter), not in the lattice or constraint catalog."*

   So the admonition channel is a **proposed** (not existing) concept. The PM's lean for S005/S006 → admonition channel is therefore proposing a new channel, not routing to an existing one.

### Recommendation

**Two-tier resolution that respects what's actually in the trait surface today**:

- **E005 — provisional `Constraint::Custom` in PR 3c; retire to `Recanonicalize { scope: Document }` in the same PR or follow-up that lands `render_canonical` on the trait surface.** The PM's lean is structurally correct (this is form-not-shape territory; renderer should place declass by construction per §E.1 + §D.1) but the trait surface is not ready. Until `render_canonical` lands, E005's invariant has no clean home — `Recanonicalize { scope: Document }` is the structurally-honest target, but it can't dispatch into a renderer body that doesn't exist. Naming E005 as a deferred-retirement `Constraint::Custom` honestly captures that the genuine fit is the renderer and the temporary fit is the constraint catalog.

- **S005 / S006 — admonition channel via the recognizer surface, but only if the channel is proposed and built in PR 3c.** Otherwise, **stay as registered rules in PR 3c; tag for retirement when the admonition channel lands**. The two rules' actual signal (recognizer-uncertainty, derived from incomplete tetragraph membership data the engine doesn't have) belongs structurally with R001 — same emission category (recognizer surface signaling about its own uncertainty). The split between S005 and S006 is a workaround for a per-rule severity limitation; merging back into one signal at two severities is the natural shape, which an admonition channel could express directly. **R001's existing surface admits this signal natively** (`engine.rs:1383-1499` shows it carries the full Diagnostic shape with FixProposal/confidence) — but only if S005/S006 fire from the recognizer, not from a Rule. Wiring this requires a recognizer extension.

   **Affects Decision 1 / Decision 7-11**: this question is "should PR 3c add the admonition channel" — that's an emission-surface scope question outside my decision scope.

### Rationale
The architecture restatement names `Constraint::Custom` as a small principled exception, not a junk drawer (`architecture.md` §"The §3.0.b purpose split"). All three rules genuinely don't fit the four structural homes (lattice law / page rewrite / Conflicts / Requires / form / decoder). E005 fits `Recanonicalize`-document but the renderer machinery isn't there. S005/S006 fit admonition / recognizer but the channel isn't there. **The honest answer is: zero genuine `Constraint::Custom` *in steady state*, two-or-three temporary `Constraint::Custom` until the missing trait-surface pieces (`render_canonical`, admonition emitter) land.** That matches what the audit and the architecture restatement both say.

The PM lean of "zero `Constraint::Custom`" is right as a steady-state commitment but lacks the bridge across the missing trait-surface pieces. Stating it without the bridge would either (a) require the bridge be built in PR 3c (scope explosion), or (b) admit `Constraint::Custom` is a temporary home and call it that.

### Tradeoffs
- **PM lean (zero `Constraint::Custom`) implemented in PR 3c**: requires both `render_canonical` trait extension AND admonition channel construction — both in scope, ~600+ lines of new code on top of everything else. **High merge risk.**
- **Recommended (provisional `Constraint::Custom` for 1-3 rules; retire when trait-surface lands)**: keeps PR 3c scope tractable; honest about which trait pieces are missing; calls out the retirement target so it doesn't become hidden debt. Cost: 3 named exceptions in the constraint catalog with explicit retirement comments.
- **Status quo (keep S005/S006 as registered rules; keep E005 as registered rule)**: smallest PR, but loses the structural commitment from the architecture restatement. Out-of-step with the rule-body audit's recommendation.

### Confidence
**Medium.** High confidence on the structural diagnosis (these three rules don't fit the four homes); medium confidence on the timing call (whether to retire in PR 3c or defer). The bridge question — does PR 3c land `render_canonical` on the trait surface — is not mine to answer. **Affects Decision 1 (trait-surface scope)**: the recommendation degrades to "stay as registered rules" if `render_canonical` is not in PR 3c scope. **Affects Decision 7-11 (admonition channel)**: the S005/S006 retirement target is dependent on the admonition channel being designed, not just declared.

---

## Cross-decision interactions

1. **Decision 2 ↔ Decision 5/6 (fix-vocabulary primitives, not in my scope)**: Inlining E058 is safe today; inlining E059 requires `FactAdd` / `FactRemove` on the trait surface in PR 3c (otherwise loses 5 currently-emitting fixes). Inlining E060 is structurally wrong regardless. The walker-vs-inline call cascades from the fix-vocabulary call.

2. **Decision 2 ↔ Decision 1 (trait-surface scope)**: Inlining either E058 or E059 requires `ConstraintViolation` to grow `Option<Span>` and `Option<Severity>` (or `evaluate_custom` to return a richer type). That's a leaf-crate API addition — Constitution VII directionality (`marque-scheme` as leaf) makes the addition legal but should be intentional. Affects Decision 1 in scope.

3. **Decision 3 ↔ Decision 5/6**: The recommended middle-path on the six no-fix rules depends on `FactAdd` / `FactRemove` being declared types in `marque-rules` (or `marque-scheme`) in PR 3c. The vocabulary lands in this PR; emission-promotion-to-`AppliedFix` happens in a small follow-up PR. If `FactAdd` / `FactRemove` is deferred entirely, the recommendation degrades to "leave as no-fix Errors and tag for the directive PR."

4. **Decision 4 ↔ Decision 1 (`render_canonical` scope) ↔ Decision 7-11 (admonition channel)**: E005's clean home is `Recanonicalize { scope: Document }` once `render_canonical` lands; S005/S006's clean home is the admonition channel once that channel exists. Both retirement targets are deferred trait-surface additions.

5. **Decision 2 ↔ Decision 4**: E060's "stay as walker" call (Decision 2) and S005/S006's "admonition or stay as rules" call (Decision 4) are the same family of decision — both are temporary parking until a renderer / admonition channel lands. The framing should be consistent: **temporary parking with an explicit retirement target named in source comments** is the right shape for both, and is what the existing E060 walker docstring already does (`rules_declarative.rs:1804-1808`). E005 / S005 / S006 retirement comments should follow that template.

6. **Affects Decision 8 / Decision 10 (citation surface and `MessageTemplate` choices)**: All catalog-row inline moves (Decision 2) and all six no-fix rule migrations (Decision 3) preserve per-row citations through the move (verified for 5 sample rows in §Decision 2 evidence). The `Constraint::Custom { label }` and `ClassFloorRow.citation` / `SciPerSystemRow.citation` / `NonCanonicalRow.citation` are **already the canonical citation site** for each row. PR 3c's `MessageTemplate` work needs to consume these without reformat.

7. **Constitution VIII discipline note**: every catalog row (37 across the three walkers) carries a per-row §-citation that traces to the manual; sampled five with full verification. **No fabricated citations found in any catalog walker** (matches the rule-body audit's finding). The citation discipline is preserved structurally if the inline moves preserve the row's `citation` field verbatim.
