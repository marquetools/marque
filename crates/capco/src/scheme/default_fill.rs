// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #704 — "default if absent" pre/post-closure fill stage.
//!
//! # What this module is for
//!
//! Pre-#704 the CAPCO closure catalog packed four "default if absent"
//! rules (Rows 0/7/8/9 in `closure_table::CLOSURE_TABLE`) alongside
//! six "unconditional implication" rules (Rows 1-6) into a single
//! Kleene fixpoint over `close()`. The default-if-absent rules use
//! a `suppressor_mask` to gate firing on "no FD&R already in the
//! marking" — which is **not a closure semantic**: the suppressor
//! makes the firing predicate non-monotone (adding bits via
//! `b_extra` can activate a suppressor and strictly lose cone bits
//! from `close(b)` vs `close(a)`), violating the closure operator's
//! algebraic monotonicity property `a ⊑ b ⟹ Cl(a) ⊑ Cl(b)`.
//!
//! The proptest `crates/capco/tests/proptest_closure_table.rs::p3_monotonicity_*`
//! shrinks to a minimal counterexample on the pre-#704 architecture.
//! Issue #704 retires the suppressor architecture and relocates the
//! default-if-absent rules HERE, into a post-close stage that is
//! deliberately non-monotone — by CAPCO design, the §B.3 / §H.7 /
//! §H.8 "default if absent" rules ARE non-monotone, and packing them
//! into a closure-shaped operator was the architectural error.
//!
//! Pipeline order:
//!
//! ```text
//! parse → join_via_lattice → close() (Rows 1-6, additive Kleene fixpoint)
//!                          → apply_default_fill (HERE; Rows 0/7/8/9)
//!                          → apply_supersession_overlays (§H.8 p145 input-explicit contradictions)
//!                          → PageRewrites → render
//! ```
//!
//! The default-fill runs AFTER `close()` so the per-marking
//! unconditional implications (e.g., `SI-G → ORCON` via Row 3) are
//! visible to the default-fill's "no FD&R present?" gate. Pre-#704
//! the chain worked because Row 0 was inside the Kleene loop and saw
//! Row 3's ORCON addition in the next iteration; the post-#704
//! post-close placement preserves this by observing the closed state.
//!
//! # The §-evidence
//!
//! §B.3 paragraph b p19 (verbatim):
//! > "IC classifiers must apply FD&R marking(s) when reusing or
//! > derivatively sourcing into an IC DAP classified information that
//! > was **not marked previously** by the originator..."
//!
//! §B.3 introductory p19 (verbatim):
//! > "When reusing information from a source document(s) that has FD&R
//! > markings, **carry forward** the FD&R markings from the source
//! > document(s)."
//!
//! §B.3.d p20 (verbatim, FGI-specific):
//! > "If the originating country allows further sharing by the United
//! > States, a REL TO USA, [LIST] marking **must be used**... When
//! > derivatively sourcing FGI that **does not have FD&R marking(s)**
//! > in a classified or controlled unclassified IC DAP, it must be
//! > marked as NOFORN in the absence of a positive release
//! > determination by the originating agency or source country."
//!
//! Combined reading: §B.3 Table 2 p21 + §B.3.d p20 + §H.8 p154 are
//! "default-if-absent" rules. The trigger is "input lacks explicit
//! FD&R"; when input HAS explicit FD&R, the default does NOT apply
//! — the explicit marking is preserved. Re-verified at authorship
//! against `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
//!
//! # Per-row default-fill predicates
//!
//! Each row inspects the **post-`close()` bitmask** (so the
//! Rows-1-6 chain additions like SI-G→ORCON are visible to the
//! "FD&R-present?" gate). Each gate uses an upward-closed
//! presence-check on the post-close state.
//!
//! | Row | §-anchor                          | Trigger                              | FD&R-absent gate                       | Effect when both true             |
//! |-----|-----------------------------------|---------------------------------------|----------------------------------------|-----------------------------------|
//! | 0   | §B.3 Table 2 p21                  | caveated triggers (20 atoms)         | `MASK_FDR_DOMINATORS == 0`             | add `Nf` to `attrs.dissem_us`     |
//! | 7   | §H.7 p127 + §G.2 Table 5 p40      | `NATO_CLASS`                          | `MASK_FDR_DOMINATORS == 0`             | add USA + NATO to `attrs.rel_to`  |
//! | 8   | §H.8 p154                         | `SCI_PRESENT`                         | `MASK_FDR_OR_RELIDO_INCOMPAT == 0`     | add `Relido` to `attrs.dissem_us` |
//! | 9   | §B.3 Table 2 p21 + §H.8 p154      | `US_COLLATERAL_CLASSIFIED`           | `MASK_RELIDO_US_CLASS_SUPPRESSORS == 0` | add `Relido` to `attrs.dissem_us` |
//!
//! Row 7's FGI-specific authority (§B.3.d p20 "REL TO USA, [LIST]
//! must be used") + §H.7 p123 "FGI may include US FD&R as
//! circumstances warrant" both ground the FGI corner of the same
//! default-fill: if FGI input has REL TO, default-fill's gate
//! fails and the user-explicit REL TO is preserved per §H.7 p124's
//! REL TO grammar.
//!
//! # Why this isn't a closure
//!
//! `close()` is required to be monotone on the marking lattice
//! (`MarkingScheme::closure` trait contract). The default-fill is
//! deliberately not monotone — adding an FD&R bit to the input
//! flips the gate from "fire" to "skip" and strictly removes
//! default cone bits from the output. That's by §-design.
//! Separating the two stages lets each carry its honest algebraic
//! contract.

