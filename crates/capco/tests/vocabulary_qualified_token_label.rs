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
use marque_capco::scheme::{TOK_FRD, TOK_NOFORN, TOK_RD};
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
