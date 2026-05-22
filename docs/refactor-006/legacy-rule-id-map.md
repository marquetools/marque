<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Legacy Rule-ID Map — T044 cutover

**Date created:** 2026-05-22
**Audit-schema cutover:** `marque-1.0` → `marque-2.0`
**Authority:** T044 in `specs/006-engine-rule-refactor/tasks.md`; FR-026 / FR-044 / FR-049 / FR-035a;
`contracts/audit-record.md` §"Post-`marque-1.0` RuleId migration";
`docs/refactor-006/2026-05-22-T044-rule-id-tuple-plan.md` §1.5;
`docs/refactor-006/2026-05-22-T044-pm-decisions.md` OD-1 refinement (`closure` surface).
**Discipline:** Constitution VIII (Authoritative Source Fidelity) — every CAPCO §-citation
in this map was re-verified against `crates/capco/docs/CAPCO-2016.md` at this PR's
authorship (2026-05-22). Citations propagate; never invent. TBD rows are explicit.

This map records the one-time rename of every flat-string rule ID
(`E### / W### / C### / S### / R### / catalog-row-label / closure-rule-name / test-fixture-id`)
to its 2-tuple successor `(scheme, predicate_id)` at the T044 cutover.

**Post-cutover audit logs use the 2-tuple form exclusively.** No runtime
translation table exists (clean break per Constitution V; FR-037).

## Discipline (binding for future renames)

This document is **appended to, never silently rewritten**. A rename added
on date X does NOT erase the prior name from this document — both rows
survive, with a `superseded_by` column linking forward if a second rename
follows. The audit trail across renames lives here.

## Purpose

The map exists for archaeological purposes only:

- Historical commit messages reference `E054`, `W003`, etc.
- Prior-art docs in `docs/refactor-006/` and `docs/plans/` reference the same.
- CAPCO §-citation cross-references in `crates/capco/CAPCO-CONTEXT.md` may
  reference the legacy IDs.

A 2031 reader of a 2026 audit log uses this map to find the modern name of
the rule that fired.

## Predicate-ID convention (from `2026-05-22-T044-rule-id-tuple-plan.md` §1.3 + PM-decisions OD-1)

`<scheme>:<surface>.<category>.<predicate>` (or `<scheme>:<class>.<predicate>` for
engine sentinels with no document surface).

- **scheme** ∈ `{capco, engine, test}` today; future schemes (`cui`, `nato`, …)
  reserve the same shape.
- **surface** ∈ `{banner, portion, page, marking, closure}` — five permissible
  values. `closure` was added by PM-decisions OD-1 refinement so closure-operator
  inferences route through the audit-note channel without conflating with
  strict page-banner rules.
- **category** matches the lattice/axis category: `classification | sci | sar |
  dissem | fgi | nato | aea | declassification | fouo | banner-rollup | metadata |
  correction | recognition | fix | recanonicalize | deprecation`.
- **predicate** is descriptive English-with-hyphens, lowercase ASCII.

Engine sentinels use `<scheme>:<class>.<predicate>` two-segment form (no `<surface>`),
because the engine itself emits them without a document surface.

The wire-string form `"<scheme>:<predicate_id>"` is what users type in
`.marque.toml [rules]` keys and what CLI text renders. JSON output uses the
structured 2-tuple `{"scheme": "...", "predicate_id": "..."}` shape.

## Per-section index

- §1 — Active CAPCO rules registered in `CapcoRuleSet` (28 rules at HEAD,
  matches `EXPECTED_RULE_IDS` in `post_3b_registration_pin.rs`)
