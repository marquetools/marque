// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #699 — cross-channel content-ignorance asymmetry pin for
//! `recognized_canonical`.
//!
//! The lint surface DOES carry the decoder-recognized canonical
//! bytes (so `marque check` can show users the recognized form
//! without running `marque fix` and diffing the output). The audit
//! surface does NOT — it carries the BLAKE3 digest of the canonical
//! bytes plus the structural `Recanonicalize` intent, never the
//! bytes themselves (Constitution V Principle V —
//! audit content-ignorance).
//!
//! This file pins the asymmetry. A regression that leaked
//! `recognized_canonical` into the audit envelope (or that dropped
//! it from the lint surface) trips this test, not the content-ignorance
//! canary corpus sweep alone.
//!
//! The CLI renderer (`marque::render::audit_line_to_json_v1_0`)
//! lives in a `[[bin]]`-only crate that integration tests cannot
//! reach. We mirror the projection inline — same pattern as
//! `audit_g13_canary.rs::render_audit_line_to_json`. Structural
//! drift between the inline projection and the CLI emit would
//! surface as a separate test failure in `audit_v3_0_parity.rs`
//! (the inline projection scans the bytes we'd otherwise rely on
//! the CLI to emit).

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{CapcoEngine, FixMode};
use marque_rules::FixSource;
use marque_rules::MessageTemplate;
use marque_rules::RuleSet;
use marque_rules::audit::{AuditLine, discriminant_from_source};
use secrecy::ExposeSecret as _;
use serde_json::json;

fn build_engine_threshold_zero() -> CapcoEngine {
    let mut config = Config::default();
    config
        .set_confidence_threshold(0.0)
        .expect("0.0 is a valid threshold");
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    CapcoEngine::new(config, rule_sets, CapcoScheme::new())
        .expect("default CAPCO scheme constructs without rewrite cycles")
}

/// Mirror of the CLI / WASM v1.0 audit-record JSON projection,
/// produced inline because `marque::render::audit_line_to_json_v1_0`
/// lives in a `[[bin]]`-only crate. Matches the shape produced by
/// `audit_g13_canary.rs::render_audit_line_to_json` so a future
/// content-leak channel surfaces identically in both tests.
fn render_audit_line(line: &AuditLine<CapcoScheme>) -> Option<String> {
    let rule_id_json = |r: &marque_rules::RuleId| {
        json!({
            "scheme": r.scheme(),
            "predicate_id": r.predicate_id(),
        })
    };
    let v = match line {
        AuditLine::AppliedFix(f) => json!({
            "type": "applied_fix",
            "schema": marque_engine::AUDIT_SCHEMA_VERSION,
            "rule": rule_id_json(&f.rule),
            "severity": f.severity.as_str(),
            "span": {"start": f.span.start, "end": f.span.end},
            "fix": {
                "replacement": {
                    "discriminant": discriminant_from_source(f.source).as_str(),
                    "canonical": {
                        "bytes_digest": format!("blake3:{}", f.fix.replacement.bytes_digest.to_hex()),
                        "category": format!("{:?}", f.fix.replacement.canonical.source()),
                    },
                    "confidence": {
                        "recognition": f.fix.replacement.confidence.recognition,
                    },
                },
                "original_span": {"start": f.fix.original_span.start, "end": f.fix.original_span.end},
                "original_digest": format!("blake3:{}", f.fix.original_digest.to_hex()),
            },
            "message": {"template": f.message.template().as_str()},
            "timestamp": humantime::format_rfc3339(f.timestamp).to_string(),
            "classifier_id": f.classifier_id.as_deref(),
            "dry_run": f.dry_run,
        }),
        AuditLine::TextCorrection(tc) => json!({
            "type": "text_correction",
            "schema": marque_engine::AUDIT_SCHEMA_VERSION,
            "rule": rule_id_json(&tc.rule),
            "severity": tc.severity.as_str(),
            "span": {"start": tc.span.start, "end": tc.span.end},
            "original_digest": format!("blake3:{}", tc.original_digest.to_hex()),
            "replacement": tc.replacement.as_str(),
            "source": match tc.source {
                FixSource::CorrectionsMap => "corrections_map",
                FixSource::BuiltinRule => "builtin_rule",
                FixSource::MigrationTable => "migration_table",
                FixSource::DecoderPosterior => "decoder_posterior",
                FixSource::DecoderClassificationHeuristic => "decoder_classification_heuristic",
            },
            "message": {"template": tc.message.template().as_str()},
            "timestamp": humantime::format_rfc3339(tc.timestamp).to_string(),
            "classifier_id": tc.classifier_id.as_deref(),
            "dry_run": tc.dry_run,
        }),
        // `AuditLine` is `#[non_exhaustive]`; future variants land
        // here. Returning `None` mirrors the canary's policy — the
        // parallel-update requirement keeps the three renderer sites
        // in lockstep (see `audit_g13_canary.rs` doc-comment).
        _ => return None,
    };
    serde_json::to_string(&v).ok()
}

