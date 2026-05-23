// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::Classification;

// ---------------------------------------------------------------------------
// NATO classification
// ---------------------------------------------------------------------------

/// NATO classification ladder.
///
/// NATO uses a separate classification system governed by treaty.
/// Not everyone with a US clearance is cleared for NATO; many US systems
/// are not approved for NATO information.
///
/// # Canonical structural model
///
/// Per CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking),
/// ATOMAL / BOHEMIA / BALK are **registered NATO control markings,
/// not NATO classifications** — each has its own ARH row with
/// `Requires {marking} read-in`. The §H.7 p122 + §H.7 p127 worked
/// examples place them in their proper category positions:
///
/// - **ATOMAL** is an AEA-axis marking shared with NATO+UK under
///   §123/§144 sharing agreements (Atomic Energy Act). It travels
///   alongside RD/FRD/TFNI in the AEA block, carried by
///   [`AeaMarking::Atomal`]. Canonical portion: `(//CTS//ATOMAL)` or
///   `(//NS//ATOMAL)`, not the legacy `CTSA` / `NSAT` portion-suffix.
/// - **BOHEMIA** and **BALK** are NATO Special Access Programs in
///   the SCI category position, carried by
///   [`SciControlSystem::NatoSap`]. They render standalone with no
///   `SAR-` prefix. Canonical portion: `(//CTS//BOHEMIA)` or
///   `(//CTS//BALK)`, not the legacy `CTS-B` / `CTS-BALK`
///   portion-suffix.
///
/// Pre-PR-9c.1 carried five fused variants — `NatoConfidentialAtomal`,
/// `NatoSecretAtomal`, `CosmicTopSecretAtomal`,
/// `CosmicTopSecretBohemia`, `CosmicTopSecretBalk` — which conflated
/// classification with AEA/SCI semantics on a single axis. The
/// parser ([`crate::parser::parse_nato_classification`] in marque-core)
/// canonicalizes legacy text at parse time; the E066 autofix rule
/// rewrites the source text to the canonical multi-block form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NatoClassification {
    NatoUnclassified, // NU
    NatoRestricted,   // NR
    NatoConfidential, // NC
    NatoSecret,       // NS
    CosmicTopSecret,  // CTS
}

impl NatoClassification {
    /// Banner form (full words, as used in banner marking lines).
    pub fn banner_str(self) -> &'static str {
        match self {
            Self::NatoUnclassified => "NATO UNCLASSIFIED",
            Self::NatoRestricted => "NATO RESTRICTED",
            Self::NatoConfidential => "NATO CONFIDENTIAL",
            Self::NatoSecret => "NATO SECRET",
            Self::CosmicTopSecret => "COSMIC TOP SECRET",
        }
    }

    /// Portion form (primary abbreviation from the CAPCO Register).
    pub fn portion_str(self) -> &'static str {
        match self {
            Self::NatoUnclassified => "NU",
            Self::NatoRestricted => "NR",
            Self::NatoConfidential => "NC",
            Self::NatoSecret => "NS",
            Self::CosmicTopSecret => "CTS",
        }
    }

    /// The base classification level, for ordering comparisons.
    ///
    /// `base_level` is a trivial mapping today (each variant is its own
    /// base level), but the indirection stays for API stability and as
    /// a hook for any future sub-level distinctions.
    pub fn base_level(self) -> NatoLevel {
        match self {
            Self::NatoUnclassified => NatoLevel::NatoUnclassified,
            Self::NatoRestricted => NatoLevel::NatoRestricted,
            Self::NatoConfidential => NatoLevel::NatoConfidential,
            Self::NatoSecret => NatoLevel::NatoSecret,
            Self::CosmicTopSecret => NatoLevel::CosmicTopSecret,
        }
    }

    /// Map the NATO level to the equivalent US classification for conflict
    /// resolution (US wins at the greater of the two).
    pub fn us_equivalent(self) -> Classification {
        match self.base_level() {
            NatoLevel::NatoUnclassified => Classification::Unclassified,
            NatoLevel::NatoRestricted => Classification::Restricted,
            NatoLevel::NatoConfidential => Classification::Confidential,
            NatoLevel::NatoSecret => Classification::Secret,
            NatoLevel::CosmicTopSecret => Classification::TopSecret,
        }
    }
}

/// NATO classification level without SAP, for ordering comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NatoLevel {
    NatoUnclassified,
    NatoRestricted,
    NatoConfidential,
    NatoSecret,
    CosmicTopSecret,
}
