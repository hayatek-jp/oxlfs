// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use crate::batch::BatchResponseObjectActionType;
use anyhow::Result;
use jsonwebtoken;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::info;

/// A struct representing claims related to the user.
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct UserClaims {
    /// User id
    pub(crate) id: String,
}

/// A struct representing claims related to the Git LFS (Large File Storage) system.
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct LfsClaims {
    pub(crate) user: String,
    pub(crate) repo: String,
    pub(crate) oid: String,
    pub(crate) action: BatchResponseObjectActionType,
}

/// Represents the claims contained within a JSON Web Token (JWT).
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Claims {
    /// Expiration time as UTC timestamp
    #[serde(with = "time::serde::timestamp")]
    pub(crate) exp: OffsetDateTime,
    /// Issued time as UTC timestamp
    #[serde(with = "time::serde::timestamp")]
    pub(crate) iat: OffsetDateTime,
    /// Issuer
    pub(crate) iss: String,
    /// User
    pub(crate) user: UserClaims,
    /// LFS
    pub(crate) lfs: LfsClaims,
}

/// Encodes JWT.
///
/// ## Parameters
/// - `claims`: JWT claims
/// - `secret`: JWT secret
///
/// ## Returns
/// JWT using the default algorithm.
pub(crate) fn encode(claims: Claims, secret: &str) -> Result<String> {
    let key: EncodingKey = EncodingKey::from_base64_secret(secret)?;
    let jwt: String = jsonwebtoken::encode(&Header::default(), &claims, &key)?;
    info!(
        "JWT issued for user {}, oid {}",
        &claims.user.id, &claims.lfs.oid
    );
    Ok(jwt)
}

/// Decodes JWT using the default algorithm.
///
/// ## Parameters
/// - `jwt`: JWT
/// - `secret`: JWT secret
///
/// ## Returns
/// JWT claims.
pub(crate) fn decode(jwt: &str, secret: &str) -> Result<Claims> {
    let key: DecodingKey = DecodingKey::from_base64_secret(secret)?;
    let claims: Claims = jsonwebtoken::decode::<Claims>(jwt, &key, &Validation::default())?.claims;
    info!(
        "JWT validated for user {}, oid {}",
        claims.user.id, claims.lfs.oid
    );
    Ok(claims)
}
