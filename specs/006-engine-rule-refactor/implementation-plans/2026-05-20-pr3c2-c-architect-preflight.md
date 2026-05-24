# PR 3c.2.C Architect Preflight

> File path: `/home/knitli/marque/docs/plans/2026-05-20-pr3c2-c-architect-preflight.md`
> Author: architect preflight, 2026-05-20
> Status: Preflight contract for PR 3c.2.C implementation agent
> Predecessor PM contract: `/home/knitli/marque/docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` §1 (D25.3) + §4 (R-2)
> Deferred-findings register: `/home/knitli/marque/specs/006-engine-rule-refactor/followups/2026-05-19-pr-3c2-a-deferred-findings.md`
>
> **Note**: This preflight pass was instantiated with only `Read`, `Grep`, `Glob` available — no `Write`/`Edit`/`Bash` tool. Content originally returned inline by the architect agent; PM persisted to this path post-return. Future preflight briefs must include explicit `Write` tool grant.

## 0. Scope verification (against master plan)

Verified at preflight authorship (`grep` runs 2026-05-20):

| Master plan estimate | Verified count | Delta |
|---|---|---|
| `Diagnostic.message: Box<str>` → `Message` field-type change | 1 field change | Confirmed |
| `Diagnostic.citation: &'static str` → `Citation` field-type change | 1 field change | Confirmed |
| "engine.rs:1389 decoder `format!`" | engine.rs:**4027** (line drift) | Master plan stale; real site found via `MessageTemplate::DecoderRecognized` references |
| "~5–6 `format!`-built capco sites" | **7 sites** in `crates/capco/src/` | Confirmed close enough |
| ~41 citation sites (architect preflight C earlier estimate) | **66 occurrences** across 7 files (production-code `citation:` field literals, excluding test-assertion `.citation` accessors) | **Master scope underestimate** |

The 7 `format!`-built `Diagnostic.message` sites (verified):

| File | Line | Rule | Funnel |
|---|---|---|---|
| `crates/capco/src/rules.rs` | 871 | E006 deprecated dissem | `make_fix_diagnostic` |
| `crates/capco/src/rules.rs` | 987 | E007 X-shorthand (table path) | `make_fix_diagnostic` |
| `crates/capco/src/rules.rs` | 1014 | E007 X-shorthand (pattern path) | `make_fix_diagnostic` |
| `crates/capco/src/rules.rs` | 1369 | C001 corrections-map | `make_fix_diagnostic` |
| `crates/capco/src/rules.rs` | 4767 | E035 SCI banner roll-up | `make_fix_diagnostic` |
| `crates/capco/src/scheme/constraints/helpers.rs` | 74 | E012 dual-classification | `ConstraintViolation.message` (`String`) — **flows through bridge** |
| `crates/capco/src/scheme/constraints/helpers.rs` | 114 | E014 joint-rel-to-coverage | `ConstraintViolation.message` (`String`) — **flows through bridge** |

The 1 engine site:

| File | Line | Purpose |
|---|---|---|
| `crates/engine/src/engine.rs` | 4026–4029 | Decoder synthetic R001 — `format!("decoder-recognized canonical form at bytes {}..{}", span.start, span.end)` |

**Architectural finding (was not in master plan)**: `ConstraintViolation.message: String` and `ConstraintViolation.citation: &'static str` (defined at `crates/scheme/src/constraint.rs:298-313`) and `PageRewrite.citation: &'static str` (at `crates/scheme/src/page_rewrite.rs:71`) flow into `Diagnostic.message` / `Diagnostic.citation` via the engine's `bridge_constraint_diagnostic` helper (`crates/engine/src/engine.rs:2207-2286`). The bridge calls `CapcoScheme::message_by_name(...) -> Option<String>` (adapter.rs:358) which currently returns `String`. **For type consistency post-C, `ConstraintViolation.message: String → Message` and `message_by_name → Option<Message>` must move in lockstep** — otherwise the bridge will have to `format!` a `String` → `Message` adapter at every call, which violates the closed-template invariant. This is a scope expansion the master plan did not anticipate.

**OQ-C7 (NEW, raised at preflight)**: Does `ConstraintViolation.message` migrate from `String` to `Message` atomically with `Diagnostic.message` in C, or stay `String` with the bridge constructing a synthetic `Message` from it? **Recommendation**: migrate `ConstraintViolation.message: String → Message` and `PageRewrite.citation → Citation` atomically in C. The alternative (synthetic `Message::new(MessageTemplate::ConstraintViolation, args { ... })`) introduces a `Message` variant whose `args` would need a `String`-typed payload, breaking the closed-args invariant. Better to migrate cleanly. This expands C's reach to `crates/scheme/src/{constraint.rs, page_rewrite.rs}` and ~3 catalog row definitions in `crates/capco/src/scheme/`.

`ExpectedDiagnostic` migration (OQ-C3): **No migration needed**. Verified at `crates/test-utils/src/lib.rs:75-81`: `ExpectedDiagnostic` carries only `rule: String`, `span: ExpectedSpan`, `severity: Option<String>`. There is NO `message` field and NO `citation` field on `ExpectedDiagnostic`. Golden-file fixtures (`.expected.json` sidecars) match on rule ID + span + severity only, never on message text. The closed-template change is invisible to the corpus harness.

## 1. Tactical plan — commit-by-commit

