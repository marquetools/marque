// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time HRTB smoke test for `MarkingScheme::type Parsed<'src>`.
//!
//! The GAT `type Parsed<'src>` was introduced at PR 3c.2.A per
//! `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` PM-1. Per
//! B-FOLLOWUP-1 in the 3c.2.A deferred-findings register
//! (`specs/006-engine-rule-refactor/followups/2026-05-19-pr-3c2-a-deferred-findings.md`),
//! this is a forward-defense pin: if a future change to `MarkingScheme`'s
//! bounds destabilizes HRTB inference (`for<'a>` quantification) over
//! `S::Parsed<'a>`, this test fails at compile time — before a generic
//! helper site downstream surfaces "implementation not general enough"
//! error noise.
//!
//! Placement (per PM-B-5): in the crate that DECLARES the GAT so
//! a future scheme implementor (CUI, NATO) whose binding destabilizes
//! HRTB inference sees the test break in the same crate that introduced
//! the regression. Engine-test placement would be downstream; trait-test
//! placement catches it earlier.

use marque_scheme::MarkingScheme;

/// Compile-time-only: takes `&S` for any scheme whose `Parsed<'a>` is
/// `Sized` for every `'a`. The empty body never runs; the type
/// signature is the load-bearing artifact.
#[allow(dead_code)]
fn _hrtb_smoke<S: MarkingScheme>(_scheme: &S)
where
    for<'a> S::Parsed<'a>: Sized,
{
}

#[test]
fn hrtb_smoke_compiles() {
    // The mere existence of `_hrtb_smoke` as a definable function
    // proves the HRTB bound resolves. No runtime assertion needed.
}
