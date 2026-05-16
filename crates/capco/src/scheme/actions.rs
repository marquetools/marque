// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` action helpers. Lifted from the monolithic `scheme.rs`
//! per the issue #466 split plan
//! (`claudedocs/refactor-466/split_proposal.md`).
//!
//! Covers `ReplacementIntent` application, category-level adders/removers,
//! page-context-to-attrs projection, foreign-source extraction, and the
//! Pattern-C strip helpers + companion emitters.

use marque_ism::{CanonicalAttrs, CountryCode, MarkingClassification, PageContext};
use marque_scheme::{
    ApplyIntentError, CategoryId, FactRef, MarkingScheme, ReplacementIntent, TokenId,
};

use super::predicates::{
    dissem_token_id_for_form, dissem_token_span, first_sci_span, infer_companion_form,
    last_dissem_span, us_level,
};
use super::*;

/// Extract the set of foreign country codes contributing to FGI
/// semantics from a `MarkingClassification`.
///
/// G-4c (PR 4b-B 9th-pass follow-up): used by the lattice path's
/// solely-non-US FGI suppression branch to detect source loss when
/// `ClassificationLattice`'s OrdMax winner discards a foreign source
/// observed on a lower-level portion. Mirrors PageContext's
/// `expected_fgi_marker` country-extraction step (`page_context.rs`
/// lines 894–921) so the two projection paths agree on which
/// portions contribute which producers to the FGI axis.
///
/// Per-variant semantic:
/// - `Us(_)`: contributes nothing (US is the home authority).
/// - `Fgi(f)`: contributes every country in `f.countries`.
///   Source-concealed FGI portions (`f.countries.is_empty()`) return
///   `None` — distinct from `Some(empty)` which would mean "no FGI".
///   The `None` sentinel propagates up to the G-4c branch, which
///   then forces `FgiMarker::SourceConcealed` on the output
///   (§H.7 p128: "a document containing portions of both
///   source-concealed FGI and source-acknowledged FGI must have only
///   the 'FGI' marking"). Verified 2026-05-16.
/// - `Nato(_)`: contributes the literal `"NATO"` trigraph (matches
///   `page_context.rs:911-912`).
/// - `Joint(j)`: contributes every non-USA country in `j.countries`
///   (matches `page_context.rs:914-921`).
/// - `Conflict { foreign, .. }`: recurses into the foreign payload
///   so the implicit US classification at `us` is excluded but the
///   foreign side's producers still contribute. Returns the same
///   set as the `foreign` payload would have produced as a stand-
///   alone classification.
///
/// Returns `Option<Vec<CountryCode>>` where:
/// - `None` = "this portion is source-concealed FGI" (distinct signal)
/// - `Some(vec)` = the contributing country codes (possibly empty if
///   the classification is not a foreign system)
///
/// The caller collects into a `BTreeSet` for deduplication.
///
/// P-9-2 (9th-pass): changed return type from `Vec<CountryCode>` to
/// `Option<Vec<CountryCode>>` to propagate the concealed signal.
/// Pre-fix, source-concealed FGI portions returned an empty `Vec`,
/// indistinguishable from "no FGI at all" — the G-4c equality check
/// then silently dropped the concealed signal and could build a
/// synthetic acknowledged marker, contradicting §H.7 p128.
pub(crate) fn extract_foreign_sources(
    c: Option<&MarkingClassification>,
) -> Option<Vec<marque_ism::CountryCode>> {
    use marque_ism::{CountryCode, ForeignClassification};
    let nato_code = || CountryCode::try_new(b"NATO").expect("NATO trigraph is valid");
    match c {
        None | Some(MarkingClassification::Us(_)) => Some(Vec::new()),
        Some(MarkingClassification::Fgi(f)) => {
            if f.countries.is_empty() {
                // Source-concealed FGI: return the sentinel None so callers
                // can detect concealment vs "no foreign source".
                None
            } else {
                Some(f.countries.to_vec())
            }
        }
        Some(MarkingClassification::Nato(_)) => Some(vec![nato_code()]),
        Some(MarkingClassification::Joint(j)) => Some(
            j.countries
                .iter()
                .filter(|c| c.as_str() != "USA")
                .copied()
                .collect(),
        ),
        Some(MarkingClassification::Conflict { foreign, .. }) => match foreign.as_ref() {
            ForeignClassification::Fgi(f) => {
                if f.countries.is_empty() {
                    None
                } else {
                    Some(f.countries.to_vec())
                }
            }
            ForeignClassification::Nato(_) => Some(vec![nato_code()]),
            ForeignClassification::Joint(j) => Some(
                j.countries
                    .iter()
                    .filter(|c| c.as_str() != "USA")
                    .copied()
                    .collect(),
            ),
        },
    }
}

/// Merge two optional `FgiMarker` values, preserving the
/// source-concealed sentinel and unioning the producer country
/// sets when both sides carry acknowledged markers.
///
/// G-5 (PR 4b-B follow-up): pre-fix, the `join_via_lattice` FGI
/// composition discarded `expected_fgi_marker`'s
/// classification-derived producers whenever an explicit FGI marker
/// existed. This helper unions both sources so the lattice output
/// preserves every non-US producer the PageContext path would
/// surface.
pub(crate) fn merge_fgi_markers(
    a: Option<marque_ism::FgiMarker>,
    b: Option<marque_ism::FgiMarker>,
) -> Option<marque_ism::FgiMarker> {
    use marque_ism::FgiMarker;
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        // Source-concealed dominates per §H.7 pp123-124 — bare `FGI`
        // (no LIST) is the most-restrictive marker. Either operand
        // carrying it produces SourceConcealed. CV-4 (PR 4b-B 8th-pass):
        // pre-CV-4 cited `§H.7 p123`; verified 2026-05-16 against
        // CAPCO-2016.md — the §H.7 block begins on p123 but the
        // load-bearing supersession sentence ("If any document
        // contains portions of both source-concealed FGI ... and
        // source-acknowledged FGI ... then only the 'FGI' marking
        // without the source trigraph(s)/tetragraph(s) must appear
        // in the banner line") lands on p124 in the Precedence Rules
        // for Banner Line Guidance block. The page-span citation is
        // the precise reference.
        (Some(FgiMarker::SourceConcealed), _) | (_, Some(FgiMarker::SourceConcealed)) => {
            Some(FgiMarker::SourceConcealed)
        }
        (
            Some(FgiMarker::Acknowledged { countries: c1, .. }),
            Some(FgiMarker::Acknowledged { countries: c2, .. }),
        ) => {
            // Union the producer sets, deduplicated and sorted.
            let mut all: std::collections::BTreeSet<marque_ism::CountryCode> =
                c1.iter().copied().collect();
            all.extend(c2.iter().copied());
            FgiMarker::acknowledged(all)
        }
    }
}

