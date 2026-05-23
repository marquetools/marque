//! Canonical reordering of token segments.
//!
//! When the input has the right tokens in the wrong category order
//! (e.g., a non-US classification segment lands after a US
//! classification), this pass detects the reorder and emits a
//! corrected attempt. Also houses the classification-floor predicates
//! (`meets_classification_floor`, `marking_classification`) used by
//! the recognizer to gate candidates below the configured floor.

use marque_capco::CapcoMarking;
use marque_ism::{
    CapcoTokenSet, Classification,
    token_set::TokenSet as _,
};

// Token reordering
// ---------------------------------------------------------------------------

/// Try to produce a canonical-order rewrite of `text`.
///
/// The CAPCO category order is: classification → SCI → SAR → dissem.
/// If the observed segments are out of order — e.g., `NOFORN//SECRET`
/// with dissem first — this helper swaps them into the canonical
/// order. Returns `None` when the input is already in canonical order
/// or when reordering doesn't apply (CAB lines, single-segment input).
pub(crate) fn try_canonical_reorder(text: &str) -> Option<String> {
    // Only banner/portion-shaped input (contains `//`) is reorderable
    // with this heuristic. CABs use keyed authority lines, not
    // category ordering.
    if !text.contains("//") {
        return None;
    }

    // Portion form: `(C//NF)` — strip the surrounding parens for
    // reasoning, re-wrap at emit.
    let (prefix, body, suffix) = if text.starts_with('(') && text.ends_with(')') {
        ("(", &text[1..text.len() - 1], ")")
    } else {
        ("", text, "")
    };

    let segments: Vec<&str> = body.split("//").collect();
    if segments.len() < 2 {
        return None;
    }

    // Classify each segment by its dominant category. We only
    // reorder when exactly one segment is classification-dominant
    // and at least one other is dissem-dominant — otherwise the
    // input is too ambiguous for a clean swap.
    let mut class_segments: Vec<&str> = Vec::new();
    let mut dissem_segments: Vec<&str> = Vec::new();
    let mut other_segments: Vec<&str> = Vec::new();
    for seg in &segments {
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        match classify_segment(seg) {
            SegmentClass::Classification => class_segments.push(seg),
            SegmentClass::Dissem => dissem_segments.push(seg),
            SegmentClass::Other => other_segments.push(seg),
        }
    }

    if class_segments.is_empty() {
        return None;
    }

    // Detect non-US markings: any classification segment is a NATO,
    // JOINT, or FGI classification (not a US classification level).
    let is_non_us = class_segments
        .iter()
        .any(|s| is_non_us_classification_segment(s));

    // Already-canonical check: if the classification segment is the
    // first non-empty segment, no reorder is needed.
    // For non-US markings: also require that the body already starts
    // with `//` (the empty US classification slot). If the class is
    // first but the `//` prefix is absent, fall through to add it.
    if let Some(first) = segments.iter().find(|s| !s.trim().is_empty()) {
        if class_segments.contains(&first.trim()) {
            // US: already canonical.
            // Non-US: already canonical only when // prefix is present.
            if !is_non_us || body.starts_with("//") {
                return None;
            }
        }
    }

    // Emit: classification → other (SCI/SAR/FGI blocks) → dissem.
    let mut ordered: Vec<&str> = Vec::new();
    ordered.extend(class_segments);
    ordered.extend(other_segments);
    ordered.extend(dissem_segments);

    let joined = ordered.join("//");

    // Non-US canonical form: `//{class}//{others}//{dissems}`. The
    // leading `//` represents the empty US classification slot (per
    // CAPCO-2016 §A.6) and signals the strict parser to use the
    // non-US classification code path.
    if is_non_us {
        Some(format!("{prefix}//{joined}{suffix}"))
    } else {
        Some(format!("{prefix}{joined}{suffix}"))
    }
}

/// Which CAPCO category a `//`-separated segment primarily belongs to.
///
/// A segment is classification-dominant if its first token is a known
/// classification level (`U`, `C`, `S`, `TS`, `CONFIDENTIAL`, …).
/// Dissem-dominant if its first token is a known dissem control
/// (`NOFORN`, `NF`, `ORCON`, …). Otherwise Other (SCI/SAR/FGI
/// sub-blocks, REL TO lists, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SegmentClass {
    Classification,
    Dissem,
    Other,
}

