// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI (Sensitive Compartmented Information) rules.
//!
//! - [`SciCustomControlInfoRule`] (`capco:marking.sci.custom-control-audit-visibility`)
//! - [`HcsBareAtConfidentialLegacyRemarkRule`] (`capco:portion.sci.hcs-bare-at-confidential-legacy`)
//! - [`HcsBareSuggestSubcompartmentRule`] (`capco:portion.sci.hcs-bare-suggest-subcompartment`)
//! - [`RsvBareRequiresCompartmentRule`] (`capco:portion.sci.rsv-bare-requires-compartment`)
//!
//! Plus the [`sci_system_text`] / [`render_sci_block`] shared helpers
//! consumed by the SCI banner-rollup evaluator in `rules/banner/eval_sci.rs`.

use marque_ism::{CanonicalAttrs, SciControlSystem, SciMarking, Span, TokenKind, TokenSpan};
use marque_rules::{
    Confidence, Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase, Rule,
    RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: W034 — SCI custom-control audit visibility
// ---------------------------------------------------------------------------

/// Per CAPCO-2016 §A.6 p16 + §H.4 p61: unpublished (agency-allocated) SCI
/// control systems are legitimate — the manual describes ODNI/P&S's
/// unpublished registry and explicitly permits these markings. This rule
/// surfaces each Custom control identifier so a classifier can verify the
/// allocation is registered.
///
/// # Severity: Warn (default)
///
/// Field experience: the four spelled-out SCI controls in CAPCO (SI, TK,
/// RSV, HCS) account for the vast majority (>99%) of real-world SCI
/// control usage. Seeing an unpublished control is more likely a typo,
/// stale legacy marking, or unregistered use than a valid agency
/// allocation. `Warn` reflects that rarity without making it
/// error-level by default. (Note: `Warn` still produces a non-zero
/// CLI exit via `EX_DIAG_WARN`, so orgs that treat any warning as
/// CI-blocking should configure `W034 = "info"` if they want
/// audit-visibility only.)
///
/// T035c-2 landed the `Severity::Info` variant and dropped the earlier
/// `Severity::Off` workaround. Previously, the rule emitted `Diagnostic`
/// values at `Severity::Off` — a state `Principle IV` declares
/// unrepresentable — and relied on the test harness bypassing
/// engine-level severity filtering to observe the diagnostics. That was
/// a constitutional-invariant violation. Users who want informational
/// (non-warn) treatment can configure `W034 = "info"` in `.marque.toml`;
/// users who want it silent can configure `W034 = "off"`.
pub(crate) struct SciCustomControlInfoRule;

/// Citations W034 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const SCI_CUSTOM_CONTROL_INFO_AUTHORITIES: &[Citation] = &[capco(SectionLetter::A, 6, 16)];

impl Rule<CapcoScheme> for SciCustomControlInfoRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.sci.unpublished-custom-control")
    }
    fn name(&self) -> &'static str {
        "sci-custom-control-info"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: audit-visibility surface for unpublished SCI
    /// control identifiers. No fix emitted; the diagnostic flags every
    /// Custom-control span in the marking. Decision is per-marking, not
    /// per-token.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        SCI_CUSTOM_CONTROL_INFO_AUTHORITIES
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();

        let mut out = Vec::new();
        for (idx, marking) in attrs.sci_markings.iter().enumerate() {
            if let SciControlSystem::Custom(text) = &marking.system {
                // Plausible-allocation suppression: 1-3 ASCII-uppercase
                // identifiers are within the typical CAPCO-2016 §A.6 p15
                // agency-allocated shape and don't warrant per-marking
                // audit-visibility noise. W034 still fires on anything
                // outside this shape (digits, longer identifiers,
                // unusual casing) where the chance of typo or
                // unregistered use is materially higher. Citation:
                // CAPCO-2016 §A.6 p15 (agency-allocated control
                // identifier shape) + §H.4 p61 (publication channel).
                let s = text.as_str();
                let is_plausible_allocation =
                    (1..=3).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase());
                if is_plausible_allocation {
                    continue;
                }
                let span = sys_spans
                    .get(idx)
                    .map(|t| t.span)
                    .unwrap_or(Span::new(0, 0));
                // G13: drop runtime byte text. Template names the
                // unpublished-control class.
                let _ = s;
                out.push(Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    span,
                    Message::new(
                        MessageTemplate::UnpublishedSciControl,
                        MessageArgs::default(),
                    ),
                    capco(SectionLetter::A, 6, 16),
                    None,
                ));
            }
        }
        out
    }
}

