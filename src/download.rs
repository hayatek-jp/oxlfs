// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use std::fs::Metadata;
use std::path::PathBuf;
use std::str::Chars;

use axum::body::Body;
use axum::extract::Path;
use axum::extract::State;
use axum::http::response::Builder;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use time::OffsetDateTime;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::{info, trace, warn};

use crate::jwt::Claims;
use crate::{AppState, jwt};

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
fn get_object_path(mut base: PathBuf, oid: &str) -> PathBuf {
    let mut filename: String = String::new();
    let mut chars: Chars = oid.chars();
    while let Some(c1) = chars.next() {
        if let Some(c2) = chars.next() {
            base.push(format!("{}{}", c1, c2));
            // TODO: Recursively search for the appropriate directory
            break;
        } else {
            filename.push(c1);
        }
    }
    filename += &chars.collect::<String>();
    base.push(filename);
    base
}

/// Handles download requests.
pub(crate) async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user, repo)): Path<(String, String)>,
    body: Body,
) -> Response {
    info!("Download from {}/{}", user, repo);
    trace!("headers: {:#?}", headers);
    let mut builder: Builder = Response::builder();
    let claims: Claims;
    if let Some(authorization) = headers.get("authorization") {
        let auth_str = authorization.to_str().unwrap().to_string();
        let jwt: &str = auth_str.trim_start_matches("Bearer ");
        match jwt::decode(jwt, &state.config.jwt_secret) {
            Ok(c) => claims = c,
            Err(e) => {
                builder = builder.status(StatusCode::UNAUTHORIZED);
                return builder
                    .body(StatusCode::UNAUTHORIZED.as_str().into())
                    .unwrap();
            }
        }
    } else {
        builder = builder.status(StatusCode::UNAUTHORIZED);
        return builder
            .body(StatusCode::UNAUTHORIZED.as_str().into())
            .unwrap();
    }
    trace!("JWT claims: {:#?}", claims);
    if claims.exp < OffsetDateTime::now_utc() {
        builder = builder.status(StatusCode::UNAUTHORIZED);
        return builder
            .body(StatusCode::UNAUTHORIZED.as_str().into())
            .unwrap();
    }
    let mut base: PathBuf = PathBuf::new();
    base.push(&state.config.storage_dir);
    base.push(format!("{}/{}/objects", user, repo));
    let object_file: PathBuf = get_object_path(base, &claims.lfs.oid);
    if !object_file.exists() {
        warn!("Requested object not found: {:?}", object_file);
        builder = builder.status(StatusCode::NOT_FOUND);
        return builder.body(StatusCode::NOT_FOUND.as_str().into()).unwrap();
    }
    trace!("object_file: {:?}", object_file);
    let file: File = File::open(object_file).await.unwrap();
    let metadata: Metadata = file.metadata().await.unwrap();

    let stream: ReaderStream<File> = ReaderStream::new(file);
    let body: Body = Body::from_stream(stream);

    let response: Response = builder
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, metadata.len())
        .body(body)
        .unwrap()
        .into_response();
    info!("Generated response for download request");
    response
}
