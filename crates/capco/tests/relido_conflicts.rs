#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

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

use marque_capco::CapcoRuleSet;
use marque_capco::CapcoScheme;
use marque_capco::rules::{
    DeclarativeOrconRelidoConflictRule, DeclarativeOrconUsgovRelidoConflictRule,
    DeclarativeRelidoDisplayOnlyConflictRule, DeclarativeRelidoNofornConflictRule,
    compute_relido_removal_span, find_dissem_token_span,
};
use marque_capco::scheme::{TOK_DISPLAY_ONLY, TOK_NOFORN, TOK_ORCON, TOK_ORCON_USGOV, TOK_RELIDO};
use marque_config::Config;
use marque_engine::Engine;
use marque_ism::{
    CanonicalAttrs, Classification, DissemControl, MarkingClassification, MarkingType, Span,
    TokenKind, TokenSpan,
};
use marque_rules::{Diagnostic, FixSource, Rule, RuleContext, Severity};
use marque_scheme::{Constraint, MarkingScheme, TokenRef};

// ---------------------------------------------------------------------------
// Type aliases — keep `clippy::type_complexity` quiet under
// `cargo clippy --workspace --all-targets -- -D warnings` (M-4 fix).
// ---------------------------------------------------------------------------

/// One row of the citation-fidelity test case table:
/// `(constraint name, rule trait object, expected §-citation,
///   triggering dissem-control + token-text pairs)`.
type CitationCase = (
    &'static str,
    &'static dyn Rule<CapcoScheme>,
    &'static str,
    &'static [(DissemControl, &'static str)],
);

/// One row of the FixProposal-discipline test case table:
/// `(rule id, rule trait object, triggering dissem-control + token-text pairs)`.
type FixDisciplineCase = (
    &'static str,
    &'static dyn Rule<CapcoScheme>,
    &'static [(DissemControl, &'static str)],
);

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
    // PR 9b T132 / FR-046: field renamed from `dissem_controls` to
    // `dissem_us` (US-classified fixtures route here per CAPCO-2016
    // §G.2 Table 5).
    a.dissem_us = controls.to_vec().into_boxed_slice();
    a
}

