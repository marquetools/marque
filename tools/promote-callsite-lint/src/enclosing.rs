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
use syn::{File, ImplItem, Item, ItemImpl, ItemMod, spanned::Spanned};

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
    /// If the function is inside an `impl Trait for T` block, the
    /// final segment of the trait path (e.g. `MarkingScheme`).
    /// `None` for free functions or inherent impls.
    pub impl_trait_last_segment: Option<String>,
    /// Whether the function carries the `unsafe` keyword.
    pub is_unsafe: bool,
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
    /// If we are currently inside an `impl Trait for T` block, the
    /// trait's final path segment.
    impl_trait_last_segment: Option<String>,
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
                impl_trait_last_segment: ctx.impl_trait_last_segment.clone(),
                is_unsafe: item_fn.sig.unsafety.is_some(),
            });
        }
        Item::Mod(item_mod) => visit_mod(item_mod, ctx, sink),
        Item::Impl(item_impl) => visit_impl(item_impl, ctx, sink),
        _ => {}
    }
}

fn visit_mod(item_mod: &ItemMod, ctx: &Context, sink: &mut Vec<FnRecord>) {
    let inner_in_cfg_test = ctx.in_cfg_test || has_cfg_test_attr(&item_mod.attrs);
    let inner_ctx = Context {
        in_cfg_test: inner_in_cfg_test,
        impl_trait_last_segment: ctx.impl_trait_last_segment.clone(),
    };
    if let Some((_, items)) = &item_mod.content {
        for item in items {
            visit_item(item, &inner_ctx, sink);
        }
    }
}

fn visit_impl(item_impl: &ItemImpl, ctx: &Context, sink: &mut Vec<FnRecord>) {
    let trait_last_segment = item_impl
        .trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .map(|seg| seg.ident.to_string());

    let inner_ctx = Context {
        in_cfg_test: ctx.in_cfg_test,
        impl_trait_last_segment: trait_last_segment.clone(),
    };

    for impl_item in &item_impl.items {
        if let ImplItem::Fn(method) = impl_item {
            let span = method.span();
            let start = span.start();
            let end = span.end();
            sink.push(FnRecord {
                name: method.sig.ident.to_string(),
                start_line: start.line,
                end_line: end.line,
                in_cfg_test: inner_ctx.in_cfg_test,
                impl_trait_last_segment: inner_ctx.impl_trait_last_segment.clone(),
                is_unsafe: method.sig.unsafety.is_some(),
            });
        }
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
