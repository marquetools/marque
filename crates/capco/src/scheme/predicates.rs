// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` predicate helpers. Lifted from the monolithic `scheme.rs`
//! per the issue #466 split plan
//! (`claudedocs/refactor-466/split_proposal.md`).
//!
//! Covers presence predicates, satisfaction evaluators, class-floor and
//! SCI-per-system catalog dispatchers, and FD&R-family membership helpers.

use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, Span,
};
use marque_scheme::{
    CategoryId, TokenId,
    TokenRef,
};

use super::*;
use super::constraints::{
    class_floor_emit, e012_dual_classification, e014_joint_rel_to_coverage,
    e021_aea_requires_noforn, e024_rd_precedence, e038_dos_dissem_requires_noforn,
    sci_per_system_emit, w002_us_commingled_with_fgi,
};


/// Map a sentinel CVE `TokenId` to its [`CategoryId`].
///
/// Used by [`<CapcoScheme as MarkingScheme>::category_of`] to route
/// `FactRef::Cve(id)` to the right marking-axis. Returns `None` for
/// sentinels not associated with a concrete category (the marker
/// sentinels like `TOK_IC_DISSEM`, `TOK_NON_US_CLASSIFICATION`,
/// `TOK_US_CLASSIFIED`, `TOK_FGI_MARKER` are excluded — they label
/// categorical predicates in the constraint catalog rather than
/// addressable atomic tokens). The engine surfaces `None` as
/// [`ApplyIntentError::UnknownToken`].
///
/// The mapping mirrors the existing per-token presence semantics in
/// `satisfies_attrs` so a rule emitting `FactRemove(TOK_X)` lands on
/// the same axis where `satisfies_attrs` would look for `X`.
pub(crate) fn capco_token_category(id: TokenId) -> Option<CategoryId> {
    // Sentinel IDs are declared in the const block above (lines 60+).
    // Keep the matches in declaration order so a reviewer can trace
    // the catalog by line position.
    match id {
        // CAT_DISSEM — IC dissemination controls
        TOK_NOFORN
        | TOK_RELIDO
        | TOK_DISPLAY_ONLY
        | TOK_ORCON
        | TOK_ORCON_USGOV
        // Stage D (T108c) additions — IC dissem controls needed for closure-rule
        // triggers (IMCON, DSEN, RSEN, FOUO per §4.7.1 implicit-NOFORN / implicit-RELIDO):
        | TOK_IMCON
        | TOK_DSEN
        | TOK_RSEN
        | TOK_FOUO
        // PR 4b-C Commit 1: PROPIN / FISA / RAWFISA live in
        // `attrs.dissem_us` (DissemControl::Pr / Fisa / Rawfisa).
        // §H.8 p148 + §H.8 p161. verified 2026-05-16.
        | TOK_PROPIN
        | TOK_FISA
        | TOK_RAWFISA
        // EYES (USA/[LIST] EYES ONLY) routes through the IC dissem axis.
        // The sentinel landed in PR 3.7 rev 3; the category routing
        // here is PR 3.7 rev 4 per Copilot review pass 4 (token_category
        // returning None would break any closure/intent/tooling path
        // that needs the host category for cone-addition or audit-note
        // projection).
        | TOK_EYES => Some(CAT_DISSEM),
        // CAT_NON_IC_DISSEM — non-IC dissemination controls.
        // PR 3c.B Sub-PR 8.F.2 added `TOK_SBU_NF` and `TOK_LES_NF` so
        // the Pattern A `capco/sbu-nf-implies-noforn` / `capco/les-nf-implies-noforn`
        // PageRewrites can route through this category.
        // Stage D (T108c) adds LIMDIS, LES, SBU, SSI as closure-rule trigger
        // sentinels (§4.7.1 implicit-NOFORN list).
        // PR 4b-C Commit 1: TOK_NNPI lives in `attrs.non_ic_dissem`
        // (NonIcDissem::Nnpi). Closes issue #407. verified 2026-05-16.
        TOK_NODIS | TOK_EXDIS | TOK_SBU_NF | TOK_LES_NF | TOK_LIMDIS | TOK_LES | TOK_SBU
        | TOK_SSI | TOK_NNPI => Some(CAT_NON_IC_DISSEM),
        // CAT_REL_TO — country codes in the dissemination context.
        // `TOK_USA` removes USA from the axis; the `TOK_REL_TO`
        // sentinel (PR 3c.B Sub-PR 8.D.2) clears the whole axis. Both
        // route through the same category so `apply_fact_remove`'s
        // CAT_REL_TO branch can discriminate.
        TOK_USA | TOK_REL_TO => Some(CAT_REL_TO),
        // CAT_AEA — atomic-energy markings. ATOMAL lives in the AEA
        // axis per CAPCO-2016 §H.7 p122 worked example
        // (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`).
        TOK_RD | TOK_FRD | TOK_TFNI | TOK_CNWDI | TOK_UCNI | TOK_ATOMAL => Some(CAT_AEA),
        // CAT_SCI — sensitive compartmented information control systems.
        // BALK / BOHEMIA are NATO SAPs in the SCI category position per
        // §G.2 p40 + §H.7 p127 (rendered standalone, no SAR- prefix).
        TOK_HCS | TOK_BALK | TOK_BOHEMIA => Some(CAT_SCI),
        // CAT_JOINT_CLASSIFICATION — JOINT classification marker
        TOK_JOINT => Some(CAT_JOINT_CLASSIFICATION),
        // CAT_CLASSIFICATION — overall classification level surface
        TOK_RESTRICTED => Some(CAT_CLASSIFICATION),
        // Sentinel marker tokens (used in catalog predicates, not as
        // addressable atomic tokens): no category mapping.
        _ => None,
    }
}

/// Always-false [`CategoryPredicate::Custom`] body used by every
/// Phase-3 stub `PageRewrite` row.
///
/// The rewrite's `reads` / `writes` axes are what the Kahn scheduler
/// consumes (T031–T032). Its trigger body does not participate in
/// Phase 3 runtime dispatch because `Engine::lint` does not route
/// aggregation through `scheme.project(Scope::Page, …)` — the
/// hand-coded [`PageContext`] aggregator handles roll-up. Pinning the
/// trigger to `false` makes that no-op explicit: any test or tool
/// that calls `scheme.project()` on today's `CapcoScheme` will see
/// these rewrites declare but never fire.
pub(crate) fn never_fires(_: &CapcoMarking) -> bool {
    false
}

// ---------------------------------------------------------------------------
// PR 4b-C Commit 3 — Pattern-C strip-row helpers
// ---------------------------------------------------------------------------
//
// Pattern C (classification-driven strip) and the UCNI NOFORN-promotion
// pair both need predicates that gate on "the page is classified". The
// existing `CategoryPredicate::Contains` shape can't express that gate,
// so the seven rows in this PR use `CategoryPredicate::Custom`. The
// helpers below are top-level `fn` items so the rows can store them as
// `fn` pointers (`CategoryPredicate::Custom(fn(&CapcoMarking) -> bool)`)
// and the `Send + Sync` invariant from Constitution VI holds trivially.
//
// Authority (each verified 2026-05-16 against
// `crates/capco/docs/CAPCO-2016.md`):
// - §H.8 p134 (FOUO Precedence Rules for Banner Line Guidance)
// - §H.6 p116-117 (DOD UCNI / DCNI Precedence Rules)
// - §H.6 p118-119 (DOE UCNI Precedence Rules)
// - §H.9 p170 (LIMDIS Precedence Rules)
// - §H.9 p176 (SBU Precedence Rules)

/// `true` when the marking carries a classification level strictly
/// greater than UNCLASSIFIED.
///
/// Classifications without an effective US-level (`None`) are
/// treated as UNCLASSIFIED — Pattern-C rules fire only when there is
/// affirmative classified state on the page. Matches the §H.8 / §H.6
/// / §H.9 wording "classified document" (which presupposes a positive
/// classification, not a no-classification state).
#[inline]
pub(crate) fn is_classified(m: &CapcoMarking) -> bool {
    m.0.classification
        .as_ref()
        .map(|c| c.effective_level() > marque_ism::Classification::Unclassified)
        .unwrap_or(false)
}

/// `true` when the projected page carries DOD UCNI (DCNI) anywhere
/// on the AEA axis. Used by the Pattern-C
/// `capco/dod-ucni-evicted-by-classified` and
/// `capco/dod-ucni-promotes-noforn-when-classified` predicates.
#[inline]
pub(crate) fn has_dod_ucni(m: &CapcoMarking) -> bool {
    m.0.aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DodUcni))
}

