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

use marque_ism::{CanonicalAttrs, Classification, MarkingClassification, PageContext, TokenKind};
use marque_scheme::{
    ApplyIntentError,
    Category,
    CategoryAction,
    CategoryId,
    // `FamilyPredicate` is referenced by `is_fdr_dominator` and
    // `is_orcon_family` (public free fns); the actual catalog rows
    // using ConflictsWithFamily were removed in PR 3.7 rev 3 per
    // Copilot review (see scheme.rs:2452 note). The fns remain as
    // public API for PR 4 to wire into the rebuilt rule-wrapper
    // dispatch when the enumerated E054-E057 rows retire.
    CategoryPredicate,
    ClosureRule,
    Constraint,
    ConstraintViolation,
    FactRef,
    Lattice,
    MarkingScheme,
    PageRewrite,
    Parsed,
    ReplacementIntent,
    Scope,
    Severity,
    Template,
    TokenId,
    TokenRef,
};

// ---------------------------------------------------------------------------
// Sibling-module declarations (issue #466 Stage-1 structural lift)
// ---------------------------------------------------------------------------
//
// The body of `scheme.rs` was split into seven sibling files per the plan at
// `claudedocs/refactor-466/split_proposal.md`. `mod.rs` keeps the type
// definitions, the `impl MarkingScheme for CapcoScheme` core, the
// `impl Lattice for CapcoMarking`, the constants tables (`CLASS_FLOOR_CATALOG`,
// `RENDER_TABLE`, `SCI_PER_SYSTEM_CATALOG`, `CAPCO_CLOSURE_RULES`,
// `FDR_DOMINATORS`), the `CapcoScheme::evaluate_named_constraint` /
// `fix_intent_by_name` / `has_diagnostic_constraints` /
// `bridge_emitted_rule_ids` / `bridge_sci_per_system_diagnostics` block,
// `render_canonical`, `CapcoMarking::join_via_lattice`, and the open-vocab
// reference enum. Sibling modules hold the supporting catalogs (rewrites,
// constraints), per-axis evaluators (predicates), and mutators (actions).
//
// **TODO #466 Stage 2**: every sibling except `shared` is still over the
// 800-line ceiling; the follow-up PR sub-splits per the plan §Risk 3:
//   - rewrites → rewrites/{pattern_a,pattern_b,pattern_c,...}.rs
//   - constraints → constraints/{class_floor,sci_per_system,rule_emitters}.rs
//   - predicates → predicates/{presence,triggers,satisfies,class_floor}.rs
//   - actions → actions/{intent,category_ops,companions,strip}.rs
//   - mod proper → split out impl MarkingScheme and CLASS_FLOOR_CATALOG.

pub(crate) mod actions;
pub(crate) mod constraints;
pub(crate) mod predicates;
pub(crate) mod rewrites;
pub(crate) mod shared;

#[cfg(test)]
mod tests;

// Public-within-crate re-exports for items that other crate modules
// (vocabulary.rs, rules_declarative.rs, lattice.rs, rules.rs) referenced
// at `crate::scheme::<name>` before the Stage-1 split. These re-exports
// preserve the pre-split paths so no external file needs to learn about
// the sibling-module layout.
pub(crate) use self::predicates::{capco_token_category, rel_to_covers};
// `is_fdr_dominator` and `is_orcon_family` are public crate API per the
// original `scheme.rs` (pre-split visibility); `pub use` keeps them
// reachable at `marque_capco::scheme::is_fdr_dominator` for downstream
// callers that wire into PR 4 rule-wrapper dispatch.
pub use self::predicates::{is_fdr_dominator, is_orcon_family};

// Re-imports of sibling-module items used by the impls below in this file.
// Glob-import everything `pub(crate)` from each sibling — mod.rs holds the
// hub impls (`impl MarkingScheme for CapcoScheme`, `impl Lattice for
// CapcoMarking`, the 323-LOC `impl CapcoScheme` block, plus
// `CapcoMarking::join_via_lattice` and the constants tables) and references
// helpers across every sibling. The glob keeps the cross-module surface
// honest: every callable item in a sibling is already `pub(crate)` per
// plan §Risk 4.
use self::actions::*;
use self::constraints::*;
use self::predicates::*;
// `self::rewrites` and `self::shared` are NOT glob-imported here:
//   - `rewrites::build_page_rewrites` is referenced explicitly inside
//     `CapcoScheme::new()` as `rewrites::build_page_rewrites()`.
//   - `shared` currently holds only `impl CompanionForm` (no free items
//     for mod.rs to import).
// Both module declarations remain `pub(crate) mod` so cross-sibling code
// can reach them; the glob omission keeps `cargo check` quiet.

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
pub const TOK_RAWFISA: TokenId = TokenId(145); // RAWFISA — §H.8 p161 (shares the FISA section)

// NNPI lives in `attrs.non_ic_dissem` as the NonIcDissem::Nnpi variant
// (per `crates/ism/src/attrs.rs:1326` doc-comment on NNPI). NNPI has
// no confirmed CAPCO-2016 §-citation in ISM-v2022-DEC; the ODNI ISM
// `attrs.rs:1326` banner-roll-up doc-comment is the in-tree authority
// for NNPI's "propagates regardless of classification" behavior, which
// makes NNPI a §H.8 p134 "other dissemination control markings"
// trigger by the same reasoning as SSI (§H.9 p189).
// Closes issue #407. verified 2026-05-16.
pub const TOK_NNPI: TokenId = TokenId(146); // NNPI — non-IC dissem

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

// ---------------------------------------------------------------------------
// CapcoMarking — newtype over CanonicalAttrs implementing Lattice
// ---------------------------------------------------------------------------

/// CAPCO marking as viewed through the `marque-scheme` lens. A thin
/// newtype around [`CanonicalAttrs`] so we can hang trait impls on it
/// without orphan-rule problems.
///
/// # ⚠️ Phase A scaffolding — do not use in production
///
/// `CapcoMarking` is exported publicly so the Phase A equivalence
/// tests can construct it, but it **does not uphold the [`Lattice`]
/// contract** on every input (see the caveat block on the `Lattice`
/// impl below). Downstream consumers must not rely on `Lattice::join`
/// / `Lattice::meet` of `CapcoMarking` producing law-abiding results
/// until Phase B replaces the impl with a proper product-lattice
/// aggregator. Use [`crate::capco_rules`] and `marque-core` directly
/// for production paths.
///
/// # Decoder provenance side channel (Phase 4 PR-4b)
///
/// Tuple-position 1 is an optional [`DecoderProvenance`] populated by
/// the Phase D probabilistic recognizer. Strict-path recognizers leave
/// it `None`. The engine reads `provenance.is_some()` to detect "this
/// recognition went through the decoder fallback" and emits a
/// synthetic `R001 decoder-recognition` diagnostic with
/// [`FixSource::DecoderPosterior`](marque_rules::FixSource::DecoderPosterior).
/// See [`crate::provenance`] for the side-channel contract.
///
/// `PartialEq` / `Eq` ignore tuple-position 1 — provenance is metadata,
/// not identity. Two markings with identical attrs but different
/// provenance traces compare equal.
#[derive(Debug, Clone)]
pub struct CapcoMarking(
    pub CanonicalAttrs,
    pub Option<crate::provenance::DecoderProvenance>,
);

impl PartialEq for CapcoMarking {
    /// Identity is the parsed attributes only — decoder provenance is
    /// audit metadata that does not participate in marking equality
    /// (see the type-level doc comment).
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for CapcoMarking {}

impl From<CanonicalAttrs> for CapcoMarking {
    #[inline]
    fn from(attrs: CanonicalAttrs) -> Self {
        Self(attrs, None)
    }
}

impl CapcoMarking {
    /// Construct a strict-path `CapcoMarking` (no decoder provenance).
    ///
    /// Convenience constructor that mirrors the pre-PR-4b tuple-struct
    /// literal `CapcoMarking(attrs)`. Use this in tests and
    /// strict-path recognizers; the decoder constructs the marking by
    /// setting tuple-position 1 directly when it has provenance to
    /// attach.
    #[inline]
    pub fn new(attrs: CanonicalAttrs) -> Self {
        Self(attrs, None)
    }

