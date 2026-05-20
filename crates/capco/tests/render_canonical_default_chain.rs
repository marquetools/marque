// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Commit 4 — `MarkingScheme::render_canonical` default chain
//! acceptance test.
//!
//! Pins the byte-identity property of the trait-default delegation
//! between `render_canonical(_, Scope::Portion, _)` and
//! `render_portion(_)`, and between
//! `render_canonical(_, Scope::Page|Document, _)` and
//! `render_banner(_)`. Plus the `Scope::Diff -> Err(fmt::Error)` contract.
//!
//! # Why this test is load-bearing
//!
//! Commit 4 is the trait-surface introduction step in PR 3c.B. The
//! commitment is **purely additive**: existing call paths that go
//! through `render_portion` / `render_banner` produce byte-identical
//! output to the pre-commit baseline, and `render_canonical` exists
//! as a new trait method that (today) delegates back to those
//! existing methods. Commit 5 inverts this dependency — populating
//! [`marque_capco::scheme::RENDER_TABLE`] and making `render_canonical`
//! the substantive body — at which point the byte-identity property
//! flips direction: `render_portion` / `render_banner` will use the
//! trait defaults that call back into `render_canonical`. Either way,
//! the property this test pins is the invariant.
//!
//! # Scope: `Scope::Diff`
//!
//! Diff is a rule-context query mode (architecture.md §3.4 type-sketch),
//! not a renderer-output scope. `marque_rules::RecanonScope` narrows
//! `Scope` precisely to exclude `Diff` from recanonicalization targets;
//! the renderer surface mirrors that by returning `Err(fmt::Error)`
//! when asked to render at `Scope::Diff`.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{CanonicalAttrs, Classification, MarkingClassification};
use marque_scheme::{EmissionForm, MarkingScheme, RenderContext, SchemaVersionId, Scope};

/// Helper: build a default-mode RenderContext (Auto + MarqueMvp3) at
/// the given scope. PR 3c.2.A: every render_canonical call site
/// constructs explicitly per PM-6 (no `Default` impl on RenderContext).
fn ctx(scope: Scope) -> RenderContext {
    RenderContext::new(scope, EmissionForm::Auto, SchemaVersionId::MarqueMvp3)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal `CapcoMarking` carrying just a US `SECRET`
/// classification. The trait-impl render path on `CapcoScheme` reads
/// `m.0.classification` and ignores the other fields; the Phase A
/// renderer covers only the classification axis. Commit 5's full
/// renderer body extends coverage to all axes; this test will continue
/// to pin the classification axis verbatim because that axis is
/// authoritative under both renderer generations.
fn make_secret() -> CapcoMarking {
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
    CapcoMarking::new(attrs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn render_canonical_portion_matches_render_portion() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();

    // Direct render_portion (the override path).
    let portion = scheme.render_portion(&marking);

    // render_canonical via Scope::Portion.
    let mut canon = String::new();
    let result = scheme.render_canonical(&marking, &ctx(Scope::Portion), &mut canon);
    assert!(
        result.is_ok(),
        "render_canonical(Scope::Portion) must succeed; got {result:?}"
    );

    assert_eq!(
        canon, portion,
        "Scope::Portion render_canonical output must be byte-identical to render_portion"
    );
    // Sanity: the Phase A renderer emits "S" for a Secret portion. If
    // this assertion fires, the renderer's classification axis
    // changed — that's a real regression, not a flake.
    assert_eq!(portion, "S");
}

#[test]
fn render_canonical_page_matches_render_banner() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();

    let banner = scheme.render_banner(&marking);

    let mut canon = String::new();
    let result = scheme.render_canonical(&marking, &ctx(Scope::Page), &mut canon);
    assert!(
        result.is_ok(),
        "render_canonical(Scope::Page) must succeed; got {result:?}"
    );

    assert_eq!(
        canon, banner,
        "Scope::Page render_canonical output must be byte-identical to render_banner"
    );
    assert_eq!(banner, "SECRET");
}

#[test]
fn render_canonical_document_matches_render_banner() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();

    let banner = scheme.render_banner(&marking);

    let mut canon = String::new();
    let result = scheme.render_canonical(&marking, &ctx(Scope::Document), &mut canon);
    assert!(
        result.is_ok(),
        "render_canonical(Scope::Document) must succeed; got {result:?}"
    );

    // Document scope agrees with Page on single-portion markings.
    // The architecture spec calls out that Page/Document may diverge
    // for multi-page documents; for a single-marking test they agree
    // by construction.
    assert_eq!(
        canon, banner,
        "Scope::Document render_canonical output must agree with render_banner on this fixture"
    );
}

#[test]
fn render_canonical_diff_returns_err() {
    let scheme = CapcoScheme::new();
    let marking = make_secret();

    let mut canon = String::new();
    let result = scheme.render_canonical(&marking, &ctx(Scope::Diff), &mut canon);
    assert!(
        result.is_err(),
        "render_canonical(Scope::Diff) must return Err(fmt::Error); got {result:?}"
    );
    // The error variant is `fmt::Error`, which is unit-typed — no
    // payload to assert beyond the `is_err()` shape. The doc comment
    // on `MarkingScheme::render_canonical` is the canonical reference
    // for the contract.
}

#[test]
fn writer_passing_appends_does_not_clear() {
    // Pins the writer-passing contract: `render_canonical` MUST
    // append to `out` and MUST NOT clear it. This is the property
    // commit 6's first consumer relies on when reusing a per-page
    // scratch buffer across multiple portions.
    let scheme = CapcoScheme::new();
    let marking = make_secret();

    let mut buf = String::from("PREFIX:");
    let result = scheme.render_canonical(&marking, &ctx(Scope::Portion), &mut buf);
    assert!(result.is_ok());

    assert!(
        buf.starts_with("PREFIX:"),
        "render_canonical must append to the buffer, not clear it; got {buf:?}"
    );
    assert_eq!(buf, "PREFIX:S");
}

#[test]
fn writer_can_be_reused_across_calls() {
    // Pins the scratch-buffer reuse pattern that commit 6 relies on:
    // the same `String` can be `clear()`ed and reused across multiple
    // `render_canonical` calls without re-allocating.
    let scheme = CapcoScheme::new();
    let marking = make_secret();

    let mut buf = String::new();

    scheme
        .render_canonical(&marking, &ctx(Scope::Portion), &mut buf)
        .unwrap();
    assert_eq!(buf, "S");

    buf.clear();
    scheme
        .render_canonical(&marking, &ctx(Scope::Page), &mut buf)
        .unwrap();
    assert_eq!(buf, "SECRET");
}

#[test]
fn fmt_write_into_arbitrary_writer() {
    // Pins that `render_canonical` works with any `fmt::Write`
    // implementor, not just `String`. The trait method signature is
    // `&mut dyn fmt::Write` precisely so callers can route output
    // into a `Formatter`, a `String`, or any custom writer without
    // forcing an intermediate `String` allocation.
    struct CountingWriter {
        bytes: usize,
    }

    impl core::fmt::Write for CountingWriter {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            self.bytes += s.len();
            Ok(())
        }
    }

    let scheme = CapcoScheme::new();
    let marking = make_secret();

    let mut w = CountingWriter { bytes: 0 };
    let result = scheme.render_canonical(&marking, &ctx(Scope::Page), &mut w);
    assert!(result.is_ok());
    assert_eq!(w.bytes, "SECRET".len());
}
