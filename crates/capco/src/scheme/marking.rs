// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoMarking` — the newtype over `CanonicalAttrs` and its impls.
//!
//! Holds the `CapcoMarking` tuple struct (with optional decoder
//! provenance side channel), `PartialEq`/`Eq`/`From<CanonicalAttrs>`
//! impls, the inherent block carrying `new` + the production
//! `join_via_lattice` lattice-path composer, the `CapcoOpenVocabRef`
//! open-vocab enum, and the test-convenience `classification()`
//! accessor.
//!
//! ## CapcoMarking is a projection target, not a lattice element
//!
//! `CapcoMarking` is a **bag-of-axes** record over the 10+ CAPCO
//! categories. The lattice claim — `JoinSemilattice` /
//! `MeetSemilattice` — lives on the **per-axis** types (`RelToBlock`,
//! `DissemSet`, `SciSet`, `SarSet`, `AeaSet`, `FgiSet`, `JointSet`,
//! `NatoDissemSet`, `ClassificationLattice`, `NatoClassLattice`,
//! `DeclassifyOnLattice`). Each per-axis type satisfies the lattice
//! laws on its own native domain (e.g. `2^{Trigraph}` for REL TO,
//! `BTreeSet<DissemControl>` for DissemSet); `CapcoMarking` is the
//! cross-axis fold that composes those lattice values back into a
//! `CanonicalAttrs` record for the renderer and the rule layer.
//!
//! Cross-axis folding is a **projection**, not a lattice op:
//! structural `Eq` on `CanonicalAttrs` is finer than the lattice
//! equivalence on the expanded per-axis domains. A `JoinSemilattice`
//! impl on `CapcoMarking` would violate structural-`Eq` idempotence
//! whenever a per-axis lattice normalizes its input (the load-bearing
//! case is `RelToBlock`'s tetragraph expansion: `m.rel_to = [NATO]`
//! → after `m.join(m)` → `m.rel_to = {30 expanded trigraphs}`, so
//! `m != m.join(m)` under derived `Eq`). `CapcoMarking` therefore makes
//! no lattice claim; the cross-axis fold is the inherent method
//! `join_via_lattice`. The per-axis lattices are real; the cross-axis
//! composition is structural folding, not a lattice operation.
//!
//! ## Production pipeline
//!
//! `CapcoScheme::project(Scope::Page, ...)` runs the full
//! `join_via_lattice → closure → page_rewrites` pipeline.
//!
//! Page state is the engine's inline `Vec<CanonicalAttrs>` accumulator.
//! The per-axis residue computations live as free helpers and lattice
//! constructors in `crates/capco/src/lattice/`
//! (`sci_controls_from_markings`, `FgiSet::from_attrs_iter`,
//! `DeclassExemptionAccumulator::from_attrs_iter`,
//! `NonIcDissemSet::from_attrs_iter`,
//! `DisplayOnlyBlock::from_attrs_iter`); the pipeline consumes
//! `&[CanonicalAttrs]` end-to-end.
//!
//! Imports reach helpers via `super::actions::*` /
//! `super::predicates::*` / `super::constraints::*`.

use marque_ism::{CanonicalAttrs, Classification, MarkingClassification};
// `JoinSemilattice` stays in scope for the per-axis lattice types
// (`SarSet::join`, `FgiSet::join`) that the cross-axis fold composes.
// `CapcoMarking` itself does not implement `JoinSemilattice`; the
// lattice claim lives on the per-axis types, not on the cross-axis fold.
use marque_scheme::JoinSemilattice;

use super::actions::*;

/// CAPCO marking as viewed through the `marque-scheme` lens. A thin
/// newtype around [`CanonicalAttrs`] so we can hang trait impls on it
/// without orphan-rule problems.
///
/// # Not a lattice
///
/// `CapcoMarking` is exported publicly so equivalence tests can
/// construct it, but it **does not uphold the [`Lattice`] contract**
/// on every input (see the caveat block below). Downstream consumers
/// must not rely on `Lattice::join` / `Lattice::meet` of `CapcoMarking`
/// producing law-abiding results. Use [`crate::capco_rules`] and
/// `marque-core` directly for production paths.
///
/// # Decoder provenance side channel
///
/// Tuple-position 1 is an optional [`DecoderProvenance`] populated by
/// the probabilistic decoder recognizer. Strict-path recognizers leave
/// it `None`. The engine reads `provenance.is_some()` to detect "this
/// recognition went through the decoder fallback" and emits a
/// synthetic decoder-recognition diagnostic with
/// [`FixSource::DecoderPosterior`](marque_rules::FixSource::DecoderPosterior).
/// See [`crate::provenance`] for the side-channel contract.
///
/// `PartialEq` / `Eq` ignore tuple-position 1 — provenance is metadata,
/// not identity. Two markings with identical attrs but different
/// provenance traces compare equal.
#[derive(Debug, Clone)]
pub struct CapcoMarking(
    pub CanonicalAttrs,
    pub Option<crate::provenance::DecoderProvenance>,
);

