// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`ClassificationLattice`] — bounded OrdMax over the US classification
//! chain with variant-preserving / payload-unioning tiebreaks.

use marque_ism::{CanonicalAttrs, Classification, MarkingClassification};
use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};

// ---------------------------------------------------------------------------
// ClassificationLattice — bounded OrdMax over US chain + variant-preserving
// ---------------------------------------------------------------------------

/// Lattice form of the classification axis: `Option<MarkingClassification>`
/// with `OrdMax` over `effective_level()` and variant-preserving
/// tie-break on equal level.
///
/// The classification axis is structurally a bounded total order:
/// `Unclassified < Restricted < Confidential < Secret < TopSecret`
/// per CAPCO-2016 §H.1 pp47-54 (US-domestic levels) and §H.2 p55 /
/// `NatoClassification::us_equivalent()` (NATO `NR` maps to
/// `Restricted` in the foreign-interop tier between U and C). M-7
/// (PR 4b-B follow-up): the chain is five elements, not four —
/// `Restricted` survives as a foreign-interop tier for portions
/// that carry NATO `NR` or an FGI source whose foreign system has
/// a RESTRICTED level (`FgiClassification.level = Restricted`).
/// Foreign classifications normalize to the US chain at portion-
/// parse time via §H.7 pp123-125's reciprocal-classification rule
/// (`MarkingClassification::effective_level()`), so cross-branch
/// joins do not arise in the lattice — the lattice always sees a
/// US-chain level.
///
/// **Variant preservation.** Naive `OrdMax` over `effective_level()`
/// would lose `Nato` / `Fgi` / `Joint` / `Conflict` variant tags. The
/// join compares two `MarkingClassification`s by `effective_level()`
/// and returns the variant with the higher level **as-is**. On
/// equal level the implementation applies a deterministic, order-
/// independent variant precedence (lower number wins, so the
/// "canonical" variant of a level survives):
///
/// 1. `Us` (canonical per §H.7 reciprocal normalization)
/// 2. `Fgi`
/// 3. `Nato`
/// 4. `Joint`
/// 5. `Conflict`
///
/// Concretely, `Us(Secret).join(Fgi(Secret)) ==
/// Fgi(Secret).join(Us(Secret)) == Us(Secret)`, so commutativity
/// holds. Downstream attribution (`JointSet`, `FgiSet`,
/// `NatoClassLattice`) reads from these tags; the chosen precedence
/// matches the post-§H.7-reciprocal-normalization order rules
/// downstream expect.
///
/// **Same-variant payload tiebreak** (C-7 PR 4b-B follow-up). At
/// same level + same variant, country-bearing payloads (`Fgi`,
/// `Joint`) are **unioned** rather than picking one operand by
/// pointer order — `Fgi(S, [GBR]).join(Fgi(S, [CAN])) =
/// Fgi(S, [CAN, GBR])`. Union is commutative and idempotent, which
/// is what makes the lattice law hold. The union semantic also
/// matches the §H.7 p123 / §D.2 p28 banner-rollup rule that the
/// banner FGI list is the union of every observed foreign source.
/// `Conflict` payloads (`foreign: Box<ForeignClassification>`)
/// recurse into the same union rule when both sides carry the same
/// foreign variant; cross-variant payloads fall back to a
/// foreign-variant rank (Fgi < Nato < Joint).
///
/// `BoundedLattice` is implemented: top = `Some(Us(TopSecret))`,
/// bottom = `None`. The class chain is closed at five elements
/// (`Unclassified < Restricted < Confidential < Secret < TopSecret`,
/// M-7 PR 4b-B follow-up); no agency-extensibility concern.
///
/// §-authority (verified 2026-05-16 against CAPCO-2016.md):
/// - §H.1 pp47-54 (US class chain).
/// - §H.2 p55 (Non-US Protective Markings — refers to NATO chain
///   and to Manual Appendix A for FVEY equivalence).
/// - §H.7 pp123-125 (FGI grammar — supports the reciprocal-
///   classification convention applied at portion-parse time).
///
/// Manual Appendix A "Non-US Protective Markings (includes the
/// Five Eyes Marking Comparisons)" is referenced from §A.4 Table 1
/// p14 and §H.2 p55. It is the equivalence table that grounds the
/// `us_equivalent()` mapping from NATO levels to US levels, but
/// Appendix A is not vendored in `crates/capco/docs/CAPCO-2016.md`
/// (the markdown extract covers the lettered sections of the
/// Manual body — A through K — only, not the Appendices); the
/// appendix is an out-of-tree cross-reference, parallel to ISOO
/// section 3.3 in the `DeclassifyOnLattice` doc-comment.
///
/// **CV-3 (PR 4b-B 8th-pass follow-up).** Pre-CV-3 wording listed
/// `§A.4 p13 (IC Markings System Structure — classification hierarchy)`.
/// Verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`:
/// §A.4 p13 is a one-paragraph framing of "IC Markings System
/// Structure"; the §A.4 Table 1 IC Markings System Artifacts (which
/// names Appendix A as the FVEY equivalence reference) lands on
/// p14, not p13. Neither sub-page enumerates the classification
/// hierarchy itself. The §H.1 + §H.2 + Manual Appendix A citations
/// above carry the hierarchy + reciprocal-mapping authority that
/// the lattice actually relies on; §A.4 p13 was decorative.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassificationLattice(Option<MarkingClassification>);

