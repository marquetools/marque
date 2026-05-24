use super::fix_impl::Pass1Result;
use super::synthesis::{span_is_within_marking, splice_fixes_forward};
use super::*;

impl Engine {
    /// Lint and apply fixes. Returns fixed source and audit log.
    ///
    /// Fix application order is `(span.end DESC, span.start DESC,
    /// rule_id ASC, replacement ASC)` so reverse-byte application preserves
    /// earlier-span offsets and equal-span ties break deterministically.
    ///
    /// Uses the confidence threshold configured in the engine's `Config`.
    /// To supply a per-call override (e.g., from a `--confidence` CLI flag
    /// or an HTTP request field), use [`Engine::fix_with_threshold`] or
    /// [`Engine::fix_with_options`].
    ///
    /// Back-compat shim over [`Engine::fix_with_options`] — `fix(src, mode)`
    /// is equivalent to `fix_with_options(src, mode, &FixOptions::default())`
    /// (no deadline, no threshold override). Both invariants make the
    /// `expect` here unreachable: the default options carry no deadline so
    /// `EngineError::DeadlineExceeded` cannot fire, and the config
    /// threshold is pre-validated at load time so
    /// `EngineError::InvalidThreshold` cannot fire.
    pub fn fix(&self, source: &[u8], mode: FixMode) -> FixResult {
        self.fix_with_options(source, mode, &FixOptions::default())
            .expect(
                "fix() default options cannot fail: no deadline + pre-validated config threshold",
            )
    }

    /// Lint and apply fixes using an optional per-call confidence threshold.
    ///
    /// When `threshold_override` is `Some`, it replaces the config-level
    /// threshold for this call only and is validated against `[0.0, 1.0]`.
    /// When `None`, the engine falls back to `Config::confidence_threshold`.
    ///
    /// This signature is preserved for back-compat. New callers should
    /// prefer [`Engine::fix_with_options`], which carries the deadline
    /// surface alongside the threshold override.
    pub fn fix_with_threshold(
        &self,
        source: &[u8],
        mode: FixMode,
        threshold_override: Option<f32>,
    ) -> Result<FixResult, InvalidThreshold> {
        let opts = FixOptions {
            threshold_override,
            ..Default::default()
        };
        match self.fix_with_options(source, mode, &opts) {
            Ok(result) => Ok(result),
            Err(EngineError::InvalidThreshold(it)) => Err(it),
            // No caller can reach this arm: `fix_with_threshold`'s
            // public signature does not accept a deadline, so the
            // `FixOptions` we built above has `deadline: None`. A
            // future signature change that introduces one would have
            // to remove this `unreachable!` deliberately.
            Err(EngineError::DeadlineExceeded { .. }) => {
                unreachable!("fix_with_threshold cannot set a deadline through its signature")
            }
        }
    }

    /// Lint and apply fixes with per-call options (spec 005 §R2).
    ///
    /// Phase 2 honors `opts.deadline` via cooperative cancellation
    /// (spec §R3). Asymmetric response per §R4 / Constitution V
    /// Principle V (audit-record integrity): a deadline expiring at
    /// any point during the fix path returns
    /// `Err(EngineError::DeadlineExceeded { partial_lint })` rather
    /// than a partial `FixResult`. The `partial_lint` carries
    /// whatever the lint phase had produced before the deadline
    /// fired (or a fully-truncated lint when the deadline was
    /// already expired on entry); no half-applied fix is ever
    /// emitted into the audit stream.
    ///
    /// `opts.threshold_override` is honored from Phase 1 onward; an
    /// out-of-range / NaN value is rejected as
    /// `EngineError::InvalidThreshold` before any work runs.
    pub fn fix_with_options(
        &self,
        source: &[u8],
        mode: FixMode,
        opts: &FixOptions,
    ) -> Result<FixResult, EngineError> {
        let threshold = match opts.threshold_override {
            Some(value) => {
                if !(0.0..=1.0).contains(&value) || value.is_nan() {
                    return Err(EngineError::InvalidThreshold(InvalidThreshold(value)));
                }
                value
            }
            None => self.config.confidence_threshold(),
        };

        self.fix_inner(source, mode, threshold, opts.deadline)
    }

