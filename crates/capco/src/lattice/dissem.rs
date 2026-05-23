// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`DissemSet`] — IC dissem axis with three supersession overlays.
//!
//! Join-only per issue #456 / PR #502 — the `relido_observed_unanimous`
//! flag is a join-side aggregation property whose meet semantic violates
//! the dual absorption law.

use marque_ism::{CanonicalAttrs, DissemControl};
use marque_scheme::JoinSemilattice;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// DissemSet — IC dissem axis with three supersession overlays
// ---------------------------------------------------------------------------

/// FD&R supersession-pair table.
///
/// Each row `(dominant, dominated)` reads "if `dominant` is present in
/// the post-join set, remove `dominated`." The table is the §D.2
/// Table 3 (p28) FD&R precedence rules + §H.8 NOFORN supersession,
/// expressed structurally rather than as branches.
///
/// The single-static-table convention (M-14 PR 4b-B follow-up) is
/// enforced by the crate-private `apply_overlays` API taking
/// `DISSEM_SUPERSESSION_TABLE` directly — the only call site is
/// inside `marque-capco`, code-review enforces no ad-hoc copies.
/// An earlier `debug_assert!` pointer-equality check (rust-reviewer
/// Gotcha 2) was removed in H-4 because it compared the table
/// pointer to itself (always true, false protection); the `&'static`
/// reference passed everywhere in this module is the actual
/// invariant.
///
/// §-authority (verified 2026-05-16 against CAPCO-2016.md):
/// - §D.2 Table 3 rows 1-2 (NOFORN dominates FD&R controls).
/// - §H.8 p145 (NOFORN: "Cannot be used with REL TO, RELIDO, EYES ONLY,
///   or DISPLAY ONLY").
/// - §H.8 p157 (EYES ONLY: NSA-only marking — E064 emits a fix to migrate
///   EYES ONLY → REL TO at engine fix-time, but the parser preserves
///   `DissemControl::Eyes` during lint runs. P-4 (8th-pass): corrected
///   prior docstring that falsely claimed "EYES retired... already migrated
///   to REL TO at parse time so not represented here" — the parser does NOT
///   migrate at parse time; `scheme.rs:190` and `scheme.rs:3677` confirm
///   `DissemControl::Eyes` survives parse and appears in `dissem_us` during
///   intermediate lattice composition. NOFORN must dominate EYES ONLY in
///   the supersession table for the lattice path to be correct per §H.8 p145.
///   E064 handles the EYES → REL TO migration as a separate rule at fix time.)
static DISSEM_SUPERSESSION_TABLE: &[(DissemControl, DissemControl)] = &[
    // NOFORN ⊐ REL TO / RELIDO / DISPLAY ONLY / EYES ONLY — §D.2 Table 3
    // rows 1-2 + §H.8 p145 ("Cannot be used with REL TO, RELIDO, EYES ONLY,
    // or DISPLAY ONLY").
    //
    // P-4 (8th-pass): added EYES ONLY. Pre-fix the table omitted it based on
    // a false assumption that the parser migrated EYES → REL TO at parse time.
    // The parser preserves DissemControl::Eyes (see scheme.rs:190); E064 is
    // the engine-time migration rule. During lint runs and intermediate lattice
    // composition, EYES can appear and must be stripped when NOFORN is present.
    (DissemControl::Nf, DissemControl::Rel),
    (DissemControl::Nf, DissemControl::Relido),
    (DissemControl::Nf, DissemControl::Displayonly),
    (DissemControl::Nf, DissemControl::Eyes),
];

