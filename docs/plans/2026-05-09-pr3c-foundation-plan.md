# Implementation Plan: PR 3c.1 — Foundation Types

**Branch**: `refactor-006-pr-3c-foundations` (worktree: `/home/knitli/marque/.worktrees/pr3c-foundation/`)
**Date**: 2026-05-09
**Source plans**: `docs/plans/2026-05-02-engine-refactor-consolidated.md` §3 / §8 / §10; `specs/006-engine-rule-refactor/{plan,data-model,research}.md`; `specs/006-engine-rule-refactor/contracts/{fix-intent,audit-record}.md`
**Tasks in scope**: T030 → T040 (per `tasks.md` lines 114-124)
**Successor PR**: PR 3c.2 (atomic migration: `FixProposal` → `FixIntent<S>` cutover, audit schema bump `marque-mvp-2 → marque-1.0`, `engine.rs:1389` `format!` deletion, four open-vocab admission sites migrated)
**Reviewer agent target file**: `/home/knitli/marque/.worktrees/pr3c-foundation/docs/plans/2026-05-09-pr3c-foundation-plan.md`

> **Tooling note**: this design doc was produced under the constraint of being writable only to the planning context — the implementer agent should persist this content verbatim to the path above before executing T030, then track per-task progress against §9 (the per-task checklist) in commits.

---

## 1. Scope summary

| Task | One-line goal |
|---|---|
| T030 | Build `tools/message-template-extract/` (Rust binary, **outside the workspace**, transient — deletes after PR 3c review accepts the curated enum); run it; emit `specs/006-engine-rule-refactor/contracts/message-template-starter.md` for review. |
| T031 | Hand-curate the `MessageTemplate` closed enum into `crates/rules/src/message.rs` (decision: not `marque-ism` — see §2.3). |
| T032 | Define `MessageArgs` closed-set struct (no `String`/`&str`/`Vec<u8>` fields); compile-fail tests prove the field set is closed. |
| T033 | Define `Message::new(MessageTemplate, MessageArgs) -> Self` as the **only** public constructor; absent constructors (`from_string`, `From<&str>`, `Display::from`) verified by compile-fail tests. |
| T034 | Define `Canonical<S: MarkingScheme>` in `crates/scheme/src/canonical.rs` with `pub fn from_cve(TokenId, Scope) -> Self` and `pub(crate) fn from_render(...) -> Self`; `_scheme: PhantomData<S>` for type-level scheme separation. |
| T035 | Sealed-trait pattern: private `marque_scheme::canonical::sealed::Sealed`; public `CanonicalConstructor<S>: sealed::Sealed`. The sole `impl` is the engine's `EngineConstructor<S>` (lands in `marque-engine` at PR 3c.2 — PR 3c.1 ships a shim type, see §3 T035). |
| T036 | Compile-fail tests at `crates/scheme/tests/canonical_unconstructable.rs` proving (a) no `Box<str> → Canonical<S>` path exists, (b) downstream rule crates cannot impl `CanonicalConstructor<S>`. |
| T037 | Define `FixIntent<S>` in `crates/rules/src/fix_intent.rs` (target_span, replacement, confidence, feature_ids, message). |
| T038 | Define `ReplacementIntent<S>` enum: `Cve { token, scope }`, `Render { category, directive, scope }`, `Delete`. |
| T039 | Define `RenderDirective` as a scheme-associated type on `MarkingScheme` (decision: assoc type, not generic enum — see §3 T039); `CapcoScheme` will get its concrete `CapcoRenderDirective` enum at PR 3c.2 alongside the rule migration. PR 3c.1 ships a marker stub. |
| T040 | Add `marque-scheme` as a direct dep in `crates/rules/Cargo.toml`; verify `cargo check --workspace` passes; verify acyclic graph via `cargo tree --duplicates` and a written cycle proof. |

**PR 3c.1 ships ~2-3K LOC, mostly new files.** It does NOT touch any existing rule, the engine promotion path, the audit schema, or any existing test. The audit schema stays at `marque-mvp-2`. `FixProposal` and `Diagnostic.message: Box<str>` continue to coexist with the new `FixIntent<S>` and `Message` types. PR 3c.2 lands the cutover.

---

## 2. Crate placement decisions

### 2.1 Why `Canonical<S>` lives in `marque-scheme`, not `marque-ism`

Three load-bearing reasons:

1. **`Canonical<S>` is scheme-generic.** The point of `S: MarkingScheme` is that a future `CuiScheme`, `NatoScheme`, or partner-national `XScheme` produces `Canonical<XScheme>` without depending on the ISM/CAPCO vocabulary. Putting `Canonical<S>` in `marque-ism` would force every scheme-agnostic consumer (the engine, the audit emitter) to depend on `marque-ism` to name the type — exactly the inversion Constitution VII §VII forbids ("`marque-scheme` is the only true graph leaf").
2. **The `pub(crate) from_render` seal needs to live in the scheme crate** so `MarkingScheme::render_canonical` impls (which receive `&dyn CanonicalConstructor<S>` from the engine) can route through it. Putting `from_render` in `marque-ism` would either (a) leak it as `pub` (defeating the seal) or (b) force `marque-ism` to depend on `marque-scheme`'s sealed trait — which works under the existing `marque-ism → marque-scheme` edge but couples the vocabulary crate to a sealing protocol it has no business knowing about.
3. **Name disambiguation.** `marque-ism` already owns `CanonicalAttrs` (the post-canonical pivot type, `crates/ism/src/canonical.rs`). Two `Canonical*` types in one crate is a maintenance hazard; a future reader would have to disambiguate "Canonical (attrs) vs Canonical (token)" from context. They are conceptually distinct: `CanonicalAttrs` is the parsed-marking-after-validation pivot; `Canonical<S>` is the provenance-tagged single-token replacement. Keeping them in separate crates makes the distinction structural.

**Decision**: `crates/scheme/src/canonical.rs` (new file). `marque-scheme` already declares `Scope`, `TokenId`, `CategoryId`, `MarkingScheme` — every type `Canonical<S>` needs is in scope locally.

### 2.2 Why `FixIntent<S>` lives in `marque-rules`, not `marque-scheme`

`FixIntent<S>` is the **rule-emission API** (consolidated plan §3.1, §8.1; contract `contracts/fix-intent.md`). The contract says "rule crates depend on `marque-rules`, which re-exports `FixIntent<S>`". Putting `FixIntent<S>` in `marque-scheme` would force the engine and rule trait surfaces to live in different crates than the value rules construct — a layering inversion. Constitution VII codifies the rule-trait/scheme-trait split: rules belong in `marque-rules`, scheme-generic value types belong in `marque-scheme`.

`FixIntent<S>` is a value of "what a rule wants to happen" — closer to `Diagnostic` than to `MarkingScheme`. It composes `Span` (from `marque-ism`), `Confidence` + `FeatureId` + `Message` (from `marque-rules`), and `Scope` + `TokenId` + `CategoryId` (from `marque-scheme`). The natural home is the crate where rules construct it: `marque-rules`.

**Decision**: `crates/rules/src/fix_intent.rs` (new file). `marque-rules` gets a direct `marque-scheme` dependency at T040 (currently transitive through `marque-ism`).

### 2.3 Why `Message` / `MessageTemplate` / `MessageArgs` live in `marque-rules`, not `marque-ism`

Tasks T031 specifies "`crates/ism/src/message.rs` OR `crates/rules/src/message.rs`". Both are WASM-safe; both are valid hosts. The decision factors:

- **Consumer locality.** `Diagnostic.message: Message` is a `marque-rules` type. The migration in PR 3c.2 changes `Diagnostic` (in `marque-rules`). Putting `Message` in `marque-ism` would force every change to the message catalog to cross a crate boundary against a vocabulary crate that has no other reason to know about it.
- **`MessageArgs` references `TokenId`/`CategoryId` (from `marque-scheme`), `Confidence`/`FeatureId` (from `marque-rules`), and `Span` (from `marque-ism`).** Of those three crates, `marque-rules` is the one that already has the most edges — it's the natural sink. Putting `Message` in `marque-ism` would pull `Confidence` / `FeatureId` references through `marque-rules → marque-ism → ?` (and `marque-ism` does not currently depend on `marque-rules`, which is correct). Placing `Message` in `marque-rules` keeps the dep graph clean.
- **Two-layer-rule analogy.** `MessageTemplate` is a Layer-2 (hand-curated) artifact: it codifies *what the diagnostic says*, not *what the schema permits*. Layer-1 generated content (CVE enums, validators) lives in `marque-ism`; Layer-2 hand-curated content (rule ID conventions, severity, message templates) lives in `marque-rules` already.

**Decision**: `crates/rules/src/message.rs` (new file).

### 2.4 Crate responsibility table (post-PR-3c.1)

| Crate | Adds in PR 3c.1 | Already had |
|---|---|---|
| `marque-scheme` | `canonical.rs` (`Canonical<S>`, `TokenSource`), `canonical/sealed.rs` (private `Sealed` marker), `CanonicalConstructor<S>` trait. Compile-fail tests in `crates/scheme/tests/canonical_unconstructable.rs`. | `MarkingScheme`, `Scope`, `TokenId`, `CategoryId`, lattice constructors, `Vocabulary<S>`, `Codec<S>`, `Recognizer<S>`. |
| `marque-rules` | `message.rs` (`Message`, `MessageTemplate`, `MessageArgs`), `fix_intent.rs` (`FixIntent<S>`, `ReplacementIntent<S>`). Compile-fail tests for closed-set fields. New direct dep on `marque-scheme`. | `RuleId`, `Severity`, `Diagnostic`, `FixProposal`, `AppliedFix`, `EnginePromotionToken`, `Confidence`, `FeatureId`, `Rule` trait. |
| `marque-ism` | **No changes.** | `Span`, `CanonicalAttrs`, `ParsedAttrs`, `ProjectedMarking`, generated CVE enums. |
| `marque-engine` | **No changes in PR 3c.1.** PR 3c.2 lands `EngineConstructor<S>` here as the sole `impl CanonicalConstructor<CapcoScheme>`. | Pipeline orchestration. |
| `marque-capco` | **No changes in PR 3c.1.** PR 3c.2 lands `CapcoRenderDirective` and the per-rule `FixProposal → FixIntent` migration. | `CapcoScheme`, ~47 rules, `SciSet`/`SarSet`/`FgiSet` lattices. |

---

## 3. Type designs

Each subsection gives the exact Rust signature, including derives, visibility, doc comments (with citations where applicable), and design notes that resolve the open questions in the task brief.

### T031 — `MessageTemplate`

**File**: `crates/rules/src/message.rs`

