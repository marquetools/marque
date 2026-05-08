// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3b.C (T026c) — authoring-contract and behavior tests for E054–E057.
//!
//! Each of the four new RELIDO incompatibility `Constraint::Conflicts` rows
//! in `CapcoScheme::build_constraints()` is covered by:
//!
//! 1. **Authoring-contract test** — asserts the catalog row is present,
//!    is a `Constraint::Conflicts` variant, has the expected `left`/`right`
//!    `TokenRef::Token(...)` values, and carries the exact §-citation string
//!    specified in the implementation plan (D13 single-citation discipline).
//!
//! 2. **Behavior tests** — three sub-cases each:
//!    - Both conflicting tokens present → one `Diagnostic` emitted with the
//!      expected `RuleId` and `citation`.
//!    - Only one token present → silent.
//!    - Neither token present → silent.
//!
//! 3. **Citation-fidelity test** — walks all four wrappers' emitted
//!    `Diagnostic.citation` strings and asserts byte-identity with the
//!    corresponding catalog `label`. Guards against drift between the two
//!    storage sites.
//!
//! 4. **Constraint-shape pin** — asserts all four entries are
//!    `Constraint::Conflicts`, not `Custom`. Guards against a future PR
//!    converting them and bypassing the generic dyadic-evaluation path.
//!
//! 5. **Count pin** — asserts `CapcoScheme::constraints().len()` equals
//!    the pre-3b.C baseline plus exactly 4. A failure here means catalog
//!    drift the reviewer should examine.
//!
//! Naming conventions follow `crates/capco/tests/transmutation_rewrites.rs`
//! (the PR 3b.B structural template).

use marque_capco::CapcoScheme;
use marque_capco::rules::{
    DeclarativeOrconRelidoConflictRule, DeclarativeOrconUsgovRelidoConflictRule,
    DeclarativeRelidoDisplayOnlyConflictRule, DeclarativeRelidoNofornConflictRule,
};
use marque_capco::scheme::{TOK_DISPLAY_ONLY, TOK_NOFORN, TOK_ORCON, TOK_ORCON_USGOV, TOK_RELIDO};
use marque_ism::{
    CanonicalAttrs, Classification, DissemControl, MarkingClassification, MarkingType, Span,
    TokenKind, TokenSpan,
};
use marque_rules::{Rule, RuleContext, Severity};
use marque_scheme::{Constraint, MarkingScheme, TokenRef};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Look up a `Constraint` by its stable name. Panics with a clear diagnostic
/// message naming both the missing entry and every declared name, so a failed
/// lookup immediately tells the reviewer whether the test expects a typo or
/// the constraint truly wasn't registered.
fn lookup_constraint<'a>(scheme: &'a CapcoScheme, name: &str) -> &'a Constraint {
    scheme
        .constraints()
        .iter()
        .find(|c| c.name() == name)
        .unwrap_or_else(|| {
            let declared: Vec<&str> = scheme.constraints().iter().map(|c| c.name()).collect();
            panic!("constraint {name:?} is not declared on CapcoScheme; declared: {declared:?}")
        })
}

/// Minimal `CanonicalAttrs` with a US Secret classification and the given
/// dissem controls. No token_spans attached — used for constraint-trigger
/// tests that go through `CapcoScheme::evaluate_named_constraint` only.
fn attrs_with_dissem(controls: &[DissemControl]) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_controls = controls.to_vec().into_boxed_slice();
    a
}

