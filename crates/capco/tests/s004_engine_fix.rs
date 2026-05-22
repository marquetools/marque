#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! S004 — Engine-fix suggest-don't-fix invariant.
//!
//! Pin the suggest-don't-fix invariant end-to-end: even though
//! S004 emits a `FixProposal`, running `Engine::fix` (the API
//! that produces audit records) must NOT include the S004 fix
//! in `applied`. The engine excludes Suggest-severity from
//! auto-apply by construction.
//!
//! This test lived in `crates/capco/src/rules.rs::tests` until
//! PR 3c.B Commit 2. The relocation was forced by the
//! `marque-capco` ↔ `marque-engine` dev-dep cycle: post-Commit-2,
//! `Engine` consumes `CapcoScheme` through a generic-typed
//! `MarkingScheme` bound, and the dev-dep cycle compiles two
//! distinct `CapcoScheme` instances when an in-lib test tries to
//! construct an `Engine` directly. Integration tests in
//! `crates/capco/tests/` see a single coherent `marque-capco` and
//! compile cleanly.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::RuleSet;

#[test]
fn s004_fix_does_not_auto_apply_under_engine_fix_call() {
    let config = Config::default();
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    let engine = Engine::new(config, rule_sets, CapcoScheme::new())
        .expect("default scheme has no rewrite cycles");

    let result = engine.fix(b"SECRET//REL TO USA, AUT, GBR\n", FixMode::Apply);
    // No S004-rule audit record may exist.
    let s004_audits: Vec<_> = result
        .applied
        .iter()
        .filter(|af| af.rule.predicate_id() == "portion.dissem.rel-to-trigraph-suggest")
        .collect();
    assert!(
        s004_audits.is_empty(),
        "S004 must never produce an AppliedFix; got: {s004_audits:?}"
    );
}
