// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pass B — signature-shape lint.
//!
//! Walk every `*.rs` file under
//! `<workspace_dir>/{crates/*/{src,tests},tests,<top-level-members>/{src,tests}}`
//! (see [`collect_rust_files`] for the exact coverage policy) and
//! flag every function whose signature shape matches "accepts
//! `ParsedAttrs` and returns `CanonicalAttrs` (or
//! `Result<CanonicalAttrs, _>`)" — outside two explicit whitelisted
//! call sites.
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
//!
//! Targeting **shape, not name** is deliberate: a future
//! contributor renaming `from_parsed_raw` evades a name-suffix
//! lint without altering the failure pattern.
//!
//! An earlier transitional adapter
//! (`crates/ism/src/canonical.rs::from_parsed_unchecked`) once carried a
//! third path-based whitelist. That carve-out has since been retired
//! along with the adapter itself; the inlined structural rename now
//! lives in `CapcoScheme::canonicalize` (whitelist 2).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use syn::{
    Block, File, FnArg, GenericArgument, ImplItem, Item, ItemImpl, ItemMod, ItemTrait,
    Path as SynPath, PathArguments, ReturnType, Signature, Stmt, TraitItem, Type,
};
use walkdir::WalkDir;

use crate::diagnostic::{Diagnostic, Severity};

/// Marker type name recognized as the source of the prohibited shape.
const PARSED_TYPE_NAME: &str = "ParsedAttrs";

/// Marker type name recognized as the target of the prohibited shape.
const CANONICAL_TYPE_NAME: &str = "CanonicalAttrs";

/// Type name accepted as a direct return-type wrapper around
/// `CanonicalAttrs` — flagging `Result<CanonicalAttrs, _>` is part of
/// the D12 contract because the wrapping fallible form is just as
/// suspicious as the bare one. Adding more wrappers (e.g. `Option`)
/// requires a deliberate amendment.
const RESULT_TYPE_NAME: &str = "Result";

/// Trait method that defines the legitimate canonical transition
/// (whitelist 2). The trait path is matched separately — see
/// [`is_marking_scheme_trait_path`].
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
    for path in collect_rust_files(workspace_dir)? {
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file =
            syn::parse_file(&source).with_context(|| format!("parsing {}", path.display()))?;
        scan_file(&path, &file, workspace_dir, &mut diagnostics);
    }
    Ok(diagnostics)
}

/// Scan a single in-memory file. Used by both [`scan_workspace`]
/// and the integration tests.
pub fn scan_file(file_path: &Path, file: &File, workspace_dir: &Path, sink: &mut Vec<Diagnostic>) {
    let rel = file_path
        .strip_prefix(workspace_dir)
        .unwrap_or(file_path)
        .to_path_buf();
    // Pre-compute whether this file imports `MarkingScheme` from
    // `marque_scheme`. The bare-form `impl MarkingScheme for X` is
    // legitimate IFF the file has a `use marque_scheme::MarkingScheme`
    // (or a `use marque_scheme::*` glob) at the top — that's the
    // established adapter convention in the codebase (see
    // `crates/capco/src/scheme.rs:1267` for the canonical example).
    // Without an import context the bare form is shadow-trait
    // suspicious.
    let imports_marking_scheme = file_imports_marking_scheme_from_marque_scheme(file);
    let mut walker = SignatureWalker {
        file_path,
        rel_path: rel,
        imports_marking_scheme,
        sink,
    };
    walker.visit_items(&file.items, /* impl_trait_last = */ None);
}

/// Walk every workspace member's `src/` and `tests/` plus the
/// workspace-root `tests/` directory, returning every `*.rs` path.
///
/// Coverage exactly mirrors the callsite pass
/// (`callsite::collect_rust_files`): `crates/*/{src,tests}/**`, any
/// top-level workspace member's `src/` and `tests/` (the binary
/// `marque/` crate today, plus any future top-level member), and the
/// workspace-root `tests/**`. Restricting the signature pass to a
/// strict subset of those trees would let a future
/// `ParsedAttrs → CanonicalAttrs` adapter added in the omitted scope
/// bypass PRC100 entirely.
///
/// Walk errors are surfaced rather than silently dropped: a permission
/// error or broken directory entry would otherwise cause the
/// signature pass to skip a subtree and still exit 0 — false-green for
/// a CI enforcement tool.
fn collect_rust_files(workspace_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();

    // crates/*/{src,tests}/**
    let crates_dir = workspace_dir.join("crates");
    if crates_dir.is_dir() {
        for crate_entry in fs::read_dir(&crates_dir)
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
                    walk_rust_files(&sub_path, &mut out)?;
                }
            }
        }
    }

    // Top-level workspace members (any directory at workspace root that
    // contains a `Cargo.toml` and a `src/` or `tests/`). Mirror the
    // skip-list and policy in `callsite::collect_rust_files`.
    for entry in fs::read_dir(workspace_dir)
        .with_context(|| format!("reading {}", workspace_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
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
                walk_rust_files(&sub_path, &mut out)?;
            }
        }
    }

    // workspace-root tests/**
    let tests_dir = workspace_dir.join("tests");
    if tests_dir.is_dir() {
        walk_rust_files(&tests_dir, &mut out)?;
    }

    Ok(out)
}

