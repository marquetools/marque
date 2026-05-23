// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! NATO-specific rules.
//!
//! - [`BareNatoRequiresRelToRule`] — bare NATO classification in a
//!   US-classified document should carry `REL TO USA, NATO`
//!   (Suggest channel).
//! - [`LegacyNatoCompoundRemarkRule`] — legacy NATO compound text
//!   (CTSA / CTS-A / etc.) emits a `Recanonicalize` fix.
//!
//! Predicate IDs live on each rule's `RuleId::new(...)` — the wire
//! string is the single source of truth.

use marque_ism::{CanonicalAttrs, CountryCode, MarkingClassification, TokenKind};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, RecanonScope, ReplacementIntent, SectionLetter, capco};

use super::eyes::build_rel_to_replacement;
use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: S007 — bare NATO classification in a US-classified document
//              should carry `REL TO USA, NATO`.
//
// PR 9c.2 / FR-048. Authority: CAPCO-2016 §H.7 p127 Notional Example 2
// worked example `(//CTS//BOHEMIA//REL TO USA, NATO)` — "a NATO COSMIC
// TOP SECRET (CTS) BOHEMIA portion within a US classified document and
// is releasable back to NATO". The citation is example-derived (no
// "MUST" prose), so the rule is `Severity::Suggest` to match the
// S004 + S005 precedent (post-PR-#488 collapse of the historical
// S005/S006 pair). Users can opt up via `[rules] S007 = "warn"` in
// `.marque.toml` if their org demands stronger surfacing.
//
// The original FR-048 wording mandated a declarative `Constraint`. The
// followup at
// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`
// tracks why that shape cannot land yet: `MarkingScheme::evaluate_custom`
// (`crates/capco/src/scheme.rs`) receives `&CanonicalAttrs` only — no
// `&PageContext` access — so a per-portion gate that needs to enumerate
// sibling portions ("not a solely-NATO document") cannot be a
// `Constraint::Custom` today. Hand-written rule with page-context access
// is the right shape until the trait surface extends.
// ---------------------------------------------------------------------------

/// Confidence scalar emitted by S007 (`bare-nato-requires-rel-to-usa-nato`)
/// alongside its `text_correction` fix.
///
/// **Calibration.** §H.7 p127 Notional Example 2 is the load-bearing
/// citation. The worked example is illustrative prose ("a NATO COSMIC
/// TOP SECRET (CTS) BOHEMIA portion within a US classified document and
/// is releasable back to NATO"), not "MUST"-mandate prose; S004 + S005
/// set the precedent (post-PR-#488 collapse of the historical S005/S006
/// pair) that example-derived FD&R guidance ships as
/// `Severity::Suggest`. Within the suggest channel, the confidence
/// scalar reflects how strongly the source dictates the rewrite. The
/// chosen value `0.85` is below the (broader) `Confidence::strict(0.95)`
/// used by mandate-prose rewrites in `rules_declarative.rs` (e.g. the
/// REL TO canonical-form rewrite at line 711, the JOINT class-floor
/// rewrite at line 1077) and above the `0.75` used by lower-evidence
/// suggest-channel rewrites in this file.
///
/// **Threshold ladder relationship (load-bearing for auto-apply).**
/// The default `confidence_threshold` in `Engine::fix_inner` is `0.95`.
/// A user who sets `[rules] S007 = "fix"` in `.marque.toml` will see the
/// engine demote the override back to `Suggest` because the diagnostic's
/// confidence is `0.85 < 0.95`. To get S007 auto-applied a user must
/// also drop the threshold to `≤ 0.80` (or any value `< 0.85`) via
/// `confidence_threshold = 0.80`. The dual-override pattern is
/// exercised end-to-end by `engine_with_s007_as_fix()` in
/// `crates/capco/tests/fr048_bare_nato_rel_to.rs`.
///
/// **S007 vs. S004.** S004 emits at the file-level constant
/// [`SUGGEST_CONFIDENCE`] (currently `0.5`), which cannot clear any
/// reasonable threshold even with a `fix` override. S007 is the first
/// text_correction-bearing Suggest-severity rule whose confidence is
/// high enough to clear a relaxed threshold; the next author who adds a
/// suggest-with-apply rule should pick a confidence consciously rather
/// than copy-paste from S004's hard-advisory channel.
const S007_SUGGEST_CONFIDENCE: f32 = 0.85;

