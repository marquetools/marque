// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T035d: Per-SCI-system constraint rules (E042–E051).
//!
//! CAPCO-2016 §H.4 spells out per-system constraints on classification
//! level, required companion dissem controls, and forbidden companions
//! for every SCI control system. This module implements those as ten
//! `Warn`-severity rules that use the **fix-and-warn** pattern: when a
//! violation has an unambiguous resolution, we attach a fix AND surface
//! the warning, so the user sees exactly what was corrected and can
//! override if the intent was actually different.
//!
//! # Why fix-and-warn for these rules
//!
//! A presence-of-compartment signal (`HCS-P ABCDEF`, `TK-BLFH`) is a
//! deliberate act — it's not a one-keystroke typo. A classification
//! letter (`S` vs `TS`) is; missing a keystroke on the `T` turns `TS`
//! into `S`. Treating the compartment as the stronger intent signal and
//! the classification letter as the more-likely-typo'd field is the
//! safer default under Constitution V: the fix is applied, the audit
//! record captures both the original and the correction, and the user
//! sees the diagnostic so they can flip it if the intent was actually
//! SECRET (in which case the answer is to remove the compartment, not
//! edit the letter).
//!
//! # Why Warn + fix rather than Fix severity
//!
//! `Severity::Fix` hides the diagnostic from human output when the fix
//! is applied. That's the right call for mechanical fixes (typo
//! corrections, format normalizations) where surfacing the correction
//! would be noise. For SCI compliance corrections the *opposite* is
//! true — the user MUST see "we upgraded your classification because X
//! required TS," both so they can verify the fix matched their intent
//! and so the correction becomes a learning signal ("next time I marked
//! this, I should have caught it myself"). `Warn` + attached
//! `FixProposal` gives both: the engine applies the fix when confidence
//! clears threshold, AND the diagnostic stays in the output stream.
//!
//! # Range-ceiling ambiguity
//!
//! Three rules (E045, E048, E049) cover "TS-or-S" ceilings. When the
//! observed classification is below both valid floors (C or U), the
//! resolution is genuinely ambiguous — the user might have meant S or
//! TS, and auto-picking either could silently mis-classify. These fire
//! at `Warn` with **no fix**; the user chooses whether to upgrade or
//! remove the SCI marking.

use marque_ism::{
    Classification, DissemControl, IsmAttributes, MarkingClassification, MarkingType,
    SciControlBare, SciControlSystem, SciMarking, Span, TokenKind, TokenSpan,
};
use marque_rules::{Diagnostic, FixSource, Rule, RuleContext, RuleId, Severity};

use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};

// ===========================================================================
// Helpers — shared by every rule in this module
// ===========================================================================

/// Is this `SciMarking` anchored on the given published bare system?
fn anchors_on(m: &SciMarking, system: SciControlBare) -> bool {
    matches!(&m.system, SciControlSystem::Published(s) if *s == system)
}

/// Does any compartment under this marking carry the given identifier?
fn has_compartment(m: &SciMarking, id: &str) -> bool {
    m.compartments
        .iter()
        .any(|c| c.identifier.as_ref() == id)
}

/// Does the specific compartment carry at least one sub-compartment?
fn compartment_has_sub(m: &SciMarking, comp_id: &str) -> bool {
    m.compartments
        .iter()
        .any(|c| c.identifier.as_ref() == comp_id && !c.sub_compartments.is_empty())
}

/// Is this a TK-BLFH, TK-IDIT, or TK-KAND marking (the three TK
/// compartments that require NOFORN per §H.4 p87 / p91 / p95)?
fn is_tk_noforn_compartment(m: &SciMarking) -> bool {
    anchors_on(m, SciControlBare::Tk)
        && m.compartments
            .iter()
            .any(|c| matches!(c.identifier.as_ref(), "BLFH" | "IDIT" | "KAND"))
}

/// Find the first SCI-system/SCI-control token span in document order.
/// Used as the diagnostic pointer when the rule fires on a portion's
/// SCI block.
fn first_sci_span(attrs: &IsmAttributes) -> Option<Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| {
            matches!(
                t.kind,
                TokenKind::SciSystem
                    | TokenKind::SciControl
                    | TokenKind::SciCompartment
                    | TokenKind::SciSubCompartment
            )
        })
        .map(|t| t.span)
}

/// Observed US classification level, if any. Returns `None` for pure
/// foreign classifications (FGI/NATO/JOINT) — SCI-on-foreign is out of
/// §H.4's scope and handled by the foreign-classification rule cluster.
fn us_level(attrs: &IsmAttributes) -> Option<Classification> {
    match attrs.classification {
        Some(MarkingClassification::Us(c)) => Some(c),
        Some(MarkingClassification::Conflict { us, .. }) => Some(us),
        _ => None,
    }
}

/// Classification-token span + current text, if present.
fn classification_token(attrs: &IsmAttributes) -> Option<&TokenSpan> {
    attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::Classification)
}

