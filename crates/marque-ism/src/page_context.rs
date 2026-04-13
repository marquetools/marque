//! Page-level aggregation context for deriving expected banner markings.
//!
//! In CAPCO, a banner marking must reflect the **most restrictive union** of all
//! portion markings on a given page (or, for non-paginated material, across all
//! portions in the logical unit). This module provides [`PageContext`], which
//! accumulates portion [`IsmAttributes`] as the engine processes candidates and
//! exposes helpers that derive the expected classification, controls, and
//! declassification date from that aggregate.
//!
//! # Aggregation rules
//!
//! | Field                  | Aggregation              | Rationale                                   |
//! |------------------------|--------------------------|---------------------------------------------|
//! | `classification`       | `max()` (most restrictive) | Higher classification wins                |
//! | `sci_controls`         | union                    | Any control on any portion applies to page  |
//! | `sar_identifiers`      | union                    | Any SAR on any portion applies to page      |
//! | `dissem_controls`      | union                    | Any restriction on any portion applies      |
//! | `rel_to`               | intersection             | Page is releasable only to countries that   |
//! |                        |                          | appear in *every* REL TO portion            |
//! | `declassify_on`        | max date (furthest out)  | Most conservative declassification applies  |
//! | `declass_exemption`    | most specific            | Exemption with longest default duration     |
//!
//! ## REL TO intersection note
//! A portion marked `REL TO USA, GBR` is accessible to GBR. A different portion
//! marked `REL TO USA, DEU` is accessible to DEU. The page as a whole is only
//! accessible to countries that can see **all** portions — the intersection
//! (typically USA only when mixed REL TO lists are present). Portions without any
//! REL TO marking contribute no restriction; if one portion has `NOFORN`, that
//! dissem control supersedes REL TO on the banner.
//!
//! ## Declassification date note
//! Per EO 13526, the declassification date defaults to:
//! - 25 years from the date of origin for most information
//! - 10 years for fleeting/tactical operational information
//! - 50 years for HUMINT sources and methods
//!
//! `PageContext` stores the raw observed dates from portions; the engine or a
//! future Phase 3 rule is responsible for applying the appropriate default when
//! no explicit date is present.

use crate::attrs::{
    Classification, DeclassExemption, DissemControl, IsmAttributes, SarIdentifier, SciControl,
    Trigraph,
};

/// Page-level aggregation context, built by the engine as it processes portion
/// markings on a page.
///
/// Pass a `PageContext` (wrapped in [`std::sync::Arc`]) through [`crate::RuleContext`]
/// (TODO: Phase 3 wiring) to allow banner-validation rules to compare the observed
/// banner against the expected composite derived from all seen portions.
///
/// # Thread-safety
/// `PageContext` is not `Sync` — the engine builds it sequentially during a single
/// document pass. If future batch processing requires sharing, wrap in `Arc<Mutex<_>>`.
#[derive(Debug, Clone, Default)]
pub struct PageContext {
    /// Accumulated portion attributes, in document order.
    portions: Vec<IsmAttributes>,
}

impl PageContext {
    /// Create an empty context (no portions seen yet).
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a newly-parsed portion marking. Must be called in document order
    /// before banner rules are checked.
    pub fn add_portion(&mut self, attrs: IsmAttributes) {
        self.portions.push(attrs);
    }

    /// Number of portions accumulated so far.
    pub fn portion_count(&self) -> usize {
        self.portions.len()
    }

    /// Whether any portions have been accumulated.
    pub fn is_empty(&self) -> bool {
        self.portions.is_empty()
    }

    // -----------------------------------------------------------------------
    // Derived banner characteristics
    // -----------------------------------------------------------------------

    /// The classification level the banner *must* carry: the maximum (most
    /// restrictive) classification across all accumulated portions.
    ///
    /// Returns `None` only if no portions have been accumulated or all
    /// portions failed to parse a classification level.
    pub fn expected_classification(&self) -> Option<Classification> {
        self.portions.iter().filter_map(|a| a.us_classification()).max()
    }