/// Rule **S007** — `bare-nato-requires-rel-to-usa-nato`.
///
/// Fires on a portion whose classification axis is a bare
/// [`MarkingClassification::Nato`] variant when the page also carries at
/// least one non-NATO portion AND the portion's existing REL TO list
/// does not already cover `{USA, NATO}`. Emits a `Severity::Suggest`
/// `text_correction` that either (a) replaces the classification token
/// with `<class>//REL TO USA, NATO` (e.g. `NS` →
/// `NS//REL TO USA, NATO` in `(//NS)`), or (b) augments an existing
/// REL TO block to include `USA` and `NATO` in canonical USA-first /
/// alpha-sorted order.
///
/// # Authority
///
/// CAPCO-2016 §H.7 p127 Notional Example 2 — the worked example
/// `(//CTS//BOHEMIA//REL TO USA, NATO)` shows the canonical form for a
/// bare-NATO portion in a US-classified document. By extension a NATO
/// classification axis (NU/NR/NC/NS/CTS) in a US-classified document
/// should carry `REL TO USA, NATO` even without an SCI block.
///
/// # Early-return clauses (in order)
///
/// 1. **Portion-only**: `ctx.marking_type != MarkingType::Portion`.
///    Banner roll-up flows automatically from the per-portion
///    `REL TO USA, NATO` via `BannerMatchesProjectedRule`; firing on a
///    banner here would double-report.
/// 2. **Bare NATO classification only**: `attrs.classification` must be
///    `Some(MarkingClassification::Nato(_))`. US/JOINT/FGI/Conflict
///    portions are not in scope. (ATOMAL companions on the AEA axis
///    coexist with `Nato(_)` and do **not** immunize the portion — see
///    project memory `project_atomal_is_aea`.)
/// 3. **Solely-NATO doc carve-out**: when `ctx.page_marking` is `Some`
///    and `ProjectedMarking::is_solely_nato_classified()` returns `true`,
///    every portion on the page is bare NATO and alliance ownership is
///    implicit — `REL TO USA, NATO` is not needed. **Today this branch
///    is forward-looking**: `Engine::lint` gates
///    `with_page_marking(ctx_page_marking)` on
///    `candidate.kind != MarkingType::Portion && !page_portions.is_empty()`,
///    so portion rules always see `page_marking = None` and the
///    carve-out is unreachable. S007 fires on every bare-NATO portion
///    regardless of solely-NATO document status until a future engine
///    pass plumbs page-level state to portion-rule dispatch
///    (load-bearing for that migration; see fr048 trip-wire test).
///    Users in solely-NATO contexts can silence with
///    `[rules] S007 = "off"` in `.marque.toml`. PM decision #2.
/// 4. **NOFORN guard**: when this portion carries `DissemControl::Nf`,
///    the conflict is owned by the page rewrite
///    `capco/noforn-conflicts-rel-to` (declarative constraint in
///    `CapcoScheme`). S007 must not propose a `REL TO` that the
///    NOFORN/REL-TO conflict rule would immediately remove.
///
/// # Coverage check (intentional byte-compare for NATO)
///
/// The predicate is `has_usa && has_nato` where:
///
/// - `has_usa` is `attrs.rel_to.contains(&CountryCode::USA)` (USA has a
///   `pub const`).
/// - `has_nato` is `attrs.rel_to.iter().any(|c| c.as_bytes() == b"NATO")`.
///
/// We deliberately do **NOT** use `rel_to_covers` here — tetragraph
/// expansion would accept `REL TO USA, DEU, GBR, FRA, ...` as covering
/// NATO, which §H.7 p127's literal example does not endorse. We also
/// deliberately do **NOT** add a `CountryCode::NATO` constant: that
/// would bump the `marque-ism` public surface for a single use site
/// without a second consumer — Constitution VII Principle IV "shallow
/// adapter" discipline. Byte-compare at the one site is the right idiom
/// until a second consumer materializes.
///
/// # Splice strategy (two helpers, two responsibilities)
///
/// The fix path uses two single-responsibility helpers:
///
/// - [`build_bare_nato_rel_to_insertion`] — used when the portion has
///   NO existing REL TO block. Replaces the **Classification token's
///   span** with `<class>//REL TO USA, NATO`, where `<class>` is the
///   canonical portion abbreviation from `NatoClassification::portion_str`
///   (one of NU/NR/NC/NS/CTS). A non-empty span is required because
///   the engine's `text_correction` synthesizer rejects empty spans
///   (`engine.rs::apply_text_corrections_to` line 1529); a zero-length
///   insert-cursor would be filtered out before promotion. Re-emitting
///   the corpus-derived classification abbreviation is G13-safe — it
///   is a closed-vocab canonical token, not raw document content.
/// - [`build_bare_nato_rel_to_augmentation`] — used when the portion
///   has an existing REL TO block. Emits the canonical
///   `REL TO USA, <sorted-existing-plus-NATO>` body via
///   [`build_rel_to_replacement`] so the canonical-form contract stays
///   in one place.
///
/// Two helpers, not one: the splice site (replace-classification-token
/// vs replace-existing-rel-to-block) is different, and the emitted
/// body shape (classification-prefixed `//REL TO ...` vs bare
/// `REL TO ...` list) is different. Folding them into one helper would
/// require a Boolean parameter that re-introduces the bug class
/// single-responsibility was supposed to remove.
///
/// # `Phase::WholeMarking`, not `Phase::Localized`
///
/// The augmentation branch modifies a `RelToBlock` token that is not
/// the same token as the classification block — the fix crosses a
/// token boundary. `Phase::Localized`'s "single-token-span contract"
/// (`marque_rules::lib.rs` line 241) would fail validation for the
/// augmentation branch. `Phase::WholeMarking` is correct for both
/// branches.
///
/// # G13 audit-content-ignorance
///
/// The diagnostic message is a `&'static`-derived string. No
/// `format!` interpolation of input bytes. The replacement string is
/// constructed from `CountryCode` values only (which are
/// corpus-derived canonical token bytes) plus the literal `NATO`
/// and `REL TO USA, ` template — no document text contributes to
/// audit output. Constitution V Principle V (G13).
///
/// # First text_correction-bearing Suggest-severity rule with apply path
///
/// S007 is the first text_correction-bearing `Severity::Suggest` rule
/// in marque-capco whose emitted confidence is high enough to clear a
/// relaxed `confidence_threshold` when paired with a `[rules] S007 =
/// "fix"` override. The threshold ladder:
///
/// - S004 emits at [`SUGGEST_CONFIDENCE`] = `0.5`. Even with `[rules]
///   S004 = "fix"` the diagnostic cannot clear the default
///   `confidence_threshold = 0.95`, so S004 stays hard-advisory under
///   any reasonable threshold.
/// - S007 emits at [`S007_SUGGEST_CONFIDENCE`] = `0.85`. With both
///   `[rules] S007 = "fix"` AND `confidence_threshold ≤ 0.80` set, the
///   engine's `lint` suggest-channel demotion pass keeps the override
///   intact and `Engine::fix_inner` applies the splice.
///
/// The dual-override pattern is exercised end-to-end by the
/// `engine_with_s007_as_fix()` helper and the augmentation tests in
/// `crates/capco/tests/fr048_bare_nato_rel_to.rs`. The next author who
/// adds a suggest-with-apply rule should pick a confidence scalar with
/// the same care: too low and the auto-apply path is unreachable; too
/// high and a Suggest-default rule auto-applies under the default
/// threshold the moment a user adds the severity override.
pub(super) struct BareNatoRequiresRelToRule;

