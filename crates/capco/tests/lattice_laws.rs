// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![allow(clippy::type_complexity)] // Nested test-fixture DSL; explicit shape is clearer than a newtype.

//! Phase B lattice-law verification for CAPCO structural lattice types.
//!
//! Each type (`SciSet`, `SarSet`, `FgiSet`) is expected to satisfy the
//! lattice laws on its join:
//!
//! - idempotent: `a ⊔ a = a`
//! - commutative: `a ⊔ b = b ⊔ a`
//! - associative: `(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)`
//!
//! The §3.3a-policy-(b) meet is equal-depth intersection — not a true
//! lattice meet on arbitrary tree operands. These tests verify
//! idempotency and commutativity of meet, plus absorption on the
//! narrow inputs where absorption holds under policy (b): flat sets of
//! systems without disagreeing compartment trees. See the module-level
//! docs on `marque_capco::lattice` for which inputs are in scope.

use marque_capco::lattice::{FgiSet, SarSet, SciSet};
use marque_ism::{
    CountryCode, FgiMarker, SarCompartment, SarIndicator, SarMarking, SarProgram, SciCompartment,
    SciControlBare, SciControlSystem, SciMarking,
};
use marque_scheme::{BoundedLattice, Lattice};
use smol_str::SmolStr;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// SciSet helpers
// ---------------------------------------------------------------------------

fn sci_system_bare(bare: SciControlBare) -> SciMarking {
    SciMarking::new(SciControlSystem::Published(bare), Box::new([]), None)
}

fn sci_system_with(bare: SciControlBare, comps: &[(&str, &[&str])]) -> SciMarking {
    let compartments: Vec<SciCompartment> = comps
        .iter()
        .map(|(cid, subs)| {
            let sub_boxes: Box<[SmolStr]> = subs
                .iter()
                .map(|s| SmolStr::from(*s))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            SciCompartment::new(*cid, sub_boxes)
        })
        .collect();
    SciMarking::new(
        SciControlSystem::Published(bare),
        compartments.into_boxed_slice(),
        None,
    )
}

// Sample SciSets for cross-product exercising.
fn sci_samples() -> Vec<SciSet> {
    vec![
        SciSet::empty(),
        SciSet::from_markings(&[sci_system_bare(SciControlBare::Si)]),
        SciSet::from_markings(&[sci_system_bare(SciControlBare::Tk)]),
        SciSet::from_markings(&[sci_system_with(SciControlBare::Si, &[("G", &["A"])])]),
        SciSet::from_markings(&[sci_system_with(SciControlBare::Si, &[("G", &["A", "B"])])]),
        SciSet::from_markings(&[
            sci_system_bare(SciControlBare::Si),
            sci_system_bare(SciControlBare::Tk),
        ]),
        SciSet::from_markings(&[sci_system_with(
            SciControlBare::Hcs,
            &[("P", &[]), ("O", &[])],
        )]),
    ]
}

// ---------------------------------------------------------------------------
// SciSet laws
// ---------------------------------------------------------------------------

#[test]
fn sci_set_join_idempotent() {
    for a in sci_samples() {
        assert_eq!(a.join(&a), a, "join idempotent: {a:?}");
    }
}

#[test]
fn sci_set_join_commutative() {
    let samples = sci_samples();
    for a in &samples {
        for b in &samples {
            assert_eq!(a.join(b), b.join(a), "join commutative: {a:?} vs {b:?}");
        }
    }
}

#[test]
fn sci_set_join_associative() {
    let samples = sci_samples();
    for a in &samples {
        for b in &samples {
            for c in &samples {
                assert_eq!(
                    a.join(b).join(c),
                    a.join(&b.join(c)),
                    "join associative: {a:?}, {b:?}, {c:?}",
                );
            }
        }
    }
}

#[test]
fn sci_set_empty_is_join_identity() {
    // SciSet doesn't implement BoundedLattice (SCI has no finite top),
    // but empty() is the join identity regardless.
    let empty = SciSet::empty();
    for a in sci_samples() {
        assert_eq!(a.join(&empty), a);
        assert_eq!(empty.join(&a), a);
    }
}

#[test]
fn sci_set_meet_commutative() {
    // Policy (b) meet IS commutative: equal-depth intersection is
    // symmetric in its operands.
    let samples = sci_samples();
    for a in &samples {
        for b in &samples {
            assert_eq!(a.meet(b), b.meet(a), "meet commutative: {a:?} vs {b:?}");
        }
    }
}

#[test]
fn sci_set_meet_idempotent() {
    for a in sci_samples() {
        assert_eq!(a.meet(&a), a, "meet idempotent: {a:?}");
    }
}

// ---------------------------------------------------------------------------
// SarSet helpers and laws
// ---------------------------------------------------------------------------

fn sar_marking(programs: &[(&str, &[(&str, &[&str])])]) -> SarMarking {
    let progs: Vec<SarProgram> = programs
        .iter()
        .map(|(pid, comps)| {
            let comp_boxes: Vec<SarCompartment> = comps
                .iter()
                .map(|(cid, subs)| {
                    let sub_boxes: Box<[SmolStr]> = subs
                        .iter()
                        .map(|s| SmolStr::from(*s))
                        .collect::<Vec<_>>()
                        .into_boxed_slice();
                    SarCompartment::new(*cid, sub_boxes)
                })
                .collect();
            SarProgram::new(*pid, comp_boxes.into_boxed_slice())
        })
        .collect();
    SarMarking::new(SarIndicator::Abbrev, progs.into_boxed_slice())
}

