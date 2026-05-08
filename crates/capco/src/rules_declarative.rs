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

/// Find the first `TokenSpan` whose kind is `DissemControl` AND whose
/// text matches any of the supplied forms.
///
/// # Form taxonomy
///
/// CAPCO-2016 §G.1 Table 4 (p36) and the §H.8 per-marking templates
/// distinguish **three marking-surface forms** for each dissem control:
///
/// 1. **Authorized Banner Line Marking Title** — the long surface form
///    that appears in banner lines. Example: `ORCON`, `ORCON-USGOV`,
///    `NOFORN`, `DISPLAY ONLY`.
/// 2. **Authorized Banner Line Abbreviation** — a short banner form
///    when one is registered (Table 4 column 2). Example: `OC`,
///    `OC-USGOV`, `NF`. For `DISPLAY ONLY` this column is **`None`**
///    per §H.8 p163 — there is no abbreviation; banner form is the
///    long surface string.
/// 3. **Authorized Portion Mark** — the form rendered inside `(...)`
///    (Table 4 column 3). Equals the banner abbreviation when one
///    exists, otherwise equals the banner long form. For
///    `DISPLAY ONLY` the portion mark is `DISPLAY ONLY [LIST]` per
///    §H.8 p163.
///
/// Plus a fourth form-space, orthogonal to the marking surface:
///
/// 4. **ODNI ISM XML CVE attribute value** — the data shape used in
///    `ism:disseminationControls="..."`. All-uppercase, no spaces.
///    `DissemControl::as_str()` returns this form. Example: `"OC"`,
///    `"OC-USGOV"`, `"NF"`, `"DISPLAYONLY"` (no space). The CVE form
///    matches the marking-surface portion mark for entries where the
///    portion is itself a short token (`OC`, `NF`), but diverges where
///    the marking surface contains spaces or is the long form (`DISPLAY
///    ONLY` vs `DISPLAYONLY`). Marque accepts CVE-form input on the
///    lookup chain so that a future programmatic / re-import path
///    feeding `ism:disseminationControls` values back through the rule
///    engine round-trips cleanly.
///
/// The parser preserves raw user input verbatim in `TokenSpan::text`
/// (see `crates/core/src/parser.rs` — every push uses `text:
/// trimmed.into()` with no canonicalization), so callers anchoring at a
/// dissem-control token MUST enumerate every form a user (or an XML
/// re-import) might have written: banner long form, banner abbreviation
/// (when distinct), portion mark, AND CVE attribute value.
///
/// # Engine gap (tracked at #323)
///
/// `crates/ism/src/marking_forms.rs::MARKING_FORMS` has no DISPLAY ONLY
/// entry, and `DissemControl::parse` only matches the CVE string
/// `"DISPLAYONLY"`. So today the parser cannot tokenize `DISPLAY ONLY`
/// (with space) as a `DissemControl` — only the CVE form is recognized.
/// The `"DISPLAY ONLY"` form in this lookup is forward-looking until
/// that gap closes (separate `marque-ism` PR per Constitution VII
/// Principle IV; tracked at #323).
///
/// `#[doc(hidden)]` because this is an internal layout helper for the
/// RELIDO incompatibility wrappers (E054–E057), not a stable public
/// API. Same convention as the four wrapper structs and the
/// `compute_relido_removal_span` helper.
#[doc(hidden)]
pub fn find_dissem_token_span(attrs: &CanonicalAttrs, forms: &[&str]) -> Option<Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DissemControl && forms.contains(&&*t.text))
        .map(|t| t.span)
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
// E022 — CNWDI classification floor (RETIRED)
// ---------------------------------------------------------------------------
//
// PR 3b.D (T026d): retired. The CNWDI floor invariant moved into the
// class-floor catalog as the row `E058/CNWDI-classification-floor`
// (CAPCO §H.6 p104). The catalog walker
// `DeclarativeClassFloorRule` (rule ID `E058`) is the new emitter;
// per-row identification (which catalog row fired) lives in the
// walker's emitted `Diagnostic.message` text.
//
// The legacy `E022` rule ID is NOT preserved as a severity-config
// alias. Per project memory
// `feedback_pre_users_no_deprecation_phasing.md`: marque is
// pre-users; we don't carry alias maps or retained namespaces.
// `.marque.toml` files keying class-floor severity overrides MUST
// use `E058` (the walker-level ID).
//
// See `crate::scheme::CLASS_FLOOR_CATALOG` for the row's predicate +
// citation, and `DeclarativeClassFloorRule` below for the walker.

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
// E025 — UCNI only with UNCLASSIFIED (RETIRED)
// ---------------------------------------------------------------------------
//
// PR 3b.D (T026d): retired. The UCNI ceiling invariant moved into the
// class-floor catalog as TWO rows
// (`E058/DOD-UCNI-classification-ceiling` at CAPCO §H.6 p116 and
// `E058/DOE-UCNI-classification-ceiling` at §H.6 p118 — split per PM
// decision so each variant has its own §H.6 sub-page citation). The
// catalog walker `DeclarativeClassFloorRule` (rule ID `E058`) is the
// new emitter.
//
// The legacy `E025` rule ID is NOT preserved as a severity-config
// alias. Per project memory
// `feedback_pre_users_no_deprecation_phasing.md`: marque is
// pre-users; we don't carry alias maps or retained namespaces.
// `.marque.toml` files keying class-floor severity overrides MUST
// use `E058` (the walker-level ID).
//
// See `crate::scheme::CLASS_FLOOR_CATALOG` for the row predicates +
// citations, and `DeclarativeClassFloorRule` below for the walker.

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
        // or DISPLAY ONLY."). Fall back to NOFORN if RELIDO span is
        // unavailable; final fallback Span::new(0, 0).
        //
        // Surface forms in `attrs.token_spans` (per parser raw-text
        // storage and §H.8 templates):
        //   RELIDO: only `"RELIDO"` — the banner long name and the CVE
        //           portion abbreviation are identical
        //           (`DissemControl::Relido::as_str()` returns "RELIDO").
        //   NOFORN: `"NOFORN"` (banner long name, §H.8 p145) or
        //           `"NF"` (CVE portion abbreviation,
        //           `DissemControl::Nf::as_str()`).
        let span = find_dissem_token_span(attrs, &["RELIDO"])
            .or_else(|| find_dissem_token_span(attrs, &["NOFORN", "NF"]))
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
        //
        // Forms in `attrs.token_spans` (per parser raw-text storage; the
        // parser preserves user input verbatim, no canonicalization):
        //   RELIDO:       only `"RELIDO"` — the banner long form, the
        //                 portion mark, AND the CVE attribute value all
        //                 render identically.
        //   DISPLAY ONLY: marking-surface form is `"DISPLAY ONLY"` (with
        //                 space) — used in BOTH banner and portion per
        //                 §H.8 p163 ("Authorized Banner Line Abbreviation:
        //                 None"; "Authorized Portion Mark: DISPLAY ONLY
        //                 [LIST]"). The CVE attribute value is
        //                 `"DISPLAYONLY"` (no space, all-caps;
        //                 `DissemControl::Displayonly::as_str()` per ODNI
        //                 `CVEnumISMDissem.xml`). Both are accepted on
        //                 input — see `find_dissem_token_span` doc for
        //                 the form taxonomy and the engine gap (tracked
        //                 at #323).
        //
        // The DISPLAY ONLY fallback branch is unreachable in correctness
        // terms — the rule's pre-condition (the dyadic `Conflicts`
        // predicate) guarantees RELIDO is in `attrs.dissem_controls` and
        // therefore in `token_spans`. The branch exists for invariant-
        // drift safety: if a future parser change ever elides the RELIDO
        // span on a triggering input, anchoring at DISPLAY ONLY is the
        // least-bad fallback. Per PM Addendum II Q1, the primary anchor
        // stays at RELIDO regardless.
        let span = find_dissem_token_span(attrs, &["RELIDO"])
            .or_else(|| find_dissem_token_span(attrs, &["DISPLAY ONLY", "DISPLAYONLY"]))
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
        // the token that contains the prohibition; the fix span (built
        // by `compute_relido_removal_span`) covers RELIDO + adjacent
        // separator regardless of the anchor.
        //
        // Surface forms in `attrs.token_spans` (per parser raw-text
        // storage; the parser preserves user input verbatim, no
        // canonicalization):
        //   ORCON: `"ORCON"` (banner long name, §H.8 p136) or `"OC"`
        //          (CVE portion abbreviation, `DissemControl::Oc::as_str()`).
        //   RELIDO: only `"RELIDO"`.
        //
        // The ORCON template is the §-asserting side per PM Addendum II
        // Q1; missing the banner-form lookup would silently fall back to
        // the RELIDO anchor for canonical banner-shaped input
        // (`SECRET//ORCON/RELIDO` — `/` separates same-category dissem
        // values per §A.6 Figure 2 p17), which is the wrong cursor
        // location.
        let span = find_dissem_token_span(attrs, &["ORCON", "OC"])
            .or_else(|| find_dissem_token_span(attrs, &["RELIDO"]))
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
        // p140 ("May not be used with RELIDO.").
        //
        // Surface forms in `attrs.token_spans` (per parser raw-text
        // storage; the parser preserves user input verbatim):
        //   ORCON-USGOV: `"ORCON-USGOV"` (banner long name, §H.8 p140)
        //                or `"OC-USGOV"` (CVE portion abbreviation,
        //                `DissemControl::OcUsgov::as_str()`).
        //   RELIDO:      only `"RELIDO"`.
        //
        // Same banner-form rationale as E056: missing the banner-form
        // lookup would silently fall back to the RELIDO anchor for
        // canonical banner-shaped input
        // (`SECRET//ORCON-USGOV/RELIDO` — `/` separates same-category
        // dissem values per §A.6 Figure 2 p17), which is the wrong
        // cursor location per PM Addendum II Q1.
        let span = find_dissem_token_span(attrs, &["ORCON-USGOV", "OC-USGOV"])
            .or_else(|| find_dissem_token_span(attrs, &["RELIDO"]))
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

