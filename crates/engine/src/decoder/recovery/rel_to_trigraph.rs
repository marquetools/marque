// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! REL TO trigraph fuzzy expansion and USA injection.
//!
//! Two probabilistic recovery passes that operate after the structural
//! repair in the sibling `rel_to.rs`. Both consult the corpus-weighted
//! trigraph priors in `marque_capco::priors` to break ties at decoder
//! score time:
//!
//! 1. **Trigraph fuzzy expansion** — emits one candidate per fuzzy
//!    match for each unknown 3- or 4-char REL TO entry, capped at
//!    `TRIGRAPH_FUZZY_TOP_K = 4` per ambiguous entry (issue #233 /
//!    #246).
//! 2. **USA injection** — when the first entry is a 1- or 2-char
//!    ASCII-uppercase token and USA is not otherwise present, emits a
//!    candidate with USA substituted (CAPCO-2016 §H.8 p151's USA-first
//!    invariant).
//!
//! Both passes are emit-only — they do not mutate the input string; the
//! decoder dispatcher chooses between the emitted alternates and the
//! original via posterior scoring.

use marque_core::fuzzy::FuzzyVocabMatcher;
use marque_ism::{CapcoTokenSet, token_set::TokenSet as _};
use marque_rules::confidence::FeatureId;

use super::super::types::FeatureEntry;

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
/// [`CapcoTokenSet::is_country_code`]). So a typo'd 3-char REL TO entry
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
/// [`CapcoTokenSet::is_country_code`] and into the
/// [`marque_ism::TRIGRAPHS`] slice this function fuzzy-matches against.
// The fuzzy / prior-weighted trigraph correction cluster lives in this file.
// Decision-archaeology for #186 / #233 / #246 relocated to
// docs/refactor-006/decoder-architecture.md §"REL TO recovery — historical archaeology".
pub(in crate::decoder) fn try_rel_to_fuzzy_trigraph_candidates(
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
            if token_set.is_country_code(trimmed) {
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
            // present elsewhere in this REL TO block.
            // Dedup rationale relocated to
            // docs/refactor-006/decoder-architecture.md §"REL TO recovery — historical archaeology".
            let other_trigraphs: Vec<&str> = block
                .split(',')
                .map(str::trim)
                .filter(|e| {
                    let elen = e.len();
                    (elen == 3 || elen == 4)
                        && e.bytes().all(|b| b.is_ascii_uppercase())
                        && *e != trimmed
                        && token_set.is_country_code(e)
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
///   `MARQUE_AUDIT_SCHEMA` stays at `marque-2.0`.
// Complements try_rel_to_fuzzy_trigraph_candidates (sibling function above) by handling
// 1- and 2-char first entries that fall below that function's 3-char floor. The
// partition is shape-based, not vocabulary-based.
pub(in crate::decoder) fn try_rel_to_usa_injection_candidates(
    text: &str,
) -> Vec<(String, FeatureEntry)> {
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
        // E052 would then need to dedup. Short-circuit here rather
        // than emit-and-redup.
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
