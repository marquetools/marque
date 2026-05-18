// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Constraint-evaluation entry points: [`satisfies_attrs`] (the
//! source of truth for CAPCO's `TokenRef` semantics),
//! [`evaluate_custom_by_attrs`] (catalog-name → predicate dispatch),
//! and [`collect_present_tokens`] (the `ConflictsWithFamily`
//! emission helper). Lifted from the monolithic `predicates.rs` per
//! the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_ism::{Classification, CountryCode};
use marque_scheme::TokenRef;

use super::super::constraints::{
    e012_dual_classification, e014_joint_rel_to_coverage, e021_aea_requires_noforn,
    e024_rd_precedence, e038_dos_dissem_requires_noforn,
};
use super::super::*;
use super::class_floor::{class_floor_catalog_eval, is_class_floor_catalog_name};
use super::joint_hcs::{hcs_system_constraints, joint_requires_usa};
use super::sci_per_system::{is_sci_per_system_catalog_name, sci_per_system_catalog_eval};

// ---------------------------------------------------------------------------
// Predicate implementations (free functions — trait impls delegate here)
// ---------------------------------------------------------------------------
//
// `satisfies_attrs` and `evaluate_custom_by_attrs` are the source of
// truth for CAPCO's constraint semantics. They take `&CanonicalAttrs`
// directly to avoid forcing callers on the fast path to wrap in
// `CapcoMarking` (which would require cloning the attributes). The
// trait impls on `CapcoScheme` delegate to them, and the fast-path
// inherent method `CapcoScheme::evaluate_named_constraint` uses them
// directly to dispatch a single named constraint without walking
// the whole catalog.

