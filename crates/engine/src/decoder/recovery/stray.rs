//! Stray-character `/X/` recovery.
//!
//! Collapse `(S/X/NF)` / `(TS / X / NF)` style inputs where a stray
//! single character was wedged between slashes. Returns the set of
//! candidate strings produced by attempting the collapse, deduplicated
//! upstream.

// ---------------------------------------------------------------------------
// Stray-character `/X/` recovery
// ---------------------------------------------------------------------------

/// Walk `text` looking for the `<alnum>/<single_alnum_char>/<alnum>`
/// pattern. For each match (currently only the first match is
/// processed — see "scope" below) emit three candidate transforms:
///
/// 1. **Drop X** — `A/X/B` → `A//B`. Recovers stray characters
///    inserted between two valid tokens. Example:
///    `SECRET//NOFORN/R/EXDIS` → `SECRET//NOFORN//EXDIS` (the stray
///    `/R/` between NOFORN and EXDIS is removed).
///
/// 2. **Right-attach X** — `A/X/B` → `A//XB`. Recovers a single
///    character that got separated from the start of the right
///    token by a `/`. Example: `TOP SECRET//SI/N/OFORN` →
///    `TOP SECRET//SI//NOFORN` (the `N` was the leading character
///    of `NOFORN`).
///
/// 3. **Left-attach X** — `A/X/B` → `AX//B`. Recovers a single
///    character that got separated from the end of the left token
///    by a `/`. Example: `SECRE/T/REL TO USA, AUS, GBR` →
///    `SECRET//REL TO USA, AUS, GBR` (the `T` was the trailing
///    character of `SECRET`).
///
/// All three transforms are emitted as candidates; the recognizer's
/// step-3a [`TokenKind::Unknown`](marque_ism::TokenKind::Unknown)
/// filter is the natural disambiguator. For each input only one of
/// the three transforms produces fully-recognized tokens — the
/// other two leave broken-token fragments (`OFORN`, `NOFORNR`,
/// `SECRER`, …) that survive strict parsing as `TokenKind::Unknown`
/// and get dropped before scoring. The decoder doesn't need a
/// per-pattern lookup table to choose the right transform; the
/// vocab does the choosing implicitly.
///
/// # Scope (PR 7)
///
/// Only the FIRST `/X/` match in the input is processed; an input
/// with multiple stray-character patterns (e.g., `S/I/T/K`) is not
/// fully recovered by a single pass. The current corpus has very
/// few multi-pattern inputs (1–2 in the unresolved Typo set), and
/// adding a multi-pass loop here would complicate the candidate cap
/// in [`generate_candidate_bytes`] without proportional benefit. A
/// future PR can iterate if multi-pattern recovery becomes
/// load-bearing for SC-004 movement.
///
/// # Pattern boundary requirements
///
/// The `/X/` match requires alphanumeric context on both sides
/// (`<alnum>/<X>/<alnum>`). Without those guards the pattern would
/// fire on edge cases like `(/X/)` (start of portion form) where
/// the surrounding context is structural punctuation, not a token —
/// the recovery would be semantically meaningless there because
/// there's no token to attach `X` to.
pub(crate) fn try_collapse_stray_char_slash(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        // `/X/` shape: bytes[i] = `/`, bytes[i+1] = single ASCII
        // alnum, bytes[i+2] = `/`. The single-alnum requirement
        // prevents matching on `/AB/` (which would be a 2-char
        // token between slashes, not a stray character).
        if bytes[i] != b'/' || !bytes[i + 1].is_ascii_alphanumeric() || bytes[i + 2] != b'/' {
            i += 1;
            continue;
        }
        // Boundary check: the slashes must be sandwiched between
        // alphanumeric tokens on both sides. Without this guard
        // `(/X/)` (start-of-portion-form) would trip the match.
        let prev_alnum = i > 0 && bytes[i - 1].is_ascii_alphanumeric();
        let next_alnum = i + 3 < bytes.len() && bytes[i + 3].is_ascii_alphanumeric();
        if !prev_alnum || !next_alnum {
            i += 1;
            continue;
        }

        let x = bytes[i + 1];
        let prefix = &bytes[..i];
        let suffix = &bytes[i + 3..];

        // The unwraps are safe: `text` is valid UTF-8, `prefix` /
        // `suffix` are slices on byte boundaries (the pattern only
        // matched on ASCII bytes), and we only insert ASCII bytes
        // (`/`, `x` which is ASCII alnum) between them.
        let mut out = Vec::with_capacity(3);

        // 1. Drop X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.extend_from_slice(b"//");
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        // 2. Right-attach X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.extend_from_slice(b"//");
        buf.push(x);
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        // 3. Left-attach X.
        let mut buf = Vec::with_capacity(bytes.len());
        buf.extend_from_slice(prefix);
        buf.push(x);
        buf.extend_from_slice(b"//");
        buf.extend_from_slice(suffix);
        out.push(String::from_utf8(buf).expect("ASCII insertions on UTF-8 prefix/suffix"));

        return out;
    }
    Vec::new()
}

