// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Classification axis renderer (US, FGI, NATO, JOINT).
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p15-16 — banner + portion grammar; "For US
//!   information, the first value of a banner line or portion mark is
//!   always the US classification marking. For non-US or Joint
//!   information, the banner line and portion mark must always start
//!   with a double forward slash (`//`) with no interjected space,
//!   followed by the non-US or JOINT classification marking."
//! - CAPCO-2016 §H.1 p49 — US classification levels (UNCLASSIFIED,
//!   CONFIDENTIAL, SECRET, TOP SECRET).
//! - CAPCO-2016 §H.3 p55-58 — non-US, JOINT, NATO classification.
//! - CAPCO-2016 §H.7 p122 — FGI classification (concealed vs.
//!   acknowledged country list).
//!
//! # Canonical forms
//!
//! - Banner US: `SECRET` (full word, see [`Classification::banner_str`]).
//! - Portion US: `S` (abbreviation, see [`Classification::portion_str`]).
//! - Banner FGI: `//GBR SECRET` (acknowledged, single country) or
//!   `//FGI SECRET` (concealed).
//! - Portion FGI: `//GBR S` (acknowledged) or `//FGI S` (concealed),
//!   matching the segregated portion-mark form per §H.7 p122 line
//!   "Authorized Portion Mark (when source is acknowledged and
//!   segregated from US)".
//! - Banner JOINT: `//JOINT SECRET CAN GBR USA` (countries alphabetical
//!   per §H.3 p56 line "Country trigraph codes are listed alphabetically
//!   followed by tetragraph codes in alphabetical order"; USA appears in
//!   alphabetical position, NOT pulled to the front — see canonical
//!   examples §H.3 p56 line `//JOINT TOP SECRET CAN ISR USA` and p58
//!   line `//JOINT SECRET CAN GBR USA`).
//! - Portion JOINT: `//JOINT S CAN GBR USA` (same ordering rules, level
//!   abbreviated).
//! - Banner NATO: `//NATO SECRET` (with `//` prefix per §A.6 p15, e.g.
//!   `//NATO SECRET//NOFORN`). The `//` is part of the non-US classification
//!   token, not prepended by the dispatch loop — this axis writes it.
//! - Portion NATO: `(//NS)` (canonical abbreviation form, e.g. `(//NS//NF)`).
//!   The `//` prefix is required per §A.6 p15 for all non-US classifications.
//!   Without `//`, the strict parser cannot enter the non-US classification
//!   code path.
//!
//! Per §A.6 p15 the leading `//` is part of the non-US / JOINT
//! classification token because it occludes the absent US-classification
//! position. The dispatch loop in `MarkingScheme::render_canonical` does
//! NOT prepend the leading `//` for classification — this axis writes
//! it itself when the classification is non-US or JOINT.
//!
//! # Conflict resolution
//!
//! When the parser populated `MarkingClassification::Conflict { us,
//! foreign }` the page-level projection already promoted the US level
//! and lifted the foreign system into an FGI marker (per CAPCO §H.3
//! p55). The renderer treats `Conflict` as an emit of the US level only
//! — the foreign side flows through the FGI axis.

use core::fmt;

use marque_ism::{
    Classification, FgiClassification, JointClassification, MarkingClassification,
    NatoClassification,
};
use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the classification axis to `out`.
pub(crate) fn render_classification(
    m: &CapcoMarking,
    scope: Scope,
    out: &mut dyn fmt::Write,
) -> fmt::Result {
    let Some(classification) = &m.0.classification else {
        return Ok(());
    };

    match classification {
        MarkingClassification::Us(c) => render_us(*c, scope, out),
        MarkingClassification::Fgi(f) => render_fgi(f, scope, out),
        MarkingClassification::Nato(n) => render_nato(*n, scope, out),
        MarkingClassification::Joint(j) => render_joint(j, scope, out),
        // Per architecture — the projected page state has already
        // promoted the US level out of the conflict and lifted the
        // foreign side into an FGI marker. Render the US level here;
        // the FGI axis renders the foreign material.
        MarkingClassification::Conflict { us, .. } => render_us(*us, scope, out),
    }
}

fn render_us(c: Classification, scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    match scope {
        Scope::Portion => out.write_str(c.portion_str()),
        // Page / Document — banner form.
        _ => out.write_str(c.banner_str()),
    }
}

fn render_fgi(f: &FgiClassification, scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    // `MarkingClassification::Fgi` is the **classification system**,
    // not an FGI content marker. Per CAPCO-2016 §H.7 p122 + §A.6 p15:
    // - Source-acknowledged FGI as classification: `//GBR S` (concise
    //   form — country code(s) immediately precede the level).
    // - Source-concealed FGI as classification: `//FGI S` (per §H.7
    //   p123 — when the originating country is sensitive, the `FGI`
    //   prefix replaces the country list).
    //
    // The `FGI` content marker that appears in addition to (or
    // alongside) the classification — e.g., `SECRET//FGI GBR//REL TO
    // USA, GBR` where SECRET is the US classification and FGI GBR is
    // an FGI content marker — flows through the FGI-marker axis
    // (`render_fgi.rs`), not this branch. The two are populated by
    // different parser paths and the renderer treats them
    // independently.
    out.write_str("//")?;
    if f.countries.is_empty() {
        // Source-concealed (no country list) — emit `FGI` prefix per
        // §H.7 p122.
        out.write_str("FGI ")?;
    } else {
        // Source-acknowledged — emit country list (alpha sorted) as
        // the classification prefix per §H.7 p122 + §A.6 p15.
        // Country list is space-delimited in ascending alphabetic
        // sort order (§A.6 p15-16 grammar applied to FGI per the
        // existing tetragraph / trigraph splits handled in the FGI
        // marker axis).
        // Inline-4 mirrors the FGI country buffer ceiling (FGI rarely
        // lists more than 2-3 source countries).
        let mut codes: SmallVec<[&str; 4]> = f.countries.iter().map(|c| c.as_str()).collect();
        codes.sort_unstable();
        for (i, code) in codes.iter().enumerate() {
            if i > 0 {
                out.write_char(' ')?;
            }
            out.write_str(code)?;
        }
        out.write_char(' ')?;
    }
    let level = match scope {
        Scope::Portion => f.level.portion_str(),
        _ => f.level.banner_str(),
    };
    out.write_str(level)
}

