// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3b.E (T026e) SCI per-system catalog rows per CAPCO-2016 §H.4.
//! Lifted from the monolithic `constraints.rs` per the issue #466
//! Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).
//!
//! Row order preserved verbatim from the pre-split catalog.

use marque_scheme::{Constraint, SectionLetter, capco};

// ================================================================
// PR 3b.E (T026e) — SCI per-system catalog (§H.4)
// ================================================================
//
// Per-SCI-system companion-required / forbid-companion
// invariants per CAPCO-2016 §H.4. Five rows at family
// granularity covering the §H.4 invariants that PR 3b.D's
// class-floor catalog does NOT already cover (companion-
// required: ORCON, NOFORN; forbid-companion: ORCON-USGOV).
// The class-floor portions of the retired E044/E045/E046/
// E048/E049/E050 rules are absorbed by PR 3b.D's class-floor
// rows and are not duplicated here.
//
// # Why Constraint::Custom (architectural choice)
//
// The §H.4 invariants are companion-presence (ORCON, NOFORN)
// + companion-forbid (ORCON-USGOV) + per-row fix-shape
// (zero-width insertion at the end of the IC dissem block,
// or a span replacement on the dominated token) — none of
// which fit the existing primitive surface. PR 4 (per-
// category Lattice impls per Stage 3 of plan.md:263) MAY
// revisit and re-classify to a `CompanionRequired<Set>` /
// `Forbid<Set>` primitive on `marque-scheme` when those
// primitives land. The walker stays until that retirement.
// See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`
// §3 for the rule-by-rule analysis; tasks.md T026e for the
// walker landing.
//
// # Per-row name and walker rule-ID
//
// The single walker `DeclarativeSciPerSystemRule` (rule ID
// `E059`) emits all diagnostics. Each catalog row's `name`
// takes the `sci-per-system/<purpose>` form. Per project
// memory `feedback_pre_users_no_deprecation_phasing.md`
// (marque is pre-users), severity-config back-compat for
// the retiring E042–E051 rule IDs is not preserved — users
// keying `.marque.toml` at any of `E042`..`E051` must
// migrate to `E059`.

/// The PR 3b.E SCI per-system section of the constraint catalog.
///
/// Returns the 5 SCI per-system rows in declaration order, ready
/// to be appended after the class-floor section by
/// [`build_constraints`](super::build_constraints).
pub(super) fn sci_per_system_constraints() -> Vec<Constraint> {
    vec![
        Constraint::Custom {
            name: "marking.sci.hcs-o-companions",
            label: capco(SectionLetter::H, 4, 64),
        },
        Constraint::Custom {
            name: "marking.sci.hcs-p-noforn-required",
            label: capco(SectionLetter::H, 4, 66),
        },
        Constraint::Custom {
            name: "marking.sci.hcs-p-sub-companions",
            label: capco(SectionLetter::H, 4, 68),
        },
        Constraint::Custom {
            name: "marking.sci.si-g-companions",
            label: capco(SectionLetter::H, 4, 80),
        },
        Constraint::Custom {
            name: "marking.sci.tk-compartment-noforn-required",
            label: capco(SectionLetter::H, 4, 87),
        },
    ]
}
