<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Quickstart: Engine + Rule Architecture Refactor

**Branch**: `006-engine-rule-refactor` | **Date**: 2026-05-03
**For**: contributors landing PRs against this branch.

The refactor lands as 18 ordered PRs (PR 0 → PR 10, with sub-letter
splits). This quickstart orients you to where things live and what
changes per PR. Read this before opening a PR against the branch.

---

## TL;DR — what's changing

- **Pivot type splits 1 → 3**: `IsmAttributes` becomes
  `ParsedAttrs<'src>` (parser output) + `CanonicalAttrs` (post-canonical,
  what rules read) + `ProjectedMarking` (page-level rollup output, what
  banner-validation rules read).
- **Rules emit `FixIntent<S>` not `FixProposal`**: the engine renders
  intents into `Canonical<S>` and promotes to `AppliedFix`. Rules
  cannot construct `Canonical<S>` directly anymore.
- **Audit records become content-ignorant by type**: `AppliedFix.original`
  becomes a `Span` (no bytes); `Diagnostic.message` becomes
  `Message { template, args }` (closed enum, no `format!`); `Canonical<S>`
  carries provenance (CVE-typed vs. open-vocab-typed).
- **Page rollup goes through `Scope::Page` projection**: `PageContext`
  deletes at PR 6 cutover; lattice projection is the source of truth.
- **Pass-split**: rules declare `Phase::Localized | WholeMarking` at
  registration; engine re-parses between passes; `R002` for
  re-parse-failure cases.
- **Audit schema lands `marque-1.0` via two atomic steps**:
  `mvp-2 → mvp-3` at PR 3c.B Commit 10 (`FixProposal` retired;
  audit envelope reshapes to a structural `proposal: FixIntent |
  TextCorrection` sub-object, closing the G13
  audit-content-ignorance channel); `mvp-3 → 1.0` at PR 3c.2.D
  (the PR 3c.2 carve-out per the 2026-05-14 PM decision). The
  `mvp-N` naming retires at the second step. Clean break post-`1.0`
  — no reader compat, no `mvp-N` shapes accepted by the build-time
  validator.
- **Rule IDs stay single-field through 006**: `RuleId::new("E###")` is
  the surface that ships at PR 10 stability-freeze (FR-049). The
  originally-planned `(scheme, predicate-id)` 2-tuple defers to a
  dedicated post-PR-10 PR per the PR 3c.2 carve-out — the freeze
  begins at PR 10 merge, and the 2-tuple change requires the freeze
  to be unfrozen for its own PR.
- **Three new CI lints**: citation (FR-018), masking-pin (FR-039),
  promote-callsite (FR-040). All AST-based, all `tools/`.

---

## Read first

In order:

1. **`spec.md`** — the user-observable correctness properties (FRs, SCs).
2. **`plan.md`** — Constitution check, project structure (per-crate scope of change), bench harness layout.
3. **`research.md`** — tactical decisions resolved before PR 0 begins.
4. **`data-model.md`** — type-system shapes for new entities.
5. **`contracts/`** — three contracts (rule-emission API, audit-record shape, engine pipeline).

The **source of truth** for the PR sequence is
`docs/plans/2026-05-02-engine-refactor-consolidated.md`. That document
is post-murder-board; substantive scope is resolved. The PR table at
§4 of the consolidated plan is what you implement against.

---

## PR sequence at a glance

```text
PR 0     — static_assertions (Send+Sync); masking-pin lint; promote-callsite lint
PR 0.5   — citation lint (8A) + F.1 corpus skeleton; runs against existing catalog
PR 0.6   — preemptive citation-defect fix (resolves what 0.5 surfaces)
PR 1     — single-pass forward splice (already landed in PR #278)
PR 2     — Vocabulary::shape_admits at parser sites; FGI silent-skip → None;
            FgiMarker::SourceConcealed | Acknowledged discriminant
PR 3a    — KEYSTONE-1: pivot type split + from_parsed_unchecked adapter
PR 3b    — KEYSTONE-2: #263 rule collapse — qualitative per-sub-PR gate
            (re-sequenced 2026-05-07 per
            docs/plans/2026-05-07-pr3b-consultation-verdict.md;
            PR-3b-proper numeric band retired 2026-05-07 — expected
            ~38–44 post-3b; end-state target ~10 surviving rules
            across stages 1–4 per plan.md D13 addendum)
PR 3c    — KEYSTONE-3: FixReplacement discriminant; Canonical sealing;
            decoder open-vocab lockout; FixIntent<S> rule API; rule-ID retire;
            audit cutover marque-mvp-2 → marque-1.0
PR 3.7   — LATTICE GATE: fill in lattice design doc §§2–8; resolve §10 open
            questions; cross-axis dominance fixtures
PR 4     — Per-category Lattice impls + property tests; CapcoMarking::join
            PageContext delegation deleted (clean break)
PR 5     — expected_classification → Option; Us hardcode at scheme.rs:365 deleted;
            FGI render-canonical drops redundant FGI when trigraph present
PR 6     — Scope::Page projection drives Engine::lint; PageContext deleted;
            three commits 6a/6b/6c (gate / bench / cutover)
PR 7     — phase-tagged rules; engine re-parses between passes; R002 diagnostic;
            E003 confidence with FeatureId::PrecedingFixPenalty
PR 8     — decoder prose null hypothesis priors (third problem class; not closure)
PR 9a    — parser separator spans
PR 9b    — 7B dissem_us/dissem_nato; banner-validation migration to
            ProjectedMarking
PR 9c    — ATOMAL/BOHEMIA; NATO-portion declarative Constraint
PR 10    — F.1 corpus gate maturation; 8C vendored-source registry declarative
```

