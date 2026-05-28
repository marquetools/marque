// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Deprecated SCI long-form canonicalization walker.
//!
//! [`DeprecatedSciLongFormRule`] (wire ID
//! `capco:portion.sci.deprecated-long-form`) is a hand-written catalog
//! walker, not a declarative `Constraint`: it matches deprecated SCI
//! long-form *source text* the parser tagged `TokenKind::SciControl`
//! against a static catalog and emits canonicalization fixes. Because
//! it predicates on raw token text rather than parsed category facts,
//! it cannot be expressed as a dyadic `Constraint` over the marking
//! model — hence a walker. (It lived in `rules_declarative.rs` until
//! that module was retired; the name was an accident of PR #578's
//! consolidation history, not a coherent grouping.)
//!
//! Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.

use marque_ism::{CanonicalAttrs, TokenKind};
use marque_rules::{
    Confidence, Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase, Rule,
    RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Deprecated SCI long-form canonicalization walker
// ---------------------------------------------------------------------------
//
// Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
//
// The walker matches deprecated SCI long-form tokens in
// `attrs.token_spans` against a static catalog and emits canonicalization
// fixes. The matching tokens are emitted by
// `recognize_deprecated_sci_long_form` in `marque-core::parser`, which
// preserves source bytes verbatim in `TokenSpan.text` so the walker can
// detect the deprecated form and propose its canonical replacement.
//
// Per the "re-mark on derivative use = Marque autofix" principle (see
// project memory): the manual's "legacy: retain on machine-to-machine
// carry; re-mark when incorporating into derivative product" language IS
// the autofix trigger — Marque exists precisely to automate the
// re-marking the manual permits handling by hand. Catalog rows that have
// an unambiguous canonical form auto-fix; rows where the compartment /
// sub-compartment context is missing emit a suggest-only diagnostic
// (the author must contact the originator to supply the missing context
// before Marque can canonicalize).

/// One catalog row per deprecated SCI long-form recognized by the walker.
struct DeprecatedSciRow {
    /// Source bytes that trigger this row, matched against
    /// `TokenSpan.text` exactly. Multi-word phrases include the spaces.
    /// Compartment-bearing forms have their compartment portion handled
    /// by `match_kind` below.
    source: &'static str,
    /// How `source` is matched against `TokenSpan.text`.
    match_kind: MatchKind,
    /// Severity emitted on every diagnostic from this row.
    severity: Severity,
    /// Canonical replacement strategy.
    replacement: ReplacementKind,
    /// Diagnostic message string — mirror manual wording verbatim where
    /// the manual carries a direct passage. The emission path does not
    /// go through this field; it stays alive as documentation
    /// citation-lint can read at compile time. Audit records carry only
    /// [`MessageTemplate::SupersededToken`] + `MessageArgs::default()`
    /// (Constitution V).
    #[allow(dead_code)] // Retained for documentation + citation-lint scanning.
    message: &'static str,
    /// Typed authoritative-source citation. Verified against
    /// `crates/capco/docs/CAPCO-2016.md` (Constitution Principle VIII).
    citation: Citation,
}

/// How a catalog row matches a `TokenSpan.text`.
#[derive(Clone, Copy)]
enum MatchKind {
    /// `TokenSpan.text == row.source` exactly.
    Exact,
    /// `TokenSpan.text.strip_prefix(row.source) == Some(<COMP>)` and
    /// `<COMP>` is `[A-Z0-9]+`. The space between the prefix and the
    /// compartment must be present in `row.source` (e.g., `"ECI "`).
    PrefixSpace,
    /// `TokenSpan.text.strip_prefix(row.source) == Some(<COMP>)` and
    /// `<COMP>` is `[A-Z0-9]+`. The hyphen between the prefix and the
    /// compartment must be present in `row.source` (e.g., `"KDK-"`).
    PrefixHyphen,
}

/// Canonical replacement strategy for a catalog row.
#[derive(Clone, Copy)]
enum ReplacementKind {
    /// Replace with a `&'static str` canonical form (no compartment context).
    Static(&'static str),
    /// Build the canonical form as `"{prefix}{compartment}"`. The walker
    /// pulls the compartment from the source token (via the `PrefixSpace`
    /// / `PrefixHyphen` match).
    WithCompartment { prefix: &'static str },
    /// Like `WithCompartment` but translates the captured compartment to
    /// its canonical short form via a `(legacy, canonical)` mapping table
    /// before prepending the prefix. For compartments not in the mapping,
    /// the walker emits a Warn-Suggest diagnostic (no text correction) —
    /// Marque cannot fabricate canonical short forms for compartments the
    /// authoritative source doesn't document. Used by the KDK / KLONDIKE
    /// rows where CAPCO-2016 §H.4 p85 + p87/p91/p95 spell out the
    /// `BLUEFISH → BLFH`, `IDITAROD → IDIT`, `KANDIK → KAND`
    /// abbreviation and the canonical CVE vocabulary has no entry for
    /// the long-form (e.g., no `TK-BLUEFISH`).
    WithMappedCompartment {
        prefix: &'static str,
        mapping: &'static [(&'static str, &'static str)],
    },
    /// Suggest-only — no fix proposal. The walker emits
    /// `Diagnostic::info` instead of `Diagnostic::text_correction`.
    SuggestOnly,
}

/// CAPCO-2016 §H.4 p85 (KLONDIKE closure: NSG PM 3802) + §H.4 p87
/// (TK-BLUEFISH portion abbreviation BLFH) + §H.4 p91 (TK-IDITAROD
/// portion abbreviation IDIT) + §H.4 p95 (TK-KANDIK portion abbreviation
/// KAND). The legacy long-form compartment names `BLUEFISH` / `IDITAROD`
/// / `KANDIK` appear under the deprecated `KDK-` / `KLONDIKE-` prefixes;
/// the canonical CVE vocabulary registers only the abbreviated forms
/// `TK-BLFH` / `TK-IDIT` / `TK-KAND`. Emitting `TK-BLUEFISH` would
/// produce a marking with no CVE entry — strictly worse than no fix.
const KDK_COMPARTMENT_MAPPING: &[(&str, &str)] = &[
    ("BLUEFISH", "BLFH"),
    ("IDITAROD", "IDIT"),
    ("KANDIK", "KAND"),
];

/// The catalog of deprecated SCI long-form rules.
///
/// Rows are ordered for documentation, not for matching semantics — the
/// walker selects the first row whose `source` / `match_kind` accepts
/// the token. Longer prefixes are listed before shorter ones (e.g.,
/// `EXCEPTIONALLY CONTROLLED INFORMATION ` before `ECI `) so the
/// first-match-wins iteration picks the longest applicable form.
const DEPRECATED_SCI_LONG_FORM_CATALOG: &[DeprecatedSciRow] = &[
    // -----------------------------------------------------------------
    // HCS family — §H.4 p62
    // -----------------------------------------------------------------
    // §H.4 p62: "When incorporating legacy material marked 'HCS' into a
    // new product, re-mark the new document and associated portion
    // according to the instructions in the HCS-O and HCS-P marking
    // templates." HUMINT is the spelled-out long form; HCS is the
    // canonical short form.
    DeprecatedSciRow {
        source: "HUMINT CONTROL SYSTEM",
        match_kind: MatchKind::Exact,
        severity: Severity::Error,
        replacement: ReplacementKind::Static("HCS"),
        message: "'HUMINT CONTROL SYSTEM' is the legacy long form per CAPCO-2016 §H.4 p62; \
             re-mark to HCS for derivative use",
        citation: capco(SectionLetter::H, 4, 62),
    },
    DeprecatedSciRow {
        source: "HUMINT",
        match_kind: MatchKind::Exact,
        severity: Severity::Error,
        replacement: ReplacementKind::Static("HCS"),
        message: "'HUMINT' is the legacy form per CAPCO-2016 §H.4 p62; \
                  re-mark to HCS for derivative use",
        citation: capco(SectionLetter::H, 4, 62),
    },
    // -----------------------------------------------------------------
    // SI family (COMINT / SPECIAL INTELLIGENCE) — §H.4 p74
    // -----------------------------------------------------------------
    // §H.4 p74: "The COMINT title for the Special Intelligence (SI)
    // control system is no longer valid".
    DeprecatedSciRow {
        source: "SPECIAL INTELLIGENCE",
        match_kind: MatchKind::Exact,
        severity: Severity::Error,
        replacement: ReplacementKind::Static("SI"),
        message: "'SPECIAL INTELLIGENCE' is the legacy long form per CAPCO-2016 §H.4 p74; \
                  use SI",
        citation: capco(SectionLetter::H, 4, 74),
    },
    DeprecatedSciRow {
        source: "COMINT",
        match_kind: MatchKind::Exact,
        severity: Severity::Error,
        replacement: ReplacementKind::Static("SI"),
        message: "'COMINT' is no longer valid per CAPCO-2016 §H.4 p74; use SI",
        citation: capco(SectionLetter::H, 4, 74),
    },
    // -----------------------------------------------------------------
    // SI family (ECI / EXCEPTIONALLY CONTROLLED INFORMATION) — §H.4 p61 + p76
    // -----------------------------------------------------------------
    // §H.4 p76: "information formerly marked TS//SI-ECI ABC must now be
    // marked TS//SI-ABC". §H.4 p61: "ECI grouping markings are NOT used
    // in banner/portion".
    DeprecatedSciRow {
        source: "EXCEPTIONALLY CONTROLLED INFORMATION ",
        match_kind: MatchKind::PrefixSpace,
        severity: Severity::Error,
        replacement: ReplacementKind::WithCompartment { prefix: "SI-" },
        message: "'EXCEPTIONALLY CONTROLLED INFORMATION' grouping must not be used \
                  per CAPCO-2016 §H.4 p61; mark as SI-<compartment>",
        citation: capco(SectionLetter::H, 4, 61),
        // Typed Citation anchors at §H.4 p61 (SCI grammar); p76
        // cross-referenced in row.message documentation.
    },
    DeprecatedSciRow {
        source: "ECI ",
        match_kind: MatchKind::PrefixSpace,
        severity: Severity::Error,
        replacement: ReplacementKind::WithCompartment { prefix: "SI-" },
        message: "ECI grouping must not be used per CAPCO-2016 §H.4 p61; \
                  mark as SI-<compartment>",
        citation: capco(SectionLetter::H, 4, 61),
        // Typed Citation anchors at §H.4 p61 (SCI grammar); p76
        // cross-referenced in row.message documentation.
    },
    DeprecatedSciRow {
        source: "EXCEPTIONALLY CONTROLLED INFORMATION",
        match_kind: MatchKind::Exact,
        // Bare EXCEPTIONALLY CONTROLLED INFORMATION fires at Error
        // severity: ECI was never a control system at all — it was a
        // 'control group' (a classification-of-SCIs attribute that
        // appeared alongside SI before the migration). The entire concept
        // is fully replaced by SI per §H.4 p61 ("ECI grouping markings
        // are NOT used in banner/portion") + §H.4 p76. Most severely
        // deprecated of the bare-X cases. Suggest-only — Marque can't
        // synthesize the compartment that the user originally intended
        // under the legacy SI-ECI nesting.
        severity: Severity::Error,
        replacement: ReplacementKind::SuggestOnly,
        message: "Bare 'EXCEPTIONALLY CONTROLLED INFORMATION' is not a control system \
                  per CAPCO-2016 §H.4 p61; contact the originator for the compartment",
        citation: capco(SectionLetter::H, 4, 61),
    },
    DeprecatedSciRow {
        source: "ECI",
        match_kind: MatchKind::Exact,
        // Bare ECI fires at Error severity: ECI was never a control system
        // at all — it was a 'control group' (a classification-of-SCIs
        // attribute that appeared alongside SI before the migration). The
        // entire concept is fully replaced by SI per §H.4 p61 ("ECI
        // grouping markings are NOT used in banner/portion") + §H.4 p76.
        // Most severely deprecated of the bare-X cases. Suggest-only —
        // Marque can't synthesize the compartment that the user originally
        // intended under the legacy SI-ECI nesting.
        severity: Severity::Error,
        replacement: ReplacementKind::SuggestOnly,
        message: "Bare ECI is not a control system per CAPCO-2016 §H.4 p61; \
                  contact the originator for the compartment",
        citation: capco(SectionLetter::H, 4, 61),
    },
    // -----------------------------------------------------------------
    // SI family (EL / ENDSEAL) — §H.4 p78 + p83
    // -----------------------------------------------------------------
    // §H.4 p78: "the EL control system is being retired and all
    // associated compartments moved to the SI control system". ECRU
    // and NONBOOK are the two legacy EL compartments named in §H.4 p78
    // (ECRU) and §H.4 p83 (NONBOOK).
    DeprecatedSciRow {
        source: "ENDSEAL ",
        match_kind: MatchKind::PrefixSpace,
        severity: Severity::Error,
        replacement: ReplacementKind::WithCompartment { prefix: "SI-" },
        message: "EL/ENDSEAL control system is being retired per CAPCO-2016 §H.4 p78; \
                  mark as SI-<compartment>",
        citation: capco(SectionLetter::H, 4, 78),
    },
    DeprecatedSciRow {
        source: "EL ",
        match_kind: MatchKind::PrefixSpace,
        severity: Severity::Error,
        replacement: ReplacementKind::WithCompartment { prefix: "SI-" },
        message: "EL control system is being retired per CAPCO-2016 §H.4 p78; \
                  mark as SI-<compartment>",
        citation: capco(SectionLetter::H, 4, 78),
    },
    DeprecatedSciRow {
        source: "ENDSEAL",
        match_kind: MatchKind::Exact,
        // Bare ENDSEAL fires at Error severity: the control system itself
        // was retired (EL/ENDSEAL → SI; transition predates CAPCO-2016,
        // likely in the 2013 manual). Unlike bare HCS/RSV (E061/E063 —
        // valid controls needing compartment context), the bare form here
        // has no canonical migration because the source control system is
        // gone — the user MUST contact the originator for the compartment
        // to complete the migration. Suggest-only (no auto-fix) because
        // Marque can't fabricate the compartment.
        severity: Severity::Error,
        replacement: ReplacementKind::SuggestOnly,
        message: "ENDSEAL is being retired into SI per CAPCO-2016 §H.4 p78; \
                  sub-compartment context required to migrate; contact the originator",
        citation: capco(SectionLetter::H, 4, 78),
    },
    DeprecatedSciRow {
        source: "EL",
        match_kind: MatchKind::Exact,
        // Bare EL fires at Error severity: the control system itself was
        // retired (EL/ENDSEAL → SI; transition predates CAPCO-2016,
        // likely in the 2013 manual). Unlike bare HCS/RSV (E061/E063 —
        // valid controls needing compartment context), the bare form here
        // has no canonical migration because the source control system is
        // gone — the user MUST contact the originator for the compartment
        // to complete the migration. Suggest-only (no auto-fix) because
        // Marque can't fabricate the compartment.
        severity: Severity::Error,
        replacement: ReplacementKind::SuggestOnly,
        message: "EL control system is being retired into SI per CAPCO-2016 §H.4 p78; \
                  compartment context required to migrate; contact the originator",
        citation: capco(SectionLetter::H, 4, 78),
    },
    // -----------------------------------------------------------------
    // TK family (KDK / KLONDIKE) — §H.4 p85 (NSG PM 3802)
    // -----------------------------------------------------------------
    // §H.4 p85 (NSG PM 3802 Closure of KLONDIKE Control System): "When
    // incorporating legacy material marked 'KLONDIKE' into a new
    // product, re-mark the new document and associated portions
    // according to the instructions in the TK-BLFH, TK-IDIT, and
    // TK-KAND marking templates."
    //
    // The legacy long-form compartment identifiers (BLUEFISH / IDITAROD
    // / KANDIK) map to canonical short forms (BLFH / IDIT / KAND) per
    // §H.4 p87 / p91 / p95. The mapping lives in
    // `KDK_COMPARTMENT_MAPPING`; the walker translates a recognized
    // legacy compartment to its canonical short form before producing
    // the `TK-<comp>` replacement. Unknown compartments emit a Warn
    // suggestion with no text correction — Marque does not fabricate
    // canonical short forms for compartments the authoritative source
    // doesn't document.
    DeprecatedSciRow {
        source: "KLONDIKE-",
        match_kind: MatchKind::PrefixHyphen,
        severity: Severity::Error,
        replacement: ReplacementKind::WithMappedCompartment {
            prefix: "TK-",
            mapping: KDK_COMPARTMENT_MAPPING,
        },
        message: "Per CAPCO-2016 §H.4 p85 (NSG PM 3802 closure), re-mark KLONDIKE \
                  compartments to TK-BLFH / TK-IDIT / TK-KAND",
        citation: capco(SectionLetter::H, 4, 85),
    },
    DeprecatedSciRow {
        source: "KDK-",
        match_kind: MatchKind::PrefixHyphen,
        severity: Severity::Error,
        replacement: ReplacementKind::WithMappedCompartment {
            prefix: "TK-",
            mapping: KDK_COMPARTMENT_MAPPING,
        },
        message: "Per CAPCO-2016 §H.4 p85 (NSG PM 3802 closure), re-mark KDK \
                  compartments to TK-BLFH / TK-IDIT / TK-KAND",
        citation: capco(SectionLetter::H, 4, 85),
    },
    DeprecatedSciRow {
        source: "KLONDIKE",
        match_kind: MatchKind::Exact,
        // Bare KLONDIKE fires at Error severity: the control system itself
        // was retired (KDK/KLONDIKE → TK; transition predates CAPCO-2016,
        // documented in NSG PM 3802). Unlike bare HCS/RSV (E061/E063 —
        // valid controls needing compartment context), the bare form here
        // has no canonical migration because the source control system is
        // gone — the user MUST contact the originator for the compartment
        // to complete the migration. Suggest-only (no auto-fix) because
        // Marque can't fabricate the compartment.
        severity: Severity::Error,
        replacement: ReplacementKind::SuggestOnly,
        message: "KLONDIKE closed per NSG PM 3802 (CAPCO-2016 §H.4 p85); \
                  compartment context required to migrate to TK-<compartment>",
        citation: capco(SectionLetter::H, 4, 85),
    },
    DeprecatedSciRow {
        source: "KDK",
        match_kind: MatchKind::Exact,
        // Bare KDK fires at Error severity: the control system itself was
        // retired (KDK/KLONDIKE → TK; transition predates CAPCO-2016,
        // documented in NSG PM 3802). Unlike bare HCS/RSV (E061/E063 —
        // valid controls needing compartment context), the bare form here
        // has no canonical migration because the source control system is
        // gone — the user MUST contact the originator for the compartment
        // to complete the migration. Suggest-only (no auto-fix) because
        // Marque can't fabricate the compartment.
        severity: Severity::Error,
        replacement: ReplacementKind::SuggestOnly,
        message: "KDK closed per NSG PM 3802 (CAPCO-2016 §H.4 p85); \
                  compartment context required to migrate to TK-<compartment>",
        citation: capco(SectionLetter::H, 4, 85),
    },
];

/// Walker that emits canonicalization diagnostics for deprecated SCI
/// long-form tokens. See the section header above for the design
/// rationale.
pub(crate) struct DeprecatedSciLongFormRule;

/// Citations the [`DeprecatedSciLongFormRule`] walker may emit on
/// diagnostics — the union of `citation` fields across every row in
/// [`DEPRECATED_SCI_LONG_FORM_CATALOG`]. The walker registers under
/// `capco:portion.sci.deprecated-long-form` and per-row IDs travel on
/// emitted diagnostics via the row metadata. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate contract.
const E065_AUTHORITIES: &[Citation] = &[
    // HUMINT / HUMINT CONTROL SYSTEM → HCS (§H.4 p62).
    capco(SectionLetter::H, 4, 62),
    // COMINT / SPECIAL INTELLIGENCE → SI (§H.4 p74).
    capco(SectionLetter::H, 4, 74),
    // ECI / EXCEPTIONALLY CONTROLLED INFORMATION → SI-<comp> (§H.4 p61).
    capco(SectionLetter::H, 4, 61),
    // ENDSEAL / EL → SI (§H.4 p78).
    capco(SectionLetter::H, 4, 78),
    // KLONDIKE / KDK → TK (§H.4 p85).
    capco(SectionLetter::H, 4, 85),
];

impl Rule<CapcoScheme> for DeprecatedSciLongFormRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.sci.deprecated-long-form")
    }

    fn name(&self) -> &'static str {
        "deprecated-sci-long-form"
    }

    fn default_severity(&self) -> Severity {
        // Per-row severities take precedence on emitted diagnostics; the
        // walker-level default severity is the strictest of the catalog
        // rows so a `.marque.toml [rules] "capco:portion.sci.deprecated-long-form" = ...`
        // override anchor cannot accidentally weaken any row below its
        // authoring intent. Mirrors the precedent set by
        // `BannerMatchesProjectedRule`.
        Severity::Error
    }

    /// Phase::Localized: every emitted `Diagnostic` carries a span that
    /// covers a single TokenSpan (the deprecated long-form token).
    /// Text-correction replacements are byte-precise single-token splices.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }

    fn cited_authorities(&self) -> &'static [Citation] {
        E065_AUTHORITIES
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut diagnostics = Vec::new();
        for token_span in attrs.token_spans.iter() {
            // Only consider tokens that the parser tagged as SCI controls
            // — `recognize_deprecated_sci_long_form` always emits the
            // deprecated long form under `TokenKind::SciControl`.
            // Filtering here prevents the walker from accidentally firing
            // on, e.g., a free-text comment block that happens to contain
            // the bytes `HUMINT`.
            if token_span.kind != TokenKind::SciControl {
                continue;
            }
            let text = token_span.text.as_str();
            if let Some(diag) = match_catalog_row(text, token_span.span, self.id()) {
                diagnostics.push(diag);
            }
        }
        diagnostics
    }
}