/// Last token span of the IC dissem block (to anchor zero-width
/// insertions). Returns `None` when no IC dissem token exists.
fn last_dissem_span(attrs: &IsmAttributes) -> Option<Span> {
    attrs
        .token_spans
        .iter()
        .rev()
        .find(|t| t.kind == TokenKind::DissemControl)
        .map(|t| t.span)
}

/// Find the span of a specific `DissemControl` token (used when a rule
/// needs to replace e.g. `OC-USGOV` with `OC`).
fn dissem_token_span(attrs: &IsmAttributes, target: DissemControl) -> Option<(Span, &str)> {
    for (dissem_idx, d) in attrs.dissem_controls.iter().enumerate() {
        if *d == target {
            // Walk token_spans to find the Nth DissemControl.
            let tok = attrs
                .token_spans
                .iter()
                .filter(|t| t.kind == TokenKind::DissemControl)
                .nth(dissem_idx)?;
            return Some((tok.span, tok.text.as_ref()));
        }
    }
    None
}

/// Banner-form ORCON / NOFORN representation, given the current form used
/// on this marking. The parser populates `text.as_ref()` with whatever
/// the user wrote, so inserting in matching form avoids a surprise
/// mixed-form output.
fn infer_companion_form(attrs: &IsmAttributes) -> CompanionForm {
    // Peek at the first dissem token's text: if it's a short-form
    // (e.g., "NF", "OC"), the portion is written in abbrev style.
    let first = attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DissemControl);
    match first.map(|t| t.text.as_ref()) {
        Some("NF") | Some("OC") | Some("OC-USGOV") => CompanionForm::Abbreviated,
        _ => CompanionForm::Full,
    }
}

#[derive(Copy, Clone)]
enum CompanionForm {
    Abbreviated,
    Full,
}

impl CompanionForm {
    fn orcon(self) -> &'static str {
        match self {
            Self::Abbreviated => "OC",
            Self::Full => "ORCON",
        }
    }
    fn noforn(self) -> &'static str {
        match self {
            Self::Abbreviated => "NF",
            Self::Full => "NOFORN",
        }
    }
}

/// Build a fix that replaces the classification token with the target
/// level, preserving the abbrev / full-form choice based on the
/// `marking_type`. Returns `None` when the classification token is
/// missing (no class to upgrade) or already at target.
fn build_class_upgrade_fix(
    attrs: &IsmAttributes,
    ctx: &RuleContext,
    target: Classification,
) -> Option<(Span, String, String)> {
    let class_tok = classification_token(attrs)?;
    let desired = match ctx.marking_type {
        MarkingType::Portion => target.portion_str(),
        _ => target.banner_str(),
    };
    let current = class_tok.text.as_ref();
    if current == desired {
        return None;
    }
    Some((class_tok.span, current.to_owned(), desired.to_owned()))
}

// ===========================================================================
// E042: HCS-O companions — requires ORCON + NOFORN, forbids ORCON-USGOV
// ===========================================================================

/// Fires on any portion or banner carrying HCS-O. One Warn diagnostic
/// per detected companion defect:
///
/// - missing ORCON → insert `/OC` (or `/ORCON`)
/// - missing NOFORN → insert `/NF` (or `/NOFORN`)
/// - ORCON-USGOV present → replace with `OC`/`ORCON`
///
/// No-fix Error when the portion has no IC dissem block at all and
/// ORCON/NOFORN would need to be inserted (inserting a whole new
/// category block is unsafe from rule context; same policy as E040).
///
/// Authority: **CAPCO-2016 §H.4 p64** — HCS-O *Relationship(s) to Other
/// Markings*: "May only be used with TOP SECRET or SECRET. Requires
/// ORCON and NOFORN. May not be used with ORCON-USGOV."
pub(crate) struct HcsOCompanionsRule;

impl Rule for HcsOCompanionsRule {
    fn id(&self) -> RuleId {
        RuleId::new("E042")
    }
    fn name(&self) -> &'static str {
        "hcs-o-companions"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // Does any marking on this portion carry HCS-O (bare-HCS
        // anchor + "O" compartment)?
        let has_hcs_o = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "O"));
        if !has_hcs_o {
            return vec![];
        }

        let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc);
        let has_noforn = attrs.dissem_controls.contains(&DissemControl::Nf);
        let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

        let mut out = Vec::new();
        let form = infer_companion_form(attrs);
        let last_dissem = last_dissem_span(attrs);
        let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

        // Missing ORCON → insert
        if !has_orcon {
            out.push(emit_companion_insert(
                RuleId::new("E042"),
                Severity::Warn,
                sci_span,
                last_dissem,
                form.orcon(),
                "HCS-O requires ORCON (§H.4 p64)".to_owned(),
                "CAPCO-2016 §H.4 p64 — HCS-O requires ORCON",
            ));
        }
        // Missing NOFORN → insert
        if !has_noforn {
            out.push(emit_companion_insert(
                RuleId::new("E042"),
                Severity::Warn,
                sci_span,
                last_dissem,
                form.noforn(),
                "HCS-O requires NOFORN (§H.4 p64)".to_owned(),
                "CAPCO-2016 §H.4 p64 — HCS-O requires NOFORN",
            ));
        }
        // ORCON-USGOV present → replace with ORCON
        if let Some((span, text)) = usgov_entry {
            let replacement = form.orcon().to_owned();
            out.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: RuleId::new("E042"),
                severity: Severity::Warn,
                source: FixSource::BuiltinRule,
                span,
                message: "HCS-O forbids ORCON-USGOV (§H.4 p64) — replace with ORCON"
                    .to_owned(),
                citation: "CAPCO-2016 §H.4 p64 — HCS-O may not be used with ORCON-USGOV",
                original: text.to_owned(),
                replacement,
                confidence: 0.9,
                migration_ref: None,
            }));
        }
        out
    }
}

