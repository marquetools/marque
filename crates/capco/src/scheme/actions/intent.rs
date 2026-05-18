// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `ReplacementIntent` dispatch into per-axis fact mutators:
//! [`apply_intent_to_marking`] + private [`apply_fact_add`] /
//! [`apply_fact_remove`]. Lifted from the monolithic `actions.rs`
//! per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_ism::CountryCode;
use marque_scheme::{ApplyIntentError, CategoryId, FactRef, MarkingScheme, ReplacementIntent};

use super::super::*;

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
        // PR 4b-D.2 D22 (decisions.md): when NOFORN is being inserted
        // into dissem_us, route through `DissemSet::with_noforn_injected`
        // so the §H.8 p145 supersession overlay strips dominated FD&R
        // controls (REL TO / RELIDO / DISPLAY ONLY / EYES ONLY) at the
        // injection site. This makes both closure-driven FactAdd
        // (closure_hotpath path) and direct rule-driven FactAdd
        // (E038-NODIS, E021-AEA, etc.) correct by construction:
        // re-insertion of NOFORN is idempotent, and the resulting
        // marking always satisfies the §H.8 p145 invariant
        // ("NOFORN: Cannot be used with REL TO / RELIDO / EYES ONLY /
        // DISPLAY ONLY") + §D.2 Table 3 rows 1-2 + §H.8 p157 (EYES
        // ONLY).
        //
        // The other FactAdd targets (Relido, Displayonly, Oc, OcUsgov)
        // do NOT need supersession routing: §H.8 p145 only specifies
        // NOFORN as a dominator on the FD&R chain. The OC-vs-OC-USGOV
        // §H.8 p136/p140 supersession runs at join time (where both
        // tokens can be observed on different portions); FactAdd of
        // OcUsgov alongside existing Oc is a per-portion config that
        // the lattice will resolve at the next join.
        //
        // Authority: §H.8 p145 (NOFORN: "Cannot be used with REL TO,
        // RELIDO, EYES ONLY, or DISPLAY ONLY") + §D.2 Table 3 rows 1-2
        // + §H.8 p157 (EYES ONLY: NSA-only, retains DissemControl::Eyes
        // through lint per scheme.rs:190).
        if target == DissemControl::Nf {
            let portion_attrs = [attrs.clone()];
            let dissem_set =
                crate::lattice::DissemSet::from_attrs_iter(&portion_attrs).with_noforn_injected();
            attrs.dissem_us = dissem_set.into_boxed_slice();
            return Ok(());
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

/// Apply a single closure-cone fact to `marking`, silencing the three
/// per-fact no-op error variants that closure propagation treats as
/// nominal.
///
/// The closure operator is monotone fact propagation. `Ok(())`,
/// `IntentInapplicable` (already present, or axis not wired for
/// FactAdd), and `UnknownToken` (a marker sentinel like
/// `TOK_NATO_CLASS` or a sentinel that has no FactAdd semantic on
/// its routed category, e.g. `TOK_REL_TO` as a whole-axis-clear
/// sentinel) are all silent no-ops. `UnknownToken` arms on legitimate
/// dispatch paths (e.g. `TOK_NATO_CLASS` as a trigger sentinel that
/// is not itself a cone fact) MUST stay silent here; an
/// `UnknownToken` on a cone fact whose `category_of()` succeeded but
/// whose specific token isn't dispatched is a catalog-authoring
/// error that the warn! call surfaces without crashing.
///
/// `IntentRejectsLattice` indicates a structural fact-set violation
/// that the scheme can't repair via fact-set delta alone — a catalog
/// regression. Today's CAPCO catalog cannot reach this branch (no
/// cone fact targets a lattice-rejecting axis); the panic guards
/// against future cone authors.
pub(crate) fn apply_closure_fact(
    scheme: &CapcoScheme,
    marking: &mut CapcoMarking,
    fact: &FactRef<CapcoScheme>,
) {
    let Some(category) = scheme.category_of(fact) else {
        // Fact isn't addressable (marker sentinel with no category
        // mapping). Closure no-op.
        return;
    };
    match apply_fact_add(marking, category, fact) {
        Ok(()) | Err(ApplyIntentError::IntentInapplicable) => {
            // Ok: fact added.
            // IntentInapplicable: already-present (idempotence) or axis
            //   not yet wired for FactAdd — closure no-op.
        }
        Err(ApplyIntentError::UnknownToken) => {
            // Token has no FactAdd semantic on its routed category.
            // For known sentinel cases (e.g., `TOK_REL_TO` whole-axis-
            // clear sentinel landing on `CAT_REL_TO`) this is expected
            // and silent. For catalog-authoring errors where a cone
            // fact's `category_of()` succeeded but the specific token
            // isn't dispatched by `apply_fact_add`, this is a real bug;
            // surface it at runtime via tracing rather than crashing
            // the hot path.
            tracing::warn!(
                target: "marque_capco::closure",
                ?category,
                fact = ?fact,
                "CapcoScheme::closure: cone fact routed to a known category \
                 but apply_fact_add returned UnknownToken. Either a sentinel \
                 with no FactAdd semantic (expected, silent) or a catalog \
                 regression (a cone fact whose specific token isn't \
                 dispatched). Audit the closure rule whose cone references \
                 this token.",
            );
        }
        Err(ApplyIntentError::IntentRejectsLattice) => {
            // The exhaustive match catches new `ApplyIntentError`
            // variants at compile time, not via silent drop.
            panic!(
                "CapcoScheme::closure: cone fact rejected by lattice during \
                 apply_fact_add — this is a catalog regression; a cone fact \
                 targets an axis whose structural invariant the fact-set delta \
                 cannot uphold. Audit every closure rule whose cone references \
                 this axis."
            );
        }
    }
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