Each commit is build-green at boundary. The B precedent used 6 implementation commits + reviewer-pass closeout (B1–B6); C follows the same shape. The opening housekeeping commit (C0) is required to unblock `cargo clippy --workspace --all-targets -- -D warnings`.

### C0 — Housekeeping (clippy unblock)

Fix `clippy::question_mark` at `crates/core/src/parser.rs:2199`. One-line mechanical change per the rust-reviewer prescription in the deferred-findings register:

```rust
// Replace:
let (prefix, full_form) = if let Some(p) = trimmed.strip_suffix(" EYES ONLY") {
    (p, true)
} else if let Some(p) = trimmed.strip_suffix(" EYES") {
    (p, false)
} else {
    return None;
};

// With:
let (prefix, full_form) = if let Some(p) = trimmed.strip_suffix(" EYES ONLY") {
    (p, true)
} else {
    let p = trimmed.strip_suffix(" EYES")?;
    (p, false)
};
```

**Build-green gate**: `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

### C1 — `citation!()` macro + Citation re-exports

Adds a declarative `citation!()` macro in `marque-rules` (re-exported to `marque-capco` via `pub use`). Shape:

```rust
// crates/rules/src/lib.rs (new module `citation_macro`)
#[macro_export]
macro_rules! citation {
    // §<L> pNN form
    (§ $letter:ident p $page:literal) => {
        $crate::Citation::new(
            $crate::AuthoritativeSource::Capco2016,
            $crate::SectionRef::new($crate::SectionLetter::$letter),
            ::core::num::NonZeroU16::new($page).expect("citation page must be non-zero"),
        )
    };
    // §<L>.<sub> pNN form
    (§ $letter:ident . $sub:literal p $page:literal) => {
        $crate::Citation::new(
            $crate::AuthoritativeSource::Capco2016,
            $crate::SectionRef::new($crate::SectionLetter::$letter)
                .with_subsection(::core::num::NonZeroU8::new($sub).expect("subsection must be non-zero")),
            ::core::num::NonZeroU16::new($page).expect("citation page must be non-zero"),
        )
    };
    // §<L>.<sub> Table <N> pNN form
    (§ $letter:ident . $sub:literal Table $table:literal p $page:literal) => {
        $crate::Citation::new(
            $crate::AuthoritativeSource::Capco2016,
            $crate::SectionRef::new($crate::SectionLetter::$letter)
                .with_subsection(::core::num::NonZeroU8::new($sub).expect("subsection must be non-zero"))
                .with_table(::core::num::NonZeroU8::new($table).expect("table must be non-zero")),
            ::core::num::NonZeroU16::new($page).expect("citation page must be non-zero"),
        )
    };
}
```

The `.expect("…")` calls run at **macro-expansion time** — they evaluate at compile time because the inputs are literals; `NonZeroU16::new(0)` is `None` and `.expect()` panics at const evaluation, which is a compile error. This preserves D25.2's "no runtime validation" stance while giving compile-time rejection of `0` page numbers.

**Tests landed in C1**:
- Doctest in `crates/rules/src/citation.rs` showing 3 usages: `citation!(§H.4 p61)`, `citation!(§B.3 Table 2 p21)`, `citation!(§H pNNN)`.
- Compile-fail doctest: `citation!(§H.4 p0)` (page-zero rejection).
- Existing `citation_display_roundtrip.rs` extended with one round-trip through citation-lint's real parser per C-FOLLOWUP-1 (cargo-add `marque-citation-lint` as a `dev-dependency`, or invoke its scanner via `include!` of `tools/citation-lint/src/citation.rs` if the tool is not a crate).

**Build-green gate**: `cargo test -p marque-rules --doc` + `cargo build --workspace` pass.

**Reversibility note**: C1 is a pure addition to the public surface. If C2-C5 are reverted, C1 stands alone as a no-op improvement.

### C2 — `Diagnostic.citation` field-type change (`&'static str → Citation`)

Atomic field-type change:
- `crates/rules/src/lib.rs`: `pub citation: &'static str` → `pub citation: Citation` on `Diagnostic<S>`.
- All 5 `Diagnostic::*` constructors update their `citation: &'static str` parameter → `citation: Citation`.
- Manual `Clone for Diagnostic<S>`: `Citation: Copy` so `self.citation` flows by value.
- `bridge_constraint_diagnostic` at `engine.rs:2281`: `v.citation` was `&'static str`; under OQ-C7 resolution, migrates to `Citation` at the `ConstraintViolation` field-type change site (deferred to C3 if `ConstraintViolation` stays `String/&'static str` in C scope; see OQ-C7).
- WASM JSON emit at `crates/wasm/src/lib.rs:381`: `citation: d.citation` was `&'static str` → now needs `format!("{}", d.citation)` for the NDJSON `String` field (or change `DiagnosticJson.citation` to a `Display`-renderable type — recommend the former to keep WASM JSON wire format byte-stable).

Per-row migration of the 66 citation literal sites in `crates/capco/src/`:
- Each `citation: "CAPCO-2016 §H.4 p61"` → `citation: citation!(§H.4 p61)`.
- Each `citation: "CAPCO-2016 §B.3 Table 2 p21"` → `citation: citation!(§B.3 Table 2 p21)`.
- The `"CAPCO-2016 "` prefix is dropped at the source; `Citation::Display` renders `§H.4 p61` per its existing impl (the prefix is implied by `AuthoritativeSource::Capco2016`).

