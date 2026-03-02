# mcp-await

Condition watcher MCP server + CLI for AI CLI assistants. Rust project using rmcp SDK.

## Project Identity

- **Package name**: `mcp-await` (Cargo.toml, crates.io)
- **Directory**: `mcp-notify-me` (legacy)
- **Remote**: `git@gitlab.com:ricardo.fgusmao/mcp-await.git`
- **Branch**: `develop`
- **Binary**: `target/release/mcp-await`
- **Version**: 0.1.0

## What It Does

8 MCP tools that block (or run in background) until a condition is met:

| Tool | Watches for |
|------|------------|
| `wait_for_port` | TCP port accepting connections |
| `wait_for_file` | Filesystem event (create/modify/delete) via inotify |
| `wait_for_url` | URL returning expected HTTP status (polls with curl) |
| `wait_for_pid` | Process exit (checks /proc) |
| `wait_for_docker` | Docker container exit (`docker wait`) |
| `wait_for_gh_run` | GitHub Actions run completion (`gh run watch`) |
| `wait_for_command` | Shell command exiting 0 (retries at interval) |
| `cancel_watch` | Cancel a non-blocking watch by ID |

## Architecture

### Dual-Mode (blocking param)

Every wait tool accepts `blocking: Option<bool>` (default `true`):

- **`blocking: true`** (default) — tool call holds until condition met, timeout, or cancellation
- **`blocking: false`** — returns immediately with `watch_id` + `resource_uri`, spawns background task, pushes `notifications/resources/updated` when done

Non-blocking watches are stored in `watches: Arc<RwLock<HashMap<String, Watch>>>` and exposed as MCP resources via `list_resources`/`read_resource`.

### Same Binary, Two Modes

- **MCP server**: `mcp-await` or `mcp-await serve` — stdio JSON-RPC transport
- **CLI**: `mcp-await <subcommand>` — direct human usage, outputs pretty JSON, exit codes (0=success, 1=timeout, 2=error)

CLI subcommands: `port`, `file`, `url`, `pid`, `docker`, `gh-run`, `cmd`

### Key Types

- `WaitResult` — `{status, elapsed_seconds, detail}` (Clone, Debug, Serialize)
- `Watch` — `{id, resource_uri, tool_name, status, result, cancel}` tracks background watches
- `WatchStatus` — enum: Watching, Fulfilled, Timeout, Error, Cancelled
- `NotifyServer` — main server struct with `tool_router`, `watches`, `peer`, `next_id`

### File Layout

```
src/
  main.rs           — entry point, clap CLI, MCP server bootstrap
  tools/
    mod.rs          — NotifyServer, all param structs, tool impls, handle_watch, ServerHandler
    port.rs         — TCP connect loop (500ms interval, 1s connect timeout)
    file.rs         — inotify via notify crate, watches parent dir
    url.rs          — curl shell-out (2s interval, 10s curl timeout)
    pid.rs          — /proc/{pid} existence check (500ms interval)
    docker.rs       — docker wait subprocess
    ghrun.rs        — gh run watch --exit-status subprocess
    command.rs      — sh -c loop at configurable interval
```

## Tech Stack

- **rmcp v0.16** — official Rust MCP SDK (`modelcontextprotocol/rust-sdk`)
  - `#[tool_router]`, `#[tool]`, `#[tool_handler]` proc macros
  - `Parameters<T>` extractor with `schemars::JsonSchema` for auto schema
  - `RequestContext<RoleServer>` with `ct: CancellationToken` for cancellation
  - `Peer<RoleServer>` for push notifications (`notify_resource_updated`)
  - `ServerCapabilities::builder().enable_tools().enable_resources().build()`
  - Resources: manual `list_resources`/`read_resource` impl on `ServerHandler` (no macro for these)
- **tokio** (full) + **tokio-util** (CancellationToken)
- **notify v6** for inotify file watching
- **clap v4** derive for CLI
- **serde/serde_json** for serialization
- **tracing/tracing-subscriber** logging to stderr

## Build & Run

```bash
cargo build --release
# MCP server (stdio):
./target/release/mcp-await
# CLI:
./target/release/mcp-await port localhost 8080 --timeout 10
./target/release/mcp-await cmd "test -f /tmp/flag" --interval 2 --timeout 30
./target/release/mcp-await file /tmp/test.txt --event create --timeout 60
```

## MCP Server Config

In `~/.claude.json`:
```json
{
  "mcpServers": {
    "await": {
      "command": "/path/to/mcp-await"
    }
  }
}
```

## Known Patterns & Pitfalls

- `docker wait` and `gh run watch` use `child.wait()` + `child.stdout.take()` (not `wait_with_output()` which takes ownership and prevents `child.kill()`)
- Closures passed to `handle_watch` need `move |ct| async move { ... }` to avoid lifetime issues
- `RawResource`, `ResourceContents`, `ReadResourceResult` don't impl Default — specify all fields explicitly
- `Implementation` struct: use `..Default::default()` for extra fields
- Non-blocking: clone `watch_id` to `bg_watch_id` before moving into spawned task
- Server `instructions` field drives agent tool selection — be directive ("PREFER these tools over shell workarounds")

## What's Next / TODO

- Wire `add-mcp` as dependency for self-install subcommand
- npm registration (blocked on 2FA)
- Rename directory from `mcp-notify-me` to `mcp-await`
- Update GitLab remote to `mcp-await`