// ---------------------------------------------------------------------------
// Category-predicate / category-action dispatch (for PageRewrite)
// ---------------------------------------------------------------------------
//
// These helpers implement the trigger and action variants of a
// `PageRewrite` against CAPCO's `CapcoMarking`. They're here rather
// than in `marque-scheme` because the variant payloads reference
// `S::Token` and `S::Marking` and each scheme has to project those
// onto its concrete storage. The `CategoryPredicate::Custom` /
// `CategoryAction::Custom` variants still skip this dispatch and let
// the rewrite author supply the closure directly, but cross-category
// rewrites such as CAPCO's NOFORN rule are also supported in
// declarative form here.

/// `CategoryPredicate::Contains { category, token }` evaluator.
///
/// Phase B supports the sample constraint set. Unhandled `(category,
/// token)` pairs return `false` — a safe conservative answer that
/// effectively disables the rewrite rather than silently misfiring.
/// Phase C expands coverage as more rewrites move to the declarative
/// form.
///
/// PR 3c.B Sub-PR 8.F adds `CAT_NON_IC_DISSEM` arms for `TOK_NODIS` and
/// `TOK_EXDIS` so the `capco/nodis-implies-noforn` and
/// `capco/exdis-implies-noforn` PageRewrites' `Contains` triggers can
/// resolve against the `non_ic_dissem` axis. Without this extension the
/// new rewrites would silently never fire (the conservative-`false`
/// fallthrough effectively disables them), making 8.F a no-op
/// masquerading as a fix (design spec §3 "Predicate-evaluator support",
/// Q2 "capco_category_contains silent-disabling root-cause").
///
/// PR 3c.B Sub-PR 8.F.2 extends the same `CAT_NON_IC_DISSEM` block with
/// arms for `TOK_SBU_NF` and `TOK_LES_NF`, scanning the
/// `NonIcDissem::SbuNf` / `NonIcDissem::LesNf` variants. Same shape,
/// same silent-disabling concern — the `capco/sbu-nf-implies-noforn`
/// (§H.9 p178) and `capco/les-nf-implies-noforn` (§H.9 p185) PageRewrite
/// triggers require these arms to resolve.
///
/// The match-arm dispatches on `TokenId` constants for routing and scans
/// the `NonIcDissem` enum variants in `attrs.non_ic_dissem` in the body —
/// the same two-form separation used by the existing `(CAT_DISSEM,
/// TOK_NOFORN)` arm (dispatches on `TOK_NOFORN`, scans
/// `DissemControl::Nf`).
pub(crate) fn capco_category_contains(
    m: &CapcoMarking,
    category: CategoryId,
    token: TokenId,
) -> bool {
    let attrs = &m.0;
    if category == CAT_DISSEM && token == TOK_NOFORN {
        // PR 9b (T132): "Contains NOFORN" is namespace-agnostic — the
        // dissem token is what matters, not its attribution. Scan
        // across both fields via `dissem_iter`.
        return attrs
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    }
    // PR 3c.B Sub-PR 8.F — CAT_NON_IC_DISSEM arms for NODIS and EXDIS.
    // These enable the `capco/nodis-implies-noforn` and
    // `capco/exdis-implies-noforn` PageRewrite triggers to resolve.
    //
    // PR 3c.B Sub-PR 8.F.2 — CAT_NON_IC_DISSEM arms for SBU-NF and
    // LES-NF. Same purpose: enable the `capco/sbu-nf-implies-noforn`
    // and `capco/les-nf-implies-noforn` PageRewrite triggers to
    // resolve against `attrs.non_ic_dissem`. Without these arms,
    // the Pattern A rewrites would silently never fire (the
    // conservative-`false` fallthrough disables them).
    if category == CAT_NON_IC_DISSEM {
        if token == TOK_NODIS {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Nodis));
        }
        if token == TOK_EXDIS {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Exdis));
        }
        if token == TOK_SBU_NF {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::SbuNf));
        }
        if token == TOK_LES_NF {
            return attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::LesNf));
        }
    }
    false
}

/// `CategoryPredicate::Empty { category }` evaluator.
///
/// Unhandled categories return `true` (treated as "non-empty / unknown")
/// so an `Empty` predicate on an unknown category **does not fire**
/// and a rewrite conditioned on it stays inert. This matches
/// [`capco_category_contains`]'s conservative-false stance and avoids
/// misfiring rewrites on categories Phase B doesn't yet inspect.
/// Phase C expands the match arms as more rewrites move into the
/// declarative form.
pub(crate) fn capco_category_has_values(m: &CapcoMarking, category: CategoryId) -> bool {
    let attrs = &m.0;
    match category {
        CAT_REL_TO => !attrs.rel_to.is_empty(),
        CAT_DISSEM => !attrs.dissem_us.is_empty() || !attrs.dissem_nato.is_empty(),
        CAT_NON_IC_DISSEM => !attrs.non_ic_dissem.is_empty(),
        CAT_SCI => !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty(),
        _ => true,
    }
}

/// `CategoryAction::Clear { category }` evaluator.
pub(crate) fn capco_category_clear(m: &mut CapcoMarking, category: CategoryId) {
    let attrs = &mut m.0;
    if category == CAT_REL_TO {
        attrs.rel_to = Box::new([]);
    } else if category == CAT_DISSEM {
        // PR 9b (T132): clearing the dissem category zeroes both
        // namespaces. The CAT_DISSEM axis is namespace-agnostic from
        // the category-id perspective.
        attrs.dissem_us = Box::new([]);
        attrs.dissem_nato = Box::new([]);
    } else if category == CAT_NON_IC_DISSEM {
        attrs.non_ic_dissem = Box::new([]);
    }
    // Other categories: no-op. Phase C expands coverage.
}

