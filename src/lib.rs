//! arith: integer arithmetic over HTTP.
//!
//! | Module | Holds |
//! |---|---|
//! | [`calc`] | the four operations on `i64` |
//! | [`api`] | the JSON handlers and their error shape |
//! | [`page`] | the page at `/` |
//! | [`observe`] | the per-request span, metrics and access log |
//! | [`metrics`] | the Prometheus registry behind `/metrics` |
//! | [`telemetry`] | where logs and traces are sent |
//! | [`config`] | the environment variables |
//! | [`server`] | listening and graceful shutdown |
//!
//! [`router`] wires them together; `main.rs` reads the environment and signals.

pub mod api;
pub mod calc;
pub mod config;
pub mod metrics;
pub mod observe;
pub mod page;
pub mod server;
pub mod telemetry;

use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::get;

use crate::metrics::Metrics;

/// What handlers share. Cloned per request, so everything inside is an `Arc`.
#[derive(Clone, Default)]
pub struct AppState {
    pub metrics: Arc<Metrics>,
}

/// Every route the process serves.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(page::index))
        .route("/style.css", get(page::style))
        .route("/app.js", get(page::script))
        .route("/api/sum", get(api::sum))
        .route("/api/sub", get(api::sub))
        .route("/api/mul", get(api::mul))
        .route("/api/div", get(api::div))
        .route("/healthz", get(api::healthz))
        .route("/metrics", get(api::metrics))
        .fallback(api::not_found)
        .method_not_allowed_fallback(api::method_not_allowed)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            observe::observe,
        ))
        .with_state(state)
}
