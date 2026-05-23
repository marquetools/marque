//! SCI delimiter recovery.
//!
//! Insert the `-` separator between an SCI control system and its
//! first compartment when the input wrote them as a glued token
//! (`HCSP` → `HCS-P`) and similar shape recoveries on SCI tokens.

use marque_ism::{SciControl, SciControlBare};

// ---------------------------------------------------------------------------
// SCI delimiter recovery
// ---------------------------------------------------------------------------

/// SCI delimiter recovery preprocessing — issue #198, #133 PR 10.
///
/// Repairs three classes of SCI delimiter typos against the closed
/// CVE vocabulary in `CVEnumISMSCIControls.xml`. Vocabulary checks
/// dispatch through the build-time-generated [`SciControlBare::parse`]
/// (bare control systems) and [`SciControl::parse`] (the full CVE set
/// including all registered control-compartment compounds), so the
/// repair surface tracks ODNI schema updates automatically — no
/// hand-maintained vocabulary slice to drift out of sync per
/// Constitution IV (Layer 1 generated predicates):
///
/// - **Pattern A (concatenated compound)**: a token equal to a compound
///   with the hyphen removed → canonical hyphenated form. `HCSP →
///   HCS-P`, `SIG → SI-G`, `TKKAND → TK-KAND`, etc.
/// - **Pattern B (concatenated bare control systems)**: a token of
///   length 4–6 that splits cleanly into two bare control systems →
///   slash-joined form (`SITK → SI/TK`, `HCSSI → HCS/SI`) per §A.6
///   p16 and the `TOP SECRET//ANB/SI/TK/XNB//NOFORN` example on p194.
///   Ambiguous splits bail out — see [`repair_sci_token`] for the
///   guard.
/// - **Pattern C (wrong delimiter)**: a token of the form
///   `<bare_cs>-<bare_cs>` that is NOT itself a registered compound →
///   slash-joined form. `SI-TK → SI/TK` (because `SI-TK` is not
///   registered), but `SI-G` is left alone (it IS registered — `-` is
///   the correct control-compartment separator per §A.6 p16).
///
/// **Out of scope** — sub-compartment fuzzy recovery (`ABCE → ABCD`),
/// unregistered-compartment recovery, and any rewrite that would
/// require fuzz-correcting against agency-assigned codewords. Those
/// require operator-supplied vocab (issue #180) — the engine cannot
/// invent identifiers it doesn't know are valid (Constitution VIII).
///
/// **Architectural shape** mirrors `try_rel_to_structural_repair`
/// (PR 9, #190): runs as preprocessing on the input string before
/// per-token fuzzy correction, returns `Some(repaired)` only when at
/// least one repair fired. The caller pushes a `BaseRateCommonMarking`
/// feature onto `delim_features` so every candidate derived from the
/// repaired text inherits the audit trace.
///
/// **Allocation behavior**: short-circuits without allocation when the
/// pre-check finds no SCI control system root in the text. The
/// per-token walk borrows the input until a fix actually fires.
pub(crate) fn try_sci_delimiter_repair(text: &str) -> Option<String> {
    if !contains_any_sci_root(text) {
        return None;
    }

    // ASCII-only guard. The SCI control-system vocabulary
    // (`SciControlBare::ALL`) and the registered compound names
    // (`SciControl::ALL`) are pure ASCII, as are the delimiters this
    // function recognizes (`-`, `/`, `(`, `)`, space, tab, newline,
    // CR, comma). So any non-ASCII input cannot match any pattern;
    // bailing early avoids the byte-vs-char-boundary hazard that
    // would otherwise arise from indexing `text` with byte offsets.
    if !text.is_ascii() {
        return None;
    }

    let bytes = text.as_bytes();
    let mut result: Option<String> = None;
    let mut last_copied = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let at_boundary = i == 0
            || matches!(
                bytes[i - 1],
                b'/' | b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r' | b','
            );
        if !at_boundary {
            i += 1;
            continue;
        }

        let token_start = i;
        let token_end = bytes[token_start..]
            .iter()
            .position(|&b| matches!(b, b'/' | b'(' | b')' | b' ' | b'\t' | b'\n' | b'\r' | b','))
            .map(|n| token_start + n)
            .unwrap_or(bytes.len());

        if token_start < token_end {
            let token = &text[token_start..token_end];
            if let Some(repaired) = repair_sci_token(token) {
                let r = result.get_or_insert_with(|| String::with_capacity(text.len()));
                r.push_str(&text[last_copied..token_start]);
                r.push_str(&repaired);
                last_copied = token_end;
            }
        }

        // Advance past the token; the next iteration will re-check the
        // boundary before the byte after the delimiter (or terminate at
        // end-of-input).
        i = token_end + 1;
    }

    result.map(|mut r| {
        r.push_str(&text[last_copied..]);
        r
    })
}

