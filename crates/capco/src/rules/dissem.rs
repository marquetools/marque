// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Dissemination-control rules.
//!
//! - [`DeprecatedDissemRule`] — walks the MIGRATIONS table for
//!   deprecated dissem entries.
//! - [`NonIcInClassifiedBannerRule`] — flags SBU / LIMDIS in
//!   classified banners.
//!
//! Predicate IDs live on each rule's `RuleId::new(...)` — the wire
//! string is the single source of truth.

use marque_ism::generated::migrations::find_migration;
use marque_ism::{CanonicalAttrs, Span, TokenKind, TokenSpan};
use marque_rules::{
    Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase, Rule, RuleContext, RuleId,
    Severity,
};
use marque_scheme::{Citation, SectionLetter, capco, capco_section};

use super::helpers::{FixDiagnosticParams, make_fix_diagnostic};
use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// E006 — Deprecated dissem control
// ---------------------------------------------------------------------------

/// Fires when a marking contains a deprecated dissemination control.
///
/// Most deprecated dissem controls (e.g., `LIMDIS`, `FOUO`) are absent from
/// the modern CVE entirely, so the parser surfaces them as `Unknown` tokens.
/// E006 walks Unknown tokens and looks each up in the migration table; a
/// hit whose replacement is a known dissem control fires the diagnostic.
///
/// Entries owned by the form-mismatch rules (banner abbreviation,
/// e.g., `NF`→`NOFORN`) are handled by those rules instead, so the
/// duplicate dispatch is suppressed via the `is_dissem_replacement`
/// filter below.
pub(super) struct DeprecatedDissemRule;

/// Citations E006 may emit on diagnostics. §F has no numbered
/// subsections; the bare-section page anchor at p35 marks the
/// start of the §F "Legacy Control Markings" passage. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const DEPRECATED_DISSEM_AUTHORITIES: &[Citation] = &[capco_section(SectionLetter::F, 35)];

impl Rule<CapcoScheme> for DeprecatedDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.deprecation.deprecated-dissem-control")
    }
    fn name(&self) -> &'static str {
        "deprecated-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::Localized: each fix rewrites a single `DissemControl` /
    /// `Unknown` token in place via the migration table (e.g.
    /// `LIMDIS → LIMITED DISTRIBUTION`). Span is strictly the one
    /// `TokenSpan` the rule walked.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        DEPRECATED_DISSEM_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut diagnostics = Vec::new();
        // Walk every TokenSpan whose kind is either DissemControl (the
        // deprecated marking is in the modern CVE — e.g., FOUO) or Unknown
        // (the deprecated marking has been removed from the CVE — e.g.,
        // LIMDIS). For each, look up the migration table by text. A hit
        // whose replacement is a known dissem name is an E006 violation.
        for token in attrs.token_spans.iter() {
            if !matches!(token.kind, TokenKind::DissemControl | TokenKind::Unknown) {
                continue;
            }
            let Some(entry) = find_migration(token.text.as_ref()) else {
                continue;
            };
            // Skip declass-shorthand entries (E007 owns those).
            if !is_dissem_replacement(entry.replacement) {
                continue;
            }
            // Form-pair ownership (NF/OC/IMC/DSEN/PR ↔ NOFORN/ORCON/IMCON/
            // DEA SENSITIVE/PROPIN): owned by
            // `capco:banner.metadata.uses-portion-form` +
            // `capco:portion.metadata.uses-banner-form` per #677. The
            // historical `is_abbreviation_expansion` guard here was
            // dead-code-by-construction: the `MIGRATIONS` table in
            // `marque_ism::generated::migrations` carries only declass
            // shorthand entries today (`25X1-` / `50X1-` X-shorthand
            // patterns per CAPCO-2016 §E.6 p34), and the
            // `is_dissem_replacement` filter above rejects every one of
            // them BEFORE reaching this point. No form-pair entry has
            // existed in `MIGRATIONS` since T035c-4 (legacy IDs E001 /
            // E009, now migrated to the wire strings cited above).
            // Tracked as a follow-up: the `MIGRATIONS` doc-comment in
            // `crates/ism/build.rs` still references the legacy E001 /
            // E009 rule IDs and the removed `is_abbreviation_expansion`
            // guard; updating that doc is engine-crate territory under
            // Constitution VII §IV and cannot land in this CAPCO PR.
            // Constitution V Principle V (G13): the original document
            // bytes (`token.text`) and the canonical replacement
            // (`entry.replacement`) do NOT flow into the typed
            // `Message`. The replacement is on the permitted-identifier
            // list (token canonical from a closed vocabulary), but
            // `MessageArgs.expected_token` carries a `TokenId`, not a
            // raw `&str` — and we do not have a guaranteed `TokenId`
            // projection for every deprecation-table entry. The
            // bytes ARE still carried by `Diagnostic.text_correction.replacement`
            // (the canonical replacement is on the permitted list).
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::MigrationTable,
                span: token.span,
                message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                // §F covers all legacy Control Marking deprecations
                // (E006 dissem migration table). §F has no numbered
                // subsections in CAPCO-2016 (the citation-index
                // confirms `section: F` carries no `subsections:`
                // list); use the bare-section helper with page 35
                // (start of §F per citation-index).
                citation: capco_section(SectionLetter::F, 35),
                original: token.text.to_string(),
                replacement: entry.replacement.to_owned(),
                confidence: entry.confidence,
                migration_ref: Some(entry.reference),
            }));
        }
        diagnostics
    }
}

