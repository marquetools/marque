#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

"""
Rule-ID 2-tuple corpus fixture rewrite — one-shot mechanical migration.

Walks `tests/corpus/**/*.expected.json` (and `*.expected_fix.json`) and
rewrites every flat-string `"rule": "E007"`-style entry into the
structured 2-tuple shape:

    {"rule": {"scheme": "capco", "predicate_id": "portion.metadata.x-shorthand-date-pattern"}, ...}

Authority:
- `docs/refactor-006/legacy-rule-id-map.md` (the lookup table)

Constitution III: out-of-workspace tooling — this script lives in `tools/`,
not a workspace crate.

# Preservation strategy

The rewrite is **text-level**, not structural-reserialization. Each fixture
has its own hand-curated formatting (compact one-line diagnostics in
`invalid/*.expected.json`, expanded multi-line in `*.expected_fix.json`,
prose `_note` fields, ordering of keys). A structural pass through
`json.loads` + `json.dumps` would normalize every fixture's whitespace
shape — a churnful side effect the rewrite does not need.

Instead we find each `"rule": "<LEGACY>"` occurrence in source order and
replace it in place with the wire-string-equivalent structured object,
keeping all surrounding whitespace, key ordering, and per-line layout
intact. The pre-rewrite JSON parses; the post-rewrite JSON parses; the
formatting between the two is byte-identical except at the rewritten
slots.

# Idempotency

A second run is a no-op: the regex matches only `"rule": "<LEGACY-ID>"`
(string value form), not `"rule": {...}` (object form). The script
treats already-migrated entries (object form) as a successful no-op.

# Special cases — E058 / E059

Bare `E058` / `E059` in fixtures encode the registered walker ID. After
the migration, the engine's constraint-catalog bridge becomes a no-op
pass-through and the catalog row's name surfaces as the predicate ID. Each
fixture's specific per-row predicate is encoded in PER_FIXTURE_OVERRIDES
(hand-derived from the fixture's `_note` field, cross-checked against
§3 (class-floor) and §4 (sci-per-system) of the legacy-rule-id-map).

Usage:
    python3 tools/migrate-corpus-rule-ids.py [--dry-run] [--verbose]

Exit code 0 on success, non-zero on any rule ID not in the map (a STOP
condition that should be flagged to PM).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Iterator


# ---------------------------------------------------------------------------
# Lookup table — derived from `docs/refactor-006/legacy-rule-id-map.md`
# ---------------------------------------------------------------------------
LEGACY_TO_PREDICATE: dict[str, tuple[str, str]] = {
    # §1 — Active CAPCO rules
    "C001": ("capco", "marking.correction.token-typo"),
    "E002": ("capco", "portion.dissem.rel-to-missing-usa"),
    "E005": ("capco", "portion.declassification.declassify-on-misplaced"),
    "E006": ("capco", "marking.deprecation.deprecated-dissem-control"),
    "E007": ("capco", "portion.metadata.x-shorthand-date-pattern"),
    "E008": ("capco", "marking.metadata.unrecognized-token"),
    "E039": ("capco", "page.dissem.nodis-exdis-clears-banner-rel-to"),
    "E041": ("capco", "portion.dissem.nodis-supersedes-exdis-in-portion"),
    "E061": ("capco", "portion.sci.hcs-bare-at-confidential-legacy-remark"),
    "E062": ("capco", "portion.sci.hcs-bare-suggest-subcompartment"),
    "E063": ("capco", "portion.sci.rsv-bare-requires-compartment"),
    "E064": ("capco", "portion.dissem.eyes-only-convert-to-rel-to"),
    "E065": ("capco", "portion.sci.deprecated-long-form"),
    "E066": ("capco", "marking.recanonicalize.legacy-nato-compound"),
    "E067": ("capco", "marking.recanonicalize.bare-canonical-compound"),
    "E071": ("capco", "portion.fgi.fgi-explicit-with-trigraph"),
    "E072": ("capco", "page.dissem.bare-rel-portion-divergence"),
    "S003": ("capco", "portion.classification.joint-usa-first-style"),
    "S004": ("capco", "portion.dissem.rel-to-trigraph-suggest"),
    "S005": ("capco", "page.dissem.rel-to-uncertain-reduction"),
    "S007": ("capco", "portion.nato.bare-nato-requires-rel-to-usa-nato"),
    "S008": ("capco", "portion.dissem.relido-implied-by-closure"),
    "S009": ("capco", "page.dissem.prefer-tetragraph-collapse"),
    "S010": ("capco", "page.dissem.collapse-uniform-rel-portions"),
    "W003": ("capco", "page.dissem.non-ic-dissem-in-classified-banner"),
    "W004": ("capco", "page.fgi.joint-disunity-collapses-to-fgi"),
    "W034": ("capco", "portion.sci.unpublished-custom-control"),

    # §2 — Retired declarative-wrapper rules (bridge-routed).
    "E010": ("capco", "portion.sci.hcs-system-constraints"),
    "E012": ("capco", "portion.classification.dual-classification"),
    "E014": ("capco", "portion.classification.joint-requires-rel-to-coverage"),
    "E015": ("capco", "portion.classification.non-us-requires-dissem"),
    "E016": ("capco", "portion.classification.joint-conflicts-restricted"),
    "E021": ("capco", "portion.aea.rd-frd-requires-noforn"),
    "E024": ("capco", "portion.aea.rd-precedence"),
    "E036": ("capco", "portion.classification.joint-conflicts-hcs"),
    "E037": ("capco", "portion.dissem.nodis-conflicts-exdis"),
    "E038": ("capco", "portion.dissem.nodis-or-exdis-requires-noforn"),
    "E053": ("capco", "portion.dissem.noforn-conflicts-rel-to"),
    "E054": ("capco", "portion.dissem.relido-conflicts-noforn"),
    "E055": ("capco", "portion.dissem.display-only-clears-relido"),
    "E056": ("capco", "portion.dissem.orcon-clears-relido"),
    "E057": ("capco", "portion.dissem.orcon-usgov-clears-relido"),

    # §5 — Banner-rollup walker per-row IDs (E031 = SAR roll-up).
    "E031": ("capco", "banner.banner-rollup.sar-portions-roll-up"),
    "E035": ("capco", "banner.banner-rollup.sci-portions-roll-up"),
    "E040": ("capco", "banner.banner-rollup.non-ic-dissem-roll-up"),
    "E068": ("capco", "banner.classification.mismatch-vs-projected"),
    "E069": ("capco", "banner.fgi.marker-mismatch-vs-projected"),

    # §6 — Other walker / declarative-emit-only rules.
    "E070": ("capco", "portion.aea.frd-tfni-precedence"),
    "W005": ("capco", "portion.classification.rel-to-not-in-joint-coverage"),

    # §7 — Engine sentinels (scheme "engine", reserved per OD-4).
    "R001": ("engine", "recognition.decoder-recognized"),
    "R002": ("engine", "fix.reparse-failed"),

    # Retired rules referenced by stale fixtures / docs.
    # E001 + E003 (MisorderedBlocksRule retirement) — both absorbed
    # into MarkingScheme::render_canonical.
    # The rename targets keep the audit shape valid for any historical
    # fixture that still carries the legacy ID.
    "E001": ("capco", "banner.classification.portion-mark-in-banner"),
    "E003": ("capco", "portion.classification.misordered-blocks"),
}

# E058 + E059 per-fixture row resolution. Each entry's list provides
# (legacy_id, predicate) in the SAME ORDER as the diagnostics appear in
# the fixture (positional). Authority for each row: the fixture's `_note`,
# verified against §3 (class-floor) / §4 (sci-per-system) of the legacy
# map.
PER_FIXTURE_OVERRIDES: dict[str, list[tuple[str, tuple[str, str]]]] = {
    "invalid/aea_rd_sg_below_secret_floor.expected.json": [
        ("E021", ("capco", "portion.aea.rd-frd-requires-noforn")),
        ("E058", ("capco", "banner.aea.floor-rd-sg")),
    ],
    "invalid/dissem_orcon_unclassified.expected.json": [
        ("E058", ("capco", "banner.dissem.floor-orcon")),
    ],
    "invalid/sar_unclassified.expected.json": [
        ("E058", ("capco", "banner.classification.floor-sar")),
    ],
    "invalid/dissem_rsen_below_secret.expected.json": [
        ("E058", ("capco", "banner.dissem.floor-rsen")),
    ],
    "invalid/nato_bohemia_low_class.expected.json": [
        ("S007", ("capco", "portion.nato.bare-nato-requires-rel-to-usa-nato")),
        ("E015", ("capco", "portion.classification.non-us-requires-dissem")),
        ("E058", ("capco", "banner.classification.floor-bohemia")),
    ],
    "invalid/sci_unclassified_si.expected.json": [
        ("E058", ("capco", "banner.classification.floor-si")),
    ],
    "invalid/aea_atomal_below_classified_floor.expected.json": [
        ("E058", ("capco", "banner.aea.floor-atomal")),
    ],
    "invalid/aea_dod_ucni_classified.expected.json": [
        ("E058", ("capco", "banner.aea.ceiling-dod-ucni")),
    ],
    "invalid/dissem_eyes_only_low_class.expected.json": [
        ("E064", ("capco", "portion.dissem.eyes-only-convert-to-rel-to")),
        ("E058", ("capco", "banner.dissem.floor-eyes-only")),
    ],
    "invalid/banner_abbrev_3.expected.json": [
        ("E058", ("capco", "banner.dissem.floor-imcon")),
    ],
    "invalid/aea_doe_ucni_classified.expected.json": [
        ("E058", ("capco", "banner.aea.ceiling-doe-ucni")),
    ],
    "invalid/aea_tfni_below_classified_floor.expected.json": [
        ("E058", ("capco", "banner.aea.floor-tfni")),
    ],
    "invalid/sci_hcs_p_sub_missing_orcon.expected.json": [
        ("E010", ("capco", "portion.sci.hcs-system-constraints")),
        ("E059", ("capco", "marking.sci.hcs-p-noforn-required")),
        ("E059", ("capco", "marking.sci.hcs-p-sub-companions")),
    ],
    "lattice/sci-cross-system.expected.json": [
        ("E059", ("capco", "marking.sci.si-g-companions")),
        ("E059", ("capco", "marking.sci.tk-compartment-noforn-required")),
    ],
    "invalid/sci_hcs_p_missing_noforn.expected.json": [
        ("E010", ("capco", "portion.sci.hcs-system-constraints")),
        ("E059", ("capco", "marking.sci.hcs-p-noforn-required")),
    ],
    "invalid/sci_hcs_o_missing_companions.expected.json": [
        ("E010", ("capco", "portion.sci.hcs-system-constraints")),
        ("E010", ("capco", "portion.sci.hcs-system-constraints")),
        ("E059", ("capco", "marking.sci.hcs-o-companions")),
        ("E059", ("capco", "marking.sci.hcs-o-companions")),
    ],
}

# Matches `"rule": "LEGACY_ID"` exactly — JSON-valid string-form rule
# fields only. Object-form (`"rule": {...}`) does NOT match and is left
# alone (idempotency).
RULE_FIELD_RE = re.compile(r'"rule"(\s*:\s*)"([A-Z0-9_-]+)"')


def find_fixture_files(corpus_root: Path) -> Iterator[Path]:
    """Yield `*.expected.json` and `*.expected_fix.json` under corpus_root."""
    for root, _dirs, files in os.walk(corpus_root):
        for name in files:
            if name.endswith(".expected.json") or name.endswith(".expected_fix.json"):
                yield Path(root) / name


def encode_rule_value(scheme: str, predicate_id: str) -> str:
    """
    Encode the structured rule value as a compact one-line JSON object,
    matching the in-fixture pattern. The keys are not quoted via
    `json.dumps` because we want a deterministic spacing format
    (`{"scheme": "...", "predicate_id": "..."}`) that flows nicely
    against the surrounding compact `"span": {"start": N, "end": N}`
    style.
    """
    return (
        '{"scheme": ' + json.dumps(scheme, ensure_ascii=False) +
        ', "predicate_id": ' + json.dumps(predicate_id, ensure_ascii=False) +
        '}'
    )


def migrate_text(text: str, rel_path: str) -> tuple[str, int, list[str]]:
    """
    Rewrite every `"rule": "LEGACY"` occurrence in `text`. Returns
    `(new_text, num_changed, errors)`. Order of replacement matches the
    order legacy IDs appear in the file — load-bearing for
    PER_FIXTURE_OVERRIDES (positional resolution).
    """
    override_list = PER_FIXTURE_OVERRIDES.get(rel_path)
    errors: list[str] = []
    position = 0

    def repl(match: re.Match) -> str:
        nonlocal position
        sep = match.group(1)  # the ": " part, preserved verbatim
        legacy_id = match.group(2)

        if override_list is not None:
            if position >= len(override_list):
                errors.append(
                    f"{rel_path}: per-fixture override only covers "
                    f"{len(override_list)} diagnostics, but the file has "
                    f"more (at position {position}, legacy_id={legacy_id})"
                )
                position += 1
                return match.group(0)
            expected_legacy, predicate = override_list[position]
            if expected_legacy != legacy_id:
                errors.append(
                    f"{rel_path}: per-fixture override mismatch at "
                    f"position {position}: expected legacy_id="
                    f"{expected_legacy}, found {legacy_id}"
                )
                position += 1
                return match.group(0)
            scheme, predicate_id = predicate
        else:
            if legacy_id not in LEGACY_TO_PREDICATE:
                errors.append(
                    f"{rel_path}: legacy rule ID `{legacy_id}` "
                    f"(position {position}) is NOT in "
                    f"LEGACY_TO_PREDICATE — STOP condition; flag for PM"
                )
                position += 1
                return match.group(0)
            scheme, predicate_id = LEGACY_TO_PREDICATE[legacy_id]

        position += 1
        return f'"rule"{sep}' + encode_rule_value(scheme, predicate_id)

    new_text, count = RULE_FIELD_RE.subn(repl, text)

    # Validate JSON shape post-rewrite. If we corrupted the file, fail
    # loudly rather than write a broken fixture.
    if new_text != text:
        try:
            json.loads(new_text)
        except json.JSONDecodeError as e:
            errors.append(f"{rel_path}: post-rewrite JSON invalid: {e}")
            return text, 0, errors

    return new_text, count, errors


def migrate_fixture(
    path: Path, corpus_root: Path, dry_run: bool, verbose: bool
) -> tuple[int, list[str]]:
    """Migrate a single fixture. Returns `(num_rules_updated, errors)`."""
    rel_path = path.relative_to(corpus_root).as_posix()
    text = path.read_text(encoding="utf-8")
    new_text, count, errors = migrate_text(text, rel_path)

    if count > 0 and not dry_run:
        path.write_text(new_text, encoding="utf-8")

    if verbose and count > 0:
        action = "would update" if dry_run else "updated"
        print(f"  {action} {count} rule field(s) in {rel_path}", file=sys.stderr)

    return count, errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="report without writing")
    parser.add_argument("--verbose", "-v", action="store_true", help="per-file progress")
    parser.add_argument(
        "--corpus-root",
        default=None,
        help="path to tests/corpus/ (default: relative to this script)",
    )
    args = parser.parse_args()

    if args.corpus_root:
        corpus_root = Path(args.corpus_root).resolve()
    else:
        # tools/migrate-corpus-rule-ids.py → workspace root → tests/corpus/
        corpus_root = (
            Path(__file__).resolve().parent.parent / "tests" / "corpus"
        ).resolve()

    if not corpus_root.is_dir():
        print(f"error: corpus root {corpus_root} is not a directory", file=sys.stderr)
        return 2

    total_files = 0
    total_updated = 0
    total_errors: list[str] = []

    for fixture in sorted(find_fixture_files(corpus_root)):
        total_files += 1
        updated, errors = migrate_fixture(fixture, corpus_root, args.dry_run, args.verbose)
        total_updated += updated
        total_errors.extend(errors)

    print(
        f"{'(dry-run) ' if args.dry_run else ''}"
        f"scanned {total_files} fixture(s); "
        f"{total_updated} rule field(s) {'would be ' if args.dry_run else ''}updated",
        file=sys.stderr,
    )

    if total_errors:
        print(f"\n{len(total_errors)} error(s):", file=sys.stderr)
        for err in total_errors:
            print(f"  - {err}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
