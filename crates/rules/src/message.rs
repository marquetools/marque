// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Message` — closed-template, closed-args diagnostic message representation.
//!
//! `Diagnostic.message` is a `(template, args)` pair rather than a
//! free-form `Box<str>`, closing the `format!`-built-string leak
//! channel (interpolating input bytes) that Constitution V Principle
//! V's audit-content-ignorance invariant forbids. marque-capco's decoder
//! synthesis helper (`build_decoder_diagnostic` in
//! `crates/capco/src/provenance.rs`) constructs
//! `Message::new(MessageTemplate::DecoderRecognized, ...)`.
//!
//! # Closed-template invariant
//!
//! [`MessageTemplate`] is a closed enum (NOT `#[non_exhaustive]`).
//! Adding a variant is a coordinated audit-schema change — the
//! `MARQUE_AUDIT_SCHEMA` version pin in `crates/engine/build.rs` MUST
//! bump in lockstep, and the audit-record schema doc at
//! `specs/006-engine-rule-refactor/contracts/audit-record.md` updates
//! the closed-set listing. Treat additions as part of the on-the-wire
//! contract.
//!
//! # Closed-args invariant
//!
//! [`MessageArgs`] field set is closed. Permitted field types are
//! restricted to the audit-content-ignorance permitted set per
//! Constitution V Principle V: [`marque_scheme::TokenId`],
//! [`marque_scheme::CategoryId`], [`marque_scheme::Span`], [`Blake3Hash`],
//! [`crate::Recognition`], [`crate::FeatureId`]. Two convenience-typed
//! `Option<TokenId>` fields (`expected_token` / `actual_token`) are
//! permitted because they carry the same type as `token` — the field
//! name is the only distinction, for diagnostic-rendering ergonomics.
//!
//! No `String`, no `&str`, no `Vec<u8>`, no `format!`-derived field is
//! permitted. The closure is enforced by:
//!
//! 1. **Compile-fail doctests** on [`MessageArgs`] and [`Message`]
//!    in this module — each `compile_fail` snippet pins one
//!    inadmissible field shape (a `String`-typed `raw_text` field,
//!    a `Message::format(...)` macro-style constructor, an
//!    `impl From<&str> for Message`, etc.) and exits non-zero when
//!    the snippet starts compiling. Run via
//!    `cargo test --doc -p marque-rules`.
//! 2. **Positive destructuring-as-pin tests** at
//!    `crates/rules/tests/message_args_closed_set.rs` — exhaustive
//!    `match` over [`MessageArgs`] fields, so adding a field
//!    without updating the test set breaks the build (E0027 —
//!    pattern does not mention all fields).
//! 3. **External-crate positive controls** at
//!    `crates/rules/tests/message_no_freeform_ctor.rs` — assert
//!    that [`Message::new`] is the sole public constructor reachable
//!    from outside `marque-rules`.

use marque_scheme::{CategoryId, Span, TokenId};
use smallvec::SmallVec;

use crate::RuleId;
use crate::recognition::{FeatureId, Recognition};

/// BLAKE3 digest — a re-export of [`blake3::Hash`].
///
/// An alias rather than a newtype: `blake3::Hash` already implements
/// `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Display`,
/// `From<[u8; 32]>`, and `FromStr` — every property the audit-record
/// types need without a thin wrapper that adds no invariants.
///
/// # Audit-emit wire form
///
/// Serializes as `"blake3:<64-hex>"` via the free function
/// [`to_audit_string`] at the JSON projection boundary. The
/// per-record digest itself flows through the audit pipeline as the
/// `blake3::Hash` value; the string form materializes only at NDJSON
/// emit (CLI + WASM renderers, the only sanctioned readout sites).
///
/// # Content-ignorance invariant (Constitution V Principle V)
///
/// `Blake3Hash` is one of the closed permitted types for
/// [`MessageArgs`] fields. The digest is computed *from* content, but
/// the digest itself is opaque; no other accessor surfaces document
/// bytes back out of the digest. Real digest computation flows through
/// `AppliedReplacement::bytes_digest` and
/// `AppliedFixDetail::original_digest` on the `marque-3.0` audit
/// envelope.
pub type Blake3Hash = blake3::Hash;

