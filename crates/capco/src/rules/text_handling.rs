// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Text-handling rules — declassification placement, X-shorthand
//! migration, unknown-token surfacing, and corrections-map typo
//! replacements.
//!
//! - [`DeclassifyMisplacedRule`] — declassify-on token misplaced in
//!   portion or banner.
//! - [`XShorthandDateRule`] — deprecated `X1` / `X2` date shorthand
//!   migration.
//! - [`UnknownTokenRule`] — generic unrecognized-token surface.
//! - [`CorrectionsMapRule`] — user-configured `[corrections]` map
//!   typo replacements.
//!
//! Predicate IDs live on each rule's `RuleId::new(...)` — the wire
//! string is the single source of truth.

use marque_ism::generated::migrations::find_migration;
use marque_ism::{CanonicalAttrs, Span, TokenKind};
use marque_rules::{
    Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Phase, Rule, RuleContext, RuleId,
    Severity,
};
use marque_scheme::{Citation, SectionLetter, capco};

use super::dissem::is_dissem_replacement;
use super::helpers::{FixDiagnosticParams, is_fgi_invalid_ownership_token, make_fix_diagnostic};
use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Rule: E005 — Declassification instruction misplaced (belongs in CAB)
// ---------------------------------------------------------------------------

/// E005 fires when a declassification exemption or `Declassify On` date
/// appears inside a banner or portion marking rather than the Classification
/// Authority Block (CAB).
///
/// # Authority
///
/// Two CAPCO-2016 passages together establish the invariant:
///
/// - **§E.1 p31** enumerates `Declassify On` as a CAB line and lists its
///   valid values: YYYYMMDD dates, events, `25X#`, `50X#`, `75X#`,
///   `50X1-HUM`, `50X2-WMD`, `25X1, EO 12951`, and the `N/A …` forms.
///   This is the authoritative "declass values live here" list.
///   §E.2 p32 reaffirms it for derivative classification: "Only a single
///   value must be used on the `Declassify On` line of the classification
///   authority block."
/// - **§D.1 p27** enumerates the banner syntax's permitted categories —
///   classification, SCI, SAP, AEA, Dissem, Non-IC Dissem. Declassification
///   is **not** on this closed list, and §C.1 p26 lines 525ff gives
///   portions the same category set. A declass token appearing between
///   `//` separators of a banner or portion is unambiguously misplaced.
///
/// The invariant is safely broader than CAPCO's OCA (§E.1 p31) vs
/// derivative (§E.2 p32) distinctions — both place `Declassify On` in the
/// CAB, so the predicate does not branch on classification source.
///
/// # Scope
///
/// Fires on `MarkingType::Banner` and `MarkingType::Portion`. Explicitly
/// does NOT fire on `MarkingType::Cab` — that is the correct location for
/// declass info and a CAB candidate carrying `declassify_on` /
/// `declass_exemption` is well-formed, not violating.
///
/// # Fix
///
/// None. Repairing a misplaced declass marking requires moving the token
/// from the banner/portion into a CAB, which is multi-span document-level
/// rewriting rather than a local replacement. E005 surfaces the
/// diagnostic; the author resolves manually.
// ---------------------------------------------------------------------------
// Migration status (PR 3c.B Sub-PR 9, 2026-05-11): provisional Path A
// per `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md` D4.
// E005 stays as a hand-written `Rule` impl in this file; it does NOT
// migrate to a `Constraint::Custom` catalog row on `CapcoScheme` in this
// PR.
//
// Retirement target: `Recanonicalize { scope: Scope::Document }` on the
// `MarkingScheme` trait surface, once `render_canonical` (deferred per
// `architecture.md` §"What this commits us to") can position declass in
// the Classification Authority Block (CAB) by construction. Authority:
// CAPCO-2016 §E.1 p31 + §E.2 p32 (`Declassify On` is a CAB line — the
// single-value mandate makes the position unambiguous) + §D.1 p27 (the
// banner category list enumerates classification + control markings;
// declassification is conspicuously absent — negative-inference). §E
// commingling exemptions at pp 33-34 are CAB-line *content* rules (e.g.,
// "N/A to RD/FRD/TFNI portions"), not placement rules, and do not weaken
// the "declass belongs in CAB" invariant.
//
// Structural blocker (why Path A in PR 3c.B Commit 9):
// `MarkingScheme::evaluate_custom` (crates/scheme/src/scheme.rs:124-130)
// receives only `&Self::Marking`. It has no access to
// `RuleContext.marking_type`, so a constraint-catalog predicate cannot
// reproduce the existing `Banner | Portion` gate (lines below). Without
// that gate, the predicate would fire on every CAB candidate — declass
// in a CAB is the correct location, not a violation. The trait-surface
// extension that would unblock this migration is tracked in
// `specs/006-engine-rule-refactor/followups/constraint-context-extension.md`.
//
// `Diagnostic::with_fix(..., None)` constructor: this rule emits
// neither a legacy `FixProposal` nor a structural `FixIntent<S>` because
// the repair is multi-span document-level rewriting (move the declass
// token from banner/portion into a CAB). The constructor swap (vs the
// `Diagnostic::new(..., None)` form) signals consciously-decided deferred
// migration evaluation, matching the PR #349 pattern for E016/E036.
// Downstream audit consumers observe no behavioral difference: both
// constructors leave `fix: None` and `fix_intent: None`.
pub(super) struct DeclassifyMisplacedRule;

