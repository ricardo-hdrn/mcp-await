use assert_cmd::Command;
use predicates::prelude::*;
use std::net::TcpListener;

#[allow(deprecated)]
fn cmd() -> Command {
    Command::cargo_bin("mcp-await").unwrap()
}

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
fn cli_pid_already_exited() {
    cmd()
        .args(["pid", "4999999", "--timeout", "5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("success"));
}
