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
use marque_engine::{AUDIT_SCHEMA_IS_V3, AUDIT_SCHEMA_VERSION, LintResult};
use marque_rules::{AppliedFix, AppliedFixProposal, Diagnostic, FeatureContribution};
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
        rule: d.rule.as_str(),
        severity: d.severity.as_str(),
        span: SpanJson {
            start: d.span.start,
            end: d.span.end,
        },
        message: d.message.as_ref(),
        citation: d.citation,
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
// Audit record NDJSON (marque-mvp-3)
//
// The `schema` field is sourced from `marque_engine::AUDIT_SCHEMA_VERSION`,
// which `crates/engine/build.rs` validates against the closed accept-list
// `["marque-mvp-3"]`. The pre-Commit-10 `mvp-1` / `mvp-2` shapes (with
// top-level `original` / `replacement` byte fields) retired alongside
// `FixProposal` to close the G13 audit-content-ignorance channel.
//
// FR-014 (single-schema-per-build) is upheld at two layers:
//   1. `crates/engine/build.rs` panics on unknown values — only the
//      accepted version can reach the emitter.
//   2. The emitter chooses the matching struct shape from the const.
// ---------------------------------------------------------------------------

/// JSON projection of the `proposal` sub-object on a `marque-mvp-3`
/// audit record. Discriminated by `kind`:
///
///   - `"kind": "FixIntent"` carries a structural fact-set delta.
///   - `"kind": "TextCorrection"` carries the canonical replacement
///     bytes for the C001 / `[corrections]` map path (a Constitution
///     V Principle V permitted identifier — corpus-derived token
///     canonical, never document content).
#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum ProposalJson {
    FixIntent { intent: serde_json::Value },
    TextCorrection { replacement: String },
}

/// JSON projection of an `AppliedFix` conforming to the
/// `marque-mvp-3` audit-record contract.
#[derive(Debug, Serialize)]
pub struct AuditRecordJsonV3 {
    pub schema: &'static str,
    pub rule: String,
    pub source: &'static str,
    pub span: SpanJson,
    pub proposal: ProposalJson,
    pub confidence: f32,
    pub migration_ref: Option<String>,
    pub timestamp: String,
    pub classifier_id: Option<String>,
    pub dry_run: bool,
    pub input: Option<String>,
    /// Recognition posterior. `1.0` for strict-path fixes;
    /// `<1.0` for decoder-sourced fixes.
    pub recognition: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner_up_ratio: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureJson>,
}

/// JSON projection of a [`FeatureContribution`] for the audit record.
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

