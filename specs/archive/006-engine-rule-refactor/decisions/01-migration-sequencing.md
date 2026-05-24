---
date: 2026-05-10
scope: PR 3c migration sequencing — Decisions 1, 5, 11
method: Read-only forensic against (a) architecture.md (2026-05-09 commitment),
  (b) crates/capco/src/rules.rs + rules_declarative.rs (47 registered rules),
  (c) rule-body-audit.md (purpose-row classification), (d) crates/scheme/src/
  (Scope, MarkingScheme, CategoryShape, Canonical<S>), (e) plan.md (PR 3c
  scope), (f) crates/capco/docs/CAPCO-2016.md (citation re-verification per
  Constitution VIII).
authors: opus-architect (parallel decision-point analysis 1 of 4)
---

# PR 3c Migration Sequencing Decisions

PR 3c is the keystone commit that retires `FixProposal` from the rule-API surface
and lands `FixIntent<S>` + the bag-of-tokens architectural shape (architecture.md
2026-05-09). Three sequencing questions need to lock before commits start landing.

## Decision 1 — Beachhead vs. renderer-first vs. buckets-first

### PM's stated lean

> Renderer-first, with a 2–3-rule conflicts/requires beachhead landed first to
> validate the emission types (`FactAdd`/`FactRemove`/`Recanonicalize`) before
> committing to renderer body.

### Evidence

**Form-rule patterns (the 16 rules + walker E060 the audit identifies):**

- **E001** (`crates/capco/src/rules.rs:369-477`, `PortionMarkInBannerRule`): walks
  `attrs.token_spans` filtered to `TokenKind::DissemControl`, indexes
  position-parallel into `attrs.dissem_controls`, and constructs a
  `FixProposal` with `original: portion.to_owned()` and `replacement:
  banner_abbrev.to_owned()` per token. The fix is span-local (one token), but
  every fix string is a redundant copy of the renderer's banner-form choice —
  `marque_ism::marking_forms::portion_to_banner` (line 403) already encodes
  the canonicalization the renderer would emit by construction.

- **E002** (`rules.rs:521-682`, `MissingUsaTrigraphRule`): one rule, two
  semantically-distinct operations welded together. Lines 662-668 explicitly
  document the conflation: when USA is missing from input, the rule first
  pushes USA into `codes`, then runs `dedup_country_codes` + `canonicalize_trigraph_list`,
  then joins on `", "`. The audit (rule-body-audit.md row E002) flags this as
  the only explicit multi-purpose split required: USA-injection is a `FactAdd
  { USA, scope: rel_to }`; USA-not-first is `Recanonicalize { scope: rel_to }`.
  The rule's `0.97` confidence (line 679) is "predicated on single-pass
  canonicalization so an E002 fix does not leave behind a latent
  alphabetical-ordering violation for a second pass" (rule's own doc, line
  500-503) — i.e., it bundles the two intents to dodge a two-pass interaction
  that the renderer would resolve trivially. The bundle exists because there
  is no fact-set delta vocabulary today; it disappears in the bag-of-tokens
  model.

- **E013** (`rules.rs:2782-2925`, `DelimiterMismatchRule`): two branches in
  one rule body. The JOINT branch (lines 2799-2848) constructs replacement
  bytes via `text.replace(',', " ").split_whitespace().collect::<Vec<_>>().join(" ")`
  — pure form canonicalization. The REL TO branch (lines 2850-2925) tokenizes,
  drops keywords, joins with `", "`, builds `format!("REL TO {canonical_list}")`
  — pure form canonicalization. Both are direct in-rule encodings of the
  delimiter alphabet (`§A.6` Figure 2, `§H.3 p56`, `§H.8 p150-151` —
  re-verified at CAPCO-2016.md lines 1225, 3713-3714). The rule is the
  delimiter knowledge living in the rule layer.

- **E029** (`rules.rs:4159-4625`, `SarCompartmentOrderRule`) and **E032**
  (`rules.rs:4524-4625`, `SciSystemOrderRule`): both rebuild whole-block
  replacements by sorting structurally-typed values (`SarMarking`,
  `SciMarking`) via `sar_sort_key`, then handing the sorted struct to
  `render_sci_block` / equivalent for byte serialization. **The renderer-
  layer code already exists** (`render_sci_block`, line 4604) — these rules
  are calling renderer-shape code from rule context, which is the inversion
  architecture.md §"What rules are" rejects.

- **E052** (`rules.rs:3225-3325+`, `RelToNoDuplicatesRule`): the rule re-
  reconstructs `Vec<CountryCode>` from token-span text (lines 3293-3302),
  calls `dedup_country_codes`, then re-renders via `actual.join(", ")`.
  Pure set-canonicalization — the renderer's choice if REL TO is treated
  as a set in the projection.

**Conflicts cluster (E054-E057, the existing right-shape precedent):**

- All four (`crates/capco/src/rules_declarative.rs:1213-1442`) consume
  `violations_for(attrs, "E0XX/...")` from `CapcoScheme`, anchor a
  diagnostic span at the asserting token (RELIDO for E054/E055; ORCON for
  E056; ORCON-USGOV for E057), and emit a fix via `build_relido_removal_fix`
  — already a subtractive (`FactRemove`-shaped) operation expressed in
  span-surgery vocabulary. The audit (row E054-E057) marks all four
  `match` for fix-logic-vs-purpose alignment.