---

## Where do I work?

Per-crate change index (more detail in `plan.md` §Project Structure):

| Crate | Touched in PR(s) |
|---|---|
| `marque-ism` | PR 3a (pivot split), PR 3c (Canonical, MessageTemplate) |
| `marque-core` | PR 2 (parser shape_admits) |
| `marque-rules` | PR 0 (Send+Sync), PR 3c (FixIntent, AppliedFix v2, Diagnostic v2), PR 7 (Phase) |
| `marque-scheme` | PR 0 (Send+Sync recognizer), PR 3c (CanonicalConstructor sealed trait) |
| `marque-capco` | PR 0.6 (citation fix), PR 2 (FgiMarker discriminant), PR 3b (rule collapse), PR 4 (Lattice impls), PR 5 (foreign banner), PR 7 (phase tags), PR 9 (separators, dissem_us/nato, banner-val migration) |
| `marque-engine` | PR 1 (splice; landed), PR 3c (carve-out delete, schema cutover), PR 6 (Scope::Page cutover), PR 7 (pass split, R002) |
| `marque-config` | (no scope change) |
| `marque-wasm` | exercised by SC-008 parity at PR 6 / PR 7 |
| `tools/masking-pin-lint/` | PR 0 (NEW) |
| `tools/promote-callsite-lint/` | PR 0 (NEW) |
| `tools/citation-lint/` | PR 0.5 (NEW) |

---

## How to add a new rule (post-PR-3c)

```rust
use marque_capco::CapcoScheme;
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate,
    Phase, Rule, RuleContext, RuleId, Severity, SmallVec,
};
use marque_scheme::{capco, Citation, SectionLetter};
use marque_scheme::fix_intent::{FactRef, ReplacementIntent};
use marque_scheme::Scope;
use marque_ism::{CanonicalAttrs, TokenKind};

pub struct MyDissemConflictRule;

// `cited_authorities` returns a `&'static [Citation]`; declare the
// authority slice as a module-level const so the F.1 corpus-fidelity
// gate (`crates/capco/tests/citation_fidelity.rs`) sees a stable
// reference.
const MY_RULE_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 150)];