**Citation re-verification (Constitution VIII)**: At every migrated site, the §-citation MUST be re-verified against `/home/knitli/marque/crates/capco/docs/CAPCO-2016.md` at point of propagation. The propagation rule applies. PM contract requires this; preflight cannot pre-verify all 66 sites.

**Stale comments (C-FOLLOWUP-3)**: 5 forward-pointer comments at:
- `crates/scheme/src/scheme.rs:604, 699, 734`
- `crates/capco/src/scheme/marking_scheme_impl.rs:587, 674`

Update each to "a future PR will land the §G.1 Table 4 dispatch body" — per OQ-C5 recommendation. EmissionForm dispatch is not in C/D/E scope per the master plan; the "future PR" language is honest.

**Build-green gate**: `cargo build --workspace` + `cargo test --workspace` pass. WASM lint output preserves byte-identity with the pre-migration form (verified by existing parity test, which compares JSON output).

### C3 — `ConstraintViolation.message: String → Message` + `message_by_name → Option<Message>` (OQ-C7 resolution)

If OQ-C7 resolves to "migrate together" (recommended):
- `crates/scheme/src/constraint.rs:300`: `pub message: String` → `pub message: Message`.
- `crates/scheme/src/constraint.rs:301`: `pub citation: &'static str` → `pub citation: Citation`.
- `crates/scheme/src/page_rewrite.rs:71`: `pub citation: &'static str` → `pub citation: Citation`.
- `crates/capco/src/scheme/adapter.rs:358-393` (`message_by_name`): `-> Option<String>` → `-> Option<Message>`. All 6 returned `.to_owned()` strings convert to `Message::new(template, args)` — the 6 rows need explicit `MessageTemplate` assignments:
  - `E015/non-us-requires-dissem` → `MessageTemplate::ConflictsWith` (or new `MessageTemplate::RequiredByPresence` arm — verify against existing variant docs).
  - `E016/joint-conflicts-restricted` → `MessageTemplate::ConflictsWith`.
  - `E036/joint-conflicts-hcs` → `MessageTemplate::ConflictsWith`.
  - `capco/noforn-conflicts-rel-to` → `MessageTemplate::ConflictsWith`.
  - `E037/nodis-conflicts-exdis` → `MessageTemplate::ConflictsWith`.
  - `E054/relido-conflicts-noforn` → `MessageTemplate::ConflictsWith`.

All 6 are `ConflictsWith` (catalog dyadic conflict rows). The `MessageArgs` payload for each is `{ token: Some(<dominated>), expected_token: Some(<dominating>) }` keyed on the catalog row's known token pair.

- `crates/capco/src/scheme/constraints/helpers.rs:74` and `:114`: `message: format!(...)` on `ConstraintViolation` → `message: Message::new(template, args)`. For `E012` (helpers.rs:74), template is `ConflictsWith` (US class vs foreign class — mutually exclusive); for `E014` (helpers.rs:114), template needs `RequiredByPresence` semantic (JOINT participants required in REL TO). The byte-context information ("missing countries [GBR, DEU]") that was in the format-string CANNOT be preserved in `MessageArgs` under the closed-args invariant. The argument list reduces to `{ category: Some(CategoryId::RelTo), token: Some(TOK_JOINT) }`.

- `bridge_constraint_diagnostic` at `engine.rs:2274`: `v.message.clone()` was a `String`; under the migration becomes `v.message.clone()` (a `Message` — `Message: Clone`).

**Loss-of-information note (PM-blocking)**: The migration explicitly drops the runtime country list ("[GBR, DEU]") from `E014`'s message. This is the Constitution V Principle V closure — runtime byte content is not audit-permissible. Diagnostic message text must be reconstructed from the closed-template + args. A human reading a diagnostic post-C will see `"JOINT participants required in REL TO list"` without the specific missing countries. The audit emit boundary CAN render `args.category` and `args.token` to give a structured-output equivalent; the human-readable rendering at the CLI/WASM output layer is a deliberate narrowing. **PM must confirm this is acceptable.**

**Build-green gate**: `cargo test --workspace` passes; `marque-test-utils ExpectedDiagnostic` matching is unaffected (no message-text matching).

### C4 — `Diagnostic.message` field-type change (`Box<str> → Message`)

Atomic field-type change:
- `crates/rules/src/lib.rs:1120`: `pub message: Box<str>` → `pub message: Message`.
- All 5 `Diagnostic::*` constructors: `message: impl Into<Box<str>>` → `message: Message`.
- Manual `Clone for Diagnostic<S>`: `self.message.clone()` works (`Message: Clone`).
- `FixDiagnosticParams.message: String` → `FixDiagnosticParams.message: Message` (at `crates/capco/src/rules.rs:4112`).
- `make_fix_diagnostic`: signature updates; the funnel for E006/E007/C001/E035 propagates through.

The 7 `format!`-built capco sites resolve as:

| Site | New construction |
|---|---|
| rules.rs:871 (E006) | `Message::new(MessageTemplate::SupersededToken, MessageArgs { token: Some(<dep_token>), expected_token: Some(<canonical_token>), ..default() })` |
| rules.rs:987 (E007 table) | `Message::new(MessageTemplate::SupersededToken, MessageArgs { token: Some(<x-shorthand-token>), expected_token: Some(<canonical-token>), ..default() })` |
| rules.rs:1014 (E007 pattern) | Same as :987 |
| rules.rs:1369 (C001) | `Message::new(MessageTemplate::CorrectionsApplied, MessageArgs { token: Some(<token-text>), expected_token: Some(<replacement>), ..default() })` |
| rules.rs:4767 (E035) | `Message::new(MessageTemplate::BannerRollupMismatch, MessageArgs { category: Some(CategoryId::Sci), ..default() })` |
| helpers.rs:74 (E012) | (Handled in C3 if OQ-C7 migrate-together; else here) `Message::new(MessageTemplate::ConflictsWith, MessageArgs { ..default() })` |
| helpers.rs:114 (E014) | (Same as helpers.rs:74) `Message::new(MessageTemplate::RequiredByPresence, MessageArgs { token: Some(TOK_JOINT), category: Some(CategoryId::RelTo), ..default() })` |