    /// **PR 4b-B Commit 7** — component-wise join via the per-category
    /// `marque-capco::lattice` types.
    ///
    /// This is the new "lattice path" exposed alongside the existing
    /// `Lattice::join` impl (which still delegates to `PageContext`).
    /// The parity-gate test
    /// `crates/capco/tests/page_context_lattice_parity.rs` (Commit 8)
    /// proves byte-identity between the two paths across 51 `#[test]`
    /// fixtures with **six documented divergences** (enumerated in
    /// `crates/capco/CAPCO-CONTEXT.md` §3): G-1 FOUO-classified, G-2
    /// AEA-UCNI-classified, G-3 pure-NATO, the
    /// RELIDO+NOFORN-dominates correctness divergence, plus the two
    /// pure-JOINT cases (`joint_unanimous_two_portions` /
    /// `joint_single_portion_no_us`) where the lattice produces
    /// `Joint(_)` per §H.3 p56 banner-fidelity and PageContext
    /// produces `Us(_)`. G-4..G-9 land as parity-RESTORING fixtures
    /// (each cited inline against its §). Corpus-fixture coverage
    /// is deferred to PR 4b-D when
    /// `CapcoScheme::project(Scope::Page, ...)` flips to use this
    /// path.
    ///
    /// **Two residues** preserved from PageContext for one more PR:
    ///
    /// 1. `non_ic_dissem` axis (classification-gated SBU-NF/LES-NF
    ///    split + the implied-NF injection family). Documented in
    ///    the plan §3.3 as a `Constraint::Custom("capco/fouo-eviction")`
    ///    PR 4b-C migration target. The `needs_nf` flag is propagated
    ///    into `out.dissem_us` (G-6 PR 4b-B follow-up) so SBU-NF /
    ///    LES-NF classified pages produce the correct NOFORN
    ///    injection on the lattice path.
    /// 2. The JOINT non-US producer FGI migration — Commit 5's
    ///    `JointSet::DisunityCollapse` carries the producer set,
    ///    and the W004 rule (Commit 9) surfaces it, but the
    ///    renderer-canonical FGI attribution is PR 5+ Stage 4
    ///    territory.
    ///
    /// Authority (verified 2026-05-15): per-axis citations are on
    /// each `lattice` module type's doc comment.
    pub fn join_via_lattice(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
        use crate::lattice::{
            AeaSet, ClassificationLattice, DeclassifyOnLattice, DissemSet, FgiSet, JointSet,
            NatoDissemSet, RelToBlock, SarSet, SciSet,
        };

        let mut out = CanonicalAttrs::default();

        // Page-composition introspection used by several axes below.
        // A page is "solely non-US" when it carries at least one
        // non-US classification AND no US-classification portion.
        // Per §H.7 pp123-125 reciprocal-raise: when ANY US portion is
        // present, NATO/FGI variants normalize to `Us(effective_level)`
        // at banner time; the non-US variant survives only when the
        // page has no US contribution at all. G-3 (PR 4b-B follow-up).
        //
        // G-9 + G-9b (PR 4b-B follow-up): three classification variants
        // are US-bearing for the purposes of the solely-non-US gate:
        //
        // - `Us(_)`: explicit US classification.
        // - `Conflict { us, .. }`: carries an implicit US classification
        //   in the `us` field (see `MarkingClassification::Conflict`
        //   doc comment at `crates/ism/src/attrs.rs:521`). The parser
        //   records "I saw two systems; US wins" — so Conflict is US
        //   from the gate's perspective. Pre-G-9 the lattice path
        //   returned `Conflict(...)` on a Conflict-only page (or
        //   `Nato(_)` on a Conflict+NATO page) while PageContext
        //   returned `Us(level)` — same authority, same §H.7
        //   reciprocal-normalization rule.
        // - `Joint(_)`: by §H.3 p56, USA is required to be in the
        //   producer list (JOINT is US co-owned by definition); JOINT
        //   classifications are therefore US-bearing for the gate.
        //   Pre-G-9b a mixed page like `JOINT C USA GBR + NATO S`
        //   kept `solely_non_us=true` (Joint not counted), so the
        //   NATO portion was preserved as `Nato(_)` rather than
        //   reciprocal-raising to `Us(_)` per §H.7 pp123-125. The
        //   same-level case is the load-bearing one: when the level
        //   chain doesn't already pick a winner via OrdMax, the
        //   variant survival in the per-portion filter loop produces
        //   the wrong banner shape.
        //
        // §-authority: §H.7 pp123-125 (reciprocal-classification rule)
        // + §H.3 p56 (JOINT requires USA in producer list). Verified
        // 2026-05-15 against CAPCO-2016.md.
        let mut has_us_class = false;
        let mut has_non_us_class = false;
        for p in portions {
            match &p.classification {
                Some(MarkingClassification::Us(_))
                | Some(MarkingClassification::Conflict { .. })
                | Some(MarkingClassification::Joint(_)) => has_us_class = true,
                Some(MarkingClassification::Fgi(_)) | Some(MarkingClassification::Nato(_)) => {
                    has_non_us_class = true
                }
                None => {}
            }
        }
        let solely_non_us = has_non_us_class && !has_us_class;

        // Axis 1: classification — variant-preserving OrdMax with
        // JointSet override. §H.1 pp47-54 + §H.7 pp123-125 +
        // §H.3 p57.
        //
        // Decision tree:
        // - JointSet::UnanimousProducers → banner is Joint(_,_) and
        //   ClassificationLattice's output is replaced.
        // - JointSet::DisunityCollapse → banner is Us(highest_level)
        //   from JointSet (non-US producers ride to FGI separately).
        // - JointSet::Mixed (JOINT + non-JOINT both seen, §H.3 p57)
        //   AND JointSet::Bottom (no JOINT portions) →
        //   ClassificationLattice wins, BUT any Joint(_) variants on
        //   per-portion classifications are flattened to their
        //   effective_level (Us) so the banner doesn't carry forward
        //   JOINT shape per §H.3 p57. G-3: in this non-JOINT branch,
        //   when the page is NOT solely-non-US, ALSO flatten
        //   Nato(_) / Fgi(_) variants to Us(effective_level) per the
        //   §H.7 pp123-125 reciprocal-raise — preserves PageContext
        //   parity on mixed US+NATO/FGI pages.
        let joint_set = JointSet::from_attrs_iter(portions);
        out.classification = match joint_set.to_marking_classification() {
            Some(mc) => Some(mc),
            None => {
                let filtered: Vec<CanonicalAttrs> = portions
                    .iter()
                    .map(|p| {
                        let mut q = p.clone();
                        match &p.classification {
                            // Always flatten JOINT to its US level in
                            // this non-JOINT branch (§H.3 p57).
                            Some(MarkingClassification::Joint(j)) => {
                                q.classification = Some(MarkingClassification::Us(j.level));
                            }
                            // §H.7 reciprocal-raise: NATO/FGI flatten
                            // to US level when ANY US portion is in
                            // scope. The solely-non-US case keeps the
                            // non-US variant intact.
                            Some(MarkingClassification::Nato(n)) if !solely_non_us => {
                                q.classification =
                                    Some(MarkingClassification::Us(n.us_equivalent()));
                            }
                            Some(MarkingClassification::Fgi(f)) if !solely_non_us => {
                                q.classification = Some(MarkingClassification::Us(f.level));
                            }
                            // G-9 (PR 4b-B follow-up): Conflict always
                            // flattens to its implicit `us` level in
                            // this non-JOINT branch. PageContext's
                            // `expected_classification` uses
                            // `effective_level()` over Conflict, which
                            // returns the `us` field, and wraps the
                            // result in `Us(_)`. The lattice path
                            // matches that semantic: Conflict is the
                            // parser's way of recording "I saw two
                            // classification systems; US wins per
                            // §H.7"; the foreign side rides separately
                            // through the FGI axis. Authority:
                            // CAPCO-2016 §H.7 pp123-125.
                            Some(MarkingClassification::Conflict { us, .. }) => {
                                q.classification = Some(MarkingClassification::Us(*us));
                            }
                            _ => {}
                        }
                        q
                    })
                    .collect();
                ClassificationLattice::from_attrs_iter(&filtered).into_inner()
            }
        };

        // Build a temporary PageContext for the axes that PR 4b-B
        // deliberately leaves on the PageContext path (see "two
        // residues" above) plus for the SCI compatibility view.
        let mut tmp_ctx = PageContext::new();
        for p in portions {
            tmp_ctx.add_portion(p.clone());
        }

        // Axis 2-5: SCI / SAR / AEA / FGI — assemble from per-portion
        // markings via the PR 4b-A precedent constructors. SciSet /
        // AeaSet take `&[Marking]` (flat per-portion union); SarSet
        // takes `Option<&SarMarking>`.
        let sci_markings_concat: Vec<marque_ism::SciMarking> = portions
            .iter()
            .flat_map(|p| p.sci_markings.iter().cloned())
            .collect();
        let sci_set = SciSet::from_markings(&sci_markings_concat);
        out.sci_markings = sci_set.to_markings();

        // Compatibility view: sci_controls is the flat CVE-enum
        // projection. The structural axis above is the authoritative
        // form; we re-derive sci_controls via the existing PageContext
        // shape so the parity gate compares both forms.
        out.sci_controls = tmp_ctx.expected_sci_controls().into_boxed_slice();

        // SAR: PR 4b-A SarSet operates on a single SarMarking
        // (`sar_markings` field is `Option<SarMarking>`). Join
        // across portions composes per-program by union.
        let mut sar_acc = SarSet::empty();
        for p in portions {
            let part = SarSet::from_marking(p.sar_markings.as_ref());
            sar_acc = sar_acc.join(&part);
        }
        out.sar_markings = sar_acc.to_marking();

        let aea_markings_concat: Vec<marque_ism::AeaMarking> = portions
            .iter()
            .flat_map(|p| p.aea_markings.iter().cloned())
            .collect();
        out.aea_markings = AeaSet::from_markings(&aea_markings_concat).to_markings();

        // FGI marker — compose via FgiSet from per-portion markers
        // AND merge with classification-derived producers
        // (PageContext::expected_fgi_marker unions NATO/JOINT/FGI
        // classification countries into the same axis).
        //
        // G-4 (PR 4b-B follow-up): when JointSet is
        // `UnanimousProducers`, the producers are already captured in
        // the JOINT classification — we must NOT also FGI-mark them,
        // because §H.3 p56 + §H.7 p123 say JOINT subsumes the FGI
        // marker for those producers.
        //
        // G-5 (PR 4b-B follow-up): when both an explicit FgiSet
        // marker AND classification-derived producers are present,
        // UNION the producer sets rather than discarding the
        // classification-derived ones.
        let mut fgi_acc = FgiSet::empty();
        for p in portions {
            let part = FgiSet::from_marker(p.fgi_marker.as_ref());
            fgi_acc = fgi_acc.join(&part);
        }
        let ctx_fgi_marker = if matches!(joint_set, JointSet::UnanimousProducers { .. }) {
            // G-4: JOINT-unanimous page — producers ride on the
            // `Joint(_)` classification, not on the FGI axis. Suppress
            // the PageContext FGI fallback so we don't double-mark
            // (§H.3 p56 + §H.7 p123).
            None
        } else if solely_non_us {
            // G-4b (PR 4b-B 7th-pass follow-up): solely-non-US page
            // where the lattice preserves a `Nato(_)` or `Fgi(_)`
            // classification intact (the §H.7 reciprocal-raise was
            // suppressed at scheme.rs:354 because there was no US
            // portion to raise toward). The foreign source is already
            // recorded on the classification axis itself; calling
            // `expected_fgi_marker()` here would derive the SAME
            // producers from the classification a second time and
            // surface them on the dissem-axis `fgi_marker`, producing
            // a doubled marker.
            //
            // PageContext doesn't have this problem because its
            // `expected_classification` ALWAYS wraps in `Us(_)`
            // regardless of source — the foreign-source info has to
            // ride on `expected_fgi_marker` since it can't ride on the
            // classification axis. The lattice path preserves the
            // foreign variant on the classification axis (per the
            // documented `pure_nato_lattice_vs_pagecontext_diverges`
            // divergence, §H.7 pp123-125), which makes the FGI-axis
            // duplication redundant.
            //
            // Per-portion `fgi_marker` fields (FgiSet) are still
            // honored — `fgi_acc.to_marker()` is what we ultimately
            // merge with this `None`. The suppression only drops the
            // classification-derived secondary fold.
            //
            // §-authority: §H.7 p123 (FGI source is recorded ONCE per
            // portion; for non-US classifications the source IS the
            // classification axis). Verified 2026-05-15 against
            // CAPCO-2016.md.
            //
            // G-4c (PR 4b-B 9th-pass follow-up): blanket suppression
            // is unsafe when the winner classification's foreign
            // payload is a STRICT SUBSET of all foreign sources
            // contributed by all non-US classification portions. The
            // failure mode:
            //
            //   Inputs:  Fgi(Confidential, [GBR]), Fgi(Secret, [CAN])
            //   ClassificationLattice winner: Fgi(Secret, [CAN])
            //     (OrdMax: Secret > Confidential)
            //   Pre-G-4c: GBR is silently lost from the FGI axis.
            //   PageContext path preserves both via its
            //   `expected_fgi_marker` union.
            //
            // The fix gathers the union of foreign sources from all
            // non-US classification portions, compares against the
            // winner's foreign sources, and:
            //   - if equal: safe to suppress (current G-4b behavior)
            //   - if winner is strict subset: build a synthetic FGI
            //     marker carrying the missing sources so they merge
            //     into `out.fgi_marker` via `merge_fgi_markers`.
            //
            // The C-7 `classification_join_same_variant` UNION
            // tiebreaker covers the same-level case (both producers
            // ride on the winner's payload, suppression remains
            // safe). G-4c only fires when level disagreement made
            // OrdMax discard a foreign source.
            //
            // §-authority: §H.7 p124 (source-concealed-dominance
            // precedence rules at the banner-line guidance block) +
            // §H.7 pp123-125 (FGI source must be preserved across
            // the projection) + §H.7 p128 (concealed-dominates
            // when mixed concealed + acknowledged portions exist).
            // Verified 2026-05-16 against
            // `crates/capco/docs/CAPCO-2016.md`.
            //
            // P-9-2 (9th-pass): `extract_foreign_sources` now returns
            // `Option<Vec<CountryCode>>` where `None` = source-concealed
            // FGI on that portion. If any portion is concealed, the page
            // must carry `FgiMarker::SourceConcealed` (§H.7 p128). Pre-
            // fix, source-concealed portions returned an empty Vec,
            // indistinguishable from "no FGI" — the equality check below
            // then silently dropped the concealed signal and could produce
            // a synthetic acknowledged marker.
            let any_concealed = portions
                .iter()
                .any(|p| extract_foreign_sources(p.classification.as_ref()).is_none());
            if any_concealed {
                // At least one portion is source-concealed → banner must
                // use bare `FGI` (no countries) per §H.7 p128.
                Some(marque_ism::FgiMarker::SourceConcealed)
            } else {
                let classification_sources: std::collections::BTreeSet<marque_ism::CountryCode> =
                    portions
                        .iter()
                        .flat_map(|p| {
                            extract_foreign_sources(p.classification.as_ref()).unwrap_or_default()
                        })
                        .collect();
                let winner_sources: std::collections::BTreeSet<marque_ism::CountryCode> =
                    extract_foreign_sources(out.classification.as_ref())
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                if winner_sources == classification_sources {
                    // G-4b safe-suppression branch: every foreign source
                    // observed across all portions is preserved on the
                    // winning classification's payload. No source loss.
                    None
                } else {
                    // G-4c source-loss branch: at least one source is
                    // missing from the winner's payload. Build a
                    // synthetic acknowledged FGI marker carrying every
                    // foreign source so `merge_fgi_markers` unions them
                    // into the final output.
                    marque_ism::FgiMarker::acknowledged(classification_sources)
                }
            }
        } else {
            tmp_ctx.expected_fgi_marker()
        };
        out.fgi_marker = merge_fgi_markers(fgi_acc.to_marker(), ctx_fgi_marker);

        // Axis 6-7: dissem_us / dissem_nato.
        // Build `dissem_us` as a `DissemSet` (rather than its
        // boxed-slice form) so cross-axis NOFORN injection below can
        // route through `DissemSet::with_noforn_injected` and have
        // the supersession overlay strip dominated controls per
        // §H.8 p145 (G-8 PR 4b-B follow-up).
        let dissem_set = DissemSet::from_attrs_iter(portions);
        out.dissem_nato = NatoDissemSet::from_attrs_iter(portions).into_boxed_slice();

        // Axis 8: rel_to.
        let rel_to_block = RelToBlock::from_attrs_iter(portions);
        let rel_to_was_noforn_superseded = rel_to_block.is_noforn_superseded();
        // P-2 (8th-pass): also capture the `Empty` variant (disjoint REL TO
        // country lists with no common [LIST] — §D.2 Table 3 row 9) BEFORE
        // `into_boxed_slice()` consumes the discriminant. An `Empty`
        // intersection means no common release audience exists, so the banner
        // MUST carry NOFORN per §D.2 Table 3 row 9.
        //
        // Pre-fix the NOFORN injection at line ~662 only checked
        // `rel_to_was_noforn_superseded` (the `NofornSuperseded` absorbing
        // state) and missed `Empty`. A page with two REL TO portions listing
        // disjoint countries produced an empty `rel_to` slice with no `Nf`
        // injected — wrong per §D.2 Table 3 row 9.
        //
        // §-authority: §D.2 p28-30 Table 3 row 9 (REL TO [USA, LIST] + REL
        // TO [USA, LIST] with no common [LIST] → NOFORN banner).
        // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
        let rel_to_was_empty_intersection = rel_to_block.is_empty_intersection();
        out.rel_to = rel_to_block.into_boxed_slice();

        // Axis 9: declassify_on (and declass_exemption rides as
        // last-observed per the existing PageContext semantic for
        // now — Phase 3 TODO at page_context.rs:639).
        out.declassify_on = DeclassifyOnLattice::from_attrs_iter(portions).into_inner();
        out.declass_exemption = tmp_ctx.expected_declass_exemption();

        // Residue 1: non_ic_dissem — classification-gated SBU-NF/
        // LES-NF split + implied-NF stays on PageContext for one
        // more PR (PR 4b-C migration target).
        //
        // G-6 (PR 4b-B follow-up): propagate `needs_nf` from
        // `expected_non_ic_dissem`. When set, inject NOFORN into
        // `dissem_us` AND clear REL TO — matches
        // PageContext::expected_dissem_us step 4 + the implicit
        // REL TO clear via §H.9 p178 (SBU-NF) / §H.9 p185 (LES-NF).
        // Pre-fix, the lattice path ignored this flag and a
        // classified page with REL TO + SBU-NF / LES-NF kept REL TO
        // and missed NOFORN.
        let (non_ic, needs_nf) = tmp_ctx.expected_non_ic_dissem();
        out.non_ic_dissem = non_ic.into_boxed_slice();

        // NOFORN-clears-REL-TO interaction + cross-axis NOFORN
        // injection.
        //
        // G-8 (PR 4b-B follow-up): when NOFORN must be injected from
        // a cross-axis source (non-IC SBU-NF/LES-NF on a classified
        // page, or NODIS/EXDIS supersession via RelToBlock), the
        // injection MUST route through `DissemSet::with_noforn_injected`
        // so the §H.8 p145 NOFORN-dominates overlay strips any
        // `Rel` / `Relido` / `Displayonly` that survived from the
        // per-portion union. Pre-G-8 the injection inserted `Nf`
        // into `out.dissem_us` directly, after `DissemSet::
        // into_boxed_slice` had already run — invalid output per
        // §H.8 p145.
        //
        // Authority: §H.8 p145 (NOFORN dominates REL TO / RELIDO /
        // EYES ONLY / DISPLAY ONLY) + §D.2 Table 3 rows 1-2 +
        // §H.9 p172 (NODIS) / §H.9 p174 (EXDIS) inject NOFORN at
        // banner.
        // P-2 (8th-pass): include the `Empty` intersection case alongside
        // `NofornSuperseded` — both require NOFORN injection per §D.2
        // Table 3 row 9 (Empty) and rows 1-2 / §H.9 p172/p174 (NofornSuperseded).
        let dissem_final =
            if rel_to_was_noforn_superseded || rel_to_was_empty_intersection || needs_nf {
                // G-6: SBU-NF / LES-NF on a classified page also clears
                // REL TO — match PageContext::expected_rel_to which
                // short-circuits to an empty slice when needs_nf fires.
                if needs_nf {
                    out.rel_to = Box::new([]);
                }
                dissem_set.with_noforn_injected()
            } else {
                dissem_set
            };
        out.dissem_us = dissem_final.into_boxed_slice();

        out
    }
}

// Phase B status note on the `Lattice` impl
// -----------------------------------------
//
// PR 4b-B (006 T112) installs per-category Lattice impls in
// `marque-capco::lattice` for every CAPCO axis (Classification,
// NatoClass, Joint, Dissem, NatoDissem, RelToBlock, DeclassifyOn,
// plus the PR 4b-A AeaSet / SciSet / SarSet / FgiSet). The
// component-wise composition is exposed on `CapcoMarking::
// join_via_lattice()` below — the new code path.
//
// The trait `Lattice::join` impl below STILL DELEGATES TO
// `PageContext::add_portion` + `page_context_to_attrs`. This is
// deliberate per the operative plan
// `docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md` §3.2:
// PR 4b-B installs the joins and the parity gate (Commit 8) proves
// byte-identity against the PageContext path. PR 4b-D flips the
// production hot path to use the lattice joins; until then, the
// PageContext delegation remains authoritative so the corpus +
// rule-set test surface stays bit-stable.
//
// Two residues for the eventual flip are documented inline in
// `join_via_lattice`:
//
// - `non_ic_dissem` axis — cross-axis classification-gated splits
//   (SBU-NF / LES-NF in classified docs) stay on PageContext for
//   one more PR. The §3 (b) FOUO eviction matrix migrates via
//   `Constraint::Custom("capco/fouo-eviction")` in PR 4b-C.
// - JOINT producer-disunity FGI migration — the `JointSet`
//   produces `DisunityCollapse` state with the non-US producer set;
//   the W004 Warn rule (registered Commit 9) surfaces it, but the
//   FGI-attribution rewrite is renderer-canonical territory
//   (PR 5+ Stage 4).
//
// `meet` keeps its narrow PageContext-free shape — it's used by a
// small set of overlap-check call sites that do not need full
// component-wise coverage. PR 4b-D widens it when `project` flips.
impl Lattice for CapcoMarking {
    /// Join = banner-aggregate both portions via `PageContext`.
    ///
    /// Delegates to [`PageContext`] so the scheme's join is
    /// definitionally equivalent to the existing hand-written
    /// aggregation on the inputs exercised by Phase A's tests. Phase B
    /// inverts this dependency — `PageContext` will be implemented in
    /// terms of component-wise aggregation, and this method will stop
    /// applying the projection's non-invertible normalizations.
    ///
    /// See the module-level "Phase A caveat" note above for the
    /// specific laws this impl does not satisfy.
    #[inline]
    fn join(&self, other: &Self) -> Self {
        let mut ctx = PageContext::new();
        ctx.add_portion(self.0.clone());
        ctx.add_portion(other.0.clone());
        CapcoMarking::new(page_context_to_attrs(&ctx))
    }

