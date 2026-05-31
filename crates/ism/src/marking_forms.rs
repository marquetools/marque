// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Single source of truth for banner ↔ portion marking form mappings.
//!
//! The CAPCO Register (CAPCO-2016 §G.1 Table 4, p36–38, spanning the SCI /
//! NATO / AEA / dissem / non-IC blocks) defines three columns per marking:
//!
//! - **Marking Title** (full descriptive name, e.g., "NOT RELEASABLE TO FOREIGN NATIONALS")
//! - **Banner Line Abbreviation** (e.g., "NOFORN")
//! - **Portion Mark** (e.g., "NF")
//!
//! For most markings, banner and portion forms are identical (e.g., HCS, FISA,
//! RELIDO). This module tracks (a) entries where the forms *differ* — for
//! E001 (banner uses portion abbreviation) and E009 (portion uses banner
//! expansion) detection — (b) same-form-with-distinct-title entries for S001
//! `prefer-banner-abbreviation` substitutions, and (c) documentation rows
//! that anchor §G.1 Table 4 fidelity even when no substitution is possible
//! (NATO programs, CVE-Value-bridge anchors). See `MARKING_FORMS` doc for the
//! three row shapes.
//!
//! Per CAPCO-2016 §D.1 p27, a banner line may spell out the Marking Title
//! OR use the Authorized Abbreviation — both are valid. Detection of the long
//! title in a banner is driven by the [`MarkingForm::title`] field and owned
//! by the S001 `prefer-banner-abbreviation` style rule. `title == banner` when
//! a marking has no distinct abbreviation (e.g., `DEA SENSITIVE`, whose
//! Register row shows `None` under the abbreviation column); S001 must not
//! fire on those.
//!
//! Classification levels (TOP SECRET ↔ TS, etc.) are handled separately by
//! [`crate::Classification::banner_str`] / [`crate::Classification::portion_str`]
//! because they follow a different structural pattern (banners use full words
//! with no abbreviation, not a shortened form).
//!
//! # Maintenance
//!
//! This table is hand-maintained from the CAPCO Register. The ODNI CVE XML
//! schemas only carry the portion-form codes; banner abbreviations and titles
//! are a CAPCO marking convention not encoded in the XML. When ODNI publishes
//! a new register, update this table and bump the schema version in
//! `crates/ism/Cargo.toml`.

/// A marking where the banner-line abbreviation differs from the portion mark.
///
/// Fields correspond to the three columns of CAPCO-2016 §G.1 Table 4.
/// [`Self::description_title`] carries an ODNI
/// `<Description>` title that diverges from the CAPCO Register
/// [`Self::title`]. Nine rows in the current ODNI ISM schema package
/// (ISM-v2022-DEC) carry `description_title: Some(...)` today (typos,
/// regulatory-citation prose, casing differences). The
/// `crates/ism/tests/description_title_divergence.rs` test pins the
/// exact divergence count and catalogs each case; remaining rows default
/// to `None`. Future ODNI / CAPCO publications that introduce additional
/// divergences gain a `description_title: Some("...")` entry and the
/// divergence-count pin updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkingForm {
    /// Long "Authorized Banner Line Marking Title" form (§G.1 Table 4,
    /// column 1), e.g., "NOT RELEASABLE TO FOREIGN NATIONALS".
    ///
    /// Equals [`Self::banner`] when the Register lists `None` under the
    /// abbreviation column (e.g., `DEA SENSITIVE`), meaning the marking has
    /// no distinct abbreviation. S001 uses `title != banner` to gate its
    /// fix proposal.
    pub title: &'static str,
    /// "Authorized Banner Line Abbreviation" form (§G.1 Table 4, column 2),
    /// e.g., "NOFORN", "ORCON". Equals [`Self::title`] when the Register
    /// lists no distinct abbreviation.
    pub banner: &'static str,
    /// "Authorized Portion Mark" form (§G.1 Table 4, column 3), e.g., "NF",
    /// "OC".
    pub short_form: &'static str,
    /// ODNI ISM CVE `<Description>` body when it diverges from
    /// [`Self::title`].
    ///
    /// The ODNI XML `<Description>` element body is the long-form title
    /// verbatim — there is no `<title>` sub-element. For nine entries
    /// in the current ODNI ISM schema package (ISM-v2022-DEC) the
    /// `<Description>` body differs from the corresponding CAPCO §G.1
    /// Table 4 title (typos like CNWDI's `"Controled"`, regulatory-
    /// citation prose for FISA / SSI / NNPI, casing or wording
    /// differences for DCNI / UCNI / OC-USGOV / SI-EU / SI-NK). Those
    /// rows carry `description_title: Some(<verbatim ODNI text>)` and
    /// surface in `marque-scheme::FormSet::recognized_aliases` with
    /// `FormKind::IsmDescriptionTitle` whenever the token has a closed-
    /// CVE sentinel (UCNI today; the rest will surface as the sentinel
    /// set expands per issue #407). Rows whose ODNI `<Description>`
    /// matches the CAPCO title (the majority of `MARKING_FORMS`)
    /// remain `None`.
    ///
    /// The exact divergent set is pinned by
    /// `crates/ism/tests/description_title_divergence.rs`; any future
    /// ODNI / CAPCO publication that introduces or retires a
    /// divergence trips that test and forces a manual review.
    pub description_title: Option<&'static str>,
}