use marque_ism::{CanonicalAttrs, CountryCode};

use crate::fact_bitmask::{
    apply_closed_bits_to, derive_bits, fact_bit, MASK_FDR_DOMINATORS,
    MASK_FDR_OR_RELIDO_INCOMPAT, MASK_RELIDO_US_CLASS_SUPPRESSORS,
};
use marque_scheme::FactBitmask;

// ---------------------------------------------------------------------------
// Trigger masks
// ---------------------------------------------------------------------------

/// Trigger mask for Row 0 (`capco/noforn-if-caveated`).
///
/// Bitmask form of the pre-#704 `CLOSURE_NOFORN_CAVEATED.triggers`
/// from the retired fn-pointer catalog:
///
/// - 1 SAR — `AnyInCategory(CAT_SAR)`
/// - 5 AEA — `TOK_RD`, `TOK_FRD`, `TOK_TFNI`, `TOK_UCNI`, `TOK_DCNI`
/// - 2 FGI — `TOK_FGI_MARKER` + `AnyInCategory(CAT_FGI_MARKER)`
///   (the dissem-axis `fgi_marker` AND the classification-axis
///   `MarkingClassification::Fgi(_)` paths both project to
///   `fact_bit::FGI_PRESENT`)
/// - 8 IC dissem — `TOK_ORCON`, `TOK_ORCON_USGOV`, `TOK_RSEN`,
///   `TOK_IMCON`, `TOK_PROPIN`, `TOK_DSEN`, `TOK_FISA`, `TOK_RAWFISA`
/// - 5 non-IC dissem — `TOK_LIMDIS`, `TOK_LES`, `TOK_NNPI`,
///   `TOK_SBU`, `TOK_SSI`
///
/// Total: 21 `TokenRef` entries on the original fn-pointer rule
/// collapse to 20 atom bits (the two FGI predicate forms project to
/// the single `FGI_PRESENT` sentinel).
///
/// Authority: §B.3 Table 2 p21 (caveated-default → NOFORN); §B.3
/// p20 Note (structural definition of "caveated" — AEA / SAP / IC
/// or non-IC dissem control); §B.3 paragraph b p19 ("NOT MARKED
/// PREVIOUSLY" gate); §H.5 p101 (SAR is a Special Access Program
/// — a caveat per §B.3 p20 Note); §H.6 (RD/FRD/TFNI/UCNI per-marking
/// authority); §H.7 p123 (FGI as caveat trigger); §H.8 pp 132-163
/// (per-IC-dissem caveat templates); §H.9 (non-IC dissem templates).
const ROW0_CAVEATED_TRIGGERS: u128 = (1u128 << fact_bit::SAR_PRESENT)
    | (1u128 << fact_bit::AEA_RD)
    | (1u128 << fact_bit::AEA_FRD)
    | (1u128 << fact_bit::AEA_TFNI)
    | (1u128 << fact_bit::AEA_DOE_UCNI)
    | (1u128 << fact_bit::AEA_DOD_UCNI)
    | (1u128 << fact_bit::FGI_PRESENT)
    | (1u128 << fact_bit::ORCON)
    | (1u128 << fact_bit::ORCON_USGOV)
    | (1u128 << fact_bit::RSEN)
    | (1u128 << fact_bit::IMCON)
    | (1u128 << fact_bit::PROPIN)
    | (1u128 << fact_bit::DSEN)
    | (1u128 << fact_bit::FISA)
    | (1u128 << fact_bit::RAWFISA)
    | (1u128 << fact_bit::LIMDIS)
    | (1u128 << fact_bit::LES)
    | (1u128 << fact_bit::NNPI)
    | (1u128 << fact_bit::SBU)
    | (1u128 << fact_bit::SSI);

