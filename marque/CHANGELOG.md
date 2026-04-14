# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.2.0 (2026-04-14)

### Chore

 - <csr-id-073f17d77c3f54da21c397ff1bdcc71248272a00/> bump versions and release to resolve crates.io backlog

### New Features

 - <csr-id-314d9055cf311a4a5b45347a400482ca843c75d0/> non-IC dissemination controls (LIMDIS, SBU, LES, SSI)
 - <csr-id-73febea8c2c7d225ed1ab94d7fdbc62073ed4106/> implement US2 auto-fix with NDJSON audit trail
   Full `marque fix` subcommand (T047-T051a) replacing the Phase 3 stub.
   Applies fixes at or above the configured confidence threshold, emits
   NDJSON audit records to stderr conforming to contracts/audit-record.json,
   and computes exit codes from post-fix remaining diagnostics.
   
   CLI changes:
   - stdin support (paths optional, `-` sentinel) with --write-stdout default
   - --in-place (default for file paths) via atomic temp-file rename (T048)
   - --dry-run emits audit with dry_run=true, does not write output
   - --fixed-timestamp <RFC3339> gated on MARQUE_ALLOW_FIXED_CLOCK=1 (T051a)
   - Mutual exclusion: --dry-run/--in-place, --in-place/--write-stdout (exit 64)
   
   Audit emission (T049, FR-005a):
   - AuditRecordJson struct mapping AppliedFix to contracts/audit-record.json
   - Atomic write_all per record (serialize to buffer, single flush with \n)
   - Error frames on serialization failure
   - -q suppresses narration but NEVER audit records
   - Schema version "marque-mvp-1" on every record
 - <csr-id-243e4c9ffbeacfa37a9058b123f109a286f5cadd/> implement Phase 3 — US1 lint with byte-precise spans + CLI
   Phase 3 (User Story 1, P1, MVP) lands the lint side of marque
   end-to-end. Nine CAPCO rules (E001-E008, W001) emit byte-precise
   diagnostics threaded through a new IsmAttributes::token_spans
   sidecar. The marque check CLI is rewritten to match contracts/cli.md
   verbatim, and 24 invalid + 20 valid corpus fixtures plus a 1010-line
   prose precision corpus pin every rule against golden .expected.json.
   
   Test count rises from 77 to 116 (+39), zero failures.
   
   Architectural changes:
   - Span and structure plumbing:
     marque-ism gets TokenKind/TokenSpan { kind, span, text } and
     IsmAttributes.token_spans: Box<[TokenSpan]>. The parser walks
     separator positions via match_indices and records absolute byte
     offsets for every recognized token, including TokenKind::Unknown
     for blocks the parser cannot classify. Banner-form full-word
     dissem controls (NOFORN, ORCON, IMCON) now parse via a new
     parse_dissem_full_form fallback so the CVE-only abbreviations
     (NF, OC, IMC) do not surface as Unknown.
   - Scanner page breaks:
     marque-ism gets MarkingType::PageBreak. The scanner emits a
     zero-length candidate at every form-feed (\f) and at the third
     consecutive newline of a \n\n\n+ run. \n\n (normal paragraph
     break) does NOT trip the reset. The engine resets PageContext
     BEFORE attempting to parse a PageBreak candidate so a corrupt
     candidate cannot block the reset. Closes the engine.rs:80
     TODO(phase-3) marker from the PR #3 review.
   - RuleContext zone/position:
     Both fields are now Option<Zone> / Option<DocumentPosition> and
     the engine passes None until a structural scanner pass can prove
     a value. The Phase-2 hardcoded Body was a silent lie to any
     future rule that read the field; Phase 3 makes the uncertainty
     type-system-enforced. Closes the engine.rs:103 TODO(phase-3)
     marker from the PR #3 review.
   
   Phase 3 rules (marque-capco/src/rules.rs, full rewrite):
   - E001 banner-abbreviation: fires on abbreviated dissem in banners.
     Span points at the literal abbreviation bytes via the parallel-
     indexed DissemControl token spans.
   - E002 missing-usa-trigraph: fires when REL TO is non-empty and
     USA is missing or not first. Span points at the first trigraph.
   - E003 misordered-blocks: walks token_spans, computes the CAPCO
     ordinal per block kind (Class < SCI < SAR < Dissem/RelTo), fires
     if any descending step appears. Span covers the whole marking.
     Confidence 0.6 — kept as suggestion under default 0.95 threshold.
   - E004 separator-count: detects back-to-back // separator runs by
     walking adjacent Separator token spans whose end == next start.
     Replaces the Phase-2 vec![] stub. Confidence 0.99.
   - E005 declassify-in-banner: fires when a banner has a declass
     exemption or date inline. Span points at the declass token.
   - E006 deprecated-dissem: walks both DissemControl tokens (FOUO,
     in modern CVE) and Unknown tokens (LIMDIS, removed from CVE).
     Looks each up in find_migration; hits whose replacement is a
     known dissem control fire E006. Skips abbreviation expansions
     (NF to NOFORN) — those are owned by E001.
   - E007 x-shorthand-date: walks Unknown tokens (the deprecated
     dashed forms 25X1- / 50X1- land here because the CVE has only
     the canonical 25X1 / 50X1-HUM). Migration table lookup via
     TokenSpan.text avoids the false-positive of firing on the
     canonical form.
   - E008 unrecognized-token: walks Unknown tokens and skips ones
     E007 will pick up (find_migration().is_some()). FR-012: no fix
     offered.
   - W001 deprecated-marking-warning: scaffold present, returns
     vec![] in Phase 3 because the seed MIGRATIONS table has no
     W001-flagged entries. Adding a flag to MigrationEntry in a
     future build.rs change starts firing diagnostics with no other
     code edits.
   
   CLI rewrite (marque/src/main.rs + new marque/src/render.rs):
   - Stdin sentinel: a path of `-` reads from stdin. No path given
     also reads from stdin. Mixed paths and `-` allowed.
   - --config <PATH>: short-circuits the upward walk and uses the
     specified path's parent as the load root.
   - --confidence-threshold <FLOAT>: per-call override for the fix
     confidence gate.
   - --format human|json: defaults to human for TTY stdout, json
     otherwise. JSON is NDJSON conforming to contracts/diagnostic.json
     with derive(Serialize) structs (no extra fields).
   - --no-color: honors NO_COLOR env var (any non-empty value) and
     TERM=dumb. Suppresses ANSI in human format.
   - -q / --quiet: suppresses non-diagnostic stderr narration.
   - --explain-config: dumps the merged Configuration as pretty JSON,
     exits 0. Mutually exclusive with input paths and with the `fix`
     subcommand. classifier_id_present is a boolean — the value
     itself is NEVER emitted, even when MARQUE_CLASSIFIER_ID is set.
   - Exit codes: 0 clean, 1 errors (Severity::Error or Severity::Fix),
     2 warnings only, 64 EX_USAGE, 65 EX_DATAERR, 74 EX_IOERR.
     Severity::Fix maps to exit 1 because a Fix-severity diagnostic
     is still a violation that should block CI.
   
   Corpus (24 invalid + 20 valid + 1 prose):
   - 3 fixtures per rule for E001-E008 with hand-computed byte spans
     validated by running each fixture through the integration test.
   - W001 fixtures deferred per the plan (the rule cannot fire on
     real corpus until a W001-flagged migration entry lands).
   - 20 known-good fixtures spanning portion, banner, and CAB types.
   - tests/corpus/prose/article.txt: 1010 lines of public-domain
     Federalist No. 10 prose with 22 incidental (S) / (a) / (i)
     mid-sentence parens. The lint is byte-clean against this corpus
     — the SC-003a precision gate is satisfied.
   
   Tests added (39 new):
   - crates/marque-core/src/scanner.rs::tests — 4 page-break tests
   - crates/marque-core/src/parser.rs::tests — 5 token-span tests
   - crates/marque-engine/src/engine.rs::tests — multi-page document
   - crates/marque-capco/src/rules.rs::tests — 13 per-rule tests
     with byte-span assertions
   - crates/marque-capco/tests/rules_us1.rs — corpus harness driving
     every fixture through the parser+rules with .expected.json
     match (drives parser/scanner directly to avoid a circular
     marque-engine dev-dep)
   - crates/marque-engine/tests/lint_pipeline.rs — 13 happy-path /
     edge-case tests + 2 insta JSON snapshots pinning the
     contracts/diagnostic.json shape against drift
   - marque/tests/cli_smoke.rs — 9 CLI smoke tests via assert_cmd
     covering stdin sentinel, --explain-config no-leak, NO_COLOR,
     exit codes, and mutual exclusion

