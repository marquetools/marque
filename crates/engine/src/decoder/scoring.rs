//! Bag-of-tokens scorer for the decoder.
//!
//! Computes per-candidate `(prior, posterior)` from baked corpus
//! log-priors plus structural penalties. The penalties are explicit
//! constants in this file so a future tuning pass can move them
//! without grepping the recognizer.

use marque_capco::CapcoMarking;
use marque_ism::{MarkingClassification, SciControlSystem, span::MarkingType};
use smallvec::SmallVec;

use super::types::CanonicalAttempt;

/// Floor log-prior for canonical tokens that don't appear in the
/// baked `TOKEN_BASE_RATES` table.
///
/// Baked priors are `log((hits + 1) / (total + |V|))` with
/// Laplace smoothing over the non-IC Enron corpus (see
/// `tools/corpus-analysis/analyze.py::derive_priors`). A token the
/// corpus never observed still receives a non-zero smoothed prior in
/// that build; this constant exists for the different, rarer case
/// where the canonical-tokens iterator produces a string that was
/// not in the build's vocabulary at all (e.g., a CVE token added
/// after the last priors regeneration). Without this floor, such
/// tokens would silently contribute `0.0` to the sum — and since
/// every real log-prior is negative, a missing token would score
/// HIGHER than a known one, inverting the ranking.
///
/// Magnitude (`-12.0` nats ≈ log(6e-6)) is chosen to be strictly
/// lower than every log-prior the generator would emit for a
/// non-empty corpus: the Enron-derived values bottom out around
/// `-11.7` for the most infrequent observed tokens (see
/// `crates/capco/corpus/priors.json`).
pub(crate) const MISSING_TOKEN_LOG_PRIOR: f32 = -12.0;

/// Posterior penalty applied when a candidate's strict parse buries a
/// reserved dissem-control token (a hard splitter — see
/// [`is_hard_splitter`]) inside a SAR or SCI sub-component slot.
///
/// **Why this exists.** Hard-splitter tokens (NOFORN, ORCON, EXDIS,
/// FOUO, …) have hard reserved meanings as dissem controls per CAPCO-
/// 2016 §H.8/§H.9; they have no in-segment role inside SCI or SAR
/// sub-components. A strict parse that places such a token under
/// [`marque_ism::SarMarking`] or [`marque_ism::SciMarking`] is
/// essentially always a missing-
/// `//` artifact in the input — the alternative parse with the token
/// emitted as a dissem control is the correct interpretation. (REL
/// TO is intentionally excluded from the penalty surface here: its
/// payload is a list of country trigraphs whose grammar accepts only
/// 3-letter alpha codes drawn from the CVE-derived trigraph table,
/// so a 4+-char hard splitter cannot land in a REL TO slot in the
/// first place. The Copilot review on PR #178 flagged a wider doc
/// claim that suggested otherwise — the doc is now scoped to the
/// slots the penalty actually defends.)
///
/// **Why scoring needs help.** The bag-of-tokens scorer above sums
/// log-priors for the marking's canonical tokens, and `for_each_canonical_token`
/// deliberately excludes SAR program/compartment/sub-compartment text
/// (open-set agency-assigned codewords). So an absorbing parse contributes
/// only the classification's prior; the equivalent delim-inserted parse
/// contributes classification + the dissem token's prior, which is a
/// MORE NEGATIVE log-posterior. Without a corrective penalty the
/// absorbing parse always wins. SCI absorption usually self-resolves
/// because [`marque_core::Parser::parse`]'s SCI subgrammar produces
/// [`marque_ism::TokenKind::Unknown`] for non-alphanumeric/wrong-shape
/// compartment tokens (which step 3a then drops), but SAR's grammar accepts any
/// `[A-Z0-9]+` identifier and absorbs cleanly — leaving SAR as the
/// observed failure mode on the SC-004 corpus (the `SAR-BP-J12 …` and
/// `SPECIAL ACCESS REQUIRED-BUTTER POPCORN …` fixtures pre-PR-5).
///
/// **Magnitude.** Empirically the absorbing-vs-delim-inserted spread
/// on those two fixtures is ~9 nats; the [`MISSING_TOKEN_LOG_PRIOR`]
/// floor (`-12.0`) gives a comfortable margin and is robust to small
/// future shifts in the priors table. Defining the penalty as
/// `MISSING_TOKEN_LOG_PRIOR` (rather than re-stating the literal)
/// keeps the two below-floor signals mechanically at parity for any
/// candidate that triggers both — a future ratchet of one constant
/// pulls the other along.
///
/// **Safety.** Hard-splitter tokens are all 4+ chars and have shapes
/// distinct from real SAR identifiers (`BP`, `CD`, `XR` are 2-char;
/// `BUTTER POPCORN`, `J12`, `K15`, `XRA` are alphanumeric short
/// codes that don't collide with the hard-splitter list). So this
/// penalty cannot fire on a legitimate SAR/SCI parse.
const HARD_SPLITTER_ABSORPTION_PENALTY: f32 = MISSING_TOKEN_LOG_PRIOR;