fn walk_rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in WalkDir::new(dir) {
        let entry = entry.with_context(|| format!("walking {}", dir.display()))?;
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|ext| ext == "rs") {
            out.push(p.to_path_buf());
        }
    }
    Ok(())
}

struct SignatureWalker<'a> {
    file_path: &'a Path,
    rel_path: PathBuf,
    /// Pre-computed flag: does the file have a `use marque_scheme::MarkingScheme`
    /// (or a `use marque_scheme::*` glob) at the top? Drives the
    /// bare-form acceptance for `is_marking_scheme_trait_path`.
    imports_marking_scheme: bool,
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
                // Recurse into the function body so nested local
                // `fn` items inside a fn's block are visited too.
                // Without this, `fn outer() { fn inner(p: ParsedAttrs)
                // -> CanonicalAttrs { ... } }` would let `inner`
                // declare the prohibited shape invisibly to PRC100.
                self.visit_block(&item_fn.block);
            }
            Item::Mod(item_mod) => self.visit_mod(item_mod, impl_trait_last),
            Item::Impl(item_impl) => self.visit_impl(item_impl),
            // Trait declarations: a `trait T { fn convert(p: ParsedAttrs) -> CanonicalAttrs; }`
            // would otherwise reopen the prohibited shape because every
            // downstream impl inherits the signature. Visit the trait's
            // method signatures so a new trait declaring the prohibited
            // shape gets flagged as PRC100. The `MarkingScheme::canonicalize`
            // carve-out is honored at the trait-declaration site too —
            // the trait DECLARATION of `MarkingScheme::canonicalize` is
            // the single legitimate definition the carve-out cites, so
            // synthesizing a one-segment trait path from the trait's
            // own name lets `is_marking_scheme_trait_path` recognize it.
            Item::Trait(item_trait) => self.visit_trait(item_trait),
            _ => {}
        }
    }

    fn visit_trait(&mut self, item_trait: &ItemTrait) {
        // The trait DECLARATION case is special: we can't tell from
        // the AST alone whether a trait named `MarkingScheme` is the
        // canonical `marque_scheme::MarkingScheme` or a shadow trait
        // declared elsewhere. Use the file path as the discriminator:
        // the canonical trait lives at `crates/scheme/src/**`, so
        // any trait with that name there is the legitimate one.
        // Anywhere else, a trait merely *named* `MarkingScheme` is
        // suspicious and gets the standard PRC100 treatment with no
        // carve-out.
        let is_canonical_marking_scheme_decl =
            item_trait.ident == "MarkingScheme" && self.rel_path_is_marque_scheme_decl_file();
        let synthesized = if is_canonical_marking_scheme_decl {
            // Synthesize the qualified path so `is_marking_scheme_trait_path`
            // recognizes it via the same shared matcher used at impl
            // sites. This keeps the matcher single-source-of-truth.
            Some(qualified_marking_scheme_path())
        } else {
            None
        };
        for trait_item in &item_trait.items {
            if let TraitItem::Fn(method) = trait_item {
                self.maybe_emit_for_signature(&method.sig, synthesized.as_ref());
            }
        }
    }

    /// True when the rel-path is exactly `crates/scheme/src/scheme.rs`,
    /// where the canonical `marque_scheme::MarkingScheme` trait is
    /// declared. Any other file under `crates/scheme/src/` could
    /// declare a shadow trait merely *named* `MarkingScheme` and is
    /// not a legitimate carve-out site — accepting the broader
    /// directory match would defeat the canonical-path discipline.
    fn rel_path_is_marque_scheme_decl_file(&self) -> bool {
        let comps: Vec<_> = self.rel_path.components().collect();
        comps.len() == 4
            && comps[0].as_os_str() == "crates"
            && comps[1].as_os_str() == "scheme"
            && comps[2].as_os_str() == "src"
            && comps[3].as_os_str() == "scheme.rs"
    }

    fn visit_mod(&mut self, item_mod: &ItemMod, impl_trait_last: Option<&str>) {
        if let Some((_, items)) = &item_mod.content {
            self.visit_items(items, impl_trait_last);
        }
    }

    fn visit_impl(&mut self, item_impl: &ItemImpl) {
        // Whitelist 2 keys on the FULL trait path written at the impl
        // site. Only the fully-qualified `marque_scheme::MarkingScheme`
        // form is accepted; the bare single-segment form is rejected
        // because at the AST level it could be a crate-local shadow
        // trait. Forcing the qualified path makes the carve-out
        // unambiguous (see `is_marking_scheme_trait_path`).
        let trait_path: Option<&SynPath> = item_impl.trait_.as_ref().map(|(_, path, _)| path);

        for impl_item in &item_impl.items {
            if let ImplItem::Fn(method) = impl_item {
                self.maybe_emit_for_signature(&method.sig, trait_path);
                // Recurse into the method body for block-scoped local
                // `fn` items. Same rationale as `Item::Fn` recursion
                // above: a `fn helper(p: ParsedAttrs) -> CanonicalAttrs`
                // declared inside a method body would otherwise be
                // invisible to PRC100.
                self.visit_block(&method.block);
            }
        }
    }

    fn visit_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            if let Stmt::Item(inner_item) = stmt {
                // Local fn inside another fn body: NOT inside any
                // `impl <Trait> for X`, so the carve-out path arg is
                // `None`. A nested `impl` or `trait` declaration
                // recurses through the standard visitor.
                self.visit_item(inner_item, None);
            }
        }
    }

    fn maybe_emit_for_signature(&mut self, sig: &Signature, impl_trait_path: Option<&SynPath>) {
        // Whitelist 1: any `unsafe fn`.
        if sig.unsafety.is_some() {
            return;
        }

        // Top-level shape match: the prohibited shape is a function
        // whose FIRST argument's top-level type is `ParsedAttrs`
        // (optionally `&ParsedAttrs<'_>`) AND whose return type is
        // `CanonicalAttrs` directly or `Result<CanonicalAttrs, _>`.
        // Matching anywhere-in-type would flag legitimate adapters
        // like `fn register(f: fn(ParsedAttrs) -> CanonicalAttrs)`,
        // `fn wrap(x: Vec<ParsedAttrs>) -> Option<CanonicalAttrs>`,
        // and similar wrappers that are not themselves performing
        // the forbidden conversion.
        if !signature_has_prohibited_shape(sig) {
            return;
        }

        // Whitelist 2: `MarkingScheme::canonicalize` where the trait
        // path is the fully-qualified `marque_scheme::MarkingScheme`
        // form. The bare single-segment form is rejected (see
        // `is_marking_scheme_trait_path`); for trait DECLARATIONS the
        // canonical-file path discriminator at
        // `rel_path_is_marque_scheme_decl_file` substitutes for the
        // qualified-path check (a `pub trait MarkingScheme` declared
        // inside `crates/scheme/src/scheme.rs` is the genuine one).
        if let Some(trait_path) = impl_trait_path {
            if is_marking_scheme_trait_path(trait_path, self.imports_marking_scheme)
                && sig.ident == CANONICALIZE_METHOD
            {
                return;
            }
        }

        // Whitelist 3 (`crates/ism/src/canonical.rs::from_parsed_unchecked`)
        // retired along with the adapter itself.

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
                 (signature-shape discipline)",
                fn_name = sig.ident,
            ),
        });
    }

}

