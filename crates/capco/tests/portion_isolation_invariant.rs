// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Portion isolation invariant — a portion's *marking decisions* are a
//! pure function of that portion, independent of its neighbors.
//!
//! # The invariant
//!
//! In CAPCO the portion is the atomic unit of classification: a
//! portion's marking reflects *its own* content. Every cross-portion
//! effect in the grammar flows *upward* into the page roll-up (the
//! banner / document level) — never *sideways* into a sibling portion.
//! Aggregation/compilation raises the container; it does not re-mark a
//! peer. Derivative carry-forward is a source→derivative flow, not a
//! portion→portion one. There is no CAPCO construction in which portion
//! B's marking should change because of portion A's marking.
//!
//! This test pins that property as a permanent canary: **the set of fix
//! proposals the engine attaches to a portion is identical whether the
//! portion is linted alone or embedded among arbitrary neighbors.**
//!
//! # Why *fixes*, not diagnostics
//!
//! Page context legitimately reaches some rules — but only to inform
//! *advisory diagnostics*, never to change a marking:
//!
//! - **Tier A (permitted):** a portion rule may read `ctx.page_marking`
//!   to surface a likely human error (a portion marked below the banner
//!   roll-up, an apparently-unmarked paragraph, citation drift). These
//!   emit a *diagnostic* with no fix; they do not alter the portion's
//!   marking. A diagnostic's neighbor-dependence is fine.
//! - **Tier B (out of scope):** the probabilistic decoder may use
//!   neighbor evidence to *recognize* an ambiguous span. That is a
//!   recognition-layer concern upstream of resolution; the strict
//!   recognizer is zero-context, and this test exercises resolution.
//! - **Tier C (forbidden):** page context *changing a portion's
//!   marking* — silent inheritance of the document classification, a
//!   caveat propagating from a sibling. CAPCO does not permit this;
//!   absence of a portion mark is a finding, not license to infer one.
//!
//! Keying the canary on **fix proposals** (the marking changes) rather
//! than on all diagnostics is what lets it bless Tier A while trapping
//! Tier C. A future Tier-A feature that plumbs read-only page context to
//! portion rules for richer diagnostics will not trip this test; a
//! feature that makes a portion's *fix* depend on its neighbors will.
//!
//! This deliberately tests the *property*, not the *mechanism*. The
//! engine achieves isolation today by handing portion candidates
//! `page_marking = None` / `page_portions = None`
//! (`crates/engine/src/engine/lint_helpers.rs`), and `fr048_bare_nato_
//! rel_to.rs` carries a hand-written trip-wire comment anticipating the
//! day that changes. A mechanism assertion ("portions get `None`") would
//! false-positive that legitimate Tier-A evolution. This behavioral
//! canary survives it and forbids only the thing CAPCO actually forbids.
//!
//! # What it compares
//!
//! For a portion `P`, the comparison key per fix is
//! `(rule_id, span-relative-to-P, replacement-repr)`:
//! - `rule_id` — the 2-tuple wire string (`RuleId: Display`).
//! - span normalized to the portion start, so the same fix at a
//!   different document offset compares equal.
//! - replacement — the `TextCorrection` canonical bytes, or the
//!   content-ignorant `Debug` of the structural `ReplacementIntent`
//!   (fact add/remove/recanonicalize over CVE token / category IDs).
//!
//! Diagnostics whose `span` falls outside `P` (the banner roll-up,
//! page-finalization rules) are filtered out by construction — they live
//! at the banner span and *should* shift with neighbors. Only fixes
//! attributed to `P`'s own bytes are compared.
//!
//! The comparison is further scoped to `capco:`-scheme fixes — CAPCO
//! marking-resolution decisions. Engine-synthesized sentinels
//! (`engine:recognition.decoder-recognized`, `engine:fix.reparse-
//! failed`) are the Tier B recognition layer and are excluded, so a
//! future neighbor-evidence decoder does not trip a resolution canary.
//!
//! # Documented exceptions
//!
//! Releasability (the dissem axis) can legitimately reflect document
//! context where the grammar says so. Such rules are listed in
//! [`EXEMPT_RULES`] and skipped:
//!
//! - `capco:portion.nato.bare-nato-requires-rel-to-usa-nato` (S007 /
//!   FR-048). A bare-NATO portion's `REL TO USA, NATO` suggestion is
//!   meant to be suppressed in a *solely-NATO* document — a property of
//!   the document aggregate, not the portion alone. Today S007 fires
//!   conservatively (the engine hands portion rules `page_marking =
//!   None`), so it stays neighbor-independent, `EXEMPT_RULES` is empty,
//!   and the seed set carries no bare-NATO portion. If a future
//!   migration makes S007's fix document-context-dependent, add its
//!   rule id to [`EXEMPT_RULES`] — see the migration trip-wire in
//!   `fr048_bare_nato_rel_to.rs`. The alternative this canary nudges
//!   toward (RFC #799's scope hierarchy) is to resolve solely-NATO
//!   suppression at document scope, so no portion fix becomes
//!   neighbor-dependent in the first place.

