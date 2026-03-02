pub mod command;
pub mod docker;
pub mod file;
pub mod ghrun;
pub mod pid;
pub mod port;
pub mod url;

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router, RoleServer, ServerHandler,
};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

const DEFAULT_TIMEOUT: u64 = 300;

// --- Common result type ---

#[derive(Clone, Debug, serde::Serialize)]
pub struct WaitResult {
    pub status: String,
    pub elapsed_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl WaitResult {
    pub fn success(elapsed: Duration, detail: Option<String>) -> Self {
        Self {
            status: "success".into(),
            elapsed_seconds: elapsed.as_secs_f64(),
            detail,
        }
    }

    pub fn timeout(elapsed: Duration, detail: Option<String>) -> Self {
        Self {
            status: "timeout".into(),
            elapsed_seconds: elapsed.as_secs_f64(),
            detail,
        }
    }

    pub fn error(elapsed: Duration, detail: Option<String>) -> Self {
        Self {
            status: "error".into(),
            elapsed_seconds: elapsed.as_secs_f64(),
            detail,
        }
    }

    pub fn into_call_tool_result(self) -> CallToolResult {
        let json = serde_json::to_string(&self).unwrap();
        if self.status == "error" {
            CallToolResult::error(vec![Content::text(json)])
        } else {
            CallToolResult::success(vec![Content::text(json)])
        }
    }
}

// --- Watch state ---

#[derive(Clone, Debug, serde::Serialize)]
pub struct Watch {
    pub id: String,
    pub resource_uri: String,
    pub tool_name: String,
    pub status: WatchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<WaitResult>,
    #[serde(skip)]
    pub cancel: CancellationToken,
}

#[derive(Clone, Debug, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    Watching,
    Fulfilled,
    Timeout,
    Error,
    Cancelled,
}

// --- Parameter types ---

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PortParams {
    #[schemars(description = "Hostname or IP address to connect to")]
    pub host: String,
    #[schemars(description = "TCP port number")]
    pub port: u16,
    #[schemars(description = "Timeout in seconds (default: 300)")]
    pub timeout_seconds: Option<u64>,
    #[schemars(
        description = "If false, return immediately and push a notification when done (default: true)"
    )]
    pub blocking: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FileParams {
    #[schemars(description = "File path to watch")]
    pub path: String,
    #[schemars(description = "Event to wait for: create, modify, or delete")]
    pub event: String,
    #[schemars(description = "Timeout in seconds (default: 300)")]
    pub timeout_seconds: Option<u64>,
    #[schemars(
        description = "If false, return immediately and push a notification when done (default: true)"
    )]
    pub blocking: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UrlParams {
    #[schemars(description = "URL to poll")]
    pub url: String,
    #[schemars(description = "Expected HTTP status code (default: 200)")]
    pub expected_status: Option<u16>,
    #[schemars(description = "Timeout in seconds (default: 300)")]
    pub timeout_seconds: Option<u64>,
    #[schemars(
        description = "If false, return immediately and push a notification when done (default: true)"
    )]
    pub blocking: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PidParams {
    #[schemars(description = "Process ID to wait for")]
    pub pid: u32,
    #[schemars(description = "Timeout in seconds (default: 300)")]
    pub timeout_seconds: Option<u64>,
    #[schemars(
        description = "If false, return immediately and push a notification when done (default: true)"
    )]
    pub blocking: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DockerParams {
    #[schemars(description = "Docker container name or ID")]
    pub container: String,
    #[schemars(description = "Timeout in seconds (default: 300)")]
    pub timeout_seconds: Option<u64>,
    #[schemars(
        description = "If false, return immediately and push a notification when done (default: true)"
    )]
    pub blocking: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GhRunParams {
    #[schemars(description = "GitHub Actions workflow run ID")]
    pub run_id: String,
    #[schemars(description = "Repository in owner/repo format (uses current repo if omitted)")]
    pub repo: Option<String>,
    #[schemars(description = "Timeout in seconds (default: 300)")]
    pub timeout_seconds: Option<u64>,
    #[schemars(
        description = "If false, return immediately and push a notification when done (default: true)"
    )]
    pub blocking: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CommandParams {
    #[schemars(description = "Shell command to run (via sh -c)")]
    pub command: String,
    #[schemars(description = "Interval between retries in seconds (default: 5)")]
    pub interval_seconds: Option<u64>,
    #[schemars(description = "Timeout in seconds (default: 300)")]
    pub timeout_seconds: Option<u64>,
    #[schemars(
        description = "If false, return immediately and push a notification when done (default: true)"
    )]
    pub blocking: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CancelWatchParams {
    #[schemars(description = "The watch_id to cancel")]
    pub watch_id: String,
}