/// Per-entry structural penalty for SCI markings whose control system
/// landed as [`SciControlSystem::Custom`]. Issue #133 PR 6.
///
/// **Why this penalty exists.** `marque_core::Parser`'s structural SCI
/// subparser (CAPCO-2016 §A.6 grammar) accepts any alphanumeric
/// identifier as a "custom" control system / compartment when the
/// segment text contains `-` or `/`. That branch was added so legal
/// compound SCI shapes (`SI-G ABCD DEFG-MMM AACD`) parse correctly,
/// but it has a side effect: a typo'd or stray segment like
/// `USAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB` parses cleanly into
/// three `Custom`-system SCI markings (USAR/CD/XR with attached
/// compartments). The bag-of-tokens scorer can't tell that this is
/// the wrong interpretation — `Custom` SCI control systems don't
/// appear in `for_each_canonical_token`, so they don't shift the prior
/// either way, and the candidate ties with structurally-richer
/// alternatives like the SAR-repaired candidate that
/// `try_sar_indicator_repair` emits.
///
/// **What the penalty does.** Adds [`MISSING_TOKEN_LOG_PRIOR`] (the
/// same below-observed-floor magnitude as
/// [`HARD_SPLITTER_ABSORPTION_PENALTY`]) per `Custom`-system SCI
/// marking. The penalty is per-entry so candidates that absorbed
/// multiple stray segments (like the 3-segment USAR/CD/XR case) get
/// progressively worse posteriors, restoring the SAR-repair
/// candidate's lead by a margin that clears
/// [`UNAMBIGUOUS_LOG_MARGIN`].
///
/// **Magnitude.** Same `-12.0` as the hard-splitter penalty: both are
/// "this parse pattern is highly unlikely in well-formed CAPCO
/// markings" structural signals, and keeping them at parity by
/// definition (rather than literal duplication) lets a future
/// ratchet of one move both together. A single legitimate custom
/// control (the §A.6 p16 `99` example) gets one `-12.0` hit but
/// remains the sole candidate when no alternative interpretation
/// exists, so the dispatcher still emits `Unambiguous`.
///
/// **Safety / discriminator choice.** The discriminator is
/// `sm.system == SciControlSystem::Custom(_)`, NOT
/// `sm.canonical_enum.is_none()`. The two are NOT equivalent:
/// `canonical_enum` is also `None` for legitimate `Published`-system
/// SCI markings whenever the `{system}-{first_compartment}` pair
/// doesn't map to a CVE atom (per the `canonical_enum` doc in
/// `crates/scheme/src/scheme.rs` — populated only when "the bare
/// control or `{ctrl}-{first_comp}` matches a CVE value AND no
/// sub-compartments are present"). Using `canonical_enum` as the
/// discriminator would penalize legitimate `SI-G ABCD DEFG-MMM AACD`-
/// style markings (system=`Published(Si)`, sub-compartments present
/// → canonical_enum=None), broadly skewing scoring against rich
/// SCI shapes. Discriminating on `system` directly catches the
/// USAR/CD/XR custom-only case while leaving every published SCI
/// marking — bare or compound — untouched. A candidate with mixed
/// SCI (e.g., `SI-G ABCD//99`) gets a single penalty for the `99`
/// `Custom` entry only, which is a reasonable cost for a
/// structurally suspicious mixed shape. The penalty does NOT fire
/// on candidates with empty `sci_markings` — so the SAR-repaired
/// candidate (which projects no SCI) is unaffected.
const CUSTOM_SCI_MARKING_PENALTY: f32 = MISSING_TOKEN_LOG_PRIOR;

