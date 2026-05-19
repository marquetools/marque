// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` — the scheme adapter struct + `CapcoParseError`.
//!
//! Hosts the `CapcoScheme` struct, the manual `Debug` / `Default` impls,
//! the two ctor `impl CapcoScheme` blocks (`new` / `with_rewrites` /
//! `with_extra_rewrite_for_tests`), the `CapcoParseError` enum, and the
//! ~336-LOC inherent `impl CapcoScheme { evaluate_named_constraint /
//! fix_intent_by_name / has_diagnostic_constraints /
//! bridge_emitted_rule_ids / bridge_sci_per_system_diagnostics }` block
//! that hosts the scheme-private predicate helpers for the engine
//! bridge.
//!
//! Carved out from `scheme/mod.rs` per the Stage 2 PR B hub-split
//! (issue #466). Module contents are byte-identical to the pre-split
//! source — imports adjusted to reach helpers via `super::constraints`
//! / `super::predicates::*` / `super::rewrites`, with the
//! `SCI_PER_SYSTEM_CATALOG` and `sci_per_system_emit` symbols crossing
//! over from the new `sci_per_system.rs` / `constraints/helpers.rs`
//! homes (re-exported through `mod.rs`).

use super::constraints::{self, sci_per_system_emit};
use super::*;
use marque_ism::{CanonicalAttrs, MarkingType};
use marque_rules::{
    Confidence, FixIntent, FixSource, Message, MessageArgs, MessageTemplate,
};
use marque_scheme::{Category, Constraint, FactRef, ReplacementIntent, Scope, Template};

/// CAPCO's implementation of `MarkingScheme`.
///
/// Stateless; construct with `CapcoScheme::new()` and pass into the
/// engine. Phase A's engine doesn't consume the trait yet — this impl
/// exists so the equivalence tests can run.
///
/// A manual `Debug` impl is provided so generic types parameterized
/// over the scheme (`Diagnostic<S>`, `AppliedFix<S>`, `LintResult` /
/// `FixResult` inside `marque-engine`) can derive `Debug` via the
/// standard derive-macro field-bound expansion. The implementation
/// prints only the struct shell — the static-table fields are large
/// and not useful for debug output, and `PageRewrite<S>` does not
/// implement `Debug`.
pub struct CapcoScheme {
    pub(super) categories: Vec<Category>,
    pub(super) constraints: Vec<Constraint>,
    pub(super) templates: Vec<Template>,
    pub(super) page_rewrites: Vec<PageRewrite<CapcoScheme>>,
}

impl std::fmt::Debug for CapcoScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapcoScheme")
            .field("categories.len", &self.categories.len())
            .field("constraints.len", &self.constraints.len())
            .field("templates.len", &self.templates.len())
            .field("page_rewrites.len", &self.page_rewrites.len())
            .finish()
    }
}

impl Default for CapcoScheme {
    fn default() -> Self {
        Self::new()
    }
}

impl CapcoScheme {
    pub fn new() -> Self {
        Self {
            categories: constraints::build_categories(),
            constraints: constraints::build_constraints(),
            templates: Vec::new(), // Phase A does not model templates yet
            page_rewrites: rewrites::build_page_rewrites(),
        }
    }
}

impl CapcoScheme {
    /// Test-only constructor that lets tests install arbitrary
    /// `PageRewrite` entries, exercising the declarative dispatch
    /// path (`CategoryPredicate::Contains` / `Empty`,
    /// `CategoryAction::Clear` / `Replace` / `Intent`) with
    /// test-provided rewrites.
    ///
    /// Exposed publicly so integration tests under `crates/capco/tests/`
    /// can exercise scheme-level behaviors (page-rewrite projection,
    /// `CategoryAction::Intent` apply paths). Production code MUST NOT
    /// use this constructor — it bypasses `build_page_rewrites()`'s
    /// curated CAPCO-2016 table. The `_for_tests` suffix on the
    /// related [`with_extra_rewrite_for_tests`](Self::with_extra_rewrite_for_tests)
    /// helper makes the intent explicit.
    ///
    /// Bypasses `validate_intent_rewrites` (the engine's
    /// construction-time validation pass for `CategoryAction::Intent`
    /// payloads). Tests that want to exercise validation MUST feed
    /// the constructed scheme to `Engine::new` so the validation
    /// runs over the test rewrites.
    #[doc(hidden)]
    pub fn with_rewrites(rewrites: Vec<PageRewrite<CapcoScheme>>) -> Self {
        Self {
            categories: constraints::build_categories(),
            constraints: constraints::build_constraints(),
            templates: Vec::new(),
            page_rewrites: rewrites,
        }
    }

