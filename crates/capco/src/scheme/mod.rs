// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` — CAPCO's implementation of the `MarkingScheme` trait.
//!
//! This is the Phase A proof that CAPCO's hand-written aggregation in
//! [`PageContext`] falls out of the generic `marque-scheme` abstraction.
//! The adapter wraps `CanonicalAttrs` as `CapcoMarking`, implements
//! [`Lattice`] by delegating the join to `PageContext`'s existing
//! rollup, and exposes a minimal three-constraint sample to validate
//! that declarative constraints can reproduce existing rule behavior.
//!
//! The bulk of the migration — moving every CAPCO rule and replacing
//! `PageContext`'s internals — is Phase B/C work. The design doc
//! `docs/plans/2026-04-17-marking-scheme-lattice-design.md` sequences
//! the full migration.
//!
//! # Category identifiers
//!
//! CAPCO's categories are assigned small stable ids here. The specific
//! numbers are opaque — the engine only compares them for equality.
//! They're kept as constants so tests can reference them.

// Mod.rs re-imports the post-split common surface so the
// `use super::*` / `use super::super::*` glob in sibling modules and
// in `tests.rs` continues to find every identifier the pre-split
// monolithic `mod.rs` made available. After the Stage 2 PR B split,
// mod.rs itself uses only a small subset of these for its ID-constant
// declarations and re-exports; the rest are kept in scope so the leaf
// + test glob pattern stays one-line.
//
// `Classification` + a wide marque_scheme set are imported here even
// though mod.rs's own body only references `CategoryId` and `TokenId`
// (for the `CAT_*` / `TOK_*` constant declarations below). The leaf-
// glob pattern (`use super::super::*;` in `actions/`, `constraints/`,
// `predicates/`, `rewrites/`) and `tests.rs`'s `use super::*;` both
// pick up these names through the parent namespace. The
// `#[allow(unused_imports)]` attribute is required because the `lib`
// build of mod.rs alone doesn't see the leaf / test consumers and
// the compiler can't prove the imports are load-bearing from this
// vantage point — the `lib test` build does see them used. Both
// `clippy -- -D warnings` and the un-suffixed `lib` build need the
// allow.
#[allow(unused_imports)]
use marque_ism::Classification;
#[allow(unused_imports)]
use marque_scheme::{
    ApplyIntentError, CategoryAction, CategoryId, CategoryPredicate, ConstraintViolation, FactRef,
    Lattice, MarkingScheme, PageRewrite, ReplacementIntent, Scope, TokenId, TokenRef,
};

// ---------------------------------------------------------------------------
// Sibling-module declarations (issue #466)
// ---------------------------------------------------------------------------
//
// The body of the original monolithic `scheme.rs` was carved into sibling
// files in two stages:
//
//   Stage 1 (PR #479) — top-level lift into `actions.rs`, `constraints.rs`,
//   `predicates.rs`, `rewrites.rs`, `shared.rs`.
//
//   Stage 2 PR A (PR #483) — the four large leaves above sub-split into
//   per-axis directories (`actions/`, `constraints/`, `predicates/`,
//   `rewrites/`).
//
//   Stage 2 PR B (this PR) — `mod.rs` itself split into per-section
//   sibling files: `marking.rs` (the `CapcoMarking` type + impls + the
//   `join_via_lattice` lattice-path composer), `adapter.rs`
//   (`CapcoScheme` + ctors + `CapcoParseError` + the
//   `evaluate_named_constraint` / `fix_intent_by_name` /
//   `has_diagnostic_constraints` / `bridge_emitted_rule_ids` /
//   `bridge_sci_per_system_diagnostics` block),
//   `marking_scheme_impl.rs` (`impl MarkingScheme for CapcoScheme`),
//   `closure.rs` (`FDR_DOMINATORS` + the closure-rule catalog),
//   `render.rs` (`AxisRenderRow` + `RENDER_TABLE`), `class_floor.rs`
//   (the class-floor catalog), and `sci_per_system.rs` (the SCI
//   per-system catalog).
//
// After Stage 2, every sibling is ≤ 800 LOC and `mod.rs` is reduced to
// the hub of module declarations, public re-exports, and the `CAT_*` /
// `TOK_*` ID constants.

