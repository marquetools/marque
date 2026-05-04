// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Resolve the enclosing function of an arbitrary [`proc_macro2::Span`].
//!
//! The lint passes need to ask, "for this call expression, what is
//! the surrounding `fn` (free, method, or trait-impl)?" — the
//! answer drives the production / test-fixture / unrelated
//! classification.
//!
//! Approach: walk the parsed [`syn::File`] collecting every
//! `(name, line-range, in_test_module)` tuple, then return the
//! innermost record whose line range contains the target line.
//! Line ranges are cheap to compute from `syn`'s `proc-macro2`
//! `span_locations` feature and avoid the byte-offset hazards that
//! arise when crates use raw strings or include macros.

use proc_macro2::LineColumn;
use syn::{Block, File, ImplItem, Item, ItemImpl, ItemMod, Stmt, spanned::Spanned};

/// Information about a single function discovered while walking a file.
#[derive(Debug, Clone)]
pub struct FnRecord {
    /// Function name (last path segment for trait-impl methods, the
    /// free-fn ident for top-level `fn`).
    pub name: String,
    /// 1-indexed start line of the function body brace.
    pub start_line: usize,
    /// 1-indexed end line (inclusive) of the function body brace.
    pub end_line: usize,
    /// Whether the enclosing module path includes a `#[cfg(test)]`
    /// gate. Needed for the test-fixture carve-out classification
    /// when the call site lives outside `tests/`.
    pub in_cfg_test: bool,
    /// For methods defined in an `impl <Self>` or `impl Trait for <Self>`
    /// block, the last path segment of the self-type (e.g. `Engine`,
    /// `CapcoScheme`). `None` for free functions. Required by the
    /// callsite lint's production allow-list: `fix_inner` is only an
    /// authorized promotion site when it's a method on `Engine`, not
    /// on some unrelated type that happens to share the name.
    pub impl_self_type: Option<String>,
}

/// Walk the AST of `file` and return every function record.
///
/// The set is deliberately ordered by appearance, but resolution
/// uses [`enclosing_fn`] which selects the innermost record by
/// line range — order in this `Vec` is irrelevant to correctness.
#[must_use]
pub fn collect_fn_records(file: &File) -> Vec<FnRecord> {
    let mut sink = Vec::new();
    let ctx = Context::default();
    for item in &file.items {
        visit_item(item, &ctx, &mut sink);
    }
    sink
}

/// Find the innermost function (by source-line range) whose body
/// contains `line`. Returns `None` if `line` is outside any function.
#[must_use]
pub fn enclosing_fn(records: &[FnRecord], line: usize) -> Option<&FnRecord> {
    let mut best: Option<&FnRecord> = None;
    for r in records {
        if r.start_line <= line && line <= r.end_line {
            match best {
                None => best = Some(r),
                Some(prev) => {
                    // Innermost = smallest range containing the line.
                    let prev_span = prev.end_line - prev.start_line;
                    let cur_span = r.end_line - r.start_line;
                    if cur_span < prev_span {
                        best = Some(r);
                    }
                }
            }
        }
    }
    best
}

/// Convert a [`LineColumn`] to a 1-indexed line. `proc-macro2`
/// lines are already 1-indexed when `span-locations` is enabled.
#[must_use]
pub fn span_line(loc: LineColumn) -> usize {
    loc.line
}

#[derive(Debug, Default, Clone)]
struct Context {
    /// Accumulating `#[cfg(test)]` gating from outer modules.
    in_cfg_test: bool,
}

