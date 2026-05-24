// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CategoryPredicate::Contains` / `CategoryPredicate::Empty` and
//! `CategoryAction::Clear` / `CategoryAction::Replace` dispatch
//! against `CapcoMarking`. Lifted from the monolithic `actions.rs`
//! per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_ism::MarkingClassification;
use marque_scheme::{CategoryId, TokenId};

use super::super::*;

// ---------------------------------------------------------------------------
// Category-predicate / category-action dispatch (for PageRewrite)
// ---------------------------------------------------------------------------
//
// These helpers implement the trigger and action variants of a
// `PageRewrite` against CAPCO's `CapcoMarking`. They're here rather
// than in `marque-scheme` because the variant payloads reference
// `S::Token` and `S::Marking` and each scheme has to project those
// onto its concrete storage. The `CategoryPredicate::Custom` /
// `CategoryAction::Custom` variants still skip this dispatch and let
// the rewrite author supply the closure directly, but cross-category
// rewrites such as CAPCO's NOFORN rule are also supported in
// declarative form here.

/// `CategoryPredicate::Contains { category, token }` evaluator.
///
/// Unhandled `(category, token)` pairs return `false` — a safe
/// conservative answer that disables the rewrite rather than silently
/// misfiring. Coverage expands as more rewrites move to the declarative
/// form.
///
/// The `CAT_NON_IC_DISSEM` arms for `TOK_NODIS` / `TOK_EXDIS` let the
/// `capco/nodis-implies-noforn` and `capco/exdis-implies-noforn`
/// PageRewrites' `Contains` triggers resolve against the
/// `non_ic_dissem` axis. Without them, the rewrites would silently
/// never fire (the conservative-`false` fallthrough disables them).
///
/// The `CAT_NON_IC_DISSEM` block also has arms for `TOK_SBU_NF` and
/// `TOK_LES_NF`, scanning the `NonIcDissem::SbuNf` / `NonIcDissem::LesNf`
/// variants. Same shape, same silent-disabling concern — the
/// `capco/sbu-nf-implies-noforn`
/// (§H.9 p178) and `capco/les-nf-implies-noforn` (§H.9 p185) PageRewrite
/// triggers require these arms to resolve.
///
/// The match-arm dispatches on `TokenId` constants for routing and scans
/// the `NonIcDissem` enum variants in `attrs.non_ic_dissem` in the body —
/// the same two-form separation used by the existing `(CAT_DISSEM,
/// TOK_NOFORN)` arm (dispatches on `TOK_NOFORN`, scans
/// `DissemControl::Nf`).
pub(crate) fn capco_category_contains(
    m: &CapcoMarking,
    category: CategoryId,
    token: TokenId,
) -> bool {
    let attrs = &m.0;
    if category == CAT_DISSEM && token == TOK_NOFORN {
        // "Contains NOFORN" is namespace-agnostic — the
        // dissem token is what matters, not its attribution. Scan
        // across both fields via `dissem_iter`.
        return attrs
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    }
    if category == CAT_DISSEM && token == TOK_DISPLAY_ONLY {
        // #618: DISPLAY ONLY has a parser-axis split. The canonical wire
        // form `DISPLAY ONLY [LIST]` is routed by the parser into
        // `attrs.display_only_to` (a country-list axis parallel to
        // `attrs.rel_to`), NOT into `dissem_us` as a `DissemControl`
        // variant — the `Displayonly` variant is set only programmatically
        // via `apply_fact_add`. Mirrors the widening in `satisfies_attrs`:
        // a PageRewrite trigger such as `capco/display-only-clears-relido`
        // would silently no-op without this arm because the canonical
        // wire form populates `display_only_to` instead of `dissem_us`.
        return attrs
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Displayonly))
            || !attrs.display_only_to.is_empty();
    }
    // CAT_NON_IC_DISSEM arms for NODIS and EXDIS enable the
    // `capco/nodis-implies-noforn` and `capco/exdis-implies-noforn`
    // PageRewrite triggers to resolve. CAT_NON_IC_DISSEM arms for SBU-NF and
    // LES-NF. Same purpose: enable the `capco/sbu-nf-implies-noforn`
    // and `capco/les-nf-implies-noforn` PageRewrite triggers to
    // resolve against `attrs.non_ic_dissem`. Without these arms,
    // the Pattern A rewrites would silently never fire (the
    // conservative-`false` fallthrough disables them).
    if category == CAT_NON_IC_DISSEM {
        if token == TOK_NODIS {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Nodis));
        }
        if token == TOK_EXDIS {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Exdis));
        }
        if token == TOK_SBU_NF {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::SbuNf));
        }
        if token == TOK_LES_NF {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::LesNf));
        }
    }
    false
}

