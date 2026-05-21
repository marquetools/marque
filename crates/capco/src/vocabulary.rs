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
//! references (18 ids post-#660; see [`SENTINEL_TO_CANONICAL`] below
//! for the authoritative count — the doc reflects the table). Each
//! is mapped to its canonical CVE value
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
//! NATO program markings (ATOMAL/BALK/BOHEMIA, issue #660) sit on a
//! divergence: ODNI publishes them with a `NATO-` prefix in
//! `CVE_NON_US_CONTROLS` (`"NATO-ATOMAL"`, etc.) while CAPCO §G.1
//! Table 4 p37 registers the bare display form (`"ATOMAL"`). The
//! `SENTINEL_TO_CANONICAL` entries use the prefixed CVE canonical so
//! `entry_for` resolves cleanly via `lookup_token_metadata`; the
//! bare display form is re-projected at `forms()` time by
//! [`nato_program_form_set`].
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
    CAT_NON_US_CLASSIFICATION, CAT_REL_TO, CAT_SAR, CAT_SCI, CapcoScheme, FDR_DOMINATORS,
    TOK_ATOMAL, TOK_BALK, TOK_BOHEMIA, TOK_CNWDI, TOK_DCNI, TOK_EXDIS, TOK_FISA, TOK_FRD, TOK_HCS,
    TOK_HCS_O, TOK_HCS_P, TOK_NNPI, TOK_NODIS, TOK_NOFORN, TOK_ORCON_USGOV, TOK_RD, TOK_RESTRICTED,
    TOK_SI_G, TOK_SSI, TOK_TFNI, TOK_TK_BLFH, TOK_TK_IDIT, TOK_TK_KAND, TOK_UCNI,
    capco_token_category,
};
use marque_ism::Classification;
use marque_ism::generated::migrations::find_migration;
use marque_ism::generated::vocabulary::{
    CveFileMetadata, TokenMetadataEntry, lookup_token_metadata,
};
use marque_ism::marking_forms::{MARKING_FORMS, MarkingForm, banner_to_portion};
use marque_scheme::TokenRef;
use marque_scheme::{
    Authority, CategoryId, Deprecation, FormKind, FormSet, OwnerProducer, OwnerProducerKind,
    PointOfContact, TokenId, TokenMetadataFull, Vocabulary,
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

/// Number of active sentinel TokenIds the CAPCO Vocabulary impl covers.
///
/// Exposed for integration tests (e.g.,
/// `crates/capco/tests/vocabulary_forms.rs`) so the expected-forms
/// table can pin its row count against the authoritative active-
/// sentinel-set size — adding a sentinel without extending the test
/// table is caught loudly. Returns the length of
/// [`SENTINEL_TO_CANONICAL`].
pub fn active_sentinel_count() -> usize {
    SENTINEL_TO_CANONICAL.len()
}

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
    // UCNI is the DOE form (`UCNI`); the DOD form (`DCNI`) is its
    // own sentinel + canonical entry below. Issue #407 split the
    // pair so `forms()` can resolve each variant to the correct
    // CVE entry.
    (TOK_UCNI, "UCNI"),
    // SCI — published in `CVEnumISMSCIControls.json`.
    (TOK_HCS, "HCS"),
    // Issue #524 (Phase 1): per-compartment SCI compounds published
    // in `CVEnumISMSCIControls.json` (verified against the generated
    // TOKEN_METADATA at build time — entries `HCS-O`, `HCS-P`, `SI-G`,
    // `TK-BLFH`, `TK-IDIT`, `TK-KAND`). Each compound is a registered
    // ODNI CVE value with its own §H.4 marking template (see the
    // `TOK_*` const doc-comments in `crates/capco/src/scheme/mod.rs`).
    (TOK_HCS_O, "HCS-O"),
    (TOK_HCS_P, "HCS-P"),
    (TOK_SI_G, "SI-G"),
    (TOK_TK_BLFH, "TK-BLFH"),
    (TOK_TK_IDIT, "TK-IDIT"),
    (TOK_TK_KAND, "TK-KAND"),
    // Classification — `R` = RESTRICTED, published in
    // `CVEnumISMClassificationAll.json`.
    (TOK_RESTRICTED, "R"),
    // Non-IC dissem — published in `CVEnumISMNonIC.json`.
    (TOK_NODIS, "ND"),
    (TOK_EXDIS, "XD"),
    // Issue #407: vocabulary surface for the five sentinels whose
    // CVE canonicals were previously unreachable through
    // `forms()`. Each entry's canonical is the §G.1 Table 4
    // (p36) authorized portion mark; the matching `MARKING_FORMS`
    // row supplies the banner title + abbreviation.
    //
    // CAT_DISSEM (IC dissem):
    (TOK_ORCON_USGOV, "OC-USGOV"), // ORCON-USGOV — §H.8 p139
    (TOK_FISA, "FISA"),            // FISA — §H.8 p161
    // CAT_NON_IC_DISSEM (non-IC dissem):
    (TOK_SSI, "SSI"), // SSI — §H.9 p189
    // NNPI has no CAPCO-2016 §-citation (closes issue #407;
    // see `crates/capco/src/scheme/mod.rs` TOK_NNPI comment for
    // the in-tree authority).
    (TOK_NNPI, "NNPI"),
    // CAT_AEA:
    (TOK_DCNI, "DCNI"), // DOD UCNI — §H.6 p116
    // Issue #660: NATO program markings — §G.1 Table 4 p37 registers
    // ATOMAL/BALK/BOHEMIA with no banner abbreviation; portion/banner
    // title columns both carry the bare display form. ODNI publishes
    // the CVE canonicals as `NATO-ATOMAL`/`NATO-BALK`/`NATO-BOHEMIA`
    // in `CVE_NON_US_CONTROLS` (verified at build time against the
    // generated `TOKEN_METADATA` — entries
    // `value: "NATO-ATOMAL"|"NATO-BALK"|"NATO-BOHEMIA"`,
    // `cve_file: &CVE_NON_US_CONTROLS`). The bare display form is
    // re-projected at `forms()` time via [`nato_program_form_set`]
    // (mirrors [`classification_form_set`]'s role for `R`/`U`/`C`/`S`/`TS`).
    //
    // - ATOMAL: AEA axis, §H.7 p122 worked example
    //   `SECRET//RD/ATOMAL//FGI NATO//NOFORN`. Resolved via
    //   `capco_token_category(TOK_ATOMAL) == Some(CAT_AEA)`.
    // - BALK / BOHEMIA: SCI axis, §G.2 p40 (Table 5 ARH registration) +
    //   §H.7 p127 worked example
    //   `(//CTS//BOHEMIA//REL TO USA, NATO)`. Resolved via
    //   `capco_token_category(TOK_BALK|TOK_BOHEMIA) == Some(CAT_SCI)`.
    //
    // Pre-#660 these sentinels were registered in `scheme/mod.rs` and
    // consumed by E066 (`LegacyNatoCompoundRemarkRule`) but missing
    // from this table — any future caller invoking `canonical_for` /
    // `entry_for` / `forms` / `metadata` on the three sentinels would
    // panic in `canonical_for` ("TokenId has no canonical CVE value").
    (TOK_ATOMAL, "NATO-ATOMAL"),
    (TOK_BALK, "NATO-BALK"),
    (TOK_BOHEMIA, "NATO-BOHEMIA"),
];

