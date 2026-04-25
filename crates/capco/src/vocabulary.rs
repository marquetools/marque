// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `impl Vocabulary<CapcoScheme> for CapcoScheme` — adapter that
//! composes `marque-ism`'s generated CVE tables into the
//! `marque-scheme::Vocabulary` trait surface (Phase 5 PR-2, T084).
//!
//! # Why this lives in `marque-capco` and not `marque-ism`
//!
//! Constitution VII pins the dependency graph: `marque-ism` does not
//! depend on `marque-scheme`, so the trait impl cannot live in
//! `marque-ism`. `marque-capco` consumes both crates and is the only
//! place the adapter can sit. The raw CVE-file and per-token
//! metadata records (`CveFileMetadata`, `TokenMetadataEntry`) are
//! emitted by `marque-ism::generated::vocabulary` (Phase 5 PR-1);
//! this module shapes them into `Authority`, `OwnerProducer`,
//! `PointOfContact`, and `TokenMetadataFull<TokenId>`.
//!
//! # Token coverage
//!
//! `CapcoScheme::Token = TokenId` — opaque numeric ids assigned
//! per-sentinel in `crate::scheme`. The active sentinel set today
//! is the small hand-curated list of TokenIds the catalog actually
//! references (~14 ids). Each is mapped to its canonical CVE value
//! by [`SENTINEL_TO_CANONICAL`]. Aggregate sentinels (`TOK_*` that
//! span multiple tokens — `TOK_US_CLASSIFIED`,
//! `TOK_NON_US_CLASSIFICATION`), trigraph sentinels (`TOK_USA` —
//! trigraphs come from XSD, not the JSON-derived `TOKEN_METADATA`),
//! and grammar-shape sentinels (`TOK_JOINT`, `TOK_FGI_MARKER`)
//! are deliberately absent: they have no single CVE value to attach
//! per-term metadata to. The trait's accessors panic if asked about
//! one of those, surfacing the misuse loudly rather than returning
//! a placeholder that would silently muddy the audit trail.
//!
//! Phase C extends both the sentinel set (auto-generated TokenIds
//! per CVE term) and this mapping. Today's hand-mapped subset is
//! enough to discharge T071–T077 against real ODNI data.
//!
//! # Static data lifetimes
//!
//! Every accessor returns `&'static` data. `Authority`,
//! `OwnerProducer`, and `PointOfContact` are derived once from the
//! `marque-ism` raw records via [`std::sync::LazyLock`] and
//! referenced by index. The composed `TokenMetadataFull<TokenId>`
//! records are similarly built once. Calls after the first do zero
//! heap allocation — exercised by the count-allocs test in
//! `crates/capco/tests/vocabulary_zero_alloc.rs` (gated on the
//! `count-allocs` feature).

use crate::scheme::{
    CapcoScheme, TOK_CNWDI, TOK_EXDIS, TOK_FRD, TOK_HCS, TOK_NODIS, TOK_NOFORN, TOK_RD,
    TOK_RESTRICTED, TOK_TFNI, TOK_UCNI,
};
use marque_ism::generated::migrations::find_migration;
use marque_ism::generated::vocabulary::{
    CveFileMetadata, TokenMetadataEntry, lookup_token_metadata,
};
use marque_ism::marking_forms::portion_to_banner;
use marque_scheme::{
    Authority, Deprecation, OwnerProducer, OwnerProducerKind, PointOfContact, TokenId,
    TokenMetadataFull, Vocabulary,
};
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Sentinel → canonical CVE value mapping.
// ---------------------------------------------------------------------------
//
// Each entry maps a CapcoScheme sentinel TokenId to the canonical
// CVE value that `marque_ism::generated::vocabulary::lookup_token_metadata`
// resolves. Aggregate / trigraph / grammar-shape sentinels are
// intentionally absent — they have no single CVE value; the trait
// accessors below panic on lookup for any TokenId not in this list.
//
// The chosen canonical strings line up with the values verified
// present in `TOKEN_METADATA` by PR-1's
// `crates/ism/tests/vocabulary_tables.rs::well_known_tokens_resolve`
// and direct inspection of the ODNI JSON sidecars.

