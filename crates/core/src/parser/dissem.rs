use super::*;

/// Map a banner-form (full-word) dissemination control to its CVE
/// abbreviation form. The CVE only ships abbreviations (`NF`, `OC`, ...),
/// but banner markings use the full words (`NOFORN`, `ORCON`, ...) and the
/// parser must accept both. Phase 3 added this fallback so banner-form
/// markings parse cleanly into a typed `DissemControl`.
///
/// Rules that detect "banner uses portion abbreviation" (E001) read the
/// raw token span via `attrs.token_spans` and inspect the original bytes,
/// so this mapping does not lose the abbreviation-vs-full-word signal.
///
/// Mapping data sourced from [`marque_ism::marking_forms`].
pub(super) fn parse_dissem_full_form(s: &str) -> Option<DissemControl> {
    // Accept both the Banner Line Abbreviation (e.g., "NOFORN") and the
    // long Marking Title (e.g., "NOT RELEASABLE TO FOREIGN NATIONALS")
    // per CAPCO-2016 §D.1 p27: "Any control markings in the banner
    // line may be spelled out per the 'Marking Title' ... or abbreviated
    // as per the 'Authorized Abbreviation' ... in accordance with the
    // Register". Long-title acceptance is what lets the S001 style rule
    // observe banner-form tokens that use the full title — without it
    // the parser would tag those as Unknown and E008 would fire instead.
    let portion = marque_ism::marking_forms::banner_to_portion(s)
        .or_else(|| marque_ism::marking_forms::title_to_portion(s))?;
    DissemControl::parse(portion)
}

/// Non-IC dissemination control parser covering both the Banner Line
/// Abbreviation (e.g., `"LIMDIS"`) and the long "Marking Title" form
/// (e.g., `"LIMITED DISTRIBUTION"`). Mirror of [`parse_dissem_full_form`]
/// for the §9 non-IC marking set so the S001 style rule can see title
/// tokens across both categories.
pub(super) fn parse_non_ic_full_form(s: &str) -> Option<NonIcDissem> {
    NonIcDissem::parse(s).or_else(|| {
        let portion = marque_ism::marking_forms::title_to_portion(s)?;
        NonIcDissem::parse(portion)
    })
}

/// SCI control system parser covering the long "Marking Title" form
/// (e.g., `"TALENT KEYHOLE"` → `TK`, `"MARVEL"` → `MVL`,
/// `"KLAMATH"` → `KLM`).
///
/// Per CAPCO-2016 §D.1 p27 any control marking in the banner line may
/// be spelled out per the Marking Title; per §H.4 p85 `TALENT KEYHOLE`
/// is the registered Authorized Banner Line Marking Title for the TK
/// control system. MARVEL and KLAMATH are published in ODNI ISM
/// `CVEnumISMSCIControls.xml` as `<Description>` long-form titles
/// (post-CAPCO-2016, but already in the ISM token set).
///
/// Mirror of [`parse_dissem_full_form`] for the §H.4 SCI control set.
/// Without this fallback the parser tags `TALENT KEYHOLE` (and the
/// other long-form bare control systems) as Unknown and E008 fires
/// on legitimate banner-line content.
pub(super) fn parse_sci_control_full_form(s: &str) -> Option<SciControl> {
    let portion = marque_ism::marking_forms::banner_to_portion(s)
        .or_else(|| marque_ism::marking_forms::title_to_portion(s))?;
    SciControl::parse(portion)
}

