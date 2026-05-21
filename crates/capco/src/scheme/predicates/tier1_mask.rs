// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tier-1 `Constraint::Custom` predicates compiled to [`FactBitmask`]
//! trigger/suppressor mask tests.
//!
//! # Scope (PR-E / issue #371)
//!
//! The four named-dispatch rows that lift cleanly to pure-presence
//! bitmask logic per the §8 audit in
//! `docs/plans/2026-05-20-371-factbitmask-refactor.md`:
//!
//! | Rule | Trigger | Suppressor | Citation |
//! |------|---------|------------|----------|
//! | `E021/rd-frd-requires-noforn` | `AEA_RD ∪ AEA_FRD` | `NOFORN ∪ RELIDO ∪ REL_TO_PRESENT` | §H.6 p104 + p111 |
//! | `E024/rd-precedence` | `AEA_RD ∩ (AEA_FRD ∪ AEA_TFNI)` | n/a (precedence is structural) | §H.6 p104 |
//! | `E038/nodis-or-exdis-requires-noforn` | `NODIS ∪ EXDIS` | `NOFORN` | §H.9 p172 + p174 |
//! | `E070/frd-tfni-precedence` | `AEA_FRD ∩ AEA_TFNI` | n/a | §H.6 p120 |
//!
//! # Semantics vs the retired structural helpers
//!
//! Pre-PR-E, the same four predicates were structural slice walks in
//! `crates/capco/src/scheme/constraints/helpers.rs` (`e021_rd_frd_requires_noforn`,
//! `e024_rd_precedence`, `e038_dos_dissem_requires_noforn`,
//! `e070_frd_tfni_precedence`). PR-E retires those helpers and routes
//! the dispatch in `predicates/satisfies.rs` to the mask forms here.
//!
//! The mask form is observationally equivalent to the retired
//! structural form — same trigger/suppressor logic, same diagnostic
//! synthesis (message, citation, span, severity), same byte-identical
//! `ConstraintViolation` output. The proptest at
//! `crates/capco/tests/proptest_tier1_mask.rs` enforces parity against
//! an independent oracle re-derived from CAPCO-2016 §H.6 / §H.9
//! verbatim.
//!
//! # Amortized `derive_bits` dispatch (this PR)
//!
//! `marque_scheme::constraint::evaluate` calls `scheme.precompute_bits`
//! once per marking before the constraint loop, then threads the
//! resulting [`FactBitmask`] into every `scheme.evaluate_custom` call.
//! The four tier-1 functions here each receive that pre-computed `bits`
//! argument — `derive_bits` is paid exactly once per marking cycle
//! regardless of how many `Constraint::Custom` rows fire. The tier-2
//! (class-floor) and tier-3 (SCI per-system) catalogs share the same
//! `bits` argument via the same dispatch path.
//!
//! # Cheap pre-check before bitmask logic
//!
//! Each mask predicate runs an O(1) presence check on the dominant
//! input axis (typically `attrs.aea_markings.is_empty()` or
//! `attrs.non_ic_dissem.is_empty()`) before entering the bitmask logic.
//! On the overwhelmingly common no-trigger path this skips the
//! mask-evaluation work entirely. The structural body's pre-PR-E early
//! returns had a similar shape; the cost of this module is bounded
//! above by the structural cost the retired helpers were already
//! paying — corpus parity and `lint_latency` non-regression gate the
//! PR.

use marque_ism::canonical::CanonicalAttrs;
use marque_rules::{SectionLetter, Severity, capco};
use marque_scheme::{ConstraintViolation, TokenRef};

use crate::fact_bitmask::fact_bit;

use super::super::{CAT_AEA, TOK_EXDIS, TOK_NODIS};
use super::spans::token_span_attrs;

// ---------------------------------------------------------------------------
// Static trigger / suppressor masks
// ---------------------------------------------------------------------------

/// E021 trigger atoms — `AEA_RD ∪ AEA_FRD`.
///
/// Intentionally narrower than `AnyInCategory(CAT_AEA)`:
/// - **TFNI is excluded** per §H.6 p120 Relationship clause and Note 4
///   (TFNI sharing is contextual, not categorical).
/// - **UCNI variants are excluded** — neither DOE UCNI (§H.6 p116) nor
///   DoD UCNI (§H.6 p118) carries a NOFORN requirement.
const E021_TRIGGER_MASK: u128 = (1u128 << fact_bit::AEA_RD) | (1u128 << fact_bit::AEA_FRD);

