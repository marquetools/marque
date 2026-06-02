<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR-D1 Tactical Rust Plan — `DeclassInstruction` 9-tier declass-precedence lattice (T043, value-type half)

**Branch**: `007-phase-d` (base `654a281b`). **Crate**: `marque-capco` only. **WASM-safe.**
**Scope**: generalize `crates/capco/src/lattice/declassify_on.rs` from a date-only
`Option<IsmDate>` MaxDate semilattice into a full 9-tier `DeclassInstruction` carrier wrapped in
`DeclassifyOnLattice(OrdMax<DeclassInstruction>)` implementing `BoundedJoinSemilattice`. No engine
node, no pivot edits, no `marque-scheme` change. The engine node (multi-edge wiring) lands in PR-D3.

This plan is an executable spec. Read it top to bottom; do not re-derive design. Every tier-ordering
claim below was re-verified against `crates/capco/docs/CAPCO-2016.md` §E.3 p32–33 (lines 660–677) on
2026-06-02 — the verbatim source text is quoted in §1.

---

## 0. Verified ground truth (read before coding)

| Fact | Source | Verified |
|------|--------|----------|
| `OrdMax<T>(pub T)` requires `T: Ord + Clone`; `join = max`, `meet = min`; `#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]`; NOT generically bounded | `crates/scheme/src/builtins/ord.rs:19-42` | yes |
| `BoundedJoinSemilattice: JoinSemilattice` exposes ONLY `fn bottom() -> Self`. There is NO `top()` on it. `top()` lives on `BoundedMeetSemilattice`. | `crates/scheme/src/lattice.rs:112-123` | yes |
| `JoinSemilattice`/`MeetSemilattice` both require `Sized + Clone + Eq` | `crates/scheme/src/lattice.rs:60,71` | yes |
| `IsmDate` derives `PartialEq, Eq, Hash` but **NOT** `Ord`/`PartialOrd` | `crates/ism/src/date.rs:150-151` | yes |
| `IsmDate::end_components(&self) -> (i32,u8,u8,u8,u8,u8,u32,i16)` is **private**; `end_cmp` is the only public comparator and it does `self.end_components().cmp(&other.end_components())` | `crates/ism/src/date.rs:311-315,324` | yes |
| `Year(2003)` and `Date(2003,12,31)` both end-resolve to `(2003,12,31,23,59,59,999_999_999,0)` → distinct variants, identical end-tuple | `crates/ism/src/date.rs:326,331` | yes |
| `DeclassExemption` is generated, `#[non_exhaustive]`, derives `Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash`; variants: `Aea, Nato, NatoAea, X25x1, X25x1Eo12951, X25x2..X25x9, X50x1, X50x1Hum, X50x2, X50x2Wmd, X50x3..X50x9, X75x` | generated `values.rs:313-367` (from `CVEnumISM25X.xml`) | yes |
| `CanonicalAttrs.declassify_on: Option<IsmDate>`; `CanonicalAttrs.declass_exemption: Option<DeclassExemption>` | `crates/ism/src/canonical.rs:122,131` | yes |
| Static-assertion `lattice_static_assertions.rs:113` currently pins `DeclassifyOnLattice: NOT BoundedJoinSemilattice` — **this line must move** (see §6) | `crates/capco/tests/lattice_static_assertions.rs:113` | yes |

> **Naming note**: the data-model says "seed from `ProjectedMarking.declassify_on`". In code the
> rollup actually reads `&[CanonicalAttrs]` (portions), and `CanonicalAttrs.declassify_on` is the
> field. `ProjectedMarking` is the *output* of `project`. The existing `from_attrs_iter(&[CanonicalAttrs])`
> contract is what seeds the date tiers; keep that signature.

---

## 1. Authoritative §E.3 hierarchy (verbatim transcription)

CAPCO-2016 §E.3 p32 "Multiple Sources and the Declassify On Line Hierarchy" (`CAPCO-2016.md`
lines 660–677). The lead sentence (line 662):

> "The 'Declassify On' line must reflect the single declassification value that provides the longest
> classification duration of any of the sources. When determining the single most restrictive
> declassification instruction among multiple source documents, adhere to the following hierarchy …"

The hierarchy, **most-restrictive / highest-precedence first** (this is the bullet order on p32–33):

1. **(line 663)** `"N/A to [RD/FRD/TFNI, as appropriate] [and NATO, if appropriate] portions. See
   source list for NSI portions."` — Note: "these values do not have a date or event associated with
   them. **Any one or combination of these declassification instructions takes precedence over all
   other declassification instructions.**" → the absolute top; dateless.
2. **(line 664)** `"50X1 - HUM" or "50X2 - WMD" or an ISOO-approved designator reflecting the ISCAP
   approval for classification beyond 50 years.` "If the source documents have both 50X1-HUM and
   50X2-WMD exemptions, apply 50X1-HUM as the exemption with the **lowest number**." Dateless. The
   "ISOO-approved >50yr designator" is the generated `X75x` (75-year ISCAP) value.
3. **(line 665)** `50X1 – 50X9, with a date or event.` "apply the exemption with the date or event
   that provides the **longest period of protection**. If all … have the same date or event, apply
   the … exemption with the **lowest number**." (Carries an `IsmDate`.)
4. **(line 672)** `"25X1, EO 12951"` — reserved exclusively for D/NGA original imagery. Dateless
   singleton. (Generated value `X25x1Eo12951`.)
