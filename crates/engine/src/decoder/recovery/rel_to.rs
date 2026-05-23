//! REL TO recovery.
//!
//! Three sub-passes:
//!
//! 1. **Structural repair** (header normalize, entry normalize, block
//!    fix) — fixes `RELT O` / `REL OT` / `A US` / `AU,S` shape
//!    deviations.
//! 2. **Trigraph fuzzy expansion** — runs the fuzzy matcher against
//!    the trigraph dictionary inside REL TO entries.
//! 3. **USA injection** — adds the missing leading `USA` country code
//!    when a REL TO block starts with a non-USA trigraph.

use marque_core::fuzzy::FuzzyVocabMatcher;
use marque_ism::{CapcoTokenSet, token_set::TokenSet as _};
use marque_rules::confidence::FeatureId;

use super::super::types::FeatureEntry;

// ---------------------------------------------------------------------------
// REL TO structural repair
// ---------------------------------------------------------------------------

/// REL TO structural repair.
///
/// Recovers four classes of REL TO structural typos that produce no
/// valid REL TO block in the strict parse path. All four are
/// **structural** (literal-shape) repairs, not vocabulary-based fuzzy
/// guesses — they fire only when the observed pattern is invalid
/// CAPCO AND the corrected pattern is unambiguously the intended form.
/// The riskier per-trigraph fuzzy-correction cluster (e.g.,
/// `USB → USA`, `AUT → AUS`) is deferred to issue #186 because it
/// requires corpus-weighted priors + block-level CAPCO §H.8
/// invariants to disambiguate safely.
///
/// # Patterns
///
/// 1. **Header transposition** — `REL OT ` → `REL TO `. The CAPCO
///    `REL` token has exactly two valid extensions (`REL TO` and
///    `RELIDO`); `REL OT` cannot appear in any valid CAPCO marking,
///    so the literal-bytes replacement is collision-free.
///
/// 2. **Header token-boundary** — `RELT O ` → `REL TO `. `RELT` is
///    not a CVE token, and `T O` as adjacent single-letter tokens
///    has no valid CAPCO meaning. The replacement reconstructs the
///    intended `REL TO ` header by migrating the trailing `T` from
///    `RELT` to the start of `O`.
///
/// 3. **Entry token-boundary** — `,A US,` → `,AUS,` (within a
///    REL TO block). A 1-letter + space + 2-letter sequence between
///    commas only fires when the joined 3-letter string is a known
///    trigraph (`is_trigraph` check) AND the 1-letter alone is not a
///    trigraph. The trigraph guard is what makes this safe — without
///    it, `,A B,` → `,AB,` would fire for any combination, but with
///    it the only joins that survive are those that round-trip
///    through the strict REL TO parser as valid trigraphs.
///
/// 4. **Entry comma misplacement** — `AU,S ` → `AUS, ` (within a
///    REL TO block). A 2-letter run + comma + 1-letter + space only
///    fires when the joined 3-letter string is a known trigraph AND
///    the 2-letter run alone is not. Same trigraph guard as
///    pattern 3 — the structural transform requires the corrected
///    output to be a valid trigraph.
///
/// # Scope (PR 9)
///
/// Patterns 1 and 2 affect the literal `REL TO` header and run
/// regardless of what follows. Patterns 3 and 4 require a `REL TO `
/// header in the input — they scan from each `REL TO ` substring
/// forward to the next `//` (or end of text) and only operate on
/// comma-separated entries within that block.
///
/// All four transforms are conservative: their false-positive risk
/// is bounded by the literal patterns not appearing in any valid
/// CAPCO text (patterns 1, 2) or by the `is_trigraph` guard
/// rejecting joins that aren't real country codes (patterns 3, 4).
/// The trigraph dictionary itself is the source of authority — no
/// new vocabulary is invented.
///
/// Returns `None` when no pattern matched. Allocation behavior:
///
/// - Inputs with no `REL` substring short-circuit before any work.
/// - Inputs with `REL` but no header-typo pattern run the header
///   walk allocation-free; the entry-level pass then short-circuits
///   on inputs lacking a literal `REL TO ` anchor.
/// - Inputs containing `REL TO ` in canonical form walk the entries
///   without allocating until a fix actually fires.
///
/// Allocation only occurs once a pattern produces a fixed string.
pub(crate) fn try_rel_to_structural_repair(text: &str) -> Option<String> {
    // Cheap pre-check: if `REL` doesn't appear at all, no repair is
    // possible. Saves the byte-walk cost on the overwhelmingly common
    // case where the input has no REL block.
    if !text.contains("REL") {
        return None;
    }

    let mut working: Option<String> = None;
    let mut any_change = false;

    // Patterns 1 and 2: header normalization. Apply first so the
    // entry-level scan in patterns 3 and 4 sees a canonical `REL TO `
    // header to anchor on.
    if let Some(normalized) = try_rel_to_header_normalize(text) {
        working = Some(normalized);
        any_change = true;
    }

    // Patterns 3 and 4: entry-level fixes. Operate on the
    // header-normalized text when patterns 1 or 2 fired, otherwise on
    // the raw input.
    let entry_input: &str = working.as_deref().unwrap_or(text);
    if let Some(entry_fixed) = try_rel_to_entry_normalize(entry_input) {
        working = Some(entry_fixed);
        any_change = true;
    }

    if any_change { working } else { None }
}

