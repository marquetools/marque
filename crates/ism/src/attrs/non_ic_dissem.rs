// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// ===========================================================================
// Non-IC dissemination controls
// ===========================================================================

/// Non-Intelligence Community dissemination control markings (CAPCO-2016 §H.9 pp 169–191).
///
/// These operate under a separate authority framework from IC dissem controls.
/// In classified documents, most non-IC dissem controls appear **only in portion
/// markings** — they are stripped from banners. However, some controls propagate
/// to classified banners: EXDIS, NODIS, LES, LES-NF, SSI, and NNPI. See
/// [`NonIcDissem::propagates_to_classified_banner`] for the authoritative list.
/// When the page is **unclassified**, all non-IC dissem controls propagate to
/// the banner.
///
/// LES-NF and SBU-NF carry NOFORN treatment even when stripped from the banner.
///
/// # CUI note
///
/// CUI (Controlled Unclassified Information) is recognized but not validated.
/// Full CUI rule support is planned for a dedicated crate. The IC equivalent
/// (FOUO) remains in active use in the `DissemControl` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum NonIcDissem {
    /// LIMITED DISTRIBUTION / LIMDIS / DS
    Limdis,
    /// EXCLUSIVE DISTRIBUTION / EXDIS / XD
    Exdis,
    /// NO DISTRIBUTION / NODIS / ND
    Nodis,
    /// SENSITIVE BUT UNCLASSIFIED / SBU / SBU
    Sbu,
    /// SENSITIVE BUT UNCLASSIFIED NOFORN / SBU NOFORN / SBU-NF
    /// Carries NOFORN treatment even when stripped from banner.
    SbuNf,
    /// LAW ENFORCEMENT SENSITIVE / LES / LES
    Les,
    /// LAW ENFORCEMENT SENSITIVE NOFORN / LES NOFORN / LES-NF
    /// Carries NOFORN treatment even when stripped from banner.
    LesNf,
    /// SENSITIVE SECURITY INFORMATION / SSI / SSI
    Ssi,
    /// NAVAL NUCLEAR PROPULSION INFORMATION / NNPI / NNPI
    ///
    /// Registered as a non-IC dissemination control in ODNI's
    /// `CVEnumISMNonIC.xml` (value `NNPI`, description
    /// `NAVAL NUCLEAR PROPULSION INFORMATION`). CAPCO-2016 §G.1
    /// Table 4 (Register of Authorized Markings) and §H.9 (Non-IC
    /// Dissemination Control Markings) do not enumerate NNPI,
    /// because the marking is governed by separate statutory
    /// authority (10 USC 7314 / 50 USC 2511; DOE / Naval Nuclear
    /// Propulsion Program) rather than IC marking policy. Per
    /// Constitution VIII, an ODNI-registered token whose authority
    /// lives outside CAPCO is admissible via the ODNI schema
    /// citation when the normative CAPCO sections cited above are
    /// silent on the marking itself.
    ///
    /// NNPI is a subject-matter marking — it identifies the
    /// information's content domain (naval nuclear propulsion plant
    /// reactor safety and radioactivity controls) rather than an
    /// access-control overlay. NNPI is "DO propagate" for
    /// `propagates_to_classified_banner` because NNPI can appear in
    /// either classified or unclassified contexts (unlike the
    /// LIMDIS / SBU / SBU-NF cluster, which is explicitly
    /// unclassified-only per §H.9 p178). When NNPI appears in any
    /// portion of a classified document, the banner must surface it
    /// — the marking identifies the information itself, not an
    /// optional access restriction. As with all non-IC dissem
    /// controls, classified+NNPI absent an explicit FD&R decision
    /// is implicitly NOFORN per the §B.3 p20 Note caveated-default
    /// (the IC cannot presume releasability or RELIDO-suitability of
    /// information governed by policy regimes outside IC marking
    /// authority); the closure-rule wiring lives in
    /// `crates/capco/src/scheme/closure.rs` under the row whose
    /// `name` field is `"capco/noforn-if-caveated"`.
    Nnpi,
}

