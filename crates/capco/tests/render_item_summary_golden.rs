// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Byte-identical golden test for the public `MarkingScheme::render_item`
//! (portion form) and `MarkingScheme::render_summary` (banner form)
//! accessors on `CapcoScheme`.
//!
//! # Why this test exists
//!
//! `render_item` / `render_summary` (formerly `render_portion` /
//! `render_banner`) are thin default-method projections over
//! `render_canonical` at `Scope::Portion` / `Scope::Page`. The Phase-B T3
//! rename of those method names MUST NOT change a single output byte — the
//! method bodies still delegate to the same `render_canonical` authority.
//!
//! This test pins exact bytes for a representative cross-axis set of CAPCO
//! markings so that the rename (and any future refactor of the projection
//! accessors) is provably output-preserving. It complements
//! `render_canonical_axis_fixtures.rs`, which exercises `render_canonical`
//! directly; this one exercises the public `render_item` / `render_summary`
//! surface specifically.
//!
//! # Authority
//!
//! Each expected string is the canonical form per CAPCO-2016 §A.6 p15-16
//! (banner + portion grammar) and the per-axis §H passages the renderer
//! already pins in `render_canonical_axis_fixtures.rs`. The verification
//! oracle is `crates/capco/docs/CAPCO-2016.md`.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_core::Parser;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_ism::CanonicalAttrs;
use marque_scheme::{MarkingScheme, Span};

/// Parse `source` as `kind` into canonicalized attrs. The strict-path
/// inputs below are, by construction, parseable. Mirrors the helper in
/// `parse_render_roundtrip.rs`.
fn parse(scheme: &CapcoScheme, source: &str, kind: MarkingType) -> CanonicalAttrs {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind,
    };
    parser
        .parse(&candidate, source.as_bytes())
        .ok()
        .map(|p| scheme.canonicalize(p.attrs))
        .expect("golden input must parse")
}

/// Render the portion (short) form of a parsed banner-or-portion input.
fn item(scheme: &CapcoScheme, source: &str, kind: MarkingType) -> String {
    let attrs = parse(scheme, source, kind);
    scheme.render_item(&CapcoMarking::from(attrs))
}

/// Render the banner (summary) form of a parsed banner input.
fn summary(scheme: &CapcoScheme, source: &str) -> String {
    let attrs = parse(scheme, source, MarkingType::Banner);
    scheme.render_summary(&CapcoMarking::from(attrs))
}

#[test]
fn render_item_byte_identical_golden() {
    let s = CapcoScheme::new();

    // (marking source, scan kind, expected exact portion-form bytes).
    // Expected bytes are the renderer's current canonical output — this
    // test pins them so the Phase-B render_portion -> render_item rename
    // (a pure method-name change delegating to render_canonical) is
    // provably output-preserving.
    let cases: &[(&str, MarkingType, &str)] = &[
        // Classification axis — §A.6 p15. UNCLASSIFIED renders empty in
        // canonical portion form, so it is not a content anchor; the
        // classified levels below are.
        ("(C)", MarkingType::Portion, "C"),
        ("(S)", MarkingType::Portion, "S"),
        ("(TS)", MarkingType::Portion, "TS"),
        // Dissem axis — NOFORN portion abbreviation.
        ("(S//NF)", MarkingType::Portion, "S//NF"),
        // SCI axis. Bare SCI implies RELIDO per §B.3 Table 2 p21, but that
        // closure is applied by the engine's project/closure layer, NOT by
        // render_canonical — so the pure render path emits the bare SCI
        // form (matches the trusted render_canonical_axis_fixtures.rs SCI
        // fixture, which renders bare SI without RELIDO).
        ("(TS//SI)", MarkingType::Portion, "TS//SI"),
        // REL TO axis — sorted release list.
        ("(S//REL TO USA, GBR)", MarkingType::Portion, "S//REL TO USA, GBR"),
        // Non-IC dissem — FOUO portion form.
        ("(S//FOUO)", MarkingType::Portion, "S//FOUO"),
    ];

    for (src, kind, expected) in cases {
        let got = item(&s, src, *kind);
        assert_eq!(
            &got, expected,
            "render_item byte drift for input {src:?}: expected {expected:?}, got {got:?}",
        );
    }
}

#[test]
fn render_summary_byte_identical_golden() {
    let s = CapcoScheme::new();

    // (banner source, expected exact banner-form bytes). See the note on
    // `render_item_byte_identical_golden`; this pins the same byte-identity
    // contract over the render_banner -> render_summary rename.
    let cases: &[(&str, &str)] = &[
        // Classification axis — §A.6 p15. UNCLASSIFIED renders empty.
        ("CONFIDENTIAL", "CONFIDENTIAL"),
        ("SECRET", "SECRET"),
        ("TOP SECRET", "TOP SECRET"),
        // Dissem axis — NOFORN banner form.
        ("SECRET//NOFORN", "SECRET//NOFORN"),
        // SCI axis. Bare SCI implies RELIDO per §B.3 Table 2 p21, but the
        // RELIDO closure is applied at the engine/project layer, not by
        // render_canonical; the pure render path emits the bare SCI form.
        ("TOP SECRET//SI", "TOP SECRET//SI"),
        // REL TO axis — sorted release list.
        ("SECRET//REL TO USA, GBR", "SECRET//REL TO USA, GBR"),
    ];

    for (src, expected) in cases {
        let got = summary(&s, src);
        assert_eq!(
            &got, expected,
            "render_summary byte drift for input {src:?}: expected {expected:?}, got {got:?}",
        );
    }
}
