// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`AeaSet`] — lattice over the AEA category (RD/FRD/TFNI + CNWDI + SIGMA +
//! UCNI + ATOMAL), with its sub-axis types [`AeaPrimary`] and [`UcniKind`].

use marque_ism::{AeaMarking, AtomalBlock, FrdBlock, RdBlock};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
use smallvec::SmallVec;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// AeaSet — lattice over the AEA category (RD/FRD/TFNI + CNWDI + SIGMA +
// UCNI + ATOMAL)
// ---------------------------------------------------------------------------

/// Primary AEA axis: a total-order supersession chain over the three
/// "primary" AEA markings — TFNI ⊏ FRD ⊏ RD per CAPCO-2016 §H.6 p104
/// + §H.6 p111 + §H.6 p120.
///
/// Variants are declared in **ascending supersession order**, which
/// makes the derived `Ord` impl match the supersession order without
/// a hand-written `cmp`. The `Lattice` impl picks `max(a, b)` as the
/// join — `Rd ⊐ Frd ⊐ Tfni` under that order.
///
/// §-authority (three subsections state the same rule from each
/// marking's vantage):
/// - §H.6 p104 (RD Precedence Rules): "If RD, FRD, and TFNI portions
///   are in a document, the RD takes precedence and is conveyed in
///   the banner line."
/// - §H.6 p111 (FRD Precedence Rules): "If RD and FRD portions are in
///   a document, the RD marking takes precedence in the banner line."
/// - §H.6 p120 (TFNI Precedence Rules): "If the TFNI marking is
///   contained in any portion of a document that contains portions
///   of RD and/or FRD, the RD or FRD takes precedence."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AeaPrimary {
    /// Transclassified Foreign Nuclear Information.
    Tfni,
    /// Formerly Restricted Data.
    Frd,
    /// Restricted Data — top of the AEA supersession chain.
    Rd,
}

/// UCNI variant: DoD or DoE.
///
/// `Ord` derivation places `DodUcni` first (Rust derives `Ord` from
/// variant declaration order, not alphabetical; happens to match
/// alphabetical here because we declared `DodUcni` then `DoeUcni`).
/// §G.1 Table 4 cat-6 order has DOD before DOE, which matches.
///
/// §-authority:
/// - §H.6 p116-117 (DOD UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION
///   / portion mark `DCNI`).
/// - §H.6 p118-119 (DOE UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION
///   / portion mark `UCNI`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UcniKind {
    /// DOD UCNI — DoD Special Nuclear Material protection per §H.6 p116.
    DodUcni,
    /// DOE UCNI — DoE post-RD-declassification controls per §H.6 p118.
    DoeUcni,
}