/// `CategoryAction::Replace { category, with }` evaluator. The `with`
/// argument supplies a full marking; Phase B copies only the named
/// category's storage out.
pub(crate) fn capco_category_replace(
    m: &mut CapcoMarking,
    category: CategoryId,
    with: &CapcoMarking,
) {
    let attrs = &mut m.0;
    if category == CAT_REL_TO {
        attrs.rel_to = with.0.rel_to.clone();
    } else if category == CAT_DISSEM {
        // PR 9b (T132): replacing the dissem category copies both
        // namespaces from `with`. The two fields are independent
        // post-attribution per CAPCO-2016 p41 — replacing only one
        // would silently drop the other.
        attrs.dissem_us = with.0.dissem_us.clone();
        attrs.dissem_nato = with.0.dissem_nato.clone();
    } else if category == CAT_NON_IC_DISSEM {
        attrs.non_ic_dissem = with.0.non_ic_dissem.clone();
    }
}

/// Apply a single [`ReplacementIntent`] to a [`CapcoMarking`].
///
/// Helper for [`<CapcoScheme as MarkingScheme>::apply_intent`]. Routes
/// the intent through [`capco_token_category`] (for CVE refs) and
/// [`<CapcoScheme as MarkingScheme>::category_of`] (for open-vocab
/// refs) to the per-axis mutators:
///
/// - `FactRemove` → [`apply_fact_remove`] (CAT_DISSEM, CAT_NON_IC_DISSEM,
///   CAT_REL_TO wired for both CVE sentinels and open-vocab country
///   codes; other axes return `IntentInapplicable`).
/// - `FactAdd` → [`apply_fact_add`] (CAT_DISSEM wired in PR 3c.B
///   Sub-PR 8.D.1 for closed-CVE adds; CAT_REL_TO wired in PR 3c.B
///   Sub-PR 8.D.4 for open-vocab CountryCode adds — E014's JOINT
///   co-owner coverage path; other axes return `IntentInapplicable`
///   until their own migration sub-PRs land).
/// - `Recanonicalize` → no fact-set mutation (the engine renders the
///   marking via `render_canonical` to produce the canonical form).
///
/// Per-axis routing tracks the minimum-needed pattern: each wired
/// axis is the one some rule migration actually emits intents
/// against. Other axes (SCI, SAR, JOINT, AEA, classification) are
/// reachable by the routing table but return
/// `Err(IntentInapplicable)` until their migration sub-PRs land.
pub(crate) fn apply_intent_to_marking(
    scheme: &CapcoScheme,
    marking: &mut CapcoMarking,
    intent: &ReplacementIntent<CapcoScheme>,
) -> Result<(), ApplyIntentError> {
    match intent {
        ReplacementIntent::FactRemove { facts, scope: _ } => {
            // Scope discriminates page vs portion projection scope.
            // For the engine-prereq's RELIDO / dissem-axis removals,
            // both scopes route to the same per-axis storage on
            // `CanonicalAttrs` — the page/document distinction is
            // handled by the engine's projection layer, not by
            // `apply_intent`.
            //
            // Multi-fact clusters (e.g. E024's RD/FRD/TFNI atomic chain)
            // iterate through all facts in the SmallVec. Per-fact
            // `IntentInapplicable` is a silent no-op; the whole intent is
            // inapplicable only when no fact applied.
            //
            // Note: `apply_fact_remove` uses `IntentInapplicable` for two
            // distinct sub-cases — "token already absent" (idempotence) and
            // "axis or token not yet wired for FactRemove" (migration stub).
            // Both are silent per-fact no-ops in this loop. The whole-batch
            // `IntentInapplicable` returned when `!any_applied` is the only
            // failure that propagates to the caller.
            let mut any_applied = false;
            for fact in facts {
                let category = scheme
                    .category_of(fact)
                    .ok_or(ApplyIntentError::UnknownToken)?;
                match apply_fact_remove(marking, category, fact) {
                    Ok(()) => any_applied = true,
                    Err(ApplyIntentError::IntentInapplicable) => {
                        // Token absent or axis not yet wired — per-fact no-op;
                        // continue to next fact in the SmallVec.
                    }
                    Err(e) => return Err(e),
                }
            }
            if any_applied {
                Ok(())
            } else {
                Err(ApplyIntentError::IntentInapplicable)
            }
        }
        ReplacementIntent::FactAdd { token, scope: _ } => {
            // PR 3c.B Sub-PR 8.D.1 — first consumer of FactAdd.
            // Routes through `category_of` then to the per-axis adder
            // (`apply_fact_add`). Pre-migration axes (SCI, SAR,
            // JOINT, AEA, REL TO, classification) return
            // `IntentInapplicable` from `apply_fact_add` so the
            // engine drops the fix; same minimum-needed scoping as
            // the FactRemove wiring.
            let category = scheme
                .category_of(token)
                .ok_or(ApplyIntentError::UnknownToken)?;
            apply_fact_add(marking, category, token)
        }
        ReplacementIntent::Recanonicalize { .. } => {
            // No fact-set mutation — the engine renders the marking
            // via render_canonical to produce the canonical form.
            Ok(())
        }
        // #[non_exhaustive] forward-compat guard: unknown future variants
        // are rejected loudly so newly added intents cannot be
        // silently dropped as no-ops without explicit wiring here.
        _ => Err(ApplyIntentError::IntentRejectsLattice),
    }
}

