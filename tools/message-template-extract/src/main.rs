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
    /// One of "diagnostic-struct-init" / "diagnostic-new-arg" /
    /// "format-near-diagnostic". Drives clustering.
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
                // Parse macro tokens looking for the first string literal.
                // For `write!(buf, "fmt", ...)` the first arg is a writer;
                // for `format!("fmt", ...)` the first arg IS the format string.
                // Accept the first string literal regardless of position.
                let tokens = mac.tokens.to_string();
                let mut chars = tokens.chars().peekable();
                let mut buf = String::new();
                let mut in_str = false;
                let mut escape = false;
                while let Some(c) = chars.next() {
                    if !in_str {
                        if c == '"' {
                            in_str = true;
                        }
                        continue;
                    }
                    if escape {
                        buf.push(c);
                        escape = false;
                        continue;
                    }
                    match c {
                        '\\' => {
                            buf.push(c);
                            escape = true;
                        }
                        '"' => {
                            return Some((buf, mac.path.span()));
                        }
                        _ => buf.push(c),
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
fn normalize_placeholders(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            // `{{` is a literal `{`; preserve.
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                out.push_str("{{");
                i += 2;
                continue;
            }
            // Find matching `}`.
            let mut depth = 1;
            let mut j = i + 1;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            out.push_str("{}");
            i = j;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
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
    out.push_str("<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->\n");
    out.push_str("<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->\n\n");
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
