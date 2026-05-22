// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`FgiSet`] — lattice over the FGI marker.

use marque_ism::{CanonicalAttrs, CountryCode, FgiMarker, MarkingClassification};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// FgiSet — lattice over the FGI marker
// ---------------------------------------------------------------------------

/// FGI marker in lattice form.
///
/// CAPCO's FGI marker has two independent axes: a set of source countries
/// and a source-concealed flag. Source-concealed supersedes source-
/// acknowledged on join — if any portion carries FGI with no countries
/// (concealed), the banner must also be concealed. Meet is dual: the
/// source-concealed form acts as the lattice top for the FGI
/// source-disclosure dimension, so meet with a concealed operand returns
/// the OTHER operand (the acknowledged side), and meet of two concealed
/// operands returns concealed. Meet of two acknowledged operands
/// intersects their country sets; an empty intersection collapses to
/// `None` (no shared FGI).
///
/// `FgiSet::None` is the bottom (no FGI anywhere).
///
/// # Source authority
///
/// Governed by CAPCO-2016 §H.7 (pp122-130) "FOREIGN GOVERNMENT INFORMATION"
/// and specifically §H.7 p122 for the source-concealed banner grammar.
/// The canonical operational rules are:
///
/// - FGI with a known source is marked as `FGI [TRIGRAPH]` in the portion
///   mark and `FGI [COUNTRY]` in the banner line (§H.7 p122-123).
/// - FGI from an unknown or concealed source uses the bare `FGI` marker
///   (no trigraph) per §H.7 p122 ("If the specific country is unknown,
///   the marking FGI may be used without identifying the country").
///   This maps to `Present { concealed: true, countries: [] }`.
///
/// Per `docs/plans/2026-05-01-lattice-design.md` §4.8 and `marque-applied.md`
/// §4.8.
///
/// ## §4.8.5 worked example
///
/// Two portions: `(C//NF)` and `(//GBR TS)`. The first portion carries US
/// CONFIDENTIAL + NOFORN; the second carries FGI `GBR` at the TS level
/// (FGI classification blocks are space-delimited per `parse_fgi_classification`;
/// the hyphenated form `GBR-TS` does not match the grammar). After
/// page-level join the result is:
///
/// - Classification: `TOP SECRET` (max of C and TS = TS)
/// - FGI: `Present { concealed: false, countries: {GBR} }` (GBR from portion 2)
/// - Dissem: `NOFORN` (from portion 1)
///
/// Banner: `TOP SECRET//FGI GBR//NOFORN`
///
/// The FgiSet join absorbs the UK classification into the page state via the
/// FGI country presence; the classification axis uses OrdMax to reach TS.
///
/// ## Coverage delimitation
///
/// `FgiSet` models FGI-attribution (country of origin) only. JOINT-attribution
/// (content jointly produced by two or more governments) is modeled separately
/// via `MarkingClassification::Joint` on the classification axis. The two are
/// mutually exclusive at the portion level. Cross-system join (e.g., a page
/// that mixes FGI GBR portions with JOINT USA GBR portions) is not modeled
/// by `FgiSet` — that is the JOINT-attribution incompatibility-class reframe
/// deferred to Stage 4 of the engine refactor (per
/// `docs/plans/2026-05-01-lattice-design.md` §4.7, open question "FGI vs
/// JOINT attribution").
///
/// **`#[non_exhaustive]`** (B-4, PR 4b-B 8th-pass follow-up): the
/// state space is closed today (`None` and `Present { concealed, countries }`
/// over an open `CountryCode` axis), but future CAPCO grammar
/// extensions or decoder-confidence partial states may add a
/// `Partial` / `Concealed { partial_countries: ... }` variant
/// without breaking the closed-set contract for the existing two
/// — declaring `#[non_exhaustive]` keeps downstream matchers honest
/// (they MUST handle the unknown case with a wildcard arm).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum FgiSet {
    /// No FGI present.
    #[default]
    None,
    /// FGI present. `concealed = true` means "source-concealed" (bare `FGI`
    /// marker per §H.7 p122) — countries must be empty when this is set;
    /// join preserves concealment because a source-concealed entry on any
    /// portion requires a source-concealed banner.
    Present {
        concealed: bool,
        countries: BTreeSet<CountryCode>,
    },
}

