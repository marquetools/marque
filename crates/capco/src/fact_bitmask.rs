// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO atom inventory + projection over [`FactBitmask`].
//!
//! This module owns the *CAPCO-specific* bit assignment for the
//! domain-neutral [`marque_scheme::FactBitmask`] storage primitive.
//! Atom names live here; the bitwise storage primitive lives in
//! `marque-scheme`. Constitution VII placement disposition from the
//! 2026-05-20 PR-B plan, OQ-1 (resolved Option C).
//!
//! # Sub-PR scope (PR-B)
//!
//! PR-B lands the projection mathematics ONLY:
//!
//! - [`fact_bit`] — the 51-atom CAPCO inventory (bit indices 0..51,
//!   the remaining 77 bits split between CAPCO future-growth at
//!   51..96 and foreign-grammar future use at 96..128).
//! - Pre-#704: three aggregate masks (`MASK_FDR_DOMINATORS`,
//!   `MASK_FDR_OR_RELIDO_INCOMPAT`, `MASK_RELIDO_US_CLASS_SUPPRESSORS`)
//!   backed `CLOSURE_TABLE`'s `suppressor_mask` field. Issue #704
//!   retired the entire suppressor architecture (it violated the
//!   closure operator's algebraic monotonicity property); the §H.8
//!   p145 / §B.3.a p19 FD&R supersession semantics moved to
//!   `CapcoScheme::apply_supersession_overlays`. The aggregate
//!   masks retired with the suppressor field.
//! - [`derive_bits`] — forward projection
//!   `&CanonicalAttrs → FactBitmask` over the closed-vocab fields.
//! - [`apply_closed_bits_to`] — inverse projection that writes the
//!   closure-cone delta back into `CanonicalAttrs`. Only the four
//!   closure-cone outputs (NOFORN, ORCON, RELIDO, REL_TO_USA) are
//!   eligible for write-back; every other atom is observation-only.
//!
//! PR-B does NOT touch `CapcoScheme::closure` or the `CLOSURE_TABLE`
//! catalog — those are PR-C / PR-D. PR-B's only consumers are the
//! co-located unit tests + the round-trip proptests in
//! `crates/capco/tests/proptest_fact_bitmask_roundtrip.rs`. The
//! `#[allow(dead_code)]` below silences the per-item warnings every
//! non-test build emits; PR-C lands the `CLOSURE_TABLE` consumer
//! and PR-D wires `CapcoScheme::closure` to call `derive_bits` /
//! `apply_closed_bits_to` on the hot path, at which point this
//! attribute deletes.
//!
//! # Visibility
//!
//! The module is declared `#[doc(hidden)] pub mod` in `lib.rs` —
//! integration tests in `crates/capco/tests/` link against the
//! crate as an external dependency and need access to the projection
//! helpers (a `pub(crate)` module would be invisible to them). The
//! `#[doc(hidden)]` keeps the module out of rustdoc and signals
//! "internal API, do not consume from outside the crate". PR-C wires
//! the `CLOSURE_TABLE` consumer; PR-D wires `CapcoScheme::closure`.
//! Once both land, the visibility tightens back to `pub(crate)` and
//! `#[doc(hidden)]` deletes.
//!
//! # Atom layout (mirrors `docs/plans/2026-05-20-371-factbitmask-refactor.md` §3)
//!
//! | Bits | Count | Axis |
//! |---|---|---|
//! | 0–12  | 13 | US IC dissem (NOFORN, RELIDO, DISPLAY_ONLY, ORCON, ORCON_USGOV, EYES, RSEN, IMCON, PROPIN, DSEN, FISA, RAWFISA, FOUO) |
//! | 13–21 | 9  | Non-IC dissem (NODIS, EXDIS, SBU_NF, LES_NF, LIMDIS, LES, SBU, SSI, NNPI) |
//! | 22–26 | 5  | Closed AEA (RD, FRD, TFNI, DOE_UCNI, DOD_UCNI) |
//! | 27–29 | 3  | US classification 3-bit OrdMax chain |
//! | 30    | 1  | `US_COLLATERAL_CLASSIFIED` derived sentinel (US chain ≥ Restricted) |
//! | 31    | 1  | `US_UNCLASSIFIED` derived sentinel (US chain = U) |
//! | 32–34 | 3  | NATO classification 3-bit OrdMax chain |
//! | 35    | 1  | `NATO_CLASS` presence sentinel |
//! | 36    | 1  | `SAR_PRESENT` |
//! | 37    | 1  | `SCI_PRESENT` |
//! | 38    | 1  | `FGI_PRESENT` (FgiMarker OR FGI classification) |
//! | 39    | 1  | `JOINT_PRESENT` |
//! | 40–45 | 6  | SCI sentinels (SI_G, HCS_O, HCS_P_SUB, TK_BLFH, TK_IDIT, TK_KAND) |
//! | 46    | 1  | `REL_TO_PRESENT` |
//! | 47    | 1  | `REL_TO_USA` |
//! | 48    | 1  | `AEA_ATOMAL` (NATO AEA) |
//! | 49    | 1  | `AEA_BOHEMIA` (NATO SAP) |
//! | 50    | 1  | `AEA_BALK` (NATO SAP) |
//!
//! **Classification chains are NOT bitwise-or-joinable.** Bits 27-29
//! (US) and 32-34 (NATO) encode an `OrdMax` ladder (`000` = absent,
//! `001` = U, ..., `100` = TS / CTS). Bitwise OR of two ladders does
//! NOT compute their max — callers MUST extract the 3-bit field and
//! perform numeric compare. The bits live in the bitmask to give the
//! `CLOSURE_TABLE` (PR-C) a uniform input shape; the joins live in
//! the lattice halves on `ClassificationLattice` / `NatoClassLattice`.

#![allow(dead_code)] // PR-B sidecar; PR-C / PR-D wire the production consumers.

use marque_ism::{
    AeaMarking, Classification, CountryCode, DissemControl, MarkingClassification,
    NatoClassification, NonIcDissem, SciControlSystem, canonical::CanonicalAttrs,
};
use marque_scheme::FactBitmask;

