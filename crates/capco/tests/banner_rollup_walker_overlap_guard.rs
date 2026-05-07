// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! C-1 overlap-guard regression test for the banner-roll-up walker
//! (T026a, PR 3b Sub-move A).
//!
//! The walker's SAR row preserves the **diagnostic shape** that the
//! retired `SarBannerRollupRule` (E031) used to coexist with E028
//! (`sar-program-order`) under the FR-016 ordering / C-1 overlap
//! guard:
//!
//! - E028 emits a whole-block rewrite: `span = [block_start,
//!   block_end]`, replacement = sorted block.
//! - E031 emits a zero-width insertion at the SAR block's end:
//!   `span = [block_end, block_end]`, replacement = `/<missing>`.
//!
//! Both spans share `span.end = block_end`, so the FR-016 sort
//! `(span.end DESC, span.start DESC, ...)` deterministically orders
//! E031 before E028. The C-1 walk admits both: `next_window_end =
//! E031.span.start = block_end`, then E028's `span.end = block_end
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
//! diagnostic ahead of E028's. Without this property, a future
//! change that flipped the walker's SAR-row fix to a whole-block
//! rewrite would silently regress the (currently-suppressed-by-
//! the-engine-filter) overlap-guard contract — and we'd have lost
//! the structural shape that's the whole point of preserving E031
//! as a per-row catalog ID instead of folding it into a single
//! E028-style rewrite.

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

fn lint(source: &str) -> Vec<Diagnostic> {
    engine().lint(source.as_bytes()).diagnostics
}

#[test]
fn e031_walker_and_e028_emit_compatible_overlap_guard_shapes() {
    // Banner: `SAR-CD/AB` — programs out of order (E028 fires).
    // Portion adds `SAR-BB` — program missing from banner (E031
    // walker row fires).
    let source = "(S//SAR-BB//NF)\nSECRET//SAR-CD/AB//NOFORN";
    let diags = lint(source);

    let e028: Vec<&Diagnostic> = diags.iter().filter(|d| d.rule.as_str() == "E028").collect();
    let e031: Vec<&Diagnostic> = diags.iter().filter(|d| d.rule.as_str() == "E031").collect();

    assert_eq!(
        e028.len(),
        1,
        "E028 must fire on out-of-order programs CD/AB; full diags: \
         {diags:?}",
    );
    assert_eq!(
        e031.len(),
        1,
        "E031 walker row must fire on missing program BB; full diags: \
         {diags:?}",
    );

    // E028 fix shape — whole-block rewrite covering the SAR block.
    let e028_fix = e028[0].fix.as_ref().expect("E028 must carry a fix");
    let e028_block_start = e028_fix.span.start;
    let e028_block_end = e028_fix.span.end;
    assert!(
        e028_block_end > e028_block_start,
        "E028 must cover a non-empty span (whole-block rewrite); \
         got {e028_block_start}..{e028_block_end}",
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
        e031_fix.span.start, e028_block_end,
        "E031 walker insertion point must equal E028's block end — \
         that's the FR-016 boundary that lets both fixes coexist \
         under the C-1 overlap guard",
    );

    // E031 inserts at the end with leading `/`, per §H.5 p100 \
    // bullet 4 (slash-separated program list, no interjected \
    // spaces).
    assert!(
        e031_fix.replacement.starts_with('/'),
        "E031 insertion must lead with `/` to separate from the \
         preceding program; got: {:?}",
        e031_fix.replacement.as_ref(),
    );

    // FR-016 ordering: with same `span.end`, the higher `span.start`
    // sorts first. E031's span.start = e028_block_end >
    // E028's span.start = e028_block_start, so under FR-016 the
    // walker's E031 row is ordered before E028. The C-1 walk admits
    // E031 first (no boundary), then E028 because E028.span.end ≤
    // E031.span.start. Both fixes survive C-1.
    //
    // We assert this by constructing the fix list the way the
    // engine does, sorting it the FR-016 way, and walking C-1
    // manually — keeps the test independent of which engine entry
    // point a future caller uses (`fix`, `fix_with_threshold`,
    // `fix_with_options`).
    let mut fix_list = vec![e031_fix.clone(), e028_fix.clone()];
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
         before E028 (span.start = block_start) — required so \
         E031's `next_window_end = block_end` admits E028's \
         block-rewrite span (E028.span.end ≤ block_end)",
    );
    assert_eq!(fix_list[1].rule.as_str(), "E028");

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
        vec!["E031", "E028"],
        "C-1 walk must admit BOTH E031 (zero-width, admitted first) \
         and E028 (whole-block rewrite, admitted because its \
         span.end ≤ E031's span.start = block_end). If a future \
         change flips the walker's SAR-row fix to a whole-block \
         rewrite, the C-1 lexicographic tiebreaker would favor \
         E028 and silently drop E031 — exactly the regression this \
         test exists to prevent.",
    );
}