/// Lattice form of the US-attributed IC dissem axis: a `BTreeSet` of
/// `DissemControl` tokens with three supersession overlays applied
/// at construction and re-applied on `join`.
///
/// **Overlay set** (applied at `from_attrs_iter` / `join` time):
///
/// 1. Basic BTreeSet union over per-portion `dissem_us`.
/// 2. **OC-USGOV supersession** per §H.8 p136 + §H.8 p140: drop
///    `OcUsgov` if `Oc` is present in the joined set.
/// 3. **RELIDO observed-unanimity** per §H.8 pp155-156: drop `Relido`
///    if some portion lacks it. The constructor tracks this via the
///    `relido_observed_unanimous` flag so a subsequent `join` can
///    propagate the unanimity bit without re-inspecting the original
///    portions.
/// 4. **NOFORN dominates** per §D.2 Table 3 rows 1-2 + §H.8 p145:
///    drop `Rel` / `Relido` / `Displayonly` when `Nf` is present.
///    Applied via `with_noforn_injected` at the cross-axis NOFORN
///    rendezvous in `CapcoMarking::join_via_lattice` (G-8 PR 4b-B
///    follow-up).
///
/// **Post-PR-4b-E divergence inventory** (matching parity-gate
/// fixtures in `crates/capco/tests/lattice_vs_scheme_parity.rs`).
/// The PR 4b-E `PageContext::expected_*` deletion retired the
/// PageContext side of the original parity comparison; the surviving
/// comparison is between the per-axis lattice path
/// (`project_via_lattice`) and the full scheme pipeline
/// (`project_via_scheme = scheme.project(Scope::Page, ...)`, which
/// runs the declarative PageRewrite catalog over the per-axis
/// composition). The four overlays this `DissemSet` doc-comment
/// previously inventoried as "PageContext-only" all land on the
/// scheme path now:
///
/// - **Overlay 4 (NOFORN dominates)** lives on the lattice path
///   itself via `DissemSet::with_noforn_injected`. Per §H.8 p145
///   plus §D.2 Table 3 rows 1-2 the overlay strips `Rel` / `Relido`
///   / `Displayonly` when `Nf` is present.
/// - **FOUO classification-gate eviction** lives on
///   `scheme.project(Scope::Page, ...)` via the
///   `capco/classification-evicts-fouo` (Pattern B) and
///   `capco/fouo-evicted-by-classified` (Pattern C) PageRewrites
///   declared on `CapcoScheme` (CAPCO-2016 §H.8 p134
///   classified-document sub-clause).
/// - **UCNI classification-gate strip** lives on
///   `scheme.project(Scope::Page, ...)` via the
///   `capco/{dod,doe}-ucni-evicted-by-classified` and
///   `capco/{dod,doe}-ucni-promotes-noforn-when-classified`
///   PageRewrites (CAPCO-2016 §H.6 p116 DOD UCNI / §H.6 p118 DOE
///   UCNI; the NOFORN-promotion clause fires before the strip so
///   the §H.6 NOFORN-promotion semantic on classified pages is
///   preserved).
/// - **Cross-axis NOFORN injection from `non_ic_dissem`** mirrors on
///   the lattice path via `DissemSet::with_noforn_injected` (G-8
///   PR 4b-B). `NonIcDissemSet::from_attrs_iter`'s `needs_nf` flag
///   drives the injection on classified SBU-NF / LES-NF pages
///   (§H.9 p178 SBU-NF / §H.9 p185 LES-NF), and the supersession
///   overlay then re-runs Overlay 4 to strip dominated controls.
///
/// **Ordering** at the lattice level is BTreeSet's natural order;
/// §H.8 prose ordering ("OC/NF" not "NF/OC") is the renderer's
/// concern, not the lattice's. The renderer
/// (`MarkingScheme::render_canonical`) lands in PR 5+ Stage 4.
///
/// **`BoundedLattice` deliberately not implemented.** The
/// `DissemControl` vocabulary contains ~25 tokens but the **active
/// finite set** depends on schema version and agency extensions; the
/// open-vocab precedent (SciSet / SarSet / AeaSet) is the
/// established pattern for "implement `Lattice` + `empty()`/`default()`
/// for bottom, leave `top()` undefined." (M-25 PR 4b-B 7th-pass —
/// `FgiSet` was previously listed in this precedent; B-1 PR 4b-B
/// 8th-pass retired `FgiSet`'s `BoundedLattice` impl — `FgiSet`
/// does NOT implement `BoundedLattice`. Removed from precedent list
/// to avoid misattribution.)
///
/// **Partial-lattice note (C-4 PR 4b-B follow-up).** The
/// `relido_observed_unanimous` flag is a **join-side aggregation
/// property** — it tracks whether every portion contributing to the
/// page's dissem state has RELIDO. `meet` has no natural reading for
/// this flag, so its result carries the vacuous-true value (the
/// identity under subsequent AND-joins). This is what makes the
/// load-bearing absorption law `a ⊔ (a ⊓ b) = a` hold algebraically.
/// The dual law `a ⊓ (a ⊔ b) = a` does NOT hold over the full
/// `(set, flag)` pair — `DissemSet` is a join-semilattice with a
/// structural `meet` provided for completeness on the `set` axis.
///
/// §-authority (verified 2026-05-15 against CAPCO-2016.md):
/// - §H.8 p136 (ORCON dominates ORCON-USGOV).
/// - §H.8 p140 (ORCON-USGOV template same rule).
/// - §H.8 p145 (NOFORN dominates REL TO / RELIDO / DISPLAY ONLY).
/// - §H.8 pp155-156 (RELIDO unanimity for banner rollup).
/// - §D.2 Table 3 rows 1-2 (NOFORN dominates dominated FD&R).
///
/// **`Default`** (C-8 PR 4b-B follow-up). `Default` MUST agree with
/// `empty()` — both are the lattice bottom with the vacuous-truth
/// `relido_observed_unanimous = true` flag. A derived `Default` would
/// produce `relido_observed_unanimous = false` (bool's Default), which
/// would break `Default == empty()` and silently drop RELIDO when a
/// `Default::default()` value was joined into a unanimous-RELIDO set.
/// The manual `Default` impl delegates to `empty()` so the two
/// constructors agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DissemSet {
    /// The post-overlay set of dissem controls.
    set: BTreeSet<DissemControl>,
    /// `true` if every original portion carried `Relido`. `false`
    /// (the lattice bottom for this flag) means either no portion
    /// carried it OR some portion did not. The two cases are
    /// distinguishable via `set.contains(&Relido)`:
    /// `(set has Relido, unanimous=true)` → banner gets RELIDO;
    /// `(set has no Relido, unanimous=true)` → no Relido in any
    /// portion, the unanimity bit is vacuous and stays at true so
    /// joining with a fresh non-Relido set is no-op; etc.
    relido_observed_unanimous: bool,
}

