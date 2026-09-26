mod health;
mod workbooks;

use axum::Router;

use crate::server::AppState;

/// Create the API router
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(workbooks::router())
}