    fn fix_inner(
        &self,
        source: &[u8],
        mode: FixMode,
        threshold: f32,
        deadline: Option<Instant>,
    ) -> Result<FixResult, EngineError> {
        // Trampoline: every stage of the pipeline lives on
        // `TwoPassFixer`; this method binds the public surface
        // (`fix_with_options` -> `fix_inner`) to that struct.
        TwoPassFixer {
            engine: self,
            source,
            mode,
            threshold,
            deadline,
        }
        .run()
    }

    /// Apply pre-scanner text corrections from lint diagnostics and
    /// return the corrected source + applied audit lines + dropped diagnostics.
    /// Used by `fix_inner` to produce an intermediate source the scanner
    /// can detect; the dropped diagnostics surface via
    /// `remaining_diagnostics`.
    pub(super) fn apply_text_corrections(
        &self,
        source: &[u8],
        lint: &LintResult,
        threshold: f32,
        mode: FixMode,
    ) -> (
        Vec<u8>,
        Vec<Diagnostic<CapcoScheme>>,
        Vec<AuditLine<CapcoScheme>>,
    ) {
        // Mirror `fix_inner`'s suggest-channel exclusion: a
        // text-correction diagnostic that the lint post-pass rewrote to
        // `Severity::Suggest` (because its confidence fell below
        // threshold) must not be auto-applied here either.
        //
        // Text-correction diagnostics carry their canonical replacement
        // bytes + provenance in `Diagnostic.text_correction` (a
        // `TextCorrection` payload). The engine synthesizes
        // `TextCorrectionProposal` records from those diagnostics and
        // promotes them via
        // `AppliedFix::__engine_promote_text_correction`. Provenance
        // (`source`, `confidence`, `migration_ref`) is preserved per
        // the rule's emission — the engine does NOT overwrite it,
        // because the corrections-map rule, the deprecation-migration
        // rule, and other byte-substitution rules all share this
        // channel but carry distinct provenance.
        let mut text_fixes: Vec<TextCorrectionProposal> = lint
            .diagnostics
            .iter()
            .filter(|d| d.severity != Severity::Suggest)
            .filter_map(|d| {
                d.text_correction.as_ref().map(|tc| TextCorrectionProposal {
                    rule: d.rule,
                    severity: d.severity,
                    span: d.span,
                    replacement: tc.replacement.clone(),
                    confidence: tc.confidence.clone(),
                    source: tc.source,
                    message: d.message.clone(),
                    migration_ref: tc.migration_ref,
                })
            })
            .filter(|p| p.confidence.combined() >= threshold)
            .filter(|p| !p.span.is_empty())
            .collect();

        if text_fixes.is_empty() {
            return (source.to_vec(), Vec::new(), Vec::new());
        }

        // Sort and deduplicate using confidence-then-span order + C-1 overlap guard.
        text_fixes.sort_by(|a, b| {
            b.span
                .end
                .cmp(&a.span.end)
                .then(b.span.start.cmp(&a.span.start))
                .then(a.rule.cmp(&b.rule))
                .then(a.replacement.cmp(&b.replacement))
        });
        let mut kept: Vec<TextCorrectionProposal> = Vec::new();
        let mut dropped_keys: HashSet<(RuleId, Span)> = HashSet::new();
        let mut next_end: Option<usize> = None;
        for fix in &text_fixes {
            let fits = next_end.is_none_or(|b| fix.span.end <= b);
            if fits {
                next_end = Some(fix.span.start);
                kept.push(fix.clone());
            } else {
                dropped_keys.insert((fix.rule, fix.span));
            }
        }
        let kept_keys: HashSet<(RuleId, Span)> = kept.iter().map(|f| (f.rule, f.span)).collect();
        // Resurrect the diagnostics for the dropped fixes so they can
        // surface via `remaining_diagnostics`.
        let dropped_diags: Vec<Diagnostic<CapcoScheme>> = lint
            .diagnostics
            .iter()
            .filter(|d| {
                d.text_correction.is_some()
                    && dropped_keys.contains(&(d.rule, d.span))
                    && !kept_keys.contains(&(d.rule, d.span))
            })
            .cloned()
            .collect();

        let classifier_id: Option<Arc<str>> =
            self.config.user.classifier_id.as_deref().map(Arc::from);
        let dry_run = mode == FixMode::DryRun;
        let now = self.clock.now();

        // Always apply text corrections to the intermediate buffer, even in
        // DryRun mode. This buffer is internal — pass 2 needs it to re-lint
        // corrected text so downstream rules fire (e.g., E001 on NF after
        // SERCET→SECRET). The final output for DryRun returns the original
        // source in fix_inner, not this intermediate buffer.
        let mut buf = source.to_vec();
        let mut audit_lines: Vec<AuditLine<CapcoScheme>> = Vec::with_capacity(kept.len());
        for fix in kept {
            // Hash pre-correction bytes BEFORE the splice (the audit
            // record carries only the digest, never the bytes).
            // `original_bytes` borrows from `source` for the hashing
            // call only — never stored in an audit-record field. Order:
            // hash → splice → audit, so the splice
            // doesn't invalidate `original_bytes` and the audit
            // captures the original-bytes digest.
            let original_bytes = &source[fix.span.start..fix.span.end];
            let original_digest = blake3::hash(original_bytes);

            // Splice the canonical replacement into the buffer.
            buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());

            // marque-1.0 promote — AppliedTextCorrection (separate
            // line type per PM-D-4; CorrectionsMap and text-correction-
            // shaped rule emissions route here, never to the marking-
            // side Discriminant).
            let text_correction = AppliedTextCorrection::__engine_promote_text_correction(
                fix.rule,
                fix.severity,
                fix.span,
                original_digest,
                fix.replacement,
                fix.source,
                fix.confidence,
                fix.migration_ref,
                fix.message,
                now,
                classifier_id.clone(),
                dry_run,
                None,
                engine_promotion_token(),
            );
            audit_lines.push(AuditLine::TextCorrection(text_correction));
        }