pub(crate) fn classify_segment(seg: &str) -> SegmentClass {
    let first_token = seg.split_whitespace().next().unwrap_or("");
    // Strip trailing commas.
    let first_token = first_token.trim_end_matches(',');
    // Single-whitespace-token classifications only. `TOP SECRET` and
    // multi-word NATO/JOINT forms are handled by the separate
    // starts_with branches below.
    const CLASSIFICATIONS: &[&str] = &[
        "U",
        "R",
        "C",
        "S",
        "TS",
        "UNCLASSIFIED",
        "RESTRICTED",
        "CONFIDENTIAL",
        "SECRET",
        // NATO classification abbreviations (single-token forms).
        // The five legacy compound forms (CTSA / NSAT / NCA / CTS-B /
        // CTS-BALK) stay in the decoder recognition set because the
        // strict parser, post-PR-9c.1 T134, canonicalizes them into
        // bare class + AEA/SCI companion writes (CAPCO-2016 §H.7 p122
        // for ATOMAL → AEA; §G.2 p40 + §H.7 p127 for BALK/BOHEMIA →
        // SCI). The E066 autofix rule then surfaces the text-level
        // re-marking suggestion per the §G.2 p40 Table 5 registration
        // of the canonical control-marking forms.
        "NS",
        "NC",
        "NU",
        "CTS",
        "CTSA",
        "NSAT",
        "NCA",
        "CTS-B",
        "CTS-BALK",
        // JOINT classification indicator.
        "JOINT",
    ];
    // Dissemination-control tokens — IC (§H.8) and non-IC (§H.9).
    // SCI controls (HCS, SI, TK, and all their sub-compartment forms)
    // are NOT in this list — they belong to their own category under
    // CAPCO §A.6 and the canonical order places them between
    // classification and dissem. Classifying an HCS segment as Dissem
    // would drive `try_canonical_reorder` to move it past the dissem
    // block, corrupting the rewrite. SCI segments therefore fall
    // through to `SegmentClass::Other`, which the reorder helper
    // inserts between classification and dissem — the right spot per
    // CAPCO-2016 §A.6.
    //
    // AEA controls (RD, FRD, TFNI, CNWDI, SIGMA) are also omitted —
    // they appear between SCI and dissem per §A.6. A pre-check above
    // `CLASSIFICATIONS.contains` prevents "RESTRICTED DATA" from being
    // mistaken for the NATO RESTRICTED classification.
    //
    // "REL" is the first token of "REL TO {country-list}" segments.
    //
    // Non-IC dissem controls (§H.9): portion marks (DS, XD, ND,
    // SBU, SBU-NF, LES, LES-NF, SSI) and banner abbreviations
    // (LIMDIS, EXDIS, NODIS) are included so reordering places them
    // in the dissem block, not the SCI/AEA block (CAPCO-2016 §A.6).
    const DISSEMS: &[&str] = &[
        // §H.8 IC dissemination controls
        "NOFORN", "NF", "ORCON", "OC", "PROPIN", "PR", "IMCON", "IMC", "RELIDO", "RS", "RSEN",
        "DSEN", "FISA", "FOUO", "EYES", "REL",
        // §H.9 non-IC dissemination controls — portion marks
        "DS", "XD", "ND", "SBU", "SBU-NF", "LES", "LES-NF", "SSI",
        // §H.9 non-IC dissemination controls — banner abbreviations
        "LIMDIS", "EXDIS", "NODIS",
    ];
    // Pre-check: "RESTRICTED DATA" (AEA marking, §H.6) must not be
    // mistaken for the NATO RESTRICTED classification even though
    // "RESTRICTED" appears in CLASSIFICATIONS. The bare token
    // "RESTRICTED" IS valid as NATO classification; "RESTRICTED DATA"
    // and longer AEA forms are not. CAPCO-2016 §H.6 p113.
    if first_token == "RESTRICTED" && seg.split_whitespace().nth(1).is_some() {
        return SegmentClass::Other;
    }
    if CLASSIFICATIONS.contains(&first_token) {
        SegmentClass::Classification
    // Single-token dissem controls and multi-word non-IC long-title forms.
    // Multi-word forms cannot be single-token-matched because their first words
    // ("LIMITED", "NO", "EXCLUSIVE", "LAW", "SENSITIVE") are too ambiguous;
    // they are checked via starts_with here. CAPCO-2016 §H.8–9.
    } else if DISSEMS.contains(&first_token)
        || (first_token == "LIMITED" && seg.starts_with("LIMITED DISTRIBUTION"))
        || (first_token == "NO" && seg.starts_with("NO DISTRIBUTION"))
        || (first_token == "EXCLUSIVE" && seg.starts_with("EXCLUSIVE DISTRIBUTION"))
        || (first_token == "LAW" && seg.starts_with("LAW ENFORCEMENT SENSITIVE"))
        || (first_token == "SENSITIVE"
            && (seg.starts_with("SENSITIVE BUT UNCLASSIFIED")
                || seg.starts_with("SENSITIVE SECURITY INFORMATION")))
    {
        SegmentClass::Dissem
    } else if (first_token == "TOP" && seg.starts_with("TOP SECRET"))
        || (first_token == "COSMIC" && seg.starts_with("COSMIC TOP SECRET"))
        || (first_token == "NATO"
            && (seg.starts_with("NATO SECRET")
                || seg.starts_with("NATO CONFIDENTIAL")
                || seg.starts_with("NATO UNCLASSIFIED")
                || seg.starts_with("NATO RESTRICTED")))
    {
        SegmentClass::Classification
    } else if CapcoTokenSet.is_trigraph(first_token) {
        // FGI pattern: {registered country trigraph} {classification level}.
        // Validated against the authoritative CVEnumISMCATRelTo vocabulary so
        // typos like "OTP" (→ TOP) don't get mistaken for FGI country codes.
        let second = seg.split_whitespace().nth(1).unwrap_or("");
        let second = second.trim_end_matches(',');
        if matches!(
            second,
            "U" | "R"
                | "C"
                | "S"
                | "TS"
                | "UNCLASSIFIED"
                | "RESTRICTED"
                | "CONFIDENTIAL"
                | "SECRET"
        ) || (second == "TOP"
            && seg
                .split_whitespace()
                .nth(2)
                .is_some_and(|t| t.trim_end_matches(',') == "SECRET"))
        {
            SegmentClass::Classification
        } else {
            SegmentClass::Other
        }
    } else {
        SegmentClass::Other
    }
}

