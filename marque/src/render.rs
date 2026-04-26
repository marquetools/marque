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

use marque_engine::{AUDIT_SCHEMA_IS_V2, AUDIT_SCHEMA_VERSION, LintResult};
use marque_rules::{AppliedFix, Diagnostic, FeatureContribution};
use serde::Serialize;
use std::path::Path;

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
    diag: &Diagnostic,
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
    let level_styled = paint(color, AnsiStyle::BoldRed, level);
    let rule_styled = paint(color, AnsiStyle::Bold, &format!("[{}]", diag.rule));

    // ---- Header line ----
    // banner.txt:1:17 error[E001] banner uses abbreviated dissem control "NF"; use "NOFORN" —Marque
    //
    // Branding suffix is appended centrally here (human render path only),
    // never at Diagnostic::new() and never in NDJSON / JSON output — those
    // are consumed by tooling (CI scripts, editor plugins, ABAC consumers)
    // that should not have to strip branding text out of the `message`
    // field.
    writeln!(
        out,
        "{path_label}:{line}:{col_start} {level_styled}{rule_styled} {} {}",
        diag.message, BRAND_SUFFIX,
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
            let hint = diag
                .fix
                .as_ref()
                .map(|f| {
                    format!(
                        " replace with {:?} (confidence {:.0}%)",
                        f.replacement.as_ref(),
                        f.confidence.combined() * 100.0
                    )
                })
                .unwrap_or_default();
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
    BoldBlue,
    Bold,
}

fn paint(color: bool, style: AnsiStyle, text: &str) -> String {
    if !color {
        return text.to_owned();
    }
    let (prefix, suffix) = match style {
        AnsiStyle::BoldRed => ("\x1b[31;1m", "\x1b[0m"),
        AnsiStyle::BoldBlue => ("\x1b[34;1m", "\x1b[0m"),
        AnsiStyle::Bold => ("\x1b[1m", "\x1b[0m"),
    };
    format!("{prefix}{text}{suffix}")
}

/// JSON projection of a Diagnostic conforming to `contracts/diagnostic.json`.
/// Marked `additionalProperties: false` in the schema, so this struct must
/// not include extra fields.
#[derive(Debug, Serialize)]
pub struct DiagnosticJson<'a> {
    pub rule: &'a str,
    pub severity: &'a str,
    pub span: SpanJson,
    pub message: &'a str,
    pub citation: &'a str,
    pub fix: Option<FixJson<'a>>,
}

#[derive(Debug, Serialize)]
pub struct SpanJson {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Serialize)]
pub struct FixJson<'a> {
    pub source: &'static str,
    pub replacement: &'a str,
    pub confidence: f32,
    pub migration_ref: Option<&'a str>,
}