impl PartialEq for CapcoMarking {
    /// Identity is the parsed attributes only — decoder provenance is
    /// audit metadata that does not participate in marking equality
    /// (see the type-level doc comment).
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for CapcoMarking {}

impl From<CanonicalAttrs> for CapcoMarking {
    #[inline]
    fn from(attrs: CanonicalAttrs) -> Self {
        Self(attrs, None)
    }
}

impl CapcoMarking {
    /// Construct a strict-path `CapcoMarking` (no decoder provenance).
    ///
    /// Use this in tests and strict-path recognizers; the decoder
    /// constructs the marking by setting tuple-position 1 directly when
    /// it has provenance to attach.
    #[inline]
    pub fn new(attrs: CanonicalAttrs) -> Self {
        Self(attrs, None)
    }

    /// Component-wise join via the per-category `marque-capco::lattice`
    /// types — the production page-projection composer.
    ///
    /// Composes per-axis lattice results across the 10+ CAPCO axes
    /// (classification together with JointSet, SciSet, SarSet, AeaSet,
    /// FgiSet, DissemSet, NatoDissemSet, RelToBlock, DeclassifyOnLattice,
    /// declass_exemption, non_ic_dissem, display_only).
    ///
    /// Both call surfaces — the trait-path
    /// `MarkingScheme::project(Scope::Page, ...)` and the engine
    /// fast-path `CapcoScheme::project_from_attrs_slice` — delegate
    /// through `CapcoScheme::project_attrs_pipeline`, which calls
    /// this method directly. The parity gate at
    /// `crates/capco/tests/lattice_vs_scheme_parity.rs` pins the
    /// `per-axis lattice` ↔ `full scheme pipeline` byte-identity
    /// claim across the fixtures (with documented divergences for the
    /// closure-rule and PageRewrite-catalog operations that the per-axis
    /// path does not yet model).
    ///
    /// Authority: per-axis citations are on each `lattice` module type's
    /// doc comment.
    ///
    /// ## Fast-paths
    ///
    /// Two short-circuit cases avoid the full `join_via_lattice_body`
    /// scan when the fold is trivially identity:
    ///
    /// **Empty slice** — zero portions → `CanonicalAttrs::default()`
    /// (the lattice bottom). This is a degenerate page with no markings;
    /// no rule fires and no banner is produced.
    ///
    /// **Single portion** — `join(x) = x` for any `x` in a
    /// join-semilattice (lattice identity law). The body is O(n) in the
    /// number of per-axis constructor calls; all constructors reduce to
    /// identity when `portions.len() == 1` provided the guards below
    /// are satisfied.
    ///
    /// ### Guards — cases where the body still runs on a single portion
    ///
    /// Three categories of body normalization are NOT identity for a
    /// single portion and require the body to run:
    ///
    /// **a) Classification variant normalization:**
    ///
    /// - `Conflict { us, foreign }` (variant 4): the body flattens it
    ///   to `Us(us)` (variant 0). A fast-path returning `Conflict`
    ///   would cause banner-mismatch false positives when the banner
    ///   parses as `Us(TS)` (variant 0 ≠ variant 4).
    ///
    /// - `Joint(j)` missing USA (§H.3 p56 violation): `JointSet`
    ///   treats a no-USA JOINT as malformed and returns `Bottom`, so the
    ///   body flattens it to `Us(j.level)` via the ClassificationLattice
    ///   path. Same false-positive risk.
    ///
    /// **b) Decomposable tetragraph expansion in `rel_to`:**
    ///
    /// `RelToBlock::from_attrs_iter` inside `join_via_lattice_body`
    /// expands decomposable tetragraphs (e.g. FVEY → {AUS, CAN, GBR,
    /// NZL, USA}) before materializing `out.rel_to`. This expansion
    /// runs only in the body — there is no equivalent PageRewrite.
    /// A single portion with `rel_to = [USA, FVEY]` would be returned
    /// as-is by the fast-path (unexpanded), while the body produces
    /// `rel_to = [USA, AUS, CAN, GBR, NZL]`. Rules that compare the
    /// projected `rel_to` against the observed banner text would then
    /// misfire (e.g. E031/E035 comparing `[USA, FVEY]` against a
    /// banner that expands FVEY to its constituents).
    ///
    /// **c) `display_only_to` processing:**
    ///
    /// `DisplayOnlyBlock::from_attrs_iter` inside the body also expands
    /// decomposable tetragraphs and subtracts REL TO countries (§D.2
    /// Table 3 row 27) + USA (§H.8 p163) from the display-permission
    /// set. The fast-path is safe for `display_only_to` only when it is
    /// empty, OR when it contains no decomposable tetragraphs, does not
    /// include USA, and `rel_to` is empty (so the row-27 subtraction is
    /// a no-op). Any other combination requires the body.
    ///
    /// Cross-axis normalizations driven by `dissem_us` (NOFORN+REL-TO
    /// clearing, NODIS/EXDIS/SBU-NF NOFORN injection) are handled by
    /// downstream PageRewrites inside `CapcoScheme::project_attrs_pipeline`
    /// and produce byte-identical results whether the fast-path or the
    /// body feeds the rewrites.
    ///
    /// §-authority: join-semilattice identity law (`join(x) = x`).
    /// Per PR LA-3 (issue marquetools/marque#584).
    pub fn join_via_lattice(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
        use marque_ism::CountryCode;

        // Fast-path 1: empty page → lattice bottom.
        if portions.is_empty() {
            return CanonicalAttrs::default();
        }

        // Fast-path 2: single portion, unless the body would normalize it.
        //
        // See doc comment §§ a/b/c above for the full rationale.
        if let [p] = portions {
            // Guard a: classification variant normalization.
            let classification_safe = match &p.classification {
                Some(MarkingClassification::Conflict { .. }) => false,
                Some(MarkingClassification::Joint(j)) => j.countries.contains(&CountryCode::USA),
                _ => true,
            };

            // Guard b: decomposable tetragraphs in rel_to.
            let rel_to_safe = !p
                .rel_to
                .iter()
                .any(|c| marque_ism::lookup_tetragraph_members(c.as_str()).is_some());

            // Guard c: display_only_to processing.
            // Safe only when empty, OR all of: no decomposable
            // tetragraph, no USA (always subtracted by §H.8 p163), and
            // rel_to is empty (no row-27 subtraction needed).
            let display_only_safe = p.display_only_to.is_empty()
                || (p.rel_to.is_empty()
                    && !p.display_only_to.contains(&CountryCode::USA)
                    && !p
                        .display_only_to
                        .iter()
                        .any(|c| marque_ism::lookup_tetragraph_members(c.as_str()).is_some()));

            if classification_safe && rel_to_safe && display_only_safe {
                return p.clone();
            }
        }

        Self::join_via_lattice_body(portions)
    }