impl Rule<CapcoScheme> for MyDissemConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E###")        // Single-field; see FR-049 freeze note above.
    }

    fn name(&self) -> &'static str {
        "my-dissem-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn phase(&self) -> Phase {
        Phase::WholeMarking        // or Phase::Localized; see contracts/fix-intent.md.
    }

    fn cited_authorities(&self) -> &'static [Citation] {
        MY_RULE_AUTHORITIES
    }

    fn check(
        &self,
        attrs: &CanonicalAttrs,
        ctx: &RuleContext<'_>,
    ) -> Vec<Diagnostic<CapcoScheme>> {
        // Predicate: bail early when the rule has nothing to say.
        if !my_predicate(attrs) {
            return vec![];
        }

        // Locate the offending token's span inside the candidate. The
        // engine populates `ctx.candidate_span` with the marking-scope
        // span (full portion or banner); narrow it to a sub-token span
        // when the diagnostic should point at a single token.
        let offending_span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl)
            .map(|t| t.span)
            .unwrap_or(ctx.candidate_span);

        // Closed-template message — no `format!`, no free-form text.
        // `MessageArgs::default()` populates only the fields the
        // template's args contract names; the rest stay `None`.
        let message = Message::new(
            MessageTemplate::ConflictsWith,
            MessageArgs {
                token: Some(my_dominated_token()),
                expected_token: Some(my_dominating_token()),
                ..MessageArgs::default()
            },
        );

        // Typed citation. Prefer the const-fn helpers — `capco(letter,
        // sub, page)` for `§<L>.<sub> p<page>`, `capco_section(letter,
        // page)` for bare-section `§<L> p<page>` (e.g. §F), and
        // `capco_table(letter, sub, table, page)` for the table form.
        // Each panics at const-eval on a zero argument; citation-lint
        // additionally verifies the (section, page) pair resolves
        // against `crates/capco/docs/CAPCO-2016.md`.
        let citation = capco(SectionLetter::H, 8, 150);

        // Structural `FixIntent`. The engine renders the canonical
        // bytes via `MarkingScheme::apply_intent` +
        // `render_canonical` at promotion time — the rule never
        // computes a byte-precise replacement (G13 / Constitution V).
        let fix = FixIntent {
            replacement: ReplacementIntent::fact_remove(
                FactRef::Cve(my_dominated_token()),
                Scope::Portion,
            ),
            confidence: Confidence::strict(0.95),
            feature_ids: SmallVec::new(),
            message: message.clone(),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };

        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            offending_span,
            ctx.candidate_span,
            message,
            citation,
            fix,
        )]
    }
}