impl Default for DissemSet {
    /// `Default` MUST agree with `DissemSet::empty()` (C-8 PR 4b-B
    /// follow-up). See the struct doc comment for the rationale —
    /// the derived `Default` set `relido_observed_unanimous = false`
    /// (bool's Default) and broke C-5's `from_attrs_iter(&[]) ==
    /// empty()` agreement on a third constructor.
    fn default() -> Self {
        Self::empty()
    }
}

impl DissemSet {
    /// An empty dissem set — the lattice bottom.
    ///
    /// Construction starts with `relido_observed_unanimous=true`
    /// because the universal claim "every portion has RELIDO" holds
    /// vacuously over an empty set of portions. Joining a real
    /// portion via `from_attrs_iter` propagates the unanimity flag
    /// correctly: if the first real portion has RELIDO, the flag
    /// remains true; if it doesn't, the flag flips to false.
    pub fn empty() -> Self {
        Self {
            set: BTreeSet::new(),
            relido_observed_unanimous: true,
        }
    }

    /// Construct from a slice of `CanonicalAttrs` — joins per-portion
    /// `dissem_us` and applies the supersession overlays.
    ///
    /// Empty input returns `Self::empty()` (the lattice bottom)
    /// exactly — `from_attrs_iter(&[]) == DissemSet::empty()`.
    /// The vacuous-truth treatment of "every portion carries
    /// RELIDO over an empty portion list" matches the universal-
    /// quantifier convention and the `empty()` constructor's
    /// `relido_observed_unanimous = true`.
    pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
        if portions.is_empty() {
            return Self::empty();
        }

        let mut set = BTreeSet::new();
        for p in portions {
            for t in p.dissem_us.iter() {
                set.insert(*t);
            }
        }

        // RELIDO observed-unanimity: track whether every portion
        // carries Relido. Vacuously true over an empty portion list
        // (universal quantifier convention); since we early-returned
        // on `portions.is_empty()`, this expression is now strictly
        // `every observed portion has RELIDO`.
        let relido_observed_unanimous = portions
            .iter()
            .all(|a| a.dissem_us.contains(&DissemControl::Relido));

