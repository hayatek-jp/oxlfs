// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

mod config;

use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;

use anyhow::Result;
use axum;
use axum::Router;
use axum::routing::get;
use clap::{ArgMatches, Command, arg};
use tokio;
use tokio::net::{TcpListener, TcpSocket};
use tokio::signal;
use tracing::{Level, debug, error, info, warn};
use tracing_subscriber;

use config::Config;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
        .with_max_level(Level::DEBUG)
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
    let config: Config = Config::load(config_path).await?;
    info!("Configuration loaded");
    debug!("Configuration parameters: {:?}", config);

    info!("Starting server...");
    let mut app: Router = Router::new();
    if config.healthcheck_endpoint.unwrap_or(true) {
        app = app.route("/", get(|| async { "OxLFS is running!" }));
    }
    let listener: TcpListener = TcpListener::bind(&config.listen).await?;
    info!("Listening on {}", &config.listen);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("Server stopped");
    Ok(())
}