- §2 — Retired declarative-wrapper rules (PR #578; 15 rules; bridge-routed)
- §3 — Class-floor catalog rows (E058 walker; 27 rows in `class_floor.rs`)
- §4 — SCI per-system catalog rows (E059 walker; 5 rows in `sci_per_system.rs`)
- §5 — Banner-rollup walker per-row identities (E031 walker; 5 row IDs)
- §6 — Other walker / declarative-emit-only rules (E060/E065/E067/W005)
- §7 — Engine sentinels (scheme `engine`; R001 / R002)
- §8 — Closure rules (10 rows; `[closure_rules]` config keyspace)
- §9 — Core constraint catalog (`core_catalog.rs` non-bridge rows)
- §10 — Test-fixture rule IDs (scheme `test`)

---

## §1 Active CAPCO rules (registered in `CapcoRuleSet`)

The 28-rule registered set matches `EXPECTED_RULE_IDS` in
`crates/capco/tests/post_3b_registration_pin.rs:116-176`. Source-of-truth for
each row: `crates/capco/src/rules.rs` (rule `impl Rule for *` block + `name()`
+ `cited_authorities()`) and `crates/capco/src/rules_declarative.rs` for the
two declarative walkers (E065 / E067).

| Legacy ID | New 2-tuple | Wire string | CAPCO §-citation | Rule name | Description |
|---|---|---|---|---|---|
| `C001` | `("capco", "marking.correction.token-typo")` | `capco:marking.correction.token-typo` | §A.6 p17 | `corrections-map` | Pre-scanner text correction from `[corrections]` config map. |
| `E002` | `("capco", "portion.dissem.rel-to-missing-usa")` | `capco:portion.dissem.rel-to-missing-usa` | §H.8 p151 | `missing-usa-trigraph` | REL TO list does not lead with USA (and/or USA absent); single-pass canonicalize to USA-first + trigraphs-alpha + tetragraphs-alpha. (§H.8 p150 is the section anchor; p151 carries the verbatim USA-first rule.) |
| `E005` | `("capco", "portion.declassification.declassify-on-misplaced")` | `capco:portion.declassification.declassify-on-misplaced` | §E.1 p31 + §D.1 p27 | `declassify-misplaced` | `Declassify On` token appears in a portion or banner instead of the CAB. |
| `E006` | `("capco", "marking.deprecation.deprecated-dissem-control")` | `capco:marking.deprecation.deprecated-dissem-control` | §H.8 + §F p35 | `deprecated-dissem` | Deprecated dissemination-control token in source; suggest superseded replacement. |
| `E007` | `("capco", "portion.metadata.x-shorthand-date-pattern")` | `capco:portion.metadata.x-shorthand-date-pattern` | §E.4 pp33-34 | `x-shorthand-date` | `X#` shorthand date pattern in `Declassify On` outside the canonical YYYYMMDD form. |
| `E008` | `("capco", "marking.metadata.unrecognized-token")` | `capco:marking.metadata.unrecognized-token` | §G.1 p36 | `unrecognized-token` | Token does not appear in the ODNI ISM CVE vocabulary or in any §H per-marking template. |
| `E031` | `("capco", "banner.banner-rollup.matches-projected")` | `capco:banner.banner-rollup.matches-projected` | per-row §-citations (see §5) | `banner-matches-projected` | Walker registered ID; emits per-row diagnostics covering SAR roll-up (§H.5 p101), SCI roll-up (§H.4 per-system), NODIS/EXDIS roll-up (§H.9 p172 + p174), banner-class mismatch (§H.7 pp123-125), and banner-FGI-marker mismatch (§H.7 p124). |
| `E039` | `("capco", "page.dissem.nodis-exdis-clears-banner-rel-to")` | `capco:page.dissem.nodis-exdis-clears-banner-rel-to` | §H.9 p172 + p174 | `nodis-exdis-clears-banner-rel-to` | NODIS or EXDIS present anywhere on the page → REL TO banned in the banner; strip banner REL TO. |
| `E041` | `("capco", "portion.dissem.nodis-supersedes-exdis-in-portion")` | `capco:portion.dissem.nodis-supersedes-exdis-in-portion` | §H.9 p174 + p172 | `nodis-supersedes-exdis-in-portion` | NODIS supersedes EXDIS within the same portion; strip EXDIS. |
| `E061` | `("capco", "portion.sci.hcs-bare-at-confidential-legacy-remark")` | `capco:portion.sci.hcs-bare-at-confidential-legacy-remark` | §H.4 p62 | `hcs-bare-at-confidential-legacy-remark` | Bare `HCS` (no compartment) at `C`-class → legacy retire path; re-mark on derivative use. |
| `E062` | `("capco", "portion.sci.hcs-bare-suggest-subcompartment")` | `capco:portion.sci.hcs-bare-suggest-subcompartment` | §H.4 p62 | `hcs-bare-suggest-subcompartment` | Bare `HCS` (no compartment) at S/TS-class — suggest HCS-O / HCS-P / HCS-P-[SUB] per current grammar. |
| `E063` | `("capco", "portion.sci.rsv-bare-requires-compartment")` | `capco:portion.sci.rsv-bare-requires-compartment` | §H.4 p70 | `rsv-bare-requires-compartment` | Bare `RSV` (RESERVE control) without compartment → error (must carry `RSV-[COMP]`). |
| `E064` | `("capco", "portion.dissem.eyes-only-convert-to-rel-to")` | `capco:portion.dissem.eyes-only-convert-to-rel-to` | §H.8 p157 + p158 | `eyes-only-convert-to-rel-to` | EYES / EYES ONLY token (NSA-deprecated, waiver expired 2017-10-01) → REL TO conversion. |
| `E065` | `("capco", "portion.sci.deprecated-long-form")` | `capco:portion.sci.deprecated-long-form` | §H.4 pp 61, 62, 74, 76, 78, 85 | `deprecated-sci-long-form` | Deprecated SCI long-form tokens (HUMINT→HCS, COMINT/SPECIAL INTELLIGENCE→SI, ECI→SI-[COMP], EL/ENDSEAL→SI-[COMP], KDK/KLONDIKE→TK-[COMP]). Declarative walker, per-row citations. |
| `E066` | `("capco", "marking.recanonicalize.legacy-nato-compound")` | `capco:marking.recanonicalize.legacy-nato-compound` | §G.2 p40 + §H.7 p122 + §H.7 p127 | `legacy-nato-compound-remark` | Legacy NATO compound text (`CTSA`/`CTS-A`/`CTS-B`/`CTS-BALK`/`NSAT`/`NS-A`/`NCA`/`NC-A`, plus banner-form equivalents) → canonical multi-block form. |
| `E067` | `("capco", "marking.recanonicalize.bare-canonical-compound")` | `capco:marking.recanonicalize.bare-canonical-compound` | §H.6 p106 + §H.4 p83 + §H.4 p78 | `bare-canonical-compound` | Bare legacy short-forms (CNWDI → RD-CNWDI; NK → SI-NK; EU → SI-EU). Declarative walker. |
| `E071` | `("capco", "portion.fgi.fgi-explicit-with-trigraph")` | `capco:portion.fgi.fgi-explicit-with-trigraph` | §H.7 p124 | `fgi-explicit-with-trigraph` | `FGI [trigraph]` when concealment is intended (Case A) or acknowledgment contradicted (Case C/D). Four-case behavioral spec covering full/empty/partial REL TO overlap. |
| `E072` | `("capco", "page.dissem.bare-rel-portion-divergence")` | `capco:page.dissem.bare-rel-portion-divergence` | §H.8 pp150-151 | `bare-rel-portion-divergence` | Bare-REL portions and explicit-REL-TO portions with divergent country lists coexist on the same page. `Phase::PageFinalization`. |
| `S003` | `("capco", "portion.classification.joint-usa-first-style")` | `capco:portion.classification.joint-usa-first-style` | §H.3 p56 + §H.8 pp150-151 | `joint-usa-first` | JOINT classification country-list — IC convention puts USA first even though §H.3 prescribes pure-alpha. Info severity. |
| `S004` | `("capco", "portion.dissem.rel-to-trigraph-suggest")` | `capco:portion.dissem.rel-to-trigraph-suggest` | §H.8 | `rel-to-trigraph-suggest` | REL TO trigraph has a corpus-rare prior and a corpus-common 1- or 2-edit neighbor (e.g. `AUT` → `AUS?`). First consumer of the suggest-don't-fix channel. |
| `S005` | `("capco", "page.dissem.rel-to-uncertain-reduction")` | `capco:page.dissem.rel-to-uncertain-reduction` | §H.8 + §D.2 Table 3 row 21 | `rel-to-opaque-uncertain-reduction` | REL TO membership-uncertain reduction at `Phase::PageFinalization`. PR #488 collapsed S005/S006 split into one Suggest. |
| `S007` | `("capco", "portion.nato.bare-nato-requires-rel-to-usa-nato")` | `capco:portion.nato.bare-nato-requires-rel-to-usa-nato` | §H.7 p127 | `bare-nato-requires-rel-to-usa-nato` | Bare NATO classification (NU/NR/NC/NS/CTS) in a US-classified document should carry `REL TO USA, NATO` (Notional Example 2). |
| `S008` | `("capco", "portion.dissem.relido-implied-by-closure")` | `capco:portion.dissem.relido-implied-by-closure` | §H.8 p154 + §D.2 Table 3 row 17 | `relido-implied-by-closure` | Byte-surfacing twin of `capco/relido-if-sci-and-not-incompatible` / `capco/relido-if-us-collateral-class` closure rules — emits `FactAdd(RELIDO, Scope::Portion)` Suggest at confidence 0.85. |
| `S009` | `("capco", "page.dissem.prefer-tetragraph-collapse")` | `capco:page.dissem.prefer-tetragraph-collapse` | §H.8 p150 | `prefer-tetragraph-collapse` | Suggest replacing an explicit member trigraph list with a compact tetragraph (FVEY/TEYE/ACGU) when all members are present. Default Off. |
| `S010` | `("capco", "page.dissem.collapse-uniform-rel-portions")` | `capco:page.dissem.collapse-uniform-rel-portions` | §H.8 p150 | `collapse-uniform-rel-portions` | When all portions with explicit REL TO carry the same list as the banner, suggest the compact `REL` form. `Phase::PageFinalization`. Default Off. |
| `W003` | `("capco", "page.dissem.non-ic-dissem-in-classified-banner")` | `capco:page.dissem.non-ic-dissem-in-classified-banner` | §H.9 (precedence rules for banner-line guidance) | `non-ic-dissem-in-classified-banner` | Non-IC dissemination control surfaces in a classified banner where the IC convention forbids it. Warn. |
| `W004` | `("capco", "page.fgi.joint-disunity-collapses-to-fgi")` | `capco:page.fgi.joint-disunity-collapses-to-fgi` | §H.3 p57 + §H.7 p123 | `joint-disunity-collapse` | All-JOINT portions disagree on producer lists; `JointSet::DisunityCollapse` fires the cross-axis migration to FGI per "Derivative Use" bullets. Warn. |
| `W034` | `("capco", "portion.sci.unpublished-custom-control")` | `capco:portion.sci.unpublished-custom-control` | §A.6 p15 + §H.4 p61 | `sci-custom-control-info` | Unpublished (agency-allocated) SCI custom control identifier within typical CAPCO §A.6 p15 length bounds; Info severity. |

**Cardinality check (sanity for Wave 2 Agent D):** the table above has
**28 rows** — matches `EXPECTED_RULE_IDS.len() == 28` in
`post_3b_registration_pin.rs`. Any future addition or retirement of a
registered rule lands a new row here AND updates that test in lockstep.

---

## §2 Retired declarative-wrapper rules (PR #578)

The 15 declarative-wrapper rules retired in PR #578 still emit diagnostics —
through the engine's constraint-catalog bridge at
`crates/engine/src/engine.rs:2406-2438`. The bridge currently recovers the
legacy `E###`/`W###` prefix from the catalog row's `name` field
(`v.constraint_label.split('/').next()`). Post-T044 the bridge becomes a
no-op pass-through: the catalog row's `name` IS the predicate ID (per
plan §1.5 + OD-8.A).

