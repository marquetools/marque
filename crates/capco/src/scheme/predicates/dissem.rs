// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pattern-B FOUO-eviction helpers plus the FD&R-membership / dissem-
//! sentinel / `rel_to_covers` utilities used across the predicate
//! family. Lifted from the monolithic `predicates.rs` per the issue
//! #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_scheme::{CategoryId, TokenId, TokenRef};

use super::super::*;

// ---------------------------------------------------------------------------
// PR 4b-C Commit 4 — Pattern-B helpers
// ---------------------------------------------------------------------------
//
// Pattern B is two structural rows per the §H.8 p134 verbatim:
//   "FOUO is not conveyed in the banner line if the document is
//    UNCLASSIFIED with FOUO and other dissemination control markings,
//    excluding any FD&R markings."
//
// The "non-FD&R" set comes from `Vocabulary::is_fdr_dissem` (broad
// membership including RELIDO). The Pattern-B trigger fires whenever
// FOUO is present AND there is at least one other non-FD&R control on
// the page (other IC dissem, non-IC dissem, AEA, or SAR). The
// `classification > U` companion gate is split into the dedicated
// Pattern-C `fouo-evicted-by-classified` row so each row has a single
// §-citation thread.
//
// `Vocabulary::is_fdr_dissem` is the authoritative FD&R-membership
// API (`crates/scheme/src/vocabulary.rs:382`; CapcoScheme override at
// `crates/capco/src/vocabulary.rs:1093`). It iterates `FDR_DOMINATORS`
// — which INCLUDES RELIDO. The neighboring `is_fdr_dominator`
// function deliberately EXCLUDES RELIDO; it answers a different
// question (RELIDO-conflict dominators) and is the wrong helper here.
// See the `FDR_DOMINATORS` doc-comment in
// `crates/capco/src/scheme/closure.rs` for the full distinction.

/// `true` when the dissem axis carries at least one IC dissem token
/// that is NOT in the FD&R-membership set (everything except
/// {Nf, Relido, Displayonly, Rel, Eyes}). Uses [`is_fdr_dissem_token`]
/// which walks `FDR_DOMINATORS` directly — the same broad-membership
/// semantic as `Vocabulary::is_fdr_dissem` but without constructing a
/// `CapcoScheme` instance inside a hot-path predicate.
///
/// The `FDR_DOMINATORS` slice includes `AnyInCategory(CAT_REL_TO)` to
/// cover bare REL marker membership, but `DissemControl::Rel` has no
/// `TOK_*` sentinel (verified against the `TOK_*` sentinel block in
/// `crates/capco/src/scheme/mod.rs`), so
/// the per-variant token lookup naturally skips REL — `dissem_to_tok`
/// returns `None` and the caller treats `None` as non-FD&R. That is
/// CORRECT for the §H.8 p134 reading: bare `Rel` IS an FD&R marker by
/// §B.3.a p19 (REL TO without the country list still belongs to the
/// FD&R family), so a future PR that lands `TOK_REL` and adds it to
/// `FDR_DOMINATORS` would automatically pick up here. Today, no
/// portion can carry a bare `Rel` without an accompanying `rel_to`
/// entry (the parser produces them together), so the gap is
/// non-load-bearing on real input.
#[inline]
pub(crate) fn dissem_has_non_fdr_other_than_fouo(m: &CapcoMarking) -> bool {
    m.0.dissem_iter().any(|d| {
        if matches!(d, marque_ism::DissemControl::Fouo) {
            return false; // not "other"; the trigger token itself
        }
        match dissem_to_tok(*d) {
            Some(tok) => !is_fdr_dissem_token(tok),
            None => true,
        }
    })
}

/// `true` when `tok` appears in the in-tree `FDR_DOMINATORS` slice as
/// a direct `Token(...)` entry. Mirrors the `Vocabulary::is_fdr_dissem`
/// override at `crates/capco/src/vocabulary.rs:1093` for the
/// `Token` arms — the `AnyInCategory` arms in `FDR_DOMINATORS`
/// (CAT_REL_TO) cover country codes on the REL TO axis, not
/// `DissemControl` tokens, so they are excluded from this lookup
/// path. PR 4b-C local helper for Pattern-B's per-token
/// membership check; once `Vocabulary::is_fdr_dissem` is reachable
/// from a no-scheme-instance context (e.g., a `&'static` helper),
/// this helper can delegate.
#[inline]
pub(crate) fn is_fdr_dissem_token(tok: TokenId) -> bool {
    FDR_DOMINATORS.iter().any(|entry| match entry {
        TokenRef::Token(id) => *id == tok,
        TokenRef::AnyInCategory(_) => false,
    })
}

