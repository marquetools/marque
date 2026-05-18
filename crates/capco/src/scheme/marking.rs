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
//! equivalence on the expanded per-axis domains. PR 4b-D.2 Copilot R1
//! review surfaced this gap (decisions.md D24): `CapcoMarking`'s
//! prior `JoinSemilattice` impl violated structural-`Eq` idempotence
//! whenever a per-axis lattice normalized its input (the load-bearing
//! case was `RelToBlock`'s tetragraph expansion: `m.rel_to = [NATO]`
//! → after `m.join(m)` → `m.rel_to = {30 expanded trigraphs}`, so
//! `m != m.join(m)` under derived `Eq`). The fix was to drop the
//! false trait claim; the cross-axis fold remains accessible as the
//! inherent method `join_via_lattice`. See `marque-applied.md` §3 for
//! the "per-axis lattices are real; the cross-axis composition is
//! structural folding, not a lattice operation" framing.
//!
//! ## Post-PR-4b-D.2 production pipeline
//!
//! `CapcoScheme::project(Scope::Page, ...)` runs the full
//! `join_via_lattice → closure → page_rewrites` pipeline (see
//! `docs/plans/2026-05-01-lattice-design.md` §4.7.4).
//!
//! Page state is the engine's inline `Vec<CanonicalAttrs>`
//! accumulator (PR 6c retired the historical `PageContext` newtype
//! wrapper). Pre-PR-4b-E this carried an `expected_*` accessor
//! surface; PR 4b-E migrated the five former residue accessors
//! (`expected_sci_controls`, `expected_fgi_marker`,
//! `expected_declass_exemption`, `expected_non_ic_dissem`,
//! `expected_display_only`) to free helpers and lattice constructors
//! in `crates/capco/src/lattice.rs` (`sci_controls_from_markings`,
//! `FgiSet::from_attrs_iter`,
//! `DeclassExemptionAccumulator::from_attrs_iter`,
//! `NonIcDissemSet::from_attrs_iter`,
//! `DisplayOnlyBlock::from_attrs_iter`). PR 4b-F retired the last
//! `&PageContext` parameter from the lattice fold body; the pipeline
//! consumes `&[CanonicalAttrs]` end-to-end.
//!
//! Carved out from `scheme/mod.rs` per the Stage 2 PR B hub-split
//! (issue #466). Imports reach helpers via `super::actions::*` /
//! `super::predicates::*` / `super::constraints::*` (the same glob
//! pattern `mod.rs` used pre-split).

use marque_ism::{CanonicalAttrs, Classification, MarkingClassification};
// `JoinSemilattice` stays in scope for the per-axis lattice types
// (`SarSet::join`, `FgiSet::join`) that the cross-axis fold composes.
// `CapcoMarking` itself no longer implements `JoinSemilattice`
// (decisions.md D24); the lattice claim lives on the per-axis types,
// not on the cross-axis fold.
use marque_scheme::JoinSemilattice;

use super::actions::*;

