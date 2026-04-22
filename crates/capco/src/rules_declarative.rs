// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Declarative CAPCO rule wrappers (T035).
//!
//! Each wrapper here replaces a hand-written `Rule` impl in
//! `crate::rules`. The wrapper calls `CapcoScheme::validate(marking)`
//! as a **trigger** (did the catalog's declared predicate fire?) and,
//! when it did, enumerates `attrs` locally to build `Diagnostic`
//! values with byte-identical message/span/fix output. This keeps
//! the catalog in `crate::scheme::build_constraints()` as the
//! authoritative source for "which invariant fires" while the
//! wrappers own the user-visible emission shape.
//!
//! ## Why trigger-only, not violation-driven
//!
//! `ConstraintViolation` carries `constraint_label`, `message`, and
//! `citation` but **not** a `Span` — the scheme has no access to the
//! `TokenSpan` slice the parser attaches to `IsmAttributes`. Widening
//! `ConstraintViolation` to carry spans would couple the scheme layer
//! to ISM's token-span model, which lives in `marque-ism` and is
//! CAPCO-specific. Trigger-only dispatch keeps the scheme layer
//! span-free; each wrapper constructs its span from
//! `attrs.token_spans` the same way the retired hand-written rule
//! did.
//!
//! ## Citation policy: wrappers carry byte-identity-frozen citations
//!
//! Every `Diagnostic` emitted here carries the *legacy* rule's
//! citation string verbatim — typically a section-only reference like
//! `"CAPCO-2016 §H.6"`. The **authoritative** citation with specific
//! page + line numbers lives on the matching catalog entry in
//! `crate::scheme::build_constraints`. We do not unify them in this
//! PR because the corpus NDJSON output is a stable surface and
//! changing the citation string breaks SC-008 byte-identity.
//!
//! When two diverge on *section* (not just precision) — currently
//! E012 (`§B.1` in wrapper vs `§H.3 p55` in catalog) and E015
//! (`§B.3` vs `§H.7 + §B.3.d`) — the catalog is correct and the
//! wrapper is pending a citation update in a follow-up that can
//! bump the NDJSON schema or carry a migration note. For now,
//! per-wrapper inline comments flag the divergence so a future
//! author reading the wrapper doesn't take `§B.1` / `§B.3` as
//! authoritative without cross-checking the catalog.
//!
//! ## E018 / E019 are NOT retired here
//!
//! `JointIcDissemRule` (E018) and `JointNonIcDissemRule` (E019) stay
//! as hand-written impls in `crate::rules` pending T035b. CAPCO-2016
//! §H.3 lines 4140-4146 explicitly permit JOINT with IC and non-IC
//! dissem controls (excluding only NOFORN and HCS); both existing
//! rules are over-restrictive relative to the source.

use std::sync::LazyLock;

use marque_ism::{IsmAttributes, Span, TokenKind, TokenSpan};
use marque_rules::{Diagnostic, FixSource, Rule, RuleContext, RuleId, Severity};
use marque_scheme::{ConstraintViolation, MarkingScheme};

use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
use crate::scheme::{CapcoMarking, CapcoScheme};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Process-global `CapcoScheme` instance shared across every wrapper
/// invocation. The scheme is stateless, deterministic, and carries
/// only `&'static` references + `Vec`s of fixed-size entries, so a
/// single instance is sound for all threads and all documents.
///
/// Hoisting this out of `violations_for` eliminates the per-wrapper
/// `CapcoScheme::new()` allocation — with 11 declarative wrappers in
/// the rule set, a single document with N markings was doing 11×N
/// scheme constructions before. Constitution VI's "rules MUST be
/// stateless" guarantee is preserved because the wrappers themselves
/// carry no state; the `LazyLock` lives outside the `Rule` impls.
static SCHEME: LazyLock<CapcoScheme> = LazyLock::new(CapcoScheme::new);

