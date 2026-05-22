<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# T044 Callsite Inventory — RuleId 2-Tuple Migration

**Date:** 2026-05-22  
**Current Format:** `RuleId(&'static str)` — single-element opaque newtype  
**Target Format:** `RuleId(scheme: &'static str, predicate: &'static str)` — 2-tuple  
**Total Callsites:** 106 `RuleId::new()` calls + numerous field reads + test fixtures

This inventory catalogs every site that constructs, reads, serializes, or tests against `RuleId` before the migration agent begins implementation.

---

## §1 Type Definition and Core Traits

### `crates/rules/src/lib.rs` — Main Definition

**Lines 115–143:** Core struct and impl

```rust
/// Unique rule identifier string (e.g., "E001", "capco/portion-mark-in-banner").
///
/// The inner `&'static str` is private; construct via [`RuleId::new`] so that
/// construction is explicit at every call site.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(&'static str);

impl RuleId {
    /// Construct a rule identifier from a static string slice.
    #[inline]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Return the rule identifier as a string slice.
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
```

---

## §2 RuleId::new() Call Sites (106 total)

### By Crate

#### **crates/engine/** (48 sites)

**src/engine.rs:**
- 113: `const DECODER_RULE_ID: &str = "R001";` (const decl, not RuleId yet)
- 147: `pub const R002_RULE_ID: RuleId = RuleId::new("R002");` — engine sentinel
- 1996: `RuleId::new("C001")` — test fixture
- 2409: `RuleId::new("E058")` — walker classifier bridging
- 2411: `RuleId::new("E059")` — walker classifier bridging
- 2430: `RuleId::new(id_part)` — dynamic from string, bridge emitted ID
- 2432: `RuleId::new("E053")` — fallback decoder mode
- 2434: `RuleId::new("E008")` — fallback unrecognized token
- 2437: `RuleId::new("E008")` — fallback unrecognized token (duplicate context)
- 4341: `let rule = RuleId::new(DECODER_RULE_ID);` — R001 decoder sentinel
- 5651: `RuleId::new(self.id)` — per-rule dynamic emit, field from Rule trait
- 5731: `rule: RuleId::new(rule)` — diagnostic construction
- 5952: `RuleId::new("E997")` — test sentinel
- 5966: `RuleId::new("E997")` — test sentinel
- 6009: `RuleId::new("S999")` — test sentinel
- 6036: `RuleId::new("S999")` — test sentinel
- 6154: `RuleId::new("RECORD")` — test synthetic ID
- 6623: `RuleId::new("PARSED_CACHE_TEST")` — test synthetic ID
- 7133: `RuleId::new(self.id)` — per-rule dynamic
- 7485: `smallvec![RuleId::new("C001"), RuleId::new("E006")];` — test fixture
- 7550: `RuleId::new("E006")` — test fixture
- 7558: `RuleId::new("E022")` — test fixture
- 7566: `RuleId::new("E999")` — test fixture
- 7574: `RuleId::new("C001")` — test fixture
- 7633: `RuleId::new("E006")` — test fixture
- 7641: `RuleId::new("E022")` — test fixture
- 7671: `RuleId::new("C001")` — test fixture
- 7731: `RuleId::new("E899")` — test fixture
- 7769: `RuleId::new("E899")` — test fixture
- 7833: `RuleId::new("E898")` — test fixture
- 7863: `RuleId::new("E898")` — test fixture
- 8049: `rule: RuleId::new(rule)` — diagnostic construction
- 8337: `RuleId::new(rule)` — dynamic from closure rule name mapping
- 8442: `smallvec::smallvec![RuleId::new("E006")];` — test fixture
- 8523: `RuleId::new("C001")` — test fixture
- 8580: `RuleId::new("E006")` — test fixture

**src/output.rs:**
- 305: `RuleId::new("E001")` — render example/test
- 331: `RuleId::new("W034")` — render example/test
- 339: `RuleId::new("W034")` — render example/test
- 347: `RuleId::new("W003")` — render example/test
- 355: `RuleId::new("E001")` — render example/test

**tests/audit.rs:**
- 521: `RuleId::new("C001")` — integration test fixture

**tests/audit_note_sealing_carve_out.rs:**
- 94: `RuleId::new("capco/noforn-if-no-fdr")` — audit-note test
- 145: `RuleId::new("test/clone")` — audit-note test

**tests/rule_panic_isolation.rs:**
- 46: `RuleId::new("Z001")` — panic isolation test
- 94: `RuleId::new("Z002")` — panic isolation test
- 264: `RuleId::new("Z003")` — panic isolation test

#### **crates/capco/** (38 sites)

**src/rules.rs:**
- 509: `RuleId::new("E002")` — rule definition
- 851: `RuleId::new("E005")` — rule definition
- 939: `RuleId::new("E006")` — rule definition
- 1085: `RuleId::new("E007")` — rule definition
- 1326: `RuleId::new("E008")` — rule definition
- 1474: `RuleId::new("C001")` — rule definition
- 1669: `RuleId::new("S003")` — rule definition
- 2028: `RuleId::new("S004")` — rule definition
- 2240: `RuleId::new("W003")` — rule definition
- 2878: `RuleId::new("S005")` — rule definition in message
- 2909: `RuleId::new("S005")` — rule definition
- 3035: `RuleId::new("W034")` — rule definition
- 3140: `RuleId::new("E061")` — rule definition
- 3239: `RuleId::new("E062")` — rule definition
- 3363: `RuleId::new("E063")` — rule definition
- 3470: `RuleId::new("E064")` — rule definition
- 3898: `RuleId::new("S007")` — rule definition
- 4205: `RuleId::new("S008")` — rule definition
- 4348: `RuleId::new("S009")` — rule definition
- 4533: `RuleId::new("S010")` — rule definition
- 4625: `RuleId::new("S010")` — additional emitted ID
- 4667: `RuleId::new("E072")` — rule definition
- 4736: `RuleId::new("E072")` — additional emitted ID
- 5021: `RuleId::new("E039")` — rule definition
- 5166: `RuleId::new("E031")` — rule definition
- 5292: `rule_id: RuleId::new("E031")` — catalog row struct
- 5303: `rule_id: RuleId::new("E035")` — catalog row struct
- 5312: `rule_id: RuleId::new("E040")` — catalog row struct
- 5333: `rule_id: RuleId::new("E068")` — catalog row struct
- 5357: `rule_id: RuleId::new("E069")` — catalog row struct
- 6104: `RuleId::new("E041")` — rule definition
- 6301: `RuleId::new("E066")` — rule definition
- 6557: `RuleId::new("W004")` — rule definition
- 6800: `RuleId::new("E071")` — rule definition

**src/rules_declarative.rs:**
- 431: `RuleId::new("E065")` — declarative walker definition (DeprecatedSciLongFormRule)
- 741: `RuleId::new("E067")` — declarative walker definition (DeclarativeRecanonicalizeRule)

**src/scheme/sci_per_system.rs:**
- 93: `pub(crate) const RULE_E059: marque_rules::RuleId = marque_rules::RuleId::new("E059");` — const decl for per-row identity

#### **crates/rules/** (9 sites)

**src/lib.rs:**
- 1319: `let r = RuleId::new("E001");` — doctest/example

**src/audit.rs:**
- 396: `/// rule: RuleId::new("E001"),` — doc comment example

**tests/engine_promotion_seal.rs:**
- 159: `RuleId::new("E001")` — promotion seal test

**tests/message_args_closed_set.rs:**
- 71: `RuleId::new("C001")` — MessageArgs test
- 72: `RuleId::new("E006")` — MessageArgs test
- 91: `&[RuleId::new("C001"), RuleId::new("E006")]` — MessageArgs test assertion

**tests/applied_text_correction_seal.rs:**
- 39: `RuleId::new("C001")` — text correction test
- 55: `assert_eq!(correction.rule, RuleId::new("C001"));` — text correction test
- 72: `RuleId::new("E006")` — text correction test

#### **crates/wasm/** (3 sites)

**tests/audit_v1_0_parity.rs:**
- 94: `RuleId::new(rule)` — parity test, parametrized
- 119: `RuleId::new("C001")` — parity test fixture
- 354: `RuleId::new("E006")` — parity test fixture

#### **marque/** (8 sites)

**src/render.rs:**
- 1022: `RuleId::new(rule)` — dynamic from audit line
- 1202: `RuleId::new("E008")` — render example/test
- 1229: `RuleId::new("S004")` — render example/test
- 1274: `RuleId::new("S004")` — render example/test
- 1310: `RuleId::new("S999")` — render example/test
- 1352: `RuleId::new("S004")` — render example/test
- 1443: `RuleId::new("E002")` — render example/test
- 1564: `RuleId::new("C001")` — render example/test

---

## §3 Const Declarations and Named Sentinels

**Engine Sentinels:**
- `crates/engine/src/engine.rs:113` — `const DECODER_RULE_ID: &str = "R001";` (string, not RuleId)
- `crates/engine/src/engine.rs:147` — `pub const R002_RULE_ID: RuleId = RuleId::new("R002");` (reparse failure)

**Catalog Sentinels:**
- `crates/capco/src/scheme/sci_per_system.rs:93` — `pub(crate) const RULE_E059: marque_rules::RuleId = marque_rules::RuleId::new("E059");` (per-system row identity)

**Note:** Most rule IDs are emitted dynamically via the `Rule::id()` trait method, not as named const.

---

## §4 Field Reads and Pattern Matches

### AppliedFix / AppliedTextCorrection Field Reads

**Pattern:** `.rule` on `AppliedFix<S>` and `AppliedTextCorrection`

**crates/engine/src/engine.rs:**
- 3480: `(&fix.rule, fix.span)` — audit grouping/trace
- 3481: `(&tc.rule, tc.span)` — audit grouping/trace
- 3524: `&fix.rule` — rule grouping
- 3525: `&tc.rule` — rule grouping
- 8453: `f.rule.as_str()` — JSON serialization
- 8454: `tc.rule.as_str()` — JSON serialization
- 8458: `f.rule.as_str()` — JSON serialization
- 8459: `tc.rule.as_str()` — JSON serialization

**crates/engine/tests/audit_completeness.rs:**
- 49: `(&f.rule, f.span, ...)` — audit record grouping
- 50: `(&tc.rule, tc.span, ...)` — audit record grouping
- 250: `&f.rule` — rule grouping
- 251: `&tc.rule` — rule grouping

**crates/engine/tests/audit.rs:**
- 616: `(&tc.rule, tc.span, ...)` — audit record grouping

### `.as_str()` Call Sites (to_string equivalents)

**crates/marque/src/render.rs:**
- 815: `rule: fix.rule.as_str(),` — AuditRecordJsonV1_0 serialization
- 839: `rule: tc.rule.as_str(),` — TextCorrectionRecordJsonV1_0 serialization

**crates/wasm/src/lib.rs:**
- Line numbers via context: serialization in `applied_fix_to_audit_json_v1_0` and `text_correction_to_audit_json_v1_0`

---

## §5 Audit-Record JSON Serialization

### Current Shape

**Contract:** `specs/006-engine-rule-refactor/contracts/audit-record.md` §107-178 (AppliedFix) + §388-402 (TextCorrection)

```json
{
  "type": "applied_fix",
  "schema": "marque-1.0",
  "rule": "E001",
  "severity": "error",
  "span": {"start": 0, "end": 5},
  ...
}
```

The `"rule"` field is a single string. Migration will require a choice:
- Option A: Change to `"rule": {"scheme": "E", "predicate": "001"}` (breaking change to schema version)
- Option B: Keep as `"rule": "E:001"` or similar compound string in JSON (forward-compat, no schema bump)
- Option C: Emit both `"scheme"` and `"predicate"` as sibling fields

### Serializers

**crates/marque/src/render.rs:**
- 791–828: `applied_fix_to_audit_json_v1_0()` — line 815 `rule: fix.rule.as_str(),`
- 832–856: `text_correction_to_audit_json_v1_0()` — line 839 `rule: tc.rule.as_str(),`
- 871–883: `audit_line_to_json_v1_0()` — dispatcher that calls the above

**crates/wasm/src/lib.rs:**
- 641–679: `applied_fix_to_audit_json_v1_0()` (WASM version, parallel to marque)
- 681–718: `text_correction_to_audit_json_v1_0()` (WASM version)
- 720–724: Dispatcher with `serde_json::to_value()` calls

### Schema Constant

**crates/engine/src/lib.rs:**
- 90: `pub const AUDIT_SCHEMA_VERSION: &str = env!("MARQUE_AUDIT_SCHEMA");`
- 102: `pub const AUDIT_SCHEMA_IS_V1_0: bool = const_str_eq(AUDIT_SCHEMA_VERSION, "marque-1.0");`

Currently pinned to `"marque-1.0"` (env var `MARQUE_AUDIT_SCHEMA`). Changing `RuleId` shape in audit output requires coordination with schema versioning.

---

## §6 Test Fixtures and Corpus

### Test Corpus (62 files with rule IDs)

**Location:** `tests/corpus/invalid/*.expected.json`

Example:
```json
{"rule": "E008", "span": {"start": 8, "end": 13}}
```

All 62 expected.json files use the flat `"rule": "ID"` format. These are regression-pinning fixtures that will require updating if audit-record JSON shape changes.

**Critical:** Any change to RuleId serialization must sweep all corpus fixtures or test harness must adapt.

### Integration Tests with RuleId Assertions

**crates/capco/tests/post_3b_registration_pin.rs:**
- Lines 116–176: `EXPECTED_RULE_IDS: &[&str]` — 28 registered rule IDs (pinned, exact set)
- Test `post_pr_578_registers_exact_28_rule_ids()` matches via `r.id().as_str()` → `BTreeSet<String>`

**crates/capco/tests/post_4b_lattice_inventory_pin.rs:**
- Lattice static-assertion pins; searches for `JoinSemilattice` impl presence, not ID strings

**crates/capco/tests/corpus_parity.rs:**
- Line 125: Comment noting `Diagnostic.rule = "E058"` (bridge-emitted, not walker)
- Line 148: Comment noting `Diagnostic.rule = "E059"` (bridge-emitted)
- Line 166: Comment noting `Diagnostic.rule = "E060"` (bridge-emitted, per-row identification)

**crates/engine/tests/audit_g13_canary.rs:**
- Line 462: JSON fixture with `"rule":"R999"`
- Line 529: `RuleId::new("R999")` — synthetic canary rule for content-ignorance scan
- Line 591: JSON fixture with `"rule":"R999"`

**crates/wasm/tests/audit_v1_0_parity.rs:**
- Byte-identity parity test between CLI and WASM renderers
- Uses `RuleId::new(rule)` with parametrized `rule` values
- Validates `marque-1.0` shape invariants across variants

### Test Synthetic Rule IDs (Non-Production)

Used only in tests, not in real rule registration:
- `"Z001"`, `"Z002"`, `"Z003"` — panic-isolation tests
- `"E997"`, `"E998"` — test/canary markers
- `"S999"`, `"E899"`, `"E898"` — test fixtures
- `"RECORD"`, `"PARSED_CACHE_TEST"` — test synthetic identifiers
- `"R999"` — audit canary (content-ignorance check)
- `"capco/noforn-if-no-fdr"`, `"test/clone"` — audit-note tests

---

## §7 Walker Rules with Per-Row Identity

**Key Pattern:** One registered walker emits diagnostics with multiple rule IDs.

### BannerMatchesProjectedRule (E031)

**File:** `crates/capco/src/rules.rs` lines 5134–5251

**Registered ID:** `E031` (bookkeeping)

**Per-Row IDs Emitted:**
- `E031` — SAR roll-up
- `E035` — SCI roll-up
- `E040` — Non-IC dissem roll-up
- `E068` — Banner classification mismatch
- `E069` — Banner FGI marker mismatch

**Catalog Structure:**
```rust
struct BannerCategoryRow {
    rule_id: RuleId,  // Lines 5291–5357: five RuleId::new() calls
    severity: Severity,
    evaluate: fn(...) -> Vec<Diagnostic<CapcoScheme>>,
}
```

Each row's evaluator function constructs `Diagnostic` with the row's `rule_id` field.

**Migration Impact:** Each `RuleId::new()` in the catalog must change to 2-tuple form. The per-row structure persists unchanged; only the `RuleId` field type changes.

### DeprecatedSciLongFormRule (E065)

**File:** `crates/capco/src/rules_declarative.rs` lines 408–470

**Registered ID:** `E065`

**Pattern:** Line 431 `RuleId::new("E065")` — single emission, not multi-row. No special per-row identity pattern.

### DeclarativeRecanonicalizeRule (E067)

**File:** `crates/capco/src/rules_declarative.rs` lines 718–760

**Registered ID:** `E067`

**Pattern:** Line 741 `RuleId::new("E067")` — single emission.

### Retired Walkers (Bridge-Emitted, Not Registered)

These no longer emit via walker code; the engine's constraint-catalog bridge emits them:
- `DeclarativeClassFloorRule` (E058) — 27 class-floor catalog rows
- `DeclarativeSciPerSystemRule` (E059) — 5 SCI per-system catalog rows

**Bridge Emission:** `crates/engine/src/engine.rs` lines 2409–2434, dynamic ID construction via `RuleId::new(id_part)`.

**Migration Impact:** These are emitted dynamically from the bridge, not as const `RuleId::new()` calls. The dynamic emission at line 2430 `RuleId::new(id_part)` will need special handling if `id_part` cannot be split into scheme + predicate at runtime.

---

## §8 Config File Parsing and Severity Overrides

### .marque.toml Schema

**Section:** `[rules]` — rule-ID to severity mappings

Example:
```toml
[rules]
E001 = "error"
E035 = "warn"
```

### Parser Location

**crates/config/src/lib.rs:**
- Lines 606–893+: TOML parsing, rule-ID to severity resolution
- Rule IDs are string keys in the `[rules]` map; they are looked up against registered rule IDs

### Severity Override Resolution

The config loader maps TOML keys (strings like `"E001"`, `"E035"`) to `RuleId` values via:
1. String key from TOML
2. Lookup against `CapcoRuleSet::new()` registered rule IDs
3. Map to the registered `Rule::id()` value (a `RuleId`)

**Migration Impact:** The config key is a string; the lookup is currently a string-to-RuleId comparison. If `RuleId` becomes a 2-tuple, the config parser must decide:
- Keep string keys (parse scheme + predicate from the string)?
- Change to structured TOML (e.g., `[rules."E:001"]`)?

### Closure Rules Section

**crates/config/tests/closure_severity.rs** lines 9–400+

The `[closure_rules]` section (T108f) uses closure-rule names (strings like `"capco/noforn-if-no-fdr"`), not RuleIds. No direct `RuleId` dependency here, but same config-merge logic applies.

---

## §9 Cross-References and Hidden Coupling

### Hardcoded Rule ID Prefixes

Search results confirm no bare string concatenation like `"E" + "001"`. All rule IDs are:
1. Literal `RuleId::new("E001")` calls (106 sites)
2. Dynamic via `self.id` from `Rule` trait
3. Dynamic via bridge emission (`id_part` from constraint catalog)

### Documentation References

**CLAUDE.md:**
- Line 159: Mentions `MARQUE_AUDIT_SCHEMA` coordination with `FeatureId` additions
- Line 290: Audit schema pinned at `"marque-1.0"`
- Line 345: Schema validation and re-export as `marque_engine::AUDIT_SCHEMA_VERSION`

**crates/capco/README.md:** Not checked for rule-ID doc strings (would be stale post-migration)

**Rule Comments:** Many rule implementations carry authority citations (CAPCO-2016 §X.Y pZ) that reference rule IDs in passing (e.g., "E058 emits per-row"). These are stale post-migration if rule-ID format changes in audit output or user-facing text.

---

## §10 Summary by Artifact Type

| Type | Count | Notes |
|------|-------|-------|
| `RuleId::new()` calls | 106 | Mechanical rename to 2-tuple constructor |
| `.rule` field reads | ~12 | On `AppliedFix`, `AppliedTextCorrection` — cascade from type change |
| `.as_str()` calls | ~8 | Serialization boundary; requires format decision |
| Const `RuleId` declarations | 2 | `R002_RULE_ID`, `RULE_E059` |
| Test corpus files with rule IDs | 62 | Regression fixtures; update if JSON shape changes |
| Walker rules emitting per-row IDs | 1 active | `BannerMatchesProjectedRule` (5 per-row IDs in catalog) |
| Retired walkers (bridge-emitted) | 2 | `E058`, `E059` — dynamic ID construction via bridge |
| Engine sentinels | 1 | `R002_RULE_ID` (reparse failure) |
| Test synthetic sentinels | ~12 | Non-production rule IDs for testing |

---

## §11 Blast-Radius Zones

### Zone A: Type Definition and Trait Bounds (Minimal)
- `RuleId` struct in `crates/rules/src/lib.rs`
- Any trait impls that depend on field layout (e.g., `Ord`, `Hash`)
- **Action:** Mechanical struct-field change, trait impls regenerate

### Zone B: Constructor Sites (106 High-Touch)
- All `RuleId::new("...")` → change to `RuleId::new("E", "001")` or equivalent
- **Action:** Systematic rename + validation that scheme/predicate split is correct

### Zone C: Serialization Boundary (Decision Point)
- `applied_fix_to_audit_json_v1_0()` in `marque/src/render.rs` (line 815)
- `text_correction_to_audit_json_v1_0()` in `marque/src/render.rs` (line 839)
- Parallel functions in `crates/wasm/src/lib.rs`
- **Action:** Choose JSON shape:
  - Compound string `"E:001"` (no schema bump)
  - Struct `{"scheme": "E", "predicate": "001"}` (schema version bump)
  - Sibling fields `"scheme": "E", "predicate": "001"` (schema extension)

### Zone D: Test Fixtures (62 Expected.json Files)
- All `tests/corpus/invalid/*.expected.json` hardcode `"rule": "E001"` etc.
- **Action:** Automated sweep or test harness adaptation if JSON shape changes

### Zone E: Config File Parser (One Site)
- `crates/config/src/lib.rs` TOML `[rules]` parsing
- **Action:** Decide if config keys remain strings or change to structured TOML

### Zone F: Walker Catalog Rows (Low Touch, 1 Rule)
- `BannerMatchesProjectedRule` catalog at `crates/capco/src/rules.rs` lines 5291–5357
- Five `rule_id: RuleId::new()` fields in const array
- **Action:** Migrate all five in lockstep

---

## §12 Silent Risks and Non-Obvious Patterns

### Risk 1: Bridge-Emitted IDs (Engine Constraint-Catalog Bridge)

**Location:** `crates/engine/src/engine.rs:2430` `RuleId::new(id_part)`

The constraint-catalog bridge dynamically emits rule IDs (E058, E059) by reading from the catalog's `rule_id` string field. If the new `RuleId` 2-tuple requires parsing a scheme from the string at runtime, this site must do the split. Currently:

```rust
RuleId::new(id_part)  // id_part = "E058"
```

Post-migration, this must become:

```rust
RuleId::new_from_catalog_entry(id_part)  // or manual parse
```

**Impact:** Blocks migration until bridge-ID generation logic is clarified.

### Risk 2: Dynamic Rule ID from Rule::id()

**Locations:** `crates/engine/src/engine.rs:5651`, line 7133, and others

Many sites construct `RuleId` from the `Rule::id()` trait method:

```rust
RuleId::new(self.id)  // self.id is &'static str from Rule trait
```

After migration, the trait method must return either:
- A new `RuleId(scheme, predicate)` value (change return type)
- A compound string like `"E:001"` (parse at constructor)
- A literal that the constructor can split (add scheme/predicate parsing to `new()`)

**Impact:** Requires updating all 28 registered rules' `fn id()` implementations.

### Risk 3: Test Fixture Parametrization

**Location:** `crates/wasm/tests/audit_v1_0_parity.rs:94` `RuleId::new(rule)`

The parity test parametrizes rule IDs as strings:

```rust
fn synth_applied_fix(rule: &'static str, ...) -> AuditAppliedFix<CapcoScheme> {
    ...
    AuditAppliedFix::<CapcoScheme>::__engine_promote(
        RuleId::new(rule),  // rule comes from parametrized input
        ...
    )
}
```

Post-migration, either:
- The parameter must be a parsed tuple or struct
- The constructor must infer scheme from a compound string
- Test fixtures must change signature

**Impact:** Low blast radius (test code only), but test parametrization pattern may need rethinking.

### Risk 4: Per-Row Identity in Catalog Rows (BannerMatchesProjectedRule)

**Location:** `crates/capco/src/rules.rs` lines 5291–5357 (BannerCategoryRow struct)

Five `rule_id` fields are const-initialized. If the new `RuleId` 2-tuple requires named variant constructors (e.g., `RuleId::capco(...)` vs. `RuleId::engine(...)`) to distinguish schemes, the catalog rows must use the right constructor. Currently, all five are identical `RuleId::new()` calls; post-migration, they might differ if scheme inference is not automatic.

**Impact:** Verify that all five catalog rows use the same scheme post-migration.

### Risk 5: JSON Serialization Format Not Specified

The inventory has identified the serialization points but NOT the JSON output format for the 2-tuple. Three options exist; only one can be chosen:

1. **Compound string** (no schema bump): `"rule": "E:001"`
2. **Structured object** (schema bump): `"rule": {"scheme": "E", "predicate": "001"}`
3. **Sibling fields** (schema extension): `"scheme": "E", "predicate": "001"`

This decision gates 62 corpus-fixture updates and audit-stream consumers.

**Impact:** Decision point — requires architecture sign-off before implementation.

---

## Appendix: Files Not Checked (Out of Scope)

- `crates/ism/`, `crates/scheme/`, `crates/decision/` — no RuleId definitions or direct rule-ID usage
- `tools/`, `benchmarks/` — no rule-ID logic identified
- CAPCO rule-documentation comments with rule-ID citations (stale post-migration, not executable)

