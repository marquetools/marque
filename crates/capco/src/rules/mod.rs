// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO rule implementations — Layer 2 diagnostic intelligence.
//!
//! Each rule uses Layer 1 schema predicates (from generated/validators.rs) to
//! detect violations, then produces enriched diagnostics with fixes and
//! confidence, with byte-precise spans threaded through
//! `CanonicalAttrs::token_spans`.
//!
//! Rule IDs follow the 2-tuple wire-string form
//! `<scheme>:<surface>.<category>.<predicate>`. The current registered
//! set is enumerated in `crates/capco/README.md` and pinned at
//! `crates/capco/tests/post_3b_registration_pin.rs`. The legacy-ID ↔
//! wire-string translation table lives at
//! `docs/refactor-006/legacy-rule-id-map.md`.

mod banner;
mod dissem;
mod dissem_closure;
mod eyes;
mod fgi;
mod fgi_concealment;
mod form_mismatch;
mod helpers;
mod joint;
mod nato;
mod nodis_exdis;
mod recanonicalize;
mod registry;
mod rel_to;
mod rel_to_suggest;
mod rel_to_uncertainty;
mod sci;
mod sci_deprecated;
mod text_handling;

pub(crate) use helpers::{FixDiagnosticParams, make_fix_diagnostic};
pub use registry::CapcoRuleSet;

// `helpers::sar_block_span` is intentionally NOT re-exported here.
// Its only consumer is `rules::banner::eval_sar`, which reaches it
// via `super::super::helpers::sar_block_span` (grandchild path).
// No cross-`rules/` consumer exists; re-exporting at the `rules`
// module surface would advertise an internal helper to other crate
// modules with no caller. Promote to a `pub(crate) use` here if a
// future scheme/render/audit-side consumer needs the SAR span
// computation.

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod citation_cross_refs_tests;
