// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::Chars;

use axum::Json;
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode, header};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};
use tokio::fs::try_exists;
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

/// Constructs a full file system path to a specific object within a base directory
/// using an object ID (OID). The OID is used to create a directory structure and
/// filename.
///
/// The function processes the provided OID in pairs of characters to determine
/// subdirectory names. These subdirectories are appended to the base path until
/// the first pair is resolved. The rest of the OID determines the file name.
///
/// ## Parameters
///
/// - `base`: A reference to the base directory [`Path`] where the object resides.
/// - `oid`: A string slice representing the object identifier (OID).
///
/// # Returns
///
/// Returns the constructed [`PathBuf`] representing the file path of the object.
fn get_object_path(base: &Path, oid: &str) -> PathBuf {
    let mut path: PathBuf = base.to_path_buf();
    let mut filename: String = String::new();
    let mut chars: Chars = oid.chars();
    while let Some(c1) = chars.next() {
        if let Some(c2) = chars.next() {
            path.push(format!("{}{}", c1, c2));
            // TODO: Recursively search for the appropriate directory
            break;
        } else {
            filename.push(c1);
        }
    }
    filename += &chars.collect::<String>();
    path.join(filename)
}

/// Checks whether an object with the specified object ID (OID) exists in the file structure.
///
/// This function searches for a file based on the given base directory and the object ID.
/// The OID is expected to be a string where the file structure is organized such that the
/// first two characters form a subdirectory name, and the remaining portion of the OID
/// determines the file name within that subdirectory.
///
/// ## Parameters
///
/// - `base`: A reference to the base directory path where the search begins.
/// - `oid`: A string containing the object ID used to locate the file.
///
/// ## Returns
///
/// - `true` if the file corresponding to the OID exists in the expected
///   directory structure under the given base directory.
/// - `false` otherwise.
async fn is_object_exists(base: &Path, oid: &str) -> bool {
    try_exists(&get_object_path(base, oid))
        .await
        .unwrap_or(false)
}

/// Handles LFS batch API requests.
pub(crate) async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxPath((user, repo)): AxPath<(String, String)>,
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
    let mut base_path: PathBuf = PathBuf::new();
    base_path.push(&state.config.storage_dir);
    base_path.push(format!("{}/{}/objects", user, repo));
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
                objects: join_all(payload.objects.iter().map(|o| async {
                    let mut actions: HashMap<
                        BatchResponseObjectActionType,
                        BatchResponseObjectAction,
                    > = HashMap::new();
                    if !is_object_exists(&base_path, &o.oid).await {
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
                    }
                    BatchResponseObject::Ok {
                        oid: o.oid.clone(),
                        size: o.size,
                        authenticated: Some(true),
                        actions,
                    }
                }))
                .await,
                hash_algo: HashAlgo::Sha256,
            };
            info!("Successfully generated response for batch upload request");
            (
                StatusCode::OK,
                HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
                Json(serde_json::to_value(response).unwrap()),
            )
        }
        Operation::Download => {
            info!("Batch download request to {}/{}", user, repo);
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
            let response = BatchResponse {
                transfer: Transfer::Basic,
                objects: join_all(payload.objects.iter().map(|o| async {
                    let mut actions: HashMap<
                        BatchResponseObjectActionType,
                        BatchResponseObjectAction,
                    > = HashMap::new();
                    // TODO: Authentication
                    let object_path: PathBuf = get_object_path(&base_path, &o.oid);
                    // TODO: Check object size
                    if try_exists(&object_path).await.unwrap_or(false) {
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
                        actions.insert(
                            BatchResponseObjectActionType::Download,
                            BatchResponseObjectAction {
                                href: format!(
                                    "http{}://{}{}/download",
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
                            authenticated: Some(true),
                            actions,
                        }
                    } else {
                        warn!("Requested object not found, oid {}", o.oid);
                        BatchResponseObject::Err {
                            oid: o.oid.clone(),
                            size: o.size,
                            error: BatchResponseObjectError {
                                code: 404,
                                message: "Object not found".to_string(),
                            },
                        }
                    }
                }))
                .await,
                hash_algo: HashAlgo::Sha256,
            };
            info!("Successfully generated response for batch download request");
            (
                StatusCode::OK,
                HeaderMap::from_iter([(header::CONTENT_TYPE, CONTENT_TYPE.parse().unwrap())]),
                Json(serde_json::to_value(response).unwrap()),
            )
        }
    }
}