/// Trigger mask for Row 7 (`capco/rel-to-usa-nato-if-nato-classification`).
const ROW7_NATO_CLASS_TRIGGER: u128 = 1u128 << fact_bit::NATO_CLASS;

/// Trigger mask for Row 8 (`capco/relido-if-sci-and-not-incompatible`).
const ROW8_SCI_PRESENT_TRIGGER: u128 = 1u128 << fact_bit::SCI_PRESENT;

/// Trigger mask for Row 9 (`capco/relido-if-us-collateral-class`).
const ROW9_US_COLLATERAL_TRIGGER: u128 = 1u128 << fact_bit::US_COLLATERAL_CLASSIFIED;

// ---------------------------------------------------------------------------
// Per-row predicates (pure functions over the post-close bitmask)
// ---------------------------------------------------------------------------

/// Row 0 default-fill predicate.
///
/// `(post_close ∩ ROW0_CAVEATED_TRIGGERS != 0) ∧
///  (post_close ∩ MASK_FDR_DOMINATORS == 0)`.
///
/// Authority: §B.3 Table 2 p21 + §B.3 paragraph b p19.
#[inline]
fn row0_should_fill(post_close: u128) -> bool {
    (post_close & ROW0_CAVEATED_TRIGGERS) != 0 && (post_close & MASK_FDR_DOMINATORS) == 0
}

/// Row 7 default-fill predicate.
///
/// `(post_close ∩ NATO_CLASS != 0) ∧
///  (post_close ∩ MASK_FDR_DOMINATORS == 0)`.
///
/// Authority: §H.7 p127 (NATO worked example) + §G.2 Table 5 p40
/// (alliance-reciprocity ARH grounding) + §B.3 paragraph b p19
/// (FD&R-absent gate).
#[inline]
fn row7_should_fill(post_close: u128) -> bool {
    (post_close & ROW7_NATO_CLASS_TRIGGER) != 0 && (post_close & MASK_FDR_DOMINATORS) == 0
}

/// Row 8 default-fill predicate.
///
/// `(post_close ∩ SCI_PRESENT != 0) ∧
///  (post_close ∩ MASK_FDR_OR_RELIDO_INCOMPAT == 0)`.
///
/// Authority: §H.8 p154 (RELIDO grammar — defaulting marking for
/// IC SCI content absent FD&R); §H.7 p123 + §H.3 p56 + §G.1 Table 4
/// p38 (foreign-equity bar on RELIDO eligibility); §H.4 marking
/// templates for the six SCI sentinels (pp 64 / 68 / 80 / 87 / 91 /
/// 95) — sentinels are excluded because their per-marking
/// implications already drive NOFORN/ORCON and make RELIDO
/// inapplicable.
#[inline]
fn row8_should_fill(post_close: u128) -> bool {
    (post_close & ROW8_SCI_PRESENT_TRIGGER) != 0
        && (post_close & MASK_FDR_OR_RELIDO_INCOMPAT) == 0
}

/// Row 9 default-fill predicate.
///
/// `(post_close ∩ US_COLLATERAL_CLASSIFIED != 0) ∧
///  (post_close ∩ MASK_RELIDO_US_CLASS_SUPPRESSORS == 0)`.
///
/// Authority: §B.3 Table 2 p21 ("uncaveated, on/after 28 Jun 2010 →
/// RELIDO" obligation) + §H.8 p154 (RELIDO grammar) + §B.3
/// paragraph b p19 (FD&R-absent gate).
#[inline]
fn row9_should_fill(post_close: u128) -> bool {
    (post_close & ROW9_US_COLLATERAL_TRIGGER) != 0
        && (post_close & MASK_RELIDO_US_CLASS_SUPPRESSORS) == 0
}

// ---------------------------------------------------------------------------
// apply_default_fill entry point
// ---------------------------------------------------------------------------