/// Resolve a sentinel TokenId to its canonical CVE value, or panic
/// with a clear message if the id is outside the supported set.
///
/// **Phase C scaling note (L1 in `docs/reviews/phase5-review.md`).**
/// The current `.iter().find()` walks `SENTINEL_TO_CANONICAL` (18
/// entries post-#660) on every accessor call. At this size the linear
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
/// `ism_ismcat::package_root() / Schema/ISMCAT/CVEGenerated/`.
/// Adding a new code to `CVEnumISMCATOwnerProducer.xml` triggers the
/// `unknown` panic arm here and forces the contributor to either
/// extend the match or document an intentional fallback. Constitution
/// VIII fail-loud.
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
             with the human-readable name from the `ism-ismcat` crate's \
             Schema/ISMCAT/CVEGenerated/CVEnumISMCATOwnerProducer.xml."
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
    /// PR 3d (FR-053) — aggregated `FormSet` for `token`. Borrowed
    /// `&'static` via the surrounding `LazyLock`; the per-form
    /// default-method projections in `Vocabulary<S>` read this.
    form_set: FormSet,
}

static TOKEN_DERIVED: LazyLock<Vec<TokenDerived>> = LazyLock::new(|| {
    SENTINEL_TO_CANONICAL
        .iter()
        .map(|(token, canonical)| TokenDerived {
            token: *token,
            metadata: build_metadata(*token),
            form_set: build_form_set(canonical),
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
    // PR 3d (FR-053): metadata reads through `build_form_set` so the
    // single-source-of-truth invariant from PR review #2 extends to
    // the form fields. `metadata.banner_form` matches
    // `scheme.banner_form(t)` (which projects `forms(t)`) by
    // construction — no risk of drift between the per-field
    // accessors and the metadata struct.
    let form_set = build_form_set(canonical);
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
        portion_form: form_set.portion,
        banner_form: form_set
            .banner_abbreviation
            .unwrap_or(form_set.banner_title),
        banner_abbreviation: form_set.banner_abbreviation,
    }
}

/// Synthesize a US classification token's `FormSet` from
/// [`Classification::banner_str`] / [`Classification::portion_str`].
///
/// US classifications do not appear in `MARKING_FORMS` (they follow a
/// different structural pattern — banners use full words with no
/// abbreviation). Per CAPCO-2016 §G.1 Table 4 (no abbreviation column
/// for any classification row), `banner_abbreviation = None` and
/// `banner_title = banner_str()` (e.g., "SECRET", "TOP SECRET").
///
/// **`R` (RESTRICTED) is deliberately omitted from this arm.** The
/// pre-3d code returned `"R"` (not `"RESTRICTED"`) for the banner
/// form because no `MARKING_FORMS` row exists for `R` and the
/// canonical-collapse fallback applied. The byte-identity invariant
/// for PR 3d requires preserving that pre-3d behavior; the test
/// fixture in
/// `crates/capco/tests/vocabulary.rs::banner_abbreviation_none_for_same_form`
/// pins `TOK_RESTRICTED` as a same-form token. Routing `R` through
/// `Classification::banner_str` would surface `"RESTRICTED"` instead
/// and break the test. The pre-3d treatment is a marker convention
/// glitch (CAPCO §A.6 p15 says banners spell out classifications) but
/// fixing it is out of scope for PR 3d — file a follow-on if needed.
///
/// The TS / S / C / U arms are wired forward-looking: today's active
/// sentinel set does not include them, so the function returns `None`
/// for every active token. When Phase C expands the sentinel set to
/// the full closed-CVE vocabulary, those arms light up.
fn classification_form_set(canonical: &'static str) -> Option<FormSet> {
    let class = match canonical {
        "TS" => Classification::TopSecret,
        "S" => Classification::Secret,
        "C" => Classification::Confidential,
        "U" => Classification::Unclassified,
        // `R` intentionally not handled here — see doc-comment.
        _ => return None,
    };
    Some(FormSet {
        portion: class.portion_str(),
        banner_title: class.banner_str(),
        banner_abbreviation: None,
        recognized_aliases: &[],
    })
}

/// Synthesize a NATO program token's `FormSet` from the §G.1 Table 4
/// p37 registration.
///
/// ODNI publishes ATOMAL/BALK/BOHEMIA in `CVE_NON_US_CONTROLS` with a
/// `NATO-` prefix (`NATO-ATOMAL`, `NATO-BALK`, `NATO-BOHEMIA`) to
/// namespace them against other NATO controls in the same CVE file.
/// CAPCO §G.1 Table 4 p37 registers them bare:
///
/// ```text
/// | ATOMAL  | None | ATOMAL  |
/// | BALK    | None | BALK    |
/// | BOHEMIA | None | BOHEMIA |
/// ```
///
/// (column 1 = banner title, column 2 = banner abbreviation, column 3
/// = portion mark). `None` in col 2 means no distinct abbreviation —
/// `banner_abbreviation` is `None`. Same-form means portion =
/// banner_title.
///
/// This helper mirrors [`classification_form_set`]'s role for US
/// classifications: a token where the ODNI CVE canonical and the
/// user-visible §G.1 Table 4 form diverge gets a hand-built `FormSet`
/// rather than routing through the `MARKING_FORMS` lookup (which is
/// keyed by the bare display form and would miss the `NATO-`-prefixed
/// canonical, falling through to the canonical-collapse arm and
/// emitting `portion="NATO-ATOMAL"` — wrong per §G.1 Table 4 p37).
///
/// `MARKING_FORMS` already carries bare rows
/// (`portion=banner=title="ATOMAL"` etc.) at lines 283-303 of
/// `crates/ism/src/marking_forms.rs` per §G.1 Table 4 p37; this helper
/// is the bridge that lets a `NATO-`-prefixed canonical reach the
/// authorized bare display form without depending on those rows
/// (decoupling: a future MARKING_FORMS reorganization that removes the
/// bare same-form rows must not silently break the FormSet here).
///
/// Authority: CAPCO-2016 §G.1 Table 4 p37 (registration with no banner
/// abbreviation, bare display form across all three columns).
fn nato_program_form_set(canonical: &'static str) -> Option<FormSet> {
    let bare = match canonical {
        "NATO-ATOMAL" => "ATOMAL",
        "NATO-BALK" => "BALK",
        "NATO-BOHEMIA" => "BOHEMIA",
        _ => return None,
    };
    Some(FormSet {
        portion: bare,
        banner_title: bare,
        banner_abbreviation: None,
        recognized_aliases: &[],
    })
}

/// Build the aggregated `FormSet` for a sentinel token's canonical
/// CVE value (PR 3d, FR-053).
///
/// Field mapping from `MARKING_FORMS` per D1 / FR-053:
/// - `FormSet.portion` = `MarkingForm.portion` (CAPCO §G.1 Table 4
///   col 3);
/// - `FormSet.banner_title` = `MarkingForm.title` (col 1, "Authorized
///   Banner Line Marking Title" — the long descriptive form);
/// - `FormSet.banner_abbreviation` = `Some(banner)` when
///   `MarkingForm.banner != MarkingForm.title`, else `None` (col 2,
///   "Authorized Banner Line Abbreviation" — `None` represents the
///   row's empty col 2).
///
/// **Byte-identity preservation note.** Pre-3d code computed
/// `banner_abbreviation` via `banner != portion` (a different
/// predicate). PR 3d adopts the corrected D1 semantic per the spec.
/// The default `banner_form` projection
/// (`banner_abbreviation.unwrap_or(banner_title)`) still returns the
/// pre-3d output byte-for-byte:
/// - For NOFORN (portion=NF, banner=NOFORN, title=NOT RELEASABLE...):
///   `Some("NOFORN").unwrap_or(...)` = "NOFORN" — same as pre-3d.
/// - For RD (portion=RD, banner=RD, title=RESTRICTED DATA):
///   `Some("RD").unwrap_or("RESTRICTED DATA")` = "RD" — same as pre-3d.
/// - For HCS (no MARKING_FORMS row, canonical-collapse arm):
///   `None.unwrap_or("HCS")` = "HCS" — same as pre-3d.
///
/// `banner_abbreviation` itself diverges from pre-3d only for tokens
/// with `banner == portion` but a distinct CAPCO title (RD / FRD /
/// TFNI today). The corrected semantic surfaces the short banner
/// abbreviation that CAPCO §G.1 Table 4 col 2 lists for those rows;
/// the pre-3d test
/// `crates/capco/tests/vocabulary.rs::banner_abbreviation_none_for_same_form`
/// is updated in PR 3d Commit 1 to reflect the corrected semantic.
///
/// US classifications are not in `MARKING_FORMS` and route through
/// [`classification_form_set`] instead. Tokens with no `MARKING_FORMS`
/// row and not a classification (HCS, RD-CNWDI, R today) fall back
/// to the "canonical IS the portion/banner/title" shape (verbatim
/// canonical for all three forms, with `banner_abbreviation = None`).
///
/// `recognized_aliases` payload for `build_form_set` — surfaces the
/// ODNI `<Description>` text when it diverges from the CAPCO
/// `MarkingForm.title` (PR 3d.3 wire-through of FR-053).
///
/// ## Why per-canonical static slices and not a runtime read of
/// `MarkingForm.description_title`
///
/// `FormSet.recognized_aliases` is `&'static [(FormKind, &'static str)]`
/// — a slice, not a single `&str`. `MarkingForm.description_title` is
/// `Option<&'static str>`; lifting that into a `&'static
/// [(FormKind, &'static str)]` requires either a const slice per
/// canonical (this approach) or a `LazyLock<Vec<...>>` table side-
/// channel. The static-slice-per-row approach preserves the SC-008
/// zero-runtime-allocation invariant and keeps the data co-located with
/// its citation; the `LazyLock` route adds an allocation site (the
/// `Vec` of per-row slices) for no payoff at the active sentinel scale
/// (only 1 active sentinel — UCNI — is divergent today).
///
/// Each `const` carries the ODNI `<Description>` text verbatim;
/// updates flow through `MARKING_FORMS.description_title` and this
/// table in lockstep. The `recognized_aliases_pin_ism_description_divergences`
/// test in `crates/capco/tests/vocabulary_forms.rs` enforces the
/// round-trip.
///
/// ## Active sentinel coverage
///
/// `SENTINEL_TO_CANONICAL` (18 entries post-#660) intersects the
/// divergent `MARKING_FORMS` rows at six canonicals: `"UCNI"` (DOE),
/// `"DCNI"` (DOD UCNI), `"OC-USGOV"` (ORCON-USGOV), `"FISA"`, `"SSI"`,
/// `"NNPI"`. `TOK_CNWDI` maps to canonical `"RD-CNWDI"`, not the bare
/// `"CNWDI"` that the MARKING_FORMS row keys on — so its description
/// title divergence is captured on the row but unreachable through
/// `forms(TOK_CNWDI)`. The remaining divergent canonical (SI-EU /
/// SI-NK) has no active sentinel today (the bare-form CNWDI / NK / EU
/// rewriters in E067 operate at the rule layer, not the vocabulary
/// surface); the divergence remains visible via direct iteration of
/// `MARKING_FORMS.description_title` (exercised by
/// `crates/ism/tests/description_title_divergence.rs`).
///
/// The NATO program sentinels (TOK_ATOMAL/TOK_BALK/TOK_BOHEMIA,
/// #660) route through [`nato_program_form_set`] before the
/// `MARKING_FORMS` scan — their CVE canonicals (`"NATO-ATOMAL"`,
/// etc.) have no MARKING_FORMS row by design; the bare display form
/// is hand-built per §G.1 Table 4 p37.
const ALIASES_UCNI: &[(FormKind, &str)] = &[(
    FormKind::IsmDescriptionTitle,
    "DoE CONTROLLED NUCLEAR INFORMATION",
)];

const ALIASES_DCNI: &[(FormKind, &str)] = &[(
    FormKind::IsmDescriptionTitle,
    "DoD CONTROLLED NUCLEAR INFORMATION",
)];

const ALIASES_OC_USGOV: &[(FormKind, &str)] = &[(
    FormKind::IsmDescriptionTitle,
    "ORIGINATOR CONTROLLED US GOVERNMENT",
)];

const ALIASES_FISA: &[(FormKind, &str)] = &[(
    FormKind::IsmDescriptionTitle,
    "Foreign Intelligence Surveillance Act. Related to unclassified \
     and declassified information that is collected from \
     unconsenting individuals under the authority of the Foreign \
     Intelligence Surveillance Act (FISA).",
)];

const ALIASES_SSI: &[(FormKind, &str)] = &[(
    FormKind::IsmDescriptionTitle,
    "Sensitive Security Information. As defined in 49 C.F.R. Part \
     15.5, Sensitive Security Information is information obtained \
     or developed in the conduct of security activities, including \
     research and development, the disclosure of which DOT has \
     determined would constitute an unwarranted invasion of \
     privacy, reveal trade secrets or privileged or confidential \
     information, or be detrimental to transportation safety. As \
     defined in 49 C.F.R. Part 1520.5, Sensitive Security \
     Information is information obtained or developed in the \
     conduct of security activities, including research and \
     development, the disclosure of which DHS/TSA has determined \
     would, among other things, be detrimental to the security \
     of transportation.",
)];

const ALIASES_NNPI: &[(FormKind, &str)] = &[(
    FormKind::IsmDescriptionTitle,
    "Naval Nuclear Propulsion Information. Related to the safety \
     of reactors and associated naval nuclear propulsion plants, \
     and control of radiation and radioactivity associated with \
     naval nuclear propulsion activities, including prescribing \
     and enforcing standards and regulations for these areas as \
     they affect the environment and the safety and health of \
     workers, operators, and the general public.",
)];

/// Lookup the `recognized_aliases` static slice for a `MarkingForm`'s
/// canonical. Returns `&[]` for non-divergent rows.
///
/// Match arms cover canonicals that are BOTH active sentinels AND
/// carry a populated `MarkingForm.description_title`. Issue #407
/// added five sentinels (DCNI / OC-USGOV / FISA / SSI / NNPI), each
/// of which has a divergent `description_title` per `MARKING_FORMS`.
/// The remaining divergent canonicals (SI-EU / SI-NK / CNWDI) have
/// no active sentinel — they fall through to `&[]` here; the
/// divergence remains visible via direct iteration of
/// `MARKING_FORMS.description_title` (exercised by
/// `crates/ism/tests/description_title_divergence.rs`).
fn recognized_aliases_for_canonical(
    canonical: &'static str,
) -> &'static [(FormKind, &'static str)] {
    match canonical {
        "UCNI" => ALIASES_UCNI,
        "DCNI" => ALIASES_DCNI,
        "OC-USGOV" => ALIASES_OC_USGOV,
        "FISA" => ALIASES_FISA,
        "SSI" => ALIASES_SSI,
        "NNPI" => ALIASES_NNPI,
        _ => &[],
    }
}