/// Numeric bit indices for every CAPCO atom that participates in
/// closure-table dispatch.
///
/// Bit values are sequential within each axis and grouped by axis
/// per the layout table in this module's docstring. Each constant is
/// a `u32` so it composes directly with
/// [`FactBitmask::with_bit`] / [`FactBitmask::is_set`].
#[allow(non_upper_case_globals)]
pub mod fact_bit {
    // ----- US IC dissem (13 atoms, bits 0..13) -----
    pub const NOFORN: u32 = 0;
    pub const RELIDO: u32 = 1;
    pub const DISPLAY_ONLY: u32 = 2;
    pub const ORCON: u32 = 3;
    pub const ORCON_USGOV: u32 = 4;
    pub const EYES: u32 = 5;
    pub const RSEN: u32 = 6;
    pub const IMCON: u32 = 7;
    pub const PROPIN: u32 = 8;
    pub const DSEN: u32 = 9;
    pub const FISA: u32 = 10;
    pub const RAWFISA: u32 = 11;
    pub const FOUO: u32 = 12;

    // ----- Non-IC dissem (9 atoms, bits 13..22) -----
    pub const NODIS: u32 = 13;
    pub const EXDIS: u32 = 14;
    pub const SBU_NF: u32 = 15;
    pub const LES_NF: u32 = 16;
    pub const LIMDIS: u32 = 17;
    pub const LES: u32 = 18;
    pub const SBU: u32 = 19;
    pub const SSI: u32 = 20;
    pub const NNPI: u32 = 21;

    // ----- Closed AEA (5 atoms, bits 22..27) -----
    pub const AEA_RD: u32 = 22;
    pub const AEA_FRD: u32 = 23;
    pub const AEA_TFNI: u32 = 24;
    pub const AEA_DOE_UCNI: u32 = 25;
    pub const AEA_DOD_UCNI: u32 = 26;

    // ----- US classification 3-bit OrdMax chain (bits 27..30) -----
    // Encoded as a small integer in the [27, 30) field — NOT bitwise-
    // OR-joinable. Use `extract_us_class_level` to read back.
    pub const US_CLASS_BIT0: u32 = 27;
    pub const US_CLASS_BIT1: u32 = 28;
    pub const US_CLASS_BIT2: u32 = 29;

    // ----- Derived US sentinels (bits 30..32) -----
    pub const US_COLLATERAL_CLASSIFIED: u32 = 30;
    pub const US_UNCLASSIFIED: u32 = 31;

    // ----- NATO classification 3-bit OrdMax chain (bits 32..35) -----
    pub const NATO_CLASS_BIT0: u32 = 32;
    pub const NATO_CLASS_BIT1: u32 = 33;
    pub const NATO_CLASS_BIT2: u32 = 34;

    // ----- Presence sentinels (bits 35..40) -----
    pub const NATO_CLASS: u32 = 35;
    pub const SAR_PRESENT: u32 = 36;
    pub const SCI_PRESENT: u32 = 37;
    pub const FGI_PRESENT: u32 = 38;
    pub const JOINT_PRESENT: u32 = 39;

    // ----- SCI compartment sentinels (6 atoms, bits 40..46) -----
    pub const SCI_SI_G: u32 = 40;
    pub const SCI_HCS_O: u32 = 41;
    pub const SCI_HCS_P_SUB: u32 = 42;
    pub const SCI_TK_BLFH: u32 = 43;
    pub const SCI_TK_IDIT: u32 = 44;
    pub const SCI_TK_KAND: u32 = 45;

    // ----- REL TO sentinels (bits 46..48) -----
    pub const REL_TO_PRESENT: u32 = 46;
    pub const REL_TO_USA: u32 = 47;

    // ----- NATO AEA + NATO SAP sentinels (bits 48..51) -----
    pub const AEA_ATOMAL: u32 = 48;
    pub const AEA_BOHEMIA: u32 = 49;
    pub const AEA_BALK: u32 = 50;
}

/// Highest assigned bit + 1. Drives the [`FactBitmask::WIDTH`]
/// fits-in-128 compile-time guard. Whenever a new atom is added,
/// bump this and re-run the static assert below.
///
/// `pub` visibility (not `pub(crate)`) so the PR-C proptest harness
/// at `crates/capco/tests/proptest_closure_table.rs` can pin its
/// local `INVENTORY_MASK` against this single source of truth via
/// `const _: () = assert!(...)`. Tightening back to `pub(crate)`
/// after PR-D wires production would break the staleness gate;
/// the public-but-doc-hidden visibility (`#[doc(hidden)] pub mod
/// fact_bitmask` in `lib.rs`) keeps this off the documentation
/// surface while making it reachable for the gate.
pub const CAPCO_ATOM_COUNT: u32 = fact_bit::AEA_BALK + 1;

// FactBitmask::WIDTH is 128. The atom inventory MUST fit. A future
// scheme that exceeds 128 atoms requires a wider primitive in
// `marque-scheme`; that's a planned migration, not a runtime panic.
const _: () = {
    assert!(
        CAPCO_ATOM_COUNT <= marque_scheme::FACT_BITMASK_WIDTH,
        "CAPCO atom inventory exceeds FactBitmask::WIDTH; widen the \
         primitive in marque-scheme or split a CAPCO atom out."
    );
};

// `MASK_FDR_DOMINATORS`, `MASK_FDR_OR_RELIDO_INCOMPAT`, and
// `MASK_RELIDO_US_CLASS_SUPPRESSORS` were retired in issue #704
// along with the rest of the `CLOSURE_TABLE` suppressor architecture.
// The §H.8 p145 NOFORN-dominates and §B.3.a p19 FD&R supersession
// semantics that the masks gated moved to
// `CapcoScheme::apply_supersession_overlays` and the per-axis
// `DissemSet::with_fdr_dominance_stripped` /
// `RelToBlock::with_nato_implicit_stripped` overlays. The canonical
// FD&R-membership source remains the `FDR_DOMINATORS` `TokenRef`
// slice in `crates/capco/src/scheme/closure.rs`, which is consumed
// by `Vocabulary::is_fdr_dissem` and the supersession overlays.

/// Closure-cone outputs that [`apply_closed_bits_to`] is willing to
/// write back to [`CanonicalAttrs`].
///
/// Every CAPCO closure-row cone in section 4 of the PR-B plan resolves to
/// one of these four atoms (NOFORN / ORCON / RELIDO for the dissem
/// axis; REL_TO_USA for the country-list axis). Other delta bits in
/// `(closed & !input)` are silently ignored by `apply_closed_bits_to`
/// — they reflect derived sentinels or input-only atoms that have no
/// inverse-projection semantics.
///
/// Domain crates never call this constant; it's an internal
/// invariant of the projection layer.
const APPLY_ELIGIBLE_MASK: u128 = (1u128 << fact_bit::NOFORN)
    | (1u128 << fact_bit::ORCON)
    | (1u128 << fact_bit::RELIDO)
    | (1u128 << fact_bit::REL_TO_USA);

