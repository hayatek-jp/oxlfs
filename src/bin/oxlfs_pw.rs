// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use std::env;

use anyhow::{Result, anyhow};
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHasher};

/// Hashes a password using the Argon2 hashing algorithm.
///
/// ## Parameters
/// - `password`: The plain text password to be hashed.
///
/// ## Returns
/// - `Ok(String)`: The hashed password as a [`String`] if the operation is successful.
/// - `Err(_)`: An error if the hashing operation fails.
fn hash_password(password: String) -> Result<String> {
    let salt: SaltString = SaltString::generate(&mut OsRng);
    let argon2: Argon2 = Argon2::default();
    let password_hash: String = argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string();
    Ok(password_hash)
}

fn main() -> Result<()> {
    if let Some(password) = env::args().nth(1) {
        println!("{}", hash_password(password)?);
        Ok(())
    } else {
        eprintln!("Usage: oxlfs_pw <password>");
        Err(anyhow!("Missing password argument"))
    }
}