        (buf, dropped_diags, audit_lines)
    }
}

pub(super) struct TwoPassFixer<'engine> {
    pub(super) engine: &'engine Engine,
    pub(super) source: &'engine [u8],
    pub(super) mode: FixMode,
    pub(super) threshold: f32,
    pub(super) deadline: Option<Instant>,
}

/// Promoted-fix tuple returned by [`TwoPassFixer::apply_kept_fixes`].
///
/// The post-splice buffer is wrapped in [`zeroize::Zeroizing`] so the
/// scratch bytes wipe on drop — Constitution Principle II. Pass-1's
/// post-buffer flows into pass-2's dispatch and then drops; pass-2's
/// post-buffer flows into the public [`FixResult.source`]
/// [`secrecy::SecretSlice`] via [`into_secret_slice`]. Either way, the
/// transient `Vec<u8>` never sits in freed memory unwiped.
/// Tuple returned by [`TwoPassFixer::apply_kept_fixes`].
///
/// Carries the post-pass output buffer + the audit-line stream + the
/// `(rule_id, span)` keys of applied fixes. `audit_lines` is the sole
/// audit channel.
pub(super) type AppliedTuple = (
    Zeroizing<Vec<u8>>,
    HashSet<(RuleId, Span)>,
    Vec<AuditLine<CapcoScheme>>,
);

/// Move a [`Zeroizing<Vec<u8>>`] into a [`SecretSlice<u8>`] without
/// leaking content through a shrink-reallocation.
///
/// The naive `vec.into()` path goes through [`Vec::into_boxed_slice`],
/// which **may reallocate to shrink** when `capacity > len` — and
/// the freed source allocation contains the content bytes the
/// realloc just copied, never wiped. [`splice_fixes_forward`]'s
/// pre-sized `Vec::with_capacity(source.len() + extra)` lands in
/// that state whenever a fix shrinks bytes (replacement shorter
/// than its span), so the channel is reachable on every
/// shrinking-fix path.
///
/// This helper sidesteps the channel by allocating the destination
/// `Box<[u8]>` separately via [`Box::from`] over a borrowed slice.
/// The `T: Copy` specialization for `u8` allocates exactly
/// `slice.len()` bytes through `RawVec::with_capacity` and
/// constructs the `Box<[u8]>` with `len == slice.len()` — no
/// shrinking realloc, no freed-and-unwiped buffer. The source
/// [`Zeroizing<Vec<u8>>`] retains ownership of its (possibly
/// over-allocated) original buffer through the copy and drops at
/// end-of-scope, wiping its full capacity via the `Vec<u8>`
/// [`zeroize::Zeroize`] impl before the Vec's backing memory is
/// freed.
///
/// Trade-off: one additional `len`-byte allocation and memcpy per
/// fix call (~1µs on 10KB inputs). The interactive-latency ceiling
/// (16ms p95 on 10KB) remains comfortable.
///
/// Constitution Principle II — the single Zeroizing → SecretSlice
/// transition for the public `FixResult.source` field. Engine-only
/// helper; not exported.
#[inline]
pub(super) fn into_secret_slice(z: Zeroizing<Vec<u8>>) -> SecretSlice<u8> {
    let bytes: Box<[u8]> = Box::from(&z[..]);
    SecretBox::new(bytes)
    // `z` drops here. `Zeroizing::drop` wipes its full capacity
    // (including the over-allocation tail that motivated this
    // helper) BEFORE the backing Vec frees its buffer.
}

