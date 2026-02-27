use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};
use tokio_util::sync::CancellationToken;

use super::WaitResult;

pub async fn wait(path: &str, event: &str, timeout: Duration, ct: CancellationToken) -> WaitResult {
    let start = Instant::now();
    let target = PathBuf::from(path);

    if !matches!(event, "create" | "modify" | "delete") {
        return WaitResult::error(
            start.elapsed(),
            Some(format!(
                "Invalid event type '{}'. Must be create, modify, or delete",
                event
            )),
        );
    }

    // Quick checks for already-satisfied conditions
    if event == "create" && target.exists() {
        return WaitResult::success(start.elapsed(), Some(format!("{} already exists", path)));
    }
    if event == "delete" && !target.exists() {
        return WaitResult::success(start.elapsed(), Some(format!("{} already absent", path)));
    }

    // Watch the parent directory
    let watch_dir = target
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    if !watch_dir.exists() {
        return WaitResult::error(
            start.elapsed(),
            Some(format!(
                "Watch directory {} does not exist",
                watch_dir.display()
            )),
        );
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let mut watcher =
        match notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(evt) = res {
                let _ = tx.send(evt);
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                return WaitResult::error(
                    start.elapsed(),
                    Some(format!("Failed to create file watcher: {}", e)),
                )
            }
        };

    if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::NonRecursive) {
        return WaitResult::error(
            start.elapsed(),
            Some(format!("Failed to watch {}: {}", watch_dir.display(), e)),
        );
    }

    // Build the canonical target path for comparison
    let target_abs = if target.is_absolute() {
        target.clone()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .join(&target)
    };

    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        tokio::select! {
            _ = ct.cancelled() => {
                return WaitResult::error(start.elapsed(), Some("cancelled".into()));
            }
            _ = tokio::time::sleep_until(deadline) => {
                return WaitResult::timeout(
                    start.elapsed(),
                    Some(format!("No {} event on {} within timeout", event, path)),
                );
            }
            Some(evt) = rx.recv() => {
                let path_matches = evt.paths.iter().any(|p| {
                    *p == target_abs || *p == target
                });

                let event_matches = match event {
                    "create" => matches!(evt.kind, EventKind::Create(_)),
                    "modify" => matches!(evt.kind, EventKind::Modify(_)),
                    "delete" => matches!(evt.kind, EventKind::Remove(_)),
                    _ => false,
                };

                if path_matches && event_matches {
                    return WaitResult::success(
                        start.elapsed(),
                        Some(format!("{} event detected on {}", event, path)),
                    );
                }
            }
        }
    }
}
