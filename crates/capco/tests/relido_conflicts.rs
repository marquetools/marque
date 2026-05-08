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
    compute_relido_removal_span,
};
use marque_capco::scheme::{TOK_DISPLAY_ONLY, TOK_NOFORN, TOK_ORCON, TOK_ORCON_USGOV, TOK_RELIDO};
use marque_ism::{
    CanonicalAttrs, Classification, DissemControl, MarkingClassification, MarkingType, Span,
    TokenKind, TokenSpan,
};
use marque_rules::{FixSource, Rule, RuleContext, Severity};
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

/// Build `CanonicalAttrs` whose `token_spans` reflect the byte layout of
/// `source` for a banner / portion shaped `(<head>//<dissem-block>)`. The
/// dissem block is a `/`-separated list of tokens: `RELIDO`, `NOFORN`,
/// `DISPLAYONLY`, `OC`, `OC-USGOV`, `NF` (the CVE abbreviation form per
/// `DissemControl::as_str()` in generated values.rs).
///
/// `head` is a pseudo-classification token (e.g., `"S"` for SECRET) that
/// occupies bytes `[0, head.len()]` followed by a `//` category separator
/// at bytes `[head.len(), head.len() + 2]`. Each dissem token is emitted
/// as a `TokenKind::DissemControl` `TokenSpan` with adjacent (`prev.end + 1
/// == curr.start`) byte offsets — exactly mirroring the parser's actual
/// output for a dissem block (see `crates/core/src/parser.rs:1422`).
///
/// Returns `(attrs, source_bytes)` so behavior tests can apply a
/// `FixProposal` to `source_bytes` and assert the post-fix substring.
fn attrs_for_dissem_block(
    head: &str,
    dissem: &[(DissemControl, &str)],
) -> (CanonicalAttrs, String) {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_controls = dissem
        .iter()
        .map(|(d, _)| *d)
        .collect::<Vec<_>>()
        .into_boxed_slice();

    // Construct the source string and walk byte offsets in lock-step.
    let mut source = String::new();
    source.push_str(head); // head occupies [0, head.len()]
    source.push_str("//"); // category boundary at [head.len(), head.len() + 2]

    let mut spans: Vec<TokenSpan> = Vec::with_capacity(dissem.len());
    for (i, (_, text)) in dissem.iter().enumerate() {
        if i > 0 {
            source.push('/'); // intra-block separator
        }
        let start = source.len();
        source.push_str(text);
        let end = source.len();
        spans.push(TokenSpan {
            kind: TokenKind::DissemControl,
            text: (*text).to_string().into_boxed_str(),
            span: Span::new(start, end),
        });
    }
    a.token_spans = spans.into_boxed_slice();
    (a, source)
}

/// Convenience: build `CanonicalAttrs` whose `token_spans` mirror a
/// `S//<dissem block>` layout but discard the source string. Used in tests
/// that don't need to apply the fix to bytes (e.g., the trigger / silence
/// behavior tests, the citation-fidelity test).
fn attrs_with_dissem_and_spans(dissem: &[(DissemControl, &str)]) -> CanonicalAttrs {
    let (attrs, _src) = attrs_for_dissem_block("S", dissem);
    attrs
}