/// Patterns 1 and 2 — header normalization.
///
/// Walks `text` once, replacing each occurrence of `REL OT ` and
/// `RELT O ` (each at a token boundary) with `REL TO `. Lazy-allocates
/// the output string only on the first match — inputs that contain
/// `REL` but no header-typo pattern (the common case for canonical
/// `REL TO USA, AUS, GBR` markings) walk the bytes without ever
/// allocating.
///
/// The "token boundary" check (`at_boundary`) prevents matches
/// embedded inside a longer alphanumeric run. Without it `XREL OT`
/// would match the substring `REL OT` even though the leading `X`
/// makes the whole thing a single 6-character token, not a `REL`
/// header at all.
pub(crate) fn try_rel_to_header_normalize(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut result: Option<String> = None;
    let mut last_copied: usize = 0;
    let mut i = 0;

    while i < bytes.len() {
        let at_boundary =
            i == 0 || matches!(bytes[i - 1], b'/' | b'(' | b' ' | b'\t' | b'\n' | b'\r');

        if at_boundary && i + 7 <= bytes.len() {
            let window = &bytes[i..i + 7];
            // Pattern A (transposition): `REL OT ` → `REL TO `.
            // Pattern B (token-boundary): `RELT O ` → `REL TO `.
            // Both patterns are exactly 7 bytes; the same 7-byte
            // window is compared against each full literal
            // explicitly, so a single window read covers both.
            if window == b"REL OT " || window == b"RELT O " {
                let r = result.get_or_insert_with(|| String::with_capacity(text.len()));
                r.push_str(&text[last_copied..i]);
                r.push_str("REL TO ");
                last_copied = i + 7;
                i = last_copied;
                continue;
            }
        }

        let ch = text[i..]
            .chars()
            .next()
            .expect("byte index must remain on a char boundary");
        i += ch.len_utf8();
    }

    result.map(|mut r| {
        r.push_str(&text[last_copied..]);
        r
    })
}

/// Patterns 3 and 4 — entry-level normalization within REL TO blocks.
///
/// Scans `text` for each `REL TO ` substring and processes the
/// comma-separated entries that follow until the next `//` (or end of
/// text). Two patterns apply per entry pair:
///
/// - **Token-boundary** — within a single entry, `<single-upper> <two-upper>`
///   is replaced with the joined 3-letter trigraph when the join is a
///   known trigraph and the 1-letter prefix alone is not.
///
/// - **Comma misplacement** — across an entry pair,
///   `<2-upper>,<1-upper><space>...` (entry N ends with two letters,
///   entry N+1 starts with one letter followed by a space and then
///   content) is replaced with `<3-upper joined>,` and the leading
///   character is stripped from entry N+1, when the join is a known
///   trigraph and the 2-letter prefix alone is not. The space guard
///   (the 1-upper must be followed by ASCII space) is what
///   distinguishes the misplacement shape from a legitimate
///   shorter-than-3 entry typo and is enforced by `fix_rel_to_block`.
///
/// Both patterns require the corrected output to be a known trigraph
/// (`CapcoTokenSet::is_trigraph`). The trigraph dictionary is the
/// arbiter of "valid country code" — no fuzzy guessing.
pub(crate) fn try_rel_to_entry_normalize(text: &str) -> Option<String> {
    // Cheap pre-check: entry-level patterns 3 and 4 only fire inside a
    // `REL TO ` block, so `apply_rel_to_entry_pass` cannot match
    // without that anchor. Skip the `to_owned()` allocation entirely
    // when the input has no `REL TO ` substring (the common path for
    // canonical inputs and for non-REL-TO segments of the broader
    // structural-repair caller).
    if !text.contains("REL TO ") {
        return None;
    }

    let token_set = CapcoTokenSet;
    let mut any_change = false;
    let mut current: Option<String> = None;

    // Loop until no further fix fires. Most inputs converge in one
    // pass; the loop guards against the rare case where fixing one
    // pattern exposes another (e.g., a comma misplacement that ends a
    // block adjacent to a token-boundary pattern in the next entry).
    // First iteration borrows `text`; subsequent iterations re-pass the
    // previously rewritten `String` so the only allocation is the one
    // produced by the first successful fix (and any further passes).
    loop {
        let input: &str = current.as_deref().unwrap_or(text);
        match apply_rel_to_entry_pass(input, &token_set) {
            Some(rewritten) => {
                current = Some(rewritten);
                any_change = true;
            }
            None => break,
        }
    }

    if any_change { current } else { None }
}

