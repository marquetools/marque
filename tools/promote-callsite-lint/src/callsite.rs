// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pass A — call-site origin lint (FR-040 base).
//!
//! Walks every `*.rs` file under the workspace's covered trees —
//! `crates/*/src/**`, `crates/*/tests/**`, every top-level workspace
//! member's `src/**` and `tests/**` (including the `marque/` binary
//! crate and any future top-level member, discovered via each
//! directory's `Cargo.toml`), plus the workspace-root `tests/**` —
//! and for each call to `AppliedFix::__engine_promote(...)`,
//! `AppliedTextCorrection::__engine_promote_text_correction(...)`, or
//! `EnginePromotionToken::__engine_construct()`, classifies the call
//! site as one of:
//!
//! 1. **Production-allowed** — the call lives in
//!    `crates/engine/src/**` and the enclosing function is one of
//!    the engine's promotion-gate functions
//!    (`fix_inner`, `apply_text_corrections`, or
//!    `engine_promotion_token`). Allowed.
//! 2. **Test-fixture-allowed** — the call lives in a `tests/`
//!    integration file (`crates/*/tests/**` or workspace `tests/**`)
//!    *or* inside a `#[cfg(test)]` module elsewhere, AND the
//!    Constitution V Principle V comment marker
//!    (`Test-fixture carve-out per Constitution V`) appears within
//!    five source lines above the call. Allowed.
//! 3. **Test-fixture-unmarked** — same scope as (2) but the
//!    comment marker is absent within the lookback window. Fails
//!    with code `PRC001`.
//! 4. **Other** — anywhere else. Fails with code `PRC002`.
//!
//! The lookback window is consistent with the masking-pin lint
//! (FR-039): five lines above the call expression, inclusive.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use syn::{Expr, ExprCall, ExprMethodCall, ExprPath, File, Path as SynPath, spanned::Spanned};
use walkdir::WalkDir;

use crate::diagnostic::{Diagnostic, Severity};
use crate::enclosing::{collect_fn_records, enclosing_fn};

/// Comment marker required by Constitution V Principle V test-fixture carve-out.
pub const CARVE_OUT_MARKER: &str = "Test-fixture carve-out per Constitution V";

/// Maximum number of source lines above a call expression in which the
/// carve-out comment may appear. Matches the FR-039 masking-pin window.
pub const COMMENT_LOOKBACK_LINES: usize = 5;

/// Engine functions allowed to call `AppliedFix::__engine_promote` /
/// `AppliedTextCorrection::__engine_promote_text_correction` /
/// `EnginePromotionToken::__engine_construct` in production code.
///
/// Per Constitution V Principle V, only `Engine::fix_inner` may
/// promote marking `FixIntent` values to `AppliedFix` audit records,
/// and only `Engine::apply_text_corrections` may promote text-
/// correction matches to `AppliedTextCorrection` audit records (the
/// PR 3c.2.D split). The token-mint helper `engine_promotion_token`
/// (called only by the prior two) is part of the same gate and is
/// permitted here for the same reason: a future refactor that adds
/// a fourth production caller has to thread through this list,
/// making the expansion an explicit decision.
/// Engine methods on the `Engine` type permitted to call the three
/// reserved-name constructors in production code. These names match
/// ONLY when the enclosing `impl` block targets `Engine` — a free
/// function with one of these names elsewhere in
/// `crates/engine/src/**` is rejected.
const ENGINE_METHOD_ALLOW_LIST: &[&str] = &["fix_inner", "apply_text_corrections"];