    /// Shared body for the `join_via_lattice` entry point.
    ///
    /// Composes per-axis lattice results across 10+ axes
    /// (classification + JointSet, SciSet, SarSet, AeaSet, FgiSet,
    /// DissemSet, NatoDissemSet, RelToBlock, DeclassifyOnLattice,
    /// declass_exemption, non_ic_dissem, display_only). The body
    /// consumes only `portions: &[CanonicalAttrs]`; the residue-axis
    /// computations are free helpers in `crates/capco/src/lattice/`.
    ///
    /// ## Size guideline
    ///
    /// Clippy's `too_many_lines` lint fires on this function. The
    /// structural justification (axis ordering + inline citations +
    /// cross-axis state flow) is load-bearing — splitting this
    /// cross-axis fold into per-axis sub-functions would require
    /// threading every intermediate state value via a struct, which
    /// pays the readability cost without the maintainability win.
    ///
    /// - Axis ordering is load-bearing. The solely-non-US handling, the
    ///   NOFORN-supersession overlay, and the SBU-NF/LES-NF NOFORN
    ///   injection are encoded as ordered phases within this function.
    ///   Each phase reads state computed by the prior phase (e.g.
    ///   `out.classification` informs the foreign-source comparison;
    ///   `rel_to_was_*` flags drive the final DissemSet overlay).
    ///   Splitting into per-axis sub-functions would either (a) require
    ///   threading all the cross-axis state via a struct, paying the
    ///   readability cost it would notionally save, or (b) duplicate
    ///   per-portion walks across sub-function boundaries, breaking the
    ///   read-only-attrs invariant's audit surface.
    /// - The per-axis citations (`§H.7 pp123-125`, `§H.3 p57`,
    ///   `§H.8 p145`, etc.) live inline alongside the code they
    ///   justify. A split would scatter them across files and harm
    ///   Constitution VIII (citation-fidelity) maintainability.
    ///
    /// Future maintainers hitting `clippy::too_many_lines` here should
    /// `#[allow]` rather than split. The `#[allow]` below is permanent —
    /// not a TODO.
    ///
    /// Authority: axis ordering per CAPCO-2016 §G.1 Table 4 p37; per-axis
    /// citations are inline below.
    #[allow(
        clippy::too_many_lines,
        reason = "Cross-axis state flow + inline §-citations are \
                  structurally justified; see doc comment above."
    )]
    fn join_via_lattice_body(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
        use crate::lattice::{
            AeaSet, ClassificationLattice, DeclassExemptionAccumulator, DeclassifyOnLattice,
            DisplayOnlyBlock, DissemSet, FgiSet, JointSet, NatoDissemSet, NonIcDissemSet,
            RelToBlock, SarSet, SciSet, sci_controls_from_markings,
        };

        let mut out = CanonicalAttrs::default();

        // Page-composition introspection used by several axes below.
        // A page is "solely non-US" when it carries at least one
        // non-US classification AND no US-classification portion.
        // Per §H.7 pp123-125 reciprocal-raise: when ANY US portion is
        // present, NATO/FGI variants normalize to `Us(effective_level)`
        // at banner time; the non-US variant survives only when the
        // page has no US contribution at all.
        //
        // Three classification variants are US-bearing for the
        // solely-non-US gate:
        //
        // - `Us(_)`: explicit US classification.
        // - `Conflict { us, .. }`: carries an implicit US classification
        //   in the `us` field (see the `MarkingClassification::Conflict`
        //   doc comment). The parser records "I saw two systems; US
        //   wins" — so Conflict is US from the gate's perspective.
        // - `Joint(_)`: by §H.3 p56, USA is required to be in the
        //   producer list (JOINT is US co-owned by definition); JOINT
        //   classifications are therefore US-bearing for the gate. A
        //   mixed page like `JOINT C USA GBR + NATO S` must NOT keep
        //   `solely_non_us=true`, or the NATO portion would be preserved
        //   as `Nato(_)` rather than reciprocal-raising to `Us(_)` per
        //   §H.7 pp123-125. The same-level case is load-bearing: when the
        //   level chain doesn't already pick a winner via OrdMax, variant
        //   survival in the per-portion filter loop produces the wrong
        //   banner shape.
        //
        // §-authority: §H.7 pp123-125 (reciprocal-classification rule)
        // + §H.3 p56 (JOINT requires USA in producer list). Verified
        // against CAPCO-2016.md.
        let mut has_us_class = false;
        let mut has_non_us_class = false;
        for p in portions {
            match &p.classification {
                Some(MarkingClassification::Us(_))
                | Some(MarkingClassification::Conflict { .. })
                | Some(MarkingClassification::Joint(_)) => has_us_class = true,
                Some(MarkingClassification::Fgi(_)) | Some(MarkingClassification::Nato(_)) => {
                    has_non_us_class = true
                }
                None => {}
            }
        }
        let solely_non_us = has_non_us_class && !has_us_class;

        // Axis 1: classification — variant-preserving OrdMax with
        // JointSet override. §H.1 pp47-54 + §H.7 pp123-125 +
        // §H.3 p57.
        //
        // Decision tree:
        // - JointSet::UnanimousProducers → banner is Joint(_,_) and
        //   ClassificationLattice's output is replaced.
        // - JointSet::DisunityCollapse → banner is Us(highest_level)
        //   from JointSet (non-US producers ride to FGI separately).
        // - JointSet::Mixed (JOINT + non-JOINT both seen, §H.3 p57)
        //   AND JointSet::Bottom (no JOINT portions) →
        //   ClassificationLattice wins, BUT any Joint(_) variants on
        //   per-portion classifications are flattened to their
        //   effective_level (Us) so the banner doesn't carry forward
        //   JOINT shape per §H.3 p57. In this non-JOINT branch,
        //   when the page is NOT solely-non-US, ALSO flatten
        //   Nato(_) / Fgi(_) variants to Us(effective_level) per the
        //   §H.7 pp123-125 reciprocal-raise — preserves PageContext
        //   parity on mixed US+NATO/FGI pages.
        let joint_set = JointSet::from_attrs_iter(portions);
        out.classification = match joint_set.to_marking_classification() {
            Some(mc) => Some(mc),
            None => {
                // CLONE-1 (issue #606): avoid cloning the full `CanonicalAttrs`
                // slice just to modify the `classification` field.
                // `ClassificationLattice::from_classification_iter` folds over
                // pre-computed `Option<MarkingClassification>` values directly,
                // skipping all the non-classification fields that `CanonicalAttrs`
                // carries (sci_markings, sar_markings, aea_markings, rel_to, etc.).
                // Country-bearing variants (`Fgi`, `Conflict`) in the
                // `Some(mc) => Some(mc.clone())` pass-through arm can still
                // allocate for their country-list payload, but that cost is
                // incurred only when solely_non_us is true (Nato/Fgi not raised),
                // compared to the prior code which allocated a full
                // `CanonicalAttrs` clone for every portion unconditionally. The
                // per-portion classification adjustments (JOINT flatten,
                // §H.7 reciprocal-raise, Conflict flatten) are computed inline.
                ClassificationLattice::from_classification_iter(portions.iter().map(|p| {
                    match &p.classification {
                        // Always flatten JOINT to its US level in
                        // this non-JOINT branch (§H.3 p57).
                        Some(MarkingClassification::Joint(j)) => {
                            Some(MarkingClassification::Us(j.level))
                        }
                        // §H.7 reciprocal-raise: NATO/FGI flatten
                        // to US level when ANY US portion is in
                        // scope. The solely-non-US case keeps the
                        // non-US variant intact.
                        Some(MarkingClassification::Nato(n)) if !solely_non_us => {
                            Some(MarkingClassification::Us(n.us_equivalent()))
                        }
                        Some(MarkingClassification::Fgi(f)) if !solely_non_us => {
                            Some(MarkingClassification::Us(f.level))
                        }
                        // Conflict always flattens to its implicit `us`
                        // level in this non-JOINT branch. `Conflict` is
                        // the parser's way of recording "I saw two
                        // classification systems; US wins per §H.7"; the
                        // foreign side rides separately through the FGI
                        // axis, so the classification axis carries only
                        // the US level here. Authority:
                        // CAPCO-2016 §H.7 pp123-125.
                        Some(MarkingClassification::Conflict { us, .. }) => {
                            Some(MarkingClassification::Us(*us))
                        }
                        // Remaining variants (None, Us(_), Nato/Fgi when
                        // solely_non_us) pass through. Split None vs Some to
                        // avoid cloning when there is no data.
                        None => None,
                        Some(mc) => Some(mc.clone()),
                    }
                }))
                .into_inner()
            }
        };

        // Axis 2-5: SCI / SAR / AEA / FGI — assemble from per-portion
        // markings. SciSet / AeaSet take `&[Marking]` (flat per-portion
        // union); SarSet takes `Option<&SarMarking>`.
        //
        // Use the iterator-based constructors (`from_markings_iter`) to
        // avoid the intermediate `sci_markings_concat` /
        // `aea_markings_concat` `Vec` allocations that the slice-based
        // APIs required (issue #606).
        let sci_set =
            SciSet::from_markings_iter(portions.iter().flat_map(|p| p.sci_markings.iter()));
        out.sci_markings = sci_set.to_markings();

        // Compatibility view: sci_controls is the flat CVE-enum
        // projection. The structural axis above is the authoritative
        // form. `sci_controls_from_markings` reads `attrs.sci_controls`
        // per portion — the parser-populated CVE projection — not from
        // `out.sci_markings` (the structural roll-up sets
        // `canonical_enum: None` on every output, so a
        // project-from-markings path would always return empty). §H.4 p61.
        out.sci_controls = sci_controls_from_markings(portions);

        // SAR: SarSet operates on a single SarMarking (`sar_markings`
        // field is `Option<SarMarking>`). Join across portions composes
        // per-program by union.
        let mut sar_acc = SarSet::empty();
        for p in portions {
            let part = SarSet::from_marking(p.sar_markings.as_ref());
            sar_acc = sar_acc.join(&part);
        }
        out.sar_markings = sar_acc.to_marking();

        let aea_markings_iter = portions.iter().flat_map(|p| p.aea_markings.iter());
        out.aea_markings = AeaSet::from_markings_iter(aea_markings_iter).to_markings();

        // FGI marker — compose via FgiSet from per-portion markers
        // AND merge with classification-derived producers
        // (`FgiSet::from_attrs_iter` unions per-portion `fgi_marker`
        // values with classification-derived producers — NATO/JOINT/FGI
        // classification countries are surfaced onto the FGI axis per
        // §H.7 p123 + p128).
        //
        // When JointSet is `UnanimousProducers`, the producers are
        // already captured in the JOINT classification — we must NOT
        // also FGI-mark them, because §H.3 p56 + §H.7 p123 say JOINT
        // subsumes the FGI marker for those producers.
        //
        // When both an explicit FgiSet marker AND classification-derived
        // producers are present, UNION the producer sets rather than
        // discarding the classification-derived ones.
        let mut fgi_acc = FgiSet::empty();
        for p in portions {
            let part = FgiSet::from_marker(p.fgi_marker.as_ref());
            fgi_acc = fgi_acc.join(&part);
        }
        let ctx_fgi_marker = if matches!(joint_set, JointSet::UnanimousProducers { .. }) {
            // JOINT-unanimous page — producers ride on the `Joint(_)`
            // classification, not on the FGI axis. Suppress the
            // classification-derived FGI fallback so we don't double-mark
            // (§H.3 p56 + §H.7 p123).
            None
        } else if solely_non_us {
            // Solely-non-US page where the lattice preserves a `Nato(_)`
            // or `Fgi(_)` classification intact (the §H.7 reciprocal-raise
            // is suppressed earlier in this method when no US portion is
            // present to raise toward). The foreign source is already
            // recorded on the classification axis itself; running
            // `FgiSet::from_attrs_iter` here would derive the SAME
            // producers from the classification a second time and surface
            // them on the dissem-axis `fgi_marker`, producing a doubled
            // marker (§H.7 pp123-125).
            //
            // Per-portion `fgi_marker` fields (FgiSet) are still
            // honored — `fgi_acc.to_marker()` is what we ultimately
            // merge with this `None`. The suppression only drops the
            // classification-derived secondary fold.
            //
            // §-authority: §H.7 p123 (FGI source is recorded ONCE per
            // portion; for non-US classifications the source IS the
            // classification axis).
            //
            // Blanket suppression is unsafe when the winner
            // classification's foreign payload is a STRICT SUBSET of all
            // foreign sources contributed by all non-US classification
            // portions. The failure mode:
            //
            //   Inputs:  Fgi(Confidential, [GBR]), Fgi(Secret, [CAN])
            //   ClassificationLattice winner: Fgi(Secret, [CAN])
            //     (OrdMax: Secret > Confidential)
            //   GBR would be silently lost from the FGI axis.
            //   The correct §H.7 p124 behavior is to surface both
            //   producers on the FGI axis (source-loss reconstruction).
            //
            // So gather the union of foreign sources from all non-US
            // classification portions, compare against the winner's
            // foreign sources, and:
            //   - if equal: safe to suppress.
            //   - if winner is strict subset: build a synthetic FGI
            //     marker carrying the missing sources so they merge
            //     into `out.fgi_marker` via `merge_fgi_markers`.
            //
            // The same-variant UNION tiebreaker covers the same-level
            // case (both producers ride on the winner's payload,
            // suppression remains safe); source-loss reconstruction only
            // fires when level disagreement made OrdMax discard a foreign
            // source.
            //
            // §-authority: §H.7 p124 (source-concealed-dominance
            // precedence rules at the banner-line guidance block) +
            // §H.7 pp123-125 (FGI source must be preserved across
            // the projection) + §H.7 p128 (concealed-dominates
            // when mixed concealed + acknowledged portions exist).
            // Verified against `crates/capco/docs/CAPCO-2016.md`.
            //
            // `extract_foreign_sources` returns `Option<Vec<CountryCode>>`
            // where `None` = source-concealed FGI on that portion. If any
            // portion is concealed, the page must carry
            // `FgiMarker::SourceConcealed` (§H.7 p128) — an empty Vec
            // would be indistinguishable from "no FGI" and could produce
            // a synthetic acknowledged marker.
            let any_concealed = portions
                .iter()
                .any(|p| extract_foreign_sources(p.classification.as_ref()).is_none());
            if any_concealed {
                // At least one portion is source-concealed → banner must
                // use bare `FGI` (no countries) per §H.7 p128.
                Some(marque_ism::FgiMarker::SourceConcealed)
            } else {
                let classification_sources: std::collections::BTreeSet<marque_ism::CountryCode> =
                    portions
                        .iter()
                        .flat_map(|p| {
                            extract_foreign_sources(p.classification.as_ref()).unwrap_or_default()
                        })
                        .collect();
                let winner_sources: std::collections::BTreeSet<marque_ism::CountryCode> =
                    extract_foreign_sources(out.classification.as_ref())
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                if winner_sources == classification_sources {
                    // Safe-suppression branch: every foreign source
                    // observed across all portions is preserved on the
                    // winning classification's payload. No source loss.
                    None
                } else {
                    // Source-loss branch: at least one source is missing
                    // from the winner's payload. Build a synthetic
                    // acknowledged FGI marker carrying every foreign
                    // source so `merge_fgi_markers` unions them into the
                    // final output.
                    marque_ism::FgiMarker::acknowledged(classification_sources)
                }
            }
        } else {
            // The "non-solely-non-US" branch unions per-portion
            // `fgi_marker` with classification-derived producers (NATO
            // / JOINT / FGI variants). `FgiSet::from_attrs_iter`
            // carries the §H.7 p122 + p123 + p128 semantics; the
            // result is then merged with the explicit-FGI-marker fold
            // via `merge_fgi_markers`.
            FgiSet::from_attrs_iter(portions).to_marker()
        };
        out.fgi_marker = merge_fgi_markers(fgi_acc.to_marker(), ctx_fgi_marker);

        // Axis 6-7: dissem_us / dissem_nato.
        // Build `dissem_us` as a `DissemSet` (rather than its
        // boxed-slice form) so cross-axis NOFORN injection below can
        // route through `DissemSet::with_noforn_injected` and have
        // the supersession overlay strip dominated controls per
        // §H.8 p145.
        let dissem_set = DissemSet::from_attrs_iter(portions);
        out.dissem_nato = NatoDissemSet::from_attrs_iter(portions).into_boxed_slice();

        // Axis 8: rel_to.
        let rel_to_block = RelToBlock::from_attrs_iter(portions);
        let rel_to_was_noforn_superseded = rel_to_block.is_noforn_superseded();
        // Also capture the `Empty` variant (disjoint REL TO country
        // lists with no common [LIST] — §D.2 Table 3 row 9) BEFORE
        // `into_boxed_slice()` consumes the discriminant. An `Empty`
        // intersection means no common release audience exists, so the
        // banner MUST carry NOFORN per §D.2 Table 3 row 9 (a page with
        // two REL TO portions listing disjoint countries otherwise
        // produces an empty `rel_to` slice with no `Nf` injected).
        //
        // §-authority: §D.2 p28-30 Table 3 row 9 (REL TO [USA, LIST] + REL
        // TO [USA, LIST] with no common [LIST] → NOFORN banner).
        // Verified against crates/capco/docs/CAPCO-2016.md.
        let rel_to_was_empty_intersection = rel_to_block.is_empty_intersection();
        // Defer `out.rel_to` assignment until after
        // `DisplayOnlyBlock::from_attrs_iter` borrows `rel_to_block`
        // for the row-27 banner-REL-TO subtraction — see below.

        // Axis 9: declassify_on. The lattice now folds each portion's
        // (declassify_on, declass_exemption) pair into a single §E.3
        // `DeclassInstruction` (CAPCO-2016 §E.3 p32-33 nine-tier
        // "longest period of protection" precedence) and joins across
        // portions; the resolved date is projected back onto the
        // (still date-only) pivot field via `.into_date()`. The full
        // instruction tier is realized at the engine node in PR-D3.
        // `declass_exemption` continues to ride as last-observed via
        // `DeclassExemptionAccumulator` until that node retires it.
        out.declassify_on = DeclassifyOnLattice::from_attrs_iter(portions).into_date();
        out.declass_exemption = DeclassExemptionAccumulator::from_attrs_iter(portions).into_inner();

        // Axis 10: non_ic_dissem — classification-gated SBU-NF /
        // LES-NF split (§H.9 p178 / p185) + implied-NF for
        // NODIS / EXDIS (§H.9 p172 / p174).
        //
        // `NonIcDissemSet::from_attrs_iter` carries the non-IC dissem
        // roll-up. The `needs_nf` flag is consumed at the cross-axis
        // NOFORN injection rendezvous below: when set, NOFORN is injected
        // into `dissem_us` AND REL TO is cleared.
        let non_ic_set = NonIcDissemSet::from_attrs_iter(portions);
        let needs_nf = non_ic_set.needs_nf();
        out.non_ic_dissem = non_ic_set.into_boxed_slice();

        // DISPLAY ONLY axis (§D.2 Table 3 rows 18-20 + 25-27, §H.8
        // p163). Cross-axis intersection over (REL TO ∪ DO) with
        // banner-REL-TO and USA subtraction.
        //
        // The dedicated `DisplayOnlyBlock` lattice (parallel to
        // `RelToBlock`) carries the §D.2 Table 3 row 18-20 + 25-27 +
        // §H.8 p163 semantics. The constructor consumes the pre-computed
        // `rel_to_block` (for row-27 subtraction) and `needs_nf` (for the
        // NODIS/EXDIS short-circuit per §H.9 p172 / p174). NOFORN
        // supersession (§D.2 Table 3 rows 1-2 + §H.8 p145) is applied
        // inside the lattice constructor.
        out.display_only_to =
            DisplayOnlyBlock::from_attrs_iter(portions, &rel_to_block, needs_nf).into_boxed_slice();

        // Now that DisplayOnlyBlock has consumed its read of
        // `rel_to_block`, materialize `out.rel_to` from the same value.
        // (Deferred from the §H.8 p150-151 / §D.2 Table 3 row 9
        // computation above so a single RelToBlock value serves both
        // consumers.)
        out.rel_to = rel_to_block.into_boxed_slice();

        // NOFORN-clears-REL-TO interaction + cross-axis NOFORN
        // injection.
        //
        // When NOFORN must be injected from a cross-axis source (non-IC
        // SBU-NF/LES-NF on a classified page, or NODIS/EXDIS supersession
        // via RelToBlock), the injection MUST route through
        // `DissemSet::with_noforn_injected` so the §H.8 p145
        // NOFORN-dominates overlay strips any `Rel` / `Relido` /
        // `Displayonly` that survived from the per-portion union.
        // Inserting `Nf` into `out.dissem_us` directly after
        // `DissemSet::into_boxed_slice` had run would produce invalid
        // output per §H.8 p145.
        //
        // Authority: §H.8 p145 (NOFORN dominates REL TO / RELIDO /
        // EYES ONLY / DISPLAY ONLY) + §D.2 Table 3 rows 1-2 +
        // §H.9 p172 (NODIS) / §H.9 p174 (EXDIS) inject NOFORN at
        // banner. The `Empty` intersection case joins `NofornSuperseded`
        // — both require NOFORN injection per §D.2 Table 3 row 9 (Empty)
        // and rows 1-2 / §H.9 p172/p174 (NofornSuperseded).
        let dissem_final =
            if rel_to_was_noforn_superseded || rel_to_was_empty_intersection || needs_nf {
                // SBU-NF / LES-NF on a classified page also clears REL TO,
                // short-circuiting to an empty slice when needs_nf fires.
                if needs_nf {
                    out.rel_to = Box::new([]);
                }
                dissem_set.with_noforn_injected()
            } else {
                dissem_set
            };
        out.dissem_us = dissem_final.into_boxed_slice();

        out
    }
}

