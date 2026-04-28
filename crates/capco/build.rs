// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! marque-capco build script (T004a + T042).
//!
//! Reads `corpus/priors.json` at compile time and emits `&'static` Rust
//! tables into `OUT_DIR/priors.rs`, included by `src/priors.rs`. No
//! runtime JSON parsing; no runtime `serde_json` dependency — the
//! decoder reads plain const tables (Constitution II, SC-008).
//!
//! Fails closed on missing or malformed input. A clean checkout MUST
//! have a valid `corpus/priors.json` checked in; regenerate via:
//!
//! ```text
//! python3 tools/corpus-analysis/analyze.py \
//!     --mode priors \
//!     --corpus <path-to-corpus> \
//!     --output crates/capco/corpus/priors.json
//! ```
//!
//! See `crates/capco/corpus/README.md` for the JSON schema contract.

use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

const SUPPORTED_PRIORS_SCHEMA_VERSION: &str = "marque-priors-2";

fn main() {
    let priors_json_path = Path::new("corpus").join("priors.json");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set by cargo");
    let out_path = Path::new(&out_dir).join("priors.rs");

    println!("cargo:rerun-if-changed=corpus/priors.json");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let raw = match fs::read_to_string(&priors_json_path) {
        Ok(s) => s,
        Err(err) => {
            panic!(
                "marque-capco build failed: could not read {}: {}.\n\
                 Regenerate via:\n  \
                 python3 tools/corpus-analysis/analyze.py --mode priors \\\n    \
                 --corpus <corpus-dir> --output crates/capco/corpus/priors.json\n\
                 See crates/capco/corpus/README.md.",
                priors_json_path.display(),
                err,
            );
        }
    };

    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|err| {
        panic!(
            "marque-capco build failed: {} is not valid JSON: {}",
            priors_json_path.display(),
            err,
        );
    });

    let emitted = emit_priors(&parsed, priors_json_path.as_path());

    let mut f = fs::File::create(&out_path)
        .unwrap_or_else(|err| panic!("failed to create {}: {}", out_path.display(), err));
    f.write_all(emitted.as_bytes())
        .unwrap_or_else(|err| panic!("failed to write {}: {}", out_path.display(), err));
}