// ===========================================================================
// E043: HCS-P requires NOFORN
// ===========================================================================

/// Fires on any portion or banner carrying HCS-P (including HCS-P with
/// sub-compartments) when NOFORN is absent. Fix: insert `/NF` (or
/// `/NOFORN`) at the end of the IC dissem block. Error no-fix when no
/// dissem block exists.
///
/// Authority: **CAPCO-2016 §H.4 p66** — HCS-P *Relationship(s) to Other
/// Markings*: "Requires NOFORN. ORCON or ORCON-USGOV may be used."
pub(crate) struct HcsPRequiresNofornRule;

impl Rule for HcsPRequiresNofornRule {
    fn id(&self) -> RuleId {
        RuleId::new("E043")
    }
    fn name(&self) -> &'static str {
        "hcs-p-requires-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let has_hcs_p = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "P"));
        if !has_hcs_p {
            return vec![];
        }
        let has_noforn = attrs.dissem_controls.contains(&DissemControl::Nf);
        if has_noforn {
            return vec![];
        }
        let form = infer_companion_form(attrs);
        let last_dissem = last_dissem_span(attrs);
        let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));
        vec![emit_companion_insert(
            RuleId::new("E043"),
            Severity::Warn,
            sci_span,
            last_dissem,
            form.noforn(),
            "HCS-P requires NOFORN (§H.4 p66)".to_owned(),
            "CAPCO-2016 §H.4 p66 — HCS-P requires NOFORN",
        )]
    }
}

// ===========================================================================
// E044: HCS-P sub-compartment TS-only
// ===========================================================================

/// Fires on any portion or banner carrying HCS-P with at least one
/// sub-compartment when the US classification is below TS. Fix:
/// upgrade classification token to TS. The sub-compartment is treated
/// as the stronger intent signal (see module doc on the fix-and-warn
/// rationale).
///
/// Authority: **CAPCO-2016 §H.4 p68** — HCS-P sub-compartment
/// *Relationship(s) to Other Markings*: "May only be used with TOP
/// SECRET. Requires HCS-P, ORCON, and NOFORN. May not be used with
/// ORCON-USGOV."
///
/// (This rule covers the TS-only constraint. E043 covers NOFORN; the
/// ORCON + no-ORCON-USGOV half of p68 composes cleanly with E042's
/// HCS-O coverage when an HCS-P sub-compartment co-occurs with HCS-O.)
pub(crate) struct HcsPSubcompartmentTsOnlyRule;

impl Rule for HcsPSubcompartmentTsOnlyRule {
    fn id(&self) -> RuleId {
        RuleId::new("E044")
    }
    fn name(&self) -> &'static str {
        "hcs-p-subcompartment-top-secret"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        let has_hcs_p_sub = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Hcs) && compartment_has_sub(m, "P"));
        if !has_hcs_p_sub {
            return vec![];
        }
        let Some(level) = us_level(attrs) else {
            return vec![];
        };
        if level >= Classification::TopSecret {
            return vec![];
        }
        let Some((span, original, replacement)) =
            build_class_upgrade_fix(attrs, ctx, Classification::TopSecret)
        else {
            return vec![];
        };
        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: RuleId::new("E044"),
            severity: Severity::Warn,
            source: FixSource::BuiltinRule,
            span,
            message: format!(
                "HCS-P sub-compartment requires TOP SECRET; upgraded {original:?} → \
                 {replacement:?}. If this should be SECRET, remove the HCS-P \
                 sub-compartment (§H.4 p68)."
            ),
            citation: "CAPCO-2016 §H.4 p68 — HCS-P sub-compartment: May only be used with TOP SECRET",
            original,
            replacement,
            confidence: 0.9,
            migration_ref: None,
        })]
    }
}

// ===========================================================================
// E045: HCS classification ceiling (TS or S, warn only — ambiguous)
// ===========================================================================

