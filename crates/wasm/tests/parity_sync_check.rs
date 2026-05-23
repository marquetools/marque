// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T088 — sync gate for the WASM parity corpus artifact.
//!
//! Runs natively (host target) and verifies that
//! `tests/parity_corpus.json` is in sync with what `Engine::lint`
//! produces today. The companion test `tests/parity.rs` runs under
//! `wasm-pack test --node` and `include_str!`s the same artifact, so
//! drift between native and WASM only shows up if this sync check
//! still passes but `parity.rs` fails — that's the byte-equal-output
//! divergence we want to surface.
//!
//! ## Regenerating the artifact
//!
//! When a rule, vocabulary, or NDJSON projection change legitimately
//! moves the expected output, regenerate the artifact:
//!
//! ```sh
//! MARQUE_REGEN_PARITY_CORPUS=1 cargo test \
//!     -p marque-wasm \
//!     --test parity_sync_check
//! ```
//!
//! Then commit the updated `tests/parity_corpus.json`. CI runs
//! without the env var and fails on drift, with a clear "regenerate"
//! message naming the env var.

#![cfg(not(target_arch = "wasm32"))]

use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, RuleId};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// JSON projection — kept independent of `marque_wasm`'s internal types so
// that the artifact format does not silently change when the WASM crate's
// internals refactor. The native parity test (`native_parity.rs`) already
// asserts that `marque_wasm::lint_native` produces the same NDJSON shape
// as this projection on the same corpus, so the SC-008 chain is:
//
//   Engine::lint  ==  marque_wasm::lint_native (native)  ==  parity_corpus.json
//                 ==  marque_wasm::lint_native (wasm32, via parity.rs)
//
// Three independent renders of the same shape; any pair diverging breaks
// CI with a localized failure.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct DiagnosticJson<'a> {
    /// T044 PM OD-2: 2-tuple `RuleId` wire shape. Mirrors the CLI and
    /// WASM emitters' `RuleIdJson` for SC-008 byte-identical NDJSON.
    rule: RuleIdJson<'a>,
    severity: &'a str,
    span: SpanJson,
    message: MessageJson<'a>,
    citation: String,
    fix: Option<FixJson<'a>>,
    /// Decoder-recognized canonical form (issue #699). Mirrors the
    /// CLI and WASM emitters' `recognized_canonical` field.
    #[serde(skip_serializing_if = "Option::is_none")]
    recognized_canonical: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct RuleIdJson<'a> {
    scheme: &'a str,
    predicate_id: &'a str,
}

impl<'a> From<&'a RuleId> for RuleIdJson<'a> {
    fn from(r: &'a RuleId) -> Self {
        Self {
            scheme: r.scheme(),
            predicate_id: r.predicate_id(),
        }
    }
}

#[derive(Debug, Serialize)]
struct MessageJson<'a> {
    template: &'a str,
}

#[derive(Debug, Serialize)]
struct SpanJson {
    start: usize,
    end: usize,
}

#[derive(Debug, Serialize)]
struct FixJson<'a> {
    source: &'static str,
    intent_kind: &'static str,
    replacement: Option<&'a str>,
    confidence: f32,
    migration_ref: Option<&'a str>,
}

fn fix_source_str(source: marque_rules::FixSource) -> &'static str {
    match source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
        marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
        marque_rules::FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

fn diagnostic_to_json(d: &Diagnostic<marque_capco::CapcoScheme>) -> DiagnosticJson<'_> {
    // Principle II readout — parity-corpus mirror (issue #699).
    let recognized_canonical = d
        .recognized_canonical
        .as_ref()
        .and_then(|sb| std::str::from_utf8(secrecy::ExposeSecret::expose_secret(sb)).ok());
    DiagnosticJson {
        rule: (&d.rule).into(),
        severity: d.severity.as_str(),
        span: SpanJson {
            start: d.span.start,
            end: d.span.end,
        },
        message: MessageJson {
            template: d.message.template().as_str(),
        },
        citation: d.citation.to_string(),
        fix: match (d.fix.as_ref(), d.text_correction.as_ref()) {
            (Some(f), _) => Some(FixJson {
                source: fix_source_str(f.source),
                intent_kind: match &f.replacement {
                    marque_scheme::ReplacementIntent::FactAdd { .. } => "FactAdd",
                    marque_scheme::ReplacementIntent::FactRemove { .. } => "FactRemove",
                    marque_scheme::ReplacementIntent::Recanonicalize { .. } => "Recanonicalize",
                    _ => "Unknown",
                },
                replacement: None,
                confidence: f.confidence.combined(),
                migration_ref: f.migration_ref,
            }),
            (None, Some(tc)) => Some(FixJson {
                source: fix_source_str(tc.source),
                intent_kind: "TextCorrection",
                replacement: Some(tc.replacement.as_ref()),
                confidence: tc.confidence.combined(),
                migration_ref: tc.migration_ref,
            }),
            (None, None) => None,
        },
        recognized_canonical,
    }
}

fn shared_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        Engine::new(
            Config::default(),
            marque_engine::default_ruleset(),
            marque_engine::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
    })
}

