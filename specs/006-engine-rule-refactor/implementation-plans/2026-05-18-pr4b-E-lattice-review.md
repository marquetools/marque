<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# PR 4b-E Lattice-Domain Review

**Date:** 2026-05-18
**Reviewer scope:** Lattice algebra + §-citation source-fidelity for the
**5 new lattice helpers** landing in `crates/capco/src/lattice.rs`:
1. `NonIcDissemSet::from_attrs_iter` (lines 3331-3369)
2. `DeclassExemptionLattice` + `from_attrs_iter` + `JoinSemilattice` impl (lines 3399-3473)
3. `FgiSet::from_attrs_iter` (lines 3196-3273) — new constructor on existing type
4. `DisplayOnlyBlock` enum + `from_attrs_iter` + `JoinSemilattice` impl (lines 3539-3727)
5. Free `sci_controls_from_markings` (line 3160)

**Status:** Read-only. Findings only. PM triages.

## Methodology

1. Loaded `marque-lattice-consultant` skill (canonical lattice-shape guide).
2. Read `crates/capco/CAPCO-CONTEXT.md` (553 lines) end-to-end.
3. Read architect plan `docs/plans/2026-05-18-pr4b-E-page-context-deletion-plan.md`
   (411 lines) — §2 inventory + §3 Decision 5 + §10 OQ-7.
4. Verified every cited §X.Y pNN reference against `crates/capco/docs/CAPCO-2016.md`
   verbatim (the authoritative source per Constitution VIII).
5. Walked each helper's implementation body against the join laws
   (associativity / commutativity / idempotence / identity).
6. Cross-checked the new tests at `crates/capco/src/lattice.rs:4445-4933`
   for property-test coverage of the standard semilattice laws.

---

## Findings

### Severity grid

| ID | Title | Severity | Confidence | Block PR? |
|---|---|---|---|---|
| L-1 | `DeclassExemptionLattice::join` is non-commutative — violates `JoinSemilattice` trait contract | **HIGH** | high | **Yes — block on either trait-impl removal OR commutative redefinition.** |
| L-2 | `DeclassExemptionLattice` `§E.1 p31` citation is for the wrong section — duration-aware hierarchy lives at `§E.3 pp 32-33`, not `§E.1 p31` | **MEDIUM** | high | No — citation fix |
| L-3 | `NonIcDissemSet`: SBU-NF classified-context split keeps SBU in non-IC dissem; CAPCO §H.9 p178 says SBU is NOT reflected on classified portions (pre-existing semantic carried forward from PageContext) | **MEDIUM** | medium | No — preserve parity now, fix in follow-up |
| L-4 | `DisplayOnlyBlock::from_attrs_iter` unconditional banner-REL-TO subtraction will produce wrong output for §D.2 Table 3 Row 26 (release-implies-disclosure) — but tracked as deferred at `CAPCO-CONTEXT.md` lines 308-311 | **LOW** | high | No — deferred per CAPCO-CONTEXT §3 |
| L-5 | `DisplayOnlyBlock`: missing dedicated commutativity + idempotence property tests; coverage rides on associativity + identity-with-bottom + sentinel-absorbs alone | **LOW** | high | No — coverage gap |
| L-6 | `DeclassExemptionLattice` has no associativity test; doc-comment admits join is non-commutative so associativity is the only meaningful remaining law and it's untested | **LOW** | high | No — coverage gap, tied to L-1 |
| L-7 | `sci_controls_from_markings(portions: &[CanonicalAttrs])` — function name says `from_markings` but signature takes `CanonicalAttrs` (the architect plan §3 row 22 specified `&[SciMarking]`); name diverges from input | **LOW** | high | No — naming clarity |
| L-8 | `NonIcDissemSet::from_attrs_iter` is the production entry point but does not propagate `BoundedJoinSemilattice::bottom()` — the `Default` impl gives the bottom-equivalent but is not bound to the trait surface | **INFO** | medium | No — intentional per doc-comment |
| L-9 | `FgiSet::from_attrs_iter` synthesizes FGI marker from `MarkingClassification::Joint(_)` producers on every JOINT portion — produces non-`None` FgiSet on pure-JOINT pages, which is the legacy semantic but diverges from `marque-applied.md §4.8.5` worked example expecting `bare(JOINT, ...)` not FGI rollup | **INFO** | high | No — preserves PageContext byte-identity |

