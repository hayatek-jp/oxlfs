// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use std::path::Path;

use anyhow::Result;
use serde::Deserialize;
use tokio::fs::read_to_string;
use toml;

/// Configuration for the application.
#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    // TODO: Improve config format
    /// Address to listen
    pub(crate) listen: String,
    /// Whether to use TLS
    pub(crate) tls: bool,
    /// TLS certificate file path
    pub(crate) tls_cert: Option<String>,
    /// TLS key file path
    pub(crate) tls_key: Option<String>,
    /// Git root directory
    ///
    /// ## Examples
    ///
    /// * If the Git URL is `https://github.com/hayatek-jp/oxlfs.git`, the value is nothing.
    /// * If the Git URL is `https://example.com/repos/username/reponame.git`, the value is `repos`.
    pub(crate) git_root: Option<String>,
    /// Storage directory
    pub(crate) storage_dir: String,
    /// Log level
    ///
    /// The default value is `info`.
    pub(crate) log_level: Option<String>,
    /// Log directory
    pub(crate) log_dir: String,
    /// Whether to enable health check endpoint (`/`)
    pub(crate) healthcheck_endpoint: Option<bool>,
}

impl Config {
    /// Loads the configuration from the given path.
    ///
    /// ## Parameters
    ///
    /// * `path` - The configuration file path.
    ///
    /// ## Returns
    ///
    /// Configuration parameters
    pub(crate) async fn load(path: &Path) -> Result<Self> {
        let content: String = read_to_string(path).await?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}