/// Resolve a [`TokenRef`] against raw [`marque_ism::CanonicalAttrs`].
///
/// **Token-presence semantics** (T035):
/// - [`TokenRef::Token(id)`] returns true when the marking carries
///   the named token *anywhere* relevant — `TOK_USA` ⇒ "USA in
///   REL TO" (the dissemination context), `TOK_RD` ⇒ "RD anywhere in
///   `aea_markings`", etc.
/// - [`TokenRef::AnyInCategory(cat)`] returns true when the category
///   has at least one populated value. `CAT_DISSEM` intentionally
///   counts both the dissem axis (`dissem_us` and `dissem_nato`
///   together, walked via `attrs.dissem_iter()` post PR 9b / FR-046
///   split) AND `rel_to` as dissem-flavored presence, matching the
///   historical E015 predicate.
///
/// `MarkingClassification::Conflict` is deliberately excluded from
/// `TOK_NON_US_CLASSIFICATION` / `CAT_NON_US_CLASSIFICATION` — that
/// state is E012's concern, not E015's.
///
/// Sentinel `TokenId`s not used by the current catalog
/// (`TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM` — the *token* sentinels,
/// distinct from the category form) fall through to `false`;
/// they are declared for future T035b consumption. The
/// `AnyInCategory(CAT_NON_IC_DISSEM)` arm IS live as of Issue
/// #524 Phase 3 (consumed by `CLOSURE_RELIDO_US_CLASS`'s "no
/// other dissem" suppressor list); the token-form
/// `TOK_NON_IC_DISSEM` stays in the fall-through arm.
pub(crate) fn satisfies_attrs(attrs: &marque_ism::CanonicalAttrs, token_ref: &TokenRef) -> bool {
    use marque_ism::{
        AeaMarking, DissemControl, MarkingClassification, SciControl, SciControlBare,
        SciControlSystem,
    };
    match token_ref {
        TokenRef::Token(id) => match *id {
            TOK_NOFORN => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf)),
            TOK_USA => attrs.rel_to.contains(&CountryCode::USA),
            TOK_JOINT => {
                matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
            }
            // PR #505: per-variant classification sentinels. Strict
            // match on the classification-axis variant, excluding
            // `MarkingClassification::Conflict { .. }` (that state is
            // E012's concern — see fn doc above for the same convention
            // applied to `TOK_NON_US_CLASSIFICATION`).
            TOK_NATO_CLASS => {
                matches!(&attrs.classification, Some(MarkingClassification::Nato(_)))
            }
            TOK_FGI_CLASS => {
                matches!(&attrs.classification, Some(MarkingClassification::Fgi(_)))
            }
            TOK_RESTRICTED => matches!(
                &attrs.classification,
                Some(c) if c.effective_level() == Classification::Restricted
            ),
            TOK_RD => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Rd(_))),
            TOK_FRD => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Frd(_))),
            TOK_TFNI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Tfni)),
            TOK_CNWDI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Rd(rd) if rd.cnwdi)),
            // Issue #407: `TOK_UCNI` now resolves to the DOE
            // variant only; `TOK_DCNI` covers the DOD variant per
            // CAPCO-2016 §H.6 p118 (DOE UCNI) and §H.6 p116 (DOD
            // UCNI / DCNI). Prior behavior aliased both variants
            // under `TOK_UCNI` which the vocabulary surface then
            // collapsed onto a single canonical form. Pattern-C
            // strip closures read the AEA axis directly by variant
            // match and are unaffected by this sentinel split.
            TOK_UCNI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::DoeUcni)),
            TOK_DCNI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::DodUcni)),
            // PR 9c.1 (T134): ATOMAL lives in the AEA axis per
            // CAPCO-2016 §H.7 p122 (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`).
            TOK_ATOMAL => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Atomal(_))),
            // PR 9c.1 (T134): BALK / BOHEMIA are NATO SAPs living in
            // the SCI axis per CAPCO-2016 §G.2 p40 + §H.7 p127.
            TOK_BALK => attrs.sci_markings.iter().any(|m| {
                matches!(
                    m.system,
                    SciControlSystem::NatoSap(marque_ism::NatoSap::Balk)
                )
            }),
            TOK_BOHEMIA => attrs.sci_markings.iter().any(|m| {
                matches!(
                    m.system,
                    SciControlSystem::NatoSap(marque_ism::NatoSap::Bohemia)
                )
            }),
            // Issue #524 (Phase 1): per-compartment SCI sentinels.
            //
            // Each arm scans `attrs.sci_markings` for a marking whose
            // `system` anchors on the matching `SciControlBare` AND
            // whose `compartments` carry the matching identifier. The
            // structural shape — not `canonical_enum` — is the
            // witness so sub-compartmented forms still resolve
            // (`canonical_enum` is `None` whenever sub-compartments
            // are present; see `marque_ism::SciMarking.canonical_enum`
            // doc). Delegates to the existing `presence::anchors_on`
            // / `presence::has_compartment` helpers (zero allocation,
            // already used by the §H.4 per-system rules).
            //
            // Authority: §H.4 marking templates — see `TOK_SI_G` /
            // `TOK_HCS_O` / etc. doc-comments in
            // `crates/capco/src/scheme/mod.rs`.
            TOK_SI_G => attrs.sci_markings.iter().any(|m| {
                super::presence::anchors_on(m, SciControlBare::Si)
                    && super::presence::has_compartment(m, "G")
            }),
            TOK_HCS_O => attrs.sci_markings.iter().any(|m| {
                super::presence::anchors_on(m, SciControlBare::Hcs)
                    && super::presence::has_compartment(m, "O")
            }),
            TOK_HCS_P => attrs.sci_markings.iter().any(|m| {
                super::presence::anchors_on(m, SciControlBare::Hcs)
                    && super::presence::has_compartment(m, "P")
            }),
            // Issue #524 (Phase 2): grammar-shape sentinel fires only
            // when HCS-P additionally carries at least one sub-
            // compartment. §H.4 p66 (bare HCS-P) implies NOFORN only;
            // §H.4 p68 (HCS-P [SUB]) implies NOFORN + ORCON. See
            // `TOK_HCS_P_SUB` doc-comment in
            // `crates/capco/src/scheme/mod.rs`.
            TOK_HCS_P_SUB => attrs.sci_markings.iter().any(|m| {
                super::presence::anchors_on(m, SciControlBare::Hcs)
                    && m.compartments
                        .iter()
                        .any(|c| c.identifier.as_str() == "P" && !c.sub_compartments.is_empty())
            }),
            TOK_TK_BLFH => attrs.sci_markings.iter().any(|m| {
                super::presence::anchors_on(m, SciControlBare::Tk)
                    && super::presence::has_compartment(m, "BLFH")
            }),
            TOK_TK_IDIT => attrs.sci_markings.iter().any(|m| {
                super::presence::anchors_on(m, SciControlBare::Tk)
                    && super::presence::has_compartment(m, "IDIT")
            }),
            TOK_TK_KAND => attrs.sci_markings.iter().any(|m| {
                super::presence::anchors_on(m, SciControlBare::Tk)
                    && super::presence::has_compartment(m, "KAND")
            }),
            // "HCS markings" is plural in CAPCO §H.3 p57 — it covers
            // the bare `HCS` token AND the compound forms `HCS-O` /
            // `HCS-P` / `HCS-O-P`. CVE-projection variants `Hcs`,
            // `HcsO`, `HcsP` are all matched explicitly; the
            // structural path via `sci_markings` covers any compound
            // anchored on `SciControlBare::Hcs` regardless of the
            // specific compartments attached.
            TOK_HCS => {
                attrs
                    .sci_controls
                    .iter()
                    .any(|s| matches!(s, SciControl::Hcs | SciControl::HcsO | SciControl::HcsP))
                    || attrs.sci_markings.iter().any(|m| {
                        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                    })
            }
            TOK_FGI_MARKER => {
                // FGI presence covers two disjoint axes:
                //   - `attrs.fgi_marker` for explicit `FGI` token in
                //     the dissem-axis position
                //   - `MarkingClassification::Fgi(_)` for foreign-classified
                //     portions like `//GBR SECRET` (the FGI lives on the
                //     classification axis, not the dissem-axis fgi_marker)
                // Per Copilot PR 3.7 review pass 3: prior to this fix
                // `satisfies_attrs(TOK_FGI_MARKER)` checked only
                // `attrs.fgi_marker.is_some()`, missing the
                // classification-axis case. The implicit-NOFORN closure
                // row `capco/noforn-if-caveated` (which counts FGI as one
                // of its triggers) would therefore not fire on foreign-
                // classified portions even though the trigger list
                // declares both `TOK_FGI_MARKER` and
                // `AnyInCategory(CAT_FGI_MARKER)`.
                attrs.fgi_marker.is_some()
                    || matches!(&attrs.classification, Some(MarkingClassification::Fgi(_)))
            }
            TOK_US_CLASSIFIED => attrs.us_classification().is_some(),
            // Issue #524 Phase 3: grammar-shape sentinel firing on
            // US collateral classification (any of Restricted /
            // Confidential / Secret / TopSecret). Used as the
            // trigger for `CLOSURE_RELIDO_US_CLASS` to gate the
            // implicit-RELIDO closure to collateral classified
            // content (§H.8 p154 carves out unclassified). Fires
            // on the Conflict variant whose US side is collateral
            // classified — `us_classification()` returns the
            // resolved US side for Conflict. Pinned by
            // `phase3_closure_pin::us_class_fires_on_collateral_levels`
            // and `phase3_closure_pin::us_class_excluded_for_unclassified`.
            //
            // Why a trigger gate (vs. a suppressor): the trigger
            // predicate is upward-closed (adding more facts to a
            // collateral-classified marking doesn't make this stop
            // firing), preserving closure-operator monotonicity
            // per the `MarkingScheme::closure` contract. Encoding
            // "Us is not Unclassified" as a suppressor would have
            // been anti-monotone in the same way that the broader
            // "no other dissem" qualifier was (Copilot HIGH on the
            // Phase 3 PR review).
            TOK_US_COLLATERAL_CLASSIFIED => attrs
                .us_classification()
                .is_some_and(|l| l != Classification::Unclassified),
            // `Conflict` deliberately excluded — see fn doc.
            TOK_NON_US_CLASSIFICATION => matches!(
                &attrs.classification,
                Some(
                    MarkingClassification::Fgi(_)
                        | MarkingClassification::Nato(_)
                        | MarkingClassification::Joint(_)
                )
            ),
            // `TOK_IC_DISSEM` and `TOK_NON_IC_DISSEM` have no live
            // consumers — the legacy E018/E019 constraints that
            // would have used them were retired in T035b as
            // over-restrictive. Kept as declared sentinels so any
            // future narrowly-scoped IC/non-IC dissem invariant
            // can dispatch against them without re-adding a
            // `TokenId` constant.
            TOK_IC_DISSEM | TOK_NON_IC_DISSEM => false,
            // T035c-21 PR-A: NODIS / EXDIS live in `non_ic_dissem`.
            // Both are DoS non-IC dissem controls per §H.9 (NODIS p174;
            // EXDIS p172).
            TOK_NODIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Nodis)),
            TOK_EXDIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Exdis)),
            // PR 3b.C (T026c): RELIDO incompatibility sentinels.
            // Pattern mirrors TOK_NOFORN above — scan via
            // `attrs.dissem_iter()` (namespace-agnostic walk over
            // `dissem_us ++ dissem_nato` post PR 9b / FR-046 split) for
            // the matching DissemControl variant. All four variants
            // exist in the generated values.rs; no new marque-ism edits
            // needed (Constitution VII compliance verified).
            TOK_RELIDO => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Relido)),
            TOK_DISPLAY_ONLY => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Displayonly)),
            TOK_ORCON => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Oc)),
            TOK_ORCON_USGOV => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::OcUsgov)),
            // Stage D (T108c) — new IC dissem sentinels for closure-rule triggers:
            TOK_IMCON => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Imc)),
            TOK_DSEN => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Dsen)),
            TOK_RSEN => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Rs)),
            TOK_FOUO => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Fouo)),
            // PR 4b-C Commit 1 — PROPIN / FISA / RAWFISA scan attrs.dissem_us
            // (the DissemControl variants `Pr`, `Fisa`, `Rawfisa`).
            // §H.8 p148 (PROPIN) + §H.8 p161 (FISA / RAWFISA).
            // verified 2026-05-16 against CAPCO-2016.md.
            TOK_PROPIN => attrs.dissem_iter().any(|d| matches!(d, DissemControl::Pr)),
            TOK_FISA => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Fisa)),
            TOK_RAWFISA => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Rawfisa)),
            // Stage D (T108c) — non-IC dissem sentinels for closure-rule triggers:
            TOK_LIMDIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Limdis)),
            TOK_LES => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Les)),
            TOK_SBU => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Sbu)),
            TOK_SSI => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Ssi)),
            // NNPI scans attrs.non_ic_dissem for the Nnpi variant.
            // The CAPCO-2016 manual does not explicitly enumerate NNPI;
            // the in-tree authority is the `NonIcDissem::Nnpi` variant
            // doc-comment in `crates/ism/src/attrs.rs` (NNPI banner-roll-up
            // semantic — propagates regardless of classification).
            TOK_NNPI => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Nnpi)),
            // EYES sentinel for FD&R-set coverage (§H.8 p157). Per
            // Copilot PR 3.7 review pass 3: earlier comments claimed
            // EYES was covered via `CAT_REL_TO` fallthrough, which is
            // false — `CAT_REL_TO` only checks `attrs.rel_to`. EYES is
            // a `DissemControl::Eyes` variant produced by the parser
            // (deprecated 2017-10-01 per §H.8 p157 but still recognized
            // for legacy-input compatibility); this arm provides the
            // satisfies_attrs path that `FDR_DOMINATORS` membership
            // and `is_fdr_dominator` rely on.
            TOK_EYES => attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Eyes)),
            _ => false,
        },
        TokenRef::AnyInCategory(cat) => match *cat {
            CAT_CLASSIFICATION => attrs.classification.is_some(),
            // `Conflict` deliberately excluded — see fn doc.
            CAT_NON_US_CLASSIFICATION => matches!(
                &attrs.classification,
                Some(
                    MarkingClassification::Fgi(_)
                        | MarkingClassification::Nato(_)
                        | MarkingClassification::Joint(_)
                )
            ),
            CAT_JOINT_CLASSIFICATION => {
                matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
            }
            CAT_SCI => !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty(),
            CAT_SAR => attrs.sar_markings.is_some(),
            CAT_AEA => !attrs.aea_markings.is_empty(),
            CAT_FGI_MARKER => {
                // Mirror TOK_FGI_MARKER (above): cover BOTH the
                // dissem-axis explicit-FGI token and the
                // classification-axis MarkingClassification::Fgi case.
                attrs.fgi_marker.is_some()
                    || matches!(&attrs.classification, Some(MarkingClassification::Fgi(_)))
            }
            CAT_DISSEM => attrs.dissem_iter().next().is_some() || !attrs.rel_to.is_empty(),
            // Issue #524 Phase 3 gap fix: `CAT_NON_IC_DISSEM` was
            // previously unreachable (fell through to `_ => false`),
            // silently making `TokenRef::AnyInCategory(CAT_NON_IC_DISSEM)`
            // a dead reference for any closure / constraint that needed
            // category-level non-IC-dissem suppression. Consumed by
            // `CLOSURE_RELIDO_US_CLASS`'s "no other dissem" suppressor
            // list (`marque-applied.md` Section 4.7.5).
            CAT_NON_IC_DISSEM => !attrs.non_ic_dissem.is_empty(),
            CAT_REL_TO => !attrs.rel_to.is_empty(),
            CAT_DECLASSIFY_ON => attrs.declassify_on.is_some(),
            _ => false,
        },
    }
}