fn emit_priors(parsed: &serde_json::Value, priors_path: &Path) -> String {
    let schema_version = require_str(parsed, "schema_version", priors_path);
    if schema_version != SUPPORTED_PRIORS_SCHEMA_VERSION {
        panic!(
            "marque-capco build failed: {} has schema_version {:?}, but this \
             build only supports {:?}. Regenerate priors.json with a matching \
             tool version.",
            priors_path.display(),
            schema_version,
            SUPPORTED_PRIORS_SCHEMA_VERSION,
        );
    }
    let corpus_fingerprint = require_str(parsed, "corpus_fingerprint", priors_path);
    validate_corpus_fingerprint(corpus_fingerprint, priors_path);
    // `generated_at` is optional per `crates/capco/corpus/README.md`
    // ("Any field beyond this set is ignored"). Default to empty string
    // so the emitted const is always present for downstream consumers.
    let generated_at = parsed
        .get("generated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let token_base_rates = require_object(parsed, "token_base_rates", priors_path);
    let template_base_rates = require_object(parsed, "template_base_rates", priors_path);
    // Trigraph base rates land in marque-priors-2 (issue #233). Same
    // shape as token_base_rates: ``{token, count, log_prior}``. The
    // decoder consumes this at score time so REL TO fuzzy candidates
    // are weighted by real-world country-code frequency rather than
    // collapsing to ``MISSING_TOKEN_LOG_PRIOR`` for everything.
    let trigraph_base_rates = require_object(parsed, "trigraph_base_rates", priors_path);
    let strict_context = require_object(parsed, "strict_context_priors", priors_path);

    let confidential_floor = require_probability(
        strict_context,
        "confidential_floor",
        "strict_context_priors",
        priors_path,
    );
    let secret_floor = require_probability(
        strict_context,
        "secret_floor",
        "strict_context_priors",
        priors_path,
    );
    let top_secret_floor = require_probability(
        strict_context,
        "top_secret_floor",
        "strict_context_priors",
        priors_path,
    );

    let mut token_rows: Vec<(String, u64, f64)> = token_base_rates
        .iter()
        .map(|(name, record)| {
            let count = require_u64(record, "count", name, priors_path);
            let log_prior = require_log_prior(record, "log_prior", name, priors_path);
            (name.clone(), count, log_prior)
        })
        .collect();
    // Fail the build closed if the generator emitted an empty token
    // table. An empty TOKEN_BASE_RATES means the decoder has no corpus
    // evidence for any canonical token — a generator regression, not a
    // valid state. The #[cfg(test)] `tables_are_non_empty` check
    // asserts the same invariant, but only fires when tests run; this
    // guard fails `cargo build` even when tests are skipped.
    if token_rows.is_empty() {
        panic!(
            "[marque-capco build.rs] priors.json token_base_rates is empty. \
             {} must contain at least one token.",
            priors_path.display()
        );
    }
    // Sort so the emitted table is stable across generator runs with the
    // same input — keeps `cargo build` incremental and the generated
    // file diff-clean under VCS snapshots.
    token_rows.sort_by(|a, b| a.0.cmp(&b.0));

    let mut template_rows: Vec<(String, u64, f64)> = template_base_rates
        .iter()
        .map(|(name, record)| {
            let count = require_u64(record, "count", name, priors_path);
            let log_prior = require_log_prior(record, "log_prior", name, priors_path);
            (name.clone(), count, log_prior)
        })
        .collect();
    // Same fails-closed guard on templates: empty TEMPLATE_BASE_RATES
    // means the decoder has no grammar-shape priors to score K=8
    // candidates against.
    if template_rows.is_empty() {
        panic!(
            "[marque-capco build.rs] priors.json template_base_rates is empty. \
             {} must contain at least one template.",
            priors_path.display()
        );
    }
    template_rows.sort_by(|a, b| a.0.cmp(&b.0));

    let mut trigraph_rows: Vec<(String, u64, f64)> = trigraph_base_rates
        .iter()
        .map(|(name, record)| {
            let count = require_u64(record, "count", name, priors_path);
            let log_prior = require_log_prior(record, "log_prior", name, priors_path);
            (name.clone(), count, log_prior)
        })
        .collect();
    // Empty TRIGRAPH_BASE_RATES would defeat issue #233's purpose:
    // the decoder would fall back to MISSING_TOKEN_LOG_PRIOR for every
    // REL TO country code and the fuzzy-disambiguation guarantees
    // (USB→USA, ASU→AUS) would not hold. Mirror the token / template
    // fails-closed guards.
    if trigraph_rows.is_empty() {
        panic!(
            "[marque-capco build.rs] priors.json trigraph_base_rates is empty. \
             {} must contain at least one trigraph (issue #233).",
            priors_path.display()
        );
    }
    trigraph_rows.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    out.push_str("// AUTO-GENERATED by crates/capco/build.rs. Do not edit by hand.\n");
    out.push_str("// Source: crates/capco/corpus/priors.json\n\n");

    out.push_str(&format!(
        "pub const SCHEMA_VERSION: &str = {:?};\n",
        schema_version
    ));
    out.push_str(&format!(
        "pub const CORPUS_FINGERPRINT: &str = {:?};\n",
        corpus_fingerprint
    ));
    out.push_str(&format!(
        "pub const GENERATED_AT: &str = {:?};\n\n",
        generated_at
    ));

    out.push_str("pub const TOKEN_BASE_RATES: &[TokenPrior] = &[\n");
    for (name, count, log_prior) in &token_rows {
        // f64 → f32 downcast matches foundational-plan line 739-757.
        // Decoder internals may compute in f64 and downcast at the
        // `Confidence` boundary; this table is the `&'static` boundary.
        let log_prior_f32 = downcast_log_prior(*log_prior, name, priors_path);
        out.push_str(&format!(
            "    TokenPrior {{ token: {:?}, count: {}, log_prior: {:?}_f32 }},\n",
            name, count, log_prior_f32,
        ));
    }
    out.push_str("];\n\n");

    out.push_str("pub const TEMPLATE_BASE_RATES: &[TemplatePrior] = &[\n");
    for (name, count, log_prior) in &template_rows {
        let log_prior_f32 = downcast_log_prior(*log_prior, name, priors_path);
        out.push_str(&format!(
            "    TemplatePrior {{ name: {:?}, count: {}, log_prior: {:?}_f32 }},\n",
            name, count, log_prior_f32,
        ));
    }
    out.push_str("];\n\n");

    // Per-trigraph base rates and log-priors (issue #233). Reuses the
    // ``TokenPrior`` struct because the on-disk shape is identical:
    // an entry is just ``{ name, count, log_prior }``. Naming the
    // table ``TRIGRAPH_BASE_RATES`` mirrors ``TOKEN_BASE_RATES`` /
    // ``TEMPLATE_BASE_RATES`` so the priors module reads as a flat
    // family of "<thing>_base_rates" tables.
    out.push_str("pub const TRIGRAPH_BASE_RATES: &[TokenPrior] = &[\n");
    for (name, count, log_prior) in &trigraph_rows {
        let log_prior_f32 = downcast_log_prior(*log_prior, name, priors_path);
        out.push_str(&format!(
            "    TokenPrior {{ token: {:?}, count: {}, log_prior: {:?}_f32 }},\n",
            name, count, log_prior_f32,
        ));
    }
    out.push_str("];\n\n");

    out.push_str(&format!(
        "pub const STRICT_CONTEXT_PRIORS: StrictContextPriors = StrictContextPriors {{\n\
             \x20   confidential_floor: {:?}_f32,\n\
             \x20   secret_floor: {:?}_f32,\n\
             \x20   top_secret_floor: {:?}_f32,\n\
         }};\n",
        confidential_floor, secret_floor, top_secret_floor,
    ));

    out
}

fn require_str<'a>(value: &'a serde_json::Value, key: &str, path: &Path) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or_else(|| {
        panic!(
            "marque-capco build failed: {} missing required string field {:?}",
            path.display(),
            key,
        )
    })
}

