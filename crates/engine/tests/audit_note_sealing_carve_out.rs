// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration test exercising the `AuditNote::__engine_promote` sealing
//! pattern via the Constitution V Principle V test-fixture carve-out.
//!
//! ## Why this test exists
//!
//! `AuditNote` ships as a type + sealing surface; the production engine
//! construct-site is not yet wired (it will land alongside
//! `Engine::project::closure()`). The NDJSON renderer dispatch and
//! `"type"` discriminator already exist. Without a production
//! construct-site, an end-to-end audit_note line cannot be observed in
//! NDJSON output, so this test exercises the sealing pattern directly.
//!
//! It exercises the sealing pattern end-to-end with synthetic fixture
//! data:
//!
//! 1. Mint an `EnginePromotionToken` via `__engine_construct` (the test
//!    constructor).
//! 2. Construct an `AuditNote` via `__engine_promote` using realistic
//!    field values.
//! 3. Assert (a) the construction succeeds and (b) the fields round-trip
//!    through `Debug`.
//!
//! ## Constitution V Principle V test-fixture carve-out
//!
//! Three binding constraints apply (all satisfied here):
//!
//! 1. **`#[cfg(test)]` / `tests/` / `dev-dependencies` gating** — this
//!    file lives in `crates/engine/tests/`, the integration-test directory,
//!    so it compiles only under `cargo test` and never appears in
//!    production binaries.
//! 2. **No commingling with engine-promoted output** — the synthetic
//!    `AuditNote` constructed here is never spliced into a real audit
//!    stream. It exists in isolation to verify the sealing pattern's
//!    surface.
//! 3. **Test-fixture construction only** — this is fixture construction,
//!    not a "convenience helper for the engine"; the engine production
//!    construct-site will use the same `__engine_promote` call from
//!    inside `Engine::fix_inner` or similar.
//!
//! Each call site below carries an inline comment naming the carve-out.

use std::sync::Arc;
use std::time::SystemTime;

use marque_capco::CapcoScheme;
use marque_rules::{
    AuditNote, AuditNoteKind, AuditNoteStructural, EnginePromotionToken, Recognition, RuleId,
};
use marque_scheme::{Scope, SectionLetter, Span, TokenId, TokenRef, capco};

/// Smoke test: construct a synthetic `AuditNote` via the sealed
/// `__engine_promote` constructor and verify (a) construction succeeds,
/// (b) the fields round-trip through `Debug`.
#[test]
fn audit_note_engine_promote_sealing_pattern() {
    // Test-fixture carve-out per Constitution V Principle V — mint the
    // promotion token for synthetic AuditNote construction; never
    // commingled with engine output.
    let token = EnginePromotionToken::__engine_construct();

    let cone: &'static [TokenRef] = &[TokenRef::Token(TokenId(42)), TokenRef::Token(TokenId(99))];

    let structural = AuditNoteStructural {
        row_name: "capco/noforn-if-no-fdr",
        cone,
        scope: Scope::Page,
        span: Some(Span {
            start: 100,
            end: 120,
        }),
        suppressed_by: None, // InferredFact always has None per D19 A.
    };

    let timestamp = SystemTime::UNIX_EPOCH; // deterministic for snapshot
    // Use the allowlisted test sentinel (see marque/tests/no_classifier_id_in_commits.rs
    // ALLOWED_SENTINELS list — TEST-AUDIT-99 is the audit-test-scoped sentinel).
    let classifier_id: Option<Arc<str>> = Some(Arc::from("TEST-AUDIT-99"));

    // Recognition::strict() — strict-path recognizer, rule certainty 1.0.
    let confidence = Recognition::strict();

    // Test-fixture carve-out per Constitution V Principle V — synthetic
    // AuditNote construction exercising the __engine_promote seal;
    // verifies sealing pattern compiles + round-trips via Debug.
    let note = AuditNote::<CapcoScheme>::__engine_promote(
        // The fabricated AuditNote uses the reserved `"test"` scheme so
        // it is unambiguously a test fixture. The structural `row_name`
        // field below carries the slash form (a separate identifier
        // surface from `RuleId`).
        RuleId::new("test", "synthetic.audit-note-sealing-capco-fixture"),
        capco(SectionLetter::H, 8, 145),
        AuditNoteKind::InferredFact,
        timestamp,
        classifier_id.clone(),
        false, // dry_run
        structural,
        confidence,
        token,
    );

    // Round-trip via Debug — verifies all fields are reachable and the
    // formatter does not panic on edge cases.
    let debug = format!("{note:?}");
    assert!(debug.contains("capco/noforn-if-no-fdr"));
    assert!(debug.contains("InferredFact"));

    // Field-level spot checks.
    assert_eq!(note.citation, capco(SectionLetter::H, 8, 145));
    assert_eq!(note.kind, AuditNoteKind::InferredFact);
    assert!(!note.dry_run);
    assert!(note.classifier_id.is_some());
    assert_eq!(note.structural.row_name, "capco/noforn-if-no-fdr");
    assert_eq!(note.structural.scope, Scope::Page);
    assert_eq!(note.structural.span.unwrap().start, 100);
    // InferredFact never populates suppressed_by per D19 A.
    assert!(note.structural.suppressed_by.is_none());
}

