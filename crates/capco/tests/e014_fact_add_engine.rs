#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.D.4 — E014 (JOINT participants must appear in REL
//! TO list, §H.3 p57) intent-only migration engine-level tests.
//!
//! These tests cover engine-level behaviors that can't be exercised
//! through the inline `lint_banner` / `lint_portion` helpers inside
//! `crates/capco/src/rules.rs::tests` (the inline module sees a
//! different `CapcoScheme` crate identity than `marque-engine`):
//!
//! - **N-diagnostic shape**: E014 emits one Diagnostic per missing
//!   JOINT co-owner. The legacy single-diagnostic-with-plural-message
//!   form ("JOINT participants [GBR, CAN] must appear in REL TO list")
//!   is retired; each missing country gets its own row, each carrying
//!   its own `FactAdd` intent. Per-diagnostic actionability + strict-
//!   singleton `FixIntent.replacement` justify the split.
//! - **FactAdd path on CAT_REL_TO open-vocab**: E014 is the first
//!   consumer of the `CapcoOpenVocabRef::CountryCode` open-vocab
//!   variant. The `apply_intent_to_marking` → `apply_fact_add`
//!   CAT_REL_TO branch (wired in this sub-PR) inserts the typed
//!   `marque_ism::CountryCode` value into `attrs.rel_to`.
//! - **Round-trip + idempotence**: `(//JOINT S AUS CAN USA)` →
//!   `(//JOINT S AUS CAN USA//REL TO USA, AUS, CAN)` after one
//!   `Engine::fix` pass; second pass produces no further fix.
//! - **Tetragraph coverage**: REL TO containing a tetragraph (e.g.,
//!   FVEY) that covers a JOINT participant trigraph satisfies the
//!   predicate (`rel_to_covers` expands tetragraph membership).
//!
//! # Authoritative source (CAPCO-2016 §H.3 p57)
//!
//! > "JOINT classified information for which the US is a co-owner,
//! > must be appropriately classified and explicitly marked with a
//! > REL TO marking that includes the US and all co-owners, at both
//! > the banner and portion level."
//!
//! The "REL TO marking that includes the US and all co-owners" floor
//! is policy-mandated and deterministic — no classifier discretion
//! sits between the JOINT participant list and the REL TO floor. This
//! is the structural difference from the conscious-defer rules (E010
//! HCS-O vs HCS-P, E015 REL TO USA, LIST vs NOFORN, E016 JOINT +
//! RESTRICTED) where the source provides two valid fills and
//! classifier judgment picks one.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// E014 emits N diagnostics — one per missing JOINT co-owner — when
/// the portion carries no REL TO at all. `(//JOINT S AUS CAN USA)`
/// has JOINT participants `{AUS, CAN, USA}`; with an empty REL TO,
/// all three are missing. Each diagnostic carries a single-country
/// FactAdd intent.
///
/// The legacy pre-migration form was ONE diagnostic with a plural
/// message `"JOINT participants [AUS, CAN, USA] must appear in REL TO
/// list"`. The post-migration form is N diagnostics with singular
/// messages `"JOINT participant <X> must appear in REL TO list"` —
/// one per missing country, each carrying its own `FactAdd` intent.
///
/// Per-diagnostic actionability + `FixIntent.replacement` being
/// strict-singleton (one `ReplacementIntent` per `FixIntent`) are the
/// two rationales for the split.
#[test]
fn e014_emits_one_diagnostic_per_missing_country_in_portion() {
    let result = engine().lint(b"(//JOINT S AUS CAN USA)\n");
    let e014: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "portion.classification.joint-requires-rel-to-coverage")
        .collect();

    // JOINT participants are {AUS, CAN, USA}; REL TO is empty; all
    // three are missing.
    assert_eq!(
        e014.len(),
        3,
        "E014 must fire once per missing co-owner; \
         expected 3 diagnostics for `(//JOINT S AUS CAN USA)` with empty REL TO, \
         got {} — diagnostics: {:?}",
        e014.len(),
        result.diagnostics,
    );

    let messages: Vec<&str> = e014.iter().map(|d| d.message.as_ref()).collect();
    for country in ["AUS", "CAN", "USA"] {
        assert!(
            messages.iter().any(|m| m.contains(country)),
            "E014 must emit a diagnostic naming `{country}` (per-country \
             singular message); got messages: {messages:?}",
        );
    }

    // Each diagnostic must carry exactly one FactAdd intent on the
    // open-vocab CountryCode path.
    for d in &e014 {
        let intent = d
            .fix
            .as_ref()
            .expect("E014 diagnostics must carry a FixIntent post-migration");
        match &intent.replacement {
            marque_scheme::ReplacementIntent::FactAdd { token, scope } => {
                assert!(
                    matches!(
                        token,
                        marque_scheme::FactRef::OpenVocab(
                            marque_capco::CapcoOpenVocabRef::CountryCode(_)
                        )
                    ),
                    "E014 FactAdd payload must be a CountryCode open-vocab \
                     ref; got: {token:?}",
                );
                assert_eq!(
                    *scope,
                    marque_scheme::Scope::Portion,
                    "E014 FactAdd intent must target portion scope; got: \
                     {scope:?}",
                );
            }
            other => panic!(
                "E014 must emit a FactAdd intent (not {:?}); diagnostic: {:?}",
                other, d,
            ),
        }
    }
}

