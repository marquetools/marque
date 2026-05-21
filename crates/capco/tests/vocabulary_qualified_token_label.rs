// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T046 — PR 3c.2.D PM-D-10 pin for
//! `Vocabulary<CapcoScheme>::qualified_token_label`.
//!
//! The audit-record contract at
//! `specs/006-engine-rule-refactor/contracts/audit-record.md` requires
//! the `replacement.canonical.token_id` JSON field to carry the
//! namespaced `"category.canonical"` form so consumers can resolve
//! the canonical token's category without performing a separate
//! vocabulary lookup per record (the self-describing property).
//!
//! This integration test exercises the override on `CapcoScheme`:
//! routes a `TokenId` through `capco_token_category` to find the
//! [`marque_scheme::Category`] row, looks up the category name, and
//! composes it with [`TokenMetadataFull::canonical`].

use marque_capco::CapcoScheme;
use marque_capco::scheme::{TOK_ATOMAL, TOK_BALK, TOK_BOHEMIA, TOK_FRD, TOK_NOFORN, TOK_RD};
use marque_scheme::Vocabulary;

#[test]
fn qualified_token_label_noforn_routes_to_dissem() {
    // `TOK_NOFORN` routes to `CAT_DISSEM` (name = "dissem") per
    // `crates/capco/src/scheme/predicates/token_routing.rs`. Canonical
    // form is "NF" (CVE value column from
    // `CVEnumISMDissem.json` — the short form is the CVE value; the
    // long form "NOFORN" is the description). The audit-record
    // `token_id` field reflects the CVE-canonical short form by
    // design.
    let scheme = CapcoScheme::new();
    let label = scheme.qualified_token_label(&TOK_NOFORN);
    assert_eq!(&*label, "dissem.NF");
}

#[test]
fn qualified_token_label_rd_routes_to_aea() {
    // `TOK_RD` routes to `CAT_AEA` (name = "aea") per the
    // routing table. Canonical form is "RD".
    let scheme = CapcoScheme::new();
    let label = scheme.qualified_token_label(&TOK_RD);
    assert_eq!(&*label, "aea.RD");
}

#[test]
fn qualified_token_label_frd_routes_to_aea() {
    // `TOK_FRD` also routes to `CAT_AEA`. Confirms the
    // category lookup is per-token-correct, not hardcoded.
    let scheme = CapcoScheme::new();
    let label = scheme.qualified_token_label(&TOK_FRD);
    assert_eq!(&*label, "aea.FRD");
}

// Issue #660 — pin the audit-record label for the NATO program
// tokens. The CVE canonical (NATO- prefixed) lands in the
// `token_id` audit field, NOT the bare §G.1 Table 4 p37 display
// form. The divergence is by design: audit consumers resolve
// against the ODNI CVE vocabulary, while the display form is the
// CAPCO-marking-text projection. A future routing refactor that
// silently moved ATOMAL out of `CAT_AEA` (or harmonized the audit
// field to the display form) would corrupt the audit stream
// without triggering any other test — these pins are the gate.

#[test]
fn qualified_token_label_atomal_routes_to_aea() {
    // `TOK_ATOMAL` routes to `CAT_AEA` (NATO Atomic Energy Act
    // marking per CAPCO-2016 §H.7 p122 — `SECRET//RD/ATOMAL//FGI
    // NATO//NOFORN`). Canonical form is `"NATO-ATOMAL"` (CVE
    // value in `CVE_NON_US_CONTROLS`).
    let scheme = CapcoScheme::new();
    let label = scheme.qualified_token_label(&TOK_ATOMAL);
    assert_eq!(&*label, "aea.NATO-ATOMAL");
}

#[test]
fn qualified_token_label_balk_routes_to_sci() {
    // `TOK_BALK` routes to `CAT_SCI` (NATO SAP per CAPCO-2016
    // §G.2 p40 Table 5 registration + §H.7 p127 worked example).
    // Canonical form is `"NATO-BALK"`.
    let scheme = CapcoScheme::new();
    let label = scheme.qualified_token_label(&TOK_BALK);
    assert_eq!(&*label, "sci.NATO-BALK");
}

#[test]
fn qualified_token_label_bohemia_routes_to_sci() {
    // `TOK_BOHEMIA` routes to `CAT_SCI` per the §G.2 p40 + §H.7
    // p127 worked example `(//CTS//BOHEMIA//REL TO USA, NATO)`.
    // Canonical form is `"NATO-BOHEMIA"`.
    let scheme = CapcoScheme::new();
    let label = scheme.qualified_token_label(&TOK_BOHEMIA);
    assert_eq!(&*label, "sci.NATO-BOHEMIA");
}