The catalog rows currently live in `crates/capco/src/scheme/constraints/core_catalog.rs`.

| Legacy ID | Catalog row label (pre-T044) | New 2-tuple (post-T044 — `name` IS the predicate) | Wire string | CAPCO §-citation | Description |
|---|---|---|---|---|---|
| `E010` | `E010/HCS-system-constraints` | `("capco", "portion.sci.hcs-system-constraints")` | `capco:portion.sci.hcs-system-constraints` | §H.4 (HCS family) | HCS control-system structural constraints (HCS-O/HCS-P/HCS-P-[SUB] companion + class-floor pairing). |
| `E012` | `E012/dual-classification` | `("capco", "portion.classification.dual-classification")` | `capco:portion.classification.dual-classification` | §A.6 + §H.1 + §H.2 + §H.3 | Two classification tokens in the same portion (e.g., US class + non-US class without proper segregation). |
| `E014` | `E014/joint-requires-rel-to-coverage` | `("capco", "portion.classification.joint-requires-rel-to-coverage")` | `capco:portion.classification.joint-requires-rel-to-coverage` | §H.3 p56 | JOINT classification requires `REL TO USA, LIST` covering the JOINT country list. |
| `E015` | `E015/non-us-requires-dissem` | `("capco", "portion.classification.non-us-requires-dissem")` | `capco:portion.classification.non-us-requires-dissem` | §H.2 + §A.6 | Non-US classification portion requires a dissemination-control marking. |
| `E016` | `E016/joint-conflicts-restricted` | `("capco", "portion.classification.joint-conflicts-restricted")` | `capco:portion.classification.joint-conflicts-restricted` | §H.3 p56 | JOINT classification is mutually exclusive with `RESTRICTED` (allowed levels are TS/S/C/U only). |
| `E021` | `E021/rd-frd-requires-noforn` | `("capco", "portion.aea.rd-frd-requires-noforn")` | `capco:portion.aea.rd-frd-requires-noforn` | §H.6 p104 + p111 | RD / FRD require NOFORN unless §123/§144 sharing agreement applies. |
| `E024` | `E024/rd-precedence` | `("capco", "portion.aea.rd-precedence")` | `capco:portion.aea.rd-precedence` | §H.6 p104 | RD takes precedence over FRD / TFNI within a portion; FRD / TFNI evicted when RD is present. |
| `E036` | `E036/joint-conflicts-hcs` | `("capco", "portion.classification.joint-conflicts-hcs")` | `capco:portion.classification.joint-conflicts-hcs` | §H.3 p57 | JOINT classification is mutually exclusive with HCS (the only specific JOINT exclusion §H.3 names). |
| `E037` | `E037/nodis-conflicts-exdis` | `("capco", "portion.dissem.nodis-conflicts-exdis")` | `capco:portion.dissem.nodis-conflicts-exdis` | §H.9 p174 + p172 | NODIS and EXDIS are mutually exclusive within a portion. |
| `E038` | `E038/nodis-or-exdis-requires-noforn` | `("capco", "portion.dissem.nodis-or-exdis-requires-noforn")` | `capco:portion.dissem.nodis-or-exdis-requires-noforn` | §H.9 p172 + p174 | NODIS or EXDIS requires NOFORN. |
| `E053` | `capco/noforn-conflicts-rel-to` | `("capco", "portion.dissem.noforn-conflicts-rel-to")` | `capco:portion.dissem.noforn-conflicts-rel-to` | §H.8 p145 + pp150-151 | NOFORN and REL TO are mutually exclusive within a portion (NOFORN dominates per Table 3 row 2). |
| `E054` | `E054/relido-conflicts-noforn` | `("capco", "portion.dissem.relido-conflicts-noforn")` | `capco:portion.dissem.relido-conflicts-noforn` | §H.8 p145 + p154 | RELIDO and NOFORN are mutually exclusive within a portion. |
| `E055` | `capco/display-only-clears-relido` | `("capco", "portion.dissem.display-only-clears-relido")` | `capco:portion.dissem.display-only-clears-relido` | §H.8 p154 + p163 | DISPLAY ONLY clears RELIDO within a portion (DISPLAY ONLY supersedes RELIDO). |
| `E056` | `capco/orcon-clears-relido` | `("capco", "portion.dissem.orcon-clears-relido")` | `capco:portion.dissem.orcon-clears-relido` | §H.8 p136 + p154 | ORCON clears RELIDO within a portion. |
| `E057` | `capco/orcon-usgov-clears-relido` | `("capco", "portion.dissem.orcon-usgov-clears-relido")` | `capco:portion.dissem.orcon-usgov-clears-relido` | §H.8 p140 + p154 | ORCON-USGOV clears RELIDO within a portion. |

Wave-2 Agent A renames the catalog row `name` fields in
`core_catalog.rs` to the new predicate-ID strings; Wave-2 Agent B rewrites
the bridge dispatch at `engine.rs:2406-2438` to drop the `split('/').next()`
prefix recovery (per plan §2.2 + OD-8.A).

