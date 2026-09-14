//! eaten.at web application binary.

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use eaten_at::settings::Settings;
use eaten_at::{app, state};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

/// How often expired cache rows are swept.
const PURGE_INTERVAL: Duration = Duration::from_secs(10 * 60);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let settings = Settings::from_env().context("invalid configuration")?;
    let listener = TcpListener::bind(settings.listen)
        .await
        .with_context(|| format!("failed to bind {}", settings.listen))?;
    tracing::info!(addr = %settings.listen, public_url = %settings.public_url, "listening");

    let state = settings
        .build_state()
        .context("failed to build application state")?;
    tracing::info!(db = %settings.db.display(), "cache database open");
    tokio::spawn(purge_loop(state.clone()));

    // The peer address is the fallback for locating a request (plan 12).
    axum::serve(
        listener,
        app::router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server error")
}

/// Sweep expired cache rows on an interval for the life of the process.
async fn purge_loop(state: state::AppState) {
    let mut ticker = tokio::time::interval(PURGE_INTERVAL);
    ticker.tick().await; // the first tick fires immediately; skip it
    loop {
        ticker.tick().await;
        let removed = state.cache().purge_expired().await;
        if removed > 0 {
            tracing::debug!(removed, "purged expired cache rows");
        }
        let removed =
            state.sessions().purge_expired().await + state.oauth_store().purge_expired().await;
        if removed > 0 {
            tracing::debug!(removed, "purged expired sessions");
        }
    }
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(%err, "failed to install ctrl-c handler");
        return;
    }
    tracing::info!("shutdown requested");
}
