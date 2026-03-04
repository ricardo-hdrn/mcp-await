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
        std::env::current_dir().unwrap_or_default().join(&target)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn file_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.txt");
        std::fs::write(&path, "hello").unwrap();

        let ct = CancellationToken::new();
        let r = wait(path.to_str().unwrap(), "create", Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
        assert!(r.detail.as_deref().unwrap().contains("already exists"));
    }

    #[tokio::test]
    async fn file_already_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.txt");

        let ct = CancellationToken::new();
        let r = wait(path.to_str().unwrap(), "delete", Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
        assert!(r.detail.as_deref().unwrap().contains("already absent"));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn file_create_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_file.txt");
        let path_clone = path.clone();

        let ct = CancellationToken::new();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            std::fs::write(&path_clone, "created").unwrap();
        });

        let r = wait(path.to_str().unwrap(), "create", Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn file_modify_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mod_file.txt");
        std::fs::write(&path, "initial").unwrap();
        let path_clone = path.clone();

        let ct = CancellationToken::new();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            std::fs::write(&path_clone, "modified").unwrap();
        });

        let r = wait(path.to_str().unwrap(), "modify", Duration::from_secs(5), ct).await;
        assert_eq!(r.status, "success");
    }

    #[tokio::test]
    async fn file_invalid_event() {
        let ct = CancellationToken::new();
        let r = wait("/tmp/whatever", "foobar", Duration::from_secs(1), ct).await;
        assert_eq!(r.status, "error");
        assert!(r.detail.as_deref().unwrap().contains("Invalid event type"));
    }

    #[tokio::test]
    async fn file_parent_dir_missing() {
        let ct = CancellationToken::new();
        let r = wait(
            "/no/such/dir/file.txt",
            "create",
            Duration::from_secs(1),
            ct,
        )
        .await;
        assert_eq!(r.status, "error");
        assert!(r.detail.as_deref().unwrap().contains("does not exist"));
    }
}
