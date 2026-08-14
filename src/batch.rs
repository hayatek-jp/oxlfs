// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};
use tracing::{info, trace, warn};
use url::Url;

use crate::AppState;
use crate::jwt;
use crate::jwt::{Claims, LfsClaims, UserClaims};

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
#[derive(Debug, Deserialize, Serialize, PartialEq)]
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
#[derive(Debug, Deserialize, Serialize)]
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
    pub(crate) oid: String, // TODO: Validate hash format
    pub(crate) size: u64,
}

/// Git LFS Batch API request.
///
/// ## See also
/// * https://github.com/git-lfs/git-lfs/blob/main/docs/api/batch.md
#[derive(Debug, Deserialize)]
pub(crate) struct BatchRequest {
    /// Operation
    pub(crate) operation: Operation,
    /// Transfer methods, default is [`Transfer::Basic`]
    pub(crate) transfers: Option<Vec<Transfer>>,
    /// Git reference
    pub(crate) r#ref: Option<BatchRequestRef>,
    /// LFS objects
    pub(crate) objects: Vec<BatchRequestObject>,
    /// Hash algorithm, default is [`HashAlgo::Sha256`]
    pub(crate) hash_algo: Option<HashAlgo>,
}

/// Batch API action types.
#[derive(Debug, Serialize, Eq, Hash, PartialEq)]
pub(crate) enum BatchResponseObjectActionType {
    #[serde(rename = "download")]
    Download,
    #[serde(rename = "upload")]
    Upload,
    #[serde(rename = "verify")]
    Verify,
}

/// Batch API action.
#[derive(Debug, Serialize)]
pub(crate) struct BatchResponseObjectAction {
    pub(crate) href: Url,
    pub(crate) header: HashMap<String, String>,
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) expires_at: OffsetDateTime,
}

/// Batch API error.
#[derive(Debug, Serialize)]
pub(crate) struct BatchResponseObjectError {
    pub(crate) code: u16, // StatusCode::VARIANT.as_u16()
    pub(crate) message: String,
}

/// Batch API object response.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum BatchResponseObject {
    Ok {
        oid: String,
        size: u64,
        authenticated: Option<bool>,
        actions: HashMap<BatchResponseObjectActionType, BatchResponseObjectAction>,
    },
    Err {
        oid: String,
        size: u64,
        error: BatchResponseObjectError,
    },
}

/// Git LFS Batch API response.
///
/// ## See also
/// * https://github.com/git-lfs/git-lfs/blob/main/docs/api/batch.md
#[derive(Debug, Serialize)]
pub(crate) struct BatchResponse {
    /// Transfer adapter
    pub(crate) transfer: Transfer,
    /// LFS Objects
    pub(crate) objects: Vec<BatchResponseObject>,
    /// LFS object hash algorithm
    pub(crate) hash_algo: HashAlgo,
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
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user, repo)): Path<(String, String)>,
    Json(payload): Json<BatchRequest>,
) -> (StatusCode, HeaderMap, Json<Value>) {
    trace!("headers: {:#?}", headers);
    if !validate_content_type(&headers) {
        warn!("Invalid content-type: {:?}", headers.get("content-type"));
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
        warn!("Invalid accept: {:?}", headers.get("accept"));
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
    // TODO: Authentication
    match payload.operation {
        Operation::Upload => {
            info!("Batch upload request to {}/{}", user, repo);
            if let Some(transfers) = payload.transfers
                && !transfers.contains(&Transfer::Basic)
            {
                warn!("Client does not support basic transfer adapter");
                return (
                    StatusCode::NOT_ACCEPTABLE,
                    HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
                    Json(json!({
                        "message": "Unacceptable transfer adapters"
                    })),
                );
            }
            let now: OffsetDateTime = OffsetDateTime::now_utc().truncate_to_second();
            let expires_at: OffsetDateTime = now + Duration::hours(1); // TODO: Use config
            // TODO: fallocate
            // NOTE: If `fallocate` fails for even a single object, return an error for all objects not present on the server.
            let response = BatchResponse {
                transfer: Transfer::Basic,
                objects: payload
                    .objects
                    .iter()
                    .map(|o| {
                        let mut actions: HashMap<
                            BatchResponseObjectActionType,
                            BatchResponseObjectAction,
                        > = HashMap::new();
                        let claims = Claims {
                            exp: expires_at,
                            iat: now,
                            iss: "OxLFS".to_string(),
                            user: UserClaims {
                                id: "anonymous".to_string(),
                            },
                            lfs: LfsClaims {
                                user: user.clone(),
                                repo: repo.clone(),
                                oid: o.oid.clone(),
                            },
                        };
                        let jwt: String = jwt::encode(claims, &state.config.jwt_secret).unwrap();
                        let header: HashMap<String, String> = HashMap::from_iter([(
                            header::AUTHORIZATION.to_string(),
                            format!("Bearer {}", jwt),
                        )]);
                        // TODO: Support verify action
                        actions.insert(
                            BatchResponseObjectActionType::Upload,
                            BatchResponseObjectAction {
                                href: format!(
                                    "http{}://{}{}/upload",
                                    if state.config.tls { "s" } else { "" },
                                    headers.get("host").unwrap().to_str().unwrap(),
                                    state.lfs_endpoint,
                                )
                                .replacen("{user}", &user, 1)
                                .replacen("{repo}", &repo, 1)
                                .parse()
                                .unwrap(),
                                header,
                                expires_at,
                            },
                        );
                        BatchResponseObject::Ok {
                            oid: o.oid.clone(),
                            size: o.size,
                            authenticated: Some(false), // TODO: Implement authentication
                            actions,
                        }
                    })
                    .collect(),
                hash_algo: HashAlgo::Sha256,
            };
            info!("Successfully generated response for batch upload request");
            (
                StatusCode::OK,
                HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
                Json(serde_json::to_value(response).unwrap()),
            )
        }
        Operation::Download => (
            StatusCode::NOT_IMPLEMENTED,
            HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
            Json(json!({
                "message": "Download operation is not implemented yet",
            })),
        ),
    }
}
