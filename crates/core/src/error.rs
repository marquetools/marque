// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use marque_scheme::Span;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("malformed marking: {0:?}")]
    MalformedMarking(String),

    #[error("unrecognized token at offset {offset}: {token:?}")]
    UnrecognizedToken { token: String, offset: usize },

    #[error("invalid UTF-8 in span {0:?}")]
    InvalidUtf8(Span),

    #[error("empty source buffer")]
    EmptySource,
}