/// Single pass of REL TO entry normalization. Returns the rewritten
/// text on first fix, or `None` if no pattern matched.
pub(crate) fn apply_rel_to_entry_pass(text: &str, token_set: &CapcoTokenSet) -> Option<String> {
    let mut search_start = 0;
    while let Some(rel_pos) = text[search_start..].find("REL TO ") {
        let header_end = search_start + rel_pos + "REL TO ".len();
        // Block ends at the next `//` (start of next category) or end
        // of text. The `//` boundary is always 2 bytes; we exclude it
        // from the block contents.
        let block_end = text[header_end..]
            .find("//")
            .map(|p| header_end + p)
            .unwrap_or(text.len());
        let block = &text[header_end..block_end];

        if let Some((rel_local_offset, fixed_block)) = fix_rel_to_block(block, token_set) {
            let mut result = String::with_capacity(text.len());
            result.push_str(&text[..header_end]);
            result.push_str(&fixed_block);
            result.push_str(&text[block_end..]);
            // Suppress unused-variable warning when the helper returns
            // a fix — `rel_local_offset` is reserved for a future
            // localized-emit optimization but not needed today since
            // we rebuild the full text.
            let _ = rel_local_offset;
            return Some(result);
        }

        search_start = block_end;
    }
    None
}

/// Walk the comma-separated entries of one REL TO block; apply
/// pattern 3 (token-boundary inside an entry) and pattern 4 (comma
/// misplaced between adjacent entries) on first match. Returns
/// `(local_offset, rewritten_block)` for the first fix, or `None` if
/// the block is already canonical.
///
/// `local_offset` is the byte offset within `block` where the fix
/// landed; reserved for future localized emit optimizations.
pub(crate) fn fix_rel_to_block(block: &str, token_set: &CapcoTokenSet) -> Option<(usize, String)> {
    // Collect entries with their byte offsets within the block so a
    // fix can be emitted with precise positioning.
    let mut entries: Vec<(usize, &str)> = Vec::new();
    let mut cursor = 0;
    for entry in block.split(',') {
        entries.push((cursor, entry));
        cursor += entry.len() + 1; // +1 for the comma separator
    }

    // Pattern 3: token-boundary inside a single entry.
    // `<lead-ws><single-upper> <two-upper><trail-ws>` → joined trigraph.
    for (entry_offset, entry) in &entries {
        let trimmed = entry.trim();
        // Need exactly 4 chars: `A US` shape. Anything else (3, 5, etc.)
        // is either canonical or a different recovery shape.
        if trimmed.len() != 4 {
            continue;
        }
        let bytes = trimmed.as_bytes();
        if !bytes[0].is_ascii_uppercase()
            || bytes[1] != b' '
            || !bytes[2].is_ascii_uppercase()
            || !bytes[3].is_ascii_uppercase()
        {
            continue;
        }
        let joined = format!(
            "{}{}{}",
            bytes[0] as char, bytes[2] as char, bytes[3] as char
        );
        if !token_set.is_trigraph(&joined) {
            continue;
        }
        // Defensive: don't fire if the 1-letter prefix is itself a
        // trigraph (no real CAPCO trigraph is 1-letter, but guard
        // anyway against future schema changes).
        let one_letter = std::str::from_utf8(&bytes[..1]).expect("ASCII upper");
        if token_set.is_trigraph(one_letter) {
            continue;
        }

        // Rebuild the block: replace the 4-char entry contents with
        // the 3-char joined trigraph, preserving any leading/trailing
        // whitespace inside the entry.
        // entry = lead_ws + trimmed + trail_ws; replace `trimmed`
        // (4 chars) with `joined` (3 chars), preserving the
        // surrounding whitespace verbatim.
        let lead_ws_len = entry.len() - entry.trim_start().len();
        let mut rewritten_entry = String::with_capacity(entry.len() - 1);
        rewritten_entry.push_str(&entry[..lead_ws_len]);
        rewritten_entry.push_str(&joined);
        rewritten_entry.push_str(&entry[lead_ws_len + trimmed.len()..]);

        let mut result = String::with_capacity(block.len());
        result.push_str(&block[..*entry_offset]);
        result.push_str(&rewritten_entry);
        result.push_str(&block[*entry_offset + entry.len()..]);
        return Some((*entry_offset, result));
    }

    // Pattern 4: comma misplaced between entries.
    // entries[i] = `<2-upper>` (trimmed) AND
    // entries[i+1] = `<1-upper><space><rest>` (trimmed) AND
    // joined 3-letter is a trigraph AND 2-letter alone is not.
    for i in 0..entries.len().saturating_sub(1) {
        let (left_off, left_entry) = &entries[i];
        let (right_off, right_entry) = &entries[i + 1];
        let left_trim = left_entry.trim();
        let right_trim_start = right_entry.trim_start();
        if left_trim.len() != 2 || !left_trim.chars().all(|c| c.is_ascii_uppercase()) {
            continue;
        }
        let right_bytes = right_trim_start.as_bytes();
        if right_bytes.len() < 2 || !right_bytes[0].is_ascii_uppercase() || right_bytes[1] != b' ' {
            continue;
        }
        let joined = format!("{}{}", left_trim, right_bytes[0] as char);
        if !token_set.is_trigraph(&joined) {
            continue;
        }
        if token_set.is_trigraph(left_trim) {
            // 2-letter alone is already a trigraph (e.g., EU); the
            // comma might be intentional. Skip.
            continue;
        }

        // Rebuild: left entry becomes `<lead-ws><joined>`, right
        // entry becomes ` <rest-after-first-char-and-space>` (we
        // strip the first char and the space, prepend a single
        // canonical space).
        let left_lead = left_entry.len() - left_entry.trim_start().len();
        let mut new_left = String::with_capacity(left_entry.len() + 1);
        new_left.push_str(&left_entry[..left_lead]);
        new_left.push_str(&joined);

        let right_lead = right_entry.len() - right_trim_start.len();
        // Skip the first char and the following space.
        let after_first = &right_trim_start[2..];
        let mut new_right = String::with_capacity(right_entry.len());
        new_right.push_str(&right_entry[..right_lead]);
        new_right.push(' ');
        new_right.push_str(after_first);

        // Emit: block[..left_off] + new_left + ',' + new_right + block[right_off+right_entry.len()..]
        let mut result = String::with_capacity(block.len() + 1);
        result.push_str(&block[..*left_off]);
        result.push_str(&new_left);
        result.push(',');
        result.push_str(&new_right);
        result.push_str(&block[*right_off + right_entry.len()..]);
        return Some((*left_off, result));
    }

    None
}

