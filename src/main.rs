// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

mod batch;
mod config;
mod download;
mod jwt;
mod upload;

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use axum;
use axum::Router;
use axum::routing::{get, post, put};
use clap::{ArgMatches, Command, arg};
use tokio;
use tokio::fs::create_dir_all;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{Level, debug, error, info, warn};
use tracing_subscriber;

use config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub(crate) struct AppState {
    pub config: Arc<Config>,
    pub lfs_endpoint: String,
}

/// Shutdown signal handler.
async fn shutdown_signal() {
    info!("Server started"); // This function will be called when the server is started.
    // TODO: Trap other signals
    if let Err(e) = signal::ctrl_c().await {
        error!("Signal error: {e}");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .init();

    let matches: ArgMatches = Command::new("OxLFS")
        .version(VERSION)
        .author("KATO Hayate")
        .about("A Git LFS server written in Rust")
        .arg(arg!(-c --config <FILE> "Configuration file path").required(false))
        .get_matches();

    info!("OxLFS v{}", VERSION);
    warn!("As this application is a beta version, it may contain many breaking changes.");
    debug!("Command line arguments: {:?}", matches);

    let config_path: &Path = if let Some(file) = matches.get_one::<String>("config") {
        Path::new(file)
    } else if cfg!(debug_assertions) {
        Path::new("./sysroot/etc/oxlfs/config.toml")
    } else if cfg!(target_family = "windows") {
        Path::new(r"C:\ProgramData\oxlfs\config.toml")
    } else {
        Path::new("/etc/oxlfs/config.toml")
    };

    info!("Loading configuration...");
    debug!("Configuration file path: {:?}", config_path);
    let config: Arc<Config> = Arc::new(Config::load(config_path).await?);
    info!("Configuration loaded");
    debug!("Configuration parameters: {:?}", config);

    info!("Starting server...");
    let storage_dir: &Path = Path::new(&config.storage_dir);
    create_dir_all(storage_dir).await?;
    let git_root: &str = config.git_root.as_deref().unwrap_or("").trim_matches('/');
    let lfs_endpoint: String = if git_root.is_empty() {
        "/{user}/{repo}/info/lfs".to_owned()
    } else {
        format!("/{git_root}/{{user}}/{{repo}}/info/lfs")
    };
    debug!("LFS endpoint: {}", lfs_endpoint);
    let batch_endpoint: String = lfs_endpoint.clone() + "/objects/batch";
    let upload_endpoint: String = lfs_endpoint.clone() + "/upload";
    let download_endpoint: String = lfs_endpoint.clone() + "/download";
    let state = AppState {
        config,
        lfs_endpoint,
    };
    let mut app: Router = Router::new()
        .route(&batch_endpoint, post(batch::handle))
        .route(&upload_endpoint, put(upload::handle))
        .route(&download_endpoint, get(download::handle))
        .with_state(state.clone());
    if state.config.healthcheck_endpoint.unwrap_or(true) {
        app = app.route("/", get(|| async { "OxLFS is running!" }));
    }
    let listener: TcpListener = TcpListener::bind(&state.config.listen).await?;
    info!("Listening on {}", &state.config.listen);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("Server stopped");
    Ok(())
}
