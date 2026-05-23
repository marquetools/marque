// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::scheme::{
    TOK_ATOMAL, TOK_BALK, TOK_BOHEMIA, TOK_CNWDI, TOK_DCNI, TOK_EXDIS, TOK_FISA, TOK_FRD, TOK_HCS,
    TOK_HCS_O, TOK_HCS_P, TOK_NNPI, TOK_NODIS, TOK_NOFORN, TOK_ORCON_USGOV, TOK_RD, TOK_RESTRICTED,
    TOK_SI_G, TOK_SSI, TOK_TFNI, TOK_TK_BLFH, TOK_TK_IDIT, TOK_TK_KAND, TOK_UCNI,
};
use marque_ism::Classification;
use marque_ism::generated::migrations::find_migration;
use marque_ism::generated::vocabulary::{
    CVE_FILES, CveFileMetadata, TokenMetadataEntry, lookup_token_metadata,
};
use marque_ism::marking_forms::{MARKING_FORMS, MarkingForm};
use marque_scheme::{
    Authority, Deprecation, FormKind, FormSet, OwnerProducer, OwnerProducerKind, PointOfContact,
    TokenId, TokenMetadataFull,
};
use std::sync::LazyLock;

pub fn active_sentinel_count() -> usize {
    SENTINEL_TO_CANONICAL.len()
}

const SENTINEL_TO_CANONICAL: &[(TokenId, &str)] = &[
    (TOK_NOFORN, "NF"),
    (TOK_RD, "RD"),
    (TOK_FRD, "FRD"),
    (TOK_TFNI, "TFNI"),
    (TOK_CNWDI, "RD-CNWDI"),
    (TOK_UCNI, "UCNI"),
    (TOK_HCS, "HCS"),
    (TOK_HCS_O, "HCS-O"),
    (TOK_HCS_P, "HCS-P"),
    (TOK_SI_G, "SI-G"),
    (TOK_TK_BLFH, "TK-BLFH"),
    (TOK_TK_IDIT, "TK-IDIT"),
    (TOK_TK_KAND, "TK-KAND"),
    (TOK_RESTRICTED, "R"),
    (TOK_NODIS, "ND"),
    (TOK_EXDIS, "XD"),
    (TOK_ORCON_USGOV, "OC-USGOV"),
    (TOK_FISA, "FISA"),
    (TOK_SSI, "SSI"),
    (TOK_NNPI, "NNPI"),
    (TOK_DCNI, "DCNI"),
    (TOK_ATOMAL, "NATO-ATOMAL"),
    (TOK_BALK, "NATO-BALK"),
    (TOK_BOHEMIA, "NATO-BOHEMIA"),
];

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
                 `SENTINEL_TO_CANONICAL` in crates/capco/src/vocabulary/tokens.rs."
            )
        })
}

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

struct CveFileDerived {
    cve_const_name: &'static str,
    authority: Authority,
    owner_producer: OwnerProducer,
}

static CVE_FILE_DERIVED: LazyLock<Vec<CveFileDerived>> = LazyLock::new(|| {
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
            panic!(
                "Vocabulary<CapcoScheme>: CveFile {cve_const_name:?} missing \
                 from CVE_FILE_DERIVED — build.rs and the LazyLock init \
                 disagree on the CVE-file set."
            )
        })
}

fn build_authority(cve_file: &'static CveFileMetadata) -> Authority {
    Authority {
        source_name: cve_file.source,
        urn: cve_file.urn,
        schema_version: cve_file.schema_version,
        point_of_contact: build_capco_point_of_contact(cve_file),
    }
}

