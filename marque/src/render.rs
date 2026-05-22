// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Diagnostic rendering for the `marque` CLI.
//!
//! Two formats are supported:
//! - **human**: rustc-style diagnostic — a header line with location and
//!   rule identifier, followed by a source snippet with a `|` gutter, the
//!   offending line(s), and a `^^^` caret line pointing at the span.
//!   ANSI-colored by default.
//! - **json**: NDJSON conforming to `contracts/diagnostic.json`.
//!
//! ANSI is suppressed when any of these is true:
//! - `--no-color` was passed on the command line
//! - `NO_COLOR` env var is set to a non-empty value
//! - `TERM=dumb`
//! - stdout is not a terminal
//!
//! The contract for stream usage:
//! - `check` writes diagnostics to **stdout**.
//! - Operator narration goes to **stderr** (suppressible via `-q`).
//!
//! # Human output shape
//!
//! ```text
//! banner.txt:1:17 error[E001] banner uses abbreviated dissem control "NF"; use "NOFORN"
//!   --> banner.txt:1:17-19
//!    |
//!  1 | TOP SECRET//SI//NF
//!    |                 ^^ replace with "NOFORN"
//!    |
//!    = citation: CAPCO-2016 §A.6
//! ```
//!
//! The line-number gutter width auto-sizes to the largest line number in
//! the diagnostic. For multi-line spans, the caret line extends to the
//! end of the first line of the span; additional lines are not rendered
//! (CAPCO markings are always single-line so this is a corner-case).

use marque_capco::CapcoScheme;
use marque_engine::{AUDIT_SCHEMA_IS_V2_0, AUDIT_SCHEMA_VERSION, LintResult};
use marque_rules::audit::{AppliedTextCorrection, AuditLine, discriminant_from_source};
use marque_rules::{Diagnostic, RuleId};
use marque_scheme::{TokenSource, Vocabulary};
use serde::Serialize;
use std::path::Path;

/// JSON projection of a [`RuleId`] as a `{scheme, predicate_id}` 2-tuple
/// object (T044 PM OD-2 structured-object shape; `contracts/audit-record.md`
/// "Post-`marque-1.0` RuleId migration" §128-176).
///
/// Borrows the two `&'static str` segments out of the `RuleId` without
/// allocation. CLI text output uses the `Display` wire-string form
/// (`<scheme>:<predicate_id>`); JSON output uses this structured object
/// so audit-log consumers can filter on `scheme` directly (e.g.,
/// `rule.scheme == "engine"` to surface engine-internal records).
///
/// Mirrored on the WASM side by `crates/wasm/src/lib.rs::RuleIdJson` for
/// byte-identical NDJSON parity (SC-008).
#[derive(Debug, Serialize)]
pub struct RuleIdJson<'a> {
    pub scheme: &'a str,
    pub predicate_id: &'a str,
}

impl<'a> From<&'a RuleId> for RuleIdJson<'a> {
    fn from(r: &'a RuleId) -> Self {
        Self {
            scheme: r.scheme(),
            predicate_id: r.predicate_id(),
        }
    }
}

/// Output format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Human,
    Json,
}

/// Branding suffix appended to every human-format diagnostic header line.
///
/// Only applied on the pretty-printed terminal output path. Structured
/// outputs — NDJSON (`diagnostic_to_json`), JSON audit records
/// (`applied_fix_to_audit_json`), and `contracts/diagnostic.json`-schema
/// consumers — keep `Diagnostic::message` byte-identical so downstream
/// tooling (CI annotations, editor plugins, the planned ABAC
/// cross-checker) does not have to strip brand text back out.
const BRAND_SUFFIX: &str = "—Marque";

/// Effective color mode after honoring `--no-color`, `NO_COLOR`, `TERM=dumb`,
/// and TTY detection.
pub fn use_color(no_color_flag: bool) -> bool {
    if no_color_flag {
        return false;
    }
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return false;
    }
    if std::env::var("TERM").as_deref() == Ok("dumb") {
        return false;
    }
    use is_terminal::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Pick a default format when the user did not pass `--format`.
/// JSON for non-TTY (pipelines), human for TTY.
pub fn default_format() -> Format {
    use is_terminal::IsTerminal;
    if std::io::stdout().is_terminal() {
        Format::Human
    } else {
        Format::Json
    }
}

/// Render a single diagnostic in rustc-style human format. Caller writes to
/// stdout. Produces 6+ lines per diagnostic: header, location arrow, source
/// snippet with line number and caret, citation footer.
pub fn render_human(
    out: &mut dyn std::io::Write,
    path_label: &str,
    source: &[u8],
    diag: &Diagnostic<CapcoScheme>,
    color: bool,
) -> std::io::Result<()> {
    let (line, col_start) = byte_to_line_col(source, diag.span.start);
    let (end_line, col_end_raw) = byte_to_line_col(source, diag.span.end);
    // For multi-line spans, clamp the caret to the end of the first line.
    // CAPCO markings are single-line so this is a defensive clamp.
    let col_end = if end_line == line {
        col_end_raw
    } else {
        // end of source line: look up the line bytes and use its length + 1
        extract_line(source, line)
            .map(|l| l.len() + 1)
            .unwrap_or(col_start + 1)
    };

    let level = level_str(diag.severity);
    // Suggest gets a distinct color (BoldYellow) from Error/Warn (BoldRed)
    // so the suggest-don't-fix channel reads as a hint, not a problem.
    let level_style = match diag.severity {
        marque_rules::Severity::Suggest => AnsiStyle::BoldYellow,
        _ => AnsiStyle::BoldRed,
    };
    let level_styled = paint(color, level_style, level);
    let rule_styled = paint(color, AnsiStyle::Bold, &format!("[{}]", diag.rule));

    // ---- Header line ----
    // banner.txt:1:17 error[E001] banner uses abbreviated dissem control "NF"; use "NOFORN" —Marque
    //
    // Branding suffix is appended centrally here (human render path only),
    // never at Diagnostic::new() and never in NDJSON / JSON output — those
    // are consumed by tooling (CI scripts, editor plugins, ABAC consumers)
    // that should not have to strip branding text out of the `message`
    // field.
    // PR 3c.2.C C5: `diag.message` is now a typed `Message` with no
    // `Display` impl by design. Render the closed-template label;
    // future renderer expansion can derive richer human text from
    // `(template, args, source, span)` per PM-C-5.
    writeln!(
        out,
        "{path_label}:{line}:{col_start} {level_styled}{rule_styled} {} {}",
        diag.message.template().as_str(),
        BRAND_SUFFIX,
    )?;

    // ---- Source snippet ----
    //   --> path:line:col_start-col_end
    //    |
    //  N | <source line>
    //    |   ^^^^ hint
    //    |
    //    = citation: ...
    let line_num_str = line.to_string();
    let gutter_width = line_num_str.len();
    let gutter = " ".repeat(gutter_width);
    let arrow = paint(color, AnsiStyle::BoldBlue, "-->");
    let pipe = paint(color, AnsiStyle::BoldBlue, "|");
    let eq = paint(color, AnsiStyle::BoldBlue, "=");

    writeln!(
        out,
        "{gutter} {arrow} {path_label}:{line}:{col_start}-{col_end}"
    )?;

    if let Some(line_bytes) = extract_line(source, line) {
        if let Ok(line_text) = std::str::from_utf8(line_bytes) {
            // Blank gutter line above the source snippet
            writeln!(out, "{gutter} {pipe}")?;
            // Source line with line number gutter
            let line_num_styled = paint(color, AnsiStyle::BoldBlue, &line_num_str);
            writeln!(out, "{line_num_styled} {pipe} {line_text}")?;
            // Caret line: spaces to col_start-1, then carets spanning col_start..col_end
            let caret_pad_width = col_start.saturating_sub(1);
            let caret_width = col_end.saturating_sub(col_start).max(1);
            let caret_pad = " ".repeat(caret_pad_width);
            let carets = "^".repeat(caret_width);
            let carets_styled = paint(color, AnsiStyle::BoldRed, &carets);
            // For Suggest-severity diagnostics, the "fix" is a
            // candidate hint rather than a confirmed replacement;
            // surface the wording difference so the reader doesn't
            // think it will be auto-applied.
            //
            // Post Commit 10 the renderer cannot show the exact
            // replacement bytes here — `FixIntent` carries the
            // structural intent only, and the renderer is on the
            // diagnostic path (no engine projection available). C001
            // text-correction diagnostics still carry their canonical
            // replacement bytes via `text_correction`, so those
            // render the legacy "replace with X" form.
            let hint = if let Some(tc) = &diag.text_correction {
                match diag.severity {
                    marque_rules::Severity::Suggest => {
                        format!(" did you mean {:?}?", tc.replacement.as_str())
                    }
                    _ => format!(" replace with {:?}", tc.replacement.as_str()),
                }
            } else if let Some(f) = diag.fix.as_ref() {
                match diag.severity {
                    marque_rules::Severity::Suggest => format!(
                        " (suggested fix; confidence {:.0}%)",
                        f.confidence.combined() * 100.0
                    ),
                    _ => format!(
                        " (auto-fixable; confidence {:.0}%)",
                        f.confidence.combined() * 100.0
                    ),
                }
            } else {
                String::new()
            };
            writeln!(out, "{gutter} {pipe} {caret_pad}{carets_styled}{hint}")?;
            // Blank gutter line below the caret
            writeln!(out, "{gutter} {pipe}")?;
        }
    }

    // Citation footer
    writeln!(out, "{gutter} {eq} citation: {}", diag.citation)?;

    Ok(())
}