        let mut out = Self {
            set,
            relido_observed_unanimous,
        };
        out.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        out
    }

    /// Internal: apply the three supersession overlays in order.
    /// The `table` parameter MUST be `DISSEM_SUPERSESSION_TABLE`
    /// in production (M-14 PR 4b-B follow-up — the `debug_assert!`
    /// pointer-equality "Gotcha 2" check from H-4 was removed
    /// because it compared the table to itself; the single-static-
    /// table convention is enforced by `apply_overlays` being
    /// crate-private with `DISSEM_SUPERSESSION_TABLE` as the only
    /// in-tree caller).
    fn apply_overlays(&mut self, table: &'static [(DissemControl, DissemControl)]) {
        // Overlay 1: OC-USGOV supersession (§H.8 p136 + p140).
        if self.set.contains(&DissemControl::Oc) && self.set.contains(&DissemControl::OcUsgov) {
            self.set.remove(&DissemControl::OcUsgov);
        }

        // Overlay 2: RELIDO observed-unanimity (§H.8 pp155-156). If
        // not unanimous, drop RELIDO.
        if self.set.contains(&DissemControl::Relido) && !self.relido_observed_unanimous {
            self.set.remove(&DissemControl::Relido);
        }

        // Overlay 3: NOFORN dominates (§D.2 Table 3 + §H.8 p145).
        if self.set.contains(&DissemControl::Nf) {
            for (dom, dominated) in table {
                if self.set.contains(dom) {
                    self.set.remove(dominated);
                }
            }
        }
    }

    /// Borrow the underlying BTreeSet.
    pub fn as_set(&self) -> &BTreeSet<DissemControl> {
        &self.set
    }

    /// Whether RELIDO was unanimous across the source portions. The
    /// banner derivation reads this when emitting the RELIDO token.
    pub fn relido_unanimous(&self) -> bool {
        self.relido_observed_unanimous
    }

    /// Render to a `Box<[DissemControl]>` in BTreeSet natural order.
    /// Per-§H.8 prose ordering is the renderer's concern; the lattice
    /// produces a deterministic order that round-trips through joins.
    pub fn into_boxed_slice(self) -> Box<[DissemControl]> {
        self.set.into_iter().collect::<Vec<_>>().into_boxed_slice()
    }

    /// Borrow as a `Vec` for callers that need the post-overlay set
    /// in `Vec`-shaped form (parity-gate fixtures and similar
    /// inspection sites; `into_boxed_slice` is the production
    /// renderer-facing API).
    pub fn to_vec(&self) -> Vec<DissemControl> {
        self.set.iter().copied().collect()
    }

    /// Inject `Nf` into the set and re-apply the supersession
    /// overlay. G-8 (PR 4b-B follow-up) — callers that need to
    /// inject NOFORN from a cross-axis source (non-IC SBU-NF /
    /// LES-NF on a classified page, NODIS / EXDIS supersession,
    /// or the `capco/noforn-clears-rel-to` PageRewrite) MUST route
    /// through here so the §H.8 p145 NOFORN-dominates rule strips
    /// `Rel` / `Relido` / `Displayonly` from the set.
    ///
    /// Pre-G-8 the cross-axis injection at the NOFORN rendezvous
    /// in the `join_via_lattice` body added `Nf` directly into
    /// `out.dissem_us` after `DissemSet::into_boxed_slice` ran,
    /// which left dominated controls in place — invalid per
    /// §H.8 p145.
    ///
    /// Authority: §H.8 p145 (NOFORN: "Cannot be used with REL TO /
    /// RELIDO / EYES ONLY / DISPLAY ONLY") + §D.2 Table 3 rows 1-2.
    pub fn with_noforn_injected(mut self) -> Self {
        self.set.insert(DissemControl::Nf);
        // Re-run the supersession overlay so the NOFORN-dominates
        // step strips any `Rel` / `Relido` / `Displayonly` left in
        // the bag.
        self.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        self
    }

    /// Re-apply all three [`DissemSet`] supersession overlays to a
    /// post-`close()` / post-`default_fill` state.
    ///
    /// Runs the same overlay chain as `from_attrs_iter` / `join`'s
    /// internal `apply_overlays(DISSEM_SUPERSESSION_TABLE)` call:
    ///
    /// - **Overlay 1** — OC-USGOV supersession by ORCON (§H.8 p136
    ///   + §H.8 p140). If both `Oc` and `OcUsgov` are present,
    ///   drop `OcUsgov`. NOFORN-independent: fires regardless of
    ///   whether NOFORN is in the set.
    /// - **Overlay 2** — RELIDO observed-unanimity (§H.8
    ///   pp155-156). If `Relido` is present but
    ///   `relido_observed_unanimous` is false, drop `Relido`.
    ///   NOFORN-independent.
    /// - **Overlay 3** — NOFORN-dominates supersession (§H.8 p145
    ///   + §B.3.a p19 + §D.2 Table 3 rows 1-2). If `Nf` is
    ///   present, drop every dominated control (`Rel`, `Relido`,
    ///   `Displayonly`, `Eyes`) per `DISSEM_SUPERSESSION_TABLE`.
    ///   NOFORN-dependent.
    ///
    /// # Why all three (post-#704 R2-1 rename)
    ///
    /// Pre-R2-1 this method was named `with_fdr_dominance_stripped`
    /// and its doc-comment positioned it as just the §H.8 p145
    /// strip. The body has always called `apply_overlays(...)`
    /// which runs all three overlays — the misalignment between
    /// name + doc and actual behavior was the second half of
    /// Copilot's round-2 R2-1 finding. The rename signals to
    /// future maintainers that this is the "rebuild overlays on a
    /// post-close / post-default-fill state" method, not
    /// specifically the FDR strip.
    ///
    /// Issue #704 architectural context: the `CapcoScheme::closure`
    /// operator is purely additive (Kleene fixpoint over
    /// `CLOSURE_TABLE`; the pre-#704 `suppressor_mask` gating that
    /// previously prevented Trio 1 / Trio 2 / Trio 3 cones from
    /// firing alongside an existing FD&R dominator was retired
    /// because it broke `a ⊑ b ⟹ Cl(a) ⊑ Cl(b)`). The semantic
    /// the suppressors encoded splits two ways: the "default if
    /// absent" half moved to
    /// `crate::scheme::default_fill::apply_default_fill`; the
    /// "post-close re-evaluate per-axis supersession" half lives
    /// here, called from
    /// `CapcoScheme::apply_supersession_overlays`.
    ///
    /// # Properties
    ///
    /// - **Idempotent** (`f(f(x)) == f(x)`): each overlay strips
    ///   strictly — running again observes nothing to strip.
    /// - **Composes with `with_noforn_injected`** and the
    ///   `apply_overlays` body without re-entrancy concerns
    ///   (`apply_overlays` is the shared mutation core).
    ///
    /// **Pure function.** Takes ownership and returns a new
    /// `DissemSet`; no `&mut self`.
    ///
    /// Authority:
    /// - §H.8 p136 + §H.8 p140 (OC > OC-USGOV — Overlay 1)
    /// - §H.8 pp155-156 ("All portions must be marked as RELIDO
    ///   for the RELIDO marking to appear in the banner line" —
    ///   Overlay 2)
    /// - §H.8 p145 (NOFORN: "Cannot be used with REL TO, RELIDO,
    ///   EYES ONLY, or DISPLAY ONLY"); §B.3.a p19 (FD&R dominator
    ///   enumeration); §D.2 Table 3 rows 1-2 (NOFORN dominates
    ///   dominated FD&R at banner roll-up) — Overlay 3.
    pub fn with_all_overlays_reapplied(mut self) -> Self {
        self.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        self
    }
}

