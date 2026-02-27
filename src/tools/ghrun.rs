use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(
    run_id: &str,
    repo: Option<&str>,
    timeout: Duration,
    ct: CancellationToken,
) -> WaitResult {
    let start = Instant::now();

    let mut cmd = Command::new("gh");
    cmd.args(["run", "watch", run_id, "--exit-status"]);
    if let Some(r) = repo {
        cmd.args(["--repo", r]);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return WaitResult::error(
                start.elapsed(),
                Some(format!("Failed to run gh: {}", e)),
            )
        }
    };

    let mut stdout_handle = child.stdout.take();
    let mut stderr_handle = child.stderr.take();

    tokio::select! {
        _ = ct.cancelled() => {
            let _ = child.kill().await;
            WaitResult::error(start.elapsed(), Some("cancelled".into()))
        }
        _ = tokio::time::sleep(timeout) => {
            let _ = child.kill().await;
            WaitResult::timeout(
                start.elapsed(),
                Some(format!("Run {} did not complete within timeout", run_id)),
            )
        }
        status = child.wait() => {
            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();
            if let Some(ref mut h) = stdout_handle {
                let _ = h.read_to_string(&mut stdout_buf).await;
            }
            if let Some(ref mut h) = stderr_handle {
                let _ = h.read_to_string(&mut stderr_buf).await;
            }
            let detail = if stdout_buf.trim().is_empty() {
                stderr_buf.trim().to_string()
            } else {
                stdout_buf.trim().to_string()
            };

            match status {
                Ok(s) if s.success() => {
                    WaitResult::success(
                        start.elapsed(),
                        Some(format!("Run {} completed successfully. {}", run_id, detail)),
                    )
                }
                Ok(_) => {
                    // Run completed but failed — still a successful "wait"
                    WaitResult::success(
                        start.elapsed(),
                        Some(format!("Run {} completed with failure. {}", run_id, detail)),
                    )
                }
                Err(e) => WaitResult::error(
                    start.elapsed(),
                    Some(format!("gh run watch error: {}", e)),
                ),
            }
        }
    }
}
