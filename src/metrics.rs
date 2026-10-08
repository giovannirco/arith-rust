//! Prometheus metrics, served at `/metrics` in the OpenMetrics text format.
//!
//! Labels are kept to bounded sets: the route template rather than the path,
//! and an outcome enum rather than the error text.

use std::fmt;
use std::time::Duration;

use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue, LabelValueEncoder};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::metrics::info::Info;
use prometheus_client::registry::Registry;

use crate::calc::{CalcError, Operation};

pub const CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

pub struct Metrics {
    registry: Registry,
    requests: Family<RequestLabels, Counter>,
    duration: Family<RouteLabels, Histogram>,
    operations: Family<OperationLabels, Counter>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct RequestLabels {
    method: String,
    route: String,
    status: u16,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct RouteLabels {
    method: String,
    route: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct OperationLabels {
    operation: &'static str,
    outcome: Outcome,
}

/// How a call to `/api/<op>` ended.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    BadInput,
    DivisionByZero,
    Overflow,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::BadInput => "bad_input",
            Outcome::DivisionByZero => "division_by_zero",
            Outcome::Overflow => "overflow",
        }
    }
}

impl From<CalcError> for Outcome {
    fn from(err: CalcError) -> Self {
        match err {
            CalcError::DivisionByZero => Outcome::DivisionByZero,
            CalcError::Overflow => Outcome::Overflow,
        }
    }
}

impl EncodeLabelValue for Outcome {
    fn encode(&self, encoder: &mut LabelValueEncoder<'_>) -> fmt::Result {
        use std::fmt::Write;
        encoder.write_str(self.as_str())
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        let mut registry = Registry::default();

        let requests = Family::<RequestLabels, Counter>::default();
        registry.register(
            "http_requests",
            "HTTP requests served, by method, route template and status code",
            requests.clone(),
        );

        // Buckets from half a millisecond to a few seconds. Arithmetic is fast;
        // anything in the upper buckets is the network or the scheduler.
        let duration = Family::<RouteLabels, Histogram>::new_with_constructor(|| {
            Histogram::new([
                0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5,
            ])
        });
        registry.register(
            "http_request_duration_seconds",
            "Time to serve an HTTP request",
            duration.clone(),
        );

        let operations = Family::<OperationLabels, Counter>::default();
        registry.register(
            "arith_operations",
            "Arithmetic requests, by operation and outcome",
            operations.clone(),
        );

        registry.register(
            "arith_build",
            "Version of the running binary",
            Info::new(vec![("version", env!("CARGO_PKG_VERSION"))]),
        );

        Metrics {
            registry,
            requests,
            duration,
            operations,
        }
    }

    pub fn observe_request(&self, method: &str, route: &str, status: u16, elapsed: Duration) {
        self.requests
            .get_or_create(&RequestLabels {
                method: method.to_owned(),
                route: route.to_owned(),
                status,
            })
            .inc();
        self.duration
            .get_or_create(&RouteLabels {
                method: method.to_owned(),
                route: route.to_owned(),
            })
            .observe(elapsed.as_secs_f64());
    }

    pub fn observe_operation(&self, operation: Operation, outcome: Outcome) {
        self.operations
            .get_or_create(&OperationLabels {
                operation: operation.name(),
                outcome,
            })
            .inc();
    }

    /// The whole registry in OpenMetrics text format.
    pub fn render(&self) -> String {
        let mut out = String::new();
        prometheus_client::encoding::text::encode(&mut out, &self.registry)
            .expect("writing to a String cannot fail");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_requests_operations_and_build_info() {
        let m = Metrics::new();
        m.observe_request("GET", "/api/sum", 200, Duration::from_millis(2));
        m.observe_request("GET", "/api/sum", 200, Duration::from_millis(3));
        m.observe_request("GET", "/api/div", 400, Duration::from_micros(300));
        m.observe_operation(Operation::Sum, Outcome::Ok);
        m.observe_operation(Operation::Sum, Outcome::Ok);
        m.observe_operation(Operation::Div, CalcError::DivisionByZero.into());
        m.observe_operation(Operation::Mul, CalcError::Overflow.into());
        m.observe_operation(Operation::Sub, Outcome::BadInput);

        let text = m.render();
        assert!(
            text.contains(r#"http_requests_total{method="GET",route="/api/sum",status="200"} 2"#),
            "{text}"
        );
        assert!(
            text.contains(r#"http_requests_total{method="GET",route="/api/div",status="400"} 1"#),
            "{text}"
        );
        assert!(
            text.contains(
                r#"http_request_duration_seconds_count{method="GET",route="/api/sum"} 2"#
            ),
            "{text}"
        );
        assert!(
            text.contains(r#"arith_operations_total{operation="sum",outcome="ok"} 2"#),
            "{text}"
        );
        assert!(
            text.contains(
                r#"arith_operations_total{operation="div",outcome="division_by_zero"} 1"#
            ),
            "{text}"
        );
        assert!(
            text.contains(r#"arith_operations_total{operation="mul",outcome="overflow"} 1"#),
            "{text}"
        );
        assert!(
            text.contains(r#"arith_operations_total{operation="sub",outcome="bad_input"} 1"#),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                r#"arith_build_info{{version="{}"}} 1"#,
                env!("CARGO_PKG_VERSION")
            )),
            "{text}"
        );
        assert!(text.ends_with("# EOF\n"), "{text}");
    }
}
