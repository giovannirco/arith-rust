//! Logs, traces and their exporters.
//!
//! Metrics are not here: they are pulled from `/metrics` and live in
//! `metrics.rs`. This module decides where log lines and spans go, from the
//! `OTEL_*` variables in `Config`:
//!
//! | Variable | Effect |
//! |---|---|
//! | `OTEL_LOGS_EXPORTER=console` | structured lines on stdout (default) |
//! | `OTEL_LOGS_EXPORTER=otlp` | log records over OTLP, trace ids attached |
//! | `OTEL_TRACES_EXPORTER=otlp` | one span per request over OTLP |
//! | `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` (default) or `grpc` |
//! | `OTEL_EXPORTER_OTLP_ENDPOINT` | the collector, Tempo or Loki; per-signal `_TRACES_` and `_LOGS_` variants work too |
//!
//! The endpoint, headers, timeout and sampler variables are read by the
//! OpenTelemetry SDK itself, so the whole `OTEL_EXPORTER_OTLP_*` family applies.

use std::fmt;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{
    ExporterBuildError, LogExporter, Protocol, SpanExporter, WithExportConfig,
};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Subscriber;
use tracing_subscriber::filter::{EnvFilter, filter_fn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, Registry};

use crate::config::{Config, LogFormat, OtlpProtocol};

/// Handles to the exporters, so they can be flushed on shutdown.
pub struct Telemetry {
    tracer_provider: Option<SdkTracerProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

#[derive(Debug)]
pub enum TelemetryError {
    LogFilter(tracing_subscriber::filter::ParseError),
    Exporter(&'static str, ExporterBuildError),
    AlreadyInitialised,
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelemetryError::LogFilter(err) => {
                write!(f, "ARITH_LOG_LEVEL is not a valid filter: {err}")
            }
            TelemetryError::Exporter(signal, err) => {
                write!(f, "cannot build the OTLP {signal} exporter: {err}")
            }
            TelemetryError::AlreadyInitialised => f.write_str("telemetry was initialised twice"),
        }
    }
}

impl std::error::Error for TelemetryError {}

/// Installs the global `tracing` subscriber. Call once, from inside the Tokio
/// runtime: the gRPC exporter needs it to open its channel.
pub fn init(config: &Config) -> Result<Telemetry, TelemetryError> {
    let (subscriber, telemetry) = build(config)?;
    subscriber
        .try_init()
        .map_err(|_| TelemetryError::AlreadyInitialised)?;
    Ok(telemetry)
}

/// Assembles the subscriber and its exporters without installing anything.
pub fn build(
    config: &Config,
) -> Result<(impl Subscriber + Send + Sync + 'static, Telemetry), TelemetryError> {
    // Both OTLP transports speak TLS through rustls. Naming the crypto provider
    // once here beats a panic at the first https export.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let filter = EnvFilter::try_new(&config.log_level).map_err(TelemetryError::LogFilter)?;
    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .build();

    let console: Option<Box<dyn Layer<Registry> + Send + Sync>> =
        match (config.logs_console, config.log_format) {
            (false, _) => None,
            (true, LogFormat::Json) => Some(
                tracing_subscriber::fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(false)
                    .with_span_list(false)
                    .boxed(),
            ),
            (true, LogFormat::Text) => Some(tracing_subscriber::fmt::layer().compact().boxed()),
        };

    let mut tracer_provider = None;
    let traces = if config.traces_otlp {
        let exporter = span_exporter(config.otlp_protocol)
            .map_err(|e| TelemetryError::Exporter("traces", e))?;
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource.clone())
            .build();
        global::set_text_map_propagator(TraceContextPropagator::new());
        let layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("arith"));
        tracer_provider = Some(provider);
        Some(layer)
    } else {
        None
    };

    let mut logger_provider = None;
    let logs = if config.logs_otlp {
        let exporter =
            log_exporter(config.otlp_protocol).map_err(|e| TelemetryError::Exporter("logs", e))?;
        let provider = SdkLoggerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build();
        // The exporter's own HTTP stack logs through `tracing` as well. Shipping
        // those lines would make each export produce more lines to export.
        let layer = OpenTelemetryTracingBridge::new(&provider).with_filter(filter_fn(|meta| {
            ![
                "opentelemetry",
                "hyper",
                "tonic",
                "h2",
                "reqwest",
                "rustls",
                "tower",
            ]
            .iter()
            .any(|noisy| meta.target().starts_with(noisy))
        }));
        logger_provider = Some(provider);
        Some(layer)
    } else {
        None
    };

    // The level filter goes last; as a plain layer it applies to the whole
    // stack wherever it sits, and last keeps the other layers' types simple.
    let subscriber = tracing_subscriber::registry()
        .with(console)
        .with(traces)
        .with(logs)
        .with(filter);

    Ok((
        subscriber,
        Telemetry {
            tracer_provider,
            logger_provider,
        },
    ))
}