/// Forward projection: project the closed-vocab axes of
/// [`CanonicalAttrs`] into a [`FactBitmask`].
///
/// The mapping is structural — every atom whose presence the
/// `CLOSURE_TABLE` (PR-C) needs to read flips the corresponding bit.
/// Atoms outside the closed inventory (open-vocab FGI tetragraphs
/// beyond `FGI_PRESENT`, open-vocab REL TO country codes beyond
/// `USA`, custom SCI control-system identifiers, custom SAR program
/// names) are observed only as presence sentinels; their detail
/// survives on `CanonicalAttrs` itself (PR-D's `closure()` rewire
/// runs the bitmask Kleene loop AND a follow-up open-vocab cone pass
/// for Row 7's `cone_derived` NATO tetragraph emission).
///
/// Branchless and single-pass over each input slice. Allocates
/// nothing. Round-trips with [`apply_closed_bits_to`] on the four
/// `APPLY_ELIGIBLE_MASK` cone-output atoms.
///
/// # Classification-chain encoding
///
/// US classification (bits 27-29) and NATO classification (bits 32-34)
/// are NOT bitwise-OR-joinable atoms — the 3-bit field encodes the
/// classification level as a small integer (`001 = U`, `010 = R`,
/// `011 = C`, `100 = S`, `101 = TS / CTS`). Read back with
/// [`extract_us_class_level`] / [`extract_nato_class_level`]. The
/// derived sentinels `US_COLLATERAL_CLASSIFIED` (bit 30) and
/// `US_UNCLASSIFIED` (bit 31) are bitwise-friendly and exist
/// precisely so the `CLOSURE_TABLE` can use them as triggers without
/// chain-extract logic.
pub fn derive_bits(attrs: &CanonicalAttrs) -> FactBitmask {
    let mut bits: u128 = 0;

    // --- US IC dissem (bits 0..13) + EYES ---
    //
    // `dissem_iter()` walks both `dissem_us` and `dissem_nato`
    // namespaces. The closure operator does not distinguish IC dissem
    // by attribution namespace (see `iter_present_tokens`) so the
    // bitmask follows the same any-of semantic — a NATO portion
    // carrying ORCON on the NATO side still trips the ORCON bit.
    for dc in attrs.dissem_iter() {
        bits |= match dc {
            DissemControl::Nf => 1u128 << fact_bit::NOFORN,
            DissemControl::Relido => 1u128 << fact_bit::RELIDO,
            DissemControl::Displayonly => 1u128 << fact_bit::DISPLAY_ONLY,
            DissemControl::Oc => 1u128 << fact_bit::ORCON,
            DissemControl::OcUsgov => 1u128 << fact_bit::ORCON_USGOV,
            DissemControl::Eyes => 1u128 << fact_bit::EYES,
            DissemControl::Rs => 1u128 << fact_bit::RSEN,
            DissemControl::Imc => 1u128 << fact_bit::IMCON,
            DissemControl::Pr => 1u128 << fact_bit::PROPIN,
            DissemControl::Dsen => 1u128 << fact_bit::DSEN,
            DissemControl::Fisa => 1u128 << fact_bit::FISA,
            DissemControl::Rawfisa => 1u128 << fact_bit::RAWFISA,
            DissemControl::Fouo => 1u128 << fact_bit::FOUO,
            // `Rel` is the dissem-axis marker that the parser emits
            // alongside a populated `rel_to` country list; the
            // `REL_TO_PRESENT` bit downstream captures that more
            // robustly. `ExemptFromIcd501Discovery` is a special
            // marking outside the §H.8 family; no `CLOSURE_TABLE` row
            // reads it. The wildcard arm catches any future
            // `#[non_exhaustive]` additions and is the explicit
            // forward-compat guard — a newly registered dissem token
            // contributes no bit until its closure semantics land
            // alongside an atom-inventory bump.
            DissemControl::Rel | DissemControl::ExemptFromIcd501Discovery => 0,
            _ => 0,
        };
    }

    // --- Non-IC dissem (bits 13..22) ---
    for nd in attrs.non_ic_dissem.iter() {
        bits |= match nd {
            NonIcDissem::Nodis => 1u128 << fact_bit::NODIS,
            NonIcDissem::Exdis => 1u128 << fact_bit::EXDIS,
            NonIcDissem::SbuNf => 1u128 << fact_bit::SBU_NF,
            NonIcDissem::LesNf => 1u128 << fact_bit::LES_NF,
            NonIcDissem::Limdis => 1u128 << fact_bit::LIMDIS,
            NonIcDissem::Les => 1u128 << fact_bit::LES,
            NonIcDissem::Sbu => 1u128 << fact_bit::SBU,
            NonIcDissem::Ssi => 1u128 << fact_bit::SSI,
            NonIcDissem::Nnpi => 1u128 << fact_bit::NNPI,
            // `#[non_exhaustive]` forward-compat: new non-IC dissem
            // tokens contribute no bit until their closure semantics
            // land alongside an atom-inventory bump.
            _ => 0,
        };
    }

    // --- AEA (bits 22..27 + 48 ATOMAL) ---
    for aea in attrs.aea_markings.iter() {
        bits |= match aea {
            AeaMarking::Rd(_) => 1u128 << fact_bit::AEA_RD,
            AeaMarking::Frd(_) => 1u128 << fact_bit::AEA_FRD,
            AeaMarking::Tfni => 1u128 << fact_bit::AEA_TFNI,
            AeaMarking::DoeUcni => 1u128 << fact_bit::AEA_DOE_UCNI,
            AeaMarking::DodUcni => 1u128 << fact_bit::AEA_DOD_UCNI,
            AeaMarking::Atomal(_) => 1u128 << fact_bit::AEA_ATOMAL,
            // `#[non_exhaustive]` forward-compat: new AEA tokens
            // contribute no bit until their closure semantics land.
            _ => 0,
        };
    }

    // --- Classification chains (bits 27..30 US, 32..35 NATO) + presence sentinels ---
    if let Some(class) = &attrs.classification {
        match class {
            MarkingClassification::Us(c) | MarkingClassification::Conflict { us: c, .. } => {
                bits |= encode_us_class(*c) << fact_bit::US_CLASS_BIT0;
                if *c >= Classification::Restricted {
                    bits |= 1u128 << fact_bit::US_COLLATERAL_CLASSIFIED;
                } else {
                    // U: Conflict only ever stores Unclassified..TopSecret;
                    // matches the `attrs.us_classification()` accessor.
                    bits |= 1u128 << fact_bit::US_UNCLASSIFIED;
                }
            }
            MarkingClassification::Fgi(f) => {
                bits |= 1u128 << fact_bit::FGI_PRESENT;
                let _ = f; // FGI level lives on lattice halves, not bitmask
            }
            MarkingClassification::Nato(n) => {
                bits |= encode_nato_class(*n) << fact_bit::NATO_CLASS_BIT0;
                bits |= 1u128 << fact_bit::NATO_CLASS;
            }
            MarkingClassification::Joint(_) => {
                bits |= 1u128 << fact_bit::JOINT_PRESENT;
            }
        }
    }

    // FGI marker presence (dissem-axis FGI, distinct from
    // classification-axis FgiClassification). Either path lights
    // `FGI_PRESENT`; the suppressor masks treat them uniformly per
    // CAPCO §H.7 p123.
    if attrs.fgi_marker.is_some() {
        bits |= 1u128 << fact_bit::FGI_PRESENT;
    }

    // --- Structural presence sentinels (bits 36, 37, 46) ---
    if attrs.sar_markings.is_some() {
        bits |= 1u128 << fact_bit::SAR_PRESENT;
    }
    if !attrs.sci_markings.is_empty() || !attrs.sci_controls.is_empty() {
        bits |= 1u128 << fact_bit::SCI_PRESENT;
    }
    if !attrs.rel_to.is_empty() {
        bits |= 1u128 << fact_bit::REL_TO_PRESENT;
    }
    if attrs.rel_to.contains(&CountryCode::USA) {
        bits |= 1u128 << fact_bit::REL_TO_USA;
    }
    // DISPLAY ONLY presence: the canonical wire form
    // `DISPLAY ONLY [LIST]` populates `attrs.display_only_to` (the
    // country-list axis) rather than `attrs.dissem_us`. Mirror
    // `satisfies_attrs(TOK_DISPLAY_ONLY)` in
    // `crates/capco/src/scheme/predicates/satisfies.rs` —
    // DISPLAY_ONLY fires iff EITHER the `Displayonly` dissem token
    // is present OR `display_only_to` is non-empty. The dissem-axis
    // branch is already covered in the `dissem_iter()` loop above
    // (line 290); this branch closes the country-list axis case so
    // the supersession overlays (post-#704
    // `CapcoScheme::apply_supersession_overlays`) and
    // `apply_closed_bits_to`'s NOFORN-strip path observe DISPLAY
    // ONLY presence on any §H.8 portion regardless of which axis
    // the parser routed it through.
    //
    // Authority: §H.8 p163 (DISPLAY ONLY marking template) + the
    // existing satisfies_attrs(TOK_DISPLAY_ONLY) wiring.
    if !attrs.display_only_to.is_empty() {
        bits |= 1u128 << fact_bit::DISPLAY_ONLY;
    }

    // --- SCI compartment sentinels (bits 40..46) + NATO SAPs (49, 50) ---
    //
    // The structural reads mirror
    // `crates/capco/src/scheme/predicates/satisfies.rs` exactly so the
    // bitmask reflects the same any-portion semantic the existing
    // `TOK_SI_G` / `TOK_HCS_O` etc. predicates use. Branchless single
    // pass; the hot path lives in PR-D's `closure()` rewire which calls
    // `derive_bits` once per page.
    for sci in attrs.sci_markings.iter() {
        match &sci.system {
            SciControlSystem::Published(bare) => {
                use marque_ism::SciControlBare;
                for comp in sci.compartments.iter() {
                    let ident = comp.identifier.as_str();
                    let has_sub = !comp.sub_compartments.is_empty();
                    match (bare, ident, has_sub) {
                        (SciControlBare::Si, "G", _) => bits |= 1u128 << fact_bit::SCI_SI_G,
                        (SciControlBare::Hcs, "O", _) => bits |= 1u128 << fact_bit::SCI_HCS_O,
                        (SciControlBare::Hcs, "P", true) => {
                            bits |= 1u128 << fact_bit::SCI_HCS_P_SUB;
                        }
                        (SciControlBare::Tk, "BLFH", _) => bits |= 1u128 << fact_bit::SCI_TK_BLFH,
                        (SciControlBare::Tk, "IDIT", _) => bits |= 1u128 << fact_bit::SCI_TK_IDIT,
                        (SciControlBare::Tk, "KAND", _) => bits |= 1u128 << fact_bit::SCI_TK_KAND,
                        _ => {}
                    }
                }
            }
            SciControlSystem::NatoSap(sap) => {
                use marque_ism::NatoSap;
                bits |= match sap {
                    NatoSap::Bohemia => 1u128 << fact_bit::AEA_BOHEMIA,
                    NatoSap::Balk => 1u128 << fact_bit::AEA_BALK,
                    // `#[non_exhaustive]` forward-compat: future
                    // NATO SAPs contribute no bit until registered.
                    _ => 0,
                };
            }
            SciControlSystem::Custom(_) => {
                // Custom agency-allocated SCI control systems carry no
                // bit; presence is already captured via `SCI_PRESENT`.
            }
        }
    }

    FactBitmask::from_bits(bits)
}