    /// Meet = partial component-wise minimum.
    ///
    /// Implemented only on classification, SCI, and dissem — enough to
    /// satisfy the trait bound and serve Phase A's test inputs. All
    /// other fields reset to `Default`. This is not a full
    /// product-lattice meet; see the module-level "Phase A caveat"
    /// note above. Phase B replaces it with a proper component-wise
    /// meet across every category.
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        let a = &self.0;
        let b = &other.0;

        let classification = match (&a.classification, &b.classification) {
            (Some(x), Some(y)) => {
                let min = x.effective_level().min(y.effective_level());
                Some(marque_ism::MarkingClassification::Us(min))
            }
            _ => None,
        };

        let sci: Vec<_> = a
            .sci_controls
            .iter()
            .filter(|t| b.sci_controls.contains(t))
            .copied()
            .collect();
        // PR 9b (T132): meet operates component-wise on each dissem
        // namespace independently. The two fields share the
        // `DissemControl` type but live on opposite sides of the
        // CAPCO-2016 p41 reciprocity boundary; mixing them would
        // collapse the namespace distinction.
        let dissem_us: Vec<_> = a
            .dissem_us
            .iter()
            .filter(|t| b.dissem_us.contains(t))
            .copied()
            .collect();
        let dissem_nato: Vec<_> = a
            .dissem_nato
            .iter()
            .filter(|t| b.dissem_nato.contains(t))
            .copied()
            .collect();

        let mut out = CanonicalAttrs::default();
        out.classification = classification;
        out.sci_controls = sci.into_boxed_slice();
        out.dissem_us = dissem_us.into_boxed_slice();
        out.dissem_nato = dissem_nato.into_boxed_slice();
        CapcoMarking::new(out)
    }
}

// ---------------------------------------------------------------------------
// CapcoScheme — the trait implementation
// ---------------------------------------------------------------------------

/// CAPCO's open-vocabulary structural reference.
///
/// Unifies the open-vocab carriers CAPCO ships today — SAR program
/// identifiers, SCI compartment and sub-compartment paths, and FGI
/// tetragraphs. `FactRef::OpenVocab(CapcoOpenVocabRef)` in
/// `marque-rules` names a token in the projected fact set by its
/// structural form, never by raw input bytes.
///
/// Each variant carries the *canonicalize-produced* structural value
/// (a SAR program ID value, a tetragraph code) — never source-buffer
/// surgery payloads. This preserves the G13 audit-content-ignorance
/// invariant (Constitution V Principle V): an `AppliedFix` referring
/// to a CAPCO open-vocab token stores a typed structural reference,
/// not document content.
///
/// PR 3c.B Commit 2 stubs the variant set with one nominal variant
/// per category. Construction sites (canonicalize-side population of
/// these references) land in Commit 6 alongside the rule migration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CapcoOpenVocabRef {
    /// A SAR program identifier (CAPCO-2016 §H.5).
    Sar(Box<str>),
    /// An SCI compartment name (CAPCO-2016 §A.6 / §H.4).
    SciCompartment(Box<str>),
    /// An SCI sub-compartment name (CAPCO-2016 §A.6 / §H.4).
    SciSubCompartment(Box<str>),
    /// An FGI tetragraph (CAPCO-2016 §H.3 / ISMCAT Tetragraph Taxonomy).
    FgiTetragraph(Box<str>),
    /// A REL TO country code or country-group (CAPCO-2016 §H.3 / §H.8).
    ///
    /// Carries the structural [`marque_ism::CountryCode`] value
    /// (16-byte fixed buffer, no heap) already produced by the parser,
    /// never raw input bytes — preserves the G13 audit-content-
    /// ignorance invariant (Constitution V Principle V). Wired by
    /// PR 3c.B Sub-PR 8.D.4 as the first open-vocab consumer of the
    /// CAT_REL_TO axis: E014 (JOINT participants require REL TO
    /// coverage, §H.3 p57) emits one `FactAdd { CountryCode(...),
    /// Scope::Portion }` per missing JOINT co-owner.
    CountryCode(marque_ism::CountryCode),
}

/// CAPCO's implementation of `MarkingScheme`.
///
/// Stateless; construct with `CapcoScheme::new()` and pass into the
/// engine. Phase A's engine doesn't consume the trait yet — this impl
/// exists so the equivalence tests can run.
///
/// A manual `Debug` impl is provided so generic types parameterized
/// over the scheme (`Diagnostic<S>`, `AppliedFix<S>`, `LintResult` /
/// `FixResult` inside `marque-engine`) can derive `Debug` via the
/// standard derive-macro field-bound expansion. The implementation
/// prints only the struct shell — the static-table fields are large
/// and not useful for debug output, and `PageRewrite<S>` does not
/// implement `Debug`.
pub struct CapcoScheme {
    categories: Vec<Category>,
    constraints: Vec<Constraint>,
    templates: Vec<Template>,
    page_rewrites: Vec<PageRewrite<CapcoScheme>>,
}

impl std::fmt::Debug for CapcoScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapcoScheme")
            .field("categories.len", &self.categories.len())
            .field("constraints.len", &self.constraints.len())
            .field("templates.len", &self.templates.len())
            .field("page_rewrites.len", &self.page_rewrites.len())
            .finish()
    }
}

impl Default for CapcoScheme {
    fn default() -> Self {
        Self::new()
    }
}

impl CapcoScheme {
    pub fn new() -> Self {
        Self {
            categories: constraints::build_categories(),
            constraints: constraints::build_constraints(),
            templates: Vec::new(), // Phase A does not model templates yet
            page_rewrites: rewrites::build_page_rewrites(),
        }
    }
}

impl CapcoScheme {
    /// Test-only constructor that lets tests install arbitrary
    /// `PageRewrite` entries, exercising the declarative dispatch
    /// path (`CategoryPredicate::Contains` / `Empty`,
    /// `CategoryAction::Clear` / `Replace` / `Intent`) with
    /// test-provided rewrites.
    ///
    /// Exposed publicly so integration tests under `crates/capco/tests/`
    /// can exercise scheme-level behaviors (page-rewrite projection,
    /// `CategoryAction::Intent` apply paths). Production code MUST NOT
    /// use this constructor — it bypasses `build_page_rewrites()`'s
    /// curated CAPCO-2016 table. The `_for_tests` suffix on the
    /// related [`with_extra_rewrite_for_tests`](Self::with_extra_rewrite_for_tests)
    /// helper makes the intent explicit.
    ///
    /// Bypasses `validate_intent_rewrites` (the engine's
    /// construction-time validation pass for `CategoryAction::Intent`
    /// payloads). Tests that want to exercise validation MUST feed
    /// the constructed scheme to `Engine::new` so the validation
    /// runs over the test rewrites.
    #[doc(hidden)]
    pub fn with_rewrites(rewrites: Vec<PageRewrite<CapcoScheme>>) -> Self {
        Self {
            categories: constraints::build_categories(),
            constraints: constraints::build_constraints(),
            templates: Vec::new(),
            page_rewrites: rewrites,
        }
    }

    /// Append one extra `PageRewrite` to a scheme's table, returning
    /// the modified scheme. Test-only — production code MUST NOT use
    /// this; the production rewrite table is the curated
    /// CAPCO-2016 table built by `build_page_rewrites()`.
    ///
    /// Bypasses `validate_intent_rewrites` (the engine's
    /// construction-time validation pass). Tests that want to exercise
    /// validation MUST construct the scheme separately and feed it to
    /// `Engine::new` so the validation runs over the appended rewrite.
    #[doc(hidden)]
    pub fn with_extra_rewrite_for_tests(mut self, rewrite: PageRewrite<CapcoScheme>) -> Self {
        self.page_rewrites.push(rewrite);
        self
    }
}

/// Parse errors surfaced by `CapcoScheme::parse`.
///
/// Phase A does not actually parse through the trait — callers continue
/// to use `marque_core::Parser` directly — so `parse()` unconditionally
/// returns [`CapcoParseError::NotImplemented`]. Phase B/E will wrap
/// `marque-core`'s `CoreError` here once parsing is routed through the
/// scheme trait (and the `(C)` ambiguity surface lands).
#[derive(Debug)]
pub enum CapcoParseError {
    /// `CapcoScheme::parse` is intentionally unimplemented in Phase A.
    /// Use `marque_core::Parser` for actual parsing until Phase B/E
    /// routes it through the scheme trait.
    NotImplemented,
}

impl CapcoScheme {
    /// Evaluate a single constraint by `name` against raw
    /// `CanonicalAttrs`. Fast path for rule wrappers that want "did
    /// this specific predicate fire?" without the overhead of a
    /// full `MarkingScheme::validate()` call.
    ///
    /// Compared to `scheme.validate(&CapcoMarking::new(attrs.clone()))`:
    /// - **No `CanonicalAttrs` clone** — works on the borrow directly
    /// - **No full catalog walk** — linear `find` by `name` over the
    ///   ~13 catalog entries, then single dispatch. O(1) effectively;
    ///   the filter step that the wrappers previously did after
    ///   `validate()` is eliminated.
    /// - **No `CapcoMarking` wrap** — delegates straight to the
    ///   free-function predicates (`satisfies_attrs`,
    ///   `evaluate_custom_by_attrs`), which is also what the trait
    ///   impls use.
    ///
    /// Contract: the emitted `ConstraintViolation.constraint_label`
    /// and `.citation` are populated from the catalog entry's
    /// declared `name` and `label`, matching the normalization that
    /// `marque_scheme::constraint::evaluate` performs in its
    /// `Custom` arm. Dyadic-variant violations carry a generic
    /// "conflicting tokens" / "token X requires Y" message — same
    /// as the generic evaluator — because the wrapper layer is
    /// responsible for constructing the user-visible diagnostic
    /// text, not the scheme.
    pub(crate) fn evaluate_named_constraint(
        &self,
        attrs: &marque_ism::CanonicalAttrs,
        name: &'static str,
    ) -> Vec<ConstraintViolation> {
        let Some(c) = self.constraints.iter().find(|c| c.name() == name) else {
            return Vec::new();
        };
        let label = c.label();
        match c {
            Constraint::Conflicts { left, right, .. } => {
                if satisfies_attrs(attrs, left) && satisfies_attrs(attrs, right) {
                    vec![ConstraintViolation {
                        constraint_label: name,
                        message: format!("conflicting tokens: {left:?} and {right:?}"),
                        citation: label,
                        span: None,
                        severity: None,
                    }]
                } else {
                    Vec::new()
                }
            }
            Constraint::Requires { left, right, .. } => {
                if satisfies_attrs(attrs, left) && !satisfies_attrs(attrs, right) {
                    vec![ConstraintViolation {
                        constraint_label: name,
                        message: format!("token {left:?} requires {right:?} but it is missing"),
                        citation: label,
                        span: None,
                        severity: None,
                    }]
                } else {
                    Vec::new()
                }
            }
            // `Supersedes` is a lattice hint for banner roll-up, not
            // a violation trigger. No diagnostic emission.
            // Note: `Constraint::Implies` was retired in PR 3.7 T108g
            // (decisions.md D19 C) — fact-propagation is handled by
            // the closure operator (ClosureRule) instead.
            Constraint::Supersedes { .. } => Vec::new(),
            // `ConflictsWithFamily` evaluates LHS-presence plus the
            // distributive expansion: emit one violation per token
            // present in `attrs` for which `family.0` holds. Mirrors
            // `marque_scheme::constraint::evaluate`'s
            // `ConflictsWithFamily` arm so wrapper-layer callers
            // (`rules_declarative.rs::violations_for`) get identical
            // diagnostics to the generic walker. Per Copilot PR 3.7
            // review: prior to this fix the fast path treated
            // `ConflictsWithFamily` as a no-op, silently dropping
            // every family-row diagnostic — that was a regression
            // the moment any wrapper dispatched by a family-row name.
            Constraint::ConflictsWithFamily { left, family, .. } => {
                if !satisfies_attrs(attrs, left) {
                    Vec::new()
                } else {
                    collect_present_tokens(attrs)
                        .into_iter()
                        .filter(|t| family.0(t))
                        .map(|present| ConstraintViolation {
                            // G13: `TokenRef` carries only integer IDs
                            // (`TokenId`/`CategoryId`), never document
                            // content bytes. Safe to format into the
                            // audit-stream message per Constitution V
                            // Principle V audit-content-ignorance.
                            constraint_label: name,
                            message: format!(
                                "conflicting tokens: {left:?} and {present:?} (family match)"
                            ),
                            citation: label,
                            span: None,
                            severity: None,
                        })
                        .collect()
                }
            }
            Constraint::Custom { .. } => evaluate_custom_by_attrs(attrs, name)
                .into_iter()
                .map(|mut v| {
                    v.constraint_label = name;
                    v.citation = label;
                    v
                })
                .collect(),
        }
    }

    /// Look up the [`FixIntent`] a catalog row produces against
    /// `attrs`, when one is defined.
    ///
    /// This is the engine-bridge counterpart to the scheme's
    /// [`MarkingScheme::validate`] path. The lint loop walks
    /// `scheme.validate(...)`, gets back a stream of
    /// [`ConstraintViolation`] values whose `span` and `severity` are
    /// populated by catalog rows that want to fire as user-facing
    /// diagnostics. For each such violation, the engine asks the
    /// scheme: *given this row name and these attributes, is there a
    /// `FixIntent` you'd like attached to the diagnostic?* For most
    /// rows the answer is `None`. For rows whose CAPCO §-citation
    /// commits to a specific repair shape (companion-insert for
    /// HCS-O / HCS-P sub / SI-G; subtractive for ORCON-USGOV conflict
    /// cases — see CAPCO-2016 §H.4 p64 / p66 / p68 / p80), the helper
    /// constructs the matching [`FixIntent`].
    ///
    /// # Why scheme-side, not on `ConstraintViolation`
    ///
    /// [`FixIntent<S>`] lives in `marque-rules`, and `marque-rules`
    /// depends on `marque-scheme` (Constitution VII Appendix D —
    /// post-PR-3c.A graph). Attaching a `fix_intent: Option<FixIntent<S>>`
    /// field to `ConstraintViolation` (in `marque-scheme`) would invert
    /// the graph and create a cycle. The bridge instead reconstructs
    /// the [`FixIntent`] from the row name on the way out — this is
    /// the side-table pattern the now-retiring walker rules
    /// (`DeclarativeClassFloorRule`, `DeclarativeSciPerSystemRule`)
    /// used internally; PR 3c.B Commit 7.4 relocates the table to the
    /// scheme so the walker can be deleted.
    ///
    /// # Cold-land contract (PR 3c.B Commit 7.2)
    ///
    /// This method returns `None` for every input in Commit 7.2; the
    /// only catalog rows that produce fixes today are E059's five
    /// SCI-per-system rows (companion-insert, HCS-O / HCS-P sub /
    /// SI-G; forbid-companion, HCS-P sub vs ORCON-USGOV). Those rows
    /// still fire diagnostics through the walker until Commit 7.4
    /// retires the walker and populates this helper. `None` is the
    /// safe shape — the engine attaches no fix and the diagnostic
    /// flows through unchanged. No behavior change at 7.2; the only
    /// purpose of the method's existence here is to give the engine
    /// bridge a stable scheme-side entry point to query.
    pub fn fix_intent_by_name(
        &self,
        _name: &str,
        _attrs: &CanonicalAttrs,
    ) -> Option<marque_rules::FixIntent<CapcoScheme>> {
        // PR 3c.B Commit 7.4 will populate the E059 catalog rows here.
        // Until then, the walker rule `DeclarativeSciPerSystemRule`
        // owns the E059 fixes via its own side-table.
        None
    }