impl FgiSet {
    pub fn empty() -> Self {
        Self::None
    }

    pub fn from_marker(marker: Option<&FgiMarker>) -> Self {
        match marker {
            None => Self::None,
            Some(FgiMarker::SourceConcealed) => Self::Present {
                concealed: true,
                countries: BTreeSet::new(),
            },
            Some(FgiMarker::Acknowledged { countries, .. }) => Self::Present {
                concealed: false,
                countries: countries.iter().copied().collect(),
            },
        }
    }

    pub fn to_marker(&self) -> Option<FgiMarker> {
        match self {
            Self::None => None,
            Self::Present {
                concealed,
                countries,
            } => {
                if *concealed {
                    Some(FgiMarker::SourceConcealed)
                } else {
                    // `Present { concealed: false, countries }` is
                    // produced only by lattice operations that either
                    // carry over a non-empty input set or intersect to
                    // a non-empty result (the meet collapses to `None`
                    // when the intersection is empty — see `meet`
                    // below). So `acknowledged(...)` should always
                    // yield `Some` here in practice; if a future
                    // refactor produces a `Present` with an empty
                    // open-source set, we surface `None` rather than
                    // fabricating `SourceConcealed`, which would be a
                    // semantic lie about the source.
                    FgiMarker::acknowledged(countries.iter().copied())
                }
            }
        }
    }
}

impl JoinSemilattice for FgiSet {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::None, o) | (o, Self::None) => o.clone(),
            (
                Self::Present {
                    concealed: a_c,
                    countries: a_cs,
                },
                Self::Present {
                    concealed: b_c,
                    countries: b_cs,
                },
            ) => {
                let concealed = *a_c || *b_c;
                if concealed {
                    Self::Present {
                        concealed: true,
                        countries: BTreeSet::new(),
                    }
                } else {
                    let mut countries = a_cs.clone();
                    countries.extend(b_cs.iter().copied());
                    Self::Present {
                        concealed: false,
                        countries,
                    }
                }
            }
        }
    }
}

impl MeetSemilattice for FgiSet {
    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::None, _) | (_, Self::None) => Self::None,
            (
                Self::Present {
                    concealed: a_c,
                    countries: a_cs,
                },
                Self::Present {
                    concealed: b_c,
                    countries: b_cs,
                },
            ) => {
                // P-9-1 (9th-pass): source-concealed acts as lattice TOP
                // in the FGI source-disclosure dimension.  The join already
                // makes concealed dominate (P-1, 8th-pass), so the dual
                // absorption law `a ⊓ (a ⊔ b) = a` requires meet to treat
                // the concealed form as top — meet(x, top) = x.
                //
                // Three cases:
                //   (a) both concealed  → concealed (idempotent top)
                //   (b) one concealed, one acknowledged → acknowledged side
                //       (meet with top returns the other operand)
                //   (c) both acknowledged → intersect country sets
                //
                // Authority: §H.7 p128 ("A document containing portions of
                // both source-concealed FGI and source-acknowledged FGI must
                // have only the 'FGI' marking without source
                // trigraph(s)/tetragraph(s) in the banner line, as it is the
                // most restrictive form of the marking") — concealed is the
                // strictest / highest element. Verified 2026-05-16 against
                // crates/capco/docs/CAPCO-2016.md.
                match (*a_c, *b_c) {
                    (true, true) => {
                        // (a) both concealed — top ⊓ top = top.
                        Self::Present {
                            concealed: true,
                            countries: BTreeSet::new(),
                        }
                    }
                    (true, false) => {
                        // (b) self is concealed (top) → return other.
                        Self::Present {
                            concealed: false,
                            countries: b_cs.clone(),
                        }
                    }
                    (false, true) => {
                        // (b) other is concealed (top) → return self.
                        Self::Present {
                            concealed: false,
                            countries: a_cs.clone(),
                        }
                    }
                    (false, false) => {
                        // (c) both acknowledged — intersect country sets.
                        let countries: BTreeSet<CountryCode> =
                            a_cs.intersection(b_cs).copied().collect();
                        if countries.is_empty() {
                            // No common countries — collapse to bottom
                            // (no shared FGI on this page).
                            Self::None
                        } else {
                            Self::Present {
                                concealed: false,
                                countries,
                            }
                        }
                    }
                }
            }
        }
    }
}

