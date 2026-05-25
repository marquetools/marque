// Adapted from CocoIndex
// SPDX-FileCopyrightText: 2025-2026 CocoIndex
// SPDX-License-Identifier: Apache-2.0
//
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(feature = "batching")]
pub mod batching;
#[cfg(feature = "concur_control")]
pub mod concur_control;

#[cfg(feature = "deserialize")]
pub mod deser;
pub mod error;
#[cfg(feature = "fingerprint")]
pub mod fingerprint;
#[cfg(feature = "immutable")]
pub mod immutable;
#[cfg(feature = "retryable")]
pub mod retryable;

pub mod prelude;

#[cfg(feature = "bytes_decode")]
pub mod bytes_decode;
#[cfg(any(feature = "reqwest", feature = "http"))]
pub mod http;
#[cfg(any(feature = "sqlx", feature = "str_sanitize"))]
pub mod str_sanitize;
#[cfg(feature = "yaml")]
pub mod yaml_ser;
