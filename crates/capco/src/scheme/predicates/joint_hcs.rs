// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! JOINT REL TO + HCS-system-constraints predicate bodies. Both are
//! `Constraint::Custom` handlers that need richer-than-`Conflicts`
//! semantics (USA dual-presence for JOINT; per-compartment
//! classification with companion logic for HCS). Lifted from the
//! monolithic `predicates.rs` per the issue #466 Stage 2 PR A leaf
//! split (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_ism::{Classification, CountryCode};
use marque_scheme::{SectionLetter, Severity, TokenRef, capco};

use super::super::*;
use super::spans::{first_sci_span, token_span_attrs};

/// `capco/joint-requires-usa` — JOINT classifications must list USA in BOTH
/// `joint.countries` AND `rel_to`. CAPCO §H.3 p55 (USA always included in
/// JOINT [LIST]) + §H.3 p57 (Requires REL TO USA, LIST).
pub(crate) fn joint_requires_usa(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let joint = match &attrs.classification {
        Some(marque_ism::MarkingClassification::Joint(j)) => j,
        _ => return Vec::new(),
    };
    let has_usa_in_rel_to = attrs.rel_to.contains(&CountryCode::USA);
    let joint_includes_usa = joint.countries.contains(&CountryCode::USA);
    if has_usa_in_rel_to && joint_includes_usa {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "capco/joint-requires-usa",
        message: "JOINT classifications must list USA in both the \
                  classification countries and REL TO"
            .to_owned(),
        citation: capco(SectionLetter::H, 3, 55),
        span: token_span_attrs(attrs, &TokenRef::Token(TOK_JOINT)),
        severity: Some(Severity::Error),
    }]
}

// ---------------------------------------------------------------------------
// HCS constraint handler (CAPCO-2016 §H.4 pp 62–66)
// ---------------------------------------------------------------------------