/// Fires on any portion or banner carrying HCS-O or HCS-P when the US
/// classification is below SECRET (i.e., CONFIDENTIAL or UNCLASSIFIED).
/// **No fix** — the resolution is ambiguous (upgrade to S? to TS?
/// remove the HCS marking?); the user decides.
///
/// Authority: **CAPCO-2016 §H.4 p64** (HCS-O) and **p66** (HCS-P) —
/// both constrain *"May only be used with TOP SECRET or SECRET."*
/// HCS-P sub-compartment is TS-only and covered by E044; that case
/// is pre-emptied here (a TS-only marking that's below TS is also
/// below the HCS min, so firing both would be noise).
pub(crate) struct HcsClassificationCeilingRule;

impl Rule for HcsClassificationCeilingRule {
    fn id(&self) -> RuleId {
        RuleId::new("E045")
    }
    fn name(&self) -> &'static str {
        "hcs-classification-ceiling"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // Cover HCS-O and HCS-P (with or without sub-compartments). Bare
        // HCS is legacy — E006 / E008 already surface it; no need to
        // double-diagnose here.
        let has_hcs_o_or_p = attrs.sci_markings.iter().any(|m| {
            anchors_on(m, SciControlBare::Hcs)
                && (has_compartment(m, "O") || has_compartment(m, "P"))
        });
        if !has_hcs_o_or_p {
            return vec![];
        }
        let Some(level) = us_level(attrs) else {
            return vec![];
        };
        if level >= Classification::Secret {
            return vec![];
        }
        let Some(class_tok) = classification_token(attrs) else {
            return vec![];
        };
        vec![Diagnostic::new(
            RuleId::new("E045"),
            Severity::Warn,
            class_tok.span,
            "HCS requires TOP SECRET or SECRET; resolve by upgrading the \
             classification or removing the HCS marking (§H.4 p64, p66)",
            "CAPCO-2016 §H.4 p64 (HCS-O) + p66 (HCS-P) — May only be used with TOP SECRET or SECRET",
            None,
        )]
    }
}

// ===========================================================================
// E046: SI compartment TS-only (any SI-[comp], GAMMA or not)
// ===========================================================================

/// Fires on any portion or banner carrying a SI compartment (GAMMA or
/// non-GAMMA) when the US classification is below TS. Fix: upgrade to
/// TS. Bare `SI` is unaffected (SI alone is allowed on C/S/TS per §H.4
/// p74) — this rule only fires when SI has a compartment attached.
///
/// Authority:
/// - **CAPCO-2016 §H.4 p76** — SI non-GAMMA compartment: *"Applicable
///   only to Top Secret information. May only be used with TOP SECRET.
///   Requires SI."*
/// - **CAPCO-2016 §H.4 p80** — SI-G: *"Applicable only to Top Secret
///   information. May only be used with TOP SECRET. Requires SI and
///   ORCON."* (ORCON requirement covered by E047.)
/// - **CAPCO-2016 §H.4 p81** — SI-G sub-compartment: same TS-only
///   constraint (inherited).
pub(crate) struct SiCompartmentTopSecretRule;

impl Rule for SiCompartmentTopSecretRule {
    fn id(&self) -> RuleId {
        RuleId::new("E046")
    }
    fn name(&self) -> &'static str {
        "si-compartment-top-secret"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        let has_si_comp = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Si) && !m.compartments.is_empty());
        if !has_si_comp {
            return vec![];
        }
        let Some(level) = us_level(attrs) else {
            return vec![];
        };
        if level >= Classification::TopSecret {
            return vec![];
        }
        let Some((span, original, replacement)) =
            build_class_upgrade_fix(attrs, ctx, Classification::TopSecret)
        else {
            return vec![];
        };
        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: RuleId::new("E046"),
            severity: Severity::Warn,
            source: FixSource::BuiltinRule,
            span,
            message: format!(
                "SI compartments require TOP SECRET; upgraded {original:?} → \
                 {replacement:?}. If this should be SECRET/CONFIDENTIAL, remove \
                 the compartment and mark as bare SI (§H.4 p76, p80, p81)."
            ),
            citation: "CAPCO-2016 §H.4 p76 (SI-[comp]) + p80 (SI-G) + p81 (SI-G sub-comp) — TS-only",
            original,
            replacement,
            confidence: 0.9,
            migration_ref: None,
        })]
    }
}

// ===========================================================================
// E047: SI-G companions — requires ORCON, forbids ORCON-USGOV
// ===========================================================================

/// Fires on any portion or banner carrying SI-G (GAMMA, with or without
/// sub-compartments) with missing ORCON or present ORCON-USGOV.
///
/// Authority:
/// - **CAPCO-2016 §H.4 p80** — SI-G: *"Requires SI and ORCON. May not
///   be used with ORCON-USGOV."*
/// - **CAPCO-2016 §H.4 p81** — SI-G sub-compartment: *"Requires SI, G,
///   and ORCON. May not be used with ORCON-USGOV."*
pub(crate) struct SiGammaCompanionsRule;