/// Extract the bytes of the 1-indexed `line_num` from `source`, excluding
/// the trailing `\n` (and `\r` for CRLF). Returns `None` if the source
/// doesn't have that many lines.
fn extract_line(source: &[u8], line_num: usize) -> Option<&[u8]> {
    let mut current_line = 1;
    let mut line_start = 0;
    for (i, &b) in source.iter().enumerate() {
        if b == b'\n' {
            if current_line == line_num {
                // Strip trailing `\r` for CRLF line endings.
                let end = if i > line_start && source[i - 1] == b'\r' {
                    i - 1
                } else {
                    i
                };
                return Some(&source[line_start..end]);
            }
            current_line += 1;
            line_start = i + 1;
        }
    }
    // EOF without trailing `\n` — the last line is everything from
    // `line_start` to the end.
    if current_line == line_num {
        return Some(&source[line_start..]);
    }
    None
}

fn level_str(severity: marque_rules::Severity) -> &'static str {
    match severity {
        marque_rules::Severity::Error => "error",
        marque_rules::Severity::Warn => "warning",
        marque_rules::Severity::Info => "info",
        marque_rules::Severity::Suggest => "suggest",
        marque_rules::Severity::Fix => "fix",
        marque_rules::Severity::Off => "off", // unreachable in practice
    }
}

/// ANSI style selectors for the human renderer. Keeping the set small
/// avoids a dependency on `owo-colors` or similar — the escape codes are
/// inlined in `paint`.
#[derive(Debug, Clone, Copy)]
enum AnsiStyle {
    BoldRed,
    BoldYellow,
    BoldBlue,
    Bold,
}

fn paint(color: bool, style: AnsiStyle, text: &str) -> String {
    if !color {
        return text.to_owned();
    }
    let (prefix, suffix) = match style {
        AnsiStyle::BoldRed => ("\x1b[31;1m", "\x1b[0m"),
        AnsiStyle::BoldYellow => ("\x1b[33;1m", "\x1b[0m"),
        AnsiStyle::BoldBlue => ("\x1b[34;1m", "\x1b[0m"),
        AnsiStyle::Bold => ("\x1b[1m", "\x1b[0m"),
    };
    format!("{prefix}{text}{suffix}")
}

/// JSON projection of a Diagnostic conforming to `contracts/diagnostic.json`.
/// Marked `additionalProperties: false` in the schema, so this struct must
/// not include extra fields.
///
/// PR 3c.2.C C5 changed the `message` and `citation` fields' wire
/// shape per PM-C-7:
/// - `message` is now a structured object `{ "template": "..." }`
///   (was a free-form string). Future iterations expand `args`.
/// - `citation` is now the [`Display`] form of typed [`Citation`]
///   — `§<L>.<sub> p<page>` for CAPCO sources, `[config]` /
///   `[engine-internal]` for sentinel sources.
#[derive(Debug, Serialize)]
pub struct DiagnosticJson<'a> {
    /// 2-tuple `RuleId` shape per T044 PM OD-2 (structured-object
    /// `{scheme, predicate_id}`). Pre-T044 this was a flat
    /// `"rule": "E001"` string; post-T044 it is
    /// `"rule": {"scheme": "capco", "predicate_id": "..."}`.
    pub rule: RuleIdJson<'a>,
    pub severity: &'a str,
    pub span: SpanJson,
    pub message: MessageJson<'a>,
    pub citation: String,
    pub fix: Option<FixJson<'a>>,
}

#[derive(Debug, Serialize)]
pub struct SpanJson {
    pub start: usize,
    pub end: usize,
}

/// Structured JSON projection of a [`Message`].
///
/// PR 3c.2.C C5 introduced this wrapper per PM-C-7's structured-JSON
/// shape requirement. Phase 1 carries the [`MessageTemplate::as_str`]
/// canonical label; per-template arg expansion lands when consumers
/// need it.
#[derive(Debug, Serialize)]
pub struct MessageJson<'a> {
    pub template: &'a str,
}

#[derive(Debug, Serialize)]
pub struct FixJson<'a> {
    pub source: &'static str,
    /// Structural intent kind (`"FactAdd"` / `"FactRemove"` /
    /// `"Recanonicalize"`) for rule-emitted fixes; `"TextCorrection"`
    /// for the C001 path. Replaces the legacy `replacement: &str`
    /// field — the structural intent has no byte-precise replacement
    /// at the diagnostic boundary.
    pub intent_kind: &'static str,
    /// Canonical replacement bytes for text-correction diagnostics
    /// (C001). `None` for structural rule fixes; the engine renders
    /// them from the per-page projection at promotion time.
    pub replacement: Option<&'a str>,
    pub confidence: f32,
    pub migration_ref: Option<&'a str>,
}

pub fn diagnostic_to_json(d: &Diagnostic<CapcoScheme>) -> DiagnosticJson<'_> {
    DiagnosticJson {
        rule: (&d.rule).into(),
        severity: d.severity.as_str(),
        span: SpanJson {
            start: d.span.start,
            end: d.span.end,
        },
        message: MessageJson {
            template: d.message.template().as_str(),
        },
        citation: d.citation.to_string(),
        fix: match (d.fix.as_ref(), d.text_correction.as_ref()) {
            (Some(f), _) => Some(FixJson {
                source: fix_source_str(f.source),
                intent_kind: intent_kind_str(&f.replacement),
                replacement: None,
                confidence: f.confidence.combined(),
                migration_ref: f.migration_ref,
            }),
            (None, Some(tc)) => Some(FixJson {
                source: fix_source_str(tc.source),
                intent_kind: "TextCorrection",
                replacement: Some(tc.replacement.as_str()),
                confidence: tc.confidence.combined(),
                migration_ref: tc.migration_ref,
            }),
            (None, None) => None,
        },
    }
}