pub fn diagnostic_to_json(d: &Diagnostic) -> DiagnosticJson<'_> {
    DiagnosticJson {
        rule: d.rule.as_str(),
        severity: d.severity.as_str(),
        span: SpanJson {
            start: d.span.start,
            end: d.span.end,
        },
        message: d.message.as_ref(),
        citation: d.citation,
        fix: d.fix.as_ref().map(|f| FixJson {
            source: match f.source {
                marque_rules::FixSource::BuiltinRule => "BuiltinRule",
                marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
                marque_rules::FixSource::MigrationTable => "MigrationTable",
                marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
                marque_rules::FixSource::DecoderClassificationHeuristic => {
                    "DecoderClassificationHeuristic"
                }
            },
            replacement: f.replacement.as_ref(),
            confidence: f.confidence.combined(),
            migration_ref: f.migration_ref,
        }),
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
// Audit record NDJSON
//
// The `schema` field is sourced from `marque_engine::AUDIT_SCHEMA_VERSION`,
// which `crates/engine/build.rs` validates against the closed accept-list
// `["marque-mvp-1", "marque-mvp-2"]`. Default is `"marque-mvp-2"` (Phase D).
//
// Two struct shapes coexist:
//
// - `AuditRecordJsonV1` — the v1 contract (`contracts/audit-record.json`,
//   schema `"marque-mvp-1"`); the 12-field shape that pre-Phase-D
//   downstream consumers parse.
// - `AuditRecordJsonV2` — the v2 contract (`contracts/audit-record-v2.md`,
//   schema `"marque-mvp-2"`); strict superset of v1 — adds `recognition`
//   (always present), `runner_up_ratio` (omitted via `skip_serializing_if`
//   when `None`), and `features` (omitted when empty).
//
// `render_audit_record` dispatches to the right emitter at the
// `AUDIT_SCHEMA_IS_V2` const-folded branch. Both emitters are always
// compiled; the dead arm is eliminated by the optimizer at the matching
// build's expense.
//
// FR-014 (single-schema-per-build) is upheld at two layers:
//   1. `crates/engine/build.rs` panics on unknown values — only one of the
//      two accepted versions can ever reach the emitter.
//   2. The emitter chooses the matching struct shape from the const, so a
//      v1 build never produces a v2-shaped record and vice versa.
// ---------------------------------------------------------------------------

/// JSON projection of an `AppliedFix` conforming to
/// `contracts/audit-record.json` (schema `"marque-mvp-1"`).
///
/// Every field from the v1 schema is present; the type definition is the
/// authoritative shape contract. Emitted to stderr as NDJSON (one record
/// per line). FR-005a requires atomic emission: serialize to buffer, then
/// single `write_all`.
#[derive(Debug, Serialize)]
pub struct AuditRecordJsonV1 {
    pub schema: &'static str,
    pub rule: String,
    pub source: &'static str,
    pub span: SpanJson,
    pub original: String,
    pub replacement: String,
    pub confidence: f32,
    pub migration_ref: Option<String>,
    pub timestamp: String,
    pub classifier_id: Option<String>,
    pub dry_run: bool,
    pub input: Option<String>,
}

/// JSON projection of an `AppliedFix` conforming to
/// `contracts/audit-record-v2.md` (schema `"marque-mvp-2"`).
///
/// Strict superset of [`AuditRecordJsonV1`]: every v1 field is preserved
/// in v1's serialized order, then the v2 extensions (`recognition`,
/// `runner_up_ratio`, `features`) follow. v2 ⊃ v1 is the back-compat
/// guarantee — a v1 consumer reading a v2 record sees all the v1 fields
/// it knows about and ignores the unknown `recognition` /
/// `runner_up_ratio` / `features` keys (assuming a tolerant parser, which
/// is the standard JSON contract).
///
/// `recognition` is always emitted (strict-path = `1.0`, decoder = `<1.0`)
/// because the recognition axis is always meaningful in a v2 record.
/// `runner_up_ratio` and `features` are omitted via
/// `skip_serializing_if` when absent, so a strict-path v2 record stays
/// minimal — a downstream consumer can detect a decoder-sourced fix by
/// the presence of the latter two fields plus a non-1.0 `recognition`.
#[derive(Debug, Serialize)]
pub struct AuditRecordJsonV2 {
    pub schema: &'static str,
    pub rule: String,
    pub source: &'static str,
    pub span: SpanJson,
    pub original: String,
    pub replacement: String,
    pub confidence: f32,
    pub migration_ref: Option<String>,
    pub timestamp: String,
    pub classifier_id: Option<String>,
    pub dry_run: bool,
    pub input: Option<String>,
    /// Recognition posterior, always present in v2. `1.0` for strict-path
    /// fixes (the strict grammar matched unambiguously), `<1.0` for
    /// decoder-sourced fixes.
    pub recognition: f32,
    /// Top-vs-runner-up posterior ratio for decoder-sourced fixes;
    /// `None` (omitted from JSON) for strict-path fixes with no
    /// candidate set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner_up_ratio: Option<f32>,
    /// Per-feature contributions to `recognition` for decoder-sourced
    /// fixes; empty Vec (omitted from JSON) for strict-path fixes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureJson>,
}

/// JSON projection of a [`FeatureContribution`] for the v2 audit record.
#[derive(Debug, Serialize)]
pub struct FeatureJson {
    pub id: &'static str,
    pub delta: f32,
}

fn feature_to_json(feature: &FeatureContribution) -> FeatureJson {
    FeatureJson {
        id: feature.id.as_str(),
        delta: feature.delta,
    }
}

fn fix_source_str(source: marque_rules::FixSource) -> &'static str {
    match source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
        marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
        marque_rules::FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

/// Convert an `AppliedFix` to the v1 JSON audit record shape.
pub fn applied_fix_to_audit_json_v1(fix: &AppliedFix) -> AuditRecordJsonV1 {
    AuditRecordJsonV1 {
        schema: AUDIT_SCHEMA_VERSION,
        rule: fix.proposal.rule.as_str().to_owned(),
        source: fix_source_str(fix.proposal.source),
        span: SpanJson {
            start: fix.proposal.span.start,
            end: fix.proposal.span.end,
        },
        original: fix.proposal.original.to_string(),
        replacement: fix.proposal.replacement.to_string(),
        confidence: fix.proposal.confidence.combined(),
        migration_ref: fix.proposal.migration_ref.map(|s| s.to_owned()),
        timestamp: humantime::format_rfc3339(fix.timestamp).to_string(),
        classifier_id: fix.classifier_id.as_ref().map(|s| s.to_string()),
        dry_run: fix.dry_run,
        input: fix.input.as_ref().map(|s| s.to_string()),
    }
}

/// Convert an `AppliedFix` to the v2 JSON audit record shape.
///
/// Reads `confidence` and `source` from the **top-level** snapshot fields
/// on `AppliedFix`, not from `proposal.*`. The two are identical copies
/// today (the engine's `__engine_promote` snapshots them unchanged), but
/// the v2 schema's contract — documented on `AppliedFix` itself — is that
/// v2 reads the snapshot so a future region-context adjustment landing in
/// the engine doesn't silently bypass v2 emission.
pub fn applied_fix_to_audit_json_v2(fix: &AppliedFix) -> AuditRecordJsonV2 {
    let c = &fix.confidence;
    AuditRecordJsonV2 {
        schema: AUDIT_SCHEMA_VERSION,
        rule: fix.proposal.rule.as_str().to_owned(),
        source: fix_source_str(fix.source),
        span: SpanJson {
            start: fix.proposal.span.start,
            end: fix.proposal.span.end,
        },
        original: fix.proposal.original.to_string(),
        replacement: fix.proposal.replacement.to_string(),
        confidence: c.combined(),
        migration_ref: fix.proposal.migration_ref.map(|s| s.to_owned()),
        timestamp: humantime::format_rfc3339(fix.timestamp).to_string(),
        classifier_id: fix.classifier_id.as_ref().map(|s| s.to_string()),
        dry_run: fix.dry_run,
        input: fix.input.as_ref().map(|s| s.to_string()),
        recognition: c.recognition,
        runner_up_ratio: c.runner_up_ratio,
        features: c.features.iter().map(feature_to_json).collect(),
    }
}

/// Emit a single audit record as NDJSON to `stderr`.
///
/// Dispatches to the v1 or v2 emitter based on `AUDIT_SCHEMA_IS_V2`,
/// the const-folded selector set by `crates/engine/build.rs`. Each
/// record is serialized to an in-memory buffer and flushed with a
/// single `write_all` ending in `\n` (FR-005a). A partially-serialized
/// record is never flushed; on serialization failure, emits an error
/// frame and returns `Err`.
pub fn render_audit_record(
    stderr: &mut dyn std::io::Write,
    fix: &AppliedFix,
) -> std::io::Result<()> {
    let serialized = if AUDIT_SCHEMA_IS_V2 {
        serde_json::to_vec(&applied_fix_to_audit_json_v2(fix))
    } else {
        serde_json::to_vec(&applied_fix_to_audit_json_v1(fix))
    };
    match serialized {
        Ok(mut buf) => {
            buf.push(b'\n');
            stderr.write_all(&buf)
        }
        Err(e) => {
            render_audit_error_frame(stderr, fix.proposal.rule.as_str(), &e.to_string())?;
            Err(std::io::Error::other(format!(
                "audit record serialization failed for rule {}: {e}",
                fix.proposal.rule
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
/// `MARQUE_AUDIT_SCHEMA` env var (default `marque-mvp-2`; see
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
    use marque_rules::{FixProposal, FixSource, RuleId, Severity};

    fn make_diagnostic(
        rule: &'static str,
        span: Span,
        message: &str,
        fix: Option<FixProposal>,
    ) -> Diagnostic {
        Diagnostic::new(
            RuleId::new(rule),
            Severity::Fix,
            span,
            message,
            "CAPCO-2016 §A.6",
            fix,
        )
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
        let fix = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            span,
            "NF".to_owned(),
            "NOFORN".to_owned(),
            marque_rules::Confidence::strict(1.0),
            None,
        );
        let diag = make_diagnostic(
            "E001",
            span,
            "banner uses abbreviated dissem control \"NF\"; use \"NOFORN\"",
            Some(fix),
        );

        let mut out = Vec::new();
        render_human(&mut out, "banner.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        // Header line: path:line:col with level + rule + message
        assert!(rendered.contains("banner.txt:1:17 fix[E001]"));
        assert!(rendered.contains("banner uses abbreviated dissem control"));
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
        // Hint includes the replacement
        assert!(rendered.contains("replace with \"NOFORN\""));
        assert!(rendered.contains("(confidence 100%)"));
        // Citation footer
        assert!(rendered.contains("= citation: CAPCO-2016 §A.6"));
    }

    #[test]
    fn render_human_without_color_has_no_ansi_escapes() {
        let src = b"TOP SECRET//SI//NF\n";
        let span = Span::new(16, 18);
        let diag = make_diagnostic("E001", span, "test", None);

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
        let diag = make_diagnostic("E001", span, "test", None);

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
        // MUST NOT carry the suffix — they keep `diag.message` byte-
        // identical so downstream tooling does not have to strip it.
        let src = b"TOP SECRET//SI//NF\n";
        let span = Span::new(16, 18);
        let diag = make_diagnostic("E001", span, "test message", None);

        let mut human_out = Vec::new();
        render_human(&mut human_out, "x.txt", src, &diag, false).unwrap();
        let human = String::from_utf8(human_out).unwrap();
        assert!(
            human.contains("test message —Marque"),
            "human header must end with \" —Marque\"; got:\n{human}"
        );

        // NDJSON path must not carry the brand suffix.
        let json = diagnostic_to_json(&diag);
        assert_eq!(
            json.message, "test message",
            "NDJSON `message` field must stay byte-identical to Diagnostic.message"
        );
        assert!(
            !json.message.contains("Marque"),
            "NDJSON message field must never be branded"
        );
    }

    #[test]
    fn render_human_diagnostic_without_fix_omits_hint() {
        // E008-style: no fix proposal, caret only
        let src = b"SECRET//XYZZY//NOFORN\n";
        let span = Span::new(8, 13);
        let diag = Diagnostic::new(
            RuleId::new("E008"),
            Severity::Error,
            span,
            "unrecognized token",
            "CAPCO-2016 §A.6",
            None,
        );

        let mut out = Vec::new();
        render_human(&mut out, "x.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert!(rendered.contains("^^^^^"));
        assert!(!rendered.contains("replace with"));
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
    fn render_audit_record_produces_valid_ndjson() {
        use marque_rules::{AppliedFix, EnginePromotionToken};
        use std::sync::Arc;
        use std::time::{Duration, UNIX_EPOCH};

        let fix = FixProposal::new(
            RuleId::new("E001"),
            FixSource::BuiltinRule,
            Span::new(8, 10),
            "NF",
            "NOFORN",
            marque_rules::Confidence::strict(1.0),
            Some("CAPCO-2016 §A.6"),
        );
        // Test-fixture carve-out per Constitution V Principle V:
        // synthetic AppliedFix for renderer unit testing only;
        // never commingled with engine output, never reachable from
        // cfg(not(test)). The token is minted via the engine-only
        // door for the same reason — the test exercises the audit
        // emitter, not the engine's promotion gate.
        let applied = AppliedFix::__engine_promote(
            fix,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            Some(Arc::from("classifier-42")),
            false,
            Some(Arc::from("test.txt")),
            EnginePromotionToken::__engine_construct(),
        );

        let mut buf = Vec::new();
        render_audit_record(&mut buf, &applied).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'));

        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(v["schema"], AUDIT_SCHEMA_VERSION);
        assert_eq!(v["rule"], "E001");
        assert_eq!(v["source"], "BuiltinRule");
        assert_eq!(v["span"]["start"], 8);
        assert_eq!(v["span"]["end"], 10);
        assert_eq!(v["original"], "NF");
        assert_eq!(v["replacement"], "NOFORN");
        assert_eq!(v["confidence"], 1.0);
        assert_eq!(v["migration_ref"], "CAPCO-2016 §A.6");
        assert_eq!(v["classifier_id"], "classifier-42");
        assert_eq!(v["dry_run"], false);
        assert_eq!(v["input"], "test.txt");
        // timestamp must be a valid RFC3339 string
        assert!(v["timestamp"].as_str().unwrap().contains('T'));

        // v2 contract: a strict-path fix has `recognition: 1.0` always
        // present, and `runner_up_ratio` / `features` omitted (skip when
        // empty / None). v1 builds (downgrade) drop all three new fields.
        if AUDIT_SCHEMA_IS_V2 {
            // `serde_json::Value` deserializes JSON numbers as `f64`, so
            // compare against `1.0` (f64) — matches the
            // `assert_eq!(v["confidence"], 1.0)` line above and avoids
            // f32→f64 widening surprises.
            assert_eq!(v["recognition"], 1.0);
            assert!(
                v.get("runner_up_ratio").is_none(),
                "strict-path v2 record must omit runner_up_ratio when None; \
                 got {:?}",
                v.get("runner_up_ratio")
            );
            assert!(
                v.get("features").is_none(),
                "strict-path v2 record must omit features when empty; got {:?}",
                v.get("features")
            );
        } else {
            assert!(
                v.get("recognition").is_none(),
                "v1 build must not emit recognition field; got {:?}",
                v.get("recognition")
            );
        }
    }
}
