// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// Atomic Energy Act markings
// ---------------------------------------------------------------------------

/// Atomic Energy Act information markings (CAPCO-2016 §H.6 pp 103–121).
///
/// AEA markings appear as a single `//`-delimited block in the marking string,
/// using hyphen separators for compound forms:
/// - `SECRET//RD//NOFORN` — RD alone
/// - `SECRET//RD-CNWDI//NOFORN` — RD with CNWDI modifier
/// - `SECRET//RD-SIGMA 20//NOFORN` — RD with SIGMA compartment
/// - `SECRET//RD-SIGMA 18 20//NOFORN` — RD with multiple SIGMAs
/// - `SECRET//FRD//NOFORN` — FRD alone
/// - `SECRET//FRD-SIGMA 14//NOFORN` — FRD with SIGMA
///
/// Standalone (non-compound) markings:
/// - `UNCLASSIFIED//DOD UCNI` / `(U//DCNI)`
/// - `UNCLASSIFIED//DOE UCNI` / `(U//UCNI)`
/// - `SECRET//TFNI//NOFORN` / `(S//TFNI//NF)`
///
/// # Key rules (CAPCO-2016)
///
/// - RD and FRD always require NOFORN unless a sharing agreement exists
///   (default severity: Error, configurable to Warn via `.marque.toml`)
/// - CNWDI may only be used with TS or S RD (not standalone, not with FRD)
/// - SIGMA 14, 15, 18, 20 may only be used with TS or S RD or FRD
/// - RD takes precedence over FRD and TFNI in both banners and portions
/// - SIGMA numbers must be in numerical order, space-separated
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AeaMarking {
    /// Compound RD block: `RD`, `RD-CNWDI`, `RD-SIGMA 20`, `RD-CNWDI-SIGMA 18 20`
    Rd(RdBlock),
    /// Compound FRD block: `FRD`, `FRD-SIGMA 14`
    Frd(FrdBlock),
    /// DOD UCNI / DCNI — standalone, unclassified only
    DodUcni,
    /// DOE UCNI / UCNI — standalone, unclassified only
    DoeUcni,
    /// TFNI — standalone
    Tfni,
    /// ATOMAL — NATO Atomic Energy Act information shared with the US
    /// and UK under bilateral §123 / §144 agreements (Atomic Energy
    /// Act §141–§144). Travels in the AEA axis alongside RD/FRD/TFNI
    /// (CAPCO-2016 §H.7 p122 worked example:
    /// `SECRET//RD/ATOMAL//FGI NATO//NOFORN`). Rendered as `ATOMAL`
    /// in both banner and portion forms.
    ///
    /// `AtomalBlock` is currently empty — ATOMAL has no registered
    /// sub-markings in CAPCO-2016 §H.7. The block carrier mirrors
    /// [`RdBlock`] / [`FrdBlock`] so a future CAPCO publication that
    /// grammar-extends ATOMAL with sub-markings remains a planned
    /// migration rather than a breaking-shape change.
    Atomal(AtomalBlock),
}

/// Restricted Data block with optional modifiers.
///
/// Rendered as `RD`, `RD-CNWDI`, `RD-SIGMA 20`, or `RD-CNWDI-SIGMA 18 20`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RdBlock {
    /// Whether CNWDI is present. Only valid with TS or S classification.
    pub cnwdi: bool,
    /// SIGMA compartment numbers (14, 15, 18, 20). Must be in numerical order.
    /// Empty if no SIGMA designation.
    pub sigma: Box<[u8]>,
}

impl Default for RdBlock {
    fn default() -> Self {
        Self {
            cnwdi: false,
            sigma: Box::new([]),
        }
    }
}

/// Formerly Restricted Data block with optional SIGMA modifier.
///
/// Rendered as `FRD` or `FRD-SIGMA 14`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrdBlock {
    /// SIGMA compartment numbers. Must be in numerical order.
    /// Empty if no SIGMA designation.
    pub sigma: Box<[u8]>,
}

impl Default for FrdBlock {
    fn default() -> Self {
        Self {
            sigma: Box::new([]),
        }
    }
}

/// ATOMAL block.
///
/// Currently empty: CAPCO-2016 §G.2 p40 + §H.7 p122 register
/// ATOMAL as a standalone control marking with no enumerated
/// sub-markings. The carrier struct mirrors the [`RdBlock`] /
/// [`FrdBlock`] shape so that adding sub-markings in a future CAPCO
/// publication remains a planned, intentional grammar extension
/// rather than a structural-shape change at the variant level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AtomalBlock;

impl AeaMarking {
    /// Banner-line form.
    pub fn banner_str(&self) -> String {
        match self {
            Self::Rd(rd) => {
                let mut s = "RD".to_owned();
                if rd.cnwdi {
                    s.push_str("-CNWDI");
                }
                if !rd.sigma.is_empty() {
                    s.push_str("-SIGMA ");
                    let nums: Vec<String> = rd.sigma.iter().map(|n| n.to_string()).collect();
                    s.push_str(&nums.join(" "));
                }
                s
            }
            Self::Frd(frd) => {
                let mut s = "FRD".to_owned();
                if !frd.sigma.is_empty() {
                    s.push_str("-SIGMA ");
                    let nums: Vec<String> = frd.sigma.iter().map(|n| n.to_string()).collect();
                    s.push_str(&nums.join(" "));
                }
                s
            }
            Self::DodUcni => "DOD UCNI".to_owned(),
            Self::DoeUcni => "DOE UCNI".to_owned(),
            Self::Tfni => "TFNI".to_owned(),
            Self::Atomal(_) => "ATOMAL".to_owned(),
        }
    }