// --- Server ---

#[derive(Clone)]
pub struct NotifyServer {
    tool_router: ToolRouter<Self>,
    watches: Arc<RwLock<HashMap<String, Watch>>>,
    peer: Arc<RwLock<Option<rmcp::service::Peer<RoleServer>>>>,
    next_id: Arc<AtomicU64>,
}

impl NotifyServer {
    fn generate_watch_id(&self, tool_name: &str) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("{}-{}", tool_name, n)
    }

    async fn save_peer(&self, peer: &rmcp::service::Peer<RoleServer>) {
        let mut p = self.peer.write().await;
        if p.is_none() {
            *p = Some(peer.clone());
        }
    }

    async fn handle_watch<F, Fut>(
        &self,
        ctx: RequestContext<RoleServer>,
        blocking: bool,
        tool_name: &str,
        watch_fn: F,
    ) -> Result<CallToolResult, ErrorData>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = WaitResult> + Send + 'static,
    {
        self.save_peer(&ctx.peer).await;

        if blocking {
            let result = watch_fn(ctx.ct).await;
            return Ok(result.into_call_tool_result());
        }

        // Non-blocking: create watch, spawn background task
        let watch_id = self.generate_watch_id(tool_name);
        let resource_uri = format!("watch://{}", watch_id);
        let cancel = CancellationToken::new();

        let watch = Watch {
            id: watch_id.clone(),
            resource_uri: resource_uri.clone(),
            tool_name: tool_name.into(),
            status: WatchStatus::Watching,
            result: None,
            cancel: cancel.clone(),
        };

        self.watches.write().await.insert(watch_id.clone(), watch);

        // Spawn background task
        let watches = self.watches.clone();
        let peer_ref = self.peer.clone();
        let uri = resource_uri.clone();
        let bg_watch_id = watch_id.clone();

        tokio::spawn(async move {
            let result = watch_fn(cancel).await;

            let status = match result.status.as_str() {
                "success" => WatchStatus::Fulfilled,
                "timeout" => WatchStatus::Timeout,
                _ => WatchStatus::Error,
            };

            // Update watch state
            {
                let mut w = watches.write().await;
                if let Some(watch) = w.get_mut(&bg_watch_id) {
                    if watch.status == WatchStatus::Watching {
                        watch.status = status;
                        watch.result = Some(result);
                    }
                }
            }

            // Push notification
            if let Some(peer) = peer_ref.read().await.as_ref() {
                let _ = peer
                    .notify_resource_updated(ResourceUpdatedNotificationParam { uri })
                    .await;
            }
        });

        // Return immediately with watch info
        let response = serde_json::json!({
            "watch_id": watch_id,
            "resource": resource_uri,
            "status": "watching",
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&response).unwrap(),
        )]))
    }
}