/// Run the scheme's constraint evaluator and return only the
/// violations whose `constraint_label` matches one of `wanted`.
///
/// Still allocates: each call clones `IsmAttributes` into a
/// `CapcoMarking` and runs the full constraint loop. Sharing one
/// `validate()` result across all 11 wrappers per marking would
/// require threading a per-marking cache through `RuleContext`
/// (a `marque-rules` trait-surface change) — deferred until
/// benchmark data shows the remaining overhead is material on the
/// SC-001 p95 path.
fn violations_for(attrs: &IsmAttributes, wanted: &[&'static str]) -> Vec<ConstraintViolation> {
    SCHEME
        .validate(&CapcoMarking(attrs.clone()))
        .into_iter()
        .filter(|v| wanted.contains(&v.constraint_label))
        .collect()
}

/// Return the `Span` of the first token in `attrs.token_spans` whose
/// kind matches `kind`, or `(0, 0)` if none is present. Matches the
/// span-selection idiom used by the retired hand-written rules.
fn first_span_of(attrs: &IsmAttributes, kind: TokenKind) -> Span {
    attrs
        .token_spans
        .iter()
        .find(|t| t.kind == kind)
        .map(|t| t.span)
        .unwrap_or(Span::new(0, 0))
}

/// Collect all token spans of a given kind in document order.
fn spans_of_kind(attrs: &IsmAttributes, kind: TokenKind) -> Vec<&TokenSpan> {
    attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == kind)
        .collect()
}

// ---------------------------------------------------------------------------
// E010 — bare HCS requires compartment suffix
// ---------------------------------------------------------------------------

/// Replaces the hand-written `BareHcsRule`.
///
/// The catalog's `E010/HCS-system-constraints` Custom fires multiple
/// violations per offending marking (one per failing sub-rule: bare-
/// HCS detection, HCS-O/P classification floor, ORCON pairing, etc.).
/// Only the bare-HCS sub-violation corresponds to a legacy hand-
/// written diagnostic; the other sub-rules weren't emitted by any
/// rule before T035. The wrapper discriminates by message prefix so
/// byte-identity with the pre-branch corpus is preserved; the other
/// sub-rules drop silently until a future PR wires wrappers for them.
pub(crate) struct DeclarativeBareHcsRule;

impl Rule for DeclarativeBareHcsRule {
    fn id(&self) -> RuleId {
        RuleId::new("E010")
    }
    fn name(&self) -> &'static str {
        "bare-hcs"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::SciControl;

        let violations = violations_for(attrs, &["E010/HCS-system-constraints"]);
        let bare_hcs_fired = violations.iter().any(|v| {
            v.message.starts_with("Bare HCS is legacy")
                || v.message.starts_with("HCS requires a compartment")
        });
        if !bare_hcs_fired {
            return vec![];
        }

        // Byte-identity: reproduce the retired rule's message +
        // confidence selection. Inspect `sci_controls` again locally
        // because the scheme's violation message doesn't carry the
        // sub-discriminator.
        let has_hcs_o = attrs.sci_controls.contains(&SciControl::HcsO);
        let has_hcs_p = attrs.sci_controls.contains(&SciControl::HcsP);
        let (confidence, message) = if has_hcs_o {
            (
                0.5,
                "bare HCS requires a compartment suffix (-O or -P); \
                 HCS-O appears in this marking — verify whether HCS should be HCS-O or HCS-P"
                    .to_owned(),
            )
        } else if has_hcs_p {
            (
                0.95,
                "bare HCS requires a compartment suffix; \
                 HCS-P already present — this HCS likely should be HCS-P"
                    .to_owned(),
            )
        } else {
            (
                0.95,
                "bare HCS requires a compartment suffix (-O or -P); \
                 use HCS-P unless this involves operational source information"
                    .to_owned(),
            )
        };

        // Find the token span for the bare HCS entry (matches legacy
        // rule: position-indexed lookup into SciControl spans).
        let sci_spans = spans_of_kind(attrs, TokenKind::SciControl);
        let hcs_idx = attrs
            .sci_controls
            .iter()
            .position(|s| *s == SciControl::Hcs);
        let span = hcs_idx
            .and_then(|i| sci_spans.get(i))
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span,
            message,
            citation: "CAPCO-2016 §H.4",
            original: "HCS".to_owned(),
            replacement: "HCS-P".to_owned(),
            confidence,
            migration_ref: None,
        })]
    }
}

// ---------------------------------------------------------------------------
// E012 — dual classification (US + foreign in one marking)
// ---------------------------------------------------------------------------

/// Replaces the hand-written `DualClassificationRule`.
pub(crate) struct DeclarativeDualClassificationRule;

