pub mod api;
pub mod collab;
pub mod config;
pub mod db;
pub mod error;

use axum::Router;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::server::collab::DocumentStore;
use crate::server::config::Config;
use crate::server::db::Database;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub docs: Arc<DocumentStore>,
}

/// Run the server with the given configuration
pub async fn run_server(config: Config) -> anyhow::Result<()> {
    // Initialize database (now using cclab-sheet-db instead of PostgreSQL)
    let db = Database::connect(&config.database_path).await?;

    // Initialize database (replaces PostgreSQL migrations)
    db.migrate().await?;

    // Initialize document store for collaboration
    let docs = Arc::new(DocumentStore::new());

    // Create application state
    let state = AppState { db, docs };

    // Build the router
    let app = Router::new()
        .merge(api::router())
        .merge(collab::router())
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // Start the server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
