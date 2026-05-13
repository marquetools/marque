// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T030 — mechanical extraction tool.
//!
//! Walks the post-PR-3b CAPCO rule catalog plus `marque-engine` and
//! extracts every literal string that becomes a `Diagnostic.message`
//! field. Clusters them by structural similarity (placeholder shape)
//! and emits a starter doc that T031 hand-curates into the closed
//! `MessageTemplate` enum.
//!
//! Standalone binary, NOT a workspace member, per Constitution III.
//! The tool deletes after PR 3c.1 review accepts the curated enum.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use walkdir::WalkDir;

/// Where in source a captured literal lives.
#[derive(Debug, Clone)]
struct CaptureSite {
    file: PathBuf,
    line: usize,
}

/// One captured message literal.
#[derive(Debug, Clone)]
struct Capture {
    raw: String,
    site: CaptureSite,
    /// Capture-site classification — drives clustering. One of
    /// `"diagnostic-struct-init"` (a literal `Diagnostic { message: ... }`),
    /// `"diagnostic-helper-struct-init"` (a `*Diagnostic*` /
    /// `FixDiag*` helper-struct init with a `message` field, e.g.
    /// `FixDiagnosticParams { message: ..., .. }`), or
    /// `"diagnostic-new-arg"` (a positional arg in `Diagnostic::new(...)`).
    /// The format-macro paths (`format!`/`write!`/etc.) are walked
    /// inside `extract_message_expr` so they appear under the same
    /// kind as the surrounding struct-init or `::new` call site.
    kind: &'static str,
}

/// Visitor that walks each source file's AST.
struct Extractor {
    file: PathBuf,
    captures: Vec<Capture>,
}

impl Extractor {
    fn new(file: PathBuf) -> Self {
        Self {
            file,
            captures: Vec::new(),
        }
    }

    fn line_of(&self, span: Span) -> usize {
        span.start().line
    }

    fn record(&mut self, raw: String, span: Span, kind: &'static str) {
        self.captures.push(Capture {
            raw,
            site: CaptureSite {
                file: self.file.clone(),
                line: self.line_of(span),
            },
            kind,
        });
    }