/// Build the insertion body for the no-existing-REL-TO branch.
///
/// The splice strategy replaces the **classification token's span**
/// (a non-empty span; the engine's `text_correction` synthesizer
/// rejects empty spans per `engine.rs::apply_text_corrections_to`
/// line 1529) with `<class>//REL TO USA, NATO`, where `<class>` is the
/// canonical portion abbreviation from
/// [`marque_ism::NatoClassification::portion_str`] (one of NU/NR/NC/
/// NS/CTS). For input `(//NS)` the splice replaces `NS` with
/// `NS//REL TO USA, NATO`, yielding `(//NS//REL TO USA, NATO)`.
///
/// Pure function over `&NatoClassification`: input is a corpus-derived
/// canonical token; no document content flows through. Constitution V
/// Principle V (G13).
///
/// **Branch invariant.** This helper is the no-REL-TO branch only. The
/// caller in `BareNatoRequiresRelToRule::check` gates the call site
/// behind `rel_to_blocks.is_empty()` (the branch that selects this
/// helper over [`build_bare_nato_rel_to_augmentation`]); the
/// `debug_assert!` at the call site re-checks `attrs.rel_to.is_empty()`
/// as a redundant guard. If the existing-REL-TO case ever needs the
/// classification-token splice as well, factor a shared helper rather
/// than re-introducing an `existing` parameter here.
fn build_bare_nato_rel_to_insertion(class: marque_ism::NatoClassification) -> String {
    // 18 bytes for `//REL TO USA, NATO`: 2 for `//` + 16 for the
    // suffix literal. `class.portion_str()` is `NU`/`NR`/`NC`/`NS`/
    // `CTS` (2 or 3 bytes); capacity covers the longest variant.
    let mut out = String::with_capacity(class.portion_str().len() + 18);
    out.push_str(class.portion_str());
    out.push_str("//REL TO USA, NATO");
    out
}

