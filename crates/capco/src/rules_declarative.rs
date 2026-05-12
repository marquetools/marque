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
//! `citation` but **not** a `Span` today — the scheme had no access
//! to the `TokenSpan` slice the parser attaches to `CanonicalAttrs`,
//! and the earlier comment here noted that widening
//! `ConstraintViolation` to carry spans would couple the scheme layer
//! to ISM's token-span model. [`marque_scheme::Span`] (and
//! [`marque_scheme::Severity`]) now live in the scheme leaf crate
//! itself — both are pure data primitives with no ISM coupling, so a
//! future `ConstraintViolation { span, severity, ... }` extension
//! stays Principle-VII-clean. The extension itself, plus the
//! catalog-row decomposition that depends on it (E058 / E059
//! inlining), is sequenced separately so the per-row decomposition
//! strategy can be decided independently. For now, trigger-only
//! dispatch is still the path: each wrapper constructs its span from
//! `attrs.token_spans` the same way the retired hand-written rule did.
//!
//! ## Citation policy: wrappers match the catalog
//!
//! Every `Diagnostic` emitted here carries the same authoritative
//! `§X.Y pNN` citation as the matching catalog entry in
//! `crate::scheme::build_constraints`. The earlier byte-identity
//! freeze — which kept legacy umbrella references like `§B.1` /
//! `§B.3` / unpaginated `§H.4` / `§H.6` / `§H.7` in the wrappers
//! while the catalog already cited the page-precise forms — is
//! retired; wrappers and catalog rows are now in lockstep across
//! every shared rule (E010, E012, E014, E015, E016, E021, E024,
//! W002).
//!
//! New wrappers MUST cite the same authoritative passage as the
//! corresponding catalog row, page-precise where the audit
//! (`specs/006-engine-rule-refactor/rule-body-audit.md`) gives a
//! page anchor. Citation-lint (`tools/citation-lint/`) is a hard
//! CI gate: every `§X.Y pNN` in either surface must resolve to a
//! real passage in `crates/capco/docs/CAPCO-2016.md`, and the page
//! anchor must fall within the cited subsection's span.
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
    Confidence, Diagnostic, FixIntent, FixProposal, FixSource, Message, MessageArgs,
    MessageTemplate, Rule, RuleContext, RuleId, Severity,
};
use marque_scheme::{ConstraintViolation, FactRef, ReplacementIntent, Scope, TokenId};

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
//
// **Migration status (PR 3c.B Sub-PR 8.D.3, 2026-05-12):** consciously
// landed at `fix_intent: None`. The authoritative source (CAPCO-2016
// §H.4 at `crates/capco/docs/CAPCO-2016.md` lines 1393–1395) does NOT
// mandate HCS-P as the default fill for bare HCS. The Relationship(s)
// to Other Markings paragraph (line 1395) reads in relevant part:
//
//   "When incorporating legacy material marked 'HCS' into a new
//    product, re-mark the new document and associated portion
//    according to the instructions in the HCS-O and HCS-P marking
//    templates. However, legacy information previously marked HCS
//    and transmitted via machine-to-machine processes may retain
//    the HCS marking without requiring translation to either HCS-O
//    or HCS-P."
//
// The classifier MUST read the HCS-O and HCS-P marking templates and
// decide which applies for the specific information — operational
// source (HCS-O) versus analytical product (HCS-P). The decision
// depends on facts about the underlying intelligence that marque
// cannot see. Marque's prior auto-pick of HCS-P (at 0.95 absent HCS-O,
// or 0.5 when HCS-O also appeared) was a UX heuristic, not a manual
// directive.
//
// Per project memory `feedback_pre_users_no_deprecation_phasing`,
// marque is pre-users; we drop the heuristic rather than preserve it
// at higher confidence. A dual-population intent that auto-applied
// HCS-P would corrupt the audit log under Constitution V (Audit-
// First Compliance) by attributing a policy decision to the engine
// that only a human can make. Matches the `with_fix_intent(..., None)`
// pattern E015 / E016 established in Sub-PR 8.D.2 / 8.B.
//
// The Stage-4 target is a `Severity::Suggest` companion diagnostic
// pair ("did you mean `HCS-O`?" / "did you mean `HCS-P`?") — the
// same Reject-with-suggest pattern named for E015 / E036. No auto-
// applied fix exists for this combination because the marking shape
// is ambiguous in a way no single removal-or-addition can resolve
// without classifier input.

/// Replaces the hand-written `BareHcsRule`.
///
/// The catalog's `E010/HCS-system-constraints` Custom fires multiple
/// violations per offending marking (one per failing sub-rule: bare-
/// HCS detection, HCS-O/P classification floor, ORCON pairing, etc.).
/// Only the bare-HCS sub-violation corresponds to a legacy hand-
/// written diagnostic; the other sub-rules weren't emitted by any
/// rule before T035. The wrapper discriminates by message prefix.
///
/// Note: post Sub-PR 8.D.3 (PR #375), the diagnostic message and
/// fix shape changed (mentions HCS-O / HCS-P / HCS-O-P with
/// semantics; `fix_intent: None`). Byte-identity with the
/// pre-branch corpus is no longer preserved — the wrapper's
/// purpose is now to translate the catalog's bare-HCS sub-
/// violation into the conscious-defer emission, not to reproduce
/// the legacy fix string. The other sub-rules drop silently until
/// a future PR wires wrappers for them.
pub(crate) struct DeclarativeBareHcsRule;

impl Rule<CapcoScheme> for DeclarativeBareHcsRule {
    fn id(&self) -> RuleId {
        RuleId::new("E010")
    }
    fn name(&self) -> &'static str {
        "bare-hcs"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::SciControl;

        let violations = violations_for(attrs, "E010/HCS-system-constraints");
        let bare_hcs_fired = violations.iter().any(|v| {
            v.message.starts_with("Bare HCS is legacy")
                || v.message.starts_with("HCS requires a compartment")
        });
        if !bare_hcs_fired {
            return vec![];
        }

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