---

## §3 Class-floor catalog (E058 walker, 27 rows)

Per `crates/capco/src/scheme/class_floor.rs::CLASS_FLOOR_CATALOG` (27 rows
positionally-pinned by `crates/capco/tests/class_floor_catalog.rs`). Each
row's CAPCO §-citation lives on the `citation` field of the row literal and
was verified inline at row authorship; the citations are duplicated here for
the rename target.

| Catalog row label (pre-T044) | New 2-tuple (post-T044) | Wire string | CAPCO §-citation | Description |
|---|---|---|---|---|
| `class-floor/HCS-comp-sub` | `("capco", "banner.classification.floor-hcs-comp-sub")` | `capco:banner.classification.floor-hcs-comp-sub` | §H.4 p60 (anchor); per-system §H.4 p68 | HCS sub-compartment markings require TS-class floor. |
| `class-floor/SI-comp` | `("capco", "banner.classification.floor-si-comp")` | `capco:banner.classification.floor-si-comp` | §H.4 p60 (per-system §H.4 p76) | SI compartments require TS-class floor. |
| `class-floor/TK-BLFH` | `("capco", "banner.classification.floor-tk-blfh")` | `capco:banner.classification.floor-tk-blfh` | §H.4 p60 (per-system §H.4 p87) | TK-BLFH (BLUEFISH) requires TS-class floor. |
| `class-floor/BALK` | `("capco", "banner.classification.floor-balk")` | `capco:banner.classification.floor-balk` | §G.2 p40 | BALK (NATO SAP) typically requires TS-class. Warn severity (citation depth too soft for Error per PR 9c.1 D5). |
| `class-floor/BOHEMIA` | `("capco", "banner.classification.floor-bohemia")` | `capco:banner.classification.floor-bohemia` | §G.2 p40 | BOHEMIA (NATO SAP) typically requires TS-class. Warn severity. |
| `class-floor/HCS-comp` | `("capco", "banner.classification.floor-hcs-comp")` | `capco:banner.classification.floor-hcs-comp` | §H.4 p60 (per-system §H.4 p64 + p66) | HCS-O / HCS-P (no sub-compartment) requires S-class floor. |
| `class-floor/RSV-comp` | `("capco", "banner.classification.floor-rsv-comp")` | `capco:banner.classification.floor-rsv-comp` | §H.4 p60 (per-system §H.4 p72) | RSV-[COMP] requires S-class floor. |
| `class-floor/TK` | `("capco", "banner.classification.floor-tk")` | `capco:banner.classification.floor-tk` | §H.4 p60 (per-system §H.4 p85) | TK / TK-IDIT / TK-KAND requires S-class floor. |
| `class-floor/RD-SG` | `("capco", "banner.aea.floor-rd-sg")` | `capco:banner.aea.floor-rd-sg` | §H.6 p108 | RD-SIGMA requires S-class floor. (RD-SIGMA template on p108; pre-T044 the row drifted to p113 — that's FRD-SIGMA's page. Corrected per Constitution VIII propagation-trigger discipline.) |
| `class-floor/FRD-SG` | `("capco", "banner.aea.floor-frd-sg")` | `capco:banner.aea.floor-frd-sg` | §H.6 p113 | FRD-SIGMA requires S-class floor. |
| `E058/CNWDI-classification-floor` | `("capco", "banner.aea.floor-cnwdi")` | `capco:banner.aea.floor-cnwdi` | §H.6 p104 | CNWDI requires S-class floor (replaces retired E022). Walker-prefixed `E058/` per PR D R3.2 naming-prefix invariant. |
| `class-floor/RSEN` | `("capco", "banner.dissem.floor-rsen")` | `capco:banner.dissem.floor-rsen` | §H.8 p149 | RSEN requires S-class floor. |
| `class-floor/IMCON` | `("capco", "banner.dissem.floor-imcon")` | `capco:banner.dissem.floor-imcon` | §H.8 p144 | IMCON requires S-class floor. |
| `class-floor/SI` | `("capco", "banner.classification.floor-si")` | `capco:banner.classification.floor-si` | §H.4 p60 (per-system §H.4 p74) | Bare SI requires C-class floor. |
| `E058/SAR-classification-floor` | `("capco", "banner.classification.floor-sar")` | `capco:banner.classification.floor-sar` | §H.5 p99 | SAR requires C-class floor (replaces retired E027). |
| `class-floor/RD` | `("capco", "banner.aea.floor-rd")` | `capco:banner.aea.floor-rd` | §H.6 p104 | Bare RD requires C-class floor. |
| `class-floor/FRD` | `("capco", "banner.aea.floor-frd")` | `capco:banner.aea.floor-frd` | §H.6 p104 | Bare FRD requires C-class floor. |
| `class-floor/TFNI` | `("capco", "banner.aea.floor-tfni")` | `capco:banner.aea.floor-tfni` | §H.6 p107 | TFNI requires C-class floor. |
| `class-floor/ATOMAL` | `("capco", "banner.aea.floor-atomal")` | `capco:banner.aea.floor-atomal` | §H.7 p122 | ATOMAL (NATO AEA marking) requires C-class floor. PR 9c.1 T134 reclassified as AEA-axis (not NATO-class portion suffix). |
| `class-floor/ORCON` | `("capco", "banner.dissem.floor-orcon")` | `capco:banner.dissem.floor-orcon` | §H.8 p136 | ORCON / ORCON-USGOV requires C-class floor. |
| `class-floor/EYES-ONLY` | `("capco", "banner.dissem.floor-eyes-only")` | `capco:banner.dissem.floor-eyes-only` | §H.8 p152 | EYES ONLY requires C-class floor (deprecated marker, waiver expired). |
| `E058/DOD-UCNI-classification-ceiling` | `("capco", "banner.aea.ceiling-dod-ucni")` | `capco:banner.aea.ceiling-dod-ucni` | §H.6 p116 | DOD UCNI requires UNCLASSIFIED equality (ceiling, not floor — replaces retired E025). |
| `E058/DOE-UCNI-classification-ceiling` | `("capco", "banner.aea.ceiling-doe-ucni")` | `capco:banner.aea.ceiling-doe-ucni` | §H.6 p118 | DOE UCNI requires UNCLASSIFIED equality (ceiling). |
| `class-floor/passthrough-BUR` | `("capco", "banner.classification.floor-passthrough-bur")` | `capco:banner.classification.floor-passthrough-bur` | engine-internal (`marque-applied.md` §3.7) | Unknown-floor passthrough for `BUR` family — open-vocab ISM-known token outside the closed atom inventory. Warn. |
| `class-floor/passthrough-HCS-X` | `("capco", "banner.classification.floor-passthrough-hcs-x")` | `capco:banner.classification.floor-passthrough-hcs-x` | engine-internal (`marque-applied.md` §3.7) | Unknown-floor passthrough for `HCS-X`. Warn. |
| `class-floor/passthrough-KLM` | `("capco", "banner.classification.floor-passthrough-klm")` | `capco:banner.classification.floor-passthrough-klm` | engine-internal (`marque-applied.md` §3.7) | Unknown-floor passthrough for `KLM` family. Warn. |
| `class-floor/passthrough-MVL` | `("capco", "banner.classification.floor-passthrough-mvl")` | `capco:banner.classification.floor-passthrough-mvl` | engine-internal (`marque-applied.md` §3.7) | Unknown-floor passthrough for `MVL`. Warn. |