/// `CanonicalAttrs` with dissem controls AND a synthetic `TokenSpan` slice
/// so wrapper span-selection logic is exercised. Text strings mirror the
/// CVE abbreviation form from `DissemControl::as_str()` in generated
/// values.rs: `"RELIDO"`, `"DISPLAYONLY"`, `"OC"`, `"OC-USGOV"`, `"NOFORN"`.
fn attrs_with_dissem_and_spans(controls: &[(DissemControl, &str)]) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_controls = controls
        .iter()
        .map(|(d, _)| *d)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    // Build synthetic token_spans with sequential byte offsets so each
    // span is distinguishable. Span::new takes usize.
    let mut offset: usize = 10;
    a.token_spans = controls
        .iter()
        .map(|(_, text)| {
            let len = text.len();
            let span = Span::new(offset, offset + len);
            offset += len + 1; // +1 for a notional space between tokens
            TokenSpan {
                kind: TokenKind::DissemControl,
                text: text.to_string().into_boxed_str(),
                span,
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    a
}

/// Minimal `RuleContext` for wrapper `check()` calls.
/// Mirrors the construction pattern from `crates/capco/tests/rules_us1.rs`.
fn ctx() -> RuleContext {
    RuleContext {
        marking_type: MarkingType::Portion,
        zone: None,
        position: None,
        page_context: None,
        corrections: None,
    }
}

// ---------------------------------------------------------------------------
// Authoring-contract tests (one per entry × four entries)
// ---------------------------------------------------------------------------

#[test]
fn e054_relido_noforn_constraint_is_correctly_authored() {
    let scheme = CapcoScheme::new();
    let c = lookup_constraint(&scheme, "E054/relido-conflicts-noforn");

    // Variant
    assert!(
        matches!(c, Constraint::Conflicts { .. }),
        "E054/relido-conflicts-noforn must be Constraint::Conflicts, got: {c:?}"
    );

    // LHS / RHS token refs — per PM Q2: LHS = asserting token (RELIDO at p154).
    assert!(
        matches!(c, Constraint::Conflicts { left: TokenRef::Token(id), .. } if *id == TOK_RELIDO),
        "E054 left must be TOK_RELIDO ({:?}), got: {c:?}",
        TOK_RELIDO
    );
    assert!(
        matches!(c, Constraint::Conflicts { right: TokenRef::Token(id), .. } if *id == TOK_NOFORN),
        "E054 right must be TOK_NOFORN ({:?}), got: {c:?}",
        TOK_NOFORN
    );

    // Citation — D13 single-citation discipline (CAPCO-2016.md line 3808).
    assert_eq!(
        c.label(),
        "CAPCO-2016 §H.8 p154",
        "E054/relido-conflicts-noforn catalog label drifted from plan §3"
    );
}

#[test]
fn e055_relido_display_only_constraint_is_correctly_authored() {
    let scheme = CapcoScheme::new();
    let c = lookup_constraint(&scheme, "E055/relido-conflicts-display-only");

    assert!(
        matches!(c, Constraint::Conflicts { .. }),
        "E055/relido-conflicts-display-only must be Constraint::Conflicts, got: {c:?}"
    );

    assert!(
        matches!(c, Constraint::Conflicts { left: TokenRef::Token(id), .. } if *id == TOK_RELIDO),
        "E055 left must be TOK_RELIDO ({:?}), got: {c:?}",
        TOK_RELIDO
    );
    assert!(
        matches!(c, Constraint::Conflicts { right: TokenRef::Token(id), .. } if *id == TOK_DISPLAY_ONLY),
        "E055 right must be TOK_DISPLAY_ONLY ({:?}), got: {c:?}",
        TOK_DISPLAY_ONLY
    );

    assert_eq!(
        c.label(),
        "CAPCO-2016 §H.8 p154",
        "E055/relido-conflicts-display-only catalog label drifted from plan §3"
    );
}

#[test]
fn e056_orcon_relido_constraint_is_correctly_authored() {
    let scheme = CapcoScheme::new();
    let c = lookup_constraint(&scheme, "E056/orcon-conflicts-relido");

    assert!(
        matches!(c, Constraint::Conflicts { .. }),
        "E056/orcon-conflicts-relido must be Constraint::Conflicts, got: {c:?}"
    );

    // LHS = ORCON (asserting token at p136 per PM Q2).
    assert!(
        matches!(c, Constraint::Conflicts { left: TokenRef::Token(id), .. } if *id == TOK_ORCON),
        "E056 left must be TOK_ORCON ({:?}), got: {c:?}",
        TOK_ORCON
    );
    assert!(
        matches!(c, Constraint::Conflicts { right: TokenRef::Token(id), .. } if *id == TOK_RELIDO),
        "E056 right must be TOK_RELIDO ({:?}), got: {c:?}",
        TOK_RELIDO
    );

    assert_eq!(
        c.label(),
        "CAPCO-2016 §H.8 p136",
        "E056/orcon-conflicts-relido catalog label drifted from plan §3"
    );
}

#[test]
fn e057_orcon_usgov_relido_constraint_is_correctly_authored() {
    let scheme = CapcoScheme::new();
    let c = lookup_constraint(&scheme, "E057/orcon-usgov-conflicts-relido");

    assert!(
        matches!(c, Constraint::Conflicts { .. }),
        "E057/orcon-usgov-conflicts-relido must be Constraint::Conflicts, got: {c:?}"
    );

    // LHS = ORCON-USGOV (asserting token at p140 per PM Q2).
    assert!(
        matches!(c, Constraint::Conflicts { left: TokenRef::Token(id), .. } if *id == TOK_ORCON_USGOV),
        "E057 left must be TOK_ORCON_USGOV ({:?}), got: {c:?}",
        TOK_ORCON_USGOV
    );
    assert!(
        matches!(c, Constraint::Conflicts { right: TokenRef::Token(id), .. } if *id == TOK_RELIDO),
        "E057 right must be TOK_RELIDO ({:?}), got: {c:?}",
        TOK_RELIDO
    );

    assert_eq!(
        c.label(),
        "CAPCO-2016 §H.8 p140",
        "E057/orcon-usgov-conflicts-relido catalog label drifted from plan §3"
    );
}

// ---------------------------------------------------------------------------
// Behavior tests — E054 (RELIDO ⊥ NOFORN)
// ---------------------------------------------------------------------------

#[test]
fn e054_fires_when_both_relido_and_noforn_present() {
    let attrs = attrs_with_dissem_and_spans(&[
        (DissemControl::Relido, "RELIDO"),
        (DissemControl::Nf, "NOFORN"),
    ]);
    let rule = DeclarativeRelidoNofornConflictRule;
    let diags = rule.check(&attrs, &ctx());

    assert_eq!(
        diags.len(),
        1,
        "expected exactly one diagnostic; got: {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.rule.as_str(), "E054");
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.citation, "CAPCO-2016 §H.8 p154");
    // Span must point at RELIDO (offset 10, len 6 → 10..16).
    assert_eq!(d.span.start, 10, "E054 span should anchor at RELIDO token");
    assert!(d.fix.is_none(), "E054 must not emit a FixProposal");
}

#[test]
fn e054_silent_when_only_relido_present() {
    let attrs = attrs_with_dissem(&[DissemControl::Relido]);
    let rule = DeclarativeRelidoNofornConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E054 must be silent when only RELIDO is present"
    );
}

#[test]
fn e054_silent_when_neither_relido_nor_noforn_present() {
    let attrs = attrs_with_dissem(&[DissemControl::Oc]);
    let rule = DeclarativeRelidoNofornConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E054 must be silent when neither RELIDO nor NOFORN is present"
    );
}

