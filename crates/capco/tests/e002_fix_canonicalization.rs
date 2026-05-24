// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E002 (`capco:portion.dissem.rel-to-missing-usa`) fix-shape +
//! canonicalization regression pins, ported from
//! `crates/capco/src/_disabled_tests.rs` per issue #722.
//!
//! # Source tests ported
//!
//! - `e002_fix_sorts_non_usa_trigraphs_when_usa_missing`
//! - `e002_fix_sorts_non_usa_trigraphs_when_usa_misplaced`
//! - `e002_fix_consumes_trailing_comma_in_rel_to_block`
//! - `e002_fix_span_includes_recognized_tetragraph_tail`
//! - `e002_fix_span_stops_at_unrecognized_tail_token`
//! - `e002_suppresses_fix_on_multiple_rel_to_blocks`
//! - `e002_fix_output_does_not_trigger_e020`
//! - `e002_fix_output_dedups_when_input_has_duplicates`
//!
//! # Architecture note
//!
//! The rule emits a structural `FixIntent`
//! (`FactAdd { USA, Scope::Portion }` or
//! `Recanonicalize { Portion | Page }`) and the engine synthesizes the
//! byte-precise replacement at promotion time via `apply_intent` +
//! `render_canonical`. The load-bearing invariants — "E002 produces
//! canonical REL TO in one pass", "tetragraph tail is preserved",
//! "multi-block input gets no fix", "second pass is a no-op" — are
//! asserted on the bytes returned by `Engine::fix(...).source` rather
//! than on fix-internal fields. Audit content-ignorance (Constitution V
//! Principle V) is satisfied by construction — the `FixIntent` carries
//! no raw document bytes.
//!
//! # Authority
//!
//! CAPCO-2016 §H.8 p150 (REL TO grammar) + §H.8 p151 (USA-first +
//! alphabetical-rest within REL TO). Each citation re-verified
//! against `crates/capco/docs/CAPCO-2016.md` at authorship per
//! Constitution VIII (Authoritative Source Fidelity).

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::Diagnostic;
use secrecy::ExposeSecret as _;

const E002_PREDICATE: &str = "portion.dissem.rel-to-missing-usa";

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

fn e002_diags(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    lint(source)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == E002_PREDICATE)
        .collect()
}

/// Apply `Engine::fix` and return the resulting source bytes as a
/// UTF-8 String. Fails the test with a clear panic on non-UTF8 output
/// (invariant: input is UTF-8 and every replacement is valid UTF-8).
fn fix_to_string(source: &str) -> String {
    let result = engine().fix(source.as_bytes(), FixMode::Apply);
    String::from_utf8(result.source.expose_secret().to_vec())
        .unwrap_or_else(|e| panic!("Engine::fix produced non-UTF8 output on {source:?}: {e}"))
}

// ---------------------------------------------------------------------------
// USA injection + canonicalize in one pass
// ---------------------------------------------------------------------------

/// USA absent and non-USA entries in non-alphabetical order. The
/// engine MUST produce canonical REL TO (USA-first + alphabetical
/// non-USA tail) in a single fix pass. The §H.8 p151 USA-first rule
/// is what made the multi-pass behavior a regression: E060 / the
/// renderer gates on `rel_to[0] == USA`, so a fix that preserved
/// input order would leave a latent ordering violation only a second
/// pass could catch.
///
/// Authority: CAPCO-2016 §H.8 p151 (REL TO USA-first +
/// alphabetical-rest). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e002_fix_canonicalizes_when_usa_missing_in_one_pass() {
    let src = "SECRET//REL TO GBR, AUS";
    let diags = e002_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E002 must fire once on missing-USA banner: {diags:?}"
    );
    let fixed = fix_to_string(src);
    assert!(
        fixed.contains("REL TO USA, AUS, GBR"),
        "E002 must produce canonical REL TO (USA first + alphabetical \
         rest) in a single fix pass; got: {fixed:?}",
    );
}