    /// Append one extra `PageRewrite` to a scheme's table, returning
    /// the modified scheme. Test-only — production code MUST NOT use
    /// this; the production rewrite table is the curated
    /// CAPCO-2016 table built by `build_page_rewrites()`.
    ///
    /// Bypasses `validate_intent_rewrites` (the engine's
    /// construction-time validation pass). Tests that want to exercise
    /// validation MUST construct the scheme separately and feed it to
    /// `Engine::new` so the validation runs over the appended rewrite.
    #[doc(hidden)]
    pub fn with_extra_rewrite_for_tests(mut self, rewrite: PageRewrite<CapcoScheme>) -> Self {
        self.page_rewrites.push(rewrite);
        self
    }
}

/// Parse errors surfaced by `CapcoScheme::parse`.
///
/// Phase A does not actually parse through the trait — callers continue
/// to use `marque_core::Parser` directly — so `parse()` unconditionally
/// returns [`CapcoParseError::NotImplemented`]. Phase B/E will wrap
/// `marque-core`'s `CoreError` here once parsing is routed through the
/// scheme trait (and the `(C)` ambiguity surface lands).
#[derive(Debug)]
pub enum CapcoParseError {
    /// `CapcoScheme::parse` is intentionally unimplemented in Phase A.
    /// Use `marque_core::Parser` for actual parsing until Phase B/E
    /// routes it through the scheme trait.
    NotImplemented,
}

