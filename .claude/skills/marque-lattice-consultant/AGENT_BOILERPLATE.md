# Shared Brief — Lattice-Consultant Reference Authors

This file is the shared header for agents A–E, who each author one or more reference files inside `.claude/skills/marque-lattice-consultant/references/`. Read this before reading your domain-specific brief.

## What you are contributing to

A skill called `marque-lattice-consultant`. The skill exists because the `marque` project (a Rust-based rule engine for classification markings — see the parent repo's `CLAUDE.md` and `.specify/memory/constitution.md`) is stalled on a refactor that needs genuine lattice-algebra expertise to unblock. The user is an expert in classification markings and is not a lattice theorist. The skill provides a "lattice design consultant" that Claude consults whenever the marque codebase asks a structural question about whether a construction is a lattice, what laws it satisfies, what construction the literature already names for it, or whether the question is really a lattice question at all.

## Audience for the file you write

Claude, in consultant mode, in a future conversation. Not a textbook reader. Claude will scan your file looking for the closest named construction matching a problem the user just described in informal English. Therefore:

- **Catalog form, not chapter form.** Each entry is a named construction with definition, laws, examples, non-examples, and one sentence on "when this comes up." Not connected prose.
- **Table of contents at the top.** With one-line "what this entry is" pointers so Claude can route quickly.
- **Cross-references between entries.** When entry X is a special case or generalization of entry Y, say so explicitly and link.
- **Citations on every claim.** A citation lets Claude tell the user "according to source Z…" — that's the consultant's authority.

## Rigor target: "good enough to work"

Sketches with citation to a textbook proof, not full reconstructed proofs. The user has been clear: they need to *unblock design decisions*, not produce a paper. If a law's proof is two lines, write the two lines. If it's twenty pages, write "follows from Theorem 3.4 of \[citation\]" and stop. Never fabricate or paraphrase a proof you can't actually trace to a source.

## Outcome progression — the consultant's bias

When Claude uses your file to advise the user, the bias is:

1. **(a) Try order-theory-adapted approaches first.** Look at the marque construct in front of us, search the catalog, propose the closest match. If the match is exact, recommend it. If partial, name the gap.
2. **(b) Pivot toward a known pattern.** If the construct doesn't fit anything cleanly, suggest a redesign that *would* fit a known pattern, with the trade-off named.
3. **(c) Refuse / redirect.** If neither (a) nor (b) is honest, say "this isn't a lattice problem" or "you need a graph-theory expert / proof-assistant / domain expert in X." That's a valid outcome and saves the user from forcing a square peg into a round hole.

Tag entries in your file with which mode they support. An entry that's frequently the answer in mode (a) is high-traffic; an entry that's mostly cited in mode (c) deserves a "limits" note.

## Sources policy

- **Cite-and-link only — do not vendor PDFs.** Surface any source for human review before vendoring.
- **Catalog every source** in `sources/SOURCES.md` (URL, license) AND in `references/bibliography.md` (full bibliographic citation, DOI, publisher, and an `archive.org` permalink if one exists — search `web.archive.org` for the canonical URL).
- **Never embed copyrighted text wholesale.** Paraphrase definitions, quote at most a short sentence with attribution, and refer to the source for the full development.
- Append to `references/bibliography.md`; don't worry about ordering yet — the human will sort.

## Citation format

Inline use BibTeX-style keys: `[davey-priestley-2002]`, `[denning-1976]`, `[cousot-cousot-1977]`. Resolve in `references/bibliography.md` like this:

```
[davey-priestley-2002]
Davey, B. A. & Priestley, H. A. *Introduction to Lattices and Order*, 2nd ed.
Cambridge University Press, 2002. ISBN 978-0521784511.
DOI: 10.1017/CBO9780511809088. Paywalled.
Author archive: https://web.archive.org/web/2024.../...
```

```
[burris-sankappanavar-1981]
Burris, S. & Sankappanavar, H. P. *A Course in Universal Algebra*. Springer, 1981.
Open access (Millennium Edition, 2012):
https://www.math.uwaterloo.ca/~snburris/htdocs/UALG/univ-algebra2012.pdf
Cite-and-link only.
```

## "When this comes up" hooks

For every catalog entry, include a short tag listing the marque-question-shapes that would route a consultant to this entry. You don't need to know marque internals — describe the question shape generically. Examples:

> **When this comes up.** A construction unions sets across pages and the user asks whether the result is a lattice. (Entry: bounded join-semilattice; missing a top is fine if no operation requires one.)

> **When this comes up.** Two operations need to commute over a finite-height domain and the question is whether iteration converges. (Entry: Knaster-Tarski / Kleene fixed-point.)

These hooks are what makes the file usable — without them it's a textbook. With them it's a consultant.

## Output

Write your file directly to its target path under `.claude/skills/marque-lattice-consultant/references/`. Append your bibliography entries to `.claude/skills/marque-lattice-consultant/references/bibliography.md` (create if missing). Append vendored-source license notes to `.claude/skills/marque-lattice-consultant/sources/SOURCES.md` (create if missing).

Return a short summary (≤300 words) when done: file written, line count, entry count, gaps you couldn't fill, vendored sources, paywalled citations.

Read your domain-specific brief next.
