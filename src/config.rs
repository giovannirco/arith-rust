//! Settings. Everything comes from the environment; nothing is read from disk.
//!
//! `ARITH_*` variables are this program's own. The `OTEL_*` variables follow the
//! OpenTelemetry specification, so a collector's documentation applies as is.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `ARITH_ADDR`, default `0.0.0.0:8000`.
    pub addr: SocketAddr,
    /// `ARITH_LOG_LEVEL`, default `info`. Accepts `tracing` filter directives
    /// such as `info,arith::access=debug`.
    pub log_level: String,
    /// `ARITH_LOG_FORMAT`, `json` (default) or `text`.
    pub log_format: LogFormat,
    /// `ARITH_SHUTDOWN_TIMEOUT`, seconds to wait for in-flight requests after
    /// SIGTERM. Default 10.
    pub shutdown_timeout: Duration,
    /// `OTEL_TRACES_EXPORTER`, `none` (default) or `otlp`.
    pub traces_otlp: bool,
    /// `OTEL_LOGS_EXPORTER`, a comma-separated set of `console` and `otlp`.
    /// Default `console`.
    pub logs_console: bool,
    pub logs_otlp: bool,
    /// `OTEL_EXPORTER_OTLP_PROTOCOL`, `http/protobuf` (default) or `grpc`.
    pub otlp_protocol: OtlpProtocol,
    /// `OTEL_SERVICE_NAME`, default `arith`.
    pub service_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OtlpProtocol {
    HttpProtobuf,
    Grpc,
}

/// A variable that is set but cannot be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub var: &'static str,
    pub value: String,
    pub expected: &'static str,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}={:?} is not valid: expected {}",
            self.var, self.value, self.expected
        )
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn from_env() -> Result<Config, ConfigError> {
        Config::from_lookup(|name| std::env::var(name).ok())
    }

    /// Builds a config from any lookup function, so tests do not touch the
    /// process environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Config, ConfigError> {
        let get = |name: &'static str| get(name).filter(|v| !v.trim().is_empty());

        let addr = match get("ARITH_ADDR") {
            None => SocketAddr::from(([0, 0, 0, 0], 8000)),
            Some(v) => v
                .parse()
                .map_err(|_| invalid("ARITH_ADDR", &v, "host:port"))?,
        };

        let log_level = get("ARITH_LOG_LEVEL").unwrap_or_else(|| "info".to_owned());

        let log_format = match get("ARITH_LOG_FORMAT").as_deref() {
            None | Some("json") => LogFormat::Json,
            Some("text") => LogFormat::Text,
            Some(v) => return Err(invalid("ARITH_LOG_FORMAT", v, "json or text")),
        };

        let shutdown_timeout = match get("ARITH_SHUTDOWN_TIMEOUT") {
            None => Duration::from_secs(10),
            Some(v) => Duration::from_secs(
                v.parse()
                    .map_err(|_| invalid("ARITH_SHUTDOWN_TIMEOUT", &v, "a number of seconds"))?,
            ),
        };

        let traces_otlp = match get("OTEL_TRACES_EXPORTER").as_deref() {
            None | Some("none") => false,
            Some("otlp") => true,
            Some(v) => return Err(invalid("OTEL_TRACES_EXPORTER", v, "none or otlp")),
        };

        let (logs_console, logs_otlp) = match get("OTEL_LOGS_EXPORTER") {
            None => (true, false),
            Some(v) => {
                let mut console = false;
                let mut otlp = false;
                for item in v.split(',').map(str::trim) {
                    match item {
                        "console" => console = true,
                        "otlp" => otlp = true,
                        "none" => {}
                        _ => {
                            return Err(invalid(
                                "OTEL_LOGS_EXPORTER",
                                &v,
                                "console, otlp, console,otlp or none",
                            ));
                        }
                    }
                }
                (console, otlp)
            }
        };

        let otlp_protocol = match get("OTEL_EXPORTER_OTLP_PROTOCOL").as_deref() {
            None | Some("http/protobuf") => OtlpProtocol::HttpProtobuf,
            Some("grpc") => OtlpProtocol::Grpc,
            Some(v) => {
                return Err(invalid(
                    "OTEL_EXPORTER_OTLP_PROTOCOL",
                    v,
                    "http/protobuf or grpc",
                ));
            }
        };

        let service_name = get("OTEL_SERVICE_NAME").unwrap_or_else(|| "arith".to_owned());

        Ok(Config {
            addr,
            log_level,
            log_format,
            shutdown_timeout,
            traces_otlp,
            logs_console,
            logs_otlp,
            otlp_protocol,
            service_name,
        })
    }
}