#[tool_router]
impl NotifyServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            watches: Arc::new(RwLock::new(HashMap::new())),
            peer: Arc::new(RwLock::new(None)),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    #[tool(
        description = "Wait until a TCP port accepts connections. Use blocking: false to return immediately and get notified via resource update."
    )]
    async fn wait_for_port(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<PortParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT));
        let host = p.host.clone();
        let port_num = p.port;
        self.handle_watch(
            ctx,
            p.blocking.unwrap_or(true),
            "port",
            move |ct| async move { port::wait(&host, port_num, timeout, ct).await },
        )
        .await
    }

    #[tool(
        description = "Wait until a filesystem event (create, modify, or delete) occurs on a path. Uses OS-native file watching (inotify). Use blocking: false for async notification."
    )]
    async fn wait_for_file(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<FileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT));
        let path = p.path.clone();
        let event = p.event.clone();
        self.handle_watch(
            ctx,
            p.blocking.unwrap_or(true),
            "file",
            move |ct| async move { file::wait(&path, &event, timeout, ct).await },
        )
        .await
    }

    #[tool(
        description = "Wait until a URL returns an expected HTTP status code. Polls with curl every 2s. Use blocking: false for async notification."
    )]
    async fn wait_for_url(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<UrlParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT));
        let url_str = p.url.clone();
        let expected = p.expected_status.unwrap_or(200);
        self.handle_watch(
            ctx,
            p.blocking.unwrap_or(true),
            "url",
            move |ct| async move { url::wait(&url_str, expected, timeout, ct).await },
        )
        .await
    }

    #[tool(
        description = "Wait until a process exits. Monitors /proc/<pid> on Linux. Use blocking: false for async notification."
    )]
    async fn wait_for_pid(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<PidParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT));
        let pid_num = p.pid;
        self.handle_watch(
            ctx,
            p.blocking.unwrap_or(true),
            "pid",
            move |ct| async move { pid::wait(pid_num, timeout, ct).await },
        )
        .await
    }

    #[tool(
        description = "Wait until a Docker container exits. Runs `docker wait` under the hood. Use blocking: false for async notification."
    )]
    async fn wait_for_docker(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<DockerParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT));
        let container = p.container.clone();
        self.handle_watch(
            ctx,
            p.blocking.unwrap_or(true),
            "docker",
            move |ct| async move { docker::wait(&container, timeout, ct).await },
        )
        .await
    }

    #[tool(
        description = "Wait until a GitHub Actions workflow run completes. Runs `gh run watch` under the hood. Use blocking: false for async notification."
    )]
    async fn wait_for_gh_run(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GhRunParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT));
        let run_id = p.run_id.clone();
        let repo = p.repo.clone();
        self.handle_watch(
            ctx,
            p.blocking.unwrap_or(true),
            "ghrun",
            move |ct| async move { ghrun::wait(&run_id, repo.as_deref(), timeout, ct).await },
        )
        .await
    }

    #[tool(
        description = "Wait until a shell command exits with code 0. Re-runs at the specified interval. Use blocking: false for async notification."
    )]
    async fn wait_for_command(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CommandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let timeout = Duration::from_secs(p.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT));
        let interval = Duration::from_secs(p.interval_seconds.unwrap_or(5));
        let cmd = p.command.clone();
        self.handle_watch(
            ctx,
            p.blocking.unwrap_or(true),
            "command",
            move |ct| async move { command::wait(&cmd, interval, timeout, ct).await },
        )
        .await
    }

    #[tool(description = "Cancel an active non-blocking watch by its watch_id.")]
    async fn cancel_watch(
        &self,
        _ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CancelWatchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut watches = self.watches.write().await;
        if let Some(watch) = watches.get_mut(&p.watch_id) {
            if watch.status == WatchStatus::Watching {
                watch.cancel.cancel();
                watch.status = WatchStatus::Cancelled;
                watch.result = Some(WaitResult::error(
                    Duration::ZERO,
                    Some("Cancelled by user".into()),
                ));
                let response = serde_json::json!({
                    "watch_id": p.watch_id,
                    "status": "cancelled",
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&response).unwrap(),
                )]))
            } else {
                let response = serde_json::json!({
                    "watch_id": p.watch_id,
                    "status": watch.status,
                    "message": "Watch already completed",
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&response).unwrap(),
                )]))
            }
        } else {
            Err(ErrorData::invalid_params(
                format!("No watch found with id '{}'", p.watch_id),
                None,
            ))
        }
    }
}

