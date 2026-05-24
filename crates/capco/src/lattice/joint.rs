// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`JointSet`] — 4-variant state with producer-disunity collapse.
//!
//! Join-only per issue #456 / PR #502 — `meet` cannot be defined over
//! the `Mixed` / `DisunityCollapse` variants without violating the
//! dual absorption law and (pre-split) idempotence on
//! `DisunityCollapse` self-pairs.

use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, JointClassification, MarkingClassification,
};
use marque_scheme::JoinSemilattice;
use smallvec::SmallVec;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// JointSet — 4-variant state with producer-disunity collapse
// ---------------------------------------------------------------------------

/// Lattice form of the JOINT classification axis.
///
/// The state space is a closed four-variant enum that captures the
/// decision tree from CAPCO-2016 §H.3 + §H.7. The `Mixed` variant
/// distinguishes "no JOINT seen" (the lattice identity `Bottom`) from
/// "JOINT and non-JOINT both observed" (an absorbing state) so `join`
/// stays **associative**.
///
/// - `Bottom`: no JOINT-bearing portion observed. Lattice identity.
/// - `UnanimousProducers`: every observed portion is JOINT with the
///   same producer set. The banner is `//JOINT [class] [LIST]` per
///   §H.3 p56.
/// - `DisunityCollapse`: every observed portion is JOINT but the
///   producer lists differ. Non-US producers migrate to FGI per
///   §H.7 p123.
/// - `Mixed`: at least one JOINT portion AND at least one
///   non-JOINT portion observed. Absorbing for the JOINT axis —
///   §H.3 p57 "JOINT marking is not carried forward to the banner
///   line in US documents." Once `Mixed`, the JOINT axis cannot
///   resurrect to `UnanimousProducers` regardless of subsequent
///   joins.
///
/// The transitions on `JoinSemilattice::join` are structural operations on
/// the deterministic state space — not content normalization — and the
/// property test `joint_disunity_lattice_laws` exhausts the state-space
/// cube to verify assoc/comm/idem.
///
/// **`JointDisunityCollapseRule`** (the JOINT Warn rule, in
/// `crates/capco/src/rules.rs`) reads the post-projection JointSet
/// state from the engine's `PageContext` flow. It fires only on
/// `DisunityCollapse`; `Mixed` is the §H.3 p57 case where FGI
/// migration rides through `expected_fgi_marker` and the rule does
/// not fire. The lattice does not itself emit the diagnostic; the
/// rule does.
///
/// §-authority (CAPCO-2016.md):
///
/// - §H.3 p56 (JOINT classification grammar).
/// - §H.3 pp55-59 (JOINT worked examples).
/// - §H.3 p57 ("JOINT marking not carried forward to
///   the banner line in US documents").
/// - §H.7 p123 (FGI source-acknowledged form for disunity-collapse
///   non-US producer migration).
///
/// **`#[non_exhaustive]`**: the four-variant decision tree is the
/// lawful closed set per §H.3 p57 today, but future CAPCO revisions
/// or partial-decoder states may add a `PartialDisunity` / `Inferred`
/// variant — declaring `#[non_exhaustive]` requires downstream
/// matchers to handle the unknown case with a wildcard arm so a future
/// variant addition is a non-breaking change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum JointSet {
    /// No JOINT-bearing portion observed. Lattice identity for `join`.
    #[default]
    Bottom,

    /// Every portion is JOINT-classified and every portion carries
    /// the same producer list. The banner is `//JOINT [class]
    /// [LIST]` per §H.3 p56.
    UnanimousProducers {
        /// Highest level observed via OrdMax across portions.
        level: Classification,
        /// The unanimous producer list (USA always in).
        producers: BTreeSet<CountryCode>,
    },

    /// Disunity observed: every portion is JOINT-classified but the
    /// producer lists differ across portions. The lattice records the
    /// union of non-US producers; the engine's banner rendering migrates
    /// them to FGI [LIST] per §H.7 p123 and `JointDisunityCollapseRule`
    /// surfaces the cross-axis transformation to the user.
    DisunityCollapse {
        /// Highest level observed via OrdMax across portions.
        highest_level: Classification,
        /// Union of non-US producers across JOINT portions.
        union_non_us_producers: BTreeSet<CountryCode>,
    },

    /// At least one JOINT portion AND at least one non-JOINT
    /// portion observed. §H.3 p57: JOINT does not roll up to the
    /// banner in US documents. Absorbing for the JOINT axis — once
    /// `Mixed`, subsequent joins cannot resurrect a JOINT roll-up
    /// state. Non-US producers ride to `FgiSet` via
    /// `expected_fgi_marker`; `JointDisunityCollapseRule` does not fire
    /// on `Mixed`.
    Mixed,
}

