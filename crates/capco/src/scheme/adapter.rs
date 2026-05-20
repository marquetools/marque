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
use marque_rules::{Confidence, FixIntent, FixSource, Message, MessageArgs, MessageTemplate};
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
    /// PR #578 wired this method to synthesize `FixIntent` values
    /// for the 15 declarative wrappers it retired into the bridge.
    /// The method returns `Some(FixIntent { ... })` for the
    /// following catalog row names and `None` otherwise:
    ///
    /// - `"E021/rd-frd-requires-noforn"` — `FactAdd(NOFORN, Portion)`
    ///   at confidence 0.95 per CAPCO-2016 §H.6 p104 + p111. Severity
    ///   is `Warn` per #559 close-out (the §123/§144 sharing-agreement
    ///   carve-out is documentary and Marque cannot verify it).
    /// - `"E038/nodis-or-exdis-requires-noforn"` — `FactAdd(NOFORN,
    ///   Portion | Page)` at confidence 1.0 per §H.9 p172 + p174.
    ///   The scope tracks `marking_type` (portion → `Portion`,
    ///   banner → `Page`); other marking types return `None`.
    /// - `"capco/noforn-conflicts-rel-to"` (when `marking_type ==
    ///   Portion`) — `FactRemove(REL_TO, Portion)` at confidence
    ///   1.0 per §H.8.
    /// - `"E054/relido-conflicts-noforn"` —
    ///   `FactRemove(RELIDO, Portion)` at confidence 0.95 per
    ///   §H.8 p154. E055 / E056 / E057 retired here in #559 close-out
    ///   (2026-05-19) / #618 (DISPLAY ONLY sibling); see
    ///   `crates/capco/src/scheme/rewrites/relido_clears.rs` for the
    ///   PageRewrite forms that replaced them.
    ///
    /// Other catalog families that ride the bridge take different
    /// paths and remain `None` here:
    ///
    /// - The class-floor catalog (E058 rows) produces no fixes —
    ///   every class-floor violation requires human review — so
    ///   there is nothing for this method to synthesize.
    /// - The SCI per-system catalog (E059 rows) DO produce fixes
    ///   (companion-insertion, `ORCON-USGOV → ORCON` token
    ///   replacement), but those fixes ride the direct bridge path
    ///   via [`Self::bridge_sci_per_system_diagnostics`] rather than
    ///   the `(name, attrs, marking_type)` side-table — a single
    ///   row can emit multiple diagnostics with distinct fixes,
    ///   which the side-table shape cannot disambiguate.
    /// - S004 (`"S004/rel-to-trigraph-suggest"`) is intentionally
    ///   NOT a catalog row — its replacement string is corpus-derived
    ///   during evaluation and the side-table cannot reproduce it.
    ///   S004 stays a registered walker (`RelToTrigraphSuggestRule`).
    pub fn fix_intent_by_name(
        &self,
        name: &str,
        attrs: &CanonicalAttrs,
        marking_type: MarkingType,
    ) -> Option<marque_rules::FixIntent<CapcoScheme>> {
        use crate::scheme::{TOK_NOFORN, TOK_REL_TO, TOK_RELIDO};

        match name {
            // #559 close-out (2026-05-19): renamed from
            // `E021/aea-requires-noforn`. Severity dropped from `Fix`
            // to `Warn`, so the FactAdd(NOFORN) intent now ships as
            // a suggestion the user can accept rather than an
            // auto-applied repair — §123/§144 sharing-agreement
            // determinations are documentary and Marque cannot
            // verify them at byte level. The carve-out (suppress
            // when REL TO / RELIDO present) lives in the helper
            // predicate, so any diagnostic that reaches this
            // intent has already cleared the byte-level carve-out
            // check.
            "E021/rd-frd-requires-noforn" => Some(FixIntent {
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
                    replacement: ReplacementIntent::fact_remove(
                        FactRef::Cve(TOK_REL_TO),
                        Scope::Portion,
                    ),
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
            // #559 close-out (2026-05-19) + #618: E055 / E056 / E057
            // removed from this arm. All three Conflicts rows were
            // retired in favor of PageRewrites at
            // `crates/capco/src/scheme/rewrites/relido_clears.rs`
            // (`capco/display-only-clears-relido` per §H.8 p154,
            // `capco/orcon-clears-relido` per §H.8 p136, and
            // `capco/orcon-usgov-clears-relido` per §H.8 p140). The
            // rewrite-side intent (FactRemove(RELIDO) at Scope::Page)
            // is embedded in each PageRewrite row's `action` directly,
            // so `fix_intent_by_name` has nothing to synthesize for
            // those names.
            "E054/relido-conflicts-noforn" => Some(FixIntent {
                replacement: ReplacementIntent::fact_remove(
                    FactRef::Cve(TOK_RELIDO),
                    Scope::Portion,
                ),
                confidence: Confidence::strict(0.95),
                feature_ids: Default::default(),
                message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            }),
            _ => None,
        }
    }

    /// Return a scheme-specific, user-friendly diagnostic message for a
    /// catalog constraint row identified by `name`.
    ///
    /// This is the engine-bridge counterpart to [`fix_intent_by_name`]:
    /// the bridge calls this hook after resolving the violation and
    /// prefers the returned `String` over the generic evaluator message
    /// (e.g., `"conflicting tokens: Token(TokenId(122)) and …"`) when
    /// `Some` is returned. `None` falls back to `ConstraintViolation.message`.
    ///
    /// Only dyadic `Constraint::Conflicts` and `Constraint::Requires`
    /// rows need entries here; `Constraint::Custom` helpers already
    /// produce well-formed messages in their own predicate bodies
    /// (see `e012_dual_classification`, `e014_joint_rel_to_coverage`,
    /// `e021_rd_frd_requires_noforn`, `e024_rd_precedence`,
    /// `e038_dos_dissem_requires_noforn`, `class_floor_emit`, etc.)
    /// and the generic evaluator never touches their `message` field.
    ///
    /// # Rows covered (PR #578 / issue-fix follow-up)
    ///
    /// - `"E015/non-us-requires-dissem"` — non-US classification
    ///   requires an explicit dissemination control marking per
    ///   CAPCO-2016 §H.7 p122 + §B.3 p20.
    /// - `"E016/joint-conflicts-restricted"` — JOINT cannot be used
    ///   with RESTRICTED per CAPCO-2016 §H.3 p56.
    /// - `"E036/joint-conflicts-hcs"` — JOINT cannot be used with HCS
    ///   markings per CAPCO-2016 §H.3 p57.
    /// - `"capco/noforn-conflicts-rel-to"` — NOFORN cannot be used
    ///   with REL TO per CAPCO-2016 §H.8 p145.
    /// - `"E037/nodis-conflicts-exdis"` — NODIS and EXDIS must not
    ///   coexist per CAPCO-2016 §H.9 p172 + p174.
    /// - `"E054/relido-conflicts-noforn"` — RELIDO cannot be used
    ///   with NOFORN per CAPCO-2016 §H.8 p154.
    ///
    /// # Class-floor and SCI-per-system catalog rows (PR 3c.2.C C7)
    ///
    /// PR 3c.2.C C7 (reviewer R-C1) extended the dispatch to cover the
    /// 27 `class-floor/*` + `E058/*` catalog rows and the 5
    /// `sci-per-system/*` catalog rows. Both prefixes route through
    /// [`find_class_floor_row`](Self::find_class_floor_row) /
    /// [`find_sci_per_system_row`](Self::find_sci_per_system_row)
    /// label lookups so the per-row [`MessageTemplate`] +
    /// [`Citation`] are read from the catalog rather than synthesized
    /// at the bridge. Without this, the bridge fell back to a generic
    /// `ConflictsWith` template + `[engine-internal]` citation
    /// sentinel.
    ///
    /// [`fix_intent_by_name`]: Self::fix_intent_by_name
    /// [`MessageTemplate`]: marque_rules::MessageTemplate
    /// [`Citation`]: marque_rules::Citation
    pub fn message_by_name(
        &self,
        name: &str,
        _attrs: &CanonicalAttrs,
        _marking_type: MarkingType,
    ) -> Option<marque_rules::Message> {
        use marque_rules::{Message, MessageArgs, MessageTemplate};
        // PR 3c.2.C C5: typed `Message` return per PM-C-1. Each
        // arm maps the constraint label to a closed-template +
        // closed-args record. Runtime byte text is dropped per G13;
        // the narrative descriptions live in the legacy `&str` arms
        // (preserved in git history) and the bridge's renderer
        // re-derives display text from `(template, args, source, span)`.

        // PR 3c.2.C C7 (R-C1): class-floor catalog rows. The 27
        // rows in CLASS_FLOOR_CATALOG all carry the
        // ClassificationFloorViolated template; the per-row category
        // axis is inferred from the row's `primary_kind` so the audit
        // record carries the right category. Per-row §-citations
        // resolve via citation_by_name (kept in lockstep with this
        // function for the same label set).
        if name.starts_with("class-floor/") || name.starts_with("E058/") {
            // Row lookup is O(27); cheap. Avoids re-encoding the
            // dispatch in two places.
            if let Some(row) = self.find_class_floor_row(name) {
                let category = match row.primary_kind {
                    Some(marque_ism::TokenKind::SciSystem) => Some(crate::scheme::CAT_SCI),
                    Some(marque_ism::TokenKind::SarIndicator) => Some(crate::scheme::CAT_SAR),
                    Some(marque_ism::TokenKind::AeaMarking) => Some(crate::scheme::CAT_AEA),
                    Some(marque_ism::TokenKind::DissemControl) => Some(crate::scheme::CAT_DISSEM),
                    // Classification-anchored rows (BALK/BOHEMIA/ATOMAL legacy compound text).
                    _ => Some(crate::scheme::CAT_CLASSIFICATION),
                };
                return Some(Message::new(
                    MessageTemplate::ClassificationFloorViolated,
                    MessageArgs {
                        category,
                        ..MessageArgs::default()
                    },
                ));
            }
            // Unknown class-floor row label — fall through to generic.
        }

        // PR 3c.2.C C7 (R-C1): sci-per-system catalog rows. The 5
        // rows in SCI_PER_SYSTEM_CATALOG all carry RequiredByPresence
        // semantics (CompanionRequired forbids absence of a required
        // companion; Custom rows enforce companion presence + forbid
        // conflict). Audit category is CAT_SCI.
        if name.starts_with("sci-per-system/") && self.find_sci_per_system_row(name).is_some() {
            return Some(Message::new(
                MessageTemplate::RequiredByPresence,
                MessageArgs {
                    category: Some(crate::scheme::CAT_SCI),
                    ..MessageArgs::default()
                },
            ));
        }

        match name {
            "E015/non-us-requires-dissem" => Some(Message::new(
                MessageTemplate::RequiredByPresence,
                MessageArgs {
                    category: Some(crate::scheme::CAT_DISSEM),
                    ..MessageArgs::default()
                },
            )),
            "E016/joint-conflicts-restricted" => Some(Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_JOINT_CLASSIFICATION),
                    ..MessageArgs::default()
                },
            )),
            "E036/joint-conflicts-hcs" => Some(Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_JOINT_CLASSIFICATION),
                    ..MessageArgs::default()
                },
            )),
            "capco/noforn-conflicts-rel-to" => Some(Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_DISSEM),
                    ..MessageArgs::default()
                },
            )),
            "E037/nodis-conflicts-exdis" => Some(Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                    ..MessageArgs::default()
                },
            )),
            "E054/relido-conflicts-noforn" => Some(Message::new(
                MessageTemplate::ConflictsWith,
                MessageArgs {
                    category: Some(crate::scheme::CAT_DISSEM),
                    ..MessageArgs::default()
                },
            )),
            _ => None,
        }
    }

    /// Typed [`Citation`](marque_rules::Citation) lookup for known
    /// constraint labels. Bridge layer per PR 3c.2.C C5 / PM-C-1.
    /// Returns `None` for labels not in the explicit mapping; the
    /// bridge falls back to a parser-based conversion from
    /// `ConstraintViolation.citation: &'static str` if needed.
    ///
    /// Constitution VIII propagation: each citation re-verified
    /// against `crates/capco/docs/CAPCO-2016.md` at PR 3c.2.C
    /// authorship.
    ///
    /// # Class-floor and SCI-per-system catalog rows (PR 3c.2.C C7)
    ///
    /// PR 3c.2.C C7 (reviewer R-C1) extended this dispatch to cover
    /// the 27 `class-floor/*` + `E058/*` catalog rows and the 5
    /// `sci-per-system/*` catalog rows. Both prefixes return the
    /// row's pre-computed [`Citation`](marque_rules::Citation)
    /// (the `citation_typed` field on each row) rather than the
    /// `[engine-internal]` sentinel previously emitted by the
    /// bridge fallback.
    pub fn citation_by_name(&self, name: &str) -> Option<marque_rules::Citation> {
        use marque_rules::{SectionLetter, capco};

        // PR 3c.2.C C7 (R-C1): class-floor catalog row citations.
        if (name.starts_with("class-floor/") || name.starts_with("E058/"))
            && let Some(row) = self.find_class_floor_row(name)
        {
            return Some(row.citation_typed);
        }

        // PR 3c.2.C C7 (R-C1): sci-per-system catalog row citations.
        if name.starts_with("sci-per-system/")
            && let Some(row) = self.find_sci_per_system_row(name)
        {
            return Some(row.citation_typed);
        }

        match name {
            // E015 §H.7 p122 (FGI grammar) + §B.3 p20 (caveated
            // definition); typed anchor at §H.7 p122.
            "E015/non-us-requires-dissem" => Some(capco(SectionLetter::H, 7, 122)),
            // E016 §H.3 p56 (JOINT grammar).
            "E016/joint-conflicts-restricted" => Some(capco(SectionLetter::H, 3, 56)),
            // E036 §H.3 p57 (Derivative Use).
            "E036/joint-conflicts-hcs" => Some(capco(SectionLetter::H, 3, 57)),
            // §H.8 p145 (NOFORN-dominates rule).
            "capco/noforn-conflicts-rel-to" => Some(capco(SectionLetter::H, 8, 145)),
            // E037 §H.9 p172 (EXDIS) + §H.9 p174 (NODIS); typed
            // anchor at §H.9 p172.
            "E037/nodis-conflicts-exdis" => Some(capco(SectionLetter::H, 9, 172)),
            // E054 §H.8 p154 (RELIDO grammar).
            "E054/relido-conflicts-noforn" => Some(capco(SectionLetter::H, 8, 154)),
            _ => None,
        }
    }

    /// O(27) linear lookup for a `ClassFloorRow` by `name`. The
    /// catalog has 27 rows; the linear scan is faster than a
    /// `&'static phf::Map` build-time cost for this size. Used by
    /// [`message_by_name`](Self::message_by_name) and
    /// [`citation_by_name`](Self::citation_by_name) — the bridge
    /// hook from `marque-engine`'s `bridge_constraint_diagnostic` —
    /// to surface per-row [`MessageTemplate`](marque_rules::MessageTemplate)
    /// + [`Citation`](marque_rules::Citation) on emission.
    ///
    /// Returns `None` when `name` doesn't match any row — typically
    /// indicates a stale label in the engine's constraint catalog
    /// or a typo in a new row's `name` field. The bridge falls back
    /// to a generic template + sentinel citation in that case (the
    /// pre-PR-3c.2.C-C7 behavior).
    fn find_class_floor_row(&self, name: &str) -> Option<&'static super::ClassFloorRow> {
        super::CLASS_FLOOR_CATALOG.iter().find(|r| r.name == name)
    }

    /// O(5) linear lookup for a `SciPerSystemRow` by `name`. The
    /// SCI per-system catalog has 5 family rows; lookup is O(N=5).
    /// Mirrors [`find_class_floor_row`](Self::find_class_floor_row).
    fn find_sci_per_system_row(&self, name: &str) -> Option<&'static super::SciPerSystemRow> {
        super::SCI_PER_SYSTEM_CATALOG
            .iter()
            .find(|r| r.name == name)
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
            ("E021", "rd-frd-requires-noforn"),
            ("E024", "rd-precedence"),
            ("E037", "nodis-conflicts-exdis"),
            ("E038", "nodis-or-exdis-requires-noforn"),
            ("E053", "noforn-conflicts-rel-to"),
            ("E054", "relido-conflicts-noforn"),
            // #559 close-out (2026-05-19): E055 / E056 / E057
            // retired here. The constraint rows moved to
            // `relido_clears.rs` as PageRewrites (one per
            // dominator: DISPLAY ONLY / ORCON / ORCON-USGOV →
            // FactRemove(RELIDO) at Scope::Page) and emit through
            // the PageRewrite path, not the constraint-catalog
            // bridge. Their canonicalize_rule_overrides aliases are
            // no longer accepted; per project memory
            // `feedback_pre_users_no_deprecation_phasing.md` marque
            // is pre-users and we don't carry alias maps.
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