fn engine_lint_to_ndjson(source: &[u8]) -> String {
    let engine = shared_engine();
    let result = engine.lint(source);
    let mut out = String::new();
    for d in &result.diagnostics {
        let json = serde_json::to_string(&diagnostic_to_json(d)).expect("serialize diagnostic");
        out.push_str(&json);
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Artifact schema
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ParityEntry {
    /// Corpus filename (e.g., `banner_abbrev.txt`). Identifies the
    /// fixture so a parity failure points at a single corpus file.
    name: String,
    /// Corpus subdir: `invalid` or `valid`. Prose corpus is intentionally
    /// excluded — it exists at 125KB which would dominate the artifact
    /// size, and `native_parity.rs::lint_parity_prose_fixtures` already
    /// exercises it natively. SC-008 still holds because the algorithmic
    /// equivalence between `Engine::lint` and `marque_wasm::lint_native`
    /// is what the prose native parity catches; the WASM-target parity
    /// gate runs on the smaller corpus and catches WASM-vs-native
    /// compilation divergence, which is orthogonal to corpus size.
    category: String,
    /// Raw input text (UTF-8).
    input: String,
    /// Expected NDJSON output (one diagnostic per line, newline-terminated).
    /// May be empty when the fixture produces no diagnostics.
    expected_lint: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ParityCorpus {
    /// Schema version of this artifact. Bump when the entry shape changes
    /// in a way the WASM test consumer must adapt to.
    schema: String,
    entries: Vec<ParityEntry>,
}

const PARITY_SCHEMA: &str = "marque-wasm-parity-1";
const ARTIFACT_REL: &str = "tests/parity_corpus.json";

fn artifact_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ARTIFACT_REL)
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

fn txt_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .map(|e| e.path())
        .collect();
    files.sort();
    files
}

fn build_corpus() -> ParityCorpus {
    let mut entries = Vec::new();
    for category in ["invalid", "valid"] {
        let dir = corpus_dir().join(category);
        for path in txt_files_in(&dir) {
            let input =
                std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let expected_lint = engine_lint_to_ndjson(&input);
            let input_str = String::from_utf8(input)
                .unwrap_or_else(|_| panic!("non-UTF-8 fixture: {}", path.display()));
            entries.push(ParityEntry {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                category: category.to_owned(),
                input: input_str,
                expected_lint,
            });
        }
    }
    ParityCorpus {
        schema: PARITY_SCHEMA.to_owned(),
        entries,
    }
}

fn read_artifact() -> Option<ParityCorpus> {
    let bytes = std::fs::read(artifact_path()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[test]
fn parity_corpus_artifact_in_sync_with_native_engine() {
    let regen = std::env::var("MARQUE_REGEN_PARITY_CORPUS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let fresh = build_corpus();

    if regen {
        // Pretty-printed for readable git diffs. Size impact is small
        // (~50KB vs ~30KB minified) and the artifact is committed.
        let json = serde_json::to_string_pretty(&fresh).expect("serialize parity corpus");
        std::fs::write(artifact_path(), json).expect("write artifact");
        eprintln!(
            "[parity_sync_check] regenerated {} ({} entries)",
            ARTIFACT_REL,
            fresh.entries.len()
        );
        return;
    }

    let on_disk = read_artifact().unwrap_or_else(|| {
        panic!(
            "missing or corrupt {}; regenerate with \
             `MARQUE_REGEN_PARITY_CORPUS=1 cargo test -p marque-wasm \
             --test parity_sync_check`",
            ARTIFACT_REL
        )
    });

    assert_eq!(
        on_disk.schema, fresh.schema,
        "parity corpus schema drift: on-disk {:?} != current {:?}; \
         regenerate the artifact and bump consumer if the schema field changed.",
        on_disk.schema, fresh.schema
    );

    assert_eq!(
        on_disk.entries.len(),
        fresh.entries.len(),
        "parity corpus entry count drift (on-disk {}, current {}); \
         regenerate with \
         `MARQUE_REGEN_PARITY_CORPUS=1 cargo test -p marque-wasm \
         --test parity_sync_check`",
        on_disk.entries.len(),
        fresh.entries.len(),
    );

    // Compare entry-by-entry so a drift error names the offending fixture
    // instead of dumping a diff of the whole artifact.
    for (a, b) in on_disk.entries.iter().zip(fresh.entries.iter()) {
        assert_eq!(
            a.name, b.name,
            "parity corpus name order drift; regenerate the artifact"
        );
        assert_eq!(
            a.category, b.category,
            "parity corpus category drift on {}; regenerate the artifact",
            a.name
        );
        assert_eq!(
            a.input, b.input,
            "parity corpus input drift on {}; corpus fixture changed — \
             regenerate with `MARQUE_REGEN_PARITY_CORPUS=1 cargo test \
             -p marque-wasm --test parity_sync_check`",
            a.name
        );
        assert_eq!(
            a.expected_lint, b.expected_lint,
            "parity corpus expected_lint drift on {} — the native \
             engine's lint output for this fixture has changed since \
             the artifact was last regenerated. If the engine change \
             is intentional, regenerate with \
             `MARQUE_REGEN_PARITY_CORPUS=1 cargo test -p marque-wasm \
             --test parity_sync_check` and commit the updated artifact. \
             If unintentional, the engine has regressed.",
            a.name
        );
    }
}