/// Add a single closed-vocab token to the marking's axis.
///
/// Idempotent at the per-intent level: if the token is already
/// present on the target axis, returns `Err(IntentInapplicable)`
/// (per-intent no-op, NOT a hard failure — the batch dispatcher in
/// [`CapcoScheme::apply_intent`] silently skips inapplicable intents
/// and continues the batch). This mirrors [`apply_fact_remove`]'s
/// "absent token is inapplicable" policy: both axes report
/// per-intent inapplicability when the requested mutation is a
/// no-op. The trait contract at
/// [`marque_scheme::MarkingScheme::apply_intent`] (scheme.rs:185-194)
/// is explicit that per-intent inapplicability is not failure; the
/// batch aggregates to `Err(IntentInapplicable)` only when the whole
/// batch produced no mutation.
///
/// Wired axes today:
///
/// - **CAT_DISSEM** (PR 3c.B Sub-PR 8.D.1): closed-CVE FactAdd —
///   E038 (NODIS/EXDIS-requires-NOFORN) emits `FactAdd { TOK_NOFORN,
///   Portion }`; E021 (AEA-requires-NOFORN) emits the same shape.
/// - **CAT_REL_TO** (PR 3c.B Sub-PR 8.D.4): open-vocab FactAdd via
///   `FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(...))` —
///   E014 (JOINT-requires-REL-TO-coverage) emits one FactAdd per
///   missing JOINT co-owner.
///
/// Other axes return `Err(IntentInapplicable)` until their migration
/// sub-PRs land:
///
/// - **CAT_AEA**: `AeaMarking` is a compound structural value, not
///   an atomic token; FactAdd requires the same value-decomposition
///   that blocks AEA FactRemove (queued for the AEA Requires-bucket
///   sub-PR alongside FactRemove).
/// - **CAT_NON_IC_DISSEM / CAT_SCI / CAT_SAR /
///   CAT_JOINT_CLASSIFICATION / CAT_CLASSIFICATION**: no rule
///   currently emits `FactAdd` against these axes; the first rule
///   that does lands the routing alongside its fixtures.
fn apply_fact_add(
    marking: &mut CapcoMarking,
    category: CategoryId,
    token: &FactRef<CapcoScheme>,
) -> Result<(), ApplyIntentError> {
    use marque_ism::DissemControl;

    let attrs = &mut marking.0;

    // CAT_REL_TO is the first axis wired for open-vocab FactAdd
    // (PR 3c.B Sub-PR 8.D.4 — E014 JOINT co-owner coverage). Handle
    // the open-vocab CountryCode branch BEFORE the CVE-only `id`
    // extraction so we don't have to thread the `FactRef` itself
    // through the closed-vocab match below.
    //
    // Other open-vocab adds (SAR program registration, FGI tetragraph
    // addition) land in their own sub-PRs.
    if category == CAT_REL_TO {
        let country = match token {
            FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(c)) => *c,
            // CVE-side TOK_USA is mapped to `CountryCode::USA` for
            // back-compat with E002 (`crates/capco/src/rules.rs:559`),
            // which emits `FactAdd { token: Cve(TOK_USA), scope }` to
            // ensure USA appears in REL TO. Before this arm existed,
            // E002's FactAdd silently no-op'd through the CAT_REL_TO
            // fall-through (returning `IntentInapplicable`) and the
            // dual-population legacy `FixProposal` did the real work;
            // post-PR-3c.B-Sub-PR-8.D.4 the open-vocab path is wired and
            // we honor the existing CVE emission too. Mapping is safe
            // because `CountryCode::USA` is a `const` literal validated
            // against `try_new` at compile time.
            FactRef::Cve(id) if *id == TOK_USA => marque_ism::CountryCode::USA,
            // TOK_REL_TO is the FactRemove "clear whole axis" sentinel
            // (see the doc block on `TOK_REL_TO` above, lines 110–126);
            // FactAdd of this sentinel has no meaning. Return
            // `IntentInapplicable` (per-intent no-op, batch continues)
            // rather than `UnknownToken` (programmer error, batch
            // aborts) — the sentinel is a known token routed correctly,
            // it just has no FactAdd semantic.
            FactRef::Cve(id) if *id == TOK_REL_TO => {
                return Err(ApplyIntentError::IntentInapplicable);
            }
            // Any other token routed to CAT_REL_TO is a programmer
            // error — no other token shape has REL TO axis meaning.
            _ => return Err(ApplyIntentError::UnknownToken),
        };
        if attrs.rel_to.contains(&country) {
            // Per-intent no-op: country already present, no mutation
            // applied. Per the trait contract at
            // `scheme::MarkingScheme::apply_intent` (scheme/src/scheme.rs:185-194)
            // and the CAT_DISSEM precedent below: per-intent
            // inapplicability is NOT failure — the batch loop skips
            // and continues. Returning Ok here would let a redundant-
            // add intent appear as an applied no-op in the audit log.
            return Err(ApplyIntentError::IntentInapplicable);
        }
        let mut next: Vec<CountryCode> = attrs.rel_to.to_vec();
        next.push(country);
        attrs.rel_to = next.into_boxed_slice();
        return Ok(());
    }

    let id = match token {
        FactRef::Cve(id) => *id,
        // Open-vocab adds for SAR program registration / FGI
        // tetragraph addition land in their own sub-PRs. The
        // CountryCode open-vocab branch is handled above under the
        // CAT_REL_TO arm; reaching this fall-through with an open-
        // vocab ref means we're on an axis (SAR, SCI, FGI) that has
        // not yet wired its FactAdd path.
        FactRef::OpenVocab(_) => return Err(ApplyIntentError::IntentInapplicable),
    };

    if category == CAT_DISSEM {
        let target = match id {
            TOK_NOFORN => DissemControl::Nf,
            TOK_RELIDO => DissemControl::Relido,
            TOK_DISPLAY_ONLY => DissemControl::Displayonly,
            TOK_ORCON => DissemControl::Oc,
            TOK_ORCON_USGOV => DissemControl::OcUsgov,
            _ => return Err(ApplyIntentError::UnknownToken),
        };
        // PR 9b (T132): FactAdd on the CAT_DISSEM axis writes to
        // `dissem_us` by default. The CAPCO-2016 p41 reciprocity rule
        // says these tokens are US-attributed in any US-classified
        // marking (the overwhelming majority of FactAdd consumers);
        // for the rare pure-NATO portion, the engine's caller would
        // need a namespace-aware intent (out of scope for PR 9b — see
        // `specs/006-engine-rule-refactor/decisions.md` D9b-1).
        // Presence check spans both namespaces to avoid duplicating a
        // token already attributed to the NATO side.
        if attrs.dissem_iter().any(|d| d == &target) {
            // Per-intent no-op: token already present, no mutation
            // applied. Return `IntentInapplicable` so the batch-level
            // `apply_intent` dispatcher does NOT flip `any_applied =
            // true` for a non-mutation, and a whole-batch redundant
            // add aggregates to `Err(IntentInapplicable)` (engine
            // silently drops the synthesized no-op fix). The trait
            // contract at `scheme::MarkingScheme::apply_intent`
            // (scheme/src/scheme.rs:185-194) is explicit: per-intent
            // inapplicability is NOT a failure — the batch loop skips
            // and continues; whole-batch no-op surfaces as Err so the
            // engine drops the fix. Returning Ok here would let a
            // redundant-add intent appear as an applied no-op in the
            // audit log (Copilot review of PR #372).
            return Err(ApplyIntentError::IntentInapplicable);
        }
        let mut next: Vec<DissemControl> = attrs.dissem_us.to_vec();
        next.push(target);
        // D9b-1 (decisions.md): FactAdd writes to dissem_us unconditionally;
        // pure-NATO portions needing FactAdd on dissem_nato require namespace-
        // aware intent. Deferred to PR 10+ if cross-system translation surfaces
        // the need.
        attrs.dissem_us = next.into_boxed_slice();
        return Ok(());
    }

    // Other categories (CAT_NON_IC_DISSEM, CAT_AEA, CAT_SCI, CAT_SAR,
    // CAT_JOINT_CLASSIFICATION, CAT_CLASSIFICATION): not yet wired
    // for FactAdd. The first rule that needs each axis lands the
    // routing alongside its migration fixtures.
    Err(ApplyIntentError::IntentInapplicable)
}

