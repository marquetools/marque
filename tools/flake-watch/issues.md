<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# flake-watch — quarantine queue

**Cap**: 10 active entries. Cap exceedance blocks PR merges (see
[README.md](./README.md)).

**Active count**: 0 / 10.

---

## Entry template (do NOT edit; copy below "Active entries")

> ## flake-YYYY-MM-DD-shortname
>
> - **Test path**: `crates/<crate>/tests/<file>.rs::<test_name>`
> - **First observed**: `YYYY-MM-DD` (CI run URL: `<url>`)
> - **Symptom**: `<one-line description: e.g., "intermittent timeout under load">`
> - **Suspected cause**: `<one-sentence hypothesis>`
> - **Quarantined by**: `<PR or commit SHA>`
> - **Triage owner**: `<name>`
> - **Resolution path** (one of: fix-underlying / convert-to-smoke / delete / pending-investigation): `<choice>`
> - **Target resolution date**: `YYYY-MM-DD`

---

## Active entries

<!--
No active entries. Subsequent entries follow the template above.
The CI gate counts entries by matching the line `^## flake-` (the
template heading uses a blockquote `> ##` so it does not count toward
the cap).
-->