/// Evaluate the `Constraint::Custom("HCS-system-constraints")` sample.
///
/// CAPCO-2016 §H.4 (pp 62–66) defines the interlocking HCS rules:
///
/// 1. **Bare `HCS` (no compartment)** is a legacy form (§H.4 p62). It
///    must be remarked to `HCS-P`, `HCS-O`, or `HCS-O-P`, which requires
///    document-level analysis (the correct variant depends on whether
///    the content is HUMINT product, operations, or both). Legacy
///    `C//HCS` (CONFIDENTIAL with bare HCS -- no compartment) must
///    additionally be identified to the originator for correction.
/// 2. **`HCS-O`** (§H.4 p64) **requires ORCON and NOFORN** and must
///    **not** include ORCON-USGOV (banner would drop -USGOV).
/// 3. **`HCS-P`** (§H.4 p66) **requires NOFORN**; ORCON or ORCON-USGOV
///    **may** be used (permitted, not required).
/// 4. **`HCS-O` / `HCS-P`** are only authorized for SECRET and TOP
///    SECRET classifications (§H.4 p64 / p66).
///
/// This helper inspects both `sci_controls` (the CVE-projection for
/// legacy-shape bare HCS tokens) and `sci_markings` (the structural
/// view that carries compartment identifiers). Emits one
/// `ConstraintViolation` per failing rule per offending HCS entry.
///
/// By far the most common HCS compartment is `HCS-P` (Product).
/// HCS-O (Operations) is rarely encountered outside of CIA's walls.
/// But for users in that environment, they may encounter all three variants routinely.
pub(crate) fn hcs_system_constraints(
    attrs: &marque_ism::CanonicalAttrs,
    citation: marque_scheme::Citation,
) -> Vec<marque_scheme::ConstraintViolation> {
    use marque_ism::{DissemControl, SciControl, SciControlBare, SciControlSystem};

    let mut out = Vec::new();

    let classification = attrs.us_classification();
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc);
    let has_orcon_usgov = attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let high_enough = matches!(
        classification,
        Some(Classification::Secret) | Some(Classification::TopSecret)
    );

    // Walk structural sci_markings for HCS systems. This is the
    // authoritative source for the compartment identifier.
    for marking in attrs.sci_markings.iter() {
        let is_hcs = matches!(
            marking.system,
            SciControlSystem::Published(SciControlBare::Hcs)
        );
        if !is_hcs {
            continue;
        }

        if marking.compartments.is_empty() {
            // Bare HCS — legacy per CAPCO-2016 §H.4 p62.
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-bare",
                message: "Bare HCS is legacy; remark to HCS-P, HCS-O, or HCS-O-P per CAPCO-2016 \
                     §H.4 p62 (requires document-level analysis)."
                    .to_owned(),
                citation,
                span: first_sci_span(attrs),
                severity: Some(Severity::Error),
            });
            if classification == Some(Classification::Confidential) {
                out.push(marque_scheme::ConstraintViolation {
                    constraint_label: "HCS-legacy-confidential",
                    message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction \
                              per CAPCO-2016 §H.4 p62."
                        .to_owned(),
                    citation,
                    span: first_sci_span(attrs),
                    severity: Some(Severity::Error),
                });
            }
            continue;
        }

        // For each HCS-{first compartment} variant, apply the O/P
        // specific rules and the SECRET / TOP SECRET floor.
        for comp in marking.compartments.iter() {
            let id = comp.identifier.as_ref();
            match id {
                "O" => {
                    if !high_enough {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-classification-floor",
                            message: "HCS-O is only authorized for SECRET and TOP SECRET per \
                                      CAPCO-2016 §H.4 p64."
                                .to_owned(),
                            citation,
                            span: first_sci_span(attrs),
                            severity: Some(Severity::Error),
                        });
                    }
                    if !has_orcon {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-ORCON",
                            message: "HCS-O requires ORCON per CAPCO-2016 §H.4 p64.".to_owned(),
                            citation,
                            span: first_sci_span(attrs),
                            severity: Some(Severity::Error),
                        });
                    }
                    if has_orcon_usgov {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-forbids-ORCON-USGOV",
                            message: "HCS-O must not be used with ORCON-USGOV per CAPCO-2016 \
                                      §H.4 p64."
                                .to_owned(),
                            citation,
                            span: first_sci_span(attrs),
                            severity: Some(Severity::Error),
                        });
                    }
                    // HCS-O requires NOFORN per CAPCO-2016 §H.4 p64
                    // ("Relationship(s) to Other Markings: ... Requires
                    // ORCON and NOFORN"). The ORCON side is enforced
                    // above; NOFORN is the second mandatory side. Same
                    // shape as the HCS-P NOFORN-required predicate
                    // below; tracked-and-resolved per #304.
                    let has_noforn = attrs.dissem_iter().any(|d| d == &DissemControl::Nf);
                    if !has_noforn {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-NOFORN",
                            message: "HCS-O requires NOFORN per CAPCO-2016 §H.4 p64.".to_owned(),
                            citation,
                            span: first_sci_span(attrs),
                            severity: Some(Severity::Error),
                        });
                    }
                }
                "P" => {
                    if !high_enough {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-classification-floor",
                            message: "HCS-P is only authorized for SECRET and TOP SECRET per \
                                      CAPCO-2016 §H.4 p66."
                                .to_owned(),
                            citation,
                            span: first_sci_span(attrs),
                            severity: Some(Severity::Error),
                        });
                    }
                    // HCS-P requires NOFORN per CAPCO-2016 §H.4 p66
                    // ("Relationship(s) to Other Markings: ... Requires
                    // NOFORN"). ORCON / ORCON-USGOV are permitted but
                    // not required ("ORCON or ORCON-USGOV may be
                    // used."), so the ORCON-required predicate that
                    // previously fired here was over-strict; it is
                    // dropped in favor of the actually-required
                    // NOFORN predicate.
                    let has_noforn = attrs.dissem_iter().any(|d| d == &DissemControl::Nf);
                    if !has_noforn {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-requires-NOFORN",
                            message: "HCS-P requires NOFORN per CAPCO-2016 §H.4 p66.".to_owned(),
                            citation,
                            span: first_sci_span(attrs),
                            severity: Some(Severity::Error),
                        });
                    }
                }
                _ => {
                    // Other HCS compartments (e.g., agency-specific
                    // extensions not yet in this sample) fall through.
                }
            }
        }
    }

    // Back-compat: a portion may carry `SciControl::Hcs` (the CVE
    // projection for bare HCS) without producing a `sci_markings`
    // entry in every test path. Treat a bare `SciControl::Hcs` in the
    // projection but no corresponding `sci_markings` entry as legacy
    // bare HCS too. This keeps the handler robust to the two-path
    // storage (CVE enum vs structural) that `CanonicalAttrs` carries
    // for back-compat — see crate-level docs on the hybrid SCI model.
    let structural_has_hcs = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs)));
    let projection_has_bare_hcs = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Hcs));
    if projection_has_bare_hcs && !structural_has_hcs {
        out.push(marque_scheme::ConstraintViolation {
            constraint_label: "HCS-legacy-bare",
            // suggested fix should be HCS-P but we should expose a default override path for users in the HCS-O environment
            message: "HCS requires a compartment (O or P); remark to HCS-P, HCS-O, or HCS-O-P \
                 per CAPCO-2016 §H.4 p62 (requires document-level analysis)."
                .to_owned(),
            citation,
            span: first_sci_span(attrs),
            severity: Some(Severity::Error),
        });
        if classification == Some(Classification::Confidential) {
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-confidential",
                message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction per \
                          CAPCO-2016 §H.4 p62."
                    .to_owned(),
                citation,
                span: first_sci_span(attrs),
                severity: Some(Severity::Error),
            });
        }
    }

    out
}