// ===========================================================================
// PR 3b.D (T026d) — Class-floor catalog walker (E058)
// ===========================================================================
//
// `DeclarativeClassFloorRule` is the single walker rule that dispatches
// over the 27-row class-floor catalog declared in
// `crate::scheme::CLASS_FLOOR_CATALOG` (and registered as
// `Constraint::Custom` rows in `CapcoScheme::build_constraints` under
// the "PR 3b.D (T026d) — class-floor catalog" section).
//
// # Walker rule-ID convention
//
// Per the PR 3b.D planning doc §5.2 + PM directive #5: ONE walker rule
// `E058` with a fresh ID. All emitted diagnostics carry
// `Diagnostic.rule = "E058"`. Per-row identification flows via the
// catalog row's `name` field — either `"E058/<purpose>"` (for the
// four rows replacing retired E022/E025/E027 invariants —
// `E058/CNWDI-classification-floor`, `E058/SAR-classification-floor`,
// `E058/DOD-UCNI-classification-ceiling`,
// `E058/DOE-UCNI-classification-ceiling`) or
// `"class-floor/<marking>"` (for the 23 new family rows with no
// retired-rule predecessor) — into the diagnostic message text. The
// legacy E022 / E025 / E027 IDs are NOT preserved as severity-config
// aliases (per `feedback_pre_users_no_deprecation_phasing.md`:
// marque is pre-users; rewrite freely).
//
// # Severity convention
//
// The walker's `default_severity()` is `Severity::Error` (matches the
// majority of catalog rows). Per-row severities are stored in
// `ClassFloorRow.severity` and copied onto each emitted `Diagnostic`
// — the unknown-floor passthrough rows (BUR / HCS-X / KLM / MVL) emit
// at `Severity::Warn` per `marque-applied.md` §3.4.6 Q-3.4.6b. The
// engine's severity-override layer can downgrade or upgrade per
// `.marque.toml [rules] E058 = "off|warn|error|..."`.
//
// # Span anchoring
//
// PM directive #2: anchor at the marking token, not the classification
// token. The diagnostic squiggle should be under the offending presence,
// not the classification value. Span resolution per row dispatches on
// the marking axis: AEA-axis rows (RD, FRD, TFNI, CNWDI, SIGMA, UCNI)
// anchor at the first `TokenKind::AeaMarking` span; SCI-axis rows
// (HCS, SI, TK, RSV, BUR, HCS-X, KLM, MVL) anchor at the first
// `TokenKind::SciSystem` or `TokenKind::SciControl` span; SAR rows at
// `TokenKind::SarIndicator`; dissem-axis rows (RSEN, IMCON, ORCON,
// EYES) at the first `TokenKind::DissemControl` span; NATO rows at the
// first `TokenKind::Classification` span (NATO classification token is
// the marking surface). When no specific token-kind span is found, fall
// back to the first `Classification` span, and finally to `(0, 0)`.

pub(crate) struct DeclarativeClassFloorRule;

