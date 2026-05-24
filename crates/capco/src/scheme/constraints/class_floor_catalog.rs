// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Class-floor catalog rows: per-marking classification floors.
//!
//! Row order is load-bearing (the topological scheduler breaks ties on
//! declaration order).

use marque_scheme::{Constraint, SectionLetter, capco};

use super::super::class_floor::PASSTHROUGH_CITATION;

// ================================================================
// Class-floor catalog
// ================================================================
//
// Per-marking classification floors: presence of marking M requires the
// page's classification level to be at least F(M). This is *not* part
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
// class-floor predicate doesn't fit. A future change may
// re-classify to a primitive form (e.g.,
// `TokenRef::ClassAtLeast(ClassLevel)` or `Constraint::ClassFloor`)
// once that primitive lands in marque-scheme.
//
// # Why family granularity (~26 rows, not ~38)
//
// The catalog is written at family granularity (HCS-[comp][sub],
// SI-[comp], TK, RD-SG, etc. — pattern-matching family rows,
// not enumerated per-template rows). Family granularity is
// deliberate: clean lattice algebra, stable ImplTable shape
// that survives a future closure-operator landing without
// re-shaping, uniform §-citation discipline. Family-pattern
// matching is implemented in the predicate body
// (`class_floor_catalog_eval`) — each predicate iterates the
// relevant axis (`attrs.sci_markings`, `attrs.aea_markings`,
// etc.) looking for any token matching the family.
//
// # Per-row name and rule-ID
//
// Each catalog row's `name` is the canonical predicate ID
// (`banner.<axis>.<floor|ceiling>-<marking>`). The engine's
// constraint-catalog bridge constructs `RuleId::new("capco", name)`
// directly, so each row is independently configurable in
// `.marque.toml` by that predicate ID. Per-row identification flows via
// the catalog's `name` field into `ConstraintViolation.constraint_label`
// and is referenced in `Diagnostic.message`.
//
// # Citation methodology
//
// Each row's `label` carries a chosen citation. Some rows cite
// operative-authority pages (precedence rules, FD&R-supersession
// anchors, AEA-chain references) rather than the marking-template-body
// page. The marking-body floor language is verifiable in the §H.x
// section body of each marking.

/// The class-floor section of the constraint catalog.
///
/// Returns the 27 class-floor rows in declaration order, ready
/// to be appended to the core catalog by
/// [`build_constraints`](super::build_constraints).
pub(super) fn class_floor_constraints() -> Vec<Constraint> {
    vec![
        // ---- Floor TS — single classification level (5 rows) -
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
        // §G.2 p40 is the authoritative anchor — CAPCO-2016 Table 5
        // (ARH by Registered Marking) lists BALK / BOHEMIA at p40 as
        // registered NATO control markings (not classifications). The
        // manual's Appendix B is the NATO classification ladder
        // appendix, not the BALK/BOHEMIA registration.
        Constraint::Custom {
            name: "banner.classification.floor-balk",
            label: capco(SectionLetter::G, 2, 40),
        },
        Constraint::Custom {
            name: "banner.classification.floor-bohemia",
            label: capco(SectionLetter::G, 2, 40),
        },
        // ---- Floor S — TS-or-S allowed (8 rows) --------------
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
        // CNWDI classification floor.
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
        // ---- Floor C — any classified level (8 rows) --------
        Constraint::Custom {
            name: "banner.classification.floor-si",
            label: capco(SectionLetter::H, 4, 60),
        },
        // SAR classification floor.
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
        // Citation tightened from "§H.7 Appendix B"
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
        // ---- Floor =U — UNCLASSIFIED-only (2 rows; UCNI split) -
        //
        // Split into two rows (DOD UCNI and DOE UCNI) so each carries
        // its own §H.6 sub-page citation.
        Constraint::Custom {
            name: "banner.aea.ceiling-dod-ucni",
            label: capco(SectionLetter::H, 6, 116),
        },
        Constraint::Custom {
            name: "banner.aea.ceiling-doe-ucni",
            label: capco(SectionLetter::H, 6, 118),
        },
        // ---- Unknown-floor passthrough (4 rows) -------------
        //
        // Unknown-floor passthrough policy. Provisional `F(M) = C`
        // (minimal classified). Severity Warn — fired by the walker at
        // the per-row severity stored in the catalog table.
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