The 1 engine site:

| Site | New construction |
|---|---|
| engine.rs:4026–4029 (decoder R001) | `Message::new(MessageTemplate::DecoderRecognized, MessageArgs { span: Some(span), ..default() })` |

**OQ-C1 unresolved gap — corrections-map runtime bytes**: C001 corrections-map (`rules.rs:1369`) interpolates `text` (the user's typo) and `replacement` (the corrected token). Both flow from `corrections.get(text)` where `text` is `token_span.text` — a runtime byte slice. The `MessageArgs.token: Option<TokenId>` field requires a `TokenId` from the active vocabulary; runtime typo bytes (e.g., `"SERCET"`) are not registered TokenIds. **Two options**:

- (a) Use `MessageArgs.token: Some(TokenId::lookup(text).unwrap_or(TokenId::UNKNOWN))` — requires a `TokenId::UNKNOWN` sentinel. Loss of the actual typo text in audit output.
- (b) Drop `text` from the message args entirely; rely on `span` to locate the typo in source. The audit emit renders something like `"corrections-map applied at bytes 142..148, replacement: SECRET"`. Original typo bytes recoverable from `(source, span)` by the audit consumer.

**Recommendation**: option (b). Audit consumers re-derive the typo bytes from `(source, span)` if needed; the audit record stays content-ignorant. This is the same posture as the decoder R001 site.

**WASM JSON emit (`crates/wasm/src/lib.rs:380`)**: `message: d.message.as_ref()` was `&str`. Post-C4, `d.message: Message`. The JSON emit currently writes a free-form string; under the Constitution V closure, the JSON should write `{ "template": "ConflictsWith", "args": { "token": "TOK_RELIDO", "expected_token": "TOK_NOFORN" } }`. **This is a wire-format change for WASM lint output.** Master plan §D25.3 says "Audit wire format doesn't change in C (Diagnostic flows to CLI/WASM output, not directly to `AppliedFix`)" — that statement is **incorrect at the WASM boundary**. The CLI output goes through `marque-engine` → `AppliedFix` (which is audit-versioned to `marque-mvp-3`), but the `lint` output is a `Diagnostic` JSON dump that has no schema version. PM must decide whether the WASM `Diagnostic` JSON shape is wire-stable; per `feedback_pre_users_no_deprecation_phasing.md`, marque is pre-users so the break is acceptable.

**Two engine test sites** at `engine.rs:6745` and `engine.rs:6760` (`diag.message.as_ref()`) need rewriting:
- `:6745`: `msg.contains("C001")` and `msg.contains("E006")` — these test the R002 contributing-rules render. Post-C, the R002 diagnostic carries `MessageArgs.contributing_rule_ids: SmallVec<[RuleId; 4]>` instead of an inline format string. Assertion updates to `diag.message.args().contributing_rule_ids.iter().any(|r| r.as_str() == "C001")`.
- `:6760`: `msg.contains("post-pass-1 buffer failed to re-parse")` — this is human-readable text from `build_r002_diagnostic`. Post-C, the template is `MessageTemplate::ReparseFailed`; assertion updates to `assert_eq!(diag.message.template(), MessageTemplate::ReparseFailed)` and `assert!(diag.message.args().contributing_rule_ids.is_empty())`.

`build_r002_diagnostic` itself moves from `format!`-style inline rendering to:

```rust
Diagnostic::new(
    R002_RULE_ID,
    Severity::Error,
    failure_span,
    Message::new(
        MessageTemplate::ReparseFailed,
        MessageArgs { contributing_rule_ids, ..Default::default() },
    ),
    citation!(§A pNNN),  // R002 is engine-synthetic — citation TBD (engine.rs:R002_CITATION currently)
    None,
)
```

The R002_CITATION constant migrates from `&'static str` to a `Citation` constant via `citation!()`.

**Build-green gate**: `cargo test --workspace` passes; `cargo test --workspace --doc` passes; WASM JSON output regression test (if exists) updates to the new closed-template shape.

### C5 — Reviewer-pass closeout

Mirrors the B6 commit structure. Single reviewer-pass commit addressing any code-review / architect-review / rust-review findings. Documentation updates:
- The `crates/rules/src/message.rs` doc-comments referencing `crates/engine/src/engine.rs:1462` (line drift from PR 3c.1's authorship) update to the current line.
- The `Diagnostic.message` doc-comment retires the "PR 3c.1 ships the new types **alongside** the existing `Box<str>` channel" framing.
- `Blake3Hash::zero()` placeholder DOES NOT migrate in C — it lands in 3c.2.D per the master plan. C does not touch the audit record.

## 2. OQ resolutions

### OQ-C1: MessageTemplate variant coverage

**Resolution**: Existing 15 variants cover 6 of 7 capco sites + 1 engine site cleanly. The C001 corrections-map site (rules.rs:1369) needs option (b) above (drop runtime bytes from args; consumers re-derive from `span`). No new MessageTemplate variant required for C — the closed-set discipline holds. If a future site genuinely requires a template not in the closed set, the audit-schema bump is the cost of adding it, which is the intended deterrent against template proliferation.

**Trade-off**: option (b) reduces human-readable detail in CLI diagnostics for C001 corrections (the user sees "corrections-map fix applied at byte range X..Y" instead of "`SERCET` → `SECRET`"). This is the constitutional cost of content-ignorance. Mitigation: the audit renderer at the CLI surface can re-render the bytes from `(source, span, args.expected_token)` if it knows the source; the engine itself cannot.

### OQ-C2: `citation!()` macro

**Resolution**: Ship `citation!()` macro as **opening commit C1 of 3c.2.C** (not a separate landed-first PR). Rationale: the macro and the migration are mutually dependent — without the macro, 66 sites convert verbosely; without the migration, the macro is unused. Splitting into a separate PR adds review overhead without separating risk (the macro itself is ~30 lines of macro_rules with compile-time validation via NonZeroU8/U16's panic-on-zero).

**5-year-maintenance argument**: A future grammar (CUI / NATO) requiring a different citation shape extends the macro additively. The current `§<L>[.<sub>] [Table <N>] p<page>` shape is CAPCO-specific; adding `cui!(...)` or `nato!(...)` macros as siblings is the planned-migration path.

### OQ-C3: `ExpectedDiagnostic` migration

**Resolution**: No migration. `ExpectedDiagnostic` does not mirror `Diagnostic.message` or `Diagnostic.citation`. Verified at `crates/test-utils/src/lib.rs:75-81`. The corpus harness matches on `(rule, span, severity)` only.

### OQ-C4: cfg-gate lift on `s004` / `rules_us1.rs`

**Resolution**: **Do NOT lift in C.** The gate comment says "PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite". `FixProposal` was retired entirely in PR 3c.B Commit 10 — the field is gone from `Diagnostic` and replaced by `fix: Option<FixIntent<S>>` + `text_correction: Option<TextCorrection>`. These tests' bodies assume `FixProposal` as a named type that no longer exists. Lifting the gate would require a full test rewrite that is out-of-scope for the Diagnostic-shape change. Re-enablement belongs in 3c.2.D (when `AppliedFix` shape changes anyway) or a dedicated post-D test-resurrection PR.

**Trade-off**: We continue to lose the s004 audit-content-ignorance test coverage and the rules_us1 corpus coverage. T055 (deterministic NDJSON canary scan, scheduled for 3c.2.D) is the replacement gate for s004; rules_us1's coverage overlaps with the lint corpus runner at `crates/capco/tests/lattice_corpus_runner.rs` (T118 closeout). Net coverage loss is bounded.

### OQ-C5: stale forward-pointer comment targets

**Resolution**: Update each of the 5 sites (scheme.rs:604, 699, 734; marking_scheme_impl.rs:587, 674) to **"a future PR will land the §G.1 Table 4 dispatch body"** — vague but truthful. Per master plan, EmissionForm dispatch is not scoped to C, D, or E; it lands "post-1.0" per deferred-findings register C-FOLLOWUP-3 wording. A GitHub issue is OK but not required — the comment shape names the deliverable, not a PR identifier, so it doesn't go stale on the next refactor.

### OQ-C6: commit sequence

**Resolution**: 6-commit sequence (C0 housekeeping + C1 macro + C2 citation field + C3 ConstraintViolation/PageRewrite migration + C4 message field + C5 reviewer-pass closeout). Each commit is build-green at boundary. C3 is the largest single commit (touches `crates/scheme/` engine-crate boundary — see Constitution check below); C2 and C4 are the field-type-change atomic commits; C5 mirrors B6.

### OQ-C7 (NEW): `ConstraintViolation.message: String → Message`

**Resolution (recommendation)**: Migrate atomically with `Diagnostic.message` in C3. The alternative (keep `String`, synthesize `Message` at bridge time) requires a `Message` variant whose `args` carry a `String`-typed field, breaking the Constitution V Principle V closed-args invariant. Better to migrate the source-of-truth type.

**PM-blocking question**: Constitution VII says scheme-adoption PRs MUST NOT edit engine crates (`marque-scheme` is an engine crate per the canonical dependency graph). C3 touches `crates/scheme/src/{constraint.rs, page_rewrite.rs}`. **Is C3's `marque-scheme` touch authorized?** This is a within-006 precedent question; the master plan §3 "Constitution check" row for VII says "C touches `crates/{capco,rules,engine}/`" — `marque-scheme` is NOT listed. The master plan is **silent on `marque-scheme`** for C scope.

**Resolution path**: Either (a) PM authorizes `marque-scheme` touch as within-006 precedent (parallel to PR 4b-D.2's relaxation of `MarkingScheme::Marking: JoinSemilattice`), OR (b) the `ConstraintViolation.message` migration defers to a separate sub-PR (3c.2.C2?) with engine-crate authorization, OR (c) the bridge synthesizes `Message` from `String` and accepts the closed-args violation as a temporary contract debt closed at 3c.2.D. **Recommend (a)**: the precedent shape exists; the closed-template invariant is load-bearing for the C deliverable.

## 3. Risk register

### R-1 — `MessageTemplate` variant insufficiency

**Likelihood**: LOW. The 15 existing variants were authored at PR 3c.1 with the 7 capco sites + engine R001 site as the design targets. Variant coverage is by construction.

**Impact if it bites**: Need to add a `MessageTemplate` variant mid-C. Each variant addition requires audit-schema coordination per the closed-set invariant — but since C stays on `marque-mvp-3`, an addition is allowed. The closed-set invariant only blocks variant churn at `marque-1.0` (3c.2.D) and later.

**Mitigation**: At commit C4, before authoring the 7 mapped constructions, run `cargo build -p marque-capco` against a draft mapping. If a template is missing, raise as a PR scope expansion before continuing.

### R-2 — `cargo clippy --workspace --all-targets -- -D warnings` failure cascade

**Likelihood**: MEDIUM. C0 unblocks the known `clippy::question_mark` at parser.rs:2199, but C1-C4 introduce ~100 new code sites that may trip clippy lints (e.g., `clippy::redundant_clone` if `Message` is `Clone`-heavy; `clippy::needless_pass_by_value` on `impl Into<Message>`-style helpers).

**Impact**: Each lint that fires per-site multiplies across 66 citation sites or 7 message sites. Could block CI for a full commit cycle.

**Mitigation**: Run `cargo clippy --workspace --all-targets -- -D warnings` after each commit; address per-commit, not at end. The `+stable` qualifier per `feedback_clippy_nightly_vs_stable_drift.md` memory: use `cargo +stable clippy ...` to mirror CI behavior.

### R-3 — WASM JSON output wire-format change

**Likelihood**: HIGH (it WILL change). The `crates/wasm/src/lib.rs:380` JSON emit currently writes `message: <free-form string>`. Post-C4, it writes `{ "template": "...", "args": { ... } }`. Any WASM consumer parsing the current `message` field as a string breaks.

**Impact**: Hypothetical only — marque is pre-users per the project memory `feedback_pre_users_no_deprecation_phasing.md`.

**Mitigation**: Document the WASM JSON shape change in the PR description. Add a `MARQUE_LINT_JSON_SCHEMA` constant (companion to `MARQUE_AUDIT_SCHEMA`) bumped to a new label so downstream consumers can detect the change explicitly. Wire-format-stable transition path is to render `Display` of `Message` (which currently has no Display impl per the closed-template doctests) — but adding `Display` to `Message` would create a covert free-form channel. **Recommendation**: ship the structured JSON shape; do NOT add `Display`.

### R-4 — Constitution VII engine-crate boundary at C3

**Likelihood**: CERTAIN if OQ-C7 resolves to "migrate together".

**Impact**: C3 touches `crates/scheme/src/constraint.rs` and `crates/scheme/src/page_rewrite.rs`. `marque-scheme` is an engine crate per the canonical graph. Constitution VII §IV: "A scheme-adoption PR MUST NOT edit the engine crates."

**Mitigation**: This is a refactor PR (006 engine + rule architecture refactor), not a scheme-adoption PR. Within-006 precedent allows engine-crate touch for refactor-internal changes (PR 4b-D.2 set this precedent). PM authorization explicit on the C PR description; reviewer attestation references the precedent. **PM must confirm at preflight time.**

### R-5 — Citation re-verification scale (66 sites)

**Likelihood**: HIGH (citation drift is the empirically dominant failure mode for marque historically — see `feedback_audit_predicates_against_source.md`).

**Impact**: Of 66 citation sites, statistically ~3-5 may not match the manual exactly (typos, copy-paste from sibling rules, post-edit drift). A single wrong citation is a Constitution VIII correctness defect of the same severity as a wrong predicate.

**Mitigation**: At C2, the implementation agent MUST run the `crates/capco/docs/CAPCO-2016_citation_index.yml` lookup against every migrated citation. The propagation rule applies. Architect-pass review (C5 closeout) re-verifies a sampled subset (10-15 sites) against the manual. Any citation that cannot be traced to a real passage is removed, not left in place.

### R-6 — Loss-of-detail in E014 / corrections-map messages

**Likelihood**: CERTAIN (intentional under Constitution V Principle V).

**Impact**: Human-readable diagnostics lose runtime byte detail. CLI users may complain.

**Mitigation**: The CLI/WASM renderer at the audit-emit boundary CAN render `(template, args, span)` into a richer string for human consumption — `args.span` plus the source buffer lets the renderer extract the original bytes for display. The constitutional invariant is that the bytes don't enter the `Diagnostic` or `AppliedFix` record; renderers downstream CAN extract them. Document the renderer's responsibility in the C PR description.

### R-7 — `bridge_constraint_diagnostic` ergonomic regression

**Likelihood**: MEDIUM. Post-C3, the bridge `unwrap_or_else(|| v.message.clone())` returns a `Message`; the synthesized fallback path (when `message_by_name` returns `None`) uses `v.message` (typed). For Custom-arm rows (E012/E014), the helper-emitted `ConstraintViolation.message` is the only message source; the bridge has no override hook for Custom rows. The single-`MessageTemplate::ConflictsWith` constraint per row means most rows produce identical args, which is fine but unergonomic.

**Mitigation**: Acceptable. The audit emitter sees `template = ConflictsWith` plus the row's `constraint_label` (e.g., `"E054/relido-conflicts-noforn"`) which provides the per-row distinction at the audit boundary.

## 4. Reviewer attestation checklist

A reviewer approving 3c.2.C MUST verify:

- [ ] **C0 lands first**: `cargo clippy --workspace --all-targets -- -D warnings` exits 0 at the C0 commit boundary.
- [ ] **Citation discipline**: Every `citation!(...)` invocation in C2 re-verified against `/home/knitli/marque/crates/capco/docs/CAPCO-2016.md` at point of authorship. Sampled subset (≥10 sites) hand-checked.
- [ ] **Closed-template invariant preserved**: No new `format!`-derived `Diagnostic.message`. No `impl Display for Message`. No `Message::from_string` / `Message::from_str`. Compile-fail doctests at `crates/rules/src/message.rs` still passing.
- [ ] **Closed-args invariant preserved**: No new `String` / `Vec<u8>` / `&str` fields on `MessageArgs`. Positive destructure-pin test at `crates/rules/tests/message_args_closed_set.rs` still passing.
- [ ] **Constitution V Principle V (G13) preserved**: No runtime byte text flows from input into `Diagnostic.message`. C001 corrections-map "before-text" dropped per OQ-C1 option (b). E014 country list dropped per R-6.
- [ ] **Constitution VII boundary**: PM authorization explicit for C3's `marque-scheme` touch (OQ-C7 resolution).
- [ ] **Constitution VIII propagation**: Stale forward-pointer comments updated (C-FOLLOWUP-3); existing citations preserved verbatim during type migration.
- [ ] **66-site citation migration complete**: `grep -rn 'citation:\s*"' crates/capco/src/` returns 0 hits post-C2.
- [ ] **7-site message migration complete**: `grep -rn 'message:\s*format!' crates/capco/src/` returns 0 hits post-C4.
- [ ] **1-site engine migration complete**: `grep -rn 'format!.*decoder-recognized' crates/engine/src/` returns 0 hits post-C4.
- [ ] **WASM JSON wire-format change documented**: PR description explicitly notes the structured-shape change; no silent break.
- [ ] **No `__engine_promote` calls outside Constitution V Principle V carve-out**: `grep -rn '__engine_promote' crates/` returns only cfg-gated test sites; each carries the inline carve-out comment.
- [ ] **Bench-check non-blocking** (D25.6): `lint_10kb` reported but not blocking.
- [ ] **cfg-gates on s004 / rules_us1 unchanged** (OQ-C4): `cargo test --workspace` does NOT exercise them.
- [ ] **`citation!()` macro doctest coverage**: 3 positive doctests + 1 compile-fail doctest in C1.
- [ ] **`citation-lint` round-trip** (C-FOLLOWUP-1): integration test in C1 routes through real citation-lint parser.

## 5. 5-year-maintenance assessment

| Choice | Reversible? | Lock-in concern |
|---|---|---|
| `citation!()` macro shape | YES — the macro is a thin wrapper; can be inlined to verbose `Citation::new(...)` if a future need arises | None; sugar only |
| `Diagnostic.message: Box<str> → Message` | NO at the type level — reverting is a major refactor | This is the constitutional G13 closure; reverting violates Constitution V Principle V |
| `Diagnostic.citation: &'static str → Citation` | Partially — `Citation: Display` renders to the prior `&'static str` shape | Citation-lint integration locks in the structured form; rolling back loses the CI gate |
| `ConstraintViolation.message: String → Message` (OQ-C7) | Partially | Same as `Diagnostic.message`; constitutional |
| Drop of runtime country list from E014 message (R-6) | Recoverable at renderer layer | Renderer responsibility shifts to CLI/WASM crates; documented |
| Drop of `text` from C001 corrections-map args (R-3, OQ-C1 (b)) | Recoverable from `(source, span)` | Same as above |
| `MessageTemplate::DecoderRecognized` variant slot | NO — locked into audit-schema | Audit-schema discipline locks variant set at `marque-1.0`; bump cost is real |

The two load-bearing 5-year locks are (a) the closed-template invariant on `Diagnostic.message` and (b) the typed `Citation` surface. Both are constitutional commitments (V and VIII respectively); reverting either is not a refactor-class change but a constitutional amendment.

**Future-grammar adoption (CUI / NATO)**: The `citation!()` macro is CAPCO-specific by virtue of accepting `§<L>[.<sub>] [Table <N>] p<page>` shape. A future `cui!()` macro lands as a sibling at scheme #2 adoption time. The `Citation` type itself is grammar-neutral via `AuthoritativeSource::Capco2016` plus `#[non_exhaustive]` on the enum — adding `AuthoritativeSource::Cui` is additive. `SectionLetter` is CAPCO-coupled (A-H); CUI/NATO will likely need a generic `SectionToken` per GH-FOLLOWUP-1. The 5-year cost of `SectionLetter` is tracked at GH-FOLLOWUP-1.

## 6. Constitution check

| Principle | C compliance | Rationale |
|---|---|---|
| **I (Uncompromising Performance)** | PASS | `Message` is `Clone` but the type is small (one `MessageTemplate` u8-ish enum + `MessageArgs` with mostly `Option<u32>` payloads + 2 `SmallVec<[T; 4]>`). Diagnostic clone cost ~unchanged; `Citation: Copy` improves over `&'static str` clone (was already trivial). No hot-path change. SC-001 16ms ceiling preserved. |
| **II (Zero-Copy)** | PASS | No new heap allocation on the hot path. `Message` carries `Option<TokenId>` / `Option<CategoryId>` / `Option<Span>` (all `Copy`), `SmallVec` inline-4. `Diagnostic.message` size changes from `Box<str>` (8 bytes on stack) to `Message` (~64 bytes — `MessageTemplate` enum + struct of options). Stack-bound. |
| **III (WASM-Safe)** | PASS | `citation!()` macro expands to const-fn calls; no runtime validation code ships. `Message` carries no allocator-bound types except the two `SmallVec<[_; 4]>` which spill heap only on contributing_rule_ids > 4 (rare per the existing constitutional 4-rule partition). |
| **IV (Two-Layer Rule Architecture)** | PASS | No Layer 1 / Layer 2 boundary change. The migration is at the Layer 2 emission surface (rules → diagnostics) and the engine bridge; generated CVE-predicate code untouched. |
| **V (Audit-First Compliance / G13)** | PASS — load-bearing | This IS the G13 closure for `Diagnostic.message`. The `format!` of input bytes channel is closed at the type level. The C001 / E014 byte-content drops are intentional under Constitution V Principle V. `AppliedFix.__engine_promote` constraints unchanged. |
| **VI (Dataflow Pipeline Model)** | PASS | No phase change. Scanner → Parser → Rules → Roll-up untouched. The diagnostic shape change is orthogonal to the pipeline structure. |
| **VII (Crate Discipline)** | CONDITIONAL — PM resolves OQ-C7 | If C3 migrates `ConstraintViolation`, `marque-scheme` is touched. Within-006 precedent (PR 4b-D.2 / 4b-D.3) allows this; PM authorization explicit in PR description. |
| **VIII (Authoritative Source Fidelity)** | PASS — load-bearing | The `Citation` typed surface IS the Constitution VIII closure for citation propagation. Every migrated citation re-verifies against `crates/capco/docs/CAPCO-2016.md` per the propagation rule. Reviewer attestation requires sampled hand-verification. |

## Appendix A — Files touched by C (estimated)

- `crates/rules/src/lib.rs` — `Diagnostic.message` + `Diagnostic.citation` field types; 5 `Diagnostic::*` constructor signatures
- `crates/rules/src/citation.rs` — re-export updates (if `citation!()` is here vs. lib.rs)
- `crates/rules/src/macros.rs` (new) — `citation!()` macro
- `crates/capco/src/rules.rs` — 22 `Diagnostic::new` / `with_fix_at_span` sites + 5 `format!`-built sites + `make_fix_diagnostic` helper + `FixDiagnosticParams`
- `crates/capco/src/rules_declarative.rs` — 5+ `row.message` / `row.citation` references; `DeprecatedSciRow` field types
- `crates/capco/src/scheme/constraints/helpers.rs` — 2 `format!`-built sites + multiple `ConstraintViolation` constructions
- `crates/capco/src/scheme/adapter.rs` — `message_by_name` return type + 6 dyadic row returns
- `crates/capco/src/scheme/predicates/sci_per_system.rs` — line 136 `String::from(d.message)` (incompatible with `Message` → adjust)
- `crates/capco/src/scheme/` (subdirs) — catalog rows with `citation:` fields (sci_per_system.rs, class_floor.rs)
- `crates/scheme/src/constraint.rs` — `ConstraintViolation.message` + `.citation` (OQ-C7)
- `crates/scheme/src/page_rewrite.rs` — `PageRewrite.citation` (OQ-C7)
- `crates/scheme/src/scheme.rs` — 3 stale forward-pointer comments
- `crates/scheme/src/marking_scheme_impl.rs` — 2 stale forward-pointer comments
- `crates/engine/src/engine.rs` — R001 synthesis site (~line 4026) + R002 site + `bridge_constraint_diagnostic` + 2 test assertions at ~6745 + ~6760
- `crates/wasm/src/lib.rs` — JSON emit at line 380-381 (wire-format change)
- `crates/core/src/parser.rs` — line 2199 (C0 housekeeping)
- `crates/rules/tests/message_*.rs` — closed-set / no-freeform-ctor compile-fail doctest pins (preserved)
- `crates/rules/tests/citation_display_roundtrip.rs` — extended with citation-lint integration round-trip (C-FOLLOWUP-1)
- `crates/capco/Cargo.toml` — possibly `marque-citation-lint` as `dev-dependency` for C-FOLLOWUP-1
- `docs/plans/2026-05-20-pr3c2-c-*.md` — PM contract + tactical plan + reviewer attestation docs

## Appendix B — Outstanding PM decisions before implementation

1. **OQ-C1 option (a) vs (b)** for corrections-map runtime bytes — recommend (b).
2. **OQ-C7 migrate-together vs defer** for `ConstraintViolation` / `PageRewrite` — recommend migrate-together with Constitution VII within-006 precedent authorization.
3. **WASM JSON wire-format change acceptance** (R-3) — confirm pre-users posture from `feedback_pre_users_no_deprecation_phasing.md` applies.
4. **`citation!()` macro inclusion** — recommend ship in C1 (C-FOLLOWUP-2 resolution).
5. **cfg-gate handling on s004 / rules_us1** — recommend leave gated (OQ-C4).
6. **R-6 message detail loss acceptance** — confirm renderer-layer responsibility shift is acceptable.

Once items 1-6 are decided, the implementation agent has an unambiguous contract for C0-C5.

---

**Preflight authored**: 2026-05-20 by architect preflight agent
**Implementation contract**: BLOCKED on PM resolution of items 1-6 in Appendix B
**Estimated implementation effort**: ~3-4 implementation cycles (one per C2, C3, C4, plus C0+C1 housekeeping cycle and C5 reviewer-pass closeout)
