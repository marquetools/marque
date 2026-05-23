// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use marque_ism::{CanonicalAttrs, DocumentPosition, MarkingType, Zone};
use marque_scheme::Span;
use std::collections::HashMap;
use std::sync::Arc;

/// Document position context passed to rules alongside parsed markings.
///
/// Phase 3 made `zone` and `position` `Option`-typed: the scanner cannot
/// reliably determine header/footer/body or document position from raw
/// text alone, so a rule that reads either field must handle `None`.
/// They will become populated in a future scanner pass that consumes
/// document structural metadata (page count, line numbers, header/footer
/// detection on extracted documents).
///
/// `page_portions` and `page_marking` are two views over the same
/// per-page state, populated by the engine for every non-portion
/// candidate (Banner, CAB) and reset at scanner-emitted
/// `MarkingType::PageBreak` candidates (form-feed `\f` and `\n\n\n+`
/// heuristics) so each reflects only the current page:
///
/// - **`page_portions`** — `Arc<Box<[CanonicalAttrs]>>` raw per-portion
///   slice. Rules that need per-portion membership (e.g. W004's
///   `JointSet::from_attrs_iter` for the `DisunityCollapse` state, S005's
///   per-portion REL TO intersection analysis) read this directly. NOT
///   the surface a banner-rollup walker should compare against — see
///   `page_marking` below.
/// - **`page_marking`** — `Arc<ProjectedMarking>` composite roll-up of
///   the page's lattice projection. `BannerMatchesProjectedRule` (the
///   walker dispatching E031 / E035 / E040) and E039
///   (`NodisExdisClearsBannerRelToRule`) compare the observed banner /
///   CAB against this composite. Constructed by
///   `CapcoScheme::project_from_attrs_slice(&page_portions)` lazily at
///   first banner/CAB use; PR 9b T133 / FR-006.
///
/// New banner / CAB validation rules SHOULD read `page_marking` (the
/// rolled-up shape the banner is supposed to convey). Reach for
/// `page_portions` only when the rule's logic is genuinely
/// per-portion-structural (i.e. the projection has flattened away
/// information the rule needs).
///
/// **`#[non_exhaustive]`** (PR 4b-B 9th-pass follow-up): the engine
/// has added several public fields during the 006 refactor
/// (`page_marking`, `corrections`, `pre_pass_1_attrs`) and is likely
/// to add more before the API stability freeze at PR 10. Marking the
/// struct `#[non_exhaustive]` means a future field addition is a
/// non-breaking change for downstream consumers.
///
/// **Note on future cross-portion aggregation rules** (N-9-2, PR 437
/// 10th-pass): the `cross_portion_context` field was removed because
/// eager per-portion accumulator cloning is O(N²) over portions per
/// page and had zero active rule consumers. Future cross-portion
/// rules that need the post-add accumulator state should add a
/// lazy/gated field with explicit capability declaration rather than
/// restoring the eager-clone shape. Per Constitution Principle I,
/// any O(N²) hot-path cost MUST be benchmarked before shipping.
///
/// **Cross-crate consumers MUST construct via the engine-provided
/// constructor path** (`RuleContext::new`). `#[non_exhaustive]` blocks
/// BOTH bare literal construction (`RuleContext { marking_type, zone,
/// ... }`) AND functional-update syntax (`RuleContext { marking_type,
/// ..base }`) across crate boundaries — the Rust reference specifies
/// that functional update with `..base` requires the struct to be
/// fully-exhaustively constructible at the call site, so
/// `#[non_exhaustive]` blocks it just as it blocks literal
/// construction. See the constructor doc below for the correct
/// cross-crate pattern.
///
/// Same-crate construction (this crate's own unit tests within
/// `marque-rules` itself) is unaffected — `#[non_exhaustive]` only
/// restricts construction in EXTERNAL crates. ALL other crates —
/// including `marque-engine` (which is a separate crate from
/// `marque-rules`), `marque-capco`, and `crates/capco/tests/*` — are
/// external and must use the constructor helpers. The FR-040
/// cargo-rules check enforces the pattern.
///
/// P-5 (8th-pass): corrected prior doc that claimed `..base`
/// functional-update "works" for downstream rule crates — it does not.
/// The constructor doc at `RuleContext::new` is authoritative.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RuleContext<'a> {
    pub marking_type: MarkingType,
    /// Document zone (header/footer/body/CAB) when known. `None` in Phase 3
    /// — the scanner cannot prove header vs footer from raw text.
    pub zone: Option<Zone>,
    /// Coarse document position when known. `None` in Phase 3.
    pub position: Option<DocumentPosition>,
    /// Byte span of the scanner-emitted candidate this rule check is
    /// running against. This is the marking-scope span (the full
    /// portion or banner candidate), distinct from any sub-span a
    /// diagnostic might point at (e.g., a single token within the
    /// portion).
    ///
    /// Rules that attach a structural fix via `Diagnostic::fix` copy
    /// this into `Diagnostic::candidate_span` so the engine's
    /// intent-synthesis path knows which scope-bytes to re-render via
    /// `MarkingScheme::apply_intent` +
    /// `MarkingScheme::render_canonical`.
    ///
    /// Added in the PR 3c.B engine-prereq commit. Populated by the
    /// engine from `candidate.span` before invoking each rule.
    pub candidate_span: Span,
    /// Per-page accumulated portion attributes — the slice form that
    /// banner / CAB / PageFinalization rules consume when they need
    /// per-portion membership (W004's `JointSet::from_attrs_iter` and
    /// S005's `analyze_uncertain_reduction` both walk this slice).
    ///
    /// `Some(Arc::new(boxed_slice))` is the same per-page snapshot
    /// every rule on the same page shares; the engine builds it once
    /// lazily at the first banner / CAB / PageFinalization dispatch
    /// and reuses the `Arc` across consecutive dispatches on the page.
    /// `None` for portion candidates and for banner / CAB candidates
    /// on an empty page.
    ///
    /// PR 6c (T069) introduced this field as the structural successor
    /// to the historical `page_context: Option<Arc<PageContext>>`
    /// field. `Box<[CanonicalAttrs]>` (immutable snapshot) is what
    /// `Arc` wraps because the slice form mirrors Constitution
    /// Principle II "pivot fields use `Box<[T]>`" and the snapshot
    /// is genuinely immutable once frozen at the banner/CAB
    /// boundary.
    pub page_portions: Option<std::sync::Arc<Box<[CanonicalAttrs]>>>,
    /// Page-level rolled-up marking — the `Scope::Page` projection of
    /// every portion accumulated since the last
    /// [`marque_ism::MarkingType::PageBreak`]. PR 9b (T133 / FR-006)
    /// added this alongside the per-page portion snapshot
    /// ([`Self::page_portions`]) so banner-validation rules can
    /// consume the rolled-up shape directly.
    ///
    /// Populated by the engine for every non-portion candidate
    /// (Banner, CAB) once at least one portion has accumulated on the
    /// page. `None` otherwise. The shape mirrors
    /// [`Self::page_portions`]: same engine pass populates both; same
    /// `PageBreak` reset semantics; same `Arc` clone discipline so a
    /// per-page snapshot is shared cheaply across all banner-rule
    /// invocations on that page.
    ///
    /// **Phase::PageFinalization invariant (issue #461).** For
    /// `Phase::PageFinalization` dispatches the engine force-initializes
    /// this to `Some` before invoking the rule; see
    /// [`crate::Phase::PageFinalization`]. PageFinalization rules MAY rely on
    /// `Some(_)` for both this field and [`Self::page_portions`].
    ///
    /// Banner-validation rules read fields directly:
    ///
    /// ```ignore
    /// if let Some(page) = ctx.page_marking.as_ref() {
    ///     // page.dissem_us / page.dissem_nato / page.sci_markings / ...
    /// }
    /// ```
    pub page_marking: Option<std::sync::Arc<marque_ism::ProjectedMarking>>,
    /// Byte span of the most recent banner candidate observed on the
    /// current page (issue #663). `Some(span)` once a
    /// [`marque_ism::MarkingType::Banner`] candidate has cleared the
    /// engine's decoder confidence gate (`prov.recognition_score()
    /// >= self.config.confidence_threshold()`) and been processed; a
    /// sub-threshold decoder banner recognition does NOT populate this
    /// field — the same discipline that gates downstream rule dispatch
    /// and `PageContext` accumulation per issue #471. `None` until
    /// then and after the per-page reset at every
    /// [`marque_ism::MarkingType::PageBreak`].
    ///
    /// **Visibility contract** (mirrors [`Self::page_portions`] post-PR
    /// #674): for `Phase::PageFinalization` dispatches the engine
    /// populates this field with the closing page's banner span (when
    /// the page had one); the main candidate dispatch loop passes
    /// `None` unconditionally. The justification today is YAGNI, not
    /// architectural: every existing per-portion / per-banner rule sees
    /// the in-flight banner via [`Self::candidate_span`] when the
    /// candidate IS the banner — no `Phase::WholeMarking` or
    /// `Phase::Localized` consumer of "the banner on this page from a
    /// position elsewhere on the page" exists. If a future
    /// `Phase::WholeMarking` rule needs the retroactive banner span
    /// from a portion-candidate dispatch point, populating this field
    /// in the main loop is a one-line additive change (the per-page
    /// accumulator is already maintained); revisit the visibility
    /// contract at that point. This field exists today specifically so
    /// a `Phase::PageFinalization` rule (the only phase that fires
    /// AFTER every candidate on the page is processed) can target a
    /// fix at the banner-scope bytes from a position where the
    /// candidate-scope is just the synthetic boundary anchor at the
    /// page break or EOD.
    ///
    /// **Sub-span discipline**: this is the FULL banner candidate span
    /// (the bytes the scanner identified as the banner line). Rules
    /// that need to target a sub-block (e.g., the REL TO token group
    /// alone) emit a [`crate::FixIntent`] keyed off the full banner
    /// span and let the engine's intent-application path
    /// ([`MarkingScheme::apply_intent`] +
    /// [`MarkingScheme::render_canonical`]) re-render the whole
    /// banner from the page-level lattice projection
    /// ([`Self::page_marking`]). This is necessary because
    /// [`crate::Rule::check`] does NOT receive the source byte buffer — the
    /// rule cannot read `&[u8]` slices itself from the span. The
    /// single-span shape matches the current `Diagnostic::fix` +
    /// `Diagnostic::candidate_span` contract and avoids storing
    /// per-category sub-spans the engine doesn't already track on the
    /// page accumulator.
    ///
    /// **Multi-banner pages.** A page MAY contain more than one banner
    /// in pathological inputs (e.g., a header banner + a footer banner
    /// without an intervening `\f`). This field carries the MOST
    /// RECENT banner span observed on the page. Constitution VI's
    /// "page resets at scanner-emitted page-break candidates" invariant
    /// is what keeps the field bounded to a single page; rules that
    /// need to disambiguate header vs footer banners can read
    /// [`Self::zone`] once it becomes populated (Phase 3 has it as
    /// `None`).
    ///
    /// **Motivating consumer**: S010 (`collapse-uniform-rel-portions`)
    /// and E072 (`bare-rel-portion-divergence`) resolution paths per
    /// CAPCO-2016 §H.8 pp150-156 — both need to atomically rewrite
    /// per-portion REL TO blocks AND the banner's dissem block. Issue
    /// #663 closes the engine gap; the rule wire-up is a follow-up PR
    /// against the corpus regression harness once this plumbing is in
    /// place.
    ///
    /// [`MarkingScheme::apply_intent`]: marque_scheme::MarkingScheme::apply_intent
    /// [`MarkingScheme::render_canonical`]: marque_scheme::MarkingScheme::render_canonical
    pub page_banner_span: Option<Span>,
    /// Organization-specific corrections map from config `[corrections]`.
    /// `None` when no corrections are configured.
    pub corrections: Option<Arc<HashMap<String, String>>>,
    /// Pre-pass-1 attributes for this marking when a pass-1 fix
    /// reshaped its bytes (FR-023 / R-4). `Some` iff the marking's
    /// span overlaps a pass-1 fix span; `None` otherwise.
    ///
    /// Rules MUST handle `None` — never unconditionally unwrap. The
    /// field is populated by the engine's `TwoPassFixer` from a stack-
    /// scoped `SmallVec<[(Span, CanonicalAttrs); 4]>` cache built before
    /// the pass-1 splice. The borrow lifetime `'a` is tied to that
    /// cache and dies when pass-2 dispatch completes.
    ///
    /// The field is the architectural two-pass-reshape signal: rules
    /// that need to differentiate "this defect existed before pass-1"
    /// from "pass-1 exposed this defect by reshaping bytes" can branch
    /// on `pre_pass_1_attrs.is_some()`. No current rule consumes the
    /// signal — it is plumbed through every rule's `check` signature
    /// so future consumers can read it without re-threading the
    /// lifetime parameter. The engine-applied `PrecedingFixPenalty`
    /// mechanism originally planned to consume the field's PRESENCE
    /// was retired in PR 7c per D-7.22 (misunderstanding-derived,
    /// path was independently confirmed dead code under current
    /// `Phase::Localized` rules). Future evolution (deferred per
    /// D-7.7) replaces this borrow with `Arc<CanonicalAttrs>` when
    /// the parse cache adopts refcount-shared attrs alongside the
    /// v0.2 LMDB incremental cache.
    pub pre_pass_1_attrs: Option<&'a CanonicalAttrs>,
}