impl Rule for SiGammaCompanionsRule {
    fn id(&self) -> RuleId {
        RuleId::new("E047")
    }
    fn name(&self) -> &'static str {
        "si-gamma-companions"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let has_si_g = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Si) && has_compartment(m, "G"));
        if !has_si_g {
            return vec![];
        }
        let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc);
        let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

        let mut out = Vec::new();
        let form = infer_companion_form(attrs);
        let last_dissem = last_dissem_span(attrs);
        let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

        if !has_orcon {
            out.push(emit_companion_insert(
                RuleId::new("E047"),
                Severity::Warn,
                sci_span,
                last_dissem,
                form.orcon(),
                "SI-G requires ORCON (§H.4 p80)".to_owned(),
                "CAPCO-2016 §H.4 p80 — SI-G requires SI and ORCON",
            ));
        }
        if let Some((span, text)) = usgov_entry {
            out.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: RuleId::new("E047"),
                severity: Severity::Warn,
                source: FixSource::BuiltinRule,
                span,
                message: "SI-G forbids ORCON-USGOV (§H.4 p80) — replace with ORCON"
                    .to_owned(),
                citation: "CAPCO-2016 §H.4 p80 — SI-G may not be used with ORCON-USGOV",
                original: text.to_owned(),
                replacement: form.orcon().to_owned(),
                confidence: 0.9,
                migration_ref: None,
            }));
        }
        out
    }
}

// ===========================================================================
// E048: RSV classification ceiling (TS or S, warn only)
// ===========================================================================

/// Fires on any portion or banner carrying RSV (with or without
/// compartment) when the US classification is below SECRET. **No fix**
/// — ambiguous resolution.
///
/// Authority:
/// - **CAPCO-2016 §H.4 p70** — RSV: *"May only be used with TOP SECRET
///   or SECRET."*
/// - **CAPCO-2016 §H.4 p72** — RSV compartment: same ceiling.
pub(crate) struct RsvClassificationCeilingRule;

impl Rule for RsvClassificationCeilingRule {
    fn id(&self) -> RuleId {
        RuleId::new("E048")
    }
    fn name(&self) -> &'static str {
        "rsv-classification-ceiling"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let has_rsv = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Rsv));
        if !has_rsv {
            return vec![];
        }
        let Some(level) = us_level(attrs) else {
            return vec![];
        };
        if level >= Classification::Secret {
            return vec![];
        }
        let Some(class_tok) = classification_token(attrs) else {
            return vec![];
        };
        vec![Diagnostic::new(
            RuleId::new("E048"),
            Severity::Warn,
            class_tok.span,
            "RSV requires TOP SECRET or SECRET; resolve by upgrading the \
             classification or removing the RSV marking (§H.4 p70, p72)",
            "CAPCO-2016 §H.4 p70 (RSV) + p72 (RSV-[comp]) — May only be used with TOP SECRET or SECRET",
            None,
        )]
    }
}

// ===========================================================================
// E049: TK classification ceiling (TS or S, warn only)
// ===========================================================================

/// Fires on any portion or banner carrying TK (bare, compartmented, or
/// sub-compartmented) when the US classification is below SECRET.
/// **No fix** — ambiguous resolution. The TS-only compartment case
/// (TK-BLFH) is covered separately by E050 with a fix, and E050's
/// narrower TS-only ceiling pre-empts this rule for BLFH portions.
///
/// Authority: **CAPCO-2016 §H.4 p85** — TK: *"May only be used with
/// TOP SECRET or SECRET."*
pub(crate) struct TkClassificationCeilingRule;

impl Rule for TkClassificationCeilingRule {
    fn id(&self) -> RuleId {
        RuleId::new("E049")
    }
    fn name(&self) -> &'static str {
        "tk-classification-ceiling"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let has_tk = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Tk));
        if !has_tk {
            return vec![];
        }
        let Some(level) = us_level(attrs) else {
            return vec![];
        };
        if level >= Classification::Secret {
            return vec![];
        }
        let Some(class_tok) = classification_token(attrs) else {
            return vec![];
        };
        vec![Diagnostic::new(
            RuleId::new("E049"),
            Severity::Warn,
            class_tok.span,
            "TK requires TOP SECRET or SECRET; resolve by upgrading the \
             classification or removing the TK marking (§H.4 p85)",
            "CAPCO-2016 §H.4 p85 — TK: May only be used with TOP SECRET or SECRET",
            None,
        )]
    }
}

// ===========================================================================
// E050: TK-BLFH TS-only
// ===========================================================================

/// Fires on any portion or banner carrying TK-BLFH (with or without
/// sub-compartments) when the US classification is below TS. Fix:
/// upgrade classification to TS. NOFORN requirement is covered by E051.
///
/// Authority:
/// - **CAPCO-2016 §H.4 p87** — TK-BLFH: *"May only be used with TOP
///   SECRET. Requires TK. Requires NOFORN."*
/// - **CAPCO-2016 §H.4 p89** — TK-BLFH sub-compartment: inherits
///   TS-only + NOFORN from p87.
pub(crate) struct TkBlfhTopSecretRule;

