// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Custom-constraint predicate bodies (E012, E014, E021, E024, E038)
//! plus the catalog-row emit helpers (`class_floor_emit`,
//! `sci_per_system_emit`). Lifted from the monolithic `constraints.rs`
//! per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`). W002 retired in
//! the PR closing #470 — its predicate body and catalog row are gone.

use super::super::actions::emit_companion_required;
use super::super::predicates::{
    class_floor_anchor_span, class_floor_satisfied, rel_to_covers, token_span_attrs,
};
use super::super::*;
use marque_ism::TokenKind;
use marque_scheme::{Severity, Span, TokenRef};

// ---------------------------------------------------------------------------
// T035 Custom-constraint helpers
// ---------------------------------------------------------------------------
//
// Each helper is the predicate body for a `Constraint::Custom` entry in
// `build_constraints`. The helpers do NOT reference `RuleContext` — only
// `CanonicalAttrs`. Per-context filtering lives in the wrapper layer
// (`crate::rules_declarative`); the catalog represents "this marking is
// structurally inconsistent" without regard to where the marking appears.
//
// The returned `ConstraintViolation` populates `message` with text that the
// wrapper inspects when constructing the user-facing `Diagnostic`. The
// `constraint_label` and `citation` fields are overwritten by the caller
// (`marque_scheme::constraint::evaluate`'s `Custom` arm) so any placeholder
// values are fine — using the catalog name + label keeps the helpers
// self-documenting in isolation.

/// E012 — `MarkingClassification::Conflict` indicates the parser saw a US
/// classification AND a foreign classification in the same marking. CAPCO
/// §H.3 p55 forbids this ("The US, non-US, and JOINT classification
/// markings are mutually exclusive").
pub(crate) fn e012_dual_classification(
    attrs: &marque_ism::CanonicalAttrs,
) -> Vec<ConstraintViolation> {
    if let Some(marque_ism::MarkingClassification::Conflict { us, foreign }) = &attrs.classification
    {
        let foreign_desc = match foreign.as_ref() {
            marque_ism::ForeignClassification::Nato(n) => format!("NATO ({})", n.banner_str()),
            marque_ism::ForeignClassification::Fgi(f) => {
                let countries: Vec<&str> = f.countries.iter().map(|c| c.as_str()).collect();
                if countries.is_empty() {
                    "FGI".to_owned()
                } else {
                    format!("FGI {}", countries.join(" "))
                }
            }
            marque_ism::ForeignClassification::Joint(j) => {
                let countries: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
                format!("JOINT {}", countries.join(" "))
            }
        };

        // Second Classification token span — that's the foreign one.
        let span = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Classification)
            .nth(1)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0));

        vec![ConstraintViolation {
            constraint_label: "E012/dual-classification",
            // The message here matches the retired `DeclarativeDualClassificationRule`.
            message: format!(
                "marking has both US ({}) and foreign ({}) classification; §H.3 p55 mandates \
                 these are mutually exclusive. CAPCO's pattern when US and non-US classifications \
                 are commingled is to express the overall as a US classification with foreign \
                 provenance in an FGI block (§H.3 p57 JOINT derivative use; §H.3 p59 Example 4 \
                 note); consult §H.7 for the FGI marking format",
                us.banner_str(),
                foreign_desc
            ),
            citation: "CAPCO-2016 §H.3 p55",
            span: Some(span),
            severity: Some(Severity::Fix),
        }]
    } else {
        Vec::new()
    }
}

/// E014 — every JOINT participant must appear in the marking's REL TO list.
/// CAPCO §H.3 p57 ("Requires REL TO USA, LIST" relationship statement).
/// Tetragraphs in REL TO expand to their constituent trigraphs: a participant
/// covered by a tetragraph (e.g., GBR via FVEY) is considered present.
pub(crate) fn e014_joint_rel_to_coverage(
    attrs: &marque_ism::CanonicalAttrs,
) -> Vec<ConstraintViolation> {
    let joint = match &attrs.classification {
        Some(marque_ism::MarkingClassification::Joint(j)) => j,
        _ => return Vec::new(),
    };
    let missing: Vec<&str> = joint
        .countries
        .iter()
        .filter(|c| !rel_to_covers(&attrs.rel_to, c.as_str()))
        .map(|c| c.as_str())
        .collect();
    if missing.is_empty() {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E014/joint-requires-rel-to-coverage",
        message: format!(
            "JOINT participants [{}] must appear in REL TO list",
            missing.join(", ")
        ),
        citation: "CAPCO-2016 §H.3 p57",
        span: token_span_attrs(attrs, &TokenRef::Token(TOK_JOINT)),
        severity: Some(Severity::Fix),
    }]
}

