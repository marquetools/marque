// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Page-level portion accumulator.
//!
//! [`PageContext`] accumulates per-portion [`CanonicalAttrs`] as the engine
//! processes candidates on a page. Banner and CAB validation rules consume
//! the accumulator via [`PageContext::portions`] (the raw pre-rollup view)
//! and the engine drives page-level aggregation through
//! [`marque_scheme::MarkingScheme::project`] on the scheme adapter
//! ([`marque_capco::scheme::CapcoScheme::project_from_page_context`] is
//! the engine's fast path).
//!
//! # PR 4b-E retirement note
//!
//! Pre-PR-4b-E this module also exposed an `expected_*` accessor surface
//! (`expected_classification`, `expected_sci_controls`, ...,
//! `render_expected_banner`, `is_classified`, `project`) that derived
//! the per-axis page rollup from the accumulated portions. PR 4b-E
//! retired the entire surface — every consumer migrated to the
//! lattice-native helpers in `marque-capco::lattice` (`SciSet`,
//! `SarSet`, `AeaSet`, `DissemSet`, `NatoDissemSet`, `RelToBlock`,
//! `DisplayOnlyBlock`, `FgiSet`, `NonIcDissemSet`,
//! `DeclassifyOnLattice`, `DeclassExemptionLattice`,
//! `ClassificationLattice`, `NatoClassLattice`, `JointSet`,
//! `sci_controls_from_markings`) and the scheme-level
//! `render_canonical(Scope::Page, ...)` for banner rendering. See
//! `docs/plans/2026-05-18-pr4b-E-page-context-deletion-plan.md` for
//! the migration map.
//!
//! The accumulator survives because the engine drives page aggregation
//! through `scheme.project(Scope::Page, ...)` which consumes a slice of
//! per-portion markings — `PageContext` is the accumulator that bridges
//! `Engine::lint`'s per-candidate processing into that slice.
//!
//! # Thread-safety
//! `PageContext` is `Send + Sync` (verified at compile time by
//! `crates/ism/tests/send_sync.rs`). It is intended to be built
//! sequentially during a single document pass; sharing across threads
//! requires the caller to externally synchronize `add_portion`.

use crate::canonical::CanonicalAttrs;

// PR 4b-E: `sar_sort_key` relocated to `crates/ism/src/sar_sort.rs`
// per architect plan §3 Decision 4. The `marque_ism::sar_sort_key`
// public re-export is preserved via `lib.rs`.