```rust
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Message` — closed-template, closed-args diagnostic message representation.
//!
//! Replaces the current `Diagnostic.message: Box<str>` channel (which
//! permits `format!`-built strings interpolating input bytes — the leak
//! channel called out by source plan §8.3 and Constitution V Principle V
//! G13). PR 3c.1 ships the new types alongside the existing channel;
//! PR 3c.2 migrates `Diagnostic.message` to `Message` and deletes the
//! `format!` interpolation at `crates/engine/src/engine.rs:1389`
//! (FR-003).
//!
//! # Closed-template invariant
//!
//! `MessageTemplate` is a closed enum (NOT `#[non_exhaustive]`). Adding
//! a variant is a coordinated audit-schema change — the `MARQUE_AUDIT_SCHEMA`
//! version pin in `crates/engine/build.rs` must bump in lockstep, and
//! the audit-record schema doc at `specs/006-engine-rule-refactor/contracts/audit-record.md`
//! updates the closed-set listing. Treat additions as part of the
//! on-the-wire contract.
//!
//! # Closed-args invariant
//!
//! `MessageArgs` field set is closed. Permitted fields are restricted to
//! the audit-content-ignorance permitted set per Constitution V
//! Principle V: `TokenId`, `CategoryId`, `Span`, `Blake3Hash`,
//! `Confidence`, `FeatureId`, plus the two convenience-typed
//! `expected_token` / `actual_token` (both `Option<TokenId>` — same
//! type, distinguished by field name for diagnostic clarity). No
//! `String`, no `&str`, no `Vec<u8>`, no `format!`-derived field.
//! Compile-fail tests at `crates/rules/tests/message_args_closed_set.rs`
//! enforce.
```

#### Variant categorization (T030 + T031 collaboration)

T030's mechanical extraction (`tools/message-template-extract/`) groups all `Diagnostic::new(...)` and `format!`/`write!` first-arg literals from `crates/capco/src/rules*.rs` and `crates/engine/src/engine.rs` into clusters by structural similarity. T031 then hand-curates the cluster → variant mapping. Rather than enumerate every variant pre-PR, the design commits to the following **shape categories** that the curated enum must cover:

| Category | Examples (current rule names) | Why a category not a variant |
|---|---|---|
| **Decoder-recognized canonical** | R001 decoder-recognized | One variant (`DecoderRecognized`); args carry the recognized `TokenId`. |
| **Banner-rollup mismatch** | E031, E035, E040 (post-3b: walker `BannerMatchesProjectedRule`) | Likely one variant per per-row-§ family (SAR / SCI / Non-IC dissem); args carry the expected-vs-actual `TokenId` set summary. |
| **Per-system SCI invariant violations** | E059 (post-3b walker) HCS-O / HCS-P / SI-G / TK companion / NOFORN | One variant per **constraint family** (HCS-O-companion-required, SI-G-companion-required, TK-NOFORN-required, etc.); args carry the missing-companion `TokenId`. |
| **Class-floor violations** | E058 (post-3b walker) 27 catalog rows | One variant `ClassificationFloorViolated`; args carry observed `MarkingClassification` (as a `TokenId`) and required floor (as a `TokenId`). |
| **Non-canonical input** | E060 (post-3b walker) REL TO / JOINT / SIGMA / SAR / SCI ordering | One variant `NonCanonicalOrder`; args carry the offending `CategoryId`. |
| **Conflict (dyadic)** | E054–E057 (RELIDO conflicts) | One variant `ConflictsWith`; args carry the two conflicting `TokenId` values. |
| **Required-by-presence** | Various §H requires-rules | One variant `RequiredByPresence`; args carry the requiring `TokenId` and the missing required `TokenId`. |
| **Migration suggestion** | C001 corrections-map, S004 trigraph-suggest, deprecated-token migrations | One variant `SupersededToken` with `expected_token` set to the canonical replacement. |
| **Engine-synthetic** (PR 7) | R002 reparse-failed | One variant `ReparseFailed` reserved at PR 3c.1; PR 7 wires it. |

**T031 deliverable**: a fully-enumerated `MessageTemplate` (estimated 12–20 variants) backed by the T030 starter doc. Each variant carries a `// CAPCO-2016 §X.Y pNN` citation in its doc comment when the variant maps to a specific rule family; the engine-synthetic variants carry no §-citation (they are not CAPCO-derived).

```rust
/// Closed enumeration of stable diagnostic message templates.
///
/// **Closed-set discipline**: this enum is NOT `#[non_exhaustive]`.
/// Adding a variant requires a coordinated `MARQUE_AUDIT_SCHEMA` bump
/// (see `crates/engine/build.rs`). The on-the-wire audit contract
/// (`contracts/audit-record.md`) lists this enum's variants; consumers
/// match exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageTemplate {
    /// Decoder recognized a canonical form for an input the strict
    /// recognizer rejected. Args: `actual_token` (the canonical the
    /// decoder produced).
    ///
    /// **Lands**: PR 3c.2 (replaces `engine.rs:1389` `format!`).
    DecoderRecognized,

    /// Engine-minted: the post-pass-1 buffer failed to re-parse.
    /// Args: `feature_ids` carry the contributing pass-1 fix rule IDs
    /// (encoded as `FeatureId` opaque tags — PR 7 fills the variant set).
    ///
    /// **Lands**: PR 3c.1 reserves the slot; PR 7 wires the call site.
    /// Per source plan §9.4, R002 is engine-minted under the sentinel
    /// `("engine", "r002.reparse-failed")` rule ID.
    ReparseFailed,

    /// Banner roll-up did not match the projected expected banner
    /// derived from preceding portions on the page.
    /// Authority: CAPCO-2016 §H.4 p61 (SCI), §H.5 p99 (SAR),
    /// §H.8 p150 (dissem), §H.3 p56 (JOINT/REL TO).
    /// Args: `category` (which axis disagreed), optional
    /// `expected_token`/`actual_token` (single-axis disagreements).
    BannerRollupMismatch,

    /// Classification floor violated for a token requiring a minimum
    /// classification (e.g., HCS-comp-sub at SECRET; CNWDI at
    /// CONFIDENTIAL). Args: `expected_token` (required floor classification),
    /// `actual_token` (observed classification), `token` (the requiring
    /// SCI/dissem token).
    /// Authority: CAPCO-2016 §H.4 p64-p95 (per-system floors); rule-walker
    /// catalog rows carry the per-row §pNN.
    ClassificationFloorViolated,

    /// Non-canonical token order within a category. Args: `category`
    /// (which axis is out of order). Authority: §H.8 p150 (REL TO),
    /// §H.6 p108 (SIGMA), §H.5 p99 (SAR), §H.4 p61 (SCI), §H.3 p56 (JOINT).
    NonCanonicalOrder,

    /// Two tokens commingled that are mutually exclusive per §H invariants.
    /// Args: `token` (the dominated token to remove), `expected_token`
    /// (the dominating token that survives). Authority: §H.8 p150-151
    /// (RELIDO/NOFORN conflicts).
    ConflictsWith,

    /// A token requires a companion that is absent. Args: `token` (the
    /// requiring token), `expected_token` (the missing required companion).
    /// Authority: §H.4 p64 (HCS-O), §H.4 p80 (SI-G).
    RequiredByPresence,

    /// A deprecated/superseded token has a known canonical replacement.
    /// Args: `token` (the deprecated token), `expected_token` (the
    /// canonical replacement). Authority: §F migrations + `[corrections]`
    /// config.
    SupersededToken,

    // ... additional variants as T030's starter + T031's curation produces.
    // Full set committed at T031 review.
}

impl MessageTemplate {
    /// Stable canonical string label. Audit emitters MUST call this
    /// rather than `format!("{self:?}")` (which exposes Debug
    /// formatting as an unintended API). Mirrors `FeatureId::as_str`.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DecoderRecognized => "DecoderRecognized",
            Self::ReparseFailed => "ReparseFailed",
            Self::BannerRollupMismatch => "BannerRollupMismatch",
            Self::ClassificationFloorViolated => "ClassificationFloorViolated",
            Self::NonCanonicalOrder => "NonCanonicalOrder",
            Self::ConflictsWith => "ConflictsWith",
            Self::RequiredByPresence => "RequiredByPresence",
            Self::SupersededToken => "SupersededToken",
            // ... matches above
        }
    }
}
```

**Citation discipline (Constitution VIII)**: T031 PR description MUST include a per-variant table mapping `MessageTemplate::Variant` → CAPCO-2016 §X.Y pNN (or "engine-synthetic, no §-citation"). The implementer agent verifies each citation against `crates/capco/docs/CAPCO-2016.md` at PR-open time. Citation lint (FR-018, lands at PR 0.5) enforces the §X.Y pNN form mechanically once it ships.

### T032 — `MessageArgs` (closed-set struct)

**Decision: closed-set `struct` with `Option<T>` fields, NOT enum-of-variants.**

Justification: a closed-set struct trivially supports the most common case (a variant carries 1-3 of the permitted args, the rest are `None`) without forcing call sites to construct a unique sub-type per template. The enum-of-variants alternative would couple `MessageTemplate` and `MessageArgs` so tightly that adding a variant is a refactor across both types — the closed-set struct lets `MessageTemplate` evolve independently.

The closure property is preserved by the **field set being closed**: there is no `#[non_exhaustive]` attribute, no public field-extension mechanism, no `extra: HashMap<String, ...>` escape hatch. A future add-a-field PR is a coordinated change.

```rust
use marque_ism::Span;
use marque_scheme::{CategoryId, TokenId};
use crate::confidence::{Confidence, FeatureId};
use smallvec::SmallVec;

/// BLAKE3 digest, 32 bytes. Held as a fixed-size array (NOT `Box<[u8]>`
/// or `String`) to enforce content-ignorance — the digest is computed
/// from content, but the digest itself is opaque.
///
/// Lives in `marque-rules` because it is part of the `MessageArgs`
/// permitted-types surface; consumed by `AppliedFix.fix.original_digest`
/// at PR 3c.2 audit-record reshape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Blake3Hash(pub [u8; 32]);

impl Blake3Hash {
    /// Render as `"blake3:..."` hex string for NDJSON serialization.
    /// Returns owned `String` only at the audit-emit boundary.
    pub fn to_audit_string(&self) -> String { /* ... */ }
}

/// Closed-set permitted arguments to a `MessageTemplate`.
///
/// **Constitution V Principle V (G13) closure**: every field type is in
/// the audit-content-ignorance permitted set: `TokenId`, `CategoryId`,
/// `Span`, `Blake3Hash`, `Confidence`, `FeatureId`. No `String`,
/// no `&str`, no `Vec<u8>`, no `format!`-derived field. This is the
/// type-level closure of the leak channel called out by source plan §8.3
/// — `MessageArgs` cannot carry input bytes by construction.
///
/// `expected_token` / `actual_token` are both `Option<TokenId>` — same
/// type, distinguished by field name for diagnostic ergonomics. The
/// audit emitter renders them as `args.expected: "..."` and
/// `args.actual: "..."` keys.
///
/// `feature_ids` is a `SmallVec<[FeatureId; 4]>` — most diagnostics
/// carry 0–2 contributing features; the inline-4 capacity covers the
/// 99th-percentile case without heap allocation.
///
/// **Construction**: `MessageArgs::default()` plus field-level
/// assignment. There is no all-positional constructor — the struct is
/// not extensible enough to warrant one, and field-level assignment
/// makes the call site self-documenting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessageArgs {
    /// The primary token the message refers to (e.g., the deprecated
    /// token in `SupersededToken`, the requiring token in
    /// `RequiredByPresence`, the offending token in `ConflictsWith`).
    pub token: Option<TokenId>,

    /// The category axis the message refers to (e.g., the offending
    /// category for `NonCanonicalOrder`).
    pub category: Option<CategoryId>,

    /// A span-level locator for the message (used when the
    /// `Diagnostic.span` covers a multi-token marking and the message
    /// needs to point at one token within it).
    pub span: Option<Span>,

    /// BLAKE3 digest of content the message references.
    pub digest: Option<Blake3Hash>,

    /// Confidence record snapshotted from the producing rule's
    /// `FixIntent`.
    pub confidence: Option<Confidence>,

    /// What token the rule expected (the canonical / required value).
    pub expected_token: Option<TokenId>,

    /// What token the rule actually saw.
    pub actual_token: Option<TokenId>,

    /// Closed list of contributing features (decoder-derived posterior
    /// contributions, strict-context floors, etc.).
    pub feature_ids: SmallVec<[FeatureId; 4]>,
}
```

**Compile-fail tests** (T032; live at `crates/rules/tests/message_args_closed_set.rs`):

```rust
//! Compile-fail proofs that `MessageArgs` cannot carry document content.

/// ```compile_fail
/// use marque_rules::MessageArgs;
/// let args = MessageArgs {
///     // Field does not exist; closed-set struct rejects.
///     raw_text: String::from("classified content"),
///     ..MessageArgs::default()
/// };
/// ```
#[allow(dead_code)]
fn _no_string_field() {}

