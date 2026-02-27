use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(
    command: &str,
    interval: Duration,
    timeout: Duration,
    ct: CancellationToken,
) -> WaitResult {
    let start = Instant::now();

    loop {
        let result = Command::new("sh").args(["-c", command]).output().await;

        match result {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let detail = if stdout.is_empty() {
                    "Command exited with code 0".into()
                } else {
                    stdout
                };
                return WaitResult::success(start.elapsed(), Some(detail));
            }
            Ok(_) => {} // non-zero exit, retry
            Err(e) => {
                return WaitResult::error(
                    start.elapsed(),
                    Some(format!("Failed to execute command: {}", e)),
                );
            }
        }

        if start.elapsed() >= timeout {
            return WaitResult::timeout(
                start.elapsed(),
                Some("Command did not exit 0 within timeout".into()),
            );
        }

        tokio::select! {
            _ = ct.cancelled() => {
                return WaitResult::error(start.elapsed(), Some("cancelled".into()));
            }
            _ = tokio::time::sleep(interval) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_immediate_success() {
        let ct = CancellationToken::new();
        let r = wait("true", Duration::from_secs(1), Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
        assert!(r.elapsed_seconds < 1.0);
    }

    #[tokio::test]
    async fn command_timeout_on_failure() {
        let ct = CancellationToken::new();
        let r = wait("false", Duration::from_secs(1), Duration::from_secs(2), ct).await;
        assert_eq!(r.status, "timeout");
    }

    #[tokio::test]
    async fn command_stdout_captured() {
        let ct = CancellationToken::new();
        let r = wait(
            "echo hello_world",
            Duration::from_secs(1),
            Duration::from_secs(5),
            ct,
        )
        .await;
        assert_eq!(r.status, "success");
        assert!(r.detail.as_deref().unwrap().contains("hello_world"));
    }

    #[tokio::test]
    async fn command_cancellation() {
        let ct = CancellationToken::new();
        let ct2 = ct.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            ct2.cancel();
        });

        // Use "false" (exits immediately with non-zero) so the loop reaches
        // the cancellation check during the inter-retry sleep.
        let r = wait(
            "false",
            Duration::from_secs(60),
            Duration::from_secs(300),
            ct,
        )
        .await;
        assert_eq!(r.status, "error");
        assert!(r.detail.as_deref().unwrap().contains("cancelled"));
    }
}