/// Page-level portion accumulator, built by the engine as it processes
/// portion markings on a page.
///
/// Consumers reach `PageContext` through [`marque_rules::RuleContext::page_context`]
/// (wrapped in [`std::sync::Arc`]). Only two rule callsites read it directly
/// today — W004 (`JointDisunityCollapseRule`) and S005 (`analyze_uncertain_reduction`);
/// both consume the raw slice from [`Self::portions`] because their
/// diagnostics depend on per-portion membership, not on a rolled-up answer.
///
/// New rules SHOULD prefer `MarkingScheme::project(Scope::Page, ...)` (via
/// the engine's `ProjectedMarking` accessor on [`marque_rules::RuleContext::page_marking`])
/// for the rolled-up view; reach for `portions()` only when per-portion
/// membership genuinely matters.
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

    /// Borrow the raw accumulated portion attributes, in document order.
    ///
    /// Most banner-validation rules want the rolled-up view through
    /// `MarkingScheme::project(Scope::Page, ...)`. A handful of rules — W004
    /// (`JointDisunityCollapseRule`) and S005 (`rel-to-opaque-uncertain-reduction`,
    /// issue #206) — need the pre-rollup view because the diagnostic depends
    /// on **which** portion contributed which code, not on the rolled-up
    /// answer.
    ///
    /// New rules SHOULD prefer the projected view via
    /// [`marque_rules::RuleContext::page_marking`] (which the engine
    /// computes through `scheme.project(Scope::Page, ...)`); reach for
    /// `portions()` only when per-portion membership genuinely matters.
    pub fn portions(&self) -> &[CanonicalAttrs] {
        &self.portions
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod shim_tests {
    //! PR 4b-E shim coverage for the surviving `PageContext` surface.
    //!
    //! The pre-PR-4b-E test module exercised every `expected_*` accessor
    //! (≥120 tests across SCI / SAR / AEA / dissem / REL TO / DISPLAY ONLY
    //! / FGI / declass / banner-render axes). All of those moved to
    //! `crates/capco/src/lattice.rs::tests` (lattice-native helper
    //! coverage) and `crates/capco/tests/page_context_lattice_parity.rs`
    //! (the byte-identity parity gate). What remains is dedicated
    //! coverage for the surviving shim surface — `new` / `Default`,
    //! `Clone`, `add_portion`, `portion_count`, `is_empty`, `portions()`
    //! — plus the issue #430 pre-size invariant.
    use super::*;
    use crate::attrs::{Classification, MarkingClassification};

    fn attrs(c: Classification) -> CanonicalAttrs {
        CanonicalAttrs {
            classification: Some(MarkingClassification::Us(c)),
            ..Default::default()
        }
    }

    #[test]
    fn default_produces_empty_with_default_capacity() {
        // Issue #430: `Default::default()` pre-allocates
        // `DEFAULT_PORTIONS_CAPACITY` so the first 8 portions
        // accumulate without reallocation.
        let ctx = PageContext::default();
        assert!(ctx.is_empty());
        assert_eq!(ctx.portion_count(), 0);
        assert!(
            ctx.portions.capacity() >= DEFAULT_PORTIONS_CAPACITY,
            "Default must pre-size to at least DEFAULT_PORTIONS_CAPACITY; got {}",
            ctx.portions.capacity(),
        );
    }

    #[test]
    fn clone_preserves_default_capacity_invariant() {
        // Issue #430: manual `Clone` impl preserves the pre-size on
        // clone — derived `Clone` would call `Vec::clone()` which
        // strips capacity to `len()`. The engine clones into
        // `Arc<PageContext>` for the banner/CAB rule hand-off; we
        // want the cloned ctx to keep its headroom.
        let mut src = PageContext::new();
        src.add_portion(attrs(Classification::Secret));
        let cloned = src.clone();
        assert_eq!(cloned.portion_count(), 1);
        assert!(
            cloned.portions.capacity() >= DEFAULT_PORTIONS_CAPACITY,
            "Clone must preserve the pre-size; got {}",
            cloned.portions.capacity(),
        );
    }

    #[test]
    fn add_portion_grows_portions_in_document_order() {
        let mut ctx = PageContext::new();
        ctx.add_portion(attrs(Classification::Confidential));
        ctx.add_portion(attrs(Classification::Secret));
        ctx.add_portion(attrs(Classification::TopSecret));
        let p = ctx.portions();
        assert_eq!(p.len(), 3);
        assert_eq!(
            p[0].classification.as_ref().map(|c| c.effective_level()),
            Some(Classification::Confidential),
        );
        assert_eq!(
            p[1].classification.as_ref().map(|c| c.effective_level()),
            Some(Classification::Secret),
        );
        assert_eq!(
            p[2].classification.as_ref().map(|c| c.effective_level()),
            Some(Classification::TopSecret),
        );
    }

    #[test]
    fn portion_count_matches_add_portion_calls() {
        let mut ctx = PageContext::new();
        assert_eq!(ctx.portion_count(), 0);
        ctx.add_portion(attrs(Classification::Secret));
        assert_eq!(ctx.portion_count(), 1);
        ctx.add_portion(attrs(Classification::Secret));
        assert_eq!(ctx.portion_count(), 2);
    }

    #[test]
    fn is_empty_after_new_then_false_after_add() {
        let mut ctx = PageContext::new();
        assert!(ctx.is_empty());
        ctx.add_portion(attrs(Classification::Secret));
        assert!(!ctx.is_empty());
    }

    #[test]
    fn portions_borrows_slice_in_document_order() {
        let mut ctx = PageContext::new();
        let a = attrs(Classification::Confidential);
        let b = attrs(Classification::Secret);
        ctx.add_portion(a.clone());
        ctx.add_portion(b.clone());
        let view = ctx.portions();
        assert_eq!(view.len(), 2);
        // Round-trip identity on the structurally-equal CanonicalAttrs
        // (CanonicalAttrs derives PartialEq).
        assert_eq!(view[0], a);
        assert_eq!(view[1], b);
    }
}