    /// Extract the raw string from a `syn::Expr` if it is a string-
    /// literal (possibly wrapped in `format!`/`format_args!`/`write!`/
    /// `writeln!`). Returns `None` for non-literal expressions.
    fn extract_message_expr(&self, expr: &syn::Expr) -> Option<(String, Span)> {
        match expr {
            // Bare string literal: `"text"`.
            syn::Expr::Lit(expr_lit) => {
                if let syn::Lit::Str(lit) = &expr_lit.lit {
                    Some((lit.value(), expr_lit.span()))
                } else {
                    None
                }
            }
            // `format!("text", ...)` / `format_args!(...)` / `write!(_, "text", ...)` / etc.
            syn::Expr::Macro(em) => {
                let mac = &em.mac;
                let last_seg = mac.path.segments.last()?;
                let name = last_seg.ident.to_string();
                let is_msg_macro = matches!(
                    name.as_str(),
                    "format" | "format_args" | "write" | "writeln" | "println" | "eprintln"
                );
                if !is_msg_macro {
                    return None;
                }
                // Parse the macro body as a comma-separated list of
                // expressions and return the first `LitStr`. Using
                // `syn::LitStr::value()` properly unescapes and handles
                // raw strings (`r#"..."#`), `\u{...}` escapes, and
                // multi-byte UTF-8 — the previous byte-level token-
                // string scan corrupted these, producing the mojibake
                // observed in the starter doc (e.g. `→` rendered as
                // `â^F^R`). For `write!(buf, "fmt", ...)` the first
                // string literal is the second arg; the iterator scan
                // skips over the writer expression naturally.
                let parser =
                    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
                let args = mac.parse_body_with(parser).ok()?;
                for arg in args.iter() {
                    if let syn::Expr::Lit(expr_lit) = arg
                        && let syn::Lit::Str(lit) = &expr_lit.lit
                    {
                        return Some((lit.value(), expr_lit.span()));
                    }
                }
                None
            }
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for Extractor {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        // Match any struct-init with a `message` field whose value is
        // a string literal or a format!-family macro. This covers
        // both `Diagnostic { message: ..., ... }` and helper structs
        // like `FixDiagnosticParams { message: ..., ... }` that wrap
        // diagnostic construction.
        let path_name = node
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        let kind = if path_name == "Diagnostic" {
            "diagnostic-struct-init"
        } else if path_name.contains("Diagnostic") || path_name.contains("FixDiag") {
            "diagnostic-helper-struct-init"
        } else {
            // Skip non-diagnostic structs to avoid noise from
            // unrelated types that happen to have a message field.
            visit::visit_expr_struct(self, node);
            return;
        };
        for field in &node.fields {
            if let syn::Member::Named(ref ident) = field.member
                && ident == "message"
                && let Some((raw, span)) = self.extract_message_expr(&field.expr)
            {
                self.record(raw, span, kind);
            }
        }
        visit::visit_expr_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        // Look for `Diagnostic::new(rule, severity, span, message, citation, fix)`.
        // Match on the call function's last path segment to handle qualified
        // and unqualified call sites.
        let is_diagnostic_new = match &*node.func {
            syn::Expr::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident == "new")
                .unwrap_or(false)
                && p.path.segments.len() >= 2
                && p.path.segments[p.path.segments.len() - 2].ident == "Diagnostic",
            _ => false,
        };
        if is_diagnostic_new {
            // Diagnostic::new signature: rule, severity, span, message, citation, fix.
            // message is positional index 3.
            if let Some(arg) = node.args.iter().nth(3)
                && let Some((raw, span)) = self.extract_message_expr(arg)
            {
                self.record(raw, span, "diagnostic-new-arg");
            }
        }
        visit::visit_expr_call(self, node);
    }
}

/// Replace placeholder substrings (`{...}`, `{}`) with the literal
/// marker `{}` so format strings that interpolate different variable
/// names cluster together.
///
/// Iterates over `chars()` rather than bytes so multi-byte UTF-8
/// sequences (`→`, `—`, etc.) round-trip cleanly — the previous
/// byte-level cast `bytes[i] as char` corrupted these into Latin-1
/// garbage.
///
/// `{{` and `}}` are Rust format-string escapes for literal `{` and
/// `}` respectively (per `core::fmt` syntax). They are preserved as
/// `{{` / `}}` in the normalized output rather than counted as
/// placeholders. Format-spec syntax (`{name:>5.2}`) does not nest
/// `{` inside the spec, so a flat scan to the next `}` is correct.
fn normalize_placeholders(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push_str("{{");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push_str("}}");
            }
            '{' => {
                // Consume up to and including the matching `}`.
                while let Some(inner) = chars.next() {
                    if inner == '}' {
                        break;
                    }
                }
                out.push_str("{}");
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_handles_double_open_and_close_brace_escapes() {
        // `{{` and `}}` are literal `{` and `}` in Rust format-string
        // syntax. They MUST NOT be counted as placeholders.
        let n = normalize_placeholders("literal {{ and }} braces");
        assert_eq!(n, "literal {{ and }} braces");
        // Round-trip: count `{}` substrings to verify zero
        // placeholders are inflated.
        assert_eq!(n.matches("{}").count(), 0);
    }

    #[test]
    fn normalize_replaces_named_placeholders_with_marker() {
        let n = normalize_placeholders("hello {name}, age {age:>3}");
        assert_eq!(n, "hello {}, age {}");
        assert_eq!(n.matches("{}").count(), 2);
    }

    #[test]
    fn normalize_preserves_multi_byte_utf8() {
        // The previous byte-level implementation produced mojibake
        // here (`→` was three UTF-8 bytes cast individually to char).
        let n = normalize_placeholders("foo → bar — {x}");
        assert_eq!(n, "foo → bar — {}");
    }

    #[test]
    fn normalize_handles_mix_of_escapes_and_placeholders() {
        let n = normalize_placeholders("set {{key}} = {value}");
        assert_eq!(n, "set {{key}} = {}");
        assert_eq!(n.matches("{}").count(), 1);
    }
}

fn collect_targets(workspace_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let candidate_dirs = [
        workspace_root.join("crates/capco/src"),
        workspace_root.join("crates/engine/src"),
    ];
    for dir in candidate_dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir).into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if p.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            // capco: rules.rs, rules_*.rs only; everything else under capco/src is skipped.
            // engine: engine.rs only.
            let file_name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let parent_marker = p
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let in_capco = parent_marker == "capco";
            let in_engine = parent_marker == "engine";
            let keep = (in_capco && (file_name == "rules.rs" || file_name.starts_with("rules_")))
                || (in_engine && file_name == "engine.rs");
            if keep {
                out.push(p.to_path_buf());
            }
        }
    }
    out.sort();
    out
}

fn extract_file(path: &Path) -> Result<Vec<Capture>, String> {
    let src = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let file = syn::parse_file(&src).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let mut extractor = Extractor::new(path.to_path_buf());
    extractor.visit_file(&file);
    Ok(extractor.captures)
}