use std::collections::BTreeSet;

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use proptest::prelude::*;

/// One representative portion per CAPCO axis, drawn from the distinct
/// markings in `tests/fixtures/decision-tracing/cascade-demo.txt` (the
/// fixture we know exercises every interesting resolution path:
/// intra-portion default-fill, closure, AEA recanonicalization, the
/// UCNI page-rewrite chain, FGI naming, JOINT, SCI compartments).
///
/// A regression in any single axis trips the canary: if embedding a
/// portion alongside neighbors changes that portion's own fix
/// proposals, the property is broken for that axis.
const PORTIONS: &[&str] = &[
    "(U)",                    // unclassified baseline
    "(S//SI)",                // SCI → intra-portion RELIDO implication
    "(S//SI-G)",              // SCI compartment → closure cone
    "(S//HCS)",               // HCS system constraint
    "(S//ORCON)",             // caveated → NOFORN default-fill
    "(S//FGI GBR)",           // FGI naming (trigraph)
    "(S//FGI NATO)",          // FGI NATO (tetragraph)
    "(S//RD)",                // AEA RD → requires-NOFORN constraint
    "(S//RD/ATOMAL)",         // AEA ATOMAL
    "(S//CTS//ATOMAL)",       // NATO class + AEA ATOMAL recanonicalization
    "(JOINT TS USA GBR CAN)", // JOINT ownership list
    "(U//UCNI)", // UCNI — unclassified by definition (the valid form); zero portion fixes, guards against acquiring a neighbor-dependent one
    "(S//REL TO USA, GBR)", // REL TO
    "(S//SAR-BLACKLIGHT)", // SAR program
];

/// Rules whose portion fix is a *documented* exception to the isolation
/// invariant — the grammar intends their releasability to reflect
/// document context. Empty today: every shipped portion fix is
/// neighbor-independent. The S007 / FR-048 migration (see the module
/// docs and `fr048_bare_nato_rel_to.rs`) adds
/// `"capco:portion.nato.bare-nato-requires-rel-to-usa-nato"` here when
/// it makes that suggestion solely-NATO-context-dependent.
const EXEMPT_RULES: &[&str] = &[];

fn engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme constructs without rewrite cycles")
}

/// Content-ignorant key for a single fix proposal attributed to the
/// portion occupying `[portion_start, portion_start + portion_len)`.
/// Returns `None` for diagnostics outside that span or carrying no fix
/// (advisory diagnostics are permitted to depend on neighbors — Tier A).
fn fix_key(
    diag: &marque_rules::Diagnostic<CapcoScheme>,
    portion_start: usize,
    portion_len: usize,
) -> Option<String> {
    let portion_end = portion_start + portion_len;
    // Filter to fixes attributed to *this* portion's bytes. Banner /
    // page-finalization diagnostics live at the banner span and are
    // excluded by construction.
    if diag.span.start < portion_start || diag.span.start >= portion_end {
        return None;
    }
    // Scope the invariant to CAPCO marking-*resolution* fixes. Engine-
    // synthesized sentinels (`engine:recognition.decoder-recognized`,
    // `engine:fix.reparse-failed`) are recognition-layer / fix-path
    // artifacts — the Tier B boundary called out in the module docs.
    // The decoder is per-candidate today, but neighbor-evidence
    // recognition is a blessed future; excluding it keeps this canary
    // about resolution isolation and prevents a Tier-B feature from
    // tripping a resolution test.
    let rule_str = diag.rule.to_string();
    if !rule_str.starts_with("capco:") || EXEMPT_RULES.contains(&rule_str.as_str()) {
        return None;
    }
    let rel_start = diag.span.start - portion_start;
    // The `Span` invariant (`end >= start >= portion_start`, the latter
    // just checked) means this never underflows; `saturating_sub` is
    // defensive only. Do NOT mirror it onto `rel_start` — a saturating
    // `rel_start` could silently collapse distinct spans to offset 0 and
    // mask a real difference (false negative).
    let rel_end = diag.span.end.saturating_sub(portion_start);

    // The replacement repr — text-correction bytes (canonical tokens,
    // never document content) or the structural intent's content-
    // ignorant Debug. A diagnostic with neither is advisory: not a
    // marking decision, so not part of the invariant.
    let repl = if let Some(tc) = diag.text_correction.as_ref() {
        format!("text:{}", tc.replacement)
    } else if let Some(fix) = diag.fix.as_ref() {
        format!("intent:{:?}", fix.replacement)
    } else {
        return None;
    };

    Some(format!("{rule_str}|{rel_start}..{rel_end}|{repl}"))
}

