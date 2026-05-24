use super::*;

// ===========================================================================
// SAR subparser (§H.5 / §A.6)
// ===========================================================================

/// Parse a single SAR category block.
///
/// `block_text` is the full block text (everything between `//` separators)
/// INCLUDING the `SAR-` or `SPECIAL ACCESS REQUIRED-` indicator prefix.
/// `base` is the absolute byte offset in the original source where
/// `block_text` starts.
///
/// Returns `Some((marking, spans))` when `block_text` starts with a recognized
/// SAR indicator AND the remainder is grammatically non-empty. Each returned
/// [`TokenSpan`] carries absolute byte offsets into the source.
///
/// Grammar:
///
/// ```text
/// SAR_BLOCK    := INDICATOR PROGRAM ("/" PROGRAM)*
/// INDICATOR    := "SAR-" | "SPECIAL ACCESS REQUIRED-"
/// PROGRAM      := PROG_ID ( "-" COMPARTMENT )?
/// COMPARTMENT  := COMP_ID (" " SUB_COMP)*
/// PROG_ID      := [A-Z0-9]{2,3}           (SAR- form)
///               | [A-Z ]+                  (full-indicator form)
/// COMP_ID      := [A-Z0-9]+
/// SUB_COMP     := [A-Z0-9]+
/// ```
///
/// Rejection returns `None`:
/// - `SAR` without trailing hyphen.
/// - `SAR-` with an empty program identifier.
/// - A `//` sequence inside `block_text` (should not happen — the outer
///   category-block splitter would have handed us two separate blocks —
///   but we reject defensively).
/// - Empty string.
///
/// Ordering, classification, and roll-up constraints are NOT enforced here;
/// they are rule-layer (P3/P4) concerns.
pub(super) fn parse_sar_category(
    block_text: &str,
    base: usize,
) -> Option<(SarMarking, SmallVec<[TokenSpan; 16]>)> {
    // Defensive: `//` would mean the outer splitter gave us more than one
    // block. Refuse so the caller can record the text as Unknown and let
    // E030 handle it separately.
    if block_text.contains("//") {
        return None;
    }

    // Identify the indicator variant. Longer prefix first so `SPECIAL
    // ACCESS REQUIRED-` wins over any putative `SAR-` substring.
    let (indicator, indicator_lit) = if block_text.starts_with("SPECIAL ACCESS REQUIRED-") {
        (SarIndicator::Full, "SPECIAL ACCESS REQUIRED-")
    } else if block_text.starts_with("SAR-") {
        (SarIndicator::Abbrev, "SAR-")
    } else {
        return None;
    };
    let rest_offset = indicator_lit.len();
    let rest = &block_text[rest_offset..];
    if rest.is_empty() {
        return None;
    }

    // Inline-16: a SAR category emits the indicator span plus per-program
    // (id + per-compartment (1 + subs)) spans. The §A.6 grammar example
    // `SAR-ABC-DEF 123/SDA-121` totals ~8 spans; multi-program markings
    // with several compartments approach but rarely exceed 16.
    let mut spans: SmallVec<[TokenSpan; 16]> = SmallVec::new();

    // Record the indicator span (does NOT include the first character of
    // the program identifier — only the literal `SAR-` / `SPECIAL ACCESS
    // REQUIRED-` including the trailing hyphen).
    spans.push(TokenSpan {
        kind: TokenKind::SarIndicator,
        span: Span::new(base, base + indicator_lit.len()),
        text: indicator_lit.into(),
    });

    // Inline-4: CAPCO-2016 §H.5 examples typically show 1-2 SAR programs;
    // the §A.6 grammar example `SAR-ABC-DEF 123/SDA-121` is two; multi-
    // program markings rarely exceed 4.
    let mut programs: SmallVec<[SarProgram; 4]> = SmallVec::new();

    // Split the remainder on `/` into program chunks. Each chunk is a
    // `PROGRAM` production: `PROG_ID` optionally followed by `-COMPARTMENT`.
    //
    // Emit a `TokenKind::Separator` span (text = `"/"`) for each within-
    // category `/` byte (T131 / issue #106). The SAR variant is strict:
    // CAPCO-2016 §A.6 p16 forbids interjected whitespace in SAP-`/`, so
    // the separator span is always exactly the single `/` byte — no
    // adjacent-whitespace tolerance like the dissem/SCI multi-token path.
    let mut chunk_offset = rest_offset; // offset within block_text
    for (i, prog_chunk) in rest.split('/').enumerate() {
        if i > 0 {
            // The `/` byte we just consumed sits at `chunk_offset` (in
            // `block_text` coordinates). Record it before bumping past.
            let slash_abs = base + chunk_offset;
            spans.push(TokenSpan {
                kind: TokenKind::Separator,
                span: Span::new(slash_abs, slash_abs + 1),
                text: "/".into(),
            });
            chunk_offset += 1; // account for the `/` just consumed
        }
        let program_base = base + chunk_offset;

        let program = parse_sar_program(prog_chunk, program_base, indicator, &mut spans)?;
        programs.push(program);
        chunk_offset += prog_chunk.len();
    }

    if programs.is_empty() {
        return None;
    }

    Some((
        SarMarking::new(indicator, programs.into_boxed_slice()),
        spans,
    ))
}

