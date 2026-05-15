// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Confidence — Phase D audit-provenance payload.
//!
//! Every [`FixProposal`](crate::FixProposal) carries a `Confidence`
//! record describing how the engine arrived at the proposal. The
//! record stores two primary scalar confidence axes —
//! `recognition` and `rule` — plus optional auxiliary fields
//! (`region` and `runner_up_ratio`) and a list of named feature
//! contributions. Together they reconstruct the decoder's scoring
//! path so an auditor can verify *why* a given fix was promoted.
//!
//! The engine's current threshold-facing combined score is
//! `recognition * rule` as exposed by [`Confidence::combined`].
//! `region` is recorded as additional audit/context metadata when
//! available, but it does not currently participate in that
//! combined score. `runner_up_ratio` likewise provides decoder
//! provenance rather than a direct multiplicative/additive input to
//! `combined()`.
//!
//! ## Precision: `f32` throughout
//!
//! All scores are `f32`. The decoder scores in `f64` internally
//! (log-priors and posteriors accumulate across many features), but
//! the emitted `Confidence` downcasts once at the boundary so the
//! audit record stays compact and byte-stable. This matches the
//! foundational-plan invariant line 739-757.
//!
//! ## `features` is closed
//!
//! [`FeatureId`] is a non-`#[non_exhaustive]` closed enum. The
//! on-the-wire contract is the `as_str()` table plus the
//! `feature_id_as_str_matches_audit_contract` pinned-strings test;
//! both update in lock-step when a variant is added.
//!
//! Pre-1.0 the project carries no downstream audit-record
//! consumers, so the `MARQUE_AUDIT_SCHEMA` pin (in
//! `crates/engine/build.rs`) is performative — extending
//! `FeatureId` does not currently require a schema bump.
//!
//! **TODO(marque-1.0)**: re-tighten the schema-bump contract
//! before GA. The atomic cutover lives in PR 3c.2 of the
//! engine + rule architecture refactor (see CLAUDE.md "PR 3c.2
//! carved out + `marque-1.0` deferral"); the four structural
//! commitments that land there (Canonical wired into audit
//! emit, BLAKE3 audit-record digesting, closed
//! `MessageTemplate` JSON serialization, `from_parsed_unchecked`
//! adapter deletion) include the audit-schema accept-list
//! cutover. After PR 3c.2 the accept-list becomes the single
//! source of truth and the doc comment above MUST be rewritten
//! to "any new variant requires a coordinated schema bump."
//!
//! ## `features` storage
//!
//! `Confidence::features` is a `SmallVec<[FeatureContribution; 4]>`.
//! Strict-path fixes record zero features and never allocate; decoder-
//! path fixes record 1–4 features per the empirical distribution of
//! the corpus, which fits inline. The inline-4 bound matches the
//! existing `MessageArgs::feature_ids` / `FixIntent::feature_ids`
//! pattern — same cardinality, same audit-record proximity. The
//! `SmallVec` storage is an implementation detail; the field iterates
//! and indexes the same as a `Vec`, so consumers that only read the
//! contributions are unaffected. Struct-literal construction must use
//! `SmallVec::new()` or the [`smallvec!`] macro (the rules crate root
//! re-exports both so external callers do not need their own
//! `smallvec` dep).

use smallvec::SmallVec;

/// Multi-axis confidence attached to every [`FixProposal`](crate::FixProposal).
///
/// Fields:
///
/// - `recognition` — posterior from the [`Recognizer`](marque_scheme::Recognizer)
///   that surfaced this candidate (0.0–1.0).
/// - `rule` — confidence the emitting rule has in its own fix
///   (0.0–1.0). Strict-path rules report 1.0 when the invariant is
///   unambiguous.
/// - `region` — optional region-level confidence (a page-context
///   prior, for example).
/// - `runner_up_ratio` — optional ratio of top candidate to runner-up
///   posterior. Decoder-sourced fixes carry this; strict-path fixes
///   leave it `None` because the strict grammar has no runner-up by
///   construction.
/// - `features` — the concrete evidence features that contributed to
///   `recognition`. Used by the corpus-accuracy harness to break down
///   where posterior mass came from.
///
/// Construction happens via [`Confidence::strict`] (for rules that
/// bypass the decoder) or the decoder's scoring path (Phase 4 / task
/// T061).
#[derive(Debug, Clone, PartialEq)]
pub struct Confidence {
    /// Recognizer posterior in `[0.0, 1.0]`.
    pub recognition: f32,
    /// Rule-level confidence in `[0.0, 1.0]`.
    pub rule: f32,
    /// Region / page-context confidence, when a rule computes one.
    pub region: Option<f32>,
    /// Posterior ratio between top candidate and runner-up
    /// (`None` for strict-path fixes; set by decoder-sourced fixes).
    pub runner_up_ratio: Option<f32>,
    /// Per-feature contributions to `recognition`.
    ///
    /// Stored as `SmallVec<[FeatureContribution; 4]>` so the inline-4
    /// case is heap-free. See the module-level docs for the inline-N
    /// rationale.
    pub features: SmallVec<[FeatureContribution; 4]>,
}

