# PR 9c.2 / FR-048 Rust-idiom + Type-Surface Preflight

**Branch**: `refactor-006-pr-9c2-fr048` off `f249c033`. **Companion**: `pr9c2-architect-preflight.md`.

## 1. Type-surface

Architect rules out `Constraint::Custom` (no page-context access). Hand-written `Rule<CapcoScheme>` with `(&CanonicalAttrs, &RuleContext<'a>)`, S005/S006 shape. Idiom: `let Some(MarkingClassification::Nato(_)) = &attrs.classification else { return vec![]; };` (single-arm let-else; `match` here is a `clippy::single_match` smell). `ctx.marking_type` is `MarkingType`, not `Option<MarkingType>` (rules.rs:299). `ctx.page_context: Option<Arc<PageContext>>` (rules.rs:331).

## 2. REL TO USA, NATO predicate

`rel_to: Box<[CountryCode]>` (canonical.rs:117). `CountryCode::USA` is `pub const` (attrs.rs:1383); NATO has no const equivalent. **Byte-compare via `as_bytes()` at the one site; do NOT add `CountryCode::NATO`** (would bump `marque-ism` surface — Constitution VII §IV scheme-adoption-PR restriction).

```rust
let has_usa = attrs.rel_to.contains(&CountryCode::USA);
let has_nato = attrs.rel_to.iter().any(|c| c.as_bytes() == b"NATO");
if has_usa && has_nato { return vec![]; }
```

Do **not** use `rel_to_covers` (scheme.rs:4684) — tetragraph expansion would accept `REL TO USA, DEU, GBR, FRA, ...` as covering NATO, which §H.7 p127 does not endorse.

## 3. Diagnostic + fix emission

**`Severity::Suggest`** (severity.rs:121 — fires without auto-applying). User's "Warn + Suggest" maps here; S005/S006 precedent. **`Phase::WholeMarking`** — splice can augment a non-classification token (existing REL TO block); `Phase::Localized`'s "strictly inside a single token" contract (lib.rs:241) fails for that branch.

API: `Diagnostic::text_correction` (lib.rs:1032), NOT `Diagnostic::with_fix_at_span` — no `FixIntent` variant repairs a country-code list (`FactAdd` carries `FactRef::Cve(TokenId) | OpenVocab(S::OpenVocabRef)`; REL TO countries are neither — fix_intent.rs:143). `FixSource::BuiltinRule` (lib.rs:392; `SuggestFromIntent` does **not** exist). Confidence: `Confidence::strict(0.8)` (example-derived, calibrates with S005/S006). Message and `citation: &'static str` are both `&'static`-derived; zero `format!` interpolation of input bytes (G13).

## 4. Replacement helpers

Splice constructed from `&[CountryCode]` only — never reads document bytes (G13). `CountryCode::as_str()` is UTF-8-safe by construction (attrs.rs:1440). Two helpers, one responsibility each:

- `build_fr048_rel_to_insertion(&[CountryCode]) -> String` — no REL TO block; emit `REL TO USA, NATO`, splice after the classification token's trailing `//`.
- `build_fr048_rel_to_augmentation(&[CountryCode]) -> String` — existing REL TO; emit `REL TO USA, <sorted-existing-merged-with-NATO>`. Reuse `build_rel_to_replacement:3192`'s USA-first / alpha-sort skeleton.

## 5. PageContext helper + perf

```rust
#[inline]
pub fn is_solely_nato_classified(&self) -> bool {
    !self.portions.is_empty()
        && self.portions.iter().all(|a| {
            matches!(&a.classification, Some(MarkingClassification::Nato(_)))
                && a.fgi_marker.is_none()
        })
}
```

O(n) over `portions.len()` (typically ≤10); short-circuits via `Iterator::all`. No accumulator field — perf gain (~50ns/portion) is negligible vs. invariant-maintenance on `add_portion`; YAGNI until a second consumer. Place after `expected_classification:229`. WASM size delta <1 KB; no baseline bump.

## 6. Closed-enum + message template

No new `MessageTemplate` variant. `Diagnostic::text_correction` takes `impl Into<Box<str>>` (lib.rs:1036). The closed-enum migration is gated behind PR 3c.2 (deferred). G13 preserved — message is `&'static`-derived, zero input-byte interpolation. Future variant (post-3c.2): `BareNatoRequiresRelTo` with `MessageArgs { token: Some(TOK_NATO), .. }`.

## 7. Error handling + tests

1. `Conflict` classification: `matches!(_, Nato(_))` false → silent. E012 owns it.
2. `dissem_us` + `Nf` (NOFORN): structurally invalid; `capco/noforn-conflicts-rel-to` (scheme.rs:2466) owns it. **Early-return** `if attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf)) { return vec![]; }` — Constitution VIII.
3. Banner candidate: gate `ctx.marking_type == MarkingType::Portion`; roll-up flows through `BannerMatchesProjectedRule`.
4. First portion (`page_context.is_none()`): fire conservatively; solely-NATO docs silence via `.marque.toml`.
5. `Joint(_)` with NATO participant: outer-variant gate → silent. E014 owns it.

**Tests** (`crates/capco/tests/fr048_bare_nato_rel_to.rs`): mirror `nato_atomal_aea_routing.rs:50-83` helpers verbatim. ≥8 cases per architect's matrix plus a 9th idempotency test (force-promote in test config, re-parse, assert 0 FR-048 diagnostics — Constitution V convergence). Bump `post_3b_registration_pin.rs` and `corpus_parity.rs:170-194` count (47 → 48).

## 8. Open Rust decisions for the PM

- **Severity**: recommend `Severity::Suggest` (matches S005/S006; FixIntent advisory by construction). Confirm interpretation of user's "Warn + Suggest" — user can opt into `Warn` via `[rules] S007 = "warn"`.
- **Rule ID**: recommend `S007` (S-prefix signals suggest-channel at registration). Alternates `W004` / new `R###` are weaker.
- **Phase**: recommend single rule at `Phase::WholeMarking`. Splitting into Localized-insertion + WholeMarking-augmentation saves one pass on the insertion branch but doubles surface and complicates migration.
- **Two splice helpers vs one**: recommend two (single-responsibility, independently testable).
- **`CountryCode::NATO` constant**: defer — adding it now is Constitution VII §IV violation; byte-compare at the one site is right idiom until second consumer materializes.
- **`is_solely_nato_classified` accumulator vs derivation**: recommend derivation (YAGNI). Revisit when Pattern A NATO-implies-NOFORN variants need the same predicate.
- **`FgiMarker` exclusion from solely-NATO predicate**: confirm. NATO + populated `fgi_marker` is conceptually NATO+FGI commingled (per `project_nato_transmutes_to_fgi.md`), distinct from pure NATO. Excluding is the safer default.