impl JointSet {
    /// An empty JointSet — the lattice bottom.
    pub fn empty() -> Self {
        Self::Bottom
    }

    /// Construct from a slice of `CanonicalAttrs`.
    ///
    /// Per §H.3 p57, the all-JOINT-or-not distinction
    /// drives the state-space branch:
    ///
    /// 1. **No portions / no JOINT portion** → `Bottom` (identity).
    /// 2. **All portions JOINT** with identical producer lists →
    ///    `UnanimousProducers { OrdMax(level), countries }`.
    /// 3. **All portions JOINT** with disagreeing producer lists →
    ///    `DisunityCollapse { OrdMax(level), union_non_us }`.
    /// 4. **Mixed JOINT + non-JOINT** → `Mixed`. The §H.3 p57
    ///    "JOINT does not roll up in US documents" rule.
    ///    `JointDisunityCollapseRule` does not fire in this case —
    ///    JOINT non-US producers ride to FGI via the
    ///    PageContext-resident `expected_fgi_marker` path.
    ///
    /// **Empty-producer-list defensive shape**: an `UnanimousProducers`
    /// variant with an empty producer set is malformed per §H.3
    /// (JOINT requires USA + at least one co-owner). This
    /// constructor returns `Bottom` rather than the malformed
    /// `UnanimousProducers { producers: ∅ }` to keep the lattice
    /// state space well-formed.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        if portions.is_empty() {
            return Self::Bottom;
        }

        // Separate JOINT portions from non-JOINT portions.
        //
        // **Malformed JOINT portions are dropped at this point.** A
        // JOINT portion is malformed when it fails either of the two
        // §H.3 p56 grammar invariants:
        //
        // 1. Producer list must be non-empty (`!j.countries.is_empty()`).
        // 2. **USA must appear in the producer list** ("USA always
        //    appears as the OWNER/PRODUCER" per §H.3 p56). Pre-fix
        //    (PR 4b-B 9th-pass), only invariant #1 was enforced; a
        //    `JointClassification { countries: [GBR] }` (one country,
        //    no USA) was pushed to `joint_portions`, treated as
        //    well-formed unanimous, and emitted a JOINT banner
        //    without USA — unrepresentable in the §H.3 grammar.
        //
        // Per the existing empty-producer rationale: dropping
        // malformed portions at scan time keeps the remaining
        // (well-formed) portions in the correct shape to drive the
        // lattice state per the standard rules: zero remaining →
        // `Bottom`; well-formed unanimous → `UnanimousProducers`;
        // well-formed disagreement → `DisunityCollapse`.
        //
        // The malformed portion is **invisible to the JOINT axis**
        // (does not count as "non-JOINT" either). The classification
        // axis still consumes the malformed portion's
        // `effective_level()` for the level-chain max via
        // `ClassificationLattice`; this normalization is
        // JOINT-axis-only.
        //
        // Authority: §H.3 p56 (JOINT grammar requires non-empty
        // `[LIST]` AND USA in the producer list). Verified
        // 2026-05-16 against CAPCO-2016.md.
        let has_usa = |j: &JointClassification| j.countries.contains(&CountryCode::USA);
        // Inline-4 covers the typical JOINT portion count per page;
        // deeply collaborative documents with 5+ JOINT portions spill
        // to heap cleanly.
        let mut joint_portions: SmallVec<[&JointClassification; 4]> =
            SmallVec::with_capacity(portions.len().min(4));
        let mut has_non_joint = false;
        for p in portions {
            match &p.classification {
                // Well-formed: non-empty AND contains USA.
                Some(MarkingClassification::Joint(j)) if !j.countries.is_empty() && has_usa(j) => {
                    joint_portions.push(j)
                }
                // Malformed JOINT (empty producer list OR no USA):
                // drop, treat as invisible to the JOINT axis. The
                // portion is still a CanonicalAttrs entry on the
                // page, so it doesn't count as "non-JOINT" either —
                // the malformed shape contributes nothing.
                Some(MarkingClassification::Joint(_)) => {}
                Some(_) => has_non_joint = true,
                None => has_non_joint = true,
            }
        }

        if joint_portions.is_empty() {
            return Self::Bottom;
        }

        // §H.3 p57: in US documents (mixed JOINT + US),
        // JOINT does not roll up. The FGI-migration path is
        // PageContext::expected_fgi_marker; we return `Mixed`
        // (absorbing) and the JOINT disunity rule does not fire.
        if has_non_joint {
            return Self::Mixed;
        }

        // All (well-formed) portions JOINT: check unanimity on
        // producer lists.
        let first_producers: BTreeSet<CountryCode> =
            joint_portions[0].countries.iter().copied().collect();
        let highest_level = joint_portions
            .iter()
            .map(|j| j.level)
            .max()
            .unwrap_or(Classification::Unclassified);

        let unanimous = joint_portions.iter().all(|j| {
            let set: BTreeSet<CountryCode> = j.countries.iter().copied().collect();
            set == first_producers
        });

        if unanimous {
            // Note: `first_producers` is guaranteed non-empty here
            // because empty-producer portions were dropped above.
            // The defensive `is_empty()` check at this site is
            // therefore redundant post-fix; we keep an assertion-
            // shaped early return for belt-and-braces (any future
            // refactor that re-introduces empty-producer portions
            // before this point will fail loud rather than producing
            // a malformed `UnanimousProducers { producers: ∅ }`).
            if first_producers.is_empty() {
                return Self::Bottom;
            }
            Self::UnanimousProducers {
                level: highest_level,
                producers: first_producers,
            }
        } else {
            // Disunity: union of non-US producers across all JOINT
            // portions.
            let mut union_non_us: BTreeSet<CountryCode> = BTreeSet::new();
            for j in &joint_portions {
                for c in j.countries.iter() {
                    if *c != CountryCode::USA {
                        union_non_us.insert(*c);
                    }
                }
            }
            Self::DisunityCollapse {
                highest_level,
                union_non_us_producers: union_non_us,
            }
        }
    }

    /// Whether this JointSet represents a disunity-collapse state
    /// (`JointDisunityCollapseRule` reads this).
    pub fn is_disunity_collapse(&self) -> bool {
        matches!(self, Self::DisunityCollapse { .. })
    }

    /// Read access to the non-US producer set on a `DisunityCollapse`
    /// state, or `None` otherwise.
    pub fn disunity_collapse_non_us_producers(&self) -> Option<&BTreeSet<CountryCode>> {
        match self {
            Self::DisunityCollapse {
                union_non_us_producers,
                ..
            } => Some(union_non_us_producers),
            _ => None,
        }
    }

    /// Read access to the highest level observed across JOINT
    /// portions; `None` for `Bottom` and `Mixed` (the latter does
    /// not carry a per-axis level since JOINT doesn't roll up).
    pub fn highest_level(&self) -> Option<Classification> {
        match self {
            Self::Bottom | Self::Mixed => None,
            Self::UnanimousProducers { level, .. } => Some(*level),
            Self::DisunityCollapse { highest_level, .. } => Some(*highest_level),
        }
    }

    /// Whether the page is in the `Mixed` state — JOINT and non-JOINT
    /// portions both observed. JOINT does not roll up to the banner
    /// in this case (§H.3 p57).
    pub fn is_mixed(&self) -> bool {
        matches!(self, Self::Mixed)
    }

    /// Convert back to a `MarkingClassification` for the banner.
    ///
    /// - `Bottom` → `None` (no JOINT portion observed; the banner
    ///   reads the class from `ClassificationLattice` and FGI from
    ///   `FgiSet` per the existing PageContext flow).
    /// - `Mixed` → `None` (§H.3 p57: JOINT does not roll up in US
    ///   documents; the banner reads the class from `Us(_)` and FGI
    ///   from the cross-axis fold).
    /// - `UnanimousProducers { level, producers }` → `Some(Joint(...))`.
    /// - `DisunityCollapse { highest_level, .. }` → `Some(Us(highest_level))`
    ///   (the non-US producers ride to FgiSet via a separate flow in
    ///   `CapcoMarking::join`).
    pub fn to_marking_classification(&self) -> Option<MarkingClassification> {
        match self {
            Self::Bottom | Self::Mixed => None,
            Self::UnanimousProducers { level, producers } => {
                let countries: Box<[CountryCode]> = producers
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                Some(MarkingClassification::Joint(JointClassification {
                    level: *level,
                    countries,
                }))
            }
            Self::DisunityCollapse { highest_level, .. } => {
                Some(MarkingClassification::Us(*highest_level))
            }
        }
    }
}

