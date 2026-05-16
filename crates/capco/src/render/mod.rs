// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Commit 5 — per-axis canonical renderers for [`CapcoScheme`].
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
//! Per the architecture restatement
//! (`specs/006-engine-rule-refactor/architecture.md` §"What this commits
//! us to"): the renderer is the single source of canonical form. Form
//! rules retire into it. The dispatch table walks per-axis renderers in
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
