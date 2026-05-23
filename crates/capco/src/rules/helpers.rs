// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared helpers consumed by multiple rules.
//!
//! - [`sar_block_span`] — byte span covering a SAR block. Used by the
//!   banner walker.
//! - [`FixDiagnosticParams`] + [`make_fix_diagnostic`] — text-correction
//!   builder used by E006 / E007 / E008 / C001 plus the cross-module
//!   companion-emit path in `scheme/actions/companions.rs`.
//! - [`canonicalize_trigraph_list`] + [`dedup_country_codes`] — country-
//!   code list canonicalization used by E002 and S003.
//! - [`is_fgi_invalid_ownership_token`] — FGI ownership-token shape
//!   predicate used by E073 and the E008 suppression chain.

use std::collections::HashSet;

use marque_ism::{CanonicalAttrs, CountryCode, Span, TokenKind};
use marque_rules::{Confidence, Diagnostic, FixSource, Message, RuleId, Severity};
use marque_scheme::Citation;

use crate::scheme::CapcoScheme;

/// Compute the byte span covering the full SAR block: from the start of
/// its `SarIndicator` token through the end of the last SAR-constituent
/// token (`SarProgram` / `SarCompartment` / `SarSubCompartment`).
pub(crate) fn sar_block_span(attrs: &CanonicalAttrs) -> Option<Span> {
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    for tok in attrs.token_spans.iter() {
        let is_sar = matches!(
            tok.kind,
            TokenKind::SarIndicator
                | TokenKind::SarProgram
                | TokenKind::SarCompartment
                | TokenKind::SarSubCompartment
        );
        if !is_sar {
            continue;
        }
        if tok.kind == TokenKind::SarIndicator && start.is_none() {
            start = Some(tok.span.start);
        }
        let new_end = tok.span.end;
        end = Some(end.map_or(new_end, |e| e.max(new_end)));
    }
    match (start, end) {
        (Some(s), Some(e)) if e >= s => Some(Span::new(s, e)),
        _ => None,
    }
}

/// Bundle of all the inputs `make_fix_diagnostic` needs. Replaces a 9-arg
/// positional helper signature so call sites read top-down by name.
///
/// PR 3c.2.C C4 migrated `message: String` → `message: Message`
/// (closed-template, closed-args). PR 3c.2.C C5 migrated `citation:
/// &'static str` → `citation: Citation` per the atomic
/// `Diagnostic.citation` field-type flip. The `original` field is
/// retained on the struct so existing call sites stay byte-identical,
/// but the `make_fix_diagnostic` helper discards it per the existing
/// G13 invariant.
pub(crate) struct FixDiagnosticParams {
    pub rule: RuleId,
    pub severity: Severity,
    pub source: FixSource,
    pub span: Span,
    pub message: Message,
    pub citation: Citation,
    pub original: String,
    pub replacement: String,
    pub confidence: f32,
    pub migration_ref: Option<&'static str>,
}

/// Build a text-correction diagnostic from [`FixDiagnosticParams`].
///
/// Post PR 3c.B Commit 10 the engine's `apply_text_corrections`
/// reads `Diagnostic.text_correction` for the replacement bytes +
/// provenance. The helper preserves the legacy call shape and
/// faithfully threads `source`, `confidence`, and `migration_ref`
/// through to the `TextCorrection` payload — every rule that emits
/// a byte-substitution fix (C001 corrections-map, E006 deprecation
/// migration, and other [`make_fix_diagnostic`] callers) gets the
/// correct provenance on its audit record. The `original` field
/// is discarded (G13 closure on the legacy emission channel).
pub(crate) fn make_fix_diagnostic(p: FixDiagnosticParams) -> Diagnostic<CapcoScheme> {
    let _ = p.original; // G13: never copy document bytes into audit
    Diagnostic::text_correction(
        p.rule,
        p.severity,
        p.span,
        p.message,
        p.citation,
        p.replacement,
        p.source,
        Confidence::strict(p.confidence),
        p.migration_ref,
    )
}