pub(crate) mod actions;
pub(crate) mod adapter;
pub(crate) mod class_floor;
pub(crate) mod closure;
// `closure_table` is `#[doc(hidden)] pub mod` (not `pub(crate)`)
// because integration tests in `crates/capco/tests/` need to reach
// `CLOSURE_TABLE` + `close` via the `marque_capco::closure_table::*`
// re-export at `lib.rs` — which requires the module be `pub` at this
// level for the re-export to compile. The `#[doc(hidden)]` keeps it
// out of rustdoc; signals "internal API, do not consume from outside
// the crate". PR-D wires the production consumer in
// `CapcoScheme::closure`; PR-F tightens visibility back to
// `pub(crate)` once integration tests migrate to observing through
// `MarkingScheme::closure`.
#[doc(hidden)]
pub mod closure_table;
pub(crate) mod constraints;
pub(crate) mod marking;
pub(crate) mod marking_scheme_impl;
pub(crate) mod predicates;
pub(crate) mod render;
pub(crate) mod rewrites;
pub(crate) mod sci_per_system;
pub(crate) mod shared;

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests;

// Public-within-crate re-exports for items that other crate modules
// (vocabulary.rs, rules_declarative.rs, lattice.rs, rules.rs) referenced
// at `crate::scheme::<name>` before the Stage-1 split. These re-exports
// preserve the pre-split paths so no external file needs to learn about
// the sibling-module layout.
pub(crate) use self::predicates::capco_token_category;
// `is_fdr_dominator` and `is_orcon_family` are public crate API per the
// original `scheme.rs` (pre-split visibility); `pub use` keeps them
// reachable at `marque_capco::scheme::is_fdr_dominator` for downstream
// callers that wire into PR 4 rule-wrapper dispatch.
pub use self::predicates::{is_fdr_dominator, is_orcon_family};

// Stage 2 PR B (issue #466) — re-exports for the new sibling modules
// carved out of the pre-split monolithic `mod.rs`. Each `pub use`
// preserves the canonical `marque_capco::scheme::<name>` path of every
// symbol that was reachable at that path before the split, AND keeps
// the leaf-glob pattern (`use super::super::*;` in `actions/`,
// `constraints/`, `predicates/`, `rewrites/`) finding the symbol by
// its established name. The split is purely structural — no public-API
// surface change.
pub use self::adapter::{CapcoParseError, CapcoScheme};
pub use self::marking::{CapcoMarking, CapcoOpenVocabRef};

// `FDR_DOMINATORS` and `CAPCO_CLOSURE_RULES` were `pub(crate)` /
// private respectively in the pre-split `mod.rs`. Re-export both into
// the parent namespace at `pub(crate)` so the `use super::super::*;`
// glob in PR-A leaf modules continues to find `FDR_DOMINATORS`, AND so
// `crate::scheme::FDR_DOMINATORS` continues to resolve for the
// `vocabulary.rs` consumer that hard-references it. `CAPCO_CLOSURE_RULES`
// stays at `pub(super)` in its sibling file but is re-bridged here so
// `marking_scheme_impl.rs` can name it via the parent module's glob.
pub(crate) use self::closure::FDR_DOMINATORS;
// Render dispatch surface — pulled into the parent namespace at
// `pub(crate)` so `marking_scheme_impl.rs`'s `use super::*;` glob (and
// the `predicates::dissem` consumer of `dissem_family_of` which keeps
// returning `DissemFamilyMembership`) keeps resolving them via the
// established `scheme::*` namespace.
pub(crate) use self::render::{DissemFamilyMembership, RENDER_TABLE};
// Class-floor catalog surface — every consumer reaches these through
// `crate::scheme::{ClassFloorRow, ClassFloorPolicy, CLASS_FLOOR_CATALOG}`
// today (the catalog type names are scanned by the test harness in
// `class_floor_catalog.rs`, and the predicates leaf reads
// `CLASS_FLOOR_CATALOG` directly via the leaf-glob pattern). Keep them
// reachable at the parent namespace path so neither stale.
pub(crate) use self::class_floor::{CLASS_FLOOR_CATALOG, ClassFloorPolicy, ClassFloorRow};
// SCI per-system catalog surface — `shared.rs` carries `impl CompanionForm`
// for the parent module's `CompanionForm` enum, and the
// `bridge_sci_per_system_diagnostics` body in `adapter.rs` references
// `SCI_PER_SYSTEM_CATALOG` directly. Both the actions and predicates
// leaves also reach `SciPerSystemRow` / `SciPerSystemKind` / `RULE_E059`
// through the leaf-glob pattern, so the catalog surface needs to live
// in the parent namespace.
pub(crate) use self::sci_per_system::{
    CompanionForm, RULE_E059, SCI_PER_SYSTEM_CATALOG, SciPerSystemKind, SciPerSystemRow,
};