### Bug Fixes

 - <csr-id-f90ab0da894be888b3cc7e3fcba2f3b72a84b46d/> spelling corrections and config exemptions
 - <csr-id-d0d7b2df50a4c62167f7b0b4d5c8914a0c8400d7/> fixed formating and linting issues in multiple places; corrected an issue where the release action did not generate a workspace version or changelog
 - <csr-id-f211cea82893ab678cbd7f01f47e47f41697a0d1/> fixed formating and linting issues in multiple places; corrected an issue where the release action did not generate a workspace version or changelog
 - <csr-id-4dd20f13bdd799192442e383be04b9e9caaa8b30/> address PR review comments — null→undefined, workspace dep, aho-corasick scan
   - harness.html: pass `undefined` instead of `null` for optional config_json in lint/fix buttons
   - benches/wasm_latency.md: use `undefined` instead of `null` in measurement script
   - marque-wasm/Cargo.toml: use `workspace = true` for marque-engine dependency
   - Cargo.toml: set default-features = false on workspace marque-engine dep
   - marque/Cargo.toml, marque-server/Cargo.toml: add features = ["batch"] to restore batch support
   - marque-engine/Cargo.toml: add aho-corasick dependency
   - engine.rs: replace O(n*m) windows/position loop with single-pass AhoCorasick automaton for pre-scanner corrections
 - <csr-id-521bbd9525310b117a76c1f1ce2e181e315544f3/> address 6 review findings from PR code review
 - <csr-id-b23c52c4c524ab37aec9d38a124df5209b48a5d1/> address Copilot PR comments — UTF-8 validation + tempdir tests
   Copilot's inline review on PR #5 identified 8 items. Three (stdin
   input="-", error kind differentiation, eprintln deadlock) were already
   resolved in prior commits. This commit addresses the remaining five.
   
   UTF-8 input validation (contracts/cli.md §"Input handling"):
     Added validate_utf8() guard called at the top of both run_check and
     run_fix per-input loops. Non-UTF-8 input now exits 74 EX_IOERR with
     a clear error message. Previously raw bytes were passed to the engine
     without validation.
   
   Arc<str> readability:
     Changed Arc::from(p.display().to_string().as_str()) to
     Arc::<str>::from(p.display().to_string()) — avoids the temporary
     borrow that required reasoning about drop order.
   
   Windows file-locking in tests:
     Replaced NamedTempFile (which holds an open file handle) with
     tempdir() + path in three CLI integration tests. The old pattern
     would deadlock on Windows where file locks are mandatory.
 - <csr-id-33147c96fd0ea3a830ad43b945e648c5e008425f/> address PR #5 review — NDJSON stream purity + threshold validation
 - <csr-id-ef1a6c9bc95db1f3e9102b54145c8fe026333227/> address three review findings in audit emission loop
   - Replace eprintln! (while holding stderr.lock()) with writeln! on
     the already-locked handle to eliminate the potential deadlock
   - Distinguish EX_IOERR vs EX_DATAERR: ErrorKind::Other (set by
     render_audit_record for serde_json failures) returns EX_DATAERR;
     all other IO error kinds return EX_IOERR per contracts/cli.md
   - Set audit_fix.input to Some(Arc::from("-")) for stdin so the
     audit record emits input:"-" instead of input:null per
     contracts/audit-record.json
 - <csr-id-15b595796b518df62ea7438909c5efa8432055c4/> address review findings — 6 code fixes + 10 test gaps
   Independent Rust specialist and QA specialist reviews identified 1
   CRITICAL, 6 HIGH, 8 MEDIUM, and 5 LOW findings. This commit resolves
   all non-deferred items. Tests rise 164 → 179 (+15).
   
   Code fixes (C1, H1, H2, M1, M2):
   
     C1 — Post-fix re-lint for exit codes (T050 compliance). run_fix now
     calls engine.lint() on the actual post-fix text instead of using
     remaining_diagnostics (which is a subtraction from the original lint,
     not a true re-lint). For --dry-run, runs a second Apply-mode fix to
     get the would-be text. Catches cascading resolutions and introduced
     violations.
   
     H1 — JSON injection in render_audit_error_frame. error_code and
     rule_id are now JSON-escaped via serde_json::to_string() so special
     characters in error messages cannot produce malformed JSON on the
     audit stream (FR-005a compliance).
   
     H2 — Dry-run bypasses engine DryRun mode. run_fix now uses
     FixMode::DryRun directly instead of always running Apply and patching
     dry_run=true post-hoc. Respects the AppliedFix construction invariant
     documented in CLAUDE.md.
   
     M1 — --dry-run + --write-stdout now rejected (exit 64). Previously
     silently accepted with no effect.
   
     M2 — IO errors during file write now return EX_IOERR immediately
     instead of continue-ing to the next file. Prevents partial-batch
     confusion where diagnostic exit codes mask earlier IO failures.
   
   Test gaps (H3-H6, L3, L4, M6):
   
     H3 — Two insta snapshot tests pinning the full audit NDJSON shape
     for Apply and DryRun modes with FixedClock determinism.
   
     H4 — fix_empty_input_exits_zero_no_audit: empty input → exit 0, no
     audit records, empty stdout.
   
     H5 — fix_dry_run_stdin_produces_no_stdout: dry-run stdin → empty
     stdout, audit emitted with dry_run=true.
   
     H6 — fix_all_below_threshold_exits_one_no_audit: only E003 (0.6),
     no audit records, exit 1.
   
     L3 — fix_write_stdout_on_file_input: --write-stdout overrides
     --in-place, file unchanged. fix_dry_run_exit_code_matches_apply:
     parity assertion.
   
     L4 — dry_run_parity_rule_ids_match: verifies remaining diagnostic
     rule IDs match, not just count.
   
     M6 — Two FR-016 tiebreaker tests: same-span/different-rule-ID picks
     lower rule ID; same-span/same-rule picks lower replacement.
   
   Cosmetics (L1, L5):
   
     L1 — Narration "applied N fix(es)" now suppressed when N=0.
     L5 — #[allow(clippy::too_many_arguments)] documented with rationale.
   
   Deferred to Phase 5: M5 (CorrectionsMap source coverage depends on
   C001), M7 (remaining_diagnostics naming), M8 (--confidence-threshold
   on check).
 - <csr-id-60978104ab8ee7806f8157fa4d99fdfd0ed8f41f/> address reviewer-identified items R-1 through R-4
   Independent PR review on #4 (commit 8bb899a) identified four items.
   This commit addresses all four: R-3 and R-4 as real Phase 3 fixes,
   R-1 and R-2 as explicit Phase 4 deferrals with TODO markers pointing
   at the exact follow-up tasks. Tests rise 132 → 144 (+12 across R-3
   pattern tests, R-4 renderer tests, and CLI smoke coverage).
   
   R-1 — Missing audit-record NDJSON in `marque fix` (HIGH, deferred):
   
   The reviewer correctly flagged that FR-005a and contracts/cli.md
   mandate an NDJSON audit stream to stderr for every AppliedFix. The
   current implementation only emits a summary line.
   
   This is Phase 4 (US2 auto-fix with audit trail) scope per my Phase 3
   plan's "NOT Building" section: "NDJSON audit-record stream from `fix`
   — Phase 4 / US2 (T049). The `check` NDJSON is the diagnostic stream;
   `fix`'s audit stream is a separate codepath under Phase 4."
   
   Added an explicit `TODO(phase-4: T049)` comment in `run_fix`
   pointing at the task and reiterating the FR-005a contract (atomic
   per-record serialization, `-q` must NOT suppress audit lines). The
   marker is visible in code so whoever picks up Phase 4 cannot miss
   the hook point.
   
   R-2 — Non-atomic file write in `marque fix` (HIGH, deferred):
   
   Same Phase-4 story. T048 in tasks.md specifies the atomic temp-file
   rename, and my Phase 3 plan's "NOT Building" lists it: "Atomic
   temp-file rename for fix output — Phase 4 / US2 (T048)."
   
   Added an explicit `TODO(phase-4: T048)` comment above the
   `std::fs::write` call pointing at the task and the contract section.
   
   R-3 — E007 pattern matching for X-shorthand forms (MEDIUM, fixed):
   
   A real Phase 3 correctness gap. The seed MIGRATIONS table only had
   exact entries for `25X1-` and `50X1-`. A user writing any other
   X-shorthand deprecated form (e.g., `25X2-`, `25X9-`, `25X3-WMD-`)
   would see the token fall through to E008 ("unrecognized") instead of
   E007 ("deprecated X-shorthand"). T036's wording ("X-shorthand
   declassification date marking, e.g. 25X1-") clearly implies the rule
   should cover the class of X-shorthand forms, not just the two
   hardcoded entries.
   
   Added a `looks_like_deprecated_x_shorthand` helper that recognizes
   the `\d+X\d+(-[A-Z]+)?-` pattern shape. E007's check has two paths:
   
     1. Migration-table lookup (unchanged) — uses the table's
        authoritative confidence (0.97) and reference.
     2. Pattern-match fallback — for forms not in the seed table, strip
        the trailing `-` to produce the canonical form and emit E007
        at confidence 0.95 (slightly lower because the canonical form
        is derived rather than CVE-authoritative).
   
   E008's filter now also consults `looks_like_deprecated_x_shorthand`
   so the two rules cannot double-fire on the same Unknown token. Both
   rules share the single predicate; adding a new X-shorthand form only
   requires extending the helper, not touching either rule.
   
   Three new unit tests:
   - `looks_like_deprecated_x_shorthand_matches_expected_patterns` —
     pins the pattern recognition against canonical and malformed inputs
   - `e007_fires_on_pattern_matched_x_shorthand_not_in_migration_table` —
     exercises the new fallback path via `25X2-`, asserts confidence
     0.95 and that E008 does NOT also fire on the same span
   - `e007_still_fires_on_migration_table_entries` — regression guard on
     the existing `25X1-` path
   
   One new corpus fixture:
   - tests/corpus/invalid/x_shorthand_date_pattern.{txt,expected.json}
     — `SECRET//25X2-//NOFORN` exercises the fallback path via the
     integration harness
   
   R-4 — Rustc-style caret renderer (LOW, fixed):
   
   My Phase 3 plan's "Desired state" UX section promised rustc-style
   output with a source snippet, line-number gutter, and caret pointing
   at the span. The actual render_human was a simpler three-line form.
   Copilot's earlier fix correctly updated the doc comment to match
   reality ("location-prefixed header") but that masked the plan
   regression — the reviewer correctly flagged the delta.
   
   Rewrote render_human to produce rustc-style output:
   
       banner.txt:1:17 fix[E001] banner uses abbreviated dissem control "NF"; use "NOFORN"
         --> banner.txt:1:17-19
         |
       1 | TOP SECRET//SI//NF
         |                 ^^ replace with "NOFORN" (confidence 100%)
         |
         = citation: CAPCO-ISM-v2022-DEC-§3.2
   
   The renderer:
   - Computes (line, col_start) and (end_line, col_end) from byte spans
     via byte_to_line_col
   - Clamps col_end to end-of-line for multi-line spans (defensive;
     CAPCO markings are single-line)
   - Extracts the source line via a new extract_line helper that strips
     trailing \r for CRLF line endings
   - Renders the rustc-style 6-line block (header, arrow, gutter, source,
     caret+hint, gutter, citation footer)
   - Uses ANSI styles (BoldRed for level and carets, BoldBlue for arrow/
     pipe/equals, Bold for rule ID) when color is enabled; plain text
     when --no-color / NO_COLOR / TERM=dumb / non-TTY
   
   Six new render unit tests plus existing CLI smoke tests cover:
   - `extract_line` returns correct bytes for first/middle/last lines
   - `extract_line` strips trailing \r for CRLF
   - `extract_line` returns None for out-of-range line numbers
   - `render_human_produces_rustc_style_shape_with_caret` — full shape
     assertion against the E001 banner_abbrev fixture
   - `render_human_without_color_has_no_ansi_escapes` — pins the plain
     path
   - `render_human_with_color_emits_ansi_escapes` — pins the styled path
   - `render_human_diagnostic_without_fix_omits_hint` — pins E008-style
     output (caret only, no "replace with" hint)
   
   The module doc comment also got updated to accurately describe the
   new output shape, replacing Copilot's earlier "location-prefixed
   header" note with a proper rustc-style example.
 - <csr-id-8bb899afb111e894ca60d6c05df6ddb940cea05c/> address three MEDIUM review findings from pr-4 pass
   Third-pass review of PR #4 surfaced three MEDIUM-severity items, all
   cosmetic or defensive-programming observations. This commit resolves
   all three so the PR lands clean. Tests rise 131 → 132.
   
   M-1 — E003 drop the broken `original` reconstruction:
   
   The E003 `MisorderedBlocksRule` emitted a FixProposal whose `original`
   field was reconstructed by concatenating token texts (block tokens +
   separators) from `attrs.token_spans`. For a REL TO block like
   `SECRET//REL TO USA//SI//NOFORN`, the parser stores individual
   trigraph spans (`USA`) rather than a single block span with the full
   `REL TO USA` text. The reconstruction therefore produced
   `SECRET//USA//SI//NOFORN` instead of the actual source bytes — a
   cosmetic mismatch that could confuse downstream audit-stream
   consumers comparing `original` against `source[span]`.
   
   The engine does NOT read `FixProposal.original` at splice time (only
   `span.start..span.end` and `replacement`), so this was never a runtime
   correctness bug. Fix: set `original` to an empty string. The field is
   audit-display data only; consumers that need the actual original
   bytes should read them from the authoritative source buffer.
   
   M-2 — Document run_fix exit-code guard's EX_DIAG_WARN inclusion:
   
   `run_fix` guards exit-code escalation with
   `matches!(exit_code, EX_OK | EX_DIAG_WARN)`. `EX_DIAG_WARN` is
   unreachable in `run_fix` today (the function only emits `EX_IOERR` or
   `EX_DIAG_ERROR`), but the guard includes it for defensive parallelism
   with `run_check`'s exit-code logic. A future addition of fix-path
   warning support will not need to revisit this guard. Added a comment
   explaining the priority order (EX_IOERR > EX_DIAG_ERROR > EX_DIAG_WARN
   > EX_OK) and the defensive-parallelism rationale.
   
   M-3 — Reject form feed (`\f`) inside portion markings:
   
   `find_portion_end` rejected `\n`, `\r`, and `(` but not `\f`. A form
   feed inside `(...)` parens would produce a portion candidate spanning
   the control character, and if `scan_page_breaks` also emitted a
   PageBreak at the same offset, the portion would shadow the page-break
   reset under the composite sort (Portion at lower start, PageBreak at
   higher, engine processes Portion first).
   
   Form feed inside portion parens is unrealistic in real documents, but
   the defensive rejection is a one-character change. Added `b'\x0c'` to
   the reject arm and updated the doc comment. New regression test
   `rejects_form_feed_in_portion` pins both the Portion rejection AND
   the PageBreak emission at the form-feed offset.
 - <csr-id-1fc4bff2d9ff6b443ce7958b4b249793d550579d/> address all 5 PR review findings
   - parser.rs: push separator TokenSpans after the block loop then sort by
     span.start so the final token_spans slice is in document (source) order
     (previously separators were all pushed before any block tokens)
   - rules.rs: remove parens wrapping from reorder_marking() return value for
     Portion markings — span excludes outer parens so adding them would produce
     double-parens ((…)) on splice; also remove now-unused `kind` parameter
     and MarkingType import
   - main.rs: preserve higher-priority exit codes — EX_IOERR recorded from an
     earlier file is no longer overwritten by EX_DIAG_ERROR from a later file
     (only escalate when current code is EX_OK or EX_DIAG_WARN)
   - render.rs: update module doc comment to accurately describe current output
     (location-prefixed header + citation, not rustc-style caret)
 - <csr-id-71e98eb208526ead45a420fcbd9dbc7129d747b3/> address MEDIUM/LOW review findings
   Follow-up to the HIGH fix-up commit. Resolves the remaining eight
   findings from the Rust specialist review of Phase 3. Tests rise
   from 128 to 131, zero failures.
   
   A.1 — E001 abbreviation filter replaced with direct source-text check:
   
   The previous implementation emitted every potentially-abbreviated
   dissem diagnostic and then post-filtered by comparing the diagnostic's
   span byte-length against the proposal's `original` string length. The
   logic worked by coincidence: `control.as_str()` for the CVE `NF`
   variant happened to equal the source "NF" length, so `NOFORN` (length
   6) was dropped while "NF" (length 2) was kept. A future abbreviation
   whose CVE form has a different length from its source bytes would
   silently regress.
   
   The new path checks at emit time: compare `token_span.text.as_ref()`
   against `control.as_str()` and skip when they don't match. Zero
   post-hoc filtering. Same observable behavior, much clearer intent.
   
   C.3 — E007 uses the shared `is_dissem_replacement` predicate:
   
   E006 used the `is_dissem_replacement` helper (7 entries including
   PROPIN) while E007 had an inline `matches!` missing `"PROPIN"`. If a
   future migration entry mapped a deprecated token to `"PROPIN"`, E007
   would claim it instead of E006. Both rules now share the single
   `is_dissem_replacement` predicate so the guard sets cannot drift.
   
   B.3 — Scanner sort stability for zero-length PageBreak spans:
   
   `sort_unstable_by_key(|c| c.span.start)` is unstable, so a PageBreak
   at byte N and a content candidate at byte N could sort in either
   order. The PageBreak must sort first for the engine's PageContext
   reset to run BEFORE the co-located content is processed, otherwise
   the reset is defeated.
   
   Replaced with `sort_unstable_by` using a `(start, kind_priority)`
   composite key. `kind_sort_priority` returns 0 for PageBreak and 1 for
   everything else, so PageBreak always sorts first at equal offsets.
   New unit test pins the priority ordering.
   
   A.3 — Parser cursor comment corrected:
   
   The `parse_rel_to_with_spans` cursor advances `cursor += entry.len()
   + 1` on each iteration; the previous comment claimed "OK to overflow
   on the last entry — unused after the loop". `usize` overflow is NOT
   "OK" in debug — it panics — and the arithmetic is actually safe
   because the cursor is bounded by document size and is never read
   after the loop terminates. Comment rewritten to explain the real
   safety argument.
   
   D.2 — `--explain-config` emits sorted `corrections` key list:
   
   Contract says "corrections-map keys". The previous implementation
   emitted `corrections_count` (an integer), which is not the contract
   shape. Now emits a sorted `Vec<&String>` of the keys, which is also
   deterministic across HashMap iteration orders (important for
   CI-golden consumers). New CLI smoke test guards the shape and
   rejects the deprecated `corrections_count` field name.
   
   D.3 — `-v` / `--verbose` flag wired to tracing subscriber:
   
   The previous implementation parsed the flag but never consulted it —
   the subscriber was initialized from MARQUE_LOG only, before CLI
   parsing. Contract: "-v equivalent to MARQUE_LOG=marque=debug".
   
   Moved `Cli::parse()` before subscriber init. The subcommand-level
   `CommonOptions.verbose` is extracted from the parsed command, and
   used to build the env filter with precedence: CLI flag > env var >
   default (`marque=info`). If `-v` is set, `marque=debug` wins
   regardless of MARQUE_LOG — matching the FR-007 precedence chain.
   
   D.4 — `-q` / `--quiet` no-op documented for `run_check`:
   
   The `check` subcommand currently emits no operator narration to
   stderr, so `-q` has nothing to suppress. Rather than deleting the
   field (which would break the clap derive if other subcommands also
   read it, which `fix` does), added an explicit `let _ = common.quiet`
   to mark the intentional no-op plus a comment pointing future
   contributors at how to wire it when narration lands.
   
   D.6 — Removed dead `let _ = LintResult::default()`:
   
   The line was a leftover from an earlier refactor. It served no
   purpose, didn't suppress any real warning, and confused readers.
   Removed along with the now-unused `LintResult` import.
 - <csr-id-842ada81b8d1b3e20d926d96f27a58e0b8ad161f/> address HIGH review findings (C.1, C.2, D.1, F.1)
   Rust specialist review of the Phase 3 commit surfaced four
   HIGH-severity correctness gaps against the plan and contracts. This
   commit resolves all four, adding 12 new tests (116 to 128).
   
   C.1 — E003 emits FixProposal with confidence 0.6 (T032):
   
   The previous implementation emitted Diagnostic::new(..., None), which
   silently dropped the suggestion-only fix that T032 mandates. Without
   the proposal, the NDJSON `fix` field was null for E003 and Phase 5
   corrections-map integration (which correlates by confidence range)
   would never see E003 violations even at lower thresholds.
   
   Added `reorder_marking()` helper that rebuilds the marking string from
   `attrs.token_spans` grouped by CAPCO ordinal (Class // SCI // SAR //
   Dissem // REL TO). Within each block, tokens preserve their document
   order; REL TO trigraphs are reassembled into a single `REL TO ...`
   block. The emitted FixProposal carries confidence 0.6, which sits
   below the default 0.95 threshold so it never auto-applies but is
   present for consumers that lower the threshold or surface suggestions
   in IDE quick-fixes.
   
   C.2 — E004 detects missing separators (T033):
   
   T033 specified "extra/missing //", but the Phase 3 commit only
   detected extra separators (back-to-back `////`). The missing case
   (`SECRET/NOFORN` with a single slash) produced zero diagnostics
   because the parser cannot split on `//` and lands the entire marking
   in one Classification block.
   
   Extended E004 to walk Classification and Unknown TokenSpans for
   single `/` bytes that are not adjacent to another `/` (i.e., not part
   of a `//` separator). Each stray slash becomes one diagnostic with
   span = the single byte, original = "/", replacement = "//",
   confidence 0.99. Works in two cases:
   
     - `SECRET/NOFORN` — the whole banner lands as a single
       Classification block with text containing `/`. E004 walks the
       byte range and emits a fix at the stray slash.
     - `SECRET//SI/NF` — the parser splits on `//`, leaving `SI/NF` in
       an Unknown block. E004 still catches the stray slash inside the
       Unknown token's text.
   
   E008 already skipped Unknown tokens whose text is in the migration
   table (E007 territory), and it continues to fire alongside E004 on
   the same Unknown block — the two diagnostics point at different byte
   offsets so they do not collide in the overlap guard.
   
   D.1 — marque-config::load walks upward for .marque.toml (FR-007):
   
   The previous implementation checked only `project_root.join(
   ".marque.toml")` — exactly one directory. Running `marque check` from
   any subdirectory of a real project silently ignored `.marque.toml` at
   the root and ran with built-in defaults. This is a correctness
   failure against FR-007 and contracts/cli.md §"Configuration
   discovery", which mandates an upward walk stopping at the first of:
   
     (a) a directory containing `.marque.toml`
     (b) a directory containing `.git/` (git repo root)
     (c) the filesystem root
   
   Implemented `discover_project_dir(start)` with exactly those stop
   conditions. A repo whose root has BOTH `.git/` AND `.marque.toml` is
   the common case and must succeed — the walk checks `.marque.toml`
   before `.git/` inside each directory so the `.git/` stop does not
   defeat a co-located project config.
   
   Local config (.marque.local.toml) is searched only in the same
   directory as the discovered `.marque.toml`, never independently
   walked. This matches the contract's explicit clause: "a stray
   .marque.local.toml in a parent directory cannot silently attach to
   a child project's config."
   
   For `--config <PATH>`, which the contract says "short-circuits the
   walk and uses the specified path as the project config", added a
   separate `load_with_explicit_config(path)` entry point. The CLI's
   `load_config` dispatches between `load` (upward walk from cwd) and
   `load_with_explicit_config` (no walk, use exact path + look for
   local config in the same directory) based on whether `--config` was
   passed.
   
   Seven new marque-config tests cover:
     - discover finds .marque.toml in start dir
     - discover walks upward for .marque.toml
     - discover stops at .git root without .marque.toml
     - discover returns .marque.toml when both it and .git are at root
     - load walks upward to find project config
     - load returns defaults when walk finds nothing
     - load's local-config search is anchored to the project dir,
       NOT independently walked (rejects sub/.marque.local.toml when
       the project config lives at the parent)
   
   F.1 — PageContext reset semantics are now observably tested:
   
   The existing `lint_handles_multi_page_document_with_form_feed` test
   used `engine_with(vec![])` — a stub rule that emits no diagnostics
   regardless of context. It only verified that `Engine::lint` did not
   panic on multi-page input. A silent bug where banner #2 inherited
   the full document's accumulated portions would have passed that
   test.
   
   Added ContextRecorderRule, a stub that captures the live
   `ctx.page_context.portion_count()` per check invocation via an
   Arc<Mutex<Vec<(MarkingType, usize)>>>. Two new tests:
   
     - page_context_resets_observably_across_form_feed:
       Two-page input with one portion + one banner per page, separated
       by `\f`. Asserts banner #1 saw 1 accumulated portion and banner #2
       ALSO saw 1 (not 2) — proving the form feed reset PageContext.
   
     - page_context_lint_starts_fresh_on_each_call:
       Calls `engine.lint(src)` twice on the same engine and asserts
       both calls produce identical observations. Guards against any
       future refactor that leaks PageContext state between lint calls
       via e.g. caching.
   
   New corpus fixture:
     - tests/corpus/invalid/separator_count_missing.{txt,expected.json}
       — `SECRET/NOFORN` exercises the E004 missing-separator path via
       the integration test harness.

### Other

 - <csr-id-c1574766cf76e985bc8955128580a5da46c12626/> Improved parsing and token handling, expanded validation using official tests, demo pilot

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 38 commits contributed to the release over the course of 33 calendar days.
 - 19 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 2 unique issues were worked on: [#12](https://github.com/marquetools/marque/issues/12), [#14](https://github.com/marquetools/marque/issues/14)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#12](https://github.com/marquetools/marque/issues/12)**
    - Non-IC dissemination controls (LIMDIS, SBU, LES, SSI) ([`314d905`](https://github.com/marquetools/marque/commit/314d9055cf311a4a5b45347a400482ca843c75d0))
 * **[#14](https://github.com/marquetools/marque/issues/14)**
    - Improved parsing and token handling, expanded validation using official tests, demo pilot ([`c157476`](https://github.com/marquetools/marque/commit/c1574766cf76e985bc8955128580a5da46c12626))
 * **Uncategorized**
    - Bump versions and release to resolve crates.io backlog ([`073f17d`](https://github.com/marquetools/marque/commit/073f17d77c3f54da21c397ff1bdcc71248272a00))
    - Spelling corrections and config exemptions ([`f90ab0d`](https://github.com/marquetools/marque/commit/f90ab0da894be888b3cc7e3fcba2f3b72a84b46d))
    - Fixed formating and linting issues in multiple places; corrected an issue where the release action did not generate a workspace version or changelog ([`d0d7b2d`](https://github.com/marquetools/marque/commit/d0d7b2df50a4c62167f7b0b4d5c8914a0c8400d7))
    - Fixed formating and linting issues in multiple places; corrected an issue where the release action did not generate a workspace version or changelog ([`f211cea`](https://github.com/marquetools/marque/commit/f211cea82893ab678cbd7f01f47e47f41697a0d1))
    - Release marque-ism v0.1.0, marque-rules v0.1.0, marque-core v0.1.0, marque-config v0.1.0, marque-extract v0.1.0, marque-capco v0.1.0, marque-engine v0.1.0, marque-wasm v0.1.0, marque-server v0.1.0, marque v0.1.0 ([`c6981d4`](https://github.com/marquetools/marque/commit/c6981d4b030b98ec407cfef1319c5dc6579c8cef))
    - Merge pull request #10 from marquetools/ci/pipeline-setup ([`a406810`](https://github.com/marquetools/marque/commit/a406810185423e5f37f6f8b81ec5c8b5b5f25eff))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfix: simplify boolean expression in corpus_provenance test[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mClippy nonminimal_bool lint: replace !x.is_some_and(|e| e == "txt")[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231mwith x.is_none_or(|e| e != "txt").[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`caeab46`](https://github.com/marquetools/marque/commit/caeab46f20a45b3ba76cb65751edd285e605a588))
    - Merge pull request #9 from marquetools/001-marque-mvp ([`e42ffd1`](https://github.com/marquetools/marque/commit/e42ffd104294dffca280164a08e2080174031d5e))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfix(phase-7): address four LOW review findings from pr-9 review[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mL-1: Rename baseline.json keys from p50/p95/p99 to lower_ci/mean/upper_ci[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231m     to accurately reflect that values are criterion confidence interval[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;231m     bounds, not percentile distribution samples. Update bench-check.sh[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;231m     to read the renamed key.[0m [38;5;238m   7[0m [38;5;238m│[0m [38;5;238m   8[0m [38;5;238m│[0m [38;5;231mL-2: Use input.len() for BenchmarkId in linear_scaling so axis labels[0m [38;5;238m   9[0m [38;5;238m│[0m [38;5;231m     match actual input size after block-aligned truncation.[0m [38;5;238m  10[0m [38;5;238m│[0m [38;5;238m  11[0m [38;5;238m│[0m [38;5;231mL-3: Add numeric validation for baseline value before interpolating[0m [38;5;238m  12[0m [38;5;238m│[0m [38;5;231m     into Python inline script in bench-check.sh.[0m [38;5;238m  13[0m [38;5;238m│[0m [38;5;238m  14[0m [38;5;238m│[0m [38;5;231mL-4: Add TEST-WASM-42 to ALLOWED_SENTINELS in no_classifier_id_in_commits.rs[0m [38;5;238m  15[0m [38;5;238m│[0m [38;5;231m     so the SC-006 scanner does not rely on raw-string parsing behavior.[0m [38;5;238m  16[0m [38;5;238m│[0m [38;5;238m  17[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`d54a4c9`](https://github.com/marquetools/marque/commit/d54a4c9736d1f118323aed58fbf2fef1c21b1037))
    - Merge pull request #8 from marquetools/001-marque-mvp ([`52670d5`](https://github.com/marquetools/marque/commit/52670d531c6b5b4b9189b69c390196ab26803614))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfix(phase-6): address PR review findings M-1, M-2, L-2[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mM-1: Cache AhoCorasick automaton at Engine construction time instead[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231m     of rebuilding on every lint() call. The automaton and active[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;231m     (key, value) pairs are stored as CachedAhoCorasick on the Engine[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;231m     struct, built once in with_clock().[0m [38;5;238m   7[0m [38;5;238m│[0m [38;5;238m   8[0m [38;5;238m│[0m [38;5;231mM-2: Log tracing::warn! when AhoCorasick::new fails instead of[0m [38;5;238m   9[0m [38;5;238m│[0m [38;5;231m     silently skipping the pre-scanner corrections pass.[0m [38;5;238m  10[0m [38;5;238m│[0m [38;5;238m  11[0m [38;5;238m│[0m [38;5;231mL-2: Change marque-capco from path = to workspace = true in both[0m [38;5;238m  12[0m [38;5;238m│[0m [38;5;231m     marque/Cargo.toml and marque-server/Cargo.toml for consistency[0m [38;5;238m  13[0m [38;5;238m│[0m [38;5;231m     with other workspace deps.[0m [38;5;238m  14[0m [38;5;238m│[0m [38;5;238m  15[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`e51f8ad`](https://github.com/marquetools/marque/commit/e51f8ad5cbc3349ee4207692d80669be43d69a0c))
    - Address PR review comments — null→undefined, workspace dep, aho-corasick scan ([`4dd20f1`](https://github.com/marquetools/marque/commit/4dd20f13bdd799192442e383be04b9e9caaa8b30))
    - Merge pull request #6 from marquetools/001-marque-mvp ([`ae1b97c`](https://github.com/marquetools/marque/commit/ae1b97c63246e146fc4c79d819ff5183f21d795c))
    - Apply suggestions from code review ([`a9dff8e`](https://github.com/marquetools/marque/commit/a9dff8e5fa49013d215853c2699e0271cadc7f41))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfix(phase-5): address 8 PR review findings for merge readiness[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mHIGH:[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231m  Engine::new delegates to with_clock, eliminating duplicate[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;231m  corrections_arc construction logic.[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;238m   7[0m [38;5;238m│[0m [38;5;231mMEDIUM:[0m [38;5;238m   8[0m [38;5;238m│[0m [38;5;231m  ENV_MUTEX doc comment now explicitly states within-binary scope[0m [38;5;238m   9[0m [38;5;238m│[0m [38;5;231m  and warns about cross-binary limitations.[0m [38;5;238m  10[0m [38;5;238m│[0m [38;5;231m  C001 migration_ref changed from "corrections-map" to None — the[0m [38;5;238m  11[0m [38;5;238m│[0m [38;5;231m  source field (FixSource::CorrectionsMap) already carries provenance;[0m [38;5;238m  12[0m [38;5;238m│[0m [38;5;231m  migration_ref is reserved for CAPCO document citations.[0m [38;5;238m  13[0m [38;5;238m│[0m [38;5;231m  Config::corrections field gains doc comment warning against[0m [38;5;238m  14[0m [38;5;238m│[0m [38;5;231m  post-Engine-construction mutation (cached Arc would go stale).[0m [38;5;238m  15[0m [38;5;238m│[0m [38;5;238m  16[0m [38;5;238m│[0m [38;5;231mLOW:[0m [38;5;238m  17[0m [38;5;238m│[0m [38;5;231m  Test name capco_rule_set_registers_all_phase3_rules renamed to[0m [38;5;238m  18[0m [38;5;238m│[0m [38;5;231m  capco_rule_set_registers_all_rules.[0m [38;5;238m  19[0m [38;5;238m│[0m [38;5;231m  explain_config test uses exact array assertion instead of contains.[0m [38;5;238m  20[0m [38;5;238m│[0m [38;5;231m  Added c001_fires_only_on_matching_token_in_multi_token_marking test.[0m [38;5;238m  21[0m [38;5;238m│[0m [38;5;238m  22[0m [38;5;238m│[0m [38;5;231mTests 222 → 223.[0m [38;5;238m  23[0m [38;5;238m│[0m [38;5;238m  24[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`ef3a0f4`](https://github.com/marquetools/marque/commit/ef3a0f4593818b04b369ed11a3263aea4e54b789))
    - Address 6 review findings from PR code review ([`521bbd9`](https://github.com/marquetools/marque/commit/521bbd9525310b117a76c1f1ce2e181e315544f3))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfix(phase-5): address 14 review findings from Rust + QA specialists[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mCode fixes:[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231m  F-02: apply non_empty() guard on MARQUE_CLASSIFIER_ID env var so[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;231m        empty string does not overwrite populated local-config value[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;231m  M1:   skip TokenKind::Separator in C001 corrections-map rule[0m [38;5;238m   7[0m [38;5;238m│[0m [38;5;231m  M2:   skip no-op corrections where replacement == original text[0m [38;5;238m   8[0m [38;5;238m│[0m [38;5;231m  M3:   remove dead corrections.is_empty() guard (engine invariant)[0m [38;5;238m   9[0m [38;5;238m│[0m [38;5;231m  F-12: add test-only exception comment on render.rs __engine_promote[0m [38;5;238m  10[0m [38;5;238m│[0m [38;5;238m  11[0m [38;5;238m│[0m [38;5;231mTest fixes:[0m [38;5;238m  12[0m [38;5;238m│[0m [38;5;231m  F-03/04/05: make all corrections_map.rs assertions unconditional;[0m [38;5;238m  13[0m [38;5;238m│[0m [38;5;231m              fix c001_fires test to use valid scanner input (NF→NOFORN)[0m [38;5;238m  14[0m [38;5;238m│[0m [38;5;231m  F-09:  add rule ID assertion to severity_override_downgrades test[0m [38;5;238m  15[0m [38;5;238m│[0m [38;5;231m  F-11:  add hard_fail_env_threshold_nan test[0m [38;5;238m  16[0m [38;5;238m│[0m [38;5;231m  F-13:  add us3_acceptance_scenario_combined test for exact spec input[0m [38;5;238m  17[0m [38;5;238m│[0m [38;5;231m  F-07:  add test documenting local config cannot override rule severity[0m [38;5;238m  18[0m [38;5;238m│[0m [38;5;231m  F-08:  add cli_confidence_threshold_overrides_config (Layer 4 test)[0m [38;5;238m  19[0m [38;5;238m│[0m [38;5;231m  F-06:  add sc002a_fixture_tokens_within_known_vocabulary (T055a-d)[0m [38;5;238m  20[0m [38;5;238m│[0m [38;5;231m  S1:    extend SC-006 scanner to cover marque/tests/, add sentinels[0m [38;5;238m  21[0m [38;5;238m│[0m [38;5;231m  F-02:  add empty_env_classifier_id_does_not_overwrite_local test[0m [38;5;238m  22[0m [38;5;238m│[0m [38;5;238m  23[0m [38;5;238m│[0m [38;5;231mTests rise 214 → 222.[0m [38;5;238m  24[0m [38;5;238m│[0m [38;5;238m  25[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`311ef07`](https://github.com/marquetools/marque/commit/311ef074dca41c9d953c7c86253f296a0d9011aa))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfeat(phase-5): implement US3 — configurable severity, corrections map, and C001 rule[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mAdd corrections-map rule C001 that scans token spans against the[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231m[corrections] table in .marque.toml, emitting FixSource::CorrectionsMap[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;231mwith confidence 1.0. FR-009 precedence (user corrections win over[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;231mbuilt-in rules on the same span) is automatic under FR-016 sort order.[0m [38;5;238m   7[0m [38;5;238m│[0m [38;5;238m   8[0m [38;5;238m│[0m [38;5;231mWire the corrections map from Config through Engine::lint into[0m [38;5;238m   9[0m [38;5;238m│[0m [38;5;231mRuleContext.corrections as Option<Arc<HashMap>> so rules access it[0m [38;5;238m  10[0m [38;5;238m│[0m [38;5;231mwithout owning config state.[0m [38;5;238m  11[0m [38;5;238m│[0m [38;5;238m  12[0m [38;5;238m│[0m [38;5;231mAdd 33 new tests (181 → 214):[0m [38;5;238m  13[0m [38;5;238m│[0m [38;5;231m  - T052: 13 config precedence chain tests (4-layer: committed → local → env → CLI)[0m [38;5;238m  14[0m [38;5;238m│[0m [38;5;231m  - T053: hard-fail validators ([user] in committed, schema mismatch, threshold OOR)[0m [38;5;238m  15[0m [38;5;238m│[0m [38;5;231m  - T054: 7 corrections-map precedence + classifier_id propagation tests[0m [38;5;238m  16[0m [38;5;238m│[0m [38;5;231m  - T055: SC-006 classifier-id-in-commits repo-wide guard[0m [38;5;238m  17[0m [38;5;238m│[0m [38;5;231m  - T055a: SC-002a corpus provenance scan (patterns, reviewer, no PII)[0m [38;5;238m  18[0m [38;5;238m│[0m [38;5;231m  - CLI: 9 integration tests (--explain-config, severity override, audit NDJSON)[0m [38;5;238m  19[0m [38;5;238m│[0m [38;5;238m  20[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`62c69c6`](https://github.com/marquetools/marque/commit/62c69c6d7c190d76e78ac8ae4d3f78be7f5b68d6))
    - Merge pull request #5 from marquetools/001-marque-mvp ([`283145b`](https://github.com/marquetools/marque/commit/283145b5b131370b27dd096958bceb5d9c0ed90b))
    - Address Copilot PR comments — UTF-8 validation + tempdir tests ([`b23c52c`](https://github.com/marquetools/marque/commit/b23c52c4c524ab37aec9d38a124df5209b48a5d1))
    - Address PR #5 review — NDJSON stream purity + threshold validation ([`33147c9`](https://github.com/marquetools/marque/commit/33147c96fd0ea3a830ad43b945e648c5e008425f))
    - Address three review findings in audit emission loop ([`ef1a6c9`](https://github.com/marquetools/marque/commit/ef1a6c9bc95db1f3e9102b54145c8fe026333227))
    - Address review findings — 6 code fixes + 10 test gaps ([`15b5957`](https://github.com/marquetools/marque/commit/15b595796b518df62ea7438909c5efa8432055c4))
    - Implement US2 auto-fix with NDJSON audit trail ([`73febea`](https://github.com/marquetools/marque/commit/73febea8c2c7d225ed1ab94d7fdbc62073ed4106))
    - Merge pull request #4 from marquetools/001-marque-mvp ([`57e3833`](https://github.com/marquetools/marque/commit/57e3833b37ea7af3917b58515bf65f75656fbfc7))
    - Address reviewer-identified items R-1 through R-4 ([`6097810`](https://github.com/marquetools/marque/commit/60978104ab8ee7806f8157fa4d99fdfd0ed8f41f))
    - Address three MEDIUM review findings from pr-4 pass ([`8bb899a`](https://github.com/marquetools/marque/commit/8bb899afb111e894ca60d6c05df6ddb940cea05c))
    - Address all 5 PR review findings ([`1fc4bff`](https://github.com/marquetools/marque/commit/1fc4bff2d9ff6b443ce7958b4b249793d550579d))
    - Address MEDIUM/LOW review findings ([`71e98eb`](https://github.com/marquetools/marque/commit/71e98eb208526ead45a420fcbd9dbc7129d747b3))
    - Address HIGH review findings (C.1, C.2, D.1, F.1) ([`842ada8`](https://github.com/marquetools/marque/commit/842ada81b8d1b3e20d926d96f27a58e0b8ad161f))
    - Implement Phase 3 — US1 lint with byte-precise spans + CLI ([`243e4c9`](https://github.com/marquetools/marque/commit/243e4c9ffbeacfa37a9058b123f109a286f5cadd))
    - Merge pull request #3 from marquetools/001-marque-mvp ([`44451ab`](https://github.com/marquetools/marque/commit/44451abdf0124c148d54f115184d046db396b297))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfeat: implement Phase 2 — CVE codegen, engine pipeline, validators[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mPhase 2 (Foundational) lands the real ODNI CVE XML → Rust codegen,[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231mthe FixProposal/AppliedFix type split, FR-016 deterministic fix[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;231mordering with overlap guard, layered config with hard-fail[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;231mvalidators, and a Clock-injected engine for deterministic audit[0m [38;5;238m   7[0m [38;5;238m│[0m [38;5;231mtests. Also addresses 14 review findings from the Phase 2[0m [38;5;238m   8[0m [38;5;238m│[0m [38;5;231mlocal review (4 CRITICAL + 10 HIGH + 10 MEDIUM + 9 LOW).[0m [38;5;238m   9[0m [38;5;238m│[0m [38;5;238m  10[0m [38;5;238m│[0m [38;5;231mPhase 2 core work:[0m [38;5;238m  11[0m [38;5;238m│[0m [38;5;231m- marque-ism/build.rs: real CVE XML and XSD parsing producing[0m [38;5;238m  12[0m [38;5;238m│[0m [38;5;231m  typed enums (SciControl, DissemControl, SarIdentifier,[0m [38;5;238m  13[0m [38;5;238m│[0m [38;5;231m  DeclassExemption, ExemptFrom) + TRIGRAPHS + ALL_CVE_TOKENS +[0m [38;5;238m  14[0m [38;5;238m│[0m [38;5;231m  validators + migrations, with T010 schema-version pinning[0m [38;5;238m  15[0m [38;5;238m│[0m [38;5;231m- marque-rules: FixProposal (pure, timestamp-free) vs AppliedFix[0m [38;5;238m  16[0m [38;5;238m│[0m [38;5;231m  (engine-only promotion via __engine_promote), FixSource enum,[0m [38;5;238m  17[0m [38;5;238m│[0m [38;5;231m  Severity::Off as first variant, Diagnostic carries citation[0m [38;5;238m  18[0m [38;5;238m│[0m [38;5;231m- marque-engine: Engine with Clock injection, FixMode::{Apply,[0m [38;5;238m  19[0m [38;5;238m│[0m [38;5;231m  DryRun}, Config-driven severity overrides, FR-016 sort[0m [38;5;238m  20[0m [38;5;238m│[0m [38;5;231m- marque-config: layered load with FR-010 (committed [user][0m [38;5;238m  21[0m [38;5;238m│[0m [38;5;231m  refusal), FR-011 (schema version match), confidence_threshold[0m [38;5;238m  22[0m [38;5;238m│[0m [38;5;231m  validation, MARQUE_CONFIDENCE_THRESHOLD env binding[0m [38;5;238m  23[0m [38;5;238m│[0m [38;5;231m- marque-capco: E001 banner abbreviation, E002 missing/misordered[0m [38;5;238m  24[0m [38;5;238m│[0m [38;5;231m  USA trigraph, E004 separator count (stub), E005 declass in banner[0m [38;5;238m  25[0m [38;5;238m│[0m [38;5;231m- marque-core/parser: uses generated enum parse() methods for[0m [38;5;238m  26[0m [38;5;238m│[0m [38;5;231m  SCI/dissem/SAR/declass; Classification remains hand-written[0m [38;5;238m  27[0m [38;5;238m│[0m [38;5;231m- Clock trait + SystemClock/FixedClock for deterministic audit[0m [38;5;238m  28[0m [38;5;238m│[0m [38;5;231m  timestamps in tests[0m [38;5;238m  29[0m [38;5;238m│[0m [38;5;238m  30[0m [38;5;238m│[0m [38;5;231mCritical review findings:[0m [38;5;238m  31[0m [38;5;238m│[0m [38;5;231m- C-1 Overlap guard in Engine::fix_inner drops overlapping fixes[0m [38;5;238m  32[0m [38;5;238m│[0m [38;5;231m  after FR-016 sort; dropped fixes surface in remaining_diagnostics[0m [38;5;238m  33[0m [38;5;238m│[0m [38;5;231m  so they cannot be silently lost[0m [38;5;238m  34[0m [38;5;238m│[0m [38;5;231m- C-2 MARQUE_CONFIDENCE_THRESHOLD parse failure returns[0m [38;5;238m  35[0m [38;5;238m│[0m [38;5;231m  ConfigError::InvalidEnvVar instead of silently using default[0m [38;5;238m  36[0m [38;5;238m│[0m [38;5;231m- C-3 Dropped latent dual-Classification ambiguity: build.rs no[0m [38;5;238m  37[0m [38;5;238m│[0m [38;5;231m  longer emits a generated Classification enum (canonical lives[0m [38;5;238m  38[0m [38;5;238m│[0m [38;5;231m  in attrs.rs); validator uses literal matches![0m [38;5;238m  39[0m [38;5;238m│[0m [38;5;231m- C-4 to_rust_ident resolves idents through resolve_idents which[0m [38;5;238m  40[0m [38;5;238m│[0m [38;5;231m  asserts non-empty and unique variants at build time[0m [38;5;238m  41[0m [38;5;238m│[0m [38;5;238m  42[0m [38;5;238m│[0m [38;5;231mHigh review findings:[0m [38;5;238m  43[0m [38;5;238m│[0m [38;5;231m- H-1 Server calls marque_config::load() and runs FR-011 check[0m [38;5;238m  44[0m [38;5;238m│[0m [38;5;231m  instead of Config::default() fallback[0m [38;5;238m  45[0m [38;5;238m│[0m [38;5;231m- H-2/H-3 Engine::fix_with_threshold plumbs --confidence CLI flag[0m [38;5;238m  46[0m [38;5;238m│[0m [38;5;231m  and FixRequest.confidence_threshold; InvalidThreshold error type[0m [38;5;238m  47[0m [38;5;238m│[0m [38;5;231m- H-4 CLI current_dir() failure → exit 74 (EX_IOERR); load[0m [38;5;238m  48[0m [38;5;238m│[0m [38;5;231m  failures exit with ConfigError::exit_code instead of silent[0m [38;5;238m  49[0m [38;5;238m│[0m [38;5;231m  Config::default fallback[0m [38;5;238m  50[0m [38;5;238m│[0m [38;5;231m- H-5 Trigraph inner field private; try_new enforces ASCII[0m [38;5;238m  51[0m [38;5;238m│[0m [38;5;231m  uppercase; as_str is infallible via from_utf8_unchecked with[0m [38;5;238m  52[0m [38;5;238m│[0m [38;5;231m  a SAFETY comment documenting the invariant[0m [38;5;238m  53[0m [38;5;238m│[0m [38;5;231m- H-6 marque-config validates rule severity strings at load time[0m [38;5;238m  54[0m [38;5;238m│[0m [38;5;231m  via Severity::parse_config; ConfigError::UnknownSeverity on typo[0m [38;5;238m  55[0m [38;5;238m│[0m [38;5;231m- H-7 applied_fixes is HashSet<(RuleId, Span)> for O(1) filter[0m [38;5;238m  56[0m [38;5;238m│[0m [38;5;231m- H-8 ALL_CVE_TOKENS collected via BTreeSet (sorted, deduplicated)[0m [38;5;238m  57[0m [38;5;238m│[0m [38;5;231m  so canonicalize uses binary_search; NOFORN/ORCON/PROPIN/IMCON[0m [38;5;238m  58[0m [38;5;238m│[0m [38;5;231m  duplicates eliminated[0m [38;5;238m  59[0m [38;5;238m│[0m [38;5;231m- H-9 parse_classification doc explains it is the single[0m [38;5;238m  60[0m [38;5;238m│[0m [38;5;231m  deliberately hand-coded path (both portion and banner forms)[0m [38;5;238m  61[0m [38;5;238m│[0m [38;5;231m- H-10 45 unit tests backfill core-crate coverage[0m [38;5;238m  62[0m [38;5;238m│[0m [38;5;238m  63[0m [38;5;238m│[0m [38;5;231mMedium/Low:[0m [38;5;238m  64[0m [38;5;238m│[0m [38;5;231m- FixProposal::new confidence is assert! (release-mode), not[0m [38;5;238m  65[0m [38;5;238m│[0m [38;5;231m  debug_assert — NaN and INFINITY are correctness bugs in release[0m [38;5;238m  66[0m [38;5;238m│[0m [38;5;231m- Span::new upgraded to assert!; Span::try_as_slice checked variant[0m [38;5;238m  67[0m [38;5;238m│[0m [38;5;231m- RuleContext zone/position carry TODO(phase-3) marker[0m [38;5;238m  68[0m [38;5;238m│[0m [38;5;231m- AppliedFix classifier_id and input are Arc<str> for O(1) clone[0m [38;5;238m  69[0m [38;5;238m│[0m [38;5;231m- BatchEngine lint_many/fix_many yield Result<_, BatchError>[0m [38;5;238m  70[0m [38;5;238m│[0m [38;5;231m  instead of aborting the process on JoinError[0m [38;5;238m  71[0m [38;5;238m│[0m [38;5;231m- Severity has as_str + Display; CLI/server/wasm use it instead[0m [38;5;238m  72[0m [38;5;238m│[0m [38;5;231m  of format!("{:?}")[0m [38;5;238m  73[0m [38;5;238m│[0m [38;5;231m- RuleId inner field private with new/as_str accessors[0m [38;5;238m  74[0m [38;5;238m│[0m [38;5;231m- FixedClock tuple field private with new constructor[0m [38;5;238m  75[0m [38;5;238m│[0m [38;5;231m- LintResult::fix_count requires d.fix.is_some() AND Severity::Fix[0m [38;5;238m  76[0m [38;5;238m│[0m [38;5;231m- Eliminated double source.to_vec() allocation in FixMode::DryRun[0m [38;5;238m  77[0m [38;5;238m│[0m [38;5;231m- make_fix_diagnostic takes FixDiagnosticParams struct[0m [38;5;238m  78[0m [38;5;238m│[0m [38;5;231m- Removed unused thiserror/phf/anyhow runtime deps from marque-ism[0m [38;5;238m  79[0m [38;5;238m│[0m [38;5;231m- marque_ism::SCHEMA_VERSION re-exported at crate root[0m [38;5;238m  80[0m [38;5;238m│[0m [38;5;231m- run_metadata exits 69 (EX_UNAVAILABLE) instead of 0[0m [38;5;238m  81[0m [38;5;238m│[0m [38;5;238m  82[0m [38;5;238m│[0m [38;5;231mTests: 45 unit + 1 doctest, zero failures. Clippy clean across[0m [38;5;238m  83[0m [38;5;238m│[0m [38;5;231m--all-targets. rustfmt clean.[0m [38;5;238m  84[0m [38;5;238m│[0m [38;5;238m  85[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`dacea46`](https://github.com/marquetools/marque/commit/dacea460225b1ccdd046ca90bcb822abfe4156e4))
    - Merge pull request #1 from marquetools/001-marque-mvp ([`65fcc2a`](https://github.com/marquetools/marque/commit/65fcc2afb1f0a5491e2ff9a93484b000b7bb52e8))
    - [38;5;238m─────┬──────────────────────────────────────────────────────────────────────────[0m      [38;5;238m│ [0m[1mSTDIN[0m [38;5;238m─────┼──────────────────────────────────────────────────────────────────────────[0m [38;5;238m   1[0m [38;5;238m│[0m [38;5;231mfeat: implement Phase 1 setup — create marque-ism crate and dev scaffolding[0m [38;5;238m   2[0m [38;5;238m│[0m [38;5;238m   3[0m [38;5;238m│[0m [38;5;231mExtract shared ISM vocabulary types (Span, IsmAttributes, TokenSet,[0m [38;5;238m   4[0m [38;5;238m│[0m [38;5;231mCapcoTokenSet) and build.rs codegen into new marque-ism leaf crate,[0m [38;5;238m   5[0m [38;5;238m│[0m [38;5;231mresolving the circular dependency between marque-core and marque-capco.[0m [38;5;238m   6[0m [38;5;238m│[0m [38;5;231mMove ODNI schemas from marque-capco/schemas/ to marque-ism/schemas/.[0m [38;5;238m   7[0m [38;5;238m│[0m [38;5;238m   8[0m [38;5;238m│[0m [38;5;231mPhase 1 deliverables (T000–T005a):[0m [38;5;238m   9[0m [38;5;238m│[0m [38;5;231m- T000: marque-ism crate with span, attrs, token_set, generated modules[0m [38;5;238m  10[0m [38;5;238m│[0m [38;5;231m- T001: All 11 workspace crates compile clean (clippy -D warnings)[0m [38;5;238m  11[0m [38;5;238m│[0m [38;5;231m- T002/T002a/T002b: Test corpus scaffolding with contract and provenance docs[0m [38;5;238m  12[0m [38;5;238m│[0m [38;5;231m- T003: Criterion benchmark skeletons (lint_latency, linear_scaling)[0m [38;5;238m  13[0m [38;5;238m│[0m [38;5;231m- T004: marque-test-utils corpus loader crate (dev-dependency)[0m [38;5;238m  14[0m [38;5;238m│[0m [38;5;231m- T005: scripts/check.sh (fmt + clippy + nextest)[0m [38;5;238m  15[0m [38;5;238m│[0m [38;5;231m- T005a: .config/nextest.toml with default + ci profiles[0m [38;5;238m  16[0m [38;5;238m│[0m [38;5;238m  17[0m [38;5;238m│[0m [38;5;231mmarque-core retains thin re-exports for backward compatibility.[0m [38;5;238m  18[0m [38;5;238m│[0m [38;5;231mmarque-rules now depends directly on marque-ism (not marque-core).[0m [38;5;238m  19[0m [38;5;238m│[0m [38;5;238m  20[0m [38;5;238m│[0m [38;5;231mCo-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>[0m [38;5;238m─────┴──────────────────────────────────────────────────────────────────────────[0m ([`71e1f11`](https://github.com/marquetools/marque/commit/71e1f1170de99d676ef6feb834cc55c0f04c1d31))
    - Initial commit from Specify template ([`d3154b9`](https://github.com/marquetools/marque/commit/d3154b935b4fc5d312fef184136d42689cc6aad7))
</details>

