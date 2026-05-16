// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! RELIDO family predicates ([`is_fdr_dominator`] /
//! [`is_orcon_family`]) used by `Constraint::ConflictsWithFamily`
//! rows. These are the two genuinely-`pub` names re-exported from
//! `crate::scheme::is_fdr_dominator` / `is_orcon_family` for the
//! PR 4 rule-wrapper dispatch. Lifted from the monolithic
//! `predicates.rs` per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_scheme::TokenRef;

use super::super::*;

// ---------------------------------------------------------------------------
// Stage D (PR 3.7 T108b) — RELIDO family predicates
// ---------------------------------------------------------------------------
//
// Family predicates for `Constraint::ConflictsWithFamily` rows. These
// express the RELIDO incompatibility set in a compact, distributively-
// equivalent form rather than enumerating each individual conflict.

/// Returns `true` if `t` is an FD&R dominator — a token that sits at or
/// above RELIDO in the FD&R supersession chain per CAPCO-2016 §D.2
/// Table 3 p28.
///
/// FD&R dominators are the tokens from Table 2 (p21) whose presence in
/// a marking means an explicit FD&R decision exists; RELIDO is
/// structurally incompatible with any FD&R dominator because RELIDO's
/// SFDRA-deferred-release semantic conflicts with the manifest FD&R
/// authority of the dominator.
///
/// Per the family-predicate framing in `marque-applied.md` (RELIDO
/// incompatibility roster) + CAPCO-2016 §H.8 p154 (RELIDO
/// Relationship(s) to Other Markings: "Cannot be used with NOFORN or
/// DISPLAY ONLY") + §D.2 Table 3 p28.
///
/// Used by `Constraint::ConflictsWithFamily` in `CapcoScheme::constraints`
/// to compact the RELIDO conflict catalog from two enumerated rows
/// (E054/E055) to one family row.
pub fn is_fdr_dominator(t: &TokenRef) -> bool {
    match t {
        TokenRef::Token(id) => {
            // NOFORN, DISPLAY_ONLY, and EYES are FD&R dominators over
            // RELIDO per §D.2 Table 3 p28. RELIDO-vs-RELIDO is a
            // tautology and is omitted. EYES added in PR 3.7 rev 3
            // per Copilot review pass 3: the parser produces
            // `DissemControl::Eyes` for legacy `(U//EYES)` inputs
            // (deprecated 2017-10-01 per §H.8 p157 but still
            // recognized), so `is_fdr_dominator` must match it for
            // RELIDO + EYES conflicts to be reportable.
            matches!(*id, TOK_NOFORN | TOK_DISPLAY_ONLY | TOK_EYES)
        }
        TokenRef::AnyInCategory(cat) => {
            // REL TO (any country list) is an FD&R dominator over RELIDO
            // per §H.8 p154 (the RELIDO prohibition text covers NOFORN and
            // DISPLAY ONLY explicitly; REL TO is covered by §H.8 p150-153
            // which establishes REL TO as a mutual-exclusion peer of RELIDO
            // in the FD&R family). The CAT_REL_TO arm captures this.
            *cat == CAT_REL_TO
        }
    }
}

/// Returns `true` if `t` is an ORCON-family token (ORCON or ORCON-USGOV).
///
/// Used by `Constraint::ConflictsWithFamily` to express E056 (ORCON ⊥ RELIDO)
/// and E057 (ORCON-USGOV ⊥ RELIDO) as a single family row. Per CAPCO-2016
/// §H.8 p136 (ORCON) and §H.8 p140 (ORCON-USGOV), both "May not be used
/// with RELIDO."
pub fn is_orcon_family(t: &TokenRef) -> bool {
    match t {
        TokenRef::Token(id) => matches!(*id, TOK_ORCON | TOK_ORCON_USGOV),
        TokenRef::AnyInCategory(_) => false,
    }
}