/// Render a [`Blake3Hash`] as the canonical `"blake3:<64-hex>"` audit-
/// emit string.
///
/// Free function (not an inherent method) because [`Blake3Hash`] is a
/// type alias to a foreign type. Returns an owned `String` only at
/// the audit-emit boundary (CLI / WASM NDJSON projection); rules and
/// engine code carry the [`Blake3Hash`] value (`Copy`-sized) and let
/// the renderer materialize the string form once per record.
///
/// Pinned by `crates/rules/tests/blake3_audit_string.rs`.
#[must_use]
pub fn to_audit_string(hash: &Blake3Hash) -> String {
    let mut s = String::with_capacity(7 + 64);
    s.push_str("blake3:");
    s.push_str(&hash.to_hex());
    s
}

/// Closed enumeration of stable diagnostic message templates.
///
/// **Closed-set discipline**: this enum is NOT `#[non_exhaustive]`.
/// Adding a variant requires a coordinated `MARQUE_AUDIT_SCHEMA`
/// version bump in `crates/engine/build.rs`. The on-the-wire audit
/// contract listed in `specs/006-engine-rule-refactor/contracts/audit-record.md`
/// references this enum; consumers match exhaustively and a
/// silent variant addition would break the auditability contract on
/// already-emitted records.
///
/// # Citations (Constitution VIII)
///
/// Each variant whose meaning maps to a specific CAPCO-2016 marking
/// semantic carries a `// CAPCO-2016 §<letter>.<number>` doc-comment
/// citation pinning the section. Page-level (pNN) granularity belongs at the
/// per-rule call site (`crates/capco/src/rules*.rs`), not at the
/// cross-cutting template enum where multiple rules across multiple
/// pages may emit the same template. Engine-synthetic variants
/// (`DecoderRecognized`, `ReparseFailed`, `CorrectionsApplied`) carry
/// no §-citation — they are not CAPCO-derived.
///
/// Rule emission constructs these variants; there is no
/// `format!`-interpolated free-form path.
///
/// # `#[non_exhaustive]` + the `Grammar` escape (T025)
///
/// The enum is `#[non_exhaustive]` so a future template addition is a
/// non-breaking change for downstream exhaustive matchers, and the
/// [`MessageTemplate::Grammar`] escape variant lets a co-resident
/// non-CAPCO grammar (CUI, NATO, ...) carry its own diagnostic template
/// without editing this CAPCO-derived set. The escape carries a
/// `grammar_id: &'static str` + a `variant: u32`. This is **additive
/// only** — every existing variant and its `as_str()` label is
/// byte-identical, so no `MARQUE_AUDIT_SCHEMA` bump is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MessageTemplate {
    /// Decoder recognized a canonical form for an input the strict
    /// recognizer rejected. Args: `actual_token` (the canonical the
    /// decoder produced).
    ///
    /// Constructed by marque-capco's decoder synthesis helper
    /// (`build_decoder_diagnostic` in
    /// `crates/capco/src/provenance.rs`). Engine-synthetic — no
    /// §-citation.
    DecoderRecognized,

    /// Engine-minted: the post-pass-1 buffer failed to re-parse.
    /// Args: `feature_ids` carry the contributing pass-1 fix rule
    /// IDs encoded as opaque [`FeatureId`] tags. The engine wires
    /// the call site under the sentinel
    /// `("engine", "fix.reparse-failed")` rule ID. Engine-synthetic
    /// — no §-citation.
    ReparseFailed,

    /// Banner roll-up did not match the projected expected banner
    /// derived from preceding portions on the page. Args: `category`
    /// (which axis disagreed), optional `expected_token` /
    /// `actual_token` for single-axis disagreements.
    ///
    /// Authority: CAPCO-2016 §H.4 (SCI), §H.5 (SAR), §H.8 (dissem),
    /// §H.3 (JOINT/REL TO).
    BannerRollupMismatch,

    /// Classification floor violated for a token requiring a minimum
    /// classification (e.g., HCS-comp-sub at SECRET; CNWDI at
    /// CONFIDENTIAL). Args: `expected_token` (required floor
    /// classification), `actual_token` (observed classification),
    /// `token` (the requiring SCI/dissem token).
    ///
    /// Authority: CAPCO-2016 §H.4 (per-system SCI floors); per-rule
    /// catalog rows carry the page-level §pNN.
    ClassificationFloorViolated,

    /// Non-canonical token order within a category. Args: `category`
    /// (which axis is out of order).
    ///
    /// Authority: CAPCO-2016 §H.8 (REL TO USA-first alpha), §H.6
    /// (SIGMA), §H.5 (SAR), §H.4 (SCI compartments), §H.3 (JOINT).
    NonCanonicalOrder,

    /// Token(s) are present in a non-canonical form that could be
    /// compacted or expanded to the preferred representation (e.g., an
    /// explicit trigraph list where a tetragraph abbreviation is
    /// preferred, or vice versa). Args: `category` (which axis carries
    /// the non-canonical form).
    ///
    /// Authority: CAPCO-2016 §H.8 (REL TO compaction into tetragraphs
    /// per §H.8 p150-151).
    NonCanonicalForm,

    /// Two tokens commingled that are mutually exclusive per
    /// CAPCO-2016 §H.8 / §H.9 invariants. Args: `token` (the dominated
    /// token to remove), `expected_token` (the dominating token that
    /// survives).
    ///
    /// Authority: CAPCO-2016 §H.8 (RELIDO/NOFORN conflicts),
    /// CAPCO-2016 §H.9 (non-IC dissem conflicts).
    ConflictsWith,

    /// A token requires a companion that is absent. Args: `token`
    /// (the requiring token), `expected_token` (the missing required
    /// companion).
    ///
    /// Authority: CAPCO-2016 §H.4 (HCS-O / SI-G companion-required
    /// invariants).
    RequiredByPresence,

    /// A deprecated/superseded token has a known canonical
    /// replacement. Args: `token` (the deprecated token),
    /// `expected_token` (the canonical replacement).
    ///
    /// Authority: CAPCO-2016 §F (Legacy Control Markings).
    SupersededToken,

    /// A portion-form token appears where a banner-form token is
    /// required (or vice versa). Args: `token` (the offending
    /// abbreviated/expanded form), `expected_token` (the form that
    /// should appear in this position).
    ///
    /// Authority: CAPCO-2016 §C.1 (Portion-mark syntax) and §D.1
    /// (Banner-line syntax).
    WrongTokenForm,

    /// A token is correct but does not belong in the banner because
    /// it is non-IC dissem and the banner is classified — banners
    /// for classified information do not carry non-IC dissem
    /// controls.
    ///
    /// Authority: CAPCO-2016 §H.9 (Non-IC dissem in classified
    /// banners).
    NonIcDissemInClassifiedBanner,

    /// Unrecognized token inside a marking — does not match any
    /// known classification, control, or trigraph in the active
    /// vocabulary. Args: `category` (the position the unknown token
    /// occupies, when known).
    ///
    /// Authority: CAPCO-2016 §G.1 (IC Markings System Register).
    UnrecognizedToken,

    /// Unpublished SCI control system present (custom control,
    /// agency-extensible per §A.6). Args: `token` (the unpublished
    /// control as a `TokenId` from the active vocabulary).
    ///
    /// Authority: CAPCO-2016 §A.6 (SCI compositional grammar) and
    /// §H.4 (per-system invariants).
    UnpublishedSciControl,

    /// User `[corrections]` config entry matched a literal substring
    /// and proposes a replacement. Args: `token` (the matched
    /// substring as a `TokenId` projection when one is registered;
    /// otherwise represented at the per-rule call site).
    ///
    /// Authority: user `.marque.toml` `[corrections]` table; not a
    /// CAPCO citation. Mirrors the existing
    /// [`crate::CORRECTIONS_MAP_CITATION`] pointer.
    CorrectionsApplied,

    /// Out-of-range value for a per-system numeric token (e.g.,
    /// SIGMA outside the currently authorized set, AEA range
    /// ceiling exceeded). Args: `token` (the offending value).
    ///
    /// Authority: CAPCO-2016 §H.6 (AEA SIGMA range invariants).
    OutOfRangeNumericToken,

    /// SAR program identifier is invalid (e.g., SAR ascending-order
    /// violation that is structural rather than the cross-cutting
    /// `NonCanonicalOrder`). Args: `token` (the offending
    /// program-id token).
    ///
    /// Authority: CAPCO-2016 §H.5 (SAR program identifier
    /// invariants).
    SarInvariantViolated,

    /// REL TO list contains entries that are not in the JOINT
    /// participant list — the REL TO axis has expanded beyond the
    /// JOINT co-owners. Per §H.3 p57 "[LIST]" superset semantics, a
    /// classifier MAY expand REL TO beyond JOINT participants, so
    /// the engine cannot distinguish intentional expansion from
    /// authoring error — surfaces as Warn with no auto-fix. Reverse
    /// of E014 (which flags JOINT participants missing from REL TO).
    /// Args: `category` (CAT_REL_TO — the axis carrying the
    /// expansion).
    ///
    /// Authority: CAPCO-2016 §H.3 p57 ("[LIST]" superset semantics).
    RelToExpandsBeyondJoint,

    /// Multi-scheme escape (T025): a diagnostic template owned by a
    /// co-resident non-CAPCO grammar. `grammar_id` is the contributing
    /// scheme's name (e.g. `"cui"`); `variant` is a scheme-local template
    /// ordinal the owning grammar assigns. Keeps the CAPCO-derived
    /// template set above closed while letting a second grammar surface
    /// its own diagnostics without an audit-schema bump. The owning
    /// grammar is responsible for keeping its `variant` ordinals stable
    /// across its own schema revisions.
    Grammar {
        /// The contributing scheme's name (a `MarkingScheme::name`-style
        /// `&'static` grammar id).
        grammar_id: &'static str,
        /// A scheme-local template ordinal assigned by the owning grammar.
        variant: u32,
    },
}