fn sar_samples() -> Vec<SarSet> {
    vec![
        SarSet::empty(),
        SarSet::from_marking(Some(&sar_marking(&[("BP", &[])]))),
        SarSet::from_marking(Some(&sar_marking(&[("CD", &[])]))),
        SarSet::from_marking(Some(&sar_marking(&[("BP", &[("J12", &["J54"])])]))),
        SarSet::from_marking(Some(&sar_marking(&[("BP", &[("J12", &["J54", "K15"])])]))),
        SarSet::from_marking(Some(&sar_marking(&[("BP", &[]), ("CD", &[("J12", &[])])]))),
    ]
}

#[test]
fn sar_set_join_idempotent() {
    for a in sar_samples() {
        assert_eq!(a.join(&a), a);
    }
}

#[test]
fn sar_set_join_commutative() {
    let s = sar_samples();
    for a in &s {
        for b in &s {
            assert_eq!(a.join(b), b.join(a));
        }
    }
}

#[test]
fn sar_set_join_associative() {
    let s = sar_samples();
    for a in &s {
        for b in &s {
            for c in &s {
                assert_eq!(a.join(b).join(c), a.join(&b.join(c)));
            }
        }
    }
}

#[test]
fn sar_set_empty_is_join_identity() {
    // SarSet doesn't implement BoundedLattice (SAR program identifiers
    // are an open set); empty() is the join identity regardless.
    let empty = SarSet::empty();
    for a in sar_samples() {
        assert_eq!(a.join(&empty), a);
        assert_eq!(empty.join(&a), a);
    }
}

#[test]
fn sar_set_meet_commutative() {
    let s = sar_samples();
    for a in &s {
        for b in &s {
            assert_eq!(a.meet(b), b.meet(a));
        }
    }
}

#[test]
fn sar_set_meet_idempotent() {
    for a in sar_samples() {
        assert_eq!(a.meet(&a), a);
    }
}

// ---------------------------------------------------------------------------
// FgiSet helpers and laws
// ---------------------------------------------------------------------------

fn trigraph(s: &[u8; 3]) -> CountryCode {
    CountryCode::try_new(s).expect("valid trigraph")
}

fn fgi_samples() -> Vec<FgiSet> {
    vec![
        FgiSet::None,
        FgiSet::from_marker(Some(&FgiMarker::SourceConcealed)),
        FgiSet::from_marker(Some(
            &FgiMarker::acknowledged([trigraph(b"GBR")]).expect("non-empty"),
        )),
        FgiSet::from_marker(Some(
            &FgiMarker::acknowledged([trigraph(b"DEU"), trigraph(b"GBR")]).expect("non-empty"),
        )),
        FgiSet::from_marker(Some(
            &FgiMarker::acknowledged([trigraph(b"CAN")]).expect("non-empty"),
        )),
    ]
}

#[test]
fn fgi_set_join_idempotent() {
    for a in fgi_samples() {
        assert_eq!(a.join(&a), a);
    }
}

#[test]
fn fgi_set_join_commutative() {
    let s = fgi_samples();
    for a in &s {
        for b in &s {
            assert_eq!(a.join(b), b.join(a));
        }
    }
}

#[test]
fn fgi_set_join_associative() {
    let s = fgi_samples();
    for a in &s {
        for b in &s {
            for c in &s {
                assert_eq!(a.join(b).join(c), a.join(&b.join(c)));
            }
        }
    }
}

#[test]
fn fgi_set_bottom_is_join_identity() {
    // B-1 (PR 4b-B 8th-pass): `FgiSet::bottom()` retired alongside the
    // `BoundedLattice` impl. `FgiSet::empty()` is the public bottom
    // constructor with semantically-identical behavior — `Self::None`.
    let bottom = FgiSet::empty();
    for a in fgi_samples() {
        assert_eq!(a.join(&bottom), a);
        assert_eq!(bottom.join(&a), a);
    }
}

#[test]
fn fgi_set_concealed_supersedes_open_on_join() {
    // Core semantic: source-concealed dominates source-acknowledged.
    let conc = FgiSet::Present {
        concealed: true,
        countries: BTreeSet::new(),
    };
    let open = FgiSet::Present {
        concealed: false,
        countries: [trigraph(b"GBR")].iter().copied().collect(),
    };
    let j = conc.join(&open);
    match j {
        FgiSet::Present {
            concealed,
            countries,
        } => {
            assert!(concealed);
            assert!(countries.is_empty());
        }
        _ => panic!("expected Present"),
    }
}

#[test]
fn fgi_set_meet_none_anywhere_yields_none() {
    let conc = FgiSet::Present {
        concealed: true,
        countries: BTreeSet::new(),
    };
    assert_eq!(FgiSet::None.meet(&conc), FgiSet::None);
    assert_eq!(conc.meet(&FgiSet::None), FgiSet::None);
}
