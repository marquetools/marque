// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T057 — Decoder accuracy harness (SC-004 gate).
//!
//! Walks `tests/fixtures/mangled/**/*.json`, runs each fixture's
//! `observed` form through `DecoderRecognizer::recognize` (per the
//! T057 spec — recognizer-level, not Engine-level), and pins the
//! decoder's empirical accuracy against the fixture set.
//!
//! ## Three gates, three purposes
//!
//! - `resolution_rate_at_0_85` — the literal SC-004 target.
//!   Asserts ≥85% resolution rate at recognition ≥0.85. Marked
//!   `#[ignore]` until the decoder reaches the target (currently
//!   ~53% aggregate; see per-class table below). PRs that improve
//!   the decoder run `cargo test -p marque-engine \
//!   --features decoder-harness -- --ignored` to verify; once
//!   passing, remove the `#[ignore]` attribute and the test becomes
//!   the load-bearing SC-004 gate.
//! - `resolution_rate_does_not_regress` — always-on aggregate
//!   accuracy floor pinned just below the current measured rate.
//!   Catches decoder regressions before they reach the corpus.
//!   Ratchet the floor up in lockstep with measured accuracy
//!   improvements; never lower it without a planned reason recorded
//!   in the commit message.
//! - `resolution_rate_per_class_does_not_regress` — always-on
//!   *per-class* accuracy floors. Catches a regression in one
//!   mangling class that another class's improvement would
//!   otherwise mask in the aggregate (e.g., Reordering 100%→60%
//!   offset by Typo 20%→40% leaves the aggregate flat). The floors
//!   are pinned per-class against the current measured rates; see
//!   `PER_CLASS_FLOORS` below.
//!
//! ## Why three gates and not one
//!
//! Landing T057 with a single ≥85%-gated test would either (a) fail
//! today and put CI in a known-bad state, or (b) be silently lowered
//! to a passing threshold and lose its meaning. The aggregate-floor
//! split fixed (b) but had a residual hole: an aggregate gate cannot
//! distinguish "every class held its rate" from "class A regressed
//! and class B compensated", and at the current ~53% aggregate the
//! offset window is wide enough for a real regression in a
//! perfect-today class (Reordering, WrongCase, GarbledDelimiter) to
//! sit underneath it. The per-class gate closes that hole. Together
//! the three gates preserve the SC-004 contract intact, give CI a
//! meaningful aggregate floor today, and surface per-class
//! regressions as soon as they happen instead of after the next
//! ratchet.
//!
//! ## Why recognizer-level, not Engine-level
//!
//! The task spec for T057 explicitly says "runs DecoderRecognizer
//! with confidence ≥0.85". Going through `Engine::lint` would also
//! exercise `StrictOrDecoderRecognizer`'s strict-first dispatch
//! (`strict_parse_is_complete` short-circuits the decoder when
//! strict parses cleanly), which would silently classify many
//! mangled fixtures as "strict succeeded" and never measure the
//! decoder. That is an interesting end-to-end metric, but it is not
//! what SC-004 measures — SC-004 is the recognizer's accuracy
//! claim.
//!
//! Per Constitution VII the harness lives in `marque-engine`, not
//! `marque-capco`, because `DecoderRecognizer` and `StrictRecognizer`
//! both live in `marque-engine` (the only crate where the
//! `marque-core` parser surface and the `marque-capco` rule surface
//! converge — see T058 / T059 placement notes in tasks.md).
//!
//! ## What "resolution" means
//!
//! "Resolved" for a fixture is defined as: the recognizer returns
//! `Parsed::Unambiguous(marking)` whose parsed attributes equal
//! (modulo `token_spans`, which are byte-offset metadata that always
//! differ between the mangled `observed` and the canonical
//! `expected`) the attributes parsed from the fixture's `expected`
//! form by the strict recognizer. A `Parsed::Ambiguous` result —
//! including the zero-candidate honesty signal — counts as
//! unresolved regardless of how dense its candidate set is, because
//! SC-004's claim is about *unambiguous* recovery (an operator who
//! has to pick from a candidate list has not been resolved).
//!
//! Marking equality uses [`same_meaning`] which clears
//! `token_spans` before comparison. The strict-attrs-on-canonical
//! → decoder-attrs-on-mangled equality is what SC-004's
//! "resolved to the expected canonical marking" wording asks for —
//! semantic recovery, not source-byte round-trip.
//!
//! ## Failure messages
//!
//! When either gate fails the assertion message includes:
//!   - the per-class resolution rate breakdown
//!   - the aggregate rate
//!   - up to 5 sample unresolved fixtures with `(observed,
//!     expected, decoder verdict, recognition)` tuples
//!
//! so a regression points the reviewer at concrete fixtures, not just
//! "85% gate failed".

