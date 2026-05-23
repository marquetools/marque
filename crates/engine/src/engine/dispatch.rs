use super::*;

/// Pass-1 (Localized) rule-index partition. Each entry indexes back
/// into `Engine::rule_sets[i].rules()[j]` as `(i, j)`. Inline-4
/// because the production CAPCO ruleset has 4 Localized rules; future
/// schemes are expected to stay in the same order of magnitude.
pub(super) type Pass1Indices = SmallVec<[(usize, usize); 4]>;
/// Pass-2 (WholeMarking) rule-index partition. Inline-32 covers the
/// current 27-rule whole-marking subset; the SmallVec spills to the
/// heap at the 33rd entry, leaving 5 entries of headroom. The
/// rule-collapse trajectory (PR 3b retired 13 rules into walkers;
/// end-state target ~10 across all 4 stages) means the count is
/// contracting, so 32 stays comfortable. See [`Engine::pass2_rule_indices`]
/// for the same rationale at greater length.
pub(super) type Pass2Indices = SmallVec<[(usize, usize); 32]>;
/// PageFinalization rule-index partition (issue #461). Inline-4 —
/// PageFinalization is dispatched once per page (not per candidate),
/// so the registered-rule count drives this, not call frequency. W004
/// (issue #461) and S005 (issue #488) are today's consumers (two
/// rules). Two scheduled follow-up PRs (S007, BannerMatchesProjectedRule)
/// will add at most a handful more; if the count grows beyond inline
/// capacity the SmallVec spills to the heap (one allocation at engine
/// construction time — not on the hot path). 4 is a deliberate
/// small-inline budget: this
/// partition is consulted once per page-break + once at EOD, which
/// is O(pages) per document, not O(candidates).
pub(super) type PassFinalizationIndices = SmallVec<[(usize, usize); 4]>;

/// Pre-resolved registered-ID severity table consumed by Site A's
/// fast-path Off-skip. Outer-indexed by rule-set, inner by rule-index-
/// within-set — same shape as [`Pass1Indices`] / [`Pass2Indices`].
/// See [`Engine::fast_path_severities`] for the full invariant.
pub(super) type FastPathSeverities = Box<[Box<[Severity]>]>;

/// Pre-resolved per-emitted-ID severity overrides. Keyed by `&'static
/// str` matching [`RuleId::predicate_id()`] — the predicate-id half of
/// the 2-tuple. Lookups use `map.get(d.rule.predicate_id())` with no
/// owned allocation (both sides are `&'static str`). See
/// [`Engine::emitted_id_overrides`] for the full invariant.
///
/// **Single-scheme assumption.** Today every registered rule lives in
/// the `"capco"` scheme; predicate ids are unique within the scheme by
/// construction (`crates/capco/tests/post_3b_registration_pin.rs`).
/// If a future scheme is added (CUI, NATO, …), this key shape MUST
/// widen to `(scheme, predicate_id)` — e.g., `HashMap<RuleId, _>` —
/// to disambiguate cross-scheme collisions. The T044 PM decision
/// addendum (OD-7) lets users type wire-string keys
/// (`"capco:predicate"`) in `.marque.toml`; canonicalization resolves
/// those to the same `&'static str` predicate intern.
pub(super) type EmittedIdOverrides = HashMap<&'static str, Severity>;

