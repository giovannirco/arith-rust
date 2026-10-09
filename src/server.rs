//! Listening, and stopping without dropping requests on the floor.

use std::io;
use std::time::Duration;

use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::watch;

/// Serves `app` until `stop` turns true, then lets in-flight requests finish
/// for up to `grace`. Kubernetes sends SIGTERM and waits
/// `terminationGracePeriodSeconds`; keep `grace` below that.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    stop: watch::Receiver<bool>,
    grace: Duration,
) -> io::Result<()> {
    let mut drain = stop.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = drain.wait_for(|stopped| *stopped).await;
        tracing::info!("draining connections");
    });

    let mut deadline = stop;
    let deadline = async move {
        let _ = deadline.wait_for(|stopped| *stopped).await;
        tokio::time::sleep(grace).await;
    };

    tokio::select! {
        result = server.into_future() => result,
        () = deadline => {
            tracing::warn!(
                grace_seconds = grace.as_secs_f64(),
                "grace period over, exiting with connections still open"
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppState, router};
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    /// One HTTP/1.1 request over a plain socket, so the test depends on
    /// nothing but the server.
    async fn get(addr: SocketAddr, path: &str) -> String {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let request = format!("GET {path} HTTP/1.1\r\nHost: arith\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    }

    async fn start(
        grace: Duration,
    ) -> (
        SocketAddr,
        watch::Sender<bool>,
        tokio::task::JoinHandle<io::Result<()>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(serve(listener, router(AppState::default()), stop_rx, grace));
        (addr, stop_tx, task)
    }

    #[tokio::test]
    async fn serves_then_stops_when_asked() {
        let (addr, stop, task) = start(Duration::from_secs(5)).await;

        let response = get(addr, "/api/sum?term_one=4&term_two=1").await;
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        assert!(response.ends_with(r#"{"result":5}"#), "{response}");

        stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("stops within the grace period")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn gives_up_on_a_connection_that_never_finishes() {
        let (addr, stop, task) = start(Duration::from_millis(100)).await;

        // Half a request keeps the connection open; graceful shutdown alone
        // would wait on it forever.
        let mut stuck = TcpStream::connect(addr).await.unwrap();
        stuck
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: x")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        stop.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the deadline fires")
            .unwrap()
            .unwrap();
        drop(stuck);
    }
}
