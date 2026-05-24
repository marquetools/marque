# WASM bundle composition analysis — 2026-05-16

Scope: what is **packaged** into `marque-wasm` (`crates/wasm/`) and what could
be excluded or replaced. Per task constraints: no `wasm-opt` / LTO / strip /
codegen-units recommendations; no `aho-corasick → daachorse` swap; no API
surface changes.

Methodology: read-only inspection of `crates/wasm/Cargo.toml`,
`crates/wasm/src/lib.rs`, workspace `Cargo.toml`, each WASM-safe crate's
`Cargo.toml`, the `marque-ism` and `marque-capco` build outputs under
`target/debug/build/marque-ism-*/out/` and `target/debug/build/marque-capco-*/out/`,
plus `cargo tree` against the wasm32 target.

## 1. Dep tree summary

Command executed:

```
cargo tree -p marque-wasm --target wasm32-unknown-unknown \
    --no-default-features --features web --edges normal
```

Top-level runtime deps (per `crates/wasm/Cargo.toml:135-166`) and rough
weights:

| Dep | Used by | Weight | Notes |
|-----|---------|--------|-------|
| `wasm-bindgen` 0.2.120 | required | medium | JS glue is sized by `#[wasm_bindgen]` export count (10 exports — see §3 row 7) |
| `console_error_panic_hook` 0.1.7 | `feature web` | small | Optional, gated by `web` |
| `talc` 5.0.3 + `allocator-api2` + `lock_api` | global allocator | small-medium | `lock_api` only on `multi-threading`/`talc_debug` (`crates/wasm/src/lib.rs:164-168`) |
| `serde` 1.0.228 + `serde_core` | required | medium | Pulled in by every JSON serializer in `lib.rs` |
| `serde_json` 1.0.149 (+ `zmij`, `itoa`, `memchr`) | required | medium | `zmij` is a double→string algo crate pulled in by `serde_json` runtime |
| `humantime` 2.3.0 | timestamp format | small | RFC3339 formatting at `lib.rs:495` |
| `marque-engine` 0.2.1 | required | large | Pulls in decoder (`crates/engine/src/decoder.rs`, 7089 LOC), recognizer, scheduler, batch (gated on `batch` feature, default-on in engine) |
| `marque-capco` 0.2.1 | required | large | Rules, CapcoScheme, lattice types, priors, vocabulary tables |
| `marque-core` 0.2.1 | required | medium | Scanner + parser; `memchr` + `aho-corasick` |
| `marque-ism` 0.2.1 | required | massive | Build-time-generated CVE/vocab tables — see §2 |
| `marque-rules` 0.2.1 | required | small | Trait defs only |
| `marque-scheme` 0.2.1 | required | small | Trait surface + lattice constructors (`crates/scheme/src/builtins.rs`, 1392 LOC) |
| `marque-config` 0.2.1 | required | medium | Pulls `toml` 1.1.2 + `toml_parser` + `winnow` 1.0.1 + `serde_spanned` + `toml_datetime` + `toml_writer` |
| `aho-corasick` 1.1.4 | parser + corrections | medium | Already in `Cargo.toml` reservation note (kept) |
| `memchr` 2.8.0 | SIMD | small | |
| `jiff` 0.2.24 (`features = ["std"]`, no tzdb) | `marque-ism` via `IsmDate` | medium | See §3 row 9 — used only by `crates/ism/src/date.rs` (2289 LOC), unreachable from WASM hot path |
| `smol_str` 0.3.2, `smallvec` 1.15.1, `thiserror` 2 | required | small | Used pervasively |
| `web-time` 1.1.0 (+ `js-sys`, `futures-util`, `futures-core`, `futures-task`, `pin-project-lite`, `slab`) | `marque-engine` deadline polyfill | medium | `futures-util`/`slab` pulled by `js-sys` chain |
| `tracing` 0.1.44 (`tracing-core`, `tracing-attributes`, `pin-project-lite`, `once_cell`) | `marque-engine`, `marque-capco` | small-medium | No subscriber pulled (good); call-site overhead remains |

## 2. Static-data inventory (compile-time generated, ship in binary)

