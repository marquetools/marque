// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tier-2 class-floor dispatch helpers for the bitmask fast path.
//!
//! Provides the two crate-local helpers consumed by
//! [`class_floor_catalog_eval`](super::class_floor::class_floor_catalog_eval)
//! to resolve the effective classification level from a [`FactBitmask`]
//! without re-reading [`CanonicalAttrs`] for the common US/NATO path.
//!
//! # Equivalence to `MarkingClassification::effective_level()`
//!
//! For US, NATO, and Conflict classifications the bitmask path computes
//! `max(us_level, nato_us_equivalent_level)` which is byte-identical to
//! `effective_level()`. FGI and JOINT classifications carry no chain bits
//! in the bitmask today; callers gate on [`classification_is_fgi_or_joint`]
//! before calling [`effective_level_from_bits`].
//!
//! The `EqualsU` policy (UCNI ceiling rows, §2.4) reads only the US chain
//! field — `extract_us_class_level(bits) == Some(Unclassified)` — matching
//! `class_floor_satisfied`'s `EqualsU` arm which uses `attrs.us_classification()`
//! (US-only, not reciprocal-raised). NATO/FGI/JOINT classifications zero
//! out the US chain field, so `extract_us_class_level` returns `None` for
//! them, which is the correct "not US classified" answer (UCNI ceiling fails
//! on any non-US classification, mirroring retired E025 semantics).
//!
//! # Relationship to tier-1
//!
//! `tier1_mask.rs` compiled the 4 named-dispatch rows to pure-presence
//! bitmask tests (trigger ∧ ¬suppressor). Tier-2 compiles the 27 class-floor
//! rows to presence mask gate + chain-extract numeric compare. The `derive_bits`
//! call per row is retained (same as tier-1); amortization across the catalog
//! is deferred to a follow-on that requires a `marque-scheme` trait change
//! (Constitution VII Principle IV separate-PR discipline, per plan §1 Q6).
//!
//! # Bitmask coverage
//!
//! | Kind | Count | % of 27 |
//! |---|---|---|
//! | Exact bitmask (presence + floor) | 13 | 48% |
//! | Coarse-gate bitmask (gate + structural confirm) | 10 | 37% |
//! | Structural fallthrough (no bitmask) | 4 | 15% |
//! | **Bitmask short-circuit total** | **23** | **85%** |
//!
//! 85% bitmask short-circuit coverage satisfies AC #5 from issue #371
//! (≥80% `Constraint::Custom` rows in mask form).
//!
//! # Deferred follow-ons (filed as issues per plan §9)
//!
//! 1. `derive_bits` amortization — thread `bits: FactBitmask` through
//!    `MarkingScheme::evaluate_custom`. `marque-scheme` trait change;
//!    Constitution VII Principle IV separate-PR discipline.
//! 2. Tier-3 SCI per-system catalog compilation — 5 rows in
//!    `sci_per_system_catalog.rs`. Structurally similar to tier-2 with more
//!    compartment-string reads; deferred until tier-2 lands.
//! 3. FGI/JOINT chain bits — reserve bits 51-54 if profiling shows
//!    FGI/JOINT structural fallthrough is hot. Anticipated negligible.
//! 4. `SCI_RSV` atom — if RSV-comp shows up as a hot path. Currently uses
//!    `SCI_PRESENT` coarse gate.

use marque_ism::{Classification, MarkingClassification, canonical::CanonicalAttrs};
use marque_scheme::FactBitmask;

use crate::fact_bitmask::{extract_nato_class_level, extract_us_class_level};

