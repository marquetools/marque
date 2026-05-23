//! NATO longhand → canonical portion fold.
//!
//! Map `NATO SECRET` / `NATO CONFIDENTIAL` / etc. banner-form longhand
//! to the canonical portion abbreviations (`NS`, `NC`, …). Per
//! CAPCO-2016 §G.1 Table 4 pp 36-38 the portion form is the
//! authoritative wire encoding inside marking compounds.

use marque_ism::{
    NatoClassification,
    span::MarkingType,
};

// ---------------------------------------------------------------------------
// NATO longhand → canonical portion fold
// ---------------------------------------------------------------------------

/// Mapping from NATO longhand classification level tokens to canonical
/// [`NatoClassification`] variants. Keyed on the token string (abbreviation
/// or full-word form); the canonical portion string (`NS`, `CTS`, etc.) is
/// derived via [`NatoClassification::portion_str`] so that a future
/// enum-variant addition (ATOMAL sub-levels, PR 9 T134 BOHEMIA/BALK) is
/// enough to extend coverage without touching this fold logic.
///
/// Rows ordered: abbreviations first (U/R/C/S/TS), then full words. The
/// lookup is a linear scan over 10 rows — the total set is small and
/// bounded by the five standard NATO classification levels.
///
/// **Out of scope.** Parametric NATO-NAC-Activity rows from §G.1 Table 4
/// lines 776-779 (`NATO [NAC Activity] SECRET → N[NAC Activity]S` and
/// equivalents for C/R/U) are not covered here; they require distinct parser
/// support for the open-ended activity identifier and are not tracked in this
/// PR or PR 9 T134.
///
/// Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (canonical Register).
const NATO_LONGHAND_FOLD: &[(&str, NatoClassification)] = &[
    // Abbreviation forms (single-letter / two-letter)
    ("U", NatoClassification::NatoUnclassified),
    ("R", NatoClassification::NatoRestricted),
    ("C", NatoClassification::NatoConfidential),
    ("S", NatoClassification::NatoSecret),
    ("TS", NatoClassification::CosmicTopSecret),
    // Full-word forms ("TOP SECRET" is a two-word compound handled separately)
    ("UNCLASSIFIED", NatoClassification::NatoUnclassified),
    ("RESTRICTED", NatoClassification::NatoRestricted),
    ("CONFIDENTIAL", NatoClassification::NatoConfidential),
    ("SECRET", NatoClassification::NatoSecret),
    // Note: "TOP SECRET" requires two-token detection; see `fold_nato_segment`.
    // "TOP" alone is not a valid abbreviation and is excluded from this table.
];