impl Rule for DeclarativeClassFloorRule {
    fn id(&self) -> RuleId {
        RuleId::new("E058")
    }
    fn name(&self) -> &'static str {
        "class-floor-catalog"
    }
    fn default_severity(&self) -> Severity {
        // Catalog rows individually carry `Severity::Error` (enumerated
        // rows) or `Severity::Warn` (passthrough rows); each row's
        // severity is stored in `ClassFloorRow.severity` and is what
        // the emitted `Diagnostic.severity` carries when no
        // `.marque.toml` override is configured for `E058`.
        //
        // `default_severity` governs the no-override case ONLY. If a
        // user sets `[rules] E058 = "warn"`, the engine's severity-
        // override layer replaces every emitted `Diagnostic.severity`
        // with `Warn` regardless of the per-row authoring intent — so
        // this default value cannot prevent downgrading. Returning
        // `Severity::Error` here matches the strictest per-row floor
        // so an unconfigured catalog defaults to error-severity for
        // the enumerated rows; passthrough rows still emit at `Warn`
        // because the walker copies `row.severity` onto each
        // `Diagnostic` directly (see `check` below).
        //
        // A per-row severity floor mechanism (preventing config from
        // downgrading specific rows below their authoring intent) does
        // not exist in the engine and is not in scope for PR D.
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // PR D R2 perf-1: per-portion early-out guard. Pre-compute
        // axis-presence flags once. On a 10KB document where most
        // portions are prose body text (no SCI / AEA / SAR / dissem /
        // NATO classification), all five flags are `false` and the
        // catalog walk is skipped entirely. The flags are O(1) each
        // (Box<[T]> length checks + one classification-variant match).
        let any_sci = !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty();
        let any_aea = !attrs.aea_markings.is_empty();
        let any_sar = attrs.sar_markings.is_some();
        let any_dissem = !attrs.dissem_controls.is_empty();
        let any_nato_class = matches!(
            &attrs.classification,
            Some(marque_ism::MarkingClassification::Nato(_))
        );
        if !(any_sci || any_aea || any_sar || any_dissem || any_nato_class) {
            return Vec::new();
        }

        // PR D R2 perf-2: direct catalog-row dispatch. Walk the static
        // `CLASS_FLOOR_CATALOG` table once; for each row whose axis is
        // present, fire the row's predicate via `class_floor_eval_row`
        // (which calls `(row.presence)(attrs)` and
        // `class_floor_satisfied(attrs, row.policy)` directly — no
        // string-keyed dispatch through `evaluate_custom_by_attrs`).
        let mut diags = Vec::new();
        for row in crate::scheme::class_floor_catalog() {
            // Axis-empty short-circuit: skip rows whose axis carries
            // no tokens in this portion. The walker can then call
            // `class_floor_eval_row` only for rows whose axis is
            // populated.
            let axis_present = match row.axis {
                crate::scheme::ClassFloorAxis::Sci => any_sci,
                crate::scheme::ClassFloorAxis::Aea => any_aea,
                crate::scheme::ClassFloorAxis::Sar => any_sar,
                crate::scheme::ClassFloorAxis::Dissem => any_dissem,
                crate::scheme::ClassFloorAxis::NatoClass => any_nato_class,
            };
            if !axis_present {
                continue;
            }
            let Some(message) = crate::scheme::class_floor_eval_row(attrs, row) else {
                continue;
            };
            // PR D R2 perf-3: span anchor read from `row.primary_kind`
            // (hoisted from the previous `primary_token_kind_for_row`
            // string match into a struct field).
            let span = class_floor_anchor_span(attrs, row);
            diags.push(Diagnostic::new(
                self.id(),
                row.severity,
                span,
                message,
                row.citation,
                None,
            ));
        }
        diags
    }
}

/// Resolve the diagnostic span anchor for a class-floor catalog row.
///
/// Per PM directive #2, the span anchors at the marking token (not the
/// classification token) so the diagnostic UX puts the squiggle under
/// the offending presence. PR D R2 perf-3: reads
/// `row.primary_kind` directly (hoisted from the previous
/// `primary_token_kind_for_row` string-match table into a struct
/// field on `ClassFloorRow`). Falls back to the first
/// `Classification` token span if no axis-specific span is found, and
/// finally to `Span::new(0, 0)` if neither is present.
fn class_floor_anchor_span(attrs: &CanonicalAttrs, row: &crate::scheme::ClassFloorRow) -> Span {
    if let Some(kind) = row.primary_kind
        && let Some(span) = first_span_of_optional(attrs, kind)
    {
        return span;
    }
    // Some rows have no single primary kind (e.g., NATO rows have no
    // marking-side token; `row.primary_kind == None`). Try
    // classification as a fallback.
    if let Some(span) = first_span_of_optional(attrs, TokenKind::Classification) {
        return span;
    }
    Span::new(0, 0)
}

/// Variant of `first_span_of` that returns `Option` instead of
/// substituting `Span::new(0, 0)` for "no token". Used by the
/// class-floor span-anchor resolver to chain fallbacks.
fn first_span_of_optional(attrs: &CanonicalAttrs, kind: TokenKind) -> Option<Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| t.kind == kind)
        .map(|t| t.span)
}

// ===========================================================================
// PR 3b.E (T026e) — SCI per-system catalog walker (E059)
// ===========================================================================
//
// `DeclarativeSciPerSystemRule` is the single walker that dispatches over
// the 5-row SCI per-system catalog declared in
// `crate::scheme::SCI_PER_SYSTEM_CATALOG` (and registered as
// `Constraint::Custom` rows in `CapcoScheme::build_constraints` under the
// "PR 3b.E (T026e) — SCI per-system catalog (§H.4)" section).
//
// # Walker rule-ID convention
//
// Per the PR 3b.E planning doc §4.2 + PM directive: ONE walker rule
// `E059` with a fresh ID. All emitted diagnostics carry
// `Diagnostic.rule = "E059"`. Per-row identification flows via the
// catalog row's `name` field (always `sci-per-system/<purpose>`) into
// the diagnostic message text. The legacy `E042`–`E051` IDs are NOT
// preserved as severity-config aliases (per
// `feedback_pre_users_no_deprecation_phasing.md`: marque is pre-users;
// rewrite freely).
//
// # Severity convention
//
// The walker's `default_severity()` is `Severity::Warn` (matches the
// per-row authoring intent on every PR-E row). Per-row severities are
// stored in `SciPerSystemRow.severity` and the emit helper escalates
// per-branch to `Severity::Error` no-fix when no IC dissem block exists
// (companion-insertion would need to synthesize a whole `//`-separated
// category from rule context, which is unsafe; same policy as E040).
// The engine's severity-override layer can downgrade or upgrade per
// `.marque.toml [rules] E059 = "off|warn|error|..."`.
//
// # Span anchoring (varies by emit-branch shape)
//
// **Companion-insertion branches** (missing ORCON / missing NOFORN):
// the diagnostic anchors at the offending SCI marking token via
// `first_sci_span(attrs)` (which walks `attrs.token_spans` and returns
// the span of the first `TokenKind::SciSystem` / `SciControl` /
// `SciCompartment` / `SciSubCompartment` token in document order). The
// fix span is a zero-width insertion at the end of the IC dissem block
// — i.e., the diagnostic and fix span differ, and the user sees the SCI
// marking that triggered the requirement while the edit applies at the
// dissem-block anchor where the insertion belongs. Same diagnostic-vs-
// fix-span split used by `SarPortionFormRule` (E026).
//
// **Token-replacement branches** (e.g., HCS-O / HCS-P-sub / SI-G with
// ORCON-USGOV present → replace with ORCON): both the diagnostic and
// the fix anchor on the offending dissem token's own span so the user
// sees the dissem token directly. There is no SCI-vs-dissem split for
// these branches.
//
// `first_sci_span` returns the lexically-first SCI token regardless of
// which row matched — preserved verbatim from the legacy E042–E051
// rules (a pre-existing imperfection; on a multi-marking portion like
// `(TS//SI-G HCS-O//OC-USGOV/NF)` the row #1 (HCS-O) diagnostic anchors
// at `SI-G`). PR 4's per-category Lattice impls + dedicated span-
// resolution machinery are expected to address this.

