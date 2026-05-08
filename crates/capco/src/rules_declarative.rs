// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Declarative CAPCO rule wrappers (T035).
//!
//! Each wrapper here replaces a hand-written `Rule` impl in
//! `crate::rules`. The wrapper calls
//! [`CapcoScheme::evaluate_named_constraint`] as a **trigger** (did
//! the catalog's declared predicate fire?) and, when it did,
//! enumerates `attrs` locally to build `Diagnostic` values with
//! byte-identical message/span/fix output. This keeps the catalog
//! in `crate::scheme::build_constraints()` as the authoritative
//! source for "which invariant fires" while the wrappers own the
//! user-visible emission shape.
//!
//! `evaluate_named_constraint` is the inherent fast path on
//! `CapcoScheme` that takes `&CanonicalAttrs` directly and dispatches
//! only the single named predicate — no `CapcoMarking` wrap, no full
//! catalog walk. The trait-path `scheme.validate()` + post-hoc
//! filtering that an earlier revision used iterated all ~13 catalog
//! entries per wrapper (11× overhead per marking); the named path
//! reduces that to a linear `find`-by-name plus one predicate
//! dispatch.
//!
//! ## Why trigger-only, not violation-driven
//!
//! `ConstraintViolation` carries `constraint_label`, `message`, and
//! `citation` but **not** a `Span` — the scheme has no access to the
//! `TokenSpan` slice the parser attaches to `CanonicalAttrs`. Widening
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
//! ## T035b audit: E017/E018/E019 retired, E036 added
//!
//! The T035b correctness audit (2026-04-21) retired three
//! over-restrictive JOINT rules that contradicted CAPCO-2016
//! §H.3 p57 (Relationship(s) to Other Markings, "May be used
//! with SCI (excluding HCS markings), SAP, AEA, FGI, IC and
//! Non-IC dissemination control markings (excluding NOFORN),
//! as appropriate"):
//!
//! - **E017** (`JointFgiRule`) — JOINT + FGI marker forbidden.
//!   Wrong: §H.3 p57 lists FGI among markings JOINT "may be
//!   used with"; the FGI commingling syntax is cross-referenced
//!   to §H.7. Retired entirely.
//! - **E018** (`JointIcDissemRule`) — JOINT + any non-REL IC
//!   dissem control forbidden. Wrong: §H.3 p57 permits
//!   IC dissem "as appropriate"; the only specific exclusions
//!   called out are NOFORN and HCS. Retired entirely
//!   (see replacement below).
//! - **E019** (`JointNonIcDissemRule`) — JOINT + any non-IC
//!   dissem forbidden. Wrong: §H.3 p57 permits non-IC
//!   dissem with JOINT "as appropriate". Retired entirely.
//!
//! Replacement: **E036** (`DeclarativeJointHcsRule`) — the only
//! specific exclusion CAPCO actually calls out. JOINT + NOFORN is
//! covered indirectly by `capco/noforn-conflicts-rel-to` + E014's
//! REL TO requirement.

use std::sync::LazyLock;

