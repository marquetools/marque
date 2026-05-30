# Contract: Reversibility Pre-State (#824 — rough-in only)

**Realization is DEFERRED (#824).** This feature lands only the **reserved fields** in Phase 0 so
that the later reversal pass + audit-schema bump is *additive*, not breaking. The contract below
fixes the shape of those reserved fields.

## Reserved fix-intent pre-state (`marque-scheme`, Phase 0)

`ReplacementIntent`/`FactRef`/`RecanonScope` are defined in `marque-scheme`
(`crates/scheme/src/fix_intent.rs`), kept there rather than in `marque-rules` to avoid a
scheme↔rules dependency cycle. The reserved fields below land in that module.

```rust
pub enum ReplacementIntent<S: MarkingScheme + ?Sized> {
    // Self-inverting today: the removed/added facts ARE the inverse.
    FactAdd    { token: FactRef<S>, scope: Scope },
    FactRemove { facts: SmallVec<[FactRef<S>; 2]>, scope: Scope },

    // NEW reserved field — Recanonicalize was not invertible (it didn't store prior form).
    Recanonicalize { scope: RecanonScope, prior: Option<RecanonPriorState<S>> },

    // NEW variant (relocate-not-evict, research D8) — carries pre-state to invert the move.
    Relocate { from: Scope, to: Scope, token: FactRef<S>, prior: RelocatePriorState<S> },
}

// Pre-state in audit-permitted terms ONLY (Constitution V / G13): token canonicals, category IDs,
// `Span` offsets (marque_scheme::Span), BLAKE3 digests. NO free-form content.
pub struct RecanonPriorState<S: MarkingScheme + ?Sized> { pub prior_tokens: Box<[FactRef<S>]>, pub prior_span: Span, pub digest: [u8; 32] }
pub struct RelocatePriorState<S: MarkingScheme + ?Sized> { pub token: FactRef<S>, pub origin_span: Span, pub digest: [u8; 32] }
```

**`Recanonicalize.prior` is `Option<_>` in Phase 0 on purpose, and that bounds what SC-006
claims.** `Recanonicalize` already exists in-tree without pre-state; making the field
non-optional would break every existing construction site at once, so Phase 0 lands it as
`Option` (additive — existing sites pass `None`). A `prior: None` record is therefore **not**
reversible from the log. Two consequences, both explicit:

- **SC-006 is scoped to intents whose pre-state is populated** — it verifies the round-trip for
  `FactAdd`/`FactRemove` (always self-inverting) and for `Recanonicalize`/`Relocate` whose
  `prior` is `Some(_)`. It does **not** claim every historical `Recanonicalize` is reversible.
- **#824 realization owns the tightening**: a dedicated migration task updates every
  `Recanonicalize` construction site to populate `prior`, after which #824 MAY make the field
  mandatory (or keep `Option` and have the reversal pass treat `None` as "not rewindable, fall
  back to the caller's retained original" — the same boundary as free-form text corrections,
  below). That decision lands with #824, not here.

## Two reversal classes (research D9 — informs realization, not landed here)

1. **Token-level fixes** (`NF → NOFORN`, `Recanonicalize`, `Relocate`) — **self-reversible from
   the audit log alone**; canonical tokens are on the G13 allow-list and are stored.
2. **Free-form text corrections** (`SERCET → SECRET`, the corrections map) — the pre-text is
   free-form content and **cannot** enter the audit record (content-ignorance). Reversible only
   against the **caller's retained original buffer** (Constitution II: Marque wipes the buffers it
   owns on drop; the original is the caller's to hold).

## Derivations vs. substitutions (research D9)

- Inverting a **substitution** is a token swap (#824, this contract).
- Inverting a **derivation** (source-derived `Declassify On`, #823) is a **recomputation**
  recorded via the `DecisionSink` cascade, not a stored token pair. Two mechanisms.

## Mode-gated apply (#645 M3 — lands with Phase F, gates #824 realization)

Reversibility turns "never auto-apply a contested resolution" into a *deployment-mode* decision:
- **Interactive editing** — MAY apply-and-rewind; pre-state recorded, nothing lost.
- **Network-boundary / egress audit** — still blocks (or requires confirmation): a rewind in the
  ledger does not un-transmit a document that already left with a wrong marking. The harm at
  egress is not in the log.

The disposition is recorded with full pre-state regardless of mode; whether it auto-applies is
policy.

## Schema impact (deferred)

Realization is an additive bump in the `marque-3.x` line; coordinate the `MARQUE_AUDIT_SCHEMA`
accept-list per the Stable API Surface section of `CLAUDE.md`. No new free-form surface — the G13
canary (`crates/engine/tests/audit_g13_canary.rs`) MUST still pass. **SC-006** verifies the Phase-0
reserved fields round-trip for token-level fixes and that the canary holds.
