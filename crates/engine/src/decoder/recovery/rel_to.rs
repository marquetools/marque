// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! REL TO structural repair.
//!
//! Header + entry normalization passes that fix shape-deviation typos
//! against the closed CAPCO §H.8 grammar — `RELT O` / `REL OT` /
//! `A US` / `AU,S`. Conservative by construction: every transform is
//! gated either by a literal pattern that cannot appear in valid CAPCO
//! text (header patterns 1, 2) or by an `is_country_code` guard on the
//! corrected output (entry patterns 3, 4).
//!
//! Does **not** handle vocabulary-based fuzzy correction — that lives
//! in the sibling `rel_to_trigraph.rs` (trigraph fuzzy expansion + USA
//! injection), which fires after this pass on text that still parses
//! as zero-candidate.

use marque_ism::{CapcoTokenSet, token_set::TokenSet as _};

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
///    trigraph (`is_country_code` check) AND the 1-letter alone is not a
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
/// # Scope
///
/// Patterns 1 and 2 affect the literal `REL TO` header and run
/// regardless of what follows. Patterns 3 and 4 require a `REL TO `
/// header in the input — they scan from each `REL TO ` substring
/// forward to the next `//` (or end of text) and only operate on
/// comma-separated entries within that block.
///
/// All four transforms are conservative: their false-positive risk
/// is bounded by the literal patterns not appearing in any valid
/// CAPCO text (patterns 1, 2) or by the `is_country_code` guard
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
// The fuzzy / prior-weighted trigraph correction cluster lives in the sibling rel_to_trigraph.rs.
// Decision-archaeology for #186 relocated to docs/refactor-006/decoder-architecture.md §"REL TO recovery — historical archaeology".
pub(in crate::decoder) fn try_rel_to_structural_repair(text: &str) -> Option<String> {
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
pub(super) fn try_rel_to_header_normalize(text: &str) -> Option<String> {
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
/// (`CapcoTokenSet::is_country_code`). The trigraph dictionary is the
/// arbiter of "valid country code" — no fuzzy guessing.
pub(super) fn try_rel_to_entry_normalize(text: &str) -> Option<String> {
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
fn apply_rel_to_entry_pass(text: &str, token_set: &CapcoTokenSet) -> Option<String> {
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
fn fix_rel_to_block(block: &str, token_set: &CapcoTokenSet) -> Option<(usize, String)> {
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
        if !token_set.is_country_code(&joined) {
            continue;
        }
        // Defensive: don't fire if the 1-letter prefix is itself a
        // trigraph (no real CAPCO trigraph is 1-letter, but guard
        // anyway against future schema changes).
        let one_letter = std::str::from_utf8(&bytes[..1]).expect("ASCII upper");
        if token_set.is_country_code(one_letter) {
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
        if !token_set.is_country_code(&joined) {
            continue;
        }
        if token_set.is_country_code(left_trim) {
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
// Tests
// ---------------------------------------------------------------------------
//
// Tests live in `../tests/rel_to_recovery_tests.rs`. They were carved
// out of this file to keep the combined production + test surface
// within the 800-line gate.

#[path = "../tests/rel_to_recovery_tests.rs"]
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(unused_imports)]
mod tests;
