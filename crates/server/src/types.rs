// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use serde::{Deserialize, Deserializer, Serialize};

/// Records whether a JSON key was present, without materializing or
/// examining the value.
///
/// Used by the T3 body-field guard. `#[serde(default)]` on the field
/// means an absent key deserializes as `PresenceMarker(false)`; any
/// present key — including `null`, `{}`, `[]`, numbers, strings —
/// runs the `Deserialize` impl, which consumes the value via
/// `IgnoredAny` (never stored, never logged) and returns
/// `PresenceMarker(true)`.
///
/// This matches the contract wording "Any such field is rejected
/// with 400" more precisely than `Option<IgnoredAny>` would, because
/// the latter cannot distinguish an absent key from an explicit
/// `null` value.
#[derive(Default)]
struct PresenceMarker(bool);

impl PresenceMarker {
    fn is_present(&self) -> bool {
        self.0
    }
}

impl<'de> Deserialize<'de> for PresenceMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Consume the value without storing it. `IgnoredAny` accepts
        // any JSON shape — including `null`, objects, arrays — so
        // presence of the key alone is the observable signal.
        serde::de::IgnoredAny::deserialize(deserializer)?;
        Ok(PresenceMarker(true))
    }
}

#[derive(Deserialize)]
pub struct LintRequest {
    pub text: String,
    /// Calling context hint — affects scanner heuristics.
    #[allow(dead_code)]
    pub context: Option<String>,
    /// T3 guard: if the key is present (regardless of value), the
    /// handler rejects with 400. `PresenceMarker` records key presence
    /// without deserializing or storing the payload, so even
    /// `"corpus_override": null` still trips the guard — matching the
    /// contract's "any such field is rejected" wording.
    #[serde(default, rename = "corpus_override")]
    _corpus_override: PresenceMarker,
}

impl LintRequest {
    pub(crate) fn carries_corpus_override(&self) -> bool {
        self._corpus_override.is_present()
    }
}

#[derive(Serialize)]
pub struct LintResponse {
    pub diagnostics: Vec<DiagnosticJson>,
    pub error_count: usize,
    pub warn_count: usize,
    pub fix_count: usize,
    /// Spec 005 §R3 — `true` when the engine aborted the lint pass
    /// because the per-request deadline expired. Older clients that
    /// do not deserialize unknown fields will silently ignore this;
    /// new clients should pair it with the `Marque-Truncated`
    /// response header (set on the wire-level shell).
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// Number of candidate spans whose rule pass started before the
    /// deadline tripped. On a non-truncated response, equals
    /// `candidates_total`.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub candidates_processed: usize,
    /// Total candidate spans the scanner produced for this document.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub candidates_total: usize,
}

fn is_false(b: &bool) -> bool {
    !*b
}
fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

#[derive(Serialize)]
pub struct DiagnosticJson {
    pub rule_id: String,
    pub severity: String,
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub fix: Option<FixJson>,
}

#[derive(Serialize)]
pub struct FixJson {
    /// Provenance of the fix — `"BuiltinRule" | "CorrectionsMap" |
    /// "MigrationTable" | "DecoderPosterior" |
    /// "DecoderClassificationHeuristic"`. Mirrors the CLI/WASM
    /// `source` field.
    pub source: &'static str,
    /// The kind of fix payload — `"FactAdd" | "FactRemove" |
    /// "Recanonicalize"` for structural rule fixes, `"TextCorrection"`
    /// for byte-substitution fixes (the corrections-map / migration
    /// channel). Mirrors the CLI and WASM diagnostic JSON shape.
    pub intent_kind: &'static str,
    /// Replacement bytes, present only for `TextCorrection` payloads.
    /// `None` for structural-intent fixes (the engine synthesizes the
    /// canonical bytes at fix-application time via `apply_intent` +
    /// `render_canonical`; the server response carries only the
    /// structural commitment, not the materialized bytes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    pub confidence: f32,
    pub migration_ref: Option<String>,
}

#[derive(Deserialize)]
pub struct FixRequest {
    pub text: String,
    /// Optional per-request override of the engine's confidence threshold.
    /// When `None`, the engine uses its configured value. When `Some`, the
    /// value is validated against `[0.0, 1.0]` and a 422 is returned on
    /// invalid input.
    pub confidence_threshold: Option<f32>,
    /// T3 guard: see `LintRequest::_corpus_override`.
    #[serde(default, rename = "corpus_override")]
    _corpus_override: PresenceMarker,
}

impl FixRequest {
    pub(crate) fn carries_corpus_override(&self) -> bool {
        self._corpus_override.is_present()
    }
}

#[derive(Serialize)]
pub struct FixResponse {
    pub fixed_text: String,
    pub applied_count: usize,
    pub remaining_diagnostics: usize,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub schema_version: &'static str,
}

/// JSON body for a 504 deadline-exceeded fix response.
///
/// `truncated_by` distinguishes which phase tripped the deadline:
/// `"lint"` if the lint pass itself aborted (the engine never
/// reached the fix loop), `"fix"` if the lint pass completed and the
/// fix-application loop was the one that ran out of time.
#[derive(Serialize)]
pub struct DeadlineExceededBody {
    pub truncated_by: &'static str,
    pub diagnostics: Vec<DiagnosticJson>,
    pub error_count: usize,
    pub warn_count: usize,
    pub fix_count: usize,
    pub candidates_processed: usize,
    pub candidates_total: usize,
}
