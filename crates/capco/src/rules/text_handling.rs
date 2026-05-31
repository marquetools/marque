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
// Rule: Declassification instruction misplaced (belongs in CAB)
// ---------------------------------------------------------------------------

/// Fires when a declassification exemption or `Declassify On` date
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
/// rewriting rather than a local replacement. The rule surfaces the
/// diagnostic; the author resolves manually. The retirement target is a
/// document-scope `Recanonicalize` once `render_canonical` can position
/// declass in the CAB by construction. Authority: CAPCO-2016 §E.1 p31 +
/// §E.2 p32 (`Declassify On` is a CAB line — the single-value mandate
/// makes the position unambiguous) + §D.1 p27 (the banner category list
/// enumerates classification + control markings; declassification is
/// conspicuously absent — negative-inference). §E.3 p33's
/// commingling exemptions are CAB-line *content* rules (e.g., "N/A to RD/FRD/TFNI
/// portions"), not placement rules, and do not weaken the "declass
/// belongs in CAB" invariant.
///
/// This stays a hand-written `Rule` rather than a `Constraint::Custom`
/// catalog row because constraint predicates receive only
/// `&Self::Marking` — they have no access to `RuleContext.marking_type`,
/// so they could not reproduce the `Banner | Portion` gate below, and
/// would fire on every (well-formed) CAB candidate.
pub(super) struct DeclassifyMisplacedRule;

/// Secondary CAPCO §-citation for this rule.
///
/// The typed `Citation` on the emitted diagnostic carries one passage
/// (§E.1 p31). This constant pins the cross-reference to §D.1 p27 (banner
/// categories exclude declassification) structurally, so a regression
/// that loses the connection still fails a test rather than only mutating
/// a doc-comment.
///
/// §D.1 p27 enumerates the banner-line categories and conspicuously
/// excludes declassification, the negative-inference complement to
/// §E.1 p31's positive "Declassify On is a CAB line" rule.
///
/// The only consumer is the `citation_cross_refs_tests` module at the
/// bottom of this file (`#[cfg(test)]`-gated). The const is `pub(crate)`
/// so the test mod can reach it directly; under non-test builds,
/// `#[allow(dead_code)]` keeps the compiler quiet and the linker DCEs the
/// const at use-site (an unused `pub(crate) const` adds nothing to the
/// production binary, including the WASM-shipped surface).
#[allow(dead_code)] // used by `citation_cross_refs_tests` at end of file
pub(crate) const DECLASSIFY_MISPLACED_CROSS_REFS: &[Citation] = &[capco(SectionLetter::D, 1, 27)];