/// Returns true when `seg` is a non-US classification segment: a NATO
/// classification abbreviation, a JOINT classification phrase, or an FGI
/// `{trigraph} {level}` pattern.
///
/// Used by `try_canonical_reorder` to decide whether the reordered output
/// needs a leading `//` (the empty US classification slot that signals the
/// strict parser to take the non-US code path).
pub(crate) fn is_non_us_classification_segment(seg: &str) -> bool {
    const NATO_ABBREVS: &[&str] = &[
        "NS", "NC", "NU", "CTS", "CTSA", "NSAT", "NCA", "CTS-B", "CTS-BALK",
    ];
    let mut tokens = seg.split_whitespace();
    let first = tokens.next().unwrap_or("");
    let first = first.trim_end_matches(',');
    if NATO_ABBREVS.contains(&first) {
        return true;
    }
    if first == "JOINT" {
        return true;
    }
    if first == "COSMIC" && seg.starts_with("COSMIC TOP SECRET") {
        return true;
    }
    if first == "NATO"
        && (seg.starts_with("NATO SECRET")
            || seg.starts_with("NATO CONFIDENTIAL")
            || seg.starts_with("NATO UNCLASSIFIED")
            || seg.starts_with("NATO RESTRICTED"))
    {
        return true;
    }
    // FGI: {registered country trigraph} {classification level}.
    // Validated against the authoritative CVEnumISMCATRelTo vocabulary so
    // typos like "OTP" (→ TOP) are not mistaken for FGI country codes.
    if CapcoTokenSet.is_trigraph(first) {
        let second = tokens.next().unwrap_or("");
        let second = second.trim_end_matches(',');
        if matches!(
            second,
            "U" | "R"
                | "C"
                | "S"
                | "TS"
                | "UNCLASSIFIED"
                | "RESTRICTED"
                | "CONFIDENTIAL"
                | "SECRET"
        ) {
            return true;
        }
        if second == "TOP"
            && tokens
                .next()
                .is_some_and(|t| t.trim_end_matches(',') == "SECRET")
        {
            return true;
        }
    }
    false
}

/// Prepends the non-US leading `//` when the entire input (no existing `//`)
/// looks like a non-US classification segment.
///
/// This covers bare non-US markings like `NS`, `JOINT S GBR USA`, or
/// `CAN S` that arrive with no delimiter at all — `try_canonical_reorder`
/// cannot act on them because it requires at least two `//`-separated
/// segments. Emitting `//NS`, `//JOINT S GBR USA`, etc. lets the strict
/// parser recognize the non-US code path (CAPCO-2016 §A.6, parser block 1).
pub(crate) fn try_add_non_us_prefix(text: &str) -> Option<String> {
    // Only act when there is no `//` at all — try_canonical_reorder
    // handles the has-// but missing-prefix case.
    if text.contains("//") {
        return None;
    }
    let (prefix, body, suffix) = if text.starts_with('(') && text.ends_with(')') {
        ("(", &text[1..text.len() - 1], ")")
    } else {
        ("", text, "")
    };
    if is_non_us_classification_segment(body.trim()) {
        Some(format!("{prefix}//{body}{suffix}"))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// FR-011 strict-context floor
// ---------------------------------------------------------------------------

/// True when `marking`'s classification level is ≥ `floor`.
///
/// FR-011 invariant. `floor` is the `Classification as u8` encoding
/// (Unclassified=0 … TopSecret=4) — see [`ParseContext::classification_floor`].
///
/// A marking with no classification info cannot clear a non-trivial
/// floor — return `false` so the candidate is dropped when the floor
/// is CONFIDENTIAL or above.
pub(crate) fn meets_classification_floor(marking: &CapcoMarking, floor: u8) -> bool {
    let Some(level) = marking_classification(marking) else {
        return floor == Classification::Unclassified as u8;
    };
    (level as u8) >= floor
}

/// Extract the effective classification level from a parsed marking.
///
/// Delegates to [`marque_ism::MarkingClassification::effective_level`],
/// which handles all variants (`Us`, `Fgi`, `Nato`, `Joint`,
/// `Conflict`) by mapping each to the canonical [`Classification`]
/// ladder. NATO levels map through
/// [`NatoClassification::us_equivalent`](marque_ism::NatoClassification::us_equivalent).
pub(crate) fn marking_classification(marking: &CapcoMarking) -> Option<Classification> {
    marking
        .0
        .classification
        .as_ref()
        .map(|c| c.effective_level())
}