/// Partition the registered rules by their declared [`Phase`] (FR-021).
///
/// Returns `(pass1, pass2, pass_finalization)` where each entry is a
/// `(rule_set_index, rule_index_within_set)` pair indexing back into
/// the caller's `rule_sets[i].rules()[j]`:
///
/// - `pass1` enumerates every [`Phase::Localized`] rule (pass-1
///   forward-splice in `TwoPassFixer`).
/// - `pass2` enumerates every [`Phase::WholeMarking`] rule (pass-2
///   apply_intent in `TwoPassFixer`).
/// - `pass_finalization` enumerates every [`Phase::PageFinalization`]
///   rule (issue #461). Dispatched by the engine's synthetic-candidate
///   path at each scanner-emitted `MarkingType::PageBreak` (BEFORE the
///   PageContext reset) and once at end-of-document — see
///   `dispatch_page_finalization`.
///
/// Together they cover every registered rule exactly once. `Phase` is
/// `#[non_exhaustive]` per issue #461; the wildcard arm below is a
/// loud failure for a future variant rather than a silent bucket.
///
/// Walked once at [`Engine::with_clock`] time and cached on the engine.
/// Per-document `fix` dispatch reads `pass1_rule_indices` via
/// [`TwoPassFixer::localized_rule_id_set`] (PR 7b); the walk does not
/// re-run. `pass2_rule_indices` is stored against a deferred future
/// migration that would switch pass-2 from the current
/// complement-of-pass-1 dispatch to a positive whitelist (read off
/// that field) for symmetry with pass-1. PR 7c retained the
/// complement dispatch (implementer Decision #4); see
/// [`Engine::pass2_rule_indices`] for the rationale and the
/// deferred-migration framing. `pass_finalization_rule_indices` is
/// the issue #461 third bucket — read directly by
/// `dispatch_page_finalization` at lint time and (when a future
/// PageFinalization rule emits a fix) by the corresponding pass-2
/// path at fix time. Today's consumers (W004 from issue #461; S005
/// from issue #488) emit no fix.
pub(super) fn partition_rules_by_phase(
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
) -> (Pass1Indices, Pass2Indices, PassFinalizationIndices) {
    let mut pass1: Pass1Indices = SmallVec::new();
    let mut pass2: Pass2Indices = SmallVec::new();
    let mut pass_finalization: PassFinalizationIndices = SmallVec::new();
    for (set_idx, rule_set) in rule_sets.iter().enumerate() {
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            match rule.phase() {
                Phase::Localized => pass1.push((set_idx, rule_idx)),
                Phase::WholeMarking => pass2.push((set_idx, rule_idx)),
                Phase::PageFinalization => pass_finalization.push((set_idx, rule_idx)),
                // `Phase` is `#[non_exhaustive]` (issue #461). A
                // future variant should fail loudly at engine
                // construction time so the dispatch path stays
                // explicit — never silently bucket a new phase
                // into an existing pass.
                _ => panic!(
                    "partition_rules_by_phase: unknown Phase variant for rule {}; \
                     `Phase` is #[non_exhaustive] and a new variant requires explicit \
                     engine plumbing before it can be registered",
                    rule.id()
                ),
            }
        }
    }
    (pass1, pass2, pass_finalization)
}

pub(super) fn build_severity_tables(
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
    overrides: &HashMap<String, String>,
    bridge_rule_ids: &'static [(&'static str, &'static str)],
) -> (FastPathSeverities, EmittedIdOverrides) {
    // Pass 1: collect every canonical `&'static str` rule ID emitted
    // by the rule set — both registered IDs (`rule.id().predicate_id()`)
    // and per-row catalog IDs from dispatcher walkers
    // (`rule.additional_emitted_ids()`). The override map's keys
    // canonicalize against this superset; everything not in it would
    // have been rejected by `canonicalize_rule_overrides`.
    // T044: include both the wire-string form (Agent A's
    // `additional_emitted_ids` first col, `bridge_emitted_rule_ids`
    // first col) AND the predicate-id slice — the canonicalize step
    // reduces wire strings to predicate-id at registration time, so
    // the canonical intern stored in `overrides` is the predicate-id
    // half. Both must appear in `known_ids` for the Pass 2 `.expect()`
    // to hold across the transitional period while Agent A's
    // `bridge_emitted_rule_ids` rename is mid-flight.
    let mut known_ids: HashSet<&'static str> = HashSet::new();
    for rule_set in rule_sets {
        for rule in rule_set.rules() {
            known_ids.insert(rule.id().predicate_id());
            for (catalog_id, _catalog_name) in rule.additional_emitted_ids() {
                known_ids.insert(catalog_id);
                if let Some(predicate) = predicate_id_of_wire(catalog_id) {
                    known_ids.insert(predicate);
                }
            }
        }
    }
    // Bridge-emitted IDs (E058 / E059) are valid override keys too,
    // registered through `bridge_emitted_rule_ids` in the
    // canonicalizer.
    for (bridge_id, _bridge_name) in bridge_rule_ids {
        known_ids.insert(bridge_id);
        if let Some(predicate) = predicate_id_of_wire(bridge_id) {
            known_ids.insert(predicate);
        }
    }

    // Pass 2: walk the canonicalized override map. The map's keys are
    // owned `String` (canonical IDs the canonicalizer produced from
    // either `id` or `name` forms), but we look them up against
    // `known_ids: HashSet<&'static str>` and store the resolved
    // intern. Malformed severities are silently skipped to preserve
    // the pre-hoist `.and_then(parse_config)` semantics (which would
    // have returned `None` and fallen through to the unwrap_or
    // default).
    let mut emitted_id_overrides: HashMap<&'static str, Severity> = HashMap::new();
    for (canonical_id, severity_str) in overrides {
        let Some(severity) = Severity::parse_config(severity_str.as_str()) else {
            // Pre-hoist behavior: the `.and_then(parse_config)` arm
            // would return `None` for an unparseable severity string,
            // so the fast path fell through to `default_severity()`
            // and the per-emitted-id path preserved the emitted
            // severity. Match that by skipping the insert here.
            continue;
        };
        // The canonicalizer guarantees every surviving key is a
        // known `&'static str` intern (it walks the same superset
        // and `unwrap_or(rule.default_severity())`'s the lookup);
        // if we miss here it means the canonicalizer's invariant
        // has been broken upstream.
        let intern = *known_ids
            .get(canonical_id.as_str())
            .expect("canonicalized override key has a registered &'static intern");
        emitted_id_overrides.insert(intern, severity);
    }

    // Pass 3: build the registered-ID severity table. For each
    // rule-set in declared order, walk its rules in registered order
    // and resolve each rule's registered-ID severity. Lookup against
    // `emitted_id_overrides` (cheaper than the original
    // `overrides.get(...).and_then(parse_config)` chain because the
    // parse already happened in pass 2); fall back to
    // `rule.default_severity()` when absent.
    let fast_path_severities: Box<[Box<[Severity]>]> = rule_sets
        .iter()
        .map(|rule_set| {
            rule_set
                .rules()
                .iter()
                .map(|rule| {
                    emitted_id_overrides
                        .get(rule.id().predicate_id())
                        .copied()
                        .unwrap_or(rule.default_severity())
                })
                .collect::<Vec<Severity>>()
                .into_boxed_slice()
        })
        .collect::<Vec<Box<[Severity]>>>()
        .into_boxed_slice();

    (fast_path_severities, emitted_id_overrides)
}