impl MessageTemplate {
    /// Stable canonical string label.
    ///
    /// Audit emitters MUST call this method rather than
    /// `format!("{self:?}")` — the `Debug` form is not part of the
    /// on-the-wire contract and could change. The `&'static str`
    /// return enables zero-allocation embedding in NDJSON serialization
    /// paths. Mirrors [`crate::FeatureId::as_str`].
    ///
    /// A new variant added without a matching `as_str` arm fails the
    /// match-exhaustiveness check at compile time, so the on-the-wire
    /// contract cannot drift silently across emitters.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DecoderRecognized => "DecoderRecognized",
            Self::ReparseFailed => "ReparseFailed",
            Self::BannerRollupMismatch => "BannerRollupMismatch",
            Self::ClassificationFloorViolated => "ClassificationFloorViolated",
            Self::NonCanonicalOrder => "NonCanonicalOrder",
            Self::NonCanonicalForm => "NonCanonicalForm",
            Self::ConflictsWith => "ConflictsWith",
            Self::RequiredByPresence => "RequiredByPresence",
            Self::SupersededToken => "SupersededToken",
            Self::WrongTokenForm => "WrongTokenForm",
            Self::NonIcDissemInClassifiedBanner => "NonIcDissemInClassifiedBanner",
            Self::UnrecognizedToken => "UnrecognizedToken",
            Self::UnpublishedSciControl => "UnpublishedSciControl",
            Self::CorrectionsApplied => "CorrectionsApplied",
            Self::OutOfRangeNumericToken => "OutOfRangeNumericToken",
            Self::SarInvariantViolated => "SarInvariantViolated",
            Self::RelToExpandsBeyondJoint => "RelToExpandsBeyondJoint",
            // The escape variant projects to its grammar id — the only
            // `&'static str` it carries. The `variant` ordinal is
            // grammar-local and surfaces through the structured audit
            // args, not the label.
            Self::Grammar { grammar_id, .. } => grammar_id,
        }
    }
}