/// `CategoryPredicate::Empty { category }` evaluator.
///
/// Unhandled categories return `true` (treated as "non-empty / unknown")
/// so an `Empty` predicate on an unknown category **does not fire**
/// and a rewrite conditioned on it stays inert. This matches
/// [`capco_category_contains`]'s conservative-false stance and avoids
/// misfiring rewrites on categories not yet inspected. Coverage
/// expands as more rewrites move into the declarative form.
pub(crate) fn capco_category_has_values(m: &CapcoMarking, category: CategoryId) -> bool {
    let attrs = &m.0;
    match category {
        CAT_REL_TO => !attrs.rel_to.is_empty(),
        CAT_DISPLAY_ONLY_TO => !attrs.display_only_to.is_empty(),
        CAT_DISSEM => !attrs.dissem_us.is_empty() || !attrs.dissem_nato.is_empty(),
        CAT_NON_IC_DISSEM => !attrs.non_ic_dissem.is_empty(),
        CAT_SCI => !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty(),
        _ => true,
    }
}

/// `CategoryAction::Clear { category }` evaluator.
pub(crate) fn capco_category_clear(m: &mut CapcoMarking, category: CategoryId) {
    let attrs = &mut m.0;
    if category == CAT_REL_TO {
        attrs.rel_to = Box::new([]);
    } else if category == CAT_DISPLAY_ONLY_TO {
        // DISPLAY ONLY country-list axis. Parallel to `attrs.rel_to`
        // for symmetric clearing under
        // `capco/noforn-clears-display-only-to` (§H.8 p145 + §D.2
        // Table 3 rows 1-2).
        attrs.display_only_to = Box::new([]);
    } else if category == CAT_DISSEM {
        // clearing the dissem category zeroes both
        // namespaces. The CAT_DISSEM axis is namespace-agnostic from
        // the category-id perspective.
        attrs.dissem_us = Box::new([]);
        attrs.dissem_nato = Box::new([]);
    } else if category == CAT_NON_IC_DISSEM {
        attrs.non_ic_dissem = Box::new([]);
    }
    // Other categories: no-op.
}

/// `CategoryAction::Replace { category, with }` evaluator. The `with`
/// argument supplies a full marking; this copies only the named
/// category's storage out.
pub(crate) fn capco_category_replace(
    m: &mut CapcoMarking,
    category: CategoryId,
    with: &CapcoMarking,
) {
    let attrs = &mut m.0;
    if category == CAT_REL_TO {
        attrs.rel_to = with.0.rel_to.clone();
    } else if category == CAT_DISSEM {
        // replacing the dissem category copies both
        // namespaces from `with`. The two fields are independent
        // post-attribution per CAPCO-2016 p41 — replacing only one
        // would silently drop the other.
        attrs.dissem_us = with.0.dissem_us.clone();
        attrs.dissem_nato = with.0.dissem_nato.clone();
    } else if category == CAT_NON_IC_DISSEM {
        attrs.non_ic_dissem = with.0.non_ic_dissem.clone();
    }
}

