// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO rule implementations — Layer 2 diagnostic intelligence.
//!
//! Each rule uses Layer 1 schema predicates (from generated/validators.rs) to
//! detect violations, then produces enriched diagnostics with fixes and
//! confidence. Phase 3 lands the full set of MVP rules with byte-precise
//! spans threaded through `CanonicalAttrs::token_spans`.
//!
//! Rule IDs follow the post-T044 (2026-05-22) 2-tuple wire-string form
//! `<scheme>:<surface>.<category>.<predicate>`. The current registered
//! set is enumerated in `crates/capco/README.md` and pinned at
//! `crates/capco/tests/post_3b_registration_pin.rs`.
//!
//! Retirement provenance for the historical `E### / W### / S###`
//! flat-string IDs lives at `crates/capco/docs/archaeology/`
//! (`retirement-history.md` for the per-rule retirement record,
//! `rule-id-cross-refs.md` for inline cross-refs grouped by live rule).
//! The T044 legacy-ID ↔ wire-string translation table lives at
//! `docs/refactor-006/legacy-rule-id-map.md`.

pub(crate) mod banner;
pub(crate) mod dissem;
pub(crate) mod dissem_closure;
pub(crate) mod eyes;
pub(crate) mod fgi;
pub(crate) mod fgi_concealment;
pub(crate) mod form_mismatch;
pub(crate) mod helpers;
pub(crate) mod joint;
pub(crate) mod nato;
pub(crate) mod nodis_exdis;
mod registry;
pub(crate) mod rel_to;
pub(crate) mod rel_to_suggest;
pub(crate) mod rel_to_uncertainty;
pub(crate) mod sci;
pub(crate) mod text_handling;

pub use registry::CapcoRuleSet;
pub(crate) use helpers::{FixDiagnosticParams, make_fix_diagnostic};

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod citation_cross_refs_tests;
