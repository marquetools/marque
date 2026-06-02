// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`DeclassifyOnLattice`] — the §E.3 declassification-precedence
//! semilattice over [`DeclassInstruction`].

use marque_ism::{CanonicalAttrs, DeclassExemption, IsmDate};
use marque_scheme::{BoundedJoinSemilattice, JoinSemilattice, MeetSemilattice, OrdMax};

pub use super::declass_instruction::DeclassInstruction;

// ---------------------------------------------------------------------------
// DeclassifyOnLattice — §E.3 precedence semilattice (join-bounded by absence)
// ---------------------------------------------------------------------------

/// Lattice form of the declassification axis.
///
/// Carries the single §E.3 [`DeclassInstruction`] with an
/// [`OrdMax`] join — "the most restrictive / longest-protection
/// instruction wins". Per CAPCO-2016 §E.3 p32 (verbatim):
/// *"The 'Declassify On' line must reflect the single declassification
/// value that provides the longest classification duration of any of
/// the sources. When determining the single most restrictive
/// declassification instruction among multiple source documents,
/// adhere to the following hierarchy …"*. The nine-tier hierarchy that
/// follows (§E.3 p32–33) is encoded by the
/// [`DeclassInstruction`] [`Ord`]; this newtype lifts it into the
/// engine's semilattice machinery.
///
/// # Bounds
///
/// `None` is the lattice **bottom** (absence — a portion with no
/// declassification instruction), and is the join identity, so the
/// type implements [`BoundedJoinSemilattice`] (`bottom() = None`). The
/// chain has a genuine maximum,
/// [`DeclassInstruction::NaSeeSourceList`] (§E.3 p32: "takes precedence
/// over all other declassification instructions"), but that top is a
/// property of the [`DeclassInstruction`] [`Ord`] (tier rank), NOT a
/// [`BoundedMeetSemilattice`] method — the meet side stays unbounded
/// (`NaSeeSourceList` is the chain's top-of-chain element, not a `meet`
/// identity, and there is no lawful finite meet-top for the open date
/// /exemption space). See
/// `crates/capco/tests/lattice_static_assertions.rs`.
///
/// # Authority
///
/// §E.3 p32 is the proper authority for declass aggregation, NOT §H.6
/// p104. §H.6 p104 governs RD/FRD/TFNI banner roll-up; its rule for
/// declass dates is the opposite ("Automatic declassification of
/// documents containing RD information is prohibited"), forbidding a
/// declass date on RD documents entirely. ISOO §3.3 is the out-of-tree
/// primary source §E.3 derives from; cited for cross-reference only,
/// not as primary authority per Constitution VIII.
///
/// §-authority (CAPCO-2016.md):
/// - §E.3 p32 + §E.3 p33 (Multiple Sources
///   and the Declassify On Line Hierarchy — the nine-tier precedence).
/// - §E.4 p33 / §E.5 p33 (commingling: the N/A
///   string replaces any date/event → tier-1 `NaSeeSourceList`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclassifyOnLattice(Option<OrdMax<DeclassInstruction>>);

impl DeclassifyOnLattice {
    /// An empty declassify-on — the lattice bottom (absence).
    pub fn empty() -> Self {
        Self(None)
    }

    /// Construct from an `Option<DeclassInstruction>`.
    pub fn new(instruction: Option<DeclassInstruction>) -> Self {
        Self(instruction.map(OrdMax))
    }

    /// Construct from a bare `Option<IsmDate>`, lifting a present date
    /// into the tier-7 [`DeclassInstruction::SpecificDate`] (§E.3 p33 —
    /// a specific declassification date with no exemption
    /// code). A seeded bare date is an authored value, not the tier-9
    /// calculated fallback (which is engine-minted in PR-D3 — see
    /// [`DeclassInstruction::Calculated25Year`]).
    pub fn from_date(d: Option<IsmDate>) -> Self {
        Self::new(d.map(|date| DeclassInstruction::SpecificDate { date }))
    }

