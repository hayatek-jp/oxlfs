// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 KATO Hayate <dev@hayatek.jp>

mod batch;
mod config;
mod download;
mod hash;
mod jwt;
mod upload;
mod users;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum;
use axum::Router;
use axum::extract::ConnectInfo;
use axum::http::{HeaderValue, Request, header};
use axum::routing::{get, post, put};
use axum_server::Server;
#[cfg(feature = "tls-openssl")]
use axum_server::tls_openssl::{OpenSSLAcceptor, OpenSSLConfig};
#[cfg(feature = "tls-rustls")]
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use clap::{ArgMatches, Command, arg};
use tokio;
use tokio::fs::{create_dir_all, try_exists};
use tokio::net::TcpListener;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::ServiceBuilderExt;
use tower_http::request_id::{MakeRequestUuid, RequestId};
use tower_http::trace::TraceLayer;
use tracing::{Span, debug, error, info, info_span, warn};
use tracing_appender;
use tracing_appender::rolling;
use tracing_appender::rolling::RollingFileAppender;
use tracing_subscriber;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use config::Config;
use config::LogLevel;
use users::UserDB;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub(crate) struct AppState {
    pub config: Arc<Config>,
    pub user_db: Arc<UserDB>,
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
    let matches: ArgMatches = Command::new("OxLFS")
        .version(VERSION)
        .author("KATO Hayate")
        .about("A Git LFS server written in Rust")
        .arg(arg!(-c --config <FILE> "Configuration file path").required(false))
        .get_matches();

    let config_path: &Path = if let Some(file) = matches.get_one::<String>("config") {
        Path::new(file)
    } else if cfg!(debug_assertions) {
        Path::new("./sysroot/etc/oxlfs/config.toml")
    } else if cfg!(target_family = "windows") {
        Path::new(r"C:\ProgramData\oxlfs\config.toml")
    } else {
        Path::new("/etc/oxlfs/config.toml")
    };
    if !try_exists(config_path).await? {
        return Err(anyhow!("Configuration file not found: {:?}", config_path));
    }

    let mut config: Config = Config::load(config_path).await?;
    let config_dir: &Path = if let Some(file) = &config.config_dir {
        Path::new(file)
    } else if cfg!(debug_assertions) {
        Path::new("./sysroot/etc/oxlfs")
    } else if cfg!(target_family = "windows") {
        Path::new(r"C:\ProgramData\oxlfs")
    } else {
        Path::new("/etc/oxlfs")
    };

    let file_appender: RollingFileAppender = rolling::daily(&config.log_dir, "oxlfs.log");
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(if let Some(level) = &config.log_level {
            match level {
                LogLevel::Trace => LevelFilter::TRACE,
                LogLevel::Debug => LevelFilter::DEBUG,
                LogLevel::Info => LevelFilter::INFO,
                LogLevel::Warn => LevelFilter::WARN,
                LogLevel::Error => LevelFilter::ERROR,
                LogLevel::Off => LevelFilter::OFF,
            }
        } else if cfg!(debug_assertions) {
            LevelFilter::TRACE
        } else {
            LevelFilter::INFO
        })
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_writer)
                .with_ansi(false),
        )
        .try_init()
        .expect("Logger initialization failed");

    info!("OxLFS v{}", VERSION);
    warn!("As this application is a beta version, it may contain many breaking changes.");
    debug!("Command line arguments: {:?}", matches);
    debug!("Configuration file path: {:?}", config_path);
    let jwt_secret_stash: String = config.jwt_secret;
    config.jwt_secret = "[MASKED]".to_string();
    debug!("Configuration parameters: {:?}", config);
    config.jwt_secret = jwt_secret_stash;
    debug!("Configuration directory: {:?}", config_dir);

    info!("Loading user database...");
    let user_db_path: &Path = &config_dir.join("users.toml");
    let user_db: Arc<UserDB> = Arc::new(UserDB::load(user_db_path).await?);
    info!("User database loaded");
    // trace!("User database: {:?}", user_db);

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
    let access_id_middleware = ServiceBuilder::new()
        .set_x_request_id(MakeRequestUuid)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let request_id = request.extensions().get::<RequestId>().unwrap();
                    match request_id.header_value().to_str() {
                        Ok(request_id) => info_span!("http_request", request_id),
                        Err(_) => info_span!("http_request", request_id = ?request_id),
                    }
                })
                .on_request(|request: &Request<_>, _span: &Span| {
                    let remote_addr = request
                        .extensions()
                        .get::<ConnectInfo<SocketAddr>>()
                        .map(|ConnectInfo(addr)| *addr);
                    let empty_header_value = "".parse::<HeaderValue>().unwrap();
                    let forwarded_for = request
                        .headers()
                        .get(header::FORWARDED)
                        .unwrap_or(
                            request
                                .headers()
                                .get("x-forwarded-for")
                                .unwrap_or(&empty_header_value),
                        )
                        .to_str()
                        .unwrap();
                    // TODO: Improve handling for proxied requests

                    match remote_addr {
                        Some(addr) => {
                            info!(
                                "{} {} from {}{}",
                                request.method(),
                                request.uri().path(),
                                addr.ip(),
                                if !forwarded_for.is_empty() {
                                    format!(" (forwarded: {})", forwarded_for)
                                } else {
                                    String::new()
                                },
                            );
                        }
                        None => {
                            info!(
                                "{} {} from <unknown>",
                                request.method(),
                                request.uri().path(),
                            );
                        }
                    }
                }),
        )
        .propagate_x_request_id();
    let state = AppState {
        config: Arc::new(config),
        user_db,
        lfs_endpoint,
    };
    let mut app: Router = Router::new()
        .route(&batch_endpoint, post(batch::handle))
        .route(&upload_endpoint, put(upload::handle))
        .route(&download_endpoint, get(download::handle))
        .with_state(state.clone());
    if state.config.healthcheck_endpoint.unwrap_or(true) {
        app = app.route("/", get(|| async { "OxLFS is running!\n" }));
    }
    app = app.layer(access_id_middleware);
    if state.config.tls {
        if state.config.tls_cert.is_none() {
            return Err(anyhow!("TLS is enabled but cert is missing"));
        }
        if state.config.tls_key.is_none() {
            return Err(anyhow!("TLS is enabled but key is missing"));
        }
        if !cfg!(feature = "tls-rustls") && !cfg!(feature = "tls-openssl") {
            return Err(anyhow!(
                "TLS is enabled but no TLS implementation is available"
            ));
        }
        #[cfg(feature = "tls-openssl")]
        let tls_config: OpenSSLConfig = OpenSSLConfig::from_pem_file(
            state.config.tls_cert.as_ref().unwrap(),
            state.config.tls_key.as_ref().unwrap(),
        )?;
        #[cfg(feature = "tls-rustls")]
        let tls_config: RustlsConfig = RustlsConfig::from_pem_file(
            state.config.tls_cert.as_ref().unwrap(),
            state.config.tls_key.as_ref().unwrap(),
        )
        .await?;
        #[cfg(feature = "tls-openssl")]
        let server: Server<SocketAddr, OpenSSLAcceptor> =
            axum_server::bind_openssl(state.config.listen.parse::<SocketAddr>()?, tls_config);
        #[cfg(feature = "tls-rustls")]
        let server: Server<SocketAddr, RustlsAcceptor> =
            axum_server::bind_rustls(state.config.listen.parse::<SocketAddr>()?, tls_config);
        info!("HTTPS server listening on {}", state.config.listen);
        server
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    } else {
        let listener: TcpListener = TcpListener::bind(&state.config.listen).await?;
        info!("HTTP server listening on {}", state.config.listen);
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    }
    info!("Server stopped");
    Ok(())
}
