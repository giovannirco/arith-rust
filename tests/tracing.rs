//! Each request becomes one span, named after its route, and joins the trace
//! a caller started. Spans are captured in memory; no collector involved.

use arith::{AppState, router};
use axum::body::Body;
use axum::http::Request;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use tower::ServiceExt;
use tracing_subscriber::layer::SubscriberExt;

const TRACEPARENT: &str = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";

#[test]
fn one_span_per_request_joined_to_the_callers_trace() {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));

    tracing::subscriber::with_default(subscriber, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(async {
                let app = router(AppState::default());
                let traced = Request::get("/api/sum?term_one=4&term_two=1")
                    .header("traceparent", TRACEPARENT)
                    .body(Body::empty())
                    .unwrap();
                app.clone().oneshot(traced).await.unwrap();
                let fresh = Request::get("/api/div?term_one=1&term_two=0")
                    .body(Body::empty())
                    .unwrap();
                app.oneshot(fresh).await.unwrap();
            });
    });
    provider.force_flush().unwrap();

    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 2, "{spans:#?}");

    let sum = spans
        .iter()
        .find(|s| s.name == "GET /api/sum")
        .expect("sum span");
    assert_eq!(
        sum.span_context.trace_id().to_string(),
        "0af7651916cd43dd8448eb211c80319c",
        "continues the caller's trace"
    );
    assert_eq!(sum.parent_span_id.to_string(), "b7ad6b7169203331");
    let attr = |name: &str| {
        sum.attributes
            .iter()
            .find(|kv| kv.key.as_str() == name)
            .map(|kv| kv.value.to_string())
    };
    assert_eq!(attr("http.route").as_deref(), Some("/api/sum"));
    assert_eq!(attr("http.request.method").as_deref(), Some("GET"));
    assert_eq!(attr("http.response.status_code").as_deref(), Some("200"));

    let div = spans
        .iter()
        .find(|s| s.name == "GET /api/div")
        .expect("div span");
    assert_ne!(
        div.span_context.trace_id(),
        sum.span_context.trace_id(),
        "starts its own trace"
    );
    assert!(div.span_context.is_valid());
}