pub(crate) struct DeclarativeSciPerSystemRule;

impl Rule for DeclarativeSciPerSystemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E059")
    }
    fn name(&self) -> &'static str {
        "sci-per-system-catalog"
    }
    fn default_severity(&self) -> Severity {
        // Catalog rows individually carry `Severity::Warn` (the fix-and-
        // warn pattern from the legacy E042–E051 cluster). The emit
        // helper escalates per-branch to `Severity::Error` no-fix when
        // no IC dissem block exists. `default_severity` governs the
        // no-override case ONLY — if a user sets
        // `[rules] E059 = "error"`, the engine's severity-override
        // layer replaces every emitted `Diagnostic.severity` with
        // `Error`. A per-row severity floor mechanism (preventing
        // config from downgrading specific rows below their authoring
        // intent) does not exist in the engine and is not in scope for
        // PR E.
        Severity::Warn
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic> {
        // PR 3b.E perf-1: per-portion early-out guard. All PR-E rows
        // are SCI-axis-only — if `attrs.sci_markings` is empty, no row
        // can fire and the catalog walk is skipped entirely. On a 10KB
        // document where most portions are prose body text (no SCI
        // markings), this is a single boolean check that costs
        // effectively nothing.
        if attrs.sci_markings.is_empty() {
            return Vec::new();
        }

        // PR 3b.E perf-2: direct catalog-row dispatch. Walk the static
        // catalog table; for each row whose presence predicate fires,
        // call `sci_per_system_emit` with the row in hand — no string-
        // keyed lookup, no wrapper indirection. The explicit per-row
        // presence check elides the function-call overhead for non-
        // firing rows; `sci_per_system_emit` also re-checks presence
        // internally (idempotent — predicates are pure functions of
        // `attrs`) so the trait/validate path through
        // `sci_per_system_catalog_eval`, which calls emit without
        // going through this walker, stays correct.
        let mut diags = Vec::new();
        for row in crate::scheme::sci_per_system_catalog() {
            if !(row.presence)(attrs) {
                continue;
            }
            let row_diags = crate::scheme::sci_per_system_emit(attrs, row);
            diags.extend(row_diags);
        }
        diags
    }
}

// ===========================================================================
// PR 3b.F (T026f) — Non-canonical input walker (E060)
// ===========================================================================
//
// `DeclarativeNonCanonicalInputRule` is a single hand-written walker that
// dispatches over a private `&'static [NonCanonicalRow]` catalog declared
// in this file. It collapses four retired ordering-validation rules:
//
//   E020 CountryCodeOrderingRule    (REL TO + JOINT alphabetical, §H.8 + §H.3)
//   E023 SigmaValidationRule        (AEA SIGMA valid set + numeric sort, §H.6)
//   E028 SarProgramOrderRule        (SAR programs ascending, §H.5)
//   E033 SciCompartmentOrderRule    (SCI compartment / sub-compartment, §H.4)
//
// PR 3b.F (T026f) — Non-canonical input walker.
//
// This walker exists as a STAGE-1 INTERIM. The four ordering rules
// collapsed here (E020 REL TO + JOINT alpha, E023 SIGMA numeric sort,
// E028 SAR ascending alpha, E033 SCI compartment + sub-compartment
// alpha) are renderer-canonical-form concerns per `marque-applied.md`
// §3.6 + §3.10 Move 7. Once `MarkingScheme::render_canonical` lands
// in PR 5+ (Stage 4 of the engine refactor) the renderer absorbs
// canonical-form rendering, and "your input doesn't match the
// canonical form" becomes a normalization fix in the renderer's
// correctness surface, not a `Rule`.
//
// When that happens, this entire walker — `DeclarativeNonCanonicalInputRule`,
// the `NON_CANONICAL_CATALOG` table, and the per-row evaluators — retires
// cleanly. The audit-stream consumers must keep working through the
// transition: the renderer-emitted normalization fix carries a
// `FixProposal` with the same shape as today's walker emits (span +
// replacement + confidence + source), and `Engine::fix_inner` continues
// to be the sole `AppliedFix::__engine_promote` caller. See
// `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md` for
// the architectural rationale; `docs/plans/2026-05-02-engine-refactor-
// consolidated.md` Stage 4 + `tasks.md` T026f checkbox for the
// retirement plan.
//
// # Why this is structurally different from PR 3b.D / 3b.E
//
// 3b.D and 3b.E declared their rows as `Constraint::Custom` on
// `CapcoScheme` because their invariants (class-floor partial-order
// thresholds; SCI per-system companion-required + forbid-companion)
// are *cross-axis predicates* over canonical attributes. PR 3b.F's
// invariants are not predicates over canonical attributes; they are
// *non-canonical input detection* — the invariant fires when the
// surface-form token order in the source bytes differs from the
// canonical representative, not when the canonical attributes
// themselves violate an algebraic law. The catalog therefore lives
// privately inside this walker module and the rows do NOT participate
// in `evaluate_custom_by_attrs` dispatch on the scheme.
//
// # Rule-ID convention
//
// ONE walker rule `E060` (verified next-free slot after PR 3b.E took
// E059). All emitted diagnostics carry `Diagnostic.rule = "E060"`.
// Per-row identification flows via:
//
//   - the diagnostic message text (preserved verbatim from the retired
//     rules — "REL TO country codes must be alphabetically ordered",
//     "SIGMA numbers must be in numerical order", etc.)
//   - the `Diagnostic.citation` field (per-row §-citation propagated
//     onto the diagnostic by the per-row evaluator)
//   - the `name` field on `NonCanonicalRow` (private to this module;
//     used only by tests via the engine's public lint surface, not as
//     a public API)
//
// The legacy E020 / E023 / E028 / E033 IDs are NOT preserved as
// severity-config aliases (per `feedback_pre_users_no_deprecation_phasing.md`:
// marque is pre-users; rewrite freely). `additional_emitted_ids()`
// returns `&[]`.
//
// # Severity convention
//
// The walker's `default_severity()` is `Severity::Error` — the
// strictest of the per-row defaults (matches PR 3b.A banner walker
// precedent: a config that uses `E060` as the override anchor cannot
// accidentally weaken any row below its authoring intent without an
// explicit user choice). Per-row severities are stored in
// `NonCanonicalRow.severity` and are what each `Diagnostic` carries
// when no config override engages: `Severity::Fix` for rows 1-4
// (REL TO / JOINT / SIGMA / SAR), `Severity::Error` for row 5 (SCI).
// The engine's severity-override layer can downgrade or upgrade per
// `[rules] E060 = "off|warn|error|..."`.