/// Build the canonical replacement body for the existing-REL-TO branch.
/// Takes the existing `rel_to` slice, drops `USA` and `NATO` (so they
/// aren't double-listed), appends both back via
/// [`build_rel_to_replacement`] which enforces USA-first +
/// alpha-sorted canonical form per §A.6 p16 + §H.8 p150-151.
///
/// Pure function over `&[CountryCode]`: byte input is the rel_to slice
/// (canonical token bytes only), byte output is the canonical
/// replacement body. Constitution V G13: no document text flows
/// through this function.
fn build_bare_nato_rel_to_augmentation(existing: &[CountryCode]) -> String {
    // Collect the existing codes minus USA and NATO, then add both back
    // via the canonical builder — the builder prepends USA and sorts
    // the remainder alphabetically. `NATO` (a tetragraph) sorts among
    // the remainder by `Vec::sort` byte ordering.
    let mut codes: Vec<String> = existing
        .iter()
        .map(|c| c.as_str().to_owned())
        .filter(|s| s != "USA" && s != "NATO")
        .collect();
    codes.push("USA".to_owned());
    codes.push("NATO".to_owned());
    build_rel_to_replacement(&codes)
}

/// Citations S007 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const BARE_NATO_REQUIRES_REL_TO_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 7, 127)];

