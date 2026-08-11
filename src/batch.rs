// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use axum::Json;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode, header};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{debug, trace};

/// The content type for LFS.
const CONTENT_TYPE: &str = "application/vnd.git-lfs+json";

/// Batch API Operation.
#[derive(Debug, Deserialize)]
pub(crate) enum Operation {
    #[serde(rename = "upload")]
    Upload,
    #[serde(rename = "download")]
    Download,
}

/// Batch API Transfer Method.
///
/// The default is [`Self::Basic`].
#[derive(Debug, Deserialize)]
pub(crate) enum Transfer {
    #[serde(rename = "basic")]
    Basic,
    #[serde(other)]
    Unknown,
}

impl Default for Transfer {
    fn default() -> Self {
        Self::Basic
    }
}

/// Batch API Hash Algorithm.
///
/// The default is [`Self::Sha256`]
#[derive(Debug, Deserialize)]
pub(crate) enum HashAlgo {
    #[serde(rename = "sha256")]
    Sha256,
}

impl Default for HashAlgo {
    fn default() -> Self {
        Self::Sha256
    }
}

/// Git Reference.
#[derive(Debug, Deserialize)]
pub(crate) struct BatchRequestRef {
    pub(crate) name: String,
}

/// Git LFS Object.
#[derive(Debug, Deserialize)]
pub(crate) struct BatchRequestObject {
    pub(crate) oid: String,
    pub(crate) size: u64,
}

/// Git LFS Batch API Request.
///
/// ## See also
/// * https://github.com/git-lfs/git-lfs/blob/main/docs/api/batch.md
#[derive(Debug, Deserialize)]
pub(crate) struct BatchRequest {
    /// Operation
    pub(crate) operation: Operation,
    /// Transfer methods, default is [`Transfer::Basic`]
    pub(crate) transfers: Vec<Transfer>,
    /// Git reference
    pub(crate) r#ref: Option<BatchRequestRef>,
    /// LFS objects
    pub(crate) objects: Vec<BatchRequestObject>,
    /// Hash algorithm, default is [`HashAlgo::Sha256`]
    pub(crate) hash_algo: Option<HashAlgo>,
}

/// Checks if the `content-type` header in the provided `HeaderMap` matches the
/// expected value of `application/vnd.git-lfs+json`.
///
/// ## Parameters
/// - `headers`: A reference to a [`HeaderMap`] containing the HTTP headers to validate.
///
/// ## Returns
/// - `true` if the `content-type` header exists and matches `application/vnd.git-lfs+json`.
/// - `false` if the `content-type` header is missing, cannot be converted to a valid string,
///   or does not match the expected value.
fn validate_content_type(headers: &HeaderMap) -> bool {
    headers
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .map_or(false, |ct| ct.starts_with(CONTENT_TYPE))
}

/// Checks if the `accept` header in the provided `HeaderMap` matches the
/// expected value of `application/vnd.git-lfs+json` or `*/*.
///
/// ## Parameters
/// - `headers`: A reference to a [`HeaderMap`] containing the HTTP headers to validate.
///
/// ## Returns
/// - `true` if the `accept` header exists and matches `application/vnd.git-lfs+json` or `*/*`.
/// - `false` if the `accept` header is missing, cannot be converted to a valid string,
///   or does not match the expected value.
fn validate_accept(headers: &HeaderMap) -> bool {
    headers
        .get("accept")
        .and_then(|ct| ct.to_str().ok())
        .map_or(false, |ct| {
            ct.starts_with(CONTENT_TYPE) || ct.starts_with("*/*")
        })
}

/// Handles LFS batch API requests.
pub(crate) async fn handle(
    headers: HeaderMap,
    Path((user, repo)): Path<(String, String)>,
    Json(payload): Json<BatchRequest>,
) -> (StatusCode, HeaderMap, Json<Value>) {
    trace!("headers: {:#?}", headers);
    if !validate_content_type(&headers) {
        return (
            StatusCode::BAD_REQUEST,
            HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
            Json(json!({
                "message": "Invalid Content-Type",
                // TODO: Add `documentation_url` and `request_id`
            })),
        );
    }
    if !validate_accept(&headers) {
        return (
            StatusCode::NOT_ACCEPTABLE,
            HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
            Json(json!({
                "message": format!("Accept header should be {}", CONTENT_TYPE),
                // TODO: Add `documentation_url` and `request_id`
            })),
        );
    }
    trace!("user: {}, repo: {}, payload: {:#?}", user, repo, payload);
    match payload.operation {
        Operation::Upload => (
            StatusCode::NOT_IMPLEMENTED,
            HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
            Json(json!({
                "message": "Upload operation is not implemented yet",
            })),
        ),
        Operation::Download => (
            StatusCode::NOT_IMPLEMENTED,
            HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
            Json(json!({
                "message": "Download operation is not implemented yet",
            })),
        ),
    }
}
