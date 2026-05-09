// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T032 — closed-set field pin for [`marque_rules::MessageArgs`].
//!
//! The compile-fail proofs that inadmissible field shapes
//! (`String`-typed fields, `From<&str>` impls, etc.) do NOT compile
//! live as `compile_fail` doctests on the [`marque_rules::MessageArgs`]
//! type itself in `crates/rules/src/message.rs`. They run via
//! `cargo test --doc -p marque-rules`. This integration file pins the
//! complementary positive case: the destructuring of every permitted
//! field with a per-field type assertion, exercised from outside the
//! `marque-rules` crate so the test sees only the public surface.
//!
//! # The closure pattern (G13 / Constitution V Principle V)
//!
//! Adding a field to [`marque_rules::MessageArgs`] without updating
//! the destructuring below fails the build with E0027 (pattern does
//! not mention all fields). Removing a field fails with E0026 (no
//! such field). Either case forces a reviewer-visible attention loop
//! to either accept the field-set change (and update this test plus
//! `audit-record.md`) or revert.
//!
//! Together with the `compile_fail` doctests in `src/message.rs`,
//! the field-set is closed at the type level — no `String`,
//! `&str`, `Vec<u8>`, or `format!`-derived field can land without
//! a coordinated audit-schema change.

use marque_ism::Span;
use marque_rules::{Blake3Hash, Confidence, FeatureId, MessageArgs};
use marque_scheme::{CategoryId, TokenId};
use smallvec::SmallVec;

#[test]
fn message_args_field_set_pin_destructures_every_permitted_field() {
    let args = MessageArgs::default();
    let MessageArgs {
        token,
        category,
        span,
        digest,
        confidence,
        expected_token,
        actual_token,
        feature_ids,
    } = args;
    let _: Option<TokenId> = token;
    let _: Option<CategoryId> = category;
    let _: Option<Span> = span;
    let _: Option<Blake3Hash> = digest;
    let _: Option<Confidence> = confidence;
    let _: Option<TokenId> = expected_token;
    let _: Option<TokenId> = actual_token;
    let _: SmallVec<[FeatureId; 4]> = feature_ids;
}

#[test]
fn message_args_round_trips_each_permitted_field() {
    // Smoke test that each permitted field can hold its declared
    // value without surprising clobber semantics. Pairs with the
    // structural pin above. Constructed via struct-init rather than
    // field-by-field assignment to avoid `clippy::field_reassign_with_default`.
    let mut feature_ids: SmallVec<[FeatureId; 4]> = SmallVec::new();
    feature_ids.push(FeatureId::EditDistance1);
    let args = MessageArgs {
        token: Some(TokenId(1)),
        category: Some(CategoryId(2)),
        span: Some(Span::new(0, 4)),
        digest: Some(Blake3Hash::zero()),
        confidence: Some(Confidence::strict(0.9)),
        expected_token: Some(TokenId(3)),
        actual_token: Some(TokenId(4)),
        feature_ids,
    };
    assert_eq!(args.token, Some(TokenId(1)));
    assert_eq!(args.category, Some(CategoryId(2)));
    assert_eq!(args.expected_token, Some(TokenId(3)));
    assert_eq!(args.actual_token, Some(TokenId(4)));
    assert_eq!(args.feature_ids.as_slice(), &[FeatureId::EditDistance1]);
}
