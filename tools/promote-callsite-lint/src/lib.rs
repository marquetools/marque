// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `promote-callsite-lint` — AST-based CI lint enforcing the
//! engine-only promote-callsite discipline for marque.
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
//!   `Result<CanonicalAttrs, _>`) outside the two whitelisted
//!   sites: `unsafe fn` and `MarkingScheme::canonicalize`.
//!   Diagnostic code `PRC100`. (A third path-based whitelist for
//!   the transitional `from_parsed_unchecked` adapter retired
//!   along with the adapter itself.)
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