**Cardinality check:** the table above has **27 rows** — matches
`CLASS_FLOOR_CATALOG.len()`. The 4 walker-prefixed rows (`E058/...`)
are the rows replacing retired legacy rules (E022 → CNWDI, E027 → SAR,
E025 → DOD-UCNI + DOE-UCNI), per PR D R3.2 naming-prefix invariant.
Source: `crates/capco/src/scheme/class_floor.rs`.

Wave-2 Agent A renames the `name` field on each `ClassFloorRow` literal;
the `ConstraintViolation::label` source-of-truth shape on the catalog
declaration is unchanged.

---

## §4 SCI per-system catalog (E059 walker, 5 rows)

Per `crates/capco/src/scheme/sci_per_system.rs::SCI_PER_SYSTEM_CATALOG`.
Mirror catalog at `crates/capco/src/scheme/constraints/sci_per_system_catalog.rs`.

| Catalog row label (pre-T044) | New 2-tuple (post-T044) | Wire string | CAPCO §-citation | Description |
|---|---|---|---|---|
| `sci-per-system/HCS-O-companions` | `("capco", "marking.sci.hcs-o-companions")` | `capco:marking.sci.hcs-o-companions` | §H.4 p64 | HCS-O requires ORCON + NOFORN companions. |
| `sci-per-system/HCS-P-NOFORN` | `("capco", "marking.sci.hcs-p-noforn-required")` | `capco:marking.sci.hcs-p-noforn-required` | §H.4 p66 | HCS-P requires NOFORN companion. |
| `sci-per-system/HCS-P-sub-companions` | `("capco", "marking.sci.hcs-p-sub-companions")` | `capco:marking.sci.hcs-p-sub-companions` | §H.4 p68 | HCS-P [SUB] requires HCS-P + ORCON + NOFORN. |
| `sci-per-system/SI-G-companions` | `("capco", "marking.sci.si-g-companions")` | `capco:marking.sci.si-g-companions` | §H.4 p80 | SI-G requires SI + ORCON. |
| `sci-per-system/TK-compartment-NOFORN` | `("capco", "marking.sci.tk-compartment-noforn-required")` | `capco:marking.sci.tk-compartment-noforn-required` | §H.4 p87 + p91 + p95 | TK-BLFH / TK-IDIT / TK-KAND require NOFORN. |

Wave-2 Agent A renames the `name` fields in both
`sci_per_system.rs` and `sci_per_system_catalog.rs` (mirror) — and also
deletes the `RULE_E059` const at `sci_per_system.rs:93` per plan §2.6.

---

## §5 Banner-rollup walker (E031, 5 row identities)

Per `crates/capco/src/rules.rs:5291-5357` (`BannerMatchesProjectedRule`
per-row catalog). The walker is registered as `E031` (per §1 above), but the
catalog emits diagnostics with 5 distinct rule IDs:

| Legacy per-row ID | New 2-tuple | Wire string | CAPCO §-citation | Description |
|---|---|---|---|---|
| `E031` (walker registration + SAR roll-up) | `("capco", "banner.banner-rollup.sar-portions-roll-up")` | `capco:banner.banner-rollup.sar-portions-roll-up` | §H.5 p101 | Unique SARs from portions must appear in banner. |
| `E035` (SCI roll-up) | `("capco", "banner.banner-rollup.sci-portions-roll-up")` | `capco:banner.banner-rollup.sci-portions-roll-up` | §H.4 per-system precedence + §D.2 p28 | Unique SCI control systems from portions must roll up to banner. |
| `E040` (NODIS / EXDIS roll-up) | `("capco", "banner.banner-rollup.non-ic-dissem-roll-up")` | `capco:banner.banner-rollup.non-ic-dissem-roll-up` | §H.9 p174 (NODIS) + §H.9 p172 (EXDIS) | NODIS supersedes EXDIS in banner; banner must reflect the strongest non-IC dissem. |
| `E068` (banner-class mismatch) | `("capco", "banner.classification.mismatch-vs-projected")` | `capco:banner.classification.mismatch-vs-projected` | §H.7 pp123-125 | Observed banner classification disagrees with the cross-portion projection (reciprocal classification grammar). Pure no-fix Error. |
| `E069` (banner-FGI-marker mismatch) | `("capco", "banner.fgi.marker-mismatch-vs-projected")` | `capco:banner.fgi.marker-mismatch-vs-projected` | §H.7 p124 | Observed banner FGI marker (concealed vs acknowledged) disagrees with cross-portion projection. |

Walker registration (`Rule::id() = RuleId::new("E031")`) renames to the SAR
row ID per §1 — the walker's registered tuple IS the SAR roll-up tuple.
Per `additional_emitted_ids` contract.

---

## §6 Other walker / declarative-emit-only rules