/// Inverse projection: write the closure-cone delta back into
/// [`CanonicalAttrs`].
///
/// Computes `delta = (closed.bits() & !input.bits()) &
/// APPLY_ELIGIBLE_MASK` and adds the corresponding atom to the
/// matching axis on `attrs`. Only the four closure-cone outputs
/// (NOFORN, ORCON, RELIDO, REL_TO_USA) are eligible — other delta
/// bits are silently ignored. They reflect derived sentinels or
/// input-only atoms with no inverse-projection semantic.
///
/// # Allocation profile (Constitution Principle II)
///
/// At most TWO heap allocations per call — one rebuild of
/// `attrs.dissem_us` (if any of NOFORN / ORCON / RELIDO is in the
/// delta), one rebuild of `attrs.rel_to` (if REL_TO_USA is in the
/// delta). The three cone-output dissem atoms are coalesced into a
/// single `Vec::with_capacity(...)` + `into_boxed_slice` rebuild
/// rather than per-bit allocate-push cycles. PR-D calls this
/// function on the closure hot path; the allocation profile here
/// is the budget that PR-D's `lint_latency` SC-001 non-regression
/// gate enforces.
///
/// # Idempotence
///
/// If `closed == input`, no mutation happens (delta is empty).
/// Calling the function twice with the same `(closed, input)`
/// arguments is also a no-op on the second call — the eligible-bit
/// guards check `attrs.dissem_iter()` / `attrs.rel_to.contains` to
/// short-circuit re-adds. This is the round-trip law the PR-B
/// proptest harness asserts.
///
/// # §H.8 p145 NOFORN supersession
///
/// When the NOFORN bit is in the delta, `apply_closed_bits_to` runs
/// the full §H.8 p145 dominators overlay alongside the dissem-axis
/// insertion. NOFORN cannot coexist with `Rel`, `Relido`,
/// `Displayonly`, `Eyes` in the same axis, nor with a populated
/// `rel_to` or `display_only_to` country list. The overlay mirrors
/// the existing `apply_fact_add` NOFORN insertion path in
/// `scheme/actions/intent.rs:327-340`:
///
/// 1. Strip `Rel` / `Relido` / `Displayonly` / `Eyes` from
///    `attrs.dissem_us` while inserting `Nf`.
/// 2. Clear `attrs.rel_to` (the country-list axis §H.8 p145
///    dominates alongside the token-axis eviction).
/// 3. Clear `attrs.display_only_to` (mirror of `rel_to` for the
///    DISPLAY ONLY country-list axis).
///
/// **Why this is needed even though `CLOSURE_NOFORN_CAVEATED`
/// gates on `REL_TO_PRESENT`.** PR-C's closure-table catalog
/// has five per-marking unconditional rows (HCS-O / HCS-P[sub] /
/// TK-BLFH / TK-IDIT / TK-KAND) that add NOFORN with NO
/// suppressors. Without this overlay, those rows could leave the
/// marking in an invalid mixed state (NOFORN + REL TO) on
/// portions where the SCI sentinel coexists with a pre-existing
/// FD&R decision — exactly the bug `apply_fact_add` already
/// guards against on the manual-FactAdd path.
///
/// Authority: §H.8 p145 ("NOFORN: cannot be used with REL TO,
/// RELIDO, EYES ONLY, or DISPLAY ONLY") + §D.2 Table 3 rows 1-2.
/// Coverage: `apply_preserves_h8_p145_invariant` proptest +
/// `apply_noforn_supersedes_dominated_*` unit tests.
pub fn apply_closed_bits_to(attrs: &mut CanonicalAttrs, closed: FactBitmask, input: FactBitmask) {
    let delta = (closed.bits() & !input.bits()) & APPLY_ELIGIBLE_MASK;
    if delta == 0 {
        return;
    }

    let noforn_in_delta = (delta & (1u128 << fact_bit::NOFORN)) != 0;
    let orcon_in_delta = (delta & (1u128 << fact_bit::ORCON)) != 0;
    let relido_in_delta = (delta & (1u128 << fact_bit::RELIDO)) != 0;

    // Phase 1: rebuild `attrs.dissem_us` in a single allocation.
    //
    // If NOFORN is in the delta, the §H.8 p145 supersession overlay
    // ALSO strips dominated controls (`Rel`, `Relido`, `Displayonly`,
    // `Eyes`) from the surviving slice — so the rebuild path runs
    // even when only NOFORN is added (no new ORCON / RELIDO) because
    // existing dominated tokens still need eviction. The
    // existing-presence guards on ORCON / RELIDO use
    // `attrs.dissem_iter()` (both namespaces) for symmetry with
    // pre-PR-B `apply_fact_add`.
    //
    // Eviction precedence: NOFORN evicts RELIDO before the RELIDO
    // bit's add-guard fires; if both NOFORN and RELIDO are in the
    // delta in the same call, the result honors §H.8 p145 (NOFORN
    // wins, RELIDO is NOT added). This matches the existing
    // `apply_fact_add` behavior at intent.rs:283-292.
    let dissem_rebuild_needed = noforn_in_delta || orcon_in_delta || relido_in_delta;

    if dissem_rebuild_needed {
        // Stack buffer sized for the worst case: NOFORN + ORCON +
        // RELIDO. RELIDO is only ever added when NOFORN is NOT in the
        // delta, so the buffer is exactly large enough.
        let mut additions: [DissemControl; 3] = [DissemControl::Nf; 3];
        let mut add_count = 0usize;

        if noforn_in_delta && !attrs.dissem_iter().any(|d| *d == DissemControl::Nf) {
            additions[add_count] = DissemControl::Nf;
            add_count += 1;
        }
        if orcon_in_delta && !attrs.dissem_iter().any(|d| *d == DissemControl::Oc) {
            additions[add_count] = DissemControl::Oc;
            add_count += 1;
        }
        // §H.8 p145: RELIDO is dominated by NOFORN. Skip the RELIDO
        // add if NOFORN is in the delta or already present — the
        // dissem-axis eviction below handles any pre-existing RELIDO.
        if relido_in_delta
            && !noforn_in_delta
            && !attrs.dissem_iter().any(|d| *d == DissemControl::Relido)
        {
            additions[add_count] = DissemControl::Relido;
            add_count += 1;
        }

        let strip_dominators = noforn_in_delta;
        let dissem_us_keep_count = if strip_dominators {
            attrs
                .dissem_us
                .iter()
                .filter(|d| !is_noforn_dominated(d))
                .count()
        } else {
            attrs.dissem_us.len()
        };

        // Only rebuild if there's something to add OR something to
        // strip; otherwise the existing slice is byte-identical.
        let strip_needed = strip_dominators && dissem_us_keep_count != attrs.dissem_us.len();
        if add_count > 0 || strip_needed {
            let mut next: Vec<DissemControl> = Vec::with_capacity(dissem_us_keep_count + add_count);
            for d in attrs.dissem_us.iter() {
                if !strip_dominators || !is_noforn_dominated(d) {
                    next.push(*d);
                }
            }
            next.extend_from_slice(&additions[..add_count]);
            attrs.dissem_us = next.into_boxed_slice();
        }
    }

    // Phase 2: §H.8 p145 country-list dominance. NOFORN clears
    // `rel_to` AND `display_only_to` alongside the token-axis
    // eviction above. Mirror `apply_fact_add` at intent.rs:334-339.
    if noforn_in_delta {
        if !attrs.rel_to.is_empty() {
            attrs.rel_to = Box::new([]);
        }
        if !attrs.display_only_to.is_empty() {
            attrs.display_only_to = Box::new([]);
        }
    }

    // Phase 3: REL TO rebuild — at most one country (USA) added.
    // Suppressed when NOFORN was in the delta (Phase 2 just cleared
    // rel_to; adding USA back would re-violate §H.8 p145).
    if !noforn_in_delta
        && (delta & (1u128 << fact_bit::REL_TO_USA)) != 0
        && !attrs.rel_to.contains(&CountryCode::USA)
    {
        let mut next: Vec<CountryCode> = Vec::with_capacity(attrs.rel_to.len() + 1);
        next.extend_from_slice(&attrs.rel_to);
        next.push(CountryCode::USA);
        attrs.rel_to = next.into_boxed_slice();
    }
}

