// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T022 (#641 T1-4) — `MarkingScheme::constraint_rule_id` delegation.
//!
//! Pins the seam the engine's `bridge_constraint_diagnostic` delegates to:
//! a scheme maps a constraint `label` to its canonical rule-id 2-tuple
//! `(scheme, predicate_id)`. CapcoScheme uses the trait default, which
//! namespaces under `scheme_id()` (`"capco"`) and uses the label verbatim
//! as the predicate id (labels are authored in the canonical
//! `<surface>.<category>.<predicate>` form).

use marque_capco::scheme::CapcoScheme;
use marque_scheme::MarkingScheme;

#[test]
fn constraint_rule_id_namespaces_under_scheme_id() {
    let scheme = CapcoScheme::new();
    let (ns, predicate) = scheme.constraint_rule_id("dissem.noforn.conflicts-rel-to");
    assert_eq!(
        ns, "capco",
        "CapcoScheme namespaces constraint rule ids under \"capco\"",
    );
    assert_eq!(
        ns,
        scheme.scheme_id(),
        "the constraint namespace MUST equal the scheme's scheme_id()",
    );
    assert_eq!(
        predicate, "dissem.noforn.conflicts-rel-to",
        "the default projection uses the label verbatim as the predicate id",
    );
}

#[test]
fn two_label_namespaces_map_to_distinct_rule_ids() {
    let scheme = CapcoScheme::new();

    // Two distinct constraint labels in different category namespaces
    // must project to two distinct rule-id 2-tuples.
    let a = scheme.constraint_rule_id("dissem.noforn.conflicts-rel-to");
    let b = scheme.constraint_rule_id("classification.floor.below-page-floor");

    assert_ne!(
        a, b,
        "distinct constraint labels MUST map to distinct rule ids; got {a:?} and {b:?}",
    );
    // Same namespace, different predicate — the distinctness is carried
    // entirely by the predicate id.
    assert_eq!(a.0, b.0, "both labels share the CapcoScheme namespace");
    assert_ne!(a.1, b.1, "the predicate ids differ");
}

#[test]
fn same_label_is_stable() {
    // The mapping is a pure projection: the same label always yields the
    // same rule id (the bridge relies on this for config-override and
    // audit-record addressing stability).
    let scheme = CapcoScheme::new();
    let first = scheme.constraint_rule_id("dissem.relido.requires-unanimity");
    let second = scheme.constraint_rule_id("dissem.relido.requires-unanimity");
    assert_eq!(first, second);
}
