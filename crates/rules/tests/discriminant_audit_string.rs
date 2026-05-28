// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Wire-format pin for [`marque_rules::Discriminant`].
//!
//! `Discriminant::as_str()` returns the JSON wire string for the
//! `marque-3.0` audit record's `replacement.discriminant` field. A
//! silent rename of either arm — or an attribute change that swaps
//! the encoded form — breaks this test, forcing a coordinated
//! audit-schema bump per the closed-set discipline.
//!
//! Mirrors the pinning pattern used by
//! `crates/rules/tests/message_args_closed_set.rs` (closed-set
//! `MessageArgs` field pin).

use marque_rules::Discriminant;

#[test]
fn discriminant_strict_wire_form_pinned() {
    assert_eq!(Discriminant::Strict.as_str(), "strict");
}

#[test]
fn discriminant_decoder_wire_form_pinned() {
    assert_eq!(Discriminant::Decoder.as_str(), "decoder");
}

#[test]
fn discriminant_is_copy() {
    // Proves the `Copy` derive — required by audit-emit code that
    // moves `Discriminant` through dispatch helpers without
    // by-value-then-take patterns.
    let d: Discriminant = Discriminant::Strict;
    let _copy: Discriminant = d;
    let _again: Discriminant = d; // would fail to compile if Discriminant were move-only
}

#[test]
fn discriminant_eq_and_hash() {
    // Pin the `Eq` + `Hash` derives. Audit-emit dispatch tables and
    // tests may key on Discriminant; without `Eq` + `Hash` they'd
    // need a workaround.
    use std::collections::HashMap;
    let mut counts: HashMap<Discriminant, u32> = HashMap::new();
    *counts.entry(Discriminant::Strict).or_default() += 1;
    *counts.entry(Discriminant::Decoder).or_default() += 1;
    *counts.entry(Discriminant::Strict).or_default() += 1;
    assert_eq!(counts.len(), 2);
    assert_eq!(counts[&Discriminant::Strict], 2);
    assert_eq!(counts[&Discriminant::Decoder], 1);
}
