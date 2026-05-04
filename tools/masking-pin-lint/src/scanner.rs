// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! AST walker that finds every `with_recognizer(...StrictRecognizer...)` call
//! site under `<workspace>/tests/` and `<workspace>/crates/*/tests/`, and
//! classifies the comment-marker on each.
//!
//! Per FR-039 the scanner intentionally does NOT walk `crates/*/benches/`;
//! benchmark pins are out of FR-039 scope.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use proc_macro2::Span;
use regex::Regex;
use syn::visit::Visit;
use walkdir::WalkDir;

use crate::pin::{Pin, PinKind};

/// Maximum distance, in lines, between a marker comment and its associated
/// call site. Per source-plan §6 rules 1–2 ("within 5 lines"), the inclusive
/// window is `[call_line - LOOKBACK ..= call_line]` — so the marker may
/// appear up to 5 lines *above* the call (a window of 6 source lines total).
const LOOKBACK: u32 = 5;

/// Walk the workspace and return every pin site found in test files.
///
/// `workspace_dir` is the marque repo root. Test files searched:
///
/// - `<workspace>/tests/**/*.rs`
/// - `<workspace>/crates/*/tests/**/*.rs`
///
/// Errors are returned only for I/O or AST-parse failures the caller cannot
/// recover from. Individual call-site classifications (including
/// `Unmarked`/`BadFormat`) are encoded in the returned `Pin` values, not as
/// errors.
pub fn scan_workspace(workspace_dir: &Path) -> Result<Vec<Pin>> {
    let mut pins = Vec::new();
    let test_roots = collect_test_roots(workspace_dir);
    for root in test_roots {
        for entry in WalkDir::new(&root).follow_links(false) {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("warning: walkdir error under {}: {err}", root.display());
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            // Skip target/ and other build artifacts that may sit under tests/.
            if path.components().any(|c| c.as_os_str() == "target") {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            scan_file(path, &mut pins)?;
        }
    }
    Ok(pins)
}

/// Collect the set of test directory roots beneath `workspace_dir`.
fn collect_test_roots(workspace_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let top_tests = workspace_dir.join("tests");
    if top_tests.is_dir() {
        roots.push(top_tests);
    }
    let crates_dir = workspace_dir.join("crates");
    if let Ok(read) = fs::read_dir(&crates_dir) {
        for entry in read.flatten() {
            let crate_tests = entry.path().join("tests");
            if crate_tests.is_dir() {
                roots.push(crate_tests);
            }
        }
    }
    roots
}

/// Parse one `.rs` file and append any pin sites found to `out`.
fn scan_file(path: &Path, out: &mut Vec<Pin>) -> Result<()> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let lines: Vec<&str> = source.lines().collect();
    let parsed = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(err) => {
            // Don't fail the run for an unparseable test fixture.
            // Report and move on.
            eprintln!(
                "warning: syn::parse_file failed for {}: {err}",
                path.display()
            );
            return Ok(());
        }
    };
    let mut visitor = CallSiteVisitor::new(path, &lines);
    visitor.visit_file(&parsed);
    out.append(&mut visitor.found);
    Ok(())
}

/// AST visitor that records every method-call expression matching
/// `expr.with_recognizer( <something containing StrictRecognizer> )`.
struct CallSiteVisitor<'a> {
    file: &'a Path,
    lines: &'a [&'a str],
    found: Vec<Pin>,
    masking_re: Regex,
    intentional_re: Regex,
}

