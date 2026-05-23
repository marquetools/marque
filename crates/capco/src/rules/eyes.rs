// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`EyesOnlyConvertToRelToRule`] — EYES / EYES ONLY → REL TO conversion
//! per CAPCO-2016 §H.8 p157 + §H.8 p158.

use marque_ism::{CanonicalAttrs, CountryCode, MarkingType, TokenKind};
use marque_rules::{
    Confidence, Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase, Rule,
    RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

// ===========================================================================
// E064 — EYES / EYES ONLY → REL TO conversion (T135a Commit 5)
// ===========================================================================
//
// Authority: CAPCO-2016 §H.8 p157 + §H.8 p158.
//
// §H.8 p157: EYES ONLY is NSA-only and deprecated; the markings waiver
// expired 1 Oct 2017 (post-manual). §H.8 p158: "When extracting EYES
// ONLY portions from SIGINT reporting, convert the EYES ONLY portion
// marks to REL TO" and "carry forward the trigraph/tetragraph codes
// listed in the source document banner line to the new portion mark."
//
// E064 emits a `text_correction` covering the source-bytes of the EYES
// block (the parser preserves `<TRIGRAPHS> EYES [ONLY]` source text
// verbatim in `TokenSpan.text` per the Commit 2 recognizer). The
// replacement is the canonical `REL TO USA, <list>` form: USA
// prepended per §A.6 p16 + §H.8 p150-151 REL TO template, remaining
// codes sorted alphabetically, comma-space delimited per §A.6 p16.
//
// Note: the EYES source format is trigraph-only per §H.8 p157 line
// 3874-3875 ("Country trigraph codes are separated by single forward
// slashes"), so the recognizer rejects tetragraph inputs in the EYES
// prefix. The diagnostic message still mirrors §H.8 p158's
// "trigraph/tetragraph" wording verbatim because that wording refers
// to the carry-forward from the source-document banner line, where
// tetragraphs may legitimately appear. A future page-context-aware
// pass may surface banner-line tetragraphs into REL TO output, but
// is out of PR 9a scope.
//
// Implementation note: cross-axis migration (remove EYES from dissem +
// add trigraphs to rel_to) is not expressible as a single
// `ReplacementIntent` — the intent vocabulary's `FactAdd` /
// `FactRemove` / `Recanonicalize` variants are strictly single-axis-
// scoped. A `FixIntent` mirror of the E041 pattern would either need a
// new `Migrate { from, to, scope }` intent variant (engine/scheme
// edit out of scope here) or an engine-side composition of two atomic
// intents (architectural change beyond Commit 5's scope). The
// `text_correction` channel is the existing route that delivers the
// same user-facing outcome — a byte-precise canonicalization splice
// at the EYES block span. The brief's "FixIntent / mirror E041"
// guidance assumed intra-axis migration shape; the EYES → REL TO
// case is documented as cross-axis in `project_incompatibility_class.md`
// (memory). Selecting the existing text_correction path is the
// citation-honest implementation under today's intent vocabulary.

/// Rule E064 — convert EYES / EYES ONLY portions to REL TO per §H.8 p157.
pub(super) struct EyesOnlyConvertToRelToRule;

/// Citations E064 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 157)];

impl Rule<CapcoScheme> for EyesOnlyConvertToRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.eyes-only-convert-to-rel-to")
    }
    fn name(&self) -> &'static str {
        "eyes-only-convert-to-rel-to"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::Localized: the diagnostic span covers a single
    /// `TokenKind::DissemControl` block (the EYES compound block).
    /// `text_correction` is a byte-precise single-span splice that
    /// fits inside one token boundary — exactly the Localized
    /// contract. Pass-1 applies the fix; the re-parse for pass-2
    /// sees the canonical REL TO output.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut out = Vec::new();
        for token in attrs.token_spans.iter() {
            if token.kind != TokenKind::DissemControl {
                continue;
            }
            // The compound EYES block carries `<trigraph>(/<trigraph>)*
            // EYES [ONLY]`. We detect the compound form by suffix-
            // matching ` EYES ONLY` / ` EYES` (with a space before EYES)
            // so the prefix is the trigraph list. The bare forms (`"EYES"`
            // and `"EYES ONLY"` without any preceding list) are handled
            // by the explicit equality arms below — they do not carry the
            // leading space that `strip_suffix` requires.
            let text = token.text.as_str();
            let (prefix, _full_form) = if let Some(p) = text.strip_suffix(" EYES ONLY") {
                (p, true)
            } else if let Some(p) = text.strip_suffix(" EYES") {
                (p, false)
            } else if text == "EYES ONLY" {
                // Bare ODNI-title form: token text is the full ODNI long
                // description "EYES ONLY" (from MARKING_FORMS banner
                // form). No trigraph prefix — empty prefix triggers the
                // banner-FVEY branch below.
                ("", true)
            } else if text == "EYES" {
                // Bare CVE-value form: token text is the raw CVE value
                // "EYES". Same semantics as bare "EYES ONLY" — no
                // trigraph prefix, banner-FVEY branch below.
                ("", false)
            } else {
                continue;
            };
            if prefix.is_empty() {
                // Bare `EYES` / `EYES ONLY` token — no preceding country
                // list. Semantics differ by marking context:
                //
                // • Banner context: per §H.8 p157, a bare EYES ONLY banner
                //   without a country list implies the full Five Eyes (FVEY)
                //   membership (USA, AUS, CAN, GBR, NZL). Fire E064 with the
                //   FVEY REL TO replacement so the author gets a canonical
                //   conversion rather than a silent, unresolvable token.
                //
                // • Portion context: out of scope. §H.8 p158 says "carry
                //   forward the trigraph codes listed in the source document
                //   banner line" — a bare portion `EYES` is intentionally
                //   abbreviated when the page banner has the full `[LIST]
                //   EYES ONLY` form. Marque cannot synthesize the country
                //   list from the portion alone without banner context.
                //
                // Authority: CAPCO-2016 §H.8 p157 + p158.
                if ctx.marking_type == MarkingType::Banner {
                    out.push(Diagnostic::text_correction(
                        self.id(),
                        self.default_severity(),
                        token.span,
                        Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                        capco(SectionLetter::H, 8, 157),
                        build_rel_to_replacement(&[
                            CountryCode::USA.to_string(),
                            CountryCode::AUS.to_string(),
                            CountryCode::CAN.to_string(),
                            CountryCode::GBR.to_string(),
                            CountryCode::NZL.to_string(),
                        ]),
                        FixSource::BuiltinRule,
                        Confidence::strict(1.0),
                        None,
                    ));
                }
                continue;
            }

            // Parse the trigraph list, USA-first sort the rest.
            let trigraphs = parse_eyes_trigraphs(prefix);
            let canonical = build_rel_to_replacement(&trigraphs);

            // No-op guard: if the trigraph list is somehow empty after
            // sorting (should not happen given the parser's
            // shape gate), skip emission.
            if canonical.is_empty() {
                continue;
            }

            out.push(Diagnostic::text_correction(
                self.id(),
                self.default_severity(),
                token.span,
                Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                capco(SectionLetter::H, 8, 157),
                canonical,
                FixSource::BuiltinRule,
                Confidence::strict(1.0),
                None,
            ));
        }
        out
    }
}