/// Methods on `TwoPassFixer` permitted to call
/// `AppliedFix::__engine_promote` / `EnginePromotionToken::__engine_construct`
/// in production code (PR 7b). `TwoPassFixer` is the phase-split fix
/// orchestrator extracted from `Engine::fix_inner`; the two promotion
/// call sites that used to live directly inside `fix_inner` now live
/// inside `TwoPassFixer::apply_kept_fixes`. The Constitution V Principle V
/// engine-only contract still holds — `TwoPassFixer` is a private
/// struct in `crates/engine/src/engine.rs`, the promotion token is
/// minted by the same `engine_promotion_token()` free helper as
/// before, and the threshold gate / FR-016 sort / C-1 overlap guard
/// run in the same orchestrator.
///
/// This list is matched only when (a) the enclosing `impl` block
/// targets a type whose last-segment name is `TwoPassFixer` AND (b)
/// the call site lives in the canonical file
/// `crates/engine/src/engine.rs` (the [`is_engine_canonical_helper_file`]
/// check). The path guard closes the Copilot round-3 R3-1 finding:
/// type-name-only matching against `impl_self_type` would let any
/// `struct TwoPassFixer` defined elsewhere under `crates/engine/src/**`
/// inherit the allow-list — same-name shadowing is exactly the
/// engine-only-contract bypass the FR-040 lint must mechanically
/// prevent. The pattern mirrors [`ENGINE_FREE_FN_ALLOW_LIST`]'s
/// `is_engine_canonical_helper_file` companion-check: "one allow-list
/// entry, one canonical home."
const TWOPASSFIXER_METHOD_ALLOW_LIST: &[&str] = &["apply_kept_fixes"];

/// Free helper(s) in `crates/engine/src/engine.rs` (the exact file)
/// that are permitted to mint an `EnginePromotionToken`. Currently
/// exactly one — the `engine_promotion_token()` token-mint helper.
/// The list is matched in conjunction with
/// [`is_engine_canonical_helper_file`] so a free fn with the same
/// name in a different file under `crates/engine/src/**` is rejected:
/// the FR-040 contract centralizes token-mint privilege in ONE
/// helper, located at one specific path. A separate file-level
/// matcher (rather than just the ident name) closes the bypass that
/// any module under `crates/engine/src/**` could otherwise re-declare
/// the helper and pass PRC002.
const ENGINE_FREE_FN_ALLOW_LIST: &[&str] = &["engine_promotion_token"];

/// Scan `<workspace_dir>` and return any callsite-lint diagnostics.
///
/// Errors propagate from filesystem traversal or `syn` parse
/// failures; an unreadable file is itself a CI hazard the caller
/// should surface, so we do not silently skip them.
///
/// # Errors
///
/// Returns an error if a directory walk fails or a Rust source file
/// cannot be parsed by `syn`.
pub fn scan_workspace(workspace_dir: &Path) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for path in collect_rust_files(workspace_dir)? {
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file =
            syn::parse_file(&source).with_context(|| format!("parsing {}", path.display()))?;
        scan_file(&path, &source, &file, workspace_dir, &mut diagnostics);
    }
    Ok(diagnostics)
}

/// Scan a single in-memory file. Used by both [`scan_workspace`]
/// and the integration tests.
pub fn scan_file(
    file_path: &Path,
    source: &str,
    file: &File,
    workspace_dir: &Path,
    sink: &mut Vec<Diagnostic>,
) {
    let fn_records = collect_fn_records(file);
    let source_lines: Vec<&str> = source.lines().collect();
    let mut visitor = CallSiteVisitor {
        file_path,
        workspace_dir,
        source_lines: &source_lines,
        fn_records: &fn_records,
        sink,
    };
    syn::visit::Visit::visit_file(&mut visitor, file);
}

