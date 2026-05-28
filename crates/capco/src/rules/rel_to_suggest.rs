// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`RelToTrigraphSuggestRule`] — REL TO trigraph fuzzy-match
//! Suggest rule.
//!
//! First consumer of the suggest-don't-fix channel; surfaces
//! corpus-rare trigraphs with a corpus-common neighbor within edit
//! distance. The wire predicate ID lives on `RuleId::new(...)` — the
//! single source of truth.

use marque_ism::{CanonicalAttrs, TokenKind, TokenSpan};
use marque_rules::{
    Confidence, Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase, Rule,
    RuleContext, RuleId, Severity,
};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: S004 — rel-to-trigraph-suggest (suggest-don't-fix channel)
// ---------------------------------------------------------------------------

/// S004: Surface a suggest-channel hint when a REL TO trigraph entry
/// is corpus-rare AND has a corpus-common neighbor within edit
/// distance 2.
///
/// # Authority and scope
///
/// Per CAPCO-2016 §H.8 p150 (REL TO grammar: Authorized Banner Line
/// Marking Title, Authorized Portion Mark) and §H.8 p151 (REL TO
/// "[USA, LIST]" syntax — "Register, Annex B trigraph country
/// codes"), REL TO entries are drawn from the CAPCO Register Annex
/// B trigraph code list. Every entry in `attrs.rel_to` has already
/// passed the strict-grammar trigraph check; the rule does not
/// invalidate any of them. The signal here is **statistical**:
/// `AUT` (Austria, ISO 3166-1 alpha-3) is a legitimate trigraph but
/// appears two orders of magnitude less often in real REL TO blocks
/// than `AUS` (Australia), and the two are 1 substitution apart.
/// When a low-prior entry has a high-prior 1- or 2-edit neighbor,
/// the entry might be correct (Austria really IS the recipient) or
/// might be a typo (`AUT` → `AUS`). The rule cannot tell which —
/// hence the suggest channel: emit a candidate replacement, do not
/// auto-apply.
///
/// # Severity
///
/// `Suggest` by default. The engine never auto-applies a fix
/// attached to a `Severity::Suggest` diagnostic regardless of
/// `confidence`, so the candidate replacement stays informational.
///
/// # Predicate
///
/// For each `CountryCode` in `attrs.rel_to`:
///
/// 1. Look up the entry's `country_code_log_prior`. Skip if absent
///    (decoder fallback is not in scope here — S004 only fires on
///    parsed-and-priored trigraphs).
/// 2. Iterate the corpus's country-code priors table. Find the
///    highest-prior code at edit distance 1 (or 2 for 3-letter
///    trigraphs only) from the entry, where the prior delta vs the
///    entry exceeds [`SUGGEST_LOG_MARGIN`].
/// 3. If such a neighbor exists, emit a `Severity::Suggest`
///    diagnostic via `Diagnostic::text_correction` whose
///    `replacement` is the neighbor. `Severity::Suggest` is a hard
///    exclusion from auto-apply regardless of confidence value.
///
/// # Coverage of #186 ambiguous fixtures
///
/// - `USB` → decoder PR-A (#238) handles. USB is not a trigraph; it
///   never reaches `attrs.rel_to`. S004 is silent.
/// - `AUT` → S004 fires, suggesting `AUS`.
///   `log_prior(AUS) - log_prior(AUT)` ≈ 4.36 nats, above
///   [`SUGGEST_LOG_MARGIN`].
/// - `ASU` → the decoder handles. ASU is not a trigraph; never
///   reaches `attrs.rel_to`.
/// - `SA` → 2-character non-trigraph; same as USB / ASU, not in
///   `attrs.rel_to`. Decoder/parser path.
///
/// # Coverage exclusion (issue #439)
///
/// If the candidate replacement trigraph is **already covered** by
/// another entry in the same `attrs.rel_to` block — either directly
/// (the other entry equals the candidate) or transitively (the other
/// entry is a decomposable tetragraph whose
/// [`expand_tetragraph`](crate::vocab::expand_tetragraph) members
/// contain the candidate) — S004 stays silent for that entry. The
/// author's `AUT` cannot be a typo for `AUS` if `FVEY` (or `ACGU`, or
/// a direct `AUS`) already covers Australia in the same block:
/// `AUS` is *already* a permitted recipient, so duplicating it as
/// `AUT` would have produced redundant content rather than a typo.
/// The remaining hypothesis is "the author meant Austria"; S004
/// respects that and emits nothing.
///
/// The check is general over the ODNI ISMCAT Tetragraph Taxonomy —
/// `FVEY`, `ACGU`, `NATO`, `AUSTRALIA_GROUP`, and any other
/// `decomposable="Yes"` row are all consulted via the same table.
/// Atomic tetragraphs (`decomposable="No"` — `EU`, `GCCH`, `KFOR`,
/// …), deprecated entries (`decomposable="NA"`), and codes unknown
/// to both the taxonomy and `country_extensions.toml` return
/// `None` from `expand_tetragraph` and therefore cannot suppress
/// the diagnostic.
///
/// Authority: CAPCO-2016 §D.2 Table 3 Row 23 pp28–30 explicitly
/// licenses tetragraph-to-trigraph expansion for banner-line REL TO
/// roll-up — "Expansion of the TEYE, ACGU, and FVEY tetragraphs is
/// allowed for common country roll-up of banner line REL TO [USA,
/// LIST] marking". §H.8 p151 (REL TO Precedence Rules for Banner
/// Line Guidance) delegates roll-up semantics to §D.2 Table 3 by
/// reference. The suppression operationalizes that already-licensed
/// equivalence: if `FVEY` is in the block, its expanded members are
/// already permitted recipients of the same banner-rolled-up release
/// decision, so a "did you mean a member of that expansion?"
/// suggestion against a different rare trigraph is corpus noise.
/// The data source for the expansion (the ODNI ISMCAT
/// `decomposable="Yes"` rows) is described under
/// [`expand_tetragraph`](crate::vocab::expand_tetragraph).
///
/// # Constitution V audit-content-ignorance
///
/// The diagnostic carries a closed [`MessageTemplate`] (no
/// string-bearing arg by design — see `crates/rules/src/message.rs`),
/// a typed [`Citation`], and a `text_correction` replacement that is
/// a bare closed-vocabulary trigraph (the candidate neighbor). No
/// document content, no surrounding span text, no user-provided
/// fields, and no free-form formatted prose appear on any emitted
/// surface. Verified by `s004_audit_content_ignorance` in
/// `crates/capco/tests/`.
///
/// # Reuse for #206
///
/// Issue #206 (REL TO opaque-uncertain reduction) wants the same
/// rendering channel without a candidate replacement: emit
/// `Severity::Suggest` with `fix: None`. The engine and renderer
/// both handle the missing-fix case cleanly (verified by
/// `s004_suggest_with_no_fix_round_trips_renderer`). #206 will land
/// as a separate rule that constructs `Diagnostic { severity:
/// Suggest, fix: None, .. }` directly.
pub(super) struct RelToTrigraphSuggestRule;

/// Minimum log-prior delta for S004 to suggest a neighbor over the
/// observed entry. `4.0` nats ≈ `e^4.0` ≈ 55× odds ratio — the
/// neighbor is at least 55× more likely than the observed entry in
/// real REL TO contexts. Empirically calibrated against the AUT/AUS
/// pair (delta ≈ 4.36) so the canonical #186 fixture fires while
/// closer pairs (e.g., `USA`/`UKR` at delta ≈ 1.2 if it were ever
/// triggered) do not.
pub(crate) const SUGGEST_LOG_MARGIN: f32 = 4.0;

/// Compute Levenshtein edit distance between two byte slices.
///
/// Trigraphs are short (≤ 3 bytes for the S004 use case) so the
/// O(m*n) two-row DP allocates two `Vec<usize>` of size `≤ 4` per
/// call — negligible. Inlined here rather than depending on
/// `marque-engine` (which `marque-capco` does not depend on).
pub(crate) fn s004_edit_distance(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Issue #439: returns `true` when `candidate` (the trigraph the
/// caller would suggest) is already covered by some other entry in
/// `block` — either directly (another entry equals `candidate`) or
/// transitively (another entry is a tetragraph whose
/// [`expand_tetragraph`](crate::vocab::expand_tetragraph) members
/// contain `candidate`). Generic over the ODNI ISMCAT
/// `decomposable="Yes"` rows; atomic and unknown entries return
/// `None` from `expand_tetragraph` and therefore never cover.
///
/// The `self_idx` parameter excludes the rare entry itself from
/// the scan — `expand_tetragraph` returns `None` for a trigraph
/// like `AUT`, but skipping the self-index avoids both the lookup
/// and any future-edit pitfall if the table grows to include
/// trigraph rows.
///
/// # Naming
///
/// Originally `s004_candidate_covered_by_block(rel_to: &[CountryCode], ...)`.
/// Issue #545's `FgiOwnershipTrigraphSuggestRule` reuses the same
/// coverage-exclusion semantics on an FGI ownership list (a
/// `&[CountryCode]` from `attrs.fgi_marker.countries()`), so the
/// parameter was renamed `block` and the function generalized.
/// Both S004 (`attrs.rel_to`) and the FGI rule (`fgi_marker.countries()`)
/// pass their respective country-list slice; the helper is
/// shape-agnostic.
pub(crate) fn s004_candidate_covered_by_block(
    block: &[marque_ism::CountryCode],
    candidate: &str,
    self_idx: usize,
) -> bool {
    block.iter().enumerate().any(|(i, code)| {
        if i == self_idx {
            return false;
        }
        let s = code.as_str();
        if s == candidate {
            return true;
        }
        crate::vocab::expand_tetragraph(s).is_some_and(|members| members.contains(&candidate))
    })
}

/// Citations S004 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate
/// contract.
const AUTHORITIES: &[Citation] = &[capco(SectionLetter::H, 8, 150)];

impl Rule<CapcoScheme> for RelToTrigraphSuggestRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.dissem.rel-to-trigraph-suggest")
    }
    fn name(&self) -> &'static str {
        "rel-to-trigraph-suggest"
    }
    fn default_severity(&self) -> Severity {
        Severity::Suggest
    }
    /// Phase::Localized: each emitted `Diagnostic::text_correction`
    /// replaces a single `RelToTrigraph` token with a corpus-derived
    /// canonical trigraph (e.g. `GRB → GBR`). Span is one token.
    /// `Severity::Suggest` means the engine never auto-promotes, but
    /// the phase declaration governs dispatch even for suggest-only
    /// rules.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use crate::priors::{COUNTRY_CODE_BASE_RATES, country_code_log_prior};

        if attrs.rel_to.is_empty() {
            return Vec::new();
        }

        // Build a lookup from CountryCode → its `RelToTrigraph` token
        // span so we can attach the diagnostic to the exact source
        // bytes the user typed. Per-CountryCode mapping is positional:
        // the parser emits one `RelToTrigraph` token per `rel_to` entry
        // in source order.
        let trigraph_spans: Vec<&TokenSpan> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::RelToTrigraph)
            .collect();

        let mut diagnostics = Vec::new();
        for (idx, code) in attrs.rel_to.iter().enumerate() {
            let trigraph = code.as_str();
            // Only operate on 3-letter trigraphs. 2-letter codes (EU)
            // and longer codes (FVEY, AUSTRALIA_GROUP) have a different
            // ambiguity profile and would need their own calibration.
            if trigraph.len() != 3 {
                continue;
            }
            let Some(entry_log_prior) = country_code_log_prior(trigraph) else {
                continue;
            };

            // Find the highest-prior neighbor within edit distance 2.
            // Iterating the full `COUNTRY_CODE_BASE_RATES` table is
            // O(n) but the table is bounded (~340 codes) and the rule
            // fires once per `rel_to` entry. Acceptable.
            //
            // The triple `(token, log_prior, dist)` is what the
            // tie-breaking ladder reads — distance is tracked so a
            // log-prior tie deterministically picks the shorter-edit
            // candidate, and a same-distance tie picks the
            // lexicographically smaller token. Corpus-derived priors
            // tie exactly only when two entries share a build-time
            // smoothing floor, but pinning the order makes the rule's
            // output reproducible across `COUNTRY_CODE_BASE_RATES`
            // table reorderings.
            let mut best: Option<(&'static str, f32, usize)> = None;
            for cand in COUNTRY_CODE_BASE_RATES {
                if cand.token == trigraph {
                    continue;
                }
                if cand.token.len() != 3 {
                    continue;
                }
                if cand.log_prior - entry_log_prior < SUGGEST_LOG_MARGIN {
                    // Neighbor isn't substantially more likely — skip.
                    continue;
                }
                let dist = s004_edit_distance(trigraph, cand.token);
                if dist == 0 || dist > 2 {
                    continue;
                }
                // Pick the higher-prior candidate. On a log-prior
                // tie, prefer the shorter edit distance; on a
                // distance tie too, fall back to lexicographic
                // order on the token. Each rung of the ladder is a
                // strict comparison so the resolution is total.
                let take = match best {
                    None => true,
                    Some((prev_token, prev_prior, prev_dist)) => {
                        if cand.log_prior > prev_prior {
                            true
                        } else if cand.log_prior < prev_prior {
                            false
                        } else if dist < prev_dist {
                            true
                        } else if dist > prev_dist {
                            false
                        } else {
                            cand.token < prev_token
                        }
                    }
                };
                if take {
                    best = Some((cand.token, cand.log_prior, dist));
                }
            }

            let Some((candidate, _candidate_log_prior, _candidate_dist)) = best else {
                continue;
            };

            // Issue #439: skip when the candidate replacement is
            // already covered by another entry in the same REL TO
            // block (direct trigraph match OR transitive coverage via
            // a decomposable tetragraph like FVEY / ACGU / NATO /
            // AUSTRALIA_GROUP). The author cannot have meant the
            // candidate trigraph as a typo target when it's already
            // a permitted recipient — the rare entry is either
            // intentional or a typo for something else entirely, and
            // S004's signal in that regime is at its weakest.
            if s004_candidate_covered_by_block(&attrs.rel_to, candidate, idx) {
                continue;
            }

            // Pull the matching span. If the parser's RelToTrigraph
            // tokens don't match `rel_to.len()` (defensive against a
            // future parser change), skip rather than emit a
            // misaligned diagnostic.
            let Some(span_token) = trigraph_spans.get(idx) else {
                continue;
            };
            let span = span_token.span;

            // S004 suggests a trigraph swap (corpus-derived
            // canonical replacement, no fact-set delta). Encode as
            // a `text_correction` diagnostic. Even though
            // `apply_text_corrections` filters by C001, S004 emits
            // at `Severity::Suggest` so the engine's auto-apply
            // path correctly excludes it (the engine's Suggest
            // exclusion is a hard channel-cutoff). The text
            // correction carries the canonical trigraph for
            // renderer / UI display.
            //
            // Audit content-ignorance (Constitution V): the diagnostic's
            // human-facing prose lives in the closed `MessageTemplate`
            // enum — there is no string-bearing template arg, by
            // design — so the candidate trigraph rides only on the
            // `text_correction` replacement (a closed-vocabulary
            // identifier), never as a formatted free-form string.
            diagnostics.push(Diagnostic::text_correction(
                self.id(),
                self.default_severity(),
                span,
                Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
                capco(SectionLetter::H, 8, 150),
                candidate.to_owned(),
                FixSource::BuiltinRule,
                Confidence::strict(),
                None,
            ));
        }

        diagnostics
    }
}