impl Confidence {
    /// Confidence record for a strict-path fix where recognition was
    /// unambiguous.
    ///
    /// `rule_confidence` is the rule's own confidence in its proposed
    /// fix (typically 1.0 for migrations, lower for heuristics). The
    /// recognition axis is pinned at 1.0 because the strict grammar
    /// has one unambiguous match by definition, and no feature
    /// contributions are recorded — strict-path fixes do not traverse
    /// the decoder's feature graph.
    #[inline]
    pub fn strict(rule_confidence: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&rule_confidence) && !rule_confidence.is_nan(),
            "Confidence::strict rule confidence must be in [0.0, 1.0] and not NaN, got {rule_confidence}"
        );
        Self {
            recognition: 1.0,
            rule: rule_confidence,
            region: None,
            runner_up_ratio: None,
            features: SmallVec::new(),
        }
    }

    /// Product of `recognition` and `rule`. The engine's
    /// confidence-threshold gate compares this combined score against
    /// the configured threshold (FR-016).
    #[inline]
    pub fn combined(&self) -> f32 {
        self.recognition * self.rule
    }

    /// Validate every axis of this `Confidence` record.
    ///
    /// Returns `Err(message)` naming the first invalid axis. Checks:
    ///
    /// - `recognition` and `rule` in `[0.0, 1.0]` and not NaN.
    /// - `region`, when `Some`, in `[0.0, 1.0]` and not NaN.
    /// - `runner_up_ratio`, when `Some`, finite and not NaN. No range
    ///   constraint — a well-behaved decoder returns `≥ 1.0` (top /
    ///   runner-up) but infinity (runner-up posterior = 0) and values
    ///   `< 1.0` are legal for debugging / inspection code.
    /// - Every `features[i].delta` finite and not NaN. `delta` carries
    ///   signed log-posterior contributions so any finite value is
    ///   legal; `NaN` / infinity would poison downstream audit-sum
    ///   invariants silently.
    ///
    /// The zero-axis edge case (recognition = 0 or rule = 0) is valid
    /// — `combined() = 0.0` is a legitimate below-threshold result,
    /// not an invariant violation.
    pub fn validate(&self) -> Result<(), String> {
        let check_unit = |label: &str, v: f32| -> Result<(), String> {
            if v.is_nan() || !(0.0..=1.0).contains(&v) {
                Err(format!(
                    "Confidence.{label} must be in [0.0, 1.0] and not NaN, got {v}"
                ))
            } else {
                Ok(())
            }
        };
        let check_finite = |label: &str, v: f32| -> Result<(), String> {
            if v.is_nan() || !v.is_finite() {
                Err(format!(
                    "Confidence.{label} must be finite and not NaN, got {v}"
                ))
            } else {
                Ok(())
            }
        };

        check_unit("recognition", self.recognition)?;
        check_unit("rule", self.rule)?;
        if let Some(r) = self.region {
            check_unit("region", r)?;
        }
        if let Some(r) = self.runner_up_ratio {
            check_finite("runner_up_ratio", r)?;
        }
        for (i, feature) in self.features.iter().enumerate() {
            if feature.delta.is_nan() || !feature.delta.is_finite() {
                return Err(format!(
                    "Confidence.features[{i}].delta must be finite and not NaN, got {}",
                    feature.delta
                ));
            }
        }
        Ok(())
    }
}

/// One named contribution to [`Confidence::recognition`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureContribution {
    /// Which feature.
    pub id: FeatureId,
    /// Signed delta added to the log-posterior by this feature.
    pub delta: f32,
}