impl<'a> RuleContext<'a> {
    /// Construct a minimal `RuleContext` with all `Option`-typed
    /// context fields set to `None`. Required-field arguments
    /// (`marking_type`, `candidate_span`) come from the engine's
    /// per-candidate dispatch loop or the test fixture's synthetic
    /// inputs.
    ///
    /// External (cross-crate) construction of `RuleContext` MUST go
    /// through this constructor because `#[non_exhaustive]` blocks
    /// both bare literal construction AND `..base` functional-update
    /// syntax across crate boundaries. Callers that need to populate
    /// optional fields chain `with_*` setters or assign on the
    /// returned mutable binding:
    ///
    /// ```ignore
    /// let ctx = RuleContext::new(MarkingType::Banner, span)
    ///     .with_page_portions(Some(portions))
    ///     .with_corrections(corrections);
    /// ```
    ///
    /// or
    ///
    /// ```ignore
    /// let mut ctx = RuleContext::new(MarkingType::Banner, span);
    /// ctx.page_portions = Some(portions);
    /// ```
    ///
    /// PR 4b-B 9th-pass follow-up: added alongside the
    /// `#[non_exhaustive]` attribute on `RuleContext` so external
    /// consumers (downstream rule crates, integration tests in
    /// `marque-capco`, the `marque-engine` rule loop) have a stable
    /// construction entrypoint regardless of which optional fields
    /// the engine adds in future PRs.
    pub fn new(marking_type: MarkingType, candidate_span: marque_scheme::Span) -> Self {
        Self {
            marking_type,
            zone: None,
            position: None,
            candidate_span,
            page_portions: None,
            page_marking: None,
            page_banner_span: None,
            corrections: None,
            pre_pass_1_attrs: None,
        }
    }

