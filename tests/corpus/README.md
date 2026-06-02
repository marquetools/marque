<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Test Corpus

Each subdirectory under `tests/corpus/` holds one marking grammar's fixture
corpus. Today the workspace ships a single grammar:

- `capco/` — the CAPCO/ISM MVP corpus (valid, invalid, prose, documents,
  lattice, foreign, and mangled fixtures, plus the corpus contract and
  provenance records).

A new grammar lands its fixtures under `tests/corpus/<grammar>/` and its
corpus-derived priors under `crates/<grammar>/corpus/`.

Test code resolves a grammar's corpus root through
`marque_test_utils::grammar_corpus_root("<grammar>")`. The shorthand
`marque_test_utils::corpus_root()` returns the CAPCO corpus root for the
workspace's CAPCO-only consumers.