/// E021 — RD or FRD requires NOFORN (unless a sharing agreement under
/// Atomic Energy Act section 123 or 144 applies). CAPCO §H.6 p104 (RD)
/// + p111 (FRD).
///
/// Intentionally narrower than `AnyInCategory(CAT_AEA)`:
/// - **TFNI is excluded.** §H.6 p120 Relationship clause is silent on
///   NOFORN ("May only be used with TOP SECRET, SECRET, or
///   CONFIDENTIAL"); §H.6 p121 Notional Example 2 shows
///   `SECRET//TFNI//REL TO USA, ACGU` as a valid release-authorized
///   marking, and Note 4 ("TFNI may be shared with foreign partners
///   in accordance with existing DNI and IC element guidance") makes
///   the NOFORN requirement contextual, not categorical. Lumping
///   TFNI with RD/FRD would auto-rewrite valid release-authorized
///   TFNI markings — a Constitution VIII fidelity defect.
/// - **UCNI variants are excluded.** Neither DOE UCNI (§H.6 p116) nor
///   DoD UCNI (§H.6 p118) carries the NOFORN requirement.
pub(crate) fn e021_aea_requires_noforn(
    attrs: &marque_ism::CanonicalAttrs,
) -> Vec<ConstraintViolation> {
    let has_rd_or_frd = attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Rd(_) | marque_ism::AeaMarking::Frd(_)
        )
    });
    if !has_rd_or_frd {
        return Vec::new();
    }
    let has_noforn = attrs
        .dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    if has_noforn {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E021/aea-requires-noforn",
        message: "RD/FRD requires NOFORN unless a sharing agreement exists \
                  per the Atomic Energy Act"
            .to_owned(),
        citation: "CAPCO-2016 §H.6 p104 + p111",
        span: token_span_attrs(attrs, &TokenRef::AnyInCategory(CAT_AEA)),
        severity: Some(Severity::Fix),
    }]
}

/// E038 — NODIS / EXDIS require NOFORN. CAPCO-2016 §H.9 p172
/// RELIDO is already cleared by Pattern B (UNCLASSIFIED + other
/// control).
pub(crate) fn e038_dos_dissem_requires_noforn(
    attrs: &marque_ism::CanonicalAttrs,
) -> Vec<ConstraintViolation> {
    use crate::scheme::predicates::satisfies_attrs;
    use marque_ism::NonIcDissem;

    let has_nodis_or_exdis = attrs
        .non_ic_dissem
        .iter()
        .any(|d| matches!(d, NonIcDissem::Nodis | NonIcDissem::Exdis));

    if !has_nodis_or_exdis {
        return Vec::new();
    }

    let has_noforn = satisfies_attrs(attrs, &TokenRef::Token(TOK_NOFORN));
    if has_noforn {
        return Vec::new();
    }

    let trigger_token = attrs
        .non_ic_dissem
        .iter()
        .find_map(|d| match d {
            NonIcDissem::Nodis => Some(TOK_NODIS),
            NonIcDissem::Exdis => Some(TOK_EXDIS),
            _ => None,
        })
        .unwrap_or(TOK_NODIS);

    vec![ConstraintViolation {
        constraint_label: "E038/nodis-or-exdis-requires-noforn",
        message: "NODIS and EXDIS may be used only with NOFORN information".to_string(),
        citation: "CAPCO-2016 §H.9 p172 + p174",
        span: token_span_attrs(attrs, &TokenRef::Token(trigger_token)),
        severity: Some(Severity::Error),
    }]
}

// S004 (REL TO trigraph suggest) is implemented by the
// `RelToTrigraphSuggestRule` walker in `crates/capco/src/rules.rs`,
// not by a `Constraint::Custom` row. The walker owns the predicate
// because its candidate replacement is corpus-derived during
// evaluation and cannot be reproduced from `(name, attrs)` alone via
// the bridge's `fix_intent_by_name` shape.