/// `true` when the projected page carries DOE UCNI anywhere on the
/// AEA axis. Mirrors [`has_dod_ucni`] for the DOE side.
#[inline]
pub(crate) fn has_doe_ucni(m: &CapcoMarking) -> bool {
    m.0.aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DoeUcni))
}

/// `true` when an FD&R dissem marker is already present on the page.
///
/// §H.6 p116 / p118 verbatim: "NOFORN must be applied if a less
/// restrictive FD&R marking would otherwise be conveyed with the
/// classified information." The promotion is suppressed when an
/// equally- or more-restrictive FD&R marker is already present;
/// NOFORN is the most-restrictive member of the FD&R family
/// (§H.8 p145), so a present NOFORN is its own suppressor (no
/// double-add). The other FD&R-family tokens (REL TO / RELIDO /
/// DISPLAY ONLY / EYES) are "less restrictive" and DO NOT suppress
/// the promotion — they would be cleared by `noforn-clears-rel-to`
/// / `noforn-clears-fdr-family` downstream once the promotion fires.
///
/// Set membership matches the §H.8 p145 NOFORN-dominates family
/// scoped to "would otherwise be conveyed in the banner". We check
/// only `Nf` here so the promotion fires whenever NOFORN is absent
/// from the dissem axis. This is consistent with how the existing
/// `*-implies-noforn` rewrites add NOFORN with no FD&R-suppressor
/// gate (FactAdd of an already-present token is a per-intent no-op
/// per the idempotence policy in `apply_fact_add`).
#[inline]
pub(crate) fn dissem_has_noforn(m: &CapcoMarking) -> bool {
    m.0.dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf))
}

/// Pattern-C trigger: `classification > U ∧ contains FOUO in dissem`.
/// Drives `capco/fouo-evicted-by-classified` (§H.8 p134).
pub(crate) fn fouo_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m)
        && m.0
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Fouo))
}

/// Pattern-C trigger: `classification > U ∧ contains LIMDIS in non_ic`.
/// Drives `capco/limdis-evicted-by-classified` (§H.9 p170).
pub(crate) fn limdis_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m)
        && m.0
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, marque_ism::NonIcDissem::Limdis))
}

/// Pattern-C trigger: `classification > U ∧ contains SBU in non_ic`.
/// Drives `capco/sbu-evicted-by-classified` (§H.9 p176).
///
/// NOTE: This trigger matches the bare `Sbu` variant ONLY; the compound
/// `SbuNf` variant is a distinct token (TOK_SBU_NF) and is handled by
/// the existing `capco/sbu-nf-implies-noforn` rewrite at PR 3c.B
/// Sub-PR 8.F.2. §3.5 compound-NF invariant.
pub(crate) fn sbu_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m)
        && m.0
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, marque_ism::NonIcDissem::Sbu))
}

/// Pattern-C trigger: `classification > U ∧ DOD UCNI on AEA axis`.
/// Drives `capco/dod-ucni-evicted-by-classified` (§H.6 p116).
pub(crate) fn dod_ucni_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m) && has_dod_ucni(m)
}

/// Pattern-C trigger: `classification > U ∧ DOE UCNI on AEA axis`.
/// Drives `capco/doe-ucni-evicted-by-classified` (§H.6 p118).
pub(crate) fn doe_ucni_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m) && has_doe_ucni(m)
}

/// Pattern-C trigger: `dod-ucni-classified ∧ NOFORN absent from dissem`.
/// Drives `capco/dod-ucni-promotes-noforn-when-classified` (§H.6 p116).
pub(crate) fn dod_ucni_promotes_noforn_trigger(m: &CapcoMarking) -> bool {
    dod_ucni_classified_trigger(m) && !dissem_has_noforn(m)
}

/// Pattern-C trigger: `doe-ucni-classified ∧ NOFORN absent from dissem`.
/// Drives `capco/doe-ucni-promotes-noforn-when-classified` (§H.6 p118).
pub(crate) fn doe_ucni_promotes_noforn_trigger(m: &CapcoMarking) -> bool {
    doe_ucni_classified_trigger(m) && !dissem_has_noforn(m)
}

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
// See the `scheme.rs:5018-5039` doc-comment on `FDR_DOMINATORS` for
// the full distinction.

