// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` predicate helpers.
//!
//! Covers presence predicates, satisfaction evaluators, class-floor and
//! SCI-per-system catalog dispatchers, and FD&R-family membership helpers.
//!
//! Public-API names resolve at `super::predicates::NAME` via the
//! re-exports below; sub-module boundaries are an internal
//! organization detail.
//! Only names that are referenced from outside the `predicates/`
//! directory are re-exported here — sub-modules that need each other
//! import directly via `super::<sub>::NAME` (e.g.
//! `super::class_floor::is_class_floor_catalog_name`).

mod class_floor;
mod dissem;
mod families;
mod joint_hcs;
mod presence;
mod satisfies;
mod sci_per_system;
mod spans;
mod tier1_mask;
// PR-G (#650 tier-2): class-floor bitmask dispatch helpers.
// Consumed by `class_floor::class_floor_catalog_eval` via the local path
// `super::tier2_mask::*`; no re-export at this surface — the mask helpers
// are an internal optimization detail, not part of the predicates-leaf
// public-via-`pub(crate)` API.
mod tier2_mask;
mod token_routing;
mod triggers;

// `pub` re-exports — the two true public-API names per the original
// (pre-split) `scheme.rs` `pub use self::predicates::{is_fdr_dominator, is_orcon_family}`.
pub use self::families::{is_fdr_dominator, is_orcon_family};

// `pub(crate)` re-exports — surface every helper at the historical
// `super::predicates::NAME` path so cross-module imports in
// `scheme/mod.rs`, `scheme/tests.rs`, and the other leaves continue
// to compile unchanged.
pub(crate) use self::class_floor::{class_floor_anchor_span, class_floor_satisfied};
pub(crate) use self::dissem::{
    dissem_family_of, fouo_with_non_fdr_other_control_trigger, rel_to_covers,
};
pub(crate) use self::presence::{
    presence_atomal, presence_balk, presence_bohemia, presence_dod_ucni, presence_doe_ucni,
    presence_eyes_only, presence_frd_bare, presence_frd_sigma, presence_hcs_comp_only,
    presence_hcs_comp_sub, presence_imcon, presence_orcon_family, presence_passthrough_bur,
    presence_passthrough_hcs_x, presence_passthrough_klm, presence_passthrough_mvl,
    presence_rd_bare, presence_rd_cnwdi, presence_rd_sigma, presence_rsen, presence_rsv_comp,
    presence_sar, presence_si_bare, presence_si_comp, presence_tfni, presence_tk_blfh,
    presence_tk_family,
};
pub(crate) use self::satisfies::{
    collect_present_tokens, evaluate_custom_by_attrs, satisfies_attrs,
};
pub(crate) use self::sci_per_system::{
    presence_hcs_o, presence_hcs_p_any, presence_hcs_p_sub, presence_si_g,
    presence_tk_compartment_noforn,
};
pub(crate) use self::spans::{
    dissem_token_id_for_form, dissem_token_span, first_sci_span, infer_companion_form,
    token_span_attrs, us_level,
};
// PR-E (#371) tier-1 mask-compiled predicates. Consumed by
// `super::satisfies::evaluate_custom_by_attrs` via the local path
// `super::tier1_mask::*`; no re-export at this surface — the mask
// predicates are an internal optimization detail, not part of the
// predicates-leaf public-via-`pub(crate)` API.
pub(crate) use self::token_routing::{capco_token_category, never_fires};
pub(crate) use self::triggers::{
    dod_ucni_classified_trigger, dod_ucni_promotes_noforn_trigger, doe_ucni_classified_trigger,
    doe_ucni_promotes_noforn_trigger, fouo_classified_trigger, les_nf_classified_trigger,
    limdis_classified_trigger, sbu_classified_trigger, sbu_nf_classified_trigger,
};
