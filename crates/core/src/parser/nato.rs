use super::*;

/// Parse a NATO classification string in either banner form (`"NATO SECRET"`,
/// `"COSMIC TOP SECRET"`, etc.) or portion form (`"NS"`, `"CTS"`, etc.).
///
/// Parse a NATO classification block.
///
/// Returns a [`NatoBlock`] carrying the bare NATO classification level
/// (the canonical structural form, never the legacy compound variants)
/// plus optional companion writes for either the AEA axis (ATOMAL per
/// CAPCO-2016 §H.7 p122) or the SCI axis (BALK / BOHEMIA per §G.2 p40
/// + §H.7 p127).
///
/// # Legacy text canonicalization (PR 9c.1 T134)
///
/// Pre-PR-9c.1 the legacy text forms `CTSA` / `NSAT` / `NCA` /
/// `CTS-A` / `NS-A` / `NC-A` / `CTS-B` / `CTS-BALK` parsed into
/// fused `NatoClassification::*Atomal` / `*Bohemia` / `*Balk`
/// variants. Per CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered
/// Marking) those forms are **structurally wrong**: ATOMAL,
/// BOHEMIA, and BALK each have their own ARH row at §G.2 p40,
/// confirming they are registered control markings, not classification
/// suffixes. The §H.7 p122 worked example
/// (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`) places ATOMAL in the AEA
/// axis alongside RD; the §H.7 p127 worked example
/// (`TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN`) places
/// BOHEMIA in the SCI category position.
///
/// The legacy compound variants were retired in PR 9c.1 Commit 5;
/// this parser canonicalizes the legacy text at parse time so
/// existing markings produce the correct structural canonical form.
/// Marque's autofix channel (rule E066 in `marque-capco/src/rules.rs`)
/// drives the source-text re-marking when the rule severity is
/// configured to fire.
///
/// Longer patterns are checked first to avoid prefix ambiguity
/// (e.g., `"COSMIC TOP SECRET ATOMAL"` before `"COSMIC TOP SECRET"`).
pub(super) fn parse_nato_classification(s: &str) -> Option<NatoBlock> {
    use marque_ism::{AtomalBlock, NatoSap};
    // Check longer patterns first to avoid prefix matches.
    let (class, companion) = match s {
        // Banner forms (full words) — longer patterns first.
        // Companion AEA = ATOMAL; companion SCI = BALK / BOHEMIA.
        "COSMIC TOP SECRET ATOMAL" => (
            NatoClassification::CosmicTopSecret,
            NatoCompanion::Aea(AeaMarking::Atomal(AtomalBlock)),
        ),
        "COSMIC TOP SECRET-BOHEMIA" => (
            NatoClassification::CosmicTopSecret,
            NatoCompanion::Sci(NatoSap::Bohemia),
        ),
        "COSMIC TOP SECRET-BALK" => (
            NatoClassification::CosmicTopSecret,
            NatoCompanion::Sci(NatoSap::Balk),
        ),
        "COSMIC TOP SECRET" => (NatoClassification::CosmicTopSecret, NatoCompanion::Bare),
        "NATO SECRET ATOMAL" => (
            NatoClassification::NatoSecret,
            NatoCompanion::Aea(AeaMarking::Atomal(AtomalBlock)),
        ),
        "NATO SECRET" => (NatoClassification::NatoSecret, NatoCompanion::Bare),
        "NATO CONFIDENTIAL ATOMAL" => (
            NatoClassification::NatoConfidential,
            NatoCompanion::Aea(AeaMarking::Atomal(AtomalBlock)),
        ),
        "NATO CONFIDENTIAL" => (NatoClassification::NatoConfidential, NatoCompanion::Bare),
        "NATO RESTRICTED" => (NatoClassification::NatoRestricted, NatoCompanion::Bare),
        "NATO UNCLASSIFIED" => (NatoClassification::NatoUnclassified, NatoCompanion::Bare),
        // Portion forms — primary (CAPCO Register) + legacy compounds.
        // Legacy `CTSA` / `CTS-A` / `NSAT` / `NS-A` / `NCA` / `NC-A`
        // canonicalize to bare class + AEA::Atomal companion.
        // Legacy `CTS-B` / `CTS-BALK` canonicalize to bare class + SCI
        // (BOHEMIA / BALK NatoSap) companion.
        "CTSA" | "CTS-A" => (
            NatoClassification::CosmicTopSecret,
            NatoCompanion::Aea(AeaMarking::Atomal(AtomalBlock)),
        ),
        "CTS-B" => (
            NatoClassification::CosmicTopSecret,
            NatoCompanion::Sci(NatoSap::Bohemia),
        ),
        "CTS-BALK" => (
            NatoClassification::CosmicTopSecret,
            NatoCompanion::Sci(NatoSap::Balk),
        ),
        "CTS" => (NatoClassification::CosmicTopSecret, NatoCompanion::Bare),
        "NSAT" | "NS-A" => (
            NatoClassification::NatoSecret,
            NatoCompanion::Aea(AeaMarking::Atomal(AtomalBlock)),
        ),
        "NS" => (NatoClassification::NatoSecret, NatoCompanion::Bare),
        "NCA" | "NC-A" => (
            NatoClassification::NatoConfidential,
            NatoCompanion::Aea(AeaMarking::Atomal(AtomalBlock)),
        ),
        "NC" => (NatoClassification::NatoConfidential, NatoCompanion::Bare),
        "NR" => (NatoClassification::NatoRestricted, NatoCompanion::Bare),
        "NU" => (NatoClassification::NatoUnclassified, NatoCompanion::Bare),
        _ => return None,
    };
    Some(NatoBlock { class, companion })
}