/// Build the `FormSet` for a sentinel token's canonical CVE value
/// (PR 3d, FR-053).
///
/// `recognized_aliases` is populated for canonicals where the
/// matching `MARKING_FORMS` row's `description_title` field is
/// `Some(ism_title)` AND the canonical is an active sentinel — see
/// [`recognized_aliases_for_canonical`] for the coverage notes.
fn build_form_set(canonical: &'static str) -> FormSet {
    // Dispatch order:
    // 1. `classification_form_set` — US classifications (TS/S/C/U).
    //    LOAD-BEARING ORDER: these canonicals (`"S"`, `"TS"`, etc.)
    //    would collide with `MARKING_FORMS` rows if the scan ran
    //    first; the classification arm must precede the scan to
    //    return the `Classification::banner_str` / `portion_str`
    //    projection rather than the row's `(title, banner, portion)`.
    // 2. `nato_program_form_set` — issue #660 ATOMAL/BALK/BOHEMIA
    //    where the CVE canonical (`NATO-`-prefixed in
    //    `CVE_NON_US_CONTROLS`) diverges from the §G.1 Table 4 p37
    //    bare display form. This arm is NOT order-sensitive against
    //    arm 3 — the `NATO-` prefix guarantees no `MARKING_FORMS`
    //    row matches (rows carry the bare `"ATOMAL"` etc. — see
    //    `crates/ism/src/marking_forms.rs`); without this arm the
    //    canonical-collapse fallback (arm 4) would emit
    //    `portion="NATO-ATOMAL"` etc., which is wrong per §G.1
    //    Table 4 p37.
    // 3. `MARKING_FORMS` scan — every other CVE canonical whose
    //    portion or banner column matches `canonical` literally.
    // 4. Canonical-collapse fallback — `portion = banner = title =
    //    canonical`, `banner_abbreviation = None` (e.g., HCS,
    //    RD-CNWDI, R; see `same_form_sentinels` in
    //    `crates/capco/tests/vocabulary.rs`).
    if let Some(class_form_set) = classification_form_set(canonical) {
        return class_form_set;
    }
    if let Some(nato_form_set) = nato_program_form_set(canonical) {
        return nato_form_set;
    }

    // Look up the MARKING_FORMS row keyed off either the portion or
    // banner column — rows are keyed on whichever form is the
    // canonical CVE value for the token.
    let row: Option<&'static MarkingForm> = MARKING_FORMS
        .iter()
        .find(|f| f.portion == canonical || f.banner == canonical);

    let recognized_aliases = recognized_aliases_for_canonical(canonical);

    match row {
        Some(f) => {
            // D1 / FR-053: `banner != title` is the "distinct
            // abbreviation" predicate that CAPCO §G.1 Table 4 col 2
            // emptiness encodes.
            let banner_abbreviation = if f.banner != f.title {
                Some(f.banner)
            } else {
                None
            };
            FormSet {
                portion: f.portion,
                banner_title: f.title,
                banner_abbreviation,
                recognized_aliases,
            }
        }
        // No MARKING_FORMS row — every form collapses to the
        // canonical (preserves byte-identity with the pre-3d
        // derive_portion_form / derive_banner_form codepaths for
        // unrouted tokens like HCS, RD-CNWDI).
        None => FormSet {
            portion: canonical,
            banner_title: canonical,
            banner_abbreviation: None,
            recognized_aliases,
        },
    }
}

