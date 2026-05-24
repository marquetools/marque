// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Form-mismatch rules — banner uses portion form, or portion uses
//! banner form, per CAPCO-2016 §G.1 Table 4 p38 (closed authorized
//! form pairs).
//!
//! - [`PortionFormInBannerRule`] — banner-context detection of a
//!   portion-form token (e.g. `SECRET//NF` instead of
//!   `SECRET//NOFORN`).
//! - [`BannerFormInPortionRule`] — portion-context detection of a
//!   banner-form token (e.g. `(S//NOFORN)` instead of `(S//NF)`).
//!
//! Predicate IDs live on each rule's `RuleId::new(...)` — the wire
//! string is the single source of truth.

use marque_ism::{CanonicalAttrs, MarkingType, Span, TokenKind};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, RecanonScope, ReplacementIntent, SectionLetter, capco};

use crate::scheme::CapcoScheme;

// ===========================================================================
// Issue #677 — banner.metadata.uses-portion-form / portion.metadata.uses-banner-form
// ===========================================================================
//
// `MarkingScheme::render_canonical` owns the portion↔banner form-fix
// path, but the renderer only runs once a rule emits the
// `Recanonicalize` `FixIntent` that triggers it. Without these two
// rules, `SECRET//NF`, `(S//NOFORN)`, `SECRET//OC`, and parallel cases
// produce zero diagnostics. The two rules emit that intent.
//
// ## Scope decision (BROAD)
//
// Both rules walk every token in `attrs.token_spans` and dispatch through
// the `MARKING_FORMS` helpers `portion_to_banner` / `banner_to_portion`.
// Those helpers gate on the `f.banner != f.portion` condition built into
// `MARKING_FORMS`, so same-form entries (`FOUO`, `RELIDO`, `RD`, `TK`,
// etc.) return `None` and do not fire — exactly the desired filter.
//
// Coverage is therefore broad by construction:
// - Dissem pairs: NF↔NOFORN, OC↔ORCON, IMC↔IMCON, DSEN↔DEA SENSITIVE,
//   PR↔PROPIN, RS↔RSEN. (Authority: CAPCO-2016 §H.8 prose pp 132-167.)
// - Non-IC dissem: DS↔LIMDIS, XD↔EXDIS, ND↔NODIS, SBU-NF↔SBU NOFORN,
//   LES-NF↔LES NOFORN, DCNI↔DOD UCNI, UCNI↔DOE UCNI. (Authority:
//   CAPCO-2016 §H.9 prose pp 169-191.)
// - NATO classifications: CTS↔COSMIC TOP SECRET, NS↔NATO SECRET,
//   NC↔NATO CONFIDENTIAL, NR↔NATO RESTRICTED, NU↔NATO UNCLASSIFIED.
//   (Authority: CAPCO-2016 §H.2 p55, §G.1 Table 4 p36.)
// - SCI compounds: SI-EU↔SI-ECRU, SI-NK↔SI-NONBOOK. (Authority:
//   CAPCO-2016 §H.4 p78, p83.)
//
// US classification shorthand (S↔SECRET, TS↔TOP SECRET, C↔CONFIDENTIAL,
// U↔UNCLASSIFIED, R↔RESTRICTED) is NOT in `MARKING_FORMS` — the table's
// header doc-comment explicitly carves classification levels out
// because `Classification::banner_str` / `portion_str` own that mapping.
// The banner rule adds a small classification branch reading
// `attrs.classification` to cover this; it catches the PM-4 sister bug
// (`S//NOFORN` — classification abbreviation in banner) per CAPCO-2016
// §D.1 p27 ("The classification level must be in English without
// abbreviation"). One rule covers two gaps; no separate
// sister-bug issue is needed.
//
// ## Emission shape — ONE diagnostic per marking
//
// Both rules emit exactly ONE diagnostic per offending marking even when
// multiple tokens in the same marking carry the wrong form (e.g.,
// `S//NF` has both `S` and `NF` defective in a banner). The fix payload
// is `ReplacementIntent::Recanonicalize { scope }` — the engine's
// `render_canonical` re-emits the entire marking from canonical attrs,
// so a single intent at the marking scope covers every wrong-form token
// within it. Per-token emission would force the C-1 overlap guard to
// deduplicate effectively-identical `Recanonicalize` intents.
//
// The diagnostic's primary `span` is the first offending token's span
// (so the user sees where the violation is). The `candidate_span` is
// the marking-scope span (where the fix applies).
//
// ## EYES suppression — banner direction only
//
// Bare `EYES` / `EYES ONLY` in a banner is owned by E064
// (`EyesOnlyConvertToRelToRule`), whose §H.8 p157 + p158 authority
// covers the cross-axis conversion to `REL TO USA, AUS, CAN, GBR, NZL`
// (FVEY) on derivative use. `PortionFormInBannerRule` suppresses these
// tokens so the engine's C-1 overlap guard does not have to arbitrate
// between E064's richer cross-axis fix and our `Recanonicalize` — E064
// wins on §-grounded richness and the suppression keeps that intent
// reachable.
//
// E064 does NOT fire in portion context for bare `EYES` (§H.8 p158
// says "carry forward the trigraph codes listed in the source document
// banner line" — synthesis from a portion alone is not safe), so
// `BannerFormInPortionRule` does NOT suppress; the modest improvement
// of canonicalizing `(S//EYES ONLY)` → `(S//EYES)` via the portion-form
// re-render is better than silent acceptance.
//
// ## Authority
//
// - `PortionFormInBannerRule` emits at §D.1 p27 — "Any control markings
//   in the banner line may be spelled out per the 'Marking Title'
//   (e.g., TALENT KEYHOLE) or abbreviated as per the 'Authorized
//   Abbreviation' (e.g., TK)". A portion form (`NF`) is neither.
// - `BannerFormInPortionRule` emits at §C.1 p25 — "An authorized
//   portion mark is listed for each classification and control marking
//   entry in the Register." A banner form (`NOFORN`) is not the listed
//   portion mark.
//
// Each citation re-verified at authorship per Constitution VIII
// against `crates/capco/docs/CAPCO-2016.md` lines 503 and 560.