// Why `CapcoMarking` is not a lattice
// -----------------------------------
//
// Per-category lattice impls live in `marque-capco::lattice` for every
// CAPCO axis (Classification, NatoClass, Joint, Dissem, NatoDissem,
// RelToBlock, DeclassifyOn, plus AeaSet / SciSet / SarSet / FgiSet). The
// component-wise composition is exposed on
// `CapcoMarking::join_via_lattice()` above — but `CapcoMarking` itself
// implements neither `JoinSemilattice` nor `MeetSemilattice`.
//
// A `JoinSemilattice` impl would violate the idempotence law, driven by
// tetragraph expansion in `RelToBlock::from_attrs_iter`:
//
//   let m = CapcoMarking::new(CanonicalAttrs {
//       rel_to: [CountryCode::NATO].into(),
//       ..
//   });
//   let joined = m.join(&m);
//   // joined.0.rel_to would be the 30-trigraph NATO expansion,
//   // NOT [NATO]. Structural Eq fails: `m != joined`.
//
// The reasoning:
//
//   1. Per-axis lattices (`RelToBlock`, `DissemSet`, `SciSet`,
//      `SarSet`, etc.) ARE sound lattices on their native domains
//      (e.g. `2^{Trigraph}` for REL TO). Idempotence holds on the
//      lattice type's own structural `Eq`, which compares the
//      expanded representative.
//   2. `CapcoMarking` is a **cross-axis fold** of those lattice
//      values back into a `CanonicalAttrs` record. The fold is a
//      *projection*, not a join. Claiming `JoinSemilattice` on the
//      record type would promise a law (structural-`Eq` idempotence)
//      that the construction cannot keep without either lossy
//      eager canonicalization at construction (would erase the
//      `NATO` atom from the renderer's input form) or a
//      quotient-`Eq` rewrite across all `CanonicalAttrs` fields
//      (massive blast radius). Both are rejected.
//
// The cross-axis-fold entry is the inherent method
// `CapcoMarking::join_via_lattice` above; engine and scheme call sites
// call it directly. `MeetSemilattice` is omitted for the same
// algebraic reason — a "partial component-wise minimum" would not
// satisfy the meet laws on the cross-axis record type either.
//
// Per-axis `JoinSemilattice` / `MeetSemilattice` impls on `RelToBlock`,
// `DissemSet`, `SciSet`, `SarSet`, `AeaSet`, `FgiSet`, `JointSet`,
// `NatoDissemSet`, `ClassificationLattice`, `NatoClassLattice`, and
// `DeclassifyOnLattice` remain — they are the algebraically-sound site
// for the lattice claim. The per-axis lattices are real; the cross-axis
// composition is structural folding, not a lattice operation.

