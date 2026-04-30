<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Implementation Plan: Issues #208 and #206 — ISMCAT Tetragraph Taxonomy + Uncertain-Reduction Diagnostic

**Date:** 2026-04-28
**Status:** draft — pending review
**Revision:** rev3 (2026-04-28).

**Revision history:**
- **rev1 (2026-04-28)** — initial draft based on ultraplan output.
- **rev2 (2026-04-28)** — corrected against the actual ISMCAT
  Convenience-Light V2022-NOV package after download. Schema is
  three-state (`Yes` / `No` / `NA`), not four; `NA` means *deprecated*
  (per XSD docstring), not "membership unknown"; `<Membership>` is an
  `xs:choice` of `Country/Organization | Description | MembershipSupressed`
  (note ODNI's misspelling); 18 of 18 `NA` entries carry the optional
  `deprecated="YYYY-MM-DD"` attribute.
- **rev3 (2026-04-28)** — recursion is a non-issue. The denormalized
  file is pre-flattened by ODNI: GMIF → 30 NATO trigraphs inline.
  Only BHTF (NA-deprecated) carries an `<Organization>` ref in the
  denormalized form, and `NA → None` already covers it. Both
  `TetragraphTaxonomy.xml` (canonical hierarchical) and
  `TetragraphTaxonomyDenormalized.xml` are vendored at
  `crates/ism/schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/`; build.rs
  parses only the denormalized form. No recursion code or follow-on
  issue needed — replaced with a defensive `cargo:warning=…` if a
  future revision introduces unexpected `<Organization>` refs in the
  denormalized file. PR review (Copilot, 2026-04-29) caught residual
  rev1 wording in §1, §2.3, and §2.8 that contradicted rev2's
  three-state schema; corrected in this revision.
**Related issues:** #208 (prerequisite), #206 (follow-on), #183 (parent),
#205 (PR-B that introduced `BUILTIN_TETRAGRAPH_MEMBERS`)
**Authoritative sources:**
- ODNI ISMCAT Tetragraph Taxonomy V2022-NOV
  (`Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml`, distributed in
  the ISMCAT spec package — see §1.1 below).
- CAPCO-2016 §D Table 3 rule 23 (`crates/capco/docs/CAPCO-2016.md` p32 / line 610).
- ODNI Schematron `ISM_XML-ROLLUP-phase.xsl` (already vendored at
  `crates/ism/schemas/ISM-v2022-DEC/Schematron/`) — defines the
  decomposability semantics machine-readably.
**Constitution gates:** Principle III (WASM safety — `marque-ism`
build-time only), Principle IV (two-layer rule architecture, schema
version pinning), Principle VIII (authoritative source fidelity —
the XML is the source, vendored in-tree).

---

## 1. Context

#208 and #206 together close the silent-loss gap in REL TO intersection
that #183 / #205 left open. Today `BUILTIN_TETRAGRAPH_MEMBERS` only
knows FVEY and ACGU; every other code (NATO, EU, GCCH, KFOR, MCFI,
org-fork extensions) is treated identically as an opaque atom. The
ISMCAT Tetragraph Taxonomy distinguishes three semantically different
kinds of code:

1. **Decomposable, members known** (FVEY, ACGU, TEYE, NATO,
   AUSTRALIA_GROUP, NSG, …). `decomposable = "Yes"` in the taxonomy;
   `<Membership>` lists trigraph (and occasionally tetragraph) members.
2. **Atom by authority** (EU, GCCH, KFOR, …). `decomposable = "No"`.
   Atom semantics is the *correct* answer; the code IS the recipient.
3. **Reduction unknown from available taxonomy data.** Two distinct
   sub-cases that share the same runtime mapping (`is_decomposable →
   None`) but differ in provenance:
   - **Deprecated, membership not published** (RSMA, ISAF, MCFI,
     CFOD, …). `decomposable = "NA"` with `deprecated="YYYY-MM-DD"`
     in the taxonomy; `<Membership>` is suppressed (or carries an
     OCA-deferral `<Description>`, or recurses into another
     tetragraph). Reduction is not computable from the vendored
     data.
   - **Absent from the taxonomy** — org-fork extensions and any
     other code outside ODNI's V2022-NOV publication. Not an ISMCAT
     state at all; the operator's local data is the only source.

   In both sub-cases the atom-semantics result happens to be correct
   only by coincidence — the operator can't tell from the marking
   alone whether reduction would have removed or preserved a code in
   the intersection.

#208 surfaces this trichotomy at build time. #206 surfaces it at lint
time when an unknown / uncertain code drops out of REL TO intersection
and there is reason to think the operator should look closer.

**Sequential ordering is required**: #208's `is_decomposable` API is
the discriminator #206 depends on. Ship as **two PRs**, #208 first.

### 1.1 Source of truth: which ODNI bundle

The ISMCAT Tetragraph Taxonomy is published as XML inside an ISMCAT
package on the ODNI IC Technical Specifications page. ODNI publishes
three bundle variants per spec, distinguished as follows (from the ODNI
package frontmatter, which is consistent across specs):

| Variant | Contents | Use case |
|---|---|---|
| **Standalone** | The spec self-contained — its own copy of every dependency vendored inside. Largest. Validates in isolation. | Vendoring a single spec without other ODNI packages alongside. |
| **Convenience** | References dependencies via relative path to other ODNI specs. Smaller; assumes you have ISM, GENC, etc. checked out next to it. | Most production users (multiple specs validated as a set). |
| **Light** | Same shape as Convenience but with non-normative artifacts (samples, rendered HTML, change logs) stripped. | Minimum download for build pipelines that only need the schemas + schematron + taxonomy. |

We already vendor ISM-v2022-DEC under
`crates/ism/schemas/ISM-v2022-DEC/`. We need ISMCAT alongside it; we do
*not* want a second redundant copy of the ISM CVE files. **Convenience-Light
is the recommended pick** — it gives us the Taxonomy directory and the
ISMCAT-specific Schema/Schematron/CVEGenerated, and references our
existing ISM tree by relative path.

Download URL (V2022-NOV):

```
https://www.odni.gov/files/documents/CIO/ICEA/Dec2022/ISMCAT/ISMCAT-Public-Convenience-2022-DEC-Public-Light.zip
```

After extraction, the file we care about is:

```
Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml
```

This file is what the rollup XSL at
`crates/ism/schemas/ISM-v2022-DEC/Schematron/ISM_XML-ROLLUP-phase.xsl`
already references (via `document('../../Taxonomy/ISMCAT/...')` —
i.e. ODNI's own schematron expects this directory to be present
alongside `Schematron/`). Vendoring it satisfies that path resolution
*and* gives us a parseable source for `build.rs`.

**Vendor target**: `crates/ism/schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/`.
Pull in the entire `Taxonomy/` directory the package ships, not just
the one file — the rollup XSL may dereference siblings, and keeping the
directory shape intact preserves the relative-path contract.

### 1.2 Decomposability state space (rev2 — corrected against the schema)

The taxonomy XSD restricts `decomposable` to three values: **`Yes`**,
**`No`**, **`NA`**. The rollup XSL groups `Yes ∪ NA` → "decomposable"
and `No` → "do not decompose." `NA` is documented in the XSD as
*"applied to deprecated tetragraphs"* — every `NA` entry in the actual
data also carries `deprecated="YYYY-MM-DD"`.

`<Membership>` is an `xs:choice` of three mutually-exclusive bodies:

- `<Country>…</Country>` and/or `<Organization>…</Organization>` (one or more) — concrete members.
- `<Description>…</Description>` — free text, typically a "refer to original classification authority" deferral.
- `<MembershipSupressed/>` — sentinel that data exists but is not published. (ODNI's spelling — `Supressed`, single `p` — is wrong, but it is the schema, so we match it.)

`<Organization>` references another tetragraph (recursive
decomposition in the *canonical* `TetragraphTaxonomy.xml`). The
**denormalized** form `TetragraphTaxonomyDenormalized.xml` —
which is what `build.rs` parses, and what ODNI's own rollup XSL reads
— is pre-flattened: GMIF, for example, lists 30 NATO trigraphs
inline rather than `<Organization>NATO</Organization>`. Only one
entry retains an `<Organization>` ref in the denormalized file: BHTF
(NA-deprecated) → `<Country>USA</Country> + <Organization>MNTF</Organization>`,
where MNTF is itself `decomposable="No"` + suppressed. BHTF maps to
`None` from the NA → deprecated branch regardless, so the recursion
is operationally inert. We keep the recursive-detection flag in
`TaxMembership::Members` as a defensive build-time guard against
future taxonomy revisions, but no recursion code is needed.

The runtime API is three-state:

```rust
pub fn is_decomposable(code: &str) -> Option<bool>;
//   Some(true)  ← taxonomy decomposable=Yes (always has Country list in V2022-NOV)
//                 OR taxonomy decomposable=NA AND <Country>/<Organization> present and non-recursive
//   Some(false) ← taxonomy decomposable=No (always Suppressed in V2022-NOV — atom by authority)
//   None        ← taxonomy decomposable=NA AND (Suppressed | Description | recursive Organization)
//                 OR code absent from taxonomy                        (org-fork extension)
```

**Empirical V2022-NOV distribution** (61 entries):

| `decomposable` | Membership body | Count | Runtime mapping |
|---|---|---:|---|
| `Yes` | `<Country>` list | **24** | `Some(true)` |
| `No`  | `<MembershipSupressed/>` | **19** | `Some(false)` |
| `NA` (deprecated) | `<MembershipSupressed/>` | 14 | `None` |
| `NA` (deprecated) | `<Description>` (OCA deferral) | 3 | `None` |
| `NA` (deprecated) | `<Country>` + `<Organization>` (recursive) | 1 (BHTF) | `None` (v1) |

**Why three states, not two**: #206's signal value is the third
state. Collapsing `None → false` would erase the operator's signal
that membership data could change the answer.

---

## 2. Phase 1 — Issue #208: ISMCAT Taxonomy build-time parsing

### 2.1 Files changed

```
crates/ism/schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/                  (NEW — vendored)
crates/ism/build.rs                                                 (MODIFY)
crates/ism/Cargo.toml                                               (MODIFY)
crates/ism/src/lib.rs                                               (MODIFY)
crates/ism/src/generated.rs                                         (MODIFY — include new file)
crates/capco/src/vocab.rs                                           (MODIFY)
crates/capco/tests/tetragraph_consolidation.rs                      (MODIFY)
```

### 2.2 Vendoring the taxonomy (✅ done in rev3)

Already landed alongside this plan revision:

```
crates/ism/schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/TetragraphTaxonomy.xml              (58,798 bytes, sha256 11a1757a…)
crates/ism/schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml  (59,633 bytes, sha256 dc972f6f…)
```

The directory shape preserves the rollup XSL's relative-path
contract (`document('../../Taxonomy/ISMCAT/…')`). Both files are
recorded in `crates/ism/schemas/ISM-v2022-DEC/SHA256SUMS`. The
existing `crates/ism/REUSE.toml` annotation covers `schemas/**` as
`LicenseRef-PublicDomain` (matches every other ODNI-vendored
asset), so no per-file SPDX work is needed.

**Why both XML files** (rev3 nuance): the canonical hierarchical
form (`TetragraphTaxonomy.xml`) is the source of record — it shows
GMIF as `AUS, JPN, NZL, USA + <Organization>NATO</Organization>`.
The denormalized form (`TetragraphTaxonomyDenormalized.xml`) is the
pre-flattened operational artifact — GMIF expanded inline to all 30
NATO trigraphs deduped against the original 4. We parse only the
denormalized form (it's what the rollup XSL also reads), but having
the canonical form vendored means a reviewer can verify ODNI's
denormalization stayed in sync at any moment, and a future
recursive-membership ODNI shape change is detectable as a hash
divergence on the canonical file with the denormalized file
unchanged.

Still TODO at PR-1 time: add the two `Taxonomy/**/*.xml` paths to
`[package.include]` in `crates/ism/Cargo.toml` so they ship with the
published crate. The PDF under `Documents/ISMCAT/` stays out of the
include list deliberately (repo-only).

**PDF reference vendored, repo-only (rev3 follow-up, ✅ done):**

```
crates/ism/schemas/ISM-v2022-DEC/Documents/ISMCAT/TetragraphTaxonomy.pdf  (407,807 bytes, sha256 db9a3d5b…)
```

Hash added to `SHA256SUMS` (759 lines). The PDF is **deliberately not
added to `crates/ism/Cargo.toml` `include`** — `include` is an
allowlist, so the PDF auto-excludes from the published crate. This
mirrors the convention documented in `Cargo.toml:14–21` for other
repo-only reference material (Schematron, registers/, etc.) that
inflates the crate past crates.io's 10 MiB limit. The PDF is for
reviewer / auditor use against the in-repo data only.

### 2.3 `build.rs` changes

1. Add `const ISMCAT_TETRA_VERSION: &str = "2022-NOV"` and a
   `verify_ismcat_tetra_version()` function paralleling the existing
   `verify_schema_version()` — reads
   `[package.metadata.marque] ismcat-tetra-version` from `Cargo.toml`,
   panics on mismatch (Constitution Principle IV: schema version pin).

2. Add `parse_tetragraph_taxonomy(path: &Path) -> Vec<TaxEntry>` using
   `quick-xml` (already a build dep). Schema (rev2 — typed against the
   actual XML / XSD):

   ```rust
   #[derive(Debug)]
   struct TaxEntry {
       code: String,                       // <TetraToken>FVEY</TetraToken>
       decomposable: TaxDecomposable,      // Yes | No | Na   (3-state, not 4)
       deprecated: Option<NaiveDate>,      // <Tetragraph deprecated="YYYY-MM-DD"> — always present when NA in V2022-NOV
       last_verified: NaiveDate,           // <Membership dateLastVerified="YYYY-MM-DD"> (required by XSD)
       membership: TaxMembership,
   }

   enum TaxDecomposable { Yes, No, Na }

   /// Mirrors the <Membership> xs:choice exactly.
   enum TaxMembership {
       /// One-or-more <Country>/<Organization>. `recursive == true` if
       /// any <Organization> reference appears (BHTF case in V2022-NOV).
       Members { countries: Vec<String>, organizations: Vec<String>, recursive: bool },
       /// "Refer to original classification authority …" free text.
       Description(String),
       /// <MembershipSupressed/> sentinel. (ODNI spelling.)
       Suppressed,
   }
   ```

   Print `cargo:rerun-if-changed=…/TetragraphTaxonomyDenormalized.xml`.

3. **Remove** `BUILTIN_TETRAGRAPH_MEMBERS` (`build.rs:1456–1473`).
   Replace the `emit_tetragraph_members` body to source rows from
   `parse_tetragraph_taxonomy` filtered to entries where
   `membership` is `TaxMembership::Members { recursive: false, .. }`
   AND `!countries.is_empty()`. (Empirically that's the 24 `Yes` entries
   in V2022-NOV; the BHTF recursive case is excluded.) Append
   `country_extensions.toml` rows after, exactly as PR-B does today.

4. Add `emit_is_decomposable(out: &mut String, &[TaxEntry], &[CountryExtension])`
   (rev2 — corrected against actual data):

   ```rust
   /// Returns:
   /// - `Some(true)`  — taxonomy decomposable=Yes (24 codes in V2022-NOV).
   ///                  In principle also NA + non-recursive Members, but no such
   ///                  entry exists in V2022-NOV (BHTF is recursive).
   /// - `Some(false)` — taxonomy decomposable=No (19 codes — EU, GCCH, KFOR, …).
   /// - `None`        — taxonomy decomposable=NA (deprecated; 18 codes — ISAF, RSMA, MCFI, …);
   ///                   OR code absent from taxonomy (org-fork extensions).
   pub fn is_decomposable(code: &str) -> Option<bool> {
       match code {
           // Yes-decomposable, member list materialized (24)
           "ACGU" | "AMSP" | "ASEA" | "AUSTRALIA_GROUP" | "BWCS" | "CFCK" |
           "CMFC" | "CMFP" | "CPMT" | "CWCS" | "FRME" | "FVEY" | "GMIF" |
           "IMSP" | "MLEC" | "NATO" | "NCFE" | "NSG" | "OSTY" | "PAWA" |
           "PSMX" | "TEYE" | "TFTC" | "UNCK" => Some(true),

           // No-decomposable, atom by authority (19)
           "APFS" | "CLFC" | "CTOC" | "EU" | "GCCH" | "GFNX" | "IMSC" |
           "IPMC" | "IRKS" | "ISSG" | "KFOR" | "MESF" | "MGEU" | "MNTF" |
           "NACT" | "NKIC" | "NRDC" | "RISC" | "SOFP" => Some(false),

           // NA-deprecated (18 codes: ISAF, RSMA, MCFI, BHTF, EUDA, MPFL,
           // PGMF, CFOD, CFUP, AOSC, ECTF, EFOR, GCTF, IESC, MIFH, OSAG,
           // SFOR, SPAA) intentionally fall through to None — they're
           // either suppressed, OCA-deferred, or recursive.

           _ => None,
       }
   }
   ```

   The arms are emitted from the parsed taxonomy at build time, not
   typed by hand — listing them here is illustrative.

   Org extensions are **not** auto-included in the `Some(true)` arm
   even if `country_extensions.toml` declares members — extensions can
   be wrong, misnamed, or stale, and they don't carry ODNI authority.
   An extension with `members = [...]` still resolves correctly through
   `lookup_tetragraph_members`; `is_decomposable` reflects taxonomy
   status only. (Consequence: org extension causes `is_decomposable ==
   None`, which means S005 fires — that's correct; we *do* want the
   operator told that the marking depends on org-local data ODNI didn't
   bless.)

   **NA exposure follow-up (out of scope here):** all 18 NA codes are
   *deprecated* per the XSD and carry a date. A follow-on rule (W### —
   "deprecated tetragraph in active marking") should fire on use, with
   a fix proposal pointing at the chapter-3 remarking aid. Issue #208
   already lists this as out-of-scope for this PR pair; track separately.

   Future extension: a follow-on issue can add a `kind = "membership-shorthand"
   | "organization-atom"` field to extensions per #206's "open
   discriminator" discussion, allowing a deliberate
   `is_decomposable("MNFI") == Some(true)` for sites that have
   verifiable membership data outside ODNI's taxonomy. Out of scope here.

5. Add `emit_tax_provenance(out: &mut String, &[TaxEntry])` emitting a
   parallel `TETRAGRAPH_PROVENANCE` static table with the original
   three-state `decomposable` value (`Yes` / `No` / `NA`), the
   `<Membership>` shape variant (Members / Description / Suppressed /
   Recursive), the `deprecated` date if present, and the
   `dateLastVerified` date. Keeps the audit-traceability that the
   binary `is_decomposable` runtime API collapses (`Yes-with-members`
   and a hypothetical future `NA-with-members` would both map to
   `Some(true)`, but a reviewer can still see which of the two it
   was). Not exposed publicly yet; reserved for the `DecisionRecord`
   work in the 2026-04-20 roadmap.

6. Call `verify_ismcat_tetra_version()` from `main()`; call
   `emit_is_decomposable` and `emit_tax_provenance` from
   `generate_values`.

### 2.4 Build-time data-quality guards (rev3)

The original review flagged a possible asymmetry between
`is_decomposable` and `lookup_tetragraph_members`. With the actual
schema (`<Membership>` is `xs:choice`), the asymmetry is impossible by
construction — every entry has exactly one of {Members, Description,
Suppressed}. But four guard conditions are worth emitting as
`cargo:warning=…` at build time so future taxonomy revisions don't
silently change behavior:

1. **`Yes` with non-Members membership** (Suppressed or Description).
   The XSD allows it; V2022-NOV has zero such entries. If one ever
   appears, that's a substantive policy change worth flagging.
2. **`NA` without `deprecated` attribute**. V2022-NOV has 18 NA entries
   and 18 deprecated entries (1:1). If a future revision decouples
   them, our `NA → None` mapping might need revisiting.
3. **`No` with non-Suppressed membership**. V2022-NOV has zero such
   entries. If one ever appears, atom-by-authority semantics may not
   apply.
4. **`<Organization>` reference in the denormalized file** *outside*
   of NA-deprecated entries (i.e. anywhere except BHTF). V2022-NOV
   has zero such cases — ODNI pre-flattens all `Yes`-decomposable
   entries inline. If a future revision lands an unflattened
   `<Organization>` ref on a `Yes` or `No` code, our build should
   warn so we know to (a) re-vendor against a freshly denormalized
   release, or (b) implement the recursive-fixed-point iterator that
   rev2 incorrectly assumed we'd need.

Warnings, not errors — the build should still succeed against future
revisions; we want signal, not breakage.

### 2.5 `crates/ism/Cargo.toml`

```toml
[package.metadata.marque]
ism-schema-version    = "ISM-v2022-DEC"
ismcat-tetra-version  = "2022-NOV"           # NEW

[package]
include = [
    # ...existing entries...
    "schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/**/*.xml",   # NEW
]
```

### 2.6 `crates/ism/src/lib.rs`

Add to the re-export list:

```rust
pub use generated::values::{
    SCHEMA_VERSION,
    ISMCAT_TETRA_VERSION,         // NEW
    TETRAGRAPH_MEMBERS,
    TRIGRAPHS,
    is_bare_cve_value,
    lookup_tetragraph_members,
    is_decomposable,              // NEW
    // ...existing entries...
};
```

### 2.7 `crates/capco/src/vocab.rs`

1. Add `pub fn is_decomposable_tetragraph(code: &str) -> Option<bool>`
   that delegates to `marque_ism::is_decomposable`. Wrap so capco rules
   never reach across crates for this — keep the dependency arrow
   pointed where the constitution says it should be.

2. The existing test `expand_tetragraph_nato_is_opaque_pass_through`
   becomes obsolete (NATO is now decomposable). Replace with two tests:
   - `expand_tetragraph_nato_returns_members` — `Some(...)`, length > 0.
   - `is_decomposable_eu_returns_false` —
     `is_decomposable_tetragraph("EU") == Some(false)`.

### 2.8 `crates/capco/tests/tetragraph_consolidation.rs`

Add tests covering all three trichotomy branches:

- **Decomposable, known** (`decomposable="Yes"`): FVEY, ACGU, TEYE,
  NATO → `is_decomposable == Some(true)`, `lookup_tetragraph_members`
  returns `Some(non-empty)`.
- **Atom by authority** (`decomposable="No"`): EU, GCCH, KFOR →
  `is_decomposable == Some(false)`, `lookup_tetragraph_members ==
  None`.
- **NA-deprecated** (`decomposable="NA"` with `deprecated="…"`):
  RSMA, ISAF, MCFI → `is_decomposable == None`,
  `lookup_tetragraph_members == None`. Cover all three NA membership
  shapes in fixtures: suppressed (RSMA / ISAF / MCFI), Description /
  OCA-deferral (EUDA), recursive (BHTF).
- **Unknown / absent from taxonomy**: a synthetic 4-letter code in
  neither the taxonomy nor `country_extensions.toml` (e.g.
  `"XYZW"`) → `is_decomposable == None`,
  `lookup_tetragraph_members == None`.
- **Round-trip** §D Table 3 rule 23 example:
  `expected_rel_to([REL TO USA, FVEY], [REL TO USA, GBR]) →
  {USA, GBR}` — the silent-loss case from #183.

### 2.9 Acceptance criteria (#208 verbatim)

Each criterion below maps to a test or build assertion.

1. ✅ `BUILTIN_TETRAGRAPH_MEMBERS` removed; `build.rs` parses the
   taxonomy.
2. ✅ `lookup_tetragraph_members` returns the published member sets
   for FVEY / ACGU / TEYE / NATO / AUSTRALIA_GROUP / NSG.
3. ✅ `lookup_tetragraph_members` returns `None` for NOT DECOMPOSABLE
   (EU / GCCH / KFOR / …).
4. ✅ `is_decomposable(code)` exposes the three-state flag.
5. ✅ `expected_rel_to` and `CapcoScheme::project(Scope::Page, …)`
   produce §D Table 3 rule 23 outputs.
6. ✅ Corpus regression harness updated.
7. ✅ `ismcat-tetra-version = "2022-NOV"` pinned in `Cargo.toml`.

### 2.10 Out of scope (carried over from #208)

- Recursive decomposition (GMIF → {…, NATO} → further). Single-level
  only for v1; tracked as separate issue.
- Deprecated-code remarking aid (chapter 3 mapping for MCFI / ISAF /
  RSMA — "no longer authorized"). The taxonomy data is parsed but not
  exposed via a runtime API yet. Separate rule, separate issue.
- Per-document temporal resolution (`as_of` on `ParseContext`).
  Versioning the taxonomy gives us snapshot-at-marking-date data; the
  consumer plumbing is the temporal-stub work from the 2026-04-20
  roadmap.

---

## 3. Phase 2 — Issue #206: S005 `rel-to-opaque-uncertain-reduction`

### 3.1 Rule ID, severity, and the two-mode question

**ID**: S005. The S-prefix is correct per the suggest-don't-fix
convention shipped in PR #242 (S004 was the precedent). E052 is taken
(`rel-to-no-duplicates`, issue #234 PR-B). The plan-review noted
`fix: None` for S005, which lines up — there's nothing to auto-fix
because we don't know the answer.

**Severity is split, not single.** The plan-review surfaced a
behavioral nuance from the issue discussion: S005's signal value
differs based on whether the *existing* banner is consistent with the
atom-semantics result.

| Banner state | Atom-semantics consistency | Severity | Rationale |
|---|---|---|---|
| Banner present | Consistent (existing banner == atom-semantics intersection, or atom-semantics ⊆ existing banner) | **Info** | Producer plausibly knew what they were doing; trust but flag. Rare false-positive cost is high if this is a Suggest. |
| Banner present | Inconsistent | **Suggest** | Producer's banner can't be reconciled with portions under either atom or expanded semantics. Operator should look. |
| Banner absent (we'd be computing it) | n/a | **Suggest** | Active validation context. We *don't* know the answer; tell the operator. |
| Single portion (no intersection to compute) | n/a | skip | Nothing to surface. |

The decision data lives in `RuleContext.page_context` (already plumbed
for E035 / banner roll-up rules). The rule reads both `expected_rel_to()`
(atom-semantics) and the actual banner candidate to make the call.

`Severity::Off` configurability is preserved as always — operators can
silence S005 entirely via `.marque.toml` if a particular site's policy
is to trust producers wholesale.

### 3.2 Trigger logic

```text
for each banner / CAB candidate:
  pc = ctx.page_context()  // None → skip
  portions_with_rel_to = pc.portions where rel_to is non-empty
  if |portions_with_rel_to| < 2: skip                             // no intersection

  expected = pc.expected_rel_to()                                 // atom semantics
  actual_banner_rel_to = parse_banner_rel_to(this candidate)      // Option<Set<Code>>

  for each unique code X across portions where is_decomposable(X) == None:
      // X is uncertain. Did it survive intersection?
      X_in_every_portion = portions_with_rel_to.all(|p| p.rel_to.contains(X))
      if X_in_every_portion: continue                             // X survived — fine

      // X dropped. Are there OTHER codes the operator might care about
      // (because X's hypothetical membership might have included them)?
      other_codes = union(portions_with_rel_to.rel_to) − {X} − atoms_known_outside_X
      if other_codes is empty: continue                           // no candidates; nothing to surface

      // Pick severity
      severity = match actual_banner_rel_to {
          None                                            => Suggest,  // active validation
          Some(b) if b == expected                        => Info,     // banner matches atom-semantics
          Some(b) if expected ⊆ b                         => Info,     // banner is supersets atom-semantics; operator may have used external data
          Some(_)                                         => Suggest,  // inconsistent
      };

      emit Diagnostic { rule: "S005", severity, fix: None,
                        span: candidate.span,
                        message: build_message(X, other_codes, expected, actual_banner_rel_to),
                        citation: "CAPCO-2016 §H.8 / ISMCAT 2022-NOV ch.2" }
```

### 3.3 Diagnostic message (rev2)

The issue's example message is the right shape. Codify in
`build_message`:

```text
S005: REL TO membership-uncertain reduction
  Code `{X}` does not have an authoritative member list (ODNI ISMCAT
  taxonomy: {state}). Atom-semantics intersection produced
  REL TO {expected_set}, but {X}'s hypothetical membership may include
  {other_codes} from other portions.

  If `{X}` includes {GBR}, the banner should be:
      {classification}//REL TO USA, GBR
  If `{X}` excludes {GBR}, the banner is:
      {classification}//{NOFORN or atom-result}

  Resolution: (a) add `{X}` membership to country_extensions.toml with
  authoritative source citation, or (b) revise the marking to use codes
  with known membership.
```

`{state}` is one of:

- `"deprecated, membership suppressed (NA-Suppressed in V2022-NOV)"`
  — the 14 NA-suppressed codes. Most common case (ISAF, RSMA, MCFI, …).
- `"deprecated, refer to original classification authority"` — the 3
  NA-Description codes (EUDA, MPFL, PGMF). Diagnostic should quote the
  taxonomy's `<Description>` text verbatim since it's already
  classification U/USA-marked and points the operator at next steps.
- `"deprecated, recursive membership (out of scope for v1)"` — BHTF.
- `"absent (org-fork extension or unknown code)"` — codes not in the
  taxonomy at all.

The `{state}` text is selected from `TETRAGRAPH_PROVENANCE`, not derived
at runtime — keeps the diagnostic stable across taxonomy revisions and
keeps the `is_decomposable` runtime API single-purpose.

**Constitution V audit-content-ignorance applies**: the diagnostic emits
canonical code strings, span offsets, classification canonical names,
and the verbatim taxonomy `<Description>` text (which is itself an
ODNI-classified payload, not user-document content). No surrounding
document text or other portion content is reproduced.

### 3.4 Files changed

```
crates/capco/src/rules.rs               (MODIFY — register and implement S005)
crates/capco/tests/rel_to_invariants.rs (MODIFY — add S005 test cases)
```

### 3.5 Tests

Five cases at minimum — all five already implied by §3.2:

1. **Suggest fire (active validation)**: two portions, one has org-fork
   `MNFI` (not in taxonomy), other has different trigraphs, no banner
   → `Severity::Suggest`.
2. **Info fire (consistent banner)**: same portions as (1) but banner
   present and equals `REL TO USA` (the atom-semantics result) →
   `Severity::Info`.
3. **Suggest fire (inconsistent banner)**: same portions as (1) but
   banner reads `REL TO USA, GBR, FRA` (cannot derive from atom
   semantics) → `Severity::Suggest`.
4. **No fire — uncertain code in all portions**: `MNFI` appears in every
   portion, survives atom intersection → no diagnostic.
5. **No fire — atom by authority**: portions reference `EU`
   (`is_decomposable == Some(false)`) → no diagnostic. Same for `KFOR`.
6. **No fire — decomposable known**: portions reference `FVEY` only →
   intersection computed correctly, no diagnostic.
7. **No fire — single portion**: only one portion with REL TO → skip.

The `capco_rule_set_registers_all_rules` test gets
`assert!(ids.contains(&"S005"));`.

### 3.6 Rule registration

In `CapcoRuleSet::new()`, after `RelToNoDuplicatesRule` (E052):

```rust
// #206 — S005: REL TO membership-uncertain reduction.
// Severity is selected dynamically based on banner consistency
// with atom-semantics; see RelToOpaqueUncertainReductionRule::check.
Box::new(RelToOpaqueUncertainReductionRule),
```

---

## 4. Data Flow

```
Build time (build.rs):
  Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml
    └──► parse_tetragraph_taxonomy() ──► Vec<TaxEntry>  (4-state decomposability preserved)
                                            │
                                            ├──► emit_tetragraph_members()  (Yes/NA + non-empty)
                                            │      └─► TETRAGRAPH_MEMBERS, lookup_tetragraph_members
                                            │
                                            ├──► emit_is_decomposable()
                                            │      └─► is_decomposable() — three-state
                                            │
                                            └──► emit_tax_provenance()
                                                   └─► TETRAGRAPH_PROVENANCE — full 4-state, deprecated, last_verified

  country_extensions.toml ──► load_country_extensions() ──► appended to TETRAGRAPH_MEMBERS only
                                                            (NOT to is_decomposable arms)

Runtime (S005 in lint):
  IsmAttributes (per portion) ──► rel_to: Box<[CountryCode]>
  PageContext                  ──► all portions' rel_to + expected_rel_to() (atom)
                               ──► banner candidate parse (when present)
  marque_ism::is_decomposable(code) ──► Some(true) | Some(false) | None
                       │
                       ▼ (code in rel_to, is_decomposable == None, dropped from intersection,
                          AND there are 'other codes' to surface)
       choose severity from (banner present? consistent?)
       Diagnostic { rule: "S005", severity: Info|Suggest, fix: None, … }
```

---

## 5. Verification

```bash
# Phase 1 (#208)
cargo build -p marque-ism
cargo test -p marque-ism                                   # generated tests
cargo test -p marque-capco tetragraph                      # tetragraph_consolidation
# Spot check
echo 'fn main() { println!("{:?}", marque_ism::is_decomposable("EU")); }' \
  | rustc - --edition 2024 -L target/debug/deps -o /tmp/x && /tmp/x
# expect: Some(false)

# Phase 2 (#206)
cargo test -p marque-capco rules::tests::s005

# Full workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings

# Regression guards
cargo test -p marque-capco scheme_equivalence
cargo test -p marque-capco tetragraph_consolidation
cargo test -p marque-capco rel_to_invariants
cargo test -p marque-capco corpus_parity                   # SC-002 / SC-004 gates

# WASM parity (SC-008)
wasm-pack build crates/wasm --target web --profile release
# (corpus parity harness consumes this — runs separately)
```

---

## 6. PR sequencing

Two PRs, sequential:

- **PR-1 (#208)**: vendoring + build.rs + `is_decomposable` + tests.
  Lands first. Reviewable in isolation: the API is exposed but no rule
  uses it yet beyond consolidation tests.
- **PR-2 (#206)**: S005 rule + registration + tests. Depends on PR-1's
  `is_decomposable`. Reviewable as "given the discriminator, here's
  the diagnostic."

Each PR ships its own corpus regression harness update so CI catches
silent loss between merges.

---

## 7. Critical files (line refs as of HEAD)

| File | Purpose |
|---|---|
| `crates/ism/schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml` | NEW — vendored authoritative source. |
| `crates/ism/build.rs:1456–1473` | Remove `BUILTIN_TETRAGRAPH_MEMBERS`. |
| `crates/ism/build.rs:1856–1939` | `emit_tetragraph_members` rewrites to consume `parse_tetragraph_taxonomy` output. |
| `crates/ism/build.rs` (new section) | `parse_tetragraph_taxonomy`, `emit_is_decomposable`, `emit_tax_provenance`, `verify_ismcat_tetra_version`. |
| `crates/ism/src/lib.rs:35–38` | Re-export `is_decomposable`, `ISMCAT_TETRA_VERSION`. |
| `crates/ism/Cargo.toml` | `ismcat-tetra-version` metadata; `Taxonomy/**/*.xml` in include. |
| `crates/capco/src/vocab.rs:71` | Update / replace NATO opacity test; add `is_decomposable_tetragraph` delegate. |
| `crates/capco/src/rules.rs:80–197` | Register `RelToOpaqueUncertainReductionRule` in `CapcoRuleSet::new()`. |
| `crates/capco/src/rules.rs` (after E052 region, ~3195) | Implement S005 with two-mode severity selection. |
| `crates/capco/tests/tetragraph_consolidation.rs` | Add `is_decomposable` tests covering all three trichotomy branches. |
| `crates/capco/tests/rel_to_invariants.rs` | Add S005 fire / no-fire / Info-vs-Suggest tests. |

---

## 8. Open questions

1. ~~**Recursive decomposition**~~ — **resolved (rev3)**. ODNI
   denormalizes all `Yes`-decomposable codes inline in the
   denormalized file. Only BHTF (NA-deprecated) carries an
   `<Organization>` ref; `NA → None` already covers it. No recursion
   code, no follow-on issue. Defensive build warning landed in §2.4
   guard #4 to catch future taxonomy-shape regressions.
2. **`country_extensions.toml` shadowing**: if an extension declares a
   different member list than the taxonomy for the same code, what
   happens? Recommend: build error, force operator to either (a) rename
   the extension to a non-conflicting code or (b) use a future
   `override = true` opt-in. Tracked in #208 acceptance criterion 3
   ("rejected at build time — TBD"). Decide before PR-1 merges.
3. **Banner consistency check in S005 §3.2**: the consistency comparison
   uses set equality / containment on `{atoms-only}`. Is that the right
   primitive when the banner contains tetragraphs that themselves
   decompose? Probably yes (we're comparing post-expansion sets), but
   worth a cross-check with the rollup XSL behavior before merge.
4. ~~**Vendor `TetragraphTaxonomy.xml` alongside the denormalized form?**~~
   — **resolved (rev3, ✅ done)**. Both XML files vendored at
   `crates/ism/schemas/ISM-v2022-DEC/Taxonomy/ISMCAT/`, hashes added
   to SHA256SUMS. Build parses only the denormalized form; canonical
   form retained for provenance and divergence detection.
5. ~~**Vendor `TetragraphTaxonomy.pdf` for human reference?**~~ —
   **resolved (rev3, ✅ done)**. PDF vendored at
   `crates/ism/schemas/ISM-v2022-DEC/Documents/ISMCAT/TetragraphTaxonomy.pdf`
   (407,807 bytes, sha256 `db9a3d5b…`); hash recorded in
   SHA256SUMS. Deliberately NOT added to `[package.include]` so it
   stays repo-only and does not bloat the published crate.

