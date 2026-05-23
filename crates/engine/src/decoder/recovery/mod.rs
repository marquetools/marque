//! Decoder recovery pipeline — structural repair passes.
//!
//! Each sub-module owns one recovery pass. `candidates.rs` invokes
//! them in series via the `pub(in crate::decoder) use` re-exports below;
//! the per-pass entry points stay scoped to the decoder module tree.
//!
//! # Visibility note
//!
//! The re-exports below use `pub(in crate::decoder)` rather than
//! `pub(super)`. The consumer (`decoder/candidates.rs`) lives at the
//! `decoder/` scope, not under `recovery/`, so the re-exported items
//! must be reachable from `decoder/` itself. `pub(super)` from
//! `recovery/X.rs` would only reach `recovery/mod.rs` — the chain
//! stops there and `candidates.rs` cannot see the symbol. A future
//! contributor "tightening" these to `pub(super)` will break the
//! build; the precise-scope form is the minimum visibility that
//! satisfies the re-export.

// Sub-modules and their re-exported entry points sit at
// `pub(in crate::decoder)` visibility — wide enough that `candidates.rs`
// and the per-sub-file `mod tests` blocks can reach them through the
// `recovery::` re-export, narrow enough that no caller outside `decoder/`
// can.
pub(in crate::decoder) mod delimiter;
pub(in crate::decoder) mod nato;
pub(in crate::decoder) mod rel_to;
pub(in crate::decoder) mod rel_to_trigraph;
pub(in crate::decoder) mod reorder;
pub(in crate::decoder) mod sar;
pub(in crate::decoder) mod sci;
pub(in crate::decoder) mod stray;

pub(in crate::decoder) use delimiter::try_insert_delimiter;
pub(in crate::decoder) use nato::try_nato_fold;
pub(in crate::decoder) use rel_to::try_rel_to_structural_repair;
pub(in crate::decoder) use rel_to_trigraph::{
    try_rel_to_fuzzy_trigraph_candidates, try_rel_to_usa_injection_candidates,
};
pub(in crate::decoder) use reorder::{
    meets_classification_floor, try_add_non_us_prefix, try_canonical_reorder,
};
pub(in crate::decoder) use sar::try_sar_indicator_repair;
pub(in crate::decoder) use sci::try_sci_delimiter_repair;
pub(in crate::decoder) use stray::try_collapse_stray_char_slash;
