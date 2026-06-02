// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`DeclassInstruction`] — the §E.3 declassification-precedence chain.
//!
//! CAPCO-2016 §E.3 p32 "Multiple Sources and the Declassify On Line
//! Hierarchy" defines a nine-tier ordering for selecting the single
//! declassification value that "provides the longest classification
//! duration of any of the sources" (verbatim, §E.3 p32). This
//! type carries one such instruction; the [`Ord`] impl is the §E.3
//! precedence relation, and `OrdMax`-join over it picks the
//! most-restrictive / longest-protection instruction.

use core::cmp::Ordering;
use marque_ism::{DeclassExemption, IsmDate};

/// One §E.3 declassification instruction — the single value the
/// "Declassify On" line may carry (CAPCO-2016 §E.2 p32:
/// "Only a single value must be used on the 'Declassify On' line").
///
/// Variants are ordered by §E.3 precedence (tier 1 = highest =
/// [`NaSeeSourceList`](DeclassInstruction::NaSeeSourceList)). The hand-written
/// total [`Ord`] (see [`DeclassInstruction::cmp`]) is the load-bearing
/// law; the declaration order here is documentation, NOT the
/// comparison key.
///
/// # `Eq` is precedence-equivalence, not structural identity
///
/// `Eq`/`PartialEq` are derived from `Ord` (`a == b ⟺ a.cmp(b) ==
/// Equal`), NOT structural. Two instructions that resolve to the same
/// precedence key compare equal even if their data differs (e.g.
/// `Year(2003)` and `Date(2003, 12, 31)` at the same tier share an
/// end-of-span instant and so are precedence-equal). This is required
/// so [`marque_scheme::OrdMax`]'s join (which keys on `>=`) is
/// consistent with equality and the `JoinSemilattice: Eq` contract
/// holds. None of `PartialEq` / `Eq` / `PartialOrd` / `Ord` is
/// `#[derive]`d — see the four hand-written impls below.
///
/// # Tier → variant map (§E.3 p32–33)
///
/// | §E.3 tier | Precedence (1 = top) | Variant |
/// |-----------|----------------------|---------|
/// | 1 N/A commingling (p32) | 1 | [`NaSeeSourceList`](Self::NaSeeSourceList) |
/// | 2 50X-HUM/WMD/>50yr (p32) | 2 | [`Exempt50xBeyond`](Self::Exempt50xBeyond) |
/// | 3 50X#, dated (p32) | 3 | [`Exempt50xDated`](Self::Exempt50xDated) |
/// | (50X#, undated — §E.3-structure inference) | between 3 and 4 | [`Exempt50xUndated`](Self::Exempt50xUndated) |
/// | 4 25X1 EO 12951 (p33) | 4 | [`Eo12951`](Self::Eo12951) |
/// | 5 25X#, dated (p33) | 5 | [`Exempt25xDated`](Self::Exempt25xDated) |
/// | 6 25X#, undated (p33) | 6 | [`Exempt25xUndated`](Self::Exempt25xUndated) |
/// | 7 specific date ≤25yr (p33) | 7 | [`SpecificDate`](Self::SpecificDate) |
/// | 8 event <10yr (p33) | 8 | [`EventUnder10Year`](Self::EventUnder10Year) |
/// | 9 calc 25yr fallback (p33) | 9 | [`Calculated25Year`](Self::Calculated25Year) |
///
/// Bottom (absence of any instruction) is NOT a variant of this enum;
/// it is modeled at the [`DeclassifyOnLattice`](super::DeclassifyOnLattice)
/// newtype layer as `None`. Every value of `DeclassInstruction` is a
/// real §E.3 instruction.
#[derive(Debug, Clone)]
pub enum DeclassInstruction {
    /// Tier 9 — calculated 25-year fallback (§E.3 p33).
    /// Dateless tier marker; the resolved date rides in `date` when
    /// known. `from_attrs_iter` never mints this — the canonical pivot
    /// carries no "calculated-fallback" marker; the engine node
    /// (PR-D3) mints it from a derived edge.
    Calculated25Year { date: Option<IsmDate> },