/// Citations this rule may emit on diagnostics. Combines the primary
/// `Diagnostic.citation` value (§E.1 p31) with the
/// [`DECLASSIFY_MISPLACED_CROSS_REFS`] cross-references. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate contract.
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
    fn check(
        &self,
        attrs: &CanonicalAttrs,
        ctx: &RuleContext<'_, CapcoScheme>,
    ) -> Vec<Diagnostic<CapcoScheme>> {
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

        // No fix: the repair is multi-span document-level rewriting.
        // See the doc comment on `DeclassifyMisplacedRule` for the
        // rationale and retirement target.
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
// Rule: X-shorthand declassification date
// ---------------------------------------------------------------------------

/// CAPCO X-shorthand declass codes (e.g., `25X1-`, `25X2-`, `50X1-`,
/// `50X1-HUM-`) are deprecated in favor of the canonical forms (`25X1`,
/// `50X1-HUM`, etc.). The deprecated dashed form is not in the CVE, so
/// the parser surfaces it as `TokenKind::Unknown`. This rule walks Unknown
/// tokens via two paths:
///
/// 1. **Migration table lookup**: exact match in the seed `MIGRATIONS`
///    table (e.g., `25X1-` → `25X1`, `50X1-` → `50X1-HUM`). The
///    authoritative replacement and policy reference are taken from the
///    table entry.
/// 2. **Pattern match** (fallback): any `TokenKind::Unknown` whose text
///    matches the `\d+X\d+(-[A-Z]+)?-` shape — i.e., a CAPCO
///    X-shorthand form with a trailing `-`. This catches forms the
///    seed table does not enumerate (e.g., `25X2-`, `25X5-`, `25X9-`).
///    The suggested replacement is the text with the trailing `-`
///    stripped.
pub(super) struct XShorthandDateRule;

/// Citations E007 may emit on diagnostics. See
/// [`Rule::cited_authorities`] for the corpus-fidelity gate
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
    fn check(
        &self,
        attrs: &CanonicalAttrs,
        _ctx: &RuleContext<'_, CapcoScheme>,
    ) -> Vec<Diagnostic<CapcoScheme>> {
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
                // Audit content-ignorance: original `text` and
                // `entry.replacement` do not flow into the typed
                // `Message`; the canonical replacement still rides on
                // `Diagnostic.text_correction.replacement`.
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                    citation: capco(SectionLetter::E, 6, 33),
                    original: text.to_owned(),
                    replacement: entry.replacement.to_owned(),
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
                // Audit content-ignorance: pattern-derived `replacement`
                // is a canonical form produced by deterministic
                // stripping (a permitted audit identifier). The typed
                // `Message` carries no args for this path — the template
                // label identifies the migration class.
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: self.id(),
                    severity: self.default_severity(),
                    source: FixSource::MigrationTable,
                    span: token.span,
                    message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
                    citation: capco(SectionLetter::E, 6, 33),
                    original: text.to_owned(),
                    replacement,
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
// Rule: Unrecognized token inside marking
// ---------------------------------------------------------------------------

/// Any token inside a marking candidate boundary that the parser
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
/// [`Rule::cited_authorities`] for the corpus-fidelity gate
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
    /// Phase::WholeMarking: no fix is emitted; diagnostics point at a
    /// single `Unknown` span but the firing decision reads cross-token
    /// state (`attrs.sar_markings.is_some()` to suppress repeated-SAR
    /// shapes E030 owns). Whole-marking is the conservative default.
    fn phase(&self) -> Phase {
        Phase::WholeMarking
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        UNKNOWN_TOKEN_AUTHORITIES
    }
    fn check(
        &self,
        attrs: &CanonicalAttrs,
        _ctx: &RuleContext<'_, CapcoScheme>,
    ) -> Vec<Diagnostic<CapcoScheme>> {
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
                    && !super::recanonicalize::is_bare_canonical_compound_form(text)
                    && !is_fgi_invalid_ownership_token(text)
            })
            .map(|t| {
                Diagnostic::new(
                    self.id(),
                    self.default_severity(),
                    t.span,
                    Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default()),
                    capco(SectionLetter::G, 1, 36),
                    None, // no fix offered for unrecognized tokens
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Rule: Corrections-map typo replacement
// ---------------------------------------------------------------------------

/// Scans token spans against the organization-specific corrections map from
/// `[corrections]` in `.marque.toml`. Each match produces a fix proposal with
/// `FixSource::CorrectionsMap` and `Recognition::strict()`.
///
/// # Not a CAPCO rule
///
/// This rule is intentionally NOT anchored to a CAPCO passage. No CAPCO
/// section governs user-defined typo replacements — they are
/// organization-specific mappings supplied through `.marque.toml`. The
/// citation [`marque_rules::CORRECTIONS_MAP_CITATION`] (`"CONFIG:[corrections]"`)
/// is a config pointer rather than a §/page reference. This is deliberate
/// and Constitution VIII-compliant: fabricating a CAPCO citation for a
/// user-defined mapping would be worse than no citation. Auditors
/// distinguish corrections-map fixes from CAPCO-authoritative fixes via
/// `FixSource::CorrectionsMap` in the audit record.
///
/// # Precedence over built-in rules
///
/// User corrections take precedence over built-in rules on the same span,
/// and this falls out of the engine's deterministic fix-ordering and
/// overlap guard rather than any special-case code.
///
/// # `migration_ref = None`
///
/// This rule emits `migration_ref: None`. `migration_ref` identifies a
/// deterministic ODNI migration-table entry (`FixSource::MigrationTable`)
/// — a user corrections map is not an ODNI migration, so there is no ref
/// to carry; the `FixSource` enum distinguishes provenance without a
/// string label.
///
/// # Emission paths
///
/// Two call sites emit corrections-map diagnostics:
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
/// and [`Rule::cited_authorities`] for the gate contract.
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
    /// The corrections map also runs as a separate pre-pass in
    /// `Engine::fix_inner` (text-correction Aho-Corasick scan against
    /// raw bytes before parsing). The phase tag governs the
    /// rule-dispatch path; that pre-pass is a separate channel that
    /// bypasses rule dispatch entirely.
    fn phase(&self) -> Phase {
        Phase::Localized
    }
    fn trusted(&self) -> bool {
        true
    }
    fn cited_authorities(&self) -> &'static [Citation] {
        CORRECTIONS_MAP_AUTHORITIES
    }
    fn check(
        &self,
        attrs: &CanonicalAttrs,
        ctx: &RuleContext<'_, CapcoScheme>,
    ) -> Vec<Diagnostic<CapcoScheme>> {
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
            // Audit content-ignorance: original document bytes (`text`)
            // and the user-config replacement (`replacement`) do not
            // flow into the typed `Message` — `MessageArgs.token` would
            // need a `TokenId` projection that does not exist for
            // arbitrary user-config `String → String` mappings. The
            // closed-template label identifies the corrections-map class;
            // the canonical replacement still rides on
            // `Diagnostic.text_correction.replacement` for the engine's
            // apply path.
            diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                rule: self.id(),
                severity: self.default_severity(),
                source: FixSource::CorrectionsMap,
                span: token_span.span,
                message: Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
                citation: marque_rules::CORRECTIONS_MAP_CITATION,
                original: text.to_owned(),
                replacement: replacement.clone(),
                migration_ref: None,
            }));
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// `looks_like_deprecated_x_shorthand` is the private predicate shared by
// the X-shorthand rule (emit) and the unrecognized-token rule (suppress),
// so the two cannot drift on which X-shorthand variants each owns. The
// colocated `mod tests` keeps the predicate private rather than widening
// visibility for test reach.
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
    /// `-` and must match here so the X-shorthand rule owns them (and the
    /// unrecognized-token rule suppresses on the same span).
    ///
    /// Authority: CAPCO-2016 §E.6 pp 33-34 (X-shorthand date-pattern
    /// migration). Verified against `crates/capco/docs/CAPCO-2016.md`.
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
