// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pattern-C strip-action bodies (`strip_dod_ucni_action`,
//! `strip_doe_ucni_action`) and the [`noop_action`] used by Phase-3
//! stub `PageRewrite` rows. Lifted from the monolithic `actions.rs`
//! per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use super::super::*;

/// No-op [`CategoryAction::Custom`] body for Phase-3 stub
/// `PageRewrite` rows whose action would otherwise need a multi-axis
/// or within-axis transform that the Phase-3 declarative surface
/// can't express cleanly (e.g., the §3.4.1 transmutations).
///
/// Runtime page-rewrite dispatch stays in [`PageContext`] until
/// Phase D / Phase E lands real rewrite bodies; until then the
/// action body is a no-op and only the row's `reads` / `writes`
/// axis annotations are consumed (by the engine's topological
/// scheduler, T031–T032). Pairs with [`never_fires`] for triggers.
pub(crate) fn noop_action(_marking: &mut CapcoMarking) {}

/// Pattern-C action body: strip every `AeaMarking::DodUcni` from the
/// AEA axis. Pairs with [`dod_ucni_classified_trigger`].
pub(crate) fn strip_dod_ucni_action(m: &mut CapcoMarking) {
    let attrs = &mut m.0;
    let kept: Vec<marque_ism::AeaMarking> = attrs
        .aea_markings
        .iter()
        .filter(|a| !matches!(a, marque_ism::AeaMarking::DodUcni))
        .cloned()
        .collect();
    attrs.aea_markings = kept.into_boxed_slice();
}

/// Pattern-C action body: strip every `AeaMarking::DoeUcni` from the
/// AEA axis. Pairs with [`doe_ucni_classified_trigger`].
pub(crate) fn strip_doe_ucni_action(m: &mut CapcoMarking) {
    let attrs = &mut m.0;
    let kept: Vec<marque_ism::AeaMarking> = attrs
        .aea_markings
        .iter()
        .filter(|a| !matches!(a, marque_ism::AeaMarking::DoeUcni))
        .cloned()
        .collect();
    attrs.aea_markings = kept.into_boxed_slice();
}