/// E005 secondary CAPCO §-citations.
///
/// PR 10.A.1 Commit 4: the migration to typed `Citation` collapsed the
/// pre-migration string form `"CAPCO-2016 §E.1 p31 + §D.1 p27"` into a
/// single `capco(SectionLetter::E, 1, 31)` value on the emitted
/// diagnostic (typed `Citation` carries one passage). The cross-reference
/// to §D.1 p27 (banner categories exclude declassification) survived in
/// the rule's doc-comment but was un-checked — a rename or removal of
/// the comment wouldn't trip a test. This constant pins the dropped
/// cross-reference structurally so a regression that loses the §D.1 p27
/// connection still fails a test.
///
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1
/// Commit 4 authorship per Constitution VIII propagation rule:
/// §D.1 p27 enumerates the banner-line categories and conspicuously
/// excludes declassification, the negative-inference complement to
/// §E.1 p31's positive "Declassify On is a CAB line" rule.
///
/// The constant is rule-authoritative metadata intended for runtime
/// introspection by a future PR 10.A.2 `Rule::cited_authorities()`
/// trait method (deferred per the PR brief). Today the only consumer
/// is the `citation_cross_refs_tests` module at the bottom of this
/// file (`#[cfg(test)]`-gated, parallel to but not conflated with the
/// `#[cfg(any())]`-gated inline `mod tests` that's dead code pending a
/// separate rewrite). The const is `pub(crate)` so the test mod can
/// reach it directly; under non-test builds, the `#[allow(dead_code)]`
/// keeps the compiler quiet and the linker DCEs the const at use-site
/// (consts in Rust are inlined; an unused `pub(crate) const` does not
/// add to the production binary footprint, including the WASM-shipped
/// crate surface).
#[allow(dead_code)] // used by `citation_cross_refs_tests` at end of file
pub(crate) const DECLASSIFY_MISPLACED_CROSS_REFS: &[Citation] = &[capco(SectionLetter::D, 1, 27)];

/// Citations E005 may emit on diagnostics. Combines the primary
/// `Diagnostic.citation` value (§E.1 p31) with the
/// [`DECLASSIFY_MISPLACED_CROSS_REFS`] cross-references. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const DECLASSIFY_MISPLACED_AUTHORITIES: &[Citation] = &[
    capco(SectionLetter::E, 1, 31),
    capco(SectionLetter::D, 1, 27),
];

impl Rule<CapcoScheme> for DeclassifyMisplacedRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.declassification.declassify-on-misplaced")
    }
    fn name(&self) -> &'static str {
        "declassify-misplaced"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: no auto-fix; flags declass-token placement
    /// at document scope (move into the CAB). Decision reads across the
    /// banner/portion/CAB axes.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        DECLASSIFY_MISPLACED_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;
        // Fire on banner AND portion. CAB candidates are the correct
        // location for declass info and must be skipped. PageBreak is
        // not a marking and carries no attributes.
        if !matches!(ctx.marking_type, MarkingType::Banner | MarkingType::Portion) {
            return vec![];
        }
        if attrs.declassify_on.is_none() && attrs.declass_exemption.is_none() {
            return vec![];
        }

        // Span: whichever declass-related token is present.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| matches!(t.kind, TokenKind::DeclassExemption | TokenKind::DeclassDate))
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        // PR 3c.B Sub-PR 9: provisional Path A — `with_fix_intent(..., None)`
        // signals consciously-decided deferred migration evaluation. See the
        // migration-status block above `struct DeclassifyMisplacedRule;` for
        // the full rationale and retirement target.
        //
        // Citation: §E.1 p31 governs the "Declassify On is a CAB line"
        // rule; §D.1 p27 affirms banner categories do not include
        // declassification. The typed `Citation` field anchors at §E.1
        // p31; the cross-reference to §D.1 p27 lives in the doc-comment
        // above this rule (the typed-Citation struct carries one
        // §-citation per Diagnostic).
        vec![Diagnostic::with_fix(
            self.id(),
            self.default_severity(),
            span,
            Message::new(MessageTemplate::WrongTokenForm, MessageArgs::default()),
            capco(SectionLetter::E, 1, 31),
            None, // Fix requires document-level context (moving a token
                  // from banner/portion into a CAB is multi-span).
        )]
    }
}