/// Canonicalize a country code list. The `usa_first` flag selects the
/// convention:
///
/// - `usa_first = true` — REL TO convention per CAPCO-2016 §H.8 p151:
///   "After 'USA', list the required one or more trigraph country
///   codes in alphabetical order followed by tetragraph codes listed
///   in alphabetical order." USA is elevated to the front when
///   present; remaining codes are alphabetical.
/// - `usa_first = false` — JOINT convention per CAPCO-2016 §H.3 p56:
///   "Country trigraph codes are listed alphabetically followed
///   by tetragraph codes in alphabetical order." Pure alphabetical;
///   USA is NOT elevated.
///
/// Duplicates in the input are preserved as-is — this helper does
/// not deduplicate. Callers that need a fully canonical list (USA-
/// first + alphabetical + unique) compose [`dedup_country_codes`]
/// before this canonicalizer:
///
/// ```text
/// canonicalize_trigraph_list(&dedup_country_codes(codes), usa_first)
/// ```
///
/// E002 (REL TO, fix path) uses that composition so its fix
/// replacement is byte-canonical and stays single-pass idempotent.
///
/// The IC practice of rendering USA first in JOINT lists is widespread
/// but is convention, not CAPCO rule. The S003 `joint-usa-first`
/// style rule surfaces deviations; this helper does NOT encode the
/// convention into correctness.
///
/// This is the shared ordering rule for E002 (REL TO, fix path).
/// It gives E002 a single source of truth for the USA-first +
/// alphabetical invariant cited in §H.8 p151, mirroring what the
/// renderer's REL TO axis (`render_rel_to.rs`) produces at fix
/// time.
///
/// Visibility is `pub(crate)`: the decoder text-level path in
/// `marque-engine` does not call this helper directly — it operates
/// pre-strict-parse on raw text — and no other crate currently needs
/// it. Should a future consumer (e.g., a downstream formatter or a
/// programmatic API) need to canonicalize a `&[CountryCode]` list, it
/// should call through `marque-capco`'s public surface or this helper
/// can be promoted to `pub` at that point with an honest rationale.
///
/// Tetragraph partition handling is deferred — issue #183 PR-A
/// widened `CountryCode` so 4-byte tetragraphs are now first-class
/// entries in `attrs.rel_to`, but this helper still sorts the whole
/// list flat-alphabetically rather than the §H.3 p56 / §H.8 p151
/// "trigraphs alpha, then tetragraphs alpha" partition. Follow-up:
/// bucket true trigraphs (`code.len() == 3`) before everything else
/// (the 2-byte `EU`, the 4-byte tetragraphs, and 15-byte
/// `AUSTRALIA_GROUP` go in the non-trigraph bucket), or ideally
/// derive the buckets from the CVE schema groups in
/// `CVEnumISMCATRelTo.xsd`.
pub(crate) fn canonicalize_trigraph_list(codes: &[CountryCode], usa_first: bool) -> Vec<&str> {
    if usa_first {
        let has_usa = codes.contains(&CountryCode::USA);
        let mut sorted: Vec<&str> = codes
            .iter()
            .filter(|t| **t != CountryCode::USA)
            .map(|t| t.as_str())
            .collect();
        sorted.sort_unstable();
        if has_usa {
            sorted.insert(0, "USA");
        }
        sorted
    } else {
        let mut sorted: Vec<&str> = codes.iter().map(|t| t.as_str()).collect();
        sorted.sort_unstable();
        sorted
    }
}

/// Collapse duplicate country codes while preserving first-occurrence
/// order. Composed with [`canonicalize_trigraph_list`] inside E002's
/// fix path so its replacement is byte-canonical (USA-first +
/// alphabetical + unique).
///
/// **CAPCO authority**: §H.8 p151 specifies the REL TO list grammar
/// as "After 'USA', list the required one or more trigraph country
/// codes in alphabetical order followed by tetragraph codes listed
/// in alphabetical order." There is no textual prohibition of
/// duplicates — the rationale is structural: a list of country codes
/// describing a release decision is a set, and a duplicate is
/// redundant by construction. Mirrors the rationale block in
/// `try_rel_to_fuzzy_trigraph_candidates` (decoder side, issue #233)
/// for why duplicate-creating fuzzy candidates are filtered.
//
// Dead-code allow: the only remaining caller is the inline `mod tests`
// block quarantined to `_disabled_tests.rs` (`cfg(any())` pending the
// post-Commit-10 test rewrite, disposition #722). The helper retains
// its public-crate visibility because future rule emissions on the
// REL TO axis may consume it; removing it now would force a re-
// creation when those tests come back online.
#[allow(dead_code)]
pub(crate) fn dedup_country_codes(codes: &[CountryCode]) -> Vec<CountryCode> {
    let mut seen: HashSet<CountryCode> = HashSet::with_capacity(codes.len());
    let mut out: Vec<CountryCode> = Vec::with_capacity(codes.len());
    for &code in codes.iter() {
        if seen.insert(code) {
            out.push(code);
        }
    }
    out
}

/// Predicate: does this `Unknown` text span carry the FGI-marker shape
/// rejected by the strict parser?
///
/// True iff `text` starts with the FGI banner-abbreviation prefix
/// (`"FGI "`) or the long-form prefix
/// (`"FOREIGN GOVERNMENT INFORMATION "`) and at least one whitespace-
/// separated token in the tail fails the FGI-ownership shape gate
/// ([`CountryCode::admits_fgi_ownership_token`]).
///
/// Used by both the E073 walker (to emit one diagnostic per invalid
/// token) and the E008 suppression chain (to avoid co-firing a generic
/// "unrecognized token" Error on the same FGI-marker span — the user
/// sees only the actionable E073 diagnostic instead).
///
/// Authority: CAPCO-2016 §H.7 p123. The FGI Authorized Portion / Banner
/// forms define the ownership-token shape; this predicate is the
/// rule-layer surface of the parser's `parse_fgi_marker` rejection
/// path. Re-verified against `crates/capco/docs/CAPCO-2016.md` at
/// authorship per Constitution VIII.
pub(crate) fn is_fgi_invalid_ownership_token(text: &str) -> bool {
    let Some(tail) = text
        .strip_prefix("FGI ")
        .or_else(|| text.strip_prefix("FOREIGN GOVERNMENT INFORMATION "))
    else {
        return false;
    };
    // Forward-compat: the empty-tail branch (`"FGI "` followed only by
    // whitespace) is unreachable via the production parser path. The
    // block-walker trims input with `raw.trim()` before dispatch, so
    // `"FGI "` collapses to `"FGI"`, which `parse_fgi_marker` admits
    // as `FgiMarker::SourceConcealed` (no `Unknown` span is produced).
    // This branch covers synthetic `TokenKind::Unknown` spans (e.g.,
    // test-harness injection or out-of-tree consumers that bypass the
    // production parser) and any future parser change that allows an
    // empty-tail FGI to reach the rule layer. Keeping it preserves the
    // E073-owns-malformed-FGI invariant under those drift scenarios.
    let mut saw_token = false;
    for token in tail.split_whitespace() {
        saw_token = true;
        if !CountryCode::admits_fgi_ownership_token(token.as_bytes()) {
            return true;
        }
    }
    !saw_token
}