use marque_ism::{AeaMarking, MarkingClassification};

use crate::rules::{
    canonicalize_trigraph_list, check_trigraph_ordering, render_sar_block, sar_block_span,
};

/// One catalog row per non-canonical-input ordering invariant.
///
/// Ordering of rows in `NON_CANONICAL_CATALOG` controls only emit
/// order for a single candidate; correctness is independent of row
/// order.
struct NonCanonicalRow {
    /// Stable per-row identifier; used in tests that pin per-row
    /// behavior via the engine's public lint surface. Not emitted as
    /// the diagnostic's `rule` field — that's `E060` via
    /// `Rule::id()`. Contributes to the audit-stream traceability
    /// invariant (a reviewer can grep diagnostic message text or
    /// `Diagnostic.citation` to identify which row fired).
    ///
    /// Currently unused at runtime (the walker dispatches on
    /// `presence` + `evaluate` directly). Kept for symmetry with
    /// `SciPerSystemRow` / `ClassFloorRow` and as anchor for the
    /// catalog-pin test in `non_canonical_input_walker.rs`. Marked
    /// `#[allow(dead_code)]` rather than removed so a future audit-
    /// trail extension (e.g. emitting per-row name into the audit
    /// record) can populate it without re-introducing the field.
    #[allow(dead_code)]
    name: &'static str,
    /// Per-row default severity: copied onto each emitted diagnostic
    /// when the engine's severity-override layer is silent for
    /// `E060`.
    severity: Severity,
    /// Per-row §-citation; propagated onto each emitted
    /// `Diagnostic.citation` by the per-row evaluator. Verified
    /// against the vendored `crates/capco/docs/CAPCO-2016.md` per
    /// Constitution VIII; see `tests/non_canonical_input_walker.rs`
    /// for the per-row citation-fidelity test.
    citation: &'static str,
    /// Quick presence check; gates the per-row evaluator so the hot
    /// path skips rows whose axis is empty for this candidate.
    presence: fn(&CanonicalAttrs) -> bool,
    /// Per-row evaluator. Returns the diagnostics this row produces
    /// for the given attributes + context. Body is a verbatim move
    /// of the retired rule's `check` body (with the
    /// `self.id()` / `self.default_severity()` / inline-citation
    /// strings replaced by the row's stored values), so the
    /// diagnostic message text + fix shapes + spans are byte-
    /// identical to the retired rule's output.
    evaluate: fn(&CanonicalAttrs, &RuleContext, &NonCanonicalRow) -> Vec<Diagnostic>,
}

const NON_CANONICAL_CATALOG: &[NonCanonicalRow] = &[
    NonCanonicalRow {
        name: "non-canonical/rel-to-usa-first",
        severity: Severity::Fix,
        citation: concat!(
            "CAPCO-2016 §H.8 p150–151 ",
            "(REL TO: trigraphs alpha, then tetragraphs alpha, USA first)",
        ),
        presence: presence_rel_to_usa_first,
        evaluate: evaluate_rel_to_usa_first_alpha,
    },
    NonCanonicalRow {
        name: "non-canonical/joint-alphabetical",
        severity: Severity::Fix,
        citation: concat!(
            "CAPCO-2016 §H.3 p56 ",
            "(JOINT: trigraphs alpha, then tetragraphs alpha)",
        ),
        presence: presence_joint_alphabetical,
        evaluate: evaluate_joint_alphabetical,
    },
    NonCanonicalRow {
        name: "non-canonical/sigma-numeric-sort",
        severity: Severity::Fix,
        citation: "CAPCO-2016 §H.6 p108",
        presence: presence_sigma_numeric_sort,
        evaluate: evaluate_sigma_numeric_sort,
    },
    NonCanonicalRow {
        name: "non-canonical/sar-program-ascending-sort",
        severity: Severity::Fix,
        citation: "CAPCO-2016 §H.5 p99 \
                   (programs: ascending, numeric first, then alpha)",
        presence: presence_sar_program_ascending_sort,
        evaluate: evaluate_sar_program_ascending_sort,
    },
    NonCanonicalRow {
        name: "non-canonical/sci-compartment-numeric-then-alpha",
        severity: Severity::Error,
        citation: "CAPCO-2016 §H.4 p61",
        presence: presence_sci_compartment_numeric_then_alpha,
        evaluate: evaluate_sci_compartment_numeric_then_alpha,
    },
];

/// Quick axis-presence check. When all five ordering axes are empty
/// (the dominant case on prose body text in a 10KB document), the
/// catalog walk is skipped entirely. Each sub-check is O(1) modulo
/// classification-variant matching.
fn axis_presence_any(attrs: &CanonicalAttrs) -> bool {
    !attrs.rel_to.is_empty()
        || matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
        || !attrs.aea_markings.is_empty()
        || attrs.sar_markings.is_some()
        || !attrs.sci_markings.is_empty()
}

// ---------------------------------------------------------------------------
// Per-row presence predicates
// ---------------------------------------------------------------------------

