# PR 3c — Decision Set 03: Empirical Concerns (Decisions 7–10)

**Status.** Recommendation document. Not a plan. One of four parallel decision-point analyses for PR 3c.

**Date.** 2026-05-10.

**Scope.** Operational / empirical decisions: renderer test strategy (D7), `ProjectedMarking` materialization cost (D8), audit-schema cutover timing (D9), recognizer diagnostic surface (D10).

**Method.** Read-only investigation of `crates/{scheme,ism,rules,engine,capco,core}/`, `crates/capco/CAPCO-CONTEXT.md`, `architecture.md`, `rule-body-audit.md`, `crates/capco/docs/CAPCO-2016.md`. Bench numbers measured locally on this dev box (WSL2 / Ryzen-class single-thread). Citation discipline per Constitution VIII.

---

## Decision 7 — Renderer test strategy

### PM's lean

Renderer test harness inside PR 3c. Property tests (`render(parse(render(parse(x)))) == render(parse(x))`; lattice-equal inputs render byte-identical) plus golden-output corpus.

### Evidence

**Existing test surface — what is in tree today.**

- Round-trip property test exists at `/home/knitli/marque/crates/capco/tests/parse_render_roundtrip.rs` (T097, 460 lines). It runs the corpus-wide narrow round-trip on the **classification axis only**, because the current `MarkingScheme::render_portion` / `render_banner` impl on `CapcoScheme` is a Phase-A stub that drops every non-classification axis (`crates/capco/src/scheme.rs:2210-2224`). The full-attribute round-trip (`fn full_attribute_round_trip_across_strict_corpus`, line 420) is gated `#[ignore = "blocked on T048 / PR 3c: full MarkingScheme::render_canonical not yet implemented; current renderer covers classification level only. Re-enable when T048 lands."]`. The body of that test is already the property the PM wants — it asserts `attrs1 == attrs2` after one render-then-reparse cycle on every fixture in `tests/corpus/valid/` (68 fixtures). It is one `#[ignore]` line away from being live.
- Lattice-laws property tests live at `/home/knitli/marque/crates/capco/tests/lattice_laws.rs` (idempotence, commutativity, associativity for `SciSet` / `SarSet` join + meet). They do NOT cover render-byte-identity.
- Targeted FR-016/FR-017 round-trips at the same file (lines 294-398) — synthetic fixtures for bare FGI, single-trigraph FGI, multi-trigraph FGI, SAR program-only, SAR program+compartment. Five fixtures. Same narrow classification-only invariant.
- Golden-byte audit-record snapshots exist (`crates/engine/tests/snapshots/fix_pipeline__audit_record_snapshot_e001_apply@marque-mvp-{1,2}.snap`). They pin **audit-record bytes**, not **rendered marking bytes**.
- WASM↔native byte-identity is enforced (`crates/wasm/tests/parity.rs`, `deep_scan_parity.rs`, `deadline_parity.rs`) — but byte-identity is across **NDJSON output of the same `Engine::lint`/`fix` call**, not across renders of lattice-equal inputs.
- `crates/capco/tests/scheme_equivalence.rs` exists per CLAUDE.md but pins `PageContext::expected_*` ↔ `scheme.project(Scope::Page, ...)` byte-identity for **lattice output values**, again not rendered byte-identity.

**No byte-output renderer test today.** The renderer body is a stub; the property test framework anticipates its arrival; the rule-body audit (`rule-body-audit.md` lines 96-99, 130-143) names ~16 hand-written form rules whose end-state is "absorbed by the renderer".

**Articulating the property tests in Rust pseudocode.**

```rust
// 1. Idempotent render-parse round-trip (FR-019 / SC-010)
//    Already exists, gated #[ignore].
//    Reading: render(parse(x)) is what canonical form should be; re-parsing
//    that should produce the same projected facts; re-rendering should be
//    byte-identical; one cycle is enough because canonical form is a fixed
//    point under render(parse(_)).
fn render_parse_idempotent(input: &str) {
    let attrs1 = parse(input);
    let s1     = render_canonical(&attrs1);   // first render
    let attrs2 = parse(&s1);
    let s2     = render_canonical(&attrs2);   // second render
    assert_eq!(attrs1, attrs2,
        "facts must survive one render→parse cycle");
    assert_eq!(s1, s2,
        "canonical form must be a fixed point under render(parse(_))");
}

// 2. Lattice-equal renders byte-identical (architecture.md §"Render"
//    line 67: "Two ProjectedMarkings that are lattice-equal render to
//    byte-identical output.")
fn lattice_equal_renders_byte_identical(a: &str, b: &str) {
    let pa = project(parse(a));               // ProjectedMarking
    let pb = project(parse(b));
    if pa == pb {                             // lattice-equal under Eq
        assert_eq!(render_canonical(&pa),
                   render_canonical(&pb),
                   "lattice equality must imply render equality");
    }
}
```

The property the PM names — `render(parse(render(parse(x)))) == render(parse(x))` — is the **fixed-point** of `canonicalize = render ∘ parse`. Restate as: "running canonicalize twice equals running it once," `canonicalize(canonicalize(x)) == canonicalize(x)`. This is the right property given the bag-of-tokens architecture: the input form may vary (different delimiters, sort orders, abbreviations); the canonical output is the **representative** of an equivalence class on lattice-equal inputs. It is not full bidirectional round-trip identity (`render(parse(x)) == x`); that would fail by design on `EYES` ↔ `EYES ONLY` and similar — and architecture.md §"Render" line 67 explicitly says canonical form picks one representative.

**Lattice-equal renders-byte-identical — concrete fixture pairs.** Five candidates from CAPCO-2016 / `CAPCO-CONTEXT.md` §1.2 + §H:

1. `OC/NF` ↔ `NF/OC` (within-IC-dissem reorder; §G.1 Table 4 fixes order to `OC/NF`; the fact set is `{ORCON, NOFORN}` either way).
2. `REL TO USA, GBR, CAN` ↔ `REL TO CAN, GBR, USA` (USA-first canonicalization per `§H.8 p150-151`; fact set is `{USA, GBR, CAN}` — the renderer chooses USA-first then alpha).
3. `EYES` ↔ `EYES ONLY` (`§H.8 p157` allows both forms; canonicalize picks one — choice belongs to the renderer per architecture.md §"Render" line 65).
4. `(SECRET//FGI DEU GBR//NOFORN)` ↔ `(SECRET//FGI GBR DEU//NOFORN)` (FGI multi-country list is space-separated alphabetical per `§H.7 p123`).
5. `JOINT SECRET CAN GBR USA` ↔ `JOINT SECRET GBR CAN USA` (JOINT list pure-alpha per `§H.3 p56`; the user-feedback note in `MEMORY.md` — "joint-usa-first" S003 is convention layered above §H.3 — is a separate style-rule consideration).

Plus a reverse-direction case:

