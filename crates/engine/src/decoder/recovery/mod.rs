//! Decoder recovery pipeline — structural repair passes.
//!
//! Each sub-module owns one recovery pass. `candidates.rs` invokes
//! them in series via the `pub(super) use` re-exports below; the
//! per-pass entry points stay scoped to the decoder module tree.

mod delimiter;
mod nato;
mod rel_to;
mod reorder;
mod sar;
mod sci;
mod stray;

pub(super) use delimiter::try_insert_delimiter;
pub(super) use nato::try_nato_fold;
pub(super) use rel_to::{
    try_rel_to_fuzzy_trigraph_candidates, try_rel_to_structural_repair,
    try_rel_to_usa_injection_candidates,
};
pub(super) use reorder::{meets_classification_floor, try_add_non_us_prefix, try_canonical_reorder};
pub(super) use sar::try_sar_indicator_repair;
pub(super) use sci::try_sci_delimiter_repair;
pub(super) use stray::try_collapse_stray_char_slash;