fn presence_rel_to_usa_first(attrs: &CanonicalAttrs) -> bool {
    // Precondition for the REL TO ordering check: REL TO has 2+
    // entries AND USA is first. If USA is missing or not first, E002
    // fires for those cases and its fix produces the fully canonical
    // list (USA first, non-USA entries alphabetical), so this row's
    // concern is silently absorbed there. Mirrors the retired
    // `CountryCodeOrderingRule` E020 REL TO sub-check at lines
    // 3086-3091.
    attrs.rel_to.len() >= 2
        && attrs
            .rel_to
            .first()
            .is_some_and(|t| *t == marque_ism::CountryCode::USA)
}

fn presence_joint_alphabetical(attrs: &CanonicalAttrs) -> bool {
    matches!(
        &attrs.classification,
        Some(MarkingClassification::Joint(j)) if j.countries.len() >= 2
    )
}

fn presence_sigma_numeric_sort(attrs: &CanonicalAttrs) -> bool {
    attrs.aea_markings.iter().any(|aea| match aea {
        AeaMarking::Rd(rd) => !rd.sigma.is_empty(),
        AeaMarking::Frd(frd) => !frd.sigma.is_empty(),
        _ => false,
    })
}

fn presence_sar_program_ascending_sort(attrs: &CanonicalAttrs) -> bool {
    attrs
        .sar_markings
        .as_ref()
        .is_some_and(|sar| sar.programs.len() >= 2)
}

fn presence_sci_compartment_numeric_then_alpha(attrs: &CanonicalAttrs) -> bool {
    !attrs.sci_markings.is_empty()
}

// ---------------------------------------------------------------------------
// Per-row evaluators (verbatim moves of the retired rules' check bodies)
// ---------------------------------------------------------------------------

/// Row 1: REL TO USA-first alphabetical (§H.8 p150-151).
///
/// Verbatim move of the REL TO sub-check from
/// `CountryCodeOrderingRule::check` (rules.rs:3086-3151).
/// Multi-block REL TO suppression preserved at lines 3110-3133.
fn evaluate_rel_to_usa_first_alpha(
    attrs: &CanonicalAttrs,
    _ctx: &RuleContext,
    row: &NonCanonicalRow,
) -> Vec<Diagnostic> {
    let rule_id = RuleId::new("E060");
    let mut diagnostics = Vec::new();

    // Locate the `RelToBlock` for this list. A single first→last
    // `RelToTrigraph` splice across the whole marking would delete
    // intervening `//...//` content when more than one REL TO block
    // is present (e.g.,
    // `SECRET//REL TO USA, GBR//NF//REL TO AUS`). Mirrors E002 in
    // scoping the fix to a single block and suppressing it when
    // multiple blocks are present.
    let rel_to_blocks: Vec<&TokenSpan> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToBlock)
        .collect();

    if rel_to_blocks.len() > 1 {
        // Suppress the fix rather than risk cross-block corruption.
        // Span the first block so downstream consumers have a
        // location to display.
        let actual: Vec<&str> = attrs.rel_to.iter().map(|t| t.as_str()).collect();
        // REL TO is USA-first per §H.8 p151.
        let sorted = canonicalize_trigraph_list(&attrs.rel_to, true);
        if actual != sorted {
            diagnostics.push(Diagnostic::new(
                rule_id,
                row.severity,
                rel_to_blocks[0].span,
                format!(
                    "REL TO country codes must be alphabetically ordered \
                     (USA first when present): [{}] → [{}] \
                     (multiple REL TO blocks present; fix suppressed to avoid \
                     cross-block corruption — resolve manually)",
                    actual.join(", "),
                    sorted.join(", "),
                ),
                row.citation,
                None,
            ));
        }
    } else if let Some(&block) = rel_to_blocks.first() {
        if let Some(diag) = check_trigraph_ordering(
            &attrs.rel_to,
            "REL TO",
            rule_id,
            row.severity,
            attrs,
            Some(block.span),
            row.citation,
            true, // REL TO: USA-first per §H.8 p151
        ) {
            diagnostics.push(diag);
        }
    }
    // If `rel_to_blocks` is empty while `attrs.rel_to` is populated,
    // the parser is in an inconsistent state; skip silently rather
    // than synthesize a span.

    diagnostics
}

/// Row 2: JOINT alphabetical (§H.3 p56).
///
/// Verbatim move of the JOINT sub-check from
/// `CountryCodeOrderingRule::check` (rules.rs:3164-3183). JOINT
/// prescribes pure alphabetical order — no USA-first carve-out per
/// §H.3 p56 (the widespread IC practice of rendering USA first in
/// JOINT lists is style convention; S003 `joint-usa-first` covers
/// that separately).
fn evaluate_joint_alphabetical(
    attrs: &CanonicalAttrs,
    _ctx: &RuleContext,
    row: &NonCanonicalRow,
) -> Vec<Diagnostic> {
    let rule_id = RuleId::new("E060");
    let mut diagnostics = Vec::new();

    if let Some(MarkingClassification::Joint(j)) = &attrs.classification {
        if j.countries.len() >= 2 {
            if let Some(diag) = check_trigraph_ordering(
                &j.countries,
                "JOINT",
                rule_id,
                row.severity,
                attrs,
                None,
                row.citation,
                false, // JOINT: pure alpha per §H.3 p56 (no USA-first)
            ) {
                diagnostics.push(diag);
            }
        }
    }

    diagnostics
}