/// E021 suppressors — `NOFORN ∪ RELIDO ∪ REL_TO_PRESENT`.
///
/// `NOFORN` makes the rule trivially satisfied. The §123/§144
/// sharing-agreement carve-out (CAPCO §H.6 p104) is documentary and
/// not detectable from byte form alone; the pragmatic substitute is
/// "any explicit FD&R decision on the portion" — `REL TO` with a
/// non-empty country list or `RELIDO` — which is evidence that the
/// author has chosen a release path under some sharing instrument.
const E021_SUPPRESSOR_MASK: u128 =
    (1u128 << fact_bit::NOFORN) | (1u128 << fact_bit::RELIDO) | (1u128 << fact_bit::REL_TO_PRESENT);

/// E024 superseded atoms — `AEA_FRD ∪ AEA_TFNI`. RD takes precedence
/// over either of these per §H.6 p104.
const E024_SUPERSEDED_MASK: u128 = (1u128 << fact_bit::AEA_FRD) | (1u128 << fact_bit::AEA_TFNI);

/// E038 trigger atoms — `NODIS ∪ EXDIS`.
const E038_TRIGGER_MASK: u128 = (1u128 << fact_bit::NODIS) | (1u128 << fact_bit::EXDIS);

// ---------------------------------------------------------------------------
// Mask-form predicates
// ---------------------------------------------------------------------------

/// E021 — RD or FRD requires NOFORN (unless a §123/§144 sharing
/// agreement applies, byte-approximated by FD&R-dominator presence).
///
/// CAPCO-2016 §H.6 p104 (RD) + p111 (FRD). Severity `Warn` — the
/// §123/§144 carve-out is documentary, not byte-observable, so a
/// hard `Error` would over-reach.
pub(crate) fn e021_rd_frd_requires_noforn(
    attrs: &CanonicalAttrs,
    bits: marque_scheme::FactBitmask,
) -> Vec<ConstraintViolation> {
    // Cheap pre-check on the dominant axis — AEA atoms are an open-vocab
    // closed slice on `CanonicalAttrs`; if it's empty the trigger mask
    // cannot fire.
    if attrs.aea_markings.is_empty() {
        return Vec::new();
    }
    let bits = bits.bits();
    if (bits & E021_TRIGGER_MASK) == 0 || (bits & E021_SUPPRESSOR_MASK) != 0 {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E021/rd-frd-requires-noforn",
        message: "RD/FRD typically requires NOFORN unless a §123/§144 \
                  sharing agreement has been established under the \
                  Atomic Energy Act"
            .to_owned(),
        citation: capco(SectionLetter::H, 6, 104),
        span: token_span_attrs(attrs, &TokenRef::AnyInCategory(CAT_AEA)),
        severity: Some(Severity::Warn),
    }]
}

/// E024 — RD takes precedence over FRD/TFNI. CAPCO-2016 §H.6 p104.
///
/// Helper emits ONE `ConstraintViolation`; the wrapper rule in
/// `crates/capco/src/rules.rs` enumerates per-offending-marking to
/// produce byte-precise spans.
pub(crate) fn e024_rd_precedence(
    attrs: &CanonicalAttrs,
    bits: marque_scheme::FactBitmask,
) -> Vec<ConstraintViolation> {
    if attrs.aea_markings.is_empty() {
        return Vec::new();
    }
    let bits = bits.bits();
    let has_rd = (bits & (1u128 << fact_bit::AEA_RD)) != 0;
    let has_superseded = (bits & E024_SUPERSEDED_MASK) != 0;
    if !has_rd || !has_superseded {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E024/rd-precedence",
        message: "RD takes precedence over FRD/TFNI; FRD/TFNI should not appear alongside RD"
            .to_owned(),
        citation: capco(SectionLetter::H, 6, 104),
        span: token_span_attrs(attrs, &TokenRef::AnyInCategory(CAT_AEA)),
        severity: Some(Severity::Fix),
    }]
}