impl<'a> CallSiteVisitor<'a> {
    fn new(file: &'a Path, lines: &'a [&'a str]) -> Self {
        // Compile-time-stable regexes; failing to compile is a programmer
        // error and warrants `unwrap`.
        let masking_re =
            Regex::new(r"//\s*MASKING-PIN:\s*tracks\s*#(?P<n>\d+)(?:\s*[—-]\s*(?P<reason>.+))?")
                .expect("masking-pin regex compiles");
        let intentional_re = Regex::new(r"//\s*INTENTIONAL-STRICT:\s*(?P<reason>.+)")
            .expect("intentional-strict regex compiles");
        Self {
            file,
            lines,
            found: Vec::new(),
            masking_re,
            intentional_re,
        }
    }

    /// Classify the 5-line lookback window (inclusive of the call-site line)
    /// for marker presence.
    fn classify_window(&self, call_line: u32) -> PinKind {
        // Inclusive window: lines [call_line - LOOKBACK ..= call_line].
        // 1-indexed: line N is index N-1. Per source-plan §6, "within 5
        // lines" means the marker may appear up to 5 lines above the call.
        let start = call_line.saturating_sub(LOOKBACK).max(1);
        let end = call_line;
        let mut masking_hit: Option<(u32, String)> = None;
        let mut intentional_hit: Option<String> = None;
        let mut bad_format: Option<String> = None;

        for ln in start..=end {
            let idx = ln.saturating_sub(1) as usize;
            let Some(line) = self.lines.get(idx) else {
                continue;
            };
            // MASKING-PIN
            if line.contains("MASKING-PIN") {
                if let Some(caps) = self.masking_re.captures(line) {
                    let issue: u32 = caps
                        .name("n")
                        .and_then(|m| m.as_str().parse().ok())
                        .unwrap_or(0);
                    let reason = caps
                        .name("reason")
                        .map_or_else(String::new, |m| m.as_str().trim().to_string());
                    masking_hit = Some((issue, reason));
                } else {
                    bad_format = Some((*line).to_string());
                }
            }
            // INTENTIONAL-STRICT
            if line.contains("INTENTIONAL-STRICT") {
                if let Some(caps) = self.intentional_re.captures(line) {
                    let reason = caps["reason"].trim().to_string();
                    intentional_hit = Some(reason);
                } else {
                    bad_format = Some((*line).to_string());
                }
            }
        }

        match (masking_hit, intentional_hit, bad_format) {
            (Some(_), Some(_), _) => PinKind::BothMarkers,
            (Some((issue, reason)), None, _) => PinKind::Masking { issue, reason },
            (None, Some(reason), _) => PinKind::IntentionalStrict { reason },
            (None, None, Some(line)) => PinKind::BadFormat(line),
            (None, None, None) => PinKind::Unmarked,
        }
    }

    fn record(&mut self, span: Span) {
        let start = span.start();
        // proc-macro2 line/column are 1-indexed line, 0-indexed column.
        let line = u32::try_from(start.line).unwrap_or(0);
        let column = u32::try_from(start.column).unwrap_or(0).saturating_add(1);
        let kind = self.classify_window(line);
        self.found.push(Pin {
            file: self.file.to_path_buf(),
            line,
            column,
            kind,
        });
    }
}

impl<'ast> Visit<'ast> for CallSiteVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "with_recognizer" {
            // Single-arg or multi-arg: search every argument expression for
            // a path whose terminal segment is `StrictRecognizer`.
            let any_strict = node.args.iter().any(expr_contains_strict_recognizer);
            if any_strict {
                // Record the position of the `with_recognizer` ident itself,
                // not the start of the receiver expression. Chained builders
                // like `Engine::new(...).with_recognizer(...)` would otherwise
                // report the line where `Engine::new` starts, several lines
                // above the actual `.with_recognizer` call — a developer
                // placing the marker comment immediately above
                // `.with_recognizer` would then fall outside the 5-line
                // proximity window. Using `node.method.span()` matches
                // intuition.
                self.record(node.method.span());
            }
        }
        // Continue visiting nested expressions even after a hit so that
        // nested method-call chains are still discovered.
        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Recursively check whether `expr` mentions a path ending in `StrictRecognizer`.
///
/// Handles the call shapes that show up in real test files:
///
/// - `StrictRecognizer::new()`
/// - `Arc::new(StrictRecognizer::new())`
/// - `Box::new(StrictRecognizer::new())`
/// - `std::sync::Arc::new(marque_engine::StrictRecognizer::new())`
/// - `(StrictRecognizer::new())` (parenthesized)
/// - `&StrictRecognizer::new()` (reference)
fn expr_contains_strict_recognizer(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Path(p) => path_ends_with_strict_recognizer(&p.path),
        syn::Expr::Call(c) => {
            if let syn::Expr::Path(p) = &*c.func
                && path_ends_with_strict_recognizer(&p.path)
            {
                return true;
            }
            // Also descend into the function expr (rare) and arguments.
            expr_contains_strict_recognizer(&c.func)
                || c.args.iter().any(expr_contains_strict_recognizer)
        }
        syn::Expr::MethodCall(m) => {
            expr_contains_strict_recognizer(&m.receiver)
                || m.args.iter().any(expr_contains_strict_recognizer)
        }
        syn::Expr::Reference(r) => expr_contains_strict_recognizer(&r.expr),
        syn::Expr::Group(g) => expr_contains_strict_recognizer(&g.expr),
        syn::Expr::Paren(p) => expr_contains_strict_recognizer(&p.expr),
        syn::Expr::Block(b) => b
            .block
            .stmts
            .iter()
            .any(|s| matches!(s, syn::Stmt::Expr(e, _) if expr_contains_strict_recognizer(e))),
        syn::Expr::Cast(c) => expr_contains_strict_recognizer(&c.expr),
        syn::Expr::Try(t) => expr_contains_strict_recognizer(&t.expr),
        syn::Expr::Await(a) => expr_contains_strict_recognizer(&a.base),
        syn::Expr::Field(f) => expr_contains_strict_recognizer(&f.base),
        _ => false,
    }
}

/// Returns true if any segment of `path` is `StrictRecognizer`.
///
/// Both the type-as-value form (`StrictRecognizer`) and the constructor-call
/// form (`StrictRecognizer::new`) match — and so do fully-qualified variants
/// (`marque_engine::StrictRecognizer::new`). Other type names ending in
/// the same suffix (`MyStrictRecognizer`) are not matched because the
/// segment ident equality is exact.
fn path_ends_with_strict_recognizer(path: &syn::Path) -> bool {
    path.segments
        .iter()
        .any(|seg| seg.ident == "StrictRecognizer")
}
