use super::*;

/// `(offset, trimmed_token)` per non-empty slash-separated sub-token.
pub(super) type SlashTokens<'a> = SmallVec<[(usize, &'a str); 4]>;

/// Per-`/` byte separator descriptor. Carries the byte span plus
/// `left_nonempty` / `right_nonempty` flags indicating whether the
/// trimmed `s.split('/')` part on each side of the slash was non-empty
/// (i.e., produced an entry in `SlashTokens`). The flags let the
/// emission site enforce the `TokenKind::Separator` contract: a
/// Separator span lives between two *committed* same-category tokens.
/// Empty-boundary slashes — trailing `/`, leading `/`, or a slash
/// between empty parts — have `false` on at least one side and MUST
/// NOT emit a Separator (downstream byte-precise splice rules rely on
/// the invariant; see Copilot R3 fix on PR #416).
#[derive(Clone, Copy, Debug)]
pub(super) struct SlashSeparator {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) left_nonempty: bool,
    pub(super) right_nonempty: bool,
}

pub(super) type SlashSeparators = SmallVec<[SlashSeparator; 4]>;

/// Splits `s` on `/` and returns both the tokens and the separator
/// positions.
///
/// Returns a pair of vectors:
/// - `.0` — `(token_offset, trimmed_token)` per non-empty token.
/// - `.1` — one [`SlashSeparator`] per `/` byte, carrying the span
///   `(start, end)` and two `*_nonempty` flags identifying whether the
///   adjacent trimmed `s.split('/')` parts on each side produced a
///   committed token. `start` is `slash_offset` minus any ASCII
///   whitespace immediately preceding the slash (bounded by the end of
///   the previous emitted token, so the span never crosses into the
///   previous token's bytes). `end` is `slash_offset + 1` plus any
///   ASCII whitespace immediately following the slash. The span covers
///   the `/` plus any whitespace an author drifted into the gap on
///   either side of the slash.
///
/// CAPCO-2016 §A.6 p16 disallows interjected whitespace in SAP-`/` but is
/// silent for dissem-`/` and SCI-`/`. Spanning adjacent ASCII whitespace
/// on both sides is engineering tolerance for author drift, NOT a §A.6
/// rule — it lets downstream rules see a single Separator token that
/// owns the inter-token byte range whether the author wrote `OC/NF`,
/// `OC /NF`, `OC/ NF`, or `OC / NF`. The bidirectional coverage is what
/// makes audit-record byte ranges contiguous: every byte between adjacent
/// non-empty tokens belongs to exactly one span (token or separator),
/// with no gaps.
///
/// The `*_nonempty` flags on each [`SlashSeparator`] are emission-side
/// gates: callers MUST drop separators whose either neighbor is empty
/// (e.g., trailing `OC/`, leading `/OC`, all-empty `//`). Combined with
/// a same-category check against the emitted token results, this is
/// what enforces the Separator-between-two-committed-same-category-
/// tokens contract.
pub(super) fn split_slash_with_separator_offsets(s: &str) -> (SlashTokens<'_>, SlashSeparators) {
    let mut tokens: SlashTokens<'_> = SmallVec::new();
    let mut separators: SlashSeparators = SmallVec::new();
    let bytes = s.as_bytes();
    let mut pos = 0usize;
    // Tracks the end byte of the previously emitted non-empty trimmed
    // token. Bounds the leading-whitespace walk-back of the next
    // separator so the separator span cannot cross into the previous
    // token's bytes. Initialized to 0 so the first separator's
    // walk-back stops at the start of the slice (which is what we want:
    // any leading whitespace at the very start belongs to neither a
    // token nor a separator and stays uncovered by design).
    let mut prev_token_end: usize = 0;
    // Tracks whether the trimmed part immediately preceding the next
    // slash was non-empty (i.e., produced a token push). For each
    // slash, this becomes the `left_nonempty` flag; the right flag is
    // determined by the next iteration's `trimmed.is_empty()` check
    // via back-patching.
    let mut prev_part_nonempty = false;
    for (i, part) in s.split('/').enumerate() {
        if i > 0 {
            // The slash byte sits at index `pos - 1` (we advanced past the
            // previous part and the `/` separator). Span the slash plus
            // any ASCII whitespace on both sides, bounded by
            // `prev_token_end` on the left so the separator never
            // overlaps the previous emitted token.
            let slash_pos = pos - 1;
            let mut slash_start = slash_pos;
            while slash_start > prev_token_end && bytes[slash_start - 1].is_ascii_whitespace() {
                slash_start -= 1;
            }
            let mut slash_end = slash_pos + 1;
            while slash_end < bytes.len() && bytes[slash_end].is_ascii_whitespace() {
                slash_end += 1;
            }
            // `left_nonempty` is decided now (it's the prior part's
            // status). `right_nonempty` is back-patched after we
            // classify the current part below.
            separators.push(SlashSeparator {
                start: slash_start,
                end: slash_end,
                left_nonempty: prev_part_nonempty,
                right_nonempty: false, // filled in below
            });
        }
        let trim_lead = part.len() - part.trim_start().len();
        let trimmed = part.trim();
        let part_nonempty = !trimmed.is_empty();
        if part_nonempty {
            let token_start = pos + trim_lead;
            tokens.push((token_start, trimmed));
            prev_token_end = token_start + trimmed.len();
        }
        // Back-patch the just-pushed separator's `right_nonempty` with
        // the current part's status. The first iteration (i == 0)
        // pushes no separator, so there's nothing to patch.
        if i > 0 {
            if let Some(sep) = separators.last_mut() {
                sep.right_nonempty = part_nonempty;
            }
        }
        prev_part_nonempty = part_nonempty;
        pos += part.len() + 1; // +1 for the `/` separator
    }
    (tokens, separators)
}