impl Rule for DeclarativeDualClassificationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E012")
    }
    fn name(&self) -> &'static str {
        "dual-classification"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::{ForeignClassification, MarkingClassification};

        if violations_for(attrs, &["E012/dual-classification"]).is_empty() {
            return vec![];
        }

        let Some(MarkingClassification::Conflict { us, foreign }) = &attrs.classification else {
            return vec![];
        };

        let foreign_desc = match foreign.as_ref() {
            ForeignClassification::Nato(n) => format!("NATO ({})", n.banner_str()),
            ForeignClassification::Fgi(f) => {
                let countries: Vec<&str> = f.countries.iter().map(|c| c.as_str()).collect();
                if countries.is_empty() {
                    "FGI".to_owned()
                } else {
                    format!("FGI {}", countries.join(" "))
                }
            }
            ForeignClassification::Joint(j) => {
                let countries: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
                format!("JOINT {}", countries.join(" "))
            }
        };

        let fgi_replacement = match foreign.as_ref() {
            ForeignClassification::Nato(_) => "FGI NATO".to_owned(),
            ForeignClassification::Fgi(f) => {
                let countries: Vec<&str> = f.countries.iter().map(|c| c.as_str()).collect();
                if countries.is_empty() {
                    "FGI".to_owned()
                } else {
                    format!("FGI {}", countries.join(" "))
                }
            }
            ForeignClassification::Joint(j) => {
                let countries: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
                format!("FGI {}", countries.join(" "))
            }
        };

        // Second Classification token span — that's the foreign one.
        let class_spans = spans_of_kind(attrs, TokenKind::Classification);
        let span = class_spans
            .get(1)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));
        let original = class_spans
            .get(1)
            .map(|t| t.text.to_string())
            .unwrap_or_default();

        vec![make_fix_diagnostic(FixDiagnosticParams {
            rule: self.id(),
            severity: self.default_severity(),
            source: FixSource::BuiltinRule,
            span,
            message: format!(
                "marking has both US ({}) and foreign ({foreign_desc}) classification; \
                 US wins at {}; move foreign to FGI block",
                us.banner_str(),
                us.banner_str(),
            ),
            // Byte-identity freeze. Catalog cites §H.3 p55 (correct
            // authoritative passage); §B.1 is the legacy wrapper
            // citation — update in a separate NDJSON-schema-migration
            // PR. See module-level "Citation policy" doc.
            citation: "CAPCO-2016 §B.1",
            original,
            replacement: fgi_replacement,
            confidence: 0.90,
            migration_ref: None,
        })]
    }
}

// ---------------------------------------------------------------------------
// E014 — JOINT participants must appear in REL TO
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeJointRelToRule;

impl Rule for DeclarativeJointRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("E014")
    }
    fn name(&self) -> &'static str {
        "joint-rel-to"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingClassification;

        if violations_for(attrs, &["E014/joint-requires-rel-to-coverage"]).is_empty() {
            return vec![];
        }

        let joint = match &attrs.classification {
            Some(MarkingClassification::Joint(j)) => j,
            _ => return vec![],
        };

        let missing: Vec<&str> = joint
            .countries
            .iter()
            .filter(|c| !attrs.rel_to.contains(c))
            .map(|c| c.as_str())
            .collect();
        if missing.is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::Classification);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            format!(
                "JOINT participants [{}] must appear in REL TO list",
                missing.join(", "),
            ),
            "CAPCO-2016 §H.3",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E015 — non-US classification requires dissem control
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeNonUsMissingDissemRule;

impl Rule for DeclarativeNonUsMissingDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E015")
    }
    fn name(&self) -> &'static str {
        "non-us-missing-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, &["E015/non-us-requires-dissem"]).is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::Classification);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "non-US classification must be accompanied by a dissemination control \
             (e.g., REL TO, NOFORN)",
            // Byte-identity freeze. Catalog cites §H.7 + §B.3.d
            // (correct authoritative passages — FGI commingling +
            // FD&R procedures); §B.3 alone is the legacy wrapper
            // citation. See module-level "Citation policy" doc.
            "CAPCO-2016 §B.3",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E016 — JOINT cannot be RESTRICTED
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeJointRestrictedRule;

