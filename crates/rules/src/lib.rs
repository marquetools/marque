// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-rules — trait definitions for the marque rule system.
//!
//! This crate defines the contract every rule crate must satisfy.
//! It has no rule implementations — those live in `marque-capco` and future crates.
//! The engine depends only on this crate, enabling rule crates to be swapped.
//!
//! # Module layout
//!
//! - The typed citation surface (`Citation`, `SectionRef`,
//!   `SectionLetter`, `PageNumber`, `AuthoritativeSource`) lives in
//!   [`marque_scheme::citation`] (relocated from `marque-rules` in
//!   PR 10.A.1 so the scheme-level catalog rows can carry typed
//!   citations without inverting the crate dependency graph). The
//!   `Display` impl emits the citation-lint regex form
//!   (`§<L>.<sub>[.<sub_sub>] [Table <N>] p<page>`). Const-fn
//!   construction; no runtime validation per D25.2 in
//!   `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md`. The
//!   `&'static str` → `Citation` literal-site migration completed at
//!   PR 3c.2.C; PR 10.A.1 finished the type flip on catalog row
//!   declarations (`Constraint`, `PageRewrite`, `ClosureRule`).
//! - [`confidence`] — `Confidence` (recognition × rule axes), `FeatureId`,
//!   `FeatureContribution`. Phase D audit-provenance payload attached to
//!   every `FixIntent<S>`.
//! - [`message`] — `Message`, `MessageTemplate` (closed enum), `MessageArgs`
//!   (closed-set struct). The G13 type-system closure of the diagnostic-message
//!   leak channel: only `Message::new(template, args)` constructs a `Message`,
//!   and `MessageArgs` cannot carry input bytes (no `String` / `&str` / `Vec<u8>`
//!   fields).
//! - [`fix_intent`] — `FixIntent<S>`. The rule-emission API for the
//!   bag-of-tokens vocabulary from `architecture.md` §"What fixes are":
//!   fact-set deltas (`FactAdd` / `FactRemove`) and renderer
//!   recanonicalization (`Recanonicalize`). `ReplacementIntent<S>`,
//!   `FactRef<S>`, and `RecanonScope` live in `marque-scheme`; rules
//!   import them directly from there. The engine promotes a
//!   `FixIntent<S>` to an `AppliedFix<S>` via `__engine_promote`.
//!
//! # Type split: FixIntent vs AppliedFix
//!
//! `FixIntent<S>` is pure data emitted by rules — deterministic,
//! timestamp-free, classifier-free, safe to snapshot in tests.
//! `AppliedFix<S>` wraps it with runtime context (timestamp,
//! classifier id, dry-run flag) and is constructed **only** by
//! `Engine::fix_inner`. This makes "suggested vs applied" a
//! type-system invariant.
//!
//! The Commit 2–9 transition through a legacy `FixProposal` shape
//! retired in PR 3c.B Commit 10 (`mvp-1`/`mvp-2` → `mvp-3`); the
//! `marque-mvp-3 → marque-1.0` atomic cutover then landed at
//! PR 3c.2.D, reshaping `AppliedFix<S>` to carry `Canonical<S>` +
//! `Discriminant` + BLAKE3 digests of pre-fix and canonical bytes,
//! and splitting non-marking text corrections into a separate
//! `AppliedTextCorrection` type (the marking-side seal stays on
//! `AppliedFix<S>`; the text-correction-side seal lives on
//! `AppliedTextCorrection`).
//!
//! # G13 (audit content ignorance)
//!
//! `AppliedFix<S>` carries a sealed [`marque_scheme::Canonical<S>`]
//! payload (rendered token canonicals) + BLAKE3 digests of the
//! pre-fix and canonical bytes — no document content. The
//! `AppliedTextCorrection` channel carries only canonical
//! replacement strings (corpus-derived token canonicals on
//! Constitution V's permitted-identifier list, e.g. `"SECRET"`
//! replacing a typo). The T055 content-ignorance canary
//! (`crates/engine/tests/audit_g13_canary.rs`) sweeps the
//! regression corpora to verify no input substring ≥4 bytes appears
//! in any emitted NDJSON record outside the permitted-identifier
//! list.