---

### L-1 — `DeclassExemptionLattice::join` is non-commutative (CRITICAL trait violation)

**File:** `crates/capco/src/lattice.rs` symbol `impl JoinSemilattice for DeclassExemptionLattice` (lines 3455-3473)

**Description.** The `JoinSemilattice` trait at `crates/scheme/src/lattice.rs:55-64` carries the rustdoc-level invariant:

> Implementors must satisfy the three join laws: **commutativity**, associativity, and idempotency.

The `DeclassExemptionLattice::join` body (lines 3467-3471) is **explicitly non-commutative by design** — the doc-comment at lines 3459-3460 admits this:

> Commutative? NO — last-observed is order-sensitive by design.

The match arm `(_, Some(b)) => Self(Some(*b))` returns the right operand's value when both are `Some`, so `Some(a).join(Some(b)) = Some(b)` but `Some(b).join(Some(a)) = Some(a)`. These are unequal when `a ≠ b`, breaking the trait's commutativity invariant.

The doc-comment justifies the violation with "Production composition routes through `from_attrs_iter`, never through repeated `join` calls; this impl is here for type-system symmetry with sibling lattices." This is a real argument BUT it's the wrong solution — implementing a broken trait to "match the shape" of sibling types poisons the trait surface. Any future caller that reaches for the `JoinSemilattice` blanket impls (e.g., a generic helper that folds a slice via `.join`) will produce order-dependent output without a compile error.

**Authority.** The trait contract at `crates/scheme/src/lattice.rs:55-64` is binding. The 2026-05-01 lattice-design plan §12 split `Lattice` into `JoinSemilattice + MeetSemilattice` halves precisely because some types satisfy only one half's laws — implementing a half with broken laws defeats the split's purpose.

**Cross-reference.** Production callsite is `crates/capco/src/scheme/marking.rs:685`:
```rust
DeclassExemptionLattice::from_attrs_iter(portions).into_inner()
```
The `.join` impl is exercised only by tests at lines 4652-4679. The impl is dead in production today, but the dead surface is a footgun.

**Suggested fix.** Two acceptable resolutions:
- **(a)** Remove the `impl JoinSemilattice for DeclassExemptionLattice`. The type stays — `from_attrs_iter` + `into_inner` give callers the only operations they actually need. The doc-comment's "type-system symmetry" justification dissolves because the symmetry is between **`from_attrs_iter` constructors on lattice-shaped types** (which `NonIcDissemSet` already follows without implementing `JoinSemilattice`), not between trait impls.
- **(b)** Redefine join to be commutative. Options:
  - `(Some(a), Some(b)) => Self(Some(max(a, b)))` if `DeclassExemption: Ord` and there's a meaningful order.
  - `(Some(a), Some(b)) => Self(Some(*a))` (left wins) — still non-commutative, equivalent bug.
  - Disagreement-promotes-to-bottom: `(Some(a), Some(b)) if a != b => Self(None)` — commutative + idempotent + associative, but loses signal.

**(a) is strongly preferred** because it preserves the helper's actual semantic (last-observed in document-order construction) while honoring the trait contract by not claiming a trait the type doesn't satisfy. The architect plan §10 OQ list never asked for the trait impl — it asked for the constructor. The trait impl is a downstream choice that should be undone.

**Confidence:** high. The trait doc-comment is unambiguous; the impl admits the violation; the impl is dead in production.

---

### L-2 — `DeclassExemptionLattice` cites the wrong CAPCO section

**File:** `crates/capco/src/lattice.rs` doc-comment at lines 3418-3422

**Description.** The doc-comment claims:

> §-authority (verified 2026-05-18 against `crates/capco/docs/CAPCO-2016.md`):
> - §E.1 p31 (EO 13526 default-duration framing).
> - §H.6 p104 (declass exemption interaction with AEA — orthogonal, ...).

