use std::error::Error;
use std::process::ExitCode;

use arith::config::Config;
use arith::{AppState, router, server, telemetry};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("arith: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let config = Config::from_env()?;
    let telemetry = telemetry::init(&config)?;

    let listener = TcpListener::bind(config.addr).await?;
    tracing::info!(
        addr = %config.addr,
        version = env!("CARGO_PKG_VERSION"),
        traces = telemetry.exports_traces(),
        logs_otlp = telemetry.exports_logs(),
        "listening"
    );

    let (stop_tx, stop_rx) = watch::channel(false);
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("shutdown signal received");
        let _ = stop_tx.send(true);
    });

    server::serve(
        listener,
        router(AppState::default()),
        stop_rx,
        config.shutdown_timeout,
    )
    .await?;

    tracing::info!("stopped");
    telemetry.shutdown();
    Ok(())
}

async fn shutdown_signal() {
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}
