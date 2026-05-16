// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Page-level aggregation context for deriving expected banner markings.
//!
//! In CAPCO, a banner marking must reflect the **most restrictive union** of all
//! portion markings on a given page (or, for non-paginated material, across all
//! portions in the logical unit). This module provides [`PageContext`], which
//! accumulates portion [`CanonicalAttrs`] as the engine processes candidates and
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
//! | `dissem_us`            | union (per namespace)    | Any US-attributed restriction on any portion applies |
//! | `dissem_nato`          | union (per namespace)    | Any NATO-attributed restriction on any portion applies |
//! | `rel_to`               | intersection (all-or-none) | Page is releasable only to countries that |
//! |                        |                          | appear in *every* portion's REL TO list     |
//! | `display_only_to`      | cross-axis intersection  | Banner DO = intersection of (REL TO ∪ DO)  |
//! |                        |                          | across all portions, minus banner REL TO   |
//! | `declassify_on`        | max date (furthest out)  | Most conservative declassification applies  |
//! | `declass_exemption`    | most specific            | Exemption with longest default duration     |
//!
//! ## REL TO intersection note
//! A portion marked `REL TO USA, GBR` is accessible to GBR. A different portion
//! marked `REL TO USA, DEU` is accessible to DEU. The page as a whole is only
//! accessible to countries that can see **all** portions — the intersection
//! (typically USA only when mixed REL TO lists are present). A portion that
//! carries no REL TO axis at all clears banner REL TO entirely per CAPCO-2016
//! §D.2 Table 3 row 16 (REL TO plus portion w/o FD&R → NOFORN) and row 26
//! (REL TO plus DO-only portion → DO banner, no REL TO banner). If one
//! portion has `NOFORN`, that dissem control supersedes REL TO on the banner.
//!
//! ## DISPLAY ONLY cross-axis note
//! Per §D.2 Table 3 row 26 (Note): "if information is approved for release to
//! a given audience it has automatically been approved for disclosure to that
//! audience." So each portion's *display-permission* set is
//! `REL TO ∪ DISPLAY ONLY` (release subsumes disclosure). Banner DO is the
//! intersection of display-permission across all portions, minus countries
//! already covered by banner REL TO (row 27: when both axes carry the same
//! country, REL TO is the stricter axis and DO does not repeat it) and minus
//! USA (the originator — the §H.8 p163 worked examples never list USA in the
//! DO axis). If any portion lacks both REL TO and DO, banner DO is empty and
//! the page falls into NOFORN per rows 19/20.
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
    MarkingClassification, NonIcDissem, SarCompartment, SarIndicator, SarMarking, SarProgram,
    SciCompartment, SciControl, SciControlSystem, SciMarking,
};
use crate::canonical::CanonicalAttrs;
use crate::date::IsmDate;
use crate::projected::{ProjectedMarking, ProjectionProvenance};
use marque_scheme::Scope;
use smol_str::SmolStr;

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
#[derive(Debug)]
pub struct PageContext {
    /// Accumulated portion attributes, in document order. Pre-sized to 8
    /// because the typical CAPCO document carries 1-10 portions per page
    /// (the scanner emits PageBreak candidates at form-feed and `\n\n\n+`
    /// runs, slicing larger docs into multiple per-page contexts). 8
    /// covers the typical case in zero reallocations; larger pages
    /// pay only the reallocations needed past 8 instead of the early
    /// growth sequence a `Vec::new()` path would incur on the first
    /// several pushes (the exact stdlib growth schedule is an
    /// implementation detail, but pre-sizing eliminates it for the
    /// 1-10 portion range). Issue #430.
    ///
    /// The pre-size flows through `Default` AND `Clone` — see the manual
    /// impls below. Derived `Clone` would call `Vec::clone()`, which
    /// strips capacity to `len()` (the engine clones at
    /// `engine.rs:1025` via `Arc::new(page_context.clone())` when
    /// handing the page roll-up to banner/CAB rules); the manual impl
    /// preserves the invariant "every `PageContext` has at least
    /// `DEFAULT_PORTIONS_CAPACITY` headroom" through every code path.
    portions: Vec<CanonicalAttrs>,
}

/// Default capacity for `PageContext.portions`. Sized to the typical
/// CAPCO per-page portion count; see field doc on `PageContext::portions`
/// for the rationale (issue #430).
const DEFAULT_PORTIONS_CAPACITY: usize = 8;

impl Default for PageContext {
    fn default() -> Self {
        Self {
            portions: Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY),
        }
    }
}

impl Clone for PageContext {
    fn clone(&self) -> Self {
        // Derived `Clone` would forward to `Vec::clone()` which sizes
        // the new buffer to `self.portions.len()`, stripping the
        // pre-size on every clone. The engine clones at
        // `engine.rs:1025` to wrap in `Arc<PageContext>` for the
        // banner/CAB rule hand-off; without this impl, that path would
        // silently undo the pre-size for any page with fewer than
        // `DEFAULT_PORTIONS_CAPACITY` portions accumulated so far.
        //
        // Sizing uses `len().max(DEFAULT_PORTIONS_CAPACITY)` rather than
        // `self.portions.capacity()` so a portion-heavy source ctx
        // doesn't amplify into a clone with up to 2× wasted slack from
        // the source Vec's last growth step (a 33-portion page would
        // sit at capacity 64; cloning at that capacity wastes ~31 slots
        // × ~300B of `CanonicalAttrs` per slot in the Arc'd snapshot).
        // The cloned ctx is treated as read-only by the banner/CAB
        // hand-off; future pushes are not the cloned ctx's concern.
        // Issue #430.
        let cap = self.portions.len().max(DEFAULT_PORTIONS_CAPACITY);
        let mut portions = Vec::with_capacity(cap);
        portions.extend(self.portions.iter().cloned());
        Self { portions }
    }
}