/// Remove a single closed-vocab token from the marking's axis.
///
/// Returns `Err(IntentInapplicable)` when the token is not present
/// in the axis (idempotence: nothing to remove). The dissem /
/// non-IC-dissem / REL TO axes are wired — PR #370 (8.E.2) and
/// PR #372 (8.D.1) exercise these for `FactRemove` (E041 / RELIDO
/// conflicts) and `FactAdd` (E038) respectively; the AEA arm is
/// reachable but still unwired pending a later sub-PR. Other axes
/// (SCI, SAR, JOINT) are reachable by the routing table but will
/// return `Err(IntentInapplicable)` until their migration sub-PRs
/// land.
fn apply_fact_remove(
    marking: &mut CapcoMarking,
    category: CategoryId,
    token_ref: &FactRef<CapcoScheme>,
) -> Result<(), ApplyIntentError> {
    use marque_ism::{DissemControl, NonIcDissem};

    let attrs = &mut marking.0;

    // CAT_REL_TO open-vocab country-code removal: symmetric with the
    // FactAdd path wired in PR 3c.B Sub-PR 8.D.4. Wired for
    // round-trip symmetry; no current emitter targets per-country
    // FactRemove on REL TO (E053 uses the `TOK_REL_TO` whole-axis-
    // clear sentinel; E002 USA-not-first uses `Recanonicalize`, not
    // FactRemove). Handle the open-vocab branch BEFORE the
    // CVE-only `id` extraction so the closed-vocab match below
    // stays unchanged.
    if category == CAT_REL_TO {
        if let FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(c)) = token_ref {
            if !attrs.rel_to.contains(c) {
                return Err(ApplyIntentError::IntentInapplicable);
            }
            let next: Vec<CountryCode> = attrs.rel_to.iter().copied().filter(|x| x != c).collect();
            attrs.rel_to = next.into_boxed_slice();
            return Ok(());
        }
        // Fall through to the closed-CVE `TOK_USA` / `TOK_REL_TO`
        // sentinel handling below.
    }

    let id = match token_ref {
        FactRef::Cve(id) => *id,
        // Open-vocab removal for SAR program retirement / FGI
        // tetragraph removal lands in the Stage-4 sub-PRs. The
        // CountryCode open-vocab branch is handled above under the
        // CAT_REL_TO arm; reaching this fall-through with an open-
        // vocab ref means we're on an axis (SAR, SCI, FGI) that has
        // not yet wired its FactRemove path.
        FactRef::OpenVocab(_) => return Err(ApplyIntentError::IntentInapplicable),
    };

    if category == CAT_DISSEM {
        let target = match id {
            TOK_NOFORN => DissemControl::Nf,
            TOK_RELIDO => DissemControl::Relido,
            TOK_DISPLAY_ONLY => DissemControl::Displayonly,
            TOK_ORCON => DissemControl::Oc,
            TOK_ORCON_USGOV => DissemControl::OcUsgov,
            // PR 4b-C Commit 3 — TOK_FOUO removal for the
            // `capco/classification-evicts-fouo` and
            // `capco/non-fdr-control-evicts-fouo` Pattern-B + C rows.
            // §H.8 p134 (FOUO Precedence Rules for Banner Line Guidance):
            //   "FOUO is not conveyed in the banner line if the document
            //    is UNCLASSIFIED with FOUO and other dissemination
            //    control markings, excluding any FD&R markings."
            //   "FOUO does not appear in the banner line of classified
            //    documents."
            // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
            // §H.8 p134 (FOUO subsection p131-134 in citation index).
            TOK_FOUO => DissemControl::Fouo,
            _ => return Err(ApplyIntentError::UnknownToken),
        };
        // PR 9b (T132): FactRemove on the CAT_DISSEM axis filters the
        // target token from BOTH namespaces — a removal request is
        // namespace-agnostic at the rule level (the rule says "drop
        // RELIDO", not "drop RELIDO from US"; consumers that need
        // namespace-aware removal would have to plumb a new
        // ReplacementIntent variant — out of scope per PR 9b
        // decision D9b-1).
        let before = attrs.dissem_us.len() + attrs.dissem_nato.len();
        let kept_us: Vec<DissemControl> = attrs
            .dissem_us
            .iter()
            .copied()
            .filter(|d| *d != target)
            .collect();
        let kept_nato: Vec<DissemControl> = attrs
            .dissem_nato
            .iter()
            .copied()
            .filter(|d| *d != target)
            .collect();
        if kept_us.len() + kept_nato.len() == before {
            return Err(ApplyIntentError::IntentInapplicable);
        }
        attrs.dissem_us = kept_us.into_boxed_slice();
        attrs.dissem_nato = kept_nato.into_boxed_slice();
        return Ok(());
    }

    if category == CAT_NON_IC_DISSEM {
        // PR 4b-C Commit 3 — extended LIMDIS / SBU removal for the
        // Pattern-C `capco/limdis-evicted-by-classified` /
        // `capco/sbu-evicted-by-classified` rows. SbuNf / LesNf
        // remain UnknownToken — they are the §H.9 p178 / p185
        // compound-NF variants and Pattern-C strip rows MUST NOT
        // touch them (Pattern §3.5 compound-NF guard); the existing
        // `capco/sbu-nf-implies-noforn` + `capco/les-nf-implies-noforn`
        // rewrites carry NF identity separately.
        //
        // Per the `ApplyIntentError::UnknownToken` doc-comment
        // (`crates/scheme/src/scheme.rs:454-458`), an emitter that
        // targets an unsupported token is treated as a programmer-
        // emission defect: the engine logs the error and drops the
        // fix without panicking.
        let target = match id {
            TOK_NODIS => NonIcDissem::Nodis,
            TOK_EXDIS => NonIcDissem::Exdis,
            // PR 4b-C Commit 3 — §H.9 p170 (LIMITED DISTRIBUTION
            // Precedence Rules for Banner Line Guidance): "Classified
            // documents: LIMDIS does not appear in the banner line."
            // verified 2026-05-16 against CAPCO-2016.md §H.9 p170.
            TOK_LIMDIS => NonIcDissem::Limdis,
            // PR 4b-C Commit 3 — §H.9 p176 (SENSITIVE BUT
            // UNCLASSIFIED Precedence Rules for Banner Line Guidance):
            // "Classified documents: SBU does not appear in the
            // banner line."
            // verified 2026-05-16 against CAPCO-2016.md §H.9 p176.
            TOK_SBU => NonIcDissem::Sbu,
            _ => return Err(ApplyIntentError::UnknownToken),
        };
        let before = attrs.non_ic_dissem.len();
        let kept: Vec<NonIcDissem> = attrs
            .non_ic_dissem
            .iter()
            .copied()
            .filter(|d| *d != target)
            .collect();
        if kept.len() == before {
            return Err(ApplyIntentError::IntentInapplicable);
        }
        attrs.non_ic_dissem = kept.into_boxed_slice();
        return Ok(());
    }

    if category == CAT_REL_TO {
        // Three paths land on this axis today:
        //
        // - `FactRef::OpenVocab(CountryCode(...))`: per-country
        //   removal (handled above before the CVE-id extraction).
        //   Wired by PR 3c.B Sub-PR 8.D.4 for round-trip symmetry
        //   with the E014 FactAdd path; no current emitter targets
        //   FactRemove on a per-country basis (E053 uses the whole-
        //   axis-clear sentinel, E002 USA-not-first uses
        //   Recanonicalize).
        // - `FactRef::Cve(TOK_USA)`: remove only the USA entry from
        //   `attrs.rel_to`.
        // - `FactRef::Cve(TOK_REL_TO)` (PR 3c.B Sub-PR 8.D.2):
        //   whole-axis clear. E053 (NOFORN ⊥ REL TO, §H.8 p145)
        //   emits this sentinel — NOFORN supersedes the entire
        //   REL TO list, not just USA. Analog to the
        //   CAT_NON_IC_DISSEM EXDIS-sentinel path that PR #370
        //   wired.
        match id {
            TOK_USA => {
                let before = attrs.rel_to.len();
                let kept: Vec<CountryCode> = attrs
                    .rel_to
                    .iter()
                    .copied()
                    .filter(|c| c != &CountryCode::USA)
                    .collect();
                if kept.len() == before {
                    return Err(ApplyIntentError::IntentInapplicable);
                }
                attrs.rel_to = kept.into_boxed_slice();
                return Ok(());
            }
            TOK_REL_TO => {
                // Whole-axis clear. Per the trait contract
                // (`crates/scheme/src/scheme.rs:185-194`), an already-
                // empty axis is per-intent inapplicable — return
                // `Err(IntentInapplicable)`. The batch dispatcher
                // aggregates to whole-batch inapplicable only when no
                // intent applied.
                if attrs.rel_to.is_empty() {
                    return Err(ApplyIntentError::IntentInapplicable);
                }
                attrs.rel_to = Box::<[CountryCode]>::default();
                return Ok(());
            }
            _ => return Err(ApplyIntentError::UnknownToken),
        }
    }

    if category == CAT_AEA {
        // PR 3c.B Sub-PR 8.C — E024 atomic-cluster migration.
        // Wire FRD and TFNI removal so the multi-fact FactRemove intent
        // can atomically remove both superseded markings when RD is present.
        // TOK_CNWDI and TOK_UCNI removal are deferred to later sub-PRs
        // (their compound-value decomposition is more complex).
        use marque_ism::AeaMarking;
        let before = attrs.aea_markings.len();
        let kept: Vec<AeaMarking> = match id {
            TOK_FRD => attrs
                .aea_markings
                .iter()
                .filter(|a| !matches!(a, AeaMarking::Frd(_)))
                .cloned()
                .collect(),
            TOK_TFNI => attrs
                .aea_markings
                .iter()
                .filter(|a| !matches!(a, AeaMarking::Tfni))
                .cloned()
                .collect(),
            // TOK_RD removal and other AEA tokens are deferred — the
            // compound RdBlock decomposition is an open question
            // (CNWDI, SIGMA modifiers complicate atomic semantics).
            _ => return Err(ApplyIntentError::IntentInapplicable),
        };
        if kept.len() == before {
            return Err(ApplyIntentError::IntentInapplicable);
        }
        attrs.aea_markings = kept.into_boxed_slice();
        return Ok(());
    }

    // Other categories (SCI, SAR, JOINT, FGI_MARKER, CLASSIFICATION):
    // not yet wired for FactRemove. The first rule that needs each
    // axis lands the routing alongside its migration fixtures.
    Err(ApplyIntentError::IntentInapplicable)
}