// Post-Stage-2 PR B (issue #466) — the hub's own implementation bodies
// moved into sibling modules, but the pre-split mod.rs cross-sibling
// glob imports stay HERE because the in-tree `tests.rs` and the
// `super::super::*` glob in PR-A leaf modules were authored against
// the assumption that every `pub(crate)` item declared in
// `actions/` / `constraints/` / `predicates/` is reachable through
// mod.rs's namespace via the glob (Rust resolves `use super::*;` in a
// child by walking the parent's namespace, and plain `use foo::*;`
// brings each imported `pub(crate)` name into the parent at that
// visibility). Keeping the three globs intact preserves leaf + test
// glob-import resolution byte-for-byte; the hub's own impls no longer
// reference these symbols directly post-split. `#[allow(unused_imports)]`
// is required because the lib build proper of mod.rs alone doesn't
// track the leaf consumers — clippy `-D warnings` would otherwise
// reject the globs even though they're load-bearing.
#[allow(unused_imports)]
use self::actions::*;
#[allow(unused_imports)]
use self::constraints::*;
#[allow(unused_imports)]
use self::predicates::*;

// ---------------------------------------------------------------------------
// Category ids
// ---------------------------------------------------------------------------

pub const CAT_CLASSIFICATION: CategoryId = CategoryId(1);
pub const CAT_NON_US_CLASSIFICATION: CategoryId = CategoryId(2);
pub const CAT_JOINT_CLASSIFICATION: CategoryId = CategoryId(3);
pub const CAT_SCI: CategoryId = CategoryId(4);
pub const CAT_SAR: CategoryId = CategoryId(5);
pub const CAT_AEA: CategoryId = CategoryId(6);
pub const CAT_FGI_MARKER: CategoryId = CategoryId(7);
pub const CAT_DISSEM: CategoryId = CategoryId(8);
pub const CAT_REL_TO: CategoryId = CategoryId(9);
pub const CAT_DECLASSIFY_ON: CategoryId = CategoryId(10);
/// Non-IC dissemination controls (NODIS, EXDIS, SBU-NF, LES-NF, ...)
/// — backed by `CanonicalAttrs.non_ic_dissem`. Introduced in the PR
/// 3c.B engine-prereq commit so `MarkingScheme::apply_intent` can
/// route `FactRemove(EXDIS, Scope::Portion)` to the right axis
/// instead of silently no-opping (rust-reviewer preflight CRITICAL).
pub const CAT_NON_IC_DISSEM: CategoryId = CategoryId(11);

/// DISPLAY ONLY country list — backed by
/// `CanonicalAttrs.display_only_to`. Parallel to `CAT_REL_TO` for
/// the REL TO country list. Introduced in PR 4b-D.2 Copilot R1 #2
/// so the `capco/noforn-clears-display-only-to` PageRewrite can
/// declare `Clear { CAT_DISPLAY_ONLY_TO }` symmetrically with
/// `capco/noforn-clears-rel-to`'s `Clear { CAT_REL_TO }`.
///
/// Authority: §H.8 p163 (DISPLAY ONLY template, country-list axis);
/// §D.2 Table 3 rows 25-27 (DISPLAY ONLY banner roll-up; country-list
/// intersection mirrors REL TO).
pub const CAT_DISPLAY_ONLY_TO: CategoryId = CategoryId(12);

// ---------------------------------------------------------------------------
// Sentinel token ids for constraint expressions
// ---------------------------------------------------------------------------
//
// Phase C will replace these with generated ids pointing to specific
// CVE tokens. For Phase A we only need enough ids to express the three
// sample constraints that the equivalence tests exercise.