/// CAPCO marking as viewed through the `marque-scheme` lens. A thin
/// newtype around [`CanonicalAttrs`] so we can hang trait impls on it
/// without orphan-rule problems.
///
/// # ⚠️ Phase A scaffolding — do not use in production
///
/// `CapcoMarking` is exported publicly so the Phase A equivalence
/// tests can construct it, but it **does not uphold the [`Lattice`]
/// contract** on every input (see the caveat block on the `Lattice`
/// impl below). Downstream consumers must not rely on `Lattice::join`
/// / `Lattice::meet` of `CapcoMarking` producing law-abiding results
/// until Phase B replaces the impl with a proper product-lattice
/// aggregator. Use [`crate::capco_rules`] and `marque-core` directly
/// for production paths.
///
/// # Decoder provenance side channel (Phase 4 PR-4b)
///
/// Tuple-position 1 is an optional [`DecoderProvenance`] populated by
/// the Phase D probabilistic recognizer. Strict-path recognizers leave
/// it `None`. The engine reads `provenance.is_some()` to detect "this
/// recognition went through the decoder fallback" and emits a
/// synthetic `R001 decoder-recognition` diagnostic with
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
    /// Convenience constructor that mirrors the pre-PR-4b tuple-struct
    /// literal `CapcoMarking(attrs)`. Use this in tests and
    /// strict-path recognizers; the decoder constructs the marking by
    /// setting tuple-position 1 directly when it has provenance to
    /// attach.
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
    /// declass_exemption, non_ic_dissem, display_only). Originally
    /// introduced in PR 4b-B Commit 7 alongside a `_with_context`
    /// fast-path variant; PR 4b-D.2 flipped the production
    /// `MarkingScheme::project` to drive page aggregation through this
    /// path, and PR 4b-F collapsed the `_with_context` fast-path now
    /// that no caller threads a `&PageContext` through the body.
    ///
    /// Both call surfaces — the trait-path
    /// `MarkingScheme::project(Scope::Page, ...)` and the engine
    /// fast-path `CapcoScheme::project_from_attrs_slice` — delegate
    /// through `CapcoScheme::project_attrs_pipeline`, which calls
    /// this method directly. The parity gate at
    /// `crates/capco/tests/lattice_vs_scheme_parity.rs` pins the
    /// `per-axis lattice` ↔ `full scheme pipeline` byte-identity
    /// claim across 74 fixtures (with documented divergences for the
    /// closure-rule and PageRewrite-catalog operations that the per-axis
    /// path does not yet model).
    ///
    /// Authority (verified 2026-05-15): per-axis citations are on each
    /// `lattice` module type's doc comment.
    pub fn join_via_lattice(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
        Self::join_via_lattice_body(portions)
    }

    /// Shared body for the `join_via_lattice` entry point.
    ///
    /// Composes per-axis lattice results across 10+ axes
    /// (classification + JointSet, SciSet, SarSet, AeaSet, FgiSet,
    /// DissemSet, NatoDissemSet, RelToBlock, DeclassifyOnLattice,
    /// declass_exemption, non_ic_dissem, display_only). The body now
    /// consumes only `portions: &[CanonicalAttrs]` — the residue-axis
    /// `PageContext` bridge that earlier PRs threaded through this
    /// function retired in PR 4b-E (free helpers in
    /// `crates/capco/src/lattice.rs`) and PR 4b-F (the `_tmp_ctx`
    /// parameter itself).
    ///
    /// ## Size guideline
    ///
    /// Clippy's `too_many_lines` lint fires on this function at
    /// ~420 LOC vs the 100-line default. The structural justification
    /// (axis ordering + inline citations + cross-axis state flow) is
    /// load-bearing — splitting a 400+ LOC cross-axis fold into
    /// per-axis sub-functions would require threading every
    /// intermediate state value via a struct, which pays the
    /// readability cost without the maintainability win.
    ///
    /// - Axis ordering is load-bearing. The G-3 / G-4 / G-4c
    ///   solely-non-US handling, the G-8 NOFORN-supersession overlay,
    ///   and the G-6 SBU-NF/LES-NF NOFORN injection are encoded as
    ///   ordered phases within this function. Each phase reads
    ///   state computed by the prior phase (e.g. `out.classification`
    ///   informs G-4c's foreign-source comparison; `rel_to_was_*`
    ///   flags drive the final DissemSet overlay). Splitting into
    ///   per-axis sub-functions would either (a) require threading
    ///   all the cross-axis state via a struct, paying the
    ///   readability cost it would notionally save, or
    ///   (b) duplicate per-portion walks across sub-function
    ///   boundaries, breaking the §3 (e.1) read-only-attrs
    ///   invariant's audit surface.
    /// - The per-axis citations (`§H.7 pp123-125`, `§H.3 p57`,
    ///   `§H.8 p145`, etc.) live inline alongside the code they
    ///   justify. A split would scatter them across files and harm
    ///   Constitution VIII (citation-fidelity) maintainability.
    ///
    /// Per the PR 4b-D.2 reviewer attestation, future maintainers
    /// hitting `clippy::too_many_lines` here should `#[allow]` rather
    /// than split. The `#[allow]` below is permanent — not a TODO.
    ///
    /// Authority: `docs/plans/2026-05-01-lattice-design.md` §2 (axis
    /// ordering rationale per CAPCO-2016 §G.1 Table 4 p38) +
    /// §11 (PR 4b-B per-axis follow-ups encoded as inline phases).
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
        // page has no US contribution at all. G-3 (PR 4b-B follow-up).
        //
        // G-9 + G-9b (PR 4b-B follow-up): three classification variants
        // are US-bearing for the purposes of the solely-non-US gate:
        //
        // - `Us(_)`: explicit US classification.
        // - `Conflict { us, .. }`: carries an implicit US classification
        //   in the `us` field (see `MarkingClassification::Conflict`
        //   doc comment at `crates/ism/src/attrs.rs:521`). The parser
        //   records "I saw two systems; US wins" — so Conflict is US
        //   from the gate's perspective. Pre-G-9 the lattice path
        //   returned `Conflict(...)` on a Conflict-only page (or
        //   `Nato(_)` on a Conflict+NATO page) while PageContext
        //   returned `Us(level)` — same authority, same §H.7
        //   reciprocal-normalization rule.
        // - `Joint(_)`: by §H.3 p56, USA is required to be in the
        //   producer list (JOINT is US co-owned by definition); JOINT
        //   classifications are therefore US-bearing for the gate.
        //   Pre-G-9b a mixed page like `JOINT C USA GBR + NATO S`
        //   kept `solely_non_us=true` (Joint not counted), so the
        //   NATO portion was preserved as `Nato(_)` rather than
        //   reciprocal-raising to `Us(_)` per §H.7 pp123-125. The
        //   same-level case is the load-bearing one: when the level
        //   chain doesn't already pick a winner via OrdMax, the
        //   variant survival in the per-portion filter loop produces
        //   the wrong banner shape.
        //
        // §-authority: §H.7 pp123-125 (reciprocal-classification rule)
        // + §H.3 p56 (JOINT requires USA in producer list). Verified
        // 2026-05-15 against CAPCO-2016.md.
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
        //   JOINT shape per §H.3 p57. G-3: in this non-JOINT branch,
        //   when the page is NOT solely-non-US, ALSO flatten
        //   Nato(_) / Fgi(_) variants to Us(effective_level) per the
        //   §H.7 pp123-125 reciprocal-raise — preserves PageContext
        //   parity on mixed US+NATO/FGI pages.
        let joint_set = JointSet::from_attrs_iter(portions);
        out.classification = match joint_set.to_marking_classification() {
            Some(mc) => Some(mc),
            None => {
                let filtered: Vec<CanonicalAttrs> = portions
                    .iter()
                    .map(|p| {
                        let mut q = p.clone();
                        match &p.classification {
                            // Always flatten JOINT to its US level in
                            // this non-JOINT branch (§H.3 p57).
                            Some(MarkingClassification::Joint(j)) => {
                                q.classification = Some(MarkingClassification::Us(j.level));
                            }
                            // §H.7 reciprocal-raise: NATO/FGI flatten
                            // to US level when ANY US portion is in
                            // scope. The solely-non-US case keeps the
                            // non-US variant intact.
                            Some(MarkingClassification::Nato(n)) if !solely_non_us => {
                                q.classification =
                                    Some(MarkingClassification::Us(n.us_equivalent()));
                            }
                            Some(MarkingClassification::Fgi(f)) if !solely_non_us => {
                                q.classification = Some(MarkingClassification::Us(f.level));
                            }
                            // G-9 (PR 4b-B follow-up): Conflict always
                            // flattens to its implicit `us` level in
                            // this non-JOINT branch. `Conflict` is the
                            // parser's way of recording "I saw two
                            // classification systems; US wins per
                            // §H.7"; the foreign side rides separately
                            // through the FGI axis, so the
                            // classification axis carries only the US
                            // level here. Authority:
                            // CAPCO-2016 §H.7 pp123-125.
                            Some(MarkingClassification::Conflict { us, .. }) => {
                                q.classification = Some(MarkingClassification::Us(*us));
                            }
                            _ => {}
                        }
                        q
                    })
                    .collect();
                ClassificationLattice::from_attrs_iter(&filtered).into_inner()
            }
        };

        // Axis 2-5: SCI / SAR / AEA / FGI — assemble from per-portion
        // markings via the PR 4b-A precedent constructors. SciSet /
        // AeaSet take `&[Marking]` (flat per-portion union); SarSet
        // takes `Option<&SarMarking>`.
        let sci_markings_concat: Vec<marque_ism::SciMarking> = portions
            .iter()
            .flat_map(|p| p.sci_markings.iter().cloned())
            .collect();
        let sci_set = SciSet::from_markings(&sci_markings_concat);
        out.sci_markings = sci_set.to_markings();

        // Compatibility view: sci_controls is the flat CVE-enum
        // projection. The structural axis above is the authoritative
        // form; PR 4b-E migrated the flat union of per-portion
        // `sci_controls` to the free helper
        // `marque_capco::lattice::sci_controls_from_markings` (which
        // reads `attrs.sci_controls` per portion — the parser-populated
        // CVE projection — not from `out.sci_markings`; the structural
        // roll-up sets `canonical_enum: None` on every output so a
        // project-from-markings path would always return empty).
        // §H.4 p61.
        out.sci_controls = sci_controls_from_markings(portions);

        // SAR: PR 4b-A SarSet operates on a single SarMarking
        // (`sar_markings` field is `Option<SarMarking>`). Join
        // across portions composes per-program by union.
        let mut sar_acc = SarSet::empty();
        for p in portions {
            let part = SarSet::from_marking(p.sar_markings.as_ref());
            sar_acc = sar_acc.join(&part);
        }
        out.sar_markings = sar_acc.to_marking();

        let aea_markings_concat: Vec<marque_ism::AeaMarking> = portions
            .iter()
            .flat_map(|p| p.aea_markings.iter().cloned())
            .collect();
        out.aea_markings = AeaSet::from_markings(&aea_markings_concat).to_markings();

        // FGI marker — compose via FgiSet from per-portion markers
        // AND merge with classification-derived producers
        // (`FgiSet::from_attrs_iter` unions per-portion `fgi_marker`
        // values with classification-derived producers — NATO/JOINT/FGI
        // classification countries are surfaced onto the FGI axis per
        // §H.7 p123 + p128).
        //
        // G-4 (PR 4b-B follow-up): when JointSet is
        // `UnanimousProducers`, the producers are already captured in
        // the JOINT classification — we must NOT also FGI-mark them,
        // because §H.3 p56 + §H.7 p123 say JOINT subsumes the FGI
        // marker for those producers.
        //
        // G-5 (PR 4b-B follow-up): when both an explicit FgiSet
        // marker AND classification-derived producers are present,
        // UNION the producer sets rather than discarding the
        // classification-derived ones.
        let mut fgi_acc = FgiSet::empty();
        for p in portions {
            let part = FgiSet::from_marker(p.fgi_marker.as_ref());
            fgi_acc = fgi_acc.join(&part);
        }
        let ctx_fgi_marker = if matches!(joint_set, JointSet::UnanimousProducers { .. }) {
            // G-4: JOINT-unanimous page — producers ride on the
            // `Joint(_)` classification, not on the FGI axis. Suppress
            // the classification-derived FGI fallback so we don't
            // double-mark (§H.3 p56 + §H.7 p123).
            None
        } else if solely_non_us {
            // G-4b (PR 4b-B 7th-pass follow-up): solely-non-US page
            // where the lattice preserves a `Nato(_)` or `Fgi(_)`
            // classification intact (the §H.7 reciprocal-raise is
            // suppressed earlier in this method when no US portion
            // is present to raise toward). The foreign source is already
            // recorded on the classification axis itself; running
            // `FgiSet::from_attrs_iter` here would derive the SAME
            // producers from the classification a second time and
            // surface them on the dissem-axis `fgi_marker`, producing
            // a doubled marker. The lattice path preserves the
            // foreign variant on the classification axis (per the
            // `pure_nato_both_paths_preserve_nato_variant` parity
            // fixture — renamed from `pure_nato_lattice_vs_pagecontext_diverges`
            // in PR 4b-E review fix-up; §H.7 pp123-125), which makes the FGI-axis
            // duplication redundant.
            //
            // Per-portion `fgi_marker` fields (FgiSet) are still
            // honored — `fgi_acc.to_marker()` is what we ultimately
            // merge with this `None`. The suppression only drops the
            // classification-derived secondary fold.
            //
            // §-authority: §H.7 p123 (FGI source is recorded ONCE per
            // portion; for non-US classifications the source IS the
            // classification axis). Verified 2026-05-15 against
            // CAPCO-2016.md.
            //
            // G-4c (PR 4b-B 9th-pass follow-up): blanket suppression
            // is unsafe when the winner classification's foreign
            // payload is a STRICT SUBSET of all foreign sources
            // contributed by all non-US classification portions. The
            // failure mode:
            //
            //   Inputs:  Fgi(Confidential, [GBR]), Fgi(Secret, [CAN])
            //   ClassificationLattice winner: Fgi(Secret, [CAN])
            //     (OrdMax: Secret > Confidential)
            //   Pre-G-4c: GBR is silently lost from the FGI axis.
            //   The correct §H.7 p124 behavior is to surface both
            //   producers on the FGI axis (source-loss reconstruction).
            //
            // The fix gathers the union of foreign sources from all
            // non-US classification portions, compares against the
            // winner's foreign sources, and:
            //   - if equal: safe to suppress (current G-4b behavior)
            //   - if winner is strict subset: build a synthetic FGI
            //     marker carrying the missing sources so they merge
            //     into `out.fgi_marker` via `merge_fgi_markers`.
            //
            // The C-7 `classification_join_same_variant` UNION
            // tiebreaker covers the same-level case (both producers
            // ride on the winner's payload, suppression remains
            // safe). G-4c only fires when level disagreement made
            // OrdMax discard a foreign source.
            //
            // §-authority: §H.7 p124 (source-concealed-dominance
            // precedence rules at the banner-line guidance block) +
            // §H.7 pp123-125 (FGI source must be preserved across
            // the projection) + §H.7 p128 (concealed-dominates
            // when mixed concealed + acknowledged portions exist).
            // Verified 2026-05-16 against
            // `crates/capco/docs/CAPCO-2016.md`.
            //
            // P-9-2 (9th-pass): `extract_foreign_sources` now returns
            // `Option<Vec<CountryCode>>` where `None` = source-concealed
            // FGI on that portion. If any portion is concealed, the page
            // must carry `FgiMarker::SourceConcealed` (§H.7 p128). Pre-
            // fix, source-concealed portions returned an empty Vec,
            // indistinguishable from "no FGI" — the equality check below
            // then silently dropped the concealed signal and could produce
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
                    // G-4b safe-suppression branch: every foreign source
                    // observed across all portions is preserved on the
                    // winning classification's payload. No source loss.
                    None
                } else {
                    // G-4c source-loss branch: at least one source is
                    // missing from the winner's payload. Build a
                    // synthetic acknowledged FGI marker carrying every
                    // foreign source so `merge_fgi_markers` unions them
                    // into the final output.
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
        // §H.8 p145 (G-8 PR 4b-B follow-up).
        let dissem_set = DissemSet::from_attrs_iter(portions);
        out.dissem_nato = NatoDissemSet::from_attrs_iter(portions).into_boxed_slice();

        // Axis 8: rel_to.
        let rel_to_block = RelToBlock::from_attrs_iter(portions);
        let rel_to_was_noforn_superseded = rel_to_block.is_noforn_superseded();
        // P-2 (8th-pass): also capture the `Empty` variant (disjoint REL TO
        // country lists with no common [LIST] — §D.2 Table 3 row 9) BEFORE
        // `into_boxed_slice()` consumes the discriminant. An `Empty`
        // intersection means no common release audience exists, so the banner
        // MUST carry NOFORN per §D.2 Table 3 row 9.
        //
        // Pre-fix the NOFORN injection only checked
        // `rel_to_was_noforn_superseded` (the `NofornSuperseded` absorbing
        // state) and missed `Empty`. A page with two REL TO portions listing
        // disjoint countries produced an empty `rel_to` slice with no `Nf`
        // injected — wrong per §D.2 Table 3 row 9.
        //
        // §-authority: §D.2 p28-30 Table 3 row 9 (REL TO [USA, LIST] + REL
        // TO [USA, LIST] with no common [LIST] → NOFORN banner).
        // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
        let rel_to_was_empty_intersection = rel_to_block.is_empty_intersection();
        // PR 4b-E: defer `out.rel_to` assignment until after
        // `DisplayOnlyBlock::from_attrs_iter` borrows `rel_to_block`
        // for the row-27 banner-REL-TO subtraction — see below.

        // Axis 9: declassify_on (and declass_exemption rides as
        // last-observed per the existing semantic — Phase 3 TODO
        // carried over on `DeclassExemptionAccumulator`).
        // PR 4b-E: `declass_exemption` migrated from
        // `PageContext::expected_declass_exemption` to the
        // `DeclassExemptionAccumulator::from_attrs_iter` helper. Same
        // semantics; the Phase-3 duration-aware comparator (§E.3 pp
        // 32-33 "longest period of protection") is queued on the
        // accumulator type's doc-comment.
        out.declassify_on = DeclassifyOnLattice::from_attrs_iter(portions).into_inner();
        out.declass_exemption = DeclassExemptionAccumulator::from_attrs_iter(portions).into_inner();

        // Axis 10: non_ic_dissem — classification-gated SBU-NF /
        // LES-NF split (§H.9 p178 / p185) + implied-NF for
        // NODIS / EXDIS (§H.9 p172 / p174).
        //
        // PR 4b-E: `NonIcDissemSet::from_attrs_iter` lifts the same
        // semantics off `PageContext::expected_non_ic_dissem`. The
        // `needs_nf` flag is still consumed at the cross-axis NOFORN
        // injection rendezvous below (G-6 PR 4b-B follow-up):
        // when set, NOFORN is injected into `dissem_us` AND REL TO
        // is cleared.
        let non_ic_set = NonIcDissemSet::from_attrs_iter(portions);
        let needs_nf = non_ic_set.needs_nf();
        out.non_ic_dissem = non_ic_set.into_boxed_slice();

        // DISPLAY ONLY axis (§D.2 Table 3 rows 18-20 + 25-27, §H.8
        // p163). Cross-axis intersection over (REL TO ∪ DO) with
        // banner-REL-TO and USA subtraction.
        //
        // PR 4b-E: the residue migration. The dedicated
        // `DisplayOnlyBlock` lattice (parallel to `RelToBlock`)
        // lifts the §D.2 Table 3 row 18-20 + 25-27 + §H.8 p163
        // semantics out of `PageContext::expected_display_only`.
        // The constructor consumes the pre-computed `rel_to_block`
        // (for row-27 subtraction) and `needs_nf` (for the
        // NODIS/EXDIS short-circuit per §H.9 p172 / p174). NOFORN
        // supersession (§D.2 Table 3 rows 1-2 + §H.8 p145) is
        // applied inside the lattice constructor.
        out.display_only_to =
            DisplayOnlyBlock::from_attrs_iter(portions, &rel_to_block, needs_nf).into_boxed_slice();

        // PR 4b-E: now that DisplayOnlyBlock has consumed its read
        // of `rel_to_block`, materialize `out.rel_to` from the same
        // value. (Deferred from the §H.8 p150-151 / §D.2 Table 3
        // row 9 computation above so a single RelToBlock value
        // serves both consumers.)
        out.rel_to = rel_to_block.into_boxed_slice();

        // NOFORN-clears-REL-TO interaction + cross-axis NOFORN
        // injection.
        //
        // G-8 (PR 4b-B follow-up): when NOFORN must be injected from
        // a cross-axis source (non-IC SBU-NF/LES-NF on a classified
        // page, or NODIS/EXDIS supersession via RelToBlock), the
        // injection MUST route through `DissemSet::with_noforn_injected`
        // so the §H.8 p145 NOFORN-dominates overlay strips any
        // `Rel` / `Relido` / `Displayonly` that survived from the
        // per-portion union. Pre-G-8 the injection inserted `Nf`
        // into `out.dissem_us` directly, after `DissemSet::
        // into_boxed_slice` had already run — invalid output per
        // §H.8 p145.
        //
        // Authority: §H.8 p145 (NOFORN dominates REL TO / RELIDO /
        // EYES ONLY / DISPLAY ONLY) + §D.2 Table 3 rows 1-2 +
        // §H.9 p172 (NODIS) / §H.9 p174 (EXDIS) inject NOFORN at
        // banner.
        // P-2 (8th-pass): include the `Empty` intersection case alongside
        // `NofornSuperseded` — both require NOFORN injection per §D.2
        // Table 3 row 9 (Empty) and rows 1-2 / §H.9 p172/p174 (NofornSuperseded).
        let dissem_final =
            if rel_to_was_noforn_superseded || rel_to_was_empty_intersection || needs_nf {
                // G-6: SBU-NF / LES-NF on a classified page also clears
                // REL TO — match PageContext::expected_rel_to which
                // short-circuits to an empty slice when needs_nf fires.
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

// PR 4b-D.2 status note on the `Lattice` impl
// -------------------------------------------
//
// PR 4b-B (006 T112) installed per-category Lattice impls in
// `marque-capco::lattice` for every CAPCO axis (Classification,
// NatoClass, Joint, Dissem, NatoDissem, RelToBlock, DeclassifyOn,
// plus the PR 4b-A AeaSet / SciSet / SarSet / FgiSet). The
// component-wise composition is exposed on `CapcoMarking::
// join_via_lattice()` above.
//
// PR 4b-D.2 Copilot R1 / decisions.md D24
// -----------------------------------------------------------------
//
// The `impl JoinSemilattice for CapcoMarking` and
// `impl MeetSemilattice for CapcoMarking` blocks were dropped in
// PR 4b-D.2 Commit 11. Copilot's R1 review surfaced an idempotence-
// law violation on `JoinSemilattice::join` driven by tetragraph
// expansion in `RelToBlock::from_attrs_iter`:
//
//   let m = CapcoMarking::new(CanonicalAttrs {
//       rel_to: [CountryCode::NATO].into(),
//       ..
//   });
//   let joined = m.join(&m);
//   // Pre-fix: joined.0.rel_to is the 30-trigraph NATO expansion,
//   // NOT [NATO]. Structural Eq fails: `m != joined`.
//
// The lattice consultant ruled (Option D-extended) that:
//
//   1. Per-axis lattices (`RelToBlock`, `DissemSet`, `SciSet`,
//      `SarSet`, etc.) ARE sound lattices on their native domains
//      (e.g. `2^{Trigraph}` for REL TO). Idempotence holds on the
//      lattice type's own structural `Eq`, which compares the
//      expanded representative.
//   2. `CapcoMarking` is a **cross-axis fold** of those lattice
//      values back into a `CanonicalAttrs` record. The fold is a
//      *projection*, not a join. Claiming `JoinSemilattice` on the
//      record type promised a law (structural-`Eq` idempotence)
//      that the construction could not keep without either lossy
//      eager canonicalization at construction (would erase the
//      `NATO` atom from the renderer's input form) or a
//      quotient-`Eq` rewrite across all `CanonicalAttrs` fields
//      (massive blast radius). Both options were rejected.
//
// The cross-axis-fold entry remains accessible as the inherent
// method `CapcoMarking::join_via_lattice` above (PR 4b-F collapsed
// the earlier `_with_context` fast-path variant once no body in the
// chain still read a `&PageContext`). Engine and scheme call sites
// that used `<CapcoMarking as JoinSemilattice>::join` previously now
// call the inherent method directly. The `MarkingScheme::Marking`
// trait bound was also relaxed in `crates/scheme/src/scheme.rs` to
// remove the false claim at the trait surface.
//
// `MeetSemilattice for CapcoMarking` was dropped for the same
// algebraic reason — the implementation was a "partial component-
// wise minimum" (its own doc comment said so) that did not satisfy
// the meet laws on the cross-axis record type either. The trait
// claim was unsound; no production caller depended on it.
//
// Per-axis `JoinSemilattice` / `MeetSemilattice` impls on
// `RelToBlock`, `DissemSet`, `SciSet`, `SarSet`, `AeaSet`,
// `FgiSet`, `JointSet`, `NatoDissemSet`, `ClassificationLattice`,
// `NatoClassLattice`, and `DeclassifyOnLattice` remain — they are
// the algebraically-sound site for the lattice claim.
//
// See `marque-applied.md` §3 (PR 3b stall walkthrough) for the
// "per-axis lattices are real; the cross-axis composition is
// structural folding, not a lattice operation" framing. The
// systematic audit of remaining per-axis types for similar
// structural-vs-lattice-`Eq` mismatches
// (`DissemSet::relido_observed_unanimous`, `JointSet::Mixed` /
// `DisunityCollapse`, `SupersessionSet`) is tracked as a follow-up
// issue, NOT addressed by PR 4b-D.2.

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
/// surgery payloads. This preserves the G13 audit-content-ignorance
/// invariant (Constitution V Principle V): an `AppliedFix` referring
/// to a CAPCO open-vocab token stores a typed structural reference,
/// not document content.
///
/// PR 3c.B Commit 2 stubs the variant set with one nominal variant
/// per category. Construction sites (canonicalize-side population of
/// these references) land in Commit 6 alongside the rule migration.
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
    /// never raw input bytes — preserves the G13 audit-content-
    /// ignorance invariant (Constitution V Principle V). Wired by
    /// PR 3c.B Sub-PR 8.D.4 as the first open-vocab consumer of the
    /// CAT_REL_TO axis: E014 (JOINT participants require REL TO
    /// coverage, §H.3 p57) emits one `FactAdd { CountryCode(...),
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