        // PR 3c.B Sub-PR 8.D.3 — migrated to `with_fix_intent`
        // constructor signaling consciously-decided-no-fix-intent.
        // See module-level comment block above for the HCS-O vs
        // HCS-P classifier-decision rationale.
        vec![Diagnostic::with_fix_intent(
            self.id(),
            self.default_severity(),
            span,
            "bare HCS is a legacy marking; consult the HCS-O and HCS-P marking templates \
             per §H.4 to determine the correct compartment (HCS-O for operational source \
             information, HCS-P for analytical product, HCS-O-P when both are present)",
            "CAPCO-2016 §H.4 p62",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E012 — dual classification (US + foreign in one marking)
// ---------------------------------------------------------------------------
//
// **Migration status (PR 3c.B Sub-PR 8.D.5, 2026-05-12):** consciously
// landed at `fix_intent: None`. The §H.3 p55 mutual-exclusion
// predicate is authoritative ("The US, non-US, and JOINT classification
// markings are mutually exclusive — a banner line or portion mark may
// contain only one type and value for the classification marking"),
// but the *remediation* — "move foreign to FGI block" — is a
// CROSS-AXIS renormalization (classification axis → FGI axis) that
// the current intent vocabulary cannot express:
//
// 1. `ReplacementIntent::FactAdd` and `FactRemove` are strictly
//    single-axis-scoped — they add or remove a token within ONE
//    axis. The E012 fix mutates two axes atomically (drop the
//    foreign-classification token, add an FGI block with the
//    corresponding countries and level).
// 2. `ReplacementIntent::Recanonicalize` re-renders an existing
//    axis from `CanonicalAttrs`. Today `CapcoScheme::project`
//    does not resolve `MarkingClassification::Conflict` into a
//    well-formed FGI projection — the projector treats it as a
//    pure US classification at the `max` level and discards the
//    foreign side. Without that resolution, Recanonicalize cannot
//    materialize the FGI block.
//
// Either path forward (a new `Migrate { from, to, scope }` intent
// variant, or extending the `CapcoScheme::project` Conflict
// resolution) is an engine/scheme edit forbidden in scheme-adoption
// sub-PRs by Constitution VII §IV. The retirement target is named
// below.
//
// **Citation-honesty note.** Earlier revisions emitted a
// hard-coded `FGI {countries}` / `FGI NATO` replacement string under
// `make_fix_diagnostic` at confidence 0.90. The diagnostic separates
// the load-bearing cites:
//
//   - **Detection** — §H.3 p55: "The US, non-US, and JOINT
//     classification markings are mutually exclusive — a banner line
//     or portion mark may contain only one type and value for the
//     classification marking." Authoritative for the fact that a
//     `Conflict { us, foreign }` shape is malformed.
//   - **US-precedence pattern** — §H.3 p57 (JOINT derivative use,
//     normative): when JOINT portions are extracted into a US
//     document, "the banner line contains the highest classification
//     level of all portions, expressed as a US classification
//     marking" with "the FGI marking including all trigraph/
//     tetragraph codes identified in the JOINT portion(s)." §H.3
//     p59 Notional Example 4 note generalizes this: "when US and
//     non-US portions are combined in a single document, the
//     overall marking is a US classification." These passages
//     establish the US-precedence + foreign-to-FGI structural
//     pattern that the now-retired auto-repair was imitating.
//   - **FGI marking format** — §H.7: the shape an FGI block takes
//     once a classifier has decided to express the foreign side as
//     FGI.
//
// What CAPCO does NOT directly say: "if a classifier writes
// `C//NATO C` in a single marking, treat the marking as US C." The
// p57/p59 passages cover document-level commingling (JOINT
// extraction, mixed US+non-US portions), not the malformed-input
// case where two classifications share one banner/portion. The
// inference from "document commingling → US classification + FGI
// block" to "malformed dual marking → US classification + FGI
// block" is a defensible pattern application, but it is
// application of a pattern, not direct citation. Path B's no-fix
// posture is the citation-honest choice: the rule fires, surfaces
// the §H.3 p55 mutual-exclusion problem, names the CAPCO
// US-precedence pattern (§H.3 p57 / p59), and lets the classifier
// consult §H.7 for the correct FGI marking shape.
//
// **Severity preservation.** `default_severity()` stays at
// `Severity::Fix`. Severity classifies the rule's PROBLEM-CATEGORY,
// not a fix-emission promise — E010 kept its severity at `Error`
// under the same conscious-defer pattern (Sub-PR 8.D.3, PR #375),
// and E015 / E016 follow the same shape. Confidence-threshold
// math also concurs: the legacy 0.90 confidence sat below the
// default `Config::confidence_threshold` (0.95), so the
// dual-population legacy path was never reaching `result.applied`
// in production — Path B closes the proposal channel cleanly
// without altering observable auto-apply behavior.
//
// **Stage-4 retirement target.** E012 is the canonical example of
// the "type incompatibility / ejection" pattern class (alongside
// JOINT, NODIS/EXDIS, RELIDO/REL TO/NOFORN) — the entire family
// is tracked under
// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`.
// E012 retires to either:
//   (a) a new `ReplacementIntent::Migrate { from, to, scope }`
//       variant that expresses cross-axis renormalization
//       atomically (preferred — generalizes to the rest of the
//       incompatibility family), OR
//   (b) `Recanonicalize { Portion }` once `CapcoScheme::project`
//       admits classification-axis `Conflict` resolution and emits
//       an FGI projection from the foreign side.
// The conscious-defer landing here keeps the rule firing and the
// diagnostic message accurate while the consolidation pass decides
// between (a) and (b).

/// Replaces the hand-written `DualClassificationRule`.
pub(crate) struct DeclarativeDualClassificationRule;

impl Rule<CapcoScheme> for DeclarativeDualClassificationRule {
    fn id(&self) -> RuleId {
        RuleId::new("E012")
    }
    fn name(&self) -> &'static str {
        "dual-classification"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{ForeignClassification, MarkingClassification};

        if violations_for(attrs, "E012/dual-classification").is_empty() {
            return vec![];
        }

        let Some(MarkingClassification::Conflict { us, foreign }) = &attrs.classification else {
            return vec![];
        };

        // Token-canonical foreign description (G13-clean: derived
        // from vocabulary / typed structural values, not from
        // source-buffer slices).
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

        // Second Classification token span — that's the foreign one.
        let class_spans = spans_of_kind(attrs, TokenKind::Classification);
        let span = class_spans
            .get(1)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        let us_banner = us.banner_str();

        // PR 3c.B Sub-PR 8.D.5 — migrated to `with_fix_intent`
        // constructor signaling consciously-decided-no-fix-intent.
        // See module-level comment block above for the cross-axis-
        // renormalization rationale and Stage-4 retirement target.
        vec![Diagnostic::with_fix_intent(
            self.id(),
            self.default_severity(),
            span,
            format!(
                "marking has both US ({us_banner}) and foreign ({foreign_desc}) \
                 classification; §H.3 p55 mandates these are mutually exclusive. \
                 CAPCO's pattern when US and non-US classifications are commingled \
                 is to express the overall as a US classification with foreign \
                 provenance in an FGI block (§H.3 p57 JOINT derivative use; §H.3 \
                 p59 Example 4 note); consult §H.7 for the FGI marking format",
            ),
            // §H.3 p55 is the authoritative passage for the US +
            // non-US classification mutual exclusion (the JOINT
            // template's "The US, non-US, and JOINT classification
            // markings are mutually exclusive" sentence). Matches
            // the catalog row at
            // `scheme.rs:E012/dual-classification`.
            "CAPCO-2016 §H.3 p55",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E014 — JOINT participants must appear in REL TO
// ---------------------------------------------------------------------------
//
// **Migration status (PR 3c.B Sub-PR 8.D.4, 2026-05-12):** migrated
// to `with_fix_intent` carrying one `FactAdd { CountryCode(...),
// Scope::Portion }` intent per missing JOINT co-owner. This is the
// FIRST consumer of the open-vocab `CapcoOpenVocabRef::CountryCode`
// FactAdd path on the CAT_REL_TO axis (wired in
// `crates/capco/src/scheme.rs::apply_fact_add` in the same sub-PR).
//
// # Authoritative source (CAPCO-2016 §H.3 p57)
//
// > "JOINT classified information for which the US is a co-owner,
// > must be appropriately classified and explicitly marked with a
// > REL TO marking that includes the US and all co-owners, at both
// > the banner and portion level."
//
// — `crates/capco/docs/CAPCO-2016.md` §H.3 p57, under
// "Relationship(s) to Other Markings → Requires REL TO USA, LIST".
//
// # Why this is FactAdd, not no-fix-intent (vs. E010 / E015 / E016)
//
// The §H.3 p57 "REL TO marking that includes the US and all
// co-owners" floor is policy-mandated and deterministic. Given
// `JOINT(X, Y, Z)`, the REL TO minimum is `{USA, X, Y, Z}` — no
// classifier discretion sits between the JOINT participant list and
// the REL TO floor. The classifier's discretion is around expanding
// the LIST beyond the floor (adding additional release-authorized
// partners); the floor itself is auto-fillable audit-faithfully.
// This is the structural difference from the conscious-defer rules
// (E010 HCS-O vs HCS-P, E015 REL TO USA, LIST vs NOFORN, E016 JOINT
// + RESTRICTED) where the source provides two valid fills and
// classifier judgment picks one.
//
// # N-diagnostic emission shape
//
// Each missing co-owner produces ONE Diagnostic carrying ONE
// `FactAdd` intent. Three rationales:
// 1. `FixIntent.replacement` is strict-singleton (one
//    `ReplacementIntent` per `FixIntent`); the natural way to express
//    "add multiple countries" is multiple intents — one per
//    diagnostic.
// 2. Per-diagnostic actionability: a classifier UI displaying "GBR
//    missing" is more actionable than "[GBR, CAN, AUS] missing" with
//    a single combined fix. Each row is independently
//    suppress/accept-able.
// 3. The engine's batch dispatcher (`apply_intent_to_marking`)
//    aggregates per-intent applications — N FactAdds compose
//    correctly into the canonical REL TO list.
//
// Span anchors to the classification level token (`TokenKind::
// Classification` — `S`/`SECRET`/`TS`/`TOP SECRET`, the same anchor
// the legacy single-diagnostic emission used). `TokenKind` carries
// no JOINT-specific variant — the JOINT keyword is parsed into the
// `MarkingClassification::Joint(_)` shape, not into its own token
// span — and the level token is the closest available structural
// anchor that surfaces the violation at the classification site
// rather than the (potentially absent) REL TO position. All N
// per-co-owner diagnostics share this single span.

pub(crate) struct DeclarativeJointRelToRule;

impl Rule<CapcoScheme> for DeclarativeJointRelToRule {
    fn id(&self) -> RuleId {
        RuleId::new("E014")
    }
    fn name(&self) -> &'static str {
        "joint-rel-to"
    }
    fn default_severity(&self) -> Severity {
        // PR 3c.B Sub-PR 8.D.4: Error → Fix. The §H.3 p57
        // "REL TO marking that includes the US and all co-owners"
        // floor is policy-mandated and deterministic — no classifier
        // discretion gates the per-co-owner addition. Fix-severity is
        // appropriate because the intent is unambiguous and the
        // engine auto-applies at the default 0.95 threshold.
        Severity::Fix
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingClassification;

        if violations_for(attrs, "E014/joint-requires-rel-to-coverage").is_empty() {
            return vec![];
        }

        let joint = match &attrs.classification {
            Some(MarkingClassification::Joint(j)) => j,
            _ => return vec![],
        };

        // Iterate over `&CountryCode` (Copy) so we can both diagnose
        // by `.as_str()` and pass the typed value into the FactAdd
        // intent without an intermediate parse.
        let missing: Vec<marque_ism::CountryCode> = joint
            .countries
            .iter()
            .filter(|c| !crate::scheme::rel_to_covers(&attrs.rel_to, c.as_str()))
            .copied()
            .collect();
        if missing.is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::Classification);

        // Use `with_intent_at_span` so the engine's
        // `synthesize_intent_only_fixes` can locate this diagnostic
        // via `candidate_span` and re-render the marking after
        // `apply_intent`. Without `candidate_span` populated, the
        // synthesis pipeline skips the intent and no fix lands in the
        // applied audit stream. Same pattern as E038 (FactAdd
        // CAT_DISSEM) and E041 (FactRemove CAT_NON_IC_DISSEM).
        missing
            .iter()
            .map(|country| {
                Diagnostic::with_intent_at_span(
                    self.id(),
                    self.default_severity(),
                    span,
                    ctx.candidate_span,
                    format!(
                        "JOINT participant {} must appear in REL TO list",
                        country.as_str(),
                    ),
                    "CAPCO-2016 §H.3 p57",
                    e014_add_country_intent(*country, ctx.marking_type),
                )
            })
            .collect()
    }
}

/// Build the `FactAdd { CountryCode(...), Scope }` intent emitted by
/// [`DeclarativeJointRelToRule`] for one missing JOINT co-owner.
///
/// **Scope follows `marking_type`**: a portion-context firing emits
/// `Scope::Portion` (one portion's REL TO list); a banner or CAB-context
/// firing emits `Scope::Page` (the page-aggregated REL TO list per
/// §H.3 p57 "at both the banner and portion level"). Matches the
/// established E002 pattern at `crates/capco/src/rules.rs:548-555`
/// for the analogous CAT_REL_TO axis FactAdd.
///
/// **Confidence is `Confidence::strict(0.95)`**: §H.3 p57 is
/// unambiguous about the floor (co-owners ⊆ REL TO list); the
/// strict-recognizer path produced the JOINT parse that surfaced the
/// triggering classification token. Threshold matches the engine's
/// default `Config::confidence_threshold` (0.95) so the fix
/// auto-applies, but stays at the precedent-aligned level — same as
/// E021 (AEA-requires-NOFORN, §H.6 p104 + p111) which has an
/// identical "always used with X unless agreement" shape.
///
/// **Structured message**: `MessageTemplate::RequiredByPresence`
/// with `token: Some(TOK_JOINT)`. `expected_token` is intentionally
/// `None` — the missing companion is a country code, not a CVE
/// `TokenId`, and `MessageArgs.expected_token` is closed-CVE-only.
/// The country itself is carried structurally via the
/// `ReplacementIntent::FactAdd { token: FactRef::OpenVocab(
/// CapcoOpenVocabRef::CountryCode(...)), ... }` payload, which the
/// audit emitter renders separately from the structured message.
fn e014_add_country_intent(
    country: marque_ism::CountryCode,
    marking_type: marque_ism::MarkingType,
) -> FixIntent<CapcoScheme> {
    use crate::scheme::{CapcoOpenVocabRef, TOK_JOINT};
    let scope = match marking_type {
        marque_ism::MarkingType::Portion => Scope::Portion,
        _ => Scope::Page,
    };
    FixIntent {
        replacement: ReplacementIntent::FactAdd {
            token: FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(country)),
            scope,
        },
        confidence: Confidence::strict(0.95),
        feature_ids: Default::default(),
        message: Message::new(
            MessageTemplate::RequiredByPresence,
            MessageArgs {
                token: Some(TOK_JOINT),
                ..MessageArgs::default()
            },
        ),
    }
}

// ---------------------------------------------------------------------------
// E015 — non-US classification requires dissem control
// ---------------------------------------------------------------------------
//
// **Migration status (PR 3c.B Sub-PR 8.D.2, 2026-05-12):** consciously
// landed at `fix_intent: None`. The authoritative source (CAPCO-2016
// §H.7 p122 + §B.3 p20) offers **two distinct valid fills** keyed to
// a foreign-arrangement fact outside marque's view:
//
// 1. **REL TO USA, [LIST]** — when the originating country allows
//    further sharing by the United States (§B.3 p20 paragraph d).
//    Marque cannot synthesize the [LIST] because the sharing
//    agreement defines which partner nations are authorized.
// 2. **NOFORN** — when the originating country prohibits further
//    sharing by the United States (§B.3 p20 paragraph d, second
//    bullet), or in the absence of a positive release determination
//    by the originating agency.
//
// Both fills are correct per the source; only the classifier knows
// which applies for the specific information being marked. A
// dual-population intent that picked one branch arbitrarily would
// corrupt the audit log under Constitution V (Audit-First
// Compliance) by attributing a policy decision to the engine that
// only a human can make. Matches the `with_fix_intent(..., None)`
// pattern E016 (JOINT+RESTRICTED) established in Sub-PR 8.B.
//
// The Stage-4 target is a `Severity::Suggest` companion diagnostic
// pair ("did you mean `REL TO USA, [LIST]`?" / "did you mean
// `NOFORN`?") — the same Reject-with-suggest pattern named for E036
// (JOINT+HCS). No auto-applied fix exists for this combination
// because the marking shape is ambiguous in a way no single
// removal-or-addition can resolve without classifier input.

pub(crate) struct DeclarativeNonUsMissingDissemRule;

impl Rule<CapcoScheme> for DeclarativeNonUsMissingDissemRule {
    fn id(&self) -> RuleId {
        RuleId::new("E015")
    }
    fn name(&self) -> &'static str {
        "non-us-missing-dissem"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if violations_for(attrs, "E015/non-us-requires-dissem").is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::Classification);

        // PR 3c.B Sub-PR 8.D.2 — migrated to `with_fix_intent`
        // constructor signaling consciously-decided-no-fix-intent.
        // See module-level comment block above for the two-valid-
        // fills rationale.
        vec![Diagnostic::with_fix_intent(
            self.id(),
            self.default_severity(),
            span,
            "non-US classification must be accompanied by a dissemination control \
             (e.g., REL TO, NOFORN)",
            // §H.7 p122 (FGI commingling + sharing-agreement basis)
            // + §B.3 p20 (FD&R markings on FGI in IC DAPs) are the
            // authoritative passages. Earlier revisions cited `§B.3`
            // (a legacy umbrella pointer) under a byte-identity
            // freeze; the wrapper now matches the catalog row at
            // `scheme.rs:E015/non-us-requires-dissem`.
            "CAPCO-2016 §H.7 p122 + §B.3 p20",
            None,
        )]
    }
}

// ---------------------------------------------------------------------------
// E016 — JOINT cannot be RESTRICTED
// ---------------------------------------------------------------------------
//
// **Migration status (PR 3c.B Sub-PR 8.B, 2026-05-11):** consciously
// landed at `fix_intent: None`. Per the 2026-05-11 lattice-consultant
// session captured in
// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`,
// this rule is a **Category A.3 — Transmute via foreign-equivalence map**
// case under the eventual `Constraint::Incompatible` umbrella primitive.
// The Stage-4 target is `Remove(RESTRICTED) ⊕ Add(CONFIDENTIAL)` via a
// foreign-equivalence vocabulary table (UK RESTRICTED → US CONFIDENTIAL
// per Five Eyes practice), emitted as one atomic audit repair. The
// vocabulary table does not exist in `marque-capco::vocab` today and
// its source is open — see the followup file's Open Question 1.
// Candidate authoritative sources include CAPCO-2016 Appendix A §4
// (Five Eyes Marking Comparisons; not currently vendored in
// `crates/capco/docs/`) and bilateral disclosure-policy tables.
// Per Constitution VIII, CAPCO-2016 §H.3 p56 itself does NOT publish
// this equivalence — it only says "RESTRICTED is not an authorized
// US classification marking." A.3 lands when the source is resolved.
//
// For now, this rule emits a `Severity::Error` diagnostic with
// `fix_intent: None` — the engine surfaces the error to the user but
// applies no auto-fix. The diagnostic message names the CONFIDENTIAL
// hint as Five Eyes practice (not as a §H.3 claim) so the user can
// re-mark the violating text manually. Wording stays context-neutral
// because this rule's `check` does not consult `RuleContext` and can
// fire on either a portion or a banner.
//
// **Do not** dual-populate this rule with a single-fact
// `FactRemove(RESTRICTED, Portion)` intent in the interim — that would
// land a half-fix (leaving the marking without a classification level)
// and corrupt the audit log under Constitution V.

pub(crate) struct DeclarativeJointRestrictedRule;

impl Rule<CapcoScheme> for DeclarativeJointRestrictedRule {
    fn id(&self) -> RuleId {
        RuleId::new("E016")
    }
    fn name(&self) -> &'static str {
        "joint-restricted"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if violations_for(attrs, "E016/joint-conflicts-restricted").is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::Classification);