/// Lattice form of the full AEA category state on a page.
///
/// `AeaSet` is the lattice-form counterpart of the
/// [`marque_ism::AeaMarking`] sequence in `CanonicalAttrs.aea_markings`.
/// It composes five algebraically-distinct sub-axes as a `Product`:
///
/// 1. **Primary** ([`AeaPrimary`]): total-order supersession
///    `Tfni ⊏ Frd ⊏ Rd` per §H.6 p104 + p111 + p120.
/// 2. **CNWDI** (`bool`): presence flag, OR-monotone per §H.6 p106.
/// 3. **SIGMA** (`BTreeSet<u8>`): flat set union of SIGMA program
///    numbers per §H.6 p108 (currently {14, 15, 18, 20}; open-vocab
///    per the prose "SIGMA # currently represents one or more of
///    the following numbers").
/// 4. **UCNI** (`BTreeSet<UcniKind>`): flat set union of DOD / DOE
///    UCNI presence per §H.6 p116-117 + p118-119.
/// 5. **ATOMAL** (`Option<AtomalBlock>`): optional-singleton presence
///    per §G.2 Table 5 p40 (ATOMAL registered as a standalone control
///    marking) + §H.7 p122 worked example (`SECRET//RD/ATOMAL//FGI
///    NATO//NOFORN` places ATOMAL in the AEA category position
///    alongside RD — confirming AEA-axis routing). Note §H.7 is the
///    FGI section, not an ATOMAL subsection; ATOMAL has no dedicated
///    subsection in §H.1 through §H.9, its registration lives in
///    §G.2 Table 5 and its AEA-axis routing is established by the
///    §H.7 p122 worked example, not by Table 5 itself. The PR 9c.1
///    T134 routing decision tracked this through the parser layer.
///
///    **CV-2 (PR 4b-B 8th-pass follow-up).** Pre-CV-2 wording said
///    `§G.2 Table 5 p40 (ATOMAL registered as a standalone control
///    marking; ARH = AEA)`. Verified 2026-05-16 against
///    `crates/capco/docs/CAPCO-2016.md`: Table 5 places ATOMAL under
///    its own row (no group header in the markdown rendering between
///    the NATO classification rows and the BOHEMIA/BALK rows), with
///    the ARH column reading "Requires ATOMAL read-in" — it does NOT
///    say "ARH = AEA". The "AEA category position" routing claim
///    derives from the §H.7 p122 worked example placement, not from
///    Table 5. The "ARH = AEA" parenthetical was a Constitution VIII
///    misattribution; the corrected citation pair (§G.2 Table 5 p40
///    for registration + §H.7 p122 worked example for AEA-axis
///    placement) preserves the routing-decision rationale without
///    over-claiming what Table 5 says.
///
/// `AeaSet` round-trips with `&[AeaMarking]` via
/// [`AeaSet::from_markings`] / [`AeaSet::to_markings`], mirroring
/// the existing `SciSet::from_markings` / `SciSet::to_markings`
/// pattern.
///
/// # `BoundedLattice` deliberately not implemented
///
/// Per the `SciSet` / `SarSet` precedent in this module, AeaSet's
/// SIGMA axis is **open-vocabulary** per §H.6 p108 ("currently
/// represents one or more of the following numbers" — i.e., future
/// CAPCO revisions may add SIGMAs). No lawful finite top exists for
/// the Product as a whole. Callers needing the bottom use
/// [`AeaSet::default`] or [`AeaSet::empty`].
///
/// # Cross-axis invariants (validated by `CapcoScheme`, not the lattice)
///
/// - **CNWDI requires RD** (§H.6 p106): the lattice admits the
///   syntactically-reachable state `cnwdi=true, primary=None`, which
///   the `Constraint::Requires` row `E067/cnwdi-requires-rd` on
///   `CapcoScheme::build_constraints()` catches at validation time.
/// - **CNWDI requires class ≥ S** (§H.6 p106): covered by
///   `E058/CNWDI-classification-floor` in the class-floor catalog
///   (PR 3b.D T026d). Not duplicated here.
/// - **UCNI strip on classified** (§H.6 p116-117 + p118-119): a
///   post-projection cross-axis rewrite suppresses UCNI from the
///   banner and adds NOFORN when banner classification > U. The
///   algebraic shape mirrors the §3 (b) FOUO eviction matrix;
///   PR 4b-C wires the catalog row.
/// - **SIGMA cross-modifier coalescing** (§H.6 p108-109 + p113):
///   handled by the existing `capco/frd-sigma-consolidates-into-rd-sigma`
///   PageRewrite. PR 4b-B wires the runtime `AeaSet`-driven mutation
///   to replace the current `never_fires` / `noop_action` stub.
///
/// See `docs/plans/2026-05-01-lattice-design.md` §7.5 for the
/// formal join semantics, four worked examples, and acceptance
/// attestation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AeaSet {
    /// Primary axis: `Option` because not every page carries an
    /// RD/FRD/TFNI portion (a page with only UCNI portions joins to
    /// `primary: None`).
    primary: Option<AeaPrimary>,
    /// CNWDI presence (only meaningful when `primary == Some(Rd)`;
    /// see the cross-axis invariant note above).
    cnwdi: bool,
    /// SIGMA program numbers per §H.6 p108. Sorted ascending by the
    /// `BTreeSet`'s natural order so banner rendering ("Multiple
    /// SIGMA numbers must be listed in numerical order") is a
    /// no-extra-work iteration.
    sigmas: BTreeSet<u8>,
    /// UCNI variants. The two-element vocabulary makes this a
    /// bounded flat-set in isolation; included in the open-vocab
    /// `AeaSet` Product, it stays a flat-set without contributing
    /// boundedness.
    ucni: BTreeSet<UcniKind>,
    /// ATOMAL presence. `AtomalBlock` is currently empty per
    /// §G.2 Table 5 p40 (ATOMAL is a registered standalone control
    /// marking with no enumerated sub-markings — Table 5 lives in
    /// §G.2, the ARH subsection, not §G.1); the carrier struct
    /// mirrors `RdBlock` / `FrdBlock` so a future CAPCO grammar
    /// extension remains a planned migration.
    atomal: Option<AtomalBlock>,
}