impl ClassificationLattice {
    /// An empty classification — the lattice bottom.
    pub fn empty() -> Self {
        Self(None)
    }

    /// Construct a `ClassificationLattice` from an `Option<MarkingClassification>`.
    pub fn new(c: Option<MarkingClassification>) -> Self {
        Self(c)
    }

    /// Construct from a `CanonicalAttrs` slice — joins per-portion
    /// classifications by `OrdMax` over `effective_level()`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        portions
            .iter()
            .map(|p| Self(p.classification.clone()))
            .fold(Self::empty(), |acc, p| acc.join(&p))
    }

    /// Construct from an iterator of pre-computed `Option<MarkingClassification>`
    /// values — joins them by `OrdMax` over `effective_level()`.
    ///
    /// Prefer this over [`Self::from_attrs_iter`] when the caller has already
    /// mapped or transformed each portion's classification, because it avoids
    /// the need to clone a full `CanonicalAttrs` slice just to modify the
    /// `classification` field. CLONE-1 performance fix (issue #606): eliminates
    /// the `filtered: Vec<CanonicalAttrs>` allocation in `join_via_lattice_body`.
    pub fn from_classification_iter(
        iter: impl Iterator<Item = Option<MarkingClassification>>,
    ) -> Self {
        iter.map(Self).fold(Self::empty(), |acc, p| acc.join(&p))
    }

    /// Consume into the inner `Option<MarkingClassification>`.
    pub fn into_inner(self) -> Option<MarkingClassification> {
        self.0
    }

    /// Borrow the inner `Option<MarkingClassification>`.
    pub fn as_inner(&self) -> Option<&MarkingClassification> {
        self.0.as_ref()
    }
}

/// Deterministic variant-precedence rank for equal-effective-level
/// tiebreaks in `ClassificationLattice::join` / `meet`. Lower rank
/// wins. Order rationale: per CAPCO-2016 §H.7 pp123-125 reciprocal
/// normalization, `Us` is the canonical form at portion-parse time
/// for any portion that carries a US classification; the remaining
/// variants are foreign-source (`Fgi`), foreign-system (`Nato`),
/// or co-owned (`Joint`), with `Conflict` as the absorbing top
/// (it already carries the US-upgraded level in `us`).
fn classification_variant_rank(c: &MarkingClassification) -> u8 {
    match c {
        MarkingClassification::Us(_) => 0,
        MarkingClassification::Fgi(_) => 1,
        MarkingClassification::Nato(_) => 2,
        MarkingClassification::Joint(_) => 3,
        MarkingClassification::Conflict { .. } => 4,
    }
}

