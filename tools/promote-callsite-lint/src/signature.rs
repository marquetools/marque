// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pass B — D12 / R-11 signature-shape lint.
//!
//! Walk every `*.rs` file under `<workspace_dir>/crates/**` and
//! flag every function whose signature shape matches "accepts
//! `ParsedAttrs` and returns `CanonicalAttrs` (or
//! `Result<CanonicalAttrs, _>`)" — outside three explicit
//! whitelisted call sites.
//!
//! Whitelists:
//!
//! 1. **`unsafe fn`** — any function carrying the `unsafe` keyword.
//!    Rust stdlib uses `_unchecked` for `unsafe` APIs (e.g.
//!    `get_unchecked`, `from_utf8_unchecked`); the keyword acts as
//!    a documented audit barrier.
//! 2. **`MarkingScheme::canonicalize`** — the trait method that
//!    *defines* the legitimate `ParsedAttrs → CanonicalAttrs`
//!    transition. Detected by enclosing `impl <...> MarkingScheme<...> for T`
//!    plus method ident `canonicalize`.
//! 3. **Transitional `from_parsed_unchecked`** in
//!    `crates/ism/src/attrs.rs` — path-based carve-out scoped to
//!    the PR 3a → PR 3c keystone window. **Auto-expires** when
//!    PR 3c lands and tasks.md T054 deletes the function: the
//!    whitelist match becomes a no-op (nothing to whitelist) but
//!    is harmless dead code that can be removed on the next pass.
//!
//! Targeting **shape, not name** is the D12 rationale: a future
//! contributor renaming `from_parsed_raw` evades a name-suffix
//! lint without altering the failure pattern.
//!
//! At PR 0 land, no functions in the workspace match this shape
//! (the types `ParsedAttrs` / `CanonicalAttrs` arrive at PR 3a).
//! The lint is forward-looking; the whitelist is scaffolding.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use syn::{
    File, FnArg, GenericArgument, ImplItem, Item, ItemImpl, ItemMod, PathArguments, ReturnType,
    Signature, Type,
};
use walkdir::WalkDir;

use crate::diagnostic::{Diagnostic, Severity};

/// Marker type names recognized as the source of the prohibited shape.
const PARSED_TYPE_NAME: &str = "ParsedAttrs";

/// Marker type names recognized as the target of the prohibited shape.
const CANONICAL_TYPE_NAME: &str = "CanonicalAttrs";

/// File-relative path of the transitional whitelist site (whitelist 3).
/// Components are joined at runtime to stay portable.
const TRANSITIONAL_WHITELIST_PATH: &[&str] = &["crates", "ism", "src", "attrs.rs"];

/// Function ident of the transitional whitelist site (whitelist 3).
const TRANSITIONAL_WHITELIST_FN: &str = "from_parsed_unchecked";

/// Trait name and method that define the legitimate canonical
/// transition (whitelist 2).
const MARKING_SCHEME_TRAIT: &str = "MarkingScheme";
const CANONICALIZE_METHOD: &str = "canonicalize";

/// Scan `<workspace_dir>/crates/**` and return any signature-shape
/// diagnostics.
///
/// # Errors
///
/// Returns an error if a directory walk fails or a Rust source file
/// cannot be parsed by `syn`.
pub fn scan_workspace(workspace_dir: &Path) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for path in collect_rust_files(workspace_dir) {
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file = syn::parse_file(&source)
            .with_context(|| format!("parsing {}", path.display()))?;
        scan_file(&path, &file, workspace_dir, &mut diagnostics);
    }
    Ok(diagnostics)
}

/// Scan a single in-memory file. Used by both [`scan_workspace`]
/// and the integration tests.
pub fn scan_file(
    file_path: &Path,
    file: &File,
    workspace_dir: &Path,
    sink: &mut Vec<Diagnostic>,
) {
    let rel = file_path
        .strip_prefix(workspace_dir)
        .unwrap_or(file_path)
        .to_path_buf();
    let mut walker = SignatureWalker {
        file_path,
        rel_path: rel,
        sink,
    };
    walker.visit_items(&file.items, /* impl_trait_last = */ None);
}

fn collect_rust_files(workspace_dir: &Path) -> Vec<PathBuf> {
    let crates_dir = workspace_dir.join("crates");
    if !crates_dir.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(&crates_dir).into_iter().filter_map(Result::ok) {
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|ext| ext == "rs") {
            out.push(p.to_path_buf());
        }
    }
    out
}

struct SignatureWalker<'a> {
    file_path: &'a Path,
    rel_path: PathBuf,
    sink: &'a mut Vec<Diagnostic>,
}