#[test]
fn lint_carries_recognized_canonical_fix_audit_does_not() {
    // `(TS//SAR-fk)` — drives R001 at Severity::Fix under the
    // zero-threshold engine. Lint produces a diagnostic carrying the
    // canonical bytes via `recognized_canonical`; fix produces an
    // AppliedFix whose audit-record JSON carries the BLAKE3 digest
    // of the canonical bytes but NOT the bytes themselves.
    //
    // The substring guard at the end is the test's load-bearing
    // assertion: a regression that adds the canonical bytes to the
    // audit envelope (a future careless field-add, a copy-paste of
    // the lint renderer into the audit renderer) makes the bytes
    // literally appear in the rendered NDJSON line. The content-ignorance
    // canary corpus sweep catches the same regression on a wider corpus;
    // this test catches it on the specific R001 surface in isolation.
    let input = b"(TS//SAR-fk)";
    let canonical: &[u8] = b"(TS//SAR-FK)";

    // --- Lint side: recognized_canonical IS populated ---
    let engine = build_engine_threshold_zero();
    let lint = engine.lint(input);
    let r001 = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "recognition.decoder-recognized")
        .expect("R001 must fire on SAR lowercase program id");
    assert_eq!(
        r001.message.template(),
        MessageTemplate::DecoderRecognized,
        "R001 lint diagnostic must use DecoderRecognized template",
    );
    let recognized = r001
        .recognized_canonical
        .as_ref()
        // Principle II readout — lint-side content-ignorance pin (issue #699).
        // This call is the load-bearing "lint surface carries the
        // bytes" half of the cross-channel asymmetry; the negative
        // half (audit envelope contains only the digest) is asserted
        // below.
        .map(|sb| sb.expose_secret().to_vec())
        .expect("R001 must carry recognized_canonical");
    assert_eq!(
        recognized.as_slice(),
        canonical,
        "lint-side surface must carry the canonical bytes literally",
    );

    // --- Fix side: AppliedFix JSON carries digest, NOT bytes ---
    let fix = engine.fix(input, FixMode::DryRun);
    let r001_audit = fix
        .audit_lines
        .iter()
        .find(|line| matches!(line, AuditLine::AppliedFix(a) if a.rule.predicate_id() == "recognition.decoder-recognized"))
        .expect("R001 must produce an AppliedFix audit line under threshold=0");
    let applied_template = match r001_audit {
        AuditLine::AppliedFix(a) => a.message.template(),
        _ => unreachable!("search above guarantees AppliedFix"),
    };
    assert_eq!(
        applied_template,
        MessageTemplate::DecoderRecognized,
        "R001 audit AppliedFix must use DecoderRecognized template",
    );
    assert_eq!(
        applied_template,
        r001.message.template(),
        "lint and audit template labels must agree for R001",
    );

    let audit_ndjson = render_audit_line(r001_audit).expect("audit line must serialize to NDJSON");

    // Positive guard: the BLAKE3 digest of the canonical bytes
    // appears verbatim in the audit NDJSON. Construct the prefix-
    // tagged digest directly so the assertion is byte-precise.
    let canonical_hash = blake3::hash(canonical);
    let digest_wire = format!("blake3:{}", canonical_hash.to_hex());
    assert!(
        audit_ndjson.contains(&digest_wire),
        "audit NDJSON must carry the BLAKE3 digest of the canonical \
         bytes (Constitution V Principle V — permitted-identifier \
         list); got: {audit_ndjson}",
    );

    // Negative guard: the canonical bytes themselves MUST NOT
    // appear verbatim in the audit NDJSON. This is the
    // content-ignorance pin specific to issue #699 — the lint
    // surface carries the bytes; the audit surface carries only
    // the digest + structural intent.
    let canonical_utf8 = std::str::from_utf8(canonical).expect("canonical must be UTF-8");
    assert!(
        !audit_ndjson.contains(canonical_utf8),
        "audit NDJSON must NOT contain the canonical bytes verbatim \
         (Constitution V Principle V / audit content-ignorance); got: {audit_ndjson}",
    );
}
