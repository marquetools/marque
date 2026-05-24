use super::dispatch::{
    EmittedIdOverrides, FastPathSeverities, PassFinalizationIndices, panic_payload_to_string,
};
use super::*;

/// Per-page accumulator state bundled at a [`Phase::PageFinalization`]
/// boundary, threaded through [`dispatch_page_finalization`] as a
/// single struct rather than six individual parameters.
///
/// Constructed by field literal at each of the two
/// [`dispatch_page_finalization`] call sites in [`Engine::lint_inner`]
/// — the [`marque_ism::MarkingType::PageBreak`] branch and the
/// end-of-document flush — and consumed by the dispatch for the
/// duration of a single statement. The struct holds borrows into
/// `lint_inner`'s accumulator locals; it MUST be built on the line
/// immediately above the dispatch call and let die at the semicolon.
/// Constructing it earlier or holding it across the post-dispatch
/// reset block (where `page_join_acc` is reassigned to a fresh
/// `CanonicalAttrs::default()`) would fight the borrow checker for
/// no purpose. See `lint_inner`'s `MarkingType::PageBreak` arm for
/// the canonical construction site.
///
/// Crate-internal (`pub(crate)`). The struct is constructed via field
/// literal at two call sites in this same file; `#[non_exhaustive]`
/// is deliberately omitted so those literals stay terse and a future
/// reader doesn't have to wonder whether the omission was an
/// oversight.
///
/// # Field choices
///
/// - `portions` is `&'a [CanonicalAttrs]` (not `Arc<...>`), matching
///   the borrowed slice the dispatch already consumed before the
///   refactor; the dispatch lazily promotes it to `Arc<Box<[_]>>`
///   via `portions_arc.get_or_insert_with(...)` so consecutive
///   same-page banner/CAB candidates share the allocation.
/// - `portions_arc` and `marking_arc` are `&'a mut Option<Arc<...>>`
///   because the dispatch force-initializes both `Some(_)`
///   (PageFinalization rules expect populated Arcs) and the caller
///   threads the same Arcs to any subsequent same-page banner/CAB
///   candidate.
/// - `join_acc` is `&'a CanonicalAttrs` (not `Arc<_>` and not
///   `Cow<_>`) so the hot path's `std::mem::take(&mut page_join_acc)`
///   in `lint_inner` (the issue #306 / PR #674 O(N) accumulation
///   fix) stays a move, not a clone.
/// - `banner_span` is `Option<Span>` by value — `Span` is `Copy` and
///   the dispatch needs an owned `None`/`Some(_)` snapshot that
///   survives the caller's post-dispatch reset of its own
///   accumulator. The field's semantics — Copy snapshot of the
///   closing page's most-recent banner candidate, with the
///   PageFinalization-only visibility contract preserved by the
///   caller — are governed by issue #663 / PR #681; see
///   `RuleContext::page_banner_span` and the `Invariants` section
///   on [`dispatch_page_finalization`] for the clearing guarantee.
/// - `boundary_offset` is the `usize` byte offset of the synthetic
///   page-boundary candidate's zero-length span anchor (the
///   `candidate.span.start` of the trailing page-break candidate on
///   the PageBreak branch, or `source.len()` on the EOD flush).
pub(super) struct PageFinalizationContext<'a> {
    pub(crate) portions: &'a [marque_ism::CanonicalAttrs],
    pub(crate) portions_arc: &'a mut Option<Arc<Box<[marque_ism::CanonicalAttrs]>>>,
    pub(crate) marking_arc: &'a mut Option<Arc<marque_ism::ProjectedMarking>>,
    pub(crate) join_acc: &'a marque_ism::CanonicalAttrs,
    pub(crate) banner_span: Option<Span>,
    pub(crate) boundary_offset: usize,
}