#![cfg(feature = "decoder-harness")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use marque_capco::CapcoMarking;
use marque_engine::{DecoderRecognizer, StrictRecognizer};
use marque_ism::IsmAttributes;
use marque_scheme::ambiguity::Parsed;
use marque_scheme::recognizer::{ParseContext, Recognizer};
use serde::Deserialize;

/// Decoder confidence floor (per recognition score) at which a
/// candidate counts as "resolved". Matches the spec's SC-004
/// definition.
const RECOGNITION_FLOOR: f32 = 0.85;

/// Aggregate SC-004 gate. ≥85% resolution across all fixtures.
const AGGREGATE_FLOOR_TARGET: f64 = 0.85;

/// Aggregate regression floor. Pinned just under the current
/// measured rate so noise doesn't trip the gate but a real drop in
/// decoder accuracy does. Ratchet up alongside measured
/// improvements.
///
/// Complementary to [`PER_CLASS_FLOORS`]: this floor catches an
/// across-the-board collapse in decoder accuracy; the per-class
/// floors catch a single-class collapse that another class's
/// improvement would mask here. Both are needed.
///
/// Current measured rate (2026-04-26, branch
/// `fix/issue-133-pr3-missing-delimiter` after issue #133 PR 3
/// landed `try_insert_delimiter` — the helper inserts `//` at
/// category-transition whitespace gaps via classification-boundary
/// (Rule 1) and hard-splitter dissem-long-form (Rule 2) rules):
///
/// | Class             | Resolved | Total | Rate    |
/// |-------------------|----------|-------|---------|
/// | GarbledDelimiter  | 51       | 51    | 100.0%  |
/// | MissingDelimiter  | 15       | 17    |  88.2%  |
/// | Reordering        | 41       | 41    | 100.0%  |
/// | SupersededToken   | 2        | 3     |  66.7%  |
/// | Typo              | 58       | 130   |  44.6%  |
/// | WrongCase         | 18       | 18    | 100.0%  |
/// | **Aggregate**     | **185**  | **260** | **71.2%** |
///
/// 68% gives ~3 percentage-point noise margin against the 71.2%
/// floor. Remaining gap to SC-004's 85% target: (a) the 2
/// unresolved MissingDelimiter fixtures need SCI-starter
/// (`TOP SECRET HCS-P//...`) and SAR-prefix (`TOP SECRET SAR-BP//...`)
/// insertion rules — those need classification-context lookahead
/// and are deferred to PR 4. (b) Typo class still concentrates
/// 3+ char tail-token typos (`UK→TK`, `USAR→SAR`, missing-hyphen
/// SAR forms) outside the PR 2 heuristic's scope — addressed by
/// extending the corpus-confidence work in PR 4.
const AGGREGATE_FLOOR_REGRESSION: f64 = 0.68;