impl Telemetry {
    /// Flushes what is still buffered. Errors go to stderr: by now the
    /// subscriber may be the thing that is shutting down.
    pub fn shutdown(self) {
        if let Some(provider) = self.tracer_provider
            && let Err(err) = provider.shutdown()
        {
            eprintln!("arith: flushing traces: {err}");
        }
        if let Some(provider) = self.logger_provider
            && let Err(err) = provider.shutdown()
        {
            eprintln!("arith: flushing logs: {err}");
        }
    }
}

impl Telemetry {
    /// Whether spans are being exported.
    pub fn exports_traces(&self) -> bool {
        self.tracer_provider.is_some()
    }

    /// Whether log records are being exported over OTLP.
    pub fn exports_logs(&self) -> bool {
        self.logger_provider.is_some()
    }
}

fn span_exporter(protocol: OtlpProtocol) -> Result<SpanExporter, ExporterBuildError> {
    match protocol {
        OtlpProtocol::Grpc => SpanExporter::builder().with_tonic().build(),
        OtlpProtocol::HttpProtobuf => SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build(),
    }
}

fn log_exporter(protocol: OtlpProtocol) -> Result<LogExporter, ExporterBuildError> {
    match protocol {
        OtlpProtocol::Grpc => LogExporter::builder().with_tonic().build(),
        OtlpProtocol::HttpProtobuf => LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(vars: &[(&str, &str)]) -> Config {
        let map: HashMap<&str, &str> = vars.iter().copied().collect();
        Config::from_lookup(|name| map.get(name).map(|v| v.to_string())).unwrap()
    }

    #[test]
    fn console_only_exports_nothing() {
        let (_subscriber, telemetry) = build(&config(&[])).unwrap();
        assert!(!telemetry.exports_traces());
        assert!(!telemetry.exports_logs());
        telemetry.shutdown();
    }

    #[test]
    fn text_format_and_silent_console_both_build() {
        let (_subscriber, telemetry) = build(&config(&[("ARITH_LOG_FORMAT", "text")])).unwrap();
        telemetry.shutdown();
        let (_subscriber, telemetry) = build(&config(&[("OTEL_LOGS_EXPORTER", "none")])).unwrap();
        telemetry.shutdown();
    }

    #[tokio::test]
    async fn otlp_over_http_builds_both_exporters() {
        let (_subscriber, telemetry) = build(&config(&[
            ("OTEL_TRACES_EXPORTER", "otlp"),
            ("OTEL_LOGS_EXPORTER", "console,otlp"),
        ]))
        .unwrap();
        assert!(telemetry.exports_traces());
        assert!(telemetry.exports_logs());
        telemetry.shutdown();
    }

    #[tokio::test]
    async fn otlp_over_grpc_builds_both_exporters() {
        let (_subscriber, telemetry) = build(&config(&[
            ("OTEL_TRACES_EXPORTER", "otlp"),
            ("OTEL_LOGS_EXPORTER", "otlp"),
            ("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc"),
        ]))
        .unwrap();
        assert!(telemetry.exports_traces());
        assert!(telemetry.exports_logs());
        telemetry.shutdown();
    }

    #[test]
    fn a_bad_log_level_is_reported_by_name() {
        let err = build(&config(&[("ARITH_LOG_LEVEL", "info,=warn")]))
            .err()
            .unwrap();
        assert!(matches!(err, TelemetryError::LogFilter(_)));
        assert!(
            err.to_string()
                .starts_with("ARITH_LOG_LEVEL is not a valid filter")
        );
    }
}
