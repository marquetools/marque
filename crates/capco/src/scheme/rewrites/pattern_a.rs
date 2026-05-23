// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pattern-A NOFORN-supremacy rows: `capco/nodis-implies-noforn`,
//! `capco/exdis-implies-noforn`, `capco/sbu-nf-implies-noforn`,
//! `capco/les-nf-implies-noforn`. Lifted from the monolithic
//! `rewrites.rs` per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).
//!
//! Row declaration order preserved verbatim from the pre-split
//! catalog — DAG-sibling rows whose declaration order seeds Kahn's
//! algorithm with the right cohort ordering.

use marque_scheme::{
    CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent, Scope,
    SectionLetter, capco,
};

use super::super::predicates::{les_nf_classified_trigger, sbu_nf_classified_trigger};
use super::super::*;

/// The four Pattern-A NOFORN-supremacy rows in declaration order.
///
/// Every row writes `CAT_DISSEM` (FactAdd NOFORN). The Kahn scheduler
/// orders all four BEFORE `capco/noforn-clears-rel-to` (DISSEM-reader)
/// so the REL TO axis is correctly cleared in the same projection
/// pass when any of the triggers is present.
///
/// **Reads diverge by classification-gating shape**:
/// - **NODIS** and **EXDIS** rows read `CAT_NON_IC_DISSEM` only.
///   They use `Contains(CAT_NON_IC_DISSEM, TOK_*)` triggers and are
///   classification-agnostic by authority — §H.9 p174 / p172 say both
///   markings "May be used with TOP SECRET, SECRET, CONFIDENTIAL, or
///   UNCLASSIFIED" + "Requires NOFORN.", so NF is mandatory at every
///   classification level.
/// - **SBU-NF** and **LES-NF** rows read `CAT_CLASSIFICATION` only
///   (#554). They use `Custom(sbu_nf_classified_trigger)` /
///   `Custom(les_nf_classified_trigger)` predicates that gate on
///   `is_classified ∧ contains compound-NF token`. The non_ic_dissem
///   scan stays in the predicate body per Pattern-C's
///   predicate-scan-vs-dataflow convention (see
///   `pattern_c.rs::PATTERN_C_SBU_NF_READS`). The gate exists because
///   the compound tokens (`SBU-NF`, `LES-NF`) themselves encode
///   NOFORN per the §H.9 p178 / p185 Example Banner Lines
///   (`UNCLASSIFIED//SBU NOFORN`, `UNCLASSIFIED//LES NOFORN`); a
///   separate NF on the dissem axis would be redundant on
///   unclassified pages and produces a divergence from the lattice
///   helper's `needs_nf = false` semantic.
pub(super) fn pattern_a_rows() -> Vec<PageRewrite<CapcoScheme>> {
    // PR 3c.B Sub-PR 8.F — Pattern A NOFORN-supremacy: NODIS and EXDIS.
    //
    // Both rewrites read `CAT_NON_IC_DISSEM` (to detect the
    // NODIS / EXDIS token) and write `CAT_DISSEM` (to add NOFORN).
    // The `reads = [CAT_NON_IC_DISSEM]` / `writes = [CAT_DISSEM]`
    // dataflow annotations make the Kahn scheduler order these two
    // rewrites BEFORE `capco/noforn-clears-rel-to`, which reads
    // `CAT_DISSEM`. This guarantees that once a NODIS or EXDIS
    // portion is seen on the page, NOFORN is in the projected dissem
    // state before the clearer runs — so the REL TO axis is correctly
    // cleared in the same projection pass.
    //
    // No upstream producer constraint on these two rows: they read
    // `CAT_NON_IC_DISSEM` for the NODIS / EXDIS token scan, and
    // Pattern-C + #552 supersession rows now also write
    // `CAT_NON_IC_DISSEM` (LIMDIS / SBU / SBU-NF strip + the
    // bare-SBU / bare-LES supersession), so the Kahn scheduler
    // orders these NODIS / EXDIS readers AFTER those writers. That
    // ordering is correct for the NODIS / EXDIS case — no portion
    // contains NODIS or EXDIS as a non-token co-presence side
    // effect; the upstream Pattern-C / supersession rows only
    // touch SBU / SBU-NF / LIMDIS, never NODIS / EXDIS, so the
    // scan still sees the trigger token after upstream rows run.
    //
    // FUTURE (Pattern A SCI follow-on): 5 more `*-implies-noforn`
    // rewrites for SCI systems (HCS-O / HCS-P-sub / TK-IDIT /
    // TK-BLFH / TK-KAND per §H.4 p64 / p68 / p87 / p91 / p95)
    // will read `CAT_SCI` and write `CAT_DISSEM`. They are a
    // structural peer of these two entries but land in a follow-on
    // sub-PR (8.F.2 or Stage-4 SCI NOFORN-implication PR) after
    // `capco_category_contains` is extended for `CAT_SCI` + token
    // dispatch.
    //
    // Runtime execution gap (design spec §5): these rewrites are
    // scheduler-validated (Engine::new validates intent payloads +
    // topological ordering) but execution-deferred (`Engine::lint` /
    // `Engine::fix` drives banner-validation through
    // `marque_ism::PageContext` directly, not through
    // `scheme.project`). Callers that invoke
    // `scheme.project(Scope::Page, …)` directly see the full
    // declarative effect today. Engine-level effect lands when
    // Phase D/E wires the banner-validation path through
    // `scheme.project`.
    const NODIS_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const NODIS_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];
    const EXDIS_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const EXDIS_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // PR 3c.B Sub-PR 8.F.2 — SBU-NF and LES-NF Pattern A axes (axis
    // annotations revised in #554; classification gate added).
    //
    // Reads `CAT_CLASSIFICATION` only — predicate-scan-vs-dataflow
    // convention from PR 4b-C Pattern-C (see `pattern_c.rs`
    // `PATTERN_C_FOUO_READS` / `PATTERN_C_LIMDIS_READS`). The
    // non_ic_dissem scan (SBU-NF / LES-NF presence) lives in the
    // Custom predicate body (`sbu_nf_classified_trigger` /
    // `les_nf_classified_trigger`) rather than as a declared read,
    // so the scheduler doesn't manufacture cross-axis edges from
    // CAT_NON_IC_DISSEM writers (Pattern-C SBU-NF strip + #552
    // supersession rows). The data dependency we DO need to express
    // is the classification gate: this row reads classification to
    // decide whether to fire at all. Writes `CAT_DISSEM` (FactAdd
    // NOFORN); both rows remain in the DISSEM-writer cohort ordered
    // BEFORE `capco/noforn-clears-rel-to` (DISSEM-reader) by Kahn.
    const SBU_NF_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const SBU_NF_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];
    const LES_NF_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const LES_NF_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    vec![
        // PR 3c.B Sub-PR 8.F — `capco/nodis-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p174 (NO DISTRIBUTION, Relationship(s) to
        // Other Markings):
        //   "- May be used with TOP SECRET, SECRET, CONFIDENTIAL,
        //      or UNCLASSIFIED.
        //    - NODIS and EXDIS markings cannot be used together.
        //    - Requires NOFORN."
        //
        // The "Requires NOFORN." line is the operative authority for
        // this rewrite. The NODIS entry's "Precedence Rules for Banner
        // Line Guidance" (p174) further states: "REL TO is not
        // authorized in the banner line if any portion contains NODIS
        // information. In this case, NOFORN would convey in the banner
        // line." — confirming NOFORN as the foreign-release vehicle
        // when NODIS is present.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_NODIS)` — fires
        // when any portion on the page carries NODIS in its
        // `non_ic_dissem` axis. Resolved by the
        // `capco_category_contains` extension in PR 3c.B Sub-PR 8.F.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Monotone-
        // additive: FactAdd with an already-present token is a
        // per-intent no-op (IntentInapplicable, silent) per the
        // idempotence policy in `apply_fact_add`
        // (`crates/capco/src/scheme/actions/intent.rs`).
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. The Kahn scheduler (engine/src/scheduler.rs)
        // places this rewrite BEFORE `capco/noforn-clears-rel-to`
        // (which reads CAT_DISSEM) so the REL TO axis is correctly
        // cleared in the same projection pass when NODIS is present.
        // Declaration order here also respects this invariant: the two
        // `*-implies-noforn` entries appear before `noforn-clears-rel-to`
        // in the vec so `project`'s sequential scan sees them first.
        //
        // Classification-agnostic: §H.9 p174 says "May be used with
        // TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED" — the
        // trigger predicate is classification-agnostic and fires at
        // any classification level, including UNCLASSIFIED.
        //
        // FUTURE (SCI Pattern A follow-on): 5 more `*-implies-noforn`
        // rewrites reading `CAT_SCI` / writing `CAT_DISSEM` will land
        // alongside this entry in a follow-on sub-PR after
        // `capco_category_contains` is extended for `CAT_SCI` dispatch
        // (§H.4 p64 / p68 / p87 / p91 / p95).
        //
        // Runtime execution gap: this rewrite is scheduler-validated
        // (Engine::new validates the intent payload + topological
        // ordering) but execution-deferred (`Engine::lint` / `Engine::fix`
        // drives banner-validation through PageContext directly). Effect
        // is visible through `scheme.project(Scope::Page, …)`. Engine-
        // level effect lands when Phase D/E wires banner-validation
        // through `scheme.project`.
        PageRewrite::declarative(
            "capco/nodis-implies-noforn",
            capco(SectionLetter::H, 9, 174),
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_NODIS,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            NODIS_IMPLIES_NF_READS,
            NODIS_IMPLIES_NF_WRITES,
        ),
        // PR 3c.B Sub-PR 8.F — `capco/exdis-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p172 (EXCLUSIVE DISTRIBUTION, Relationship(s)
        // to Other Markings):
        //   "- May be used with TOP SECRET, SECRET, CONFIDENTIAL,
        //      or UNCLASSIFIED.
        //    - EXDIS and NODIS markings cannot be used together.
        //    - Requires NOFORN."
        //
        // The "Requires NOFORN." line is the operative authority for
        // this rewrite. The EXDIS entry's "Precedence Rules for Banner
        // Line Guidance" (p172) further states: "REL TO is not
        // authorized in the banner line if any portion contains EXDIS
        // information. In this case, NOFORN would convey in the banner
        // line." — confirming NOFORN as the foreign-release vehicle
        // when EXDIS is present.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_EXDIS)` — fires
        // when any portion on the page carries EXDIS in its
        // `non_ic_dissem` axis.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Same
        // monotone-additive + idempotence policy as the NODIS entry.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. Scheduler ordering: same sibling position
        // as `capco/nodis-implies-noforn` — both are DISSEM-writers
        // ordered before `noforn-clears-rel-to` (DISSEM-reader).
        // The two `*-implies-noforn` entries are DAG siblings (no
        // ordering dependency between them). Declaration order here
        // also respects this invariant: both appear before
        // `noforn-clears-rel-to` in the vec.
        //
        // Classification-agnostic: §H.9 p172 says "May be used with
        // TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED" — same
        // as the NODIS entry.
        //
        // Note: §H.9 p172 specifies "EXDIS and NODIS markings cannot
        // be used together." — the NODIS ⊥ EXDIS conflict is already
        // enforced by E037 (stays registered per design spec §5 Option
        // R2). Under malformed input where both appear simultaneously,
        // both rewrites fire; the second FactAdd hits the idempotence
        // no-op path (NOFORN already present), producing exactly one
        // NOFORN with no panic.
        //
        // FUTURE: see the NODIS entry doc-comment for the SCI Pattern A
        // follow-on note.
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        PageRewrite::declarative(
            "capco/exdis-implies-noforn",
            capco(SectionLetter::H, 9, 172),
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_EXDIS,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            EXDIS_IMPLIES_NF_READS,
            EXDIS_IMPLIES_NF_WRITES,
        ),
        // PR 3c.B Sub-PR 8.F.2 — `capco/sbu-nf-implies-noforn`
        // (#554: classification gate added; was classification-agnostic).
        //
        // CAPCO-2016 §H.9 p178 (SBU-NF) does NOT contain a "Requires NOFORN."
        // sentence — unlike NODIS (§H.9 p174) and EXDIS (§H.9 p172). The NF
        // implication on **classified** portions is the Commingling Rule's
        // `(C//NF)` canonical example. Two structural anchors:
        //   (a) Commingling Rule at §H.9 p178: "If the portion is
        //       classified, the classification level of the portion
        //       adequately protects the SBU information, so SBU is not
        //       reflected in the portion mark; however a NOFORN marking
        //       must be added to the portion mark, e.g., (C//NF)." Also:
        //       "If there is other NOFORN information in the commingled
        //       portion, the 'SBU' marking is used and a NOFORN marking
        //       is added, e.g., (U//NF//SBU)."
        //   (b) §D.2 Table 3 row 3-5 lists NOFORN as the FD&R banner
        //       consequence for SBU-NF on classified pages.
        //
        // # Classification gate (#554)
        //
        // On unclassified pages the compound `SBU-NF` token itself
        // encodes NOFORN per the §H.9 p178 Example Banner Line
        // `UNCLASSIFIED//SBU NOFORN` and Notional Example Page 1; a
        // separate NOFORN segment on the dissem axis is redundant and
        // produces a divergence from the lattice helper's
        // `needs_nf = false` semantic on unclassified `{SbuNf}`. The
        // Custom trigger gates on `is_classified` to fire only on
        // classified pages; the unclassified compound carries its own
        // NF identity through the non_ic_dissem axis. Pre-#554, this
        // row used a classification-agnostic `Contains` trigger and
        // overfired on unclassified compound tokens — the divergence
        // documented on the #552 `parity_unclassified_sbu_co_present_*`
        // fixture, closed here.
        //
        // Trigger: `Custom(sbu_nf_classified_trigger)` — fires when
        // `is_classified(m) ∧ contains SBU-NF in non_ic_dissem`. Co-
        // fires with `capco/sbu-nf-evicted-by-classified` (Pattern-C,
        // §H.9 p178) on the same input; they touch different axes
        // (CAT_DISSEM FactAdd vs. CAT_NON_IC_DISSEM FactRemove), so
        // the net effect is `(C//SBU-NF) → (C//NF)`.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Monotone-
        // additive: FactAdd of an already-present token is a per-
        // intent no-op (IntentInapplicable, silent) per
        // `apply_fact_add`'s CAT_DISSEM arm (grep `apply_fact_add`
        // to find the current location).
        //
        // Axis annotations: reads `[CAT_CLASSIFICATION]`, writes
        // `[CAT_DISSEM]`. The non_ic_dissem scan lives in the Custom
        // predicate body per Pattern-C convention. Kahn places this
        // row BEFORE `capco/noforn-clears-rel-to` (DISSEM-reader)
        // so the REL TO axis is correctly cleared in the same
        // projection pass.
        //
        // FUTURE (SCI Pattern A follow-on): see the NODIS entry
        // doc-comment for the SCI follow-on (§H.4 p64/p68/p87/p91/p95).
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        // Scheduler-validated but execution-deferred; visible through
        // `scheme.project(Scope::Page, …)`.
        PageRewrite::custom(
            "capco/sbu-nf-implies-noforn",
            capco(SectionLetter::H, 9, 178),
            CategoryPredicate::Custom(sbu_nf_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            SBU_NF_IMPLIES_NF_READS,
            SBU_NF_IMPLIES_NF_WRITES,
        ),
        // PR 3c.B Sub-PR 8.F.2 — `capco/les-nf-implies-noforn`
        // (#554: classification gate added; was classification-agnostic).
        //
        // CAPCO-2016 §H.9 p185 (LES-NF) does NOT contain a "Requires NOFORN."
        // sentence — unlike NODIS (§H.9 p174) and EXDIS (§H.9 p172). The NF
        // implication on **classified** portions is derived from the
        // Precedence Rules for Banner Line Guidance verbatim quote:
        // "When a classified document contains portions of U//LES- NF,
        // the 'LES' marking is used in the banner line and the NOFORN
        // marking is applied as a Dissemination Control Marking. For
        // example: SECRET//NOFORN//LES." (Source has whitespace OCR
        // artifact `LES- NF` rendered with a space; canonical token
        // is `LES-NF`.) §D.2 Table 3 rows 6-8 list NOFORN as the
        // FD&R banner consequence for LES-NF on classified pages.
        //
        // # Classification gate (#554)
        //
        // On unclassified pages the compound `LES-NF` token itself
        // encodes NOFORN per the §H.9 p185 Example Banner Line
        // `UNCLASSIFIED//LES NOFORN` and Notional Example Page 1; a
        // separate NOFORN segment on the dissem axis is redundant
        // and produces a divergence from the lattice helper's
        // `needs_nf = false` semantic on unclassified `{LesNf}`. The
        // Custom trigger gates on `is_classified` to fire only on
        // classified pages. Pre-#554, this row used a classification-
        // agnostic `Contains` trigger and overfired on unclassified
        // compound tokens — the divergence documented on the #552
        // `parity_unclassified_les_co_present_*` fixture, closed
        // here.
        //
        // # LES survives classification (asymmetric with SBU)
        //
        // Unlike SBU-NF, there is no `capco/les-nf-evicted-by-classified`
        // Pattern-C row — §H.9 p185 explicitly retains the LES marking on
        // classified pages (`SECRET//NOFORN//LES`, not `SECRET//NOFORN`).
        // The lattice helper transmutes `{LesNf} → {Les}` while setting
        // `needs_nf = true` for cross-axis NF injection; this Pattern-A
        // row keeps the scheme path in step. See
        // `les_nf_classified_trigger`'s doc-comment for the full
        // asymmetry rationale and the
        // `parity_classified_les_nf_lattice_and_scheme_both_retain_les`
        // fixture for the regression gate.
        //
        // Trigger: `Custom(les_nf_classified_trigger)` — fires when
        // `is_classified(m) ∧ contains LES-NF in non_ic_dissem`.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Same
        // monotone-additive + idempotence policy as the SBU-NF entry.
        //
        // Axis annotations: reads `[CAT_CLASSIFICATION]`, writes
        // `[CAT_DISSEM]`. The non_ic_dissem scan lives in the
        // Custom predicate body per Pattern-C convention. Kahn places
        // this row BEFORE `capco/noforn-clears-rel-to` (DISSEM-reader)
        // so the REL TO axis is correctly cleared in the same
        // projection pass.
        //
        // # Source-doc internal contradiction (preserved note)
        //
        // The §H.9 p185 entry's Additional Marking Instructions field
        // reads "Applicable only to unclassified information" — which
        // conflicts with the Relationship(s) enumeration ("May be used
        // with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED"),
        // and conflicts with the Precedence Rules quote above
        // describing the canonical `SECRET//NOFORN//LES` form for
        // classified docs. The Relationship(s) field + the Precedence
        // Rule are the operative authorities; `NonIcDissem::LesNf`'s
        // doc-comment in `crates/ism/src/attrs.rs` makes the same
        // determination. A future ODNI manual revision may resolve
        // the Additional Marking Instructions artifact; for now this
        // row defers to the Precedence Rule.
        //
        // FUTURE: see the NODIS entry doc-comment for the SCI
        // Pattern A follow-on note.
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        PageRewrite::custom(
            "capco/les-nf-implies-noforn",
            capco(SectionLetter::H, 9, 185),
            CategoryPredicate::Custom(les_nf_classified_trigger),
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            LES_NF_IMPLIES_NF_READS,
            LES_NF_IMPLIES_NF_WRITES,
        ),
    ]
}
