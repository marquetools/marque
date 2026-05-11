# Decision 04 — Citation Hygiene (PR 3c)

**Date.** 2026-05-10.
**Scope.** Decision 6 of the four parallel PR-3c-prep analyses: opportunistic vs. systematic citation cleanup, with a permanent citation-integrity harness as a paired question.
**Method.** Read-only audit. Each `Diagnostic.citation` and catalog-row `label`/`citation` re-verified against `crates/capco/docs/CAPCO-2016.md` (the vendored authoritative source) via Grep, with line-to-page mapping derived from the markdown's `begin page N` / `end page N` markers (Constitution VIII propagation re-verification).

---

## 1. Audit's six claimed defects — verified

| Audit defect | Code site | Current citation in code | Verified correct citation | CAPCO-2016.md anchor (line, page) | Edit cost |
|---|---|---|---|---|---|
| **E012** wrapper drift | `rules_declarative.rs:400` | `"CAPCO-2016 §B.1"` | `"CAPCO-2016 §H.3 p55"` | line 1223 (page 55) — "The US, non-US, and JOINT classification markings are mutually exclusive" | ~16 chars |
| **E015** wrapper drift | `rules_declarative.rs:498` | `"CAPCO-2016 §B.3"` | `"CAPCO-2016 §H.7 p122 + §B.3 p20"` (matches catalog at `scheme.rs:1180`) | line 3030 (p122, FGI template) + line 401 (p20, §B.3.d FGI in DAPs) | ~30 chars |
| **E001** imprecise | `rules.rs:430, 469` | `"CAPCO-2016 §H.8"` and `"CAPCO-2016 §H.9"` (two branches) | §H.8 p131-168 / §H.9 p169-191 are the per-template "Authorized Banner Line Abbreviation" columns; no single page; existing form is the tightest the rule can carry without listing every per-marking page | not a defect by the standard the audit applies; the per-template column is what the rule keys on, and the citation correctly names the section that contains all those columns | 0 if accepted as tight enough |
| **E009** imprecise | `rules.rs:1799, 1834, 1866` | `"CAPCO-2016 §H.1 (US Classification Markings)"` and `§H.8` and `§H.9` (three branches) | §H.1 p47-54 (US class portion-mark column); §H.8 / §H.9 per-template; same per-template-column reasoning as E001 | same — not strictly a defect | 0 |
| **E011** imprecise | `rules.rs:2685` | `"CAPCO-2016 §A.6 + §H.3"` | `"CAPCO-2016 §A.6 p15 + §H.3 p55"` | line 317 (page 15, "non-US or Joint information ... must always start with `//`") + line 1225 (page 55, JOINT "always starts with a double forward slash") | ~16 chars |
| **E013** imprecise | `rules.rs:2839, 2915` | `"CAPCO-2016 §H.3 p56 (...)"` and `"CAPCO-2016 §H.8 p150–151 (...)"` | already page-precise | line 1234 area (page 56) + lines 3637-3700 area (pp 150-151) | **0 — audit got this one wrong; E013 is already page-precise.** The audit's claim that E013's `Diagnostic.citation` is imprecise contradicts the source. |

**Newly-discovered citation defect during E011 verification (audit miss).** The E011 doc-comment at `rules.rs:2677` cites "§H.3 p163" for the JOINT "always starts with `//`" claim. Page 163 is the DISPLAY ONLY section under §H.8, NOT the JOINT section. The actual JOINT prose is on page 55 (`begin page 55` at line 1170; the "always starts with a double forward slash" passage at line 1225 is on page 55). This is a concrete propagation defect — the comment was likely written against an earlier markdown render where lines drifted from pages, and the surrounding §A.6 reference (lines 771-772) is also stale (lines 771-772 are in §A.6 but the actual content is on lines 317; the file appears to have been re-rendered). Total comment edit: 2 §-references, ~10 chars each.