impl NonIcDissem {
    /// Banner-line abbreviation form.
    pub fn banner_str(self) -> &'static str {
        match self {
            Self::Limdis => "LIMDIS",
            Self::Exdis => "EXDIS",
            Self::Nodis => "NODIS",
            Self::Sbu => "SBU",
            Self::SbuNf => "SBU NOFORN",
            Self::Les => "LES",
            Self::LesNf => "LES NOFORN",
            Self::Ssi => "SSI",
            Self::Nnpi => "NNPI",
        }
    }

    /// Portion mark abbreviation.
    pub fn portion_str(self) -> &'static str {
        match self {
            Self::Limdis => "DS",
            Self::Exdis => "XD",
            Self::Nodis => "ND",
            Self::Sbu => "SBU",
            Self::SbuNf => "SBU-NF",
            Self::Les => "LES",
            Self::LesNf => "LES-NF",
            Self::Ssi => "SSI",
            Self::Nnpi => "NNPI",
        }
    }

    /// Parse from either banner or portion form.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "LIMDIS" | "DS" => Some(Self::Limdis),
            "EXDIS" | "XD" => Some(Self::Exdis),
            "NODIS" | "ND" => Some(Self::Nodis),
            "SBU" => Some(Self::Sbu),
            "SBU NOFORN" | "SBU-NF" => Some(Self::SbuNf),
            "LES" => Some(Self::Les),
            "LES NOFORN" | "LES-NF" => Some(Self::LesNf),
            "SSI" => Some(Self::Ssi),
            // Banner and portion forms collide on `NNPI` (same
            // single-token form for both, per the `MARKING_FORMS`
            // row at `crates/ism/src/marking_forms.rs`). The long
            // form `NAVAL NUCLEAR PROPULSION INFORMATION` resolves
            // to `NNPI` via `parse_non_ic_full_form`'s
            // `title_to_portion` fallback, not here.
            "NNPI" => Some(Self::Nnpi),
            _ => None,
        }
    }

    /// Returns true if this control carries NOFORN treatment.
    pub fn carries_noforn(self) -> bool {
        matches!(self, Self::SbuNf | Self::LesNf)
    }

    /// Returns true if this control propagates to classified banners.
    ///
    /// Authoritative source: `crates/capco/docs/CAPCO-2016.md` §H.9
    /// "Precedence Rules for Banner Line Guidance" for each marking.
    /// The per-marking rows below cite the specific line of the vendored
    /// manual.
    ///
    /// | Marking  | Propagates | Source (CAPCO-2016 §H.9)                                                                                            |
    /// |----------|------------|----------------------------------------------------------------------------------------------------------------------|
    /// | LIMDIS   | no         | §H.9 p170: "When a document contains LIMDIS and classified portions, LIMDIS is not used in the banner line."         |
    /// | EXDIS    | yes        | §H.9 p172: "If EXDIS is contained in any portion … EXDIS must appear in the banner line." Example banner: `SECRET//NOFORN//EXDIS` |
    /// | NODIS    | yes        | §H.9 p174: "If NODIS is contained in any portion of a document, it must appear in the banner line." Example banner: `SECRET//NOFORN//NODIS` |
    /// | SBU      | no         | §H.9 p176: "When a document contains SBU and classified portions, SBU is not used in the banner line."               |
    /// | SBU-NF   | no (†)     | §H.9 p178: SBU NOFORN "Applicable only to unclassified information." (The §H.9 notional example on p179 shows a `SECRET//NOFORN` banner with a `(U//SBU-NF)` portion — SBU-NF absent from banner.) |
    /// | LES      | yes        | §H.9 p181: "The LES marking always appears in the banner line if contained in any portion, regardless of classification level." |
    /// | LES-NF   | yes (*)    | §H.9 p185: "The LES marking always appears in the banner line if LES information (either LES or LES NOFORN) is contained in the document, regardless of the document's classification level." |
    /// | SSI      | yes        | §H.9 p189: "If the SSI marking is contained in any portion of a document it must appear in the banner line, regardless of the document's overall classification level." |
    /// | NNPI     | yes (‡)    | Not in §H.9. ODNI `CVEnumISMNonIC.xml` registers NNPI as a non-IC dissem control; 10 USC 7314 / 50 USC 2511 (Naval Nuclear Propulsion Program) governs the marking itself. NNPI can appear in either classified or unclassified contexts (unlike the LIMDIS / SBU / SBU-NF cluster, which §H.9 p170 / p176 / p178 restrict to unclassified-only). When NNPI appears in any portion of a classified document, the banner must surface it — the marking identifies the information's content domain (naval nuclear propulsion plant reactor safety + radioactivity controls), not an optional access overlay. |
    ///
    /// (*) LES-NF carries a §H.9 canonicalization that is **not modeled
    ///     here**: in classified docs, `LES NOFORN` → `LES` at the banner
    ///     with NOFORN split into the dissem block (§H.9 p185: "the 'LES'
    ///     marking is used in the banner line and the NOFORN marking is
    ///     applied as a Dissemination Control Marking. For example:
    ///     `SECRET//NOFORN//LES`."). The split itself lives in the
    ///     scheme-side lattice path (`NonIcDissemSet::from_attrs_iter`
    ///     in `marque-capco::lattice`); this predicate only answers
    ///     the binary "does the marking appear in the banner at all?"
    ///     question, which is what W003 consumes. Treating
    ///     `SECRET//LES NOFORN` as non-canonical (so that the
    ///     canonicalization becomes fixable) is a separate
    ///     page-rewrite concern, not a W003 concern.
    ///
    /// (‡) NNPI does not have a §H.9 row at all — its authority lives
    ///     outside CAPCO's IC-marking scope (10 USC 7314 / 50 USC
    ///     §2511). Per Constitution VIII, citing the ODNI schema
    ///     when the normative CAPCO sections cited above are silent
    ///     on the marking itself is admissible.
    ///
    /// (†) "Does not propagate" for SBU-NF refers to the **SBU** half of
    ///     the marking — the literal `SBU NOFORN` banner form is
    ///     non-canonical in a classified document per §H.9 p178
    ///     ("applicable only to unclassified information"). The **NOFORN
    ///     half does propagate** via
    ///     `NonIcDissemSet::from_attrs_iter` in `marque-capco::lattice`,
    ///     which splits a portion-level `SBU-NF` into `SBU + NF-flag`
    ///     and emits the resulting NOFORN into the classified banner's
    ///     dissem block. So a document with a `(U//SBU-NF)` portion
    ///     rolls up to a `SECRET//NOFORN` banner (NOFORN present, SBU
    ///     dropped), not `SECRET//SBU NOFORN`. W003 therefore fires on
    ///     the literal `SECRET//SBU NOFORN` banner input because that
    ///     surface form is the non-canonical one, not because NOFORN
    ///     is disallowed.
    pub fn propagates_to_classified_banner(self) -> bool {
        match self {
            // Do NOT propagate — banner-absent in classified documents.
            Self::Limdis | Self::Sbu | Self::SbuNf => false,
            // DO propagate — "must appear in the banner line" per §H.9.
            //
            // NNPI propagates: it is a subject-matter identification
            // marking (10 USC 7314 / 50 USC 2511 Naval Nuclear
            // Propulsion Program), not an optional access overlay,
            // so any portion carrying NNPI requires the banner to
            // surface it as well. ODNI's `CVEnumISMNonIC.xml`
            // registers it alongside the §H.9 propagating set; the
            // governing statutes treat banner visibility as
            // mandatory for any document containing NNPI content.
            Self::Exdis | Self::Nodis | Self::Les | Self::LesNf | Self::Ssi | Self::Nnpi => true,
        }
    }

    /// All valid values.
    pub const ALL: &[NonIcDissem] = &[
        Self::Limdis,
        Self::Exdis,
        Self::Nodis,
        Self::Sbu,
        Self::SbuNf,
        Self::Les,
        Self::LesNf,
        Self::Ssi,
        Self::Nnpi,
    ];
}

impl std::fmt::Display for NonIcDissem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.portion_str())
    }
}