fn render_starter_doc(
    workspace_root: &Path,
    clusters: &BTreeMap<String, Vec<Capture>>,
) -> String {
    let mut out = String::new();
    // Reuse detects this as an invalid copyright (duplicating the one above)
    // This makes REUSE ignore them.
    // REUSE-IgnoreStart
    out.push_str("<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->\n");
    out.push_str("<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->\n\n");
    // REUSE-IgnoreEnd
    out.push_str("# MessageTemplate Starter (T030 mechanical extraction)\n\n");
    out.push_str(
        "Generated by `tools/message-template-extract/` against the post-PR-3b\n\
         rule catalog plus `marque-engine`. Hand-curated into the closed\n\
         `MessageTemplate` enum at T031. Each cluster groups source-site\n\
         literals that share placeholder shape after `{...}` normalization.\n\n",
    );
    out.push_str(
        "**Curation rules** (per design doc §3 T031):\n\n\
         1. One closed-enum variant per *semantic* (not per per-rule literal).\n\
            If two clusters convey the same diagnostic intent, collapse them.\n\
         2. Each variant's `MessageArgs` field set MUST be in the closed\n\
            permitted set: `TokenId`, `CategoryId`, `Span`, `Blake3Hash`,\n\
            `Confidence`, `FeatureId`. No `String`, no `&str`, no `Vec<u8>`.\n\
         3. Each variant carries a `// CAPCO-2016 §X.Y pNN` doc comment when\n\
            mapped to a specific marking semantic. Verify the citation against\n\
            `crates/capco/docs/CAPCO-2016.md` before committing the variant.\n\
         4. Engine-synthetic variants (R001/R002, corrections-map, decoder\n\
            heuristics) carry no §-citation.\n\n",
    );
    out.push_str(&format!(
        "Total clusters: **{}**. Total capture sites: **{}**.\n\n",
        clusters.len(),
        clusters.values().map(|v| v.len()).sum::<usize>(),
    ));
    out.push_str("---\n\n");

    for (i, (template, sites)) in clusters.iter().enumerate() {
        out.push_str(&format!("## Cluster {} — `{}`\n\n", i + 1, template));
        out.push_str("Sites:\n\n");
        for capture in sites {
            let rel = capture
                .site
                .file
                .strip_prefix(workspace_root)
                .unwrap_or(&capture.site.file);
            out.push_str(&format!(
                "- `{}:{}` ({})\n",
                rel.display(),
                capture.site.line,
                capture.kind
            ));
        }
        out.push_str("\n");
        // Placeholder count = count of {} markers in the normalized template.
        let placeholder_count = template.matches("{}").count();
        out.push_str(&format!(
            "Placeholder count: **{placeholder_count}**.\n\n\
             Suggested variant: _TBD by T031 curator._\n\n\
             Suggested args: _TBD by T031 curator (must be subset of \
             `{{token, category, span, digest, confidence, expected_token, \
             actual_token, feature_ids}}`)._\n\n\
             ---\n\n",
        ));
    }

    out
}

fn parse_args() -> Result<(PathBuf, PathBuf), String> {
    let mut workspace_root: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--workspace-root" => {
                workspace_root = Some(PathBuf::from(args.next().ok_or_else(|| {
                    "--workspace-root requires a value".to_string()
                })?));
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or_else(|| "--output requires a value".to_string())?,
                ));
            }
            "--help" | "-h" => {
                println!(
                    "Usage: message-template-extract --workspace-root <path> --output <path>"
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!("unknown arg: {other}"));
            }
        }
    }
    let workspace_root = workspace_root
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| "could not determine workspace root".to_string())?;
    let output = output.ok_or_else(|| "--output is required".to_string())?;
    Ok((workspace_root, output))
}

fn run() -> Result<(), String> {
    let (workspace_root, output) = parse_args()?;
    let workspace_root = fs::canonicalize(&workspace_root)
        .map_err(|e| format!("canonicalize workspace root {}: {e}", workspace_root.display()))?;

    let targets = collect_targets(&workspace_root);
    if targets.is_empty() {
        return Err(format!(
            "no target source files found under {}",
            workspace_root.display()
        ));
    }
    eprintln!("scanning {} files", targets.len());

    let mut all_captures: Vec<Capture> = Vec::new();
    for path in &targets {
        match extract_file(path) {
            Ok(caps) => all_captures.extend(caps),
            Err(e) => eprintln!("warn: {e}"),
        }
    }
    eprintln!("captured {} message literals", all_captures.len());

    // Cluster by normalized template.
    let mut clusters: BTreeMap<String, Vec<Capture>> = BTreeMap::new();
    for capture in all_captures {
        let key = normalize_placeholders(&capture.raw);
        clusters.entry(key).or_default().push(capture);
    }
    // Sort each cluster's sites for stable output.
    for sites in clusters.values_mut() {
        sites.sort_by(|a, b| {
            a.site
                .file
                .cmp(&b.site.file)
                .then_with(|| a.site.line.cmp(&b.site.line))
        });
    }
    eprintln!("clustered into {} templates", clusters.len());

    let doc = render_starter_doc(&workspace_root, &clusters);

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create_dir_all {}: {e}", parent.display()))?;
    }
    fs::write(&output, doc).map_err(|e| format!("write {}: {e}", output.display()))?;
    eprintln!("wrote {}", output.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
