use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(host: &str, port: u16, timeout: Duration, ct: CancellationToken) -> WaitResult {
    let start = Instant::now();
    let addr = format!("{}:{}", host, port);

    loop {
        match tokio::time::timeout(Duration::from_secs(1), TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => {
                return WaitResult::success(
                    start.elapsed(),
                    Some(format!("{}:{} is accepting connections", host, port)),
                );
            }
            _ => {}
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
