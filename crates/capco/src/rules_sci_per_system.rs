// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T035d: Per-SCI-system constraint rules (E042–E051).
//!
//! CAPCO-2016 §H.4 spells out per-system constraints on classification
//! level, required companion dissem controls, and forbidden companions
//! for every SCI control system. This module implements those as ten
//! rules with `Warn`-severity defaults that use the **fix-and-warn**
//! pattern: when a violation has an unambiguous resolution, we attach a
//! fix AND surface the warning, so the user sees exactly what was
//! corrected and can override if the intent was actually different.
//!
//! **Severity escalation.** The companion-insertion rules (E042, E043,
//! E044, E047, E051) escalate to `Severity::Error` with **no fix** when
//! the portion or banner has no IC dissem block at all. Inserting a
//! whole new `//`-separated dissem category from rule-level context is
//! unsafe (there is no known anchor for the `//`); the engine surfaces
//! the violation so the user can add the missing block by hand. Same
//! policy as E040.
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
use marque_rules::{
    Confidence, Diagnostic, FixProposal, FixSource, Rule, RuleContext, RuleId, Severity,
};

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
    m.compartments.iter().any(|c| c.identifier.as_ref() == id)
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
        // §H.4 per-SCI-system constraints are scoped to US
        // classifications. SCI-on-foreign (pure FGI/NATO/JOINT) is out
        // of scope; inserting NOFORN on a JOINT marking would in fact
        // violate §H.8 (JOINT forbids NOFORN). Skip when the observed
        // level is not US-or-Conflict.
        if us_level(attrs).is_none() {
            return vec![];
        }
        // Does any marking on this portion carry HCS-O (bare-HCS
        // anchor + "O" compartment)?
        let has_hcs_o = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "O"));
        if !has_hcs_o {
            return vec![];
        }

        // ORCON is considered "present" if either bare `Oc` or
        // `OcUsgov` appears: when OcUsgov is present we emit the
        // forbidden-OC-USGOV → OC replacement below, and the post-fix
        // state satisfies the ORCON-required constraint. Without this
        // check the rule would emit both an insertion AND the
        // replacement, producing a duplicate ORCON token when both
        // fixes apply.
        let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc)
            || attrs.dissem_controls.contains(&DissemControl::OcUsgov);
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
                message: "HCS-O forbids ORCON-USGOV (§H.4 p64) — replace with ORCON".to_owned(),
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
        // Scope guard: §H.4 applies only to US classifications. Skip
        // pure FGI/NATO/JOINT where a NOFORN insertion would be wrong
        // (JOINT in particular forbids NOFORN per §H.8).
        if us_level(attrs).is_none() {
            return vec![];
        }
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
/// sub-compartment. Enforces the full §H.4 p68 constraint set:
///
/// - classification must be TOP SECRET (fix: upgrade the class token
///   when the observed level is below TS — the sub-compartment is
///   treated as the stronger intent signal; see the module doc on the
///   fix-and-warn rationale);
/// - ORCON must be present (fix: insert `/OC` or `/ORCON`);
/// - ORCON-USGOV must be absent (fix: replace with `OC`/`ORCON`).
///
/// NOFORN is enforced by E043 (which fires on any HCS-P including
/// sub-compartments), so it is not duplicated here.
///
/// Authority: **CAPCO-2016 §H.4 p68** — HCS-P sub-compartment
/// *Relationship(s) to Other Markings*: "May only be used with TOP
/// SECRET. Requires HCS-P, ORCON, and NOFORN. May not be used with
/// ORCON-USGOV."
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
        // Scope guard: §H.4 applies only to US classifications. Pure
        // FGI/NATO/JOINT with HCS-P sub is out of scope — both the
        // class-upgrade path and the ORCON / no-ORCON-USGOV companion
        // paths must be skipped.
        let Some(level) = us_level(attrs) else {
            return vec![];
        };
        let has_hcs_p_sub = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Hcs) && compartment_has_sub(m, "P"));
        if !has_hcs_p_sub {
            return vec![];
        }

        let mut out = Vec::new();

        // (1) Classification must be TOP SECRET.
        if level < Classification::TopSecret {
            if let Some((span, original, replacement)) =
                build_class_upgrade_fix(attrs, ctx, Classification::TopSecret)
            {
                out.push(make_fix_diagnostic(FixDiagnosticParams {
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
                }));
            }
        }

        // (2) ORCON required. Treat either bare `Oc` or `OcUsgov` as
        // satisfying the presence check; OcUsgov is rewritten to Oc by
        // the forbidden-OC-USGOV fix below, so the post-fix state is
        // compliant. Without this check we'd emit both an insertion
        // AND the replacement, producing a duplicate ORCON token.
        let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc)
            || attrs.dissem_controls.contains(&DissemControl::OcUsgov);
        let form = infer_companion_form(attrs);
        let last_dissem = last_dissem_span(attrs);
        let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

        if !has_orcon {
            out.push(emit_companion_insert(
                RuleId::new("E044"),
                Severity::Warn,
                sci_span,
                last_dissem,
                form.orcon(),
                "HCS-P sub-compartment requires ORCON (§H.4 p68)".to_owned(),
                "CAPCO-2016 §H.4 p68 — HCS-P sub-compartment requires ORCON",
            ));
        }

        // (3) ORCON-USGOV forbidden → replace with bare ORCON.
        if let Some((span, text)) = dissem_token_span(attrs, DissemControl::OcUsgov) {
            out.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: RuleId::new("E044"),
                severity: Severity::Warn,
                source: FixSource::BuiltinRule,
                span,
                message:
                    "HCS-P sub-compartment forbids ORCON-USGOV (§H.4 p68) — replace with ORCON"
                        .to_owned(),
                citation:
                    "CAPCO-2016 §H.4 p68 — HCS-P sub-compartment may not be used with ORCON-USGOV",
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
// E045: HCS classification ceiling (TS or S, warn only — ambiguous)
// ===========================================================================

/// Fires on any portion or banner carrying HCS-O or bare HCS-P when the
/// US classification is below SECRET (i.e., CONFIDENTIAL or
/// UNCLASSIFIED). **No fix** — the resolution is ambiguous (upgrade to
/// S? to TS? remove the HCS marking?); the user decides.
///
/// Authority: **CAPCO-2016 §H.4 p64** (HCS-O) and **p66** (HCS-P) —
/// both constrain *"May only be used with TOP SECRET or SECRET."*
///
/// HCS-P with sub-compartments is TS-only and handled by E044's
/// unambiguous upgrade fix; this rule explicitly skips that case to
/// avoid firing a redundant no-fix Warn on top of E044's actionable
/// upgrade.
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
        // Pre-empt HCS-P-with-sub-compartment: E044 will emit the
        // unambiguous TS-only upgrade. Firing E045 here too would be
        // redundant no-fix noise on top of an actionable fix.
        let has_hcs_p_sub = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Hcs) && compartment_has_sub(m, "P"));
        if has_hcs_p_sub {
            return vec![];
        }
        // Cover HCS-O and bare HCS-P. Bare HCS is legacy — E006 / E008
        // already surface it; no need to double-diagnose here.
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
        // Scope guard: §H.4 applies only to US classifications. SI-G
        // on pure FGI/NATO/JOINT is out of scope.
        if us_level(attrs).is_none() {
            return vec![];
        }
        let has_si_g = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Si) && has_compartment(m, "G"));
        if !has_si_g {
            return vec![];
        }
        // Treat `OcUsgov` as satisfying ORCON presence: the
        // forbidden-OC-USGOV → OC replacement below makes the post-fix
        // state compliant. Without this check we'd emit both an
        // insertion AND the replacement, producing a duplicate ORCON.
        let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc)
            || attrs.dissem_controls.contains(&DissemControl::OcUsgov);
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
                message: "SI-G forbids ORCON-USGOV (§H.4 p80) — replace with ORCON".to_owned(),
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