fn collect_rust_files(workspace_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    // crates/*/src/**, crates/*/tests/**
    let crates_dir = workspace_dir.join("crates");
    if crates_dir.is_dir() {
        for crate_entry in std::fs::read_dir(&crates_dir)
            .with_context(|| format!("reading {}", crates_dir.display()))?
        {
            let crate_entry = crate_entry?;
            let crate_path = crate_entry.path();
            if !crate_path.is_dir() {
                continue;
            }
            for sub in ["src", "tests"] {
                let sub_path = crate_path.join(sub);
                if sub_path.is_dir() {
                    push_rust_files(&sub_path, &mut paths)?;
                }
            }
        }
    }

    // Top-level workspace members (any directory at workspace root that
    // contains both `Cargo.toml` and a `src/` or `tests/` subdirectory).
    // This catches the workspace binary crate `marque/` (which lives
    // outside `crates/`) plus any future top-level workspace member —
    // necessary for FR-040 enforcement to be workspace-wide.
    for entry in std::fs::read_dir(workspace_dir)
        .with_context(|| format!("reading {}", workspace_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip directories already covered above or out of scope.
        if matches!(
            path.file_name().and_then(|s| s.to_str()),
            Some(
                "crates"
                    | "tests"
                    | "tools"
                    | "target"
                    | ".git"
                    | "site"
                    | "specs"
                    | "docs"
                    | "scripts"
                    | "benches"
            )
        ) {
            continue;
        }
        if !path.join("Cargo.toml").is_file() {
            continue;
        }
        for sub in ["src", "tests"] {
            let sub_path = path.join(sub);
            if sub_path.is_dir() {
                push_rust_files(&sub_path, &mut paths)?;
            }
        }
    }

    // workspace-root tests/**
    let tests_dir = workspace_dir.join("tests");
    if tests_dir.is_dir() {
        push_rust_files(&tests_dir, &mut paths)?;
    }

    Ok(paths)
}

/// Walk `dir` and append every `*.rs` file path to `paths`.
///
/// Walk errors are surfaced rather than silently swallowed: a
/// permission error or broken directory entry would otherwise hide
/// call sites from PRC001/PRC002 while the lint still exited
/// successfully — a false-green for a CI enforcement tool.
fn push_rust_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    for entry in WalkDir::new(dir) {
        let entry = entry.with_context(|| format!("walking {}", dir.display()))?;
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|ext| ext == "rs") {
            paths.push(p.to_path_buf());
        }
    }
    Ok(())
}

struct CallSiteVisitor<'a> {
    file_path: &'a Path,
    workspace_dir: &'a Path,
    source_lines: &'a [&'a str],
    fn_records: &'a [crate::enclosing::FnRecord],
    sink: &'a mut Vec<Diagnostic>,
}

