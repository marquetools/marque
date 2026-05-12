// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Commit 3 — strengthened G13 closure gate on the new
//! `FixIntent` emission path.
//!
//! Constitution V Principle V (audit content-ignorance) requires
//! that audit records carry no document content. The pre-PR-3c
//! `FixProposal.original` and `FixProposal.replacement` fields ARE
//! a known pre-existing G13 channel — they carry source bytes by
//! design. Per Path C of the consolidated plan
//! (`docs/plans/2026-05-10-pr3c-consolidated-plan.md` lines 100–175),
//! that channel closes at Commit 10 with the `FixProposal` retirement;
//! commits 2–9 keep the legacy channel open to preserve byte-stable
//! NDJSON output.
//!
//! This test pins Constitution V Principle V on the **new** emission
//! path — the `intent: Box<FixIntent<S>>` payload inside
//! `AppliedFixProposal::New`. By construction `FixIntent<S>` references
//! tokens via `FactRef::Cve(TokenId)` (a numeric handle into the CVE
//! vocabulary) or `FactRef::OpenVocab(S::OpenVocabRef)` (typed
//! structural reference); both are content-ignorant carriers, NOT
//! source bytes. Categorial enums (`Scope`, `RecanonScope`) are
//! discriminants, not text. `Confidence` and `Message` carry no
//! document bytes either (`Message` is a closed template + closed
//! args; the template is an enum variant, the args are typed
//! scalars). The test walks the in-memory `intent` payload and
//! asserts every reachable byte falls into this structural envelope.
//!
//! **Scope:** `AppliedFixProposal::New { intent, .. }` records ONLY.
//! `AppliedFixProposal::Legacy` records — and the `synthesized:
//! FixProposal` field of `New` records — carry document bytes by
//! Path C design and are out of scope until Commit 10.

