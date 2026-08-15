// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use std::io;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::str::Chars;

use anyhow::Result;
use axum::BoxError;
use axum::body::{Body, BodyDataStream, Bytes};
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use futures_util::{Stream, TryStreamExt};
use time::OffsetDateTime;
use tokio::fs::{File, create_dir_all, rename, try_exists};
use tokio::io::BufWriter;
use tokio_util::io::StreamReader;
use tracing::{debug, info, trace, warn};

use crate::AppState;
use crate::batch::BatchResponseObjectActionType;
use crate::jwt;
use crate::jwt::Claims;

/// Streams data from a given stream to a file at the specified path.
///
/// This function takes a stream of `Result<Bytes, E>` and writes its contents
/// to a file asynchronously. It returns an `Ok(())` upon successful file
/// creation and streaming, or an `Err` with a HTTP `StatusCode` and an error
/// message if an error occurs.
///
/// ## Type Parameters
///
/// - `S`: The type of the input stream. It must implement the `Stream` trait
/// containing items of type `Result<Bytes, E>`.
/// - `E`: The error type of the stream, which must implement `Into<BoxError>`.
///
/// ## Parameters
///
/// - `path`: A reference to the `Path` specifying the file to write the stream data to.
/// - `stream`: A stream of `Result<Bytes, E>` representing the data source.
///
/// ## Returns
///
/// - `Ok(())` if the file is created and the stream is successfully written to the file.
/// - `Err((StatusCode::INTERNAL_SERVER_ERROR, String))` if any error occurs during the
///   file creation, writing, or streaming process. The contained `String` provides details
///   about the error.
///
/// ## Errors
///
/// - Returns `StatusCode::INTERNAL_SERVER_ERROR` along with a descriptive error string if:
///   - The file cannot be created at the specified path.
///   - An I/O error occurs during the file writing or streaming process.
///
/// ## Notes
///
/// - The function uses `tokio::io::copy` to efficiently copy data from the stream to the file.
/// - A `BufWriter` is used to optimize file write operations.
async fn stream_to_file<S, E>(path: &Path, stream: S) -> Result<(), (StatusCode, String)>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    async {
        let body_with_io_error = stream.map_err(io::Error::other);
        let mut body_reader = pin!(StreamReader::new(body_with_io_error));

        let mut file: BufWriter<File> = BufWriter::new(File::create(path).await?);

        tokio::io::copy(&mut body_reader, &mut file).await?;

        Ok::<_, io::Error>(())
    }
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

/// Asynchronously retrieves or constructs the file path for a given object ID (oid).
///
/// This function takes a base path and an object ID and ensures that the directory
/// structure implied by the object ID exists within the base directory. The function
/// then computes the full path to a file corresponding to the provided object ID.
/// It creates any necessary directories along the way.
///
/// ## Parameters
///
/// - `base`: A [`PathBuf`] representing the base directory where the object file should reside.
/// - `oid`: A string slice representing the object ID, which is used to determine the directory structure.
///
/// # Returns
///
/// Returns a [`Result`] containing the full [`PathBuf`] of the object file if the operation
/// succeeds, or an error if any filesystem operation fails.
///
/// # Errors
///
/// This function will return an error in the following cases:
/// - If checking for the existence of the base directory fails.
/// - If creating the required directories fails.
/// - If there are other errors related to filesystem operations.
///
/// ## Notes
///
/// - The first two characters of the `oid` are used to create a subdirectory within the
///   base directory, which is an optimization commonly used in systems that handle a large
///   number of files (e.g., Git object storage).
pub(crate) async fn get_object_file_path(mut base: PathBuf, oid: &str) -> Result<PathBuf> {
    if !try_exists(&base).await? {
        create_dir_all(&base).await?;
    }
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
    create_dir_all(&base).await?;
    filename += &chars.collect::<String>();
    base.push(filename);
    Ok(base)
}

/// Handles upload requests.
pub(crate) async fn handle(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxPath((user, repo)): AxPath<(String, String)>,
    body: Body,
) -> StatusCode {
    info!("Upload to {}/{}", user, repo);
    trace!("headers: {:#?}", headers);
    let claims: Claims;
    if let Some(authorization) = headers.get("authorization") {
        let auth_str = authorization.to_str().unwrap().to_string();
        let jwt: &str = auth_str.trim_start_matches("Bearer ");
        match jwt::decode(jwt, &state.config.jwt_secret) {
            Ok(c) => claims = c,
            Err(e) => return StatusCode::UNAUTHORIZED,
        }
    } else {
        return StatusCode::UNAUTHORIZED;
    }
    trace!("JWT claims: {:#?}", claims);
    if claims.exp < OffsetDateTime::now_utc() {
        warn!("JWT expired");
        return StatusCode::UNAUTHORIZED;
    }
    if claims.lfs.action != BatchResponseObjectActionType::Upload {
        warn!("Invalid action");
        return StatusCode::BAD_REQUEST;
    }

    let mut path: PathBuf = PathBuf::new();
    path.push(&state.config.storage_dir);
    path.push(format!("{}/{}/objects", user, repo));
    path = get_object_file_path(path, &claims.lfs.oid).await.unwrap();
    debug!("path: {:?}", path);

    let mut tmp_path: PathBuf = path.clone();
    tmp_path.set_extension("tmp");
    debug!("tmp_path: {:?}", tmp_path);
    let stream: BodyDataStream = body.into_data_stream();
    stream_to_file(&tmp_path, stream).await.unwrap();
    // TODO: Check the checksum of the uploaded file
    rename(&tmp_path, &path).await.unwrap();

    info!("Upload complete, oid {}", claims.lfs.oid);
    StatusCode::OK
}