const SENTINEL_TO_CANONICAL: &[(TokenId, &str)] = &[
    // Dissem — `NF` is the canonical portion form in
    // `CVEnumISMDissem.json` (the long form `NOT RELEASABLE TO
    // FOREIGN NATIONALS` is the term's description, not its value).
    (TOK_NOFORN, "NF"),
    // AEA — published in `CVEnumISMAtomicEnergyMarkings.json`.
    (TOK_RD, "RD"),
    (TOK_FRD, "FRD"),
    (TOK_TFNI, "TFNI"),
    // CNWDI is structurally a sub-marker of RD; the closest
    // CVE-value granularity ODNI publishes is the compound
    // `RD-CNWDI`. Sentinel resolution here matches the AEA JSON's
    // canonical token; CAPCO's rule layer continues to treat CNWDI
    // as a sub-flag on `AeaMarking::Rd { cnwdi: true }` (see
    // `satisfies_attrs` in `scheme.rs`).
    (TOK_CNWDI, "RD-CNWDI"),
    // UCNI defaults to the DOE form (`UCNI`); the DOD form
    // (`DCNI`) is its own marking with its own metadata. The
    // sentinel was never disambiguated, so this is a deliberate
    // best-fit choice — see the gap note in `satisfies_attrs`.
    (TOK_UCNI, "UCNI"),
    // SCI — published in `CVEnumISMSCIControls.json`.
    (TOK_HCS, "HCS"),
    // Classification — `R` = RESTRICTED, published in
    // `CVEnumISMClassificationAll.json`.
    (TOK_RESTRICTED, "R"),
    // Non-IC dissem — published in `CVEnumISMNonIC.json`.
    (TOK_NODIS, "ND"),
    (TOK_EXDIS, "XD"),
];

/// Resolve a sentinel TokenId to its canonical CVE value, or panic
/// with a clear message if the id is outside the supported set.
fn canonical_for(token: TokenId) -> &'static str {
    SENTINEL_TO_CANONICAL
        .iter()
        .find(|(id, _)| *id == token)
        .map(|(_, s)| *s)
        .unwrap_or_else(|| {
            panic!(
                "Vocabulary<CapcoScheme>: TokenId {token:?} has no canonical CVE \
                 value. Aggregate / trigraph / grammar-shape sentinels are not \
                 part of the per-term vocabulary. See \
                 `SENTINEL_TO_CANONICAL` in crates/capco/src/vocabulary.rs."
            )
        })
}

/// Resolve the per-token entry for a sentinel TokenId, panicking
/// loudly if `token` resolves to a canonical value missing from the
/// generated vocabulary table (which would indicate a build-time
/// drift between `crates/ism/build.rs` and the `SENTINEL_TO_CANONICAL`
/// curated set).
fn entry_for(token: TokenId) -> &'static TokenMetadataEntry {
    let canonical = canonical_for(token);
    lookup_token_metadata(canonical).unwrap_or_else(|| {
        panic!(
            "Vocabulary<CapcoScheme>: canonical {canonical:?} (from {token:?}) \
             missing from TOKEN_METADATA. The active ODNI schema package \
             ({schema}) no longer publishes this term — update \
             `SENTINEL_TO_CANONICAL` or bump `[package.metadata.marque] \
             ism-schema-version`.",
            schema = marque_ism::SCHEMA_VERSION,
        )
    })
}

// ---------------------------------------------------------------------------
// Per-CveFile composed records.
// ---------------------------------------------------------------------------
//
// Each `CveFileMetadata` from `marque-ism` produces one `Authority`,
// one `OwnerProducer`, and one `PointOfContact`. We build them once
// inside a `LazyLock<Vec<...>>` keyed by `cve_file.const_name` and
// hand out borrowed `&'static` references on every accessor call.

struct CveFileDerived {
    /// `cve_file.const_name` (stable identifier across builds).
    cve_const_name: &'static str,
    /// `Authority` carries `point_of_contact` as an embedded field;
    /// the per-token POC accessor returns `&authority.point_of_contact`
    /// rather than a duplicate copy. Single source of truth — drift
    /// between `scheme.authority(t).point_of_contact` and
    /// `scheme.point_of_contact(t)` is unrepresentable.
    authority: Authority,
    owner_producer: OwnerProducer,
}

/// Lazy table of derived per-CveFile records. Initialized on the
/// first vocabulary call; subsequent calls dereference into the
/// already-built `Vec`. The `LazyLock` itself is `static`, so all
/// references into the contained `Vec` are `&'static`.
static CVE_FILE_DERIVED: LazyLock<Vec<CveFileDerived>> = LazyLock::new(|| {
    use marque_ism::generated::vocabulary::CVE_FILES;
    CVE_FILES
        .iter()
        .map(|f| CveFileDerived {
            cve_const_name: f.const_name,
            authority: build_authority(f),
            owner_producer: build_owner_producer(f),
        })
        .collect()
});

fn derived_for_token(token: TokenId) -> &'static CveFileDerived {
    let entry = entry_for(token);
    let cve_const_name = entry.cve_file.const_name;
    CVE_FILE_DERIVED
        .iter()
        .find(|d| d.cve_const_name == cve_const_name)
        .unwrap_or_else(|| {
            // Unreachable in practice: `CVE_FILE_DERIVED` is built
            // from the same `CVE_FILES` slice that `entry.cve_file`
            // points into. Kept as a clear panic so a future
            // refactor that breaks the invariant fails fast.
            panic!(
                "Vocabulary<CapcoScheme>: CveFile {cve_const_name:?} missing \
                 from CVE_FILE_DERIVED — build.rs and the LazyLock init \
                 disagree on the CVE-file set."
            )
        })
}