/// Write the full lint result as NDJSON (one record per line) to stdout.
pub fn render_ndjson(out: &mut dyn std::io::Write, result: &LintResult) -> std::io::Result<()> {
    for d in &result.diagnostics {
        let json = serde_json::to_string(&diagnostic_to_json(d)).map_err(std::io::Error::other)?;
        out.write_all(json.as_bytes())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Write the full lint result in human format.
pub fn render_human_result(
    out: &mut dyn std::io::Write,
    path_label: &str,
    source: &[u8],
    result: &LintResult,
    color: bool,
) -> std::io::Result<()> {
    for d in &result.diagnostics {
        render_human(out, path_label, source, d, color)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Audit record NDJSON (marque-2.0)
//
// The `schema` field is sourced from `marque_engine::AUDIT_SCHEMA_VERSION`,
// which `crates/engine/build.rs` validates against the closed accept-list
// `["marque-2.0"]` (was `["marque-1.0"]` pre-T044). PR 3c.2.D retired the
// pre-cutover `mvp-1` / `mvp-2` / `mvp-3` shapes atomically with the v2
// `AppliedFix` reshape, BLAKE3 digesting, closed `MessageTemplate` JSON
// serialization, and `Canonical<S>` provenance wiring, to close the G13
// audit-content-ignorance channel structurally. T044 then bumped to
// `marque-2.0` alongside the `RuleId` 2-tuple migration.
//
// FR-014 (single-schema-per-build) is upheld at two layers:
//   1. `crates/engine/build.rs` panics on unknown values — only the
//      accepted version can reach the emitter.
//   2. The emitter chooses the matching struct shape from the const.
// ---------------------------------------------------------------------------

fn fix_source_str(source: marque_rules::FixSource) -> &'static str {
    match source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
        marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
        marque_rules::FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

/// Schema-pinned string projection of a `ReplacementIntent` variant
/// discriminator. The enum is `#[non_exhaustive]` so a wildcard arm
/// is unavoidable; the helper logs a tracing warning on unknown
/// variants so an unrecognized addition surfaces operationally
/// (rather than the audit / diagnostic JSON silently emitting
/// "Unknown").
fn intent_kind_str(intent: &marque_scheme::ReplacementIntent<CapcoScheme>) -> &'static str {
    match intent {
        marque_scheme::ReplacementIntent::FactAdd { .. } => "FactAdd",
        marque_scheme::ReplacementIntent::FactRemove { .. } => "FactRemove",
        marque_scheme::ReplacementIntent::Recanonicalize { .. } => "Recanonicalize",
        _ => {
            tracing::warn!(
                target: "marque::render",
                "unrecognized ReplacementIntent variant in audit projection; downstream consumers will see kind=\"Unknown\""
            );
            "Unknown"
        }
    }
}

// ---------------------------------------------------------------------------
// `marque-1.0` audit-record JSON projection (PR 3c.2.D)
//
// Per `specs/006-engine-rule-refactor/contracts/audit-record.md` body §
// (active spec post-cutover). The CLI emit path reads
// `FixResult.audit_lines` via `render_audit_line`.
//
// SC-008 invariant: byte-identical NDJSON output between the CLI
// (`marque/src/render.rs`) and WASM (`crates/wasm/src/lib.rs`) emitters.
// The two crates carry parallel struct definitions deliberately (per
// architect D-D-1 — shared `marque-audit-render` crate deferred to
// post-PR-10). The WASM-side mirror in `crates/wasm/src/lib.rs` MUST
// stay byte-identical; `crates/wasm/tests/audit_v1_0_parity.rs`
// pins this at integration-test time.
// ---------------------------------------------------------------------------

/// JSON projection of a `marque-1.0` `AppliedFix<CapcoScheme>` audit
/// record. Top-level outer shape per contract §107-178.
///
/// PM-D-2: `input` stays at top level (architect D-D-2 ratified); the
/// contract example does not show it but the field is structurally
/// peer-level to `timestamp` / `classifier_id` / `dry_run`.
#[derive(Debug, Serialize)]
pub struct AuditRecordJsonV1_0<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub schema: &'static str,
    /// 2-tuple `RuleId` per T044 PM OD-2. See [`RuleIdJson`].
    pub rule: RuleIdJson<'a>,
    pub severity: &'static str,
    pub span: SpanJson,
    pub fix: AuditFixJson<'a>,
    pub message: AuditMessageJson<'a>,
    pub timestamp: String,
    /// Classifier identity. Emitted as `null` rather than elided when
    /// absent so audit consumers can detect "classifier_id field
    /// expected but not configured" deterministically — same shape as
    /// v3 (mvp-3) emitted to preserve consumer behavior across the
    /// schema flip. PM-D-2 ratifies `classifier_id` stays at top level.
    pub classifier_id: Option<&'a str>,
    pub dry_run: bool,
    /// Caller-supplied input identifier (file path / `-` for stdin).
    /// Emitted as `null` when absent for the same audit-consumer
    /// stability rationale as `classifier_id`.
    pub input: Option<&'a str>,
}

/// `AppliedFixDetail<S>` projection per contract §123-152 (`fix`
/// sub-object).
#[derive(Debug, Serialize)]
pub struct AuditFixJson<'a> {
    pub replacement: AuditReplacementJson<'a>,
    pub original_span: SpanJson,
    pub original_digest: String,
}

/// `AppliedReplacement<S>` projection per contract §124-148
/// (`fix.replacement` sub-object).
#[derive(Debug, Serialize)]
pub struct AuditReplacementJson<'a> {
    pub discriminant: &'static str,
    pub canonical: AuditCanonicalJson<'a>,
    pub confidence: AuditConfidenceJson<'a>,
}

/// `Canonical<S>` projection per contract §253-291.
///
/// The `source` discriminator is `"cve"` for [`TokenSource::Cve`] and
/// `"open_vocab"` for [`TokenSource::OpenVocab`]. The two arms emit
/// different optional fields per contract §259-286.
#[derive(Debug, Serialize)]
pub struct AuditCanonicalJson<'a> {
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_id: Option<std::borrow::Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_call_site: Option<String>,
    pub bytes_digest: String,
}

/// `Confidence` projection per contract §140-147.
#[derive(Debug, Serialize)]
pub struct AuditConfidenceJson<'a> {
    pub recognition: f32,
    pub rule: f32,
    pub combined: f32,
    pub region: Option<f32>,
    pub runner_up_ratio: Option<f32>,
    pub features: Vec<AuditFeatureJson<'a>>,
}

/// `FeatureContribution` projection per contract §146 (closed-set
/// `FeatureId` labels).
#[derive(Debug, Serialize)]
pub struct AuditFeatureJson<'a> {
    pub id: &'a str,
    pub delta: f32,
}

/// `Message` projection per contract §154-166.
///
/// `template` is the closed [`MessageTemplate::as_str`] wire form
/// (per PM-D-12 the contract example is illustrative — the variant
/// name verbatim is the wire form). `args` is the closed
/// [`MessageArgs`] permitted-set per contract §316-323; empty /
/// `None` fields elide so the JSON object is minimal.
#[derive(Debug, Serialize)]
pub struct AuditMessageJson<'a> {
    pub template: &'static str,
    pub args: serde_json::Map<String, serde_json::Value>,
    // Lifetime-binding marker so the renderer can compose the args
    // map from borrowed token labels without requiring an explicit
    // `'static` bound on every projection.
    #[serde(skip)]
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// JSON projection of an [`AppliedTextCorrection`] audit record.
///
/// The text-correction NDJSON line type per contract §388-402. Carries
/// a corpus-derived canonical replacement string (Constitution V
/// Principle V permitted identifier: a `SmolStr` token canonical, never
/// document content).
#[derive(Debug, Serialize)]
pub struct TextCorrectionRecordJsonV1_0<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub schema: &'static str,
    /// 2-tuple `RuleId` per T044 PM OD-2. See [`RuleIdJson`].
    pub rule: RuleIdJson<'a>,
    pub severity: &'static str,
    pub span: SpanJson,
    pub original_digest: String,
    pub replacement: &'a str,
    pub source: &'static str,
    pub confidence: AuditConfidenceJson<'a>,
    /// Citation reference; `None` for C001 corrections-map matches,
    /// `Some(...)` for E006-style deprecation migrations.
    pub migration_ref: Option<&'a str>,
    pub message: AuditMessageJson<'a>,
    pub timestamp: String,
    /// Same null-emit semantics as [`AuditRecordJsonV1_0::classifier_id`].
    pub classifier_id: Option<&'a str>,
    pub dry_run: bool,
    /// Same null-emit semantics as [`AuditRecordJsonV1_0::input`].
    pub input: Option<&'a str>,
}

/// Format a BLAKE3 hash for the `marque-1.0` audit record's
/// `original_digest` / `canonical.bytes_digest` fields.
///
/// Wire form: `"blake3:<64-hex>"` per contract §137 / §151.
fn blake3_audit_string(hash: &blake3::Hash) -> String {
    format!("blake3:{}", hash.to_hex())
}

/// Format a `SystemTime` as an RFC3339 string for the audit record's
/// `timestamp` field.
fn format_timestamp(ts: std::time::SystemTime) -> String {
    humantime::format_rfc3339(ts).to_string()
}

