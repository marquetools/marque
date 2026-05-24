// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Per-axis canonical renderers for [`CapcoScheme`].
//!
//! This module hosts the per-axis renderer functions that
//! [`crate::scheme::RENDER_TABLE`] dispatches over. Each file owns one
//! CAPCO category (classification, SCI, SAR, AEA, FGI, dissem, REL TO,
//! non-IC dissem, declassify-on); each `pub(crate) fn` appends bytes to
//! the caller-supplied writer per the
//! [`marque_scheme::MarkingScheme::render_canonical`] contract.
//!
//! # Design
//!
//! The renderer is the single source of canonical form; form rules
//! retire into it. The dispatch table walks per-axis renderers in
//! `Category::ordering_rank` order, inserting `//` between major
//! categories (per CAPCO-2016 §A.6 p15-17 Figure 2). Each axis renderer
//! writes ONLY its own bytes — never the leading `//` separator.
//!
//! # Verification oracle
//!
//! Per Constitution VIII (Authoritative Source Fidelity), every per-axis
//! renderer body's behavior matches `crates/capco/docs/CAPCO-2016.md`
//! §H.1, §H.3, §H.4, §H.5, §H.6, §H.7, §H.8, §H.9 (the per-axis
//! sections cited in each render_*.rs module). Where existing rule
//! logic and the manual disagree, the manual wins. The §-citation for
//! each axis lives in that file's module-level doc comment; per-row
//! golden-output fixtures cite the specific subsection (e.g.,
//! CAPCO-2016 §H.4 p61) that defines the canonical form.
//!
//! # Writer-passing contract
//!
//! All per-axis fns:
//! - Take `&CapcoMarking` (read-only; the scheme caller projects the
//!   page-context-rolled-up [`marque_ism::CanonicalAttrs`] before
//!   render).
//! - Take [`marque_scheme::Scope`] to choose between portion form and
//!   banner form (and Document, which agrees with Page on
//!   single-portion markings).
//! - Take `&mut dyn fmt::Write` and APPEND to it. They MUST NOT clear
//!   the writer; the caller owns the buffer's lifetime and may reuse it
//!   across portions.
//! - Return `Ok(())` on success; the only `Err` path is when the
//!   underlying writer returns an error (the dispatch loop propagates).

pub(crate) mod render_aea;
pub(crate) mod render_classification;
pub(crate) mod render_declassify;
pub(crate) mod render_display_only;
pub(crate) mod render_dissem;
pub(crate) mod render_fgi;
pub(crate) mod render_non_ic_dissem;
pub(crate) mod render_rel_to;
pub(crate) mod render_sar;
pub(crate) mod render_sci;

// ---------------------------------------------------------------------------
// Shared sort helpers
// ---------------------------------------------------------------------------

/// Compare two tokens with "numeric tokens first, then alphabetic; within
/// each bucket lex order" semantics.
///
/// Per CAPCO-2016 §A.6 p15-16: "Multiple values within each hierarchical
/// level are listed in ascending sort order with all numbered values first,
/// then followed by alphabetic values." A numeric token is one whose first
/// character is an ASCII digit; mixed alphanumerics like `BLFH` are
/// alphabetic. Example p16: `123` (numeric) sorts before `SI-G` (alpha).
///
/// Used by SCI compartment/sub-compartment sort (§H.4 p61), SAR program /
/// compartment sort (§H.5 p99), AEA SIGMA numeric sort (§H.6 p108). Shared
/// here so a future per-axis update to the sort convention applies in one
/// place rather than drifting across §H.4 / §H.5 / §H.6 callers.
pub(crate) fn numeric_then_alpha_cmp(a: &str, b: &str) -> core::cmp::Ordering {
    let a_num = is_numeric_first(a);
    let b_num = is_numeric_first(b);
    match (a_num, b_num) {
        (true, false) => core::cmp::Ordering::Less,
        (false, true) => core::cmp::Ordering::Greater,
        _ => a.cmp(b),
    }
}