impl AeaSet {
    /// An empty AEA set — the lattice bottom.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct an `AeaSet` from a slice of `AeaMarking`.
    ///
    /// Each variant of `AeaMarking` decomposes into one or more
    /// sub-axes:
    /// - `Rd(RdBlock { cnwdi, sigma })` → axis 1 = `Rd`,
    ///   axis 2 = `cnwdi`, axis 3 ⊇ `sigma`.
    /// - `Frd(FrdBlock { sigma })` → axis 1 = max(current, `Frd`),
    ///   axis 3 ⊇ `sigma`.
    /// - `Tfni` → axis 1 = max(current, `Tfni`).
    /// - `DodUcni` → axis 4 ⊇ `{DodUcni}`.
    /// - `DoeUcni` → axis 4 ⊇ `{DoeUcni}`.
    /// - `Atomal(_)` → axis 5 = `Some(AtomalBlock)`.
    ///
    /// Duplicate atoms within `markings` collapse via the per-axis
    /// joins (idempotent in every axis).
    pub fn from_markings(markings: &[AeaMarking]) -> Self {
        Self::from_markings_iter(markings.iter())
    }

    /// Construct an `AeaSet` from an iterator over `AeaMarking` references.
    ///
    /// Prefer this over [`Self::from_markings`] when the caller already has an
    /// iterator, such as a flattened per-portion slice — avoids the intermediate
    /// `Vec<AeaMarking>` allocation. CLONE-1 performance fix (issue #606).
    pub fn from_markings_iter<'a>(markings: impl Iterator<Item = &'a AeaMarking>) -> Self {
        let mut out = Self::empty();
        for m in markings {
            match m {
                AeaMarking::Rd(rd) => {
                    out.primary = Some(match out.primary {
                        Some(prev) => prev.max(AeaPrimary::Rd),
                        None => AeaPrimary::Rd,
                    });
                    out.cnwdi = out.cnwdi || rd.cnwdi;
                    out.sigmas.extend(rd.sigma.iter().copied());
                }
                AeaMarking::Frd(frd) => {
                    out.primary = Some(match out.primary {
                        Some(prev) => prev.max(AeaPrimary::Frd),
                        None => AeaPrimary::Frd,
                    });
                    out.sigmas.extend(frd.sigma.iter().copied());
                }
                AeaMarking::Tfni => {
                    out.primary = Some(match out.primary {
                        Some(prev) => prev.max(AeaPrimary::Tfni),
                        None => AeaPrimary::Tfni,
                    });
                }
                AeaMarking::DodUcni => {
                    out.ucni.insert(UcniKind::DodUcni);
                }
                AeaMarking::DoeUcni => {
                    out.ucni.insert(UcniKind::DoeUcni);
                }
                AeaMarking::Atomal(block) => {
                    out.atomal = Some(*block);
                }
                // `AeaMarking` is `#[non_exhaustive]`. A future variant
                // (e.g., a hypothetical CAPCO grammar extension that adds
                // a new AEA marking) lands here as a silent no-op so the
                // existing AEA-lattice builds continue to compile while
                // surfacing the gap explicitly in code review. The fix
                // is to add a sub-axis to `AeaSet` (or extend an
                // existing one) and ship it as a separate atomic PR.
                _ => {}
            }
        }
        out
    }

    /// Render this set back to a boxed slice of `AeaMarking`. The
    /// per-portion emission order is:
    /// `primary → DOD UCNI → DOE UCNI → ATOMAL`
    /// where the primary arm emits one of RD/FRD/TFNI (supersession
    /// guarantees at most one survives the lattice join — so on a
    /// post-join `AeaSet` this is at most one atom, never the
    /// three-atom sequence the §G.1 Table 4 cat-6 register would
    /// suggest). The full §G.1 Table 4 cat-6 register order
    /// (`RD → CNWDI → SIGMA → FRD → SIGMA → DOD UCNI → DOE UCNI →
    /// TFNI → ATOMAL`) is the spec for a per-document banner; the
    /// post-join lattice already collapses to a single primary,
    /// making the emission order above isomorphic to the register
    /// order in every realizable case. The §G.1 Table 4 cat-2
    /// position of ATOMAL (`Non-US Protective Markings`) governs
    /// inter-category placement; this method emits only the within-
    /// category atoms.
    ///
    /// CNWDI rides on the RD block; SIGMA numbers ride on the RD
    /// block (per §H.6 p108-109 cross-modifier coalescing — when
    /// `primary == Rd` any SIGMA numbers from RD or FRD portions
    /// emit under RD-SIGMA in the banner). When `primary == Frd`,
    /// SIGMA numbers ride on the FRD block. When `primary == Tfni`,
    /// SIGMA numbers are dropped (TFNI has no SIGMA modifier per
    /// §H.6 p120).
    ///
    /// `AeaMarking::DodUcni` / `DoeUcni` are emitted regardless of
    /// classification; the §H.6 p116-117 / p118-119 "does not appear
    /// in the banner line on classified docs" rule is a
    /// post-projection rewrite (see the cross-axis invariant note
    /// on [`AeaSet`]), not a lattice render-time strip.
    pub fn to_markings(&self) -> Box<[AeaMarking]> {
        // LA-2 empty-axis fast-path: skip SmallVec / sigmas-box
        // construction when no AEA markings were accumulated (the
        // common case on documents with no RD/FRD/TFNI/UCNI/ATOMAL
        // portions).
        if self.is_empty() {
            return Box::default();
        }
        // Inline-5 covers all AEA variants (Rd/Frd, DodUcni, DoeUcni,
        // Tfni, Atomal); the output stays heap-free for typical
        // documents (LA-4).
        let mut out: SmallVec<[AeaMarking; 5]> = SmallVec::with_capacity(5);
        // Sort SIGMA numbers ascending for §H.6 p108 canonical form.
        // `BTreeSet` already iterates in sorted order. Inline-8 covers
        // the observed SIGMA range (1–99; in practice 1–5); (LA-4).
        let sigmas: Box<[u8]> = self
            .sigmas
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .into_boxed_slice();

        // Emission order matches the §G.1 Table 4 cat-6 register:
        // `RD → FRD → DOD UCNI → DOE UCNI → TFNI → ATOMAL`. The
        // primary axis collapses to at most one of {RD, FRD, TFNI}
        // under supersession, and TFNI emits AFTER the UCNI atoms
        // per Table 4's register-order — not in the same arm as RD
        // and FRD. SIGMA rides on whichever of RD or FRD survives
        // per the §H.6 p108-109 cross-modifier coalescing rule;
        // under Tfni-primary the SIGMA set is silently dropped
        // because §H.6 p120 has no SIGMA modifier and the inputs
        // that produced it came from RD or FRD portions that got
        // superseded.

        // Step 1: RD or FRD (if either is the primary).
        match self.primary {
            Some(AeaPrimary::Rd) => {
                out.push(AeaMarking::Rd(RdBlock {
                    cnwdi: self.cnwdi,
                    sigma: sigmas,
                }));
            }
            Some(AeaPrimary::Frd) => {
                // CNWDI is RD-only per §H.6 p106 — the marque-ism
                // type system already enforces this (CNWDI is a
                // `bool` field on `RdBlock`, not on `FrdBlock`), so
                // a `cnwdi=true, primary=Frd` state cannot arise
                // from valid parser output. The render here drops
                // cnwdi silently as a defensive measure against
                // lattice-internal-only constructions.
                out.push(AeaMarking::Frd(FrdBlock { sigma: sigmas }));
            }
            Some(AeaPrimary::Tfni) | None => {
                // TFNI emission is deferred to Step 3 (post-UCNI)
                // to honor the §G.1 Table 4 register order.
                // None — no primary on the page; CNWDI / SIGMA
                // alone are not renderable without a primary
                // anchor.
            }
        }
        // Step 2: UCNI variants per §G.1 Table 4 register order.
        if self.ucni.contains(&UcniKind::DodUcni) {
            out.push(AeaMarking::DodUcni);
        }
        if self.ucni.contains(&UcniKind::DoeUcni) {
            out.push(AeaMarking::DoeUcni);
        }
        // Step 3: TFNI (if primary; emits AFTER UCNI per Table 4).
        if matches!(self.primary, Some(AeaPrimary::Tfni)) {
            out.push(AeaMarking::Tfni);
        }
        // Step 4: ATOMAL.
        if let Some(block) = self.atomal {
            out.push(AeaMarking::Atomal(block));
        }
        out.into_boxed_slice()
    }

    /// Whether the set is empty (all five sub-axes at bottom).
    pub fn is_empty(&self) -> bool {
        self.primary.is_none()
            && !self.cnwdi
            && self.sigmas.is_empty()
            && self.ucni.is_empty()
            && self.atomal.is_none()
    }

    /// Read access to the primary axis. Exposed for cross-axis
    /// rewrite predicates (e.g., a future PR's UCNI-strip-on-
    /// classified that needs to inspect whether an RD/FRD/TFNI
    /// primary exists).
    pub fn primary(&self) -> Option<AeaPrimary> {
        self.primary
    }

    /// Read access to the CNWDI presence flag. Exposed for the
    /// `E067/cnwdi-requires-rd` constraint and analogous cross-axis
    /// validation.
    pub fn cnwdi(&self) -> bool {
        self.cnwdi
    }

    /// Read access to the SIGMA program-number set.
    pub fn sigmas(&self) -> &BTreeSet<u8> {
        &self.sigmas
    }

    /// Read access to the UCNI variant set.
    pub fn ucni(&self) -> &BTreeSet<UcniKind> {
        &self.ucni
    }

    /// Read access to the ATOMAL presence.
    pub fn atomal(&self) -> Option<AtomalBlock> {
        self.atomal
    }
}