    /// Construct from a slice of [`CanonicalAttrs`]: fold each portion's
    /// `(declassify_on, declass_exemption)` into a
    /// [`DeclassInstruction`] per §E.3, then [`OrdMax`]-join across
    /// portions so the most-restrictive instruction wins.
    ///
    /// [`DeclassInstruction::EventUnder10Year`] (tier 8) and
    /// [`DeclassInstruction::Calculated25Year`] (tier 9) are never
    /// minted here — the canonical pivot carries no event string or
    /// calculated-fallback marker. The engine node (PR-D3) mints them
    /// from derived edges.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        let folded = portions
            .iter()
            .fold(
                None::<OrdMax<DeclassInstruction>>,
                |acc, p| match instruction_from_attrs(p.declassify_on.clone(), p.declass_exemption)
                {
                    None => acc,
                    Some(instr) => match acc {
                        None => Some(OrdMax(instr)),
                        Some(a) => Some(a.join(&OrdMax(instr))),
                    },
                },
            );
        Self(folded)
    }

    /// Consume into the inner `Option<DeclassInstruction>`.
    pub fn into_inner(self) -> Option<DeclassInstruction> {
        self.0.map(|OrdMax(i)| i)
    }

    /// Project the resolved declassification date out of the carried
    /// instruction, for the (still date-only) `CanonicalAttrs`
    /// /`ProjectedMarking` pivot field. A dateless instruction
    /// (`NaSeeSourceList`, `Exempt50xBeyond`, `Eo12951`, etc.) projects to
    /// `None`.
    pub fn into_date(self) -> Option<IsmDate> {
        self.0.and_then(|OrdMax(i)| i.resolved_date())
    }

    /// Borrow the inner [`DeclassInstruction`].
    pub fn as_inner(&self) -> Option<&DeclassInstruction> {
        self.0.as_ref().map(|OrdMax(i)| i)
    }

    /// Borrow the carried instruction's resolved date, if any.
    pub fn as_date(&self) -> Option<&IsmDate> {
        self.0.as_ref().and_then(|OrdMax(i)| i.resolved_date_ref())
    }
}

/// Classify a portion's `(declassify_on, declass_exemption)` pair into
/// one §E.3 [`DeclassInstruction`], or `None` when the portion carries
/// neither (absence → bottom).
///
/// §E.3 mapping (CAPCO-2016 §E.3 p32–33):
/// - `Aea` / `Nato` / `NatoAea` → tier-1 `NaSeeSourceList` (§E.3 p32 /
///   §E.4 p33 / §E.5 p33 — the commingling N/A markers;
///   §E.4/§E.5: the line "must not contain a declassification date or
///   event", so any coexisting date is discarded).
/// - `X50x1Hum` / `X50x2Wmd` / `X75x` → tier-2 `Exempt50xBeyond`
///   (§E.3 p32, dateless by source).
/// - other `X50x#` + date → tier-3 `Exempt50xDated` (§E.3 p32).
/// - other `X50x#`, no date → `Exempt50xUndated` (§E.3-structure
///   inference; see the variant doc-comment).
/// - `X25x1Eo12951` → tier-4 `Eo12951` (§E.3 p33, dateless).
/// - `X25x#` + date → tier-5 `Exempt25xDated` (§E.3 p33).
/// - `X25x#`, no date → tier-6 `Exempt25xUndated` (§E.3 p33).
/// - no exemption + date → tier-7 `SpecificDate` (§E.3 p33).
///
/// [`DeclassExemption`] is `#[non_exhaustive]`; the match carries the
/// sanctioned catch-all (foreign non-exhaustive enum) so a future ODNI
/// exemption value still compiles, treated as a dated/undated 50X or
/// 25X only via its explicit arms and otherwise falling through to the
/// bare-date / absence arms.
fn instruction_from_attrs(
    date: Option<IsmDate>,
    code: Option<DeclassExemption>,
) -> Option<DeclassInstruction> {
    match code {
        // Tier 1 — commingling N/A (dateless; §E.4/§E.5 discard date).
        Some(DeclassExemption::Aea | DeclassExemption::Nato | DeclassExemption::NatoAea) => {
            Some(DeclassInstruction::NaSeeSourceList)
        }
        // Tier 2 — 50X beyond-50-year family (dateless by source).
        Some(
            c @ (DeclassExemption::X50x1Hum | DeclassExemption::X50x2Wmd | DeclassExemption::X75x),
        ) => Some(DeclassInstruction::Exempt50xBeyond { code: c }),
        // Tier 3 / undated-50X — other 50X exemptions.
        Some(
            c @ (DeclassExemption::X50x1
            | DeclassExemption::X50x2
            | DeclassExemption::X50x3
            | DeclassExemption::X50x4
            | DeclassExemption::X50x5
            | DeclassExemption::X50x6
            | DeclassExemption::X50x7
            | DeclassExemption::X50x8
            | DeclassExemption::X50x9),
        ) => Some(match date {
            Some(date) => DeclassInstruction::Exempt50xDated { code: c, date },
            None => DeclassInstruction::Exempt50xUndated { code: c },
        }),
        // Tier 4 — 25X1, EO 12951 (dateless singleton).
        Some(DeclassExemption::X25x1Eo12951) => Some(DeclassInstruction::Eo12951),
        // Tier 5 / tier 6 — other 25X exemptions, dated vs undated.
        Some(
            c @ (DeclassExemption::X25x1
            | DeclassExemption::X25x2
            | DeclassExemption::X25x3
            | DeclassExemption::X25x4
            | DeclassExemption::X25x5
            | DeclassExemption::X25x6
            | DeclassExemption::X25x7
            | DeclassExemption::X25x8
            | DeclassExemption::X25x9),
        ) => Some(match date {
            Some(date) => DeclassInstruction::Exempt25xDated { code: c, date },
            None => DeclassInstruction::Exempt25xUndated {
                code: c,
                date: None,
            },
        }),
        // `#[non_exhaustive]` catch-all: a future numbered exemption we
        // do not yet special-case falls through to the bare-date arm.
        // Tier 7 — bare authored date, no exemption (§E.3 p33); or
        // absence → bottom (`None`).
        Some(_) | None => date.map(|date| DeclassInstruction::SpecificDate { date }),
    }
}