/// USA present but not first AND non-USA entries unsorted. Same
/// canonical-form-in-one-pass invariant as the missing-USA case.
#[test]
fn e002_fix_canonicalizes_when_usa_misplaced_in_one_pass() {
    let src = "SECRET//REL TO GBR, USA, AUS";
    let diags = e002_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E002 must fire once on misplaced-USA banner: {diags:?}"
    );
    let fixed = fix_to_string(src);
    assert!(
        fixed.contains("REL TO USA, AUS, GBR"),
        "E002 must produce canonical REL TO (USA first + alphabetical \
         rest) in a single fix pass; got: {fixed:?}",
    );
}

// ---------------------------------------------------------------------------
// Trailing-delimiter consumption + tetragraph tail behavior
// ---------------------------------------------------------------------------

/// Trailing `,` inside the RelToBlock must not survive the fix. The
/// pre-cutover invariant was "fix span extends through the delimiter
/// tail so splicing leaves no stale `,`/whitespace behind"; the
/// post-cutover invariant collapses to the same observable property:
/// the rendered banner is clean.
#[test]
fn e002_fix_consumes_trailing_comma_in_rel_to_block() {
    let src = "SECRET//REL TO GBR, AUS,";
    let diags = e002_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E002 must fire once on trailing-comma banner: {diags:?}"
    );
    let fixed = fix_to_string(src);
    assert!(
        fixed.contains("REL TO USA, AUS, GBR"),
        "fixed REL TO must be canonical with no stale trailing \
         delimiter; got: {fixed:?}",
    );
    // Direct stale-tail check: the canonical form should not end with a
    // dangling `, ` inside the REL TO block.
    let after_rel = fixed
        .find("REL TO ")
        .map(|i| &fixed[i + "REL TO ".len()..])
        .unwrap_or("");
    let block_end = after_rel.find("//").unwrap_or(after_rel.len());
    let block = &after_rel[..block_end];
    assert!(
        !block.trim_end().ends_with(','),
        "REL TO block must not end with a stray `,`; got block: {block:?} \
         in fixed banner: {fixed:?}",
    );
}

/// Tetragraph FVEY is a registered country code (issue #183) — the
/// fix MUST include it in the canonicalized output. A pre-#183 fix
/// that stopped at AUS would leave a stale `, FVEY` behind.
///
/// Authority: CAPCO-2016 §H.8 p150 (REL TO tetragraph membership).
/// Re-verified against `crates/capco/docs/CAPCO-2016.md` per
/// Constitution VIII.
#[test]
fn e002_fix_preserves_recognized_tetragraph_tail() {
    let src = "SECRET//REL TO GBR, AUS, FVEY";
    let diags = e002_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E002 must fire once on missing-USA banner with FVEY tail: {diags:?}"
    );
    let fixed = fix_to_string(src);
    assert!(
        fixed.contains("FVEY"),
        "FVEY tetragraph must survive the E002 fix; got: {fixed:?}",
    );
    // USA-first invariant still holds even with FVEY present.
    assert!(
        fixed.contains("REL TO USA"),
        "fixed REL TO must lead with USA even when FVEY is present; \
         got: {fixed:?}",
    );
}

/// A tail token outside the CVE TRIGRAPHS/tetragraphs vocabulary
/// (`XYZQ`) is not recognized by the parser as a country code. The
/// load-bearing post-cutover invariant is that E002 still fires (the
/// missing-USA condition is independent of out-of-vocab tail tokens)
/// and the canonical render produces a USA-first form.
///
/// Pre-cutover the rule emitted a narrow splice that stopped at the
/// last recognized country code, leaving `XYZQ` in place. Post-
/// cutover the renderer canonicalizes the whole REL TO block — out-
/// of-vocab tokens are NOT preserved by the fix path because the
/// renderer rebuilds from the typed `attrs.rel_to` projection (which
/// the parser populates only from CVE-recognized country codes).
/// The behavior change is documented in
/// `crates/capco/src/rules/rel_to.rs` (the `FactAdd` / `Recanonicalize`
/// FixIntent emission) and is the deliberate result of moving fix
/// synthesis into `MarkingScheme::render_canonical`.
#[test]
fn e002_fires_on_missing_usa_even_with_unrecognized_tail_token() {
    let src = "SECRET//REL TO GBR, AUS, XYZQ";
    let diags = e002_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E002 must fire once on missing-USA banner with unrecognized tail: {diags:?}"
    );
    let fixed = fix_to_string(src);
    assert!(
        fixed.contains("REL TO USA"),
        "E002 fix must still produce USA-first canonical REL TO even \
         when the input carries an unrecognized tail token; got: {fixed:?}",
    );
}