/// Returns `true` when the signature matches the prohibited
/// `ParsedAttrs → CanonicalAttrs` shape *at the top level* of its
/// argument and return types.
///
/// "Top level" matters: the lint target is a function that *itself*
/// performs the forbidden conversion, not one that incidentally
/// mentions the types deep inside a generic. Concretely:
///
/// - At least one `FnArg::Typed` whose type is `ParsedAttrs` or
///   `&ParsedAttrs` (with optional lifetime / generic-arg list).
/// - Return type is `CanonicalAttrs`, `&CanonicalAttrs`, or
///   `Result<CanonicalAttrs, _>` at the top level.
///
/// This rejects false positives like `Vec<ParsedAttrs>`,
/// `Option<CanonicalAttrs>`, `fn(ParsedAttrs) -> CanonicalAttrs` (a
/// function-pointer parameter), and similar shapes where the types
/// appear nested inside a wrapper.
fn signature_has_prohibited_shape(sig: &Signature) -> bool {
    let any_arg_is_parsed = sig.inputs.iter().any(|arg| match arg {
        FnArg::Typed(pat_type) => is_top_level_named_type_or_ref(&pat_type.ty, PARSED_TYPE_NAME),
        FnArg::Receiver(_) => false,
    });
    if !any_arg_is_parsed {
        return false;
    }
    let ReturnType::Type(_, ret_ty) = &sig.output else {
        return false;
    };
    return_is_canonical_or_result_canonical(ret_ty)
}

