pub mod config_api;
pub mod config_db;
pub mod routes;
pub mod sse;

use axum::routing::get;
use axum::Router;

use crate::routes::AppState;

/// Build the web UI router with all /_ui/ routes.
/// HTML pages are unauthenticated; API routes require auth.
pub fn web_ui_routes(state: AppState) -> Router {
    use crate::middleware;

    // API routes (require auth)
    let api_routes = Router::new()
        .route("/metrics", get(routes::get_metrics))
        .route("/system", get(routes::get_system_info))
        .route("/models", get(routes::get_models))
        .route("/logs", get(routes::get_logs))
        .route(
            "/config",
            get(routes::get_config).put(routes::update_config),
        )
        .route("/config/history", get(routes::get_config_history))
        .route("/stream/metrics", get(sse::metrics_stream))
        .route("/stream/logs", get(sse::logs_stream))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ))
        .with_state(state.clone());

    // HTML pages + static assets (no auth)
    let page_routes = Router::new()
        .route("/", get(routes::dashboard_page))
        .route("/config", get(routes::config_page))
        .route("/assets/{filename}", get(routes::static_asset));

    Router::new()
        .nest("/_ui/api", api_routes)
        .nest("/_ui", page_routes)
}