- Citations re-verified against CAPCO-2016.md per Constitution VIII:
  - **E054** `§H.8 p145` line 3585: "Cannot be used with REL TO, RELIDO,
    EYES ONLY, or DISPLAY ONLY." (NOFORN entry) — **verified**.
  - **E055** `§H.8 p154` line 3808: "Cannot be used with NOFORN or
    DISPLAY ONLY." (RELIDO entry) — **verified**.
  - **E056** `§H.8 p136` line 3363: "May not be used with RELIDO." (ORCON
    entry) — **verified**.
  - **E057** `§H.8 p140` line 3444: "May not be used with RELIDO." (ORCON-
    USGOV entry) — **verified**.

**Requires-shape precedent (E021):**

- `rules_declarative.rs:605-636`, `DeclarativeAeaNofornRule`. Citation
  `CAPCO-2016 §H.6` (broad — rule-body-audit.md row E021). Fires on
  `violations_for(attrs, "E021/aea-requires-noforn")`, emits a no-fix
  diagnostic anchored at the AEA marking token. The audit row's "natural
  shape: `FactAdd { NOFORN, scope: portion.dissem }`" — meaning E021 is
  the cleanest single-step `FactAdd` rule in the catalog, with a
  closed-vocab token (`NOFORN`), a single scope, and an existing
  declarative violation predicate.

**Architecture.md commitments (2026-05-09):**

- "Form lives [in the renderer]. **Two `ProjectedMarking`s that are
  lattice-equal render to byte-identical output**" (line 67).
- "The renderer is the single source of canonical form. There is no
  second canonicalization layer above it (no closed enum of `directive`
  variants dispatched from rules; no per-rule rebuild fragments embedded
  in `Rule::check` bodies)" (line 69).
- "[`Recanonicalize`] subsumes: Delimiter normalization (...); Sort
  canonicalization (...); Abbreviation canonicalization (...); Block
  reordering (...); Banner roll-up form (...). The renderer re-renders
  the scope. **No directive payload**; the renderer canonicalizes per
  axis and per scope by construction" (lines 130-136).
- "The pivot-type triple was always the answer ... what got mis-designed
  in PR 3c.1 was the type that flows from rules to the engine for fix
  promotion: it was modeled on input-byte surgery (closed enum with
  `Box<str>` escape hatches) instead of fact-set delta + re-render"
  (lines 144-146).