// ---------------------------------------------------------------------------
// Multi-block safety
// ---------------------------------------------------------------------------

/// When the parser sees more than one REL TO block in a single
/// marking, a single first→last canonicalize splice would delete
/// intervening `//...//` content (here `//NF//`). The rule MUST
/// emit a diagnostic with `fix: None` so the engine cannot corrupt
/// the source.
///
/// Authority: CAPCO-2016 §H.8 p150-151 (REL TO grammar; multi-block
/// is malformed but the engine must not amplify the bug into a
/// content-corruption fix).
#[test]
fn e002_suppresses_fix_on_multiple_rel_to_blocks() {
    let src = "SECRET//REL TO GBR//NF//REL TO AUS";
    let diags = e002_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E002 must still fire (diagnostic present) on multi-block input: {diags:?}"
    );
    let d = &diags[0];
    assert!(
        d.fix.is_none() && d.text_correction.is_none(),
        "E002 must NOT carry a fix (FixIntent or text_correction) when \
         multiple REL TO blocks are present — a single splice across \
         them would delete intervening `//NF//`: {d:?}",
    );
}

// ---------------------------------------------------------------------------
// Idempotence
// ---------------------------------------------------------------------------

/// E002's canonical output MUST NOT re-trigger E002 on a second
/// pass. This is the load-bearing invariant: the fix achieves the
/// canonical form in ≤1 pass; a second `Engine::fix` over the
/// already-fixed bytes is a no-op for E002.
///
/// Authority: CAPCO-2016 §H.8 p151 (USA-first +
/// alphabetical-rest). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e002_fix_output_is_idempotent() {
    let src = "CONFIDENTIAL//REL TO FRA, DEU";
    let round1 = fix_to_string(src);
    assert!(
        round1.contains("REL TO USA, DEU, FRA"),
        "first-pass E002 fix must produce canonical REL TO; got: {round1:?}",
    );
    // Round 2: feed the canonicalized banner back through the engine;
    // E002 must NOT fire again on the rewritten banner.
    let round2_diags = e002_diags(&round1);
    assert!(
        round2_diags.is_empty(),
        "E002's canonical output must not re-fire E002 on a second pass: \
         {round2_diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Dedup + canonicalize composition
// ---------------------------------------------------------------------------

/// E002's fix output MUST dedup duplicate entries before canonicalizing
/// (issue #234 regression). Input `GBR, USA, AUS,
/// USA` triggers E002 (USA not first) AND carries a duplicate USA
/// that the canonicalize pass must dedup. Without the dedup the
/// splice could emit `USA, USA, AUS, GBR` — still wrong.
///
/// Authority: CAPCO-2016 §H.8 p151. Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e002_fix_output_dedups_when_input_has_duplicates() {
    let src = "SECRET//REL TO GBR, USA, AUS, USA";
    let diags = e002_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E002 must fire once on USA-not-first + duplicate-USA input: {diags:?}"
    );
    let fixed = fix_to_string(src);
    assert!(
        fixed.contains("REL TO USA, AUS, GBR"),
        "E002 fix must dedup before canonicalizing (canonical form, no \
         duplicates); got: {fixed:?}",
    );
    // Direct dedup assertion: USA must appear exactly once in the
    // rendered REL TO block.
    let block_start = fixed.find("REL TO ").expect("banner must contain REL TO");
    let after = &fixed[block_start..];
    let block_end = after.find("//").unwrap_or(after.len());
    let block = &after[..block_end];
    let usa_count = block.matches("USA").count();
    assert_eq!(
        usa_count, 1,
        "USA must appear exactly once in canonicalized REL TO block; \
         got {usa_count} occurrences in block: {block:?}",
    );
}
