// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Transmutation stubs (8 rows) from the transmutation
//! roster. Each row declares
//! `Custom(never_fires)` + `Custom(noop_action)`; only the
//! `reads` / `writes` axis annotations are consumed today
//! (by the Kahn scheduler). Lifted from the monolithic
//! `rewrites.rs` per the issue #466 Stage 2 PR A leaf split

use marque_scheme::{CategoryAction, CategoryPredicate, PageRewrite, SectionLetter, capco};

use super::super::actions::noop_action;
use super::super::predicates::never_fires;
use super::super::*;

/// The eight Phase-3 transmutation-stub rows in declaration
/// order: entry 4 (FRD-SIGMA → RD-SIGMA), entries 1-3 (FGI
/// rollups, JOINT cross-class), entry 7 (US-presence promotes
/// bare FGI attribution), entry 5 (ORCON-NATO → US ORCON),
/// entries 6a/6b (SBU-NF / LES-NF transmutations).
pub(super) fn transmutation_stub_rows() -> Vec<PageRewrite<CapcoScheme>> {
    // Entry 4 (consultant §3.4.1 #4): FRD-SIGMA consolidates into
    // RD-SIGMA. Within-axis transform on CAT_AEA — reads and
    // writes the same axis (self-edge skipped per
    // `crates/engine/src/scheduler.rs:84-87`). Topologically
    // independent of every other entry.
    const E4_READS: &[marque_scheme::CategoryId] = &[CAT_AEA];
    const E4_WRITES: &[marque_scheme::CategoryId] = &[CAT_AEA];

    // Entry 1 (consultant §3.4.1 #1): bare-FGI rollup on US
    // contact. Narrow-form reads: CLASS only. Predicate-scan of
    // CAT_FGI_MARKER (for bare-FGI atoms) is documented in the
    // per-entry doc-comment, not in `reads`; declaring it would
    // cycle against entries 2 and 3 (each writes FGI_MARKER and
    // would read it through their own predicate-scan). Reciprocal
    // class raise is parser-side, so CLASS is
    // not in `writes`.
    const E1_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E1_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 2 (consultant §3.4.1 #2): bare-FGI-R rollup on US
    // contact. Narrow-form reads: CLASS only (see Entry 1 note
    // on predicate-scan vs dataflow reads). Class lift to ≥ C is
    // parser-side.
    const E2_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E2_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 3 (consultant §3.4.1 #3): JOINT cross-class rollup.
    // Reads CLASS plus JOINT_CLASSIFICATION (the trigger
    // axis — the JOINT scan IS the read, no predicate-scan
    // doc-comment needed). Writes FGI_MARKER only — §H.3 p57
    // is explicit that JOINT does NOT carry forward to the
    // banner line in US documents, so this rewrite consumes
    // JOINT state without writing it back; class lift is
    // parser-side.
    const E3_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION, CAT_JOINT_CLASSIFICATION];
    const E3_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 7 (consultant §3.4.1 #7): US-presence promotes bare
    // FGI attribution. The CAT_FGI_MARKER read IS structural
    // here — entry 7 consumes the post-rewrite FGI state
    // produced by entries 1, 2, 3 and idempotently promotes any
    // remaining `bare(_, C, _)` to `⊤(C)`. This is the one
    // entry whose FGI_MARKER read is a real dataflow dep, not a
    // predicate-scan artifact, so it stays in `reads`.
    const E7_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION, CAT_FGI_MARKER];
    const E7_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

    // Entry 5 (consultant §3.4.1 #5): ORCON-NATO transmutes to
    // US ORCON on US-class contact. Narrow-form reads: CLASS
    // only. Predicate-scan of CAT_DISSEM (for ORCON-NATO) is
    // doc-comment only; declaring it would cycle against
    // entries 6a/6b (each writes DISSEM and would read it
    // through their own predicate-scan).
    const E5_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E5_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // Entry 6a (consultant §3.4.1 #6): SBU-NF
    // transmutes on classified contact. Narrow-form reads:
    // CLASS only (see Entry 5 note on predicate-scan vs
    // dataflow reads — predicate also scans `non_ic_dissem`
    // field for SBU-NF). Pragmatically, the non-IC dissem axis is
    // folded into CAT_DISSEM until a separate axis is exposed for a
    // `CAT_NON_IC_DISSEM`.
    const E6A_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E6A_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // Entry 6b (consultant §3.4.1 #6): LES-NF
    // transmutes on classified contact. Same narrow-form +
    // axis-mapping pragmatism as Entry 6a. Cited at §H.9 p185
    // (LES-NF is its own §H.9 subsection p185–186, distinct
    // from SBU-NF p178).
    const E6B_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
    const E6B_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    vec![
        // Entry 4 — `capco/frd-sigma-consolidates-into-rd-sigma`.
        //
        // CAPCO-2016 §H.6 states this same precedence rule from
        // two complementary vantages — the RD-SIGMA subsection
        // (§H.6 p108-109) and the FRD-SIGMA subsection (§H.6
        // p113). The two passages are mutual references with
        // identical operational content:
        //
        //   §H.6 p109 (top-of-page continuation from the p108
        //   RD-SIGMA Precedence Rules block): "If both RD and FRD
        //   SIGMA [#] portions are in a document, the RD-SIGMA [#]
        //   marking takes precedence over the FRD-SIGMA [#]
        //   marking in the banner line and all SIGMA numbers are
        //   listed in the RD-SIGMA [#] marking in the banner line,
        //   regardless of whether the information was RD or FRD."
        //
        //   §H.6 p113 (FRD-SIGMA Precedence Rules for Banner Line
        //   Guidance): "If both RD and FRD SIGMA [#] portions are
        //   in a document, the RD-SIGMA [#] marking takes
        //   precedence over the FRD-SIGMA [#] marking in the
        //   banner line and all SIGMA numbers are listed in the
        //   banner line RD-SIGMA [#] marking, regardless of
        //   whether the information was RD or FRD."
        //
        // Within-axis transform — drops FRD-SIGMA atoms from
        // CAT_AEA and folds their numbers into the surviving
        // RD-SIGMA atom. The `lattice::AeaSet` `Product`
        // composition implements this as the union of axis 3
        // (SIGMA numbers) when axis 1's supersession join lands
        // on `Rd`.
        //
        // This row cites BOTH
        // §H.6 p108-109 and §H.6 p113 in this comment so future
        // readers find the rule from whichever subsection they
        // open first. The row's `citation` field stays
        // `§H.6 p113` (the original landing's citation) — the
        // double-citation lives in this doc-comment, not in the
        // citation string, because Marque's audit emitter reads
        // the citation field as a single token. The brief's
        // working name (`capco/rd-coalesces-sigmas`, §H.6 p108)
        // refers to the same rewrite as this row — same algebra,
        // same axis, same body, mutually-cited subsections.
        //
        // Monotonicity: shrinking on CAT_AEA (FRD-SIGMA atoms
        // dropped). Sound under fixed topological order.
        //
        // Phase-3 stub: trigger is `never_fires` and action is
        // `noop_action` because runtime dispatch stays in
        // the hand-coded aggregator until the lattice path (
        // wires the runtime `AeaSet`-driven mutation through
        // `CapcoScheme::project(Scope::Page, ...)`). Only the
        // `reads` / `writes` annotations are consumed (by the
        // scheduler). Topologically independent of every other
        // entry: the AEA axis is otherwise un-written.
        PageRewrite::custom(
            "capco/frd-sigma-consolidates-into-rd-sigma",
            capco(SectionLetter::H, 6, 113),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E4_READS,
            E4_WRITES,
        ),
        // Entry 1 — `capco/fgi-rollup-on-us-contact`.
        // §H.7 p122 (Precedence Rules for Banner Line Guidance):
        // "If any document contains portions of both source-
        // concealed FGI ... and source-acknowledged FGI ..., then
        // only the 'FGI' marking without the source
        // trigraph(s)/tetragraph(s) must appear in the banner
        // line." Trigger surface is bare-FGI portion contacting
        // US-class; effect is FGI banner rollup. Reciprocal
        // class raise is performed at portion-parse-time per
        // the transmutation roster, NOT as a rewrite
        // transform — CLASS is not in `writes`.
        //
        // Monotonicity: monotone-additive on FGI axis (concealed
        // wins over acknowledged; acknowledged unions). CLASS
        // not mutated by this rewrite.
        //
        // Predicate scans `CAT_FGI_MARKER` for bare-FGI atoms.
        // The scan axis is documented here, not in `reads`:
        // entries 1, 2, 3 each trigger on disjoint portion-level
        // patterns and each writes `CAT_FGI_MARKER`; declaring
        // FGI_MARKER as a read here would manufacture a
        // false-cycle against entries 2 and 3. The scheduler's
        // coarse "writes determines order" model is sufficient
        // because the three rewrites' FGI outputs are
        // commutative shape-modifications. If a future change
        // discovers a real dataflow dep on the FGI state, add
        // FGI_MARKER to `reads` then.
        //
        // Shared §-citation with Entry 7 is admissible under
        // this entry is the rollup TRIGGER (bare-FGI
        // contacts US-class); Entry 7 is the idempotent
        // generalization that runs after 1–3 settle.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/fgi-rollup-on-us-contact",
            capco(SectionLetter::H, 7, 122),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E1_READS,
            E1_WRITES,
        ),
        // Entry 2 — `capco/fgi-restricted-rollup-on-us-contact`.
        // §H.7 p122 (Relationship(s) to Other Markings): FGI
        // "may be used with TOP SECRET, SECRET, CONFIDENTIAL,
        // RESTRICTED, UNCLASSIFIED, and other designators ...
        // applied by the non-US originator". Combined with the
        // p123 rollup contract (quoted under Entry 1), bare-
        // FGI-R contacting US-class rolls FGI attribution to
        // `[list]`. Class lift to ≥ C (RESTRICTED is not an
        // authorized US classification, so the reciprocal raise
        // floors at C) is parser-side per
        // the transmutation roster, NOT a rewrite
        // transform — CLASS is not in `writes`.
        //
        // Monotonicity: monotone-additive on FGI axis
        // (R-classified countries union into the trigraph list).
        // Class lift is parser-side and monotone (R → C is
        // upward only).
        //
        // Predicate scans `CAT_FGI_MARKER` for bare-FGI-R atoms.
        // Same predicate-scan-vs-dataflow convention as Entry 1
        // (see Entry 1 doc-comment); FGI_MARKER excluded from
        // `reads` to avoid manufactured cycles against entries
        // 1 and 3.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/fgi-restricted-rollup-on-us-contact",
            capco(SectionLetter::H, 7, 122),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E2_READS,
            E2_WRITES,
        ),
        // Entry 3 — `capco/joint-cross-class-rollup`.
        // §H.3 p57 (Derivative Use, banner-line construction):
        // "Highest classification level of all portions,
        // expressed as a US classification marking. ... The
        // FGI marking including all trigraph/tetragraph codes
        // identified in the JOINT portion(s). REL TO, including
        // all common non-US country trigraph/tetragraph codes
        // identified in the JOINT portions, unless a portion is
        // marked NOFORN, in which case the NOFORN marking must
        // appear in the banner line." JOINT [list] contacting a
        // non-US-class portion rolls FGI attribution to list
        // the non-US JOINT members; banner class is the
        // highest-US-class of all portions, established
        // parser-side per §H.3 p57 + `marque-applied.md`
        // JOINT does NOT carry forward to the
        // banner line in US documents, so this rewrite consumes
        // JOINT state without writing it back, and CLASS is not
        // in `writes`.
        //
        // Monotonicity: monotone-additive on FGI axis (non-US
        // JOINT members union in). Class lift is parser-side
        // and monotone.
        //
        // No predicate-scan note: the `JOINT_CLASSIFICATION`
        // read IS the trigger axis (§H.3 p57 names JOINT
        // explicitly), so it stays in `reads` as a real
        // dataflow read of the page-level JOINT state.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/joint-cross-class-rollup",
            capco(SectionLetter::H, 3, 57),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E3_READS,
            E3_WRITES,
        ),
        // Entry 7 — `capco/us-presence-promotes-bare-fgi-attribution`.
        // §H.7 p122 (Precedence Rules for Banner Line Guidance,
        // quoted under Entry 1) establishes both the trigger and
        // the post-rollup-cleanup contracts. This entry is the
        // idempotent generalization: after entries 1–3 consolidate
        // FGI state, any remaining `bare(_, C, _)` FGI attribution
        // is promoted to a fully-rolled-up `⊤(C)` form.
        //
        // Monotonicity: monotone-additive. `bare(_, C, _) → ⊤(C)`
        // is a join-monotone `FgiSet` promotion; idempotent on
        // already-promoted state.
        //
        // No predicate-scan note: the `CAT_FGI_MARKER` read here
        // IS a real dataflow dependency on entries 1, 2, 3 —
        // entry 7 consumes their post-rewrite FGI state and
        // promotes any remaining `bare(_, C, _)` attribution.
        // This is the one entry in the table whose FGI_MARKER
        // read is structural, not a predicate-scan artifact, so
        // it stays in `reads` and the scheduler orders entry 7
        // after 1, 2, 3.
        //
        // Shared §-citation with Entry 1 is admissible under
        // Entry 1 is the trigger (bare-FGI contacts
        // US-class); this entry is the idempotent cleanup that
        // runs after 1–3 settle.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/us-presence-promotes-bare-fgi-attribution",
            capco(SectionLetter::H, 7, 122),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E7_READS,
            E7_WRITES,
        ),
        // Entry 5 — `capco/orcon-nato-to-us-orcon-on-us-contact`.
        // §H.8 p136 (ORCON Precedence Rules for Banner Line
        // Guidance): "If ORCON and ORCON-USGOV portions are in a
        // document, ORCON takes precedence and is conveyed in
        // the banner line." ORCON-NATO (CAPCO-2016 §G p40,
        // Register Table 5 cross-reference to Appendix B NATO
        // protective markings: "ORCON (NATO dissemination control
        // marking) ... See US ORCON ARH requirements") maps onto
        // the same precedence surface — ORCON-NATO contacting
        // US-class transmutes to US ORCON in the page dissem
        // axis. The §H.8 p136 cite is the primary
        // anchor; the Appendix B mapping (line 895) is the
        // supplementary reference for ORCON-NATO ↔ US ORCON
        // equivalence.
        //
        // Monotonicity: mixed — drops ORCON-NATO (shrinking) and
        // adds ORCON (additive). Sound under fixed topological
        // order.
        //
        // Predicate scans `CAT_DISSEM` for ORCON-NATO. The scan
        // axis is documented here, not in `reads`: entries 5,
        // 6a, 6b each trigger on disjoint dissem-token patterns
        // and each writes `CAT_DISSEM`; declaring DISSEM as a
        // read here would manufacture a false-cycle against
        // 6a and 6b. The DISSEM-writers are commutative
        // shape-modifications on the page dissem set, so the
        // scheduler's "writes determines order" model is
        // sufficient.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/orcon-nato-to-us-orcon-on-us-contact",
            capco(SectionLetter::H, 8, 136),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E5_READS,
            E5_WRITES,
        ),
        // Entry 6a — `capco/sbu-nf-transmutes-on-classified-contact`.
        // §H.9 p178 (SBU-NF Commingling Rule(s) Within a
        // Portion): "The SBU-NF marking is conveyed in the
        // portion mark only if the commingled portion is
        // unclassified and there is no other NOFORN information
        // included in the portion. If there is other NOFORN
        // information in the commingled portion, the 'SBU'
        // marking is used and a NOFORN marking is added, e.g.,
        // (U//NF//SBU)." Class > U drops SBU-NF entirely; class
        // = U replaces SBU-NF with NOFORN + SBU.
        //
        // Monotonicity: mixed — shrinking on class > U;
        // mostly-additive on class = U. Sound under fixed
        // topological order.
        //
        // Predicate scans `CAT_DISSEM` (and the
        // `CanonicalAttrs.non_ic_dissem` field) for SBU-NF.
        // Same predicate-scan-vs-dataflow convention as
        // Entry 5 (see Entry 5 doc-comment); DISSEM excluded
        // from `reads` to avoid manufactured cycles against
        // entries 5 and 6b.
        //
        // Phase-3 axis-mapping pragmatic (plan §8 Q1): SBU/SBU-NF
        // live in `CanonicalAttrs.non_ic_dissem` but no
        // `CAT_NON_IC_DISSEM` CategoryId is exposed yet, so the
        // write axis is `CAT_DISSEM`. A future change may add the
        // separate axis.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/sbu-nf-transmutes-on-classified-contact",
            capco(SectionLetter::H, 9, 178),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E6A_READS,
            E6A_WRITES,
        ),
        // Entry 6b — `capco/les-nf-transmutes-on-classified-contact`.
        // §H.9 p185 (LES-NF Precedence Rules for Banner Line
        // Guidance): "When a
        // classified document contains portions of U//LES-NF,
        // the 'LES' marking is used in the banner line and the
        // NOFORN marking is applied as a Dissemination Control
        // Marking. For example: SECRET//NOFORN//LES." LES-NF
        // transmutes to NOFORN + LES; banner consolidates as
        // `[class]//NOFORN//LES`.
        //
        // Monotonicity: monotone-additive on the dissem axis
        // (NOFORN and LES both added; LES-NF dropped is the
        // input-side projection of the transmutation, not a
        // separate axis shrink). Sound under fixed topological
        // order.
        //
        // Predicate scans `CAT_DISSEM` (and the
        // `CanonicalAttrs.non_ic_dissem` field) for LES-NF.
        // Same predicate-scan-vs-dataflow convention as
        // Entry 5 / 6a; DISSEM excluded from `reads` to avoid
        // manufactured cycles against entries 5 and 6a.
        //
        // Phase-3 axis-mapping pragmatic (plan §8 Q1): same
        // CAT_DISSEM fold as Entry 6a.
        //
        // Phase-3 stub: see Entry 4 doc-comment.
        PageRewrite::custom(
            "capco/les-nf-transmutes-on-classified-contact",
            capco(SectionLetter::H, 9, 185),
            CategoryPredicate::Custom(never_fires),
            CategoryAction::Custom(noop_action),
            E6B_READS,
            E6B_WRITES,
        ),
    ]
}