/// Pre-pass-1 attribute cache entries.
///
/// One entry per marking whose span overlaps a pass-1 fix. The
/// engine builds the cache before the pass-1 splice so the
/// `CanonicalAttrs` snapshot reflects the bytes the rule originally
/// matched against. Inline-4 matches the small `Phase::Localized` rule
/// count (at most one fix per Localized rule per marking; the typical
/// document
/// has ≤4 reshape sites). Spills to heap on dense documents.
///
/// The `Span` keys are the **marking spans** (i.e., scanner
/// candidate spans), not the fix sub-spans. Lookup is a linear scan
/// using `span_is_within_marking` so a query span (a candidate
/// span at re-lint time) finds the cache entry whose marking
/// contains it. The post-pass-1 re-lint may produce candidates with
/// shifted offsets (the splice changed byte positions), so an
/// exact-equality keying scheme would miss every entry. The
/// containment scan is robust because spans grow monotonically
/// left-to-right; for any post-splice candidate the originating
/// pre-pass-1 marking is the unique cache entry whose pre-splice
/// span contained the same source bytes.
///
/// Inline-4 storage is 4 × `sizeof(Span) + sizeof(CanonicalAttrs)`
/// ≈ 4 × (16 + 112) = ~512 B on the stack. SmallVec spill to heap
/// is acceptable when documents exceed the cap (rust pre-flight §2).
pub(super) type PrePass1Cache = SmallVec<[(Span, marque_ism::CanonicalAttrs); 4]>;

/// Look up the pre-pass-1 attrs for a marking whose span contains
/// `query_span`. Linear scan over the ≤4-entry cache; the call is
/// per-candidate inside `lint_with_options_internal_with_cache` and
/// the inline-4 bound makes the scan stack-only on typical inputs.
///
/// Returns `Some(&attrs)` when a cache entry's marking span contains
/// the query span; `None` otherwise. The CONTENT of the entry
/// (`CanonicalAttrs`) is borrowed into `RuleContext.pre_pass_1_attrs`
/// as the architectural two-pass-reshape signal — no current rule
/// reads it, but the field stays plumbed for future consumers
/// (D-7.22).
#[inline]
pub(super) fn pre_pass_1_attrs_for_span(
    cache: &[(Span, marque_ism::CanonicalAttrs)],
    query_span: Span,
) -> Option<&marque_ism::CanonicalAttrs> {
    cache.iter().find_map(|(marking_span, attrs)| {
        if span_is_within_marking(query_span, *marking_span) {
            Some(attrs)
        } else {
            None
        }
    })
}

/// Pass-1 diagnostic-reference partition.
pub(super) type Pass1DiagRefs<'a> = SmallVec<[&'a Diagnostic<CapcoScheme>; 4]>;

/// Pass-2 diagnostic-reference partition.
pub(super) type Pass2DiagRefs<'a> = SmallVec<[&'a Diagnostic<CapcoScheme>; 32]>;

pub(super) fn partition_diags_by_phase<'a>(
    diagnostics: &'a [Diagnostic<CapcoScheme>],
    localized_ids: &HashSet<&'static str>,
) -> (Pass1DiagRefs<'a>, Pass2DiagRefs<'a>) {
    let mut pass1_diags: Pass1DiagRefs<'a> = SmallVec::new();
    let mut pass2_diags: Pass2DiagRefs<'a> = SmallVec::new();
    for d in diagnostics {
        // text_correction diagnostics flow through pass-0 only;
        // they are excluded from both pass-1 and pass-2 splicing
        // (their fixes have already been promoted into
        // `pass0.applied`). The remaining-diagnostics filter
        // resurfaces any text_correction diagnostic whose fix did
        // not apply, via the pre-existing keying path.
        if d.text_correction.is_some() && d.fix.is_none() {
            continue;
        }
        if localized_ids.contains(d.rule.predicate_id()) {
            pass1_diags.push(d);
        } else {
            pass2_diags.push(d);
        }
    }
    (pass1_diags, pass2_diags)
}

