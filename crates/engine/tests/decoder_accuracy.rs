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
//!   Asserts ≥85% resolution rate at recognition ≥0.85. Always-on
//!   as of issue #133 PR 9 (2026-04-26): the decoder cleared 85.8%
//!   aggregate after REL TO structural repair landed, so this test
//!   is no longer `#[ignore]`d and is the load-bearing SC-004 gate.
//! - `resolution_rate_does_not_regress` — always-on aggregate
//!   accuracy floor. Now pinned at the same 85% as the SC-004
//!   target gate; both fail CI together if accuracy drops. Ratchet
//!   below the new measured rate when subsequent PRs land further
//!   accuracy gains.
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
//! historically and put CI in a known-bad state during the climb
//! from ~53% aggregate to today's 85.8%, or (b) be silently lowered
//! to a passing threshold and lose its meaning. The aggregate-floor
//! split fixed (b); the per-class gate closes the residual hole that
//! an aggregate gate cannot distinguish "every class held its rate"
//! from "class A regressed and class B compensated". With the
//! decoder now clearing 85% (issue #133 PR 9), all three gates are
//! always-on and the SC-004 contract is enforced strictly.
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
/// Current measured rate (2026-04-28, after issue #234 PR-B landed
/// REL TO USA-injection for short first entries, complementing
/// PR-A's 3-char fuzzy trigraph priors):
///
/// | Class             | Resolved | Total | Rate    |
/// |-------------------|----------|-------|---------|
/// | GarbledDelimiter  | 51       | 51    | 100.0%  |
/// | MissingDelimiter  | 17       | 17    | 100.0%  |
/// | Reordering        | 41       | 41    | 100.0%  |
/// | SupersededToken   | 2        | 3     |  66.7%  |
/// | Typo              | 97       | 130   |  74.6%  |
/// | WrongCase         | 18       | 18    | 100.0%  |
/// | **Aggregate**     | **226**  | **260** | **86.9%** |
///
/// Movement from prior pin (issue #233, 2026-04-28):
/// `Typo` 73.8% → 74.6% (+1 fixture: `SA → USA` via the new
/// USA-injection path covering 1-2 char first entries below
/// PR-A's `MIN_FUZZY_LEN = 3` threshold). Aggregate
/// 86.5% → 86.9% (+0.4pp).
///
/// Issue #133 PR 9 landed the literal-shape REL TO structural
/// repair patterns (`REL OT`/`RELT O`/`A US`/`AU,S `); issue #233
/// landed the riskier per-trigraph fuzzy cluster (`USB`, `ASU`)
/// behind corpus-weighted log-priors and the new
/// `try_rel_to_fuzzy_trigraph_candidates` candidate expander.
/// Issue #234 PR-B added the §H.8 p151 USA-first invariant as a
/// complementary structural recovery for short first entries
/// (`SA`, `S`) that fall below the fuzzy matcher's length floor.
///
/// Pinned at the SC-004 target (0.85). Now that the decoder
/// clears 85% (PR 9 lifted it to 85.8%), the regression floor and
/// the SC-004 target gate enforce the same threshold — both fail
/// CI together if accuracy drops below 85%. Prior PRs kept the
/// regression floor several percentage points below the measured
/// rate as headroom against noise; that gap is no longer needed
/// because the target gate (`resolution_rate_at_0_85`) is now
/// load-bearing rather than `#[ignore]`d. A future PR that
/// improves accuracy further can ratchet this back to a noise-
/// tolerant gap below the new measured rate; until then,
/// 0.85-equals-0.85 is the simplest correct policy.
///
/// Remaining accuracy gains (REL TO trigraph fuzzy via #186, SCI
/// compartment fuzzy, SAR program-nickname / identifier-internal
/// typos via #180) are tracked separately.
const AGGREGATE_FLOOR_REGRESSION: f64 = 0.85;

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
/// - **`Typo`** pinned at `0.70` (~5 percentage points below the
///   current 97/130 = 74.6% rate after issue #234 PR-B landed REL
///   TO USA-injection for short first entries). Wide-enough margin
///   to absorb one or two fixtures dropping; a sustained drop trips
///   the gate. Ratchet up as subsequent PRs land SCI compartment
///   fuzzy and the SAR / program-nickname recovery work blocked on
///   #180.
/// - **`MissingDelimiter`** pinned at `1.00`. After #133 PR 5 the
///   class is at 17/17 = 100% — the PR-3 `try_insert_delimiter`
///   helper already produced canonical bytes for every fixture, and
///   PR 5's `HARD_SPLITTER_ABSORPTION_PENALTY` flipped the scoring
///   contest for the 2 SAR-with-trailing-NOFORN cases that were
///   losing to the absorbing parse. Any future fixture that
///   regresses fails the gate.
///
/// Last ratcheted (2026-04-28, issue #234 PR-B) after the REL TO
/// USA-injection path landed: `try_rel_to_usa_injection_candidates`
/// emits a candidate replacing a 1-2 char first entry of a REL TO
/// block with `USA`, anchored on the §H.8 p151 USA-first invariant.
/// One class moved: `Typo` (73.8% → 74.6%, +1 fixture:
/// `SA → USA`); the aggregate moved (86.5% → 86.9%, +1 fixture).
/// The per-class `Typo` floor stays at `0.70` (no ratchet) because
/// the PR-B gain is small enough that the existing margin still
/// absorbs noise without gold-plating.
const PER_CLASS_FLOORS: &[(&str, f64)] = &[
    ("GarbledDelimiter", 1.00),
    ("MissingDelimiter", 1.00),
    ("Reordering", 1.00),
    ("SupersededToken", 0.50),
    ("Typo", 0.70),
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
        as_of: None,
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

/// SC-004 literal target gate — load-bearing as of issue #133 PR 9
/// (2026-04-26). The decoder reached 85.8% aggregate after REL TO
/// structural repair landed as preprocessing in
/// `generate_candidate_bytes`, crossing the 85% threshold.
///
/// This gate is now always-on: a regression below 85% blocks CI.
/// The complementary `resolution_rate_does_not_regress` gate is
/// currently pinned at the same 0.85 floor as this target (no
/// headroom buffer until a future PR ratchets accuracy past 85%);
/// see [`AGGREGATE_FLOOR_REGRESSION`] for the floor policy. Both
/// gates fail CI together if accuracy drops below 85%.
#[test]
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
