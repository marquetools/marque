// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Structural templates for marking positions (portion / banner / CAB).
//!
//! A `Template` records how categories compose into a complete marking
//! in a specific position. The parser uses templates to disambiguate
//! position-dependent token semantics: a trigraph means different
//! things depending on whether it appears as a non-US classification
//! prefix, a REL TO target, or an FGI source indicator.

use crate::category::CategoryId;

/// Where tokens wrap at a structural boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wrapping {
    /// `(S//NF)` — portions.
    Parenthesized,
    /// `SECRET//NOFORN` — banners.
    None,
    /// `[S//NF]` — some non-CAPCO formats.
    Bracketed,
    /// Arbitrary open/close delimiters.
    Custom {
        open: &'static str,
        close: &'static str,
    },
}

/// Whether the template uses abbreviated or expanded token forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenForm {
    /// Portions: `S`, `NF`, `TS`.
    Abbreviated,
    /// Banners: `SECRET`, `NOFORN`, `TOP SECRET`.
    Expanded,
    /// Either form is accepted; don't normalize.
    AsWritten,
}

/// Whether a category is required, optional, or forbidden in a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Required,
    Optional,
    Forbidden,
}

/// One category's role in a template.
#[derive(Debug, Clone)]
pub struct CategoryRule {
    pub category: CategoryId,
    pub presence: Presence,
}

/// A structural template — what a valid marking looks like in a
/// specific position.
#[derive(Debug, Clone)]
pub struct Template {
    pub name: &'static str,
    /// Default delimiter between categories (e.g., `//` for CAPCO).
    pub category_delimiter: &'static str,
    pub wrapping: Wrapping,
    pub token_form: TokenForm,
    pub category_rules: Vec<CategoryRule>,
}
