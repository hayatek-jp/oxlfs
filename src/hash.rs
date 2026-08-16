// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use tokio::task::spawn_blocking;

/// Asynchronously verifies if a provided plain text password matches a given hashed password.
///
/// This function offloads the computationally expensive password verification to a separate
/// thread to avoid blocking the async runtime. It uses the Argon2 hashing algorithm for
/// verification.
///
/// ## Parameters
///
/// - `password`: The plain text password to verify.
/// - `password_hash`: The hashed password to compare against.
///
/// ## Returns
///
/// A `Result` containing:
/// - `Ok(true)` if the password matches the hash.
/// - `Ok(false)` if the password does not match the hash.
/// - `Err` if an error occurs during parsing the hash or during the password verification process.
pub(crate) async fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let password: String = password.to_owned();
    let password_hash: String = password_hash.to_owned();
    spawn_blocking(move || -> Result<bool> {
        let parsed_hash: PasswordHash = PasswordHash::new(&password_hash)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    })
    .await?
}
