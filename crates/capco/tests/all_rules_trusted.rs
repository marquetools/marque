// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Drift gate: every registered CAPCO rule must declare `trusted() == true`.
//!
//! `Rule::trusted()` defaults to `false`. A `false`-trusted rule runs inside
//! `std::panic::catch_unwind` in the engine's hot loop; a `true`-trusted
//! rule bypasses the wrapper for the perf win that motivates issue #436.
//! Every in-tree CAPCO rule was audited for panic-safety and statelessness
//! as part of the catalog and overrides `trusted()` to `true`.
//!
//! This test fails if a future contributor adds a new rule without making
//! the deliberate trust decision — forcing them to either audit the rule
//! and override, or leave it at the safe `false` default and explain why
//! the new rule is exempt from the catalog-wide audit (in which case this
//! test needs an explicit allowlist update with rationale).
//!
//! See `Rule::trusted()` in `crates/rules/src/lib.rs` for the contract.

use marque_capco::CapcoRuleSet;
use marque_rules::RuleSet;

#[test]
fn every_registered_capco_rule_is_trusted() {
    let rule_set = CapcoRuleSet::new();
    let untrusted: Vec<&str> = rule_set
        .rules()
        .iter()
        .filter(|r| !r.trusted())
        .map(|r| r.id().predicate_id())
        .collect();

    assert!(
        untrusted.is_empty(),
        "Every CAPCO rule must override `trusted()` to `true` after audit. \
         The following registered rules still report the safe-by-default \
         `false`: {untrusted:?}. Either add `fn trusted(&self) -> bool {{ true }}` \
         to each `impl Rule<CapcoScheme>` block, or document an explicit \
         allowlist exception in this test with audit rationale."
    );
}
