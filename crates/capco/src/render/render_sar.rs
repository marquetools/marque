// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SAR axis renderer (Special Access Required programs).
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p16 — SAR formatting: "The first value in the SAP
//!   category is the SAP category indicator either `SPECIAL ACCESS
//!   REQUIRED-` or `SAR-` ... If multiple SAP program identifiers are
//!   required, each subsequent SAP program identifier must be listed in
//!   ascending sort order with all numbered values first, followed by
//!   alphabetic values separated by a single forward slash without
//!   interjected spaces. The SAR- category indicator is not repeated
//!   when multiple program indicators are used. ... Compartment(s) (if
//!   any) ... must be kept with the SAP program identifier listed in
//!   ascending sort order with all numbered values first, followed by
//!   alphabetic values separated by a hyphen. Sub-compartment(s) (if
//!   any), must be kept with the compartment, listed in ascending sort
//!   order with numbered values first, followed by alphabetic values
//!   and separated by a single space."
//! - CAPCO-2016 §A.6 p16 example: `SECRET//SAR-ABC-DEF 123/SDA-121//
//!   NOFORN`.
//! - CAPCO-2016 §H.5 p99-100 — SAR program/compartment/sub-compartment
//!   hierarchy and indicator forms.
//!
//! # Canonical form
//!
//! `SAR-PROG[-COMP[ SUB ...][-COMP ...]][/PROG2-...]`
//!
//! - Single `SAR-` indicator (the `SPECIAL ACCESS REQUIRED-` long form
//!   appears only in banners and is normalized to the abbreviated form
//!   per §H.5 p100; the existing `PageContext::expected_sar_marking`
//!   already canonicalizes to `SarIndicator::Abbrev` and the renderer
//!   honors that choice).
//! - Programs are `/`-separated, ascending alpha (numeric-first).
//! - Compartments are `-`-separated within a program, ascending alpha
//!   (numeric-first).
//! - Sub-compartments are space-separated, ascending alpha
//!   (numeric-first).
//! - Banner and portion forms are identical.

use core::fmt;

use marque_ism::SarMarking;
use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the SAR axis to `out`. Banner and portion forms are
/// identical (the `SAR-` indicator is the canonical short form per
/// §H.5 p100). The full form `SPECIAL ACCESS REQUIRED-` is required
/// when any program identifier contains a space — multi-word program
/// names (per §H.5 p99-100 "program's nickname or authorized digraph
/// or trigraph") cannot be carried under the abbreviated `SAR-` form.
/// The space-detection heuristic is the load-bearing rule; the
/// per-identifier character set is not constrained by the manual to
/// any specific regex despite the examples showing 2-3 char
/// abbreviations.
pub(crate) fn render_sar(m: &CapcoMarking, _scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    let Some(sar) = &m.0.sar_markings else {
        return Ok(());
    };
    render_block(sar, out)
}

fn render_block(sar: &SarMarking, out: &mut dyn fmt::Write) -> fmt::Result {
    // Indicator choice: any multi-word program identifier (containing
    // a space) requires the full `SPECIAL ACCESS REQUIRED-` indicator
    // — the abbreviated `SAR-` form is reserved for compact program
    // identifiers (CAPCO-2016 §H.5 p99 "program's nickname or
    // authorized digraph or trigraph" + §H.5 p100 indicator
    // grammar). Canonical default is `SAR-` when no program
    // identifier contains a space.
    let needs_full = sar.programs.iter().any(|p| p.identifier.contains(' '));
    if needs_full {
        out.write_str("SPECIAL ACCESS REQUIRED-")?;
    } else {
        out.write_str("SAR-")?;
    }

    // Programs ascending alpha (numeric first per §A.6 p16). Inline-4
    // covers the typical SAR cardinality (single program common; up to
    // ~4 programs in compound markings); compartments/sub-compartments
    // similarly cap at ~4 per program in observed §H.5 markings.
    let mut programs: SmallVec<[_; 4]> = sar.programs.iter().collect();
    programs.sort_by(|a, b| numeric_then_alpha_cmp(&a.identifier, &b.identifier));

    let mut first_prog = true;
    for prog in programs {
        if !first_prog {
            out.write_char('/')?;
        }
        first_prog = false;
        out.write_str(&prog.identifier)?;

        // Compartments ascending alpha (numeric first), `-`-separated.
        let mut comps: SmallVec<[_; 4]> = prog.compartments.iter().collect();
        comps.sort_by(|a, b| numeric_then_alpha_cmp(&a.identifier, &b.identifier));
        for comp in comps {
            out.write_char('-')?;
            out.write_str(&comp.identifier)?;

            // Sub-compartments ascending alpha (numeric first),
            // space-separated.
            let mut subs: SmallVec<[&str; 4]> =
                comp.sub_compartments.iter().map(|s| s.as_ref()).collect();
            subs.sort_by(|a, b| numeric_then_alpha_cmp(a, b));
            for sub in subs {
                out.write_char(' ')?;
                out.write_str(sub)?;
            }
        }
    }
    Ok(())
}

/// Numeric tokens sort before alphabetic; within each bucket, lex
/// order. Same convention as the SCI axis (§A.6 p15-16). Shared helper
/// imported from `super` — see [`super::numeric_then_alpha_cmp`].
use super::numeric_then_alpha_cmp;