/// `true` iff `token` is one of the §H.8 p145 NOFORN dominators
/// (`Rel`, `Relido`, `Displayonly`, `Eyes`). Used by Phase 1 of
/// `apply_closed_bits_to` to strip dominated controls from
/// `attrs.dissem_us` when NOFORN is in the delta.
///
/// Authority: CAPCO-2016 §H.8 p145 + §D.2 Table 3 rows 1-2.
#[inline]
fn is_noforn_dominated(token: &DissemControl) -> bool {
    matches!(
        token,
        DissemControl::Rel
            | DissemControl::Relido
            | DissemControl::Displayonly
            | DissemControl::Eyes
    )
}

/// Extract the 3-bit US classification level from a [`FactBitmask`].
///
/// Returns `Some(level)` when bits 27-29 encode a known
/// `Classification` ladder value; `None` when the field is zero
/// (absent) or holds a reserved bit pattern. The
/// classification ladder is `U=1, R=2, C=3, S=4, TS=5` per
/// [`encode_us_class`].
#[inline]
pub(crate) fn extract_us_class_level(bits: FactBitmask) -> Option<Classification> {
    let field = ((bits.bits() >> fact_bit::US_CLASS_BIT0) & 0b111) as u8;
    match field {
        0 => None,
        1 => Some(Classification::Unclassified),
        2 => Some(Classification::Restricted),
        3 => Some(Classification::Confidential),
        4 => Some(Classification::Secret),
        5 => Some(Classification::TopSecret),
        _ => None,
    }
}

