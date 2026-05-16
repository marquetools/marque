# PR 9c.2 / FR-048 Architectural Preflight

**Branch**: `refactor-006-pr-9c2-fr048` off `f249c033` (post PR 9c.1).
**Scope**: Bare NATO classification in a US-classified document MUST carry `REL TO USA, NATO`. Severity Warn + Suggest.
**Authoritative citation**: CAPCO-2016 §H.7 p127 Notional Example 2 — `(//CTS//BOHEMIA//REL TO USA, NATO)`. Example-derived, no "MUST" prose; Warn+Suggest is the right severity floor.

---

## 1. Recommended modeling

**Reject `Constraint::Custom`. Adopt a hand-written `Rule<CapcoScheme>` impl — same shape as S005/S006 (Path A from `decisions/02-catalog-shape.md` D4).**

Load-bearing reason: `MarkingScheme::evaluate_custom` and `evaluate_custom_by_attrs` (`crates/capco/src/scheme.rs:3154`) receive `&CanonicalAttrs` only. They cannot read `RuleContext::page_context` or `RuleContext::page_marking`. FR-048 is **page-conditional by definition** (the "solely-NATO doc" carve-out and the "parent has US classification axis" gate both require enumerating sibling portions). The followup tracking the constraint-context extension is `specs/006-engine-rule-refactor/followups/constraint-context-extension.md` — already cites S005/S006 as the precedent for deferring catalog migration until the trait surface lands. FR-048 inherits the same blocker by the same reasoning.

Reject `PageRewrite`: this is not an axis-rewrite (no token added to the projected banner); it is a per-portion advisory. Reject `ClosureRule`: closure is fact-propagation, not deficit detection.

Place the new rule next to `RelToOpaqueUncertainReductionSuggestRule` in `crates/capco/src/rules.rs` (~line 2181). Register in `CapcoRuleSet::new()` (~line 275). Use `RuleId::new("S007")` to extend the Suggest series; reuse the `JointUsaFirstRule` / `EyesOnlyConvertToRelToRule` patterns for `text_correction` emission.

---

## 2. Implementation file map (dependency order)

1. **`crates/ism/src/page_context.rs`** (~after the `expected_rel_to` block, near line ~600): add `PageContext::is_solely_nato_classified() -> bool`. Pure derivation from existing `portions: Vec<CanonicalAttrs>`; no new accumulator field needed.
2. **`crates/capco/src/rules.rs`** ~line 2160 (just before `RelToOpaqueUncertainReductionSuggestRule`): add `BareNatoRequiresRelToRule` struct + `impl Rule<CapcoScheme>`. Add doc-block matching the S005/S006 migration-status pattern.
3. **`crates/capco/src/rules.rs`** ~line 275 inside `CapcoRuleSet::new()`: `Box::new(BareNatoRequiresRelToRule),`.
4. **`crates/capco/tests/fr048_bare_nato_rel_to.rs`** (new file): integration tests using `lint_text(...)` against the three user-provided examples + boundary cases.
5. **`crates/capco/tests/post_3b_registration_pin.rs`** + `crates/capco/tests/corpus_parity.rs:170-194`: bump the exact-rule-ID-set pin and the rule count (47 → 48).
6. **`specs/006-engine-rule-refactor/tasks.md`** T135: mark complete; add the §H.7 p127 citation breadcrumb.

---

## 3. Predicate body pseudocode

```rust
fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
    // Portion-level scope (FR-048 corrected semantic).
    if ctx.marking_type != Some(MarkingType::Portion) { return vec![]; }

    // Clause 1: bare NATO classification axis present.
    // Post-9c.1: NatoClassification has 5 bare-class variants only (NU/NR/NC/NS/CTS).
    // ATOMAL is on AEA axis; BOHEMIA/BALK on SCI axis — neither sets MarkingClassification::Nato.
    let Some(MarkingClassification::Nato(_)) = &attrs.classification else { return vec![]; };

    // Clause 2: not a solely-NATO document. Requires page-context.
    if let Some(page) = ctx.page_context.as_ref() {
        if page.is_solely_nato_classified() { return vec![]; }
    } // else: no page context yet (first portion in doc) — fire conservatively; pass-2 will re-evaluate if reset.

    // Clause 3: REL TO does not already cover {USA, NATO}.
    // NATO is a tetragraph; rel_to_covers expands tetragraphs to constituent trigraphs.
    // Membership check on the literal NATO tetragraph is what the §H.7 p127 example uses.
    let has_usa = attrs.rel_to.iter().any(|c| c == &CountryCode::USA);
    let has_nato = attrs.rel_to.iter().any(|c| c.as_str() == "NATO");
    if has_usa && has_nato { return vec![]; }

    // G13: message MUST NOT interpolate input bytes. Static text only.
    Diagnostic::text_correction(
        self.id(), Severity::Warn, candidate_span,
        "bare NATO classification in a US-classified document should carry \
         REL TO USA, NATO per §H.7 p127 Notional Example 2".to_owned(),
        "CAPCO-2016 §H.7 p127",
        canonical_replacement,   // built by helper, structural-only
        FixSource::SuggestFromIntent,
        Confidence::strict(0.85),
        None,
    )
}
```

