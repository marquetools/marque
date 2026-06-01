use super::fix::{PrePass1Cache, TwoPassFixer, into_secret_slice, partition_diags_by_phase};
use super::synthesis::{
    apply_fr023_and_i18, build_r002_diagnostic, find_containing_marking, lookup_marking,
    sort_and_c1_dedup, span_is_within_marking, synthesize_fixes,
};
use super::*;

/// Outcome of pass-0 (text-corrections, UNCHANGED behavior).
///
/// `effective_source` is wrapped in [`Zeroizing`] per Constitution
/// Principle II — Marque-owned scratch buffers wipe on drop.
pub(super) struct Pass0Result<S: MarkingScheme = CapcoScheme> {
    /// Source bytes after pass-0 text corrections have been applied.
    /// Equals `source.to_vec()` when no text corrections fired.
    pub(super) effective_source: Zeroizing<Vec<u8>>,
    /// Promoted `marque-1.0` audit-line records from pass-0. Each
    /// entry is an [`AuditLine::TextCorrection`] for the pass-0
    /// path.
    pub(super) audit_lines: Vec<AuditLine<S>>,
    /// Diagnostics whose text-correction fixes were dropped by the
    /// C-1 overlap guard during pass-0. Surfaced via
    /// `FixResult.remaining_diagnostics` because pass-2's re-lint
    /// runs on the corrected buffer and would not re-emit them.
    pub(super) dropped_diags: Vec<Diagnostic<S>>,
}

/// Outcome of pass-1 ([`Phase::Localized`] rule fixes).
///
/// `post_buffer` is wrapped in [`Zeroizing`] per Constitution
/// Principle II.
pub(super) struct Pass1Result<S: MarkingScheme = CapcoScheme> {
    /// Buffer after pass-1 fixes have been spliced into `effective_source`.
    /// Equals `effective_source` when pass-1 produced no fixes.
    pub(super) post_buffer: Zeroizing<Vec<u8>>,
    /// Promoted `marque-1.0` audit-line records from pass-1. Each
    /// entry is an [`AuditLine::AppliedFix`] for the pass-1 marking
    /// path.
    pub(super) audit_lines: Vec<AuditLine<S>>,
    /// `(rule_id, span)` keys of pass-1 fixes — feeds the
    /// `remaining_diagnostics` filter so a fixed diagnostic is not
    /// reported again.
    pub(super) applied_keys: HashSet<(RuleId, Span)>,
}

/// Outcome of pass-2 ([`Phase::WholeMarking`] rule fixes).
///
/// `output` is wrapped in [`Zeroizing`] per Constitution Principle II.
/// On the happy path it transfers to [`FixResult.source`]
/// ([`SecretSlice<u8>`]) via [`into_secret_slice`] — the wipe
/// guarantee flows from the scratch wrapper to the public wrapper.
pub(super) struct Pass2Result<S: MarkingScheme = CapcoScheme> {
    /// Final buffer (Apply mode) or original source (DryRun).
    pub(super) output: Zeroizing<Vec<u8>>,
    /// Promoted `marque-1.0` audit-line records from pass-2. Each
    /// entry is an [`AuditLine::AppliedFix`].
    pub(super) audit_lines: Vec<AuditLine<S>>,
    pub(super) applied_keys: HashSet<(RuleId, Span)>,
}