/// Returns `true` if `replacement` is one of the dissemination-control
/// replacements that E006 is allowed to claim from MIGRATIONS.
///
/// This is intentionally a narrow allowlist, not a general "is this a
/// current CAPCO dissem control?" predicate. E006 uses it as a guard
/// because the migration table can also contain non-dissem replacements
/// (for example, declass-shorthand entries like `25X1-` → `25X1`, which
/// E007 owns), and those MUST NOT dispatch as E006. Active dissem
/// controls absent from this allowlist (e.g., FOUO) simply never appear
/// as a replacement today — adding one is a deliberate E006 scope change,
/// not a passive widening.
///
/// `CUI` is intentionally excluded. Per CAPCO-2016 §F (and
/// `CVEnumISMDissem.xml`), `CUI` is not a CAPCO dissem control — it is a
/// NARA marking system. No MIGRATIONS entry currently has `CUI` as a
/// replacement (a prior `FOUO → CUI` entry was removed as factually
/// incorrect; see `crates/ism/build.rs` MIGRATIONS doc block). Keeping
/// `CUI` out of this set defends against re-introduction.
pub(crate) fn is_dissem_replacement(replacement: &str) -> bool {
    matches!(
        replacement,
        "RELIDO" | "NOFORN" | "ORCON" | "IMCON" | "DEA SENSITIVE" | "PROPIN"
    )
}

// ---------------------------------------------------------------------------
// W003 — Non-IC dissem in classified banner
// ---------------------------------------------------------------------------

/// Some non-IC dissemination controls must not appear in classified banners.
///
/// Per CAPCO-2016 §H.9 "Precedence Rules for Banner Line Guidance" (see
/// the per-marking rows on [`marque_ism::NonIcDissem::propagates_to_classified_banner`]):
///
/// - **Propagate to classified banners** (no W003): EXDIS, NODIS, LES,
///   LES-NF, SSI.
/// - **Do NOT propagate** (W003 fires): LIMDIS, SBU, SBU-NF. These
///   markings are "applicable only to unclassified information" per
///   §H.9 and their precedence rules explicitly say the marking is
///   stripped from the banner when the document is classified.
///
/// W003 is banner-only — a non-IC dissem control in a *portion* marking
/// is fine at any classification.
///
/// ## Important Exceptions
///
/// `LES-NF` has a further §H.9 canonicalization — the banner form
/// `SECRET//NOFORN//LES` rather than `SECRET//LES NOFORN`. That split
/// is a page-rewrite concern, not a W003 concern, so LES-NF is
/// considered propagating here.
///
/// Importantly, SBU-NF behaves similarly to LES-NF. 'SBU'
/// never propagates to a classified marking, but its
/// `NF` attribute *does*.
pub(super) struct NonIcInClassifiedBannerRule;

/// Citations W003 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const NON_IC_IN_CLASSIFIED_BANNER_AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 9, 169)];

impl Rule<CapcoScheme> for NonIcInClassifiedBannerRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "page.dissem.non-ic-dissem-in-classified-banner")
    }
    fn name(&self) -> &'static str {
        "non-ic-dissem-in-classified-banner"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    /// Phase::WholeMarking: banner-only decision reading the
    /// classification axis × non-IC dissem axis together; emits no fix
    /// (the SBU/LIMDIS removal is intentionally manual).
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        NON_IC_IN_CLASSIFIED_BANNER_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;
        if ctx.marking_type != MarkingType::Banner {
            return vec![];
        }

        if attrs.non_ic_dissem.is_empty() {
            return vec![];
        }

        // Non-IC dissem controls are fine only in UNCLASSIFIED banners.
        // Determine classification from the full banner classification, not
        // just the US-specific view, so non-US classified banners (NATO,
        // JOINT, FGI forms) are also checked.
        let is_classified = match &attrs.classification {
            Some(marque_ism::MarkingClassification::Us(c)) => {
                *c > marque_ism::Classification::Unclassified
            }
            Some(
                marque_ism::MarkingClassification::Fgi(_)
                | marque_ism::MarkingClassification::Nato(_)
                | marque_ism::MarkingClassification::Joint(_)
                | marque_ism::MarkingClassification::Conflict { .. },
            ) => true,
            None => false,
        };
        if !is_classified {
            return vec![];
        }

        let mut diagnostics = Vec::new();
        let nic_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::NonIcDissem)
            .collect();

        for (idx, nic) in attrs.non_ic_dissem.iter().enumerate() {
            // LIMDIS, LES, LES-NF, SSI propagate to classified banners.
            if nic.propagates_to_classified_banner() {
                continue;
            }

            let span = nic_spans
                .get(idx)
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));

            // G13: drop the runtime token-text interpolation. Template
            // identifies the violation class; the affected category is
            // CAT_NON_IC_DISSEM.
            let _ = nic; // emit-class is known without the runtime value
            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                Message::new(
                    MessageTemplate::NonIcDissemInClassifiedBanner,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                        ..MessageArgs::default()
                    },
                ),
                capco(SectionLetter::H, 9, 169),
                None,
            ));
        }

        diagnostics
    }
}