/// Apply the "default if absent" cones (Rows 0/7/8/9) to `attrs`
/// after `close()` has converged.
///
/// Reads the post-close bitmask once, evaluates each per-row
/// predicate, and writes any firing cones back to `attrs` via the
/// existing [`apply_closed_bits_to`] writeback path. The writeback
/// path retains its §H.8 p145 NOFORN-dominates strip (when NOFORN
/// is in the cone delta, dominated dissem tokens / `rel_to` /
/// `display_only_to` get stripped) so the dominance semantics
/// remain consistent with the supersession overlay.
///
/// Row 7's open-vocab `CountryCode::NATO` tetragraph is written
/// directly here because the bitmask projection cannot represent it
/// (the `apply_closed_bits_to` path covers the `REL_TO_USA` static
/// half via the `REL_TO_USA` cone bit; the NATO tetragraph rides a
/// separate write).
///
/// # Idempotence
///
/// `apply_default_fill(apply_default_fill(attrs)) == apply_default_fill(attrs)`:
/// once any default fires, the cone bit (NOFORN / RELIDO /
/// REL_TO_USA + NATO) becomes part of `attrs.dissem_us` /
/// `attrs.rel_to`, which lights the corresponding `FD&R_DOMINATORS`
/// bit on the second pass and makes the predicate gate fail. Each
/// row's cone is in the gate's mask, so post-fill the predicate
/// reads `false` for that row.
///
/// # Non-monotonicity (by design)
///
/// `a ⊑ b` does NOT imply `apply_default_fill(a) ⊑ apply_default_fill(b)`.
/// If `b` adds an FD&R bit `a` lacks, the predicate gate flips
/// from "fire" to "skip" and `b`'s output loses the default cone
/// that `a`'s output gained. This is the §-spec design (§B.3
/// paragraph b's "NOT MARKED PREVIOUSLY" gate is inherently
/// anti-monotone) and is why default-fill lives OUTSIDE
/// `MarkingScheme::closure`'s monotone contract.
pub(crate) fn apply_default_fill(attrs: &mut CanonicalAttrs) {
    // Snapshot the post-close bitmask ONCE. Reading per-row would
    // observe intra-stage mutations (Row 0 adds NOFORN; Row 9's
    // FD&R-absent gate then reads NOFORN and skips its add) which
    // would couple row order to outcome. Reading once preserves the
    // pre-#704 semantic "every default-fill row evaluates its gate
    // against the same close()-output state."
    let post_close = derive_bits(attrs).bits();

    // Bitmask short-circuit: if no Row 0/7/8/9 trigger is present,
    // skip the whole stage. The four rows together cover SAR / AEA /
    // FGI / IC dissem caveats / non-IC dissem / NATO classification /
    // SCI presence / US-collateral classification — typical
    // bench-corpus portions hit ≥ 1 of these, but a bare
    // unclassified portion does not.
    const ANY_TRIGGER: u128 = ROW0_CAVEATED_TRIGGERS
        | ROW7_NATO_CLASS_TRIGGER
        | ROW8_SCI_PRESENT_TRIGGER
        | ROW9_US_COLLATERAL_TRIGGER;
    if (post_close & ANY_TRIGGER) == 0 {
        return;
    }

    // Build a cone-bits accumulator. The writeback path handles all
    // §H.8 p145 dominance bookkeeping (strip dominated controls when
    // NOFORN is in the cone delta, clear country lists, dedup).
    let mut cone_delta: u128 = 0;
    if row0_should_fill(post_close) {
        cone_delta |= 1u128 << fact_bit::NOFORN;
    }
    if row7_should_fill(post_close) {
        cone_delta |= 1u128 << fact_bit::REL_TO_USA;
    }
    if row8_should_fill(post_close) {
        cone_delta |= 1u128 << fact_bit::RELIDO;
    }
    if row9_should_fill(post_close) {
        cone_delta |= 1u128 << fact_bit::RELIDO;
    }

    if cone_delta == 0 {
        return;
    }

    // Use the existing apply_closed_bits_to writeback. It expects
    // `input` and `closed` arguments and writes only the delta
    // `(closed & !input) & APPLY_ELIGIBLE_MASK`. Pass `post_close`
    // as input (the state BEFORE default-fill) and `post_close |
    // cone_delta` as closed. The writeback path will deduplicate
    // against any pre-existing dissem tokens and apply the §H.8
    // p145 NOFORN-in-delta strip.
    let input_fb = FactBitmask::from_bits(post_close);
    let closed_fb = FactBitmask::from_bits(post_close | cone_delta);
    apply_closed_bits_to(attrs, closed_fb, input_fb);

    // Row 7 open-vocab tail: if Row 7 fired, ALSO inject
    // `CountryCode::NATO` into `attrs.rel_to`. The bitmask cone bit
    // covers the static USA half (`apply_closed_bits_to` writes USA
    // when `REL_TO_USA` is in the delta); NATO has no closed-vocab
    // sentinel and rides this path separately. Mirrors the pre-#704
    // `CLOSURE_REL_TO_USA_NATO.cone_derived` semantic.
    if (cone_delta & (1u128 << fact_bit::REL_TO_USA)) != 0
        && !attrs.rel_to.contains(&CountryCode::NATO)
    {
        let mut next = Vec::with_capacity(attrs.rel_to.len() + 1);
        next.extend_from_slice(&attrs.rel_to);
        next.push(CountryCode::NATO);
        attrs.rel_to = next.into_boxed_slice();
    }
}