/// `true` when the dissem axis carries at least one IC dissem token
/// that is NOT in the FD&R-membership set (everything except
/// {Nf, Relido, Displayonly, Rel, Eyes}). Uses [`is_fdr_dissem_token`]
/// which walks `FDR_DOMINATORS` directly — the same broad-membership
/// semantic as `Vocabulary::is_fdr_dissem` but without constructing a
/// `CapcoScheme` instance inside a hot-path predicate.
///
/// The `FDR_DOMINATORS` slice includes `AnyInCategory(CAT_REL_TO)` to
/// cover bare REL marker membership, but `DissemControl::Rel` has no
/// `TOK_*` sentinel (verified `scheme.rs:4885` inventory comment), so
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
        // would otherwise have fired. See `scheme.rs:4885` for the full
        // sentinel inventory.
        other => {
            debug_assert!(
                matches!(other, DC::Rel | DC::ExemptFromIcd501Discovery),
                "dissem_to_tok hit an unexpected None arm for {other:?} — \
                 a DissemControl variant was added without a paired TOK_* \
                 sentinel. Extend `scheme.rs::dissem_to_tok` (and the broad-\
                 set `is_fdr_dissem_token` helper if the new control is \
                 FD&R-class) so Pattern-B / Pattern-C predicates can see it.",
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Predicate implementations (free functions — trait impls delegate here)
// ---------------------------------------------------------------------------
//
// `satisfies_attrs` and `evaluate_custom_by_attrs` are the source of
// truth for CAPCO's constraint semantics. They take `&CanonicalAttrs`
// directly to avoid forcing callers on the fast path to wrap in
// `CapcoMarking` (which would require cloning the attributes). The
// trait impls on `CapcoScheme` delegate to them, and the fast-path
// inherent method `CapcoScheme::evaluate_named_constraint` uses them
// directly to dispatch a single named constraint without walking
// the whole catalog.

/// Resolve a [`TokenRef`] against raw [`marque_ism::CanonicalAttrs`].
///
/// **Token-presence semantics** (T035):
/// - [`TokenRef::Token(id)`] returns true when the marking carries
///   the named token *anywhere* relevant — `TOK_USA` ⇒ "USA in
///   REL TO" (the dissemination context), `TOK_RD` ⇒ "RD anywhere in
///   `aea_markings`", etc.
/// - [`TokenRef::AnyInCategory(cat)`] returns true when the category
///   has at least one populated value. `CAT_DISSEM` intentionally
///   counts both the dissem axis (`dissem_us` and `dissem_nato`
///   together, walked via `attrs.dissem_iter()` post PR 9b / FR-046
///   split) AND `rel_to` as dissem-flavored presence, matching the
///   historical E015 predicate.
///
/// `MarkingClassification::Conflict` is deliberately excluded from
/// `TOK_NON_US_CLASSIFICATION` / `CAT_NON_US_CLASSIFICATION` — that
/// state is E012's concern, not E015's.
///
/// Sentinel `TokenId`s not used by the current catalog
/// (`TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`) fall through to `false`;
/// they are declared for future T035b consumption.
pub(crate) fn satisfies_attrs(attrs: &marque_ism::CanonicalAttrs, token_ref: &TokenRef) -> bool {
    use marque_ism::{
        AeaMarking, DissemControl, MarkingClassification, SciControl, SciControlBare,
        SciControlSystem,
    };
    match token_ref {
        TokenRef::Token(id) => match *id {
            TOK_NOFORN => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf)),
            TOK_USA => attrs.rel_to.contains(&CountryCode::USA),
            TOK_JOINT => {
                matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
            }
            TOK_RESTRICTED => matches!(
                &attrs.classification,
                Some(c) if c.effective_level() == Classification::Restricted
            ),
            TOK_RD => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Rd(_))),
            TOK_FRD => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Frd(_))),
            TOK_TFNI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Tfni)),
            TOK_CNWDI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Rd(rd) if rd.cnwdi)),
            TOK_UCNI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::DodUcni | AeaMarking::DoeUcni)),
            // PR 9c.1 (T134): ATOMAL lives in the AEA axis per
            // CAPCO-2016 §H.7 p122 (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`).
            TOK_ATOMAL => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Atomal(_))),
            // PR 9c.1 (T134): BALK / BOHEMIA are NATO SAPs living in
            // the SCI axis per CAPCO-2016 §G.2 p40 + §H.7 p127.
            TOK_BALK => attrs.sci_markings.iter().any(|m| {
                matches!(
                    m.system,
                    SciControlSystem::NatoSap(marque_ism::NatoSap::Balk)
                )
            }),
            TOK_BOHEMIA => attrs.sci_markings.iter().any(|m| {
                matches!(
                    m.system,
                    SciControlSystem::NatoSap(marque_ism::NatoSap::Bohemia)
                )
            }),
            // "HCS markings" is plural in CAPCO §H.3 p57 — it covers
            // the bare `HCS` token AND the compound forms `HCS-O` /
            // `HCS-P` / `HCS-O-P`. CVE-projection variants `Hcs`,
            // `HcsO`, `HcsP` are all matched explicitly; the
            // structural path via `sci_markings` covers any compound
            // anchored on `SciControlBare::Hcs` regardless of the
            // specific compartments attached.
            TOK_HCS => {
                attrs
                    .sci_controls
                    .iter()
                    .any(|s| matches!(s, SciControl::Hcs | SciControl::HcsO | SciControl::HcsP))
                    || attrs.sci_markings.iter().any(|m| {
                        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                    })
            }
            TOK_FGI_MARKER => {
                // FGI presence covers two disjoint axes:
                //   - `attrs.fgi_marker` for explicit `FGI` token in
                //     the dissem-axis position
                //   - `MarkingClassification::Fgi(_)` for foreign-classified
                //     portions like `//GBR SECRET` (the FGI lives on the
                //     classification axis, not the dissem-axis fgi_marker)
                // Per Copilot PR 3.7 review pass 3: prior to this fix
                // `satisfies_attrs(TOK_FGI_MARKER)` checked only
                // `attrs.fgi_marker.is_some()`, missing the
                // classification-axis case. The closure rule
                // `capco/noforn-if-fgi` would therefore not fire on
                // foreign-classified portions even though the trigger
                // declares both `TOK_FGI_MARKER` and
                // `AnyInCategory(CAT_FGI_MARKER)`.
                attrs.fgi_marker.is_some()
                    || matches!(&attrs.classification, Some(MarkingClassification::Fgi(_)))
            }
            TOK_US_CLASSIFIED => attrs.us_classification().is_some(),
            // `Conflict` deliberately excluded — see fn doc.
            TOK_NON_US_CLASSIFICATION => matches!(
                &attrs.classification,
                Some(
                    MarkingClassification::Fgi(_)
                        | MarkingClassification::Nato(_)
                        | MarkingClassification::Joint(_)
                )
            ),
            // `TOK_IC_DISSEM` and `TOK_NON_IC_DISSEM` have no live
            // consumers — the legacy E018/E019 constraints that
            // would have used them were retired in T035b as
            // over-restrictive. Kept as declared sentinels so any
            // future narrowly-scoped IC/non-IC dissem invariant
            // can dispatch against them without re-adding a
            // `TokenId` constant.
            TOK_IC_DISSEM | TOK_NON_IC_DISSEM => false,
            // T035c-21 PR-A: NODIS / EXDIS live in `non_ic_dissem`.
            // Both are DoS non-IC dissem controls per §H.9 (NODIS p174;
            // EXDIS p172).
            TOK_NODIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Nodis)),
            TOK_EXDIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Exdis)),
            // PR 3b.C (T026c): RELIDO incompatibility sentinels.
            // Pattern mirrors TOK_NOFORN above — scan via
            // `attrs.dissem_iter()` (namespace-agnostic walk over
            // `dissem_us ++ dissem_nato` post PR 9b / FR-046 split) for
            // the matching DissemControl variant. All four variants
            // exist in the generated values.rs; no new marque-ism edits
            // needed (Constitution VII compliance verified).
            TOK_RELIDO => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Relido)),
            TOK_DISPLAY_ONLY => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Displayonly)),
            TOK_ORCON => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Oc)),
            TOK_ORCON_USGOV => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::OcUsgov)),
            // Stage D (T108c) — new IC dissem sentinels for closure-rule triggers:
            TOK_IMCON => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Imc)),
            TOK_DSEN => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Dsen)),
            TOK_RSEN => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Rs)),
            TOK_FOUO => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Fouo)),
            // PR 4b-C Commit 1 — PROPIN / FISA / RAWFISA scan attrs.dissem_us
            // (the DissemControl variants `Pr`, `Fisa`, `Rawfisa`).
            // §H.8 p148 (PROPIN) + §H.8 p161 (FISA / RAWFISA).
            // verified 2026-05-16 against CAPCO-2016.md.
            TOK_PROPIN => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Pr)),
            TOK_FISA => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Fisa)),
            TOK_RAWFISA => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Rawfisa)),
            // Stage D (T108c) — non-IC dissem sentinels for closure-rule triggers:
            TOK_LIMDIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Limdis)),
            TOK_LES => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Les)),
            TOK_SBU => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Sbu)),
            TOK_SSI => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Ssi)),
            // PR 4b-C Commit 1 — NNPI scans attrs.non_ic_dissem for the
            // Nnpi variant. Closes issue #407. The CAPCO-2016 manual
            // does not explicitly enumerate NNPI; the in-tree authority
            // is `crates/ism/src/attrs.rs:1326` (NNPI banner-roll-up
            // doc-comment, propagates regardless of classification).
            // verified 2026-05-16 against the marque-ism attrs.rs entry.
            TOK_NNPI => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Nnpi)),
            // EYES sentinel for FD&R-set coverage (§H.8 p157). Per
            // Copilot PR 3.7 review pass 3: earlier comments claimed
            // EYES was covered via `CAT_REL_TO` fallthrough, which is
            // false — `CAT_REL_TO` only checks `attrs.rel_to`. EYES is
            // a `DissemControl::Eyes` variant produced by the parser
            // (deprecated 2017-10-01 per §H.8 p157 but still recognized
            // for legacy-input compatibility); this arm provides the
            // satisfies_attrs path that `FDR_DOMINATORS` membership
            // and `is_fdr_dominator` rely on.
            TOK_EYES => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Eyes)),
            _ => false,
        },
        TokenRef::AnyInCategory(cat) => match *cat {
            CAT_CLASSIFICATION => attrs.classification.is_some(),
            // `Conflict` deliberately excluded — see fn doc.
            CAT_NON_US_CLASSIFICATION => matches!(
                &attrs.classification,
                Some(
                    MarkingClassification::Fgi(_)
                        | MarkingClassification::Nato(_)
                        | MarkingClassification::Joint(_)
                )
            ),
            CAT_JOINT_CLASSIFICATION => {
                matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
            }
            CAT_SCI => !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty(),
            CAT_SAR => attrs.sar_markings.is_some(),
            CAT_AEA => !attrs.aea_markings.is_empty(),
            CAT_FGI_MARKER => {
                // Mirror TOK_FGI_MARKER (above): cover BOTH the
                // dissem-axis explicit-FGI token and the
                // classification-axis MarkingClassification::Fgi case.
                attrs.fgi_marker.is_some()
                    || matches!(&attrs.classification, Some(MarkingClassification::Fgi(_)))
            }
            CAT_DISSEM => attrs.dissem_iter().next().is_some() || !attrs.rel_to.is_empty(),
            CAT_REL_TO => !attrs.rel_to.is_empty(),
            CAT_DECLASSIFY_ON => attrs.declassify_on.is_some(),
            _ => false,
        },
    }
}

/// Route a `Constraint::Custom` by name to its scheme-private
/// predicate helper. Returns an empty `Vec` for unknown names
/// (forward-compat with future catalog entries).
///
/// PR 3b.D (T026d): catalog-row names with the prefixes
/// `class-floor/` or `E058/` are dispatched to
/// [`class_floor_catalog_eval`] over the static
/// [`CLASS_FLOOR_CATALOG`] table. The retired `e022_cnwdi_floor` /
/// `e025_ucni_classification` helpers were absorbed into the
/// catalog's static-table form; their replacement catalog rows
/// (`E058/CNWDI-classification-floor`,
/// `E058/DOD-UCNI-classification-ceiling`,
/// `E058/DOE-UCNI-classification-ceiling`,
/// `E058/SAR-classification-floor`) reuse the walker's `E058`
/// prefix rather than the legacy E022/E025/E027 IDs. Per project
/// memory `feedback_pre_users_no_deprecation_phasing.md`,
/// severity-config back-compat for the legacy IDs is intentionally
/// not preserved; `.marque.toml` keys must use `E058` (walker-level).
pub(crate) fn evaluate_custom_by_attrs(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    if is_class_floor_catalog_name(name) {
        return class_floor_catalog_eval(attrs, name);
    }
    if is_sci_per_system_catalog_name(name) {
        return sci_per_system_catalog_eval(attrs, name);
    }
    match name {
        "E010/HCS-system-constraints" => hcs_system_constraints(attrs, "CAPCO-2016 §H.4 pp 62-66"),
        "E012/dual-classification" => e012_dual_classification(attrs),
        "E014/joint-requires-rel-to-coverage" => e014_joint_rel_to_coverage(attrs),
        "E021/aea-requires-noforn" => e021_aea_requires_noforn(attrs),
        "E024/rd-precedence" => e024_rd_precedence(attrs),
        "W002/us-commingled-with-fgi" => w002_us_commingled_with_fgi(attrs),
        "capco/joint-requires-usa" => joint_requires_usa(attrs),
        "E038/nodis-or-exdis-requires-noforn" => e038_dos_dissem_requires_noforn(attrs),
        _ => Vec::new(),
    }
}

