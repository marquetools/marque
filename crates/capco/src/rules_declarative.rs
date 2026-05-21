// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Walker-style CAPCO rules that survive PR #578 consolidation.
//!
//! PR #578 retired the 16 thin declarative wrappers that used to live
//! here (E010 / E012 / E014 / E015 / E016 / E021 / E024 / E036 / E037 /
//! E038 / E053 / E054 / E055 / E056 / E057 + S004) into the engine's
//! constraint-catalog bridge. `severity` + `span_anchor` now ride on
//! [`marque_scheme::Constraint::Conflicts`] / [`marque_scheme::Constraint::Requires`]
//! directly, [`marque_scheme::MarkingScheme::token_span`] resolves the
//! diagnostic anchor against the scheme's marking, and the engine
//! synthesizes the optional [`marque_rules::FixIntent`] via
//! [`crate::scheme::CapcoScheme::fix_intent_by_name`].
//!
//! What remains here are the **walker rules** whose structural logic
//! (catalog matching with prefix / compound-form dispatch, multi-row
//! emission, suggest-only fall-throughs) does not fit the dyadic
//! `Constraint` shape:
//!
//! - [`DeprecatedSciLongFormRule`] (E065) — deprecated SCI long-form
//!   canonicalization walker per CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
//! - [`BareCanonicalCompoundRule`] (E067) — bare CNWDI / NK / EU
//!   short-form → canonical compound portion-mark rewriter per
//!   §H.6 p106 / §H.4 p83 / §H.4 p78.
//!
//! Both walkers are registered as `Box<dyn Rule>` in
//! `CapcoRuleSet::new()` and emit `Diagnostic` values directly — they
//! do not flow through the constraint-catalog bridge.

use marque_ism::{CanonicalAttrs, TokenKind};
use marque_rules::{
    Citation, Confidence, Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase,
    Rule, RuleContext, RuleId, SectionLetter, Severity, capco,
};

use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// E065 — Deprecated SCI long-form canonicalization walker (T135a)
// ---------------------------------------------------------------------------
//
// Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
//
// The walker matches deprecated SCI long-form tokens in
// `attrs.token_spans` against a static catalog and emits canonicalization
// fixes. The matching tokens are emitted by Commit 2's
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
    /// the manual carries a direct passage. PR 3c.2.C C5 retired the
    /// emission path through this field; the field stays alive as
    /// documentation citation-lint can read at compile time. Audit
    /// records carry only [`MessageTemplate::SupersededToken`] +
    /// `MessageArgs::default()` per Constitution V Principle V.
    #[allow(dead_code)] // Retained for documentation + citation-lint scanning.
    message: &'static str,
    /// Typed authoritative-source citation. Verified against
    /// `crates/capco/docs/CAPCO-2016.md` (Constitution Principle VIII).
    /// PR 10.A.1 consolidated the dual-track `citation: &'static str` +
    /// `citation_typed: Citation` design into this single typed field.
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

impl Rule<CapcoScheme> for DeprecatedSciLongFormRule {
    fn id(&self) -> RuleId {
        RuleId::new("E065")
    }