// ---------------------------------------------------------------------------
// REL TO trigraph fuzzy expansion
// ---------------------------------------------------------------------------

/// Emit one canonical-byte alternate per fuzzy candidate for each
/// unknown 3- or 4-char REL TO entry.
///
/// The standard fuzzy path in [`fuzzy_correct_tokens`] operates against
/// the [`CapcoTokenSet::correction_vocab`] slice, which deliberately
/// excludes country trigraphs (the design comment on `ALL_CVE_TOKENS`
/// in `crates/ism/build.rs` calls this out — country codes live
/// exclusively in [`marque_ism::TRIGRAPHS`] and are reached through
/// [`CapcoTokenSet::is_trigraph`]). So a typo'd 3-char REL TO entry
/// like `USB` gets no correction from the standard pass — there's
/// nothing in the vocab to match it against. The strict parser then
/// emits a `TokenKind::Unknown` for the entry (issue #233 change in
/// `parse_rel_to_with_spans`), and the dispatcher's step 3a rejects
/// the "drop USB" candidate.
///
/// With the original candidate filtered out, this function provides
/// the alternates the dispatcher chooses between: it walks each
/// `REL TO ` block in `text`, finds 3- or 4-char comma-separated
/// entries that aren't already valid trigraphs/tetragraphs, asks the
/// trigraph-vocab matcher for all candidates within the edit-distance
/// bound, and emits one alternate text per candidate (with the
/// substitution applied in-place).
///
/// Each emitted alternate carries an `EditDistance1` /
/// `EditDistance2` feature (paired with the candidate's distance) so
/// the audit trail records the fuzzy work. The caller pushes a
/// `BaseRateCommonMarking` feature acknowledging the trigraph-prior
/// contribution. The decoder's `score_candidate` later sums the
/// trigraph-prior contribution over the parsed `rel_to` slice; the
/// popular-vs-rare log-prior delta (e.g., `log_prior(USA) -
/// log_prior(UZB)` ≈ +7 nats) decides which alternate wins the
/// `UNAMBIGUOUS_LOG_MARGIN` (~1.6 nat) contest.
///
/// **Scope**: 3-char (trigraph) and 4-char (tetragraph) ASCII
/// uppercase entries only. Two-letter entries (`EU`) are below
/// `MIN_FUZZY_LEN`; longer multi-char entries (`AUSTRALIA_GROUP`)
/// have low fuzzy-tie risk because their lengths rarely collide.
/// Only fires when the entry token is NOT already a valid
/// trigraph/tetragraph — so `AUT`, `UZB`, `FVEY`, `ACGU`, `ISAF`
/// in legitimate use pass through unchanged. 4-char scope added to
/// recover coalition-shorthand typos (`FVYE` → `FVEY`,
/// `SGAF` → `ISAF`); issue #246.
///
/// **CAPCO authority**: REL TO syntax is defined in CAPCO-2016 §H.8.
/// The trigraph/tetragraph dictionary itself comes from the ODNI CVE
/// schema in `CVEnumISMCATRelTo.xsd`, baked into
/// [`CapcoTokenSet::is_trigraph`] and into the
/// [`marque_ism::TRIGRAPHS`] slice this function fuzzy-matches against.
pub(crate) fn try_rel_to_fuzzy_trigraph_candidates(
    text: &str,
    trigraph_matcher: &FuzzyVocabMatcher<'_>,
) -> Vec<(String, FeatureEntry)> {
    let token_set = CapcoTokenSet;
    let mut out: Vec<(String, FeatureEntry)> = Vec::new();

    let mut search_start = 0;
    while let Some(rel_pos) = text[search_start..].find("REL TO ") {
        let header_end = search_start + rel_pos + "REL TO ".len();
        // Block ends at the EARLIEST of: `//` (next category), `\n`
        // (banner/CAB candidates from `Scanner::scan_banners` arrive
        // as full lines, so a REL TO line can have trailing prose
        // beyond the marking), or `)` (portion-form close). CAPCO
        // §H.8 / §A authority: `//` is the category separator; `,`
        // separates entries within the REL TO category itself.
        // Mirrors the corpus analyzer's terminator priority in
        // `tools/corpus-analysis/analyze.py` (`_extract_rel_to_trigraphs`).
        let tail = &text[header_end..];
        let block_len = ["//", "\n", ")"]
            .iter()
            .filter_map(|sep| tail.find(sep))
            .min()
            .unwrap_or(tail.len());
        let block_end = header_end + block_len;
        let block = &text[header_end..block_end];

        // Walk the comma-separated entries with their byte offsets.
        let mut cursor = 0usize;
        for entry in block.split(',') {
            let entry_start = cursor;
            let entry_end = cursor + entry.len();
            cursor = entry_end + 1; // skip the comma

            let trimmed = entry.trim();
            // 3-char (trigraph) or 4-char (tetragraph) ASCII-uppercase
            // entries only — see fn doc for scope rationale.
            let tlen = trimmed.len();
            if (tlen != 3 && tlen != 4) || !trimmed.bytes().all(|b| b.is_ascii_uppercase()) {
                continue;
            }
            // Skip already-valid trigraphs/tetragraphs (the matcher's
            // binary search would also short-circuit on a vocab hit, but
            // keeping the explicit check means a token like `FVEY`
            // appearing legitimately never gets multi-cast).
            if token_set.is_trigraph(trimmed) {
                continue;
            }

            // Bypass the standard `MIN_USEFUL_CONFIDENCE` floor:
            // for a 3-char input, distance-2 corrections sit at
            // confidence 0.40, below the default 0.45 cutoff that
            // protects the standalone fuzzy path. Issue #233's score-
            // time tiebreak (corpus-weighted trigraph priors +
            // `UNAMBIGUOUS_LOG_MARGIN`) supplies the safety the
            // confidence-floor was substituting for; without lowering
            // it here, a typo like `ASU → AUS` (plain Levenshtein
            // distance 2) never reaches the scorer.
            let mut candidates = trigraph_matcher.correct_all_with_floor(trimmed, 0.0);
            if candidates.is_empty() {
                continue;
            }

            // Drop candidates that would duplicate a trigraph already
            // present elsewhere in this REL TO block. CAPCO-2016 §H.8
            // does not state "no duplicates" as an explicit textual
            // prohibition — the REL TO grammar (§A.6 / §H.8 p131-150)
            // describes a list of country codes ordered USA-first then
            // ascending alphabetic, which structurally implies a set of
            // distinct codes but does not forbid repetition in so many
            // words. The reason we drop duplicates here is mechanical,
            // not citational: the bag-of-tokens scorer happens to
            // *reward* duplicates (each instance adds its log-prior
            // again), so without this filter an ambiguous typo
            // adjacent to a popular trigraph could collapse to
            // "REL TO USA, USA, GBR" because USA's log-prior
            // contribution is additive. Emitting a duplicate-creating
            // candidate would therefore be structurally redundant and
            // cause the scorer to erroneously favor it. The block's
            // other entries are computed by re-walking
            // `block.split(',')` and taking the trigraph form of any
            // 3-char ASCII-uppercase entry that's in the CVE
            // recognition set.
            let other_trigraphs: Vec<&str> = block
                .split(',')
                .map(str::trim)
                .filter(|e| {
                    let elen = e.len();
                    (elen == 3 || elen == 4)
                        && e.bytes().all(|b| b.is_ascii_uppercase())
                        && *e != trimmed
                        && token_set.is_trigraph(e)
                })
                .collect();
            candidates.retain(|c| !other_trigraphs.contains(&c.token));
            if candidates.is_empty() {
                continue;
            }

            // Rank candidates by (distance, then country-code
            // log-prior). The plain Levenshtein hits for a 3-char
            // input often produce 20+ distance-2 candidates (every
            // other 3-char trigraph that shares one letter). Without
            // a prior-rank pre-filter, the K=16 attempt cap upstream
            // gets exhausted by low-prior alternates and the
            // high-prior ones get dropped. Sorting by (distance asc,
            // log-prior desc) keeps the most plausible candidates
            // first; we cap at TRIGRAPH_FUZZY_TOP_K per ambiguous
            // entry to bound the candidate-set growth.
            //
            // The cap value (4) is sized so a single ambiguous entry
            // doesn't crowd out the other decoder paths
            // (`fuzzy_corrected`, reorder, delimiter-insert, etc.):
            // 4 alternates ≤ K_MAX_CANDIDATES (8) leaves room for
            // the standard candidates the dispatcher also needs.
            const TRIGRAPH_FUZZY_TOP_K: usize = 4;
            candidates.sort_by(|a, b| {
                a.distance.cmp(&b.distance).then_with(|| {
                    let pa = marque_capco::priors::country_code_log_prior(a.token)
                        .unwrap_or(f32::NEG_INFINITY);
                    let pb = marque_capco::priors::country_code_log_prior(b.token)
                        .unwrap_or(f32::NEG_INFINITY);
                    pb.total_cmp(&pa)
                })
            });
            candidates.truncate(TRIGRAPH_FUZZY_TOP_K);

            for cand in &candidates {
                // Reconstruct the full `text` with the entry replaced.
                // The 3-char trimmed sub-slice within the entry
                // preserves any surrounding whitespace.
                let lead_ws_len = entry.len() - entry.trim_start().len();
                let trail_ws_len = entry.len() - entry.trim_end().len();
                let mut rewritten_entry = String::with_capacity(entry.len());
                rewritten_entry.push_str(&entry[..lead_ws_len]);
                rewritten_entry.push_str(cand.token);
                rewritten_entry.push_str(&entry[entry.len() - trail_ws_len..]);

                let mut alt = String::with_capacity(text.len());
                alt.push_str(&text[..header_end + entry_start]);
                alt.push_str(&rewritten_entry);
                alt.push_str(&text[header_end + entry_end..]);

                // `FeatureId` is a closed audit-schema enum (see
                // `crates/rules/src/confidence.rs` and `MARQUE_AUDIT_SCHEMA`);
                // pair each (id, delta) directly off `cand.distance`
                // so the match is total over the only two outcomes
                // `cand.distance` can take here. The standalone fuzzy
                // matcher caps results at `MAX_EDIT_DISTANCE = 2`.
                let entry = if cand.distance <= 1 {
                    FeatureEntry {
                        id: FeatureId::EditDistance1,
                        delta: -0.5,
                    }
                } else {
                    FeatureEntry {
                        id: FeatureId::EditDistance2,
                        delta: -1.2,
                    }
                };
                out.push((alt, entry));
            }
        }

        search_start = block_end;
    }

    out
}