5. **(line 673)** `25X1 through 25X9, with a date or event.` longest-protection date, then lowest
   number on tie. (Carries an `IsmDate`.)
6. **(line 674)** `25X1 through 25X9 without a date or event.` "determine the longest period of
   protection by calculating a 50-year date from the source document date … If all 25X#s with a
   calculated 50-year date have the same date, apply the single exemption with the **lowest number**
   and the calculated 50-year date." (No authored date in this PR — see §7 Open Question OQ-2.)
7. **(line 675)** `A specific declassification date no more than 25 years in the future.` (Carries an
   `IsmDate`, no exemption code.)
8. **(line 676)** `An event less than 10 years in the future.` (CAPCO models the event as free text;
   marque does not capture event strings on the canonical pivot today — see §7 OQ-3. This PR
   represents the *tier* as a dateless point.)
9. **(line 677)** `Absent guidance … a calculated 25-year date from the date of the source
   information.` (Carries an `IsmDate`.)

§E.4 p33 (line 683) and §E.5 p33 (line 687): the commingling N/A string **replaces** any date/event.
The choice *among* the AEA-only / NATO-only / combined N/A wordings is a **render** decision (Phase G
/ T070), keyed on document AEA-present / NATO-present flags — **not** a lattice distinction. Tier 1 is
ONE lattice point.

---

## 2. Exact `DeclassInstruction` enum shape

New file: `crates/capco/src/lattice/declass_instruction.rs`.

```rust
use marque_ism::{DeclassExemption, IsmDate};
use core::cmp::Ordering;

/// One §E.3 declassification instruction — the single value the "Declassify On"
/// line may carry (CAPCO-2016 §E.3 p32: "Only a single value must be used").
///
/// Variants are ordered by §E.3 precedence (tier 1 = highest = `Commingled`).
/// The hand-written total `Ord` (see below) is the load-bearing law; the
/// declaration order here is documentation, NOT the comparison key.
///
/// `Eq`/`PartialEq` are derived from `Ord` (`a == b  ⟺  a.cmp(b) == Equal`),
/// NOT structural. Two instructions that resolve to the same precedence key
/// compare equal even if their data differs. This is **precedence-equivalence,
/// not structural identity** — required so `OrdMax::join` (which keys on `>=`)
/// is consistent with equality and the `JoinSemilattice: Eq` contract holds.
#[derive(Debug, Clone)]
pub enum DeclassInstruction {
    /// Tier 9 — calculated 25-year fallback (§E.3 p33 line 677). Dateless tier
    /// marker; the resolved date rides in `date` when known.
    Calculated25Year { date: Option<IsmDate> },          // lowest precedence above Unset

    /// Tier 8 — event less than 10 years in the future (§E.3 p33 line 676).
    /// marque does not capture the event string on the pivot; dateless point.
    EventUnder10Year,

    /// Tier 7 — a specific declassification date ≤25 years (§E.3 p33 line 675).
    SpecificDate { date: IsmDate },

    /// Tier 6 — 25X1–25X9 without a date or event (§E.3 p33 line 674).
    /// `code` is the 25X# exemption (lowest-number tiebreak); `date` is the
    /// computed 50-yr-from-source date when available (None this PR — OQ-2).
    Exempt25xUndated { code: DeclassExemption, date: Option<IsmDate> },

    /// Tier 5 — 25X1–25X9 with a date or event (§E.3 p33 line 673).
    Exempt25xDated { code: DeclassExemption, date: IsmDate },

    /// Tier 4 — "25X1, EO 12951", D/NGA imagery only (§E.3 p33 line 672).
    /// Dateless singleton. (`DeclassExemption::X25x1Eo12951`.)
    Eo12951,

    /// Tier 3 — 50X1–50X9 with a date or event (§E.3 p32 line 665).
    Exempt50xDated { code: DeclassExemption, date: IsmDate },

    /// Tier 2 — 50X1-HUM / 50X2-WMD / ISOO >50yr designator (75X) (§E.3 p32
    /// line 664). Dateless. `code` retained for lowest-number tiebreak +
    /// render; the join keys on (tier, code-number).
    Exempt50xBeyond { code: DeclassExemption },

    /// Tier 1 — the commingling N/A string (§E.3 p32 line 663 / §E.4 / §E.5).
    /// "Takes precedence over all other declassification instructions." Dateless
    /// by construction: a dated `Commingled` is unconstructible (no field).
    /// Which exact N/A wording renders (AEA-only / NATO-only / combined) is a
    /// RENDER concern (Phase G / T070), not a lattice distinction.
    Commingled,
}
```

**Tier → variant map** (re-verify against §1 before edit):

| §E.3 tier | Precedence (1=top) | Variant | Carries date? | Carries exemption code? |
|-----------|-------------------|---------|---------------|------------------------|
| 1 N/A commingling | 1 | `Commingled` | no (unconstructible) | no |
| 2 50X-HUM/WMD/>50yr | 2 | `Exempt50xBeyond { code }` | no | yes (`X50x1Hum`/`X50x2Wmd`/`X75x`) |
| 3 50X#, dated | 3 | `Exempt50xDated { code, date }` | yes | yes (`X50x1..X50x9`) |
| 4 25X1 EO 12951 | 4 | `Eo12951` | no | implicitly `X25x1Eo12951` |
| 5 25X#, dated | 5 | `Exempt25xDated { code, date }` | yes | yes (`X25x1..X25x9`) |
| 6 25X#, undated | 6 | `Exempt25xUndated { code, date: None }` | optional (OQ-2) | yes |
| 7 specific date ≤25yr | 7 | `SpecificDate { date }` | yes | no |
| 8 event <10yr | 8 | `EventUnder10Year` | no (OQ-3) | no |
| 9 calc 25yr fallback | 9 | `Calculated25Year { date }` | optional | no |
| (absent) bottom | — | (modeled at the newtype as `OrdMax<Option<DeclassInstruction>>`-equivalent — see §3) | — | — |