/// Match `text` against the catalog and produce a diagnostic when a row
/// fires. Returns `None` if no row matches.
///
/// The walker emits a single diagnostic per matched token; first-match-
/// wins iteration order means the catalog must list longer prefixes
/// before shorter ones (already enforced by the catalog ordering).
fn match_catalog_row(
    text: &str,
    span: marque_scheme::Span,
    rule_id: RuleId,
) -> Option<Diagnostic<CapcoScheme>> {
    for row in DEPRECATED_SCI_LONG_FORM_CATALOG {
        let compartment = match row.match_kind {
            MatchKind::Exact => {
                if text == row.source {
                    None
                } else {
                    continue;
                }
            }
            MatchKind::PrefixSpace | MatchKind::PrefixHyphen => {
                let Some(rest) = text.strip_prefix(row.source) else {
                    continue;
                };
                if rest.is_empty() || !is_alnum_upper(rest) {
                    continue;
                }
                Some(rest)
            }
        };

        return Some(emit_diagnostic(row, span, rule_id, compartment));
    }
    None
}

/// Build the canonical replacement string and emit the diagnostic.
fn emit_diagnostic(
    row: &'static DeprecatedSciRow,
    span: marque_scheme::Span,
    rule_id: RuleId,
    compartment: Option<&str>,
) -> Diagnostic<CapcoScheme> {
    // All branches emit the typed `MessageTemplate::SupersededToken`
    // per the deprecation-class. The narrative `row.message` lives as
    // documentation only.
    let message = Message::new(MessageTemplate::SupersededToken, MessageArgs::default());
    match row.replacement {
        ReplacementKind::Static(canonical) => Diagnostic::text_correction(
            rule_id,
            row.severity,
            span,
            message,
            row.citation,
            canonical,
            FixSource::BuiltinRule,
            // Authoritative migration per §H.4 — full confidence (1.0).
            // The fix performs the manual's "re-mark to HCS / SI / TK"
            // instruction verbatim; no ambiguity in either the trigger
            // or the canonical form.
            Confidence::strict(),
            None,
        ),
        ReplacementKind::WithCompartment { prefix } => {
            let comp = compartment.expect("PrefixSpace/PrefixHyphen always supplies compartment");
            let replacement = format!("{prefix}{comp}");
            Diagnostic::text_correction(
                rule_id,
                row.severity,
                span,
                message,
                row.citation,
                replacement,
                FixSource::BuiltinRule,
                // Compound forms: the canonical form is constructed
                // mechanically from the deprecated prefix's documented
                // replacement (`SI-` / `TK-`) and the original
                // compartment. Confidence stays at 1.0 for the
                // recognized prefixes (`HUMINT`/`COMINT`/`ECI`/`EL`/
                // `KDK`/`KLONDIKE`); rule-level severity-override is
                // the user's escape hatch when org policy disagrees
                // with the manual.
                Confidence::strict(),
                None,
            )
        }
        ReplacementKind::WithMappedCompartment { prefix, mapping } => {
            let comp = compartment.expect("PrefixSpace/PrefixHyphen always supplies compartment");
            // Translate the captured legacy compartment to its canonical
            // short form. If the compartment is not in the mapping table,
            // emit a Warn-Suggest diagnostic (no text correction) —
            // Marque cannot fabricate canonical short forms for
            // compartments the authoritative source (CAPCO-2016 §H.4
            // p85 + p87 / p91 / p95) does not document. Producing an
            // invalid CVE marking like `TK-FROBNITZ` would be strictly
            // worse than no fix.
            let canonical_comp = mapping
                .iter()
                .find_map(|(legacy, canonical)| (*legacy == comp).then_some(*canonical));
            match canonical_comp {
                Some(canonical) => {
                    let replacement = format!("{prefix}{canonical}");
                    Diagnostic::text_correction(
                        rule_id,
                        row.severity,
                        span,
                        message,
                        row.citation,
                        replacement,
                        FixSource::BuiltinRule,
                        // Authoritative mapping: §H.4 p85 + p87/p91/p95
                        // document the BLUEFISH→BLFH / IDITAROD→IDIT /
                        // KANDIK→KAND abbreviation. Full confidence.
                        Confidence::strict(),
                        None,
                    )
                }
                None => {
                    // Audit content-ignorance: the runtime `comp` is not
                    // interpolated; the unrecognized-compartment class is
                    // captured by MessageTemplate::SupersededToken (the
                    // upstream form is deprecated regardless of
                    // compartment recognition).
                    let _ = comp;
                    Diagnostic::info(rule_id, Severity::Warn, span, message, row.citation)
                }
            }
        }
        ReplacementKind::SuggestOnly => {
            Diagnostic::info(rule_id, row.severity, span, message, row.citation)
        }
    }
}

/// `[A-Z0-9]+` shape gate for compartment slot. Inlined here to keep
/// the walker self-contained; mirrors the predicate used by
/// `marque_core::parser::is_alnum_upper`.
fn is_alnum_upper(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}