The architecture commits the project to *retiring* most of the form bucket
(rule-body-audit.md "What this implies for the next plan", lines 156-158:
"a non-trivial number of rows (E001 / E009 / E026 / S001) all collapse to
the same banner-vs-portion form choice, which is one decision in the
renderer"). Migrating form rules to a `Recanonicalize { scope }` emission
without the renderer body in place would require the rules to carry
their own canonical-form knowledge through the migration — defeating the
whole point of the architectural commitment. The renderer body must
exist before form rules can retire into it.

### Recommendation

**Renderer-first sequencing is correct. Beachhead must be 3 rules covering
2 emission shapes: E054 (FactRemove, smallest viable conflicts test) +
E057 (FactRemove, validates the dispatch over multiple Conflicts rows
sharing one scope) + E021 (FactAdd, simplest closed-vocab single-token
add).** Land the beachhead *before* renderer-trait-surface commit, not
after.

### Rationale

1. **The PM's "2-3 rule beachhead" is right; the picks need to be specific.**
   The beachhead's job is to validate `FactAdd` / `FactRemove` round-trip
   through the engine's promotion path before form-rule migration depends
   on `Recanonicalize`. E054 + E057 give two `FactRemove` instances with
   different binding partners (NOFORN vs ORCON-USGOV) — proves the
   emission type isn't accidentally specialized to one conflict family.
   E021 gives one `FactAdd` instance — proves both halves of the fact-set
   delta vocabulary are operational. Three rules; two emission shapes;
   each citation single-§ verified above; all three already wear the
   declarative-wrapper pattern (no hand-written rule logic to retire).
   Audit-record byte-identity (SC-008) is preserved because all four
   RELIDO-cluster rules already emit subtractive fixes today (rule-body-
   audit.md "Subtractive-fix pattern (RELIDO cluster, E054-E057): Already
   in the right purpose-row, already emitting the right shape").

2. **The renderer body cannot land before beachhead.** If the renderer body
   lands first and `FactRemove`/`FactAdd` aren't proven, every form-rule
   migration becomes a two-variable change at once — emit-shape correctness
   *and* renderer-canonicalization correctness — and a regression cannot be
   localized. Beachhead-first lets the form-rule wave (commits 6+) test
   only the renderer's canonicalization correctness; the emit-shape risk
   is already retired.

3. **Buckets-first is wrong.** Migrating the conflicts bucket (~9 rules) or
   the requires bucket (~6 + walkers) without the renderer in place leaves
   the form bucket (16 rules + E060) untouched — the largest mismatch
   category by count (rule-body-audit.md summary line 104: "~22 of 47
   ≈47%, driven by the form bucket"). Form rules cannot retire into a
   renderer that doesn't exist; postponing them past 3c lengthens the
   transitional state where two canonicalization layers (rule bodies +
   future renderer) coexist. That state is where citation drift,
   audit-output divergence, and SC-008 byte-identity regressions
   accumulate.

4. **Risk if emission-type design is wrong after renderer body lands.** With
   the beachhead (commits 2-3) preceding renderer body (commit 5), an
   emission-type defect in `FactAdd`/`FactRemove` is caught before the
   ~16 form-rule migration begins. Cost: revert 3 commits, redesign,
   relandcomplete. Without the beachhead, the defect is discovered when
   form-rule migrations start (`Recanonicalize { scope }` interactions
   with `FactRemove`-shape audit records); cost is reverting 5+ commits
   plus renderer body, which is a multi-day rollback at minimum and
   risks cascading the spec's PR sequence (plan.md §"PR sequence is 18
   PRs ... the keystone window (3a→3b→3c)").

### Tradeoffs surfaced

- Beachhead-first gives the renderer body 1-2 commits' worth of design
  feedback from the conflicts cluster *before* it lands — but it also
  means PR 3c carries cosmetic "audit-record diff" for the three
  beachhead rules across two commits (their messages and confidence
  values may shift slightly when re-coded onto `FixIntent`). The
  alternative — PM's lean as stated, "2-3 rule conflicts/requires
  beachhead" without specifying picks — risks the implementer choosing
  a beachhead from rules with citation defects (E012 wrapper `§B.1` vs
  catalog `§H.3 p55`; E015 wrapper `§B.3` vs catalog `§H.7 + §B.3.d`,
  per rule-body-audit.md "Citation defects found").
- Reasonable people may prefer the FactRemove-only beachhead (drop E021,
  use E054+E057). Argument: FactAdd is structurally simpler than
  FactRemove (additive, no token-removal-with-separator-eating mechanic
  like `compute_relido_removal_span`), so it could be deferred to PR
  3c's later commits with low risk. Counter-argument: the audit-record
  schema (`marque-1.0`, contracts/audit-record.md "schema": "marque-1.0")
  must support both shapes from day one of cutover commit 9; deferring
  FactAdd validation past commit 3 means the schema is live without
  full coverage. The 1-commit cost of E021 buys schema-completeness
  before audit cutover.

### Confidence

**High.** Evidence is concrete (specific file:line citations on form-rule
patterns; verified citations on the entire conflicts cluster; rule-body-
audit.md provides the categorical purpose-row map and the existing
mismatch percentage). What would raise confidence further: empirical
verification that PR 3c.1 (the foundation-types PR per `docs/plans/2026-
05-09-pr3c-foundation-plan.md`) has actually landed `FixIntent<S>`,
`ReplacementIntent<S>`, and the sealed `Canonical<S>` types — `Glob` for
`crates/rules/src/fix_intent.rs` returned no file, suggesting PR 3c.1
may not yet be landed or may live in a worktree branch. If 3c.1 is not
landed, the beachhead recommendation extends: PR 3c.1 (foundation
types) → 2-commit citation-hygiene sweep + emission-types pin → 3-rule
beachhead → renderer.

---

## Decision 5 — Renderer interface design (per-axis canonicalization)

### PM's stated lean

> Per-axis canonicalization functions, parameterized by `Scope` (Portion /
> Page / Document — already a primitive on `marque-scheme`).

### Evidence

**`Scope` exists and is fixed.** `crates/scheme/src/scope.rs:33-46`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Individual portion marking. Identity under projection.
    Portion,
    /// Page-level rollup (banner / CAB). Corresponds to CAPCO's
    /// per-page `expected_*` aggregate.
    Page,
    /// Document-level rollup. Usually agrees with `Page` on
    /// single-page documents.
    Document,
    /// Diff-rule context; the caller supplies a [`DiffInput`] rather
    /// than a slice of markings. See crate docs.
    Diff,
}
```

The variant set is documented as "fixed by the design doc: scheme authors
don't introduce new scopes" (line 31-32). For a renderer interface this
is exactly the granularity needed — the four form-rule clusters in the
audit map cleanly to scopes (E001/E009/S001 → Portion vs Page choice;
E029/E032/E052 → within-portion canonicalization; E031/E035/E040 banner
walker → Page projection).

**Existing trait surface — what's there and what's missing.** The
`MarkingScheme` trait (`crates/scheme/src/scheme.rs:29-155`) currently has:
- `render_portion(&self, m: &Self::Marking) -> String;` (line 151)
- `render_banner(&self, m: &Self::Marking) -> String;` (line 154)

But **no `render_canonical` method**. The `Canonical<S>` doc references
`MarkingScheme::render_canonical_cve(token, scope, vocab)` (`crates/scheme/
src/canonical.rs:107`, `:213`) and `MarkingScheme::render_canonical`
(line 114, line 558), but those are *future* signatures referenced in
doc comments — the actual trait method does not yet exist. PR 3c lands
it.

The current `CapcoScheme` impl (`crates/capco/src/scheme.rs:2210-2224`)
is degenerate:

```rust
fn render_portion(&self, m: &Self::Marking) -> String {
    // Phase A: render only the classification level — enough to
    // exercise the trait method. Full renderer is Phase B.
    match &m.0.classification {
        Some(c) => c.effective_level().portion_str().to_owned(),
        None => String::new(),
    }
}
```

Real rendering work has not started; the renderer body lands in PR 3c.

**Per-axis dispatch — what the data structure must look like.**
`Category` (`crates/scheme/src/category.rs:135-146`) already carries:

```rust
pub struct Category {
    pub id: CategoryId,
    pub name: &'static str,
    pub ordering_rank: u16,                  // §A.6 Figure 2 sequence
    pub cardinality: Cardinality,
    pub aggregation: AggregationOp,
    pub intra_ordering: IntraOrdering,        // alpha / NumericThenAlpha / FixedFirst
    pub expansion: Option<ExpansionFn>,
}
```

`ordering_rank` is the inter-category sequence; `intra_ordering` is the
within-category sort. The renderer doesn't need to invent either —
both are already declared as scheme-data and used during projection.

**Prototyping per-axis calls for 5 form rules:**

1. **E001** (banner abbrev for IC dissem) — no Scope dependency *between*
   tokens; this is `Vocabulary<S>::lookup(token).banner_form()` for
   each token in the dissem axis. The "canonicalize_axis" call is
   `axis::dissem.canonicalize(scope=Page, attrs.dissem)` → emits each
   token via the vocabulary's banner form. Per-axis fits.

2. **E002** (REL TO USA-first) — within-axis sort uses `IntraOrdering::
   FixedFirst { first: TokenId(USA), rest: Box::new(IntraOrdering::
   Alphabetical) }` (already a primitive at `category.rs:59-63`). The
   renderer reads `category.intra_ordering` and sorts the projected
   `rel_to`. Per-axis fits.

3. **E013** (JOINT space-delim, REL TO comma-delim) — this is a per-axis
   *delimiter alphabet* choice: JOINT category emits with `b' '`
   between codes, REL TO category emits with `b", "` between codes.
   Both are static per-category data. Per-axis fits — but this surfaces
   a sub-decision: `Category` doesn't currently carry an
   `intra_delimiter: &'static [u8]` field. Either add it (one `&'static`
   field, no semantic cost) or hand it via the per-axis canonicalize-fn
   closure. **Affects Decision 2: emission-type extension.**

4. **E029** (SAR per-program compartment sort) — within one axis (`sar`),
   needs to recursively canonicalize `SarMarking → programs[i] →
   compartments[j] → sub_compartments[k]`. This is a *nested* per-axis
   call: the SAR axis canonicalizer dispatches into program-level →
   compartment-level → sub-compartment-level rendering. Per-axis fits
   but argues for the renderer dispatch table operating on a recursive
   `CategoryShape` (`category.rs:163-183`, already a `#[non_exhaustive]`-
   minded recursive enum: `Product(Vec<CategoryShape>)`). **Per-axis
   fits with recursion natural.**

5. **E031/E035/E040** (banner roll-up walker) — needs *cross-portion*
   access: banner = join of portions. But this is **not** the
   renderer's job; it's the projection's job. `MarkingScheme::project(
   Scope::Page, &portions) -> Self::Marking` (`scheme.rs:131`) is the
   join-over-portions operation; the renderer receives the *already-
   joined* `ProjectedMarking` and renders it. The audit
   (rule-body-audit.md row E031/E035/E040) confirms: "lattice-property
   (banner = join of portions) ... walker splits per row: each per-
   axis row is a property test on `Lattice::join` for that axis (banner
   observed = join over portions). Rules disappear for the lattice
   axes."

   **The interface does NOT need cross-portion access on the renderer
   side.** Per-axis on `&ProjectedMarking` is sufficient.

**Switch-statement risk on `render_canonical`.** Without dispatch-table
discipline, `render_canonical` becomes a 13-arm match (one per CAPCO
category from `crates/capco/src/scheme.rs:36-49`: classification,
non-US class, joint, SCI, SAR, AEA, FGI, dissem, non-IC dissem,
declassify-on, ...). Long match arms in a single function is the
"God Object" anti-pattern (per project rules common/coding-style.md
"Long Functions"). Architecture.md explicitly rejects "no closed enum
of `directive` variants dispatched from rules" (line 69) — but a 13-
arm match in `render_canonical` itself reproduces exactly that
shape, just one layer down.

**The right pattern is data, not match.** The fix: a `&'static [(CategoryId,
fn(&ProjectedMarking, Scope, &mut dyn fmt::Write))]` table on `CapcoScheme`
sorted by `Category::ordering_rank`. `render_canonical` iterates the
table, calling each axis's fn. Each fn is its own small file
(`render_dissem.rs`, `render_rel_to.rs`, `render_sci.rs`, etc., per
common/coding-style.md "MANY SMALL FILES > FEW LARGE FILES").

### Recommendation

**Per-axis canonicalization parameterized by `Scope` is correct. The
dispatch must be a `&'static [(CategoryId, fn pointer)]` table on
`CapcoScheme`, sorted by `Category::ordering_rank` for inter-category
sequencing. The fn-pointer signature is writer-passing:**