impl CapcoScheme {
    /// Look up the [`FixIntent`] a catalog row produces against
    /// `attrs`, when one is defined.
    ///
    /// This is the engine-bridge counterpart to the scheme's
    /// [`MarkingScheme::validate`] path. The lint loop walks
    /// `scheme.validate(...)`, gets back a stream of
    /// [`ConstraintViolation`] values whose `span` and `severity` are
    /// populated by catalog rows that want to fire as user-facing
    /// diagnostics. For each such violation, the engine asks the
    /// scheme: *given this row name and these attributes, is there a
    /// `FixIntent` you'd like attached to the diagnostic?* For most
    /// rows the answer is `None`. For rows whose CAPCO §-citation
    /// commits to a specific repair shape (companion-insert for
    /// HCS-O / HCS-P sub / SI-G; subtractive for ORCON-USGOV conflict
    /// cases — see CAPCO-2016 §H.4 p64 / p66 / p68 / p80), the helper
    /// constructs the matching [`FixIntent`].
    ///
    /// # Why scheme-side, not on `ConstraintViolation`
    ///
    /// [`FixIntent<S>`] lives in `marque-rules`, and `marque-rules`
    /// depends on `marque-scheme` (Constitution VII Appendix D —
    /// post-PR-3c.A graph). Attaching a `fix_intent: Option<FixIntent<S>>`
    /// field to `ConstraintViolation` (in `marque-scheme`) would invert
    /// the graph and create a cycle. The bridge instead reconstructs
    /// the [`FixIntent`] from the row name on the way out — this is
    /// the side-table pattern the now-retired walker rules
    /// (`DeclarativeClassFloorRule` retired in PR 3c.B Commit 7.3;
    /// `DeclarativeSciPerSystemRule` retired in PR 3c.B Commit 7.4)
    /// used internally.
    ///
    /// # Current contract
    ///
    /// This method returns `None` for every input today. The two
    /// catalog families that ride the bridge take different paths:
    ///
    /// - The class-floor catalog (E058 rows) produces no fixes —
    ///   every class-floor violation requires human review — so
    ///   there is nothing for this method to synthesize.
    /// - The SCI per-system catalog (E059 rows) DO produce fixes
    ///   (companion-insertion, `ORCON-USGOV → ORCON` token
    ///   replacement), but those fixes ride the direct bridge path
    ///   via [`Self::bridge_sci_per_system_diagnostics`] rather than
    ///   the `(name, attrs)` side-table — a single row can emit
    ///   multiple diagnostics with distinct fixes, which the
    ///   `(name, attrs)` shape cannot disambiguate.
    ///
    /// The method is kept as a stable scheme-side entry point so
    /// future catalog families that DO fit the `(name, attrs) → fix`
    /// shape can be wired in without changing the engine bridge
    /// surface.
    pub fn fix_intent_by_name(
        &self,
        name: &str,
        attrs: &CanonicalAttrs,
        marking_type: MarkingType,
    ) -> Option<marque_rules::FixIntent<CapcoScheme>> {
        use crate::scheme::{TOK_NOFORN, TOK_RELIDO, TOK_REL_TO};

        match name {
            "E021/aea-requires-noforn" => Some(FixIntent {
                replacement: ReplacementIntent::FactAdd {
                    token: FactRef::Cve(TOK_NOFORN),
                    scope: Scope::Portion,
                },
                confidence: Confidence::strict(0.95),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            }),
            "E038/nodis-or-exdis-requires-noforn" => {
                let trigger_token = attrs
                    .non_ic_dissem
                    .iter()
                    .find_map(|d| match d {
                        marque_ism::NonIcDissem::Nodis => Some(crate::scheme::TOK_NODIS),
                        marque_ism::NonIcDissem::Exdis => Some(crate::scheme::TOK_EXDIS),
                        _ => None,
                    })
                    .unwrap_or(crate::scheme::TOK_NODIS); // Should be unreachable if predicate fired

                let scope = match marking_type {
                    MarkingType::Portion => Scope::Portion,
                    MarkingType::Banner => Scope::Page,
                    _ => return None,
                };
                Some(FixIntent {
                    replacement: ReplacementIntent::FactAdd {
                        token: FactRef::Cve(TOK_NOFORN),
                        scope,
                    },
                    confidence: Confidence::strict(1.0),
                    feature_ids: Default::default(),
                    message: Message::new(
                        MessageTemplate::RequiredByPresence,
                        MessageArgs {
                            token: Some(trigger_token),
                            expected_token: Some(TOK_NOFORN),
                            ..MessageArgs::default()
                        },
                    ),
                    source: FixSource::BuiltinRule,
                    migration_ref: None,
                })
            }
            "capco/noforn-conflicts-rel-to" if marking_type == MarkingType::Portion => {
                Some(FixIntent {
                    replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_REL_TO), Scope::Portion),
                    confidence: Confidence::strict(1.0),
                    feature_ids: Default::default(),
                    message: Message::new(
                        MessageTemplate::ConflictsWith,
                        MessageArgs {
                            token: Some(TOK_REL_TO),
                            expected_token: Some(TOK_NOFORN),
                            ..MessageArgs::default()
                        },
                    ),
                    source: FixSource::BuiltinRule,
                    migration_ref: None,
                })
            }
            "E054/relido-conflicts-noforn"
            | "E055/relido-conflicts-display-only"
            | "E056/orcon-conflicts-relido"
            | "E057/orcon-usgov-conflicts-relido" => Some(FixIntent {
                replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
                confidence: Confidence::strict(0.95),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            }),
            _ => None,
        }
    }

    /// Reports whether the scheme's `Constraint::Custom` catalog has
    /// any rows that *can* produce user-facing diagnostics (i.e., rows
    /// whose `evaluate_custom` arm populates `ConstraintViolation::span`
    /// AND `::severity`). Used by the engine's constraint-catalog
    /// bridge (`crates/engine/src/engine.rs` lint loop) to short-
    /// circuit the whole `scheme.validate(...)` walk — including the
    /// per-candidate `CapcoMarking::from(attrs.clone())` allocation —
    /// when no catalog row could possibly fire.
    ///
    /// # Why a static `true` now (PR 3c.B Commit 7.3)
    ///
    /// PR 3c.B Commit 7.3 retired `DeclarativeClassFloorRule` (E058)
    /// and rewired its 27 class-floor catalog rows to populate
    /// `ConstraintViolation::span` (via [`class_floor_anchor_span`])
    /// and `::severity` (from `ClassFloorRow::severity`) directly in
    /// [`class_floor_emit`]. The bridge is the sole emitter for the
    /// class-floor rule set as of this commit; the previous walker
    /// path no longer exists. PR 3c.B Commit 7.4 retired
    /// `DeclarativeSciPerSystemRule` (E059) via a separate
    /// direct-path mechanism — `bridge_sci_per_system_diagnostics`
    /// — that does NOT participate in the `validate()` /
    /// `ConstraintViolation` envelope flow (decision record
    /// Amendment 6). The 5 SCI per-system rows therefore do not
    /// contribute to this predicate's value; it stays `true`
    /// because the 27 class-floor rows from 7.3 already require
    /// the bridge walk.
    ///
    /// # Why static (not derived from the catalog at runtime)
    ///
    /// Catalog membership doesn't change across the engine's
    /// lifetime — `build_constraints()` is invoked once at
    /// `CapcoScheme::new()` and never mutated. A runtime walk over
    /// `self.constraints` to look for "any Custom row that produces
    /// span/severity" would itself defeat the optimization (the data
    /// we're avoiding fetching is the per-candidate walk's output;
    /// learning that the catalog has zero such rows shouldn't itself
    /// require a per-candidate walk). The constant `true` here
    /// reflects the post-7.3 catalog state and is a one-line override
    /// for any future scheme that wires no diagnostic-shape rows.
    pub fn has_diagnostic_constraints(&self) -> bool {
        true
    }

    /// Rule IDs emitted by the engine's constraint-catalog bridge that
    /// do not correspond to any registered `Rule::id()`. Each entry is
    /// a `(rule_id, name)` pair shaped to match the existing
    /// `Rule::additional_emitted_ids()` walker convention so the
    /// engine's `canonicalize_rule_overrides` validator can accept
    /// `.marque.toml [rules] <id-or-name> = "off"` references to
    /// these IDs without an `UnknownRuleOverride` failure.
    ///
    /// # User-facing surface
    ///
    /// Both fields are user-facing config keys: `canonicalize_rule_overrides`
    /// inserts the `rule_id` and the `name` into the known-key map,
    /// aliasing both to the canonical ID. A `.marque.toml` entry
    /// `[rules] class-floor-catalog = "off"` is therefore silently
    /// accepted as an alias for `[rules] E058 = "off"`. The shorter
    /// `E058` form is the recommended one (matches what `Diagnostic.rule`
    /// emits, what audit-stream consumers see, and what `did_you_mean`
    /// suggests for typos); the longer name is the descriptive alias
    /// users discovering rule IDs in source might also reach for.
    /// This convention parallels the `id-or-name` aliasing every
    /// registered `Rule` already accepts.
    ///
    /// # Entries (PR 3c.B Commit 7.3)
    ///
    ///   - `("E058", "class-floor-catalog")` — retired
    ///     `DeclarativeClassFloorRule` walker. The 27 class-floor
    ///     catalog rows fire through the bridge with
    ///     `Diagnostic.rule = "E058"`; the bridge folds the per-row
    ///     `E058/...` / `class-floor/...` constraint-label names to
    ///     this collapsed ID.
    ///
    /// PR 3c.B Commit 7.4 added `("E059", "sci-per-system-catalog")`.
    ///
    /// PR #578 added the remaining declarative catalog IDs.
    pub fn bridge_emitted_rule_ids(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("E058", "class-floor-catalog"),
            ("E059", "sci-per-system-catalog"),
            ("E010", "HCS-system-constraints"),
            ("E012", "dual-classification"),
            ("E014", "joint-requires-rel-to-coverage"),
            ("E015", "non-us-requires-dissem"),
            ("E016", "joint-conflicts-restricted"),
            ("E036", "joint-conflicts-hcs"),
            ("E021", "aea-requires-noforn"),
            ("E024", "rd-precedence"),
            ("E036", "joint-conflicts-hcs"),
            ("E037", "nodis-conflicts-exdis"),
            ("E038", "nodis-or-exdis-requires-noforn"),
            ("E053", "noforn-conflicts-rel-to"),
            ("E054", "relido-conflicts-noforn"),
            ("E055", "relido-conflicts-display-only"),
            ("E056", "orcon-conflicts-relido"),
            ("E057", "orcon-usgov-conflicts-relido"),
        ]
    }

    /// Walk the SCI per-system catalog and return one `Diagnostic` per
    /// firing emit-branch, with the row's `FixProposal` attached
    /// (matching the retired `DeclarativeSciPerSystemRule` walker's
    /// output byte-for-byte).
    ///
    /// # Why this bypasses the `ConstraintViolation` envelope
    ///
    /// The class-floor catalog (PR 3c.B Commit 7.3) emits diagnostics
    /// through the standard `MarkingScheme::validate()` →
    /// `Vec<ConstraintViolation>` → engine bridge path because its
    /// rows produce no fixes (every class-floor violation requires
    /// human review). The SCI per-system catalog rows DO produce
    /// fixes — companion-insertion at the dissem-block anchor and
    /// `ORCON-USGOV → ORCON` token replacement — and a single row
    /// can emit multiple diagnostics (HCS-O missing ORCON AND
    /// missing NOFORN → 2 violations, each with its own fix).
    ///
    /// `ConstraintViolation` (in `marque-scheme`) cannot carry a
    /// `FixProposal` (in `marque-rules`) because `marque-scheme` is
    /// the workspace dependency-graph leaf (Constitution VII). A
    /// `fix_intent_by_name(name, attrs)` helper called per
    /// `ConstraintViolation` cannot disambiguate "which of N
    /// violations on this row do I synthesize a fix for" with only
    /// `(name, attrs)` as input. Rather than thread message text
    /// through the bridge for disambiguation, the SCI per-system
    /// rows take the direct path: this method returns full
    /// `Diagnostic` values straight from `sci_per_system_emit`, the
    /// engine bridge invokes it once per candidate (gated on
    /// `[rules] E059 != "off"`), and the existing fix-promotion
    /// path treats each diagnostic identically to a registered
    /// `Rule` impl's output.
    ///
    /// # Severity override handling
    ///
    /// The caller passes the resolved `Severity` for `E059`
    /// (`severity_override` = the `[rules] E059 = ...` config, or
    /// `None` to use each diagnostic's authoring severity). When
    /// `severity_override = Some(Severity::Off)` the method returns
    /// an empty `Vec` (FR-008: an `Off`-severity diagnostic is
    /// unrepresentable). A non-`Off` override replaces the per-
    /// diagnostic severity uniformly.
    pub fn bridge_sci_per_system_diagnostics(
        &self,
        attrs: &CanonicalAttrs,
        candidate_span: marque_scheme::Span,
        fix_scope: marque_scheme::Scope,
        severity_override: Option<marque_rules::Severity>,
    ) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
        // FR-008 early-out — `Off` suppresses the entire catalog.
        if matches!(severity_override, Some(marque_rules::Severity::Off)) {
            return Vec::new();
        }
        // Hot-path early-out — every SCI per-system row is SCI-axis-
        // only. If no SCI markings are present, no row can fire and
        // the catalog walk costs effectively nothing. Mirrors the
        // retired walker's `attrs.sci_markings.is_empty()` guard.
        if attrs.sci_markings.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for row in SCI_PER_SYSTEM_CATALOG {
            if !(row.presence)(attrs) {
                continue;
            }
            for mut diag in sci_per_system_emit(attrs, candidate_span, fix_scope, row) {
                if let Some(sev) = severity_override {
                    diag.severity = sev;
                }
                out.push(diag);
            }
        }
        out
    }
}