/// Free-function form of [`CapcoScheme::iter_present_tokens`] that
/// works directly on `&CanonicalAttrs`. Used by the trait impl above
/// AND by [`CapcoScheme::evaluate_named_constraint`]'s
/// `ConflictsWithFamily` dispatch (which receives raw attrs, not a
/// `CapcoMarking` — so it cannot call the trait method that wraps
/// `&marking.0`).
///
/// Per Copilot PR review on PR 3.7 (`evaluate_named_constraint` was
/// silently treating `ConflictsWithFamily` as a no-op): the fast-path
/// dispatch must emit one violation per (LHS, present_token) pair
/// where the family predicate holds — same algorithm as
/// `marque_scheme::constraint::evaluate`'s `ConflictsWithFamily` arm.
///
/// ## Forward-compat note (FGI / JOINT family predicates)
///
/// This function emits `TokenRef::Token(TOK_FGI_MARKER)` for FGI
/// classifications and `TokenRef::Token(TOK_JOINT)` for JOINT
/// classifications (concrete sentinels), but NATO is emitted as
/// `TokenRef::AnyInCategory(CAT_NON_US_CLASSIFICATION)` (category
/// shape). Family predicates that need to match FGI or JOINT MUST
/// accept either shape — a predicate that only matches
/// `AnyInCategory(CAT_FGI_MARKER)` will silently miss FGI portions
/// emitted as `Token(TOK_FGI_MARKER)`. PR 3.7 has no active
/// FGI- or JOINT-targeting family predicate so the asymmetry is
/// dormant; a future row that does match those axes should be
/// written as
/// `|t| matches!(t, TokenRef::Token(TOK_FGI_MARKER) | TokenRef::AnyInCategory(CAT_FGI_MARKER))`
/// (and analogously for JOINT / NATO).
pub(crate) fn collect_present_tokens(attrs: &marque_ism::CanonicalAttrs) -> Vec<TokenRef> {
    use marque_ism::{AeaMarking, DissemControl, MarkingClassification, NonIcDissem};
    let mut tokens = Vec::new();

    // Classification tokens
    if let Some(ref cls) = attrs.classification {
        match cls {
            MarkingClassification::Us(_) | MarkingClassification::Conflict { .. } => {}
            MarkingClassification::Fgi(_) => {
                tokens.push(TokenRef::Token(TOK_FGI_MARKER));
            }
            MarkingClassification::Nato(_) => {
                // NATO classification uses AnyInCategory(CAT_NON_US_CLASSIFICATION).
                tokens.push(TokenRef::AnyInCategory(CAT_NON_US_CLASSIFICATION));
            }
            MarkingClassification::Joint(_) => {
                tokens.push(TokenRef::Token(TOK_JOINT));
            }
        }
        if cls.effective_level() == marque_ism::Classification::Restricted {
            tokens.push(TokenRef::Token(TOK_RESTRICTED));
        }
    }

    // IC dissemination controls. PR 9b (T132): iterate across both
    // namespaces — the predicate emitter is namespace-agnostic; the
    // `TOK_*` sentinel reflects token identity, not attribution.
    for d in attrs.dissem_iter() {
        let tok = match d {
            DissemControl::Nf => Some(TOK_NOFORN),
            DissemControl::Relido => Some(TOK_RELIDO),
            DissemControl::Displayonly => Some(TOK_DISPLAY_ONLY),
            DissemControl::Oc => Some(TOK_ORCON),
            DissemControl::OcUsgov => Some(TOK_ORCON_USGOV),
            DissemControl::Imc => Some(TOK_IMCON),
            DissemControl::Dsen => Some(TOK_DSEN),
            DissemControl::Rs => Some(TOK_RSEN),
            DissemControl::Fouo => Some(TOK_FOUO),
            DissemControl::Eyes => Some(TOK_EYES),
            // Variants without TOK_* sentinels yet:
            //   Rel, Pr, Rawfisa, Fisa, ExemptFromIcd501Discovery
            //
            // DRIFT GUARD: `DissemControl` is `#[non_exhaustive]`. If
            // a future ODNI ISM schema bump adds a new variant, it
            // silently falls through to `None` here — meaning any
            // `Constraint::ConflictsWithFamily` row whose family
            // predicate should match the new control will silently
            // stop firing on it. When adding a new dissem control,
            // also: (a) add a `TOK_*` sentinel above, (b) add the
            // arm here, (c) consider whether existing family
            // predicates (`is_fdr_dominator`, `is_orcon_family`)
            // should include it. The compile-time signal is the
            // missing TOK_*; this code path is the runtime
            // backstop.
            _ => None,
        };
        if let Some(id) = tok {
            tokens.push(TokenRef::Token(id));
        }
    }

    // Non-IC dissemination controls
    for d in attrs.non_ic_dissem.iter() {
        let tok = match d {
            NonIcDissem::Nodis => Some(TOK_NODIS),
            NonIcDissem::Exdis => Some(TOK_EXDIS),
            NonIcDissem::SbuNf => Some(TOK_SBU_NF),
            NonIcDissem::LesNf => Some(TOK_LES_NF),
            NonIcDissem::Limdis => Some(TOK_LIMDIS),
            NonIcDissem::Les => Some(TOK_LES),
            NonIcDissem::Sbu => Some(TOK_SBU),
            NonIcDissem::Ssi => Some(TOK_SSI),
            // NonIcDissem is non-exhaustive; future variants fall through.
            _ => None,
        };
        if let Some(id) = tok {
            tokens.push(TokenRef::Token(id));
        }
    }

    // REL TO countries — emit AnyInCategory(CAT_REL_TO) if any country present
    if !attrs.rel_to.is_empty() {
        tokens.push(TokenRef::AnyInCategory(CAT_REL_TO));
    }

    // AEA markings
    for a in attrs.aea_markings.iter() {
        let tok = match a {
            AeaMarking::Rd(_) => Some(TOK_RD),
            AeaMarking::Frd(_) => Some(TOK_FRD),
            AeaMarking::Tfni => Some(TOK_TFNI),
            AeaMarking::DodUcni | AeaMarking::DoeUcni => Some(TOK_UCNI),
            _ => None,
        };
        if let Some(id) = tok {
            tokens.push(TokenRef::Token(id));
        }
    }

    // SCI controls
    if !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty() {
        tokens.push(TokenRef::AnyInCategory(CAT_SCI));
    }

    // SAR markings
    if attrs.sar_markings.is_some() {
        tokens.push(TokenRef::AnyInCategory(CAT_SAR));
    }

    // FGI marker
    if attrs.fgi_marker.is_some() {
        tokens.push(TokenRef::Token(TOK_FGI_MARKER));
    }

    tokens
}

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

/// `capco/joint-requires-usa` — JOINT classifications must list USA in BOTH
/// `joint.countries` AND `rel_to`. CAPCO §H.3 p55 (USA always included in
/// JOINT [LIST]) + §H.3 p57 (Requires REL TO USA, LIST).
pub(crate) fn joint_requires_usa(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let joint = match &attrs.classification {
        Some(marque_ism::MarkingClassification::Joint(j)) => j,
        _ => return Vec::new(),
    };
    let has_usa_in_rel_to = attrs.rel_to.contains(&CountryCode::USA);
    let joint_includes_usa = joint.countries.contains(&CountryCode::USA);
    if has_usa_in_rel_to && joint_includes_usa {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "capco/joint-requires-usa",
        message: "JOINT classifications must list USA in both the \
                  classification countries and REL TO"
            .to_owned(),
        citation: "CAPCO-2016 §H.3 pp 55–57",
        span: None,
        severity: None,
    }]
}

