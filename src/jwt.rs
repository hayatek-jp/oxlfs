// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use anyhow::Result;
use jsonwebtoken;
use jsonwebtoken::{EncodingKey, Header};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::info;

/// A struct representing claims related to the user.
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct UserClaims<'a> {
    /// User id
    pub(crate) id: &'a str,
}

/// A struct representing claims related to the Git LFS (Large File Storage) system.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct LfsClaims<'a> {
    pub(crate) user: &'a str,
    pub(crate) repo: &'a str,
    pub(crate) oid: &'a str,
}

/// Represents the claims contained within a JSON Web Token (JWT).
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Claims<'a> {
    /// Expiration time as UTC timestamp
    #[serde(with = "time::serde::timestamp")]
    pub(crate) exp: OffsetDateTime,
    /// Issued time as UTC timestamp
    #[serde(with = "time::serde::timestamp")]
    pub(crate) iat: OffsetDateTime,
    /// Issuer
    pub(crate) iss: &'a str,
    /// User
    pub(crate) user: UserClaims<'a>,
    /// LFS
    pub(crate) lfs: LfsClaims<'a>,
}

/// Encodes JWT.
///
/// ## Parameters
/// - `claims`: JWT claims
/// - `secret`: JWT secret
///
/// ## Returns
/// JWT using the default Algorithm.
pub(crate) fn encode(claims: Claims, secret: &str) -> Result<String> {
    let key: EncodingKey = EncodingKey::from_base64_secret(secret)?;
    let jwt: String = jsonwebtoken::encode(&Header::default(), &claims, &key)?;
    info!(
        "JWT issued for user {}, oid {}",
        &claims.user.id, &claims.lfs.oid
    );
    Ok(jwt)
}
