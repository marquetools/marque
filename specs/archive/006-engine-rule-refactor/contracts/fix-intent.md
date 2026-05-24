<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Contract: `FixIntent<S>` — Rule-Emission API

**Lands at**: PR 3c
**Spec FRs**: FR-001, FR-002, FR-003, FR-004, FR-005, FR-021, FR-022, FR-023, FR-025, FR-026, FR-027
**Source-plan refs**: §3.1 (FixIntent no longer deferred), §8.1 (Canonical sealing + cross-crate emission), §9 (phase-tagged pass split)
**Audience**: rule-crate authors (`marque-capco` today; `marque-cui` and partner-national rule crates eventually).

---

## Contract surface

A rule that wants to emit a fix returns a `Diagnostic` whose `fix:
Option<FixIntent<S>>` is `Some(intent)`. The engine — not the rule —
renders the intent into a `Canonical<S>` and promotes it into an
`AppliedFix` via `Engine::fix_inner`. Rules MUST NOT construct
`Canonical<S>`, `AppliedFix`, or any other audit-promotion type
directly.

```rust
// In a rule's evaluate():
fn evaluate(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic> {
    if !some_predicate(attrs) {
        return vec![];
    }

    vec![Diagnostic {
        rule: self.id(),
        severity: Severity::Error,
        span: detected_span,
        message: Message::new(
            MessageTemplate::PortionUnknownDissem,
            MessageArgs {
                token: Some(seen_token),
                expected_token: Some(canonical_token),
                ..MessageArgs::default()
            },
        ),
        citation: Citation::new(SectionRef::H8, PageNumber::new(150), AuthoritativeSource::Capco2016),
        fix: Some(FixIntent {
            target_span: detected_span,
            replacement: ReplacementIntent::Cve { token: canonical_token, scope: Scope::Portion },
            confidence: Confidence::new(0.95, 1.0).unwrap(),
            feature_ids: smallvec![],
            message: /* ... */,
        }),
    }]
}
```

---

## What rules MAY construct

- **`FixIntent<S>` values**: target span, replacement intent (CVE / Render / Delete), confidence axes, feature-id contributions.
- **`Diagnostic` values**: rule ID, severity, span, message (template + args only), citation, optional fix intent.
- **`Message` values**: via `Message::new(template, args)` only.
- **`Citation` values**: via `Citation::new(section, page, document)` — runtime-validated against the vendored source; lint-validated mechanically (FR-018).
- **`MessageArgs` values**: via `MessageArgs::default()` plus field-level assignment of permitted scalar/ID types only (TokenId, CategoryId, Span, Blake3Hash, Confidence, FeatureId).
- **`ReplacementIntent::Render` values**: only when the rule needs an open-vocabulary replacement (SCI sub-comp, SAR program ID, custom trigraph). The `directive: RenderDirective<S>` carries scheme-specific structured data; the engine routes it through `S::render_canonical`.

## What rules MUST NOT construct

- **`Canonical<S>`**: no `Box<str> → Canonical` path; closed-CVE goes through `Canonical::from_cve(TokenId, Scope)` (callable from anywhere, but `TokenId` itself comes from `Vocabulary<S>::lookup`); open-vocab goes through `Canonical::from_render` which is `pub(crate)` to `marque-scheme` and reachable only from `MarkingScheme::render_canonical` impls.
- **`AppliedFix`**: `pub #[doc(hidden)] __engine_promote` is reachable only from `Engine::fix_inner` (FR-005, FR-040 lint enforces).
- **`EnginePromotionToken`**: same — `__engine_construct` is engine-only.
- **`Message` from a free-form string**: `Message::new` takes `MessageTemplate` + `MessageArgs`; no `Message::from_str`, no `impl From<&str> for Message`. `format!("...{input_bytes}...")` constructions of message text fail the citation lint and the type signature.
- **Synthetic engine diagnostics (R001 / R002)**: minted by `marque-engine`, not by rule crates (FR-041).

---

## Phase contract

Each rule declares `Phase::Localized | WholeMarking` at construction
(FR-021). The engine enforces span-shape constraints at registration
(`Engine::new`) and rejects violating rules:

| Phase | `FixIntent::target_span` constraint |
|---|---|
| `Localized` | Strictly inside a single token boundary. |
| `WholeMarking` | Covers a full marking span end-to-end. |

A rule that needs both phases registers two entries (one per phase)
sharing a backend module — no `Phase::Both` escape hatch. The two
entries each carry their own `RuleId` so they appear separately in
audit records.

---

## Pre-pass-1 attributes (Phase::WholeMarking only)

`Phase::WholeMarking` rules receive `RuleContext.pre_pass_1_attrs:
Option<&CanonicalAttrs<'src>>`:
- `Some(pre_attrs)` when the rule's span overlaps a pass-1
  `AppliedFix`'s span. The engine populates from a `SmallVec<[CanonicalAttrs<'src>; 4]>`
  cache owned by `Engine::fix_inner`'s stack frame (R-4).
- `None` when no overlap, or for `Phase::Localized` rules (which only
  dispatch in pass-1).

I-19 reshape-aware re-validation (FR-023): if the predicate held
against `pre_pass_1_attrs`, treat as already-fixed and DO NOT re-fire
(unless a different `(scheme, predicate-id)` rule is the post-pass-1
match — see plan §9.3 disambiguation). If the predicate was introduced
by the pass-1 reshape, fire — but the I-18 non-overlap invariant
(FR-022) means the pass-2 fix span cannot overlap the pass-1 fix span;
overlapping pass-2 diagnostics demote to suggestions, not auto-applied.

---

## Confidence and feature IDs

```rust
pub struct Confidence {
    pub recognition: f32,    // [0, 1]
    pub rule: f32,           // [0, 1]
    pub region: Option<f32>, // optional posterior region marker
    pub runner_up_ratio: Option<f32>,
}