#[inline]
pub(super) fn engine_promotion_token() -> EnginePromotionToken {
    EnginePromotionToken::__engine_construct()
}

impl<'engine> TwoPassFixer<'engine> {
    pub(super) fn apply_kept_fixes(
        &self,
        source_buf: &[u8],
        kept_fixes: Vec<SynthesizedFix>,
        lint: &LintResult,
    ) -> Result<AppliedTuple, EngineError> {
        let classifier_id: Option<std::sync::Arc<str>> = self
            .engine
            .config
            .user
            .classifier_id
            .as_deref()
            .map(std::sync::Arc::from);
        let dry_run = self.mode == FixMode::DryRun;
        let now = self.engine.clock.now();

        let mut applied_keys: HashSet<(RuleId, Span)> = HashSet::with_capacity(kept_fixes.len());
        // `marque-1.0` audit-line stream — the sole audit-output
        // channel post-cutover. The CLI / WASM renderers project
        // each line to its NDJSON record type.
        let mut audit_lines: Vec<AuditLine<CapcoScheme>> = Vec::with_capacity(kept_fixes.len());

        if deadline_expired(self.deadline) {
            return Err(EngineError::DeadlineExceeded {
                partial_lint: lint.clone(),
            });
        }

        // Build the post-splice buffer in both modes — pass-2 needs
        // the post-pass-1 coordinate space to dispatch correctly even
        // in DryRun. Wrap in `Zeroizing` so the scratch bytes wipe on
        // drop per Constitution Principle II.
        let post_buffer = Zeroizing::new(splice_fixes_forward(source_buf, &kept_fixes));

        // EngineConstructor mints the open-vocab Canonical<S> values
        // for v2 audit records. The sealed constructor name + the
        // sealed `CanonicalConstructor` supertrait keep this path
        // engine-only per `marque-scheme::canonical` doc comment.
        let constructor: EngineConstructor<CapcoScheme> =
            EngineConstructor::<CapcoScheme>::__engine_construct();

        for fix in kept_fixes {
            if deadline_expired(self.deadline) {
                return Err(EngineError::DeadlineExceeded {
                    partial_lint: lint.clone(),
                });
            }
            let key = (fix.rule, fix.span);
            applied_keys.insert(key);

            // Hash pre-fix bytes for the `original_digest` (the audit
            // record carries only the digest, never the bytes).
            // `original_bytes` borrows from `source_buf` for the
            // lifetime of the hashing call only.
            let original_bytes = &source_buf[fix.span.start..fix.span.end];

            // Build the v2 Canonical<S> via EngineConstructor (the
            // sealed open-vocab path). PM-D / D-A3: derive the
            // CategoryId from the originating ReplacementIntent so the
            // audit-record renderer can project the
            // `replacement.canonical.category` JSON field accurately
            // per `contracts/audit-record.md` §272:
            //
            // - `FactAdd { token, .. }`: route the token through
            //   `MarkingScheme::category_of`. The scheme resolves both
            //   closed-CVE [`FactRef::Cve`] and open-vocab
            //   [`FactRef::OpenVocab`] arms; if the routing table is
            //   missing the token (e.g., a future variant not yet
            //   wired) the engine falls back to the multi-category
            //   sentinel rather than panicking — the audit consumer
            //   sees `"Marking"` and can investigate, but the audit
            //   record stays well-formed.
            // - `FactRemove { facts, .. }`: route the first fact's
            //   token (all facts in a single intent share an axis per
            //   FR-???; the multi-fact form is a chained removal on
            //   one axis like E024's RD/FRD/TFNI cluster).
            // - `Recanonicalize { .. }`: spans multiple categories by
            //   construction — re-renders an entire `Scope::Page` or
            //   `Scope::Document`. Routes to
            //   [`CategoryId::MARKING`] (the reserved multi-category
            //   sentinel; projects to `"Marking"` in the JSON).
            let scheme_ref: &CapcoScheme = &self.engine.scheme;
            let category_id: CategoryId = match &fix.intent.replacement {
                marque_scheme::ReplacementIntent::FactAdd { token, .. } => {
                    scheme_ref.category_of(token).unwrap_or(CategoryId::MARKING)
                }
                marque_scheme::ReplacementIntent::FactRemove { facts, .. } => facts
                    .first()
                    .and_then(|fact| scheme_ref.category_of(fact))
                    .unwrap_or(CategoryId::MARKING),
                marque_scheme::ReplacementIntent::Recanonicalize { .. } => CategoryId::MARKING,
                // `#[non_exhaustive]` guard — a future variant
                // routes to the multi-category sentinel until the
                // scheme's `category_of` mapping is extended.
                _ => CategoryId::MARKING,
            };
            let canonical: Canonical<CapcoScheme> = constructor.build_open_vocab(
                category_id,
                Box::from(fix.replacement.as_ref()),
                fix.scope,
            );

            // v2 promote — AppliedFix (marque-1.0 shape). The
            // constructor hashes both `original_bytes` and
            // `canonical.bytes()` inline per PM-D-6.
            let v2_applied = marque_rules::audit::AppliedFix::<CapcoScheme>::__engine_promote(
                fix.rule,
                fix.severity,
                fix.span,
                fix.intent,
                original_bytes,
                canonical,
                now,
                classifier_id.clone(),
                dry_run,
                None,
                engine_promotion_token(),
            );
            audit_lines.push(AuditLine::AppliedFix(v2_applied));
        }
        Ok((post_buffer, applied_keys, audit_lines))
    }