        // PR 3c.B Sub-PR 8.B — migrated to `with_fix_intent` constructor
        // signaling consciously-decided-no-fix-intent (Category A.3,
        // Stage-4 target). See module-level comment block above.
        //
        // Message wording per Constitution VIII (Authoritative Source
        // Fidelity): the §H.3 p56 citation supports the prohibition and
        // the "not a US classification level" claim verbatim from p56.
        // The CONFIDENTIAL hint is framed as Five Eyes practice — NOT
        // attributed to §H.3 — because the equivalence lives in
        // CAPCO-2016 Appendix A §4 (Five Eyes Marking Comparisons), not
        // in §H.3 itself.
        vec![Diagnostic::with_fix_intent(
            self.id(),
            self.default_severity(),
            span,
            "RESTRICTED may not be used with JOINT — RESTRICTED is not \
             an authorized US classification level. Re-mark using an \
             authorized US classification (per Five Eyes practice, the \
             operational equivalent of UK/Commonwealth RESTRICTED is \
             CONFIDENTIAL; consult Five Eyes Marking Comparisons for \
             the authoritative table)",
            "CAPCO-2016 §H.3 p56",
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
//
// **Migration status (PR 3c.B Sub-PR 8.B, 2026-05-11):** consciously
// landed at `fix_intent: None`. Per the 2026-05-11 lattice-consultant
// session captured in
// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`,
// this rule is a **Category B — genuine mutual exclusion without
// policy decision** case under the eventual `Constraint::Incompatible`
// umbrella primitive. Stage-4 target: `Reject { suggest: Some(...) }` —
// emit the error plus an optional `Severity::Suggest` companion
// diagnostic ("did you mean `SECRET//HCS-P//REL TO [LIST]`?"). No
// auto-applied fix exists for this combination — JOINT changes the
// attribution semantics; HCS is CIA-owned and US-only; the marking
// shape is contradictory in a way no removal can resolve.
//
// JOINT+HCS is academic in practice (JOINT classifications are largely
// DOD-only; HCS is CIA-only; the agencies' marking vocabularies don't
// overlap on this axis), so the diagnostic-only landing is functionally
// sufficient. The Stage-4 `Suggest` channel adds polish, not correctness.

pub(crate) struct DeclarativeJointHcsRule;

impl Rule<CapcoScheme> for DeclarativeJointHcsRule {
    fn id(&self) -> RuleId {
        RuleId::new("E036")
    }
    fn name(&self) -> &'static str {
        "joint-hcs"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
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

        // PR 3c.B Sub-PR 8.B — migrated to `with_fix_intent` constructor
        // signaling consciously-decided-no-fix-intent (Category B,
        // Stage-4 target). See module-level comment block above.
        vec![Diagnostic::with_fix_intent(
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
// E021 — RD/FRD requires NOFORN
// ---------------------------------------------------------------------------
//
// TFNI is intentionally excluded from this rule per §H.6 p120 + p121
// (see `e021_aea_requires_noforn` doc in `crate::scheme` for the
// authority trace).

pub(crate) struct DeclarativeAeaNofornRule;

impl Rule<CapcoScheme> for DeclarativeAeaNofornRule {
    fn id(&self) -> RuleId {
        RuleId::new("E021")
    }
    fn name(&self) -> &'static str {
        "aea-noforn"
    }
    fn default_severity(&self) -> Severity {
        // PR 3c.B Commit 3: Error → Fix. CAPCO §H.6 p104 (RD) + p111
        // (FRD) both state the marking "Is always used with NOFORN
        // unless a sharing agreement has been established per the
        // Atomic Energy Act." The fix is unambiguous (insert NOFORN);
        // the rule emits a structural FactAdd that the engine
        // auto-applies at the default 0.95 threshold. Orgs with
        // sharing agreements override via `.marque.toml [rules]
        // E021 = "warn"`.
        Severity::Fix
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if violations_for(attrs, "E021/aea-requires-noforn").is_empty() {
            return vec![];
        }

        let span = first_span_of(attrs, TokenKind::AeaMarking);

        // PR 3c.B Commit 3 (E021 migration). Severity flipped
        // Error → Fix. Dual-population per Path C: legacy `fix`
        // (byte-precise zero-width `/NOFORN` insertion at the end
        // of the IC dissem block) + structural
        // `FactAdd { NOFORN, Scope::Portion }` intent. When the
        // portion has no IC dissem block at all, the legacy
        // helper returns None and the rule emits no fix — the
        // engine surfaces the diagnostic but does not auto-apply.
        // Inserting a whole `//`-separated dissem category from
        // rule context would synthesize content the user didn't
        // type (same defensive policy as `emit_companion_insert`
        // and `compute_relido_removal_span`).
        //
        // E021 has no pre-PR-3c byte-identity baseline because it
        // was previously Error-no-fix (no audit record emitted).
        // The byte-identity gate is vacuous for E021; correctness
        // is exercised by the per-rule shape tests.
        match build_aea_noforn_addition_fix(self.id(), attrs) {
            Some(fix) => vec![Diagnostic::with_fix_and_intent(
                self.id(),
                self.default_severity(),
                span,
                "RD/FRD requires NOFORN unless a sharing agreement exists \
                 per the Atomic Energy Act; override to warn via rule severity \
                 config if sharing agreements apply",
                "CAPCO-2016 §H.6 p104 + p111",
                fix,
                aea_noforn_add_intent(),
            )],
            None => vec![Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                "RD/FRD requires NOFORN unless a sharing agreement exists \
                 per the Atomic Energy Act; override to warn via rule severity \
                 config if sharing agreements apply",
                "CAPCO-2016 §H.6 p104 + p111",
                None,
            )],
        }
    }
}

/// Build a `<last-dissem-token>/<NOFORN-form>` append-fix anchored on
/// the last existing IC dissem token. Analogous to
/// `build_relido_removal_fix` (subtractive); the `emit_companion_insert`
/// helper used by SCI per-system catalog rules also emits an additive
/// fix but at a zero-width span (`Span::new(end, end)`), which the
/// engine's `!f.span.is_empty()` filter
/// (`crates/engine/src/engine.rs` line ~1108) silently drops. This
/// helper anchors on the last dissem token's full span and re-emits
/// the token plus `/NOFORN` so the engine actually applies the fix
/// (E021 is `Severity::Fix`, not the warn-no-fix posture of the SCI
/// per-system additive rows).
///
/// Returns `None` when the portion has no IC dissem block at all —
/// same defensive policy as `compute_relido_removal_span`: never
/// synthesize structural input from rule context (inserting a whole
/// `//`-separated category absent an explicit anchor is unsafe).
///
/// The inserted form (`NF` vs `NOFORN`) tracks the form of the first
/// existing dissem token via `infer_companion_form` so the post-fix
/// bytes don't mix banner-form and portion-form. Matches the
/// surface-form policy `emit_companion_insert` uses for SCI per-system
/// companion insertions.
///
/// Confidence is `Confidence::strict(0.95)` — same as
/// `build_relido_removal_fix` and the SCI per-system catalog inserts
/// (CAPCO precedent for at-threshold, auto-apply fixes).
///
/// `FixSource::BuiltinRule` per the strict-path provenance convention
/// for hand-written CAPCO rules.
fn build_aea_noforn_addition_fix(rule_id: RuleId, attrs: &CanonicalAttrs) -> Option<FixProposal> {
    // Walk to the LAST DissemControl token span — same as
    // `scheme::last_dissem_span` but we also need the token's text so
    // we can re-emit it in the replacement. Inlining keeps the helper
    // self-contained and avoids a second pass over `token_spans`.
    let last = attrs
        .token_spans
        .iter()
        .rev()
        .find(|t| t.kind == TokenKind::DissemControl)?;
    let form = crate::scheme::infer_companion_form(attrs);
    Some(FixProposal::new(
        rule_id,
        FixSource::BuiltinRule,
        last.span,
        last.text.as_ref(),
        format!("{}/{}", last.text, form.noforn()),
        Confidence::strict(0.95),
        None,
    ))
}

/// Build the canonical `FactAdd { NOFORN, Scope::Portion }` intent
/// emitted by E021. NOFORN addition is scope-portion: the fact set
/// the rule mutates is a single portion's dissem-axis projection
/// (§H.6 p104 applies per-portion, not per-page).
///
/// Confidence mirrors `build_aea_noforn_addition_fix` so the
/// engine's threshold gate produces identical filter behavior on
/// `fix_intent.confidence.combined()` vs the legacy
/// `fix.confidence.combined()` path through the Path C transition
/// window. Commit 10 collapses to a single emission path.
fn aea_noforn_add_intent() -> FixIntent<CapcoScheme> {
    use crate::scheme::TOK_NOFORN;
    FixIntent {
        replacement: ReplacementIntent::FactAdd {
            token: FactRef::Cve(TOK_NOFORN),
            scope: Scope::Portion,
        },
        confidence: Confidence::strict(0.95),
        feature_ids: Default::default(),
        message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
    }
}

// ---------------------------------------------------------------------------
// E022 — CNWDI classification floor (RETIRED)
// ---------------------------------------------------------------------------
//
// PR 3b.D (T026d) → PR 3c.B Commit 7.3: retired. The CNWDI floor
// invariant lives in `CapcoScheme`'s constraint catalog as the row
// `E058/CNWDI-classification-floor` (CAPCO §H.6 p104). The engine's
// constraint-catalog bridge (see `crates/engine/src/engine.rs` lint
// loop) is the sole emitter; emitted diagnostics carry
// `Diagnostic.rule = "E058"` (audit-stream + config-override
// continuity with the retired walker convention). Per-row
// identification flows via the diagnostic message text.
//
// The legacy `E022` rule ID is NOT preserved as a severity-config
// alias. Per project memory
// `feedback_pre_users_no_deprecation_phasing.md`: marque is
// pre-users; we don't carry alias maps. Valid `.marque.toml` keys
// for class-floor severity overrides are `E058` (recommended; matches
// `Diagnostic.rule` and audit-stream output) and `class-floor-catalog`
// (the descriptive alias the canonicalizer accepts via
// `CapcoScheme::bridge_emitted_rule_ids()`). The retired `E022` key
// is rejected as `UnknownRuleOverride`.

// ---------------------------------------------------------------------------
// E024 — RD takes precedence over FRD/TFNI (multi-emission)
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeRdPrecedenceRule;

impl Rule<CapcoScheme> for DeclarativeRdPrecedenceRule {
    fn id(&self) -> RuleId {
        RuleId::new("E024")
    }
    fn name(&self) -> &'static str {
        "rd-precedence"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
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
                "CAPCO-2016 §H.6 p104",
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
// PR 3b.D (T026d) → PR 3c.B Commit 7.3: retired. The UCNI ceiling
// invariant lives in `CapcoScheme`'s constraint catalog as two rows
// (`E058/DOD-UCNI-classification-ceiling` at CAPCO §H.6 p116 and
// `E058/DOE-UCNI-classification-ceiling` at §H.6 p118 — split so
// each variant has its own §H.6 sub-page citation). The engine's
// constraint-catalog bridge is the sole emitter; emitted diagnostics
// carry `Diagnostic.rule = "E058"`.
//
// The legacy `E025` rule ID is NOT preserved as a severity-config
// alias. Per project memory
// `feedback_pre_users_no_deprecation_phasing.md`: marque is
// pre-users; we don't carry alias maps. Valid `.marque.toml` keys
// for class-floor severity overrides are `E058` (recommended; matches
// `Diagnostic.rule` and audit-stream output) and `class-floor-catalog`
// (the descriptive alias the canonicalizer accepts via
// `CapcoScheme::bridge_emitted_rule_ids()`). The retired `E025` key
// is rejected as `UnknownRuleOverride`.

// ---------------------------------------------------------------------------
// W002 — US + FGI comingling in portion (portion-only)
// ---------------------------------------------------------------------------

pub(crate) struct DeclarativeCominglingWarningRule;

impl Rule<CapcoScheme> for DeclarativeCominglingWarningRule {
    fn id(&self) -> RuleId {
        RuleId::new("W002")
    }
    fn name(&self) -> &'static str {
        "us-fgi-comingling"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
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
            "CAPCO-2016 §H.7 p124",
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
//
// **Migration status (PR 3c.B Sub-PR 8.E, 2026-05-11):** this rule
// intentionally remains at `fix_intent: None`; Stage-4 target:
// `Reject { suggest: None }`.
//
// Canonical rationale: see
// `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`
// (Category B / eventual `Constraint::Incompatible` mapping for the
// NODIS+EXDIS mutual-exclusion case).

pub(crate) struct DeclarativeNodisConflictsExdisRule;

impl Rule<CapcoScheme> for DeclarativeNodisConflictsExdisRule {
    fn id(&self) -> RuleId {
        RuleId::new("E037")
    }
    fn name(&self) -> &'static str {
        "nodis-conflicts-exdis"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        if violations_for(attrs, "E037/nodis-conflicts-exdis").is_empty() {
            return vec![];
        }

        // Point at the first non-IC dissem token span. Either NODIS
        // or EXDIS is the first offender per source order; the user
        // needs to remove one of them to resolve.
        let span = first_span_of(attrs, TokenKind::NonIcDissem);

        // PR 3c.B Sub-PR 8.E — migrated to `with_fix_intent` constructor
        // signaling consciously-decided-no-fix-intent (Category B Reject,
        // Stage-4 target). See module-level Migration status comment block
        // above.
        vec![Diagnostic::with_fix_intent(
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
//
// # Auto-fix mechanism (PR 3c.B Sub-PR 8.D.1 — FactAdd first consumer)
//
// PR 3c.B Sub-PR 8.D.1 migrates E038 from a no-fix diagnostic to
// intent-only emission and lands FactAdd wiring for CAT_DISSEM in
// `CapcoScheme::apply_intent_to_marking` at the same time. The rule
// emits `FixIntent { ReplacementIntent::FactAdd { TOK_NOFORN, Portion } }`
// alongside `RuleContext::candidate_span`; the engine's
// `synthesize_intent_only_fixes` calls `CapcoScheme::apply_intent`
// to add NOFORN to the marking's `dissem_controls` axis, then
// re-renders the portion via `MarkingScheme::render_canonical`. The
// synthesized `FixProposal.span` covers the full candidate, so the
// re-render places NOFORN in the canonical dissem-controls position
// (§G.1 Table 4 ordering) — no parser-span manipulation by the rule
// is required.
//
// Issue #106 remains open as a tracking ticket for FR-045 parser
// within-category-separator-spans work that other rules genuinely
// need; E038 itself sidesteps it via `synthesize_intent_only_fixes`,
// the same pattern PR #370 established for E041.

pub(crate) struct DeclarativeDosDissemNofornRule;

impl Rule<CapcoScheme> for DeclarativeDosDissemNofornRule {
    fn id(&self) -> RuleId {
        RuleId::new("E038")
    }
    fn name(&self) -> &'static str {
        "dos-dissem-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::{MarkingType, NonIcDissem};

        if violations_for(attrs, "E038/nodis-or-exdis-requires-noforn").is_empty() {
            return vec![];
        }

        // Identify the FIRST NODIS-or-EXDIS entry in source order.
        // The span anchor (`Diagnostic.span`) and the structured-
        // message trigger token (`MessageArgs.token`) MUST agree so a
        // renderer doesn't say "NODIS requires NOFORN" while
        // highlighting an EXDIS token. Scan the full `non_ic_dissem`
        // collection — NOT just the first entry — so a marking that
        // contains other NonIcDissem variants before the trigger
        // (e.g. `(S//DS/ND)` — `DS` is LIMDIS's portion form,
        // `ND` is NODIS's) still fires correctly. The
        // earlier `.first()` shortcut silently dropped diagnostics
        // on such inputs (Copilot review of PR #372, second round).
        //
        // §H.9 supersession (NODIS dominates EXDIS) is the concern of
        // E041, not E038: E041 emits a FactRemove(EXDIS) when both
        // are present, and the supersession-driven token-survival
        // choice flows through that rule's intent. E038's diagnostic
        // simply names *which* triggering token caused this firing,
        // anchored at the same source-position the user sees.
        let nid_spans = spans_of_kind(attrs, TokenKind::NonIcDissem);
        let (trigger_token, trigger_idx) =
            match attrs
                .non_ic_dissem
                .iter()
                .enumerate()
                .find_map(|(i, d)| match d {
                    NonIcDissem::Nodis => Some((crate::scheme::TOK_NODIS, i)),
                    NonIcDissem::Exdis => Some((crate::scheme::TOK_EXDIS, i)),
                    _ => None,
                }) {
                Some(pair) => pair,
                // Catalog predicate fired but the collection contains no
                // NODIS or EXDIS — should be unreachable per the
                // `E038/nodis-or-exdis-requires-noforn` predicate's
                // axis-presence gate, but bail rather than emit a
                // mis-attributed diagnostic.
                None => return vec![],
            };
        let span = nid_spans
            .get(trigger_idx)
            .map(|ts| ts.span)
            .unwrap_or_else(|| first_span_of(attrs, TokenKind::NonIcDissem));

        // Scope follows the marking surface — banner-form firings
        // (e.g., `SECRET//NODIS`) need `Scope::Page` so the engine's
        // synthesis path re-renders the banner; portion-form firings
        // need `Scope::Portion`. CABs and page-break candidates are
        // never §H.9 surfaces and bail (per Copilot review of PR #372).
        let scope = match ctx.marking_type {
            MarkingType::Portion => Scope::Portion,
            MarkingType::Banner => Scope::Page,
            _ => return vec![],
        };

        vec![Diagnostic::with_intent_at_span(
            self.id(),
            self.default_severity(),
            span,
            ctx.candidate_span,
            "NODIS and EXDIS may be used only with NOFORN information; \
             add NOFORN to the dissem controls",
            "CAPCO-2016 §H.9 p172 + p174",
            e038_add_noforn_intent(trigger_token, scope),
        )]
    }
}

/// Build the `FactAdd { NOFORN, scope }` intent emitted by
/// [`DeclarativeDosDissemNofornRule`]. NOFORN is the missing required
/// token per CAPCO-2016 §H.9 p172 (EXDIS "Requires NOFORN") + p174
/// (NODIS "Requires NOFORN") — both passages use the verb "Requires"
/// verbatim, which is what makes `MessageTemplate::RequiredByPresence`
/// the right structured-message variant.
///
/// `trigger_token` carries the NODIS-or-EXDIS token that fired the
/// rule, derived from the first NODIS-or-EXDIS entry in source order
/// (scanning the full `non_ic_dissem` collection, not just position
/// zero — `(S//DS/ND)` must still fire, where `DS` is LIMDIS's
/// portion form and `ND` is NODIS's). It agrees with
/// `Diagnostic.span` (the rule's surface anchor) and flows into
/// `MessageArgs.token` so consumers can render "NODIS requires
/// NOFORN" vs "EXDIS requires NOFORN" without re-parsing the
/// message string. `expected_token` is `TOK_NOFORN` — the absent
/// token whose presence the source requires.
///
/// `scope` follows the marking surface: `Scope::Portion` for portion
/// marks, `Scope::Page` for banner marks. The engine's
/// `synthesize_intent_only_fixes` re-renders the corresponding
/// candidate-span window via `MarkingScheme::apply_intent` +
/// `MarkingScheme::render_canonical`; the scope tag tells the codec
/// which surface to emit.
///
/// Confidence is `Confidence::strict(1.0)` — the source is
/// unambiguous about the required companion, and the strict
/// recognizer path is what produced the parse that surfaced the
/// triggering NODIS/EXDIS token. Mirrors the calibration used by
/// `nodis_supersedes_exdis_intent` in `rules.rs` (the matching §H.9
/// supersession rule).
fn e038_add_noforn_intent(trigger_token: TokenId, scope: Scope) -> FixIntent<CapcoScheme> {
    use crate::scheme::TOK_NOFORN;
    FixIntent {
        replacement: ReplacementIntent::FactAdd {
            token: FactRef::Cve(TOK_NOFORN),
            scope,
        },
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        message: Message::new(
            MessageTemplate::RequiredByPresence,
            MessageArgs {
                token: Some(trigger_token),
                expected_token: Some(TOK_NOFORN),
                ..MessageArgs::default()
            },
        ),
    }
}

// ---------------------------------------------------------------------------
// E053 — NOFORN conflicts with REL TO (§H.8 p145)
// ---------------------------------------------------------------------------
// (See below for E054–E057, PR 3b.C RELIDO incompatibility wrappers.)
//
// **Migration status (PR 3c.B Sub-PR 8.D.2, 2026-05-12):** scope-
// keyed migration. CAPCO-2016 §H.8 p145 NOFORN entry states verbatim:
// "Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY."
// NOFORN unambiguously supersedes REL TO — there is no policy
// ambiguity, no alternative fill, no foreign-arrangement fact
// required to choose: NOFORN wins, REL TO must be removed.
//
// Two emission surfaces:
//
// 1. **Portion scope** — intent-only emission.
//    `FactRemove { FactRef::Cve(TOK_REL_TO), Scope::Portion }` via the
//    `TOK_REL_TO` whole-axis-clear sentinel `apply_fact_remove`'s
//    CAT_REL_TO branch was extended to accept in this sub-PR. The
//    engine's `synthesize_intent_only_fixes` re-renders the portion
//    after `apply_intent` clears the REL TO axis. Analog to the
//    CAT_NON_IC_DISSEM EXDIS sentinel that PR #370 / Sub-PR 8.E.2
//    wired for E041.
//
// 2. **Banner / CAB scope** — no-fix-intent diagnostic.
//    The page-level mutation is the responsibility of the
//    `capco/noforn-clears-rel-to` PageRewrite declared in
//    `CapcoScheme::build_page_rewrites`
//    (`crates/capco/src/scheme.rs:786`, cited at §D.2 Table 3 +
//    §H.8 p145). Adding a `Scope::Page` arm here would cause a
//    double rewrite — the page would be re-rolled once by the
//    rewrite scheduler and once by this rule's intent synthesis.
//    The rule still emits a diagnostic at banner scope so a user
//    typing the malformed banner directly sees the violation, but
//    the diagnostic carries no `fix_intent`: the banner mutation
//    flows through the PageRewrite, not through the rule.

pub(crate) struct DeclarativeNofornRelToConflictRule;

impl Rule<CapcoScheme> for DeclarativeNofornRelToConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E053")
    }
    fn name(&self) -> &'static str {
        "noforn-rel-to-conflict"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        use marque_ism::MarkingType;

        if violations_for(attrs, "capco/noforn-conflicts-rel-to").is_empty() {
            return vec![];
        }

        // Point to NOFORN, the disallowing control: §H.8 p145 says NOFORN
        // "Cannot be used with REL TO." The REL TO block is also present,
        // but NOFORN is the asserting token that makes REL TO invalid.
        // Reuse `find_dissem_token_span` (shared with RELIDO / ORCON
        // conflict wrappers) to keep span-selection consistent across
        // declarative dissem-conflict rules.
        let span = find_dissem_token_span(attrs, &["NOFORN", "NF"])
            .unwrap_or_else(|| first_span_of(attrs, TokenKind::RelToBlock));

        // Scope-keyed emission. Portion → intent-only with FactRemove;
        // banner/CAB → diagnostic-only (page mutation is the
        // `capco/noforn-clears-rel-to` PageRewrite's responsibility).
        // PageBreak and other marking types do not carry §H.8 surfaces
        // and bail (matches E038's pattern post-Copilot review of PR #372).
        match ctx.marking_type {
            MarkingType::Portion => {
                // PR 3c.B Sub-PR 8.D.2 — intent-only emission. The
                // diagnostic's `span` points at the NOFORN token (the
                // user-facing pointer); `candidate_span` is the full
                // portion candidate so the engine's
                // `synthesize_intent_only_fixes` knows which
                // scope-bytes to re-render after
                // `CapcoScheme::apply_intent` clears the REL TO axis
                // via the `TOK_REL_TO` whole-axis sentinel.
                vec![Diagnostic::with_intent_at_span(
                    self.id(),
                    self.default_severity(),
                    span,
                    ctx.candidate_span,
                    "NOFORN supersedes REL TO (§H.8 p145); REL TO removed",
                    "CAPCO-2016 §H.8 p145",
                    e053_remove_rel_to_intent(),
                )]
            }
            MarkingType::Banner | MarkingType::Cab => {
                // Diagnostic-only at banner / CAB scope — see module-
                // level comment. The `capco/noforn-clears-rel-to`
                // PageRewrite handles the page-level mutation; a
                // page-scope `FactRemove` intent here would
                // double-rewrite.
                //
                // Audit-correlation note for downstream consumers:
                // the `AppliedFix` record for the corresponding
                // mutation lands under the `capco/noforn-clears-rel-to`
                // PageRewrite identity, NOT under E053. A consumer
                // joining "what fixed this NOFORN/REL TO violation?"
                // at banner scope must look at the PageRewrite audit
                // row; the E053 row at banner scope carries the
                // diagnostic only.
                vec![Diagnostic::with_fix_intent(
                    self.id(),
                    self.default_severity(),
                    span,
                    "NOFORN cannot be used with REL TO (§H.8 p145); \
                     remove one or the other",
                    "CAPCO-2016 §H.8 p145",
                    None,
                )]
            }
            _ => vec![],
        }
    }
}

/// Build the `FactRemove { TOK_REL_TO, Scope::Portion }` intent
/// emitted by [`DeclarativeNofornRelToConflictRule`]. The
/// `TOK_REL_TO` sentinel routes to `apply_fact_remove`'s CAT_REL_TO
/// whole-axis-clear arm (PR 3c.B Sub-PR 8.D.2) — REL TO is cleared
/// entirely, not just USA, because §H.8 p145 says NOFORN cannot be
/// used with REL TO **at all** (no per-country exemption).
///
/// Confidence is `Confidence::strict(1.0)` — the source is
/// unambiguous ("Cannot be used with REL TO" is a categorical
/// prohibition, not a context-dependent guideline), and the strict
/// recognizer path is what produced the parse that surfaced both
/// the NOFORN token and the REL TO block. Mirrors the calibration
/// used by `nodis_supersedes_exdis_intent` in `rules.rs` and the
/// other strict-path intent builders in this crate.
///
/// `feature_ids` uses `Default::default()` (empty `SmallVec`) to
/// stay consistent with the other strict-path intent builders.
///
/// Message uses `MessageTemplate::ConflictsWith`: §H.8 mutual-
/// exclusion with a dominated + surviving token, NOT §F deprecation
/// / canonical-replacement. `token` = the dominated REL TO (carried
/// via the `TOK_REL_TO` sentinel that identifies the axis);
/// `expected_token` = the surviving NOFORN.
fn e053_remove_rel_to_intent() -> FixIntent<CapcoScheme> {
    use crate::scheme::{TOK_NOFORN, TOK_REL_TO};
    FixIntent {
        replacement: ReplacementIntent::FactRemove {
            token_ref: FactRef::Cve(TOK_REL_TO),
            scope: Scope::Portion,
        },
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        message: Message::new(
            MessageTemplate::ConflictsWith,
            MessageArgs {
                token: Some(TOK_REL_TO),
                expected_token: Some(TOK_NOFORN),
                ..MessageArgs::default()
            },
        ),
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
/// for hand-written CAPCO rules (the PM Addendum II Section 4 reference to
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

impl Rule<CapcoScheme> for DeclarativeRelidoNofornConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E054")
    }

    fn name(&self) -> &'static str {
        "relido-noforn-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
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
        //
        // PR 3c.B Commit 3 (E054 migration). Dual-population per Path C:
        // `fix` carries the byte-identical pre-migration projection
        // (the engine's NDJSON shape stays stable through commits 2–9);
        // `fix_intent` carries the new structural FactRemove emission.
        // The engine pairs them at promotion time and routes to
        // `AppliedFixProposal::New { intent, synthesized: fix }`. See
        // `crates/engine/src/engine.rs::fix_inner` and the consolidated
        // plan §"Path C" (lines 100–175). Commit 10 retires the
        // synthesized projection atomically with the audit-schema flip.
        match build_relido_removal_fix(self.id(), attrs) {
            Some(fix) => vec![Diagnostic::with_fix_and_intent(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: cannot be used with NOFORN (§H.8 p154)",
                "CAPCO-2016 §H.8 p154",
                fix,
                relido_remove_intent(),
            )],
            None => vec![Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: cannot be used with NOFORN (§H.8 p154)",
                "CAPCO-2016 §H.8 p154",
                None,
            )],
        }
    }
}

/// Build the canonical `FactRemove { RELIDO, Scope::Portion }` intent
/// shared by every RELIDO-removal wrapper (E054 / E057 in Commit 3;
/// E055 / E056 follow in later commits of PR 3c.B). RELIDO removal is
/// scope-portion: the fact set the rule mutates is a single portion's
/// dissem-axis projection, not a page-level roll-up.
///
/// Confidence here mirrors the pre-migration FixProposal's
/// `Confidence::strict(0.95)` (see `build_relido_removal_fix`) so the
/// engine's threshold gate produces identical filter behavior on
/// `fix_intent.confidence.combined()` vs the legacy
/// `fix.confidence.combined()` path. PR 3c.B Commit 10 collapses these
/// to a single emission path; until then both must agree.
fn relido_remove_intent() -> FixIntent<CapcoScheme> {
    use crate::scheme::TOK_RELIDO;
    FixIntent {
        replacement: ReplacementIntent::FactRemove {
            token_ref: FactRef::Cve(TOK_RELIDO),
            scope: Scope::Portion,
        },
        confidence: Confidence::strict(0.95),
        feature_ids: Default::default(),
        message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
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

impl Rule<CapcoScheme> for DeclarativeRelidoDisplayOnlyConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E055")
    }

    fn name(&self) -> &'static str {
        "relido-display-only-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
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
        //
        // PR 3c.B Commit 8 (E055 migration). Dual-population per Path C
        // — same shape as E054 above. See `relido_remove_intent()` for
        // the shared structural emission and the Path C / Commit-10
        // retirement rationale. RELIDO is the rejected token in both
        // §H.8 p154 conflict cases (NOFORN, DISPLAY ONLY), so the
        // intent helper is reused as-is.
        match build_relido_removal_fix(self.id(), attrs) {
            Some(fix) => vec![Diagnostic::with_fix_and_intent(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: cannot be used with DISPLAY ONLY (§H.8 p154)",
                "CAPCO-2016 §H.8 p154",
                fix,
                relido_remove_intent(),
            )],
            None => vec![Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: cannot be used with DISPLAY ONLY (§H.8 p154)",
                "CAPCO-2016 §H.8 p154",
                None,
            )],
        }
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

impl Rule<CapcoScheme> for DeclarativeOrconRelidoConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E056")
    }

    fn name(&self) -> &'static str {
        "orcon-relido-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
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
        //
        // PR 3c.B Commit 8 (E056 migration). Dual-population per Path C
        // — same shape as E054 / E057 above. See `relido_remove_intent()`
        // for the shared structural emission and the Path C / Commit-10
        // retirement rationale.
        match build_relido_removal_fix(self.id(), attrs) {
            Some(fix) => vec![Diagnostic::with_fix_and_intent(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: ORCON may not be used with RELIDO (§H.8 p136)",
                "CAPCO-2016 §H.8 p136",
                fix,
                relido_remove_intent(),
            )],
            None => vec![Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: ORCON may not be used with RELIDO (§H.8 p136)",
                "CAPCO-2016 §H.8 p136",
                None,
            )],
        }
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

impl Rule<CapcoScheme> for DeclarativeOrconUsgovRelidoConflictRule {
    fn id(&self) -> RuleId {
        RuleId::new("E057")
    }