> **Do NOT** add a `Unset` variant to `DeclassInstruction`. Bottom (absence) is represented at the
> newtype layer (§3), keeping `DeclassInstruction` itself a clean closed chain whose every value is a
> real §E.3 instruction. This matches the consultant verdict "make a dated `Commingled`
> unconstructible" and avoids a junk variant the `Ord` would have to special-case below everything.

---

## 3. `Ord` impl strategy (the load-bearing law)

### 3.1 Comparison key

Define a private `precedence_key(&self) -> (u8, DateKey, u16)` and route `Ord` through it. **Pick the
hand-written `PartialOrd`/`Ord` scheme; do NOT `#[derive(Ord)]` alongside a hand-written
`PartialOrd`** (that trips `clippy::derive_ord_xor_partial_ord` on stable — see §"clippy" below).

```rust
// Higher tuple == higher §E.3 precedence == "more restrictive / longer protection".
// OrdMax::join picks the max, so the most-restrictive instruction wins, matching
// §E.3 "single declassification value that provides the longest classification duration".
//
// Key fields, compared lexicographically:
//   .0  tier rank: 1 (lowest, Calculated25Year) .. 9 (highest, Commingled).
//       NOTE: this is the INVERSE of the §E.3 bullet numbering — §E.3 lists most-
//       restrictive first, but `max` needs most-restrictive = largest. Document loudly.
//   .1  date sub-key: end_components() of the resolved date, or the dateless sentinel.
//   .2  exemption tiebreak: LOWER exemption number wins on a tier+date tie, so store the
//       NEGATED number (u16::MAX - n) to keep "max picks lowest-number".
type DateKey = (i32, u8, u8, u8, u8, u8, u32, i16);

const DATELESS: DateKey = (i32::MIN, 0, 0, 0, 0, 0, 0, 0);
```

Tier ranks (largest = highest precedence, the inverse of §E.3 bullet order):

| Variant | tier rank `.0` |
|---------|----------------|
| `Commingled` | `9` |
| `Exempt50xBeyond` | `8` |
| `Exempt50xDated` | `7` |
| `Eo12951` | `6` |
| `Exempt25xDated` | `5` |
| `Exempt25xUndated` | `4` |
| `SpecificDate` | `3` |
| `EventUnder10Year` | `2` |
| `Calculated25Year` | `1` |

### 3.2 Date sub-key uses `end_components()` indirectly

`end_components()` is **private** on `IsmDate`. The consultant's instruction "compare the
`end_components()` tuple directly (do NOT call `end_cmp`)" cannot be done verbatim because the tuple
is not exposed. Two lawful options — **choose A**:

- **Option A (recommended, no `marque-ism` change)**: build the `DateKey` from the *public*
  accessors plus the documented end-of-span convention, reproducing `end_components()` for the
  `Year`/`YearMonth`/`Date` precisions this axis actually sees, OR — simpler and exactly faithful —
  key the date sub-component by calling `IsmDate::end_cmp` **inside** a manual comparator rather than
  materializing a tuple. Concretely: do the date comparison with `a.end_cmp(b)` in the `Ord` body
  (it returns a genuine total `Ordering` over the private tuple), and only fall through to the
  exemption tiebreak when `end_cmp` returns `Equal`. This sidesteps the private-tuple problem and is
  byte-faithful to the existing `DeclassifyOnLattice` join semantics. **This is the recommended
  path** — it needs zero `marque-ism` edits.

  > Why this is consistent: the consultant's worry was that `end_cmp` collapses `Year(2003)` and
  > `Date(2003,12,31)` to `Equal`. That is *correct and intended* here — for declass precedence two
  > dates with the same end-of-span instant ARE equivalent (the rule keys on "longest protection",
  > i.e. end-of-span). So `end_cmp == Equal` → defer to the exemption-number tiebreak. We are NOT
  > deriving `Eq` from a structural date compare; we derive it from precedence (§3.3).

- **Option B (only if a tuple key is required for some downstream sort)**: add
  `pub fn end_components(&self) -> (i32,u8,u8,u8,u8,u8,u32,i16)` to `IsmDate` (flip the existing
  private `fn` to `pub`). This is a `marque-ism` edit and is **out of PR-D1 scope** per the "no pivot
  edits" constraint. Do not take Option B in this PR.

**Decision: Option A.** The `Ord` body is a `match`-on-pair that (1) compares tier rank, (2) on equal
tier compares dates via `end_cmp` (treating dateless variants as "earliest"), (3) on equal date
compares the negated exemption number.

### 3.3 `Eq`/`PartialEq` from `cmp`