/// Parse a single `PROGRAM` production.
///
/// `chunk` is everything between adjacent `/` separators (or between the
/// indicator and the next `/`, or the tail of the block). `base` is the
/// absolute offset of `chunk[0]` in the source buffer. `indicator` drives
/// the shape of the program identifier only; compartment and
/// sub-compartment parsing is identical for both indicator forms.
///
/// Grammar: `PROG_ID ( "-" COMPARTMENT )? ( "-" COMPARTMENT )* `, where
/// `COMPARTMENT` is `COMP_ID (" " SUB_COMP)*`. `PROG_ID` shape is:
///
/// - **Abbrev** (`SAR-`): 2–3 alphanumeric characters.
/// - **Full** (`SPECIAL ACCESS REQUIRED-`): one or more uppercase ASCII
///   letters, optionally with spaces. Hyphens are NOT permitted inside
///   the program identifier for the full form — the first `-` always
///   marks the program/compartment boundary (CAPCO-2016 §H.5 p100).
///
/// Canonical example per §H.5 p100: `SAR-BP-J12 J54-K15/CD-...` decomposes
/// BP as two compartments `J12` (with sub-compartment `J54`) and `K15`.
/// Within one program the sequence alternates:
///   `PROG "-" COMP (" " SUB)* ( "-" COMP (" " SUB)* )*`
///
/// # Shape gates
///
/// Token admission goes through the documented `marque-ism`
/// predicates rather than inline byte-class checks:
///
/// - Program identifier (Abbrev): [`SarProgram::admits_program_id_abbrev`]
///   — 2-3 ASCII alnum.
/// - Program identifier (Full): [`SarProgram::admits_program_id_full`]
///   — uppercase ASCII letters with optional spaces, must contain
///   at least one non-space byte; hyphens and digits rejected.
/// - Compartment identifier: [`SarCompartment::admits_identifier`]
///   — ≥1 ASCII alnum.
/// - Sub-compartment identifier: [`SarCompartment::admits_identifier`]
///   (same predicate; CAPCO-2016 §H.5 pp 99-100 places both grammar
///   positions under one rule).
///
/// Routing the parser through the same predicates the
/// `Vocabulary<CapcoScheme>::shape_admits(CAT_SAR, _)` arm calls
/// surface pins the parser's accept set to the documented vocabulary
/// surface (admission via documented vocabulary, no inline
/// `is_ascii_alphanumeric` byte-class checks). The same pattern is
/// used at [`parse_fgi_marker`] for FGI trigraph admission.
fn parse_sar_program(
    chunk: &str,
    base: usize,
    indicator: SarIndicator,
    spans: &mut SmallVec<[TokenSpan; 16]>,
) -> Option<SarProgram> {
    if chunk.is_empty() {
        return None;
    }

    // Split the chunk on `-`. The first segment is the program identifier;
    // each subsequent segment is a compartment (with optional space-joined
    // sub-compartments).
    let mut segments = split_with_offsets(chunk, '-');
    if segments.is_empty() {
        return None;
    }

    // Program identifier: first segment. Shape check depends on indicator.
    let (prog_off, prog_id) = segments.remove(0);
    if prog_id.is_empty() {
        return None;
    }
    // Route the program identifier shape gate through the canonical
    // `marque-ism` predicates, one per indicator form. Both predicates
    // are pure / allocation-free (Constitution Principle II) and carry
    // their CAPCO-2016 §H.5 citations alongside the predicate body —
    // keeping the gate single-sited prevents drift between the parser
    // and the `Vocabulary<CapcoScheme>::shape_admits(CAT_SAR, _)`
    // admission surface. Mirrors the FGI marker site at
    // [`parse_fgi_marker`] which routes through
    // [`CountryCode::admits_country_token`].
    let prog_shape_ok = match indicator {
        // §H.5 p101: "A program identifier abbreviation is the two
        // or three-character designator for the program."
        // §H.5 p99: "SAR program identifiers are alphanumeric values."
        SarIndicator::Abbrev => SarProgram::admits_program_id_abbrev(prog_id.as_bytes()),
        // §H.5 p101 + Table 7 §H.5 p100: full nickname is uppercase
        // letters with optional spaces (no digits, no hyphens). The
        // hyphen exclusion is load-bearing — the first hyphen after
        // the indicator literal always marks the program/compartment
        // boundary at this parser site.
        SarIndicator::Full => SarProgram::admits_program_id_full(prog_id.as_bytes()),
    };
    if !prog_shape_ok {
        return None;
    }
    spans.push(TokenSpan {
        kind: TokenKind::SarProgram,
        span: Span::new(base + prog_off, base + prog_off + prog_id.len()),
        text: prog_id.into(),
    });

    // Remaining segments: each is a compartment, possibly with
    // space-separated sub-compartments. Inline-4: CAPCO-2016 §H.5 p100
    // example `SAR-BP-J12 J54-K15/CD-...` shows two compartments per
    // program; markings rarely exceed 4 compartments per program.
    let mut compartments: SmallVec<[SarCompartment; 4]> = SmallVec::new();
    for (seg_off, seg) in segments {
        if seg.is_empty() {
            return None;
        }
        // Split segment on ` ` — first token is compartment, rest are subs.
        let mut parts = split_with_offsets(seg, ' ');
        let (comp_rel_off, comp_id) = parts.remove(0);
        // Compartment identifier shape gated through the canonical
        // `marque-ism` predicate. CAPCO-2016 §H.5 pp 99-100: "SAR
        // program identifiers are alphanumeric values"; the
        // surrounding prose applies the same rule to compartments and
        // sub-compartments. Length bound is ≥1 (manual silent on upper
        // bound; marque admits length 1+, with the divergence
        // documented at the predicate). Same predicate handles the
        // sub-compartment case below.
        if !SarCompartment::admits_identifier(comp_id.as_bytes()) {
            return None;
        }
        let comp_abs_off = seg_off + comp_rel_off;
        spans.push(TokenSpan {
            kind: TokenKind::SarCompartment,
            span: Span::new(base + comp_abs_off, base + comp_abs_off + comp_id.len()),
            text: comp_id.into(),
        });

        // Inline-4: CAPCO-2016 §H.5 example `SAR-BP-J12 J54` shows one
        // sub-compartment per compartment; markings rarely carry more
        // than 4 subs.
        let mut subs: SmallVec<[SmolStr; 4]> = SmallVec::new();
        for (sub_rel_off, sub_id) in parts {
            // Sub-compartment identifier shape gated through the same
            // canonical predicate as the compartment slot. CAPCO-2016
            // §H.5 pp 99-100 places both grammar positions under one
            // rule (alphanumeric values, no character-class or length
            // distinction); a single predicate admits both correctly.
            if !SarCompartment::admits_identifier(sub_id.as_bytes()) {
                return None;
            }
            let sub_abs_off = seg_off + sub_rel_off;
            spans.push(TokenSpan {
                kind: TokenKind::SarSubCompartment,
                span: Span::new(base + sub_abs_off, base + sub_abs_off + sub_id.len()),
                text: sub_id.into(),
            });
            subs.push(sub_id.into());
        }

        compartments.push(SarCompartment::new(comp_id, subs.into_boxed_slice()));
    }

    Some(SarProgram::new(prog_id, compartments.into_boxed_slice()))
}

/// Split `s` on `delim`, returning `(offset_in_s, token)` pairs. Unlike
/// [`split_slash_with_offsets`], this preserves empty tokens so callers can
/// detect malformed input (e.g., `SAR--BP` → two segments, the first empty).
fn split_with_offsets(s: &str, delim: char) -> SmallVec<[(usize, &str); 4]> {
    // Inline-4: the two SAR-parser call sites split on `-` (segment
    // count = compartments + 1, typically ≤ 4) and on ` ` (segment
    // count = sub-compartments + 1, typically ≤ 4) per CAPCO-2016
    // §H.5 — see callers in [`parse_sar_program`].
    let mut result: SmallVec<[(usize, &str); 4]> = SmallVec::new();
    let mut pos = 0usize;
    let delim_len = delim.len_utf8();
    for part in s.split(delim) {
        result.push((pos, part));
        pos += part.len() + delim_len;
    }
    result
}