// ---------------------------------------------------------------------------
// Behavior tests — E055 (RELIDO ⊥ DISPLAY ONLY)
// ---------------------------------------------------------------------------

#[test]
fn e055_fires_when_both_relido_and_display_only_present() {
    let attrs = attrs_with_dissem_and_spans(&[
        (DissemControl::Relido, "RELIDO"),
        (DissemControl::Displayonly, "DISPLAYONLY"),
    ]);
    let rule = DeclarativeRelidoDisplayOnlyConflictRule;
    let diags = rule.check(&attrs, &ctx());

    assert_eq!(
        diags.len(),
        1,
        "expected exactly one diagnostic; got: {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.rule.as_str(), "E055");
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.citation, "CAPCO-2016 §H.8 p154");
    // Span must point at RELIDO (offset 10, len 6 → 10..16).
    assert_eq!(d.span.start, 10, "E055 span should anchor at RELIDO token");
    assert!(d.fix.is_none(), "E055 must not emit a FixProposal");
}

#[test]
fn e055_silent_when_only_display_only_present() {
    let attrs = attrs_with_dissem(&[DissemControl::Displayonly]);
    let rule = DeclarativeRelidoDisplayOnlyConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E055 must be silent when only DISPLAY ONLY is present"
    );
}

#[test]
fn e055_silent_when_neither_relido_nor_display_only_present() {
    let attrs = attrs_with_dissem(&[DissemControl::Nf]);
    let rule = DeclarativeRelidoDisplayOnlyConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E055 must be silent when neither RELIDO nor DISPLAY ONLY is present"
    );
}

