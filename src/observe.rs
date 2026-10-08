//! One middleware for the three signals: a span per request, the request
//! counters and histogram, and one access-log line.
//!
//! The span joins an incoming W3C `traceparent` when there is one, so a trace
//! started by a gateway in front of the service continues into it.

use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::global;
use opentelemetry::trace::TraceContextExt;
use opentelemetry_http::HeaderExtractor;
use tracing::{Instrument, Level, field};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::AppState;
use crate::api::ErrorMessage;

pub async fn observe(State(app): State<AppState>, request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query = request.uri().query().map(str::to_owned);
    // The route template (`/api/sum`), not the path, keeps label cardinality
    // fixed. Requests that match no route share one label.
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str)
        .to_owned();

    let span = tracing::info_span!(
        "request",
        otel.name = format!("{method} {route}"),
        otel.kind = "server",
        http.request.method = %method,
        http.route = %route,
        url.path = %path,
        http.response.status_code = field::Empty,
        otel.status_code = field::Empty,
    );
    let parent = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });
    // Fails only when traces are off: there is no OpenTelemetry layer to hand
    // the parent to, and then there is nothing to join.
    let _ = span.set_parent(parent);

    let response = next.run(request).instrument(span.clone()).await;

    let status = response.status();
    span.record("http.response.status_code", status.as_u16());
    if status.is_server_error() {
        span.record("otel.status_code", "ERROR");
    }
    app.metrics
        .observe_request(method.as_str(), &route, status.as_u16(), started.elapsed());

    let error = response
        .extensions()
        .get::<ErrorMessage>()
        .map(|e| e.0.clone());
    let trace_id = {
        let context = span.context();
        let span_context = context.span().span_context().clone();
        span_context
            .is_valid()
            .then(|| span_context.trace_id().to_string())
    };
    // Probes and scrapes arrive every few seconds; they are logged at debug so
    // the default level shows the requests people made. The level of an event
    // has to be a constant, hence the small macro instead of a variable.
    macro_rules! access_log {
        ($level:expr) => {
            tracing::event!(
                target: "arith::access",
                $level,
                method = %method,
                path = %path,
                query = query.as_deref().unwrap_or(""),
                route = %route,
                status = status.as_u16(),
                duration_ms = started.elapsed().as_secs_f64() * 1000.0,
                error,
                trace_id,
                "request"
            )
        };
    }
    // Logged after the span closes, with the trace id as a field. Inside the
    // span the text formatter would repeat every span field on the line.
    if route == "/healthz" || route == "/metrics" {
        access_log!(Level::DEBUG);
    } else {
        access_log!(Level::INFO);
    }
    response
}