use marque_ism::{CanonicalAttrs, Span, TokenKind, TokenSpan};
use marque_rules::{
    Confidence, Diagnostic, FixProposal, FixSource, Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::ConstraintViolation;

use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
use crate::scheme::CapcoScheme;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Process-global `CapcoScheme` instance shared across every wrapper
/// invocation. The scheme is stateless, deterministic, and carries
/// only `&'static` references + `Vec`s of fixed-size entries, so a
/// single instance is sound for all threads and all documents.
///
/// Constitution VI's "rules MUST be stateless" guarantee holds
/// because the wrapper structs themselves carry no state; the
/// `LazyLock` lives at module scope, outside the `Rule` impls.
static SCHEME: LazyLock<CapcoScheme> = LazyLock::new(CapcoScheme::new);

/// Evaluate a single named constraint via the fast path
/// ([`CapcoScheme::evaluate_named_constraint`]), returning the
/// violations (if any) that the named predicate produced.
///
/// **No clone, no catalog walk.** This is the key perf-difference
/// from the earlier `validate()`-plus-filter pattern:
///
/// - `evaluate_named_constraint` takes `&CanonicalAttrs` directly, so
///   the wrapper doesn't have to `CapcoMarking::new(attrs.clone())` to
///   cross the trait boundary.
/// - It finds the constraint by name (linear scan of ~13 entries)
///   and dispatches only that one predicate. The old `validate()`
///   path walked the entire catalog per wrapper call — with 11
///   declarative wrappers that was an 11× overhead on every
///   marking.
///
/// The wrapper struct + its `check()` signature stay unchanged;
/// this is a pure perf path swap.
fn violations_for(attrs: &CanonicalAttrs, name: &'static str) -> Vec<ConstraintViolation> {
    SCHEME.evaluate_named_constraint(attrs, name)
}

/// Return the `Span` of the first token in `attrs.token_spans` whose
/// kind matches `kind`, or `(0, 0)` if none is present. Matches the
/// span-selection idiom used by the retired hand-written rules.
fn first_span_of(attrs: &CanonicalAttrs, kind: TokenKind) -> Span {
    attrs
        .token_spans
        .iter()
        .find(|t| t.kind == kind)
        .map(|t| t.span)
        .unwrap_or(Span::new(0, 0))
}

/// Collect all token spans of a given kind in document order.
fn spans_of_kind(attrs: &CanonicalAttrs, kind: TokenKind) -> Vec<&TokenSpan> {
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::SciControl;

        let violations = violations_for(attrs, "E010/HCS-system-constraints");
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::{ForeignClassification, MarkingClassification};

        if violations_for(attrs, "E012/dual-classification").is_empty() {
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingClassification;

        if violations_for(attrs, "E014/joint-requires-rel-to-coverage").is_empty() {
            return vec![];
        }

        let joint = match &attrs.classification {
            Some(MarkingClassification::Joint(j)) => j,
            _ => return vec![],
        };

        let missing: Vec<&str> = joint
            .countries
            .iter()
            .filter(|c| !crate::scheme::rel_to_covers(&attrs.rel_to, c.as_str()))
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E015/non-us-requires-dissem").is_empty() {
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E016/joint-conflicts-restricted").is_empty() {
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
// E036 — JOINT cannot be used with HCS markings
// ---------------------------------------------------------------------------
//
// Replaces the retired E017/E018/E019 (T035b audit). CAPCO-2016 §H.3
// p57 (Relationship(s) to Other Markings): "May not be used with the
// HCS markings or NOFORN markings." The JOINT-NOFORN exclusion is
// already enforced indirectly via `capco/noforn-conflicts-rel-to` +
// E014's REL TO requirement. The HCS exclusion is the only remaining
// specific constraint this rule fires on.
//
// "HCS markings" is plural — covers `HCS`, `HCS-O`, `HCS-P`, and any
// compound anchored on `SciControlBare::Hcs` in `sci_markings`.
// `TOK_HCS` in `satisfies_attrs` matches all of them.

pub(crate) struct DeclarativeJointHcsRule;

impl Rule for DeclarativeJointHcsRule {
    fn id(&self) -> RuleId {
        RuleId::new("E036")
    }
    fn name(&self) -> &'static str {
        "joint-hcs"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E036/joint-conflicts-hcs").is_empty() {
            return vec![];
        }

        // Span selection: point at the offending HCS SCI control
        // token. When multiple SCI controls are present (e.g.,
        // `SI//HCS-P`), the first SciControl span may be SI, which
        // is not the violation. Prefer the first span whose text
        // starts with "HCS"; fall back to the first SciControl span
        // only if no HCS-prefixed token span is attached (parser
        // gaps). The JOINT classification itself is not in error;
        // the user needs to remove or re-categorize HCS.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::SciControl && t.text.starts_with("HCS"))
            .map(|t| t.span)
            .unwrap_or_else(|| first_span_of(attrs, TokenKind::SciControl));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "HCS markings may not be used with JOINT classification \
             (CAPCO-2016 §H.3 explicitly excludes HCS from JOINT \
             documents; use a US classification marking with HCS instead)",
            "CAPCO-2016 §H.3 p57",
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E021/aea-requires-noforn").is_empty() {
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E022/CNWDI-classification-floor").is_empty() {
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::AeaMarking;

        if violations_for(attrs, "E024/rd-precedence").is_empty() {
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

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E025/ucni-conflicts-classification").is_empty() {
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

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic> {
        use marque_ism::MarkingType;
        // Portion-only filter: the catalog predicate fires on any
        // US+FGI presence; user-facing diagnostic is portion-only per
        // CAPCO §H.7 lines 8254-8268 (banner-level commingling is
        // governed by different rules).
        if ctx.marking_type != MarkingType::Portion {
            return vec![];
        }

        if violations_for(attrs, "W002/us-commingled-with-fgi").is_empty() {
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

// ---------------------------------------------------------------------------
// E037 — NODIS and EXDIS must not coexist (T035c-21 PR-A)
// ---------------------------------------------------------------------------
//
// CAPCO-2016 §H.9 p172 (EXDIS) and §H.9 p174
// (NODIS) both state the same mutual-exclusion invariant: NODIS and
// EXDIS MUST NOT coexist on the same information. This is the
// canonical conflict rule — two-way textually stated in both
// template entries, no carve-out.
//
// Declarative: modeled as a symmetric `Conflicts { TOK_NODIS,
// TOK_EXDIS }` constraint on `CapcoScheme`. The wrapper below
// dispatches via the constraint's `name` and emits the user-facing
// diagnostic.

pub(crate) struct DeclarativeNodisConflictsExdisRule;

impl Rule for DeclarativeNodisConflictsExdisRule {
    fn id(&self) -> RuleId {
        RuleId::new("E037")
    }
    fn name(&self) -> &'static str {
        "nodis-conflicts-exdis"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E037/nodis-conflicts-exdis").is_empty() {
            return vec![];
        }

        // Point at the first non-IC dissem token span. Either NODIS
        // or EXDIS is the first offender per source order; the user
        // needs to remove one of them to resolve.
        let span = first_span_of(attrs, TokenKind::NonIcDissem);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "NODIS and EXDIS must not coexist; each State Department \
             dissem control is mutually exclusive per CAPCO-2016 §H.9",
            "CAPCO-2016 §H.9 p172 + p174",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E038 — NODIS / EXDIS require NOFORN (T035c-21 PR-A)
// ---------------------------------------------------------------------------
//
// CAPCO-2016 §H.9 EXDIS entry p172 and NODIS entry p174
// both state: "May be used only with NOFORN information." A marking
// carrying NODIS or EXDIS without NOFORN violates both template
// entries.
//
// Declarative via `Constraint::Custom` because folding "NODIS OR
// EXDIS without NOFORN" into a single predicate — one diagnostic
// ID, one violation — keeps the wrapper trivial. Splitting into two
// separate `Requires` constraints would produce two distinct
// violation names for one rule ID.

pub(crate) struct DeclarativeDosDissemNofornRule;

impl Rule for DeclarativeDosDissemNofornRule {
    fn id(&self) -> RuleId {
        RuleId::new("E038")
    }
    fn name(&self) -> &'static str {
        "dos-dissem-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E038/nodis-or-exdis-requires-noforn").is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::NonIcDissem);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "NODIS and EXDIS may be used only with NOFORN information; \
             add NOFORN to the dissem controls",
            "CAPCO-2016 §H.9 p172 + p174",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E053 — NOFORN conflicts with REL TO (§H.8 p145)
// ---------------------------------------------------------------------------
// (See below for E054–E057, PR 3b.C RELIDO incompatibility wrappers.)

pub(crate) struct DeclarativeNofornRelToConflictRule;

impl Rule for DeclarativeNofornRelToConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E053")
    }
    fn name(&self) -> &'static str {
        "noforn-rel-to-conflict"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "capco/noforn-conflicts-rel-to").is_empty() {
            return vec![];
        }

        // Point to NOFORN, the disallowing control: §H.8 p145 says NOFORN
        // "Cannot be used with REL TO." The REL TO block is also present,
        // but NOFORN is the asserting token that makes REL TO invalid.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "NOFORN")
            .or_else(|| {
                attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "NF")
            })
            .map(|t| t.span)
            .unwrap_or_else(|| first_span_of(attrs, TokenKind::RelToBlock));

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "NOFORN cannot be used with REL TO (§H.8 p145); \
             remove one or the other",
            "CAPCO-2016 §H.8 p145",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// PR 3b.C (T026c) — RELIDO incompatibility wrappers (E054 / E055 / E056 / E057)
// ---------------------------------------------------------------------------
//
// Four directly-cited §H.8 RELIDO mutual-exclusion pairs, each wrapping
// one `Constraint::Conflicts` row in `CapcoScheme::constraints()`:
//
//   E054 — RELIDO ⊥ NOFORN        (§H.8 p154; reciprocal §H.8 p145)
//   E055 — RELIDO ⊥ DISPLAY ONLY  (§H.8 p154; reciprocal §H.8 p163)
//   E056 — ORCON  ⊥ RELIDO        (§H.8 p136; asymmetric — no p154 reciprocal)
//   E057 — ORCON-USGOV ⊥ RELIDO   (§H.8 p140; asymmetric — no p154 reciprocal)
//
// Pattern: each wrapper calls `violations_for(attrs, "<catalog-name>")` as the
// trigger check (did the named `Constraint::Conflicts` predicate fire?), then
// selects a diagnostic-anchor span from `attrs.token_spans` at the LHS
// (asserting) token per PM Q1 + Q2 resolution, computes a removal span via
// `compute_relido_removal_span` that covers RELIDO + an adjacent `/`
// separator, and emits a single `Diagnostic` carrying a subtractive
// `FixProposal` whose replacement is `""`. The citation-fidelity test in
// `tests/relido_conflicts.rs` enforces byte-identity between wrapper emission
// and catalog label.
//
// Scope note: the broader §3.4.2 family roster (RELIDO ⊥ {LES-NF, SBU-NF,
// each FGI atom, each JOINT atom, each NATO atom}) is deferred to PR 3.7
// (T108b) where `Constraint::Conflicts::RhsFamily(predicate)` ships. See
// `docs/plans/2026-05-07-pr3b-C-relido-conflicts-plan.md §2` for rationale.
//
// Subtractive-fix direction (PM Addendum II, 2026-05-07).
//
// Marque is a guidance tool for dissem markings, not just a checker. The
// dissemination axis is the unique area where the engine can apply true
// fixes — we are never *inventing* a token (we couldn't say "this should be
// LES" if the user never typed LES); we are only *removing* one that the
// surrounding tokens have already excluded by their §-cited Relationship(s)
// prose. RELIDO is the unambiguous remove-target in all four cases:
//
//   E054: NOFORN dominates per FD&R supersession (§D.2 Table 3 +
//         §H.8 p145). Remove RELIDO.
//   E055: DISPLAY ONLY is a positive disclosure decision (specific country
//         list); RELIDO defers release to a SFDRA. The deferred decision
//         can't operate when a positive decision is already on the marking.
//         Remove RELIDO.
//   E056: §H.8 p136 explicitly asserts "May not be used with RELIDO" on the
//         ORCON template — RELIDO is the rejected token. Remove RELIDO.
//   E057: §H.8 p140 explicitly asserts "May not be used with RELIDO" on the
//         ORCON-USGOV template. Same logic as E056. Remove RELIDO.
//
// The fix is `confidence = 0.95` (definite, at-threshold) so it auto-applies
// under the engine's default `Config::confidence_threshold = 0.95`
// (auto-apply gate is `confidence >= threshold`). The §-cited prose is
// categorical in every case and the user has explicitly endorsed RELIDO as
// the remove-target, so the matching CAPCO convention is the 0.95 tier
// (rules.rs:998 / :1327 / :2622 / :2777 / :2853 — definite-fix sites);
// 0.85–0.9 is reserved for conditional or lower-confidence cases. See
// `build_relido_removal_fix` doc-comment for the full calibration rationale.
//
// Generalization scope: this subtractive-fix pattern applies to **dissem-axis
// `Constraint::Conflicts`** rules ONLY. Non-dissem axis conflicts
// (classification E012, JOINT cross-system, SCI grammar) remain
// "user resolves" because the fix direction cannot be inferred without
// policy input.
//
// Constitution V (audit-first) compliance preserved: every `FixProposal` is
// pure data (span + replacement + confidence + source + migration_ref). The
// engine snapshots runtime state into `AppliedFix` at promotion time. The
// wrappers never construct `AppliedFix`.
//
// Constitution VI: all four structs are stateless zero-size; `Send + Sync`
// compliance is automatic (no interior mutability, no heap state).

/// Compute a `(removal_span, original_text)` pair for the RELIDO token in
/// `attrs.token_spans`, including the adjacent separator(s) so a fix with
/// replacement `""` produces a well-formed marking (no dangling `//`,
/// no leading `/` after `//`, no trailing `/`, no source bytes outside the
/// dissem-block category consumed).
///
/// Layout cases (PM Addendum II §3 + 2026-05-08 idempotency-fix extension):
///
/// - **Middle / last in dissem block** — RELIDO has a `/`-adjacent prior
///   sibling (`prior.end + 1 == relido.start`). Consume the preceding `/`:
///   removal span `[relido.start - 1, relido.end]`, original `"/RELIDO"`.
///   After fix: surrounding tokens close up cleanly.
/// - **First in dissem block (with following sibling)** — RELIDO has a
///   `/`-adjacent following sibling (`next.start == relido.end + 1`) but
///   no `/`-adjacent prior. Consume the trailing `/`: removal span
///   `[relido.start, relido.end + 1]`, original `"RELIDO/"`.
/// - **Sole dissem in `//`-delimited category** — RELIDO has a prior
///   `TokenSpan` separated by `//` (`prior.end + 2 == relido.start`) AND
///   no `/`-adjacent prior or following sibling. Consume both preceding
///   `/`s: removal span `[relido.start - 2, relido.end]`, original
///   `"//RELIDO"`. This case covers banner-form input like
///   `TOP SECRET//NOFORN//RELIDO` (where NOFORN and RELIDO sit in
///   separate dissem categories under malformed-but-recognizable input)
///   AND portion-form `(S//RELIDO)` (where RELIDO is a sole-payload
///   dissem block). Without this branch the fix would leave a stranded
///   `//` separator, and a follow-on E004 separator-collapse fix would
///   apply on a second pass — breaking idempotency.
///
/// Discrimination follows the parser's actual `TokenSpan` emission
/// pattern, which is asymmetric across separator kinds (no source-buffer
/// access required — Constitution V keeps `FixProposal` pure data):
///
///   - **Cross-category `//` separators** are emitted as
///     `TokenKind::Separator` spans with `text == "//"`, occupying two
///     bytes between the bordering content tokens.
///   - **Intra-block `/` separators** are NOT emitted as TokenSpans;
///     adjacent dissem-control content tokens carry adjacent byte
///     offsets (`prev.span.end + 1 == relido.span.start`) and the `/`
///     occupies the gap byte without a span of its own.
///
/// So the helper discriminates Cases 1 / 2 (intra-block) by **content-
/// token byte adjacency** (`prev.kind != Separator && prev.span.end + 1
/// == relido.span.start`, and the symmetric check for `next`) and Case 3
/// (cross-category) by the **explicit `Separator` span with `text ==
/// "//"`** immediately preceding RELIDO. Earlier byte-offset-only logic
/// missed Case 3; an interim Separator-only attempt broke Cases 1 / 2;
/// the combined model handles both correctly. This was the
/// `proptest_engine::fix_idempotent` regression caught and resolved on
/// 2026-05-08 for banner-form `TOP SECRET//NOFORN//RELIDO` input.
///
/// # Returns `None` when no sound removal can be proved
///
/// - `attrs.token_spans` contains no `RELIDO` token (caller's check fired
///   on the constraint predicate but the parser didn't surface a span — a
///   rare-but-real layout where the fast-path elided the span).
/// - **RELIDO has no adjacent neighbor on either side** (not preceded by
///   `/` or `//`, not followed by a `/`-adjacent sibling). Without a
///   recognized layout the helper cannot prove which separator to consume;
///   eating bytes blindly risks reaching outside the marking (the closing
///   `)`, `\n`, or end-of-source). Realistic parser output always provides
///   at least one anchor; this branch is defensive against synthetic /
///   malformed inputs.
/// - **Non-canonical whitespace between the prior token and RELIDO** (e.g.,
///   `(S//OC /RELIDO)`). Byte-offset adjacency fails when whitespace or
///   other content occupies the would-be-separator position. The parser
///   canonicalizes whitespace in normal CAPCO-shaped input, so this is
///   defensive against synthetic / malformed inputs.
///
/// **The helper is for canonical-whitespace inputs only.** Broader fix
/// coverage (long-form `RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL`,
/// non-canonical whitespace, RELIDO with FGI/JOINT/NATO atoms in shapes
/// like `(S//FGI XXX//RELIDO)`) is a future-PR concern — see plan §8 and
/// PR 3.7 (T108b) deferred work for the family-predicate roster, where
/// the subtractive-fix pattern extends without changing this helper.
///
/// Long-form RELIDO is not yet auto-fix-supported; the conflict predicate
/// fires correctly (the parser canonicalizes long-form to
/// `DissemControl::Relido`), but the wrapper's span lookup keys on the
/// abbreviation-form `t.text == "RELIDO"`, so long-form input surfaces
/// the diagnostic without a fix span. Tracked for follow-up; ICD long-form
/// is rare in practice.
///
/// `None` causes the caller to emit the `Diagnostic` without a fix —
/// preserving Constitution V (no malformed `FixProposal` ever leaves the
/// rule). The `Severity::Error` diagnostic still surfaces; the user
/// resolves it manually in the rare-but-real ambiguous-layout case.
///
/// `#[doc(hidden)] pub` — `pub` for integration-test access via the
/// re-export in `crate::rules`, but excluded from rendered docs because
/// this is implementation-detail support for E054–E057 (and the future
/// PR 3.7 RELIDO RhsFamily entries that inherit the subtractive-fix
/// pattern), not a stable public API. Same convention as the four
/// wrapper structs and as `marque_rules::AppliedFix::__engine_promote`.
#[doc(hidden)]
pub fn compute_relido_removal_span(attrs: &CanonicalAttrs) -> Option<(Span, Box<str>)> {
    let spans = &attrs.token_spans;
    // Find the RELIDO TokenSpan and its index for adjacency lookups.
    let (relido_idx, relido) = spans
        .iter()
        .enumerate()
        .find(|(_, t)| t.kind == TokenKind::DissemControl && &*t.text == "RELIDO")?;

    // Adjacency model — the parser's actual TokenSpan emission pattern
    // (verified against `marque check` output 2026-05-08):
    //
    //   - Intra-block `/` separators: NOT emitted as TokenSpans.
    //     Adjacent dissem-control content tokens carry adjacent byte
    //     offsets (`prior.span.end + 1 == curr.span.start`); the `/`
    //     occupies the gap byte but has no span of its own.
    //   - Cross-category `//` separators: emitted as
    //     `TokenKind::Separator` spans with `text == "//"`, occupying
    //     two bytes between the bordering content tokens.
    //
    // The earlier byte-offset-only model (`prior.end + 1 ==
    // relido.start`) handled intra-block adjacency correctly but missed
    // the cross-category case. The earlier Separator-only model handled
    // cross-category but broke intra-block. The combined model below
    // checks BOTH: prior content-token byte-adjacency for case 1, prior
    // Separator span for case 3.
    let prev = relido_idx.checked_sub(1).and_then(|i| spans.get(i));
    let next = spans.get(relido_idx + 1);

    // Case 1: middle / last in dissem block — prior is a `/`-adjacent
    // **content** token (no Separator span between, intra-block).
    let preceded_by_single_slash =
        prev.is_some_and(|p| p.kind != TokenKind::Separator && p.span.end + 1 == relido.span.start);
    if preceded_by_single_slash {
        // Removal span = [relido.start - 1, relido.end]. Original = "/RELIDO".
        let start = relido.span.start.checked_sub(1)?;
        let end = relido.span.end;
        return Some((Span::new(start, end), "/RELIDO".into()));
    }

    // Case 2: first in dissem block — following is a `/`-adjacent
    // **content** token (intra-block sibling).
    let followed_by_single_slash =
        next.is_some_and(|n| n.kind != TokenKind::Separator && n.span.start == relido.span.end + 1);
    if followed_by_single_slash {
        // Removal span = [relido.start, relido.end + 1]. Original = "RELIDO/".
        let start = relido.span.start;
        let end = relido.span.end.checked_add(1)?;
        return Some((Span::new(start, end), "RELIDO/".into()));
    }

    // Case 3: sole dissem in `//`-delimited category — prior is a
    // double-slash Separator (`prev.text == "//"`, `prev.span.end ==
    // relido.span.start`). Consume both preceding `/`s so the stranded
    // category separator goes with the payload. Covers banner-form
    // `... // <other-cat> // RELIDO` AND portion-form `(... // RELIDO)`.
    let preceded_by_double_slash = prev.is_some_and(|p| {
        p.kind == TokenKind::Separator && &*p.text == "//" && p.span.end == relido.span.start
    });
    if preceded_by_double_slash {
        // Removal span = [relido.start - 2, relido.end]. Original = "//RELIDO".
        let start = relido.span.start.checked_sub(2)?;
        let end = relido.span.end;
        return Some((Span::new(start, end), "//RELIDO".into()));
    }

    // No recognized layout — defensive fall-through to None.
    None
}

/// Build a subtractive RELIDO `FixProposal` for the four §H.8 conflict
/// wrappers. Returns `None` when `compute_relido_removal_span` cannot find
/// a sound removal layout (rare; caller emits the diagnostic without a fix
/// in that case so Constitution V's "never emit a malformed fix" invariant
/// holds).
///
/// Confidence is fixed at **0.95** per PM Addendum II §3 (post-2026-05-08
/// calibration) so the fix clears the engine's default
/// `Config::confidence_threshold` of 0.95 (`crates/config/src/lib.rs:156`,
/// auto-apply gate is `confidence >= threshold`). The §-cited prose in
/// every E054–E057 case is categorical ("Cannot be used with..." / "May
/// not be used with RELIDO"); the marking IS invalid and the user has
/// explicitly endorsed RELIDO as the remove-target. 0.95 matches the
/// established CAPCO convention for definite, at-threshold, auto-apply
/// fixes (e.g. `crates/capco/src/rules.rs:998 / :1327 / :2622 / :2777 /
/// :2853`); 0.85–0.9 is reserved for conditional / lower-confidence cases.
///
/// The earlier 0.9 value left the fix as a manual-review suggestion under
/// the default threshold — opposite of the user-stated guidance behavior
/// ("remove RELIDO and tell them why"). Bumped to 0.95 in PR 3b.C
/// pre-merge.
///
/// `FixSource::BuiltinRule` is the existing strict-path provenance variant
/// for hand-written CAPCO rules (the PM Addendum II §4 reference to
/// `FixSource::Rule { rule_id }` was nomenclature-only — no such variant
/// exists in `marque-rules`; `BuiltinRule` is the existing-pattern match).
fn build_relido_removal_fix(rule_id: RuleId, attrs: &CanonicalAttrs) -> Option<FixProposal> {
    let (span, original) = compute_relido_removal_span(attrs)?;
    Some(FixProposal::new(
        rule_id,
        FixSource::BuiltinRule,
        span,
        original,
        "",
        Confidence::strict(0.95),
        None,
    ))
}

// ---------------------------------------------------------------------------
// E054 — RELIDO conflicts with NOFORN (§H.8 p154)
// ---------------------------------------------------------------------------

// `pub` so the integration tests in `crates/capco/tests/relido_conflicts.rs`
// can instantiate these wrappers via the `pub use` re-export in
// `crate::rules` (integration tests link the crate as an external
// dependency and only see `pub` items). `#[doc(hidden)]` signals
// "technically pub for compilation but not stable public API" — the same
// convention `marque_rules::AppliedFix::__engine_promote` uses (Constitution
// V Principle V test-fixture carve-out). Future refactors are free to
// consolidate or rename these without a breaking-change concern.
#[doc(hidden)]
pub struct DeclarativeRelidoNofornConflictRule;

impl Rule for DeclarativeRelidoNofornConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E054")
    }

    fn name(&self) -> &'static str {
        "relido-noforn-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E054/relido-conflicts-noforn").is_empty() {
            return vec![];
        }

        // Diagnostic-anchor span — the user's cursor lands here. RELIDO is
        // the asserting token per §H.8 p154 ("Cannot be used with NOFORN
        // or DISPLAY ONLY."). Fall back to NOFORN/NF if RELIDO span is
        // unavailable; final fallback Span::new(0, 0).
        //
        // Token text in attrs.token_spans: "RELIDO" (DissemControl::Relido),
        // "NOFORN" or "NF" (DissemControl::Nf).
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "RELIDO")
            .or_else(|| {
                attrs.token_spans.iter().find(|t| {
                    t.kind == TokenKind::DissemControl && (&*t.text == "NOFORN" || &*t.text == "NF")
                })
            })
            .map(|t| t.span)
            .unwrap_or_else(|| Span::new(0, 0));

        // Subtractive fix: remove RELIDO. NOFORN dominates per FD&R
        // supersession (§D.2 Table 3 + §H.8 p145 NOFORN entry: "Cannot be
        // used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY"). NOFORN
        // is the binding constraint, so the only well-defined fix is to
        // remove the rejected token (RELIDO). PM Addendum II §3.
        let fix = build_relido_removal_fix(self.id(), attrs);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RELIDO removed: cannot be used with NOFORN (§H.8 p154)",
            "CAPCO-2016 §H.8 p154",
            fix,
        )]
    }
}