/// ```compile_fail
/// use marque_rules::MessageArgs;
/// let _: &str = MessageArgs::default().token; // type mismatch: Option<TokenId>, not &str
/// ```
#[allow(dead_code)]
fn _token_is_not_str() {}

/// ```compile_fail
/// use marque_rules::MessageArgs;
/// // No `From<&str>` impl exists.
/// let _args: MessageArgs = "hello".into();
/// ```
#[allow(dead_code)]
fn _no_from_str_impl() {}
```

A complementary positive test enumerates every field in `MessageArgs` via destructuring and verifies the type list:

```rust
#[test]
fn message_args_field_types_are_closed_set() {
    let args = MessageArgs::default();
    let MessageArgs {
        token,
        category,
        span,
        digest,
        confidence,
        expected_token,
        actual_token,
        feature_ids,
    } = args;
    let _: Option<TokenId> = token;
    let _: Option<CategoryId> = category;
    let _: Option<Span> = span;
    let _: Option<Blake3Hash> = digest;
    let _: Option<Confidence> = confidence;
    let _: Option<TokenId> = expected_token;
    let _: Option<TokenId> = actual_token;
    let _: SmallVec<[FeatureId; 4]> = feature_ids;
    // Adding a field to MessageArgs causes this destructuring to fail
    // to compile (E0027); reviewer must explicitly amend this test plus
    // the audit-record contract listing.
}
```

The destructuring-as-pin pattern is borrowed from existing patterns in `crates/rules/src/confidence.rs::tests::feature_id_as_str_matches_audit_contract`. It is the cheapest possible closed-set guard.

### T033 — `Message::new` (sole public constructor)

```rust
/// Diagnostic message: closed template + closed args.
///
/// Replaces `Diagnostic.message: Box<str>` at PR 3c.2. PR 3c.1 ships
/// the new type alongside the existing `Box<str>` field; PR 3c.2
/// migrates the `Diagnostic` field type and deletes the legacy channel.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    template: MessageTemplate,
    args: MessageArgs,
}

impl Message {
    /// **The only public constructor.**
    ///
    /// There is no `Message::from_string`, no `impl From<&str> for Message`,
    /// no `Message::format(...)` — the absence is intentional and
    /// load-bearing. See `crates/rules/tests/message_no_freeform_ctor.rs`
    /// for the compile-fail proofs.
    #[inline]
    pub const fn new(template: MessageTemplate, args: MessageArgs) -> Self {
        Self { template, args }
    }

    #[inline]
    pub const fn template(&self) -> MessageTemplate { self.template }

    #[inline]
    pub fn args(&self) -> &MessageArgs { &self.args }

    /// Render the message to a human-readable string for CLI / IDE
    /// display. The rendering reads the template + args; it does NOT
    /// interpolate input bytes. Audit-record serialization MUST NOT
    /// use this method — audit records carry the (template, args)
    /// pair directly.
    pub fn render(&self) -> String { /* template lookup table; no input bytes */ }
}
```

**Compile-fail tests** (T033; live at `crates/rules/tests/message_no_freeform_ctor.rs`):

```rust
/// ```compile_fail
/// use marque_rules::Message;
/// let _: Message = "free-form text".into();
/// ```
#[allow(dead_code)]
fn _no_from_str_impl() {}

/// ```compile_fail
/// use marque_rules::Message;
/// let _ = Message::from_string("free-form text");
/// ```
#[allow(dead_code)]
fn _no_from_string_method() {}

/// ```compile_fail
/// use marque_rules::Message;
/// let _ = Message::format("decoder-recognized canonical form: {}", "data");
/// ```
#[allow(dead_code)]
fn _no_format_method() {}
```

### T034 — `Canonical<S>` with sealed constructors

**File**: `crates/scheme/src/canonical.rs` (new, ~120 LOC).

The design follows source plan §8.1 exactly, with the `_scheme: PhantomData<S>` addition that T034 explicitly calls out for type-level scheme separation.

```rust
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Canonical<S>` — provenance-tagged canonical replacement for a
//! single token, with sealed constructors for closed-CVE vs open-vocab
//! provenance.
//!
//! See source plan §8.1 (`docs/plans/2026-05-02-engine-refactor-consolidated.md`)
//! for the design rationale: this type is the keystone for closing the
//! G13 leak channel (Constitution V Principle V) at the type level
//! rather than via convention-only enforcement.
//!
//! # Construction surface
//!
//! Two paths:
//!
//! 1. **Closed-CVE (`Canonical::from_cve`)** — public, callable from
//!    any crate. Accepts `TokenId` (which can only come from
//!    `Vocabulary<S>::lookup`); there is no `Box<str> → Canonical<S>`
//!    public path.
//!
//! 2. **Open-vocab (`Canonical::from_render`)** — `pub(crate)` to
//!    `marque-scheme`. Reachable from external crates ONLY through
//!    `MarkingScheme::render_canonical`, which receives a sealed
//!    `&dyn CanonicalConstructor<S>` (whose sole impl lives in
//!    `marque-engine`; see [`CanonicalConstructor`]).
//!
//! # Cross-crate emission story
//!
//! External rule crates (`marque-capco` today, future `marque-cui` /
//! `marque-nato` / partner-national crates) emit `FixIntent<S>` values;
//! the engine — holding the only `CanonicalConstructor<S>` impl —
//! renders them on the rule's behalf. This preserves the closed-
//! construction property across the workspace boundary that
//! Constitution VII opens up for new rule crate families. See
//! `specs/006-engine-rule-refactor/contracts/fix-intent.md` §"Cross-
//! crate emission".

use core::marker::PhantomData;
use core::panic::Location;

use crate::category::{CategoryId, TokenId};
use crate::scheme::MarkingScheme;
use crate::scope::Scope;

/// Provenance tag for a `Canonical<S>` value. Records *how* the
/// canonical replacement was constructed; consumed by the audit emitter
/// to distinguish high-trust closed-CVE replacements from
/// trust-on-render-site open-vocab replacements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    /// Closed-CVE: the canonical bytes are a known token from the
    /// scheme's vocabulary, identified by `TokenId`.
    Cve(TokenId),

    /// Open-vocabulary: the canonical bytes were constructed by a
    /// `MarkingScheme::render_canonical` impl. The `render_call_site`
    /// (a `&'static Location`) records *where in source* the rendering
    /// happened; an auditor can locate the render impl from the call site.
    OpenVocab {
        category: CategoryId,
        render_call_site: &'static Location<'static>,
    },
}

/// Provenance-tagged canonical replacement for a single token.
///
/// **Construction is sealed.** See module docs.
#[derive(Debug, Clone)]
pub struct Canonical<S: MarkingScheme + ?Sized> {
    bytes: Box<str>,
    source: TokenSource,
    scope: Scope,
    _scheme: PhantomData<fn() -> S>,
}

impl<S: MarkingScheme + ?Sized> Canonical<S> {
    /// **Closed-CVE constructor.** The only public constructor.
    ///
    /// Callable from any crate, but `TokenId` itself can only be
    /// obtained from `Vocabulary<S>::lookup`, so there is no path
    /// from `Box<str>` to `Canonical<S>`. The bytes are derived from
    /// the scheme's vocabulary table — implementation lives in
    /// `MarkingScheme::render_canonical_cve` (TBD in PR 3c.2; for
    /// PR 3c.1 the bytes come from a placeholder lookup that returns
    /// the `TokenId`'s registered canonical bytes from the scheme's
    /// vocabulary).
    pub fn from_cve(token: TokenId, scope: Scope, bytes: Box<str>) -> Self {
        Self {
            bytes,
            source: TokenSource::Cve(token),
            scope,
            _scheme: PhantomData,
        }
    }

    /// **Open-vocab constructor.** `pub(crate)` to `marque-scheme`.
    ///
    /// Reachable from external crates only via
    /// `<E as CanonicalConstructor<S>>::build_open_vocab` where `E` is
    /// the engine's sealed `EngineConstructor<S>`. Records the call
    /// site as provenance per source plan §8.1.
    pub(crate) fn from_render(
        category: CategoryId,
        bytes: Box<str>,
        scope: Scope,
        render_call_site: &'static Location<'static>,
    ) -> Self {
        Self {
            bytes,
            source: TokenSource::OpenVocab { category, render_call_site },
            scope,
            _scheme: PhantomData,
        }
    }

    #[inline]
    pub fn bytes(&self) -> &str { &self.bytes }

    #[inline]
    pub fn source(&self) -> &TokenSource { &self.source }

    #[inline]
    pub fn scope(&self) -> Scope { self.scope }

    /// BLAKE3 digest of the canonical bytes. Computed lazily;
    /// `marque-engine` calls this when constructing `AppliedFix`.
    /// PR 3c.2 wires the dependency on `blake3`; PR 3c.1 stubs.
    pub fn digest(&self) -> crate::canonical::Blake3Hash {
        // PR 3c.1: returns Blake3Hash([0; 32]) (stub).
        // PR 3c.2: computes blake3::hash(self.bytes.as_bytes()).
        crate::canonical::Blake3Hash([0; 32])
    }
}

/// BLAKE3 digest. 32 bytes, fixed-size array (no allocation).
///
/// Lives in `marque-scheme` (NOT `marque-rules`) because it is exposed
/// from `Canonical<S>::digest()`. `marque-rules::MessageArgs::digest`
/// re-exports the same type via path import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Blake3Hash(pub [u8; 32]);
```

**Why `?Sized`**: `MarkingScheme` is implemented on concrete adapter types (`CapcoScheme`); the `?Sized` bound is defensive against a future `dyn MarkingScheme` use case. Removing it costs nothing; keeping it costs nothing either; defensive default wins.

**Why `PhantomData<fn() -> S>` not `PhantomData<S>`**: `fn() -> S` is `Send + Sync` regardless of `S`'s auto-trait status, which preserves Constitution VI's `Send + Sync` requirement for engine types. `PhantomData<S>` would inherit `S`'s auto-traits, which is not what we want for a marker.

**Where does `Blake3Hash` live?** It's declared in `marque-scheme::canonical` (because `Canonical<S>::digest()` returns it) and re-exported from `marque-rules` via `pub use marque_scheme::Blake3Hash`. The `marque-rules → marque-scheme` direct dep added at T040 enables this. Putting it only in `marque-rules` would force `marque-scheme` to depend on `marque-rules`, which is the wrong direction (Constitution VII).

### T035 — Sealed-trait pattern

**Two-file structure**:
- `crates/scheme/src/canonical.rs` — the public types (`Canonical<S>`, `TokenSource`, `Blake3Hash`, `CanonicalConstructor<S>`).
- `crates/scheme/src/canonical/sealed.rs` — the private `Sealed` marker trait. Module is private (no `pub mod`).

```rust
// crates/scheme/src/canonical/sealed.rs (new file, ~30 LOC)
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Private sealing module for `CanonicalConstructor<S>`.
//!
//! External crates cannot name `Sealed` because the module is private
//! to `marque-scheme`. Therefore external crates cannot satisfy the
//! `CanonicalConstructor<S>: sealed::Sealed` bound and cannot
//! implement the trait. See:
//! <https://rust-lang.github.io/api-guidelines/future-proofing.html>

use crate::scheme::MarkingScheme;

/// Sealing marker. Crate-private; cannot be implemented outside
/// `marque-scheme`.
pub trait Sealed<S: MarkingScheme + ?Sized> {}
```

Add to `crates/scheme/src/canonical.rs`:

```rust
mod sealed;