/// E038 — NODIS / EXDIS require NOFORN. CAPCO-2016 §H.9 p172 + p174.
///
/// The structural form (retired) checked NOFORN via
/// [`satisfies_attrs`] with `TokenRef::Token(TOK_NOFORN)`. The
/// `NOFORN` bit in [`derive_bits`] reads from `attrs.dissem_iter()`
/// — the same set the structural call resolves over (post PR 9b /
/// FR-046, `dissem_iter()` walks both `dissem_us` and `dissem_nato`).
/// Behavior matches byte-for-byte.
pub(crate) fn e038_dos_dissem_requires_noforn(
    attrs: &CanonicalAttrs,
    bits: marque_scheme::FactBitmask,
) -> Vec<ConstraintViolation> {
    use marque_ism::NonIcDissem;

    if attrs.non_ic_dissem.is_empty() {
        return Vec::new();
    }
    let bits = bits.bits();
    if (bits & E038_TRIGGER_MASK) == 0 || (bits & (1u128 << fact_bit::NOFORN)) != 0 {
        return Vec::new();
    }

    // Surface the actual triggering token (NODIS or EXDIS) for the
    // span anchor — preserves the retired structural helper's
    // diagnostic shape so existing audit-stream consumers see
    // byte-identical output.
    let trigger_token = attrs
        .non_ic_dissem
        .iter()
        .find_map(|d| match d {
            NonIcDissem::Nodis => Some(TOK_NODIS),
            NonIcDissem::Exdis => Some(TOK_EXDIS),
            _ => None,
        })
        .unwrap_or(TOK_NODIS);

    vec![ConstraintViolation {
        constraint_label: "E038/nodis-or-exdis-requires-noforn",
        message: "NODIS and EXDIS may be used only with NOFORN information".to_owned(),
        citation: capco(SectionLetter::H, 9, 172),
        span: token_span_attrs(attrs, &TokenRef::Token(trigger_token)),
        severity: Some(Severity::Error),
    }]
}