```rust
impl PartialEq for DeclassInstruction {
    fn eq(&self, other: &Self) -> bool { self.cmp(other) == Ordering::Equal }
}
impl Eq for DeclassInstruction {}
impl PartialOrd for DeclassInstruction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl Ord for DeclassInstruction {
    fn cmp(&self, other: &Self) -> Ordering { /* tier, then end_cmp, then neg-exemption */ }
}
```

- **Do NOT `#[derive(PartialEq, Eq)]`** on the enum — they would be structural and disagree with
  `cmp` (e.g. `Year(2003)` vs `Date(2003,12,31)` at the same tier are precedence-equal but
  structurally distinct). A structural/`cmp` disagreement violates the `Ord`/`Eq` consistency
  contract and breaks `OrdMax`'s `JoinSemilattice: Eq` law.
- Document loudly in the doc-comment: **"Eq is precedence-equivalence, not structural identity."**

### 3.4 Exemption-number source

Add a private helper that maps a `DeclassExemption` to its competing integer for the lowest-number
tiebreak. `DeclassExemption` is `#[non_exhaustive]`, so the `match` MUST have a catch-all arm
(non-exempt variants and any future-added variant map to a neutral high number so they never win a
"lowest" tie spuriously):

```rust
// Lower number = higher precedence on a same-tier same-date tie (§E.3 lines 664/665/673/674).
fn exemption_rank(code: DeclassExemption) -> u16 {
    match code {
        DeclassExemption::X50x1Hum | DeclassExemption::X50x1 | DeclassExemption::X25x1
            | DeclassExemption::X25x1Eo12951 => 1,
        DeclassExemption::X50x2Wmd | DeclassExemption::X50x2 | DeclassExemption::X25x2 => 2,
        DeclassExemption::X50x3 | DeclassExemption::X25x3 => 3,
        // … 4..9 …
        DeclassExemption::X75x => 75,
        _ => u16::MAX, // #[non_exhaustive] catch-all + Aea/Nato/NatoAea (not numbered exemptions)
    }
}
```
The `Ord` stores `u16::MAX - exemption_rank(code)` in key field `.2` so `max` picks the **lowest**
number. Dateless/code-less variants (`Commingled`, `Eo12951`, `EventUnder10Year`) use `.2 = 0`
(irrelevant — their tier already disambiguates).

`DeclassExemptionAccumulator` (`crates/capco/src/lattice/declass_exemption.rs`) stays as-is in PR-D1
(it is a separate last-observed projection helper, not a lattice). Its long-standing
"needs a duration-aware comparator" TODO is **subsumed** by `DeclassInstruction` but the wiring that
retires it is PR-D3 engine work — see §7 OQ-4. Do not delete it in PR-D1.

---

## 4. Newtype + trait impls

Rewrite `crates/capco/src/lattice/declassify_on.rs`:

```rust
use marque_ism::{CanonicalAttrs, DeclassExemption, IsmDate};
use marque_scheme::{BoundedJoinSemilattice, JoinSemilattice, MeetSemilattice, OrdMax};

mod declass_instruction; // or keep DeclassInstruction in its own sibling file + `pub use`
pub use declass_instruction::DeclassInstruction;

/// Lattice form of the declassification axis (§E.3 p32). Carries the single
/// §E.3 instruction with `OrdMax` join = "most restrictive / longest protection wins".
///
/// `None` is the lattice bottom (absence). The inner `OrdMax<DeclassInstruction>`
/// is only constructed when an instruction is present; bottom is the `None` arm,
/// handled in the join/meet bodies (mirrors the old `Option<IsmDate>` shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclassifyOnLattice(Option<OrdMax<DeclassInstruction>>);
```

> **Why `Option<OrdMax<…>>` and not `OrdMax<DeclassInstruction>` directly**: `OrdMax<T>` has no
> bottom for an open `T`, and `DeclassInstruction` deliberately has no `Unset` variant (§2). Absence
> is a real lattice state (a portion with no declass instruction). Keep the `Option` wrapper exactly
> as the current type does, so `bottom() == None` and the existing `empty()`/`into_inner()`-shaped
> API can migrate with minimal caller churn.

### 4.1 `JoinSemilattice` / `MeetSemilattice` (delegate to inner `OrdMax`)

```rust
impl JoinSemilattice for DeclassifyOnLattice {
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, x) | (x, None) => Self(x.clone()),
            (Some(a), Some(b)) => Self(Some(a.join(b))), // OrdMax::join = max precedence
        }
    }
}
impl MeetSemilattice for DeclassifyOnLattice {
    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => Self(Some(a.meet(b))), // OrdMax::meet = min precedence
        }
    }
}
```

### 4.2 `BoundedJoinSemilattice` (NEW — the T043 requirement)

`BoundedJoinSemilattice` exposes only `bottom()`. The conceptual "top = `Commingled`" is **not** a
method on this trait (it would be `BoundedMeetSemilattice::top`, which we do NOT implement because the
*absence* bottom + a meet-top is not the model we want, and the assertion block keeps these unbounded
on the meet side). Implement only the join half:

```rust
impl BoundedJoinSemilattice for DeclassifyOnLattice {
    fn bottom() -> Self { Self(None) } // absence is the join identity
}
```

> The data-model line "top = the single `Commingled` tier-1 point ⇒ implements
> `BoundedJoinSemilattice`" is slightly imprecise: a *join* bottom is what `BoundedJoinSemilattice`
> requires (`bottom() = None`). The `Commingled` "takes precedence over all" property makes it the
> top *element of the chain* and is enforced by the `Ord` (tier rank 9), not by a trait method. We do
> NOT add `BoundedMeetSemilattice` — see §6 static-assertion change (it moves to the
> `BoundedJoinSemilattice`-only positive lane, staying out of the `BoundedMeetSemilattice` lane).