// ---------------------------------------------------------------------------
// E055 — RELIDO conflicts with DISPLAY ONLY (§H.8 p154)
// ---------------------------------------------------------------------------

// `#[doc(hidden)] pub` for the same reason as
// `DeclarativeRelidoNofornConflictRule` above — integration-test access
// via `crate::rules` re-export, not a stable public API.
#[doc(hidden)]
pub struct DeclarativeRelidoDisplayOnlyConflictRule;

impl Rule for DeclarativeRelidoDisplayOnlyConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E055")
    }

    fn name(&self) -> &'static str {
        "relido-display-only-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E055/relido-conflicts-display-only").is_empty() {
            return vec![];
        }

        // Diagnostic-anchor span — the user's cursor lands here. RELIDO is
        // the asserting token per §H.8 p154 ("Cannot be used with NOFORN
        // or DISPLAY ONLY.").
        // Fall back to DISPLAY ONLY span. Note: the CVE abbreviation for
        // DISPLAY ONLY in token_spans.text is "DISPLAYONLY" (no space) —
        // this matches `DissemControl::Displayonly::as_str()` in generated
        // values.rs. The canonical portion form per CAPCO-2016 §H.8 p161
        // is "DISPLAY ONLY [LIST]" in the banner, but the parser stores the
        // CVE abbreviation in `TokenSpan::text`.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "RELIDO")
            .or_else(|| {
                attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "DISPLAYONLY")
            })
            .map(|t| t.span)
            .unwrap_or_else(|| Span::new(0, 0));

        // Subtractive fix: remove RELIDO. DISPLAY ONLY is a positive
        // disclosure decision (specific country list); RELIDO defers
        // release to a SFDRA. The deferred decision can't operate when a
        // positive decision is already on the marking — DISPLAY ONLY is
        // the binding constraint. PM Addendum II §3.
        let fix = build_relido_removal_fix(self.id(), attrs);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RELIDO removed: cannot be used with DISPLAY ONLY (§H.8 p154)",
            "CAPCO-2016 §H.8 p154",
            fix,
        )]
    }
}