/// Extract the 3-bit NATO classification level from a [`FactBitmask`].
///
/// Returns `Some(level)` when bits 32-34 encode a known
/// `NatoClassification` ladder value; `None` when the field is zero
/// or holds a reserved bit pattern. The NATO ladder is
/// `NU=1, NR=2, NC=3, NS=4, CTS=5` per [`encode_nato_class`].
#[inline]
pub(crate) fn extract_nato_class_level(bits: FactBitmask) -> Option<NatoClassification> {
    let field = ((bits.bits() >> fact_bit::NATO_CLASS_BIT0) & 0b111) as u8;
    match field {
        0 => None,
        1 => Some(NatoClassification::NatoUnclassified),
        2 => Some(NatoClassification::NatoRestricted),
        3 => Some(NatoClassification::NatoConfidential),
        4 => Some(NatoClassification::NatoSecret),
        5 => Some(NatoClassification::CosmicTopSecret),
        _ => None,
    }
}

// ----------------------------------------------------------------------
// Internal helpers
// ----------------------------------------------------------------------

/// Encode a [`Classification`] as a 3-bit ladder value (1..=5; 0 means
/// absent and is reserved for callers).
const fn encode_us_class(c: Classification) -> u128 {
    match c {
        Classification::Unclassified => 1,
        Classification::Restricted => 2,
        Classification::Confidential => 3,
        Classification::Secret => 4,
        Classification::TopSecret => 5,
    }
}

