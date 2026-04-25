// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Table-level smoke tests for the build.rs JSON codepath
//! (Phase 5 PR-1, tasks T080 / T081 / T082).
//!
//! These tests run against the raw `marque-ism::generated::vocabulary`
//! tables, *not* the `marque-scheme::Vocabulary<S>` trait surface —
//! the trait impl lives in `marque-capco` (Phase 5 PR-2 task T084),
//! and `marque-ism` cannot reference scheme types per Constitution VII.
//!
//! The trait-level tests (T071 / T072 / T073 / T074 / T077) land in
//! `crates/capco/tests/vocabulary.rs` once the impl is in place.

use marque_ism::generated::vocabulary::{
    CVE_DISSEM, CVE_FILES, CVE_SCI_CONTROLS, TOKEN_METADATA, lookup_token_metadata,
};

#[test]
fn cve_files_table_is_nonempty() {
    assert!(
        !CVE_FILES.is_empty(),
        "CVE_FILES is empty — build.rs collected zero CveFileMetadata records. \
         Almost always a missing-or-broken JSON sidecar set under \
         crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/."
    );
}

#[test]
fn token_metadata_table_is_nonempty() {
    assert!(
        !TOKEN_METADATA.is_empty(),
        "TOKEN_METADATA is empty — build.rs collected zero token entries."
    );
}

/// Every CVE file in the U.S. ISM-v2022-DEC bundle is published by ODNI
/// under the same URN prefix and identifies the U.S. as owner/producer.
/// A future foreign or coalition vocabulary (NATO, FGI) would need its
/// own bundle, not a member of this slice — so the assertion is binding
/// for the active package.
#[test]
fn every_cve_file_carries_odni_provenance() {
    for f in CVE_FILES {
        assert!(
            !f.urn.is_empty(),
            "CVE file {} has empty URN — ODNI sidecars must publish a URN",
            f.const_name
        );
        assert!(
            f.urn.starts_with("urn:us:gov:ic:cvenum:"),
            "CVE file {} URN {:?} does not start with the expected ODNI prefix \
             `urn:us:gov:ic:cvenum:`",
            f.const_name,
            f.urn
        );
        assert_eq!(
            f.owner_producer, "USA",
            "CVE file {} owner_producer is {:?}, expected \"USA\" (U.S. ISM bundle)",
            f.const_name, f.owner_producer
        );
        assert_eq!(
            f.schema_version, "ISM-v2022-DEC",
            "CVE file {} schema_version is {:?}, expected the pinned package \
             version \"ISM-v2022-DEC\"",
            f.const_name, f.schema_version
        );
    }
}

#[test]
fn every_cve_file_has_a_point_of_contact() {
    for f in CVE_FILES {
        assert!(
            !f.poc_name.is_empty(),
            "CVE file {} has empty POC name — required for audit-trail provenance",
            f.const_name
        );
        assert!(
            !f.poc_email.is_empty(),
            "CVE file {} has empty POC email",
            f.const_name
        );
        assert!(
            f.poc_email.contains('@'),
            "CVE file {} POC email {:?} is not an email address",
            f.const_name,
            f.poc_email
        );
    }
}

/// `TOKEN_METADATA` is emitted in sorted-by-value order so
/// `lookup_token_metadata` can use `binary_search_by_key`. A future
/// build.rs change that breaks the sort invariant would silently make
/// some lookups return `None` for valid tokens — so guard the
/// invariant here.
#[test]
fn token_metadata_is_sorted_by_value() {
    let mut prev: Option<&str> = None;
    for entry in TOKEN_METADATA {
        assert!(
            !entry.value.is_empty(),
            "TOKEN_METADATA contains an entry with empty value"
        );
        if let Some(p) = prev {
            assert!(
                p < entry.value,
                "TOKEN_METADATA out of order: {p:?} appears before {:?}",
                entry.value
            );
        }
        prev = Some(entry.value);
    }
}

/// Every token entry references a CveFileMetadata that lives in
/// `CVE_FILES` — guards against codegen drift where an entry's
/// `cve_file` pointer goes stale.
#[test]
fn every_token_references_a_known_cve_file() {
    use std::collections::HashSet;
    let known: HashSet<&str> = CVE_FILES.iter().map(|f| f.const_name).collect();
    for entry in TOKEN_METADATA {
        assert!(
            known.contains(entry.cve_file.const_name),
            "TOKEN_METADATA entry {:?} references CVE file {:?} which is not in CVE_FILES",
            entry.value,
            entry.cve_file.const_name
        );
    }
}

/// Specific known-published tokens MUST be present in the table —
/// these are anchors for the rest of the tool. If `FOUO` falls out of
/// `CVEnumISMDissem.json` in a future ODNI release, the FOUO-handling
/// rule logic everywhere else will silently rot; we want a loud test
/// failure instead.
#[test]
fn well_known_tokens_resolve() {
    for (token, expected_file) in [
        ("FOUO", &CVE_DISSEM),
        ("RELIDO", &CVE_DISSEM),
        ("NF", &CVE_DISSEM),
        ("SI", &CVE_SCI_CONTROLS),
        ("TK", &CVE_SCI_CONTROLS),
        ("HCS", &CVE_SCI_CONTROLS),
    ] {
        let entry = lookup_token_metadata(token).unwrap_or_else(|| {
            panic!(
                "lookup_token_metadata({token:?}) returned None — token expected \
                 in {expected_file:?}",
                expected_file = expected_file.const_name
            )
        });
        assert_eq!(
            entry.cve_file.const_name,
            expected_file.const_name,
            "lookup_token_metadata({token:?}) returned an entry from {actual:?}, \
             expected {expected:?}",
            actual = entry.cve_file.const_name,
            expected = expected_file.const_name,
        );
    }
}

#[test]
fn lookup_unknown_token_returns_none() {
    assert!(
        lookup_token_metadata("DEFINITELY_NOT_A_REAL_MARKING_VALUE").is_none(),
        "lookup_token_metadata returned Some for a synthetic unknown value — \
         binary search must return None for missing tokens"
    );
    assert!(
        lookup_token_metadata("").is_none(),
        "lookup_token_metadata returned Some for the empty string"
    );
}