    /// Tier 8 — event less than 10 years in the future (§E.3 p33 line
    /// 676). marque does not capture the event string on the pivot, so
    /// this is a dateless tier marker that `from_attrs_iter` never
    /// mints; it exists for total-order completeness + PR-D3 edges.
    EventUnder10Year,

    /// Tier 7 — a specific declassification date ≤25yr (§E.3 p33 line
    /// 675).
    SpecificDate { date: IsmDate },

    /// Tier 6 — 25X1–25X9 without a date or event (§E.3 p33).
    /// `code` is the 25X# exemption (lowest-number tiebreak); `date` is
    /// the computed 50-yr-from-source date when available (`None` from
    /// `from_attrs_iter` in PR-D1 — the calculation is engine work).
    Exempt25xUndated {
        code: DeclassExemption,
        date: Option<IsmDate>,
    },

    /// Tier 5 — 25X1–25X9 with a date or event (§E.3 p33).
    Exempt25xDated {
        code: DeclassExemption,
        date: IsmDate,
    },

    /// Tier 4 — "25X1, EO 12951", D/NGA imagery only (§E.3 p33 line
    /// 672). Dateless singleton ([`DeclassExemption::X25x1Eo12951`]).
    Eo12951,

    /// Between tier 3 and tier 4 — a 50X1–50X9 exemption (other than
    /// HUM/WMD/75X) carrying NO date or event.
    ///
    /// §E.3 tier 3 (p32) is "50X#, *with* a date or event"; the
    /// source does not name an undated-50X tier. This variant is a
    /// **§E.3-structure-derived inference**, flagged for corpus
    /// validation in PR-D5: §E.3 places the entire 50X family
    /// (tiers 2–3) above the entire 25X family (tiers 4–6), so an
    /// undated 50X must outrank every 25X/EO-12951 tier. It carries
    /// less protection information than a dated 50X (tier 3), so it
    /// sorts below `Exempt50xDated` and above `Eo12951`.
    Exempt50xUndated { code: DeclassExemption },

    /// Tier 3 — 50X1–50X9 with a date or event (§E.3 p32).
    Exempt50xDated {
        code: DeclassExemption,
        date: IsmDate,
    },

    /// Tier 2 — 50X1-HUM / 50X2-WMD / ISOO >50yr designator (75X)
    /// (§E.3 p32). Dateless. `code` retained for the
    /// lowest-number tiebreak + render; the join keys on (tier,
    /// code-number).
    Exempt50xBeyond { code: DeclassExemption },

    /// Tier 1 (top) — §E.4/§E.5 canned "Declassify On:" line: when sources
    /// include AEA (RD/FRD/TFNI) and/or NATO portions, those portions are
    /// N/A (no auto-declassification) and the NSI portion declass lives in
    /// the source list. Dateless by construction; "takes precedence over
    /// all other declassification instructions" (§E.3 p32). The exact N/A
    /// wording (AEA-only / NATO-only / combined) is a RENDER concern
    /// (Phase G / T070), not a lattice distinction.
    NaSeeSourceList,
}

impl DeclassInstruction {
    /// §E.3 tier rank — LARGEST = highest precedence = most restrictive.
    ///
    /// This is the INVERSE of the §E.3 bullet numbering: §E.3 p32–33
    /// lists the most-restrictive instruction first (bullet 1), but
    /// `OrdMax`-join needs most-restrictive to compare as the largest,
    /// so [`NaSeeSourceList`](Self::NaSeeSourceList) (§E.3 bullet 1) gets the
    /// largest rank here.
    fn tier_rank(&self) -> u8 {
        match self {
            DeclassInstruction::NaSeeSourceList => 10,
            DeclassInstruction::Exempt50xBeyond { .. } => 9,
            DeclassInstruction::Exempt50xDated { .. } => 8,
            // §E.3-structure inference: undated 50X between dated-50X
            // (tier 3) and EO-12951 (tier 4). See variant doc-comment.
            DeclassInstruction::Exempt50xUndated { .. } => 7,
            DeclassInstruction::Eo12951 => 6,
            DeclassInstruction::Exempt25xDated { .. } => 5,
            DeclassInstruction::Exempt25xUndated { .. } => 4,
            DeclassInstruction::SpecificDate { .. } => 3,
            DeclassInstruction::EventUnder10Year => 2,
            DeclassInstruction::Calculated25Year { .. } => 1,
        }
    }