From `target/debug/build/marque-ism-*/out/`:

| File | Bytes | Rows | What it contains |
|------|-------|------|------------------|
| `vocabulary.rs` | **104,037 B** | 347 `TokenMetadataEntry` rows + 27 `CveFileMetadata` `pub static` records | Per-token `description` (full English prose, e.g. the 200+ char 25X1 reveal-source text), `cve_file.urn`, `cve_file.title`, `cve_file.source`, `cve_file.poc_name`, `cve_file.poc_email`, `cve_file.owner_producer`, `spec_version`, `des_version`, `schema_version` |
| `values.rs` | 43,864 B | 340 `TRIGRAPHS` + 24 `TETRAGRAPH_MEMBERS` + 46 `ALL_CVE_TOKENS` + closed enums | CVE enums, trigraph list, tetragraph members (`&[(&str, &[&str])]`), provenance |
| `migrations.rs` | 4,428 B | ~30 entries | Deprecated → replacement mappings |
| `validators.rs` | 2,053 B | small | Schematron-derived predicates |
| **total ISM-gen** | **154,382 B** of generated Rust source | | Largest single contributor by far |

From `target/debug/build/marque-capco-*/out/priors.rs`:

| File | Bytes | Rows | What it contains |
|------|-------|------|------------------|
| `priors.rs` | 18,107 B | 236 `TokenPrior` + ~5 `TemplatePrior` (241 entries combined) | Per-token marking/prose log-priors, all `f32` |

In-tree compile-time tables in `crates/capco/src/`:

| File | LOC | What |
|------|-----|------|
| `vocab.rs` | 302 | `FVEY`, `ACGU`, etc. ground-truth country-code arrays |
| `vocabulary.rs` | 1773 | `impl Vocabulary<CapcoScheme>` — projects `TOKEN_METADATA` into `Authority` / `OwnerProducer` / `PointOfContact` structs; **§3 row 1** |
| `priors.rs` | 558 | Re-exports + accessor wrappers over the generated `priors.rs` |

`crates/scheme/src/builtins.rs` (1392 LOC) ships `OrdMax`, `OrdMin`, `FlatSet`,
`IntersectSet`, `SupersessionSet`, `ModeSet`, `MaxDate`, `OptionalSingleton`,
`Product`. These are generic and instantiated only at the types `CapcoScheme`
actually uses — DCE should handle unused monomorphizations cleanly, but `dyn`
dispatch in `PageRewrite` / `Lattice` could keep some alive.

## 3. Findings