/// E070 — FRD takes precedence over TFNI. CAPCO-2016 §H.6 p120.
///
/// Mirror of [`e024_rd_precedence`] for the FRD-side leg per #559
/// close-out (PM decision 2026-05-19). Returns
/// `span: None, severity: None` to match the dyadic-helper shape —
/// end-user-visible diagnostic emission lands in the broader
/// engine-bridge generalization tracked at issue #578.
pub(crate) fn e070_frd_tfni_precedence(
    attrs: &CanonicalAttrs,
    bits: marque_scheme::FactBitmask,
) -> Vec<ConstraintViolation> {
    if attrs.aea_markings.is_empty() {
        return Vec::new();
    }
    let bits = bits.bits();
    let need_mask: u128 = (1u128 << fact_bit::AEA_FRD) | (1u128 << fact_bit::AEA_TFNI);
    if (bits & need_mask) != need_mask {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E070/frd-tfni-precedence",
        message: "FRD takes precedence over TFNI; TFNI should not appear alongside FRD".to_owned(),
        citation: capco(SectionLetter::H, 6, 120),
        span: None,
        severity: None,
    }]
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact_bitmask::derive_bits;
    use marque_ism::{
        AeaMarking, Classification, DissemControl, MarkingClassification, NonIcDissem,
        canonical::CanonicalAttrs,
    };

    fn classified_us_secret() -> CanonicalAttrs {
        let mut attrs = CanonicalAttrs::default();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        attrs
    }

    // --- E021 ---

    #[test]
    fn e021_no_aea_no_fire() {
        let attrs = classified_us_secret();
        assert!(e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e021_rd_no_noforn_fires() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Rd(Default::default())].into_boxed_slice();
        let out = e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].constraint_label, "E021/rd-frd-requires-noforn");
    }

    #[test]
    fn e021_rd_with_noforn_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Rd(Default::default())].into_boxed_slice();
        attrs.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
        assert!(e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e021_rd_with_relido_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Rd(Default::default())].into_boxed_slice();
        attrs.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
        assert!(e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e021_frd_no_noforn_fires() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Frd(Default::default())].into_boxed_slice();
        assert_eq!(
            e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs)).len(),
            1
        );
    }

    #[test]
    fn e021_tfni_alone_does_not_fire() {
        // §H.6 p120 — TFNI is intentionally excluded from E021.
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Tfni].into_boxed_slice();
        assert!(e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e021_ucni_alone_does_not_fire() {
        // §H.6 p116 (DOE UCNI) + p118 (DoD UCNI) — neither variant
        // carries a NOFORN requirement, so they are excluded from the
        // E021 trigger mask. Without a corresponding regression test,
        // a future bit-layout shuffle could silently capture UCNI into
        // the trigger.
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::DoeUcni].into_boxed_slice();
        assert!(e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());

        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
        assert!(e021_rd_frd_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    // --- E024 ---

    #[test]
    fn e024_rd_alone_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Rd(Default::default())].into_boxed_slice();
        assert!(e024_rd_precedence(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e024_rd_with_frd_fires() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![
            AeaMarking::Rd(Default::default()),
            AeaMarking::Frd(Default::default()),
        ]
        .into_boxed_slice();
        let out = e024_rd_precedence(&attrs, derive_bits(&attrs));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].constraint_label, "E024/rd-precedence");
    }

    #[test]
    fn e024_rd_with_tfni_fires() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings =
            vec![AeaMarking::Rd(Default::default()), AeaMarking::Tfni].into_boxed_slice();
        assert_eq!(e024_rd_precedence(&attrs, derive_bits(&attrs)).len(), 1);
    }

    #[test]
    fn e024_frd_tfni_no_rd_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings =
            vec![AeaMarking::Frd(Default::default()), AeaMarking::Tfni].into_boxed_slice();
        assert!(e024_rd_precedence(&attrs, derive_bits(&attrs)).is_empty());
    }

    // --- E038 ---

    #[test]
    fn e038_no_non_ic_dissem_no_fire() {
        let attrs = classified_us_secret();
        assert!(e038_dos_dissem_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e038_nodis_no_noforn_fires() {
        let mut attrs = classified_us_secret();
        attrs.non_ic_dissem = vec![NonIcDissem::Nodis].into_boxed_slice();
        let out = e038_dos_dissem_requires_noforn(&attrs, derive_bits(&attrs));
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].constraint_label,
            "E038/nodis-or-exdis-requires-noforn"
        );
    }

    #[test]
    fn e038_exdis_no_noforn_fires() {
        let mut attrs = classified_us_secret();
        attrs.non_ic_dissem = vec![NonIcDissem::Exdis].into_boxed_slice();
        assert_eq!(
            e038_dos_dissem_requires_noforn(&attrs, derive_bits(&attrs)).len(),
            1
        );
    }

    #[test]
    fn e038_nodis_with_noforn_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.non_ic_dissem = vec![NonIcDissem::Nodis].into_boxed_slice();
        attrs.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
        assert!(e038_dos_dissem_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e038_unrelated_non_ic_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.non_ic_dissem = vec![NonIcDissem::Limdis].into_boxed_slice();
        assert!(e038_dos_dissem_requires_noforn(&attrs, derive_bits(&attrs)).is_empty());
    }

    // --- E070 ---

    #[test]
    fn e070_no_aea_no_fire() {
        let attrs = classified_us_secret();
        assert!(e070_frd_tfni_precedence(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e070_frd_alone_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Frd(Default::default())].into_boxed_slice();
        assert!(e070_frd_tfni_precedence(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e070_tfni_alone_no_fire() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings = vec![AeaMarking::Tfni].into_boxed_slice();
        assert!(e070_frd_tfni_precedence(&attrs, derive_bits(&attrs)).is_empty());
    }

    #[test]
    fn e070_frd_with_tfni_fires() {
        let mut attrs = classified_us_secret();
        attrs.aea_markings =
            vec![AeaMarking::Frd(Default::default()), AeaMarking::Tfni].into_boxed_slice();
        let out = e070_frd_tfni_precedence(&attrs, derive_bits(&attrs));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].constraint_label, "E070/frd-tfni-precedence");
        assert!(out[0].span.is_none());
        assert!(out[0].severity.is_none());
    }
}
