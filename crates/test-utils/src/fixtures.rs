// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared in-memory benchmark fixtures.
//!
//! These are realistic, hand-written documents used by the engine's
//! latency and throughput benches so a single source of truth is shared
//! across `lint_latency`, `fix_10kb`, and `throughput_pages` rather than
//! triplicating a multi-kilobyte string literal in each bench binary.
//!
//! Kept here (a `.rs` file with an inline SPDX header) rather than as raw
//! `.txt` fixtures so no new REUSE licensing surface is added.

/// A realistic, generously-spaced ~3 KB single-page classified
/// memorandum, correctly marked.
///
/// Shape: a top/bottom `SECRET//SI//NOFORN` classification banner, an
/// unclassified header + references block, ten double-spaced
/// portion-marked paragraphs (a mix of `(U)`, `(C)`, `(S//NF)`, an
/// `(S//REL TO ...)` coalition-releasable paragraph, an `(S//SI//NF)` SCI
/// paragraph, and a `(U//FOUO)` administrative note), and a closing
/// classification authority block.
///
/// Paragraph breaks use `\n\n` only — no `\n\n\n+` runs and no form-feed —
/// so the whole fixture stays on ONE page (`PageContext` never resets) and
/// the SCI roll-up resolves against a single banner. The banner correctly
/// rolls the `(S//SI//NF)` portion up to `SECRET//SI//NOFORN`, so the
/// document produces zero error/warning diagnostics; exactly one advisory
/// `suggest` fires (a RELIDO closure inference on the bare `(C)` portion,
/// §H.8 p154).
pub const SINGLE_PAGE: &str = r#"SECRET//SI//NOFORN

(U) MEMORANDUM FOR THE RECORD

(U) SUBJECT: Regional Stability Assessment - Quarterly Update (Q2)

(U) REFERENCES: (a) Prior quarterly assessment, dated 15 March. (b) Field
collection summaries received through the reporting cutoff. (c) Coalition
liaison reporting forwarded by the partner integration cell.

1. (S//NF) The intelligence community assesses with high confidence that
regional force posture has shifted measurably over the reporting period.
Multiple corroborating sources indicate a sustained logistical buildup that
is consistent with preparations for extended operations rather than a
routine seasonal rotation. The pace of activity has not slackened since the
previous assessment and shows no sign of an imposed operational pause.

2. (S//REL TO USA, GBR, AUS) Partner-shared reporting confirms the movement
of materiel through established overland corridors. The volume and cadence
of shipments exceed the levels observed during the comparable period last
year, and the mix of materiel has shifted toward sustainment stocks. This
paragraph is releasable to the named coalition partners under existing
bilateral and multilateral agreements and may be shared at the working
level without further originator coordination.

3. (C) Open-source reporting and routine diplomatic traffic broadly align
with the classified picture. Attribution of specific shipments to specific
end users remains less certain at this classification level and is addressed
more fully in the signals-derived paragraph below. Analysts caution that the
open-source baseline lags the classified reporting by several weeks.

4. (S//SI//NF) Signals intelligence collected during the period corroborates
the human-source reporting and provides additional granularity on the
command-and-control relationships among the organizations involved.
Collection gaps persist over the southern approaches, and confidence in the
order-of-battle picture there is correspondingly lower than elsewhere in the
area of responsibility.

5. (S//NF) Taken together, the reporting supports a judgment that the
activity reflects a coordinated effort directed at the regional level rather
than a series of independent local decisions. Recognition in this judgment is
high but is tempered by the aforementioned collection gaps and by the
inherent difficulty of inferring intent from disposition alone.

6. (U) The next scheduled update will incorporate findings from the ongoing
collection effort and any partner contributions received before the cutoff.
No change to the current collection posture is recommended at this time, and
no additional resourcing request accompanies this assessment.

7. (U//FOUO) Administrative point of contact, distribution instructions, and
reproduction limits are maintained separately by the originating office and
are available on request through normal channels.

Classified By: John Q. Analyst, Senior Intelligence Officer
Derived From: Multiple Sources
Declassify On: 20501231

SECRET//SI//NOFORN
"#;

/// [`SINGLE_PAGE`] with BOTH classification banners deliberately
/// misspelled `SERCET`.
///
/// Paired with a `SERCET → SECRET` corrections-map entry, each banner
/// fires a `Phase::Localized` text correction in pass-1, driving the
/// engine through the full two-pass fix pipeline (pre-pass-1 cache
/// population, post-pass-1 re-lint, disambiguation, overlap demotion).
/// After correction the page is identical to [`SINGLE_PAGE`] and valid
/// again. This is the fix-path companion to the lint-path [`SINGLE_PAGE`].
pub const SINGLE_PAGE_TO_FIX: &str = r#"SERCET//SI//NOFORN

(U) MEMORANDUM FOR THE RECORD

(U) SUBJECT: Regional Stability Assessment - Quarterly Update (Q2)

(U) REFERENCES: (a) Prior quarterly assessment, dated 15 March. (b) Field
collection summaries received through the reporting cutoff. (c) Coalition
liaison reporting forwarded by the partner integration cell.

1. (S//NF) The intelligence community assesses with high confidence that
regional force posture has shifted measurably over the reporting period.
Multiple corroborating sources indicate a sustained logistical buildup that
is consistent with preparations for extended operations rather than a
routine seasonal rotation. The pace of activity has not slackened since the
previous assessment and shows no sign of an imposed operational pause.

2. (S//REL TO USA, GBR, AUS) Partner-shared reporting confirms the movement
of materiel through established overland corridors. The volume and cadence
of shipments exceed the levels observed during the comparable period last
year, and the mix of materiel has shifted toward sustainment stocks. This
paragraph is releasable to the named coalition partners under existing
bilateral and multilateral agreements and may be shared at the working
level without further originator coordination.

3. (C) Open-source reporting and routine diplomatic traffic broadly align
with the classified picture. Attribution of specific shipments to specific
end users remains less certain at this classification level and is addressed
more fully in the signals-derived paragraph below. Analysts caution that the
open-source baseline lags the classified reporting by several weeks.

4. (S//SI//NF) Signals intelligence collected during the period corroborates
the human-source reporting and provides additional granularity on the
command-and-control relationships among the organizations involved.
Collection gaps persist over the southern approaches, and confidence in the
order-of-battle picture there is correspondingly lower than elsewhere in the
area of responsibility.

5. (S//NF) Taken together, the reporting supports a judgment that the
activity reflects a coordinated effort directed at the regional level rather
than a series of independent local decisions. Recognition in this judgment is
high but is tempered by the aforementioned collection gaps and by the
inherent difficulty of inferring intent from disposition alone.

6. (U) The next scheduled update will incorporate findings from the ongoing
collection effort and any partner contributions received before the cutoff.
No change to the current collection posture is recommended at this time, and
no additional resourcing request accompanies this assessment.

7. (U//FOUO) Administrative point of contact, distribution instructions, and
reproduction limits are maintained separately by the originating office and
are available on request through normal channels.

Classified By: John Q. Analyst, Senior Intelligence Officer
Derived From: Multiple Sources
Declassify On: 20501231

SERCET//SI//NOFORN
"#;
