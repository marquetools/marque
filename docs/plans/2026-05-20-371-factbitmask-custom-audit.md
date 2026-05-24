# FactBitmask `Constraint::Custom` audit — PR-E (#371)

Sibling doc to `docs/plans/2026-05-20-371-factbitmask-refactor.md` §8.
Catalogs every `Constraint::Custom` row in `CapcoScheme` against the
tier classification from the refactor plan, with the §-citation for
the rule, the compilation status under PR-E (issue #371), and a
pointer to the structural body each row replaces (or retains).

The audit table is the AC #5 deliverable. PR-E lands tier 1 only per
OQ-4 disposition; tier 2 (class-floor catalog, 27 rows) and tier 3
(SCI per-system catalog, 5 rows + 6 structural) are tracked as the
#371 carry-over issue filed by PR-F per the refactor plan §9.

Last verified against `crates/capco/docs/CAPCO-2016.md` at PR-E
authorship.

## Tier classification

The §8 audit in the refactor plan partitions the 39 catalog rows
(7 named-dispatch + 27 class-floor + 5 SCI-per-system) by what shape
of mask test resolves the predicate:

| Tier | Mask shape (firing condition) | Rows | PR-E status |
|------|--------------------------------|------|-------------|
| **1** | Pure-presence — `(bits & TRIGGER) != 0 && (bits & SUPPRESSOR) == 0` | 4 named full + 2 named partial | **LANDED (4 full)** |
| **2** | Numeric chain compare on classification + atom presence | 27 class-floor rows | Deferred (#371 carry-over, filed by PR-F) |
| **3** | Structural compartment / sub-compartment reads | ~6 SCI-per-system rows | Deferred (#371 carry-over, filed by PR-F) |

PR-E compiles the 4 tier-1-full rows. The 2 tier-1-partial rows
(`E014/joint-requires-rel-to-coverage` and `capco/joint-requires-usa`)
have a pure-presence outer guard but a per-country list-walk inner
predicate — they are tracked for partial-compilation in the follow-on,
not in PR-E.

## Tier 1 — landed in PR-E

| Constraint name | Citation | Trigger mask | Suppressor mask | Structural body location |
|----------------|----------|--------------|------------------|--------------------------|
| `E021/rd-frd-requires-noforn` | §H.6 p104 + p111 | `AEA_RD ∪ AEA_FRD` | `NOFORN ∪ RELIDO ∪ REL_TO_PRESENT` | `crates/capco/src/scheme/predicates/tier1_mask.rs::e021_rd_frd_requires_noforn` (retired structural was `constraints/helpers.rs::e021_rd_frd_requires_noforn`) |
| `E024/rd-precedence` | §H.6 p104 | `AEA_RD ∩ (AEA_FRD ∪ AEA_TFNI)` | n/a (precedence is structural) | `crates/capco/src/scheme/predicates/tier1_mask.rs::e024_rd_precedence` (retired structural was `constraints/helpers.rs::e024_rd_precedence`) |
| `E038/nodis-or-exdis-requires-noforn` | §H.9 p172 + p174 | `NODIS ∪ EXDIS` | `NOFORN` | `crates/capco/src/scheme/predicates/tier1_mask.rs::e038_dos_dissem_requires_noforn` (retired structural was `constraints/helpers.rs::e038_dos_dissem_requires_noforn`) |
| `E070/frd-tfni-precedence` | §H.6 p120 | `AEA_FRD ∩ AEA_TFNI` | n/a | `crates/capco/src/scheme/predicates/tier1_mask.rs::e070_frd_tfni_precedence` (retired structural was `constraints/helpers.rs::e070_frd_tfni_precedence`) |

All four rows have:

- An O(1) presence pre-check on the dominant input axis
  (`attrs.aea_markings.is_empty()` for E021/E024/E070;
  `attrs.non_ic_dissem.is_empty()` for E038) before `derive_bits` is
  called.
- A single `derive_bits(attrs).bits()` evaluation.
- A bitwise trigger / suppressor test.
- Inline diagnostic synthesis (message, citation, span, severity)
  preserved byte-identically from the retired structural helpers.

Parity vs the retired structural form is enforced by:

- Co-located unit tests at
  `crates/capco/src/scheme/predicates/tier1_mask.rs::tests` (19 tests
  covering trigger / suppressor / pre-check axes per row).
- Independent oracle proptest at
  `crates/capco/tests/proptest_tier1_mask.rs` (4 cases × 1024
  iterations) where the oracle is re-derived from CAPCO-2016 verbatim
  and does NOT call `derive_bits` or any sibling production code.
- The full workspace `cargo test --workspace` corpus parity gate
  (188 test-result lines, 0 failures at PR-E authorship).

## Tier 1 partial — deferred to follow-on

| Constraint name | Citation | Notes |
|----------------|----------|-------|
| `E014/joint-requires-rel-to-coverage` | §H.3 p57 | Outer guard `JOINT_PRESENT` is pure-presence; inner predicate walks the JOINT producer list against the REL TO list. Compilation requires lifting "every JOINT producer is in REL TO" to a bitmask + open-vocab country code pass — non-trivial. |
| `capco/joint-requires-usa` | §H.3 p57 | Same shape as E014 — outer guard `JOINT_PRESENT`, inner predicate checks `USA ∈ REL TO` via open-vocab country code. Compiles cleanly to `JOINT_PRESENT ∩ REL_TO_USA`-style mask once the REL TO list is lifted into the bitmask (or once the bitmask exposes a `REL_TO_USA` bit; PR-B's atom inventory already has it). |

## Tier 2 — deferred to follow-on (`#371` carry-over issue)

The 27 class-floor catalog rows in
`crates/capco/src/scheme/constraints/class_floor_catalog.rs` share a
common shape: present(atom) → US classification ≥ MIN (numeric chain
compare) AND NATO classification ≥ MIN (numeric chain compare). The
3-bit OrdMax chains at bits 27-29 (US) and 32-34 (NATO) of the
`FactBitmask` (per `crates/capco/src/fact_bitmask.rs` atom inventory
table) are designed precisely for this compilation; the follow-on
issue lifts each row to `ClassFloorBitmaskRow { token_bit,
min_us_chain_level, min_nato_chain_level, ... }` plus a 3-bit chain
extract helper.

This is the heavy lift of the §8 audit (~85% mask coverage when
landed). It defers from PR-E because (a) the row count is large
(27 rows × ~3 unit tests per row × parity proptest), (b) the
classification-chain extract requires a small helper API on
`FactBitmask` that PR-A intentionally omitted, and (c) the
class-floor walker is already declarative and the existing
`class_floor_emit` path is well-tested — the compilation is a perf
refactor, not a correctness change.

## Tier 3 — deferred to follow-on

The 5 SCI per-system catalog rows in
`crates/capco/src/scheme/constraints/sci_per_system_catalog.rs`
read SCI compartment / sub-compartment structure (e.g. "HCS-O
requires NOFORN + ORCON if HCS-P is also present"). Compartment
state is not closed-vocab (custom SCI control systems are
agency-extensible per §H.4) — the bitmask can carry presence
sentinels (`SCI_PRESENT`, `SCI_SI_G`, `SCI_HCS_O`, `SCI_HCS_P_SUB`,
`SCI_TK_BLFH`, `SCI_TK_IDIT`, `SCI_TK_KAND` per the atom inventory)
but not the per-system compartment / sub-compartment graph itself.

The structural reads stay structural in any tier-3 compilation; the
mask test only short-circuits the obvious negative paths. This is
the smallest tier (~6 of 39 = 15% of total), within the §8 audit's
"≥80% mask coverage" slack budget.

## AC #5 status

AC #5 from issue #371 — "≥80% Constraint::Custom rows compiled to
mask form" — is **deferred** per OQ-4 disposition in the refactor
plan §10. PR-E lifts 4 of 39 = 10% rows; the remaining 27 + 6
tier-2/tier-3 rows are the follow-on issue's scope.

PR-D's land-time issue carries the deferral context. PR-F's final
attestation re-states the deferral and the path forward.

## Per-row §-citation re-verification log

Each citation in the tier-1 table above was re-verified against
`crates/capco/docs/CAPCO-2016.md` at PR-E authorship per Constitution
Principle VIII. The citations are byte-identical to the retired
structural helpers in `crates/capco/src/scheme/constraints/helpers.rs`
(pre-PR-E), which were themselves re-verified at #559 close-out
(2026-05-19) per the same principle.

| Citation | Verified |
|----------|----------|
| §H.6 p104 (RD precedence + NOFORN-or-§123/§144) | ✓ |
| §H.6 p111 (FRD: same rule as RD) | ✓ |
| §H.6 p120 (TFNI: not subject to E021; precedence under RD and FRD; FRD-side via E070) | ✓ |
| §H.6 pp116/118 (DOE / DoD UCNI: explicitly excluded from E021 trigger) | ✓ |
| §H.9 p172 (NODIS: NOFORN required) | ✓ |
| §H.9 p174 (EXDIS: NOFORN required) | ✓ |