/// No-op [`CategoryAction::Custom`] body for Phase-3 stub
/// `PageRewrite` rows whose action would otherwise need a multi-axis
/// or within-axis transform that the Phase-3 declarative surface
/// can't express cleanly (e.g., the §3.4.1 transmutations).
///
/// Runtime page-rewrite dispatch stays in [`PageContext`] until
/// Phase D / Phase E lands real rewrite bodies; until then the
/// action body is a no-op and only the row's `reads` / `writes`
/// axis annotations are consumed (by the engine's topological
/// scheduler, T031–T032). Pairs with [`never_fires`] for triggers.
pub(crate) fn noop_action(_marking: &mut CapcoMarking) {}

/// Pattern-C action body: strip every `AeaMarking::DodUcni` from the
/// AEA axis. Pairs with [`dod_ucni_classified_trigger`].
pub(crate) fn strip_dod_ucni_action(m: &mut CapcoMarking) {
    let attrs = &mut m.0;
    let kept: Vec<marque_ism::AeaMarking> = attrs
        .aea_markings
        .iter()
        .filter(|a| !matches!(a, marque_ism::AeaMarking::DodUcni))
        .cloned()
        .collect();
    attrs.aea_markings = kept.into_boxed_slice();
}

/// Pattern-C action body: strip every `AeaMarking::DoeUcni` from the
/// AEA axis. Pairs with [`doe_ucni_classified_trigger`].
pub(crate) fn strip_doe_ucni_action(m: &mut CapcoMarking) {
    let attrs = &mut m.0;
    let kept: Vec<marque_ism::AeaMarking> = attrs
        .aea_markings
        .iter()
        .filter(|a| !matches!(a, marque_ism::AeaMarking::DoeUcni))
        .cloned()
        .collect();
    attrs.aea_markings = kept.into_boxed_slice();
}