/// AEA / nuclear-information marking parser covering the long
/// "Marking Title" forms per CAPCO-2016 §G.1 Table 4 + §H.6, layered
/// over [`AeaMarking::parse`].
///
/// Handles three classes of long-form input the bare
/// [`AeaMarking::parse`] does not recognize:
///
/// 1. **Standalone titles** — `"DOE UNCLASSIFIED CONTROLLED NUCLEAR
///    INFORMATION"` / `"DOD UNCLASSIFIED CONTROLLED NUCLEAR
///    INFORMATION"` map to `DoeUcni` / `DodUcni`. Resolved via the
///    `MARKING_FORMS` title → portion lookup, then re-parsed through
///    `AeaMarking::parse` so the resolution stays single-sourced in
///    `MARKING_FORMS`.
/// 2. **`RD-{long-form modifier}` compounds** — `RD-CRITICAL NUCLEAR
///    WEAPON DESIGN INFORMATION` maps to `RD-CNWDI` (`Rd(RdBlock {
///    cnwdi: true, sigma: [] })`). Strip the `RD-` / `RESTRICTED DATA-`
///    prefix, look up the trailing long-form title to obtain its
///    portion abbreviation, recompose with `RD-` and delegate back to
///    `AeaMarking::parse`.
/// 3. **`FRD-{long-form modifier}` mirror** — same shape for
///    Formerly Restricted Data.
///
/// Mirror of [`parse_dissem_full_form`] for the §H.6 AEA marking set.
/// Without this fallback the parser tags `RD-CRITICAL NUCLEAR WEAPON
/// DESIGN INFORMATION` and `DOE UNCLASSIFIED CONTROLLED NUCLEAR
/// INFORMATION` as Unknown and E008 fires on legitimate banner-line
/// content. Authority: CAPCO-2016 §G.1 Table 4 (Marking Title /
/// Banner Abbreviation / Portion Mark columns; rows on pp36-38) +
/// §H.6 p106 (CNWDI requires RD), §H.6 p116-117 (DOD UCNI), §H.6
/// p118-119 (DOE UCNI).
pub(super) fn parse_aea_full_form(s: &str) -> Option<AeaMarking> {
    if let Some(m) = AeaMarking::parse(s) {
        return Some(m);
    }
    if let Some(rest) = s
        .strip_prefix("RD-")
        .or_else(|| s.strip_prefix("RESTRICTED DATA-"))
        && let Some(portion) = marque_ism::marking_forms::title_to_portion(rest)
    {
        let mut buf = String::with_capacity(3 + portion.len());
        buf.push_str("RD-");
        buf.push_str(portion);
        return AeaMarking::parse(&buf);
    }
    if let Some(rest) = s
        .strip_prefix("FRD-")
        .or_else(|| s.strip_prefix("FORMERLY RESTRICTED DATA-"))
        && let Some(portion) = marque_ism::marking_forms::title_to_portion(rest)
    {
        let mut buf = String::with_capacity(4 + portion.len());
        buf.push_str("FRD-");
        buf.push_str(portion);
        return AeaMarking::parse(&buf);
    }
    let portion = marque_ism::marking_forms::banner_to_portion(s)
        .or_else(|| marque_ism::marking_forms::title_to_portion(s))?;
    AeaMarking::parse(portion)
}