/// Fold NATO longhand classification levels into canonical forms.
///
/// Recovers inputs the strict parser doesn't recognize. Handles both
/// portion and banner kinds:
///
/// For `MarkingType::Portion`, NATO abbreviation → portion abbreviation
/// (equivalence transform, both are valid surface forms):
///   NATO U → NU, NATO R → NR, NATO C → NC, NATO S → NS,
///   NATO TS → CTS, NATO UNCLASSIFIED → NU, NATO SECRET → NS, … (long-word
///   forms too). NATO TOP SECRET → CTS (two-word compound, handled explicitly).
///   Canonical inputs (e.g. `(//NS//NF)`) return `None` (idempotent).
///
/// For `MarkingType::Banner`, NATO abbreviation → banner long form:
///   NATO U → NATO UNCLASSIFIED, NATO R → NATO RESTRICTED,
///   NATO C → NATO CONFIDENTIAL, NATO S → NATO SECRET,
///   NATO TS → COSMIC TOP SECRET.
///   Inputs already in banner canonical form (e.g. `NATO SECRET//NOFORN`)
///   return `None` (idempotent). This closes the unimplemented half of #260:
///   the strict parser recognizes full banner forms (`NATO SECRET`,
///   `COSMIC TOP SECRET`) but not abbreviations (`NATO S`, `NATO TS`), so
///   a banner input `NATO S//NOFORN` fails strict parse and the fold
///   recovers it.
///
/// For `MarkingType::Cab` and `MarkingType::PageBreak`, returns `None`
/// (CAB authority lines and page-break sentinels don't carry NATO classifications).
///
/// **Segment restriction (FIX-2, CAPCO §H.7 FGI transmutation).** The fold
/// fires ONLY on the first non-empty `//`-separated segment (the
/// classification slot). NATO content in a non-first-slot position
/// (e.g., `(S//NATO C)`) indicates commingled US+NATO info, which per
/// CAPCO-2016 §H.7 should transmute to FGI (`(S//FGI NATO)`) — not
/// produce a NATO-axis canonical. PR 8 does not implement the transmutation
/// (Stage 4 / PR 9+ territory); restricting the fold to the first segment
/// ensures we don't manufacture wrong intermediates while the proper fix waits.
/// Cross-segment NATO inputs return decode-miss.
///
/// **Idempotence**: returns `None` when no segment was changed (including
/// when the input is already canonical).
///
/// **Pure function**: no captures, no global state. `Send + Sync` follows
/// automatically. Pre-uppercased input assumed (caller passes the
/// post-`normalize_delimiters_and_case` string).
///
/// Citation: CAPCO-2016 §G.1 Table 4 pp 36-38 (canonical Register);
/// §A.6 p15 (`//` prefix for non-US classifications); §H.7 (FGI transmutation).
pub(crate) fn try_nato_fold(text: &str, kind: MarkingType) -> Option<String> {
    // CAB, PageBreak, and PageFinalization inputs don't carry NATO
    // classifications — they are non-content / engine-synthesized
    // boundary candidates. PageFinalization (issue #461) is dispatched
    // only to `Phase::PageFinalization` rules and never enters the
    // strict/decoder recognize path on actual bytes; the early-return
    // mirrors the existing Cab/PageBreak shape.
    if matches!(
        kind,
        MarkingType::Cab | MarkingType::PageBreak | MarkingType::PageFinalization
    ) {
        return None;
    }
    // All NATO classification tokens are pure ASCII; non-ASCII input
    // cannot contain them.
    if !text.is_ascii() {
        return None;
    }

    // Strip surrounding parens — only portion form has them. Banner inputs
    // like `NATO S//NOFORN` never carry parens so this branch is
    // naturally a no-op for Banner kind.
    let (has_parens, inner) =
        if kind == MarkingType::Portion && text.starts_with('(') && text.ends_with(')') {
            (true, &text[1..text.len() - 1])
        } else {
            (false, text)
        };

    // Split into `//`-separated segments. A leading `//` (canonical
    // non-US form) produces an empty first element; we track this to
    // avoid adding a spurious second `//` prefix.
    let segments: Vec<&str> = inner.split("//").collect();
    let had_leading_empty = segments.first().map(|s| s.is_empty()).unwrap_or(false);

    // Determine the index of the first non-empty segment (the
    // classification slot). The fold ONLY fires on that segment;
    // all other segments are passed through verbatim.
    //
    // Rationale: NATO classifications always occupy the first
    // `//`-separated slot per CAPCO-2016 §A.6. `NATO X` in a
    // non-first slot (e.g., `(S//NATO C)`) indicates commingled
    // US+NATO info. Correct canonical form per §H.7 is FGI transmutation
    // (`(S//FGI NATO)`), not a NATO-axis canonical. PR 9+ handles that
    // transmutation; PR 8 produces a decode-miss to avoid wrong intermediates.
    let first_nonempty_idx = segments.iter().position(|s| !s.is_empty());
    let Some(class_slot_idx) = first_nonempty_idx else {
        return None; // All empty — degenerate input, nothing to fold.
    };

    let mut any_changed = false;
    let mut first_segment_folded = false;
    let mut result_segments: Vec<String> = Vec::with_capacity(segments.len());

    for (i, seg) in segments.iter().enumerate() {
        if i == class_slot_idx {
            // Classification slot — attempt the fold.
            match fold_nato_segment(seg, kind) {
                Some(folded) => {
                    any_changed = true;
                    if i == 0 {
                        first_segment_folded = true;
                    }
                    result_segments.push(folded);
                }
                None => {
                    result_segments.push(seg.to_string());
                }
            }
        } else {
            // Non-classification slot — pass through unchanged.
            result_segments.push(seg.to_string());
        }
    }

    if !any_changed {
        return None;
    }

    let rejoined = result_segments.join("//");

    // For portion inputs that arrived without a leading `//` (e.g., `(NATO S)` or
    // `(NATO S//NF)`), the fold converts the first segment to a canonical
    // NATO abbreviation. Non-US classifications require the `//` prefix per
    // CAPCO-2016 §A.6 p15 so the strict parser enters the non-US classification
    // code path. We add it only when the first segment was the one folded
    // AND the original had no leading empty segment (= no prior `//`).
    //
    // The same `//` logic applies to banner inputs: banner `NATO S//NF`
    // (no leading `//`) folds to `NATO SECRET//NF` → needs `//NATO SECRET//NF`
    // per §A.6 p15. The `first_segment_folded` flag is set whenever the
    // classification-slot segment folds, regardless of kind.
    let inner_out = if first_segment_folded && !had_leading_empty {
        format!("//{rejoined}")
    } else {
        rejoined
    };

    if has_parens {
        Some(format!("({inner_out})"))
    } else {
        Some(inner_out)
    }
}