    /// Reports whether the scheme's `Constraint::Custom` catalog has
    /// any rows that *can* produce user-facing diagnostics (i.e., rows
    /// whose `evaluate_custom` arm populates `ConstraintViolation::span`
    /// AND `::severity`). Used by the engine's constraint-catalog
    /// bridge (`crates/engine/src/engine.rs` lint loop) to short-
    /// circuit the whole `scheme.validate(...)` walk — including the
    /// per-candidate `CapcoMarking::from(attrs.clone())` allocation —
    /// when no catalog row could possibly fire.
    ///
    /// # Why a static `true` now (PR 3c.B Commit 7.3)
    ///
    /// PR 3c.B Commit 7.3 retired `DeclarativeClassFloorRule` (E058)
    /// and rewired its 27 class-floor catalog rows to populate
    /// `ConstraintViolation::span` (via [`class_floor_anchor_span`])
    /// and `::severity` (from `ClassFloorRow::severity`) directly in
    /// [`class_floor_emit`]. The bridge is the sole emitter for the
    /// class-floor rule set as of this commit; the previous walker
    /// path no longer exists. PR 3c.B Commit 7.4 retired
    /// `DeclarativeSciPerSystemRule` (E059) via a separate
    /// direct-path mechanism — `bridge_sci_per_system_diagnostics`
    /// — that does NOT participate in the `validate()` /
    /// `ConstraintViolation` envelope flow (decision record
    /// Amendment 6). The 5 SCI per-system rows therefore do not
    /// contribute to this predicate's value; it stays `true`
    /// because the 27 class-floor rows from 7.3 already require
    /// the bridge walk.
    ///
    /// # Why static (not derived from the catalog at runtime)
    ///
    /// Catalog membership doesn't change across the engine's
    /// lifetime — `build_constraints()` is invoked once at
    /// `CapcoScheme::new()` and never mutated. A runtime walk over
    /// `self.constraints` to look for "any Custom row that produces
    /// span/severity" would itself defeat the optimization (the data
    /// we're avoiding fetching is the per-candidate walk's output;
    /// learning that the catalog has zero such rows shouldn't itself
    /// require a per-candidate walk). The constant `true` here
    /// reflects the post-7.3 catalog state and is a one-line override
    /// for any future scheme that wires no diagnostic-shape rows.
    pub fn has_diagnostic_constraints(&self) -> bool {
        true
    }

    /// Rule IDs emitted by the engine's constraint-catalog bridge that
    /// do not correspond to any registered `Rule::id()`. Each entry is
    /// a `(rule_id, name)` pair shaped to match the existing
    /// `Rule::additional_emitted_ids()` walker convention so the
    /// engine's `canonicalize_rule_overrides` validator can accept
    /// `.marque.toml [rules] <id-or-name> = "off"` references to
    /// these IDs without an `UnknownRuleOverride` failure.
    ///
    /// # User-facing surface
    ///
    /// Both fields are user-facing config keys: `canonicalize_rule_overrides`
    /// inserts the `rule_id` and the `name` into the known-key map,
    /// aliasing both to the canonical ID. A `.marque.toml` entry
    /// `[rules] class-floor-catalog = "off"` is therefore silently
    /// accepted as an alias for `[rules] E058 = "off"`. The shorter
    /// `E058` form is the recommended one (matches what `Diagnostic.rule`
    /// emits, what audit-stream consumers see, and what `did_you_mean`
    /// suggests for typos); the longer name is the descriptive alias
    /// users discovering rule IDs in source might also reach for.
    /// This convention parallels the `id-or-name` aliasing every
    /// registered `Rule` already accepts.
    ///
    /// # Entries (PR 3c.B Commit 7.3)
    ///
    ///   - `("E058", "class-floor-catalog")` — retired
    ///     `DeclarativeClassFloorRule` walker. The 27 class-floor
    ///     catalog rows fire through the bridge with
    ///     `Diagnostic.rule = "E058"`; the bridge folds the per-row
    ///     `E058/...` / `class-floor/...` constraint-label names to
    ///     this collapsed ID.
    ///
    /// PR 3c.B Commit 7.4 added `("E059", "sci-per-system-catalog")`.
    pub fn bridge_emitted_rule_ids(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("E058", "class-floor-catalog"),
            ("E059", "sci-per-system-catalog"),
        ]
    }

    /// Walk the SCI per-system catalog and return one `Diagnostic` per
    /// firing emit-branch, with the row's `FixProposal` attached
    /// (matching the retired `DeclarativeSciPerSystemRule` walker's
    /// output byte-for-byte).
    ///
    /// # Why this bypasses the `ConstraintViolation` envelope
    ///
    /// The class-floor catalog (PR 3c.B Commit 7.3) emits diagnostics
    /// through the standard `MarkingScheme::validate()` →
    /// `Vec<ConstraintViolation>` → engine bridge path because its
    /// rows produce no fixes (every class-floor violation requires
    /// human review). The SCI per-system catalog rows DO produce
    /// fixes — companion-insertion at the dissem-block anchor and
    /// `ORCON-USGOV → ORCON` token replacement — and a single row
    /// can emit multiple diagnostics (HCS-O missing ORCON AND
    /// missing NOFORN → 2 violations, each with its own fix).
    ///
    /// `ConstraintViolation` (in `marque-scheme`) cannot carry a
    /// `FixProposal` (in `marque-rules`) because `marque-scheme` is
    /// the workspace dependency-graph leaf (Constitution VII). A
    /// `fix_intent_by_name(name, attrs)` helper called per
    /// `ConstraintViolation` cannot disambiguate "which of N
    /// violations on this row do I synthesize a fix for" with only
    /// `(name, attrs)` as input. Rather than thread message text
    /// through the bridge for disambiguation, the SCI per-system
    /// rows take the direct path: this method returns full
    /// `Diagnostic` values straight from `sci_per_system_emit`, the
    /// engine bridge invokes it once per candidate (gated on
    /// `[rules] E059 != "off"`), and the existing fix-promotion
    /// path treats each diagnostic identically to a registered
    /// `Rule` impl's output.
    ///
    /// # Severity override handling
    ///
    /// The caller passes the resolved `Severity` for `E059`
    /// (`severity_override` = the `[rules] E059 = ...` config, or
    /// `None` to use each diagnostic's authoring severity). When
    /// `severity_override = Some(Severity::Off)` the method returns
    /// an empty `Vec` (FR-008: an `Off`-severity diagnostic is
    /// unrepresentable). A non-`Off` override replaces the per-
    /// diagnostic severity uniformly.
    pub fn bridge_sci_per_system_diagnostics(
        &self,
        attrs: &CanonicalAttrs,
        candidate_span: marque_ism::Span,
        fix_scope: marque_scheme::Scope,
        severity_override: Option<marque_rules::Severity>,
    ) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
        // FR-008 early-out — `Off` suppresses the entire catalog.
        if matches!(severity_override, Some(marque_rules::Severity::Off)) {
            return Vec::new();
        }
        // Hot-path early-out — every SCI per-system row is SCI-axis-
        // only. If no SCI markings are present, no row can fire and
        // the catalog walk costs effectively nothing. Mirrors the
        // retired walker's `attrs.sci_markings.is_empty()` guard.
        if attrs.sci_markings.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for row in SCI_PER_SYSTEM_CATALOG {
            if !(row.presence)(attrs) {
                continue;
            }
            for mut diag in sci_per_system_emit(attrs, candidate_span, fix_scope, row) {
                if let Some(sev) = severity_override {
                    diag.severity = sev;
                }
                out.push(diag);
            }
        }
        out
    }
}

// T035 (2026-04-21): `satisfies` and `evaluate_custom` are now
// implemented on `CapcoScheme`, so calling
// `marque_scheme::constraint::evaluate(&CapcoScheme::new(), &m)`
// (or equivalently `scheme.validate(&m)` via the trait default)
// fires every dyadic and Custom constraint in the catalog.
//
// The 11 hand-written rule impls retired by T035 dispatch through
// `crate::rules_declarative`, which uses the inherent fast-path
// method `CapcoScheme::evaluate_named_constraint` above (not the
// trait-path `validate`) and constructs `Diagnostic` values
// locally for byte-identical message/span/fix output. E018 / E019
// remain hand-written pending the T035b predicate audit.
impl MarkingScheme for CapcoScheme {
    type Token = marque_scheme::TokenId;
    type Marking = CapcoMarking;
    type ParseError = CapcoParseError;
    type OpenVocabRef = CapcoOpenVocabRef;

    fn name(&self) -> &str {
        "CAPCO-ISM"
    }

    fn schema_version(&self) -> &str {
        crate::SCHEMA_VERSION
    }

    fn categories(&self) -> &[Category] {
        &self.categories
    }

    fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    fn templates(&self) -> &[Template] {
        &self.templates
    }

    fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        // Phase A: the trait impl exists to validate the abstraction's
        // shape against CAPCO. Callers continue to use
        // `marque_core::Parser` directly. Phase B/E tie parse() into
        // the engine once the ambiguity resolver lands.
        Err(CapcoParseError::NotImplemented)
    }

    /// Resolve a [`TokenRef`] against a `CapcoMarking`'s concrete
    /// storage. Drives the dyadic-variant arms of
    /// [`marque_scheme::constraint::evaluate`].
    ///
    /// **Token-presence semantics** (T035):
    /// - [`TokenRef::Token(id)`] returns true when the marking carries
    ///   the named token *anywhere* relevant — `TOK_USA` ⇒ "USA in
    ///   REL TO" (the dissemination context), `TOK_RD` ⇒ "RD anywhere
    ///   in `aea_markings`", etc. The mapping is per-sentinel and
    ///   documented inline below.
    /// - [`TokenRef::AnyInCategory(cat)`] returns true when the
    ///   category has at least one populated value. `CAT_DISSEM`
    ///   intentionally counts both the dissem axis (`dissem_us` and
    ///   `dissem_nato` together, walked via `attrs.dissem_iter()`
    ///   post PR 9b / FR-046 split) AND `rel_to` as dissem-flavored
    ///   presence, matching the historical E015
    ///   predicate ("non-US classification needs SOME dissem").
    ///
    /// Sentinel `TokenId`s not used by the current catalog
    /// (`TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`) fall through to `false`
    /// — they remain declared for future T035b consumption when the
    /// E018/E019 catalog entries are added back with corrected
    /// predicates. Categories not listed (none today) likewise fall
    /// through.
    /// Resolve a [`TokenRef`] against a `CapcoMarking`'s concrete
    /// storage. Drives the dyadic-variant arms of
    /// [`marque_scheme::constraint::evaluate`] when callers go through
    /// the trait path; the free-function `satisfies_attrs` below is
    /// the authoritative implementation.
    ///
    /// See `satisfies_attrs` for the full sentinel-to-predicate
    /// table.
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        satisfies_attrs(&marking.0, token_ref)
    }

    /// Map a [`FactRef`] to its [`CategoryId`].
    ///
    /// Closed-CVE sentinels in the current constraint catalog get
    /// explicit mappings; open-vocab references route by variant.
    /// Tokens not in the table return `None`, signaling
    /// [`ApplyIntentError::UnknownToken`] when the engine asks
    /// `apply_intent` to route them.
    fn category_of(&self, token: &FactRef<Self>) -> Option<CategoryId> {
        match token {
            FactRef::Cve(id) => capco_token_category(*id),
            FactRef::OpenVocab(r) => Some(match r {
                CapcoOpenVocabRef::Sar(_) => CAT_SAR,
                CapcoOpenVocabRef::SciCompartment(_) | CapcoOpenVocabRef::SciSubCompartment(_) => {
                    CAT_SCI
                }
                CapcoOpenVocabRef::FgiTetragraph(_) => CAT_FGI_MARKER,
                // PR 3c.B Sub-PR 8.D.4 — open-vocab REL TO country codes
                // route to CAT_REL_TO so E014's `FactAdd { CountryCode,
                // Portion }` intents land on the same axis as the
                // closed-CVE `TOK_USA` / `TOK_REL_TO` sentinels used by
                // FactRemove paths in PR 3c.B Sub-PR 8.D.2.
                CapcoOpenVocabRef::CountryCode(_) => CAT_REL_TO,
            }),
        }
    }

    /// Apply a batch of [`ReplacementIntent`]s to a [`CapcoMarking`].
    ///
    /// Clones the input marking, dispatches each intent through the
    /// per-axis category mutators ([`capco_category_clear`] /
    /// [`capco_category_replace`]) for `FactRemove` and an analogous
    /// closed-vocab add path for `FactAdd`. `Recanonicalize` returns
    /// the cloned marking unchanged — the engine renders it via
    /// [`MarkingScheme::render_canonical`] to produce canonical form.
    ///
    /// # Idempotence
    ///
    /// **Per-intent vs batch-level `IntentInapplicable`**: the trait
    /// invariants for `apply_intent` require idempotence and
    /// commutativity *within a batch*. A redundant or already-satisfied
    /// intent (e.g., a second `FactRemove` of the same token, or a
    /// `FactRemove` of a token a prior intent in the same batch
    /// already removed) MUST be treated as a per-intent no-op — it
    /// MUST NOT abort the rest of the batch. Only when EVERY intent
    /// in the batch is inapplicable does this method return
    /// `Err(IntentInapplicable)`, signaling to the engine that the
    /// whole fix is a no-op and should be dropped.
    ///
    /// Other error variants (`UnknownToken`, `IntentRejectsLattice`)
    /// propagate immediately — they're not idempotency cases.
    fn apply_intent(
        &self,
        marking: &Self::Marking,
        intents: &[ReplacementIntent<Self>],
    ) -> Result<Self::Marking, ApplyIntentError> {
        let mut out = marking.clone();
        let mut any_applied = false;
        for intent in intents {
            match apply_intent_to_marking(self, &mut out, intent) {
                Ok(()) => {
                    any_applied = true;
                }
                Err(ApplyIntentError::IntentInapplicable) => {
                    // Per-intent no-op: redundant intent (e.g., a
                    // prior intent in the same batch already produced
                    // the desired state, or two rules emitted the
                    // same FactRemove). Idempotence/commutativity
                    // invariant requires the batch to continue.
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        if any_applied {
            Ok(out)
        } else {
            // Whole-batch no-op: engine drops the fix silently.
            Err(ApplyIntentError::IntentInapplicable)
        }
    }

    /// Dispatch a [`Constraint::Custom`] entry to its scheme-private
    /// predicate body. Delegates to `evaluate_custom_by_attrs`, the
    /// name→helper router that the fast-path
    /// [`Self::evaluate_named_constraint`] uses.
    fn evaluate_custom(
        &self,
        name: &'static str,
        marking: &Self::Marking,
    ) -> Vec<ConstraintViolation> {
        evaluate_custom_by_attrs(&marking.0, name)
    }

    fn project(&self, scope: Scope, markings: &[Self::Marking]) -> Self::Marking {
        match scope {
            Scope::Portion => {
                // Identity under portion scope: if the caller passed a
                // single marking we return it; empty → bottom.
                markings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| CapcoMarking::new(CanonicalAttrs::default()))
            }
            Scope::Page | Scope::Document | Scope::Diff => {
                // Page / Document rollup: drive through the existing
                // `PageContext` aggregator (which is already
                // category-component-wise), then apply page rewrites.
                //
                // Byte-identical equivalence with `PageContext` is the
                // Phase B verification gate — see the
                // `scheme_equivalence.rs` tests. When CAPCO's categories
                // move to individual `impl Lattice` types in their own
                // right (Phase C continuation), this implementation
                // swaps in the category-wise composition directly
                // without changing the outward contract.
                let mut ctx = PageContext::new();
                for p in markings {
                    ctx.add_portion(p.0.clone());
                }
                let mut out = CapcoMarking::new(page_context_to_attrs(&ctx));
                // Apply declarative page rewrites. `PageContext`
                // already applies NOFORN-clears-REL-TO internally, so
                // the rewrite is effectively a no-op on today's
                // storage — but declaring it here makes the semantic
                // inspectable per §7a.
                for rw in &self.page_rewrites {
                    let fires = match &rw.trigger {
                        CategoryPredicate::Contains { category, token } => {
                            capco_category_contains(&out, *category, *token)
                        }
                        CategoryPredicate::Empty { category } => {
                            !capco_category_has_values(&out, *category)
                        }
                        CategoryPredicate::Custom(f) => f(&out),
                    };
                    if fires {
                        match &rw.action {
                            CategoryAction::Clear { category } => {
                                tracing::debug!(
                                    rewrite_id = rw.id,
                                    action = "Clear",
                                    ?category,
                                    "PageRewrite fired",
                                );
                                capco_category_clear(&mut out, *category);
                            }
                            CategoryAction::Replace { category, with } => {
                                tracing::debug!(
                                    rewrite_id = rw.id,
                                    action = "Replace",
                                    ?category,
                                    "PageRewrite fired",
                                );
                                capco_category_replace(&mut out, *category, with);
                            }
                            CategoryAction::Promote { from, to, .. } => {
                                // Phase 3 T034 declares the JOINT-
                                // promotion and FGI-absorption rewrites
                                // for the scheduler + catalog surface,
                                // but runtime dispatch stays with
                                // [`PageContext`] (engine.lint does not
                                // drive aggregation through project()
                                // yet — see the note on
                                // `build_page_rewrites`). Treat
                                // `Promote` as a no-op for now; full
                                // transform-driven dispatch lands in
                                // Phase D / Phase E when the engine
                                // switches to scheme-driven roll-up.
                                tracing::debug!(
                                    rewrite_id = rw.id,
                                    action = "Promote",
                                    ?from,
                                    ?to,
                                    "PageRewrite fired (Phase-3 no-op)",
                                );
                            }
                            CategoryAction::Custom(f) => {
                                tracing::debug!(
                                    rewrite_id = rw.id,
                                    action = "Custom",
                                    "PageRewrite fired",
                                );
                                f(&mut out);
                            }
                            CategoryAction::Intent(intent) => {
                                // Bridge to the existing per-intent helper. Errors are handled
                                // as follows:
                                // - `Ok(())`: rewrite applied, marking mutated.
                                // - `IntentInapplicable`: silent no-op for this rewrite (idempotent
                                //   — the marking was already in the post-rewrite state).
                                // - `UnknownToken`: pre-validated for callers that go through
                                //   `Engine::new` (see `validate_intent_rewrites` in
                                //   marque-engine). Direct callers of `CapcoScheme::project` (e.g.,
                                //   tests, scheme-exploration tooling) bypass that validation, so
                                //   this arm IS reachable on the project path; it's also reachable
                                //   if the scheme is mutated between Engine construction and call.
                                // - `IntentRejectsLattice`: NOT pre-validated — it's a runtime
                                //   condition (lattice invariant violation) that
                                //   `validate_intent_rewrites` cannot detect without simulating
                                //   the intent application.
                                // - Future `ReplacementIntent` variants that reach the
                                //   `apply_intent_to_marking` `_` arm also land here.
                                // In every error-arm case, log and treat as a silent no-op rather
                                // than panic; `Engine::lint`'s hot path must not unwind into Tower
                                // middleware. The corpus-parity tests will surface incorrect
                                // projection output.
                                match apply_intent_to_marking(self, &mut out, intent) {
                                    Ok(()) => {
                                        tracing::debug!(
                                            rewrite_id = rw.id,
                                            action = "Intent",
                                            "PageRewrite fired (CategoryAction::Intent)",
                                        );
                                    }
                                    Err(ApplyIntentError::IntentInapplicable) => {
                                        tracing::debug!(
                                            rewrite_id = rw.id,
                                            action = "Intent",
                                            "PageRewrite no-op (intent already satisfied)",
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            rewrite_id = rw.id,
                                            error = ?e,
                                            "PageRewrite Intent failed at runtime — expected to be \
                                             caught at Engine::new validation. Treating as no-op.",
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                out
            }
        }
    }

    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &self.page_rewrites
    }

    /// Commit 5 — substantive `render_canonical` body driven by the
    /// per-axis dispatch table [`RENDER_TABLE`].
    ///
    /// The dispatch loop walks `RENDER_TABLE` in declaration order
    /// (which matches `Category::ordering_rank` per §A.6 p15-17
    /// Figure 2), inserting `//` between consecutive non-empty axes.
    /// Each per-axis renderer in [`crate::render`] writes ONLY its own
    /// bytes to `out`; the dispatch loop is the sole owner of the
    /// `//` major-category separator (CAPCO-2016 §A.6 p15-16).
    ///
    /// `Scope::Diff` returns `Err(fmt::Error)` because diff is a
    /// rule-context query mode, not a renderer-output scope. See
    /// the trait-method doc comment and `marque-rules`'
    /// `RecanonScope` (which narrows `Scope` to exclude `Diff`).
    ///
    /// # Byte-identity invariant
    ///
    /// `scheme.render_canonical(m, Scope::Portion, &mut s)` and
    /// `scheme.render_portion(m)` MUST produce byte-identical output
    /// for any input the existing `render_portion` override handled
    /// (and similarly for `Page` / `render_banner`). The
    /// `render_canonical_default_chain.rs` integration tests pin this
    /// property.
    fn render_canonical(
        &self,
        m: &Self::Marking,
        scope: Scope,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        if matches!(scope, Scope::Diff) {
            return Err(core::fmt::Error);
        }

        // Track whether any axis has emitted bytes yet AND the family
        // of the last-emitting row so the major-category separator
        // `//` can be downgraded to within-category `/` when two
        // consecutive emitting rows belong to the same dissem family.
        //
        // Authority: CAPCO-2016 §A.6 p16 "Dissemination Control
        // Markings ... A single forward slash with no interjected
        // space must be used to separate multiple dissemination
        // controls." Per §G.1 Table 4 row 8 the dissem category
        // includes single-token dissems (ORCON, NOFORN, ...),
        // REL TO, and DISPLAY ONLY — all of which must be `/`
        // separated when commingled in the same `//`-delimited
        // dissem slot. Previously this loop unconditionally inserted
        // `//` between every emitting row, producing canonical
        // strings like `//ORCON//REL TO USA, GBR` (wrong) instead
        // of `//ORCON/REL TO USA, GBR` (canonical).
        //
        // Implementation: render each axis to a per-axis scratch
        // buffer; if non-empty, prepend `//` (different family) or
        // `/` (same dissem family) and copy to `out`. Classification
        // is special: for non-US / JOINT classifications it carries
        // its OWN leading `//` (per §A.6 p15-16 — the `//` occludes
        // the absent US position), so this loop does not prepend
        // ANY separator to the very first axis that emits.
        let mut scratch = String::new();
        let mut prev_family: Option<DissemFamilyMembership> = None;
        for row in RENDER_TABLE {
            scratch.clear();
            (row.render)(m, scope, &mut scratch)?;
            if scratch.is_empty() {
                continue;
            }
            let curr_family = dissem_family_of(row.category);
            match prev_family {
                None => {
                    // First emitting row: classification owns its
                    // leading `//`; every other first-emit just
                    // writes its own bytes.
                }
                Some(prev) => {
                    if prev == DissemFamilyMembership::Member
                        && curr_family == DissemFamilyMembership::Member
                    {
                        // Two consecutive dissem-family rows:
                        // within-category `/` separator.
                        out.write_str("/")?;
                    } else {
                        // Cross-category: §A.6 p16 `//`.
                        out.write_str("//")?;
                    }
                }
            }
            out.write_str(&scratch)?;
            prev_family = Some(curr_family);
        }
        Ok(())
    }

    fn render_portion(&self, m: &Self::Marking) -> String {
        // Override retained for the Phase A byte-identity gate
        // (`render_canonical_default_chain.rs`). Commit 5's
        // render_canonical body is the substantive renderer; this
        // override delegates to it through the trait-default String
        // round-trip. Removing the override is a follow-up once the
        // engine call sites move off `render_portion` to
        // `render_canonical` (commit 6+).
        //
        // `Write for String` is infallible, so a `String` write target
        // never produces `fmt::Error`. The only way the discarded
        // `Result` could be `Err` is a contract violation: an impl
        // returning `Err` for `Scope::Portion`. The
        // [`MarkingScheme::render_canonical`] doc comment forbids
        // this. Debug-assert in development; in release, the contract
        // violation produces an empty / partial `String` rather than
        // a panic (matching the trait-default behavior in
        // `MarkingScheme::render_portion`).
        let mut s = String::new();
        let result = self.render_canonical(m, Scope::Portion, &mut s);
        debug_assert!(
            result.is_ok(),
            "MarkingScheme::render_canonical contract violation: Err returned for Scope::Portion. \
             Conforming impls MUST return Ok(()) for Portion / Page / Document — see trait doc."
        );
        s
    }

    fn render_banner(&self, m: &Self::Marking) -> String {
        // See `render_portion`. Override retained for byte-identity
        // gate; the substantive body is `render_canonical`. Same
        // contract-violation invariant: `Write for String` is
        // infallible, so `Err` here would be a conforming-impl bug
        // forbidden by the trait doc.
        let mut s = String::new();
        let result = self.render_canonical(m, Scope::Page, &mut s);
        debug_assert!(
            result.is_ok(),
            "MarkingScheme::render_canonical contract violation: Err returned for Scope::Page. \
             Conforming impls MUST return Ok(()) for Portion / Page / Document — see trait doc."
        );
        s
    }

    /// Map a closed CVE [`TokenId`] to its host [`CategoryId`].
    ///
    /// Used by the closure operator to route cone tokens to the correct
    /// marking axis when adding facts during implicit-fact propagation.
    /// Per `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md`
    /// §2 finding F1, this is the scheme-layer hook required because
    /// [`Self::category_of`] is keyed by `FactRef<S>` (a `marque-rules`
    /// type) and unavailable at the scheme layer.
    ///
    /// Delegates to the free function `capco_token_category` which is
    /// also used by `category_of`. Returns `None` for sentinel marker
    /// tokens (e.g., `TOK_IC_DISSEM`, `TOK_FGI_MARKER`) that label
    /// categorical predicates rather than addressable atomic tokens.
    fn token_category(&self, id: TokenId) -> Option<CategoryId> {
        capco_token_category(id)
    }

    /// CAPCO implicit-fact propagation catalog (closure operator).
    ///
    /// Returns the static catalog of [`ClosureRule`] rows. The PR 3.7
    /// catalog contains **only the Trio 1 NOFORN rows**, seven rows
    /// covering the implicit-NOFORN markings whose default release
    /// posture is "no foreign disclosure" unless an explicit FD&R
    /// decision is present:
    ///
    /// | Rule key                              | Triggers                          |
    /// |---------------------------------------|-----------------------------------|
    /// | `capco/noforn-if-sar`                 | any SAR program                   |
    /// | `capco/noforn-if-aea`                 | RD / FRD / TFNI                   |
    /// | `capco/noforn-if-ucni`                | UCNI                              |
    /// | `capco/noforn-if-fgi`                 | any FGI atom                      |
    /// | `capco/noforn-if-orcon`               | ORCON / ORCON-USGOV               |
    /// | `capco/noforn-if-imcon-dsen`          | IMCON / DSEN                      |
    /// | `capco/noforn-if-non-ic-controls`     | LIMDIS / LES / SBU / SSI          |
    ///
    /// Each row is suppressed by `FDR_DOMINATORS` (any present
    /// FD&R-axis fact: NOFORN, RELIDO, REL TO, EYES, DISPLAY ONLY).
    ///
    /// The Trio 2 / Trio 3 placeholder rows and the per-marking SCI
    /// implication rows (HCS-O/P[sub] ⇒ {NOFORN, ORCON};
    /// TK-BLFH/KAND/IDIT ⇒ {NOFORN}; SI-G ⇒ {ORCON}) were removed in
    /// PR 3.7 review pass 4 because their proxy triggers
    /// (`AnyInCategory(CAT_SCI)`, `AnyInCategory(CAT_CLASSIFICATION)`)
    /// were imprecise relative to the actual `marque-applied.md`
    /// §4.7.1 semantics; the precise sentinel-based rows land in PR 4
    /// once the per-marking SCI sentinels and open-vocab country-list
    /// FactAdd primitive are available.
    ///
    /// Per `specs/006-engine-rule-refactor/decisions.md` D18, this is a
    /// PUBLIC catalog surface — visible to tooling, scheme-exploration
    /// UIs, and docs generators.
    ///
    /// # Engine wiring (PR 4 future)
    ///
    /// `CapcoScheme` does NOT override `MarkingScheme::closure()` in
    /// PR 3.7 — it inherits the trait's no-op default. The catalog
    /// data ships here as PUBLIC inspection surface (tooling, proptest
    /// harnesses, docs generators can read it via `should_fire`) but
    /// no production code path applies the cone in PR 3.7. PR 4 (T112)
    /// lands both the `CapcoScheme::closure()` override (with Kleene-
    /// fixpoint cone application via the runtime-resolved severity
    /// per `decisions.md` D19 B) and the `Engine::project` call-site
    /// that drives it.
    fn closure_rules(&self) -> &[marque_scheme::ClosureRule] {
        CAPCO_CLOSURE_RULES
    }

    /// Enumerate all tokens present in `marking`.
    ///
    /// Required by `Constraint::ConflictsWithFamily` evaluation: the
    /// generic evaluator walks every present token and applies the
    /// [`FamilyPredicate`] to each. Without this override, the family
    /// predicate never fires (the default returns an empty iterator).
    ///
    /// This implementation walks each attribute field and emits two
    /// shapes of `TokenRef` per the trait contract on
    /// [`MarkingScheme::iter_present_tokens`]:
    ///
    /// - `TokenRef::Token(id)` for concrete closed-CVE tokens whose
    ///   identity matters (the common case — dissem controls, AEA
    ///   markings, non-IC dissem, classification sentinels).
    /// - `TokenRef::AnyInCategory(cat)` for facts whose presence is
    ///   axis-level only: `CAT_REL_TO` when a REL TO country list is
    ///   present, `CAT_SCI` when any SCI marking is present, `CAT_SAR`
    ///   when any SAR program is present, and `CAT_NON_US_CLASSIFICATION`
    ///   for FGI / NATO / JOINT classifications. The
    ///   `AnyInCategory` shape lets family predicates (e.g.
    ///   `is_fdr_dominator`) match against an axis without enumerating
    ///   each open-vocab token (REL TO trigraphs, SAR program names,
    ///   SCI compartments).
    ///
    /// Open-vocab tokens whose **identity** is needed (specific REL TO
    /// trigraphs, individual SAR program names, individual SCI
    /// compartments) are not emitted as `TokenRef::Token` because no
    /// current `ConflictsWithFamily` row needs them on the RHS. If a
    /// future family predicate needs per-token granularity on those
    /// axes, this method's emission set should be extended.
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        Box::new(collect_present_tokens(&marking.0).into_iter())
    }
}

// ---------------------------------------------------------------------------
// Stage D (PR 3.7 T108c) — Closure-rule catalog + family predicates
// ---------------------------------------------------------------------------
//
// The CAPCO §4.7 implicit-fact propagation catalog. See
// `docs/plans/2026-05-01-lattice-design.md` §3 (e) and
// `marque-applied.md` §4.7 for the algebraic treatment.
//
// Engine wiring at `Engine::project` is deferred to PR 4 (T112). This
// module ships the catalog data; the `MarkingScheme::closure_rules()`
// impl on `CapcoScheme` exposes it as the public catalog surface per D18.

// --- Shared suppressor slices ---
//
// FD&R-dominator family: any of these present on a marking/page means an
// explicit FD&R decision exists; the implicit-default trio (Trio 1, 2, 3)
// should NOT fire. Per CAPCO-2016 §B.3.a p19 (canonical enumeration —
// "NOFORN, REL TO, RELIDO, or DISPLAY ONLY"), §B.3 Table 2 pp 21-22
// (scenario-summary table, derivative), and `marque-applied.md` §4.7.1.
//
// Includes:
//   - NOFORN (most restrictive FD&R, top of chain per §H.8 p145)
//   - RELIDO (deferred-release per SFDRA arrangement, §H.8 p154)
//   - DISPLAY ONLY (viewing-only FD&R, §H.8 p163)
//   - REL TO (any country list; `AnyInCategory` covers all partial lists,
//     §H.8 p150)
//   - EYES (US/[LIST] EYES ONLY is an FD&R marking at §H.8 p157)
//
// Note: LES-NF and SBU-NF are NOT included. They are non-IC dissem controls
// that carry NOFORN treatment via PageRewrite, not FD&R markers themselves.
// §B.3.a p19 is the authoritative enumeration of the FD&R set; §B.3 Table 2
// pp 21-22 is the per-scenario marking-summary table (derivative, not the
// definition).
//
// Algebraic note (re: `marque-applied.md` §4.7.3 has_fdr definition):
// §4.7.3 defines `has_fdr(x)` to include LES-NF / SBU-NF for the
// table-design-property monotonicity proof. The in-tree FDR_DOMINATORS
// omits them because (a) LES-NF and SBU-NF entail NOFORN through their
// own PageRewrite (so the operational behavior is preserved — when LES-NF
// is present, NOFORN is added via PageRewrite, and the Trio-1 row would
// then be suppressed by the post-PageRewrite NOFORN regardless), and
// (b) the §4.7.3 case-2 table-design property is preserved per-row because
// the suppressed cone {NOFORN} is exactly the fact that LES-NF / SBU-NF's
// PageRewrite would have added. The monotonicity proof holds via the
// downstream PageRewrite step rather than via FDR_DOMINATORS membership;
// the Trio-1 row is permitted to over-fire on bare-LES-NF / bare-SBU-NF
// because the PageRewrite supplies the suppressor fact downstream.
// `pub(crate)` so the `Vocabulary::is_fdr_dissem` override in
// `crates/capco/src/vocabulary.rs` and the bidirectional value-pin test
// (`mod fdr_dissem_pin` in the same file) can read this slice as the
// single source-of-truth.
//
// **Maintenance contract.** This slice and the neighboring
// `is_fdr_dominator` function answer *different* questions about
// the FD&R family, and the two enumerations are independent on
// purpose:
//   - `FDR_DOMINATORS` (this slice) enumerates **FD&R-set
//     membership** per §B.3.a p19 — the four canonical FD&R
//     markings (NOFORN / REL TO / RELIDO / DISPLAY ONLY) plus the
//     §H.8 p157 EYES legacy. `Vocabulary::is_fdr_dissem` walks
//     this slice and is the authoritative FD&R-membership API.
//   - `is_fdr_dominator` (below) enumerates **FD&R dominators
//     *over* RELIDO** for the `Constraint::ConflictsWithFamily`
//     dispatch on the RELIDO conflict catalog (E054/E055). It
//     deliberately **excludes RELIDO itself** because RELIDO-vs-
//     RELIDO is a tautology in the conflict family — there is no
//     such conflict to detect.
// The intersection of the two sets is "FD&R members that conflict
// with RELIDO" (NOFORN, DISPLAY ONLY, REL TO, EYES). The slice is
// the strict superset. Do not collapse them: a future refactor
// that delegates `is_fdr_dissem` through `is_fdr_dominator` will
// silently under-fire on RELIDO and is pinned against in
// `vocabulary.rs::fdr_dissem_pin::relido_admits_despite_is_fdr_dominator_excluding_it`.
//
// Adding a `Token` entry to this slice requires:
//   1. Considering whether the new token should also dominate
//      RELIDO. If yes, add a parallel arm to `is_fdr_dominator`'s
//      `matches!`. If no, leave `is_fdr_dominator` alone.
//   2. The `Vocabulary::is_fdr_dissem` override picks up the new
//      entry automatically — it iterates this slice directly.
// Adding an `AnyInCategory(CAT_X)` entry requires updating the
// override's per-category routing in `vocabulary.rs` because the
// override receives a single `TokenId` and dispatches through
// `capco_token_category` rather than passing a `TokenRef`.
pub(crate) static FDR_DOMINATORS: &[TokenRef] = &[
    TokenRef::Token(TOK_NOFORN),
    TokenRef::Token(TOK_RELIDO),
    TokenRef::Token(TOK_DISPLAY_ONLY),
    TokenRef::AnyInCategory(CAT_REL_TO),
    // EYES (USA/[LIST] EYES ONLY) is an FD&R marking per §H.8 p157.
    // The sentinel (`TOK_EYES`), the `satisfies_attrs` arm, and the
    // `iter_present_tokens` mapping all land in PR 3.7 rev 3 so that
    // EYES-only portions correctly suppress the implicit-NOFORN
    // trio rows. Per Copilot PR 3.7 review pass 3: an earlier rev
    // claimed EYES was covered via `CAT_REL_TO` fallthrough, which
    // was false — `CAT_REL_TO` only checks `attrs.rel_to`. EYES is
    // a `DissemControl::Eyes` variant produced by the parser
    // (deprecated 2017-10-01 per §H.8 p157 but still recognized for
    // legacy-input compatibility).
    TokenRef::Token(TOK_EYES),
];

// `FDR_OR_RELIDO_INCOMPAT` (the Trio 2 / Trio 3 extended suppressor
// covering FD&R dominators + RELIDO-incompatible tokens like FGI / JOINT
// / NATO / ORCON / LES-NF / SBU-NF) was removed from the active catalog
// in PR 3.7 rev 4. It was consumed by `CLOSURE_RELIDO_US_CLASS` and
// `CLOSURE_RELIDO_RSEN_FOUO` (the Trio 2 placeholder rows), both of
// which retired alongside the SCI per-marking placeholder rows because
// their over-broad triggers (`AnyInCategory(CAT_CLASSIFICATION)` and
// `Token(TOK_RSEN)`/`Token(TOK_FOUO)`) would over-fire on SCI-bearing
// markings before the SCI rows could add their suppressors.
//
// PR 4 (T112) re-introduces the suppressor data when the Trio 2 rows
// land with proper triggers + the closure() engine wiring + runtime-
// resolved severity (per D19 B). For now the suppressor knowledge
// lives only in the inline comments on E054/E055/E056/E057 rows; the
// algebraic shape is documented in `marque-applied.md` §4.7.1.

// --- The implicit-default trio (FD&R-suppressed) ---

// Trio 1 triggers: all markings that imply NOFORN when no explicit FD&R
// decision is present. Per `marque-applied.md` §4.7.1 implicit_NOFORN list.
// One row per trigger group (grouped by source §-citation for traceability).

/// Trio 1, row 1: SAR programs imply NOFORN unless FD&R-marked.
///
/// SAR program identifiers live on `CAT_SAR`. Any SAR marking is a
/// US-originator-controlled marking for which NOFORN is the implicit
/// release posture. CAPCO-2016 §H.5 (pp99-102) governs SAR markings;
/// the NOFORN implication flows from §B.3 Table 2 p21.
const CLOSURE_NOFORN_SAR: ClosureRule = ClosureRule {
    name: "capco/noforn-if-sar",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::AnyInCategory(CAT_SAR)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    default_severity: Severity::Info,
};

/// Trio 1, row 2: RD / FRD / TFNI imply NOFORN unless FD&R-marked.
///
/// Atomic Energy Act markings (Restricted Data, Formerly Restricted Data,
/// Transclassified Foreign Nuclear Information) carry NOFORN by definition
/// for the IC marking context. Per CAPCO-2016 §H.6 (pp104-121) and
/// §B.3 Table 2 p21.
const CLOSURE_NOFORN_AEA_RD: ClosureRule = ClosureRule {
    name: "capco/noforn-if-aea",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[
        TokenRef::Token(TOK_RD),
        TokenRef::Token(TOK_FRD),
        TokenRef::Token(TOK_TFNI),
    ],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    default_severity: Severity::Info,
};

/// Trio 1, row 3: DOD/DOE UCNI implies NOFORN unless FD&R-marked.
///
/// Unclassified Controlled Nuclear Information markings carry a NOFORN
/// treatment in the IC context per §B.3 Table 2 p21. The UCNI marking
/// itself is constrained to UNCLASSIFIED per §H.6 DCNI pp116-117 (DoD)
/// and §H.6 UCNI pp118-119 (DoE); the NOFORN closure fires regardless
/// of class.
const CLOSURE_NOFORN_UCNI: ClosureRule = ClosureRule {
    name: "capco/noforn-if-ucni",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::Token(TOK_UCNI)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    default_severity: Severity::Info,
};

/// Trio 1, row 4: Any FGI atom implies NOFORN unless FD&R-marked.
///
/// Foreign Government Information markings carry an implicit NOFORN posture
/// because the equity belongs to a foreign government and its release requires
/// FD&R authority. Per CAPCO-2016 §H.7 (pp122-130) and §B.3 Table 2 p21.
const CLOSURE_NOFORN_FGI: ClosureRule = ClosureRule {
    name: "capco/noforn-if-fgi",
    label: "CAPCO-2016 §H.7 p122",
    // BOTH triggers are required to cover the two FGI sources per
    // Copilot PR 3.7 review #12:
    //   - `TokenRef::Token(TOK_FGI_MARKER)` is satisfied by
    //     `MarkingClassification::Fgi` (foreign-classified portions
    //     like `//GBR SECRET`) because `satisfies_attrs`'s
    //     classification arm emits `TOK_FGI_MARKER` for that case.
    //   - `TokenRef::AnyInCategory(CAT_FGI_MARKER)` is satisfied by
    //     `attrs.fgi_marker` (explicit `FGI` token).
    // An earlier cleanup dropped the explicit token thinking
    // `AnyInCategory` was a superset; it is NOT — they cover
    // disjoint FGI surfaces. Both must be present so a foreign-
    // classified portion like `//GBR SECRET` reaches the
    // implicit-NOFORN closure.
    triggers: &[
        TokenRef::Token(TOK_FGI_MARKER),
        TokenRef::AnyInCategory(CAT_FGI_MARKER),
    ],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    default_severity: Severity::Info,
};

/// Trio 1, row 5: ORCON / ORCON-USGOV implies NOFORN unless FD&R-marked.
///
/// ORCON and ORCON-USGOV require originator approval before further
/// dissemination; their implicit release posture is NOFORN when no explicit
/// FD&R decision is present. Per CAPCO-2016 §H.8 p136 (ORCON) and
/// §H.8 p139 (ORCON-USGOV), cross-referenced with §B.3 Table 2 p21.
const CLOSURE_NOFORN_ORCON: ClosureRule = ClosureRule {
    name: "capco/noforn-if-orcon",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::Token(TOK_ORCON), TokenRef::Token(TOK_ORCON_USGOV)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    default_severity: Severity::Info,
};

/// Trio 1, row 6: IMCON / DEA SENSITIVE imply NOFORN unless FD&R-marked.
///
/// Controlled Imagery (IMCON) and DEA Sensitive (DSEN) are originator-
/// controlled markings whose implicit release posture is NOFORN. Per
/// CAPCO-2016 §H.8 p142 (IMCON) and §H.8 p159 (DEA SENSITIVE), cross-
/// referenced with §B.3 Table 2 p21.
const CLOSURE_NOFORN_IMCON_DSEN: ClosureRule = ClosureRule {
    name: "capco/noforn-if-imcon-dsen",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::Token(TOK_IMCON), TokenRef::Token(TOK_DSEN)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    default_severity: Severity::Info,
};

/// Trio 1, row 7: Non-IC controls LIMDIS / LES / SBU / SSI imply NOFORN
/// unless FD&R-marked.
///
/// These non-IC dissemination controls have a NOFORN-equivalent treatment in
/// the IC marking context when no explicit FD&R decision is present. Per
/// CAPCO-2016 §H.9 p170 (LIMDIS), §H.9 p181 (LES), §H.9 p176 (SBU),
/// §H.9 p189 (SSI), cross-referenced with §B.3 Table 2 p21.
const CLOSURE_NOFORN_NONICCONTROLS: ClosureRule = ClosureRule {
    name: "capco/noforn-if-non-ic-controls",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[
        TokenRef::Token(TOK_LIMDIS),
        TokenRef::Token(TOK_LES),
        TokenRef::Token(TOK_SBU),
        TokenRef::Token(TOK_SSI),
    ],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    default_severity: Severity::Info,
};

/// The full static CAPCO closure-rule catalog.
///
/// Rows are grouped by the three-trio framing from `marque-applied.md` §4.7.1:
///   1. Trio 1 — implicit NOFORN (FD&R-suppressed)
///   2. Trio 2 — implicit RELIDO (FD&R + RELIDO-incompatible-suppressed)
///   3. Trio 3 — implicit REL TO USA, NATO (FD&R-suppressed)
///   4. Per-marking unconditional implications (unsuppressed)
///
/// Per-row monotonicity attestation (§4.7.3 table-design property, case 2):
/// Every suppressor fact either contains the cone's intent or makes it
/// redundant. For Trio 1/3 (FDR_DOMINATORS): the suppressor is always a
/// manifest FD&R decision that supersedes the implicit default. For Trio 2
/// (FDR_OR_RELIDO_INCOMPAT): same, plus RELIDO-incompatible tokens make the
/// RELIDO cone inapplicable by definition. Unconditional rows have no
/// suppressor — monotonicity is trivial (empty suppressor → no case 2).
///
/// # Coalesced triggers (PR 3.7 limitation)
///
/// Several per-marking unconditional implications (HCS-O/P[sub], SI-G,
/// TK-BLFH/KAND/IDIT) currently use `AnyInCategory(CAT_SCI)` as a proxy
/// trigger because per-compartment sentinels (`TOK_HCS_O`, `TOK_SI_G`, etc.)
/// do not yet exist. This makes the catalog CONSERVATIVE (fires NOFORN/ORCON
/// on any SCI marking, not just the specific compartments) rather than
/// PRECISE. The engine call-site at PR 4 will add precise triggers
/// alongside the per-compartment sentinels (T112 follow-up).
static CAPCO_CLOSURE_RULES: &[ClosureRule] = &[
    // Trio 1: implicit NOFORN rows — these have correct token-level
    // triggers and ship as functional catalog data. The Trio 1 rows
    // are the load-bearing closure-operator entries the engine wires
    // through `Engine::project` at PR 4.
    CLOSURE_NOFORN_SAR,
    CLOSURE_NOFORN_AEA_RD,
    CLOSURE_NOFORN_UCNI,
    CLOSURE_NOFORN_FGI,
    CLOSURE_NOFORN_ORCON,
    CLOSURE_NOFORN_IMCON_DSEN,
    CLOSURE_NOFORN_NONICCONTROLS,
    // Trio 2 (implicit RELIDO), Trio 3 (implicit REL TO USA, NATO),
    // and the per-marking unconditional SCI implications (HCS-O,
    // HCS-P[sub], SI-G, TK-BLFH, TK-KAND, TK-IDIT) were REMOVED
    // from the active catalog in PR 3.7 rev 4 per Copilot review
    // pass 4. Three reasons:
    //   1. Their triggers proxy via broad `AnyInCategory(CAT_SCI)` or
    //      `AnyInCategory(CAT_CLASSIFICATION)` because per-compartment
    //      sentinels (TOK_HCS_O, TOK_SI_G, etc.) don't exist yet —
    //      they over-fire on bare `SI` / bare `TK` / any classified
    //      marking respectively.
    //   2. The Trio 3 cone was an `AnyInCategory(CAT_REL_TO)`
    //      placeholder, structurally incapable of adding the specific
    //      `REL TO USA, NATO` fact.
    //   3. The previous "Severity::Off as catalog-data dormancy gate"
    //      mitigation contradicted D19 B (severity is runtime-resolved,
    //      not catalog-baked), so any user enabling these rows via
    //      `[closure_rules]` config would trigger the over-firing.
    // PR 4 (T112) lands these rows with proper sentinels, real
    // cone-addition machinery (open-vocab FactAdd for the Trio 3
    // country-list case), and the engine wiring to consult runtime
    // severity per-row.
];

// ---------------------------------------------------------------------------
// Per-axis renderer dispatch table (commit 5 populated)
// ---------------------------------------------------------------------------
//
// The dispatch primitive consumed by [`MarkingScheme::render_canonical`].
// One [`AxisRenderRow`] per CAPCO category, in the §A.6 p15-17 Figure 2
// canonical sequence (matches `Category::ordering_rank` declared in
// `build_categories`). The `render_canonical` body walks this table in
// declaration order and inserts the `//` major-category separator
// between consecutive non-empty axis emissions.
//
// The `render` field is a bare function pointer so the table can be
// `const` and shared across `CapcoScheme` instances; per-axis
// renderers cannot capture `&self` or scheme-instance state. All
// inputs come from [`CapcoMarking`] (which wraps
// [`marque_ism::CanonicalAttrs`]) or `&'static` vocabulary tables in
// `crates/capco/src/vocab.rs`.

/// Whether a render row's category is in the §A.6 / §G.1 Table 4
/// row-8 dissem family. Two consecutive emitting rows from the
/// dissem family get a within-category `/` separator instead of
/// the major-category `//` separator (§A.6 p16: "A single forward
/// slash with no interjected space must be used to separate
/// multiple dissemination controls").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DissemFamilyMembership {
    /// Row renders a §H.8 dissem-category axis (single-token
    /// dissems, REL TO, DISPLAY ONLY). Consecutive Members get
    /// `/` between them.
    Member,
    /// Row renders something else (classification, SCI, SAR, AEA,
    /// FGI, non-IC dissem, declassify). Always `//` between this
    /// row and any neighbor that emits.
    Other,
}

/// Per-axis renderer dispatch row.
///
/// `render` writes the axis's canonical bytes for the given `scope`
/// into `out`. Same writer-passing contract as
/// [`MarkingScheme::render_canonical`]: append; do not clear; return
/// `Ok(())` on success.
pub(crate) struct AxisRenderRow {
    /// The category this row renders (e.g., [`CAT_CLASSIFICATION`],
    /// [`CAT_DISSEM`]). Read by the dispatch loop's
    /// [`dissem_family_of`] helper to choose `/` vs `//` between
    /// consecutive emitting rows.
    pub category: CategoryId,
    /// Render the axis's contribution to the canonical form for the
    /// given `scope`, appending bytes to `out`.
    pub render: fn(&CapcoMarking, Scope, &mut dyn core::fmt::Write) -> core::fmt::Result,
}

/// Per-axis renderer dispatch table.
///
/// Order matches `Category::ordering_rank` (CAPCO-2016 §A.6 p15-17
/// Figure 2). The `render_canonical` body walks this table in
/// declaration order; the §A.6 `//` major-category separator is
/// inserted by the dispatch loop, NOT by individual axis renderers.
/// Classification is the sole axis that owns its leading `//` — for
/// non-US / JOINT classifications, the `//` is part of the
/// classification token because it occludes the absent US position
/// (§A.6 p15-16).
pub(crate) const RENDER_TABLE: &[AxisRenderRow] = &[
    AxisRenderRow {
        category: CAT_CLASSIFICATION,
        render: crate::render::render_classification::render_classification,
    },
    AxisRenderRow {
        category: CAT_SCI,
        render: crate::render::render_sci::render_sci,
    },
    AxisRenderRow {
        category: CAT_SAR,
        render: crate::render::render_sar::render_sar,
    },
    AxisRenderRow {
        category: CAT_AEA,
        render: crate::render::render_aea::render_aea,
    },
    AxisRenderRow {
        category: CAT_FGI_MARKER,
        render: crate::render::render_fgi::render_fgi,
    },
    AxisRenderRow {
        category: CAT_DISSEM,
        render: crate::render::render_dissem::render_dissem,
    },
    AxisRenderRow {
        category: CAT_REL_TO,
        render: crate::render::render_rel_to::render_rel_to,
    },
    // DISPLAY ONLY between REL TO and non-IC dissem per CAPCO-2016
    // §G.1 Table 4 row 8 ordering (the IC dissem-category sequence
    // ends with `DISPLAY ONLY [LIST]`). Like REL TO, DISPLAY ONLY
    // carries a country list rather than a single token, so it
    // gets its own renderer (the flat-token `render_dissem` can't
    // emit a list). The category id is reused from `CAT_DISSEM`
    // (DISPLAY ONLY is §H.8 dissem, not §H.9 non-IC) — `category`
    // is informational; dispatch is by declaration order.
    AxisRenderRow {
        category: CAT_DISSEM,
        render: crate::render::render_display_only::render_display_only,
    },
    // Non-IC dissem comes after REL TO in §A.6 sequence (§A.6 p16:
    // "Non-IC Dissemination Control Markings — must follow,
    // Dissemination Controls"). REL TO and DISPLAY ONLY are part
    // of the §H.8 dissem axis; non-IC is its own §H.9 major
    // category. Use the dedicated `CAT_NON_IC_DISSEM` id so the
    // dispatch loop's [`dissem_family_of`] helper correctly emits
    // `//` between this row and the preceding REL TO / DISPLAY
    // ONLY dissem-family rows (not `/`).
    AxisRenderRow {
        category: CAT_NON_IC_DISSEM,
        render: crate::render::render_non_ic_dissem::render_non_ic_dissem,
    },
    // Declassify-on is a no-op in the banner-line dispatch (the CAB
    // is a separate block; see render::render_declassify module doc).
    // Kept in the table so the declassify axis is visible to future
    // CAB-rendering work.
    AxisRenderRow {
        category: CAT_DECLASSIFY_ON,
        render: crate::render::render_declassify::render_declassify,
    },
];

// ===========================================================================
// PR 3b.D (T026d) — Class-floor catalog dispatch (§3.4.6)
// ===========================================================================
//
// `class_floor_catalog_eval` is the static-table dispatcher for the 27
// `Constraint::Custom` rows declared by `build_constraints` under the
// "PR 3b.D (T026d) — class-floor catalog (§3.4.6)" section header.
//
// Each row's predicate has a uniform shape: "if marking M is present in
// `attrs`, the page's classification must satisfy F(M)" where F(M) is
// either a floor (`level >= floor`) or an equality (`level == U`). The
// table stores one entry per row carrying:
//
//   - `name`: catalog row identifier (matches `Constraint::Custom { name }`)
//   - `marking_label`: human-readable marking name for the diagnostic
//   - `presence`: predicate `fn(&CanonicalAttrs) -> bool` checking whether
//      the family pattern is present
//   - `policy`: `ClassFloorPolicy` — either `AtLeast(level)` or `EqualsU`
//   - `severity`: `Severity` — `Error` for enumerated rows, `Warn` for
//      passthrough rows (§3.4.6 Q-3.4.6b)
//   - `citation`: per-row §-citation matching `Constraint::Custom { label }`
//   - `passthrough`: `true` for unknown-floor passthrough rows (drives the
//      diagnostic message variant)
//
// The walker `DeclarativeClassFloorRule` (in `rules_declarative.rs`)
// iterates the table and emits one `Diagnostic` per row whose presence
// predicate fires AND whose floor/equality predicate is violated.
//
// FORWARD LINK to PR 3.7 (T108b): once `TokenRef::ClassAtLeast(ClassLevel)`
// or `Constraint::ClassFloor` lands as a primitive in `marque-scheme`,
// these rows can re-classify from `Constraint::Custom` to the new
// primitive form without changing per-row semantics. See
// `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md` §3 for the
// architectural rationale.

/// Floor policy for a class-floor catalog row.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ClassFloorPolicy {
    /// Classification level must be ≥ this floor (TS / S / C semantics).
    AtLeast(Classification),
    /// Classification must be exactly UNCLASSIFIED. Used by the UCNI
    /// ceiling rows (§2.4 of the planning doc).
    EqualsU,
}

/// One catalog row. The walker dispatches over the `&[ClassFloorRow]`
/// table; each row owns its presence predicate, floor policy, severity,
/// citation, and human-readable marking label for diagnostic messages.
///
/// # Naming-prefix invariant (PR D R3.2)
///
/// Every row's `name` MUST start with one of two prefixes:
///
///   - **`E058/<purpose>`** — for rows replacing a retired legacy rule
///     (the four E022 / E025 / E027 successors:
///     `E058/CNWDI-classification-floor`,
///     `E058/SAR-classification-floor`,
///     `E058/DOD-UCNI-classification-ceiling`,
///     `E058/DOE-UCNI-classification-ceiling`).
///   - **`class-floor/<marking>`** — for rows with no retired-rule
///     predecessor (e.g., `class-floor/HCS-comp-sub`,
///     `class-floor/SI-comp`, `class-floor/BALK`,
///     `class-floor/passthrough-BUR`).
///
/// The prefix invariant is what makes the
/// [`is_class_floor_catalog_name`] dispatch routing O(1) instead of
/// a linear catalog scan. The
/// `class_floor_catalog_naming_convention` test in
/// `crates/capco/tests/class_floor_catalog.rs` enforces this at
/// build time; adding a row whose name doesn't match either prefix
/// will fail CI.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassFloorRow {
    /// Catalog row name — matches the `Constraint::Custom { name }` of
    /// the same logical row. MUST start with `E058/` or
    /// `class-floor/` per the naming-prefix invariant above.
    pub(crate) name: &'static str,
    /// Human-readable marking name for the diagnostic message
    /// (e.g., `"CNWDI"`, `"HCS-P sub-compartment"`, `"BUR family"`).
    pub(crate) marking_label: &'static str,
    /// Marking-presence predicate.
    pub(crate) presence: fn(&marque_ism::CanonicalAttrs) -> bool,
    /// Floor policy.
    pub(crate) policy: ClassFloorPolicy,
    /// Per-row severity (`Error` for enumerated rows, `Warn` for
    /// passthrough rows per §3.4.6 Q-3.4.6b).
    pub(crate) severity: marque_rules::Severity,
    /// Per-row §-citation, matching `Constraint::Custom { label }`.
    pub(crate) citation: &'static str,
    /// True for the unknown-floor passthrough rows. Drives the
    /// diagnostic message variant (passthrough rows quote the §3.7
    /// passthrough-policy framing).
    pub(crate) passthrough: bool,
    /// Diagnostic-span anchor token kind. Used by
    /// [`class_floor_anchor_span`] when populating
    /// `ConstraintViolation::span` in [`class_floor_emit`]. `None`
    /// means "fall back to the classification span" (NATO rows where
    /// the classification token IS the marking surface).
    pub(crate) primary_kind: Option<marque_ism::TokenKind>,
}

// ---------------------------------------------------------------------------
// The catalog — 27 rows at §3.4.6 family granularity
// ---------------------------------------------------------------------------

const CLASS_FLOOR_CATALOG: &[ClassFloorRow] = &[
    // ---- §2.1 Floor TS (5 rows) ------------------------------------
    ClassFloorRow {
        name: "class-floor/HCS-comp-sub",
        marking_label: "HCS sub-compartment markings",
        presence: presence_hcs_comp_sub,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/SI-comp",
        marking_label: "SI compartments",
        presence: presence_si_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/TK-BLFH",
        marking_label: "TK-BLFH (BLUEFISH)",
        presence: presence_tk_blfh,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
    },
    // BALK and BOHEMIA: NATO Special Access Programs per CAPCO-2016
    // §G.2 p40 + §H.7 p127. PR 9c.1 T134 corrected the structural
    // model — BALK/BOHEMIA now live in `sci_markings` as
    // `SciControlSystem::NatoSap` entries (not as fused
    // `NatoClassification::*Balk/*Bohemia` variants which were retired
    // as a wrong fusion of classification and control-marking
    // semantics). The presence predicates fire on the SCI axis;
    // the floor checks effective US-equivalent classification level
    // (typically TS for NATO SAPs per §G.2 p40).
    //
    // Severity = Warn at the catalog row level per PR 9c.1 D5 (the
    // architect's pre-flight decision): §G.2 p40's citation depth is
    // too soft to drive Error — the manual identifies BOHEMIA/BALK as
    // SAPs and lists them in the ARH table but does not enumerate a
    // classification floor with the precision §H.6 has for RD/CNWDI.
    // A Warn-with-suggest fires when the data is structurally
    // inconsistent (BALK/BOHEMIA marked but classification < TS) and
    // surfaces an actionable suggestion without blocking.
    ClassFloorRow {
        name: "class-floor/BALK",
        marking_label: "BALK (NATO)",
        presence: presence_balk,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §G.2 p40",
        passthrough: false,
        // `None` falls through to the Classification token span. PR
        // 9c.1 Commit 3's parser writes the BALK SciMarking but does
        // not push a `TokenKind::SciSystem` span for the legacy
        // compound text (`CTS-BALK` is a single Classification token
        // that carries both the bare-class and the companion semantic);
        // anchoring at the Classification token is the right UX.
        primary_kind: None,
    },
    ClassFloorRow {
        name: "class-floor/BOHEMIA",
        marking_label: "BOHEMIA (NATO)",
        presence: presence_bohemia,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §G.2 p40",
        passthrough: false,
        primary_kind: None,
    },
    // ---- §2.2 Floor S (8 rows) -------------------------------------
    ClassFloorRow {
        name: "class-floor/HCS-comp",
        marking_label: "HCS-O / HCS-P (compartment, no sub-compartment)",
        presence: presence_hcs_comp_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/RSV-comp",
        marking_label: "RSV compartment",
        presence: presence_rsv_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/TK",
        marking_label: "TK / TK-IDIT / TK-KAND",
        presence: presence_tk_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/RD-SG",
        marking_label: "RD-SIGMA",
        presence: presence_rd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p113",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    ClassFloorRow {
        name: "class-floor/FRD-SG",
        marking_label: "FRD-SIGMA",
        presence: presence_frd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p113",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    // CNWDI — replaces retired E022. Walker-prefixed name per PM
    // directive #5.
    ClassFloorRow {
        name: "E058/CNWDI-classification-floor",
        marking_label: "CNWDI",
        presence: presence_rd_cnwdi,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    ClassFloorRow {
        name: "class-floor/RSEN",
        marking_label: "RSEN",
        presence: presence_rsen,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p149",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
    },
    ClassFloorRow {
        name: "class-floor/IMCON",
        marking_label: "IMCON",
        presence: presence_imcon,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p144",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
    },
    // ---- §2.3 Floor C (8 rows) -------------------------------------
    ClassFloorRow {
        name: "class-floor/SI",
        marking_label: "SI (bare)",
        presence: presence_si_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
    },
    // SAR — replaces retired E027.
    ClassFloorRow {
        name: "E058/SAR-classification-floor",
        marking_label: "SAR",
        presence: presence_sar,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.5",
        passthrough: false,
        primary_kind: Some(TokenKind::SarIndicator),
    },
    ClassFloorRow {
        name: "class-floor/RD",
        marking_label: "RD",
        presence: presence_rd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    ClassFloorRow {
        name: "class-floor/FRD",
        marking_label: "FRD",
        presence: presence_frd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    ClassFloorRow {
        name: "class-floor/TFNI",
        marking_label: "TFNI",
        presence: presence_tfni,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p107",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    // ATOMAL: PR 9c.1 T134 reclassified as AEA-axis marking per
    // CAPCO-2016 §H.7 p122 worked example
    // (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`). The class floor is the
    // same Confidential lower-bound as the rest of §H.6's AEA family
    // (RD/FRD/TFNI). Severity stays `Error` because §H.7 p122 is a
    // direct, worked-example-grounded citation (parallel depth to
    // §H.6's class-floor citations for RD/FRD), distinguishing it from
    // the softer §G.2 p40 BALK/BOHEMIA citation.
    //
    // `primary_kind: None` (falls back to Classification): same
    // rationale as BALK/BOHEMIA — legacy compound text like `NCA` /
    // `CTSA` is a single `TokenKind::Classification` carrying both
    // the bare-class and the AEA companion semantic; the parser does
    // not emit a separate `TokenKind::AeaMarking` span for the
    // canonicalized companion write. Anchoring at the Classification
    // token is the right UX for the legacy-compound case.
    ClassFloorRow {
        name: "class-floor/ATOMAL",
        marking_label: "ATOMAL (NATO)",
        presence: presence_atomal,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.7 p122",
        passthrough: false,
        primary_kind: None,
    },
    ClassFloorRow {
        name: "class-floor/ORCON",
        marking_label: "ORCON / ORCON-USGOV",
        presence: presence_orcon_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p136",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
    },
    ClassFloorRow {
        name: "class-floor/EYES-ONLY",
        marking_label: "EYES ONLY",
        presence: presence_eyes_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p152",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
    },
    // ---- §2.4 Floor =U (2 rows; UCNI split per PM decision) ----------
    ClassFloorRow {
        name: "E058/DOD-UCNI-classification-ceiling",
        marking_label: "DOD UCNI",
        presence: presence_dod_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p116",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    ClassFloorRow {
        name: "E058/DOE-UCNI-classification-ceiling",
        marking_label: "DOE UCNI",
        presence: presence_doe_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p118",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
    },
    // ---- §2.6 Unknown-floor passthrough (4 rows; Warn) ---------------
    //
    // `row.citation` uses the `Section 3.7` form (not `§3.7`) because
    // the citation-lint tool (FR-018) parses `§N.M` in `citation:`
    // struct-field literals as a CAPCO section reference and would
    // flag `§3` as a bare section without subsection letter (CAPCO
    // sections are A-K, not digits). The cross-document
    // `marque-applied.md` prefix doesn't currently disambiguate.
    //
    // The corresponding `Constraint::Custom { label: "marque-applied.md §3.7 ..." }`
    // entries in `build_constraints()` keep the `§3.7` form because
    // the lint only scans `citation:`, `message:`, and
    // `constraint_label:` struct fields (not `label:`). The bridge's
    // user-visible `Diagnostic.citation` IS the `§3.7` form because
    // `marque_scheme::constraint::evaluate` overrides
    // `ConstraintViolation::citation` from the constraint's `label`
    // field after `evaluate_custom` returns — so the lint is happy
    // AND end users see the canonical `§3.7` form. `row.citation` is
    // internal scratch (never user-visible post-7.3) after the
    // `evaluate` override step.
    //
    // Tracking issue: the citation-lint tool's CAPCO-context
    // implicit-treatment of `citation:` fields should learn to
    // recognize cross-document prefixes (`<word>.md §`) so the
    // `§3.7` form can be used uniformly. Not in scope here.
    ClassFloorRow {
        name: "class-floor/passthrough-BUR",
        marking_label: "BUR family",
        presence: presence_passthrough_bur,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/passthrough-HCS-X",
        marking_label: "HCS-X",
        presence: presence_passthrough_hcs_x,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/passthrough-KLM",
        marking_label: "KLM family",
        presence: presence_passthrough_klm,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
    },
    ClassFloorRow {
        name: "class-floor/passthrough-MVL",
        marking_label: "MVL",
        presence: presence_passthrough_mvl,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
    },
];

// ===========================================================================
// PR 3b.E (T026e) — SCI per-system catalog (§H.4)
// ===========================================================================
//
// `sci_per_system_catalog_eval` is the static-table dispatcher for the 5
// `Constraint::Custom` rows declared by `build_constraints` under the
// "PR 3b.E (T026e) — SCI per-system catalog (§H.4)" section header.
//
// Each row's predicate has a uniform shape: "if SCI marking M is present in
// `attrs`, the portion's IC dissem block must satisfy F(M)" where F(M) is
// either a companion-required check (NOFORN must appear) or a multi-branch
// check covering required-and-forbidden companions (ORCON required, ORCON-
// USGOV forbidden, etc.). The table stores one entry per row carrying:
//
//   - `name`: catalog row identifier (matches `Constraint::Custom { name }`,
//      and starts with the `sci-per-system/` prefix)
//   - `marking_label`: human-readable marking name for the diagnostic
//   - `presence`: predicate `fn(&CanonicalAttrs) -> bool` checking whether
//      the family pattern is present
//   - `kind`: dispatch tag — `CompanionRequired` (single dissem-control
//      insertion) or `Custom` (closure for multi-branch emit logic)
//   - `severity`: per-row default `Severity` (typically `Warn`; the emit
//      helper escalates per-branch to `Error` no-fix when no IC dissem
//      block exists)
//   - `citation`: per-row §-citation matching `Constraint::Custom { label }`
//
// Diagnostic-span anchoring is NOT a row field — companion-insertion
// branches anchor the diagnostic at the offending SCI marking token via
// `first_sci_span(attrs)`, while token-replacement branches (e.g., the
// OC-USGOV → OC fix in row #1 / #3 / #4) anchor both the diagnostic and
// the fix at the dissem token's own span so the user sees the offending
// dissem token directly. See the per-emit-fn doc comments for the
// branch-specific anchor.
//
// The walker `DeclarativeSciPerSystemRule` (in `rules_declarative.rs`)
// iterates the table and emits per-row diagnostics.
//
// FORWARD LINK to PR 4 (per-category Lattice impls): once `marque-scheme`
// exposes `Constraint::CompanionRequired<Set>` / `Forbid<Set>` primitives
// (or the equivalent ImplTable / closure-operator machinery from
// `marque-applied.md` §3.4.6), these rows can re-classify from
// `Constraint::Custom` to a primitive form without changing per-row
// semantics. See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`
// §1 for the architectural rationale.

/// Companion form (abbreviated vs full) inferred from the dissem-token
/// text observed on a portion. Used to keep the inserted token's surface
/// form consistent with the existing block (so `(S//HCS-O//OC)` inserts
/// `/NF`, not `/NOFORN`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CompanionForm {
    /// Short form: `OC`, `NF`, `OC-USGOV`. Used when the first observed
    /// dissem token on the portion is a portion/abbrev surface form.
    Abbreviated,
    /// Full form: `ORCON`, `NOFORN`. Used otherwise (banner long-form or
    /// no dissem block yet).
    Full,
}

/// Walker rule ID shared by every SCI per-system catalog emit body.
/// `RuleId::new` is `const fn`, so this is a zero-cost replacement for
/// the four prior inline `RuleId::new("E059")` call sites (one per
/// row-emit helper). Hoisting also makes a future rule-ID change a
/// single edit.
const RULE_E059: marque_rules::RuleId = marque_rules::RuleId::new("E059");

/// Dispatch tag for an SCI per-system catalog row's emit body. Two
/// variants keep the `match row.kind` arm count under the ≤3-branch
/// reviewer-attestation cap (§7(b) of the PR 3b.E plan).
#[derive(Copy, Clone)]
pub(crate) enum SciPerSystemKind {
    /// Single dissem-control insertion. The row encodes "if marking M is
    /// present, dissem control D must appear; if absent, emit a
    /// zero-width insertion fix at the end of the IC dissem block." The
    /// only PR-E rows using this kind are the NOFORN-only rows (#2 and
    /// #5).
    CompanionRequired {
        /// The dissem control whose presence is required.
        dissem: marque_ism::DissemControl,
        /// Component for the diagnostic message (e.g., "NOFORN").
        token_name: &'static str,
    },
    /// Custom multi-branch emit. The row encodes a closure that produces
    /// the full emit list, used by rows whose emit logic spans 2-3 distinct
    /// branches with row-specific text and span logic (rows #1, #3, #4).
    /// The `candidate_span` argument is the full marking-scope span
    /// (portion or banner) that the engine's `synthesize_fixes` path
    /// uses to look up the parsed marking for `apply_intent` +
    /// `render_canonical`. `fix_scope` is the scope discriminator
    /// embedded in any `FactAdd` / `Recanonicalize` intent the row
    /// emits — `Scope::Portion` for portion candidates, `Scope::Page`
    /// for banner candidates.
    Custom(
        fn(
            &marque_ism::CanonicalAttrs,
            marque_ism::Span,
            marque_scheme::Scope,
            &SciPerSystemRow,
        ) -> Vec<marque_rules::Diagnostic<CapcoScheme>>,
    ),
}

/// One catalog row. The walker dispatches over `&[SciPerSystemRow]`;
/// each row owns its presence predicate, dispatch kind, severity,
/// citation, and human-readable marking label.
///
/// # Naming-prefix invariant
///
/// Every row's `name` MUST start with `sci-per-system/`. The
/// `sci_per_system_catalog_naming_convention` test in
/// `crates/capco/tests/sci_per_system_catalog.rs` enforces this at build
/// time so adding a row that doesn't follow the convention fails CI.
/// The prefix is what makes [`is_sci_per_system_catalog_name`] dispatch
/// O(1) instead of a linear catalog scan.
#[derive(Copy, Clone)]
pub(crate) struct SciPerSystemRow {
    /// Catalog row name — matches the `Constraint::Custom { name }` of
    /// the same logical row. MUST start with `sci-per-system/`.
    pub(crate) name: &'static str,
    /// Human-readable marking name for the diagnostic message
    /// (e.g., `"HCS-O"`, `"TK-{BLFH|IDIT|KAND}"`).
    pub(crate) marking_label: &'static str,
    /// Marking-presence predicate.
    pub(crate) presence: fn(&marque_ism::CanonicalAttrs) -> bool,
    /// Dispatch kind — `CompanionRequired` (single-token) or `Custom`
    /// (multi-branch closure).
    pub(crate) kind: SciPerSystemKind,
    /// Default severity (typically `Warn`). The emit helper escalates
    /// per-branch to `Error` no-fix when no IC dissem block exists.
    pub(crate) severity: marque_rules::Severity,
    /// Per-row §-citation, matching `Constraint::Custom { label }`.
    pub(crate) citation: &'static str,
}

// ---------------------------------------------------------------------------
// The catalog — 5 rows at §H.4 family granularity
// ---------------------------------------------------------------------------

const SCI_PER_SYSTEM_CATALOG: &[SciPerSystemRow] = &[
    // Row #1 — HCS-O companions (ORCON + NOFORN required, ORCON-USGOV
    // forbidden). §H.4 p64.
    SciPerSystemRow {
        name: "sci-per-system/HCS-O-companions",
        marking_label: "HCS-O",
        presence: presence_hcs_o,
        kind: SciPerSystemKind::Custom(emit_hcs_o_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p64",
    },
    // Row #2 — HCS-P NOFORN (NOFORN required). §H.4 p66.
    SciPerSystemRow {
        name: "sci-per-system/HCS-P-NOFORN",
        marking_label: "HCS-P",
        presence: presence_hcs_p_any,
        kind: SciPerSystemKind::CompanionRequired {
            dissem: marque_ism::DissemControl::Nf,
            token_name: "NOFORN",
        },
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p66",
    },
    // Row #3 — HCS-P sub-compartment companions (ORCON required,
    // ORCON-USGOV forbidden). §H.4 p68. NOFORN is covered by row #2.
    SciPerSystemRow {
        name: "sci-per-system/HCS-P-sub-companions",
        marking_label: "HCS-P sub-compartment",
        presence: presence_hcs_p_sub,
        kind: SciPerSystemKind::Custom(emit_hcs_p_sub_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p68",
    },
    // Row #4 — SI-G companions (ORCON required, ORCON-USGOV forbidden).
    // §H.4 p80.
    SciPerSystemRow {
        name: "sci-per-system/SI-G-companions",
        marking_label: "SI-G",
        presence: presence_si_g,
        kind: SciPerSystemKind::Custom(emit_si_g_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p80",
    },
    // Row #5 — TK compartment NOFORN (BLFH/IDIT/KAND require NOFORN).
    // §H.4 p87 (TK-BLFH) + p91 (TK-IDIT) + p95 (TK-KAND).
    SciPerSystemRow {
        name: "sci-per-system/TK-compartment-NOFORN",
        marking_label: "TK-{BLFH|IDIT|KAND}",
        presence: presence_tk_compartment_noforn,
        kind: SciPerSystemKind::CompanionRequired {
            dissem: marque_ism::DissemControl::Nf,
            token_name: "NOFORN",
        },
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p87 + p91 + p95",
    },
];

// ---------------------------------------------------------------------------
// Convenience: expose the classification level for test assertions
// ---------------------------------------------------------------------------

impl CapcoMarking {
    /// The effective US classification level, if any. Thin shim over
    /// `CanonicalAttrs::us_classification` for test readability.
    #[inline]
    pub fn classification(&self) -> Option<Classification> {
        self.0.us_classification()
    }
}