// ---------------------------------------------------------------------------
// Inline unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::{Classification, DissemControl, MarkingClassification};

    fn empty_attrs() -> CanonicalAttrs {
        CanonicalAttrs::default()
    }

    fn us_secret() -> CanonicalAttrs {
        let mut a = empty_attrs();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        a
    }

    #[test]
    fn no_op_on_empty_attrs() {
        let mut attrs = empty_attrs();
        let before = attrs.clone();
        apply_default_fill(&mut attrs);
        assert_eq!(attrs, before);
    }

    #[test]
    fn row0_fires_on_orcon_no_fdr_adds_noforn() {
        // (S, ORCON) — caveat trigger, no FD&R → Row 0 fires.
        let mut attrs = us_secret();
        attrs.dissem_us = vec![DissemControl::Oc].into();
        apply_default_fill(&mut attrs);
        assert!(attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(attrs.dissem_us.contains(&DissemControl::Oc));
    }

    #[test]
    fn row0_suppressed_on_orcon_plus_rel_to() {
        // (S, ORCON, REL TO USA, GBR) — caveat trigger but FD&R
        // present → Row 0 does NOT fire. §H.7 p124 reading
        // preserved.
        let mut attrs = us_secret();
        attrs.dissem_us = vec![DissemControl::Oc].into();
        attrs.rel_to = vec![CountryCode::USA, CountryCode::GBR].into();
        apply_default_fill(&mut attrs);
        assert!(!attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(attrs.rel_to.contains(&CountryCode::USA));
        assert!(attrs.rel_to.contains(&CountryCode::GBR));
    }

    #[test]
    fn row0_suppressed_on_orcon_plus_relido() {
        let mut attrs = us_secret();
        attrs.dissem_us = vec![DissemControl::Oc, DissemControl::Relido].into();
        apply_default_fill(&mut attrs);
        assert!(!attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(attrs.dissem_us.contains(&DissemControl::Relido));
    }

    #[test]
    fn row0_suppressed_on_orcon_plus_displayonly() {
        let mut attrs = us_secret();
        attrs.dissem_us = vec![DissemControl::Oc, DissemControl::Displayonly].into();
        apply_default_fill(&mut attrs);
        assert!(!attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(attrs.dissem_us.contains(&DissemControl::Displayonly));
    }

    #[test]
    fn row0_suppressed_on_orcon_plus_eyes() {
        let mut attrs = us_secret();
        attrs.dissem_us = vec![DissemControl::Oc, DissemControl::Eyes].into();
        apply_default_fill(&mut attrs);
        assert!(!attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(attrs.dissem_us.contains(&DissemControl::Eyes));
    }

    #[test]
    fn row0_suppressed_when_noforn_already_present() {
        let mut attrs = us_secret();
        attrs.dissem_us = vec![DissemControl::Oc, DissemControl::Nf].into();
        let before = attrs.clone();
        apply_default_fill(&mut attrs);
        // Idempotent: nothing changes.
        assert_eq!(attrs.dissem_us, before.dissem_us);
    }

    #[test]
    fn row7_fires_on_bare_nato_adds_usa_and_nato() {
        let mut attrs = empty_attrs();
        attrs.classification = Some(MarkingClassification::Nato(
            marque_ism::NatoClassification::NatoSecret,
        ));
        apply_default_fill(&mut attrs);
        assert!(attrs.rel_to.contains(&CountryCode::USA));
        assert!(attrs.rel_to.contains(&CountryCode::NATO));
    }

    #[test]
    fn row7_suppressed_on_nato_plus_noforn() {
        // (NATO, NOFORN) — FD&R present → Row 7 does NOT fire.
        // The supersession overlay handles the §H.8 p145 conflict
        // separately at a later pipeline stage; default-fill simply
        // doesn't add the implicit default in the first place.
        let mut attrs = empty_attrs();
        attrs.classification = Some(MarkingClassification::Nato(
            marque_ism::NatoClassification::NatoSecret,
        ));
        attrs.dissem_us = vec![DissemControl::Nf].into();
        apply_default_fill(&mut attrs);
        assert!(attrs.rel_to.is_empty());
        assert!(attrs.dissem_us.contains(&DissemControl::Nf));
    }

    #[test]
    fn row8_fires_on_bare_sci_adds_relido() {
        use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};
        use smol_str::SmolStr;
        let mut attrs = us_secret();
        attrs.classification = Some(MarkingClassification::Us(Classification::TopSecret));
        let comp = SciCompartment::new(SmolStr::new("Z9"), Box::new([]));
        attrs.sci_markings = Box::new([SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([comp]),
            None,
        )]);
        apply_default_fill(&mut attrs);
        assert!(attrs.dissem_us.contains(&DissemControl::Relido));
    }

    #[test]
    fn row8_suppressed_on_sci_plus_si_g_compartment() {
        // SI-G is one of the six per-compartment SCI sentinels
        // excluded from Row 8 because its per-marking implication
        // (Row 3 of CLOSURE_TABLE) drives ORCON which then triggers
        // Row 0's caveated → NOFORN default. NOFORN supersedes
        // RELIDO per §H.8 p145 so the Row 8 default would be
        // immediately wrong.
        use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};
        use smol_str::SmolStr;
        let mut attrs = us_secret();
        attrs.classification = Some(MarkingClassification::Us(Classification::TopSecret));
        // Pre-load ORCON to simulate Row 3 (SI-G → ORCON) firing.
        attrs.dissem_us = vec![DissemControl::Oc].into();
        let comp = SciCompartment::new(SmolStr::new("G"), Box::new([]));
        attrs.sci_markings = Box::new([SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([comp]),
            None,
        )]);
        apply_default_fill(&mut attrs);
        // Row 8 suppressed; Row 0 fires on ORCON → NOFORN.
        assert!(!attrs.dissem_us.contains(&DissemControl::Relido));
        assert!(attrs.dissem_us.contains(&DissemControl::Nf));
    }

    #[test]
    fn row9_fires_on_bare_us_secret_adds_relido() {
        let mut attrs = us_secret();
        apply_default_fill(&mut attrs);
        assert!(attrs.dissem_us.contains(&DissemControl::Relido));
    }

    #[test]
    fn row9_suppressed_on_us_secret_plus_rel_to() {
        // (S, REL TO USA, GBR) — Row 9 suppressed by REL_TO_PRESENT
        // in MASK_RELIDO_US_CLASS_SUPPRESSORS. §H.8 p154 reading
        // preserved.
        let mut attrs = us_secret();
        attrs.rel_to = vec![CountryCode::USA, CountryCode::GBR].into();
        apply_default_fill(&mut attrs);
        assert!(!attrs.dissem_us.contains(&DissemControl::Relido));
    }

    #[test]
    fn row9_does_not_fire_on_us_unclassified() {
        // US_COLLATERAL_CLASSIFIED is not set on Unclassified per
        // derive_bits; §H.8 p154 carve-out preserved.
        let mut attrs = empty_attrs();
        attrs.classification = Some(MarkingClassification::Us(Classification::Unclassified));
        apply_default_fill(&mut attrs);
        assert!(!attrs.dissem_us.contains(&DissemControl::Relido));
    }

    #[test]
    fn idempotent_si_g_chain() {
        // (TS, SI-G) → Row 3 of close() would add ORCON, then Row 0
        // default-fill adds NOFORN. Pre-load the ORCON to simulate
        // the post-close state.
        use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};
        use smol_str::SmolStr;
        let mut attrs = us_secret();
        attrs.classification = Some(MarkingClassification::Us(Classification::TopSecret));
        attrs.dissem_us = vec![DissemControl::Oc].into();
        let comp = SciCompartment::new(SmolStr::new("G"), Box::new([]));
        attrs.sci_markings = Box::new([SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([comp]),
            None,
        )]);
        apply_default_fill(&mut attrs);
        let once = attrs.clone();
        apply_default_fill(&mut attrs);
        assert_eq!(attrs, once, "default-fill must be idempotent");
    }
}