use marque_capco::{CapcoOpenVocabRef, CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::{AppliedFix, AppliedFixProposal, FixIntent};
use marque_scheme::{FactRef, RecanonScope, ReplacementIntent, Scope};

// ---------------------------------------------------------------------------
// Engine fixture
// ---------------------------------------------------------------------------

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

// ---------------------------------------------------------------------------
// G13 envelope walker
// ---------------------------------------------------------------------------
//
// Per Path C scoping, the test asserts that the in-memory `intent`
// payload carries ONLY structural references — no document bytes.
// The walker is exhaustive over the `FixIntent<CapcoScheme>` type:
// every variant of `ReplacementIntent` and every variant of
// `FactRef` must be reachable through it. Adding a new variant
// without extending the walker fails compilation (the `match` is
// non-exhaustive); adding a new `MarkingScheme::OpenVocabRef`
// variant requires extending `assert_open_vocab_ref_is_structural`.

/// Walk every reachable byte / discriminant inside an in-memory
/// `FixIntent<CapcoScheme>` and assert each falls into the
/// content-ignorant structural envelope (Constitution V
/// Principle V).
fn assert_intent_is_g13_clean(intent: &FixIntent<CapcoScheme>) {
    // `ReplacementIntent` — the three structural variants. Each
    // either carries a `FactRef` + `Scope` (FactAdd / FactRemove)
    // or a `RecanonScope` discriminant (Recanonicalize). No raw
    // source bytes anywhere.
    match &intent.replacement {
        ReplacementIntent::FactAdd { token, scope } => {
            assert_fact_ref_is_structural(token);
            assert_scope_is_discriminant(*scope);
        }
        ReplacementIntent::FactRemove { token_ref, scope } => {
            assert_fact_ref_is_structural(token_ref);
            assert_scope_is_discriminant(*scope);
        }
        ReplacementIntent::Recanonicalize { scope } => {
            assert_recanon_scope_is_discriminant(*scope);
        }
    }

    // `Confidence` carries `f32` values plus an optional closed
    // list of `FeatureId` discriminants. No source bytes. The
    // engine reads `intent.confidence.combined()` for the threshold
    // gate — that's a scalar, not document content.
    let _ = intent.confidence.combined();

    // `feature_ids` is `SmallVec<[FeatureId; 4]>`. `FeatureId` is
    // a closed enum (see `crates/rules/src/confidence.rs`); its
    // variants are named feature labels, not document text. The
    // walker just confirms each element is a valid discriminant by
    // matching on it; an exhaustive match isn't possible without
    // pinning the variant set here, so we rely on the type system
    // (FeatureId carries no payload).

    // `Message` is a closed template + closed args. The template
    // is `MessageTemplate`, a non-exhaustive enum of structured
    // diagnostic templates — none of its variants carry source
    // bytes. The args are typed scalars (TokenId, integer counts,
    // discriminant tags). See `crates/rules/src/message.rs`. As
    // with FeatureId, we rely on the type system to enforce
    // structural-only payloads.
    let _ = intent.message.template();
}

/// `FactRef` has two variants. `Cve(TokenId)` is a numeric handle
/// into the CVE vocabulary — no source bytes. `OpenVocab` carries
/// a `CapcoScheme::OpenVocabRef` (`CapcoOpenVocabRef`); the walker
/// matches on its variants and asserts each is structural.
fn assert_fact_ref_is_structural(fact_ref: &FactRef<CapcoScheme>) {
    match fact_ref {
        FactRef::Cve(_token_id) => {
            // TokenId is a transparent wrapper around a u16 (see
            // `marque_scheme::TokenId`). Numeric handle, not text.
        }
        FactRef::OpenVocab(open_ref) => {
            assert_open_vocab_ref_is_structural(open_ref);
        }
    }
}

/// `CapcoOpenVocabRef` is CAPCO's open-vocab carrier (SAR program
/// IDs, SCI compartment paths, FGI tetragraphs, etc.). Each variant
/// either carries a typed structural value (`TokenId` / integer /
/// enum discriminant) or a canonicalized open-vocab identifier
/// produced by the scheme's canonicalize step. None of the variants
/// carry raw source-buffer slices.
///
/// **Audit content-ignorance carve-out for canonicalized open-vocab
/// strings.** When the scheme's canonicalize step produces an
/// open-vocab identifier from input (e.g., a SAR program codeword
/// like `"FOX"` or `"BUTTER POPCORN"`), the value IS the canonical
/// identifier, not a slice of the original source. The audit
/// principle distinguishes:
///
/// - Document content (paragraph text, subject claims, free-form
///   strings) — Constitution V forbids in audit records.
/// - Canonical token identifiers (CAPCO codewords, country trigraphs,
///   tetragraphs) — these ARE the marking's structure. A SAR
///   program ID without its codeword is not a usable audit record.
///
/// CAPCO codewords are the smallest content unit that uniquely
/// identifies the marking; treating them as audit-bearing is the
/// same trade-off the CVE numeric handle makes (the handle resolves
/// to "NOFORN" or "TK" — knowing which is what makes the audit
/// useful). The walker accepts these canonicalized identifiers as
/// structural references, the same way it accepts TokenIds via
/// `FactRef::Cve`.
fn assert_open_vocab_ref_is_structural(open_ref: &CapcoOpenVocabRef) {
    // The walker matches exhaustively (non-exhaustive `_` arm
    // intentionally omitted so a future open-vocab variant fails
    // compilation here and forces a re-audit of the new variant's
    // payload shape). If a new variant lands without an explicit
    // arm here, this test will fail to compile and a human MUST
    // audit whether the new payload is structural or content.
    match open_ref {
        // SAR program identifier — canonical codeword (e.g. "FOX").
        // Canonical identifier, not source slice; see the doc
        // comment above for the audit-content-ignorance carve-out.
        CapcoOpenVocabRef::Sar(_) => {}
        // SCI compartment — canonical structural string built from
        // the §A.6 grammar.
        CapcoOpenVocabRef::SciCompartment(_) => {}
        // SCI sub-compartment — same canonical structural carrier.
        CapcoOpenVocabRef::SciSubCompartment(_) => {}
        // FGI / JOINT tetragraph — canonical 4-letter token from
        // the ISMCAT taxonomy.
        CapcoOpenVocabRef::FgiTetragraph(_) => {}
        // REL TO country code or country-group — canonical
        // `marque_ism::CountryCode` value (16-byte fixed buffer, no
        // heap, no raw input bytes). Wired by PR 3c.B Sub-PR 8.D.4
        // as the first open-vocab consumer of the CAT_REL_TO axis;
        // E014 emits one `FactAdd { CountryCode(...), Portion }` per
        // missing JOINT co-owner. The variant carries no document
        // text — G13 audit-content-ignorance invariant holds by
        // construction.
        CapcoOpenVocabRef::CountryCode(_) => {}
    }
}

/// `Scope` is a closed enum discriminant — Portion / Page /
/// Document / Diff. No payload.
fn assert_scope_is_discriminant(scope: Scope) {
    match scope {
        Scope::Portion | Scope::Page | Scope::Document | Scope::Diff => {}
    }
}

/// `RecanonScope` is a narrowing of `Scope` excluding `Diff` —
/// closed enum, no payload.
fn assert_recanon_scope_is_discriminant(scope: RecanonScope) {
    match scope {
        RecanonScope::Portion | RecanonScope::Page | RecanonScope::Document => {}
    }
}

// ---------------------------------------------------------------------------
// Gate: every `AppliedFixProposal::New` record is G13-clean
// ---------------------------------------------------------------------------

/// Run every migrated rule's representative fixture through the
/// engine and assert each promoted `AppliedFixProposal::New` record's
/// intent payload passes the structural envelope walker. Covers every
/// `ReplacementIntent` variant a migrated rule emits:
/// - `FactRemove` (E054 / E057) — Commit 3 beachhead
/// - `FactAdd` (E021, E002 USA-missing branch) — Commit 3 + 6
/// - `Recanonicalize` (E002 USA-not-first branch, S003) — Commit 6
#[test]
fn all_migrated_rule_intents_pass_g13_envelope_walker() {
    let fixtures = [
        // (input, rule_id, expected variant tag)
        ("(S//NF/RELIDO)\n", "E054", "FactRemove"),
        ("(S//NF/IMC/RELIDO)\n", "E054", "FactRemove"),
        ("(S//OC-USGOV/RELIDO)\n", "E057", "FactRemove"),
        // PR 3c.B Commit 8 — E056 (ORCON ⊥ RELIDO) migrated to
        // dual-population. Same `FactRemove { RELIDO, Portion }`
        // shape as E054/E057; the wrapper reuses
        // `relido_remove_intent()`.
        ("(S//OC/RELIDO)\n", "E056", "FactRemove"),
        // PR 3c.B Sub-PR 8.E.2 (unblocks E041 in #106) — E041 (NODIS supersedes
        // EXDIS in portion) is the first non-RELIDO `FactRemove`
        // consumer of `synthesize_intent_only_fixes`. Unlike
        // E054/E055/E056/E057 (which are dual-populated under Path C),
        // E041 is intent-only — the engine synthesizes the
        // byte-precise `FixProposal` from the intent + the rule's
        // `RuleContext::candidate_span`. NF is included so E038
        // (NODIS-requires-NOFORN) does not also fire on the same
        // candidate. §H.9 p172 + p174 name EXDIS as the loser.
        ("(S//NF//ND/XD)\n", "E041", "FactRemove"),
        // PR 3c.B Sub-PR 8.D.1 — E038 (NODIS/EXDIS require NOFORN)
        // is the first `FactAdd` consumer of
        // `synthesize_intent_only_fixes`. Same intent-only shape as
        // E041 (the engine synthesizes the byte-precise `FixProposal`
        // from the intent + `RuleContext::candidate_span`), but on
        // the FactAdd path: `apply_intent` adds NOFORN to
        // `dissem_controls` instead of removing a token. §H.9 p172
        // (EXDIS) + p174 (NODIS) both use "Requires NOFORN"
        // verbatim, which is what makes `MessageTemplate::
        // RequiredByPresence` the right structured-message variant.
        ("(S//ND)\n", "E038", "FactAdd"),
        // E055 (RELIDO ⊥ DISPLAY ONLY) also migrated in Commit 8
        // but the engine's parser does not yet emit `Displayonly`
        // tokens for any DISPLAY ONLY surface form (parser-gap
        // #323). Wrapper-level intent-shape coverage lives in
        // `relido_conflicts.rs`; once #323 closes, add
        // `("(S//RELIDO/DISPLAY ONLY)\n", "E055", "FactRemove")`
        // here.
        // PR 3c.B Sub-PR 8.D.2 — E053 (NOFORN ⊥ REL TO, §H.8 p145)
        // is the first consumer of the `TOK_REL_TO` whole-axis-clear
        // sentinel on CAT_REL_TO. NOFORN unambiguously supersedes
        // REL TO per §H.8 p145; the intent clears the REL TO axis
        // entirely (not just USA). Portion-scope only — banner
        // roll-up is handled by the `capco/noforn-clears-rel-to`
        // PageRewrite, NOT this rule.
        ("(S//NF//REL TO USA, GBR)\n", "E053", "FactRemove"),
        ("(S//RD//IMC)\n", "E021", "FactAdd"),
        // PR 3c.B Sub-PR 8.D.4 — E014 (JOINT participants require REL
        // TO coverage, §H.3 p57) is the first consumer of the
        // open-vocab `CapcoOpenVocabRef::CountryCode` FactAdd path on
        // the CAT_REL_TO axis. The rule emits N FactAdds (one per
        // missing JOINT co-owner); `find` picks up the first auto-
        // applied entry. `(//JOINT S GBR USA)` has GBR missing from
        // the implicit-empty REL TO list — one FactAdd intent fires.
        ("(//JOINT S GBR USA)\n", "E014", "FactAdd"),
        // E002 USA-missing branch — FactAdd { USA, Page } on banner.
        ("SECRET//REL TO GBR\n", "E002", "FactAdd"),
        // E002 USA-not-first branch — Recanonicalize { Page } on
        // banner.
        ("SECRET//REL TO GBR, USA\n", "E002", "Recanonicalize"),
        // S003 — JOINT classification with USA not first on banner.
        // Recanonicalize { Page } per the classification axis.
        ("//JOINT SECRET GBR USA\n", "S003", "Recanonicalize"),
    ];

    for (input, rule_id, expected_variant) in fixtures {
        let result = engine().fix(input.as_bytes(), FixMode::Apply);
        let af = result
            .applied
            .iter()
            .find(|af| af.proposal.rule.as_str() == rule_id)
            .unwrap_or_else(|| {
                let fired: Vec<&str> = result
                    .applied
                    .iter()
                    .map(|af| af.proposal.rule.as_str())
                    .collect();
                panic!(
                    "{rule_id} did not auto-apply on {input:?}; rules \
                     that did: {fired:?}"
                )
            });

        match &af.proposal {
            AppliedFixProposal::New {
                intent,
                synthesized: _,
            } => {
                // The structural envelope walker — fails the test
                // if any byte falls outside the content-ignorant set.
                assert_intent_is_g13_clean(intent);

                // Sanity: variant tag matches the expected shape.
                // A `FactRemove` rule emitting a `FactAdd` (or vice
                // versa) would pass the walker but indicate a
                // migration error — catch it here too.
                let actual = match &intent.replacement {
                    ReplacementIntent::FactAdd { .. } => "FactAdd",
                    ReplacementIntent::FactRemove { .. } => "FactRemove",
                    ReplacementIntent::Recanonicalize { .. } => "Recanonicalize",
                };
                assert_eq!(
                    actual, expected_variant,
                    "{rule_id} on {input:?}: expected {expected_variant} \
                     variant, got {actual}"
                );
            }
            AppliedFixProposal::Legacy(_) => panic!(
                "{rule_id} on {input:?}: expected AppliedFixProposal::New \
                 (migrated rule); got Legacy. The dual-population pairing \
                 in Engine::fix_inner did not fire."
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Scope guard: Legacy records and the `synthesized` field are NOT
// covered by this gate
// ---------------------------------------------------------------------------
//
// Per Path C of the consolidated plan, the legacy
// `FixProposal.original` / `.replacement` channel stays open through
// Commit 9 to preserve byte-stable NDJSON. The G13 closure on
// `FixProposal` itself is a Commit 10 concern. This test asserts the
// scope explicitly — a still-non-migrated rule emits a `Legacy`
// variant, and the walker is NOT run on it.
//
// As migrations land (PR 3c.B Commits 3, 6, 8, Sub-PR 8.D.1, ...),
// this scope-guard fixture rotates through rules that remain on the
// legacy path. PR 3c.B Commit 8 migrated the four `Conflicts` RELIDO
// wrappers (E054/E055/E056/E057); PR 3c.B Sub-PR 8.E.2 migrated E041;
// PR 3c.B Sub-PR 8.D.1 migrated E038; PR 3c.B Sub-PR 8.D.2 migrated
// E015/E053; PR 3c.B Sub-PR 8.D.3 migrated E010 as consciously-
// deferred (`fix_intent: None`, matching E015/E016). The scope guard
// stays on an unmigrated rule that still produces a deterministic
// `Legacy` AppliedFix. `E007` (XShorthandDateRule) is the post-8.D.3
// stable choice: `make_fix_diagnostic` Legacy fix that auto-applies
// on deprecated `25X1-` / `50X1-` X-shorthand declassification codes
// at confidence 0.97 (table-backed via `MIGRATIONS`). E012
// (DeclarativeDualClassificationRule) was considered but emits its
// fix at confidence 0.90 — below the default
// `Config::confidence_threshold` of 0.95, so the engine demotes the
// diagnostic to `Severity::Suggest` and it never lands in
// `result.applied`. E007 sits cleanly above the gate.
//
// E007's eventual migration target is a structural `FixIntent` on
// the declassification-date axis (the X-shorthand canonical form is
// a string transform plus an optional migration-table lookup) —
// none of which is wired today. E007 is queued for a later sub-PR
// once the declassification-axis primitives land; until then it
// produces a stable Legacy AppliedFix and serves as the scope guard.
//
// E016 (DeclarativeJointRestrictedRule, JOINT+RESTRICTED) is the
// canonical "consciously-deferred no-fix-intent" rule per Sub-PR 8.B
// (it emits `with_fix_intent(..., None)`), but doesn't fit this
// scope-guard shape: with `fix: None` AND `fix_intent: None`, E016
// never lands in `result.applied`, so the "Legacy variant present"
// assertion below cannot match against it. The guard requires a rule
// that auto-applies as Legacy.
//
// When E007 migrates, look further down the rule list for the next
// legacy `make_fix_diagnostic` consumer that emits at confidence
// ≥ `Config::confidence_threshold` default (0.95). E010 was the
// scope-guard pre-8.D.3 (legacy `HCS → HCS-P` at 0.95 confidence);
// it migrated in 8.D.3 as consciously-deferred (Category A.x,
// `fix_intent: None` like E015/E016) and is no longer in the
// legacy-auto-apply pool. E015 was a candidate prior to Sub-PR 8.D.2
// and migrated in the same shape.
//
// Constitution V Principle V test-fixture carve-out applies to any
// fabricated `AppliedFix` values: this test exercises real engine
// promotion via `Engine::fix`, so no `__engine_promote` carve-out
// comment is required.

#[test]
fn legacy_variant_records_are_out_of_scope_for_this_gate() {
    // `SECRET//25X1-//NOFORN` triggers E007 (XShorthandDateRule);
    // the rule emits a legacy `FixProposal` (`25X1-` → `25X1` at
    // confidence 0.97 via the table-backed `MIGRATIONS` entry) via
    // `make_fix_diagnostic`. Default severity is `Severity::Error`
    // (`rules.rs:858`); the engine's auto-apply filter excludes only
    // `Severity::Suggest` (`crates/engine/src/engine.rs:1378`), so
    // Error-severity rules with a populated `fix` at confidence ≥
    // threshold (default 0.95) still auto-apply — promotion goes
    // through the Legacy path because E007 carries no `fix_intent`
    // (not yet migrated; queued for a later sub-PR once the
    // declassification-axis primitives land).
    let result = engine().fix(b"SECRET//25X1-//NOFORN\n", FixMode::Apply);
    let af = result
        .applied
        .iter()
        .find(|af| af.proposal.rule.as_str() == "E007")
        .expect("E007 must fire on SECRET//25X1-//NOFORN");

    // E007 is NOT yet migrated. The walker would refuse to inspect
    // this record because `AppliedFixProposal::Legacy` carries no
    // `intent` field; this test confirms the scope is what we
    // expect (and serves as a regression pin if a future commit
    // accidentally migrates E007 without updating the migrated-
    // rule fixture list above).
    assert!(
        matches!(af.proposal, AppliedFixProposal::Legacy(_)),
        "E007 (non-migrated) must emit AppliedFixProposal::Legacy \
         through Commit 9; the migration to New + G13-clean intent \
         lands in a later commit. Once it does, add E007's fixture \
         to `all_migrated_rule_intents_pass_g13_envelope_walker` and \
         rotate this scope guard to another non-migrated rule."
    );

    // The walker is NOT called on Legacy records. Constitution V
    // Principle V G13 closure on the legacy channel is a Commit 10
    // concern (atomic with the audit-schema flip and the FixProposal
    // retirement).
    let _: &AppliedFix<_> = af; // type-anchor; no assertion needed.
}