    /// Collect the unique contributing pass-1 rule IDs in a stable
    /// (sorted, deduped) order for the R002 payload. Capped at 4 entries
    /// to fit the `SmallVec<[RuleId; 4]>` inline capacity exactly —
    /// pass-1 has a small number of Localized rule families, and a
    /// future expansion can lift the cap in lockstep with the inline-N
    /// bump.
    pub(super) fn contributing_pass1_rule_ids(
        &self,
        pass1_audit_lines: &[AuditLine<CapcoScheme>],
    ) -> SmallVec<[RuleId; 4]> {
        let mut seen: HashSet<RuleId> = HashSet::new();
        let mut ids: Vec<RuleId> = Vec::new();
        for line in pass1_audit_lines {
            let rule = match line {
                AuditLine::AppliedFix(fix) => &fix.rule,
                AuditLine::TextCorrection(tc) => &tc.rule,
                _ => continue,
            };
            if seen.insert(*rule) {
                ids.push(*rule);
            }
        }
        ids.sort();
        let mut out: SmallVec<[RuleId; 4]> = SmallVec::new();
        for id in ids.into_iter().take(out.inline_size()) {
            out.push(id);
        }
        out
    }

    /// Assemble the R002 `FixResult` — pass-1 buffer + union of
    /// pass-0/pass-1 applied + R002 diagnostic appended to remaining.
    ///
    /// Takes `pass0_applied` and `pass0_dropped_diags` by value (rather
    /// than borrowing a `&Pass0Result`) so the assembler can `extend`
    /// directly without per-element clones — `pass0` is dead at the
    /// caller after destructuring, so move-semantics is appropriate.
    pub(super) fn assemble_r002_result(
        &self,
        pass0_audit_lines: Vec<AuditLine<CapcoScheme>>,
        pass0_dropped_diags: Vec<Diagnostic<CapcoScheme>>,
        pass1: Pass1Result,
        lint: LintResult,
        r002: Diagnostic<CapcoScheme>,
    ) -> FixResult {
        // Audit-line merge. R002 is a remaining-diagnostic synthetic —
        // it does NOT contribute an `AuditLine::AppliedFix` entry; only
        // promoted fixes do. The
        // R002 itself surfaces via `remaining_diagnostics` below.
        let mut all_audit_lines: Vec<AuditLine<CapcoScheme>> =
            Vec::with_capacity(pass0_audit_lines.len() + pass1.audit_lines.len());
        all_audit_lines.extend(pass0_audit_lines);
        all_audit_lines.extend(pass1.audit_lines);

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

        let mut remaining_diagnostics: Vec<Diagnostic<CapcoScheme>> = lint
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
        remaining_diagnostics.extend(pass0_dropped_diags);
        remaining_diagnostics.push(r002);

        // Output buffer: post-pass-1 in Apply mode, original in DryRun.
        // Per D-7.6 the pass-1 buffer is the returned source even on
        // R002 (the fixes happened; the audit log is honest about it).
        let output = match self.mode {
            FixMode::Apply => pass1.post_buffer,
            FixMode::DryRun => Zeroizing::new(self.source.to_vec()),
        };

        FixResult {
            source: into_secret_slice(output),
            audit_lines: all_audit_lines,
            remaining_diagnostics,
            r002_fired: true,
        }
    }
}