/// True when `ty`, after stripping any `Paren` / `Group` AST
/// wrappers, is a `Type::Path` whose terminal segment is `name`.
///
/// The argument-side caller (`signature_has_prohibited_shape` for
/// `FnArg::Typed`) wraps this matcher in a separate reference-stripper
/// so `&ParsedAttrs<'_>` arguments are recognized — the prohibited
/// shape on the input side is "takes ownership of OR borrows
/// `ParsedAttrs`."
///
/// The return-side matcher does NOT strip references because the
/// D12 contract is `CanonicalAttrs` or `Result<CanonicalAttrs, _>`
/// only. A function returning `&CanonicalAttrs` is a borrow-returning
/// helper (e.g. accessing a cached value) and is outside the
/// prohibited-constructor shape; flagging it would block legitimate
/// helpers.
fn is_top_level_named_type(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Paren(p) => is_top_level_named_type(&p.elem, name),
        Type::Group(g) => is_top_level_named_type(&g.elem, name),
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == name),
        _ => false,
    }
}

/// True when `ty` is `name` directly OR a single-layer reference
/// (`&name`, `&mut name`) to it. Used for argument-side matching
/// where the prohibited shape covers both owned and borrowed inputs.
fn is_top_level_named_type_or_ref(ty: &Type, name: &str) -> bool {
    if is_top_level_named_type(ty, name) {
        return true;
    }
    if let Type::Reference(r) = ty {
        return is_top_level_named_type(&r.elem, name);
    }
    false
}

/// True when `ty` is `CanonicalAttrs` (or a reference to one) or
/// `Result<CanonicalAttrs, _>` at the top level.
///
/// The `Result` variant matches **only on the Ok slot** — the first
/// generic argument. A return type like `Result<ParseError, CanonicalAttrs>`
/// (`CanonicalAttrs` in the *Err* slot) is not the D12 forbidden shape
/// because the function is not converting *to* `CanonicalAttrs`; it's
/// returning `CanonicalAttrs` only on failure. Matching either generic
/// argument was a precision bug — the rule is specifically
/// `Result<CanonicalAttrs, _>`.
fn return_is_canonical_or_result_canonical(ty: &Type) -> bool {
    if is_top_level_named_type(ty, CANONICAL_TYPE_NAME) {
        return true;
    }
    let inner = strip_paren_group_layers(ty);
    let Type::Path(type_path) = inner else {
        return false;
    };
    let Some(last) = type_path.path.segments.last() else {
        return false;
    };
    if last.ident != RESULT_TYPE_NAME {
        return false;
    }
    let PathArguments::AngleBracketed(angle) = &last.arguments else {
        return false;
    };
    // Find the FIRST type-shaped generic argument — that's the Ok
    // slot of `Result<Ok, Err>`. (Lifetime arguments like `Result<'a,
    // T, E>` don't exist for std `Result`, but if a contributor used
    // a custom `Result`-named type with leading-lifetime generics we
    // correctly skip past them.)
    angle
        .args
        .iter()
        .find_map(|ga| match ga {
            GenericArgument::Type(t) => Some(t),
            _ => None,
        })
        .is_some_and(|ok_ty| is_top_level_named_type(ok_ty, CANONICAL_TYPE_NAME))
}

/// Strip leading paren/group wrappers, leaving the inner type that
/// decides the top-level shape. References are deliberately NOT
/// stripped: the D12 prohibited return shape covers
/// `CanonicalAttrs` and `Result<CanonicalAttrs, _>` only — the
/// borrowed forms (`&CanonicalAttrs`, `&Result<CanonicalAttrs, _>`)
/// are legitimate helper shapes outside the contract.
fn strip_paren_group_layers(ty: &Type) -> &Type {
    match ty {
        Type::Paren(p) => strip_paren_group_layers(&p.elem),
        Type::Group(g) => strip_paren_group_layers(&g.elem),
        _ => ty,
    }
}

