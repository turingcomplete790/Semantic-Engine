//! FR-016: the plugin never echoes the session cookie, reports failures as structured envelopes,
//! and contains no code that listens on a network socket.

mod common;
use std::io::Write;
use std::process::{Command, Stdio};

const COOKIE: &str = "cookie-secret";

/// Runs the real binary and returns (stdout, stderr).
fn run_binary(stdin: &[u8]) -> (String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-engine"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn cookie_never_appears_in_output_or_logs() {
    // Unreachable Stash exercises every error path that logs.
    for op in [
        "ping",
        "status",
        "getServerId",
        "generateServerId",
        "nope",
        "",
    ] {
        let (stdout, stderr) = run_binary(&common::input(common::dead_port(), op));
        assert!(!stdout.contains(COOKIE), "{op}: stdout leaks the cookie");
        assert!(!stderr.contains(COOKIE), "{op}: stderr leaks the cookie");
    }
}

#[test]
fn failures_are_structured_envelopes() {
    let (stdout, _) = run_binary(&common::input(common::dead_port(), "status"));
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        v["error"].is_null(),
        "Stash-level error must stay null: {v}"
    );
    assert_eq!(v["output"]["error"]["code"], "STASH_UNREACHABLE");

    let (port, _rx) = common::fake_stash(vec![r#"{"data":{"version":{"version":"v0.29.0"}}}"#]);
    let (stdout, _) = run_binary(&common::input(port, "status"));
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["output"]["error"]["code"], "STASH_TOO_OLD");
}

#[test]
fn shipped_code_opens_no_listening_socket() {
    // Principle IV / FR-016: the engine is reached only through Stash. Scan the source of every
    // crate (tests excluded, they run fake servers) for listening APIs.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let forbidden = ["TcpListener", "UdpSocket", "UnixListener", "bind("];
    let mut stack = vec![];
    for krate in std::fs::read_dir(&root).unwrap().flatten() {
        let src = krate.path().join("src");
        if src.is_dir() {
            stack.push(src);
        }
    }
    let mut scanned = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                scanned += 1;
                let text = std::fs::read_to_string(&p).unwrap();
                for f in forbidden {
                    assert!(!text.contains(f), "{} uses {f}", p.display());
                }
            }
        }
    }
    assert!(
        scanned >= 10,
        "expected to scan the crates' sources, scanned {scanned}"
    );
}
