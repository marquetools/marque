// Adapted from code originally in [CocoIndex](https://CocoIndex)
// Original code from CocoIndex is copyrighted by CocoIndex
// and licensed under the Apache-2.0 License.
// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2026 CocoIndex
//
// All modifications from the upstream for Marque are copyrighted by Knitli Inc.
// SPDX-FileCopyrightText: 2026 Knitli Inc. (Marque)
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! The names most marque code wants from this crate.
//!
//! Glob-import this module (`use marque_utils::prelude::*`) to bring the
//! workspace [`Error`] / [`Result`] types, the [`ContextExt`] combinators, and
//! the `client_*` / `internal_*` bail macros into scope.

pub use crate::error::ApiError;
pub use crate::error::invariance_violation;
pub use crate::error::{ContextExt, Error, Result};
pub use crate::{client_bail, client_error, internal_bail, internal_error};