impl Rule for TkBlfhTopSecretRule {
    fn id(&self) -> RuleId {
        RuleId::new("E050")
    }
    fn name(&self) -> &'static str {
        "tk-blfh-top-secret"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        let has_blfh = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Tk) && has_compartment(m, "BLFH"));
        if !has_blfh {
            return vec![];
        }
        let Some(level) = us_level(attrs) else {
            return vec![];
        };
        if level >= Classification::TopSecret {
            return vec![];
        }
        let Some((span, original, replacement)) =
            build_class_upgrade_fix(attrs, ctx, Classification::TopSecret)
        else {
            return vec![];
        };
        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: RuleId::new("E050"),
            severity: Severity::Warn,
            source: FixSource::BuiltinRule,
            span,
            message: format!(
                "TK-BLFH requires TOP SECRET; upgraded {original:?} → {replacement:?}. \
                 If this should be SECRET, remove the BLFH compartment (§H.4 p87)."
            ),
            citation: "CAPCO-2016 §H.4 p87 — TK-BLFH: May only be used with TOP SECRET",
            original,
            replacement,
            confidence: 0.9,
            migration_ref: None,
        })]
    }
}

// ===========================================================================
// E051: TK compartment NOFORN requirement (BLFH / IDIT / KAND and sub-comps)
// ===========================================================================

/// Fires on any portion or banner carrying TK-BLFH, TK-IDIT, or TK-KAND
/// (or any of their sub-compartments) when NOFORN is absent. Fix:
/// insert `/NF` (or `/NOFORN`) at the end of the IC dissem block;
/// Error no-fix when no dissem block exists.
///
/// Authority:
/// - **CAPCO-2016 §H.4 p87** — TK-BLFH: *"Requires NOFORN."* (inherited
///   by p89 sub-compartment)
/// - **CAPCO-2016 §H.4 p91** — TK-IDIT: *"Requires NOFORN."* (inherited
///   by p93 sub-compartment)
/// - **CAPCO-2016 §H.4 p95** — TK-KAND: *"Requires NOFORN."* (inherited
///   by p97 sub-compartment)
pub(crate) struct TkCompartmentRequiresNofornRule;

impl Rule for TkCompartmentRequiresNofornRule {
    fn id(&self) -> RuleId {
        RuleId::new("E051")
    }
    fn name(&self) -> &'static str {
        "tk-compartment-requires-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let applies = attrs.sci_markings.iter().any(is_tk_noforn_compartment);
        if !applies {
            return vec![];
        }
        let has_noforn = attrs.dissem_controls.contains(&DissemControl::Nf);
        if has_noforn {
            return vec![];
        }
        let form = infer_companion_form(attrs);
        let last_dissem = last_dissem_span(attrs);
        let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));
        vec![emit_companion_insert(
            RuleId::new("E051"),
            Severity::Warn,
            sci_span,
            last_dissem,
            form.noforn(),
            "TK-{BLFH|IDIT|KAND} require NOFORN (§H.4 p87, p91, p95)".to_owned(),
            "CAPCO-2016 §H.4 p87 (TK-BLFH) + p91 (TK-IDIT) + p95 (TK-KAND) — all require NOFORN",
        )]
    }
}

// ===========================================================================
// Shared: companion-insert emitter
// ===========================================================================

