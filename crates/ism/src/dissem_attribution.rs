// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! IC dissem attribution — the post-parse pass that splits a parser's
//! single dissem-token stream into [`ParsedAttrs::dissem_us`] vs.
//! [`ParsedAttrs::dissem_nato`] per the CAPCO-2016 §G.2 Table 5
//! (pp 40-45) NATO-dissem ARH rule.
//!
//! # The rule
//!
//! CAPCO-2016 §G.2 Table 5 (pp 40-45) enumerates two NATO dissemination
//! control markings — "ORCON (NATO dissemination control marking)" and
//! "RELEASEABLE TO or [LIST] ONLY" — and directs the Access Rights and
//! Handling (ARH) for both to "See US X ARH requirements." No
//! NATO-specific dissem form (e.g., `ORCON-NATO`) exists in the
//! Register.
//!
//! Operational consequence: when OC or REL TO appears in a US-classified
//! marking, the resolved namespace is US — the NATO-origin form shares
//! the US ARH machinery and the token is US-attributed. The NATO
//! namespace populates only when no US classification axis is present
//! (pure-NATO portions like `(//CTS//OC/REL TO USA, NATO)`).
//!
//! Tokens that are NATO-only by spec (ATOMAL, BALK, BOHEMIA) are NOT
//! dissems and route to the AEA / SCI axes per FR-047 — they never
//! pass through this attribution code path.
//!
//! # The decision
//!
//! For each portion the parser emits, `attribute_dissems` inspects
//! [`ParsedAttrs::classification`] and partitions the parser's
//! incoming dissem stream into the two output slices:
//!
//! - [`MarkingClassification::Us`] **or** [`MarkingClassification::Conflict`]
//!   (carries a US axis) → all dissems land in `dissem_us`.
//! - [`MarkingClassification::Nato`] (no US axis) → all dissems land
//!   in `dissem_nato`.
//! - [`MarkingClassification::Fgi`] or [`MarkingClassification::Joint`]
//!   → all dissems land in `dissem_us` (Fgi falls through to the
//!   default; Joint has US as a co-owner so reciprocity attaches to
//!   US).
//! - [`Option::None`] (no classification axis at all) → use the
//!   caller-supplied [`DefaultOrigin`]. CAPCO passes
//!   [`DefaultOrigin::Us`].
//!
//! # Why a free function and not `Parser`-internal
//!
//! `attribute_dissems` operates only on [`ParsedAttrs`] and
//! [`MarkingClassification`], both of which live in this crate. The
//! parser in `marque-core` calls it as the last step before returning
//! `ParsedAttrs`, and the `marque-ism::from_parsed_unchecked` adapter
//! relies on the fields already being split (it is a pure structural
//! rename and MUST NOT contain attribution logic).
//!
//! Hosting `attribute_dissems` in `marque-ism` is what keeps the
//! one-directional dependency graph (`marque-ism ← marque-core`)
//! intact: `marque-ism` can be the single source of truth for the
//! attribution rule without depending on `marque-core`.

use crate::attrs::MarkingClassification;
use crate::parsed::{ParsedAttrs, ParsedDissem};

/// Default attribution to use when [`ParsedAttrs::classification`] is
/// [`Option::None`].
///
/// CAPCO consumers pass [`DefaultOrigin::Us`]. Future schemes targeting
/// a foreign-origin-dominant context could pass
/// [`DefaultOrigin::Nato`] to flip the no-context fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DefaultOrigin {
    /// No-context fallback attributes dissems to US.
    #[default]
    Us,
    /// No-context fallback attributes dissems to NATO.
    Nato,
}