impl Rule<CapcoScheme> for BareNatoRequiresRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.nato.bare-nato-requires-rel-to-usa-nato")
    }
    fn name(&self) -> &'static str {
        "bare-nato-requires-rel-to-usa-nato"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// `Phase::WholeMarking`: the augmentation branch's splice target
    /// is a `RelToBlock` token distinct from the classification token
    /// — the fix crosses a token boundary. `Phase::Localized`'s
    /// single-token-span contract fails the augmentation branch. The
    /// insertion branch is technically Localized-compatible (single
    /// zero-length insert site at the classification-block edge) but
    /// declaring two different phases for the two branches would
    /// double the rule's registration surface; one phase covers both.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        BARE_NATO_REQUIRES_REL_TO_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;

        // Clause 1: portion-only.
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        // Clause 2: bare NATO classification only. Capture the NATO
        // variant for the insertion-branch canonical text (used by
        // `build_bare_nato_rel_to_insertion`).
        let Some(MarkingClassification::Nato(nato_class)) = &attrs.classification else {
            return vec![];
        };
        let nato_class = *nato_class;

        // Clause 3: solely-NATO doc carve-out. When page-marking is
        // populated AND every portion is bare-NATO, alliance ownership
        // is implicit — silence. **Today this branch is forward-looking**:
        // `Engine::lint` does not populate `RuleContext::page_marking`
        // for `MarkingType::Portion` candidates, so `ctx.page_marking`
        // is always `None` here and the carve-out is unreachable. S007
        // fires on every bare-NATO portion regardless of solely-NATO
        // document status until a future engine pass plumbs page state
        // to portion-rule dispatch (see fr048 trip-wire test).
        //
        // PR 4b-D.3 (2026-05-18): migrated from `ctx.page_context` to
        // `ctx.page_marking`. Both predicates return identical answers
        // (the legacy `PageContext::is_solely_nato_classified` walks
        // `self.portions` with the same `matches!` pattern); the
        // migration is architectural alignment ahead of PR 4b-E
        // retiring the `PageContext.expected_*` machinery and
        // consolidating page-aggregate reads on `ctx.page_marking`.
        if let Some(page) = ctx.page_marking.as_ref()
            && page.is_solely_nato_classified()
        {
            return vec![];
        }

        // Clause 4: NOFORN guard. NOFORN on this portion is structurally
        // incompatible with REL TO; the conflict is owned by the page
        // rewrite `capco/noforn-conflicts-rel-to`. S007 must not propose
        // a REL TO that the NOFORN/REL-TO conflict rule would
        // immediately remove.
        if attrs
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf))
        {
            return vec![];
        }

        // Coverage check: byte-compare against the literal NATO
        // tetragraph plus the `CountryCode::USA` const. Do NOT use
        // `rel_to_covers` here — tetragraph expansion is not what
        // §H.7 p127's example endorses (see the doc-block above).
        let has_usa = attrs.rel_to.contains(&CountryCode::USA);
        let has_nato = attrs.rel_to.iter().any(|c| c.as_bytes() == b"NATO");
        if has_usa && has_nato {
            return vec![];
        }

        // Locate the splice point. Two branches:
        //
        //  (a) no REL TO block: replace the Classification token's
        //      span with `<class>//REL TO USA, NATO` where `<class>`
        //      is the canonical portion abbreviation from
        //      `NatoClassification::portion_str()`. For `(//NS)` the
        //      Classification token spans the `NS` bytes; the splice
        //      replaces them with `NS//REL TO USA, NATO`, yielding
        //      `(//NS//REL TO USA, NATO)`. A non-empty span is
        //      required because the engine's `text_correction`
        //      synthesizer rejects empty spans
        //      (`engine.rs::apply_text_corrections_to` line 1529).
        //  (b) existing REL TO block: replace that token's span with
        //      the augmented canonical body. The first RelToBlock
        //      token wins; if multiple RelToBlock tokens exist (a
        //      malformed shape), we skip the rule rather than risk
        //      cross-block corruption — the cleaner authoring
        //      decision is deferred to the operator.
        // The discriminator only needs to distinguish 0 / 1 / many
        // RelToBlock tokens, so we pull two iterator items and match
        // on the shape rather than allocating a `Vec`.
        let mut rel_to_blocks_iter = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToBlock);
        let (span, replacement) = match (rel_to_blocks_iter.next(), rel_to_blocks_iter.next()) {
            (None, _) => {
                // No existing REL TO block — insertion branch. We
                // replace the **classification token's span** (a
                // non-empty span; the engine's `text_correction`
                // synthesizer rejects empty spans) with
                // `<class>//REL TO USA, NATO`, where `<class>` is the
                // canonical portion abbreviation. The result for
                // `(//NS)` is `(//NS//REL TO USA, NATO)`.
                //
                // INVARIANT: `attrs.rel_to` is empty in this branch.
                // The parser populates `attrs.rel_to` from
                // `RelToBlock` token spans
                // (`marque-core::parser::parse_rel_to_block`), so no
                // `RelToBlock` tokens implies an empty
                // `attrs.rel_to`. We re-check defensively so a future
                // parser change that populates `attrs.rel_to` from a
                // non-`RelToBlock` source can't silently corrupt the
                // splice.
                debug_assert!(
                    attrs.rel_to.is_empty(),
                    "S007 insertion branch requires empty attrs.rel_to (no \
                     RelToBlock tokens implies no rel_to entries); the \
                     augmentation branch (build_bare_nato_rel_to_augmentation) \
                     owns the non-empty case",
                );
                let Some(class_tok) = attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::Classification)
                else {
                    // Defensive: a NATO classification axis without a
                    // Classification token span would mean the parser
                    // failed to emit the token. Skip emission rather
                    // than risk a wrong-span splice.
                    return vec![];
                };
                let body = build_bare_nato_rel_to_insertion(nato_class);
                (class_tok.span, body)
            }
            (Some(block), None) => {
                // Single existing REL TO block — augmentation branch.
                let body = build_bare_nato_rel_to_augmentation(&attrs.rel_to);
                (block.span, body)
            }
            (Some(_), Some(_)) => {
                // Multiple REL TO blocks is structurally malformed; an
                // E002 / parser-shape diagnostic owns that case. Skip
                // S007 emission to avoid splice-collision damage.
                return vec![];
            }
        };

        vec![Diagnostic::text_correction(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            capco(SectionLetter::H, 7, 127),
            replacement,
            FixSource::BuiltinRule,
            Confidence::strict(S007_SUGGEST_CONFIDENCE),
            None,
        )]
    }
}

