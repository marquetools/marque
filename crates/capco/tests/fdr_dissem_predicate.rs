// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for
//! `Vocabulary<CapcoScheme>::is_fdr_dissem`.
//!
//! Per CAPCO-2016 §B.3.a p19 + §H.8 p157, the FD&R set is
//! `{NOFORN, RELIDO, DISPLAY ONLY, REL TO [any LIST], EYES}`
//! (EYES deprecated 2017-10-01 but still recognized for legacy-
//! input compatibility per §H.8 p157).
//!
//! These tests exercise the public `Vocabulary` trait surface only —
//! the bidirectional value-pin against the private
//! `FDR_DOMINATORS` slice (`crates/capco/src/scheme/closure.rs`) lives in
//! `crates/capco/src/vocabulary.rs::fdr_dissem_pin` (a `#[cfg(test)]`
//! unit-test module) because `FDR_DOMINATORS` is `pub(crate)` per
//! the project-memory `pub_doc_hidden_is_still_public_api` discipline:
//! `pub(crate)` + unit-tests is preferred over exposing a
//! test-only `pub` surface.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{
    TOK_DCNI, TOK_DISPLAY_ONLY, TOK_DSEN, TOK_EXDIS, TOK_EYES, TOK_FOUO, TOK_FRD, TOK_HCS,
    TOK_IMCON, TOK_JOINT, TOK_NODIS, TOK_NOFORN, TOK_ORCON, TOK_ORCON_USGOV, TOK_RD, TOK_RELIDO,
    TOK_RESTRICTED, TOK_RSEN, TOK_TFNI, TOK_UCNI, TOK_USA,
};
use marque_scheme::Vocabulary;

fn vocab() -> CapcoScheme {
    CapcoScheme::new()
}

/// The four canonical FD&R-by-TokenId dominators per §B.3.a p19
/// (plus EYES per §H.8 p157, recognize-only) all admit.
///
/// REL TO is the fifth member of the FD&R set but is exercised
/// separately in `is_fdr_dissem_admits_rel_to_country_tokens` below —
/// REL TO has no single `TokenId` form (it's an open-vocab country
/// list), so it can only be admitted through its participating
/// country codes via the override's `FDR_DOMINATORS` iteration.
#[test]
fn is_fdr_dissem_admits_canonical_dominators() {
    let v = vocab();
    assert!(
        v.is_fdr_dissem(&TOK_NOFORN),
        "NOFORN is FD&R per §B.3.a p19",
    );
    assert!(
        v.is_fdr_dissem(&TOK_RELIDO),
        "RELIDO is FD&R per §B.3.a p19. Important regression \
         pin: the neighboring is_fdr_dominator predicate \
         (`crates/capco/src/scheme/predicates/families.rs`) \
         excludes RELIDO (RELIDO-vs-RELIDO is a tautology in the \
         RELIDO-conflict family catalog). A regression that delegates \
         is_fdr_dissem through is_fdr_dominator would fail this test.",
    );
    assert!(
        v.is_fdr_dissem(&TOK_DISPLAY_ONLY),
        "DISPLAY ONLY is FD&R per §B.3.a p19",
    );
    assert!(
        v.is_fdr_dissem(&TOK_EYES),
        "EYES is FD&R per §H.8 p157 (deprecated 2017-10-01, \
         recognize-only for legacy input compatibility)",
    );
}

/// REL TO country trigraph sentinels admit via the
/// `capco_token_category == Some(CAT_REL_TO)` arm of the override.
#[test]
fn is_fdr_dissem_admits_rel_to_country_tokens() {
    let v = vocab();
    assert!(
        v.is_fdr_dissem(&TOK_USA),
        "TOK_USA (the country trigraph sentinel routed to CAT_REL_TO) \
         must admit so REL TO country lists are recognized as FD&R \
         per §B.3.a p19 + §H.8 p150",
    );
}

