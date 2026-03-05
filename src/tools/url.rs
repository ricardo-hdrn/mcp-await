use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(
    url: &str,
    expected_status: u16,
    timeout: Duration,
    ct: CancellationToken,
) -> WaitResult {
    let start = Instant::now();

    loop {
        let result = Command::new("curl")
            .args([
                "-s",
                "-o",
                "/dev/null",
                "-w",
                "%{http_code}",
                "--max-time",
                "10",
                url,
            ])
            .output()
            .await;

        match result {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return WaitResult::error(
                    start.elapsed(),
                    Some("curl is not installed — wait_for_url requires curl".into()),
                );
            }
            Ok(output) => {
                if let Ok(code) = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u16>()
                {
                    if code == expected_status {
                        return WaitResult::success(
                            start.elapsed(),
                            Some(format!("{} returned status {}", url, code)),
                        );
                    }
                }
            }
            _ => {}
        }

        if start.elapsed() >= timeout {
            return WaitResult::timeout(
                start.elapsed(),
                Some(format!("{} did not return status {}", url, expected_status)),
            );
        }

        tokio::select! {
            _ = ct.cancelled() => {
                return WaitResult::error(start.elapsed(), Some("cancelled".into()));
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn url_success_localhost() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                }
            }
        });

        let ct = CancellationToken::new();
        let url = format!("http://127.0.0.1:{}", port);
        let r = wait(&url, 200, Duration::from_secs(10), ct).await;
        assert_eq!(r.status, "success");
    }

    #[tokio::test]
    async fn url_timeout_bad_port() {
        let ct = CancellationToken::new();
        let r = wait("http://127.0.0.1:19222", 200, Duration::from_secs(3), ct).await;
        assert_eq!(r.status, "timeout");
    }
}
