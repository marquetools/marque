// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `promote-callsite-lint` — AST-based CI lint enforcing FR-040 of
//! the marque engine refactor (spec `006-engine-rule-refactor`).
//!
//! Two independent passes:
//!
//! - [`callsite`] — flag any call to `AppliedFix::__engine_promote`
//!   or `EnginePromotionToken::__engine_construct` whose origin is
//!   not the engine's promotion gate (production code) or a
//!   correctly-marked test fixture (Constitution V Principle V
//!   carve-out). Diagnostic codes `PRC001` / `PRC002`.
//! - [`signature`] — flag any function whose signature shape is
//!   `fn(...ParsedAttrs<'_>...) -> CanonicalAttrs` (or
//!   `Result<CanonicalAttrs, _>`) outside the three whitelisted
//!   sites: `unsafe fn`, `MarkingScheme::canonicalize`, and the
//!   transitional `from_parsed_unchecked` adapter. Diagnostic
//!   code `PRC100`.
//!
//! Both passes share the [`enclosing`] utility for resolving the
//! enclosing function of an arbitrary span, and the [`diagnostic`]
//! module for the rustc-style finding type.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]

pub mod callsite;
pub mod diagnostic;
pub mod enclosing;
pub mod signature;

pub use diagnostic::{Diagnostic, Severity};
