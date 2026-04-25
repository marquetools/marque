// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Regression guards for the deprecated-marking `MIGRATIONS` table
//! (Phase 5 PR-2, task T075).
//!
//! The Phase E recursive-lattice plan (see `crates/ism/build.rs`
//! doc-comment under `MIGRATIONS`) explicitly removed the legacy
//! `FOUO → CUI` migration entry. FOUO is still a valid CAPCO dissem
//! control per `CVEnumISMDissem.json` and remains active in the
//! `TOKEN_METADATA` table. CUI is a separate marking system under
//! NARA jurisdiction; any "suggest CUI on non-IC documents" behavior
//! belongs in a future CUI adapter, not as a blanket CAPCO
//! migration.
//!
//! These tests exist to prevent the entry from being silently
//! reintroduced in a future build.rs change. A migration map is the
//! kind of table where a stray copy/paste — "well, the old code had
//! it, so I added it back" — would re-violate FR-020 without
//! tripping any other test.

use marque_ism::generated::{
    migrations::{MIGRATIONS, find_migration},
    vocabulary::lookup_token_metadata,
};

#[test]
fn fouo_is_not_in_migration_table() {
    assert!(
        find_migration("FOUO").is_none(),
        "FOUO appears in the MIGRATIONS table — the FOUO→CUI entry was \
         intentionally removed per Phase E (FR-020). Re-introducing it \
         would emit fix proposals against an active dissem control. See \
         the doc-comment on `MIGRATIONS` in crates/ism/build.rs.",
    );

    for entry in MIGRATIONS {
        // `eq_ignore_ascii_case` avoids the per-iteration `String`
        // allocation that `to_ascii_uppercase()` would cost.
        assert!(
            !entry.replacement.eq_ignore_ascii_case("CUI"),
            "MIGRATIONS contains a CUI replacement (deprecated={:?}). \
             CUI is not a CAPCO marking — it belongs to a future CUI \
             adapter, not the CAPCO migration table.",
            entry.deprecated,
        );
    }
}

#[test]
fn fouo_remains_in_active_token_metadata() {
    let entry = lookup_token_metadata("FOUO").expect(
        "FOUO is missing from TOKEN_METADATA — \
         CVEnumISMDissem.json should still publish it as an active dissem \
         control (FR-020). If ODNI has retired FOUO in a newer schema \
         package, bump `[package.metadata.marque] ism-schema-version` and \
         update this test.",
    );
    assert_eq!(entry.value, "FOUO");
    assert_eq!(
        entry.cve_file.const_name, "CVE_DISSEM",
        "FOUO is published by an unexpected CVE file: {:?}",
        entry.cve_file.const_name,
    );
}