/// E024 — RD takes precedence over FRD/TFNI. Fires when RD AND any of
/// (FRD, TFNI) are present. The wrapper enumerates per-element to emit one
/// `Diagnostic` per offending marking with byte-precise spans; this helper
/// emits ONE `ConstraintViolation` whose presence signals the wrapper to do
/// that work. CAPCO §H.6 p104.
pub(crate) fn e024_rd_precedence(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_rd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(_)));
    if !has_rd {
        return Vec::new();
    }
    let has_superseded = attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Frd(_) | marque_ism::AeaMarking::Tfni
        )
    });
    if !has_superseded {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E024/rd-precedence",
        message: "RD takes precedence over FRD/TFNI; FRD/TFNI should not appear alongside RD"
            .to_owned(),
        citation: "CAPCO-2016 §H.6 p104",
        span: token_span_attrs(attrs, &TokenRef::AnyInCategory(CAT_AEA)),
        severity: Some(Severity::Fix),
    }]
}

/// E070 — FRD takes precedence over TFNI. Fires when FRD AND TFNI are
/// both present in the same portion.
///
/// CAPCO §H.6 p120 (TFNI subsection precedence rules): *"If the TFNI
/// marking is contained in any portion of a document that contains
/// portions of RD and/or FRD, the RD or FRD takes precedence."* Same
/// page commingling rule: *"If TFNI is commingled with RD or FRD within
/// a portion, the RD or FRD takes precedence and 'RD' or 'FRD,' as
/// appropriate, is annotated in the portion mark."*
///
/// Mirror of [`e024_rd_precedence`] for the FRD-side leg per #559
/// close-out (PM decision 2026-05-19). E024 already covers RD>FRD AND
/// RD>TFNI; this helper adds the FRD>TFNI leg as a distinct row so a
/// "remove TFNI" fix can be attributed to the FRD policy decision
/// independently of RD.
///
/// Co-firing with E024 (when RD AND FRD AND TFNI all present in one
/// portion) is intentional: both relationships hold simultaneously and
/// either fix drives the marking toward canonical form. Constitution V
/// Principle V — each row is one policy decision with its own audit
/// repair lineage.
///
/// # Diagnostic surfacing (deferred)
///
/// Returns ConstraintViolation with `span: None, severity: None` to
/// match the current shape of every dyadic helper (`e012`, `e014`,
/// `e021`, `e024`, `e038`). End-user-visible diagnostic emission lands
/// in a follow-up commit once the broader engine-bridge generalization
/// in `specs/006-engine-rule-refactor/` (issue #578 et al.) wires the
/// catalog-row → `Diagnostic` path. Today the predicate exists and is
/// addressable via [`CapcoScheme::evaluate_named_constraint`] for
/// unit-test validation; no wrapper rule is added in this commit per
/// the parallelization plan at `claudedocs/plans/559-307-closeout.md`.
pub(crate) fn e070_frd_tfni_precedence(
    attrs: &marque_ism::CanonicalAttrs,
) -> Vec<ConstraintViolation> {
    let has_frd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Frd(_)));
    if !has_frd {
        return Vec::new();
    }
    let has_tfni = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Tfni));
    if !has_tfni {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E070/frd-tfni-precedence",
        message: "FRD takes precedence over TFNI; TFNI should not appear alongside FRD".to_owned(),
        citation: "CAPCO-2016 §H.6 p120",
        span: None,
        severity: None,
    }]
}