/// Resolve every key in `config.rules.overrides` against the registered
/// rule sets. Both the rule ID (`"E001"`) and the rule name
/// (`"portion-mark-in-banner"`) are accepted — after canonicalization
/// the override map keys by canonical ID only, and the per-rule lookup
/// in `lint()` / `fix_inner()` keeps working unchanged.
///
/// Fails closed on:
/// - **Unknown keys** — `E999 = "warn"` or `not-a-rule = "error"` → the
///   user has almost certainly typo'd a rule reference. Silent acceptance
///   (the pre-#49 behavior) means the user thought they were configuring
///   the rule, but nothing happened at lint time. Emits
///   `EngineConstructionError::UnknownRuleOverride` with a best-effort
///   `did_you_mean` suggestion (Levenshtein ≤ 3 against the union of
///   known IDs and names).
/// - **Conflicting duplicate forms** — `E001 = "warn"` AND
///   `portion-mark-in-banner = "error"` in the same merged config →
///   the two entries resolved to the same rule but with different
///   severities. One form would have silently won the HashMap race.
///   Emits `EngineConstructionError::ConflictingRuleOverride`.
///
/// Duplicate forms with the *same* severity are silently accepted —
/// a user writing both `E001 = "warn"` and `portion-mark-in-banner =
/// "warn"` (intentionally or via copy-paste across config layers) gets
/// the expected behavior.
/// Slice the predicate-id half out of a wire-string form
/// `"<scheme>:<predicate_id>"`. Returns `None` when `s` is not in
/// wire-string shape (no colon). Used by `canonicalize_rule_overrides`
/// to reduce wire-string config keys + walker `additional_emitted_ids`
/// wire-string entries to their predicate-id intern.
///
/// Slicing a `&'static str` returns a `&'static str` — the result is
/// usable as a HashMap key without allocation.
#[inline]
pub(super) fn predicate_id_of_wire(s: &'static str) -> Option<&'static str> {
    s.find(':').map(|pos| &s[pos + 1..])
}

/// Return the wire-string form `"<scheme>:<predicate_id>"` for the
/// given rule id when both halves trace back to known `&'static str`
/// interns. Unlike `predicate_id_of_wire`, this needs to construct a
/// `String` because Rust cannot concat two distinct `&'static str` into
/// a single `&'static str` at runtime. Used by the canonicalize step
/// to register every registered-rule wire-string form as a config-key
/// alias. The returned `String` is intentionally leaked to obtain a
/// `&'static str` (the alternative — owning the canonical map's keys —
/// inflates the HashMap key shape across every lookup site).
///
/// Bounded by the number of registered rules (one leak per rule's
/// wire-string alias at `Engine::new` time, in the tens for the
/// current CAPCO rule set). The same leak pattern is used by
/// `marque_capco::vocabulary` for the per-token name interns.
pub(super) fn wire_string_of(id: marque_rules::RuleId) -> &'static str {
    // `format!("{}", id)` is infallible; `Box::leak` is infallible;
    // the return type is `&'static str` directly per the rust-reviewer
    // T044 MEDIUM-1 finding (the prior `Option<&'static str>` wrapper
    // was dead — every call site immediately unwrapped).
    Box::leak(format!("{}", id).into_boxed_str())
}