// ---------------------------------------------------------------------------
// E056 — ORCON conflicts with RELIDO (§H.8 p136)
// ---------------------------------------------------------------------------

// `#[doc(hidden)] pub` for the same reason as
// `DeclarativeRelidoNofornConflictRule` above — integration-test access
// via `crate::rules` re-export, not a stable public API.
#[doc(hidden)]
pub struct DeclarativeOrconRelidoConflictRule;

impl Rule for DeclarativeOrconRelidoConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E056")
    }

    fn name(&self) -> &'static str {
        "orcon-relido-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E056/orcon-conflicts-relido").is_empty() {
            return vec![];
        }

        // Diagnostic-anchor span — the user's cursor lands at ORCON. The
        // asserting prose lives on the ORCON template at §H.8 p136 ("May
        // not be used with RELIDO."). Anchoring at ORCON shows the user
        // the token that contains the prohibition; the fix span (below)
        // covers RELIDO + adjacent separator regardless of the anchor.
        // Note: the CVE abbreviation for ORCON in token_spans.text is "OC"
        // (DissemControl::Oc::as_str()). Fall back to RELIDO span.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "OC")
            .or_else(|| {
                attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "RELIDO")
            })
            .map(|t| t.span)
            .unwrap_or_else(|| Span::new(0, 0));

        // Subtractive fix: remove RELIDO. §H.8 p136 explicitly asserts
        // "May not be used with RELIDO" on the ORCON template — RELIDO is
        // the rejected token. ORCON requires originator approval for
        // further dissemination, which RELIDO's SFDRA-deferred release
        // bypasses. ORCON is the binding constraint. PM Addendum II §3.
        let fix = build_relido_removal_fix(self.id(), attrs);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RELIDO removed: ORCON may not be used with RELIDO (§H.8 p136)",
            "CAPCO-2016 §H.8 p136",
            fix,
        )]
    }
}

