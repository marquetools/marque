// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! #559 close-out (PM decision 2026-05-19) — RELIDO-eviction
//! PageRewrites that convert two of the three retired
//! `Constraint::Conflicts` rows (E056 + E057) from flag-only
//! diagnostics into subtractive page-level rewrites.
//!
//! Per Marque convention dissem-axis conflicts emit subtractive fixes
//! (see `~/.claude/memory/feedback_dissem_conflicts_emit_subtractive_fix.md`).
//! Pre-#559 these pairs lived as `Constraint::Conflicts` rows
//! whose `fix_intent_by_name` resolution attached a
//! `FactRemove(RELIDO, Portion)` repair. The portion-scope intent
//! could not fire at page projection: when ORCON appeared on one
//! portion and RELIDO on another, the per-portion conflict check
//! never triggered. PageRewrite at `Scope::Page` is the correct
//! scope for cross-portion supersession.
//!
//! ## Authority
//!
//! - **E056** (ORCON > RELIDO): CAPCO-2016 §H.8 p136 — ORCON entry,
//!   Relationship(s) to Other Markings: *"May not be used with
//!   RELIDO."*
//! - **E057** (ORCON-USGOV > RELIDO): CAPCO-2016 §H.8 p140 —
//!   ORCON-USGOV entry, Relationship(s) to Other Markings: same
//!   exclusion as ORCON.
//!
//! Each citation re-verified against `crates/capco/docs/CAPCO-2016.md`
//! at the time of authorship per Constitution VIII.
//!
//! ## E.4.4 (DISPLAY ONLY > RELIDO) — deferred
//!
//! The PM closeout's third item — converting E055 (DISPLAY ONLY >
//! RELIDO at §H.8 p154) to a PageRewrite — is deferred. The closure
//! predicate `satisfies(TOK_DISPLAY_ONLY)` scans `attrs.dissem_iter()`
//! for `DissemControl::Displayonly`, but the parser routes the
//! canonical wire form `DISPLAY ONLY [LIST]` into
//! `attrs.display_only_to` (a separate axis); the `DissemControl`
//! variant is set only programmatically (`apply_fact_add`). A
//! page-scope `Contains(CAT_DISSEM, TOK_DISPLAY_ONLY)` predicate
//! therefore won't fire on the canonical input. Two paths exist:
//!
//! 1. A `CategoryPredicate::Custom` that reads both axes — but the
//!    cross-axis read/write graph (this row writes CAT_DISSEM; the
//!    sibling `noforn-clears-display-only-to` row reads CAT_DISSEM
//!    and writes CAT_DISPLAY_ONLY_TO) forms a 3-row cycle with
//!    `noforn-clears-fdr-family`. The Kahn scheduler rejects it at
//!    `Engine::new` with `RewriteCycle`. Empty `reads` would break
//!    the cycle but `Custom` triggers require non-empty `reads`
//!    (scheduler `UnannotatedCustomAxes`).
//! 2. A wider engine-layer fix that widens
//!    `satisfies(TOK_DISPLAY_ONLY)` to also check
//!    `attrs.display_only_to`. That's the right home for the
//!    predicate gap (same gap motivates S008's clause 2b in
//!    `crates/capco/src/rules.rs`) but touches engine-adjacent code
//!    outside #559's scope.
//!
//! Net effect: the post-#578 byte-level behavior of E055 is partial.
//! `(S//RELIDO//DISPLAY ONLY GBR)` on a single portion still trips
//! the closure's RELIDO-incompatibility paths via S008's clause 2b
//! (S008 won't suggest adding RELIDO to a portion that already has
//! DISPLAY ONLY); the cross-portion case (RELIDO on one portion,
//! DISPLAY ONLY on another) loses the auto-eviction this PR
//! intended to deliver. Tracked as a follow-up against the
//! `satisfies(TOK_DISPLAY_ONLY)` engine-layer widening; the
//! corresponding `capco/display-only-clears-relido` PageRewrite
//! will land once the predicate gap is closed.
//!
//! ## Direction of supersession
//!
//! Both shipped rows remove RELIDO (the dominated token) rather
//! than the dominator. This matches the pre-existing
//! `fix_intent_by_name` resolution that #578 left in place for the
//! retired Conflicts rows. RELIDO is the "release-by-IDO" advisory
//! marker; ORCON / ORCON-USGOV each encode a stronger originator
//! decision that supersedes the IDO discretion.
//!
//! ## Trigger and action shape
//!
//! Each row uses `Contains(CAT_DISSEM, <dominator-token>)` as the
//! trigger and `Intent(FactRemove { [TOK_RELIDO], Scope::Page })` as
//! the action — exactly parallel to the existing
//! `capco/noforn-clears-fdr-family` row in `noforn_clears.rs`.
//! Idempotent: FactRemove of an absent token is a per-intent no-op
//! (`IntentInapplicable`, silent), so pages without RELIDO
//! experience no effect.