/// Deprecation lookup against `marque-ism::generated::migrations`.
/// `MIGRATIONS` is keyed by the deprecated marking string; if a
/// canonical CVE value appears as a `deprecated` entry, build a
/// `Deprecation { since, valid_from, valid_until, replacement }` from
/// it. Otherwise return `None` (active token — FR-017 silence).
///
/// PR 3d (FR-054) added `valid_from` / `valid_until`. ODNI XSD
/// annotations and JSON sidecars carry version metadata at the file
/// level, not per-token, so today every entry leaves `valid_from` as
/// `None`. `valid_until` is plumbed from the migration table; every
/// current migration entry has `valid_until: None` because none of
/// them carry an explicit cutoff schema version yet. The data
/// pathway is complete so a future ODNI revision can populate
/// either field without a trait change.
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
        // PR 3d (FR-054): no source data for per-token first-publish
        // version. Defaults to `None` per `project_no_per_token_valid_from`.
        valid_from: None,
        // PR 3d (FR-054): read through from the generated migration
        // entry. Today every entry has `valid_until: None` because no
        // per-term cutoff data is published in the active ODNI source
        // package; a future ODNI revision can wire the field without
        // a trait or signature change.
        valid_until: migration.valid_until,
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
    if matches!(set, CveFileSet::UsClassification) && classification_banner_to_portion(s).is_some()
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

