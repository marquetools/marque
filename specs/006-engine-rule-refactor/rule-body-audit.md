# Rule-Body Audit (PR 3c re-grounding)

**Date.** 2026-05-10. **Updated 2026-05-17 (PR #488):** the S006 rows
in this audit (line ~57 in §"Per-rule table" and the tally at line ~100
counting S006 as a (no clean fit) member) describe the pre-PR-#488
world where S005 and S006 were both registered. PR #488 retired S006
entirely; the historical Suggest/Info split was an engine workaround
(per-rule severity overwrite), NOT §-grounded — CAPCO-2016 §H.8 +
§D.2 Table 3 rule 21 apply uniformly to REL TO atom-semantics. S005
is now the sole survivor (dispatched at `Phase::PageFinalization`).
This audit document is a frozen snapshot of the 2026-05-10 state and
is not being rewritten for #488; future readers should subtract S006
from any post-2026-05-17 reasoning about (no clean fit) counts, while
treating S005's row as still-accurate aside from the now-collapsed
Suggest/Info branch description.

**Provenance.** Restarted after the user merged the bag-of-tokens architectural
restatement at `specs/006-engine-rule-refactor/architecture.md` (2026-05-09).
PR 3c.2's b-2.B audit had stalled mid-migration on a directive-enum design that
re-introduced span surgery as the fix model. The architecture restatement
re-grounded fixes in the fact-set delta vocabulary (`FactAdd` / `FactRemove` /
`Recanonicalize`) and named §3.0.b purpose-row classification — not §3.4 fix
shape — as the structural commitment the next plan starts from. This audit is
that table.

**Scope.** 47 registered rules: 28 hand-written impls in
`/home/knitli/marque/crates/capco/src/rules.rs` (27 + the
`BannerMatchesProjectedRule` walker at line 5064) and 19 declarative-pattern
impls in `/home/knitli/marque/crates/capco/src/rules_declarative.rs`. The
walker `BannerMatchesProjectedRule` registers under `Rule::id() = "E031"` but
emits per-row diagnostics tagged `E031` / `E035` / `E040`; the walker is
counted as one row in the table below with the per-row IDs broken out in the
"Notable findings" section.

**Method.** Each rule body read in source. Purpose-row classified against the
seven categories in `architecture.md` §"The §3.0.b purpose split". Citations
re-verified by Grep against `/home/knitli/marque/crates/capco/docs/CAPCO-2016.md`
per Constitution VIII. Fix-logic-match column compares the rule's existing
emission shape against the canonical shape vocabulary (`none` / `FactAdd` /
`FactRemove` / `Recanonicalize`) — `mismatch` flags any case where the rule
constructs replacement bytes by span surgery in a shape the lattice would
otherwise express directly.

---

## Table

| Rule | Citation (verified) | Purpose-row | ProjectedMarking axis | Fix-logic match | If form: canonicalization needed | If multi-purpose: natural split |
|---|---|---|---|---|---|---|
| **E001** portion-mark-in-banner | §H.8 (IC dissem) + §H.9 (non-IC dissem) per-template Authorized Banner Line Abbreviation columns | form | dissem, non_ic_dissem | mismatch — rule does in-place span replace of `NF`→`NOFORN` etc.; canonical for banner is the abbreviation, so the renderer would emit the same bytes by construction | banner: emit IC dissem in §G.1 Table-4 banner-abbreviation form; non-IC dissem in §H.9 banner form (`SBU NOFORN` not `SBU-NF`) | — |
| **E002** missing-usa-trigraph | §H.8 p150-151 ("USA must always appear first") | page-rewrite (USA-injection sub-shape) AND form (sort) | rel_to | mismatch — rule synthesizes `USA` into the list AND sorts in the same span splice; the USA-injection is a fact-set add (`FactAdd { CountryCode::USA, scope: rel_to }`) and the sort is renderer territory | rel_to canonicalization: USA first, remaining trigraphs alpha, then tetragraphs alpha, comma-space delimiter | E002 splits: `USA missing` → `FactAdd { USA, scope: rel_to }` (per §H.8 p151 mandate); USA-not-first → form (`Recanonicalize { scope: rel_to }`); the existing one-shot byte splice conflates both |
| **E003** misordered-blocks | §A.6 p15-16 (block ordinal sequence) | form (block reordering) | multiple (Classification, sci, sar, aea, fgi, dissem, rel_to, non_ic_dissem) | mismatch — `reorder_marking` rebuilds the whole-marking string from `attrs.token_spans`; the renderer canonicalizes block order by construction | render whole-marking blocks in §A.6 ordinal sequence: Classification // SCI // SAR // AEA // FGI // Dissem // Non-IC | — |
| **E004** separator-count | §D.1 p27 (no slashes hold place) + §A.6 Figure 2 p17 (`/` within / `//` between) | form (delimiter normalize) | multiple — runs across all axes | mismatch — both branches splice canonical separators into the input; the renderer emits canonical separators by construction | render `//` between categories, `/` within categories; collapse `////+` runs by construction (the renderer never emits a run because it never holds an empty category position) | — |
| **E005** declassify-misplaced | §E.1 p31 + §E.2 p32 (Declassify On lives in the CAB) + §D.1 p27 / §C.1 p26 (banner/portion category sets) | (no clean fit) — see Notable findings | declassify | none-needed (no fix proposed today) | — | true `Constraint::Custom` candidate — moving a declass token from banner-into-CAB is multi-span document rewriting, not a fact-set delta on a single scope; existing rule is honest about that ("None. Repairing a misplaced declass marking requires moving the token from the banner/portion into a CAB, which is multi-span document-level rewriting") |
| **E006** deprecated-dissem | §F (Legacy Control Markings, p35) | decoder | dissem | match — emits via `FixSource::MigrationTable`, which aligns with recognizer/canonicalize-time migration | — | — |
| **E007** x-shorthand-date | §E.6 (X-shorthand declass) | decoder | declassify | match — same migration-table channel as E006 | — | — |
| **E008** unrecognized-token | §G.1 p36 (Register of Authorized Markings — closed vocabulary) | decoder | (any token kind) | none-needed | — | — |
| **C001** corrections-map | `CONFIG:[corrections]` (config pointer; honestly NOT a CAPCO citation per the rule's own doc) | decoder | (any token kind) | match — recognizer/corrections-time substitution by user-supplied map | — | — |
| **E009** portion-abbreviation | §H.1 (US class) + §H.8 (IC dissem) + §H.9 (non-IC dissem) per-template Authorized Portion Mark columns | form (mirror of E001 in portion scope) | classification, dissem, non_ic_dissem | mismatch — same byte splice pattern as E001 | portion: emit US class abbrev (TS/S/C/U), dissem portion abbrev (NF/OC/...), non-IC portion abbrev where banner ≠ portion | — |
| **S001** prefer-banner-abbreviation | §A.6 p15 + §G.1 Table 4 p36 — both forms legal; this is style, not mandate (rule is honest about it) | form (style preference, not mandate) | dissem, non_ic_dissem | mismatch — splices abbreviation in place; absorbed by renderer choosing the abbreviation form | banner: prefer abbreviation when title and banner-abbrev differ; same canonicalization as E001 (this rule disappears entirely if the renderer chooses abbreviation as canonical) | — |
| **S002** banner-consistent-form | §A.6 p15 + §G.1 Table 4 p36 — convention, not mandate (rule is honest about it) | form (consistency check on a property the renderer enforces by construction) | dissem, non_ic_dissem | none-needed (no FixProposal — runs S001 to converge) | — | absorbed: a renderer that always emits abbreviation form makes "mixed forms" structurally unrepresentable; the diagnostic disappears |
| **S003** joint-usa-first | "IC convention (not CAPCO mandate)" per the rule's own citation; §H.3 p56 mandates pure alpha for JOINT lists, NO USA-first carve-out | form (style — convention layered over §H.3) | classification (Joint variant) | mismatch — splices a re-rendered classification token | renderer emits JOINT pure-alpha per §H.3 p56; S003 is configurable convention layered above ("disable via S003=off for strict §H.3"), so it does not retire into the renderer — it is a separate code path the user opts into | — |
| **S004** rel-to-trigraph-suggest | §H.8 p150-151 (Annex B trigraph code list); rule body acknowledges this is a statistical signal, not a vocabulary violation | decoder (suggest-don't-fix channel) | rel_to | match — `Severity::Suggest`, fix never auto-applied; pure recognizer-confidence territory | — | — |
| **E011** missing-non-us-prefix | §A.6 p15 ("non-US or Joint information ... must always start with `//`") + §H.3 p55 (JOINT classification "always starts with `//`") | form (delimiter — leading `//` for non-US classification slot) | classification (Fgi, Nato, Joint variants) | mismatch — splices `//` prefix; renderer emits leading `//` whenever classification has a non-US variant | renderer prefixes `//` to non-US classification block by construction (the slot is empty so the leading separator is structural) | — |
| **E013** delimiter-mismatch | §H.3 p56 (JOINT space-delimited) + §H.8 p150-151 (REL TO comma-space-delimited); rule cites §A.6 p15-16 as reinforcement | form (delimiter normalize, two list shapes) | classification (Joint), rel_to | mismatch — splices canonical delimiters into both lists; renderer chooses the delimiter per axis | renderer: JOINT list space-delimited; REL TO list comma-space-delimited | — |
| **W003** non-ic-dissem-in-classified-banner | §H.9 per-template "Precedence Rules for Banner Line Guidance" (LIMDIS p170, SBU p176, SBU-NF p178 etc.) | page-rewrite (cross-axis: classification ≥ C clears non-IC dissem subset) | classification, non_ic_dissem | none-needed (no FixProposal) | — | absorbed by a `PageRewrite` shape: when classification ≥ C, project non_ic_dissem ∩ {LIMDIS, SBU, SBU-NF} → ∅ on the banner. The diagnostic surfaces the divergence; the rewrite does the elision in the projection |
| **E052** rel-to-no-duplicates | §H.8 p150-151 (REL TO list grammar describes a set of country codes; structural rather than textual prohibition — rule is honest about the structural reasoning) | form (set-canonicalization in REL TO) | rel_to | mismatch — splices deduped list; absorbed by renderer treating REL TO as a set | renderer: render REL TO as a deduplicated USA-first alpha list (set semantics in canonical form) | — |
| **S005** rel-to-opaque-uncertain-reduction | §H.8 + ODNI ISMCAT Tetragraph Taxonomy | (no clean fit) — recognizer-uncertainty signal driven by tetragraph membership data the engine doesn't fully have | rel_to | none-needed (no FixProposal — engine cannot resolve from in-tree data) | — | true `Constraint::Custom` candidate (or admonition territory) — surfaces an uncertainty band: page-level intersection on REL TO produces an empty/reduced set when an opaque tetragraph is dropped; the user is told what would survive under the hypothetical membership |
| **S006** rel-to-opaque-uncertain-reduction-info | §H.8 + ODNI ISMCAT Tetragraph Taxonomy | (no clean fit) — sister of S005, audit-only signal when banner is consistent with atom-semantics | rel_to | none-needed (no FixProposal) | — | same as S005 — admonition / `Constraint::Custom`; the two rules exist as two registered impls only because the engine's severity-override layer cannot stably emit one rule at two severities |
| **E026** sar-portion-form | §H.5 p101 (Authorized Portion Mark: `SAR-[program identifier abbreviation]`) | form (abbrev choice in portion) | sar | mismatch — splices `SAR-` for `SPECIAL ACCESS REQUIRED-` when all programs are abbrev-shaped | renderer: emit `SAR-` in portion scope (long form is banner-only) | — |
| **E029** sar-compartment-order | §H.5 p100 (compartments ascending numeric-then-alpha; sub-compartments alphanumerically, single space) | form (sort within program) | sar | mismatch — rebuilds per-program block from sorted compartments; renderer does this by construction | renderer: per-program emit compartments numeric-first-then-alpha hyphen-joined; sub-compartments numeric-first-then-alpha space-joined per §H.5 p100 | — |
| **E030** sar-indicator-repeat | §H.5 p100 (SAR category indicator must not be repeated; programs separated by `/`) | form (single indicator, multi-program separator) | sar | mismatch — splices `/PROG` for `//SAR-PROG` (coalesces repeated indicators); renderer never emits the repeat by construction | renderer: emit single `SAR-` (or `SPECIAL ACCESS REQUIRED-`) indicator with `/`-separated programs | — |
| **E032** sci-system-order | §H.4 p61 (multiple SCI control systems ascending numeric-first-then-alpha, `/`-separated) | form (sort within SCI block) + form (sub-axis sort within compartments — see "multi-purpose" column) | sci | mismatch — rebuilds whole SCI block; renderer does this by construction | renderer: emit SCI block with systems sorted numeric-first-then-alpha, compartments sorted numeric-first-then-alpha hyphen-joined, sub-compartments sorted numeric-first-then-alpha space-joined per §H.4 p61 | E032 splits: SCI system ordering → form (the rule's own scope); the rule's fix ALSO reorders compartments and sub-compartments to absorb E033's invariant in one pass — that sub-shape is now in `DeclarativeNonCanonicalInputRule` row 5 (E060). Either both retire into the renderer together, or one consolidated row stays |
| **W034** sci-custom-control-info | §A.6 p16 + §H.4 p61 (unpublished SCI control systems are legitimate, ODNI/P&S unpublished registry) | admonition | sci | none-needed (no FixProposal) | — | — |
| **E039** nodis-exdis-clears-banner-rel-to | §H.9 p172 (EXDIS) + p174 (NODIS) — "REL TO is not authorized in the banner line if any portion contains EXDIS/NODIS information" | page-rewrite (NODIS-or-EXDIS-clears-rel_to, parallel to NOFORN-clears-REL-TO) | rel_to, non_ic_dissem | none-needed today (rule emits diagnostic, no fix) | — | natural home: a `PageRewrite` row `nodis-or-exdis-clears-banner-rel-to` declared on `CapcoScheme`; the diagnostic falls out of divergence detection between input rel_to and projected rel_to (which the rewrite empties) |
| **E031 / E035 / E040** banner-matches-projected (walker) | §H.5 p101 (SAR roll-up) + §H.4 per-system precedence templates (SCI roll-up; HCS p62, SI p74, TK p85) + §H.9 p172/p174 (NODIS/EXDIS roll-up) | lattice-property (banner = join of portions) | sar, sci, non_ic_dissem | none-needed for SAR-with-block-present (zero-width insertion fix; OK as `FactAdd { missing-program, scope: banner.sar }`); none for SAR-no-block (Error, no fix); SCI / non-IC rows emit no fix — see Notable findings on multi-row split | — | walker splits per row: each per-axis row is a property test on `Lattice::join` for that axis (banner observed = join over portions). Rules disappear for the lattice axes; each becomes a `#[test]` on the impl |
| **S003** | covered above | | | | | |
| **E041** nodis-supersedes-exdis-in-portion | §H.9 p172 (EXDIS) + p174 (NODIS) — NODIS supersedes EXDIS | conflicts (intra-axis supersession) | non_ic_dissem | mismatch (none-needed today, but the natural shape is `FactRemove { EXDIS, scope: portion.non_ic_dissem }`) | — | — |
| **E010** bare-hcs (declarative) | §H.4 (HCS family — bare HCS retained only for legacy machine-to-machine carry; new content uses HCS-O / HCS-P) | requires (companion-required: bare HCS implies HCS-O or HCS-P) | sci | mismatch — emits a span splice replacing `HCS` with `HCS-P`; the natural shape is `FactRemove { HCS, scope: portion.sci }` + `FactAdd { HCS-P, scope: portion.sci }` (or equivalently a re-canonicalize after closure on HCS-O / HCS-P presence) | — | — |
| **E012** dual-classification (declarative) | wrapper cites §B.1 (legacy); catalog cites §H.3 p55 (correct authoritative passage). **Citation defect** — see summary | conflicts (US classification ⊥ foreign classification in same marking) | classification | mismatch — splices the second classification token into FGI form; the natural shape is `FactRemove { foreign-classification }` + `FactAdd { FGI <list>, scope: portion.fgi }` | — | — |
| **E014** joint-rel-to (declarative) | §H.3 p56 (JOINT marking takes the form `//JOINT [class] [LIST]//REL TO [USA, LIST]`); wrapper citation `§H.3` is correct but imprecise | requires (JOINT participants ⊆ rel_to) | classification (Joint), rel_to | none-needed today (no fix) | — | natural shape: `FactAdd { country, scope: rel_to }` for each JOINT participant missing from rel_to; or absorbed by a closure operator that propagates JOINT.countries into rel_to as part of canonicalize/project |
| **E015** non-us-missing-dissem (declarative) | wrapper cites §B.3; catalog cites §H.7 + §B.3.d (correct). **Citation defect** — see summary | requires (non-US classification ⇒ ≥1 dissem control) | classification, dissem | none-needed today (no fix) | — | — |
| **E016** joint-restricted (declarative) | §H.3 p56 (JOINT TS/S/C/U only — no RESTRICTED carve-out — line 1263: "May not be used with RESTRICTED") | conflicts (JOINT ⊥ RESTRICTED level) | classification | none-needed today | — | — |
| **E036** joint-hcs (declarative) | §H.3 p57 line 1272 ("May not be used with the HCS markings or NOFORN markings") | conflicts (JOINT ⊥ HCS) | classification (Joint), sci | none-needed today (Error, no fix) | — | natural shape: `FactRemove { HCS, scope: portion.sci }` since JOINT is the more-binding marking (per the §H.3 p57 specific exclusion); see "What this implies" — dissem-axis subtractive-fix pattern from the RELIDO conflict cluster (E054-E057) suggests a parallel SCI-axis subtractive-fix is justified |
| **E021** aea-noforn (declarative) | §H.6 (RD/FRD/TFNI requires NOFORN unless §123/§144 sharing agreement) | requires (AEA implies NOFORN) | aea, dissem | none-needed today (Error, no fix) | — | natural shape: `FactAdd { NOFORN, scope: portion.dissem }` |
| **E024** rd-precedence (declarative) | §H.6 p104 (RD takes precedence over FRD/TFNI in both banner and portion; RD evicts FRD/TFNI from banner when all three present) | conflicts (RD ⊥ {FRD, TFNI} co-presence — RD wins) | aea | none-needed today (multi-Error, no fix) | — | natural shape: `FactRemove { FRD, scope }` and `FactRemove { TFNI, scope }` per §H.6 p104; today the multi-emit pattern flags both losers without proposing the removal |
| **W002** us-fgi-comingling (declarative) | §H.7 (US + FGI commingling in portion is permitted under ICD-206 but cautioned) | admonition (cautionary advice when ICD-206 commingling is in play) | classification, fgi | none-needed (Warn, no fix) | — | — |
| **E037** nodis-conflicts-exdis (declarative) | §H.9 p172 (EXDIS) + p174 (NODIS) — symmetric mutual exclusion | conflicts (NODIS ⊥ EXDIS) | non_ic_dissem | none-needed today (Error, no fix) | — | natural shape: `FactRemove`; one or the other goes (NODIS wins per §H.9 — see E041 portion-level supersession). The pair conflicts/supersedes is what the dissem-axis subtractive-fix family exemplifies (PR 3b.C RELIDO cluster) |
| **E038** dos-dissem-noforn (declarative) | §H.9 p172 (EXDIS) + p174 (NODIS) — both "May be used only with NOFORN information" | requires (NODIS-or-EXDIS implies NOFORN) | non_ic_dissem, dissem | none-needed today (Error, no fix) | — | natural shape: `FactAdd { NOFORN, scope: portion.dissem }` |
| **E053** noforn-rel-to-conflict (declarative) | §H.8 p145 NOFORN entry line 3585 ("Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY") | conflicts (NOFORN ⊥ REL TO) | dissem, rel_to | none-needed today (Error, no fix) | — | natural home: subsumed by the existing `noforn-clears-rel-to` PageRewrite (architecture.md §"Project" closure) — once the page rewrite runs, the projected REL TO is empty when NOFORN is present, and the input-vs-projected divergence is the diagnostic. The rule retires |
| **E054** relido-noforn-conflict (declarative) | §H.8 p154 RELIDO entry ("Cannot be used with NOFORN or DISPLAY ONLY") | conflicts (RELIDO ⊥ NOFORN) | dissem | match — emits `FactRemove { RELIDO }` shape (subtractive fix at confidence 0.95); today expressed as a span splice with replacement `""` plus separator-eating, which is the right semantics in the wrong vocabulary | — | — |
| **E055** relido-display-only-conflict (declarative) | §H.8 p154 RELIDO entry | conflicts (RELIDO ⊥ DISPLAY ONLY) | dissem | match — same `FactRemove { RELIDO }` pattern | — | — |
| **E056** orcon-relido-conflict (declarative) | §H.8 p136 ORCON entry line 3363 ("May not be used with RELIDO") | conflicts (ORCON ⊥ RELIDO) | dissem | match — same `FactRemove { RELIDO }` pattern | — | — |
| **E057** orcon-usgov-relido-conflict (declarative) | §H.8 p140 ORCON-USGOV entry line 3444 ("May not be used with RELIDO") | conflicts (ORCON-USGOV ⊥ RELIDO) | dissem | match — same `FactRemove { RELIDO }` pattern | — | — |
| **E058** class-floor-catalog (walker) | §H.6 p104 (CNWDI floor TS/S RD only) + §H.6 p116/p118 (DOD/DOE UCNI U-only ceiling) + §H.5 (SAR floor TS/S/C) + 23 additional `class-floor/<marking>` family rows per §H.4 / §H.6 / §H.8 / §H.9 | requires (per-token classification floor — `Constraint::Requires { token, class ≥ X }`) | sci, aea, sar, dissem, classification (Nato variant) | none-needed today (no fix; per-row Error or Warn passthrough); natural shape is `FactAdd { class-bump, scope }` for the floor cases and `FactRemove { token }` (or admonition) for the ceiling cases | — | walker is already in the right purpose-row (`requires`); the table of 27 rows IS the constraint catalog. Splits by row only when fix shape per row diverges (floor → FactAdd; ceiling → either FactRemove on the marking or escalate class on the portion — policy choice) |
| **E059** sci-per-system-catalog (walker) | §H.4 p64 (HCS-O companions) + p66 (HCS-P NOFORN) + p68 (HCS-P sub companions) + p80 (SI-G companions) + p87/p91/p95 (TK-{BLFH,IDIT,KAND} NOFORN) | requires (companion-required) AND conflicts (forbid-companion: HCS-P sub vs ORCON-USGOV; SI-G vs ORCON-USGOV) | sci, dissem | none-needed today (no fix; some rows have token-replacement fixes for the forbid-companion sub-cases per the in-tree comment) | — | walker is in the right purpose-row but mixes Requires and Conflicts across rows; natural split is two catalogs: `requires/<marking>-companion` (FactAdd shape) and `conflicts/<marking>-forbid` (FactRemove shape). Done per-row, not at walker level |
| **E060** non-canonical-input (walker) | §H.8 p150-151 (REL TO USA-first alpha) + §H.3 p56 (JOINT alpha) + §H.6 p108 (SIGMA numeric sort + valid set) + §H.5 p99 (SAR ascending alpha) + §H.4 p61 (SCI compartment + sub-compartment numeric-then-alpha) | form (sort canonicalization across 5 list axes) | rel_to, classification (Joint), aea, sar, sci | mismatch — every row splices a re-rendered list back into the input span; the renderer does this by construction. Walker doc explicitly self-identifies as "STAGE-1 INTERIM ... retires cleanly when `MarkingScheme::render_canonical` lands" | renderer: per-axis sort canonicalization for all 5 lists. **All 5 rows retire together when the renderer lands** | — |

---

## Summary

**Counts per purpose-row** (47 rules total; banner walker counted once but emits 3 IDs):

- `lattice-property` — 1 (the walker `E031` covering 3 row IDs E031/E035/E040)
- `page-rewrite` — 3 (E039 NODIS/EXDIS-clears-rel-to; E053 NOFORN-clears-rel-to absorbed; W003 classified-clears-non-IC-dissem-subset)
- `conflicts` — 9 (E016 JOINT/RESTRICTED, E024 RD/FRD-TFNI, E036 JOINT/HCS, E037 NODIS/EXDIS, E041 portion-level, E054, E055, E056, E057)
- `requires` — 6 declarative + 1 walker (E010, E012, E014, E015, E021, E038, plus E058 class-floor walker (27 rows) and E059 SCI per-system walker (5 rows mixing requires + conflicts))
- `form` — 16 hand-written + 1 walker (E001, E002, E003, E004, E009, S001, S002, S003, E011, E013, E026, E029, E030, E032, E041, E052, plus E060 walker (5 rows))
- `admonition` — 2 (W002 commingling caution, W034 SCI custom-control)
- `decoder` — 4 (E006, E007, E008, C001, S004)
- `(no clean fit)` — 3 (E005 declassify-misplaced, S005, S006 — see "Notable findings")

(Counts overlap categories where a walker spans multiple purpose-rows; E058 walker has 27 row-level rules, E060 walker has 5, E059 walker has 5, banner walker has 3 — the table treats each walker as one row but the per-row split is the real refactor surface.)

**Fix-logic mismatches:** ~22 of the 47 (≈47%). PR 3c.2's b-2.B sample of 11 found 36% mismatch; the wider audit confirms a higher rate, driven by the form bucket where every rule splices canonical bytes into the input span instead of expressing the divergence as `Recanonicalize`. The form bucket is mostly mismatch by design — the architecture restatement names the renderer as the single source of canonical form, so any rule that constructs its own replacement bytes for a form concern is inherently mis-housed.

**Multi-purpose splits required:** 2 explicit (E002 USA-injection vs sort; E032 SCI system-order vs sub-compartment-order). Several walker rows are also implicit splits — E058's 27 rows mix floor (FactAdd) with ceiling (FactRemove or admonition), and E059's 5 rows mix requires with conflicts.

**Citation defects found:**

- **E012** (declarative wrapper, line 400): wrapper carries `"CAPCO-2016 §B.1"`; the catalog row carries `"CAPCO-2016 §H.3 p55"` which is the correct authoritative passage (US/foreign mutual exclusion is stated on the JOINT template at §H.3 p55 — verified at CAPCO-2016.md line 1223 "The US, non-US, and JOINT classification markings are mutually exclusive"). The wrapper's `§B.1` is rule-shape preamble, not the authoritative mandate. The wrapper has an inline comment acknowledging the divergence and pending update; the audit records it as a defect that hasn't been fixed.
- **E015** (declarative wrapper, line 498): wrapper carries `"CAPCO-2016 §B.3"`; the catalog row carries `"CAPCO-2016 §H.7 + §B.3.d"` which is the correct authoritative pair (FGI commingling + FD&R procedures). Same shape as E012 — wrapper acknowledges the gap inline.
- **S003** (rules.rs line 2271-2277): the rule's citation string is honest and explicit that this is "IC convention (not CAPCO mandate)" and lists "S003 = \"off\" for strict §H.3 conformance." This is NOT a defect; it is correct citation hygiene for a style rule that doesn't claim §H.3 authority. The audit notes the rule body for completeness because S003 is the only rule in the catalog where the citation string is itself a justification, not a §-pointer.
- **E001 / E009 / E011 / E013** all carry slightly imprecise citations (`§H.8` without page; `§A.6 + §H.3` without page where both have specific pages). Not defects — they are tighter than the §H.X-only form would be, but tighter still is available (e.g., E013 cites `§H.3 p56` and `§H.8 p150-151` correctly in the message body but the `Diagnostic.citation` field carries the imprecise pair). Picking up the precision in the move to the renderer is a chance to tighten without breaking byte-identity downstream.

No fabricated citations found. Every cited §X.Y or §X.Y pNN reference traces to a real passage in `CAPCO-2016.md`. The S003 admission ("convention, not mandate") is the only place the rule body is openly skeptical of its own citation; that skepticism is correctly encoded.

---

## Notable findings

**Rules that don't fit any purpose-row cleanly (genuine `Constraint::Custom` candidates):**

- **E005** declassify-misplaced — the invariant ("declass tokens live in the CAB, not in banner or portion") IS source-fidelity (§E.1 + §E.2 + §D.1 + §C.1), but the fix shape is multi-span document-level rewriting (move the token from `banner.declassify_on` slot to `cab.declassify_on` slot), which the fact-set delta vocabulary doesn't directly express. A genuine `Constraint::Custom` row, OR an admonition.
- **S005 / S006** rel-to-opaque-uncertain-reduction — not divergence between input and projection (both views agree); a recognizer-uncertainty band surfaced because tetragraph membership data is incomplete in-tree. Closer to admonition than to any fact-set delta.

**Rules whose existing fix logic was discovered to be incorrect against the source — none.** Every fix surveyed encodes the §-cited mandate correctly; the citation-defects in E012/E015 are wrapper-vs-catalog drift on which §-pointer is named, not predicate errors.

**Rules whose end-state is "delete entirely" (absorbed by renderer / lattice / closure with no residual divergence detection needed):**

- **E001** (form rule absorbed by renderer's banner-abbreviation choice)
- **E003** (block-order absorbed by renderer's §A.6 ordinal)
- **E004** (separator-canonicalization absorbed)
- **E009** (mirror of E001 in portion scope)
- **S001** (becomes the renderer's choice — abbrev as canonical)
- **S002** (consistency falls out by construction)
- **E011** (`//` prefix is structural)
- **E013** (delimiter normalize per axis)
- **E026** (`SAR-` is portion-canonical by construction)
- **E029** (sort within program)
- **E030** (single indicator with `/`-separated programs)
- **E032** (sort within SCI block)
- **E052** (REL TO is a set; renderer dedupes by construction)
- **E060 walker (5 rows)** — explicitly self-identifies in its own doc as "STAGE-1 INTERIM ... retires cleanly when `MarkingScheme::render_canonical` lands"

**Banner walker row decomposition:** `BannerMatchesProjectedRule` registers under `Rule::id() = "E031"` but emits `Diagnostic.rule = E031` (SAR roll-up), `E035` (SCI roll-up), `E040` (non-IC dissem roll-up). All three rows are lattice-property: banner-axis = `Lattice::join` over portions on that axis. The natural retirement is per row to a `#[test]` on each axis's `Lattice::join` impl plus residual divergence detection only when the lattice law holds but the canonical render diverges (e.g., compartments-optional in SAR per §H.5 p101 + p99 vs. compartments-required in SCI). The "compartments optional" carve-out for SAR is captured today by `sar_missing_programs` comparing programs only; the renderer would handle the compartment depth choice on the projection.

**Walker row decomposition (E058 / E059 / E060):** The three walker rules are each a catalog of small declarative rows held inside a single registered `Rule` impl for ergonomic dispatch. The end-state is row-level retirement, not rule-level — most E060 rows retire into the renderer; most E058 rows become declarative `Constraint::Requires` entries; most E059 rows split between `Requires` and `Conflicts`. The walker's own `Rule::id()` is bookkeeping; the migration math is at the row level.

**Subtractive-fix pattern (RELIDO cluster, E054-E057):** Already in the right purpose-row, already emitting the right shape (subtractive removal of one token when its conflict-binding partner is the more authoritative). Today expressed as span surgery with `replacement: ""` plus a separator-eating helper (`compute_relido_removal_span`). Direct retarget to `FactRemove { RELIDO, scope: portion.dissem }` is a pure vocabulary swap; no semantic change, no test churn beyond audit-stream byte-identity if/when the audit format reflects the directive vocabulary.

**Confidence calibration drift signal:** E054-E057 use `0.95` (auto-apply at default threshold); E001 uses `1.0`; E002 uses `0.97`; E003 uses `0.6` (suggestion-only); E029 / E030 use `0.85-0.9`; E032 uses `0.85`; E058 walker has per-row severity but rule-level confidence not surfaced; the SAR fixes are calibrated 0.35 (E026 abbreviation suggestion). The calibration spread is not a citation defect, but it's an observation: when fix shapes move to the canonical vocabulary, confidence becomes a property of the shape (fact-add vs fact-remove vs recanonicalize) more than a per-rule judgment, which suggests calibration consolidation as a side effect of the migration.

---

## What this implies for the next plan

The form bucket is the largest purpose-row by count and the largest mismatch source — 16 of 47 rules sit there, every one of them encoding a piece of the renderer's eventual canonicalization knowledge in the wrong layer (rule body emitting span splices). The architecture restatement names `render_canonical` as the single source of canonical form; the bulk of the migration is moving knowledge from `Rule::check` bodies into per-axis canonicalization functions inside the scheme adapter, then deleting the rules. The renderer-canonicalization specs that need to land to absorb each row are listed in the table's column 4; a non-trivial number of rows (E001 / E009 / E026 / S001) all collapse to the same banner-vs-portion form choice, which is one decision in the renderer.

The conflicts and requires buckets are smaller (9 + 6) but more structural. Most rows are already declarative wrappers around `Constraint::Conflicts` / `Constraint::Requires` predicates in `CapcoScheme`. The work is shifting their emission shape from "diagnostic with no fix or with a span-splice fix" to `FactAdd` / `FactRemove` directives the engine promotes into `AppliedFix`. The RELIDO cluster (E054-E057) is the existing template — `FactRemove { RELIDO }` semantics already expressed in subtractive-byte form. Other declarative rules that today are no-fix (E021, E024, E036, E037, E038, E041) gain real fix proposals once the directive vocabulary lands; the mandate is already in the catalog row, only the emission shape is missing.

The lattice-property bucket (E031 / E035 / E040 walker) and the page-rewrite bucket (E039, E053-already-folded, W003) are the smallest counts and the cleanest retirements — each row becomes either a property test on a `Lattice::join` impl or a declared row on `CapcoScheme::page_rewrites()`. The decoder bucket (E006, E007, E008, C001, S004) is already correctly housed at the recognizer layer; no migration needed beyond the audit-record annotations the existing recognizer trail already emits. The admonition bucket (W002, W034) and the no-clean-fit residue (E005, S005, S006) are the genuine `Constraint::Custom` candidates the architecture restatement explicitly names as a small, principled exception — three to five rules total, each with a real reason it doesn't fit a structural row.