```rust
pub trait MarkingScheme {
    // ... existing ...

    /// Render `m` in canonical form for the given scope, writing
    /// into `out`. Per-axis canonicalization functions implement
    /// the form rules (delimiter, sort, abbreviation, position).
    /// Two `ProjectedMarking`s that are lattice-equal render to
    /// byte-identical output.
    fn render_canonical(
        &self,
        m: &Self::Marking,
        scope: Scope,
        out: &mut dyn fmt::Write,
    ) -> fmt::Result;
}
```

`render_portion` and `render_banner` deprecate to default impls that
call `render_canonical` with `Scope::Portion` / `Scope::Page` and
collect into a `String`.

### Rationale

1. **`Scope` is already domain-neutral.** `crates/scheme/src/scope.rs`
   declares the four-variant fixed set. The PM's lean is direct;
   no new scope variants needed for any form rule the audit identifies.

2. **Writer-passing (not `String` return) matches Constitution II
   (zero-copy / streaming).** Every `String` allocation in the
   renderer is a heap allocation per call; writer-passing lets the
   engine pre-allocate `Vec<u8>` once and reuse it across all per-
   axis calls in one render. Also lets WASM callers pass a
   `js_sys::String` builder without intermediate `String` ownership.
   `&mut dyn fmt::Write` is the standard Rust idiom; works for `String`,
   `Vec<u8>` (via `BufWriter`), and `formatter::Formatter`.