/// Schema-pinned JSON projection of `CapcoOpenVocabRef`. Each variant
/// is encoded as a discriminated object so downstream consumers can
/// parse without Debug-string heuristics — `Debug` would not be a
/// stable wire format (variant renames or `#[derive(Debug)]`
/// regenerations would silently change the JSON). The string
/// payloads are SAR program identifiers, SCI compartment names, FGI
/// tetragraphs, and structural CountryCodes — all on Constitution V
/// Principle V's permitted-identifier list (token canonicals from
/// agency-allocated vocabularies + structural codes).
fn open_vocab_ref_to_json(r: &marque_capco::CapcoOpenVocabRef) -> serde_json::Value {
    match r {
        marque_capco::CapcoOpenVocabRef::Sar(name) => serde_json::json!({
            "kind": "Sar",
            "name": name.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::SciCompartment(name) => serde_json::json!({
            "kind": "SciCompartment",
            "name": name.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::SciSubCompartment(name) => serde_json::json!({
            "kind": "SciSubCompartment",
            "name": name.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::FgiTetragraph(code) => serde_json::json!({
            "kind": "FgiTetragraph",
            "code": code.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::CountryCode(c) => serde_json::json!({
            "kind": "CountryCode",
            "code": c.as_str(),
        }),
    }
}

/// JSON projection of a `FactRef<CapcoScheme>`. Discriminated by
/// `kind`. Constitution V Principle V permits emitting CVE token IDs
/// and category IDs (closed-vocabulary identifiers) and open-vocab
/// canonical refs (vocab-canonical, not document bytes) in audit
/// output — these are explicitly on the permitted-identifier list.
fn fact_ref_to_json(fact: &marque_scheme::FactRef<CapcoScheme>) -> serde_json::Value {
    match fact {
        marque_scheme::FactRef::Cve(token_id) => serde_json::json!({
            "kind": "Cve",
            "token_id": token_id.0,
        }),
        marque_scheme::FactRef::OpenVocab(r) => serde_json::json!({
            "kind": "OpenVocab",
            "ref": open_vocab_ref_to_json(r),
        }),
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

/// Schema-pinned string projection of `Scope`. Used in the audit JSON
/// `proposal.intent.scope` field — `Debug` would not be a stable wire
/// format (small refactors / variant renames would change the JSON
/// silently).
fn scope_str(scope: marque_scheme::Scope) -> &'static str {
    match scope {
        marque_scheme::Scope::Portion => "Portion",
        marque_scheme::Scope::Page => "Page",
        marque_scheme::Scope::Document => "Document",
        marque_scheme::Scope::Diff => "Diff",
    }
}

/// Schema-pinned string projection of `RecanonScope`. Same rationale
/// as [`scope_str`].
fn recanon_scope_str(scope: marque_scheme::fix_intent::RecanonScope) -> &'static str {
    match scope {
        marque_scheme::fix_intent::RecanonScope::Portion => "Portion",
        marque_scheme::fix_intent::RecanonScope::Page => "Page",
        marque_scheme::fix_intent::RecanonScope::Document => "Document",
    }
}

fn proposal_to_json(proposal: &AppliedFixProposal<CapcoScheme>) -> ProposalJson {
    match proposal {
        AppliedFixProposal::FixIntent(intent) => {
            // FixIntent doesn't `derive(Serialize)`; encode the
            // replacement variant + scope into a structured JSON
            // object that downstream consumers can match on. The
            // shape mirrors the `ReplacementIntent` enum
            // discriminator.
            let inner: serde_json::Value = match &intent.replacement {
                marque_scheme::ReplacementIntent::FactAdd { token, scope } => {
                    serde_json::json!({
                        "kind": "FactAdd",
                        "scope": scope_str(*scope),
                        "token": fact_ref_to_json(token),
                    })
                }
                marque_scheme::ReplacementIntent::FactRemove { scope, facts } => {
                    let facts_json: Vec<serde_json::Value> =
                        facts.iter().map(fact_ref_to_json).collect();
                    serde_json::json!({
                        "kind": "FactRemove",
                        "scope": scope_str(*scope),
                        "facts": facts_json,
                    })
                }
                marque_scheme::ReplacementIntent::Recanonicalize { scope } => {
                    serde_json::json!({
                        "kind": "Recanonicalize",
                        "scope": recanon_scope_str(*scope),
                    })
                }
                _ => {
                    tracing::warn!(
                        target: "marque::render",
                        "unrecognized ReplacementIntent variant in audit projection; downstream consumers will see kind=\"Unknown\""
                    );
                    serde_json::json!({ "kind": "Unknown" })
                }
            };
            ProposalJson::FixIntent { intent: inner }
        }
        AppliedFixProposal::TextCorrection { replacement } => ProposalJson::TextCorrection {
            replacement: replacement.to_string(),
        },
    }
}

/// Convert an `AppliedFix` to the v3 JSON audit record shape.
pub fn applied_fix_to_audit_json_v3(fix: &AppliedFix<CapcoScheme>) -> AuditRecordJsonV3 {
    let c = &fix.confidence;
    AuditRecordJsonV3 {
        schema: AUDIT_SCHEMA_VERSION,
        rule: fix.rule.as_str().to_owned(),
        source: fix_source_str(fix.source),
        span: SpanJson {
            start: fix.span.start,
            end: fix.span.end,
        },
        proposal: proposal_to_json(&fix.proposal),
        confidence: c.combined(),
        migration_ref: fix.migration_ref.map(|s| s.to_owned()),
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
/// Single accepted schema (`marque-mvp-3`) so dispatch is a no-op;
/// the const lookup is kept so a future schema bump can land via the
/// same dispatch shape without restructuring callers.
pub fn render_audit_record(
    stderr: &mut dyn std::io::Write,
    fix: &AppliedFix<CapcoScheme>,
) -> std::io::Result<()> {
    let _ = AUDIT_SCHEMA_IS_V3;
    let serialized = serde_json::to_vec(&applied_fix_to_audit_json_v3(fix));
    match serialized {
        Ok(mut buf) => {
            buf.push(b'\n');
            stderr.write_all(&buf)
        }
        Err(e) => {
            render_audit_error_frame(stderr, fix.rule.as_str(), &e.to_string())?;
            Err(std::io::Error::other(format!(
                "audit record serialization failed for rule {}: {e}",
                fix.rule
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
    use marque_rules::{
        FixIntent, FixSource, Message, MessageArgs, MessageTemplate, RuleId, Severity,
    };
    use marque_scheme::{ReplacementIntent, fix_intent::RecanonScope};

    fn make_diagnostic(
        rule: &'static str,
        span: Span,
        message: &str,
        fix: Option<FixIntent<CapcoScheme>>,
    ) -> Diagnostic<CapcoScheme> {
        Diagnostic::new(
            RuleId::new(rule),
            Severity::Fix,
            span,
            message,
            "CAPCO-2016 §A.6",
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
        // Post-Commit-10: FixIntent carries no replacement bytes (the
        // renderer is on the diagnostic path; no engine projection
        // available). The hint shows the auto-fixable signal +
        // confidence, not the literal replacement.
        assert!(
            rendered.contains("auto-fixable; confidence 100%"),
            "expected auto-fixable hint; got:\n{rendered}"
        );
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
            RuleId::new("S004"),
            Severity::Suggest,
            span,
            "\"AUT\" (Austria) is far less common in REL TO than \
             \"AUS\" (Australia); did you mean \"AUS\"?",
            "CAPCO-2016 §H.8 p150",
            Some(fix),
        );

        let mut out = Vec::new();
        render_human(&mut out, "rel.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        // Header carries the "suggest" level string, not "error" / "fix".
        assert!(
            rendered.contains("suggest[S004]"),
            "header must read suggest[S004]; got:\n{rendered}"
        );
        // Caret hint uses "did you mean" phrasing (Suggest-specific) —
        // not the imperative "replace with" used by Fix-severity rules.
        assert!(
            rendered.contains("did you mean \"AUS\""),
            "Suggest hint must read \"did you mean ...\"; got:\n{rendered}"
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
            RuleId::new("S004"),
            Severity::Suggest,
            span,
            "did you mean \"AUS\"?",
            "CAPCO-2016 §H.8 p150",
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
            RuleId::new("S999"),
            Severity::Suggest,
            span,
            "REL TO list contains an opaque tetragraph; \
             release decision may be ambiguous",
            "TEST",
            None,
        );

        let mut out = Vec::new();
        render_human(&mut out, "rel.txt", src, &diag, false).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        assert!(
            rendered.contains("suggest[S999]"),
            "Suggest with no fix still renders header at suggest level"
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
            RuleId::new("S004"),
            Severity::Suggest,
            span,
            "did you mean \"AUS\"?",
            "CAPCO-2016 §H.8 p150",
            Some(fix),
        );

        let json = diagnostic_to_json(&diag);
        assert_eq!(json.severity, "suggest");
        assert_eq!(json.rule, "S004");
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
    fn render_audit_record_produces_valid_ndjson() {
        use marque_ism::Span;
        use marque_rules::{AppliedFix, EnginePromotionToken, RuleId};
        use std::sync::Arc;
        use std::time::{Duration, UNIX_EPOCH};

        let fix = make_intent_fix();
        // Test-fixture carve-out per Constitution V Principle V.
        let token = EnginePromotionToken::__engine_construct();
        let applied = AppliedFix::__engine_promote(
            RuleId::new("E002"),
            Span::new(8, 10),
            fix,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            Some(Arc::from("classifier-42")),
            false,
            Some(Arc::from("test.txt")),
            token,
        );

        let mut buf = Vec::new();
        render_audit_record(&mut buf, &applied).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with('\n'));

        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(v["schema"], AUDIT_SCHEMA_VERSION);
        assert_eq!(v["rule"], "E002");
        assert_eq!(v["source"], "BuiltinRule");
        assert_eq!(v["span"]["start"], 8);
        assert_eq!(v["span"]["end"], 10);
        assert_eq!(v["proposal"]["kind"], "FixIntent");
        assert_eq!(v["confidence"], 1.0);
        assert_eq!(v["classifier_id"], "classifier-42");
        assert_eq!(v["dry_run"], false);
        assert_eq!(v["input"], "test.txt");
        // timestamp must be a valid RFC3339 string
        assert!(v["timestamp"].as_str().unwrap().contains('T'));

        // mvp-3: a strict-path fix has `recognition: 1.0` always
        // present, and `runner_up_ratio` / `features` omitted (skip
        // when empty / None).
        #[allow(clippy::assertions_on_constants)]
        // Drift-gate: failure here means the build-time const desynced from the schema literal.
        {
            assert!(AUDIT_SCHEMA_IS_V3);
        }
        assert_eq!(v["recognition"], 1.0);
        assert!(
            v.get("runner_up_ratio").is_none(),
            "strict-path record must omit runner_up_ratio when None"
        );
        assert!(
            v.get("features").is_none(),
            "strict-path record must omit features when empty"
        );
    }
}