impl Confidence {
    pub fn combined(&self) -> f32 { self.recognition * self.rule }
    pub fn new(recognition: f32, rule: f32) -> Result<Self, ConfidenceError> { /* validates [0,1] */ }
}
```

The engine filter (`Engine::fix_inner`) applies the configured
threshold to `Confidence::combined()` (I-6). Rules supply both axes
(recognition from the recognizer dispatch, rule from the rule's own
self-assessed confidence) plus a closed list of named `FeatureId`
contributions:

```rust
pub enum FeatureId {
    PrecedingFixPenalty,    // PR 7 — E003 confidence reduces when a preceding fix is staged
    DecoderRecognized,      // produced by DecoderRecognizer dispatch
    StrictExactMatch,       // produced by StrictRecognizer dispatch
    /* ... closed set; adding a variant requires a coordinated MARQUE_AUDIT_SCHEMA bump */
}
```

Rules SHOULD populate `feature_ids` with relevant `FeatureId` values
when their confidence scoring depends on them. The audit emitter
records the `feature_ids` set; consumers can reproduce the confidence
calculation from `(recognition, rule, feature_ids)`.

---

## Cross-crate emission (sealed-trait pattern, R-7)

External rule crates depend on `marque-rules` (which re-exports
`FixIntent<S>`, `ReplacementIntent<S>`, `RenderDirective<S>`,
`Message`, `MessageTemplate`, `MessageArgs`, `Citation`, `Confidence`,
`FeatureId`, `Phase`, `Rule`, `Diagnostic`). They do NOT depend on
`marque-engine` or on the sealed `marque_scheme::canonical::sealed`
module.

When the engine processes a `Diagnostic { fix: Some(intent), .. }`:
1. The engine's `EngineConstructor<S>` (the only `impl
   CanonicalConstructor<S> for ...`) is in scope.
2. The engine calls `S::render_canonical::<EngineConstructor<S>>(&intent, &ctx)`.
3. The scheme's render impl reads the intent, constructs a
   `Canonical<S>` via either `Canonical::from_cve(TokenId, Scope)` (for
   `ReplacementIntent::Cve`) or `EngineConstructor::build_open_vocab(...)`
   (for `ReplacementIntent::Render`).
4. The engine wraps the resulting `Canonical<S>` in a `FixReplacement`
   (Strict | Decoder discriminant, picked from the recognizer that
   produced the parse) and calls `AppliedFix::__engine_promote(...)`
   to construct the audit record.

External rule crates never see `EngineConstructor<S>` and cannot
implement `CanonicalConstructor<S>` themselves (sealed). The closure
property holds across crate boundaries.

---

## Migration from `FixProposal`

PR 3c retires `FixProposal` from the rule-API surface. The migration
shape:

```rust
// PRE (current):
FixProposal::new(span, original_bytes, replacement_str, confidence, source, migration_ref)

// POST (PR 3c):
Some(FixIntent {
    target_span: span,
    replacement: ReplacementIntent::Cve { token: ..., scope: Scope::Portion },
    confidence: Confidence::new(recognition, rule)?,
    feature_ids: smallvec![FeatureId::StrictExactMatch],
    message: Message::new(MessageTemplate::..., MessageArgs { ... }),
})
```

The pre-cutover `FixProposal::new(..., "", replacement, ...)` carve-out
at `engine.rs::build_decoder_diagnostic` (the `proposal.original = ""`
branch — currently `engine.rs:1369-1384`) deletes at PR 3c (FR-028);
the decoder path produces `FixIntent` values like any other rule, and
the engine renders them through the same path.

---

## Compile-time guarantees

The contract is type-enforced at the workspace boundary:

1. **Audit-record content-ignorance (FR-002)**: `MessageArgs` field set is closed; `AppliedFix::message` is `Message`, not `String`. Compile-fail tests demonstrate that `impl From<&str> for Message` and `impl From<String> for MessageArgs` do not exist.
2. **Audit-promotion engine-only (FR-005)**: `AppliedFix::__engine_promote` and `EnginePromotionToken::__engine_construct` are `pub #[doc(hidden)]`; the AST-based promote-callsite lint (FR-040) catches any production-code call site outside `Engine::fix_inner`. Each test-fixture carve-out call site MUST carry an inline comment naming the carve-out (e.g., `// Test-fixture carve-out per Constitution V`); the lint verifies the comment is present within 5 lines of the call.
3. **Open-vocab Canonical sealed (FR-001, FR-027)**: `Canonical::from_render` is `pub(crate)` to `marque-scheme`; the sealed `CanonicalConstructor<S>` trait closes the cross-crate door.
4. **Phase span-shape (FR-021)**: enforced at `Engine::new` registration; violating rules fail to register.

These compile-time guarantees plus the runtime invariants (I-3 non-
overlap, I-4 pass-2 reads post-pass-1 buffer, I-18 / I-19 pass-split
invariants) compose into the spec's SC-001, SC-007, SC-012 measurable
outcomes.
