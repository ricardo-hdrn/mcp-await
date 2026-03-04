# mcp-await demo — shell workarounds vs mcp-await
# Run: vhs demo.tape

Output docs/images/demo.gif

Set Shell "bash"
Set FontSize 15
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 30ms

# ─── ACT 1: The pain ───

Type `echo "--- The shell way ---"`
Enter
Sleep 1s

Type `while ! nc -z localhost 8080 2>/dev/null; do sleep 1; done`
Enter
Sleep 3s
Ctrl+C
Sleep 400ms

Type `until curl -sf http://localhost:8080/health; do sleep 2; done`
Enter
Sleep 3s
Ctrl+C
Sleep 400ms

Type `while [ ! -f /tmp/deploy.done ]; do sleep 1; done`
Enter
Sleep 3s
Ctrl+C
Sleep 400ms

Type `while kill -0 99999 2>/dev/null; do sleep 1; done`
Enter
Sleep 2s

Type `echo "No timeouts. No JSON. No structure. Just vibes."`
Enter
Sleep 2.5s

# ─── ACT 2: mcp-await ───

Enter
Type `echo "--- With mcp-await ---"`
Enter
Sleep 1s

Type "mcp-await cmd 'echo hello world' --timeout 5"
Enter
Sleep 2s

Type "mcp-await pid 99999 --timeout 5"
Enter
Sleep 2s

Type "mcp-await port localhost 8080 --timeout 2"
Enter
Sleep 3.5s

Enter
Type `echo "JSON output. Built-in timeouts. CLI + MCP server."`
Enter
Sleep 400ms
Type `echo "cargo install mcp-await"`
Enter
Sleep 3s