    /// The date this instruction carries for the within-tier
    /// longest-protection comparison, if any.
    fn date(&self) -> Option<&IsmDate> {
        match self {
            DeclassInstruction::SpecificDate { date }
            | DeclassInstruction::Exempt50xDated { date, .. }
            | DeclassInstruction::Exempt25xDated { date, .. } => Some(date),
            DeclassInstruction::Calculated25Year { date }
            | DeclassInstruction::Exempt25xUndated { date, .. } => date.as_ref(),
            DeclassInstruction::NaSeeSourceList
            | DeclassInstruction::Exempt50xBeyond { .. }
            | DeclassInstruction::Exempt50xUndated { .. }
            | DeclassInstruction::Eo12951
            | DeclassInstruction::EventUnder10Year => None,
        }
    }

    /// The exemption code this instruction carries for the
    /// lowest-number tiebreak, if any.
    fn code(&self) -> Option<DeclassExemption> {
        match self {
            DeclassInstruction::Exempt50xBeyond { code }
            | DeclassInstruction::Exempt50xUndated { code }
            | DeclassInstruction::Exempt50xDated { code, .. }
            | DeclassInstruction::Exempt25xDated { code, .. }
            | DeclassInstruction::Exempt25xUndated { code, .. } => Some(*code),
            DeclassInstruction::NaSeeSourceList
            | DeclassInstruction::Eo12951
            | DeclassInstruction::SpecificDate { .. }
            | DeclassInstruction::EventUnder10Year
            | DeclassInstruction::Calculated25Year { .. } => None,
        }
    }
}

/// The competing integer for the §E.3 lowest-number tiebreak.
///
/// Lower number = higher precedence on a same-tier same-date tie
/// (§E.3 lines 664/665/673/674: "apply the exemption with the lowest
/// number"). The [`Ord`] impl stores `u16::MAX - exemption_rank(code)`
/// so `max` (the `OrdMax` join) picks the LOWEST number.
///
/// [`DeclassExemption`] is `#[non_exhaustive]`, so the match MUST carry
/// a catch-all arm. This is the one sanctioned wildcard (a foreign
/// non-exhaustive enum, not a local business enum): non-numbered codes
/// (`Aea` / `Nato` / `NatoAea`) and any future-added variant map to a
/// neutral high number so they never spuriously win a "lowest" tie.
fn exemption_rank(code: DeclassExemption) -> u16 {
    match code {
        DeclassExemption::X50x1Hum
        | DeclassExemption::X50x1
        | DeclassExemption::X25x1
        | DeclassExemption::X25x1Eo12951 => 1,
        DeclassExemption::X50x2Wmd | DeclassExemption::X50x2 | DeclassExemption::X25x2 => 2,
        DeclassExemption::X50x3 | DeclassExemption::X25x3 => 3,
        DeclassExemption::X50x4 | DeclassExemption::X25x4 => 4,
        DeclassExemption::X50x5 | DeclassExemption::X25x5 => 5,
        DeclassExemption::X50x6 | DeclassExemption::X25x6 => 6,
        DeclassExemption::X50x7 | DeclassExemption::X25x7 => 7,
        DeclassExemption::X50x8 | DeclassExemption::X25x8 => 8,
        DeclassExemption::X50x9 | DeclassExemption::X25x9 => 9,
        DeclassExemption::X75x => 75,
        // `#[non_exhaustive]` catch-all + the non-numbered commingling
        // markers (Aea / Nato / NatoAea) which never reach a numbered
        // tiebreak (they map to the dateless `NaSeeSourceList` tier).
        _ => u16::MAX,
    }
}