// `FgiSet` deliberately does NOT implement `BoundedLattice` (B-1, PR 4b-B
// 8th-pass follow-up). Although `SourceConcealed` is a valid syntactic
// supersession-top for the `JoinSemilattice::join` operation (it dominates every
// non-concealed state), the `CountryCode` axis underneath
// `Present { concealed: false, countries: BTreeSet<CountryCode> }` is
// **open-vocabulary** — new trigraphs and tetragraphs land per ISMCAT
// schema updates without an FgiSet code change. There is no lawful
// finite "top" over the full `(concealed, countries)` Cartesian
// product, so the `SciSet` / `SarSet` / `AeaSet` open-vocab precedent
// applies. Use `FgiSet::empty()` / `FgiSet::default()` (== `Self::None`)
// for the bottom; callers that need the source-concealed supersession
// sentinel construct it explicitly via
// `FgiSet::from_marker(Some(&FgiMarker::SourceConcealed))`.

// ---------------------------------------------------------------------------
// FgiSet::from_attrs_iter — unions per-portion FgiMarker with
// classification-derived producers (NATO / JOINT / FGI variants).
// ---------------------------------------------------------------------------

impl FgiSet {
    /// Construct an `FgiSet` from a slice of `CanonicalAttrs` —
    /// unions per-portion `fgi_marker` with the producers implied by
    /// the per-portion classification axis:
    ///
    /// - `MarkingClassification::Fgi(_)` contributes its trigraph list
    ///   (or `SourceConcealed` if the list is empty).
    /// - `MarkingClassification::Nato(_)` contributes the `NATO` code.
    /// - `MarkingClassification::Joint(_)` contributes the non-US
    ///   producers from its country list.
    /// - Other classification variants contribute nothing.
    /// - An explicit `FgiMarker::SourceConcealed` on any portion makes
    ///   the result source-concealed (`Present { concealed: true, .. }`)
    ///   regardless of other contributions — concealed is the dominating
    ///   element per §H.7 p124 (precedence rule) + §H.7 p128 (worked-
    ///   example restatement / most-restrictive-form justification).
    ///
    /// §-authority (verified 2026-05-22 against
    /// `crates/capco/docs/CAPCO-2016.md`):
    /// - §H.7 p122 (FGI source-concealed grammar).
    /// - §H.7 p123 (FGI acknowledged + classification-derived producers).
    /// - §H.7 p124 (precedence rule for banner-line guidance when a
    ///   document contains portions of both source-concealed and
    ///   source-acknowledged FGI — only the bare "FGI" form appears in
    ///   the banner line, no trigraphs/tetragraphs).
    /// - §H.7 p128 (worked-example restatement: concealed-dominates-
    ///   acknowledged because the concealed form is the most
    ///   restrictive).
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        let mut has_any_fgi = false;
        let mut has_source_concealed = false;
        let mut countries: BTreeSet<CountryCode> = BTreeSet::new();

