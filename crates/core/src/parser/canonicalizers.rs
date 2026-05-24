use super::*;

// =============================================================================
// Deprecated SCI long-form recognition (T135a / issue #307 Group D)
// =============================================================================

/// Result of recognizing a deprecated SCI long-form block.
///
/// Holds the parser's internal classification (canonical SCI control system
/// with optional compartment / sub-compartment context). Source bytes are
/// preserved verbatim in the caller's `TokenSpan.text` — this struct does
/// NOT rewrite the user's input. The walker rule in
/// `marque-capco`'s `rules::sci_deprecated` module
/// (`capco:portion.sci.deprecated-long-form`) consumes the original
/// `TokenSpan.text` plus this canonical projection to emit
/// `Diagnostic::text_correction` fixes.
///
/// Authority (per-variant citations in [`recognize_deprecated_sci_long_form`]):
/// CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
#[derive(Debug, Clone)]
pub(super) struct DeprecatedSciLongForm {
    /// Canonical control system this long-form maps to.
    /// `Hcs` for HUMINT-family, `Si` for COMINT / ECI / EL family,
    /// `Tk` for KDK / KLONDIKE family.
    pub(super) system: SciControlBare,
    /// Compartment identifier (e.g., `BLUEFISH` for `KDK-BLUEFISH`,
    /// `ABC` for `ECI ABC`, `ECRU` for `EL ECRU`). `None` for bare
    /// long-forms (`HUMINT`, `COMINT`, `ECI` alone, `KDK` alone).
    pub(super) compartment: Option<SmolStr>,
}

