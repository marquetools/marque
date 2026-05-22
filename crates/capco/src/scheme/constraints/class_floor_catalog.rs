// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3b.D (T026d) class-floor catalog rows per `marque-applied.md`
//! §3.4.6. Lifted from the monolithic `constraints.rs` per the
//! issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).
//!
//! Row order preserved verbatim from the pre-split catalog.

use marque_scheme::{Constraint, SectionLetter, capco};

use super::super::class_floor::PASSTHROUGH_CITATION;

// ================================================================
// PR 3b.D (T026d) — class-floor catalog (§3.4.6)
// ================================================================
//
// Per-marking classification floors per `marque-applied.md`
// §3.4.6: presence of marking M requires the page's
// classification level to be at least F(M). This is *not* part
// of the lattice axis itself (the class chain is
// `OrdMax(TS > CTS > S > NS > C > NC > R > NR > U > NU)`); it
// is a *constraint* over the joint fact-set: the page is
// malformed if M is present and the class level is below F(M).
//
// # Why Constraint::Custom (architectural choice — Option A)
//
// Class-floor RHS is "classification level ≥ F(M)" — a
// partial-order threshold over the OrdMax classification
// chain, not a token-presence assertion. The existing
// `Constraint::Requires` shape is dyadic token-presence; the
// class-floor predicate doesn't fit. PR 3.7 (T108b) may
// revisit and re-classify to a primitive form
// (e.g., `TokenRef::ClassAtLeast(ClassLevel)` or
// `Constraint::ClassFloor`) once that primitive lands in
// marque-scheme. See
// `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md`
// §3 for the architectural rationale.
//
// # Why family granularity (~26 rows, not ~38)
//
// The §3.4.6 author wrote at family granularity (HCS-[comp][sub],
// SI-[comp], TK, RD-SG, etc. — pattern-matching family rows,
// not enumerated per-template rows). Family granularity is
// deliberate: clean lattice algebra, stable ImplTable shape
// that survives PR 3.7's closure-operator landing without
// re-shaping, uniform §-citation discipline. Family-pattern
// matching is implemented in the predicate body
// (`class_floor_catalog_eval`) — each predicate iterates the
// relevant axis (`attrs.sci_markings`, `attrs.aea_markings`,
// etc.) looking for any token matching the family.
//
// # Per-row name and walker rule-ID
//
// The single walker `DeclarativeClassFloorRule` (rule ID
// `E058`) emits all diagnostics. Each catalog row's `name`
// takes one of two forms:
//
//   - `E058/<purpose>` for rows that REPLACE a retired
//     legacy rule. Specifically:
//     `E058/CNWDI-classification-floor` (replaces retired
//     E022), `E058/SAR-classification-floor` (replaces
//     retired E027), `E058/DOD-UCNI-classification-ceiling`
//     and `E058/DOE-UCNI-classification-ceiling` (replace
//     retired E025; split per PM decision so each carries
//     its own §H.6 sub-page citation).
//   - `class-floor/<marking>` for rows with no retired-rule
//     predecessor (e.g., `class-floor/HCS-comp-sub`,
//     `class-floor/SI-comp`, `class-floor/BALK`,
//     `class-floor/passthrough-BUR`).
//
// Per-row identification flows via the catalog's `name`
// field into `ConstraintViolation.constraint_label` and is
// referenced in `Diagnostic.message` for human-readable
// identification.
//
// Severity-config compatibility for the legacy IDs (E022,
// E025, E027) is intentionally NOT preserved. Per project
// memory `feedback_pre_users_no_deprecation_phasing.md`:
// marque is pre-users, so we don't carry alias maps,
// retained namespaces, or phased deprecation.
// `.marque.toml` files keying class-floor severity
// overrides MUST use `E058` (walker-level) — there's no
// per-row severity-override surface in PR D.
//
// # Citation methodology
//
// Each row's `label` carries the §3.4.6 author's chosen
// citation. Some rows cite operative-authority pages
// (precedence rules, FD&R-supersession anchors, AEA-chain
// references) rather than the marking-template-body page; the
// §3.4.6 author's choice is authoritative per
// `marque-applied.md` line 783-808. The marking-body floor
// language is verifiable in the H.x section body of each
// marking; see the planning doc §2 for the verification
// matrix.