3. **Per-axis fn-pointer table is data, not control flow.** The
   `BannerCategoryRow` walker pattern at `crates/capco/src/rules.rs:5145-
   5179` (`BANNER_CATEGORY_CATALOG`) is the existing precedent: a
   `&'static [BannerCategoryRow]` with each row carrying a `fn`-pointer
   evaluator. Reusing the same shape for `render_canonical` keeps the
   codebase one-pattern-deep:

   ```rust
   pub(crate) struct AxisRenderRow {
       pub category: CategoryId,
       pub render: fn(&CapcoMarking, Scope, &mut dyn fmt::Write) -> fmt::Result,
   }

   pub(crate) const RENDER_TABLE: &[AxisRenderRow] = &[
       AxisRenderRow { category: CAT_CLASSIFICATION,
                       render: render_axis_classification },
       AxisRenderRow { category: CAT_NON_US_CLASSIFICATION,
                       render: render_axis_non_us_classification },
       AxisRenderRow { category: CAT_SCI,
                       render: render_axis_sci },
       // ... 13 rows total, one per CAPCO category.
   ];
   ```

   `render_canonical` becomes:

   ```rust
   fn render_canonical(&self, m: &Self::Marking, scope: Scope,
                       out: &mut dyn fmt::Write) -> fmt::Result {
       let mut cats: Vec<&Category> = self.categories().iter().collect();
       cats.sort_by_key(|c| c.ordering_rank);  // §A.6 Figure 2 order
       for cat in cats {
           if let Some(row) = RENDER_TABLE.iter().find(|r| r.category == cat.id) {
               (row.render)(m, scope, out)?;
           }
       }
       Ok(())
   }
   ```

   Three lines of dispatch logic; everything else is data + per-axis fns.

4. **Per-axis fns live in separate files.** Per Constitution VII (crate
   discipline) and project-level "MANY SMALL FILES > FEW LARGE FILES",
   `crates/capco/src/render/` becomes a module directory with one file
   per axis: `render_classification.rs`, `render_dissem.rs`,
   `render_rel_to.rs` (encodes USA-first + comma-space), `render_sci.rs`
   (encodes numeric-then-alpha + hyphen + space), etc. Each file is
   ~50-100 lines, each absorbs the form knowledge of 1-3 retired rules,
   each is independently testable.