/// Route a `Constraint::Custom` by name to its scheme-private
/// predicate helper. Returns an empty `Vec` for unknown names
/// (forward-compat with future catalog entries).
///
/// PR 3b.D (T026d): catalog-row names with the prefixes
/// `class-floor/` or `E058/` are dispatched to
/// [`class_floor_catalog_eval`] over the static
/// [`CLASS_FLOOR_CATALOG`] table. The retired `e022_cnwdi_floor` /
/// `e025_ucni_classification` helpers were absorbed into the
/// catalog's static-table form; their replacement catalog rows
/// (`E058/CNWDI-classification-floor`,
/// `E058/DOD-UCNI-classification-ceiling`,
/// `E058/DOE-UCNI-classification-ceiling`,
/// `E058/SAR-classification-floor`) reuse the walker's `E058`
/// prefix rather than the legacy E022/E025/E027 IDs. Per project
/// memory `feedback_pre_users_no_deprecation_phasing.md`,
/// severity-config back-compat for the legacy IDs is intentionally
/// not preserved; `.marque.toml` keys must use `E058` (walker-level).
pub(crate) fn evaluate_custom_by_attrs(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    if is_class_floor_catalog_name(name) {
        return class_floor_catalog_eval(attrs, name);
    }
    if is_sci_per_system_catalog_name(name) {
        return sci_per_system_catalog_eval(attrs, name);
    }
    match name {
        "E010/HCS-system-constraints" => hcs_system_constraints(attrs, "CAPCO-2016 §H.4 pp 62-66"),
        "E012/dual-classification" => e012_dual_classification(attrs),
        "E014/joint-requires-rel-to-coverage" => e014_joint_rel_to_coverage(attrs),
        "E021/aea-requires-noforn" => e021_aea_requires_noforn(attrs),
        "E024/rd-precedence" => e024_rd_precedence(attrs),
        // W002/us-commingled-with-fgi retired in the PR closing #470.
        // The catalog row + helper are removed in the same commit.
        "capco/joint-requires-usa" => joint_requires_usa(attrs),
        "E038/nodis-or-exdis-requires-noforn" => e038_dos_dissem_requires_noforn(attrs),
        _ => Vec::new(),
    }
}

