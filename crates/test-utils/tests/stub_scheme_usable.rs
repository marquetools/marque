// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Smoke test: the shared `stub_scheme` fixture is usable from a
//! consumer crate's test, against the generic `marque_scheme` trait
//! surface, without reaching into any engine-side crate.
//!
//! This is the Phase B PR-B1 payload: a *second* `MarkingScheme` that
//! later generic-surface PRs (a generic `Engine<S>`, `RuleContext<S>`)
//! instantiate alongside `CapcoScheme` so generic code is provably
//! exercised against more than one scheme. The compile of this file in
//! a downstream crate IS the assertion that the stub is reusable.

use marque_scheme::lattice::{BoundedJoinSemilattice, BoundedMeetSemilattice};
use marque_scheme::scheme::MarkingScheme;
use marque_scheme::scope::Scope;
use marque_test_utils::stub_scheme::{STUB_TOKEN, StubMarking, StubScheme};

/// The shared stub satisfies the `MarkingScheme` bound and round-trips
/// its render projections — enough to confirm a consumer can drive it
/// generically.
#[test]
fn shared_stub_drives_generic_marking_scheme_surface() {
    fn name_of<S: MarkingScheme>(scheme: &S) -> &str {
        scheme.name()
    }

    let scheme = StubScheme::new();
    assert_eq!(name_of(&scheme), "stub");
    // StubScheme overrides scheme_id() to a genuine second namespace
    // ("stub"), not the unoverridden default "scheme".
    assert_eq!(scheme.scheme_id(), "stub");

    let top = StubMarking::top();
    let bottom = StubMarking::bottom();
    assert_eq!(scheme.render_item(&top), "(STUB)");
    assert_eq!(scheme.render_summary(&top), "STUB");
    assert_eq!(scheme.render_summary(&bottom), "");

    // project is join-fold over the lattice: top ∨ bottom = top.
    let projected = scheme.project(Scope::Page, &[top, bottom]);
    assert!(projected.has_token);

    // The default constraint_rule_id projection namespaces under
    // scheme_id() (overridden to "stub") and uses the label verbatim.
    let (ns, predicate) = scheme.constraint_rule_id("stub.cat.predicate");
    assert_eq!(ns, "stub");
    assert_eq!(predicate, "stub.cat.predicate");

    // satisfies resolves the sentinel token.
    use marque_scheme::constraint::TokenRef;
    assert!(scheme.satisfies(
        &StubMarking { has_token: true },
        &TokenRef::Token(STUB_TOKEN)
    ));
}
