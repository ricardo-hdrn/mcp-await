use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(container: &str, timeout: Duration, ct: CancellationToken) -> WaitResult {
    let start = Instant::now();

    let mut child = match Command::new("docker")
        .args(["wait", container])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return WaitResult::error(
                start.elapsed(),
                Some(format!("Failed to run docker: {}", e)),
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
                Some(format!("Container {} did not exit within timeout", container)),
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
            let exit_code = stdout_buf.trim().to_string();

            match status {
                Ok(s) if s.success() => {
                    WaitResult::success(
                        start.elapsed(),
                        Some(format!("Container {} exited with code {}", container, exit_code)),
                    )
                }
                Ok(_) => {
                    let stderr = stderr_buf.trim().to_string();
                    WaitResult::error(
                        start.elapsed(),
                        Some(format!("docker wait failed: {}", stderr)),
                    )
                }
                Err(e) => WaitResult::error(
                    start.elapsed(),
                    Some(format!("docker wait error: {}", e)),
                ),
            }
        }
    }
}
