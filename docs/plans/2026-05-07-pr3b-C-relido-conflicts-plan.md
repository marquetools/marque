# Implementation Plan: PR 3b.C — Four Declarative `Constraint::Conflicts` for RELIDO Incompatibilities

**Target file**: `docs/plans/2026-05-07-pr3b-C-relido-conflicts-plan.md`
**Branch**: `refactor-006-pr-3b-conflicts` (worktree `/home/knitli/marque-pr3b-C/`, off `origin/staging` at 13fdc085 — PR 3b.B just merged)
**Scope**: Sub-task T026c of PR 3b. Declarative-data + thin wrapper. PR lands against `staging`.

---

## PM Addendum (resolves planner open questions)

The planner produced this plan after re-verifying every PM-proposed citation against the vendored `crates/capco/docs/CAPCO-2016.md` (lines 3363, 3444, 3808 — all confirmed verbatim). Three open questions were flagged for PM resolution; the resolutions below are binding for the gan-generator and reviewers.

**Q1 — Span anchor for E056 / E057 (ORCON / ORCON-USGOV span vs RELIDO span)** → **Approved as proposed**: anchor at the §-asserting side (ORCON span for E056; ORCON-USGOV span for E057; RELIDO span for E054 / E055). Rationale: the wrapper's span anchor pairs the user-visible diagnostic location with the page where the asserting prose lives. A reviewer reading the diagnostic at "ORCON" sees "may not be used with RELIDO" inline at §H.8 p136. The pattern is "anchor at the §-cited template's primary token" and matches the existing E053 convention (which anchors at NOFORN, the §H.8 p145 asserting side).

**Q2 — TokenRef LHS/RHS catalog convention** → **Approved as proposed**: LHS = the §-asserting token. So E054 / E055 declare `left: TOK_RELIDO`, `right: TOK_NOFORN` / `TOK_DISPLAY_ONLY`; E056 / E057 declare `left: TOK_ORCON` / `TOK_ORCON_USGOV`, `right: TOK_RELIDO`. This matches E053 (`capco/noforn-conflicts-rel-to`: `left: TOK_NOFORN, right: TokenRef::AnyInCategory(CAT_REL_TO)` — NOFORN is the §H.8 p145 asserting side). The catalog row's LHS naturally pairs with the wrapper's span anchor (Q1), keeping LHS-token / span-anchor / §-cited-page in sync row-by-row.

**Q3 — Severity::Error for all four** → **Confirmed.** Conflicts pairs are correctness violations; mirroring E053's `Severity::Error` is correct. No FixProposal — Constitution V audit-first compliance: the engine cannot decide which of two conflicting tokens to remove without policy input; user resolution required.

**Q4 — `decisions.md` row numbering** → **Approved as D17 (D14/D15/D16 already in register from PR-0; agent used D17, the next free index).** A fresh row surfaces the scope correction at the top of the decisions register; a D13 sub-bullet would bury it. The D17 row title is: "PR 3b.C scope correction: RELIDO Conflicts roster pruned from ~15–20 to 4 rows under Constitution VIII; broader §3.4.2 family roster deferred to PR 3.7 T108b."