6. `SI-G ABCD EFGH` ↔ `SI-G EFGH ABCD` (SCI sub-compartments numeric-then-alpha space-separated per `§H.4 p61` + `§A.6 p15`; fixture for sub-compartment ordering inside a compartment).

**Golden corpus — can existing fixtures be reused?**

Yes. `tests/corpus/valid/` carries 68 fixtures (`ls /home/knitli/marque/tests/corpus/valid/ | wc -l = 68`) of single-marking-per-file, well-formed inputs. Existing `parse_render_roundtrip.rs` already drives every fixture through `detect_kind` + parse + render + reparse; flipping the `#[ignore]` line on `full_attribute_round_trip_across_strict_corpus` activates the round-trip on the full attribute surface. The mismatch the file currently calls out — that the renderer is a stub — is exactly what PR 3c is delivering.

For the lattice-equal-renders-byte-identical property, fixtures need **pairs**, not singletons. Existing `tests/corpus/valid/` does not carry pairs. Estimate: ~15–25 hand-written pairs covering the form-rule axes catalog from `rule-body-audit.md` (delimiter, sort, abbreviation, set-canonicalization, separator collapse, JOINT-USA-first, SCI sub-compartment ordering, REL TO trigraph dedup). One pair per axis at minimum; a few axes deserve 2–3 pairs (REL TO and SCI grammar are richer than single-axis). 15–25 pairs is the same order of magnitude as the FR-016/FR-017 targeted fixtures already in `parse_render_roundtrip.rs` (5 cases) — adding 15–25 more pairs is a few hours of fixture authorship.

**Cost estimate.** ~250–400 lines of test code:

- Re-enable existing `full_attribute_round_trip_across_strict_corpus` (1 line: delete the `#[ignore]`).
- One new test file `crates/capco/tests/render_canonical_property.rs` for the lattice-equal-byte-identical property (~150 lines: harness + ~20 fixture pairs).
- Optionally ~50 lines extending `parse_render_roundtrip.rs` synthetic FR-* tests to cover SCI grammar, dissem ordering, AEA SIGMA, JOINT lists, declass dates.

The harness body is small because the surface under test is one trait method (`MarkingScheme::render_canonical` once it exists; today `render_portion` / `render_banner`). Most of the LOC is fixture data, not test logic.

**Risk if NOT added in PR 3c — concrete regressions the rule-layer tests would miss.**

- **Form rule retirement masks renderer drift.** `rule-body-audit.md` lists ~16 form rules whose end-state is "absorbed by the renderer with byte-identical diagnostics." If the renderer's USA-first sort drifts to alphabetical, the **rule** retires correctly (no E002 diagnostic emitted because the rule no longer exists), but the **rendered output** is wrong (`REL TO CAN, GBR, USA` instead of `REL TO USA, CAN, GBR`). No test catches this without a renderer-byte property. Constitution Principle VIII makes this a correctness defect: §H.8 p150-151 explicitly mandates USA-first.
- **Lattice-equal but non-canonical renders.** If `OC/NF` parses to the same projected fact set as `NF/OC` (which it must, by lattice law), but the renderer sometimes emits `OC/NF` and sometimes `NF/OC` based on input order, the fact-set delta vocabulary (`FactAdd` / `FactRemove`) cannot detect the regression — both inputs produce zero `FactAdd` / `FactRemove` deltas; only `Recanonicalize` would fire, and the rule body that decides whether to emit `Recanonicalize` is **the renderer comparing input bytes to its own output**. Without a render-byte-identity property test, the comparison's correctness is unverified.
- **WASM↔native parity at the audit-stream level survives** (existing `crates/wasm/tests/*parity*.rs`), but only because both targets share the same `Engine::lint` / `Engine::fix` codepath. Render-byte drift is invisible to that gate.
- **Snapshot tests** (`snapshots/fix_pipeline__audit_record_snapshot_e001_*`) pin audit bytes for **specific** input fixtures — they catch wholesale regressions on those fixtures but say nothing about the property being tested.

### Recommendation

**Add the renderer test harness in PR 3c.**

Concretely:

1. Re-enable `full_attribute_round_trip_across_strict_corpus` (`crates/capco/tests/parse_render_roundtrip.rs:420`) by removing the `#[ignore]`. The body is already the right property for FR-019 / SC-010 across the 68-fixture corpus.
2. Add `crates/capco/tests/render_canonical_property.rs` with:
   - **Property 1**: `canonicalize(canonicalize(x)) == canonicalize(x)` over the corpus + the 6 lattice-equal pair fixtures listed above (pick one representative from each pair; both must canonicalize to the same bytes).
   - **Property 2**: `lattice_equal_renders_byte_identical` over the 6 pair fixtures plus 10–15 additional pairs covering the form-rule axes catalog from `rule-body-audit.md` (delimiter, sort, abbreviation, set-canonicalization, separator collapse, JOINT-USA-first, SCI sub-compartment ordering, REL TO trigraph dedup).
3. Cite §A.6 p15-16 (canonical syntax authority), §G.1 p36-38 (Register order), §H.3 p56 (JOINT alpha), §H.4 p61 (SCI), §H.5 p99-100 (SAR), §H.7 p123 (FGI), §H.8 p150-151 (REL TO) per Principle VIII; verify each citation against `crates/capco/docs/CAPCO-2016.md` before merge.
4. Defer adversarial / proptest fuzzing of the renderer to a follow-up. The harness above is the load-bearing minimum — proptest with shrinking is a worthwhile next step but is not the first move.

### Rationale

- The property test infrastructure already exists and is one `#[ignore]` line away from being live; declining to re-enable it in PR 3c is functionally a choice to ship the renderer without testing the property the file was authored to test. That is the unforced-error path.
- ~16 form rules retiring into the renderer means the renderer becomes a load-bearing trust surface. The rule-layer tests retire alongside the rules; without renderer property tests, **the form-correctness gate is gone**.
- The lattice-equal-renders-byte-identical property is the **defining** invariant of architecture.md's "form is not shape" principle (§3.0.a, lines 168-172). PR 3c is where the principle becomes load-bearing; the test that pins it should land with it. Deferring the test to a follow-up is functionally deferring "do we believe the principle holds in our code?" to a follow-up.
- Cost is modest (~250–400 LOC, mostly fixture data; the harness logic is reused from `parse_render_roundtrip.rs`).

### Tradeoffs

- **Pro**: catches the regression class the rule-layer tests cannot see; activates the existing T097 / SC-010 surface; restores the FR-019 round-trip closure.
- **Pro**: gives the form-rule retirements a backstop — they retire safely because the renderer-property tests guard the byte output the rules used to enforce.
- **Con**: 15–25 hand-authored fixture pairs is a non-trivial authorship task; getting the §H.* canonical form exactly right per pair is what Constitution Principle VIII demands and what makes the work non-trivial. The cost is real, just not large.
- **Con**: when renderer behavior shifts (e.g., a future PR changes USA-first to alphabetical for some defensible reason), every fixture pair would need re-curation. The fixture set is **versioned with the renderer**; this is the same maintenance cost the corpus-byte-identity gate already carries (`crates/capco/tests/corpus_parity.rs`).

