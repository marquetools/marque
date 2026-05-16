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
};

use super::super::*;

/// The four Pattern-A NOFORN-supremacy rows in declaration order.
///
/// Each row reads `CAT_NON_IC_DISSEM` (to detect the NODIS / EXDIS /
/// SBU-NF / LES-NF trigger) and writes `CAT_DISSEM` (to inject
/// NOFORN). The Kahn scheduler orders all four BEFORE
/// `capco/noforn-clears-rel-to` (DISSEM-reader) so the REL TO axis
/// is correctly cleared in the same projection pass when any of the
/// triggers is present.
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
    // No existing rewrite writes `CAT_NON_IC_DISSEM`, so the new
    // rewrites have no upstream producers on that axis and can run
    // in declaration order relative to each other.
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

    // PR 3c.B Sub-PR 8.F.2 — SBU-NF and LES-NF Pattern A axes.
    // Same axis-flow as the 8.F NODIS/EXDIS pair: reads
    // `CAT_NON_IC_DISSEM` (to detect the SBU-NF / LES-NF token)
    // and writes `CAT_DISSEM` (to add NOFORN). Both new entries
    // join the same DISSEM-writer cohort and are ordered BEFORE
    // `capco/noforn-clears-rel-to` (DISSEM-reader) by the Kahn
    // scheduler.
    const SBU_NF_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
    const SBU_NF_IMPLIES_NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];
    const LES_NF_IMPLIES_NF_READS: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];
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
        // idempotence policy in `apply_fact_add` (scheme.rs:624-639).
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
            "CAPCO-2016 §H.9 p174",
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
            "CAPCO-2016 §H.9 p172",
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
        // PR 3c.B Sub-PR 8.F.2 — `capco/sbu-nf-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p178 (SBU-NF) does NOT contain a "Requires NOFORN."
        // sentence — unlike NODIS (§H.9 p174) and EXDIS (§H.9 p172). The NF
        // implication is derived from three structural anchors:
        //   (a) Banner-form heading at `CAPCO-2016.md:4388-4398`: the marking's
        //       Authorized Banner Line Marking Title literally names it
        //       "SENSITIVE BUT UNCLASSIFIED NOFORN"; portion mark is `SBU-NF`.
        //       NOFORN is a structural component of the marking's identity.
        //   (b) Commingling Rule at `CAPCO-2016.md:4420-4421`: confirms NOFORN
        //       persists after transmutation strips the SBU half — even when
        //       the source token is dropped, the NF must remain in the portion.
        //       Verbatim: "The SBU-NF marking is conveyed in the portion
        //       mark only if the commingled portion is unclassified and there
        //       is no other NOFORN information included in the portion. If
        //       there is other NOFORN information in the commingled portion,
        //       the 'SBU' marking is used and a NOFORN marking is added,
        //       e.g., (U//NF//SBU)." And p4421: "If the portion is
        //       classified, the classification level of the portion
        //       adequately protects the SBU information, so SBU is not
        //       reflected in the portion mark; however a NOFORN marking
        //       must be added to the portion mark, e.g., (C//NF)."
        //   (c) §D.2 Table 3 row 3-5 at `CAPCO-2016.md:590-595`: lists NOFORN
        //       as the FD&R banner consequence for SBU-NF. Back-reference
        //       confirms the page-level dissem-axis invariant.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_SBU_NF)` — fires
        // when any portion on the page carries SBU-NF in its
        // `non_ic_dissem` axis. Resolved by the
        // `capco_category_contains` extension in PR 3c.B Sub-PR 8.F.2.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Monotone-
        // additive: FactAdd with an already-present token is a
        // per-intent no-op (IntentInapplicable, silent) per the
        // idempotence policy in `apply_fact_add`'s `CAT_DISSEM` arm
        // (the `if category == CAT_DISSEM` block; the
        // `attrs.dissem_iter().any(|d| d == &target)` check returns
        // `IntentInapplicable`). NOT the unmatched-arm fallthrough at
        // the bottom of `apply_fact_add`, which is forward-
        // compatibility only — see the TODO at the CAT_NON_IC_DISSEM
        // arm of `apply_fact_remove`. Line numbers omitted because
        // they drift with refactors; grep `apply_fact_add` to find
        // the current location.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. The Kahn scheduler places this rewrite
        // BEFORE `capco/noforn-clears-rel-to` (which reads
        // CAT_DISSEM) so the REL TO axis is correctly cleared in the
        // same projection pass when SBU-NF is present.
        //
        // Classification: §H.9 p178 at `:4410` says SBU-NF "May only
        // be used with UNCLASSIFIED" — but the trigger predicate is
        // classification-agnostic (it scans the `non_ic_dissem`
        // axis only). On malformed classified input `(C//SBU-NF)`,
        // Pattern A still fires defensively; the eventual Pattern C
        // `classified-strips-sbu` rewrite will canonicalize the
        // portion to `(C//NF)` per the §H.9 Commingling Rule.
        //
        // FUTURE (SCI Pattern A follow-on): see the NODIS entry
        // doc-comment for the SCI follow-on (§H.4 p64/p68/p87/p91/p95).
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        // Scheduler-validated but execution-deferred; visible through
        // `scheme.project(Scope::Page, …)`.
        PageRewrite::declarative(
            "capco/sbu-nf-implies-noforn",
            "CAPCO-2016 §H.9 p178",
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_SBU_NF,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            SBU_NF_IMPLIES_NF_READS,
            SBU_NF_IMPLIES_NF_WRITES,
        ),
        // PR 3c.B Sub-PR 8.F.2 — `capco/les-nf-implies-noforn`.
        //
        // CAPCO-2016 §H.9 p185 (LES-NF) does NOT contain a "Requires NOFORN."
        // sentence — unlike NODIS (§H.9 p174) and EXDIS (§H.9 p172). The NF
        // implication is derived from three structural anchors:
        //   (a) Banner-form heading at `CAPCO-2016.md:4532-4542`: the marking's
        //       Authorized Banner Line Marking Title literally names it
        //       "LAW ENFORCEMENT SENSITIVE NOFORN"; portion mark is `LES-NF`.
        //       NOFORN is a structural component of the marking's identity.
        //   (b) Precedence Rules for Banner Line Guidance at `CAPCO-2016.md:4558`:
        //       "When a classified document contains portions of U//LES- NF,
        //       the 'LES' marking is used in the banner line and the NOFORN
        //       marking is applied as a Dissemination Control Marking. For
        //       example: SECRET//NOFORN//LES."
        //       // note: source has whitespace OCR artifact "LES- NF" rendered
        //       // with a space; canonical token is LES-NF.
        //       Confirms NOFORN materializes on the projected page dissem
        //       axis even when the LES-NF source token is consolidated into
        //       its LES form by transmutation.
        //   (c) §D.2 Table 3 rows 6-8 at `CAPCO-2016.md:590-595`: lists NOFORN
        //       as the FD&R banner consequence for LES-NF. Back-reference
        //       confirms the page-level dissem-axis invariant.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_LES_NF)` — fires
        // when any portion on the page carries LES-NF in its
        // `non_ic_dissem` axis. Resolved by the
        // `capco_category_contains` extension in PR 3c.B Sub-PR 8.F.2.
        //
        // Action: `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })`
        // — adds NOFORN to the projected page dissem axis. Same
        // monotone-additive + idempotence policy as the SBU-NF entry.
        //
        // Axis annotations: reads `[CAT_NON_IC_DISSEM]`, writes
        // `[CAT_DISSEM]`. Scheduler ordering: same sibling position
        // as the other three `*-implies-noforn` entries — all four
        // are DISSEM-writers ordered before `noforn-clears-rel-to`
        // (DISSEM-reader). The four entries are DAG siblings (no
        // ordering dependency between them).
        //
        // Classification: §H.9 p185's Relationship(s) field at
        // `:4554` says LES-NF "May be used with TOP SECRET, SECRET,
        // CONFIDENTIAL, or UNCLASSIFIED." Unlike SBU-NF, LES-NF is
        // valid at any classification level. Pattern A fires
        // regardless.
        //
        // Source-doc internal contradiction: the same §H.9 p185 entry
        // at `:4552` (Additional Marking Instructions field) reads
        // "Applicable only to unclassified information" — which
        // appears to conflict with the Relationship(s) enumeration
        // at `:4554`. The Relationship(s) field governs behavioral
        // scope (it explicitly enumerates the permitted classification
        // levels) and is the authority for §H.9 entries. The
        // `:4552` line appears to be a vestigial paste from the
        // sibling LES entry (`:4471`, where LES IS unclassified-
        // only) — it is internally inconsistent with `:4554` AND
        // with the Precedence Rule at `:4558` which describes the
        // canonical `SECRET//NOFORN//LES` form for classified docs.
        // `NonIcDissem`'s implementation at `crates/ism/src/attrs.rs`
        // (LesNf variant doc-comment) makes the same `:4554`-governs
        // determination. A future ODNI manual revision may resolve
        // the `:4552` artifact; for now Pattern A defers to `:4554`.
        //
        // FUTURE: see the NODIS entry doc-comment for the SCI
        // Pattern A follow-on note.
        //
        // Runtime execution gap: see the NODIS entry doc-comment.
        PageRewrite::declarative(
            "capco/les-nf-implies-noforn",
            "CAPCO-2016 §H.9 p185",
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_LES_NF,
            },
            CategoryAction::Intent(ReplacementIntent::FactAdd {
                token: FactRef::Cve(TOK_NOFORN),
                scope: Scope::Page,
            }),
            LES_NF_IMPLIES_NF_READS,
            LES_NF_IMPLIES_NF_WRITES,
        ),
    ]
}