| # | Location | Observation | Est. size | Risk | Recommendation |
|---|----------|-------------|----------|------|----------------|
| 1 | `crates/ism/build.rs:90` → `OUT_DIR/vocabulary.rs` (104 KB src; ~30-60 KB compressed in wasm rodata after deduplication of repeated authority strings) | `TOKEN_METADATA`'s `description` fields contain full English prose (avg ~100-300 bytes/token across 347 tokens). The `impl Vocabulary<CapcoScheme>` (`crates/capco/src/vocabulary.rs:984-996`) exposes `authority()`, `owner_producer()`, `point_of_contact()`, `deprecation()` — **none of which are called from production code in `crates/capco/src/`, `crates/engine/src/`, `crates/core/src/`, or `crates/wasm/src/`** (`grep` for `scheme.authority(`, `scheme.owner_producer(`, etc. returns 0 hits in `src/` trees; all 5 hits live in `tests/`). The hot-path consumers that *do* fire (`forms()`, `is_fdr_dissem()`) only need the `FormSet` and the `is_fdr_dissem` bit. | **large (~50-80 KB rodata)** | low-medium — Vocabulary surface is preserved for scheme adoption (`scheme/tests/adoption_readiness.rs`), but the WASM artifact does not need the data | Gate the per-token `description`, `cve_file.source`, `poc_name`, `poc_email` strings behind a `vocab-metadata` Cargo feature in `marque-ism`. WASM declines the feature; CLI/server keep it for audit-record narration. Keep `value`, `urn`, `owner_producer`, `spec_version`, `des_version`, `schema_version` (small) unconditional. |
| 2 | `crates/engine/src/decoder.rs` (7089 LOC) + `crates/capco/build.rs` priors (18 KB generated) | Decoder is reachable from WASM (`Engine::new` installs `StrictOrDecoderRecognizer` per `crates/engine/src/engine.rs:306-311`; WASM never calls `with_recognizer`, so decoder + priors are live). The WASM doc-comment at `crates/wasm/src/lib.rs:31-46` explicitly states the decoder fallback runs in WASM. Cannot remove without changing semantics (decoder default-on landed in #259). | not applicable | high if removed | **No change.** Confirmed decoder is genuinely used. |
| 3 | `crates/scheme/src/builtins.rs` (1392 LOC) | Eight lattice constructors generic over `T`. `CapcoScheme` instantiates a closed set (`SciSet`, `SarSet`, `FgiSet`, classification `OrdMax`, supersession `SupersessionSet`, etc.). Unused monomorphizations are DCE'd, but each generic *method* — `extend_counts`, `from_iter_sorted`, `try_present` — gets a copy per instantiation. Some are reachable only through `impl Lattice for CapcoScheme`'s `dyn` projection. | medium (10-25 KB; speculative — needs `twiggy`) | low | Audit which `T` instantiations actually fire on the WASM hot path. If a generic method is only used in tests, mark it `#[cfg(test)]` or move it behind a `lattice-extras` feature. Cheapest experiment: build with `RUSTFLAGS="--emit=llvm-ir"` and grep the monomorphizations. |
| 4 | `crates/config/Cargo.toml:30` — `toml` 1.1.2 unconditional | WASM config arrives as a JSON string from JS (`crates/wasm/src/lib.rs:602-612`), not TOML. `marque-config` pulls `toml`, `toml_parser`, `winnow`, `toml_datetime`, `toml_writer`, `serde_spanned` — none of which are reachable from `Config::default()` + `set_confidence_threshold()` + `corrections` assignment in `build_engine_config` (`crates/wasm/src/lib.rs:622-634`). | medium (~30-60 KB combined for the toml chain) | low-medium — requires feature-gating `marque-config`'s file-loading code | Add `toml-loader` feature to `marque-config`, gated default-on for CLI/server, off for WASM. Move all `toml::from_str` call sites under that gate. WASM dep on `marque-config` becomes `default-features = false`. |
| 5 | `crates/wasm/Cargo.toml:163` — `serde_json = { features = ["raw_value"] }` + `zmij` 1.0.21 transitive | `RawValue` is used (10 sites in `lib.rs`) to avoid re-parsing pre-serialized diagnostic JSON. `zmij` is `serde_json`'s default `dtoa` impl. Cannot drop `raw_value`; it is load-bearing for `lint_batch` correctness. | small | none | **No change.** `raw_value` is genuinely needed. |
| 6 | `crates/ism/Cargo.toml:44` — `jiff` 0.2.24 (default-features-off, std-only) | `jiff` is only used by `crates/ism/src/date.rs` (2289 LOC, `IsmDate` precision tiers). `IsmAttributes.declassify_on` is computed by the scanner but **never referenced from `crates/capco/src/` or `crates/core/src/` production code** (`grep declassify_on` returns 0 src/ hits in those trees). | medium (~80-150 KB; `jiff` is non-trivial even without tzdb) | medium — `IsmDate` is referenced from `IsmAttributes`, so it's reachable through the parser; need to confirm `declassify_on` parsing actually runs in WASM | Confirm declassify-on parsing is on the WASM hot path via a `twiggy paths` from `lint_native`. If unused, gate `IsmDate` / `date.rs` behind a `dates` feature in `marque-ism`. Even if used, the `jiff::civil` API surface in use (`Date::new`, `Date::days_in_month`) is small enough that a hand-rolled leap-year + ordinal-day helper would be drop-in. |
| 7 | `crates/wasm/src/lib.rs` `#[wasm_bindgen]` exports | 10 exported functions: `configure_native`, `lint_native`, `fix_native`, `lint_batch_native`, `init`, `configure`, `lint`, `fix`, `lint_batch`, `compute_banner` + `compute_banner_native` + `generate_cab` + `generate_cab_native` (12 total, lines 810-1562). Each pulls in a wasm-bindgen shim and (where it takes/returns strings) UTF-8 conversion paths. All parameters are `&str` / `Option<String>` / `f32` — none pass complex structs. | small | none | **No change.** Surface is already minimal; do not add more `#[wasm_bindgen]` exports. |
| 8 | `crates/wasm/src/lib.rs:307-509` — JSON serializer functions duplicated from `marque/src/render.rs` | Necessary for SC-008 parity (CLI and WASM emit byte-identical NDJSON). The duplication is intentional and tested by `tests/native_parity.rs`. Each `match` arm in `fact_ref_to_json`, `scope_str`, `recanon_scope_str`, `fix_source_str`, `proposal_to_json` adds ~50-200 bytes. | small-medium | high if changed (parity) | **No change.** Parity test is load-bearing. |
| 9 | `crates/engine/Cargo.toml:86` — `default = ["batch"]` pulls `futures`, `recoco-utils`, `tokio` | WASM deps tree shows `futures-core` reaching the binary via `web-time → js-sys → futures-util` — that's the `js-sys` chain, not the `batch` feature. Verified: `marque-wasm` pins `marque-engine = { default-features = false }` (`crates/wasm/Cargo.toml:158`), so `batch` (tokio, recoco-utils) is correctly excluded. | none (already excluded) | none | **No change.** Already correctly configured. |
| 10 | `crates/wasm/Cargo.toml:137` — `humantime` only for `format_rfc3339(timestamp)` at `lib.rs:495` | The full crate is pulled for one function. `humantime::format_rfc3339` brings in date arithmetic shared with `humantime::Duration` parsing. | small | low | Replace with a 20-line hand-rolled RFC3339 formatter for `SystemTime`. Direct: divide millis since epoch by 86400000, compute Y/M/D via Zeller-like arithmetic, format `%04d-%02d-%02dT%02d:%02d:%02d.%03dZ`. Possibly merge with the `current_year()` logic at `lib.rs:246-261`. |

## 4. Top 3 prioritized wins

### Win 1 — Gate token-metadata strings behind a Cargo feature (Finding 1)

Highest expected impact, lowest risk. `vocabulary.rs` is the single largest
build-time-generated artifact (104 KB Rust source → ~50-80 KB rodata after
duplicate-string deduplication). The metadata is unused in production rules
today; gating the heavy strings behind a `vocab-metadata` feature (off in
WASM, on in CLI/server) preserves the trait surface for scheme adoption and
the audit-narration use case without shipping the data into the browser.

Source touchpoints: `crates/ism/build.rs:90` (generate_vocabulary),
`crates/capco/src/vocabulary.rs:166-180` (entry_for) and `:236-358`
(build_authority / build_owner_producer / build_capco_point_of_contact),
`crates/ism/Cargo.toml`, `crates/capco/Cargo.toml`, `crates/wasm/Cargo.toml`.

### Win 2 — Feature-gate `toml` out of WASM (Finding 4)

Medium impact, low-medium risk. WASM receives JSON-only config; the toml chain
(`toml`, `toml_parser`, `winnow`, `toml_datetime`, `toml_writer`,
`serde_spanned`) is dead weight. Risk lies in correctly isolating
`marque-config`'s file-loading code under a feature flag without breaking the
CLI's layered `.marque.toml` → `.marque.local.toml` precedence.

Source touchpoints: `crates/config/Cargo.toml`, every `toml::*` call site in
`crates/config/src/`, `crates/wasm/Cargo.toml` (add
`default-features = false` to `marque-config` workspace-pinned dep).

### Win 3 — Confirm and feature-gate `jiff` / `IsmDate` (Finding 6)

Medium-high impact contingent on verification. `jiff` is a non-trivial dep
(~80-150 KB) and `declassify_on` parsing is the only consumer. If the WASM
hot path does not need date arithmetic (likely — no rule in `crates/capco/src/`
production code references `declassify_on`), gating `date.rs` behind a
`dates` feature in `marque-ism` removes the entire `jiff` dep from the WASM
build.

Verification step before commit: run `twiggy paths` from `lint_native` and
confirm no edges reach `jiff::civil::Date`.

## 5. Anti-recommendations

- **Don't remove `serde_json`'s `raw_value` feature.** It's load-bearing for
  `lint_batch_native` (`crates/wasm/src/lib.rs:550`) to avoid re-parsing
  diagnostic JSON when assembling batch responses.
- **Don't remove the decoder.** Despite its 7089-LOC weight, the WASM target
  intentionally runs `StrictOrDecoderRecognizer` per `lib.rs:31-46` and PR #259.
- **Don't dedup the JSON serializer between WASM and CLI.** The duplication is
  the load-bearing input to the SC-008 parity test
  (`tests/native_parity.rs`); merging them creates a single point of failure
  for byte-identical output.
- **Don't replace `talc` with the default `dlmalloc`.** `talc` is specifically
  configured for WASM `memory.grow` semantics
  (`crates/wasm/src/lib.rs:151-159`); switching back to the default allocator
  saves a few KB but pessimizes runtime allocator behavior.
- **Don't strip `tracing` calls.** They compile to near-zero overhead without
  a subscriber and removing them shows up in the *source*, not the artifact —
  no subscriber is pulled into WASM (verified in dep tree).
- **Don't feature-gate the `marque-scheme` lattice constructors individually.**
  DCE plus a single `CapcoScheme` instantiation already strips unused
  monomorphizations; explicit gating would multiply the configuration matrix
  for marginal gain.

## 6. Measurement plan

Baseline first. Build current artifact and record the headline numbers:

```
wasm-pack build crates/wasm --target web --profile release-web
ls -la crates/wasm/pkg/marque_wasm_bg.wasm     # uncompressed size
gzip -kn crates/wasm/pkg/marque_wasm_bg.wasm   # gzipped size proxy
```

Per top-3 win:

1. **Win 1 — vocab metadata gate.** Land the `vocab-metadata` feature in a
   throwaway branch. Build WASM with the feature off; diff the `.wasm` byte
   count against baseline. Run `twiggy top crates/wasm/pkg/marque_wasm_bg.wasm`
   on both builds and compare the top 20 contributors — expect `vocabulary.rs`
   strings to drop out of the rodata section.
2. **Win 2 — toml gate.** Land the `toml-loader` feature in `marque-config`.
   `cargo tree -p marque-wasm --target wasm32-unknown-unknown --no-default-features
   --features web` should no longer list `toml`, `toml_parser`, `winnow`,
   `toml_datetime`, `toml_writer`, `serde_spanned`. Confirm with size diff.
3. **Win 3 — jiff gate.** Before touching code, run `twiggy paths -o jiff
   crates/wasm/pkg/marque_wasm_bg.wasm` against the baseline to confirm `jiff`
   symbols are present. After landing the `dates` feature gate, confirm
   `cargo tree` no longer includes `jiff` for the WASM target and that the
   parity test (`crates/wasm/tests/native_parity.rs`) still passes — if
   `declassify_on` is silently exercised, the parity test will catch
   divergent NDJSON.

Cross-check across all three: `wasm-pack` size delta, `twiggy top` delta,
parity test green, `cargo test -p marque-wasm` green.

## References

- `crates/wasm/Cargo.toml:135-166` — direct deps
- `crates/wasm/src/lib.rs:31-46` — Constitution III decoder-in-WASM rationale
- `crates/wasm/src/lib.rs:810-1562` — `#[wasm_bindgen]` export surface
- `crates/engine/src/engine.rs:306-311` — `Engine::new` installs
  `StrictOrDecoderRecognizer`
- `crates/capco/src/vocabulary.rs:984-996` — `impl Vocabulary<CapcoScheme>`
- `target/debug/build/marque-ism-*/out/vocabulary.rs` — 104 KB metadata
- `target/debug/build/marque-capco-*/out/priors.rs` — 18 KB priors
- `crates/scheme/src/builtins.rs` — 1392 LOC lattice constructors
- `crates/ism/src/date.rs` — 2289 LOC `jiff`-dependent `IsmDate`
- Constitution III (CLAUDE.md, top): WASM-safe set + runtime-config restriction
