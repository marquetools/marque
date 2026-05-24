<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3c.B Commit 3 — Byte-identity baseline NDJSON

These NDJSON files are the pre-PR-3c `marque fix` audit output for the
three beachhead-rule fixtures (E054 × 2, E057 × 1). They are the
"golden" inputs to the byte-identity acceptance gate in
`tests/byte_identity_pr3c.rs`: after the Commit 3 migration, the
engine's audit output for these same inputs MUST be byte-for-byte
identical to what is stored here.

**Path C invariant.** Per the consolidated plan
(`docs/plans/2026-05-10-pr3c-consolidated-plan.md` lines 100–175),
the audit-record JSON shape does NOT change in commits 2–9. Migrated
rules emit both `Diagnostic.fix` (legacy projection, byte-identical
to pre-migration) AND `Diagnostic.fix_intent` (new structural
intent). The engine pairs them at promotion time; the
`AppliedFixProposal::New { intent, synthesized }` variant's Deref
returns `&synthesized`, which carries the pre-migration FixProposal
verbatim, so the NDJSON serializer reads byte-identical values.

If this gate fails, **STOP** — do not regenerate the snapshot. A
failure means either (a) the synthesized projection on a migrated
rule diverged from the pre-migration FixProposal, or (b) Commit 5's
renderer produced different canonical bytes than the pre-PR-3c
FixProposal layout. Both are real defects, not snapshot drift.

## Baseline capture procedure

Each fixture was captured at merge-base `30e11b0d` (the pre-PR-3c.A
HEAD on `006-engine-rule-refactor`, equal to the staging merge before
PR 3c.A's Commit 1 landed):

```
git checkout 30e11b0d
cargo build -p marque
for fixture in e054_simple e054_multi e057_simple; do
    INPUT=...  # see "Fixtures" below for each
    echo "$INPUT" | \
        MARQUE_ALLOW_FIXED_CLOCK=1 \
        target/debug/marque fix - \
            --format json \
            --fixed-timestamp "2026-05-10T12:00:00Z" \
        2> "crates/capco/tests/fixtures/pr3c_baseline/${fixture}.ndjson" \
        > /dev/null
    # strip the "-: applied N fix(es)" narration line:
    grep '^{' "crates/capco/tests/fixtures/pr3c_baseline/${fixture}.ndjson" > tmp \
        && mv tmp "crates/capco/tests/fixtures/pr3c_baseline/${fixture}.ndjson"
done
```

After PR 3c.A merges, byte-identity for these three was independently
verified against current HEAD `6e3d7861` (after Commits 1, 2, 4, 5)
— none of those commits touched E054 / E057 emission, so the
baselines apply to both pre-PR-3c.A and post-Commit-5 starting
points.

## Fixtures

- **`e054_simple.ndjson`** — input `(S//NF/RELIDO)`. RELIDO is the
  last dissem token; the fix consumes the preceding `/`. Post-fix
  output: `(S//NF)`. Single E054 audit record.

- **`e054_multi.ndjson`** — input `(S//NF/IMC/RELIDO)`. RELIDO is the
  last dissem token in a three-token block. Same fix shape as
  `e054_simple`, but the post-fix sibling block has more tokens —
  exercises the renderer's sibling preservation under fix application.
  Post-fix output: `(S//NF/IMC)`. Single E054 audit record.

- **`e057_simple.ndjson`** — input `(S//OC-USGOV/RELIDO)`. RELIDO is
  the last dissem token following the ORCON-USGOV asserting template.
  Post-fix output: `(S//OC-USGOV)`. Single E057 audit record.

E021 has no pre-PR-3c baseline because pre-PR-3c E021 was
`Severity::Error` with no fix — no audit record was emitted. PR 3c.B
Commit 3 flips it to `Severity::Fix` with `FactAdd { NOFORN }`; the
byte-identity gate is therefore vacuous for E021 and the test
documents this. E021's correctness is exercised by the per-rule
shape tests in the migration test file instead.