// Partial-lattice divergence note for `JointSet`.
//
// `JointSet` implements only `JoinSemilattice`, NOT `MeetSemilattice`.
// The `Mixed` / `DisunityCollapse` distinction is a record of observed
// page composition (join-side aggregation), not an algebraic element;
// `meet` has no natural reading for non-identical producer sets — the
// dual absorption law `a ⊓ (a ⊔ b) = a` cannot hold over the full state
// space. A `meet` would also be non-idempotent on `DisunityCollapse`
// self-pairs (`a ⊓ a = Bottom ≠ a`), since the fallback arm collapses
// every non-identical-payload pair to `Bottom`. The `Lattice` trait
// split (issue #456 / PR #502) into `JoinSemilattice` and
// `MeetSemilattice` halves lets `JointSet` implement only the join half,
// so the type system rejects any attempt to call `.meet()` on it at
// compile time.
impl JoinSemilattice for JointSet {
    ///   with union of non-US producers and max level.
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            // Mixed is absorbing for non-Bottom operands. §H.3 p57.
            // We deliberately let Bottom ⊔ Mixed = Mixed propagate
            // (Bottom is the identity, Mixed is the new state).
            (Self::Mixed, _) | (_, Self::Mixed) => Self::Mixed,
            (Self::Bottom, x) | (x, Self::Bottom) => x.clone(),
            (
                Self::UnanimousProducers {
                    level: l1,
                    producers: p1,
                },
                Self::UnanimousProducers {
                    level: l2,
                    producers: p2,
                },
            ) => {
                if p1 == p2 {
                    Self::UnanimousProducers {
                        level: (*l1).max(*l2),
                        producers: p1.clone(),
                    }
                } else {
                    let mut non_us: BTreeSet<CountryCode> = BTreeSet::new();
                    for c in p1.iter().chain(p2.iter()) {
                        if *c != CountryCode::USA {
                            non_us.insert(*c);
                        }
                    }
                    Self::DisunityCollapse {
                        highest_level: (*l1).max(*l2),
                        union_non_us_producers: non_us,
                    }
                }
            }
            (
                Self::UnanimousProducers {
                    level: lu,
                    producers: pu,
                },
                Self::DisunityCollapse {
                    highest_level: ld,
                    union_non_us_producers: nd,
                },
            )
            | (
                Self::DisunityCollapse {
                    highest_level: ld,
                    union_non_us_producers: nd,
                },
                Self::UnanimousProducers {
                    level: lu,
                    producers: pu,
                },
            ) => {
                let mut non_us = nd.clone();
                for c in pu.iter() {
                    if *c != CountryCode::USA {
                        non_us.insert(*c);
                    }
                }
                Self::DisunityCollapse {
                    highest_level: (*lu).max(*ld),
                    union_non_us_producers: non_us,
                }
            }
            (
                Self::DisunityCollapse {
                    highest_level: l1,
                    union_non_us_producers: n1,
                },
                Self::DisunityCollapse {
                    highest_level: l2,
                    union_non_us_producers: n2,
                },
            ) => {
                let mut non_us = n1.clone();
                non_us.extend(n2.iter().copied());
                Self::DisunityCollapse {
                    highest_level: (*l1).max(*l2),
                    union_non_us_producers: non_us,
                }
            }
        }
    }
}

// `JointSet` does NOT implement `BoundedLattice`: producer lists are
// open-vocabulary over `CountryCode`, and there is no lawful finite
// top variant under the §H.3 grammar. Use `JointSet::empty()` /
// `JointSet::default()` for the bottom.
