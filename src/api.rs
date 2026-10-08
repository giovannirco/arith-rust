//! The JSON API: `/api/{sum,sub,mul,div}` and `/healthz`.
//!
//! Every error the API returns is `{"error": "<why>"}` with a 4xx status, and
//! the text is specific enough to act on without reading this file.

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::AppState;
use crate::calc::Operation;
use crate::metrics::Outcome;

/// The success body: `{"result": 5}`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ResultBody {
    pub result: i64,
}

/// An error the client can fix. Rendered as `{"error": "..."}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

/// The error text, left on the response for the access log to pick up.
#[derive(Clone, Debug)]
pub struct ErrorMessage(pub String);

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct Body<'a> {
            error: &'a str,
        }
        let mut response = (
            self.status,
            Json(Body {
                error: &self.message,
            }),
        )
            .into_response();
        response.extensions_mut().insert(ErrorMessage(self.message));
        response
    }
}

pub async fn healthz() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn sum(State(app): State<AppState>, RawQuery(query): RawQuery) -> Response {
    operate(&app, Operation::Sum, query.as_deref())
}

pub async fn sub(State(app): State<AppState>, RawQuery(query): RawQuery) -> Response {
    operate(&app, Operation::Sub, query.as_deref())
}

pub async fn mul(State(app): State<AppState>, RawQuery(query): RawQuery) -> Response {
    operate(&app, Operation::Mul, query.as_deref())
}

pub async fn div(State(app): State<AppState>, RawQuery(query): RawQuery) -> Response {
    operate(&app, Operation::Div, query.as_deref())
}

pub async fn metrics(State(app): State<AppState>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, crate::metrics::CONTENT_TYPE)],
        app.metrics.render(),
    )
}

pub async fn not_found() -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        message: "not found".to_owned(),
    }
}

pub async fn method_not_allowed() -> ApiError {
    ApiError {
        status: StatusCode::METHOD_NOT_ALLOWED,
        message: "method not allowed".to_owned(),
    }
}

/// Parses the two terms, applies the operation, counts the outcome.
fn operate(app: &AppState, op: Operation, query: Option<&str>) -> Response {
    let (a, b) = match parse_terms(query.unwrap_or("")) {
        Ok(terms) => terms,
        Err(err) => {
            app.metrics.observe_operation(op, Outcome::BadInput);
            return err.into_response();
        }
    };
    match op.apply(a, b) {
        Ok(result) => {
            app.metrics.observe_operation(op, Outcome::Ok);
            Json(ResultBody { result }).into_response()
        }
        Err(err) => {
            app.metrics.observe_operation(op, err.into());
            ApiError::bad_request(err.to_string()).into_response()
        }
    }
}

/// Reads `term_one` and `term_two` from a query string.
///
/// The query is parsed by hand rather than through a serde extractor so that a
/// malformed request still gets the same `{"error": ...}` shape, and so the
/// message can quote exactly what was sent.
pub fn parse_terms(query: &str) -> Result<(i64, i64), ApiError> {
    let mut term_one = None;
    let mut term_two = None;
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "term_one" => term_one = Some(value.into_owned()),
            "term_two" => term_two = Some(value.into_owned()),
            _ => {}
        }
    }
    Ok((
        parse_term("term_one", term_one.as_deref())?,
        parse_term("term_two", term_two.as_deref())?,
    ))
}

fn parse_term(name: &str, value: Option<&str>) -> Result<i64, ApiError> {
    let Some(value) = value else {
        return Err(ApiError::bad_request(format!("{name} is required")));
    };
    match value.parse::<i64>() {
        Ok(n) => Ok(n),
        Err(_) if looks_like_an_integer(value) => Err(ApiError::bad_request(format!(
            "{name} does not fit in a 64-bit integer, got {value:?}"
        ))),
        Err(_) => Err(ApiError::bad_request(format!(
            "{name} must be an integer, got {value:?}"
        ))),
    }
}

/// An optional sign followed by digits: an integer, just not one that fits.
fn looks_like_an_integer(value: &str) -> bool {
    let digits = value.strip_prefix(['-', '+']).unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(message: &str) -> ApiError {
        ApiError::bad_request(message)
    }

    #[test]
    fn parses_both_terms() {
        assert_eq!(parse_terms("term_one=4&term_two=1"), Ok((4, 1)));
        assert_eq!(parse_terms("term_two=1&term_one=4"), Ok((4, 1)));
        // In a query string `+` is a space; a plus sign travels as `%2B`.
        assert_eq!(parse_terms("term_one=-7&term_two=%2B2"), Ok((-7, 2)));
        assert_eq!(
            parse_terms("term_one=-7&term_two=+2"),
            Err(err("term_two must be an integer, got \" 2\""))
        );
        assert_eq!(
            parse_terms("term_one=0&term_two=0&extra=ignored"),
            Ok((0, 0))
        );
        assert_eq!(
            parse_terms("term_one=9223372036854775807&term_two=-9223372036854775808"),
            Ok((i64::MAX, i64::MIN))
        );
    }

    #[test]
    fn repeated_keys_take_the_last_value() {
        assert_eq!(parse_terms("term_one=1&term_one=2&term_two=3"), Ok((2, 3)));
    }

    #[test]
    fn missing_terms() {
        assert_eq!(parse_terms(""), Err(err("term_one is required")));
        assert_eq!(parse_terms("term_two=1"), Err(err("term_one is required")));
        assert_eq!(parse_terms("term_one=1"), Err(err("term_two is required")));
    }

    #[test]
    fn non_integers() {
        assert_eq!(
            parse_terms("term_one=abc&term_two=1"),
            Err(err("term_one must be an integer, got \"abc\""))
        );
        assert_eq!(
            parse_terms("term_one=1&term_two=1.5"),
            Err(err("term_two must be an integer, got \"1.5\""))
        );
        assert_eq!(
            parse_terms("term_one=&term_two=1"),
            Err(err("term_one must be an integer, got \"\""))
        );
        assert_eq!(
            parse_terms("term_one=%201&term_two=1"),
            Err(err("term_one must be an integer, got \" 1\""))
        );
        assert_eq!(
            parse_terms("term_one=1&term_two=-"),
            Err(err("term_two must be an integer, got \"-\""))
        );
    }

    #[test]
    fn integers_that_do_not_fit_say_so() {
        assert_eq!(
            parse_terms("term_one=9223372036854775808&term_two=1"),
            Err(err(
                "term_one does not fit in a 64-bit integer, got \"9223372036854775808\""
            ))
        );
        assert_eq!(
            parse_terms("term_one=1&term_two=-99999999999999999999"),
            Err(err(
                "term_two does not fit in a 64-bit integer, got \"-99999999999999999999\""
            ))
        );
    }

    #[test]
    fn first_problem_wins() {
        assert_eq!(
            parse_terms("term_one=x&term_two=y"),
            Err(err("term_one must be an integer, got \"x\""))
        );
    }
}
