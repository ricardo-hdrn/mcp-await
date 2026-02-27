# mcp-watch

MCP server that watches for real-world conditions and notifies AI CLI assistants (Claude Code, Codex, etc.).

Instead of polling with `timeout N && command` loops that waste API round-trips, call a watch tool once — it handles the monitoring and delivers the result.

## Two Modes

| Mode | Flow |
|------|------|
| `blocking: true` (default) | Tool call holds until condition is met, then returns result |
| `blocking: false` | Returns immediately with a `watch_id` + resource URI. Monitors in background. Pushes `notifications/resources/updated` when done. Read the resource for result. |

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

All tools return JSON: `{ "status": "success"|"timeout"|"error", "elapsed_seconds": N, "detail": "..." }`

Non-blocking mode returns: `{ "watch_id": "port-1", "resource": "watch://port-1", "status": "watching" }`

## Resources

Non-blocking watches are exposed as MCP resources at `watch://{watch_id}`. Use `read_resource` to check current status, or wait for the push notification.

## Build

```bash
cargo build --release
```

## Configure in Claude Code

```json
// ~/.claude/settings.json
{
  "mcpServers": {
    "watch": {
      "command": "/path/to/mcp-watch"
    }
  }
}
```

## Test with MCP Inspector

```bash
npx @modelcontextprotocol/inspector ./target/release/mcp-watch
```

## License

MIT