/// Cheap pre-check for [`try_sci_delimiter_repair`]: returns true when
/// the input contains at least one bare SCI control system identifier
/// as a substring. False positives just mean we walk the bytes and
/// return `None` — no correctness impact, only a performance
/// optimization for the overwhelmingly common case where the input has
/// no SCI category at all.
pub(crate) fn contains_any_sci_root(text: &str) -> bool {
    text.contains("HCS")
        || text.contains("KLM")
        || text.contains("MVL")
        || text.contains("RSV")
        || text.contains("BUR")
        || text.contains("SI")
        || text.contains("TK")
}

/// Per-token classifier for SCI delimiter repair. Returns the repaired
/// token if one of patterns A/B/C matches; otherwise `None`.
///
/// All vocabulary checks dispatch through the build-time-generated
/// [`SciControlBare::parse`] and [`SciControl::parse`] (from
/// `marque-ism`'s generated `values.rs`), so the repair surface tracks
/// `CVEnumISMSCIControls.xml` automatically. New CVE compounds added
/// in a future ODNI schema bump (e.g., a hypothetical `SI-XYZ`) are
/// auto-discovered by Pattern A without any code change here.
///
/// Pattern dispatch order:
/// 1. Pattern A (split into bare-CS prefix + suffix; if
///    `{prefix}-{suffix}` is a registered CVE value, return it)
/// 2. Pattern C (token contains `-`, neither side is a registered
///    compound's compartment, both halves are bare CS)
/// 3. Pattern B (no `-`, splits into two bare CS, unambiguous)
pub(crate) fn repair_sci_token(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }

    // ASCII-only guard. The CVE vocabulary is pure ASCII, so a non-
    // ASCII token cannot match any pattern; bailing early ensures
    // the byte-offset slicing below (`token[..split]`,
    // `token[split..]`, `token[..dash_pos]`, `token[dash_pos + 1..]`)
    // never lands in the middle of a multi-byte UTF-8 sequence. This
    // is a defense-in-depth check — the only production caller
    // (`try_sci_delimiter_repair`) already gates on ASCII — but
    // keeping it here makes the function's invariant local and
    // self-evident for any future caller (e.g., a unit test).
    if !token.is_ascii() {
        return None;
    }

    let len = token.len();

    // Pattern A — concatenated registered compound. Walk every split
    // where the prefix is a bare control system; if `{prefix}-{suffix}`
    // is in the CVE vocabulary, return the canonical hyphenated form.
    // Bare CS lengths are 2 or 3; suffix length range comes from CVE
    // (max compartment-form suffix is 4 chars, e.g. TK-BLFH).
    if !token.contains('-') && (3..=8).contains(&len) {
        for &split in &[2usize, 3] {
            if split >= len {
                continue;
            }
            let prefix = &token[..split];
            let suffix = &token[split..];
            if SciControlBare::parse(prefix).is_some() {
                let canonical = format!("{prefix}-{suffix}");
                if SciControl::parse(&canonical).is_some() {
                    return Some(canonical);
                }
            }
        }
    }

    // Pattern C — wrong delimiter (`-` between two bare CS). Skip if
    // the whole token is itself a registered CVE compound.
    if let Some(dash_pos) = token.find('-') {
        if SciControl::parse(token).is_some() {
            return None;
        }
        let prefix = &token[..dash_pos];
        let suffix = &token[dash_pos + 1..];
        if SciControlBare::parse(prefix).is_some() && SciControlBare::parse(suffix).is_some() {
            return Some(format!("{prefix}/{suffix}"));
        }
        return None;
    }

    // Pattern B — concatenated bare control systems (no delimiter).
    // Bare CS lengths are 2 or 3; the concatenation is therefore in
    // [4..=6]. Try splits at positions 2 and 3 (the only split points
    // that can yield two valid bare-CS halves) and require an
    // unambiguous match.
    if !(4..=6).contains(&len) {
        return None;
    }
    let mut found: Option<(&str, &str)> = None;
    for &split in &[2usize, 3] {
        if split >= len {
            continue;
        }
        let suffix_len = len - split;
        if !(2..=3).contains(&suffix_len) {
            continue;
        }
        let prefix = &token[..split];
        let suffix = &token[split..];
        if SciControlBare::parse(prefix).is_some() && SciControlBare::parse(suffix).is_some() {
            if found.is_some() {
                return None;
            }
            found = Some((prefix, suffix));
        }
    }
    found.map(|(p, s)| format!("{p}/{s}"))
}

