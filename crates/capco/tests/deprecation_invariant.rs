// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Deprecation::valid_from <= since` temporal invariant test, observed
//! at the `CapcoScheme.deprecation()` surface.
//!
//! ## Why this lives in `crates/capco/tests/` and not
//! `crates/ism/tests/`
//!
//! `Deprecation` is composed in
//! `crates/capco/src/vocabulary.rs::build_deprecation` (which reads
//! through `marque_ism::generated::migrations::find_migration` and
//! shapes the entry into `Deprecation { since, valid_from,
//! valid_until, replacement }`). The temporal invariant
//! `valid_from <= since` can only be observed once that composition
//! has happened — at the `Vocabulary<CapcoScheme>::deprecation()`
//! call site.
//!
//! Putting this test in `crates/ism/tests/` would require
//! `marque-capco` as a dev-dep of `marque-ism`, which creates a
//! cycle (`marque-capco` depends on `marque-ism`). The migration-
//! table-level parse-shape check stays in
//! `crates/ism/tests/migrations_invariant.rs`; this file owns the
//! temporal check that requires the composed `Deprecation`.
//!
//! ## Vacuous-at-PR-3d.3 note
//!
//! Every `Deprecation` today carries `valid_from: None` because no
//! per-term first-publish source data exists in the active ODNI
//! schema package (see `project_no_per_token_valid_from`). The
//! inner `if let Some(vf) = deprecation.valid_from` short-circuits
//! for every iteration today — the test is vacuous on data but the
//! loop iterates every active sentinel and the assertion is
//! reachable. When a future ODNI revision populates `valid_from`,
//! this test catches any temporal-ordering violation at the
//! `Deprecation` shape.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{
    TOK_CNWDI, TOK_EXDIS, TOK_FRD, TOK_HCS, TOK_NODIS, TOK_NOFORN, TOK_RD, TOK_RESTRICTED,
    TOK_TFNI, TOK_UCNI,
};
use marque_scheme::{TokenId, Vocabulary};

/// Every active sentinel TokenId. Kept hand-rolled (not iterated
/// through a private `SENTINEL_TO_CANONICAL` view) so the test
/// surfaces the full set explicitly — adding a sentinel without
/// adding its token here is caught by
/// `crates/capco/tests/vocabulary_forms.rs::expected_forms_covers_full_active_sentinel_set`,
/// which pins the same coupling against `active_sentinel_count()`.
const ACTIVE_SENTINELS: &[TokenId] = &[
    TOK_NOFORN,
    TOK_RD,
    TOK_FRD,
    TOK_TFNI,
    TOK_CNWDI,
    TOK_UCNI,
    TOK_HCS,
    TOK_RESTRICTED,
    TOK_NODIS,
    TOK_EXDIS,
];

/// Parse an ODNI schema-version label `"ISM-v{YYYY}-{MMM}"` into a
/// `(year, month_as_u8)` tuple suitable for temporal comparison.
///
/// Returns `None` for any input that does not match the expected
/// shape — the caller treats `None` as "cannot compare temporally"
/// and short-circuits the invariant rather than panicking.
///
/// Duplicated from `crates/ism/tests/migrations_invariant.rs` (same
/// shape, same semantics) because the integration-test boundary is
/// per-file in Rust; sharing helpers across `crates/*/tests/`
/// requires a `tests/common/mod.rs` setup that would be larger than
/// the helper itself. When/if a public `SchemaVersionId` type lands,
/// both copies retire together.
fn parse_schema_version(s: &str) -> Option<(u16, u8)> {
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
fn deprecation_valid_from_lte_since_for_every_sentinel() {
    let scheme = CapcoScheme::new();

    for token in ACTIVE_SENTINELS {
        let Some(deprecation) = scheme.deprecation(token) else {
            // Active (non-deprecated) sentinel — nothing to check
            // here; the silent `None` is correct.
            continue;
        };

        // Temporal invariant: a token cannot be deprecated before it
        // was published. Today every `valid_from` is
        // `None`, so this inner branch is unreachable on real data;
        // the assertion stays in place so a future ODNI revision
        // that populates `valid_from` lands a temporal-ordering
        // violation loudly here rather than silently propagating an
        // invalid `Deprecation` into the engine.
        if let Some(valid_from_label) = deprecation.valid_from {
            let valid_from = parse_schema_version(valid_from_label).unwrap_or_else(|| {
                panic!(
                    "Deprecation for {token:?} carries valid_from={valid_from_label:?} \
                     that does not parse as an ISM schema version. Fix the migration \
                     table in crates/ism/build.rs.",
                )
            });
            let since = parse_schema_version(deprecation.since).unwrap_or_else(|| {
                panic!(
                    "Deprecation for {token:?} carries since={:?} that does not parse \
                     as an ISM schema version. Fix the migration table in \
                     crates/ism/build.rs.",
                    deprecation.since,
                )
            });

            assert!(
                valid_from <= since,
                "Deprecation for {token:?} violates the temporal invariant: \
                 valid_from={valid_from_label:?} > since={:?}. A token cannot be \
                 deprecated before it was published.",
                deprecation.since,
            );
        }
    }
}