/// `capco:banner.metadata.uses-portion-form` — portion-form token
/// appearing in a banner-line marking. CAPCO-2016 §D.1 p27.
pub(super) struct PortionFormInBannerRule;

/// Citations `PortionFormInBannerRule` may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate
/// contract.
const PORTION_FORM_IN_BANNER_AUTHORITIES: &[Citation] = &[capco(SectionLetter::D, 1, 27)];

impl Rule<CapcoScheme> for PortionFormInBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "banner.metadata.uses-portion-form")
    }
    fn name(&self) -> &'static str {
        "portion-form-in-banner"
    }
    fn default_severity(&self) -> Severity {
        // Severity::Fix — deterministic auto-applicable canonicalization.
        // The closed Register (§G.1 Table 4 p38) guarantees a single
        // canonical banner form per marking; the renderer's
        // `render_canonical` produces those bytes from the parsed
        // attrs without classifier judgment.
        Severity::Fix
    }
    /// `Phase::WholeMarking`: the `Recanonicalize { Page }` intent
    /// covers the full banner span at promotion time. The diagnostic
    /// span points at the first offending token (a sub-region of the
    /// banner), but the fix scope is the whole marking by construction
    /// — matches the precedent set by E066
    /// (`LegacyNatoCompoundRemarkRule`).
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        PORTION_FORM_IN_BANNER_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Fire on Banner only. Portion-form-in-portion is canonical;
        // CAB/PageBreak/PageFinalization are not banner-line surfaces.
        if ctx.marking_type != MarkingType::Banner {
            return Vec::new();
        }
        if let Some(offending_span) = find_portion_form_in_banner(attrs) {
            vec![emit_form_mismatch(
                self.id(),
                self.default_severity(),
                offending_span,
                ctx.candidate_span,
                RecanonScope::Page,
                capco(SectionLetter::D, 1, 27),
            )]
        } else {
            Vec::new()
        }
    }
}

/// `capco:portion.metadata.uses-banner-form` — Authorized Banner Line
/// Marking Title OR Authorized Banner Line Abbreviation (i.e., either
/// column 1 or column 2 of §G.1 Table 4 p38) appearing in portion-mark
/// position where the canonical portion form per column 3 differs.
///
/// Authority: CAPCO-2016 §C.1 p25 (portion marks are Register-closed)
/// + §G.1 Table 4 p38 (the three columns are the authoritative
///   Title/Abbreviation/Portion-Mark surface).
pub(super) struct BannerFormInPortionRule;

/// Citations `BannerFormInPortionRule` may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate
/// contract.
const BANNER_FORM_IN_PORTION_AUTHORITIES: &[Citation] = &[capco(SectionLetter::C, 1, 25)];

