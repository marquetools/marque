// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Companion-write deduplication — the post-parse pass that removes
//! duplicate axis entries introduced when the parser's legacy NATO
//! compound canonicalization and its canonical-form recognition both
//! write the same value to the AEA or SCI axis.
//!
//! # The bug this closes
//!
//! PR 9c.1 T134 (`crates/core/src/parser.rs` legacy-compound path,
//! the canonicalization site around line 387) unconditionally pushes
//! an [`AeaMarking::Atomal`] companion into the AEA axis, or a
//! [`SciControlSystem::NatoSap`]-anchored [`SciMarking`] into the SCI
//! axis, whenever the parser recognizes a legacy NATO compound text
//! (`CTSA` / `NSAT` / `CTS-B` / `CTS-BALK` / banner-form equivalents
//! per CAPCO-2016 §G.1 Table 4 p38).
//!
//! When the same marking ALSO carries an explicit canonical
//! ATOMAL / BOHEMIA / BALK block (e.g., `(//NSAT//ATOMAL)` — legacy class
//! `NSAT` plus an explicit AEA `ATOMAL`), the parser pushes the same
//! value twice: once from the legacy-compound canonicalization, once
//! from the canonical-form recognition pass over the AEA / SCI block
//! that follows. Result on `ParsedAttrs`:
//!
//! - `aea_markings = [Atomal, Atomal]`, or
//! - `sci_markings = [NatoSap(Bohemia), NatoSap(Bohemia)]`.
//!
//! The canonical renderer reads the axis verbatim and emits the
//! duplicated token (`ATOMAL/ATOMAL`, `BOHEMIA/BOHEMIA`). The E066
//! `Recanonicalize` fix-text contract requires byte-identical canonical
//! output; duplicate tokens break that contract.
//!
//! # The fix
//!
//! [`dedup_companions`] walks [`ParsedAttrs::aea_markings`] and
//! [`ParsedAttrs::sci_markings`] once and retains only the first
//! occurrence of each value in source order. The companion-source spans
//! follow the surviving entry verbatim, so audit-record provenance
//! reads the source span the user actually typed — the canonical-form
//! span typically wins because the legacy-compound canonicalization
//! runs first in the block loop and the canonical-form recognition
//! pass appends the second entry.
//!
//! # Why a post-pass, not a per-push gate
//!
//! 1. The two push sites sit on opposite sides of the block loop and
//!    have unrelated surrounding control flow; a per-site dedup check
//!    would duplicate the predicate at every push site and remain
//!    fragile to future push-site additions.
//! 2. Centralizing dedup mirrors the [`attribute_dissems`] pattern
//!    introduced in PR 9b — both passes treat the parser's per-token
//!    output as "intent" and let a post-pass resolve cross-token
//!    invariants.
//! 3. Future axes that grow companion-write paths (e.g., a SAR
//!    companion path) inherit the same dedup property without
//!    auditing every push site.
//!
//! # Authority
//!
//! - CAPCO-2016 §G.1 Table 4 p38 (the eight portion-form + five
//!   banner-form legacy compounds whose canonicalization triggers the
//!   duplicate-push pattern).
//! - CAPCO-2016 §G.2 p40 (registers ATOMAL / BOHEMIA / BALK as
//!   standalone control markings — the canonical destination axis the
//!   dedup pass guards).
//! - CAPCO-2016 §H.7 p122 (ATOMAL worked example —
//!   `SECRET//RD/ATOMAL//FGI NATO//NOFORN` — places ATOMAL in the
//!   AEA block with no duplicate).
//! - CAPCO-2016 §H.7 p127 (BOHEMIA worked example —
//!   `TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN` — places
//!   BOHEMIA in the SCI block with no duplicate).
//!
//! [`attribute_dissems`]: crate::attribute_dissems
//! [`AeaMarking::Atomal`]: crate::attrs::AeaMarking::Atomal
//! [`SciControlSystem::NatoSap`]: crate::attrs::SciControlSystem::NatoSap
//! [`SciMarking`]: crate::attrs::SciMarking
//! [`ParsedAttrs::aea_markings`]: crate::parsed::ParsedAttrs::aea_markings
//! [`ParsedAttrs::sci_markings`]: crate::parsed::ParsedAttrs::sci_markings