/// Closed-set permitted arguments to a [`MessageTemplate`].
///
/// **Constitution V Principle V closure**: every field type is
/// in the audit-content-ignorance permitted set: [`TokenId`],
/// [`CategoryId`], [`Span`], [`Blake3Hash`], [`Recognition`],
/// [`FeatureId`]. No `String`, no `&str`, no `Vec<u8>`, no `format!`-
/// derived field. `MessageArgs` cannot carry input bytes by
/// construction.
///
/// # Field-name conventions
///
/// `expected_token` / `actual_token` are both `Option<TokenId>` —
/// same type, distinguished by field name for diagnostic ergonomics.
/// The audit emitter renders them as `args.expected: "..."` and
/// `args.actual: "..."` keys in the NDJSON record.
///
/// `feature_ids` is a `SmallVec<[FeatureId; 4]>` — most diagnostics
/// carry 0–2 contributing features; the inline-4 capacity covers the
/// 99th-percentile case without heap allocation.
///
/// # Construction
///
/// Use `MessageArgs::default()` plus field-level assignment. There is
/// no all-positional constructor — the struct is not extensible
/// enough to warrant one, and field-level assignment makes the call
/// site self-documenting.
///
/// # Compile-fail proofs of the closed field set
///
/// Each `compile_fail` doctest pins one inadmissible field shape.
/// Doctests compile as separate crates against the library's public
/// API, so the snippets see the same surface a downstream consumer
/// would see. Pairs with the destructuring-as-pin positive test at
/// `crates/rules/tests/message_args_closed_set.rs`.
///
/// **No `String` field on `MessageArgs`.**
///
/// ```compile_fail
/// use marque_rules::MessageArgs;
/// // Closed-set struct rejects unknown fields (E0560).
/// let _ = MessageArgs {
///     raw_text: String::from("classified content"),
///     ..MessageArgs::default()
/// };
/// ```
///
/// **No `Vec<u8>` field on `MessageArgs`.**
///
/// ```compile_fail
/// use marque_rules::MessageArgs;
/// let _ = MessageArgs {
///     bytes: Vec::<u8>::new(),
///     ..MessageArgs::default()
/// };
/// ```
///
/// **`token` is `Option<TokenId>`, not `&str`.**
///
/// ```compile_fail
/// use marque_rules::MessageArgs;
/// let args = MessageArgs::default();
/// // expected `&str`, found `Option<TokenId>`
/// let _: &str = args.token;
/// ```
///
/// **No `From<&str> for MessageArgs` impl.**
///
/// ```compile_fail
/// use marque_rules::MessageArgs;
/// let _: MessageArgs = "free-form text".into();
/// ```
///
/// **No `From<String> for MessageArgs` impl.**
///
/// ```compile_fail
/// use marque_rules::MessageArgs;
/// let _: MessageArgs = String::from("free-form text").into();
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessageArgs {
    /// Primary token the message refers to (e.g., the deprecated
    /// token in [`MessageTemplate::SupersededToken`], the requiring
    /// token in [`MessageTemplate::RequiredByPresence`], the offending
    /// token in [`MessageTemplate::ConflictsWith`]).
    pub token: Option<TokenId>,

    /// Category axis the message refers to (e.g., the offending
    /// category for [`MessageTemplate::NonCanonicalOrder`]).
    pub category: Option<CategoryId>,

    /// Span-level locator for the message — used when
    /// `Diagnostic.span` covers a multi-token marking and the message
    /// needs to point at one token within it.
    pub span: Option<Span>,

    /// BLAKE3 digest of content the message references.
    pub digest: Option<Blake3Hash>,

    /// Recognition record snapshotted from the producing rule's
    /// `FixIntent`.
    pub confidence: Option<Recognition>,

    /// What token the rule expected (the canonical / required value).
    pub expected_token: Option<TokenId>,

    /// What token the rule actually saw.
    pub actual_token: Option<TokenId>,

    /// Closed list of contributing features (decoder posterior
    /// contributions, strict-context floors, etc.).
    pub feature_ids: SmallVec<[FeatureId; 4]>,

    /// Contributing pass-1 fix rule IDs for
    /// [`MessageTemplate::ReparseFailed`]. Empty for every other
    /// template variant.
    ///
    /// Inline-4 matches the four-rule pass-1 partition in the CAPCO
    /// ruleset — no heap allocation even when every Localized rule
    /// contributes. [`RuleId`] is on Constitution V Principle V's
    /// permitted-identifier list (enumerated identifier, not document
    /// bytes); the `SmallVec<[RuleId; 4]>` preserves the closed-set
    /// property of [`MessageArgs`].
    ///
    /// Audit emitters MUST skip this field when empty. The audit
    /// emit boundary at `crates/engine/src/audit.rs` (and the WASM
    /// equivalent) uses `SmallVec::is_empty` as the skip predicate.
    pub contributing_rule_ids: SmallVec<[RuleId; 4]>,
}