    /// Portion mark form.
    pub fn portion_str(&self) -> String {
        match self {
            Self::Rd(rd) => {
                let mut s = "RD".to_owned();
                if rd.cnwdi {
                    s.push_str("-CNWDI");
                }
                if !rd.sigma.is_empty() {
                    s.push_str("-SG ");
                    let nums: Vec<String> = rd.sigma.iter().map(|n| n.to_string()).collect();
                    s.push_str(&nums.join(" "));
                }
                s
            }
            Self::Frd(frd) => {
                let mut s = "FRD".to_owned();
                if !frd.sigma.is_empty() {
                    s.push_str("-SG ");
                    let nums: Vec<String> = frd.sigma.iter().map(|n| n.to_string()).collect();
                    s.push_str(&nums.join(" "));
                }
                s
            }
            Self::DodUcni => "DCNI".to_owned(),
            Self::DoeUcni => "UCNI".to_owned(),
            Self::Tfni => "TFNI".to_owned(),
            Self::Atomal(_) => "ATOMAL".to_owned(),
        }
    }

    /// Parse a `//`-delimited AEA block from either banner or portion form.
    ///
    /// Handles compound tokens: `RD`, `RD-CNWDI`, `RD-SIGMA 20`,
    /// `RD-CNWDI-SIGMA 18 20`, `FRD`, `FRD-SIGMA 14`, etc.
    pub fn parse(s: &str) -> Option<Self> {
        // Standalone non-compound markings.
        match s {
            "DOD UCNI" | "DCNI" => return Some(Self::DodUcni),
            "DOE UCNI" | "UCNI" => return Some(Self::DoeUcni),
            "TFNI" | "TRANSCLASSIFIED FOREIGN NUCLEAR INFORMATION" => return Some(Self::Tfni),
            // ATOMAL — NATO §123/§144 sharing per CAPCO-2016 §H.7 p122.
            // Banner and portion forms are identical (`ATOMAL`).
            "ATOMAL" => return Some(Self::Atomal(AtomalBlock)),
            _ => {}
        }

        // RD compound block: RD, RD-CNWDI, RD-SIGMA ##, RD-CNWDI-SIGMA ##,
        // RESTRICTED DATA, RESTRICTED DATA-CNWDI, etc.
        if s == "RD" || s == "RESTRICTED DATA" {
            return Some(Self::Rd(RdBlock::default()));
        }
        if let Some(rest) = s
            .strip_prefix("RD-")
            .or_else(|| s.strip_prefix("RESTRICTED DATA-"))
        {
            return Self::parse_rd_modifiers(rest);
        }

        // FRD compound block: FRD, FRD-SIGMA ##,
        // FORMERLY RESTRICTED DATA, etc.
        if s == "FRD" || s == "FORMERLY RESTRICTED DATA" {
            return Some(Self::Frd(FrdBlock::default()));
        }
        if let Some(rest) = s
            .strip_prefix("FRD-")
            .or_else(|| s.strip_prefix("FORMERLY RESTRICTED DATA-"))
        {
            return Self::parse_frd_modifiers(rest);
        }

        None
    }

    /// Parse RD modifiers after the `RD-` prefix.
    /// Handles: `CNWDI`, `SIGMA ##`, `CNWDI-SIGMA ##`, `SG ##`, `CNWDI-SG ##`.
    fn parse_rd_modifiers(s: &str) -> Option<Self> {
        let mut cnwdi = false;
        let mut rest = s;

        // Check for CNWDI prefix.
        if let Some(after) = rest.strip_prefix("CNWDI") {
            cnwdi = true;
            rest = after.strip_prefix('-').unwrap_or(after);
        } else if rest == "N" {
            // DoD shorthand: RD-N means RD-CNWDI (per CAPCO-2016 §6)
            return Some(Self::Rd(RdBlock {
                cnwdi: true,
                sigma: Box::new([]),
            }));
        }

        // Check for SIGMA/SG.
        let sigma = parse_sigma_numbers(rest);

        if rest.is_empty() || !sigma.is_empty() {
            Some(Self::Rd(RdBlock {
                cnwdi,
                sigma: sigma.into(),
            }))
        } else {
            None
        }
    }

    /// Parse FRD modifiers after the `FRD-` prefix.
    /// Handles: `SIGMA ##`, `SG ##`.
    fn parse_frd_modifiers(s: &str) -> Option<Self> {
        let sigma = parse_sigma_numbers(s);
        if !sigma.is_empty() {
            Some(Self::Frd(FrdBlock {
                sigma: sigma.into(),
            }))
        } else {
            None
        }
    }
}

/// Parse SIGMA/SG numbers from a string like `SIGMA 18 20` or `SG 14`.
fn parse_sigma_numbers(s: &str) -> Vec<u8> {
    let rest = s
        .strip_prefix("SIGMA ")
        .or_else(|| s.strip_prefix("SG "))
        .unwrap_or("");
    if rest.is_empty() {
        return vec![];
    }
    rest.split_whitespace()
        .filter_map(|n| n.parse::<u8>().ok())
        .collect()
}

impl std::fmt::Display for AeaMarking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.portion_str())
    }
}