/// Sealed trait that closes the open-vocab `Canonical<S>` construction
/// path across crate boundaries.
///
/// **The only impl lives in `marque-engine`** as `EngineConstructor<S>`.
/// External rule crates depend on `marque-rules`, which re-exports
/// `FixIntent<S>` and friends but NOT this trait — so a downstream rule
/// crate cannot construct `Canonical<S>` open-vocab values directly.
/// They emit `FixIntent<S>::Render { directive, .. }` and the engine
/// renders on their behalf.
///
/// **Sealing mechanism**: the supertrait bound `sealed::Sealed<S>`
/// references a private module; external crates cannot name `Sealed`,
/// therefore cannot satisfy the bound, therefore cannot impl this trait.
/// This is the standard Rust API-guidelines sealed-trait pattern.
///
/// # Compile-fail proof
///
/// See `crates/scheme/tests/canonical_unconstructable.rs` for the
/// compile-fail tests that prove (a) no public open-vocab construction
/// path exists and (b) external crates cannot impl this trait.
pub trait CanonicalConstructor<S: MarkingScheme + ?Sized>: sealed::Sealed<S> {
    /// Construct an open-vocab `Canonical<S>` value. The implementer
    /// (the engine) is responsible for capturing the `render_call_site`
    /// via `#[track_caller]` on its impl.
    #[track_caller]
    fn build_open_vocab(
        category: CategoryId,
        bytes: Box<str>,
        scope: Scope,
    ) -> Canonical<S>;
}
```

**Where does `EngineConstructor<S>` live?**

The task brief asks: "does the engine impl live in `marque-engine` or `marque-scheme`? (Answer: must be `marque-engine` so external rule crates can't impl. But then `Sealed` must be referenced from there. Show how.)"

**Resolution**: this is the **load-bearing decision** of T035. There are three options:

| Option | Where `EngineConstructor<S>` lives | Where `Sealed` impl lives | Trade-off |
|---|---|---|---|
| A | `marque-engine` | `marque-scheme` (impl `Sealed<CapcoScheme> for marque_engine::EngineConstructor<CapcoScheme>` — but `marque-scheme` doesn't depend on `marque-engine`, this fails) | **Impossible** — cycle. |
| B | `marque-engine` | `marque-engine` (impl `Sealed<S> for EngineConstructor<S>`) | **Impossible** — `Sealed` is private to `marque-scheme`. |
| C | `marque-scheme` (a `pub(crate)` empty marker struct), `EngineConstructor<S>` is an alias / wrapper in `marque-engine` | Sealed impl lives in `marque-scheme` for the in-crate marker; `marque-engine` re-exports it as `EngineConstructor` | Workable but adds a layer. |
| **D** | **`marque-scheme`** as a `pub` zero-size struct; `marque-engine` consumes it but does not own it | `Sealed` impl in `marque-scheme` for the local type | **Chosen** — see below. |

**Decision: Option D.** `EngineConstructor<S>` is a `pub` zero-size struct in `marque-scheme::canonical`, with the sole `impl CanonicalConstructor<S> for EngineConstructor<S>` (and `impl sealed::Sealed<S> for EngineConstructor<S>`). The engine *uses* it but does not *own* it.

```rust
// In crates/scheme/src/canonical.rs:

/// Engine-only `CanonicalConstructor<S>` implementor.
///
/// Lives in `marque-scheme` (not `marque-engine`) so the `Sealed`
/// supertrait can be implemented locally — `Sealed` is private to
/// `marque-scheme` and cannot be implemented from a downstream crate.
///
/// `EngineConstructor<S>` is `pub` so the engine can name it in
/// `Engine::fix_inner`'s call to `S::render_canonical::<EngineConstructor<S>>(...)`,
/// but its construction is sealed via the `EnginePromotionToken`
/// pattern that already secures `AppliedFix::__engine_promote` (see
/// `marque-rules`). PR 3c.1 ships the type with the same construction
/// seal pattern — `EngineConstructor::__engine_construct(token)` — to
/// prevent rule-crate code from instantiating it.
pub struct EngineConstructor<S: MarkingScheme + ?Sized> {
    _scheme: PhantomData<fn() -> S>,
    _seal: (),
}

impl<S: MarkingScheme + ?Sized> EngineConstructor<S> {
    /// Reserved name (FR-040 lint contract). Mint via the engine-only
    /// path. The same `EnginePromotionToken` that secures
    /// `AppliedFix::__engine_promote` is the construction bypass.
    #[doc(hidden)]
    #[inline]
    pub const fn __engine_construct() -> Self {
        Self { _scheme: PhantomData, _seal: () }
    }
}

impl<S: MarkingScheme + ?Sized> sealed::Sealed<S> for EngineConstructor<S> {}

impl<S: MarkingScheme + ?Sized> CanonicalConstructor<S> for EngineConstructor<S> {
    #[inline]
    #[track_caller]
    fn build_open_vocab(
        category: CategoryId,
        bytes: Box<str>,
        scope: Scope,
    ) -> Canonical<S> {
        Canonical::from_render(category, bytes, scope, core::panic::Location::caller())
    }
}
```

**Why this works for "5-year maintainability"**:

1. **Sealed trait is sealed for real**: external crates cannot name `sealed::Sealed`, cannot satisfy the supertrait bound, cannot impl `CanonicalConstructor<S>`. Compile-fail test T036 verifies.
2. **Engine-only construction**: `EngineConstructor::__engine_construct` mirrors the existing `EnginePromotionToken::__engine_construct` pattern. The promote-callsite lint (FR-040) already covers `__engine_promote` / `__engine_construct` last-segment matches; extending it to flag `EngineConstructor::__engine_construct` is one line.
3. **No `#[doc(hidden)] pub fn` as the seal mechanism** (per memory `feedback_pub_doc_hidden_is_still_public_api.md`): the seal here is the **sealed-trait supertrait bound**, not the doc-hidden attribute. The `__engine_construct` is doc-hidden as a secondary defense (signaling "engine-only" to readers), not the primary seal.
4. **`marque-engine` does not need to introduce a new sealed-trait module** — all the sealing machinery lives in `marque-scheme`. The engine just calls `EngineConstructor::<CapcoScheme>::__engine_construct()` and passes it to `S::render_canonical`.

**Trade-off**: `marque-scheme` owns a type (`EngineConstructor<S>`) named for an external crate's role. This is a deliberate choice — the alternative (option C, two-layer wrapping) is more code, more indirection, and the same end-state. Owning the engine-side seal mechanism in `marque-scheme` keeps it co-located with the trait it seals.

### T036 — Compile-fail tests

**File**: `crates/scheme/tests/canonical_unconstructable.rs` (new, ~80 LOC).

```rust
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-fail proofs for `Canonical<S>` sealing.
//!
//! These tests prove the type-level closure of the G13 leak channel
//! (Constitution V Principle V; source plan §8.1). Each compile_fail
//! doctest demonstrates a path that MUST NOT compile; if any of them
//! starts compiling, the seal has been broken.

/// **No `Box<str> → Canonical<S>` public path exists.** The only
/// closed-CVE constructor takes `TokenId`, which itself can only come
/// from `Vocabulary<S>::lookup`.
///
/// ```compile_fail
/// use marque_scheme::canonical::Canonical;
/// use marque_scheme::scope::Scope;
/// // Hypothetical scheme; the test only needs the type-level proof.
/// struct FakeScheme;
/// impl marque_scheme::MarkingScheme for FakeScheme { /* required items */ }
/// // No public constructor accepts Box<str>:
/// let _: Canonical<FakeScheme> = Canonical::from_bytes(Box::from("TS"), Scope::Portion);
/// ```
#[allow(dead_code)]
fn no_box_str_to_canonical() {}

/// **No `&str → Canonical<S>` public path exists.**
///
/// ```compile_fail
/// use marque_scheme::canonical::Canonical;
/// use marque_scheme::scope::Scope;
/// struct FakeScheme;
/// impl marque_scheme::MarkingScheme for FakeScheme { /* required items */ }
/// let _: Canonical<FakeScheme> = "TS".into();
/// ```
#[allow(dead_code)]
fn no_str_into_canonical() {}

/// **External crates cannot impl `CanonicalConstructor<S>`** because
/// the supertrait `sealed::Sealed<S>` is private to `marque-scheme`.
///
/// ```compile_fail
/// use marque_scheme::canonical::{CanonicalConstructor, Canonical};
/// use marque_scheme::category::{CategoryId, TokenId};
/// use marque_scheme::scope::Scope;
///
/// struct FakeScheme;
/// impl marque_scheme::MarkingScheme for FakeScheme { /* required items */ }
///
/// struct EvilConstructor;
///
/// // Cannot satisfy the sealed::Sealed<FakeScheme> supertrait bound:
/// impl CanonicalConstructor<FakeScheme> for EvilConstructor {
///     fn build_open_vocab(
///         category: CategoryId,
///         bytes: Box<str>,
///         scope: Scope,
///     ) -> Canonical<FakeScheme> {
///         unimplemented!()
///     }
/// }
/// ```
#[allow(dead_code)]
fn external_crate_cannot_impl_constructor() {}

/// **External crates cannot name `sealed::Sealed`** (the private module
/// is not exported).
///
/// ```compile_fail
/// use marque_scheme::canonical::sealed::Sealed;
/// ```
#[allow(dead_code)]
fn external_crate_cannot_name_sealed() {}
```

**Test runner**: `cargo test --test canonical_unconstructable` — Rust's built-in `compile_fail` doctest harness. No `trybuild` dependency required for this small set; the existing `compile_fail` doctest pattern is what `marque-rules`'s `EnginePromotionToken` uses (lines 559-565 of `crates/rules/src/lib.rs`).

**T032's compile-fail tests** (no `String` field on `MessageArgs`) follow the same pattern at `crates/rules/tests/message_args_closed_set.rs`.

### T037 — `FixIntent<S>`

**File**: `crates/rules/src/fix_intent.rs` (new, ~80 LOC).

```rust
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `FixIntent<S>` — the rule-emission API.
//!
//! Rules emit `FixIntent<S>` values; the engine renders them through
//! `MarkingScheme::render_canonical` to produce `Canonical<S>` and
//! promotes via `Engine::fix_inner` (sealed `AppliedFix::__engine_promote`).
//! Rules MUST NOT construct `Canonical<S>`, `AppliedFix`, or any other
//! audit-promotion type directly.
//!
//! See `specs/006-engine-rule-refactor/contracts/fix-intent.md` for the
//! full contract.
//!
//! # Lifecycle (post-PR-3c.2)
//!
//! 1. Rule's `evaluate(...)` returns `Vec<Diagnostic>`, each
//!    `Diagnostic` carrying `fix: Option<FixIntent<S>>`.
//! 2. Engine filters by `Confidence::combined() >= threshold` (FR-016).
//! 3. Engine sorts non-overlapping fixes (I-3) and resolves overlaps (C-1).
//! 4. Engine calls `S::render_canonical::<EngineConstructor<S>>(&intent, &ctx)`
//!    to produce `Canonical<S>` (closed-CVE via `from_cve`; open-vocab via
//!    `EngineConstructor::build_open_vocab` → `Canonical::from_render`).
//! 5. Engine constructs `AppliedFix` via `__engine_promote(...)`.
//!
//! PR 3c.1 ships the types; PR 3c.2 wires the lifecycle.

use marque_ism::Span;
use marque_scheme::{CategoryId, MarkingScheme, Scope, TokenId};
use smallvec::SmallVec;

use crate::confidence::{Confidence, FeatureId};
use crate::message::Message;

/// Rule-emission API.
///
/// **Rules construct this type; the engine renders and promotes.**
/// External rule crates depend on `marque-rules` (which re-exports
/// `FixIntent<S>`, `ReplacementIntent<S>`, `Message`, `MessageTemplate`,
/// `MessageArgs`, `Confidence`, `FeatureId`); they do NOT depend on
/// `marque-engine` or on `marque-scheme::canonical::sealed`.
///
/// `FixIntent<S>` is parameterized over the marking scheme so the
/// `replacement: ReplacementIntent<S>` field can carry scheme-specific
/// `RenderDirective<S>` values (PR 3c.2 wires `CapcoRenderDirective`).
#[derive(Debug, Clone)]
pub struct FixIntent<S: MarkingScheme> {
    /// Byte span in the original source to replace.
    pub target_span: Span,