fn render_nato(n: NatoClassification, scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    // Leading `//` per §A.6 p15: "For non-US or Joint information, the banner
    // line and portion mark must always start with a double forward slash (`//`)
    // with no interjected space, followed by the non-US or JOINT classification
    // marking." NATO is non-US, so the `//` is part of the classification axis
    // output — matching the pattern in `render_fgi` and `render_joint`.
    // CAPCO-2016 §H.3 p55 + §G.1 Table 4 pp 36-38.
    out.write_str("//")?;
    let s = match scope {
        Scope::Portion => n.portion_str(),
        _ => n.banner_str(),
    };
    out.write_str(s)
}

fn render_joint(j: &JointClassification, scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    // Leading `//` per §A.6 p15. JOINT level + space-separated
    // country list per §H.3 p56:
    //
    //   "Country trigraph codes are listed alphabetically followed by
    //    tetragraph codes in alphabetical order. Multiple codes are
    //    separated by a single space."
    //
    // USA is always required in the JOINT [LIST] — JOINT is a U.S.
    // classification by definition, and §H.3 p55 / p56 explicitly state
    // "USA is always a co-owner/producer". This is a CONSTRAINT on
    // valid JOINT inputs, not a render-time canonicalization. The
    // renderer renders whatever countries are present in the input
    // model; a future Constraint::Requires entry on CapcoScheme
    // (Commit 7+ catalog work) will fire `FactAdd { token: USA, scope:
    // joint }` when JOINT inputs are missing USA. Until that constraint
    // lands, malformed JOINT inputs lacking USA will render without
    // USA — a follow-up gap, not a render-correctness defect.
    //
    // USA does NOT have a USA-first ordering rule in the JOINT [LIST] —
    // the canonical examples on §H.3 p56 / p58 / p59
    // (`//JOINT TOP SECRET CAN ISR USA`, `//JOINT SECRET CAN GBR USA`,
    // `//JOINT SECRET GBR USA`) all show USA in alphabetical position,
    // not pulled to the front.
    //
    // The USA-first rule belongs to the REL TO [USA, LIST] axis only
    // (§H.8 p150-151) — that is a different list with a different
    // ordering rule. Conflating the two was a Constitution VIII defect
    // caught in pre-flight review.
    //
    // The S003 convention (JOINT-USA-first, gated by config) is layered
    // above the renderer in a future commit's Recanonicalize emit — it
    // is NOT the canonical form per the manual. See the consolidated
    // PR 3c.B plan, Commit 6.
    out.write_str("//JOINT ")?;
    let level = match scope {
        Scope::Portion => j.level.portion_str(),
        _ => j.level.banner_str(),
    };
    out.write_str(level)?;

    // Per §H.3 p56: "Country trigraph codes are listed alphabetically
    // followed by tetragraph codes in alphabetical order." `CountryCode`
    // supports 2-16 bytes — trigraphs (3 chars: CAN, GBR, USA, ...) and
    // tetragraphs (4 chars: NATO, FVEY, ACGU, ...) both fit. Bucket by
    // length, alpha-sort within each bucket, emit trigraphs first then
    // tetragraphs. Mirrors the bucketing in `render_fgi` (§A.6 p16) and
    // `render_rel_to` (§H.8 p150-151).
    //
    // Today's parser populates `JointClassification.countries` only with
    // trigraphs (the parser's JOINT path runs `parse_fgi_classification`
    // gated on `length == 3`), but the renderer must produce canonical
    // output for all valid `CountryCode` values — programmatically
    // constructed JOINT markings (e.g., from a lattice projection or a
    // future parser extension) carrying tetragraphs would otherwise
    // render in a non-canonical interleaved order.
    // Inline-8 / inline-4 matches the REL TO renderer's buckets —
    // JOINT typically lists Five Eyes plus a small tail of additional
    // partners (trigraphs), with international organization
    // tetragraphs spilling cleanly past inline-4.
    let mut trigraphs: SmallVec<[&str; 8]> = SmallVec::new();
    let mut tetragraphs: SmallVec<[&str; 4]> = SmallVec::new();
    for c in j.countries.iter() {
        let s = c.as_str();
        if s.len() == 3 {
            trigraphs.push(s);
        } else {
            // Tetragraphs (4 chars) per §H.3 p56; any other length is
            // defensive — the type permits it and the renderer should
            // emit something deterministic rather than dropping bytes.
            tetragraphs.push(s);
        }
    }
    trigraphs.sort_unstable();
    tetragraphs.sort_unstable();
    for code in trigraphs.into_iter().chain(tetragraphs) {
        out.write_char(' ')?;
        out.write_str(code)?;
    }
    Ok(())
}

// `ForeignClassification` is not currently consumed at this layer — the
// `MarkingClassification::Conflict { foreign, .. }` branch above ignores
// the `foreign` arm because the page-level projection already lifted the
// foreign system into the FGI marker axis (per CAPCO §H.3 p55). When a
// future commit threads cross-axis classification rendering into this
// file (e.g., for portion forms that retain both US and foreign system
// indicators), import `marque_ism::ForeignClassification` at that point.