/// Apply a span-and-replacement to source bytes, mirroring
/// `Engine::fix`'s in-place substitution. Test-only.
fn apply_fix(source: &str, span: Span, replacement: &str) -> String {
    let mut out = String::with_capacity(source.len() + replacement.len());
    out.push_str(&source[..span.start]);
    out.push_str(replacement);
    out.push_str(&source[span.end..]);
    out
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
    // Source "(S//RELIDO/NOFORN)" — RELIDO is the FIRST dissem token,
    // immediately after `//`. The fix must consume the trailing `/` so
    // the post-fix bytes are "S//NOFORN".
    let (attrs, source) = attrs_for_dissem_block(
        "S",
        &[
            (DissemControl::Relido, "RELIDO"),
            (DissemControl::Nf, "NOFORN"),
        ],
    );
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
    assert_eq!(
        d.message.as_ref(),
        "RELIDO removed: cannot be used with NOFORN (§H.8 p154)"
    );
    // Diagnostic span anchors at RELIDO (the user's cursor lands here).
    // RELIDO starts at byte 3 (after "S//"); spans `[3, 9]`.
    assert_eq!(d.span.start, 3, "E054 anchor span should point at RELIDO");
    assert_eq!(d.span.end, 9);

    // Subtractive FixProposal: span covers RELIDO + trailing `/` → [3, 10].
    let fix = d.fix.as_ref().expect("E054 must emit a FixProposal");
    assert_eq!(fix.span.start, 3, "fix span should start at RELIDO");
    assert_eq!(fix.span.end, 10, "fix span should consume trailing `/`");
    assert_eq!(fix.replacement.as_ref(), "");
    assert_eq!(fix.source, FixSource::BuiltinRule);
    assert_eq!(fix.confidence.rule, 0.9);
    assert!(fix.migration_ref.is_none());

    // Apply the fix and verify the result is well-formed: NOFORN remains,
    // RELIDO is gone, no `//` adjacent to a `/`, no leading or trailing `/`
    // in the dissem block.
    let fixed = apply_fix(&source, fix.span, &fix.replacement);
    assert_eq!(fixed, "S//NOFORN");
    assert!(!fixed.contains("RELIDO"));
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
    // Source "S//RELIDO/DISPLAYONLY" — RELIDO is FIRST in the dissem
    // block. The fix consumes the trailing `/`.
    let (attrs, source) = attrs_for_dissem_block(
        "S",
        &[
            (DissemControl::Relido, "RELIDO"),
            (DissemControl::Displayonly, "DISPLAYONLY"),
        ],
    );
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
    assert_eq!(
        d.message.as_ref(),
        "RELIDO removed: cannot be used with DISPLAY ONLY (§H.8 p154)"
    );
    // Anchor span points at RELIDO ([3, 9]).
    assert_eq!(d.span.start, 3, "E055 anchor span should point at RELIDO");
    assert_eq!(d.span.end, 9);

    let fix = d.fix.as_ref().expect("E055 must emit a FixProposal");
    assert_eq!(fix.span.start, 3);
    assert_eq!(fix.span.end, 10, "fix span should consume trailing `/`");
    assert_eq!(fix.replacement.as_ref(), "");
    assert_eq!(fix.source, FixSource::BuiltinRule);
    assert_eq!(fix.confidence.rule, 0.9);

    let fixed = apply_fix(&source, fix.span, &fix.replacement);
    assert_eq!(fixed, "S//DISPLAYONLY");
    assert!(!fixed.contains("RELIDO"));
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
    // Source "S//OC/RELIDO" — RELIDO is the LAST dissem token, preceded
    // by `/`. The fix consumes the preceding `/`.
    let (attrs, source) = attrs_for_dissem_block(
        "S",
        &[(DissemControl::Oc, "OC"), (DissemControl::Relido, "RELIDO")],
    );
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
    assert_eq!(
        d.message.as_ref(),
        "RELIDO removed: ORCON may not be used with RELIDO (§H.8 p136)"
    );
    // Anchor span points at OC (ORCON), the asserting-template token.
    // OC starts at byte 3 (after "S//"); spans `[3, 5]`.
    assert_eq!(
        d.span.start, 3,
        "E056 anchor span should point at OC (ORCON)"
    );
    assert_eq!(d.span.end, 5);

    // Subtractive FixProposal: span covers preceding `/` + RELIDO →
    // RELIDO is at `[6, 12]`; the `/` separator is at byte 5. Fix span
    // is `[5, 12]`.
    let fix = d.fix.as_ref().expect("E056 must emit a FixProposal");
    assert_eq!(fix.span.start, 5, "fix span should consume preceding `/`");
    assert_eq!(fix.span.end, 12, "fix span should end after RELIDO");
    assert_eq!(fix.replacement.as_ref(), "");
    assert_eq!(fix.source, FixSource::BuiltinRule);
    assert_eq!(fix.confidence.rule, 0.9);

    let fixed = apply_fix(&source, fix.span, &fix.replacement);
    assert_eq!(fixed, "S//OC");
    assert!(!fixed.contains("RELIDO"));
    assert!(!fixed.ends_with('/'), "no trailing `/` after fix");
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
    // Source "S//OC-USGOV/RELIDO" — RELIDO is LAST in the dissem block,
    // preceded by `/`. The fix consumes the preceding `/`.
    let (attrs, source) = attrs_for_dissem_block(
        "S",
        &[
            (DissemControl::OcUsgov, "OC-USGOV"),
            (DissemControl::Relido, "RELIDO"),
        ],
    );
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
    assert_eq!(
        d.message.as_ref(),
        "RELIDO removed: ORCON-USGOV may not be used with RELIDO (§H.8 p140)"
    );
    // Anchor span points at OC-USGOV ([3, 11]).
    assert_eq!(
        d.span.start, 3,
        "E057 anchor span should point at OC-USGOV (ORCON-USGOV)"
    );
    assert_eq!(d.span.end, 11);

    // Subtractive FixProposal: RELIDO is at `[12, 18]`; the `/` separator
    // is at byte 11. Fix span is `[11, 18]`.
    let fix = d.fix.as_ref().expect("E057 must emit a FixProposal");
    assert_eq!(fix.span.start, 11, "fix span should consume preceding `/`");
    assert_eq!(fix.span.end, 18, "fix span should end after RELIDO");
    assert_eq!(fix.replacement.as_ref(), "");
    assert_eq!(fix.source, FixSource::BuiltinRule);
    assert_eq!(fix.confidence.rule, 0.9);

    let fixed = apply_fix(&source, fix.span, &fix.replacement);
    assert_eq!(fixed, "S//OC-USGOV");
    assert!(!fixed.contains("RELIDO"));
    assert!(!fixed.ends_with('/'), "no trailing `/` after fix");
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
    let cases: &[(&str, &dyn Rule, &str, &[(DissemControl, &str)])] = &[
        (
            "E054/relido-conflicts-noforn",
            &DeclarativeRelidoNofornConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p154",
            &[
                (DissemControl::Relido, "RELIDO"),
                (DissemControl::Nf, "NOFORN"),
            ],
        ),
        (
            "E055/relido-conflicts-display-only",
            &DeclarativeRelidoDisplayOnlyConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p154",
            &[
                (DissemControl::Relido, "RELIDO"),
                (DissemControl::Displayonly, "DISPLAYONLY"),
            ],
        ),
        (
            "E056/orcon-conflicts-relido",
            &DeclarativeOrconRelidoConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p136",
            &[(DissemControl::Oc, "OC"), (DissemControl::Relido, "RELIDO")],
        ),
        (
            "E057/orcon-usgov-conflicts-relido",
            &DeclarativeOrconUsgovRelidoConflictRule as &dyn Rule,
            "CAPCO-2016 §H.8 p140",
            &[
                (DissemControl::OcUsgov, "OC-USGOV"),
                (DissemControl::Relido, "RELIDO"),
            ],
        ),
    ];

    for (constraint_name, rule, expected_citation, dissem) in cases {
        // Catalog side: the `label` must equal the expected citation.
        let c = lookup_constraint(&scheme, constraint_name);
        assert_eq!(
            c.label(),
            *expected_citation,
            "catalog label for {constraint_name} drifted from plan §3"
        );

        // Wrapper emission side: build triggering attrs WITH spans (so the
        // FixProposal helper can compute a removal layout) and confirm
        // Diagnostic.citation == catalog.label, plus FixProposal exists
        // with confidence 0.9 (PM Addendum II §3, §8).
        let trigger_attrs = attrs_with_dissem_and_spans(dissem);
        let diags = rule.check(&trigger_attrs, &ctx());
        assert_eq!(
            diags.len(),
            1,
            "expected exactly one diagnostic from {constraint_name} on triggering attrs"
        );
        let d = &diags[0];
        assert_eq!(
            d.citation, *expected_citation,
            "Diagnostic.citation from {constraint_name} wrapper drifted from catalog label"
        );
        // PM Addendum II §8: every triggering wrapper must emit a FixProposal
        // with confidence 0.9. The only legitimate `None` case is the rare
        // ambiguous-span layout where `compute_relido_removal_span` cannot
        // pick a sound removal — none of the four canonical triggering
        // shapes used here exercises that path.
        let fix = d.fix.as_ref().unwrap_or_else(|| {
            panic!("{constraint_name} must emit a FixProposal on the canonical triggering shape")
        });
        assert!(
            (fix.confidence.rule - 0.9).abs() < f32::EPSILON,
            "{constraint_name} FixProposal confidence must be 0.9, got {}",
            fix.confidence.rule
        );
        assert_eq!(
            fix.replacement.as_ref(),
            "",
            "{constraint_name} FixProposal must replace with empty string (subtractive)"
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
         pin, or the baseline count drifted. Verify in decisions.md D17."
    );
}

// ---------------------------------------------------------------------------
// `compute_relido_removal_span` helper — separator-position cases
// ---------------------------------------------------------------------------
//
// Per PM Addendum II §3, the helper distinguishes three layout cases:
//
//   - **First**: `S//RELIDO/...` — RELIDO is the leading dissem token.
//     Removal span consumes the trailing `/`.
//   - **Middle**: `S//.../RELIDO/...` — RELIDO is between two siblings.
//     Removal span consumes the preceding `/`.
//   - **Last**: `S//.../RELIDO` — RELIDO is the final dissem token.
//     Removal span consumes the preceding `/`.
//
// Each case is exercised below. The `apply_fix` round-trip confirms the
// post-fix bytes are well-formed (no `//` adjacent to `/`, no leading or
// trailing `/` in the dissem block, and the surviving tokens are intact).

#[test]
fn helper_first_position_consumes_trailing_slash() {
    // "S//RELIDO/NOFORN" — RELIDO at byte [3, 9]; trailing `/` at byte 9.
    let (attrs, source) = attrs_for_dissem_block(
        "S",
        &[
            (DissemControl::Relido, "RELIDO"),
            (DissemControl::Nf, "NOFORN"),
        ],
    );
    let (span, original) = compute_relido_removal_span(&attrs)
        .expect("first-position layout must produce a removal span");
    assert_eq!(span.start, 3);
    assert_eq!(span.end, 10, "first-position fix must consume trailing `/`");
    assert_eq!(original.as_ref(), "RELIDO/");

    let fixed = apply_fix(&source, span, "");
    assert_eq!(fixed, "S//NOFORN");
    // Well-formedness: no `//` immediately followed by `/` (which would
    // indicate a stranded separator), no leading `/` after `//`, no
    // trailing `/`.
    assert!(!fixed.contains("///"));
    assert!(!fixed.ends_with('/'));
}

#[test]
fn helper_middle_position_consumes_preceding_slash() {
    // "S//OC/RELIDO/NOFORN" — RELIDO at byte [6, 12]; preceding `/` at
    // byte 5.
    let (attrs, source) = attrs_for_dissem_block(
        "S",
        &[
            (DissemControl::Oc, "OC"),
            (DissemControl::Relido, "RELIDO"),
            (DissemControl::Nf, "NOFORN"),
        ],
    );
    let (span, original) = compute_relido_removal_span(&attrs)
        .expect("middle-position layout must produce a removal span");
    assert_eq!(
        span.start, 5,
        "middle-position fix must consume preceding `/`"
    );
    assert_eq!(span.end, 12);
    assert_eq!(original.as_ref(), "/RELIDO");

    let fixed = apply_fix(&source, span, "");
    assert_eq!(fixed, "S//OC/NOFORN");
    assert!(!fixed.contains("///"));
    assert!(!fixed.ends_with('/'));
    assert!(!fixed.contains("RELIDO"));
}

#[test]
fn helper_last_position_consumes_preceding_slash() {
    // "S//OC/RELIDO" — RELIDO at byte [6, 12]; preceding `/` at byte 5.
    let (attrs, source) = attrs_for_dissem_block(
        "S",
        &[(DissemControl::Oc, "OC"), (DissemControl::Relido, "RELIDO")],
    );
    let (span, original) = compute_relido_removal_span(&attrs)
        .expect("last-position layout must produce a removal span");
    assert_eq!(
        span.start, 5,
        "last-position fix must consume preceding `/`"
    );
    assert_eq!(span.end, 12);
    assert_eq!(original.as_ref(), "/RELIDO");

    let fixed = apply_fix(&source, span, "");
    assert_eq!(fixed, "S//OC");
    assert!(!fixed.ends_with('/'));
    assert!(!fixed.contains("RELIDO"));
}

#[test]
fn helper_returns_none_when_relido_absent() {
    // No RELIDO in token_spans → helper must return None so the caller
    // emits the diagnostic without a fix (defensive Constitution V
    // posture: never emit a malformed fix).
    let (attrs, _source) = attrs_for_dissem_block("S", &[(DissemControl::Nf, "NOFORN")]);
    assert!(
        compute_relido_removal_span(&attrs).is_none(),
        "helper must return None when RELIDO is absent from token_spans"
    );
}

// ---------------------------------------------------------------------------
// FixProposal source / migration_ref / source_field discipline
// ---------------------------------------------------------------------------
//
// Per PM Addendum II §4: every emitted FixProposal must use
// FixSource::BuiltinRule (the existing strict-path provenance for
// hand-written CAPCO rules) and migration_ref: None (no schema migration
// involved). This test pins both fields across all four wrappers.

#[test]
fn relido_fix_proposals_carry_builtin_rule_source_and_no_migration_ref() {
    type W = (
        &'static str,
        &'static dyn Rule,
        &'static [(DissemControl, &'static str)],
    );
    let cases: &[W] = &[
        (
            "E054",
            &DeclarativeRelidoNofornConflictRule,
            &[
                (DissemControl::Relido, "RELIDO"),
                (DissemControl::Nf, "NOFORN"),
            ],
        ),
        (
            "E055",
            &DeclarativeRelidoDisplayOnlyConflictRule,
            &[
                (DissemControl::Relido, "RELIDO"),
                (DissemControl::Displayonly, "DISPLAYONLY"),
            ],
        ),
        (
            "E056",
            &DeclarativeOrconRelidoConflictRule,
            &[(DissemControl::Oc, "OC"), (DissemControl::Relido, "RELIDO")],
        ),
        (
            "E057",
            &DeclarativeOrconUsgovRelidoConflictRule,
            &[
                (DissemControl::OcUsgov, "OC-USGOV"),
                (DissemControl::Relido, "RELIDO"),
            ],
        ),
    ];
    for (rule_id, rule, dissem) in cases {
        let attrs = attrs_with_dissem_and_spans(dissem);
        let diags = rule.check(&attrs, &ctx());
        let fix = diags[0]
            .fix
            .as_ref()
            .unwrap_or_else(|| panic!("{rule_id} must emit a FixProposal"));
        assert_eq!(
            fix.source,
            FixSource::BuiltinRule,
            "{rule_id} FixProposal source must be BuiltinRule"
        );
        assert!(
            fix.migration_ref.is_none(),
            "{rule_id} FixProposal must have migration_ref = None"
        );
        assert_eq!(
            fix.rule.as_str(),
            *rule_id,
            "{rule_id} FixProposal rule field must match the wrapper's id()"
        );
    }
}
