// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Active NOFORN clears: `capco/noforn-clears-rel-to` (PR 4b-A,
//! the canonical worked example) and `capco/noforn-clears-fdr-family`
//! (DISPLAY ONLY Phase 2). Lifted from the monolithic `rewrites.rs`
//! per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_scheme::{
    CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent, Scope,
};

use super::super::*;

/// The two NOFORN-clears rows in declaration order:
/// `capco/noforn-clears-rel-to` followed by
/// `capco/noforn-clears-fdr-family`.
pub(super) fn noforn_clears_rows() -> Vec<PageRewrite<CapcoScheme>> {
    // `capco/noforn-clears-rel-to` reads `CAT_DISSEM` to look for
    // NOFORN and writes `CAT_REL_TO` to clear it. The CAT_DISSEM
    // read is a real dataflow dependency on entries 5/6a/6b,
    // which write CAT_DISSEM (ORCON-NATO → ORCON, SBU-NF/LES-NF
    // transmutations) — the scheduler must order this rewrite
    // AFTER those entries so the clearer sees the post-
    // transmutation NOFORN state. The CAT_REL_TO read is a
    // self-edge (skipped by the scheduler at
    // `crates/engine/src/scheduler.rs:84-87`), retained as
    // defensive ordering for future REL-TO writers.
    //
    // (REL TO appearing as its own category — rather than as a
    // dissem-control subtype — is an artifact of `CanonicalAttrs`
    // modeling country-list resolution separately; the rewrite
    // semantics treat it as a first-class category that
    // producers can write.)
    const NF_READS: &[marque_scheme::CategoryId] = &[CAT_DISSEM, CAT_REL_TO];
    const NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_REL_TO];

    // `capco/noforn-clears-fdr-family` reads CAT_DISSEM (to find
    // both the NOFORN trigger and the RELIDO / EYES / DISPLAY ONLY
    // targets) and writes CAT_DISSEM (the multi-fact FactRemove
    // removes the FD&R-family tokens from the same category).
    // Self-edge skipped per the scheduler. Same DAG sibling
    // position as `capco/noforn-clears-rel-to`: both read
    // CAT_DISSEM (post `*-implies-noforn` writes) and operate on
    // axes the *-implies-noforn entries don't touch.
    const NF_CLEARS_FDR_FAMILY_READS: &[marque_scheme::CategoryId] = &[CAT_DISSEM];
    const NF_CLEARS_FDR_FAMILY_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

    // `capco/noforn-clears-display-only-to` reads CAT_DISSEM (to
    // find the NOFORN trigger) and writes CAT_DISPLAY_ONLY_TO (the
    // country-list axis on `attrs.display_only_to`). PR 4b-D.2
    // Copilot R1 #2 added `CAT_DISPLAY_ONLY_TO` so this rewrite
    // could use the symmetric `Clear { CAT_DISPLAY_ONLY_TO }`
    // shape — exactly parallel to `capco/noforn-clears-rel-to`'s
    // `Clear { CAT_REL_TO }`.
    //
    // The rewrite is needed because the closure operator (e.g.
    // `CLOSURE_NOFORN_SAR` on a portion that ALSO carries DISPLAY
    // ONLY USA, GBR) injects NOFORN AFTER `join_via_lattice` has
    // set `attrs.display_only_to` from the per-portion union.
    // Without this rewrite the renderer would emit an inconsistent
    // banner: NOFORN in `dissem_us` AND a populated
    // `display_only_to` country list, violating §H.8 p145.
    //
    // Self-edge on CAT_DISPLAY_ONLY_TO is skipped by the scheduler
    // (no other rewrite reads/writes this axis today).
    const NF_CLEARS_DISPLAY_ONLY_TO_READS: &[marque_scheme::CategoryId] =
        &[CAT_DISSEM, CAT_DISPLAY_ONLY_TO];
    const NF_CLEARS_DISPLAY_ONLY_TO_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISPLAY_ONLY_TO];

    vec![
        // §D.2 Table 3 (FD&R Markings Precedence Rules for Banner
        // Line Roll-Up) Rule #2 specifies that NOFORN supersedes
        // REL TO at banner scope; the §H.8 NOFORN entry (p145)
        // back-references this table via "Refer to Section D.2.,
        // Table 3 FD&R Markings Precedence Rules for Banner Line
        // Roll-Up for guidance" in its Precedence Rules section.
        //
        // Declaration order note: this entry is placed AFTER the
        // `*-implies-noforn` entries (PR 3c.B Sub-PR 8.F + 8.F.2)
        // which write CAT_DISSEM. The Kahn scheduler also enforces
        // this ordering via the `reads/writes` dataflow annotations;
        // matching the declaration order to the topological order
        // ensures both `scheme.project(Scope::Page, …)` (which
        // iterates declaration order) and the scheduler-driven
        // execution path (Phase D/E) produce the same result.
        PageRewrite::declarative(
            "capco/noforn-clears-rel-to",
            "CAPCO-2016 §D.2 Table 3 + §H.8 p145",
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_NOFORN,
            },
            CategoryAction::Clear {
                category: CAT_REL_TO,
            },
            NF_READS,
            NF_WRITES,
        ),
        // `capco/noforn-clears-fdr-family` — NOFORN supersedes
        // every other FD&R-class dissem token at banner scope.
        //
        // §D.2 Table 3 rows 1 + 2: "NF + no other FD&R markings →
        // NOFORN" / "NF + any other FD&R marking ... → NOFORN".
        // Row 2's enumeration covers REL TO, RELIDO, USA/[LIST]
        // EYES ONLY, and DISPLAY ONLY explicitly. §H.8 p154 (RELIDO
        // entry) and §H.8 p157-158 (EYES ONLY entry) make the same
        // exclusion at the marking-relationship level.
        //
        // When NF and any of these other FD&R tokens end up
        // together in the projected CAT_DISSEM (e.g., one portion
        // carries the other-FD&R token and another carries NF, or
        // a `*-implies-noforn` rewrite adds NF after
        // `page_context_to_attrs` unions an FD&R portion in), the
        // banner roll-up must keep NF and drop the other tokens.
        // The PageContext-direct path (`expected_dissem_us` Step 6)
        // handles this for callers that read PageContext accessors
        // directly; this PageRewrite mirrors the same policy for
        // `scheme.project(Scope::Page, …)` callers.
        //
        // The companion `capco/noforn-clears-rel-to` rewrite covers
        // the REL TO country-list axis (CAT_REL_TO); this rewrite
        // covers the CAT_DISSEM tokens. There is no `TOK_REL`
        // constant for the bare `REL` dissem marker (CAPCO uses
        // the country list in CAT_REL_TO as the canonical form),
        // so the bare-`Rel` case is handled only at the
        // PageContext layer where the DissemControl enum is
        // visible.
        //
        // Trigger: `Contains(CAT_DISSEM, TOK_NOFORN)` — fires when
        // NOFORN is in the projected page dissem axis (either via
        // direct portion union or via a `*-implies-noforn` rewrite
        // upstream in declaration order).
        //
        // Action: `Intent(FactRemove { [TOK_RELIDO, TOK_EYES,
        // TOK_DISPLAY_ONLY], Scope::Page })` — surgically removes
        // each FD&R-family token from CAT_DISSEM. Idempotent:
        // FactRemove of an absent token is a per-intent no-op
        // (IntentInapplicable, silent), so most pages experience
        // no effect.
        //
        // Axis annotations: reads `[CAT_DISSEM]`, writes
        // `[CAT_DISSEM]` (self-edge skipped per the scheduler).
        // DAG sibling of `capco/noforn-clears-rel-to`: both read
        // CAT_DISSEM after the `*-implies-noforn` writers and
        // operate on disjoint targets (REL TO country axis vs
        // CAT_DISSEM FD&R tokens).
        PageRewrite::declarative(
            "capco/noforn-clears-fdr-family",
            "CAPCO-2016 §D.2 Table 3 row 2 + §H.8 p154 + §H.8 p157",
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_NOFORN,
            },
            CategoryAction::Intent(ReplacementIntent::FactRemove {
                facts: smallvec::smallvec![
                    FactRef::Cve(TOK_RELIDO),
                    FactRef::Cve(TOK_DISPLAY_ONLY),
                    FactRef::Cve(TOK_EYES),
                ],
                scope: Scope::Page,
            }),
            NF_CLEARS_FDR_FAMILY_READS,
            NF_CLEARS_FDR_FAMILY_WRITES,
        ),
        // `capco/noforn-clears-display-only-to` — companion to
        // `capco/noforn-clears-rel-to` for the `display_only_to`
        // country-list axis. PR 4b-D.2 Copilot R1 #2: pre-fix,
        // closure-injected NOFORN on a portion that also carried
        // DISPLAY ONLY USA, GBR left `attrs.display_only_to`
        // populated even though NOFORN had landed in `dissem_us`
        // (the `fdr-family` row above strips the token but the
        // country list is a separate field). The renderer would
        // then emit an inconsistent banner per §H.8 p145 ("NOFORN
        // ... Cannot be used with REL TO / RELIDO / EYES ONLY /
        // DISPLAY ONLY") + §D.2 Table 3 rows 1-2 (NOFORN dominates
        // the FD&R family).
        //
        // Uses `CategoryAction::Clear { CAT_DISPLAY_ONLY_TO }`
        // symmetrically with the REL TO clearer above; the
        // `CAT_DISPLAY_ONLY_TO` CategoryId was added in PR 4b-D.2
        // Copilot R1 #2 (`crates/capco/src/scheme/mod.rs`) and
        // routed through `capco_category_clear` /
        // `capco_category_has_values`.
        PageRewrite::declarative(
            "capco/noforn-clears-display-only-to",
            "CAPCO-2016 §H.8 p145 + §D.2 Table 3 rows 1-2",
            CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_NOFORN,
            },
            CategoryAction::Clear {
                category: CAT_DISPLAY_ONLY_TO,
            },
            NF_CLEARS_DISPLAY_ONLY_TO_READS,
            NF_CLEARS_DISPLAY_ONLY_TO_WRITES,
        ),
    ]
}
