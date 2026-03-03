# mcp-await

[![crates.io](https://img.shields.io/crates/v/mcp-await.svg)](https://crates.io/crates/mcp-await)
[![pipeline status](https://gitlab.com/ricardo.fgusmao/mcp-await/badges/develop/pipeline.svg)](https://gitlab.com/ricardo.fgusmao/mcp-await/-/pipelines)
[![license](https://img.shields.io/crates/l/mcp-await.svg)](LICENSE)

Condition watcher MCP server + CLI for AI CLI assistants (Claude Code, Codex, Cursor, etc.).

Instead of polling with `sleep` loops and `curl --retry` that waste API round-trips, call a wait tool once — it blocks until the condition is met and returns the result.

![demo](docs/images/demo.gif)

## Installation

```bash
# From crates.io
cargo install mcp-await

# From source
git clone https://gitlab.com/ricardo.fgusmao/mcp-await.git
cd mcp-await
cargo build --release
```

## Quick Start

```bash
# Wait for a service to be ready
mcp-await port localhost 8080 --timeout 30

# Wait for a file to appear
mcp-await file /tmp/deploy.lock --event create --timeout 60

# Wait for a command to succeed
mcp-await cmd "curl -sf http://localhost:8080/health" --interval 2 --timeout 30
```

## Tools

| Tool | Key Params | How it watches |
|------|------------|----------------|
| `wait_for_port` | `host`, `port` | TCP dial loop, 500ms interval |
| `wait_for_file` | `path`, `event` (create/modify/delete) | inotify via `notify` crate, no polling |
| `wait_for_url` | `url`, `expected_status` (default 200) | `curl` loop, 2s interval |
| `wait_for_pid` | `pid` | `/proc/{pid}` check, 500ms interval |
| `wait_for_docker` | `container` | `docker wait <container>` |
| `wait_for_gh_run` | `run_id`, `repo` (optional) | `gh run watch <run_id>` |
| `wait_for_command` | `command`, `interval_seconds` (default 5) | Re-run via `sh -c` until exit 0 |
| `cancel_watch` | `watch_id` | Cancels a non-blocking watch |

All tools accept `timeout_seconds` (default: 300) and `blocking` (default: true).

## CLI Usage

The binary doubles as a standalone CLI tool:

```bash
# TCP port
mcp-await port localhost 5432 --timeout 30

# File events
mcp-await file /var/log/app.log --event modify --timeout 120
mcp-await file /tmp/flag --event create --timeout 60
mcp-await file /tmp/old.pid --event delete --timeout 30

# HTTP status
mcp-await url https://api.example.com/health --status 200 --timeout 120

# Process exit
mcp-await pid 12345 --timeout 300

# Docker container exit
mcp-await docker my-container --timeout 600

# GitHub Actions run
mcp-await gh-run 12345678 --repo owner/repo --timeout 1800

# Arbitrary shell command (exit 0 = success)
mcp-await cmd "test -f /tmp/ready" --interval 2 --timeout 30
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Condition met (success) |
| 1 | Timeout |
| 2 | Error |

### Output Format

All commands output JSON:

```json
{
  "status": "success",
  "elapsed_seconds": 1.23,
  "detail": "localhost:8080 is accepting connections"
}
```

## MCP Server Setup

### Claude Code

Add to `~/.claude.json`:

```json
{
  "mcpServers": {
    "await": {
      "command": "/path/to/mcp-await"
    }
  }
}
```

The binary runs as a stdio MCP server when invoked without a subcommand (or with `mcp-await serve`).

### MCP Inspector

```bash
npx @modelcontextprotocol/inspector ./target/release/mcp-await
```

## Blocking vs Non-Blocking Mode

### Blocking (default)

The tool call holds until the condition is met, times out, or is cancelled. This is the simplest mode — the AI assistant waits for the result.

### Non-Blocking

Set `blocking: false` to get an immediate response with a `watch_id` and resource URI. The server monitors in the background and pushes a notification when done.

Flow:

1. Call `wait_for_port` with `blocking: false`
2. Get back immediately:
   ```json
   {"watch_id": "port-1", "resource": "watch://port-1", "status": "watching"}
   ```
3. Do other work while waiting
4. Receive `notifications/resources/updated` when the condition is met
5. Read `watch://port-1` for the full result

### Cancellation

Cancel any non-blocking watch with `cancel_watch`:

```json
{"watch_id": "port-1"}
```

## Resources

Non-blocking watches are exposed as MCP resources at `watch://{watch_id}`.

- `list_resources` — returns all active and completed watches
- `read_resource("watch://port-1")` — returns JSON with the watch status and result

## Development

```bash
cargo build           # debug build
cargo build --release # release build
cargo test            # run tests
cargo clippy          # lint
cargo fmt             # format
```

## License

[MIT](LICENSE)