impl PageContext {
    /// Create an empty context (no portions seen yet).
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a newly-parsed portion marking. Must be called in document order
    /// before banner rules are checked.
    pub fn add_portion(&mut self, attrs: CanonicalAttrs) {
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

    /// Materialize the current accumulator as a [`ProjectedMarking`]
    /// under [`Scope::Page`]. Wired by PR 9b (T133 / FR-006) for the
    /// engine's `RuleContext::page_marking` field, so banner-validation
    /// rules can consume the rolled-up shape directly without going
    /// through the `expected_*` accessor surface.
    ///
    /// Mirrors the field-by-field composition `page_context_to_attrs`
    /// performs for `CanonicalAttrs`, but emits the engine-facing
    /// projection type instead. The page-rewrite layer (CAPCO's
    /// `capco/noforn-clears-rel-to` etc.) is NOT applied here —
    /// `PageContext` does not have access to a scheme's
    /// `page_rewrites` table; the engine applies those by going
    /// through `MarkingScheme::project` separately when it needs the
    /// post-rewrite form. PR 9b consumers reading `page_marking`
    /// today only need the pre-rewrite shape (banner-validation rules
    /// inspecting the union of portion-contributed dissems / SCI /
    /// REL TO etc.); a future migration that needs the post-rewrite
    /// form should plumb that separately.
    ///
    /// **Known gap (PR 5 / FR-007)**: the `classification` field below
    /// wraps the rolled-up US classification level as
    /// `MarkingClassification::Us(_)` unconditionally. This contradicts
    /// `ProjectedMarking::classification`'s documented invariant that
    /// pure-foreign pages should project as `None` (no US contribution)
    /// or as the foreign variant. FGI/NATO classification provenance is
    /// currently lost at projection time. No banner rule reads
    /// `page.classification` directly today, so the gap is latent. Full
    /// `MarkingClassification` roll-up — including a parallel
    /// `expected_classification_full()` that distinguishes US / NATO /
    /// FGI provenance — lands in PR 5 (see `research.md` / FR-007).
    /// Read sites MUST tolerate this gap until then.
    pub fn project(&self) -> ProjectedMarking {
        ProjectedMarking {
            scope: Scope::Page,
            // TODO(PR5/FR-007): classification narrowed to Us(_) here; FGI/NATO
            // provenance is currently lost. Full MarkingClassification roll-up
            // lands when PR 5 wires `expected_classification_full()` (see
            // research.md / FR-007). Read sites must tolerate this gap until then.
            classification: self
                .expected_classification()
                .map(MarkingClassification::Us),
            sci_controls: self.expected_sci_controls().into_boxed_slice(),
            sci_markings: self.expected_sci_markings(),
            sar_markings: self.expected_sar_marking(),
            aea_markings: self.expected_aea_markings().into_boxed_slice(),
            fgi_marker: self.expected_fgi_marker(),
            dissem_us: self.expected_dissem_us().into_boxed_slice(),
            dissem_nato: self.expected_dissem_nato().into_boxed_slice(),
            non_ic_dissem: self.expected_non_ic_dissem().0.into_boxed_slice(),
            rel_to: self.expected_rel_to().into_boxed_slice(),
            // DISPLAY ONLY axis roll-up per §D.2 Table 3 rows 18-20 +
            // 25-27 — cross-axis intersection over (REL TO ∪ DO) with
            // banner-REL-TO and USA subtraction. See
            // [`Self::expected_display_only`].
            display_only_to: self.expected_display_only().into_boxed_slice(),
            declassify_on: self.expected_declassify_on().cloned(),
            provenance: ProjectionProvenance::default(),
        }
    }

    /// Borrow the raw accumulated portion attributes, in document order.
    ///
    /// Most banner-validation rules want one of the rolled-up
    /// `expected_*` accessors (defined later in this `impl`:
    /// [`Self::expected_classification`],
    /// [`Self::expected_dissem_us`], [`Self::expected_dissem_nato`],
    /// [`Self::expected_rel_to`],
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
    pub fn portions(&self) -> &[CanonicalAttrs] {
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

    /// Returns `true` when **every** accumulated portion carries a NATO
    /// classification axis ([`MarkingClassification::Nato`]) and has no
    /// populated `fgi_marker`. Empty-accumulator returns `false`.
    ///
    /// # Authority
    ///
    /// CAPCO-2016 §H.7 p127 Notional Example 2 worked example —
    /// `(//CTS//BOHEMIA//REL TO USA, NATO)` is the canonical form for a
    /// bare-NATO portion in a *US-classified* document. By extension, a
    /// document whose portions are *solely* NATO does not need a NATO
    /// portion to carry an explicit `REL TO USA, NATO` block — alliance
    /// ownership is implicit.
    ///
    /// # Predicate
    ///
    /// 1. `!self.portions.is_empty()` — an empty accumulator is **not**
    ///    "solely NATO". Without this guard the
    ///    [`Iterator::all`] short-circuit would return `true` on the
    ///    empty set, causing S007 (the current consumer) to wrongly
    ///    suppress on a freshly-reset page or before any portion has
    ///    been observed.
    /// 2. Each portion has `classification` matching
    ///    [`MarkingClassification::Nato`]`(_)`. US, FGI, JOINT, or
    ///    Conflict classifications disqualify the page.
    /// 3. Each portion has `fgi_marker.is_none()`. A populated
    ///    [`FgiMarker`] elevates the portion out of "pure NATO"
    ///    status — NATO commingled with FGI is a *commingled-NATO*
    ///    document, distinct from the pure-NATO case the §H.7 p127
    ///    worked example endorses. (Project memory:
    ///    `project_nato_transmutes_to_fgi.md` — NATO transmutes to
    ///    FGI when commingled.)
    ///
    /// # Current consumer
    ///
    /// Rule **S007** (`bare-nato-requires-rel-to-usa-nato`) reads this
    /// predicate to silence the bare-NATO → `REL TO USA, NATO`
    /// suggestion in solely-NATO documents. This helper lives on
    /// [`PageContext`] rather than as a free function because it is
    /// derived from the accumulator's existing per-portion state — no
    /// new accumulator field is required.
    pub fn is_solely_nato_classified(&self) -> bool {
        !self.portions.is_empty()
            && self.portions.iter().all(|a| {
                matches!(&a.classification, Some(MarkingClassification::Nato(_)))
                    && a.fgi_marker.is_none()
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
            std::collections::BTreeMap<SmolStr, std::collections::BTreeSet<SmolStr>>,
        > = std::collections::BTreeMap::new();

        for attrs in &self.portions {
            for marking in attrs.sci_markings.iter() {
                let key = SystemKey::from_system(&marking.system);
                let comp_map = acc.entry(key).or_default();
                for comp in marking.compartments.iter() {
                    let sub_set = comp_map.entry(comp.identifier.clone()).or_default();
                    sub_set.extend(comp.sub_compartments.iter().cloned());
                }
            }
        }

        // Now produce sorted output per §A.6 p15: numeric first, alpha after.
        let mut systems: Vec<(SystemKey, _)> = acc.into_iter().collect();
        systems.sort_by(|a, b| sar_sort_key(a.0.text()).cmp(&sar_sort_key(b.0.text())));

        let mut out: Vec<SciMarking> = Vec::with_capacity(systems.len());
        for (sys_key, comp_map) in systems {
            let mut comps: Vec<(SmolStr, std::collections::BTreeSet<SmolStr>)> =
                comp_map.into_iter().collect();
            comps.sort_by(|a, b| sar_sort_key(&a.0).cmp(&sar_sort_key(&b.0)));

            let compartments: Vec<SciCompartment> = comps
                .into_iter()
                .map(|(id, sub_set)| {
                    let mut subs: Vec<SmolStr> = sub_set.into_iter().collect();
                    subs.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));
                    let sub_boxes: Box<[SmolStr]> = subs.into_boxed_slice();
                    SciCompartment::new(id, sub_boxes)
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
        let mut programs: BTreeMap<SmolStr, BTreeMap<SmolStr, BTreeSet<SmolStr>>> = BTreeMap::new();

        for attrs in &self.portions {
            let Some(sar) = attrs.sar_markings.as_ref() else {
                continue;
            };
            for prog in sar.programs.iter() {
                let comps = programs.entry(prog.identifier.clone()).or_default();
                for comp in prog.compartments.iter() {
                    let subs = comps.entry(comp.identifier.clone()).or_default();
                    subs.extend(comp.sub_compartments.iter().cloned());
                }
            }
        }

        if programs.is_empty() {
            return None;
        }

        // Sort each hierarchical level per CAPCO §H.5 (numeric-first, then alpha).
        let mut prog_keys: Vec<SmolStr> = programs.keys().cloned().collect();
        prog_keys.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));

        let built_programs: Vec<SarProgram> = prog_keys
            .into_iter()
            .map(|pid| {
                let comp_map = programs.remove(&pid).expect("key enumerated above");
                let mut comp_keys: Vec<SmolStr> = comp_map.keys().cloned().collect();
                comp_keys.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));

                let built_compartments: Vec<SarCompartment> = comp_keys
                    .into_iter()
                    .map(|cid| {
                        let subs = comp_map.get(&cid).expect("key enumerated above");
                        let mut sub_vec: Vec<SmolStr> = subs.iter().cloned().collect();
                        sub_vec.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));
                        let boxed: Box<[SmolStr]> = sub_vec.into_boxed_slice();
                        SarCompartment::new(cid, boxed)
                    })
                    .collect();

                SarProgram::new(pid, built_compartments.into_boxed_slice())
            })
            .collect();