/// Per-class regression floors. Pinned against the current measured
/// rates so a regression in any one mangling class fails CI even
/// when the aggregate floor is satisfied — closes the
/// "Reordering 100%→60% offset by Typo 20%→40%" hole that the
/// aggregate gate cannot detect.
///
/// Floor policy by class:
///
/// - **Currently-perfect classes** (`Reordering`, `WrongCase`,
///   `GarbledDelimiter`) pinned at `1.00`. Any single fixture
///   regressing in those classes fails the gate. The fixture
///   samples (41, 18, 51) are large enough that a `1.00` floor is
///   honest, not noise-tripping.
/// - **`SupersededToken`** pinned at `0.50`. The class has only
///   3 fixtures (one per `SUPERSEDED_TOKEN_MAP` entry), so the
///   only achievable rates are 0.0, 0.333, 0.667, and 1.0. A 0.5
///   floor catches a regression to 1/3 or 0/3 while tolerating the
///   current 2/3 measurement.
/// - **`Typo`** pinned at `0.42` (~3 percentage points below the
///   current 58/130 = 44.6% rate). Wide-enough margin to absorb
///   one or two fixtures dropping; a sustained drop trips the
///   gate. Ratchet up as #133 PR 4 corpus-confidence work lands.
/// - **`MissingDelimiter`** pinned at `0.85` (~3 percentage points
///   below the current 15/17 = 88.2% rate after #133 PR 3 landed
///   `try_insert_delimiter`). The 2 remaining unresolved fixtures
///   need SCI-starter / SAR-prefix / SPECIAL-ACCESS-REQUIRED
///   insertion rules deferred to PR 4.
///
/// Last ratcheted (2026-04-26, branch
/// `fix/issue-133-pr3-missing-delimiter`) to the rates observed
/// after the missing-delimiter helper landed in
/// `marque_engine::decoder::try_insert_delimiter`. Two classes
/// moved from the prior baseline: `MissingDelimiter`
/// (0.0% → 88.2%, +15 fixtures) and the aggregate
/// (65.4% → 71.2%). No other scoring or recognition code
/// changed; the PR 2 heuristic is unchanged from its
/// previous-baseline behavior on the existing corpus.
const PER_CLASS_FLOORS: &[(&str, f64)] = &[
    ("GarbledDelimiter", 1.00),
    ("MissingDelimiter", 0.85),
    ("Reordering", 1.00),
    ("SupersededToken", 0.50),
    ("Typo", 0.42),
    ("WrongCase", 1.00),
];

/// SC-004 also pins the minimum fixture count at ≥200 (so the gate is
/// not vacuously satisfied by deleting fixtures down to a handful that
/// happen to pass). Mirrors the floor enforced at fixture-generation
/// time in `tools/corpus-analysis/analyze.py`.
const MIN_FIXTURE_COUNT: usize = 200;

/// Fixture record schema, mirrored from `tests/fixtures/mangled/README.md`.
/// `source_confidence` is loaded but unused by this harness — the
/// SC-004 gate cares about decoder confidence on `observed`, not the
/// generator's confidence in the `(observed, expected)` pair.
#[derive(Debug, Deserialize)]
struct MangledFixture {
    observed: String,
    expected: String,
    mangling_class: String,
    #[serde(rename = "source_confidence", default)]
    _source_confidence: f64,
}

#[derive(Debug)]
struct FixtureCase {
    fixture: MangledFixture,
    path: PathBuf,
}

#[derive(Debug, Default, Clone)]
struct ClassStats {
    total: usize,
    resolved: usize,
}

#[derive(Debug)]
struct UnresolvedSample {
    observed: String,
    expected: String,
    verdict: String,
    recognition: Option<f32>,
    class: String,
}

#[derive(Debug)]
struct AccuracyReport {
    total: usize,
    resolved: usize,
    aggregate_rate: f64,
    per_class: BTreeMap<String, ClassStats>,
    unresolved_samples: Vec<UnresolvedSample>,
}

