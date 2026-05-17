//! Integration tests for the [`CliServer`] stdio subprocess transport.
//!
//! These tests drive a *real* child process: the `cli_server_fixture` binary
//! built from this crate is a genuine `rmcp` stdio MCP server. Each test
//! spawns it through [`CliServer`], exercises the transport, and asserts the
//! observable result — a tool round-trip in one case, subprocess termination
//! in the other.
//!
//! [`CliServer`]: swissarmyhammer_plugin::CliServer

use std::time::Duration;

use serde_json::json;
use swissarmyhammer_plugin::{CallerId, CliServer, McpServer};

/// A generous upper bound on any single subprocess interaction.
///
/// Every await in these tests is wrapped in this timeout: a real subprocess
/// that hangs must fail the test fast rather than blocking CI indefinitely.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Path to the fixture stdio MCP server binary.
///
/// Cargo sets `CARGO_BIN_EXE_<name>` for every binary target when it builds
/// the crate's integration tests, so the test always points at the freshly
/// built fixture.
fn fixture_binary() -> String {
    env!("CARGO_BIN_EXE_cli_server_fixture").to_string()
}

/// Connects a [`CliServer`] to the fixture binary, failing fast on a hang.
async fn connect_fixture() -> CliServer {
    tokio::time::timeout(
        TIMEOUT,
        CliServer::connect(vec![fixture_binary()], None, None),
    )
    .await
    .expect("connecting to the fixture subprocess should not hang")
    .expect("connecting to a real stdio MCP server should succeed")
}

/// Returns whether a process with `pid` currently exists.
///
/// Sends signal `0` via `kill(2)`, which performs the usual permission and
/// existence checks without delivering a signal: success means the process is
/// alive, `ESRCH` means it is gone.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: `kill` with signal 0 only probes for the process; it never
    // mutates process state and cannot violate any Rust invariant.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[tokio::test]
async fn invoke_round_trips_a_tools_call_over_the_subprocess() {
    let server = connect_fixture().await;

    let names: Vec<String> = server
        .tools()
        .into_iter()
        .map(|tool| tool.name().to_string())
        .collect();
    assert!(
        names.contains(&"echo".to_string()),
        "the subprocess's tools/list should surface the echo tool, got {names:?}"
    );

    let result = tokio::time::timeout(
        TIMEOUT,
        server.invoke(
            CallerId::HostInternal,
            "echo",
            json!({ "message": "hello over stdio" }),
        ),
    )
    .await
    .expect("a tools/call against the subprocess should not hang")
    .expect("invoking the echo tool on the subprocess should succeed");

    let rendered = serde_json::to_string(&result).expect("a tools/call result is serializable");
    assert!(
        rendered.contains("hello over stdio"),
        "the echoed payload should round-trip back over stdio, got {rendered}"
    );
}

#[tokio::test]
async fn unknown_tool_yields_unknown_tool_error() {
    let server = connect_fixture().await;

    let err = tokio::time::timeout(
        TIMEOUT,
        server.invoke(CallerId::HostInternal, "no-such-tool", json!({})),
    )
    .await
    .expect("invoking a missing tool should not hang")
    .expect_err("invoking a tool the subprocess does not expose should fail");

    assert!(
        matches!(err, swissarmyhammer_plugin::Error::UnknownTool),
        "a tool absent from the subprocess's tools/list should map to UnknownTool, got {err:?}"
    );
}

/// Sends `SIGKILL` to `pid`, panicking if the signal cannot be delivered.
///
/// Used to crash the fixture subprocess out from under a live [`CliServer`]
/// so the transport closes the way it would on a genuine subprocess crash.
#[cfg(unix)]
fn kill_process(pid: u32) {
    // SAFETY: `kill` only signals an existing process; it touches no Rust
    // state and cannot violate any invariant.
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    assert_eq!(
        rc, 0,
        "sending SIGKILL to the fixture subprocess should succeed"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn invoke_after_subprocess_crash_yields_server_unavailable() {
    let server = connect_fixture().await;
    let pid = server
        .child_pid()
        .expect("a freshly spawned subprocess should report a PID");

    // `echo` is in the subprocess's tools/list, so it is in the CliServer's
    // cached tool list. Invoking it cannot short-circuit on the local cache
    // guard — every invoke below reaches the wire and `map_service_error`.
    assert!(
        server.tools().iter().any(|tool| tool.name() == "echo"),
        "the fixture must expose echo so the crash-path invoke targets a known tool"
    );

    // Crash the subprocess directly. After SIGKILL the child's stdio pipes
    // close and the rmcp Peer reports the connection as closed.
    kill_process(pid);

    // The transport detects the closed connection asynchronously, so the very
    // first invoke after the kill may still race a not-yet-closed peer. Poll,
    // bounded, until an invoke fails — then assert *how* it failed. The cache
    // guard never fires here (echo stays cached), so every iteration exercises
    // `call_tool` -> `map_service_error`.
    let mut last_ok = true;
    let mut crash_error = None;
    for _ in 0..200 {
        let outcome = tokio::time::timeout(
            TIMEOUT,
            server.invoke(
                CallerId::HostInternal,
                "echo",
                json!({ "message": "after the crash" }),
            ),
        )
        .await
        .expect("an invoke against a crashed subprocess must not hang");

        match outcome {
            Ok(_) => {
                last_ok = true;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err) => {
                last_ok = false;
                crash_error = Some(err);
                break;
            }
        }
    }

    assert!(
        !last_ok,
        "invoking a known tool on a crashed subprocess should eventually fail, not keep succeeding"
    );
    let crash_error = crash_error.expect("the crash-path invoke should have produced an error");
    assert!(
        matches!(crash_error, swissarmyhammer_plugin::Error::ServerUnavailable),
        "a crashed subprocess should surface ServerUnavailable from map_service_error, got {crash_error:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn dropping_the_server_terminates_the_child_process() {
    let server = connect_fixture().await;
    let pid = server
        .child_pid()
        .expect("a freshly spawned subprocess should report a PID");
    assert!(
        process_alive(pid),
        "the fixture subprocess should be alive while the CliServer is held"
    );

    drop(server);

    // The rmcp child-process transport kills the child asynchronously on drop.
    // Poll, bounded, for the process to disappear so a leak fails fast.
    let mut alive = true;
    for _ in 0..200 {
        if !process_alive(pid) {
            alive = false;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        !alive,
        "dropping the CliServer should terminate the child process (pid {pid} still alive)"
    );
}