/// The PR 3b.D class-floor section of the constraint catalog.
///
/// Returns the 27 class-floor rows in declaration order, ready
/// to be appended to the core catalog by
/// [`build_constraints`](super::build_constraints).
pub(super) fn class_floor_constraints() -> Vec<Constraint> {
    vec![
        // ---- §2.1 Floor TS — single classification level (5 rows) -
        Constraint::Custom {
            name: "banner.classification.floor-hcs-comp-sub",
            label: capco(SectionLetter::H, 4, 60),
        },
        Constraint::Custom {
            name: "banner.classification.floor-si-comp",
            label: capco(SectionLetter::H, 4, 60),
        },
        Constraint::Custom {
            name: "banner.classification.floor-tk-blfh",
            label: capco(SectionLetter::H, 4, 60),
        },
        // PR 9c.1 T134: citation tightened from "§H.7 Appendix B"
        // to "§G.2 p40". §G.2 p40 is the authoritative anchor —
        // CAPCO-2016 Table 5 (ARH by Registered Marking) lists
        // BALK / BOHEMIA at p40 as registered NATO control
        // markings; the December 2010 history note at §H.7 line
        // 4702 confirms they are control markings (not
        // classifications). The §H.7 Appendix B reference was an
        // imprecise pre-PR-9c.1 anchor; the manual's actual
        // Appendix B is the NATO classification ladder
        // appendix, not the BALK/BOHEMIA registration.
        Constraint::Custom {
            name: "banner.classification.floor-balk",
            label: capco(SectionLetter::G, 2, 40),
        },
        Constraint::Custom {
            name: "banner.classification.floor-bohemia",
            label: capco(SectionLetter::G, 2, 40),
        },
        // ---- §2.2 Floor S — TS-or-S allowed (8 rows) --------------
        Constraint::Custom {
            name: "banner.classification.floor-hcs-comp",
            label: capco(SectionLetter::H, 4, 60),
        },
        Constraint::Custom {
            name: "banner.classification.floor-rsv-comp",
            label: capco(SectionLetter::H, 4, 60),
        },
        Constraint::Custom {
            name: "banner.classification.floor-tk",
            label: capco(SectionLetter::H, 4, 60),
        },
        Constraint::Custom {
            name: "banner.aea.floor-rd-sg",
            label: capco(SectionLetter::H, 6, 113),
        },
        Constraint::Custom {
            name: "banner.aea.floor-frd-sg",
            label: capco(SectionLetter::H, 6, 113),
        },
        // CNWDI — replaces retired E022. Per PM directive #5 + the
        // PR 3b.D planning doc §5.2, catalog row names use the
        // walker-prefixed form `E058/<suffix>`. Per
        // `feedback_pre_users_no_deprecation_phasing.md` (marque is
        // pre-users), severity-config back-compat for the retiring
        // E022 rule ID is not preserved — users keying `.marque.toml`
        // at `E022` will need to migrate to `E058`.
        Constraint::Custom {
            name: "banner.aea.floor-cnwdi",
            label: capco(SectionLetter::H, 6, 104),
        },
        Constraint::Custom {
            name: "banner.dissem.floor-rsen",
            label: capco(SectionLetter::H, 8, 149),
        },
        Constraint::Custom {
            name: "banner.dissem.floor-imcon",
            label: capco(SectionLetter::H, 8, 144),
        },
        // ---- §2.3 Floor C — any classified level (8 rows) --------
        Constraint::Custom {
            name: "banner.classification.floor-si",
            label: capco(SectionLetter::H, 4, 60),
        },
        // SAR — replaces retired E027. Walker-prefixed name per PM
        // directive #5.
        Constraint::Custom {
            name: "banner.classification.floor-sar",
            label: capco(SectionLetter::H, 5, 99),
        },
        Constraint::Custom {
            name: "banner.aea.floor-rd",
            label: capco(SectionLetter::H, 6, 104),
        },
        Constraint::Custom {
            name: "banner.aea.floor-frd",
            label: capco(SectionLetter::H, 6, 104),
        },
        Constraint::Custom {
            name: "banner.aea.floor-tfni",
            label: capco(SectionLetter::H, 6, 107),
        },
        // PR 9c.1 T134: citation tightened from "§H.7 Appendix B"
        // to "§H.7 p122". §H.7 p122 is the worked example showing
        // ATOMAL in the AEA axis: `SECRET//RD/ATOMAL//FGI NATO//
        // NOFORN` — the direct, structurally-grounded citation for
        // the canonical AEA-axis placement (paralleling §H.6's
        // RD/CNWDI worked-example citations).
        Constraint::Custom {
            name: "banner.aea.floor-atomal",
            label: capco(SectionLetter::H, 7, 122),
        },
        Constraint::Custom {
            name: "banner.dissem.floor-orcon",
            label: capco(SectionLetter::H, 8, 136),
        },
        Constraint::Custom {
            name: "banner.dissem.floor-eyes-only",
            label: capco(SectionLetter::H, 8, 152),
        },
        // ---- §2.4 Floor =U — UNCLASSIFIED-only (2 rows; UCNI split) -
        //
        // Replaces retired `DeclarativeUcniClassificationRule` (E025).
        // Split per PM decision into two rows (DOD UCNI and DOE UCNI)
        // so each row carries its own §H.6 sub-page citation. Both
        // use the walker-prefixed name `E058/<suffix>`.
        Constraint::Custom {
            name: "banner.aea.ceiling-dod-ucni",
            label: capco(SectionLetter::H, 6, 116),
        },
        Constraint::Custom {
            name: "banner.aea.ceiling-doe-ucni",
            label: capco(SectionLetter::H, 6, 118),
        },
        // ---- §2.6 Unknown-floor passthrough (4 rows) -------------
        //
        // Per `marque-applied.md` §3.4.6 unknown-floor sub-catalog +
        // §3.7 passthrough policy. Provisional `F(M) = C` (minimal
        // classified). Severity Warn (per §3.4.6 Q-3.4.6b) — fired by
        // the walker at the per-row severity stored in the catalog
        // table.
        Constraint::Custom {
            name: "banner.classification.floor-passthrough-bur",
            label: PASSTHROUGH_CITATION,
        },
        Constraint::Custom {
            name: "banner.classification.floor-passthrough-hcs-x",
            label: PASSTHROUGH_CITATION,
        },
        Constraint::Custom {
            name: "banner.classification.floor-passthrough-klm",
            label: PASSTHROUGH_CITATION,
        },
        Constraint::Custom {
            name: "banner.classification.floor-passthrough-mvl",
            label: PASSTHROUGH_CITATION,
        },
    ]
}