/// Single source of truth for the class-floor catalog's
/// presence-check + floor-satisfaction-check + diagnostic message
/// shape. PR D R3.1 (R3 C2) consolidated the walker hot-path and the
/// trait/validate path here so a citation, message-text, or
/// floor-comparison change to one row cannot silently diverge between
/// emitters. Post PR 3c.B Commit 7.3 the walker is retired and the
/// engine's constraint-catalog bridge is the sole emitter — but the
/// convergence shape stays for any future second emitter path.
///
/// Returns `None` when the row's predicate does not fire (presence
/// false OR floor satisfied). Returns `Some(ConstraintViolation)`
/// when the row fires; the violation carries the row's `name` as
/// `constraint_label`, the formatted diagnostic message, and the
/// row's `citation` verbatim — matching the
/// `marque_scheme::constraint::evaluate` Custom-arm contract.
///
/// The diagnostic message uses the *effective* classification level
/// (reciprocal-raised for NATO / FGI / JOINT classifications via
/// [`marque_ism::MarkingClassification::effective_level`]) so a
/// portion classified `//NATO SECRET//ATOMAL` reports `SECRET` —
/// not `unknown` — even though `attrs.us_classification()` returns
/// `None` for non-US classification kinds. This is the C1 fix from
/// PR #324 R1; see [`class_floor_satisfied`] doc for the AtLeast vs
/// EqualsU split.
///
/// # Span and severity (PR 3c.B Commit 7.3)
///
/// `span` and `severity` are populated here so the engine's
/// constraint-catalog bridge can surface the violation as a
/// user-facing `Diagnostic` without going through the retired
/// `DeclarativeClassFloorRule` walker:
///   - `span` resolves via [`class_floor_anchor_span`] (lifted from
///     the walker in this commit) so the diagnostic squiggle anchors
///     at the marking token, not the classification token (PM
///     directive #2).
///   - `severity` is the row's authoring intent (`Error` for
///     enumerated rows; `Warn` for passthrough rows per
///     `marque-applied.md` §3.4.6 Q-3.4.6b).
pub(crate) fn class_floor_emit(
    attrs: &marque_ism::CanonicalAttrs,
    row: &ClassFloorRow,
) -> Option<ConstraintViolation> {
    if !(row.presence)(attrs) {
        return None;
    }
    if class_floor_satisfied(attrs, row.policy) {
        return None;
    }
    let level_str = attrs
        .classification
        .as_ref()
        .map(|c| c.effective_level().banner_str())
        .unwrap_or("unknown");
    let message = if row.passthrough {
        format!(
            "{} is known from ISM but not enumerated in CAPCO-2016; provisional classification \
             floor is C (classified). Verify against the current ODNI manual; current \
             classification is {level_str}. (See marque-applied.md §3.7 passthrough policy.)",
            row.marking_label
        )
    } else {
        match row.policy {
            ClassFloorPolicy::AtLeast(floor) => format!(
                "{} requires classification ≥ {} ({}); current classification is {level_str}",
                row.marking_label,
                floor.banner_str(),
                row.citation
            ),
            ClassFloorPolicy::EqualsU => format!(
                "{} may only be used with UNCLASSIFIED information ({}); current classification \
                 is {level_str}",
                row.marking_label, row.citation
            ),
        }
    };
    Some(ConstraintViolation {
        constraint_label: row.name,
        message,
        citation: row.citation,
        span: Some(class_floor_anchor_span(attrs, row)),
        severity: Some(row.severity),
    })
}

/// Single source of truth for the SCI per-system catalog's emit logic.
/// Post-PR-3c.B-Commit-7.4 the engine's constraint-catalog bridge
/// (`CapcoScheme::bridge_sci_per_system_diagnostics`) is the only
/// production caller; the legacy walker `DeclarativeSciPerSystemRule`
/// retired in 7.4 and the trait/validate path
/// (`sci_per_system_catalog_eval`) emits `ConstraintViolation` envelopes
/// without `FixProposal` for non-bridge consumers.
///
/// `#[inline]` because the bridge's hot path is the bench-gate-relevant
/// one and the emit dispatch is a 2-arm match on a `Copy` enum field —
/// inlining lets the compiler hoist the row's presence predicate +
/// kind dispatch into the catalog-walk loop.
///
/// Returns an empty `Vec` when the row's presence predicate doesn't fire
/// or when no diagnostic is warranted; otherwise returns one or more
/// `Diagnostic` values per the row's emit logic.
#[inline]
pub(crate) fn sci_per_system_emit(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_scheme::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    if !(row.presence)(attrs) {
        return Vec::new();
    }
    match row.kind {
        SciPerSystemKind::CompanionRequired { dissem, token_name } => {
            emit_companion_required(attrs, candidate_span, fix_scope, row, dissem, token_name)
        }
        SciPerSystemKind::Custom(emit_fn) => emit_fn(attrs, candidate_span, fix_scope, row),
    }
}
