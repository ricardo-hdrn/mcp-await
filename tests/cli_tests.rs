use assert_cmd::Command;
use predicates::prelude::*;
use std::net::TcpListener;

#[allow(deprecated)]
fn cmd() -> Command {
    Command::cargo_bin("mcp-await").unwrap()
}

// --- port ---

#[test]
fn cli_port_success() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    cmd()
        .args(["port", "127.0.0.1", &port.to_string(), "--timeout", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn cli_port_timeout() {
    cmd()
        .args(["port", "127.0.0.1", "19333", "--timeout", "1"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("timeout"));
}

// --- cmd ---

#[test]
fn cli_cmd_success() {
    cmd()
        .args(["cmd", "true", "--timeout", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn cli_cmd_timeout() {
    cmd()
        .args(["cmd", "false", "--timeout", "2", "--interval", "1"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("timeout"));
}

#[test]
fn cli_cmd_timeout_shows_last_output() {
    cmd()
        .args([
            "cmd",
            "echo 'db not ready' >&2; exit 1",
            "--timeout",
            "2",
            "--interval",
            "1",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("db not ready"));
}

#[test]
fn cli_cmd_abort_pattern_triggers() {
    cmd()
        .args([
            "cmd",
            "echo 'conclusion: failed'; exit 1",
            "--timeout",
            "30",
            "--interval",
            "1",
            "--abort-pattern",
            "failed",
        ])
        .assert()
        .code(2) // error exit code
        .stdout(predicate::str::contains("Abort pattern"));
}

#[test]
fn cli_cmd_abort_pattern_stderr() {
    cmd()
        .args([
            "cmd",
            "echo 'FATAL: broken' >&2; exit 1",
            "--timeout",
            "30",
            "--interval",
            "1",
            "--abort-pattern",
            "FATAL",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("FATAL"));
}

#[test]
fn cli_cmd_abort_pattern_no_match_still_succeeds() {
    cmd()
        .args([
            "cmd",
            "echo ok",
            "--timeout",
            "5",
            "--interval",
            "1",
            "--abort-pattern",
            "fatal",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn cli_cmd_captures_stdout() {
    cmd()
        .args(["cmd", "echo hello_from_cmd", "--timeout", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello_from_cmd"));
}

// --- file ---

#[test]
fn cli_file_already_exists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.txt");
    std::fs::write(&path, "hello").unwrap();

    cmd()
        .args([
            "file",
            path.to_str().unwrap(),
            "--event",
            "create",
            "--timeout",
            "5",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn cli_file_already_absent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.txt");

    cmd()
        .args([
            "file",
            path.to_str().unwrap(),
            "--event",
            "delete",
            "--timeout",
            "5",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[test]
fn cli_file_invalid_event() {
    cmd()
        .args([
            "file",
            "/tmp/whatever",
            "--event",
            "explode",
            "--timeout",
            "1",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("error"));
}

#[test]
fn cli_file_parent_dir_missing() {
    cmd()
        .args([
            "file",
            "/nonexistent_dir_12345/file.txt",
            "--event",
            "create",
            "--timeout",
            "1",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("error"));
}

// --- pid ---

#[test]
fn cli_pid_already_exited() {
    cmd()
        .args(["pid", "4999999", "--timeout", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

#[cfg(target_os = "linux")]
#[test]
fn cli_pid_current_process_timeout() {
    // PID 1 (init) is always running on Linux
    cmd()
        .args(["pid", "1", "--timeout", "1"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("timeout"));
}

// --- url ---

#[test]
fn cli_url_timeout_no_server() {
    cmd()
        .args(["url", "http://127.0.0.1:19444/health", "--timeout", "2"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("timeout"));
}

#[test]
fn cli_url_success_with_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    // Spawn a minimal HTTP responder
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::Write;
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    cmd()
        .args([
            "url",
            &format!("http://127.0.0.1:{}/", port),
            "--timeout",
            "10",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}

// --- help / version ---

#[test]
fn cli_help() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Condition watcher"));
}

#[test]
fn cli_cmd_help_shows_abort_pattern() {
    cmd()
        .args(["cmd", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("abort-pattern"));
}