/// FGI / REL TO / JOINT country list-token admission: delegates to
/// [`marque_ism::CountryCode::admits_country_token`], the single
/// source of truth for the FGI/REL TO list shape gate — 2, 3, or 4
/// ASCII uppercase letters (registered 2-letter exception code,
/// Annex B trigraph, or Annex A tetragraph respectively).
///
/// Per CAPCO-2016 §H.7 p122, a lawful FGI list mixes trigraphs and
/// tetragraphs (canonical example: `SECRET//FGI GBR JPN
/// NATO//REL TO USA, GBR, JPN, NATO`). §H.8 p150 admits the same
/// shape on REL TO. The 2-letter exception (notably `EU`, shipped
/// in ODNI ISMCAT `CVEnumISMCATRelTo`) admits at the same gate.
/// Restricting this predicate to trigraphs alone would silently
/// reject lawful inputs; that bug was the PR #311 review finding.
/// See [`marque_ism::CountryCode::admits_country_token`] for the
/// authority chain.
///
/// Registry membership (whether `NATO` is a registered Annex A
/// tetragraph) is intentionally out of scope for the shape gate —
/// it's the rule-layer's responsibility (rules walk
/// `marque_ism::TETRAGRAPH_MEMBERS` / `marque_ism::TRIGRAPHS`). The
/// shape gate's job is to refuse to mint malformed AST nodes; the
/// rule layer's job is to flag in-shape-but-out-of-registry tokens
/// with a diagnostic. Same separation as `admits_fgi_trigraph` ↔
/// Annex B membership.
///
/// Note: per issue #280, `crates/core/src/parser.rs::parse_fgi_marker`
/// was narrowed to `marque_ism::CountryCode::admits_fgi_ownership_token`
/// (a stricter predicate that rejects distribution-list tetragraphs
/// like `FVEY`/`CFIUS`/`ACGU`/`ISAF` while admitting `NATO` + 2-3
/// byte `CountryCode`-admissible tokens including the `EU` exception
/// code). This vocabulary surface intentionally remains on
/// `admits_country_token` for round-trip compatibility with the
/// broader FGI-marker vocabulary contract; see the `parse_fgi_marker`
/// doc-comment for the full divergence rationale. Future changes that
/// harmonize `CAT_FGI_MARKER`'s vocabulary admission to match the
/// parser MUST treat that as a deliberate contract narrowing, not a
/// cleanup. The `fgi_country_token_admits_tetragraphs` test pins this
/// divergence.
#[inline]
fn shape_country_token(bytes: &[u8]) -> bool {
    marque_ism::CountryCode::admits_country_token(bytes)
}

