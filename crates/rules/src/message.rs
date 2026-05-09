// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Message` — closed-template, closed-args diagnostic message representation.
//!
//! Replaces the current `Diagnostic.message: Box<str>` channel (which
//! permits `format!`-built strings interpolating input bytes — the leak
//! channel called out by the source plan
//! `docs/plans/2026-05-02-engine-refactor-consolidated.md` §8.3 and
//! Constitution V Principle V's audit-content-ignorance invariant).
//!
//! PR 3c.1 (this PR) ships the new types **alongside** the existing
//! `Box<str>` channel. PR 3c.2 migrates `Diagnostic.message` to
//! `Message` and deletes the `format!` interpolation at
//! `crates/engine/src/engine.rs:1462` (`format!("decoder-recognized
//! canonical form: {replacement:?}")`).
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
//! [`marque_scheme::CategoryId`], [`marque_ism::Span`], [`Blake3Hash`],
//! [`crate::Confidence`], [`crate::FeatureId`]. Two convenience-typed
//! `Option<TokenId>` fields (`expected_token` / `actual_token`) are
//! permitted because they carry the same type as `token` — the field
//! name is the only distinction, for diagnostic-rendering ergonomics.
//!
//! No `String`, no `&str`, no `Vec<u8>`, no `format!`-derived field is
//! permitted. The closure is enforced by:
//!
//! 1. **Compile-fail tests** at
//!    `crates/rules/tests/message_args_closed_set.rs` — each tests
//!    that an inadmissible field shape (a `String`-typed `raw_text`
//!    field, a `From<&str>` impl, etc.) does not compile.
//! 2. **A positive destructuring-as-pin test** that exhaustively
//!    destructures every permitted field, so adding a field without
//!    updating the test set breaks the build (E0027 — pattern does
//!    not mention all fields).

use marque_ism::Span;
use marque_scheme::{CategoryId, TokenId};
use smallvec::SmallVec;

use crate::confidence::{Confidence, FeatureId};

/// BLAKE3 digest, 32 bytes, fixed-size array.
///
/// Held inline (not `Box<[u8]>` or `String`) to enforce content-
/// ignorance — the digest is computed *from* content, but the digest
/// itself is opaque. Consumed by the `AppliedFix.fix.original_digest`
/// field at PR 3c.2's audit-record reshape (T041 / FR-002 / FR-004).
///
/// # Content-ignorance invariant (Constitution V Principle V / G13)
///
/// `Blake3Hash` is one of the closed permitted types for
/// [`MessageArgs`] fields. The hex string returned by
/// [`Blake3Hash::to_audit_string`] is the only audit-emit boundary
/// where the digest leaves the inline byte representation; that
/// boundary lives in the engine's audit emitter.
///
/// PR 3c.1 ships the type with a placeholder all-zero `[0; 32]`
/// constructor pattern; PR 3c.2 wires real `blake3::hash` computation
/// at the audit-emit boundary alongside the `AppliedFix` reshape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Blake3Hash(pub [u8; 32]);

impl Blake3Hash {
    /// All-zero sentinel value. PR 3c.1 placeholder; PR 3c.2 replaces
    /// every construction site with `blake3::hash(...)`-derived values.
    #[inline]
    pub const fn zero() -> Self {
        Self([0u8; 32])
    }

    /// Render as a `"blake3:..."` hex string for NDJSON serialization.
    /// Returns owned `String` only at the audit-emit boundary.
    pub fn to_audit_string(&self) -> String {
        let mut s = String::with_capacity(7 + 64);
        s.push_str("blake3:");
        for byte in &self.0 {
            // Hex-encode each byte. Manual encode keeps zero non-WASM
            // dependencies; this method runs only at the audit-emit
            // boundary, not on the hot path.
            const HEX: &[u8; 16] = b"0123456789abcdef";
            s.push(HEX[(byte >> 4) as usize] as char);
            s.push(HEX[(byte & 0x0f) as usize] as char);
        }
        s
    }
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
/// PR 3c.1 ships the enum **alongside** the existing `Box<str>`
/// channel. PR 3c.2 migrates rule emission to construct these
/// variants and deletes the `format!`-interpolated free-form path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageTemplate {
    /// Decoder recognized a canonical form for an input the strict
    /// recognizer rejected. Args: `actual_token` (the canonical the
    /// decoder produced).
    ///
    /// **Lands**: PR 3c.2 (replaces the `format!` at
    /// `crates/engine/src/engine.rs:1462`). Engine-synthetic — no
    /// §-citation.
    DecoderRecognized,

    /// Engine-minted: the post-pass-1 buffer failed to re-parse.
    /// Args: `feature_ids` carry the contributing pass-1 fix rule
    /// IDs encoded as opaque [`FeatureId`] tags (PR 7 fills the
    /// variant set).
    ///
    /// **Lands**: PR 3c.1 reserves the slot; PR 7 wires the call
    /// site under the sentinel `("engine", "r002.reparse-failed")`
    /// rule ID per source plan §9.4. Engine-synthetic — no §-citation.
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
        }
    }
}

/// Closed-set permitted arguments to a [`MessageTemplate`].
///
/// **Constitution V Principle V (G13) closure**: every field type is
/// in the audit-content-ignorance permitted set: [`TokenId`],
/// [`CategoryId`], [`Span`], [`Blake3Hash`], [`Confidence`],
/// [`FeatureId`]. No `String`, no `&str`, no `Vec<u8>`, no `format!`-
/// derived field. This is the type-level closure of the leak channel
/// called out by source plan §8.3 — `MessageArgs` cannot carry input
/// bytes by construction.
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

    /// BLAKE3 digest of content the message references. PR 3c.1
    /// ships the `Blake3Hash` type; PR 3c.2 wires real digest
    /// computation at the audit-emit boundary.
    pub digest: Option<Blake3Hash>,

    /// Confidence record snapshotted from the producing rule's
    /// [`crate::FixProposal`] (PR 3c.1) or
    /// `FixIntent` (PR 3c.2 onward).
    pub confidence: Option<Confidence>,

    /// What token the rule expected (the canonical / required value).
    pub expected_token: Option<TokenId>,

    /// What token the rule actually saw.
    pub actual_token: Option<TokenId>,

    /// Closed list of contributing features (decoder posterior
    /// contributions, strict-context floors, etc.).
    pub feature_ids: SmallVec<[FeatureId; 4]>,
}

/// Diagnostic message: closed template + closed args.
///
/// Replaces [`crate::Diagnostic::message`]'s `Box<str>` field at
/// PR 3c.2. PR 3c.1 ships the new type alongside the existing
/// `Box<str>` field; PR 3c.2 migrates the `Diagnostic` field type and
/// deletes the legacy channel.
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
        // Mirrors `confidence::tests::feature_id_as_str_matches_audit_contract`.
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
        let z = Blake3Hash::zero();
        assert_eq!(z.0, [0u8; 32]);
        // Audit-string form prefixes "blake3:" and emits 64 hex chars.
        let s = z.to_audit_string();
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
        let h = Blake3Hash(bytes);
        let s = h.to_audit_string();
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
    }
}