/// Build a `CanonicalAttrs` banner projection from the `expected_*`
/// accessors on `PageContext`. Intentionally narrow: only fills the
/// fields exercised by Phase A's equivalence tests. Other fields land
/// at their defaults, which matches Phase B's goal of handing
/// everything off to scheme-driven aggregation.
#[inline]
pub(crate) fn page_context_to_attrs(ctx: &PageContext) -> CanonicalAttrs {
    let mut out = CanonicalAttrs::default();

    // Destructure `expected_non_ic_dissem` up front so both the
    // non-IC dissem assignment below AND the DISPLAY ONLY defensive
    // clear (which fires when a later `*-implies-noforn` rewrite
    // will inject NOFORN at banner) see the same `needs_nf` flag.
    let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();

    out.classification = ctx
        .expected_classification()
        .map(marque_ism::MarkingClassification::Us);
    out.sci_controls = ctx.expected_sci_controls().into_boxed_slice();
    out.sci_markings = ctx.expected_sci_markings();
    out.sar_markings = ctx.expected_sar_marking();
    out.aea_markings = ctx.expected_aea_markings().into_boxed_slice();
    out.fgi_marker = ctx.expected_fgi_marker();
    // PR 9b (T132): page-rollup composes each dissem namespace
    // independently. CAPCO-2016 p41 reciprocity is intrinsic to each
    // portion's attribution; the page-level union preserves it.
    out.dissem_us = ctx.expected_dissem_us().into_boxed_slice();
    out.dissem_nato = ctx.expected_dissem_nato().into_boxed_slice();
    out.rel_to = ctx.expected_rel_to().into_boxed_slice();
    // DISPLAY ONLY axis (Phase 2 / §D.2 Table 3 rows 18-20, 25-27).
    // Cross-axis intersection over (REL TO ∪ DO) with banner-REL-TO
    // and USA subtraction — see `PageContext::expected_display_only`.
    //
    // Belt-and-suspenders defense against deferred NOFORN injection
    // handled by the page-rewrite layer below: per §H.8 p154 + §D.2
    // Table 3 row 2, NOFORN and DISPLAY ONLY cannot coexist in the
    // projected banner. `expected_display_only` already short-
    // circuits to empty when `needs_nf` is true (NODIS/EXDIS/SBU-NF/
    // LES-NF), but this defensive `.clear()` keeps the scheme-layer
    // invariant explicit and survives a future refactor that drops
    // the PageContext-side short-circuit.
    let mut display_only_to = ctx.expected_display_only();
    if needs_nf {
        display_only_to.clear();
    }
    out.display_only_to = display_only_to.into_boxed_slice();
    out.declassify_on = ctx.expected_declassify_on().cloned();
    out.declass_exemption = ctx.expected_declass_exemption();
    // `needs_nf` is also consumed above to suppress DISPLAY ONLY when
    // a later rewrite will inject NOFORN.
    // NOFORN injection into `out.dissem_us` (post PR 9b / FR-046 split;
    // the field was `out.dissem_controls` pre-split) for the non-IC
    // dissem trigger family (SBU-NF/LES-NF classified-context split, and
    // NODIS/EXDIS imply-NF per CAPCO-2016 §H.9 p172 / p174) is handled at
    // the final-projection layer by the PageRewrites
    // `capco/{sbu-nf,les-nf,nodis,exdis}-implies-noforn`
    // (declared in `CapcoScheme::page_rewrites`). Adding a second
    // injection path here would duplicate work the PageRewrites already
    // do and split the "what does the projected page look like?" answer
    // across two code paths. The PageRewrites are authoritative for final
    // mutations on CAT_DISSEM; this function only assembles the
    // intermediate snapshot from raw portion reads. `out.rel_to` (set on
    // the line above) is consistent with the post-rewrite state via the
    // `expected_rel_to` short-circuit that fires whenever `needs_nf` is
    // true.
    out.non_ic_dissem = non_ic.into_boxed_slice();

    out
}

/// Build a diagnostic that points at `anchor_span` (the offending SCI
/// token) with a structural `FixIntent::FactAdd` fix at the marking
/// scope. Diagnostic span and fix-scope span intentionally differ:
/// the user sees the SCI marking that triggered the requirement; the
/// engine's `synthesize_fixes` path applies the intent to the parsed
/// marking covering `candidate_span` and re-renders the canonical
/// bytes via `apply_intent` + `render_canonical`. Same
/// diagnostic-vs-fix-scope split used by `SarPortionFormRule` (E026).
///
/// Falls back to `Severity::Error` no-fix when no dissem block exists
/// — inserting a whole new dissem category from rule context is
/// unsafe (the structural addition has no existing block to compose
/// with for canonical re-rendering). Same policy as E040.
//
// 8 args is the irreducible carrying capacity: id/severity for the
// catalog row, anchor_span/candidate_span for the diagnostic-vs-fix
// span split, last_dissem for the anchor lookup, token/message/citation
// for the emission. Folding into a struct would shift the count
// without reducing it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_companion_insert(
    rule: marque_rules::RuleId,
    severity: marque_rules::Severity,
    anchor_span: marque_ism::Span,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    last_dissem: Option<marque_ism::Span>,
    token: &str,
    message: String,
    citation: &'static str,
) -> marque_rules::Diagnostic<CapcoScheme> {
    use marque_rules::{
        Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate,
        Severity,
    };
    use marque_scheme::{FactRef, ReplacementIntent};
    let token_id = match dissem_token_id_for_form(token) {
        Some(id) => id,
        None => {
            // Unrecognized surface form — fail loudly with a no-fix
            // diagnostic rather than silently substituting NOFORN.
            // In normal flow this is unreachable (the catalog rows
            // pass `form.noforn()` / `form.orcon()` which return one
            // of the six recognized forms); reaching this arm means
            // a new surface form was added without updating the
            // lookup, which is a programming error worth surfacing.
            tracing::warn!(
                target: "marque_capco::scheme",
                token = token,
                "emit_companion_insert: unrecognized dissem-control surface form; emitting no-fix Error diagnostic"
            );
            return Diagnostic::info(rule, Severity::Error, anchor_span, message, citation);
        }
    };
    match last_dissem {
        Some(_dissem_span) => {
            // Insert the companion token via a `FactAdd` intent.
            // `fix_scope` is the caller-derived scope: `Scope::Portion`
            // for portion candidates, `Scope::Page` for banner
            // candidates (the banner roll-up's per-page projection).
            // Both `NF`/`NOFORN` and `OC`/`ORCON`/`OC-USGOV`/
            // `ORCON-USGOV` resolve to the same canonical `TokenId`
            // per CVE — the engine's `render_canonical` decides
            // surface form from the inferred companion form.
            let intent = FixIntent::<CapcoScheme> {
                replacement: ReplacementIntent::FactAdd {
                    token: FactRef::Cve(token_id),
                    scope: fix_scope,
                },
                confidence: Confidence::strict(0.9),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            };
            Diagnostic::with_fix_at_span(
                rule,
                severity,
                anchor_span,
                candidate_span,
                message,
                citation,
                intent,
            )
        }
        None => {
            // No dissem block — escalate to Error with no fix.
            Diagnostic::info(rule, Severity::Error, anchor_span, message, citation)
        }
    }
}

