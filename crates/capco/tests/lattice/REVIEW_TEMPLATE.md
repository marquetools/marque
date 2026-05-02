<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Per-category lattice review template

**Use:** the PR 3.7 reviewer fills out one copy of this template per
category in `CapcoScheme::categories()` before signing off PR 4.
Sign-off is **structural**, not narrative — the reviewer attests to
having executed the adversarial fixture pack and observed it fail
against the synthetic violation, not to having read the section.

This file is the structural prevention of the gate-as-stub failure
mode named in `docs/plans/2026-05-02-engine-refactor-consolidated.md`
§11 — a tired reviewer can sign off a fillable acceptance checklist
without resolving the underlying design questions, reproducing the
previous lattice attempt's "skimmed complexity, ended up bandaged"
outcome. The template binds the reviewer with executable evidence.

**Authority:**
- `docs/plans/2026-05-01-lattice-design.md` — design doc and gate.
- `docs/plans/2026-05-02-engine-refactor-consolidated.md` §11.1 —
  required deliverables (the §3-shape bar).
- `docs/plans/2026-05-02-engine-refactor-consolidated.md` §6 Layer 1
  + Layer 3 negative-control discipline.

---

## Reviewer attestation

- **Category under review**: `<MarkingClassification | DissemSet | SciSet | SarSet | FgiSet | NatoControlSet | DeclassifyOn>`
- **Reviewer (named in PR description)**: `<github-handle>`
- **Date of review**: `<YYYY-MM-DD>`
- **PR under review**: `<PR number / link>`

---

## Section 1 — §-citations to `crates/capco/docs/CAPCO-2016.md`

For each citation in the lattice doc section under review, the
reviewer confirms:

| Citation | Verified against `CAPCO-2016.md`? | Citation accurately reflects the passage? | Citation in normative range §A–§H? |
|----------|-----------------------------------|-------------------------------------------|-----|
| `§<X.Y>` p`<NN>` | `[ ]` | `[ ]` | `[ ]` |
| ... | `[ ]` | `[ ]` | `[ ]` |

If any cell is unchecked, list the issue and the resolution path:

```
- <citation>: <issue> → <resolution> (PR / commit)
```

A fabricated, hallucinated, misattributed, or silently-drifted
citation is a Constitution VIII correctness defect — same severity
as a wrong predicate. The reviewer does **not** carry forward "the
citation lint passed" as substitute for re-verification at review
time.

---

## Section 2 — Formal join semantics

The lattice doc section under review states the join function as a
function with preconditions and postconditions, not prose.

- **Function signature**: `<paste from doc>`
- **Preconditions on inputs**: `<paste>`
- **Postconditions on output**: `<paste>`
- **Cross-axis interactions named**: `<list, or "none if in-category only">`

Reviewer confirms:

- `[ ]` Preconditions are testable (a property test can construct
  inputs that satisfy / violate them).
- `[ ]` Postconditions are testable (a property test can verify
  them on the output).
- `[ ]` Cross-axis interactions, if any, name the dominating
  category and the §-citation governing the interaction.

---

## Section 3 — Worked examples

The lattice doc section under review carries at least two non-
trivial worked examples, including edge cases the §-citation
calls out.

For each example:

| # | Inputs (a, b) | Expected join result | §-citation for the edge case (if any) | Reviewer hand-verified against CAPCO-2016? |
|---|---------------|----------------------|---------------------------------------|--------------------------------------------|
| 1 | | | | `[ ]` |
| 2 | | | | `[ ]` |
| ... | | | | `[ ]` |

Reviewer confirms:

- `[ ]` Each example terminates correctly per the §-citation passage.
- `[ ]` Examples cover at least one edge case the §-citation
  explicitly calls out (not just the canonical happy path).
- `[ ]` If the join algorithm is non-trivial (e.g., greedy
  size-descending decomposable-group re-fold), the algorithmic
  sketch is present and matches the worked examples.

---

## Section 4 — Property-test fixtures (in-category laws)