/// Same-variant / same-level payload tiebreaker for
/// `ClassificationLattice::join` (UNION semantic).
///
/// C-7 (PR 4b-B follow-up): the variant-rank tiebreaker alone is not
/// sufficient — two `Fgi` (or two `Joint`) values at the same level
/// with different country payloads previously fell through `ra <= rb`
/// returning the left operand, which broke commutativity. This helper
/// produces a join-result whose country payload is the **union** of
/// both operands' country lists, matching the §H.7 p123 banner-rollup
/// rule that the banner FGI list unions every observed foreign source
/// ("the one or more unique country trigraph(s) and/or tetragraph(s)
/// used in the portions"). Union is commutative and idempotent, so
/// commutativity + idempotence + associativity all hold without
/// further branching.
///
/// `Us`, `Nato`, and `Conflict` have no list payload at this level
/// (Nato carries only a tag; Us carries only the level; Conflict's
/// `foreign` is `Box<ForeignClassification>` which would need a
/// dedicated tiebreaker — for now we union the `foreign` payload
/// via the same rule when both sides are the same `ForeignClassification`
/// shape, else fall back to picking the canonically-smaller operand
/// by `effective_level()` + variant-rank tiebreak applied to
/// `foreign`'s inner variant).
///
/// **Companion**: see [`classification_meet_same_variant`] for the
/// dual-side semantic (INTERSECTION; bottom on disjoint payloads).
/// C-9 (PR 4b-B follow-up) split the two operations because using
/// union for both broke the absorption laws — `a ⊔ (a ⊓ b) = a` and
/// `a ⊓ (a ⊔ b) = a` cannot hold if `meet` and `join` are the same
/// op.
fn classification_join_same_variant(
    a: &MarkingClassification,
    b: &MarkingClassification,
) -> MarkingClassification {
    use std::collections::BTreeSet;
    // Idempotency short-circuit: if a == b, return a unchanged so
    // input order is preserved through the round-trip (avoids the
    // BTreeSet canonical-ordering side effect when a caller is
    // joining a payload with itself).
    if a == b {
        return a.clone();
    }
    match (a, b) {
        (MarkingClassification::Us(_), MarkingClassification::Us(_)) => a.clone(),
        (MarkingClassification::Fgi(fa), MarkingClassification::Fgi(fb)) => {
            // P-1 (8th-pass): source-concealed-dominates — if either side
            // has an empty countries list (the `//FGI [level]` form per
            // CAPCO-2016 §H.7 p124), the joined result MUST also be
            // source-concealed (empty countries). Chaining two lists when
            // one is empty returns the non-empty side and silently loses
            // the concealed signal — the banner incorrectly becomes
            // acknowledged `FGI [LIST]` instead of bare `FGI`.
            //
            // §-authority: §H.7 p124 (precedence rules for banner line
            // guidance: "if any of the portions have concealed FGI source
            // information... only the 'FGI' marking without the source
            // trigraph(s)/tetragraph(s) must appear in the banner line").
            // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
            let countries = if fa.countries.is_empty() || fb.countries.is_empty() {
                // Concealed dominates: produce the source-concealed form.
                Box::new([]) as Box<[marque_ism::CountryCode]>
            } else {
                let merged: BTreeSet<marque_ism::CountryCode> = fa
                    .countries
                    .iter()
                    .copied()
                    .chain(fb.countries.iter().copied())
                    .collect();
                merged.into_iter().collect::<Vec<_>>().into_boxed_slice()
            };
            MarkingClassification::Fgi(marque_ism::FgiClassification {
                level: fa.level, // same level — invariant of the tiebreaker
                countries,
            })
        }
        (MarkingClassification::Nato(_), MarkingClassification::Nato(_)) => a.clone(),
        (MarkingClassification::Joint(ja), MarkingClassification::Joint(jb)) => {
            let merged: BTreeSet<marque_ism::CountryCode> = ja
                .countries
                .iter()
                .copied()
                .chain(jb.countries.iter().copied())
                .collect();
            MarkingClassification::Joint(marque_ism::JointClassification {
                level: ja.level,
                countries: merged.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            })
        }
        (
            MarkingClassification::Conflict {
                us: ua,
                foreign: fa,
            },
            MarkingClassification::Conflict {
                us: ub,
                foreign: fb,
            },
        ) => {
            // us level matches by invariant (effective_level equality).
            // foreign payloads may differ; union the country-bearing
            // shapes when both sides carry the same ForeignClassification
            // variant; otherwise the variant-rank precedence on the
            // foreign payload picks the canonically-smaller side.
            let _ = (ua, ub);
            let foreign = merge_foreign_classification(fa, fb);
            MarkingClassification::Conflict {
                us: *ua,
                foreign: Box::new(foreign),
            }
        }
        // Different variants reach here only through a programming
        // error in `join`; defensively return `a`.
        _ => a.clone(),
    }
}

