// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T046 — PR 3c.2.D PM-D-4 seal-acceptance pin for
//! `AppliedTextCorrection::__engine_promote_text_correction`.
//!
//! Test-fixture carve-out per Constitution V Principle V — mints an
//! [`EnginePromotionToken`] via the same `__engine_construct` doorway
//! that the existing `engine_promotion_seal.rs` test exercises. The
//! purpose here is to prove the v2 type can be constructed through
//! its seal AND that `AppliedTextCorrection` carries the field-set
//! the audit-record contract requires (every closed-list permitted
//! identifier accessible by name).
//!
//! Mirrors `crates/rules/tests/engine_promotion_seal.rs` (the
//! marking-side seal pin); this file is the text-correction-side
//! counterpart.

use std::sync::Arc;
use std::time::SystemTime;

use marque_rules::{
    AppliedTextCorrection, Blake3Hash, Confidence, EnginePromotionToken, FixSource, Message,
    MessageArgs, MessageTemplate, RuleId,
};
use marque_scheme::{Severity, Span};
use smol_str::SmolStr;

#[test]
fn applied_text_correction_promote_seal_constructs_through_token() {
    // Test-fixture carve-out per Constitution V Principle V:
    // mint a token, construct an AppliedTextCorrection, assert the
    // field set. Never commingled with a real engine audit stream.
    // Test-fixture carve-out per Constitution V Principle V.
    let token = EnginePromotionToken::__engine_construct();

    let original_digest: Blake3Hash = Blake3Hash::from([0u8; 32]);
    let correction = AppliedTextCorrection::__engine_promote_text_correction(
        RuleId::new("C001"),
        Severity::Fix,
        Span::new(0, 6),
        original_digest,
        SmolStr::new("SECRET"),
        FixSource::CorrectionsMap,
        Confidence::strict(1.0),
        None,
        Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
        SystemTime::UNIX_EPOCH,
        Some(Arc::from("test-classifier")),
        false,
        Some(Arc::from("/tmp/file.txt")),
        token,
    );

    assert_eq!(correction.rule, RuleId::new("C001"));
    assert_eq!(correction.severity, Severity::Fix);
    assert_eq!(correction.span, Span::new(0, 6));
    assert_eq!(correction.replacement.as_str(), "SECRET");
    assert_eq!(correction.source, FixSource::CorrectionsMap);
    assert!(!correction.dry_run);
    assert!(correction.classifier_id.is_some());
    assert!(correction.migration_ref.is_none());
}

#[test]
fn applied_text_correction_clone_preserves_fields() {
    // Pin the manual `Clone` impl — every field round-trips.
    // Test-fixture carve-out per Constitution V Principle V.
    let token = EnginePromotionToken::__engine_construct();
    let original_digest: Blake3Hash = Blake3Hash::from([1u8; 32]);
    let original = AppliedTextCorrection::__engine_promote_text_correction(
        RuleId::new("E006"),
        Severity::Error,
        Span::new(10, 16),
        original_digest,
        SmolStr::new("CUI"),
        FixSource::MigrationTable,
        Confidence::strict(0.8),
        Some("§F.1 p41"),
        Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
        SystemTime::UNIX_EPOCH,
        None,
        true,
        None,
        token,
    );

    let cloned = original.clone();
    assert_eq!(cloned.rule, original.rule);
    assert_eq!(cloned.severity, original.severity);
    assert_eq!(cloned.span, original.span);
    assert_eq!(cloned.replacement, original.replacement);
    assert_eq!(cloned.source, original.source);
    assert_eq!(cloned.migration_ref, original.migration_ref);
    assert_eq!(cloned.dry_run, original.dry_run);
    assert_eq!(cloned.original_digest, original.original_digest);
}