/// CAPCO's open-vocabulary structural reference.
///
/// Unifies the open-vocab carriers CAPCO ships today — SAR program
/// identifiers, SCI compartment and sub-compartment paths, and FGI
/// tetragraphs. `FactRef::OpenVocab(CapcoOpenVocabRef)` in
/// `marque-rules` names a token in the projected fact set by its
/// structural form, never by raw input bytes.
///
/// Each variant carries the *canonicalize-produced* structural value
/// (a SAR program ID value, a tetragraph code) — never source-buffer
/// surgery payloads. This preserves the audit content-ignorance
/// invariant (Constitution V): an `AppliedFix` referring to a CAPCO
/// open-vocab token stores a typed structural reference, not document
/// content.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CapcoOpenVocabRef {
    /// A SAR program identifier (CAPCO-2016 §H.5).
    Sar(Box<str>),
    /// An SCI compartment name (CAPCO-2016 §A.6 / §H.4).
    SciCompartment(Box<str>),
    /// An SCI sub-compartment name (CAPCO-2016 §A.6 / §H.4).
    SciSubCompartment(Box<str>),
    /// An FGI tetragraph (CAPCO-2016 §H.3 / ISMCAT Tetragraph Taxonomy).
    FgiTetragraph(Box<str>),
    /// A REL TO country code or country-group (CAPCO-2016 §H.3 / §H.8).
    ///
    /// Carries the structural [`marque_ism::CountryCode`] value
    /// (16-byte fixed buffer, no heap) already produced by the parser,
    /// never raw input bytes — preserves the audit content-ignorance
    /// invariant (Constitution V). The JOINT-coverage rule (JOINT
    /// participants require REL TO coverage, §H.3 p57) consumes this on
    /// the CAT_REL_TO axis, emitting one `FactAdd { CountryCode(...),
    /// Scope::Portion }` per missing JOINT co-owner.
    CountryCode(marque_ism::CountryCode),
}

// ---------------------------------------------------------------------------
// Convenience: expose the classification level for test assertions
// ---------------------------------------------------------------------------

impl CapcoMarking {
    /// The effective US classification level, if any. Thin shim over
    /// `CanonicalAttrs::us_classification` for test readability.
    #[inline]
    pub fn classification(&self) -> Option<Classification> {
        self.0.us_classification()
    }
}
