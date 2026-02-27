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