fn owner_producer_name(code: &'static str) -> &'static str {
    match code {
        "USA" => "United States of America",
        "NATO" => "North Atlantic Treaty Organization",
        "FGI" => "Foreign Government Information",
        unknown => panic!(
            "Vocabulary<CapcoScheme>: unknown owner-producer code {unknown:?}. \
             Extend `owner_producer_name` in crates/capco/src/vocabulary/tokens.rs \
             with the human-readable name from the `ism-ismcat` crate's \
             Schema/ISMCAT/CVEGenerated/CVEnumISMCATOwnerProducer.xml."
        ),
    }
}

fn build_owner_producer(cve_file: &'static CveFileMetadata) -> OwnerProducer {
    OwnerProducer {
        code: cve_file.owner_producer,
        name: owner_producer_name(cve_file.owner_producer),
        kind: OwnerProducerKind::National,
    }
}

fn build_capco_point_of_contact(cve_file: &'static CveFileMetadata) -> PointOfContact {
    PointOfContact {
        name: cve_file.poc_name,
        email: cve_file.poc_email,
        organization: "ODNI",
    }
}

struct TokenDerived {
    token: TokenId,
    metadata: TokenMetadataFull<TokenId>,
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
            panic!(
                "Vocabulary<CapcoScheme>: TokenId {token:?} has no canonical \
                 CVE value. See `SENTINEL_TO_CANONICAL` in \
                 crates/capco/src/vocabulary/tokens.rs."
            )
        })
}

fn build_metadata(token: TokenId) -> TokenMetadataFull<TokenId> {
    let entry = entry_for(token);
    let derived = derived_for_token(token);
    let canonical = entry.value;
    let form_set = build_form_set(canonical);
    TokenMetadataFull {
        canonical,
        urn: entry.cve_file.urn,
        schema_version: entry.cve_file.schema_version,
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

fn classification_form_set(canonical: &'static str) -> Option<FormSet> {
    let class = match canonical {
        "TS" => Classification::TopSecret,
        "S" => Classification::Secret,
        "C" => Classification::Confidential,
        "U" => Classification::Unclassified,
        _ => return None,
    };
    Some(FormSet {
        portion: class.portion_str(),
        banner_title: class.banner_str(),
        banner_abbreviation: None,
        recognized_aliases: &[],
    })
}

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

fn build_form_set(canonical: &'static str) -> FormSet {
    if let Some(class_form_set) = classification_form_set(canonical) {
        return class_form_set;
    }
    if let Some(nato_form_set) = nato_program_form_set(canonical) {
        return nato_form_set;
    }

    let row: Option<&'static MarkingForm> = MARKING_FORMS
        .iter()
        .find(|f| f.portion == canonical || f.banner == canonical);
    let recognized_aliases = recognized_aliases_for_canonical(canonical);

    match row {
        Some(f) => {
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
        None => FormSet {
            portion: canonical,
            banner_title: canonical,
            banner_abbreviation: None,
            recognized_aliases,
        },
    }
}

fn build_deprecation(canonical: &'static str) -> Option<Deprecation<TokenId>> {
    let migration = find_migration(canonical)?;
    Some(Deprecation {
        since: marque_ism::SCHEMA_VERSION,
        valid_from: None,
        valid_until: migration.valid_until,
        replacement: SENTINEL_TO_CANONICAL
            .iter()
            .find(|(_, s)| *s == migration.replacement)
            .map(|(id, _)| *id),
    })
}

pub(super) fn authority_static(token: TokenId) -> &'static Authority {
    &derived_for_token(token).authority
}

pub(super) fn owner_producer_static(token: TokenId) -> &'static OwnerProducer {
    &derived_for_token(token).owner_producer
}

pub(super) fn point_of_contact_static(token: TokenId) -> &'static PointOfContact {
    &derived_for_token(token).authority.point_of_contact
}

pub(super) fn deprecation_static(token: TokenId) -> Option<&'static Deprecation<TokenId>> {
    token_derived(token).metadata.deprecation.as_ref()
}

pub(super) fn forms_static(token: TokenId) -> &'static FormSet {
    &token_derived(token).form_set
}

pub(super) fn metadata_static(token: TokenId) -> &'static TokenMetadataFull<TokenId> {
    &token_derived(token).metadata
}
