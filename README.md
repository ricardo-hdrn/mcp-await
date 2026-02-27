# mcp-watch

[![pipeline status](https://gitlab.com/ricardo.fgusmao/mcp-watch/badges/main/pipeline.svg)](https://gitlab.com/ricardo.fgusmao/mcp-watch/-/pipelines)

Condition watcher MCP server + CLI for AI CLI assistants (Claude Code, Codex, etc.).

Instead of polling with `timeout N && command` loops that waste API round-trips, call a watch tool once — it handles the monitoring and delivers the result.

## Installation

```bash
git clone https://gitlab.com/ricardo.fgusmao/mcp-watch.git
cd mcp-watch
cargo build --release
# Binary at: target/release/mcp-watch
```

## Quick Start

```bash
# Wait for a service to be ready
mcp-watch port localhost 8080 --timeout 30

# Wait for a file to appear
mcp-watch file /tmp/deploy.lock --event create --timeout 60

# Wait for a command to succeed
mcp-watch cmd "curl -sf http://localhost:8080/health" --interval 2 --timeout 30
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
mcp-watch port localhost 5432 --timeout 30

# File events
mcp-watch file /var/log/app.log --event modify --timeout 120
mcp-watch file /tmp/flag --event create --timeout 60
mcp-watch file /tmp/old.pid --event delete --timeout 30

# HTTP status
mcp-watch url https://api.example.com/health --status 200 --timeout 120

# Process exit
mcp-watch pid 12345 --timeout 300

# Docker container exit
mcp-watch docker my-container --timeout 600

# GitHub Actions run
mcp-watch gh-run 12345678 --repo owner/repo --timeout 1800

# Arbitrary shell command (exit 0 = success)
mcp-watch cmd "test -f /tmp/ready" --interval 2 --timeout 30
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

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "watch": {
      "command": "/path/to/mcp-watch"
    }
  }
}
```

The binary runs as a stdio MCP server when invoked without a subcommand (or with `mcp-watch serve`).

### MCP Inspector

```bash
npx @modelcontextprotocol/inspector ./target/release/mcp-watch
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