// ===========================================================================
// E061 — Bare HCS at CONFIDENTIAL (class-specific legacy guidance)
// ===========================================================================
//
// §H.4 p62 carries a class-specific note for legacy CONFIDENTIAL//HCS
// information: "When legacy information at the CONFIDENTIAL//HCS level
// is discovered, contact the originator for guidance prior to reusing
// the information." Distinct from the general bare-HCS guidance that
// recommends the HCS-O / HCS-P / HCS-O-P templates (covered by E010).
//
// E061 fires only when classification is CONFIDENTIAL AND a bare HCS
// is present. The diagnostic carries no fix (the manual prescribes
// contacting the originator, not a mechanical re-mark).
//
// Bare HCS is a structurally-incomplete marking, not an invalid one —
// the HCS control system is canonical per §H.4 p62; the user just
// hasn't specified the required compartment. Marque can't pick the
// compartment without content-domain context. Severity::Warn (not
// Error): the marking will be valid once the user adds the compartment;
// the rule's job is to surface the gap, not to claim the marking is
// structurally invalid. Contrast with E065's deprecated-control-system
// rows (bare KDK/KLONDIKE/EL/ENDSEAL/ECI) where the source control
// system itself is retired and the marking has no canonical migration.

/// Rule E061 — bare HCS at CONFIDENTIAL: legacy guidance per §H.4 p62.
pub(crate) struct HcsBareAtConfidentialLegacyRemarkRule;

/// Citations E061 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const HCS_BARE_AT_CONFIDENTIAL_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 4, 62)];

impl Rule<CapcoScheme> for HcsBareAtConfidentialLegacyRemarkRule {
    fn id(&self) -> RuleId {
        RuleId::new(
            "capco",
            "portion.sci.hcs-bare-at-confidential-legacy-remark",
        )
    }
    fn name(&self) -> &'static str {
        "hcs-bare-at-confidential-legacy-remark"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: needs cross-token classification + SCI
    /// state to determine "bare HCS at CONFIDENTIAL" class-specific
    /// trigger. No fix emitted; the manual prescribes contacting the
    /// originator.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        HCS_BARE_AT_CONFIDENTIAL_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{Classification, SciControlBare, SciControlSystem};

        // Class-specific gate: only fires at CONFIDENTIAL.
        if attrs.us_classification() != Some(Classification::Confidential) {
            return vec![];
        }

        // Find bare HCS (Published Hcs system with no compartments).
        let bare_hcs_idx = attrs.sci_markings.iter().position(|m| {
            matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                && m.compartments.is_empty()
        });
        let Some(idx) = bare_hcs_idx else {
            return vec![];
        };

        // Anchor span at the bare HCS SciSystem token. The structural
        // parser emits one `TokenKind::SciSystem` per SCI marking; we
        // index by position to align with the matched `sci_markings`
        // entry. Defensive fallback to `Span::new(0, 0)` if the spans
        // got out of sync (would indicate a parser regression caught
        // elsewhere).
        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();
        let span = sys_spans
            .get(idx)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
            capco(SectionLetter::H, 4, 62),
            None,
        )]
    }
}