// ---------------------------------------------------------------------------
// HCS constraint handler (CAPCO-2016 §H.4 pp 62–66)
// ---------------------------------------------------------------------------

/// Evaluate the `Constraint::Custom("HCS-system-constraints")` sample.
///
/// CAPCO-2016 §H.4 (pp 62–66) defines the interlocking HCS rules:
///
/// 1. **Bare `HCS` (no compartment)** is a legacy form (§H.4 p62). It
///    must be remarked to `HCS-P`, `HCS-O`, or `HCS-O-P`, which requires
///    document-level analysis (the correct variant depends on whether
///    the content is HUMINT product, operations, or both). Legacy
///    `C//HCS` (CONFIDENTIAL with bare HCS -- no compartment) must
///    additionally be identified to the originator for correction.
/// 2. **`HCS-O`** (§H.4 p64) **requires ORCON and NOFORN** and must
///    **not** include ORCON-USGOV (banner would drop -USGOV).
/// 3. **`HCS-P`** (§H.4 p66) **requires NOFORN**; ORCON or ORCON-USGOV
///    **may** be used (permitted, not required).
/// 4. **`HCS-O` / `HCS-P`** are only authorized for SECRET and TOP
///    SECRET classifications (§H.4 p64 / p66).
///
/// This helper inspects both `sci_controls` (the CVE-projection for
/// legacy-shape bare HCS tokens) and `sci_markings` (the structural
/// view that carries compartment identifiers). Emits one
/// `ConstraintViolation` per failing rule per offending HCS entry.
///
/// By far the most common HCS compartment is `HCS-P` (Product).
/// HCS-O (Operations) is rarely encountered outside of CIA's walls.
/// But for users in that environment, they may encounter all three variants routinely.
pub(crate) fn hcs_system_constraints(
    attrs: &marque_ism::CanonicalAttrs,
    citation: &'static str,
) -> Vec<marque_scheme::ConstraintViolation> {
    use marque_ism::{DissemControl, SciControl, SciControlBare, SciControlSystem};

    let mut out = Vec::new();

    let classification = attrs.us_classification();
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc);
    let has_orcon_usgov = attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let high_enough = matches!(
        classification,
        Some(Classification::Secret) | Some(Classification::TopSecret)
    );

    // Walk structural sci_markings for HCS systems. This is the
    // authoritative source for the compartment identifier.
    for marking in attrs.sci_markings.iter() {
        let is_hcs = matches!(
            marking.system,
            SciControlSystem::Published(SciControlBare::Hcs)
        );
        if !is_hcs {
            continue;
        }

        if marking.compartments.is_empty() {
            // Bare HCS — legacy per CAPCO-2016 §H.4 p62.
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-bare",
                message: "Bare HCS is legacy; remark to HCS-P, HCS-O, or HCS-O-P per CAPCO-2016 \
                     §H.4 p62 (requires document-level analysis)."
                    .to_owned(),
                citation,
                span: None,
                severity: None,
            });
            if classification == Some(Classification::Confidential) {
                out.push(marque_scheme::ConstraintViolation {
                    constraint_label: "HCS-legacy-confidential",
                    message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction \
                              per CAPCO-2016 §H.4 p62."
                        .to_owned(),
                    citation,
                    span: None,
                    severity: None,
                });
            }
            continue;
        }

        // For each HCS-{first compartment} variant, apply the O/P
        // specific rules and the SECRET / TOP SECRET floor.
        for comp in marking.compartments.iter() {
            let id = comp.identifier.as_ref();
            match id {
                "O" => {
                    if !high_enough {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-classification-floor",
                            message: "HCS-O is only authorized for SECRET and TOP SECRET per \
                                      CAPCO-2016 §H.4 p64."
                                .to_owned(),
                            citation,
                            span: None,
                            severity: None,
                        });
                    }
                    if !has_orcon {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-ORCON",
                            message: "HCS-O requires ORCON per CAPCO-2016 §H.4 p64.".to_owned(),
                            citation,
                            span: None,
                            severity: None,
                        });
                    }
                    if has_orcon_usgov {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-forbids-ORCON-USGOV",
                            message: "HCS-O must not be used with ORCON-USGOV per CAPCO-2016 \
                                      §H.4 p64."
                                .to_owned(),
                            citation,
                            span: None,
                            severity: None,
                        });
                    }
                    // HCS-O requires NOFORN per CAPCO-2016 §H.4 p64
                    // ("Relationship(s) to Other Markings: ... Requires
                    // ORCON and NOFORN"). The ORCON side is enforced
                    // above; NOFORN is the second mandatory side. Same
                    // shape as the HCS-P NOFORN-required predicate
                    // below; tracked-and-resolved per #304.
                    let has_noforn = attrs.dissem_iter().any(|d| d == &DissemControl::Nf);
                    if !has_noforn {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-NOFORN",
                            message: "HCS-O requires NOFORN per CAPCO-2016 §H.4 p64.".to_owned(),
                            citation,
                            span: None,
                            severity: None,
                        });
                    }
                }
                "P" => {
                    if !high_enough {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-classification-floor",
                            message: "HCS-P is only authorized for SECRET and TOP SECRET per \
                                      CAPCO-2016 §H.4 p66."
                                .to_owned(),
                            citation,
                            span: None,
                            severity: None,
                        });
                    }
                    // HCS-P requires NOFORN per CAPCO-2016 §H.4 p66
                    // ("Relationship(s) to Other Markings: ... Requires
                    // NOFORN"). ORCON / ORCON-USGOV are permitted but
                    // not required ("ORCON or ORCON-USGOV may be
                    // used."), so the ORCON-required predicate that
                    // previously fired here was over-strict; it is
                    // dropped in favor of the actually-required
                    // NOFORN predicate.
                    let has_noforn = attrs.dissem_iter().any(|d| d == &DissemControl::Nf);
                    if !has_noforn {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-requires-NOFORN",
                            message: "HCS-P requires NOFORN per CAPCO-2016 §H.4 p66.".to_owned(),
                            citation,
                            span: None,
                            severity: None,
                        });
                    }
                }
                _ => {
                    // Other HCS compartments (e.g., agency-specific
                    // extensions not yet in this sample) fall through.
                }
            }
        }
    }

    // Back-compat: a portion may carry `SciControl::Hcs` (the CVE
    // projection for bare HCS) without producing a `sci_markings`
    // entry in every test path. Treat a bare `SciControl::Hcs` in the
    // projection but no corresponding `sci_markings` entry as legacy
    // bare HCS too. This keeps the handler robust to the two-path
    // storage (CVE enum vs structural) that `CanonicalAttrs` carries
    // for back-compat — see crate-level docs on the hybrid SCI model.
    let structural_has_hcs = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs)));
    let projection_has_bare_hcs = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Hcs));
    if projection_has_bare_hcs && !structural_has_hcs {
        out.push(marque_scheme::ConstraintViolation {
            constraint_label: "HCS-legacy-bare",
            // suggested fix should be HCS-P but we should expose a default override path for users in the HCS-O environment
            message: "HCS requires a compartment (O or P); remark to HCS-P, HCS-O, or HCS-O-P \
                 per CAPCO-2016 §H.4 p62 (requires document-level analysis)."
                .to_owned(),
            citation,
            span: None,
            severity: None,
        });
        if classification == Some(Classification::Confidential) {
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-confidential",
                message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction per \
                          CAPCO-2016 §H.4 p62."
                    .to_owned(),
                citation,
                span: None,
                severity: None,
            });
        }
    }

    out
}

/// Returns true if `name` is a catalog row name dispatched by
/// [`class_floor_catalog_eval`]. Used by `evaluate_custom_by_attrs`
/// to route on the table.
///
/// PR D R3.2 (R3 C1): O(1) prefix check. Every catalog row's `name`
/// follows one of two prefix conventions (see [`ClassFloorRow`]
/// docstring):
///
///   - `E058/<purpose>` for rows replacing a retired legacy rule.
///   - `class-floor/<marking>` for rows with no retired-rule
///     predecessor.
///
/// New catalog rows MUST follow one of these prefixes; the
/// `class_floor_catalog_naming_convention` test in
/// `crates/capco/tests/class_floor_catalog.rs` enforces the
/// invariant at build time so adding a row that doesn't follow the
/// convention fails CI.
pub(crate) fn is_class_floor_catalog_name(name: &str) -> bool {
    name.starts_with("E058/") || name.starts_with("class-floor/")
}