impl AccuracyReport {
    fn per_class_summary(&self) -> String {
        self.per_class
            .iter()
            .map(|(class, stats)| {
                let rate = if stats.total == 0 {
                    0.0
                } else {
                    stats.resolved as f64 / stats.total as f64
                };
                format!(
                    "  {class}: {}/{} ({:.1}%)",
                    stats.resolved,
                    stats.total,
                    rate * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn unresolved_summary(&self) -> String {
        if self.unresolved_samples.is_empty() {
            String::from("  (none)")
        } else {
            self.unresolved_samples
                .iter()
                .map(|s| {
                    let recognition = s
                        .recognition
                        .map(|r| format!("{r:.3}"))
                        .unwrap_or_else(|| "—".to_string());
                    format!(
                        "  [{}] {:?} → expected {:?}; verdict={} (recognition={})",
                        s.class, s.observed, s.expected, s.verdict, recognition,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

/// Path to the mangled-fixture tree, anchored to the workspace root
/// so the test runs identically from `cargo test` (workspace cwd) and
/// from `cargo test -p marque-engine` (crate cwd). Walking up from
/// `CARGO_MANIFEST_DIR` (`crates/engine`) by two levels lands at the
/// workspace root.
fn fixtures_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("tests").join("fixtures").join("mangled"))
        .expect("CARGO_MANIFEST_DIR has a workspace-root grandparent")
}

fn load_fixtures() -> Vec<FixtureCase> {
    let root = fixtures_root();
    assert!(
        root.is_dir(),
        "decoder accuracy harness requires {} to exist; \
         regenerate with \
         `python3 tools/corpus-analysis/analyze.py --mode mangled \
          --corpus tests/corpus --output tests/fixtures/mangled/ \
          --min-cases 200 --seed 0`",
        root.display(),
    );

    let mut cases = Vec::new();
    for class_entry in std::fs::read_dir(&root).expect("read mangled fixtures dir") {
        let class_entry = class_entry.expect("read class dir entry");
        let class_path = class_entry.path();
        if !class_path.is_dir() {
            continue;
        }
        for json_entry in std::fs::read_dir(&class_path).expect("read class fixture dir") {
            let json_entry = json_entry.expect("read fixture dir entry");
            let json_path = json_entry.path();
            if json_path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&json_path)
                .unwrap_or_else(|e| panic!("read fixture {}: {e}", json_path.display()));
            let fixture: MangledFixture = serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("parse fixture {}: {e}", json_path.display()));
            cases.push(FixtureCase {
                fixture,
                path: json_path,
            });
        }
    }
    cases
}

/// Deep-scan parse context — no zone, no position, no classification floor.
/// Mirrors `decoder_recovery.rs::deep_cx`.
fn deep_cx() -> ParseContext {
    ParseContext {
        strict_evidence: false,
        zone: None,
        position: None,
        classification_floor: None,
    }
}

/// Parse the canonical `expected` form via the strict recognizer for
/// equality comparison against the decoder's verdict. Returns `None`
/// if strict parsing fails — those fixtures are unmarkable as
/// "expected attrs" and must be flagged separately (see
/// `expected_form_parses_strictly`).
fn parse_expected(strict: &StrictRecognizer, expected: &str) -> Option<CapcoMarking> {
    match strict.recognize(expected.as_bytes(), &deep_cx()) {
        Parsed::Unambiguous(m) => Some(m),
        // The strict recognizer collapses to `Ambiguous { vec![] }`
        // on parse failure; `Unambiguous` is the only form that can
        // function as ground-truth attrs.
        Parsed::Ambiguous { .. } => None,
    }
}

/// Two markings have "the same meaning" iff every parsed-attribute
/// field except `token_spans` is equal. `token_spans` carries byte
/// offsets and literal token bytes from the source the marking was
/// parsed from — those will always differ between the decoder's
/// input (the mangled `observed`) and the strict parser's input (the
/// canonical `expected`), even for fixtures where the decoder
/// recovers the right marking. SC-004 ("resolved to the expected
/// canonical marking") asks whether the *meaning* matches, not
/// whether the source-byte-offset table round-trips. Token spans
/// are diagnostic-presentation metadata, not identity.
fn same_meaning(a: &IsmAttributes, b: &IsmAttributes) -> bool {
    let mut left = a.clone();
    let mut right = b.clone();
    left.token_spans = Box::new([]);
    right.token_spans = Box::new([]);
    left == right
}

/// Project a decoded unambiguous marking into its recognition
/// score. `Parsed::Unambiguous` means the recognizer has already
/// cleared its internal `UNAMBIGUOUS_LOG_MARGIN` threshold
/// (1.6 nats ≈ 5x odds ratio between top and runner-up), but the
/// decoder still exposes the actual softmax-derived recognition
/// score via the marking's provenance. SC-004 must use that real
/// score for its external 0.85 floor — at the unambiguous
/// threshold the softmax is ~0.832 (see
/// `provenance.rs::recognition_softmax_at_unambiguous_threshold`),
/// which is below 0.85, so a hard-coded 1.0 here would silently
/// overstate accuracy by counting near-threshold decodes that the
/// 0.85 floor was specifically designed to reject.
///
/// The decoder always populates provenance on its
/// `Parsed::Unambiguous` outputs (see
/// [`marque_engine::DecoderRecognizer`]'s recognize implementation
/// — every candidate is constructed by canonicalization, which
/// records provenance unconditionally). The `expect` therefore
/// names a real invariant; if it ever fires the decoder broke its
/// contract and the harness is right to surface that loudly rather
/// than silently fall back to a sentinel.
///
/// The Ambiguous arm is treated as unresolved regardless of how
/// dense its candidate set is, because SC-004's claim is about
/// *unambiguous* recovery — a candidate-list disambiguation step
/// is not "resolved" to an operator.
fn unambiguous_recognition_score(m: &CapcoMarking) -> f32 {
    m.1.as_ref()
        .expect(
            "DecoderRecognizer must populate DecoderProvenance on \
             every Parsed::Unambiguous output (Phase D contract)",
        )
        .recognition_score()
}

/// Run the full fixture sweep through the decoder and produce an
/// `AccuracyReport`. Both gates (target and regression) consume
/// this report; computing once amortizes fixture I/O + decoder cost
/// across the two `#[test]`s when both run in the same harness
/// invocation. Each `#[test]` calls this independently so neither
/// depends on test-execution order.
fn run_sweep() -> AccuracyReport {
    let cases = load_fixtures();
    assert!(
        cases.len() >= MIN_FIXTURE_COUNT,
        "decoder accuracy harness requires ≥{MIN_FIXTURE_COUNT} fixtures \
         (SC-004 floor); got {} — regenerate the fixture tree with the \
         `--min-cases` flag",
        cases.len(),
    );

    let decoder = DecoderRecognizer::new();
    let strict = StrictRecognizer::new();
    let mut per_class: BTreeMap<String, ClassStats> = BTreeMap::new();
    let mut total = 0usize;
    let mut resolved = 0usize;
    let mut unresolved_samples: Vec<UnresolvedSample> = Vec::new();

    for case in &cases {
        let class = case.fixture.mangling_class.clone();
        let stats = per_class.entry(class.clone()).or_default();
        stats.total += 1;
        total += 1;

        let expected_marking = match parse_expected(&strict, &case.fixture.expected) {
            Some(m) => m,
            None => {
                if unresolved_samples.len() < 5 {
                    unresolved_samples.push(UnresolvedSample {
                        observed: case.fixture.observed.clone(),
                        expected: case.fixture.expected.clone(),
                        verdict: String::from("(expected unparseable)"),
                        recognition: None,
                        class,
                    });
                }
                continue;
            }
        };

        let (verdict, recognition, is_resolved) = match decoder
            .recognize(case.fixture.observed.as_bytes(), &deep_cx())
        {
            Parsed::Unambiguous(m) => {
                let r = unambiguous_recognition_score(&m);
                let attrs_match = same_meaning(&m.0, &expected_marking.0);
                let resolved = r >= RECOGNITION_FLOOR && attrs_match;
                let verdict = if attrs_match {
                    String::from("(unambiguous, attrs match)")
                } else {
                    format!(
                        "(unambiguous, attrs differ: cls={:?} sci={} sar={} dissem={} rel_to={})",
                        m.0.classification.as_ref().map(|c| c.effective_level()),
                        m.0.sci_markings.len(),
                        m.0.sar_markings.is_some() as u8,
                        m.0.dissem_controls.len(),
                        m.0.rel_to.len(),
                    )
                };
                (verdict, Some(r), resolved)
            }
            Parsed::Ambiguous { candidates } if candidates.is_empty() => {
                (String::from("(zero-candidate)"), None, false)
            }
            Parsed::Ambiguous { candidates } => (
                format!("(ambiguous, {} candidates)", candidates.len()),
                None,
                false,
            ),
        };

        if is_resolved {
            stats.resolved += 1;
            resolved += 1;
        } else if unresolved_samples.len() < 5 {
            unresolved_samples.push(UnresolvedSample {
                observed: case.fixture.observed.clone(),
                expected: case.fixture.expected.clone(),
                verdict,
                recognition,
                class,
            });
        }
    }

    let aggregate_rate = resolved as f64 / total as f64;

    AccuracyReport {
        total,
        resolved,
        aggregate_rate,
        per_class,
        unresolved_samples,
    }
}

/// SC-004 literal target gate. Currently `#[ignore]` because the
/// decoder is at ~53% aggregate vs the 85% target. PRs that improve
/// decoder accuracy run with `-- --ignored` to verify; once passing
/// this becomes the load-bearing SC-004 gate.
///
/// The gate exists today (rather than landing it later when the
/// decoder reaches the target) so:
///
/// - The exact accuracy claim from the spec is encoded in code, not
///   floating in a doc.
/// - PRs that touch the decoder can run this with `-- --ignored` to
///   measure progress against the target without relying on ad-hoc
///   shell pipelines.
/// - When the target is reached, the only change required is
///   removing `#[ignore]` — no test rewrite, no threshold-tuning
///   PR.
#[test]
#[ignore = "SC-004 ≥85% target; current decoder ~53%, see resolution_rate_does_not_regress for the always-on regression gate"]
fn resolution_rate_at_0_85() {
    let report = run_sweep();
    assert!(
        report.aggregate_rate >= AGGREGATE_FLOOR_TARGET,
        "SC-004 target NOT YET MET: decoder resolved {}/{} fixtures \
         ({:.1}%) at recognition ≥{} — below the {:.0}% target.\n\n\
         Per-class breakdown:\n{}\n\n\
         First {} unresolved sample(s):\n{}",
        report.resolved,
        report.total,
        report.aggregate_rate * 100.0,
        RECOGNITION_FLOOR,
        AGGREGATE_FLOOR_TARGET * 100.0,
        report.per_class_summary(),
        report.unresolved_samples.len(),
        report.unresolved_summary(),
    );
}

/// Always-on accuracy regression gate. Catches the decoder getting
/// worse without blocking CI on the SC-004 target gap. Pinned just
/// under the current measured rate; ratchet up alongside measured
/// improvements.
#[test]
fn resolution_rate_does_not_regress() {
    let report = run_sweep();
    assert!(
        report.aggregate_rate >= AGGREGATE_FLOOR_REGRESSION,
        "decoder accuracy REGRESSED below the regression floor: \
         resolved {}/{} fixtures ({:.1}%), floor is {:.0}%. \
         Either restore the lost accuracy or — if the regression \
         is intentional and reviewed — lower \
         AGGREGATE_FLOOR_REGRESSION explicitly.\n\n\
         Per-class breakdown:\n{}\n\n\
         First {} unresolved sample(s):\n{}",
        report.resolved,
        report.total,
        report.aggregate_rate * 100.0,
        AGGREGATE_FLOOR_REGRESSION * 100.0,
        report.per_class_summary(),
        report.unresolved_samples.len(),
        report.unresolved_summary(),
    );
}

/// Always-on per-class accuracy regression gate. Catches a
/// regression in one mangling class that another class's improvement
/// would mask in the aggregate. Each pinned class has its own floor
/// — see [`PER_CLASS_FLOORS`] for the rationale per class.
///
/// The gate also enforces two structural invariants:
///   1. Every class in `PER_CLASS_FLOORS` MUST appear in the
///      observed fixture set. A class going missing (typo dir
///      emptied, fixture generator broken) would otherwise be
///      silently absorbed into the aggregate while leaving the
///      per-class floor vacuously satisfied.
///   2. Every class observed in the fixture set MUST appear in
///      `PER_CLASS_FLOORS`. A new mangling class added to the
///      generator without a corresponding floor entry would be
///      silently uncovered. Forcing the table to be exhaustive
///      makes the addition of a class an explicit code change in
///      this file.
fn resolution_rate_per_class_does_not_regress_inner(report: &AccuracyReport) {
    let pinned: std::collections::BTreeSet<&str> =
        PER_CLASS_FLOORS.iter().map(|(c, _)| *c).collect();
    let observed: std::collections::BTreeSet<&str> =
        report.per_class.keys().map(String::as_str).collect();

    let missing_in_fixtures: Vec<&&str> = pinned.difference(&observed).collect();
    let missing_in_floors: Vec<&&str> = observed.difference(&pinned).collect();

    let mut violations: Vec<String> = Vec::new();

    for (class, floor) in PER_CLASS_FLOORS {
        let Some(stats) = report.per_class.get(*class) else {
            continue; // reported via missing_in_fixtures below
        };
        if stats.total == 0 {
            violations.push(format!(
                "  {class}: 0 fixtures observed (floor {:.0}%); \
                 fixture-tree generator may be broken",
                floor * 100.0,
            ));
            continue;
        }
        let rate = stats.resolved as f64 / stats.total as f64;
        if rate < *floor {
            violations.push(format!(
                "  {class}: {}/{} ({:.1}%) — below floor {:.1}%",
                stats.resolved,
                stats.total,
                rate * 100.0,
                floor * 100.0,
            ));
        }
    }

    let structural_problems = !missing_in_fixtures.is_empty() || !missing_in_floors.is_empty();

    assert!(
        violations.is_empty() && !structural_problems,
        "T057 per-class gate failed (accuracy regression and/or \
         fixture/floor class-list mismatch).\n\n\
         To resolve:\n\
           - If a class regressed below its floor: restore the \
             accuracy, or — if the change is intentional and \
             reviewed — lower the entry in PER_CLASS_FLOORS \
             explicitly with a rationale in the commit message.\n\
           - If a class is missing from fixtures or from \
             PER_CLASS_FLOORS: reconcile the two lists. Adding a \
             new mangling class requires both a generator update \
             and a floor entry in this file.\n\n\
         Accuracy violations:\n{}\n\n\
         Pinned classes missing from fixtures: {:?}\n\
         Observed classes missing from PER_CLASS_FLOORS: {:?}\n\n\
         Per-class breakdown:\n{}",
        if violations.is_empty() {
            String::from("  (none)")
        } else {
            violations.join("\n")
        },
        missing_in_fixtures,
        missing_in_floors,
        report.per_class_summary(),
    );
}

#[test]
fn resolution_rate_per_class_does_not_regress() {
    let report = run_sweep();
    resolution_rate_per_class_does_not_regress_inner(&report);
}

/// Cross-class invariant: every fixture's `mangling_class` field must
/// match the directory it lives under. The accuracy harness's
/// per-class breakdown depends on the field being trustworthy; an
/// out-of-place fixture (e.g., a typo case in `wrong-case/`) would
/// silently distort the breakdown without changing the aggregate.
#[test]
fn fixture_class_matches_directory() {
    let cases = load_fixtures();
    let mut mismatches: Vec<String> = Vec::new();

    for case in &cases {
        let parent_dir = case
            .path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Map directory name (kebab-case) ↔ mangling_class field
        // (PascalCase) per `tests/fixtures/mangled/README.md`.
        let expected_class = match parent_dir {
            "typo" => "Typo",
            "reordering" => "Reordering",
            "missing-delimiter" => "MissingDelimiter",
            "superseded-token" => "SupersededToken",
            "wrong-case" => "WrongCase",
            "garbled-delimiter" => "GarbledDelimiter",
            other => {
                mismatches.push(format!(
                    "  {} lives in unknown class directory `{}/`",
                    case.path.display(),
                    other
                ));
                continue;
            }
        };

        if case.fixture.mangling_class != expected_class {
            mismatches.push(format!(
                "  {}: lives in `{}/` but has `mangling_class = {:?}`; \
                 expected {:?}",
                case.path.display(),
                parent_dir,
                case.fixture.mangling_class,
                expected_class,
            ));
        }
    }

    assert!(
        mismatches.is_empty(),
        "fixture class/directory mismatch in {} fixture(s):\n{}",
        mismatches.len(),
        mismatches.join("\n"),
    );
}

/// Vacuity guard for the resolution gates: every fixture's
/// `expected` form MUST parse via the strict recognizer. If the
/// canonical form fails strict parse, the resolution check is
/// vacuously unsatisfiable for that fixture and the gates become
/// dishonest (an aggregate that omits unparseable canonicals would
/// silently overcount). This test fails loudly with the offending
/// fixtures listed so the corpus generator can be fixed before the
/// gates get stricter.
#[test]
fn expected_form_parses_strictly() {
    let cases = load_fixtures();
    let strict = StrictRecognizer::new();
    let mut unparseable: Vec<String> = Vec::new();

    for case in &cases {
        if parse_expected(&strict, &case.fixture.expected).is_none() {
            unparseable.push(format!(
                "  [{}] expected={:?} (in {})",
                case.fixture.mangling_class,
                case.fixture.expected,
                case.path.display(),
            ));
        }
    }

    assert!(
        unparseable.is_empty(),
        "{} fixture(s) have an `expected` form that fails strict parse — \
         the SC-004 resolution check is vacuously unsatisfiable for \
         these. Either the fixture is genuinely outside the strict \
         recognizer's coverage (regenerate it) or the strict recognizer \
         has lost coverage of a form it used to handle (regression \
         deserving its own fix). First 10:\n{}",
        unparseable.len(),
        unparseable
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
