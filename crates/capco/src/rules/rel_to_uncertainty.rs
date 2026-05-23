// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`RelToOpaqueUncertainReductionSuggestRule`] — S005
//! (`capco:page.dissem.rel-to-membership-uncertain-reduction`).
//!
//! Suggest-channel rule that fires when an `is_decomposable == None`
//! tetragraph drops out of the page-level REL TO atom-semantics
//! intersection.
//!
//! Issue #206; PR #488 collapsed the original S005/S006 Suggest/Info
//! split into one Suggest-severity rule under `Phase::PageFinalization`.
//! Authority: CAPCO-2016 §H.8 + §D.2 Table 3 rule 21.

use marque_ism::CanonicalAttrs;
use marque_rules::{
    Diagnostic, Message, MessageArgs, MessageTemplate, Phase, Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

// Rule: S005 — REL TO membership-uncertain reduction (issue #206; PR #488)
// ---------------------------------------------------------------------------
//
// What S005 detects. An `is_decomposable == None` tetragraph (NA-
// deprecated, taxonomy-absent, or org-fork extension code) drops out
// of the page-level REL TO atom-semantics intersection because at
// least one portion does not carry the code, AND there exist atoms in
// every-portion-without-the-code that the operator might have
// intended to release to via the dropped code's hypothetical
// membership. The rule cannot decide whether the producer drew on
// external membership data we don't have or simply mis-marked, so it
// surfaces the uncertainty for the operator to resolve.
//
// Phase. Phase::PageFinalization. The trigger is page-scoped — it
// computes the REL TO atom-semantics intersection across every
// portion on the page and emits one diagnostic per uncertain
// tetragraph that drops out of that intersection. The rule reads
// `ctx.page_portions` only; under PageFinalization dispatch the
// engine passes `CanonicalAttrs::default()` as `attrs`, so the rule
// neither reads nor depends on banner-witness state (pre-PR-#488
// the rule read `attrs.rel_to` to decide a Suggest-vs-Info branch;
// see "History — retired S006" below for why that branch was
// removed). The rule therefore must run once per page on the
// closed page-level fixpoint snapshot, not once per banner/CAB
// candidate. The pre-PR-#488 Phase::WholeMarking dispatch produced
// a documented false-negative on banner-first layouts (no closing
// banner ⇒ no Banner candidate ⇒ no firing surface) and a 6th-pass
// false-positive on intermediate snapshots when the rule briefly
// ran on Portion candidates. Phase::PageFinalization closes both —
// the engine dispatches S005 exactly once per page at every
// scanner-emitted `MarkingType::PageBreak` BEFORE the per-page
// accumulator reset, plus once at end-of-document.
//
// Severity / fix. Severity::Suggest with no fix. The ambiguity is not
// resolvable from in-tree data — only the producer's external
// membership data can settle it. The engine never auto-applies a
// Suggest-severity diagnostic regardless of confidence
// (`Engine::fix_inner` excludes `Severity::Suggest` from the apply
// gate), so the no-fix shape is the safest and most honest signal.
//
// History — retired S006. Pre-PR-#488 the rule was a Suggest/Info
// pair: S005 emitted when the banner was inconsistent or missing
// (active validation), S006 emitted at Info severity when the banner
// was consistent with atom-semantics (`expected ⊆ banner_atomic`).
// The two-rule split was an engine-workaround, NOT §-grounded —
// CAPCO-2016 §H.8 treats REL TO via pure set-membership language and
// §D.2 Table 3 rule 21 (the roll-up intersection law) applies
// uniformly without distinguishing "active validation" from
// "consistent case." The split existed because
// `marque_engine::Engine::lint` overwrites every emitted diagnostic's
// severity with the rule's configured/default severity, so a single
// rule could not stably emit at two severities. PR #488 collapsed the
// pair to a single Suggest-severity rule. The eventual admonition
// channel (deferred per
// `specs/006-engine-rule-refactor/followups/admonition-channel.md`)
// will restore per-emission severity if a future need arises; the
// collapse-now matches that eventual end state.
//
// Authority. CAPCO-2016 §H.8 (REL TO list grammar — syntax and
// tetragraph definition) + ODNI ISMCAT
// V[`marque_ism::ISMCAT_TETRA_VERSION`] Tetragraph Taxonomy (member-
// country expansion). The ISMCAT taxonomy is the authoritative
// member-country source per ODNI; §H.8 itself does not delegate to
// ISMCAT (the string "ISMCAT" does not appear in
// `crates/capco/docs/CAPCO-2016.md`). The two authorities compose;
// they are not in a delegating relationship. `CITATION` below
// uses an additive `+` form; read it as "§H.8 (grammar) plus ISMCAT
// (expansion data)", not as "§H.8 delegating to ISMCAT."
//
// Citations explicitly NOT load-bearing for S005:
//   - §D.2 Table 3 rule 23 (TEYE/ACGU/FVEY-only intersection special
//     case) — strictly outside S005's general-tetragraph case.
//   - §H.8 p151 ("Commingling Rule(s) Within a Portion" — per-portion,
//     not page-level roll-up).
// Reviewers verifying citation chains for S005 should not follow
// either of these as authority for the rule's behavior.
//
// Audit-content-ignorance per Constitution V Principle V G13. The
// diagnostic message embeds canonical token strings (CAPCO REL TO
// codes that survived parsing — closed vocabulary, never document
// text) plus verbatim ODNI taxonomy `<Description>` text from
// `lookup_tetragraph_provenance`. No input bytes from the document
// being linted are interpolated. §-citation re-verified 2026-05-17
// against `crates/capco/docs/CAPCO-2016.md`.
pub(crate) struct RelToOpaqueUncertainReductionSuggestRule;

/// Format the `{state}` text for an S005 diagnostic. Pulls from the
/// build-time-generated [`marque_ism::TetragraphProvenance`] table so
/// the description text stays stable across taxonomy revisions and
/// the `is_decomposable` runtime API stays single-purpose.
///
/// The match arms cover the four `is_decomposable == None` shapes
/// the V2022-NOV taxonomy actually produces, plus the
/// taxonomy-absent case. A hypothetical future revision that maps
/// some code to `Some(_)` won't reach this function (the rule's
/// outer guard filters on `is_decomposable == None`); the defensive
/// fallback exists so a future taxonomy revision that introduces a
/// new `(decomposable, membership_shape)` pair still produces a
/// readable diagnostic instead of panicking.
fn s005_state_text(code: &str) -> String {
    use marque_ism::{ISMCAT_TETRA_VERSION, lookup_tetragraph_provenance};
    match lookup_tetragraph_provenance(code) {
        None => "absent (org-fork extension or unknown code)".to_owned(),
        Some(p) => match (p.decomposable, p.membership_shape) {
            ("NA", "Suppressed") => format!(
                "deprecated, membership suppressed \
                 (NA-Suppressed in V{ISMCAT_TETRA_VERSION})"
            ),
            ("NA", "Description") => {
                let desc = p.description.unwrap_or("(no description text)").trim();
                format!(
                    "deprecated, refer to original classification authority \
                     per ODNI: \"{desc}\""
                )
            }
            ("NA", shape) if shape.starts_with("Members") => {
                // Members(recursive) — BHTF in V2022-NOV.
                "deprecated, recursive membership (out of scope for v1)".to_owned()
            }
            (decomp, shape) => format!(
                "ISMCAT V{ISMCAT_TETRA_VERSION} taxonomy: \
                 decomposable={decomp:?}, membership_shape={shape:?}"
            ),
        },
    }
}

/// Expand a slice of `CountryCode` entries into a flat set of
/// atomic country-code strings. Decomposable tetragraphs (FVEY,
/// ACGU, NATO, …) expand to their constituent trigraphs;
/// opaque atoms (EU, KFOR, MNFI, …) pass through unchanged.
///
/// Lifetime: the returned set borrows from the input slice for
/// passthrough atoms and from `'static` storage for tetragraph
/// expansions. Both narrow into `&'a str` cleanly.
fn s005_expand_atomic(rel_to: &[marque_ism::CountryCode]) -> std::collections::BTreeSet<&str> {
    use crate::vocab::expand_tetragraph;
    let mut set = std::collections::BTreeSet::new();
    for code in rel_to.iter() {
        let s = code.as_str();
        if let Some(members) = expand_tetragraph(s) {
            for &m in members {
                set.insert(m);
            }
        } else {
            set.insert(s);
        }
    }
    set
}

/// Render an atomic country-code set as a `, `-joined string with
/// `USA` first (per CAPCO §H.8) and the rest alphabetical.
fn s005_render_set(set: &std::collections::BTreeSet<&str>) -> String {
    let mut codes: Vec<&str> = set.iter().copied().collect();
    if let Some(pos) = codes.iter().position(|s| *s == "USA") {
        if pos != 0 {
            let usa = codes.remove(pos);
            codes.insert(0, usa);
        }
    }
    codes.join(", ")
}

/// Run the S005 trigger analysis on the page-level fixpoint snapshot
/// and emit one Suggest-severity diagnostic per uncertain code that
/// dropped out of the intersection and had a non-empty "other codes"
/// candidate set.
///
/// Called by `RelToOpaqueUncertainReductionSuggestRule::check` under
/// `Phase::PageFinalization`. The `_attrs` parameter is unused — the
/// engine passes `CanonicalAttrs::default()` for PageFinalization
/// dispatch — and the entire decision is made from `ctx.page_portions`
/// (the closed page state) per the rule's doc comment.
///
/// The cost is bounded by the number of portions with non-empty REL
/// TO and the number of uncertain codes across them — a handful of
/// operations over `BTreeSet`s in practice.
///
/// **PR 4b-D.3 note (2026-05-18):** This helper intentionally reads
/// `ctx.page_portions` rather than `ctx.page_marking`. S005's
/// per-portion REL TO + uncertain-trigraph membership analysis
/// requires the portion-level `CanonicalAttrs` slice that
/// `ProjectedMarking` does not expose by design (a projected
/// marking is an aggregate, not a portion view). The
/// architecturally-clean successor is lifting per-portion REL TO
/// membership analysis into the lattice / scheme layer as derived
/// state on `ProjectedMarking`, deferred post-PR-6c.
fn analyze_uncertain_reduction(
    _attrs: &CanonicalAttrs,
    ctx: &RuleContext,
) -> Vec<Diagnostic<CapcoScheme>> {
    use marque_ism::is_decomposable;

    // Defensive — `dispatch_page_finalization` force-initializes
    // `ctx.page_portions` to `Some(_)` before invoking PageFinalization
    // rules (see `crates/engine/src/engine.rs::dispatch_page_finalization`
    // doc). This belt-and-suspenders early-return keeps the rule
    // safe under future engine refactors that might relax the
    // invariant; it should never fire in production. Same shape as
    // W004's defensive early-return in `JointDisunityCollapseRule`.
    //
    // PR 6c migration (T069): read `ctx.page_portions` (the
    // `Box<[CanonicalAttrs]>` slice snapshot) instead of the retired
    // `ctx.page_context` / `PageContext::portions()` accessor pair.
    let Some(page_portions) = ctx.page_portions.as_ref() else {
        return Vec::new();
    };
    let portions: &[CanonicalAttrs] = page_portions.as_ref();

    // Plan §3.2 requires "at least two portions carrying a
    // non-empty REL TO list." Anything less and there's no
    // intersection to compute.
    let portions_with_rel_to: Vec<&CanonicalAttrs> =
        portions.iter().filter(|p| !p.rel_to.is_empty()).collect();
    if portions_with_rel_to.len() < 2 {
        return Vec::new();
    }

    // NOFORN supersedes REL TO at the page level (CAPCO-2016
    // §H.8 + §H.9 — NOFORN/REL TO mutual exclusion). Four trigger
    // families cause `PageContext::expected_rel_to` to return empty
    // *because the marking is superseded*, not because the atom
    // intersection is empty:
    //
    //   1. Any portion carries DissemControl::Nf (NOFORN directly).
    //   2. SBU-NF / LES-NF classified-context split injects NF
    //      (§H.9 p178 / p185).
    //   3. Any portion carries NODIS (§H.9 p174 — "REL TO is not
    //      authorized in the banner line if any portion contains
    //      NODIS information. In this case, NOFORN would convey in
    //      the banner line.").
    //   4. Any portion carries EXDIS (§H.9 p172 — "REL TO is not
    //      authorized in the banner line if any portion contains
    //      EXDIS information. In this case, NOFORN would convey in
    //      the banner line.").
    //
    // Firing S005 under any of these conditions produces a
    // misleading "intersection produced REL TO (empty)" diagnostic
    // — the operator's actual problem is supersession, which is a
    // different rule's territory. Bail so S005 only runs when REL
    // TO is semantically in play. Mirrors the supersession checks
    // `PageContext::expected_rel_to` runs internally; we duplicate
    // them here because the rule needs to distinguish "empty due to
    // supersession" from "empty due to genuinely-disjoint portion
    // REL TO lists" (the latter is a legitimate S005 trigger).
    // Trigger 1 (NOFORN-direct) needs its own check because
    // `expected_non_ic_dissem`'s `needs_nf` only covers triggers
    // 2–4; triggers 2–4 are all reflected in the `needs_nf` flag.
    // Caught originally by Copilot review on PR #249; expanded to
    // cover triggers 3–4 in PR 3c.B-8F-engine-gap. Page-extension
    // stable post-PR-#488 — the bails fire on the same closed page
    // state PageFinalization observes.
    let any_portion_noforn = portions.iter().any(|p| {
        p.dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
    });
    if any_portion_noforn {
        return Vec::new();
    }
    // PR 4b-E: migrated from `page.expected_non_ic_dissem()` (the
    // retired PageContext method) to the lattice-native
    // `NonIcDissemSet::from_attrs_iter` constructor. Same
    // SBU-NF/LES-NF/NODIS/EXDIS NF-injection semantics
    // (§H.9 p172/p174/p178/p185); the second tuple element
    // `needs_nf` is the same flag.
    let needs_nf = crate::lattice::NonIcDissemSet::from_attrs_iter(portions).needs_nf();
    if needs_nf {
        return Vec::new();
    }

    // The atom-semantics intersection. The lattice-native
    // `RelToBlock::from_attrs_iter` does tetragraph expansion before
    // intersection and `into_boxed_slice` returns the result USA-first
    // then alphabetical per §H.8 p150-151. We project to a string set
    // for set-algebra.
    //
    // PR 4b-E: migrated from `page.expected_rel_to()` (the retired
    // PageContext method). The NOFORN-dominates / NODIS/EXDIS
    // supersession is encoded as the `NofornSuperseded` arm of
    // `RelToBlock`; the per-axis bails above already short-circuit
    // S005 in those cases (so the lattice-side supersession arms
    // produce the same empty result without redundant work).
    let expected = crate::lattice::RelToBlock::from_attrs_iter(portions).into_boxed_slice();
    let expected_set: std::collections::BTreeSet<&str> =
        expected.iter().map(|c| c.as_str()).collect();

    // Collect uncertain codes (deduped, sorted) across all portions.
    //
    // Trigraph filter: ISMCAT is — as the name says — a *tetragraph*
    // taxonomy. ISO 3166-1 alpha-3 trigraphs (USA, GBR, AUS, …)
    // aren't listed, so `is_decomposable(trigraph)` returns `None`
    // for the same reason `is_decomposable("XYZW")` does. Trigraphs
    // are atomic by ISO convention, not uncertain — skip them. The
    // shipped CVEnumISMCATRelTo recognition surface holds 280
    // length-3 trigraphs, 1 length-2 code (EU;
    // `is_decomposable=Some(false)` so already filtered by the
    // `is_none()` check), 58 length-4 tetragraphs, and 1 length-15
    // special code (AUSTRALIA_GROUP; `is_decomposable=Some(true)`).
    // The `len != 3` plus `is_none()` gates together select exactly
    // the codes the rule cares about: NA-deprecated tetragraphs and
    // taxonomy-absent (org-fork extension) tetragraphs.
    let mut uncertain_codes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for portion in &portions_with_rel_to {
        for code in portion.rel_to.iter() {
            let s = code.as_str();
            if s.len() == 3 {
                continue;
            }
            if is_decomposable(s).is_none() {
                uncertain_codes.insert(s.to_owned());
            }
        }
    }
    if uncertain_codes.is_empty() {
        return Vec::new();
    }

    // Diagnostic span anchor: the engine passes a zero-length
    // `Span(boundary_offset, boundary_offset)` at the page-break
    // boundary (or `source.len()` for the EOD dispatch). PageContext
    // does not store per-portion spans, so per-uncertain-code span
    // precision would require extending the hot-path PageContext
    // data type. The boundary anchor is the best available pointer
    // today; users locating "which page surfaced uncertainty?" map
    // the byte offset to a page number via their own document-position
    // metadata. Same convention as W004 (`JointDisunityCollapseRule`).
    let span = ctx.candidate_span;

    let mut diagnostics = Vec::new();
    for x in &uncertain_codes {
        // Per X: portions that don't contain X. Since X is opaque
        // (atom-semantics treats it as an atom), X survives
        // intersection iff X is in every portion's rel_to.
        // Conversely, X drops iff there is at least one portion
        // without X. That set of portions-without-X is what bounds
        // the "other codes" candidate set below.
        let portions_without_x: Vec<&CanonicalAttrs> = portions_with_rel_to
            .iter()
            .copied()
            .filter(|p| !p.rel_to.iter().any(|c| c.as_str() == x.as_str()))
            .collect();
        if portions_without_x.is_empty() {
            // X in every portion ⇒ X survives atom-semantics; nothing
            // to surface for this X.
            continue;
        }

        // "Other codes" = atoms that appear in EVERY portion-without-X
        // but didn't survive intersection AND aren't X itself.
        //
        // Why "every portion-without-X" (intersection across them)
        // rather than "any portion": for an atom Y to survive
        // atom-semantics intersection IF X's hypothetical membership
        // included Y, Y must be in every portion's expansion. The
        // X-containing portions get Y "for free" via the hypothesis
        // (Y ∈ M(X)); the portions without X must have Y in their
        // own rel_to atoms. So the candidate set is exactly atoms
        // present in every portion-without-X.
        //
        // Why "not in expected": those already survived; nothing for
        // X's hypothetical membership to add.
        //
        // Why "≠ X": X is the uncertain code we're hypothesizing
        // about, not a candidate to be added by its own membership.
        //
        // Note: an atom Y that appears alongside X in the same
        // portion is irrelevant here — Y is already explicitly
        // listed in that portion, so X's hypothetical membership
        // doesn't change Y's intersection survival in any direction.
        // (Caught by Copilot review on PR #249: a previous version
        // used `union(all portions) − expected − {X}`, which
        // included same-portion atoms and produced false-positive
        // diagnostics when those atoms were missing from another
        // portion.)
        let mut atoms_in_every_without_x = s005_expand_atomic(&portions_without_x[0].rel_to);
        for p in &portions_without_x[1..] {
            let exp = s005_expand_atomic(&p.rel_to);
            atoms_in_every_without_x = atoms_in_every_without_x
                .intersection(&exp)
                .copied()
                .collect();
        }
        let other_codes: std::collections::BTreeSet<&str> = atoms_in_every_without_x
            .iter()
            .copied()
            .filter(|s| !expected_set.contains(s) && *s != x.as_str())
            .collect();
        if other_codes.is_empty() {
            continue;
        }

        let state = s005_state_text(x);
        let expected_str = if expected_set.is_empty() {
            "(empty — atom intersection produced no shared codes)".to_owned()
        } else {
            s005_render_set(&expected_set)
        };
        let other_str = s005_render_set(&other_codes);

        // G13: drop the runtime variable interpolation. Template
        // identifies the rel-to ambiguity class; the affected category
        // is CAT_REL_TO.
        let _ = (x, state, expected_str, other_str);
        let message = Message::new(
            MessageTemplate::NonCanonicalOrder,
            MessageArgs {
                category: Some(crate::scheme::CAT_REL_TO),
                ..MessageArgs::default()
            },
        );

        // No fix — the ambiguity is not resolvable from in-tree
        // data. `Diagnostic::with_fix(..., None)` signals the
        // conscious deferred-migration decision per the same
        // pattern E016/E036 used pre-PR-3c.B (matching PR #349).
        diagnostics.push(Diagnostic::with_fix(
            RuleId::new("capco", "page.dissem.rel-to-uncertain-reduction"),
            Severity::Suggest,
            span,
            message,
            CITATION,
            None,
        ));
    }
    diagnostics
}

/// Citation for S005. Stays static (not formatted with
/// `ISMCAT_TETRA_VERSION`) because `Diagnostic::citation` is
/// `&'static str`. The version reference is in the state text inside
/// the message body, which is dynamically formatted via
/// `s005_state_text`. Pre-PR-#488 this constant was shared with S006;
/// post-#488 S005 is the sole consumer.
/// S005 (REL TO opaque-uncertain reduction suggestion) citation. The
/// typed `Citation` anchors at §H.8 p150 (REL TO grammar); the
/// secondary authority is the ODNI ISMCAT Tetragraph Taxonomy
/// (`ISMCAT_TETRA_VERSION`), which is not a CAPCO §-citation and
/// thus does not encode into the typed `Citation` field. The
/// per-rule doc comment carries the full provenance.
const CITATION: Citation = capco(SectionLetter::H, 8, 150);

/// Citations S005 may emit on diagnostics. Wraps [`CITATION`]
/// for the [`Rule::cited_authorities`] surface.
const AUTHORITIES: &[Citation] = &[CITATION];

impl Rule<CapcoScheme> for RelToOpaqueUncertainReductionSuggestRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.rel-to-uncertain-reduction")
    }
    fn name(&self) -> &'static str {
        "rel-to-opaque-uncertain-reduction"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// Phase::PageFinalization (PR #488): observes the page-level
    /// fixpoint snapshot of the REL TO axis and emits one diagnostic
    /// per uncertain code that dropped out of the page intersection.
    /// The engine dispatches this rule once per page at every
    /// scanner-emitted `MarkingType::PageBreak` BEFORE the
    /// `PageContext` reset, plus once at end-of-document. The
    /// pre-#488 `Phase::WholeMarking` + Banner-only gating produced
    /// a documented false-negative on banner-first layouts (closed
    /// by the EOD path) and a 6th-pass false-positive on
    /// intermediate Portion-time snapshots (does not recur under
    /// PageFinalization because the rule fires exactly once per
    /// page on the closed state).
    fn phase(&self) -> Phase {
        Phase::PageFinalization
    }
    /// Trusted: implementation is a pure read-only set-algebra walk
    /// over `PageContext::expected_rel_to` + per-portion REL TO
    /// projections plus a `format!` message synthesis using only
    /// canonical CountryCode strings (closed CAPCO vocabulary) and a
    /// fixed §-citation. No mutable global state, no I/O, no
    /// allocation that could fail unexpectedly; the rule is safe to
    /// skip `catch_unwind` per PR #448.
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        analyze_uncertain_reduction(attrs, ctx)
    }
}