    /// Set [`Self::zone`] (header / footer / body / CAB).
    pub fn with_zone(mut self, zone: Option<Zone>) -> Self {
        self.zone = zone;
        self
    }

    /// Set [`Self::position`] (coarse document position).
    pub fn with_position(mut self, position: Option<DocumentPosition>) -> Self {
        self.position = position;
        self
    }

    /// Set [`Self::page_portions`] (per-page snapshot of accumulated
    /// portion attributes; PR 6c successor to `with_page_context`).
    pub fn with_page_portions(mut self, page_portions: Option<Arc<Box<[CanonicalAttrs]>>>) -> Self {
        self.page_portions = page_portions;
        self
    }

    /// Set [`Self::page_marking`] (page-level rolled-up marking).
    pub fn with_page_marking(
        mut self,
        page_marking: Option<Arc<marque_ism::ProjectedMarking>>,
    ) -> Self {
        self.page_marking = page_marking;
        self
    }

    /// Set [`Self::page_banner_span`] (issue #663 — most recent banner
    /// candidate span on the current page; populated only for
    /// `Phase::PageFinalization` dispatches per the visibility contract
    /// documented on the field).
    pub fn with_page_banner_span(mut self, page_banner_span: Option<Span>) -> Self {
        self.page_banner_span = page_banner_span;
        self
    }

    /// Set [`Self::corrections`] (org-specific corrections map).
    pub fn with_corrections(mut self, corrections: Option<Arc<HashMap<String, String>>>) -> Self {
        self.corrections = corrections;
        self
    }

    /// Set [`Self::pre_pass_1_attrs`] (pass-1 reshape signal).
    pub fn with_pre_pass_1_attrs(mut self, pre_pass_1_attrs: Option<&'a CanonicalAttrs>) -> Self {
        self.pre_pass_1_attrs = pre_pass_1_attrs;
        self
    }
}