### 4.3 Existing API surface — preserve / migrate

Current `DeclassifyOnLattice` public methods (all in `declassify_on.rs`) and their fate:

| Method | Current | PR-D1 |
|--------|---------|-------|
| `empty() -> Self` | `Self(None)` | **keep** — `Self(None)`; also = `bottom()` |
| `new(d: Option<IsmDate>) -> Self` | wrap date | **migrate signature**: change to `new(i: Option<DeclassInstruction>) -> Self`. Add `from_date(d: Option<IsmDate>) -> Self` convenience that lifts a bare date into `SpecificDate`/`Calculated25Year` (decide which — see OQ-1) so the date-only test helpers (`category_lattice_laws.rs:1198-1206`) can be updated minimally. |
| `from_attrs_iter(&[CanonicalAttrs]) -> Self` | max date via `end_cmp` | **rewrite**: fold each portion's `(declassify_on, declass_exemption)` into a `DeclassInstruction`, join across portions via `OrdMax`. See §5. |
| `into_inner(self) -> Option<IsmDate>` | inner date | **migrate**: change to `into_inner(self) -> Option<DeclassInstruction>`. Add `into_date(self) -> Option<IsmDate>` that projects the resolved date out of the instruction (for the `marking.rs:629` caller which assigns `out.declassify_on: Option<IsmDate>`). |
| `as_inner(&self) -> Option<&IsmDate>` | borrow date | **migrate**: `as_inner(&self) -> Option<&DeclassInstruction>`; keep `as_date(&self) -> Option<&IsmDate>` if any caller needs it. |

**Caller break inventory** (every site touching `DeclassifyOnLattice`; see §6 for line-level edits):

| Caller | Breaks? | Fix |
|--------|---------|-----|
| `crates/capco/src/scheme/marking.rs:629` (`out.declassify_on = …into_inner()`) | **YES** — `out.declassify_on` is `Option<IsmDate>` but `into_inner()` now returns `Option<DeclassInstruction>` | replace with `.into_date()` projection helper. The pivot field stays `Option<IsmDate>` (no pivot edit). |
| `crates/capco/src/lib.rs:73` (re-export) | no (name unchanged) | optionally also `pub use lattice::DeclassInstruction;` |
| `crates/capco/tests/category_lattice_laws.rs:1193-1244` (`mod declassify_on_lattice`) | **YES** — `d()`/`y()`/`bottom()` helpers build from `IsmDate` | update helpers to use `from_date(...)`; add new §E.3 oracle fixtures (§5 of test plan) |
| `crates/capco/tests/lattice_static_assertions.rs:71,113` | **YES** — line 113 asserts NOT bounded | move `DeclassifyOnLattice` from the `assert_not_impl_any` block to a new `BoundedJoinSemilattice`-positive assertion (§6) |
| `crates/capco/tests/post_4b_lattice_inventory_pin.rs` | maybe (inventory pin) | re-run; if it pins exact bounded/unbounded membership, update the pinned entry for `DeclassifyOnLattice` |
| `crates/scheme/src/builtins/date.rs:13` (doc-comment mention) | no (comment only) | optionally refresh prose |
| `crates/capco/src/lattice/classification.rs:90` (doc mention) | no | none |
| `crates/wasm/src/banner.rs:175,226` (mentions `DeclassExemptionAccumulator`, not `DeclassifyOnLattice`) | no | none in PR-D1 |

---

## 5. Seeding from existing data (`from_attrs_iter`)

Fold `&[CanonicalAttrs]` → `DeclassInstruction` per portion, then `OrdMax`-join across portions:

```rust
pub fn from_attrs_iter(portions: &[CanonicalAttrs]) -> Self {
    let folded = portions.iter().fold(None::<OrdMax<DeclassInstruction>>, |acc, p| {
        let instr = instruction_from_attrs(p.declassify_on.as_ref(), p.declass_exemption);
        match (acc, instr) {
            (a, None) => a,
            (None, Some(i)) => Some(OrdMax(i)),
            (Some(a), Some(i)) => Some(a.join(&OrdMax(i))),
        }
    });
    Self(folded)
}
```

`instruction_from_attrs(date, code)` classification (§E.3 mapping; verify each arm against §1):

| `code` | `date` | → `DeclassInstruction` |
|--------|--------|------------------------|
| `Some(X50x1Hum \| X50x2Wmd \| X75x)` | any | `Exempt50xBeyond { code }` (tier 2; dateless) |
| `Some(X50x1..X50x9)` (other 50X) | `Some(d)` | `Exempt50xDated { code, date: d }` (tier 3) |
| `Some(X50x1..X50x9)` (other 50X) | `None` | treat as tier 3 with no date? — see OQ-2; default to `Exempt50xDated` only when dated, else fall to tier 2 floor `Exempt50xBeyond`? **Resolve via OQ-2 before coding.** |
| `Some(X25x1Eo12951)` | any | `Eo12951` (tier 4; dateless) |
| `Some(X25x1..X25x9)` | `Some(d)` | `Exempt25xDated { code, date: d }` (tier 5) |
| `Some(X25x1..X25x9)` | `None` | `Exempt25xUndated { code, date: None }` (tier 6) |
| `Some(Aea \| Nato \| NatoAea)` | any | `Commingled` (tier 1) — **OQ-5**: confirm these CVE values are the canned-commingling marker, not a real numbered exemption. §E.3 line 663 is the commingling tier; the generated `Aea`/`Nato`/`NatoAea` exemption codes correspond to the §E.4/§E.5 N/A strings. |
| `None` | `Some(d)` | `SpecificDate { date: d }` (tier 7) — bare date with no exemption |
| `None` | `None` | `None` (absence → bottom) |