use crate::parsed::{ParsedAea, ParsedAttrs, ParsedSciMarking};

/// Remove duplicate AEA and SCI axis entries from `attrs`, preserving
/// source order.
///
/// "Duplicate" means structural equality on the axis value: two
/// [`ParsedAea`] entries are duplicates when their `.value` fields are
/// `==`, regardless of source span; likewise for [`ParsedSciMarking`].
///
/// **Idempotent.** Calling twice produces the same output as calling
/// once. The first occurrence's bytes / span are preserved verbatim.
///
/// **Stable.** Source order is preserved among the retained entries —
/// the function never reorders surviving items.
///
/// **Order of consumption.** In the current `crates/core/src/parser.rs`
/// dispatch, the legacy-compound canonicalization at the classification
/// block (`idx == 1`, line ~372 / ~386) emits the companion FIRST, and
/// the canonical-form AEA / SCI block parse runs LATER in the block
/// loop. The surviving entry is therefore the legacy-compound
/// canonicalization companion; the canonical-form entry is dropped.
/// This is the right direction — the legacy entry's source span
/// covers the compound text the user actually typed, which is what
/// E066's `Recanonicalize` re-marking acts on.
///
/// **Hot-path cost.** Both axes typically hold ≤ 4 entries in practice
/// (RD / FRD / TFNI / ATOMAL is the upper bound for AEA; SCI is bounded
/// by the registered control-system count). The O(n²) `retain` walk
/// runs in single-digit microseconds and stays well under the SC-001
/// p95 ≤ 16 ms budget.
pub fn dedup_companions<'src>(attrs: &mut ParsedAttrs<'src>) {
    dedup_aea(&mut attrs.aea_markings);
    dedup_sci_markings(&mut attrs.sci_markings);
}

fn dedup_aea<'src>(axis: &mut Box<[ParsedAea<'src>]>) {
    if axis.len() <= 1 {
        return;
    }
    let mut out: Vec<ParsedAea<'src>> = Vec::with_capacity(axis.len());
    for entry in std::mem::replace(axis, Box::new([])).into_vec() {
        if !out.iter().any(|existing| existing.value == entry.value) {
            out.push(entry);
        }
    }
    *axis = out.into_boxed_slice();
}