fn build_authority(cve_file: &'static CveFileMetadata) -> Authority {
    Authority {
        // ODNI publishes a free-form `Source` field per CVE — the
        // human-readable name of the publishing register. Use that
        // verbatim; it is the most informative `source_name` the
        // raw record carries.
        source_name: cve_file.source,
        urn: cve_file.urn,
        schema_version: cve_file.schema_version,
        point_of_contact: build_point_of_contact(cve_file),
    }
}

fn build_owner_producer(cve_file: &'static CveFileMetadata) -> OwnerProducer {
    OwnerProducer {
        code: cve_file.owner_producer,
        // ODNI sidecars do not carry a free-form owner-name field;
        // `owner_producer` is the canonical short code. For CAPCO/
        // ISM tokens the code is always `"USA"`.
        name: cve_file.owner_producer,
        // Every Phase 5 PR-1 CVE file is published by ODNI on
        // behalf of the U.S. (verified by
        // `every_cve_file_carries_odni_provenance` in
        // `crates/ism/tests/vocabulary_tables.rs`). When future
        // FGI / NATO / Joint vocabularies land in their own
        // adapters they will surface the correct kind directly;
        // until then `National` is the only kind any active CAPCO
        // sentinel can produce.
        kind: OwnerProducerKind::National,
    }
}

fn build_point_of_contact(cve_file: &'static CveFileMetadata) -> PointOfContact {
    PointOfContact {
        name: cve_file.poc_name,
        email: cve_file.poc_email,
        // ODNI's `PointOfContact` JSON object does not include a
        // free-form organization field. The publishing
        // organization is implicit in the URN
        // (`urn:us:gov:ic:cvenum:...` ⇒ ODNI). Hardcode `"ODNI"`
        // for every active CAPCO sentinel; future non-CAPCO
        // adapters can extend the JSON shape and lift this.
        organization: "ODNI",
    }
}

// ---------------------------------------------------------------------------
// Per-token composed metadata records.
// ---------------------------------------------------------------------------
//
// `metadata()` returns `&'static TokenMetadataFull<TokenId>`. Build
// one per known sentinel inside another `LazyLock<Vec<...>>`.

struct TokenDerived {
    token: TokenId,
    metadata: TokenMetadataFull<TokenId>,
}

static TOKEN_DERIVED: LazyLock<Vec<TokenDerived>> = LazyLock::new(|| {
    SENTINEL_TO_CANONICAL
        .iter()
        .map(|(token, _)| TokenDerived {
            token: *token,
            metadata: build_metadata(*token),
        })
        .collect()
});

fn token_derived(token: TokenId) -> &'static TokenDerived {
    TOKEN_DERIVED
        .iter()
        .find(|d| d.token == token)
        .unwrap_or_else(|| {
            // Same panic as `canonical_for` — keep the message
            // consistent so the misuse surface looks identical no
            // matter which accessor was called.
            panic!(
                "Vocabulary<CapcoScheme>: TokenId {token:?} has no canonical \
                 CVE value. See `SENTINEL_TO_CANONICAL` in \
                 crates/capco/src/vocabulary.rs."
            )
        })
}

fn build_metadata(token: TokenId) -> TokenMetadataFull<TokenId> {
    // `derived_for_token` is safe to call here even though we are
    // mid-init of `TOKEN_DERIVED`: it consults `CVE_FILE_DERIVED`,
    // which has no dependency on `TOKEN_DERIVED`. The init order is
    // `TOKEN_DERIVED::new` → `build_metadata` → `derived_for_token`
    // → `CVE_FILE_DERIVED::new` (independent) → return. Reusing the
    // shared derived record makes `scheme.metadata(t).authority` and
    // `scheme.authority(t)` literally the same bytes — no risk of
    // drift between the per-field and aggregate accessors.
    let entry = entry_for(token);
    let derived = derived_for_token(token);
    let canonical = entry.value;
    TokenMetadataFull {
        canonical,
        urn: entry.cve_file.urn,
        schema_version: entry.cve_file.schema_version,
        // `Authority`, `OwnerProducer`, `PointOfContact` are `Copy`
        // (small structs of `&'static str`), so the field copy here
        // is cheap and doesn't fork ownership of the underlying
        // strings.
        authority: derived.authority,
        owner_producer: derived.owner_producer,
        point_of_contact: derived.authority.point_of_contact,
        deprecation: build_deprecation(canonical),
        portion_form: derive_portion_form(canonical),
        banner_form: derive_banner_form(canonical),
        banner_abbreviation: derive_banner_abbreviation(canonical),
    }
}