use marque_scheme::{
    CategoryAction, CategoryId, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent, Scope,
};

use super::super::{CAT_DISSEM, CapcoScheme, TOK_ORCON, TOK_ORCON_USGOV, TOK_RELIDO};

// Empty `reads` per the narrow-form rule (same convention as
// `supersession.rs::SUPERSESSION_READS`): the predicate scans
// CAT_DISSEM but declaring it would manufacture a mutual same-axis
// cycle between this row and the three siblings (display-only /
// orcon / orcon-usgov each both reading and writing CAT_DISSEM
// would form a 3-row cycle). The single-row precedent at
// `noforn_clears.rs::NF_CLEARS_FDR_FAMILY_READS` declares
// `[CAT_DISSEM]` and gets away with the self-edge because the
// scheduler skips that case; with four sibling rows on the same
// axis the cycle becomes real. `writes = [CAT_DISSEM]` declares
// the actual mutation surface.
const RELIDO_CLEARS_READS: &[CategoryId] = &[];
const RELIDO_CLEARS_WRITES: &[CategoryId] = &[CAT_DISSEM];

/// The two #559 RELIDO-eviction rows that survived the cycle-graph
/// constraints (ORCON / ORCON-USGOV → RELIDO). The DISPLAY ONLY row
/// is deferred — see the module header for the rationale + the
/// follow-up plan.
///
/// Row order is NOT load-bearing within this set — both rows are
/// idempotent FactRemove operations on the same `CAT_DISSEM` target,
/// and the Kahn scheduler will reorder them as needed against
/// upstream writers.
pub(super) fn relido_clears_rows() -> Vec<PageRewrite<CapcoScheme>> {
    vec![
        // E.4.3 (1/2) — ORCON > RELIDO at page scope.
        //
        // CAPCO-2016 §H.8 p136 (ORCON entry, Relationship(s) to
        // Other Markings): *"May not be used with RELIDO."*
        //
        // Pre-#559 this fired as `E056/orcon-conflicts-relido`
        // (`Constraint::Conflicts` row) with the same portion-scope
        // FactRemove intent. Migrated to Page scope for cross-portion
        // correctness.
        PageRewrite::declarative(
            "capco/orcon-clears-relido",
            "CAPCO-2016 §H.8 p136",
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_ORCON,
            },
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_RELIDO)],
                scope: Scope::Page,
            }),
            RELIDO_CLEARS_READS,
            RELIDO_CLEARS_WRITES,
        ),
        // E.4.3 (2/2) — ORCON-USGOV > RELIDO at page scope.
        //
        // CAPCO-2016 §H.8 p140 (ORCON-USGOV entry, Relationship(s) to
        // Other Markings): same exclusion as ORCON. ORCON-USGOV is
        // the more restrictive variant (USGOV-only release control),
        // so the same supersession holds — RELIDO is the dominated
        // marker.
        //
        // Pre-#559 this fired as `E057/orcon-usgov-conflicts-relido`
        // (`Constraint::Conflicts` row). Migrated to Page scope.
        PageRewrite::declarative(
            "capco/orcon-usgov-clears-relido",
            "CAPCO-2016 §H.8 p140",
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_ORCON_USGOV,
            },
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_RELIDO)],
                scope: Scope::Page,
            }),
            RELIDO_CLEARS_READS,
            RELIDO_CLEARS_WRITES,
        ),
    ]
}