// ---------------------------------------------------------------------------
// Behavior tests — E056 (ORCON ⊥ RELIDO)
// ---------------------------------------------------------------------------

#[test]
fn e056_fires_when_both_orcon_and_relido_present() {
    let attrs = attrs_with_dissem_and_spans(&[
        (DissemControl::Oc, "OC"),
        (DissemControl::Relido, "RELIDO"),
    ]);
    let rule = DeclarativeOrconRelidoConflictRule;
    let diags = rule.check(&attrs, &ctx());

    assert_eq!(
        diags.len(),
        1,
        "expected exactly one diagnostic; got: {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.rule.as_str(), "E056");
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.citation, "CAPCO-2016 §H.8 p136");
    // Span must point at OC/ORCON (offset 10, len 2 → 10..12) — asserting
    // token is ORCON per PM Q1: anchor at the token whose template says
    // "May not be used with RELIDO."
    assert_eq!(
        d.span.start, 10,
        "E056 span should anchor at OC (ORCON) token"
    );
    assert!(d.fix.is_none(), "E056 must not emit a FixProposal");
}

#[test]
fn e056_silent_when_only_orcon_present() {
    let attrs = attrs_with_dissem(&[DissemControl::Oc]);
    let rule = DeclarativeOrconRelidoConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E056 must be silent when only ORCON is present"
    );
}

#[test]
fn e056_silent_when_neither_orcon_nor_relido_present() {
    let attrs = attrs_with_dissem(&[DissemControl::Nf]);
    let rule = DeclarativeOrconRelidoConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E056 must be silent when neither ORCON nor RELIDO is present"
    );
}

// ---------------------------------------------------------------------------
// Behavior tests — E057 (ORCON-USGOV ⊥ RELIDO)
// ---------------------------------------------------------------------------

#[test]
fn e057_fires_when_both_orcon_usgov_and_relido_present() {
    let attrs = attrs_with_dissem_and_spans(&[
        (DissemControl::OcUsgov, "OC-USGOV"),
        (DissemControl::Relido, "RELIDO"),
    ]);
    let rule = DeclarativeOrconUsgovRelidoConflictRule;
    let diags = rule.check(&attrs, &ctx());

    assert_eq!(
        diags.len(),
        1,
        "expected exactly one diagnostic; got: {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.rule.as_str(), "E057");
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.citation, "CAPCO-2016 §H.8 p140");
    // Span must point at OC-USGOV (offset 10, len 8 → 10..18) — asserting
    // token is ORCON-USGOV per PM Q1: anchor at the token whose template says
    // "May not be used with RELIDO."
    assert_eq!(
        d.span.start, 10,
        "E057 span should anchor at OC-USGOV (ORCON-USGOV) token"
    );
    assert!(d.fix.is_none(), "E057 must not emit a FixProposal");
}

#[test]
fn e057_silent_when_only_orcon_usgov_present() {
    let attrs = attrs_with_dissem(&[DissemControl::OcUsgov]);
    let rule = DeclarativeOrconUsgovRelidoConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E057 must be silent when only ORCON-USGOV is present"
    );
}

#[test]
fn e057_silent_when_neither_orcon_usgov_nor_relido_present() {
    let attrs = attrs_with_dissem(&[DissemControl::Nf]);
    let rule = DeclarativeOrconUsgovRelidoConflictRule;
    assert!(
        rule.check(&attrs, &ctx()).is_empty(),
        "E057 must be silent when neither ORCON-USGOV nor RELIDO is present"
    );
}

// ---------------------------------------------------------------------------
// Citation-fidelity test
// ---------------------------------------------------------------------------
//
// Asserts byte-identity between each catalog row's `label` and the
// corresponding wrapper's emitted `Diagnostic.citation`. This is the
// regression guard against drift: if a future edit changes the citation in
// one place but not the other, this test fires immediately.