    fn name(&self) -> &'static str {
        "orcon-usgov-relido-conflict"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
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
        //
        // PR 3c.B Commit 3 (E057 migration). Dual-population per Path C
        // — same shape as E054 above. See `relido_remove_intent()` for
        // the shared structural emission and the Path C / Commit-10
        // retirement rationale.
        match build_relido_removal_fix(self.id(), attrs) {
            Some(fix) => vec![Diagnostic::with_fix_and_intent(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: ORCON-USGOV may not be used with RELIDO (§H.8 p140)",
                "CAPCO-2016 §H.8 p140",
                fix,
                relido_remove_intent(),
            )],
            None => vec![Diagnostic::new(
                self.id(),
                self.default_severity(),
                span,
                "RELIDO removed: ORCON-USGOV may not be used with RELIDO (§H.8 p140)",
                "CAPCO-2016 §H.8 p140",
                None,
            )],
        }
    }
}

// ===========================================================================
// PR 3b.D (T026d) — Class-floor catalog walker (E058) — RETIRED 3c.B Commit 7.3
// ===========================================================================
//
// The walker `DeclarativeClassFloorRule` (rule ID E058) retired in PR
// 3c.B Commit 7.3. Its 27 class-floor catalog rows now fire through the
// engine's constraint-catalog bridge (`crates/engine/src/engine.rs` lint
// loop, gated by `CapcoScheme::has_diagnostic_constraints()`).
//
// The span-anchor helpers (`class_floor_anchor_span`,
// `first_span_of_optional`) moved to `crate::scheme` next to
// `class_floor_emit`. The bridge folds catalog row names
// (`E058/<purpose>`, `class-floor/<marking>`) to `Diagnostic.rule = "E058"`
// so audit-stream consumers and `.marque.toml [rules] E058 = "off"` config
// overrides continue to work unchanged across the deletion.
//
// See `docs/plans/2026-05-10-pr3c-consolidated-plan.md` §"Commit 7" +
// `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`
// for the architectural rationale.