/// Resolve a `CategoryId` to its lowercase scheme-name label
/// (e.g., `"classification"`, `"sci"`, `"rel_to"`).
///
/// The reserved [`marque_scheme::CategoryId::MARKING`] sentinel
/// projects to the literal `"Marking"` (PascalCase to signal the
/// multi-category whole-marking case); scheme-allocated categories
/// project through the scheme's [`MarkingScheme::categories`] table.
/// Unknown ids — should not occur in production — project to
/// `"unknown"` so the audit record stays well-formed.
fn category_label(scheme: &CapcoScheme, category_id: marque_scheme::CategoryId) -> &'static str {
    use marque_scheme::MarkingScheme;
    if category_id == marque_scheme::CategoryId::MARKING {
        return "Marking";
    }
    scheme
        .categories()
        .iter()
        .find(|c| c.id == category_id)
        .map(|c| c.name)
        .unwrap_or("unknown")
}

/// Project a [`Canonical<CapcoScheme>`] into the audit-record JSON
/// shape per contract §253-291.
fn project_canonical_to_json<'a>(
    scheme: &'a CapcoScheme,
    canonical: &marque_scheme::Canonical<CapcoScheme>,
    precomputed_bytes_digest: &blake3::Hash,
) -> AuditCanonicalJson<'a> {
    let digest = blake3_audit_string(precomputed_bytes_digest);
    match canonical.source() {
        TokenSource::Cve(token_id) => {
            // PR 3c.2.D PM-D-10: closed-CVE provenance projects the
            // namespaced `Category.Token` form via
            // `Vocabulary::qualified_token_label`. CapcoScheme binds
            // `type Token = TokenId` so the `&S::Token` accessor
            // takes a `&TokenId` directly. The default `"unknown.unknown"`
            // label appears only for tokens the scheme's CVE table
            // does not route through `capco_token_category` — the
            // visible signal Constitution VIII calls for when audit
            // emit hits an unrecognized token rather than panicking.
            let label =
                <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(scheme, token_id);
            AuditCanonicalJson {
                source: "cve",
                token_id: Some(label),
                category: None,
                render_call_site: None,
                bytes_digest: digest,
            }
        }
        TokenSource::OpenVocab {
            category,
            render_call_site,
        } => AuditCanonicalJson {
            source: "open_vocab",
            token_id: None,
            category: Some(category_label(scheme, *category)),
            render_call_site: Some(format!(
                "{}:{}",
                render_call_site.file(),
                render_call_site.line(),
            )),
            bytes_digest: digest,
        },
    }
}

/// Project a [`marque_rules::Confidence`] into the audit-record JSON
/// shape per contract §140-147.
fn project_confidence_to_json(confidence: &marque_rules::Confidence) -> AuditConfidenceJson<'_> {
    AuditConfidenceJson {
        recognition: confidence.recognition,
        rule: confidence.rule,
        combined: confidence.combined(),
        region: confidence.region,
        runner_up_ratio: confidence.runner_up_ratio,
        features: confidence
            .features
            .iter()
            .map(|f| AuditFeatureJson {
                id: f.id.as_str(),
                delta: f.delta,
            })
            .collect(),
    }
}

/// Project a [`marque_rules::Message`] into the audit-record JSON
/// shape per contract §154-166.
///
/// `args` is a partial-emit map: only populated fields appear (empty
/// / `None` fields elide). Per Constitution V Principle V, every
/// field type in [`MessageArgs`] is on the permitted-identifier list
/// (token canonicals, category names, span offsets, BLAKE3 digests,
/// closed-enum `FeatureId` labels, `RuleId` strings).
fn project_message_to_json<'a>(
    scheme: &'a CapcoScheme,
    message: &marque_rules::Message,
) -> AuditMessageJson<'a> {
    let mut args = serde_json::Map::new();
    let m = message.args();
    if let Some(token_id) = m.token {
        let label =
            <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(scheme, &token_id);
        args.insert(
            "token".to_owned(),
            serde_json::Value::String(label.into_owned()),
        );
    }
    if let Some(category_id) = m.category {
        args.insert(
            "category".to_owned(),
            serde_json::Value::String(category_label(scheme, category_id).to_owned()),
        );
    }
    if let Some(span) = m.span {
        args.insert(
            "span".to_owned(),
            serde_json::json!({ "start": span.start, "end": span.end }),
        );
    }
    if let Some(digest) = m.digest {
        args.insert(
            "digest".to_owned(),
            serde_json::Value::String(blake3_audit_string(&digest)),
        );
    }
    if let Some(ref confidence) = m.confidence {
        args.insert(
            "confidence".to_owned(),
            serde_json::to_value(project_confidence_to_json(confidence))
                .unwrap_or(serde_json::Value::Null),
        );
    }
    if let Some(expected_token) = m.expected_token {
        let label = <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(
            scheme,
            &expected_token,
        );
        args.insert(
            "expected_token".to_owned(),
            serde_json::Value::String(label.into_owned()),
        );
    }
    if let Some(actual_token) = m.actual_token {
        let label =
            <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(scheme, &actual_token);
        args.insert(
            "actual_token".to_owned(),
            serde_json::Value::String(label.into_owned()),
        );
    }
    if !m.feature_ids.is_empty() {
        args.insert(
            "feature_ids".to_owned(),
            serde_json::Value::Array(
                m.feature_ids
                    .iter()
                    .map(|f| serde_json::Value::String(f.as_str().to_owned()))
                    .collect(),
            ),
        );
    }
    if !m.contributing_rule_ids.is_empty() {
        args.insert(
            "contributing_rule_ids".to_owned(),
            serde_json::Value::Array(
                m.contributing_rule_ids
                    .iter()
                    // T044: `RuleId.as_str()` is removed; the canonical
                    // wire-string form is `Display`-produced
                    // `"<scheme>:<predicate_id>"`. The `args` field on
                    // the `MessageArgs` projection is a partial-emit
                    // map; keeping the value a string here matches the
                    // existing shape (no behavior change for audit
                    // consumers beyond the wire-string format).
                    .map(|r| serde_json::Value::String(r.to_string()))
                    .collect(),
            ),
        );
    }
    AuditMessageJson {
        template: message.template().as_str(),
        args,
        _marker: std::marker::PhantomData,
    }
}

/// Convert a v2 `AppliedFix<CapcoScheme>` into the `marque-1.0` audit
/// JSON shape per contract §107-178.
pub fn applied_fix_to_audit_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    fix: &'a marque_rules::audit::AppliedFix<CapcoScheme>,
) -> AuditRecordJsonV1_0<'a> {
    let replacement = AuditReplacementJson {
        discriminant: discriminant_from_source(fix.source).as_str(),
        canonical: project_canonical_to_json(
            scheme,
            &fix.fix.replacement.canonical,
            &fix.fix.replacement.bytes_digest,
        ),
        confidence: project_confidence_to_json(&fix.fix.replacement.confidence),
    };
    let fix_detail = AuditFixJson {
        replacement,
        original_span: SpanJson {
            start: fix.fix.original_span.start,
            end: fix.fix.original_span.end,
        },
        original_digest: blake3_audit_string(&fix.fix.original_digest),
    };
    AuditRecordJsonV1_0 {
        kind: "applied_fix",
        schema: AUDIT_SCHEMA_VERSION,
        rule: (&fix.rule).into(),
        severity: fix.severity.as_str(),
        span: SpanJson {
            start: fix.span.start,
            end: fix.span.end,
        },
        fix: fix_detail,
        message: project_message_to_json(scheme, &fix.message),
        timestamp: format_timestamp(fix.timestamp),
        classifier_id: fix.classifier_id.as_deref(),
        dry_run: fix.dry_run,
        input: fix.input.as_deref(),
    }
}