/// Recognize a deprecated SCI long-form block.
///
/// Returns `Some(form)` when `trimmed` is a recognized deprecated form.
/// The recognizer performs exact uppercase literal matches for the legacy
/// control keywords and is strict on shape for the compartment slot — the
/// compartment must be `[A-Z0-9]+` per §H.4 p61 + p76.
///
/// # Authority (per row)
///
/// - `HUMINT` / `HUMINT CONTROL SYSTEM` → HCS — CAPCO-2016 §H.4 p62
///   (Legacy section: "When incorporating legacy material marked 'HCS'
///   into a new product, re-mark the new document and associated
///   portion according to the instructions in the HCS-O and HCS-P
///   marking templates.").
/// - `COMINT` / `SPECIAL INTELLIGENCE` → SI — CAPCO-2016 §H.4 p74:
///   "The COMINT title for the Special Intelligence (SI) control
///   system is no longer valid".
/// - `ECI <COMP>` / `EXCEPTIONALLY CONTROLLED INFORMATION <COMP>` →
///   SI-`<COMP>` — CAPCO-2016 §H.4 p76: "information formerly marked
///   TS//SI-ECI ABC must now be marked TS//SI-ABC". §H.4 p61: "ECI
///   grouping markings are NOT used in banner/portion".
/// - `EL <SUB>` / `ENDSEAL <SUB>` → SI-`<SUB>` (typically `ECRU` or
///   `NONBOOK`) — CAPCO-2016 §H.4 p78: "the EL control system is
///   being retired and all associated compartments moved to the SI
///   control system". §H.4 p83 mirrors for `NONBOOK`.
/// - `KDK-<COMP>` / `KLONDIKE-<COMP>` → TK-`<COMP>` — CAPCO-2016
///   §H.4 p85 (NSG PM 3802 Closure of KLONDIKE Control System):
///   "When incorporating legacy material marked 'KLONDIKE' into a new
///   product, re-mark the new document and associated portions
///   according to the instructions in the TK-BLFH, TK-IDIT, and
///   TK-KAND marking templates."
///
/// Bare `ECI` / `ENDSEAL` / `KDK` (no compartment) are accepted — the
/// walker rule emits a suggest-only diagnostic for them because the
/// compartment context required to migrate is unknown at the parser
/// level.
pub(super) fn recognize_deprecated_sci_long_form(trimmed: &str) -> Option<DeprecatedSciLongForm> {
    // -----------------------------------------------------------------
    // HCS family — §H.4 p62
    // -----------------------------------------------------------------
    // Multi-word phrase checks must come before the single-word ones
    // (longest-prefix wins — same pattern as `parse_nato_classification`).
    if trimmed == "HUMINT CONTROL SYSTEM" || trimmed == "HUMINT" {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Hcs,
            compartment: None,
        });
    }

    // -----------------------------------------------------------------
    // SI family (COMINT / SPECIAL INTELLIGENCE) — §H.4 p74
    // -----------------------------------------------------------------
    if trimmed == "SPECIAL INTELLIGENCE" || trimmed == "COMINT" {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: None,
        });
    }

    // -----------------------------------------------------------------
    // SI family (ECI / EXCEPTIONALLY CONTROLLED INFORMATION) — §H.4 p61 + p76
    // -----------------------------------------------------------------
    // Compound forms: `ECI <COMP>` and `EXCEPTIONALLY CONTROLLED
    // INFORMATION <COMP>`. Bare `ECI` (no compartment) is also recognized;
    // the walker emits a suggest-only diagnostic because the compartment
    // is unknown.
    if let Some(comp) = trimmed.strip_prefix("EXCEPTIONALLY CONTROLLED INFORMATION ")
        && is_alnum_upper(comp)
    {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: Some(comp.into()),
        });
    }
    if let Some(comp) = trimmed.strip_prefix("ECI ")
        && is_alnum_upper(comp)
    {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: Some(comp.into()),
        });
    }
    if trimmed == "EXCEPTIONALLY CONTROLLED INFORMATION" || trimmed == "ECI" {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: None,
        });
    }

    // -----------------------------------------------------------------
    // SI family (EL / ENDSEAL) — §H.4 p78 + p83
    // -----------------------------------------------------------------
    // §H.4 p78: "the EL control system is being retired and all
    // associated compartments moved to the SI control system". `EL
    // ECRU` → `SI-ECRU`; `EL NONBOOK` → `SI-NONBOOK` (per §H.4 p83
    // line 1938). Bare `ENDSEAL` / `EL` are recognized so the walker
    // can emit a suggest-only diagnostic.
    if let Some(comp) = trimmed.strip_prefix("ENDSEAL ")
        && is_alnum_upper(comp)
    {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: Some(comp.into()),
        });
    }
    if let Some(comp) = trimmed.strip_prefix("EL ")
        && is_alnum_upper(comp)
    {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: Some(comp.into()),
        });
    }
    if trimmed == "ENDSEAL" {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: None,
        });
    }
    // `EL` alone: 2 letters; would also collide with the SciControl
    // bare-CVE path if `SciControl::parse("EL")` were ever to match.
    // Today `EL` is NOT a published SciControl bare value (the ODNI
    // schema retired the EL control system per §H.4 p78), so accepting
    // it here is unambiguous.
    if trimmed == "EL" {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Si,
            compartment: None,
        });
    }

    // -----------------------------------------------------------------
    // TK family (KDK / KLONDIKE) — §H.4 p85 (NSG PM 3802 closure)
    // -----------------------------------------------------------------
    // Compound forms: `KDK-<COMP>` and `KLONDIKE-<COMP>`. The closure
    // note's per-compartment migration list is `TK-BLFH` / `TK-IDIT`
    // / `TK-KAND`, but the recognizer accepts any alphanumeric
    // compartment — unknown legacy compartments still need walker-level
    // diagnostic surface.
    if let Some(comp) = trimmed.strip_prefix("KLONDIKE-")
        && is_alnum_upper(comp)
    {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Tk,
            compartment: Some(comp.into()),
        });
    }
    if let Some(comp) = trimmed.strip_prefix("KDK-")
        && is_alnum_upper(comp)
    {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Tk,
            compartment: Some(comp.into()),
        });
    }
    if trimmed == "KLONDIKE" || trimmed == "KDK" {
        return Some(DeprecatedSciLongForm {
            system: SciControlBare::Tk,
            compartment: None,
        });
    }

    None
}

// =============================================================================
// EYES / EYES ONLY compound block recognizer (T135a Commit 5, issue #307)
// =============================================================================

/// Result of recognizing an EYES ONLY compound block (CAPCO-2016 §H.8 p157).
///
/// The block carries `/`-delimited country trigraphs followed by the
/// dissem-axis `EYES` (or `EYES ONLY`) marker. Without compound
/// recognition the parser's multi-token block handler splits on `/`
/// and tags each trigraph as Unknown — the walker rule (E064) needs
/// the structural block to emit a canonicalization fix.
#[derive(Debug, Clone)]
#[allow(dead_code)] // walker rule re-parses TokenSpan.text; these fields are recognizer-internal
pub(super) struct EyesOnlyBlock<'a> {
    /// The country trigraph tokens in source order.
    pub(super) trigraphs: SmallVec<[&'a str; 8]>,
    /// `true` iff the block ended with `EYES ONLY` (the full form);
    /// `false` for bare trailing `EYES`. Diagnostic / fix construction
    /// is identical in both cases.
    pub(super) full_form: bool,
}

