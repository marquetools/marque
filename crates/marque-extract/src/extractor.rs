//! Document text extraction with streaming support.
//!
//! TODO: wire Kreuzberg once crate dependency is confirmed.
//! Current implementation is a stub that reads raw text files only.

use std::path::Path;
use thiserror::Error;
use crate::metadata::MetadataReport;

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Options controlling extraction behavior.
#[derive(Debug, Clone, Default)]
pub struct ExtractionOptions {
    /// Extract and report document metadata.
    pub extract_metadata: bool,
    /// Remove metadata from the output document (creates a sanitized copy).
    pub strip_metadata: bool,
    /// Attempt OCR on image-based pages (requires OCR backend).
    pub ocr: bool,
}

/// Result of document extraction.
#[derive(Debug)]
pub struct ExtractedDocument {
    /// Extracted text content, UTF-8.
    pub text: Vec<u8>,
    /// Metadata report (populated if `extract_metadata` was set).
    pub metadata: Option<MetadataReport>,
    /// Original format detected.
    pub format: DetectedFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedFormat {
    PlainText,
    Docx,
    Pdf,
    Html,
    Xlsx,
    Pptx,
    Email,
    Unknown(String),
}

/// Stateless document extractor.
pub struct Extractor;

impl Extractor {
    /// Extract text (and optionally metadata) from a file.
    pub async fn extract(
        path: &Path,
        opts: ExtractionOptions,
    ) -> Result<ExtractedDocument, ExtractError> {
        // TODO: delegate to Kreuzberg for full format support.
        // Stub: read raw bytes and return as-is for plain text.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "txt" | "text" => {
                let text = tokio::fs::read(path).await?;
                Ok(ExtractedDocument {
                    text,
                    metadata: None,
                    format: DetectedFormat::PlainText,
                })
            }
            "docx" => Err(ExtractError::UnsupportedFormat(
                "docx extraction requires Kreuzberg integration (TODO)".into(),
            )),
            "pdf" => Err(ExtractError::UnsupportedFormat(
                "pdf extraction requires Kreuzberg integration (TODO)".into(),
            )),
            other => Err(ExtractError::UnsupportedFormat(other.to_owned())),
        }
    }

    /// Extract from an in-memory buffer with an explicit format hint.
    pub fn extract_bytes(
        data: &[u8],
        format: DetectedFormat,
        _opts: ExtractionOptions,
    ) -> Result<ExtractedDocument, ExtractError> {
        match format {
            DetectedFormat::PlainText => Ok(ExtractedDocument {
                text: data.to_vec(),
                metadata: None,
                format: DetectedFormat::PlainText,
            }),
            _ => Err(ExtractError::UnsupportedFormat(
                "non-text extraction requires Kreuzberg integration (TODO)".into(),
            )),
        }
    }
}