/// Dispatch every registered [`Phase::PageFinalization`] rule against
/// the page-level fixpoint snapshot at a page-boundary anchor offset.
///
/// Issue #461. Called by the engine's lint loop at every
/// scanner-emitted [`marque_ism::MarkingType::PageBreak`] (BEFORE the
/// per-page accumulator reset, so the dispatched rules see the
/// closing page's final state) and once at end-of-document (so
/// trailing portions that never reached a page-break boundary still
/// observe the fixpoint).
///
/// This is a free function rather than an `&self` method on `Engine`
/// because the inputs are decomposed and the helper has no need for
/// other engine state. Threading the decomposition explicitly keeps
/// the contract visible at the call site — every input the dispatch
/// depends on is named in the parameter list, and a future refactor
/// can lift it into a different orchestration shape (an iterator
/// transformation, a streaming dispatch) without spelunking through
/// `Engine`'s field list.
///
/// The per-page accumulator inputs (`portions`, `portions_arc`,
/// `marking_arc`, `join_acc`, `banner_span`, `boundary_offset`) are
/// bundled into [`PageFinalizationContext`] so the signature stays
/// readable; see that struct's doc-comment for per-field semantics
/// and lifetime constraints. The remaining parameters carry the
/// engine-wide state the dispatch reads but does not mutate: the
/// scheme handle, the registered rule sets, the
/// PageFinalization-bucket indices, the resolved per-rule severities,
/// the per-emitted-id override map, the corrections map (threaded to
/// `RuleContext::corrections`), the optional cooperative deadline,
/// and the output diagnostic sink.
///
/// # Invariants
///
/// - `pf_ctx.portions` must be non-empty at call time (the caller
///   guards on `!page_portions.is_empty()`). An empty-page dispatch
///   produces no useful work and `CapcoScheme::project_from_attrs_slice`
///   would emit a noisy default. The skip is in the caller so the
///   cost of the `is_empty()` probe is paid at the boundary, not
///   per rule.
/// - `pf_ctx.portions_arc` / `pf_ctx.marking_arc` are mutable
///   `Option` references because the dispatch path force-initializes
///   both Arcs (PageFinalization rules expect `Some(_)` for both).
///   The caller threads the same Arcs through to a possible
///   subsequent banner/CAB candidate on the same page — except for
///   the end-of-document call, where the document ends without
///   further candidates.
/// - The synthetic boundary candidate carries a zero-length `Span`
///   at `pf_ctx.boundary_offset`. Today this is the only span a
///   PageFinalization rule can emit on its `Diagnostic`: the
///   per-page accumulator stores `[CanonicalAttrs]` without
///   per-portion spans, so `RuleContext::page_portions` cannot
///   recover an offending portion's own offsets. Rules document this
///   limitation in their doc comments (W004 from issue #461 and
///   S005 from issue #488 are the worked examples). A future
///   enhancement that threads per-portion spans through the
///   accumulator — or a span-lookup helper into `RuleContext` —
///   would let rules refine the anchor to the specific offending
///   portion.
/// - `pf_ctx.banner_span` is the closing page's most-recent banner
///   span (issue #663), or `None` if the page had no banner. The
///   caller (`lint_inner`) clears its own accumulator AFTER this
///   dispatch returns; the field is `Copy`/by-value so the boundary
///   snapshot is independent of the caller's reset.
/// - `candidates_processed` is NOT incremented by this dispatch.
///   That counter tracks scanner-emitted candidates; the synthetic
///   PageFinalization candidate is engine-internal.
///
/// # Returns
///
/// `Ok(())` on a complete dispatch pass. `Err(())` on per-dispatch
/// deadline expiry — the caller propagates the truncated `LintResult`
/// shape. The deadline is checked once at the top of the dispatch
/// (the per-page work is small relative to the per-candidate rule
/// loop) so an already-expired deadline returns immediately without
/// invoking any rule.
// Clippy threshold (`too_many_arguments` default = 7) — the dispatch
// retains 9 parameters after issue #680's parameter bundling. The six
// per-page accumulator parameters that originally tripped the lint
// are now collapsed into a single `PageFinalizationContext`; what
// remains is engine-wide read-mostly state (scheme + rule sets +
// resolved severities + emitted-id overrides + corrections map +
// deadline) plus the diagnostic sink. Folding that residual set into
// a second "invariant engine state" bundle is plausibly defensible
// but would muddy the call sites without payoff at the current
// surface area; if a future dispatch grows further parameters or a
// second consumer of the same invariant state appears, that bundling
// is the right next step. The deferral is deliberate per #680's
// scope contract.
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_page_finalization(
    scheme: &CapcoScheme,
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
    pass_finalization_rule_indices: &PassFinalizationIndices,
    fast_path_severities: &FastPathSeverities,
    emitted_id_overrides: &EmittedIdOverrides,
    pf_ctx: PageFinalizationContext<'_>,
    corrections_arc: &Option<Arc<HashMap<String, String>>>,
    deadline: Option<Instant>,
    out_diagnostics: &mut Vec<Diagnostic<CapcoScheme>>,
) -> Result<(), ()> {
    use marque_ism::MarkingType;
    use marque_rules::RuleContext;

    // Destructure the bundle into locals so the dispatch body keeps
    // the same shape it had before the parameter-bundling refactor
    // (issue #680). The destructure splits the borrows on
    // `portions_arc` and `portions` so the `get_or_insert_with(...)`
    // mutable access below cannot collide with the immutable
    // `portions` read inside its closure — a pattern the borrow
    // checker would reject on direct field access through `pf_ctx`.
    let PageFinalizationContext {
        portions: page_portions,
        portions_arc: page_portions_arc,
        marking_arc: page_marking_arc,
        join_acc: page_join_acc,
        banner_span: page_banner_span,
        boundary_offset,
    } = pf_ctx;

    // Deadline guard once at the dispatch boundary. Per-rule
    // deadline checks would amortize the wall-clock probe across a
    // very short rule list (currently 1 rule); the boundary-level
    // check is cheap and keeps the failure mode aligned with the
    // main candidate loop's pre-iteration check.
    if deadline_expired(deadline) {
        return Err(());
    }

    // Empty-bucket short-circuit (Copilot review on PR #487 / issue
    // #461). If no rule declared `Phase::PageFinalization` the Arc
    // force-init below would still clone the accumulator and project
    // `page_marking` — both non-trivial — without a consumer. This
    // matters for future schemes that may register no PageFinalization
    // rules, and for any future config layer that disables every
    // PageFinalization rule via severity override `Off` (the per-rule
    // Off-skip below would no-op, but the clone has already happened).
    // Returning early keeps the cost proportional to actual consumer
    // count.
    if pass_finalization_rule_indices.is_empty() {
        return Ok(());
    }

    // All-Off short-circuit (Copilot round-2 on PR #487). If every
    // PageFinalization rule's registered-id severity resolves to
    // `Off`, the per-rule loop below would skip them all — but only
    // after the Arc force-init paid the snapshot clone +
    // `CapcoScheme::project_from_attrs_slice`. Pre-scanning the
    // bucket lets us return BEFORE those costs.
    //
    // Walker rules (those with `additional_emitted_ids()` non-empty)
    // can still fire under per-emitted-id severity overrides even
    // when their registered-id severity is `Off`, so they MUST NOT
    // be treated as Off by this gate. No PageFinalization rule
    // today registers walker IDs; the gate is shaped to stay
    // correct if one is added.
    let any_rule_can_fire = pass_finalization_rule_indices.iter().any(|&(s, r)| {
        let rule = &rule_sets[s].rules()[r];
        !rule.additional_emitted_ids().is_empty() || fast_path_severities[s][r] != Severity::Off
    });
    if !any_rule_can_fire {
        return Ok(());
    }

    // PageFinalization rules contract: ctx.page_portions AND
    // ctx.page_marking are both populated. Force-init both Arcs
    // here BEFORE building the RuleContext so the rule body can
    // unconditionally read them. Subsequent same-page banner/CAB
    // candidates reuse these Arcs through the normal lazy path.
    let page_portions_arc = page_portions_arc
        .get_or_insert_with(|| Arc::new(page_portions.to_vec().into_boxed_slice()))
        .clone();
    let page_mark_arc = page_marking_arc
        .get_or_insert_with(|| Arc::new(project_page_marking(scheme, page_join_acc)))
        .clone();

    // Zero-length span at the boundary anchor. Rules use this as
    // the candidate-span anchor; if the rule wants a user-facing
    // span on a specific portion, it walks `ctx.page_portions` and
    // refers to that portion's span (when tracked). The per-page
    // accumulator does not store per-portion spans today; rules
    // that need sub-page precision fall back to this anchor and
    // document the limitation.
    let boundary_span = Span::new(boundary_offset, boundary_offset);

    // PageFinalization rules don't read `attrs`; they read
    // `ctx.page_portions` / `ctx.page_marking`. The dummy attrs are
    // a `Default::default()` to satisfy the `Rule::check`
    // signature. We pass `&dummy` so the borrow doesn't outlive the
    // dispatch loop; rules that try to introspect dummy attrs will
    // observe `Default` values (e.g., empty `Box<[T]>` collections)
    // — they would be misimplemented PageFinalization rules anyway.
    let dummy_attrs = marque_ism::CanonicalAttrs::default();

    // `pre_pass_1_attrs` is `None` because the synthetic boundary
    // span has no preceding-portion identity in the pre-pass-1
    // attrs cache (the cache is keyed by content-candidate spans;
    // a boundary span at offset N never equals one).
    // PR #490: clone `page_portions_arc` for the `RuleContext` so the
    // original handle stays in scope through the dispatch loop and
    // remains available to the portion-snapshot sentinel below
    // (which observes the slice the rule actually reads via
    // `ctx.page_portions`). `Arc::clone` is a refcount bump, no
    // slice data is copied.
    let ctx = RuleContext::new(MarkingType::PageFinalization, boundary_span)
        .with_page_portions(Some(page_portions_arc.clone()))
        .with_page_marking(Some(page_mark_arc))
        // Issue #663: thread the closing page's most-recent banner span
        // into the PageFinalization dispatch. `None` when the page had
        // no banner (a portion-only page fragment); `Some(_)` when a
        // banner candidate cleared the decoder gate before the page
        // boundary. PageFinalization rules MAY rely on `None` meaning
        // "no banner to fix" — they MUST NOT unwrap unconditionally.
        .with_page_banner_span(page_banner_span)
        .with_corrections(corrections_arc.clone())
        .with_pre_pass_1_attrs(None);

    // PR #490: portion-snapshot sentinel for the PageRewrite
    // read-only-attrs invariant. `Phase::PageFinalization` rules
    // read `ctx.page_portions` and re-project per-portion lattices
    // from that slice (e.g., W004's `JointSet::from_attrs_iter` per
    // §H.3 p57 derivative-use migration trigger). A rule that
    // mutated portions through any future API change — or a future
    // closure-operator rewrite-application site that did so — would
    // silently break that predicate's input invariance. Today
    // `ctx.page_portions` exposes `&[CanonicalAttrs]` via
    // `Box<[_]>` with no `&mut` API, so a conformant rule cannot
    // violate the contract through the public API; the sentinel is
    // a static guard against future API changes that would open a
    // mutation path.
    //
    // **Sibling sentinel.** The closure-operator's
    // rewrite-application site lives in `CapcoScheme::project`
    // (`crates/capco/src/scheme/marking_scheme_impl.rs`), where it
    // sits between the `join_via_lattice` composition and the
    // declarative PageRewrite catalog. That site carries its own
    // `#[cfg(debug_assertions)]` snapshot-and-compare against the raw
    // per-portion CanonicalAttrs slice it observes, asserting the
    // closure's read-only-attrs invariant. The two
    // sentinels cover different invocation contexts: this one fires
    // around `Phase::PageFinalization` rule dispatch (where rules
    // read `ctx.page_portions`); the scheme-side sentinel fires
    // inside the per-projection pipeline that produces
    // `ctx.page_marking`. Together they pin the read-only contract
    // across both engine-facing surfaces.
    //
    // Snapshot the slice the rule actually observes (via
    // `page_portions_arc`). The sentinel's purpose is to catch
    // FUTURE API loosenings — e.g., a `portions_mut()` addition
    // on a future newtype wrapper, or an `Arc::get_mut` bypass via
    // a future debug API. Cost: a clone of `[CanonicalAttrs]` in
    // debug builds only; `--release` strips the snapshot and the
    // assertion entirely. Placement is AFTER the empty-bucket /
    // all-Off short-circuits (they early-return before reaching
    // this point).
    //
    // Note on the type chain: `page_portions_arc` at this point is
    // the locally-rebound `Arc<Box<[CanonicalAttrs]>>` (NOT the outer
    // `Option<Arc<Box<[CanonicalAttrs]>>>` parameter — see the
    // `.get_or_insert_with(...).clone()` rebinding earlier in this
    // function). `Arc::as_ref()` yields `&Box<[CanonicalAttrs]>`
    // which auto-derefs to `&[CanonicalAttrs]`; `<[T]>::to_vec()`
    // then produces `Vec<CanonicalAttrs>` directly.
    #[cfg(debug_assertions)]
    let portions_before: Vec<marque_ism::CanonicalAttrs> = page_portions_arc.as_ref().to_vec();

    // Mirror the main candidate-loop dispatch shape: fast-path
    // Off-skip via `fast_path_severities[set_idx][rule_idx]`,
    // `catch_unwind` for untrusted rules, per-diagnostic
    // emitted-id override via `emitted_id_overrides`. The walker
    // path (`additional_emitted_ids().is_empty()` is false) gates
    // on the per-diagnostic override loop below; no
    // PageFinalization rule today registers walker IDs, but the
    // shape is preserved for forward compatibility.
    for &(set_idx, rule_idx) in pass_finalization_rule_indices.iter() {
        let rule = &rule_sets[set_idx].rules()[rule_idx];

        if rule.additional_emitted_ids().is_empty() {
            let configured_severity = fast_path_severities[set_idx][rule_idx];
            if configured_severity == Severity::Off {
                continue;
            }
        }

        let rule_id = rule.id();
        let mut diags = if rule.trusted() {
            rule.check(&dummy_attrs, &ctx)
        } else {
            match std::panic::catch_unwind(AssertUnwindSafe(|| rule.check(&dummy_attrs, &ctx))) {
                Ok(d) => d,
                Err(payload) => {
                    let msg = panic_payload_to_string(&payload);
                    tracing::warn!(
                        target: "marque_engine::rule_panic",
                        rule = %rule_id,
                        error = %msg,
                        "PageFinalization rule check panicked; skipping this rule for the current page boundary"
                    );
                    Vec::new()
                }
            }
        };

        // Per-emitted-id override (Site B equivalent for the
        // synthetic dispatch). Mirrors the main loop's
        // `diags.retain_mut` exactly so a `[rules] W004 = "off"`
        // config silences the rule the same way it would in the
        // main candidate loop.
        diags.retain_mut(
            |d| match emitted_id_overrides.get(d.rule.predicate_id()).copied() {
                Some(Severity::Off) => false,
                Some(override_severity) => {
                    d.severity = override_severity;
                    true
                }
                None => true,
            },
        );
        out_diagnostics.extend(diags);
    }

    // PR #490: portion-snapshot assertion — see snapshot comment
    // above. The PageRewrite read-only-attrs invariant requires
    // that no `Phase::PageFinalization` rule mutate the per-portion
    // `CanonicalAttrs` slice (observed via `page_ctx_arc`, matching
    // the slice the rule itself reads).
    //
    // The comparison + error-message construction lives in
    // [`check_portions_unchanged`] below so it can be unit-tested
    // directly (Codecov patch-coverage gate on PR #498). On
    // mismatch the helper returns `Err(msg)` and the call site
    // panics with that message — the panic body is the
    // structurally-uncoverable hot-path branch.
    //
    // Why this avoids `debug_assert_eq!`: `assert_eq!` /
    // `debug_assert_eq!` call `core::panicking::assert_failed`,
    // which formats both operands via `Debug`
    // (`left: {left:?} right: {right:?}`) regardless of any custom
    // message. That would dump both `&[CanonicalAttrs]` slices —
    // token IDs, span offsets, country lists, AEA blocks — into the
    // panic output, violating audit content-ignorance (Constitution V
    // Principle V). The helper-returns-`String` shape
    // formats only counts + indices; debug builds may still run in
    // classified-content environments.
    //
    // The outer-loop placement cannot attribute the violation to a
    // specific rule; if a sentinel firing requires per-rule
    // attribution, switch to a per-iteration snapshot inside the
    // loop temporarily for debugging.
    #[cfg(debug_assertions)]
    if let Err(msg) = check_portions_unchanged(
        portions_before.as_slice(),
        page_portions_arc.as_ref(),
        pass_finalization_rule_indices.len(),
    ) {
        panic!("{msg}");
    }

    Ok(())
}