// ===========================================================================
// E066 — Legacy NATO compound text re-marking (PR 9c.1 T134)
// ===========================================================================
//
// Authority chain:
//   §G.2 p40 (Table 5: ARH by Registered Marking) lists ATOMAL,
//     BOHEMIA, and BALK as registered NATO control markings — each
//     has its own row with `Requires {marking} read-in`, confirming
//     they are control markings registered alongside (not fused with)
//     the NATO classification ladder.
//   §H.7 p122 worked example places ATOMAL in the AEA category
//     position: `SECRET//RD/ATOMAL//FGI NATO//NOFORN` ("ATOMAL is a
//     NATO Atomic Energy Act marking that follows the registered US
//     Atomic Energy Act marking RD").
//   §H.7 p127 worked example places BOHEMIA in the SCI category
//     position: `(//CTS//BOHEMIA//REL TO USA, NATO)`.
//   §G.1 Table 4 p38 portion-form column lists `ATOMAL` / `BALK` /
//     `BOHEMIA` as same-form across title / banner-abbrev / portion
//     (standalone canonical names — no `SAR-` prefix, no fused
//     class form).
//
// Per project memory `remark-on-derivative-use-is-marque-autofix`,
// "Marque exists precisely to automate the re-marking the manual
// permits doing by hand" — the §H.7 worked-example canonical forms
// ARE the autofix targets.
//
// E066 fires when the strict parser canonicalizes legacy NATO compound
// text (`CTSA`, `CTS-A`, `NSAT`, `NS-A`, `NCA`, `NC-A`, `CTS-B`,
// `CTS-BALK`, or their banner-form equivalents) into the canonical
// structural shape (bare class + AEA/SCI companion). The rule reads
// `attrs.token_spans` looking for a `TokenKind::Classification` whose
// text matches one of the legacy patterns; if found AND the parsed
// `attrs.classification` is a bare `NatoClassification::*` variant with
// the corresponding `AeaMarking::Atomal` / `SciControlSystem::NatoSap`
// companion present, the rule emits a Recanonicalize fix.
//
// The emitted FixIntent uses `ReplacementIntent::Recanonicalize {
// scope: Portion | Page }` — the engine re-renders the candidate via
// `MarkingScheme::render_canonical`, which emits the canonical
// multi-block form (`(//CTS//ATOMAL)`, `(//CTS//BOHEMIA)`, etc.) per
// the §H.7 p122 + §G.2 p40 + §H.7 p127 worked examples.
//
// Severity: `Fix` (auto-applies when confidence ≥ engine threshold).
// Confidence: `strict(1.0)` — the canonical form is unambiguous; the
// renderer produces deterministic bytes from the canonical attrs.
//
// G13 audit-content-ignorance: the diagnostic message does NOT echo
// input bytes. The message references canonical token names
// (`TOK_ATOMAL`, `TOK_BALK`, `TOK_BOHEMIA`) via `MessageArgs.token`
// and the message template's text. The fix payload is structural
// (`Recanonicalize`); the engine snapshots the canonical replacement
// at promotion time without any rule-side byte stringification.

