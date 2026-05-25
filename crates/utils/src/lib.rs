// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

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
