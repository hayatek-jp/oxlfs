// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::Deserialize;
use tokio::fs::read_to_string;

#[derive(Debug, Deserialize)]
pub(crate) struct UserPermission {
    pub(crate) repo: String,
    pub(crate) read: bool,
    pub(crate) write: bool,
}

#[derive(Debug, Deserialize)]
struct ConfigUser {
    pub(crate) name: String,
    pub(crate) password_hash: String,
    pub(crate) permissions: Vec<UserPermission>,
}

#[derive(Debug, Deserialize)]
struct Public {
    pub(crate) repos: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UserConfig {
    pub(crate) public: Public,
    pub(crate) users: Vec<ConfigUser>,
}

#[derive(Debug)]
pub(crate) struct User {
    pub(crate) name: String,
    pub(crate) password_hash: String,
    pub(crate) permissions: HashMap<String, UserPermission>,
}

#[derive(Debug)]
pub(crate) struct UserDB {
    pub(crate) users: HashMap<String, User>,
}

impl UserDB {
    pub(crate) async fn load(path: &Path) -> Result<Self> {
        let content: String = read_to_string(path).await?;
        let config: UserConfig = toml::from_str(&content)?;
        let mut db = Self {
            users: config
                .users
                .into_iter()
                .map(|u| {
                    (
                        u.name.clone(),
                        User {
                            name: u.name,
                            password_hash: u.password_hash,
                            permissions: u
                                .permissions
                                .into_iter()
                                .map(|p| (p.repo.clone(), p))
                                .collect(),
                        },
                    )
                })
                .collect(),
        };
        if db.users.get("anonymous").is_some() {
            return Err(anyhow!("Username \"anonymous\" is not allowed"));
        }
        let anonymous: String = "anonymous".to_string();
        db.users.insert(
            anonymous.clone(),
            User {
                name: anonymous,
                password_hash: String::new(),
                permissions: config
                    .public
                    .repos
                    .into_iter()
                    .map(|r| {
                        (
                            r.clone(),
                            UserPermission {
                                repo: r,
                                read: true,
                                write: false,
                            },
                        )
                    })
                    .collect(),
            },
        );
        Ok(db)
    }
}