/// Convert an [`AppliedTextCorrection`] into the `marque-1.0` text-
/// correction NDJSON line per contract §388-402.
pub fn text_correction_to_audit_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    tc: &'a AppliedTextCorrection,
) -> TextCorrectionRecordJsonV1_0<'a> {
    TextCorrectionRecordJsonV1_0 {
        kind: "text_correction",
        schema: AUDIT_SCHEMA_VERSION,
        rule: (&tc.rule).into(),
        severity: tc.severity.as_str(),
        span: SpanJson {
            start: tc.span.start,
            end: tc.span.end,
        },
        original_digest: blake3_audit_string(&tc.original_digest),
        replacement: tc.replacement.as_str(),
        source: fix_source_str(tc.source),
        confidence: project_confidence_to_json(&tc.confidence),
        migration_ref: tc.migration_ref,
        message: project_message_to_json(scheme, &tc.message),
        timestamp: format_timestamp(tc.timestamp),
        classifier_id: tc.classifier_id.as_deref(),
        dry_run: tc.dry_run,
        input: tc.input.as_deref(),
    }
}

/// Serialize a single [`AuditLine<CapcoScheme>`] to a `serde_json::Value`
/// in the `marque-2.0` shape (dispatcher; was `marque-1.0` pre-T044).
///
/// Two arms project to disjoint NDJSON record types:
/// - [`AuditLine::AppliedFix`] → `{"type": "applied_fix", ...}` per
///   contract §107-178.
/// - [`AuditLine::TextCorrection`] → `{"type": "text_correction", ...}`
///   per contract §388-402.
///
/// Non-exhaustive guard returns `serde_json::Value::Null` for any
/// future variant — the canary scan at
/// `crates/engine/tests/audit_g13_canary.rs` (T055) catches this if it
/// fires.
pub fn audit_line_to_json_v1_0(
    scheme: &CapcoScheme,
    line: &AuditLine<CapcoScheme>,
) -> serde_json::Value {
    match line {
        AuditLine::AppliedFix(fix) => {
            serde_json::to_value(applied_fix_to_audit_json_v1_0(scheme, fix))
                .unwrap_or(serde_json::Value::Null)
        }
        AuditLine::TextCorrection(tc) => {
            serde_json::to_value(text_correction_to_audit_json_v1_0(scheme, tc))
                .unwrap_or(serde_json::Value::Null)
        }
        // `AuditLine` is `#[non_exhaustive]`; a future variant lands
        // as `Null` until the renderer adds an arm. The T055 G13
        // canary catches the emitted-line shape regression in CI.
        //
        // **Parallel-update requirement.** When a new `AuditLine`
        // variant lands in `marque-rules::audit`, three call sites
        // MUST add a corresponding arm in lockstep: this renderer
        // (CLI), the WASM renderer at `crates/wasm/src/lib.rs`, and
        // the canary's `render_audit_line_to_json` at
        // `crates/engine/tests/audit_g13_canary.rs`. A silent
        // `Value::Null` from any of the three would defeat the G13
        // content-ignorance canary's regression detection — the
        // canary sweeps the corpus and asserts no input substring
        // appears in the emitted JSON, but a `Null` arm emits
        // nothing for the canary to scan, so a future variant that
        // accidentally leaked content would pass the sweep
        // vacuously. Pre-PR 3c.2.D fixup F-10.
        _ => serde_json::Value::Null,
    }
}

/// Emit a single `marque-1.0` audit record as NDJSON to `stderr`.
///
/// Per PM-D-1 the v1.0 emit path is the marque-1.0 wire-format path
/// the renderer migration in PR 3c.2.D / D4 wires. Reads from the
/// engine's [`marque_engine::FixResult::audit_lines`] (v2 stream)
/// rather than the v1 [`marque_engine::FixResult::applied`] stream —
/// `audit_lines` preserves cross-record promotion order across both
/// the marking-fix arm and the text-correction arm (PM-D-8).
///
/// Routes through [`audit_line_to_json_v1_0`] for the wire-format
/// projection, then serializes to NDJSON with a trailing newline. On
/// serialization failure emits a JSON error frame on the audit
/// stream (FR-005a fallback) so the audit channel remains
/// well-formed.
pub fn render_audit_line(
    stderr: &mut dyn std::io::Write,
    scheme: &CapcoScheme,
    line: &AuditLine<CapcoScheme>,
) -> std::io::Result<()> {
    // Single accepted schema (`marque-2.0`) so dispatch is a no-op
    // today; the const lookup is kept so a future schema bump can
    // land via the same dispatch shape without restructuring callers.
    let _ = AUDIT_SCHEMA_IS_V2_0;
    // T044: `RuleId.as_str()` is removed; the `Display` impl renders
    // the canonical wire-string form `"<scheme>:<predicate_id>"`, which
    // is what the error-frame fallback channel surfaces to humans.
    let rule_id: String = match line {
        AuditLine::AppliedFix(fix) => fix.rule.to_string(),
        AuditLine::TextCorrection(tc) => tc.rule.to_string(),
        _ => "unknown".to_owned(),
    };
    let serialized = serde_json::to_vec(&audit_line_to_json_v1_0(scheme, line));
    match serialized {
        Ok(mut buf) => {
            buf.push(b'\n');
            stderr.write_all(&buf)
        }
        Err(e) => {
            render_audit_error_frame(stderr, &rule_id, &e.to_string())?;
            Err(std::io::Error::other(format!(
                "audit record serialization failed for rule {rule_id}: {e}"
            )))
        }
    }
}

/// Emit an error frame on the audit stream when serialization fails.
///
/// FR-005a fallback: every line on the audit stream must be a complete JSON
/// object. The error frame is the last resort when the normal serializer has
/// already failed, so it JSON-escapes its inputs via `serde_json::to_string`
/// to guarantee well-formed output even if the error message contains quotes
/// or backslashes.
///
/// Shape: `{"schema":"<AUDIT_SCHEMA_VERSION>","error":"<code>","rule":"<id>"}`
///
/// where `<AUDIT_SCHEMA_VERSION>` is the build-time value of the
/// `MARQUE_AUDIT_SCHEMA` env var (default `marque-2.0`; see
/// `crates/engine/build.rs`). The schema string is emitted dynamically
/// so an audit consumer can dispatch on the schema version without
/// the renderer's docs going stale on a schema bump.
pub fn render_audit_error_frame(
    stderr: &mut dyn std::io::Write,
    rule_id: &str,
    error_code: &str,
) -> std::io::Result<()> {
    // JSON-escape both values so special characters in error messages
    // cannot produce malformed JSON on the audit stream.
    let escaped_error =
        serde_json::to_string(error_code).unwrap_or_else(|_| "\"serialization_error\"".to_owned());
    let escaped_rule = serde_json::to_string(rule_id).unwrap_or_else(|_| "\"unknown\"".to_owned());
    let frame = format!(
        "{{\"schema\":\"{AUDIT_SCHEMA_VERSION}\",\"error\":{escaped_error},\"rule\":{escaped_rule}}}\n"
    );
    stderr.write_all(frame.as_bytes())
}