impl JoinSemilattice for DeclassifyOnLattice {
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, x) | (x, None) => Self(x.clone()),
            // OrdMax::join keys on `>=` — the most-restrictive
            // (highest §E.3 precedence) instruction wins.
            (Some(a), Some(b)) => Self(Some(a.join(b))),
        }
    }
}

impl MeetSemilattice for DeclassifyOnLattice {
    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            // OrdMax::meet keys on `<=` — the least-restrictive
            // instruction wins.
            (Some(a), Some(b)) => Self(Some(a.meet(b))),
        }
    }
}

impl BoundedJoinSemilattice for DeclassifyOnLattice {
    /// The join identity: absence of any instruction.
    fn bottom() -> Self {
        Self(None)
    }
}

impl DeclassInstruction {
    /// The resolved declassification date this instruction carries, by
    /// value — for projection back onto the date-only pivot field.
    fn resolved_date(self) -> Option<IsmDate> {
        match self {
            DeclassInstruction::SpecificDate { date }
            | DeclassInstruction::Exempt50xDated { date, .. }
            | DeclassInstruction::Exempt25xDated { date, .. } => Some(date),
            DeclassInstruction::Calculated25Year { date }
            | DeclassInstruction::Exempt25xUndated { date, .. } => date,
            DeclassInstruction::NaSeeSourceList
            | DeclassInstruction::Exempt50xBeyond { .. }
            | DeclassInstruction::Exempt50xUndated { .. }
            | DeclassInstruction::Eo12951
            | DeclassInstruction::EventUnder10Year => None,
        }
    }

    /// The resolved declassification date, by reference.
    fn resolved_date_ref(&self) -> Option<&IsmDate> {
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
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::{CanonicalAttrs, DeclassExemption, IsmDate};

    fn portion(date: Option<IsmDate>, code: Option<DeclassExemption>) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.declassify_on = date;
        a.declass_exemption = code;
        a
    }