// ---------------------------------------------------------------------------
// REL TO USA-injection for short first entries
// ---------------------------------------------------------------------------

/// Emit one canonical-byte alternate per REL TO block whose first
/// entry is a 1- or 2-character ASCII-uppercase token AND USA is not
/// otherwise present in the block. The alternate replaces that short
/// first entry with `USA`.
///
/// **Why complement to PR-A.** Issue #233's
/// [`try_rel_to_fuzzy_trigraph_candidates`] handles 3-char REL TO
/// entries: an unknown trigraph-shaped token gets fuzzy-matched
/// against the [`marque_ism::TRIGRAPHS`] vocabulary, and corpus-
/// weighted log-priors break ties at score time. That path
/// deliberately skips entries below `MIN_FUZZY_LEN = 3` (see the
/// `if trimmed.len() != 3` guard in `try_rel_to_fuzzy_trigraph_candidates`)
/// because `phf`-style fuzzy matching is unreliable on inputs that
/// short — a 2-char input is edit-distance-1 from many distinct
/// trigraphs and the mapper has no signal to break the tie.
///
/// For REL TO specifically, the §H.8 p150–151 grammar gives us a
/// stronger signal that fuzzy-matching cannot exploit: **USA must
/// always appear first**. So when we see a REL TO block whose first
/// entry is a 1- or 2-character ASCII-uppercase token, the most
/// likely intent — far above any other 3-char trigraph — is that
/// the user typed USA and dropped one or two characters. The fixture
/// at `tests/fixtures/mangled/typo/ad2bcfe3ac0b0765.json`
/// (`REL TO SA, AUS, GBR` → `REL TO USA, AUS, GBR`) is the canonical
/// case: `SA` is shape-incompatible with PR-A's 3-char floor, so
/// without this complementary path the decoder produces zero
/// candidates and the fixture fails recovery.
///
/// **CAPCO authority**: the USA-first invariant is CAPCO-2016 §H.8
/// p151: "After 'USA', list the required one or more trigraph country
/// codes in alphabetical order." E020 enforces that invariant at the
/// rule layer (via the `marque-capco`-private `canonicalize_trigraph_list`
/// helper). This decoder path operates one stage earlier — pre-strict-
/// parse, on raw text — so it does NOT call the rule-layer helper; it
/// emits a candidate text and lets the downstream pipeline (strict
/// parse + E020) verify and re-canonicalize as needed.
///
/// **Scope and guards** (mirrors PR-A's design):
///
/// - Fires only when the first entry's trimmed length is 1 or 2 ASCII
///   uppercase bytes (3-char entries belong to PR-A's domain).
/// - Skips when USA is already present elsewhere in the block — that
///   case isn't a USA-typo, it's an unrelated short prefix the user
///   may have meant differently. The block stays as-is.
/// - Skips when the block has fewer than two entries — a single
///   short entry plus nothing else doesn't fit the §H.8 p151
///   "USA + trigraph list" shape.
/// - Emits the substitution transform only — full canonicalization
///   (USA first, remaining trigraphs alphabetical, no duplicates) is
///   downstream. If the original list's tail (other than the
///   corrupted first entry) wasn't already alphabetical, E020 will
///   fire on the post-decode text and produce its own fix; if the
///   injection produced a duplicate (USA was already present in the
///   block under a different shape), the `already_has_usa` guard
///   above suppresses emit. Keeping the decoder text-level (no
///   `marque-capco` imports) avoids re-entering the rule layer
///   mid-recognition while preserving the single-source-of-truth
///   property — the canonical ordering rule lives in `marque-capco`,
///   and the decoder defers to whatever it produces post-parse.
/// - Audit signal: each candidate carries
///   [`FeatureId::BaseRateCommonMarking`] as provenance only, with
///   zero delta. This records that USA is the dominant trigraph in
///   the corpus prior without changing score or double-counting that
///   prior in the posterior. Reusing `BaseRateCommonMarking` (vs
///   introducing a new variant) keeps the audit schema closed —
///   `MARQUE_AUDIT_SCHEMA` stays at `marque-1.0`.
pub(crate) fn try_rel_to_usa_injection_candidates(text: &str) -> Vec<(String, FeatureEntry)> {
    let mut out: Vec<(String, FeatureEntry)> = Vec::new();

    let mut search_start = 0;
    while let Some(rel_pos) = text[search_start..].find("REL TO ") {
        let header_end = search_start + rel_pos + "REL TO ".len();
        // Block ends at the EARLIEST of: `//` (next category), `\n`
        // (banner/CAB candidates from `Scanner::scan_banners` arrive
        // as full lines), or `)` (portion-form close). CAPCO §H.8 /
        // §A authority: `//` is the category separator; `,` separates
        // entries within the REL TO category itself. Mirrors the
        // terminator priority in `try_rel_to_fuzzy_trigraph_candidates`
        // and the corpus analyzer's `_extract_rel_to_trigraphs`.
        let tail = &text[header_end..];
        let block_len = ["//", "\n", ")"]
            .iter()
            .filter_map(|sep| tail.find(sep))
            .min()
            .unwrap_or(tail.len());
        let block_end = header_end + block_len;
        let block = &text[header_end..block_end];

        // Walk entries with their byte offsets within the block.
        // Pre-size from comma count + 1 — typical REL TO blocks have
        // 2–6 entries, so this avoids reallocations on the common case.
        let entries: Vec<(usize, &str)> = {
            let mut v = Vec::with_capacity(block.bytes().filter(|&b| b == b',').count() + 1);
            let mut cursor = 0usize;
            for entry in block.split(',') {
                v.push((cursor, entry));
                cursor += entry.len() + 1; // +1 for the comma separator
            }
            v
        };
        if entries.len() < 2 {
            // Single-entry block: doesn't match the §H.8 p151
            // "USA + trigraph list" shape we're recovering.
            search_start = block_end;
            continue;
        }

        // First entry is the candidate USA-typo position. The
        // structural guard is shape-only — len ∈ {1, 2}, all ASCII
        // uppercase. 3-char entries fall through to PR-A. Length 0
        // (e.g., a leading comma) is already filtered.
        let (first_entry_offset, first_entry) = entries[0];
        let trimmed = first_entry.trim();
        let is_short =
            (1..=2).contains(&trimmed.len()) && trimmed.bytes().all(|b| b.is_ascii_uppercase());
        if !is_short {
            search_start = block_end;
            continue;
        }

        // Skip if USA is already present elsewhere in the block —
        // a USA-injection candidate would create a duplicate, which
        // E052 would then need to dedup. Short-
        // circuit here rather than emit-and-redup.
        let already_has_usa = entries.iter().skip(1).any(|(_, e)| e.trim() == "USA");
        if already_has_usa {
            search_start = block_end;
            continue;
        }

        // Build the substituted text. Preserve the entry's
        // surrounding whitespace (lead/trail) so the splice
        // round-trips through the strict parser the same way the
        // original would have.
        let lead_ws_len = first_entry.len() - first_entry.trim_start().len();
        let trail_ws_len = first_entry.len() - first_entry.trim_end().len();
        let mut rewritten_entry = String::with_capacity(first_entry.len() + 3);
        rewritten_entry.push_str(&first_entry[..lead_ws_len]);
        rewritten_entry.push_str("USA");
        rewritten_entry.push_str(&first_entry[first_entry.len() - trail_ws_len..]);

        let mut alt = String::with_capacity(text.len() + 3);
        alt.push_str(&text[..header_end + first_entry_offset]);
        alt.push_str(&rewritten_entry);
        alt.push_str(&text[header_end + first_entry_offset + first_entry.len()..]);

        // Audit-only provenance. The load-bearing scoring lives in
        // `score_candidate`, which sums `country_code_log_prior(USA)`
        // — already an extreme positive in the baked corpus prior —
        // over the parsed `rel_to` slice and is what carries the
        // candidate to victory. The `BaseRateCommonMarking` entry
        // here records the prior's contribution in the audit log
        // without double-counting it in the decoder's score, mirror-
        // ing PR-A's trigraph-prior treatment (delta = 0.0).
        let entry = FeatureEntry {
            id: FeatureId::BaseRateCommonMarking,
            delta: 0.0,
        };
        out.push((alt, entry));

        search_start = block_end;
    }

    out
}