/// All markings where the long Marking Title differs from the banner
/// abbreviation or portion mark.
///
/// Source: CAPCO Register (Implementation Manual for the IC, current edition).
///
/// Sections covered:
/// - §H.4  SCI Control System Markings (long-title forms + SI compounds)
/// - §H.6  Atomic Energy Act Information Markings
/// - §H.7  Non-US Protective Markings (NATO classifications + programs)
/// - §H.8  Dissemination Control Markings
/// - §H.9  Non-IC Dissemination Control Markings
///
/// Three kinds of entries are included:
///
/// 1. **Differing-form entries** (`title != banner || banner != portion`): E001
///    (banner uses portion abbreviation) and E009 (portion uses banner expansion)
///    need these to detect and correct cross-form usage.
///
/// 2. **Same-form entries** (`banner == portion` but `title != banner`): S001
///    fires when it sees the long Marking Title used in a banner line instead
///    of the authorized abbreviation (e.g. "FOR OFFICIAL USE ONLY" → "FOUO").
///    Without an entry here, S001 cannot detect the substitution opportunity.
///    `title == banner` entries (e.g. `DEA SENSITIVE`, NATO classifications)
///    are still included when the portion mark differs, but S001 skips them
///    (no substitution possible).
///
/// 3. **Documentation rows** (`title == banner == portion`): present for §G.1
///    Table 4 fidelity and as anchors for the future CVE-Value↔portion-form
///    bridge (e.g., CVE Value `DISPLAYONLY` → portion `DISPLAY ONLY`,
///    NATO programs `ATOMAL` / `BALK` / `BOHEMIA`). S001 / E001 / E009 all
///    skip these — they exist so the table is the single source of truth for
///    "is this a known CAPCO marking" lookups even when no substitution is
///    possible.
pub static MARKING_FORMS: &[MarkingForm] = &[
    // §H.4 SCI Control System Markings — long-title forms.
    // CAPCO-2016 §H.4 p73 defines full names for control systems. Banner and
    // portion forms are identical (e.g. TK, HCS, SI), so these are same-form
    // entries; only S001 uses them. Titles verified against §H.4 headings.
    // For MARVEL/KLAMATH, our register doesn't list them, but they are
    // in the (later-published) ISM `CVEnumISMSCIControls` schema, along
    // with `BUR` which is the same for all forms.
    MarkingForm {
        title: "TALENT KEYHOLE",
        banner: "TK",
        short_form: "TK",
        description_title: None,
    },
    MarkingForm {
        title: "RESERVE",
        banner: "RSV",
        short_form: "RSV",
        description_title: None,
    },
    MarkingForm {
        title: "MARVEL",
        banner: "MVL",
        short_form: "MVL",
        description_title: None,
    },
    MarkingForm {
        title: "KLAMATH",
        banner: "KLM",
        short_form: "KLM",
        description_title: None,
    },
    // §H.4 SCI compartment long-form rows. Per CAPCO §G.1 Table 4
    // (rows on pp 795-803): each compartment publishes a single-word
    // long-form Authorized Banner Line Marking Title (e.g. GAMMA)
    // whose banner abbreviation and portion abbreviation collapse to
    // the same short form (e.g. G). Because `title != banner` and
    // `banner == portion`, both `title_to_banner` and
    // `title_to_portion` resolve to the same short form for these rows.
    // The meaningful distinction is that there is no separate
    // banner-vs-portion conversion here; the shared short form powers
    // SCI long-form compartment canonicalization in the structural
    // subparser per §H.4 p61 + p87 + p91 + p95).
    MarkingForm {
        title: "GAMMA",
        banner: "G",
        short_form: "G",
        description_title: None,
    },
    MarkingForm {
        title: "BLUEFISH",
        banner: "BLFH",
        short_form: "BLFH",
        description_title: None,
    },
    MarkingForm {
        title: "IDITAROD",
        banner: "IDIT",
        short_form: "IDIT",
        description_title: None,
    },
    MarkingForm {
        title: "KANDIK",
        banner: "KAND",
        short_form: "KAND",
        description_title: None,
    },
    // §H.4 SCI compound forms where the Authorized Banner Line Abbreviation
    // equals the Authorized Portion Mark. CAPCO §D.1 p27 authorizes
    // EITHER the Marking Title OR the Authorized Abbreviation in the banner
    // line — `banner` is set to the Authorized Abbreviation (== portion),
    // not the longer Marking Title, so same-form semantics apply and rules
    // do not fire a no-op substitution fix when the abbreviation appears in
    // either position. The Marking Title (`title`) remains the long-form
    // recognize-only string: `title_to_banner("SI-ECRU")` → `Some("SI-EU")`,
    // enabling S001 to suggest the abbreviated form when the Title appears
    // verbatim in a banner line.
    MarkingForm {
        // CAPCO-2016 §H.4 p78: Marking Title "SI-ECRU".
        // §H.4 p78: Authorized Banner Line Abbreviation "SI-EU".
        // §H.4 p78: Authorized Portion Mark "SI-EU".
        // §D.1 p27: banner may use either Marking Title or
        // Authorized Abbreviation; Authorized Abbreviation chosen here.
        //
        // ODNI `<Description>` for `SI-EU` is the
        // bare compartment `"ECRU"` — the CAPCO Register prepends the
        // parent control system (`SI-`) to form the compound. The ODNI
        // form is recognize-only on input via
        // `FormKind::IsmDescriptionTitle`.
        title: "SI-ECRU",
        banner: "SI-EU",
        short_form: "SI-EU",
        description_title: Some("ECRU"),
    },
    MarkingForm {
        // CAPCO-2016 §H.4 p83: Marking Title "SI-NONBOOK".
        // §H.4 p83: Authorized Banner Line Abbreviation "SI-NK".
        // §H.4 p83: Authorized Portion Mark "SI-NK".
        // §D.1 p27: banner may use either Marking Title or
        // Authorized Abbreviation; Authorized Abbreviation chosen here.
        //
        // ODNI `<Description>` for `SI-NK` is the
        // bare compartment `"NONBOOK"`; CAPCO uses the compound
        // `SI-NONBOOK`. Recognize-only on input.
        title: "SI-NONBOOK",
        banner: "SI-NK",
        short_form: "SI-NK",
        description_title: Some("NONBOOK"),
    },
    // §H.7 Non-US Protective Markings — NATO classifications and programs.
    //
    // U.S. representation of NATO markings is fully governed by CAPCO; per
    // CAPCO-2016 §G.1 Table 4 p37, NATO classifications and programs
    // appear with the abbreviation column listed as `None`, meaning the
    // banner-line form spells out the marking title (no separate banner
    // abbreviation). Classification rows have a distinct portion form
    // (`CTS`, `NS`, `NC`, `NR`, `NU`); program rows are same-form across
    // all three columns. Compound NATO forms (CTSA, NSAT, NCA, CTS-BALK,
    // CTS-BOHEMIA, …) live in CAPCO Manual Appendix B p197–199; they are
    // parametric (NATO classification × NATO program) and are not
    // enumerated here pending the CAPCO-vs-ODNI title-authority
    // architectural discussion.
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| COSMIC TOP SECRET | None | CTS |`.
        title: "COSMIC TOP SECRET",
        banner: "COSMIC TOP SECRET",
        short_form: "CTS",
        description_title: None,
    },
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| NATO SECRET | None | NS |`.
        title: "NATO SECRET",
        banner: "NATO SECRET",
        short_form: "NS",
        description_title: None,
    },
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| NATO CONFIDENTIAL | None | NC |`.
        title: "NATO CONFIDENTIAL",
        banner: "NATO CONFIDENTIAL",
        short_form: "NC",
        description_title: None,
    },
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| NATO RESTRICTED | None | NR |`.
        title: "NATO RESTRICTED",
        banner: "NATO RESTRICTED",
        short_form: "NR",
        description_title: None,
    },
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| NATO UNCLASSIFIED | None | NU |`.
        title: "NATO UNCLASSIFIED",
        banner: "NATO UNCLASSIFIED",
        short_form: "NU",
        description_title: None,
    },
    // NATO programs — same-form across all three columns. Included here
    // for §G.1-Table-4 fidelity even though S001 cannot fire on them
    // (no substitution opportunity); they document the closed set of
    // NATO program markings recognized by CAPCO.
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| ATOMAL | None | ATOMAL |`.
        title: "ATOMAL",
        banner: "ATOMAL",
        short_form: "ATOMAL",
        description_title: None,
    },
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| BALK | None | BALK |`.
        title: "BALK",
        banner: "BALK",
        short_form: "BALK",
        description_title: None,
    },
    MarkingForm {
        // CAPCO-2016 §G.1 Table 4 p37: `| BOHEMIA | None | BOHEMIA |`.
        title: "BOHEMIA",
        banner: "BOHEMIA",
        short_form: "BOHEMIA",
        description_title: None,
    },
    // §H.5 Special Access Program Markings — intentionally omitted.
    // SAR is parametric (`SPECIAL ACCESS REQUIRED-[program identifier]` ↔
    // `SAR-[program identifier abbreviation]`, CAPCO-2016 §H.5 p101) and
    // parsed structurally by `parse_sar_category` in `crates/core/src/parser.rs`.
    // A bare `SAR-` row would never match real input.

    // §H.6 Atomic Energy Act Information Markings.
    // Long Marking Titles from CAPCO-2016 §H.6 p113–122. Banner and portion
    // forms are identical for RD, FRD, TFNI, CNWDI — same-form entries for
    // S001 detection. SIGMA [##] ↔ SG [##] is parametric and handled
    // separately by the parser's pattern-matching path, not this table.
    // DOD/DOE UCNI have differing forms and are entries of the first kind.
    MarkingForm {
        title: "RESTRICTED DATA",
        banner: "RD",
        short_form: "RD",
        description_title: None,
    },
    MarkingForm {
        title: "FORMERLY RESTRICTED DATA",
        banner: "FRD",
        short_form: "FRD",
        description_title: None,
    },
    MarkingForm {
        title: "TRANSCLASSIFIED FOREIGN NUCLEAR INFORMATION",
        banner: "TFNI",
        short_form: "TFNI",
        description_title: None,
    },
    MarkingForm {
        // ODNI `<Description>` carries the
        // Class-A misspelled / non-canonical-cased form
        // `"Controled Nuclear Weapon Design Information Warning
        // statement"` — "Controled" is misspelled (single l) and
        // the casing is inconsistent with the rest of the CVE
        // register. CAPCO §H.6 is the authoritative title.
        // Recognize-only on input via `FormKind::IsmDescriptionTitle`.
        title: "CRITICAL NUCLEAR WEAPON DESIGN INFORMATION",
        banner: "CNWDI",
        short_form: "CNWDI",
        description_title: Some("Controled Nuclear Weapon Design Information Warning statement"),
    },
    MarkingForm {
        // ODNI `<Description>` is
        // `"DoD CONTROLLED NUCLEAR INFORMATION"` — drops the
        // "UNCLASSIFIED" qualifier CAPCO §H.6 carries. Class-C
        // surface-form divergence; recognize-only on input.
        title: "DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION",
        banner: "DOD UCNI",
        short_form: "DCNI",
        description_title: Some("DoD CONTROLLED NUCLEAR INFORMATION"),
    },
    MarkingForm {
        // ODNI `<Description>` is
        // `"DoE CONTROLLED NUCLEAR INFORMATION"` — drops the
        // "UNCLASSIFIED" qualifier CAPCO §H.6 carries. Class-C
        // surface-form divergence; recognize-only on input.
        title: "DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION",
        banner: "DOE UCNI",
        short_form: "UCNI",
        description_title: Some("DoE CONTROLLED NUCLEAR INFORMATION"),
    },
    // §H.8 Dissemination Control Markings.
    //
    // Titles below are transcribed from CAPCO-2016 §G.1 Table 4 (lines
    // 821–841). Each row uses columns (Title | Abbreviation | Portion).
    MarkingForm {
        title: "NOT RELEASABLE TO FOREIGN NATIONALS",
        banner: "NOFORN",
        short_form: "NF",
        description_title: None,
    },
    MarkingForm {
        // ODNI `<Description>` is
        // `"ORIGINATOR CONTROLLED US GOVERNMENT"` — spells out
        // "US GOVERNMENT" instead of the CAPCO `-USGOV` abbreviation
        // and drops the hyphen. Class-C surface-form divergence;
        // recognize-only on input.
        title: "ORIGINATOR CONTROLLED-USGOV",
        banner: "ORCON-USGOV",
        short_form: "OC-USGOV",
        description_title: Some("ORIGINATOR CONTROLLED US GOVERNMENT"),
    },
    MarkingForm {
        title: "ORIGINATOR CONTROLLED",
        banner: "ORCON",
        short_form: "OC",
        description_title: None,
    },
    MarkingForm {
        title: "CONTROLLED IMAGERY",
        banner: "IMCON",
        short_form: "IMC",
        description_title: None,
    },
    MarkingForm {
        title: "CAUTION-PROPRIETARY INFORMATION INVOLVED",
        banner: "PROPIN",
        short_form: "PR",
        description_title: None,
    },
    MarkingForm {
        title: "RISK SENSITIVE",
        banner: "RSEN",
        short_form: "RS",
        description_title: None,
    },
    MarkingForm {
        // §G.1 Table 4 p36: `| DEA SENSITIVE | None | DSEN |`. No
        // distinct banner abbreviation — `title == banner`. S001 must
        // skip this row (no substitution possible).
        title: "DEA SENSITIVE",
        banner: "DEA SENSITIVE",
        short_form: "DSEN",
        description_title: None,
    },
    MarkingForm {
        // CAPCO-2016 §H.8 p157: EYES ONLY is NSA-only and deprecated;
        // the markings waiver expired 1 Oct 2017. The CVE value (portion
        // mark) is `"EYES"`; the ODNI `<Description>` long title is
        // `"EYES ONLY"` — the two forms differ, so this is a
        // differing-form entry.
        //
        // The compound banner form `USA/[LIST] EYES ONLY` (with a
        // country-trigraph list) is recognized by the scanner/parser's
        // `recognize_eyes_only_block` function.
        // This entry covers the bare banner form `EYES ONLY` (no country
        // list), which exists in legacy documents and in the wild. The
        // mapping `banner_to_portion("EYES ONLY")` → `Some("EYES")` →
        // `DissemControl::Eyes` ensures the parser tags these tokens
        // correctly rather than falling through to Unknown.
        //
        // Authority: CAPCO-2016 §H.8 p157; ODNI CVEnumISMDissem.xml
        // (`<Value>EYES</Value>`, `<Description>EYES ONLY</Description>`).
        title: "EYES ONLY",
        banner: "EYES ONLY",
        short_form: "EYES",
        description_title: None,
    },
    // §H.8 same-form entries: banner == portion, but title differs.
    // S001 fires when a banner line spells out the Marking Title instead
    // of the authorized abbreviation. §G.1 Table 4 / §H.8 p157–171.
    MarkingForm {
        title: "FOR OFFICIAL USE ONLY",
        banner: "FOUO",
        short_form: "FOUO",
        description_title: None,
    },
    MarkingForm {
        title: "RELEASABLE BY INFORMATION DISCLOSURE OFFICIAL",
        banner: "RELIDO",
        short_form: "RELIDO",
        description_title: None,
    },
    // from ISM `CVEnumISMDissem` schema
    MarkingForm {
        title: "RAW FOREIGN INTELLIGENCE SURVEILLANCE ACT",
        banner: "RAWFISA",
        short_form: "RAWFISA",
        description_title: None,
    },
    MarkingForm {
        // ODNI `<Description>` adds a regulatory
        // citation and definition. CAPCO §G.1 Table 4 lists the
        // concise title; the ODNI long form is admissible on input
        // via `FormKind::IsmDescriptionTitle` (recognize-only).
        title: "FOREIGN INTELLIGENCE SURVEILLANCE ACT",
        banner: "FISA",
        short_form: "FISA",
        description_title: Some(
            "Foreign Intelligence Surveillance Act. Related to unclassified \
             and declassified information that is collected from \
             unconsenting individuals under the authority of the Foreign \
             Intelligence Surveillance Act (FISA).",
        ),
    },
    MarkingForm {
        title: "DISPLAY ONLY",
        banner: "DISPLAY ONLY",
        short_form: "DISPLAY ONLY",
        description_title: None,
    },
    // §H.9 Non-IC Dissemination Control Markings.
    MarkingForm {
        title: "LIMITED DISTRIBUTION",
        banner: "LIMDIS",
        short_form: "DS",
        description_title: None,
    },
    MarkingForm {
        title: "EXCLUSIVE DISTRIBUTION",
        banner: "EXDIS",
        short_form: "XD",
        description_title: None,
    },
    MarkingForm {
        title: "NO DISTRIBUTION",
        banner: "NODIS",
        short_form: "ND",
        description_title: None,
    },
    // §H.9 same-form entries: banner == portion, but title differs.
    MarkingForm {
        title: "SENSITIVE BUT UNCLASSIFIED NOFORN",
        banner: "SBU NOFORN",
        short_form: "SBU-NF",
        description_title: None,
    },
    MarkingForm {
        title: "SENSITIVE BUT UNCLASSIFIED",
        banner: "SBU",
        short_form: "SBU",
        description_title: None,
    },
    MarkingForm {
        title: "LAW ENFORCEMENT SENSITIVE NOFORN",
        banner: "LES NOFORN",
        short_form: "LES-NF",
        description_title: None,
    },
    MarkingForm {
        title: "LAW ENFORCEMENT SENSITIVE",
        banner: "LES",
        short_form: "LES",
        description_title: None,
    },
    MarkingForm {
        // ODNI `<Description>` adds the 49 C.F.R.
        // §15.5 and §1520.5 citations and definition. CAPCO §G.1
        // Table 4 lists the concise title; the ODNI long form is
        // admissible on input via `FormKind::IsmDescriptionTitle`
        // (recognize-only).
        title: "SENSITIVE SECURITY INFORMATION",
        banner: "SSI",
        short_form: "SSI",
        description_title: Some(
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
        ),
    },
    // from ISM `CVEnumISMNonIC` schema
    MarkingForm {
        // ODNI `<Description>` adds the
        // reactor-safety definition. CAPCO §G.1 Table 4 lists the
        // concise title; the ODNI long form is admissible on input
        // via `FormKind::IsmDescriptionTitle` (recognize-only).
        title: "NAVAL NUCLEAR PROPULSION INFORMATION",
        banner: "NNPI",
        short_form: "NNPI",
        description_title: Some(
            "Naval Nuclear Propulsion Information. Related to the safety \
             of reactors and associated naval nuclear propulsion plants, \
             and control of radiation and radioactivity associated with \
             naval nuclear propulsion activities, including prescribing \
             and enforcing standards and regulations for these areas as \
             they affect the environment and the safety and health of \
             workers, operators, and the general public.",
        ),
    },
];