// ---------------------------------------------------------------------------
// Per-row Custom-kind emit closures (rows #1, #3, #4)
// ---------------------------------------------------------------------------

/// Row #1 — HCS-O companions: requires ORCON + NOFORN, forbids
/// ORCON-USGOV. §H.4 p64.
pub(crate) fn emit_hcs_o_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc)
        || attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let has_noforn = attrs.dissem_iter().any(|d| d == &DissemControl::Nf);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            last_dissem,
            form.orcon(),
            "HCS-O requires ORCON (§H.4 p64)".to_owned(),
            row.citation,
        ));
    }
    if !has_noforn {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            last_dissem,
            form.noforn(),
            "HCS-O requires NOFORN (§H.4 p64)".to_owned(),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule: RULE_E059,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: "HCS-O forbids ORCON-USGOV (§H.4 p64) — replace with ORCON".to_owned(),
            citation: row.citation,
            original: text.to_owned(),
            replacement: form.orcon().to_owned(),
            confidence: 0.9,
            migration_ref: None,
        }));
    }
    out
}

/// Row #3 — HCS-P sub-compartment companions: requires ORCON, forbids
/// ORCON-USGOV. §H.4 p68. NOFORN is enforced by row #2 (HCS-P NOFORN)
/// which fires on any HCS-P including sub-compartmented variants, so
/// it is not duplicated here.
pub(crate) fn emit_hcs_p_sub_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc)
        || attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            last_dissem,
            form.orcon(),
            "HCS-P sub-compartment requires ORCON (§H.4 p68)".to_owned(),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule: RULE_E059,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: "HCS-P sub-compartment forbids ORCON-USGOV (§H.4 p68) — replace with ORCON"
                .to_owned(),
            citation: row.citation,
            original: text.to_owned(),
            replacement: form.orcon().to_owned(),
            confidence: 0.9,
            migration_ref: None,
        }));
    }
    out
}

/// Row #4 — SI-G companions: requires ORCON, forbids ORCON-USGOV.
/// §H.4 p80.
pub(crate) fn emit_si_g_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc)
        || attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            last_dissem,
            form.orcon(),
            "SI-G requires ORCON (§H.4 p80)".to_owned(),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule: RULE_E059,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: "SI-G forbids ORCON-USGOV (§H.4 p80) — replace with ORCON".to_owned(),
            citation: row.citation,
            original: text.to_owned(),
            replacement: form.orcon().to_owned(),
            confidence: 0.9,
            migration_ref: None,
        }));
    }
    out
}

// ---------------------------------------------------------------------------
// CompanionRequired single-token emit (rows #2, #5)
// ---------------------------------------------------------------------------

/// Single-token companion insertion. Used by `CompanionRequired`-kind
/// rows whose only check is "dissem control X must appear; if missing,
/// emit a zero-width-insertion fix at the end of the IC dissem block."
///
/// # Message format
///
/// Diagnostic message is uniformly `"{marking_label} requires
/// {token_name} ({citation})"`, derived entirely from row metadata
/// (`SciPerSystemRow::marking_label`, the caller-provided `token_name`,
/// and `SciPerSystemRow::citation`). This keeps the catalog as the
/// single source of truth for both message-text and citation: a 6th
/// `CompanionRequired` row added in the future inherits the same
/// shape automatically without a per-row branch. The legacy E043 /
/// E051 messages used a slightly different shape (bare `§H.4 p66`,
/// `§H.4 p87, p91, p95` instead of the full `CAPCO-2016 §H.4 …`
/// citation); pre-users (per project policy) means no fixture-stability
/// constraint, so the format is unified rather than carrying a
/// per-row exception table.
pub(crate) fn emit_companion_required(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
    dissem: marque_ism::DissemControl,
    token_name: &'static str,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use marque_ism::Span;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    if attrs.dissem_iter().any(|d| d == &dissem) {
        return Vec::new();
    }
    // ORCON-USGOV satisfies ORCON-presence checks (the OC-USGOV → OC
    // replacement covers the post-fix state). For PR-E rows #2 and #5
    // (NOFORN-only), this branch never trips because the dissem
    // control is `Nf`, not `Oc`. Guard kept for symmetry with the
    // multi-branch helpers; the explicit `dissem == Oc` check is what
    // makes the guard apply only when relevant.
    if dissem == marque_ism::DissemControl::Oc
        && attrs
            .dissem_iter()
            .any(|d| d == &marque_ism::DissemControl::OcUsgov)
    {
        return Vec::new();
    }

    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    let companion_text = match dissem {
        marque_ism::DissemControl::Nf => form.noforn(),
        marque_ism::DissemControl::Oc => form.orcon(),
        // PR-E rows do not currently use other dissem controls; fall
        // back to the abbreviated CVE form for symmetry.
        _ => dissem.as_str(),
    };

    let message = format!(
        "{label} requires {token_name} ({citation})",
        label = row.marking_label,
        citation = row.citation,
    );

    vec![emit_companion_insert(
        RULE_E059,
        row.severity,
        sci_span,
        candidate_span,
        fix_scope,
        last_dissem,
        companion_text,
        message,
        row.citation,
    )]
}