/// Fires on any portion or banner carrying TK (bare or compartmented,
/// excluding TK-BLFH which E050 handles) when the US classification is
/// below SECRET. **No fix** — ambiguous resolution. The TS-only
/// compartment case (TK-BLFH) is covered by E050 with an actionable
/// upgrade fix; firing both E049 and E050 on the same below-TS TK-BLFH
/// portion would be redundant no-fix noise on top of E050's fix, so
/// this rule explicitly skips any TK marking whose compartment set
/// includes BLFH.
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
        // Pre-empt TK-BLFH: E050 emits the actionable TS-only upgrade.
        // A TK marking with BLFH at below-SECRET is by construction also
        // below TS, so E050 covers the actual violation; E049 would
        // just add no-fix noise.
        let has_tk_non_blfh = attrs
            .sci_markings
            .iter()
            .any(|m| anchors_on(m, SciControlBare::Tk) && !has_compartment(m, "BLFH"));
        if !has_tk_non_blfh {
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
        // Scope guard: §H.4 applies only to US classifications. Pure
        // FGI/NATO/JOINT is out of scope — inserting NOFORN on JOINT
        // would violate §H.8 (JOINT forbids NOFORN).
        if us_level(attrs).is_none() {
            return vec![];
        }
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

/// Emit a diagnostic pointing at `anchor_span` (the offending SCI
/// token) with a zero-width insertion fix appending `/<token>` at the
/// end of the existing IC dissem block. The diagnostic caret and the
/// fix span intentionally differ: the user sees the SCI marking that
/// triggered the companion requirement, while the edit applies at the
/// dissem block where the insertion belongs. This follows the
/// diagnostic-vs-fix-span split used by `SarPortionFormRule` (E026).
///
/// Falls back to Error no-fix (per E040's policy) when no dissem block
/// exists on this portion/banner — inserting a whole `//`-separated
/// category from rule context is unsafe (there is no known anchor for
/// the `//`).
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
            // Zero-width insertion at end of the last dissem token;
            // diagnostic caret stays on the SCI marking that triggered
            // the requirement so the user sees the *cause*, not just
            // the patch site.
            let insert_at = dissem_span.end;
            let fix = FixProposal::new(
                rule.clone(),
                FixSource::BuiltinRule,
                Span::new(insert_at, insert_at),
                String::new(),
                format!("/{token}"),
                Confidence::strict(0.9),
                None,
            );
            Diagnostic::new(rule, severity, anchor_span, message, citation, Some(fix))
        }
        None => {
            // No dissem block at all — can't safely insert a whole new
            // category. Escalate to Error with no fix (same policy as
            // E040's no-IC-dissem case).
            Diagnostic::new(rule, Severity::Error, anchor_span, message, citation, None)
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
    fn e042_no_dissem_block_escalates_to_error_no_fix() {
        // (S//HCS-O) — HCS-O requires ORCON and NOFORN, both missing.
        // The portion has no IC dissem block, so `emit_companion_insert`
        // cannot safely synthesize a whole `//`-separated category.
        // Both companion checks fall back to the `None` arm: Severity
        // escalates to Error and no fix is attached (same policy as
        // E040's no-IC-dissem case).
        let diags = lint_portion("(S//HCS-O)");
        let e042: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E042").collect();
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
    fn e042_companion_insert_diagnostic_points_at_sci_not_dissem() {
        // (S//HCS-O//NF) — NOFORN present, ORCON missing. The fix is a
        // zero-width insertion at the end of `NF`; the diagnostic caret
        // must point at the HCS-O SCI token so the user sees WHICH
        // marking triggered the requirement. Diagnostic span ≠ fix span.
        let src = "(S//HCS-O//NF)";
        let diags = lint_portion(src);
        let e042: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E042").collect();
        assert_eq!(e042.len(), 1, "only missing-ORCON diagnostic: {e042:?}");
        let diag = e042[0];
        let fix = diag.fix.as_ref().expect("fix attached");

        // Fix span is the zero-width insertion at the end of `NF`.
        assert_eq!(
            fix.span.start, fix.span.end,
            "insertion fix must be zero-width: {fix:?}"
        );
        let inserted_at = &src.as_bytes()[..fix.span.start];
        assert!(
            inserted_at.ends_with(b"NF"),
            "fix must apply at end of the NF dissem token: inserted_at_byte_prefix={}",
            String::from_utf8_lossy(inserted_at)
        );

        // Diagnostic caret is on the HCS-O SCI token, not on the fix
        // insertion point. The two spans must differ.
        assert_ne!(
            diag.span, fix.span,
            "diagnostic caret and fix span must differ: {diag:?}"
        );
        let caret = &src.as_bytes()[diag.span.start..diag.span.end];
        assert_eq!(
            caret, b"HCS-O",
            "diagnostic caret must point at the HCS-O compound SCI token"
        );
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
    fn e042_oc_usgov_emits_only_replacement_not_duplicate_orcon() {
        // (S//HCS-O//OC-USGOV/NF) — OC-USGOV present, bare OC absent.
        // `has_orcon` must treat OcUsgov as satisfying the presence
        // check: the forbidden-OC-USGOV fix rewrites OcUsgov → OC, and
        // the post-fix state has ORCON. Without the OcUsgov check, the
        // rule would ALSO emit a missing-ORCON insertion, and both
        // fixes applied together would produce a duplicate ORCON.
        let diags = lint_portion("(S//HCS-O//OC-USGOV/NF)");
        let e042: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E042").collect();
        assert_eq!(
            e042.len(),
            1,
            "only the forbidden-OC-USGOV replacement should fire; OcUsgov \
             satisfies ORCON presence post-fix: {e042:?}"
        );
        let only = e042[0];
        assert!(
            only.message.contains("forbids ORCON-USGOV"),
            "sole diag must be the forbid diagnostic: {only:?}"
        );
        let fix = only.fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "OC");
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

    #[test]
    fn e044_inserts_orcon_when_hcs_p_sub_lacks_orcon() {
        // (TS//HCS-P JJJ//NF) — HCS-P sub-compartment on TS with NF but
        // no ORCON. §H.4 p68 requires ORCON; E044 must emit the
        // insertion (OC suffix) even when classification is already TS.
        let diags = lint_portion("(TS//HCS-P JJJ//NF)");
        let e044: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E044").collect();
        assert_eq!(e044.len(), 1, "expected ORCON-missing diag: {e044:?}");
        assert!(
            e044[0].message.contains("requires ORCON"),
            "message must cite the ORCON requirement: {:?}",
            e044[0].message
        );
        let fix = e044[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "/OC");
    }

    #[test]
    fn e044_replaces_oc_usgov_on_hcs_p_sub() {
        // (TS//HCS-P JJJ//OC-USGOV/NF) — already at TS, so no upgrade;
        // OC-USGOV is forbidden on HCS-P sub (§H.4 p68), replacement
        // fix should convert it to bare ORCON.
        let diags = lint_portion("(TS//HCS-P JJJ//OC-USGOV/NF)");
        let e044: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E044").collect();
        assert_eq!(e044.len(), 1);
        assert!(
            e044[0].message.contains("forbids ORCON-USGOV"),
            "message must cite the forbidden constraint: {:?}",
            e044[0].message
        );
        let fix = e044[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "OC");
    }

    #[test]
    fn e044_oc_usgov_without_bare_oc_does_not_duplicate_orcon_insertion() {
        // (TS//HCS-P JJJ//OC-USGOV) — only OC-USGOV present, no bare OC
        // and no NF. `has_orcon` must treat OcUsgov as satisfying
        // ORCON presence; otherwise we'd emit both an insertion AND a
        // replacement, producing duplicate ORCON. E043 covers the
        // missing NOFORN separately and is not counted here.
        let diags = lint_portion("(TS//HCS-P JJJ//OC-USGOV)");
        let e044: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E044").collect();
        assert_eq!(
            e044.len(),
            1,
            "only the OC-USGOV replacement should fire: {e044:?}"
        );
        assert!(e044[0].message.contains("forbids ORCON-USGOV"));
    }

    #[test]
    fn e044_fires_class_upgrade_and_companion_checks_independently() {
        // (S//HCS-P JJJ//OC-USGOV) — below TS and OC-USGOV present,
        // NOFORN missing. Expect E044 to emit both the class upgrade
        // AND the OC-USGOV replacement (two distinct diagnostics).
        let diags = lint_portion("(S//HCS-P JJJ//OC-USGOV)");
        let e044: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E044").collect();
        assert_eq!(e044.len(), 2, "expected upgrade + replacement: {e044:?}");
        let class_upgrade = e044
            .iter()
            .find(|d| d.message.contains("TOP SECRET"))
            .expect("upgrade diag");
        assert_eq!(
            class_upgrade.fix.as_ref().unwrap().replacement.as_ref(),
            "TS"
        );
        let forbid = e044
            .iter()
            .find(|d| d.message.contains("forbids ORCON-USGOV"))
            .expect("forbid diag");
        assert_eq!(forbid.fix.as_ref().unwrap().replacement.as_ref(), "OC");
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

    #[test]
    fn e045_suppressed_when_hcs_p_has_subcompartment() {
        // (C//HCS-P JJJ//OC/NF) — HCS-P with a sub-compartment on
        // CONFIDENTIAL. E044 emits the unambiguous TS-only upgrade;
        // E045's no-fix Warn would be redundant noise on top, so it
        // MUST be suppressed for this case.
        let diags = lint_portion("(C//HCS-P JJJ//OC/NF)");
        let e044: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E044").collect();
        assert!(
            !e044.is_empty(),
            "E044 should fire on below-TS HCS-P sub: {diags:?}"
        );
        let e045: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E045").collect();
        assert!(
            e045.is_empty(),
            "E045 must be pre-empted by E044 when HCS-P carries a sub-compartment: {e045:?}"
        );
    }

    #[test]
    fn e045_still_fires_on_bare_hcs_p_below_secret() {
        // (C//HCS-P//NF) — bare HCS-P (no sub-comp). E044 does NOT apply
        // here, so E045's range-ceiling diagnostic should still fire.
        let diags = lint_portion("(C//HCS-P//NF)");
        let e045: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E045").collect();
        assert_eq!(e045.len(), 1);
        assert!(e045[0].fix.is_none());
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
    fn e047_oc_usgov_emits_only_replacement_not_duplicate_orcon() {
        // (TS//SI-G//OC-USGOV) — OC-USGOV present without bare OC.
        // `has_orcon` must treat OcUsgov as satisfying ORCON presence
        // so the rule emits only the replacement (OcUsgov → OC); the
        // post-fix state is compliant. Without this check, the rule
        // would also emit the missing-ORCON insertion and both fixes
        // applied together would produce a duplicate ORCON token.
        let diags = lint_portion("(TS//SI-G//OC-USGOV)");
        let e047: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E047").collect();
        assert_eq!(
            e047.len(),
            1,
            "only the replacement should fire; OcUsgov satisfies ORCON \
             presence post-fix: {e047:?}"
        );
        assert!(
            e047[0].message.contains("forbids ORCON-USGOV"),
            "sole diag must be the forbid diagnostic: {:?}",
            e047[0].message
        );
        let fix = e047[0].fix.as_ref().expect("fix attached");
        assert_eq!(fix.replacement.as_ref(), "OC");
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
        assert!(
            lint_portion("(S//TK)")
                .iter()
                .all(|d| d.rule.as_str() != "E049")
        );
        assert!(
            lint_portion("(TS//TK)")
                .iter()
                .all(|d| d.rule.as_str() != "E049")
        );
    }

    #[test]
    fn e049_suppressed_when_tk_has_blfh() {
        // (C//TK-BLFH//NF) — TK with BLFH compartment on CONFIDENTIAL.
        // E050 emits the actionable TS-only upgrade for TK-BLFH; E049
        // would just add a no-fix range-ceiling warn on top, so it
        // MUST be suppressed for any TK marking whose compartments
        // include BLFH.
        let diags = lint_portion("(C//TK-BLFH//NF)");
        let e050: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E050").collect();
        assert!(
            !e050.is_empty(),
            "E050 should fire on below-TS TK-BLFH: {diags:?}"
        );
        let e049: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E049").collect();
        assert!(
            e049.is_empty(),
            "E049 must be pre-empted by E050 when TK carries BLFH: {e049:?}"
        );
    }

    #[test]
    fn e049_still_fires_on_tk_idit_below_secret() {
        // (C//TK-IDIT//NF) — TK-IDIT is NOT TS-only (§H.4 p91 allows
        // S/TS), so E050's BLFH-specific rule does NOT apply. E049's
        // ambiguous range-ceiling diagnostic must still fire.
        let diags = lint_portion("(C//TK-IDIT//NF)");
        let e049: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "E049").collect();
        assert_eq!(e049.len(), 1);
        assert!(e049[0].fix.is_none());
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

    // --- Scope: SCI-on-foreign (pure FGI / NATO / JOINT) is out of §H.4 ---
    //
    // §H.4 per-SCI-system constraints are scoped to US classifications.
    // `us_level()` returns `None` for pure foreign classifications, and
    // every rule in this module must short-circuit in that case;
    // otherwise e.g. E042 would try to insert NOFORN on a JOINT banner,
    // which §H.8 explicitly forbids.

    #[test]
    fn e042_e043_e044_e047_e051_skip_joint_classifications() {
        // JOINT banner carrying every SCI marking that has a companion
        // rule in this module. On `//JOINT ... S ...` the classification
        // is `MarkingClassification::Joint(_)`, so `us_level` returns
        // `None` and every companion rule must skip.
        let src = "//JOINT S USA GBR//HCS-O HCS-P JJJ//SI-G//TK-BLFH//REL TO USA, GBR";
        let diags = lint_banner(src);
        for rule in ["E042", "E043", "E044", "E047", "E051"] {
            let hits: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == rule).collect();
            assert!(
                hits.is_empty(),
                "{rule} must not fire on a JOINT (non-US) classification; \
                 §H.4 companion constraints are scoped to US markings: \
                 hits={hits:?}"
            );
        }
    }

    #[test]
    fn e042_and_e043_still_fire_on_us_classifications() {
        // Sanity: the scope guard must not mask genuine US-side
        // violations. A US-classified portion with HCS-O and HCS-P but
        // no ORCON/NOFORN still fires E042 and E043.
        let diags = lint_portion("(S//HCS-O HCS-P)");
        assert!(
            diags.iter().any(|d| d.rule.as_str() == "E042"),
            "E042 must fire on US-classified HCS-O without companions: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.rule.as_str() == "E043"),
            "E043 must fire on US-classified HCS-P without NOFORN: {diags:?}"
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