/// Pattern-B trigger: `contains FOUO in dissem ∧ ∃ other non-FD&R
/// control on the page`.
///
/// "Other non-FD&R control" covers four axes per §H.8 p134's "other
/// dissemination control markings, excluding any FD&R markings":
/// - CAT_DISSEM: any IC dissem token that is not FOUO and not in
///   the FD&R set.
/// - CAT_NON_IC_DISSEM: ANY non-IC dissem token (§H.9 controls are
///   all non-FD&R by construction — none of {LIMDIS, LES, SBU, SSI,
///   NODIS, EXDIS, NNPI, SbuNf, LesNf} appears in `FDR_DOMINATORS`).
/// - CAT_AEA: ANY AEA marking. AEA markings (RD / FRD / TFNI / UCNI /
///   ATOMAL) are atomic-energy controls, not FD&R markings.
/// - CAT_SAR: ANY SAR program identifier.
///
/// Drives `capco/non-fdr-control-evicts-fouo` (§H.8 p134, Correction A).
pub(crate) fn fouo_with_non_fdr_other_control_trigger(m: &CapcoMarking) -> bool {
    let has_fouo =
        m.0.dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Fouo));
    if !has_fouo {
        return false;
    }
    // AEA-non-empty triggers Pattern-B row 2 for any AEA marking
    // including ATOMAL; practical overlap with U-document is null
    // (ATOMAL requires classified per §H.7 p122). The §H.8 p134
    // sub-clause is U-document scoped, so the only AEA markings that
    // can co-occur with FOUO in practice are UCNI variants (which
    // ARE U-document valid per §H.6 p116 / p118) — RD / FRD / TFNI /
    // ATOMAL all carry per-marking class floors that exceed U.
    // Keeping the unconditional `!aea_markings.is_empty()` clause is
    // correct under §H.8 p134's wording and stays defensive against
    // future grammar extensions.
    dissem_has_non_fdr_other_than_fouo(m)
        || !m.0.non_ic_dissem.is_empty()
        || !m.0.aea_markings.is_empty()
        || m.0.sar_markings.is_some()
}

/// Helper: DissemControl variant → TOK_* sentinel for the
/// `Vocabulary::is_fdr_dissem` membership lookup.
///
/// Mirrors the `dissem_to_tok` arms scattered through `scheme.rs`
/// (no centralized helper exists at PR 4b-C time). Variants without
/// a TOK_* sentinel return `None`; the caller treats those as
/// non-FD&R (correct by inspection — FD&R members all have
/// TOK_* sentinels).
#[inline]
pub(crate) fn dissem_to_tok(d: marque_ism::DissemControl) -> Option<TokenId> {
    use marque_ism::DissemControl as DC;
    match d {
        DC::Nf => Some(TOK_NOFORN),
        DC::Relido => Some(TOK_RELIDO),
        DC::Displayonly => Some(TOK_DISPLAY_ONLY),
        DC::Oc => Some(TOK_ORCON),
        DC::OcUsgov => Some(TOK_ORCON_USGOV),
        DC::Imc => Some(TOK_IMCON),
        DC::Dsen => Some(TOK_DSEN),
        DC::Rs => Some(TOK_RSEN),
        DC::Fouo => Some(TOK_FOUO),
        DC::Pr => Some(TOK_PROPIN),
        DC::Fisa => Some(TOK_FISA),
        DC::Rawfisa => Some(TOK_RAWFISA),
        DC::Eyes => Some(TOK_EYES),
        // Variants without a TOK_* sentinel: only `DC::Rel` (REL TO
        // canonical, routed via CAT_REL_TO instead of CAT_DISSEM) and
        // `DC::ExemptFromIcd501Discovery` (parser-internal marker, never
        // emitted onto the dissem axis). Adding a new DissemControl
        // variant without extending this match arm + the catalog is a
        // silent-drift class — the debug_assert below catches it under
        // `cargo test` before it can mask a Pattern-B trigger that
        // would otherwise have fired. See the `TOK_*` sentinel block in
        // `crates/capco/src/scheme/mod.rs` for the full inventory.
        other => {
            debug_assert!(
                matches!(other, DC::Rel | DC::ExemptFromIcd501Discovery),
                "dissem_to_tok hit an unexpected None arm for {other:?} — \
                 a DissemControl variant was added without a paired TOK_* \
                 sentinel. Extend this `dissem_to_tok` function (and the broad-\
                 set `is_fdr_dissem_token` helper if the new control is \
                 FD&R-class) so Pattern-B / Pattern-C predicates can see it.",
            );
            None
        }
    }
}

/// Classify a [`CategoryId`] for dispatch-loop separator selection.
/// CAT_DISSEM and CAT_REL_TO are both §H.8 dissem-family per §G.1
/// Table 4 row 8; non-IC dissem (§H.9, CAT_NON_IC_DISSEM) is its
/// own major category per §A.6 p16 and is NOT a dissem-family
/// member here.
pub(crate) fn dissem_family_of(cat: CategoryId) -> DissemFamilyMembership {
    if cat == CAT_DISSEM || cat == CAT_REL_TO {
        DissemFamilyMembership::Member
    } else {
        DissemFamilyMembership::Other
    }
}

/// Returns `true` if `trigraph` is directly in `rel_to` or is a member of any
/// tetragraph in `rel_to` (e.g., GBR is covered when FVEY appears in REL TO).
pub(crate) fn rel_to_covers(rel_to: &[marque_ism::CountryCode], trigraph: &str) -> bool {
    rel_to.iter().any(|r| {
        r.as_str() == trigraph
            || crate::vocab::expand_tetragraph(r.as_str())
                .is_some_and(|members| members.contains(&trigraph))
    })
}