#[tool_handler]
impl ServerHandler for NotifyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "mcp-await".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            instructions: Some(
                "Condition watcher for AI CLI assistants. \
                 PREFER these tools over shell workarounds (sleep loops, polling scripts, background curl) whenever you need to wait for a condition. \
                 Use wait_for_port instead of 'while ! nc ...; do sleep 1; done'. \
                 Use wait_for_url instead of 'curl --retry'. \
                 Use wait_for_file instead of 'inotifywait' or polling loops. \
                 Use wait_for_command instead of 'while ! cmd; do sleep N; done'. \
                 Use wait_for_pid instead of 'while kill -0 PID; do sleep 1; done'. \
                 Each tool supports blocking mode (default, holds until done) and non-blocking mode (blocking: false, returns immediately with a watch resource URI, pushes notification when done). \
                 Prefer blocking: false when there is independent work that can proceed in parallel (e.g., updating docs, pushing to other remotes, preparing next steps). \
                 Use blocking: true only when no other useful work can be done until the condition resolves."
                    .into(),
            ),
            ..Default::default()
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let watches = self.watches.read().await;
        let resources: Vec<Resource> = watches
            .values()
            .map(|w| {
                let desc = format!("Watch {} ({}): {:?}", w.id, w.tool_name, w.status);
                Resource {
                    raw: RawResource {
                        uri: w.resource_uri.clone(),
                        name: w.id.clone(),
                        title: None,
                        description: Some(desc),
                        mime_type: Some("application/json".into()),
                        size: None,
                        icons: None,
                        meta: None,
                    },
                    annotations: None,
                }
            })
            .collect();

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let uri = &request.uri;

        let watch_id = uri.strip_prefix("watch://").ok_or_else(|| {
            ErrorData::resource_not_found(format!("Invalid watch URI: {}", uri), None)
        })?;

        let watches = self.watches.read().await;
        let watch = watches.get(watch_id).ok_or_else(|| {
            ErrorData::resource_not_found(format!("No watch found: {}", watch_id), None)
        })?;

        let json = serde_json::to_string_pretty(watch).unwrap();

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: uri.clone(),
                mime_type: Some("application/json".into()),
                text: json,
                meta: None,
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_result_success_fields() {
        let r = WaitResult::success(Duration::from_secs(1), Some("ok".into()));
        assert_eq!(r.status, "success");
        assert!((r.elapsed_seconds - 1.0).abs() < 0.01);
        assert_eq!(r.detail.as_deref(), Some("ok"));
    }

    #[test]
    fn wait_result_timeout_fields() {
        let r = WaitResult::timeout(Duration::from_millis(2500), None);
        assert_eq!(r.status, "timeout");
        assert!((r.elapsed_seconds - 2.5).abs() < 0.01);
        assert!(r.detail.is_none());
    }

    #[test]
    fn wait_result_error_fields() {
        let r = WaitResult::error(Duration::ZERO, Some("boom".into()));
        assert_eq!(r.status, "error");
        assert_eq!(r.detail.as_deref(), Some("boom"));
    }

    #[test]
    fn wait_result_serialization_omits_null_detail() {
        let r = WaitResult::success(Duration::from_secs(1), None);
        let json: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert!(json.get("status").is_some());
        assert!(json.get("elapsed_seconds").is_some());
        assert!(json.get("detail").is_none());
    }

    #[test]
    fn watch_status_serialization() {
        assert_eq!(
            serde_json::to_string(&WatchStatus::Watching).unwrap(),
            "\"watching\""
        );
        assert_eq!(
            serde_json::to_string(&WatchStatus::Fulfilled).unwrap(),
            "\"fulfilled\""
        );
        assert_eq!(
            serde_json::to_string(&WatchStatus::Timeout).unwrap(),
            "\"timeout\""
        );
        assert_eq!(
            serde_json::to_string(&WatchStatus::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&WatchStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn generate_watch_id_increments() {
        let server = NotifyServer::new();
        assert_eq!(server.generate_watch_id("port"), "port-1");
        assert_eq!(server.generate_watch_id("port"), "port-2");
        assert_eq!(server.generate_watch_id("file"), "file-3");
    }

    #[test]
    fn into_call_tool_result_success_vs_error() {
        let success = WaitResult::success(Duration::ZERO, None);
        let result = success.into_call_tool_result();
        assert!(!result.is_error.unwrap_or(false));

        let error = WaitResult::error(Duration::ZERO, Some("fail".into()));
        let result = error.into_call_tool_result();
        assert!(result.is_error.unwrap_or(false));
    }
}