/// Recognize a `<TRIGRAPH>(/ <TRIGRAPH>)* (SPACE)? EYES [ONLY]` block.
///
/// Returns `Some(block)` when `trimmed` matches the §H.8 p157 EYES
/// ONLY compound shape AND every trigraph in the prefix is a
/// CAPCO-registered country code. Returns `None` for bare `EYES` /
/// `EYES ONLY` without any preceding trigraphs (those fall through
/// to `parse_dissem_full_form` which handles the bare dissem-token
/// case correctly) AND for shape-matching blocks whose prefix
/// contains any unregistered trigraph.
///
/// Grammar (per CAPCO-2016 §A.6 p16 + §H.8 p157):
///   EYES_BLOCK := TRIGRAPH ('/' TRIGRAPH)* (' ' EYES_LIT)?
///   EYES_LIT   := 'EYES' | 'EYES ONLY'
///   TRIGRAPH   := registered CAPCO trigraph (3 alpha, in trigraph set)
///
/// Country-code registry gate (PR 9a Copilot R3, PR #416). The
/// shape gate alone — `[A-Z]{3}` — accepts arbitrary uppercase
/// triples like `AAA` or `XYZ`. Without registry validation, the
/// E064 walker would build `REL TO USA, AAA, XYZ` canonical output
/// — silent fabrication of trigraphs in the audit-stream. Per
/// Constitution Principle VIII (Authoritative Source Fidelity),
/// canonical output MUST reference real CAPCO registry entries.
/// Validation mirrors `parse_rel_to_with_spans` (parser.rs ~1870-
/// 1871): `tokens.is_country_code` filters by the registered trigraph
/// surface; `CountryCode::try_new` re-confirms the byte-set
/// invariant. An unregistered trigraph rejects the whole block —
/// the EYES recognizer is all-or-nothing; partial registry
/// matches fall through to Unknown.
///
/// Note: the brief's §H.8 p157 example wording uses `/` as the
/// trigraph separator within the EYES block — collision with the
/// dissem-category `/` separator per §A.6 p16. This is a real-world
/// CAPCO grammar wart, not a marque idiosyncrasy.
pub(super) fn recognize_eyes_only_block<'a>(
    trimmed: &'a str,
    tokens: &dyn TokenSet,
) -> Option<EyesOnlyBlock<'a>> {
    // The block must end with `EYES` or `EYES ONLY`. Strip the trailing
    // marker first to identify the trigraph-list prefix.
    let (prefix, full_form) = if let Some(p) = trimmed.strip_suffix(" EYES ONLY") {
        (p, true)
    } else {
        let p = trimmed.strip_suffix(" EYES")?;
        (p, false)
    };

    // Prefix is the trigraph list. Must be non-empty (otherwise it's
    // a bare `EYES` / `EYES ONLY` block, handled by the dissem-full-form
    // path).
    if prefix.is_empty() {
        return None;
    }

    // Split the prefix on `/` and require each segment to be a 3-letter
    // ASCII uppercase trigraph. Per §A.6 p16 the separator inside the
    // EYES block is `/` (not `, ` like REL TO).
    let mut trigraphs: SmallVec<[&str; 8]> = SmallVec::new();
    for seg in prefix.split('/') {
        // Shape gate: exactly 3 ASCII uppercase letters.
        if seg.len() != 3 || !seg.bytes().all(|b| b.is_ascii_uppercase()) {
            return None;
        }
        // Registry gate (Copilot R3, PR #416). Constitution
        // Principle VIII: fabricated trigraphs in canonical autofix
        // output are a correctness defect. Mirrors the validation
        // pattern in `parse_rel_to_with_spans` (~lines 1870-1871).
        if !tokens.is_country_code(seg) || CountryCode::try_new(seg.as_bytes()).is_none() {
            return None;
        }
        trigraphs.push(seg);
    }

    if trigraphs.is_empty() {
        return None;
    }

    Some(EyesOnlyBlock {
        trigraphs,
        full_form,
    })
}
