// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CategoryPredicate::Contains` / `CategoryPredicate::Empty` and
//! `CategoryAction::Clear` / `CategoryAction::Replace` dispatch
//! against `CapcoMarking`. Lifted from the monolithic `actions.rs`
//! per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

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
/// Phase B supports the sample constraint set. Unhandled `(category,
/// token)` pairs return `false` — a safe conservative answer that
/// effectively disables the rewrite rather than silently misfiring.
/// Phase C expands coverage as more rewrites move to the declarative
/// form.
///
/// PR 3c.B Sub-PR 8.F adds `CAT_NON_IC_DISSEM` arms for `TOK_NODIS` and
/// `TOK_EXDIS` so the `capco/nodis-implies-noforn` and
/// `capco/exdis-implies-noforn` PageRewrites' `Contains` triggers can
/// resolve against the `non_ic_dissem` axis. Without this extension the
/// new rewrites would silently never fire (the conservative-`false`
/// fallthrough effectively disables them), making 8.F a no-op
/// masquerading as a fix (design spec §3 "Predicate-evaluator support",
/// Q2 "capco_category_contains silent-disabling root-cause").
///
/// PR 3c.B Sub-PR 8.F.2 extends the same `CAT_NON_IC_DISSEM` block with
/// arms for `TOK_SBU_NF` and `TOK_LES_NF`, scanning the
/// `NonIcDissem::SbuNf` / `NonIcDissem::LesNf` variants. Same shape,
/// same silent-disabling concern — the `capco/sbu-nf-implies-noforn`
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
        // PR 9b (T132): "Contains NOFORN" is namespace-agnostic — the
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
    // PR 3c.B Sub-PR 8.F — CAT_NON_IC_DISSEM arms for NODIS and EXDIS.
    // These enable the `capco/nodis-implies-noforn` and
    // `capco/exdis-implies-noforn` PageRewrite triggers to resolve.
    //
    // PR 3c.B Sub-PR 8.F.2 — CAT_NON_IC_DISSEM arms for SBU-NF and
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
/// misfiring rewrites on categories Phase B doesn't yet inspect.
/// Phase C expands the match arms as more rewrites move into the
/// declarative form.
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
        // PR 4b-D.2 Copilot R1 #2: DISPLAY ONLY country-list axis.
        // Parallel to `attrs.rel_to` for symmetric clearing under
        // `capco/noforn-clears-display-only-to` (§H.8 p145 + §D.2
        // Table 3 rows 1-2).
        attrs.display_only_to = Box::new([]);
    } else if category == CAT_DISSEM {
        // PR 9b (T132): clearing the dissem category zeroes both
        // namespaces. The CAT_DISSEM axis is namespace-agnostic from
        // the category-id perspective.
        attrs.dissem_us = Box::new([]);
        attrs.dissem_nato = Box::new([]);
    } else if category == CAT_NON_IC_DISSEM {
        attrs.non_ic_dissem = Box::new([]);
    }
    // Other categories: no-op. Phase C expands coverage.
}

/// `CategoryAction::Replace { category, with }` evaluator. The `with`
/// argument supplies a full marking; Phase B copies only the named
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
        // PR 9b (T132): replacing the dissem category copies both
        // namespaces from `with`. The two fields are independent
        // post-attribution per CAPCO-2016 p41 — replacing only one
        // would silently drop the other.
        attrs.dissem_us = with.0.dissem_us.clone();
        attrs.dissem_nato = with.0.dissem_nato.clone();
    } else if category == CAT_NON_IC_DISSEM {
        attrs.non_ic_dissem = with.0.non_ic_dissem.clone();
    }
}