The lattice doc section under review names ≥3 fixtures per
in-category law (assoc / comm / idem / identity-with-bottom).

| Law | Fixture file path | Fixture test name |
|-----|-------------------|-------------------|
| Associativity | | |
| Commutativity | | |
| Idempotency | | |
| Identity-with-bottom | | |

Reviewer confirms:

- `[ ]` Each fixture exists in the repo (not just named in the doc).
- `[ ]` `cargo test -p marque-capco --test category_lattice_laws -- <category>::<law>` runs and passes against the lattice impl.
- `[ ]` Removing the impl (or substituting `BrokenLatticeFor<Category>`) causes the corresponding test to fail (negative-control discipline per consolidated plan §6 Layer 3).

---

## Section 5 — Cross-axis adversarial fixture pack

Four cross-axis interactions are load-bearing per consolidated plan
§11.1 item 6: FOUO eviction, FGI banner roll-up (#276), SCI cross-
system canonicalization, AEA exemption commingling. PR 3.7 ships
the adversarial pack — `BrokenLatticeFor<Category>` synthetic-
violation impl + property test asserting the predicate fails
against it.

Reviewer **runs the pack** and confirms:

| Cross-axis fixture | Adversarial impl present? | Property test fails against the broken impl as expected? | `cargo test` invocation that demonstrates this |
|--------------------|---------------------------|----------------------------------------------------------|------------------------------------------------|
| FOUO eviction by classification > U | `[ ]` | `[ ]` | `cargo test -p marque-capco --test cross_axis_dominance -- fouo_classification_eviction` |
| FOUO eviction by non-FD&R dissem | `[ ]` | `[ ]` | `cargo test -p marque-capco --test cross_axis_dominance -- fouo_non_fdr_eviction` |
| FGI banner roll-up (#276) | `[ ]` | `[ ]` | `cargo test -p marque-capco --test cross_axis_dominance -- fgi_banner_rollup` |
| SCI cross-system canonicalization | `[ ]` | `[ ]` | `cargo test -p marque-capco --test cross_axis_dominance -- sci_cross_system` |
| AEA exemption commingling with classification | `[ ]` | `[ ]` | `cargo test -p marque-capco --test cross_axis_dominance -- aea_classification_commingling` |

> **A test that passes today is the dominant failure mode.** The
> negative-control assertion (impl is broken → property fails) is
> what makes the property test load-bearing. If a row above passes
> the impl test but the broken-impl test does NOT fail, the property
> predicate is the bug — block the PR until the predicate is
> tightened.

---

## Section 6 — Open-question resolution (§10 of lattice doc)

For every `Status: open_gate` item in §10 of the lattice doc that
relates to this category, the reviewer confirms PR 3.7 closed it:

| §10 item # | Section | `open_gate:` criteria | Closed in PR 3.7 by | §-citation |
|------------|---------|-----------------------|---------------------|------------|
| | | | | |

Reviewer confirms:

- `[ ]` Every `open_gate:` item touching this category has an
  explicit decision and §-citation.
- `[ ]` No `open_gate:` items remain unresolved — failure to close
  blocks PR 4.
- `[ ]` Items tagged `scope_cut:` are intentional and remain
  documented as scope cuts (do not block PR 4).

---

## Section 7 — Reviewer sign-off

By signing below, the reviewer attests:

1. Every checkbox above is checked, OR every unchecked box has an
   explicit issue + resolution path documented in the corresponding
   section.
2. The reviewer **executed** the adversarial fixture pack
   (Section 5) and observed each property test fail against the
   `BrokenLatticeFor<Category>` impl. The sign-off is not based on
   reading the test names; it's based on observing the failures.
3. The reviewer hand-verified at least one §-citation per category
   section against `CAPCO-2016.md` at the offset cited (Section 1) —
   not against an extracted summary, the original.
4. The reviewer is named in the PR description and is accountable
   for the sign-off under Constitution VIII.

**Reviewer signature**: `<github-handle>` `<YYYY-MM-DD>`

**PR description link confirming attestation**: `<URL>`