/// Result of parsing a NATO classification block.
///
/// The block carries the canonical bare NATO class plus an optional
/// companion write to either the AEA axis (ATOMAL) or the SCI axis
/// (BALK / BOHEMIA). PR 9c.1 T134 introduced this split so the
/// classification axis no longer fuses sub-markings into wrong
/// classification-variant identities; see [`parse_nato_classification`]
/// doc for the rationale.
pub(super) struct NatoBlock {
    pub(super) class: NatoClassification,
    pub(super) companion: NatoCompanion,
}

/// Optional companion writes for [`NatoBlock`]. At most one — ATOMAL
/// can in principle co-occur with BALK/BOHEMIA on the same portion
/// (`(//CTS//BOHEMIA//ATOMAL)`), but that would arrive as a
/// well-formed multi-block input (separate `//`-blocks for the
/// classification and the SCI/AEA marker), not as a legacy compound
/// text. The fused legacy text always carries exactly one companion.
pub(super) enum NatoCompanion {
    /// Bare NATO classification with no AEA/SCI companion — `(//CTS)`,
    /// `(//NS)`, `(//NC)`, `(//NR)`, `(//NU)`, plus their banner
    /// equivalents. The "bare" framing matches the
    /// `nato-dissem-reciprocity` invariant (pure-NATO portions carry
    /// NATO classification + NATO dissem, no fused control marking).
    /// Renamed from `None` in PR 9c.1 to avoid shadowing the language
    /// keyword and to mirror the project's "bare" terminology.
    Bare,
    /// AEA companion — ATOMAL per CAPCO-2016 §H.7 p122. Written into
    /// `attrs.aea_markings`.
    Aea(AeaMarking),
    /// SCI companion — BALK / BOHEMIA per CAPCO-2016 §G.2 p40 +
    /// §H.7 p127. Written into `attrs.sci_markings` as a structural
    /// [`SciMarking`] whose `system` is the corresponding
    /// [`marque_ism::NatoSap`] variant.
    Sci(marque_ism::NatoSap),
}

/// Parse a JOINT classification block: `"JOINT S USA GBR"` or `"JOINT SECRET USA GBR"`.
///
/// Format: `JOINT` + classification level + space-delimited country trigraphs.
/// Countries are space-delimited (NOT comma-delimited like REL TO).
pub(super) fn parse_joint_classification(s: &str) -> Option<JointClassification> {
    let rest = s.strip_prefix("JOINT ")?;
    let mut tokens = rest.split_whitespace();

    // First token(s) after JOINT are the classification level.
    // Handle two-word levels like "TOP SECRET".
    let first = tokens.next()?;
    let (level, remaining_start) = if first == "TOP" {
        // Check if next token is "SECRET" to form "TOP SECRET"
        let mut peek_tokens = rest.split_whitespace();
        peek_tokens.next(); // skip "TOP"
        if peek_tokens.next() == Some("SECRET") {
            let level = parse_classification("TOP SECRET")?;
            // Skip past "TOP SECRET" — countries start after
            let after_ts = rest.find("SECRET").map(|i| i + "SECRET".len())?;
            (level, after_ts)
        } else {
            return None; // "TOP" alone is not a valid level
        }
    } else {
        let level = parse_classification(first)?;
        let after_level = rest.find(first).map(|i| i + first.len())?;
        (level, after_level)
    };

    // Remaining tokens are space-delimited country trigraphs.
    //
    // NOTE: JOINT classifications today drop non-3-byte tokens
    // silently (tetragraphs like NATO never appear in real JOINT
    // markings, but the parallel of issue #183's REL TO silent-drop
    // is tracked as deferred scope for PR-B / a future issue).
    let country_str = rest[remaining_start..].trim();
    // Inline-4 covers Five Eyes (USA, GBR, CAN, AUS, NZL) and typical
    // bilateral / trilateral JOINT markings; larger coalition lists
    // spill cleanly. Mirrors the `FgiMarker::Acknowledged` inline-4
    // sizing.
    let mut countries: SmallVec<[CountryCode; 4]> = SmallVec::new();
    for token in country_str.split_whitespace() {
        if token.len() == 3 {
            if let Some(t) = CountryCode::try_new(token.as_bytes()) {
                countries.push(t);
            }
        }
    }

    if countries.is_empty() {
        return None; // JOINT must have at least one country
    }

    Some(JointClassification {
        level,
        countries: countries.into_boxed_slice(),
    })
}