fn dedup_sci_markings<'src>(axis: &mut Box<[ParsedSciMarking<'src>]>) {
    if axis.len() <= 1 {
        return;
    }
    let mut out: Vec<ParsedSciMarking<'src>> = Vec::with_capacity(axis.len());
    for entry in std::mem::replace(axis, Box::new([])).into_vec() {
        if !out.iter().any(|existing| existing.value == entry.value) {
            out.push(entry);
        }
    }
    *axis = out.into_boxed_slice();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{AeaMarking, AtomalBlock, NatoSap, SciControlSystem, SciMarking};
    use crate::parsed::SourceOrigin;
    use crate::span::Span;

    fn empty_attrs<'src>(
        aea: Vec<ParsedAea<'src>>,
        sci: Vec<ParsedSciMarking<'src>>,
    ) -> ParsedAttrs<'src> {
        ParsedAttrs::new(
            None,
            sci.into_boxed_slice(),
            Box::new([]),
            None,
            aea.into_boxed_slice(),
            None,
            Box::new([]),
            Box::new([]),
            Box::new([]),
            Box::new([]),
            None,
            None,
            None,
            None,
            Box::new([]),
            SourceOrigin::Portion,
        )
    }

    fn atomal_at<'a>(bytes: &'a str, span: Span) -> ParsedAea<'a> {
        ParsedAea::new(AeaMarking::Atomal(AtomalBlock), bytes, span)
    }

    fn nato_sap_marking_at<'a>(sap: NatoSap, bytes: &'a str, span: Span) -> ParsedSciMarking<'a> {
        let sci = SciMarking::new(SciControlSystem::NatoSap(sap), Box::new([]), None);
        ParsedSciMarking::new(sci, bytes, span)
    }

    #[test]
    fn empty_axes_are_no_op() {
        let mut attrs = empty_attrs(vec![], vec![]);
        dedup_companions(&mut attrs);
        assert!(attrs.aea_markings.is_empty());
        assert!(attrs.sci_markings.is_empty());
    }

    #[test]
    fn single_entry_axes_are_preserved() {
        let aea = vec![atomal_at("ATOMAL", Span::new(5, 11))];
        let sci = vec![nato_sap_marking_at(
            NatoSap::Bohemia,
            "BOHEMIA",
            Span::new(0, 7),
        )];
        let mut attrs = empty_attrs(aea, sci);
        dedup_companions(&mut attrs);
        assert_eq!(attrs.aea_markings.len(), 1);
        assert_eq!(attrs.sci_markings.len(), 1);
    }

    #[test]
    fn duplicate_atomal_collapses_to_one_first_wins() {
        let first_span = Span::new(1, 5); // legacy companion site
        let second_span = Span::new(7, 13); // canonical-form site
        let aea = vec![
            atomal_at("NSAT", first_span),
            atomal_at("ATOMAL", second_span),
        ];
        let mut attrs = empty_attrs(aea, vec![]);
        dedup_companions(&mut attrs);
        assert_eq!(attrs.aea_markings.len(), 1);
        // First-wins: span and bytes match the legacy-companion entry.
        assert_eq!(attrs.aea_markings[0].span, first_span);
        assert_eq!(attrs.aea_markings[0].bytes, "NSAT");
    }

    #[test]
    fn duplicate_bohemia_collapses_to_one_first_wins() {
        let first_span = Span::new(1, 6);
        let second_span = Span::new(8, 15);
        let sci = vec![
            nato_sap_marking_at(NatoSap::Bohemia, "CTS-B", first_span),
            nato_sap_marking_at(NatoSap::Bohemia, "BOHEMIA", second_span),
        ];
        let mut attrs = empty_attrs(vec![], sci);
        dedup_companions(&mut attrs);
        assert_eq!(attrs.sci_markings.len(), 1);
        assert_eq!(attrs.sci_markings[0].span, first_span);
        assert_eq!(attrs.sci_markings[0].bytes, "CTS-B");
    }

    #[test]
    fn distinct_aea_values_are_not_collapsed() {
        // RD and ATOMAL are distinct AEA variants; both must survive.
        let aea = vec![
            ParsedAea::new(
                AeaMarking::Rd(crate::attrs::RdBlock::default()),
                "RD",
                Span::new(0, 2),
            ),
            atomal_at("ATOMAL", Span::new(4, 10)),
        ];
        let mut attrs = empty_attrs(aea, vec![]);
        dedup_companions(&mut attrs);
        assert_eq!(attrs.aea_markings.len(), 2);
    }

    #[test]
    fn distinct_sci_sap_values_are_not_collapsed() {
        // BOHEMIA and BALK are distinct NATO SAPs; both must survive.
        let sci = vec![
            nato_sap_marking_at(NatoSap::Bohemia, "BOHEMIA", Span::new(0, 7)),
            nato_sap_marking_at(NatoSap::Balk, "BALK", Span::new(8, 12)),
        ];
        let mut attrs = empty_attrs(vec![], sci);
        dedup_companions(&mut attrs);
        assert_eq!(attrs.sci_markings.len(), 2);
    }

    #[test]
    fn idempotent_under_repeated_invocation() {
        let aea = vec![
            atomal_at("NSAT", Span::new(1, 5)),
            atomal_at("ATOMAL", Span::new(7, 13)),
        ];
        let mut attrs = empty_attrs(aea, vec![]);
        dedup_companions(&mut attrs);
        let after_first = attrs.aea_markings.len();
        dedup_companions(&mut attrs);
        assert_eq!(attrs.aea_markings.len(), after_first);
    }
}