/// Build the canonical fully-qualified `marque_scheme::MarkingScheme`
/// path so the shared trait-path matcher can recognize a synthesized
/// path coming from a trait declaration in the canonical file.
fn qualified_marking_scheme_path() -> SynPath {
    syn::parse_str::<SynPath>("marque_scheme::MarkingScheme").expect("static path string parses")
}

/// True when the trait path written at an `impl <Trait> for <Type>`
/// site references the canonical `MarkingScheme` trait, accepting:
///
/// 1. **Fully-qualified `marque_scheme::MarkingScheme`** — always
///    accepted; the qualifier is unambiguous regardless of file
///    context.
/// 2. **Bare `MarkingScheme`** — accepted IFF the file imports
///    `MarkingScheme` from `marque_scheme` (either via
///    `use marque_scheme::MarkingScheme`, `use marque_scheme::{..., MarkingScheme, ...}`,
///    or `use marque_scheme::*` glob). The `imports_marking_scheme`
///    flag is pre-computed per-file by
///    `file_imports_marking_scheme_from_marque_scheme` and threaded
///    through here. This matches the established adapter
///    convention in the codebase (e.g. `crates/capco/src/scheme.rs`
///    uses `impl MarkingScheme for CapcoScheme` after a
///    `use marque_scheme::{...}` block at the top of the file).
///
/// Bare `MarkingScheme` in a file that does NOT import it from
/// `marque_scheme` is treated as a shadow trait and rejected — a
/// crate-local `trait MarkingScheme { ... }` defined in some other
/// file would otherwise satisfy a single-segment matcher and
/// whitelist arbitrary `ParsedAttrs → CanonicalAttrs` adapters.
/// The import-aware match closes that bypass at AST level without
/// requiring contributors to write the verbose qualified form.
fn is_marking_scheme_trait_path(path: &SynPath, imports_marking_scheme: bool) -> bool {
    let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    // Fully-qualified form: always accepted.
    if matches!(
        segs.as_slice(),
        [crate_name, trait_name] if crate_name == "marque_scheme" && trait_name == "MarkingScheme",
    ) {
        return true;
    }
    // Bare form: accepted only when the file imports it from
    // `marque_scheme` — a `use marque_scheme::MarkingScheme`,
    // a `use marque_scheme::{..., MarkingScheme, ...}` group, or a
    // `use marque_scheme::*` glob.
    if imports_marking_scheme && matches!(segs.as_slice(), [single] if single == "MarkingScheme") {
        return true;
    }
    false
}

/// Walk every `Item::Use` in the file and return `true` if the file
/// imports `MarkingScheme` from `marque_scheme`. Three import shapes
/// satisfy:
///
/// - `use marque_scheme::MarkingScheme;`
/// - `use marque_scheme::{..., MarkingScheme, ...};` (group)
/// - `use marque_scheme::*;` (glob — pulls in everything including
///   `MarkingScheme`)
///
/// `as`-rename shapes (`use marque_scheme::MarkingScheme as Foo;`)
/// are NOT counted: the local name is no longer `MarkingScheme`,
/// so a `impl MarkingScheme for X` in the same file does NOT refer
/// to the renamed import. Only the canonical local name binds.
fn file_imports_marking_scheme_from_marque_scheme(file: &File) -> bool {
    use syn::{ItemUse, UseTree};
    fn tree_matches(tree: &UseTree, expecting_marque_scheme_root: bool) -> bool {
        match tree {
            // `marque_scheme::<...>` — descend into the right child.
            UseTree::Path(path) => {
                if expecting_marque_scheme_root {
                    if path.ident == "marque_scheme" {
                        return tree_matches(&path.tree, false);
                    }
                    return false;
                }
                // Already inside marque_scheme; we don't recurse
                // through inner modules (`marque_scheme::sub::MarkingScheme`
                // would be reading from a non-canonical path and is
                // not a recognized import shape).
                false
            }
            // `marque_scheme::MarkingScheme;` — terminal name match.
            UseTree::Name(name) => !expecting_marque_scheme_root && name.ident == "MarkingScheme",
            // `marque_scheme::*;` — glob brings in MarkingScheme.
            UseTree::Glob(_) => !expecting_marque_scheme_root,
            // `marque_scheme::{...}` — group; check each entry.
            UseTree::Group(group) => {
                if expecting_marque_scheme_root {
                    return false;
                }
                group.items.iter().any(|item| tree_matches(item, false))
            }
            // `as`-rename: explicitly excluded (see fn doc comment).
            UseTree::Rename(_) => false,
        }
    }
    file.items.iter().any(|item| match item {
        syn::Item::Use(ItemUse { tree, .. }) => tree_matches(tree, true),
        _ => false,
    })
}