/// Merge two `ForeignClassification` payloads from same-level
/// `Conflict` variants. Same-variant union; cross-variant falls
/// back to the variant-rank precedence (lower rank wins).
fn merge_foreign_classification(
    a: &marque_ism::ForeignClassification,
    b: &marque_ism::ForeignClassification,
) -> marque_ism::ForeignClassification {
    use marque_ism::ForeignClassification;
    use std::collections::BTreeSet;
    match (a, b) {
        (ForeignClassification::Fgi(fa), ForeignClassification::Fgi(fb)) => {
            // P-1 (8th-pass): source-concealed-dominates — same fix as
            // `classification_join_same_variant`. Empty countries = the
            // source-concealed `//FGI [level]` form (§H.7 p124). If either
            // side is concealed, the joined result must be concealed.
            //
            // §-authority: §H.7 p124 (precedence rules for banner line
            // guidance: concealed dominates acknowledged in any mixed page).
            // Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.
            let countries = if fa.countries.is_empty() || fb.countries.is_empty() {
                Box::new([]) as Box<[marque_ism::CountryCode]>
            } else {
                let merged: BTreeSet<marque_ism::CountryCode> = fa
                    .countries
                    .iter()
                    .copied()
                    .chain(fb.countries.iter().copied())
                    .collect();
                merged.into_iter().collect::<Vec<_>>().into_boxed_slice()
            };
            ForeignClassification::Fgi(marque_ism::FgiClassification {
                level: fa.level,
                countries,
            })
        }
        (ForeignClassification::Nato(_), ForeignClassification::Nato(_)) => a.clone(),
        (ForeignClassification::Joint(ja), ForeignClassification::Joint(jb)) => {
            let merged: BTreeSet<marque_ism::CountryCode> = ja
                .countries
                .iter()
                .copied()
                .chain(jb.countries.iter().copied())
                .collect();
            ForeignClassification::Joint(marque_ism::JointClassification {
                level: ja.level,
                countries: merged.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            })
        }
        _ => {
            // Cross-variant: pick the canonically-smaller variant
            // (Fgi < Nato < Joint, mirroring `classification_variant_rank`
            // for the top-level shapes).
            let rank = |fc: &ForeignClassification| -> u8 {
                match fc {
                    ForeignClassification::Fgi(_) => 1,
                    ForeignClassification::Nato(_) => 2,
                    ForeignClassification::Joint(_) => 3,
                }
            };
            if rank(a) <= rank(b) {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}

/// Same-variant / same-level payload tiebreaker for
/// `ClassificationLattice::meet` (INTERSECTION semantic).
///
/// C-9 (PR 4b-B follow-up): the dual of [`classification_join_same_variant`].
/// `meet` is GLB on the country-list partial order:
///
/// - Equal payloads → that value (idempotence).
/// - One payload ⊆ the other → the smaller payload (it IS the GLB).
/// - Disjoint payloads → `None` (no common lower bound; meet falls
///   to the lattice bottom).
///
/// Returning `None` on disjoint payloads is what keeps the absorption
/// laws `a ⊔ (a ⊓ b) = a` and `a ⊓ (a ⊔ b) = a` holding: joining
/// `a` with `None` gives `a`, and meeting `a` with anything `≥ a`
/// gives `a`. Using `union` (the join semantic) on the meet side
/// broke both absorption laws.
///
/// `Us` and `Nato` carry no country payload at same level → meet
/// returns the value directly. `Conflict` is the absorbing top: at
/// same level + same shape (both Conflict, same foreign), meet is
/// that value; otherwise meet is `None`.
fn classification_meet_same_variant(
    a: &MarkingClassification,
    b: &MarkingClassification,
) -> Option<MarkingClassification> {
    use std::collections::BTreeSet;
    // Idempotency short-circuit: if a == b, return a unchanged so
    // input order is preserved through the round-trip.
    if a == b {
        return Some(a.clone());
    }
    match (a, b) {
        (MarkingClassification::Us(_), MarkingClassification::Us(_)) => Some(a.clone()),
        (MarkingClassification::Fgi(fa), MarkingClassification::Fgi(fb)) => {
            // P-9-1 (9th-pass): source-concealed (empty countries) is TOP in the
            // FGI source-disclosure dimension.  Meet with top returns the other
            // operand; dual of the join's concealed-dominates rule (P-1, 8th-pass).
            // Authority: §H.7 p124 (precedence rule for banner-line guidance —
            // "If any document contains portions of both source-concealed FGI
            // ... then only the 'FGI' marking without the source
            // trigraph(s)/tetragraph(s) must appear in the banner line") +
            // §H.7 p128 (worked-example restatement: concealed is most
            // restrictive form). Verified 2026-05-22 against
            // crates/capco/docs/CAPCO-2016.md.
            let a_concealed = fa.countries.is_empty();
            let b_concealed = fb.countries.is_empty();
            match (a_concealed, b_concealed) {
                (true, true) => {
                    // Both concealed → top ⊓ top = top.
                    Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                        level: fa.level,
                        countries: Box::new([]),
                    }))
                }
                (true, false) => {
                    // self is concealed (top) → return other.
                    Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                        level: fb.level,
                        countries: fb.countries.clone(),
                    }))
                }
                (false, true) => {
                    // other is concealed (top) → return self.
                    Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                        level: fa.level,
                        countries: fa.countries.clone(),
                    }))
                }
                (false, false) => {
                    let sa: BTreeSet<marque_ism::CountryCode> =
                        fa.countries.iter().copied().collect();
                    let sb: BTreeSet<marque_ism::CountryCode> =
                        fb.countries.iter().copied().collect();
                    let inter: BTreeSet<marque_ism::CountryCode> =
                        sa.intersection(&sb).copied().collect();
                    if inter.is_empty() {
                        None
                    } else {
                        Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                            level: fa.level,
                            countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                        }))
                    }
                }
            }
        }
        (MarkingClassification::Nato(_), MarkingClassification::Nato(_)) => Some(a.clone()),
        (MarkingClassification::Joint(ja), MarkingClassification::Joint(jb)) => {
            let sa: BTreeSet<marque_ism::CountryCode> = ja.countries.iter().copied().collect();
            let sb: BTreeSet<marque_ism::CountryCode> = jb.countries.iter().copied().collect();
            let inter: BTreeSet<marque_ism::CountryCode> = sa.intersection(&sb).copied().collect();
            if inter.is_empty() {
                None
            } else {
                Some(MarkingClassification::Joint(
                    marque_ism::JointClassification {
                        level: ja.level,
                        countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                    },
                ))
            }
        }
        (
            MarkingClassification::Conflict {
                us: ua,
                foreign: fa,
            },
            MarkingClassification::Conflict {
                us: ub,
                foreign: fb,
            },
        ) => {
            // us level matches by invariant. Conflict carries an
            // implicit US + a single foreign payload; meet is the
            // foreign-intersection lifted back into Conflict, or
            // None if the foreign payloads are incomparable.
            let _ = (ua, ub);
            meet_foreign_classification(fa, fb).map(|foreign| MarkingClassification::Conflict {
                us: *ua,
                foreign: Box::new(foreign),
            })
        }
        _ => None,
    }
}