/// Partition `attrs.dissem_us + attrs.dissem_nato` into the two output
/// slices per CAPCO-2016 §G.2 Table 5 (pp 40-45): NATO's two dissem
/// markings (ORCON, REL TO / [LIST] ONLY) inherit US ARH, so any
/// US-classified marking carrying those tokens routes them to
/// `dissem_us`. The NATO namespace populates only on pure-NATO portions.
///
/// **Idempotent.** Reads both incoming fields, merges them, and writes
/// the partitioned result back. Calling twice with the same `default`
/// yields the same output as calling once.
///
/// **`default`** is consulted only when `attrs.classification` is
/// [`Option::None`]. CAPCO callers should pass [`DefaultOrigin::Us`]
/// (also accessible as the
/// [`ParsedAttrs::DEFAULT_ORIGIN_CAPCO`](crate::parsed::ParsedAttrs::DEFAULT_ORIGIN_CAPCO)
/// constant).
///
/// # Invariant
///
/// On return:
/// - If `attrs.classification.is_some()` and has a US axis (`Us` or
///   `Conflict`) → `attrs.dissem_nato.is_empty()`.
/// - If `attrs.classification == Some(Nato(_))` →
///   `attrs.dissem_us.is_empty()`.
/// - If `attrs.classification.is_none()` and `default == Us` →
///   `attrs.dissem_nato.is_empty()`.
/// - Otherwise (no classification + `default == Nato`) →
///   `attrs.dissem_us.is_empty()`.
pub fn attribute_dissems<'src>(attrs: &mut ParsedAttrs<'src>, default: DefaultOrigin) {
    // Merge both fields back into one stream so the function is fully
    // idempotent under successive calls. The parser writes the
    // pre-attribution tokens into `dissem_us` by default; a subsequent
    // call after attribution has already split must still see the
    // same final shape.
    let merged: Vec<ParsedDissem<'src>> = {
        let us = std::mem::take(&mut attrs.dissem_us).into_vec();
        let nato = std::mem::take(&mut attrs.dissem_nato).into_vec();
        let mut v = Vec::with_capacity(us.len() + nato.len());
        v.extend(us);
        v.extend(nato);
        v
    };

    let target = match (&attrs.classification, default) {
        // Has classification: dispatch on the variant.
        (Some(c), _) => match c.value {
            // Any US axis (pure US or US+foreign conflict) → US.
            MarkingClassification::Us(_) | MarkingClassification::Conflict { .. } => {
                DefaultOrigin::Us
            }
            // Pure NATO (no US axis) → NATO.
            MarkingClassification::Nato(_) => DefaultOrigin::Nato,
            // FGI and JOINT route to US: JOINT has US as co-owner per
            // §H.3, and FGI portions do not exercise the NATO dissem
            // reciprocity (NATO's two dissems are ORCON / REL TO, both
            // tracked at the US axis when commingled with FGI content).
            MarkingClassification::Fgi(_) | MarkingClassification::Joint(_) => DefaultOrigin::Us,
        },
        // No classification axis: fall back to the caller's default.
        (None, d) => d,
    };

    match target {
        DefaultOrigin::Us => attrs.dissem_us = merged.into_boxed_slice(),
        DefaultOrigin::Nato => attrs.dissem_nato = merged.into_boxed_slice(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{
        Classification, DissemControl, FgiClassification, JointClassification, NatoClassification,
    };
    use crate::parsed::{ParsedClassification, ParsedDissem, SourceOrigin};
    use crate::span::Span;

    fn nf<'a>(s: &'a str) -> ParsedDissem<'a> {
        ParsedDissem::new(DissemControl::Nf, s, Span::new(0, s.len()))
    }

    fn empty_attrs<'src>(
        classification: Option<ParsedClassification<'src>>,
        dissems: Vec<ParsedDissem<'src>>,
    ) -> ParsedAttrs<'src> {
        ParsedAttrs::new(
            classification,
            Box::new([]),
            Box::new([]),
            None,
            Box::new([]),
            None,
            dissems.into_boxed_slice(),
            Box::new([]),
            Box::new([]),
            Box::new([]),
            Box::new([]), // display_only_to
            None,
            None,
            None,
            None,
            Box::new([]),
            SourceOrigin::Portion,
        )
    }

    #[test]
    fn us_classification_routes_to_dissem_us() {
        let s = "NF";
        let cls = ParsedClassification::new(
            MarkingClassification::Us(Classification::Secret),
            "S",
            Span::new(0, 1),
        );
        let mut attrs = empty_attrs(Some(cls), vec![nf(s)]);
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert_eq!(attrs.dissem_us.len(), 1);
        assert!(attrs.dissem_nato.is_empty());
    }

    #[test]
    fn nato_classification_routes_to_dissem_nato() {
        let s = "OC";
        let cls = ParsedClassification::new(
            MarkingClassification::Nato(NatoClassification::CosmicTopSecret),
            "CTS",
            Span::new(0, 3),
        );
        let oc = ParsedDissem::new(DissemControl::Oc, s, Span::new(0, s.len()));
        let mut attrs = empty_attrs(Some(cls), vec![oc]);
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert!(attrs.dissem_us.is_empty());
        assert_eq!(attrs.dissem_nato.len(), 1);
    }

    #[test]
    fn conflict_classification_routes_to_dissem_us() {
        let s = "NF";
        let cls = ParsedClassification::new(
            MarkingClassification::Conflict {
                us: Classification::Secret,
                foreign: Box::new(crate::attrs::ForeignClassification::Nato(
                    NatoClassification::NatoSecret,
                )),
            },
            "S",
            Span::new(0, 1),
        );
        let mut attrs = empty_attrs(Some(cls), vec![nf(s)]);
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert_eq!(attrs.dissem_us.len(), 1);
        assert!(attrs.dissem_nato.is_empty());
    }

    #[test]
    fn fgi_classification_routes_to_dissem_us() {
        let s = "NF";
        let cls = ParsedClassification::new(
            MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([]),
            }),
            "S",
            Span::new(0, 1),
        );
        let mut attrs = empty_attrs(Some(cls), vec![nf(s)]);
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert_eq!(attrs.dissem_us.len(), 1);
        assert!(attrs.dissem_nato.is_empty());
    }

    #[test]
    fn joint_classification_routes_to_dissem_us() {
        let s = "NF";
        let cls = ParsedClassification::new(
            MarkingClassification::Joint(JointClassification {
                level: Classification::Secret,
                countries: Box::new([]),
            }),
            "S",
            Span::new(0, 1),
        );
        let mut attrs = empty_attrs(Some(cls), vec![nf(s)]);
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert_eq!(attrs.dissem_us.len(), 1);
        assert!(attrs.dissem_nato.is_empty());
    }

    #[test]
    fn no_classification_default_us_routes_to_dissem_us() {
        let s = "NF";
        let mut attrs = empty_attrs(None, vec![nf(s)]);
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert_eq!(attrs.dissem_us.len(), 1);
        assert!(attrs.dissem_nato.is_empty());
    }

    #[test]
    fn no_classification_default_nato_routes_to_dissem_nato() {
        let s = "NF";
        let mut attrs = empty_attrs(None, vec![nf(s)]);
        attribute_dissems(&mut attrs, DefaultOrigin::Nato);
        assert!(attrs.dissem_us.is_empty());
        assert_eq!(attrs.dissem_nato.len(), 1);
    }

    #[test]
    fn idempotent_under_repeated_invocation() {
        let s = "NF";
        let cls = ParsedClassification::new(
            MarkingClassification::Us(Classification::Secret),
            "S",
            Span::new(0, 1),
        );
        let mut attrs = empty_attrs(Some(cls), vec![nf(s)]);
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        let after_first_us = attrs.dissem_us.len();
        let after_first_nato = attrs.dissem_nato.len();
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert_eq!(attrs.dissem_us.len(), after_first_us);
        assert_eq!(attrs.dissem_nato.len(), after_first_nato);
    }

    #[test]
    fn merges_pre_split_fields_under_classification_flip() {
        // Simulate a second-pass attribution where the parser had
        // initially routed to dissem_us but the classification ends up
        // being NATO. The merge step must move tokens to dissem_nato.
        let s = "OC";
        let oc = ParsedDissem::new(DissemControl::Oc, s, Span::new(0, s.len()));
        let cls = ParsedClassification::new(
            MarkingClassification::Nato(NatoClassification::NatoSecret),
            "NS",
            Span::new(0, 2),
        );
        let mut attrs = empty_attrs(Some(cls), vec![oc]);
        // attrs starts with dissem in dissem_us by construction (test
        // helper builds it that way) — attribute should flip to NATO.
        attribute_dissems(&mut attrs, DefaultOrigin::Us);
        assert!(attrs.dissem_us.is_empty());
        assert_eq!(attrs.dissem_nato.len(), 1);
    }
}