/// Look up the portion-form abbreviation for a banner-form string.
///
/// Used by:
/// - E009 (portion-abbreviation): detects banner forms in portions, suggests abbreviation
/// - Parser (`parse_dissem_full_form`): accepts banner-form input and maps to CVE code
///
/// Returns `None` if the input is not a known banner form, or if it is a
/// same-form entry (`banner == portion`, e.g., `LES`, `SBU`, `FOUO`) because
/// there is no distinct portion abbreviation to substitute.
/// Note: `NOFORN` is **not** a same-form entry — in [`MARKING_FORMS`] it maps
/// banner `NOFORN` → portion `NF`, so this function returns `Some("NF")` for it.
/// Same-form entries return `None` here; during parsing, long-title inputs are
/// resolved via `title_to_portion`, while abbreviation inputs are already
/// handled by `DissemControl::parse`.
pub fn banner_to_portion(banner: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.banner == banner && f.banner != f.short_form)
        .map(|f| f.short_form)
}

/// Look up the banner-form expansion for a portion-form abbreviation.
///
/// Used by:
/// - E001 (portion-mark-in-banner): detects portion marks used in banner lines, suggests banner abbreviation
///
/// Returns `None` if the input is not a known portion form that has a *distinct*
/// banner form (`banner != portion`). Same-form entries (e.g., `LES`, `SBU`,
/// `FOUO`, `FISA`, `RELIDO`) return `None` because there is no substitution to
/// make — E001 must not fire a no-op fix for them.
pub fn portion_to_banner(short_form: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.short_form == portion && f.banner != f.short_form)
        .map(|f| f.banner)
}