pub mod audit;
pub mod audit_note;
pub mod confidence;
pub mod context;
pub mod diagnostic;
pub mod fix;
pub mod fix_intent;
pub mod message;
pub mod rule;
pub mod severity;

pub use audit::{
    AppliedFix, AppliedFixDetail, AppliedReplacement, AppliedTextCorrection, AuditLine,
    Discriminant,
};
pub use audit_note::{AuditNote, AuditNoteKind, AuditNoteStructural};
pub use confidence::{Confidence, FeatureContribution, FeatureId};
pub use context::RuleContext;
pub use diagnostic::{Diagnostic, TextCorrection};
pub use fix::{CORRECTIONS_MAP_CITATION, EnginePromotionToken, FixSource};
pub use fix_intent::FixIntent;
// Re-export `SmallVec` + the `smallvec!` macro so external consumers
// can construct `Confidence.features` (a `SmallVec<[FeatureContribution; 4]>`)
// and any other rules-crate SmallVec field without depending on the
// `smallvec` crate directly. The inline storage is an implementation
// detail of the audit-record payload; the re-export keeps it that
// way at the boundary.
pub use smallvec::{SmallVec, smallvec};
// `FactRef`, `ReplacementIntent`, and `RecanonScope` moved to
// `marque-scheme` as of the PR 3c.B engine-prereq (the new
// `MarkingScheme::apply_intent` trait method needs them at the trait
// surface; `marque-rules` already depends on `marque-scheme`, so the
// types must live below us in the dependency graph). Import them
// directly from `marque_scheme::{FactRef, RecanonScope, ReplacementIntent}`.
pub use marque_ism::{DocumentPosition, MarkingType, Zone};
pub use message::{Blake3Hash, Message, MessageArgs, MessageTemplate, to_audit_string};
pub use rule::{Rule, RuleId, RuleSet};
pub use severity::{Phase, Severity};

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn rule_id_round_trip() {
        // T044 / FR-026 / FR-044: RuleId is a (scheme, predicate_id)
        // 2-tuple. Accessors round-trip and the Display impl renders
        // the canonical wire string `"<scheme>:<predicate_id>"`.
        let r = RuleId::new("capco", "banner.classification.usa-trigraph");
        assert_eq!(r.scheme(), "capco");
        assert_eq!(r.predicate_id(), "banner.classification.usa-trigraph");
        assert_eq!(r.to_string(), "capco:banner.classification.usa-trigraph");
    }

    #[test]
    fn rule_id_is_copy() {
        // Both fields are `&'static str`, so `RuleId: Copy` is free
        // — consumers can hand it around without `.clone()` calls.
        // Compile-time check via Copy semantics: if this regresses to
        // `Clone`-only, the use-after-move line below stops compiling.
        let r = RuleId::new("engine", "fix.reparse-failed");
        let copy = r;
        assert_eq!(r.scheme(), "engine");
        assert_eq!(copy.predicate_id(), "fix.reparse-failed");
    }

    #[test]
    fn rule_id_display_wire_string_uses_colon_separator() {
        // T044 OD-2 / OD-3: the wire-string form is reserved for text
        // contexts (CLI human-readable output, log lines,
        // `.marque.toml` config keys). Colon was picked over slash
        // (slash collides with the catalog-row label convention) and
        // over dot (dot collides with predicate-id internal segments).
        assert_eq!(
            RuleId::new("engine", "recognition.decoder-recognized").to_string(),
            "engine:recognition.decoder-recognized",
        );
        assert_eq!(
            RuleId::new("test", "synthetic.r999-fixture").to_string(),
            "test:synthetic.r999-fixture",
        );
    }

    #[test]
    fn rule_id_engine_sentinels_use_reserved_scheme() {
        // T044 §1.4 + OD-4 + PM-decisions table row OD-4:
        // engine-minted diagnostics use the reserved "engine" scheme
        // and DROP the historical "r001."/"r002." numeric prefix. The
        // two concrete sentinels are documented on the RuleId type.
        let r001 = RuleId::new("engine", "recognition.decoder-recognized");
        let r002 = RuleId::new("engine", "fix.reparse-failed");
        assert_eq!(r001.scheme(), "engine");
        assert_eq!(r002.scheme(), "engine");
        // The pair is distinct — the scheme alone is not the rule
        // identity, the predicate-id segment carries the rest.
        assert_ne!(r001.predicate_id(), r002.predicate_id());
        assert_ne!(r001, r002);
    }

    #[test]
    fn severity_parse_config_accepts_known_values() {
        assert_eq!(Severity::parse_config("off"), Some(Severity::Off));
        assert_eq!(Severity::parse_config("suggest"), Some(Severity::Suggest));
        assert_eq!(Severity::parse_config("info"), Some(Severity::Info));
        assert_eq!(Severity::parse_config("warn"), Some(Severity::Warn));
        assert_eq!(Severity::parse_config("error"), Some(Severity::Error));
        assert_eq!(Severity::parse_config("fix"), Some(Severity::Fix));
    }

    #[test]
    fn severity_parse_config_is_case_sensitive() {
        assert_eq!(Severity::parse_config("OFF"), None);
        assert_eq!(Severity::parse_config("Warn"), None);
    }

    #[test]
    fn severity_parse_config_rejects_unknown_strings() {
        assert_eq!(Severity::parse_config("err"), None);
        assert_eq!(Severity::parse_config("disable"), None);
        assert_eq!(Severity::parse_config(""), None);
    }

    #[test]
    fn severity_display_round_trips() {
        for s in [
            Severity::Off,
            Severity::Suggest,
            Severity::Info,
            Severity::Warn,
            Severity::Error,
            Severity::Fix,
        ] {
            assert_eq!(Severity::parse_config(s.as_str()), Some(s));
            assert_eq!(s.to_string(), s.as_str());
        }
    }

    #[test]
    fn severity_ord_off_is_lowest() {
        // Off < Suggest < Info < Warn < Error < Fix — see the doc comment
        // on Severity for the intentional design rationale.
        assert!(Severity::Off < Severity::Suggest);
        assert!(Severity::Suggest < Severity::Info);
        assert!(Severity::Info < Severity::Warn);
        assert!(Severity::Warn < Severity::Error);
        assert!(Severity::Error < Severity::Fix);
    }

    #[test]
    fn severity_suggest_round_trips_through_config_string() {
        // Issue #235 / #186 PR-3: the suggest-don't-fix channel must be
        // a stable parse target. The config string "suggest" must round
        // trip through both parse_config and as_str.
        assert_eq!(Severity::parse_config("suggest"), Some(Severity::Suggest));
        assert_eq!(Severity::Suggest.as_str(), "suggest");
        assert_eq!(Severity::Suggest.to_string(), "suggest");
    }

    #[test]
    fn severity_suggest_is_strictly_below_info_in_ord() {
        // The renderer relies on Suggest sorting BELOW Info so that
        // CI exit-code logic ("Info or none → exit 0") generalizes
        // to ("Info-or-Suggest or none → exit 0") via the same
        // strict-less-than comparison.
        assert!(Severity::Suggest < Severity::Info);
        assert!(Severity::Off < Severity::Suggest);
    }

    // FixProposal-construction validation tests retired in
    // PR 3c.B Commit 10 (along with the FixProposal type itself).
    // Confidence's per-axis validate() is tested directly in
    // `confidence.rs`; FixIntent<S> construction is exercised in
    // `fix_intent.rs::tests`.
}