/// Dissemination tokens that are NOT in the FD&R set return `false`.
///
/// The §B.3.a p19 enumeration is exclusive: only the listed
/// tokens are FD&R. ORCON / ORCON-USGOV / IMCON / DSEN / RSEN /
/// FOUO are dissemination controls but NOT FD&R dominators — they
/// are themselves *triggers* for the implicit-NOFORN closure rules
/// in `marque-applied.md` §4.7.1, not suppressors of those rules.
#[test]
fn is_fdr_dissem_rejects_non_fdr_dissem_tokens() {
    let v = vocab();
    let non_fdr_dissem: &[(&str, marque_scheme::TokenId)] = &[
        // IC dissemination controls — §H.8.
        ("ORCON (§H.8 p136)", TOK_ORCON),
        ("ORCON-USGOV (§H.8 p140)", TOK_ORCON_USGOV),
        ("IMCON (§H.8 p142)", TOK_IMCON),
        ("DSEN (§H.8 p159)", TOK_DSEN),
        ("RSEN (§H.8 p132)", TOK_RSEN),
        ("FOUO (§H.8 p134)", TOK_FOUO),
        // Non-IC dissemination controls — §H.9.
        ("NODIS (§H.9 p174)", TOK_NODIS),
        ("EXDIS (§H.9 p172)", TOK_EXDIS),
    ];
    for (label, id) in non_fdr_dissem {
        assert!(
            !v.is_fdr_dissem(id),
            "{label} is a dissemination control but NOT FD&R per \
             §B.3.a p19 — is_fdr_dissem returned true \
             unexpectedly",
        );
    }
}

/// Tokens outside the dissemination axis entirely return `false`.
///
/// FD&R is a dissem-axis concept; tokens from other categories
/// (classification, AEA, SCI, JOINT) cannot participate. This test
/// pins the "narrow surface" intent of the predicate so a future
/// over-broad override that returns `true` for all tokens fails.
#[test]
fn is_fdr_dissem_rejects_non_dissem_tokens() {
    let v = vocab();
    let non_dissem: &[(&str, marque_scheme::TokenId)] = &[
        // Classification — non-US RESTRICTED, cited where the JOINT-
        // conflicts-RESTRICTED rule lives (§H.3 p56) rather than §H.1
        // (which does not define a standalone "RESTRICTED" — only
        // TS / S / C / U). NATO RESTRICTED appears at §H.2 / Appendix
        // B tables, but the rule consumer is the §H.3 p56 prose.
        ("RESTRICTED (§H.3 p56)", TOK_RESTRICTED),
        // AEA — §H.6 (per CAPCO-2016 TOC §H.6 pp 104, 111, 116, 120).
        ("RD (§H.6 p104)", TOK_RD),
        ("FRD (§H.6 p111)", TOK_FRD),
        ("TFNI (§H.6 p120)", TOK_TFNI),
        // Per issue #407, the two UCNI sentinels are separate:
        // `TOK_UCNI` resolves to `AeaMarking::DoeUcni` (DOE UCNI at
        // §H.6 p118), `TOK_DCNI` resolves to `AeaMarking::DodUcni`
        // (DOD UCNI at §H.6 p116). Both are non-dissem AEA tokens.
        ("DOE UCNI (§H.6 p118)", TOK_UCNI),
        ("DOD UCNI (§H.6 p116)", TOK_DCNI),
        // SCI — §H.4 p62 (HCS template start; p61 is the §H.4 SCI
        // overview header, not the HCS marking itself).
        ("HCS (§H.4 p62)", TOK_HCS),
        // JOINT classification marker — §H.3 p56 (JOINT template;
        // §H.3 p55 is the section header).
        ("JOINT (§H.3 p56)", TOK_JOINT),
    ];
    for (label, id) in non_dissem {
        assert!(
            !v.is_fdr_dissem(id),
            "{label} is a non-dissemination token — is_fdr_dissem \
             must return false; FD&R is a dissem-axis concept \
             per §B.3.a p19",
        );
    }
}