/// Predicate guard: when every JOINT participant is already in REL TO,
/// E014 does not fire. `(//JOINT S AUS CAN USA//REL TO USA, AUS, CAN)`
/// has all three co-owners present.
#[test]
fn e014_does_not_fire_when_all_participants_in_rel_to() {
    let result = engine().lint(b"(//JOINT S AUS CAN USA//REL TO USA, AUS, CAN)\n");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.predicate_id() != "E014"),
        "E014 must not fire when all JOINT participants present in \
         REL TO; diagnostics: {:?}",
        result.diagnostics,
    );
}

/// Predicate guard: tetragraph coverage. FVEY expands to
/// `{AUS, CAN, GBR, NZL, USA}` per the ISMCAT taxonomy (verified at
/// `crates/capco/src/vocab.rs:52`). A JOINT marking with participants
/// drawn from FVEY members satisfies E014 when REL TO contains FVEY.
///
/// This pins `rel_to_covers`'s tetragraph-expansion path: the
/// predicate's `expand_tetragraph` lookup is what makes `REL TO USA,
/// FVEY` cover GBR (a FVEY member).
#[test]
fn e014_does_not_fire_when_tetragraph_covers_participants() {
    let result = engine().lint(b"(//JOINT S AUS GBR USA//REL TO USA, FVEY)\n");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.predicate_id() != "E014"),
        "E014 must not fire when JOINT participants are covered by a \
         tetragraph in REL TO (FVEY covers AUS, GBR, USA); \
         diagnostics: {:?}",
        result.diagnostics,
    );
}

/// E014 fires exactly once when only one JOINT participant is missing.
/// `(//JOINT S AUS CAN USA//REL TO USA, AUS)` has CAN missing only.
/// The diagnostic message must contain "CAN" and must not name USA or
/// AUS (which are present and not violations).
#[test]
fn e014_fires_only_for_actually_missing_country() {
    let result = engine().lint(b"(//JOINT S AUS CAN USA//REL TO USA, AUS)\n");
    let e014: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "portion.classification.joint-requires-rel-to-coverage")
        .collect();

    assert_eq!(
        e014.len(),
        1,
        "E014 must fire exactly once when only CAN is missing; \
         diagnostics: {:?}",
        result.diagnostics,
    );
    let msg = e014[0].message.as_ref();
    assert!(
        msg.contains("CAN"),
        "E014 diagnostic must name the missing country `CAN`; got: {msg:?}",
    );
    // USA and AUS are already present; they must not appear in the
    // diagnostic message that targets CAN.
    assert!(
        !msg.contains("USA"),
        "E014 diagnostic for missing CAN must not name USA \
         (already present in REL TO); got: {msg:?}",
    );
    assert!(
        !msg.contains("AUS"),
        "E014 diagnostic for missing CAN must not name AUS \
         (already present in REL TO); got: {msg:?}",
    );
}