impl<'engine, S, R> TwoPassFixer<'engine, S, R>
where
    S: MarkingScheme + ConstraintBridge,
    S::Canonical: Clone + Default + PartialEq,
    R: Recognizer<S>,
{
    /// Run the full three-stage pipeline and produce a [`FixResult`].
    ///
    /// Error mapping mirrors the pre-7b `fix_inner` shape:
    /// - `Err(EngineError::DeadlineExceeded { partial_lint })` —
    ///   deadline expired at any boundary; per Constitution V Principle V
    ///   no partial `FixResult` ever escapes.
    /// - `Err(EngineError::InvalidThreshold(_))` — cannot fire from
    ///   inside `run`; the threshold is validated upstream in
    ///   `fix_with_options`.
    ///
    /// On R002 (post-pass-1 re-parse failure) `run` returns
    /// `Ok(FixResult { r002_fired: true, ..})` carrying the pass-1
    /// buffer + the union of pass-0 / pass-1 applied fixes + the
    /// synthetic R002 diagnostic. Pass-2 does NOT run.
    pub(super) fn run(self) -> Result<FixResult<S>, EngineError<S>> {
        let lint_opts = LintOptions {
            deadline: self.deadline,
            ..Default::default()
        };

        // #176 / SC-010: route the fix path's internal lint passes by
        // the caller's recognition input-source. `SchemaDocument`
        // normalizes to the conservative text path here (no schema
        // adapter ships for `S` yet), mirroring
        // `Engine::lint_with_input_context`. `StructuredField` lifts the
        // decoder's lone-case heuristic so `fix --input-source
        // structured-field` applies the assertive recovery the flag
        // promises.
        let input_source = match self.input_source {
            marque_scheme::InputSource::StructuredField => {
                marque_scheme::InputSource::StructuredField
            }
            // DocumentContent + SchemaDocument + any future
            // `#[non_exhaustive]` variant → conservative text path.
            _ => marque_scheme::InputSource::DocumentContent,
        };

        // Pass-0: lint original + apply text corrections.
        let (lint1, parsed_markings1) = self.engine.lint_with_options_internal_with_source(
            self.source,
            &lint_opts,
            None,
            input_source,
        );
        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded {
                partial_lint: lint1,
            });
        }
        let pass0 = self.run_pass0_c001(&lint1);

        // Pass-2 lint baseline: re-lint when pass-0 changed bytes
        // (the scanner sees a different source and can detect newly-
        // valid markings — e.g. `SERCET//NF` → `SECRET//NF` → E001
        // fires on `NF`). Move ownership of the chosen `(lint,
        // markings)` pair so synthesis can consume `parsed_markings`
        // without an explicit clone.
        let (lint, parsed_markings) = if !pass0.audit_lines.is_empty() {
            self.engine.lint_with_options_internal_with_source(
                &pass0.effective_source,
                &lint_opts,
                None,
                input_source,
            )
        } else {
            (lint1, parsed_markings1)
        };

        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded { partial_lint: lint });
        }

        // Build the (Localized rule id) lookup set once per run.
        // `additional_emitted_ids` IDs ride with their owning rule's
        // declared phase — a walker that registers as `Localized`
        // emits all its catalog IDs at pass-1.
        let localized_ids = self.localized_rule_id_set();

        // Pass-1 sees the pre-pass-1 partition (`pass1_diags`) of the
        // `lint` diagnostic stream. The reference vector borrows from
        // `lint.diagnostics`; we keep the borrow scoped to the pass-1
        // dispatch call so the borrow checker can let `lint` move
        // (or be replaced by `relint`) in the rebind below.
        //
        // We deliberately drop the pre-pass-1 `pass2_diags` partition
        // immediately after pass-1: the post-pass-1 rebind below may
        // replace `lint` with `relint`, and pass-2 in either branch
        // operates on the post-rebind `lint.diagnostics`. The fresh
        // re-partition is the correct pass-2 input. Reference
        // partitioning is O(N) pointer
        // pushes; the prior owned-vec clone allocated O(N) Diagnostic
        // bodies on every call (Constitution I).
        let pass1 = {
            let (pass1_diags, _pass2_diags_pre) =
                partition_diags_by_phase(&lint.diagnostics, &localized_ids);
            self.run_pass1_localized(
                &pass0.effective_source,
                &parsed_markings,
                &pass1_diags,
                &lint,
            )?
        };

        // Capture pre-pass-1 attrs for every marking whose span
        // overlaps a pass-1 applied fix. The cache is owned on this
        // stack frame so the references it spawns
        // (`RuleContext.pre_pass_1_attrs`) cannot outlive `run()`.
        // `parsed_markings` is still the pre-pass-1 cache at this
        // point — the re-parse arm below will move ownership of it
        // (replacing it with a fresh post-pass-1 cache), so the
        // snapshot has to land BEFORE that branch. Empty when pass-1
        // promoted no fixes; the field-only consumer
        // (`RuleContext.pre_pass_1_attrs`) sees `None` in that case.
        // The cache + the field stay as the architectural
        // two-pass-reshape signal for future rule consumers.
        let pre_pass_1_cache = self.populate_pre_pass_1_cache(&pass1.audit_lines, &parsed_markings);

        // Destructure pass0 into owned locals so the re-parse / R002
        // branches can move `effective_source` directly (eliminating
        // the prior `.clone()` of the full document buffer on the
        // hot path) while `audit_lines` and `dropped_diags` flow
        // through to the merge step or the R002 assembler without an
        // intermediate `&Pass0Result` borrow that forces cloning.
        let Pass0Result {
            effective_source: pass0_effective_source,
            audit_lines: pass0_audit_lines,
            dropped_diags: pass0_dropped_diags,
        } = pass0;

        // Re-parse decision. Short-circuit when pass-1 produced no
        // applied fixes — the byte stream is unchanged, so the
        // pass-2 lint baseline is identical to the post-pass-0 lint
        // and we can reuse `parsed_markings` AND the pre-pass-1
        // `pass2_diags` partition directly.
        //
        // `S::Canonical` is owned (no borrow parameter), and
        // parsed_markings is `Vec<(Span, S::Marking)>` (issue
        // #432 swapped the type from `HashMap` to a sorted `Vec`).
        // Moving it in both branches keeps both arms producing the
        // same owned type — no `Cow`, no clone (rust pre-flight Q3).
        //
        // Reshape-aware disambiguation uses the pre-pass-1 attrs cache
        // and the `(scheme, predicate-id) → no re-fire` gate: when
        // pass-1 changed bytes, the re-parse arm dispatches pass-2 against
        // **post-pass-1 attrs AND post-pass-1 diagnostics**, NOT
        // against the stale pre-pass-1 `pass2_diags` partition. Pass-1
        // may have shifted spans (so a pre-pass-1 diagnostic's span no
        // longer points at what the rule meant) or eliminated the
        // very condition a WholeMarking diagnostic flagged (so the
        // diagnostic is obsolete). The fresh re-lint reflects current
        // truth. On the no-fix short-circuit (`pass1.applied.is_empty()`),
        // the buffer didn't change, so the pre-pass-1 `pass2_diags`
        // partition is still current and is reused — saving one
        // re-lint pass on the hot path.
        //
        // `lint` is rebound to the post-pass-1 re-lint on the re-parse
        // arm so EVERY downstream consumer — the pass-2 dispatch, the
        // `DeadlineExceeded { partial_lint }` payload, and the
        // remaining-diagnostics filter — sees the same post-pass-1
        // state. Tupling `lint` through the decision is what makes
        // post-pass-1 propagation total rather than partial — every
        // call site reads the post-pass-1 `lint`, not the stale
        // pre-pass-1 one. The
        // R002 short-circuit explicitly passes the **pre-pass-1**
        // `lint` into `assemble_r002_result`: pass-1 destroyed the
        // marking shape, so post-pass-1 diagnostics are degenerate
        // and the surfaced remaining-diagnostics stream should
        // reflect what the operator saw before pass-1 ran.
        // Rebind `(pass2_source, pass2_markings, pass1_applied,
        // pass1_applied_keys, lint)` first — splitting `pass2_diags`
        // off into its own subsequent let-binding (below) is what
        // unblocks the reference propagation. With a single-tuple
        // shape, the `else`
        // arm tried to move both `relint` and a `Vec<&relint.diagnostics>`
        // out together, which Rust correctly rejects as a
        // self-referential bundle. Splitting lets `lint` settle to
        // its final owner first, then `pass2_diags` borrows from it.
        let (pass2_source, pass2_markings, pass1_audit_lines, pass1_applied_keys, lint) =
            if pass1.audit_lines.is_empty() {
                // Short-circuit: pass-1 produced no audit lines, so the
                // byte stream is unchanged. Move `pass0_effective_source`
                // directly into `pass2_source` — no document-buffer clone.
                // The pre-pass-1 `lint` is still current here (no byte
                // change → no re-lint needed) and is moved through.
                let Pass1Result {
                    post_buffer: _,
                    audit_lines,
                    applied_keys,
                } = pass1;
                (
                    pass0_effective_source,
                    parsed_markings,
                    audit_lines,
                    applied_keys,
                    lint,
                )
            } else {
                // Pass the pre-pass-1 attrs cache into the post-pass-1
                // re-lint so every candidate's RuleContext gets a
                // populated `pre_pass_1_attrs` field when its span
                // overlaps a pass-1-reshaped marking. The field is the
                // architectural two-pass-reshape signal kept for future
                // rule consumers.
                let (relint, new_markings) = self.engine.lint_with_options_internal_with_source(
                    &pass1.post_buffer,
                    &lint_opts,
                    Some(&pre_pass_1_cache),
                    input_source,
                );
                // R002 trigger: pass-1 changed bytes,
                // but the post-pass-1 buffer no longer yields any
                // parsed markings. Marque's recognizer is total — it
                // never returns a hard `Err` — so the "re-parse failed"
                // signal is detected as "pass-1 had markings to begin
                // with, applied fixes against them, and the post-splice
                // buffer has zero markings." That's a buffer the
                // scanner walked clean of every candidate the original
                // had; pass-2 has nothing to fire against and the
                // operator needs to know pass-1 may have corrupted the
                // marking shape.
                //
                // A clean conservative trigger (no false positives on
                // partial cleanups): if the pre-pass-1 buffer had ≥1
                // marking AND the post-pass-1 buffer has zero, the
                // pass-1 splice destroyed marking shape.
                // Issue #433 note on the sentinel signals: the
                // R002 trigger is "pre had recognized markings AND
                // post has zero." Under #433's deferred cache,
                // `parsed_markings.is_empty()` no longer answers
                // "had any recognized marking" — only "had any
                // FixIntent-bearing marking." So both sides of the
                // conjunction now read `LintResult
                // .recognized_marking_count`, which is incremented
                // on every `Parsed::Unambiguous` recognition and
                // is independent of the cache-population gate.
                // Using the cache-emptiness signal here would
                // produce both false negatives (pre had only
                // text-correction-emitting markings and pass-1
                // destroyed them — sentinel never fires) and
                // false positives (pass-1 fixed the only
                // FixIntent issue and the re-lint cleanly parses
                // — sentinel fires spuriously). The recognized-
                // count signal avoids both.
                let post_pass1_had_no_markings = relint.recognized_marking_count == 0;
                let pre_pass1_had_markings = lint.recognized_marking_count > 0;
                if post_pass1_had_no_markings && pre_pass1_had_markings {
                    let contributing = self.contributing_pass1_rule_ids(&pass1.audit_lines);
                    let failure_span = Span::new(0, pass1.post_buffer.len());
                    let r002 = build_r002_diagnostic(contributing, failure_span);
                    return Ok(self.assemble_r002_result(
                        pass0_audit_lines,
                        pass0_dropped_diags,
                        pass1,
                        lint,
                        r002,
                    ));
                }
                // Non-R002 re-parse path: destructure pass1 and move
                // `post_buffer` directly into `pass2_source` — no
                // document-buffer clone. `relint` is the fresh
                // post-pass-1 LintResult — move it into the `lint`
                // slot so the deadline-error payload, the pass-2
                // dispatch borrow, the partition below, and the
                // remaining-diagnostics filter all see post-pass-1
                // state.
                let Pass1Result {
                    post_buffer,
                    audit_lines,
                    applied_keys,
                } = pass1;
                (post_buffer, new_markings, audit_lines, applied_keys, relint)
            };

        // Re-partition the (now post-pass-1, when applicable) lint's
        // diagnostic stream. Pass-2 dispatches against this fresh
        // partition. The pre-
        // pass-1 partition went out of scope when its enclosing block
        // ended above, so its borrow of the pre-rebind `lint` doesn't
        // outlive the rebind. `localized_ids` is `Engine::new`-time
        // immutable, so the partition predicate is unchanged. The
        // unused pass-1 slot here is discarded because pass-1 has
        // already run; pass-2 only needs its own phase partition.
        //
        // The partition itself is O(N) pointer pushes into two
        // reference vectors — no clones, no extra owned-diagnostic
        // bodies (Constitution I).
        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded { partial_lint: lint });
        }

        // Pass-2: WholeMarking FixIntent fixes against post-pass-1
        // buffer. The partition + dispatch live in an inner scope so
        // `pass2_diags` (a `SmallVec<[&Diagnostic; 32]>` whose Drop
        // impl is not `#[may_dangle]`) drops before the later
        // `lint.diagnostics.into_iter()` move — `Vec<&T>`'s borrowck
        // relaxation doesn't carry over to `SmallVec`.
        let pass2 = {
            let (_pass1_diags_post, pass2_diags) =
                partition_diags_by_phase(&lint.diagnostics, &localized_ids);
            self.run_pass2_whole_marking(
                &pass2_source,
                &pass2_markings,
                &pass2_diags,
                &pass1_applied_keys,
                &lint,
            )?
        };

        // Merge audit-line streams. Order matches the audit-stream
        // contract: applied-fix records emit in the order
        // corrections; pass1; pass2. The confidence-then-span fix
        // ordering generalizes to the marking-fix + text-correction
        // sum-type stream.
        let mut all_audit_lines: Vec<AuditLine<S>> = Vec::with_capacity(
            pass0_audit_lines.len() + pass1_audit_lines.len() + pass2.audit_lines.len(),
        );
        all_audit_lines.extend(pass0_audit_lines);
        all_audit_lines.extend(pass1_audit_lines);
        all_audit_lines.extend(pass2.audit_lines);

        // Build the applied-keys set from EVERY audit line so the
        // remaining-diagnostics filter knows what survived each pass.
        let mut applied_keys: HashSet<(RuleId, Span)> =
            HashSet::with_capacity(all_audit_lines.len());
        for line in &all_audit_lines {
            match line {
                AuditLine::AppliedFix(fix) => {
                    applied_keys.insert((fix.rule, fix.span));
                }
                AuditLine::TextCorrection(tc) => {
                    applied_keys.insert((tc.rule, tc.span));
                }
                _ => {}
            }
        }
        for k in &pass1_applied_keys {
            applied_keys.insert(*k);
        }
        for k in &pass2.applied_keys {
            applied_keys.insert(*k);
        }

        let mut remaining_diagnostics: Vec<Diagnostic<S>> = lint
            .diagnostics
            .into_iter()
            .filter(|d| {
                let fix_applied = if d.fix.is_some() {
                    let span = d.candidate_span.unwrap_or(d.span);
                    applied_keys.contains(&(d.rule, span))
                } else if d.text_correction.is_some() {
                    applied_keys.contains(&(d.rule, d.span))
                } else {
                    false
                };
                !fix_applied
            })
            .collect();
        // Pass-0 dropped text-correction diagnostics surface here:
        // pass-2 lint runs on the corrected buffer, so it never
        // re-emits them. The C-1 overlap guard in `apply_text_corrections`
        // recorded the dropped set so we can route it through.
        for d in pass0_dropped_diags {
            remaining_diagnostics.push(d);
        }

        Ok(FixResult {
            source: into_secret_slice(pass2.output),
            audit_lines: all_audit_lines,
            remaining_diagnostics,
            r002_fired: false,
            session_metadata: self.session_metadata.clone(),
        })
    }

    /// Pass-0 — text-correction promotion via the existing engine
    /// helper. **Behavior unchanged** from pre-7b (D-7.6: "C001 stays
    /// as pass-0"); this method exists to keep the pipeline shape
    /// visible at the `run()` call site.
    fn run_pass0_c001(&self, lint: &LintResult<S>) -> Pass0Result<S> {
        let (effective_source, dropped_diags, audit_lines) = self.engine.apply_text_corrections(
            self.source,
            lint,
            self.threshold,
            self.mode,
            self.classifier_id.clone(),
        );
        Pass0Result {
            effective_source: Zeroizing::new(effective_source),
            audit_lines,
            dropped_diags,
        }
    }

    /// Pass-1 — synthesize + filter + sort + C-1 dedup + forward-pass
    /// splice for [`Phase::Localized`] rule fixes.
    ///
    /// First-fire span-shape check drops out-of-shape fixes BEFORE the
    /// confidence-then-span sort, so a rule that misdeclared `Localized`
    /// and emitted a marking-wide span never enters the sort or the
    /// C-1 walk. `debug_assert!` panics in debug builds (CI catches);
    /// `tracing::error!` is always-on. The audit stream records
    /// nothing for a dropped fix (no `AppliedFix`).
    ///
    /// # `post_buffer` invariant
    ///
    /// When `applied.is_empty()`, `post_buffer` is returned **empty**
    /// (`Vec::new()`) — the caller MUST consume `pass0.effective_source`
    /// (or equivalent pre-pass-1 buffer) instead of `post_buffer` on
    /// the short-circuit branch. The no-op clone of the full document
    /// was load-bearing nowhere and burned an O(N) allocation per
    /// clean run; eliding it keeps the no-fix path zero-copy. Callers
    /// already destructure `post_buffer: _` on the `applied.is_empty()`
    /// branch (see `TwoPassFixer::run`).
    fn run_pass1_localized(
        &self,
        effective_source: &[u8],
        parsed_markings: &[(Span, S::Marking)],
        pass1_diags: &[&Diagnostic<S>],
        lint: &LintResult<S>,
    ) -> Result<Pass1Result<S>, EngineError<S>> {
        if pass1_diags.is_empty() {
            // No diagnostics → no fixes → caller short-circuits and
            // consumes its own pre-pass-1 buffer. Skip the document
            // clone; see the function's `post_buffer` invariant above.
            return Ok(Pass1Result {
                post_buffer: Zeroizing::new(Vec::new()),
                audit_lines: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }

        let synthesized: Vec<SynthesizedFix<S>> = synthesize_fixes(
            &self.engine.scheme,
            parsed_markings,
            effective_source,
            pass1_diags,
            self.threshold,
        );

        // First-fire span-shape filter. For a `Phase::Localized` rule
        // the fix span MUST be contained within the candidate marking's
        // parsed bytes. The synthesized record carries `span` set from
        // the diagnostic's `candidate_span` (or `span`); the parsed
        // marking's bytes are `parsed_markings`' key span. A fix whose
        // span sits outside the candidate is a misuse of the phase tag
        // and is dropped before the confidence-then-span sort.
        let in_shape: Vec<SynthesizedFix<S>> = synthesized
            .into_iter()
            .filter(
                |sf| match find_containing_marking::<S>(parsed_markings, sf.span) {
                    Some(marking_span) => {
                        if span_is_within_marking(sf.span, marking_span) {
                            true
                        } else {
                            tracing::error!(
                                rule_id = %sf.rule,
                                fix_span = ?sf.span,
                                marking_span = ?marking_span,
                                "Phase::Localized rule emitted out-of-shape span; dropping fix"
                            );
                            debug_assert!(
                                false,
                                "Localized rule '{}' emitted span {:?} outside marking {:?}",
                                sf.rule, sf.span, marking_span
                            );
                            false
                        }
                    }
                    None => {
                        tracing::error!(
                            rule_id = %sf.rule,
                            fix_span = ?sf.span,
                            "Phase::Localized rule fix span has no enclosing parsed marking; \
                             dropping fix"
                        );
                        debug_assert!(
                            false,
                            "Localized rule '{}' emitted span {:?} with no enclosing marking",
                            sf.rule, sf.span
                        );
                        false
                    }
                },
            )
            .collect();

        let kept_fixes = sort_and_c1_dedup(in_shape);
        if kept_fixes.is_empty() {
            // All synthesized fixes were filtered out (out-of-shape
            // drops or C-1 dedup losers). Caller short-circuits and
            // consumes its own pre-pass-1 buffer; skip the document
            // clone per the `post_buffer` invariant above.
            return Ok(Pass1Result {
                post_buffer: Zeroizing::new(Vec::new()),
                audit_lines: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }
        let (post_buffer, applied_keys, audit_lines) =
            self.apply_kept_fixes(effective_source, kept_fixes, lint)?;
        Ok(Pass1Result {
            post_buffer,
            audit_lines,
            applied_keys,
        })
    }

    /// Pass-2 — synthesize + sort + C-1 dedup + forward-pass splice
    /// for [`Phase::WholeMarking`] rule fixes. Operates on the
    /// post-pass-1 buffer and the corresponding freshly-parsed
    /// markings (or, when pass-1 short-circuited, on the post-pass-0
    /// buffer and its markings).
    ///
    /// The C-1 dedup walk is **independent** of pass-1's walk
    /// (architect pre-flight §2): pass-1 keeps the lex-min winner
    /// among Localized fixes; pass-2 keeps the lex-min winner among
    /// WholeMarking fixes. Because the partitions are disjoint by
    /// rule phase, the union of winners has no rule-and-span
    /// collision.
    ///
    /// Two reshape-aware adjustments run before the fixes pass into
    /// [`synthesize_fixes`]:
    ///
    /// - **Disambiguation**: a pass-2 diagnostic whose `(rule, span)`
    ///   equals a pass-1 promoted fix is dropped. The same rule has
    ///   already fired on the same marking-scope span; re-emitting it
    ///   after the reshape would double-fire and pollute
    ///   `remaining_diagnostics`.
    /// - **Overlap demotion**: a pass-2 diagnostic whose span overlaps
    ///   ANY pass-1 promoted fix span (any rule) at
    ///   `Severity::{Error, Warn, Fix}` is demoted to
    ///   `Severity::Suggest`. The pass-1 fix already shipped, so
    ///   pass-2 MUST NOT auto-apply on the same byte range
    ///   (Constitution V audit-record integrity); `Suggest` surfaces
    ///   the finding as advisory and is excluded from the audit
    ///   stream by `synthesize_fixes`' existing filter (`Suggest` does
    ///   not trigger `EX_DIAG_WARN`).
    ///
    /// Both adjustments operate on owned clones of the affected
    /// diagnostics so the input reference vector stays unmodified
    /// (pass-1 dispatch may still hold references into the same
    /// `LintResult.diagnostics` storage; cloning is the only sound
    /// way to alter severity without aliasing).
    fn run_pass2_whole_marking(
        &self,
        pass2_source: &[u8],
        parsed_markings: &[(Span, S::Marking)],
        pass2_diags: &[&Diagnostic<S>],
        pass1_applied_keys: &HashSet<(RuleId, Span)>,
        lint: &LintResult<S>,
    ) -> Result<Pass2Result<S>, EngineError<S>> {
        if pass2_diags.is_empty() {
            return Ok(Pass2Result {
                output: Zeroizing::new(match self.mode {
                    FixMode::Apply => pass2_source.to_vec(),
                    FixMode::DryRun => self.source.to_vec(),
                }),
                audit_lines: Vec::new(),
                applied_keys: HashSet::new(),
            });
        }

        // Disambiguation + overlap demotion. The
        // owned vector holds the post-adjustment diagnostics; a ref
        // vector keyed to its addresses feeds `synthesize_fixes`
        // (which signature still takes `&[&Diagnostic]`). The owned
        // vector lives for the duration of this function so the
        // refs are valid.
        let adjusted_owned = apply_fr023_and_i18(pass2_diags, pass1_applied_keys);
        let adjusted_refs: Vec<&Diagnostic<S>> = adjusted_owned.iter().collect();

        let synthesized = synthesize_fixes(
            &self.engine.scheme,
            parsed_markings,
            pass2_source,
            &adjusted_refs,
            self.threshold,
        );
        let kept_fixes = sort_and_c1_dedup(synthesized);
        let (post_buffer, applied_keys, audit_lines) =
            self.apply_kept_fixes(pass2_source, kept_fixes, lint)?;

        let output = match self.mode {
            FixMode::Apply => post_buffer,
            // DryRun returns the original source verbatim — pass-1's
            // post-buffer is discarded so callers cannot accidentally
            // consume partial bytes when they asked for dry-run.
            FixMode::DryRun => Zeroizing::new(self.source.to_vec()),
        };
        Ok(Pass2Result {
            output,
            audit_lines,
            applied_keys,
        })
    }

    /// Apply `kept_fixes` (already confidence-then-span-sorted and
    /// C-1-deduped) against `source_buf`, producing the post-splice buffer and the
    /// promoted [`AppliedFix`] records. Shared between pass-1 and
    /// pass-2 because the splice semantics are identical at this
    /// layer.
    ///
    /// The post-splice buffer is built in **both** [`FixMode::Apply`]
    /// and [`FixMode::DryRun`] because pass-1's `post_buffer` is the
    /// input to pass-2's re-lint + dispatch: pass-2
    /// MUST see the post-pass-1 coordinate space regardless of mode,
    /// or DryRun would silently dispatch pass-2 against the pre-pass-1
    /// buffer and produce a different applied set than Apply. The
    /// DryRun-vs-Apply distinction only affects [`FixResult.source`]
    /// at the outer layer — `run_pass2_whole_marking` substitutes
    /// `self.source.to_vec()` for DryRun there, so the user-visible
    /// `FixResult.source` remains the unmodified original.
    ///
    /// `dry_run` is plumbed into every [`AppliedFix`] record so the
    /// audit stream reflects mode regardless of which buffer the
    /// engine chose to keep as `FixResult.source`.
    ///
    /// Per-fix-application deadline check sits at the top of each
    /// iteration — the abort happens between fixes so we never
    /// construct a half-applied `FixResult` (Constitution V Principle
    /// V). On deadline expiry the function returns
    /// `Err(EngineError::DeadlineExceeded)` and discards any partial
    /// state; the audit stream gets nothing.
    fn localized_rule_id_set(&self) -> HashSet<&'static str> {
        let mut out: HashSet<&'static str> = HashSet::new();
        for &(set_idx, rule_idx) in self.engine.pass1_rule_indices.iter() {
            let rule = &self.engine.rule_sets[set_idx].rules()[rule_idx];
            out.insert(rule.id().predicate_id());
            for &(emitted_id, _) in rule.additional_emitted_ids() {
                out.insert(emitted_id);
            }
        }
        out
    }

    /// Capture pre-pass-1 attribute snapshots for every marking
    /// whose span overlaps a pass-1 applied fix.
    /// Returns an empty cache when pass-1 promoted no fixes — pass-2
    /// has no reshape to disambiguate against and the
    /// `RuleContext.pre_pass_1_attrs` field is `None` everywhere on
    /// the post-pass-1 re-lint.
    ///
    /// Cache shape: at most one entry per pass-1-reshaped marking.
    /// `Phase::Localized` rules emit sub-token fix spans (the
    /// first-fire check enforces this), so a single fix always anchors
    /// to a single parent marking. Two fixes against the same marking
    /// dedupe to one cache entry. Inline-4 storage matches the small
    /// Localized rule count.
    ///
    /// The `parsed_markings` slice is the pre-pass-1 cache from
    /// `lint_with_options_internal` (issue #432 swapped the storage
    /// from `HashMap<Span, _>` to a sorted `Vec<(Span, _)>` for
    /// cache-locality wins on high-candidate inputs). The
    /// `S::Canonical` snapshot is taken through
    /// `MarkingScheme::canonical_from_marking` — the rule fired against
    /// these same attributes. Cloning the attrs is unavoidable here
    /// because the cache outlives the `parsed_markings` slice (the
    /// engine moves the underlying `Vec` into the re-parse arm).
    fn populate_pre_pass_1_cache(
        &self,
        pass1_audit_lines: &[AuditLine<S>],
        parsed_markings: &[(Span, S::Marking)],
    ) -> PrePass1Cache<S> {
        let mut cache: PrePass1Cache<S> = SmallVec::new();
        if pass1_audit_lines.is_empty() {
            return cache;
        }
        // For each applied pass-1 fix, find the marking whose span
        // contains the fix span and dedupe into the cache. Sub-token
        // span containment is what `find_containing_marking` already
        // checks for the first-fire path, so reuse it.
        //
        // Walks both `AppliedFix` and `TextCorrection` arms of
        // `AuditLine` — pass-1 today only produces marking fixes
        // (text-corrections run in pass-0), but the cache shape stays
        // sum-type-safe so a future Phase::Localized text-correction
        // would still get its marking captured.
        for line in pass1_audit_lines {
            let (rule, span) = match line {
                AuditLine::AppliedFix(fix) => (&fix.rule, fix.span),
                AuditLine::TextCorrection(tc) => (&tc.rule, tc.span),
                _ => continue,
            };
            let Some(marking_span) = find_containing_marking::<S>(parsed_markings, span) else {
                // A Localized rule whose fix span has no enclosing
                // marking should already have been dropped by the
                // in_shape filter; if one slips through it
                // means a violated phase contract elsewhere. Log and
                // skip — never panic on the audit hot path.
                tracing::warn!(
                    rule_id = %rule,
                    fix_span_start = %span.start,
                    fix_span_end = %span.end,
                    "pass-1 applied fix has no enclosing parsed marking; \
                     skipping pre-pass-1 cache entry"
                );
                continue;
            };
            if cache.iter().any(|(s, _)| *s == marking_span) {
                continue;
            }
            let Some(marking) = lookup_marking::<S>(parsed_markings, marking_span) else {
                continue;
            };
            // Snapshot the marking's canonical attributes through the
            // scheme accessor. At `S = CapcoScheme` this is exactly the
            // former `marking.0.clone()`; routing through the trait keeps
            // the cache build scheme-agnostic.
            cache.push((
                marking_span,
                self.engine.scheme.canonical_from_marking(marking),
            ));
        }
        cache
    }
}
