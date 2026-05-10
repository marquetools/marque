// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! C-1 overlap-guard regression test for the banner-roll-up walker
//! (T026a, PR 3b Sub-move A) and its interaction with the
//! non-canonical input walker's SAR program ascending row (PR 3b.F /
//! T026f, rule ID `E060`, retired predecessor `E028`).
//!
//! The walker's SAR row preserves the **diagnostic shape** that the
//! retired `SarBannerRollupRule` (E031) used to coexist with the
//! retired `SarProgramOrderRule` (E028 → now E060 SAR row) under the
//! FR-016 ordering / C-1 overlap guard:
//!
//! - E060 (SAR row) emits a whole-block rewrite: `span = [block_start,
//!   block_end]`, replacement = sorted block.
//! - E031 emits a zero-width insertion at the SAR block's end:
//!   `span = [block_end, block_end]`, replacement = `/<missing>`.
//!
//! Both spans share `span.end = block_end`, so the FR-016 sort
//! `(span.end DESC, span.start DESC, rule_id ASC, ...)`
//! deterministically orders E031 before E060 (`'E031' < 'E060'` lex
//! since `'3' < '6'`). The C-1 walk admits both: `next_window_end =
//! E031.span.start = block_end`, then E060's `span.end = block_end
//! ≤ block_end = boundary` — both kept by C-1.
//!
//! Engine apply caveat (pre-existing, inherited unchanged): the
//! engine's fix-collection filter at `crates/engine/src/engine.rs`
//! `.filter(|f| !f.span.is_empty())` drops zero-width-span fixes
//! before the FR-016 sort runs, so in practice E031's insertion
//! does not reach the apply pipeline. This is an engine-level
//! limitation that pre-dates the walker; the walker neither
//! introduces it nor fixes it. (A future PR that removes the
//! `is_empty()` filter — or that changes the walker's SAR-row fix
//! shape — would let both fixes actually apply, at which point the
//! "≤ 2 fix passes converges" property documented in the walker's
//! evaluator comments becomes observable end-to-end.)
//!
//! What this test pins, therefore, is the **lint-level contract**:
//! the walker emits both diagnostics with the documented span
//! shapes, the C-1 walk admits both, and FR-016 puts the SAR row's
//! diagnostic ahead of E060 (SAR row)'s. Without this property, a
//! future change that flipped the walker's SAR-row fix to a
//! whole-block rewrite would silently regress the (currently-
//! suppressed-by-the-engine-filter) overlap-guard contract — and
//! we'd have lost the structural shape that's the whole point of
//! preserving E031 as a per-row catalog ID instead of folding it
//! into a single block-rewrite-style rule.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Diagnostic;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Filter on E060 + the SAR-program message text so we don't pick up
/// the unrelated REL TO / JOINT / SIGMA / SCI rows of the same
/// non-canonical input walker.
fn e060_sar_program_diags(diags: &[Diagnostic<CapcoScheme>]) -> Vec<&Diagnostic<CapcoScheme>> {
    diags
        .iter()
        .filter(|d| d.rule.as_str() == "E060" && d.message.starts_with("SAR programs"))
        .collect()
}