/// Emit a Warn diagnostic with a zero-width insertion fix appending
/// `/<token>` at the end of the existing IC dissem block. Falls back to
/// Error no-fix (per E040's policy) when no dissem block exists on this
/// portion/banner — inserting a whole category block is unsafe from
/// rule-level context.
///
/// `anchor_span` is the diagnostic pointer (typically the first SCI
/// token span for the offending marking), so the user sees WHICH SCI
/// triggered the requirement — independent of where the fix inserts.
fn emit_companion_insert(
    rule: RuleId,
    severity: Severity,
    anchor_span: Span,
    last_dissem: Option<Span>,
    token: &str,
    message: String,
    citation: &'static str,
) -> Diagnostic {
    match last_dissem {
        Some(dissem_span) => {
            // Zero-width insertion at end of the last dissem token.
            let insert_at = dissem_span.end;
            make_fix_diagnostic(FixDiagnosticParams {
                rule,
                severity,
                source: FixSource::BuiltinRule,
                span: Span::new(insert_at, insert_at),
                message,
                citation,
                original: String::new(),
                replacement: format!("/{token}"),
                confidence: 0.9,
                migration_ref: None,
            })
        }
        None => {
            // No dissem block at all — can't safely insert a whole new
            // category. Escalate to Error with no fix (same policy as
            // E040's no-IC-dissem case).
            Diagnostic::new(
                rule,
                Severity::Error,
                anchor_span,
                message,
                citation,
                None,
            )
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::rules::marque_capco_test_support::{lint_banner, lint_portion};

    // --- E042: HCS-O companions ---

    #[test]
    fn e042_fires_and_inserts_orcon_noforn_when_both_missing() {
        // (S//HCS-O) — HCS-O requires ORCON and NOFORN, both missing.
        let diags = lint_portion("(S//HCS-O)");
        let e042: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E042").collect();
        // Missing ORCON + Missing NOFORN = 2 diagnostics.
        // Note: no existing dissem block, so both fall to Error no-fix path.
        assert_eq!(
            e042.len(),
            2,
            "E042 must emit one diagnostic per missing companion: {diags:?}"
        );
        for d in &e042 {
            assert!(d.fix.is_none(), "no dissem block → no-fix Error: {d:?}");
            assert_eq!(d.severity, Severity::Error);
        }
    }

    #[test]
    fn e042_inserts_into_existing_dissem_block_when_some_companions_present() {
        // (S//HCS-O//NF) — NOFORN present, ORCON missing. Fix: insert /OC.
        let diags = lint_portion("(S//HCS-O//NF)");
        let e042: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E042").collect();
        assert_eq!(e042.len(), 1);
        let fix = e042[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "/OC");
    }

    #[test]
    fn e042_fires_on_forbidden_orcon_usgov() {
        // (S//HCS-O//OC-USGOV/NF) — ORCON present via OC-USGOV; rule fires.
        // Note: this also misses bare ORCON, so 2 diagnostics: missing
        // ORCON + forbidden OC-USGOV.
        let diags = lint_portion("(S//HCS-O//OC-USGOV/NF)");
        let e042: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E042").collect();
        assert!(
            e042.iter()
                .any(|d| d.message.contains("forbids ORCON-USGOV")),
            "expected ORCON-USGOV forbidden diagnostic: {diags:?}"
        );
    }

    #[test]
    fn e042_does_not_fire_when_all_companions_correct() {
        let diags = lint_portion("(S//HCS-O//OC/NF)");
        let e042: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E042").collect();
        assert!(e042.is_empty(), "compliant HCS-O: {diags:?}");
    }

    // --- E043: HCS-P requires NOFORN ---

    #[test]
    fn e043_fires_when_noforn_missing() {
        let diags = lint_portion("(S//HCS-P//OC)");
        let e043: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E043").collect();
        assert_eq!(e043.len(), 1);
        let fix = e043[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "/NF");
    }

    #[test]
    fn e043_does_not_fire_with_noforn_present() {
        let diags = lint_portion("(S//HCS-P//OC/NF)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E043"));
    }

    #[test]
    fn e043_does_not_fire_on_bare_hcs_or_hcs_o() {
        // Only HCS-P triggers this rule.
        let d1 = lint_portion("(S//HCS)");
        assert!(d1.iter().all(|d| d.rule.as_str() != "E043"));
        let d2 = lint_portion("(S//HCS-O//OC/NF)");
        assert!(d2.iter().all(|d| d.rule.as_str() != "E043"));
    }

    // --- E044: HCS-P sub-compartment TS-only ---

    #[test]
    fn e044_fires_and_upgrades_class_on_hcs_p_sub() {
        // (S//HCS-P JJJ//OC/NF) — HCS-P with sub-compartment JJJ, should be TS.
        let diags = lint_portion("(S//HCS-P JJJ//OC/NF)");
        let e044: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E044").collect();
        assert_eq!(e044.len(), 1);
        let fix = e044[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "TS");
        assert!(e044[0].message.contains("TOP SECRET"));
    }

    #[test]
    fn e044_does_not_fire_on_bare_hcs_p() {
        // HCS-P without sub-compartment is not TS-only.
        let diags = lint_portion("(S//HCS-P//NF)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E044"));
    }

    #[test]
    fn e044_does_not_fire_at_top_secret() {
        let diags = lint_portion("(TS//HCS-P JJJ//OC/NF)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E044"));
    }

    // --- E045: HCS classification ceiling (warn only) ---

    #[test]
    fn e045_fires_warn_only_below_secret() {
        // (C//HCS-O//OC/NF) — HCS-O on CONFIDENTIAL. Warn, no fix.
        let diags = lint_portion("(C//HCS-O//OC/NF)");
        let e045: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E045").collect();
        assert_eq!(e045.len(), 1);
        assert_eq!(e045[0].severity, Severity::Warn);
        assert!(
            e045[0].fix.is_none(),
            "range-ceiling rules emit no fix (ambiguous)"
        );
    }

    #[test]
    fn e045_does_not_fire_at_secret_or_top_secret() {
        assert!(
            lint_portion("(S//HCS-O//OC/NF)")
                .iter()
                .all(|d| d.rule.as_str() != "E045")
        );
        assert!(
            lint_portion("(TS//HCS-O//OC/NF)")
                .iter()
                .all(|d| d.rule.as_str() != "E045")
        );
    }

    // --- E046: SI compartment TS-only ---

    #[test]
    fn e046_fires_on_si_nongamma_compartment_below_ts() {
        let diags = lint_portion("(S//SI-ABC)");
        let e046: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E046").collect();
        assert_eq!(e046.len(), 1);
        let fix = e046[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "TS");
    }

    #[test]
    fn e046_fires_on_si_gamma_below_ts() {
        let diags = lint_portion("(S//SI-G//OC)");
        let e046: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E046").collect();
        assert_eq!(e046.len(), 1);
    }

    #[test]
    fn e046_does_not_fire_on_bare_si() {
        let diags = lint_portion("(S//SI)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E046"));
    }

    // --- E047: SI-G companions ---

    #[test]
    fn e047_fires_on_missing_orcon() {
        let diags = lint_portion("(TS//SI-G)");
        let e047: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E047").collect();
        assert_eq!(e047.len(), 1);
        // No dissem block, so no-fix Error (same policy as E040).
        assert!(e047[0].fix.is_none());
        assert_eq!(e047[0].severity, Severity::Error);
    }

    #[test]
    fn e047_inserts_orcon_when_dissem_block_present() {
        let diags = lint_portion("(TS//SI-G//NF)");
        let e047: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E047").collect();
        assert_eq!(e047.len(), 1);
        let fix = e047[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "/OC");
    }

    #[test]
    fn e047_fires_on_forbidden_oc_usgov() {
        let diags = lint_portion("(TS//SI-G//OC-USGOV)");
        let e047: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E047").collect();
        assert!(
            e047.iter()
                .any(|d| d.message.contains("forbids ORCON-USGOV")),
            "expected forbid diagnostic: {e047:?}"
        );
    }

    #[test]
    fn e047_does_not_fire_when_compliant() {
        let diags = lint_portion("(TS//SI-G//OC)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E047"));
    }

    // --- E048: RSV ceiling ---

    #[test]
    fn e048_fires_warn_only_below_secret() {
        let diags = lint_portion("(C//RSV-ABC)");
        let e048: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E048").collect();
        assert_eq!(e048.len(), 1);
        assert!(e048[0].fix.is_none());
    }

    #[test]
    fn e048_does_not_fire_at_secret() {
        let diags = lint_portion("(S//RSV-ABC)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E048"));
    }

    // --- E049: TK ceiling ---

    #[test]
    fn e049_fires_warn_only_below_secret() {
        let diags = lint_portion("(C//TK)");
        let e049: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E049").collect();
        assert_eq!(e049.len(), 1);
        assert!(e049[0].fix.is_none());
    }

    #[test]
    fn e049_does_not_fire_at_secret_or_above() {
        assert!(lint_portion("(S//TK)").iter().all(|d| d.rule.as_str() != "E049"));
        assert!(lint_portion("(TS//TK)").iter().all(|d| d.rule.as_str() != "E049"));
    }

    // --- E050: TK-BLFH TS-only ---

    #[test]
    fn e050_fires_and_upgrades_class_on_tk_blfh_below_ts() {
        let diags = lint_portion("(S//TK-BLFH//NF)");
        let e050: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E050").collect();
        assert_eq!(e050.len(), 1);
        let fix = e050[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "TS");
        assert!(e050[0].message.contains("BLFH"));
    }

    #[test]
    fn e050_does_not_fire_on_tk_idit_at_secret() {
        // TK-IDIT allows SECRET; E049 covers its ceiling, not E050.
        let diags = lint_portion("(S//TK-IDIT//NF)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E050"));
    }

    // --- E051: TK compartment NOFORN requirement ---

    #[test]
    fn e051_fires_on_blfh_without_noforn() {
        let diags = lint_portion("(TS//TK-BLFH//OC)");
        let e051: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E051").collect();
        assert_eq!(e051.len(), 1);
        let fix = e051[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "/NF");
    }

    #[test]
    fn e051_fires_on_idit_without_noforn() {
        let diags = lint_portion("(TS//TK-IDIT//OC)");
        let e051: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E051").collect();
        assert_eq!(e051.len(), 1);
    }

    #[test]
    fn e051_fires_on_kand_without_noforn() {
        let diags = lint_portion("(TS//TK-KAND//OC)");
        let e051: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E051").collect();
        assert_eq!(e051.len(), 1);
    }

    #[test]
    fn e051_does_not_fire_on_bare_tk() {
        // Bare TK does not require NOFORN (§H.4 p85 — only the compartments do).
        let diags = lint_portion("(TS//TK)");
        assert!(diags.iter().all(|d| d.rule.as_str() != "E051"));
    }

    #[test]
    fn e051_does_not_fire_when_noforn_present() {
        assert!(
            lint_portion("(TS//TK-BLFH//NF)")
                .iter()
                .all(|d| d.rule.as_str() != "E051")
        );
    }

    // --- Banner forms ---

    #[test]
    fn rules_use_full_form_in_banner_when_dissem_is_full() {
        // Banner form: "TOP SECRET//TK-BLFH" should upgrade to TOP SECRET,
        // and missing NOFORN would insert /NOFORN (full form).
        let diags = lint_banner("SECRET//TK-BLFH//ORCON");
        let e050: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E050").collect();
        assert_eq!(e050.len(), 1);
        let fix = e050[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "TOP SECRET");

        let e051: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E051").collect();
        assert_eq!(e051.len(), 1);
        let fix51 = e051[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix51.replacement.as_ref(), "/NOFORN");
    }
}