/// Parse an FGI classification block: `"GBR S"`, `"DEU TS"`, `"GBR DEU S"`,
/// or `"FGI S"` (FGI as placeholder for unknown country).
///
/// Format: one or more country trigraphs (or "FGI") + classification level.
/// Countries are space-delimited. The last token is the classification level.
///
/// Multi-country FGI at the classification axis is authoritative per
/// CAPCO-2016 §H.7 p123 worked example `(//CAN GBR S)` and the §H.7 p124
/// prose ("Multiple FGI countries must be listed alphabetically and
/// separated by a single space"; ICD 206 commingling clause).
///
/// Returns `None` if no classification level is found (e.g., bare `"FGI"` with
/// no level — that's an error, not a valid FGI classification).
pub(super) fn parse_fgi_classification(s: &str) -> Option<FgiClassification> {
    // Inline-4 covers `<country> <level>`, `<country> <country> <level>`,
    // and `<country> TOP SECRET`; longer multi-country FGI is rare in
    // practice and spills cleanly.
    let tokens: SmallVec<[&str; 4]> = s.split_whitespace().collect();
    if tokens.len() < 2 {
        return None; // Need at least country + level
    }

    // Last token is the classification level. Handle "TOP SECRET" as two tokens.
    let (level, country_end) = if tokens.len() >= 3
        && tokens[tokens.len() - 2] == "TOP"
        && tokens[tokens.len() - 1] == "SECRET"
    {
        (parse_classification("TOP SECRET")?, tokens.len() - 2)
    } else {
        (
            parse_classification(tokens[tokens.len() - 1])?,
            tokens.len() - 1,
        )
    };

    // Preceding tokens are country trigraphs (or "FGI" placeholder).
    // Inline-4 mirrors the `FgiMarker::Acknowledged` country buffer;
    // FGI rarely lists more than 2-3 source countries.
    let mut countries: SmallVec<[CountryCode; 4]> = SmallVec::new();
    for &token in &tokens[..country_end] {
        if token == "FGI" {
            // FGI as placeholder for unknown country — countries stays empty
            continue;
        }
        if token.len() == 3 {
            let t = CountryCode::try_new(token.as_bytes())?;
            countries.push(t);
        } else {
            return None; // Not a trigraph or "FGI"
        }
    }

    Some(FgiClassification {
        countries: countries.into_boxed_slice(),
        level,
    })
}

/// # PR 9c.1 T134 — companion drop in conflict scenarios
///
/// Post-PR-9c.1, [`parse_nato_classification`] returns a `NatoBlock`
/// carrying the bare NATO class plus an optional AEA/SCI companion.
/// In the conflict-scenario fallback (this function), the companion
/// is **dropped** — the conflict path stores only the foreign
/// classification axis; there's no `ForeignClassification` carrier
/// shape for companion AEA/SCI writes. A conflict input like
/// `SECRET//CTSA//NOFORN` records the foreign class as `CosmicTopSecret`
/// (the bare class) and loses the implicit ATOMAL companion.
///
/// This is acceptable for PR 9c.1 because:
///   - The conflict shape itself is malformed input (US + foreign
///     classifications shouldn't both appear), and dedicated rules
///     surface that condition first.
///   - The E066 legacy-text autofix reads raw source text via
///     `attrs.token_spans`, so the legacy-text suggestion still
///     fires even when the structural companion is dropped here.
///   - If a future revision requires conflict-aware companion
///     plumbing, the change is local to `ForeignClassification` +
///     this function — no rule-surface impact.
pub(super) fn try_parse_foreign_classification(s: &str) -> Option<ForeignClassification> {
    if let Some(nato_block) = parse_nato_classification(s) {
        // Drop the companion in the conflict path; see fn doc above.
        Some(ForeignClassification::Nato(nato_block.class))
    } else if let Some(joint) = parse_joint_classification(s) {
        Some(ForeignClassification::Joint(joint))
    } else {
        parse_fgi_classification(s).map(ForeignClassification::Fgi)
    }
}