/// Project the current per-page accumulator slice into a
/// [`marque_ism::ProjectedMarking`] via the scheme's production
/// page-projection path.
///
/// The hot path runs `scheme.project(Scope::Page, ...)` (the lattice +
/// closure + PageRewrite pipeline). The bridge from `CanonicalAttrs` to
/// `ProjectedMarking` lives in `marque_ism::ProjectedMarking::from_canonical`
/// so the scheme crate and the engine crate share one source of truth.
///
/// This helper centralizes the projection-call shape shared by the
/// primary lazy-init in `Engine::lint` (around the banner/CAB candidate
/// dispatch) and the secondary `dispatch_page_finalization`
/// initialization. Both sites need the scheme handle to drive the
/// lattice path; passing `scheme` and the accumulator slice here keeps
/// the closure capture minimal at each call site and avoids duplicating
/// the per-portion conversion logic.
///
/// The parameter is a `&[CanonicalAttrs]` so the caller does not need to
/// construct an intermediate accumulator type.
pub(super) fn project_page_marking(
    scheme: &CapcoScheme,
    page_join_acc: &marque_ism::CanonicalAttrs,
) -> marque_ism::ProjectedMarking {
    // Route through `CapcoScheme::project_from_attrs_slice`, the engine
    // fast-path that consumes the per-page accumulator slice directly.
    //
    // Issue #306 (O(N²) fix): the caller now passes the pre-computed
    // incremental join accumulator (`page_join_acc`) rather than the
    // full `page_portions` slice. The accumulator is maintained by the
    // portion-push site via `join_via_lattice([acc, new_portion])` so
    // this call projects a single element (O(1)) instead of re-folding
    // all N portions on every banner/CAB candidate (O(N)).
    let projected = scheme.project_from_attrs_slice(std::slice::from_ref(page_join_acc));
    marque_ism::ProjectedMarking::from_canonical(projected)
}