5. **`Scope` parameter handles the banner-vs-portion bifurcation.** E001
   (banner abbrev) vs E009 (portion abbrev) are not two rules; they're
   one rule (`render_axis_dissem(m, scope, out)`) reading `scope` to
   pick the form. The 4 rules E001/E009/S001/S002 retire into a single
   per-axis canonicalization function (rule-body-audit.md "What this
   implies for the next plan", lines 156-158: "all collapse to the same
   banner-vs-portion form choice, which is one decision in the
   renderer").

### Tradeoffs surfaced

- **Trait shape change.** Adding `render_canonical` to `MarkingScheme`
  is a non-trivial trait-surface edit (Constitution VII §IV: "scheme-
  adoption PR MUST NOT edit the engine crates ... If the scheme reveals
  an engine gap, the gap is fixed first in a separate PR"). The
  rule-of-the-Constitution treats this trait edit as an engine-gap PR
  that must precede the scheme-adoption work. Affects Decision 11
  sequencing: the renderer trait surface can land in PR 3c only if
  PR 3c is itself an engine PR (which it already is — `marque-rules`
  gains a `marque-scheme` dep per plan.md line 71). No additional
  separate PR is needed; just structurally distinct commits in PR 3c.

- **`fmt::Write` vs `io::Write`.** `fmt::Write` is `&str`-only, no
  byte-level write. CAPCO output is ASCII-only (per CAPCO-2016 §A.6),
  so this is fine. If a future scheme requires byte-level output
  (UTF-16 markings, binary protocols), a per-scheme writer-trait
  associated type would be needed — but that's a YAGNI for PR 3c.

- **`PageRewrite` interaction.** Page rewrites already run before
  rendering (architecture.md §"Project" lines 58-60); the renderer
  receives a *post-rewrite* `ProjectedMarking`. Some rule-body-audit
  rows (E039 NODIS/EXDIS clears banner REL TO; E053 NOFORN clears REL
  TO) become page rewrites, not renderer concerns — they apply during
  projection, not during render. This is correct and orthogonal to
  Decision 5; mentioned for completeness because the form-vs-
  page-rewrite distinction is what makes per-axis canonicalization
  *actually* per-axis (no cross-axis logic in any render fn).

### Confidence

**High** for the trait shape (per-axis, Scope-parameterized, dispatch-
table); **medium** for the writer-passing detail. The audit identifies
no form rule that needs cross-portion access (audit row E031/E035/E040
banner walker is `lattice-property`, retires to `Lattice::join` property
test — not a renderer concern). What would raise confidence further:
verifying `engine.rs::fix_inner` consumer signature for `render_canonical`
output — does it want `String`, `&[u8]`, or a writer? The `FixIntent`
contract (`contracts/fix-intent.md`) says the engine routes intents through
"`S::render_canonical::<EngineConstructor<S>>(&intent, &ctx)`" but doesn't
pin the return type. The writer-passing recommendation is the
performance-optimal default; if the engine consumer wants `String`, a 1-
line `pub fn render_canonical_string(scope) -> String { let mut s =
String::new(); render_canonical(&mut s).unwrap(); s }` helper closes the
gap.

---

## Decision 11 — PR commit shape

### PM's stated lean

> A 9–10-commit graph: (1) citation hygiene sweep → (2) emission types on
> `marque-rules` → (3) 2–3 conflicts/requires beachhead → (4) renderer
> trait surface → (5) renderer body → (6) form-bucket migration (~16
> rules + walker E060) → (7) walker decomposition (E058 27 inline + E059
> 5 inline) → (8) conflicts/requires bucket completion → (9) audit-
> schema cutover (`marque-mvp-2 → marque-1.0`) → (10) no-fix rule fix
> bodies (defer to follow-up if going lean refactor).

### Evidence

**Constitution VII §IV scheme-adoption restriction.**
`.specify/memory/constitution.md` Principle VII (and CLAUDE.md's
restated version): "A scheme-adoption PR MUST NOT edit the engine
crates (`marque-engine`, `marque-scheme`, `marque-core`, `marque-
rules`, `marque-ism`). If the scheme reveals an engine gap, the gap
is fixed first in a separate PR that lands against the corpus
regression harness, then the scheme lands."

PR 3c is *already an engine PR* (plan.md line 71: "One graph change at
PR 3c: `marque-rules` gains a `marque-scheme` dep so `FixIntent<S>`
can reference scheme-defined types"; spec.md FR-001 through FR-005,
FR-021..FR-028 all touch `marque-rules` / `marque-scheme` /
`marque-engine`). The §IV restriction does not apply to PR 3c —
this is not a scheme-adoption PR, it is an engine-keystone PR. But
the restriction applies *within* the commit graph: commits that touch
ONLY `marque-capco` (form-rule migrations, walker decomposition) MUST
land *after* commits that change engine-crate trait surfaces; otherwise
a future bisect on a regression in `marque-capco` cannot be pinned to
the right commit.

The PM's graph already respects this by sequencing emission-types (2),
trait surface (4), body (5) before form migration (6), walker (7),
bucket completion (8). The graph is structurally correct.

**The cleavage point.** Commits 1-5 are foundation work: hygiene + new
types + new trait surface + new body. Commit 6 is the first time the
new infrastructure is consumed at scale (16 form rules retire). Commits
6-8 are migration execution. Commit 9 (audit-schema cutover) is
irreversible: per `contracts/audit-record.md:24-26`, "There is **no
accept-list**; pre-cutover records are unreadable by post-cutover
binaries (FR-037 — clean break, no `marque-audit-reader` crate
scheduled)." Commits 1-5 do not require schema cutover; commits 6-10
benefit from it being already-cutover (the form-rule and walker-
decomposition diagnostics emit `marque-1.0` audit records by default).

**`render.rs` baseline (the workspace-red status PM cited).** Plan §0.7
asserts "production stays at 5 errors baseline (all in
`marque/src/render.rs`)." `crates/marque/src/render.rs` exists at
`/home/knitli/marque/marque/src/render.rs`. **I could not run `cargo
check --workspace` to verify the current baseline (no Bash tool
access).** Grep on `render.rs` for `FixIntent` / `MessageTemplate`
returned no matches — meaning if the 5 errors are about `FixIntent` /
`MessageTemplate` integration, they originate from PR 3c.1 type
introductions and need follow-through in PR 3c. Without empirical
verification, **the plan's baseline assertion must be re-checked at
commit-1 of PR 3c.**

**Audit-schema cutover (commit 9) — what triggers it.** `contracts/audit-
record.md:18-25` pins schema at `"marque-1.0"`. The current schema
(per CLAUDE.md "Recent Changes": "Audit schema: `MARQUE_AUDIT_SCHEMA`
env var pinned at build time, validated against the closed accept-list
`["marque-mvp-1", "marque-mvp-2"]`. Defaults to `"marque-mvp-2"`") is
`marque-mvp-2`. PR 3c performs a clean break — no accept-list overlap.
This means *every* commit-9-and-after audit record is incompatible with
*every* commit-8-and-before audit record. The cutover MUST land with
all migrations complete; landing it before form migration finishes
means the engine is emitting `marque-1.0` records while half the rules
still produce `FixProposal` (mvp-2-shape) → audit-record fields would
be either missing or invented.

### Recommendation

**Split PR 3c into PR 3c.A (commits 1-5: foundations) and PR 3c.B
(commits 6-10: migration execution + cutover). The cleavage falls at
"renderer body landed; migration begins" — a natural architectural
seam. Commit 9 (audit-schema cutover) lands in 3c.B as the *last* commit
before any "no-fix rule fix bodies" follow-up, NOT as part of 3c.A.**

Re-ordered graph:

| PR | Commit | Description | Rationale |
|---|---|---|---|
| 3c.A | 1 | Citation hygiene sweep (E012 §B.1→§H.3 p55; E015 §B.3→§H.7+§B.3.d; E001/E009/E011/E013 page-precision) | Pre-flight; cheapest |
| 3c.A | 2 | Emission types on `marque-rules` (`FactAdd`/`FactRemove`/`Recanonicalize`) | Foundation |
| 3c.A | 3 | 3-rule beachhead: E054 + E057 + E021 (per Decision 1) | Validates emission types |
| 3c.A | 4 | Renderer trait surface: `render_canonical` on `MarkingScheme`, dispatch-table pattern (per Decision 5) | Scheme trait edit |
| 3c.A | 5 | Renderer body: 13 axis fns under `crates/capco/src/render/` | Renderer body in place |
| **PR seam** | | **Workspace-red baseline re-pinned; foundations frozen** | |
| 3c.B | 6 | Form-bucket migration (16 rules retire into renderer; E060 walker absorbed) | Renderer is the canonicalization layer |
| 3c.B | 7 | Walker decomposition (E058 27 rows; E059 5 rows; per-row Constraint catalog) | Walker rule-IDs retire to per-row IDs |
| 3c.B | 8 | Conflicts/Requires bucket completion (~6 remaining rules: E010, E012, E014, E015, E024, E036, E037, E038, E041, E053) | Beachhead pattern propagated |
| 3c.B | 9 | Audit-schema cutover `marque-mvp-2 → marque-1.0` | Last commit before any defer follow-up |
| (defer) | 10 | No-fix rule fix bodies | Scope-creep risk; defer per PM's "if going lean refactor" |

### Rationale

1. **The cleavage is real.** Commits 1-5 are additive: new types, new
   trait method, new renderer body. They don't retire any existing rule.
   `cargo check --workspace` should pass after commit 5 with workspace-
   red baseline preserved (no rule body has migrated yet; `FixProposal`
   path still active). Commits 6-10 are subtractive: each retires
   rules and shifts callers. The two halves have different bisect
   characteristics — 3c.A regressions are trait-surface / type-shape;
   3c.B regressions are rule-behavior / audit-output.

2. **Audit cutover (commit 9) lands in 3c.B by necessity.** Per the
   evidence above (`contracts/audit-record.md:24-26`), the schema
   cutover is irreversible. Landing it in 3c.A would mean every
   commit-6-and-after had to land *before* the cutover (i.e., on top
   of a still-`marque-mvp-2` baseline), which inverts the dependency:
   the rule migrations need the new audit-record shape to express
   structural fact-set deltas correctly. The ordering "form rules
   migrate → cutover → optional no-fix follow-up" is the only one
   where each commit consumes the latest infrastructure.

3. **`render.rs` workspace-red baseline must be re-confirmed at
   commit-1.** Without empirical verification, taking the plan's "5
   errors" assertion as gospel is the same kind of stale-disclaimer
   error project memory `feedback_no_prexisting_failures.md`
   warns against. Commit-1 (citation hygiene) should re-pin the baseline
   in its commit message: "workspace-red baseline at this commit: N
   errors in marque/src/render.rs (see commit-1 description for the
   diff)". Subsequent commits assert N is preserved or shrinks; any
   commit that grows N triggers review.

4. **PR 3c size.** Even split, 3c.A is ~5 commits and 3c.B is ~5
   commits (excluding deferred no-fix bodies). The combined PR is
   ~10 commits. CLAUDE.md "Active Technologies" + "Recent Changes"
   shows the project's per-PR commit count typically runs 3-7;
   10 in one PR is large. The split is also a *reviewability*
   recommendation, not just a bisect-safety one — a reviewer can
   approve 3c.A on the type-system / trait-surface / contract correctness
   axis, then approve 3c.B on the rule-behavior / audit-output / corpus-
   regression axis. Mixing the two review modes in one PR means each
   reviewer carries the full context for both, which compounds the
   risk per CLAUDE.md "code-review.md" complexity ladder ("Functions
   are focused (<50 lines); Files are cohesive (<800 lines)" — applied
   at the PR level rather than the file level).

5. **Constitution VII §IV does NOT require splitting further.** The
   restriction applies to scheme-adoption PRs, not to engine PRs that
   change scheme-trait surfaces alongside scheme-adoption code. PR
   3c is an engine PR (plan.md "One graph change at PR 3c"), so
   commits 4-5 (trait surface + body) and commits 6-8 (rule migration)
   can land in the same PR family without invoking the §IV "engine
   gap fixed first" workflow.

### Tradeoffs surfaced

- **Single-PR alternative.** A single PR with all 10 commits lands all
  the value at once and avoids inter-PR coordination overhead. Cost:
  larger blast radius if any commit needs revert; harder to bisect a
  regression to commit; bigger ask of reviewers. The split-PR
  recommendation accepts +1 PR roundtrip in exchange for clearer
  review boundaries and faster bisect.
- **Commit 10 deferral.** The PM's lean already mentions "defer to
  follow-up if going lean refactor". Recommendation accepts the
  deferral. The no-fix rules (E021/E024/E036/E037/E038/E041 — audit
  rows: "none-needed today (Error, no fix)") gain real fix proposals
  *as a property of the migration* once the structural shape lands;
  no separate commit-10 work is needed for them. Genuine no-fix rules
  (E005 declassify-misplaced, S005/S006 rel-to-uncertain) stay no-fix
  by design (architecture.md §"Notable findings": "true `Constraint::
  Custom` candidates"). Commit 10 as written is genuinely follow-up.
- **The 3c.A→3c.B coordination on `FixProposal` retirement.** During the
  PR seam (between 3c.A merge and 3c.B start), `FixProposal` and
  `FixIntent` coexist; rules use `FixProposal` and the engine maps to
  `FixIntent` internally. This is the same shape PR 3c.1 already ships
  (per `docs/plans/2026-05-09-pr3c-foundation-plan.md` line 30: "PR
  3c.1 ships ~2-3K LOC, mostly new files. It does NOT touch any
  existing rule, the engine promotion path, the audit schema, or any
  existing test ... `FixProposal` and `Diagnostic.message: Box<str>`
  continue to coexist with the new `FixIntent<S>` and `Message`
  types"). Coexistence is acceptable as a transitional state; what's
  not acceptable is shipping it as a *long-lived* state. Bounding
  the seam to one PR-roundtrip prevents accretion.

### Confidence

**Medium-High.** Commit-graph shape and seam location are concrete
(architecture.md, audit-record.md, plan.md, Constitution VII §IV all
align on the recommendation). What I could not verify with the codebase
alone:

- Live `cargo check --workspace` error count. The "5 errors in
  marque/src/render.rs baseline" is plan-asserted but not currently
  empirically pinned. Recommendation: re-pin at commit-1 of 3c.A
  before any other work proceeds.
- Whether `tools/citation-lint/` (PR 0.5) has actually landed and is
  enforcing. If it has, commit-1 is a no-op for citation hygiene
  (the lint catches drift mechanically). If it hasn't, commit-1
  must include manual verification of every § citation in the audit
  defect list (E012, E015, E001, E009, E011, E013) per Constitution
  VIII.

---

## Cross-decision interactions

1. **Decision 5's interface shape forces Decision 1's beachhead to NOT
   include any form rule.** Per-axis canonicalization is the renderer's
   job; rules emit `Recanonicalize { scope }`. Form rules cannot
   beachhead the emission types because their emission type is the
   one that depends on the renderer body. The beachhead must be
   conflicts (FactRemove) + requires (FactAdd) only. Decision 1's
   recommendation (E054 + E057 + E021) is consistent with this.

2. **Decision 11's split sequence depends on Decision 1's beachhead
   landing in 3c.A.** Commit 3 (beachhead) is in PR 3c.A. If Decision
   1 had landed buckets-first instead, the entire conflicts cluster
   would have to fit in 3c.A (commit 3 alone), inflating 3c.A from
   ~5 commits to ~6+. Beachhead-first keeps the foundation PR small.

3. **Decision 5's writer-passing signature affects Decision 11's
   commit-9 schema cutover.** The audit record's `replacement` field
   shape (`contracts/audit-record.md:48-60`: `"discriminant": "strict"
   | "decoder"`, `"canonical": { "source": "cve" | "open_vocab", ... }`)
   is independent of how `render_canonical` writes its output — the
   audit record stores the rendered bytes' BLAKE3 digest plus
   structural metadata, not the bytes themselves. Writer-passing vs
   `String`-return is a perf/zero-copy choice, not an audit-shape
   choice. Commit 9 is unaffected.

4. **Renderer body absorbing E058's class-floor walker (Decision 5
   side note) interacts with Decision 11 commit 7 (walker
   decomposition).** Audit row E058 documents 27 rows mixing floor
   (FactAdd) with ceiling (FactRemove or admonition). The renderer
   doesn't absorb these — they're `requires`-shaped, not `form`-shaped.
   Walker decomposition (commit 7) is constraint-catalog work, not
   renderer work. Decision 5's per-axis renderer scope does *not*
   extend to E058/E059; commit 7 stays subtractive against the rule
   catalog, not the renderer body.