> `EventUnder10Year` (tier 8) and `Calculated25Year` (tier 9) are **not produced** by
> `from_attrs_iter` in PR-D1 (the canonical pivot carries no event-string or "calculated-fallback"
> marker). They exist in the enum so the total order is complete and the engine node (PR-D3) can mint
> them from derived edges. This is intentional; note it in the doc-comment and in OQ-3.

---

## 6. File-by-file change list

1. **`crates/capco/src/lattice/declass_instruction.rs`** — NEW file (~180 lines incl. tests).
   `DeclassInstruction` enum (§2), hand `Ord`/`PartialOrd`/`Eq`/`PartialEq` (§3), `exemption_rank`,
   `precedence_key`/`cmp` body, SPDX header. WASM-safe (no I/O, only `marque-ism` + `core::cmp`).

2. **`crates/capco/src/lattice/declassify_on.rs`** (currently 117 lines) — REWRITE:
   - Change struct to `DeclassifyOnLattice(Option<OrdMax<DeclassInstruction>>)`.
   - Add `mod declass_instruction; pub use declass_instruction::DeclassInstruction;` (or declare the
     submodule from `lattice/mod.rs` — match the crate's existing submodule convention; the other
     lattice files are siblings under `lattice/`, so declare `mod declass_instruction;` in
     `lattice/mod.rs` and `pub use`).
   - `JoinSemilattice`/`MeetSemilattice` delegate to inner `OrdMax` (§4.1).
   - `impl BoundedJoinSemilattice { fn bottom() }` (§4.2).
   - Migrate `new`/`into_inner`/`as_inner`; add `from_date`/`into_date`/`as_date` (§4.3).
   - Rewrite `from_attrs_iter` (§5).
   - Update the doc-comment: replace the "BoundedLattice deliberately not implemented" paragraph with
     the new "implements `BoundedJoinSemilattice` (bottom = absence); the `Commingled` top is enforced
     by `Ord` tier rank, not by `BoundedMeetSemilattice`" rationale. Keep the §E.3 p32 authority
     citation (it is now even more central — quote line 662 + line 663).

3. **`crates/capco/src/lattice/mod.rs:81`** — add `pub use declass_instruction::DeclassInstruction;`
   alongside the existing `pub use declassify_on::DeclassifyOnLattice;`, and declare
   `mod declass_instruction;` (line ~near the other `mod` declarations — confirm the module list at
   top of `mod.rs`).

4. **`crates/capco/src/lib.rs:73`** — add `DeclassInstruction` to the `pub use lattice::{…}` list so
   downstream (PR-D3 engine) and tests can name it.

5. **`crates/capco/src/scheme/marking.rs:629`** — change
   `out.declassify_on = DeclassifyOnLattice::from_attrs_iter(portions).into_inner();`
   to `… .into_date();`. The pivot field type (`Option<IsmDate>`) is unchanged → **no pivot edit, no
   `marque-ism` edit**. Refresh the Axis-9 comment (lines 624-628) to note the instruction now
   carries the full §E.3 tier, with the date projected out for the (still date-only) pivot field.

6. **`crates/capco/tests/lattice_static_assertions.rs`** — TWO edits:
   - line 71: keep `assert_impl_all!(DeclassifyOnLattice: JoinSemilattice, MeetSemilattice);`.
   - line 113: **remove** `DeclassifyOnLattice` from the `assert_not_impl_any!(… BoundedJoinSemilattice, BoundedMeetSemilattice)` list, and add a new positive assertion:
     `assert_impl_all!(DeclassifyOnLattice: BoundedJoinSemilattice);` plus keep a negative lock for the
     meet side only: `assert_not_impl_any!(DeclassifyOnLattice: BoundedMeetSemilattice);`. Update the
     surrounding comment block (lines 73-81) which currently claims declassify-on is "deliberately
     unbounded … no lawful finite top" — that rationale is now wrong for the join side (the chain has
     a genuine maximum, `Commingled`); rewrite to: bounded on join (bottom = absence, top-of-chain =
     `Commingled`), unbounded on meet.

7. **`crates/capco/tests/category_lattice_laws.rs:1193-1287`** (`mod declassify_on_lattice`) —
   update `d()`/`y()`/`bottom()` helpers to `from_date(...)`; the existing assoc/comm/idem/absorption
   tests stay valid (they exercise dates → `SpecificDate` tier). Add a new submodule
   `mod declass_instruction_e3` with the oracle fixtures (§ test plan below). Also fix the stale
   `§H.6 p104` citation on line 1191/1238 to the correct `§E.3 p32` authority (the existing comment
   mis-cites §H.6 — §H.6 forbids declass dates on RD docs; §E.3 is the aggregation authority, as the
   `declassify_on.rs` doc-comment already notes).