fn require_object<'a>(
    value: &'a serde_json::Value,
    key: &str,
    path: &Path,
) -> &'a serde_json::Map<String, serde_json::Value> {
    value
        .get(key)
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| {
            panic!(
                "marque-capco build failed: {} missing required object field {:?}",
                path.display(),
                key,
            )
        })
}

fn require_u64(record: &serde_json::Value, key: &str, parent: &str, path: &Path) -> u64 {
    record.get(key).and_then(|v| v.as_u64()).unwrap_or_else(|| {
        panic!(
            "marque-capco build failed: {} -> {} missing or non-integer {:?}",
            path.display(),
            parent,
            key,
        )
    })
}

fn require_f64(record: &serde_json::Value, key: &str, parent: &str, path: &Path) -> f64 {
    record.get(key).and_then(|v| v.as_f64()).unwrap_or_else(|| {
        panic!(
            "marque-capco build failed: {} -> {} missing or non-numeric {:?}",
            path.display(),
            parent,
            key,
        )
    })
}

/// Validate a `strict_context_priors` floor probability.
///
/// Accepts `(0.0, 1.0]`. Rejects `0.0` specifically: a `0.0` floor
/// is the algebraic identity for the strict-context-classification
/// feature contribution (the feature multiplies a posterior weight
/// against the floor; a `0.0` floor never rejects any candidate,
/// silently making the rule a no-op). Per Phase 4 review M8: a
/// regenerator emitting `0.0` accidentally — e.g., a corpus bucket
/// with no observed markings at the relevant classification level —
/// would defeat FR-011 semantics with no diagnostic at build time.
/// Operators or tools that want "very permissive" should write a
/// finite small positive (e.g., `0.01`), which has the same
/// practical effect without the no-op footgun.
fn require_probability(
    record: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    parent: &str,
    path: &Path,
) -> f32 {
    let v = record.get(key).and_then(|v| v.as_f64()).unwrap_or_else(|| {
        panic!(
            "marque-capco build failed: {} -> {} missing or non-numeric {:?}",
            path.display(),
            parent,
            key,
        )
    });
    if !v.is_finite() || v <= 0.0 || v > 1.0 {
        panic!(
            "marque-capco build failed: {} -> {}.{} = {} is not a valid \
             strict_context_priors floor in (0.0, 1.0]. `0.0` is rejected \
             because it makes the strict-context rule a silent no-op; \
             write a finite small positive (e.g., 0.01) for a permissive floor. \
             Regenerate priors.json with a fixed generator.",
            path.display(),
            parent,
            key,
            v,
        );
    }
    v as f32
}