/// Resolve a catalog row by `name`. Returns `None` for unknown
/// names.
///
/// Walked only on the trait/validate path (27-row catalog → linear
/// scan, ≪1 µs) — the walker hot path uses
/// [`class_floor_catalog`] then [`class_floor_eval_row`] directly
/// with no name lookup. A build-time perfect-hash lookup
/// (`phf::Map`) is deferred unless the trait path shows up as a
/// measurable hotspot in profiling.
pub(crate) fn class_floor_row_by_name(name: &str) -> Option<&'static ClassFloorRow> {
    CLASS_FLOOR_CATALOG.iter().find(|row| row.name == name)
}

/// Resolve the diagnostic span anchor for a class-floor catalog row.
///
/// Lifted from `rules_declarative::class_floor_anchor_span` in PR
/// 3c.B Commit 7.3 when the `DeclarativeClassFloorRule` walker
/// retired into the engine's constraint-catalog bridge. Per PM
/// directive #2 of the original PR 3b.D plan, the span anchors at
/// the marking token (not the classification token) so the
/// diagnostic UX puts the squiggle under the offending presence.
/// Reads `row.primary_kind` directly (the PR D R2 perf-3
/// optimization hoisted from the retired `primary_token_kind_for_row`
/// string-match table into a struct field on `ClassFloorRow`).
/// Falls back to the first `Classification` token span if no
/// axis-specific span is found, and finally to `Span::new(0, 0)` if
/// neither is present.
pub(crate) fn class_floor_anchor_span(attrs: &CanonicalAttrs, row: &ClassFloorRow) -> Span {
    if let Some(kind) = row.primary_kind
        && let Some(span) = first_span_of_optional(attrs, kind)
    {
        return span;
    }
    // Some rows have no single primary kind (e.g., NATO rows have no
    // marking-side token; `row.primary_kind == None`). Try
    // classification as a fallback.
    if let Some(span) = first_span_of_optional(attrs, TokenKind::Classification) {
        return span;
    }
    Span::new(0, 0)
}

/// Returns the first span of a given token kind in the attrs'
/// `token_spans`, or `None` if the kind is absent. Lifted from
/// `rules_declarative::first_span_of_optional` in PR 3c.B Commit
/// 7.3 alongside [`class_floor_anchor_span`].
pub(crate) fn first_span_of_optional(attrs: &CanonicalAttrs, kind: TokenKind) -> Option<Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| t.kind == kind)
        .map(|t| t.span)
}

/// Dispatch a single catalog row by name and return at most one
/// `ConstraintViolation`. The trait-path entry point used by
/// [`MarkingScheme::validate`] →
/// [`marque_scheme::constraint::evaluate`] when the catalog row's
/// `Constraint::Custom` arm fires.
///
/// PR 3c.B Commit 7.3: the walker hot-path equivalent
/// (`class_floor_eval_row`) retired alongside
/// `DeclarativeClassFloorRule`; the engine's constraint-catalog
/// bridge invokes this function via `evaluate_custom` → here, and
/// fields are populated in [`class_floor_emit`] so no second emitter
/// path is needed.
pub(crate) fn class_floor_catalog_eval(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    class_floor_row_by_name(name)
        .and_then(|row| class_floor_emit(attrs, row))
        .map(|v| vec![v])
        .unwrap_or_default()
}

/// Returns true when the classification axis satisfies the floor policy.
///
/// The two policy variants take different views of the classification axis:
///
/// - **`AtLeast(floor)`** uses `MarkingClassification::effective_level`
///   so NATO / FGI / JOINT classifications get reciprocal-raised to
///   their US-equivalent level per `marque-applied.md` §3.4.1 Note (i)
///   (CTS → TS, NS → S, NC → C, NR → R, NU → U). This is the C1 fix
///   from PR #324 R1: before the fix, the NATO catalog rows
///   (BALK / BOHEMIA / ATOMAL) queried `attrs.us_classification()`,
///   which returns `None` for non-US classification kinds, so the
///   reciprocal-raised NATO floors always failed and always emitted a
///   spurious diagnostic — guaranteed false positive on every
///   well-formed NATO portion. The `effective_level()` accessor
///   already lives in `marque-ism` and is the canonical answer to
///   "what's the effective classification level for ordering?";
///   capco-side we just consume it.
///
///   Behavior on a `None` classification (no classification token
///   parsed at all) stays as "fail the floor" — this preserves
///   retired-E022 / retired-E027 semantics where a CNWDI / SAR marking
///   without any classification context is treated as malformed and
///   the floor diagnostic fires.
///
/// - **`EqualsU`** keeps `attrs.us_classification()` semantics. The
///   UCNI ceiling per CAPCO-2016 §H.6 p116 (DOD UCNI) and §H.6 p118
///   (DOE UCNI) is "May only be used with UNCLASSIFIED" — strictly the
///   US-classification system, not reciprocal-raised. A NATO-class
///   portion carrying UCNI is malformed input (UCNI is US AEA,
///   parallel to NATO ATOMAL); other rules catch the malformed shape.
pub(crate) fn class_floor_satisfied(attrs: &marque_ism::CanonicalAttrs, policy: ClassFloorPolicy) -> bool {
    match policy {
        ClassFloorPolicy::AtLeast(floor) => match attrs.classification.as_ref() {
            // Reciprocal-raise via `effective_level()`. NATO / FGI /
            // JOINT classifications return their US-equivalent level
            // for the comparison; US classifications return as-is.
            Some(c) => c.effective_level() >= floor,
            // No classification parsed at all → fail the floor.
            // Preserves retired-E022 / retired-E027 behavior on the
            // "classification is missing" case.
            None => false,
        },
        ClassFloorPolicy::EqualsU => match attrs.us_classification() {
            // Equals-U is the UCNI ceiling. `Some(Unclassified)` is the
            // only allowed state; everything else (including `None` for
            // pure-FGI / NATO / JOINT) fails. Mirrors retired E025
            // semantics: UCNI is US AEA and a non-US classification
            // carrying UCNI is malformed.
            Some(Classification::Unclassified) => true,
            _ => false,
        },
    }
}

// ---------------------------------------------------------------------------
// Family-presence predicates (one per catalog row)
// ---------------------------------------------------------------------------
//
// Each predicate iterates the relevant axis (`attrs.sci_markings`,
// `attrs.aea_markings`, `attrs.dissem_iter()` over the namespace
// split, etc.) looking for any token matching the family pattern.
// Family granularity is the §3.4.6
// author's choice — the predicates pattern-match across all marking-
// template-level leaves that belong to the family.

/// HCS-[comp][sub] — any HCS-anchored marking carrying a compartment
/// that has at least one sub-compartment. Family covers HCS-P [SUB] and
/// any future HCS sub-compartmented variants.
pub(crate) fn presence_hcs_comp_sub(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments
                .iter()
                .any(|c| !c.sub_compartments.is_empty())
    })
}

/// HCS-[comp] — any HCS-anchored marking carrying a compartment but no
/// sub-compartment (HCS-O, HCS-P bare). Family does NOT include HCS-X
/// (passthrough — see `presence_passthrough_hcs_x`) or bare HCS (legacy,
/// covered by E006/E008).
pub(crate) fn presence_hcs_comp_only(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && !m.compartments.is_empty()
            && m.compartments.iter().all(|c| c.sub_compartments.is_empty())
            // Exclude HCS-X: it's a passthrough family with its own row.
            && !m.compartments.iter().any(|c| c.identifier.as_str() == "X")
    })
}

/// SI-[comp] — any SI-anchored marking carrying at least one
/// compartment. Family covers SI-G, SI-G [SUB], SI-ECRU, SI-NONBOOK, and
/// any agency SI compartment per CAPCO-2016 §H.4 p76 (TS-only).
pub(crate) fn presence_si_comp(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && !m.compartments.is_empty()
    })
}

/// SI (bare) — any SI-anchored marking with NO compartment. Family is
/// the bare SI control system per §H.4 p74 (C-or-above floor).
pub(crate) fn presence_si_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && m.compartments.is_empty()
    })
}

/// TK-BLFH — any TK-anchored marking carrying a BLFH compartment (with
/// or without sub-compartments). §H.4 p87 / p89 — TS-only.
pub(crate) fn presence_tk_blfh(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Tk))
            && m.compartments
                .iter()
                .any(|c| c.identifier.as_str() == "BLFH")
    })
}