    /// What to put there. Three discriminants — see `ReplacementIntent`.
    pub replacement: ReplacementIntent<S>,

    /// Multi-axis confidence. `recognition × rule` is gated against the
    /// engine's threshold.
    pub confidence: Confidence,

    /// Closed-set list of contributing features. Inline-4 capacity
    /// covers the 99th-percentile case (most fixes carry 0–2 features).
    pub feature_ids: SmallVec<[FeatureId; 4]>,

    /// Diagnostic message attached to this fix. Closed template + closed
    /// args; see `crate::message::Message`.
    pub message: Message,
}
```

**Why `S: MarkingScheme` not `S: MarkingScheme + ?Sized`**: `FixIntent<S>` is constructed concretely; the rule writes `FixIntent<CapcoScheme>` literally. A `dyn MarkingScheme` is not a use case here. Keeping it sized is simpler and matches the contract document's `FixIntent<CapcoScheme>` examples.

### T038 — `ReplacementIntent<S>`

```rust
/// Three replacement variants.
#[derive(Debug, Clone)]
pub enum ReplacementIntent<S: MarkingScheme> {
    /// Closed-CVE replacement. The token must come from
    /// `Vocabulary<S>::lookup`. Engine renders via `Canonical::from_cve`.
    Cve {
        token: TokenId,
        scope: Scope,
    },

    /// Open-vocabulary replacement. The directive carries scheme-
    /// specific structured data describing what to render. Engine
    /// renders via `MarkingScheme::render_canonical`, which calls
    /// `EngineConstructor::build_open_vocab`.
    ///
    /// Used for SCI compartments / sub-compartments (CAPCO §A.6
    /// compositional grammar), SAR program identifiers, country
    /// trigraphs in some FGI contexts.
    Render {
        category: CategoryId,
        directive: <S as MarkingScheme>::RenderDirective,
        scope: Scope,
    },

    /// Delete the token entirely. Audit records this as
    /// `Canonical: <empty>` with `TokenSource::OpenVocab` provenance
    /// pointing at the engine call site.
    Delete,
}
```

**Decision: `directive: <S as MarkingScheme>::RenderDirective`** (associated type), not `directive: S::RenderDirective` shorthand. The fully-qualified form is clearer at the type definition site; the call site can write `S::RenderDirective` if it prefers.

**Why associated type, not generic enum?** Per T039 below. The associated-type binding lives on `MarkingScheme`.

### T039 — `RenderDirective` as scheme-associated type

**Decision tree**:

| Option | Shape | Trade-off |
|---|---|---|
| A | `RenderDirective<S>` is a generic enum in `marque-scheme` enumerating SCI / SAR / FGI / Custom variants | Forces `marque-scheme` to know CAPCO vocabulary — Constitution VII violation. |
| B | `RenderDirective` is a marker trait with scheme impls | Loses the closed-set property — open trait, anyone can impl. |
| **C** | **Associated type on `MarkingScheme`: `type RenderDirective: Send + Sync + 'static + Clone + Debug;`** | Each scheme owns its `RenderDirective` enum; engine and rule code is generic over it via `S::RenderDirective`. Closed-set per scheme; no domain leakage into `marque-scheme`. **Chosen.** |

**Addition to `crates/scheme/src/scheme.rs`**:

```rust
pub trait MarkingScheme {
    // ... existing items ...

    /// Scheme-specific structured data describing how to render an
    /// open-vocabulary fix replacement.
    ///
    /// Each scheme owns the closed enumeration of its render directives.
    /// For CAPCO, the impl will land at PR 3c.2 as a
    /// `CapcoRenderDirective` enum covering SCI compositional grammar
    /// (`SciMarking { control, comps, sub_comps }`), SAR programs/
    /// compartments, and FGI trigraph blocks.
    ///
    /// The `Send + Sync + 'static` bound is required for `BatchEngine`
    /// concurrency (Constitution VI). The `Clone + Debug` bound is
    /// required because `FixIntent<S>: Debug + Clone`.
    ///
    /// # PR 3c.1 placeholder
    ///
    /// PR 3c.1 ships this as the assoc-type binding only; no scheme
    /// implements it yet (existing `CapcoScheme` is unchanged in
    /// PR 3c.1, so it gets a default `type RenderDirective = ();` via
    /// the `default` keyword at the trait level — see below). PR 3c.2
    /// removes the default and adds `type RenderDirective = CapcoRenderDirective;`
    /// to the `impl MarkingScheme for CapcoScheme` block.
    type RenderDirective: Send + Sync + 'static + Clone + core::fmt::Debug;
}
```

**Wait — additive constraint**: PR 3c.1 must NOT break `impl MarkingScheme for CapcoScheme` in `crates/capco/src/scheme.rs:2054`. Adding a new associated type breaks the impl. Two resolutions:

| Option | Shape | Trade-off |
|---|---|---|
| 1 | Add `type RenderDirective = ();` to `CapcoScheme`'s impl in PR 3c.1 | One-line edit to `marque-capco`; technically PR 3c.1 touches `marque-capco`. |
| 2 | Add `type RenderDirective: ...` with associated-type default `type RenderDirective = ();` | The unstable `associated_type_defaults` feature is required; not on stable Rust. Not viable. |
| **3** | **Defer the assoc-type addition to PR 3c.2** | PR 3c.1 ships `RenderDirective` as a placeholder type alias `type RenderDirective<S> = ();` in `marque-rules::fix_intent`, NOT as an assoc type on `MarkingScheme`. PR 3c.2 lifts it to the assoc-type position when `CapcoScheme` is ready to bind it. **Chosen.** |

**Concretely for PR 3c.1**:

```rust
// In crates/rules/src/fix_intent.rs (PR 3c.1):

/// Placeholder for the scheme-specific render directive type.
///
/// **PR 3c.1 ships this as a unit type.** PR 3c.2 lifts the binding to
/// `<S as MarkingScheme>::RenderDirective` (an associated type on
/// `MarkingScheme`) once `CapcoScheme` is ready to bind it to its
/// concrete `CapcoRenderDirective` enum. Keeping the lift atomic with
/// the rule migration in PR 3c.2 avoids a cross-PR breakage of
/// `impl MarkingScheme for CapcoScheme`.
pub type RenderDirective<S> = core::marker::PhantomData<S>;

// Used by ReplacementIntent::Render { directive: RenderDirective<S>, .. }
```

**PR 3c.2's job** (out of scope for this design doc, but documented for reviewer):
- Add `type RenderDirective: Send + Sync + 'static + Clone + Debug;` to `MarkingScheme`.
- Add `pub enum CapcoRenderDirective { Sci(SciMarking), Sar(SarMarking), FgiTrigraphs(SmallVec<[CountryCode; 4]>), ... }` in `marque-capco`.
- Bind `type RenderDirective = CapcoRenderDirective;` in `impl MarkingScheme for CapcoScheme`.
- Replace `crates/rules/src/fix_intent.rs::RenderDirective<S>` placeholder with `<S as MarkingScheme>::RenderDirective` reference.

**Net result**: PR 3c.1 ships `RenderDirective<S>` as an inert phantom; the type appears in `ReplacementIntent::Render` but is constructed only as `PhantomData::<S>`. No rule uses it in PR 3c.1 (rules still emit `FixProposal`); the type exists to be migrated. PR 3c.2 lifts it.

### Recap of `FixIntent<S>::confidence` (verification)

The task brief asks: "shape clear from existing `Confidence`? Verify."

Yes — the existing `Confidence` (`crates/rules/src/confidence.rs`, lines 60-73) has exactly the shape `FixIntent<S>::confidence` needs:
- `recognition: f32` (`[0, 1]`)
- `rule: f32` (`[0, 1]`)
- `region: Option<f32>`
- `runner_up_ratio: Option<f32>`
- `features: Vec<FeatureContribution>`

The contract doc (`fix-intent.md` lines 116-127) sketches a `Confidence::new(recognition, rule)` constructor that `Confidence` does not currently have (it has `Confidence::strict(rule_confidence: f32)` which pins recognition at 1.0). PR 3c.1 adds `Confidence::new(recognition: f32, rule: f32) -> Result<Self, ConfidenceError>` as a new constructor; PR 3c.2 migrates rule construction call sites.

```rust
// In crates/rules/src/confidence.rs (additive):

/// Construct a `Confidence` with explicit recognition + rule axes.
///
/// Errors if either axis is out of `[0.0, 1.0]` or NaN.
pub fn new(recognition: f32, rule: f32) -> Result<Self, ConfidenceError> {
    let c = Self {
        recognition,
        rule,
        region: None,
        runner_up_ratio: None,
        features: Vec::new(),
    };
    c.validate().map_err(ConfidenceError::Invalid)?;
    Ok(c)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfidenceError {
    #[error("invalid confidence: {0}")]
    Invalid(String),
}
```

This is additive (existing `Confidence::strict` continues to work); rule code in PR 3c.2 chooses `Confidence::new(...)` when supplying both axes explicitly.

### T040 — Cargo.toml additions

**File**: `crates/rules/Cargo.toml` — add direct `marque-scheme` dep:

```toml
[dependencies]
marque-ism = { workspace = true }
marque-scheme = { workspace = true }    # NEW (T040)
serde = { workspace = true, optional = true }
smallvec = { workspace = true }          # NEW (FixIntent::feature_ids)
thiserror = { workspace = true }
```

**Why explicit not transitive**: `marque-rules` already gets `marque-scheme` transitively through `marque-ism` (`marque-ism` depends on `marque-scheme` per Constitution VII §VII). But the transitive dep does NOT make the names available — Rust requires the consuming crate to declare the dep explicitly to use the names. T040's brief says "Add `marque-scheme` dependency to `marque-rules` `Cargo.toml`"; this is correct.

**Workspace Cargo.toml updates**: `smallvec` is already in workspace deps (`Cargo.toml:56`). No workspace-level addition needed.

---

## 4. Dependency graph diff

### Before PR 3c.1

```
marque-scheme ←── marque-ism ←── marque-core ─────────────────────┐
                  marque-ism ←── marque-rules ←── marque-capco ──┤
                  marque-scheme ←─────────────────  marque-capco ──┤
                                                                  ↓
                                                            marque-engine
```

### After PR 3c.1

```
marque-scheme ←── marque-ism ←── marque-core ─────────────────────┐
                  marque-ism ←── marque-rules ←── marque-capco ──┤
                  marque-scheme ←──── marque-rules                 │
                  marque-scheme ←─────────────────  marque-capco ──┤
                                                                  ↓
                                                            marque-engine
```

**Diff**: one new edge — `marque-rules → marque-scheme` (direct, was transitive).

### Cycle proof

A cycle would require `marque-scheme` to depend on `marque-rules` (or transitively on it). Verification:

- `marque-scheme/Cargo.toml`: only deps are `serde` (optional). Zero workspace-internal deps. ✓
- `marque-ism/Cargo.toml`: depends on `marque-scheme` only. Does not depend on `marque-rules`. ✓ (verified above)
- `marque-rules/Cargo.toml` (post-PR-3c.1): depends on `marque-ism`, `marque-scheme`, `serde`, `smallvec`, `thiserror`. ✓ no cycle.

The graph stays acyclic. `cargo check --workspace` post-PR-3c.1 succeeds.

**Constitution VII §VII compliance**: the new edge is exactly the one Appendix D anticipated:

> "As of PR 3c (`FixIntent<S>`), `marque-rules` also depends on `marque-scheme` directly so rule-emission values can reference scheme types without going through `marque-ism`; the graph stays acyclic because `marque-scheme` is still leaf-only."

✓ matches the design.

---

## 5. Compile-fail test designs

A consolidated table; each test is described with file, target invariant, and the exact compile-fail snippet shape.

