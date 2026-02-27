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
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "--max-time", "10", url])
            .output()
            .await;

        if let Ok(output) = result {
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

        if start.elapsed() >= timeout {
            return WaitResult::timeout(
                start.elapsed(),
                Some(format!(
                    "{} did not return status {}",
                    url, expected_status
                )),
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