/// Attempt to fold a single `//`-separated segment that starts with the
/// NATO keyword.
///
/// Returns `Some(canonical)` when the segment begins `NATO <level>` (with
/// `<level>` either an abbreviation from [`NATO_LONGHAND_FOLD`] or the
/// two-word compound `TOP SECRET`) AND the result differs from the input
/// (idempotence guard). Returns `None` for all other inputs, including
/// segments whose first token is not `NATO` (guard against false-positives
/// inside `REL TO USA, NATO` or FGI country lists).
///
/// The `kind` parameter controls the emission form:
/// - `MarkingType::Portion` — emits the portion abbreviation
///   (`NS`, `NC`, `CTS`, …) via [`NatoClassification::portion_str`].
/// - `MarkingType::Banner` — emits the banner long form
///   (`NATO SECRET`, `COSMIC TOP SECRET`, …) via
///   [`NatoClassification::banner_str`]. Idempotent: if the input segment
///   is already in banner long form (e.g. `NATO SECRET`), the emitted
///   `banner_str()` equals the input and `None` is returned.
///
/// Returns `None` when the segment is `NATO <level> <rest>` with non-empty
/// `<rest>` — compound forms like `NATO SECRET ATOMAL` parse through the
/// strict parser's `parse_nato_classification`, which now (PR 9c.1 T134)
/// canonicalizes legacy compound text into bare class + AEA/SCI companion
/// per CAPCO-2016 §H.7 p122 + §G.2 p40 + §H.7 p127. The fold must not
/// truncate the suffix; its job is the 5-base-level path only.
///
/// **Caller invariant.** The caller ([`try_nato_fold`]) restricts invocation
/// to the first non-empty `//`-separated segment (the classification slot) so
/// that `NATO X` in a non-classification-slot position (e.g., `(S//NATO C)`)
/// never reaches this function. This is defense-in-depth: the segment-leading
/// guard (`strip_prefix("NATO ")`) would also prevent non-NATO segments from
/// firing, but the first-segment restriction in the caller is the primary
/// mechanism ensuring semantic correctness per CAPCO-2016 §H.7. A
/// `NATO X` token in the SCI/dissem slot indicates commingled US+NATO info
/// that should transmute to FGI — not produce a NATO-axis canonical.
pub(crate) fn fold_nato_segment(seg: &str, kind: MarkingType) -> Option<String> {
    let trimmed = seg.trim();
    // Segment-leading guard: the fold ONLY fires when the first
    // non-delimiter token is the literal keyword `NATO`.
    let after_nato = trimmed.strip_prefix("NATO ")?;
    let after_nato = after_nato.trim_start();

    // Determine the `NatoClassification` variant from the level token(s).
    let nato_level: NatoClassification;

    // Special case: "TOP SECRET" is a two-word compound that cannot be
    // matched as a single entry in `NATO_LONGHAND_FOLD`. Detect it
    // explicitly before the single-token path.
    if let Some(after_ts) = after_nato.strip_prefix("TOP SECRET") {
        let rest = after_ts.trim_start();
        if !rest.is_empty() {
            // Compound NATO SAP forms (ATOMAL, BOHEMIA, BALK) are out of scope
            // for PR 8. The strict parser already accepts
            // `NATO TOP SECRET ATOMAL` / `NATO TOP SECRET-BOHEMIA` /
            // `NATO TOP SECRET-BALK` (parser.rs:1043-1052); folding the first
            // half would mangle the suffix and regress recovery.
            // PR 9 T134 will land an explicit fold for these compounds.
            return None;
        }
        nato_level = NatoClassification::CosmicTopSecret;
    } else {
        // Single-token level: split at the next whitespace to isolate the
        // level token, then look it up in `NATO_LONGHAND_FOLD`.
        let (level_token, rest) = match after_nato.find(char::is_whitespace) {
            Some(pos) => (&after_nato[..pos], after_nato[pos..].trim_start()),
            None => (after_nato, ""),
        };

        let found = NATO_LONGHAND_FOLD
            .iter()
            .find(|&&(key, _)| key == level_token)
            .map(|&(_, level)| level)?;

        if !rest.is_empty() {
            // Same rationale as the TOP SECRET branch: compound SAP forms
            // (NATO SECRET ATOMAL, NATO CONFIDENTIAL ATOMAL, etc.) are out of
            // scope. The strict parser handles them; the fold must not truncate
            // the suffix. PR 9 T134 will land the explicit ATOMAL/BOHEMIA/BALK
            // fold.
            return None;
        }
        nato_level = found;
    }

    // Emit the canonical form for the requested kind, then check idempotence.
    // For portion: `NATO SECRET` → `NS` (changed → emit). `NATO NS` would
    // not match (strip_prefix "NATO " yields "NS" which is not in table when
    // looked up as level_token; actually "NS" IS not in the table — only
    // abbreviations U/R/C/S/TS and long words). Idempotence fires on banner:
    // `NATO SECRET` → `banner_str() = "NATO SECRET"` — segment is the same.
    // But `seg` here is just the classification content without "NATO ",
    // so we need the full composed string for comparison.
    let canonical = match kind {
        MarkingType::Portion => nato_level.portion_str().to_owned(),
        // Banner form is the full level string (e.g. "NATO SECRET").
        // `banner_str()` already returns "NATO SECRET" / "COSMIC TOP SECRET"
        // etc. — it INCLUDES the "NATO " prefix for all non-CTS levels.
        _ => nato_level.banner_str().to_owned(),
    };

    // Idempotence: if the emitted canonical equals the input segment
    // (trimmed), no actual change occurred — return None so `any_changed`
    // stays false and `try_nato_fold` returns None overall.
    if canonical == trimmed {
        return None;
    }

    Some(canonical)
}

// ---------------------------------------------------------------------------