/// Free-function form of [`CapcoScheme::iter_present_tokens`] that
/// works directly on `&CanonicalAttrs`. Used by the trait impl above
/// AND by [`CapcoScheme::evaluate_named_constraint`]'s
/// `ConflictsWithFamily` dispatch (which receives raw attrs, not a
/// `CapcoMarking` — so it cannot call the trait method that wraps
/// `&marking.0`).
///
/// Per Copilot PR review on PR 3.7 (`evaluate_named_constraint` was
/// silently treating `ConflictsWithFamily` as a no-op): the fast-path
/// dispatch must emit one violation per (LHS, present_token) pair
/// where the family predicate holds — same algorithm as
/// `marque_scheme::constraint::evaluate`'s `ConflictsWithFamily` arm.
///
/// ## Per-variant classification emission (post-#505)
///
/// Each `MarkingClassification` variant emits a distinct concrete
/// sentinel, retiring the pre-#505 asymmetry where NATO was emitted as
/// `AnyInCategory(CAT_NON_US_CLASSIFICATION)` (umbrella-category shape)
/// while FGI and JOINT emitted concrete `TokenRef::Token(...)` values:
///
///   - `Fgi(_)` → `Token(TOK_FGI_MARKER)` (dual-axis — also matched by
///     `attrs.fgi_marker.is_some()`) **AND** `Token(TOK_FGI_CLASS)`
///     (strict classification-axis FGI).
///   - `Nato(_)` → `Token(TOK_NATO_CLASS)`.
///   - `Joint(_)` → `Token(TOK_JOINT)`.
///   - `Us(_)` / `Conflict { .. }` → no emission (US is the default;
///     `Conflict` is E012's concern, not a `ConflictsWithFamily` LHS).
///
/// Family predicates that need to match the per-variant case should use
/// the corresponding `TOK_*_CLASS` sentinel. The `CAT_NON_US_CLASSIFICATION`
/// category remains as the supercategory for the `E015` Requires constraint
/// (`crates/capco/src/scheme/constraints/core_catalog.rs` E015 row uses
/// `AnyInCategory(CAT_NON_US_CLASSIFICATION)` as the LHS of a
/// `Constraint::Requires`, evaluated through [`satisfies_attrs`], **not**
/// through [`collect_present_tokens`]) and for vocabulary admission;
/// it is no longer a `collect_present_tokens` emission target.
pub(crate) fn collect_present_tokens(attrs: &marque_ism::CanonicalAttrs) -> Vec<TokenRef> {
    use marque_ism::{AeaMarking, DissemControl, MarkingClassification, NonIcDissem};
    let mut tokens = Vec::new();

    // Classification tokens — per-variant emission post-#505. See the
    // "Per-variant classification emission" doc block above.
    if let Some(ref cls) = attrs.classification {
        match cls {
            MarkingClassification::Us(_) | MarkingClassification::Conflict { .. } => {}
            MarkingClassification::Fgi(_) => {
                // Dual-axis: `TOK_FGI_MARKER` (matches `fgi_marker.is_some()`
                // OR `Fgi(_)`) so family predicates that read FGI presence
                // regardless of axis still fire.
                tokens.push(TokenRef::Token(TOK_FGI_MARKER));
                // Strict: `TOK_FGI_CLASS` matches the classification-axis
                // variant only, for future `ConflictsWithFamily` rows that
                // need to distinguish classification-axis FGI from
                // dissem-axis fgi_marker.
                tokens.push(TokenRef::Token(TOK_FGI_CLASS));
            }
            MarkingClassification::Nato(_) => {
                tokens.push(TokenRef::Token(TOK_NATO_CLASS));
            }
            MarkingClassification::Joint(_) => {
                tokens.push(TokenRef::Token(TOK_JOINT));
            }
        }
        if cls.effective_level() == marque_ism::Classification::Restricted {
            tokens.push(TokenRef::Token(TOK_RESTRICTED));
        }
    }

    // IC dissemination controls. PR 9b (T132): iterate across both
    // namespaces — the predicate emitter is namespace-agnostic; the
    // `TOK_*` sentinel reflects token identity, not attribution.
    for d in attrs.dissem_iter() {
        let tok = match d {
            DissemControl::Nf => Some(TOK_NOFORN),
            DissemControl::Relido => Some(TOK_RELIDO),
            DissemControl::Displayonly => Some(TOK_DISPLAY_ONLY),
            DissemControl::Oc => Some(TOK_ORCON),
            DissemControl::OcUsgov => Some(TOK_ORCON_USGOV),
            DissemControl::Imc => Some(TOK_IMCON),
            DissemControl::Dsen => Some(TOK_DSEN),
            DissemControl::Rs => Some(TOK_RSEN),
            DissemControl::Fouo => Some(TOK_FOUO),
            DissemControl::Eyes => Some(TOK_EYES),
            DissemControl::Pr => Some(TOK_PROPIN),
            DissemControl::Fisa => Some(TOK_FISA),
            DissemControl::Rawfisa => Some(TOK_RAWFISA),
            // Variants without TOK_* sentinels yet:
            //   Rel, ExemptFromIcd501Discovery
            //
            // DRIFT GUARD: `DissemControl` is `#[non_exhaustive]`. If
            // a future ODNI ISM schema bump adds a new variant, it
            // silently falls through to `None` here — meaning any
            // `Constraint::ConflictsWithFamily` row whose family
            // predicate should match the new control will silently
            // stop firing on it. When adding a new dissem control,
            // also: (a) add a `TOK_*` sentinel above, (b) add the
            // arm here, (c) consider whether existing family
            // predicates (`is_fdr_dominator`, `is_orcon_family`)
            // should include it. The compile-time signal is the
            // missing TOK_*; this code path is the runtime
            // backstop.
            _ => None,
        };
        if let Some(id) = tok {
            tokens.push(TokenRef::Token(id));
        }
    }

    // Non-IC dissemination controls
    for d in attrs.non_ic_dissem.iter() {
        let tok = match d {
            NonIcDissem::Nodis => Some(TOK_NODIS),
            NonIcDissem::Exdis => Some(TOK_EXDIS),
            NonIcDissem::SbuNf => Some(TOK_SBU_NF),
            NonIcDissem::LesNf => Some(TOK_LES_NF),
            NonIcDissem::Limdis => Some(TOK_LIMDIS),
            NonIcDissem::Les => Some(TOK_LES),
            NonIcDissem::Sbu => Some(TOK_SBU),
            NonIcDissem::Ssi => Some(TOK_SSI),
            NonIcDissem::Nnpi => Some(TOK_NNPI),
            // NonIcDissem is non-exhaustive; future variants fall through.
            _ => None,
        };
        if let Some(id) = tok {
            tokens.push(TokenRef::Token(id));
        }
    }
    // Issue #524 Phase 3: emit `AnyInCategory(CAT_NON_IC_DISSEM)` when
    // any non-IC dissem token is present. Mirrors the SCI / SAR /
    // REL TO category-level emission pattern. Closes a latent
    // asymmetry where the category form was unreachable via
    // `collect_present_tokens` while `satisfies_attrs`'s
    // `AnyInCategory` arm resolved it correctly — any future
    // `ConflictsWithFamily` or family-predicate path using
    // `AnyInCategory(CAT_NON_IC_DISSEM)` would silently fail without
    // this emission.
    if !attrs.non_ic_dissem.is_empty() {
        tokens.push(TokenRef::AnyInCategory(CAT_NON_IC_DISSEM));
    }

    // REL TO countries — emit AnyInCategory(CAT_REL_TO) if any country present
    if !attrs.rel_to.is_empty() {
        tokens.push(TokenRef::AnyInCategory(CAT_REL_TO));
    }

    // AEA markings. Issue #407: `DodUcni` and `DoeUcni` now emit
    // distinct sentinels (`TOK_DCNI` and `TOK_UCNI` respectively) so
    // `ConflictsWithFamily` family predicates that need to address
    // one variant without the other can do so without re-walking the
    // AEA axis.
    for a in attrs.aea_markings.iter() {
        let tok = match a {
            AeaMarking::Rd(_) => Some(TOK_RD),
            AeaMarking::Frd(_) => Some(TOK_FRD),
            AeaMarking::Tfni => Some(TOK_TFNI),
            AeaMarking::DoeUcni => Some(TOK_UCNI),
            AeaMarking::DodUcni => Some(TOK_DCNI),
            _ => None,
        };
        if let Some(id) = tok {
            tokens.push(TokenRef::Token(id));
        }
    }

    // SCI controls
    if !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty() {
        tokens.push(TokenRef::AnyInCategory(CAT_SCI));
    }

    // Issue #524 (Phase 1): per-compartment SCI sentinel emission.
    //
    // Mirrors the FGI dual-emit pattern (`TOK_FGI_MARKER` +
    // `TOK_FGI_CLASS` at lines above): a marking with `SI-G` present
    // emits BOTH `AnyInCategory(CAT_SCI)` (already emitted above) AND
    // `Token(TOK_SI_G)` so future `ConflictsWithFamily` rows can
    // dispatch on compartment-level granularity without re-walking
    // `attrs.sci_markings`. Walks `sci_markings` once and emits up to
    // six per-compartment compound sentinels per marking — the count
    // is bounded by the size of the per-marking compartment list.
    //
    // The structural shape (system anchor + compartment identifier)
    // is the witness, not `canonical_enum`: sub-compartmented forms
    // (HCS-P with sub-compartments, TK-BLFH/IDIT/KAND with
    // sub-compartments per §H.4 p68 / p89 / p93 / p97) keep
    // emitting their per-compartment sentinel.
    use marque_ism::SciControlBare;
    for m in attrs.sci_markings.iter() {
        let system = match &m.system {
            marque_ism::SciControlSystem::Published(s) => s,
            _ => continue,
        };
        for compartment in m.compartments.iter() {
            let id = compartment.identifier.as_str();
            let sentinel = match (*system, id) {
                (SciControlBare::Si, "G") => TOK_SI_G,
                (SciControlBare::Hcs, "O") => TOK_HCS_O,
                (SciControlBare::Hcs, "P") => TOK_HCS_P,
                (SciControlBare::Tk, "BLFH") => TOK_TK_BLFH,
                (SciControlBare::Tk, "IDIT") => TOK_TK_IDIT,
                (SciControlBare::Tk, "KAND") => TOK_TK_KAND,
                _ => continue,
            };
            tokens.push(TokenRef::Token(sentinel));
            // Issue #524 (Phase 2): HCS-P with at least one sub-
            // compartment emits the additional grammar-shape sentinel
            // `TOK_HCS_P_SUB`. §H.4 p68 implies NOFORN + ORCON for the
            // sub-compartmented form, distinct from the bare HCS-P
            // semantics at §H.4 p66.
            if sentinel == TOK_HCS_P && !compartment.sub_compartments.is_empty() {
                tokens.push(TokenRef::Token(TOK_HCS_P_SUB));
            }
        }
    }

    // SAR markings
    if attrs.sar_markings.is_some() {
        tokens.push(TokenRef::AnyInCategory(CAT_SAR));
    }

    // FGI marker
    if attrs.fgi_marker.is_some() {
        tokens.push(TokenRef::Token(TOK_FGI_MARKER));
    }

    tokens
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod sci_compartment_sentinels_pin {
    //! Issue #524 (Phase 1) — per-compartment SCI sentinel pin.
    //!
    //! Verifies the six new sentinels (`TOK_SI_G`, `TOK_HCS_O`,
    //! `TOK_HCS_P`, `TOK_TK_BLFH`, `TOK_TK_IDIT`, `TOK_TK_KAND`)
    //! resolve correctly through [`satisfies_attrs`] and
    //! [`collect_present_tokens`], and route through
    //! [`capco_token_category`] to `CAT_SCI`.
    //!
    //! The structural-witness test (sub-compartmented forms still
    //! resolve) is the load-bearing one: the implementation reads
    //! `SciMarking.system` + `SciMarking.compartments` rather than
    //! `canonical_enum` precisely because `canonical_enum` is `None`
    //! on sub-compartmented markings per
    //! `marque_ism::SciMarking.canonical_enum` doc-comment.
    //!
    //! Authority: §H.4 marking templates — p64 (HCS-O), p66 (HCS-P),
    //! p80 (SI-G), p87 (TK-BLFH), p91 (TK-IDIT), p95 (TK-KAND).
    //! Sub-compartment authority for the structural-witness case:
    //! §H.4 p68 (HCS-P sub), §H.4 p89 (TK-BLFH sub), §H.4 p93
    //! (TK-IDIT sub), §H.4 p97 (TK-KAND sub).

    use super::*;
    use crate::scheme::{
        TOK_HCS_O, TOK_HCS_P, TOK_SI_G, TOK_TK_BLFH, TOK_TK_IDIT, TOK_TK_KAND, capco_token_category,
    };
    use marque_ism::{
        CanonicalAttrs, SciCompartment, SciControlBare, SciControlSystem, SciMarking,
    };
    use marque_scheme::CategoryId;
    use smol_str::SmolStr;

    /// Build a `CanonicalAttrs` carrying a single SciMarking with the
    /// given system and one compartment identifier. The compartment
    /// carries no sub-compartments; `canonical_enum` is `None` because
    /// the test fixture intentionally exercises the structural path,
    /// not the CVE-projection.
    fn attrs_with_sci(system: SciControlBare, comp: &str) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        let compartment = SciCompartment::new(SmolStr::from(comp), Box::<[SmolStr]>::from([]));
        let marking = SciMarking::new(
            SciControlSystem::Published(system),
            Box::<[SciCompartment]>::from([compartment]),
            None,
        );
        a.sci_markings = Box::<[SciMarking]>::from([marking]);
        a
    }

    /// Build a `CanonicalAttrs` carrying a single SciMarking whose
    /// compartment has at least one sub-compartment. Exercises the
    /// design choice that `satisfies_attrs(TOK_X)` reads the
    /// structural shape, not `canonical_enum` (which is `None`
    /// whenever sub-compartments are present per
    /// `marque_ism::SciMarking.canonical_enum`).
    fn attrs_with_sci_sub(system: SciControlBare, comp: &str, sub: &str) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        let compartment = SciCompartment::new(
            SmolStr::from(comp),
            Box::<[SmolStr]>::from([SmolStr::from(sub)]),
        );
        let marking = SciMarking::new(
            SciControlSystem::Published(system),
            Box::<[SciCompartment]>::from([compartment]),
            None,
        );
        a.sci_markings = Box::<[SciMarking]>::from([marking]);
        a
    }

    /// Build a `CanonicalAttrs` carrying a single SciMarking with
    /// **multiple** compartments under one system anchor. Mirrors a
    /// real `HCS-O-P` portion (one SciMarking, system=HCS,
    /// compartments=`["O", "P"]`) per CAPCO-2016 §H.4 p64 commingling
    /// guidance with HCS-P. Used by the multi-compartment dual-emit
    /// test below.
    fn attrs_with_sci_multi_comps(system: SciControlBare, comps: &[&str]) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        let compartments: Box<[SciCompartment]> = comps
            .iter()
            .map(|c| SciCompartment::new(SmolStr::from(*c), Box::<[SmolStr]>::from([])))
            .collect();
        let marking = SciMarking::new(SciControlSystem::Published(system), compartments, None);
        a.sci_markings = Box::<[SciMarking]>::from([marking]);
        a
    }

    /// Each sentinel resolves true on a matching system+compartment.
    /// Compact table-driven form: drift in any of the six pairs
    /// flags the same shared assertion message naming the offending
    /// pair.
    #[test]
    fn each_sentinel_resolves_true_on_matching_marking() {
        let cases: &[(TokenId, SciControlBare, &str)] = &[
            (TOK_SI_G, SciControlBare::Si, "G"),
            (TOK_HCS_O, SciControlBare::Hcs, "O"),
            (TOK_HCS_P, SciControlBare::Hcs, "P"),
            (TOK_TK_BLFH, SciControlBare::Tk, "BLFH"),
            (TOK_TK_IDIT, SciControlBare::Tk, "IDIT"),
            (TOK_TK_KAND, SciControlBare::Tk, "KAND"),
        ];
        for (tok, system, comp) in cases {
            let a = attrs_with_sci(*system, comp);
            assert!(
                satisfies_attrs(&a, &TokenRef::Token(*tok)),
                "sentinel {tok:?} ({system:?}-{comp}) should resolve true",
            );
        }
    }

    /// Each sentinel resolves false on the matching system but a
    /// different compartment identifier (e.g., TOK_SI_G against
    /// SI-X). Pins the compartment-identifier specificity — a
    /// regression that broadened the predicate to any-compartment
    /// fails here.
    #[test]
    fn sentinel_resolves_false_on_wrong_compartment_same_system() {
        let cases: &[(TokenId, SciControlBare, &str)] = &[
            (TOK_SI_G, SciControlBare::Si, "X"),
            (TOK_HCS_O, SciControlBare::Hcs, "ZZZ"),
            (TOK_HCS_P, SciControlBare::Hcs, "Q"),
            (TOK_TK_BLFH, SciControlBare::Tk, "OTHER"),
            (TOK_TK_IDIT, SciControlBare::Tk, "OTHER"),
            (TOK_TK_KAND, SciControlBare::Tk, "OTHER"),
        ];
        for (tok, system, comp) in cases {
            let a = attrs_with_sci(*system, comp);
            assert!(
                !satisfies_attrs(&a, &TokenRef::Token(*tok)),
                "sentinel {tok:?} should NOT resolve when system is \
                 {system:?} but compartment is {comp:?}",
            );
        }
    }

    /// Each sentinel resolves false on a different SCI control
    /// system regardless of compartment match. Pins system specificity
    /// — a regression that ignored the system anchor fails here
    /// (e.g., a bare `has_compartment(m, "G")` walk would pass on
    /// HCS-G when only SI-G should fire TOK_SI_G).
    #[test]
    fn sentinel_resolves_false_on_wrong_system() {
        // HCS markings with each compartment identifier should not
        // fire SI / TK sentinels. SI markings with each compartment
        // should not fire HCS / TK sentinels. TK with foreign
        // identifiers should not fire SI / HCS sentinels.
        let cases: &[(TokenId, SciControlBare, &str)] = &[
            // TOK_SI_G demands SI; HCS-G must not resolve it.
            (TOK_SI_G, SciControlBare::Hcs, "G"),
            (TOK_SI_G, SciControlBare::Tk, "G"),
            // TOK_HCS_O demands HCS; SI-O / TK-O must not resolve it.
            (TOK_HCS_O, SciControlBare::Si, "O"),
            (TOK_HCS_O, SciControlBare::Tk, "O"),
            // TOK_HCS_P demands HCS; SI-P / TK-P must not resolve it.
            (TOK_HCS_P, SciControlBare::Si, "P"),
            (TOK_HCS_P, SciControlBare::Tk, "P"),
            // TOK_TK_* demands TK; HCS / SI variants must not resolve.
            // Both wrong-system mirrors enumerated for each sentinel so
            // the table covers the full sentinel × wrong-system cross
            // product symmetrically.
            (TOK_TK_BLFH, SciControlBare::Si, "BLFH"),
            (TOK_TK_BLFH, SciControlBare::Hcs, "BLFH"),
            (TOK_TK_IDIT, SciControlBare::Si, "IDIT"),
            (TOK_TK_IDIT, SciControlBare::Hcs, "IDIT"),
            (TOK_TK_KAND, SciControlBare::Si, "KAND"),
            (TOK_TK_KAND, SciControlBare::Hcs, "KAND"),
        ];
        for (tok, system, comp) in cases {
            let a = attrs_with_sci(*system, comp);
            assert!(
                !satisfies_attrs(&a, &TokenRef::Token(*tok)),
                "sentinel {tok:?} should NOT resolve when marking is \
                 {system:?}-{comp} (wrong system anchor)",
            );
        }
    }

    /// Each sentinel resolves false on empty `sci_markings`. Trivial
    /// but pins the "no SCI = no firing" property for completeness;
    /// also catches accidental side-effects of routing changes that
    /// might inject a default value.
    #[test]
    fn sentinel_resolves_false_on_empty_sci() {
        let a = CanonicalAttrs::default();
        for tok in [
            TOK_SI_G,
            TOK_HCS_O,
            TOK_HCS_P,
            TOK_TK_BLFH,
            TOK_TK_IDIT,
            TOK_TK_KAND,
        ] {
            assert!(
                !satisfies_attrs(&a, &TokenRef::Token(tok)),
                "sentinel {tok:?} should not resolve on empty sci_markings",
            );
        }
    }

    /// Sub-compartmented markings still fire the per-compartment
    /// sentinel. **Load-bearing pin** for the design choice: a
    /// regression that delegates through `canonical_enum` (which is
    /// `None` for sub-compartmented forms per
    /// `marque_ism::SciMarking.canonical_enum` doc) would silently
    /// under-fire on every real `SI-G sub`, `HCS-P sub`, `TK-BLFH sub`,
    /// `TK-IDIT sub`, `TK-KAND sub` portion. The structural-shape
    /// witness must survive.
    ///
    /// `TOK_SI_G` carries a CAPCO-registered sub-compartment template
    /// at §H.4 p81 (GAMMA [SUB-COMPARTMENT]); HCS-P / TK-BLFH /
    /// TK-IDIT / TK-KAND carry one at §H.4 p68 / p89 / p93 / p97
    /// respectively. `TOK_HCS_O` does NOT have a CAPCO-registered
    /// sub-compartment template in §H.4 — it is still included here
    /// with a synthetic sub-compartment because the implementation
    /// contract (reads structural shape, not `canonical_enum`)
    /// applies uniformly to all six sentinels. A regression that
    /// delegated through `canonical_enum` would also break the
    /// HCS-O case if a future CAPCO revision registers a sub-
    /// compartment template for it; this test pre-emptively pins the
    /// contract for the full sentinel set.
    #[test]
    fn sub_compartmented_markings_still_fire_sentinel() {
        let cases: &[(TokenId, SciControlBare, &str, &str)] = &[
            (TOK_SI_G, SciControlBare::Si, "G", "ABCD"),
            (TOK_HCS_O, SciControlBare::Hcs, "O", "SYNTH"),
            (TOK_HCS_P, SciControlBare::Hcs, "P", "X1"),
            (TOK_TK_BLFH, SciControlBare::Tk, "BLFH", "Y2"),
            (TOK_TK_IDIT, SciControlBare::Tk, "IDIT", "Z3"),
            (TOK_TK_KAND, SciControlBare::Tk, "KAND", "W4"),
        ];
        for (tok, system, comp, sub) in cases {
            let a = attrs_with_sci_sub(*system, comp, sub);
            assert!(
                satisfies_attrs(&a, &TokenRef::Token(*tok)),
                "sub-compartmented {system:?}-{comp} {sub:?} should \
                 fire sentinel {tok:?} (structural witness, not \
                 canonical_enum)",
            );
        }
    }

    /// Multi-compartment markings emit one per-compartment sentinel
    /// per matching compartment. **Load-bearing pin** for the
    /// commingling case: `(S//HCS-O-P)` per CAPCO-2016 §H.4 p64 is
    /// modeled as ONE `SciMarking` with system=HCS and compartments
    /// `["O", "P"]`. The `collect_present_tokens` inner loop must
    /// emit both `TOK_HCS_O` and `TOK_HCS_P` — a regression that
    /// short-circuited the inner loop after the first sentinel hit
    /// would silently drop one.
    #[test]
    fn multi_compartment_markings_emit_all_per_compartment_sentinels() {
        // HCS-O-P case — CAPCO §H.4 p64 commingling guidance.
        let a = attrs_with_sci_multi_comps(SciControlBare::Hcs, &["O", "P"]);
        let emitted = collect_present_tokens(&a);
        assert!(
            emitted.contains(&TokenRef::Token(TOK_HCS_O)),
            "HCS-O-P should emit TOK_HCS_O; got {emitted:?}",
        );
        assert!(
            emitted.contains(&TokenRef::Token(TOK_HCS_P)),
            "HCS-O-P should emit TOK_HCS_P; got {emitted:?}",
        );
        // Symmetric satisfies_attrs path: both per-compartment
        // predicates fire on the same marking.
        assert!(
            satisfies_attrs(&a, &TokenRef::Token(TOK_HCS_O)),
            "HCS-O-P should satisfy TOK_HCS_O predicate",
        );
        assert!(
            satisfies_attrs(&a, &TokenRef::Token(TOK_HCS_P)),
            "HCS-O-P should satisfy TOK_HCS_P predicate",
        );
    }

    /// `collect_present_tokens` emits the per-compartment sentinel
    /// alongside the existing `AnyInCategory(CAT_SCI)`. Mirrors the
    /// FGI dual-emit pattern.
    #[test]
    fn collect_present_tokens_emits_per_compartment_sentinel() {
        let cases: &[(TokenId, SciControlBare, &str)] = &[
            (TOK_SI_G, SciControlBare::Si, "G"),
            (TOK_HCS_O, SciControlBare::Hcs, "O"),
            (TOK_HCS_P, SciControlBare::Hcs, "P"),
            (TOK_TK_BLFH, SciControlBare::Tk, "BLFH"),
            (TOK_TK_IDIT, SciControlBare::Tk, "IDIT"),
            (TOK_TK_KAND, SciControlBare::Tk, "KAND"),
        ];
        for (tok, system, comp) in cases {
            let a = attrs_with_sci(*system, comp);
            let emitted = collect_present_tokens(&a);
            assert!(
                emitted.contains(&TokenRef::Token(*tok)),
                "collect_present_tokens on {system:?}-{comp} should \
                 emit {tok:?}; got {emitted:?}",
            );
            assert!(
                emitted.contains(&TokenRef::AnyInCategory(CAT_SCI)),
                "collect_present_tokens on {system:?}-{comp} should \
                 still emit AnyInCategory(CAT_SCI); got {emitted:?}",
            );
        }
    }

    /// Each new sentinel routes to `CAT_SCI` via
    /// `capco_token_category`. Pins the routing table — the
    /// `fdr_dissem_pin` probe set in `crates/capco/src/vocabulary.rs`
    /// already covers reachability, but this test asserts the
    /// specific category target so a regression that routed one of
    /// the new sentinels to (say) `CAT_DISSEM` fails loudly here.
    #[test]
    fn each_sentinel_routes_to_cat_sci() {
        let probes: &[(TokenId, CategoryId)] = &[
            (TOK_SI_G, CAT_SCI),
            (TOK_HCS_O, CAT_SCI),
            (TOK_HCS_P, CAT_SCI),
            (TOK_TK_BLFH, CAT_SCI),
            (TOK_TK_IDIT, CAT_SCI),
            (TOK_TK_KAND, CAT_SCI),
            // Issue #524 Phase 2: grammar-shape sentinel for HCS-P
            // with at least one sub-compartment. Same category routing
            // as the rest of the SCI sentinels.
            (crate::scheme::TOK_HCS_P_SUB, CAT_SCI),
        ];
        for (tok, expected) in probes {
            assert_eq!(
                capco_token_category(*tok),
                Some(*expected),
                "sentinel {tok:?} should route to {expected:?}",
            );
        }
    }

    /// Phase 2: `TOK_HCS_P_SUB` discriminates bare HCS-P from
    /// HCS-P + at least one sub-compartment.
    ///
    /// - Bare HCS-P (no sub) emits TOK_HCS_P but NOT TOK_HCS_P_SUB.
    /// - HCS-P with sub emits both TOK_HCS_P and TOK_HCS_P_SUB.
    /// - Non-HCS-P (e.g., HCS-O) does not emit TOK_HCS_P_SUB even if
    ///   the marking has sub-compartments on a different compartment.
    ///
    /// Pinning this discrimination is load-bearing because the Phase
    /// 2 `CLOSURE_HCS_P_SUB_IMPLIES_NF_OC` row's correctness depends
    /// on it: bare HCS-P at §H.4 p66 implies NOFORN only, while
    /// HCS-P [SUB] at §H.4 p68 implies NOFORN + ORCON.
    #[test]
    fn tok_hcs_p_sub_discriminates_bare_from_sub() {
        use crate::scheme::TOK_HCS_P_SUB;

        // Bare HCS-P: no sub-compartments.
        let bare = attrs_with_sci(SciControlBare::Hcs, "P");
        let emitted = collect_present_tokens(&bare);
        assert!(
            emitted.contains(&TokenRef::Token(TOK_HCS_P)),
            "bare HCS-P should still emit TOK_HCS_P; got {emitted:?}"
        );
        assert!(
            !emitted.contains(&TokenRef::Token(TOK_HCS_P_SUB)),
            "bare HCS-P (no sub) must NOT emit TOK_HCS_P_SUB; got {emitted:?}"
        );
        assert!(
            !satisfies_attrs(&bare, &TokenRef::Token(TOK_HCS_P_SUB)),
            "satisfies_attrs(TOK_HCS_P_SUB) on bare HCS-P should be false"
        );

        // HCS-P with at least one sub-compartment.
        let mut sub = CanonicalAttrs::default();
        let comp = SciCompartment::new(
            SmolStr::new("P"),
            vec![SmolStr::new("JJJ")].into_boxed_slice(),
        );
        let marking = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Box::new([comp]),
            None,
        );
        sub.sci_markings = Box::new([marking]);
        let emitted_sub = collect_present_tokens(&sub);
        assert!(
            emitted_sub.contains(&TokenRef::Token(TOK_HCS_P)),
            "HCS-P[sub] should still emit TOK_HCS_P; got {emitted_sub:?}"
        );
        assert!(
            emitted_sub.contains(&TokenRef::Token(TOK_HCS_P_SUB)),
            "HCS-P[sub] must emit TOK_HCS_P_SUB; got {emitted_sub:?}"
        );
        assert!(
            satisfies_attrs(&sub, &TokenRef::Token(TOK_HCS_P_SUB)),
            "satisfies_attrs(TOK_HCS_P_SUB) on HCS-P[sub] should be true"
        );

        // Non-HCS-P with sub-compartments on a different compartment
        // (HCS-O with sub) must NOT emit TOK_HCS_P_SUB.
        let mut hcs_o_sub = CanonicalAttrs::default();
        let comp_o = SciCompartment::new(
            SmolStr::new("O"),
            vec![SmolStr::new("SYNTH")].into_boxed_slice(),
        );
        let marking_o = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Box::new([comp_o]),
            None,
        );
        hcs_o_sub.sci_markings = Box::new([marking_o]);
        let emitted_o = collect_present_tokens(&hcs_o_sub);
        assert!(
            !emitted_o.contains(&TokenRef::Token(TOK_HCS_P_SUB)),
            "HCS-O with sub must NOT emit TOK_HCS_P_SUB (sub-compartment \
             is on the O compartment, not P); got {emitted_o:?}"
        );
        assert!(
            !satisfies_attrs(&hcs_o_sub, &TokenRef::Token(TOK_HCS_P_SUB)),
            "satisfies_attrs(TOK_HCS_P_SUB) on HCS-O[sub] should be false"
        );
    }
}