    #[test]
    fn empty_equals_default_and_bottom() {
        assert_eq!(DeclassifyOnLattice::empty(), DeclassifyOnLattice::default());
        assert_eq!(DeclassifyOnLattice::empty(), DeclassifyOnLattice::bottom());
        assert_eq!(DeclassifyOnLattice::empty().as_inner(), None);
    }

    #[test]
    fn from_attrs_iter_empty_is_bottom() {
        assert_eq!(
            DeclassifyOnLattice::from_attrs_iter(&[]),
            DeclassifyOnLattice::bottom()
        );
    }

    #[test]
    fn from_attrs_iter_50x1_hum_is_tier_2() {
        // §E.3 p32.
        let l = DeclassifyOnLattice::from_attrs_iter(&[portion(
            None,
            Some(DeclassExemption::X50x1Hum),
        )]);
        assert_eq!(
            l.as_inner(),
            Some(&DeclassInstruction::Exempt50xBeyond {
                code: DeclassExemption::X50x1Hum
            })
        );
    }

    #[test]
    fn from_attrs_iter_bare_date_is_specific_date_tier_7() {
        // §E.3 p33.
        let l =
            DeclassifyOnLattice::from_attrs_iter(&[portion(Some(IsmDate::Date(2030, 1, 1)), None)]);
        assert_eq!(
            l.as_inner(),
            Some(&DeclassInstruction::SpecificDate {
                date: IsmDate::Date(2030, 1, 1)
            })
        );
    }

    #[test]
    fn from_attrs_iter_hum_dominates_bare_date() {
        // §E.3 p32 (tier 2) dominates a bare date (tier 7).
        let l = DeclassifyOnLattice::from_attrs_iter(&[
            portion(None, Some(DeclassExemption::X50x1Hum)),
            portion(Some(IsmDate::Date(2099, 12, 31)), None),
        ]);
        assert_eq!(
            l.as_inner(),
            Some(&DeclassInstruction::Exempt50xBeyond {
                code: DeclassExemption::X50x1Hum
            })
        );
    }

    #[test]
    fn from_attrs_iter_commingling_marker_is_tier_1() {
        // §E.3 p32 / §E.4 p33: AEA commingling N/A; any
        // coexisting date is discarded (§E.4: no date/event allowed).
        let l = DeclassifyOnLattice::from_attrs_iter(&[portion(
            Some(IsmDate::Date(2030, 1, 1)),
            Some(DeclassExemption::Aea),
        )]);
        assert_eq!(l.as_inner(), Some(&DeclassInstruction::NaSeeSourceList));
        assert_eq!(l.into_date(), None, "commingling is dateless");
    }

    #[test]
    fn from_attrs_iter_undated_50x_is_exempt50x_undated() {
        // §E.3-structure inference for an undated non-HUM/WMD 50X.
        let l =
            DeclassifyOnLattice::from_attrs_iter(&[portion(None, Some(DeclassExemption::X50x3))]);
        assert_eq!(
            l.as_inner(),
            Some(&DeclassInstruction::Exempt50xUndated {
                code: DeclassExemption::X50x3
            })
        );
    }

    #[test]
    fn into_date_projects_dated_instruction() {
        let l = DeclassifyOnLattice::new(Some(DeclassInstruction::Exempt50xDated {
            code: DeclassExemption::X50x3,
            date: IsmDate::Date(2040, 1, 1),
        }));
        assert_eq!(l.into_date(), Some(IsmDate::Date(2040, 1, 1)));
    }

    #[test]
    fn into_date_dateless_instruction_projects_none() {
        let beyond = DeclassifyOnLattice::new(Some(DeclassInstruction::Exempt50xBeyond {
            code: DeclassExemption::X50x1Hum,
        }));
        assert_eq!(beyond.into_date(), None);
    }

    #[test]
    fn from_date_lifts_bare_date_to_specific_date() {
        let l = DeclassifyOnLattice::from_date(Some(IsmDate::Year(2030)));
        assert_eq!(
            l.as_inner(),
            Some(&DeclassInstruction::SpecificDate {
                date: IsmDate::Year(2030)
            })
        );
        assert_eq!(
            DeclassifyOnLattice::from_date(None),
            DeclassifyOnLattice::bottom()
        );
    }
}
