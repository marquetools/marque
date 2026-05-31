// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Bare-form → canonical-compound portion-mark rewriters.
//!
//! [`BareCanonicalCompoundRule`] (wire ID
//! `capco:marking.recanonicalize.bare-canonical-compound`) is a
//! hand-written catalog walker, not a declarative `Constraint`: it
//! matches legacy bare short-form *source text* the parser tagged
//! `TokenKind::Unknown` against a static catalog and rewrites it to the
//! canonical CAPCO-2016 compound form. Because it predicates on raw,
//! unrecognized token text rather than parsed category facts, it cannot
//! be expressed as a dyadic `Constraint` over the marking model — hence
//! a walker. (It lived in `rules_declarative/` until that module was
//! retired; it is a sibling of `nato::LegacyNatoCompoundRemarkRule`,
//! which performs the same recanonicalization in the NATO axis.)

use marque_ism::{CanonicalAttrs, TokenKind};
use marque_rules::{
    Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase, Recognition, Rule,
    RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

// ===========================================================================
// Bare-canonical-compound rewriters (issue #407)
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
// Per-row confidence `Recognition::strict()`: authoritative §-citation
// for each row and the canonical compound form is unambiguous.

/// One row of the bare-canonical-compound catalog.
struct BareCanonicalCompoundRow {
    /// Source text the parser tags as `TokenKind::Unknown`.
    source: &'static str,
    /// Canonical replacement (hardcoded static literal).
    replacement: &'static str,
    /// Typed authoritative-source citation.
    citation: Citation,
    /// Diagnostic message text (static string).
    #[allow(dead_code)] // Retained for documentation; audit uses MessageTemplate.
    message: &'static str,
}

/// Catalog: bare legacy portion-mark short-forms → canonical
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

/// Public lookup: does `text` match any bare-canonical-compound
/// catalog source? Exposed at `pub(crate)` so E008
/// (`UnknownTokenRule`) can suppress co-firing on tokens that this rule
/// owns. Exact-string match only — keeps the walker's category gate
/// (`TokenKind::Unknown`) the sole decision point for firing.
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

/// Citations the [`BareCanonicalCompoundRule`] walker may emit on
/// diagnostics — the union of `citation` fields across every row in
/// [`BARE_CANONICAL_COMPOUND_CATALOG`]. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate
/// contract.
const E067_AUTHORITIES: &[Citation] = &[
    // CNWDI → RD-CNWDI (§H.6 p106).
    capco(SectionLetter::H, 6, 106),
    // NK → SI-NK (§H.4 p83).
    capco(SectionLetter::H, 4, 83),
    // EU → SI-EU (§H.4 p78).
    capco(SectionLetter::H, 4, 78),
];

impl Rule<CapcoScheme> for BareCanonicalCompoundRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.recanonicalize.bare-canonical-compound")
    }

    fn name(&self) -> &'static str {
        "bare-canonical-compound"
    }

    fn default_severity(&self) -> Severity {
        // Per-row severities take precedence on emitted diagnostics
        // (all rows emit `Severity::Fix`). The walker-level default is
        // the strictest of the catalog rows so a `.marque.toml [rules]
        // "capco:marking.recanonicalize.bare-canonical-compound" = ...`
        // override anchor cannot accidentally weaken any row below its
        // authoring intent — mirrors the precedent set by
        // `BannerMatchesProjectedRule` and `DeprecatedSciLongFormRule`.
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

    fn cited_authorities(&self) -> &'static [Citation] {
        E067_AUTHORITIES
    }

    fn check(
        &self,
        attrs: &CanonicalAttrs,
        _ctx: &RuleContext<'_, CapcoScheme>,
    ) -> Vec<Diagnostic<CapcoScheme>> {
        let mut out = Vec::new();
        for token in attrs.token_spans.iter() {
            // Category gate: only `Unknown` tokens are candidates.
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
                        Recognition::strict(),
                        None,
                    ));
                    break;
                }
            }
        }
        out
    }
}