impl SignatureWalker<'_> {
    fn visit_items(&mut self, items: &[Item], impl_trait_last: Option<&str>) {
        for item in items {
            self.visit_item(item, impl_trait_last);
        }
    }

    fn visit_item(&mut self, item: &Item, impl_trait_last: Option<&str>) {
        match item {
            Item::Fn(item_fn) => {
                self.maybe_emit_for_signature(
                    &item_fn.sig,
                    None, // not in an `impl Trait for T`
                );
            }
            Item::Mod(item_mod) => self.visit_mod(item_mod, impl_trait_last),
            Item::Impl(item_impl) => self.visit_impl(item_impl),
            _ => {}
        }
    }

    fn visit_mod(&mut self, item_mod: &ItemMod, impl_trait_last: Option<&str>) {
        if let Some((_, items)) = &item_mod.content {
            self.visit_items(items, impl_trait_last);
        }
    }

    fn visit_impl(&mut self, item_impl: &ItemImpl) {
        let trait_last = item_impl
            .trait_
            .as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .map(|seg| seg.ident.to_string());

        for impl_item in &item_impl.items {
            if let ImplItem::Fn(method) = impl_item {
                self.maybe_emit_for_signature(&method.sig, trait_last.as_deref());
            }
        }
    }

    fn maybe_emit_for_signature(
        &mut self,
        sig: &Signature,
        impl_trait_last: Option<&str>,
    ) {
        // Whitelist 1: any `unsafe fn`.
        if sig.unsafety.is_some() {
            return;
        }

        let arg_types = collect_arg_type_idents(sig);
        let ret_types = collect_return_type_idents(sig);

        let arg_has_parsed = arg_types.contains(PARSED_TYPE_NAME);
        let ret_has_canonical = ret_types.contains(CANONICAL_TYPE_NAME);
        if !(arg_has_parsed && ret_has_canonical) {
            return;
        }

        // Whitelist 2: `MarkingScheme::canonicalize`.
        if impl_trait_last == Some(MARKING_SCHEME_TRAIT)
            && sig.ident == CANONICALIZE_METHOD
        {
            return;
        }

        // Whitelist 3: transitional `crates/ism/src/attrs.rs::from_parsed_unchecked`.
        // Auto-expires when PR 3c lands and tasks.md T054 deletes the
        // function — the whitelist becomes a no-op at that point.
        if self.rel_path_matches_transitional_site() && sig.ident == TRANSITIONAL_WHITELIST_FN
        {
            return;
        }

        let loc = sig.ident.span().start();
        self.sink.push(Diagnostic {
            file: self.file_path.to_path_buf(),
            line: loc.line,
            column: loc.column + 1,
            severity: Severity::Error,
            code: "PRC100",
            message: format!(
                "function {fn_name} has signature shape (ParsedAttrs -> CanonicalAttrs) \
                 outside MarkingScheme::canonicalize \
                 (FR-040 amendment per D12 / R-11)",
                fn_name = sig.ident,
            ),
        });
    }

    fn rel_path_matches_transitional_site(&self) -> bool {
        let comps: Vec<_> = self.rel_path.components().collect();
        if comps.len() != TRANSITIONAL_WHITELIST_PATH.len() {
            return false;
        }
        comps
            .iter()
            .zip(TRANSITIONAL_WHITELIST_PATH.iter())
            .all(|(c, &want)| c.as_os_str() == want)
    }
}

/// Collect the set of last-segment idents appearing in any of the
/// signature's input argument types.
///
/// References (`&T`, `&mut T`), tuples, slices, arrays, and generic
/// arguments are unwrapped recursively so that
/// `&ParsedAttrs<'_>` contributes `ParsedAttrs` and
/// `Vec<ParsedAttrs<'a>>` contributes both `Vec` and `ParsedAttrs`.
fn collect_arg_type_idents(sig: &Signature) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for arg in &sig.inputs {
        // The receiver-pattern `self` lives in `FnArg::Receiver`; only
        // typed arguments contribute idents to the lint match.
        if let FnArg::Typed(pat_type) = arg {
            collect_type_idents(&pat_type.ty, &mut out);
        }
    }
    out
}

/// Collect the set of last-segment idents appearing in the
/// signature's return type. Empty for `ReturnType::Default`.
///
/// Crucially: a return type of `Result<CanonicalAttrs, E>` will
/// contribute `Result`, `CanonicalAttrs`, and `E`'s last segment —
/// the lint match condition (`CANONICAL_TYPE_NAME` present anywhere
/// in the set) catches both bare and `Result`-wrapped shapes.
fn collect_return_type_idents(sig: &Signature) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let ReturnType::Type(_, ty) = &sig.output {
        collect_type_idents(ty, &mut out);
    }
    out
}

fn collect_type_idents(ty: &Type, out: &mut BTreeSet<String>) {
    match ty {
        Type::Path(type_path) => {
            for seg in &type_path.path.segments {
                out.insert(seg.ident.to_string());
                if let PathArguments::AngleBracketed(angle) = &seg.arguments {
                    for ga in &angle.args {
                        if let GenericArgument::Type(inner) = ga {
                            collect_type_idents(inner, out);
                        }
                    }
                }
            }
        }
        Type::Reference(r) => collect_type_idents(&r.elem, out),
        Type::Slice(s) => collect_type_idents(&s.elem, out),
        Type::Array(a) => collect_type_idents(&a.elem, out),
        Type::Tuple(t) => {
            for elem in &t.elems {
                collect_type_idents(elem, out);
            }
        }
        Type::Paren(p) => collect_type_idents(&p.elem, out),
        Type::Group(g) => collect_type_idents(&g.elem, out),
        Type::Ptr(p) => collect_type_idents(&p.elem, out),
        Type::BareFn(bf) => {
            for input in &bf.inputs {
                collect_type_idents(&input.ty, out);
            }
            if let ReturnType::Type(_, ty) = &bf.output {
                collect_type_idents(ty, out);
            }
        }
        // `Type::ImplTrait` and `Type::TraitObject` are deliberately
        // not unwrapped — the lint targets concrete `ParsedAttrs` /
        // `CanonicalAttrs` type references. A function returning
        // `impl Into<CanonicalAttrs>` is a different shape and a
        // future lint extension if needed.
        _ => {}
    }
}
