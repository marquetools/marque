// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Metadata extraction and sanitization.
//!
//! Surfaces sensitive metadata that document authors are typically unaware of:
//! author identity, revision history, tracked changes, embedded image EXIF,
//! template source paths, software version strings, GPS coordinates.

use serde::{Deserialize, Serialize};

/// Complete metadata report for a document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataReport {
    pub fields: Vec<MetadataField>,
    pub warnings: Vec<MetadataWarning>,
}

impl MetadataReport {
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// A single extracted metadata field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataField {
    pub category: MetadataCategory,
    pub key: String,
    pub value: String,
}

/// Category of metadata field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataCategory {
    /// Document properties: author, company, title, subject, keywords.
    DocumentProperties,
    /// Revision history, tracked changes, comments with author attribution.
    RevisionHistory,
    /// EXIF data from embedded images (GPS, device, timestamp).
    ImageExif,
    /// XMP metadata embedded in the document.
    Xmp,
    /// Template or base document path — can reveal internal paths or systems.
    TemplateReference,
    /// Software and version strings (reveals toolchain).
    Software,
    /// Custom/application-defined properties.
    Custom,
}

/// A metadata warning — fields that may expose sensitive information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataWarning {
    pub field: MetadataField,
    pub severity: WarningSeverity,
    pub reason: String,
    /// Whether this field can be automatically stripped.
    pub strippable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WarningSeverity {
    /// Low-sensitivity metadata (title, keywords).
    Info,
    /// Potentially sensitive (author name, company, software version).
    Warn,
    /// High sensitivity — GPS coordinates, revision history with PII.
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_report_has_warnings_empty() {
        let report = MetadataReport::default();
        assert!(!report.has_warnings());
    }

    #[test]
    fn test_metadata_report_has_warnings_populated() {
        let warning = MetadataWarning {
            field: MetadataField {
                category: MetadataCategory::DocumentProperties,
                key: "Author".to_string(),
                value: "John Doe".to_string(),
            },
            severity: WarningSeverity::Warn,
            reason: "Reveals author identity".to_string(),
            strippable: true,
        };
        let report = MetadataReport {
            fields: vec![],
            warnings: vec![warning],
        };
        assert!(report.has_warnings());
    }
}
