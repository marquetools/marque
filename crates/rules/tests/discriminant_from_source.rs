// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pin for [`marque_rules::audit::discriminant_from_source`] — the
//! `FixSource → Discriminant` collapse table.
//!
//! This is the audit-emit-time mapping the CLI / WASM renderers
//! consume to project the `marque-3.0` `replacement.discriminant`
//! JSON field. Any future [`marque_rules::FixSource`] variant
//! addition must update [`marque_rules::audit::discriminant_from_source`]
//! and add a corresponding row here.

use marque_rules::FixSource;
use marque_rules::audit::{Discriminant, discriminant_from_source};

#[test]
fn maps_builtin_rule_to_strict() {
    // BuiltinRule fixes come from deterministic-parse rule logic;
    // strict-recognizer-only provenance.
    assert_eq!(
        discriminant_from_source(FixSource::BuiltinRule),
        Discriminant::Strict,
    );
}

#[test]
fn maps_migration_table_to_strict() {
    // MigrationTable fixes (E006 deprecation path) come from a
    // generated migration table at build time; still strict-path
    // — the table itself is canonical-vocabulary data.
    assert_eq!(
        discriminant_from_source(FixSource::MigrationTable),
        Discriminant::Strict,
    );
}

#[test]
fn maps_decoder_posterior_to_decoder() {
    // Decoder posterior fixes carry `confidence.recognition < 1.0`
    // and are the canonical "Decoder" wire-form arm.
    assert_eq!(
        discriminant_from_source(FixSource::DecoderPosterior),
        Discriminant::Decoder,
    );
}

#[test]
fn maps_decoder_classification_heuristic_to_decoder() {
    // The classification-heuristic decoder path is the second
    // decoder source; same Discriminant.
    assert_eq!(
        discriminant_from_source(FixSource::DecoderClassificationHeuristic),
        Discriminant::Decoder,
    );
}

#[test]
#[should_panic(expected = "AppliedTextCorrection")]
fn panics_on_corrections_map_source() {
    // FixSource::CorrectionsMap routes to AppliedTextCorrection
    // (separate NDJSON line type, no Discriminant). Reaching this
    // function with that source
    // means the engine bugged the promote-dispatch routing; we
    // panic loudly so the regression surfaces in CI rather than
    // emitting a wrong-shape audit record.
    let _ = discriminant_from_source(FixSource::CorrectionsMap);
}