/// TK family at the S floor — TK bare, TK-IDIT (with/without sub-comp),
/// TK-KAND (with/without sub-comp). Excludes TK-BLFH (covered by
/// `presence_tk_blfh` at TS-only). §H.4 p85 / p91 / p95.
pub(crate) fn presence_tk_family(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        if !matches!(m.system, SciControlSystem::Published(SciControlBare::Tk)) {
            return false;
        }
        // Exclude markings whose compartment set includes BLFH — those
        // are §2.1 row TK-BLFH (TS-only), not §2.2 row TK (S floor).
        let has_blfh = m
            .compartments
            .iter()
            .any(|c| c.identifier.as_str() == "BLFH");
        !has_blfh
    })
}

/// RSV-[comp] — any RSV-anchored marking carrying a compartment.
/// CAPCO §H.4 p72.
pub(crate) fn presence_rsv_comp(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Rsv))
            && !m.compartments.is_empty()
    })
}

/// RD bare — RD without CNWDI and without SIGMA. CAPCO §H.6 p104 floor C.
pub(crate) fn presence_rd_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Rd(rd) if !rd.cnwdi && rd.sigma.is_empty()
        )
    })
}

/// RD-CNWDI — any RD block with `cnwdi == true`. Replaces retired E022.
/// CAPCO §H.6 p104 (TS-or-S RD); matches the catalog row's
/// authoritative §3.4.6 citation
/// (`E058/CNWDI-classification-floor` → `CAPCO-2016 §H.6 p104`).
pub(crate) fn presence_rd_cnwdi(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(rd) if rd.cnwdi))
}

/// RD-SIGMA — any RD block carrying at least one SIGMA number.
/// CAPCO §H.6 p108 / p113 (RD-SIGMA TS-or-S).
pub(crate) fn presence_rd_sigma(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(rd) if !rd.sigma.is_empty()))
}

/// FRD bare — FRD without SIGMA. CAPCO §H.6 p111 floor C.
pub(crate) fn presence_frd_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Frd(frd) if frd.sigma.is_empty()
        )
    })
}

/// FRD-SIGMA — any FRD block carrying at least one SIGMA number.
/// CAPCO §H.6 p113 (FRD-SIGMA TS-or-S).
pub(crate) fn presence_frd_sigma(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Frd(frd) if !frd.sigma.is_empty()))
}

/// TFNI present. CAPCO §H.6 p120 floor C.
pub(crate) fn presence_tfni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Tfni))
}

/// DOD UCNI present. Replaces half of retired E025.
pub(crate) fn presence_dod_ucni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DodUcni))
}

/// DOE UCNI present. Replaces half of retired E025.
pub(crate) fn presence_doe_ucni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DoeUcni))
}

/// SAR markings present. Replaces retired E027.
pub(crate) fn presence_sar(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.sar_markings.is_some()
}

/// RSEN dissem control present. CAPCO §H.8 p132 (operative §H.8 p149
/// per §3.4.6 author). RSEN's CVE form is `RS`
/// (the portion-mark abbreviation; banner form is `RSEN`).
pub(crate) fn presence_rsen(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs.dissem_iter().any(|d| matches!(d, DissemControl::Rs))
}

/// IMCON dissem control present.
pub(crate) fn presence_imcon(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs.dissem_iter().any(|d| matches!(d, DissemControl::Imc))
}

/// ORCON family — ORCON or ORCON-USGOV. The §3.4.6 single family entry
/// covers both because §H.8 p136 (ORCON) and p139 (ORCON-USGOV) both
/// require classification ≥ C and the §3.4.6 author groups them.
pub(crate) fn presence_orcon_family(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Oc | DissemControl::OcUsgov))
}

/// EYES ONLY portion mark / banner form. CAPCO §H.8 p157 (operative
/// §H.8 p152 per `marque-applied.md` Section 3.4.6 author).
pub(crate) fn presence_eyes_only(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Eyes))
}

/// BALK / BOHEMIA / ATOMAL — NATO control markings (not NATO
/// classifications) per CAPCO-2016 §G.2 p40 Table 5 (ARH by
/// Registered Marking).
///
/// PR 9c.1 T134 corrected the structural model:
///   - ATOMAL is an AEA-axis marking (CAPCO-2016 §H.7 p122 worked
///     example `SECRET//RD/ATOMAL//FGI NATO//NOFORN`), shared with
///     NATO+UK under §123/§144 sharing agreements.
///   - BALK / BOHEMIA are NATO SAPs in the SCI category position
///     (§G.2 p40 + §H.7 p127), rendered standalone with no `SAR-`
///     prefix.
///
/// The presence predicates read the corresponding canonical axes:
/// `aea_markings` for ATOMAL, `sci_markings` (via
/// `SciControlSystem::NatoSap`) for BALK / BOHEMIA. Legacy text
/// (`CTSA`, `CTS-B`, `CTS-BALK`, …) canonicalizes through the parser
/// (PR 9c.1 Commit 3), so this predicate fires on both well-formed
/// canonical input and on parsed legacy text.
pub(crate) fn presence_balk(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{NatoSap, SciControlSystem};
    attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Balk)))
}

pub(crate) fn presence_bohemia(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{NatoSap, SciControlSystem};
    attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Bohemia)))
}

pub(crate) fn presence_atomal(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::AeaMarking;
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Atomal(_)))
}

// ---------------------------------------------------------------------------
// Passthrough family predicates — §3.7 unknown-floor passthrough policy
// ---------------------------------------------------------------------------

/// BUR family — `BUR`, `BUR-BLG`, `BUR-DTP`, `BUR-WRG`. ISM-known SCI
/// control system; specific floor not enumerated in CAPCO-2016.
pub(crate) fn presence_passthrough_bur(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Bur)));
    let has_via_controls = attrs.sci_controls.iter().any(|s| {
        matches!(
            s,
            SciControl::Bur | SciControl::BurBlg | SciControl::BurDtp | SciControl::BurWrg
        )
    });
    has_via_markings || has_via_controls
}

/// HCS-X — ISM-known HCS variant; specific floor not enumerated.
pub(crate) fn presence_passthrough_hcs_x(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments.iter().any(|c| c.identifier.as_str() == "X")
    });
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::HcsX));
    has_via_markings || has_via_controls
}

/// KLM family — `KLM` / `KLAMATH`, `KLM-R`. ISM-known SCI control system.
pub(crate) fn presence_passthrough_klm(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Klm)));
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Klm | SciControl::KlmR));
    has_via_markings || has_via_controls
}

/// MVL family — `MVL` / `MARVEL`. ISM-known SCI control system.
pub(crate) fn presence_passthrough_mvl(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Mvl)));
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Mvl));
    has_via_markings || has_via_controls
}

// ---------------------------------------------------------------------------
// SCI per-system helpers — moved verbatim from rules_sci_per_system.rs
// (helper-relocation Option A per planning doc §4.1)
// ---------------------------------------------------------------------------

/// Is this `SciMarking` anchored on the given published bare system?
pub(crate) fn anchors_on(m: &marque_ism::SciMarking, system: marque_ism::SciControlBare) -> bool {
    use marque_ism::SciControlSystem;
    matches!(&m.system, SciControlSystem::Published(s) if *s == system)
}

/// Does any compartment under this marking carry the given identifier?
pub(crate) fn has_compartment(m: &marque_ism::SciMarking, id: &str) -> bool {
    m.compartments.iter().any(|c| c.identifier.as_str() == id)
}

/// Does the specific compartment carry at least one sub-compartment?
pub(crate) fn compartment_has_sub(m: &marque_ism::SciMarking, comp_id: &str) -> bool {
    m.compartments
        .iter()
        .any(|c| c.identifier.as_str() == comp_id && !c.sub_compartments.is_empty())
}

/// Is this a TK-BLFH, TK-IDIT, or TK-KAND marking (the three TK
/// compartments that require NOFORN per §H.4 p87 / p91 / p95)?
pub(crate) fn is_tk_noforn_compartment(m: &marque_ism::SciMarking) -> bool {
    use marque_ism::SciControlBare;
    anchors_on(m, SciControlBare::Tk)
        && m.compartments
            .iter()
            .any(|c| matches!(c.identifier.as_str(), "BLFH" | "IDIT" | "KAND"))
}

/// Find the first SCI-system/SCI-control token span in document order.
/// Used as the diagnostic anchor when the rule fires on a portion's SCI
/// block.
pub(crate) fn first_sci_span(attrs: &marque_ism::CanonicalAttrs) -> Option<marque_ism::Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| {
            matches!(
                t.kind,
                TokenKind::SciSystem
                    | TokenKind::SciControl
                    | TokenKind::SciCompartment
                    | TokenKind::SciSubCompartment
            )
        })
        .map(|t| t.span)
}