/// Rule E066 — legacy NATO compound text re-marking per §G.2 p40
/// (Table 5 registration) + §H.7 p122 (ATOMAL → AEA) + §G.2 p40 +
/// §H.7 p127 (BALK/BOHEMIA → SCI).
pub(super) struct LegacyNatoCompoundRemarkRule;

/// Citations E066 may emit on diagnostics. Two branches:
/// §H.7 p122 (ATOMAL → AEA position) and §H.7 p127 (BALK/BOHEMIA →
/// SCI position); the rule's `check()` selects per companion type.
/// See [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const LEGACY_NATO_COMPOUND_REMARK_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::H, 7, 122),
    capco(SectionLetter::H, 7, 127),
];

impl Rule<CapcoScheme> for LegacyNatoCompoundRemarkRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.recanonicalize.legacy-nato-compound")
    }
    fn name(&self) -> &'static str {
        "legacy-nato-compound-remark"
    }
    fn default_severity(&self) -> Severity {
        // Severity::Fix — auto-apply when confidence ≥ threshold. The
        // canonical form is unambiguous (single grammar; no
        // classifier-judgment branch); the renderer produces
        // deterministic bytes from the parsed canonical attrs.
        Severity::Fix
    }
    /// Phase::WholeMarking: the canonical re-rendering spans the full
    /// candidate (the classification block AND the appended AEA/SCI
    /// companion block need to land together), so the fix scope is
    /// whole-marking by construction.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        LEGACY_NATO_COMPOUND_REMARK_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{
            AeaMarking, MarkingClassification, MarkingType, NatoSap, SciControlSystem,
        };

        // Gate on bare NATO classification + presence of an AeaMarking::Atomal
        // OR a SciControlSystem::NatoSap companion. The parser only
        // writes those companions for the canonicalized legacy text
        // paths (`CTSA`, `CTS-B`, etc., per crate::parser::parse_nato_classification).
        let is_nato = matches!(&attrs.classification, Some(MarkingClassification::Nato(_)));
        if !is_nato {
            return vec![];
        }

        let has_atomal = attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Atomal(_)));
        let nato_sap = attrs.sci_markings.iter().find_map(|m| match m.system {
            SciControlSystem::NatoSap(sap) => Some(sap),
            _ => None,
        });
        if !has_atomal && nato_sap.is_none() {
            return vec![];
        }

        // Locate the classification TokenSpan and verify its raw text
        // matches one of the thirteen legacy compound patterns (eight
        // portion forms + five banner forms). The parser writes a
        // Classification token-span for the NATO block; the span's
        // `.text` carries the original bytes. We match against the
        // closed set of legacy forms — well-formed canonical multi-block
        // inputs (`(//CTS//ATOMAL)`, `(//CTS//BOHEMIA)`) will NOT match
        // here because the classification block in those inputs is just
        // `CTS`.
        let Some(classification_tok) = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::Classification)
        else {
            return vec![];
        };

        if !is_legacy_nato_compound_text(classification_tok.text.as_str()) {
            return vec![];
        }

        // Determine the canonical companion token for `MessageArgs.token`.
        // Per G13 audit-content-ignorance: this is a closed-vocab
        // TokenId, not raw bytes.
        let companion_token = if has_atomal {
            crate::scheme::TOK_ATOMAL
        } else {
            match nato_sap {
                Some(NatoSap::Balk) => crate::scheme::TOK_BALK,
                Some(NatoSap::Bohemia) => crate::scheme::TOK_BOHEMIA,
                // `None` is unreachable: the early-return above
                // guarantees exactly one of (has_atomal,
                // nato_sap.is_some()) is true at this point. The
                // `Some(_)` wildcard covers a future `NatoSap`
                // variant introduced after PR 9c.1 ([`NatoSap`] is
                // `#[non_exhaustive]`); defensively skip-emit until
                // the rule explicitly learns the new variant.
                None | Some(_) => return vec![],
            }
        };

        let scope = match ctx.marking_type {
            MarkingType::Portion => RecanonScope::Portion,
            _ => RecanonScope::Page,
        };

        // §G.2 p40 (Table 5: ARH by Registered Marking) registers
        // ATOMAL/BOHEMIA/BALK as control markings. §H.7 p122 worked
        // example shows ATOMAL in the AEA position; §H.7 p127 worked
        // example shows BOHEMIA in the SCI position. Choose the
        // structurally-most-precise anchor based on which companion
        // was written. The cross-axis §G.2 p40 reference lives in
        // the rule doc comment, not in the typed Citation field.
        let citation = if has_atomal {
            capco(SectionLetter::H, 7, 122)
        } else {
            capco(SectionLetter::H, 7, 127)
        };

        // G13: message template identifies the wrong-form class;
        // `MessageArgs.token` carries the canonical companion token.
        // The has_atomal vs has_natosap distinction is preserved
        // through `companion_token`.
        let message = Message::new(
            MessageTemplate::WrongTokenForm,
            MessageArgs {
                token: Some(companion_token),
                ..MessageArgs::default()
            },
        );

        let fix_intent = FixIntent {
            replacement: ReplacementIntent::Recanonicalize { scope },
            confidence: Confidence::strict(1.0),
            feature_ids: Default::default(),
            message: Message::new(
                MessageTemplate::WrongTokenForm,
                MessageArgs {
                    token: Some(companion_token),
                    ..MessageArgs::default()
                },
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };

        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            classification_tok.span,
            ctx.candidate_span,
            message,
            citation,
            fix_intent,
        )]
    }
}