fn require_log_prior(record: &serde_json::Value, key: &str, parent: &str, path: &Path) -> f64 {
    let v = require_f64(record, key, parent, path);
    if !v.is_finite() || v > 0.0 {
        panic!(
            "marque-capco build failed: {} -> {}.{} = {} is not a valid log-probability \
             (must be finite and ≤ 0.0). Regenerate priors.json with a fixed generator.",
            path.display(),
            parent,
            key,
            v,
        );
    }
    v
}

/// Validate that `corpus_fingerprint` matches the `sha512:<128-hex-digits>`
/// form documented in `crates/capco/corpus/README.md`. A malformed fingerprint
/// is a generator regression — the README promises a stable shape so downstream
/// analysts can correlate a priors file back to a specific corpus snapshot.
fn validate_corpus_fingerprint(fingerprint: &str, path: &Path) {
    const PREFIX: &str = "sha512:";
    const HEX_LEN: usize = 128;
    let hex = fingerprint.strip_prefix(PREFIX).unwrap_or_else(|| {
        panic!(
            "marque-capco build failed: {} -> corpus_fingerprint {:?} must start \
             with {:?}. See crates/capco/corpus/README.md.",
            path.display(),
            fingerprint,
            PREFIX,
        )
    });
    if hex.len() != HEX_LEN || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        panic!(
            "marque-capco build failed: {} -> corpus_fingerprint hex body must be \
             exactly {} ASCII hex digits, got {} chars in {:?}",
            path.display(),
            HEX_LEN,
            hex.len(),
            fingerprint,
        );
    }
}

/// Downcast a validated `f64` log-prior to `f32`, panicking if the narrowed
/// value becomes non-finite.
///
/// `require_log_prior` already ruled out `+inf` / `NaN` / positive `f64`
/// inputs on the way in, so the only remaining failure mode here is an
/// `f64` whose magnitude exceeds `f32`'s range and overflows to `-inf`
/// on narrowing. This is the condition the check actually detects —
/// it does NOT detect small-magnitude precision loss (subnormal
/// flush-to-zero, ulp-level rounding), which is accepted as the cost
/// of keeping the baked tables `f32` per foundational-plan
/// line 739-757 (the `Confidence` boundary is `f32`).
///
/// If a generator regression produces log-priors large enough in
/// magnitude to overflow `f32`, the priors JSON needs to be
/// regenerated; if the decoder later needs sub-ulp precision on
/// these tables, the baked type is the thing to widen, not this
/// narrowing.
fn downcast_log_prior(value: f64, name: &str, path: &Path) -> f32 {
    let narrowed = value as f32;
    if !narrowed.is_finite() {
        panic!(
            "marque-capco build failed: {} -> {:?}.log_prior = {} became non-finite on \
             f64→f32 downcast (narrowed to {}). Regenerate priors.json or widen the \
             baked-table type.",
            path.display(),
            name,
            value,
            narrowed,
        );
    }
    narrowed
}
