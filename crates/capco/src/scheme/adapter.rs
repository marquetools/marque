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

use marque_ism::CanonicalAttrs;
use marque_scheme::{Category, Constraint, ConstraintViolation, PageRewrite, Template};

use super::constraints::{self, sci_per_system_emit};
use super::predicates::{collect_present_tokens, evaluate_custom_by_attrs, satisfies_attrs};
use super::rewrites;
use super::*;

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
    /// Evaluate a single constraint by `name` against raw
    /// `CanonicalAttrs`. Fast path for rule wrappers that want "did
    /// this specific predicate fire?" without the overhead of a
    /// full `MarkingScheme::validate()` call.
    ///
    /// Compared to `scheme.validate(&CapcoMarking::new(attrs.clone()))`:
    /// - **No `CanonicalAttrs` clone** — works on the borrow directly
    /// - **No full catalog walk** — linear `find` by `name` over the
    ///   ~13 catalog entries, then single dispatch. O(1) effectively;
    ///   the filter step that the wrappers previously did after
    ///   `validate()` is eliminated.
    /// - **No `CapcoMarking` wrap** — delegates straight to the
    ///   free-function predicates (`satisfies_attrs`,
    ///   `evaluate_custom_by_attrs`), which is also what the trait
    ///   impls use.
    ///
    /// Contract: the emitted `ConstraintViolation.constraint_label`
    /// and `.citation` are populated from the catalog entry's
    /// declared `name` and `label`, matching the normalization that
    /// `marque_scheme::constraint::evaluate` performs in its
    /// `Custom` arm. Dyadic-variant violations carry a generic
    /// "conflicting tokens" / "token X requires Y" message — same
    /// as the generic evaluator — because the wrapper layer is
    /// responsible for constructing the user-visible diagnostic
    /// text, not the scheme.
    pub(crate) fn evaluate_named_constraint(
        &self,
        attrs: &marque_ism::CanonicalAttrs,
        name: &'static str,
    ) -> Vec<ConstraintViolation> {
        let Some(c) = self.constraints.iter().find(|c| c.name() == name) else {
            return Vec::new();
        };
        let label = c.label();
        match c {
            Constraint::Conflicts { left, right, .. } => {
                if satisfies_attrs(attrs, left) && satisfies_attrs(attrs, right) {
                    vec![ConstraintViolation {
                        constraint_label: name,
                        message: format!("conflicting tokens: {left:?} and {right:?}"),
                        citation: label,
                        span: None,
                        severity: None,
                    }]
                } else {
                    Vec::new()
                }
            }
            Constraint::Requires { left, right, .. } => {
                if satisfies_attrs(attrs, left) && !satisfies_attrs(attrs, right) {
                    vec![ConstraintViolation {
                        constraint_label: name,
                        message: format!("token {left:?} requires {right:?} but it is missing"),
                        citation: label,
                        span: None,
                        severity: None,
                    }]
                } else {
                    Vec::new()
                }
            }
            // `Supersedes` is a lattice hint for banner roll-up, not
            // a violation trigger. No diagnostic emission.
            // Note: `Constraint::Implies` was retired in PR 3.7 T108g
            // (decisions.md D19 C) — fact-propagation is handled by
            // the closure operator (ClosureRule) instead.
            Constraint::Supersedes { .. } => Vec::new(),
            // `ConflictsWithFamily` evaluates LHS-presence plus the
            // distributive expansion: emit one violation per token
            // present in `attrs` for which `family.0` holds. Mirrors
            // `marque_scheme::constraint::evaluate`'s
            // `ConflictsWithFamily` arm so wrapper-layer callers
            // (`rules_declarative.rs::violations_for`) get identical
            // diagnostics to the generic walker. Per Copilot PR 3.7
            // review: prior to this fix the fast path treated
            // `ConflictsWithFamily` as a no-op, silently dropping
            // every family-row diagnostic — that was a regression
            // the moment any wrapper dispatched by a family-row name.
            Constraint::ConflictsWithFamily { left, family, .. } => {
                if !satisfies_attrs(attrs, left) {
                    Vec::new()
                } else {
                    collect_present_tokens(attrs)
                        .into_iter()
                        .filter(|t| family.0(t))
                        .map(|present| ConstraintViolation {
                            // G13: `TokenRef` carries only integer IDs
                            // (`TokenId`/`CategoryId`), never document
                            // content bytes. Safe to format into the
                            // audit-stream message per Constitution V
                            // Principle V audit-content-ignorance.
                            constraint_label: name,
                            message: format!(
                                "conflicting tokens: {left:?} and {present:?} (family match)"
                            ),
                            citation: label,
                            span: None,
                            severity: None,
                        })
                        .collect()
                }
            }
            Constraint::Custom { .. } => evaluate_custom_by_attrs(attrs, name)
                .into_iter()
                .map(|mut v| {
                    v.constraint_label = name;
                    v.citation = label;
                    v
                })
                .collect(),
        }
    }

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
    /// the side-table pattern the now-retiring walker rules
    /// (`DeclarativeClassFloorRule`, `DeclarativeSciPerSystemRule`)
    /// used internally; PR 3c.B Commit 7.4 relocates the table to the
    /// scheme so the walker can be deleted.
    ///
    /// # Cold-land contract (PR 3c.B Commit 7.2)
    ///
    /// This method returns `None` for every input in Commit 7.2; the
    /// only catalog rows that produce fixes today are E059's five
    /// SCI-per-system rows (companion-insert, HCS-O / HCS-P sub /
    /// SI-G; forbid-companion, HCS-P sub vs ORCON-USGOV). Those rows
    /// still fire diagnostics through the walker until Commit 7.4
    /// retires the walker and populates this helper. `None` is the
    /// safe shape — the engine attaches no fix and the diagnostic
    /// flows through unchanged. No behavior change at 7.2; the only
    /// purpose of the method's existence here is to give the engine
    /// bridge a stable scheme-side entry point to query.
    pub fn fix_intent_by_name(
        &self,
        _name: &str,
        _attrs: &CanonicalAttrs,
    ) -> Option<marque_rules::FixIntent<CapcoScheme>> {
        // PR 3c.B Commit 7.4 will populate the E059 catalog rows here.
        // Until then, the walker rule `DeclarativeSciPerSystemRule`
        // owns the E059 fixes via its own side-table.
        None
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
    pub fn bridge_emitted_rule_ids(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("E058", "class-floor-catalog"),
            ("E059", "sci-per-system-catalog"),
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
        candidate_span: marque_ism::Span,
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