/// Closed enumeration of features the decoder can record.
///
/// Adding ANY variant requires a coordinated bump of
/// `MARQUE_AUDIT_SCHEMA` (in `crates/engine/build.rs`) once Marque has
/// audit-record consumers. Pre-1.0, the audit-schema pin is performative
/// — there are no downstream readers yet — so the contract is the
/// `as_str()` mapping plus the `feature_id_as_str_matches_audit_contract`
/// pinned-strings test, both updated in lock-step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureId {
    /// Observed form is edit-distance 1 from a canonical token.
    EditDistance1,
    /// Observed form is edit-distance 2 from a canonical token.
    EditDistance2,
    /// Observed form is a token-order permutation of a canonical
    /// banner/portion shape.
    TokenReorder,
    /// Observed form is a known CAPCO-2016-superseded token whose
    /// replacement is unambiguous (e.g., `COMINT → SI`).
    SupersededToken,
    /// The candidate's base rate in the target corpus dominates the
    /// posterior (common-marking prior).
    BaseRateCommonMarking,
    /// Strict-context classification floor (FR-011) applied — e.g.,
    /// banner at TOP SECRET forces a strict posterior for
    /// classification tokens at ≥ that level on the same page.
    StrictContextClassification,
    /// Corpus-override data (opt-in, non-WASM, non-server) shifted
    /// the posterior. Recorded so an auditor can identify fixes
    /// produced under organizational overrides vs. stock priors.
    CorpusOverrideInEffect,
    /// Portion candidate appears more than a short prefix into a line
    /// whose preceding bytes do not look like a bullet or section
    /// anchor. Real portion markings nearly always appear at the very
    /// start of a line, after a bullet (`* `, `- `, `• `), or after
    /// an enumeration anchor (`1.`, `a)`, `1B.a.3.`); a portion-shaped
    /// `(x)` deep inside a line of running text is overwhelmingly a
    /// prose glyph (parenthetical, plural, copyright). Negative delta.
    LinePositionPenalty,
    /// Portion candidate's same-line preceding bytes look like a
    /// bullet or section anchor (`1B.a.3.`, `(a)`, `* `, `- `, ...).
    /// Cancels the line-position penalty so legitimate enumeration
    /// patterns common in IC/legal documents are not suppressed.
    /// Positive delta.
    BulletAnchorBonus,
    /// Candidate contains lowercase letters AND the surrounding
    /// document context is lowercase-dominant. Banner-form markings
    /// are explicitly required to be uppercase (CAPCO-2016 §D.1
    /// p27: "The banner line must be in uppercase letters"); portion
    /// form is silent in the manual but universally uppercase in
    /// practice. A lowercase candidate inside lowercase prose is
    /// overwhelmingly prose, not a mangled marking the decoder
    /// should recover. Negative delta.
    LowercaseSurroundingContext,
}

impl FeatureId {
    /// Canonical on-the-wire string label for this feature.
    ///
    /// This is the **single source of truth** for `FeatureId →
    /// audit-record-string` projection. Audit emitters (CLI, WASM,
    /// server) and snapshot tests MUST call this method rather than
    /// re-implementing the match. A new `FeatureId` variant added
    /// without a matching `as_str` arm fails the exhaustiveness check
    /// here at compile time, so the on-the-wire contract cannot drift
    /// silently across emitters.
    ///
    /// Returns a `&'static str` so callers can embed the value in
    /// zero-copy serialization paths (`Serialize` derives,
    /// `serde_json::json!` etc.) without an allocation.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            FeatureId::EditDistance1 => "EditDistance1",
            FeatureId::EditDistance2 => "EditDistance2",
            FeatureId::TokenReorder => "TokenReorder",
            FeatureId::SupersededToken => "SupersededToken",
            FeatureId::BaseRateCommonMarking => "BaseRateCommonMarking",
            FeatureId::StrictContextClassification => "StrictContextClassification",
            FeatureId::CorpusOverrideInEffect => "CorpusOverrideInEffect",
            FeatureId::LinePositionPenalty => "LinePositionPenalty",
            FeatureId::BulletAnchorBonus => "BulletAnchorBonus",
            FeatureId::LowercaseSurroundingContext => "LowercaseSurroundingContext",
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn strict_pins_recognition_at_one() {
        let c = Confidence::strict(0.85);
        assert_eq!(c.recognition, 1.0);
        assert_eq!(c.rule, 0.85);
        assert!(c.region.is_none());
        assert!(c.runner_up_ratio.is_none());
        assert!(c.features.is_empty());
    }

    #[test]
    fn combined_is_product_of_axes() {
        let c = Confidence::strict(0.9);
        assert!((c.combined() - 0.9).abs() < 1e-6);

        let c2 = Confidence {
            recognition: 0.8,
            rule: 0.5,
            region: None,
            runner_up_ratio: None,
            features: SmallVec::new(),
        };
        assert!((c2.combined() - 0.4).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "Confidence::strict rule confidence")]
    fn strict_panics_on_nan() {
        let _ = Confidence::strict(f32::NAN);
    }

    #[test]
    #[should_panic(expected = "Confidence::strict rule confidence")]
    fn strict_panics_above_one() {
        let _ = Confidence::strict(1.01);
    }