| Legacy ID | Status | Source location | Notes |
|---|---|---|---|
| `E058` (DeclarativeClassFloorRule walker) | Walker no longer a registered `Rule` impl. Per-row IDs in §3. | retired in PR 3c.B Commit 7.3; bridge-emitted only | Bridge folds all `class-floor/...` and `E058/...` row labels to `Diagnostic.rule = "E058"` today; post-T044 each row carries its own predicate ID per §3. |
| `E059` (DeclarativeSciPerSystemRule walker) | Walker no longer a registered `Rule` impl. Per-row IDs in §4. | retired in PR 3c.B Commit 7.4; bridge-emitted only | Same shape as E058 — bridge folds today; per-row predicate IDs post-T044 per §4. |
| `E060` (DeclarativeNonCanonicalInputRule walker) | Retired entirely in PR 3c.B Commit 6 | n/a | Five rows (REL TO / JOINT / SIGMA / SAR / SCI ordering) absorbed by `MarkingScheme::render_canonical` per `marque-applied.md` §3.6 + §3.10 Move 7. No predicate-ID rename target — the rule does not emit post-PR 3c.B. |
| `E070` (FRD/TFNI precedence) | Catalog row in `core_catalog.rs:289-300`; bridge-emitted | core_catalog.rs `name = "E070/frd-tfni-precedence"` | New 2-tuple: `("capco", "portion.aea.frd-tfni-precedence")` — wire `capco:portion.aea.frd-tfni-precedence`. Citation §H.6 p120 (TFNI subsection: "If the TFNI marking is used with RD or FRD, RD or FRD takes precedence"). Description: FRD takes precedence over TFNI within a portion (FRD evicts TFNI when both present). |
| `W005` (rel-to-not-in-joint-coverage) | Catalog row in `core_catalog.rs:112-128`; bridge-emitted Warn | core_catalog.rs `name = "W005/rel-to-not-in-joint-coverage"` | New 2-tuple: `("capco", "portion.classification.rel-to-not-in-joint-coverage")` — wire `capco:portion.classification.rel-to-not-in-joint-coverage`. Citation §H.3 p56. Description: REL TO list contains a country not present in the JOINT producer list (the `RelToExpandsBeyondJoint` warn per PR #666/#678). |
| `capco/joint-requires-usa` | Catalog row in `core_catalog.rs:344-355`; bridge-emitted | core_catalog.rs row | New 2-tuple: `("capco", "portion.classification.joint-requires-usa")` — wire `capco:portion.classification.joint-requires-usa`. Citation §H.3 p55-56. Description: JOINT classification requires USA in the country list (US must be a co-owner). |

---

## §7 Engine sentinels (scheme `engine`)

Per plan §1.4 + PM-decisions OD-4 (drop the `r001`/`r002` numeric prefix).

| Legacy ID | New 2-tuple | Wire string | Description | Source |
|---|---|---|---|---|
| `R001` (decoder-recognized) | `("engine", "recognition.decoder-recognized")` | `engine:recognition.decoder-recognized` | Decoder-path recognition synthetic diagnostic (Phase D probabilistic recognizer emitted a marking; the engine surfaces this as a tagged diagnostic). | `crates/engine/src/engine.rs:113` (`DECODER_RULE_ID`) + `:4341` (consumer site). |
| `R002` (reparse-failed) | `("engine", "fix.reparse-failed")` | `engine:fix.reparse-failed` | Two-pass fixer re-parse failure synthetic diagnostic. | `crates/engine/src/engine.rs:147` (`R002_RULE_ID`). |

The `"engine"` scheme is reserved (alongside `"test"`) — not a valid
`MarkingScheme` registration target. A grep-fence in
`crates/rules/src/lib.rs` doc-comment names both reserved schemes
explicitly.

---

## §8 Closure rules (audit-note channel, `[closure_rules]` config section)

Per PM-decisions OD-1 refinement. The closure surface is
`closure.<category>.<predicate>` — `closure` is the fifth permissible
`<surface>` value, added because closure-operator inferences don't fire at
a document surface (they're page-level inferences over the marking lattice
that surface via the audit-note channel, not the diagnostic channel).

The renames also update the `[closure_rules]` config-section keyspace —
users typing `[closure_rules] "capco:closure.dissem.noforn-if-caveated" =
"warn"` get the same effect as today's `[closure_rules]
"capco/noforn-if-caveated" = "warn"`. The `AuditNote.structural.row_name`
field (currently `&'static str` carrying the slash form) migrates to the
wire-string form; type unchanged, content updates.

Per `crates/capco/src/scheme/closure_table.rs::CLOSURE_TABLE` (10 rows,
positionally pinned by inline tests `catalog_has_ten_rows` +
`row_names_match_fn_pointer_catalog`).

| Legacy `ClosureRule.name` | New wire string (also config-section key) | New 2-tuple form (for `AuditNote.rule`) | CAPCO §-citation | Description |
|---|---|---|---|---|
| `capco/noforn-if-caveated` | `capco:closure.dissem.noforn-if-caveated` | `("capco", "closure.dissem.noforn-if-caveated")` | §B.3 Table 2 p21 | Classified-and-caveated post-28-Jun-2010 portion → NOFORN. Trio 1 fundamental closure. |
| `capco/hcs-o-implies-noforn-orcon` | `capco:closure.dissem.hcs-o-implies-noforn-orcon` | `("capco", "closure.dissem.hcs-o-implies-noforn-orcon")` | §H.4 p64 | HCS-O implies NOFORN + ORCON. Per-marking unconditional. |
| `capco/hcs-p-sub-implies-noforn-orcon` | `capco:closure.dissem.hcs-p-sub-implies-noforn-orcon` | `("capco", "closure.dissem.hcs-p-sub-implies-noforn-orcon")` | §H.4 p68 | HCS-P [SUB] implies NOFORN + ORCON. Per-marking unconditional. |
| `capco/si-g-implies-orcon` | `capco:closure.dissem.si-g-implies-orcon` | `("capco", "closure.dissem.si-g-implies-orcon")` | §H.4 p80 | SI-G implies ORCON (NOFORN intentionally NOT in direct cone per §H.4 p80 example banner; Trio 1 adds NOFORN transitively via ORCON in caveated trigger). |
| `capco/tk-blfh-implies-noforn` | `capco:closure.dissem.tk-blfh-implies-noforn` | `("capco", "closure.dissem.tk-blfh-implies-noforn")` | §H.4 p87 | TK-BLFH implies NOFORN. Per-marking unconditional. |
| `capco/tk-idit-implies-noforn` | `capco:closure.dissem.tk-idit-implies-noforn` | `("capco", "closure.dissem.tk-idit-implies-noforn")` | §H.4 p91 | TK-IDIT implies NOFORN. Per-marking unconditional. |
| `capco/tk-kand-implies-noforn` | `capco:closure.dissem.tk-kand-implies-noforn` | `("capco", "closure.dissem.tk-kand-implies-noforn")` | §H.4 p95 | TK-KAND implies NOFORN. Per-marking unconditional. |
| `capco/rel-to-usa-nato-if-nato-classification` | `capco:closure.nato.rel-to-usa-nato-if-nato-classification` | `("capco", "closure.nato.rel-to-usa-nato-if-nato-classification")` | §H.7 p127 | Bare NATO classification → `REL TO USA, NATO` (NATO transmutes to FGI when commingled with US). Trio 3. |
| `capco/relido-if-sci-and-not-incompatible` | `capco:closure.dissem.relido-if-sci-and-not-incompatible` | `("capco", "closure.dissem.relido-if-sci-and-not-incompatible")` | §H.8 p154 | SCI presence implies RELIDO unless FD&R or RELIDO-incompatible marker (NOFORN / DISPLAY ONLY / ORCON / ORCON-USGOV) suppresses. Trio 2. |
| `capco/relido-if-us-collateral-class` | `capco:closure.dissem.relido-if-us-collateral-class` | `("capco", "closure.dissem.relido-if-us-collateral-class")` | §B.3 Table 2 p21 + §H.8 p154 | US collateral classification implies RELIDO unless FD&R-marked or per-compartment SCI sentinel present. Trio 2. |

**Cardinality check:** 10 rows — matches
`crates/capco/src/scheme/closure_table.rs::tests::catalog_has_ten_rows`
and the row-name pin at `row_names_match_fn_pointer_catalog:461-471`.

The other half of the closure-rule surface is `CLOSURE_REL_TO_USA_NATO` at
`crates/capco/src/scheme/closure.rs:321` — currently the only
`ClosureRule<CapcoScheme>` exported via `CAPCO_CLOSURE_RULES`. The 9 other
`closure_table.rs::CLOSURE_TABLE` rows are the post-PR-4b-D bitmask
Kleene-fixpoint replacement; both surfaces carry the same names and the
same predicate-ID rename applies.

Wave-2 Agent A renames both surfaces in lockstep; the
`row_names_match_fn_pointer_catalog` test in `closure_table.rs` is the
positional pin that catches drift.

---

## §9 Core constraint catalog (non-bridge rows)

Two rows in `crates/capco/src/scheme/constraints/core_catalog.rs` are
referenced by source comments / messages but not directly bridge-routed
to an `E###`/`W###` legacy ID (they bridge through the `else { E008 }`
fallback today). These also rename per the catalog-row-IS-predicate-ID
shape of OD-8.A.

| Catalog row label (pre-T044) | New 2-tuple (post-T044) | Wire string | CAPCO §-citation | Description |
|---|---|---|---|---|
| `capco/noforn-conflicts-rel-to` | already listed in §2 (E053) — `("capco", "portion.dissem.noforn-conflicts-rel-to")` | `capco:portion.dissem.noforn-conflicts-rel-to` | §H.8 p145 + pp150-151 | (Cross-reference §2.) NOFORN + REL TO mutual exclusion; bridge currently special-cases this row at engine.rs:2431 → E053. Post-T044 the special case retires (OD-8.A no-op pass-through). |
| `capco/joint-requires-usa` | `("capco", "portion.classification.joint-requires-usa")` | `capco:portion.classification.joint-requires-usa` | §H.3 p55-56 | JOINT classification requires USA in the country list. Cross-referenced from §6. |

PageRewrite labels referenced in source comments
(`capco/noforn-clears-rel-to`, `capco/noforn-clears-display-only-to`,
`capco/noforn-clears-fdr-family`, `capco/display-only-clears-relido`,
`capco/orcon-clears-relido`, `capco/orcon-usgov-clears-relido`,
`capco/limdis-evicted-by-classified`, `capco/sbu-evicted-by-classified`,
`capco/dod-ucni-promotes-noforn-when-classified`,
`capco/dod-ucni-evicted-by-classified`, `capco/doe-ucni-promotes-noforn-when-classified`,
`capco/doe-ucni-evicted-by-classified`, `capco/fouo-evicted-by-classified`,
`capco/classification-evicts-fouo`, `capco/non-fdr-control-evicts-fouo`,
`capco/sbu-nf-evicted-by-classified`, `capco/sbu-nf-supersedes-sbu`,
`capco/les-nf-supersedes-les`, `capco/relido-conflicts-fdr-family`,
`capco/orcon-family-conflicts-relido`) are `PageRewrite::label` fields,
not rule IDs — they do not surface in `Diagnostic.rule` or
`AppliedFix.rule`. They live in the `PageRewrite` catalog
(post_4b_lattice_inventory_pin.rs positional pin) and are out of scope for
the T044 rule-ID rename. If a future PR routes PageRewrite labels through
the audit-note channel and gives them user-facing IDs, this section adds
rows then.

---

## §10 Test-fixture rule IDs (scheme `test`)

Per plan §1.7 + PM-decisions test-fixture-scheme reservation. Every
test-fixture ID in `#[cfg(test)]` modules, `tests/` integration files, and
`dev-dependencies`-gated test-utility crates.

The `"test"` scheme is reserved (alongside `"engine"`) — not a valid
`MarkingScheme` registration target. Wave-2 Agent C adds the grep-fence in
`crates/rules/src/lib.rs` doc-comment listing both reserved schemes.

| Legacy ID | New 2-tuple | Wire string | Test file (source) | Purpose |
|---|---|---|---|---|
| `E997` | `("test", "synthetic.e997-fixture")` | `test:synthetic.e997-fixture` | `crates/engine/src/engine.rs:5952` + `:5966` | Synthetic engine-test rule for harness exercises. |
| `E998` | `("test", "synthetic.e998-fixture")` | `test:synthetic.e998-fixture` | (per callsite inventory §6; no current direct callsite found at HEAD) | Reserved test sentinel per inventory. Wave-2 Agent C confirms call sites. |
| `E999` | `("test", "synthetic.e999-fixture")` | `test:synthetic.e999-fixture` | `crates/engine/src/engine.rs:7566` | Synthetic engine-test rule for harness exercises. |
| `S999` | `("test", "synthetic.s999-fixture")` | `test:synthetic.s999-fixture` | `crates/engine/src/engine.rs:6009` + `:6036`; `marque/src/render.rs:1310` | Synthetic Suggest-severity test rule. |
| `R999` | `("test", "synthetic.r999-fixture")` | `test:synthetic.r999-fixture` | `crates/engine/tests/audit_g13_canary.rs:529` | G13 content-ignorance canary synthetic rule (paired with NDJSON fixture strings at `:462` + `:591` which embed `"rule":"R999"` — Wave-2 Agent B updates those). |
| `Z001` | `("test", "synthetic.z001-rule-panic-isolation")` | `test:synthetic.z001-rule-panic-isolation` | `crates/engine/tests/rule_panic_isolation.rs:46` | Rule panic-isolation harness sentinel #1. |
| `Z002` | `("test", "synthetic.z002-rule-panic-isolation")` | `test:synthetic.z002-rule-panic-isolation` | `crates/engine/tests/rule_panic_isolation.rs:94` | Rule panic-isolation harness sentinel #2. |
| `Z003` | `("test", "synthetic.z003-rule-panic-isolation")` | `test:synthetic.z003-rule-panic-isolation` | `crates/engine/tests/rule_panic_isolation.rs:264` | Rule panic-isolation harness sentinel #3. |
| `E899` | `("test", "synthetic.e899-fixture")` | `test:synthetic.e899-fixture` | `crates/engine/src/engine.rs:7731` + `:7769` | Synthetic engine-test E-class rule (low-collision band). |
| `E898` | `("test", "synthetic.e898-fixture")` | `test:synthetic.e898-fixture` | `crates/engine/src/engine.rs:7833` + `:7863` | Synthetic engine-test E-class rule (low-collision band). |
| `RECORD` | `("test", "synthetic.record-fixture")` | `test:synthetic.record-fixture` | `crates/engine/src/engine.rs:6154` | Synthetic "record" test sentinel. |
| `PARSED_CACHE_TEST` | `("test", "synthetic.parsed-cache-test")` | `test:synthetic.parsed-cache-test` | `crates/engine/src/engine.rs:6623` | Synthetic parsed-cache-test sentinel. |
| `capco/noforn-if-no-fdr` | `("test", "synthetic.audit-note-sealing-capco-fixture")` | `test:synthetic.audit-note-sealing-capco-fixture` | `crates/engine/tests/audit_note_sealing_carve_out.rs:94` | Audit-note-sealing carve-out test fixture. Originally written as a `capco/...` slash-form to mimic the closure-rule shape; now reclassified to `test` scheme since it's never a real CAPCO rule ID. |
| `test/clone` | `("test", "synthetic.audit-note-sealing-clone-fixture")` | `test:synthetic.audit-note-sealing-clone-fixture` | `crates/engine/tests/audit_note_sealing_carve_out.rs:145` | Audit-note-sealing carve-out test fixture for the clone path. |

Render.rs example/test fixtures (`render.rs:1202` E008, `:1229`/`:1274`/`:1352` S004, `:1310` S999, `:1443` E002, `:1564` C001, `:1022` dynamic from audit line) are not synthetic test IDs — they construct real legacy rule IDs as documentation / smoke-test inputs. Wave-2 Agent C migrates them to the §1 production renames.

Engine.rs in-file unit tests at `:1996` (C001), `:7485` (smallvec: C001 + E006), `:7550`/`:7633` (E006), `:7558`/`:7641` (E022 — legacy retired rule ID retained as test input), `:7574`/`:7671`/`:8523` (C001), `:8442` (smallvec: E006), `:8580` (E006) construct real legacy rule IDs as test inputs. The `E006` and `C001` references rename to the §1 production tuples. The `E022` reference stays as a *legacy* string in the test (it's testing the engine's ability to handle ad-hoc rule IDs the registered ruleset doesn't contain — Wave-2 Agent B may either retire the test or migrate to `("test", "synthetic.e022-legacy-fixture")` if the test's intent is preserved).

`crates/rules/tests/applied_text_correction_seal.rs:39+55+72` (C001/E006), `crates/rules/tests/engine_promotion_seal.rs:159` (E001), `crates/rules/tests/message_args_closed_set.rs:71+72+91` (C001 + E006), `crates/wasm/tests/audit_v1_0_parity.rs:94+119+354` (parametrized + C001 + E006) — all rename to the §1 production tuples.

`crates/rules/src/lib.rs:1319` (doctest `RuleId::new("E001")`) — renames to `RuleId::new("capco", "banner.classification.portion-mark-in-banner")`… **TBD — Wave 1 Agent F1 may have replaced this doctest entirely with a `RuleId::new("capco", "...")` example; verify against the new lib.rs surface.** Marking TBD here doesn't block Wave 2 because the rename target is whatever Agent F1 wrote in the doctest.

`crates/rules/src/audit.rs:396` (doc-comment example `RuleId::new("E001")`) — same shape; rename to the §1 E001-equivalent production tuple (NOTE: E001 was retired in PR 3c.B Commit 6 with the "portion-mark-in-banner" semantic absorbed by `MarkingScheme::render_canonical`; the doc-comment example MAY need to switch to a still-active rule like E002 — Wave-2 Agent C verifies).

---

## TBD rows (Wave 2 must resolve before consuming this map mechanically)

- **§10 / `rules/src/lib.rs:1319` doctest**: rename target depends on Agent F1's final `RuleId` reshape (the doctest may be entirely rewritten). Verify against the new lib.rs surface; if the doctest still exists, use the §1 production tuple matching the legacy ID; if Agent F1 rewrote the doctest, no rename action needed.
- **§10 / `rules/src/audit.rs:396` doc-comment**: legacy `E001` is retired (PR 3c.B Commit 6); the doc-comment example should switch to a still-active rule. Wave-2 Agent C decision: use `E002` → `("capco", "portion.dissem.rel-to-missing-usa")` as the demonstrative example, OR delete the example. PM-confirm before commit.

All other rows are fully resolved. Citations re-verified against
`crates/capco/docs/CAPCO-2016.md` at this PR's authorship (2026-05-22) per
Constitution VIII.

---

## Walker/rule findings not in the architect's partial table

The architect's plan §1.5 table covers the major walkers but not the
following items, which I added to the map:

1. **`E070/frd-tfni-precedence`** (§6, §9 cross-ref): catalog row in
   `core_catalog.rs:289-300` that the architect's partial table did not
   enumerate explicitly. Bridge-routed via the `E###` prefix-recognition
   path at `engine.rs:2426-2430` (not via the special-case at `:2431`).
   §H.6 p104 citation.
2. **`W005/rel-to-not-in-joint-coverage`** (§6, §9 cross-ref): catalog
   row in `core_catalog.rs:112-128`. Closes the gap that previously
   misrouted all `W###` catalog rows to `E008` fallback (per the
   `engine.rs:2412-2435` doc-comment about extending `E` → `E | W`
   prefix recognition). §H.3 p56 citation.
3. **`capco/joint-requires-usa`** (§6, §9): catalog row in
   `core_catalog.rs:344-355`. Not in the architect's partial table — it's
   one of the two non-prefixed `capco/...` rows in core_catalog that
   currently fall through to the `engine.rs:2434` E008 fallback. Per
   OD-8.A this needs an explicit predicate-ID assignment so the bridge
   passthrough emits a meaningful 2-tuple.
4. **Banner-rollup walker E068 + E069 row identities** (§5): the per-row
   structs at `rules.rs:5333` (E068 banner-class mismatch) and `:5357`
   (E069 banner-FGI-marker mismatch) carry CAPCO citations the
   architect's table did not give predicate IDs for. §H.7 pp123-125
   (E068) and §H.7 p124 (E069) verified inline at the catalog rows.
5. **Closure rule `capco/rel-to-usa-nato-if-nato-classification`** (§8):
   exists in both `closure.rs:321-329` (as `CAPCO_CLOSURE_RULES`) AND
   `closure_table.rs:282` (as `CLOSURE_TABLE` row 7). The dual surface
   means Wave-2 Agent A renames in *both* files in lockstep.
6. **9 of 10 closure rows live only in `closure_table.rs::CLOSURE_TABLE`**, not in `CAPCO_CLOSURE_RULES` (which currently contains only the NATO REL TO row). Wave-2 Agent A's rename surface is `closure_table.rs::CLOSURE_TABLE` for all 10; the inline `row_names_match_fn_pointer_catalog` test at `closure_table.rs:460-476` is the positional pin that catches drift.
7. **Test fixture `capco/noforn-if-no-fdr`** (§10): the
   `audit_note_sealing_carve_out.rs:94` site uses a `capco/...` slash form
   that looks like a closure-rule key but isn't real. Reclassifying it to
   `("test", "synthetic.audit-note-sealing-capco-fixture")` matches the
   plan §1.7 + PM-decisions test-scheme reservation.
8. **`E022` legacy test input** (§10): retired in PR 3c.B Commit 6 but
   the engine.rs tests at `:7558` and `:7641` still construct
   `RuleId::new("E022")` as an ad-hoc input to exercise the engine's
   handling of unknown rule IDs. The test's intent (engine handles
   non-registered IDs gracefully) survives a rename to a `test`-scheme
   synthetic — Wave-2 Agent B decision point flagged in §10 row notes.

---

*End of map. Wave 2 agents consume this as the authoritative lookup.
Citations propagate; do not invent. Append, do not silently rewrite.*