**Replacement construction**: a `text_correction` splice that inserts `//REL TO USA, NATO` after the bare-NATO classification token (or augments an existing `REL TO ...` block to include `USA, NATO`). The exact splice point is the same mechanism `EyesOnlyConvertToRelToRule` uses (`build_rel_to_replacement` in `crates/capco/src/rules.rs:3192`).

---

## 4. PageContext helper signature

```rust
/// True iff every accumulated portion has `MarkingClassification::Nato(_)` as its
/// classification (and at least one portion exists). Returns false when ANY portion
/// has US/JOINT/FGI classification, OR when ANY portion carries a populated
/// `fgi_marker` (commingled FGI elevates the doc out of "solely-NATO" status).
///
/// Used by FR-048 (`BareNatoRequiresRelToRule`) to short-circuit the bare-NATO-
/// requires-REL-TO suggestion: in a solely-NATO doc, bare `(//CTS)` portions are
/// canonical per §H.7 p127 (alliance ownership is implicit).
///
/// CAPCO-2016 §H.7 p127 Notional Example 2.
pub fn is_solely_nato_classified(&self) -> bool {
    !self.portions.is_empty()
        && self.portions.iter().all(|a| {
            matches!(a.classification, Some(MarkingClassification::Nato(_)))
                && a.fgi_marker.is_none()
        })
}
```

No accumulator extension needed — pure read over `self.portions`.

---

## 5. Test fixture inventory

**File**: `crates/capco/tests/fr048_bare_nato_rel_to.rs`

| Case | Input | Expected S007 count |
|---|---|---|
| `example_a_nato_plus_us` | `(//NS)\n(S//REL TO USA, FVEY)` | 1 (on the `(//NS)` portion) |
| `example_a_already_rel_to` | `(//NS//REL TO USA, NATO)\n(S//REL TO USA, FVEY)` | 0 |
| `example_b_nato_plus_jpn_fgi` | `(//NU//REL TO USA, NATO)\n(//JPN U//NF)` | 0 |
| `example_c_nato_plus_us_fouo` | `(//CTS//REL TO USA, NATO)\n(U//FOUO)` | 0 |
| `example_c_relido_variant` | `(//CTS//REL TO USA, NATO)\n(U//RELIDO)` | 0 (FR-048 satisfied; banner roll-up to NOFORN per §D.2 Table 3 row 10 is separate concern) |
| `solely_nato_doc_two_portions` | `(//CTS)\n(//NS)` | 0 (solely-NATO carve-out) |
| `solely_nato_doc_one_portion` | `(//CTS)` | 0 |
| `atomal_only_no_bare_nato` | `(TS//RD/ATOMAL//FGI NATO//NOFORN)` | 0 (ATOMAL on AEA axis; no `MarkingClassification::Nato`) |
| `cts_atomal_in_us_doc` | `(//CTS//ATOMAL)\n(S//NF)` | 1 (bare NATO classification present even with ATOMAL on the AEA axis) |
| `bare_nato_us_doc_missing_only_nato` | `(//NS//REL TO USA, CAN)` + `(S//NF)` | 1 |
| `bare_nato_us_doc_missing_only_usa` | `(//NS//REL TO NATO, GBR)` + `(S//NF)` | 1 |

---

## 6. Open decision points (PM action items)

- **D-1 — `RuleContext::page_context` early-portion semantics.** When the first portion of a doc is parsed, `page_context` is empty / unavailable. Two safe defaults: (a) fire the diagnostic (conservative) OR (b) suppress until pass-2 when page-context is populated. Pass-2 rule scheduling means the diagnostic *will* re-evaluate, so (a) is harmless on a US-classified doc and (b) silently hides on a solely-NATO single-portion doc. **Recommendation: fire conservatively (a) — solely-NATO single-portion case is rare; pass-2 self-corrects via page-context.**
- **D-2 — RuleId allocation.** `S007` extends the Suggest series cleanly (S001–S006 in use). Confirm S007 over alternates (`W003`? — but it carries a fix, so `S` matches the Suggest convention).
- **D-3 — Spec wording amendment (FR-048).** The spec says "MUST trigger a declarative `Constraint` requiring `REL TO USA, NATO` derivation in the banner". Three corrections needed: (i) drop "declarative `Constraint`" — it's a hand-written `Rule` per `decisions/02-catalog-shape.md` D4 Path A and the active context-extension followup; (ii) "in the banner" is wrong — it's **portion-level**; banner roll-up follows automatically; (iii) "MUST" overstates the citation — change to "SHOULD" + Warn-severity. Flag for PM amendment; do **not** edit `specs/006-engine-rule-refactor/spec.md` from the implementation branch.
- **D-4 — Render-path round-trip.** Need confirmation that `(//CTS//REL TO USA, NATO)` round-trips cleanly through the render path post-9c.1. Likely fine (no new tokens), but the implementer should validate before claiming pass-2 idempotency.
- **D-5 — FGI-fold interaction.** Per `project_nato_transmutes_to_fgi` memory, `SECRET//FGI NATO//...` portions are FGI-fold (NATO in FGI position), distinct from FR-048's bare-NATO axis. The predicate's `MarkingClassification::Nato(_)` gate isolates correctly because FGI-fold portions have `MarkingClassification::Us(_)` + `fgi_marker = Some(NATO)`. Confirm the parser actually produces this shape (a quick `grep` test of `parse_fgi_classification` confidence).

---

## 7. Risk register

- **R-1 (M) — Double-diagnostic with W002 (`us-commingled-with-fgi`)**: W002 fires on US-classification + FGI marker; FR-048 fires on bare-NATO classification + US sibling. The two predicates are **disjoint by axis** (different `MarkingClassification` variants). No interference.
- **R-2 (L) — `capco/noforn-clears-rel-to` `PageRewrite` ordering**: portion-level FR-048 fires during `Phase::Localized` (single portion span); the PageRewrite runs over the rolled-up `ProjectedMarking` post-projection. Different phases, no schedule conflict.
- **R-3 (M) — Pass-1 vs pass-2 idempotency**: if pass-1 applies the suggest, pass-2 re-parses `(//CTS//REL TO USA, NATO)` — `has_usa && has_nato` is true → no re-fire. Idempotent by construction.
- **R-4 (H) — `(//CTS//ATOMAL)` shape in a US doc**: ATOMAL on AEA axis (post-9c.1) coexists with `MarkingClassification::Nato(_)`. FR-048 fires correctly. Audit-stream message must NOT include the ATOMAL token name (G13 — static string only).
- **R-5 (L) — RELIDO Example C variant**: `(U//RELIDO)` + `(//CTS//REL TO USA, NATO)` → banner rolls up to NOFORN per §D.2 Table 3 row 10. FR-048 is satisfied on the NATO portion regardless of the RELIDO sibling. The two concerns are orthogonal.
- **R-6 (M) — `is_solely_nato_classified` false positives on empty docs**: `!self.portions.is_empty()` guard prevents false-positive on a single-portion doc that hasn't parsed yet. Verified in the helper signature above.

---

## 8. Estimated implementation size

- **Production LOC**: ~140 LOC total
  - `PageContext::is_solely_nato_classified`: ~15 LOC + ~25 LOC doc
  - `BareNatoRequiresRelToRule` struct + impl: ~80 LOC including doc-block
  - Registration: 1 line
  - Replacement helper (reuse `build_rel_to_replacement` from `EyesOnlyConvertToRelToRule`): 0 net LOC, ~15 LOC of caller glue
- **Test LOC**: ~200 LOC
  - 11 test cases × ~15 LOC each (input string + assert)
  - 2 unit tests for `is_solely_nato_classified` (true / false / empty)
- **Registration-pin churn**: 2 LOC (rule count 47 → 48; exact-set add `"S007"`)

Total ≤ ~360 LOC. One day of implementation, two days with full multi-agent review chain.
