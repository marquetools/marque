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
//! references (10 ids today; see [`SENTINEL_TO_CANONICAL`] below for
//! the authoritative count — the doc reflects the table). Each is
//! mapped to its canonical CVE value
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
    CAT_AEA, CAT_CLASSIFICATION, CAT_DISSEM, CAT_FGI_MARKER, CAT_JOINT_CLASSIFICATION,
    CAT_NON_US_CLASSIFICATION, CAT_REL_TO, CAT_SAR, CAT_SCI, CapcoScheme, TOK_CNWDI, TOK_EXDIS,
    TOK_FRD, TOK_HCS, TOK_NODIS, TOK_NOFORN, TOK_RD, TOK_RESTRICTED, TOK_TFNI, TOK_UCNI,
};
use marque_ism::generated::migrations::find_migration;
use marque_ism::generated::vocabulary::{
    CveFileMetadata, TokenMetadataEntry, lookup_token_metadata,
};
use marque_ism::marking_forms::{banner_to_portion, portion_to_banner};
use marque_scheme::{
    Authority, CategoryId, Deprecation, OwnerProducer, OwnerProducerKind, PointOfContact, TokenId,
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
///
/// **Phase C scaling note (L1 in `docs/reviews/phase5-review.md`).**
/// The current `.iter().find()` walks `SENTINEL_TO_CANONICAL` (10
/// entries today) on every accessor call. At this size the linear
/// scan is dominated by accessor-call overhead and is not a real
/// concern — Constitution I (perceptual instantaneity) is not
/// observably violated. Phase C extends the sentinel set to the full
/// CVE vocabulary (~200+ entries); at that point this lookup, plus
/// the parallel scans in [`derived_for_token`] and [`token_derived`],
/// must move to either a sorted `&[(TokenId, &str)]` with
/// `binary_search_by_key` or a build-time `phf::Map`. The migration
/// is Pre-Phase-C work tracked as a follow-up — landing it in this
/// review-fix PR would mix a behavioral change into what is
/// otherwise a docs + invariant-tightening PR.
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
        point_of_contact: build_capco_point_of_contact(cve_file),
    }
}