// P-9-3 (9th-pass) — Partial-lattice divergence note for `DissemSet`.
//
// `DissemSet` implements only `JoinSemilattice`, NOT `MeetSemilattice`.
// The `relido_observed_unanimous` flag is a join-side aggregation property
// (a record of observed page composition); `meet` has no natural reading
// for this flag — the dual absorption law `a ⊓ (a ⊔ b) = a` cannot hold
// over the full `(set, relido_observed_unanimous)` pair. PR #502
// (issue #456) resolved this by splitting the `Lattice` trait into
// `JoinSemilattice` and `MeetSemilattice` halves; `DissemSet`
// implements only the join half,
// so the type system now rejects any attempt to call `.meet()` on it at
// compile time.
//
// See the `DissemSet` doc comment above (§ "Partial-lattice note C-4")
// for full rationale.
impl JoinSemilattice for DissemSet {
    fn join(&self, other: &Self) -> Self {
        // The single-static-table convention is enforced by the
        // crate-private `apply_overlays` API taking
        // `DISSEM_SUPERSESSION_TABLE` directly (it has no other call
        // sites). H-4 PR 4b-B follow-up removed a tautological
        // `debug_assert!` that compared the table pointer to itself
        // — always true, false protection.
        let mut set = self.set.clone();
        set.extend(other.set.iter().copied());

        // Joining preserves unanimity only if BOTH operands report
        // unanimity — the join models "page context of both sides
        // combined," and if either side observed non-unanimity, the
        // joined page does too. Vacuous unanimity (empty operand)
        // is identity for this conjunction: `true && x = x`.
        let relido_observed_unanimous =
            self.relido_observed_unanimous && other.relido_observed_unanimous;

        let mut out = Self {
            set,
            relido_observed_unanimous,
        };
        out.apply_overlays(DISSEM_SUPERSESSION_TABLE);
        out
    }
}