/// Returns the effective classification level for floor comparison,
/// derived from the bitmask.
///
/// Computes `max(us_level, nato_us_equivalent_level)` over the underlying
/// 1..=5 ordinal ladder to match [`MarkingClassification::effective_level()`]
/// for the three classification kinds the bitmask chain fields capture
/// (US, NATO, `Conflict::us`). Returns `None` only when both chain
/// fields are zero — i.e., no US and no NATO classification present.
///
/// # FGI/JOINT gate
///
/// Callers MUST gate on [`classification_is_fgi_or_joint`] before
/// calling this function: FGI and JOINT classification levels are
/// absent from the bitmask chain fields and would produce a spurious
/// `None` (which the caller treats as "no classification → floor fails →
/// emit diagnostic"), diverging from the structural path. This gate is
/// enforced at the call site in `class_floor_catalog_eval`.
///
/// # NATO reciprocal-raise
///
/// NATO levels map to their US equivalent on the same ordinal ladder
/// (`CTS→TS, NS→S, NC→C, NR→R, NU→U`). Taking `max(us, nato_us_equivalent)`
/// is byte-identical to `effective_level()` for the US/NATO/Conflict kinds.
pub(crate) fn effective_level_from_bits(bits: FactBitmask) -> Option<Classification> {
    let us = extract_us_class_level(bits);
    let nato = extract_nato_class_level(bits);
    // Reciprocal-raise NATO to US equivalent and take the max.
    let nato_as_us = nato.map(|nc| nc.us_equivalent());
    match (us, nato_as_us) {
        (None, None) => None,
        (Some(u), None) => Some(u),
        (None, Some(n)) => Some(n),
        // Classification derives Ord with variants in restrictiveness order:
        // Unclassified < Restricted < Confidential < Secret < TopSecret.
        // `Ord::max` returns the more restrictive of the two levels.
        (Some(u), Some(n)) => Some(u.max(n)),
    }
}