// (A `LENIENT_REL_PREFIX_PENALTY` scorer once lived here. It was
// retired because `try_rel_to_structural_repair` runs as preprocessing
// before any candidate is emitted, so the tie the penalty broke no
// longer exists. See `docs/refactor-006/decoder-architecture.md` §
// "Retired mechanisms" for the rationale and the gate that catches
// regressions to this recovery path.)

/// Bag-of-tokens scorer (foundational-plan §5.2).
///
/// Returns `(prior, posterior)` where:
///
/// - `prior` = Σ [`marque_capco::priors::token_log_prior`] over the
///   marking's canonical tokens **plus** Σ
///   [`marque_capco::priors::country_code_log_prior`] over the
///   marking's `rel_to` country codes (issue #233). This is the prior
///   alone — nothing else — and is what
///   [`Candidate::prior_log_odds`] is documented to carry (see
///   `crates/scheme/src/ambiguity.rs`). Tokens or country codes
///   missing from the baked tables contribute
///   [`MISSING_TOKEN_LOG_PRIOR`] (a below-observed-floor penalty)
///   rather than `0.0`. The country-code contribution is what lets
///   the decoder break fuzzy-correction ties between common (USA,
///   GBR, AUS) and rare-lookalike (USB-not-a-country, UZB, ASM, AUT)
///   trigraphs in REL TO blocks.
/// - `posterior` = `prior + Σ attempt.features[i].delta + structural
///   penalties`. This is the quantity the decoder sorts and thresholds
///   on. The only structural penalty today is
///   [`HARD_SPLITTER_ABSORPTION_PENALTY`], applied when the strict
///   parse buries a reserved dissem-control token in a SAR/SCI slot.
///
/// The null (prose) posterior is **not** computed here. Pre-#472 it
/// was, summed over the marking's canonical tokens; the canonical
/// token set is post-fuzzy-correction so the prose hypothesis was
/// evaluated on tokens the user never typed, biasing the
/// marking-vs-prose comparison whenever fuzzy correction shifted a
/// common prose acronym (e.g., `(CMS)`) to a rare CAPCO token (e.g.,
/// `CTS`). Issue #472 moves the null computation to
/// [`observed_prose_log_prior`], which walks the original `bytes`
/// parameter to `recognize` and sums prose priors per distinct
/// observed token — restoring the symmetric marking-vs-prose
/// comparison. The caller computes the observed null once per
/// `recognize` call and writes it into every
/// [`ScoredCandidate::null_posterior`].
///
/// Splitting prior and posterior prevents the caller from writing the
/// full posterior into `Candidate::prior_log_odds` — that would double-
/// count the feature deltas once any resolver re-adds
/// `EvidenceFeature.log_odds`. Structural penalties are deliberately
/// folded into the posterior only (not the prior or the per-feature
/// log-odds): they are a likelihood statement about parse plausibility,
/// not a corpus-frequency claim about token co-occurrence.
///
/// Precision: computed in `f32` — the baked priors are already `f32`
/// and the feature deltas are small constants (single-digit magnitude
/// at most), so the accumulator doesn't need `f64` headroom for the
/// K=8 candidate set.
///
/// The `kind` parameter selects portion vs banner canonical token
/// forms for the prior computation (e.g., `S` vs `SECRET`) so the
/// marking-side lookup matches the input shape.
pub(crate) fn score_candidate(
    attempt: &CanonicalAttempt,
    marking: &CapcoMarking,
    kind: MarkingType,
) -> (f32, f32) {
    // Prior: sum of baked log-priors for the canonical tokens that
    // appear in the parsed marking. Tokens missing from the baked
    // table receive the floor penalty rather than a neutral 0.0
    // contribution — see the MISSING_TOKEN_LOG_PRIOR doc.
    let mut prior: f32 = 0.0;

    // Issue #451: linear-search dedup over a SmallVec rather than a
    // BTreeSet. N (distinct canonical tokens per marking) is typically
    // ≤10, so a small stack-allocated buffer with linear `iter().any`
    // dedup is cache-friendlier than B-tree node allocations, and
    // folding the prior summation into the same dedup loop kills the
    // intermediate token collection entirely.
    let mut seen_tokens: SmallVec<[&'static str; 16]> = SmallVec::new();
    for_each_canonical_token(marking, kind, |token| {
        if !seen_tokens.contains(&token) {
            seen_tokens.push(token);
            prior +=
                marque_capco::priors::token_log_prior(token).unwrap_or(MISSING_TOKEN_LOG_PRIOR);
        }
    });

    // Country-code prior contribution (issue #233). REL TO country
    // codes are not part of the `for_each_canonical_token` set because
    // `CountryCode::as_str()` returns a borrowed `&str` rather than
    // `&'static str`, and because the per-token corpus coverage for
    // country codes used to be sparse. Issue #233 adds a parallel
    // `COUNTRY_CODE_BASE_RATES` table (issue #186 sub-feature 1) so
    // the decoder can break fuzzy ties between popular codes (USA,
    // GBR, AUS, FVEY, …) and rare lookalikes (UZB, ASM,
    // AUT-as-Austria) by log-prior delta rather than edit distance
    // alone. Look up each observed REL TO code at score-time —
    // shape-agnostic, so the loop handles 2-char (`EU`), 3-char, and
    // 4-char tetragraphs uniformly. Duplicate REL TO entries do not
    // provide additional evidence, so score each distinct country
    // code at most once. Unknown entries fall to
    // MISSING_TOKEN_LOG_PRIOR — the same penalty the non-country-code
    // path uses for unrecognized tokens, which is the correct
    // behavior for a candidate that resolved to a non-CVE country
    // string.
    //
    // Issue #451 sub-finding F3: SmallVec linear-search dedup over the
    // typical N=1-5 REL TO codes, rather than a per-call BTreeSet
    // allocation.
    let mut seen_rel_to_codes: SmallVec<[&str; 8]> = SmallVec::new();
    for country in marking.0.rel_to.iter() {
        let code = country.as_str();
        if !seen_rel_to_codes.contains(&code) {
            seen_rel_to_codes.push(code);
            prior += marque_capco::priors::country_code_log_prior(code)
                .unwrap_or(MISSING_TOKEN_LOG_PRIOR);
        }
    }

    // Posterior: prior plus feature deltas plus structural penalties.
    let feature_sum: f32 = attempt.features.iter().map(|f| f.delta).sum();
    let mut posterior = prior + feature_sum;
    if absorbs_hard_splitter_in_sar_or_sci(marking) {
        posterior += HARD_SPLITTER_ABSORPTION_PENALTY;
    }
    posterior += custom_sci_marking_penalty(marking);

    (prior, posterior)
}

/// Total per-entry penalty for SCI markings whose strict parse landed
/// with [`SciControlSystem::Custom`] as the control system. See
/// [`CUSTOM_SCI_MARKING_PENALTY`] for rationale, including why this
/// discriminates on `sm.system` rather than on
/// `sm.canonical_enum.is_none()`.
pub(crate) fn custom_sci_marking_penalty(marking: &CapcoMarking) -> f32 {
    let attrs = &marking.0;
    let custom_count = attrs
        .sci_markings
        .iter()
        .filter(|sm| matches!(sm.system, SciControlSystem::Custom(_)))
        .count();
    custom_count as f32 * CUSTOM_SCI_MARKING_PENALTY
}

/// True when the strict parse of a candidate buries a hard-splitter
/// dissem-control token (NOFORN, ORCON, EXDIS, FOUO, …) inside a SAR
/// program/compartment/sub-compartment slot or an SCI compartment/
/// sub-compartment slot.
///
/// Used by [`score_candidate`] to apply
/// [`HARD_SPLITTER_ABSORPTION_PENALTY`] — the penalty exists because
/// SAR's grammar accepts any alphanumeric identifier and quietly
/// absorbs trailing dissem-control tokens that should have been
/// separated from the SAR block by `//`. See the
/// `HARD_SPLITTER_ABSORPTION_PENALTY` doc for the full rationale.
///
/// Identifiers are checked both as whole strings AND as whitespace-
/// separated word sequences. The whitespace split matters for the
/// `Full` SAR indicator form (`SPECIAL ACCESS REQUIRED-BUTTER
/// POPCORN`): a multi-word program nickname like `"BUTTER POPCORN"`
/// may have `NOFORN` absorbed as a trailing word, producing
/// `identifier: "BUTTER POPCORN NOFORN"`. Without the per-word
/// check, the absorption pattern slips past the whole-string
/// `is_hard_splitter` lookup.
pub(crate) fn absorbs_hard_splitter_in_sar_or_sci(marking: &CapcoMarking) -> bool {
    let attrs = &marking.0;

    if let Some(sar) = attrs.sar_markings.as_ref() {
        for prog in sar.programs.iter() {
            if contains_hard_splitter_word(&prog.identifier) {
                return true;
            }
            for comp in prog.compartments.iter() {
                if contains_hard_splitter_word(&comp.identifier) {
                    return true;
                }
                if comp
                    .sub_compartments
                    .iter()
                    .any(|sub| contains_hard_splitter_word(sub))
                {
                    return true;
                }
            }
        }
    }

    for sci in attrs.sci_markings.iter() {
        for comp in sci.compartments.iter() {
            if contains_hard_splitter_word(&comp.identifier) {
                return true;
            }
            if comp
                .sub_compartments
                .iter()
                .any(|sub| contains_hard_splitter_word(sub))
            {
                return true;
            }
        }
    }

    false
}

/// True when `s` is a hard-splitter token, or contains a hard-
/// splitter token as a whitespace-separated word. The per-word check
/// covers multi-word `Full` SAR program nicknames (`BUTTER POPCORN`)
/// that absorbed a trailing dissem-control word.
pub(crate) fn contains_hard_splitter_word(s: &str) -> bool {
    if is_hard_splitter(s) {
        return true;
    }
    s.split_whitespace().any(is_hard_splitter)
}

/// Visit each canonical token present in `marking` that has a
/// `&'static str` representation suitable for
/// [`marque_capco::priors::TOKEN_BASE_RATES`] lookup.
///
/// Issue #451: this replaces the previous `canonical_tokens_for ->
/// Vec<&'static str>` shape, which allocated a `BTreeSet` for dedup
/// and a `Vec` for the return on every scored candidate (up to 16 per
/// `recognize()` call). The visitor pattern hands raw, possibly-
/// duplicate tokens to the caller; dedup happens at the call site
/// where it can ride along with whatever per-token work the caller is
/// already doing (e.g., [`score_candidate`] folds the prior summation
/// into the same SmallVec linear-search dedup).
///
/// Scored token families, by `CanonicalAttrs` field:
///
/// - `classification` — effective level's banner string
///   (`SECRET`, `TOP SECRET`, ...).
/// - `sci_controls` — each variant's `as_str()` (`SI`, `TK`, `HCS-P`, ...).
/// - `dissem_controls` — IC dissem variants' `as_str()`
///   (`NF`, `OC`, `RELIDO`, ...).
/// - `non_ic_dissem` — non-IC dissem variants' `banner_str()`
///   (`LIMDIS`, `EXDIS`, `NODIS`, `SBU`, `LES`, ...).
/// - `aea_markings` — category token `"AEA"` when any AEA marking is
///   present. Individual AEA sub-variants (RD / FRD / CNWDI /
///   SIGMA / UCNI variants) are not broken out for scoring because
///   the baked priors don't carry per-sub-variant base rates and
///   adding floor-penalty contributions for each variant would hurt
///   AEA-bearing candidates across the board.
/// - `fgi_marker` — category token `"FGI"` when an FGI marker is set.
///
/// Deliberately NOT included here:
///
/// - `sar_markings` — SAR program identifiers are agency-assigned
///   codewords (open set, not in the baked priors).
/// - `rel_to` country codes — scored separately in
///   [`score_candidate`] via
///   [`marque_capco::priors::country_code_log_prior`] (issue #233).
///   `CountryCode::as_str()` returns a `&str` tied to `&self`, not
///   `&'static str`, so the country-code contribution is summed at
///   score-time rather than collected here.
/// - CAB fields (`classified_by`, `derived_from`, `declassify_on`) —
///   free-form text, not CVE-enumerable.
///
/// Duplicate tokens (the unified `dissem_iter` can yield repeats
/// across namespaces; an `sci_controls` slot could in principle repeat
/// across positions) ARE visited per-occurrence — the caller must
/// dedup if double-counting matters. The previous `BTreeSet`-based
/// implementation deduped internally; the caller-side dedup in
/// [`score_candidate`] preserves the same per-distinct-token behavior.
pub(crate) fn for_each_canonical_token(
    marking: &CapcoMarking,
    kind: MarkingType,
    mut f: impl FnMut(&'static str),
) {
    let attrs = &marking.0;

    if let Some(class) = attrs.classification.as_ref() {
        // Pick the classification token that matches the marking shape and
        // classification system.
        //
        // For US/FGI/Joint portions: single-letter abbrevs (`S`, `C`, `U`, `TS`)
        //   via `effective_level().portion_str()`. Pre-#258 this always used the
        //   banner form; the portion form correctly matches low-prose-frequency
        //   single-letter tokens vs single-letter prose tokens, enabling the
        //   null-hypothesis filter to reject prose inputs like Federalist `(s)`.
        //
        // For NATO portions: use the NATO-specific abbreviation (`NS`, `NR`, `NC`,
        //   `NU`, `CTS`) directly from `NatoClassification::portion_str()`. Using
        //   `effective_level().portion_str()` here would yield `"R"`, `"C"`, etc.
        //   (US equivalents), which have high prose frequency (e.g., `"R"` appears
        //   5 797× in prose vs 1× in marking corpus), causing the null-hypothesis
        //   filter to reject valid NATO-folded portions like `(//NR)`. The NATO
        //   abbreviations have near-zero prose frequency and fall to
        //   `MISSING_TOKEN_LOG_PRIOR` (−12.0) on both sides, giving a neutral
        //   (zero) marking-y delta rather than a prose-weighted penalty. T129 /
        //   #260 fix; companion to `NATO_PORTION_FORMS` in `marque-ism::token_set`.
        //
        // For banner/CAB/PageBreak: always use the full-word form regardless of
        //   classification system (fold is portion-only; banners reach here via
        //   the non-folding strict-recognizer path or decoder direct banner inputs).
        let class_token = match kind {
            MarkingType::Portion => match class {
                MarkingClassification::Nato(n) => n.portion_str(),
                _ => class.effective_level().portion_str(),
            },
            // All non-Portion variants — kept as wildcard for
            // `#[non_exhaustive]` forward-compat (issue #461). The
            // feature extractor is shape-only and `banner_str()` is
            // the safe fallback for any non-portion shape, including
            // future variants of the `MarkingType` enum.
            _ => class.effective_level().banner_str(),
        };
        f(class_token);
    }

    for ctrl in attrs.sci_controls.iter() {
        f(ctrl.as_str());
    }
    // PR 9b (T132): the decoder feature extractor inserts dissem
    // canonical tokens regardless of namespace — the feature vector
    // captures "which control names appear?", not their attribution.
    for dis in attrs.dissem_iter() {
        f(dis.as_str());
    }
    for nic in attrs.non_ic_dissem.iter() {
        // `NonIcDissem::banner_str` returns `&'static str` with the
        // banner form (LIMDIS, EXDIS, NODIS, SBU, LES, SSI,
        // SBU NOFORN, LES NOFORN). The compound forms ("SBU NOFORN",
        // "LES NOFORN") won't hit a single-token priors entry — they
        // fall to MISSING_TOKEN_LOG_PRIOR. That's fine: the
        // comparison against peer candidates remains consistent.
        f(nic.banner_str());
    }
    if !attrs.aea_markings.is_empty() {
        f("AEA");
    }
    if attrs.fgi_marker.is_some() {
        f("FGI");
    }
}



/// True when `token` is an unambiguous segment-starting dissem
/// long-form. These tokens have no in-segment role inside SCI / SAR /
/// REL TO blocks, so seeing one after whitespace always indicates a
/// missing `//` separator. Pinned by
/// `try_insert_delimiter_inserts_before_long_form_dissem`.
///
/// Excluded from this set:
///
/// - 2-char short forms (`NF`, `OC`, `PR`, `IMC`, `RS`) — could
///   collide with SAR compartment / sub-compartment naming.
/// - SCI starters (`SI`, `HCS`, `TK`, `KDK`) — 2-3 char tokens that
///   appear in compartment context.
/// - SAR prefixes (`SAR-*`) — handled in v2 with classification-
///   context lookahead.
pub(crate) fn is_hard_splitter(token: &str) -> bool {
    matches!(
        token,
        "NOFORN"
            | "ORCON"
            | "ORCON-USGOV"
            | "PROPIN"
            | "IMCON"
            | "RELIDO"
            | "RSEN"
            | "EYESONLY"
            | "FOUO"
            | "FISA"
            | "DSEN"
            | "EXDIS"
            | "NODIS"
            | "LIMDIS"
    )
}
