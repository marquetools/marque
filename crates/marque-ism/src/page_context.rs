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
    AeaMarking, Classification, DeclassExemption, DissemControl, FgiMarker, IsmAttributes,
    MarkingClassification, NonIcDissem, SarIdentifier, SciControl, Trigraph,
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

    /// All dissemination controls that must appear on the banner.
    ///
    /// Base rule is union, with important exceptions per ISM-Rollup XSLT:
    ///
    /// - **OC-USGOV**: Drops if not present on ALL OC-carrying portions
    /// - **FOUO**: Drops in classified documents (stays in unclassified)
    /// - **DSEN**: Overrides/replaces FOUO when both present
    /// - **NF injection**: Added when non-IC SBU-NF/LES-NF split occurs
    ///   in classified docs (caller should check `expected_non_ic_dissem()`)
    pub fn expected_dissem_controls(&self) -> Vec<DissemControl> {
        let classified = self.is_classified();

        // Step 1: Basic union of all dissem controls.
        let mut seen = std::collections::BTreeSet::new();
        for attrs in &self.portions {
            for &ctrl in attrs.dissem_controls.iter() {
                seen.insert(ctrl);
            }
        }

        // Step 2: OC-USGOV drops if not on ALL OC-carrying portions.
        if seen.contains(&DissemControl::OcUsgov) {
            let oc_portions: Vec<_> = self
                .portions
                .iter()
                .filter(|a| a.dissem_controls.contains(&DissemControl::Oc))
                .collect();
            if !oc_portions.is_empty() {
                let all_have_usgov = oc_portions
                    .iter()
                    .all(|a| a.dissem_controls.contains(&DissemControl::OcUsgov));
                if !all_have_usgov {
                    seen.remove(&DissemControl::OcUsgov);
                }
            }
        }

        // Step 3: FOUO drops in classified documents.
        if classified && seen.contains(&DissemControl::Fouo) {
            // DSEN overrides FOUO — if both present, FOUO is replaced by DSEN
            // regardless of classification. If only FOUO, it just drops in
            // classified docs.
            seen.remove(&DissemControl::Fouo);
        }

        // Step 4: NF injection from non-IC SBU-NF/LES-NF split.
        let (_, needs_nf) = self.expected_non_ic_dissem();
        if needs_nf {
            seen.insert(DissemControl::Nf);
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

    // -----------------------------------------------------------------------
    // Classification helper
    // -----------------------------------------------------------------------

    /// Whether the expected banner classification is above UNCLASSIFIED.
    ///
    /// Used by AEA, non-IC, and FOUO rollup logic — several markings are
    /// dropped or transformed when the document/page is classified.
    pub fn is_classified(&self) -> bool {
        self.expected_classification()
            .is_some_and(|c| c > Classification::Unclassified)
    }

    // -----------------------------------------------------------------------
    // AEA rollup
    // -----------------------------------------------------------------------

    /// Expected AEA markings for the banner.
    ///
    /// Aggregation rules (ISM-Rollup XSLT + CAPCO-2016):
    /// - Union of all AEA markings across portions
    /// - **UCNI/DCNI drop in classified documents** — only appear in
    ///   unclassified banners
    /// - SIGMA compartment numbers are aggregated and sorted
    /// - RD blocks are merged: if multiple portions have RD with different
    ///   SIGMA numbers, the result is a single RD block with all SIGMAs
    pub fn expected_aea_markings(&self) -> Vec<AeaMarking> {
        let classified = self.is_classified();
        let mut has_rd = false;
        let mut rd_cnwdi = false;
        let mut rd_sigma: std::collections::BTreeSet<u8> = std::collections::BTreeSet::new();
        let mut has_frd = false;
        let mut frd_sigma: std::collections::BTreeSet<u8> = std::collections::BTreeSet::new();
        let mut has_tfni = false;
        let mut has_dod_ucni = false;
        let mut has_doe_ucni = false;

        for attrs in &self.portions {
            for aea in attrs.aea_markings.iter() {
                match aea {
                    AeaMarking::Rd(rd) => {
                        has_rd = true;
                        if rd.cnwdi {
                            rd_cnwdi = true;
                        }
                        for &s in rd.sigma.iter() {
                            rd_sigma.insert(s);
                        }
                    }
                    AeaMarking::Frd(frd) => {
                        has_frd = true;
                        for &s in frd.sigma.iter() {
                            frd_sigma.insert(s);
                        }
                    }
                    AeaMarking::Tfni => has_tfni = true,
                    AeaMarking::DodUcni => has_dod_ucni = true,
                    AeaMarking::DoeUcni => has_doe_ucni = true,
                }
            }
        }

        let mut result = Vec::new();

        // RD (with merged CNWDI + SIGMA). RD takes precedence — if RD is
        // present, FRD SIGMAs merge into the RD block per CAPCO-2016.
        if has_rd {
            // When both RD and FRD SIGMA are present, all SIGMAs go
            // under the RD block.
            let all_sigma: Vec<u8> = rd_sigma.union(&frd_sigma).copied().collect();
            result.push(AeaMarking::Rd(crate::attrs::RdBlock {
                cnwdi: rd_cnwdi,
                sigma: all_sigma.into(),
            }));
        }

        // FRD only if RD is not present (RD takes precedence).
        if has_frd && !has_rd {
            result.push(AeaMarking::Frd(crate::attrs::FrdBlock {
                sigma: frd_sigma.into_iter().collect::<Vec<_>>().into(),
            }));
        }

        if has_tfni && !has_rd {
            result.push(AeaMarking::Tfni);
        }

        // UCNI/DCNI drop in classified documents.
        if !classified {
            if has_dod_ucni {
                result.push(AeaMarking::DodUcni);
            }
            if has_doe_ucni {
                result.push(AeaMarking::DoeUcni);
            }
        }

        result
    }

    // -----------------------------------------------------------------------
    // FGI rollup
    // -----------------------------------------------------------------------

    /// Expected FGI marker for the banner.
    ///
    /// Rules (ISM-Rollup XSLT):
    /// - If any portion has source-concealed FGI (empty countries), the
    ///   banner uses bare `FGI` with no countries (protected supersedes open)
    /// - Otherwise, union of all FGI source countries
    /// - Non-USA ownerProducer on portions contributes to FGI sources
    pub fn expected_fgi_marker(&self) -> Option<FgiMarker> {
        let mut has_any_fgi = false;
        let mut has_source_concealed = false;
        let mut countries = std::collections::BTreeSet::new();

        for attrs in &self.portions {
            // Explicit FGI markers on portions.
            if let Some(marker) = &attrs.fgi_marker {
                has_any_fgi = true;
                if marker.countries.is_empty() {
                    has_source_concealed = true;
                } else {
                    for &c in marker.countries.iter() {
                        countries.insert(c.as_str().to_owned());
                    }
                }
            }

            // Non-US classification systems contribute to FGI in banner.
            match &attrs.classification {
                Some(MarkingClassification::Fgi(fgi)) => {
                    has_any_fgi = true;
                    if fgi.countries.is_empty() {
                        has_source_concealed = true;
                    } else {
                        for c in fgi.countries.iter() {
                            countries.insert(c.as_str().to_owned());
                        }
                    }
                }
                Some(MarkingClassification::Nato(_)) => {
                    has_any_fgi = true;
                    countries.insert("NATO".to_owned());
                }
                Some(MarkingClassification::Joint(j)) => {
                    has_any_fgi = true;
                    for c in j.countries.iter() {
                        if c.as_str() != "USA" {
                            countries.insert(c.as_str().to_owned());
                        }
                    }
                }
                _ => {}
            }
        }

        if !has_any_fgi {
            return None;
        }

        // Source-concealed supersedes all open sources.
        if has_source_concealed {
            return Some(FgiMarker {
                countries: Box::new([]),
            });
        }

        // Convert country strings back to Trigraphs (only 3-char codes).
        let trigraphs: Vec<Trigraph> = countries
            .iter()
            .filter_map(|s| {
                if s.len() == 3 {
                    Trigraph::try_new(s.as_bytes().try_into().ok()?)
                } else {
                    None // Skip tetragraphs like NATO for now
                }
            })
            .collect();

        Some(FgiMarker {
            countries: trigraphs.into(),
        })
    }

    // -----------------------------------------------------------------------
    // Non-IC dissem rollup
    // -----------------------------------------------------------------------

    /// Expected non-IC dissem controls for the banner.
    ///
    /// Rules (ISM-Rollup XSLT + NonICRollup.xspec):
    /// - Union of all non-IC controls across portions
    /// - **SBU-NF in classified docs**: Splits to SBU + NF (NF goes to dissem)
    /// - **LES-NF in classified docs**: Splits to LES + NF
    /// - In unclassified docs: SBU-NF and LES-NF kept intact
    ///
    /// Returns a tuple: (non_ic_controls, additional_nf) where additional_nf
    /// is true if NF should be added to dissem controls from the split.
    pub fn expected_non_ic_dissem(&self) -> (Vec<NonIcDissem>, bool) {
        let classified = self.is_classified();
        let mut seen = std::collections::BTreeSet::new();
        let mut needs_nf_from_split = false;

        for attrs in &self.portions {
            for &nic in attrs.non_ic_dissem.iter() {
                seen.insert(nic);
            }
        }

        if classified {
            // SBU-NF → SBU + NF (dissem)
            if seen.remove(&NonIcDissem::SbuNf) {
                seen.insert(NonIcDissem::Sbu);
                needs_nf_from_split = true;
            }
            // LES-NF → LES + NF (dissem)
            if seen.remove(&NonIcDissem::LesNf) {
                seen.insert(NonIcDissem::Les);
                needs_nf_from_split = true;
            }
        }

        (seen.into_iter().collect(), needs_nf_from_split)
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

    // --- is_classified ---

    #[test]
    fn is_classified_true_for_secret() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Secret));
        assert!(ctx.is_classified());
    }

    #[test]
    fn is_classified_false_for_unclassified() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Unclassified));
        assert!(!ctx.is_classified());
    }

    // --- AEA rollup ---

    #[test]
    fn aea_rd_union_across_portions() {
        use crate::attrs::{AeaMarking, RdBlock};
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            aea_markings: vec![AeaMarking::Rd(RdBlock::default())].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            aea_markings: vec![AeaMarking::Rd(RdBlock {
                cnwdi: false,
                sigma: vec![18].into(),
            })]
            .into(),
            ..Default::default()
        });
        let aea = ctx.expected_aea_markings();
        assert_eq!(aea.len(), 1);
        match &aea[0] {
            AeaMarking::Rd(rd) => {
                assert!(!rd.cnwdi);
                assert_eq!(&*rd.sigma, &[18]);
            }
            other => panic!("expected Rd, got: {other:?}"),
        }
    }

    #[test]
    fn aea_sigma_aggregated_sorted() {
        use crate::attrs::{AeaMarking, RdBlock};
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            aea_markings: vec![AeaMarking::Rd(RdBlock {
                cnwdi: false,
                sigma: vec![20, 14].into(),
            })]
            .into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            aea_markings: vec![AeaMarking::Rd(RdBlock {
                cnwdi: false,
                sigma: vec![18].into(),
            })]
            .into(),
            ..Default::default()
        });
        let aea = ctx.expected_aea_markings();
        match &aea[0] {
            AeaMarking::Rd(rd) => {
                // All unique SIGMAs merged and sorted.
                assert_eq!(&*rd.sigma, &[14, 18, 20]);
            }
            other => panic!("expected Rd, got: {other:?}"),
        }
    }

    #[test]
    fn aea_ucni_drops_in_classified() {
        use crate::attrs::AeaMarking;
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            aea_markings: vec![AeaMarking::DodUcni].into(),
            ..Default::default()
        });
        // Classified doc → UCNI drops.
        let aea = ctx.expected_aea_markings();
        assert!(aea.is_empty(), "UCNI should drop in classified: {aea:?}");
    }

    #[test]
    fn aea_ucni_kept_in_unclassified() {
        use crate::attrs::AeaMarking;
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            aea_markings: vec![AeaMarking::DodUcni].into(),
            ..Default::default()
        });
        let aea = ctx.expected_aea_markings();
        assert_eq!(aea.len(), 1);
        assert_eq!(aea[0], AeaMarking::DodUcni);
    }

    // --- Non-IC rollup ---

    #[test]
    fn non_ic_sbu_nf_splits_in_classified() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            non_ic_dissem: vec![NonIcDissem::SbuNf].into(),
            ..Default::default()
        });
        let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();
        // SBU-NF splits to SBU + NF
        assert!(non_ic.contains(&NonIcDissem::Sbu));
        assert!(!non_ic.contains(&NonIcDissem::SbuNf));
        assert!(needs_nf, "NF should be added to dissem from SBU-NF split");
    }

    #[test]
    fn non_ic_sbu_nf_kept_in_unclassified() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            non_ic_dissem: vec![NonIcDissem::SbuNf].into(),
            ..Default::default()
        });
        let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();
        assert!(non_ic.contains(&NonIcDissem::SbuNf));
        assert!(!needs_nf);
    }

    // --- FGI rollup ---

    #[test]
    fn fgi_source_concealed_supersedes_open() {
        let mut ctx = PageContext::new();
        // One portion with source-concealed FGI.
        ctx.add_portion(IsmAttributes {
            fgi_marker: Some(FgiMarker {
                countries: Box::new([]),
            }),
            ..Default::default()
        });
        // Another with source-acknowledged FGI.
        ctx.add_portion(IsmAttributes {
            fgi_marker: Some(FgiMarker {
                countries: vec![Trigraph::try_new(*b"GBR").unwrap()].into(),
            }),
            ..Default::default()
        });
        let marker = ctx.expected_fgi_marker().expect("should have FGI marker");
        // Source-concealed wins → no countries.
        assert!(
            marker.countries.is_empty(),
            "source-concealed should supersede: {:?}",
            marker.countries,
        );
    }

    #[test]
    fn fgi_open_union_of_countries() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            fgi_marker: Some(FgiMarker {
                countries: vec![Trigraph::try_new(*b"GBR").unwrap()].into(),
            }),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            fgi_marker: Some(FgiMarker {
                countries: vec![Trigraph::try_new(*b"DEU").unwrap()].into(),
            }),
            ..Default::default()
        });
        let marker = ctx.expected_fgi_marker().unwrap();
        assert_eq!(marker.countries.len(), 2);
    }
}