    #[test]
    fn feature_id_as_str_matches_audit_contract() {
        // Pin the on-the-wire labels for `FeatureId`. These strings are
        // part of the audit-record contract (see
        // `contracts/audit-record-v2.md`); a future rename here MUST be
        // a deliberate audit-schema bump (`MARQUE_AUDIT_SCHEMA`), not an
        // accidental refactor. Kept as an explicit per-variant table
        // (rather than a round-trip) so a label drift is loud.
        let cases: &[(FeatureId, &str)] = &[
            (FeatureId::EditDistance1, "EditDistance1"),
            (FeatureId::EditDistance2, "EditDistance2"),
            (FeatureId::TokenReorder, "TokenReorder"),
            (FeatureId::SupersededToken, "SupersededToken"),
            (FeatureId::BaseRateCommonMarking, "BaseRateCommonMarking"),
            (
                FeatureId::StrictContextClassification,
                "StrictContextClassification",
            ),
            (FeatureId::CorpusOverrideInEffect, "CorpusOverrideInEffect"),
            (FeatureId::LinePositionPenalty, "LinePositionPenalty"),
            (FeatureId::BulletAnchorBonus, "BulletAnchorBonus"),
            (
                FeatureId::LowercaseSurroundingContext,
                "LowercaseSurroundingContext",
            ),
        ];
        for (id, expected) in cases {
            assert_eq!(id.as_str(), *expected, "label drift for {id:?}");
        }
    }

    #[test]
    fn feature_contribution_roundtrip() {
        let fc = FeatureContribution {
            id: FeatureId::EditDistance1,
            delta: -0.3,
        };
        assert_eq!(fc.id, FeatureId::EditDistance1);
        assert!((fc.delta - (-0.3)).abs() < 1e-6);
    }

    #[test]
    fn validate_accepts_well_formed_record() {
        assert!(Confidence::strict(0.85).validate().is_ok());
        assert!(
            Confidence {
                recognition: 0.9,
                rule: 0.8,
                region: Some(0.5),
                runner_up_ratio: Some(2.7),
                features: smallvec::smallvec![FeatureContribution {
                    id: FeatureId::EditDistance1,
                    delta: -0.5,
                }],
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn validate_rejects_out_of_range_recognition() {
        let c = Confidence {
            recognition: 1.5,
            rule: 0.5,
            region: None,
            runner_up_ratio: None,
            features: SmallVec::new(),
        };
        let err = c.validate().unwrap_err();
        assert!(
            err.contains("recognition"),
            "error should name the offending axis, got: {err}"
        );
    }

    #[test]
    fn validate_rejects_out_of_range_rule() {
        let c = Confidence {
            recognition: 0.5,
            rule: -0.1,
            region: None,
            runner_up_ratio: None,
            features: SmallVec::new(),
        };
        let err = c.validate().unwrap_err();
        assert!(err.contains("rule"), "got: {err}");
    }

    #[test]
    fn validate_rejects_out_of_range_region() {
        let c = Confidence {
            recognition: 0.5,
            rule: 0.5,
            region: Some(1.5),
            runner_up_ratio: None,
            features: SmallVec::new(),
        };
        let err = c.validate().unwrap_err();
        assert!(err.contains("region"), "got: {err}");
    }

    #[test]
    fn validate_rejects_non_finite_runner_up_ratio() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let c = Confidence {
                recognition: 0.5,
                rule: 0.5,
                region: None,
                runner_up_ratio: Some(bad),
                features: SmallVec::new(),
            };
            assert!(
                c.validate().is_err(),
                "runner_up_ratio = {bad:?} should fail validation"
            );
        }
    }

    #[test]
    fn validate_accepts_finite_runner_up_ratio_of_any_magnitude() {
        // No range constraint on the ratio — verify low values pass.
        let c = Confidence {
            recognition: 0.5,
            rule: 0.5,
            region: None,
            runner_up_ratio: Some(0.01),
            features: SmallVec::new(),
        };
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_rejects_non_finite_feature_delta() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let c = Confidence {
                recognition: 0.5,
                rule: 0.5,
                region: None,
                runner_up_ratio: None,
                features: smallvec::smallvec![FeatureContribution {
                    id: FeatureId::EditDistance1,
                    delta: bad,
                }],
            };
            assert!(
                c.validate().is_err(),
                "feature delta = {bad:?} should fail validation"
            );
        }
    }

    #[test]
    fn validate_accepts_zero_axes() {
        // Zero is a legal below-threshold value, not an invariant
        // violation — check that validate doesn't treat it specially.
        let c = Confidence {
            recognition: 0.0,
            rule: 0.0,
            region: Some(0.0),
            runner_up_ratio: None,
            features: SmallVec::new(),
        };
        assert!(c.validate().is_ok());
    }
}