pub(super) fn canonicalize_rule_overrides(
    config: &mut Config,
    rule_sets: &[Box<dyn RuleSet<CapcoScheme>>],
    scheme: &CapcoScheme,
) -> Result<(), EngineConstructionError> {
    if config.rules.overrides.is_empty() {
        return Ok(());
    }

    // Build the ID-and-name → canonical-ID lookup. Both sides live in
    // `&'static str` (RuleId's inner slice, rule.name()), so the map's
    // keys and values are all `'static`.
    //
    // Dispatcher walkers like `BannerMatchesProjectedRule` (T026a)
    // register under one bookkeeping ID but emit diagnostics under
    // per-row catalog IDs (E035 / E040 in addition to E031). The
    // walker advertises those catalog (id, name) pairs through
    // `Rule::additional_emitted_ids`; each pair becomes a self-
    // canonical entry so a `.marque.toml` configuring the catalog ID
    // (`E035 = "warn"`) is accepted instead of failing as
    // `UnknownRuleOverride`. The per-emitted-id severity-override
    // path at lint time then resolves the override against the
    // diagnostic's emitted `rule` field.
    // T044: the override key universe MUST cover three user-facing
    // forms per PM-decisions OD-6 / OD-7:
    //   1. Wire-string form: `"capco:banner.banner-rollup.sar-portions-roll-up"`
    //      (what `Display` produces, what users type in
    //      `.marque.toml [rules]` keys).
    //   2. Predicate-id alone: `"banner.banner-rollup.sar-portions-roll-up"`
    //      (legacy alias; what `RuleId::predicate_id()` returns; what
    //      `Engine::emitted_id_overrides` keys on internally).
    //   3. Descriptive alias: `"sar-banner-rollup"` (Rule::name() or
    //      the second column of `additional_emitted_ids` /
    //      `bridge_emitted_rule_ids`).
    //
    // All three forms canonicalize to the predicate-id intern (form 2)
    // because the runtime lookup path
    // (`emitted_id_overrides.get(rule_id.predicate_id())`) uses form 2.
    // The wire-string form is sliced down to predicate-id at registration
    // time — slicing a `&'static str` preserves the static lifetime so
    // the canonical intern stays `&'static`.
    let mut known: HashMap<&'static str, &'static str> = HashMap::new();
    for rule_set in rule_sets {
        for rule in rule_set.rules() {
            let predicate = rule.id().predicate_id();
            let name = rule.name();
            // Predicate-id form (the canonical) maps to itself.
            known.insert(predicate, predicate);
            known.insert(name, predicate);
            // Also accept the wire-string form
            // (`"<scheme>:<predicate_id>"`) the user types.
            let wire = wire_string_of(rule.id());
            known.insert(wire, predicate);
            // Catalog IDs / names from dispatcher walkers — Agent A's
            // pattern makes `catalog_id` the wire-string form
            // (`"capco:banner..."`); we reduce it to its predicate-id
            // half so config keys typed as the wire string resolve to
            // the same intern as the runtime emit-site predicate id.
            for (catalog_id, catalog_name) in rule.additional_emitted_ids() {
                let canonical = predicate_id_of_wire(catalog_id).unwrap_or(catalog_id);
                known.insert(catalog_id, canonical);
                known.insert(canonical, canonical);
                known.insert(catalog_name, canonical);
            }
        }
    }
    // Bridge-emitted IDs (post-T044: the catalog row's `name` field IS
    // the predicate id — see the bridge no-op pass-through at
    // `bridge_constraint_diagnostic`). `scheme.bridge_emitted_rule_ids`
    // still returns `(legacy_capco_label, descriptive_alias)` pairs
    // because Agent A's rename of that surface is mid-flight at PR
    // authorship time; canonicalize accepts both via the same wire-
    // string → predicate-id reduction so `.marque.toml` configs that
    // already typed either form continue to resolve.
    for (bridge_id, bridge_name) in scheme.bridge_emitted_rule_ids() {
        let canonical = predicate_id_of_wire(bridge_id).unwrap_or(bridge_id);
        known.insert(bridge_id, canonical);
        known.insert(canonical, canonical);
        known.insert(bridge_name, canonical);
    }

    // Walk the raw overrides; resolve each key to its canonical ID, and
    // track which source key contributed each canonical entry so we can
    // report both sides of a conflict.
    let raw = std::mem::take(&mut config.rules.overrides);
    let mut by_rule: HashMap<&'static str, (String, String)> = HashMap::new();
    for (key, value) in raw {
        match known.get(key.as_str()) {
            Some(&canonical_id) => {
                if let Some((prev_key, prev_sev)) = by_rule.get(canonical_id) {
                    if prev_sev != &value {
                        return Err(EngineConstructionError::ConflictingRuleOverride {
                            rule_id: canonical_id.to_owned(),
                            keys: Box::new([prev_key.clone(), key]),
                            severities: Box::new([prev_sev.clone(), value]),
                        });
                    }
                    // Duplicate form, same severity — accept silently.
                } else {
                    by_rule.insert(canonical_id, (key, value));
                }
            }
            None => {
                let did_you_mean = suggest_closest(&key, known.keys().copied());
                return Err(EngineConstructionError::UnknownRuleOverride { key, did_you_mean });
            }
        }
    }

    config.rules.overrides = by_rule
        .into_iter()
        .map(|(id, (_, sev))| (id.to_owned(), sev))
        .collect();
    Ok(())
}