/// Return type for [`parse_rel_to_with_spans`].
///
/// Carries both the recognized country codes and any dissem/non-IC controls
/// that were appended to the last comma entry via an intra-segment `/`
/// separator (e.g., `REL TO USA, FVEY/NF` → countries=[USA, FVEY],
/// trailing_dissem=[NF]).
///
/// All three fields use `SmallVec` inline storage sized to the empirical
/// REL TO distribution: country lists are typically 1–8 entries and
/// trailing dissem/non-IC controls 0–2. The caller drains each field via
/// `Vec::extend(IntoIterator)`, so the storage type is invisible at the
/// call site.
pub(super) struct RelToParseResult<'src> {
    pub(super) countries: SmallVec<[ParsedRelToEntry<'src>; 8]>,
    pub(super) trailing_dissem: SmallVec<[ParsedDissem<'src>; 2]>,
    pub(super) trailing_non_ic: SmallVec<[ParsedNonIcDissem<'src>; 2]>,
    /// DISPLAY ONLY country entries from a trailing
    /// `/DISPLAY ONLY [LIST]` segment commingled in the same
    /// `//`-block as the REL TO list per CAPCO-2016 §H.8 p165
    /// Notional Example Page 5 (e.g.,
    /// `(S//REL TO USA, IRQ/DISPLAY ONLY AFG)`). §H.8 p164 admits
    /// DISPLAY ONLY commingling with REL TO "when all information
    /// within the portion has been reviewed through the
    /// originator's foreign disclosure channels and approved for
    /// disclosure and release to separate Register, Annex B
    /// trigraph country code(s) or Register, Annex A tetragraph
    /// code(s)."
    pub(super) trailing_display_only: SmallVec<[ParsedDisplayOnlyEntry<'src>; 4]>,
}

/// Span-aware parse of a `REL TO ...` block. Records one
/// `TokenKind::RelToTrigraph` per recognized country code.
///
/// When a comma entry ends with `/<control>` — e.g., the last entry is
/// `FVEY/NF` instead of just `FVEY` — the function splits on the `/` and
/// parses the tail as additional dissem/non-IC controls. This handles the
/// CAPCO portion-mark convention where dissem controls in the same `//`-slot
/// are separated by `/` (e.g., `(TS//REL TO USA, FVEY/NF)` is valid). The
/// caller must extend its own `dissem`/`non_ic` vecs from the returned
/// `trailing_dissem` / `trailing_non_ic` fields.
///
/// `block_offset` is the absolute byte offset of `block` within the
/// original source buffer.
pub(super) fn parse_rel_to_with_spans<'src>(
    block: &'src str,
    block_offset: usize,
    tokens: &dyn TokenSet,
    token_spans: &mut SmallVec<[TokenSpan; 16]>,
) -> RelToParseResult<'src> {
    // Skip the "REL TO" / "REL" prefix to land on the trigraph list. We
    // need the offset of the *trigraph list* within `block` so that each
    // trigraph's absolute span can be computed.
    let prefix_skip = if let Some(rest) = block.strip_prefix("REL TO") {
        block.len() - rest.len()
    } else if let Some(rest) = block.strip_prefix("REL") {
        block.len() - rest.len()
    } else {
        0
    };
    let after_rel = &block[prefix_skip..];

    let mut countries: SmallVec<[ParsedRelToEntry<'src>; 8]> = SmallVec::new();
    let mut trailing_dissem: SmallVec<[ParsedDissem<'src>; 2]> = SmallVec::new();
    let mut trailing_non_ic: SmallVec<[ParsedNonIcDissem<'src>; 2]> = SmallVec::new();
    let mut trailing_display_only: SmallVec<[ParsedDisplayOnlyEntry<'src>; 4]> = SmallVec::new();

    // Pre-scan for a `/DISPLAY ONLY` boundary so that any commas
    // appearing AFTER the DISPLAY ONLY keyword belong to the
    // DISPLAY ONLY country list, not to the REL TO list.
    //
    // Without this pre-scan, an input like
    // `REL TO USA, IRQ/DISPLAY ONLY AFG, NATO` would comma-split
    // `after_rel` first, producing entries
    // `["USA", " IRQ/DISPLAY ONLY AFG", " NATO"]` — the third
    // entry `NATO` would be processed as a REL TO trigraph
    // rather than as the second country of the DISPLAY ONLY list.
    // Copilot review on PR #445 caught this misclassification.
    //
    // The boundary detection tolerates whitespace between the `/`
    // and the `DISPLAY ONLY` keyword to match the parser's
    // existing within-category-separator relaxation
    // (`crates/core/tests/separator_spans.rs`). The substring
    // match is gated on a word-boundary check (the byte after
    // `DISPLAY ONLY` must be whitespace, `/`, or end-of-string)
    // so `DISPLAY ONLYISH` cannot false-positive.
    //
    // Authority: CAPCO-2016 §H.8 p163 (DISPLAY ONLY entry); §H.8
    // p164 + §H.8 p165 Notional Example Page 5 (commingling with
    // REL TO under defined disclosure-review conditions).
    let display_only_boundary = find_display_only_slash_boundary(after_rel);

    // The comma-split scope is restricted to bytes before the
    // DISPLAY ONLY slash (if any). The DISPLAY ONLY suffix is
    // parsed separately after the loop.
    let rel_scope = match display_only_boundary {
        Some(b) => &after_rel[..b],
        None => after_rel,
    };

    // Walk comma-separated entries, tracking each entry's offset within
    // `after_rel` so we can land an absolute span on the trigraph itself
    // (not on any leading whitespace).
    let mut cursor = 0usize;
    for entry in rel_scope.split(',') {
        let entry_start_in_after = cursor;
        // Advance past the entry and its trailing comma. On the final
        // iteration this steps one past the end of `after_rel`, but the
        // cursor is never read after the loop ends — the split iterator
        // drives loop termination, not the cursor. usize addition here
        // is bounded by the document size, so no overflow in practice.
        cursor += entry.len() + 1;

        let trim_lead = entry.len() - entry.trim_start().len();
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let abs_start = block_offset + prefix_skip + entry_start_in_after + trim_lead;

        // If the entry contains `/`, the part before the slash is the country
        // code and the part(s) after are additional dissem/non-IC controls
        // packed into the same `//`-slot (e.g., `FVEY/NF` in `REL TO USA,
        // FVEY/NF`). CAPCO portion-mark syntax uses `/` as the intra-segment
        // control separator within a `//`-delimited slot (§A.4 / §D.1).
        if let Some(slash_pos) = trimmed.find('/') {
            let country_part = trimmed[..slash_pos].trim();
            // Span arithmetic: the tail loop iterates over
            // `tail.split('/')` parts and emits per-part spans at
            // `tail_base + tail_cursor + part_trim_lead`. Two bytes
            // can be silently dropped if we don't track them:
            //
            // (1) Leading whitespace of the raw tail (`/ DISPLAY ONLY`
            //     would put the first span on the space, not `D`).
            //     `tail_lead` captures the count.
            //
            // (2) Trailing whitespace of an inner part before its
            //     following `/` (`NF / OC` — `tail.split('/')` yields
            //     `["NF ", " OC"]`; cursor advancement on iter-1 must
            //     use the UNTRIMMED `"NF "` length to land iter-2's
            //     `part_abs` on the `O`, not one byte early).
            //
            // The trailing whitespace of the WHOLE raw_tail is
            // stripped by the outer `.trim()`; it's never observable
            // inside `tail.split('/')` and so doesn't affect cursor
            // math. Copilot review on PR #445 caught both (1) and
            // (2) in both `parse_rel_to_with_spans` and
            // `parse_display_only_with_spans`.
            let raw_tail = &trimmed[slash_pos + 1..];
            let tail_lead = raw_tail.len() - raw_tail.trim_start().len();
            let tail = raw_tail.trim();

            // Parse the country part (may be empty if the slash is leading).
            if !country_part.is_empty() {
                if tokens.is_country_code(country_part) {
                    if let Some(t) = CountryCode::try_new(country_part.as_bytes()) {
                        let span = Span::new(abs_start, abs_start + country_part.len());
                        countries.push(ParsedRelToEntry::new(t, country_part, span));
                        token_spans.push(TokenSpan {
                            kind: TokenKind::RelToTrigraph,
                            span,
                            text: country_part.into(),
                        });
                    }
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span: Span::new(abs_start, abs_start + country_part.len()),
                        text: country_part.into(),
                    });
                }
            }

            // Parse each `/`-separated tail token as a dissem or non-IC control.
            let tail_base = abs_start + slash_pos + 1 + tail_lead;
            let mut tail_cursor = 0usize;
            for part in tail.split('/') {
                let part_trim_lead = part.len() - part.trim_start().len();
                let untrimmed_len = part.len();
                let part = part.trim();
                let part_abs = tail_base + tail_cursor + part_trim_lead;
                // Cursor advances by the UNTRIMMED segment length plus
                // the `/` delimiter — using `part.len()` (trimmed)
                // would drop trailing whitespace of one part before
                // the next slash and mis-anchor subsequent spans.
                tail_cursor += untrimmed_len + 1; // +1 for `/`
                if part.is_empty() {
                    continue;
                }
                // DISPLAY ONLY commingling is now handled at the
                // OUTER level by the `find_display_only_slash_boundary`
                // pre-scan + suffix parse (see end of this function).
                // The boundary detection guarantees that no
                // `/DISPLAY ONLY [LIST]` sequence ever reaches this
                // inner slash-tail loop — the boundary `/` is excluded
                // from the comma-split scope and the DISPLAY ONLY
                // suffix is parsed in one piece (with its own commas)
                // after the REL TO loop completes. Handling DISPLAY
                // ONLY inside this inner loop would misclassify the
                // multi-country case
                // `REL TO USA, IRQ/DISPLAY ONLY AFG, NATO` because the
                // outer comma-split would already have chopped NATO
                // into a separate entry (see issue #445).
                if let Some(ctrl) =
                    DissemControl::parse(part).or_else(|| parse_dissem_full_form(part))
                {
                    let span = Span::new(part_abs, part_abs + part.len());
                    trailing_dissem.push(ParsedDissem::new(ctrl, part, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::DissemControl,
                        span,
                        text: part.into(),
                    });
                } else if let Some(nic) = parse_non_ic_full_form(part) {
                    let span = Span::new(part_abs, part_abs + part.len());
                    trailing_non_ic.push(ParsedNonIcDissem::new(nic, part, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::NonIcDissem,
                        span,
                        text: part.into(),
                    });
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span: Span::new(part_abs, part_abs + part.len()),
                        text: part.into(),
                    });
                }
            }
            continue;
        }

        if !tokens.is_country_code(trimmed) {
            // Issue #233: emit an Unknown span for unrecognized
            // entries inside a REL TO block instead of silently
            // dropping them. The decoder's
            // ``DecoderRecognizer::recognize`` step 3a rejects any
            // candidate whose strict parse leaves Unknown spans,
            // which is what makes the fuzzy-trigraph expansion
            // (``try_rel_to_fuzzy_trigraph_candidates``) win the
            // score contest: the original "drop USB" candidate now
            // carries an Unknown span and is filtered out, leaving
            // the corpus-weighted log-prior to break ties between
            // the surviving fuzzy alternates (USA, UZB, …).
            //
            // Strict-path callers still see a clean ``rel_to`` slice
            // — the Unknown span is metadata for the decoder filter,
            // not a parser failure. Existing rules that walk
            // ``token_spans`` already handle ``TokenKind::Unknown``
            // (see E030 sar-indicator-repeat for the analogous
            // pattern at line ~263).
            token_spans.push(TokenSpan {
                kind: TokenKind::Unknown,
                span: Span::new(abs_start, abs_start + trimmed.len()),
                text: trimmed.into(),
            });
            continue;
        }
        // Issue #183: drop the historical `b.len() != 3` gate that
        // silently dropped tetragraphs (`FVEY`, `NATO`, `ACGU`, …)
        // and the longer registered codes (`EU`, `AUSTRALIA_GROUP`)
        // from `rel_to`. `is_country_code` already covers the full
        // registered CVE recognition surface, including trigraphs,
        // tetragraphs, and longer special forms such as `EU` and
        // `AUSTRALIA_GROUP`; `CountryCode::try_new` accepts
        // 2..=16-byte codes in the CAPCO byte set, so any code that
        // passed `is_country_code` will also pass `try_new` here.
        let Some(t) = CountryCode::try_new(trimmed.as_bytes()) else {
            continue;
        };
        let span = Span::new(abs_start, abs_start + trimmed.len());
        countries.push(ParsedRelToEntry::new(t, trimmed, span));
        token_spans.push(TokenSpan {
            kind: TokenKind::RelToTrigraph,
            span,
            text: trimmed.into(),
        });
    }

    // Parse the DISPLAY ONLY suffix (everything after the
    // boundary slash) as a complete DISPLAY ONLY block. The
    // boundary `b` points at the `/`; the DISPLAY ONLY keyword
    // begins at `b + 1` modulo any tolerated whitespace, which
    // `parse_display_only_with_spans` resolves via its own
    // prefix-strip logic.
    if let Some(b) = display_only_boundary {
        // Skip the boundary `/` AND any tolerated whitespace
        // between `/` and `DISPLAY ONLY`. The absolute offset of
        // `DISPLAY ONLY` within the source is computed so the
        // emitted `DisplayOnlyBlock` span lands on the `D`, not
        // on the slash or its trailing whitespace.
        let raw_suffix = &after_rel[b + 1..];
        let ws_skip = raw_suffix.len() - raw_suffix.trim_start().len();
        let suffix = raw_suffix.trim_end();
        let suffix_offset_in_block = prefix_skip + b + 1 + ws_skip;
        let suffix_text = &suffix[ws_skip..];
        let suffix_abs = block_offset + suffix_offset_in_block;
        token_spans.push(TokenSpan {
            kind: TokenKind::DisplayOnlyBlock,
            span: Span::new(suffix_abs, suffix_abs + suffix_text.len()),
            text: suffix_text.into(),
        });
        let parsed_do = parse_display_only_with_spans(suffix_text, suffix_abs, tokens, token_spans);
        trailing_display_only.extend(parsed_do.countries);
        // Trailing controls inside the DISPLAY ONLY block surface
        // into the outer caller's dissem / non-IC axes — these are
        // the §H.8 p164 "may be commingled" cases that include both
        // REL TO and DISPLAY ONLY plus a tail dissem control.
        trailing_dissem.extend(parsed_do.trailing_dissem);
        trailing_non_ic.extend(parsed_do.trailing_non_ic);
    }

    RelToParseResult {
        countries,
        trailing_dissem,
        trailing_non_ic,
        trailing_display_only,
    }
}

/// Locate the byte position of the first `/` that introduces a
/// `DISPLAY ONLY [LIST]` commingling tail inside a REL TO block.
/// Returns `None` if no such commingling exists (i.e., the input is
/// a plain REL TO list, possibly with trailing simple dissem
/// controls like `/NF` which do NOT trigger this).
///
/// The match is gated on a word-boundary check after
/// `DISPLAY ONLY` so suffixes like `DISPLAY ONLYISH` cannot
/// false-positive. Whitespace between the `/` and the
/// `DISPLAY ONLY` keyword is tolerated to match the parser's
/// existing within-category-separator relaxation
/// (`crates/core/tests/separator_spans.rs`); CAPCO-2016 §A.6 p16
/// forbids interjected whitespace but real-world authors drift.
fn find_display_only_slash_boundary(s: &str) -> Option<usize> {
    const KEYWORD: &str = "DISPLAY ONLY";
    let bytes = s.as_bytes();
    let mut search_from = 0usize;
    while search_from < bytes.len() {
        let slash_rel = s[search_from..].find('/')?;
        let slash_pos = search_from + slash_rel;
        // Skip ASCII whitespace after the slash to tolerate
        // drift like `/ DISPLAY ONLY` or `/\tDISPLAY ONLY`.
        // Uses `is_ascii_whitespace` to match the parser's
        // within-category-separator tolerance in
        // [`split_slash_with_separator_offsets`]; a
        // literal-space-only check would be inconsistent with the
        // rest of the parser's whitespace handling.
        let mut after_slash = slash_pos + 1;
        while after_slash < bytes.len() && bytes[after_slash].is_ascii_whitespace() {
            after_slash += 1;
        }
        if s[after_slash..].starts_with(KEYWORD) {
            let after_kw = after_slash + KEYWORD.len();
            // Word boundary: end-of-string, ASCII whitespace, or
            // another `/`. Matches the same predicate as the
            // post-slash skip above so `DISPLAY ONLY\tAFG` and
            // `DISPLAY ONLY AFG` both word-boundary correctly.
            if after_kw >= bytes.len()
                || bytes[after_kw].is_ascii_whitespace()
                || bytes[after_kw] == b'/'
            {
                return Some(slash_pos);
            }
        }
        search_from = slash_pos + 1;
    }
    None
}

/// Return type for [`parse_display_only_with_spans`].
///
/// Parallel to [`RelToParseResult`]: DISPLAY ONLY shares the
/// comma-separated country-list grammar with REL TO per CAPCO-2016
/// §H.8 p163, so the parse shape is identical. Trailing
/// `/<control>` controls are preserved for parity with the REL TO
/// path even though §H.8 p164 forbids commingling DISPLAY ONLY with
/// other dissem controls "unless consistent with IC directives" —
/// the violation case is a rule concern (E054 / E055), not a parser
/// concern. The caller drains each field via
/// `Vec::extend(IntoIterator)`.
pub(super) struct DisplayOnlyParseResult<'src> {
    pub(super) countries: SmallVec<[ParsedDisplayOnlyEntry<'src>; 4]>,
    pub(super) trailing_dissem: SmallVec<[ParsedDissem<'src>; 2]>,
    pub(super) trailing_non_ic: SmallVec<[ParsedNonIcDissem<'src>; 2]>,
}

/// Span-aware parse of a `DISPLAY ONLY ...` block. Records one
/// [`TokenKind::DisplayOnlyTrigraph`] span per recognized country
/// code plus a single [`TokenKind::DisplayOnlyBlock`] span over the
/// whole input (the caller emits the block span; this function only
/// emits the per-entry spans).
///
/// Grammar mirrors [`parse_rel_to_with_spans`] per CAPCO-2016 §H.8
/// p163 (DISPLAY ONLY: comma-separated trigraphs + tetragraphs) and
/// §H.8 p150-151 (REL TO: same comma-separated list shape). The
/// only structural difference is the prefix: `DISPLAY ONLY ` vs
/// `REL TO ` / `REL `.
///
/// `block_offset` is the absolute byte offset of `block` within the
/// original source buffer.
pub(super) fn parse_display_only_with_spans<'src>(
    block: &'src str,
    block_offset: usize,
    tokens: &dyn TokenSet,
    token_spans: &mut SmallVec<[TokenSpan; 16]>,
) -> DisplayOnlyParseResult<'src> {
    // Skip the "DISPLAY ONLY" prefix. The trailing-space variant
    // (`DISPLAY ONLY ` with a following country list) is the common
    // case; the exact-match variant (`DISPLAY ONLY` with no list)
    // is admitted defensively even though §H.8 p163 always shows
    // the marking with a `[LIST]` parameter.
    let prefix_skip = if let Some(rest) = block.strip_prefix("DISPLAY ONLY ") {
        block.len() - rest.len()
    } else if block == "DISPLAY ONLY" {
        block.len()
    } else {
        0
    };
    let after_prefix = &block[prefix_skip..];

    let mut countries: SmallVec<[ParsedDisplayOnlyEntry<'src>; 4]> = SmallVec::new();
    let mut trailing_dissem: SmallVec<[ParsedDissem<'src>; 2]> = SmallVec::new();
    let mut trailing_non_ic: SmallVec<[ParsedNonIcDissem<'src>; 2]> = SmallVec::new();

    let mut cursor = 0usize;
    for entry in after_prefix.split(',') {
        let entry_start_in_after = cursor;
        cursor += entry.len() + 1;

        let trim_lead = entry.len() - entry.trim_start().len();
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let abs_start = block_offset + prefix_skip + entry_start_in_after + trim_lead;

        // Slash-tail handling parallels `parse_rel_to_with_spans`. §H.8
        // p164 forbids commingling DISPLAY ONLY with other dissem
        // controls outside specific IC directives, so this branch is
        // structurally accommodating but the violation case is for the
        // rule layer (E054 / E055) to surface.
        if let Some(slash_pos) = trimmed.find('/') {
            let country_part = trimmed[..slash_pos].trim();
            // Track raw_tail's leading whitespace separately so the
            // `tail_base` offset for the per-part spans lands on the
            // first non-whitespace byte of the tail rather than on
            // the space after `/`. See `parse_rel_to_with_spans` for
            // the matching pattern + the rationale.
            let raw_tail = &trimmed[slash_pos + 1..];
            let tail_lead = raw_tail.len() - raw_tail.trim_start().len();
            let tail = raw_tail.trim();

            if !country_part.is_empty() {
                if tokens.is_country_code(country_part) {
                    if let Some(t) = CountryCode::try_new(country_part.as_bytes()) {
                        let span = Span::new(abs_start, abs_start + country_part.len());
                        countries.push(ParsedDisplayOnlyEntry::new(t, country_part, span));
                        token_spans.push(TokenSpan {
                            kind: TokenKind::DisplayOnlyTrigraph,
                            span,
                            text: country_part.into(),
                        });
                    }
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span: Span::new(abs_start, abs_start + country_part.len()),
                        text: country_part.into(),
                    });
                }
            }

            let tail_base = abs_start + slash_pos + 1 + tail_lead;
            let mut tail_cursor = 0usize;
            for part in tail.split('/') {
                let part_trim_lead = part.len() - part.trim_start().len();
                let untrimmed_len = part.len();
                let part = part.trim();
                let part_abs = tail_base + tail_cursor + part_trim_lead;
                // Use UNTRIMMED segment length so trailing whitespace
                // before a `/` doesn't mis-anchor subsequent spans;
                // mirrors the matching pattern in
                // `parse_rel_to_with_spans`.
                tail_cursor += untrimmed_len + 1; // +1 for `/`
                if part.is_empty() {
                    continue;
                }
                if let Some(ctrl) =
                    DissemControl::parse(part).or_else(|| parse_dissem_full_form(part))
                {
                    let span = Span::new(part_abs, part_abs + part.len());
                    trailing_dissem.push(ParsedDissem::new(ctrl, part, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::DissemControl,
                        span,
                        text: part.into(),
                    });
                } else if let Some(nic) = parse_non_ic_full_form(part) {
                    let span = Span::new(part_abs, part_abs + part.len());
                    trailing_non_ic.push(ParsedNonIcDissem::new(nic, part, span));
                    token_spans.push(TokenSpan {
                        kind: TokenKind::NonIcDissem,
                        span,
                        text: part.into(),
                    });
                } else {
                    token_spans.push(TokenSpan {
                        kind: TokenKind::Unknown,
                        span: Span::new(part_abs, part_abs + part.len()),
                        text: part.into(),
                    });
                }
            }
            continue;
        }

        if !tokens.is_country_code(trimmed) {
            // Emit Unknown for unrecognized country entries — parallel
            // to the REL TO behavior; lets E008 / the decoder
            // dispatcher surface a fuzzy-correction candidate instead
            // of silently dropping the entry.
            token_spans.push(TokenSpan {
                kind: TokenKind::Unknown,
                span: Span::new(abs_start, abs_start + trimmed.len()),
                text: trimmed.into(),
            });
            continue;
        }
        let Some(t) = CountryCode::try_new(trimmed.as_bytes()) else {
            continue;
        };
        let span = Span::new(abs_start, abs_start + trimmed.len());
        countries.push(ParsedDisplayOnlyEntry::new(t, trimmed, span));
        token_spans.push(TokenSpan {
            kind: TokenKind::DisplayOnlyTrigraph,
            span,
            text: trimmed.into(),
        });
    }
    DisplayOnlyParseResult {
        countries,
        trailing_dissem,
        trailing_non_ic,
    }
}