impl Rule<CapcoScheme> for BannerFormInPortionRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.metadata.uses-banner-form")
    }
    fn name(&self) -> &'static str {
        "banner-form-in-portion"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    /// `Phase::WholeMarking`: mirror of `PortionFormInBannerRule`. The
    /// `Recanonicalize { Portion }` intent covers the full portion span;
    /// per-token splice would not let the engine canonicalize multiple
    /// wrong-form tokens in one pass.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        BANNER_FORM_IN_PORTION_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if ctx.marking_type != MarkingType::Portion {
            return Vec::new();
        }
        if let Some(offending_span) = find_banner_form_in_portion(attrs) {
            vec![emit_form_mismatch(
                self.id(),
                self.default_severity(),
                offending_span,
                ctx.candidate_span,
                RecanonScope::Portion,
                capco(SectionLetter::C, 1, 25),
            )]
        } else {
            Vec::new()
        }
    }
}

/// Walk `attrs` for a banner-line token using the portion form (or for
/// a US-classification token using its abbreviation). Returns the span
/// of the first offending token, or `None` if every token uses an
/// authorized banner form. The traversal terminates at the first hit
/// so the caller can emit exactly one diagnostic per marking — the
/// `Recanonicalize { Page }` fix re-renders all token positions at
/// promotion time, so additional defective tokens in the same banner
/// are covered by the single fix.
///
/// EYES / EYES ONLY tokens are skipped — E064 owns the §H.8 p157 +
/// p158 cross-axis conversion to `REL TO USA, AUS, CAN, GBR, NZL`
/// (FVEY). Suppressing those tokens here keeps E064's richer fix
/// reachable when both rules would otherwise produce overlapping
/// intents on the same span.
fn find_portion_form_in_banner(attrs: &CanonicalAttrs) -> Option<Span> {
    // Classification branch — US classifications carry portion forms
    // (S/TS/C/U/R) NOT in `MARKING_FORMS` (per the table's
    // doc-comment carve-out; classification mapping lives on
    // `Classification::banner_str` / `portion_str`).
    //
    // Read the US level via `CanonicalAttrs::us_classification()` so the
    // branch covers both `MarkingClassification::Us(_)` AND
    // `MarkingClassification::Conflict { us, .. }`. §D.1 p27 ("The
    // classification level must be in English without abbreviation")
    // applies to the US classification token regardless of whether the
    // banner also carries a NATO or JOINT side — the Conflict variant
    // is what the parser emits for those compound banners, and the
    // US-side token is still in banner position.
    if let Some(level) = attrs.us_classification() {
        let portion = level.portion_str();
        let banner = level.banner_str();
        if portion != banner {
            for token in attrs.token_spans.iter() {
                if token.kind == TokenKind::Classification && token.text.as_str() == portion {
                    return Some(token.span);
                }
            }
        }
    }
    // Token-walk branch — every other axis dispatches through
    // `MARKING_FORMS::portion_to_banner` (built-in `banner != portion`
    // gate is the universal filter).
    for token in attrs.token_spans.iter() {
        let text = token.text.as_ref();
        // E064 owns bare-EYES / EYES ONLY in banner — see the module-
        // header comment for the cross-rule suppression rationale.
        if text == "EYES" || text == "EYES ONLY" {
            continue;
        }
        if marque_ism::marking_forms::portion_to_banner(text).is_some() {
            return Some(token.span);
        }
    }
    None
}