        for attrs in portions {
            // Explicit FGI marker on the portion.
            if let Some(marker) = &attrs.fgi_marker {
                has_any_fgi = true;
                match marker {
                    FgiMarker::SourceConcealed => {
                        has_source_concealed = true;
                    }
                    FgiMarker::Acknowledged {
                        countries: marker_countries,
                        ..
                    } => {
                        countries.extend(marker_countries.iter().copied());
                    }
                }
            }

            // Classification-derived producers (NATO / JOINT / FGI variants).
            match &attrs.classification {
                Some(MarkingClassification::Fgi(fgi)) => {
                    has_any_fgi = true;
                    if fgi.countries.is_empty() {
                        has_source_concealed = true;
                    } else {
                        countries.extend(fgi.countries.iter().copied());
                    }
                }
                Some(MarkingClassification::Nato(_)) => {
                    has_any_fgi = true;
                    if let Some(nato) = CountryCode::try_new(b"NATO") {
                        countries.insert(nato);
                    }
                }
                Some(MarkingClassification::Joint(j)) => {
                    has_any_fgi = true;
                    let usa = CountryCode::try_new(b"USA");
                    for c in j.countries.iter() {
                        if Some(*c) != usa {
                            countries.insert(*c);
                        }
                    }
                }
                _ => {}
            }
        }

        if !has_any_fgi {
            return Self::None;
        }

        // §H.7 p124 (precedence rule) + §H.7 p128 (worked-example
        // restatement): source-concealed dominates open sources.
        if has_source_concealed {
            return Self::Present {
                concealed: true,
                countries: BTreeSet::new(),
            };
        }

