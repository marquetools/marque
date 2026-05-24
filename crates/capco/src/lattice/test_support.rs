// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared test fixtures for the `lattice` submodules.
//!
//! Visibility scoped to the crate via `pub(crate)` so each submodule's
//! `#[cfg(test)] mod tests` can `use crate::lattice::test_support::*;`.
//! The fixtures are simple `CanonicalAttrs` / `SciMarking` / `SarMarking`
//! constructors that compose attributes a single test cares about; they
//! are not generic test infrastructure.

#![allow(dead_code)] // fixtures are conditionally used across submodules

use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification,
    SarCompartment, SarIndicator, SarMarking, SarProgram, SciCompartment, SciControlSystem,
    SciMarking,
};
use smol_str::SmolStr;

pub(crate) fn portion_us(level: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(level));
    a
}

pub(crate) fn portion_with_rel_to(level: Classification, rel: &[&str]) -> CanonicalAttrs {
    let mut a = portion_us(level);
    a.rel_to = rel
        .iter()
        .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    a
}

pub(crate) fn portion_with_display_only(level: Classification, display: &[&str]) -> CanonicalAttrs {
    let mut a = portion_us(level);
    a.display_only_to = display
        .iter()
        .map(|s| CountryCode::try_new(s.as_bytes()).unwrap())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    a
}

pub(crate) fn portion_with_dissem_us(
    level: Classification,
    dissem: &[DissemControl],
) -> CanonicalAttrs {
    let mut a = portion_us(level);
    a.dissem_us = dissem.to_vec().into_boxed_slice();
    a
}

pub(crate) fn mk_sci(system: SciControlSystem, comps: Vec<(&str, Vec<&str>)>) -> SciMarking {
    let compartments: Vec<SciCompartment> = comps
        .into_iter()
        .map(|(cid, subs)| {
            let sub_boxes: Box<[SmolStr]> = subs
                .into_iter()
                .map(SmolStr::from)
                .collect::<Vec<_>>()
                .into_boxed_slice();
            SciCompartment::new(cid, sub_boxes)
        })
        .collect();
    SciMarking::new(system, compartments.into_boxed_slice(), None)
}

#[allow(clippy::type_complexity)] // Test-fixture DSL; explicit shape is clearer than a newtype.
pub(crate) fn mk_sar_portion(programs: Vec<(&str, Vec<(&str, Vec<&str>)>)>) -> SarMarking {
    let built: Vec<SarProgram> = programs
        .into_iter()
        .map(|(pid, comps)| {
            let comp_boxes: Vec<SarCompartment> = comps
                .into_iter()
                .map(|(cid, subs)| {
                    let sub_boxes: Box<[SmolStr]> = subs
                        .into_iter()
                        .map(SmolStr::from)
                        .collect::<Vec<_>>()
                        .into_boxed_slice();
                    SarCompartment::new(cid, sub_boxes)
                })
                .collect();
            SarProgram::new(pid, comp_boxes.into_boxed_slice())
        })
        .collect();
    SarMarking::new(SarIndicator::Abbrev, built.into_boxed_slice())
}