// ---------------------------------------------------------------------------
// E057 — ORCON-USGOV conflicts with RELIDO (§H.8 p140)
// ---------------------------------------------------------------------------

// `#[doc(hidden)] pub` for the same reason as
// `DeclarativeRelidoNofornConflictRule` above — integration-test access
// via `crate::rules` re-export, not a stable public API.
#[doc(hidden)]
pub struct DeclarativeOrconUsgovRelidoConflictRule;

impl Rule for DeclarativeOrconUsgovRelidoConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E057")
    }

    fn name(&self) -> &'static str {
        "orcon-usgov-relido-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        if violations_for(attrs, "E057/orcon-usgov-conflicts-relido").is_empty() {
            return vec![];
        }

        // Diagnostic-anchor span — the user's cursor lands at ORCON-USGOV.
        // The asserting prose lives on the ORCON-USGOV template at §H.8
        // p140 ("May not be used with RELIDO."). Note: the CVE abbreviation
        // in token_spans.text is "OC-USGOV" (DissemControl::OcUsgov::as_str()).
        // Fall back to RELIDO span.
        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "OC-USGOV")
            .or_else(|| {
                attrs
                    .token_spans
                    .iter()
                    .find(|t| t.kind == TokenKind::DissemControl && &*t.text == "RELIDO")
            })
            .map(|t| t.span)
            .unwrap_or_else(|| Span::new(0, 0));

        // Subtractive fix: remove RELIDO. §H.8 p140 explicitly asserts
        // "May not be used with RELIDO" on the ORCON-USGOV template —
        // RELIDO is the rejected token. ORCON-USGOV is the
        // USGOV-pre-approved variant of ORCON; same originator-approval
        // semantic conflict with RELIDO's SFDRA-deferred release.
        // ORCON-USGOV is the binding constraint. PM Addendum II §3.
        let fix = build_relido_removal_fix(self.id(), attrs);

        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            span,
            "RELIDO removed: ORCON-USGOV may not be used with RELIDO (§H.8 p140)",
            "CAPCO-2016 §H.8 p140",
            fix,
        )]
    }
}
