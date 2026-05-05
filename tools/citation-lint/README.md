# citation-lint

AST-based CI lint that enforces **FR-018 citation fidelity** for the
marque engine refactor (`specs/006-engine-rule-refactor/`).

Constitution Principle VIII (Authoritative Source Fidelity): every
`§X.Y pNN` reference in marque source must resolve to a real passage
in the vendored authoritative source. A wrong citation is a
correctness defect of the same severity as a wrong predicate.

## What this lint enforces

For every `§X.Y` (and optional `pNN` / `pp NN–MM`) reference in
marque source code, the lint asserts:

1. The section letter is in the **normative range** (A–H).
   §I (history), §J (examples), and §K (acronyms) exist in the
   manual but are **not valid citation targets** — see
   [`correctness.md` CHK036](../../specs/006-engine-rule-refactor/checklists/correctness.md).
2. The subsection number resolves in the source's table of contents.
3. The cited page (or page range) falls inside the cited
   subsection's actual page span (CHK038 — both endpoints of a
   range must resolve).
4. The cited page does not exceed the document's max page.
5. **No retired `line NNNN` form**. The project retired
   line-anchor citations in commit `b340bec` — page numbers only.
6. **No doubled-page-anchor form** (`pNN-MM pMM`). FR-020 known
   defect class.

The lint scans **every `*.rs` file under `crates/*/src/`** in the
workspace, capturing citations from:

- `citation:` struct field literals (the canonical place).
- `message:` struct field literals.
- `constraint_label:` struct field literals.
- Any string literal containing `§`.
- `///`, `//!`, and `#[doc = "..."]` doc-comment attributes.

Citation extraction uses `syn::visit::Visit` over the parsed AST.
Regex-only scanning would false-positive on `cfg!`-gated code, on
strings constructed via `format!` argument lists, and on doc-test
code fences.

## Why this crate is out-of-workspace

Per **Constitution III** (WASM safety), the WASM-safe crate set
(`marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`,
`marque-capco`) MUST compile to WebAssembly without modification and
MUST have zero runtime I/O dependencies. `pulldown-cmark` and `syn`
are not WASM-relevant runtime deps in this codebase, but the parallel
`tools/masking-pin-lint/` and `tools/promote-callsite-lint/` crates
established the out-of-workspace pattern for CI lint binaries;
mirroring it keeps the policy uniform.

The crate's `Cargo.toml` includes an empty `[workspace]` table at the
bottom to prevent cargo's parent-directory walk from re-attaching the
crate to the parent workspace.

Verify with `cargo metadata --format-version 1 \
--manifest-path Cargo.toml | jq '.workspace_members'` from the repo
root: `citation-lint` MUST NOT appear in the list.

## Invocation

```bash
cargo run --manifest-path tools/citation-lint/Cargo.toml -- <workspace-dir>
```

Optional flags:

- `--catalog-path <path>` — override the defect-catalog output path
  (default `<workspace>/docs/refactor-006/citation-defect-catalog.md`).
- `--no-catalog` — skip writing the catalog file (local diagnostic
  runs).

Exit codes:

- `0` — no defects.
- `1` — at least one defect.
- `2` — lint itself failed (could not read source, parse error, etc.).

## Defect classes

| Class | Meaning |
|-------|---------|
| `bare-section` | `§NN` form without subsection letter |
| `letter-only-needs-subsection` | `§X` for a section that has numbered subsections; specific subsection required |
| `non-normative-section` | Cites §I, §J, or §K |
| `unknown-section` | Section letter not in the document |
| `unknown-subsection` | Subsection number does not resolve |
| `page-out-of-range` | Page anchor outside the cited subsection's span |
| `page-out-of-document` | Page anchor exceeds document max |
| `doubled-page-anchor` | `p150–151 p151` form (FR-020) |
| `legacy-line-form` | Retired `line NNNN` form |

The full taxonomy is closed (a new defect class requires a code
change, not a config change). See `src/diagnostic.rs::DefectClass`.

## Defect catalog file

On every run with at least one defect, the lint writes a
deterministically-ordered Markdown catalog at
`docs/refactor-006/citation-defect-catalog.md` (overridable via
`--catalog-path`). The catalog format is documented in its own
header. PR 0.6 fixes every entry; thereafter the file's content is
expected to be the "no defects" placeholder.

The catalog is **deterministic across runs**: defects are sorted by
`(file, line, column, class)` before rendering. Two runs over the
same input produce byte-identical files.

## Integration with PR 0.6

PR 0.5 (this PR) lands the lint and the catalog. **CI fails** on PR
0.5 because the existing source has known defects (the four
pre-identified classes from FR-020 plus whatever the lint surfaces).
That failure is the **input** to PR 0.6, not a problem to fix in
PR 0.5. From PR 0.6 forward, CI is green; the lint then **prevents
new defects** from being introduced.

## Source authority

The lint reads `crates/capco/docs/CAPCO-2016.md` as the
authoritative source (per Constitution VIII and `marque-ism`'s
schema-version pin). The parser uses the table of contents to build
the section→page index — within section H, several subsections
(H.2, H.3, H.5, H.6, H.7) are not standalone markdown headings, but
all subsections appear in the TOC, so the TOC is the single source
of truth.

When CAPCO-2016 is superseded (a hypothetical CAPCO-2030), the
parser at `src/parser.rs` may need a small revision. The page-marker
grammar (`begin page N               UNCLASSIFIED`) is likely
identical across revisions; the section structure could shift.

## Testing

```bash
cargo test --manifest-path tools/citation-lint/Cargo.toml
```

Integration tests under `tests/` exercise the scanner, parser, and
resolver against synthetic Rust source files in `tests/fixtures/`.
Each defect class is covered by at least one fixture.

## References

- `specs/006-engine-rule-refactor/spec.md` — FR-018, FR-019, FR-020
- `specs/006-engine-rule-refactor/checklists/correctness.md` §4 —
  CHK032–CHK039 review-time questions
- `docs/plans/2026-05-02-engine-refactor-consolidated.md` — PR 0.5 row
- `.specify/memory/constitution.md` — Principle VIII (Authoritative
  Source Fidelity)