        if countries.is_empty() {
            // Defensive: an explicit `Acknowledged{}` marker with an
            // empty country list (which the type-system should
            // currently prevent — `acknowledged()` returns `None`)
            // collapses to `None` rather than fabricating an
            // acknowledged-but-empty `Present`.
            Self::None
        } else {
            Self::Present {
                concealed: false,
                countries,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::{Classification, NatoClassification};

    #[test]
    fn fgi_set_concealed_supersedes_acknowledged() {
        let conc = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let ack = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let j = conc.join(&ack);
        match j {
            FgiSet::Present {
                concealed,
                countries,
            } => {
                assert!(concealed);
                assert!(countries.is_empty());
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_set_join_unions_acknowledged_countries() {
        let a = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let b = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"DEU").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let j = a.join(&b);
        match j {
            FgiSet::Present { countries, .. } => {
                assert_eq!(countries.len(), 2);
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_set_none_is_empty() {
        // B-1 (PR 4b-B 8th-pass): `FgiSet::bottom()` retired alongside
        // the `BoundedLattice` impl. `FgiSet::empty()` is the public
        // bottom constructor; `FgiSet::None` is the variant it maps to.
        assert_eq!(FgiSet::empty(), FgiSet::None);
    }

    // FgiSet — from_marker/to_marker round-trip + concealed branches

    #[test]
    fn fgi_set_from_marker_none_returns_none() {
        assert_eq!(FgiSet::from_marker(None), FgiSet::None);
    }

    #[test]
    fn fgi_set_from_marker_source_concealed_is_concealed() {
        let m = FgiMarker::SourceConcealed;
        let set = FgiSet::from_marker(Some(&m));
        assert!(matches!(
            set,
            FgiSet::Present {
                concealed: true,
                ..
            }
        ));
    }

    #[test]
    fn fgi_set_from_marker_acknowledged_is_open() {
        let m = FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()])
            .expect("non-empty country list");
        let set = FgiSet::from_marker(Some(&m));
        match set {
            FgiSet::Present {
                concealed,
                countries,
            } => {
                assert!(!concealed);
                assert_eq!(countries.len(), 1);
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_marker_acknowledged_rejects_empty_list() {
        // FR-017 / CHK028: the empty-Acknowledged shape MUST be
        // type-system-unrepresentable from the public surface.
        let empty: Vec<CountryCode> = Vec::new();
        assert!(FgiMarker::acknowledged(empty).is_none());
    }

    #[test]
    fn fgi_set_to_marker_none_for_none() {
        assert!(FgiSet::None.to_marker().is_none());
    }

    #[test]
    fn fgi_set_to_marker_concealed_emits_source_concealed_variant() {
        let set = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let marker = set.to_marker().expect("Some");
        assert!(matches!(marker, FgiMarker::SourceConcealed));
    }

    #[test]
    fn fgi_set_to_marker_open_round_trips_countries() {
        let mut countries = BTreeSet::new();
        countries.insert(CountryCode::try_new(b"GBR").unwrap());
        countries.insert(CountryCode::try_new(b"DEU").unwrap());
        let set = FgiSet::Present {
            concealed: false,
            countries,
        };
        let marker = set.to_marker().expect("Some");
        match marker {
            FgiMarker::Acknowledged { countries, .. } => assert_eq!(countries.len(), 2),
            FgiMarker::SourceConcealed => panic!("expected acknowledged variant"),
        }
    }

    #[test]
    fn fgi_set_empty_is_none() {
        assert_eq!(FgiSet::empty(), FgiSet::None);
    }

    #[test]
    fn fgi_set_default_is_none() {
        let d: FgiSet = FgiSet::default();
        assert_eq!(d, FgiSet::None);
    }

    // `fgi_set_top_is_concealed_empty` retired in B-1 (PR 4b-B 8th-pass
    // follow-up). `FgiSet` no longer implements `BoundedLattice`; the
    // `SourceConcealed` supersession sentinel is still reachable via
    // `FgiSet::from_marker(Some(&FgiMarker::SourceConcealed))`, exercised
    // by `fgi_set_meet_both_concealed_preserved` below.

    #[test]
    fn fgi_set_join_none_right_preserves_left() {
        let left = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        assert_eq!(left.join(&FgiSet::None), left);
    }

    #[test]
    fn fgi_set_join_none_left_preserves_right() {
        let right = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        assert_eq!(FgiSet::None.join(&right), right);
    }

    #[test]
    fn fgi_set_meet_both_concealed_preserved() {
        let a = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let b = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        let m = a.meet(&b);
        assert!(matches!(
            m,
            FgiSet::Present {
                concealed: true,
                ..
            }
        ));
    }

    #[test]
    fn fgi_set_meet_disjoint_countries_collapses_to_none() {
        let a = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"GBR").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        let b = FgiSet::Present {
            concealed: false,
            countries: [CountryCode::try_new(b"DEU").unwrap()]
                .iter()
                .copied()
                .collect(),
        };
        assert_eq!(a.meet(&b), FgiSet::None);
    }

    #[test]
    fn fgi_set_meet_common_country_preserved() {
        let a = FgiSet::Present {
            concealed: false,
            countries: [
                CountryCode::try_new(b"GBR").unwrap(),
                CountryCode::try_new(b"DEU").unwrap(),
            ]
            .iter()
            .copied()
            .collect(),
        };
        let b = FgiSet::Present {
            concealed: false,
            countries: [
                CountryCode::try_new(b"GBR").unwrap(),
                CountryCode::try_new(b"FRA").unwrap(),
            ]
            .iter()
            .copied()
            .collect(),
        };
        let m = a.meet(&b);
        match m {
            FgiSet::Present {
                concealed,
                countries,
            } => {
                assert!(!concealed);
                assert_eq!(countries.len(), 1);
                assert!(countries.contains(&CountryCode::try_new(b"GBR").unwrap()));
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fgi_set_meet_none_collapses_to_none() {
        let a = FgiSet::Present {
            concealed: true,
            countries: BTreeSet::new(),
        };
        assert_eq!(FgiSet::None.meet(&a), FgiSet::None);
        assert_eq!(a.meet(&FgiSet::None), FgiSet::None);
        assert_eq!(FgiSet::None.meet(&FgiSet::None), FgiSet::None);
    }

    // FgiSet::from_attrs_iter — happy-path + concealed-dominates + JOINT
    // producer extraction + associativity.

    #[test]
    fn fgi_set_from_attrs_iter_empty_returns_none() {
        let portions: [CanonicalAttrs; 0] = [];
        assert_eq!(FgiSet::from_attrs_iter(&portions), FgiSet::None);
    }

    #[test]
    fn fgi_set_from_attrs_iter_nato_classification_yields_nato_producer() {
        let mut p = CanonicalAttrs::default();
        p.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
        let result = FgiSet::from_attrs_iter(&[p]);
        match result {
            FgiSet::Present {
                concealed: false,
                countries,
            } => {
                let nato = CountryCode::try_new(b"NATO").unwrap();
                assert!(countries.contains(&nato), "expected NATO producer");
            }
            other => panic!("expected Present {{concealed: false}}, got {other:?}"),
        }
    }

    #[test]
    fn fgi_set_from_attrs_iter_concealed_dominates_acknowledged() {
        // §H.7 p124 (precedence rule) + §H.7 p128 (worked-example
        // restatement): mixed concealed + acknowledged → concealed wins.
        let mut concealed_portion = CanonicalAttrs::default();
        concealed_portion.fgi_marker = Some(FgiMarker::SourceConcealed);
        let mut acknowledged_portion = CanonicalAttrs::default();
        acknowledged_portion.fgi_marker =
            FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]);
        let result = FgiSet::from_attrs_iter(&[concealed_portion, acknowledged_portion]);
        assert!(
            matches!(
                result,
                FgiSet::Present {
                    concealed: true,
                    ..
                }
            ),
            "concealed must dominate; got {result:?}"
        );
    }

    #[test]
    fn fgi_set_from_attrs_iter_joint_excludes_usa_producer() {
        // JOINT producers contribute to FGI minus USA (USA is implicit
        // owner, not a foreign source).
        let mut joint_portion = CanonicalAttrs::default();
        joint_portion.classification = Some(MarkingClassification::Joint(
            marque_ism::JointClassification {
                level: Classification::Secret,
                countries: Box::new([
                    CountryCode::try_new(b"USA").unwrap(),
                    CountryCode::try_new(b"GBR").unwrap(),
                ]),
            },
        ));
        let result = FgiSet::from_attrs_iter(&[joint_portion]);
        match result {
            FgiSet::Present {
                concealed: false,
                countries,
            } => {
                let usa = CountryCode::try_new(b"USA").unwrap();
                let gbr = CountryCode::try_new(b"GBR").unwrap();
                assert!(!countries.contains(&usa), "USA must NOT appear");
                assert!(countries.contains(&gbr), "GBR must appear");
            }
            other => panic!("expected Present {{concealed: false}}, got {other:?}"),
        }
    }

    #[test]
    fn fgi_set_from_attrs_iter_associative_with_join() {
        // Lattice law: from_attrs_iter(&a ++ b ++ c) == from_attrs_iter(&a).join(&...)
        // The construction path is union-based; assembling per-portion
        // via repeated join must agree with bulk construction.
        let mut p1 = CanonicalAttrs::default();
        p1.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]);
        let mut p2 = CanonicalAttrs::default();
        p2.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"DEU").unwrap()]);
        let mut p3 = CanonicalAttrs::default();
        p3.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"FRA").unwrap()]);

        let bulk = FgiSet::from_attrs_iter(&[p1.clone(), p2.clone(), p3.clone()]);
        let step = FgiSet::from_attrs_iter(&[p1])
            .join(&FgiSet::from_attrs_iter(&[p2]))
            .join(&FgiSet::from_attrs_iter(&[p3]));
        assert_eq!(
            bulk, step,
            "bulk construction must agree with iterated join"
        );
    }
}
