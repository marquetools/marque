// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! #559 close-out (PM decision 2026-05-19) + #618 — RELIDO-eviction
//! PageRewrites that convert the three retired
//! `Constraint::Conflicts` rows (E055, E056, E057) from flag-only
//! diagnostics into subtractive page-level rewrites.
//!
//! Per Marque convention dissem-axis conflicts emit subtractive
//! fixes: the engine guides the author toward a canonical resolution
//! (RELIDO removed when a stronger originator decision is on the
//! page) rather than just flagging the conflict. The pre-#559 path
//! through `Constraint::Conflicts` + `fix_intent_by_name` carried
//! the same subtractive shape; this PR moves the shape into the
//! PageRewrite layer because that's the right scope for
//! cross-portion supersession (the per-portion Conflicts gate
//! missed the cross-portion case).
//!
//! Pre-#559 these triples lived as `Constraint::Conflicts` rows
//! whose `fix_intent_by_name` resolution attached a
//! `FactRemove(RELIDO, Portion)` repair. The portion-scope intent
//! could not fire at page projection: when ORCON appeared on one
//! portion and RELIDO on another, the per-portion conflict check
//! never triggered. PageRewrite at `Scope::Page` is the correct
//! scope for cross-portion supersession.
//!
//! ## Authority
//!
//! - **E055** (DISPLAY ONLY > RELIDO): CAPCO-2016 §H.8 p154 —
//!   DISPLAY ONLY entry, Relationship(s) to Other Markings clause
//!   marking RELIDO as incompatible.
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
//! ## DISPLAY ONLY parser-axis note (#618 fix)
//!
//! The DISPLAY ONLY row uses the same `Contains(CAT_DISSEM,
//! TOK_DISPLAY_ONLY)` trigger shape as its ORCON / ORCON-USGOV
//! siblings. Until #618 landed, this row was deferred because
//! the closure-suppressor predicate `satisfies(TOK_DISPLAY_ONLY)`
//! only scanned `attrs.dissem_iter()` for the `DissemControl::
//! Displayonly` variant — which the parser populates only
//! programmatically (`apply_fact_add`). The canonical wire form
//! `DISPLAY ONLY [LIST]` routes into `attrs.display_only_to`
//! instead, so the trigger would have silently no-op'd. #618
//! widened the predicate to OR both axes, unblocking this row.
//!
//! ## Direction of supersession
//!
//! All three rows remove RELIDO (the dominated token) rather than
//! the dominator. This matches the pre-existing
//! `fix_intent_by_name` resolution that #578 left in place for the
//! retired Conflicts rows. RELIDO is the "release-by-IDO" advisory
//! marker; ORCON / ORCON-USGOV each encode a stronger originator
//! decision that supersedes the IDO discretion; DISPLAY ONLY
//! encodes an authoritative release-list decision that likewise
//! overrides the advisory.
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
    SectionLetter, capco,
};

use super::super::{
    CAT_DISSEM, CapcoScheme, TOK_DISPLAY_ONLY, TOK_ORCON, TOK_ORCON_USGOV, TOK_RELIDO,
};

// Empty `reads` per the narrow-form rule (same convention as
// `supersession.rs::SUPERSESSION_READS`): the predicate scans
// CAT_DISSEM but declaring it would manufacture a mutual same-axis
// cycle between these three rows and their siblings (each both
// reading and writing CAT_DISSEM would form a 3-row cycle through
// `noforn-clears-fdr-family`). The single-row precedent at
// `noforn_clears.rs::NF_CLEARS_FDR_FAMILY_READS` declares
// `[CAT_DISSEM]` and gets away with the self-edge because the
// scheduler skips that case; with four sibling rows on the same
// axis the cycle becomes real. `writes = [CAT_DISSEM]` declares
// the actual mutation surface.
const RELIDO_CLEARS_READS: &[CategoryId] = &[];
const RELIDO_CLEARS_WRITES: &[CategoryId] = &[CAT_DISSEM];

/// The three #559 / #618 RELIDO-eviction rows (DISPLAY ONLY /
/// ORCON / ORCON-USGOV → RELIDO).
///
/// Row order is NOT load-bearing within this set — all three rows
/// are idempotent FactRemove operations on the same `CAT_DISSEM`
/// target, and the Kahn scheduler will reorder them as needed
/// against upstream writers.
pub(super) fn relido_clears_rows() -> Vec<PageRewrite<CapcoScheme>> {
    vec![
        // E.4.4 — DISPLAY ONLY > RELIDO at page scope (#618).
        //
        // CAPCO-2016 §H.8 p154 (DISPLAY ONLY entry, Relationship(s)
        // to Other Markings clause marking RELIDO incompatible).
        //
        // Pre-#559 this fired as `E055/display-only-conflicts-relido`
        // (`Constraint::Conflicts` row) with the same portion-scope
        // FactRemove intent. Migrated to Page scope for cross-portion
        // correctness; deferred behind #618 until
        // `satisfies(TOK_DISPLAY_ONLY)` was widened to recognize the
        // canonical `display_only_to` axis.
        PageRewrite::declarative(
            "capco/display-only-clears-relido",
            capco(SectionLetter::H, 8, 154),
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_DISPLAY_ONLY,
            },
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![FactRef::Cve(TOK_RELIDO)],
                scope: Scope::Page,
            }),
            RELIDO_CLEARS_READS,
            RELIDO_CLEARS_WRITES,
        ),
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
            capco(SectionLetter::H, 8, 136),
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
            capco(SectionLetter::H, 8, 140),
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