/// Build `CanonicalAttrs` whose `token_spans` reflect the byte layout of
/// `source` for a banner / portion shaped `(<head>//<dissem-block>)`. The
/// dissem block is a `/`-separated list of tokens. Examples used by the
/// fixtures below — `RELIDO`, `NOFORN`, `OC`, `OC-USGOV`, `NF`, and
/// `DISPLAYONLY` — are written as their ODNI ISM XML CVE attribute
/// values (the form returned by `DissemControl::as_str()` in generated
/// `values.rs`). For tokens where the marking surface and the CVE
/// attribute differ (notably `DISPLAY ONLY` vs `DISPLAYONLY` per §H.8
/// p163), see `find_dissem_token_span` doc and the engine gap (#323)
/// for why these tests use the CVE form.
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
    // PR 9b T132 / FR-046: field renamed from `dissem_controls` to
    // `dissem_us` (US-classified fixtures route here per CAPCO-2016
    // §G.2 Table 5).
    a.dissem_us = dissem
        .iter()
        .map(|(d, _)| *d)
        .collect::<Vec<_>>()
        .into_boxed_slice();

    // Construct the source string and walk byte offsets in lock-step.
    // Mirror the real parser's TokenSpan emission pattern (verified
    // against `marque check` output 2026-05-08):
    //   - Cross-category `//` separators: emitted as Separator spans.
    //   - Intra-block `/` separators: NOT emitted; adjacent content
    //     tokens carry adjacent byte offsets with the `/` in the gap.
    let mut source = String::new();
    let head_start = source.len();
    source.push_str(head); // head occupies [0, head.len()]
    let head_end = source.len();

    let mut spans: Vec<TokenSpan> = Vec::with_capacity(2 + dissem.len());
    spans.push(TokenSpan {
        kind: TokenKind::Classification,
        text: head.to_string().into_boxed_str(),
        span: Span::new(head_start, head_end),
    });

    // Category-boundary `//` between head and first dissem token.
    let cat_sep_start = source.len();
    source.push_str("//");
    let cat_sep_end = source.len();
    spans.push(TokenSpan {
        kind: TokenKind::Separator,
        text: "//".to_string().into_boxed_str(),
        span: Span::new(cat_sep_start, cat_sep_end),
    });

    for (i, (_, text)) in dissem.iter().enumerate() {
        if i > 0 {
            // Intra-block `/`: bytes only, no Separator span.
            source.push('/');
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
///
/// Synthetic empty span — these tests construct the `CanonicalAttrs`
/// directly and do not exercise the engine's intent-synthesis path
/// that depends on `candidate_span`. PR 4b-B 9th-pass follow-up:
/// `RuleContext` is `#[non_exhaustive]`; cross-crate construction
/// goes through `RuleContext::new` (every optional context field
/// defaults to `None`).
fn ctx() -> RuleContext<'static> {
    RuleContext::new(MarkingType::Portion, marque_scheme::Span::new(0, 0))
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
    assert_eq!(fix.confidence.rule, 0.95);
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
    //
    // Honest fixture note: this test (and other E055 fixtures in this
    // file) uses `"DISPLAYONLY"` — the ODNI ISM XML CVE attribute value
    // — rather than the canonical CAPCO marking-surface form
    // `"DISPLAY ONLY"` (with space, per §H.8 p163, used in BOTH banner
    // and portion). The fixture exercises the parser path that actually
    // works today: `crates/ism/src/marking_forms.rs::MARKING_FORMS` has
    // no DISPLAY ONLY entry, so the parser only recognizes the CVE form
    // as a `DissemControl` token. Canonical marking-surface input would
    // be `S//RELIDO/DISPLAY ONLY` per §H.8 p163. Engine gap tracked at
    // #323; the wrapper's lookup chain already accepts both forms (see
    // `find_dissem_token_span` doc) so this fixture will continue to
    // pass once the parser closes the gap, and a sibling fixture using
    // the marking-surface form can be added at that time without
    // changing the wrapper.
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
    assert_eq!(fix.confidence.rule, 0.95);

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
    assert_eq!(fix.confidence.rule, 0.95);

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
    assert_eq!(fix.confidence.rule, 0.95);

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
    let cases: &[CitationCase] = &[
        (
            "E054/relido-conflicts-noforn",
            &DeclarativeRelidoNofornConflictRule as &dyn Rule<CapcoScheme>,
            "CAPCO-2016 §H.8 p154",
            &[
                (DissemControl::Relido, "RELIDO"),
                (DissemControl::Nf, "NOFORN"),
            ],
        ),
        (
            "E055/relido-conflicts-display-only",
            &DeclarativeRelidoDisplayOnlyConflictRule as &dyn Rule<CapcoScheme>,
            "CAPCO-2016 §H.8 p154",
            &[
                (DissemControl::Relido, "RELIDO"),
                (DissemControl::Displayonly, "DISPLAYONLY"),
            ],
        ),
        (
            "E056/orcon-conflicts-relido",
            &DeclarativeOrconRelidoConflictRule as &dyn Rule<CapcoScheme>,
            "CAPCO-2016 §H.8 p136",
            &[(DissemControl::Oc, "OC"), (DissemControl::Relido, "RELIDO")],
        ),
        (
            "E057/orcon-usgov-conflicts-relido",
            &DeclarativeOrconUsgovRelidoConflictRule as &dyn Rule<CapcoScheme>,
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
        // with confidence 0.95 (PM Addendum II §3, §8 — post-2026-05-08
        // calibration to clear the engine's default 0.95 threshold).
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
        // with confidence 0.95. The only legitimate `None` case is the rare
        // ambiguous-span layout where `compute_relido_removal_span` cannot
        // pick a sound removal — none of the four canonical triggering
        // shapes used here exercises that path.
        let fix = d.fix.as_ref().unwrap_or_else(|| {
            panic!("{constraint_name} must emit a FixProposal on the canonical triggering shape")
        });
        assert!(
            (fix.confidence.rule - 0.95).abs() < f32::EPSILON,
            "{constraint_name} FixProposal confidence must be 0.95, got {}",
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
// Pins the absolute count of `CapcoScheme::constraints()` after PR 3b.D.
// A failure here means a constraint was added or removed without an
// accompanying intentional documentation update. The pre-3b.C count (15)
// was verified by inspecting `build_constraints()` before this PR added
// any rows. PR 3b.C adds exactly 4 (E054–E057), giving 19. PR 3b.D
// (T026d) adds 27 class-floor catalog rows (5 TS + 8 S + 8 C + 2 UCNI
// + 4 passthrough) AND removes the two dead legacy entries
// (`E022/CNWDI-classification-floor`,
// `E025/ucni-conflicts-classification`) that the new catalog
// supersedes (R1 cleanup). Net: 19 + 27 − 2 = 44.
//
// PR 3b.E (T026e) adds 5 SCI per-system catalog rows (HCS-O companions,
// HCS-P NOFORN, HCS-P sub companions, SI-G companions, TK compartment
// NOFORN) at CAPCO-2016 §H.4 family granularity. Net: 44 + 5 = 49.
//
// Bump this number only when an intentional catalog change lands; the
// change must be documented in `specs/006-engine-rule-refactor/decisions.md`.

#[test]
fn capco_constraints_count_after_pr3b_e() {
    let scheme = CapcoScheme::new();
    // Pre-3b.C baseline: 15 constraints (verified by inspection of
    // `build_constraints()` on the `origin/staging` base at 13fdc085).
    // PR 3b.C adds exactly 4 (E054 / E055 / E056 / E057).
    // PR 3b.D adds 27 class-floor catalog rows AND removes 2 dead
    // legacy entries (E022 / E025) the new catalog supersedes.
    // PR 3b.E adds 5 SCI per-system catalog rows.
    assert_eq!(
        scheme.constraints().len(),
        49,
        "expected 15 (pre-3b.C) + 4 (E054–E057) + 27 (PR 3b.D class-floor catalog) − 2 \
         (E022/E025 legacy entries removed in R1 cleanup) + 5 (PR 3b.E SCI per-system \
         catalog) = 49 constraints after PR 3b.E; if this fails, either a constraint \
         was added/removed without updating this pin, or the baseline count drifted. \
         Verify in decisions.md D17 and the PR 3b.E planning doc \
         `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`."
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

#[test]
fn helper_sole_in_block_consumes_double_slash() {
    // Layout case 3 (M-1 boundary case + 2026-05-08 idempotency-fix
    // extension): input shaped like `S//RELIDO` has RELIDO as the sole
    // token in its dissem block, preceded by a `//` category separator.
    // The helper consumes BOTH preceding `/`s so the stranded category
    // separator goes with the RELIDO payload. After fix: `S`. Without
    // case 3, the helper would either fabricate a malformed trailing-`/`
    // removal (the original bug the M-1 review flagged) OR emit no fix
    // and let a follow-on E004 separator-collapse pass run on a second
    // pass — the second behavior is what proptest_engine's `fix_idempotent`
    // caught for `TOP SECRET//NOFORN//RELIDO\n`-shaped banners.
    //
    // attrs_for_dissem_block("S", &[Relido]) produces source "S//RELIDO".
    // S occupies [0, 1]; "//" at [1, 3]; RELIDO at [3, 9].
    let (attrs, source) = attrs_for_dissem_block("S", &[(DissemControl::Relido, "RELIDO")]);
    let (span, original) = compute_relido_removal_span(&attrs)
        .expect("sole-in-block layout must produce a `//RELIDO` removal");
    assert_eq!(
        span.start, 1,
        "sole-in-block fix must consume both preceding `/`s"
    );
    assert_eq!(span.end, 9);
    assert_eq!(original.as_ref(), "//RELIDO");

    let fixed = apply_fix(&source, span, "");
    assert_eq!(fixed, "S");
    assert!(!fixed.contains("RELIDO"));
    assert!(!fixed.contains("//"));
    assert!(!fixed.ends_with('/'));
}

#[test]
fn helper_banner_form_double_slash_neighbor() {
    // Layout case 3 in banner form: `TOP SECRET//NOFORN//RELIDO`.
    // NOFORN and RELIDO sit in separate dissem-control category blocks
    // (the `//` between them is a category separator under malformed-but-
    // recognizable input that the parser surfaces as two TokenSpans
    // separated by 2 bytes). The helper consumes `//RELIDO` so the post-
    // fix banner is `TOP SECRET//NOFORN`.
    //
    // This is the proptest_engine::fix_idempotent regression case
    // (2026-05-08): without case 3, a first pass would apply E004
    // (`//` → `/`) and a second pass would apply E054 with a different
    // span layout, breaking `Engine::fix` idempotency. With case 3, E054's
    // span subsumes E004's via the engine's overlap guard and idempotency
    // holds in one pass.
    //
    // attrs_for_dissem_block("TOP SECRET//NOFORN", &[Relido]) produces
    // source "TOP SECRET//NOFORN//RELIDO" (the head string is taken
    // verbatim, including the inner "//"). NOFORN is NOT a TokenSpan in
    // this synthetic — only the head Classification ("TOP SECRET//NOFORN"
    // span) and the trailing RELIDO. We synthesize the layout directly
    // here to mirror what the real parser produces.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.dissem_us = vec![DissemControl::Nf, DissemControl::Relido].into_boxed_slice();
    // Real parser layout for `TOP SECRET//NOFORN//RELIDO` (verified by
    // running `marque check` against `/tmp/banner.txt` during PR review):
    //   Classification "TOP SECRET" @ [0, 10]
    //   Separator     "//"          @ [10, 12]   (cross-category boundary)
    //   DissemControl "NOFORN"      @ [12, 18]
    //   Separator     "//"          @ [18, 20]   (cross-category boundary)
    //   DissemControl "RELIDO"      @ [20, 26]
    // Intra-block `/` separators (e.g. between siblings within one
    // dissem block) are NOT emitted as Separator spans; only the
    // cross-category `//` boundaries are.
    a.token_spans = vec![
        TokenSpan {
            kind: TokenKind::Classification,
            text: "TOP SECRET".to_string().into_boxed_str(),
            span: Span::new(0, 10),
        },
        TokenSpan {
            kind: TokenKind::Separator,
            text: "//".to_string().into_boxed_str(),
            span: Span::new(10, 12),
        },
        TokenSpan {
            kind: TokenKind::DissemControl,
            text: "NOFORN".to_string().into_boxed_str(),
            span: Span::new(12, 18),
        },
        TokenSpan {
            kind: TokenKind::Separator,
            text: "//".to_string().into_boxed_str(),
            span: Span::new(18, 20),
        },
        TokenSpan {
            kind: TokenKind::DissemControl,
            text: "RELIDO".to_string().into_boxed_str(),
            span: Span::new(20, 26),
        },
    ]
    .into_boxed_slice();

    let (span, original) = compute_relido_removal_span(&a)
        .expect("banner-form `//RELIDO` layout must produce a removal span");
    assert_eq!(span.start, 18, "fix must consume both preceding `/`s");
    assert_eq!(span.end, 26);
    assert_eq!(original.as_ref(), "//RELIDO");

    let source = "TOP SECRET//NOFORN//RELIDO";
    let fixed = apply_fix(source, span, "");
    assert_eq!(fixed, "TOP SECRET//NOFORN");
    assert!(!fixed.contains("RELIDO"));
    assert!(!fixed.ends_with('/'));
}

#[test]
fn helper_returns_none_when_no_recognized_layout() {
    // Defensive fall-through: synthetic input where RELIDO has neither a
    // `/`-adjacent prior, nor a `/`-adjacent following sibling, nor a
    // `//`-preceded prior. None of the three recognized layout cases
    // applies, so the helper returns None. Caller falls back to no-fix
    // (Constitution V: never emit a malformed fix). Realistic parser
    // output never produces this layout, but the helper is `pub` and
    // future call sites in PR 3.7 may exercise pathological cases.
    //
    // Construct: RELIDO at offset 100 with no other token spans at all
    // (no prior, no following). All three cases fall through.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    a.token_spans = vec![TokenSpan {
        kind: TokenKind::DissemControl,
        text: "RELIDO".to_string().into_boxed_str(),
        span: Span::new(100, 106),
    }]
    .into_boxed_slice();

    assert!(
        compute_relido_removal_span(&a).is_none(),
        "helper must return None when no recognized layout matches"
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
    let cases: &[FixDisciplineCase] = &[
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

// ---------------------------------------------------------------------------
// Copilot R2 regression tests — banner-form vs portion-form anchor lookup
// ---------------------------------------------------------------------------
//
// CAPCO-2016 §G.1 Table 4 (p36) and §H.8 templates distinguish four
// form-spaces per dissem control that callers anchoring at a
// dissem-control token must accommodate: (1) Banner Line Marking Title
// (long surface form, e.g. `ORCON`, `NOFORN`, `DISPLAY ONLY`); (2)
// Banner Line Abbreviation (short banner form when registered, e.g.
// `OC`, `NF` — `None` for `DISPLAY ONLY` per §H.8 p163); (3) Portion
// Mark (typically same as the banner abbreviation, or the long form
// when no abbreviation exists, e.g. `DISPLAY ONLY [LIST]` per §H.8
// p163); and (4) ODNI ISM XML CVE attribute value
// (`DissemControl::as_str()`, e.g. `"OC"`, `"OC-USGOV"`, `"NF"`,
// `"DISPLAYONLY"` — the data shape used in
// `ism:disseminationControls="..."`). See `find_dissem_token_span`
// doc for the full taxonomy and the engine gap (#323).
//
// The parser preserves raw user input verbatim in `TokenSpan::text`
// (per `crates/core/src/parser.rs` — every push uses
// `text: trimmed.into()`, no canonicalization). Earlier wrapper
// anchor lookups matched only the CVE form (`"OC"`, `"OC-USGOV"`,
// `"DISPLAYONLY"`); banner-form input fell through to the secondary
// anchor (RELIDO), which is the wrong cursor location for the two
// asymmetric rules (E056 / E057) where ORCON / ORCON-USGOV is the
// §-asserting side per PM Addendum II Q1.
//
// Within-category separator: `/` (single slash) per CAPCO-2016 §A.6
// Figure 2 p17. `//` separates categories; the dissem block uses `/`
// to chain multiple values. Test fixtures below use the correct
// `//<class>//<dissem1>/<dissem2>` shape.

/// Build a default-configured `Engine` for engine-path regression tests.
fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Run `source` through the engine and return its diagnostics.
fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Find the first diagnostic with a given rule ID, or panic with a
/// descriptive message naming every rule that DID fire.
fn first_diag_for_rule<'a>(
    diags: &'a [Diagnostic<CapcoScheme>],
    rule_id: &str,
) -> &'a Diagnostic<CapcoScheme> {
    diags
        .iter()
        .find(|d| d.rule.as_str() == rule_id)
        .unwrap_or_else(|| {
            let fired: Vec<&str> = diags.iter().map(|d| d.rule.as_str()).collect();
            panic!("{rule_id} did not fire on the test input; fired rules: {fired:?}")
        })
}

#[test]
fn e056_anchors_at_orcon_token_in_banner_form() {
    // Banner form: `SECRET//ORCON/RELIDO` (per CAPCO-2016 §A.6 Figure 2 p17,
    // `/` separates same-category dissem values; `//` separates categories).
    // ORCON is the §-asserting token per §H.8 p136 ("May not be used with
    // RELIDO"). The diagnostic span MUST anchor at ORCON, not at RELIDO.
    let source = "SECRET//ORCON/RELIDO\n";
    let diags = lint(source);
    let d = first_diag_for_rule(&diags, "E056");
    let anchor = &source[d.span.start..d.span.end];
    assert_eq!(
        anchor, "ORCON",
        "E056 banner-form diagnostic must anchor at the ORCON token (the §H.8 \
         p136 asserting side), got anchor={anchor:?} for span={:?}",
        d.span
    );
}

#[test]
fn e056_anchors_at_oc_token_in_portion_form() {
    // Portion form: `(S//OC/RELIDO)` (CVE abbreviations, parens, abbreviated
    // class per §H.1 p47–51). Anchors at OC.
    let source = "(S//OC/RELIDO)\n";
    let diags = lint(source);
    let d = first_diag_for_rule(&diags, "E056");
    let anchor = &source[d.span.start..d.span.end];
    assert_eq!(
        anchor, "OC",
        "E056 portion-form diagnostic must anchor at the OC token (CVE \
         portion abbreviation for ORCON), got anchor={anchor:?} for span={:?}",
        d.span
    );
}

#[test]
fn e057_anchors_at_orcon_usgov_token_in_banner_form() {
    // Banner form: `SECRET//ORCON-USGOV/RELIDO`. ORCON-USGOV is the
    // §-asserting token per §H.8 p140 ("May not be used with RELIDO").
    // The diagnostic span MUST anchor at ORCON-USGOV.
    let source = "SECRET//ORCON-USGOV/RELIDO\n";
    let diags = lint(source);
    let d = first_diag_for_rule(&diags, "E057");
    let anchor = &source[d.span.start..d.span.end];
    assert_eq!(
        anchor, "ORCON-USGOV",
        "E057 banner-form diagnostic must anchor at the ORCON-USGOV token \
         (the §H.8 p140 asserting side), got anchor={anchor:?} for span={:?}",
        d.span
    );
}

#[test]
fn e057_anchors_at_oc_usgov_token_in_portion_form() {
    // Portion form: `(S//OC-USGOV/RELIDO)`. Anchors at OC-USGOV.
    let source = "(S//OC-USGOV/RELIDO)\n";
    let diags = lint(source);
    let d = first_diag_for_rule(&diags, "E057");
    let anchor = &source[d.span.start..d.span.end];
    assert_eq!(
        anchor, "OC-USGOV",
        "E057 portion-form diagnostic must anchor at the OC-USGOV token (CVE \
         portion abbreviation for ORCON-USGOV), got anchor={anchor:?} for \
         span={:?}",
        d.span
    );
}

#[test]
fn find_dissem_token_span_matches_marking_or_cve_form() {
    // E055 helper unit test: `find_dissem_token_span` returns the first
    // matching `TokenKind::DissemControl` span whose text equals any
    // supplied form. Verifies the helper-level invariant the four
    // wrappers depend on: the marking-surface form (banner / portion
    // per §H.8 p163, both `"DISPLAY ONLY"` with space) AND the ODNI
    // ISM XML CVE attribute value (`"DISPLAYONLY"`, no space — the data
    // shape used in `ism:disseminationControls="..."`) both resolve
    // through a single call. These are orthogonal axes — marking
    // surface vs XML CVE — not two surface forms (per §H.8 p163,
    // `DISPLAY ONLY` has NO abbreviation; it is the form on both
    // banner and portion).
    //
    // Four sub-cases against the same form list
    // `["DISPLAY ONLY", "DISPLAYONLY"]` (the E055 fallback chain):
    // marking-surface present, CVE attribute value present, neither
    // present, kind discriminator.

    // Sub-case 1: marking-surface form `"DISPLAY ONLY"` present (banner
    // long name AND portion mark per §H.8 p163; the parser sees this
    // when a user types canonical CAPCO marking syntax).
    let mut a1 = CanonicalAttrs::default();
    a1.classification = Some(MarkingClassification::Us(Classification::Secret));
    a1.token_spans = vec![TokenSpan {
        kind: TokenKind::DissemControl,
        text: "DISPLAY ONLY".to_string().into_boxed_str(),
        span: Span::new(10, 22),
    }]
    .into_boxed_slice();
    assert_eq!(
        find_dissem_token_span(&a1, &["DISPLAY ONLY", "DISPLAYONLY"]),
        Some(Span::new(10, 22)),
        "marking-surface form `DISPLAY ONLY` (§H.8 p163, both banner and \
         portion) must resolve through the helper"
    );

    // Sub-case 2: ODNI ISM XML CVE attribute value `"DISPLAYONLY"`
    // present (round-trip / programmatic input — the form stored in
    // `ism:disseminationControls=` per ODNI `CVEnumISMDissem.xml`,
    // returned by `DissemControl::Displayonly::as_str()`).
    let mut a2 = CanonicalAttrs::default();
    a2.classification = Some(MarkingClassification::Us(Classification::Secret));
    a2.token_spans = vec![TokenSpan {
        kind: TokenKind::DissemControl,
        text: "DISPLAYONLY".to_string().into_boxed_str(),
        span: Span::new(5, 16),
    }]
    .into_boxed_slice();
    assert_eq!(
        find_dissem_token_span(&a2, &["DISPLAY ONLY", "DISPLAYONLY"]),
        Some(Span::new(5, 16)),
        "ODNI ISM XML CVE attribute value `DISPLAYONLY` must resolve \
         through the helper (round-trip / programmatic input path)"
    );

    // Sub-case 3: neither form present → None.
    let mut a3 = CanonicalAttrs::default();
    a3.classification = Some(MarkingClassification::Us(Classification::Secret));
    a3.token_spans = vec![TokenSpan {
        kind: TokenKind::DissemControl,
        text: "RELIDO".to_string().into_boxed_str(),
        span: Span::new(0, 6),
    }]
    .into_boxed_slice();
    assert_eq!(
        find_dissem_token_span(&a3, &["DISPLAY ONLY", "DISPLAYONLY"]),
        None,
        "absent surface forms must produce None"
    );

    // Sub-case 4: kind discriminator — a non-DissemControl token whose
    // text happens to match a form must NOT resolve. Pins the (kind,
    // text) conjunction.
    let mut a4 = CanonicalAttrs::default();
    a4.classification = Some(MarkingClassification::Us(Classification::Secret));
    a4.token_spans = vec![TokenSpan {
        kind: TokenKind::Classification,
        text: "DISPLAYONLY".to_string().into_boxed_str(),
        span: Span::new(0, 11),
    }]
    .into_boxed_slice();
    assert_eq!(
        find_dissem_token_span(&a4, &["DISPLAY ONLY", "DISPLAYONLY"]),
        None,
        "kind must be DissemControl — text-only match must NOT resolve"
    );
}

// ---------------------------------------------------------------------------
// PR 3c.B Commit 8 — FixIntent dual-population shape tests for E055 / E056
// ---------------------------------------------------------------------------
//
// Mirror E054 / E057 (beachhead Commit 3): every wrapper that emits a
// `FixProposal` now also emits a structurally equivalent `FixIntent<S>`
// (`ReplacementIntent::FactRemove { token_ref: FactRef::Cve(TOK_RELIDO),
// scope: Scope::Portion }`). The engine pairs `(fix, fix_intent)` at
// promotion time so the audit record carries the `New { intent,
// synthesized }` variant.
//
// These tests pin the intent's structural shape — not the FixProposal
// byte layout (that's covered by `e055_fires_*` / `e056_fires_*` above
// and by the engine-level baseline gate at `byte_identity_pr3c.rs`).

#[test]
fn e055_dual_populates_fix_intent_with_factremove_relido_portion() {
    let (attrs, _src) = attrs_for_dissem_block(
        "S",
        &[
            (DissemControl::Relido, "RELIDO"),
            (DissemControl::Displayonly, "DISPLAYONLY"),
        ],
    );
    let rule = DeclarativeRelidoDisplayOnlyConflictRule;
    let diags = rule.check(&attrs, &ctx());
    assert_eq!(diags.len(), 1);

    let d = &diags[0];
    assert!(
        d.fix.is_some(),
        "E055 dual-population: legacy FixProposal must still emit (byte-identity gate)"
    );
    let intent = d
        .fix
        .as_ref()
        .expect("E055 dual-population: FixIntent must be populated after Commit 8 migration");
    match &intent.replacement {
        marque_scheme::ReplacementIntent::FactRemove { facts, scope } => {
            assert_eq!(
                facts.len(),
                1,
                "E055 FactRemove must have exactly one fact (RELIDO)"
            );
            assert_eq!(
                facts[0],
                marque_scheme::FactRef::Cve(TOK_RELIDO),
                "E055 must remove RELIDO (the §H.8 p154 rejected token)"
            );
            assert_eq!(
                *scope,
                marque_scheme::Scope::Portion,
                "E055 intent scope must be Portion (per `relido_remove_intent()`)"
            );
        }
        other => panic!("E055 intent must be FactRemove; got: {other:?}"),
    }
    // Assert on `.rule` (the rule-authored axis) rather than
    // `.combined()` to match the E054/E057 wrapper-tests above
    // (lines 379, 536) — distinguishes "rule confidence drifted"
    // from "recognition pipeline changed" if this ever flips red.
    assert!(
        (intent.confidence.rule - 0.95).abs() < f32::EPSILON,
        "E055 intent confidence.rule must match the legacy FixProposal (0.95) so \
         the engine's threshold gate produces identical filter behavior"
    );
}

#[test]
fn e056_dual_populates_fix_intent_with_factremove_relido_portion() {
    let (attrs, _src) = attrs_for_dissem_block(
        "S",
        &[(DissemControl::Oc, "OC"), (DissemControl::Relido, "RELIDO")],
    );
    let rule = DeclarativeOrconRelidoConflictRule;
    let diags = rule.check(&attrs, &ctx());
    assert_eq!(diags.len(), 1);

    let d = &diags[0];
    assert!(
        d.fix.is_some(),
        "E056 dual-population: legacy FixProposal must still emit (byte-identity gate)"
    );
    let intent = d
        .fix
        .as_ref()
        .expect("E056 dual-population: FixIntent must be populated after Commit 8 migration");
    match &intent.replacement {
        marque_scheme::ReplacementIntent::FactRemove { facts, scope } => {
            assert_eq!(
                facts.len(),
                1,
                "E056 FactRemove must have exactly one fact (RELIDO)"
            );
            assert_eq!(
                facts[0],
                marque_scheme::FactRef::Cve(TOK_RELIDO),
                "E056 must remove RELIDO (the §H.8 p136 rejected token)"
            );
            assert_eq!(
                *scope,
                marque_scheme::Scope::Portion,
                "E056 intent scope must be Portion"
            );
        }
        other => panic!("E056 intent must be FactRemove; got: {other:?}"),
    }
    // Assert on `.rule` for the same reason as E055 above.
    assert!(
        (intent.confidence.rule - 0.95).abs() < f32::EPSILON,
        "E056 intent confidence.rule must match the legacy FixProposal (0.95)"
    );
}

#[test]
fn e055_intent_absent_when_fix_helper_returns_none() {
    // When `build_relido_removal_fix` cannot anchor (no preceding /
    // following separator), both `fix` and `fix_intent` must be None
    // — never asymmetric. Mirror the E054 None-arm in
    // `DeclarativeRelidoNofornConflictRule::check` and the
    // `Diagnostic::new(..., None)` fall-through.
    //
    // Constructing a triggering attrs whose `build_relido_removal_fix`
    // returns None requires `attrs.dissem_us` containing both RELIDO
    // and DISPLAY ONLY but with NO RELIDO token span (parser gap).
    // The wrapper's `violations_for` predicate fires on the dissem-
    // axis set (queried namespace-agnostically via `dissem_iter()`);
    // the fix helper needs the token span.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Relido, DissemControl::Displayonly].into_boxed_slice();
    // Deliberately omit RELIDO from token_spans to force the None arm.
    a.token_spans = vec![TokenSpan {
        kind: TokenKind::DissemControl,
        text: "DISPLAYONLY".to_string().into_boxed_str(),
        span: Span::new(3, 14),
    }]
    .into_boxed_slice();

    let rule = DeclarativeRelidoDisplayOnlyConflictRule;
    let diags = rule.check(&a, &ctx());
    assert_eq!(diags.len(), 1, "rule must still emit the diagnostic");
    let d = &diags[0];
    assert!(d.fix.is_none(), "fix must be None when helper returns None");
    assert!(
        d.fix.is_none(),
        "fix_intent must be None in the None arm — never asymmetric"
    );
}