        Some(SarMarking::new(
            SarIndicator::Abbrev,
            built_programs.into_boxed_slice(),
        ))
    }

    /// US-attributed dissemination controls that must appear on the
    /// banner.
    ///
    /// Base rule is union over [`crate::CanonicalAttrs::dissem_us`]
    /// across portions, with the following CAPCO-2016 exceptions
    /// per ISM-Rollup XSLT — all of which are US-context behaviors
    /// (none apply to the NATO-attributed channel):
    ///
    /// - **OC-USGOV** (§H.8 p139): Drops if not present on ALL
    ///   OC-carrying portions.
    /// - **FOUO** (§H.8 p134): Drops in classified documents (stays
    ///   in unclassified).
    /// - **DSEN** (§H.8 p159): Overrides FOUO regardless of
    ///   classification level (`DSEN wins over FOUO`).
    /// - **NF injection** (§H.9 p178 / p185): Added when the non-IC
    ///   SBU-NF / LES-NF classified-context split fires
    ///   (`expected_non_ic_dissem` second tuple element).
    ///
    /// **PR 9b (FR-046 / T132).** Replaces the prior single
    /// `expected_dissem_controls()` accessor. NATO-attributed dissems
    /// flow through the sibling [`Self::expected_dissem_nato`].
    pub fn expected_dissem_us(&self) -> Vec<DissemControl> {
        let classified = self.is_classified();

        // Step 1: Basic union of all US-attributed dissem controls.
        let mut seen = std::collections::BTreeSet::new();
        seen.extend(
            self.portions
                .iter()
                .flat_map(|attrs| attrs.dissem_us.iter().copied()),
        );

        // Step 2: OC-USGOV drops if not on ALL OC-carrying portions.
        if seen.contains(&DissemControl::OcUsgov) {
            let oc_portions: Vec<_> = self
                .portions
                .iter()
                .filter(|a| a.dissem_us.contains(&DissemControl::Oc))
                .collect();
            if !oc_portions.is_empty() {
                let all_have_usgov = oc_portions
                    .iter()
                    .all(|a| a.dissem_us.contains(&DissemControl::OcUsgov));
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

        // Step 5: NF injection when FD&R intent is present but both
        // foreign-audience axes clear at the banner level. This catches
        // §D.2 Table 3 rows 9 (REL TO + REL TO no common), 10 (REL TO +
        // RELIDO), 11 (REL TO + DO no common), 16 (REL TO + portion w/o
        // FD&R), 18 (RELIDO + DO), 19 (DO + portion w/o FD&R), and 20
        // (DO + DO no common) — each yields "banner NOFORN" per the
        // Table 3 third column, but the existing NF/needs_nf paths
        // don't fire for these row classes. Per row 1/2: NF appears
        // whenever foreign-audience axes are unable to converge.
        //
        // No mutual-recursion risk: `expected_rel_to` and
        // `expected_display_only` both call `expected_non_ic_dissem`,
        // which does not call back into `expected_dissem_us`.
        let has_fdr_intent = self
            .portions
            .iter()
            .any(|a| !a.rel_to.is_empty() || !a.display_only_to.is_empty());
        if has_fdr_intent
            && self.expected_rel_to().is_empty()
            && self.expected_display_only().is_empty()
        {
            seen.insert(DissemControl::Nf);
        }

        seen.into_iter().collect()
    }

    /// NATO-attributed dissemination controls that must appear on the
    /// banner.
    ///
    /// Plain union over [`crate::CanonicalAttrs::dissem_nato`] across
    /// portions. None of the US-context exceptions (OC-USGOV drop,
    /// FOUO drop, DSEN override, NF injection) apply — those are
    /// §H.8 US-attributed behaviors. NATO contributes only ORCON and
    /// REL TO per CAPCO-2016 p41, both of which compose by simple
    /// union at the banner level.
    ///
    /// **PR 9b (FR-046 / T132).** The split companion to
    /// [`Self::expected_dissem_us`].
    pub fn expected_dissem_nato(&self) -> Vec<DissemControl> {
        let mut seen = std::collections::BTreeSet::new();
        seen.extend(
            self.portions
                .iter()
                .flat_map(|attrs| attrs.dissem_nato.iter().copied()),
        );
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
    /// - **Any** portion has an empty REL TO list (per CAPCO-2016 §D.2
    ///   Table 3 row 16: REL TO + portion w/o FD&R → NOFORN; row 26:
    ///   REL TO + DO-only portion → DO banner only, no REL TO), OR
    /// - Any portion carries NOFORN (which supersedes REL TO on the banner)
    ///
    /// When the intersection is empty (no common countries), this
    /// returns an empty vec — the caller should add NF to dissem
    /// controls.
    pub fn expected_rel_to(&self) -> Vec<CountryCode> {
        // If any portion is NOFORN, NOFORN wins — REL TO is superseded.
        // NOFORN is a US-context dissem (§H.8 p145); the NATO-attributed
        // channel carries only ORCON and REL TO per CAPCO-2016 p41 and
        // contains no NOFORN, so checking `dissem_us` alone is correct
        // by spec. The `dissem_iter()` call below would also work but
        // costs an extra chain link per portion.
        let any_noforn = self
            .portions
            .iter()
            .any(|a| a.dissem_us.iter().any(|d| matches!(d, DissemControl::Nf)));
        if any_noforn {
            return vec![];
        }

        // Also check if NF will be injected from non-IC split.
        let (_, needs_nf) = self.expected_non_ic_dissem();
        if needs_nf {
            return vec![];
        }

        // Row-16 / row-26 strict enforcement: every portion must carry a
        // non-empty REL TO list for the banner to carry REL TO. A
        // portion with no REL TO either has no FD&R (row 16 → NOFORN),
        // carries only DISPLAY ONLY (row 26 → DO banner, no REL TO), or
        // carries only RELIDO (row 10 → NOFORN); all three clear banner
        // REL TO. The empty-accumulator case (no portions at all) also
        // returns empty.
        if self.portions.is_empty() || self.portions.iter().any(|a| a.rel_to.is_empty()) {
            return vec![];
        }

        // Expand each portion's REL TO into a set of code strings,
        // resolving known tetragraphs (FVEY, ACGU, …) into constituent
        // trigraphs. Opaque codes (NATO, RSMA, …) pass through as
        // single atoms, so they survive intersection only when every
        // portion lists them.
        let expanded: Vec<std::collections::BTreeSet<&str>> = self
            .portions
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

    /// The DISPLAY ONLY country-code list the banner must carry.
    ///
    /// Implements CAPCO-2016 §D.2 Table 3 rows 18-20 and 25-27 (the
    /// DISPLAY ONLY axis roll-up rules).
    ///
    /// Each portion's *display-permission* set is
    /// `REL TO ∪ DISPLAY ONLY` — per the row 26 Note, "if information
    /// is approved for release to a given audience it has automatically
    /// been approved for disclosure to that audience." The banner DO
    /// list is the intersection of display-permission across all
    /// portions, with three adjustments:
    ///
    /// - **All-or-nothing gate**: if any portion has empty
    ///   display-permission (no REL TO and no DO), banner DO is empty
    ///   per rows 19/20 (the page falls into NOFORN via the dissem
    ///   layer's existing NF injection).
    /// - **REL TO subtraction**: per row 27 (REL TO/DO commingled →
    ///   REL TO/DO worked example), a country covered by banner REL TO
    ///   does not repeat in banner DO. The DO list reports only the
    ///   foreign audience that has display permission *without* release
    ///   permission.
    /// - **USA stripping**: USA is the originator; inferred from the
    ///   §H.8 p163 worked examples, which never list USA in the DO
    ///   axis (`SECRET//DISPLAY ONLY AFG`, `SECRET//DISPLAY ONLY AFG,
    ///   IRQ`). No explicit prohibition exists in the source, but the
    ///   §H.8 definition ("information that can be disclosed... to the
    ///   foreign country(ies)") implicitly excludes the US originator.
    ///
    /// Returns an empty slice when:
    /// - No portions have been accumulated.
    /// - Any portion carries NOFORN (which supersedes DO on the banner,
    ///   parallel to the REL TO supersession in
    ///   [`Self::expected_rel_to`]).
    /// - Any portion has neither REL TO nor DO (rows 19/20).
    /// - The intersection contains only countries already in banner
    ///   REL TO (row 27 — banner DO is non-empty only when DO covers
    ///   *additional* countries beyond REL TO).
    ///
    /// Tetragraph expansion (FVEY → {AUS, CAN, GBR, NZL, USA})
    /// applies on the way in, mirroring [`Self::expected_rel_to`];
    /// opaque codes (NATO, RSMA, …) pass through as atoms.
    pub fn expected_display_only(&self) -> Vec<CountryCode> {
        if self.portions.is_empty() {
            return vec![];
        }

        // NOFORN supersedes DO (parallel to expected_rel_to). NOFORN is
        // a US-context dissem (§H.8 p145); the NATO-attributed channel
        // carries only ORCON and REL TO per CAPCO-2016 p41, so checking
        // dissem_us alone is correct by spec.
        let any_noforn = self
            .portions
            .iter()
            .any(|a| a.dissem_us.iter().any(|d| matches!(d, DissemControl::Nf)));
        if any_noforn {
            return vec![];
        }

        // NODIS/EXDIS short-circuit. §H.9 p172 (EXDIS) and p174 (NODIS)
        // say "REL TO is not authorized in the banner line. In this
        // case, NOFORN would convey in the banner line." The "NOFORN
        // would convey" half is what reaches DO: once NF is in the
        // banner dissem block, §D.2 Table 3 row 2 (NF + any other FD&R
        // → NOFORN) supersedes both REL TO and DISPLAY ONLY axes. The
        // existing `expected_non_ic_dissem` already returns `needs_nf`
        // for this case; we surface it here as an early-return so the
        // result is byte-identical to `any_noforn` above.
        let (_, needs_nf) = self.expected_non_ic_dissem();
        if needs_nf {
            return vec![];
        }

        // Row-19/20 all-or-nothing gate (row 11 also caught — REL TO +
        // DO with no common country lands in the empty-intersection
        // branch below). Every portion must have a non-empty
        // display-permission set (REL TO ∪ DO).
        let any_empty = self
            .portions
            .iter()
            .any(|a| a.rel_to.is_empty() && a.display_only_to.is_empty());
        if any_empty {
            return vec![];
        }

        // Per-portion display-permission = expand(REL TO) ∪ expand(DO),
        // intersected across all portions. Tetragraph expansion mirrors
        // expected_rel_to for cross-axis consistency (a portion's
        // FVEY REL TO and another portion's explicit AUS DO must
        // intersect to {AUS}).
        let expanded: Vec<std::collections::BTreeSet<&str>> = self
            .portions
            .iter()
            .map(|a| {
                let mut set = std::collections::BTreeSet::new();
                for t in a.rel_to.iter().chain(a.display_only_to.iter()) {
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

        let mut result: std::collections::BTreeSet<&str> = expanded[0].clone();
        for set in &expanded[1..] {
            result = result.intersection(set).copied().collect();
        }

        // Subtract banner REL TO countries (row 27 — REL TO is the
        // stricter axis and DO does not repeat its countries) and USA
        // (originator, inferred from §H.8 p163 worked examples).
        let rel_to = self.expected_rel_to();
        let rel_set: std::collections::BTreeSet<&str> = rel_to.iter().map(|c| c.as_str()).collect();
        result.remove("USA");
        let result: std::collections::BTreeSet<&str> =
            result.difference(&rel_set).copied().collect();

        // Convert back to typed codes, alphabetical (§H.8 p163 — codes
        // listed alphabetically, no USA-first or other privileged
        // ordering).
        let mut codes: Vec<CountryCode> = result
            .iter()
            .filter_map(|s| CountryCode::try_new(s.as_bytes()))
            .collect();
        codes.sort();
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
        let mut has_atomal = false;

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
                    // ATOMAL — NATO §123/§144 sharing (CAPCO-2016 §H.7 p122).
                    // Travels alongside RD/FRD in the AEA axis; rolled up
                    // by presence (no merge state).
                    AeaMarking::Atomal(_) => has_atomal = true,
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

        // ATOMAL rolls up alongside RD/FRD when present. Register order
        // per CAPCO-2016 §H.7 p122 worked example
        // (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`) places ATOMAL after RD
        // and FRD in the AEA axis. The actual render order is owned by
        // the AEA renderer's `register_rank`.
        if has_atomal {
            result.push(AeaMarking::Atomal(crate::attrs::AtomalBlock));
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
                match marker {
                    FgiMarker::SourceConcealed => {
                        has_source_concealed = true;
                    }
                    FgiMarker::Acknowledged {
                        countries: marker_countries,
                    } => {
                        countries.extend(marker_countries.iter().map(|c| c.as_str().to_owned()));
                    }
                }
            }

            // Non-US classification systems contribute to FGI in banner.
            // Note: `MarkingClassification::Fgi` carries `FgiClassification`
            // (a separate type with its own `countries: Box<[CountryCode]>`
            // shape), not `FgiMarker`. The shape-collision retirement for
            // that type is tracked separately; this branch keeps the
            // existing semantics.
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

        // Source-concealed supersedes all open sources (CAPCO §H.7 p122).
        if has_source_concealed {
            return Some(FgiMarker::SourceConcealed);
        }

        // Convert country strings back to typed codes. Every entry
        // here came from a `CountryCode::as_str()` call upstream, so
        // `try_new` is infallible in practice; `filter_map` is
        // defensive for any future refactor that lets non-CountryCode
        // entries into the `countries` set.
        //
        // If every code fails to round-trip (which would imply the
        // upstream string set was corrupt), `acknowledged` returns
        // `None`. We surface that as `None` here too rather than
        // fabricating `SourceConcealed` — a banner with a corrupted
        // country set is not lawful concealment, and the caller should
        // treat it as "no FGI rollup" so the diagnostic surface stays
        // honest.
        let codes = countries
            .iter()
            .filter_map(|s| CountryCode::try_new(s.as_bytes()));
        FgiMarker::acknowledged(codes)
    }

    // -----------------------------------------------------------------------
    // Non-IC dissem rollup
    // -----------------------------------------------------------------------

    /// Expected non-IC dissem controls for the banner.
    ///
    /// Rules (ISM-Rollup XSLT + NonICRollup.xspec, plus §H.9 NODIS/EXDIS):
    /// - Union of all non-IC controls across portions
    /// - **SBU-NF in classified docs**: Splits to SBU + NF (NF goes to dissem)
    /// - **LES-NF in classified docs**: Splits to LES + NF
    /// - In unclassified docs: SBU-NF and LES-NF kept intact
    /// - **NODIS or EXDIS in any portion** (classification-independent):
    ///   sets `needs_nf` so NOFORN is injected at banner roll-up per
    ///   CAPCO-2016 §H.9 p172 (EXDIS) / p174 (NODIS). NODIS and EXDIS
    ///   stay in the non-IC set; they are NOT split.
    ///
    /// Returns a tuple `(non_ic_controls, needs_nf)` where `needs_nf` is
    /// `true` if NF must be added to dissem controls at banner roll-up.
    /// `needs_nf` is set when:
    /// - The SBU-NF / LES-NF classified-context split fires (§H.9
    ///   p178 / p185), OR
    /// - Any portion carries NODIS or EXDIS (§H.9 p172 / p174).
    ///
    /// `needs_nf` does NOT depend on classification level for the
    /// NODIS/EXDIS triggers — those passages do not gate on
    /// classification. The SBU-NF/LES-NF split IS classification-gated
    /// (the split only fires in classified context per the `if classified`
    /// guard inside the function body).
    pub fn expected_non_ic_dissem(&self) -> (Vec<NonIcDissem>, bool) {
        let classified = self.is_classified();
        let mut seen = std::collections::BTreeSet::new();
        let mut needs_nf = false;

        seen.extend(
            self.portions
                .iter()
                .flat_map(|attrs| attrs.non_ic_dissem.iter().copied()),
        );

        if classified {
            // SBU-NF → SBU + NF (dissem)
            if seen.remove(&NonIcDissem::SbuNf) {
                seen.insert(NonIcDissem::Sbu);
                needs_nf = true;
            }
            // LES-NF → LES + NF (dissem)
            if seen.remove(&NonIcDissem::LesNf) {
                seen.insert(NonIcDissem::Les);
                needs_nf = true;
            }
        }

        // NODIS / EXDIS imply NOFORN in the banner per CAPCO-2016 §H.9.
        // Source passages, verbatim:
        //
        //   §H.9 p172 (EXDIS) — "REL TO is not authorized in the banner
        //   line if any portion contains EXDIS information. In this case,
        //   NOFORN would convey in the banner line."
        //
        //   §H.9 p174 (NODIS) — "REL TO is not authorized in the banner
        //   line if any portion contains NODIS information. In this case,
        //   NOFORN would convey in the banner line."
        //
        // NODIS and EXDIS themselves stay in the non-IC dissem set (they
        // roll up to the banner per the non-IC banner-roll-up rule); we
        // only flag that NF must also be injected into CAT_DISSEM. Unlike
        // SBU-NF / LES-NF this is not a split — NODIS/EXDIS tokens are
        // NOT removed or renamed. The flag is purely additive for
        // downstream consumers (the renderer at `render_expected_banner`,
        // the REL TO short-circuit at `expected_rel_to`).
        //
        // Classification-independent for the NODIS/EXDIS triggers — the
        // §H.9 passages above do not gate on classification level. This
        // block is intentionally placed AFTER the `if classified` SBU-NF
        // / LES-NF split block so it runs in both unclassified and
        // classified contexts.
        if seen.contains(&NonIcDissem::Nodis) || seen.contains(&NonIcDissem::Exdis) {
            needs_nf = true;
        }

        (seen.into_iter().collect(), needs_nf)
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
        // category already collected by `expected_dissem_us` + `expected_dissem_nato`.
        // DissemControl::as_str() returns the portion abbreviation ("NF", "RELIDO"),
        // so convert to banner form via marking_forms::portion_to_banner().
        //
        // PR 9b (T132): banner emits the union of US- and NATO-attributed
        // dissems because the wire form is identical (`OC` / `REL TO`
        // tokens are namespace-indistinguishable on the banner line).
        // Render-order is dictated by §A.6 / Register Table 4 row 8;
        // BTreeSet collection at the source ensures the union is
        // duplicate-free.
        let rel_to = self.expected_rel_to();
        let (non_ic, needs_nf) = self.expected_non_ic_dissem();
        let mut dissem_set: std::collections::BTreeSet<DissemControl> =
            std::collections::BTreeSet::new();
        dissem_set.extend(self.expected_dissem_us());
        dissem_set.extend(self.expected_dissem_nato());
        let dissem: Vec<DissemControl> = dissem_set.into_iter().collect();

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
        // If the non-IC dissem family implies NF at banner roll-up — the
        // SBU-NF/LES-NF classified-context split (§H.9 p178 / p185) OR a
        // portion carrying NODIS/EXDIS (§H.9 p172 / p174) — inject NOFORN.
        if needs_nf && !dissem_parts.iter().any(|p| p == "NOFORN") {
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
        // DISPLAY ONLY list (comma-delimited countries with
        // "DISPLAY ONLY " prefix). Joined into the same dissem block as
        // REL TO with `/` per §A.6 p16 within-category separator (the
        // dissem family includes both REL TO and DISPLAY ONLY per
        // Register Table 4 row 8). Banner-form prefix per §H.8 p163:
        // both portion and banner forms write "DISPLAY ONLY" verbatim.
        let display_only = self.expected_display_only();
        if !display_only.is_empty() {
            let codes = display_only
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            dissem_parts.push(format!("DISPLAY ONLY {codes}"));
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
    Custom(SmolStr),
    NatoSap(crate::attrs::NatoSap),
}

impl SystemKey {
    fn from_system(sys: &SciControlSystem) -> Self {
        match sys {
            SciControlSystem::Published(b) => SystemKey::Published(*b),
            SciControlSystem::Custom(s) => SystemKey::Custom(s.clone()),
            SciControlSystem::NatoSap(sap) => SystemKey::NatoSap(*sap),
        }
    }

    fn text(&self) -> &str {
        match self {
            SystemKey::Published(b) => b.as_str(),
            SystemKey::Custom(s) => s.as_str(),
            SystemKey::NatoSap(sap) => sap.as_str(),
        }
    }

    fn into_system(self) -> SciControlSystem {
        match self {
            SystemKey::Published(b) => SciControlSystem::Published(b),
            SystemKey::Custom(s) => SciControlSystem::Custom(s),
            SystemKey::NatoSap(sap) => SciControlSystem::NatoSap(sap),
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
            SciControlSystem::NatoSap(sap) => sap.as_str().to_owned(),
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

    fn attrs_with_classification(c: Classification) -> CanonicalAttrs {
        CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoSecret)),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_classification(),
            Some(Classification::Secret),
            "NS (NATO SECRET) should drive banner to SECRET"
        );
    }

    // -----------------------------------------------------------------
    // is_solely_nato_classified — predicate for S007 (FR-048).
    // CAPCO-2016 §H.7 p127 Notional Example 2 is the authority.
    // -----------------------------------------------------------------

    #[test]
    fn is_solely_nato_classified_empty_is_false() {
        // Empty accumulator returns false: an empty page is not
        // "solely NATO." Without the `!is_empty()` guard, S007 would
        // wrongly suppress on a freshly-reset page.
        let ctx = PageContext::new();
        assert!(!ctx.is_solely_nato_classified());
    }

    #[test]
    fn is_solely_nato_classified_one_bare_nato_portion_is_true() {
        use crate::attrs::NatoClassification::CosmicTopSecret;
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(CosmicTopSecret)),
            ..Default::default()
        });
        assert!(
            ctx.is_solely_nato_classified(),
            "single bare-NATO portion is a solely-NATO page"
        );
    }

    #[test]
    fn is_solely_nato_classified_nato_plus_us_is_false() {
        // §H.7 p127 worked example surface: NATO portion sitting in a
        // US-classified document. The US portion disqualifies the
        // page from "solely NATO" — S007 must fire on the NATO portion.
        use crate::attrs::NatoClassification::NatoSecret;
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoSecret)),
            ..Default::default()
        });
        ctx.add_portion(attrs_with_classification(Classification::Secret));
        assert!(!ctx.is_solely_nato_classified());
    }

    #[test]
    fn is_solely_nato_classified_nato_plus_nato_with_fgi_marker_is_false() {
        // A NATO portion that carries a populated `fgi_marker` is
        // commingled-NATO, not pure NATO — project memory
        // `project_nato_transmutes_to_fgi`. The presence of an FGI
        // marker on any single NATO portion disqualifies the page.
        use crate::attrs::{FgiMarker, NatoClassification::NatoSecret};
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoSecret)),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoSecret)),
            fgi_marker: Some(FgiMarker::SourceConcealed),
            ..Default::default()
        });
        assert!(!ctx.is_solely_nato_classified());
    }

    #[test]
    fn is_solely_nato_classified_nato_plus_fgi_classified_is_false() {
        // A NATO portion + an FGI-classified portion is also not
        // solely-NATO. Tests the classification-variant gate
        // independently from the fgi_marker gate.
        use crate::attrs::{CountryCode, FgiClassification, NatoClassification::NatoSecret};
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Nato(NatoSecret)),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: vec![CountryCode::try_new(b"DEU").unwrap()].into(),
            })),
            ..Default::default()
        });
        assert!(!ctx.is_solely_nato_classified());
    }

    #[test]
    fn fgi_secret_contributes_to_max_classification() {
        // (C//NF) + (//DEU S//...) → banner must be SECRET
        use crate::attrs::{CountryCode, FgiClassification};
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs_with_classification(Classification::Confidential));
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        let a1 = CanonicalAttrs {
            sci_controls: vec![SciControl::Si].into_boxed_slice(),
            ..Default::default()
        };
        let a2 = CanonicalAttrs {
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
        let a1 = CanonicalAttrs {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()]
                .into_boxed_slice(),
            ..Default::default()
        };
        let a2 = CanonicalAttrs {
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
        let a1 = CanonicalAttrs {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()]
                .into_boxed_slice(),
            ..Default::default()
        };
        // Portion 2: NOFORN
        let a2 = CanonicalAttrs {
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        };
        ctx.add_portion(a1);
        ctx.add_portion(a2);
        // NOFORN wins → expected REL TO is empty (banner should say NOFORN)
        assert!(ctx.expected_rel_to().is_empty());
    }

    #[test]
    fn expected_rel_to_empty_when_nodis_in_portion() {
        // PR 3c.B-8F-engine-gap: NODIS in any portion implies NOFORN in
        // banner per CAPCO-2016 §H.9 p174 verbatim: "REL TO is not
        // authorized in the banner line if any portion contains NODIS
        // information. In this case, NOFORN would convey in the banner
        // line." `expected_rel_to` must short-circuit to empty via the
        // `needs_nf` flag from `expected_non_ic_dissem`. Unclassified
        // context — the §H.9 p174 passage does not gate on
        // classification.
        use crate::attrs::CountryCode;
        let mut ctx = PageContext::new();
        // Portion 1: REL TO USA, GBR
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()]
                .into_boxed_slice(),
            ..Default::default()
        });
        // Portion 2: NODIS (unclassified)
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            non_ic_dissem: vec![NonIcDissem::Nodis].into(),
            ..Default::default()
        });
        // Without the short-circuit, the intersection would be
        // [USA, GBR] (portion 1 alone, since portion 2 has no REL TO
        // list). With the short-circuit, it's empty.
        assert!(
            ctx.expected_rel_to().is_empty(),
            "NODIS in any portion must clear expected_rel_to per §H.9 p174"
        );
    }

    #[test]
    fn expected_rel_to_empty_when_exdis_in_portion() {
        // PR 3c.B-8F-engine-gap: EXDIS in any portion implies NOFORN in
        // banner per CAPCO-2016 §H.9 p172 verbatim: "REL TO is not
        // authorized in the banner line if any portion contains EXDIS
        // information. In this case, NOFORN would convey in the banner
        // line." `expected_rel_to` must short-circuit to empty via the
        // `needs_nf` flag from `expected_non_ic_dissem`. Classified
        // context — the §H.9 p172 passage does not gate on
        // classification (this test uses SECRET to exercise the
        // classified path; the unclassified sibling test above pins
        // the inverse classification path).
        use crate::attrs::CountryCode;
        let mut ctx = PageContext::new();
        // Portion 1: SECRET, REL TO USA, GBR
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()]
                .into_boxed_slice(),
            ..Default::default()
        });
        // Portion 2: SECRET, EXDIS
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            non_ic_dissem: vec![NonIcDissem::Exdis].into(),
            ..Default::default()
        });
        assert!(
            ctx.expected_rel_to().is_empty(),
            "EXDIS in any portion must clear expected_rel_to per §H.9 p172"
        );
    }

    #[test]
    fn expected_declassify_on_max() {
        let a1 = CanonicalAttrs {
            declassify_on: Some(IsmDate::Date(2035, 1, 1)),
            ..Default::default()
        };
        let a2 = CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            aea_markings: vec![AeaMarking::Rd(RdBlock::default())].into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            aea_markings: vec![AeaMarking::Rd(RdBlock {
                cnwdi: false,
                sigma: vec![20, 14].into(),
            })]
            .into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            non_ic_dissem: vec![NonIcDissem::SbuNf].into(),
            ..Default::default()
        });
        let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();
        assert!(non_ic.contains(&NonIcDissem::SbuNf));
        assert!(!needs_nf);
    }

    #[test]
    fn expected_non_ic_dissem_signals_needs_nf_on_nodis() {
        // PR 3c.B-8F-engine-gap: §H.9 p174 — "REL TO is not authorized
        // in the banner line if any portion contains NODIS information.
        // In this case, NOFORN would convey in the banner line." Unlike
        // the SBU-NF/LES-NF split, NODIS is NOT renamed or removed;
        // `needs_nf` is purely additive. Classification-independent.
        for classification in [Classification::Unclassified, Classification::Confidential] {
            let mut ctx = PageContext::new();
            ctx.add_portion(CanonicalAttrs {
                classification: Some(MarkingClassification::Us(classification)),
                non_ic_dissem: vec![NonIcDissem::Nodis].into(),
                ..Default::default()
            });
            let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();
            assert!(
                non_ic.contains(&NonIcDissem::Nodis),
                "NODIS must remain in non-IC set at {classification:?} \
                 (it is not split, only imply-NF)"
            );
            assert!(
                needs_nf,
                "needs_nf must be true when NODIS present at {classification:?} \
                 per §H.9 p174"
            );
        }
    }

    #[test]
    fn expected_non_ic_dissem_signals_needs_nf_on_exdis() {
        // PR 3c.B-8F-engine-gap: §H.9 p172 — "REL TO is not authorized
        // in the banner line if any portion contains EXDIS information.
        // In this case, NOFORN would convey in the banner line."
        // Symmetric to the NODIS test above. Classification-independent.
        for classification in [Classification::Unclassified, Classification::Secret] {
            let mut ctx = PageContext::new();
            ctx.add_portion(CanonicalAttrs {
                classification: Some(MarkingClassification::Us(classification)),
                non_ic_dissem: vec![NonIcDissem::Exdis].into(),
                ..Default::default()
            });
            let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();
            assert!(
                non_ic.contains(&NonIcDissem::Exdis),
                "EXDIS must remain in non-IC set at {classification:?} \
                 (it is not split, only imply-NF)"
            );
            assert!(
                needs_nf,
                "needs_nf must be true when EXDIS present at {classification:?} \
                 per §H.9 p172"
            );
        }
    }

    // --- FGI rollup ---

    #[test]
    fn fgi_source_concealed_supersedes_open() {
        let mut ctx = PageContext::new();
        // One portion with source-concealed FGI.
        ctx.add_portion(CanonicalAttrs {
            fgi_marker: Some(FgiMarker::SourceConcealed),
            ..Default::default()
        });
        // Another with source-acknowledged FGI.
        ctx.add_portion(CanonicalAttrs {
            fgi_marker: FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]),
            ..Default::default()
        });
        let marker = ctx.expected_fgi_marker().expect("should have FGI marker");
        // Source-concealed wins → bare FGI (no country list).
        assert!(
            matches!(marker, FgiMarker::SourceConcealed),
            "source-concealed should supersede: got {:?}",
            marker,
        );
    }

    #[test]
    fn fgi_open_union_of_countries() {
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            fgi_marker: FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            fgi_marker: FgiMarker::acknowledged([CountryCode::try_new(b"DEU").unwrap()]),
            ..Default::default()
        });
        let marker = ctx.expected_fgi_marker().unwrap();
        match marker {
            FgiMarker::Acknowledged { countries } => assert_eq!(countries.len(), 2),
            FgiMarker::SourceConcealed => panic!("expected acknowledged variant"),
        }
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"AUS").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"FVEY").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"KFOR").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"KFOR").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![CountryCode::USA, CountryCode::try_new(b"FVEY").unwrap()].into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_us: vec![DissemControl::Fouo].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_us();
        assert!(
            !dissem.contains(&DissemControl::Fouo),
            "FOUO should drop in classified doc: {dissem:?}"
        );
    }

    #[test]
    fn dissem_fouo_kept_in_unclassified() {
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            dissem_us: vec![DissemControl::Fouo].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_us();
        assert!(
            dissem.contains(&DissemControl::Fouo),
            "FOUO should stay in unclassified: {dissem:?}"
        );
    }

    #[test]
    fn dissem_fouo_drops_when_dsen_present_unclassified() {
        // DSEN overrides FOUO even on an unclassified page.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            dissem_us: vec![DissemControl::Dsen, DissemControl::Fouo].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_us();
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_us: vec![DissemControl::Oc, DissemControl::OcUsgov].into(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_us: vec![DissemControl::Oc].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_us();
        assert!(dissem.contains(&DissemControl::Oc));
        assert!(
            !dissem.contains(&DissemControl::OcUsgov),
            "OC-USGOV should drop when not on all OC portions: {dissem:?}"
        );
    }

    #[test]
    fn dissem_nf_injected_from_sbu_nf_split() {
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            non_ic_dissem: vec![NonIcDissem::SbuNf].into(),
            ..Default::default()
        });
        let dissem = ctx.expected_dissem_us();
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_controls: vec![SciControl::Si, SciControl::Tk].into_boxed_slice(),
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_controls: vec![SciControl::Si, SciControl::Tk].into_boxed_slice(),
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
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
        ctx.add_portion(CanonicalAttrs {
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
        SciControlSystem::Custom(SmolStr::from(s))
    }

    fn comp(id: &str, subs: &[&str]) -> SciCompartment {
        let sub_box: Box<[SmolStr]> = subs
            .iter()
            .map(|s| SmolStr::from(*s))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        SciCompartment::new(id, sub_box)
    }

    fn attrs_with_sci_markings(markings: Vec<SciMarking>) -> CanonicalAttrs {
        CanonicalAttrs {
            sci_markings: markings.into_boxed_slice(),
            ..Default::default()
        }
    }

    // --- expected_sar_marking (P4a) ---

    use crate::attrs::{SarCompartment, SarIndicator, SarMarking, SarProgram};

    fn sar_prog(id: &str, comps: Vec<SarCompartment>) -> SarProgram {
        SarProgram::new(id, comps.into_boxed_slice())
    }

    fn sar_comp(id: &str, subs: &[&str]) -> SarCompartment {
        let subs: Box<[SmolStr]> = subs
            .iter()
            .map(|s| SmolStr::from(*s))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        SarCompartment::new(id, subs)
    }

    fn attrs_with_sar(sar: SarMarking) -> CanonicalAttrs {
        CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
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
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::TopSecret)),
            sci_controls: vec![SciControl::Si].into_boxed_slice(),
            sar_markings: Some(SarMarking::new(
                SarIndicator::Abbrev,
                vec![sar_prog("BP", vec![sar_comp("J12", &["J54"])])].into_boxed_slice(),
            )),
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
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

    #[test]
    fn portions_pre_sized_to_typical_page() {
        // Regression guard: PageContext pre-sizes its portions Vec to
        // DEFAULT_PORTIONS_CAPACITY so that typical-page accumulation hits
        // zero reallocations. Covers `new()`, `default()`, AND `clone()`
        // — the engine clones via `Arc::new(page_context.clone())` at
        // `engine.rs:1025` when handing the page roll-up to banner/CAB
        // rules, so the pre-size must survive that path too. Issue #430.
        let ctx = PageContext::new();
        assert!(
            ctx.portions.capacity() >= DEFAULT_PORTIONS_CAPACITY,
            "PageContext::new should pre-size portions to at least {} (issue #430)",
            DEFAULT_PORTIONS_CAPACITY
        );
        let ctx_default = PageContext::default();
        assert!(
            ctx_default.portions.capacity() >= DEFAULT_PORTIONS_CAPACITY,
            "PageContext::default should pre-size identically to new() (issue #430)"
        );
        // Empty-ctx clone — proves the pre-size survives clone at the
        // boundary, but is NOT the engine's actual hand-off path: the
        // engine guards on `!page_context.is_empty()` at
        // `engine.rs:1019-1025` so it never clones an empty ctx in
        // production.
        let ctx_cloned_empty = ctx.clone();
        assert!(
            ctx_cloned_empty.portions.capacity() >= DEFAULT_PORTIONS_CAPACITY,
            "PageContext::clone of an empty ctx must preserve the pre-size (issue #430)"
        );
        // Non-empty clone — the engine's REAL hand-off path. The
        // failure window for the pre-size invariant under derived
        // `Clone` is `len() ∈ [1, DEFAULT_PORTIONS_CAPACITY)`: derived
        // `Vec::clone()` would size the cloned buffer to exactly
        // `len()`, dropping below the pre-size floor on every small
        // page (e.g., a 2-portion page would clone into a Vec with
        // capacity 2). This is the case the manual `Clone` impl
        // actually exists to defend.
        let mut ctx_with_portions = PageContext::new();
        ctx_with_portions.add_portion(attrs_with_classification(Classification::Unclassified));
        ctx_with_portions.add_portion(attrs_with_classification(Classification::Unclassified));
        let ctx_cloned_nonempty = ctx_with_portions.clone();
        assert!(
            ctx_cloned_nonempty.portions.capacity() >= DEFAULT_PORTIONS_CAPACITY,
            "PageContext::clone of a non-empty ctx must preserve the pre-size — the engine clones at this state (issue #430)"
        );
    }

    // ===========================================================================
    // DISPLAY ONLY axis roll-up (CAPCO-2016 §D.2 Table 3 rows 18-20, 25-27)
    // ===========================================================================

    fn cc(s: &str) -> CountryCode {
        CountryCode::try_new(s.as_bytes()).expect("valid country code")
    }

    #[test]
    fn expected_display_only_row_25_intersection_with_common() {
        // §D.2 Table 3 row 25: both portions carry DO with at least
        // one common country → banner DO = common.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG"), cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_display_only(),
            vec![cc("AFG")],
            "row 25: DO+DO intersection → common"
        );
        assert!(
            ctx.expected_rel_to().is_empty(),
            "row 25: no REL TO axis present → banner REL TO empty"
        );
    }

    #[test]
    fn expected_display_only_row_26_cross_axis_with_rel_to() {
        // §D.2 Table 3 row 26: portion A DO [AFG, IRQ], portion B
        // REL TO [USA, AFG, GBR] → banner DO [AFG], NO banner REL TO
        // (release implies disclosure; the page can't release to anyone
        // because portion A doesn't release at all).
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG"), cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("AFG"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_display_only(),
            vec![cc("AFG")],
            "row 26: DO ∩ REL TO → common (AFG)"
        );
        assert!(
            ctx.expected_rel_to().is_empty(),
            "row 26 Note: REL TO is NOT in banner when any portion is DO-only — portion A doesn't release"
        );
    }

    #[test]
    fn expected_display_only_row_27_commingled_both_axes() {
        // §D.2 Table 3 row 27: both portions carry both REL TO and DO,
        // at least one common in each → banner carries both axes.
        // REL TO covers AFG (common); DO additionally covers IRQ
        // (common across REL TO ∪ DO of each portion).
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("AFG")].into_boxed_slice(),
            display_only_to: vec![cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("AFG"), cc("IRQ")].into_boxed_slice(),
            display_only_to: vec![cc("NATO")].into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_rel_to(),
            vec![cc("USA"), cc("AFG")],
            "row 27: REL TO intersection = USA + AFG"
        );
        assert_eq!(
            ctx.expected_display_only(),
            vec![cc("IRQ")],
            "row 27: display-permission ∩ minus banner REL TO = IRQ (AFG dropped because it's in REL TO; NATO not in portion A)"
        );
    }

    #[test]
    fn expected_display_only_row_19_portion_without_fdr_clears() {
        // §D.2 Table 3 row 19: one portion has DO, another has neither
        // REL TO nor DO → banner DO empty (page falls into NOFORN via
        // the dissem layer).
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG"), cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            // No REL TO, no DO, no NF — just a bare portion.
            ..Default::default()
        });
        assert!(
            ctx.expected_display_only().is_empty(),
            "row 19: portion w/o display-permission clears banner DO"
        );
    }

    #[test]
    fn expected_display_only_row_20_no_common_country_clears() {
        // §D.2 Table 3 row 20: both portions have DO but no common
        // country → banner DO empty (banner becomes NOFORN).
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        assert!(
            ctx.expected_display_only().is_empty(),
            "row 20: DO ∩ DO with no common country → empty"
        );
    }

    #[test]
    fn expected_display_only_noforn_supersedes() {
        // NOFORN clears DO parallel to REL TO supersession.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            dissem_us: vec![DissemControl::Nf].into_boxed_slice(),
            ..Default::default()
        });
        assert!(
            ctx.expected_display_only().is_empty(),
            "NF supersedes DO (§H.8 p145 — NF wins over any foreign-audience axis)"
        );
    }

    #[test]
    fn expected_display_only_nodis_supersedes() {
        // §H.9 p174: NODIS in any portion forces banner NOFORN, which
        // supersedes both REL TO and DO. Unclassified context (§H.9
        // gates on dissem axis, not classification).
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            display_only_to: vec![cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            non_ic_dissem: vec![NonIcDissem::Nodis].into(),
            ..Default::default()
        });
        assert!(
            ctx.expected_display_only().is_empty(),
            "NODIS forces NF in banner → DO axis cleared"
        );
    }

    #[test]
    fn expected_display_only_strips_usa() {
        // USA is the originator — never appears in DO axis per
        // §H.8 p163 worked examples. If a portion's DO list includes
        // USA (mis-authored), it must be dropped from the banner.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("USA"), cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("USA"), cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_display_only(),
            vec![cc("AFG")],
            "USA stripped from DO banner — originator is implicit"
        );
    }

    #[test]
    fn expected_display_only_subtracts_banner_rel_to() {
        // §D.2 Table 3 row 21/27: a country covered by banner REL TO
        // does NOT repeat in banner DO. Banner REL TO = AFG (both
        // portions); DO list per portion includes AFG too, but banner
        // DO excludes AFG because REL TO already covers it.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("AFG")].into_boxed_slice(),
            display_only_to: vec![cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("AFG"), cc("IRQ")].into_boxed_slice(),
            display_only_to: Box::new([]),
            ..Default::default()
        });
        assert_eq!(
            ctx.expected_rel_to(),
            vec![cc("USA"), cc("AFG")],
            "REL TO intersection = USA + AFG"
        );
        assert_eq!(
            ctx.expected_display_only(),
            vec![cc("IRQ")],
            "DO axis carries only what REL TO doesn't (IRQ, not AFG)"
        );
    }

    #[test]
    fn expected_display_only_exdis_supersedes() {
        // §H.9 p172: EXDIS in any portion forces banner NOFORN
        // ("REL TO is not authorized... NOFORN would convey").
        // NF at banner supersedes DO per §D.2 Table 3 row 2.
        // Symmetric with the NODIS test above.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            display_only_to: vec![cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Unclassified)),
            non_ic_dissem: vec![NonIcDissem::Exdis].into(),
            ..Default::default()
        });
        assert!(
            ctx.expected_display_only().is_empty(),
            "EXDIS forces NF in banner → DO axis cleared (§H.9 p172 → §D.2 row 2)"
        );
    }

    #[test]
    fn expected_display_only_row_18_relido_plus_do_clears() {
        // §D.2 Table 3 row 18: RELIDO + DISPLAY ONLY → NOFORN.
        // A RELIDO portion carries no REL TO list and no DO list (RELIDO
        // is a `DissemControl`, not a country-list axis), so the
        // all-or-nothing gate fires and banner DO empties.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG"), cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            dissem_us: vec![DissemControl::Relido].into_boxed_slice(),
            ..Default::default()
        });
        assert!(
            ctx.expected_display_only().is_empty(),
            "row 18: RELIDO portion clears DO (portion has no display-permission)"
        );
    }

    #[test]
    fn expected_display_only_row_11_rel_to_plus_do_no_common_clears() {
        // §D.2 Table 3 row 11: REL TO + DO with no common country →
        // NOFORN. Each portion has display-permission, so the
        // all-or-nothing gate doesn't fire — the empty intersection
        // does.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            display_only_to: vec![cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        assert!(
            ctx.expected_display_only().is_empty(),
            "row 11: REL TO ∩ DO with no common country → empty"
        );
        assert!(
            ctx.expected_rel_to().is_empty(),
            "row 11: REL TO banner empty because portion 2 has no REL TO"
        );
    }

    #[test]
    fn expected_rel_to_row_10_relido_only_portion_clears() {
        // §D.2 Table 3 row 10: REL TO + RELIDO → NOFORN. Behavioral-
        // regression guard for the row-16 strict enforcement: a RELIDO
        // portion has no REL TO list, so banner REL TO is cleared
        // (under the pre-PR code path the RELIDO portion was filtered
        // out and the REL TO portion's intersection won — wrong per
        // row 10).
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            dissem_us: vec![DissemControl::Relido].into_boxed_slice(),
            ..Default::default()
        });
        assert!(
            ctx.expected_rel_to().is_empty(),
            "row 10: RELIDO + REL TO → banner REL TO empty"
        );
    }

    #[test]
    fn rel_to_row_16_portion_without_fdr_clears_rel_to() {
        // §D.2 Table 3 row 16: REL TO + portion w/o FD&R → NOFORN.
        // Regression guard for the row-16 strict enforcement added
        // alongside the DISPLAY ONLY axis: a portion that has no
        // REL TO axis (even without NF/NODIS/EXDIS to trip the existing
        // short-circuits) clears banner REL TO entirely.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            rel_to: vec![cc("USA"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            // Bare portion — no REL TO, no DO, no NF/NODIS/EXDIS.
            ..Default::default()
        });
        assert!(
            ctx.expected_rel_to().is_empty(),
            "row 16: portion w/o FD&R clears banner REL TO"
        );
    }

    #[test]
    fn render_banner_injects_noforn_for_row_16_rel_to_plus_bare() {
        // §D.2 Table 3 row 16: REL TO + portion w/o FD&R → NOFORN at
        // banner. The row-16 strict gate in `expected_rel_to` clears
        // the REL TO axis; the new Step 5 in `expected_dissem_us`
        // detects "FD&R intent on some portion + neither rolled-up
        // axis carries" and injects NF.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            rel_to: vec![cc("USA"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            // Bare portion — no FD&R axis.
            ..Default::default()
        });
        let banner = ctx.render_expected_banner().expect("non-empty page");
        assert_eq!(banner, "SECRET//NOFORN");
    }

    #[test]
    fn render_banner_injects_noforn_for_row_10_rel_to_plus_relido() {
        // §D.2 Table 3 row 10: REL TO + RELIDO → NOFORN. RELIDO is a
        // DissemControl, not a country-list axis, so the RELIDO portion
        // has empty REL TO — row-16 gate clears banner REL TO, Step 5
        // injects NF.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            rel_to: vec![cc("USA"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            dissem_us: vec![DissemControl::Relido].into_boxed_slice(),
            ..Default::default()
        });
        let banner = ctx.render_expected_banner().expect("non-empty page");
        // RELIDO survives (it's in dissem_us already); NOFORN injected
        // via Step 5. Within-category sort puts NOFORN before RELIDO
        // alphabetically on the dissem block.
        assert_eq!(banner, "SECRET//NOFORN/RELIDO");
    }

    #[test]
    fn render_banner_injects_noforn_for_row_19_do_plus_bare() {
        // §D.2 Table 3 row 19: DO + portion w/o FD&R → NOFORN.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            display_only_to: vec![cc("AFG")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            ..Default::default()
        });
        let banner = ctx.render_expected_banner().expect("non-empty page");
        assert_eq!(banner, "SECRET//NOFORN");
    }

    #[test]
    fn render_expected_banner_with_display_only_axis() {
        // End-to-end: a two-portion page with row-25 DO intersection
        // produces a banner with DO inside the dissem block, single-
        // slash separated per §A.6 p16 within-category separator.
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            display_only_to: vec![cc("AFG"), cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            display_only_to: vec![cc("AFG"), cc("GBR")].into_boxed_slice(),
            ..Default::default()
        });
        let banner = ctx.render_expected_banner().expect("non-empty page");
        assert_eq!(banner, "SECRET//DISPLAY ONLY AFG");
    }

    #[test]
    fn render_expected_banner_with_rel_to_and_display_only() {
        // Row 27: REL TO + DO commingled. Banner block is the dissem
        // family with REL TO and DO joined by `/` (within-category).
        let mut ctx = PageContext::new();
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            rel_to: vec![cc("USA"), cc("AFG")].into_boxed_slice(),
            display_only_to: vec![cc("IRQ")].into_boxed_slice(),
            ..Default::default()
        });
        ctx.add_portion(CanonicalAttrs {
            classification: Some(MarkingClassification::Us(Classification::Secret)),
            rel_to: vec![cc("USA"), cc("AFG"), cc("IRQ")].into_boxed_slice(),
            display_only_to: Box::new([]),
            ..Default::default()
        });
        let banner = ctx.render_expected_banner().expect("non-empty page");
        assert_eq!(banner, "SECRET//REL TO USA, AFG/DISPLAY ONLY IRQ");
    }
}