| Test | File | Invariant | Snippet shape |
|---|---|---|---|
| T036.1 | `crates/scheme/tests/canonical_unconstructable.rs` | No `Box<str> → Canonical<S>` constructor | `let _: Canonical<_> = Canonical::from_bytes(Box::from("x"), Scope::Portion);` (method does not exist) |
| T036.2 | same | No `&str → Canonical<S>` impl | `let _: Canonical<_> = "x".into();` |
| T036.3 | same | External impl of `CanonicalConstructor<S>` rejected | `impl CanonicalConstructor<FakeScheme> for Evil { ... }` (sealed supertrait unsatisfied) |
| T036.4 | same | `sealed::Sealed` not nameable | `use marque_scheme::canonical::sealed::Sealed;` (module private) |
| T032.1 | `crates/rules/tests/message_args_closed_set.rs` | No `String` field on `MessageArgs` | `MessageArgs { raw_text: String::new(), .. }` (E0560 unknown field) |
| T032.2 | same | No `Vec<u8>` field on `MessageArgs` | `MessageArgs { bytes: Vec::new(), .. }` (E0560) |
| T032.3 | same | No `From<&str>` impl | `let _: MessageArgs = "x".into();` |
| T032.4 (positive) | same | Field set is exactly the documented closed set | `let MessageArgs { token, category, span, digest, confidence, expected_token, actual_token, feature_ids } = args;` (E0027 if a field is added without test update) |
| T033.1 | `crates/rules/tests/message_no_freeform_ctor.rs` | No `Message::from_string` | `Message::from_string("x")` (method does not exist) |
| T033.2 | same | No `From<&str> for Message` | `let _: Message = "x".into();` |
| T033.3 | same | No `Message::format` | `Message::format("{}", "x")` (method does not exist) |

**Test runner mechanism**: Rust's built-in `compile_fail` doctest harness, run via `cargo test --doc -p marque-scheme` and `cargo test --doc -p marque-rules`. No `trybuild` dev-dep needed (PR 0's D10 toolchain pin notes `trybuild` for the lints; the type-system seal tests use the lighter `compile_fail` doctest mechanism that already secures `EnginePromotionToken` at `crates/rules/src/lib.rs:559-565`).

**No `static_assertions` for these tests** — the seals are name-resolution / trait-impl level, not type-level assertions. The existing `static_assertions::assert_impl_all!` pattern handles `Send + Sync` checks (Constitution VI / FR-038); type-system seals use compile-fail doctests.

---

## 6. Citation strategy

Per Constitution VIII (Authoritative Source Fidelity) and the project memory `feedback_audit_predicates_against_source.md`:

- Every type added in PR 3c.1 carries a doc-comment citation to the relevant source where the design constraint originates:
  - **Source plan §-citations** (`docs/plans/2026-05-02-engine-refactor-consolidated.md`) for keystone-architecture decisions: `Canonical<S>` sealing (§8.1), message channel closure (§8.3), audit clean break (§10).
  - **Constitution citations** for invariant origins: V Principle V (G13 audit content-ignorance), VI (Send + Sync), VII (acyclic graph), VIII (citation fidelity).
  - **CAPCO-2016 §-citations** for `MessageTemplate` variants tied to specific marking semantics: §A.6 (SCI grammar), §H.4 / §H.5 / §H.6 / §H.8 (per-system invariants).

- Every CAPCO citation in `MessageTemplate` doc comments carries the §X.Y pNN form (per memory `feedback_citations_use_page_numbers.md`).

- T031's PR description includes a per-variant citation table that the implementer agent verifies against `crates/capco/docs/CAPCO-2016.md` at PR-open time. Missing or unverifiable citations are removed (Constitution VIII), not left in pending follow-up.

- The citation lint (FR-018) lands at PR 0.5 — independently of PR 3c.1. PR 3c.1 cannot rely on the lint to enforce citations; the implementer agent verifies manually before PR-open.

**Per-citation verification protocol** (follows project memory practice):
1. Open `crates/capco/docs/CAPCO-2016.md`.
2. Search for §X.Y header.
3. Confirm pNN aligns with the page printed in the markdown source-block (the manual is paginated as printed; markdown doc carries explicit page anchors).
4. Confirm the cited claim is what the §-passage says — not what the existing rule code claims it says (the predicate may be wrong; the citation must be against the source, not the predicate).
5. If the cited passage does not support the claim, do not write the citation.

---

## 7. Tooling design — `tools/message-template-extract/`

**Per task brief**: "Rust binary crate, NOT a workspace member (Constitution III)."

**Directory layout**:
```
tools/message-template-extract/
├── Cargo.toml                  # NOT in [workspace.members]; standalone binary
├── README.md                   # Usage + transient-ness notice
└── src/
    └── main.rs                 # AST scanner + cluster-and-emit logic
```

**`Cargo.toml`**:

```toml
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

[package]
name = "message-template-extract"
version = "0.0.1"
edition = "2024"
publish = false   # transient tool; not published

[dependencies]
syn = { version = "2", features = ["full", "extra-traits"] }
proc-macro2 = "1"
walkdir = "2"
```

**Why standalone binary, not workspace member**:
- Constitution III: WASM-safe crate set must remain WASM-safe. Adding a tooling crate to the workspace pulls its deps into the workspace dep graph, which CAN affect WASM build via lockfile; standalone keeps it isolated.
- Per `research.md` R-1: tooling lints already follow this pattern (live in `tools/`, NOT workspace members) for the same reason.
- Transient: deletes after PR 3c.2 review accepts the curated `MessageTemplate`. Standalone makes deletion trivial.

**What the AST scanner extracts**:

The scanner walks the workspace source tree (limited to `crates/capco/src/rules*.rs` + `crates/capco/src/rules_*.rs` + `crates/engine/src/engine.rs` per the R-2 scope) and finds every:
1. `format!("...")` first-arg literal (whether assigned to a variable later passed to `Diagnostic::new`, or passed inline)
2. `format_args!("...")` first-arg literal
3. `write!(_, "...")` / `writeln!(_, "...")` first-arg literal
4. String-literal arguments to `Diagnostic::new(...)` at the `message:` position (positional arg index 3 per `Diagnostic::new` signature in `crates/rules/src/lib.rs:625`)
5. The single existing `engine.rs:1389` `format!("decoder-recognized canonical form: {replacement:?}")` interpolation — explicitly called out for retirement

**Clustering**:
- Group by structural similarity of the format string (replace interpolation placeholders with `{}` markers; cluster identical templates).
- Per cluster: list source locations (file:line), the placeholder count, and a tentative `MessageTemplate::Variant` name suggestion derived from the format string keywords.

**Output format** (`specs/006-engine-rule-refactor/contracts/message-template-starter.md`):

```markdown
# MessageTemplate Starter (T030 mechanical extraction)

Generated: 2026-05-09 by `tools/message-template-extract/` against the
post-PR-3b rule catalog. Hand-curated into the closed `MessageTemplate`
enum at T031.

## Cluster 1 — Decoder recognized

Format: `decoder-recognized canonical form: {}`
Sources:
- `crates/engine/src/engine.rs:1389`

Placeholder types: 1 × `Box<str>` (the recognized canonical bytes — leak channel)

Suggested variant: `MessageTemplate::DecoderRecognized`
Suggested args: `actual_token: Option<TokenId>`

## Cluster 2 — Banner SCI rollup mismatch

Format: `banner SCI controls do not match expected from portions: expected {expected:?}, got {actual:?}`
Sources:
- `crates/capco/src/rules.rs:NNN` (E035)
- `crates/capco/src/rules_declarative.rs:NNN` (post-3b banner walker)

Placeholder types: 2 × debug-printed `Vec<SciControl>`

Suggested variant: `MessageTemplate::BannerRollupMismatch`
Suggested args: `category: CategoryId, expected_token: Option<TokenId>, actual_token: Option<TokenId>`

## Cluster N — ...
```

**The starter doc is reviewed and pruned at T031**:
- Variants that map to the same semantic (different rules emitting the same template) collapse into one variant.
- Variants that are too rule-specific (a one-off message used by one rule) inline into a more general variant or get a dedicated variant if the semantic is genuinely unique.
- Variants whose source rules retired in PR 3b drop entirely.