// Predicate + token-lookup stubs (per-rule; the real shape consumes
// `CanonicalAttrs` fields and `crate::scheme::TOK_*` const TokenIds).
fn my_predicate(_: &CanonicalAttrs) -> bool { false }
fn my_dominated_token() -> marque_scheme::TokenId { marque_scheme::TokenId(0) }
fn my_dominating_token() -> marque_scheme::TokenId { marque_scheme::TokenId(0) }
```

**What you cannot do**:
- Construct `Canonical<S>` directly: the sealed constructor is engine-only.
- Construct `AppliedFix<S>`: `__engine_promote` is engine-only in production code (Constitution V Principle V; FR-040 lint enforces).
- Use `format!` over input bytes in `Message`: `Message::new(template, args)` is the only public constructor; `MessageArgs` field set is closed. The compile-fail tests at `crates/rules/tests/message_no_freeform_ctor.rs` pin every absent constructor.
- Cite a fabricated section: citation-lint (FR-018) resolves every `§X.Y pNN` against `crates/capco/docs/CAPCO-2016.md` at CI time.

**Add a corpus fixture** at `tests/corpus/...` plus declare the
authority in `cited_authorities()` so the F.1 corpus-fidelity gate
at `crates/capco/tests/citation_fidelity.rs` covers the predicate
against the canonical example from the cited passage (FR-019).

---

## How to verify a citation lints clean

```bash
cargo run --manifest-path tools/citation-lint/Cargo.toml -- crates/capco/src/rules.rs
```

The lint inspects `citation:` fields, `message:` strings,
`constraint_label:` strings, and doc-comment `§X.Y` references.
Failures point at the offending source line. Pre-conditions for green:

- `§X.Y` exists in `crates/capco/docs/CAPCO-2016.md` (subsection
  resolves against the section's own subsection table).
- `X` is in `{A, B, C, D, E, F, G, H}` (normative range; §I/§J/§K
  are non-normative and rejected).
- `pNN` is within the cited subsection's page span (not just the
  document range).
- No bare `§NN`-form (a digit without a section letter is rejected).
- A bare-letter `§X` is permitted only when `X` has no numbered
  subsections (e.g. `§F p35`). For sections with subsections (§A–§E,
  §G, §H), the subsection number is required.
- Legacy `line NNNN` citation form is rejected (retired in commit
  `b340bec`; page numbers only).

The full F.1 gate at `crates/capco/tests/citation_fidelity.rs`
additionally requires that every cited authority has a corpus fixture
exercising the predicate (FR-019).

---

## Bench discipline (per FR-029..FR-033, R-5)

Pre-refactor baselines are captured at PR 0 as
`benches/baselines/2026-05-pre-refactor.json`. Subsequent PRs assert
against this baseline.

Run a bench locally:

```bash
cargo bench --bench fix_throughput
cargo bench --bench lint_latency
cargo bench --bench fix_10kb            # PR 7+
cargo bench --bench lint_100kb_multipage # PR 6+
```

The CI gate at `scripts/bench-check.sh` reads the baseline JSON and
fails the PR if:
- Any bench's mean regresses > 5%.
- Any bench's p99 regresses > 5%.
- `fix_throughput` linear-scaling R² drops below 0.9.
- Multi-page projection latency exceeds `PageContext` baseline + 10%.

If your PR triggers a regression, the discipline (FR-033) is to back
out the originating change, not to relax the baseline.

---

## Keystone-window discipline (PRs 3a, 3b, 3c)

The keystone is the highest-blast-radius merge in the sequence.
Discipline:

1. **Each PR independently revertable** — 3a, 3b, 3c can each be
   reverted alone without breaking the workspace build (SC-014).
2. **CI matrix during the keystone window** runs corpus regression
   sweep × {3a-only, 3a+3b, 3a+3b+3c} = 3 runs to verify each
   subsequence is correct in isolation.
3. **Test-fixture migration is mechanical** at PR 3a (sed-replaceable
   `IsmAttributes { ... }` → `CanonicalAttrs::from_parsed_unchecked(...)`).
   PR 3c removes the adapter; fixtures migrate again to direct
   `CanonicalAttrs` consumption (mostly via `FixIntent` rule API).

If you're touching test fixtures during the keystone window, expect to
re-touch them at the next sub-PR. The reshape is intentional.

---

## Lattice §-resolution gate (PR 3.7)

PR 4's lattice impls cannot land until PR 3.7 fills in
`docs/plans/2026-05-01-lattice-design.md` §§2–8 with:

1. §-citations to `crates/capco/docs/CAPCO-2016.md`.
2. Formal join semantics (precondition / postcondition functional
   form, not prose).
3. Worked examples (≥2 per category, including edge cases the
   §-citation calls out).
4. Property-test fixture file/test names.
5. Cross-axis fixtures (FOUO eviction, FGI banner roll-up, SCI
   cross-system canonicalization, AEA exemption commingling).

PR 3.7 acceptance requires a named reviewer (in the PR description)
who confirms each category's worked examples by hand against the
§-citation. Default owner: consolidated-plan author or named
successor; default deadline: 2 weeks from PR 3c merge.

If PR 3.7 stalls, PRs 4–10 stall. This is the cost of taking the
gate seriously.

---

## Masking-pin discipline (FR-039, I-16)

If you write a test that pins `with_recognizer(StrictRecognizer)`,
add one of these comments within 5 lines:

```rust
// MASKING-PIN: tracks #258 — remove when issue closes.
// or
// INTENTIONAL-STRICT: SC-001 strict-latency baseline must be measured against the strict path only.
```

The AST-based lint at `tools/masking-pin-lint/` enforces:
- Every pin carries one of the two markers.
- MASKING-PIN tracked-issue numbers are checked against the GitHub
  API; if the issue has closed (including `closed_as_duplicate_of`
  chains), the pin must be removed in the same PR that closes the
  issue, with a regression test demonstrating fix necessity.
- Cascade-close-via-meta-issue is flagged at lint time.
- A third masking pin (beyond the two inventoried — `corpus_accuracy.rs`
  → #258 and `core_error_isolation.rs` → #257) requires team-review
  approval.

---

## Promote-callsite discipline (FR-040, I-15)

Don't call `AppliedFix::__engine_promote` or
`EnginePromotionToken::__engine_construct` outside `Engine::fix_inner`
in production code.

The carve-out for `#[cfg(test)]` / `tests/` / `dev-dependencies`-gated
test-utility crates is enumerated per call site in the AST-based
promote-callsite lint at `tools/promote-callsite-lint/`. Each test
call site MUST carry an inline comment naming the carve-out (e.g.,
`// Test-fixture carve-out per Constitution V`).

If you're authoring a test that needs to fabricate an `AppliedFix`
fixture, the carve-out applies — add the comment, scope the call to
test code only, never commingle the fabricated record with engine
output.

---

## Constitution check (per PR)

Each PR carries a Constitution Check in the PR description per
consolidated plan Appendix D. The check enumerates which Constitution
principles the PR exposes and how it preserves them. The constitution
gates are PASS for every PR per `plan.md` §Constitution Check;
violations require a Complexity Tracking entry justifying the
deviation (none expected for this refactor).

---

## Where to ask

- Spec questions: `specs/006-engine-rule-refactor/spec.md`.
- Plan or PR-sequence questions: `docs/plans/2026-05-02-engine-refactor-consolidated.md` and `plan.md`.
- Lattice math questions: `docs/plans/2026-05-01-lattice-design.md` (gate-filled by PR 3.7).
- Constitution questions: `.specify/memory/constitution.md`.
- Citation questions: `crates/capco/docs/CAPCO-2016.md` is the single source of truth.