**Net audit accuracy.** 4 of 6 audit-flagged defects verified (E012, E015, E001/§-only, E009/§-only, E011/§-only). E013 audit claim is wrong. E011 has a SECOND defect the audit didn't surface.

---

## 2. Spot-check sample — 8 additional rules verified

| Rule | Code site | Citation | Verified | Notes |
|---|---|---|---|---|
| **E003** | `rules.rs:801` | `"CAPCO-2016 §A.6 p15-16"` | correct | §A.6 spans pages 15-16; line 317 (p15) introduces formatting; line 327 area (p16) lists block ordinals |
| **E007** | `rules.rs:1357, 1383` | `"CAPCO-2016 §E.6"` | **§-only — defect** | §E.6 = "Retired or Invalid Declassify On Values" at line 689, page 33; correct form is `"CAPCO-2016 §E.6 p33"` |
| **E029** | `rules.rs:4238, 4244` | `"CAPCO-2016 §H.5 p100 (...)"` | correct | §H.5 SAR syntax rules on p100 verified (begin page 100 at line 2625-area) |
| **W034** | `rules.rs:4707` | `"CAPCO-2016 §A.6 p16; §H.4 p61"` | correct | §A.6 p16 covers SCI sub-marker block ordering; §H.4 p61 covers unpublished SCI control systems and is the stated authority |
| **S001** | `rules.rs:1933` | `"CAPCO-2016 §A.6 p15 + §G.1 p36 Table 4"` | correct | §A.6 p15 has the spelled-out-vs-abbreviation prose; §G.1 Table 4 is on p36-38 |
| **E006** | `rules.rs:1249` | `"CAPCO-2016 §F"` | **§-only — defect** | §F = "Legacy Control Markings" at line 724, page 35; correct form is `"CAPCO-2016 §F p35"` |
| **E024** | `rules_declarative.rs:703` | `"CAPCO-2016 §H.6"` | **§-only — defect** | §H.6 RD precedence over FRD/TFNI is on page 104 (RD entry); correct form is `"CAPCO-2016 §H.6 p104"`. The catalog row at `scheme.rs:2351` already carries the precise form |
| **E054**/**E055** | `rules_declarative.rs:1259, 1260` | `"CAPCO-2016 §H.8 p154"` | correct | §H.8 RELIDO entry verified on page 154 (begin page 154 at line ~3850; "Cannot be used with NOFORN or DISPLAY ONLY") |

**New defects found in spot-check: 3** (E007, E006, E024 — all §-only where a page is verifiable). All are wrapper-vs-catalog drift of the same shape as E012/E015: the catalog row carries the precise form, the user-emitted `Diagnostic.citation` carries the §-only form.

---

## 3. The wider §-only defect cluster the audit missed

A `grep -nE '"CAPCO-2016 §[^"]+"'` filtered to citations missing a page reveals a much larger drift surface than the audit's six:

- **`rules.rs`:** 11 §-only citations (`§H.8`, `§H.9`, `§A.6`, `§F`, `§E.6` ×2, `§H.1`, `§H.8`, `§H.9`, `§A.6 + §H.3`, `§H.9` (line 3028), `§H.8 + ODNI ISMCAT ...`).
- **`rules_declarative.rs`:** 6 §-only `citation:` strings (some are pre-canonicalized `legacy text` that already has wrapper-vs-catalog policy notes).
- **`scheme.rs`:** 16 §-only labels and citations across catalog rows (8 `§H.4` class-floor rows + 8 `§H.4` / `§H.5` `Constraint::Custom` labels).

**Total: 28 instances** of `§-only` citation strings where a page reference is verifiable from `CAPCO-2016.md` and would tighten the citation per Constitution VIII. The audit surfaced ~4 of those plus 2 wrapper-vs-catalog section drifts (E012/E015), so the audit's "6 defects" undercounts by **roughly 5×**.

A separate observation about the `scheme.rs:3320-3487` class-floor catalog: the §-only `§H.4` form is repeated 8 times for distinct rows whose actual page numbers differ — `HCS-comp-sub` belongs to §H.4 p68, `SI-comp` to §H.4 p76, `TK-BLFH` to §H.4 p87, `HCS-comp` to §H.4 p64/66, `RSV-comp` to §H.4 p72, etc. Each row's correct page is recoverable from `CAPCO-CONTEXT.md` §5.3 (which is itself derived from the manual). The §-only form merges these into a single uninformative reference even though the per-marking page is what an auditor needs.

---

## 4. Pre-flight harness analysis

### What exists today

`crates/capco/tests/citation_fidelity.rs` (260 lines, named CITED_AUTHORITIES, exists since PR 0.5). It does NOT verify citation correctness against `CAPCO-2016.md` — it only checks that for each cited section letter, at least one corpus fixture filename contains a related keyword. The doc explicitly defers the real verification to "PR 10 (F.1 maturation)" and notes the table is hand-curated pending a programmatic accessor on `Rule::citation()` that "lands in the F.1 maturation cycle." That accessor is part of the keystone refactor 3a/3b/3c — exactly the PR 3c we are now planning.

### What a real harness looks like

Two test layers, both in-tree, no new deps:

1. **Section-existence test** (cheap, pinning):
   - Build a `BTreeMap<&str, (line, page)>` once at test startup by parsing `CAPCO-2016.md` `begin page N` markers + `^### N. (U) <Title>` and `^## <Letter>. (U) <Title>` heading patterns to derive a `§X.Y → first-line, first-page` table.
   - Walk every citation string emitted by every `Rule::id() →`-keyed code-path or stored in `CapcoScheme::build_constraints()` / `CLASS_FLOOR_CATALOG` / `SCI_PER_SYSTEM_CATALOG`. (Today, these are not exposed via a stable accessor; the test would either re-implement the walk by re-running the engine over a synthetic full-coverage corpus and de-duplicating the citations, or include the citation tables via `pub(crate)` accessors gated behind `#[cfg(test)]`.)
   - For each `(citation, expected-§)`, parse the `§X.Y` and `pNN` tokens out of the citation string and assert `(§ exists in heading map) ∧ (page within ±1 of the heading's page)` (the ±1 fuzz allows a citation to point at the second page of a multi-page section without bouncing on every line-number drift).

2. **Passage-grep test** (stronger, slower):
   - For each rule with a free-text citation parenthetical (e.g., `"CAPCO-2016 §H.3 p56 (JOINT codes separated by a single space)"`), grep the parenthetical against the 5-line window around the cited page in `CAPCO-2016.md`. If the parenthetical doesn't appear (loose token-overlap match), the citation has drifted from the source.
   - Cost: ~50ms over 47 rules + ~20 catalog rows. Negligible compared to the existing corpus tests.

### Cost vs. value

- **Direct cost:** ~150 LOC to add to `citation_fidelity.rs` (or a new sibling file). One walkable static table for headings; one for `(line, page)`. No new crate dependencies — `CAPCO-2016.md` is already in-tree at `crates/capco/docs/CAPCO-2016.md`.
- **Value:**
  - Catches the entire 28-instance §-only cluster the moment someone adds another one.
  - Catches the E011-style "page number drift across re-renders of the markdown" failure mode silently.
  - Catches the wrapper-vs-catalog drift class entirely (E012, E015 — the wrapper would fail the passage-grep test against §B.1 / §B.3 even before the catalog comparison fires).
  - Closes the propagation loop Constitution VIII §"Propagation requires re-verification" describes: a citation moves through the codebase's lifecycle, and the harness re-verifies it on every CI run, not just at write time.

### Recommendation on harness

**Land it inside PR 3c, not as follow-up.** Three reasons:

1. **PR 3c.2 is itself a citation-rewrite.** The bag-of-tokens architecture restatement (`architecture.md`) collapses ~16 form rules into the renderer; their citations need to migrate too. Without a harness, the migration multiplies the existing drift-surface by the number of rule-bodies-touched.
2. **The harness's cost is small relative to PR 3c's footprint.** ~150 LOC vs. the thousands the rule migration touches. The marginal review cost is in the noise.
3. **The harness is a Constitution VIII obligation reified.** Principle VIII says citations must be "re-verifiable by any reviewer with the source in hand." The harness is the mechanical form of "any reviewer." Deferring it to a follow-up means PR 3c lands knowing the property is held by hand, not by CI — and the next migration after PR 3c lands knowing that too.

The skeleton in `citation_fidelity.rs` already documents the deferral; PR 3c is the natural close-out.

---

## 5. Recommendation on Decision 6

**Adopt the systematic approach. Land the harness in the same PR.**

### Rationale

- **The defect surface is ≥28, not 6.** Opportunistic cleanup ("fix the 6 the audit named") leaves an almost-identical second cluster of 22+ §-only citations untouched. The work to verify each of those is ~5 minutes per rule (find the section heading, find the page, append `pNN`); the work to ship them as one commit is one PR review.
- **Constitution VIII frames this as a correctness problem, not a polish problem.** Every §-only citation where a page IS verifiable is a citation that "could be re-verified by any reviewer with the source in hand" but isn't, today, without the reviewer doing the line-to-page math themselves. The systematic sweep + harness closes that gap once.
- **The "byte-identity freeze" justification in `rules_declarative.rs:38-55` is stale advice.** It cites SC-008 NDJSON byte-identity as the reason wrappers carry legacy citations verbatim. SC-008 is a parity contract between CLI and WASM emission, not a backward-compat contract with old log files. Per project memory `feedback_pre_users_no_deprecation_phasing.md` (and the constitution's Principle VIII override of any ad-hoc "freeze" policy), marque has no users and rewriting is free. The freeze policy itself is defect-shaped.
- **The harness pays for the cleanup.** Without the harness, the cleanup commit is "trust the author re-verified all 28." With the harness, the cleanup commit is "trust the author re-verified all 28 AND CI fails on the 29th if anyone adds it tomorrow." The latter shape is what compliance tooling looks like.
- **PR 3c.2 is going to touch most of these citations anyway.** The bag-of-tokens architecture restatement has ~16 form rules absorbed into the renderer; the renderer's per-axis canonicalization functions need to carry citations too. A single sweep + harness now means PR 3c.2 inherits a clean baseline; opportunistic cleanup means PR 3c.2 either re-litigates each citation per rule or carries the drift forward into the renderer.

### Tradeoffs

- **PR size grows by ~30 line-changes + ~150 LOC for the harness + heading-map + page-map tables.** Net: ~250 lines of changes on a PR that already touches thousands. Marginal.
- **The harness needs a one-time investment to land the heading and page tables.** Once it lands, it pays for itself on every subsequent rule-add.
- **There is one risk: the harness false-positives on legitimately ambiguous cases.** §A.6 spans pp 15-16; a rule that legitimately cites §A.6 broadly (because its predicate covers content on both pages) shouldn't be forced to a single page. The harness's ±1 fuzz handles the common case; for the few legitimate cross-page citations, an explicit allowlist in the test's table is the standard pattern.

### Confidence

**High** on the systematic recommendation (8/10). The audit's 6-defect count is verifiable as understated, and Constitution VIII's "re-verifiable by any reviewer" framing makes the choice between "fix 6, miss 22" and "fix 28, harness the rest" a question with a single defensible answer.

**Medium-high** on landing the harness inside PR 3c (7/10). The marginal cost is small, the alignment with PR 3c.2's renderer migration is strong, and the existing skeleton's TODO already names this as the maturation surface. The only argument for follow-up is PR-size discipline, which the cited cost numbers don't support.