Verification against the source:
- **§E.1 p31** is "Original Classification Authority", a procedural section about who can originally classify. The exemption catalog (25X/50X/75X) lives under **§E.1 page 31** for the bare list of allowed values, but the "longest period of protection" hierarchy (which is what the helper's "Phase 3 TODO carry-over" gestures at) lives under **§E.3 (Multiple Sources and the Declassify On Line Hierarchy)** on **pp 32-33**. Per `CAPCO-2016.md` table of contents at line 75 of the manual ("3. (U) Multiple Sources and the Declassify On Line Hierarchy ..... 32"). The "longest period of protection" prose appears at the manual's lines 665+ ("the exemption with the date or event that provides the longest period of protection").
- **§H.6 p104** is the RD (Restricted Data) marking definition, NOT a declass-exemption-relevant section. The doc-comment hedges with "orthogonal" but the citation is misleading — p104 is about RD's prohibition on automatic declassification, which is a different invariant from the helper's "last-observed exemption" semantic.

**Authority.** Constitution Principle VIII binds citation integrity: "Citations MUST refer to a real passage in the authoritative source ... and accurately reflect what that passage says." `§E.1 p31` describes original classification authority + lists exemption values, but the **duration-aware comparator** the helper's TODO names is on `§E.3 pp 32-33`.

**Suggested fix.** Replace the §-citation block with:

```rust
/// §-authority (verified 2026-05-18 against
/// `crates/capco/docs/CAPCO-2016.md`):
/// - §E.1 p31 (exemption-category catalog: 25X#/50X#/75X#).
/// - §E.3 pp 32-33 (Multiple Sources hierarchy — the "longest period
///   of protection" rule the Phase 3 TODO targets).
```

Drop the misleading §H.6 p104 cross-reference; if the helper truly needs to cross-reference AEA, cite §H.6 p104's "Declassify On" prohibition explicitly (`"Automatic declassification of documents containing RD information is prohibited."`).

**Confidence:** high. Verified line-by-line against the manual.

---

### L-3 — `NonIcDissemSet`: SBU-NF classified-context split keeps SBU in non-IC dissem

**File:** `crates/capco/src/lattice.rs` symbol `NonIcDissemSet::from_attrs_iter` (lines 3349-3353)

**Description.** The helper applies the SBU-NF → SBU + needs_nf split on classified pages:

```rust
if classified {
    // §H.9 p178: SBU-NF on classified pages → SBU + NF (dissem).
    if set.remove(&NonIcDissem::SbuNf) {
        set.insert(NonIcDissem::Sbu);
        needs_nf = true;
    }
```

CAPCO-2016 §H.9 p178 (verified verbatim against `crates/capco/docs/CAPCO-2016.md`) says SBU-NF is "Applicable only to unclassified information" and the commingling rule for classified portions is:

> If the portion is classified, the classification level of the portion adequately protects the SBU information, so SBU is **not** reflected in the portion mark; however a NOFORN marking must be added to the portion mark, e.g., (C//NF).

The helper retains `NonIcDissem::Sbu` in the result set when SBU-NF appears on a classified page; CAPCO §H.9 p178 says **neither SBU nor SBU-NF should appear on a classified portion** — only the bare NOFORN remains.

**However**, the helper is a **byte-for-byte port** of the pre-existing PageContext logic (verified via `git log -p` on `crates/ism/src/page_context.rs` — the deleted `expected_non_ic_dissem` body has identical SBU-NF→SBU+NF split semantics). This is **intentional for the PR 4b-E parity-gate convergence** — the byte-identity assertion against the now-retired PageContext path requires the helper to preserve the legacy semantic.

**Contrast with LES-NF**: §H.9 p185 ("When a classified document contains portions of U//LES-NF, the **'LES' marking is used in the banner line** and the NOFORN marking is applied as a Dissemination Control Marking. For example: SECRET//NOFORN//LES.") — LES → LES + NF on classified is **correct per CAPCO**. The asymmetry between SBU-NF and LES-NF is a real CAPCO grammar nuance the legacy PageContext code conflated.

**Authority.** §H.9 p178 (SBU-NF) vs §H.9 p185 (LES-NF) — different prescriptions for the classified-context split.

**Suggested fix.** Two paths:
- **(a)** Preserve the byte-identity port (the implementer's current choice) and **document the divergence** in the helper's doc-comment with a `// PR 4b-E note: pre-existing semantic carried from PageContext for parity-gate convergence; §H.9 p178 says SBU should NOT be reflected on classified, only NF — track for follow-up.` This is the path-of-least-resistance.
- **(b)** Fix the bug in this PR: `if set.remove(&NonIcDissem::SbuNf) { needs_nf = true; }` (drop the `set.insert(NonIcDissem::Sbu)`). This is correct per §H.9 p178 but breaks parity-gate byte-identity against any PageContext fixtures touching SBU-NF + classified. Each broken fixture becomes a `expected_divergences = &["non_ic_dissem"]` annotation citing §H.9 p178.

**(b) is correct CAPCO-wise but out-of-scope for the deletion-PR-policy**; **(a) is policy-aligned**. Recommend (a) with the comment; file a follow-up issue for the SBU-NF fix.

**Confidence:** medium (the policy distinction — "preserve legacy byte-identity now, fix later" — is implicit in the architect plan, not explicit; PM should confirm).

---

### L-4 — `DisplayOnlyBlock` banner-REL-TO subtraction collapses Row 26 to `Empty`

**File:** `crates/capco/src/lattice.rs` symbol `DisplayOnlyBlock::from_attrs_iter` (lines 3646-3656)

**Description.** Steps 6 + 7 of the body unconditionally subtract banner REL TO countries and USA from the DO intersection:

```rust
// (6) Subtract banner REL TO countries — §D.2 Table 3 row 27.
// (7) Subtract USA — implicit originator per §H.8 p163 worked examples.
let rel_to_codes = rel_to_block.to_vec();
let rel_set: BTreeSet<&str> = rel_to_codes.iter().map(|c| c.as_str()).collect();
result.remove("USA");
let result: BTreeSet<&str> = result.difference(&rel_set).copied().collect();
```

Under §D.2 Table 3 **Row 26** (verified against the manual at lines 612-613):

> DISPLAY ONLY [LIST] | REL TO [USA, LIST] (with at least one common [LIST] value(s)) | **DISPLAY ONLY [LIST]** (common trigraph/tetragraphs only in banner line [LIST])

Worked case: portion A = `REL TO [USA, CAN]`, portion B = `DISPLAY ONLY [CAN, GBR]`. Expected banner per Row 26: `DISPLAY ONLY [CAN]` (with REL TO axis NOT in the banner — Row 26's outcome is DO-only, **REL TO is subsumed**).

Helper output: per-portion expanded sets are `expanded[A] = {USA, CAN}` and `expanded[B] = {CAN, GBR}`; intersection = `{CAN}`. Banner REL TO via `RelToBlock::from_attrs_iter` = `[USA, CAN]` (portion A alone, since portion B has no REL TO axis). Step (6) subtracts `{USA, CAN}` from `{CAN}` → `{}` → `Empty`.

The helper **does not** drop the banner REL TO axis when DO subsumes it. So the scheme-level composition would output `REL TO [USA, CAN]` + (empty DO) — which is **not** Row 26's prescribed `DISPLAY ONLY [CAN]`.

**Authority.** §D.2 Table 3 Row 26 (verified). This is the "release-implies-disclosure" subsumption: when REL TO and DO share a country list, the banner uses DO (the more restrictive axis) and drops REL TO.

**However**, this Row 26 + Row 27 dual-channel composition is **explicitly tracked as deferred** at `crates/capco/CAPCO-CONTEXT.md` lines 308-311:

> Remaining FD&R rows the lattice path does NOT yet fully model:
> - Rule 26: cross-axis "REL TO + DISPLAY ONLY → DISPLAY ONLY when release-implies-disclosure".
> - Rule 27: dual-channel REL TO/DISPLAY ONLY composition where each channel has its own common-LIST.

So the helper's incomplete coverage is **policy-aligned**. But the helper actively does the **wrong thing** in the Row 26 case — it returns `Empty` (claiming DO has no countries), when the right behavior is to return `Lattice{CAN}` AND have the scheme layer additionally clear the REL TO axis.

**Suggested fix.** Two paths:
- **(a)** Remove the banner-REL-TO subtraction entirely. Output `Lattice{CAN}` for the Row 26 case. The scheme layer (PageRewrite) becomes responsible for resolving the Row 26 / Row 27 cross-axis composition (clear REL TO when subsumed, or keep both for Row 27).
- **(b)** Keep current behavior but document the Row-26 limitation in the helper's doc-comment, explaining that the helper currently treats Row 26 as `Empty` and that the scheme layer must paper over via a PageRewrite when Row 26 is wired.

**(a) is structurally cleaner** because it locates the cross-axis composition at the scheme layer, where the marque-lattice-consultant skill (`security-lattice.md §6` framing, `marque-applied.md §3.10`) says it belongs. **(b) is acceptable as a parity-gate-preserving choice** if the PageContext output matches this `Empty` behavior on Row 26 fixtures.

Note: per `crates/capco/tests/lattice_vs_scheme_parity.rs` line 4800-4834 (`display_only_block_cross_axis_with_rel_to`), the test for this case explicitly allows EITHER `Lattice` or `Empty` and does not pin the row-26-correct outcome. The test is a **placeholder**, not a regression gate.

**Confidence:** high (the §-citation is real; the deferral is documented; the helper's "row 27 subtraction" doc-comment claim is technically misleading because the subtraction is unconditional, not Row-27-gated).

---

### L-5 — `DisplayOnlyBlock` missing dedicated commutativity + idempotence tests

**File:** `crates/capco/src/lattice.rs` tests at lines 4865-4933

**Description.** The lattice-law tests for `DisplayOnlyBlock::join` cover:
- `display_only_block_join_associative` (line 4868) — tests three `Lattice` operands.
- `display_only_block_join_identity_with_bottom` (line 4891) — `lat.join(bot) == lat`.
- `display_only_block_join_empty_absorbs` (line 4904) — `Empty.join(lat) == Empty`.
- `display_only_block_join_noforn_supersedes_all` (line 4918) — `NofornSuperseded.join(lat) == NofornSuperseded`.

**Missing:**
- `display_only_block_join_commutative` — explicit `a.join(b) == b.join(a)` for at least one `(Lattice, Lattice)` pair (intersection is commutative so this should pass, but the test isn't there).
- `display_only_block_join_idempotent` — `a.join(a) == a` for each variant.
- A proptest exercising random `DisplayOnlyBlock` instances under commutativity / associativity / idempotence (the closest existing proptest gate in the repo is `category_lattice_laws.rs`).

Walked manually: the join table at lines 3713-3725 is correct under commutativity (each match arm is symmetric in operand position) and idempotence (`Lattice{a} ∩ Lattice{a} = Lattice{a}`; sentinel cases trivial). So the impl is correct; the test gap is a coverage issue, not a correctness issue.

**Suggested fix.** Add three small tests (≤30 lines combined):

```rust
#[test]
fn display_only_block_join_commutative() {
    let a = DisplayOnlyBlock::Lattice { countries: ... {GBR, CAN} ... };
    let b = DisplayOnlyBlock::Lattice { countries: ... {GBR, AUS} ... };
    assert_eq!(a.join(&b), b.join(&a));
}

#[test]
fn display_only_block_join_idempotent() {
    let a = DisplayOnlyBlock::Lattice { countries: ... };
    assert_eq!(a.join(&a), a);
    // ... and for each sentinel variant
}
```

Better: extend `crates/capco/tests/category_lattice_laws.rs` with a `DisplayOnlyBlock` proptest section.

**Confidence:** high. The coverage gap is real; the laws hold by walked-impl.

---

### L-6 — `DeclassExemptionLattice` missing associativity test

**File:** `crates/capco/src/lattice.rs` tests at lines 4641-4695

**Description.** The tests cover:
- `declass_exemption_lattice_default_is_bottom` (line 4641)
- `declass_exemption_lattice_empty_equals_default` (line 4647)
- `declass_exemption_lattice_idempotent_on_join` (line 4652)
- `declass_exemption_lattice_identity_with_bottom` (line 4660)
- `declass_exemption_lattice_last_observed_wins` (line 4670)
- `declass_exemption_lattice_from_attrs_iter_picks_last_observed` (line 4681)
- `declass_exemption_lattice_from_attrs_iter_empty_is_bottom` (line 4691)

**Missing:** associativity test.

If L-1 is resolved by removing the `JoinSemilattice` impl, this finding goes away — the test wouldn't apply. If L-1 is resolved by redefining join to be commutative, an associativity test becomes essential.

**Suggested fix.** Tie to L-1 resolution. If keeping the trait impl, add:

```rust
#[test]
fn declass_exemption_lattice_join_associative() {
    let a = DeclassExemptionLattice(Some(ex_a));
    let b = DeclassExemptionLattice(Some(ex_b));
    let c = DeclassExemptionLattice(Some(ex_c));
    assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
}
```

(With current non-commutative impl this gives `Some(ex_c)` on both sides — associativity holds trivially for "right-wins" but commutativity does not.)

**Confidence:** high.

---

### L-7 — `sci_controls_from_markings` name vs signature mismatch

**File:** `crates/capco/src/lattice.rs` line 3160

**Description.** Signature is:

```rust
pub fn sci_controls_from_markings(portions: &[CanonicalAttrs]) -> Box<[marque_ism::SciControl]>
```

The architect plan §3 Decision 2 row 22 specified:

> `sci_controls_from_markings(&[SciMarking]) -> Box<[SciControl]>` (free helper)

The implemented signature took `&[CanonicalAttrs]` instead of `&[SciMarking]`. The function name `from_markings` now refers to neither the `SciMarking` type nor the broader notion — it takes `CanonicalAttrs` portions and projects their `sci_controls` field.

The doc-comment at lines 3148-3154 actually **explains why** the helper takes `CanonicalAttrs` (not `SciMarking[]`): because the flat CVE projection lives in the parsed per-portion `sci_controls` field, not in the structural `SciMarking` form (which sets `canonical_enum: None` per `SciSet::to_markings`'s contract).

But that explanation makes the function name actively misleading — a future reader sees `from_markings` and reaches for `&[SciMarking]`.

**Suggested fix.** Rename to `sci_controls_from_attrs(portions: &[CanonicalAttrs])` or `sci_controls_from_portions(...)`. The doc-comment can keep its current explanation of why `&[SciMarking]` was rejected. Update callsites in `crates/capco/src/scheme/marking.rs` (one production callsite at line ~462 per the architect plan §2 inventory).

**Confidence:** high. Mechanical naming fix.

---

### L-8 — `NonIcDissemSet` `Default` ≠ `BoundedJoinSemilattice::bottom()` (intentional but worth INFO)

**File:** `crates/capco/src/lattice.rs` symbol `NonIcDissemSet` (lines 3312-3397)

**Description.** The type derives `Default` and the doc-comment at lines 3294-3295 says: `**Default** is the bottom: empty set, needs_nf = false.` But the type does NOT implement `JoinSemilattice` or `BoundedJoinSemilattice`. The "bottom" identifier is informal — Rust's type system carries no binding contract that `Default::default()` is the lattice bottom.

This is **intentional** per the doc-comment at lines 3296-3302:

> **Lattice scope.** This type currently exposes only `from_attrs_iter` + read accessors — `JoinSemilattice` is not implemented because the SBU-NF / LES-NF split is gated on the page-level `is_classified` predicate, which depends on the OUTER classification axis being known.

That reasoning is **correct** — the type cannot be a JoinSemilattice because its semantics are cross-axis. The marque-lattice-consultant skill names this pattern as "cross-axis dominance is policy at the scheme layer, not per-axis lattice composition" (per the brief's §3 guidance). The implementer correctly chose NOT to implement the trait, exposing only the `from_attrs_iter` constructor.

**This is the right call.** Compare to `DeclassExemptionLattice` (L-1) which made the wrong call by implementing a broken trait. `NonIcDissemSet`'s honesty about its non-lattice status is the structural template the other types should follow.

**Suggested fix.** None — this is INFO. But the contrast with `DeclassExemptionLattice` reinforces the L-1 fix: `DeclassExemptionLattice` should follow `NonIcDissemSet`'s example and **not implement `JoinSemilattice`** when the join semantics aren't well-defined.

**Confidence:** medium (the policy of "don't implement lattice traits when laws don't hold" is implicit; PM should confirm).

---

### L-9 — `FgiSet::from_attrs_iter` Joint-producer semantic preserves PageContext byte-identity but diverges from marque-applied §4.8.5

**File:** `crates/capco/src/lattice.rs` symbol `FgiSet::from_attrs_iter` Joint branch (lines 3234-3242)

**Description.** The helper unconditionally synthesizes an FGI marker from JOINT producers minus USA:

```rust
Some(MarkingClassification::Joint(j)) => {
    has_any_fgi = true;
    let usa = CountryCode::try_new(b"USA");
    for c in j.countries.iter() {
        if Some(*c) != usa {
            countries.insert(*c);
        }
    }
}
```

This semantic matches the legacy `PageContext::expected_fgi_marker` body (verified via `git log -p crates/ism/src/page_context.rs`). It's byte-identity-preserving and OQ-7 convergence-preserving.

But per `.claude/skills/marque-lattice-consultant/references/marque-applied.md` §4.8.5 (lines 1539-1556), the **proposed** FGI-attribution lattice would handle a pure-JOINT page differently:

> Worked example: C//NF + //GBR-TS → TOP SECRET//FGI GBR//NOFORN
> ...
> | FGI-attribution | ⊥ | bare(FGI-bare, {GBR}) | bare(FGI-bare, {GBR}) | US-presence rewrite: ⊤({GBR}) | ⊤({GBR}) |

For a pure-JOINT page (e.g., `JOINT S USA GBR` × 2 portions), the marque-applied §4.8.5 shape gives `bare(JOINT, {USA, GBR}, S)` (a JOINT atom, NOT an FGI rollup). The current `FgiSet::from_attrs_iter` synthesizes `Present { concealed: false, countries: {GBR} }` — i.e., it treats every JOINT portion as if it contributes a foreign-equity overlay, even when the page is purely JOINT.

This is a **known design gap** between the current `FgiSet` shape and the proposed `FgiAttributionLattice` shape (Q-4.8a in marque-applied.md). **Not a finding for this PR** — the helper correctly preserves the legacy PageContext semantic, and the FGI-attribution lattice redesign is its own future PR per §4.8.6.

**Suggested fix.** None for this PR. Add a `// PR 4b-E note:` comment in the helper's doc-comment referencing the marque-applied §4.8 future design so a reader doesn't mistake the current semantic for the intended end-state:

```rust
/// Note: this constructor preserves the legacy `PageContext::expected_fgi_marker`
/// semantic for parity-gate convergence. The future
/// `FgiAttributionLattice` design (marque-applied.md §4.8) refines
/// the JOINT-producer extraction to a `bare(JOINT, ...)` atom on
/// pure-JOINT pages; that is deferred to a post-PR-4b-E redesign.
```

**Confidence:** high (the legacy semantic is verified; the future-shape divergence is documented at marque-applied.md §4.8).

---

## Convergence-divergence policy check (OQ-7 / Decision §5)

### Convergence-side fixtures (must be byte-identical)

The three pre-PR-4b-E divergences (per `crates/capco/CAPCO-CONTEXT.md` lines 262-271):
- `pure_nato_lattice_vs_pagecontext_diverges` (G-3) — file `lattice_vs_scheme_parity.rs:814`
- `joint_unanimous_two_portions` — `lattice_vs_scheme_parity.rs:475`
- `joint_single_portion_no_us` — `lattice_vs_scheme_parity.rs:618`

Walked the fixture bodies. **All three correctly assert convergence** post-PR-4b-E (the PageContext side is retired; both surviving sides are lattice-derived, asserted via `matches!` on the classification variant). The fixture **names are now misleading** (e.g., `pure_nato_lattice_vs_pagecontext_diverges` no longer compares against PageContext) — the doc-comments at lines 480-484 compensate, but a reader scanning function names alone will be confused.

**Recommend:** rename the three fixtures in a separate hygiene pass after PR 4b-E lands. Not a blocking finding.

### Divergence-side fixtures (the new `expected_divergences = &["dissem_us"]` class)

Per `crates/capco/CAPCO-CONTEXT.md` lines 273-284, the PR introduces a uniform §B.3 Table 2 p21 annotation for 12 fixtures where the scheme path runs `CLOSURE_NOFORN_CAVEATED` (the caveated-classified post-28-Jun-2010 NOFORN rule) and the per-axis lattice path doesn't.

Spot-checked **two** of the 12 fixtures:
- `oc_usgov_one_orcon_many_usgov` (line 175) — annotated with `&["dissem_us"]` + §B.3 Table 2 p21 inline comment (lines 193-200). **Citation verified** against `crates/capco/docs/CAPCO-2016.md` at the manual's lines 584+ (Table 2 starts on p21). **Semantic correct** — ORCON-classified is caveated per §B.3 p20 Note ("ORCON / ORCON-USGOV ... is a caveat"), so the §B.3 Table 2 p21 row "Classified + caveated + on/after 28 June 2010 → Mark as NOFORN" fires on the scheme path.
- `joint_mixed_with_us_returns_mixed` (line 504) — same `&["dissem_us"]` annotation with same §B.3 Table 2 p21 citation. The JOINT-portion is caveated per the §B.3 p20 Note ("FGI without FD&R" qualifies; here it's JOINT, which is a related caveat class). **Citation verified.**

The other 10 fixtures use a uniform comment block; spot-checking two is sufficient to confirm the annotation pattern is structurally correct.

**Recommend:** PM confirms with implementer that all 12 fixtures use the same §B.3 Table 2 p21 citation and that all 12 represent caveated-classified scenarios. The annotation block at fixture-level is clearer than a single shared comment, but should not duplicate the §-citation 12 times if the divergence-class is uniform.

---

## Coverage on the 5 new helpers (test-file summary)

| Helper | Inherent tests | Lattice-law tests | Coverage of CAPCO §-rows touched |
|---|---|---|---|
| `NonIcDissemSet::from_attrs_iter` | 6 (default/empty, SBU-NF split, SBU-NF preserved unclass, NODIS NF, EXDIS NF, empty input) | N/A (not a lattice — by design) | §H.9 p172 / p174 / p178 / p185 — 3 of 4 covered (LES-NF missing direct test; covered indirectly via `pattern_c_les_in_classified_propagates_to_banner` in `lattice_vs_scheme_parity.rs`) |
| `DeclassExemptionLattice::from_attrs_iter` | 4 (default, empty, last-observed-pick, empty-input-bottom) | 2 (idempotent, identity-with-bottom); **missing associativity, missing commutativity** | §E.1 p31 (catalog) — cited but L-2 says wrong section |
| `FgiSet::from_attrs_iter` | 5 (empty, NATO producer, concealed-dominates, JOINT-excludes-USA, associativity-with-existing-join) | 1 (associativity_with_join via constructor + iterated join) | §H.7 p122/p123/p128 — all covered |
| `DisplayOnlyBlock::from_attrs_iter` + `JoinSemilattice` | 10 (default/empty, NF-supersedes, needs-nf-short-circuit, row-19-empty-portion, simple-intersection, disjoint-intersection, cross-axis-with-rel-to, USA-subtraction, trigraph-sort) | 4 (associative, identity, empty-absorbs, NF-supersedes); **missing commutativity, missing idempotence (single-variant)** | §H.8 p163 + §D.2 Table 3 rows 18/19/20/25/26/27 — coverage is asymmetric: Row 25 covered (`simple_intersection`), Row 20 covered (`disjoint_intersection`), Row 19 covered (`row_19_empty_portion`), Rows 26/27 covered via `cross_axis_with_rel_to` but L-4 says incomplete |
| `sci_controls_from_markings` | 3 (empty, dedup, union) | N/A (free helper; not a lattice) | §H.4 p61 — covered |

**Total new tests landed in `lattice.rs`:** 38 (matches implementer's report).

Spot-checked the §-citations in each test's body — all reference correct CAPCO sections. **No fabricated citations found in test bodies.**

---

## Conclusion

The 5 new lattice helpers are **structurally sound** with **one trait-contract violation** (L-1, HIGH severity, fixable by removing the `JoinSemilattice` impl on `DeclassExemptionLattice` per `NonIcDissemSet`'s precedent), **one §-citation defect** (L-2, MEDIUM severity, mechanical fix), **one preserved-pre-existing-bug** (L-3, MEDIUM severity, policy-aligned to preserve PageContext byte-identity), and **assorted coverage / naming / Row-26 deferral concerns** (L-4 through L-9, LOW / INFO).

The architect plan §10 OQ-5 (5 new lattice helpers in scope) is correctly executed.

The architect plan §10 OQ-7 (divergence convergence) is correctly executed for the three named fixtures; the new `&["dissem_us"]` divergence-class is documented per CAPCO-CONTEXT and uses a uniform §B.3 Table 2 p21 citation that is **verified** against the source.

**Recommended PR-blocking action:** resolve L-1 before merge. The other findings should be triaged for follow-up issues but do not block PR open.

**Recommended non-blocking actions:** fix L-2 in this PR (one doc-comment edit), document L-3 and L-9 inline, queue L-4 + L-5 + L-6 + L-7 + L-8 + the three convergence-fixture renames as follow-up issues.