impl<'a> syn::visit::Visit<'a> for CallSiteVisitor<'a> {
    fn visit_expr_call(&mut self, node: &'a ExprCall) {
        if let Expr::Path(ExprPath { path, .. }) = &*node.func {
            // Match on the call path's **last segment** (the function
            // ident) for the three specifically reserved names
            // `__engine_promote` / `__engine_promote_text_correction` /
            // `__engine_construct`. Any path that ends with one of
            // those names — qualified (`AppliedFix::__engine_promote`),
            // fully-qualified
            // (`marque_rules::AppliedFix::__engine_promote`),
            // `Self::__engine_promote` inside `impl AppliedFix`, or
            // an aliased form (`AF::__engine_promote` after
            // `use marque_rules::AppliedFix as AF`) — is a candidate.
            //
            // **Exact-equality, NOT prefix-match.** The list is closed
            // and curated by hand: a new reserved name (e.g., a future
            // `__engine_promote_v3`) requires an explicit edit here so
            // the choice is reviewed. Prefix-matching would
            // accidentally catch back-compat names like
            // `__engine_promote_legacy` — see
            // `engine_promote_legacy_is_not_caught_by_suffix_match` in
            // the test suite (PR 3c.B Commit 2 / Phase D regression
            // guard).
            //
            // This is a deliberate trade-off identified during the
            // round-8 independent audit: an earlier round required a
            // type qualifier (matching `["AppliedFix",
            // "__engine_promote"]` only) to avoid false-positive on
            // free functions with the same name. The audit
            // demonstrated that requirement made the lint trivially
            // bypassable via `use ... as ...` aliases — and these
            // reserved names are deliberately RESERVED by the project
            // (all three are `#[doc(hidden)]` engine-only seal
            // mechanisms; they bear `__` precisely to discourage any
            // re-use). So:
            //
            //   - Free function named `__engine_promote`: itself a
            //     Constitution V Principle V violation; flagging is
            //     correct. A contributor genuinely needing the name
            //     for unrelated purposes can renamed or carry an
            //     `#[allow(...)]` after explicit review.
            //   - Aliased call (`use AppliedFix as AF; AF::__engine_promote(...)`):
            //     correctly flagged because the last segment is
            //     `__engine_promote`, regardless of the alias.
            //   - Method-call form (`x.__engine_promote()`) is still NOT
            //     matched — see `visit_expr_method_call` below.
            //
            // Method-call form is excluded because all three reserved
            // names are *associated* functions on `AppliedFix` /
            // `AppliedTextCorrection` / `EnginePromotionToken`; a
            // method call with those names cannot reach the real APIs
            // at all (the receiver type is wrong) and would fail to
            // compile.
            if path_ends_with(path, &["__engine_promote"])
                || path_ends_with(path, &["__engine_promote_text_correction"])
                || path_ends_with(path, &["__engine_construct"])
            {
                let loc = node.span().start();
                self.classify_and_emit(loc.line, loc.column);
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'a ExprMethodCall) {
        // No-op classification: `AppliedFix::__engine_promote` and
        // `EnginePromotionToken::__engine_construct` are *associated*
        // functions, not methods. A method call like
        // `applied_fix.__engine_promote(...)` cannot reach either API
        // (the receiver would be of the wrong type), so flagging
        // method-call sites with one of those names would only ever
        // produce false positives on unrelated user code that happens
        // to use a colliding method name. Still recurse into the
        // expression so call-form sites *inside* the method-call
        // argument list are discovered.
        syn::visit::visit_expr_method_call(self, node);
    }
}

impl CallSiteVisitor<'_> {
    fn classify_and_emit(&mut self, line: usize, column: usize) {
        let enclosing = enclosing_fn(self.fn_records, line);
        let in_engine_src = self.is_engine_src();
        let in_test_path = self.is_test_path();
        let in_test_utils_src = self.is_test_utils_src();
        let in_cfg_test_module = enclosing.is_some_and(|r| r.in_cfg_test);

        // Classification 1: production-allowed (engine src + allow-listed
        // fn whose enclosing impl block targets the `Engine` type).
        // Both checks are required: a free function or a method on some
        // other type with one of the names below would otherwise pass
        // even though FR-040 only authorizes the genuine `Engine` gate.
        // `engine_promotion_token` is a free helper in
        // `crates/engine/src/engine.rs`, so its `impl_self_type` is
        // `None` and the assertion is "not on a different impl block."
        if in_engine_src {
            if let Some(fr) = enclosing {
                // The two allow-lists are kept disjoint and matched on
                // shape, not just name:
                //
                // - `ENGINE_METHOD_ALLOW_LIST` matches only when the
                //   enclosing impl targets `Engine`. A method on
                //   another type or a free function with one of these
                //   names is rejected.
                // - `ENGINE_FREE_FN_ALLOW_LIST` matches only when there
                //   is no enclosing impl (free function). A method on
                //   any type with the same name is rejected.
                //
                // Splitting the lists this way closes the bypass that
                // a single shared list with a permissive None-allowed
                // self-type check would have left open: a new free
                // function in `crates/engine/src/**` named `fix_inner`
                // or `apply_text_corrections` calling `__engine_promote`
                // is now correctly rejected.
                let allowed = match fr.impl_self_type.as_deref() {
                    None => {
                        ENGINE_FREE_FN_ALLOW_LIST.contains(&fr.name.as_str())
                            && self.is_engine_canonical_helper_file()
                    }
                    Some("Engine") => ENGINE_METHOD_ALLOW_LIST.contains(&fr.name.as_str()),
                    // Canonical-path guard (Copilot round-3 R3-1): same-
                    // name shadow types defined elsewhere under
                    // `crates/engine/src/**` MUST NOT inherit the
                    // `TwoPassFixer` allow-list. The path check pins
                    // the allow-list to one home — the canonical file
                    // `crates/engine/src/engine.rs` — mirroring the
                    // free-fn `ENGINE_FREE_FN_ALLOW_LIST` companion
                    // check above. Without this guard, a contributor
                    // who writes `struct TwoPassFixer { ... } impl
                    // TwoPassFixer { fn apply_kept_fixes(...) }` in
                    // `crates/engine/src/wherever.rs` gets a free pass
                    // through the FR-040 engine-only contract.
                    Some("TwoPassFixer") => {
                        TWOPASSFIXER_METHOD_ALLOW_LIST.contains(&fr.name.as_str())
                            && self.is_engine_canonical_helper_file()
                    }
                    Some(_) => false,
                };
                if allowed {
                    return;
                }
            }
        }

        // Classification 2/3: test-fixture scope. Recognized scopes:
        //
        // - `tests/**` integration files (workspace root or any member's
        //   tests/) — discovered via `is_test_path`.
        // - `#[cfg(test)]` modules anywhere in the workspace.
        // - `crates/test-utils/src/**` — a workspace-internal test
        //   utility crate used only as a `dev-dependency` (verified by
        //   `crates/rules/src/lib.rs` doc comment on
        //   `AppliedFix::__engine_promote`, which explicitly enumerates
        //   "test-utility crates gated as dev-dependencies" as part of
        //   the Constitution V Principle V carve-out's first
        //   constraint). Scoped to `test-utils` specifically rather than
        //   any `*-utils` crate so a future production-utility crate
        //   can't accidentally inherit the carve-out.
        if in_test_path || in_cfg_test_module || in_test_utils_src {
            if self.has_carve_out_marker(line) {
                return;
            }
            self.sink.push(Diagnostic {
                file: self.file_path.to_path_buf(),
                line,
                column: column + 1, // proc-macro2 columns are 0-indexed; rustc-style is 1-indexed
                severity: Severity::Error,
                code: "PRC001",
                message: format!(
                    "__engine_promote/__engine_promote_text_correction/__engine_construct \
                     test-fixture call lacks '{CARVE_OUT_MARKER}' comment within \
                     {COMMENT_LOOKBACK_LINES} lines"
                ),
            });
            return;
        }

        // Classification 4: other production code.
        self.sink.push(Diagnostic {
            file: self.file_path.to_path_buf(),
            line,
            column: column + 1,
            severity: Severity::Error,
            code: "PRC002",
            message: "__engine_promote/__engine_promote_text_correction/\
                      __engine_construct called from non-engine, non-test code \
                      (FR-040; Constitution V Principle V)"
                .to_string(),
        });
    }

    fn is_engine_src(&self) -> bool {
        // Match `<workspace>/crates/engine/src/**`. We compare via
        // path-component segments to stay portable across separators.
        let rel = self
            .file_path
            .strip_prefix(self.workspace_dir)
            .unwrap_or(self.file_path);
        let comps: Vec<_> = rel.components().collect();
        comps.len() >= 3
            && comps[0].as_os_str() == "crates"
            && comps[1].as_os_str() == "engine"
            && comps[2].as_os_str() == "src"
    }

    fn is_test_path(&self) -> bool {
        let rel = self
            .file_path
            .strip_prefix(self.workspace_dir)
            .unwrap_or(self.file_path);
        let comps: Vec<_> = rel.components().collect();
        // A path is a test path iff `tests` appears as a top-level
        // workspace-member subdir, NOT nested inside `src/`. Three
        // accepted shapes:
        //
        //   1. Workspace-root tests:           `tests/<...>`
        //   2. Crates member tests:            `crates/<name>/tests/<...>`
        //   3. Top-level member tests:         `<member>/tests/<...>`
        //
        // A production module placed under `src/tests/...` (a
        // perfectly valid Rust file layout — `tests` is just an
        // ordinary module name there) MUST NOT be classified as a
        // test path; otherwise that module could call
        // `__engine_promote` with just the carve-out comment and
        // bypass PRC002. Honoring the path-component policy strictly
        // closes that gap.
        let positions: Vec<usize> = comps
            .iter()
            .enumerate()
            .filter_map(|(i, c)| (c.as_os_str() == "tests").then_some(i))
            .collect();
        for &pos in &positions {
            // `tests` at position 0 = workspace-root `tests/<...>`.
            if pos == 0 {
                return true;
            }
            // `tests` at position 1 = `<top-level-member>/tests/<...>`,
            // accepted iff the parent component is a workspace member
            // (not `src` / `target` / etc.). Path component sanity
            // assertion: `comps[0]` is the workspace-member dir name.
            if pos == 1 {
                return true;
            }
            // `tests` at position 2 = `crates/<name>/tests/<...>`.
            if pos == 2 && comps[0].as_os_str() == "crates" {
                return true;
            }
            // Any deeper position implies `tests` is nested inside
            // `src/` (or some other intermediate dir) and is a
            // production module, NOT a test path.
        }
        false
    }

    /// Match the exact canonical file `crates/engine/src/engine.rs`,
    /// where the production token-mint helper `engine_promotion_token`
    /// lives. Used in conjunction with [`ENGINE_FREE_FN_ALLOW_LIST`]
    /// so a free fn with the helper's name in any other file under
    /// `crates/engine/src/**` is rejected by PRC002 — the FR-040
    /// contract is "one helper, one place," and the lint enforces the
    /// "one place" half here.
    fn is_engine_canonical_helper_file(&self) -> bool {
        let rel = self
            .file_path
            .strip_prefix(self.workspace_dir)
            .unwrap_or(self.file_path);
        let comps: Vec<_> = rel.components().collect();
        comps.len() == 4
            && comps[0].as_os_str() == "crates"
            && comps[1].as_os_str() == "engine"
            && comps[2].as_os_str() == "src"
            && comps[3].as_os_str() == "engine.rs"
    }

    /// Match `crates/test-utils/src/**`. The carve-out's first
    /// constraint (Constitution V Principle V, restated by
    /// `crates/rules/src/lib.rs::AppliedFix::__engine_promote`'s doc
    /// comment) lists "test-utility crates gated as `dev-dependencies`"
    /// as a permitted call-site scope alongside `#[cfg(test)]` modules
    /// and `tests/` integration files. Honor that here so the lint
    /// doesn't reject a legitimate helper added under
    /// `crates/test-utils/src/` for `crates/*/tests/` to consume.
    /// Scoped to `test-utils` specifically (not `*-utils`) so a
    /// production utility crate can't accidentally inherit the
    /// carve-out.
    fn is_test_utils_src(&self) -> bool {
        let rel = self
            .file_path
            .strip_prefix(self.workspace_dir)
            .unwrap_or(self.file_path);
        let comps: Vec<_> = rel.components().collect();
        comps.len() >= 3
            && comps[0].as_os_str() == "crates"
            && comps[1].as_os_str() == "test-utils"
            && comps[2].as_os_str() == "src"
    }

    fn has_carve_out_marker(&self, line: usize) -> bool {
        // Look at the `COMMENT_LOOKBACK_LINES` lines immediately
        // preceding `line`, plus `line` itself (carve-out comment on
        // the same source line as the call also satisfies).
        //
        // The marker MUST appear inside a `//` line comment — a raw
        // string literal, ident, or other code containing the marker
        // phrase does NOT satisfy the carve-out. Otherwise a hostile
        // or accidental inclusion of the phrase elsewhere in the
        // source would whitelist a real bypass call site. We accept
        // either a leading-trim line that starts with `//` (line
        // comment), or the comment's appearance after some code
        // followed by `//` on the same line.
        let end = line.min(self.source_lines.len());
        let start = end.saturating_sub(COMMENT_LOOKBACK_LINES + 1);
        for &src_line in &self.source_lines[start..end] {
            if line_comment_contains_marker(src_line, CARVE_OUT_MARKER) {
                return true;
            }
        }
        false
    }
}

