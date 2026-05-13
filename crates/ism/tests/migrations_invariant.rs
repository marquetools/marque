// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3d (FR-054) — `MigrationEntry::valid_until` parse-shape
//! invariant test.
//!
//! Asserts that every generated migration entry's `valid_until`
//! field, when populated, parses cleanly as an ISM schema-version
//! label (`"ISM-vYYYY-MMM"`). This is a malformed-data guard at the
//! migration-table layer; the temporal-ordering invariant
//! (`valid_from <= since`) that PR 3d's spec calls for lives on
//! `Deprecation` after `marque-capco::vocabulary::build_deprecation`
//! composes the migration entry into the `Deprecation` struct, and
//! is pinned by
//! `crates/capco/tests/deprecation_invariant.rs::deprecation_valid_from_lte_since_for_every_sentinel`.
//! The split between this file and that one is dictated by the crate
//! dependency graph (see header comment for the original PR 3d.2
//! rationale).
//!
//! ## Schema-version comparison
//!
//! ODNI schema versions are strings like `"ISM-v2022-DEC"`, with
//! a 3-letter month abbreviation. Lexical `<=` on raw strings is
//! unsafe because `"DEC" < "MAR"` lexically does not match
//! temporal order (December comes before March of the following
//! year in calendar terms, but `"DEC" < "MAR"` is also `true`
//! lexically — and the reverse direction `"FEB" < "JAN"` is false
//! when temporally `FEB > JAN` of the same year). Comparison must
//! decompose into `(year, month_as_u8)` and compare component-wise.
//!
//! The `parse_schema_version` helper inside this file is the
//! local-scoped parser — there is no public `SchemaVersionId` type
//! in marque yet, and PR 3d is not introducing one. When/if such
//! a type lands (likely as part of a future "evaluate at schema
//! version" rule context flag — see FR-054's "Used by" note), this
//! local helper retires in favor of the typed parser.
//!
//! ## Scope note
//!
//! `MigrationEntry` itself carries `valid_until` only (added in PR 3d
//! Commit 2); the matching `valid_from` lives on `Deprecation<Token>`
//! after `marque-capco::vocabulary::build_deprecation` composes the
//! migration entry into a `Deprecation`. This file therefore only
//! validates the malformed-data guard on `MigrationEntry::valid_until`
//! (its presence-or-absence and its parse shape). The temporal-
//! ordering invariant `Deprecation::valid_from <= since` is pinned in
//! `crates/capco/tests/deprecation_invariant.rs::deprecation_valid_from_lte_since_for_every_sentinel`.
//!
//! Every migration entry today has `valid_until: None` because no
//! per-term last-valid source data exists in the active ODNI schema
//! package (see `project_no_per_token_valid_from`). The parse check
//! below therefore short-circuits for every current entry. The test
//! exists to pin the invariant so a future ODNI revision that
//! populates `valid_until` cannot land a malformed schema-version
//! string silently.

// `MigrationEntry::valid_until` is a `pub` field on the generated
// migration struct (added by PR 3d Commit 2 to
// `crates/ism/build.rs`). The `MIGRATIONS` static and
// `find_migration` lookup are already exposed through the
// `generated::migrations` module.
use marque_ism::generated::migrations::MIGRATIONS;

/// Parse an ODNI schema-version label `"ISM-v{YYYY}-{MMM}"` into a
/// `(year, month_as_u8)` tuple suitable for temporal comparison.
///
/// Returns `None` for any input that does not match the expected
/// shape — the caller treats `None` as "cannot compare temporally"
/// and short-circuits the invariant rather than panicking.
///
/// Month abbreviations are the standard 3-letter uppercase English
/// month codes ODNI uses on its publication index. The match is
/// exhaustive over the 12 calendar months.
fn parse_schema_version(s: &str) -> Option<(u16, u8)> {
    // Expected shape: "ISM-vYYYY-MMM"
    let rest = s.strip_prefix("ISM-v")?;
    let (year_str, month_str) = rest.split_once('-')?;
    let year: u16 = year_str.parse().ok()?;
    let month: u8 = match month_str {
        "JAN" => 1,
        "FEB" => 2,
        "MAR" => 3,
        "APR" => 4,
        "MAY" => 5,
        "JUN" => 6,
        "JUL" => 7,
        "AUG" => 8,
        "SEP" => 9,
        "OCT" => 10,
        "NOV" => 11,
        "DEC" => 12,
        _ => return None,
    };
    Some((year, month))
}

#[test]
fn parse_schema_version_round_trips() {
    // Sanity check on the local parser — pinning the active ODNI
    // schema version's parse confirms the helper isn't silently
    // mis-reading future entries.
    assert_eq!(parse_schema_version("ISM-v2022-DEC"), Some((2022, 12)));
    assert_eq!(parse_schema_version("ISM-v2025-JAN"), Some((2025, 1)));
    assert_eq!(parse_schema_version("not-a-schema"), None);
    assert_eq!(parse_schema_version("ISM-v2022-XXX"), None);
}

#[test]
fn migration_entries_valid_until_parses_as_schema_version() {
    // PR 3d Commit 2 adds `valid_until` to MigrationEntry; the
    // `marque_scheme::Deprecation` consumer ties this to
    // `Deprecation.valid_until`.
    //
    // Migration entries today have no `valid_from` field at the
    // `MigrationEntry` shape (that lives on `Deprecation` after
    // `build_deprecation` composes it). The invariant pinned at
    // this layer is therefore parse-shape only: `valid_until`,
    // when populated, must parse cleanly as an `ISM-vYYYY-MMM`
    // schema-version label. Every current entry has
    // `valid_until: None`, so the loop is vacuous today; once
    // entries gain cutoff data, the parse check catches malformed
    // schema-version strings before they propagate into composed
    // `Deprecation` records.
    //
    // The temporal-ordering invariant (`valid_from <= since`) the
    // function name previously claimed to enforce moved to
    // `crates/capco/tests/deprecation_invariant.rs::deprecation_valid_from_lte_since_for_every_sentinel`
    // — the only place `Deprecation`s are observable (after
    // `build_deprecation` composes them in the CapcoScheme
    // adapter).
    for entry in MIGRATIONS {
        if let Some(valid_until) = entry.valid_until {
            assert!(
                parse_schema_version(valid_until).is_some(),
                "Migration entry {:?}: valid_until={:?} does not parse as \
                 an ISM schema version. Update the migration table in \
                 crates/ism/build.rs to use the `ISM-vYYYY-MMM` form.",
                entry.deprecated,
                valid_until,
            );
        }
    }
}