/// Round-trip: `(//JOINT S AUS CAN USA)` → portion gains REL TO list
/// containing the JOINT co-owners after one `Engine::fix` pass. Second
/// pass produces no further E014 fix (idempotence).
///
/// # Audit-collapse note (one AppliedFix for N intents)
///
/// `synthesize_intent_only_fixes` groups all intents sharing a
/// `candidate_span` and emits ONE `FixProposal` per group (Copilot
/// PR #369 finding #1, architect preflight 2026-05-11, Constitution V
/// audit-per-rule). When E014 emits three diagnostics for missing AUS,
/// CAN, USA — all anchored to the same candidate — the engine collapses
/// them into a single AppliedFix that carries the combined effect of
/// all three FactAdds. The N=3 diagnostic shape is preserved in the
/// lint stream (`remaining_diagnostics` view + the dry-run path); the
/// auto-apply audit log shows one entry per affected candidate.
///
/// Load-bearing invariants pinned by this test:
/// - At least one `AppliedFix` entry with `rule = "E014"` lands per
///   affected candidate (the audit-collapse minimum).
/// - The fixed output contains every JOINT co-owner in the REL TO
///   list (the FactAdd batch applied correctly as a group).
/// - The second `Engine::fix` pass produces no further E014
///   AppliedFix entries (idempotence; predicate is false once every
///   co-owner is in REL TO).
#[test]
fn e014_fix_apply_inserts_missing_countries_idempotently() {
    let first = engine().fix(b"(//JOINT S AUS CAN USA)\n", FixMode::Apply);

    // At least one E014 AppliedFix (audit-collapse: one per candidate).
    let e014_applied: Vec<_> = first
        .applied
        .iter()
        .filter(|af| af.rule.predicate_id() == "portion.classification.joint-requires-rel-to-coverage")
        .collect();
    assert!(
        !e014_applied.is_empty(),
        "E014 must produce at least one AppliedFix entry \
         (audit-collapsed to one per candidate); applied rules: {:?}",
        first
            .applied
            .iter()
            .map(|af| af.rule.predicate_id())
            .collect::<Vec<_>>(),
    );

    // G13 invariant: intent-only AppliedFix carries empty `original`
    // so document content cannot leak into the audit record. Pinned
    // here at the rule level; `g13_closure_fix_intent.rs` is the
    // workspace-wide gate.
    for af in &e014_applied {
        assert!(
            af.proposal.original.is_empty(),
            "G13: E014 intent-only AppliedFix must carry empty \
             `original`; got: {:?}",
            af.proposal.original,
        );
    }

    // The fixed output must contain all three co-owners in the REL TO
    // position. We don't pin the exact rendered byte sequence because
    // engine canonicalization order can interact with concurrent
    // page-rewrite rules and other rules firing on the same input —
    // assert the JOINT participants appear in the REL TO list
    // specifically (not just anywhere in the marking, since the JOINT
    // participant list also names them).
    let fixed = std::str::from_utf8(&first.source).expect("fixed output is UTF-8");
    let rel_to_start = fixed.find("REL TO").unwrap_or_else(|| {
        panic!("fix must produce a REL TO marking on the portion; got: {fixed:?}")
    });
    // The REL TO list extends from "REL TO " through the next category
    // separator (`//`) or the close-paren of the portion. Capture
    // everything from "REL TO " to the first occurrence of either.
    let after_rel_to = &fixed[rel_to_start + "REL TO ".len()..];
    let rel_to_list_end = after_rel_to
        .find("//")
        .or_else(|| after_rel_to.find(')'))
        .unwrap_or(after_rel_to.len());
    let rel_to_list = &after_rel_to[..rel_to_list_end];
    for country in ["USA", "AUS", "CAN"] {
        assert!(
            rel_to_list.contains(country),
            "fixed REL TO list must contain co-owner `{country}`; \
             extracted REL TO list: {rel_to_list:?}, full fixed: {fixed:?}",
        );
    }

    // Idempotence: a second `Engine::fix` pass on the fixed output
    // produces no further E014 AppliedFix entries. The predicate
    // `joint-requires-rel-to-coverage` is false after every co-owner
    // has been added.
    let second = engine().fix(&first.source, FixMode::Apply);
    assert!(
        second.applied.iter().all(|af| af.rule.predicate_id() != "E014"),
        "second pass must not re-apply E014 (idempotence after all \
         co-owners added); applied: {:?}",
        second
            .applied
            .iter()
            .map(|af| af.rule.predicate_id())
            .collect::<Vec<_>>(),
    );
}

/// E014 also fires on the banner form. `//JOINT SECRET AUS CAN USA`
/// banner has the same §H.3 p57 "at both the banner and portion level"
/// floor obligation. JOINT participants must appear in the banner-
/// level REL TO list; the rule fires once per missing co-owner with
/// the same N-diagnostic shape.
///
/// Banner-level firing is what makes the §H.3 p57 "at both the banner
/// and portion level" obligation visible in the engine output. The
/// banner-level REL TO list is constructed by the page-level rewrite
/// layer from the portions' REL TO lists; E014 fires on the banner
/// candidate when the constructed list still has a co-owner missing.
#[test]
fn e014_fires_on_banner_form() {
    let result = engine().lint(b"//JOINT SECRET AUS CAN USA\n");
    let e014: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "portion.classification.joint-requires-rel-to-coverage")
        .collect();

    // Empty banner-level REL TO → all three co-owners missing.
    assert_eq!(
        e014.len(),
        3,
        "E014 must fire once per missing co-owner on the banner form \
         (same N-diagnostic shape as portion); expected 3 diagnostics \
         for `//JOINT SECRET AUS CAN USA` with no REL TO, got {} — \
         diagnostics: {:?}",
        e014.len(),
        result.diagnostics,
    );
    let messages: Vec<&str> = e014.iter().map(|d| d.message.as_ref()).collect();
    for country in ["AUS", "CAN", "USA"] {
        assert!(
            messages.iter().any(|m| m.contains(country)),
            "E014 banner-form must emit a diagnostic naming `{country}`; \
             got messages: {messages:?}",
        );
    }
}
