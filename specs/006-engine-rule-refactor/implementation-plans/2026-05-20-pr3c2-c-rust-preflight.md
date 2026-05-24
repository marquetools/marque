<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3c.2.C — Diagnostic Reshape: Rust-Specialist Preflight

**Date**: 2026-05-20  
**Branch target**: `staging`  
**Sub-PR scope**: `Diagnostic.message: Box<str> → Message`; `Diagnostic.citation: &'static str → Citation`; engine decoder `format!` closed; ~7 capco `format!` sites migrated.  
**Preflight agent**: Rust-specialist  
**Predecessor**: PR 3c.2.B (call-site migration) — MERGED 2026-05-20

---

## Scope Realism Check (Before Any Other Section)

The PM scope statement says "~81 citation occurrences total." Grep confirms **80 `citation:` field assignments** across `crates/capco/src/`. However, only **~57 are in `Diagnostic` constructors**. The remaining ~23 are in:

- `ConstraintViolation.citation: &'static str` (defined in `marque-scheme` — the dependency-graph leaf)
- `ClassFloorRow.citation: &'static str`, `SciPerSystemRow.citation: &'static str` (defined in `marque-capco`)
- `PageRewrite.citation: &'static str` (defined in `marque-scheme`)
- `AuditNote.citation: &'static str` (defined in `marque-rules`)
- `FixDiagnosticParams.citation: &'static str` (private helper in `crates/capco/src/rules.rs`)
- `NonCanonicalRow.citation`, `CompanionAction.citation`, `JointHcsHelper.citation` (internal capco)