/// Map an ODNI owner-producer short code (`"USA"`, `"NATO"`, ...) to
/// the corresponding human-readable name.
///
/// ODNI's CVE sidecars publish only the short code (`ism:ownerProducer`
/// is a single bareword), but `OwnerProducer::name`'s field doc on
/// `marque-scheme::vocabulary` declares the field as a human-readable
/// name (e.g., `"United States of America"`). Returning the short code
/// would falsify that contract for every consumer that reads `name`.
///
/// This adapter is CAPCO-scoped (`crates/capco/src/vocabulary.rs`).
/// Future schemes (CUI, NATO, JOINT) live in their own crates and
/// will provide their own owner-producer translation.
///
/// The match is exhaustive against the codes that appear in
/// `crates/ism/schemas/ISM-v2022-DEC/CVE_ISMCAT/`. Adding a new code
/// to `CVEnumISMCATOwnerProducer.xml` triggers the `unknown` panic
/// arm here and forces the contributor to either extend the match or
/// document an intentional fallback. Constitution VIII fail-loud.
fn owner_producer_name(code: &'static str) -> &'static str {
    match code {
        "USA" => "United States of America",
        // Codes registered in CVEnumISMCATOwnerProducer.xml that
        // CAPCO does not (yet) emit. Listed here so a future schema
        // bump that adds them lands cleanly without a regression.
        "NATO" => "North Atlantic Treaty Organization",
        "FGI" => "Foreign Government Information",
        // Anything else is a CAPCO-vocabulary regression — either
        // ODNI added a new code (extend the match) or the build.rs
        // sidecar parsing emitted a corrupted value. Either way,
        // failing loud is the right call (Constitution VIII).
        unknown => panic!(
            "Vocabulary<CapcoScheme>: unknown owner-producer code {unknown:?}. \
             Extend `owner_producer_name` in crates/capco/src/vocabulary.rs \
             with the human-readable name from \
             crates/ism/schemas/ISM-v2022-DEC/CVE_ISMCAT/CVEnumISMCATOwnerProducer.xml."
        ),
    }
}

fn build_owner_producer(cve_file: &'static CveFileMetadata) -> OwnerProducer {
    OwnerProducer {
        code: cve_file.owner_producer,
        // Translated via the CAPCO-scoped lookup above. Field doc
        // on `marque-scheme::vocabulary::OwnerProducer::name`
        // requires a human-readable form, not the short code.
        name: owner_producer_name(cve_file.owner_producer),
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

/// CAPCO-specific point-of-contact builder. Hardcodes
/// `organization: "ODNI"` because every CAPCO/ISM sidecar's
/// `PointOfContact` JSON block lacks a free-form organization field
/// (the publishing organization is implicit in the
/// `urn:us:gov:ic:cvenum:...` URN). The function name carries the
/// `_capco_` infix specifically so a future FGI / NATO / JOINT
/// adapter cannot reuse this helper and silently misattribute its
/// own POCs to ODNI — Constitution VII isolates the per-domain
/// adapter, this name pins that isolation in code.
fn build_capco_point_of_contact(cve_file: &'static CveFileMetadata) -> PointOfContact {
    PointOfContact {
        name: cve_file.poc_name,
        email: cve_file.poc_email,
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
// shape_admits — open-vocab + closed-CVE category admission predicate.
// ---------------------------------------------------------------------------
//
// Design choice (T087): hand-written `match` over `CategoryId`, not a
// build-time-generated table.
//
// The `tasks.md` T087 entry suggested baking shape rules at build
// time per the Phase 5 metadata-surface mechanism. That route adds a
// new build.rs codepath for very little payoff at this scale: the
// total open-vocab category count is small (4 — SAR program-id,
// FGI/REL-TO trigraph, SCI compartment), each predicate is a few
// inline byte checks, and every predicate ties to a small set of
// CAPCO-2016 sections that change only on a planned source-revision
// migration (Constitution Principle VIII). A hand-written `match`
// keeps the rule and its citation visible together at the call site,
// which is what reviewers actually need to verify Principle VIII
// fidelity. If a second scheme (CUI, NATO, JOINT) lands and forces
// the same predicate machinery, that's the right time to lift the
// build-time generator.
//
// Each arm carries its own `// CAPCO-2016 §X.Y pNN` citation,
// verified directly against `crates/capco/docs/CAPCO-2016.md` at
// the line offsets recorded in this file's git blame. Citations
// MUST NOT be propagated from `crates/capco/CAPCO-CONTEXT.md`
// without re-verification (Principle VIII propagation discipline).

/// CVE-file membership classes the closed-CVE arms route through.
///
/// Each variant names exactly one (or a small union of) `cve_file.const_name`
/// strings emitted by `marque-ism/build.rs`. `lookup_token_metadata`
/// returns a `TokenMetadataEntry` whose `cve_file.const_name` is
/// matched against this allow-set: a token resolved by a CVE-file
/// outside the allow-set is *not* admitted to the category, even
/// though the surface bytes are technically known to ODNI.
///
/// Stays a small private enum so the dispatch table reads as data
/// rather than as a sequence of `||`-chained string comparisons.
enum CveFileSet {
    /// US classification levels — `CVE_CLASSIFICATION_ALL` covers
    /// all five US levels (TS/S/C/U/R). The `_US`-only variant in
    /// the schema is a subset; we accept the broader file because
    /// `R` (RESTRICTED) is shared by US and non-US grammars and
    /// the scheme distinguishes by category, not by token.
    UsClassification,
    /// Non-US protective markings file (NATO codes, etc.).
    NonUsControls,
    /// IC and non-IC dissemination control markings. The
    /// `CapcoScheme` has a single `CAT_DISSEM` category that
    /// covers both IC dissem (NF, OC, REL, ...) and non-IC
    /// dissem (LIMDIS/EXDIS/NODIS/SBU/...) — admission is a
    /// union of both CVE files.
    Dissem,
    /// AEA Information Markings (RD, FRD, TFNI, UCNI, DCNI,
    /// CNWDI, SIGMA compounds).
    AtomicEnergy,
}

impl CveFileSet {
    /// Returns true iff `entry.cve_file.const_name` is in this set.
    ///
    /// Equality on the static const-name string. Const-names are
    /// short (≤ 32 bytes typically) and `eq_ignore_ascii_case` is
    /// not used because the build emits canonical SCREAMING_SNAKE
    /// always.
    #[inline]
    fn contains(&self, entry: &TokenMetadataEntry) -> bool {
        let name = entry.cve_file.const_name;
        match self {
            Self::UsClassification => name == "CVE_CLASSIFICATION_ALL",
            Self::NonUsControls => name == "CVE_NON_US_CONTROLS",
            Self::Dissem => name == "CVE_DISSEM" || name == "CVE_NON_IC",
            Self::AtomicEnergy => name == "CVE_ATOMIC_ENERGY_MARKINGS",
        }
    }
}

/// Closed-CVE membership: bytes resolve to a known token in a CVE
/// file inside `set`.
///
/// Accepts either the canonical (portion) form or, for
/// classification levels, the banner form (e.g., `b"SECRET"` resolves
/// to `S` via `Classification::banner_str`). The token lookup table
/// in `marque-ism::generated::vocabulary` is keyed on the canonical
/// portion form only; banner-form admission is added explicitly here
/// because parser admission must accept both surfaces. The
/// `marking_forms::banner_to_portion` table covers dissem long-form
/// banners (e.g., `"NOFORN"` ↔ `"NF"`); classifications are handled
/// separately because they live in `Classification::banner_str`,
/// not `MARKING_FORMS`.
fn admits_closed_cve(bytes: &[u8], set: &CveFileSet) -> bool {
    // 1. Reject empty bytes — no token has zero length.
    if bytes.is_empty() {
        return false;
    }
    // 2. Reject non-ASCII fast — every CVE token in the active
    // schema is ASCII, and `lookup_token_metadata` expects
    // valid UTF-8. `from_utf8` is essentially free for ASCII.
    let Ok(s) = std::str::from_utf8(bytes) else {
        return false;
    };
    // 3. Direct canonical lookup. CVE tables key on the portion form
    // (e.g., "S", "NF", "RD"); a banner form like "SECRET" misses here
    // and falls through to step 4.
    if let Some(entry) = lookup_token_metadata(s) {
        if set.contains(entry) {
            return true;
        }
    }
    // 4. Banner-form fallback: classifications use full words in
    // banners (CAPCO-2016 §A.6 p15) and have no entry in
    // `MARKING_FORMS`, so consult `Classification::banner_str` first.
    // Then try the dissem-form table for cases like
    // "NOFORN" → "NF" (CAPCO-2016 §G.1 Table 4 pp 36-38).
    if matches!(set, CveFileSet::UsClassification)
        && classification_banner_to_portion(s).is_some()
    {
        return true;
    }
    if let Some(portion) = banner_to_portion(s) {
        if let Some(entry) = lookup_token_metadata(portion) {
            return set.contains(entry);
        }
    }
    false
}

/// Map a US classification banner form to its canonical portion
/// form, e.g., `"SECRET"` → `"S"`. Returns `None` for inputs that
/// are not a recognized US classification banner.
///
/// CAPCO-2016 §A.6 p15 fixes the banner spelling ("classification
/// marking capitalized and spelled out for US, non-US, and Joint
/// information; no abbreviations are authorized") and the per-level
/// banner forms ship in `Classification::banner_str` /
/// `Classification::portion_str`. This helper inverts that mapping
/// so closed-CVE admission can accept either surface form.
#[inline]
fn classification_banner_to_portion(s: &str) -> Option<&'static str> {
    match s {
        "TOP SECRET" => Some("TS"),
        "SECRET" => Some("S"),
        "CONFIDENTIAL" => Some("C"),
        "UNCLASSIFIED" => Some("U"),
        "RESTRICTED" => Some("R"),
        _ => None,
    }
}

/// FGI / REL-TO / JOINT country trigraph admission: delegates to
/// [`marque_ism::CountryCode::admits_fgi_trigraph`], which is the
/// single source of truth for the Annex B GENC trigraph shape
/// predicate (3 ASCII uppercase letters).
///
/// Keeping this function as a thin wrapper rather than calling
/// `CountryCode::admits_fgi_trigraph` inline at every match arm is a
/// readability convenience: `shape_trigraph(bytes)` reads cleanly in
/// the dispatch table below, and a future widening (e.g., admitting
/// the GENC numeric subset) is a single-line edit at the canonical
/// definition in `marque-ism` rather than a per-call-site change.
///
/// Tetragraph admission (4-letter codes for organizations /
/// coalitions like `NATO`, `ISAF`, `FVEY`) is intentionally NOT
/// folded in here because the existing parser routes tetragraphs
/// through a separate `vocab` lookup (see `marque_capco::vocab`); a
/// future PR can extend this predicate to admit `len == 4` against
/// the tetragraph table when the parser's tetragraph admission site
/// is migrated.
///
/// The strict parser at
/// `crates/core/src/parser.rs::parse_fgi_marker` calls into the same
/// `marque_ism::CountryCode::admits_fgi_trigraph` directly (it cannot
/// reach this private wrapper across the `marque-capco` boundary
/// without violating Constitution VII), so both surfaces are pinned
/// to the same canonical predicate by depending on the same exported
/// symbol.
#[inline]
fn shape_trigraph(bytes: &[u8]) -> bool {
    marque_ism::CountryCode::admits_fgi_trigraph(bytes)
}

/// SAR program identifier abbreviation: delegates to
/// [`marque_ism::SarProgram::admits_program_id_abbrev`], the single
/// source of truth for the 2-3 ASCII alphanumeric shape gate
/// (CAPCO-2016 §H.5 p99 + §H.5 p101).
///
/// Keeping this function as a thin wrapper rather than calling
/// `SarProgram::admits_program_id_abbrev` inline at the
/// `shape_admits` `CAT_SAR` arm is a readability convenience that
/// matches the [`shape_trigraph`] pattern: `shape_sar_program_id(bytes)`
/// reads cleanly in the dispatch table, and a future shape change
/// (e.g., narrowing to uppercase only or pinning a length cap) is
/// a single-line edit at the canonical definition in `marque-ism`
/// rather than a per-call-site change.
///
/// The strict parser at
/// `crates/core/src/parser.rs::parse_sar_program` calls into the
/// same `marque_ism::SarProgram::admits_program_id_abbrev` directly
/// (it cannot reach this private wrapper across the `marque-capco`
/// boundary without violating Constitution VII), so both surfaces
/// are pinned to the same canonical predicate by depending on the
/// same exported symbol — mirroring the FGI trigraph routing.
#[inline]
fn shape_sar_program_id(bytes: &[u8]) -> bool {
    marque_ism::SarProgram::admits_program_id_abbrev(bytes)
}

/// SCI compartment / sub-compartment shape: 2-3 ASCII alphanumeric
/// (length lower bound is 2). Per CAPCO-2016 §A.6 p15 ("SCI
/// markings are alphanumeric values") and §H.4 p76 (SI compartment
/// "2-3 alpha characters") + §H.4 p72 (RSV compartment "3
/// alphanumeric characters"). The generic SCI shape spans these
/// system-specific rules: SI is the most permissive on length (2),
/// RSV the most permissive on character class (alnum vs. alpha);
/// the union is `len in 2..=3 && all alnum`. Sub-compartment shapes
/// (≤6 alnum for HCS-P / TK families per §H.4 p68 / p87 / p89 etc.)
/// are NOT covered by this category-level predicate — they are
/// admitted by the structural SCI subparser in `marque-core`, which
/// has access to the parent control system and can apply the
/// system-specific length bound.
#[inline]
fn shape_sci_compartment(bytes: &[u8]) -> bool {
    matches!(bytes.len(), 2 | 3) && bytes.iter().all(u8::is_ascii_alphanumeric)
}

/// SCI admission with a try-CVE-then-shape policy.
///
/// CAPCO §H.4 publishes a closed set of pre-registered SCI control
/// systems and compounds (HCS, SI, TK, BUR, BUR-BLG, ...) plus
/// open-ended agency-specific compartments and sub-compartments.
/// Admission is the union: a token in the CVE table is admitted
/// regardless of shape (covers compounds with hyphens like `BUR-BLG`
/// where the shape predicate would otherwise reject), and a token
/// outside the table is admitted iff it satisfies
/// `shape_sci_compartment`.
///
/// The CVE-first ordering is deliberate: future SCI-strictness
/// changes (e.g., narrowing what counts as a valid compartment)
/// have a single audit point here.
fn admits_sci(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    // CVE membership first — covers SCI control systems and
    // pre-registered compounds whose shape would not satisfy the
    // generic compartment rule (hyphens in BUR-BLG, etc.).
    if let Ok(s) = std::str::from_utf8(bytes) {
        if let Some(entry) = lookup_token_metadata(s) {
            if entry.cve_file.const_name == "CVE_SCI_CONTROLS" {
                return true;
            }
        }
    }
    // Open-vocab fallback for agency-specific compartments.
    shape_sci_compartment(bytes)
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

    /// CAPCO admission predicate over `(CategoryId, &[u8])`.
    ///
    /// Closed-CVE arms route through `admits_closed_cve` against
    /// the category's allow-set of CVE files; open-vocab arms
    /// (SCI compartments, FGI / REL-TO trigraphs, SAR program ids,
    /// FGI marker) use the shape predicates declared above. Unknown
    /// categories return `false` (totality requirement).
    ///
    /// Each arm carries the verified CAPCO-2016 citation that
    /// authorizes its predicate. Per Constitution Principle VIII
    /// these citations have been verified against
    /// `crates/capco/docs/CAPCO-2016.md` directly — they were not
    /// propagated from `crates/capco/CAPCO-CONTEXT.md` without
    /// re-verification.
    #[inline]
    fn shape_admits(&self, category: CategoryId, bytes: &[u8]) -> bool {
        match category {
            // US classification: closed CVE set.
            // CAPCO-2016 §H.1 pp 47-54 (per-level templates) +
            // §A.6 p15 (banner spellings).
            CAT_CLASSIFICATION => admits_closed_cve(bytes, &CveFileSet::UsClassification),

            // Non-US classification: closed CVE set (NATO and
            // partner-national protective markings).
            // CAPCO-2016 §A.6 p17 (Figure 2 category lattice) +
            // Appendix A non-US protective markings.
            CAT_NON_US_CLASSIFICATION => admits_closed_cve(bytes, &CveFileSet::NonUsControls),

            // Joint classification: the JOINT marking is a structural
            // form (`//JOINT [class] [LIST]`) — its admissible
            // bytes at this category-level slot are the same US
            // classification levels (TS/S/C/U) with `RESTRICTED`
            // explicitly disallowed by §H.3 p55 ("TS/S/C/U only
            // (not RESTRICTED)"). We accept the same closed CVE
            // set as `CAT_CLASSIFICATION` here and rely on the
            // declarative E### rules (E014, joint/USA-first style)
            // to enforce the RESTRICTED exclusion at constraint
            // time — admission is shape, not constraint. This
            // mirrors the `cardinality: One` declaration in
            // `build_categories` for `CAT_JOINT_CLASSIFICATION`.
            // CAPCO-2016 §H.3 pp 55-59.
            CAT_JOINT_CLASSIFICATION => admits_closed_cve(bytes, &CveFileSet::UsClassification),

            // SCI compartments: CVE-then-shape.
            // CAPCO-2016 §A.6 p15 ("SCI markings are alphanumeric
            // values") + §H.4 p76 (SI compartment 2-3 alpha) and
            // §H.4 p72 (RSV compartment 3 alnum).
            CAT_SCI => admits_sci(bytes),

            // SAR program identifier abbreviation: 2-3 ASCII alnum.
            // CAPCO-2016 §H.5 p101 ("two or three-character
            // designator for the program") + §H.5 p99 ("SAR
            // program identifiers are alphanumeric values").
            CAT_SAR => shape_sar_program_id(bytes),

            // AEA Information Markings: closed CVE set (RD, FRD,
            // TFNI, UCNI, DCNI, CNWDI, RD-SG-#, FRD-SG-#).
            // CAPCO-2016 §H.6 pp 103-121 + §A.6 p17 (Figure 2
            // category lattice).
            CAT_AEA => admits_closed_cve(bytes, &CveFileSet::AtomicEnergy),

            // FGI marker (country trigraph in lawful position):
            // 3 ASCII uppercase letters per Annex B (GENC trigraph
            // country codes).
            // CAPCO-2016 §H.7 p122 ("Annex B trigraph country
            // codes") + §A.6 p16 ("Multiple FGI trigraph country
            // codes or tetragraph codes must be separated by a
            // single space").
            CAT_FGI_MARKER => shape_trigraph(bytes),

            // REL TO trigraph: same Annex B trigraph format as
            // FGI markers. Tetragraph admission (FVEY, NATO, ...)
            // routes through `marque_capco::vocab`'s tetragraph
            // table, not this predicate.
            // CAPCO-2016 §A.6 p17 ("'USA' trigraph code must be
            // listed first, followed by trigraph codes listed in
            // ascending alphabetic sort order") + §H.8 p150 (REL
            // TO entry).
            CAT_REL_TO => shape_trigraph(bytes),

            // Dissemination controls: closed CVE set spanning IC
            // (NF, OC, REL, RELIDO, FOUO, ...) and non-IC
            // (LIMDIS, EXDIS, NODIS, SBU, LES, ...) dissem files,
            // because the CapcoScheme has a single `CAT_DISSEM`
            // category covering both surfaces.
            // CAPCO-2016 §H.8 pp 131-168 (IC dissem) + §H.9 pp
            // 169-191 (non-IC dissem).
            CAT_DISSEM => admits_closed_cve(bytes, &CveFileSet::Dissem),

            // CAT_DECLASSIFY_ON and any unrecognized CategoryId
            // fall through to false. `CAT_DECLASSIFY_ON` carries a
            // datetime value, not a token surface, so admission
            // through this byte-class predicate is never
            // appropriate; routing it to `false` matches the
            // totality contract (unknown categories return false
            // rather than panic).
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod shape_admits_tests {
    use super::*;
    use crate::scheme::{
        CAT_AEA, CAT_CLASSIFICATION, CAT_DECLASSIFY_ON, CAT_DISSEM, CAT_FGI_MARKER,
        CAT_JOINT_CLASSIFICATION, CAT_NON_US_CLASSIFICATION, CAT_REL_TO, CAT_SAR, CAT_SCI,
    };
    use marque_scheme::Vocabulary;

    fn vocab() -> CapcoScheme {
        CapcoScheme::new()
    }

    // -------- FGI / REL TO trigraph (open vocab — 3 ASCII upper) ----

    #[test]
    fn fgi_trigraph_admits_three_uppercase_letters() {
        let v = vocab();
        assert!(v.shape_admits(CAT_FGI_MARKER, b"USA"));
        assert!(v.shape_admits(CAT_FGI_MARKER, b"GBR"));
        assert!(v.shape_admits(CAT_FGI_MARKER, b"JPN"));
    }

    #[test]
    fn fgi_trigraph_rejects_lowercase() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"usa"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"Usa"));
    }

    #[test]
    fn fgi_trigraph_rejects_wrong_length() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_FGI_MARKER, b""));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"US"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"USAA"));
    }

    #[test]
    fn fgi_trigraph_rejects_digits() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"123"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"US1"));
    }

    #[test]
    fn rel_to_uses_same_trigraph_shape_as_fgi() {
        let v = vocab();
        assert!(v.shape_admits(CAT_REL_TO, b"USA"));
        assert!(v.shape_admits(CAT_REL_TO, b"GBR"));
        assert!(!v.shape_admits(CAT_REL_TO, b"usa"));
        assert!(!v.shape_admits(CAT_REL_TO, b"123"));
        assert!(!v.shape_admits(CAT_REL_TO, b"US"));
    }

    // -------- SAR program identifier (open vocab — 2-3 ASCII alnum) -

    #[test]
    fn sar_admits_two_or_three_alnum() {
        let v = vocab();
        // §H.5 p101 examples: BP, SDA, XR (uppercase alpha)
        assert!(v.shape_admits(CAT_SAR, b"BP"));
        assert!(v.shape_admits(CAT_SAR, b"BPB"));
        assert!(v.shape_admits(CAT_SAR, b"XR"));
        // §H.5 p99 — alphanumeric values
        assert!(v.shape_admits(CAT_SAR, b"99"));
        assert!(v.shape_admits(CAT_SAR, b"A1"));
    }

    #[test]
    fn sar_rejects_wrong_length() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_SAR, b""));
        assert!(!v.shape_admits(CAT_SAR, b"B"));
        assert!(!v.shape_admits(CAT_SAR, b"BPBP"));
    }

    #[test]
    fn sar_accepts_lowercase_letting_style_rule_decide() {
        // §H.5 p99–101 prose says "alphanumeric values"; uppercase
        // is a Register convention. We accept lowercase here and
        // leave casing enforcement to a downstream style rule.
        let v = vocab();
        assert!(v.shape_admits(CAT_SAR, b"bp"));
    }

    #[test]
    fn sar_rejects_punctuation() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_SAR, b"B-"));
        assert!(!v.shape_admits(CAT_SAR, b"B P"));
    }

    // -------- SCI compartment (CVE-then-shape) ----------------------

    #[test]
    fn sci_admits_two_or_three_alnum_compartment() {
        let v = vocab();
        // 2-alpha compartment per §H.4 p76 (e.g., a custom SI compartment)
        assert!(v.shape_admits(CAT_SCI, b"BP"));
        // 3-alnum per §H.4 p72 (RSV) — covered by generic alnum path
        assert!(v.shape_admits(CAT_SCI, b"GBP"));
    }

    #[test]
    fn sci_rejects_single_char_compartment() {
        // §H.4 p76 — SI compartments are 2-3 alpha. A single
        // character `G` is the bare GAMMA marker (§H.4 p80)
        // structurally — admission as a category byte slot must
        // come through the structural subparser, not this
        // category-shape predicate. Still, `G` has no entry in the
        // closed CVE table for SCI under the current ODNI schema,
        // so the CVE-first path also fails. The test confirms
        // both paths reject `b"G"`.
        let v = vocab();
        assert!(!v.shape_admits(CAT_SCI, b"G"));
    }

    #[test]
    fn sci_admits_known_cve_compounds() {
        // CVE-first arm catches pre-registered compounds even when
        // their shape (hyphen) would otherwise reject. The active
        // ODNI v2022-DEC schema publishes a small set of
        // unclassified compounds (e.g., BUR-BLG); admission is
        // exactly CVE membership for these.
        let v = vocab();
        // BUR-BLG was confirmed in the generated TOKEN_METADATA
        // (cve_file: &CVE_SCI_CONTROLS) at this schema package.
        assert!(v.shape_admits(CAT_SCI, b"BUR-BLG"));
    }

    #[test]
    fn sci_rejects_empty() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_SCI, b""));
    }

    // -------- Classification (closed CVE — accepts banner OR portion form) -

    #[test]
    fn classification_admits_portion_form() {
        let v = vocab();
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"S"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"TS"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"C"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"U"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"R"));
    }

    #[test]
    fn classification_admits_banner_form() {
        let v = vocab();
        // CAPCO-2016 §A.6 p15 — banners spell out the classification.
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"SECRET"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"TOP SECRET"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"CONFIDENTIAL"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"UNCLASSIFIED"));
        assert!(v.shape_admits(CAT_CLASSIFICATION, b"RESTRICTED"));
    }

    #[test]
    fn classification_rejects_typos() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_CLASSIFICATION, b"SERCET"));
        assert!(!v.shape_admits(CAT_CLASSIFICATION, b"top secret")); // case-sensitive
        assert!(!v.shape_admits(CAT_CLASSIFICATION, b""));
    }

    #[test]
    fn classification_rejects_dissem_tokens() {
        // `NF` is in the dissem CVE file, not classification —
        // category isolation matters.
        let v = vocab();
        assert!(!v.shape_admits(CAT_CLASSIFICATION, b"NF"));
        assert!(!v.shape_admits(CAT_CLASSIFICATION, b"NOFORN"));
    }

    // -------- Dissem (closed CVE — IC + non-IC unioned) -------------

    #[test]
    fn dissem_admits_ic_dissem_tokens() {
        let v = vocab();
        // Portion forms.
        assert!(v.shape_admits(CAT_DISSEM, b"NF"));
        assert!(v.shape_admits(CAT_DISSEM, b"OC"));
        assert!(v.shape_admits(CAT_DISSEM, b"FOUO"));
        // Banner-form fallback (NOFORN ↔ NF in MARKING_FORMS).
        assert!(v.shape_admits(CAT_DISSEM, b"NOFORN"));
    }

    #[test]
    fn dissem_admits_non_ic_dissem_tokens() {
        let v = vocab();
        // CVE_NON_IC entries — CapcoScheme folds these into CAT_DISSEM.
        assert!(v.shape_admits(CAT_DISSEM, b"ND")); // NODIS
        assert!(v.shape_admits(CAT_DISSEM, b"XD")); // EXDIS
        assert!(v.shape_admits(CAT_DISSEM, b"DS")); // LIMDIS
    }

    #[test]
    fn dissem_rejects_classification_tokens() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_DISSEM, b"S"));
        assert!(!v.shape_admits(CAT_DISSEM, b"TS"));
    }

    // -------- AEA (closed CVE) --------------------------------------

    #[test]
    fn aea_admits_known_atomic_energy_tokens() {
        let v = vocab();
        assert!(v.shape_admits(CAT_AEA, b"RD"));
        assert!(v.shape_admits(CAT_AEA, b"FRD"));
        assert!(v.shape_admits(CAT_AEA, b"TFNI"));
        assert!(v.shape_admits(CAT_AEA, b"UCNI"));
        assert!(v.shape_admits(CAT_AEA, b"DCNI"));
    }

    #[test]
    fn aea_rejects_dissem_tokens() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_AEA, b"NF"));
        assert!(!v.shape_admits(CAT_AEA, b"S"));
    }

    // -------- JOINT classification ----------------------------------

    #[test]
    fn joint_classification_admits_us_levels() {
        // §H.3 p55 — JOINT pairs with US classification levels.
        // Admission is a SHAPE check (open/closed-CVE membership);
        // the RESTRICTED exclusion is enforced by E### constraints,
        // not at admission time.
        let v = vocab();
        assert!(v.shape_admits(CAT_JOINT_CLASSIFICATION, b"S"));
        assert!(v.shape_admits(CAT_JOINT_CLASSIFICATION, b"SECRET"));
        assert!(v.shape_admits(CAT_JOINT_CLASSIFICATION, b"TS"));
    }

    // -------- Non-US classification ---------------------------------

    #[test]
    fn non_us_classification_admits_nato_marks() {
        // CVE_NON_US_CONTROLS entries — NATO-* family.
        let v = vocab();
        assert!(v.shape_admits(CAT_NON_US_CLASSIFICATION, b"NATO-ATOMAL"));
        assert!(v.shape_admits(CAT_NON_US_CLASSIFICATION, b"NATO-BALK"));
    }

    #[test]
    fn non_us_classification_rejects_us_tokens() {
        let v = vocab();
        // `S` resolves to CVE_CLASSIFICATION_ALL, not NON_US_CONTROLS.
        assert!(!v.shape_admits(CAT_NON_US_CLASSIFICATION, b"S"));
    }

    // -------- Totality contract -------------------------------------

    #[test]
    fn declassify_on_returns_false_no_panic() {
        // Datetime-typed category should never admit through this
        // byte-class predicate; totality contract requires `false`,
        // not panic.
        let v = vocab();
        assert!(!v.shape_admits(CAT_DECLASSIFY_ON, b"2030-01-01"));
    }

    #[test]
    fn unknown_category_returns_false_no_panic() {
        let v = vocab();
        assert!(!v.shape_admits(CategoryId(9999), b"USA"));
        assert!(!v.shape_admits(CategoryId(9999), b""));
    }

    #[test]
    fn empty_bytes_reject_for_every_category() {
        let v = vocab();
        for cat in [
            CAT_CLASSIFICATION,
            CAT_NON_US_CLASSIFICATION,
            CAT_JOINT_CLASSIFICATION,
            CAT_SCI,
            CAT_SAR,
            CAT_AEA,
            CAT_FGI_MARKER,
            CAT_DISSEM,
            CAT_REL_TO,
            CAT_DECLASSIFY_ON,
        ] {
            assert!(
                !v.shape_admits(cat, b""),
                "empty bytes must reject for category {:?}",
                cat
            );
        }
    }

    #[test]
    fn non_utf8_bytes_reject_no_panic() {
        // Lone continuation byte is invalid UTF-8 — must not panic
        // and must not admit.
        let v = vocab();
        let invalid = [0x80_u8, 0x80, 0x80];
        assert!(!v.shape_admits(CAT_CLASSIFICATION, &invalid));
        assert!(!v.shape_admits(CAT_DISSEM, &invalid));
        assert!(!v.shape_admits(CAT_SCI, &invalid));
    }
}
