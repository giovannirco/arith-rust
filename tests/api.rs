//! The HTTP contract, end to end through the router: the same requests the
//! README documents, with the bodies it promises.

use arith::{AppState, router};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn app() -> Router {
    router(AppState::default())
}

async fn get(app: Router, uri: &str) -> (StatusCode, String, http::HeaderMap) {
    let response = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap(), headers)
}

#[tokio::test]
async fn the_worked_example() {
    let cases = [
        ("/api/sum?term_one=4&term_two=1", r#"{"result":5}"#),
        ("/api/sub?term_one=4&term_two=1", r#"{"result":3}"#),
        ("/api/mul?term_one=4&term_two=1", r#"{"result":4}"#),
        ("/api/div?term_one=7&term_two=2", r#"{"result":3}"#),
    ];
    for (uri, want) in cases {
        let (status, body, headers) = get(app(), uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(body, want, "{uri}");
        assert_eq!(headers[header::CONTENT_TYPE], "application/json", "{uri}");
    }
}

#[tokio::test]
async fn division_truncates_toward_zero() {
    let (_, body, _) = get(app(), "/api/div?term_one=-7&term_two=2").await;
    assert_eq!(body, r#"{"result":-3}"#);
    let (_, body, _) = get(app(), "/api/div?term_one=7&term_two=-2").await;
    assert_eq!(body, r#"{"result":-3}"#);
}

#[tokio::test]
async fn errors_are_400_with_a_reason() {
    let cases = [
        (
            "/api/div?term_one=1&term_two=0",
            r#"{"error":"division by zero"}"#,
        ),
        (
            "/api/sum?term_one=abc&term_two=1",
            r#"{"error":"term_one must be an integer, got \"abc\""}"#,
        ),
        (
            "/api/sum?term_one=1&term_two=1.5",
            r#"{"error":"term_two must be an integer, got \"1.5\""}"#,
        ),
        ("/api/sum?term_one=1", r#"{"error":"term_two is required"}"#),
        ("/api/mul", r#"{"error":"term_one is required"}"#),
        (
            "/api/sum?term_one=9223372036854775807&term_two=1",
            r#"{"error":"result does not fit in a 64-bit integer"}"#,
        ),
        (
            "/api/mul?term_one=-9223372036854775808&term_two=-1",
            r#"{"error":"result does not fit in a 64-bit integer"}"#,
        ),
        (
            "/api/div?term_one=-9223372036854775808&term_two=-1",
            r#"{"error":"result does not fit in a 64-bit integer"}"#,
        ),
        (
            "/api/sub?term_one=99999999999999999999&term_two=1",
            r#"{"error":"term_one does not fit in a 64-bit integer, got \"99999999999999999999\""}"#,
        ),
    ];
    for (uri, want) in cases {
        let (status, body, headers) = get(app(), uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(body, want, "{uri}");
        assert_eq!(headers[header::CONTENT_TYPE], "application/json", "{uri}");
    }
}

#[tokio::test]
async fn healthz() {
    let (status, body, _) = get(app(), "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, r#"{"status":"ok"}"#);
}

#[tokio::test]
async fn unknown_paths_and_methods_are_json_too() {
    let (status, body, _) = get(app(), "/api/pow?term_one=2&term_two=3").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, r#"{"error":"not found"}"#);

    let response = app()
        .oneshot(
            Request::post("/api/sum?term_one=1&term_two=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes, r#"{"error":"method not allowed"}"#.as_bytes());
}

#[tokio::test]
async fn the_page_and_its_assets() {
    let (status, body, headers) = get(app(), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
    assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
    assert!(body.contains("<title>arith</title>"));
    for op in ["sum", "sub", "mul", "div"] {
        assert!(body.contains(&format!("data-op=\"{op}\"")), "{op} button");
    }

    let (status, body, headers) = get(app(), "/style.css").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "text/css; charset=utf-8");
    assert!(body.contains("--accent"));

    let (status, body, headers) = get(app(), "/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert!(body.contains("/api/${operation}"));
}

#[tokio::test]
async fn metrics_count_what_was_served() {
    let app = app();
    for uri in [
        "/api/sum?term_one=4&term_two=1",
        "/api/sum?term_one=4&term_two=1",
        "/api/div?term_one=1&term_two=0",
        "/api/mul?term_one=x&term_two=1",
        "/nope",
    ] {
        get(app.clone(), uri).await;
    }
    let (status, body, headers) = get(app, "/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/openmetrics-text")
    );
    for line in [
        r#"http_requests_total{method="GET",route="/api/sum",status="200"} 2"#,
        r#"http_requests_total{method="GET",route="/api/div",status="400"} 1"#,
        r#"http_requests_total{method="GET",route="unmatched",status="404"} 1"#,
        r#"http_request_duration_seconds_count{method="GET",route="/api/sum"} 2"#,
        r#"arith_operations_total{operation="sum",outcome="ok"} 2"#,
        r#"arith_operations_total{operation="div",outcome="division_by_zero"} 1"#,
        r#"arith_operations_total{operation="mul",outcome="bad_input"} 1"#,
        "arith_build_info{version=",
    ] {
        assert!(body.contains(line), "missing {line:?} in:\n{body}");
    }
}
