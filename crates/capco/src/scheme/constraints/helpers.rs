// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Custom-constraint predicate bodies (E012, E014) plus the catalog-row
//! emit helpers (`class_floor_emit`, `sci_per_system_emit`). Lifted
//! from the monolithic `constraints.rs` per the issue #466 Stage 2 PR
//! A leaf split (`claudedocs/refactor-466/stage2_leaves_plan.md`).
//! W002 retired in the PR closing #470 — its predicate body and
//! catalog row are gone. PR-E (#371) retired the tier-1 helpers
//! (`e021_rd_frd_requires_noforn`, `e024_rd_precedence`,
//! `e038_dos_dissem_requires_noforn`, `e070_frd_tfni_precedence`) in
//! favor of `crates/capco/src/scheme/predicates/tier1_mask.rs`,
//! which compiles each predicate to a [`FactBitmask`] trigger /
//! suppressor mask test.
//!
//! [`FactBitmask`]: marque_scheme::FactBitmask

use super::super::actions::emit_companion_required;
use super::super::predicates::{
    class_floor_anchor_span, class_floor_satisfied, rel_to_covers, token_span_attrs,
};
use super::super::*;
use marque_ism::TokenKind;
use marque_scheme::{SectionLetter, capco};
use marque_scheme::{Severity, Span, TokenRef};

// ---------------------------------------------------------------------------
// T035 Custom-constraint helpers
// ---------------------------------------------------------------------------
//
// Each helper is the predicate body for a `Constraint::Custom` entry in
// `build_constraints`. The helpers do NOT reference `RuleContext` — only
// `CanonicalAttrs`. Per-context filtering lives in the engine-bridge
// layer (`crate::scheme::adapter`); the catalog represents "this marking
// is structurally inconsistent" without regard to where the marking appears.
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
            constraint_label: "portion.classification.dual-classification",
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
            citation: capco(SectionLetter::H, 3, 55),
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
        constraint_label: "portion.classification.joint-requires-rel-to-coverage",
        message: format!(
            "JOINT participants [{}] must appear in REL TO list",
            missing.join(", ")
        ),
        citation: capco(SectionLetter::H, 3, 57),
        span: token_span_attrs(attrs, &TokenRef::Token(TOK_JOINT)),
        severity: Some(Severity::Fix),
    }]
}

/// W005 — REL TO list contains entries not in the JOINT participant list.
/// §H.3 p57 permits expanding REL TO beyond co-owners via "[LIST]" superset
/// semantics; this Warn surfaces unexpected expansions for classifier review.
/// USA is excluded (implicit US co-ownership). Tetragraphs in REL TO expand
/// before the check.
///
/// This is the reverse direction of [`e014_joint_rel_to_coverage`]: E014
/// flags JOINT participants missing from REL TO (auto-fixable — policy
/// mandates coverage). W005 flags REL TO entries beyond JOINT (advisory
/// only — §H.3 p57 "[LIST]" superset semantics permit intentional expansion,
/// so Marque cannot distinguish intentional from accidental without
/// classifier input).
pub(crate) fn w005_rel_to_not_in_joint_coverage(
    attrs: &marque_ism::CanonicalAttrs,
) -> Vec<ConstraintViolation> {
    let joint = match &attrs.classification {
        Some(marque_ism::MarkingClassification::Joint(j)) => j,
        _ => return Vec::new(),
    };
    if attrs.rel_to.is_empty() {
        return Vec::new();
    }
    let joint_set: std::collections::HashSet<&str> =
        joint.countries.iter().map(|c| c.as_str()).collect();
    let mut not_in_joint: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for rel_entry in &attrs.rel_to {
        let code = rel_entry.as_str();
        if let Some(members) = crate::vocab::expand_tetragraph(code) {
            for member in members {
                if *member != "USA" && !joint_set.contains(member) {
                    not_in_joint.insert(member);
                }
            }
        } else if code != "USA" && !joint_set.contains(code) {
            not_in_joint.insert(code);
        }
    }
    if not_in_joint.is_empty() {
        return Vec::new();
    }
    let entries: Vec<&str> = not_in_joint.iter().copied().collect();
    vec![ConstraintViolation {
        constraint_label: "portion.classification.rel-to-not-in-joint-coverage",
        message: format!(
            "REL TO {} not in JOINT participant list: {}",
            if entries.len() == 1 {
                "entry"
            } else {
                "entries"
            },
            entries.join(", ")
        ),
        citation: capco(SectionLetter::H, 3, 57),
        span: token_span_attrs(attrs, &TokenRef::Token(TOK_JOINT)),
        severity: Some(Severity::Warn),
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