// ===========================================================================
// PR 3b.E (T026e) — SCI per-system catalog walker (E059) — RETIRED 3c.B Commit 7.4
// ===========================================================================
//
// The walker `DeclarativeSciPerSystemRule` (rule ID E059) retired in PR
// 3c.B Commit 7.4. Its 5 SCI per-system catalog rows now fire through
// the engine's constraint-catalog bridge via the direct path
// `CapcoScheme::bridge_sci_per_system_diagnostics`, with the row's
// `FixProposal` payload preserved (companion-insertion at the dissem-
// block anchor, ORCON-USGOV → ORCON token replacement). The bridge
// folds the catalog row names (`sci-per-system/<purpose>`) into
// `Diagnostic.rule = "E059"` so audit-stream consumers and
// `.marque.toml [rules] E059 = "off"` config overrides continue to
// work unchanged across the deletion.
//
// Per-branch severity escalation is preserved: the
// no-IC-dissem-block branch in `sci_per_system_emit` escalates from
// the row's default `Severity::Warn` to `Severity::Error` no-fix
// (companion-insertion needs a non-empty dissem block to anchor
// against). The bridge calls `sci_per_system_emit` directly, so the
// per-branch escalation flows through unchanged; a non-`Off`
// `[rules] E059 = ...` config override replaces the escalated
// severity uniformly (same behavior as the engine's pre-retirement
// `diags.retain_mut` post-loop override pass applied to the
// walker's output).
//
// The dispatch is "direct" — not via `ConstraintViolation` — because
// `ConstraintViolation` (in `marque-scheme`) cannot carry `FixProposal`
// (in `marque-rules`) without inverting Constitution VII's
// dependency-graph directionality, and a single SCI per-system row can
// emit multiple violations with distinct fixes (HCS-O missing ORCON
// AND missing NOFORN = 2 violations) which a `(name, attrs)` helper
// cannot disambiguate. The catalog rows remain declared as
// `Constraint::Custom` entries in `CapcoScheme::build_constraints()`
// for documentation and naming-prefix invariants; the bridge takes
// the inherent-method shortcut.
//
// See `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`
// (Amendment 4 for the fix-flow architectural decision).
