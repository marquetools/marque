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
pub trait Codec<S: MarkingScheme + ?Sized> {
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The input bytes were structurally malformed (XML not well-
    /// formed, JSON parse failure, etc.). Carries an implementation-
    /// defined message.
    Malformed(String),
    /// The codec does not implement this serialization format.
    /// Returned when a caller passes bytes in a format the codec was
    /// not built to handle.
    UnsupportedFormat(&'static str),
    /// The input decoded structurally but referenced a schema version
    /// the codec does not support. Carries `(expected, observed)`.
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
