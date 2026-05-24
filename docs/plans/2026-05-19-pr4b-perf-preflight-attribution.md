<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 4b-perf closeout — Cumulative Attribution Preflight

> **Scope.** Rough-attribution walkdown of every PR / commit landed
> between pre-PR-4 baseline and current HEAD (PR 4b umbrella + intervening
> staging merges + PR 5 + PR 6c + post-PR-4b closeout fixes). Diagnosis-
> only — no profiling executed. Estimates are qualitative magnitude bands
> based on what the code changes *should* cost given their structural
> shape; the implementation-phase profiling agent measures.
>
> **Companion docs.** Read alongside the perf-engineer preflight at
> `docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` (measurement
> methodology) and the architect preflight at
> `docs/plans/2026-05-19-pr4b-perf-preflight-architect.md` (PR scope +
> remediation contract).

---

## Reference range

| Anchor | SHA | Date | `lint_10kb` mean/upper-CI | WASM size |
|---|---|---|---|---|
| **Pre-PR-4 baseline** (PR 9c.2, S007 / 006 T135 — last commit before PR 4a) | `18cef6c9` | 2026-05-15 | ~749µs / 753µs (per 2026-04-26 GHA `ubuntu-latest` baseline, the closest pre-PR-4 reference; "sub-500µs" user recollection is pre-006-refactor) | 1,234,106 B (~1.18 MB) — established by PR 3d (#408) |
| **Current HEAD** (PR 4 closeout T119 probe) | `81694384` | 2026-05-19 | 913µs / 914µs *baseline-pinned* (GHA, PR #498 re-capture); user reports ~1.7ms actual on a recent recapture (not yet pinned) | 1,386,447 B (~1.32 MB) *baseline-pinned*; user reports ~1.6 MB |

**Cumulative regression** (per `docs/plans/2026-05-19-pr4b-perf-preflight-performance.md` §1.1 +
project memory `project_perf_regression_4_to_6`):

- `lint_10kb`: pre-PR-4 ~749µs (baseline-pinned 2026-04-26) → current
  user-reported ~1.7ms. Approximately **2.2-2.3×** cumulative across the
  PR-4-to-6 umbrella. **Not a single-PR regression** — no individual
  landed PR violated the +10% drift gate against its predecessor's
  baseline; the staging-merge cycle re-captured the baseline twice
  (2026-05-05 D8 widening 753→828; 2026-05-17 PR #498 828→914) to keep
  the gate from flapping. The user-reported ~1.7ms post-PR-6c reading
  has not yet been pinned via a re-capture.
- WASM size: 1,234,106 → 1,386,447 *pinned* (+~150 KB net per baseline.txt
  history). User reports ~1.6 MB actual (+~210 KB beyond the pinned
  baseline). Constitution III WASM-binary-size constraint not violated;
  no absolute ceiling in the constitution (the bench-check gate is a
  regression-noise gate, not a ceiling).
- Engine-crate src (engine/scheme/capco/ism/core/rules): +28,635 / -12,054
  lines = +16,581 net LOC across 92 files. Capco dominates: +12,452 net
  on 52 files.

---

## PR-by-PR walkdown

Chronological, by merge date. SHAs are squash-merge commits on
`origin/staging`. Engine LOC Δ counts `crates/{engine,scheme,capco,ism,core,rules}/src/**`
only. "Hot-path?" = per-call `Engine::lint` cost (vs `Engine::new`
construction or build-time).

| Merge SHA | Date | PR# | Title (truncated) | Engine src Δ | Hot-path? | Native impact | WASM impact | Notes |
|-----------|------|-----|-------------------|--------------|-----------|---------------|-------------|-------|
| `6b5cf45d` | 05-15 | #425 | perf(capco): drop cold-path Vec<&TokenSpan> in RelToBlock | small | construction-only | LOW (improvement) | LOW | RelToBlock cold path |
| `fc91852e` | 05-15 | #422 | **PR 4a**: Vocabulary<S>::is_fdr_dissem trait method | +345/-7 | construction-only | LOW | LOW (~few KB) | New trait method on `Vocabulary<S>`; `is_fdr_dissem` reads `&'static` tables |
| `5ee497c2` | 05-15 | #426 | **PR 4b-A**: AEA control set Lattice (`AeaSet`) | +498/-13 | construction-only at this point | LOW | LOW-MED (~5-20 KB; one Join+Meet impl + ~per-token static data) | First lattice type; not on hot path until PR 4b-D.2 flip |
| `79999f39` | 05-15 | #427 | perf(engine): pre-resolve rule severity overrides | small | hot path (helper) | LOW (improvement) | none | Construction-time resolution caches |
| `df48b660` | 05-15 | #428 | feat(decoder): integrate documents corpus + position/lowercase context features | small | decoder hot path | LOW (decoder-only) | MED (new prior tables → bigger build-time embed in `marque-capco::priors`) | Affects `decoder_10kb`, not `lint_10kb` directly |
| `25da1c23` | 05-15 | #438 | perf(core): `scan_page_breaks` uses `memchr_iter` for newline stride | small | hot path | LOW (improvement) | none | Scanner Phase 1 |
| `8c8dce13` | 05-15 | #440 | fix(parser): recognize REL TO as same-category dissem sub-token | small | parser hot path | LOW | LOW | Parse correctness; small AC additions |
| `4b5110fd` | 05-15 | #442 | feat(vocab): add NNPI to NonIcDissem enum | small | construction | LOW | LOW | Generated-vocab addition |
| `8db7b3bf` | 05-15 | #443 | chore(corpus): canonicalize five classes of authoring misuses | — | none | none | none | Corpus only |
| `5f10f94f` | 05-15 | #441 | perf(core,ism): pre-size Scanner candidates SmallVec + PageContext portions Vec | small | hot path | LOW (improvement) | none | Allocation reduction |
| `49035c37` | 05-15 | #446 | perf(engine,scheme): eliminate `shift_token_spans` post-pass via Recognizer offset param | small | hot path | LOW-MED (improvement) | none | Removes a per-portion O(n) pass |
| `c6e25fae` | 05-15 | — | fix: broken bot actions | — | none | none | none | CI only |
| `deb6158d` | 05-15 | #445 | feat(parser): DISPLAY ONLY list-aware parsing + dedicated axis (Phase 1) | medium | parser hot path | LOW-MED | MED (~20 KB; new axis + display-only block lattice + tetragraph table consumer) | Constitution VIII §H.8 p163 |
| `2951ab8a` | 05-15 | #448 | perf(engine,rules,capco): narrow `catch_unwind` to untrusted rules via `Rule::trusted()` | small | hot path | LOW-MED (improvement; removes per-rule unwind table on trusted rules) | none | Hot-path improvement |
| `3d8025ab` | 05-16 | #457 | perf(engine,scheme): drop both `ParseContext` clones in `StrictOrDecoderRecognizer` | small | hot path | LOW-MED (improvement) | none | Recognizer optimization |
| `693602c1` | 05-16 | #459 | perf(ism): gate heavy vocabulary strings on `target_arch=wasm32` | small | none (native); WASM only | none | **HIGH** (WASM-only reduction) | Native unchanged; WASM cuts strings out |
| `0eb9b4bc` | 05-16 | #449 | feat(page_context): DISPLAY ONLY axis banner roll-up (Phase 2) | medium | hot path (banner roll-up) | LOW-MED | LOW-MED | Adds another expected_* accessor (pre-flip era) |
| `bf2f74b7` | 05-16 | #462 | test(corpus_parity): bump page-rewrite count pins 13 → 14 | — | none | none | none | Test pin |
| `c9d8ef29` | 05-16 | #437 | **PR 4b-B**: per-category Lattice impls + JOINT W004 + OC-USGOV+RELIDO bugfix | **+3531/-294** | construction-only at this point; bugfix lands in PageContext | LOW (still pre-flip) | **HIGH** (~60-150 KB; 7 new Lattice impls: ClassificationLattice/NatoClassLattice/JointSet/DissemSet/NatoDissemSet/RelToBlock/DeclassifyOnLattice; new W004 rule; per-axis trait monomorphizations) | The bulk of "10+ lattice types added in 4b-A/B"; this is the WASM-size pivot point |
| `30272511` | 05-16 | #463 | perf(engine): kill BTreeSet/Vec allocs + Cow-borrow decoder preprocessing | small | decoder hot path | LOW-MED (improvement, decoder) | none | Decoder allocation reduction |
| `1f135119` | 05-16 | #464 | perf(wasm): feature-gate toml chain out of marque-config | small | none (native); WASM only | none | **HIGH** (WASM-only reduction; toml + serde-toml chain dropped on WASM) | Native unchanged |
| `c1cee96f` | 05-16 | #465 | feat(core): accept SCI + FGI banner long-form titles | small | parser hot path | LOW | LOW | Lexer extension |
| `e53e4720` | 05-16 | #468 | **PR 4b-C**: Pattern B + Pattern C declarative PageRewrite migration + UCNI bugfix | +1135/-59 | construction-only at this point; bugfix lands in declarative catalog | LOW (still pre-flip; PageRewrites run at construction in scheduler but data table at runtime) | LOW-MED (9 new PageRewrite rows = ~static data) | Adds 9 PageRewrite rows; PageContext still primary at runtime |
| `320515f0` | 05-16 | #469 | perf(wasm): feature-gate jiff behind `dates` for marque-ism | small | none (native); WASM only | none | **HIGH** (WASM-only; jiff date chain dropped) | Native unchanged |
| `4a2ec30a` | 05-16 | #474 | fix(typos): behaviour -> behavior in date.rs | trivial | none | none | none | Typo fix |
| `7a2a04e8` | 05-16 | #473 | test(corpus): strict per-rule allowlist | — | none | none | none | Test only |
| `f416083d` | 05-16 | #475 | perf(engine): eliminate `add_portion` clone by consuming attrs at end-of-iteration | small | hot path | LOW-MED (improvement; saves a clone per portion) | none | Hot-path improvement |
| `e92c6a6a` | 05-16 | #476 | perf(engine): defer `parsed_markings` clone until a FixIntent fires | small | hot path (fix path) | LOW-MED (improvement; only `Engine::fix` benefits) | none | Lazy-clone |
| `d59ad22a` | 05-16 | #479 | refactor(capco): split scheme.rs into scheme/ module (Stage 1, #466) | structural | none | none | none | Module split, no semantics |
| `8f3cc5e4` | 05-16 | #478 | feat(server): Landlock process sandbox | — | none (server-only) | none | none | Out of core path |
| `6b1922d4` | 05-16 | #477 | fix(citation-lint): correct per-surface column arithmetic | small | none (tool only) | none | none | Tooling fix |
| `1741f356` | 05-16 | #480 | fix(capco): resolve clippy::module_inception in scheme/tests.rs | trivial | none | none | none | Lint fix |
| `574a0fb4` | 05-16 | #483 | refactor(capco): Stage 2 PR A — sub-split scheme/ leaf modules to <=800 LOC | structural | none | none | none | Module split |
| `c82f5865` | 05-16 | #484 | fix(capco): drop § sigil from §I-K prose to satisfy citation-lint | trivial | none | none | none | Doc fix |
| `b40cd1da` | 05-16 | #481 | perf(engine): swap parsed_markings HashMap for sorted Vec | small | hot path | LOW-MED (improvement) | none | Hot-path improvement |
| `71f83c92` | 05-16 | #486 | perf(engine): add advisory SCI-composite-dense bench | — | bench-only | none | none | New advisory bench |
| `0e456734` | 05-16 | #485 | refactor(capco): Stage 2 PR B — split scheme/mod.rs hub into per-section modules | structural | none | none | none | Module split |
| `43cac355` | 05-16 | #487 | **feat(engine,capco): Phase::PageFinalization + W004 fixpoint migration** | medium | **hot path (new fixpoint loop on page-finalization rules)** | **MED-HIGH** (each W004-emitting page now runs a fixpoint loop; bounded by rule-count but a new structural cost on the per-page loop) | LOW | Adds a new Phase variant; rules can now run after page aggregate |
| `94981070` | 05-16 | #491 | feat(capco): close #407 — expand Vocabulary sentinel set + bare-form rewriters (E067) | medium | construction + parser hot path | LOW-MED | LOW-MED (~10-20 KB sentinel tables) | New TOK_* sentinels + bare-form rewriters |
| `7f61496b` | 05-17 | #492 | refactor(capco): migrate S005 to Phase::PageFinalization + retire S006 | small | hot path (S005 now runs in finalization fixpoint) | LOW-MED | none | Net rule count -1 |
| `ec02b361` | 05-17 | #495 | test(capco): close #489 — JointSet proptest | — | none | none | none | Test only |
| `e344d21a` | 05-17 | #494 | fix(ism,core): tighten SAR shape predicates + narrow FGI ownership-token gate | small | parser hot path | LOW (improvement, narrower path) | none | Predicate refinement |
| `46900585` | 05-17 | #499 | fix(capco): close #439 — S004 stays silent when covered by REL TO | small | hot path (S004) | LOW | none | S004 silence cond |
| `d9354273` | 05-17 | #498 | docs(lattice),feat(engine): PageRewrite read-only-attrs invariant + portion-snapshot sentinel | small | hot path (PageRewrite scheduler adds a sentinel snapshot per rewrite-call) | LOW-MED (small overhead per scheduler invocation) | LOW | **Triggered the 2026-05-17 baseline re-capture** (PR #498, 828→914µs / 1011→1158µs). The PR's diff is small; the baseline jump was attributed in the commit message to *intervening* staging merges (PR 4b-B/C, decoder priors, parser additions), not to this PR itself |
| `3b7cbbfa` | 05-17 | #500 | test(engine): pin decoder-dispatch contract for SAR/FGI strict-parse rejections | — | none | none | none | Test only |
| `0930b779` | 05-17 | #502 | feat(scheme): split Lattice into JoinSemilattice + MeetSemilattice halves (#456) | medium | none (trait split, blanket impl marker) | LOW | LOW-MED (trait split changes monomorphization shape but `Lattice` stays a blanket-impl marker) | Trait surface change; semantics unchanged |
| `b130d93c` | 05-17 | #504 | fix(engine): gate rule dispatch on sub-threshold decoder parses | small | hot path | LOW | none | Decoder-gate fix |
| `11730fa0` | 05-17 | #503 | feat(lexer/E064): EYES ONLY banner-form support + bare-banner FVEY conversion | medium | parser hot path | LOW-MED | LOW-MED | Lexer extension |
| `2636eeed` | 05-17 | #506 | fix(ism): add FVEY constituent constants for E064 follow-up | small | construction | LOW | LOW | Static-data add |
| `b0e62f71` | 05-17 | #507 | refactor(capco): retire W002 us-fgi-comingling rule (#470) | small (net -) | hot path (-1 rule) | LOW (improvement) | LOW (improvement) | Registered rule count -1 |
| `183804fe` | 05-17 | #509 | feat(capco): per-variant classification sentinels + asymmetry repair (#505) | medium | hot path | LOW-MED | LOW-MED | Sentinel additions |
| `c9aa39a8` | 05-17 | #512 | fix(engine/bench): replace stale E001 invariant in fix_latency bench with E054 | — | bench-only | none | none | Bench fix |
| `5fa1a414` | 05-17 | #513 | fix(decoder): null-hypothesis gate over observed tokens for non-bare-class portions (#472) | medium | decoder hot path | LOW-MED (decoder) | none | Decoder gate |
| `b1c082a0` | 05-17 | #514 | **PR 4b-D.0**: ClosureRule generic + cone_derived | +184/-35 | construction-only (trait surface) | LOW | LOW-MED (~10-30 KB; new trait + 10 closure-rule rows pending wire-up) | Engine-gap; closure operator scaffolding |
| `b67220a5` | 05-17 | #516 | feat(engine): wipe Marque-owned content on drop (Constitution Principle II) | small | hot path (Drop impl on FixResult source) | LOW (Drop runs per call, zeroize is volatile-write) | LOW | New Drop impls; verify zeroize doesn't dominate hot path |
| `aeb31824` | 05-17 | #517 | **PR 4b-D.1**: closure operator runtime activation | +385/-119 | hot path (closure operator now runs per project call) | **MED** (~30-100µs; Kleene-fixpoint walk over 10 closure rows, gated by `closure short-circuit on empty cone`) | LOW-MED | Runtime activation; PM-confirmed the closure short-circuit is the architect's R-1 mitigation |
| `0342f9a2` | 05-17 | #519 | fix(capco): close CLOSURE_NOFORN_UCNI DOD-UCNI catalog gap | small | hot path (closure runs) | LOW | LOW | Catalog row |
| `ef7577bb` | 05-17 | #521 | fix(capco): close NNPI implicit-NOFORN closure gap + correct ISM doc | small | hot path (closure runs) | LOW | LOW | Catalog row |
| `ebbefda0` | 05-18 | #527 | **PR 4b-D.2**: hot-path flip (Engine::project + JoinSemilattice::join) | **+1357/-303** | **hot path (THE flip)** | **HIGH** (the flip itself; pre-flip ~914µs → post-flip WSL2 1033µs after 3 commits of optimization, was +65% unmitigated per PR description) | **MED** (new fast-path code, but PageContext stays alive until 4b-E) | **Highest-confidence individual driver**. Per-call work moves from PageContext's 13 `expected_*` accessors to `scheme.project → join_via_lattice (10 lattices) → closure → page_rewrites`. Self-reported: commit-6 closure-empty-cone short-circuit + commit-7 `project_from_attrs_slice` fast-path + commit-8 `project_from_page_context` PageContext-borrow brought +65% → +13% on WSL2. The post-PR-4b-D.2 bench shows the path is structurally ~50% more O(n) walks per call than the pre-flip path |
| `f188999a` | 05-18 | #529 | refactor(capco): collapse §4.7 Trio 1 closure rows into single CAVEATED row | small (net -) | hot path (closure runs) | LOW (improvement, -2 closure rows) | LOW (improvement) | Catalog consolidation |
| `f9e151d0` | 05-18 | #535 | **PR 4b-D.3**: consumer migration (S007 to ProjectedMarking) | +179/-20 | hot path (S007) | LOW-MED | LOW | S007 now reads `is_solely_nato_classified` projection |
| `b8754dde` | 05-18 | #536 | test(capco): pin FDR_DOMINATORS runtime suppression-matrix | — | none | none | none | Test only |
| `1b9ebfe4` | 05-18 | #537 | feat(capco): per-compartment SCI sentinels (#524 Phase 1) | medium | hot path (parser + closure) | LOW-MED | MED (~20-50 KB; new per-compartment sentinel const tables for HCS-O, HCS-P, etc.) | Per-compartment static data |
| `f9c91d31` | 05-18 | #538 | Proptest join-law audit | — | none | none | none | Test only |
| `ef7de07f` | 05-18 | #539 | **PR 4b-E**: PageContext expected_*/renderer deletion + 5 new lattice helpers | +1624/**-3808** | hot path (PageContext retires) | **MED-HIGH improvement** (~3457-line deletion of the 17 `expected_*` accessors + render_expected_banner + project + helpers; *should* recover headroom) | **MED-HIGH improvement** (large dead-code deletion) | Lifts the PageContext residue; scheme.project becomes the sole production path |
| `c3f544d6` | 05-18 | #540 | feat(capco): per-compartment SCI closure rows (#524 Phase 2) | medium | hot path (closure runs) | LOW-MED | MED (~10-30 KB; more closure rows + per-compartment data) | New rows in CAPCO_CLOSURE_RULES |
| `ed879a18` | 05-18 | #542 | **PR 4b-F**: retire `&PageContext` residue parameters + close PR-4 tasks | +245/-381 | hot path | LOW (improvement; signature cleanup) | LOW (improvement) | Net -136 LOC |
| `840700c3` | 05-18 | #544 | feat(capco): Trio 2 RELIDO closeout — implicit RELIDO on US collateral (#524 Phase 3) | medium | hot path (closure runs) | LOW-MED | LOW-MED | Closure-rule additions |
| `26e6c685` | 05-18 | #548 | feat(capco): add FISA / RAWFISA / PROPIN to CLOSURE_NOFORN_CAVEATED triggers | small | hot path (closure runs) | LOW | LOW | Closure-trigger row |
| `9b050680` | 05-18 | #549 | fix(capco): add TOK_EYES arm to apply_fact_remove(CAT_DISSEM) | small | hot path (intent apply) | LOW | LOW | Match-arm fix |
| `6fee9818` | 05-18 | #547 | **PR 6c**: PageContext struct retirement (T069) | +483/-675 | hot path | LOW-MED **improvement** (net -192 LOC; PageContext struct deleted; engine wraps CanonicalAttrs directly) | LOW-MED **improvement** | PageContext fully gone; `inline accumulator, delete page_context field` |
| `7d4ad231` | 05-18 | #550 | docs(capco,rules): PR 6c follow-up — Copilot R2 doc/comment drift fix-ups | trivial | none | none | none | Doc only |
| `9bdba9de` | 05-19 | #551 | fix(capco): SBU vanishes on classified portions per §H.9 p178 | medium | hot path (PageRewrite catalog) | LOW | LOW | New PageRewrite row |
| `e0ce3ec3` | 05-19 | #553 | **PR 5**: foreign banner correctness (E068 + E069 banner-rollup catalog rows, closes #276) | +309/0 | hot path (PageRewrite catalog) | LOW-MED (+2 rows in catalog) | LOW | E068/E069 |
| `5d3415cd` | 05-19 | #555 | fix(capco): SBU-NF/LES-NF supersede bare SBU/LES per §H.9 p178/p185 | medium | hot path | LOW | LOW | New PageRewrite rows |
| `82f5b477` | 05-19 | #556 | fix(capco): Pattern-A SBU-NF/LES-NF implies-NOFORN gates on classification | medium | hot path | LOW | LOW | PageRewrite predicate refinement |
| `c7a433d0` | 05-19 | #557 | chore(capco,docs): PR 4b umbrella closeout — attestation + drift pins + CI job | — | none | none | none | Doc + CI + drift pin |
| `0ffd30a0` | 05-19 | #558 | docs(specs/006): audit + flip 87 stale task checkboxes | — | none | none | none | Doc only |
| `81694384` | 05-19 | #560 | test(capco): PR 4 closeout — fill T116/T117/T117a/T118/T119 gaps + T119 probe | — | none | none | none | Test only |

---

## High-confidence drivers (likely top contributors to the regression)

Ranked by structural cost × confidence, not measured magnitude. The
implementation-phase profiling agent measures.

### Native `lint_10kb` drivers

1. **PR 4b-D.2 hot-path flip (`ebbefda0`, #527) — HIGH.** This is the
   single load-bearing structural change. Per-page work moved from
   PageContext's 13 `expected_*` accessors (each O(n_portions)) to
   `scheme.project(Scope::Page, ...) → join_via_lattice → closure
   → page_rewrites`. The PR description self-reports the path is
   structurally ~50% more O(n) walks per call than pre-flip; commits 6-8
   brought the gross +65% regression down to +13% on WSL2 dev, but the
   structural cost is intrinsic to the new pipeline shape. **PM precedent
   in CLAUDE.md "Recent Changes" PR 4b-D.2 entry confirms this is
   intentional architectural cost.**

2. **PR 4b-D.1 closure operator runtime activation (`aeb31824`, #517)
   — MEDIUM-HIGH.** Kleene-fixpoint walk over the closure-rule catalog
   (10 rules pre-4b-D.1, now ~14-16 with subsequent additions in PRs
   #519, #521, #529, #540, #544, #548). Even with the architect's R-1
   short-circuit on empty cone triggers, every page with any closure-
   triggering marking pays the fixpoint walk. **Compounds with the
   per-portion `add_portion` path** because closure rules read post-
   join lattice state.

3. **Cumulative PageRewrite catalog growth — MEDIUM (cumulative).** Pre-
   PR-4 had ~14 PageRewrite rows. Current HEAD has 27+ (per CLAUDE.md
   "Recent Changes" PR 4b closeout T144 entry: positional list of 27 IDs).
   The scheduler's per-call cost is O(rows + edges) and runs at
   construction only — but the per-page evaluation cost is O(rows × portions)
   when any row fires. Pattern-B/C strip rows (FOUO/UCNI/etc., 9 rows
   in PR 4b-C) and Pattern-A NOFORN-supremacy rows (4 rows pre-4b, +5
   in PR 5 and post-PR-4b fixes) all add evaluation cost.

### WASM-size drivers

1. **PR 4b-B per-category Lattice impls (`c9d8ef29`, #437) — HIGH.**
   7 new `JoinSemilattice + MeetSemilattice` impls (ClassificationLattice,
   NatoClassLattice, JointSet, DissemSet, NatoDissemSet, RelToBlock,
   DeclassifyOnLattice) plus their `*::from_attrs_iter` constructors,
   plus `CapcoMarking::join_via_lattice` composing them. Each lattice
   carries per-axis static data (variant tables, supersession overlays,
   constructor helpers). Per the user's "10+ lattice types added in
   4b-A/B" rough mental model — this is the bulk. Capco-src LOC delta
   for this PR alone is +3531 / -294.

2. **Vocabulary FormSet + sentinel tables — MEDIUM-HIGH (cumulative).**
   - PR 3d (`830b0f52`, #408): jumped wasm-size from initial baseline
     to 1,234,106 B — established the FormSet plumbing.
   - PR 9b (`48b2a3ce`, #417): bumped to 1,310,888 B (+~77 KB; dissem
     split into `dissem_us` + `dissem_nato`).
   - PRs #491 / #509 / #537 / #540: each adds sentinel const tables or
     per-compartment SCI data.
   - The PR #498 baseline jump to 1,386,447 B (+~75 KB) was attributed
     in the commit message to intervening staging-merge accumulation,
     not to PR #498's own diff.

3. **PR 4b-D.0 + 4b-D.1 closure-rule trait + catalog — MEDIUM.** The
   `ClosureRule<S>` generic trait surface (PR 4b-D.0) + 10 initial
   closure-rule rows (PR 4b-D.1) + subsequent expansions (#519, #521,
   #529, #540, #544, #548). Each row carries per-rule predicate +
   action closures (zero-size structs, but static data tables grow with
   row count).

### Improvements that should partly offset (potential headroom on a re-capture)

- **PR 4b-E (`ef7de07f`, #539)** — net **-3808 deletions** in engine src
  for the 17 `expected_*` accessor surface + render_expected_banner +
  project + helpers. *Should* reduce both native and WASM cost.
- **PR 4b-F (`ed879a18`, #542)** — `&PageContext` residue retirement,
  net -136 LOC.
- **PR 6c (`6fee9818`, #547)** — PageContext struct retirement, net -192
  LOC. Removes the entire pre-flip accumulator type.
- **WASM-specific feature gates** (PRs #459, #464, #469) — gated jiff /
  toml / heavy vocabulary strings out of WASM. **Should be HIGH WASM-only
  reduction**, not yet visible in the baseline.txt history because the
  next re-capture was during the PR-4b-D.2 era.

The user-reported ~1.7ms vs the PR-4b-D.2 WSL2 capture of ~1.03ms
suggests the post-PR-4b-D.2 + PR-5 + PR-6c additions (closure-rule
catalog growth, PageRewrite catalog growth, per-compartment SCI
sentinels, multiple Pattern-A/B/C rule additions) ate the headroom
PR 4b-E / 4b-F / 6c were *supposed* to recover. Profiling needs to
resolve this.

---

## Self-reported perf claims found in PR descriptions

Captured for cross-reference; the implementation-phase profiling agent
verifies. SHA refs to the commit where the claim lands.

| Source | Self-reported claim | Verification status |
|---|---|---|
| PR 4b-D.2 (`ebbefda0`, #527) commit message | Hot-path flip is structurally ~50% more O(n) walks per call than pre-flip path (10 lattice constructors + closure + page rewrites vs 13 `expected_*` accessors). Unmitigated +65%, post-3-commits +13% on WSL2 dev. | **Pinned in `benches/baseline.json` `_wsl2_dev_capture` sub-object: 1030/1033/1036µs.** Verify on GHA `ubuntu-latest` |
| PR 4b-D.2 commit 6 (`671aa560`, in #527) | Closure short-circuit on empty cone triggers — architect's R-1 mitigation | Verify magnitude of empty-cone vs non-empty-cone cost split |
| PR 4b-D.2 commit 7 (`6cfd4132`, in #527) | Engine-side `project_from_attrs_slice` fast-path | Verify call frequency |
| PR 4b-D.2 commit 8 (`5ea386ba`, in #527) | PageContext-borrowing `project_from_page_context` fast-path eliminates inner tmp_ctx rebuild's clone round | Verify |
| PR #498 (`d9354273`) commit message | The 828→914 / 1011→1158 baseline jump in this re-capture is attributed to *intervening* staging merges (PR 4b-B/C lattice landings, decoder priors #258/#259/#262, parser/recognizer additions), **not** to this PR's own diff | This is the load-bearing PM claim that PR 4b is the structural cost center, not the lattice trait split or decoder prior work. Verify via per-PR isolation profiling |
| PR 4b-D.0 (`b1c082a0`, #514) | Closure operator trait surface — pure scaffolding, no runtime cost | Verify (should be zero per-call cost) |
| PR 4b-D.1 (`aeb31824`, #517) | Closure operator runtime activation — Kleene-fixpoint walk gated by empty-cone short-circuit | Verify per-page fixpoint iteration count |
| PR 4b-E (`ef7de07f`, #539) | -3808 deletions; should retire residue tmp_ctx cost from PR-4b-D.2 | Verify net headroom recovery |
| PR 6c (`6fee9818`, #547) | PageContext struct entirely retired; engine inlines accumulator | Verify net delta from #539 |
| `benches/baseline.json::lint_10kb._note` | "Per the `project_perf_baseline_pr5_trigger.md` project memory, further perf-analysis work is scheduled for PR 5+ if structural costs survive the GHA re-capture; PR 4b-E will retire the remaining residue-axis tmp_ctx requirement, expected to bring the GHA value back down." | The user-reported ~1.7ms suggests this expectation has NOT held — net regression survived 4b-E + 4b-F + 6c. **First-priority profiling question** |

---

## WASM-size contributors specifically

Per the perf-engineer preflight §1.3, the production artifact uses
`wasm-opt -O3` and the user-reported ~1.6 MB may be measuring the
optimized artifact while `tools/wasm-size-baseline.txt` (1,386,447) is
pre-opt. The implementation agent must reconcile measurement basis.

### High-confidence WASM growth contributors (additive)

1. **PR 4b-B per-category Lattice impls (`c9d8ef29`, #437) — HIGH.**
   7 lattice types × {Join, Meet} = 14 trait impls + per-axis static
   data (variant payload union tables, supersession overlay vectors).
2. **PR 3d Vocabulary FormSet + Deprecation plumbing (`830b0f52`, #408).**
   First baseline pin at 1,234,106 B.
3. **PR 9b dissem split (`48b2a3ce`, #417) — MEDIUM.** +~77 KB at the
   baseline-pin boundary. `dissem_us` / `dissem_nato` axis split required
   per-axis CVE projections.
4. **PR 4b-D.0 + 4b-D.1 closure-rule trait + catalog (#514 + #517) —
   MEDIUM.** Trait surface + 10-row catalog + subsequent expansions
   (#519/#521/#529/#540/#544/#548). Each row = predicate + action
   closures.
5. **PRs #491, #509, #537, #540 sentinel + per-compartment SCI tables
   — MEDIUM (cumulative).** Per-compartment static `[u16]` token-ID
   tables for HCS-O / HCS-P / SI-G / RSV / TK-* compartments.
6. **PR 4a Vocabulary trait method (`fc91852e`, #422) — LOW.** Adds
   `is_fdr_dissem` method; impl reads `&'static` tables.
7. **PR 8 NATO decoder fold (`c3509bf7`, #415) — MEDIUM.** New decoder
   fold for NATO {level} longhand; expands the recognizer's match set.
8. **PR 9c.1 NATO retirement (`f249c033`, #418) — MEDIUM (cumulative).**
   Retired legacy NATO variants but added BOHEMIA/BALK in
   `SciControlSystem::NatoSap` + ATOMAL in `AeaMarking::Atomal`.

### WASM-specific size reductions (subtractive — not yet visible in baseline)

1. **PR #459 (`693602c1`) — HIGH.** Gates heavy vocabulary strings on
   `target_arch=wasm32`.
2. **PR #464 (`1f135119`) — HIGH.** Feature-gates toml chain out of
   marque-config on WASM.
3. **PR #469 (`320515f0`) — HIGH.** Feature-gates jiff date chain
   behind `dates` feature, off by default on WASM.

The next post-4b-E + 6c WASM baseline re-capture should show the
subtractive PRs' headroom against the additive growth. Quantifying
that net is an implementation-phase task.

---

## Open questions for implementation-phase profiling

In priority order, for the profiling agent to answer.

1. **(Highest)** Does the user-reported ~1.7ms measurement reproduce on
   GHA `ubuntu-latest`, or is it WSL2-specific drift? The
   `benches/baseline.json::lint_10kb` is pinned at 914µs (GHA) and
   1033µs (WSL2 PR 4b-D.2 capture); 1700µs is 1.85× the GHA pin and
   1.65× the WSL2 pin. Re-capture on both hosts to anchor.

2. **(Highest)** Did PR 4b-E + 4b-F + 6c (PageContext retirement, net
   -3808 deletions) recover the headroom the `_note` in
   `benches/baseline.json` *expected* them to? If yes, then the
   regression source is the post-PR-4b-D.2 catalog growth (closure
   rows, PageRewrite rows, per-compartment SCI sentinels). If no, then
   the structural ~50% O(n)-walk increase from the flip is the cost
   center and `scheme.project` itself needs profiling.

3. Per-PR isolation profiling for PR 4b-B vs PR 4b-D.1 vs PR 4b-D.2.
   The PR #498 commit message attributes the upward drift to "PR 4b-B/C
   lattice landings" specifically, but PR 4b-B/C are pre-flip and the
   lattices were only used in tests at that point. Either the
   attribution is approximate (the PR descriptions are sometimes
   intentionally diplomatic about which sub-PR ate the headroom), or
   there's pre-flip lattice cost we're not accounting for.

4. Cost of the closure-operator Kleene fixpoint per page: how many
   iterations on a typical 10KB document? The empty-cone short-circuit
   (PR 4b-D.2 commit 6) is the load-bearing mitigation; quantify its
   hit rate.

5. WASM size: separate native-vs-WASM-only allocations. The PR #459 /
   #464 / #469 feature gates should have produced HIGH WASM-only
   reductions; the baseline file hasn't been re-captured since, so
   the net is unknown. The user's ~1.6 MB report may include or exclude
   `wasm-opt -O3`; reconcile measurement basis.

6. PageRewrite catalog growth: the count went 14 → 27 across the PR 4b
   umbrella. Per-row evaluation cost on a representative page (1 portion,
   5 portions, 50 portions) — is the cost dominated by predicate
   evaluation, by action application, or by the topological scheduler's
   per-call dispatch?

7. Per-compartment SCI sentinel tables (PRs #537, #540): are these
   loaded into the hot path, or only consumed by specific rules? If the
   latter, their cost is bounded by which rules read them.

8. `Phase::PageFinalization` fixpoint loop (PR #487 / #492): how often
   does the page-finalization rule set run multiple iterations? If
   bounded by `W004` + `S005` only, the cost is small; if more rules
   migrate into PageFinalization in PR 5+, the per-page cost grows
   per-rule-added.