#[test]
fn relido_conflict_wrappers_carry_catalog_citations() {
    let scheme = CapcoScheme::new();
    let cases: &[(&str, &dyn Rule, &str)] = &[
        (
            "E054/relido-conflicts-noforn",
            &DeclarativeRelidoNofornConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p154",
        ),
        (
            "E055/relido-conflicts-display-only",
            &DeclarativeRelidoDisplayOnlyConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p154",
        ),
        (
            "E056/orcon-conflicts-relido",
            &DeclarativeOrconRelidoConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p136",
        ),
        (
            "E057/orcon-usgov-conflicts-relido",
            &DeclarativeOrconUsgovRelidoConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p140",
        ),
    ];

    for (constraint_name, rule, expected_citation) in cases {
        // Catalog side: the `label` must equal the expected citation.
        let c = lookup_constraint(&scheme, constraint_name);
        assert_eq!(
            c.label(),
            *expected_citation,
            "catalog label for {constraint_name} drifted from plan §3"
        );

        // Wrapper emission side: build a minimal triggering attrs and confirm
        // Diagnostic.citation == catalog.label. For E054/E055 both tokens are
        // RELIDO-side; for E056/E057 the LHS token is ORCON/ORCON-USGOV.
        let trigger_attrs = match *constraint_name {
            "E054/relido-conflicts-noforn" => {
                attrs_with_dissem(&[DissemControl::Relido, DissemControl::Nf])
            }
            "E055/relido-conflicts-display-only" => {
                attrs_with_dissem(&[DissemControl::Relido, DissemControl::Displayonly])
            }
            "E056/orcon-conflicts-relido" => {
                attrs_with_dissem(&[DissemControl::Oc, DissemControl::Relido])
            }
            "E057/orcon-usgov-conflicts-relido" => {
                attrs_with_dissem(&[DissemControl::OcUsgov, DissemControl::Relido])
            }
            other => panic!("unhandled constraint name in fidelity test: {other}"),
        };
        let diags = rule.check(&trigger_attrs, &ctx());
        assert_eq!(
            diags.len(),
            1,
            "expected exactly one diagnostic from {constraint_name} on triggering attrs"
        );
        assert_eq!(
            diags[0].citation, *expected_citation,
            "Diagnostic.citation from {constraint_name} wrapper drifted from catalog label"
        );
    }
}

// ---------------------------------------------------------------------------
// Constraint-shape pin
// ---------------------------------------------------------------------------
//
// Asserts all four new entries are `Constraint::Conflicts`, NOT `Custom`.
// Guards against a future PR converting them and bypassing the generic
// dyadic-evaluation path in `CapcoScheme::evaluate_named_constraint`.

#[test]
fn relido_conflict_rows_are_dyadic_conflicts_variant() {
    let scheme = CapcoScheme::new();
    for name in [
        "E054/relido-conflicts-noforn",
        "E055/relido-conflicts-display-only",
        "E056/orcon-conflicts-relido",
        "E057/orcon-usgov-conflicts-relido",
    ] {
        let c = lookup_constraint(&scheme, name);
        assert!(
            matches!(c, Constraint::Conflicts { .. }),
            "constraint {name} must be Constraint::Conflicts, got: {c:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Constraint count pin
// ---------------------------------------------------------------------------
//
// Pins the absolute count of `CapcoScheme::constraints()` after PR 3b.C.
// A failure here means a constraint was added or removed without an
// accompanying intentional documentation update. The pre-3b.C count (15)
// was verified by inspecting `build_constraints()` before this PR added
// any rows. PR 3b.C adds exactly 4 (E054–E057), giving 19.
//
// Bump this number only when an intentional catalog change lands; the
// change must be documented in `specs/006-engine-rule-refactor/decisions.md`.

#[test]
fn capco_constraints_count_after_pr3b_c() {
    let scheme = CapcoScheme::new();
    // Pre-3b.C baseline: 15 constraints (verified by inspection of
    // `build_constraints()` on the `origin/staging` base at 13fdc085).
    // PR 3b.C adds exactly 4 (E054 / E055 / E056 / E057).
    assert_eq!(
        scheme.constraints().len(),
        19,
        "expected 15 (pre-3b.C) + 4 (E054–E057) = 19 constraints after PR 3b.C; \
         if this fails, either a constraint was added/removed without updating this \
         pin, or the baseline count drifted. Verify in decisions.md D14."
    );
}