fn visit_item(item: &Item, ctx: &Context, sink: &mut Vec<FnRecord>) {
    match item {
        Item::Fn(item_fn) => {
            let span = item_fn.span();
            let start = span.start();
            let end = span.end();
            sink.push(FnRecord {
                name: item_fn.sig.ident.to_string(),
                start_line: start.line,
                end_line: end.line,
                in_cfg_test: ctx.in_cfg_test,
                impl_self_type: None,
            });
            // Recurse into the function body so block-scoped local
            // function items are recorded too. A `fn helper(...) { }`
            // declared inside another function would otherwise be
            // invisible — the `enclosing_fn` resolver would attribute
            // a call inside `helper` to the outer fn (e.g.
            // `Engine::fix_inner`) and silently grant it the outer
            // fn's allow-list status, even though `helper` itself is
            // not on any allow-list. The walker descends recursively
            // through nested blocks for the same reason.
            visit_block(&item_fn.block, ctx, sink);
        }
        Item::Mod(item_mod) => visit_mod(item_mod, ctx, sink),
        Item::Impl(item_impl) => visit_impl(item_impl, ctx, sink),
        _ => {}
    }
}

fn visit_block(block: &Block, ctx: &Context, sink: &mut Vec<FnRecord>) {
    for stmt in &block.stmts {
        if let Stmt::Item(inner_item) = stmt {
            visit_item(inner_item, ctx, sink);
        }
        // Statement expressions (`Stmt::Expr`, `Stmt::Local`, etc.)
        // can also contain nested closures or block expressions, but
        // closures don't produce `Item::Fn` nodes — they're
        // `ExprClosure` and the lint targets named functions, not
        // closures. So no recursion into expression statements is
        // needed here.
    }
}

fn visit_mod(item_mod: &ItemMod, ctx: &Context, sink: &mut Vec<FnRecord>) {
    let inner_in_cfg_test = ctx.in_cfg_test || has_cfg_test_attr(&item_mod.attrs);
    let inner_ctx = Context {
        in_cfg_test: inner_in_cfg_test,
    };
    if let Some((_, items)) = &item_mod.content {
        for item in items {
            visit_item(item, &inner_ctx, sink);
        }
    }
}

fn visit_impl(item_impl: &ItemImpl, ctx: &Context, sink: &mut Vec<FnRecord>) {
    // Trait-name resolution for the D12 signature-shape lint runs
    // directly off `ItemImpl` in `signature.rs`; we DO thread the
    // self-type's last path segment so the callsite lint can verify
    // a method named e.g. `fix_inner` is actually `Engine::fix_inner`
    // and not some unrelated type's method that happens to share the
    // name (FR-040 production allow-list integrity).
    let self_ty_last = self_type_last_segment(&item_impl.self_ty);
    for impl_item in &item_impl.items {
        if let ImplItem::Fn(method) = impl_item {
            let span = method.span();
            let start = span.start();
            let end = span.end();
            sink.push(FnRecord {
                name: method.sig.ident.to_string(),
                start_line: start.line,
                end_line: end.line,
                in_cfg_test: ctx.in_cfg_test,
                impl_self_type: self_ty_last.clone(),
            });
            // Recurse into the method body for block-scoped local
            // function items. Same rationale as `visit_item`: a
            // `fn helper(...) { }` declared inside `Engine::fix_inner`
            // would otherwise inherit `fix_inner`'s allow-list status
            // for any `__engine_promote` call inside it. Local-fn
            // items in a method body do NOT get the enclosing
            // method's `impl_self_type`; they're free functions in
            // their own right (Rust resolution treats them that way).
            let local_ctx = Context {
                in_cfg_test: ctx.in_cfg_test,
            };
            visit_block(&method.block, &local_ctx, sink);
        }
    }
}

/// Last path segment of an `impl <Self>` self-type. Returns `None`
/// for non-path types (impls of e.g. tuples, references, etc.) — the
/// callsite production allow-list relies on a path-typed self,
/// matching `Engine` / `CapcoScheme` / etc.
fn self_type_last_segment(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string()),
        syn::Type::Reference(r) => self_type_last_segment(&r.elem),
        syn::Type::Paren(p) => self_type_last_segment(&p.elem),
        syn::Type::Group(g) => self_type_last_segment(&g.elem),
        _ => None,
    }
}

fn has_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("test") {
                found = true;
            }
            Ok(())
        });
        found
    })
}
