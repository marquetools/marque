// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! #552 — same-axis compound-supersedes-bare PageRewrites: bare SBU
//! superseded by compound SBU-NF; bare LES superseded by compound
//! LES-NF. Both rows are classification-independent and write the
//! `non_ic_dissem` axis they trigger on.
//!
//! # Pattern shape
//!
//! Each row uses a `Contains` trigger over the compound token
//! (`TOK_SBU_NF` / `TOK_LES_NF`) plus an `Intent(FactRemove)` action
//! against the bare token. The trigger fires on every compound
//! presence (regardless of bare co-presence); the FactRemove
//! action is the per-intent no-op (`IntentInapplicable`) when the
//! bare token isn't present, so the effective behavior is the
//! co-presence supersession. Mirrors the monotone-additive
//! idempotence pattern in Pattern-A's `*-implies-noforn` rows.
//!
//! Reads/writes annotations use empty `reads` and a single-axis
//! `writes = [CAT_NON_IC_DISSEM]`. The scheduler's
//! `schedule_rewrites` tolerates empty `reads` on `Contains+Intent`
//! rewrites (see its doc-comment on the `UnannotatedCustomAxes` vs.
//! declarative-empty-allowed distinction). Declaring
//! `CAT_NON_IC_DISSEM` as `reads` on either row would create a
//! mutual cycle because both rows ALSO write `CAT_NON_IC_DISSEM`
//! — the scheduler would add an edge in each direction (writer
//! → reader for each of the two rows), forming a 2-cycle. Empty
//! `reads` is the narrow-form choice per `rewrites/mod.rs`.
//!
//! # Runtime execution gap
//!
//! Per the `pattern_a.rs:131-137` doc-comment, declarative rewrites
//! are scheduler-validated but execution-deferred today —
//! banner-validation flows through the lattice helper
//! ([`marque_capco::lattice::NonIcDissemSet::from_attrs_iter`])
//! which carries the same supersession semantics. These two rows
//! mirror the lattice fix so the scheme path stays in shape for
//! runtime wiring. End-to-end behavior today is visible
//! through `scheme.project(Scope::Page, ...)` exercising the
//! lattice helper.

use marque_scheme::{
    CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent, Scope,
    SectionLetter, capco,
};

use super::super::*;

/// The two #552 same-axis supersession rows: bare SBU dropped on
/// co-presence with SBU-NF; bare LES dropped on co-presence with
/// LES-NF.
pub(super) fn supersession_rows() -> Vec<PageRewrite<CapcoScheme>> {
    // Empty `reads` per the narrow-form rule (predicate scans
    // CAT_NON_IC_DISSEM but declaring it would manufacture a mutual
    // same-axis cycle with the sibling row); single-axis
    // `writes = [CAT_NON_IC_DISSEM]`.
    const SUPERSESSION_READS: &[marque_scheme::CategoryId] = &[];
    const SUPERSESSION_WRITES: &[marque_scheme::CategoryId] = &[CAT_NON_IC_DISSEM];

    vec![
        // #552 — `capco/sbu-nf-supersedes-sbu`.
        //
        // §H.9 p178 (SBU NOFORN Precedence Rules for Banner Line
        // Guidance, verbatim): "When a document contains both SBU-NF
        // and SBU portions, SBU NOFORN supersedes SBU in the banner
        // line."
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_SBU_NF)` — fires
        // when SBU-NF is present on the page's `non_ic_dissem` axis.
        //
        // Action: `Intent(FactRemove { Cve(TOK_SBU), Scope::Page })`
        // — drops bare SBU. Per-intent no-op
        // (`IntentInapplicable`, silent) when bare SBU isn't in
        // `non_ic_dissem`, so the net effect is the co-presence
        // supersession. On classified pages the existing Pattern-C
        // `capco/sbu-nf-evicted-by-classified` (§H.9 p178 Commingling
        // Rule) then strips SbuNf, leaving the non-IC axis empty
        // with NOFORN injected via the parallel Pattern-A
        // `capco/sbu-nf-implies-noforn` — net banner
        // `SECRET//NOFORN`. On unclassified pages SbuNf survives and
        // renders as `UNCLASSIFIED//SBU NOFORN`.
        PageRewrite::declarative(
            "capco/sbu-nf-supersedes-sbu",
            capco(SectionLetter::H, 9, 178),
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_SBU_NF,
            },
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_SBU)],
                scope: Scope::Page,
            }),
            SUPERSESSION_READS,
            SUPERSESSION_WRITES,
        ),
        // #552 — `capco/les-nf-supersedes-les`.
        //
        // §H.9 p185 derivation: the LES-NF entry's Authorized Banner
        // Line Marking Title is "LAW ENFORCEMENT SENSITIVE NOFORN"
        // and its Example Banner Line is `UNCLASSIFIED//LES NOFORN`
        // (banner-form heading + Notional Example Page 1 at §H.9
        // p185). The LES-NF compound carries the LES family marker
        // in unclassified banner form, so bare LES is redundant when
        // LES-NF is also present.
        //
        // Trigger: `Contains(CAT_NON_IC_DISSEM, TOK_LES_NF)` —
        // fires when LES-NF is present on the page's
        // `non_ic_dissem` axis.
        //
        // Action: `Intent(FactRemove { Cve(TOK_LES), Scope::Page })`
        // — drops bare LES. Per-intent no-op when bare LES isn't
        // present (`IntentInapplicable`, silent). On classified
        // pages the lattice helper's classified gate
        // (`NonIcDissemSet::from_attrs_iter`, §H.9 p185 LES NOFORN
        // Precedence Rules for Banner Line Guidance) then splits
        // LesNf back into `{Les}` + NOFORN injection — net banner
        // `SECRET//NOFORN//LES`. On unclassified pages LesNf
        // survives and renders as `UNCLASSIFIED//LES NOFORN`.
        //
        // Asymmetry note: bare-SBU dispatch was already wired in
        // PR 4b-C / #541. The TOK_LES dispatch arm was previously
        // unwired (LesNf was treated as `UnknownToken` per the
        // pre-#552 doc-comment in
        // `actions/intent.rs::apply_fact_remove`); #552 adds the
        // TOK_LES arm to enable this row.
        PageRewrite::declarative(
            "capco/les-nf-supersedes-les",
            capco(SectionLetter::H, 9, 185),
            CategoryPredicate::Contains {
                category: CAT_NON_IC_DISSEM,
                token: TOK_LES_NF,
            },
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_LES)],
                scope: Scope::Page,
            }),
            SUPERSESSION_READS,
            SUPERSESSION_WRITES,
        ),
    ]
}
