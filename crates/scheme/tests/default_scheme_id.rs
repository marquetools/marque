// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `MarkingScheme::scheme_id()` / `constraint_rule_id()` trait DEFAULTS.
//!
//! `CapcoScheme` overrides `scheme_id()` to `"capco"` (covered in
//! `crates/capco/tests/constraint_rule_id.rs`), and the shared
//! `marque_test_utils::stub_scheme::StubScheme` overrides it to
//! `"stub"` — so both exercise the OVERRIDE path. Issue #834 restored
//! these trait defaults but added no test for the DEFAULT projection.
//!
//! This fixture overrides NEITHER method, pinning the trait DEFAULT:
//! `scheme_id()` returns `"scheme"`, and `constraint_rule_id(label)`
//! returns the identity projection `(scheme_id(), label)` ==
//! `("scheme", label)`. Engine-internal behavior — no CAPCO citation.

use marque_scheme::ambiguity::Parsed;
use marque_scheme::category::{Category, TokenId};
use marque_scheme::constraint::{Constraint, TokenRef};
use marque_scheme::lattice::{BoundedJoinSemilattice, JoinSemilattice, MeetSemilattice};
use marque_scheme::page_rewrite::PageRewrite;
use marque_scheme::scheme::MarkingScheme;
use marque_scheme::scope::Scope;
use marque_scheme::template::Template;

/// Minimal lattice-trivial marking — a single presence bit; `false` is
/// bottom.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct DefaultIdMarking {
    has_token: bool,
}

impl JoinSemilattice for DefaultIdMarking {
    fn join(&self, other: &Self) -> Self {
        Self {
            has_token: self.has_token || other.has_token,
        }
    }
}

impl MeetSemilattice for DefaultIdMarking {
    fn meet(&self, other: &Self) -> Self {
        Self {
            has_token: self.has_token && other.has_token,
        }
    }
}

impl BoundedJoinSemilattice for DefaultIdMarking {
    fn bottom() -> Self {
        Self { has_token: false }
    }
}

/// Stub parse error — never returned (the stub `parse` yields a
/// zero-candidate `Ambiguous`), but the trait requires a named type.
#[derive(Debug)]
struct DefaultIdParseError;

impl std::fmt::Display for DefaultIdParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DefaultIdParseError")
    }
}
impl std::error::Error for DefaultIdParseError {}

/// Minimal scheme that overrides NEITHER `scheme_id()` nor
/// `constraint_rule_id()`, so both fall through to the trait default.
#[derive(Clone, Debug, Default)]
struct DefaultIdScheme;

impl MarkingScheme for DefaultIdScheme {
    type Token = TokenId;
    type Marking = DefaultIdMarking;
    type ParseError = DefaultIdParseError;
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();
    type Projected = ();

    fn name(&self) -> &str {
        "default-id"
    }
    fn schema_version(&self) -> &str {
        "default-id-1"
    }
    fn categories(&self) -> &[Category] {
        &[]
    }
    fn constraints(&self) -> &[Constraint] {
        &[]
    }
    fn templates(&self) -> &[Template] {
        &[]
    }
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &[]
    }
    fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Ok(Parsed::Ambiguous {
            candidates: Vec::new(),
        })
    }
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        matches!(token_ref, TokenRef::Token(TokenId(1))) && marking.has_token
    }
    fn project(&self, _scope: Scope, markings: &[Self::Marking]) -> Self::Marking {
        markings
            .iter()
            .fold(DefaultIdMarking::bottom(), |acc, m| acc.join(m))
    }
    fn render_item(&self, _marking: &Self::Marking) -> String {
        String::new()
    }
    fn render_summary(&self, _markings: &Self::Marking) -> String {
        String::new()
    }
    fn render_canonical(
        &self,
        _marking: &Self::Marking,
        _ctx: &marque_scheme::RenderContext,
        _out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }

    // `scheme_id` and `constraint_rule_id` intentionally NOT overridden —
    // that is the point of this fixture.
}

#[test]
fn default_scheme_id_is_scheme() {
    let scheme = DefaultIdScheme;
    assert_eq!(scheme.scheme_id(), "scheme");
}

#[test]
fn default_constraint_rule_id_is_identity_under_scheme_namespace() {
    let scheme = DefaultIdScheme;
    let label = "banner.classification.foo";
    // The default projection is the identity `(scheme_id(), label)`.
    assert_eq!(scheme.constraint_rule_id(label), ("scheme", label));
    assert_eq!(scheme.constraint_rule_id(label).0, scheme.scheme_id());
    assert_eq!(scheme.constraint_rule_id(label).1, label);
}

#[test]
fn default_constraint_rule_id_distinguishes_by_label() {
    let scheme = DefaultIdScheme;
    let a = scheme.constraint_rule_id("dissem.noforn.conflicts-rel-to");
    let b = scheme.constraint_rule_id("classification.floor.below-page-floor");
    assert_ne!(a, b);
    // Distinctness is carried entirely by the label half — both share
    // the default `"scheme"` namespace.
    assert_eq!(a.0, b.0);
    assert_ne!(a.1, b.1);
}
