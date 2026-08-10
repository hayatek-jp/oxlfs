// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

use anyhow::Result;
use axum;
use axum::Router;
use axum::routing::get;
use tokio;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{Level, error, info};
use tracing_subscriber;

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
    info!(concat!("OxLFS v", env!("CARGO_PKG_VERSION")));

    info!("Starting server...");
    let app: Router = Router::new().route("/", get(|| async { "OxLFS is running!" })); // TODO: Provide an option to disable this endpoint
    let addr: &str = "0.0.0.0:8080"; // TODO: Load from config file
    let listener: TcpListener = TcpListener::bind(addr).await?;
    info!("Listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