fn invalid(var: &'static str, value: &str, expected: &'static str) -> ConfigError {
    ConfigError {
        var,
        value: value.to_owned(),
        expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<&str, &str> = vars.iter().copied().collect();
        Config::from_lookup(|name| map.get(name).map(|v| v.to_string()))
    }

    #[test]
    fn defaults() {
        let c = config(&[]).unwrap();
        assert_eq!(c.addr, "0.0.0.0:8000".parse().unwrap());
        assert_eq!(c.log_level, "info");
        assert_eq!(c.log_format, LogFormat::Json);
        assert_eq!(c.shutdown_timeout, Duration::from_secs(10));
        assert!(!c.traces_otlp);
        assert!(c.logs_console);
        assert!(!c.logs_otlp);
        assert_eq!(c.otlp_protocol, OtlpProtocol::HttpProtobuf);
        assert_eq!(c.service_name, "arith");
    }

    #[test]
    fn everything_set() {
        let c = config(&[
            ("ARITH_ADDR", "127.0.0.1:9000"),
            ("ARITH_LOG_LEVEL", "debug"),
            ("ARITH_LOG_FORMAT", "text"),
            ("ARITH_SHUTDOWN_TIMEOUT", "3"),
            ("OTEL_TRACES_EXPORTER", "otlp"),
            ("OTEL_LOGS_EXPORTER", "console, otlp"),
            ("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc"),
            ("OTEL_SERVICE_NAME", "calc"),
        ])
        .unwrap();
        assert_eq!(c.addr, "127.0.0.1:9000".parse().unwrap());
        assert_eq!(c.log_level, "debug");
        assert_eq!(c.log_format, LogFormat::Text);
        assert_eq!(c.shutdown_timeout, Duration::from_secs(3));
        assert!(c.traces_otlp);
        assert!(c.logs_console);
        assert!(c.logs_otlp);
        assert_eq!(c.otlp_protocol, OtlpProtocol::Grpc);
        assert_eq!(c.service_name, "calc");
    }

    #[test]
    fn empty_values_mean_unset() {
        let c = config(&[("ARITH_ADDR", "  "), ("OTEL_LOGS_EXPORTER", "")]).unwrap();
        assert_eq!(c.addr.port(), 8000);
        assert!(c.logs_console);
    }

    #[test]
    fn logs_none_turns_everything_off() {
        let c = config(&[("OTEL_LOGS_EXPORTER", "none")]).unwrap();
        assert!(!c.logs_console);
        assert!(!c.logs_otlp);
    }

    #[test]
    fn bad_values_name_the_variable() {
        let cases = [
            ("ARITH_ADDR", "eight thousand", "host:port"),
            ("ARITH_LOG_FORMAT", "yaml", "json or text"),
            ("ARITH_SHUTDOWN_TIMEOUT", "soon", "a number of seconds"),
            ("OTEL_TRACES_EXPORTER", "jaeger", "none or otlp"),
            (
                "OTEL_LOGS_EXPORTER",
                "syslog",
                "console, otlp, console,otlp or none",
            ),
            (
                "OTEL_EXPORTER_OTLP_PROTOCOL",
                "http/json",
                "http/protobuf or grpc",
            ),
        ];
        for (var, value, expected) in cases {
            let err = config(&[(var, value)]).unwrap_err();
            assert_eq!(err.var, var);
            assert_eq!(err.value, value);
            assert_eq!(err.expected, expected);
            assert_eq!(
                err.to_string(),
                format!("{var}={value:?} is not valid: expected {expected}")
            );
        }
    }
}