impl PartialEq for DeclassInstruction {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for DeclassInstruction {}

impl PartialOrd for DeclassInstruction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeclassInstruction {
    /// Total §E.3 precedence order: higher = more restrictive / longer
    /// protection.
    ///
    /// Compared lexicographically: (1) tier rank, (2) within a tier the
    /// end-of-span date (later = longer protection; dateless sorts
    /// earliest), (3) the negated exemption number so a LOWER number
    /// wins the §E.3 lowest-number tiebreak.
    ///
    /// The date comparison uses [`IsmDate::end_cmp`], which keys on the
    /// end-of-span instant. `Year(2003)` and `Date(2003, 12, 31)`
    /// collapse to `Equal` — that is correct and intended: for declass
    /// precedence two dates with the same end-of-span ARE equivalent
    /// (§E.3 keys on "longest period of protection" = end-of-span). On
    /// an `Equal` date the comparison falls through to the
    /// exemption-number tiebreak.
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. tier rank.
        let tier = self.tier_rank().cmp(&other.tier_rank());
        if tier != Ordering::Equal {
            return tier;
        }

        // 2. within-tier date: later end-of-span wins; a dateless
        //    instruction sorts below any dated one. Both dateless ⇒
        //    Equal, fall through to the tiebreak.
        let date = match (self.date(), other.date()) {
            (Some(a), Some(b)) => a.end_cmp(b),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        };
        if date != Ordering::Equal {
            return date;
        }

        // 3. exemption-number tiebreak: a LOWER number is more
        //    restrictive, so compare the negated rank (larger negated
        //    value = lower number = higher precedence). Code-less
        //    variants share `None` and so compare equal here.
        let neg =
            |c: Option<DeclassExemption>| c.map(|c| u16::MAX - exemption_rank(c)).unwrap_or(0);
        neg(self.code()).cmp(&neg(other.code()))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::{DeclassExemption, IsmDate};

    fn date(y: i32, m: u8, d: u8) -> IsmDate {
        IsmDate::Date(y, m, d)
    }

    // --- §E.3 tier precedence ------------------------------------------