/// Observed US classification level, if any. Returns `None` for pure
/// foreign classifications (FGI/NATO/JOINT) — SCI-on-foreign is out of
/// §H.4's scope and handled by the foreign-classification rule cluster.
pub(crate) fn us_level(attrs: &marque_ism::CanonicalAttrs) -> Option<Classification> {
    use marque_ism::MarkingClassification;
    match attrs.classification {
        Some(MarkingClassification::Us(c)) => Some(c),
        Some(MarkingClassification::Conflict { us, .. }) => Some(us),
        _ => None,
    }
}

/// Last token span of the IC dissem block (anchors zero-width insertions).
/// Returns `None` when no IC dissem token exists.
pub(crate) fn last_dissem_span(attrs: &marque_ism::CanonicalAttrs) -> Option<marque_ism::Span> {
    attrs
        .token_spans
        .iter()
        .rev()
        .find(|t| t.kind == TokenKind::DissemControl)
        .map(|t| t.span)
}

/// Find the span (and current text) of a specific `DissemControl` token —
/// used when a rule needs to replace e.g. `OC-USGOV` with `OC`.
///
/// PR 9b (T132): walks the unified [`dissem_iter`](marque_ism::CanonicalAttrs::dissem_iter)
/// — which visits `dissem_us` first, then `dissem_nato` — and
/// correlates against the `token_spans` `DissemControl`-kind sequence
/// in document order. The parser emits dissem tokens to
/// `token_spans` once per source occurrence, irrespective of
/// post-parse attribution, so the iteration order through
/// `dissem_iter()` MUST match `token_spans` document order. This
/// holds because `attribute_dissems` partitions but does not
/// re-order: all `dissem_us` tokens come first by construction
/// (every non-NATO classification routes here), and `dissem_nato`
/// is non-empty only on pure-NATO portions where `dissem_us` is
/// empty by spec.
pub(crate) fn dissem_token_span(
    attrs: &marque_ism::CanonicalAttrs,
    target: marque_ism::DissemControl,
) -> Option<(marque_ism::Span, &str)> {
    for (dissem_idx, d) in attrs.dissem_iter().enumerate() {
        if *d == target {
            // Walk token_spans to find the Nth DissemControl.
            let tok = attrs
                .token_spans
                .iter()
                .filter(|t| t.kind == TokenKind::DissemControl)
                .nth(dissem_idx)?;
            return Some((tok.span, tok.text.as_ref()));
        }
    }
    None
}

/// Banner-form vs portion-form companion representation, given the
/// current dissem block. The parser preserves user-written text verbatim
/// in `TokenSpan::text`, so inserting in matching form avoids surprise
/// mixed-form output.
pub(crate) fn infer_companion_form(attrs: &marque_ism::CanonicalAttrs) -> CompanionForm {
    let first = attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DissemControl);
    match first.map(|t| t.text.as_ref()) {
        Some("NF") | Some("OC") | Some("OC-USGOV") => CompanionForm::Abbreviated,
        _ => CompanionForm::Full,
    }
}

/// Map a dissem-control surface form (`"NF"` / `"NOFORN"` / `"OC"` /
/// `"ORCON"` / `"OC-USGOV"` / `"ORCON-USGOV"`) to its CVE `TokenId`.
/// Surface-form distinction (banner abbrev vs portion abbrev vs full)
/// collapses at the canonical layer; the engine's `render_canonical`
/// decides emission form from the inferred companion form at the
/// insertion site. Returns `None` for unrecognized forms — the
/// caller routes those to the no-fix `Severity::Error` path rather
/// than silently substituting NOFORN. In normal flow the catalog
/// rows only ever pass `form.noforn()` or `form.orcon()` which
/// return one of the six recognized surface forms; an unrecognized
/// input represents a programming error (e.g., a new surface form
/// added without updating this lookup), and failing loudly is the
/// correct behavior.
#[inline]
pub(crate) fn dissem_token_id_for_form(token: &str) -> Option<TokenId> {
    match token {
        "NF" | "NOFORN" => Some(TOK_NOFORN),
        "OC" | "ORCON" => Some(TOK_ORCON),
        "OC-USGOV" | "ORCON-USGOV" => Some(TOK_ORCON_USGOV),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Family-presence predicates (one per PR-E catalog row)
// ---------------------------------------------------------------------------

/// HCS-O — any HCS-anchored marking carrying the "O" compartment.
/// §H.4 p64.
#[inline]
pub(crate) fn presence_hcs_o(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "O"))
}

/// HCS-P (any) — any HCS-anchored marking carrying the "P" compartment,
/// with or without sub-compartments. §H.4 p66 (and p68 inheriting NOFORN).
#[inline]
pub(crate) fn presence_hcs_p_any(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "P"))
}

/// HCS-P [SUB] — any HCS-anchored marking carrying a "P" compartment
/// with at least one sub-compartment. §H.4 p68. By §H.4 grammar, P is
/// the only HCS compartment that can carry sub-compartments, so this
/// coincides with `presence_hcs_comp_sub` from the class-floor catalog
/// in practice; we keep a separate predicate here to make the row
/// surface-explicit ("requires ORCON / forbids ORCON-USGOV on
/// sub-compartmented HCS-P").
#[inline]
pub(crate) fn presence_hcs_p_sub(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && compartment_has_sub(m, "P"))
}

/// SI-G — any SI-anchored marking carrying the "G" compartment, with or
/// without sub-compartments. §H.4 p80 (and p81 inheriting ORCON).
#[inline]
pub(crate) fn presence_si_g(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Si) && has_compartment(m, "G"))
}

/// TK with BLFH/IDIT/KAND compartment — any TK-anchored marking carrying
/// at least one of the three NOFORN-required compartments. §H.4 p87 +
/// p91 + p95.
#[inline]
pub(crate) fn presence_tk_compartment_noforn(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.sci_markings.iter().any(is_tk_noforn_compartment)
}

// ---------------------------------------------------------------------------
// Catalog dispatch
// ---------------------------------------------------------------------------

/// Returns true if `name` is a catalog row name dispatched by
/// [`sci_per_system_catalog_eval`]. Used by `evaluate_custom_by_attrs`
/// to route on the table.
///
/// O(1) prefix check — every catalog row's `name` MUST start with
/// `sci-per-system/`. The `sci_per_system_catalog_naming_convention`
/// test in `crates/capco/tests/sci_per_system_catalog.rs` enforces the
/// invariant at build time.
pub(crate) fn is_sci_per_system_catalog_name(name: &str) -> bool {
    name.starts_with("sci-per-system/")
}

/// Resolve a catalog row by `name`. Returns `None` for unknown names.
///
/// Walked only on the trait/validate path (5-row catalog → linear scan,
/// ≪1 µs). The walker hot path uses [`sci_per_system_catalog`] then
/// [`sci_per_system_emit`] directly with no name lookup.
pub(crate) fn sci_per_system_row_by_name(name: &str) -> Option<&'static SciPerSystemRow> {
    SCI_PER_SYSTEM_CATALOG.iter().find(|row| row.name == name)
}

/// Dispatch a single catalog row by name and return any
/// `ConstraintViolation`s. Trait-path entry point used by
/// [`MarkingScheme::validate`] →
/// [`marque_scheme::constraint::evaluate`] when the catalog row's
/// `Constraint::Custom` arm fires.
///
/// Note: PR-E rows produce `FixProposal` values on the walker path,
/// but `ConstraintViolation` doesn't carry a fix — the trait/validate
/// path drops the fix (this is the same divergence PR D's class-floor
/// catalog has). The engine path is the only path that produces
/// `AppliedFix` records, and the engine path always uses the walker.
pub(crate) fn sci_per_system_catalog_eval(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    let Some(row) = sci_per_system_row_by_name(name) else {
        return Vec::new();
    };
    // Trait-path doesn't have a candidate span (the engine's
    // bridge_sci_per_system_diagnostics direct path does). The
    // emitted Diagnostics are projected to ConstraintViolation
    // below which drops the fix payload — so the candidate_span
    // a Diagnostic's fix would have keyed on isn't observed here.
    // Pass an empty span as a sentinel; the resulting fix would be
    // dropped by the engine's `!f.span.is_empty()` filter even if a
    // hypothetical caller threaded it through.
    sci_per_system_emit(
        attrs,
        marque_ism::Span::new(0, 0),
        marque_scheme::Scope::Portion,
        row,
    )
    .into_iter()
    .map(|d| ConstraintViolation {
        constraint_label: row.name,
        message: String::from(d.message),
        citation: row.citation,
        span: None,
        severity: None,
    })
    .collect()
}
