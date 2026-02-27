use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(host: &str, port: u16, timeout: Duration, ct: CancellationToken) -> WaitResult {
    let start = Instant::now();
    let addr = format!("{}:{}", host, port);

    loop {
        if let Ok(Ok(_)) =
            tokio::time::timeout(Duration::from_secs(1), TcpStream::connect(&addr)).await
        {
            return WaitResult::success(
                start.elapsed(),
                Some(format!("{}:{} is accepting connections", host, port)),
            );
        }

        if start.elapsed() >= timeout {
            return WaitResult::timeout(
                start.elapsed(),
                Some(format!("{}:{} did not accept connections", host, port)),
            );
        }

        tokio::select! {
            _ = ct.cancelled() => {
                return WaitResult::error(start.elapsed(), Some("cancelled".into()));
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn port_already_listening() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let ct = CancellationToken::new();

        let r = wait("127.0.0.1", port, Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
        assert!(r.elapsed_seconds < 2.0);
    }

    #[tokio::test]
    async fn port_timeout_no_listener() {
        let ct = CancellationToken::new();
        let r = wait("127.0.0.1", 19111, Duration::from_secs(1), ct).await;
        assert_eq!(r.status, "timeout");
    }

    #[tokio::test]
    async fn port_delayed_listen() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener); // free the port

        let ct = CancellationToken::new();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(600)).await;
            let _listener = TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .unwrap();
            // hold it open long enough
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        let r = wait("127.0.0.1", port, Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
        assert!(r.elapsed_seconds >= 0.5);
    }

    #[tokio::test]
    async fn port_cancellation() {
        let ct = CancellationToken::new();
        let ct2 = ct.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            ct2.cancel();
        });

        let r = wait("127.0.0.1", 19112, Duration::from_secs(30), ct).await;
        assert_eq!(r.status, "error");
        assert!(r.detail.as_deref().unwrap().contains("cancelled"));
    }
}