/// True if `s` starts with an ASCII digit.
pub(crate) fn is_numeric_first(s: &str) -> bool {
    s.bytes().next().is_some_and(|b| b.is_ascii_digit())
}

// ---------------------------------------------------------------------------
// Named-fn-item comparators — WASM bundle-size collapse (R1 / issue #689)
// ---------------------------------------------------------------------------
//
// These helpers extend the PR #585 precedent established by
// `crate::lattice::helpers::sort_smolstrs_by_sar` (see the doc-comment on that
// function for the original mono-collapse rationale). Each comparator is a
// concrete-typed `fn`-item — not a closure, not generic — so every callsite
// that passes the same `fn`-item to `slice::sort_by` (resp.
// `slice::sort_by_key`) shares one closure-axis monomorphization with every
// other callsite using the same `fn`-item.
//
// Two-axis Rust trap recap (per the R1 rust-feasibility preflight §3):
//
// 1. `slice::sort_by` passes `&T` to its comparator. When the slice element
//    type is itself a borrow (`&SciMarking`, `&str`, …), the comparator
//    receives `&&T`. Each comparator below declares the double-borrow shape
//    explicitly so the `fn`-item-to-`fn`-pointer coercion at the call site
//    succeeds.
//
// 2. Adding generics to these comparators (`T: Ord`) breaks the coercion.
//    Each comparator stays concrete-typed — duplicate one per `T` rather
//    than generalize.
//
// Deliberately NOT `#[inline]` per the PR #585 doc-comment: inlining a
// wrapping `fn` re-monomorphizes its body at every inline site, defeating
// the consolidation. Workspace `lto = "fat"` (`Cargo.toml`) handles
// profitable inlining naturally if it would be a win.

/// Compare two `&&str` references via [`numeric_then_alpha_cmp`].
///
/// Targets `slice::sort_by` over `&mut [&str]` (the
/// `SmallVec<[&str; 4]>` shape used at SAR sub-compartment + SCI
/// sub-compartment render sites). Per CAPCO-2016 §A.6 p15-16.
pub(crate) fn cmp_str_numeric_then_alpha(a: &&str, b: &&str) -> core::cmp::Ordering {
    numeric_then_alpha_cmp(a, b)
}

/// Compare two `&&SarProgram` references on `identifier` via
/// [`numeric_then_alpha_cmp`].
///
/// Targets `slice::sort_by` over `&mut [&SarProgram]` (the
/// `SmallVec<[&SarProgram; 4]>` programs scratch at SAR render). Per
/// CAPCO-2016 §H.5 p99-100 (SAR program ascending sort).
pub(crate) fn cmp_sar_program_ident(
    a: &&marque_ism::SarProgram,
    b: &&marque_ism::SarProgram,
) -> core::cmp::Ordering {
    numeric_then_alpha_cmp(&a.identifier, &b.identifier)
}

/// Compare two `&&SarCompartment` references on `identifier` via
/// [`numeric_then_alpha_cmp`].
///
/// Targets `slice::sort_by` over `&mut [&SarCompartment]` (the
/// `SmallVec<[&SarCompartment; 4]>` compartments scratch at SAR render).
/// Per CAPCO-2016 §H.5 p99-100.
pub(crate) fn cmp_sar_compartment_ident(
    a: &&marque_ism::SarCompartment,
    b: &&marque_ism::SarCompartment,
) -> core::cmp::Ordering {
    numeric_then_alpha_cmp(&a.identifier, &b.identifier)
}

/// Compare two `&&SciCompartment` references on `identifier` via
/// [`numeric_then_alpha_cmp`].
///
/// Targets `slice::sort_by` over `&mut [&SciCompartment]` (the
/// `SmallVec<[&SciCompartment; 4]>` compartments scratch at SCI
/// render). Per CAPCO-2016 §A.6 p15-16 + §H.4 p61.
pub(crate) fn cmp_sci_compartment_ident(
    a: &&marque_ism::SciCompartment,
    b: &&marque_ism::SciCompartment,
) -> core::cmp::Ordering {
    numeric_then_alpha_cmp(&a.identifier, &b.identifier)
}
