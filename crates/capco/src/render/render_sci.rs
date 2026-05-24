// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI axis renderer (Sensitive Compartmented Information).
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p15-16 — SCI control system formatting:
//!   "SCI control systems and their compartments must be kept together,
//!   connected by a hyphen (`-`). SCI control system compartments and
//!   their sub-compartments must be kept together, separated by a space.
//!   ... Multiple SCI control systems must be separated by a single
//!   forward slash (`/`). All SCI control systems, their compartments,
//!   and sub-compartments must be listed within each hierarchical level
//!   in ascending sort order with all numbered values first, then
//!   followed by alphabetic values."
//! - CAPCO-2016 §A.6 p16 example: `TOP SECRET//123/SI-G ABCD DEFG-MMM
//!   AACD//ORCON/NOFORN`.
//! - CAPCO-2016 §H.4 p61 — SCI compositional grammar (control →
//!   compartment → sub-compartment).
//!
//! # Canonical form
//!
//! `SYSTEM[-COMP[ SUB...][-COMP[ SUB...]]...][/SYSTEM2-...]`
//!
//! - Multiple control systems are `/`-separated.
//! - Compartments are `-`-separated within a system.
//! - Sub-compartments are space-separated within a compartment.
//! - At every level, numeric identifiers sort first then alphabetic.
//!
//! Banner and portion forms are identical for SCI (§A.6 p15-16
//! describes a single grammar that applies to both).
//!
//! # Source ordering
//!
//! `CanonicalAttrs::sci_markings` carries the structural representation
//! authoritatively (compartments + sub-compartments). The legacy
//! `sci_controls` (CVE projection) is rendered only as a back-compat
//! fallback when no structural markings exist — that path is removed
//! once all consumers move to `sci_markings`.

use core::fmt;

use marque_ism::{SciControlSystem, SciMarking};
use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the SCI axis to `out`. SCI is portion/banner-symmetric per
/// §A.6 p15-16; `scope` is accepted for trait conformance but does
/// not change the form.
pub(crate) fn render_sci(m: &CapcoMarking, _scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    // Prefer the structural projection when present. Only fall back to
    // the CVE projection when no structural markings exist (legacy
    // ingestion path).
    if !m.0.sci_markings.is_empty() {
        return render_structural(&m.0.sci_markings, out);
    }
    if !m.0.sci_controls.is_empty() {
        return render_cve_only(&m.0.sci_controls, out);
    }
    Ok(())
}

fn render_structural(markings: &[SciMarking], out: &mut dyn fmt::Write) -> fmt::Result {
    // Sort the systems numeric-then-alpha per §A.6 p15-16. Stable sort
    // on the system text yields a deterministic order that matches the
    // numeric-first convention because ASCII '0'..'9' < 'A'..'Z'.
    //
    // Inline-4 covers typical SCI usage (SI/TK/HCS/G as the four bare
    // control systems); inline-4 for compartments/sub-compartments
    // matches the SCI hierarchy ceiling in observed CAPCO markings.
    //
    // The three sorts below pass named `fn`-item comparators
    // (`cmp_sci_marking_system` file-local; `super::cmp_sci_compartment_ident`
    // and `super::cmp_str_numeric_then_alpha` shared) for closure-axis
    // monomorphization collapse — WASM-cut per issue #689 and the
    // PR #585 precedent at `crate::lattice::helpers::sort_smolstrs_by_sar`.
    // `cmp_sci_marking_system` stays file-local because it reaches into
    // `system_text`, which is the SCI-axis private encoding of the
    // `SciControlSystem` variants and not shared by other axes.
    let mut sorted: SmallVec<[&SciMarking; 4]> = markings.iter().collect();
    sorted.sort_by(cmp_sci_marking_system);

    let mut first = true;
    for marking in sorted {
        if !first {
            out.write_char('/')?;
        }
        first = false;
        out.write_str(system_text(&marking.system))?;
        // Compartments numeric-then-alpha within the system.
        let mut comps: SmallVec<[_; 4]> = marking.compartments.iter().collect();
        comps.sort_by(cmp_sci_compartment_ident);
        for comp in comps {
            out.write_char('-')?;
            out.write_str(&comp.identifier)?;
            // Sub-compartments numeric-then-alpha within the
            // compartment, space-separated.
            let mut subs: SmallVec<[&str; 4]> =
                comp.sub_compartments.iter().map(|s| s.as_ref()).collect();
            subs.sort_by(cmp_str_numeric_then_alpha);
            for sub in subs {
                out.write_char(' ')?;
                out.write_str(sub)?;
            }
        }
    }
    Ok(())
}

/// Compare two `&&SciMarking` references on the textual encoding of
/// their `system` field via [`super::numeric_then_alpha_cmp`].
///
/// File-local — reaches into [`system_text`], the SCI-axis encoding
/// of the `SciControlSystem` variant tag, which is not shared across
/// axes. Named `fn`-item (not closure) for closure-axis mono collapse
/// per issue #689. Mirrors the cross-file shape of the
/// `super::cmp_*_ident` family.
fn cmp_sci_marking_system(a: &&SciMarking, b: &&SciMarking) -> core::cmp::Ordering {
    super::numeric_then_alpha_cmp(system_text(&a.system), system_text(&b.system))
}

fn render_cve_only(controls: &[marque_ism::SciControl], out: &mut dyn fmt::Write) -> fmt::Result {
    let mut first = true;
    for c in controls {
        if !first {
            out.write_char('/')?;
        }
        first = false;
        out.write_str(c.as_str())?;
    }
    Ok(())
}

fn system_text(system: &SciControlSystem) -> &str {
    match system {
        SciControlSystem::Published(b) => b.as_str(),
        SciControlSystem::Custom(s) => s.as_ref(),
        // NATO SAPs (BOHEMIA, BALK) render their canonical name standalone
        // in the SCI block position. CAPCO-2016 §G.2 p40 + §H.7 p127.
        SciControlSystem::NatoSap(sap) => sap.as_str(),
    }
}

// Shared named-fn-item comparators imported from `super` — see the
// "Named-fn-item comparators" section of `crate::render::mod` for the
// mono-collapse rationale (issue #689; extends PR #585's
// `sort_smolstrs_by_sar`).
//
// `numeric_then_alpha_cmp` is accessed via the `super::` path inside
// the file-local `cmp_sci_marking_system` above (which needs it under
// a `&str` slot, not a `&&str` slot the cross-file comparator provides);
// only the comparators that ARE re-exported are listed in the `use` below.
use super::{cmp_sci_compartment_ident, cmp_str_numeric_then_alpha};