/// Encode a [`NatoClassification`] as a 3-bit ladder value (1..=5).
const fn encode_nato_class(n: NatoClassification) -> u128 {
    match n {
        NatoClassification::NatoUnclassified => 1,
        NatoClassification::NatoRestricted => 2,
        NatoClassification::NatoConfidential => 3,
        NatoClassification::NatoSecret => 4,
        NatoClassification::CosmicTopSecret => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marque_ism::{Classification, DissemControl, MarkingClassification};

    fn empty() -> CanonicalAttrs {
        CanonicalAttrs::default()
    }

    #[test]
    fn derive_empty_attrs_is_zero() {
        let attrs = empty();
        assert_eq!(derive_bits(&attrs), FactBitmask::EMPTY);
    }

    #[test]
    fn derive_us_secret_sets_class_chain_and_collateral() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        let bits = derive_bits(&attrs);
        assert_eq!(extract_us_class_level(bits), Some(Classification::Secret));
        assert!(bits.is_set(fact_bit::US_COLLATERAL_CLASSIFIED));
        assert!(!bits.is_set(fact_bit::US_UNCLASSIFIED));
    }

    #[test]
    fn derive_us_unclassified_sets_unclassified_sentinel_only() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Unclassified));
        let bits = derive_bits(&attrs);
        assert!(bits.is_set(fact_bit::US_UNCLASSIFIED));
        assert!(!bits.is_set(fact_bit::US_COLLATERAL_CLASSIFIED));
        assert_eq!(
            extract_us_class_level(bits),
            Some(Classification::Unclassified),
        );
    }

    #[test]
    fn derive_dissem_us_sets_atoms() {
        let mut attrs = empty();
        attrs.dissem_us = vec![DissemControl::Nf, DissemControl::Oc].into();
        let bits = derive_bits(&attrs);
        assert!(bits.is_set(fact_bit::NOFORN));
        assert!(bits.is_set(fact_bit::ORCON));
        assert!(!bits.is_set(fact_bit::RELIDO));
    }

    #[test]
    fn derive_dissem_nato_sets_same_bits_as_us() {
        let mut attrs = empty();
        attrs.dissem_nato = vec![DissemControl::Oc].into();
        let bits = derive_bits(&attrs);
        // `dissem_iter()` walks both namespaces; the closure operator
        // does not differentiate ORCON-on-NATO from ORCON-on-US.
        assert!(bits.is_set(fact_bit::ORCON));
    }

    #[test]
    fn derive_rel_includes_usa_sentinel() {
        let mut attrs = empty();
        attrs.rel_to = vec![CountryCode::USA, CountryCode::GBR].into();
        let bits = derive_bits(&attrs);
        assert!(bits.is_set(fact_bit::REL_TO_PRESENT));
        assert!(bits.is_set(fact_bit::REL_TO_USA));
    }

    #[test]
    fn derive_rel_without_usa_sets_present_only() {
        let mut attrs = empty();
        attrs.rel_to = vec![CountryCode::GBR].into();
        let bits = derive_bits(&attrs);
        assert!(bits.is_set(fact_bit::REL_TO_PRESENT));
        assert!(!bits.is_set(fact_bit::REL_TO_USA));
    }

    #[test]
    fn apply_zero_delta_is_no_op() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        let before = attrs.clone();
        let bits = derive_bits(&attrs);
        apply_closed_bits_to(&mut attrs, bits, bits);
        assert_eq!(attrs, before);
    }

    #[test]
    fn apply_noforn_delta_adds_to_dissem_us() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        let input = derive_bits(&attrs);
        let closed = input.with_bit(fact_bit::NOFORN);
        apply_closed_bits_to(&mut attrs, closed, input);
        assert!(attrs.dissem_us.contains(&DissemControl::Nf));
    }

    #[test]
    fn apply_is_idempotent_under_repeated_call() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        let input = derive_bits(&attrs);
        let closed = input.with_bit(fact_bit::NOFORN).with_bit(fact_bit::RELIDO);
        apply_closed_bits_to(&mut attrs, closed, input);
        let after_first = attrs.clone();
        // Second call: the input is now `closed`, delta is empty.
        apply_closed_bits_to(&mut attrs, closed, closed);
        assert_eq!(attrs, after_first);
    }

    #[test]
    fn apply_rel_to_usa_delta_appends_usa() {
        let mut attrs = empty();
        // Existing GBR-only REL TO; closure adds USA per Row 7.
        attrs.rel_to = vec![CountryCode::GBR].into();
        let input = derive_bits(&attrs);
        let closed = input.with_bit(fact_bit::REL_TO_USA);
        apply_closed_bits_to(&mut attrs, closed, input);
        assert!(attrs.rel_to.contains(&CountryCode::USA));
        assert!(attrs.rel_to.contains(&CountryCode::GBR));
    }

    /// §H.8 p145: adding NOFORN evicts `Rel` / `Relido` /
    /// `Displayonly` / `Eyes` from `attrs.dissem_us` in the same
    /// call. Covers the Copilot-flagged hole: PR-C's unconditional
    /// per-marking NOFORN rows (HCS-O / HCS-P[sub] / TK-BLFH/IDIT/KAND
    /// at plan section 4 rows 1, 2, 4, 5, 6) have no suppressors and can
    /// fire on portions with pre-existing FD&R tokens.
    #[test]
    fn apply_noforn_supersedes_dominated_dissem_tokens() {
        for dominated in [
            DissemControl::Rel,
            DissemControl::Relido,
            DissemControl::Displayonly,
            DissemControl::Eyes,
        ] {
            let mut attrs = empty();
            attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
            attrs.dissem_us = vec![dominated].into();

            let input = derive_bits(&attrs);
            let closed = input.with_bit(fact_bit::NOFORN);
            apply_closed_bits_to(&mut attrs, closed, input);

            assert!(
                attrs.dissem_us.contains(&DissemControl::Nf),
                "NOFORN missing after apply for dominated={dominated:?}",
            );
            assert!(
                !attrs.dissem_us.contains(&dominated),
                "§H.8 p145: NOFORN did not evict {dominated:?}",
            );
        }
    }

    /// §H.8 p145: adding NOFORN clears `attrs.rel_to`. Closure rows
    /// without REL_TO_PRESENT in their suppressor mask can fire NOFORN
    /// onto a portion with a populated REL TO; the country-list axis
    /// must clear alongside the token-axis eviction.
    #[test]
    fn apply_noforn_clears_rel_to_country_list() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        attrs.rel_to = vec![CountryCode::USA, CountryCode::GBR].into();

        let input = derive_bits(&attrs);
        let closed = input.with_bit(fact_bit::NOFORN);
        apply_closed_bits_to(&mut attrs, closed, input);

        assert!(attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(attrs.rel_to.is_empty(), "§H.8 p145: rel_to not cleared");
    }

    /// §H.8 p145: adding NOFORN clears `attrs.display_only_to` —
    /// mirror of the rel_to clear above.
    #[test]
    fn apply_noforn_clears_display_only_to_country_list() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        attrs.display_only_to = vec![CountryCode::GBR].into();

        let input = derive_bits(&attrs);
        let closed = input.with_bit(fact_bit::NOFORN);
        apply_closed_bits_to(&mut attrs, closed, input);

        assert!(
            attrs.display_only_to.is_empty(),
            "§H.8 p145: display_only_to not cleared",
        );
    }

    /// §H.8 p145: when NOFORN AND RELIDO are both in the delta,
    /// NOFORN wins — RELIDO is NOT added to the dissem axis. Mirrors
    /// `apply_fact_add`'s same-call dominance guard at
    /// `intent.rs:283-292`.
    #[test]
    fn apply_noforn_dominates_simultaneous_relido_delta() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));

        let input = derive_bits(&attrs);
        let closed = input.with_bit(fact_bit::NOFORN).with_bit(fact_bit::RELIDO);
        apply_closed_bits_to(&mut attrs, closed, input);

        assert!(attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(
            !attrs.dissem_us.contains(&DissemControl::Relido),
            "§H.8 p145: NOFORN+RELIDO same-call should drop RELIDO",
        );
    }

    /// §H.8 p145: REL_TO_USA delta is suppressed when NOFORN is also
    /// in the delta — Phase 2 just cleared `rel_to`, re-adding USA
    /// would re-violate the dominance rule.
    #[test]
    fn apply_noforn_suppresses_simultaneous_rel_to_usa_delta() {
        let mut attrs = empty();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));

        let input = derive_bits(&attrs);
        let closed = input
            .with_bit(fact_bit::NOFORN)
            .with_bit(fact_bit::REL_TO_USA);
        apply_closed_bits_to(&mut attrs, closed, input);

        assert!(attrs.dissem_us.contains(&DissemControl::Nf));
        assert!(
            attrs.rel_to.is_empty(),
            "§H.8 p145: NOFORN+REL_TO_USA should leave rel_to empty",
        );
    }

    #[test]
    fn apply_ignores_non_eligible_delta_bits() {
        // ORCON_USGOV (bit 4) is in the atom inventory but NOT in
        // `APPLY_ELIGIBLE_MASK`. apply_closed_bits_to must not touch
        // dissem_us for it.
        let mut attrs = empty();
        let input = FactBitmask::EMPTY;
        let closed = FactBitmask::EMPTY.with_bit(fact_bit::ORCON_USGOV);
        apply_closed_bits_to(&mut attrs, closed, input);
        assert!(attrs.dissem_us.is_empty());
    }

    #[test]
    fn derive_us_secret_then_apply_noforn_is_byte_identical_to_manual_add() {
        // Bit-by-bit equivalence with the existing FactAdd path (for
        // closed-vocab axes only — the supersession overlay only
        // triggers on REL TO populated, which this fixture does not
        // exercise).
        let mut via_bitmask = empty();
        via_bitmask.classification = Some(MarkingClassification::Us(Classification::Secret));
        let input = derive_bits(&via_bitmask);
        let closed = input.with_bit(fact_bit::NOFORN);
        apply_closed_bits_to(&mut via_bitmask, closed, input);

        let mut via_manual = empty();
        via_manual.classification = Some(MarkingClassification::Us(Classification::Secret));
        via_manual.dissem_us = vec![DissemControl::Nf].into();

        assert_eq!(via_bitmask, via_manual);
    }

    #[test]
    fn apply_eligible_mask_within_inventory() {
        // The three pre-#704 aggregate suppressor masks
        // (MASK_FDR_DOMINATORS / MASK_FDR_OR_RELIDO_INCOMPAT /
        // MASK_RELIDO_US_CLASS_SUPPRESSORS) retired in issue #704
        // along with the rest of the `CLOSURE_TABLE` suppressor
        // architecture. The remaining APPLY_ELIGIBLE_MASK still gates
        // the apply_closed_bits_to writeback set and must stay inside
        // the active atom inventory.
        let inventory = (1u128 << CAPCO_ATOM_COUNT) - 1;
        assert_eq!(
            APPLY_ELIGIBLE_MASK & !inventory,
            0,
            "APPLY_ELIGIBLE_MASK contains a bit outside the atom inventory",
        );
    }

    /// Both DISPLAY ONLY projection paths — dissem-axis
    /// `DissemControl::Displayonly` and country-list axis non-empty
    /// `display_only_to` — must light `fact_bit::DISPLAY_ONLY`.
    /// Mirrors `satisfies_attrs(TOK_DISPLAY_ONLY)` in
    /// `crates/capco/src/scheme/predicates/satisfies.rs`. Closes the
    /// Copilot-flagged hole where the pre-#704 `MASK_FDR_DOMINATORS`
    /// would have
    /// missed a `DISPLAY ONLY USA GBR` portion (populated
    /// `display_only_to`, no `Displayonly` dissem variant) and
    /// allowed PR-C's closure to spuriously imply NOFORN/RELIDO.
    #[test]
    fn display_only_paths_both_set_display_only_bit() {
        // Path 1: dissem-axis Displayonly token.
        let mut attrs1 = empty();
        attrs1.dissem_us = vec![DissemControl::Displayonly].into();
        assert!(
            derive_bits(&attrs1).is_set(fact_bit::DISPLAY_ONLY),
            "dissem-axis Displayonly must set DISPLAY_ONLY bit",
        );

        // Path 2: country-list axis only (canonical `DISPLAY ONLY [LIST]`).
        let mut attrs2 = empty();
        attrs2.display_only_to = vec![CountryCode::USA, CountryCode::GBR].into();
        assert!(
            derive_bits(&attrs2).is_set(fact_bit::DISPLAY_ONLY),
            "non-empty display_only_to must set DISPLAY_ONLY bit",
        );

        // Path 3: both axes populated simultaneously.
        let mut attrs3 = empty();
        attrs3.dissem_us = vec![DissemControl::Displayonly].into();
        attrs3.display_only_to = vec![CountryCode::GBR].into();
        assert!(derive_bits(&attrs3).is_set(fact_bit::DISPLAY_ONLY));

        // Negative: empty `display_only_to` + no Displayonly token =>
        // DISPLAY_ONLY bit MUST stay zero.
        let attrs4 = empty();
        assert!(
            !derive_bits(&attrs4).is_set(fact_bit::DISPLAY_ONLY),
            "empty attrs must not set DISPLAY_ONLY bit",
        );
    }

    /// Both FGI projection paths — classification-axis `Fgi(_)` and
    /// dissem-axis `fgi_marker.is_some()` — must light the
    /// `FGI_PRESENT` sentinel bit. Catches the drift class where a
    /// future `FgiMarker` variant or a new `MarkingClassification`
    /// FGI form is added without an `is_set(FGI_PRESENT)` update.
    /// `MASK_FDR_OR_RELIDO_INCOMPAT`'s correctness depends on this
    /// (the bitmask collapses three separate `TokenRef` entries in
    /// `closure.rs::FDR_OR_RELIDO_INCOMPAT` — `TOK_FGI_MARKER`,
    /// `AnyInCategory(CAT_FGI_MARKER)`, `TOK_FGI_CLASS` — into the
    /// single `FGI_PRESENT` bit).
    #[test]
    fn fgi_paths_both_set_fgi_present() {
        use marque_ism::{FgiClassification, FgiMarker};

        // Path 1: dissem-axis FgiMarker::SourceConcealed.
        let mut attrs1 = empty();
        attrs1.fgi_marker = Some(FgiMarker::SourceConcealed);
        assert!(
            derive_bits(&attrs1).is_set(fact_bit::FGI_PRESENT),
            "fgi_marker::SourceConcealed must set FGI_PRESENT",
        );

        // Path 1b: dissem-axis FgiMarker::Acknowledged with a country list.
        let mut attrs1b = empty();
        attrs1b.fgi_marker = Some(
            FgiMarker::acknowledged(vec![CountryCode::GBR])
                .expect("constructor accepts non-empty country list"),
        );
        assert!(
            derive_bits(&attrs1b).is_set(fact_bit::FGI_PRESENT),
            "fgi_marker::Acknowledged must set FGI_PRESENT",
        );

        // Path 2: classification-axis MarkingClassification::Fgi(_)
        // (source-acknowledged form with a country list).
        let mut attrs2 = empty();
        attrs2.classification = Some(MarkingClassification::Fgi(FgiClassification {
            countries: vec![CountryCode::GBR].into_boxed_slice(),
            level: Classification::Secret,
        }));
        assert!(
            derive_bits(&attrs2).is_set(fact_bit::FGI_PRESENT),
            "MarkingClassification::Fgi must set FGI_PRESENT",
        );

        // Path 2b: classification-axis source-concealed FGI
        // (empty `countries` per the `//FGI S//...` form).
        let mut attrs2b = empty();
        attrs2b.classification = Some(MarkingClassification::Fgi(FgiClassification {
            countries: Box::new([]),
            level: Classification::Secret,
        }));
        assert!(
            derive_bits(&attrs2b).is_set(fact_bit::FGI_PRESENT),
            "source-concealed MarkingClassification::Fgi must set FGI_PRESENT",
        );
    }

    #[test]
    fn atom_inventory_fits_in_bitmask_width() {
        const _: () = assert!(CAPCO_ATOM_COUNT <= marque_scheme::FACT_BITMASK_WIDTH);
    }
}