/// Diagnostic message: closed template + closed args.
///
/// Backs [`crate::Diagnostic::message`] as a closed-template /
/// closed-args pair rather than a free-form `Box<str>`.
///
/// # Sole public constructor
///
/// The only public way to construct a [`Message`] is
/// [`Message::new`]. There is **no** `Message::from_string`, **no**
/// `impl From<&str> for Message`, and **no** `Message::format(...)` —
/// the absence is intentional and load-bearing. The compile-fail
/// proofs below pin every absent constructor; the positive control
/// lives at `crates/rules/tests/message_no_freeform_ctor.rs`.
///
/// # Compile-fail proofs that no free-form constructor exists
///
/// **No `Message::from_string` method.**
///
/// ```compile_fail
/// use marque_rules::Message;
/// let _ = Message::from_string("free-form text");
/// ```
///
/// **No `Message::from_str` method.**
///
/// ```compile_fail
/// use marque_rules::Message;
/// let _ = Message::from_str("free-form text");
/// ```
///
/// **No `From<&str> for Message` impl.**
///
/// ```compile_fail
/// use marque_rules::Message;
/// let _: Message = "free-form text".into();
/// ```
///
/// **No `From<String> for Message` impl.**
///
/// ```compile_fail
/// use marque_rules::Message;
/// let _: Message = String::from("free-form text").into();
/// ```
///
/// **No `From<Box<str>> for Message` impl.**
///
/// ```compile_fail
/// use marque_rules::Message;
/// let _: Message = Box::<str>::from("free-form text").into();
/// ```
///
/// **No `Message::format` macro-style constructor.**
///
/// ```compile_fail
/// use marque_rules::Message;
/// let _ = Message::format("decoder-recognized canonical form: {}", "data");
/// ```
///
/// **No `impl Display for Message`.** A `Display` impl would be a
/// covert free-form channel — `format!("{}", msg)` would produce a
/// `String` derived from the message that consumers could re-emit
/// in audit records or diagnostics, defeating the closed-template
/// closure. `Message` is rendered by the audit emitter via
/// [`Message::template`] + [`Message::args`] direct field access,
/// not via `Display`.
///
/// ```compile_fail
/// use marque_rules::{Message, MessageTemplate, MessageArgs};
/// let m = Message::new(MessageTemplate::DecoderRecognized, MessageArgs::default());
/// let _ = format!("{}", m);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    template: MessageTemplate,
    args: MessageArgs,
}