/// Compare two `CanonicalAttrs` slices for the PageFinalization
/// read-only-attrs sentinel. Returns `Ok(())` on equality,
/// `Err(msg)` with a content-ignorant diagnostic message on mismatch
/// (counts + indices only — never portion content).
///
/// Debug-only — only callers inside a `#[cfg(debug_assertions)]`
/// block invoke this. `pub(crate)` for unit-testability: the
/// helper is the detection primitive for the read-only-attrs
/// invariant. Extracted from the inline `debug_assert!` body in
/// [`dispatch_page_finalization`] so the comparison +
/// error-message-construction paths land in Codecov patch coverage
/// (PR #498 / issue #490).
///
/// # Audit content-ignorance (Constitution V Principle V) compliance
///
/// The returned error message contains **only**:
///
/// - The literal string `"PageFinalization rule dispatch ..."`
/// - The before-slice length (a `usize` count, not content)
/// - The after-slice length (a `usize` count, not content)
/// - The `rule_count` parameter (a `usize` count, not content)
/// - The doc-cross-reference literal
///
/// It MUST NOT contain any `CanonicalAttrs` field values, type
/// names that imply field content (e.g., `"SciControl"`,
/// `"Span"`), or any string formed from slice element content.
/// `sentinel_tests::check_portions_unchanged_error_message_is_g13_compliant`
/// pins this invariant with a synthetic distinctive-content
/// fixture — modifying the format string MUST be done together
/// with re-running that test.
#[cfg(debug_assertions)]
pub(crate) fn check_portions_unchanged(
    before: &[marque_ism::CanonicalAttrs],
    after: &[marque_ism::CanonicalAttrs],
    rule_count: usize,
) -> Result<(), String> {
    if before == after {
        Ok(())
    } else {
        Err(format!(
            "PageFinalization rule dispatch mutated the per-page portion slice \
             ({} portion(s) before vs {} after, {} rule(s) dispatched). \
             This violates the PageRewrite read-only-attrs invariant in \
             docs/plans/2026-05-01-lattice-design.md section 3 (e.1). \
             The portion-snapshot sentinel cannot pin the violating rule \
             from this outer-loop placement; to attribute, switch to \
             a per-iteration snapshot inside the loop temporarily.",
            before.len(),
            after.len(),
            rule_count,
        ))
    }
}