/// Best-effort string extraction from a `catch_unwind` payload.
///
/// Rust panic payloads are `Box<dyn Any + Send>`. The standard
/// shapes a `panic!()` produces are `&'static str` (literal message)
/// and `String` (formatted message); arbitrary types are also
/// permissible. We try the two common cases and fall back to a
/// generic placeholder so the warning we emit always carries
/// *something* identifying the rule even if a future crate panics
/// with a custom payload type.
pub(super) fn panic_payload_to_string(
    payload: &Box<dyn std::any::Any + Send + 'static>,
) -> std::borrow::Cow<'static, str> {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        std::borrow::Cow::Borrowed(*s)
    } else if let Some(s) = payload.downcast_ref::<String>() {
        std::borrow::Cow::Owned(s.clone())
    } else {
        std::borrow::Cow::Borrowed("<unstringifiable panic payload>")
    }
}

/// Return the closest known rule key (ID or name) to `needle` by
/// Levenshtein distance, if the closest candidate is within a small
/// edit-distance threshold. Threshold scales with `needle.len()`: short
/// strings only match on ≤ 1 edit, longer strings tolerate more.
///
/// Returns `None` when no candidate is close enough to be useful —
/// "did you mean 'REL-TO-noforn-supersession'?" for a user who typed
/// "E999" would be worse than no suggestion at all.
pub(super) fn suggest_closest<'a, I>(needle: &str, candidates: I) -> Option<String>
where
    I: Iterator<Item = &'a str>,
{
    // Keep the threshold tight so we don't suggest matches that share
    // only a couple of characters. The max-distance formula mirrors
    // what rustc uses for its "did you mean" hints:
    //   - length 0–3: 1 edit max (too short to suggest at all, really)
    //   - length 4–7: 2 edits max
    //   - length 8+:  3 edits max
    let max_distance = match needle.len() {
        0..=3 => 1,
        4..=7 => 2,
        _ => 3,
    };

    let mut best: Option<(&'a str, usize)> = None;
    for cand in candidates {
        let dist = levenshtein(needle, cand);
        if dist > max_distance {
            continue;
        }
        match best {
            Some((_, prev_dist)) if dist >= prev_dist => {}
            _ => best = Some((cand, dist)),
        }
    }
    best.map(|(cand, _)| cand.to_owned())
}

/// Levenshtein edit distance between two byte strings. Small, inlineable,
/// no external dependency — the engine crate is on the WASM-safe surface
/// and adding a new runtime dep for a once-per-construction helper would
/// be a disproportionate trade (Constitution III).
///
/// Operates on bytes, not `char`s: rule IDs and names are ASCII by
/// construction, so the byte-level diff equals the codepoint-level diff.
pub(super) fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    // Two-row DP: only the previous row is needed at any step.
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}
