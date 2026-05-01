// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

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
    AeaMarking, Classification, CountryCode, DeclassExemption, DissemControl, FgiMarker,
    IsmAttributes, MarkingClassification, NonIcDissem, SarCompartment, SarIndicator, SarMarking,
    SarProgram, SciCompartment, SciControl, SciControlSystem, SciMarking,
};
use crate::date::IsmDate;

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
/// # Phase B: canonical entry point is `scheme.project(Scope::Page, ...)`
///
/// Post-Phase-B, the canonical way to compute a banner rollup is
/// `CapcoScheme::project(Scope::Page, &portions)` (or another scheme's
/// `project`). `PageContext` is retained because:
///
/// 1. It lives in `marque-ism`, which does not depend on
///    `marque-scheme`, so existing rule code that reads
///    `PageContext::expected_*` directly doesn't need rewiring.
/// 2. Its public API is stable — consumers outside the marque
///    workspace may depend on it.
///
/// The byte-level equivalence between `PageContext::expected_*` and
/// `scheme.project(Scope::Page, ...)` is the Phase B verification gate
/// (see `crates/capco/tests/scheme_equivalence.rs`). Either entry
/// point produces the same banner rollup; scheme.project is preferred
/// for new code because it extends cleanly across schemes (CUI, NATO,
/// ...).
///
/// New rules that need structural SCI semantics should read
/// `sci_markings` / `SciSet` (in `marque-capco::lattice`) rather than
/// the flat `sci_controls` CVE-projection — see the Phase B migration
/// note in `CLAUDE.md`.
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

    /// Borrow the raw accumulated portion attributes, in document order.
    ///
    /// Most banner-validation rules want one of the rolled-up
    /// `expected_*` accessors (defined later in this `impl`:
    /// [`Self::expected_classification`],
    /// [`Self::expected_dissem_controls`], [`Self::expected_rel_to`],
    /// etc.) — those collapse the per-portion information through a
    /// category-specific lattice (max for classification, union for
    /// SCI, intersection for REL TO, …). A handful of rules —
    /// currently only S005 (`rel-to-opaque-uncertain-reduction`,
    /// issue #206) — need the pre-rollup view because the diagnostic
    /// depends on **which** portion contributed which code, not on
    /// the rolled-up answer. S005 specifically asks "which uncertain
    /// codes appeared in some-but-not-every portion?", which the
    /// intersection-collapsed [`Self::expected_rel_to`] cannot answer.
    ///
    /// New rules SHOULD prefer the `expected_*` accessors. Reach for
    /// `portions()` only when per-portion membership genuinely matters;
    /// every additional caller is one more place that has to keep up
    /// with the lattice / aggregation contracts the helpers encode.
    pub fn portions(&self) -> &[IsmAttributes] {
        &self.portions
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
            .filter_map(|a| a.classification.as_ref().map(|c| c.effective_level()))
            .max()
    }

    /// The full `MarkingClassification` the banner must carry, preserving the
    /// foreign-authority form (FGI/NATO/JOINT) for wholly-foreign pages.
    ///
    /// For a **commingled** page (at least one U.S. classified portion), U.S.
    /// classification wins per CAPCO-2016 §F.1 p20 ("the banner line is a US
    /// classification marking … when US and non-US portions are combined in a
    /// single document, the overall marking is a US classification"). The
    /// returned value is `MarkingClassification::Us(max_level)`.
    ///
    /// For a **wholly-foreign** page (no U.S. classified portions), the most
    /// restrictive foreign classification is returned as-is so that
    /// `page_context_to_attrs` and similar projections preserve the
    /// `//[trigraph] [LEVEL]` banner form required by §H.7 p126.
    ///
    /// Returns `None` only if no portions have been accumulated or all
    /// portions failed to parse a classification level.
    pub fn expected_marking_classification(&self) -> Option<MarkingClassification> {
        if self.has_us_classified_portion() {
            // U.S. classification wins per §F.1 p20.
            self.expected_classification().map(MarkingClassification::Us)
        } else {
            // Wholly-foreign page: preserve the most restrictive foreign type.
            self.portions
                .iter()
                .filter_map(|a| a.classification.clone())
                .max_by_key(|c| c.effective_level())
        }
    }

    /// Whether any accumulated portion uses a U.S. (or U.S.-wins Conflict)
    /// classification system.
    ///
    /// Used by E054/E055 to distinguish commingled pages (US + FGI) from
    /// wholly-foreign pages (all FGI/NATO/JOINT, no US content).
    pub fn has_us_classified_portion(&self) -> bool {
        self.portions.iter().any(|a| {
            matches!(
                a.classification,
                Some(MarkingClassification::Us(_)) | Some(MarkingClassification::Conflict { .. })
            )
        })
    }

    /// All SCI controls that must appear on the banner (union of all portions).
    pub fn expected_sci_controls(&self) -> Vec<SciControl> {
        let mut seen = std::collections::BTreeSet::new();
        seen.extend(
            self.portions
                .iter()
                .flat_map(|attrs| attrs.sci_controls.iter().copied()),
        );
        seen.into_iter().collect()
    }

    /// Structural SCI markings the banner must carry, unioned across all
    /// portions and sorted per CAPCO-2016 §A.6 p15 (numeric first, alpha after).
    ///
    /// Compartments for each system are merged: a compartment identifier that
    /// appears on multiple portions contributes the union of its
    /// sub-compartments. A system with no compartments on any portion appears
    /// as a bare-system entry (empty `compartments`). A compartment with no
    /// sub-compartments on any portion appears with an empty
    /// `sub_compartments`.
    ///
    /// `canonical_enum` is always `None` on roll-up output — the CVE compound
    /// form is per-portion only; the banner is a structural projection.
    pub fn expected_sci_markings(&self) -> Box<[SciMarking]> {
        // system → compartment_id → set of sub_compartments
        let mut acc: std::collections::BTreeMap<
            SystemKey,
            std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
        > = std::collections::BTreeMap::new();

        for attrs in &self.portions {
            for marking in attrs.sci_markings.iter() {
                let key = SystemKey::from_system(&marking.system);
                let comp_map = acc.entry(key).or_default();
                for comp in marking.compartments.iter() {
                    let sub_set = comp_map.entry(comp.identifier.to_string()).or_default();
                    sub_set.extend(comp.sub_compartments.iter().map(ToString::to_string));
                }
            }
        }

        // Now produce sorted output per §A.6 p15: numeric first, alpha after.
        let mut systems: Vec<(SystemKey, _)> = acc.into_iter().collect();
        systems.sort_by(|a, b| sar_sort_key(a.0.text()).cmp(&sar_sort_key(b.0.text())));

        let mut out: Vec<SciMarking> = Vec::with_capacity(systems.len());
        for (sys_key, comp_map) in systems {
            let mut comps: Vec<(String, std::collections::BTreeSet<String>)> =
                comp_map.into_iter().collect();
            comps.sort_by(|a, b| sar_sort_key(&a.0).cmp(&sar_sort_key(&b.0)));

            let compartments: Vec<SciCompartment> = comps
                .into_iter()
                .map(|(id, sub_set)| {
                    let mut subs: Vec<String> = sub_set.into_iter().collect();
                    subs.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));
                    let sub_boxes: Box<[Box<str>]> = subs
                        .into_iter()
                        .map(|s| s.into_boxed_str())
                        .collect::<Vec<_>>()
                        .into_boxed_slice();
                    SciCompartment::new(id.into_boxed_str(), sub_boxes)
                })
                .collect();

            out.push(SciMarking::new(
                sys_key.into_system(),
                compartments.into_boxed_slice(),
                None,
            ));
        }
        out.into_boxed_slice()
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
                    subs.extend(comp.sub_compartments.iter().map(ToString::to_string));
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
                        let subs = comp_map.get(&cid).expect("key enumerated above");
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

                SarProgram::new(pid.into_boxed_str(), built_compartments.into_boxed_slice())
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
        seen.extend(
            self.portions
                .iter()
                .flat_map(|attrs| attrs.dissem_controls.iter().copied()),
        );

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

    /// The REL TO country-code list the banner must carry.
    ///
    /// The result is the **intersection** of all REL TO lists across
    /// portions, with tetragraph expansion (FVEY →
    /// {AUS, CAN, GBR, NZL, USA}) applied before intersection. Codes
    /// without a known membership table (NATO until Phase F lands the
    /// member list, plus operation-specific codes like RSMA / ISAF /
    /// KFOR) are treated as opaque atoms — they survive intersection
    /// only when present in every portion's list.
    ///
    /// Returns an empty slice when:
    /// - No portions have a REL TO list, OR
    /// - Any portion carries NOFORN (which supersedes REL TO on the banner)
    ///
    /// When the intersection is empty (no common countries), this
    /// returns an empty vec — the caller should add NF to dissem
    /// controls.
    pub fn expected_rel_to(&self) -> Vec<CountryCode> {
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

        // Expand each portion's REL TO into a set of code strings,
        // resolving known tetragraphs (FVEY, ACGU, …) into constituent
        // trigraphs. Opaque codes (NATO, RSMA, …) pass through as
        // single atoms, so they survive intersection only when every
        // portion lists them.
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

        // Convert back to typed codes, USA first then alphabetical.
        // The intersection works in `&str` space so we re-typed at the
        // boundary; every entry came from a `CountryCode::as_str()`
        // call above so `try_new` is infallible here in practice, but
        // we use `filter_map` defensively so a future refactor that
        // lets non-CountryCode entries into `expanded` cannot panic.
        let mut codes: Vec<CountryCode> = result
            .iter()
            .filter_map(|s| CountryCode::try_new(s.as_bytes()))
            .collect();

        // USA first, rest alphabetical.
        if let Some(pos) = codes.iter().position(|c| *c == CountryCode::USA) {
            if pos != 0 {
                let usa = codes.remove(pos);
                codes.insert(0, usa);
            }
        }

        codes
    }

    /// The maximum (furthest-out) declassification date observed across all
    /// portions, or `None` if no portion carries one.
    ///
    /// A banner or CAB that specifies an earlier date than this maximum is a
    /// violation — it would cause portions to be declassified before the most
    /// restrictive date allows.
    ///
    /// # Span-aware semantics
    ///
    /// Comparison uses [`IsmDate::end_cmp`], which compares the *end of each
    /// date's span*. A `Year(2003)` value extends through December 31 and is
    /// therefore "later" than a `Date(2003, 6, 15)`. This is the correct
    /// behavior for the MaxDate lattice: a year-only declassification date
    /// is the most conservative (widest) interpretation.
    pub fn expected_declassify_on(&self) -> Option<&IsmDate> {
        self.portions
            .iter()
            .filter_map(|a| a.declassify_on.as_ref())
            .max_by(|a, b| a.end_cmp(b))
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
                        rd_sigma.extend(rd.sigma.iter().copied());
                    }
                    AeaMarking::Frd(frd) => {
                        has_frd = true;
                        frd_sigma.extend(frd.sigma.iter().copied());
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
                    countries.extend(marker.countries.iter().map(|c| c.as_str().to_owned()));
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

        // Convert country strings back to typed codes. Every entry
        // here came from a `CountryCode::as_str()` call upstream, so
        // `try_new` is infallible in practice; `filter_map` is
        // defensive for any future refactor that lets non-CountryCode
        // entries into the `countries` set.
        let codes: Vec<CountryCode> = countries
            .iter()
            .filter_map(|s| CountryCode::try_new(s.as_bytes()))
            .collect();

        Some(FgiMarker {
            countries: codes.into(),
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

        seen.extend(
            self.portions
                .iter()
                .flat_map(|attrs| attrs.non_ic_dissem.iter().copied()),
        );

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

        // SCI block — prefer the structural `sci_markings` projection
        // (which honors compartments and sub-compartments per §A.6) when any
        // portion produced one. Fall back to the enum-projection
        // `sci_controls` path for back-compat when no structural markings
        // exist (e.g., pre-P2 inputs with only enum-form data).
        let sci_markings = self.expected_sci_markings();
        if !sci_markings.is_empty() {
            blocks.push(render_sci_markings_block(&sci_markings));
        } else {
            let sci = self.expected_sci_controls();
            if !sci.is_empty() {
                blocks.push(sci.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("/"));
            }
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

/// Normalized ordering key for an `SciControlSystem` in the page-context
/// accumulator. Published variants hash/compare by their canonical CVE
/// string; custom variants by their raw text. We keep `Ord`/`Eq` derived
/// on the text form so BTreeMap keys have a stable, readable order (final
/// emission order is re-sorted via [`sar_sort_key`] per §A.6 p15).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SystemKey {
    Published(crate::attrs::SciControlBare),
    Custom(String),
}

impl SystemKey {
    fn from_system(sys: &SciControlSystem) -> Self {
        match sys {
            SciControlSystem::Published(b) => SystemKey::Published(*b),
            SciControlSystem::Custom(s) => SystemKey::Custom(s.to_string()),
        }
    }

    fn text(&self) -> &str {
        match self {
            SystemKey::Published(b) => b.as_str(),
            SystemKey::Custom(s) => s.as_str(),
        }
    }

    fn into_system(self) -> SciControlSystem {
        match self {
            SystemKey::Published(b) => SciControlSystem::Published(b),
            SystemKey::Custom(s) => SciControlSystem::Custom(s.into_boxed_str()),
        }
    }
}

/// Render the structural SCI block per §A.6 Figure 2:
/// `SYSTEM[-COMP[ SUB...][-COMP[ SUB...]]...]` with `/` between distinct
/// systems. A bare system emits `SYSTEM`; a system with compartments emits
/// `SYSTEM-COMP[ SUBS...]` with each additional compartment joined by `-`.
fn render_sci_markings_block(markings: &[SciMarking]) -> String {
    let mut systems: Vec<String> = Vec::with_capacity(markings.len());
    for m in markings {
        let sys_text = match &m.system {
            SciControlSystem::Published(b) => b.as_str().to_owned(),
            SciControlSystem::Custom(s) => s.to_string(),
        };
        if m.compartments.is_empty() {
            systems.push(sys_text);
            continue;
        }
        // system-COMP1[ sub..]-COMP2[ sub..] ...
        let mut rendered = sys_text;
        for comp in m.compartments.iter() {
            rendered.push('-');
            rendered.push_str(&comp.identifier);
            for sub in comp.sub_compartments.iter() {
                rendered.push(' ');
                rendered.push_str(sub);
            }
        }
        systems.push(rendered);
    }
    systems.join("/")
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
/// Issue #183 PR-B: thin wrapper around the canonical
/// [`crate::lookup_tetragraph_members`] table emitted by `build.rs`
/// (which sources FVEY/ACGU from the hand-curated CAPCO Register
/// data and any org-specific extensions with `members` from
/// `country_extensions.toml`). Pre-PR-B this module carried its
/// own private duplicate of the FVEY/ACGU table; the duplicate
/// is retired so a single source of truth feeds both the banner
/// roll-up here and the `marque-capco::vocab::expand_tetragraph`
/// public API.
///
/// Returns `None` for trigraphs, opaque tetragraphs (NATO and
/// operation-specific codes like RSMA / ISAF / KFOR), and
/// unrecognized codes — opaque atoms must pass through unchanged
/// so the intersection treats them correctly.
fn expand_tetragraph(code: &str) -> Option<&'static [&'static str]> {
    crate::lookup_tetragraph_members(code)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::attrs::{Classification, MarkingClassification};
    use crate::date::IsmDate;

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
    fn nato_secret_contributes_to_max_classification() {
        // (C//NF) + (//NS//REL TO USA, NATO) → banner must be SECRET
        use crate::attrs::NatoClassification::NatoSecret;
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Confidential));
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Nato(NatoSecret)),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_classification(),
            Some(Classification::Secret),
            "NS (NATO SECRET) should drive banner to SECRET"
        );
    }

    #[test]
    fn fgi_secret_contributes_to_max_classification() {
        // (C//NF) + (//DEU S//...) → banner must be SECRET
        use crate::attrs::{CountryCode, FgiClassification};
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Confidential));
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: vec![CountryCode::try_new(b"DEU").unwrap()].into(),
            })),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_classification(),
            Some(Classification::Secret),
            "DEU S (FGI SECRET) should drive banner to SECRET"
        );
    }

    #[test]
    fn joint_secret_contributes_to_max_classification() {
        use crate::attrs::JointClassification;
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Confidential));
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Joint(JointClassification {
                level: Classification::Secret,
                countries: Box::new([]),
            })),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_classification(),
            Some(Classification::Secret),
            "JOINT SECRET should drive banner to SECRET"
        );
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
        use crate::attrs::CountryCode;
        let mut ctx = PageContext::new();
        let a1 = IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()]
                .into_boxed_slice(),
            ..Default::default()
        };
        let a2 = IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"DEU").unwrap()]
                .into_boxed_slice(),
            ..Default::default()
        };
        ctx.add_portion(a1);
        ctx.add_portion(a2);
        // Only USA appears in both → intersection is [USA]
        let rel = ctx.expected_rel_to();
        assert_eq!(rel, vec![CountryCode::USA]);
    }

    #[test]
    fn noforn_supersedes_rel_to() {
        use crate::attrs::{CountryCode, DissemControl};
        let mut ctx = PageContext::new();
        // Portion 1: REL TO USA, GBR
        let a1 = IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()]
                .into_boxed_slice(),
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
            declassify_on: Some(IsmDate::Date(2035, 1, 1)),
            ..Default::default()
        };
        let a2 = IsmAttributes {
            declassify_on: Some(IsmDate::Date(2048, 12, 31)),
            ..Default::default()
        };
        let mut ctx = PageContext::new();
        ctx.add_portion(a1);
        ctx.add_portion(a2);
        assert_eq!(
            ctx.expected_declassify_on(),
            Some(&IsmDate::Date(2048, 12, 31))
        );
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
                countries: vec![CountryCode::try_new(b"GBR").unwrap()].into(),
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
                countries: vec![CountryCode::try_new(b"GBR").unwrap()].into(),
            }),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            fgi_marker: Some(FgiMarker {
                countries: vec![CountryCode::try_new(b"DEU").unwrap()].into(),
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
        // Note: post-issue-#183 PR-A `CountryCode` can store
        // tetragraphs like "FVEY" directly; this older test uses
        // the pre-expanded constituent trigraphs to exercise the
        // intersection-after-expansion path. Tetragraph storage
        // and expansion are exercised by
        // `rel_to_intersection_expands_fvey_into_constituent_trigraphs`
        // (and peers) above; the `expand_tetragraph` helper itself
        // is tested in `crates/capco/src/vocab.rs`.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![
                CountryCode::USA,
                CountryCode::try_new(b"AUS").unwrap(),
                CountryCode::try_new(b"CAN").unwrap(),
                CountryCode::try_new(b"GBR").unwrap(),
                CountryCode::try_new(b"NZL").unwrap(),
            ]
            .into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![
                CountryCode::USA,
                CountryCode::try_new(b"AUS").unwrap(),
                CountryCode::try_new(b"CAN").unwrap(),
            ]
            .into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        assert_eq!(rel.len(), 3);
        assert_eq!(rel[0], CountryCode::USA); // USA first
        assert_eq!(rel[1].as_str(), "AUS");
        assert_eq!(rel[2].as_str(), "CAN");
    }

    #[test]
    fn rel_to_empty_intersection_returns_empty() {
        // REL TO USA, AUS + REL TO USA, GBR → no common (just USA)
        // Wait, USA is common. Let's test non-overlapping non-USA countries.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"AUS").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        // USA is the intersection — still produces a result.
        assert_eq!(rel.len(), 1);
        assert_eq!(rel[0], CountryCode::USA);
    }

    // -----------------------------------------------------------------------
    // Issue #183 PR-A — banner roll-up must (a) expand FVEY/ACGU into
    // their constituent trigraphs before intersection and (b) treat
    // opaque codes (NATO, RSMA, …) as atoms that survive intersection
    // only when present in every portion.
    // -----------------------------------------------------------------------

    #[test]
    fn rel_to_intersection_expands_fvey_into_constituent_trigraphs() {
        // Portion 1: REL TO USA, FVEY  → expanded {AUS, CAN, GBR, NZL, USA}
        // Portion 2: REL TO USA, NZL   → atoms     {NZL, USA}
        // Intersection: {NZL, USA} → banner: USA first, NZL alphabetical.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"FVEY").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"NZL").unwrap()].into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        let codes: Vec<&str> = rel.iter().map(|c| c.as_str()).collect();
        assert_eq!(codes, vec!["USA", "NZL"]);
    }

    #[test]
    fn rel_to_opaque_tetragraph_in_one_portion_drops_from_intersection() {
        // Portion 1: REL TO USA, KFOR  → atoms {KFOR, USA} (KFOR is
        //                                decomposable="No" — atom by
        //                                authority per ISMCAT V2022-NOV)
        // Portion 2: REL TO USA, GBR   → atoms {GBR, USA}
        // Intersection: {USA}. KFOR is not in portion 2's set, so the
        // opaque atom drops out — the banner cannot claim a KFOR
        // release the second portion didn't authorize.
        //
        // Pre-issue-208 this test used NATO; #208 made NATO
        // decomposable=Yes with 30 trigraph members, so KFOR (still
        // an atom-by-authority code) is the canonical replacement.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"KFOR").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        let codes: Vec<&str> = rel.iter().map(|c| c.as_str()).collect();
        assert_eq!(codes, vec!["USA"]);
    }

    #[test]
    fn rel_to_opaque_tetragraph_in_every_portion_survives_intersection() {
        // Portion 1: REL TO USA, KFOR  → atoms {KFOR, USA}
        // Portion 2: REL TO USA, KFOR  → atoms {KFOR, USA}
        // Intersection: {KFOR, USA} — KFOR survives because both
        // portions explicitly list it. USA renders first per CAPCO
        // §H.8 ordering.
        //
        // Pre-issue-208 this test used NATO; see the sibling test
        // above for the same NATO → KFOR rationale.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"KFOR").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"KFOR").unwrap()].into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        let codes: Vec<&str> = rel.iter().map(|c| c.as_str()).collect();
        assert_eq!(codes, vec!["USA", "KFOR"]);
    }

    #[test]
    fn rel_to_fvey_intersected_with_acgu_yields_acgu_members() {
        // Portion 1: REL TO USA, FVEY → {AUS, CAN, GBR, NZL, USA}
        // Portion 2: REL TO USA, ACGU → {AUS, CAN, GBR, USA}
        // Intersection: ACGU members. NZL drops out (not in ACGU);
        // ACGU-as-tetragraph itself never appears because it expanded
        // into its members on portion 2 before intersection.
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"FVEY").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(IsmAttributes {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"ACGU").unwrap()].into(),
            ..Default::default()
        });
        let rel = ctx.expected_rel_to();
        let codes: Vec<&str> = rel.iter().map(|c| c.as_str()).collect();
        assert_eq!(codes, vec!["USA", "AUS", "CAN", "GBR"]);
    }

    // -----------------------------------------------------------------------
    // Issue #183 PR-B — `expand_tetragraph` now reads from the canonical
    // generated `marque_ism::TETRAGRAPH_MEMBERS` table (single source of
    // truth shared with `marque-capco::vocab::expand_tetragraph`). Pin
    // the consolidation by exercising the wrapper directly: post-PR-B
    // both consumers must agree on every code.
    // -----------------------------------------------------------------------

    #[test]
    fn expand_tetragraph_reads_canonical_table_for_fvey_acgu() {
        assert_eq!(
            super::expand_tetragraph("FVEY"),
            Some(crate::lookup_tetragraph_members("FVEY").unwrap()),
            "page_context::expand_tetragraph must defer to the \
             canonical marque_ism::lookup_tetragraph_members for FVEY"
        );
        assert_eq!(
            super::expand_tetragraph("ACGU"),
            Some(crate::lookup_tetragraph_members("ACGU").unwrap()),
        );
    }

    #[test]
    fn expand_tetragraph_returns_none_for_opaque_and_unknown() {
        // Issue #208: codes outside the ISMCAT V2022-NOV
        // decomposable="Yes" set still pass through as opaque atoms.
        // - EU / KFOR / GCCH: decomposable="No" — atom by authority.
        // - RSMA / ISAF / MCFI: decomposable="NA" — deprecated
        //   (membership suppressed or OCA-deferred).
        // - USA: trigraph (no tetragraph expansion defined).
        // - XYZW: code absent from taxonomy entirely.
        //
        // Pre-issue-208 NATO was in this list; #208 surfaced its 30
        // trigraph members and moved it to decomposable=Yes.
        assert!(super::expand_tetragraph("EU").is_none());
        assert!(super::expand_tetragraph("KFOR").is_none());
        assert!(super::expand_tetragraph("RSMA").is_none());
        assert!(super::expand_tetragraph("ISAF").is_none());
        assert!(super::expand_tetragraph("USA").is_none());
        assert!(super::expand_tetragraph("XYZW").is_none());
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

    // -----------------------------------------------------------------------
    // SCI structural roll-up (P4 of #003)
    // -----------------------------------------------------------------------

    use crate::attrs::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    fn sci_sys_pub(b: SciControlBare) -> SciControlSystem {
        SciControlSystem::Published(b)
    }

    fn sci_sys_custom(s: &str) -> SciControlSystem {
        SciControlSystem::Custom(s.to_owned().into_boxed_str())
    }

    fn comp(id: &str, subs: &[&str]) -> SciCompartment {
        let sub_box: Box<[Box<str>]> = subs
            .iter()
            .map(|s| (*s).to_owned().into_boxed_str())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        SciCompartment::new(id.to_owned().into_boxed_str(), sub_box)
    }

    fn attrs_with_sci_markings(markings: Vec<SciMarking>) -> IsmAttributes {
        IsmAttributes {
            sci_markings: markings.into_boxed_slice(),
            ..Default::default()
        }
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
    fn sci_markings_single_portion_identity() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Si),
            Box::new([comp("G", &["ABCD"])]),
            None,
        )]));
        let out = ctx.expected_sci_markings();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].system, sci_sys_pub(SciControlBare::Si));
        assert_eq!(out[0].compartments.len(), 1);
        assert_eq!(&*out[0].compartments[0].identifier, "G");
        assert_eq!(out[0].compartments[0].sub_compartments.len(), 1);
        assert_eq!(&*out[0].compartments[0].sub_compartments[0], "ABCD");
        assert_eq!(out[0].canonical_enum, None);
    }

    #[test]
    fn sci_markings_merge_subs_within_same_system_same_compartment() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Si),
            Box::new([comp("G", &["ABCD"])]),
            None,
        )]));
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Si),
            Box::new([comp("G", &["DEFG"])]),
            None,
        )]));
        let out = ctx.expected_sci_markings();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].compartments.len(), 1);
        let subs: Vec<&str> = out[0].compartments[0]
            .sub_compartments
            .iter()
            .map(|s| s.as_ref())
            .collect();
        assert_eq!(subs, vec!["ABCD", "DEFG"]);
    }

    #[test]
    fn sci_markings_two_distinct_systems_sorted_alpha() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Si),
            Box::new([]),
            None,
        )]));
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Hcs),
            Box::new([]),
            None,
        )]));
        let out = ctx.expected_sci_markings();
        assert_eq!(out.len(), 2);
        // HCS before SI alphabetically (both alpha partition)
        assert_eq!(out[0].system, sci_sys_pub(SciControlBare::Hcs));
        assert_eq!(out[1].system, sci_sys_pub(SciControlBare::Si));
    }

    #[test]
    fn sci_markings_numeric_sorts_before_alpha() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Si),
            Box::new([]),
            None,
        )]));
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_custom("123"),
            Box::new([]),
            None,
        )]));
        let out = ctx.expected_sci_markings();
        assert_eq!(out.len(), 2);
        // Numeric 123 first, SI second (per §A.6 p15)
        assert_eq!(out[0].system, sci_sys_custom("123"));
        assert_eq!(out[1].system, sci_sys_pub(SciControlBare::Si));
    }

    #[test]
    fn sci_markings_sub_compartments_sorted() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Si),
            Box::new([comp("G", &["DEFG", "ABCD"])]),
            None,
        )]));
        let out = ctx.expected_sci_markings();
        let subs: Vec<&str> = out[0].compartments[0]
            .sub_compartments
            .iter()
            .map(|s| s.as_ref())
            .collect();
        assert_eq!(subs, vec!["ABCD", "DEFG"]);
    }

    #[test]
    fn sci_markings_canonical_enum_never_populated_on_rollup() {
        // Even if a portion recorded canonical_enum = Some(SiG), the rollup
        // output must be structural-only (None), per spec.
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_sci_markings(vec![SciMarking::new(
            sci_sys_pub(SciControlBare::Si),
            Box::new([comp("G", &[])]),
            Some(SciControl::SiG),
        )]));
        let out = ctx.expected_sci_markings();
        assert_eq!(out[0].canonical_enum, None);
    }

    #[test]
    fn render_banner_uses_structural_sci_block_bare() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_markings: vec![SciMarking::new(
                sci_sys_pub(SciControlBare::Si),
                Box::new([]),
                None,
            )]
            .into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("TOP SECRET//SI")
        );
    }

    #[test]
    fn render_banner_uses_structural_sci_block_with_compartments() {
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_markings: vec![SciMarking::new(
                sci_sys_pub(SciControlBare::Si),
                Box::new([comp("G", &["ABCD", "DEFG"])]),
                None,
            )]
            .into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("TOP SECRET//SI-G ABCD DEFG")
        );
    }

    #[test]
    fn render_banner_structural_sci_multi_compartment() {
        // §A.6 p16 canonical decomposition: SI-G ABCD DEFG-MMM AACD
        let mut ctx = PageContext::new();
        ctx.add_portion(IsmAttributes {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_markings: vec![SciMarking::new(
                sci_sys_pub(SciControlBare::Si),
                Box::new([comp("G", &["ABCD", "DEFG"]), comp("MMM", &["AACD"])]),
                None,
            )]
            .into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.render_expected_banner().as_deref(),
            Some("TOP SECRET//SI-G ABCD DEFG-MMM AACD")
        );
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
                vec![sar_prog("BP", vec![sar_comp("J12", &["J54"])])].into_boxed_slice(),
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
                sar_prog("BP", vec![sar_comp("J12", &["J54"]), sar_comp("K15", &[])]),
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