/// Verifies `AuditNote<S>` is `Clone` — the manual `Clone` impl exists
/// for parity with `AppliedFix<S>` (which uses a manual impl to avoid
/// over-constraining `S: Clone`). Without this test, a future refactor
/// could accidentally drop the manual impl and the over-constraint would
/// silently propagate.
#[test]
fn audit_note_clone_does_not_require_scheme_clone() {
    // Test-fixture carve-out per Constitution V Principle V — same
    // carve-out as the sealing-pattern test above; this test exercises
    // the Clone surface.
    let token = EnginePromotionToken::__engine_construct();
    let cone: &'static [TokenRef] = &[TokenRef::Token(TokenId(1))];
    let structural = AuditNoteStructural {
        row_name: "test/clone",
        cone,
        scope: Scope::Portion,
        span: None,
        suppressed_by: None,
    };
    // Test-fixture carve-out per Constitution V Principle V — synthetic
    // AuditNote construction for the Clone-surface test below.
    let note: AuditNote<CapcoScheme> = AuditNote::__engine_promote(
        // Reserved `"test"` scheme + synthetic predicate id. The
        // `row_name` field below is a separate identifier surface.
        RuleId::new("test", "synthetic.audit-note-sealing-clone-fixture"),
        capco(SectionLetter::A, 1, 1),
        AuditNoteKind::InferredFact,
        SystemTime::UNIX_EPOCH,
        None,
        true,
        structural,
        Recognition::strict(),
        token,
    );

    let cloned = note.clone();
    assert_eq!(cloned.citation, note.citation);
    assert_eq!(cloned.structural.row_name, note.structural.row_name);
    assert_eq!(cloned.dry_run, note.dry_run);
    assert_eq!(cloned.kind, note.kind);
}

/// Verifies the forward-compat `suppressed_by` slot can be populated
/// for when a future `SuppressedByFact` `AuditNoteKind` variant lands.
/// Smoke test only — ensures the `Option<Box<[TokenId]>>` shape does
/// not break under non-`None` values.
#[test]
fn audit_note_structural_suppressed_by_slot_populated() {
    // Test-fixture carve-out per Constitution V Principle V — exercise
    // the M4 forward-compat slot without constructing a full AuditNote.
    let cone: &'static [TokenRef] = &[TokenRef::Token(TokenId(1))];
    let structural = AuditNoteStructural {
        row_name: "test/suppressed",
        cone,
        scope: Scope::Page,
        span: None,
        suppressed_by: Some(Box::from([TokenId(10), TokenId(20)])),
    };
    assert!(structural.suppressed_by.is_some());
    assert_eq!(structural.suppressed_by.as_ref().unwrap().len(), 2);
    assert_eq!(structural.suppressed_by.as_ref().unwrap()[0], TokenId(10));
    assert_eq!(structural.suppressed_by.as_ref().unwrap()[1], TokenId(20));
}