/// Row 3: AEA SIGMA numeric sort (§H.6 p108).
///
/// Verbatim move of `SigmaValidationRule::check` (rules.rs:4164-
/// 4244). Two emit branches per AEA marking with non-empty SIGMA:
///
///   1. **Invalid-set check** — values outside the currently
///      authorized set `[14, 15, 18, 20]` produce a no-fix
///      diagnostic. §H.6 p108: "SIGMA # currently represents one or
///      more of the following numbers: 14, 15, 18, and 20."
///   2. **Numerical-order check** — `sigma.len() >= 2` AND `sigma
///      != sorted_dedup(sigma)` produces a fix diagnostic. §H.6 p108
///      (RD block): "Multiple SIGMA numbers shall be listed in
///      numerical order with a space preceding each value."
///
/// Both branches preserved verbatim under one walker row (splitting
/// would force a 6-row catalog with no citation-cleanness benefit —
/// both branches cite §H.6 p108).
fn evaluate_sigma_numeric_sort(
    attrs: &CanonicalAttrs,
    _ctx: &RuleContext,
    row: &NonCanonicalRow,
) -> Vec<Diagnostic> {
    let rule_id = RuleId::new("E060");
    let mut diagnostics = Vec::new();
    let valid_sigmas: &[u8] = &[14, 15, 18, 20];

    for aea in attrs.aea_markings.iter() {
        let sigma = match aea {
            AeaMarking::Rd(rd) => &rd.sigma,
            AeaMarking::Frd(frd) => &frd.sigma,
            _ => continue,
        };
        if sigma.is_empty() {
            continue;
        }

        let span = attrs
            .token_spans
            .iter()
            .find(|t| t.kind == TokenKind::AeaMarking)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        // Check for values outside the currently authorized set.
        // Unified message (no obsolete/invalid bifurcation) — CAPCO
        // 2016 §H.6 p108 only names the current four, not any
        // specific obsolete subset. Contact the originating
        // program for guidance on historical SIGMA numbers (CAPCO
        // v1.2 2008 permitted 1-99).
        let invalid: Vec<u8> = sigma
            .iter()
            .filter(|n| !valid_sigmas.contains(n))
            .copied()
            .collect();
        if !invalid.is_empty() {
            diagnostics.push(Diagnostic::new(
                rule_id.clone(),
                row.severity,
                span,
                format!(
                    "SIGMA {:?} not in the currently authorized set \
                     (14, 15, 18, 20); contact the originating \
                     program for guidance on historical values",
                    invalid,
                ),
                row.citation,
                None,
            ));
        }

        // Check numerical order.
        if sigma.len() >= 2 {
            let mut sorted = sigma.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            if sigma.as_ref() != sorted.as_slice() {
                let original: Vec<String> = sigma.iter().map(|n| n.to_string()).collect();
                let replacement: Vec<String> = sorted.iter().map(|n| n.to_string()).collect();
                diagnostics.push(make_fix_diagnostic(FixDiagnosticParams {
                    rule: rule_id.clone(),
                    severity: row.severity,
                    source: FixSource::BuiltinRule,
                    span,
                    message: format!(
                        "SIGMA numbers must be in numerical order: {} → {}",
                        original.join(" "),
                        replacement.join(" "),
                    ),
                    // §H.6 p108 (RD block): "Multiple SIGMA
                    // numbers shall be listed in numerical order
                    // with a space preceding each value."
                    citation: row.citation,
                    original: original.join(" "),
                    replacement: replacement.join(" "),
                    confidence: 1.0,
                    migration_ref: None,
                }));
            }
        }
    }
    diagnostics
}

/// Row 4: SAR program ascending sort (§H.5 p99).
///
/// Verbatim move of `SarProgramOrderRule::check` (rules.rs:4392-
/// 4446). Whole-block rewrite: the fix sorts programs AND normalizes
/// per-program compartments + sub-compartments in the same pass, so
/// applying this fix alone fully normalizes the block even when E029
/// violations are present (E029 covers per-program sub-spans and is
/// dropped under the C-1 overlap guard when this row's whole-block
/// fix wins).
fn evaluate_sar_program_ascending_sort(
    attrs: &CanonicalAttrs,
    _ctx: &RuleContext,
    row: &NonCanonicalRow,
) -> Vec<Diagnostic> {
    let rule_id = RuleId::new("E060");
    use marque_ism::sar_sort_key;

    let Some(sar) = attrs.sar_markings.as_ref() else {
        return vec![];
    };
    if sar.programs.len() < 2 {
        return vec![];
    }
    let in_order = sar
        .programs
        .windows(2)
        .all(|w| sar_sort_key(&w[0].identifier) <= sar_sort_key(&w[1].identifier));
    if in_order {
        return vec![];
    }
    let Some(span) = sar_block_span(attrs) else {
        return vec![];
    };
    let original = render_sar_block(sar.indicator, &sar.programs);

    // Sort programs and also normalize compartments/subs within each program
    // in the same pass. This ensures applying the fix alone fully normalizes
    // the block even when E029 violations are present.
    let mut sorted = sar.programs.to_vec();
    for prog in sorted.iter_mut() {
        let mut comps = prog.compartments.to_vec();
        for comp in comps.iter_mut() {
            let mut subs = comp.sub_compartments.to_vec();
            subs.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));
            *comp =
                marque_ism::SarCompartment::new(comp.identifier.clone(), subs.into_boxed_slice());
        }
        comps.sort_by(|a, b| sar_sort_key(&a.identifier).cmp(&sar_sort_key(&b.identifier)));
        *prog = marque_ism::SarProgram::new(prog.identifier.clone(), comps.into_boxed_slice());
    }
    sorted.sort_by(|a, b| sar_sort_key(&a.identifier).cmp(&sar_sort_key(&b.identifier)));
    let replacement = render_sar_block(sar.indicator, &sorted);

    vec![make_fix_diagnostic(FixDiagnosticParams {
        rule: rule_id,
        severity: row.severity,
        source: FixSource::BuiltinRule,
        span,
        message: "SAR programs must be in ascending order (numeric first, \
             then alphabetic)"
            .to_owned(),
        citation: row.citation,
        original,
        replacement,
        confidence: 0.85,
        migration_ref: None,
    })]
}