/// Convert a byte offset into 1-based (line, column).
fn byte_to_line_col(source: &[u8], offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for &b in &source[..offset.min(source.len())] {
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Path-label helper. Uses `-` for stdin sentinels.
pub fn label_for(path: Option<&Path>) -> String {
    match path {
        Some(p) => p.display().to_string(),
        None => "-".to_owned(),
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::Span;
    use marque_rules::{
        FixIntent, FixSource, Message, MessageArgs, MessageTemplate, RuleId, Severity,
    };
    use marque_scheme::{ReplacementIntent, fix_intent::RecanonScope};

    fn make_diagnostic(
        rule: RuleId,
        span: Span,
        _message: &str,
        fix: Option<FixIntent<CapcoScheme>>,
    ) -> Diagnostic<CapcoScheme> {
        // PR 3c.2.C C5: typed Message + Citation. Test fixtures use a
        // generic template/citation; the test bodies inspect rule/span/
        // severity, not message content.
        // T044: helper now takes a constructed `RuleId` directly so the
        // 2-tuple form is visible at every call site (the `&'static str`
        // single-arg shape was tied to the pre-T044 `RuleId::new(id)`
        // constructor).
        Diagnostic::new(
            rule,
            Severity::Fix,
            span,
            Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs::default(),
            ),
            marque_scheme::capco(marque_scheme::SectionLetter::A, 6, 15),
            fix,
        )
    }

    fn make_intent_fix() -> FixIntent<CapcoScheme> {
        FixIntent {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Portion,
            },
            confidence: marque_rules::Confidence::strict(1.0),
            feature_ids: Default::default(),
            message: Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs::default(),
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        }
    }

    #[test]
    fn extract_line_returns_first_line() {
        let src = b"first\nsecond\nthird";
        assert_eq!(extract_line(src, 1), Some(&b"first"[..]));
    }

    #[test]
    fn extract_line_returns_middle_line() {
        let src = b"first\nsecond\nthird";
        assert_eq!(extract_line(src, 2), Some(&b"second"[..]));
    }

    #[test]
    fn extract_line_returns_last_line_without_trailing_newline() {
        let src = b"first\nsecond";
        assert_eq!(extract_line(src, 2), Some(&b"second"[..]));
    }

    #[test]
    fn extract_line_strips_crlf() {
        let src = b"first\r\nsecond\r\n";
        assert_eq!(extract_line(src, 1), Some(&b"first"[..]));
        assert_eq!(extract_line(src, 2), Some(&b"second"[..]));
    }

    #[test]
    fn extract_line_returns_none_when_out_of_range() {
        let src = b"first\nsecond";
        assert_eq!(extract_line(src, 5), None);
    }

    #[test]
    fn render_human_produces_rustc_style_shape_with_caret() {
        // Single-line source with a span pointing at "NF" in banner form.
        let src = b"TOP SECRET//SI//NF\n";
        let span = Span::new(16, 18);
        let fix = make_intent_fix();
        // T044: legacy `E001` retired (PR 3c.B Commit 6); test fixture
        // uses the canonical illustrative tuple
        // `("capco", "banner.classification.usa-trigraph")` matching
        // F1's lib.rs unit-test convention. The renderer's `[{}]`
        // formatter now emits the `Display` wire-string form
        // `<scheme>:<predicate_id>`.
        let diag = make_diagnostic(
            RuleId::new("capco", "banner.classification.usa-trigraph"),
            span,
            "banner uses abbreviated dissem control \"NF\"; use \"NOFORN\"",
            Some(fix),
        );

        let mut out = Vec::new();
        render_human(&mut out, "banner.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        // Header line: path:line:col with level + rule + template label.
        // PR 3c.2.C C5: the message column renders the closed-template
        // label, no longer a free-form sentence; `make_diagnostic`
        // uses `MessageTemplate::BannerRollupMismatch`.
        // T044: the rule label is the wire-string form
        // `"<scheme>:<predicate_id>"` (Display impl) per PM OD-3.
        assert!(rendered.contains("banner.txt:1:17 fix[capco:banner.classification.usa-trigraph]"));
        assert!(rendered.contains("BannerRollupMismatch"));
        // Location arrow
        assert!(rendered.contains("--> banner.txt:1:17-19"));
        // Source snippet with line number gutter
        assert!(rendered.contains("1 | TOP SECRET//SI//NF"));
        // Caret line: 16 spaces of padding + "^^" carets
        // (span starts at col 17, so 16 chars of padding from col 1)
        assert!(
            rendered.contains("                ^^"),
            "expected caret at col 17; got:\n{rendered}"
        );
        // Post-Commit-10: FixIntent carries no replacement bytes (the
        // renderer is on the diagnostic path; no engine projection
        // available). The hint shows the auto-fixable signal +
        // confidence, not the literal replacement.
        assert!(
            rendered.contains("auto-fixable; confidence 100%"),
            "expected auto-fixable hint; got:\n{rendered}"
        );
        // Citation footer. PR 3c.2.C C5: typed `Citation` Display
        // emits the bare `§<L>.<sub> p<N>` shape (no "CAPCO-2016"
        // prefix); the prefix lives in the renderer's surrounding
        // text or in the JSON `document` field for non-CAPCO sources.
        assert!(rendered.contains("= citation: §A.6 p15"));
    }

    #[test]
    fn render_human_without_color_has_no_ansi_escapes() {
        let src = b"TOP SECRET//SI//NF\n";
        let span = Span::new(16, 18);
        // T044: 2-tuple form; see canonical-illustrative comment above.
        let diag = make_diagnostic(
            RuleId::new("capco", "banner.classification.usa-trigraph"),
            span,
            "test",
            None,
        );

        let mut out = Vec::new();
        render_human(&mut out, "x.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert!(
            !rendered.contains('\x1b'),
            "color=false must not emit ANSI escapes, got:\n{rendered:?}"
        );
    }

    #[test]
    fn render_human_with_color_emits_ansi_escapes() {
        let src = b"TOP SECRET//SI//NF\n";
        let span = Span::new(16, 18);
        // T044: 2-tuple form; see canonical-illustrative comment above.
        let diag = make_diagnostic(
            RuleId::new("capco", "banner.classification.usa-trigraph"),
            span,
            "test",
            None,
        );

        let mut out = Vec::new();
        render_human(&mut out, "x.txt", src, &diag, true).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert!(
            rendered.contains('\x1b'),
            "color=true must emit ANSI escapes, got:\n{rendered:?}"
        );
    }

    #[test]
    fn render_human_appends_marque_brand_suffix_to_header_line() {
        // Branding invariant: the human-pretty-print path appends "—Marque"
        // to every diagnostic header. Structured outputs (NDJSON / JSON)
        // MUST NOT carry the suffix — they keep the template label
        // byte-identical so downstream tooling does not have to strip it.
        //
        // PR 3c.2.C C5: `Diagnostic.message` is now a closed `Message`
        // (template + args). The rendered human header shows the
        // template label (e.g. `BannerRollupMismatch`); the NDJSON
        // emits the same label under `message.template`.
        let src = b"TOP SECRET//SI//NF\n";
        let span = Span::new(16, 18);
        // T044: 2-tuple form; see canonical-illustrative comment above.
        let diag = make_diagnostic(
            RuleId::new("capco", "banner.classification.usa-trigraph"),
            span,
            "test message",
            None,
        );
        let template_label = diag.message.template().as_str();

        let mut human_out = Vec::new();
        render_human(&mut human_out, "x.txt", src, &diag, false).unwrap();
        let human = String::from_utf8(human_out).unwrap();
        assert!(
            human.contains(&format!("{template_label} —Marque")),
            "human header must end with \" —Marque\"; got:\n{human}"
        );

        // NDJSON path must not carry the brand suffix.
        let json = diagnostic_to_json(&diag);
        assert_eq!(
            json.message.template, template_label,
            "NDJSON `message.template` must be the closed-template label"
        );
        assert!(
            !json.message.template.contains("Marque"),
            "NDJSON message.template field must never be branded"
        );
    }

    #[test]
    fn render_human_diagnostic_without_fix_omits_hint() {
        // E008-style: no fix proposal, caret only.
        // PR 3c.2.C C5: typed Message + Citation via make_diagnostic.
        let src = b"SECRET//XYZZY//NOFORN\n";
        let span = Span::new(8, 13);
        let diag = Diagnostic::new(
            // T044: `E008` → `("capco", "marking.metadata.unrecognized-token")`
            // per `docs/refactor-006/legacy-rule-id-map.md` §1.
            RuleId::new("capco", "marking.metadata.unrecognized-token"),
            Severity::Error,
            span,
            Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default()),
            marque_scheme::capco(marque_scheme::SectionLetter::A, 6, 15),
            None,
        );

        let mut out = Vec::new();
        render_human(&mut out, "x.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert!(rendered.contains("^^^^^"));
        assert!(!rendered.contains("replace with"));
    }

    // --- Suggest-channel tests (issue #235 / #186 PR-3) ---

    #[test]
    fn render_human_suggest_severity_uses_did_you_mean_phrasing() {
        // The Suggest channel must read as a hint, not a confirmed
        // fix. The renderer swaps "replace with" for "did you mean"
        // and changes the level color (BoldYellow vs BoldRed) so the
        // user can tell at a glance the engine will not auto-apply.
        let src = b"SECRET//REL TO USA, AUT, GBR\n";
        let span = Span::new(20, 23);
        let fix = make_intent_fix();
        let diag = Diagnostic::new(
            // T044: `S004` → `("capco", "portion.dissem.rel-to-trigraph-suggest")`
            // per `docs/refactor-006/legacy-rule-id-map.md` §1.
            RuleId::new("capco", "portion.dissem.rel-to-trigraph-suggest"),
            Severity::Suggest,
            span,
            Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
            marque_scheme::capco(marque_scheme::SectionLetter::H, 8, 150),
            Some(fix),
        );

        let mut out = Vec::new();
        render_human(&mut out, "rel.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        // Header carries the "suggest" level string, not "error" / "fix".
        // T044: header includes the wire-string form `<scheme>:<predicate_id>`.
        let expected_header = "suggest[capco:portion.dissem.rel-to-trigraph-suggest]";
        assert!(
            rendered.contains(expected_header),
            "header must read {expected_header}; got:\n{rendered}"
        );
        // PR 3c.2.C C5: the fixture uses `make_intent_fix()` (a
        // `FixIntent` carrying a structural `Recanonicalize` intent),
        // not a `TextCorrection`. Per the renderer at `render.rs:187-204`,
        // the "did you mean X" prose only fires when `text_correction`
        // is set with the candidate bytes; when only a `FixIntent` is
        // present the Suggest-severity branch renders the generic
        // "(suggested fix; confidence ...)" hint.
        assert!(
            rendered.contains("(suggested fix; confidence"),
            "Suggest hint must read \"(suggested fix; confidence ...)\"; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("replace with"),
            "Suggest must not use the imperative \"replace with\" form; got:\n{rendered}"
        );
    }

    #[test]
    fn render_human_suggest_severity_uses_bold_yellow_with_color_enabled() {
        // The BoldYellow ANSI escape (`\x1b[33;1m`) for the "suggest"
        // header is what visually distinguishes the suggest-don't-fix
        // channel from Error/Warn (which use BoldRed). With color=false
        // the styling collapses to plain text; with color=true the
        // escape sequence MUST be present in the output.
        let src = b"SECRET//REL TO USA, AUT, GBR\n";
        let span = Span::new(20, 23);
        let fix = make_intent_fix();
        let diag = Diagnostic::new(
            // T044: see canonical map row for `S004`.
            RuleId::new("capco", "portion.dissem.rel-to-trigraph-suggest"),
            Severity::Suggest,
            span,
            Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
            marque_scheme::capco(marque_scheme::SectionLetter::H, 8, 150),
            Some(fix),
        );

        let mut out = Vec::new();
        render_human(&mut out, "rel.txt", src, &diag, true).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        // BoldYellow opening sequence must wrap the "suggest" header.
        assert!(
            rendered.contains("\x1b[33;1msuggest\x1b[0m"),
            "Suggest header must use BoldYellow ANSI escape; got:\n{rendered}"
        );
        // BoldRed (used by Error/Warn headers) must NOT appear on the
        // level header — but the caret line still uses BoldRed for
        // visibility, so we only assert the level isn't styled red.
        assert!(
            !rendered.contains("\x1b[31;1msuggest"),
            "Suggest header must not use BoldRed; got:\n{rendered}"
        );
    }

    #[test]
    fn render_human_suggest_with_no_fix_round_trips() {
        // Issue #206 spike: future rules (REL TO opaque-uncertain
        // reduction) will emit `Severity::Suggest` with `fix: None`
        // — informational suggestion with no candidate replacement.
        // The renderer must not panic on the missing fix and must
        // still produce a clean diagnostic.
        let src = b"SECRET//REL TO USA, FVEY\n";
        let span = Span::new(20, 24);
        let diag = Diagnostic::new(
            // T044: `S999` → `("test", "synthetic.s999-fixture")` per
            // `docs/refactor-006/legacy-rule-id-map.md` §10 (reserved
            // `test` scheme for synthetic Constitution V Principle V
            // test-fixture identifiers).
            RuleId::new("test", "synthetic.s999-fixture"),
            Severity::Suggest,
            span,
            Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
            // S999 is a hypothetical test rule with no CAPCO citation;
            // use the EngineInternal sentinel per PM-C-4 so citation-lint
            // skips the entry.
            marque_scheme::Citation::new(
                marque_scheme::AuthoritativeSource::EngineInternal,
                marque_scheme::SectionRef::new(marque_scheme::SectionLetter::A),
                core::num::NonZeroU16::new(1).unwrap(),
            ),
            None,
        );

        let mut out = Vec::new();
        render_human(&mut out, "rel.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        // T044: rule label is the wire-string form.
        assert!(
            rendered.contains("suggest[test:synthetic.s999-fixture]"),
            "Suggest with no fix still renders header at suggest level; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("did you mean"),
            "no fix means no \"did you mean\" hint; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("replace with"),
            "no fix means no replace-with hint; got:\n{rendered}"
        );
    }

    #[test]
    fn diagnostic_to_json_carries_suggest_severity_string() {
        // NDJSON consumers depend on the canonical lowercase string
        // form of `Severity`. Phase D's NDJSON contract is otherwise
        // unchanged: a Suggest-severity diagnostic round-trips
        // through `severity: "suggest"` with no schema bump.
        let span = Span::new(0, 3);
        let fix = make_intent_fix();
        let diag = Diagnostic::new(
            // T044: see canonical map row for `S004`.
            RuleId::new("capco", "portion.dissem.rel-to-trigraph-suggest"),
            Severity::Suggest,
            span,
            Message::new(MessageTemplate::NonCanonicalOrder, MessageArgs::default()),
            marque_scheme::capco(marque_scheme::SectionLetter::H, 8, 150),
            Some(fix),
        );

        let json = diagnostic_to_json(&diag);
        assert_eq!(json.severity, "suggest");
        // T044 PM OD-2: `json.rule` is the structured-object form
        // `RuleIdJson { scheme, predicate_id }` (not a flat string).
        // The NDJSON serializes it as
        // `"rule": {"scheme": "capco", "predicate_id": "..."}`.
        assert_eq!(json.rule.scheme, "capco");
        assert_eq!(
            json.rule.predicate_id,
            "portion.dissem.rel-to-trigraph-suggest"
        );
        // Fix payload is preserved on the wire so a downstream
        // consumer can render the candidate replacement themselves.
        assert!(json.fix.is_some());
        // Post Commit 10 the wire shape carries `intent_kind` (the
        // structural emission discriminant); `replacement` is `None`
        // for non-text-correction fixes since the engine renders
        // bytes from the per-page projection.
        let fix_json = json.fix.as_ref().unwrap();
        assert!(matches!(
            fix_json.intent_kind,
            "FactAdd" | "FactRemove" | "Recanonicalize" | "TextCorrection"
        ));
    }

    // --- Audit record tests ---

    #[test]
    fn render_audit_error_frame_produces_valid_json() {
        let mut buf = Vec::new();
        render_audit_error_frame(&mut buf, "E001", "some error").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'), "must end with newline");
        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(v["schema"], AUDIT_SCHEMA_VERSION);
        assert_eq!(v["error"], "some error");
        assert_eq!(v["rule"], "E001");
    }

    #[test]
    fn render_audit_error_frame_escapes_special_characters() {
        let mut buf = Vec::new();
        // Error message with quotes and backslashes that would break raw interpolation.
        render_audit_error_frame(&mut buf, "E001", "key \"foo\\bar\"").unwrap();
        let s = String::from_utf8(buf).unwrap();
        // Must parse as valid JSON despite special characters in the error.
        let v: serde_json::Value =
            serde_json::from_str(s.trim()).expect("error frame must be valid JSON");
        assert_eq!(v["error"], "key \"foo\\bar\"");
        assert_eq!(v["schema"], AUDIT_SCHEMA_VERSION);
    }

    #[test]
    fn render_audit_line_produces_valid_v1_0_ndjson() {
        // PR 3c.2.D / D4: migrated to the marque-1.0 wire format.
        // Constructs a v2 `AppliedFix<CapcoScheme>` through the
        // engine-promotion seal and asserts the wire-shape matches
        // `contracts/audit-record.md` body §107-178.
        //
        // Test-fixture carve-out per Constitution V Principle V —
        // the `__engine_promote` call is inside `#[cfg(test)]` and
        // constructs a synthetic AppliedFix to exercise the
        // renderer, never commingled with engine output.
        use marque_ism::Span;
        use marque_rules::audit::AppliedFix as AuditAppliedFix;
        use marque_rules::{EnginePromotionToken, RuleId};
        use marque_scheme::canonical::{Canonical, CanonicalConstructor, EngineConstructor};
        use std::sync::Arc;
        use std::time::{Duration, UNIX_EPOCH};

        let fix = make_intent_fix();
        // Build the v2 Canonical<S> via EngineConstructor (the same
        // sealed open-vocab path the engine uses at promotion time).
        //
        // Test-fixture carve-out per Constitution V Principle V —
        // renderer exercise synthetic fixture; never reaches the
        // real audit stream.
        let constructor: EngineConstructor<CapcoScheme> =
            EngineConstructor::<CapcoScheme>::__engine_construct();
        // Use the multi-category sentinel — the synthetic fix above
        // uses `Recanonicalize`, which routes through
        // `CategoryId::MARKING` per the engine's resolution path.
        let canonical: Canonical<CapcoScheme> = constructor.build_open_vocab(
            marque_scheme::CategoryId::MARKING,
            Box::from("(S)"),
            marque_scheme::Scope::Portion,
        );
        // Test-fixture carve-out per Constitution V Principle V —
        // synthetic AppliedFix for renderer exercise only.
        let token = EnginePromotionToken::__engine_construct();
        let applied = AuditAppliedFix::<CapcoScheme>::__engine_promote(
            // T044: `E002` → `("capco", "portion.dissem.rel-to-missing-usa")`
            // per `docs/refactor-006/legacy-rule-id-map.md` §1.
            RuleId::new("capco", "portion.dissem.rel-to-missing-usa"),
            marque_rules::Severity::Fix,
            Span::new(8, 10),
            fix,
            b"(S)",
            canonical,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            Some(Arc::from("classifier-42")),
            false,
            Some(Arc::from("test.txt")),
            token,
        );

        let mut buf = Vec::new();
        let scheme = marque_engine::default_scheme();
        let line = marque_rules::AuditLine::AppliedFix(applied);
        render_audit_line(&mut buf, &scheme, &line).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'));

        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap();

        // Top-level marque-1.0 shape per contract §107-178.
        assert_eq!(v["type"], "applied_fix");
        assert_eq!(v["schema"], AUDIT_SCHEMA_VERSION);
        // T044 PM OD-2: structured-object `rule` shape on the wire.
        assert_eq!(v["rule"]["scheme"], "capco");
        assert_eq!(
            v["rule"]["predicate_id"],
            "portion.dissem.rel-to-missing-usa"
        );
        assert_eq!(v["severity"], "fix");
        assert_eq!(v["span"]["start"], 8);
        assert_eq!(v["span"]["end"], 10);

        // `fix` sub-object per contract §123-152.
        let fix_obj = &v["fix"];
        assert_eq!(fix_obj["original_span"]["start"], 8);
        assert_eq!(fix_obj["original_span"]["end"], 10);
        assert!(
            fix_obj["original_digest"]
                .as_str()
                .unwrap()
                .starts_with("blake3:"),
            "original_digest must be a 'blake3:<hex>' string; got: {fix_obj:?}"
        );

        // `replacement` sub-object per contract §124-148.
        let replacement = &fix_obj["replacement"];
        assert_eq!(
            replacement["discriminant"], "strict",
            "BuiltinRule source projects to discriminant=strict per PM-D-7"
        );

        // `canonical` sub-object per contract §253-291. The fix uses
        // `Recanonicalize`, which routes through CategoryId::MARKING
        // → open_vocab path with `category: "Marking"`.
        let canonical_json = &replacement["canonical"];
        assert_eq!(canonical_json["source"], "open_vocab");
        assert_eq!(canonical_json["category"], "Marking");
        assert!(
            canonical_json["bytes_digest"]
                .as_str()
                .unwrap()
                .starts_with("blake3:"),
            "bytes_digest must be a 'blake3:<hex>' string"
        );
        assert!(
            canonical_json["render_call_site"].as_str().is_some(),
            "open_vocab canonical must carry render_call_site"
        );
        // CVE-only field elides for open_vocab arms.
        assert!(canonical_json.get("token_id").is_none());

        // `confidence` sub-object per contract §140-147.
        let confidence = &replacement["confidence"];
        assert_eq!(confidence["recognition"], 1.0);
        assert_eq!(confidence["rule"], 1.0);
        assert_eq!(confidence["combined"], 1.0);
        // Strict-path fix: region / runner_up_ratio are present but
        // None per Confidence::strict; serde emits them as `null`.
        assert!(confidence["region"].is_null());
        assert!(confidence["runner_up_ratio"].is_null());
        // Features SmallVec defaulted to empty for the test fixture.
        assert!(confidence["features"].as_array().unwrap().is_empty());

        // `message` sub-object per contract §154-166.
        let message = &v["message"];
        assert_eq!(message["template"], "BannerRollupMismatch");
        // `args` is a partial-emit map; the test fixture uses
        // MessageArgs::default() so every field elides.
        let args = message["args"].as_object().unwrap();
        assert!(
            args.is_empty(),
            "default MessageArgs emits an empty args map; got: {args:?}"
        );

        // Top-level runtime context.
        assert!(v["timestamp"].as_str().unwrap().contains('T'));
        assert_eq!(v["classifier_id"], "classifier-42");
        assert_eq!(v["dry_run"], false);
        assert_eq!(v["input"], "test.txt");
    }

    #[test]
    fn render_audit_line_text_correction_arm() {
        // PR 3c.2.D / D4: separate NDJSON line type for the C001 /
        // text-correction path per PM-D-4. The arm carries a
        // corpus-derived `SmolStr` replacement (Constitution V
        // Principle V permitted identifier) rather than a
        // `Canonical<S>` payload.
        use marque_ism::Span;
        use marque_rules::audit::AppliedTextCorrection;
        use marque_rules::{
            Confidence, EnginePromotionToken, FixSource, Message, MessageArgs, MessageTemplate,
            RuleId, Severity,
        };
        use std::sync::Arc;
        use std::time::{Duration, UNIX_EPOCH};

        // Synthetic digest for the test fixture.
        let original_digest = blake3::hash(b"SERCET");

        // Test-fixture carve-out per Constitution V Principle V.
        let token = EnginePromotionToken::__engine_construct();
        let tc = AppliedTextCorrection::__engine_promote_text_correction(
            // T044: `C001` → `("capco", "marking.correction.token-typo")`
            // per `docs/refactor-006/legacy-rule-id-map.md` §1.
            RuleId::new("capco", "marking.correction.token-typo"),
            Severity::Fix,
            Span::new(0, 6),
            original_digest,
            "SECRET".into(),
            FixSource::CorrectionsMap,
            Confidence::strict(1.0),
            None,
            Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            Some(Arc::from("classifier-42")),
            false,
            Some(Arc::from("test.txt")),
            token,
        );

        let scheme = marque_engine::default_scheme();
        let line = marque_rules::AuditLine::TextCorrection(tc);
        let mut buf = Vec::new();
        render_audit_line(&mut buf, &scheme, &line).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap();

        // Per contract §388-402.
        assert_eq!(v["type"], "text_correction");
        assert_eq!(v["schema"], AUDIT_SCHEMA_VERSION);
        // T044 PM OD-2: structured-object `rule` shape on the wire.
        assert_eq!(v["rule"]["scheme"], "capco");
        assert_eq!(v["rule"]["predicate_id"], "marking.correction.token-typo");
        assert_eq!(v["severity"], "fix");
        assert_eq!(v["span"]["start"], 0);
        assert_eq!(v["span"]["end"], 6);
        assert_eq!(v["replacement"], "SECRET");
        assert_eq!(v["source"], "CorrectionsMap");
        assert!(
            v["original_digest"]
                .as_str()
                .unwrap()
                .starts_with("blake3:")
        );
        assert!(
            v["migration_ref"].is_null(),
            "None migration_ref emits as null per audit-consumer stability"
        );
        assert_eq!(v["message"]["template"], "CorrectionsApplied");
        assert_eq!(v["classifier_id"], "classifier-42");
        assert_eq!(v["dry_run"], false);
        assert_eq!(v["input"], "test.txt");
    }
}