/// Compute a per-page axis-presence bitmask for `m`.
///
/// Each bit `1u64 << cat.0` is set when the corresponding CAPCO category
/// has at least one value in `m`. The bitmask covers all twelve currently
/// declared `CAT_*` constants (IDs 1–12), so all bits fit comfortably in
/// a single `u64`.
///
/// Used by `project_attrs_pipeline` to build a per-page eligibility mask
/// that lets the rewrite loop skip rows whose trigger category is
/// definitively absent from the page, avoiding unnecessary predicate
/// evaluations on sparse pages (e.g. a pure-US page with only classification
/// and NOFORN skips every AEA / FGI / SAR / non-IC-dissem rewrite row).
///
/// The mask is an *over-approximation*: it may include categories that
/// become empty after a prior rewrite's `Clear` action, but it never
/// excludes categories that are actually present. Downstream callers that
/// maintain the mask monotonically (only ORing in write-axis bits, never
/// clearing) preserve this invariant throughout the rewrite loop.
pub(crate) fn capco_axis_mask(m: &CapcoMarking) -> u64 {
    let attrs = &m.0;
    let mut mask = 0u64;

    // CAT_CLASSIFICATION (1): any recognized classification system.
    if attrs.classification.is_some() {
        mask |= 1 << CAT_CLASSIFICATION.0;
    }

    // CAT_NON_US_CLASSIFICATION (2): FGI, NATO, or Conflict (which
    // carries a foreign component alongside the resolved US level).
    if matches!(
        attrs.classification,
        Some(MarkingClassification::Fgi(_))
            | Some(MarkingClassification::Nato(_))
            | Some(MarkingClassification::Conflict { .. })
    ) {
        mask |= 1 << CAT_NON_US_CLASSIFICATION.0;
    }

    // CAT_JOINT_CLASSIFICATION (3): JOINT co-owned classification.
    if matches!(attrs.classification, Some(MarkingClassification::Joint(_))) {
        mask |= 1 << CAT_JOINT_CLASSIFICATION.0;
    }

    // CAT_SCI (4): either CVE projection or structural markings.
    if !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty() {
        mask |= 1 << CAT_SCI.0;
    }

    // CAT_SAR (5): Special Access Required block.
    if attrs.sar_markings.is_some() {
        mask |= 1 << CAT_SAR.0;
    }

    // CAT_AEA (6): AEA markings (RD, FRD, CNWDI, SIGMA, UCNI, TFNI).
    if !attrs.aea_markings.is_empty() {
        mask |= 1 << CAT_AEA.0;
    }

    // CAT_FGI_MARKER (7): FGI marker in a US-classified marking.
    if attrs.fgi_marker.is_some() {
        mask |= 1 << CAT_FGI_MARKER.0;
    }

    // CAT_DISSEM (8): IC dissemination controls (US or NATO namespace).
    //
    // `display_only_to` is included here because
    // `capco_category_contains(CAT_DISSEM, TOK_DISPLAY_ONLY)` returns
    // `true` when `attrs.display_only_to` is non-empty — the canonical
    // parsed form `DISPLAY ONLY [LIST]` routes into `display_only_to`
    // rather than into a `DissemControl::Displayonly` entry in `dissem_us`.
    // Without this arm the eligibility gate in `project_attrs_pipeline`
    // would skip every `Contains(CAT_DISSEM, TOK_DISPLAY_ONLY)` trigger
    // (e.g. `capco/display-only-clears-relido`) on canonical DISPLAY ONLY
    // input where `dissem_us` and `dissem_nato` are both empty.
    if !attrs.dissem_us.is_empty()
        || !attrs.dissem_nato.is_empty()
        || !attrs.display_only_to.is_empty()
    {
        mask |= 1 << CAT_DISSEM.0;
    }

    // CAT_REL_TO (9): REL TO country / country-group codes.
    if !attrs.rel_to.is_empty() {
        mask |= 1 << CAT_REL_TO.0;
    }

    // CAT_DECLASSIFY_ON (10): declassification date from CAB.
    if attrs.declassify_on.is_some() {
        mask |= 1 << CAT_DECLASSIFY_ON.0;
    }

    // CAT_NON_IC_DISSEM (11): non-IC dissemination controls.
    if !attrs.non_ic_dissem.is_empty() {
        mask |= 1 << CAT_NON_IC_DISSEM.0;
    }

    // CAT_DISPLAY_ONLY_TO (12): DISPLAY ONLY country list.
    if !attrs.display_only_to.is_empty() {
        mask |= 1 << CAT_DISPLAY_ONLY_TO.0;
    }

    mask
}
