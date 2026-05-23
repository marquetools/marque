// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::types::HealthResponse;
use axum::response::Json;

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        schema_version: marque_capco::SCHEMA_VERSION,
    })
}

pub async fn schema_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": marque_capco::SCHEMA_VERSION }))
}