    /// All SCI controls that must appear on the banner (union of all portions).
    pub fn expected_sci_controls(&self) -> Vec<SciControl> {
        let mut seen = std::collections::BTreeSet::new();
        for attrs in &self.portions {
            for &ctrl in attrs.sci_controls.iter() {
                seen.insert(ctrl);
            }
        }
        seen.into_iter().collect()
    }

    /// All SAR identifiers that must appear on the banner (union of all portions).
    pub fn expected_sar_identifiers(&self) -> Vec<SarIdentifier> {
        let mut seen = std::collections::BTreeSet::new();
        for attrs in &self.portions {
            for &sar in attrs.sar_identifiers.iter() {
                seen.insert(sar);
            }
        }
        seen.into_iter().collect()
    }

    /// All dissemination controls that must appear on the banner (union of all
    /// portions). Includes controls from the banner form (e.g., NOFORN, ORCON).
    pub fn expected_dissem_controls(&self) -> Vec<DissemControl> {
        let mut seen = std::collections::BTreeSet::new();
        for attrs in &self.portions {
            for &ctrl in attrs.dissem_controls.iter() {
                seen.insert(ctrl);
            }
        }
        seen.into_iter().collect()
    }

    /// The REL TO trigraph list the banner must carry.
    ///
    /// Because the banner must be accessible only to parties that can see every
    /// portion, the result is the **intersection** of all REL TO lists. Portions
    /// with no REL TO list are treated as unrestricted (contributing all countries
    /// to the intersection); a NOFORN portion makes the result empty (the banner
    /// should use NOFORN instead of REL TO).
    ///
    /// Returns an empty slice when:
    /// - No portions have a REL TO list, OR
    /// - Any portion carries NOFORN (which supersedes REL TO on the banner)
    pub fn expected_rel_to(&self) -> Vec<Trigraph> {
        use crate::attrs::DissemControl;

        // If any portion is NOFORN, NOFORN wins — REL TO is superseded.
        let any_noforn = self.portions.iter().any(|a| {
            a.dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::Nf))
        });
        if any_noforn {
            return vec![];
        }

        // Gather only portions that actually have a REL TO list.
        let rel_to_portions: Vec<_> = self
            .portions
            .iter()
            .filter(|a| !a.rel_to.is_empty())
            .collect();

        if rel_to_portions.is_empty() {
            return vec![];
        }

        // Intersection: collect trigraphs from the first portion that appear
        // in every subsequent portion, using iterators to avoid mutation.
        rel_to_portions[0]
            .rel_to
            .iter()
            .copied()
            .filter(|t| {
                rel_to_portions[1..]
                    .iter()
                    .all(|attrs| attrs.rel_to.contains(t))
            })
            .collect()
    }

    /// The maximum (furthest-out) declassification date observed across all
    /// portions, as an `YYYYMMDD` or `YYYY` string, or `None` if no portion
    /// carries one.
    ///
    /// A banner or CAB that specifies an earlier date than this maximum is a
    /// violation — it would cause portions to be declassified before the most
    /// restrictive date allows.
    ///
    /// # Encoding invariant (read before editing the comparator)
    ///
    /// Lexicographic `String::cmp` is used here and is **semantically
    /// correct** under the encoding documented in
    /// `marque_core::parser::is_declass_date`:
    /// - `YYYYMMDD` vs `YYYYMMDD` → raw ASCII order = chronological.
    /// - `YYYY` means "declassify at the **start** of year YYYY" (Jan 1).
    ///   When compared to a `YYYYMMDD` in the same year, `"YYYY"` is a
    ///   proper prefix of `"YYYYMMDD"`, so the shorter string sorts first,
    ///   which matches the semantic "Jan 1 ≤ any later date in that year."
    /// - `YYYY` vs a `YYYYMMDD` in a different year is decided by the
    ///   first four digits, which are already chronological.
    ///
    /// If the `YYYY` convention is ever redefined to mean "end of year"
    /// (Dec 31), this comparator must switch to a parsing-based one: lex
    /// order would silently return wrong answers for `"2030"` vs
    /// `"20300101"`. `is_declass_date` in `marque-core` is the single
    /// source of truth for the encoding.
    pub fn expected_declassify_on(&self) -> Option<&str> {
        self.portions
            .iter()
            .filter_map(|a| a.declassify_on.as_deref())
            .max_by(|a, b| a.cmp(b))
    }

    /// The last-observed declass exemption across all portions, or `None` if
    /// no portion carries one.
    ///
    /// **Phase 3 TODO**: A correct implementation would return the exemption
    /// with the longest default retention duration (e.g., `50X1-HUM` > `25X1`
    /// under EO 13526 § 3.3(b)). The current implementation returns the
    /// last-seen exemption as a conservative placeholder; Phase 3 should add
    /// a duration-aware comparator using `max()`.
    pub fn expected_declass_exemption(&self) -> Option<DeclassExemption> {
        // Use the ordering defined on `DeclassExemption` (PartialOrd/Ord comes
        // from the derived Ord on the generated enum, which follows CVE order).
        // Until a richer ordering is available, return the last seen exemption
        // as a conservative default (Phase 3 can add duration-aware ordering).
        self.portions
            .iter()
            .filter_map(|a| a.declass_exemption)
            .next_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{Classification, MarkingClassification};

    fn attrs_with_classification(c: Classification) -> IsmAttributes {
        IsmAttributes {
            classification: Some(MarkingClassification::Us(c)),
            ..Default::default()
        }
    }

    #[test]
    fn expected_classification_returns_max() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Secret));
        ctx.add_portion(attrs_with_classification(Classification::Confidential));
        assert_eq!(ctx.expected_classification(), Some(Classification::Secret));
    }

    #[test]
    fn expected_classification_empty_returns_none() {
        assert_eq!(PageContext::new().expected_classification(), None);
    }

    #[test]
    fn expected_sci_controls_union() {
        use crate::attrs::SciControl;
        let mut ctx = PageContext::new();
        let a1 = IsmAttributes {
            sci_controls: vec![SciControl::Si].into_boxed_slice(),
            ..Default::default()
        };
        let a2 = IsmAttributes {
            sci_controls: vec![SciControl::Tk].into_boxed_slice(),
            ..Default::default()
        };
        ctx.add_portion(a1);
        ctx.add_portion(a2);
        let expected = ctx.expected_sci_controls();
        assert!(expected.contains(&SciControl::Si));
        assert!(expected.contains(&SciControl::Tk));
        assert_eq!(expected.len(), 2);
    }

    #[test]
    fn expected_rel_to_intersection() {
        use crate::attrs::Trigraph;
        let mut ctx = PageContext::new();
        let a1 = IsmAttributes {
            rel_to: vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into_boxed_slice(),
            ..Default::default()
        };
        let a2 = IsmAttributes {
            rel_to: vec![Trigraph::USA, Trigraph::try_new(*b"DEU").unwrap()].into_boxed_slice(),
            ..Default::default()
        };
        ctx.add_portion(a1);
        ctx.add_portion(a2);
        // Only USA appears in both → intersection is [USA]
        let rel = ctx.expected_rel_to();
        assert_eq!(rel, vec![Trigraph::USA]);
    }

    #[test]
    fn noforn_supersedes_rel_to() {
        use crate::attrs::{DissemControl, Trigraph};
        let mut ctx = PageContext::new();
        // Portion 1: REL TO USA, GBR
        let a1 = IsmAttributes {
            rel_to: vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into_boxed_slice(),
            ..Default::default()
        };
        // Portion 2: NOFORN
        let a2 = IsmAttributes {
            dissem_controls: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        };
        ctx.add_portion(a1);
        ctx.add_portion(a2);
        // NOFORN wins → expected REL TO is empty (banner should say NOFORN)
        assert!(ctx.expected_rel_to().is_empty());
    }

    #[test]
    fn expected_declassify_on_max() {
        let a1 = IsmAttributes {
            declassify_on: Some("20350101".into()),
            ..Default::default()
        };
        let a2 = IsmAttributes {
            declassify_on: Some("20481231".into()),
            ..Default::default()
        };
        let mut ctx = PageContext::new();
        ctx.add_portion(a1);
        ctx.add_portion(a2);
        assert_eq!(ctx.expected_declassify_on(), Some("20481231"));
    }
}