pub const TOK_NOFORN: TokenId = TokenId(100);
pub const TOK_JOINT: TokenId = TokenId(103);
pub const TOK_USA: TokenId = TokenId(104);

// Sentinel token ids for the Phase 3 declarative constraint catalog
// (T033). These identify specific tokens referenced by
// `Constraint::{Conflicts, Requires, Supersedes}` entries in the
// 12-rule migration set. Phase 4 replaces them with generated
// per-CVE-value ids; Phase 3 uses sentinels because the engine's
// `lint` path still consults hand-written rule impls as the
// authoritative diagnostic source, and the declarative constraint
// data here exists for scheme-exploration + Phase 4 decoder
// consumption — not (yet) for runtime evaluation.

pub const TOK_RESTRICTED: TokenId = TokenId(110);
pub const TOK_RD: TokenId = TokenId(111);
pub const TOK_FRD: TokenId = TokenId(112);
pub const TOK_TFNI: TokenId = TokenId(113);
pub const TOK_CNWDI: TokenId = TokenId(114);
pub const TOK_UCNI: TokenId = TokenId(115);
pub const TOK_HCS: TokenId = TokenId(116);
pub const TOK_FGI_MARKER: TokenId = TokenId(117);
pub const TOK_US_CLASSIFIED: TokenId = TokenId(118);
pub const TOK_IC_DISSEM: TokenId = TokenId(119);
pub const TOK_NON_IC_DISSEM: TokenId = TokenId(120);
pub const TOK_NON_US_CLASSIFICATION: TokenId = TokenId(121);

// T035c-21: NODIS / EXDIS sentinels for E037 (Conflicts) + E038
// (Requires NOFORN). Resolved via `satisfies_attrs` against
// `attrs.non_ic_dissem`, where the `NonIcDissem::Nodis` and
// `NonIcDissem::Exdis` variants live.
pub const TOK_NODIS: TokenId = TokenId(122);
pub const TOK_EXDIS: TokenId = TokenId(123);

// PR 3b.C (T026c): RELIDO incompatibility roster sentinels.
// Resolved via `satisfies_attrs` against `attrs.dissem_iter()`
// (the namespace-agnostic walk over `dissem_us ++ dissem_nato`,
// post PR 9b / FR-046 split) — all four tokens are IC dissem
// controls living in `marque_ism::DissemControl`.
//
// DissemControl variant → CVE string form (from generated values.rs):
//   Relido     → "RELIDO"
//   Displayonly → "DISPLAYONLY"
//   Oc         → "OC"      (ORCON portion abbreviation)
//   OcUsgov    → "OC-USGOV" (ORCON-USGOV portion abbreviation)
pub const TOK_RELIDO: TokenId = TokenId(124);
pub const TOK_DISPLAY_ONLY: TokenId = TokenId(125);
pub const TOK_ORCON: TokenId = TokenId(126);
pub const TOK_ORCON_USGOV: TokenId = TokenId(127);

// PR 3c.B Sub-PR 8.D.2 — REL TO whole-axis-clear sentinel.
//
// Resolved via `apply_fact_remove`'s CAT_REL_TO arm. Unlike `TOK_USA`
// (which removes only the USA entry from `attrs.rel_to`),
// `TOK_REL_TO` is a sentinel meaning "clear the entire CAT_REL_TO
// axis." E053 (NOFORN ⊥ REL TO, §H.8 p145) emits
// `FactRemove { FactRef::Cve(TOK_REL_TO), Scope::Portion }`; the
// per-country open-vocab removal channel will land alongside the
// `FactRef::OpenVocab` open-vocab country-removal Stage-4 sub-PR.
//
// The sentinel does NOT introduce a new category/axis in
// `capco_token_category` — CAT_REL_TO already exists (USA maps
// `TOK_USA → CAT_REL_TO`). `TOK_REL_TO` adds a second token routed
// to the same CAT_REL_TO category, and `apply_fact_remove`'s
// CAT_REL_TO branch discriminates between the two sentinels:
// `TOK_USA` removes only USA; `TOK_REL_TO` clears the whole axis.
pub const TOK_REL_TO: TokenId = TokenId(128);

