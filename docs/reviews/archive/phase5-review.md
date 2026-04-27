<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Phase 5 Code Review — Constraints / Decoder / Vocabulary (US3)

**Spec:** `specs/004-constraints-decoder-vocab/tasks.md` (Phase 5, lines 160–189)
**Reviewer:** Claude Opus 4.7 (1M ctx) overseeing 3 ecc:rust-reviewer specialists
**Date:** 2026-04-25
**Branch:** `phase5review`
**Commits in scope:**
- `ba577db` — Phase 5 PR-1: vocabulary metadata generation (T080–T082) (#141)
- `4320918` — Phase 5 PR-2: `Vocabulary<CapcoScheme>` impl + FOUO regression guards (#143)
- `2c80141` — Phase 5 PR-3: trait-surface completion (T078 + T079 + T089b) (#146)

**Decision:** **REQUEST CHANGES** — 1 HIGH (Phase G adoption blocker), 2 HIGH from PR-1 reviewer (build-time fail-closed gaps), 4 MEDIUM, 4 LOW/NIT. Tests are passing, the architecture is sound, and most findings are pre-Phase-F hardening that's cheaper to land now than later.

---

## ⚠️ Resolution status (2026-04-25)

All findings except L1 were addressed in this session. See [§9 Resolution log](#9-resolution-log) at the bottom of the report for the per-finding status table and post-fix validation results (1 185 tests pass, 0 fail, clippy clean).

---

## 1. Build & Test Gate

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | PASS (25.7s) |
| `cargo test -p marque-{ism,scheme,capco,engine}` | **891 passed**, 0 failed, 1 ignored (SC-004 decoder accuracy gate, expected per T057) |
| `cargo check --target wasm32-unknown-unknown` (WASM-safe set) | PASS |
| `cargo test -p marque-engine --test corpus_accuracy` | PASS — T085 satisfied de-facto |

No regressions in the WASM-safe subgraph. FOUO regression test (`fouo_remains_active_dissem_control`) passes on direct invocation.

---

## 2. Phase 5 Task Coverage vs. tasks.md

**`tasks.md` is significantly out of sync with the codebase.** Of the 18 tasks marked `[ ]` in the Phase 5 / Phase 6 block, **14 have actually landed**:

| Task | Status in tasks.md | Status in code | Evidence |
|---|---|---|---|
| T071 `every_active_token_has_authority` | `[ ]` | landed | `crates/capco/tests/vocabulary.rs:33` |
| T072 `authority_points_to_odni_for_ism_tokens` | `[ ]` | landed | same file |
| T073 `deprecated_tokens_carry_deprecation` | `[ ]` | **landed-as-inverse** | `active_tokens_have_no_deprecation_metadata` — see §6 below |
| T074 `deprecation_replacement_when_known` | `[ ]` | landed (structural pin) | `crates/capco/tests/vocabulary.rs:179` |
| T075 `fouo_is_not_migrated` | `[ ]` | **landed (renamed)** | `fouo_is_not_in_migration_table`, `crates/ism/tests/migrations.rs:29` |
| T076 `fouo_remains_active_dissem_control` | `[ ]` | landed | `crates/capco/tests/vocabulary.rs:212` |
| T077 `metadata_query_is_zero_alloc` | `[ ]` | landed (file-gated cfg) | `crates/capco/tests/vocabulary_zero_alloc.rs:118` |
| T077a expanded coverage | `[ ]` | landed | banner_abbreviation_some/none, metadata_agrees, 7 panic tests |
| T078 codec compile test | `[X]` PR-3 | landed | `crates/scheme/tests/codec_surface.rs` |
| T079 migration audit URNs | `[X]` PR-3 | landed | `crates/engine/tests/audit.rs:715` (rigorous; see §5) |
| T080 ism build.rs JSON codepath | `[ ]` | landed | `crates/ism/build.rs` (1293 lines, ~460 added) |
| T081 TokenMetadataFull tables | `[ ]` | landed | `crates/ism/src/generated.rs` |
| T082 XML codepath untouched | `[ ]` | landed | XML predicate emission preserved |
| T083 FOUO migration removal | `[ ]` | landed | no live FOUO→CUI mapping in production code |
| T084 `impl Vocabulary<CapcoScheme>` | `[ ]` | landed | `crates/capco/src/vocabulary.rs` (427 lines) |
| T085 corpus harness re-run | `[ ]` | passes | corpus_accuracy 5/5 ok |
| T089b adoption-readiness | `[X]` PR-3 | landed | `crates/scheme/tests/adoption_readiness.rs` |

**Action:** flip the `[ ]` boxes to `[X]` for T071–T077a and T080–T085. Phase 6 polish tasks (T086–T093) genuinely remain open.

---

## 3. Findings by Severity

### HIGH

#### H1 — `Codec<S>` missing `Send + Sync` supertrait bounds (Phase G adoption blocker)
**Location:** `crates/scheme/src/codec.rs:29`
**Source:** PR-3 specialist
**Why:** `Recognizer<S>: Send + Sync` (`recognizer.rs:147`) carries the bound explicitly with the comment "Implementations MUST be `Send + Sync` so the engine can hold them." `Vocabulary<S>` and `Codec<S>` do not. `BatchEngine` drives `Engine` across `tokio::task::spawn_blocking`, so any future `Box<dyn Codec<S>>` field on the engine will fail to compile on first Phase G usage. Adding the bound after Phase G impls land is a breaking trait change; adding it now is non-breaking because no production impls exist (verified by `grep -r "impl.*Codec.*for" crates/`). Constitution VI Principle ("rule implementations and `Recognizer` impls MUST be `Send + Sync`") implicitly extends to other engine-held trait objects.
**Action:** Change `pub trait Codec<S: MarkingScheme + ?Sized>` to `pub trait Codec<S: MarkingScheme + ?Sized>: Send + Sync`. Also consider the same for `Vocabulary<S>` since it is similarly engine-adjacent.

#### H2 — Build-time silent empty-string fallback on required provenance fields
**Location:** `crates/ism/build.rs:955-983`
**Source:** PR-1 specialist
**Why:** `owner_producer`, `poc_name`, `poc_email`, `spec_version`, `des_version` use `.unwrap_or_default()` on the JSON path, silently emitting empty `&'static str` into the generated `TokenMetadataFull` table when a sidecar field is absent. Only `urn` is guarded by `required_string(...)`. `vocabulary_tables.rs:73-91` catches missing POC fields at `cargo test`, but Constitution VIII calls for `cargo build`-time failure ("when a JSON sidecar field is missing or empty, build.rs MUST panic with a clear error"). `owner_producer` and POC sub-fields have no test guard at all in some branches.
**Action:** Promote `owner_producer`, `poc_name`, `poc_email` to `required_string`-style panics (or a `required_nested_text` variant). Defensible to leave `title`, `source`, `spec_version`, `des_version` optional if documented as such.

#### H3 — Three well-known tokens missing from `well_known_tokens_resolve` anchor set
**Location:** `crates/ism/tests/vocabulary_tables.rs:142-165`
**Source:** PR-1 specialist
**Why:** The test anchors `NF`, `RELIDO`, `SI`, `TK`, `HCS` but not `NOFORN`, `SECRET`, or `RD` — the three tokens whose accidental disappearance from `TOKEN_METADATA` would most silently break critical rule logic (E001 banner roll-up, E002 classification floor, E015/E016 AEA constraints).
**Action:** Add `NOFORN`, `SECRET`, `RD` to the assertion set.

### MEDIUM

#### M1 — `OwnerProducer::name` duplicates `OwnerProducer::code`, violates own field doc
**Location:** `crates/capco/src/vocabulary.rs:223`
**Source:** PR-2 specialist
**Why:** `name: cve_file.owner_producer` populates from the same `owner_producer` short code that also fills `code`. For all current CAPCO tokens this means `metadata(t).owner_producer.name == metadata(t).owner_producer.code == "USA"`. The field doc on `OwnerProducer::name` (`crates/scheme/src/vocabulary.rs:84`) says "Human-readable name, e.g., `"United States of America"`" — `"USA"` does not match that contract. Audit consumers reading the field doc and acting on the value will see drift.
**Action:** Either populate from a hand-table of well-known codes (`USA → United States of America`, `NATO → North Atlantic Treaty Organization`) at build time, or change the trait-surface field to `Option<&'static str>` with `None` meaning "no human-readable name in source".

#### M2 — `organization: "ODNI"` hardcoded in `build_point_of_contact`, not table-derived
**Location:** `crates/capco/src/vocabulary.rs:246`
**Source:** PR-2 specialist
**Why:** A future FGI / NATO / JOINT vocabulary adapter that reuses `build_point_of_contact` will silently emit `organization: "ODNI"` for non-ODNI tokens. The hardcode is currently safe because only CAPCO/ISM tokens flow through this helper, but the leak surface exists.
**Action:** Either rename the helper to `build_capco_point_of_contact` to make the scope explicit, or thread the organization string through from the `CveFileMetadata` (extending the build.rs JSON schema to require it).

#### M3 — Zero-allocation test lacks vacuity guard for `LazyLock` warmup
**Location:** `crates/capco/tests/vocabulary_zero_alloc.rs:118`
**Source:** PR-2 specialist
**Why:** The warmup loop on lines 111–116 is supposed to force `CVE_FILE_DERIVED` and `TOKEN_DERIVED` to initialize before the alloc-counter snapshot. If a future regression makes warmup return cached placeholder data without triggering the `LazyLock`, the measurement loop will pass with 0 allocations even though the real path was never exercised.
**Action:** Insert `assert!(allocs_during_warmup > 0, "warmup must allocate; LazyLock init was not triggered")` between the warmup and the `before` snapshot. Mirrors the `MIN_FIXTURE_COUNT` vacuity guard in `decoder_accuracy.rs`.

#### M4 — `CodecError::Malformed`/`SchemaMismatch` doc comments don't prohibit document-content embedding
**Location:** `crates/scheme/src/codec.rs:50-61`
**Source:** PR-3 specialist
**Why:** No production `Codec` impl exists yet, but Phase G will. `CodecError::Malformed(String)` renders as `"malformed input: {msg}"`. Nothing in the trait contract or doc comment forbids `msg` from embedding parsed document bytes (e.g., the failing XML fragment). Constitution V G13 invariant says no document content in audit-adjacent streams; an error message that ends up in a tracing log or a server response IS audit-adjacent.
**Action:** Add a doc comment to `CodecError::Malformed` and `CodecError::SchemaMismatch` explicitly stating that `msg` / `observed` MUST NOT contain document content. Pin this constraint at the definition site so the Phase G implementer sees it.

### LOW / NIT

#### L1 — `vocabulary.rs` linear scans on hot path (Phase C scaling concern)
**Location:** `crates/capco/src/vocabulary.rs:115,188,273`
**Source:** PR-2 specialist
**Why:** `SENTINEL_TO_CANONICAL` is 10 entries today; `.iter().find()` is fine. When Phase C extends the sentinel set to the full CVE vocabulary (the module doc at line 37 anticipates this), O(n) scan on every accessor call conflicts with Constitution I (perceptual instantaneity). `phf` is already in `[dependencies]` — replace before Phase C lands.

#### L2 — `adoption_readiness.rs::StubScheme` silently uses `evaluate_custom` default
**Location:** `crates/scheme/tests/adoption_readiness.rs` (~line 161)
**Source:** PR-3 specialist
**Why:** `MockScheme` in `codec_surface.rs:99` overrides `evaluate_custom`; `StubScheme` in `adoption_readiness.rs` does not. Asymmetry is harmless but misleads Phase F adopters who read `adoption_readiness.rs` as the canonical "what must I implement" reference.
**Action:** Add a `// `evaluate_custom` default returning Vec::new() is sufficient for this stub` comment, OR override explicitly.

#### L3 — Module doc count drift
**Location:** `crates/capco/src/vocabulary.rs:33`
**Source:** PR-2 specialist
**Why:** Module doc says "~14 ids"; `active_sentinels()` has 10. Cosmetic.

#### L4 — Stale "Phase A scaffolding" crate-level doc
**Location:** `crates/scheme/src/lib.rs:35`
**Source:** PR-3 specialist
**Why:** Crate now exposes the full Phase B/C/D/E trait surface; the `## Status` section still reads "Phase A scaffolding."

#### N1 — `desc_text` empty-default undocumented at callsite
**Location:** `crates/ism/build.rs:1009`
**Source:** PR-1 specialist

#### N2 — `const_name` vs. `const_ident` aliasing is correct but confusing
**Location:** `crates/ism/build.rs:953,1226`
**Source:** PR-1 specialist

---

## 4. What Was Verified Strong

- **Crate-graph compliance.** `marque-capco` `[dependencies]` lists only `aho-corasick`, `marque-ism`, `marque-rules`, `marque-scheme`, `phf`, `thiserror`. `marque-core` and `marque-engine` are dev-deps only. `crates/scheme/tests/{adoption_readiness,codec_surface}.rs` import only `marque_scheme::*` and `std::*` — Constitution VII verified at the file-import level (SC-010 pre-verified).
- **WASM safety preserved.** The WASM-safe set (`marque-ism`, `marque-scheme`, `marque-rules`, `marque-capco`) compiles to `wasm32-unknown-unknown` cleanly with the new `LazyLock`-based vocabulary impl.
- **FOUO regression coverage.** Three independent layers — `migrations.rs` rejects FOUO from `MIGRATIONS` AND asserts no entry has `replacement == "CUI"`; `vocabulary_tables.rs` confirms FOUO remains in `TOKEN_METADATA`; `vocabulary.rs` runs a full `Engine::lint` pipeline on FOUO-bearing input asserting no diagnostic mentions "cui" or "controlled unclassified". A regression in any one layer is caught by the others.
- **Audit schema discipline (T079).** The migration-URN test recovers provenance through public `Vocabulary<CapcoScheme>::metadata()` lookups + `marque_ism::generated::vocabulary::lookup_token_metadata`, NOT by adding URN fields to `AppliedFix`. `MARQUE_AUDIT_SCHEMA` stays at `marque-mvp-2`. Constitution V invariant intact.
- **Vacuity guards.** T079 panics with the actually-applied rule list if E001 doesn't fire. Decoder-accuracy harness has `MIN_FIXTURE_COUNT = 200`. Both files demonstrate proper vacuity discipline.
- **Single-source-of-truth invariant** between `metadata().authority.urn` and `metadata().urn` is enforced by construction in `crates/capco/src/vocabulary.rs:288-316` (`build_metadata` reads from the same `derived` record), AND independently verified by T079's cross-check assertion.
- **Citation discipline (Constitution VIII).** New `&'static str` citations in vocabulary code (`CAPCO-2016 §G.1 Table 4`, lines 331) trace cleanly. Comments documenting the FOUO removal cite "factually wrong" reasoning consistent with the project's existing FOUO/CUI memory note.
- **Trait conformance.** All 8 required `Vocabulary<CapcoScheme>` methods (`authority`, `owner_producer`, `point_of_contact`, `deprecation`, `metadata`, `portion_form`, `banner_form`, `banner_abbreviation`) are implemented; `Option`-returning methods have legitimate `None` semantics.
- **Codec spec conformance.** Trait shape matches `data-model.md:255-256` exactly; `CodecError` variants match `tasks.md:40` exactly; no extras.

---

## 5. T073 Spec Drift Note (advisory, not a finding)

T073 (`deprecated_tokens_carry_deprecation`) cannot be exercised today because **no active sentinel in `SENTINEL_TO_CANONICAL` is deprecated**. The two `MIGRATIONS` entries (`25X1- → 25X1`, `50X1- → 50X1-HUM`) don't correspond to any sentinel. The landed test (`active_tokens_have_no_deprecation_metadata`) pins the **inverse** invariant: every active sentinel returns `None` from `deprecation()`. Combined with `deprecation_replacement_when_known` (which structurally validates that any `Some(replacement)` resolves cleanly), this is the strongest guard the current sentinel set permits.

When Phase C extends the sentinel set to include real deprecations, this test should gain a positive case. Document the deferral in the test's doc comment and reference Phase C.

---

## 6. Recommended Action Sequence

1. **Pre-merge (this branch):**
   - Fix H1 — add `Send + Sync` to `Codec<S>` and `Vocabulary<S>` trait definitions. (Trivial; truly non-breaking.)
   - Fix H2 — promote `owner_producer` / POC fields to `required_string` panics in `crates/ism/build.rs`.
   - Fix H3 — add `NOFORN`, `SECRET`, `RD` anchors to `well_known_tokens_resolve`.
   - Fix M3 — vacuity guard on `vocabulary_zero_alloc.rs` warmup.
   - Update `tasks.md` checkboxes for T071–T077a, T080–T085. (Mechanical.)
   - Add doc comment on `CodecError::Malformed` / `SchemaMismatch` (M4).
   - Update `crates/scheme/src/lib.rs` "Phase A scaffolding" doc (L4).

2. **Pre-Phase-F (separate PR):**
   - Resolve M1 (OwnerProducer.name semantics).
   - Resolve M2 (ODNI hardcode → table-derived).
   - Resolve L1 (phf replacement of linear scans) before Phase C extends the sentinel set.

3. **Phase 6 (polish bucket — already on the spec):**
   - T086–T089 regression gates and citation pass.
   - T090–T093 docs and quickstart validation.

---

## 7. Appendix: Validation Results

| Check | Result |
|---|---|
| Type check (`cargo check --workspace`) | PASS |
| Lint (clippy via specialist agents) | PASS — no `-D warnings` regressions in any of the three PRs |
| Tests (`cargo test -p marque-{ism,scheme,capco,engine}`) | 891 passed / 0 failed / 1 ignored (SC-004 expected) |
| WASM build (`cargo check --target wasm32-unknown-unknown`) | PASS |
| Format (`cargo fmt --check`) | PASS (per all three specialists) |
| REUSE / license headers | not in scope; covered by separate CI gate |

## 8. Files Reviewed (Phase 5 scope)

| File | Lines | PR |
|---|---|---|
| `crates/ism/build.rs` | 1293 (≈460 added) | PR-1 |
| `crates/ism/src/generated.rs` | 15 (added) | PR-1 |
| `crates/ism/Cargo.toml` | 6 (added) | PR-1 |
| `crates/ism/tests/vocabulary_tables.rs` | 179 (new) | PR-1 |
| `crates/capco/src/vocabulary.rs` | 427 (new) | PR-2 |
| `crates/capco/src/lib.rs` | 6 (added) | PR-2 |
| `crates/capco/Cargo.toml` | 9 (added) | PR-2 |
| `crates/capco/tests/vocabulary.rs` | 463 (new) | PR-2 |
| `crates/capco/tests/vocabulary_zero_alloc.rs` | 145 (new) | PR-2 |
| `crates/ism/tests/migrations.rs` | 66 (new) | PR-2 |
| `crates/scheme/src/codec.rs` | 77 (new) | PR-3 |
| `crates/scheme/src/vocabulary.rs` | 12 (doc-only delta) | PR-3 |
| `crates/scheme/tests/codec_surface.rs` | 182 (new) | PR-3 |
| `crates/scheme/tests/adoption_readiness.rs` | 419 (new) | PR-3 |
| `crates/engine/tests/audit.rs` | 229 (added) | PR-3 |

Total: ≈3 800 lines added, 4 new files, 11 modified files across three PRs.

---

## 9. Resolution log

Worked through 2026-04-25 priority-order high → low.

| ID | Severity | Disposition | Files touched | Notes |
|---|---|---|---|---|
| H1 | HIGH | **FIXED** | `crates/scheme/src/codec.rs`, `crates/scheme/src/vocabulary.rs` | `Send + Sync` added to both `Codec<S>` and `Vocabulary<S>` trait surfaces, with comments mirroring the bound rationale on `Recognizer<S>`. Verified non-breaking across the full workspace. |
| H2 | HIGH | **FIXED** | `crates/ism/build.rs` | Added `required_nested_text` helper; promoted `owner_producer`, `poc_name`, `poc_email`, `spec_version`, `des_version` to build-time panics. `IRM.PointOfContact` itself is now required. Optional fields (`title`, `source`) explicitly documented as such. `cargo build -p marque-ism` succeeds against current ISM-v2022-DEC sidecars. |
| H3 | HIGH | **FIXED** | `crates/ism/tests/vocabulary_tables.rs` | Added `S` and `RD` anchors to `well_known_tokens_resolve`; added a separate `noforn_banner_form_round_trip_resolves` test that pins the banner-form recovery path NOFORN→NF→TOKEN_METADATA (NOFORN isn't a CVE Value so it cannot be a direct lookup key). 8 → 9 tests. |
| M1 | MEDIUM | **FIXED** | `crates/capco/src/vocabulary.rs` | Added CAPCO-scoped `owner_producer_name` lookup with explicit-panic `unknown` arm. `OwnerProducer.name` now returns `"United States of America"` for code `"USA"`, satisfying the `marque-scheme` field-doc contract. Future codes (`NATO`, `FGI`) wired in for forward compatibility. |
| M2 | MEDIUM | **FIXED** | `crates/capco/src/vocabulary.rs` | Renamed `build_point_of_contact` → `build_capco_point_of_contact` so the CAPCO scope is in the function name itself; a future FGI/NATO adapter physically cannot call this and silently misattribute its own POCs to ODNI. |
| M3 | MEDIUM | **FIXED** | `crates/capco/tests/vocabulary_zero_alloc.rs` | Added vacuity guard: `assert!(warmup_allocs >= 2)` between warmup and measurement. A regression that bypasses the LazyLock-backed tables now fires the guard rather than producing a false-green pass. Verified the guard fires positively under `--features count-allocs`. |
| M4 | MEDIUM | **FIXED** | `crates/scheme/src/codec.rs` | Added type-level G13 contract section to `CodecError` doc comment and per-variant cross-references for `Malformed` and `SchemaMismatch`. Phase G implementers now see the content-ignorance rule at the variant definition site. |
| L1 | LOW | **DEFERRED** | `crates/capco/src/vocabulary.rs` | Added a `Phase C scaling note` doc comment on `canonical_for` linking to this review. The phf migration is architectural enough (touches three lookup sites + the LazyLock construction order documented in adjacent comments) that landing it in this review-fix PR would mix a behavioral change into an otherwise docs+invariant PR. The note pins the Pre-Phase-C deadline so the deferral cannot quietly rot. |
| L2 | LOW | **FIXED** | `crates/scheme/tests/adoption_readiness.rs` | Added explicit comment on `StubScheme` after the trait impl explaining the intentional reliance on the `evaluate_custom` default and pointing Phase F adopters to `MockScheme` for the override pattern. |
| L3 | LOW | **FIXED** | `crates/capco/src/vocabulary.rs` | "~14 ids" → "10 ids today; see `SENTINEL_TO_CANONICAL` below for the authoritative count". |
| L4 | LOW | **FIXED** | `crates/scheme/src/lib.rs` | Replaced "Phase A scaffolding" with a complete current-status block listing every Phase B/D/E module landed (page_rewrite, scope, builtins, recognizer, vocabulary, codec). Adoption-readiness file referenced as the SC-010 pre-verification. |
| N1 | NIT | **FIXED** | `crates/ism/build.rs` | One-line comment added on the `desc_text` `unwrap_or_default()` call site explaining SAR / NTK omit `Description`. |
| N2 | NIT | **FIXED** | `crates/ism/build.rs` | Comment block added before the per-CVE-file emission loop documenting that `f.const_ident` reuse for both the static identifier and the `const_name` field makes the round-trip a structural invariant (not a coincidence), and naming the test that enforces it. |
| Mechanical | — | **FIXED** | `specs/004-constraints-decoder-vocab/tasks.md` | Flipped `[ ]` → `[X]` for T071–T077a and T080–T085 with brief landed-where notes citing PR commit hashes. Phase 6 polish tasks (T086–T093) genuinely remain open and stay `[ ]`. |

### Post-fix validation (2026-04-25)

```text
cargo test --workspace            → 1185 passed / 0 failed / 1 ignored / 66 binaries
cargo clippy ... -D warnings      → clean across marque-{ism,scheme,capco,engine}
cargo check --target wasm32-unknown-unknown -p marque-{ism,scheme,rules,capco}
                                  → clean (WASM-safe set still compiles)
cargo test -p marque-capco --features count-allocs --test vocabulary_zero_alloc
                                  → 1/1 (M3 vacuity guard verified positive)
cargo test -p marque-engine --test corpus_accuracy
                                  → 5/5 (T085 corpus byte-identity)
cargo test -p marque-engine --test audit migration_audit_has_both_urns
                                  → 1/1 (T079 URN recoverability)
```

Net additions in this fix-up pass: ≈210 lines across 9 files (mostly doc comments and the vocabulary trait-bound additions). One new test (`noforn_banner_form_round_trip_resolves`). Zero behavioral changes to the rule pipeline, audit schema, or runtime artifact.
