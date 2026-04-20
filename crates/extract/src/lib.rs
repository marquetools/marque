// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-extract — document text and metadata extraction.
//!
//! Wraps Kreuzberg (https://github.com/kreuzberg-dev/kreuzberg):
//! Rust-core, SIMD-optimized, streaming, 75+ formats, OCR for scanned documents.
//!
//! NOT included in the marque-wasm build. In WASM context, the calling application
//! is responsible for providing pre-extracted text to the engine.
//!
//! # Metadata
//! Metadata extraction runs in the same pipeline pass as text extraction.
//! Metadata issues are surfaced as `MetadataWarning` — always reported,
//! stripping is opt-in via `ExtractionOptions::strip_metadata`.

pub mod extractor;
pub mod metadata;

pub use extractor::{ExtractedDocument, ExtractionOptions, Extractor};
pub use metadata::{MetadataField, MetadataReport, MetadataWarning};