    fn name(&self) -> &'static str {
        "deprecated-sci-long-form"
    }

    fn default_severity(&self) -> Severity {
        // Per-row severities take precedence on emitted diagnostics; the
        // walker-level default severity is the strictest of the catalog
        // rows so a `.marque.toml [rules] E065 = ...` override anchor
        // cannot accidentally weaken any row below its authoring intent.
        // Mirrors the precedent set by `BannerMatchesProjectedRule`.
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut diagnostics = Vec::new();
        for token_span in attrs.token_spans.iter() {
            // Only consider tokens that the parser tagged as SCI controls
            // — `recognize_deprecated_sci_long_form` always emits the
            // deprecated long form under `TokenKind::SciControl` (see
            // Commit 2). Filtering here prevents the walker from
            // accidentally firing on, e.g., a free-text comment block
            // that happens to contain the bytes `HUMINT`.
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
    // PR 3c.2.C C5: all branches emit the typed
    // `MessageTemplate::SupersededToken` per the deprecation-class.
    // The narrative `row.message` lives as documentation only.
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
            Confidence::strict(1.0),
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
                Confidence::strict(1.0),
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
                        Confidence::strict(1.0),
                        None,
                    )
                }
                None => {
                    // G13: drop the runtime `comp` interpolation; the
                    // unrecognized-compartment class is captured by
                    // MessageTemplate::SupersededToken (the upstream
                    // form is deprecated regardless of compartment
                    // recognition).
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

// ===========================================================================
// E067 — bare-canonical-compound rewriters (issue #407)
// ===========================================================================
//
// Three legacy bare-form portion-mark tokens that have canonical
// CAPCO-2016 compound forms:
//
//   - bare `CNWDI` → `RD-CNWDI`  (CAPCO-2016 §H.6 p106)
//     `(U) Example Portion Mark: (S//RD-CNWDI)`. CNWDI is a sub-marker
//     of RD; the bare form omits the RD prefix that CAPCO §H.6 requires.
//   - bare `NK`    → `SI-NK`     (CAPCO-2016 §H.4 p83)
//     `(U) Authorized Portion Mark: SI-NK`. NK (NONBOOK) is an SI
//     compartment; the bare token-shape omits the SI control system.
//   - bare `EU`    → `SI-EU`     (CAPCO-2016 §H.4 p78)
//     `(U) Authorized Portion Mark: SI-EU`. EU (ECRU) is an SI
//     compartment; same omission shape as NK.
//
// All three of these legacy short-forms predate the relevant CAPCO
// re-org (ECRU/NONBOOK migration from EL → SI; CNWDI's compound
// portion form). When users carry forward the bare form, the
// authoritative manual permits the manual re-marking the
// `marque-autofix` channel exists to automate — per project memory
// `remark-on-derivative-use-is-marque-autofix`.
//
// All three bare forms parse as `TokenKind::Unknown` today (verified
// 2026-05-16): the structural SCI subparser dispatches on hyphenated
// shapes; bare `CNWDI` / `NK` / `EU` lack the hyphen so they fall
// through. `EU` in REL TO position parses as `TokenKind::RelToTrigraph`
// (registered 2-byte country code); in FGI position it routes through
// the FGI grammar — so filtering on `TokenKind::Unknown` is a sufficient
// category gate. No position-tracking logic is needed.
//
// The walker emits per-row severity `Severity::Fix` (`text_correction`
// channel; replacement is a hardcoded static string literal per
// Constitution V — no document content flows into the audit record).
// Per-row confidence `Confidence::strict(1.0)`: authoritative §-citation
// for each row and the canonical compound form is unambiguous.

/// One row of the E067 bare-canonical-compound catalog.
struct BareCanonicalCompoundRow {
    /// Source text the parser tags as `TokenKind::Unknown`.
    source: &'static str,
    /// Canonical replacement (hardcoded static literal).
    replacement: &'static str,
    /// Typed authoritative-source citation. Migrated from the dual-track
    /// `citation: &'static str` + `citation_typed: Citation` design to a
    /// single typed field in PR 10.A.1.
    citation: Citation,
    /// Diagnostic message text (static string).
    #[allow(dead_code)] // Retained for documentation; audit uses MessageTemplate.
    message: &'static str,
}

/// E067 catalog: bare legacy portion-mark short-forms → canonical
/// CAPCO-2016 compound portion marks. Iteration is exact-match per row;
/// no prefix/suffix logic.
const BARE_CANONICAL_COMPOUND_CATALOG: &[BareCanonicalCompoundRow] = &[
    BareCanonicalCompoundRow {
        source: "CNWDI",
        replacement: "RD-CNWDI",
        citation: capco(SectionLetter::H, 6, 106),
        message: "bare CNWDI is not a registered portion form; \
                  CAPCO-2016 §H.6 p106 specifies the canonical \
                  RD-CNWDI compound portion mark",
    },
    BareCanonicalCompoundRow {
        source: "NK",
        replacement: "SI-NK",
        citation: capco(SectionLetter::H, 4, 83),
        message: "bare NK is not a registered portion form; \
                  CAPCO-2016 §H.4 p83 specifies the canonical \
                  SI-NK portion mark for the NONBOOK SI compartment",
    },
    BareCanonicalCompoundRow {
        source: "EU",
        replacement: "SI-EU",
        citation: capco(SectionLetter::H, 4, 78),
        message: "bare EU is not a registered portion form; \
                  CAPCO-2016 §H.4 p78 specifies the canonical \
                  SI-EU portion mark for the ECRU SI compartment",
    },
];

/// Public lookup: does `text` match any E067 bare-canonical-compound
/// catalog source? Exposed at `pub(crate)` so E008
/// (`UnknownTokenRule`) can suppress co-firing on tokens that E067
/// owns. Exact-string match only — keeps the walker's category gate
/// (`TokenKind::Unknown`) the sole decision point for E067 firing.
pub(crate) fn is_bare_canonical_compound_form(text: &str) -> bool {
    BARE_CANONICAL_COMPOUND_CATALOG
        .iter()
        .any(|row| row.source == text)
}

/// Walker that rewrites bare legacy CNWDI / NK / EU portion-mark
/// short-forms to their CAPCO-2016 canonical compound forms.
///
/// See the section header above for the design rationale.
pub(crate) struct BareCanonicalCompoundRule;

impl Rule<CapcoScheme> for BareCanonicalCompoundRule {
    fn id(&self) -> RuleId {
        RuleId::new("E067")
    }

    fn name(&self) -> &'static str {
        "bare-canonical-compound"
    }

    fn default_severity(&self) -> Severity {
        // Per-row severities take precedence on emitted diagnostics
        // (all rows emit `Severity::Fix`). The walker-level default is
        // the strictest of the catalog rows so a `.marque.toml [rules]
        // E067 = ...` override anchor cannot accidentally weaken any
        // row below its authoring intent — mirrors the precedent set
        // by `BannerMatchesProjectedRule` (PR 3b.A) and
        // `DeprecatedSciLongFormRule` (E065).
        Severity::Error
    }

    /// `Phase::Localized`: every emitted `Diagnostic` carries a span
    /// that covers a single `TokenSpan` (the bare-form token).
    /// Text-correction replacements are byte-precise single-token
    /// splices that fit inside one token boundary — exactly the
    /// Localized contract.
    fn phase(&self) -> Phase {
        Phase::Localized
    }

    fn trusted(&self) -> bool {
        true
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut out = Vec::new();
        for token in attrs.token_spans.iter() {
            // Category gate: only `Unknown` tokens are E067-candidates.
            // The bare CNWDI / NK / EU forms always land as Unknown
            // (verified 2026-05-16); EU in REL TO position is
            // `RelToTrigraph` and EU in FGI position is FGI-routed —
            // both filtered out by this single check.
            if token.kind != TokenKind::Unknown {
                continue;
            }
            let text = token.text.as_str();
            for row in BARE_CANONICAL_COMPOUND_CATALOG {
                if text == row.source {
                    out.push(Diagnostic::text_correction(
                        self.id(),
                        Severity::Fix,
                        token.span,
                        Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                        row.citation,
                        row.replacement,
                        FixSource::BuiltinRule,
                        // Authoritative §-citation per row; the
                        // canonical compound form is unambiguous and
                        // the fix performs the manual's documented
                        // re-marking verbatim.
                        Confidence::strict(1.0),
                        None,
                    ));
                    break;
                }
            }
        }
        out
    }
}