// ===========================================================================
// E062 — Bare HCS at SECRET / TOP SECRET (legacy form; suggest templates)
// ===========================================================================
//
// §H.4 p62 (general bare-HCS guidance): "When incorporating legacy
// material marked 'HCS' into a new product, re-mark the new document
// and associated portion according to the instructions in the HCS-O
// and HCS-P marking templates."
//
// E062 fires at SECRET / TOP SECRET (the class levels where HCS-O /
// HCS-P / HCS-O-P are authorized). It emits per-candidate Suggest-
// severity diagnostics for HCS-O, HCS-P, and HCS-O-P. The choice
// between them is a content-domain decision Marque cannot make:
// HCS-O is operational source information; HCS-P is analytical
// product; HCS-O-P is both. Surfacing 3 candidates lets the
// classifier pick.
//
// Distinct from E010: E010 fires at any class level with a single
// text-only "consult HCS-O/HCS-P templates" message. E062 emits
// per-candidate text_corrections so editors can offer one-click
// substitution. Orgs that want either rule silenced configure
// `.marque.toml [rules] E062 = "off"` (or E010 = "off").

/// Rule E062 — bare HCS at S/TS: suggest HCS-O / HCS-P / HCS-O-P
/// templates per §H.4 p62.
pub(crate) struct HcsBareSuggestSubcompartmentRule;

/// Citations E062 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const HCS_BARE_SUGGEST_SUB_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 4, 62)];

impl Rule<CapcoScheme> for HcsBareSuggestSubcompartmentRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.sci.hcs-bare-suggest-subcompartment")
    }
    fn name(&self) -> &'static str {
        "hcs-bare-suggest-subcompartment"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: needs cross-token classification + SCI
    /// state to gate "S/TS class level". Emits per-candidate
    /// text_corrections at Suggest severity so the engine never
    /// auto-applies; the classifier picks via UI.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        HCS_BARE_SUGGEST_SUB_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{Classification, SciControlBare, SciControlSystem};

        // Class-specific gate: only fires at SECRET / TOP SECRET.
        let class = attrs.us_classification();
        if !matches!(
            class,
            Some(Classification::Secret) | Some(Classification::TopSecret)
        ) {
            return vec![];
        }

        // Find bare HCS (Published Hcs system with no compartments).
        let Some(idx) = attrs.sci_markings.iter().position(|m| {
            matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                && m.compartments.is_empty()
        }) else {
            return vec![];
        };

        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();
        let span = sys_spans
            .get(idx)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        // Emit per-candidate Suggest-severity diagnostics. Each carries
        // a text_correction whose `replacement` is the canonical short
        // form for the matching sub-compartment. The engine never
        // auto-applies Suggest-severity diagnostics by construction
        // (Severity::Suggest is a hard exclusion in Engine::fix); the
        // candidates surface in the editor / CLI for human selection.
        //
        // Diagnostics emit at Severity::Suggest by default — the engine
        // preserves the per-diagnostic severity when no
        // `.marque.toml [rules] E062 = "..."` override is configured
        // (engine.rs:1001-1007 applies the override only when present).
        // Suggest prevents auto-apply, so the classifier picks among
        // the three candidates. To escalate to Warn or Error at the
        // user surface, the operator configures
        // `[rules] E062 = "warn"` in `.marque.toml`.
        let candidates: &[&str] = &["HCS-O", "HCS-P", "HCS-O-P"];
        let mut out = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            // G13: candidate replacement is on the audit permitted list
            // (canonical token from a closed set); the typed `Message`
            // identifies the superseded-token class.
            out.push(Diagnostic::text_correction(
                self.id(),
                Severity::Suggest,
                span,
                Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                capco(SectionLetter::H, 4, 62),
                *candidate,
                FixSource::BuiltinRule,
                // Confidence 0.75: the canonical replacement is one of
                // three, and Marque cannot pick the right one. The
                // value is below typical auto-apply thresholds (0.95)
                // so even an engine that ignored the Suggest gate
                // would not auto-apply.
                Confidence::strict(0.75),
                None,
            ));
        }
        out
    }
}