/// The set of fix-proposal keys the engine attaches to the portion at
/// `[portion_start, portion_start + portion_len)` when linting `doc`.
fn fix_keys(doc: &[u8], portion_start: usize, portion_len: usize) -> BTreeSet<String> {
    engine()
        .lint(doc)
        .diagnostics
        .iter()
        .filter_map(|d| fix_key(d, portion_start, portion_len))
        .collect()
}

/// Fix keys for a portion linted entirely on its own (offset 0).
fn fix_keys_alone(portion: &str) -> BTreeSet<String> {
    let doc = format!("{portion}\n");
    fix_keys(doc.as_bytes(), 0, portion.len())
}

/// Build a single-page document of `lines` (each portion on its own
/// line) and return `(document_bytes, byte_offset_of_line[target])`.
fn embed(lines: &[&str], target: usize) -> (Vec<u8>, usize) {
    let mut doc = String::new();
    let mut target_start = 0;
    for (i, line) in lines.iter().enumerate() {
        if i == target {
            target_start = doc.len();
        }
        doc.push_str(line);
        doc.push('\n');
    }
    (doc.into_bytes(), target_start)
}

/// Deterministic baseline: every seed portion, alone vs. embedded in the
/// full seed set (maximum neighbor interaction on one page). A failure
/// here points at a specific axis without proptest shrinking noise.
#[test]
fn every_portion_is_neighbor_independent_in_full_set() {
    for (idx, &portion) in PORTIONS.iter().enumerate() {
        let alone = fix_keys_alone(portion);

        // Embed P among ALL other seed portions on one shared page.
        let (doc, start) = embed(PORTIONS, idx);
        let embedded = fix_keys(&doc, start, portion.len());

        assert_eq!(
            alone, embedded,
            "portion {portion:?} produced different fixes alone vs. embedded \
             in the full seed set — a neighbor influenced this portion's \
             marking (Tier C violation). alone={alone:?} embedded={embedded:?}"
        );
    }
}

proptest! {
    // Each case lints two small single-page documents; 512 cases is
    // cheap and searches hard for the neighbor arrangement that breaks
    // isolation.
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Property: for any portion `P` and any arbitrary multiset of
    /// neighbor portions placed at any position around it on a shared
    /// page, `P`'s own fix proposals are identical to `P` linted alone.
    #[test]
    fn portion_fixes_are_invariant_to_arbitrary_neighbors(
        target in 0usize..PORTIONS.len(),
        neighbor_idxs in prop::collection::vec(0usize..PORTIONS.len(), 0..=6),
        insert_pos in 0usize..=6,
    ) {
        let portion = PORTIONS[target];

        // Interleave: neighbors with P inserted at a clamped position.
        let mut lines: Vec<&str> = neighbor_idxs.iter().map(|&i| PORTIONS[i]).collect();
        let pos = insert_pos.min(lines.len());
        lines.insert(pos, portion);

        let (doc, start) = embed(&lines, pos);
        let embedded = fix_keys(&doc, start, portion.len());
        let alone = fix_keys_alone(portion);

        prop_assert_eq!(
            &alone, &embedded,
            "portion {:?} fixes changed under neighbors {:?} (P at line {}): \
             a sibling influenced this portion's marking (Tier C). \
             alone={:?} embedded={:?}",
            portion, lines, pos, alone, embedded
        );
    }
}