/// Companion to [`merge_foreign_classification`] for the meet side.
/// Same-variant payloads intersect; cross-variant returns the
/// HIGHER-rank operand (the dominated, lower-≤ side; the GLB dual of
/// the join's "lower variant rank wins" tiebreak).
///
/// **C-9b (PR 4b-B 7th-pass follow-up).** Pre-fix, this function
/// returned `None` on cross-variant inputs while
/// `merge_foreign_classification` returned the lower-rank operand.
/// That asymmetry broke the dual absorption law `a ⊓ (a ⊔ b) = a` for
/// `Conflict` values whose inner foreign payloads had different
/// variants — the join would settle on the lower-rank inner, but the
/// meet would collapse the entire outer Conflict to bottom. C-9b
/// aligns the cross-variant meet with the join's tiebreak (return the
/// higher-rank operand, the GLB dual), mirroring how C-9 fixed the
/// same asymmetry at the outer `ClassificationLattice::meet` level.
///
/// §-authority: §H.7 pp123-125 reciprocal-normalization grounds the
/// variant-rank ordering (Fgi=1 < Nato=2 < Joint=3). Verified
/// 2026-05-15 against CAPCO-2016.md.
fn meet_foreign_classification(
    a: &marque_ism::ForeignClassification,
    b: &marque_ism::ForeignClassification,
) -> Option<marque_ism::ForeignClassification> {
    use marque_ism::ForeignClassification;
    use std::collections::BTreeSet;
    match (a, b) {
        (ForeignClassification::Fgi(fa), ForeignClassification::Fgi(fb)) => {
            // P-9-1 (9th-pass): source-concealed (empty countries) is TOP in
            // the FGI source-disclosure dimension — dual of the join's
            // concealed-dominates rule (P-1, 8th-pass). Meet(top, x) = x.
            // Authority: §H.7 p124 (precedence rule for banner-line guidance —
            // mixed source-concealed + source-acknowledged FGI collapses to
            // the bare "FGI" form) + §H.7 p128 (worked-example restatement:
            // concealed is most restrictive form). Verified 2026-05-22
            // against crates/capco/docs/CAPCO-2016.md.
            let a_concealed = fa.countries.is_empty();
            let b_concealed = fb.countries.is_empty();
            match (a_concealed, b_concealed) {
                (true, true) => Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                    level: fa.level,
                    countries: Box::new([]),
                })),
                (true, false) => Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                    level: fb.level,
                    countries: fb.countries.clone(),
                })),
                (false, true) => Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                    level: fa.level,
                    countries: fa.countries.clone(),
                })),
                (false, false) => {
                    let sa: BTreeSet<marque_ism::CountryCode> =
                        fa.countries.iter().copied().collect();
                    let sb: BTreeSet<marque_ism::CountryCode> =
                        fb.countries.iter().copied().collect();
                    let inter: BTreeSet<marque_ism::CountryCode> =
                        sa.intersection(&sb).copied().collect();
                    if inter.is_empty() {
                        None
                    } else {
                        Some(ForeignClassification::Fgi(marque_ism::FgiClassification {
                            level: fa.level,
                            countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                        }))
                    }
                }
            }
        }
        (ForeignClassification::Nato(_), ForeignClassification::Nato(_)) => Some(a.clone()),
        (ForeignClassification::Joint(ja), ForeignClassification::Joint(jb)) => {
            let sa: BTreeSet<marque_ism::CountryCode> = ja.countries.iter().copied().collect();
            let sb: BTreeSet<marque_ism::CountryCode> = jb.countries.iter().copied().collect();
            let inter: BTreeSet<marque_ism::CountryCode> = sa.intersection(&sb).copied().collect();
            if inter.is_empty() {
                None
            } else {
                Some(ForeignClassification::Joint(
                    marque_ism::JointClassification {
                        level: ja.level,
                        countries: inter.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                    },
                ))
            }
        }
        // C-9b: cross-variant → return the HIGHER-rank operand (the
        // dominated, lower-≤ side; GLB dual of `merge_foreign_classification`'s
        // tiebreak). The rank function below MUST agree with the one
        // in `merge_foreign_classification` (Fgi=1 < Nato=2 < Joint=3).
        _ => {
            let rank = |fc: &ForeignClassification| -> u8 {
                match fc {
                    ForeignClassification::Fgi(_) => 1,
                    ForeignClassification::Nato(_) => 2,
                    ForeignClassification::Joint(_) => 3,
                }
            };
            // Dual of merge: merge returns the LOWER-rank operand
            // (the GREATER element under ≤); meet returns the
            // HIGHER-rank operand (the LESSER element under ≤).
            if rank(a) >= rank(b) {
                Some(a.clone())
            } else {
                Some(b.clone())
            }
        }
    }
}