### Confidence

**High** that adding the harness in PR 3c is the right call. The infrastructure is in place, the property is architecturally load-bearing, the cost is modest, and the regression class it catches is exactly the class that the form-rule retirements are designed to push into the renderer.

---

## Decision 8 — `ProjectedMarking` materialization cost

### PM's lean

Explicit benchmark deliverable. SC-001 (p95 ≤16 ms on 10 KB) verification before form rules migrate.

### Evidence

**`ProjectedMarking` in code today.**

The type is defined at `crates/ism/src/projected.rs:59` (`pub struct ProjectedMarking`). The crate-level doc-comment at `projected.rs:5-23` is explicit:

> At PR 3a no engine call site reads or writes `ProjectedMarking` — `PageContext::expected_*` continues to drive page roll-up. The type is `pub` and its `dead_code` is suppressed only when the workspace lints flag it (Risk #6 in the PR 3a design doc).

The type is defined; nothing constructs one yet. The engine reads `PageContext::expected_*` (`crates/ism/src/page_context.rs:114-700+`), which is the **closest current analog** — and the cost analysis below uses `PageContext` as the proxy.

**`PageContext::new()` and `add_portion`.**

```text
// crates/ism/src/page_context.rs:108-124
pub struct PageContext { portions: Vec<CanonicalAttrs> }

pub fn new() -> Self { Self::default() }                      // zero-cost
pub fn add_portion(&mut self, attrs: CanonicalAttrs) {
    self.portions.push(attrs);                                // amortized O(1)
}
```

Construction is trivial. The work happens in the lazy `expected_*` methods (12 of them — `expected_classification`, `expected_sci_controls`, `expected_sci_markings`, `expected_sar_marking`, `expected_dissem_controls`, `expected_rel_to`, `expected_declassify_on`, `expected_declass_exemption`, `is_classified`, `expected_aea_markings`, `expected_fgi_marker`, `expected_non_ic_dissem`).

**Per-page work in PageContext rollups (order-of-magnitude estimate).** Each `expected_*` is O(P × K) over P portions × K tokens per axis. The hot bodies allocate `BTreeSet<&str>` / `BTreeMap<String, BTreeMap<String, BTreeSet<String>>>` per call (`page_context.rs:178, 199-247, 262-322, 333-374, 394-471, 539-607, 620-700+`). For a typical 10 KB document with ~10–20 portions × ~5 tokens per axis: a few hundred BTreeSet inserts per axis × 12 axes = low thousands of inserts per page. On a modern x86 CPU at 3 GHz that's microsecond territory, not millisecond.

The `expected_sci_markings` pass at line 199-247 is the widest one — it builds a three-level `BTreeMap<system, BTreeMap<comp_id, BTreeSet<sub>>>` then sorts each level per §H.4 numeric-then-alpha. That is the worst-case axis on a doc with rich SCI markings; it remains low microseconds at typical scale.

**Closure-to-fixpoint cost.** Architecture.md §"Project" line 53-58 names the closure operator: NODIS implies NOFORN, EXDIS implies NOFORN, HCS-O implies NOFORN+ORCON, per-token classification floors. The per-axis lattices in `marque-capco::lattice` are finite-height up to open-vocab carriers (`SciSet`, `SarSet`, `FgiSet`). For CAPCO's listed implications:

- HCS-O → NOFORN+ORCON: 1 step (HCS-O fires → add NOFORN, add ORCON; nothing implied by NOFORN/ORCON in turn).
- NODIS → NOFORN: 1 step.
- EXDIS → NOFORN: 1 step.
- Class-floor (HCS-comp-sub requires class ≥ TS; SAR requires C; SI requires C): 1 step (classification monotone-bumps).

Worst-case iteration count: **2** (one round to add the implications, one round to confirm fixpoint reached). Closure cost is bounded by the implication catalog size — tens of entries — not by document size; it is per-page, not per-portion.

**Page rewrites currently on `CapcoScheme`.** Found at `crates/capco/src/scheme.rs:597-900+`, built by `build_page_rewrites` (called from `CapcoScheme::new` at line 421). Eight rewrites declared (per the per-entry block constants `NF_*`, `E1_*`, `E2_*`, `E3_*`, `E4_*`, `E5_*`, `E6A_*`, `E6B_*`, `E7_*`):

1. `capco/noforn-clears-rel-to` — `Contains{CAT_DISSEM, NOFORN} → Clear{CAT_REL_TO}` (declarative, scheduler-orderable).
2. `capco/frd-sigma-consolidates-into-rd-sigma` — within-AEA transform (Custom; self-edge per `crates/engine/src/scheduler.rs:84-87`).
3-9. Bare-FGI rollup, FGI-R class-floor, JOINT cross-class rollup, ORCON-NATO transmute, SBU-NF/LES-NF transmute, US-presence FGI promote (Custom).

Each is a per-page closure over the projected state. Cost = predicate scan (linear in token-set sizes) + action (constant for `Clear`/`Replace`; bounded by axis depth for `Custom`). The topological scheduler runs each rewrite once per page (`crates/capco/src/scheme.rs:2165-2200`), in deterministic Kahn order. Total per-page rewrite cost: order of tens of microseconds at typical scale.

**Topological scheduler — already-built infrastructure.** `crates/engine/src/scheduler.rs` runs Kahn's once at `Engine::new` (per CLAUDE.md "topological page-rewrite scheduler"). The cached order drives per-document evaluation without re-sorting. Schedule cost is **amortized to zero** at lint time.

**Caching strategy — per-page cache within a lint run.** The lint loop is at `crates/engine/src/engine.rs:320` (`pub fn lint`). Each candidate is a banner / portion / CAB. The current strategy builds `PageContext` incrementally as the lint loop scans portions and resets at scanner-emitted page-break candidates (per CLAUDE.md). The natural place for the `ProjectedMarking` cache is the same invalidation point: drop on page break, materialize lazily when the first banner-validation rule on the new page asks for it. Memory cost = one `ProjectedMarking` per page in flight (single-threaded lint). For batch processing, one per `BatchEngine` worker × pages-in-flight; bounded by the existing concurrency controller.

Today's `PageContext::expected_*` calls are not memoized — every banner-validation rule that calls (e.g.) `expected_classification` re-scans every accumulated portion. This is the **measured baseline** for SC-001. Materializing `ProjectedMarking` once per page is **strictly cheaper** than the current re-scan-per-axis-per-rule pattern, by a factor of ~12 (number of `expected_*` axes) × number-of-rules-per-page (~10 banner-validation rules). Even ignoring that win, the absolute cost is small.

**Current SC-001 status — measured numbers.**

I ran `cargo bench --bench lint_latency -- lint_10kb --quick` on this dev box (WSL2, single-thread):

```text
lint_10kb               time:   [821.40 µs 823.54 µs 832.09 µs]
```

Mean **824 µs**, upper-CI **832 µs**. SC-001 budget is **16,000 µs** (16 ms). Headroom: ~19× under budget. The `benches/baseline.json` reference baseline (GitHub Actions ubuntu-latest) is upper-CI 828 µs; the +10 % regression gate sits at ~911 µs.

I also ran `cargo bench --bench decoder_10kb_rel_to_invariant -- --quick`:

```text
decoder_10kb_rel_to_invariant  time:   [983.17 µs 988.29 µs 1.0088 ms]
```

Decoder fallback: ~988 µs mean against an 18,000 µs SC-002 budget. Headroom: ~18×.

**Implication.** The strict path is currently using ~5 % of its SC-001 budget. The decoder path uses ~5.5 % of its SC-002 budget. Materializing a `ProjectedMarking` once per page on the strict path — replacing the 12-method-re-scan-per-rule pattern in `PageContext::expected_*` — is **almost certainly a net latency win**, not a loss. The benchmark deliverable is still warranted, but the load-bearing risk is regression beyond ~10 % of budget, not pure budget exhaustion.

### Recommendation

**Take the PM's lean: explicit benchmark deliverable on `lint_10kb` + `decoder_10kb_rel_to_invariant` before form-rule migration begins. Pin the comparison to the `benches/baseline.json` `target_upper_ci_us` SC-001/SC-002 gates already enforced by `scripts/bench-check.sh`.**

Concretely:

1. Capture a fresh `lint_10kb` baseline immediately before the first form-rule retirement lands. Record p95 / p99 / upper-CI.
2. Re-run after `ProjectedMarking` materialization is wired (PR 3c body).
3. Re-run after each form-rule retirement batch in the migration sequence.
4. The existing `bench-check.sh` infrastructure already enforces `+10 %` vs baseline and the absolute SC-001 ceiling. No new bench infrastructure is needed; the **deliverable** is the measurement point, not new code.
5. Per-page `ProjectedMarking` cache lands inside the engine's lint loop (`crates/engine/src/engine.rs:320` and surroundings), invalidated at scanner-emitted page-break candidates per Constitution Principle VI.

### Rationale

- The cost question is not "is materialization too expensive?" — the answer is clearly no, given a 19× headroom on `lint_10kb`. The cost question is **regression detection**: when 16 form rules retire and the renderer absorbs their canonicalization knowledge, the rendering path's per-call cost grows. The benchmark gate is what catches a 10× regression on render before it ships.
- The benchmark infrastructure (`benches/baseline.json`, `bench-check.sh`, the `lint_10kb` and `decoder_10kb_rel_to_invariant` benches) already enforces the SC-001 / SC-002 ceilings on every PR via CI. The deliverable is a discipline commitment, not an engineering deliverable.
- Materialization cost is per-page and amortizes across the rules on that page. Today's `PageContext::expected_*` is per-rule per-axis — the migration is **cheaper**, not more expensive, in the steady state.
- The closure-to-fixpoint converges in ≤2 iterations on CAPCO's implication catalog. Iteration depth is not a hot-path concern.
- The 8 page rewrites are bounded-cost per-page operations under Kahn-scheduled order; the scheduler already amortizes the topological-sort cost to engine construction.

### Tradeoffs

- **Pro**: closes the SC-001 verification loop; gives PR 3c reviewers a concrete measurement to read; activates the existing CI gate.
- **Pro**: a measured baseline before/after each form-rule retirement makes the migration's per-step cost visible; if rule N retires and the bench moves +20 %, the responsible commit is identifiable.
- **Con**: each form-rule retirement PR carries a small benchmark-running cost (CI minutes). Already absorbed by the existing gate.
- **Con**: the WSL2 dev box numbers above (824 µs, 988 µs) are **not** the SC-001 / SC-002 reference; the GHA `ubuntu-latest` baseline at upper-CI 828 µs is. WSL2's virtualized HV clock means absolute numbers are not directly comparable across machines (`benches/baseline.json` `_note` on `deadline_overhead`). The discipline is to use `bench-check.sh`'s relative-to-baseline gate, not raw numbers.

### Confidence

**High** that materialization is cheap in absolute terms. **High** that the PM's benchmark-deliverable lean is the right discipline. **Medium** on the exact migration-step granularity — measuring after every retirement vs. after batches of retirements is a process call that can adjust based on observed variance.

### Measured numbers

| Bench | Mean (µs) | Upper-CI (µs) | Budget (µs) | Headroom | Source |
|-------|-----------|---------------|-------------|----------|--------|
| `lint_10kb` (strict) | 824 | 832 | 16,000 (SC-001) | 19.2× | local WSL2 dev box, `--quick` |
| `decoder_10kb_rel_to_invariant` | 988 | 1009 | 18,000 (SC-002) | 17.8× | local WSL2 dev box, `--quick` |
| `lint_10kb` reference baseline | 824 | 828 | 16,000 | 19.3× | `benches/baseline.json` (GHA ubuntu-latest, 2026-04-26) |

The local and reference numbers are within noise of each other on the strict path. Decoder path is +10 % vs the reference baseline in absolute terms; no regression flag because `--quick` mode + WSL2 jitter explains the gap.

---

## Decision 9 — Audit-schema cutover timing

### PM's lean

Cutover at end (after last form rule retires), as a single commit. Constitution V's "single binary emits exactly one schema" means it can't be mid-migration.

### Evidence

**Constitution V actual wording.**

From `/home/knitli/marque/.specify/memory/constitution.md` (Principle V "Audit-First Compliance"):

> Every applied fix MUST produce a complete audit record. Auditability is non-negotiable in the IC/DoD compliance context.
>
> [...]
>
> Audit records MUST be content-ignorant. No document content, document metadata field values, or subject-claim free-form text MAY appear in an `AppliedFix` or any future audit-adjacent record [...]. Permitted identifiers in audit output are: token canonicals, category IDs, span offsets, digests (BLAKE3 of content), posterior scalars, and enumerated feature labels. This is the G13 invariant from the 2026-04-19 recursive-lattice plan and the I-J2 / I-K2 invariants from the 2026-04-20 roadmap, unified. Corpus-level integration tests MUST verify no document text appears verbatim in engine output streams.

Note: the constitution does not literally say "single binary emits exactly one schema." That language is from `CLAUDE.md`:

> **Audit schema**: `MARQUE_AUDIT_SCHEMA` env var pinned at build time, validated against the closed accept-list `["marque-mvp-1", "marque-mvp-2"]`. Defaults to `"marque-mvp-2"` (Phase D, decoder + provenance). Re-exported as `marque_engine::AUDIT_SCHEMA_VERSION`. A single binary emits exactly one schema (FR-014).

`MARQUE_AUDIT_SCHEMA` env-pin behavior at build time, from `crates/engine/build.rs:25-38`:

```rust
const ACCEPTED: &[&str] = &["marque-mvp-1", "marque-mvp-2"];
const DEFAULT: &str = "marque-mvp-2";

let schema = std::env::var("MARQUE_AUDIT_SCHEMA").unwrap_or_else(|_| DEFAULT.to_string());
[...]
println!("cargo:rustc-env=MARQUE_AUDIT_SCHEMA={schema}");
println!("cargo:rerun-if-env-changed=MARQUE_AUDIT_SCHEMA");
```

Re-exported at `crates/engine/src/lib.rs:63`:

```rust
pub const AUDIT_SCHEMA_VERSION: &str = env!("MARQUE_AUDIT_SCHEMA");
pub const AUDIT_SCHEMA_IS_V2: bool = const_str_eq(AUDIT_SCHEMA_VERSION, "marque-mvp-2");
```

The schema is **compile-time pinned**. A binary built with `MARQUE_AUDIT_SCHEMA=marque-mvp-1` cannot emit `marque-mvp-2` records and vice versa. This makes the cutover question into "when do we bump `DEFAULT`?" plus "when do we retire the `marque-mvp-1` accept-list entry?"

**`AppliedFix`'s current shape.**

From `crates/rules/src/lib.rs:439-454`:

```rust
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct AppliedFix {
    pub proposal: FixProposal,
    pub confidence: Confidence,
    pub source: FixSource,
    pub timestamp: SystemTime,
    pub classifier_id: Option<Arc<str>>,
    pub dry_run: bool,
    pub input: Option<Arc<str>>,
}
```

The `proposal: FixProposal` field carries `span`, `original`, `replacement` (both `Box<str>`), `confidence`, `source`, `migration_ref`. The top-level `confidence` and `source` are `marque-mvp-2` v2 promotions of the same data already in `proposal` — kept redundantly for v1↔v2 emitter compatibility (`crates/rules/src/lib.rs:423-436`). `non_exhaustive` already lets the type accept new fields without a breaking change to construction sites that use `__engine_promote(...)`.

**`FactAdd` / `FactRemove` / `Recanonicalize` in code today — none.**

Grepping for `FactAdd`/`FactRemove`/`Recanonicalize` finds zero hits in `/home/knitli/marque/crates/rules/src/`. The architecture has been planned (`architecture.md` §"What fixes are", lines 105-138; rule-body audit catalog), but no concrete Rust types exist. The closest current code:

- `crates/rules/src/fix_intent.rs`: `FixIntent<S>` + `ReplacementIntent<S>` (`Cve { token: TokenId, scope: Scope }`, `Render { category, directive, scope }`, `Delete`). Defined; `RenderDirective<S> = PhantomData<S>` per the PR 3c.1 contract — directive lifts to a real assoc type in PR 3c.2.
- The mapping in `architecture.md` lines 109-138 is: `FactAdd → ReplacementIntent::Cve` or `Render` (positive token contribution); `FactRemove → ReplacementIntent::Delete` (subtractive); `Recanonicalize → ReplacementIntent::Render` over a whole scope (form rendering). The names differ; the structural commitment is the three-variant vocabulary either way.

**Walking the emission types — additive vs breaking.**

For each emission type in the architecture restatement:

- **`FactAdd`** (or `ReplacementIntent::Cve`/`Render`) — the engine renders the fact-set delta to bytes via `MarkingScheme::render_canonical`, then promotes. The `AppliedFix` it produces carries the standard fields: `proposal.span` (where the fix lands), `proposal.replacement` (the rendered canonical bytes for the new fact set), `confidence`, `source = FixSource::BuiltinRule`. **Schema-compatible with `marque-mvp-2`** — the audit-record shape is unchanged; only the rule-side emission type differs.

- **`FactRemove`** (or `ReplacementIntent::Delete`) — engine renders the fact-set with the token absent, promotes. Today's RELIDO cluster (E054-E057, per `rule-body-audit.md` lines 79-82) already does exactly this in span-surgery form: `FixProposal { replacement: "", confidence: 0.95, ... }`. The emitter shape is identical. **Schema-compatible with `marque-mvp-2`**.

- **`Recanonicalize`** (or `ReplacementIntent::Render { scope }`) — engine renders the whole scope's canonical form, promotes. The `proposal.replacement` carries the renderer's canonical bytes for the entire scope (banner / portion / CAB), not a within-token replacement. **Schema-compatible with `marque-mvp-2`** — the schema does not constrain the granularity of the replacement string, only that one exists.

**Breaking concerns I went looking for and did not find.**

- No new required field on `AppliedFix`. The `FixIntent → AppliedFix` lifecycle (from `crates/rules/src/fix_intent.rs:16-34`) is: rule emits `FixIntent`; engine renders via `MarkingScheme::render_canonical`; engine builds `FixProposal` (existing type) from the rendered bytes; engine promotes to `AppliedFix` via the existing `__engine_promote` seal. Every existing field is populated from data the new emission types already carry.
- No semantics change on existing fields. `proposal.original` continues to be the source bytes at the span. `proposal.replacement` is still the bytes that go in their place. `Confidence` is unchanged (its `recognition × rule` axis split is already in `marque-mvp-2`).
- The closed `feature_ids` list (`crates/rules/src/confidence.rs`) is the only existing schema-change frontier — adding a new `FeatureId` variant requires a coordinated `MARQUE_AUDIT_SCHEMA` bump per CLAUDE.md ("Adding a `FeatureId` variant requires a coordinated bump of `MARQUE_AUDIT_SCHEMA`"). PR 3c does not propose new feature IDs based on architecture.md.

**G13 audit-content-ignorance — walking each emission type.**

Constitution V Principle V mandates: "No document content, document metadata field values, or subject-claim free-form text MAY appear in an `AppliedFix`. Permitted identifiers: token canonicals, category IDs, span offsets, digests, posterior scalars, enumerated feature labels."

- **`FactAdd { token, scope }`**: payload is `TokenId` (a category-ID-like enumerated identifier per `crates/scheme/src/scheme.rs`), `CategoryId`, `Scope` (enum). The rendered `replacement` bytes are **token canonicals** — by construction, the renderer produces only canonical-vocabulary bytes (it reads from `MarkingScheme::vocabulary` per `architecture.md` §"Render"; canonical bytes are the closed CAPCO grammar). **No leak channel.** ✅
- **`FactRemove { token_ref, scope }`**: payload is a `TokenId` reference + `Scope`. Replacement bytes are the rendered fact-set without that token — again, canonical-vocabulary-only. **No leak channel.** ✅
- **`Recanonicalize { scope }`**: replacement is the entire-scope canonical render. The renderer **never emits content from the input**; it emits only canonical-vocabulary tokens for the projected facts. The closed `proposal.original` field carrying input bytes is the place a leak could happen — but PR 3c doesn't change `proposal.original` semantics; existing G13 enforcement (R001 decoder path sets `proposal.original = ""` per `crates/engine/src/engine.rs:1442-1448`) covers the recognizer leak, and form-rule recanonicalize emissions inherit the existing `FixProposal::original` discipline. **No new leak channel introduced by `Recanonicalize`.** ✅

The corpus-level integration test for content-ignorance lives at `crates/capco/tests/s004_audit_content_ignorance.rs` (per the `find` output earlier). The constitution mandates "Corpus-level integration tests MUST verify no document text appears verbatim in engine output streams" — that test already runs on every PR; the new emission types inherit the gate without modification.

### Recommendation

**Take the PM's lean, with one refinement: cutover at end, single commit, AND keep `marque-mvp-1` in the accept-list for at least one minor version after the bump.**

Concretely:

1. PR 3c lands with `MARQUE_AUDIT_SCHEMA=marque-mvp-2` as the default (unchanged from today). Rule emission migrates to `FixIntent<S>` (or `FactAdd`/`FactRemove`/`Recanonicalize`-shaped) types **without changing the `AppliedFix` shape or the emitted audit-record schema bytes**. Existing snapshot tests at `crates/engine/tests/snapshots/fix_pipeline__audit_record_snapshot_e001_apply@marque-mvp-{1,2}.snap` continue to match.
2. The cutover is a **logical** cutover, not a schema cutover. The form rules retire; the renderer absorbs their work; the audit-record bytes for the same input stay byte-identical. Constitution V's "single binary emits exactly one schema" stays satisfied because the schema **doesn't change**.
3. If a follow-up PR ever needs to extend `AppliedFix` (new `FeatureId` variant, new top-level field), that's the moment to introduce `marque-mvp-3` in `engine/build.rs:25` and bump `DEFAULT`. The `marque-mvp-1` accept-list entry can retire whenever the existing v1-byte snapshot tests are no longer load-bearing for a real consumer; that is a separate decision tracked outside PR 3c.

### Rationale

- **The cutover is additive at the audit-record level, not breaking.** The investigation above walked every emission type and found no new required field, no semantics change on existing fields, no new content-leak channel. The schema does not need to bump.
- **The PM's "single commit at end" framing assumed a schema bump was on the table.** It isn't, given the additive analysis. The end-of-migration commit is a **logical** consolidation point — the moment all form rules have retired and the renderer carries the canonicalization knowledge — not a schema-version event. Treating it as a schema bump would be premature and would force consumers (downstream tooling, audit-stream parsers) to carry a v1↔v2-style compat layer for a non-event.
- **`MARQUE_AUDIT_SCHEMA=marque-mvp-2` is already the default.** `crates/engine/build.rs:26`. The constitution's "single binary emits exactly one schema" already holds.
- **The existing v1↔v2 cutover is the precedent.** It introduced top-level `confidence` / `source` fields on `AppliedFix` that mirror `proposal.confidence` / `proposal.source` (`crates/rules/src/lib.rs:423-436`). That's the shape of an **additive** cutover that bumped the schema label. If PR 3c warranted a schema bump, it would look like that — adding new fields, marking `marque-mvp-3` accepted, defaulting to it. PR 3c's emission-type migration does not have that shape.

### Tradeoffs

- **Pro (recommendation)**: PR 3c does not re-version the audit format unnecessarily. Downstream consumers do not have to add a v3 parser branch for a non-change.
- **Pro**: Existing snapshot tests at both `@marque-mvp-1.snap` and `@marque-mvp-2.snap` continue to constrain the byte output; the migration is verified at the byte level by the existing gate.
- **Con (recommendation)**: if a future review discovers a leak channel I missed in the architecture-only walk-through (no concrete `FactAdd` / `FactRemove` / `Recanonicalize` types exist yet in tree), that's a strict regression I can't see today. Mitigation: the `s004_audit_content_ignorance.rs` corpus test is mandatory per Constitution V and runs on every PR; a leak introduced by the new emission types fails CI.
- **Pro (PM lean as stated)**: a single end-of-migration commit makes the schema event reviewable in one diff.
- **Con (PM lean as stated)**: if the cutover is treated as a schema bump but no field changes, the version-bump ceremony adds review overhead without semantic content.

### Confidence

**High** that the cutover is additive and does not need a schema-version bump. **Medium-high** that the recommendation generalizes — concrete `FactAdd` / `FactRemove` / `Recanonicalize` types do not yet exist in tree, so the analysis is architecture-driven. The first PR that introduces them should re-walk the G13 gate explicitly; the existing corpus content-ignorance test catches regressions automatically.

---

## Decision 10 — Recognizer diagnostic surface

### PM's lean

Distinct rule IDs for recognizer signals. R001 + whatever S005 / S006 become as separate rule IDs, not consolidation.

### Evidence

**R001 in code today.**

From `crates/engine/src/engine.rs:43-59`:

```rust
/// Synthetic rule identifier the engine attaches to decoder-path
/// `FixSource::DecoderPosterior` diagnostics emitted from
/// `Engine::lint`. Phase 4 PR-4b mints this identifier so the
/// recognition-layer rewrite carries a real `RuleId` (rules and
/// fixes share that requirement) without colliding with any CAPCO
/// `E### / W### / C### / S###` namespace. A diagnostic stamped
/// `R001` originates from the decoder, not from a CAPCO rule.
const DECODER_RULE_ID: &str = "R001";

const DECODER_CITATION: &str = "CAPCO-2016 §A.6 p15";
```

Citation verified against `/home/knitli/marque/crates/capco/docs/CAPCO-2016.md` line 49 (table of contents — §A.6 "(U) Formatting" begins on page 15). Constitution VIII compliant.

R001's emission body lives at `crates/engine/src/engine.rs:1383-1462` (`build_decoder_diagnostic`). Diagnostic shape:

- `Diagnostic.rule = RuleId::new("R001")`
- `Diagnostic.severity = Severity::Fix` (or `Severity::Warn` for `FixSource::DecoderClassificationHeuristic`)
- `Diagnostic.message = "decoder-recognized canonical form: {replacement:?}"` (replacement only — original bytes elided per G13)
- `Diagnostic.citation = "CAPCO-2016 §A.6 p15"`
- `Diagnostic.fix = Some(FixProposal { rule: R001, source: DecoderPosterior, span, original: "", replacement, confidence, migration_ref: None })`

The `proposal.original = ""` is the G13 closure (`engine.rs:1442-1448` doc comment).

**S005 / S006 bodies.**

From `crates/capco/src/rules.rs:3344-3345` header:

> // Rules: S005 + S006 — REL TO membership-uncertain reduction (issue #206)

Architecture (lines 3346-3377):

> Conceptually one diagnostic with a context-dependent severity (Info when the banner is consistent with atom-semantics; Suggest when not), per plan §3.1. Implementation-wise two registered rules because `marque_engine::Engine::lint` overwrites every emitted diagnostic's severity with the rule's configured/default severity (engine.rs `// Apply configured severity override`); a single rule cannot stably emit at two different severities. Both rules share `analyze_uncertain_reduction` and split only on which branch they keep.

Concrete shapes (lines 3409, 3427):

- `RelToOpaqueUncertainReductionSuggestRule` — `RuleId::new("S005")`, `Severity::Suggest`, no fix (`fix: None`) per the doc-comment "The rule cannot resolve the ambiguity from in-tree data".
- `RelToOpaqueUncertainReductionInfoRule` — `RuleId::new("S006")`, `Severity::Info`, no fix.

Citation: `S005_CITATION = "CAPCO-2016 §H.8 [...]"` (line 3768; verified per Constitution VIII — §H.8 is the IC dissem-controls section starting p131). Both rules share `analyze_uncertain_reduction` (line 3540) and the citation; they differ only in which branch (`S005Branch::Suggest` vs `S005Branch::Info`) they filter to.

**The S005/S006 split is a workaround.** The header at `rules.rs:3344-3357` is explicit: "a single rule cannot stably emit at two different severities" because engine.rs overwrites severity post-emission. This is the same architectural pressure the recognizer surface faces — multiple distinct signals dispatched through one rule entrypoint.

**StrictRecognizer / DecoderRecognizer dispatch surface.**

`crates/engine/src/recognizer.rs` (StrictRecognizer, lines 58-118; trait impl at line 68 `fn recognize(&self, bytes: &[u8], _cx: &ParseContext) -> Parsed<CapcoMarking>`). On strict-parse failure → `Parsed::Ambiguous { candidates: vec![] }`. No diagnostic emitted from inside the recognizer — emission is the engine's job at `engine.rs:527+` ("Synthesize an R001 `decoder-recognition` diagnostic").

`StrictOrDecoderRecognizer` at `crates/engine/src/decoder.rs:4181-4258`: composition of `StrictRecognizer` + `DecoderRecognizer`. Dispatcher logic at line 4196: strict first, decoder fallback on incomplete-strict-Unambiguous or zero-candidate strict-Ambiguous. Returns `Parsed<CapcoMarking>`; the diagnostic synthesis is downstream in `engine.rs::build_decoder_diagnostic`.

**The dispatch surface bottlenecks through `Parsed<S::Marking>`.** The `Recognizer::recognize` trait surface (in `marque-scheme::recognizer`) returns a `Parsed<M>` enum — `Unambiguous(M)` or `Ambiguous { candidates: Vec<Candidate<M>> }`. The recognizer emits **markings**, not **diagnostics**. The diagnostic shape — including the rule ID — is the engine's choice when it post-processes the recognizer output (`engine.rs:527+, 1383+`).

**Architecturally, the rule-ID surface admits multiple distinct recognizer rule IDs naturally.** The engine's call to `build_decoder_diagnostic(...)` (line 1389) returns a `Diagnostic` with whatever `RuleId` the engine constructs. The current code hardcodes `R001`; nothing in the recognizer trait or the dispatcher prevents the engine from constructing `R002` for a different recognizer signal (e.g., decoder-classification-heuristic vs decoder-vocab-mangle) or routing different signals to different IDs.

**Rule-ID prefix convention — verified.**

The full set of registered IDs in production code (from earlier grep):

`C001, E001-E041 (with gaps), E052-E060, E997, S001-S006, S999, W002, W003, W034`. Plus engine-side: `R001`.

CLAUDE.md "Adding a New Rule" §3:

> Rule IDs follow: `E###` = error, `W###` = warning, `C###` = correction.

Constitution Principle IV "Rule IDs MUST follow the convention: `E###` (error), `W###` (warning), `C###` (correction)."

The constitution lists three prefixes; the codebase uses six (`C`, `E`, `W`, `S`, `R`, plus the `E997` / `S999` debug-only sentinel suffixes). `R###` and `S###` are de facto extensions that the constitution doesn't enumerate. The engine's R001 doc-comment is explicit (`engine.rs:48`):

> A diagnostic stamped `R001` originates from the decoder, not from a CAPCO rule.

The convention extends naturally: `R###` for recognizer-emitted diagnostics, `S###` for "suggest" / style rules whose authority is not a §-mandate (S001-S006 are all style/heuristic per their own docs).

**Distinct recognizer rule IDs are naturally accommodated.** Adding `R002` (whatever S005/S006 become if they migrate to recognizer-emission) is a one-line constant addition + a `build_*_diagnostic` builder. No trait change needed.

**"Admonition channel" — proposed in architecture.md, not yet built.**

`architecture.md` §"The §3.0.b purpose split" line 161 names the channel in the purpose-row table:

| Admonition / warning notice (RD warning, RAWFISA notice, IMCON SAT warning) | admonition emitter (separate channel) | n/a |

Grep for `admonition`/`Admonition` in `crates/`: zero hits (the only matches were in `crates/capco/docs/CAPCO-2016.md` itself, lines 245, 2687, 2832 — manual content, not code). The admonition emitter does not exist as a code surface today. The two registered rules whose end-state per `rule-body-audit.md` is admonition (W002 us-fgi-comingling-caution, W034 sci-custom-control-info) currently emit through the standard `Rule::check → Vec<Diagnostic>` channel with `Severity::Warn`. They would migrate to the admonition channel as part of architecture.md's purpose-row migration.

**Scoping the admonition emitter.** Architecture.md §"What rules are" line 75: rules are divergence detectors comparing input to projection. Admonitions are **not divergence** — they are advisory information attached to a token regardless of context (RD always carries an RD warning per §H.6 p104; IMCON-SAT carries the SAT-watch notice per §H.8 p142; RAWFISA notice per §H.8 p161). Implementing the admonition channel:

- A new diagnostic-shape variant or a new emitter trait surface (~50–100 LOC in `marque-rules`).
- A scheme-side data declaration (`Vocabulary<S>::admonitions(token) -> &'static [Admonition]`) — 10–20 LOC plus per-token data tables generated from the `crates/capco/build.rs` ODNI/CAPCO ingestion pipeline.
- An engine-side dispatch point in the lint loop that runs admonition lookup over emitted tokens and surfaces them in the diagnostic stream — ~30 LOC.

Total scope: ~100–150 LOC plus build-time data, plus tests. Non-trivial but not large. **Out of scope for PR 3c.** It belongs to the form-rule retirement migration that PR 3c enables but doesn't itself execute.

### Recommendation

**Take the PM's lean: distinct rule IDs for recognizer signals, no consolidation.**

Concretely:

1. R001 stays as the existing decoder-recognition rule ID.
2. If S005 / S006 migrate to recognizer-emission as part of the rel-to-uncertain-reduction recognition surface (which architecture.md `rule-body-audit.md` lines 56-57 frames as "true `Constraint::Custom` candidate (or admonition territory) — surfaces an uncertainty band"), they become **distinct R-prefix IDs** at migration time (e.g., `R002`, `R003`), not collapsed into R001. Migration timing is **post-PR-3c**; PR 3c does not move S005/S006.
3. The `R### / S### / W### / E### / C###` prefix convention is documented as the de facto namespace partition. The constitution lists `E###`/`W###`/`C###`; CLAUDE.md should restate the de facto extensions (`R###` recognizer, `S###` suggest/style) when the next constitution amendment opens.
4. The admonition channel (architecture.md §3.0.b row 6) does not land in PR 3c. It lands in the migration that retires W002 / W034 (and absorbs RD/RAWFISA/IMCON-SAT/CNWDI admonition data). PR 3c is structural prep; admonition is a downstream consumer of that prep.

### Rationale

- **The S005/S006 split exists because severity and rule-ID are 1:1 in the current emission surface** (`crates/capco/src/rules.rs:3346-3357`). Collapsing the recognizer signals into one rule ID would re-introduce the same architectural pressure the S005/S006 split is working around. Distinct IDs are the path of less resistance.
- **Distinct rule IDs are also distinct configuration channels.** A user who wants `R001 = "warn"` (decoder fired, but I want a non-blocking signal) is configuring a different concern than `R002 = "off"` (don't tell me about uncertain REL TO reductions). Collapsing the IDs collapses the config surface; distinct IDs preserve the user's ability to opt in/out per signal.
- **The recognizer trait surface admits multiple IDs naturally.** The diagnostic shape is constructed by the engine post-`recognize()`, not by the recognizer itself. Adding R002 / R003 is a builder change in `crates/engine/src/engine.rs`, not a trait change.
- **The `R###` prefix is a de facto, not de jure, extension of the constitution's `E###`/`W###`/`C###`.** The codebase uses six prefixes today (R, S also live extensions); making the R-prefix the recognizer family is consistent with how the codebase has actually grown.
- **The admonition channel does not exist.** Building it inside PR 3c would be feature-creep. The right home is the form-rule migration that retires W002 and W034 — that PR has the natural test surface (one admonition fixture per token type) and the natural data surface (`Vocabulary<S>::admonitions`).

### Tradeoffs

- **Pro**: distinct rule IDs preserve the per-signal config and observability surface. Users can mute the decoder without muting the uncertain-REL-TO surfacer; teams can require `R### = "off"` in CI without affecting other recognizer signals.
- **Pro**: matches existing precedent (S005/S006 split for the same architectural reason).
- **Pro**: deferring the admonition channel to a downstream PR keeps PR 3c's scope tight and aligned with its structural commitments (renderer + form-rule retirement + audit-schema continuity).
- **Con**: the `R### / S### / W### / E### / C###` convention is documented in CLAUDE.md / Constitution Principle IV as `E###/W###/C###` only. Adding `R###` and `S###` formally is an amendment that hasn't happened. Mitigation: open a constitution-amendment PR alongside the next constitutional update; the de facto convention is consistent enough that documenting it is a wording change, not a behavior change.
- **Con**: distinct IDs mean more total IDs in the catalog. The end-state target ("~10 surviving rules" per CLAUDE.md PR 3b history) is a count of **CAPCO** rules; recognizer / decoder IDs sit alongside the CAPCO catalog, not inside it (see the engine.rs:48 doc-comment "without colliding with any CAPCO `E### / W### / C### / S###` namespace"). The total-ID count grows by the number of recognizer signals, which is small (R001 today, R002+ post-migration).

### Confidence

**High** that the PM's lean is correct. The S005/S006 split is the in-tree precedent — it exists for the exact architectural reason a consolidation here would re-create. The recognizer surface and the rule-ID prefix convention both naturally accommodate distinct R-prefix IDs.

**Medium** on the admonition channel scoping — I have not implemented it, only scoped its surface. The 100–150 LOC estimate is based on the trait + data-table + dispatch surfaces named by architecture.md and could grow if the engine's lint-loop integration is more involved than the read suggests.

---

## Cross-decision interactions

**D7 ↔ D8 (renderer test ↔ materialization cost).** The renderer property tests measure correctness; the bench gates measure cost. They are independent regression-detection surfaces — correctness regressions show up as failed property tests; cost regressions show up as failed `bench-check.sh` thresholds. Both should land in PR 3c; neither subsumes the other. The lattice-equal-renders-byte-identical property has a small benchmark cost (it runs as a `#[test]`, not a bench), but it does add CI minutes; estimate is small (a few seconds per property invocation × 20–25 fixture pairs).

**D7 ↔ D9 (renderer test ↔ audit cutover).** The existing `crates/engine/tests/snapshots/fix_pipeline__audit_record_snapshot_e001_apply@marque-mvp-{1,2}.snap` files pin audit bytes per-schema. If the recommendation in D9 holds (no schema bump in PR 3c), both snapshot files continue to constrain the audit format. The new renderer property tests in D7 do not touch audit bytes — they pin **rendered marking bytes**, which feed into `proposal.replacement` but are a layer below the audit-record schema. The two test surfaces are independent; both stay green by construction if neither layer regresses.

**D8 ↔ D9 (cost ↔ cutover).** Materializing `ProjectedMarking` once per page is **strictly cheaper** than the per-rule re-scan pattern in `PageContext::expected_*` today. The audit-cutover recommendation (no schema bump) means audit-emission cost is unchanged. Net: the cost story for PR 3c is **strict-recompose cheaper, audit-emission unchanged, render path bounded by the bench gate**. No interaction risk.

**D9 ↔ D10 (audit cutover ↔ recognizer surface).** R001 already lives in `marque-mvp-2`. Adding R002 / R003 (whatever S005/S006 become at recognizer-migration time) is **additive** at the schema level — same `AppliedFix` shape, different `RuleId` value. The schema does not need to bump for new recognizer rule IDs, just as it does not need to bump for new CAPCO rule IDs. This reinforces D9's recommendation that PR 3c does not need a schema-version event.

**D7 ↔ D10 (renderer test ↔ recognizer surface).** Recognizer-path R001 diagnostics carry `proposal.replacement` rendered through the strict-canonical path (per `engine.rs::build_decoder_diagnostic` passing `provenance.canonical_bytes` to `FixProposal::new`). The renderer property tests in D7 cover the strict-path canonicalization that the decoder also produces; a renderer regression caught by D7 would also affect R001 outputs, so D7 is the upstream gate for D10's correctness.

**Summary of interactions.** D7 and D8 are independent and both should land in PR 3c. D9's no-schema-bump recommendation is independent of D7 / D10 and is supported by both (no new audit-record fields proposed). D10 is a forward-pointing decision (R001 stays; R002+ at migration time, post-PR-3c) that does not block PR 3c body. PR 3c shape: structural prep (renderer + form-rule retirement), with property tests and bench gates as the load-bearing regression surfaces.