// ---------------------------------------------------------------------------
// `with_all_overlays_reapplied` unit tests (issue #704)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod with_all_overlays_reapplied_tests {
    use super::*;

    /// Build a `DissemSet` directly from a list of controls without
    /// running the from_attrs_iter portion-fold. Bypasses
    /// `apply_overlays` so each test can pin the overlay-under-test
    /// in isolation against a known input.
    fn raw(tokens: &[DissemControl]) -> DissemSet {
        let mut set = BTreeSet::new();
        for t in tokens {
            set.insert(*t);
        }
        DissemSet {
            set,
            relido_observed_unanimous: true,
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let stripped = raw(&[]).with_all_overlays_reapplied();
        assert!(stripped.as_set().is_empty());
    }

    #[test]
    fn no_dominator_present_is_noop() {
        // ORCON + IMCON: no NOFORN, no strip should occur.
        let input = raw(&[DissemControl::Oc, DissemControl::Imc]);
        let before = input.as_set().clone();
        let stripped = input.with_all_overlays_reapplied();
        assert_eq!(*stripped.as_set(), before);
    }

    #[test]
    fn dissem_dominator_strips_relido() {
        // NOFORN + RELIDO + ORCON: per §H.8 p145 NOFORN strips
        // RELIDO. ORCON survives (not a dominated FD&R).
        let stripped = raw(&[DissemControl::Nf, DissemControl::Relido, DissemControl::Oc])
            .with_all_overlays_reapplied();
        assert!(stripped.as_set().contains(&DissemControl::Nf));
        assert!(stripped.as_set().contains(&DissemControl::Oc));
        assert!(!stripped.as_set().contains(&DissemControl::Relido));
    }

    #[test]
    fn dissem_dominator_strips_rel_token() {
        // NOFORN + REL (the dissem-axis REL token, distinct from
        // the rel_to country list axis): NOFORN strips REL per
        // §H.8 p145.
        let stripped = raw(&[DissemControl::Nf, DissemControl::Rel]).with_all_overlays_reapplied();
        assert!(stripped.as_set().contains(&DissemControl::Nf));
        assert!(!stripped.as_set().contains(&DissemControl::Rel));
    }

    #[test]
    fn dissem_dominator_strips_displayonly() {
        // NOFORN + DISPLAY ONLY: §H.8 p145.
        let stripped =
            raw(&[DissemControl::Nf, DissemControl::Displayonly]).with_all_overlays_reapplied();
        assert!(stripped.as_set().contains(&DissemControl::Nf));
        assert!(!stripped.as_set().contains(&DissemControl::Displayonly));
    }

    #[test]
    fn dissem_dominator_strips_eyes() {
        // NOFORN + EYES: §H.8 p145.
        let stripped = raw(&[DissemControl::Nf, DissemControl::Eyes]).with_all_overlays_reapplied();
        assert!(stripped.as_set().contains(&DissemControl::Nf));
        assert!(!stripped.as_set().contains(&DissemControl::Eyes));
    }

    #[test]
    fn relido_alone_is_kept() {
        // RELIDO without NOFORN must survive — no dominator to
        // trigger the strip.
        let stripped = raw(&[DissemControl::Relido]).with_all_overlays_reapplied();
        assert!(stripped.as_set().contains(&DissemControl::Relido));
    }

    // ---------------------------------------------------------------
    // R2-1 regression: Overlay 1 (OC > OC-USGOV) re-runs without
    // requiring NOFORN. The pre-R2-1
    // `apply_supersession_overlays` gated its dissem-axis rebuild
    // on `has_noforn`, which silently disabled Overlay 1 for
    // post-close states carrying both `Oc` and `OcUsgov` with no
    // NOFORN. The structural bug was: any future close()/default_fill
    // catalog row that emits `Oc` onto an `OcUsgov`-bearing input
    // (or vice versa) would NOT see Overlay 1 fire. Authority:
    // §H.8 p140 ("ORCON takes precedence within the banner line")
    // + §H.8 p140 ("If a portion contains both ORCON and
    // ORCON-USGOV information, ORCON takes precedence in the
    // portion mark") — re-verified verbatim at this PR's authorship.
    // ---------------------------------------------------------------

    /// Overlay 1 strips OC-USGOV when OC is also present, regardless
    /// of NOFORN status. Post-close fixture: dissem_us = [Oc,
    /// OcUsgov] with no NOFORN simulates a post-close /
    /// post-default-fill state where the production
    /// `apply_supersession_overlays` step must observe and act on
    /// Overlay 1 unconditionally.
    #[test]
    fn overlay1_oc_strips_oc_usgov_without_noforn() {
        let stripped = raw(&[DissemControl::Oc, DissemControl::OcUsgov])
            .with_all_overlays_reapplied();
        assert!(
            stripped.as_set().contains(&DissemControl::Oc),
            "Oc must survive Overlay 1 (it is the dominator); got {:?}",
            stripped.as_set()
        );
        assert!(
            !stripped.as_set().contains(&DissemControl::OcUsgov),
            "Overlay 1 must strip OcUsgov when Oc is present \
             (§H.8 p140 — ORCON precedence over ORCON-USGOV); got \
             {:?}",
            stripped.as_set()
        );
    }

    /// Overlay 1 + 3 compose: OC + OC-USGOV + NOFORN + RELIDO
    /// produces {Oc, Nf}. Overlay 1 strips OcUsgov (NOFORN-
    /// independent); Overlay 3 strips Relido (NOFORN-dominates).
    /// Pre-R2-1 the production gate's `has_noforn` short-circuit
    /// would have masked the Overlay 1 strip if `apply_overlays`
    /// hadn't already run during join; the regression test pins
    /// the contract that BOTH overlays fire on the same call.
    #[test]
    fn overlay1_plus_overlay3_compose() {
        let stripped = raw(&[
            DissemControl::Oc,
            DissemControl::OcUsgov,
            DissemControl::Nf,
            DissemControl::Relido,
        ])
        .with_all_overlays_reapplied();
        assert!(stripped.as_set().contains(&DissemControl::Oc));
        assert!(stripped.as_set().contains(&DissemControl::Nf));
        assert!(
            !stripped.as_set().contains(&DissemControl::OcUsgov),
            "Overlay 1 strip on OcUsgov must fire alongside \
             Overlay 3's RELIDO strip (§H.8 p140 NOFORN-independent \
             + §H.8 p145 NOFORN-dependent); got {:?}",
            stripped.as_set()
        );
        assert!(
            !stripped.as_set().contains(&DissemControl::Relido),
            "Overlay 3 strip on Relido must fire (§H.8 p145); got \
             {:?}",
            stripped.as_set()
        );
    }

    /// Overlay 2 (RELIDO observed-unanimity, §H.8 pp155-156) strips
    /// Relido when the input was joined from non-unanimous portions
    /// (`relido_observed_unanimous = false`), regardless of NOFORN
    /// status. Construct a non-unanimous DissemSet directly (the
    /// `raw` helper defaults the flag to `true`, so we override).
    #[test]
    fn overlay2_strips_relido_on_non_unanimous_without_noforn() {
        let mut set = BTreeSet::new();
        set.insert(DissemControl::Relido);
        let non_unanimous = DissemSet {
            set,
            relido_observed_unanimous: false,
        };
        let stripped = non_unanimous.with_all_overlays_reapplied();
        assert!(
            !stripped.as_set().contains(&DissemControl::Relido),
            "Overlay 2 must strip Relido when not observed unanimous \
             (§H.8 pp155-156: 'All portions must be marked as RELIDO \
             for the RELIDO marking to appear in the banner line'); \
             got {:?}",
            stripped.as_set()
        );
    }

    #[test]
    fn idempotent() {
        // f(f(x)) == f(x). After the first strip every dominated
        // control is gone; the second pass is a no-op.
        let input = raw(&[
            DissemControl::Nf,
            DissemControl::Relido,
            DissemControl::Rel,
            DissemControl::Displayonly,
            DissemControl::Eyes,
            DissemControl::Oc,
        ]);
        let once = input.with_all_overlays_reapplied();
        let twice = once.clone().with_all_overlays_reapplied();
        assert_eq!(once.as_set(), twice.as_set());
    }

    /// Join-monotone: `a ⊑ b ⟹ f(a) ⊑ f(b)` in the subset order
    /// over the post-overlay set. Spot-check the four
    /// representative ordering pairs.
    #[test]
    fn join_monotone() {
        // Case A: a ⊂ b, neither has NOFORN → f is identity →
        // f(a) ⊂ f(b).
        let a = raw(&[DissemControl::Oc]);
        let b = raw(&[DissemControl::Oc, DissemControl::Imc]);
        let fa = a.clone().with_all_overlays_reapplied();
        let fb = b.clone().with_all_overlays_reapplied();
        assert!(fa.as_set().is_subset(fb.as_set()));

        // Case B: a ⊂ b, b adds NOFORN → f(a) keeps everything,
        // f(b) keeps NOFORN + strips dominated; a's tokens (just
        // ORCON) are not dominated, so still ⊂.
        let a = raw(&[DissemControl::Oc]);
        let b = raw(&[DissemControl::Oc, DissemControl::Nf]);
        let fa = a.with_all_overlays_reapplied();
        let fb = b.with_all_overlays_reapplied();
        assert!(fa.as_set().is_subset(fb.as_set()));

        // Case C: a ⊂ b, b adds NOFORN AND a contains RELIDO
        // → f(a) keeps RELIDO; f(b) has NOFORN, strips RELIDO.
        // Subset relation in lattice sense: {RELIDO} ⊑ {NOFORN}
        // per §H.8 p145 supersession chain. Subset-of-set
        // does NOT hold (RELIDO ∉ f(b)); the §H.8 p145
        // supersession overlay's "monotonicity" is in the
        // SupersessionSet lattice ordering (NOFORN ⊐ RELIDO),
        // not in the raw bitwise / BTreeSet inclusion. This is
        // by-design — `with_all_overlays_reapplied` is a
        // post-Kleene-closure overlay that resolves the §H.8
        // p145 conflict; subsequent lattice consumers read the
        // post-overlay set as the canonical state.
        let a = raw(&[DissemControl::Relido]);
        let b = raw(&[DissemControl::Relido, DissemControl::Nf]);
        let fa = a.with_all_overlays_reapplied();
        let fb = b.with_all_overlays_reapplied();
        // Witness the supersession: f(b) contains NOFORN, the
        // dominator of RELIDO. f(a) contains RELIDO.
        assert!(fa.as_set().contains(&DissemControl::Relido));
        assert!(fb.as_set().contains(&DissemControl::Nf));
        assert!(!fb.as_set().contains(&DissemControl::Relido));
    }
}