/// Row 5: SCI compartment + sub-compartment numeric-then-alpha
/// (§H.4 p61).
///
/// Verbatim move of `SciCompartmentOrderRule::check`
/// (rules.rs:5002-5159). Per-marking emit (one diagnostic per
/// out-of-order marking, not per level). The fix sorts compartments
/// AND sub-compartments together in a single rewrite — matches the
/// SAR E029 shape and ensures comp-order + sub-order violations on
/// the same marking don't produce overlapping fix spans.
///
/// Two citation strings (compartment-level vs sub-compartment-level)
/// are selected inside this evaluator based on `(comps_ok, subs_ok)`;
/// both cite §H.4 p61 with parenthetical specificity ("SCI
/// compartments: ascending..." vs "SCI sub-compartments:
/// ascending...").
fn evaluate_sci_compartment_numeric_then_alpha(
    attrs: &CanonicalAttrs,
    _ctx: &RuleContext,
    row: &NonCanonicalRow,
) -> Vec<Diagnostic> {
    let rule_id = RuleId::new("E060");
    use marque_ism::sar_sort_key;

    let mut out = Vec::new();

    let comp_spans: Vec<&TokenSpan> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::SciCompartment)
        .collect();
    let sub_spans: Vec<&TokenSpan> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::SciSubCompartment)
        .collect();

    let mut comp_cursor = 0usize;
    let mut sub_cursor = 0usize;

    for marking in attrs.sci_markings.iter() {
        let n_comps = marking.compartments.len();
        let this_sub_count: usize = marking
            .compartments
            .iter()
            .map(|c| c.sub_compartments.len())
            .sum();

        let comps_ok = n_comps < 2
            || marking.compartments.windows(2).all(|w| {
                sar_sort_key(w[0].identifier.as_ref()) <= sar_sort_key(w[1].identifier.as_ref())
            });
        let subs_ok = marking.compartments.iter().all(|c| {
            c.sub_compartments.len() < 2
                || c.sub_compartments
                    .windows(2)
                    .all(|w| sar_sort_key(w[0].as_ref()) <= sar_sort_key(w[1].as_ref()))
        });

        if comps_ok && subs_ok {
            comp_cursor += n_comps;
            sub_cursor += this_sub_count;
            continue;
        }

        // Span covers the whole compartment+sub-compartment region
        // for this marking: from the first compartment token through
        // the last sub-compartment token (or the last compartment
        // token when the marking has no sub-compartments).
        //
        // Use `.get()` defensively: if the token stream doesn't carry
        // the expected number of SciCompartment / SciSubCompartment
        // tokens (attrs built outside the parser, or future parser
        // changes), skip the fix instead of panicking.
        let this_comp_spans = if n_comps == 0 {
            &[][..]
        } else {
            match comp_spans.get(comp_cursor..comp_cursor + n_comps) {
                Some(s) => s,
                None => {
                    comp_cursor += n_comps;
                    sub_cursor += this_sub_count;
                    continue;
                }
            }
        };
        let fix_start = this_comp_spans.first().map(|t| t.span.start).unwrap_or(0);
        let fix_end = if this_sub_count > 0 {
            sub_spans
                .get(sub_cursor + this_sub_count - 1)
                .map(|t| t.span.end)
                .unwrap_or_else(|| {
                    this_comp_spans
                        .last()
                        .map(|t| t.span.end)
                        .unwrap_or(fix_start)
                })
        } else {
            this_comp_spans
                .last()
                .map(|t| t.span.end)
                .unwrap_or(fix_start)
        };
        let fix_span = Span::new(fix_start, fix_end);

        // Build the sorted marking: sort sub-compartments within
        // each compartment, then sort compartments by identifier.
        // Sub-comps ride along with their parent compartment.
        let mut sorted_comps = marking.compartments.to_vec();
        for c in sorted_comps.iter_mut() {
            let mut subs = c.sub_compartments.to_vec();
            subs.sort_by(|a, b| sar_sort_key(a.as_ref()).cmp(&sar_sort_key(b.as_ref())));
            *c = marque_ism::SciCompartment::new(c.identifier.clone(), subs.into_boxed_slice());
        }
        sorted_comps.sort_by(|a, b| {
            sar_sort_key(a.identifier.as_ref()).cmp(&sar_sort_key(b.identifier.as_ref()))
        });

        // Render this marking's compartment region (no system prefix —
        // the span only covers compartments+subs, not the system head).
        let render_comps = |comps: &[marque_ism::SciCompartment]| -> String {
            let parts: Vec<String> = comps
                .iter()
                .map(|c| {
                    if c.sub_compartments.is_empty() {
                        c.identifier.as_ref().to_owned()
                    } else {
                        let mut s = c.identifier.as_ref().to_owned();
                        for sub in c.sub_compartments.iter() {
                            s.push(' ');
                            s.push_str(sub.as_ref());
                        }
                        s
                    }
                })
                .collect();
            parts.join("-")
        };
        let original = render_comps(&marking.compartments);
        let replacement = render_comps(&sorted_comps);

        let (level, citation) = if !comps_ok {
            (
                "compartments",
                concat!(
                    "CAPCO-2016 §H.4 p61 ",
                    "(SCI compartments: ascending, numeric first, then alpha)",
                ),
            )
        } else {
            (
                "sub-compartments",
                concat!(
                    "CAPCO-2016 §H.4 p61 ",
                    "(SCI sub-compartments: ascending, numeric first, ",
                    "then alpha)",
                ),
            )
        };
        // Per-row citation field is `§H.4 p61`; the parenthetical
        // specificity ("SCI compartments" vs "SCI sub-compartments")
        // is a UX detail of the diagnostic and is selected here. Both
        // strings still contain `§H.4 p61` so the row's authoritative
        // citation is preserved on every emitted Diagnostic.
        let _ = row.citation;

        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule: rule_id.clone(),
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span: fix_span,
            message: format!(
                "SCI {level} must be listed in ascending order (numeric first, \
                 then alphabetic)"
            ),
            citation,
            original,
            replacement,
            confidence: 0.85,
            migration_ref: None,
        }));

        comp_cursor += n_comps;
        sub_cursor += this_sub_count;
    }

    out
}

// ---------------------------------------------------------------------------
// Walker
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeNonCanonicalInputRule;

impl Rule for DeclarativeNonCanonicalInputRule {
    fn id(&self) -> RuleId {
        RuleId::new("E060")
    }
    fn name(&self) -> &'static str {
        "non-canonical-input"
    }
    fn default_severity(&self) -> Severity {
        // Strictest of the per-row defaults (matches PR 3b.A banner
        // walker precedent: a config that uses E060 as the override
        // anchor cannot accidentally weaken any row below its
        // authoring intent without an explicit user choice). Per-row
        // severity is what's emitted when no override is set: `Fix`
        // for rows 1-4 (REL TO / JOINT / SIGMA / SAR), `Error` for
        // row 5 (SCI). The walker-level default engages when a user
        // keys `[rules] E060 = ...` for a coarse-grained override.
        // PM-resolved per OQ-3.
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic> {
        // PR 3b.F R-2 perf-1: axis-presence early-out. Bail when none
        // of the five ordering axes are populated. On prose body
        // text in a 10KB document this is the dominant case and the
        // catalog walk is skipped entirely.
        if !axis_presence_any(attrs) {
            return Vec::new();
        }
        // PR 3b.F R-2 perf-2: direct catalog-row dispatch with
        // per-row presence guard. Walk the static catalog table; for
        // each row whose presence predicate fires, call its
        // evaluator. The guard elides per-row evaluator overhead for
        // non-firing rows.
        let mut diags = Vec::new();
        for row in NON_CANONICAL_CATALOG {
            if (row.presence)(attrs) {
                diags.extend((row.evaluate)(attrs, ctx, row));
            }
        }
        diags
    }

    fn additional_emitted_ids(&self) -> &'static [(&'static str, &'static str)] {
        // Severity-config compatibility for the legacy IDs (E020,
        // E023, E028, E033) is intentionally NOT preserved — per
        // `feedback_pre_users_no_deprecation_phasing.md`, marque is
        // pre-users; rewrite freely. Returning an empty slice means
        // `[rules] E020 = ...` (and E023 / E028 / E033 likewise) are
        // rejected at engine construction with the standard "unknown
        // rule ID" error, forcing users to migrate to
        // `[rules] E060 = ...`.
        &[]
    }
}
