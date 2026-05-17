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
/// (`TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`) fall through to `false`;
/// they are declared for future T035b consumption.
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
                // classification-axis case. The closure rule
                // `capco/noforn-if-fgi` would therefore not fire on
                // foreign-classified portions even though the trigger
                // declares both `TOK_FGI_MARKER` and
                // `AnyInCategory(CAT_FGI_MARKER)`.
                attrs.fgi_marker.is_some()
                    || matches!(&attrs.classification, Some(MarkingClassification::Fgi(_)))
            }
            TOK_US_CLASSIFIED => attrs.us_classification().is_some(),
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
            // PR 4b-C Commit 1 — NNPI scans attrs.non_ic_dissem for the
            // Nnpi variant. Closes issue #407. The CAPCO-2016 manual
            // does not explicitly enumerate NNPI; the in-tree authority
            // is `crates/ism/src/attrs.rs:1326` (NNPI banner-roll-up
            // doc-comment, propagates regardless of classification).
            // verified 2026-05-16 against the marque-ism attrs.rs entry.
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