// ---------------------------------------------------------------------------
// Rule: E007 — X-shorthand declassification date
// ---------------------------------------------------------------------------

/// CAPCO X-shorthand declass codes (e.g., `25X1-`, `25X2-`, `50X1-`,
/// `50X1-HUM-`) are deprecated in favor of the canonical forms (`25X1`,
/// `50X1-HUM`, etc.). The deprecated dashed form is not in the CVE, so
/// the parser surfaces it as `TokenKind::Unknown`. E007 walks Unknown
/// tokens via two paths:
///
/// 1. **Migration table lookup**: exact match in the seed `MIGRATIONS`
///    table (e.g., `25X1-` → `25X1`, `50X1-` → `50X1-HUM`). This path
///    uses the table's authoritative confidence and reference.
/// 2. **Pattern match** (fallback): any `TokenKind::Unknown` whose text
///    matches the `\d+X\d+(-[A-Z]+)?-` shape — i.e., a CAPCO
///    X-shorthand form with a trailing `-`. This catches forms the
///    seed table does not enumerate (e.g., `25X2-`, `25X5-`, `25X9-`).
///    The suggested replacement is the text with the trailing `-`
///    stripped; confidence is 0.95 (slightly lower than the 0.97 used
///    for table-backed matches to reflect the lack of an authoritative
///    replacement mapping).
pub(super) struct XShorthandDateRule;

/// Citations E007 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const X_SHORTHAND_DATE_AUTHORITIES: &[Citation] = &[capco(SectionLetter::E, 6, 33)];

impl Rule<CapcoScheme> for XShorthandDateRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "portion.metadata.x-shorthand-date-pattern")
    }
    fn name(&self) -> &'static str {
        "x-shorthand-date"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::Localized: each fix rewrites a single `Unknown` token in
    /// place — either a migration-table hit or a pattern-stripped
    /// `25X1-` → `25X1` style derivation. Span is the token the rule
    /// walked.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        X_SHORTHAND_DATE_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let mut diagnostics = Vec::new();
        for token in attrs.token_spans.iter() {
            if token.kind != TokenKind::Unknown {
                continue;
            }
            let text = token.text.as_ref();

            // Path 1: exact migration-table match. Uses the table's
            // authoritative replacement and reference. Skips entries
            // owned by E006 (dissem deprecations).
            if let Some(entry) = find_migration(text) {
                if is_dissem_replacement(entry.replacement) {
                    continue;
                }
                // G13: original `text` and `entry.replacement` do not
                // flow into the typed `Message`; the canonical
                // replacement still rides on `Diagnostic.text_correction.replacement`.
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                    citation: capco(SectionLetter::E, 6, 33),
                    original: text.to_owned(),
                    replacement: entry.replacement.to_owned(),
                    confidence: entry.confidence,
                    migration_ref: Some(entry.reference),
                }));
                continue;
            }

            // Path 2: pattern match for X-shorthand forms not in the
            // seed migration table (e.g., `25X2-`, `25X5-`, `25X9-`).
            // Strip the trailing `-` to produce the canonical form.
            if looks_like_deprecated_x_shorthand(text) {
                let replacement = text.trim_end_matches('-').to_owned();
                if replacement.is_empty() {
                    continue;
                }
                // G13: pattern-derived `replacement` is on the audit
                // permitted list (canonical form, deterministic
                // stripping). The typed `Message` carries no args
                // for this path — the template label identifies the
                // migration class.
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                    citation: capco(SectionLetter::E, 6, 33),
                    original: text.to_owned(),
                    replacement,
                    // 0.95: slightly below table-backed 0.97 because
                    // the canonical form is derived by pattern stripping
                    // rather than an authoritative CVE mapping.
                    confidence: 0.95,
                    migration_ref: None,
                }));
            }
        }
        diagnostics
    }
}