/// Returns `true` if the trailing segments of `path` match `expected_suffix`
/// in order. Handles the bare, 2-segment, and fully-qualified forms uniformly:
///
/// - `path = ["__engine_promote"]`, suffix = `["__engine_promote"]` → true
/// - `path = ["AppliedFix", "__engine_promote"]`, suffix = `["AppliedFix", "__engine_promote"]` → true
/// - `path = ["marque_rules", "AppliedFix", "__engine_promote"]`,
///   suffix = `["AppliedFix", "__engine_promote"]` → true
///
/// The function intentionally does NOT require an exact-length match —
/// a length match would let a contributor bypass the lint just by
/// fully-qualifying the type at the call site. The lint cares about
/// "what's being called," not "from where."
fn path_ends_with(path: &SynPath, expected_suffix: &[&str]) -> bool {
    if path.segments.len() < expected_suffix.len() || expected_suffix.is_empty() {
        return false;
    }
    let offset = path.segments.len() - expected_suffix.len();
    path.segments
        .iter()
        .skip(offset)
        .zip(expected_suffix.iter())
        .all(|(seg, &want)| seg.ident == want)
}

/// True when `line` contains a `//` line comment whose body contains
/// the literal `marker` text. A naive `line.contains(marker)` would
/// false-positive on a string literal, raw identifier, or any other
/// non-comment occurrence of the phrase — that's the bypass closed
/// here.
///
/// The implementation handles `//`-comment detection without a full
/// Rust tokenizer by scanning for the first `//` that is NOT inside a
/// string literal. Strings are tracked with a small state machine
/// covering plain `"..."`, `r"..."` / `r#"..."#` raw strings, and
/// chars `'...'`. Block comments `/* ... */` are out of scope: line-
/// at-a-time analysis can't track multi-line block-comment state, and
/// the marker is documented as a `//` line comment — never a block
/// comment — by Constitution V Principle V.
fn line_comment_contains_marker(line: &str, marker: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_str: Option<char> = None; // `"` or `'`
    let mut raw_hashes: usize = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match in_str {
            None => {
                // Detect raw-string start: `r#*"`.
                if b == b'r' && i + 1 < bytes.len() {
                    let mut j = i + 1;
                    let mut hashes = 0;
                    while j < bytes.len() && bytes[j] == b'#' {
                        hashes += 1;
                        j += 1;
                    }
                    if j < bytes.len() && bytes[j] == b'"' {
                        in_str = Some('"');
                        raw_hashes = hashes;
                        i = j + 1;
                        continue;
                    }
                }
                if b == b'"' {
                    in_str = Some('"');
                    raw_hashes = 0;
                    i += 1;
                    continue;
                }
                if b == b'\'' {
                    // char or lifetime; lifetimes don't terminate. To
                    // avoid eating into a `'a` lifetime, peek for a
                    // closing `'` in the next ~3 bytes.
                    let limit = (i + 5).min(bytes.len());
                    let found_close = bytes[(i + 1)..limit].contains(&b'\'');
                    if found_close {
                        in_str = Some('\'');
                    }
                    i += 1;
                    continue;
                }
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    // Found a line comment. Search the comment body
                    // for the marker.
                    let comment_body = &line[i + 2..];
                    return comment_body.contains(marker);
                }
                i += 1;
            }
            Some('"') => {
                if raw_hashes == 0 {
                    if b == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if b == b'"' {
                        in_str = None;
                        i += 1;
                        continue;
                    }
                    i += 1;
                } else {
                    // Raw string: closes on `"` followed by exactly `raw_hashes` `#`s.
                    if b == b'"' {
                        let close = i + 1 + raw_hashes;
                        if close <= bytes.len() && bytes[i + 1..close].iter().all(|&h| h == b'#') {
                            in_str = None;
                            i = close;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
            Some('\'') => {
                if b == b'\'' {
                    in_str = None;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    false
}