/// Map a canonical CVE value to its portion form. CVE values are
/// already the portion form for the markings the active sentinel set
/// touches (`NF`, `RD`, `FRD`, …); `marque_ism::marking_forms` only
/// holds entries where banner ≠ portion, so a missing entry means
/// the canonical IS the portion form. Returning the canonical
/// verbatim preserves that invariant.
fn derive_portion_form(canonical: &'static str) -> &'static str {
    canonical
}

/// Map a canonical CVE value to its banner form via
/// `portion_to_banner`. When no banner-distinct form exists the
/// banner equals the portion (and the canonical), per the CAPCO
/// Register convention (CAPCO-2016 §G.1 Table 4).
fn derive_banner_form(canonical: &'static str) -> &'static str {
    portion_to_banner(canonical).unwrap_or(canonical)
}

/// Banner abbreviation when the marking has a distinct one (i.e.,
/// when `banner != portion`); `None` otherwise. The sentinel set
/// today resolves to portion-form values; the abbreviation is the
/// banner form when it differs, mirroring `Vocabulary` semantics.
fn derive_banner_abbreviation(canonical: &'static str) -> Option<&'static str> {
    portion_to_banner(canonical).filter(|banner| *banner != canonical)
}

/// Deprecation lookup against `marque-ism::generated::migrations`.
/// `MIGRATIONS` is keyed by the deprecated marking string; if a
/// canonical CVE value appears as a `deprecated` entry, build a
/// `Deprecation { since, replacement }` from it. Otherwise return
/// `None` (active token — FR-017 silence).
fn build_deprecation(canonical: &'static str) -> Option<Deprecation<TokenId>> {
    let migration = find_migration(canonical)?;
    Some(Deprecation {
        // The migration table does not record a per-entry deprecation
        // version — `marque_ism::SCHEMA_VERSION` is the closest
        // ground truth (the schema package the deprecation was
        // observed in). Future migration-table extension can carry
        // an explicit `since` field; for now this matches what every
        // active migration entry was sourced against.
        since: marque_ism::SCHEMA_VERSION,
        // Map the replacement string back to a sentinel TokenId
        // when the replacement is itself in the active set.
        // Otherwise emit `None` per FR-017 — silence beats
        // pointing at an unrecognized successor.
        replacement: SENTINEL_TO_CANONICAL
            .iter()
            .find(|(_, s)| *s == migration.replacement)
            .map(|(id, _)| *id),
    })
}

/// Helper to expose a borrowed reference into a `LazyLock` field
/// without taking the `LazyLock` lock on every call. `LazyLock`
/// derefs to its target after the first init, and the target lives
/// for `'static` because the `LazyLock` itself is static.
fn authority_static(token: TokenId) -> &'static Authority {
    &derived_for_token(token).authority
}

fn owner_producer_static(token: TokenId) -> &'static OwnerProducer {
    &derived_for_token(token).owner_producer
}

fn point_of_contact_static(token: TokenId) -> &'static PointOfContact {
    // Single source of truth: route POC through the embedded
    // `Authority.point_of_contact` so `scheme.authority(t).point_of_contact`
    // and `scheme.point_of_contact(t)` always return identical data.
    &derived_for_token(token).authority.point_of_contact
}

// ---------------------------------------------------------------------------
// `impl Vocabulary<CapcoScheme>`
// ---------------------------------------------------------------------------

impl Vocabulary<CapcoScheme> for CapcoScheme {
    fn authority(&self, token: &TokenId) -> &'static Authority {
        authority_static(*token)
    }

    fn owner_producer(&self, token: &TokenId) -> &'static OwnerProducer {
        owner_producer_static(*token)
    }

    fn point_of_contact(&self, token: &TokenId) -> &'static PointOfContact {
        point_of_contact_static(*token)
    }

    fn deprecation(&self, token: &TokenId) -> Option<&'static Deprecation<TokenId>> {
        // Borrow into the cached `TokenMetadataFull` so the lookup
        // is `&'static` without a separate allocation site.
        token_derived(*token).metadata.deprecation.as_ref()
    }

    fn portion_form(&self, token: &TokenId) -> &'static str {
        token_derived(*token).metadata.portion_form
    }

    fn banner_form(&self, token: &TokenId) -> &'static str {
        token_derived(*token).metadata.banner_form
    }

    fn banner_abbreviation(&self, token: &TokenId) -> Option<&'static str> {
        token_derived(*token).metadata.banner_abbreviation
    }

    fn metadata(&self, token: &TokenId) -> &'static TokenMetadataFull<TokenId> {
        &token_derived(*token).metadata
    }
}