**CRITICAL scoping decision required**: `ConstraintViolation.citation` and `PageRewrite.citation` live in `marque-scheme`, which MUST NOT depend on `marque-rules` (where `Citation` lives) per Constitution VII. These fields CANNOT migrate to `Citation` in PR 3c.2.C without either (a) moving `Citation` to `marque-scheme` (wrong — it's a rule-surface type) or (b) introducing a `marque-scheme → marque-rules` dependency (graph violation). **Recommendation: these `&'static str` fields stay as-is in C. Only `Diagnostic.citation` migrates.**

Similarly, `FixDiagnosticParams.citation`, `ClassFloorRow.citation`, `SciPerSystemRow.citation`, and all internal capco catalog row `citation` fields remain `&'static str` in C — the migration path for those is to convert them when they are copied into a `Diagnostic` at emission time.

**Effective C scope**: `Diagnostic.message: Box<str> → Message` (1 field change, ~57 call sites); `Diagnostic.citation: &'static str → Citation` (1 field change, ~57 call sites); 3 engine-constant citation strings (`DECODER_CITATION`, `R002_CITATION`, `CORRECTIONS_MAP_CITATION`) and the 5 engine `output.rs` test-only `Diagnostic::new` call sites.

---

## 1. Type-System Migration Mechanics

### 1.1 `Diagnostic.message: Box<str>` → `Message`

**Literal field-type edit** in `crates/rules/src/lib.rs:1120`:

```rust
// Before
pub message: Box<str>,

// After
pub message: Message,
```

**Constructor signature change** (lines 1198–1207, 1214–1232, 1249–1268, 1285–1311, 1320–1328). All constructors currently accept `impl Into<Box<str>>` for `message`. These must change to accept `Message` directly — NOT `impl Into<Message>`, because `Message` has no `From<Box<str>>` or `From<String>` impl by design (the compile-fail proofs enforce this). The existing `impl Into<Box<str>>` accept-any-string ergonomic wrapper is the thing being removed.

Sample constructor signature change:

```rust
// Before
pub fn new(
    rule: RuleId,
    severity: Severity,
    span: Span,
    message: impl Into<Box<str>>,
    citation: &'static str,
    fix: Option<FixIntent<S>>,
) -> Self

// After
pub fn new(
    rule: RuleId,
    severity: Severity,
    span: Span,
    message: Message,
    citation: Citation,
    fix: Option<FixIntent<S>>,
) -> Self
```

**Note**: `Diagnostic::with_fix`, `Diagnostic::with_fix_at_span`, `Diagnostic::text_correction`, and `Diagnostic::info` all share the same `message` parameter — all must change simultaneously.

**Clone impl** at `lib.rs:1175–1188` uses `self.message.clone()` — `Message: Clone` (derive present in `message.rs:511`), so the clone impl compiles without change.

**Sample call-site migrations**:

*From `Diagnostic::new(...)` — engine output.rs test site (lines 219–228):*

```rust
// Before
Diagnostic::new(
    RuleId::new("E001"),
    Severity::Error,
    Span::new(0, 0),
    "test",
    "test",
    None,
)

// After
Diagnostic::new(
    RuleId::new("E001"),
    Severity::Error,
    Span::new(0, 0),
    Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default()),
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::A).with_subsection(NonZeroU8::new(6).unwrap()),
        NonZeroU16::new(15).unwrap(),
    ),
    None,
)
```

*From `make_fix_diagnostic` helper — rules.rs via `FixDiagnosticParams.message: String`:*

`FixDiagnosticParams.message: String` stays `String` for now (this struct is internal to capco and is passed to `Diagnostic::text_correction`). The `make_fix_diagnostic` function passes `p.message` into the constructor. Once the constructor accepts `Message`, this internal struct becomes the last free-form string path. Resolution: change `FixDiagnosticParams.message: String` → `message: Message` and update all 7 `format!`-built sites.

*From `format!`-built `E035` banner SCI rollup site (rules.rs:4767):*

```rust
// Before
message: format!(
    "banner SCI block is missing markings present in the page's \
     portions (systems, compartments, and/or sub-compartments): {}",
    missing.join("; ")
),

// After
message: Message::new(
    MessageTemplate::BannerRollupMismatch,
    MessageArgs::default(),  // 'missing' list is not in permitted types; see §2.5 below
),
```

**Structural friction — audit emit must use `template()` + `args()` accessors**:

`Message` has no `Display` impl by design. Any code that currently calls `format!("{}", diagnostic.message)` will fail to compile after the migration. Audit emit at `crates/engine/src/` (NDJSON serialization) must access `diagnostic.message.template().as_str()` + `diagnostic.message.args()`. Same for the WASM emit surface. The CLI rendering layer must render messages via a `render_message(template, args) -> String` function (the renderer can produce human-readable output; it just cannot live on `Message` itself). Search for any `format!` / `Display` uses of `diagnostic.message` in the render/output path before C lands.

**Search required**: `grep -rn "\.message" crates/engine/src/ crates/wasm/src/ -- | grep -v "push\|field\|set"` to find render callsites before migration.

### 1.2 `Diagnostic.citation: &'static str` → `Citation`

**Literal field-type edit** in `crates/rules/src/lib.rs:1123`:

```rust
// Before
pub citation: &'static str,

// After
pub citation: Citation,
```

All constructors taking `citation: &'static str` change to `citation: Citation`. The field is `Copy` (`Citation: Copy` per `citation.rs:261` `assert_impl_all!`), so the clone impl's `self.citation` field copy remains correct without change.

**Sample migration — `Diagnostic::new` call at `output.rs:219` (test site)**:

Covered by §1.1 above — both `message` and `citation` change in the same call.

**Sample migration — `Constraint::Custom` catalog citation verbatim pass-through**:

`ConstraintViolation.citation: &'static str` stays `&'static str`. The bridge in `crates/capco/src/rules_declarative.rs` that converts `ConstraintViolation` into a `Diagnostic` must parse the string into a `Citation` at conversion time OR carry a parallel `Option<Citation>` through `ConstraintViolation`. 

**Recommended approach for the bridge**: At the `ConstraintViolation → Diagnostic` conversion site (the `DeclarativeConstraintRule` walker and `sci_per_system_emit`), parse the `&'static str` citation using a `citation_from_static` conversion function that returns `Option<Citation>`, falling back to a sentinel `Citation` for non-parseable strings (e.g., `"engine-synthetic"`, `"CONFIG:[corrections]"`). This keeps the ConstraintViolation struct unchanged (no crate-graph violation) while ensuring `Diagnostic` always carries a typed `Citation`.

**`CORRECTIONS_MAP_CITATION` migration**:

```rust
// Before (crates/rules/src/lib.rs:676)
pub const CORRECTIONS_MAP_CITATION: &str = "CONFIG:[corrections]";

// After — this is a non-CAPCO citation; use a sentinel Citation
// A bare-section `§A p0` shape would be invalid; instead introduce
// a CORRECTIONS_MAP citation with a zero-subsection §A shape or
// add an EngineInternal variant to AuthoritativeSource.
```

**Decision required**: Does `CORRECTIONS_MAP_CITATION` become a `Citation` or does `Citation` get a new sentinel constructor for non-CAPCO sources? The cleanest answer is `AuthoritativeSource::Config` (since it already has `#[non_exhaustive]` and the comment says "when a second variant lands"). **Recommendation**: add `AuthoritativeSource::Config` variant in C, define `CORRECTIONS_MAP_CITATION: Citation = Citation::new(AuthoritativeSource::Config, SectionRef::new(SectionLetter::A), NonZeroU16::new(1).unwrap())` as a sentinel.

Similarly, `DECODER_CITATION: &str = "CAPCO-2016 §A.6 p15"` in `engine.rs:118` can become a proper `Citation` constant:

```rust
const DECODER_CITATION: Citation = Citation::new(
    AuthoritativeSource::Capco2016,
    SectionRef::new(SectionLetter::A).with_subsection(NonZeroU8::new(6).unwrap()),
    NonZeroU16::new(15).unwrap(),
);
```

`R002_CITATION: &str = "engine-synthetic"` must become either an `AuthoritativeSource::EngineInternal` sentinel or a `pub const R002_CITATION: Citation` using `AuthoritativeSource::Config`. These are non-CAPCO engine citations and must NOT use `AuthoritativeSource::Capco2016`.

**`test-utils` `ExpectedDiagnostic`**:

`ExpectedDiagnostic.citation` does not exist — the struct (lib.rs lines 76–81) has only `rule`, `span`, and optional `severity`. It does NOT carry a `citation` field. No migration needed here.

**`AuditNote.citation: &'static str`** at `crates/rules/src/audit_note.rs:150`:

`AuditNote` is in `marque-rules` — `Citation` is also in `marque-rules` — so this COULD migrate. However, `AuditNote` is not in the PM-stated scope for C. Flag as a C-adjacent migration that is either in or out of scope; recommend OUT of C scope to keep C's blast radius bounded. File as GH-FOLLOWUP-C-1.

### 1.3 Engine R001 decoder `format!` migration (engine.rs:4026)

The current site:

```rust
Some(Diagnostic::with_fix_at_span(
    rule,
    severity,
    span,
    span,
    format!(
        "decoder-recognized canonical form at bytes {}..{}",
        span.start, span.end
    ),
    DECODER_CITATION,
    intent,
))
```

After migration:

```rust
Some(Diagnostic::with_fix_at_span(
    rule,
    severity,
    span,
    span,
    Message::new(MessageTemplate::DecoderRecognized, MessageArgs {
        span: Some(span),
        ..MessageArgs::default()
    }),
    DECODER_CITATION,  // becomes Citation constant per §1.2
    intent,
))
```

`MessageTemplate::DecoderRecognized` exists (message.rs:144). `MessageArgs.span: Option<Span>` is a permitted field (message.rs:396). The `span.start` / `span.end` numerals that were in the format string are not needed as separate fields — `Span` already carries both. This closes the content-ignorance leak channel per T050: byte offsets alone are on the G13 permitted-identifier list.

**R002 `format!` site (engine.rs:4109–4116)**:

```rust
// Before
let message = if rule_list.is_empty() {
    "post-pass-1 buffer failed to re-parse; pass-2 skipped".to_string()
} else {
    format!(
        "post-pass-1 buffer failed to re-parse after applying \
         pass-1 fixes from {rule_list}; pass-2 skipped"
    )
};

// After
let message = Message::new(
    MessageTemplate::ReparseFailed,
    MessageArgs {
        contributing_rule_ids: contributing_rule_ids.into_iter().collect(),
        ..MessageArgs::default()
    },
);
```

`MessageTemplate::ReparseFailed` exists (message.rs:154). `MessageArgs.contributing_rule_ids: SmallVec<[RuleId; 4]>` exists (message.rs:435). The `{rule_list}` interpolation — which was formatting `RuleId.as_str()` values — moves into the `contributing_rule_ids` field where it belongs.

---

## 2. MessageTemplate Coverage Gap Analysis

The 7 `format!`-built `message:` sites in `crates/capco/src/` are all in `FixDiagnosticParams` structs passed to `make_fix_diagnostic`. They currently carry the `message` as a free-form `String` (struct field `FixDiagnosticParams.message: String`).

### 2.1 `rules.rs:871` — E006 deprecated dissem control

```rust
format!(
    "{:?} is a deprecated dissemination control; replace with {:?}",
    token.text, entry.replacement
)
```

- Both `token.text` and `entry.replacement` are token canonical strings — **content-ignorance concern**: `token.text` is original document bytes, NOT a `TokenId`. This is the leak channel the migration must close.
- **Mapping**: `MessageTemplate::SupersededToken` — args: `token` = `TokenId` for the deprecated token (lookup from the token string), `expected_token` = `TokenId` for the replacement.
- **Gap**: `token.text` is a `&str` from the parsed source span — it is the raw document text, not a `TokenId`. The rule must look up the `TokenId` from the deprecated-control registry (the CVE enum) rather than echoing the raw text. This is a CORRECTNESS improvement, not just a type change — it closes G13. The deprecated token is always a known value in the migration table (`entry.reference`), so a `TokenId` lookup is feasible.
- **No new variant needed**.

### 2.2 `rules.rs:987` — E007 X-shorthand migration table path

```rust
format!(
    "X-shorthand declassification code {text:?} is deprecated; \
     use {:?}",
    entry.replacement
)
```

- Same pattern: `text` is raw document bytes; `entry.replacement` is a canonical token string.
- **Mapping**: `MessageTemplate::SupersededToken` — same as 2.1.
- **Gap**: Same `TokenId` lookup concern.
- **No new variant needed**.

### 2.3 `rules.rs:1014` — E007 X-shorthand pattern-stripped path

```rust
format!(
    "X-shorthand declassification code {text:?} is deprecated; \
     use {replacement:?}"
)
```

- Same pattern.
- **Mapping**: `MessageTemplate::SupersededToken`. Here `replacement` is a derived string (pattern-stripped), not a CVE-registered token. The `expected_token: Option<TokenId>` field will be `None` for these pattern-derived cases (no CVE registration). That is acceptable — the template still names the migration category.
- **No new variant needed**.

### 2.4 `rules.rs:1369` — C001 corrections-map message

```rust
format!("corrections map: {text:?} → {replacement:?}")
```

- Both `text` and `replacement` are raw strings. `text` is the original document bytes — G13 violation.
- **Mapping**: `MessageTemplate::CorrectionsApplied` — args: `token` could carry the matched pattern as a `TokenId` when one is registered; `expected_token` for the replacement.
- **Gap**: The corrections map uses arbitrary `String → String` mappings from `.marque.toml`. The match key `text` is NOT guaranteed to be a registered `TokenId`. Per `message.rs:247`: "token (the matched substring as a `TokenId` projection when one is registered; otherwise represented at the per-rule call site)". The doc comment explicitly acknowledges this case — `token` stays `None` when no `TokenId` projection exists.
- **No new variant needed**.

### 2.5 `rules.rs:4767` — E035 banner SCI rollup missing markings

```rust
format!(
    "banner SCI block is missing markings present in the page's \
     portions (systems, compartments, and/or sub-compartments): {}",
    missing.join("; ")
)
```

- `missing` is a `Vec<String>` of SCI system/compartment/sub-compartment names — these are canonical token labels derived from the registered vocabulary, not document bytes. They are on the G13 permitted list as "token canonicals".
- **Mapping**: `MessageTemplate::BannerRollupMismatch` — args: `category` = `CategoryId::Sci` (the axis that disagreed).
- **Gap (LOW)**: The "which specific SCI systems/compartments are missing" information is lost in the migration — `BannerRollupMismatch` with `category=Sci` is less specific than the current message. This is **acceptable by design**: the `MessageArgs` permitted types are `TokenId`/`CategoryId`/`Span`/`Blake3Hash`/`Confidence`/`FeatureId`. A `Vec<TokenId>` for missing items is NOT in the closed set. Downstream renderers receive `template=BannerRollupMismatch, category=Sci` and look up the specifics from the `Diagnostic.span` + the live marking if needed. The audit record intentionally loses the "which SCI systems" list.
- **If the PM wants per-system identification preserved**: add `feature_ids: SmallVec<[FeatureId; 4]>` entries where each `FeatureId` encodes a specific SCI system. This would require adding `FeatureId` variants for the SCI system set — a coordinated `MARQUE_AUDIT_SCHEMA` bump. Defer to PM decision; default recommendation is lose the detail in C and file a follow-up.
- **No new variant needed** with the default approach.

### 2.6 `helpers.rs:74` — E012 dual classification constraint message

```rust
format!(
    "marking has both US ({}) and foreign ({}) classification; §H.3 p55 mandates \
     these are mutually exclusive. CAPCO's pattern when US and non-US classifications \
     are commingled is to express the overall as a US classification with foreign \
     provenance in an FGI block (§H.3 p57 JOINT derivative use; §H.3 p59 Example 4 \
     note); consult §H.7 for the FGI marking format",
    us.banner_str(),
    foreign_desc
)
```

- **Important**: this `message: String` lives in `ConstraintViolation`, NOT in `Diagnostic`. `ConstraintViolation.message: String` is in `marque-scheme` and **stays `String`** (cannot become `Message` — no crate-graph path). This is NOT a C migration target.
- The `format!` at `helpers.rs:74` feeds `ConstraintViolation.message`, which later gets carried into a `Diagnostic.message` at the bridge layer. The bridge layer will copy the `String` into a `Message::new(SomeTemplate, args)` call — the template selection and arg mapping happen at the bridge, not in the helper.
- **Action for C**: At the `ConstraintViolation → Diagnostic` bridge in `rules_declarative.rs`, map `ConstraintViolation.message` → a `Message` via a `message_from_constraint` function that inspects the `constraint_label` to determine the template.
- **No new variant needed** — `ConflictsWith` covers the dual-classification case.

### 2.7 `helpers.rs:114` — E014 JOINT REL TO coverage constraint message

```rust
format!(
    "JOINT participants [{}] must appear in REL TO list",
    missing.join(", ")
)
```

- Same `ConstraintViolation.message` situation as 2.6. Not a direct C migration target. Bridge maps it to `MessageTemplate::RequiredByPresence` with `token = TOK_JOINT` as `TokenId`.
- **No new variant needed**.

### Summary — New MessageTemplate variants required: ZERO

All 7 format! sites map to existing templates. The `MARQUE_AUDIT_SCHEMA` stays on `marque-mvp-3` in C as planned (no new variants, no bump).

---

## 3. `citation!()` Macro Design Proposal

C-FOLLOWUP-2 from the deferred-findings register notes that `Citation::new(AuthoritativeSource::Capco2016, SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()), NonZeroU16::new(61).unwrap())` is ~120 characters for a 9-character string. With ~57 Diagnostic call sites plus ~80 catalog rows (most staying `&'static str` in C but eventually migrating), this verbosity is operationally painful.

**Recommendation: ship `citation!()` sugar in C's opening commit, before the bulk migration commits.**

**Macro signature**:

```rust
// In crates/rules/src/citation.rs or a new crates/rules/src/macros.rs

/// Ergonomic constructor for `Citation` values.
///
/// Accepts `citation!(§<L>.<sub> p<page>)` or
/// `citation!(§<L>.<sub> Table <N> p<page>)` syntax at compile time.
/// Returns a `const`-compatible `Citation` value.
///
/// # Examples
/// ```
/// use marque_rules::citation;
/// const SCI_GRAMMAR: Citation = citation!(§H.4 p61);
/// const CAVEATED_FDR: Citation = citation!(§B.3 Table 2 p21);
/// ```
#[macro_export]
macro_rules! citation {
    // §L.sub p<page>
    (§ $letter:ident . $sub:literal p $page:literal) => {
        $crate::Citation::new(
            $crate::AuthoritativeSource::Capco2016,
            $crate::SectionRef::new($crate::SectionLetter::$letter)
                .with_subsection(
                    ::core::num::NonZeroU8::new($sub).unwrap()
                ),
            ::core::num::NonZeroU16::new($page).unwrap(),
        )
    };
    // §L.sub Table N p<page>
    (§ $letter:ident . $sub:literal Table $table:literal p $page:literal) => {
        $crate::Citation::new(
            $crate::AuthoritativeSource::Capco2016,
            $crate::SectionRef::new($crate::SectionLetter::$letter)
                .with_subsection(::core::num::NonZeroU8::new($sub).unwrap())
                .with_table(::core::num::NonZeroU8::new($table).unwrap()),
            ::core::num::NonZeroU16::new($page).unwrap(),
        )
    };
}
```

**Where it lives**: `crates/rules/src/lib.rs` (via `#[macro_export]` on the macro in `citation.rs`), re-exported through the prelude. `citation!` is available at any downstream consumer via `use marque_rules::citation`.

**Const compatibility**: Yes. Every constructor in the expansion is `const fn`. The macro expands to a `const`-evaluable expression, making it usable in `const` contexts (catalog rows, `static` constants, `const fn` bodies). This is the key advantage over a proc-macro alternative.

**Limitation**: The `$letter:ident` arm requires `H`, `B`, `A`, etc. as Rust identifiers — they are. `SectionLetter::$letter` interpolation works because the variants match the letter names exactly. However, `§` is not a Rust token and cannot appear inside the macro's match arm literally. The actual invocation site would be `citation!(H 4 p61)` or `citation!(§H.4 p61)` — the `§` sigil depends on tokenization. **Rust macro rules do not accept `§` as a token**; the macro signature above with `§ $letter:ident` will not parse. 

**Revised minimal-friction alternative**: use `capco!(H.4 p61)` syntax (without `§` sigil) or accept a string literal that the macro parses:

```rust
macro_rules! capco_cite {
    ($letter:ident . $sub:literal p $page:literal) => { ... };
    ($letter:ident . $sub:literal Table $table:literal p $page:literal) => { ... };
}
```

This is less visually distinct from `Citation::new(...)` and still requires updating citation-lint to recognize the macro form. Given D25.2's stated preference for const-fn surface over a validation macro, and the fact that citation-lint's AST scanner already handles structured `Citation` values (not just string literals), the **pragmatic recommendation is to NOT ship the macro in C**. Instead, use helper functions in the catalog files:

```rust
// In crates/capco/src/scheme/class_floor.rs or a shared capco_citation mod
const fn capco(letter: SectionLetter, sub: u8, page: u16) -> Citation {
    Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(letter)
            .with_subsection(unsafe { NonZeroU8::new_unchecked(sub) }),
        unsafe { NonZeroU16::new_unchecked(page) },
    )
}
```

(Using `new_unchecked` in a `const fn` is sound when the constant is guaranteed non-zero by construction; `page = 0` never appears in CAPCO citations.) This reduces each `citation!(§H.4 p61)` to `capco(SectionLetter::H, 4, 61)` — 28 chars vs 120. Ship the helper in C1 (opening commit) before the bulk migration.

**Verdict: No macro. Ship `capco()` private const-fn helper in `crates/capco` and a parallel `rules_cite()` helper (or `pub const fn` in `crates/rules`) for the engine sites.**

---

## 4. Test Coverage Plan

### 4.1 Compile-fail tests for Citation

The existing `Message` compile-fail proofs (doctests on `Message`, positive test at `message_no_freeform_ctor.rs`) need a `Citation` parallel:

**Add to `crates/rules/tests/citation_no_from_str.rs`** (new file):

```rust
// Positive control: Citation::new is reachable from external crate.
#[test]
fn citation_new_accepts_struct_construction() {
    let _ = Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()),
        NonZeroU16::new(61).unwrap(),
    );
}
```

**Add compile-fail doctest to `citation.rs`**:

```rust
/// ```compile_fail
/// use marque_rules::Citation;
/// let _: Citation = "CAPCO-2016 §H.4 p61".into();
/// ```
```

This proves `impl From<&str> for Citation` does not exist. Without this, a future contributor could add the impl silently (it wouldn't be caught by the existing tests).

### 4.2 Citation-lint real-parser round-trip (C-FOLLOWUP-1)

Per C-FOLLOWUP-1 from the deferred-findings register: add an integration test that round-trips `format!("{citation}")` through `tools/citation-lint/src/citation.rs::find_in_fragment`.

**Placement**: `crates/rules/tests/citation_lint_roundtrip.rs`

```rust
// Requires `citation_lint` in [dev-dependencies] for marque-rules — OR
// move the test to `tools/citation-lint/tests/` where the dependency
// is natural.

#[test]
fn citation_display_parses_in_citation_lint() {
    let c = Citation::new(
        AuthoritativeSource::Capco2016,
        SectionRef::new(SectionLetter::H).with_subsection(NonZeroU8::new(4).unwrap()),
        NonZeroU16::new(61).unwrap(),
    );
    let s = format!("{c}");
    // Invoke tools/citation-lint scanner
    let parsed = citation_lint::find_in_fragment(&s);
    assert!(parsed.is_some(), "citation-lint rejected: {s:?}");
    let parsed = parsed.unwrap();
    assert_eq!(parsed.letter, 'H');
    assert_eq!(parsed.subsection, Some(4));
    assert_eq!(parsed.page, 61);
}
```

**Dependency concern**: Adding `citation_lint` as a dev-dependency to `marque-rules` creates a reverse dependency on a tool crate. Cleaner placement: put the round-trip test in `tools/citation-lint/tests/` where the dependency is already natural. Mark it as a CI-integration test (`#[cfg(feature = "integration")]` or `#[ignore]` with a comment).

### 4.3 Behavior coverage for the new Diagnostic shape

The existing test suite at `crates/capco/tests/` exercises `Engine::lint` end-to-end. These tests work by comparing `Diagnostic.rule`, `Diagnostic.span`, and `Diagnostic.severity` — they do NOT inspect `Diagnostic.message` content (the existing `ExpectedDiagnostic` struct has no `message` field). The `Diagnostic.message: Box<str> → Message` change is **transparent to existing tests** as long as the engine still emits diagnostics with the correct rule/span/severity.

**Tests that MUST be updated** — internal unit tests that construct `Diagnostic` directly:

- `crates/engine/src/output.rs:219–284` (5 `Diagnostic::new` test-only sites): these use `"test"` as string literals for both message and citation. After migration they must provide `Message::new(...)` and `Citation::new(...)` values. Using `MessageTemplate::UnrecognizedToken` + `MessageArgs::default()` as the test-fixture `Message` is fine — the tests only check count/severity, not message content.

- `crates/engine/src/engine.rs:6808` (`text_corr_no_fix` test site): same pattern.

- `crates/capco/tests/s004_audit_content_ignorance.rs` + `crates/capco/tests/rules_us1.rs` — both are `#![cfg(any())]`-gated. Per C-FOLLOWUP-4, lifting the cfg gate IS part of C's scope. When lifted:
  - `s004_audit_content_ignorance.rs` tests that `Diagnostic.message` contains no document-content bytes. After migration, the test should assert `diagnostic.message.args().token` is a `TokenId` (not a raw string), and optionally that `digest.is_none()` (no content hash present). The test's purpose is perfectly served by inspecting the closed `Message` fields.
  - `rules_us1.rs` pre-migrated shape (per B-3c finding): already uses `scheme.canonicalize(parsed.attrs)` — the `Diagnostic.message` shape rewrite is the remaining work.

- `crates/engine/tests/rule_panic_isolation.rs` (1 `Diagnostic::new` site per PM scope count): update similarly to output.rs.

**Tests that migrate transparently** (no change needed): all `crates/capco/tests/*.rs` integration tests using `Engine::lint` via `ExpectedFixture` sidecars.

---

## 5. Pre-existing `clippy::question_mark` Fix

File: `crates/core/src/parser.rs:2199`

Current code (lines 2197–2203):

```rust
let (prefix, full_form) = if let Some(p) = trimmed.strip_suffix(" EYES ONLY") {
    (p, true)
} else if let Some(p) = trimmed.strip_suffix(" EYES") {
    (p, false)
} else {
    return None;
};
```

The clippy lint fires on the `else if let Some(p) = trimmed.strip_suffix(" EYES")` arm because `else { return None; }` can be replaced by the `?` operator.

**Exact one-line edit** (replaces lines 2199–2202):

```rust
let (prefix, full_form) = if let Some(p) = trimmed.strip_suffix(" EYES ONLY") {
    (p, true)
} else {
    let p = trimmed.strip_suffix(" EYES")?;
    (p, false)
};
```

**Stable clippy validity**: `clippy::question_mark` is a stable lint (has been present in `clippy` since pre-1.65; not nightly-only). The project memory `feedback_clippy_nightly_vs_stable_drift` warns that local nightly may diverge from CI stable for some lints — `clippy::question_mark` is not in the nightly-only set; it fires on stable. Verified via clippy lint tracking: `clippy::question_mark` is in the `clippy` stable channel registry.

The fix is a no-op behavior change — the semantics of `strip_suffix(" EYES").ok_or(())?.as_str()` vs `return None` are identical since the function returns `Option<_>`. The `?` operator on an `Option` in an `Option`-returning function is exactly what `clippy::question_mark` recommends.

**Placement**: C1 (opening housekeeping commit, before any `Diagnostic` field-type changes).

---

## 6. Build-Green Commit Sequence

Each commit must leave `cargo check --workspace` green. The `Diagnostic` field-type change is atomic — no transitional dual-field per D25.3.

**Key constraint**: changing `Diagnostic.message: Box<str>` → `Message` and `Diagnostic.citation: &'static str` → `Citation` simultaneously changes the type of every constructor call site. This is a single-commit breaking change across ~57 call sites — cannot be staged without breaking intermediate states.

**Strategy**: migrate all 57 call sites in ONE commit after all infrastructure is in place. Commit sequence:

### C1: Pre-existing `clippy::question_mark` housekeeping
- `crates/core/src/parser.rs:2199` — one-line `?` replacement
- `cargo check --workspace && cargo clippy --workspace -- -D warnings` must pass

### C2: `AuthoritativeSource` additions + citation constants
- Add `AuthoritativeSource::Config` and `AuthoritativeSource::EngineInternal` variants to `crates/rules/src/citation.rs`
- Define `pub const DECODER_CITATION: Citation`, `pub const R002_CITATION: Citation`, `pub const CORRECTIONS_MAP_CITATION: Citation` constants in appropriate crates
- Add `capco()` private const-fn helper in `crates/capco/src/` (or a shared `crate::citation_helpers` module)
- Compile-fail doctest: `impl From<&str> for Citation` does not exist (add to citation.rs)
- `cargo check --workspace` green (all existing code still compiles — no field types changed yet)

### C3: Stale forward-pointer comment updates (C-FOLLOWUP-3)
- Update 5 doc-comments at `crates/scheme/src/scheme.rs:604, 699, 734` and `crates/capco/src/scheme/marking_scheme_impl.rs:587, 674`
- Text change only; no compilation impact
- `cargo check --workspace` trivially green

### C4: `FixDiagnosticParams.message: String → Message` in capco's internal helper
- Change `FixDiagnosticParams.message: String` → `pub message: Message` in `crates/capco/src/rules.rs:4112`
- Update `make_fix_diagnostic` to accept `Message` (line 4131) — removes the `.into()` conversion
- Update the 5 `message: format!(...)` call sites in `rules.rs:871, 987, 1014, 1369, 4767` to `message: Message::new(...)`
- Update the 2 `message: format!(...)` sites in `helpers.rs:74, 114` (these are `ConstraintViolation.message: String`, NOT `FixDiagnosticParams` — these stay as `String`; see §2.6/§2.7)
- `FixDiagnosticParams` is private to `crates/capco`, so this change is contained
- `cargo check -p marque-capco` must pass; `cargo check --workspace` must pass

### C5: Lift `#![cfg(any())]` gate from s004 + rules_us1; rewrite tests for new Diagnostic shape (C-FOLLOWUP-4)
- Remove `#![cfg(any())]` from `crates/capco/tests/s004_audit_content_ignorance.rs` and `crates/capco/tests/rules_us1.rs`
- Rewrite test bodies to work with the `Diagnostic.message: Message` shape (not yet live — tests will still fail compilation until C6 lands because `Diagnostic.message` is still `Box<str>` at this point)
- **Alternative**: defer this to C6 to avoid a broken-compilation intermediate. Recommendation: defer to C6.

### C6: Atomic `Diagnostic` field-type migration (the large commit)
- `crates/rules/src/lib.rs:1120` — `pub message: Box<str>` → `pub message: Message`
- `crates/rules/src/lib.rs:1123` — `pub citation: &'static str` → `pub citation: Citation`
- All 5 Diagnostic constructor signatures (`new`, `with_fix`, `with_fix_at_span`, `text_correction`, `info`) — change parameter types
- All ~57 call sites in `crates/capco/src/rules.rs`, `crates/engine/src/engine.rs`, `crates/engine/src/output.rs`, `crates/engine/tests/rule_panic_isolation.rs`
- Lift `#![cfg(any())]` from `s004` + `rules_us1` and update test bodies (if not done in C5)
- `cargo check --workspace` must pass
- `cargo test --workspace` must pass

### C7: Test additions (citation compile-fail doctest, citation-lint round-trip stub, `message_args_closed_set` update if new fields touched)
- Add `compile_fail` doctest for `impl From<&str> for Citation` (if not already in C2)
- Add `crates/rules/tests/citation_no_from_str.rs` positive control
- `cargo test --doc -p marque-rules` (compile-fail proofs still enforced)
- `cargo test --workspace` must pass

**Total commits**: C1–C7, with C6 being the large atomic migration. C6 will likely be ~300–400 lines of mechanical call-site changes. The implementation agent should use a `sed`-or-`grep`-assisted approach to stage all sites simultaneously.

---

## 7. Quantified Scope and Risk Register

### Lines-of-diff estimate

| Surface | Lines changed |
|---|---|
| C1 clippy fix (`parser.rs`) | ~3 |
| C2 `AuthoritativeSource` + citation constants | ~60 |
| C3 stale comment updates | ~10 |
| C4 `FixDiagnosticParams.message` + 5 format! sites | ~40 |
| C6 `Diagnostic` field type + all constructors + ~57 call sites | ~350 |
| C7 tests | ~60 |
| **Total** | **~523 lines** |

### Files touched

`crates/core/src/parser.rs`, `crates/rules/src/lib.rs`, `crates/rules/src/citation.rs`, `crates/rules/tests/citation_no_from_str.rs` (new), `crates/engine/src/engine.rs`, `crates/engine/src/output.rs`, `crates/engine/tests/rule_panic_isolation.rs`, `crates/capco/src/rules.rs`, `crates/capco/tests/s004_audit_content_ignorance.rs`, `crates/capco/tests/rules_us1.rs`, `crates/scheme/src/scheme.rs` (comments only), `crates/capco/src/scheme/marking_scheme_impl.rs` (comments only)  
**~12 files**, 2 new test files.

### Risk register

| ID | Severity | Description | Mitigation |
|---|---|---|---|
| R-C1 | HIGH | `ConstraintViolation.citation: &'static str` is in `marque-scheme` (graph leaf). Migrating it to `Citation` would create a `scheme → rules` dep. | **Confirmed out of scope**: C migrates only `Diagnostic.citation`. Bridge layer converts `&'static str` to `Citation` at conversion time. |
| R-C2 | HIGH | `AuditNote.citation: &'static str` is in `marque-rules` — it CAN migrate. But it is not in PM scope. Accidental migration could bloat C's diff. | Explicitly exclude `AuditNote` from C. File GH-FOLLOWUP-C-1. |
| R-C3 | MEDIUM | `render_message()` function does not exist. Audit emit and CLI rendering currently use `format!("{}", d.message)`. After migration `Message` has no `Display`. If any render path is missed in C6, the build fails. | Pre-C6 grep: `grep -rn "\.message" crates/engine/src/ crates/wasm/src/ crates/marque/src/` — verify all render callsites are updated in C6. |
| R-C4 | MEDIUM | `ConstraintViolation → Diagnostic` bridge must map `String → Message`. The bridge site in `rules_declarative.rs` does not have a clean `constraint_label → MessageTemplate` mapping today. | Add `bridge_message(constraint_label: &'static str) -> Message` function to `rules_declarative.rs` as part of C6. The ~25 constraint labels are a closed static set. |
| R-C5 | LOW | Engine `output.rs` test-only `Diagnostic::new` sites use `"test"` for citation. After migration `"test"` is not a `Citation`. | All 5 sites use sentinel values; use `Citation::new(AuthoritativeSource::Config, SectionRef::new(SectionLetter::A), NonZeroU16::new(1).unwrap())` as the test sentinel. |
| R-C6 | LOW | `DECODER_CITATION: &str = "CAPCO-2016 §A.6 p15"` in `engine.rs` — migrating to `const Citation` requires importing `Citation`, `SectionRef`, etc. into engine.rs (currently imported via `marque_rules`). | `Citation` is already in `marque_rules` which is already imported. No new dep. |
| R-C7 | LOW | Some `SciPerSystemRow.citation` and `ClassFloorRow.citation` rows use multi-page citation strings (`"CAPCO-2016 §H.4 p87 + p91 + p95"`) that don't map to a single `Citation` struct. | These stay `&'static str` in C — they are in catalog structs, not in `Diagnostic`. Flag multi-page citations as C-FOLLOWUP-C-2 (require either multiple `Citation` values or a `Citation` with `pages: &'static [PageNumber]` extension at scheme #2 adoption time). |
| R-C8 | LOW | `make_fix_diagnostic` helper internal to capco carries both `message: String` and `citation: &'static str`. Changing message to `Message` in C4 while citation stays `&'static str` is an intermediate inconsistency. | C4 changes `message` only; C6 changes `citation`. Intermediate is fine since `make_fix_diagnostic` is `pub(crate)`. |

---

## 8. Additional Finding: `Diagnostic::text_correction` Citation Signature

`Diagnostic::text_correction` at `crates/rules/src/lib.rs:1285–1311` also carries `citation: &'static str`. This constructor is used by `make_fix_diagnostic` (rules.rs) and directly by `crates/engine/src/engine.rs:1854` (pre-scanner corrections path).

The `CORRECTIONS_MAP_CITATION` migration (§1.2) must also cover the pre-scanner path at `engine.rs:1859`. After C2 defines `pub const CORRECTIONS_MAP_CITATION: Citation`, this site changes from:

```rust
// engine.rs:1859 — before
CORRECTIONS_MAP_CITATION,   // &'static str

// After C6
CORRECTIONS_MAP_CITATION,   // Citation — same binding name, new type
```

No call-site change if the constant name is preserved and type is updated. This is the cleanest migration shape for the engine pre-scanner path.

---

## 9. Carryover Item Dispositions

| Item | Action |
|---|---|
| **C-FOLLOWUP-3** (stale forward-pointer comments) | Land in **C3** (mechanical, safe early commit) |
| **C-FOLLOWUP-4** (cfg-gate lift on s004 + rules_us1) | Land in **C6** (atomically with the Diagnostic shape change, since the test bodies depend on `Message` fields) |
| **C-FOLLOWUP-5** (clippy::question_mark) | Land in **C1** (opening housekeeping commit) |
| **C-FOLLOWUP-6** (byte-equivalence §-citation at 3c.2.E) | NOT in C — confirmed deferred to 3c.2.E |
| **C-FOLLOWUP-1** (citation-lint real round-trip test) | Land in **C7**; test goes to `tools/citation-lint/tests/` to avoid dep inversion |
| **C-FOLLOWUP-2** (citation!() macro) | Do NOT ship the macro. Ship `capco()` const-fn helper in **C2** instead |

---

## 10. Open Questions for PM Resolution

**OQ-C1 (HIGH)**: Should `AuditNote.citation: &'static str` migrate to `Citation` in C (it's in `marque-rules`, so no crate-graph issue)? Or defer to 3c.2.E for consolidated cleanup? Recommendation: defer to avoid scope creep.

**OQ-C2 (MEDIUM)**: For multi-page citation strings like `"CAPCO-2016 §H.4 p87 + p91 + p95"` in `SciPerSystemRow`, should C introduce a `Citation::range(start_page, end_page)` extension to `Citation`, or leave these as `&'static str` permanently? Recommendation: leave as `&'static str` — the Citation struct's single-page design is correct per the PA design and the multi-page form is a catalog-row concern (not user-facing `Diagnostic` output).

**OQ-C3 (MEDIUM)**: Does the PM want the SCI rollup "missing systems" list preserved in the audit record? Currently `BannerRollupMismatch` + `category=Sci` drops the per-system detail. If yes, requires adding new `MessageArgs` field (`missing_tokens: SmallVec<[TokenId; 8]>`) and a coordinated `MARQUE_AUDIT_SCHEMA` bump — which contradicts the no-schema-bump-in-C stance. Recommendation: drop the detail in C, file as follow-up.

**OQ-C4 (LOW)**: For `AuthoritativeSource::Config` and `AuthoritativeSource::EngineInternal` — should both be added in C2, or just `Config` (used by `CORRECTIONS_MAP_CITATION`) while `EngineInternal` (for `R002_CITATION`) defers? Recommendation: add both in C2; the `#[non_exhaustive]` attribute already makes this additive and non-breaking.

---

*Pre-existing `clippy::question_mark` fix confirmed valid under stable clippy (not nightly-only). Constitution VIII propagation rule: all citations in this document re-verified against `crates/capco/docs/CAPCO-2016.md` at authorship where applicable; engine-synthetic citations (`§A.6 p15` for DECODER_CITATION) confirmed against `engine.rs:113–118` doc comment which cross-references the CAPCO-2016.md table of contents at line 49.*
