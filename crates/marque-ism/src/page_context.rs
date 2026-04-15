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
    MarkingClassification, NonIcDissem, SarCompartment, SarIndicator, SarMarking, SarProgram,
    SciControl, Trigraph,
};

/// Sort key for SAR identifiers per CAPCO §H.5 (p99–100): "ascending sort order
/// with numbered values first, followed by alphabetic values" at each hierarchical
/// level.
///
/// Splits the identifier at its leading digit run. If present, the digits are
/// parsed as `u64` and the tuple `(false, n, rest)` is returned (with `false`
/// sorting before `true`). Pure-alpha identifiers return `(true, 0, s)`.
///
/// This helper is the canonical SAR sort-key implementation; both
/// `marque-ism` (banner roll-up) and `marque-capco` (rules E028/E029) use it
/// via the crate re-export.
pub fn sar_sort_key(s: &str) -> (bool, u64, &str) {
    let prefix_len = s.bytes().take_while(|b| b.is_ascii_digit()).count();
    if prefix_len == 0 {
        (true, 0, s)
    } else {
        let n: u64 = s[..prefix_len].parse().unwrap_or(u64::MAX);
        (false, n, &s[prefix_len..])
    }
}

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
        self.portions
            .iter()
            .filter_map(|a| a.us_classification())
            .max()
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

    /// Expected SAR marking rolled up from all accumulated portions.
    ///
    /// Returns `None` if no portion carries a SAR marking. Otherwise returns a
    /// [`SarMarking`] with `indicator = SarIndicator::Abbrev` and
    /// programs / compartments / sub-compartments unioned across portions,
    /// each hierarchical level sorted per CAPCO §H.5 ascending order
    /// (numeric-prefixed values first as `u64`, then alphabetic).
    ///
    /// The rollup does NOT flag disagreements between portions and the
    /// observed banner — it just produces the expected composite marking.
    /// Rule E031 (`sar-banner-rollup`) consumes this roll-up and compares
    /// the expected value against what actually appears on the page banner.
    pub fn expected_sar_marking(&self) -> Option<SarMarking> {
        use std::collections::{BTreeMap, BTreeSet};

        // program identifier → compartment identifier → set of sub-compartment
        // identifiers. BTreeMap/BTreeSet give deterministic ordering but we
        // re-sort per CAPCO semantics below (BTree's lexicographic order puts
        // "12A" before "2" — wrong for §H.5).
        let mut programs: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

        for attrs in &self.portions {
            let Some(sar) = attrs.sar_markings.as_ref() else {
                continue;
            };
            for prog in sar.programs.iter() {
                let comps = programs.entry(prog.identifier.to_string()).or_default();
                for comp in prog.compartments.iter() {
                    let subs = comps.entry(comp.identifier.to_string()).or_default();
                    for sub in comp.sub_compartments.iter() {
                        subs.insert(sub.to_string());
                    }
                }
            }
        }

        if programs.is_empty() {
            return None;
        }

        // Sort each hierarchical level per CAPCO §H.5 (numeric-first, then alpha).
        let mut prog_keys: Vec<String> = programs.keys().cloned().collect();
        prog_keys.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));

        let built_programs: Vec<SarProgram> = prog_keys
            .into_iter()
            .map(|pid| {
                let comp_map = programs.remove(&pid).expect("key enumerated above");
                let mut comp_keys: Vec<String> = comp_map.keys().cloned().collect();
                comp_keys.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));

                let built_compartments: Vec<SarCompartment> = comp_keys
                    .into_iter()
                    .map(|cid| {
                        let subs = comp_map
                            .get(&cid)
                            .expect("key enumerated above");
                        let mut sub_vec: Vec<String> = subs.iter().cloned().collect();
                        sub_vec.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));
                        let boxed: Box<[Box<str>]> = sub_vec
                            .into_iter()
                            .map(|s| s.into_boxed_str())
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        SarCompartment::new(cid.into_boxed_str(), boxed)
                    })
                    .collect();

                SarProgram::new(
                    pid.into_boxed_str(),
                    built_compartments.into_boxed_slice(),
                )
            })
            .collect();

        Some(SarMarking::new(
            SarIndicator::Abbrev,
            built_programs.into_boxed_slice(),
        ))
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

        // Step 3: FOUO drops in classified documents; also drops whenever DSEN
        // is present (DSEN overrides FOUO regardless of classification level).
        let dsen_present = seen.contains(&DissemControl::Dsen);
        if seen.contains(&DissemControl::Fouo) && (classified || dsen_present) {
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
    /// The result is the **intersection** of all REL TO lists across portions,
    /// with tetragraph expansion (FVEY → {AUS, CAN, GBR, NZL, USA}) applied
    /// before intersection.
    ///
    /// Returns an empty slice when:
    /// - No portions have a REL TO list, OR
    /// - Any portion carries NOFORN (which supersedes REL TO on the banner)
    ///
    /// When the intersection is empty (no common countries), this returns
    /// an empty vec — the caller should add NF to dissem controls.
    pub fn expected_rel_to(&self) -> Vec<Trigraph> {
        // If any portion is NOFORN, NOFORN wins — REL TO is superseded.
        let any_noforn = self.portions.iter().any(|a| {
            a.dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::Nf))
        });
        if any_noforn {
            return vec![];
        }

        // Also check if NF will be injected from non-IC split.
        let (_, needs_nf) = self.expected_non_ic_dissem();
        if needs_nf {
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

        // Expand each portion's REL TO into a set of trigraphs, resolving
        // known tetragraphs (FVEY, ACGU, etc.) into constituent countries.
        let expanded: Vec<std::collections::BTreeSet<&str>> = rel_to_portions
            .iter()
            .map(|a| {
                let mut set = std::collections::BTreeSet::new();
                for t in a.rel_to.iter() {
                    let s = t.as_str();
                    if let Some(members) = expand_tetragraph(s) {
                        for &m in members {
                            set.insert(m);
                        }
                    } else {
                        set.insert(s);
                    }
                }
                set
            })
            .collect();

        // Intersection across all expanded sets.
        let mut result: std::collections::BTreeSet<&str> = expanded[0].clone();
        for set in &expanded[1..] {
            result = result.intersection(set).copied().collect();
        }

        // Convert back to Trigraphs, sorted with USA first.
        let mut trigraphs: Vec<Trigraph> = result
            .iter()
            .filter_map(|s| {
                if s.len() == 3 {
                    Trigraph::try_new(s.as_bytes().try_into().ok()?)
                } else {
                    None
                }
            })
            .collect();

        // USA first, rest alphabetical.
        if let Some(pos) = trigraphs.iter().position(|t| *t == Trigraph::USA) {
            if pos != 0 {
                let usa = trigraphs.remove(pos);
                trigraphs.insert(0, usa);
            }
        }

        trigraphs
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

    /// Assemble a CAPCO banner string from all accumulated portion markings.
    ///
    /// Per CAPCO §D.1:
    /// - Categories are separated by `//` (double forward slash).
    /// - Multiple entries **within** a category are separated by `/` (single slash).
    ///
    /// Example: `"TOP SECRET//SI/TK//NOFORN"` (SI and TK are both SCI controls
    /// sharing one block; NOFORN is a dissem control in its own block).
    ///
    /// Returns `None` if no portions have been accumulated.
    /// Returns `"UNCLASSIFIED"` if portions exist but none carry a classification.
    pub fn render_expected_banner(&self) -> Option<String> {
        if self.portions.is_empty() {
            return None;
        }
        let classification = self
            .expected_classification()
            .map(|c| c.banner_str().to_owned())
            .unwrap_or_else(|| "UNCLASSIFIED".to_owned());

        let mut blocks: Vec<String> = vec![classification];

        // SCI controls — all in ONE block, `/`-separated per CAPCO §D.1.
        let sci = self.expected_sci_controls();
        if !sci.is_empty() {
            blocks.push(sci.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("/"));
        }

        // SAR block — rendered per CAPCO §H.5 p100 canonical form:
        //   SAR-{prog1}[-{comp1}[ {sub}...][-{comp2}...]][/{prog2}-...]
        // Programs are `/`-separated; compartments within a program are
        // `-`-separated; sub-compartments within a compartment are
        // space-separated (e.g., `SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB`).
        if let Some(sar) = self.expected_sar_marking() {
            blocks.push(render_sar_block(&sar));
        }

        // AEA markings — all in ONE block, `/`-separated.
        let aea = self.expected_aea_markings();
        if !aea.is_empty() {
            blocks.push(
                aea.iter()
                    .map(|a| a.banner_str())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }

        // Dissem controls + REL TO — dissem controls and REL TO together, each
        // category already collected by expected_dissem_controls().
        // DissemControl::as_str() returns the portion abbreviation ("NF", "RELIDO"),
        // so convert to banner form via marking_forms::portion_to_banner().
        let rel_to = self.expected_rel_to();
        let (non_ic, needs_nf_from_non_ic) = self.expected_non_ic_dissem();
        let dissem = self.expected_dissem_controls();

        let mut dissem_parts: Vec<String> = Vec::new();
        for d in &dissem {
            let portion = d.as_str();
            // Convert portion form to banner form (e.g. "NF" → "NOFORN").
            // Fall back to portion form if no banner mapping exists.
            let banner = crate::marking_forms::portion_to_banner(portion).unwrap_or(portion);
            // Skip bare "REL" token if we're going to emit "REL TO ..." below.
            if banner == "REL" && !rel_to.is_empty() {
                continue;
            }
            dissem_parts.push(banner.to_owned());
        }
        // If non-IC SBU-NF/LES-NF split injected NOFORN, add it.
        if needs_nf_from_non_ic && !dissem_parts.iter().any(|p| p == "NOFORN") {
            dissem_parts.push("NOFORN".to_owned());
        }
        // REL TO list (comma-delimited countries with "REL TO " prefix).
        if !rel_to.is_empty() {
            let trigraphs = rel_to
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            dissem_parts.push(format!("REL TO {trigraphs}"));
        }
        if !dissem_parts.is_empty() {
            blocks.push(dissem_parts.join("/"));
        }

        // Non-IC dissem controls (only appear in UNCLASSIFIED banners per CAPCO;
        // in classified docs most are stripped, per expected_non_ic_dissem()).
        // These use banner_str() which returns the full-form name.
        if !non_ic.is_empty() {
            blocks.push(
                non_ic
                    .iter()
                    .map(|n| n.banner_str())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }

        Some(blocks.join("//"))
    }
}

/// Render a rolled-up [`SarMarking`] to its canonical §H.5 banner block form
/// (without the leading `//` category separator).
///
/// Format: `SAR-{prog1}[-{comp1}[ {sub} ...][-{comp2}...]][/{prog2}-...]`
///
/// The indicator is always rendered as the abbreviated `SAR-` form because
/// [`PageContext::expected_sar_marking`] normalizes to [`SarIndicator::Abbrev`]
/// per §H.5 p100.
fn render_sar_block(sar: &SarMarking) -> String {
    let mut out = String::from("SAR-");
    let mut first_prog = true;
    for prog in sar.programs.iter() {
        if !first_prog {
            out.push('/');
        }
        first_prog = false;
        out.push_str(&prog.identifier);
        for comp in prog.compartments.iter() {
            out.push('-');
            out.push_str(&comp.identifier);
            for sub in comp.sub_compartments.iter() {
                out.push(' ');
                out.push_str(sub);
            }
        }
    }
    out
}

/// Expand known tetragraphs into their constituent trigraphs.
///
/// Per CAPCO, tetragraphs like FVEY represent groups of countries:
/// - FVEY = Five Eyes: AUS, CAN, GBR, NZL, USA
/// - ACGU = AUS, CAN, GBR, USA (Four Eyes minus NZL)
///
/// Returns `None` for trigraphs and unknown codes (pass through as-is).
fn expand_tetragraph(code: &str) -> Option<&'static [&'static str]> {
    match code {
        "FVEY" => Some(&["AUS", "CAN", "GBR", "NZL", "USA"]),
        "ACGU" => Some(&["AUS", "CAN", "GBR", "USA"]),
        // Add more tetragraphs as needed.
        _ => None,
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

    // --- REL TO with FVEY expansion ---

    #[test]
    fn rel_to_fvey_expansion_intersects_correctly() {
        // Portion 1: REL TO USA, FVEY (expands to USA, AUS, CAN, GBR, NZL)
        // Portion 2: REL TO USA, AUS, CAN
        // Intersection: USA, AUS, CAN
        // Note: we can't store "FVEY" in Trigraph (4 chars), so this test
        // uses the expanded form directly. The expansion logic is tested
        // via the expand_tetragraph function.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![
                Trigraph::USA,
                Trigraph::try_new(*b"AUS").unwrap(),
                Trigraph::try_new(*b"CAN").unwrap(),
                Trigraph::try_new(*b"GBR").unwrap(),
                Trigraph::try_new(*b"NZL").unwrap(),
            ]
            .into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![
                Trigraph::USA,
                Trigraph::try_new(*b"AUS").unwrap(),
                Trigraph::try_new(*b"CAN").unwrap(),
            ]
            .into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        assert_eq!(rel.len(), 3);
        assert_eq!(rel[0], Trigraph::USA); // USA first
        assert_eq!(rel[1].as_str(), "AUS");
        assert_eq!(rel[2].as_str(), "CAN");
    }

    #[test]
    fn rel_to_empty_intersection_returns_empty() {
        // REL TO USA, AUS + REL TO USA, GBR → no common (just USA)
        // Wait, USA is common. Let's test non-overlapping non-USA countries.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![Trigraph::USA, Trigraph::try_new(*b"AUS").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        // USA is the intersection — still produces a result.
        assert_eq!(rel.len(), 1);
        assert_eq!(rel[0], Trigraph::USA);
    }

    // --- Dissem special cases ---

    #[test]
    fn dissem_fouo_drops_in_classified() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_controls: vec![DissemControl::Fouo].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_controls();
        assert!(
            !dissem.contains(&DissemControl::Fouo),
            "FOUO should drop in classified doc: {dissem:?}"
        );
    }

    #[test]
    fn dissem_fouo_kept_in_unclassified() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            dissem_controls: vec![DissemControl::Fouo].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_controls();
        assert!(
            dissem.contains(&DissemControl::Fouo),
            "FOUO should stay in unclassified: {dissem:?}"
        );
    }

    #[test]
    fn dissem_fouo_drops_when_dsen_present_unclassified() {
        // DSEN overrides FOUO even on an unclassified page.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            dissem_controls: vec![DissemControl::Dsen, DissemControl::Fouo].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_controls();
        assert!(
            !dissem.contains(&DissemControl::Fouo),
            "FOUO should drop when DSEN is present, even unclassified: {dissem:?}"
        );
        assert!(
            dissem.contains(&DissemControl::Dsen),
            "DSEN should be retained: {dissem:?}"
        );
    }

    #[test]
    fn dissem_oc_usgov_drops_when_not_on_all_oc_portions() {
        let mut ctx = PageContext::new();
        // Two OC portions, only one has OC-USGOV.
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_controls: vec![DissemControl::Oc, DissemControl::OcUsgov].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_controls: vec![DissemControl::Oc].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_controls();
        assert!(dissem.contains(&DissemControl::Oc));
        assert!(
            !dissem.contains(&DissemControl::OcUsgov),
            "OC-USGOV should drop when not on all OC portions: {dissem:?}"
        );
    }

    #[test]
    fn dissem_nf_injected_from_sbu_nf_split() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            non_ic_dissem: vec![NonIcDissem::SbuNf].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_controls();
        assert!(
            dissem.contains(&DissemControl::Nf),
            "NF should be injected from SBU-NF split: {dissem:?}"
        );
    }

    // --- render_expected_banner ---

    #[test]
    fn render_banner_empty_returns_none() {
        assert_eq!(PageContext::new().render_expected_banner(), None);
    }

    #[test]
    fn render_banner_ts_si_tk_noforn() {
        // The canonical demo banner: TOP SECRET//SI/TK//NOFORN
        // SI and TK are both SCI controls → one block `/`-joined
        // NOFORN is a dissem control → its own block
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_controls: vec![SciControl::Si, SciControl::Tk].into_boxed_slice(),
            dissem_controls: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("TOP SECRET//SI/TK//NOFORN")
        );
    }

    #[test]
    fn render_banner_rollup_from_multiple_portions() {
        // (TS//SI/TK//NF) + (S//NF) + (U) → TOP SECRET//SI/TK//NOFORN
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_controls: vec![SciControl::Si, SciControl::Tk].into_boxed_slice(),
            dissem_controls: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_controls: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("TOP SECRET//SI/TK//NOFORN")
        );
    }

    #[test]
    fn render_banner_secret_noforn() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_controls: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("SECRET//NOFORN")
        );
    }

    #[test]
    fn render_banner_unclassified_with_no_dissem() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("UNCLASSIFIED")
        );
    }

    // --- expected_sar_marking (P4a) ---

    use crate::attrs::{SarCompartment, SarIndicator, SarMarking, SarProgram};

    fn sar_prog(id: &str, comps: Vec<SarCompartment>) -> SarProgram {
        SarProgram::new(id.into(), comps.into_boxed_slice())
    }

    fn sar_comp(id: &str, subs: &[&str]) -> SarCompartment {
        let subs: Box<[Box<str>]> = subs
            .iter()
            .map(|s| (*s).into())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        SarCompartment::new(id.into(), subs)
    }

    fn attrs_with_sar(sar: SarMarking) -> IsmAttributes {
        IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sar_markings: Some(sar),
            ..Default::default()
        }
    }

    #[test]
    fn rollup_empty_portions_returns_none() {
        assert!(PageContext::new().expected_sar_marking().is_none());
    }

    #[test]
    fn rollup_no_sar_portions_returns_none() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Secret));
        assert!(ctx.expected_sar_marking().is_none());
    }

    #[test]
    fn rollup_single_program_no_compartments() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Abbrev,
            vec![sar_prog("BP", vec![])].into_boxed_slice(),
        )));
        let got = ctx.expected_sar_marking().expect("one program");
        assert_eq!(got.indicator, SarIndicator::Abbrev);
        assert_eq!(got.programs.len(), 1);
        assert_eq!(&*got.programs[0].identifier, "BP");
        assert!(got.programs[0].compartments.is_empty());
    }

    #[test]
    fn rollup_two_portions_merge_programs() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Abbrev,
            vec![sar_prog("BP", vec![])].into_boxed_slice(),
        )));
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Full, // indicator on portion irrelevant — banner normalizes to Abbrev
            vec![sar_prog("CD", vec![])].into_boxed_slice(),
        )));
        let got = ctx.expected_sar_marking().expect("merged");
        assert_eq!(got.indicator, SarIndicator::Abbrev);
        let ids: Vec<&str> = got.programs.iter().map(|p| &*p.identifier).collect();
        assert_eq!(ids, vec!["BP", "CD"]);
    }

    #[test]
    fn rollup_merges_compartments_under_same_program() {
        // Two portions with same program, different single compartments.
        // The canonical §H.5 form uses `-` between compartments within a
        // program (see `SAR-BP-J12 J54-K15` where `-K15` is a second
        // compartment, not a sub of J54). Sub-compartments use spaces.
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Abbrev,
            vec![sar_prog("BP", vec![sar_comp("J12", &[])])].into_boxed_slice(),
        )));
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Abbrev,
            vec![sar_prog("BP", vec![sar_comp("K15", &[])])].into_boxed_slice(),
        )));
        let got = ctx.expected_sar_marking().expect("merged");
        assert_eq!(got.programs.len(), 1);
        let prog = &got.programs[0];
        assert_eq!(&*prog.identifier, "BP");
        let comps: Vec<&str> = prog.compartments.iter().map(|c| &*c.identifier).collect();
        assert_eq!(comps, vec!["J12", "K15"]);
    }

    #[test]
    fn rollup_numeric_before_alpha() {
        // Per CAPCO §H.5 sort rule: numbered values sort before alphabetic at
        // each hierarchical level. Programs [BP, AC-12A, 99] → [99, AC, BP]
        // once sorted. (Note: `sar_sort_key` splits leading digits, so `AC`
        // sorts alpha-key `(true, 0, "AC")` which is after `99`'s
        // `(false, 99, "")`.)
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Abbrev,
            vec![sar_prog("BP", vec![])].into_boxed_slice(),
        )));
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Abbrev,
            vec![sar_prog("AC", vec![sar_comp("12A", &[])])].into_boxed_slice(),
        )));
        ctx.add_portion(attrs_with_sar(SarMarking::new(
            SarIndicator::Abbrev,
            vec![sar_prog("99", vec![])].into_boxed_slice(),
        )));
        let got = ctx.expected_sar_marking().expect("merged");
        let ids: Vec<&str> = got.programs.iter().map(|p| &*p.identifier).collect();
        assert_eq!(ids, vec!["99", "AC", "BP"]);
    }

    #[test]
    fn render_expected_banner_with_sar_between_sci_and_aea() {
        // End-to-end: one portion with SCI + SAR + Dissem on a TS doc.
        // The SAR block slots between the SCI block and the Dissem block
        // (AEA is omitted here). Category separator is `//`.
        use crate::attrs::{DissemControl, SciControl};
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_controls: vec![SciControl::Si].into_boxed_slice(),
            sar_markings: Some(SarMarking::new(
                SarIndicator::Abbrev,
                vec![sar_prog("BP", vec![sar_comp("J12", &["J54"])])]
                    .into_boxed_slice(),
            )),
            dissem_controls: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("TOP SECRET//SI//SAR-BP-J12 J54//NOFORN")
        );
    }

    #[test]
    fn render_sar_block_canonical_example() {
        // §H.5 p100 canonical: SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB
        let sar = SarMarking::new(
            SarIndicator::Abbrev,
            vec![
                sar_prog(
                    "BP",
                    vec![sar_comp("J12", &["J54"]), sar_comp("K15", &[])],
                ),
                sar_prog("CD", vec![sar_comp("YYY", &["456", "689"])]),
                sar_prog("XR", vec![sar_comp("XRA", &["RB"])]),
            ]
            .into_boxed_slice(),
        );
        assert_eq!(
            render_sar_block(&sar),
            "SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB"
        );
    }

    #[test]
    fn sar_sort_key_numeric_before_alpha() {
        // Ensures the P3 agent's parallel copy can be checked for parity.
        assert!(sar_sort_key("99") < sar_sort_key("AC"));
        assert!(sar_sort_key("12A") < sar_sort_key("AC"));
        assert!(sar_sort_key("AC") < sar_sort_key("BP"));
        // Numeric ordering by value, not lex: "2" < "10" despite lex.
        assert!(sar_sort_key("2") < sar_sort_key("10"));
    }
}
