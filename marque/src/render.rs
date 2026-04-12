//! Diagnostic rendering for the `marque` CLI.
//!
//! Two formats are supported:
//! - **human**: location-prefixed diagnostic header with citation, ANSI-coloured by default.
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

use marque_engine::LintResult;
use marque_rules::Diagnostic;
use serde::Serialize;
use std::path::Path;

/// Output format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Human,
    Json,
}

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

/// Render a single diagnostic in human format. Caller writes to stdout.
pub fn render_human(
    out: &mut dyn std::io::Write,
    path_label: &str,
    source: &[u8],
    diag: &Diagnostic,
    color: bool,
) -> std::io::Result<()> {
    let (line, col) = byte_to_line_col(source, diag.span.start);
    let level = match diag.severity {
        marque_rules::Severity::Error => "error",
        marque_rules::Severity::Warn => "warning",
        marque_rules::Severity::Fix => "fix",
        marque_rules::Severity::Off => "off", // unreachable in practice
    };
    let level_styled = if color {
        format!("\x1b[31;1m{level}\x1b[0m")
    } else {
        level.to_owned()
    };
    let rule_styled = if color {
        format!("\x1b[1m[{}]\x1b[0m", diag.rule)
    } else {
        format!("[{}]", diag.rule)
    };
    writeln!(
        out,
        "{path_label}:{line}:{col} {level_styled}{rule_styled} {}",
        diag.message
    )?;
    writeln!(out, "  citation: {}", diag.citation)?;
    if let Some(fix) = &diag.fix {
        writeln!(
            out,
            "  fix (confidence {:.0}%): {:?}",
            fix.confidence * 100.0,
            fix.replacement,
        )?;
    }
    Ok(())
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
            },
            replacement: f.replacement.as_ref(),
            confidence: f.confidence,
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