/// Look up the portion-form abbreviation for a long "Marking Title" string.
///
/// Used by:
/// - Parser (`parse_dissem_full_form`): accepts long-title input like
///   `"NOT RELEASABLE TO FOREIGN NATIONALS"` and maps to the same
///   `DissemControl` the abbreviation would produce.
///
/// Returns `None` if the input is not a known title, or if the marking has
/// no distinct banner abbreviation (`title == banner`). The second case
/// avoids shadowing the dedicated `banner_to_portion` path for inputs like
/// `"DEA SENSITIVE"`.
pub fn title_to_portion(title: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.title == title && f.title != f.banner)
        .map(|f| f.short_form)
}

/// Look up the banner-line abbreviation for a long "Marking Title" string.
///
/// Used by:
/// - S001 (prefer-banner-abbreviation): detects long-title forms in banner
///   markings and proposes the abbreviation as a style fix.
///
/// Returns `None` when no substitution is possible — either the input is
/// unknown, or the marking has no distinct abbreviation (`title == banner`,
/// e.g., `DEA SENSITIVE`). The second case is deliberate: S001 must not
/// fire on rows where the Register lists no abbreviation.
pub fn title_to_banner(title: &str) -> Option<&'static str> {
    MARKING_FORMS
        .iter()
        .find(|f| f.title == title && f.title != f.banner)
        .map(|f| f.banner)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn banner_to_portion_known_entries() {
        assert_eq!(banner_to_portion("NOFORN"), Some("NF"));
        assert_eq!(banner_to_portion("ORCON"), Some("OC"));
        assert_eq!(banner_to_portion("IMCON"), Some("IMC"));
        assert_eq!(banner_to_portion("DEA SENSITIVE"), Some("DSEN"));
        assert_eq!(banner_to_portion("PROPIN"), Some("PR"));
        assert_eq!(banner_to_portion("RSEN"), Some("RS"));
        assert_eq!(banner_to_portion("LIMDIS"), Some("DS"));
        assert_eq!(banner_to_portion("EXDIS"), Some("XD"));
        assert_eq!(banner_to_portion("NODIS"), Some("ND"));
        assert_eq!(banner_to_portion("SBU NOFORN"), Some("SBU-NF"));
        assert_eq!(banner_to_portion("LES NOFORN"), Some("LES-NF"));
        assert_eq!(banner_to_portion("DOD UCNI"), Some("DCNI"));
        assert_eq!(banner_to_portion("DOE UCNI"), Some("UCNI"));
        // §H.7 NATO classifications — banner spells out the title, portion
        // is the abbreviation. CAPCO-2016 §G.1 Table 4 p37.
        assert_eq!(banner_to_portion("COSMIC TOP SECRET"), Some("CTS"));
        assert_eq!(banner_to_portion("NATO SECRET"), Some("NS"));
        assert_eq!(banner_to_portion("NATO CONFIDENTIAL"), Some("NC"));
        assert_eq!(banner_to_portion("NATO RESTRICTED"), Some("NR"));
        assert_eq!(banner_to_portion("NATO UNCLASSIFIED"), Some("NU"));
        // §H.4 SCI compounds — banner == portion (same-form) after the
        // Authorized Abbreviation fix (§H.4 p78, §H.4 p83,
        // §D.1 p27). The helpers return None for same-form entries.
        assert_eq!(banner_to_portion("SI-EU"), None);
        assert_eq!(banner_to_portion("SI-NK"), None);
        // The old title form is no longer a `banner` field value.
        assert_eq!(banner_to_portion("SI-ECRU"), None);
        assert_eq!(banner_to_portion("SI-NONBOOK"), None);
        // §H.8 EYES ONLY — bare banner form maps to CVE portion `EYES`.
        // CAPCO-2016 §H.8 p157. Allows `SECRET//EYES ONLY` to parse as
        // `DissemControl::Eyes` instead of falling through to Unknown.
        assert_eq!(banner_to_portion("EYES ONLY"), Some("EYES"));
    }

    #[test]
    fn portion_to_banner_known_entries() {
        assert_eq!(portion_to_banner("NF"), Some("NOFORN"));
        assert_eq!(portion_to_banner("OC"), Some("ORCON"));
        assert_eq!(portion_to_banner("IMC"), Some("IMCON"));
        assert_eq!(portion_to_banner("DSEN"), Some("DEA SENSITIVE"));
        assert_eq!(portion_to_banner("PR"), Some("PROPIN"));
        assert_eq!(portion_to_banner("RS"), Some("RSEN"));
        assert_eq!(portion_to_banner("DS"), Some("LIMDIS"));
        assert_eq!(portion_to_banner("XD"), Some("EXDIS"));
        // spellchecker:ignore-next-line
        assert_eq!(portion_to_banner("ND"), Some("NODIS"));
        assert_eq!(portion_to_banner("SBU-NF"), Some("SBU NOFORN"));
        assert_eq!(portion_to_banner("LES-NF"), Some("LES NOFORN"));
        assert_eq!(portion_to_banner("DCNI"), Some("DOD UCNI"));
        assert_eq!(portion_to_banner("UCNI"), Some("DOE UCNI"));
        // §H.7 NATO classifications — inverse direction.
        // CAPCO-2016 §G.1 Table 4 p37.
        assert_eq!(portion_to_banner("CTS"), Some("COSMIC TOP SECRET"));
        assert_eq!(portion_to_banner("NS"), Some("NATO SECRET"));
        assert_eq!(portion_to_banner("NC"), Some("NATO CONFIDENTIAL"));
        assert_eq!(portion_to_banner("NR"), Some("NATO RESTRICTED"));
        assert_eq!(portion_to_banner("NU"), Some("NATO UNCLASSIFIED"));
        // §H.4 SCI compounds — banner == portion (same-form); helpers return None.
        // CAPCO-2016 §H.4 p78, §H.4 p83, §D.1 p27.
        assert_eq!(portion_to_banner("SI-EU"), None);
        assert_eq!(portion_to_banner("SI-NK"), None);
        // §H.8 EYES ONLY — inverse direction. CVE portion `EYES` → bare
        // banner title `EYES ONLY`. CAPCO-2016 §H.8 p157.
        assert_eq!(portion_to_banner("EYES"), Some("EYES ONLY"));
    }

    #[test]
    fn banner_to_portion_returns_none_for_unknown() {
        assert_eq!(banner_to_portion("BANANAPHONE"), None);
    }

    #[test]
    fn portion_to_banner_returns_none_for_unknown() {
        assert_eq!(portion_to_banner("BANANAPHONE"), None);
    }

    #[test]
    fn banner_to_portion_returns_none_for_portion_form() {
        // Passing a portion form to banner_to_portion should not match.
        assert_eq!(banner_to_portion("NF"), None);
        assert_eq!(banner_to_portion("OC"), None);
    }

    #[test]
    fn portion_to_banner_returns_none_for_banner_form() {
        // Passing a banner form to portion_to_banner should not match.
        assert_eq!(portion_to_banner("NOFORN"), None);
        assert_eq!(portion_to_banner("ORCON"), None);
    }

    #[test]
    fn same_form_entries_return_none_from_conversion_helpers() {
        // Same-form entries (banner == portion) must return None from both
        // helpers so E001/E009 never fire a no-op substitution fix for them.
        // Regression guard for PR #256.
        for &same_form in &[
            // §H.8 / §H.9 same-form-with-distinct-title rows (S001 targets).
            "FOUO",
            "RELIDO",
            "FISA",
            "RAWFISA",
            "SBU",
            "LES",
            "SSI",
            "NNPI",
            // §H.4 SCI control systems (same-form-with-distinct-title).
            "TK",
            // §H.4 SCI compound abbreviations (same-form after Authorized
            // Abbreviation fix: banner == portion == abbreviation).
            // CAPCO-2016 §H.4 p78, §H.4 p83.
            "SI-EU",
            "SI-NK",
            // §H.6 AEA bare forms (same-form-with-distinct-title).
            "RD",
            "FRD",
            "TFNI",
            "CNWDI",
            // §H.7 NATO programs and §H.8 DISPLAY ONLY — same-form-all-three
            // documentation rows. Lookup helpers must still return None
            // because `f.banner != f.short_form` filters them out.
            "ATOMAL",
            "BALK",
            "BOHEMIA",
            "DISPLAY ONLY",
        ] {
            assert_eq!(
                banner_to_portion(same_form),
                None,
                "banner_to_portion({same_form:?}) should be None for same-form entry"
            );
            assert_eq!(
                portion_to_banner(same_form),
                None,
                "portion_to_banner({same_form:?}) should be None for same-form entry"
            );
        }
    }

    #[test]
    fn no_duplicate_banner_entries() {
        for (i, a) in MARKING_FORMS.iter().enumerate() {
            for (j, b) in MARKING_FORMS.iter().enumerate() {
                if i != j {
                    assert_ne!(a.banner, b.banner, "duplicate banner entry: {:?}", a.banner);
                }
            }
        }
    }

    #[test]
    fn no_duplicate_portion_entries() {
        for (i, a) in MARKING_FORMS.iter().enumerate() {
            for (j, b) in MARKING_FORMS.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        a.short_form, b.short_form,
                        "duplicate portion entry: {:?}",
                        a.short_form
                    );
                }
            }
        }
    }

    // Title-column lookups for the S001 style rule and the parser's
    // long-title acceptance path.

    #[test]
    fn title_to_portion_known_entries() {
        assert_eq!(
            title_to_portion("NOT RELEASABLE TO FOREIGN NATIONALS"),
            Some("NF")
        );
        assert_eq!(title_to_portion("ORIGINATOR CONTROLLED"), Some("OC"));
        assert_eq!(title_to_portion("CONTROLLED IMAGERY"), Some("IMC"));
        assert_eq!(
            title_to_portion("CAUTION-PROPRIETARY INFORMATION INVOLVED"),
            Some("PR")
        );
        assert_eq!(title_to_portion("RISK SENSITIVE"), Some("RS"));
        assert_eq!(title_to_portion("LIMITED DISTRIBUTION"), Some("DS"));
        assert_eq!(title_to_portion("EXCLUSIVE DISTRIBUTION"), Some("XD"));
        assert_eq!(title_to_portion("NO DISTRIBUTION"), Some("ND"));
        assert_eq!(
            title_to_portion("SENSITIVE BUT UNCLASSIFIED NOFORN"),
            Some("SBU-NF")
        );
        assert_eq!(
            title_to_portion("LAW ENFORCEMENT SENSITIVE NOFORN"),
            Some("LES-NF")
        );
        assert_eq!(
            title_to_portion("DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION"),
            Some("DCNI")
        );
        assert_eq!(
            title_to_portion("DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION"),
            Some("UCNI")
        );
        // §H.4 SCI compounds — title still resolves to the Authorized Portion
        // Mark even though `banner` was corrected to the abbreviation (PR #702:
        // `banner: "SI-ECRU"` → `"SI-EU"`, `banner: "SI-NONBOOK"` → `"SI-NK"`).
        // Guards S001 long-form detection semantics against future table edits.
        // CAPCO-2016 §H.4 p78, §H.4 p83.
        assert_eq!(title_to_portion("SI-ECRU"), Some("SI-EU"));
        assert_eq!(title_to_portion("SI-NONBOOK"), Some("SI-NK"));
    }

    #[test]
    fn title_to_banner_known_entries() {
        assert_eq!(
            title_to_banner("NOT RELEASABLE TO FOREIGN NATIONALS"),
            Some("NOFORN")
        );
        assert_eq!(title_to_banner("ORIGINATOR CONTROLLED"), Some("ORCON"));
        assert_eq!(title_to_banner("CONTROLLED IMAGERY"), Some("IMCON"));
        assert_eq!(
            title_to_banner("CAUTION-PROPRIETARY INFORMATION INVOLVED"),
            Some("PROPIN")
        );
        assert_eq!(title_to_banner("RISK SENSITIVE"), Some("RSEN"));
        assert_eq!(title_to_banner("LIMITED DISTRIBUTION"), Some("LIMDIS"));
        assert_eq!(title_to_banner("EXCLUSIVE DISTRIBUTION"), Some("EXDIS"));
        assert_eq!(title_to_banner("NO DISTRIBUTION"), Some("NODIS"));
        // §H.4 SCI compounds — title still resolves to the Authorized Banner
        // Line Abbreviation even though `banner` was corrected to the
        // abbreviation (PR #702: `banner: "SI-ECRU"` → `"SI-EU"`,
        // `banner: "SI-NONBOOK"` → `"SI-NK"`). Guards S001 long-form
        // detection semantics against future table edits.
        // CAPCO-2016 §H.4 p78, §H.4 p83, §D.1 p27.
        assert_eq!(title_to_banner("SI-ECRU"), Some("SI-EU"));
        assert_eq!(title_to_banner("SI-NONBOOK"), Some("SI-NK"));
    }

    #[test]
    fn title_lookups_return_none_for_dea_sensitive() {
        // CAPCO-2016 §G.1 Table 4 p36: DEA SENSITIVE has no
        // distinct banner abbreviation (`| DEA SENSITIVE | None | DSEN |`).
        // The `title == banner` guard in the lookups must skip this row
        // so S001 does not propose a no-op substitution and the parser
        // does not double-resolve the banner-form path.
        assert_eq!(title_to_portion("DEA SENSITIVE"), None);
        assert_eq!(title_to_banner("DEA SENSITIVE"), None);
    }

    #[test]
    fn title_lookups_return_none_for_unknown() {
        assert_eq!(title_to_portion("BANANAPHONE"), None);
        assert_eq!(title_to_banner("BANANAPHONE"), None);
        // A banner abbreviation string (not a title) must not match
        // title lookups.
        assert_eq!(title_to_portion("NOFORN"), None);
        assert_eq!(title_to_banner("NOFORN"), None);
    }

    #[test]
    fn no_duplicate_title_entries() {
        for (i, a) in MARKING_FORMS.iter().enumerate() {
            for (j, b) in MARKING_FORMS.iter().enumerate() {
                if i != j {
                    assert_ne!(a.title, b.title, "duplicate title entry: {:?}", a.title);
                }
            }
        }
    }

    #[test]
    fn title_equal_banner_rows_are_the_documented_set() {
        // Guards against silent additions of `title == banner` rows. The set
        // below is the closed list of CAPCO §G.1 Table 4 markings whose
        // abbreviation column is `None` (banner doubles as title) or whose
        // §H.4 / §G.1 row is same-form-all-three (documentation-only). Any
        // new row that lands in this shape MUST be added here AND have its
        // S001 / E001 / E009 behavior pinned in the rule-crate test suite.
        let mut same_form: Vec<&'static str> = MARKING_FORMS
            .iter()
            .filter(|f| f.title == f.banner)
            .map(|f| f.title)
            .collect();
        same_form.sort_unstable();
        let mut expected = vec![
            // §H.7 NATO classifications (banner == title, distinct portion).
            // CAPCO-2016 §G.1 Table 4 p37.
            "COSMIC TOP SECRET",
            "NATO SECRET",
            "NATO CONFIDENTIAL",
            "NATO RESTRICTED",
            "NATO UNCLASSIFIED",
            // §H.7 NATO programs (same-form-all-three documentation rows).
            // CAPCO-2016 §G.1 Table 4 p37.
            "ATOMAL",
            "BALK",
            "BOHEMIA",
            // §H.8 same-form-all-three (CVE-Value-bridge anchor):
            // CVE Value `DISPLAYONLY` → portion `DISPLAY ONLY`, banner
            // `DISPLAY ONLY` per CAPCO-2016 §H.8 p163.
            "DISPLAY ONLY",
            // §H.8 EYES ONLY: title == banner ("EYES ONLY") but portion is
            // distinct ("EYES", the CVE value). The bare banner form is the
            // same as the ODNI description title; no distinct banner
            // abbreviation exists (§G.1 Table 4 abbreviation column is
            // effectively `None` for this deprecated marking). S001 must
            // NOT fire (title == banner means title_to_banner returns None,
            // so no substitution is possible). E001/E009 are retired.
            // CAPCO-2016 §H.8 p157; ODNI CVEnumISMDissem.xml.
            "EYES ONLY",
            // §H.8 dissem with `None` abbreviation column.
            // CAPCO-2016 §G.1 Table 4 p36 (DEA SENSITIVE).
            "DEA SENSITIVE",
        ];
        expected.sort_unstable();
        assert_eq!(
            same_form, expected,
            "MARKING_FORMS title==banner row set drifted. Update this test \
             when adding/removing a documentation row, and audit S001 / \
             E001 / E009 pin-down tests for the affected marking."
        );
    }
}