    #[test]
    fn tier_order_is_strictly_descending_e3() {
        // §E.3 p32–33 lines 663–677: most-restrictive first. The chain
        // here is listed most-restrictive (NaSeeSourceList) to least
        // (Calculated25Year); each must compare strictly greater than
        // its successor.
        let chain = [
            DeclassInstruction::NaSeeSourceList,
            DeclassInstruction::Exempt50xBeyond {
                code: DeclassExemption::X50x1Hum,
            },
            DeclassInstruction::Exempt50xDated {
                code: DeclassExemption::X50x3,
                date: date(2050, 1, 1),
            },
            DeclassInstruction::Exempt50xUndated {
                code: DeclassExemption::X50x3,
            },
            DeclassInstruction::Eo12951,
            DeclassInstruction::Exempt25xDated {
                code: DeclassExemption::X25x3,
                date: date(2050, 1, 1),
            },
            DeclassInstruction::Exempt25xUndated {
                code: DeclassExemption::X25x3,
                date: None,
            },
            DeclassInstruction::SpecificDate {
                date: date(2040, 1, 1),
            },
            DeclassInstruction::EventUnder10Year,
            DeclassInstruction::Calculated25Year { date: None },
        ];
        for pair in chain.windows(2) {
            assert!(
                pair[0] > pair[1],
                "§E.3 precedence: {:?} must outrank {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn fifty_x_hum_beats_dated_25x_e3_line_664() {
        // §E.3 p32 (tier 2) vs p33 (tier 5).
        let hum = DeclassInstruction::Exempt50xBeyond {
            code: DeclassExemption::X50x1Hum,
        };
        let dated_25x = DeclassInstruction::Exempt25xDated {
            code: DeclassExemption::X25x1,
            date: date(2099, 12, 31),
        };
        assert!(hum > dated_25x);
    }

    #[test]
    fn hum_beats_wmd_lowest_number_e3_line_664() {
        // §E.3 p32: "apply 50X1-HUM as the exemption with the
        // lowest number." Both are tier 2 (Exempt50xBeyond), dateless;
        // the lowest-number tiebreak picks HUM (rank 1) over WMD (rank 2).
        let hum = DeclassInstruction::Exempt50xBeyond {
            code: DeclassExemption::X50x1Hum,
        };
        let wmd = DeclassInstruction::Exempt50xBeyond {
            code: DeclassExemption::X50x2Wmd,
        };
        assert!(hum > wmd);
    }

    #[test]
    fn same_tier_later_date_wins_e3_line_665() {
        // §E.3 p32: "the date or event that provides the
        // longest period of protection."
        let earlier = DeclassInstruction::Exempt50xDated {
            code: DeclassExemption::X50x3,
            date: date(2040, 1, 1),
        };
        let later = DeclassInstruction::Exempt50xDated {
            code: DeclassExemption::X50x3,
            date: date(2050, 1, 1),
        };
        assert!(later > earlier);
    }

    #[test]
    fn same_tier_same_date_lowest_number_wins_e3_line_673() {
        // §E.3 p33: "If all '25X#, date or event' exemptions
        // have the same date or event, apply the single '25X#,
        // date/event' exemption with the lowest number."
        let x1 = DeclassInstruction::Exempt25xDated {
            code: DeclassExemption::X25x1,
            date: date(2050, 1, 1),
        };
        let x9 = DeclassInstruction::Exempt25xDated {
            code: DeclassExemption::X25x9,
            date: date(2050, 1, 1),
        };
        assert!(x1 > x9);
    }

    #[test]
    fn eo12951_sits_between_50x_and_25x_e3_line_672() {
        // §E.3 p33 (tier 4): below the 50X family, above 25X.
        let eo = DeclassInstruction::Eo12951;
        let dated_50x = DeclassInstruction::Exempt50xDated {
            code: DeclassExemption::X50x9,
            date: date(2030, 1, 1),
        };
        let dated_25x = DeclassInstruction::Exempt25xDated {
            code: DeclassExemption::X25x1,
            date: date(2099, 1, 1),
        };
        assert!(dated_50x > eo, "tier 3 > tier 4");
        assert!(eo > dated_25x, "tier 4 > tier 5");
    }

    #[test]
    fn undated_50x_sits_between_dated_50x_and_eo12951() {
        // §E.3-structure inference (see Exempt50xUndated doc): below a
        // dated 50X (tier 3), above EO-12951 (tier 4).
        let undated = DeclassInstruction::Exempt50xUndated {
            code: DeclassExemption::X50x3,
        };
        let dated = DeclassInstruction::Exempt50xDated {
            code: DeclassExemption::X50x3,
            date: date(2050, 1, 1),
        };
        let eo = DeclassInstruction::Eo12951;
        assert!(dated > undated);
        assert!(undated > eo);
    }

    #[test]
    fn specific_date_beats_event_e3_lines_675_676() {
        // §E.3 p33 (tier 7) vs p33 (tier 8).
        let specific = DeclassInstruction::SpecificDate {
            date: date(2040, 1, 1),
        };
        assert!(specific > DeclassInstruction::EventUnder10Year);
    }

    // --- Eq is precedence-equivalence, not structural ------------------

    #[test]
    fn year_and_end_of_year_date_are_precedence_equal() {
        // ISOO §3.3 (end-of-span) contract: Year(2003) and
        // Date(2003-12-31) share an end-of-span instant, so at the same
        // tier they are precedence-equal even though they are
        // structurally distinct.
        let y = DeclassInstruction::SpecificDate {
            date: IsmDate::Year(2003),
        };
        let d = DeclassInstruction::SpecificDate {
            date: date(2003, 12, 31),
        };
        assert_eq!(y, d, "precedence-equivalence on shared end-of-span");
        assert_eq!(y.cmp(&d), Ordering::Equal);
    }

    #[test]
    fn yearmonth_and_end_of_month_date_are_precedence_equal() {
        // ISOO §3.3 (end-of-span) contract beyond the Year-vs-Date pair:
        // `end_components` collapses YearMonth/DateHourMin/DateTime to the
        // same end-of-span tuple too (see crates/ism/src/date.rs). A
        // `YearMonth(2003, 12)` resolves to the Dec-31 end-of-span, so it
        // is precedence-equal to `Date(2003, 12, 31)` and to the
        // `DateTime` that lands on that same final instant — pinning the
        // Eq/Ord contract for the non-Year/Date precisions as well.
        let ym = DeclassInstruction::SpecificDate {
            date: IsmDate::YearMonth(2003, 12),
        };
        let d = DeclassInstruction::SpecificDate {
            date: date(2003, 12, 31),
        };
        let dt = DeclassInstruction::SpecificDate {
            date: IsmDate::DateTime {
                year: 2003,
                month: 12,
                day: 31,
                hour: 23,
                minute: 59,
                second: 59,
                nanosecond: 999_999_999,
                offset: None,
            },
        };
        assert_eq!(ym, d, "YearMonth end-of-month == end-of-day Date");
        assert_eq!(ym.cmp(&d), Ordering::Equal);
        assert_eq!(ym, dt, "YearMonth == DateTime sharing the end instant");
        assert_eq!(ym.cmp(&dt), Ordering::Equal);
    }

    #[test]
    fn eq_agrees_with_cmp() {
        let a = DeclassInstruction::Exempt50xBeyond {
            code: DeclassExemption::X50x1Hum,
        };
        let b = DeclassInstruction::Exempt50xBeyond {
            code: DeclassExemption::X50x1Hum,
        };
        assert_eq!(a == b, a.cmp(&b) == Ordering::Equal);
    }

    // --- Ord totality (small exhaustive cross-product) -----------------

    #[test]
    fn ord_is_total_and_consistent() {
        let samples = [
            DeclassInstruction::NaSeeSourceList,
            DeclassInstruction::Exempt50xBeyond {
                code: DeclassExemption::X50x1Hum,
            },
            DeclassInstruction::Exempt50xBeyond {
                code: DeclassExemption::X50x2Wmd,
            },
            DeclassInstruction::Exempt50xDated {
                code: DeclassExemption::X50x3,
                date: date(2050, 1, 1),
            },
            DeclassInstruction::Exempt50xUndated {
                code: DeclassExemption::X50x3,
            },
            DeclassInstruction::Eo12951,
            DeclassInstruction::Exempt25xDated {
                code: DeclassExemption::X25x1,
                date: date(2050, 1, 1),
            },
            DeclassInstruction::Exempt25xUndated {
                code: DeclassExemption::X25x3,
                date: None,
            },
            DeclassInstruction::SpecificDate {
                date: date(2040, 1, 1),
            },
            DeclassInstruction::EventUnder10Year,
            DeclassInstruction::Calculated25Year { date: None },
        ];
        for a in &samples {
            for b in &samples {
                let ab = a.cmp(b);
                let ba = b.cmp(a);
                // antisymmetry of the comparator
                assert_eq!(ab, ba.reverse(), "cmp not antisymmetric: {a:?} vs {b:?}");
                // exactly-one-of trichotomy
                assert_eq!(
                    (ab == Ordering::Less) as u8
                        + (ab == Ordering::Equal) as u8
                        + (ab == Ordering::Greater) as u8,
                    1
                );
                // Eq agrees with cmp == Equal
                assert_eq!(a == b, ab == Ordering::Equal);
            }
        }
        // transitivity over the sorted chain
        for i in 0..samples.len() {
            for j in i..samples.len() {
                for k in j..samples.len() {
                    if samples[i] <= samples[j] && samples[j] <= samples[k] {
                        assert!(samples[i] <= samples[k]);
                    }
                }
            }
        }
    }
}
