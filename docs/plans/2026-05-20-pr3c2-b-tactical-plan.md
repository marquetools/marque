# PR 3c.2.B — Tactical Implementation Plan (historical record)

**Date**: 2026-05-20
**Branch**: `refactor-006-pr-3c2-b-call-site-migration`
**Status**: Superseded by the binding PM contract at `docs/plans/2026-05-20-pr3c2-b-pm-decisions.md`.

## Note (added 2026-05-20 by PM at B6 closure)

The Plan-agent preflight pass that authored this plan returned its full content inline rather than writing to this file. The PR 3c.2.B PM contract at `docs/plans/2026-05-20-pr3c2-b-pm-decisions.md` consolidates the tactical decisions (5-commit sequence in PM-B-6, site-by-site classification reflected in PM-B-8, HRTB placement in PM-B-5, byte-equivalence test design in PM-B-10). This file exists as a redirect so future readers chasing the cross-references from the PM contract or the deferred-findings register land on the right document.

## Key tactical findings (from the Plan-agent preflight)

The Plan-agent's analysis converged with the system-architect preflight on the following load-bearing items:

- **Site count baseline**: 14 production code call sites + 16 external `tests/` sites = 30 lines containing `marque_ism::from_parsed_unchecked(`. Per-site classification subsequently split into 26 MIGRATE + 5 CARVE-OUT (the 30th line is the in-`src/` test helper at `crates/core/src/parser.rs:3890`, a carve-out under PM-B-2; the apparent 26+5=31 reconciles with `render_canonical_properties.rs:50` being absent from the preflight Appendix A but present at implementation-grep time — see §1 erratum in the PM contract).
- **Override placement** at `crates/capco/src/scheme/marking_scheme_impl.rs` adjacent to GAT bindings (PM-B-1).
- **Doc-comment split** 5 MIGRATE-NOW / 8 DEFER-to-3c.2.E (PM-B-4).
- **HRTB smoke test** at `crates/scheme/tests/hrtb_smoke.rs` (PM-B-5).
- **5-commit sequence** B1 override + HRTB → B2 engine production → B3 WASM + engine in-src tests → B4 external tests → B5 doc-comment sweep + closeout (PM-B-6).
- **R-B6 carve-out** for `crates/core/{src,tests}/` sites blocked by Constitution VII directionality (PM-B-2).
- **R-B7 carve-out** for `s004_audit_content_ignorance.rs` (already `#![cfg(any())]`-disabled; PM-B-7).

For the binding contract, the operative reference is the PM contract document. For the preflight reasoning, see also `docs/plans/2026-05-20-pr3c2-b-architect-preflight.md`.

## Lessons for future preflight cycles

- **The Plan-agent's "Output a tactical implementation plan as a markdown document under …" brief was insufficient to ensure file write**: the agent returned the content inline and skipped the file write. Future preflight briefs should explicitly require the Write tool call before returning, AND the PM should `ls` the expected output file before declaring preflight complete.
- This file is the post-hoc record of that drift; future C/D/E PRs should not repeat this pattern.