/// Mirror of [`find_portion_form_in_banner`] for the portion-mark
/// direction. Returns the span of the first banner-form token, where
/// "banner form" means either an Authorized Banner Abbreviation
/// (§G.1 Table 4 p38 column 2, e.g., `NOFORN`, `IMCON`) OR an
/// Authorized Marking Title (§G.1 Table 4 p38 column 1, e.g.,
/// `TALENT KEYHOLE`, `ORIGINATOR CONTROLLED`,
/// `NOT RELEASABLE TO FOREIGN NATIONALS`) that has a distinct
/// portion form per column 3. The portion-mark Register surface is
/// closed (§C.1 p25 — "An authorized portion mark is listed for
/// each classification and control marking entry in the Register"),
/// so anything that maps to a different portion canonical via
/// either lookup is a form mismatch.
///
/// Two lookups are consulted in order:
///
/// 1. [`marque_ism::marking_forms::banner_to_portion`] covers the
///    `MARKING_FORMS.banner` column (Authorized Abbreviation column
///    of §G.1 Table 4). Catches `NOFORN`/`ORCON`/`IMCON`/etc. in
///    portion position.
/// 2. [`marque_ism::marking_forms::title_to_portion`] covers the
///    `MARKING_FORMS.title` column (Marking Title column). Catches
///    long-title forms like `TALENT KEYHOLE` (title) in portion
///    position where the canonical portion form is `TK`. The
///    `banner_to_portion` lookup misses these because its row gate
///    is `f.banner != f.portion`, and TALENT-KEYHOLE-class rows
///    have `f.banner == f.portion` (the Authorized Abbreviation
///    column matches the Portion Mark column — they happen to be
///    the same canonical bytes); only the Marking Title column
///    differs.
///
/// Does NOT skip EYES — E064 does not fire on bare-EYES in portion
/// context (§H.8 p158 says "carry forward the trigraph codes listed
/// in the source document banner line", which Marque cannot
/// synthesize from a portion alone), so the modest improvement of
/// canonicalizing `(S//EYES ONLY)` → `(S//EYES)` via the
/// portion-form re-render is strictly better than silent
/// acceptance. The `title_to_portion` lookup returns `None` for
/// `EYES ONLY` because its row has `title == banner`, so the
/// fallback is not a new source of EYES double-emit.
///
/// US classification banner forms (`SECRET`, `TOP SECRET`, etc.)
/// are NOT in `MARKING_FORMS`, and `Classification::banner_str`
/// returns `&'static str` not a `TokenId` — but the parser's
/// classification recognizer accepts these long forms in either
/// position and canonicalizes `attrs.classification` regardless.
/// The portion rule does not surface "long-form classification in
/// portion" today because there is no MARKING_FORMS row to drive
/// the `banner_to_portion` lookup; that case is the symmetric gap
/// to the banner-direction classification branch above, deferred to
/// a follow-up if needed (the more common direction is the
/// abbreviation-in-banner case PM-4 names).
fn find_banner_form_in_portion(attrs: &CanonicalAttrs) -> Option<Span> {
    for token in attrs.token_spans.iter() {
        let text = token.text.as_ref();
        // Banner Abbreviation column — `NOFORN`/`ORCON`/`IMCON`/etc.
        if marque_ism::marking_forms::banner_to_portion(text).is_some() {
            return Some(token.span);
        }
        // Marking Title column — `TALENT KEYHOLE`/`ORIGINATOR CONTROLLED`/
        // `NOT RELEASABLE TO FOREIGN NATIONALS`/etc. Only fires when the
        // Title column differs from the Authorized Abbreviation column
        // (the `title != banner` gate inside `title_to_portion`), so
        // same-form-as-banner titles (e.g., `DEA SENSITIVE` where
        // `title == banner == "DEA SENSITIVE"`) cannot double-emit:
        // `banner_to_portion("DEA SENSITIVE")` returns `Some("DSEN")`
        // above and short-circuits; `title_to_portion("DEA SENSITIVE")`
        // returns `None` and would never reach here anyway. Authority:
        // §C.1 p25 (portion mark must be the listed Register entry) +
        // §G.1 Table 4 p38 (Register-closed-set governs both column 1
        // Marking Title and column 2 Banner Abbreviation as the
        // authorized banner forms — neither is the portion form).
        if marque_ism::marking_forms::title_to_portion(text).is_some() {
            return Some(token.span);
        }
    }
    None
}

/// Construct a form-mismatch diagnostic. Shared between the banner-
/// and portion-direction rules because the emission shape is
/// identical — only the rule id, recanon scope, and citation differ.
///
/// The fix payload is `ReplacementIntent::Recanonicalize { scope }`
/// at confidence 1.0 (deterministic per the §G.1 Table 4 closed set).
/// `MessageArgs::default()` mirrors the E005 / E006 precedent: the
/// closed-token set is too varied to bind every form-pair to a
/// `TokenId`, and the per-rule predicate ID plus the diagnostic span
/// already identify the violation kind and location for audit
/// consumers (Constitution V — no document bytes flow
/// through the typed message).
fn emit_form_mismatch(
    rule: RuleId,
    severity: Severity,
    offending_span: Span,
    candidate_span: Span,
    scope: RecanonScope,
    citation: Citation,
) -> Diagnostic<CapcoScheme> {
    let fix_intent = FixIntent {
        replacement: ReplacementIntent::Recanonicalize { scope },
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        message: Message::new(MessageTemplate::WrongTokenForm, MessageArgs::default()),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    Diagnostic::with_fix_at_span(
        rule,
        severity,
        offending_span,
        candidate_span,
        Message::new(MessageTemplate::WrongTokenForm, MessageArgs::default()),
        citation,
        fix_intent,
    )
}