impl JoinSemilattice for AeaSet {
    /// `docs/plans/2026-05-01-lattice-design.md` §7.5.
    fn join(&self, other: &Self) -> Self {
        Self {
            // Axis 1: SupersessionSet — max under Tfni ⊏ Frd ⊏ Rd.
            primary: match (self.primary, other.primary) {
                (None, x) | (x, None) => x,
                (Some(a), Some(b)) => Some(a.max(b)),
            },
            // Axis 2: OR-monotone.
            cnwdi: self.cnwdi || other.cnwdi,
            // Axis 3: flat-set union.
            sigmas: {
                let mut out = self.sigmas.clone();
                out.extend(other.sigmas.iter().copied());
                out
            },
            // Axis 4: flat-set union.
            ucni: {
                let mut out = self.ucni.clone();
                out.extend(other.ucni.iter().copied());
                out
            },
            // Axis 5: OptionalSingleton — `or` (presence-OR).
            atomal: self.atomal.or(other.atomal),
        }
    }
}

impl MeetSemilattice for AeaSet {
    /// Componentwise meet across the five Product sub-axes.
    ///
    /// Meet is included for trait-completeness; CAPCO's banner
    /// roll-up does not use it directly (banner = join over all
    /// portions on the page). The meet semantics:
    ///
    /// - Axis 1: `min` under `Tfni ⊏ Frd ⊏ Rd` (with `None` as
    ///   bottom and as the meet-identity-for-Some).
    /// - Axis 2: AND.
    /// - Axis 3, 4: set-intersection.
    /// - Axis 5: `and` (both sides must carry ATOMAL).
    fn meet(&self, other: &Self) -> Self {
        Self {
            primary: match (self.primary, other.primary) {
                (None, _) | (_, None) => None,
                (Some(a), Some(b)) => Some(a.min(b)),
            },
            cnwdi: self.cnwdi && other.cnwdi,
            sigmas: self.sigmas.intersection(&other.sigmas).copied().collect(),
            ucni: self.ucni.intersection(&other.ucni).copied().collect(),
            atomal: match (self.atomal, other.atomal) {
                (Some(a), Some(_)) => Some(a),
                _ => None,
            },
        }
    }
}

// `AeaSet` intentionally does **not** implement `BoundedLattice`:
// axis 3 (SIGMA numbers) is open-vocabulary per CAPCO-2016 §H.6 p108
// ("SIGMA # currently represents one or more of the following
// numbers" — future CAPCO revisions may add new numbers). An "empty"
// top would violate the `BoundedLattice::top ⊔ a = top` contract on
// any input carrying a SIGMA number outside the assumed top's set.
// Use [`AeaSet::empty`] / [`AeaSet::default`] when you need the
// bottom, and [`Lattice::join`] / [`Lattice::meet`] for composition.
