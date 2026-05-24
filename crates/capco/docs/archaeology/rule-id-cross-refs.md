<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Live-Rule ↔ Retired-Rule Cross-References

Historical inline comments that documented "live rule X used to share
predicate state with retired rule Y." Extracted from `rules.rs` during
the issue #561 split. Grouped by **live rule** so a developer reading
current code can trace the historical context that was once inlined
adjacent to it.

> **Scope and stability.** This file is archaeology — nothing here is
> consulted at runtime. The split (issue #561) extracted the obvious
> pure-history blocks: the top-of-file rule-ID assignment table moved
> intact to [`retirement-history.md`](./retirement-history.md), and
> the `CapcoRuleSet::new()` retirement narratives for PR 3c.B Commit 6
> / T035c-14 / PR #470 / T035b / PR #578 / PR 3c.B Commit 7.3 / PR 3c.B
> Commit 7.4 / PR #488 moved there as well.
>
> Inline comments that **explain why current code does X**
> (defending against regression, describing observable cross-rule
> behavior, documenting a load-bearing precedent) stayed in the live
> file per Decision 10.2's conservative bias. This file collects the
> few comments where the entire body was pure narration of a retired
> rule's history with no remaining purchase on live behavior.

## E006 (`DeprecatedDissemRule`)

E006 walks the `marque_ism` `MIGRATIONS` table to surface deprecated
dissemination controls. The legacy form-pair guard at this rule's
fix site (`is_abbreviation_expansion`) was dead-code-by-construction
since T035c-4 — the `MIGRATIONS` table no longer carries form-pair
entries (NF/OC/IMC/DSEN/PR ↔ NOFORN/ORCON/IMCON/DEA SENSITIVE/PROPIN);
those form-pair concerns are now owned by:

- `capco:banner.metadata.uses-portion-form`
- `capco:portion.metadata.uses-banner-form`

(both restored in issue #677 per CAPCO-2016 §D.1 p27 + §C.1 p25 +
§G.1 Table 4 p38). The retired rule IDs covering that ground were
`E001` (`PortionMarkInBannerRule`) and `E009` (`PortionAbbreviationRule`),
both retired in PR 3c.B Commit 6.

The inline comment at the E006 fix site explains why the guard is
absent today; the rule-ID lineage above is the cross-ref.

## C001 (`CorrectionsMapRule`)

C001 is the org-policy corrections-map walker, with no CAPCO authority
(it consumes `.marque.toml` `[corrections]` entries, not a CAPCO
passage). The retired W001 (`DeprecatedMarkingWarningRule`) had been
the intended home for org-policy deprecations like FOUO transitional
warnings before its T035c-14 retirement closed that channel. The
post-T035c-14 path for an org-policy deprecation is a new rule with
org-config authority, not CAPCO §F. C001 is the closest live analogue
on the org-config axis.

## E002 (`MissingUsaTrigraphRule`)

E002 owns "USA missing or not first in REL TO" per CAPCO-2016
§H.8 p150–151. Two retired rules were in this neighborhood:

- `E020` (REL TO USA-first + alpha ordering) — rolled into the `E060`
  walker in PR 3b.F, then `E060` retired in PR 3c.B Commit 6 into
  `MarkingScheme::render_canonical`'s REL TO axis (the rule-level
  USA-first sort moved into the renderer).
- `E052` (REL TO no-duplicates) — retired in PR 3c.B Commit 6 into
  the renderer's dedup pass.

E002's fix produces a fully canonical list (USA first, non-USA
trigraphs in alphabetical order) so an E002 fix does not leave a
latent ordering violation behind for a second pass — this single-pass
canonicalization is what the 0.97 confidence is predicated on.

## S005 (`RelToOpaqueUncertainReductionSuggestRule`)

Pre-PR-#488 this was a Suggest/Info pair (`S005` Suggest + `S006`
Info). PR #488 collapsed the pair into a single Suggest-severity rule
under `Phase::PageFinalization` and retired S006. The §H.8 + §D.2
Table 3 rule 21 authorities do not distinguish "active validation"
from "consistent case"; the pre-#488 split was an engine workaround
(per-rule severity override was the only way to surface two severities
for one trigger).

## E041 (`NodisSupersedesExdisInPortionRule`)

E041 is one of three NODIS/EXDIS rules — its peers `E037`
(mutual-exclusion) and `E038` (require-NOFORN) retired into the
engine bridge per PR #578. The two surviving rules in this neighborhood
are E041 (portion-level NODIS supersedes EXDIS) and E039 (REL TO clear
in banner with NODIS/EXDIS portion). Authorities for all five
NODIS/EXDIS rules trace to §H.9 p172 + p174.

## `BannerMatchesProjectedRule` (walker)

The walker subsumes the three retired literal rules `E031`
(SAR banner rollup), `E035` (SCI banner rollup), and `E040`
(NODIS/EXDIS banner rollup) per PR 3b Sub-move A (T026a). Emitted
diagnostics still carry per-row IDs (`E031` / `E035` / `E040`) for
audit-stream continuity, so a downstream consumer that pre-T044
indexed on those flat-string IDs continues to see them in the
emitted-row position; only the registering rule struct changed.
