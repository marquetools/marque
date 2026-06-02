// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Document-scope rollup accumulator (#799).
//!
//! These tests pin the engine's per-document `doc_join_acc`: a running
//! `S::Canonical` folded at every page boundary (and at end-of-document)
//! via [`MarkingScheme::canonical_document_join`]. The rollup is surfaced
//! as the third element of `lint_with_options_internal_with_source` so the
//! invariant tests can assert on it; it is not yet consumed by a
//! document-finalization dispatch.

use super::*;
use marque_ism::CanonicalAttrs;
use marque_ism::attrs::Classification;
use marque_scheme::FiringPredicate;
use marque_scheme::document_context::DocumentContext;

/// Build a strict-recognizer CAPCO engine with no extra rules — the
/// document rollup is independent of which rules fire, so a bare engine
/// keeps these tests focused on the accumulator.
fn doc_rollup_engine() -> CapcoEngine {
    Engine::with_clock(
        Config::default(),
        vec![],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_strict_recognizer()
}

/// Drive the engine over `src` (DocumentContent recognition) and return
/// just the document-scope rollup.
fn engine_doc_rollup(engine: &CapcoEngine, src: &[u8]) -> CanonicalAttrs {
    let (_, _, rollup) = engine.lint_with_options_internal_with_source(
        src,
        &LintOptions::default(),
        None,
        marque_scheme::InputSource::DocumentContent,
    );
    rollup
}

fn effective_level(a: &CanonicalAttrs) -> Option<Classification> {
    a.classification.as_ref().map(|c| c.effective_level())
}

/// Two sequential lint calls on distinct multi-page inputs must each see
/// only their own document. A fresh `doc_join_acc` per `lint_inner` call is
/// the Constitution VI fresh-per-input guarantee — no bleed from call 1
/// into call 2.
#[test]
fn fresh_document_rollup_per_input() {
    let engine = doc_rollup_engine();

    // Call A: a TOP SECRET document spanning two pages.
    let src_a = b"(TS) page one\n\x0c(TS) page two\n";
    let roll_a = engine_doc_rollup(&engine, src_a);
    assert_eq!(
        effective_level(&roll_a),
        Some(Classification::TopSecret),
        "call A rollup must reflect its TOP SECRET portions",
    );

    // Call B: a CONFIDENTIAL document. If the accumulator leaked across
    // calls it would still show TOP SECRET (OrdMax never lowers).
    let src_b = b"(C) page one\n\x0c(C) page two\n";
    let roll_b = engine_doc_rollup(&engine, src_b);
    assert_eq!(
        effective_level(&roll_b),
        Some(Classification::Confidential),
        "call B rollup must reflect ONLY call B (no bleed from call A)",
    );
    assert_ne!(
        roll_a, roll_b,
        "distinct inputs must yield distinct rollups; equality here means \
         the accumulator is not re-initialized per input",
    );
}

/// A malformed page-break region between two real pages must not block the
/// document fold. The page→document fold sits before the unconditional
/// per-page reset, so page N folds in even when the break candidate is
/// degenerate; page N+1 then starts clean and folds in at EOD. This is the
/// document-scope analogue of the `PageContext`-reset invariant
/// (Constitution VI).
#[test]
fn malformed_page_break_does_not_block_document_fold() {
    let engine = doc_rollup_engine();

    // Page N (SECRET portion), a form-feed adjacent to garbage bytes
    // (a degenerate page-break region), then page N+1 (CONFIDENTIAL
    // portion). The garbage is not a valid marking, so only the two real
    // portions contribute to the rollup.
    let src = b"(S) page one\n\x0c   ???   \n(C) page two\n";
    let roll = engine_doc_rollup(&engine, src);

    // The document rollup must absorb page N (SECRET) despite the
    // malformed break, and OrdMax across both pages yields SECRET.
    assert_eq!(
        effective_level(&roll),
        Some(Classification::Secret),
        "document rollup must fold page N (SECRET) past the malformed break \
         and roll up to the max across both pages",
    );

    // Cross-check: the rollup equals join(page_N, page_{N+1}) computed
    // independently via the reference batch fold.
    let scheme = marque_capco::scheme::CapcoScheme::new();
    let page_n = engine_page_rollup(&engine, b"(S) page one\n");
    let page_np1 = engine_page_rollup(&engine, b"(C) page two\n");
    let reference = DocumentContext::from_pages(&scheme, &[page_n, page_np1]).rollup;
    assert_eq!(
        roll, reference,
        "engine rollup must equal join(rollup_N, rollup_{{N+1}})",
    );
}

/// Drive the engine over a single-page `src` (no internal page break) and
/// return its document rollup — used as a per-page reference rollup for the
/// batch cross-check.
fn engine_page_rollup(engine: &CapcoEngine, src: &[u8]) -> CanonicalAttrs {
    engine_doc_rollup(engine, src)
}

/// The engine's incremental page→document fold must equal the batch
/// reference fold `DocumentContext::from_pages` over the same per-page
/// rollups. This catches a wiring bug where the engine folds in the wrong
/// order or drops a page, and empirically confirms the CAPCO lattice's
/// associativity/commutativity at document scope.
///
/// Classification roll-up authority: CAPCO-2016 §D.2 p28 ("Banner Line
/// 'Roll-Up' Rules" — the banner takes the highest classification level of
/// all portions). The cross-page max here exercises that rule at document
/// scope.
#[test]
fn incremental_doc_fold_matches_batch_from_pages() {
    let engine = doc_rollup_engine();
    let scheme = marque_capco::scheme::CapcoScheme::new();

    // Three pages with distinct classification levels split by form feeds.
    let page1: &[u8] = b"(C) page one\n";
    let page2: &[u8] = b"(TS) page two\n";
    let page3: &[u8] = b"(S) page three\n";

    let mut src = Vec::new();
    src.extend_from_slice(page1);
    src.push(0x0c);
    src.extend_from_slice(page2);
    src.push(0x0c);
    src.extend_from_slice(page3);

    let incremental = engine_doc_rollup(&engine, &src);

    // Independently collect each page's rollup and compute the reference
    // batch fold (C1's `from_pages`).
    let pages: Vec<CanonicalAttrs> = vec![
        engine_page_rollup(&engine, page1),
        engine_page_rollup(&engine, page2),
        engine_page_rollup(&engine, page3),
    ];
    let reference = DocumentContext::from_pages(&scheme, &pages).rollup;

    assert_eq!(
        incremental, reference,
        "engine incremental document fold must equal the batch from_pages fold",
    );
    // §D.2 p28: the document banner classification is the max across pages.
    assert_eq!(
        effective_level(&incremental),
        Some(Classification::TopSecret),
        "max classification across pages is TOP SECRET (§D.2 p28 roll-up)",
    );
}

/// A single-page document (no page-break candidate) folds only at EOD; the
/// document rollup equals that one page's rollup.
#[test]
fn single_page_document_yields_that_pages_rollup() {
    let engine = doc_rollup_engine();
    let scheme = marque_capco::scheme::CapcoScheme::new();

    let src = b"(S) the only page\n";
    let roll = engine_doc_rollup(&engine, src);

    let page = engine_page_rollup(&engine, src);
    let reference = DocumentContext::from_pages(&scheme, &[page]).rollup;
    assert_eq!(
        roll, reference,
        "single-page rollup must equal that page's rollup folded at EOD",
    );
    assert_eq!(
        effective_level(&roll),
        Some(Classification::Secret),
        "single SECRET page rolls up to SECRET",
    );
}

/// An empty document (no candidates, hence no portions) yields the
/// canonical bottom (`Default`) — the fresh accumulator, never folded.
#[test]
fn empty_document_yields_default_rollup() {
    let engine = doc_rollup_engine();

    let roll = engine_doc_rollup(&engine, b"");
    assert_eq!(
        roll,
        CanonicalAttrs::default(),
        "empty document rollup must be the canonical bottom (Default)",
    );

    // A document with only non-marking text also produces no portions, so
    // the EOD fold guard (`!page_portions.is_empty()`) never fires and the
    // rollup stays at Default.
    let roll_text = engine_doc_rollup(&engine, b"just some prose with no markings\n");
    assert_eq!(
        roll_text,
        CanonicalAttrs::default(),
        "marking-free document rollup must stay at the canonical bottom",
    );
}

// ---------------------------------------------------------------------------
// Mode placeholder + firing-predicate gating (#799).
// ---------------------------------------------------------------------------

/// A fresh engine has no active modes.
#[test]
fn active_modes_default_empty() {
    let engine = doc_rollup_engine();
    assert_eq!(engine.active_modes().count(), 0);
}

/// `firing_active(Always)` is always true; `firing_active(WhenMode(m))` is
/// false by default (no modes active), then true once `m` is activated.
/// The positive half is the engine-internal exercise of the mode-gated
/// firing path; it uses the crate-internal `set_active_modes_for_test`
/// setter (no public mode-setter ships, #799).
#[test]
fn firing_active_gates_when_mode_on_active_modes() {
    let mut engine = doc_rollup_engine();

    // Always fires regardless of active modes.
    assert!(engine.firing_active(FiringPredicate::Always));

    // WhenMode does not fire while its mode is inactive (the default).
    assert!(!engine.firing_active(FiringPredicate::WhenMode("derivative")));

    // Activate the mode; now the WhenMode edge fires.
    engine.set_active_modes_for_test(["derivative"]);
    assert!(engine.firing_active(FiringPredicate::WhenMode("derivative")));
    assert!(engine.active_modes().any(|m| m == "derivative"));

    // A different, still-inactive mode does not fire.
    assert!(!engine.firing_active(FiringPredicate::WhenMode("other")));
}

// ---------------------------------------------------------------------------
// CAPCO document-scope resolution is an empty no-op (#799 regression).
// ---------------------------------------------------------------------------

/// CAPCO declares no document artifacts, so `resolve_document` is an
/// empty-slice no-op and a CAPCO `lint()` carries an empty resolution.
/// This is the positive regression assertion that C4 leaks no resolution
/// behavior into the CAPCO path.
#[test]
fn capco_produces_empty_resolved_document() {
    let engine = doc_rollup_engine();

    // Direct: resolving any rollup yields the empty document.
    let rollup = engine_doc_rollup(&engine, b"(S) page one\n");
    let resolved = engine.resolve_document(&rollup);
    assert!(
        resolved.is_empty(),
        "CAPCO declares no document artifacts; resolution must be empty",
    );

    // Through the lint pipeline: a completed CAPCO lint surfaces the empty
    // resolution on its LintResult.
    let result = engine.lint(b"(S) page one\n");
    assert!(
        result.resolved_document.is_empty(),
        "CAPCO lint must carry an empty resolved_document",
    );
}