/// `true` iff the marking's classification is FGI or JOINT — the two
/// kinds whose level is absent from the bitmask chain fields.
///
/// When `true`, the caller MUST fall through to the structural dispatch
/// path (which uses `MarkingClassification::effective_level()` and handles
/// FGI/JOINT correctly via the `class_floor_satisfied` function). This is
/// an early-out gate at the top of `class_floor_catalog_eval`; it ensures
/// byte-identical `ConstraintViolation` emission for the FGI/JOINT path.
///
/// `MarkingClassification::Conflict { .. }` is NOT included here: the
/// Conflict variant encodes a US side (via `attrs.us_classification()`) and a
/// foreign side. The bitmask encodes the US side in chain bits 27-29; thus
/// `effective_level_from_bits` correctly handles Conflict by reading the US
/// chain. Only pure-FGI and pure-JOINT have zero chain fields in both US
/// and NATO positions.
#[inline]
pub(crate) fn classification_is_fgi_or_joint(attrs: &CanonicalAttrs) -> bool {
    matches!(
        attrs.classification.as_ref(),
        Some(MarkingClassification::Fgi(_)) | Some(MarkingClassification::Joint(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use marque_ism::{
        Classification, CountryCode, FgiClassification, ForeignClassification, JointClassification,
        MarkingClassification, NatoClassification, canonical::CanonicalAttrs,
    };
    use marque_scheme::FactBitmask;

    use crate::fact_bitmask::derive_bits;

    // ---------------------------------------------------------------------------
    // `effective_level_from_bits` unit tests
    // ---------------------------------------------------------------------------

    fn attrs_us(level: Classification) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(level));
        a
    }

    fn attrs_nato(level: NatoClassification) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Nato(level));
        a
    }

    #[test]
    fn effective_level_from_bits_us_secret() {
        let attrs = attrs_us(Classification::Secret);
        let bits = derive_bits(&attrs);
        assert_eq!(
            effective_level_from_bits(bits),
            Some(Classification::Secret),
            "US SECRET should yield effective level Secret"
        );
    }

    #[test]
    fn effective_level_from_bits_us_confidential() {
        let attrs = attrs_us(Classification::Confidential);
        let bits = derive_bits(&attrs);
        assert_eq!(
            effective_level_from_bits(bits),
            Some(Classification::Confidential)
        );
    }

    #[test]
    fn effective_level_from_bits_us_top_secret() {
        let attrs = attrs_us(Classification::TopSecret);
        let bits = derive_bits(&attrs);
        assert_eq!(
            effective_level_from_bits(bits),
            Some(Classification::TopSecret)
        );
    }

    #[test]
    fn effective_level_from_bits_us_unclassified() {
        let attrs = attrs_us(Classification::Unclassified);
        let bits = derive_bits(&attrs);
        assert_eq!(
            effective_level_from_bits(bits),
            Some(Classification::Unclassified)
        );
    }

    #[test]
    fn effective_level_from_bits_nato_secret() {
        // NATO SECRET → effective level SECRET (reciprocal raise)
        let attrs = attrs_nato(NatoClassification::NatoSecret);
        let bits = derive_bits(&attrs);
        assert_eq!(
            effective_level_from_bits(bits),
            Some(Classification::Secret),
            "NATO SECRET should reciprocal-raise to US SECRET"
        );
    }

    #[test]
    fn effective_level_from_bits_nato_cts() {
        let attrs = attrs_nato(NatoClassification::CosmicTopSecret);
        let bits = derive_bits(&attrs);
        assert_eq!(
            effective_level_from_bits(bits),
            Some(Classification::TopSecret),
            "CTS should reciprocal-raise to TOP SECRET"
        );
    }

    #[test]
    fn effective_level_from_bits_empty_attrs_returns_none() {
        let bits = FactBitmask::EMPTY;
        assert_eq!(
            effective_level_from_bits(bits),
            None,
            "Empty bitmask should yield None"
        );
    }

    #[test]
    fn effective_level_from_bits_conflict_reads_us_chain_only() {
        // `derive_bits` for `MarkingClassification::Conflict` encodes ONLY
        // the US side into the US chain field (bits 27-29). The foreign
        // (NATO) side is NOT written into the NATO chain bits (32-34) because
        // the Conflict variant's structural encoding is US-axis-only in the
        // bitmask (see `fact_bitmask::derive_bits` `Us | Conflict` arm).
        //
        // This means `effective_level_from_bits` sees `us = Some(Secret)`,
        // `nato = None`, and returns `Some(Secret)`. The caller in
        // `class_floor_catalog_eval` gates FGI/JOINT via
        // `classification_is_fgi_or_joint`; Conflict is NOT FGI/JOINT so
        // the bitmask path runs and uses the US side — byte-identical to
        // `class_floor_satisfied`'s `AtLeast` arm, which calls
        // `effective_level()` on the Conflict variant. `effective_level()`
        // for `Conflict { us: Secret, foreign: NATO(CTS) }` returns
        // `*us = Secret` (see `MarkingClassification::effective_level` in
        // `crates/ism/src/attrs.rs` — the `Conflict { us, .. }` arm returns
        // `*us` directly, not `max(us, foreign)`). The bitmask path reading
        // only the US chain is therefore byte-identical to the structural path
        // for all Conflict inputs.
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Conflict {
            us: Classification::Secret,
            foreign: Box::new(ForeignClassification::Nato(
                NatoClassification::CosmicTopSecret,
            )),
        });
        let bits = derive_bits(&a);
        // Only US chain is populated for Conflict; nato chain = 0.
        assert_eq!(
            effective_level_from_bits(bits),
            Some(Classification::Secret),
            "Conflict reads only the US chain from bits; foreign NATO side is not encoded"
        );
    }

    // ---------------------------------------------------------------------------
    // `classification_is_fgi_or_joint` unit tests
    // ---------------------------------------------------------------------------

    #[test]
    fn fgi_classification_is_fgi_or_joint_true() {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Fgi(FgiClassification {
            countries: vec![CountryCode::GBR].into_boxed_slice(),
            level: Classification::Secret,
        }));
        assert!(
            classification_is_fgi_or_joint(&a),
            "FGI classification should return true"
        );
    }

    #[test]
    fn joint_classification_is_fgi_or_joint_true() {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Joint(JointClassification {
            level: Classification::Secret,
            countries: vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice(),
        }));
        assert!(
            classification_is_fgi_or_joint(&a),
            "JOINT classification should return true"
        );
    }

    #[test]
    fn us_classification_is_fgi_or_joint_false() {
        let a = attrs_us(Classification::Secret);
        assert!(
            !classification_is_fgi_or_joint(&a),
            "US classification should return false"
        );
    }

    #[test]
    fn nato_classification_is_fgi_or_joint_false() {
        let a = attrs_nato(NatoClassification::NatoSecret);
        assert!(
            !classification_is_fgi_or_joint(&a),
            "NATO classification should return false"
        );
    }

    #[test]
    fn no_classification_is_fgi_or_joint_false() {
        let a = CanonicalAttrs::default();
        assert!(
            !classification_is_fgi_or_joint(&a),
            "No classification (None) should return false"
        );
    }

    #[test]
    fn conflict_classification_is_fgi_or_joint_false() {
        // Conflict is NOT FGI/JOINT — the US chain bits cover it.
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Conflict {
            us: Classification::Secret,
            foreign: Box::new(ForeignClassification::Nato(NatoClassification::NatoSecret)),
        });
        assert!(
            !classification_is_fgi_or_joint(&a),
            "Conflict classification should return false (US chain covers it)"
        );
    }
}