// ===========================================================================
// E063 — Bare RSV requires compartment (§H.4 p70)
// ===========================================================================
//
// §H.4 p70: "the RSV marking may not be used alone and requires the
// associated compartment". §H.4 p72: `RSV-[COMPARTMENT]` (3-alnum),
// TS/S only, requires RESERVE.
//
// Bare RSV is a structurally-incomplete marking, not an invalid one —
// the RESERVE control system is canonical per §H.4 p70; the user just
// hasn't specified the required compartment. Marque can't pick the
// compartment without content-domain context (the compartment identifier
// is org-private and not in the public vocabulary). Severity::Warn (not
// Error): the marking will be valid once the user adds the compartment;
// the rule's job is to surface the gap, not to claim the marking is
// structurally invalid. Contrast with E065's deprecated-control-system
// rows (bare KDK/KLONDIKE/EL/ENDSEAL/ECI) where the source control
// system itself is retired and the marking has no canonical migration.
// Suggest-only (no fix proposed) because the compartment identifier is
// org-private content beyond Marque's vocabulary.

/// Rule E063 — bare RSV requires compartment per §H.4 p70.
pub(crate) struct RsvBareRequiresCompartmentRule;

/// Citations E063 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const RSV_BARE_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 4, 70)];

impl Rule<CapcoScheme> for RsvBareRequiresCompartmentRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.sci.rsv-bare-requires-compartment")
    }
    fn name(&self) -> &'static str {
        "rsv-bare-requires-compartment"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: needs cross-token SCI state to find bare
    /// RSV (no compartment). No fix emitted; the compartment
    /// identifier is org-private content beyond Marque's vocabulary.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        RSV_BARE_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{SciControlBare, SciControlSystem};

        // Find bare RSV (Published Rsv system with no compartments).
        let Some(idx) = attrs.sci_markings.iter().position(|m| {
            matches!(m.system, SciControlSystem::Published(SciControlBare::Rsv))
                && m.compartments.is_empty()
        }) else {
            return vec![];
        };

        let sys_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::SciSystem)
            .collect();
        let span = sys_spans
            .get(idx)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
            capco(SectionLetter::H, 4, 70),
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// SCI rule helpers
// ---------------------------------------------------------------------------

/// Returns the text form of a SciControlSystem for sort/display purposes.
pub(crate) fn sci_system_text(system: &SciControlSystem) -> &str {
    match system {
        SciControlSystem::Published(bare) => bare.as_str(),
        SciControlSystem::Custom(text) => text.as_ref(),
        // NATO SAPs (BOHEMIA, BALK) per CAPCO-2016 §G.2 p40 + §H.7 p127.
        SciControlSystem::NatoSap(sap) => sap.as_str(),
    }
}

/// Render a list of SciMarkings back to the canonical wire form used in a
/// banner's SCI block — systems joined by `/`, each system's compartments
/// joined by `-`, and sub-compartments space-separated after a compartment.
/// Systems and compartments are emitted in source order; callers are
/// responsible for pre-sorting if they want canonical ascending output.
pub(crate) fn render_sci_block(markings: &[SciMarking]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(markings.len());
    for m in markings {
        let mut piece = sci_system_text(&m.system).to_owned();
        for comp in m.compartments.iter() {
            piece.push('-');
            piece.push_str(comp.identifier.as_ref());
            for sub in comp.sub_compartments.iter() {
                piece.push(' ');
                piece.push_str(sub.as_ref());
            }
        }
        parts.push(piece);
    }
    parts.join("/")
}

// PR 9b (T133): the `page_expected_sci_markings` helper retired with
// the migration of `evaluate_sci_banner_rollup` to read
// `ProjectedMarking::sci_markings` directly. Banner-validation rules
