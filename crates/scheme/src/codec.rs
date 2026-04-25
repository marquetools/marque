// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Grammar serialization — pinned trait surface.
//!
//! A [`Codec`] round-trips a scheme's `Marking` to bytes for transport
//! (XML, JSON, CBOR, ...). Phase E publishes the trait; Phase G lands
//! concrete XML and JSON impls without further trait evolution (FR-019,
//! SC-010).
//!
//! No concrete impls ship in Phase E. The shape exists so downstream
//! work can target a stable surface.
//!
//! # Ambiguity preservation
//!
//! [`Codec::decode`] returns [`Parsed<S::Marking>`](Parsed) rather than
//! a bare `S::Marking`. That keeps the codec layer honest about the
//! same ambiguity that the parser surfaces: a serialized marking that
//! contains a genuinely ambiguous production (the `(C)` case is the
//! canonical example) decodes as `Parsed::Ambiguous` so the engine's
//! resolver can run even on pre-serialized input. See
//! foundational-plan §9.

use crate::ambiguity::Parsed;
use crate::scheme::MarkingScheme;

/// Round-trip codec for a [`MarkingScheme`].
///
/// Implementations MUST be `Send + Sync` so the engine can hold them
/// in an `Arc<dyn Codec<S>>` and dispatch across threads. `BatchEngine`
/// drives `Engine` work onto `tokio::task::spawn_blocking` worker
/// threads — a `!Send` codec could not be held in that
/// `Arc<dyn Codec<S>>` or moved/shared into blocking workers, so the
/// engine would fail to compile rather than degrading to serialized
/// single-worker batch processing. Pinning the bound on the trait
/// surface here means Phase G implementers see the constraint at the
/// definition site instead of discovering it through a downstream
/// `Send`/`Sync` compile error. Mirrors the bound on
/// [`crate::recognizer::Recognizer`].
pub trait Codec<S: MarkingScheme + ?Sized>: Send + Sync {
    /// Serialize `marking` to bytes. Returns the encoded form or a
    /// [`CodecError`] if the marking cannot be rendered in this codec
    /// (e.g., a scheme-specific construct the encoder does not
    /// support).
    fn encode(&self, marking: &S::Marking) -> Result<Vec<u8>, CodecError>;

    /// Parse `bytes` into a [`Parsed<S::Marking>`]. Returns
    /// [`Parsed::Ambiguous`] at enumerated decision points (e.g., the
    /// CAPCO `(C)` case) — never `Unambiguous` with a sentinel. The
    /// zero-candidate form (`Parsed::Ambiguous { candidates: vec![] }`)
    /// signals "input decoded cleanly but no plausible marking was
    /// recognized".
    fn decode(&self, bytes: &[u8]) -> Result<Parsed<S::Marking>, CodecError>;
}

/// Errors surfaced by [`Codec::encode`] and [`Codec::decode`].
///
/// # Content-ignorance contract (Constitution V G13)
///
/// Implementations MUST NOT embed document content (parsed bytes,
/// classified text, marking values, free-form prose from the input)
/// into [`Self::Malformed`]'s message string or
/// [`Self::SchemaMismatch::observed`]. Both fields end up in
/// `tracing` logs, server error responses, and CLI stderr — every
/// one of which is an audit-adjacent stream. A `Malformed` error
/// constructed as `Malformed(format!("unexpected token at offset
/// {N}: {bytes}"))` leaks the bytes; correct construction is
/// `Malformed(format!("unexpected token at offset {N}"))` —
/// position only, no content.
///
/// Permitted in the message: byte offsets, line/column numbers,
/// token-class names (`"<Element>"`, `"<Attribute>"`), enumerated
/// failure-mode labels, schema-version strings (which are
/// vocabulary, not content). Forbidden: any substring of the input
/// that originated outside the codec's own const tables.
///
/// The G13 invariant is corpus-tested at the engine layer
/// (`crates/engine/tests/audit.rs::audit_stream_no_content_leak`)
/// for `AppliedFix` and `Diagnostic`; codec error messages are
/// implementation territory and rely on this contract being
/// observed at construction sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The input bytes were structurally malformed (XML not well-
    /// formed, JSON parse failure, etc.). Carries an implementation-
    /// defined message.
    ///
    /// **G13 (see type-level docs):** `String` MUST NOT contain any
    /// substring of the input bytes — position and class only.
    Malformed(String),
    /// The codec does not implement this serialization format.
    /// Returned when a caller passes bytes in a format the codec was
    /// not built to handle.
    UnsupportedFormat(&'static str),
    /// The input decoded structurally but referenced a schema version
    /// the codec does not support. Carries `(expected, observed)`.
    ///
    /// **G13 (see type-level docs):** `observed` MUST be the schema-
    /// version identifier read from a known-safe location in the
    /// decoded structure (e.g., a `version=` attribute), not raw
    /// input bytes. A schema version is vocabulary; arbitrary input
    /// is not.
    SchemaMismatch {
        expected: &'static str,
        observed: String,
    },
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(msg) => write!(f, "malformed input: {msg}"),
            Self::UnsupportedFormat(fmt) => write!(f, "unsupported codec format: {fmt}"),
            Self::SchemaMismatch { expected, observed } => write!(
                f,
                "schema mismatch: expected {expected:?}, observed {observed:?}"
            ),
        }
    }
}

impl std::error::Error for CodecError {}
