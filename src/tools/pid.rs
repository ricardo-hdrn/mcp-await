use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(pid: u32, timeout: Duration, ct: CancellationToken) -> WaitResult {
    let start = Instant::now();
    let proc_path = format!("/proc/{}", pid);

    if !std::path::Path::new(&proc_path).exists() {
        return WaitResult::success(
            start.elapsed(),
            Some(format!("Process {} already exited", pid)),
        );
    }

    loop {
        if !std::path::Path::new(&proc_path).exists() {
            return WaitResult::success(
                start.elapsed(),
                Some(format!("Process {} has exited", pid)),
            );
        }

        if start.elapsed() >= timeout {
            return WaitResult::timeout(
                start.elapsed(),
                Some(format!("Process {} still running", pid)),
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
#[cfg(target_os = "linux")]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pid_already_exited() {
        let ct = CancellationToken::new();
        let r = wait(u32::MAX, Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
        assert!(r.detail.as_deref().unwrap().contains("already exited"));
    }

    #[tokio::test]
    async fn pid_current_process_timeout() {
        let ct = CancellationToken::new();
        let r = wait(std::process::id(), Duration::from_secs(1), ct).await;
        assert_eq!(r.status, "timeout");
    }

    #[tokio::test]
    async fn pid_cancellation() {
        let ct = CancellationToken::new();
        let ct2 = ct.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            ct2.cancel();
        });

        let r = wait(std::process::id(), Duration::from_secs(30), ct).await;
        assert_eq!(r.status, "error");
        assert!(r.detail.as_deref().unwrap().contains("cancelled"));
    }
}