8. **`crates/capco/tests/post_4b_lattice_inventory_pin.rs`** — re-run; if it asserts a fixed
   bounded-membership set, update the `DeclassifyOnLattice` entry (now `BoundedJoinSemilattice`).

9. **`crates/capco/tests/proptest_lattice.rs`** — add a proptest section for `DeclassInstruction`
   `Ord` totality + `OrdMax` laws (§ test plan).

**`marque-scheme`: NO CHANGE.** `OrdMax`, `BoundedJoinSemilattice`, the lattice traits all already
exist and suffice. Keeping the type in `marque-capco` is correct (it is CAPCO/ISM vocabulary —
`DeclassExemption`, §E.3 — which `marque-scheme` must not accrete per Constitution VII).

**`marque-ism`: NO CHANGE** (Option A in §3.2 avoids exposing `end_components`).

---

## 7. Test plan

All tests in `marque-capco` (`#[cfg(test)]` in `declass_instruction.rs` for unit `Ord` laws;
`tests/category_lattice_laws.rs` + `tests/proptest_lattice.rs` for integration/proptest). `proptest`
is already a `dev-dependency` (`crates/capco/Cargo.toml:54`).

### 7.1 Proptest laws (strategy: generate arbitrary `DeclassInstruction` across all 9 tiers)

Define a `proptest` strategy that produces each variant with plausible `IsmDate`s
(`Year`/`Date` arms) and each `DeclassExemption` code. Laws:

- **Ord totality / consistency**: for all `a,b`: exactly one of `a<b`, `a==b`, `a>b`; and
  `a==b ⟺ a.cmp(b)==Equal` (the precedence-equivalence law — this is the §3.3 contract).
- **Ord antisymmetry**: `a<=b && b<=a ⟹ a==b`.
- **Ord transitivity**: `a<=b && b<=c ⟹ a<=c`.
- **`Eq` agrees with `Ord`**: `(a==b) == (a.cmp(&b)==Equal)` (guards against accidental structural
  `derive`).
- **OrdMax idempotence**: `x.join(&x)==x`, `x.meet(&x)==x`.
- **OrdMax commutativity**: `a.join(&b)==b.join(&a)`; same for meet.
- **OrdMax associativity**: `a.join(&b).join(&c)==a.join(&b.join(&c))`; same for meet.
- **bottom-identity** (newtype): `bottom().join(&x)==x` and `x.join(&bottom())==x`.
- **top-absorption** (newtype): `Commingled` wrapped ⊔ anything == `Commingled` (this is also an
  oracle below, but assert it over the generated strategy too).
- **absorption** (`DeclassifyOnLattice`, total-order ⇒ holds): `a.join(&a.meet(&b))==a`,
  `a.meet(&a.join(&b))==a`.

### 7.2 §E.3 oracle fixtures (each `#[test]`, each citing §E.3 p32/p33)

| Fixture | Assertion | §E.3 cite |
|---------|-----------|-----------|
| `commingled_dominates` | `Commingled ⊔ x == Commingled` for x ∈ {`Exempt50xBeyond(X50x1Hum)`, `SpecificDate(2030)`, `bottom`} | §E.3 p32 line 663 ("takes precedence over all") |
| `unset_is_identity` | `bottom() ⊔ x == x` for several x | §E.3 p32 (join identity; absence) |
| `hum_beats_dated_25x` | `Exempt50xBeyond(X50x1Hum) ⊔ Exempt25xDated(X25x1, 2030) == Exempt50xBeyond(X50x1Hum)` | §E.3 p32 line 664 vs p33 line 673 (tier 2 > tier 5) |
| `hum_beats_wmd_lowest_number` | `Exempt50xBeyond(X50x1Hum) ⊔ Exempt50xBeyond(X50x2Wmd) == Exempt50xBeyond(X50x1Hum)` | §E.3 p32 line 664 ("apply 50X1-HUM … lowest number") |
| `same_50x_tier_later_date_wins` | `Exempt50xDated(X50x3, 2040) ⊔ Exempt50xDated(X50x3, 2050) == Exempt50xDated(X50x3, 2050)` | §E.3 p32 line 665 ("longest period of protection") |
| `specific_date_beats_event` | `SpecificDate(date ≤25yr) ⊔ EventUnder10Year == SpecificDate(date)` | §E.3 p33 line 675 (tier 7) vs line 676 (tier 8) |
| `eo12951_between_50x_and_25x` | `Eo12951 ⊔ Exempt25xDated(X25x1,2030) == Eo12951` AND `Exempt50xDated(X50x9,2030) ⊔ Eo12951 == Exempt50xDated(X50x9,2030)` | §E.3 p33 line 672 (tier 4): below tier 3, above tier 5 |

> The data-model's compact list wrote `50X3(2040) ⊔ 50X3(2050) == 50X3(2050)` and
> `50X1-HUM ⊔ 25X1-dated(2030) == 50X1-HUM` — both encoded above and verified against §E.3.

### 7.3 `from_attrs_iter` seeding tests

- empty slice → `bottom()`.
- single portion `declass_exemption=X50x1Hum` → `Exempt50xBeyond(X50x1Hum)`.
- single portion `declassify_on=Date(2030,1,1)`, no exemption → `SpecificDate(2030-01-01)`.
- two portions, one `X50x1Hum`, one `Date(2099,...)` → join == `Exempt50xBeyond(X50x1Hum)` (tier 2
  dominates a bare date tier 7).