/// Returns `true` when `text` matches one of the thirteen legacy NATO
/// compound patterns (eight portion forms + five banner forms) retired
/// by PR 9c.1 T134.
///
/// The closed set is exactly the patterns the parser's
/// `parse_nato_classification` accepts in the legacy branch — anything
/// else is either canonical (`CTS`, `NS`, `NC`) or unrelated. Keeping
/// the predicate co-located with the rule (and citing the parser's
/// match table) means a future expansion of the legacy set requires a
/// coordinated edit in both places — the natural propagation point.
///
/// Citations: CAPCO-2016 §G.1 Table 4 p38 (portion-form column);
/// §G.2 p40 (Table 5 — registers ATOMAL/BOHEMIA/BALK as standalone
/// control markings, not classification suffixes).
fn is_legacy_nato_compound_text(text: &str) -> bool {
    matches!(
        text,
        // Portion forms.
        "CTSA"
            | "CTS-A"
            | "CTS-B"
            | "CTS-BALK"
            | "NSAT"
            | "NS-A"
            | "NCA"
            | "NC-A"
            // Banner forms (full-word legacy compounds).
            | "COSMIC TOP SECRET ATOMAL"
            | "COSMIC TOP SECRET-BOHEMIA"
            | "COSMIC TOP SECRET-BALK"
            | "NATO SECRET ATOMAL"
            | "NATO CONFIDENTIAL ATOMAL"
    )
}