impl Rule for DeclarativeJointRestrictedRule {
    fn id(&self) -> RuleId {
        RuleId::new("E016")
    }
    fn name(&self) -> &'static str {
        "joint-restricted"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, &["E016/joint-conflicts-restricted"]).is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::Classification);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RESTRICTED may not be used with JOINT — the US has no equivalent \
             classification level for RESTRICTED",
            "CAPCO-2016 §H.3",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E017 — JOINT cannot be used with FGI marker
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeJointFgiRule;

impl Rule for DeclarativeJointFgiRule {
    fn id(&self) -> RuleId {
        RuleId::new("E017")
    }
    fn name(&self) -> &'static str {
        "joint-fgi"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, &["E017/joint-conflicts-fgi"]).is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::FgiMarker);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "JOINT may not be used with FGI — a marking is either co-owned (JOINT) \
             or foreign-originated (FGI), not both",
            "CAPCO-2016 §H.3",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E021 — RD/FRD/TFNI requires NOFORN
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeAeaNofornRule;

impl Rule for DeclarativeAeaNofornRule {
    fn id(&self) -> RuleId {
        RuleId::new("E021")
    }
    fn name(&self) -> &'static str {
        "aea-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, &["E021/aea-requires-noforn"]).is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::AeaMarking);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RD/FRD/TFNI requires NOFORN unless a sharing agreement exists \
             per the Atomic Energy Act; override to warn via rule severity \
             config if sharing agreements apply",
            "CAPCO-2016 §H.6",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E022 — CNWDI classification floor
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeCnwdiConstraintRule;

impl Rule for DeclarativeCnwdiConstraintRule {
    fn id(&self) -> RuleId {
        RuleId::new("E022")
    }
    fn name(&self) -> &'static str {
        "cnwdi-constraint"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, &["E022/CNWDI-classification-floor"]).is_empty() {
            return vec![];
        }

        let level = attrs.us_classification();
        let level_str = level.map(|c| c.banner_str()).unwrap_or("unknown");
        let span = first_span_of(attrs, TokenKind::AeaMarking);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            format!(
                "CNWDI may only be used with TOP SECRET or SECRET RD; \
                 current classification is {level_str}"
            ),
            "CAPCO-2016 §H.6",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E024 — RD takes precedence over FRD/TFNI (multi-emission)
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeRdPrecedenceRule;

impl Rule for DeclarativeRdPrecedenceRule {
    fn id(&self) -> RuleId {
        RuleId::new("E024")
    }
    fn name(&self) -> &'static str {
        "rd-precedence"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::AeaMarking;

        if violations_for(attrs, &["E024/rd-precedence"]).is_empty() {
            return vec![];
        }

        let mut diagnostics = Vec::new();
        let aea_spans = spans_of_kind(attrs, TokenKind::AeaMarking);
        for (idx, aea) in attrs.aea_markings.iter().enumerate() {
            let superseded = match aea {
                AeaMarking::Frd(_) => "FRD",
                AeaMarking::Tfni => "TFNI",
                _ => continue,
            };
            let span = aea_spans
                .get(idx)
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));
            diagnostics.push(Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                format!(
                    "{superseded} should not appear alongside RD; \
                     RD takes precedence over {superseded} in both banners and portions"
                ),
                "CAPCO-2016 §H.6",
                None,
            ));
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// E025 — UCNI only with UNCLASSIFIED
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeUcniClassificationRule;

impl Rule for DeclarativeUcniClassificationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E025")
    }
    fn name(&self) -> &'static str {
        "ucni-classification"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, &["E025/ucni-conflicts-classification"]).is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::AeaMarking);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "DOD/DOE UCNI may only be used with UNCLASSIFIED information",
            "CAPCO-2016 §H.6",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// W002 — US + FGI comingling in portion (portion-only)
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeCominglingWarningRule;

impl Rule for DeclarativeCominglingWarningRule {
    fn id(&self) -> RuleId {
        RuleId::new("W002")
    }
    fn name(&self) -> &'static str {
        "us-fgi-comingling"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        // Portion-only filter: the catalog predicate fires on any
        // US+FGI presence; user-facing diagnostic is portion-only per
        // CAPCO §H.7 lines 8254-8268 (banner-level commingling is
        // governed by different rules).
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        if violations_for(attrs, &["W002/us-commingled-with-fgi"]).is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::FgiMarker);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "portion mark comingles US classification with FGI; \
             consider splitting into separate US and foreign paragraphs",
            "CAPCO-2016 §H.7",
            None,
        )]
    }
}
