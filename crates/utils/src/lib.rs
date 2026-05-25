// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared infrastructure for the marque workspace, ported from `recoco-utils`.
//!
//! The crate collects small, dependency-light building blocks that several
//! marque crates reuse. Every module past [`error`] and [`prelude`] sits behind
//! its own Cargo feature, so a consumer pulls in only what it needs and nothing
//! drags in the others:
//!
//! - [`error`] — the workspace error type, context combinators, and the
//!   `client_*` / `internal_*` / `api_*` bail macros. Always compiled.
//! - [`prelude`] — the handful of names most call sites want in scope.
//! - `concur_control` (feature `concur_control`) — semaphore-based backpressure
//!   on in-flight rows and bytes.
//! - `batching` (feature `batching`) — coalesces concurrent single-item calls
//!   into batched runner invocations.
//! - `fingerprint` (feature `fingerprint`) — a 128-bit structural hash of any
//!   `Serialize` value, built on BLAKE3.
//! - `retryable` (feature `retryable`) — retry-with-backoff around a fallible
//!   async operation.
//! - `bytes_decode` (feature `bytes_decode`) — decodes a byte buffer to text,
//!   sniffing the BOM and falling back to UTF-8.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(feature = "batching")]
pub mod batching;
#[cfg(feature = "bytes_decode")]
pub mod bytes_decode;
#[cfg(feature = "concur_control")]
pub mod concur_control;
pub mod error;
#[cfg(feature = "fingerprint")]
pub mod fingerprint;
pub mod prelude;
#[cfg(feature = "retryable")]
pub mod retryable;
