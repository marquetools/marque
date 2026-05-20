// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 9c.2 / FR-048 — S007 `bare-nato-requires-rel-to-usa-nato`.
//!
//! A bare NATO classification portion appearing in a US-classified
//! document should carry `REL TO USA, NATO` per CAPCO-2016 §H.7 p127
//! Notional Example 2 worked example
//! `(//CTS//BOHEMIA//REL TO USA, NATO)` — "a NATO COSMIC TOP SECRET
//! (CTS) BOHEMIA portion within a US classified document and is
//! releasable back to NATO".
//!
//! The rule is `Severity::Suggest` (the citation is example-derived
//! rather than mandate prose; S004 + S005 precedent — the latter
//! post-PR-#488 collapse of the historical S005/S006 pair).
//! Solely-NATO documents are carved out via
//! [`marque_ism::ProjectedMarking::is_solely_nato_classified`].
//!
//! # Authority
//!
//! - CAPCO-2016 §H.7 p127 Notional Example 2 — the worked example for
//!   bare-NATO portions in a US-classified document.
//! - Project memory `project_fr048_nato_rel_to_portion_level` — fix
//!   is portion-level, not banner-level; banner roll-up flows
//!   automatically.
//! - Project memory `project_atomal_is_aea` — ATOMAL on the AEA axis
//!   coexists with `MarkingClassification::Nato`; S007 fires regardless.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::{Config, RuleConfig};
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::Severity;
use secrecy::ExposeSecret as _;
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme constructs without rewrite cycles")
}

/// Engine with S007 severity overridden to `Fix` so the engine applies
/// the text correction. The default `Severity::Suggest` is a hard
/// exclusion from auto-apply (`engine.rs::Engine::fix_inner` line
/// 2853); the override mirrors the production code path users take via
/// `[rules] S007 = "fix"` in `.marque.toml`.
///
/// The default `confidence_threshold` is `0.95`; S007 emits at the
/// `S007_SUGGEST_CONFIDENCE` constant defined in
/// `marque_capco::rules` (currently `0.85` — example-derived citation
/// calibrates with S005). Without the threshold drop the
/// suggest-channel demotion loop in `engine.rs::lint` would demote the
/// overridden `Fix` back to `Suggest` because `0.85 < 0.95`, which
/// would defeat the override. We drop the threshold to `0.80` here to
/// exercise the apply path end-to-end — users who want auto-apply set
/// both `[rules] S007 = "fix"` AND `confidence_threshold = 0.80` (or
/// lower) in `.marque.toml`. The threshold ladder relationship and
/// the rationale for the `0.85` calibration live on the
/// `S007_SUGGEST_CONFIDENCE` constant doc-comment in `rules.rs`; this
/// helper deliberately does NOT import the constant (the indirection
/// is the production contract, not a test detail to pin numerically).
fn engine_with_s007_as_fix() -> Engine {
    let mut overrides = HashMap::new();
    overrides.insert("S007".to_owned(), "fix".to_owned());
    let mut config = Config::default();
    config.rules = RuleConfig { overrides };
    config
        .set_confidence_threshold(0.80)
        .expect("0.80 is in [0.0, 1.0]");

    Engine::with_clock(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme constructs without rewrite cycles")
}

fn lint_s007(source: &[u8]) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    let result = engine().lint(source);
    result
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.as_str() == "S007")
        .collect()
}

fn fix_with_s007_promoted(source: &[u8]) -> String {
    let result = engine_with_s007_as_fix().fix(source, FixMode::Apply);
    String::from_utf8(result.source.expose_secret().to_vec()).expect("engine output is valid UTF-8")
}

// =========================================================================
// Example A — NATO portion + US portion: S007 fires on the NATO portion.
// =========================================================================