/// Parse the `/`-delimited trigraph prefix of an EYES block into a
/// `Vec<String>`. The prefix is the part before ` EYES` / ` EYES ONLY`.
/// Trigraphs are uppercase 3-letter codes per §H.8 p150-151.
fn parse_eyes_trigraphs(prefix: &str) -> Vec<String> {
    prefix
        .split('/')
        .map(|s| s.to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build the canonical `REL TO USA, <list>` replacement string.
///
/// Per CAPCO-2016 §A.6 p16 + §H.8 p150-151 the country list begins
/// with USA when USA is present; remaining codes are sorted
/// alphabetically. The list separator is `, ` (comma-space) per
/// §A.6 p16. (§H.3's USA-first rule applies to JOINT's own
/// `[LIST]`, not to REL TO.)
pub(crate) fn build_rel_to_replacement(trigraphs: &[String]) -> String {
    if trigraphs.is_empty() {
        return String::new();
    }
    let mut deduped: Vec<String> = Vec::with_capacity(trigraphs.len());
    for t in trigraphs {
        if !deduped.contains(t) {
            deduped.push(t.clone());
        }
    }
    // After dedup the list is non-empty by virtue of the caller's
    // parser shape gate plus the early-return above; `rest` may be
    // empty (input was just `USA`), but `out` always starts with
    // `REL TO USA`, so no truncated partial output is possible.
    let mut rest: Vec<String> = deduped.into_iter().filter(|t| t != "USA").collect();
    rest.sort();
    let mut out = String::with_capacity(8 + 5 * (rest.len() + 1));
    out.push_str("REL TO USA");
    for code in rest {
        out.push_str(", ");
        out.push_str(&code);
    }
    out
}