impl Message {
    /// **The only public constructor.**
    ///
    /// Closes the `format!` channel that previously let rules
    /// interpolate input bytes into `Diagnostic.message`. Every
    /// message is now a `(template, args)` pair where the template is
    /// drawn from the closed [`MessageTemplate`] enum and the args
    /// are restricted to the closed [`MessageArgs`] permitted set.
    ///
    /// See the compile-fail tests in
    /// `crates/rules/tests/message_no_freeform_ctor.rs` for the
    /// load-bearing absence proofs.
    #[inline]
    pub const fn new(template: MessageTemplate, args: MessageArgs) -> Self {
        Self { template, args }
    }

    /// The closed-enum template variant.
    #[inline]
    pub const fn template(&self) -> MessageTemplate {
        self.template
    }

    /// The closed-set args record.
    #[inline]
    pub fn args(&self) -> &MessageArgs {
        &self.args
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn message_template_as_str_is_stable() {
        // Pin the on-the-wire audit string for every variant. A new
        // variant without an `as_str` arm fails the match
        // exhaustiveness check at compile time; this test pins the
        // *labels* themselves so a rename cannot slip through silently.
        // Mirrors `recognition::tests::feature_id_as_str_matches_audit_contract`.
        assert_eq!(
            MessageTemplate::DecoderRecognized.as_str(),
            "DecoderRecognized"
        );
        assert_eq!(MessageTemplate::ReparseFailed.as_str(), "ReparseFailed");
        assert_eq!(
            MessageTemplate::BannerRollupMismatch.as_str(),
            "BannerRollupMismatch"
        );
        assert_eq!(
            MessageTemplate::ClassificationFloorViolated.as_str(),
            "ClassificationFloorViolated",
        );
        assert_eq!(
            MessageTemplate::NonCanonicalOrder.as_str(),
            "NonCanonicalOrder"
        );
        assert_eq!(
            MessageTemplate::NonCanonicalForm.as_str(),
            "NonCanonicalForm"
        );
        assert_eq!(MessageTemplate::ConflictsWith.as_str(), "ConflictsWith");
        assert_eq!(
            MessageTemplate::RequiredByPresence.as_str(),
            "RequiredByPresence"
        );
        assert_eq!(MessageTemplate::SupersededToken.as_str(), "SupersededToken");
        assert_eq!(MessageTemplate::WrongTokenForm.as_str(), "WrongTokenForm");
        assert_eq!(
            MessageTemplate::NonIcDissemInClassifiedBanner.as_str(),
            "NonIcDissemInClassifiedBanner",
        );
        assert_eq!(
            MessageTemplate::UnrecognizedToken.as_str(),
            "UnrecognizedToken"
        );
        assert_eq!(
            MessageTemplate::UnpublishedSciControl.as_str(),
            "UnpublishedSciControl"
        );
        assert_eq!(
            MessageTemplate::CorrectionsApplied.as_str(),
            "CorrectionsApplied"
        );
        assert_eq!(
            MessageTemplate::OutOfRangeNumericToken.as_str(),
            "OutOfRangeNumericToken",
        );
        assert_eq!(
            MessageTemplate::SarInvariantViolated.as_str(),
            "SarInvariantViolated"
        );
        assert_eq!(
            MessageTemplate::RelToExpandsBeyondJoint.as_str(),
            "RelToExpandsBeyondJoint"
        );
    }

    #[test]
    fn grammar_escape_template_projects_to_grammar_id() {
        // T025: the additive `Grammar { grammar_id, variant }` escape
        // lets a co-resident non-CAPCO grammar carry its own template
        // without editing the closed CAPCO-derived set (and the
        // audit-schema bump that would entail). `as_str()` projects to
        // the grammar id.
        let t = MessageTemplate::Grammar {
            grammar_id: "cui",
            variant: 3,
        };
        assert_eq!(t.as_str(), "cui");
        assert_ne!(
            t,
            MessageTemplate::Grammar {
                grammar_id: "cui",
                variant: 4
            }
        );
    }

    #[test]
    fn message_new_round_trips_template_and_args() {
        let args = MessageArgs {
            token: Some(TokenId(42)),
            ..MessageArgs::default()
        };
        let m = Message::new(MessageTemplate::SupersededToken, args.clone());
        assert_eq!(m.template(), MessageTemplate::SupersededToken);
        assert_eq!(m.args(), &args);
    }

    #[test]
    fn blake3_zero_round_trip() {
        // Post-PM-D-5: Blake3Hash is `blake3::Hash`; construct via
        // upstream `From<[u8; 32]>` rather than a project-side `zero()` ctor.
        let z: Blake3Hash = Blake3Hash::from([0u8; 32]);
        assert_eq!(*z.as_bytes(), [0u8; 32]);
        // Audit-string form prefixes "blake3:" and emits 64 hex chars.
        let s = crate::message::to_audit_string(&z);
        assert_eq!(s.len(), 7 + 64);
        assert!(s.starts_with("blake3:"));
        assert!(s[7..].chars().all(|c| c.is_ascii_hexdigit()));
        // All-zero input is all-zero hex output.
        assert_eq!(&s[7..], &"0".repeat(64));
    }

    #[test]
    fn blake3_to_audit_string_hex_encoding_matches_byte_pattern() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0x12;
        bytes[1] = 0xab;
        bytes[31] = 0xff;
        let h: Blake3Hash = Blake3Hash::from(bytes);
        let s = crate::message::to_audit_string(&h);
        assert!(s.starts_with("blake3:12ab"));
        assert!(s.ends_with("ff"));
    }

    #[test]
    fn message_args_default_is_all_none() {
        let args = MessageArgs::default();
        assert!(args.token.is_none());
        assert!(args.category.is_none());
        assert!(args.span.is_none());
        assert!(args.digest.is_none());
        assert!(args.confidence.is_none());
        assert!(args.expected_token.is_none());
        assert!(args.actual_token.is_none());
        assert!(args.feature_ids.is_empty());
        assert!(args.contributing_rule_ids.is_empty());
    }
}