/// REL TO list-token admission: union of the shape gate
/// ([`shape_country_token`]) AND registered-code membership in
/// [`marque_ism::TRIGRAPHS`].
///
/// REL TO §H.8 admits a strictly broader surface than FGI §H.7. The
/// strict parser at `crates/core/src/parser.rs::parse_rel_to_with_spans`
/// uses `tokens.is_trigraph(...)` (binary search over
/// `TRIGRAPHS`), which accepts:
/// - 2-byte registered exception codes (e.g., `EU`).
/// - 3-byte Annex B trigraphs.
/// - 4-byte Annex A tetragraphs (e.g., `NATO`, `FVEY`, `ISAF`).
/// - **15-byte registered long codes** — `AUSTRALIA_GROUP` is the
///   canonical case. These have non-uniform shape (length + interior
///   underscore) and are admitted by registry membership only;
///   `admits_country_token` cannot encode them as a byte-class
///   predicate.
///
/// A pure-shape gate (`admits_country_token`, 2/3/4 ASCII upper)
/// would silently reject `AUSTRALIA_GROUP` even though the strict
/// parser admits it — that's the asymmetry the PR #311 round-3
/// review caught. This helper restores parity by ORing the two
/// admission paths:
/// - Shape-admissible (2/3/4 ASCII upper) → admit. Catches the
///   common case alloc-free.
/// - Registry membership (any length, exact match in `TRIGRAPHS`)
///   → admit. Catches the long-code exceptions and is also a
///   stricter check than the shape gate for caller convenience
///   (a code that fails shape but is in the registry is still
///   admissible).
///
/// CAT_FGI_MARKER deliberately does NOT widen this way — CAPCO §H.7
/// p123 admits trigraphs and tetragraphs only; AUSTRALIA_GROUP-class
/// codes are not lawful FGI material.
///
/// Authority: CAPCO-2016 §H.8 p150 (REL TO list grammar) +
/// `marque_ism::generated::values::TRIGRAPHS` (the canonical REL TO
/// admission registry, generated from ODNI ISMCAT
/// `CVEnumISMCATRelTo` plus org extensions, sorted for binary
/// search).
#[inline]
fn shape_or_registered_rel_to_token(bytes: &[u8]) -> bool {
    if shape_country_token(bytes) {
        return true;
    }
    // Long-code path: convert to `&str` (registry is `&[&str]`) and
    // binary-search. Non-UTF-8 bytes can't be in the registry by
    // construction (registry entries are ASCII), so failed
    // conversion is a guaranteed `false`.
    let Ok(s) = std::str::from_utf8(bytes) else {
        return false;
    };
    marque_ism::TRIGRAPHS.binary_search(&s).is_ok()
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

    /// PR 3d (FR-053): the aggregated `FormSet` for `token` is built
    /// once per sentinel inside `TOKEN_DERIVED` and borrowed
    /// `&'static` on every call. The per-form trait default methods
    /// (`portion_form` / `banner_form` / `banner_abbreviation`)
    /// project this struct — no override needed on this impl.
    fn forms(&self, token: &TokenId) -> &'static marque_scheme::FormSet {
        &token_derived(*token).form_set
    }

    fn metadata(&self, token: &TokenId) -> &'static TokenMetadataFull<TokenId> {
        &token_derived(*token).metadata
    }

    /// CAPCO FD&R-membership predicate.
    ///
    /// Iterates over [`crate::scheme::FDR_DOMINATORS`] — the slice
    /// that defines the FD&R set itself per CAPCO-2016 §B.3.a p19
    /// ("NOFORN, REL TO, RELIDO, or DISPLAY ONLY"; §B.3 Table 2 pp
    /// 21-22 is a scenario-summary table, not the definition) —
    /// so this method and the slice stay in lock-step against a
    /// single source-of-truth.
    ///
    /// **Adding a `TokenRef::Token` entry to `FDR_DOMINATORS`
    /// automatically updates this method** — the iteration walks
    /// every entry and matches `Token` arms by `TokenId` equality.
    /// **Adding a `TokenRef::AnyInCategory(CAT_X)` entry only
    /// admits tokens that `capco_token_category` routes to
    /// `Some(CAT_X)`** — the override receives a single `TokenId`
    /// and must route through `capco_token_category` to hit the
    /// category arm. If a new `AnyInCategory(CAT_NEW)` lands and
    /// `capco_token_category` has no arm for `CAT_NEW`'s
    /// participating tokens, those tokens silently fall through
    /// to `false`. See the maintenance contract on
    /// `FDR_DOMINATORS` in `crates/capco/src/scheme.rs` for the
    /// dual-update rule.
    ///
    /// # Why `FDR_DOMINATORS` and not `is_fdr_dominator`?
    ///
    /// The neighboring [`crate::scheme::is_fdr_dominator`] function
    /// is a related-but-distinct predicate: it answers "is `t` an
    /// FD&R dominator *over RELIDO*" for the
    /// [`marque_scheme::constraint::Constraint::ConflictsWithFamily`]
    /// dispatch on the RELIDO conflict catalog. That predicate
    /// deliberately excludes RELIDO itself (RELIDO-vs-RELIDO is a
    /// tautology) — so delegating through it would under-fire on
    /// `is_fdr_dissem(TOK_RELIDO)`. RELIDO is unambiguously an FD&R
    /// member per §B.3.a p19, so this method iterates over the
    /// full `FDR_DOMINATORS` slice directly.
    ///
    /// # Open-vocab routing
    ///
    /// `FDR_DOMINATORS` carries
    /// [`TokenRef::AnyInCategory`]`(CAT_REL_TO)` so every REL TO
    /// country code (open-vocab, not addressable by `TokenId`)
    /// participates in the FD&R family for the
    /// `ConflictsWithFamily` dispatch. This method routes a single
    /// [`TokenId`] through [`crate::scheme::capco_token_category`]
    /// to hit the same arm. The country-trigraph sentinels
    /// (`TOK_USA`, `TOK_REL_TO`) already resolve to
    /// `Some(CAT_REL_TO)` via `capco_token_category`, so they
    /// admit here.
    ///
    /// # Maintenance contract
    ///
    /// If a future revision adds an `AnyInCategory(CAT_X)` entry
    /// for a category whose tokens are NOT routed through
    /// `capco_token_category` (e.g., a hypothetical aggregate
    /// sentinel that isn't a real per-token category), this method
    /// silently under-fires for those tokens. The bidirectional
    /// value-pin test in `mod fdr_dissem_pin` (this file) catches
    /// the case by walking `FDR_DOMINATORS` and asserting every
    /// `AnyInCategory` entry is reachable via `capco_token_category`.
    ///
    /// # Performance
    ///
    /// `FDR_DOMINATORS` is a `&'static [TokenRef]` with 5 entries
    /// today (NOFORN, RELIDO, DISPLAY ONLY, any REL TO, EYES). The
    /// `iter().any(...)` walk is bounded by that constant and
    /// branches on each entry — no allocation, no hash lookup. The
    /// [`marque_scheme::builtins::SupersessionSet`] dissem join
    /// will call this method on the hot path (PR 4b); the constant
    /// bound is what makes the delegation viable instead of a
    /// hand-rolled `matches!`.
    ///
    /// Citations:
    /// - CAPCO-2016 §B.3.a p19 — canonical FD&R-set enumeration
    ///   ("NOFORN, REL TO, RELIDO, or DISPLAY ONLY"). §B.3 Table 2 pp
    ///   21-22 is a scenario-summary table, not the definition.
    /// - CAPCO-2016 §H.8 p157 — EYES deprecation marker; recognized
    ///   for legacy-input compatibility.
    #[inline]
    fn is_fdr_dissem(&self, token: &TokenId) -> bool {
        FDR_DOMINATORS.iter().any(|entry| match entry {
            TokenRef::Token(id) => *id == *token,
            TokenRef::AnyInCategory(cat) => capco_token_category(*token) == Some(*cat),
        })
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

            // FGI marker list-token: 3 ASCII upper (Annex B
            // trigraph) OR 4 ASCII upper (Annex A tetragraph).
            // CAPCO-2016 §H.7 p122 admits both shapes (ownership
            // semantic + `FGI [LIST]` Register form); the
            // canonical multi-country example
            // (`SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN,
            // NATO`) lives at §A.6 p16. The canonical-order
            // invariant (trigraphs alphabetic, then tetragraphs
            // alphabetic) is rule-layer, not admission. Registry
            // membership (whether the tetragraph appears in Annex
            // A / `TETRAGRAPH_MEMBERS`) is also rule-layer.
            //
            // Note: vocabulary surface admits the broader CountryCode shape here; the
            // strict parser narrows to admits_fgi_ownership_token (NATO + 2-3 byte
            // CountryCode only). See parse_fgi_marker for the FR-015 divergence
            // rationale (issue #280).
            CAT_FGI_MARKER => shape_country_token(bytes),

            // REL TO list-token: shape (2/3/4 ASCII upper) OR
            // registered long-code membership in
            // `marque_ism::TRIGRAPHS`. The strict parser at
            // `parse_rel_to_with_spans` uses the same registry,
            // so the API surface and the parser admit the same
            // set — including 15-byte registered codes like
            // `AUSTRALIA_GROUP` that cannot be encoded as a
            // byte-class shape predicate. CAT_FGI_MARKER does NOT
            // widen this way — §H.7 p122 admits trigraphs and
            // tetragraphs only.
            // CAPCO-2016 §H.8 p150 + §A.6 p17 ("'USA' trigraph
            // code must be listed first, followed by trigraph
            // codes listed in ascending alphabetic sort order,
            // then tetragraph codes ...") + `TRIGRAPHS` registry
            // generated from ODNI ISMCAT `CVEnumISMCATRelTo`.
            CAT_REL_TO => shape_or_registered_rel_to_token(bytes),

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

    /// Qualified `"category.token"` form for audit-record `token_id`
    /// emission per `contracts/audit-record.md` `marque-1.0` shape.
    ///
    /// Composes two existing accessors:
    /// - [`Vocabulary::metadata`] → [`TokenMetadataFull::canonical`]
    ///   for the token's canonical name (e.g. `"SECRET"`).
    /// - [`capco_token_category`] + a category-name lookup over
    ///   [`MarkingScheme::categories`] for the category name (e.g.
    ///   `"classification"`).
    ///
    /// Returns `Cow::Owned(String)` because the compose step needs to
    /// concatenate; a future per-token build-time table can return
    /// `Cow::Borrowed(&'static str)` once the constituent strings are
    /// pre-baked in the same row. Audit emit runs off the lint/scan
    /// hot path so the short owned-string cost (typically ≤32 bytes)
    /// is acceptable.
    ///
    /// Returns `Cow::Borrowed("unknown.<canonical>")` when the token
    /// does not route through [`capco_token_category`] (defensive —
    /// shouldn't happen for any registered token; visible signal in
    /// audit output if it does).
    ///
    /// PM-D-10 (PR 3c.2.D) — see
    /// `docs/plans/2026-05-20-pr3c2-d-pm-decisions.md`.
    fn qualified_token_label(&self, token: &TokenId) -> std::borrow::Cow<'static, str> {
        use marque_scheme::MarkingScheme;
        let canonical = self.metadata(token).canonical;
        let Some(cat_id) = capco_token_category(*token) else {
            return std::borrow::Cow::Owned(format!("unknown.{canonical}"));
        };
        // O(n) scan over ~12 categories — off the lint/scan hot path.
        let cat_name = self
            .categories()
            .iter()
            .find(|c| c.id == cat_id)
            .map(|c| c.name)
            .unwrap_or("unknown");
        std::borrow::Cow::Owned(format!("{cat_name}.{canonical}"))
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

    // -------- FGI / REL TO list-token (open vocab — 3 OR 4 ASCII upper) -
    //
    // Per CAPCO-2016 §H.7 p122 ("Multiple FGI trigraph country codes
    // or tetragraph codes must be separated by a single space ...
    // example may appear as: SECRET//FGI GBR JPN NATO//REL TO USA,
    // GBR, JPN, NATO."), FGI lists admit BOTH 3-letter Annex B
    // trigraphs AND 4-letter Annex A tetragraphs. §H.8 p150 admits
    // the same shape on REL TO. These tests pin that contract so a
    // future "trigraph-only" regression is caught here.

    #[test]
    fn fgi_country_token_admits_trigraphs() {
        let v = vocab();
        assert!(v.shape_admits(CAT_FGI_MARKER, b"USA"));
        assert!(v.shape_admits(CAT_FGI_MARKER, b"GBR"));
        assert!(v.shape_admits(CAT_FGI_MARKER, b"JPN"));
    }

    #[test]
    fn fgi_country_token_admits_tetragraphs() {
        // NOTE: CAT_FGI_MARKER at the vocabulary surface admits these
        // tetragraphs (FVEY, etc.) for round-trip compatibility, while
        // `parse_fgi_marker` (the strict parser) rejects all non-`NATO`
        // tetragraphs via `admits_fgi_ownership_token`. This is a
        // deliberate divergence per issue #280; see `parse_fgi_marker`
        // doc-comment for the rationale. A future change that
        // "harmonizes" the two surfaces would silently re-narrow
        // CAT_FGI_MARKER — that is the contract this test pins against.
        //
        // Per CAPCO-2016 §H.7 p122, the FGI list grammar admits
        // tetragraphs (e.g., NATO, FVEY, ISAF). Registry membership
        // is rule-layer; this gate is pure shape.
        let v = vocab();
        assert!(v.shape_admits(CAT_FGI_MARKER, b"NATO"));
        assert!(v.shape_admits(CAT_FGI_MARKER, b"FVEY"));
        assert!(v.shape_admits(CAT_FGI_MARKER, b"ISAF"));
        assert!(v.shape_admits(CAT_FGI_MARKER, b"ACGU"));
    }

    #[test]
    fn fgi_country_token_admits_two_letter_exception() {
        // ODNI ISMCAT `CVEnumISMCATRelTo` ships `EU` as a registered
        // 2-letter exception code admitted in FGI/REL TO list slots.
        // Pre-PR-2 admission accepted it via the union TRIGRAPHS
        // table; the shape gate must not narrow that surface.
        let v = vocab();
        assert!(v.shape_admits(CAT_FGI_MARKER, b"EU"));
    }

    #[test]
    fn fgi_country_token_rejects_lowercase() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"usa"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"Usa"));
        // Tetragraph case must reject lowercase too — admission is
        // shape, and the shape requires uniform ASCII upper.
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"nato"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"NaTO"));
        // Same for the 2-letter exception.
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"eu"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"Eu"));
    }

    #[test]
    fn fgi_country_token_rejects_wrong_length() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_FGI_MARKER, b""));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"U")); // single letter
        // 5+ bytes (e.g., `AUSTRALIA_GROUP`) explicitly out of
        // scope at this gate — admitted via `try_new` for a
        // separate non-FGI/REL-TO admission path.
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"USAGB"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"AUSTRALIA_GROUP"));
    }

    #[test]
    fn fgi_country_token_rejects_digits() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"123"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"US1"));
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"NAT0")); // 0 not O
    }

    #[test]
    fn rel_to_admits_shape_eligible_codes() {
        let v = vocab();
        // Trigraph + tetragraph + 2-letter EU exception admit
        // symmetrically across CAT_FGI_MARKER and CAT_REL_TO.
        assert!(v.shape_admits(CAT_REL_TO, b"USA"));
        assert!(v.shape_admits(CAT_REL_TO, b"GBR"));
        assert!(v.shape_admits(CAT_REL_TO, b"NATO"));
        assert!(v.shape_admits(CAT_REL_TO, b"FVEY"));
        assert!(v.shape_admits(CAT_REL_TO, b"EU"));
    }

    #[test]
    fn rel_to_admits_registered_long_codes() {
        // `AUSTRALIA_GROUP` is a 15-byte registered REL TO code in
        // `marque_ism::TRIGRAPHS`. Pure shape (`admits_country_token`,
        // 2/3/4 ASCII upper) cannot encode it; registry membership
        // does. This is the asymmetry CAT_REL_TO has vs CAT_FGI_MARKER:
        // strict parser (`parse_rel_to_with_spans` via
        // `tokens.is_trigraph(...)`) admits AUSTRALIA_GROUP, so
        // shape_admits must too — otherwise callers that adopt
        // shape_admits as the category admission contract will
        // disagree with the strict parser on lawful inputs.
        let v = vocab();
        assert!(v.shape_admits(CAT_REL_TO, b"AUSTRALIA_GROUP"));
    }

    #[test]
    fn rel_to_rejects_arbitrary_long_codes_not_in_registry() {
        // The registry-membership widening admits ONLY codes the
        // registry actually contains. An arbitrary 5+-byte
        // upper/underscore string that isn't a registered REL TO
        // code must still reject — registry membership ≠ shape gate.
        let v = vocab();
        assert!(!v.shape_admits(CAT_REL_TO, b"ARBITRARY_LONG_CODE"));
        assert!(!v.shape_admits(CAT_REL_TO, b"USAGB"));
        assert!(!v.shape_admits(CAT_REL_TO, b"FAKE_GROUP"));
    }

    #[test]
    fn rel_to_rejects_invalid_inputs() {
        let v = vocab();
        assert!(!v.shape_admits(CAT_REL_TO, b"usa"));
        assert!(!v.shape_admits(CAT_REL_TO, b"nato"));
        assert!(!v.shape_admits(CAT_REL_TO, b"eu"));
        assert!(!v.shape_admits(CAT_REL_TO, b"australia_group"));
        assert!(!v.shape_admits(CAT_REL_TO, b"123"));
        assert!(!v.shape_admits(CAT_REL_TO, b"U")); // single letter
        assert!(!v.shape_admits(CAT_REL_TO, b""));
    }

    #[test]
    fn fgi_marker_rejects_australia_group_class_codes() {
        // CAT_FGI_MARKER does NOT widen via TRIGRAPHS membership —
        // CAPCO §H.7 p122 admits trigraphs + tetragraphs only;
        // AUSTRALIA_GROUP-class registered codes are not lawful
        // FGI material. This test pins the asymmetry between
        // CAT_FGI_MARKER and CAT_REL_TO so a future cleanup
        // doesn't accidentally widen FGI by symmetry argument.
        let v = vocab();
        assert!(!v.shape_admits(CAT_FGI_MARKER, b"AUSTRALIA_GROUP"));
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
    fn sar_rejects_lowercase_open_vocab_shape_is_validation() {
        // Issue #280: SAR has no CVE registry (`CVEnumISMSAR.xml`
        // intentionally empty per ODNI policy). With no registry to
        // validate against, the shape gate IS the validation. Per
        // CAPCO-2016 §A.6 p15 + §G.1 p36, all banner-line and
        // portion-mark Register entries are uppercase; SAR
        // identifiers must conform. The `CAT_SAR` shape gate
        // delegates to `SarProgram::admits_program_id_abbrev`, which
        // was tightened in #280. Lowercase falls through to the
        // decoder, which handles demangling.
        let v = vocab();
        assert!(!v.shape_admits(CAT_SAR, b"bp"));
        assert!(!v.shape_admits(CAT_SAR, b"Bp"));
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

// ---------------------------------------------------------------------------
// FD&R dominator predicate — bidirectional pin against FDR_DOMINATORS.
// ---------------------------------------------------------------------------
//
// The override `Vocabulary::is_fdr_dissem` iterates `FDR_DOMINATORS`
// directly, matching `TokenRef::Token` entries by `TokenId` equality
// and `TokenRef::AnyInCategory` entries via `capco_token_category`.
// It does NOT delegate to `is_fdr_dominator` — that function excludes
// RELIDO (RELIDO-vs-RELIDO is a tautology in its RELIDO-conflict
// family role), but RELIDO is itself an FD&R marking per §B.3.a p19,
// so the override walks the slice instead. These unit tests pin the
// production-side `FDR_DOMINATORS` slice bidirectionally so a future
// addition (either a new `Token` entry or a new `AnyInCategory`
// entry) cannot silently drift away from the override.
//
// The integration test file `crates/capco/tests/fdr_dissem_predicate.rs`
// exercises the public API surface (canonical dominators, REL TO
// country tokens, non-FD&R rejections). It lives outside the crate
// and cannot read `pub(crate) static FDR_DOMINATORS`; the pin tests
// here exercise the in-crate source-of-truth instead. This split
// follows the project memory `pub_doc_hidden_is_still_public_api`:
// `pub(crate)` + unit tests over exposing a `pub` test-only surface.

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod fdr_dissem_pin {
    use super::*;
    use crate::scheme::FDR_DOMINATORS;
    use marque_scheme::{TokenRef, Vocabulary};

    fn vocab() -> CapcoScheme {
        CapcoScheme::new()
    }

    /// Walk every entry in `FDR_DOMINATORS` and assert
    /// `is_fdr_dissem` reaches the underlying token (or category).
    ///
    /// For `TokenRef::Token(id)` entries the test passes the id
    /// directly. For `TokenRef::AnyInCategory(cat)` entries the
    /// assertion is indirect: there is no single `TokenId` standing
    /// in for the whole category, so the test verifies the override
    /// can route AT LEAST ONE `TokenId` to that category via
    /// `capco_token_category`. If a future revision adds an
    /// `AnyInCategory(CAT_X)` entry for a category whose tokens are
    /// not routed by `capco_token_category`, the override silently
    /// under-fires; this test fails with a clear message so the
    /// gap is fixed before the row lands.
    #[test]
    fn fdr_dominators_entries_all_reachable() {
        let v = vocab();
        for entry in FDR_DOMINATORS {
            match entry {
                TokenRef::Token(id) => assert!(
                    v.is_fdr_dissem(id),
                    "FDR_DOMINATORS entry {entry:?} not recognized by \
                     is_fdr_dissem — the override has drifted from the \
                     authoritative slice. Verify the iter().any(...) \
                     walk in vocabulary.rs reaches every entry.",
                ),
                TokenRef::AnyInCategory(cat) => {
                    // Verify at least one sentinel `TokenId` resolves to
                    // this category via `capco_token_category`. Today
                    // CAT_REL_TO has TOK_USA and TOK_REL_TO routed to it;
                    // a future category-only FD&R entry MUST have a
                    // matching `capco_token_category` arm.
                    let routed = sentinel_routed_to_category(*cat);
                    assert!(
                        routed,
                        "FDR_DOMINATORS contains AnyInCategory({cat:?}) \
                         but no known sentinel TokenId resolves to that \
                         category via capco_token_category. The \
                         is_fdr_dissem override iterates over \
                         FDR_DOMINATORS and routes single TokenIds \
                         through `capco_token_category` to hit \
                         AnyInCategory arms; without a category-routing \
                         arm the new category is unreachable. Add a \
                         capco_token_category arm covering the new \
                         category before this row lands.",
                    );
                }
            }
        }
    }

    /// Walk a sample of sentinel `TokenId`s and look for one that
    /// `capco_token_category` resolves to `cat`. Used by the
    /// reachability assertion above. The probe set is the closed
    /// CAPCO sentinel set — enumerating it directly keeps the test
    /// honest about what `capco_token_category` actually maps,
    /// without taking a dependency on the open-vocab routing
    /// machinery.
    fn sentinel_routed_to_category(cat: marque_scheme::CategoryId) -> bool {
        use crate::scheme::*;
        // The sentinel set is enumerated explicitly so a new sentinel
        // is added here intentionally rather than picked up via reflection
        // (which Rust doesn't offer for static consts anyway).
        let probes: &[TokenId] = &[
            TOK_NOFORN,
            TOK_JOINT,
            TOK_USA,
            TOK_RESTRICTED,
            TOK_RD,
            TOK_FRD,
            TOK_TFNI,
            TOK_CNWDI,
            TOK_UCNI,
            // Issue #407: TOK_DCNI added so the probe set covers the
            // DOD UCNI variant sentinel introduced alongside the
            // UCNI/DCNI variant split.
            TOK_DCNI,
            TOK_HCS,
            TOK_NODIS,
            TOK_EXDIS,
            TOK_RELIDO,
            TOK_DISPLAY_ONLY,
            TOK_ORCON,
            TOK_ORCON_USGOV,
            TOK_REL_TO,
            TOK_SBU_NF,
            TOK_LES_NF,
            TOK_IMCON,
            TOK_DSEN,
            TOK_RSEN,
            TOK_FOUO,
            // PR 4b-C Commit 1 IC dissem additions (sentinels already
            // declared; absent from this probe set pre-#407).
            TOK_PROPIN,
            TOK_FISA,
            TOK_RAWFISA,
            TOK_LIMDIS,
            TOK_LES,
            TOK_SBU,
            TOK_SSI,
            // Issue #407: NNPI sentinel was missing from the probe
            // set; add for coverage parity with the rest of the
            // CAT_NON_IC_DISSEM family.
            TOK_NNPI,
            TOK_EYES,
            TOK_ATOMAL,
            TOK_BALK,
            TOK_BOHEMIA,
            // Issue #524 (Phase 1): per-compartment SCI sentinels routed
            // to CAT_SCI alongside `TOK_HCS` / `TOK_BALK` / `TOK_BOHEMIA`.
            TOK_SI_G,
            TOK_HCS_O,
            TOK_HCS_P,
            TOK_TK_BLFH,
            TOK_TK_IDIT,
            TOK_TK_KAND,
        ];
        probes
            .iter()
            .any(|id| capco_token_category(*id) == Some(cat))
    }

    /// Pin: RELIDO is an FD&R member even though it is excluded from
    /// `is_fdr_dominator` (which models RELIDO-conflict, not FD&R
    /// membership). The override iterates over `FDR_DOMINATORS`
    /// directly so this case admits — a regression that delegates
    /// through `is_fdr_dominator` would fail this test.
    #[test]
    fn relido_admits_despite_is_fdr_dominator_excluding_it() {
        use crate::scheme::TOK_RELIDO;
        let v = vocab();
        assert!(
            v.is_fdr_dissem(&TOK_RELIDO),
            "RELIDO is unambiguously an FD&R member per §B.3.a p19. \
             The override must not delegate through \
             `is_fdr_dominator`, which deliberately excludes RELIDO \
             for the RELIDO-conflict family predicate.",
        );
    }
}