#[test]
fn e031_walker_and_e060_emit_compatible_overlap_guard_shapes() {
    // Banner: `SAR-CD/AB` — programs out of order (E060 SAR row
    // fires; pre-PR-3b.F was E028). Portion adds `SAR-BB` — program
    // missing from banner (E031 walker row fires).
    let source = "(S//SAR-BB//NF)\nSECRET//SAR-CD/AB//NOFORN";
    let diags = lint(source);

    let e060_sar = e060_sar_program_diags(&diags);
    let e031: Vec<&Diagnostic<CapcoScheme>> =
        diags.iter().filter(|d| d.rule.as_str() == "E031").collect();

    assert_eq!(
        e060_sar.len(),
        1,
        "E060 (SAR row) must fire on out-of-order programs CD/AB; \
         full diags: {diags:?}",
    );
    assert_eq!(
        e031.len(),
        1,
        "E031 walker row must fire on missing program BB; full diags: \
         {diags:?}",
    );

    // E060 SAR-row fix shape — whole-block rewrite covering the SAR
    // block (byte-identical to retired E028 fix shape).
    let e060_fix = e060_sar[0]
        .fix
        .as_ref()
        .expect("E060 (SAR row) must carry a fix");
    let e060_block_start = e060_fix.span.start;
    let e060_block_end = e060_fix.span.end;
    assert!(
        e060_block_end > e060_block_start,
        "E060 (SAR row) must cover a non-empty span (whole-block rewrite); \
         got {e060_block_start}..{e060_block_end}",
    );

    // E031 walker fix shape — zero-width insertion at SAR block end.
    let e031_fix = e031[0]
        .fix
        .as_ref()
        .expect("E031 walker row must carry a fix");
    assert_eq!(
        e031_fix.span.start, e031_fix.span.end,
        "E031 walker fix must be a zero-width insertion (FR-016 / \
         C-1 overlap-guard contract preserved by the walker)",
    );
    assert_eq!(
        e031_fix.span.start, e060_block_end,
        "E031 walker insertion point must equal E060 (SAR row)'s \
         block end — that's the FR-016 boundary that lets both \
         fixes coexist under the C-1 overlap guard",
    );

    // E031 inserts at the end with leading `/`, per §H.5 p100
    // bullet 4 (slash-separated program list, no interjected
    // spaces).
    assert!(
        e031_fix.replacement.starts_with('/'),
        "E031 insertion must lead with `/` to separate from the \
         preceding program; got: {:?}",
        e031_fix.replacement.as_ref(),
    );

    // FR-016 ordering: with same `span.end`, the higher `span.start`
    // sorts first. E031's span.start = e060_block_end >
    // E060's span.start = e060_block_start, so under FR-016 the
    // walker's E031 row is ordered before E060 (SAR row). The C-1
    // walk admits E031 first (no boundary), then E060 because
    // E060.span.end ≤ E031.span.start. Both fixes survive C-1.
    //
    // R-1 lex-tiebreaker review (PR 3b.F): pre-rename, the rule-id
    // tiebreaker resolved `E028 < E031` (`'2' < '3'`) — which this
    // test inverted by relying on `span.start` rather than `rule_id`
    // to break the tie. Post-rename, the tiebreaker resolves
    // `E031 < E060` (`'3' < '6'`). The previous tiebreaker order
    // (`E031` first, then `E028`) is preserved verbatim under the
    // rename (`E031` first, then `E060`), since `span.start` still
    // dominates the tiebreaker in this fixture.
    //
    // We assert this by constructing the fix list the way the
    // engine does, sorting it the FR-016 way, and walking C-1
    // manually — keeps the test independent of which engine entry
    // point a future caller uses (`fix`, `fix_with_threshold`,
    // `fix_with_options`).
    let mut fix_list = vec![e031_fix.clone(), e060_fix.clone()];
    fix_list.sort_by(|a, b| {
        b.span
            .end
            .cmp(&a.span.end)
            .then(b.span.start.cmp(&a.span.start))
            .then(a.rule.cmp(&b.rule))
            .then(a.replacement.cmp(&b.replacement))
    });
    assert_eq!(
        fix_list[0].rule.as_str(),
        "E031",
        "FR-016 must order E031 (walker, span.start = block_end) \
         before E060 SAR row (span.start = block_start) — required \
         so E031's `next_window_end = block_end` admits E060's \
         block-rewrite span (E060.span.end ≤ block_end)",
    );
    assert_eq!(fix_list[1].rule.as_str(), "E060");

    // C-1 walk simulation.
    let mut next_window_end: Option<usize> = None;
    let mut admitted_rule_ids: Vec<&str> = Vec::new();
    for fix in &fix_list {
        let fits = match next_window_end {
            Some(boundary) => fix.span.end <= boundary,
            None => true,
        };
        if fits {
            next_window_end = Some(fix.span.start);
            admitted_rule_ids.push(fix.rule.as_str());
        }
    }
    assert_eq!(
        admitted_rule_ids,
        vec!["E031", "E060"],
        "C-1 walk must admit BOTH E031 (zero-width, admitted first) \
         and E060 (whole-block rewrite, admitted because its \
         span.end ≤ E031's span.start = block_end). If a future \
         change flips the walker's SAR-row fix to a whole-block \
         rewrite, the C-1 lexicographic tiebreaker would favor \
         E060 and silently drop E031 — exactly the regression this \
         test exists to prevent.",
    );
}