// PR 3c.B Sub-PR 8.F.2 — SBU-NF and LES-NF Pattern A sentinels.
//
// These tokens route through `capco_token_category` to
// `CAT_NON_IC_DISSEM`, scanning `attrs.non_ic_dissem` for the
// `NonIcDissem::SbuNf` and `NonIcDissem::LesNf` variants
// respectively (declared at `crates/ism/src/attrs.rs:1163`/`:1168`).
//
// Used by the new `capco/sbu-nf-implies-noforn` (§H.9 p178) and
// `capco/les-nf-implies-noforn` (§H.9 p185) PageRewrites in
// `build_page_rewrites()` — Pattern A NOFORN-supremacy for SBU-NF
// and LES-NF. Mirrors the NODIS/EXDIS pair (`TOK_NODIS`, `TOK_EXDIS`)
// added in PR 3c.B Sub-PR 8.F.
pub const TOK_SBU_NF: TokenId = TokenId(129);
pub const TOK_LES_NF: TokenId = TokenId(130);

// Stage D (PR 3.7 T108c): Closure-rule catalog sentinels.
//
// These tokens are needed to express trigger and suppressor predicates in the
// §4.7 implicit-default trio and per-marking unconditional implication rows.
// All resolve via `satisfies_attrs` against the appropriate ISM attribute field.
//
// IC dissemination controls (DissemControl variants):
pub const TOK_IMCON: TokenId = TokenId(131); // CONTROLLED IMAGERY — §H.8 p142
pub const TOK_DSEN: TokenId = TokenId(132); // DEA SENSITIVE — §H.8 p159
pub const TOK_RSEN: TokenId = TokenId(133); // RISK SENSITIVE — §H.8 p132
pub const TOK_FOUO: TokenId = TokenId(134); // FOR OFFICIAL USE ONLY — §H.8 p134
// Non-IC dissemination controls (NonIcDissem variants):
pub const TOK_LIMDIS: TokenId = TokenId(135); // LIMITED DISTRIBUTION — §H.9 p170
pub const TOK_LES: TokenId = TokenId(136); // LAW ENFORCEMENT SENSITIVE — §H.9 p181
pub const TOK_SBU: TokenId = TokenId(137); // SENSITIVE BUT UNCLASSIFIED — §H.9 p176
pub const TOK_SSI: TokenId = TokenId(138); // SENSITIVE SECURITY INFORMATION — §H.9 p189
pub const TOK_EYES: TokenId = TokenId(139); // USA/[LIST] EYES ONLY — §H.8 p157
// (deprecated 2017-10-01 per §H.8 p157;
// parser preserves DissemControl::Eyes
// for legacy-input recognition).

// PR 4b-C Commit 1 (T112 OQ-1 Path A): vocab sentinels for Pattern B
// + future-decoder coverage. Each token is resolved by `satisfies_attrs`
// against the appropriate ISM attribute field; the
// `capco_token_category` table below routes them to the correct
// CategoryId. Routed AS-IF the §H.8 / §H.9 trigger family they
// belong to.
//
// PROPIN, FISA, RAWFISA live in `attrs.dissem_us` as the DissemControl
// variants `Pr`, `Fisa`, `Rawfisa` (per `crates/ism/src/attrs.rs`).
// Their CAPCO §-citations are §H.8 p148 (PROPIN) and §H.8 p161
// (FISA / RAWFISA); §H.8 p134 names them as "other dissemination
// control markings" that trigger FOUO eviction in UNCLASSIFIED
// docs (Pattern B). verified 2026-05-16 against CAPCO-2016.md.
pub const TOK_PROPIN: TokenId = TokenId(143); // PROPIN — §H.8 p148
pub const TOK_FISA: TokenId = TokenId(144); // FISA — §H.8 p161
pub const TOK_RAWFISA: TokenId = TokenId(145); // RAWFISA — ODNI `CVEnumISMDissem.xml` (post-CAPCO-2016; not in vendored manual)