impl JoinSemilattice for ClassificationLattice {
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, x) | (x, None) => Self(x.clone()),
            (Some(a), Some(b)) => {
                let la = a.effective_level();
                let lb = b.effective_level();
                if la > lb {
                    Self(Some(a.clone()))
                } else if lb > la {
                    Self(Some(b.clone()))
                } else {
                    // Equal effective level: deterministic variant
                    // tiebreak. Lower rank wins, so the join is
                    // commutative (a.join(b) == b.join(a)).
                    //
                    // C-7 (PR 4b-B follow-up): when both operands
                    // share the same variant AND the same level, the
                    // payloads may still differ — e.g.
                    // `Fgi(S, [GBR]).join(Fgi(S, [CAN]))`. The
                    // variant-rank tiebreak alone fell through
                    // `ra <= rb` returning the left operand, which
                    // broke commutativity on same-variant payload
                    // diffs. We union the country payloads per the
                    // §H.7 p123 / §D.2 p28 banner-rollup rule that
                    // the banner FGI list is the union of every
                    // observed foreign source.
                    let ra = classification_variant_rank(a);
                    let rb = classification_variant_rank(b);
                    if ra == rb {
                        Self(Some(classification_join_same_variant(a, b)))
                    } else if ra < rb {
                        Self(Some(a.clone()))
                    } else {
                        Self(Some(b.clone()))
                    }
                }
            }
        }
    }
}

