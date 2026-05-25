// Adapted from CocoIndex
// SPDX-FileCopyrightText: 2025-2026 CocoIndex
// SPDX-License-Identifier: Apache-2.0
//
// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::error::{Error, Result};
use crate::retryable;

pub async fn request(
    req_builder: impl Fn() -> reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    let resp = retryable::run(
        || async {
            let req = req_builder();
            let resp = req.send().await?;
            let Err(err) = resp.error_for_status_ref() else {
                return Ok(resp);
            };

            let is_retryable = retryable::IsRetryable::is_retryable(&err);

            let mut error: Error = err.into();
            let body = resp.text().await?;
            if !body.is_empty() {
                error = error.context(format!("Error message body:\n{body}"));
            }

            Err(retryable::Error {
                error,
                is_retryable,
            })
        },
        &retryable::HEAVY_LOADED_OPTIONS,
    )
    .await?;
    Ok(resp)
}
