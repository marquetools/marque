// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI per-system catalog rows per CAPCO-2016 §H.4.
//!
//! Row order is load-bearing.

use marque_scheme::{Constraint, SectionLetter, capco};

// ================================================================
// SCI per-system catalog (§H.4)
// ================================================================
//
// Per-SCI-system companion-required / forbid-companion invariants per
// CAPCO-2016 §H.4. Five rows at family granularity covering the §H.4
// invariants that the class-floor catalog does NOT already cover
// (companion-required: ORCON, NOFORN; forbid-companion: ORCON-USGOV).
// The class-floor portions of these systems are absorbed by the
// class-floor catalog rows and are not duplicated here.
//
// # Why Constraint::Custom (architectural choice)
//
// The §H.4 invariants are companion-presence (ORCON, NOFORN) +
// companion-forbid (ORCON-USGOV) + per-row fix-shape (zero-width
// insertion at the end of the IC dissem block, or a span replacement on
// the dominated token) — none of which fit the existing primitive
// surface. A future change MAY re-classify to a `CompanionRequired<Set>`
// / `Forbid<Set>` primitive on `marque-scheme` when those primitives
// land.
//
// # Per-row name and rule-ID
//
// Each catalog row's `name` is the canonical predicate ID of the form
// `marking.sci.<purpose>`. The engine's constraint-catalog bridge
// constructs `RuleId::new("capco", name)` directly, so each row is
// independently configurable in `.marque.toml` by that predicate ID.

/// The SCI per-system section of the constraint catalog.
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