// NNPI lives in `attrs.non_ic_dissem` as the NonIcDissem::Nnpi variant.
// NNPI has no confirmed CAPCO-2016 §-citation in ISM-v2022-DEC; the
// ODNI ISM `NonIcDissem::Nnpi` banner-roll-up doc-comment is the
// in-tree authority for NNPI's "propagates regardless of classification"
// behavior. The §H.8 p134 "other dissemination control markings" phrase
// is reasoning-by-analogy bridge prose, NOT a normative §-citation for
// NNPI — §H.8 p134 does not enumerate the token, and elevating the
// bridge phrase to a citation would constitute citation drift per
// Constitution VIII. For the closure-rule wiring see
// `crates/capco/src/scheme/closure.rs` — the row whose `name` field
// is `"capco/noforn-if-caveated"` (NNPI is one of its triggers; see
// the per-trigger authority table on the row's doc-comment).
// Closes issue #407. verified 2026-05-16.
pub const TOK_NNPI: TokenId = TokenId(146); // NNPI — non-IC dissem

// Issue #407 (PR feat/407): DOD UCNI sentinel disambiguation.
//
// DCNI lives in `attrs.aea_markings` as the `AeaMarking::DodUcni`
// variant per CAPCO-2016 §H.6 p116 (`DOD UNCLASSIFIED CONTROLLED
// NUCLEAR INFORMATION`, banner abbrev `DOD UCNI`, portion mark
// `DCNI`). Prior to this PR `TOK_UCNI` aliased both `AeaMarking::
// DodUcni` and `AeaMarking::DoeUcni`, which the vocabulary surface
// then collapsed onto the single canonical `"UCNI"` (DOE). Splitting
// the sentinel pair (`TOK_UCNI` → DOE-only, `TOK_DCNI` → DOD-only)
// lets `forms(TOK_DCNI)` resolve to the correct DCNI portion form
// and gives a per-system sentinel surface for any future class-
// floor / banner-roll-up rule that needs to distinguish the two
// agency variants.
//
// Routed via `capco_token_category` to `CAT_AEA` (mirrors
// `TOK_UCNI`). Pattern-C strip closures
// (`strip_dod_ucni_action` / `strip_doe_ucni_action` in
// `crates/capco/src/scheme/actions/strip.rs`) read the AEA axis
// directly via `AeaMarking::DodUcni` / `AeaMarking::DoeUcni`
// variant match and do NOT depend on sentinel identity, so adding
// `TOK_DCNI` is Pattern-C-neutral. verified 2026-05-16.
pub const TOK_DCNI: TokenId = TokenId(147); // DCNI — DOD UCNI portion form, §H.6 p116

// PR 9c.1 (T134): canonical NATO control-marking sentinels for
// ATOMAL / BALK / BOHEMIA. These tokens identify the new structural
// shapes added in `marque-ism` PR 9c.1 Commit 1:
//   - ATOMAL lives in the AEA axis as `AeaMarking::Atomal(AtomalBlock)`
//     per CAPCO-2016 §H.7 p122 worked example
//     `SECRET//RD/ATOMAL//FGI NATO//NOFORN`.
//   - BALK / BOHEMIA live in the SCI axis as
//     `SciControlSystem::NatoSap(NatoSap::{Balk,Bohemia})` per
//     CAPCO-2016 §G.2 p40 + §H.7 p127 worked example.
//
// All three render same-form across title / banner-abbrev / portion
// columns per §G.1 Table 4 p38 (the row "ATOMAL/BALK/BOHEMIA" lists
// the canonical name in all three columns).
//
// Resolved by `satisfies_attrs` against `attrs.aea_markings` and
// `attrs.sci_markings` respectively.
pub const TOK_ATOMAL: TokenId = TokenId(140);
pub const TOK_BALK: TokenId = TokenId(141);
pub const TOK_BOHEMIA: TokenId = TokenId(142);