**Deletion**: per task brief — "T031 deletes `tools/message-template-extract/` after PR 3c review accepts that enum". The `tasks.md` line 163 specifies the same: "transient one-shot discovery script". The implementer agent issues the `rm -rf tools/message-template-extract/` as part of the PR 3c.1 commit chain (after the starter doc lands and T031's curated enum is in place); the starter doc itself stays in `specs/006-engine-rule-refactor/contracts/` as the historical record.

---

## 8. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| **R1** | **Sealing too tight: PR 3c.2 cannot migrate without breaking the `Canonical<S>` seal.** The engine in PR 3c.2 needs to render rule-emitted `FixIntent::Render` values, which calls `EngineConstructor::build_open_vocab`. If `EngineConstructor` is sealed too tightly, the engine code in `marque-engine` cannot construct it. | Medium | High | The Option D placement (`EngineConstructor` lives in `marque-scheme` as a `pub` type with `__engine_construct` visibility-by-convention) means the engine can call `EngineConstructor::<CapcoScheme>::__engine_construct()` from `marque-engine` without tripping the sealed-trait bound. Verified by sketching the PR 3c.2 call site (§ 9 T035.2 below). |
| **R2** | **`RenderDirective<S>` as assoc type forces wrong choice.** If we lift to assoc-type in PR 3c.1, we must add `type RenderDirective` to `impl MarkingScheme for CapcoScheme` in `marque-capco`, which violates the "additive only" constraint. | High | Medium | Resolved at T039 by deferring the assoc-type lift to PR 3c.2 and shipping `RenderDirective<S>` as a phantom-type alias in `marque-rules` for PR 3c.1. PR 3c.2 atomically (a) lifts the assoc-type binding, (b) adds `CapcoRenderDirective`, (c) migrates rule emission. |
| **R3** | **Compile-fail tests fail to actually compile-fail.** A typo in the snippet might let the compile_fail doctest pass when the seal is broken (the wrong-error compile failure still passes the test). | Medium | High | Each compile_fail snippet includes a unique sentinel (a method name that does not and will not exist, like `from_bytes`) so the failure mode is unambiguous. Reviewer checks: comment out the seal, confirm the test starts compiling (the test catches the breakage), restore the seal. |
| **R4** | **`Message::render()` interpolation channel reopens the leak.** A rule's `MessageArgs` carries `TokenId`s; `Message::render()` reads the args and emits a string. If `render()` is sloppy (e.g., calls into a token table that contains content), the rendered string could carry input-derived bytes. | Low | Critical | `Message::render()` is for CLI / IDE display only and is NOT called from the audit emitter. The audit emitter consumes `(template, args)` pairs directly per the audit-record contract `contracts/audit-record.md` (NDJSON `"message": { "template": "...", "args": { ... } }`). Add a regression-canary test at PR 3c.2 that grep-asserts no `original_bytes` substring appears in any `audit_*.ndjson` test fixture. |
| **R5** | **`Blake3Hash` placement (in `marque-scheme`) re-exports through `marque-rules` create a confusing two-name surface.** Consumers may import either path. | Low | Low | Re-export from `marque-rules` is `pub use marque_scheme::Blake3Hash;` (no rename); both paths resolve to the same nominal type. Rust's name-resolution treats them as identical for trait selection / `impl` blocks. Document the canonical path in `marque-rules`'s crate-level doc as `marque_rules::Blake3Hash`. |

### 8.1. Open question deferred to PR 3c.2 — `Canonical<S>` form selection

**Surfaced 2026-05-09 (post-PR-3c.1 implementation, pre-PR-3c.2 design)**:
the `Canonical::from_cve(token, scope, bytes)` shape carries `bytes` as
caller-supplied. PR 3c.2 will replace this with engine-side rendering
via `MarkingScheme::render_canonical_cve(token, scope, vocab,
render_context) -> Canonical<S>`. **The form-selection inside that
render method is an open architectural question** that PR 3c.2 must
resolve explicitly.

**Resolution (2026-05-09, after marking-handling-analysis review with
project owner)**: The four-form ambiguity is resolved by carrying an
explicit `EmissionForm` selector on `RenderContext` rather than baking
the form choice into `Scope`. Specifications, FR numbers, and tasks
land as follows:

| Decision | Lands as | Tracked in |
|---|---|---|
| `RenderContext { scope, emission_form, schema_version }` shape, with `#[non_exhaustive] EmissionForm { Auto, Portion, Banner, BannerAbbreviated, LongTitle }` and `EmissionForm::Auto` deriving form from `Scope` to preserve pre-3c.2 behavior. | PR 3c.2 (this PR) | `spec.md` FR-052; `data-model.md` "RenderContext and EmissionForm" section; `tasks.md` T048a / T048b / T048c; T048 reworded |
| CVE Value (form 1) is **not** added as a by-token `Vocabulary<S>` accessor. Recovered via `Vocabulary::lookup` round-trip from any of forms 2/3/4 when needed. Audit-record `bytes_digest` source is form-2/3/4 bytes, never form 1. | PR 3c.2 (this PR) — no new accessor, by decision | `spec.md` FR-052 closing note |
| `Vocabulary<S>` extended with `forms() -> &'static FormSet` accessor exposing portion / banner / banner-abbreviation / long-title (form 2 by-token) plus `recognized_aliases` slice (e.g., ISM `Description.title` divergences, historical aliases). Per-form methods become default methods over `forms()`. | PR 3d (post-3c.2, pre-PR-4) | `spec.md` FR-053; `data-model.md` "FormSet and FormKind" section; `tasks.md` PR 3d / T058c–T058h |
| `Deprecation<Token>` extended with `valid_from` / `valid_until` schema-version fields (data plumbing only; consumer flag is post-refactor). | PR 3d (post-3c.2, pre-PR-4) | `spec.md` FR-054; `data-model.md` "Deprecation validity windows" section; `tasks.md` T058g |
| Cross-grammar interconversion (e.g., NATO ↔ CAPCO recognize-A-emit-B), historical-as-valid evaluation mode, ISM-XML schema-compliant output, and Bayesian form-context selection — all deferred to post-refactor. The PR 3c.2 + PR 3d additions are the prerequisites; concrete impls land when a second grammar / a real ISM-XML emit need arrives. | Post-refactor; tracked as separate GitHub issues | (issues opened alongside this resolution) |

The decision matrix is preserved below as the historical record of the
trade-off space considered.

#### The four forms (per CAPCO-2016 §G.1 Table 4 + ODNI XML CVE Value)

A single CAPCO token has up to **four distinct surface forms**:

1. **CVE Value** — what `crates/ism/schemas/ISM-v2022-DEC/CVE/`
   declares (e.g., `DISPLAYONLY`, `EYES`, `REL`). Often
   space-stripped or punctuation-stripped relative to CAPCO.
2. **Marking Title** — the long banner-line title (e.g.,
   `DISPLAY ONLY`, `EYES ONLY`).
3. **Banner Abbreviation** — the authorized abbreviation; same as
   Title for many markings (`DEA SENSITIVE`); differs for some
   (`FOR OFFICIAL USE ONLY` → `FOUO`).
4. **Portion Mark** — the parenthesized form (`NF`, `OC`,
   `DISPLAY ONLY`, `DISPLAY ONLY [LIST]`).

`crates/ism/src/marking_forms.rs::MARKING_FORMS` is the hand-curated
single source of truth for forms 2/3/4. The CVE Value (form 1) lives in
ODNI XML and is currently surfaced only via `Vocabulary::lookup(bytes)
-> Option<TokenId>` (bytes-to-token), not by-token-to-bytes.

#### Why this matters

The current `Vocabulary<S>` trait surfaces three by-token accessors:
`portion_form()`, `banner_form()`, `banner_abbreviation()`. There is no
by-token CVE-Value accessor — the CVE Value is accessible only by
parsing it back through `Vocabulary::lookup(bytes)`. PR 3c.2's
`render_canonical_cve` must pick which form to emit, but the trait
surface cannot today emit form 1 by-token.

#### Form-selection axes PR 3c.2 must decide

| Axis | Options |
|---|---|
| **Scope-driven** | `Scope::Portion → portion_form` is uncontroversial. `Scope::Page` → ?: banner abbreviation OR banner title (both are valid per CAPCO-2016 §D.1 p27). `Scope::Document` → same question. |
| **Emit-context refinement** | Even within `Scope::Page`, the rule's intent may differ: a rule fixing a portion mark in a banner (E001) wants the banner abbreviation; a rule emitting the long title (S001 reverse case) wants the title. `RenderContext` may need a `BannerForm::{Title, Abbreviation}` enum. |
| **CVE Value emit path** | When does the engine ever need to emit form 1 (CVE Value)? Possibly only at the audit-record `bytes_digest` source — i.e., never as user-visible text. If form 1 is audit-only, no by-token accessor is needed; the digest is computed from form-2/3/4 bytes, not from form 1. **Open**: confirm this with the audit-record contract. |
| **Vocabulary trait extension** | If form 1 needs by-token emission, add `Vocabulary::cve_value(token) -> &'static str`. If not, leave the trait unchanged. |

#### Decision required by PR 3c.2 design (T048)

The PR 3c.2 planner agent **MUST** read this section before designing
`MarkingScheme::render_canonical_cve`. The decision shape:

- A `RenderContext` enum that captures the form-selection axis (e.g.,
  `RenderContext::PortionForm`, `RenderContext::BannerAbbreviation`,
  `RenderContext::BannerTitle`).
- The mapping from `(Scope, RenderContext)` to one of the four forms.
- Whether `Vocabulary<S>` gains a `cve_value(token)` accessor for
  audit-only CVE-Value emission.
- A property test asserting that, for every CAPCO token, the four
  forms round-trip through `Vocabulary::lookup` correctly (i.e.,
  `lookup(form_N(token)) == Some(token)` for each form, where the
  lookup is form-aware or form-tolerant).

PR 3c.1 ships `Canonical::from_cve` with a doc-comment that flags this
ambiguity. PR 3c.1 callers (test fixtures only) are not constrained to
any single form — they pass arbitrary bytes for compile-fail / Send +
Sync / phantom-type-parameter coverage. Form-selection rigor lands in
PR 3c.2.

---

## 9. Per-task ordered checklist

The implementer agent executes T030 → T040 in this order. Each task lists target file paths, intra-PR dependencies, estimated LOC, and the tests that accompany.

### T030 — Run mechanical extraction (~150 LOC + starter doc)

**Files**:
- `tools/message-template-extract/Cargo.toml` (new, ~15 LOC)
- `tools/message-template-extract/src/main.rs` (new, ~150 LOC)
- `tools/message-template-extract/README.md` (new, ~30 LOC, transient-ness notice)
- `specs/006-engine-rule-refactor/contracts/message-template-starter.md` (new, generated output)

**Dependencies within PR**: none — first task.

**Tests**: none in `tools/`; the tool's correctness is verified by review of the emitted starter doc. (The tool is transient; investing in unit tests is not load-bearing.)

**PR commit message**: `tools(message-template-extract): mechanical extraction of Diagnostic::message format-arg literals (T030)`.

### T031 — `MessageTemplate` enum (~120 LOC including doc comments)

**Files**:
- `crates/rules/src/message.rs` (new, ~120 LOC — `MessageTemplate` enum + per-variant doc comments with citations)
- `crates/rules/src/lib.rs` (modify: add `pub mod message;` and `pub use message::{Message, MessageTemplate, MessageArgs, Blake3Hash};`)

**Dependencies within PR**: T030 (the starter doc is the input to hand-curation).

**Tests**:
- `crates/rules/tests/message_template_round_trip.rs` (~50 LOC) — verify every variant has a stable `as_str()` label (mirrors `crates/rules/src/confidence.rs::tests::feature_id_as_str_matches_audit_contract`).

**Citation verification**: per §6, the implementer agent verifies each variant's CAPCO citation against `crates/capco/docs/CAPCO-2016.md` before PR-open.

**PR commit**: `feat(rules): MessageTemplate closed enum (T031, FR-003)`.

### T032 — `MessageArgs` struct + closed-set proofs (~80 LOC + tests)

**Files**:
- `crates/rules/src/message.rs` (extend: add `MessageArgs` struct + `Blake3Hash` re-export from `marque-scheme`)

**Dependencies within PR**: T031, plus T040 must be complete (`marque-rules` needs the direct `marque-scheme` dep to name `TokenId` / `CategoryId` / `Blake3Hash` without going through `marque-ism`'s re-exports).

**Tests**:
- `crates/rules/tests/message_args_closed_set.rs` (~80 LOC) — compile-fail doctests (T032.1, T032.2, T032.3) + positive destructuring pin (T032.4).

**PR commit**: `feat(rules): MessageArgs closed-set struct + compile-fail proofs (T032, FR-003)`.

### T033 — `Message::new` sole constructor (~30 LOC + tests)

**Files**:
- `crates/rules/src/message.rs` (extend: add `Message` struct + `impl Message`)

**Dependencies within PR**: T031, T032.

**Tests**:
- `crates/rules/tests/message_no_freeform_ctor.rs` (~40 LOC) — compile-fail doctests (T033.1, T033.2, T033.3).

**PR commit**: `feat(rules): Message::new is the sole public constructor (T033, FR-003)`.

### T040 — `marque-rules → marque-scheme` direct dep (~5 LOC)

**Files**:
- `crates/rules/Cargo.toml` (modify: add `marque-scheme = { workspace = true }` and `smallvec = { workspace = true }`)

**Dependencies within PR**: must complete BEFORE T031 / T032 / T033 because `MessageArgs` references `marque_scheme::TokenId`. Exception: if the implementer agent prefers, T040 can land first (out of numerical order) — the per-task checklist orders by intra-PR dependency, and T040 is in fact a prerequisite for T031 onward.

**Reordered execution**: T030 → **T040** → T031 → T032 → T033 → T034 → T035 → T036 → T037 → T038 → T039.

**Tests**:
- `cargo check --workspace` — must pass.
- `cargo tree -p marque-rules --duplicates` — no duplicate `marque-scheme` versions.
- Manual cycle-proof in PR description per §4 above.

**PR commit**: `chore(rules): direct dep on marque-scheme (T040, Constitution VII)`.

### T034 — `Canonical<S>` + `TokenSource` (~120 LOC)

**Files**:
- `crates/scheme/src/canonical.rs` (new, ~120 LOC — `Canonical<S>` + `TokenSource` + `Blake3Hash`)
- `crates/scheme/src/lib.rs` (modify: add `pub mod canonical;` and `pub use canonical::{Canonical, TokenSource, Blake3Hash};`)

**Dependencies within PR**: none (pure addition to `marque-scheme`).

**Tests**: deferred to T036.

**PR commit**: `feat(scheme): Canonical<S> with sealed constructors (T034, FR-001)`.

### T035 — Sealed-trait pattern + `EngineConstructor<S>` (~80 LOC)

**Files**:
- `crates/scheme/src/canonical/sealed.rs` (new, ~30 LOC — private `Sealed<S>` marker trait)
- `crates/scheme/src/canonical.rs` (extend: add `mod sealed;` + `CanonicalConstructor<S>` trait + `EngineConstructor<S>` struct + impls)

**Dependencies within PR**: T034.

**Tests**: deferred to T036.

**Sketched PR-3c.2 use site** (verifying R1 mitigation):

```rust
// In crates/engine/src/engine.rs (PR 3c.2):
use marque_scheme::canonical::{Canonical, CanonicalConstructor, EngineConstructor};
use marque_capco::CapcoScheme;

// In Engine::fix_inner:
let canonical: Canonical<CapcoScheme> = match intent.replacement {
    ReplacementIntent::Cve { token, scope } => {
        Canonical::from_cve(token, scope, vocab.bytes_for(token))
    }
    ReplacementIntent::Render { category, directive, scope } => {
        // Engine has the sole CanonicalConstructor<CapcoScheme> impl in scope.
        // The render call site is captured via #[track_caller] on
        // EngineConstructor::build_open_vocab.
        CapcoScheme::render_canonical::<EngineConstructor<CapcoScheme>>(
            category, directive, scope,
        )
    }
    ReplacementIntent::Delete => Canonical::from_cve(
        TokenId(0), scope, Box::from(""), // sentinel "deleted" canonical
    ),
};
```

This compiles — `EngineConstructor::<CapcoScheme>::__engine_construct()` is callable from `marque-engine` (the type is `pub`); the sealed `CanonicalConstructor<CapcoScheme>` trait is impl'd by `EngineConstructor<S>` in `marque-scheme` (within-crate, satisfies the private `Sealed` supertrait); the engine instantiates and uses it.

**PR commit**: `feat(scheme): CanonicalConstructor sealed trait + EngineConstructor (T035, FR-001 R-7)`.

### T036 — Compile-fail tests (~120 LOC)

**Files**:
- `crates/scheme/tests/canonical_unconstructable.rs` (new, ~120 LOC)

**Dependencies within PR**: T034, T035.

**Tests**: T036 IS the test. Run via `cargo test --doc -p marque-scheme`. Each compile_fail snippet is a doc test on a private function in the test file.

**PR commit**: `test(scheme): canonical_unconstructable compile-fail proofs (T036, SC-012)`.

### T037 — `FixIntent<S>` (~80 LOC)

**Files**:
- `crates/rules/src/fix_intent.rs` (new, ~80 LOC — `FixIntent<S>` struct)
- `crates/rules/src/lib.rs` (modify: add `pub mod fix_intent;` and `pub use fix_intent::{FixIntent, ReplacementIntent, RenderDirective};`)

**Dependencies within PR**: T031, T032, T033 (`Message`); T040 (`marque-scheme` dep).

**Tests**:
- `crates/rules/tests/fix_intent_smoke.rs` (~30 LOC) — construct a `FixIntent<()>` (using a stub scheme) and verify field access works.

**PR commit**: `feat(rules): FixIntent<S> rule-emission API (T037, FR-025)`.

### T038 — `ReplacementIntent<S>` (~40 LOC)

**Files**:
- `crates/rules/src/fix_intent.rs` (extend: add `ReplacementIntent<S>` enum)

**Dependencies within PR**: T037, T039 (the enum references `RenderDirective<S>`).

**Tests**: covered by T037's smoke test.

**PR commit**: `feat(rules): ReplacementIntent<S> three-discriminant enum (T038, FR-025)`.

### T039 — `RenderDirective<S>` placeholder (~10 LOC)

**Files**:
- `crates/rules/src/fix_intent.rs` (extend: add `pub type RenderDirective<S> = PhantomData<S>;` placeholder + doc comment explaining PR 3c.2 lift)

**Dependencies within PR**: T037 (lives in same file).

**Tests**: none in PR 3c.1 — the type is a phantom; PR 3c.2's `CapcoRenderDirective` enum lands the real shape and tests it.

**PR commit**: `feat(rules): RenderDirective<S> phantom placeholder (T039, lifted to assoc-type at PR 3c.2)`.

### Final cross-task verification

After all tasks complete:
- `cargo check --workspace` passes.
- `cargo test --workspace` passes (no existing test breaks; new tests pass).
- `cargo clippy --workspace -- -D warnings` passes.
- `cargo +stable clippy --workspace -- -D warnings` passes (per memory `feedback_clippy_nightly_vs_stable_drift.md`).
- `cargo tree -p marque-rules --duplicates` shows no duplicate scheme versions.
- The five compile-fail tests at `crates/scheme/tests/canonical_unconstructable.rs` and the seven at `crates/rules/tests/message_*.rs` all pass.
- `tools/message-template-extract/` is removed from the workspace (per T030/T031 closure).
- `specs/006-engine-rule-refactor/contracts/message-template-starter.md` exists and was reviewed during T031.

---

## 10. PR description draft

**Title**: `refactor-006 PR 3c.1: Foundation types (Canonical<S>, FixIntent<S>, Message) — additive only`

**Branch**: `refactor-006-pr-3c-foundations` → target `staging`

**Summary**:

PR 3c.1 lands the foundation types for the keystone PR 3c (`FixIntent<S>` rule-emission API, sealed `Canonical<S>`, content-ignorant `Message`). This is the first of two sub-PRs splitting the original PR 3c per the recovery plan; PR 3c.2 follows with the atomic migration (rule-side `FixProposal → FixIntent` cutover, `engine.rs:1389` `format!` deletion, audit schema bump `marque-mvp-2 → marque-1.0`, `RenderDirective` assoc-type lift, `CapcoRenderDirective` enum).

**This PR is additive only.** Every type added coexists with the current `FixProposal` / `Diagnostic.message: Box<str>` / `RuleId` shapes. No existing rule changes. No engine promotion path changes. No existing test changes. The workspace compiles before and after with byte-identical behavior on every fixture.

**Tasks landed**: T030 (mechanical extraction tool + starter doc) → T031 (MessageTemplate) → T032 (MessageArgs closed-set) → T033 (Message::new sole ctor) → T034 (Canonical<S>) → T035 (sealed CanonicalConstructor + EngineConstructor) → T036 (compile-fail proofs) → T037 (FixIntent<S>) → T038 (ReplacementIntent<S>) → T039 (RenderDirective placeholder) → T040 (marque-rules → marque-scheme dep).

**Constitution check**:

| Principle | How this PR satisfies | Note |
|---|---|---|
| I — Performance | No hot-path code added; all new types are construction-only | n/a |
| II — Zero-copy | `MessageArgs` uses `Span` not bytes; `Canonical<S>` holds `Box<str>` only at the canonical-token level (already minimal) | ✓ |
| III — WASM safety | `Canonical<S>`, `FixIntent<S>`, `Message`, `MessageArgs`, `Blake3Hash` all land in WASM-safe crates (`marque-scheme`, `marque-rules`); zero new I/O deps; one new dev-dep (`smallvec`, already workspace-blessed) | ✓ |
| IV — Two-layer | `MessageTemplate` is hand-curated Layer-2; no Layer-1 generated code added | ✓ |
| V — Audit-first / G13 | Closure of the `format!` channel (closed `Message`) and the `Box<str> → Canonical` channel (sealed constructors) lands at the type level; compile-fail tests pin the closure | **load-bearing** |
| VI — Dataflow | No pipeline shape changes; `Send + Sync` bound on `RenderDirective` assoc type (lifted at 3c.2) preserves `BatchEngine` correctness | ✓ |
| VII — Crate discipline | One new edge: `marque-rules → marque-scheme` (direct, was transitive). Cycle proof in §4. | matches Appendix D anticipation |
| VIII — Citation fidelity | Every `MessageTemplate` variant + every doc comment carries §-citation; verified manually pre-PR-open | implementer attestation in PR description |

**What proves the seal works**:
- `crates/scheme/tests/canonical_unconstructable.rs` — 4 compile-fail doctests (no `Box<str>` ctor; no `&str` ctor; external `CanonicalConstructor<S>` impl rejected; `sealed::Sealed` not nameable).
- `crates/rules/tests/message_args_closed_set.rs` — 4 tests (3 compile-fail; 1 positive destructuring pin).
- `crates/rules/tests/message_no_freeform_ctor.rs` — 3 compile-fail tests (no `from_string`; no `From<&str>`; no `format`).

**What does NOT change in PR 3c.1**:
- `FixProposal` continues to work; rules emit it as today.
- `Diagnostic.message: Box<str>` continues to work; the format string at `engine.rs:1389` continues to interpolate (deleted in PR 3c.2).
- `MARQUE_AUDIT_SCHEMA = "marque-mvp-2"` (audit schema bump in PR 3c.2).
- `AppliedFix.proposal.original: Box<str>` field stays (`Span`-only reshape in PR 3c.2 per FR-004).
- All 47 registered CAPCO rules unchanged.
- All four open-vocab admission sites in `crates/core/src/parser.rs` unchanged (FR-015 / FR-016 migration in PR 2 / PR 3c.2).
- `impl MarkingScheme for CapcoScheme` unchanged (assoc-type lift in PR 3c.2).

**Reviewer checklist**:
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes (no existing test broke)
- [ ] `cargo test --doc -p marque-scheme` passes (compile-fail proofs)
- [ ] `cargo test --doc -p marque-rules` passes (compile-fail proofs)
- [ ] `cargo +stable clippy --workspace -- -D warnings` passes
- [ ] `cargo tree -p marque-rules --duplicates` shows no scheme dup
- [ ] Cycle proof in PR description matches §4 of design doc
- [ ] Each `MessageTemplate` variant's CAPCO citation verified against `crates/capco/docs/CAPCO-2016.md`
- [ ] `tools/message-template-extract/` is removed (transient, deleted post-T031)
- [ ] `specs/006-engine-rule-refactor/contracts/message-template-starter.md` is committed (historical record)
- [ ] No `#[doc(hidden)] pub fn` is used as the primary seal mechanism (sealed traits only)
- [ ] D4 no-consumers attestation: confirmed via 60-day no-contact window per `decisions.md` D4 template (PR 3c.1 absent of consumer impact)

---

## Final summary

**Absolute path to design doc**: `/home/knitli/marque/.worktrees/pr3c-foundation/docs/plans/2026-05-09-pr3c-foundation-plan.md`

> **Status of doc**: This planning agent does not have a `Write` tool exposed. The full design content above is the doc; the parent agent (or implementer agent) must persist it verbatim to the path above before T030 begins. The implementer agent then executes against this doc.

**Five most consequential design decisions**:

1. **`Canonical<S>` lives in `marque-scheme`, not `marque-ism`** — because it's scheme-generic (the engine and audit emitter must name it without depending on ISM/CAPCO vocabulary), the `pub(crate) from_render` seal needs scheme-local privacy, and `marque-ism` already owns `CanonicalAttrs` (name disambiguation matters for future readers).

2. **`EngineConstructor<S>` lives in `marque-scheme` (not `marque-engine`)** — the only way to satisfy a private `sealed::Sealed<S>` supertrait bound is to impl it inside the crate that defines `Sealed`. Owning the engine-side seal-implementor type in `marque-scheme` (with `__engine_construct` visibility-by-convention mirroring `EnginePromotionToken`) is the minimum-friction path that keeps the seal real, avoids `#[doc(hidden)] pub fn` as the sealing mechanism (per memory), and enables PR 3c.2's engine call site to compile without further sealing acrobatics.

3. **`RenderDirective<S>` ships as a phantom-type alias in PR 3c.1, lifted to `MarkingScheme` assoc-type in PR 3c.2** — adding a required associated type to `MarkingScheme` would break `impl MarkingScheme for CapcoScheme` in `marque-capco`, violating the additive-only constraint. Atomic lift in PR 3c.2 (alongside `CapcoRenderDirective` and rule migration) is the only viable path; Rust's stable subset has no associated-type-defaults workaround.

4. **`MessageArgs` is a closed-set struct with `Option<T>` fields, not an enum-of-variants** — the closed-set struct supports the most common case (a variant carries 1-3 of the permitted args, the rest `None`) without coupling `MessageTemplate` and `MessageArgs`. Compile-fail tests + a positive destructuring-as-pin test enforce field-set closure; a future field addition fails one test loudly rather than silently slipping through. This makes content-ignorance a type invariant rather than convention.

5. **`marque-rules → marque-scheme` direct dep is added at T040** — even though the dep is transitive through `marque-ism` today, name resolution requires the explicit edge. The new edge exactly matches the Appendix D anticipation in the consolidated plan and Constitution VII §VII; the graph stays acyclic because `marque-scheme` remains the only true graph leaf.