#[test]
fn example_a_nato_plus_us_fires_once() {
    // CAPCO-2016 §H.7 p127 Notional Example 2: a NATO portion sitting
    // in a US-classified document needs `REL TO USA, NATO`. The
    // sibling US portion `(S//REL TO USA, FVEY)` evidences the
    // document's US-classification status; S007 must fire on the bare
    // NATO portion.
    let source = b"(//NS)\n(S//REL TO USA, FVEY)";
    let diags = lint_s007(source);
    assert!(
        !diags.is_empty(),
        "S007 must fire on bare NATO `(//NS)` portion sitting in a \
         US-classified document; got {} diagnostic(s)",
        diags.len(),
    );
    assert!(
        diags.iter().any(|d| d.severity == Severity::Suggest),
        "S007 default severity is Suggest; got severities: {:?}",
        diags.iter().map(|d| d.severity).collect::<Vec<_>>(),
    );
    assert!(
        diags
            .iter()
            .all(|d| format!("{}", d.citation).contains("§H.7 p127")),
        "S007 citation must reference §H.7 p127; got: {:?}",
        diags
            .iter()
            .map(|d| format!("{}", d.citation))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn example_a_already_rel_to_silent() {
    // CAPCO-2016 §H.7 p127 Notional Example 2: the canonical form
    // already carries `REL TO USA, NATO`. S007 must NOT fire.
    let source = b"(//NS//REL TO USA, NATO)\n(S//REL TO USA, FVEY)";
    let diags = lint_s007(source);
    assert!(
        diags.is_empty(),
        "S007 must NOT fire when bare NATO portion already carries \
         `REL TO USA, NATO`; got {} diagnostic(s)",
        diags.len(),
    );
}

// =========================================================================
// Example B — NATO + JPN FGI: NATO portion canonical; FGI sibling is
// FGI-classified, not US. S007 silent on the NATO portion.
// =========================================================================

#[test]
fn example_b_nato_plus_jpn_fgi_silent() {
    // CAPCO-2016 §H.7 p127 Notional Example 2 baseline: the NATO
    // portion `(//NU//REL TO USA, NATO)` is already canonical (carries
    // both USA and NATO in REL TO). S007 silent regardless of the
    // sibling portion's character.
    let source = b"(//NU//REL TO USA, NATO)\n(//JPN U//NF)";
    let diags = lint_s007(source);
    assert!(
        diags.is_empty(),
        "S007 must NOT fire when NATO portion already canonical; \
         got {} diagnostic(s)",
        diags.len(),
    );
}

// =========================================================================
// Example C — NATO + US FOUO: NATO portion canonical, US sibling.
// =========================================================================

#[test]
fn example_c_nato_plus_us_fouo_silent() {
    // CAPCO-2016 §H.7 p127 Notional Example 2 baseline: NATO portion
    // already canonical. S007 silent.
    let source = b"(//CTS//REL TO USA, NATO)\n(U//FOUO)";
    let diags = lint_s007(source);
    assert!(
        diags.is_empty(),
        "S007 must NOT fire when NATO portion already canonical; \
         got {} diagnostic(s)",
        diags.len(),
    );
}

// =========================================================================
// Solely-NATO carve-out — `ProjectedMarking::is_solely_nato_classified`
// is read by the rule but the engine's current portion-rule dispatch
// in `Engine::lint` gates `with_page_marking(ctx_page_marking)` on
// `candidate.kind != MarkingType::Portion && !page_context.is_empty()`
// — only banner/CAB candidates receive a populated `page_marking`,
// not per-portion `RuleContext`s. The rule's carve-out branch is
// therefore load-bearing for a future engine migration that plumbs
// `page_marking` to portion rules; today it is preserved as
// forward-looking code with no current short-circuit.
//
// Until that migration lands, S007 fires on every bare-NATO portion
// regardless of solely-NATO document status. Users in solely-NATO
// contexts can silence with `[rules] S007 = "off"` in `.marque.toml`.
// =========================================================================

#[test]
fn solely_nato_doc_two_portions_conservative_fire() {
    // CAPCO-2016 §H.7 p127 Notional Example 2: the solely-NATO
    // carve-out cannot evaluate under the current engine (portion
    // rules receive `ctx.page_marking = None`), so S007 fires on
    // both bare-NATO portions conservatively. PM decision #2: this
    // single-pass false-positive in solely-NATO docs is an
    // acceptable rare case (users silence via config).
    //
    // The S007 rule's `is_solely_nato_classified` branch is
    // load-bearing for a future engine pass that plumbs page_marking
    // to portions; this test pins the present-day behavior so the
    // migration trip-wires.
    //
    // ============================================================
    // MIGRATION TRIP-WIRE — read before flipping the assertion.
    // ============================================================
    //
    // When the engine plumbs `page_marking` to portion-rule dispatch
    // (today only banner/CAB candidates receive a populated
    // `RuleContext::page_marking`), this test must change. The
    // migrating engineer MUST first decide which semantic the engine
    // implements for "when does a portion rule see the accumulator":
    //
    // (a) **Snapshot-before-add.** Portion rule sees the accumulator
    //     state BEFORE the current portion is folded in. For input
    //     `(//CTS)\n(//NS)`:
    //       - Portion 1 (`//CTS`) — accumulator empty;
    //         `is_solely_nato_classified()` returns `false` (the
    //         `!is_empty()` guard). Carve-out does NOT fire. S007
    //         emits → 1 diagnostic.
    //       - Portion 2 (`//NS`) — accumulator contains the bare-NATO
    //         portion 1; `is_solely_nato_classified()` returns `true`.
    //         Carve-out fires. S007 silent → 0 diagnostics.
    //     Migration assertion: `assert_eq!(diags.len(), 1, ...)`.
    //
    // (b) **Snapshot-after-add.** Portion rule sees the accumulator
    //     state AFTER the current portion is folded in. For the same
    //     input both snapshots contain ≥1 bare-NATO portion and no
    //     non-NATO portions; `is_solely_nato_classified()` returns
    //     `true` both times and the carve-out silences both. S007
    //     silent on both portions → 0 diagnostics.
    //     Migration assertion: `assert_eq!(diags.len(), 0, ...)`.
    //
    // The single-pass behavior pinned today (2 diagnostics) is the
    // `page_marking = None` conservative-fire path; it matches
    // neither (a) nor (b). The migration author MUST pick the engine
    // semantic, update both the assertion AND this comment to reflect
    // the chosen branch, and re-verify against the
    // `is_solely_nato_classified_true_on_pure_nato` unit test in
    // `crates/ism/src/projected.rs` (which is the
    // single-portion-bare-NATO base case the analysis above relies
    // on). Do NOT blindly flip to a hard-coded number.
    let source = b"(//CTS)\n(//NS)";
    let diags = lint_s007(source);
    assert_eq!(
        diags.len(),
        2,
        "S007 conservative-fire path: both bare-NATO portions fire \
         under the current engine because portion rules receive \
         `ctx.page_marking = None`. See the migration trip-wire \
         comment above for the snapshot-semantic decision the next \
         engineer must make before changing this assertion. Got {} \
         diagnostic(s)",
        diags.len(),
    );
}

#[test]
fn solely_nato_single_portion_fires_then_silent_after_fix() {
    // CAPCO-2016 §H.7 p127 Notional Example 2: a single-portion
    // solely-NATO document is the conservative-fire case per the
    // S007 docblock (PR 9c.2 PM decision #2). The first portion has
    // no sibling evidence; the conservative path fires.
    //
    // Users in solely-NATO contexts who consistently see false
    // positives can silence via `[rules] S007 = "off"` in
    // `.marque.toml`.
    //
    // Two-pass coverage:
    //   1. Pass-1 lint sees `(//CTS)` and emits exactly one S007
    //      diagnostic — the conservative-fire path is engaged
    //      because `ctx.page_marking = None` today.
    //   2. Applying the S007 fix produces `(//CTS//REL TO USA,
    //      NATO)`, which already covers `{USA, NATO}`. Pass-2 lint
    //      over the post-fix bytes silences S007 (the coverage
    //      check `has_usa && has_nato` short-circuits before the
    //      page_marking branch). This is the convergence guarantee
    //      a future engine migration that flips the conservative
    //      path also has to preserve; pinning it here documents
    //      the property independent of the carve-out's plumbing.
    let source = b"(//CTS)";
    let diags = lint_s007(source);
    // Conservative-fire: a single-portion bare-NATO doc produces
    // exactly one S007 diagnostic on pass-1.
    assert_eq!(
        diags.len(),
        1,
        "S007 fires conservatively on a single-portion bare-NATO \
         document (no sibling evidence to invoke the solely-NATO \
         carve-out); got {} diagnostic(s)",
        diags.len(),
    );

    // Apply the S007 fix and re-lint. Pass-2 sees canonical
    // `REL TO USA, NATO` coverage and must be silent.
    let pass1 = fix_with_s007_promoted(source);
    assert_eq!(
        pass1.trim_end(),
        "(//CTS//REL TO USA, NATO)",
        "pass-1 must produce the canonical insertion splice; got: \
         {:?}",
        pass1,
    );
    let diags_pass2 = lint_s007(pass1.as_bytes());
    assert!(
        diags_pass2.is_empty(),
        "pass-2 over canonical output must produce zero S007 \
         diagnostics (fixed point reached); got: {:?}",
        diags_pass2,
    );
}

// =========================================================================
// ATOMAL on AEA axis — bare NATO classification + AEA companion.
// S007 fires; ATOMAL routing is orthogonal to the bare-NATO predicate.
// =========================================================================

#[test]
fn atomal_in_us_doc_fires() {
    // CAPCO-2016 §H.7 p127 Notional Example 2: ATOMAL travels on the
    // AEA axis (PR 9c.1 T134) and the classification axis still
    // carries a bare NATO variant. The bare-NATO predicate is
    // orthogonal to AEA companions — S007 fires on the
    // `(//CTS//ATOMAL)` portion when a US sibling is present.
    let source = b"(//CTS//ATOMAL)\n(S//NF)";
    let diags = lint_s007(source);
    assert!(
        !diags.is_empty(),
        "S007 must fire on bare CTS + ATOMAL companion in a \
         US-classified document; ATOMAL on AEA axis does not \
         immunize the classification axis; got {} diagnostic(s)",
        diags.len(),
    );
}

// =========================================================================
// Augmentation branch — partial REL TO coverage.
// =========================================================================

#[test]
fn bare_nato_us_doc_missing_only_nato_fires() {
    // CAPCO-2016 §H.7 p127 Notional Example 2: `REL TO USA, CAN`
    // covers USA but not NATO. The augmentation branch should emit
    // `REL TO USA, CAN, NATO` (USA first; alpha-sorted remainder).
    let source = b"(//NS//REL TO USA, CAN)\n(S//NF)";
    let diags = lint_s007(source);
    assert!(
        !diags.is_empty(),
        "S007 must fire when REL TO covers USA but not NATO; got \
         {} diagnostic(s)",
        diags.len(),
    );

    let fixed = fix_with_s007_promoted(source);
    assert!(
        fixed.contains("REL TO USA, CAN, NATO"),
        "S007 augmentation must produce `REL TO USA, CAN, NATO` \
         (USA-first + alpha-sorted); got: {:?}",
        fixed,
    );
}

#[test]
fn bare_nato_us_doc_missing_only_usa_fires() {
    // CAPCO-2016 §H.7 p127 Notional Example 2: `REL TO NATO, GBR`
    // covers NATO but not USA (§A.6 / §H.8 mandates USA-first
    // canonical form). The augmentation branch should emit
    // `REL TO USA, GBR, NATO`.
    let source = b"(//NS//REL TO NATO, GBR)\n(S//NF)";
    let diags = lint_s007(source);
    assert!(
        !diags.is_empty(),
        "S007 must fire when REL TO covers NATO but not USA; got \
         {} diagnostic(s)",
        diags.len(),
    );

    let fixed = fix_with_s007_promoted(source);
    assert!(
        fixed.contains("REL TO USA, GBR, NATO"),
        "S007 augmentation must produce `REL TO USA, GBR, NATO` \
         (USA-first + alpha-sorted); got: {:?}",
        fixed,
    );
}

// =========================================================================
// NOFORN guard — `capco/noforn-conflicts-rel-to` owns the conflict.
// =========================================================================

#[test]
fn noforn_present_silent() {
    // CAPCO-2016 §H.8 p145: NOFORN cannot be used with REL TO. S007
    // must NOT propose a REL TO that another rule would immediately
    // remove. The `capco/noforn-conflicts-rel-to` page rewrite (in
    // `CapcoScheme`) is the owner of that conflict.
    let source = b"(//NS//NF)\n(S//NF)";
    let diags = lint_s007(source);
    assert!(
        diags.is_empty(),
        "S007 must NOT fire on a portion that carries NOFORN; \
         conflict is owned by `capco/noforn-conflicts-rel-to`; got \
         {} diagnostic(s)",
        diags.len(),
    );
}

// =========================================================================
// Constitution V convergence — apply S007 once, re-lint, assert silent.
// =========================================================================

#[test]
fn idempotency_after_one_pass() {
    // Constitution V Principle V convergence: applying S007 once
    // produces canonical output; a second pass over the canonical
    // output must be silent (zero S007 diagnostics).
    //
    // The pin below is exact-byte-equality rather than a substring
    // check so a regression that emits e.g. `(//NS//XYZ//REL TO
    // USA, NATO)` or duplicates the REL TO block fails this test.
    // The augmentation tests already do tight substring pinning;
    // the insertion test should hold the same bar.
    let source = b"(//NS)\n(S//NF)";
    let pass1 = fix_with_s007_promoted(source);

    // Pass-1 must produce exactly this byte sequence:
    //   - portion 1 has the splice `NS` → `NS//REL TO USA, NATO`
    //     applied at the Classification-token span;
    //   - portion 2 is unchanged (`S//NF` carries NOFORN, S007
    //     guard skips it);
    //   - `\n` separator between portions is preserved.
    // The engine emits no trailing whitespace beyond what the
    // input carried; the input has no trailing newline so the
    // output does not either.
    assert_eq!(
        pass1, "(//NS//REL TO USA, NATO)\n(S//NF)",
        "pass-1 must produce the exact canonical splice for the \
         insertion branch; got: {:?}",
        pass1,
    );

    let diags_pass2 = lint_s007(pass1.as_bytes());
    assert!(
        diags_pass2.is_empty(),
        "pass-2 over canonical output must produce zero S007 \
         diagnostics (fixed point reached); got: {:?}",
        diags_pass2,
    );
}
