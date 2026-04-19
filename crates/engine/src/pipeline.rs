// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Async stream pipeline types.
//!
//! The full pipeline:
//!   Source → TextStream → SpanStream → AttributeStream → DiagnosticStream → Sink
//!
//! Each stage is a `Stream`. Middleware inserts between stages.
//! This module defines the stage types; full async streaming implementation is TODO.

use marque_ism::MarkingCandidate;
use marque_rules::Diagnostic;

/// A chunk of source text with its byte offset in the original document.
#[derive(Debug)]
pub struct TextChunk {
    pub offset: usize,
    pub data: Vec<u8>,
}

/// A stream source — anything that produces `TextChunk`s.
/// Implemented by: string buffer (WASM/server), file reader (CLI/batch), HTTP body.
pub trait Source: Send {
    // TODO: implement as futures::Stream<Item = Result<TextChunk, SourceError>>
}

/// A stream sink — anything that consumes pipeline output.
pub trait Sink: Send {
    fn accept_diagnostic(&mut self, diag: Diagnostic);
    fn accept_candidate(&mut self, candidate: MarkingCandidate);
}
