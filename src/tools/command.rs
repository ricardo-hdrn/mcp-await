use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(
    command: &str,
    interval: Duration,
    timeout: Duration,
    abort_pattern: Option<&str>,
    ct: CancellationToken,
) -> WaitResult {
    let start = Instant::now();
    // Tracks last failure output for context on timeout; initial value used if
    // the very first iteration hits the timeout check before running the command.
    let mut last_output = String::from("(no output)");

    loop {
        if start.elapsed() >= timeout {
            return WaitResult::timeout(
                start.elapsed(),
                Some(format!(
                    "Command did not exit 0 within timeout. Last: {}",
                    last_output
                )),
            );
        }

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
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let code = output.status.code().unwrap_or(-1);
                last_output = if !stderr.is_empty() {
                    format!("exit {}: {}", code, stderr)
                } else if !stdout.is_empty() {
                    format!("exit {}: {}", code, stdout)
                } else {
                    format!("exit {}", code)
                };

                if let Some(pattern) = abort_pattern {
                    let combined = format!("{}\n{}", stdout, stderr);
                    if combined.contains(pattern) {
                        return WaitResult::error(
                            start.elapsed(),
                            Some(format!(
                                "Abort pattern '{}' matched: {}",
                                pattern, last_output
                            )),
                        );
                    }
                }
            }
            Err(e) => {
                return WaitResult::error(
                    start.elapsed(),
                    Some(format!("Failed to execute command: {}", e)),
                );
            }
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
        let r = wait(
            "true",
            Duration::from_secs(1),
            Duration::from_secs(5),
            None,
            ct,
        )
        .await;
        assert_eq!(r.status, "success");
        assert!(r.elapsed_seconds < 1.0);
    }

    #[tokio::test]
    async fn command_timeout_on_failure() {
        let ct = CancellationToken::new();
        let r = wait(
            "false",
            Duration::from_secs(1),
            Duration::from_secs(2),
            None,
            ct,
        )
        .await;
        assert_eq!(r.status, "timeout");
        assert!(r.detail.as_deref().unwrap().contains("Last: exit 1"));
    }

    #[tokio::test]
    async fn command_timeout_includes_last_stderr() {
        let ct = CancellationToken::new();
        let r = wait(
            "echo 'something went wrong' >&2; exit 1",
            Duration::from_secs(1),
            Duration::from_secs(2),
            None,
            ct,
        )
        .await;
        assert_eq!(r.status, "timeout");
        let detail = r.detail.as_deref().unwrap();
        assert!(detail.contains("something went wrong"), "got: {}", detail);
    }

    #[tokio::test]
    async fn command_stdout_captured() {
        let ct = CancellationToken::new();
        let r = wait(
            "echo hello_world",
            Duration::from_secs(1),
            Duration::from_secs(5),
            None,
            ct,
        )
        .await;
        assert_eq!(r.status, "success");
        assert!(r.detail.as_deref().unwrap().contains("hello_world"));
    }

    #[tokio::test]
    async fn command_abort_pattern_triggers() {
        let ct = CancellationToken::new();
        let r = wait(
            "echo 'status: failed'; exit 1",
            Duration::from_secs(1),
            Duration::from_secs(30),
            Some("failed"),
            ct,
        )
        .await;
        assert_eq!(r.status, "error");
        let detail = r.detail.as_deref().unwrap();
        assert!(detail.contains("Abort pattern"), "got: {}", detail);
        assert!(detail.contains("failed"), "got: {}", detail);
    }

    #[tokio::test]
    async fn command_abort_pattern_no_match_continues() {
        let ct = CancellationToken::new();
        let r = wait(
            "echo ok",
            Duration::from_secs(1),
            Duration::from_secs(5),
            Some("fatal"),
            ct,
        )
        .await;
        assert_eq!(r.status, "success");
    }

    #[tokio::test]
    async fn command_abort_pattern_matches_stderr() {
        let ct = CancellationToken::new();
        let r = wait(
            "echo 'FATAL: pipeline broken' >&2; exit 1",
            Duration::from_secs(1),
            Duration::from_secs(30),
            Some("FATAL"),
            ct,
        )
        .await;
        assert_eq!(r.status, "error");
        let detail = r.detail.as_deref().unwrap();
        assert!(detail.contains("FATAL"), "got: {}", detail);
    }

    #[tokio::test]
    async fn command_cancellation() {
        let ct = CancellationToken::new();
        let ct2 = ct.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            ct2.cancel();
        });

        let r = wait(
            "false",
            Duration::from_secs(60),
            Duration::from_secs(300),
            None,
            ct,
        )
        .await;
        assert_eq!(r.status, "error");
        assert!(r.detail.as_deref().unwrap().contains("cancelled"));
    }
}