// PR #505 (T112 follow-up): per-variant classification sentinels for the
// `MarkingClassification::{Nato,Fgi}(_)` arms. These complement
// `TOK_JOINT` (`TokenId(103)`) which already carries `Joint(_)` per-variant
// matching, and round out the classification-axis sentinel surface so
// `collect_present_tokens` emits a concrete `TokenRef::Token(...)` for
// every non-US classification variant.
//
// Disambiguation vs the existing tokens / categories:
//   - `TOK_NON_US_CLASSIFICATION` (`TokenId(121)`) and
//     `CAT_NON_US_CLASSIFICATION` (`CategoryId(2)`) match ALL of
//     `Fgi(_) | Nato(_) | Joint(_)` (the supercategory umbrella).
//   - `TOK_FGI_MARKER` (`TokenId(117)`) matches BOTH
//     `attrs.fgi_marker.is_some()` AND `Fgi(_)` (dual-axis), to support
//     family predicates that need FGI presence regardless of which axis
//     the FGI lives on.
//   - `TOK_NATO_CLASS` / `TOK_FGI_CLASS` (this block) match the
//     classification axis variant ONLY (strict per-variant).
//
// Reserved for the NATO closure cone deferred to #508 (PR 4b-D) and for
// any future `ConflictsWithFamily` row that needs strict-classification
// FGI/NATO match without the dissem-axis or umbrella shape.
//
// Routed as marker sentinels in `token_routing.rs::capco_token_category`
// (no addressable category — they label categorical predicates, not
// atomic addressable tokens). Mirrors `TOK_FGI_MARKER`'s routing.
pub const TOK_NATO_CLASS: TokenId = TokenId(148);
pub const TOK_FGI_CLASS: TokenId = TokenId(149);

// Issue #524 (Phase 1): per-compartment SCI sentinels.
//
// Six closed-CVE compound tokens addressing specific SCI
// system+compartment pairs that CAPCO-2016 §H.4 registers with their
// own marking templates and ARH read-in (§G.2 Table 5 p40, the
// "Conceptual ARH by Registered Marking" row block).
// The bare `TOK_HCS` sentinel (TokenId(116)) already matches any HCS
// compound via the structural `attrs.sci_markings` scan; these new
// sentinels resolve at finer compartment granularity so future
// per-marking unconditional implications (HCS-O ⇒ NOFORN+ORCON,
// SI-G ⇒ ORCON, TK-BLFH/KAND/IDIT ⇒ NOFORN) and per-compartment
// class-floor rules can address them without re-walking the SCI axis.
//
// All six resolve via `satisfies_attrs` against
// `attrs.sci_markings`: the system anchor must match
// `SciControlSystem::Published(SciControlBare::{Hcs,Si,Tk})` AND at
// least one entry in `marking.compartments` must carry the matching
// identifier (e.g., `"G"` for SI-G, `"BLFH"` for TK-BLFH). The
// structural shape — not `canonical_enum` — is the load-bearing
// witness so sub-compartmented forms (HCS-P with sub-compartments
// per §H.4 p68, TK-BLFH/KAND/IDIT with sub-compartments per §H.4
// p89/p93/p97) still resolve. `canonical_enum` is `None` whenever
// sub-compartments are present (see `SciMarking.canonical_enum`
// doc-comment at `crates/ism/src/attrs.rs`), so reading it would
// silently under-fire on the sub-compartment cases.
//
// Routed to `CAT_SCI` via `capco_token_category` (mirrors
// `TOK_HCS`/`TOK_BALK`/`TOK_BOHEMIA`). Phase 2 (issue #524 follow-up)
// will consume these as triggers of the per-marking unconditional
// implication rows; the sentinels land in Phase 1 because the
// introduction is itself a substantial change to the predicate /
// routing / vocabulary surface and merits its own review window.
//
// Authority (per §H.4 marking templates):
//   - TOK_SI_G — SI-GAMMA, §H.4 p80 (sub-compartment §H.4 p81)
//   - TOK_HCS_O — HCS-OPERATIONS, §H.4 p64
//   - TOK_HCS_P — HCS-PRODUCT, §H.4 p66 (sub-compartment §H.4 p68)
//   - TOK_TK_BLFH — TALENT KEYHOLE BLUEFISH, §H.4 p87
//     (sub-compartment §H.4 p89)
//   - TOK_TK_IDIT — TALENT KEYHOLE IDITAROD, §H.4 p91
//     (sub-compartment §H.4 p93)
//   - TOK_TK_KAND — TALENT KEYHOLE KANDIK, §H.4 p95
//     (sub-compartment §H.4 p97)
pub const TOK_SI_G: TokenId = TokenId(150);
pub const TOK_HCS_O: TokenId = TokenId(151);
pub const TOK_HCS_P: TokenId = TokenId(152);
pub const TOK_TK_BLFH: TokenId = TokenId(153);
pub const TOK_TK_IDIT: TokenId = TokenId(154);
pub const TOK_TK_KAND: TokenId = TokenId(155);