/// Returns `true` if `s` looks like a DEPRECATED CAPCO X-shorthand
/// declassification form — specifically a canonical form with a
/// trailing `-`.
///
/// Matched patterns:
/// - `NNXNN-`             (e.g., `25X1-`, `25X2-`, `50X1-`)
/// - `NNXNN-AAA-`         (e.g., `50X1-HUM-`, `25X9-WMD-`)
///
/// The canonical (modern) forms (`25X1`, `50X1-HUM`) are in the CVE and
/// parse as `DeclassExemption`, so they never reach this function via
/// the `TokenKind::Unknown` walk.
///
/// Used by both E007 (to emit) and E008 (to skip) so the two rules
/// cannot drift on which tokens each owns.
fn looks_like_deprecated_x_shorthand(s: &str) -> bool {
    let bytes = s.as_bytes();
    // Must end with `-`.
    if bytes.last() != Some(&b'-') {
        return false;
    }
    let inner = &bytes[..bytes.len() - 1];
    if inner.is_empty() {
        return false;
    }
    let mut i = 0;
    // Leading digits.
    while i < inner.len() && inner[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i >= inner.len() {
        return false;
    }
    // `X` separator.
    if inner[i] != b'X' {
        return false;
    }
    i += 1;
    // One or more digits after `X`.
    let start_digits = i;
    while i < inner.len() && inner[i].is_ascii_digit() {
        i += 1;
    }
    if i == start_digits {
        return false;
    }
    // Optional `-LETTERS` suffix (e.g., `-HUM`, `-WMD`).
    if i == inner.len() {
        return true;
    }
    if inner[i] != b'-' {
        return false;
    }
    i += 1;
    while i < inner.len() {
        if !inner[i].is_ascii_uppercase() {
            return false;
        }
        i += 1;
    }
    true
}

/// Whether an `Unknown` token matches the repeated-SAR shape that E008
/// suppresses in favor of E030.
///
/// This helper intentionally implements only the subset of checks needed
/// here — a cheap, string-only predicate on the `Unknown` token itself:
///   - A first SAR parsed successfully (`attrs.sar_markings.is_some()`).
///   - The Unknown text starts with `SAR-` or `SPECIAL ACCESS REQUIRED-`.
///   - The suffix after the prefix is non-empty.
///
/// `SarIndicatorRepeatRule::check` applies additional gates before it
/// emits (preceding-Separator lookup, byte-contiguity between the
/// separator and the Unknown token). Those gates are kept inside E030
/// — when they fail E030 emits a no-fix diagnostic so the shape is
/// still surfaced to the user rather than being silently dropped. This
/// helper therefore does NOT need to model them.
///
/// When any of this helper's checks fails, E008 must fire — the token
/// is not something E030 treats as a repeated-SAR shape. Without this
/// gate, a malformed first SAR like `SAR-` (empty program) would be
/// silently dropped: E030 early-exits on `sar_markings.is_none()`, and
/// E008's old prefix-only suppression would swallow the token.
fn is_repeated_sar_owned_by_e030(text: &str, has_first_sar: bool) -> bool {
    if !has_first_sar {
        return false;
    }
    let suffix = if let Some(rest) = text.strip_prefix("SAR-") {
        rest
    } else if let Some(rest) = text.strip_prefix("SPECIAL ACCESS REQUIRED-") {
        rest
    } else {
        return false;
    };
    !suffix.is_empty()
}

// ---------------------------------------------------------------------------
// Rule: E008 — Unrecognized token inside marking
// ---------------------------------------------------------------------------

/// FR-012: any token inside a marking candidate boundary that the parser
/// could not classify is reported as an error with no fix offered.
///
/// Authority: CAPCO-2016 §G.1 (Register of Authorized Markings, p36):
/// "All markings used in a banner line and portion mark must be in
/// accordance with the values listed in the Register, unless a waiver
/// has been obtained from P&S/IMD in accordance with ICD 710 and
/// applicable ICS." Any token not matching a Register entry (or an
/// Annex A/B code, or a structurally-valid SCI/SAR/REL TO composition)
/// is by definition unauthorized and must be surfaced.
///
/// Suppression paths (an `Unknown` that hits any is NOT unrecognized —
/// another rule owns it):
///
/// 1. **Migration-table hit** — deprecated forms like `25X1-` that
///    `crates/ism/build.rs` MIGRATIONS captures. E007 (X-shorthand)
///    or E006 (migrated-dissem) fires instead.
/// 2. **X-shorthand pattern** — any `\d+X\d+(-[A-Z]+)?-` shape the
///    seed table does not enumerate (e.g., `25X2-`, `25X9-`). E007
///    catches these via its pattern fallback.
/// 3. **Repeated SAR block** — when a first SAR parsed successfully
///    into `attrs.sar_markings`, the parser tags every subsequent
///    same-marking SAR block as `Unknown` whose text starts with
///    `SAR-` or `SPECIAL ACCESS REQUIRED-` AND has a non-empty
///    suffix. E030 (sar-indicator-repeat) owns those; E008 steps
///    aside. The suppression predicate matches the token-shape
///    preconditions `SarIndicatorRepeatRule::check` keys on: it
///    only applies when `attrs.sar_markings.is_some()` and the
///    stripped SAR suffix is non-empty, so a malformed FIRST SAR
///    block — which leaves `sar_markings = None` or has an empty
///    suffix — still fires E008. Without this tightening a marking
///    like `SECRET//SAR-` would be silently dropped: the first SAR
///    fails grammar (no `SarMarking` produced), E008's old
///    prefix-only suppression matched anyway, and E030 early-exited
///    on its `attrs.sar_markings.is_none()` gate. Note E030 also
///    applies a byte-contiguity gate between the Unknown token and
///    its preceding separator; this helper does not model that gate
///    because E030 emits a no-fix diagnostic when contiguity fails,
///    so the shape is still surfaced to the user.
///
/// Malformed SCI-shaped tokens the structural subparser rejected
/// (e.g., `SI-`, `SI--G`) DO fire E008 — users see a real error,
/// not a silent fallback.
pub(super) struct UnknownTokenRule;

/// Citations E008 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the F.1 corpus-fidelity gate
/// contract.
const UNKNOWN_TOKEN_AUTHORITIES: &[Citation] = &[capco(SectionLetter::G, 1, 36)];

impl Rule<CapcoScheme> for UnknownTokenRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.metadata.unrecognized-token")
    }
    fn name(&self) -> &'static str {
        "unrecognized-token"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
    /// Phase::WholeMarking: no fix is emitted (FR-012); diagnostics
    /// point at a single `Unknown` span but the firing decision reads
    /// cross-token state (`attrs.sar_markings.is_some()` to suppress
    /// repeated-SAR shapes E030 owns). Default to whole-marking per
    /// D-7.2 — the dispatch consequence is conservative.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        UNKNOWN_TOKEN_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Precompute whether a first SAR block parsed successfully. The
        // repeated-SAR suppression path below must only fire when E030's
        // own token-shape preconditions are met; otherwise a malformed
        // FIRST SAR block would be silently dropped (E030 early-exits,
        // E008 suppresses). The relevant gates inside
        // `SarIndicatorRepeatRule::check` are the `attrs.sar_markings
        // .is_none()` early-exit and the `stripped.is_empty()` skip.
        let has_first_sar = attrs.sar_markings.is_some();
        attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown)
            // Skip entries that E006/E007/E030 will pick up. Three paths:
            //   1. Migration-table hit (covers LIMDIS/FOUO for E006 and
            //      25X1-/50X1- for E007).
            //   2. Pattern-matched X-shorthand with a trailing `-` for
            //      forms not in the seed table (25X2-, 25X9-, etc.).
            //   3. A repeated SAR category block — but ONLY when a
            //      first SAR succeeded AND the stripped suffix is
            //      non-empty (E030's actual preconditions). A
            //      malformed first SAR like `SAR-` (empty suffix)
            //      must still fire E008, not be silently swallowed.
            // An Unknown that hits any path is not "unrecognized" — it
            // is a deprecated or structurally-owned form another rule
            // will surface.
            .filter(|t| {
                let text = t.text.as_ref();
                // Note: malformed SCI-shaped tokens (e.g., `SI-`, `SI--G`)
                // that the structural subparser rejected DO fire E008 —
                // the user sees a real diagnostic instead of a silent
                // fallback. Only suppress well-known specialized paths.
                //
                // Issue #407: bare canonical compound forms (CNWDI / NK /
                // EU in SCI position) are owned by E067
                // (`BareCanonicalCompoundRule`); suppress E008 co-fire
                // so the user sees only the actionable E067
                // `text_correction` diagnostic, not a redundant
                // "unrecognized token" Error.
                //
                // Issue #501: invalid FGI ownership tokens (e.g.,
                // `"FGI FVEY"`, `"FGI DEUX"`, `"FOREIGN GOVERNMENT
                // INFORMATION ACGU"`) are owned by E073
                // (`FgiInvalidOwnershipTokenRule`); suppress E008 co-
                // fire so the user sees only the actionable, category-
                // specific E073 diagnostic.
                find_migration(text).is_none()
                    && !looks_like_deprecated_x_shorthand(text)
                    && !is_repeated_sar_owned_by_e030(text, has_first_sar)
                    && !crate::rules_declarative::is_bare_canonical_compound_form(text)
                    && !is_fgi_invalid_ownership_token(text)
            })
            .map(|t| {
                Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    t.span,
                    Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default()),
                    capco(SectionLetter::G, 1, 36),
                    None, // FR-012: no fix offered
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Rule: C001 — Corrections-map typo replacement
// ---------------------------------------------------------------------------

/// Scans token spans against the organization-specific corrections map from
/// `[corrections]` in `.marque.toml`. Each match produces a fix proposal with
/// `FixSource::CorrectionsMap` and `confidence = 1.0`.
///
/// # Not a CAPCO rule
///
/// C001 is intentionally NOT anchored to a CAPCO passage. No CAPCO section
/// governs user-defined typo replacements — they are organization-specific
/// mappings supplied through `.marque.toml`. The citation string
/// [`marque_rules::CORRECTIONS_MAP_CITATION`] (`"CONFIG:[corrections]"`) is
/// a config pointer rather than a §/page/line reference. This is deliberate
/// and Constitution VIII-compliant: fabricating a CAPCO citation for a
/// user-defined mapping would be worse than no citation. Auditors
/// distinguish C001 fixes from CAPCO-authoritative fixes via
/// `FixSource::CorrectionsMap` in the audit record.
///
/// # FR-009 precedence (spec: `specs/001-marque-mvp/spec.md` §Functional
/// Requirements, FR-009)
///
/// User corrections take precedence over built-in rules on the same span.
/// This is automatic under FR-016 sort order — `"C001" < "E001"`
/// lexicographically, so C001 wins under the C-1 overlap guard. No
/// special-case code in the engine; the invariant falls out of the sort
/// key alone. Exercised by
/// `fr009_c001_wins_over_builtin_rule_on_same_span` in
/// `crates/capco/tests/corrections_map.rs`.
///
/// # `migration_ref = None`
///
/// C001 emits `migration_ref: None`. `migration_ref` identifies a
/// deterministic migration-table entry (FR-004a, `FixSource::MigrationTable`)
/// — C001 is a user map, not an ODNI migration, so there is no ref to
/// carry. PR #6 review explicitly rejected the earlier
/// `Some("corrections-map")` placeholder; the `FixSource` enum already
/// distinguishes provenance without a string label.
///
/// # Emission paths
///
/// Two call sites emit C001 diagnostics:
/// 1. This rule's `check` method — triggered when the scanner detected a
///    marking and the parser produced a `TokenSpan` whose text matches a
///    corrections key.
/// 2. `Engine::lint` pre-scanner text scan — triggered when the scanner
///    missed a marking (e.g., `SERCET//NF` whose classification prefix is
///    not recognized). Both paths use
///    [`marque_rules::CORRECTIONS_MAP_CITATION`] so the audit record shape
///    is identical.
pub(super) struct CorrectionsMapRule;

/// Citations `CorrectionsMapRule` may emit on diagnostics. The
/// rule is **not** a CAPCO rule — it surfaces user-defined
/// `[corrections]` map entries, so its citation is the
/// [`AuthoritativeSource::Config`] sentinel (`[config]`) rather than
/// a §/page reference. See [`marque_rules::CORRECTIONS_MAP_CITATION`]
/// and [`Rule::cited_authorities`] for the F.1 gate contract.
const CORRECTIONS_MAP_AUTHORITIES: &[Citation] = &[marque_rules::CORRECTIONS_MAP_CITATION];

impl Rule<CapcoScheme> for CorrectionsMapRule {
    fn id(&self) -> RuleId {
        RuleId::new("capco", "marking.correction.token-typo")
    }
    fn name(&self) -> &'static str {
        "corrections-map"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    /// Phase::Localized: each fix replaces a single `TokenSpan` with the
    /// user-configured `[corrections]` mapping (e.g. `SERCET → SECRET`).
    /// Span is strictly one token.
    ///
    /// Architecturally C001 also runs as a separate pre-pass-0 in
    /// `Engine::fix_inner` (text-correction Aho-Corasick scan against
    /// raw bytes before parsing — `docs/refactor-006/pr-7-architect-plan.md`
    /// §3.5). The phase tag governs the rule-dispatch path; the
    /// pre-pass-0 path is a separate channel that bypasses rule
    /// dispatch entirely.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        CORRECTIONS_MAP_AUTHORITIES
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Engine guarantees corrections is Some only when the map is non-empty
        // (engine.rs: corrections_arc is None when config.corrections.is_empty()).
        let Some(corrections) = ctx.corrections.as_ref() else {
            return vec![];
        };

        let mut diagnostics = Vec::new();
        for token_span in attrs.token_spans.iter() {
            // M1: skip structural separators — corrections never apply to "//"
            if token_span.kind == TokenKind::Separator {
                continue;
            }
            let text = token_span.text.as_str();
            let Some(replacement) = corrections.get(text) else {
                continue;
            };
            // M2: skip no-op corrections (replacement == original)
            if replacement == text {
                continue;
            }
            // G13: drop the runtime byte text from the message per
            // PM-C-5. Original document bytes (`text`) and the
            // user-config replacement (`replacement`) do not flow into
            // the typed `Message` — `MessageArgs.token` would need a
            // `TokenId` projection that does not exist for arbitrary
            // user-config `String → String` mappings. The closed-template
            // label identifies the corrections-map class; the canonical
            // replacement still rides on `Diagnostic.text_correction.replacement`
            // for the engine's apply path.
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::CorrectionsMap,
                span: token_span.span,
                message: Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
                citation: marque_rules::CORRECTIONS_MAP_CITATION,
                original: text.to_owned(),
                replacement: replacement.clone(),
                confidence: 1.0,
                migration_ref: None,
            }));
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Issue #722 — ported from quarantined `_disabled_tests.rs`.
//
// `looks_like_deprecated_x_shorthand` is the private predicate shared by
// E007 (emit) and E008 (suppress), so the two rules cannot drift on
// which X-shorthand variants each owns. Colocated `mod tests` is the
// correct port destination per `feedback_pub_doc_hidden_is_still_public_api`
// — widening visibility for test reach is forbidden.
// ---------------------------------------------------------------------------
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    /// Pin the `looks_like_deprecated_x_shorthand` pattern set. The
    /// canonical (modern) declass-exemption forms (`25X1`, `50X1-HUM`)
    /// live in the ODNI ISM CVE vocabulary and parse as
    /// `DeclassExemption`; they never reach this helper via the
    /// `TokenKind::Unknown` walk. The deprecated forms carry a trailing
    /// `-` and must match here so E007 owns them (and E008 suppresses
    /// on the same span — see `is_x_shorthand_for_suppression` in
    /// this module).
    ///
    /// Authority: CAPCO-2016 §E.6 pp 33-34 (X-shorthand date-pattern
    /// migration). Re-verified against `crates/capco/docs/CAPCO-2016.md`
    /// at authorship per Constitution VIII.
    #[test]
    fn looks_like_deprecated_x_shorthand_matches_expected_patterns() {
        let m = looks_like_deprecated_x_shorthand;
        // Deprecated forms (must match).
        assert!(m("25X1-"));
        assert!(m("25X2-"));
        assert!(m("25X9-"));
        assert!(m("50X1-"));
        assert!(m("50X1-HUM-"));
        assert!(m("25X3-WMD-"));
        // Canonical forms (must NOT match — no trailing dash).
        assert!(!m("25X1"));
        assert!(!m("50X1-HUM"));
        // Malformed / unrelated.
        assert!(!m(""));
        assert!(!m("-"));
        assert!(!m("X1-"));
        assert!(!m("25-X1-"));
        assert!(!m("25X-"));
        assert!(!m("ABCX1-"));
        assert!(!m("25X1-hum-"), "lowercase suffix should not match");
        assert!(!m("NOFORN"));
    }
}