impl MeetSemilattice for ClassificationLattice {
    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => {
                let la = a.effective_level();
                let lb = b.effective_level();
                if la < lb {
                    Self(Some(a.clone()))
                } else if lb < la {
                    Self(Some(b.clone()))
                } else {
                    // Equal effective level: meet must be the GLB
                    // dual of `join`. The join policy is:
                    //   - lower variant-rank wins at same level
                    //     (Us < Fgi < Nato < Joint < Conflict),
                    //     so the lower-rank variant is the GREATER
                    //     element in the lattice ≤ order;
                    //   - same variant + same level, payloads union.
                    //
                    // GLB (meet) is therefore the dual:
                    //   - cross-variant: return the HIGHER variant-
                    //     rank operand (the dominated, lower-≤ side).
                    //     §H.7 pp123-125 reciprocal-normalization.
                    //   - same variant + same level, payloads
                    //     INTERSECT (country-list GLB). Empty
                    //     intersection drops to the lattice bottom.
                    //
                    // C-9 (PR 4b-B follow-up): pre-fix, meet mirrored
                    // join's tiebreaker (lower rank wins) AND used
                    // the UNION helper for same-variant payloads.
                    // Both branches broke the absorption laws
                    // `a ⊔ (a ⊓ b) = a` / `a ⊓ (a ⊔ b) = a`.
                    let ra = classification_variant_rank(a);
                    let rb = classification_variant_rank(b);
                    if ra == rb {
                        match classification_meet_same_variant(a, b) {
                            Some(m) => Self(Some(m)),
                            None => Self(None),
                        }
                    } else if ra < rb {
                        // a has lower rank → a is GREATER in ≤ →
                        // b is the meet (the dominated, lower-≤).
                        Self(Some(b.clone()))
                    } else {
                        // a has higher rank → a is LESSER in ≤ →
                        // a is the meet.
                        Self(Some(a.clone()))
                    }
                }
            }
        }
    }
}

impl BoundedJoinSemilattice for ClassificationLattice {
    fn bottom() -> Self {
        Self(None)
    }
}

impl BoundedMeetSemilattice for ClassificationLattice {
    fn top() -> Self {
        Self(Some(MarkingClassification::Us(Classification::TopSecret)))
    }
}