**Q5 — decoder-canonical-contamination channels (#257 / #259)** → **N/A.** Conflicts wrappers don't emit `FixProposal`, so `proposal.original` / `proposal.replacement` leak channels don't apply. No mitigation needed.

**One additional PM constraint not in the planner output**: the rule-count update in `crates/capco/README.md` should also bump the rule-inventory paragraph counts (currently "errors `E001`–`E052`"; with E053 from issue #256 already merged plus E054–E057 from this PR, the inventory band extends to E057). Verify against the current state before writing the README diff — issue #256's E053 may or may not have made it into the inventory paragraph yet.

— PM, 2026-05-07

---

## PM Addendum II — Subtractive FixProposal direction (supersedes plan §3 / §5.3 / §10 "no FixProposal")

**Reverses Q3's "no FixProposal" determination** based on user feedback, 2026-05-07.

The user's correction:

> "Marque is correct, yes, but it also *guides* people to the right choice. If a user is trying to add RELIDO to something that can't have RELIDO, IMO, we remove RELIDO and tell them why. *It can't have RELIDO* as written, so the fix is to make it correct — by removing RELIDO."
>
> "Dissemination markings are really the only area where we can apply true fixes — other areas we have to take the user at face value. We can't say 'this should be LES' if the user never provided the LES symbol, but we absolutely can say 'this combination of tokens can't be RELIDO'."

This is right. Marque is a guidance tool, not just a checker. Dissemination markings are the unique axis where corrective fixes are well-defined because we are never *inventing* a new token (we couldn't say "this should be LES" if the user never typed LES); we are only *removing* one that the surrounding tokens have already excluded by their own §-cited Relationship(s) prose. The other token in each conflict pair is the binding constraint, so the resolution direction is unambiguous:

- E054 (RELIDO ⊥ NOFORN): NOFORN dominates per FD&R supersession (§D.2 Table 3 + §H.8 p145 NOFORN entry "Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY"). **Remove RELIDO.**
- E055 (RELIDO ⊥ DISPLAY ONLY): DISPLAY ONLY is a positive disclosure decision (specific country list); RELIDO is a deferred decision. The deferred decision can't operate when a positive decision is already on the marking. **Remove RELIDO.**
- E056 (ORCON ⊥ RELIDO): ORCON requires originator approval for further dissemination; RELIDO defers to a SFDRA. ORCON is the binding constraint. §H.8 p136 explicitly says "May not be used with RELIDO" on the ORCON template — RELIDO is the rejected token. **Remove RELIDO.**
- E057 (ORCON-USGOV ⊥ RELIDO): same logic as E056. §H.8 p140. **Remove RELIDO.**

**Resolution applied to plan:**

1. **Severity** stays `Severity::Error`. The marking is a CAPCO violation, not a style suggestion. Error+Fix is precedented (E001 / many other E### rules). The fix-and-warn pattern from `rules_sci_per_system.rs` (E042–E051) uses Warn because the user "may have intended" the wrong combo for SCI; for RELIDO conflicts the §-cited Relationship(s) prose is unambiguous, so Error is right.

2. **`FixProposal`** now emitted by each of the four wrappers, removing RELIDO from `dissem_controls` and from the corresponding span in the source bytes. Confidence **0.95** (post-2026-05-08 calibration — the engine's default `Config::confidence_threshold` is `0.95` per `crates/config/src/lib.rs:156` and the auto-apply gate is `confidence >= threshold`; the earlier 0.9 value left the fix as a manual-review suggestion under the default, opposite of the user-stated guidance behavior). 0.95 matches the established CAPCO convention for definite, at-threshold, auto-apply fixes (`crates/capco/src/rules.rs:998 / :1327 / :2622 / :2777 / :2853` — definite-fix sites); 0.85–0.9 is reserved for conditional or lower-confidence cases. The §-cited prose in every E054–E057 case is categorical ("Cannot be used with..." / "May not be used with RELIDO") and the user has explicitly endorsed RELIDO as the remove-target, so 0.95 is the right tier.

3. **Span policy for the fix** (NOT the diagnostic span — that stays per Q1):
   - The fix span MUST cover RELIDO **plus the surrounding `/` separator** so the replacement is empty-string. Don't leave dangling `//` separators or stray `/`.
   - For RELIDO appearing as `.../RELIDO` (last token in dissem block), span is `[len_before("/RELIDO") .. len_after("RELIDO")]`, replacement is `""`.
   - For RELIDO appearing as `.../RELIDO/...` (middle token), span is `[len_before("/RELIDO") .. len_after("RELIDO")]`, replacement is `""`.
   - For RELIDO appearing as `RELIDO/...` (first dissem token, immediately after `//`), span is `[len_before("RELIDO") .. len_after("RELIDO/")]`, replacement is `""`.
   - The implementer should write a small helper `compute_relido_removal_span(attrs, token_spans) -> Option<(Span, &'static str)>` that returns `None` if the parser's span layout is ambiguous (rare-but-real; fall back to no-fix in that case rather than emit a malformed fix). The `attrs.token_spans` slice carries adjacent-separator information indirectly via byte offsets — use it.
   - If the implementer finds the helper is hard to get right correctly across all four wrappers, factor it into `crates/capco/src/rules_declarative.rs` as a private free function and unit-test it in isolation.

4. **`FixProposal` source field**: use `FixSource::Rule { rule_id }` per the existing pattern. Each wrapper's `FixProposal::new(span, replacement, confidence, source, migration_ref)` — `migration_ref: None` for these (no schema migration involved).

5. **Diagnostic message** updated to reflect the fix direction:
   - E054: `"RELIDO removed: cannot be used with NOFORN (§H.8 p154)"`
   - E055: `"RELIDO removed: cannot be used with DISPLAY ONLY (§H.8 p154)"`
   - E056: `"RELIDO removed: ORCON may not be used with RELIDO (§H.8 p136)"`
   - E057: `"RELIDO removed: ORCON-USGOV may not be used with RELIDO (§H.8 p140)"`

6. **Constitution V (audit-first) compliance**: each `FixProposal` is pure data (span + replacement + confidence + source + migration_ref). The engine snapshots runtime state into `AppliedFix` at promotion time. The wrappers never construct `AppliedFix`. **Compliance preserved.**

7. **Behavior tests** (plan §6.1.2) extended:
   - Both tokens present → expect ONE Diagnostic that carries a `FixProposal` whose span covers RELIDO+separator and whose replacement is `""`.
   - Apply the fix to the source bytes and parse the result; assert RELIDO is absent and the dissem block is well-formed (no `//`, no leading `/`, no trailing `/`).
   - Confidence equals `0.95`.
   - The non-RELIDO token in the pair remains untouched in the post-fix bytes.

8. **Citation-fidelity test** (plan §6.1.3) extended: assert that each emitted `Diagnostic.proposal.is_some()` AND `Diagnostic.proposal.unwrap().confidence` equals `0.95`.

9. **Acceptance criterion §10 amended**: replace "No `FixProposal` is emitted by any of the four wrappers" with "Each of the four wrappers emits a `FixProposal` with span covering `RELIDO` + adjacent `/` separator, replacement = `""`, confidence = 0.95, source = `FixSource::BuiltinRule`, migration_ref = `None`. Applying the fix produces a parsed marking with RELIDO absent and dissem block well-formed."

10. **Generalization scope**: this rule applies to dissem-axis `Constraint::Conflicts` rules ONLY. Non-dissem axis conflicts (e.g., classification conflicts E012, JOINT cross-system constraints) remain "user resolves" because we cannot infer the fix direction without policy input. Future PR 3.7's RELIDO RhsFamily entries inherit this same subtractive-fix pattern (remove RELIDO; remove the FGI/JOINT/NATO atom would be wrong because the foreign equity is the document's reason-for-existing).

— PM, 2026-05-07 (Addendum II)

---

## 1. Executive Summary

PR 3b.C adds **four new `Constraint::Conflicts` rows** to `CapcoScheme::build_constraints()` (in `crates/capco/src/scheme.rs`) and **four thin `impl Rule` wrappers** in `crates/capco/src/rules_declarative.rs`, mirroring the precedent set by PR 3b.B's predecessor work (`capco/noforn-conflicts-rel-to` + `DeclarativeNofornRelToConflictRule` / E053). Each row declares one of the four directly-cited RELIDO mutual-exclusion pairs from CAPCO-2016 §H.8 — the Relationship(s) prose on the RELIDO, ORCON, and ORCON-USGOV templates.

**Headline scope correction.** The 2026-05-07 consultation verdict (`docs/plans/2026-05-07-pr3b-consultation-verdict.md` line 82) projected "~15–20 RELIDO Conflicts rows" for PR 3b.C, citing `marque-applied.md` §3.4.2's enumerated table (FD&R-family domination + non-US-equity-family conflict — RELIDO ⊥ {NOFORN, LES-NF, SBU-NF, DISPLAY ONLY} ∪ {each FGI atom, each JOINT atom, each NATO atom}). Re-verification of CAPCO-2016 against that roster surfaces only **four pairs** with direct, citation-backed authority. The remaining ~11–16 pairs are structural inferences (the consultant's "IDO has no authority over foreign equity" rationale) without a re-verifiable §-passage saying "may not be used with RELIDO" in either direction.

**Constitution VIII** is binding: a citation that cannot be re-traced to a real passage is a correctness defect of the same severity as a wrong predicate. Rather than fabricate fifteen specific §-citations, PR 3b.C lands the 4 directly-cited rows now and **defers the broader §3.4.2 family roster to PR 3.7 (T108b)**, where the new `Constraint::Conflicts::RhsFamily(predicate)` variant ships. At that point a single family-predicate row can carry the consultant's structural argument with one well-documented citation explaining the IDO-vs-foreign-equity rationale, instead of fifteen single-token rows each fabricating a §-cite.

**D13 attestation.** Each of the four new entries carries exactly one §-citation string verified against the vendored CAPCO-2016 source. Reciprocal mentions (NOFORN p145, DISPLAY ONLY p163) are documented in doc-comments but are NOT the primary `label`. Citations are quoted in §3 below for reviewer re-verification.

**Net rule delta.** +4 new declarative wrappers (E054, E055, E056, E057). 0 retirements. The 3b consultation verdict's "−1 to −2" target for 3b.C does NOT bind under the post-2026-05-07 retired numeric band; the gate is "stayed within the sub-move's authorized primitive scope" (Constraint::Conflicts only, no new variants), which this plan satisfies. Rule count moves from 56 → 60 (or 57 → 61 if E053 from issue #256 is already counted in the inventory paragraph; verify during impl).

---

## 2. Scope Determination (4 vs ~15–20)

### 2.1 The PM-proposed set (re-verified)

| LHS (asserting) | RHS | Primary citation (catalog `label`) | Verbatim cited passage | CAPCO-2016.md line |
|-----------------|-----|------------------------------------|------------------------|--------------------|
| RELIDO | NOFORN | `CAPCO-2016 §H.8 p154` | "Cannot be used with NOFORN or DISPLAY ONLY." (RELIDO entry, Relationship(s) to Other Markings) | 3808 |
| RELIDO | DISPLAY ONLY | `CAPCO-2016 §H.8 p154` | (same line as above; reciprocal at p163 line 4050) | 3808 |
| ORCON | RELIDO | `CAPCO-2016 §H.8 p136` | "May not be used with RELIDO." (ORCON entry, Relationship(s) to Other Markings) | 3363 |
| ORCON-USGOV | RELIDO | `CAPCO-2016 §H.8 p140` | "May not be used with RELIDO." (ORCON-USGOV entry, Relationship(s) to Other Markings) | 3444 |

Reciprocal cross-checks (informational only — NOT primary citations under D13):
- NOFORN entry p145 line 3585: "Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY."
- DISPLAY ONLY entry p163 line 4050: "Cannot be used with RELIDO or NOFORN."

The ORCON p136 and ORCON-USGOV p140 citations are **reciprocal-only** — the RELIDO p154 entry does NOT list ORCON or ORCON-USGOV in its Relationship(s) section. This is a real authority asymmetry (the assertion lives on the ORCON / ORCON-USGOV side of the relationship). The catalog rows reflect that asymmetry by citing the page where the prose actually says "may not be used with RELIDO."

### 2.2 The consultant's broader roster (deferred to PR 3.7)

Per `marque-applied.md` §3.4.2 (lines 603–638 of the consultant skill bridge at `.claude/skills/marque-lattice-consultant/references/marque-applied.md`), the full RELIDO incompatibility set has two groupings:

- **FD&R-family domination**: RELIDO ⊥ {NOFORN, LES-NF, SBU-NF, DISPLAY ONLY}. Of these, only NOFORN and DISPLAY ONLY have direct §H.8 p154 citations. LES-NF and SBU-NF are structural inferences; the §H.9 p178 (SBU-NF) and §H.9 p185 (LES-NF) prose does NOT say "may not be used with RELIDO."
- **Non-US-equity-family conflict**: RELIDO ⊥ {each FGI atom, each JOINT atom, each NATO atom}. None of these have direct §-citations; they are structural derivations from "IDO authority does not extend to foreign equity."

Constitution VIII's rule — "every citation must (a) refer to a real passage, (b) accurately reflect what the passage says, (c) be re-verifiable by any reviewer with the source in hand" — makes it impossible to land these 11–16 rows with §-cites in PR 3b.C without fabrication. The right primitive for them is `Constraint::Conflicts::RhsFamily(predicate)`, which lands in PR 3.7 (T108b). At that point a single row (or two — one per grouping) carries the structural argument with one well-documented citation chain explaining the IDO-vs-foreign-equity reasoning.

**Decision: 4 rows in PR 3b.C; broader roster lands in PR 3.7 alongside RhsFamily.**

### 2.3 Verdict-line-82 amendment

The 2026-05-07 consultation verdict at line 82 reads:

> 3b.C ~15–20 RELIDO Conflicts rows (mostly additive, ~−1 to −2)

This plan amends that line to read:

> 3b.C 4 RELIDO Conflicts rows (single-token RHS, all directly §-cited; broader §3.4.2 family roster deferred to PR 3.7 T108b under Constitution VIII)

The amendment is documented in this plan (§10 Acceptance Criteria) and reflected in `decisions.md` D17 (a fresh decision row, see §7; see also Q4 PM Addendum at top of plan — D14/D15/D16 already in register from PR-0).

---

## 3. The Four New Entries — Full Implementation Spec

### Entry 1: `E054/relido-conflicts-noforn`

- **Summary**: RELIDO and NOFORN may not coexist on the same marking. RELIDO is a permissive FD&R marking authorizing SFDRA-mediated foreign release; NOFORN is the most restrictive FD&R marking; the two are in direct semantic conflict on the FD&R axis.
- **Citation string** (catalog `label` + wrapper `Diagnostic.citation`): `"CAPCO-2016 §H.8 p154"`
- **Cited passage** (verified against `crates/capco/docs/CAPCO-2016.md` line 3808):
  > "Cannot be used with NOFORN or DISPLAY ONLY."
- **Reciprocal** (doc-comment only, NOT the primary citation): `crates/capco/docs/CAPCO-2016.md` line 3585 — NOFORN entry p145: "Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY."
- **Catalog row**:
  ```rust
  Constraint::Conflicts {
      name: "E054/relido-conflicts-noforn",
      left: TokenRef::Token(TOK_RELIDO),
      right: TokenRef::Token(TOK_NOFORN),
      label: "CAPCO-2016 §H.8 p154",
  }
  ```
- **Wrapper struct**: `DeclarativeRelidoNofornConflictRule` in `rules_declarative.rs`.
- **RuleId**: `RuleId::new("E054")`.
- **Severity**: `Severity::Error`.
- **Span policy**: prefer the RELIDO token span (RELIDO is the asserting token per p154). Fallback to NOFORN span if RELIDO span is unavailable. Pattern: lookup `TokenKind::DissemControl` with text `"RELIDO"`; fallback to `TokenKind::DissemControl` with text `"NOFORN"` or `"NF"`; final fallback `Span::new(0, 0)`.
- **Message**: `"RELIDO cannot be used with NOFORN (§H.8 p154); remove one or the other"`.

### Entry 2: `E055/relido-conflicts-display-only`

- **Summary**: RELIDO and DISPLAY ONLY may not coexist on the same marking. DISPLAY ONLY authorizes disclosure (not release); RELIDO defers release to a SFDRA. The two FD&R semantics are in direct conflict.
- **Citation string**: `"CAPCO-2016 §H.8 p154"`
- **Cited passage** (same line as Entry 1, verified at line 3808):
  > "Cannot be used with NOFORN or DISPLAY ONLY."
- **Reciprocal** (doc-comment only): `crates/capco/docs/CAPCO-2016.md` line 4050 — DISPLAY ONLY entry p163: "Cannot be used with RELIDO or NOFORN."
- **Catalog row**:
  ```rust
  Constraint::Conflicts {
      name: "E055/relido-conflicts-display-only",
      left: TokenRef::Token(TOK_RELIDO),
      right: TokenRef::Token(TOK_DISPLAY_ONLY),
      label: "CAPCO-2016 §H.8 p154",
  }
  ```
- **Wrapper struct**: `DeclarativeRelidoDisplayOnlyConflictRule`.
- **RuleId**: `RuleId::new("E055")`.
- **Severity**: `Severity::Error`.
- **Span policy**: prefer RELIDO token span; fallback to DISPLAY ONLY span (looking for `TokenKind::DissemControl` with text starting `"DISPLAY ONLY"`); final fallback `Span::new(0, 0)`.
- **Message**: `"RELIDO cannot be used with DISPLAY ONLY (§H.8 p154); remove one or the other"`.

### Entry 3: `E056/orcon-conflicts-relido`

- **Summary**: ORCON (Originator Controlled) and RELIDO may not coexist on the same marking. ORCON requires originator approval for further dissemination; RELIDO defers release to a SFDRA. The two control semantics are incompatible.
- **Citation string**: `"CAPCO-2016 §H.8 p136"`
- **Cited passage** (verified against `crates/capco/docs/CAPCO-2016.md` line 3363):
  > "May not be used with RELIDO."
  Plus the surrounding ORCON Relationship(s) prose at line 3361–3363:
  > "May not be used with ORCON-USGOV in a portion mark or banner line. May be used with NOFORN, REL TO, DISPLAY ONLY. May not be used with RELIDO."
- **Citation note** (doc-comment): the asserting prose lives on the ORCON template (p136), NOT in RELIDO's p154 Relationship(s) section. The directionality is real: §H.8 p154 does NOT mention ORCON. The catalog row anchors at the page where the assertion is made.
- **Catalog row**:
  ```rust
  Constraint::Conflicts {
      name: "E056/orcon-conflicts-relido",
      left: TokenRef::Token(TOK_ORCON),
      right: TokenRef::Token(TOK_RELIDO),
      label: "CAPCO-2016 §H.8 p136",
  }
  ```
- **Wrapper struct**: `DeclarativeOrconRelidoConflictRule`.
- **RuleId**: `RuleId::new("E056")`.
- **Severity**: `Severity::Error`.
- **Span policy**: prefer the ORCON span (the asserting token per p136 — anchoring the diagnostic where the user reads "May not be used with RELIDO"); fallback to RELIDO span; final fallback `Span::new(0, 0)`. Lookup pattern: `TokenKind::DissemControl` with text `"ORCON"` (or `"OC"`).
- **Message**: `"ORCON cannot be used with RELIDO (§H.8 p136); remove one or the other"`.

### Entry 4: `E057/orcon-usgov-conflicts-relido`

- **Summary**: ORCON-USGOV (the USGOV-pre-approved variant of ORCON) and RELIDO may not coexist. Same semantic conflict as Entry 3, declared on the ORCON-USGOV template.
- **Citation string**: `"CAPCO-2016 §H.8 p140"`
- **Cited passage** (verified against `crates/capco/docs/CAPCO-2016.md` line 3444):
  > "May not be used with RELIDO."
  In context (lines 3442–3446):
  > "May not be used with ORCON in a portion mark or banner line. May be used with NOFORN, REL TO, DISPLAY ONLY. May not be used with RELIDO."
- **Citation note** (doc-comment): ORCON-USGOV's template begins p139 (line 3407); the Relationship(s) subsection straddles p139–p140, with the RELIDO exclusion landing on p140. p140 is the catalog primary because that is where the specific RELIDO prose appears.
- **Catalog row**:
  ```rust
  Constraint::Conflicts {
      name: "E057/orcon-usgov-conflicts-relido",
      left: TokenRef::Token(TOK_ORCON_USGOV),
      right: TokenRef::Token(TOK_RELIDO),
      label: "CAPCO-2016 §H.8 p140",
  }
  ```
- **Wrapper struct**: `DeclarativeOrconUsgovRelidoConflictRule`.
- **RuleId**: `RuleId::new("E057")`.
- **Severity**: `Severity::Error`.
- **Span policy**: prefer the ORCON-USGOV span; fallback to RELIDO span. Lookup pattern: `TokenKind::DissemControl` with text `"ORCON-USGOV"` (or `"OC-USGOV"`).
- **Message**: `"ORCON-USGOV cannot be used with RELIDO (§H.8 p140); remove one or the other"`.

---

## 4. TokenId Allocations and `satisfies_attrs` Wiring

### 4.1 New TokenId constants in `crates/capco/src/scheme.rs`

Highest existing `TokenId` is `TOK_EXDIS = TokenId(123)` (lines 86–87). Append:

```rust
// PR 3b.C (T026c): RELIDO incompatibility roster sentinels.
// Resolved via `satisfies_attrs` against `attrs.dissem_controls` —
// all four tokens are IC dissem controls living in
// `marque_ism::DissemControl`.
pub const TOK_RELIDO: TokenId = TokenId(124);
pub const TOK_DISPLAY_ONLY: TokenId = TokenId(125);
pub const TOK_ORCON: TokenId = TokenId(126);
pub const TOK_ORCON_USGOV: TokenId = TokenId(127);
```

### 4.2 New `satisfies_attrs` arms

Add four arms inside the `TokenRef::Token(id) => match *id` block at `scheme.rs:1393`. Each arm scans `attrs.dissem_controls` for the corresponding `DissemControl` variant.

**Verified `DissemControl` variant names** (from `marque-ism` generated `values.rs`): `Relido`, `Displayonly`, `Oc` (ORCON), `OcUsgov` (ORCON-USGOV), `Nf` (NOFORN — already wired).

```rust
// Pattern mirrors the existing TOK_NOFORN arm at scheme.rs:1394–1397.
TOK_RELIDO => attrs
    .dissem_controls
    .iter()
    .any(|d| matches!(d, DissemControl::Relido)),
TOK_DISPLAY_ONLY => attrs
    .dissem_controls
    .iter()
    .any(|d| matches!(d, DissemControl::Displayonly)),
TOK_ORCON => attrs
    .dissem_controls
    .iter()
    .any(|d| matches!(d, DissemControl::Oc)),
TOK_ORCON_USGOV => attrs
    .dissem_controls
    .iter()
    .any(|d| matches!(d, DissemControl::OcUsgov)),
```

Insertion point: between the existing `TOK_NOFORN` arm and the `TOK_USA` arm (group all dissem-control sentinel resolutions together for catalog readability).

### 4.3 Constitution VII compliance

- TokenId constants added: `crates/capco/src/scheme.rs` only.
- `satisfies_attrs` arms added: `crates/capco/src/scheme.rs` only.
- New `DissemControl` variants needed: **none**. `Relido`, `Displayonly`, `Oc`, `OcUsgov` all already live in `marque-ism`'s generated `values.rs`.
- **Zero edits** to `marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`. Scheme-internal additions only. (Confirmed against the engine-edit prohibition in Principle VII.)

---

## 5. Rule-Set Registration and Wrapper Implementation

### 5.1 Rule-set registration in `crates/capco/src/rules.rs`

Append four `Box::new(...)` entries at the end of `CapcoRuleSet::new()`'s rule vector (after `DeclarativeNofornRelToConflictRule`, around `crates/capco/src/rules.rs:232`). Pattern mirrors the issue-#256 comment block at lines 227–232:

```rust
// PR 3b.C (T026c): RELIDO incompatibility declarative wrappers.
// Four directly-cited §H.8 conflict pairs from CAPCO-2016 — RELIDO ⊥
// {NOFORN, DISPLAY ONLY, ORCON, ORCON-USGOV}. Each wraps a
// Constraint::Conflicts row in CapcoScheme::constraints(). The
// broader §3.4.2 family roster (LES-NF / SBU-NF / FGI / JOINT /
// NATO atoms) is deferred to PR 3.7 (T108b) where
// Constraint::Conflicts::RhsFamily(predicate) lands — see
// docs/plans/2026-05-07-pr3b-C-relido-conflicts-plan.md §2.
Box::new(DeclarativeRelidoNofornConflictRule),
Box::new(DeclarativeRelidoDisplayOnlyConflictRule),
Box::new(DeclarativeOrconRelidoConflictRule),
Box::new(DeclarativeOrconUsgovRelidoConflictRule),
```

The four new wrapper struct names also need to be listed in the `use crate::rules_declarative::{...}` import block at `rules.rs:85–91`.

### 5.2 Wrapper struct implementation in `crates/capco/src/rules_declarative.rs`

Append below `DeclarativeNofornRelToConflictRule` (after line 887). Each wrapper is a zero-size struct following the exact shape of `DeclarativeNofornRelToConflictRule` (lines 843–887). Skeleton (Entry 1 fully written, Entries 2–4 follow the same shape):

```rust
// ---------------------------------------------------------------------------
// PR 3b.C (T026c) — RELIDO incompatibility wrappers (E054 / E055 / E056 / E057)
// ---------------------------------------------------------------------------
//
// Four directly-cited §H.8 RELIDO mutual-exclusion pairs:
//
//   E054 — RELIDO ⊥ NOFORN          (§H.8 p154; reciprocal §H.8 p145)
//   E055 — RELIDO ⊥ DISPLAY ONLY    (§H.8 p154; reciprocal §H.8 p163)
//   E056 — ORCON  ⊥ RELIDO          (§H.8 p136; no reciprocal — asymmetric)
//   E057 — ORCON-USGOV ⊥ RELIDO     (§H.8 p140; no reciprocal — asymmetric)
//
// Pattern follows DeclarativeNofornRelToConflictRule (E053): each
// wrapper calls violations_for(attrs, "<catalog-name>") as a trigger
// check, picks a span from attrs.token_spans, and constructs a
// single Diagnostic with the catalog's label as the citation.
//
// No FixProposal — Conflicts pairs cannot be auto-resolved (the
// engine cannot decide which token to remove).

pub(crate) struct DeclarativeRelidoNofornConflictRule;

impl Rule for DeclarativeRelidoNofornConflictRule {
    fn id(&self) -> RuleId { RuleId::new("E054") }
    fn name(&self) -> &'static str { "relido-noforn-conflict" }
    fn default_severity(&self) -> Severity { Severity::Error }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E054/relido-conflicts-noforn").is_empty() {
            return vec![];
        }

        // Prefer the RELIDO span — RELIDO is the asserting token per
        // §H.8 p154 ("Cannot be used with NOFORN or DISPLAY ONLY").
        // Fall back to NOFORN if RELIDO span isn't found in token_spans
        // (e.g., the parser fast-path elided it).
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "RELIDO")
            .or_else(|| attrs.token_spans.iter().find(|t| {
                t.kind == TokenKind::DissemControl
                    && (&*t.text == "NOFORN" || &*t.text == "NF")
            }))
            .map(|t| t.span)
            .unwrap_or_else(|| Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RELIDO cannot be used with NOFORN (§H.8 p154); \
             remove one or the other",
            "CAPCO-2016 §H.8 p154",
            None,
        )]
    }
}
```

E055 mirrors E054 with `"E055/relido-conflicts-display-only"` and the DISPLAY ONLY token text. E056 / E057 anchor at the ORCON / ORCON-USGOV spans respectively (asserting prose lives on those templates per §3 above), with RELIDO as the fallback span. All four wrappers are stateless zero-size structs; Constitution VI Send + Sync compliance is automatic.

### 5.3 No `FixProposal` for any wrapper

Each wrapper returns `Diagnostic::new(..., None)` — the trailing `None` is the optional `FixProposal`. **Constitution V (audit-first) compliance**: Conflicts wrappers cannot propose fixes because the engine cannot decide which of the two conflicting tokens to remove without policy input. The user must resolve the ambiguity manually. Confirmed against E053's pattern (line 884 — `None`).

### 5.4 `evaluate_custom_by_attrs` is NOT touched

The four new rows are `Constraint::Conflicts`, not `Constraint::Custom`. The dyadic-Conflicts dispatch path in `CapcoScheme::evaluate_named_constraint` (`scheme.rs:1558–1568`) already handles them generically — no per-name custom predicate helper is needed. `evaluate_custom_by_attrs` (line 1503) is untouched.

---

## 6. Test Plan

Mirror the PR 3b.B test posture (`crates/capco/tests/transmutation_rewrites.rs`).

### 6.1 New test file: `crates/capco/tests/relido_conflicts.rs`

Authoring-contract + behavior + citation-fidelity tests for the four new entries. Approximate size: ~250 lines.

#### 6.1.1 Authoring-contract tests (one per entry × four entries = 4 tests)

For each new row, assert against `CapcoScheme::new().constraints()`:
- The constraint is found by `name`.
- The variant is `Constraint::Conflicts` (not `Custom`, not `Requires`, etc.).
- `left` and `right` are the expected `TokenRef::Token(...)` values.
- `label` is the exact §-citation string from §3 above.

Helper similar to PR 3b.B's `lookup_rewrite()`:

```rust
fn lookup_constraint<'a>(scheme: &'a CapcoScheme, name: &str) -> &'a Constraint {
    scheme.constraints().iter()
        .find(|c| c.name() == name)
        .unwrap_or_else(|| {
            let declared: Vec<&str> = scheme.constraints().iter().map(|c| c.name()).collect();
            panic!("constraint {name:?} is not declared on CapcoScheme; declared: {declared:?}")
        })
}
```

#### 6.1.2 Behavior tests (one per entry × four entries = 4 tests, each with three sub-cases)

For each of the four conflicts, three sub-cases:

- **Both tokens present**: construct minimal `CanonicalAttrs` with both `DissemControl` variants populated; run the wrapper's `check()` and assert one `Diagnostic` is returned with the expected `RuleId`, `citation`, and span class (RELIDO span for E054/E055; ORCON / ORCON-USGOV span for E056/E057).
- **Only one token present**: `check()` returns empty.
- **Neither token present**: `check()` returns empty.

Use the existing helpers in `crates/capco/src/rules.rs` `marque_capco_test_support` module (`lint_banner` / `lint_portion` at lines 11405+) where convenient, or construct `CanonicalAttrs` directly via `from_parsed_unchecked` for tighter unit-style tests.

#### 6.1.3 Citation-fidelity test (1 test)

Walk all four new wrappers' emitted diagnostics and assert that each `Diagnostic.citation` equals the catalog's `label` exactly. This is the regression guard against drift the issue-#256 work surfaced — if a future edit changes the citation in one place but not the other, this test fires.

```rust
#[test]
fn relido_conflict_wrappers_carry_catalog_citations() {
    let scheme = CapcoScheme::new();
    let cases = [
        ("E054/relido-conflicts-noforn",       "CAPCO-2016 §H.8 p154"),
        ("E055/relido-conflicts-display-only", "CAPCO-2016 §H.8 p154"),
        ("E056/orcon-conflicts-relido",        "CAPCO-2016 §H.8 p136"),
        ("E057/orcon-usgov-conflicts-relido",  "CAPCO-2016 §H.8 p140"),
    ];
    for (name, expected_citation) in cases {
        let c = lookup_constraint(&scheme, name);
        assert_eq!(c.label(), expected_citation,
            "constraint {name} catalog label drifted from plan §3");
    }
    // Wrapper-emission side: build minimal triggering attrs and
    // confirm Diagnostic.citation == catalog.label for each rule.
    // [...]
}
```

#### 6.1.4 Constraint-shape pin (1 test)

Assert that all four new entries are `Constraint::Conflicts` (NOT `Custom`) — guards against a future PR converting them to Custom and bypassing the generic dyadic-evaluation path.

```rust
#[test]
fn relido_conflict_rows_are_dyadic_conflicts_variant() {
    let scheme = CapcoScheme::new();
    for name in ["E054/relido-conflicts-noforn",
                 "E055/relido-conflicts-display-only",
                 "E056/orcon-conflicts-relido",
                 "E057/orcon-usgov-conflicts-relido"] {
        let c = lookup_constraint(&scheme, name);
        assert!(matches!(c, Constraint::Conflicts { .. }),
            "constraint {name} must be Constraint::Conflicts, got {c:?}");
    }
}
```

### 6.2 Existing tests to update

#### 6.2.1 `crates/capco/tests/scheme_equivalence.rs:1081`

The existing test `assert_eq!(d.constraints().len(), n.constraints().len())` is determinism-only (compares two scheme instances), not absolute count — adding 4 rows preserves the determinism, so this test passes unchanged. **No change needed**.

#### 6.2.2 Constraint count pin (NEW — recommended)

PR 3b.B did not have an absolute count pin for `constraints().len()`, but PR 3b.B's `transmutation_rewrites.rs` does pin `page_rewrites().len() == 9` indirectly via the per-row tests. Add a pin in `relido_conflicts.rs`:

```rust
#[test]
fn capco_constraints_count_after_pr3b_c() {
    // Pre-3b.C: N constraints. PR 3b.C adds exactly 4 (E054–E057).
    // Bump this number when intentional new constraints land; a
    // failure here means catalog drift the reviewer should look at.
    let scheme = CapcoScheme::new();
    let pre_3b_c_count = /* count after PR 3b.B merge — verify with
                            `cargo test capco_constraints_count_after_pr3b_b`
                            during impl */;
    assert_eq!(scheme.constraints().len(), pre_3b_c_count + 4);
}
```

(The implementer determines the exact pre-3b.C count by running the workspace tests against the branch base before adding any rows. This avoids a round-the-bend race where the PM and the agent disagree on what "current" means.)

#### 6.2.3 `crates/capco/tests/send_sync.rs`

The four new wrapper structs must satisfy the `Send + Sync` invariant. The existing `assert_send_sync<T>` helper covers `Box<dyn Rule>`. If `send_sync.rs` enumerates concrete wrapper types, add the four new ones; if it asserts via the trait object only, the new wrappers are covered automatically. **Implementer verifies during review of `send_sync.rs`.**

#### 6.2.4 `crates/capco/tests/citation_fidelity.rs`

If this file walks `scheme.constraints()` validating `label` strings against the vendored CAPCO-2016 source (analogous to its potential `page_rewrites()` walk), the four new rows are covered automatically by adding them to the catalog. **Implementer verifies during review of `citation_fidelity.rs`.**

#### 6.2.5 Rule-set count assertion (if any)

Scan `crates/capco/tests/rules_us1.rs`, `crates/capco/tests/corpus_parity.rs`, and other rule-set-touching tests for `rule_set.rules().len() == N` assertions. PR 3b.C bumps the count by 4 (56 → 60); update any such pin. Implementer verifies.

### 6.3 Regression

Full workspace test suite passes:
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo build -p marque-wasm` (WASM-safe set unaffected; sanity check)

### 6.4 Coverage

≥80% on lines added in `scheme.rs` (TokenId constants + 4 satisfies_attrs arms + 4 catalog rows) and `rules_declarative.rs` (4 wrapper structs). Behavior tests above exercise the trigger / no-trigger / non-trigger paths for each.

---

## 7. Files Touched

Relative to `/home/knitli/marque-pr3b-C/`. Approximate line counts.

| File | Change | Approx. lines |
|---|---|---|
| `crates/capco/src/scheme.rs` | Add 4 `TokenId` constants (TOK_RELIDO, TOK_DISPLAY_ONLY, TOK_ORCON, TOK_ORCON_USGOV); add 4 `satisfies_attrs` arms; add 4 `Constraint::Conflicts` rows in `build_constraints()` with full doc-comments | +~80 |
| `crates/capco/src/rules_declarative.rs` | Add 4 wrapper structs (E054 / E055 / E056 / E057) following the `DeclarativeNofornRelToConflictRule` shape | +~150 |
| `crates/capco/src/rules.rs` | Register 4 wrappers in `CapcoRuleSet::new()`; add wrapper names to the `use crate::rules_declarative::{...}` block; update the rule-ID inventory comment header (lines 13–60) with E054–E057 entries | +~25 |
| `crates/capco/tests/relido_conflicts.rs` | New test file: authoring-contract + behavior + citation-fidelity + constraint-shape-pin + count-pin tests | ~250 |
| `crates/capco/README.md` | Update rule-inventory paragraph: "56 rules" → "60 rules"; add E054–E057 to the rule-ID list | ~3 |
| `docs/plans/2026-05-07-pr3b-consultation-verdict.md` | Amend line 82 to reflect "4 RELIDO Conflicts rows ... broader roster deferred to PR 3.7 T108b" — see plan §2.3 | ~2 |
| `specs/006-engine-rule-refactor/decisions.md` | Add **D17** decision row (D14/D15/D16 already in register from PR-0; D17 is the next free index): "PR 3b.C scope correction: RELIDO Conflicts roster pruned from ~15–20 to 4 rows under Constitution VIII; broader §3.4.2 family roster deferred to PR 3.7 T108b" | ~10 |
| `specs/006-engine-rule-refactor/tasks.md` | Update T026c row to reflect "4 enumerated `Constraint::Conflicts` rows" instead of "~15–20"; note the deferral | ~3 |
| `specs/006-engine-rule-refactor/plan.md` | If the D13 addendum table references "~15–20 RELIDO rows" for 3b.C, amend to "4 rows" with deferral note | ~2 (audit during impl) |

**No changes** to: `crates/scheme/`, `crates/engine/`, `crates/core/`, `crates/rules/`, `crates/ism/`, any other workspace crate. Constitution VII compliance verified.

---

## 8. Out of Scope / Follow-Up

The following are explicitly NOT in scope for PR 3b.C and are tracked for later work:

1. **Broader §3.4.2 family roster** (RELIDO ⊥ {LES-NF, SBU-NF, each FGI atom, each JOINT atom, each NATO atom}) — deferred to **PR 3.7 (T108b)** where `Constraint::Conflicts::RhsFamily(predicate)` lands. At that point a single (or two) family-predicate row(s) carry the structural argument with one well-documented citation chain (likely §H.8 RELIDO definition + §H.7 FGI + §H.3 JOINT introduction prose explaining IDO scope, packaged as a new "structural derivation" citation form to be designed in T108b).
2. **Closure-operator implications for RELIDO** — e.g., RELIDO implies REL TO under certain conditions per §B.3 Table 2 — deferred to **PR 3.7 (T108c)** where the closure-operator primitive ships with `ImplTable`.
3. **Runtime activation** of the four new wrappers in the engine's lint pipeline — already activated for E053 and E054–E057 follow the same dispatch path; nothing new to wire. (Verified: `Engine::lint` walks `RuleSet::rules()`, which the registration in §5.1 covers.)
4. **`AppliedFix` audit-record changes** — none. Conflicts wrappers emit no `FixProposal`, so no audit records are produced. Constitution V untouched.
5. **`Codec<S>` round-trip** — no codec impls in-tree (Phase G), so the Conflicts entries don't cross the codec boundary. No round-trip obligations.

---

## 9. Open questions (all resolved by PM addendum at top of file)

See PM Addendum at top. Q1 / Q2 / Q3 / Q4 / Q5 are all locked; gan-generator should not re-open them.

---

## 10. Acceptance Criteria

PR 3b.C is ready to merge when:

- [ ] Each of the four entries' citation is verified against `crates/capco/docs/CAPCO-2016.md` line numbers stated in §3 (Constitution VIII, mandatory).
- [ ] `cargo check --workspace` passes.
- [ ] `cargo test --workspace` passes. New `relido_conflicts.rs` has 24 tests under PM Addendum II: 4 authoring-contract + 4 "fires + emits FixProposal" behavior + 8 silence behavior (2 sub-cases × 4 wrappers) + 1 citation-fidelity (extended to assert `proposal.is_some()` and `confidence == 0.95` per PM Addendum II §8 post-2026-05-08 calibration) + 1 constraint-shape pin + 1 count pin + 4 helper-position tests (first / middle / last / RELIDO-absent) + 1 `FixSource` / `migration_ref` discipline test.
- [ ] `cargo clippy --workspace -- -D warnings` produces no new warnings.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo build -p marque-wasm` succeeds (WASM-safe set still compiles).
- [ ] Each new `Constraint::Conflicts` row has exactly one §-citation as `label` (D13 single-citation discipline).
- [ ] Each new wrapper's emitted `Diagnostic.citation` equals its catalog `label` exactly (citation-fidelity test passes).
- [ ] **(Superseded by PM Addendum II §9 — confidence calibrated 2026-05-08)** ~~No `FixProposal` is emitted by any of the four wrappers (Conflicts pairs are user-resolution-required).~~ → Each of the four wrappers emits a `FixProposal` with span covering `RELIDO` + adjacent `/` separator, replacement = `""`, **confidence = 0.95** (clears the engine's default `Config::confidence_threshold = 0.95` so the fix auto-applies — the 2026-05-08 calibration; the earlier 0.9 value left the fix as a manual-review suggestion under the default), source = `FixSource::BuiltinRule`, migration_ref = `None`. Applying the fix produces a parsed marking with RELIDO absent and the dissem block well-formed (no `//`, no leading or trailing `/`). The behavior tests in `tests/relido_conflicts.rs` exercise all three separator-position cases (first / middle / last in the dissem block).
- [ ] No edits to `marque-engine`, `marque-scheme`, `marque-rules`, `marque-ism`, `marque-core` (Constitution VII).
- [ ] No new `DissemControl` variants in `marque-ism` (verified — `Relido`, `Displayonly`, `Oc`, `OcUsgov` already exist).
- [ ] Rule count in `crates/capco/README.md` updated from "56 rules" to "60 rules" (or whatever the current pre-3b.C count is + 4) with E054–E057 listed in the inventory.
- [ ] Consultation verdict line 82 amended (or in-line note added documenting the scope correction).
- [ ] `decisions.md` D17 row added documenting the scope correction with Constitution VIII rationale (D14/D15/D16 already in register from PR-0; D17 is the next free index).
- [ ] `tasks.md` T026c row updated to reflect "4 enumerated rows" instead of "~15–20" with deferral note pointing to PR 3.7 T108b.
- [ ] PR description includes net-rule-delta math: 56 → 60 (+4); D13 attestation: every new entry has one §-cite verified against vendored source; reviewer named who confirmed each of the four citations by hand.
- [ ] PR description includes the audible note: "4 vs ~15–20 scope correction is binding under Constitution VIII; broader roster lands in PR 3.7 alongside `Constraint::Conflicts::RhsFamily(predicate)` (T108b)."

**Quality bar reminder (5-year maintainability)**: Each row should make a reviewer say "I see exactly which §H.8 passage this enforces, and I can verify it in 30 seconds with the vendored manual." The four-row form satisfies this; a fifteen-row form with structural-inference citations would not.

End of plan.
