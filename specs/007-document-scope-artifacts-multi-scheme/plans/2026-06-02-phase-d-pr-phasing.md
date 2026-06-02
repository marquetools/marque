# Phase D — CAB decoupling: PR-phasing plan (T040–T045)

> Authored 2026-06-02 by the Phase-D phasing architect review. Drives the rest of Phase D.
> All `file:line` refs verified against worktree `007-phase-d` (base `654a281b`; Phases 0/A/B/C/H merged).

## 0. Executive summary

The largest risk is **not** the `DeclassInstruction` lattice (self-contained, WASM-safe) — it is the
**engine CAB-routing rewrite**. CAB candidates today flow through the *identical per-candidate
rule-dispatch loop* as portions/banners (`dispatch_rules_for_marking`,
`crates/engine/src/engine/lint_helpers.rs:254`): canonicalized (`lint_helpers.rs:206`), projected,
run through every rule. Phase D must **intercept CAB candidates upstream** and route them to
artifact-node production — an engine-orchestration change to the hot lint loop, a different blast
radius than the leaf field move. So D2 (field move) is split from D3 (engine routing).

**Three confirmed decisions (resolve before implementing):**

1. **`token_spans` is NOT CAB-only — leave it on `CanonicalAttrs`.** Read by ~30 capco rule files,
   the recognizer, and the decoder. The data-model T041 line (`data-model.md:229`) is a defect.
2. **The decoder's CAB completeness / non-triviality signal must be retained as a boolean.**
   `is_nontrivial_marking` (`shape.rs:269-272`) + `strict_parse_is_complete` (`shape.rs:315-320`)
   read the CAB field disjunction. Replace with `Cab::is_nontrivial(&self) -> bool`.
3. **`declassify_on` / `declass_exemption` are dual-role** — genuine CAB content *and* present on
   portion/banner markings (read by `DeclassifyMisplacedRule` `text_handling.rs:155`; seed the page
   rollup at `marking.rs:629-630`). They **cannot move wholesale into `Cab`**; the `Cab` payload
   carries its *own* `DeclassInstruction` from the CAB `Declassify On:` line — a distinct inbound
   edge into the declassify-on node (data-model:241).

## 1. PR breakdown (five PRs)

| PR | Tasks | Crates | Why |
|----|-------|--------|-----|
| **D1** | T043 `DeclassInstruction` + `OrdMax<DeclassInstruction>` value type | `marque-capco` (+ maybe `marque-scheme`) | Lattice-shaped, WASM-safe, self-contained, no pivot edits. Lattice-consultant-gated. |
| **D2** | T040 + T041 (define `Cab`; `ArtifactPayload = Cab`; move genuine CAB-only fields off `CanonicalAttrs`; delete `projected.rs` null-out; update readers) | `marque-ism`, `marque-capco`; ripple `marque-engine`, `marque-wasm`, `marque-core` | Breaking pivot change as a pure data-shape migration; `parse_cab` still produces a marking. |
| **D3** | T042 (`parse_cab`→node) + engine CAB-routing interception + decoder CAB-shape finalization + declassify-on node/edges | `marque-core`, `marque-engine` | Semantic pipeline change: CAB stops emitting marking diagnostics, becomes a `DocumentArtifact`. |
| **D4** | T044 forward-evaluable serializer | `marque-capco`; retires WASM `generate_cab_native` | Build a `Declassify On` line from state. |
| **D5** | T045 SC-001 type-level + CAB→node test; SC-008a bench gate | `marque-engine`, `marque-ism` | Verification. |

D1 ships only the **value type** (+ §E.3 oracle fixtures); the declassify-on **node** (DocumentArtifact
+ inbound edges) wires in D3 where the engine artifact path lives. T040+T041 stay atomic (defining
`Cab` without removing the fields it subsumes leaves two homes for the same data).

## 2. Dependency graph

```
D1 (value type) → D2 (Cab payload + field move) → { D3 (parse_cab+routing)  ∥  D4 (serializer) } → D5 (verify)
```

Serial spine D1 → D2 → {D3 ∥ D4} → D5. D3 (core+engine) and D4 (capco serializer) touch disjoint
files; either order once D2 fixes the `Cab` shape. Solo-driven: "concurrent" = "no rebase coupling".

