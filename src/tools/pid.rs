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
