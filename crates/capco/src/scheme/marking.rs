// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoMarking` — the newtype over `CanonicalAttrs` and its impls.
//!
//! Holds the `CapcoMarking` tuple struct (with optional decoder
//! provenance side channel), `PartialEq`/`Eq`/`From<CanonicalAttrs>`
//! impls, the inherent block carrying `new` + the 486-LOC
//! `join_via_lattice` lattice-path composer, the `Lattice` trait impl
//! (which currently delegates `join` to `PageContext::add_portion`
//! per PR 4b-B's plan), the `CapcoOpenVocabRef` open-vocab enum, and
//! the test-convenience `classification()` accessor.
//!
//! Carved out from `scheme/mod.rs` per the Stage 2 PR B hub-split
//! (issue #466). Module contents are byte-identical to the pre-split
//! source — imports adjusted to reach helpers via `super::actions::*`
//! / `super::predicates::*` / `super::constraints::*` (the same glob
//! pattern `mod.rs` used pre-split).

use marque_ism::{CanonicalAttrs, Classification, MarkingClassification, PageContext};
use marque_scheme::Lattice;

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

    /// **PR 4b-B Commit 7** — component-wise join via the per-category
    /// `marque-capco::lattice` types.
    ///
    /// This is the new "lattice path" exposed alongside the existing
    /// `Lattice::join` impl (which still delegates to `PageContext`).
    /// The parity-gate test
    /// `crates/capco/tests/page_context_lattice_parity.rs` (Commit 8)
    /// proves byte-identity between the two paths across 51 `#[test]`
    /// fixtures with **six documented divergences** (enumerated in
    /// `crates/capco/CAPCO-CONTEXT.md` §3): G-1 FOUO-classified, G-2
    /// AEA-UCNI-classified, G-3 pure-NATO, the
    /// RELIDO+NOFORN-dominates correctness divergence, plus the two
    /// pure-JOINT cases (`joint_unanimous_two_portions` /
    /// `joint_single_portion_no_us`) where the lattice produces
    /// `Joint(_)` per §H.3 p56 banner-fidelity and PageContext
    /// produces `Us(_)`. G-4..G-9 land as parity-RESTORING fixtures
    /// (each cited inline against its §). Corpus-fixture coverage
    /// is deferred to PR 4b-D when
    /// `CapcoScheme::project(Scope::Page, ...)` flips to use this
    /// path.
    ///
    /// **Two residues** preserved from PageContext for one more PR:
    ///
    /// 1. `non_ic_dissem` axis (classification-gated SBU-NF/LES-NF
    ///    split + the implied-NF injection family). Documented in
    ///    the plan §3.3 as a `Constraint::Custom("capco/fouo-eviction")`
    ///    PR 4b-C migration target. The `needs_nf` flag is propagated
    ///    into `out.dissem_us` (G-6 PR 4b-B follow-up) so SBU-NF /
    ///    LES-NF classified pages produce the correct NOFORN
    ///    injection on the lattice path.
    /// 2. The JOINT non-US producer FGI migration — Commit 5's
    ///    `JointSet::DisunityCollapse` carries the producer set,
    ///    and the W004 rule (Commit 9) surfaces it, but the
    ///    renderer-canonical FGI attribution is PR 5+ Stage 4
    ///    territory.
    ///
    /// Authority (verified 2026-05-15): per-axis citations are on
    /// each `lattice` module type's doc comment.
    pub fn join_via_lattice(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
        use crate::lattice::{
            AeaSet, ClassificationLattice, DeclassifyOnLattice, DissemSet, FgiSet, JointSet,
            NatoDissemSet, RelToBlock, SarSet, SciSet,
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
                            // this non-JOINT branch. PageContext's
                            // `expected_classification` uses
                            // `effective_level()` over Conflict, which
                            // returns the `us` field, and wraps the
                            // result in `Us(_)`. The lattice path
                            // matches that semantic: Conflict is the
                            // parser's way of recording "I saw two
                            // classification systems; US wins per
                            // §H.7"; the foreign side rides separately
                            // through the FGI axis. Authority:
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

        // Build a temporary PageContext for the axes that PR 4b-B
        // deliberately leaves on the PageContext path (see "two
        // residues" above) plus for the SCI compatibility view.
        let mut tmp_ctx = PageContext::new();
        for p in portions {
            tmp_ctx.add_portion(p.clone());
        }

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
        // form; we re-derive sci_controls via the existing PageContext
        // shape so the parity gate compares both forms.
        out.sci_controls = tmp_ctx.expected_sci_controls().into_boxed_slice();

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
        // (PageContext::expected_fgi_marker unions NATO/JOINT/FGI
        // classification countries into the same axis).
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
            // the PageContext FGI fallback so we don't double-mark
            // (§H.3 p56 + §H.7 p123).
            None
        } else if solely_non_us {
            // G-4b (PR 4b-B 7th-pass follow-up): solely-non-US page
            // where the lattice preserves a `Nato(_)` or `Fgi(_)`
            // classification intact (the §H.7 reciprocal-raise was
            // suppressed at scheme.rs:354 because there was no US
            // portion to raise toward). The foreign source is already
            // recorded on the classification axis itself; calling
            // `expected_fgi_marker()` here would derive the SAME
            // producers from the classification a second time and
            // surface them on the dissem-axis `fgi_marker`, producing
            // a doubled marker.
            //
            // PageContext doesn't have this problem because its
            // `expected_classification` ALWAYS wraps in `Us(_)`
            // regardless of source — the foreign-source info has to
            // ride on `expected_fgi_marker` since it can't ride on the
            // classification axis. The lattice path preserves the
            // foreign variant on the classification axis (per the
            // documented `pure_nato_lattice_vs_pagecontext_diverges`
            // divergence, §H.7 pp123-125), which makes the FGI-axis
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
            //   PageContext path preserves both via its
            //   `expected_fgi_marker` union.
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
            tmp_ctx.expected_fgi_marker()
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
        // Pre-fix the NOFORN injection at line ~662 only checked
        // `rel_to_was_noforn_superseded` (the `NofornSuperseded` absorbing
        // state) and missed `Empty`. A page with two REL TO portions listing
        // disjoint countries produced an empty `rel_to` slice with no `Nf`
        // injected — wrong per §D.2 Table 3 row 9.
        //
        // §-authority: §D.2 p28-30 Table 3 row 9 (REL TO [USA, LIST] + REL
        // TO [USA, LIST] with no common [LIST] → NOFORN banner).
        // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
        let rel_to_was_empty_intersection = rel_to_block.is_empty_intersection();
        out.rel_to = rel_to_block.into_boxed_slice();

        // Axis 9: declassify_on (and declass_exemption rides as
        // last-observed per the existing PageContext semantic for
        // now — Phase 3 TODO at page_context.rs:639).
        out.declassify_on = DeclassifyOnLattice::from_attrs_iter(portions).into_inner();
        out.declass_exemption = tmp_ctx.expected_declass_exemption();

        // Residue 1: non_ic_dissem — classification-gated SBU-NF/
        // LES-NF split + implied-NF stays on PageContext for one
        // more PR (PR 4b-C migration target).
        //
        // G-6 (PR 4b-B follow-up): propagate `needs_nf` from
        // `expected_non_ic_dissem`. When set, inject NOFORN into
        // `dissem_us` AND clear REL TO — matches
        // PageContext::expected_dissem_us step 4 + the implicit
        // REL TO clear via §H.9 p178 (SBU-NF) / §H.9 p185 (LES-NF).
        // Pre-fix, the lattice path ignored this flag and a
        // classified page with REL TO + SBU-NF / LES-NF kept REL TO
        // and missed NOFORN.
        let (non_ic, needs_nf) = tmp_ctx.expected_non_ic_dissem();
        out.non_ic_dissem = non_ic.into_boxed_slice();

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

// Phase B status note on the `Lattice` impl
// -----------------------------------------
//
// PR 4b-B (006 T112) installs per-category Lattice impls in
// `marque-capco::lattice` for every CAPCO axis (Classification,
// NatoClass, Joint, Dissem, NatoDissem, RelToBlock, DeclassifyOn,
// plus the PR 4b-A AeaSet / SciSet / SarSet / FgiSet). The
// component-wise composition is exposed on `CapcoMarking::
// join_via_lattice()` below — the new code path.
//
// The trait `Lattice::join` impl below STILL DELEGATES TO
// `PageContext::add_portion` + `page_context_to_attrs`. This is
// deliberate per the operative plan
// `docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md` §3.2:
// PR 4b-B installs the joins and the parity gate (Commit 8) proves
// byte-identity against the PageContext path. PR 4b-D flips the
// production hot path to use the lattice joins; until then, the
// PageContext delegation remains authoritative so the corpus +
// rule-set test surface stays bit-stable.
//
// Two residues for the eventual flip are documented inline in
// `join_via_lattice`:
//
// - `non_ic_dissem` axis — cross-axis classification-gated splits
//   (SBU-NF / LES-NF in classified docs) stay on PageContext for
//   one more PR. The §3 (b) FOUO eviction matrix migrates via
//   `Constraint::Custom("capco/fouo-eviction")` in PR 4b-C.
// - JOINT producer-disunity FGI migration — the `JointSet`
//   produces `DisunityCollapse` state with the non-US producer set;
//   the W004 Warn rule (registered Commit 9) surfaces it, but the
//   FGI-attribution rewrite is renderer-canonical territory
//   (PR 5+ Stage 4).
//
// `meet` keeps its narrow PageContext-free shape — it's used by a
// small set of overlap-check call sites that do not need full
// component-wise coverage. PR 4b-D widens it when `project` flips.
impl Lattice for CapcoMarking {
    /// Join = banner-aggregate both portions via `PageContext`.
    ///
    /// Delegates to [`PageContext`] so the scheme's join is
    /// definitionally equivalent to the existing hand-written
    /// aggregation on the inputs exercised by Phase A's tests. Phase B
    /// inverts this dependency — `PageContext` will be implemented in
    /// terms of component-wise aggregation, and this method will stop
    /// applying the projection's non-invertible normalizations.
    ///
    /// See the module-level "Phase A caveat" note above for the
    /// specific laws this impl does not satisfy.
    #[inline]
    fn join(&self, other: &Self) -> Self {
        let mut ctx = PageContext::new();
        ctx.add_portion(self.0.clone());
        ctx.add_portion(other.0.clone());
        CapcoMarking::new(page_context_to_attrs(&ctx))
    }

    /// Meet = partial component-wise minimum.
    ///
    /// Implemented only on classification, SCI, and dissem — enough to
    /// satisfy the trait bound and serve Phase A's test inputs. All
    /// other fields reset to `Default`. This is not a full
    /// product-lattice meet; see the module-level "Phase A caveat"
    /// note above. Phase B replaces it with a proper component-wise
    /// meet across every category.
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        let a = &self.0;
        let b = &other.0;

        let classification = match (&a.classification, &b.classification) {
            (Some(x), Some(y)) => {
                let min = x.effective_level().min(y.effective_level());
                Some(marque_ism::MarkingClassification::Us(min))
            }
            _ => None,
        };

        let sci: Vec<_> = a
            .sci_controls
            .iter()
            .filter(|t| b.sci_controls.contains(t))
            .copied()
            .collect();
        // PR 9b (T132): meet operates component-wise on each dissem
        // namespace independently. The two fields share the
        // `DissemControl` type but live on opposite sides of the
        // CAPCO-2016 p41 reciprocity boundary; mixing them would
        // collapse the namespace distinction.
        let dissem_us: Vec<_> = a
            .dissem_us
            .iter()
            .filter(|t| b.dissem_us.contains(t))
            .copied()
            .collect();
        let dissem_nato: Vec<_> = a
            .dissem_nato
            .iter()
            .filter(|t| b.dissem_nato.contains(t))
            .copied()
            .collect();

        let mut out = CanonicalAttrs::default();
        out.classification = classification;
        out.sci_controls = sci.into_boxed_slice();
        out.dissem_us = dissem_us.into_boxed_slice();
        out.dissem_nato = dissem_nato.into_boxed_slice();
        CapcoMarking::new(out)
    }
}

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