## 3. Blockers & decisions

- **B1 (CRITICAL, data-model defect):** `token_spans` stays on `CanonicalAttrs`/`ParsedAttrs`.
  Amend the data-model T041 line.
- **B2 (CRITICAL, cross-PR seam):** add `Cab::is_nontrivial(&self) -> bool`; replace decoder's
  four-field disjunction. Preserve "isolated authority block stands on its own" (shape.rs:300-302).
- **B3 (CRITICAL, load-bearing):** keep portion-level `declassify_on`/`declass_exemption` on the
  canonical; `Cab` carries its own `DeclassInstruction`. Portion declassify feeds the `Rollup`
  edge; CAB-line declassify feeds the structural edge — T043's multi-inbound design. Genuinely
  CAB-only removed fields reduce to `classified_by`/`derived_from` (free text) + the CAB *parsed
  instruction* into the payload. `ProjectedMarking.declassify_on` stays.
- **B4 (lattice):** `OrdMax<T>` gives Join+Meet but not `BoundedJoinSemilattice` — impl on a local
  `DeclassifyOnLattice(OrdMax<DeclassInstruction>)` newtype (orphan rules). `IsmDate` has no total
  `Ord` (`date.rs:21-25`) — hand-written `DeclassInstruction::Ord` keys date tiers on
  `IsmDate::end_cmp`. **Consult `marque-lattice-consultant` in the D1 plan.**
- **B5 (engine semantics):** enumerate rules firing on `MarkingType::Cab` (`DeclassifyMisplacedRule`
  skips CAB; `nodis_exdis.rs:146` gates `Banner|Cab`). D3 decides fate: subsumed by artifact
  resolution or retained as document-scope rules reading `Cab` via `DocumentContext`. Corpus +
  G13 are the safety net.

## 4. Risk register (summary)

- **Latency (SC-001/SC-008a):** LOW-MODERATE. Phase D removes per-CAB rule dispatch; reuse cached
  scheduler order (`scheduler.rs:20-22`); C3 already schedules `DerivationEdge`s. D5 owns the number;
  noise-band failures → `gh run rerun --failed` before treating as real.
- **WASM (Constitution III):** LOW. `Cab` + `DeclassInstruction` WASM-safe; verify `wasm-pack` after D2/D4.
- **Audit / G13:** LOW tripwire. `classified_by`/`derived_from` held by `Cab` are the parsed marking
  value threaded to rules, NOT audit-adjacent. Move part5.rs:391 sentinel onto `Cab`. Re-run canary D3/D5.
- **Stable-API / audit-schema:** LOW — no bump forced. `CanonicalAttrs` is the pivot, not frozen.
  `MARQUE_AUDIT_SCHEMA` stays `marque-3.2`; D5 asserts unchanged.
- **Source fidelity (Constitution VIII):** verification-gated. §E.3 p32, §E.4/§E.5 p33 confirmed in
  the citation index; re-verify every tier-ordering claim against `crates/capco/docs/CAPCO-2016.md`.

## 5. Open questions (decide early, not blockers)

- **`Cab` home:** recommend `marque-capco` (carries §E vocabulary). Binding stays in capco artifacts.rs.
- **`PresentNonCanonical` payload (malformed declassify):** structural "unparseable" marker + `Span`,
  never raw bytes in audit-reachable position.
- **Multi-page CAB:** declassify-on node aggregates at `Scope::Document`, not per-page.

## 6. Sequencing checklist

1. D1 (lattice-consultant-gated): resolve B4; value type + §E.3 oracle fixtures.
2. D2: resolve B1/B3/B2; define `Cab`, move CAB-only fields, `ArtifactPayload = Cab`, move G13 sentinel; full corpus + G13.
3. D3: resolve B5; `parse_cab`→node, engine routing, decoder finalization, declassify-on node + edges; full corpus + G13 + decoder-accuracy.
4. D4 (may interleave with D3): serializer; retire WASM generator; defer §E.4/§E.5 string selection to Phase G.
5. D5: SC-001 type-level + CAB-node test; SC-008a bench gate; assert audit-schema unchanged.

Constitution IV: Phase D editing engine crates is permitted — it is engine work, not a scheme-adoption PR.