// Issue #524 (Phase 2): grammar-shape sentinel distinguishing HCS-P
// with at least one sub-compartment from bare HCS-P. Sub-compartments
// are open-vocabulary alphanumeric strings (up to 6 characters) per
// §H.4 p68, not a closed/registered set.
//
// CAPCO-2016 §H.4 p66 (bare HCS-P) shows the Example Banner Line
// `SECRET//HCS-P//NOFORN` (NOFORN only) while §H.4 p68 (HCS-P
// [SUB-COMPARTMENT]) shows `TOP SECRET//HCS-P JJJ//ORCON/NOFORN`
// (ORCON + NOFORN). The two markings carry different per-marking
// unconditional implications, so the closure operator needs to
// distinguish them at the trigger level. The Phase 1 sentinel
// `TOK_HCS_P` fires for both bare and sub-compartmented forms (it
// witnesses the HCS-P compartment), and per the structural-witness
// design that semantic is correct; `TOK_HCS_P_SUB` is the
// additional sentinel that fires only when HCS-P carries at least
// one sub-compartment.
//
// `TOK_HCS_P_SUB` is a **grammar-shape sentinel** (like
// `TOK_FGI_MARKER` and `TOK_JOINT`). It has no CVE-registered
// canonical because sub-compartments are open-vocabulary
// alphanumeric strings, not pre-registered compounds. It is
// deliberately excluded from `SENTINEL_TO_CANONICAL` and the
// `EXPECTED_FORMS` test catalog in `crates/capco/tests/
// vocabulary_forms.rs` — see the `canonical_for` panic message in
// `crates/capco/src/vocabulary.rs` for the active-sentinel
// admission contract.
//
// Routed to `CAT_SCI` via `capco_token_category`. Consumed by the
// `CLOSURE_HCS_P_SUB_IMPLIES_NF_OC` per-marking unconditional row in
// `crates/capco/src/scheme/closure.rs`.
//
// Authority: §H.4 p68 (HCS-P [SUB-COMPARTMENT] marking template).
// The Example Banner Line `TOP SECRET//HCS-P JJJ//ORCON/NOFORN`
// and the Notional Example Page (`TOP SECRET//HCS-P EFG//ORCON/NOFORN`
// — "originator controlled, and not releasable to foreign
// nationals") establish the per-marking ORCON+NOFORN implication
// for the sub-compartmented form.
pub const TOK_HCS_P_SUB: TokenId = TokenId(156);

// Issue #524 Phase 3: grammar-shape sentinel matching
// `MarkingClassification::Us(level)` AND
// `MarkingClassification::Conflict { us: level, .. }` where
// `level != Classification::Unclassified` — i.e., US collateral
// classification (Restricted / Confidential / Secret / TopSecret).
// Used as the trigger for `CLOSURE_RELIDO_US_CLASS` so the
// implicit-RELIDO closure is gated at the trigger level (not via
// an anti-monotone suppressor).
//
// CAPCO-2016 §H.8 p154 explicitly states "Explicit foreign
// disclosure and release markings are not required on unclassified
// information. Follow internal agency procedures for the use of
// RELIDO with unclassified information." Embedding the gate in the
// trigger (rather than a `TOK_US_UNCLASSIFIED` suppressor) keeps
// the closure rule monotone — `m1 ⊑ m2 ⇒ closure(m1) ⊑ closure(m2)`
// per the `MarkingScheme::closure` contract — because the trigger
// is an upward-closed predicate on attrs while a suppressor on
// "Us is Unclassified" would have made the rule anti-monotone in
// the same way the broader "no other dissem" qualifier did.
//
// Routed to `CAT_CLASSIFICATION` via `capco_token_category`.
// Resolves via `satisfies_attrs` against
// `attrs.us_classification().is_some_and(|l| l != Classification::Unclassified)`.
// Conflict-variant note: the predicate fires on Conflict markings
// whose US side is collateral classified — same trigger semantic
// as the pre-Issue-#524-Phase-3-revision design, but no longer
// reliant on a suppressor list to enforce the Unclassified
// carve-out. A future opt-in agency-style rule can re-enable
// U → RELIDO for organizations whose policy requires it (see
// follow-up issue tracked in the Phase 3 PR description).
pub const TOK_US_COLLATERAL_CLASSIFIED: TokenId = TokenId(157);