- `into_date()` projection: `Exempt50xDated(X50x3, Date(2040,1,1)).into_date() == Some(Date(2040,1,1))`;
  `Commingled.into_date() == None`; `Exempt50xBeyond(...).into_date() == None`.

---

## 8. clippy / stable-vs-nightly notes

- **`clippy::derive_ord_xor_partial_ord`**: fires (on stable) if you derive one of `Ord`/`PartialOrd`
  and hand-write the other. **Mitigation**: hand-write ALL FOUR (`PartialEq`, `Eq`, `PartialOrd`,
  `Ord`) — do not `#[derive]` any of them. `#[derive(Debug, Clone)]` only.
- **`clippy::non_canonical_partial_ord_impl`**: fires if `partial_cmp` does real work while `Ord::cmp`
  is also defined; the canonical form is `partial_cmp` delegating to `cmp` via `Some(self.cmp(other))`
  — exactly the §3.3 shape. Keep it.
- **`clippy::derived_hash_with_manual_eq`**: do NOT `#[derive(Hash)]` on `DeclassInstruction`
  (manual `Eq` + derived `Hash` would be inconsistent). If a `Hash` impl is ever needed, hand-write it
  off the same `precedence_key`. PR-D1 does not need `Hash` (the `OrdMax` derive of `Hash` requires
  `T: Hash`; `DeclassifyOnLattice` does NOT derive `Hash`, so this is fine — confirm no caller needs
  `DeclassifyOnLattice: Hash`).
- Local clippy is nightly (per project memory `clippy-nightly-vs-stable-drift`). Run
  `cargo +stable clippy -p marque-capco --all-targets -- -D warnings` before opening the PR, not just
  the default nightly clippy.
- **`#[non_exhaustive]` on `DeclassExemption`**: the `exemption_rank` and `instruction_from_attrs`
  matches MUST carry a catch-all `_ =>` arm or they won't compile against the non-exhaustive enum.
  This is the one sanctioned wildcard (it is a foreign non-exhaustive enum, not a local business
  enum) — document why.

---

## 9. Open questions / risks (resolve before/with implementation)

- **OQ-1 (bare-date tier)**: when a portion has `declassify_on=Some(date)` and NO exemption code,
  §E.3 tier 7 ("specific date ≤25yr") vs tier 9 ("calculated 25yr fallback") both fit a bare date.
  Plan assumes **tier 7 `SpecificDate`** (the date is an authored value, not a fallback calculation).
  Confirm: is any seeded bare date ever a tier-9 fallback in practice, or is tier 9 strictly
  engine-minted (PR-D3)? Recommended: tier 7 for seeded dates; reserve tier 9 for derived edges.

- **OQ-2 (undated 50X)**: §E.3 tier 3 is "50X#, *with* a date or event". A portion with a 50X code
  (not HUM/WMD/75X) but NO date is under-specified by the source. Options: (a) treat as tier 2 floor,
  (b) treat as tier 3 with `date: None` sorting earliest within tier 3. The enum as drafted forces a
  date on `Exempt50xDated`. **Decision needed**: either add `Exempt50xUndated { code }` (parallel to
  `Exempt25xUndated`) or document that an undated 50X seeds as `Exempt50xBeyond`-floor. Recommend
  adding `Exempt50xUndated` for faithfulness; it slots between tier 3 and tier 4.

- **OQ-3 (event tier)**: tier 8 ("event <10yr") is free text in CAPCO; the canonical pivot has no
  event-string field. `EventUnder10Year` is therefore a dateless tier marker that `from_attrs_iter`
  never mints. Confirm this is acceptable for PR-D1 (the engine node in PR-D3 mints it from a derived
  edge), or whether the pivot needs an event field (that would be a `marque-ism` edit, out of scope).

- **OQ-4 (`DeclassExemptionAccumulator` retirement)**: `DeclassInstruction` subsumes the
  duration-aware comparator the accumulator's doc-comment flags as a known limitation. PR-D1 leaves
  the accumulator in place (the `marking.rs:630` caller still uses it for the separate
  `out.declass_exemption` field). Confirm the accumulator is retired in PR-D3 (engine node), not D1.

- **OQ-5 (`Aea`/`Nato`/`NatoAea` exemption codes → `Commingled`)**: the generated `DeclassExemption`
  carries `Aea`, `Nato`, `NatoAea` (the §E.4/§E.5 commingling markers). The plan maps these to tier-1
  `Commingled`. Verify against §E.4 p33 line 683 + §E.5 p33 line 687 that a portion carrying these
  codes is exactly the commingling case (it is, per the source), and that no *date* should ever
  coexist (§E.4: "must not contain a declassification date or event") — which is why `Commingled` is
  dateless and the §E.4/§E.5 N/A-string choice is render-only (T070).

- **Risk — `Ord` non-totality bug**: the date sub-key via `end_cmp` returning `Equal` for
  `Year`/`Date` end-of-span collisions is *intended* (precedence-equivalence). The proptest totality
  law (§7.1) is the guard. If a reviewer reads `end_cmp == Equal` as a bug, point them at §3.2/§3.3
  and the §E.3 "longest protection = end-of-span" framing.

- **Risk — stale `§H.6 p104` citation** in `category_lattice_laws.rs:1191/1238`: the existing test
  comment mis-cites §H.6; this PR corrects it to §E.3 p32 (Constitution VIII: propagation requires
  re-verification). Do not carry the §H.6 citation forward.
