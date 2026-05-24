mod app;
mod auth;
mod cache;
mod config;
mod db;
mod images;
mod web;

use anyhow::Context;
use axum::{extract::DefaultBodyLimit, Router};
use config::Config;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "picbad=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    config.ensure_dirs().await?;

    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;

    let state = Arc::new(app::AppState::new(config.clone(), pool));
    let app = router(state.clone()).layer(TraceLayer::new_for_http());

    let addr: SocketAddr = config.bind.parse().context("invalid PICBAD_BIND")?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Picbad listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

fn router(state: Arc<app::AppState>) -> Router {
    Router::new()
        .merge(web::routes())
        .merge(auth::routes())
        .merge(images::routes())
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(state.config.max_upload_bytes))
        .layer(RequestBodyLimitLayer::new(state.config.max_upload_bytes))
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
}
