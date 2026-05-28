// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Aggregated views over a recorded decision stream.
//!
//! [`DecisionReport`] is the type [`crate::decision::sinks::CountingSink`]
//! and [`crate::decision::sinks::RecordingSink`] produce via their
//! `into_report` methods. It carries the per-kind / per-category /
//! per-portion tallies a counting sink already produces, plus the
//! cascade-chain reconstruction a recording sink derives by walking
//! [`crate::decision::DecisionEvent::triggered_by`] edges.

use std::collections::BTreeMap;

#[cfg(feature = "serde")]
use serde::Serialize;

use crate::category::CategoryId;
use crate::decision::{DecisionKind, DecisionSite};

/// Aggregated view over a recorded decision stream.
///
/// Produced by [`crate::decision::sinks::CountingSink::into_report`]
/// (without chain reconstruction) or
/// [`crate::decision::sinks::RecordingSink::into_report`] (with chain
/// reconstruction).
///
/// The breakdown maps use `BTreeMap` so iteration order is stable
/// (alphabetical by key, with [`CategoryId::MARKING`] sorting first
/// because it has the lowest numeric value). Stable order makes the
/// report directly usable in snapshot tests without a follow-up sort.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct DecisionReport {
    /// Total number of recorded events.
    pub total: u64,
    /// Per-[`CategoryId`] counts. Categories with zero events are
    /// omitted; [`CategoryId::MARKING`] (the multi-category sentinel)
    /// appears under its own key when whole-marking decisions were
    /// recorded.
    pub by_category: BTreeMap<CategoryId, u64>,
    /// Per-[`DecisionKind`] counts. Kinds with zero events are
    /// omitted.
    pub by_kind: BTreeMap<DecisionKind, u64>,
    /// Per-portion counts, indexed by portion number.
    /// `by_portion[i]` is the count of events whose site is
    /// `DecisionSite::Portion(i as u32)`. Portions never seen carry
    /// zero. Trailing zeros are not trimmed.
    pub by_portion: Vec<u64>,
    /// Reconstructed cascade chains rooted at events with
    /// `triggered_by == None`. Empty for reports produced by a
    /// counting sink.
    pub cascade_chains: Vec<CascadeChain>,
    /// Longest path from root to leaf across all cascade chains.
    /// Zero when no chain has any descendants (every event is a root).
    pub max_cascade_depth: u32,
}

/// One root-rooted cascade in a decision stream.
///
/// Produced by [`crate::decision::sinks::RecordingSink::into_report`].
/// The chain contains every event transitively triggered by
/// [`CascadeChain::root_event`]; the order is pre-order DFS with
/// children visited in ascending step order, which agrees with the
/// engine's chronological emit order when descendants don't reorder
/// against their siblings.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct CascadeChain {
    /// Step number of the root event (the cascade's `triggered_by`
    /// is `None`, or its parent step is not present in the recorded
    /// stream).
    pub root_event: u32,
    /// Site of the root event. Carried explicitly so reporting
    /// surfaces don't have to index back into the full event stream.
    pub root_site: DecisionSite,
    /// Steps of every event in the cascade, including the root,
    /// in pre-order DFS.
    pub events: Vec<u32>,
    /// Longest path from the root to a leaf in this chain. Zero when
    /// the root has no descendants.
    pub depth: u32,
}
