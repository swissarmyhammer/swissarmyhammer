//! End-to-end tests for `sah tool ...` output formatting.
//!
//! These run the compiled `sah` binary, because the contract they guard lives
//! in the real dispatch path: the tool declares that its CLI output is machine
//! read, and `main.rs` has to honor that when it prints. A unit test on the
//! formatter alone would pass while `sah tool ralph ralph check` still printed
//! YAML.
//!
//! The consumer is a Claude Code Stop hook. It pipes the hook payload in on
//! stdin and does a strict JSON parse of stdout, so the tests parse the whole
//! of stdout rather than matching a substring: a leading blank line, a log
//! line, or trailing prose would each break the hook.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Run the compiled `sah` binary in `dir` with `stdin` piped, and return stdout.
///
/// `HOME` and the working directory both point inside the throwaway directory,
/// so a run cannot read or write the developer's real `.sah/`, `.ralph/`,
/// `.kanban/` or config.
fn run_sah_in(dir: &Path, args: &[&str], stdin: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sah"))
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sah");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for sah");
    assert!(
        output.status.success(),
        "sah {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("utf8 stdout")
}

/// The ralph Stop hook payload, as Claude Code pipes it in.
const HOOK_STDIN: &str = r#"{"session_id":"probe"}"#;

/// Strict-parse `stdout` as one JSON document, the way a hook runner does.
fn parse_strict_json(op: &str, stdout: &str) -> serde_json::Value {
    assert!(
        stdout.starts_with('{'),
        "`{op}` output must begin with the JSON object — a leading blank line \
         breaks a strict parse. Got {stdout:?}"
    );
    serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("`{op}` output is not a single JSON document: {e}\n{stdout:?}"))
}

/// Every ralph operation answers a machine, so every one must leave exactly one
/// parseable JSON object on stdout.
///
/// The operations run in one directory and in this order because they share
/// `.ralph/probe.md`: `set` creates it, `check` and `get` read it, `clear`
/// removes it.
#[test]
fn every_ralph_operation_emits_strict_parseable_json() {
    let temp = tempfile::TempDir::new().expect("temp dir");

    let set = parse_strict_json(
        "ralph set",
        &run_sah_in(
            temp.path(),
            &[
                "tool",
                "ralph",
                "ralph",
                "set",
                "--instruction",
                "keep going",
                "--",
            ],
            HOOK_STDIN,
        ),
    );
    assert_eq!(set["session_id"], serde_json::json!("probe"));

    let check = parse_strict_json(
        "ralph check",
        &run_sah_in(
            temp.path(),
            &["tool", "ralph", "ralph", "check", "--"],
            HOOK_STDIN,
        ),
    );
    assert_eq!(
        check["decision"],
        serde_json::json!("block"),
        "an active instruction must block the stop"
    );

    let get = parse_strict_json(
        "ralph get",
        &run_sah_in(
            temp.path(),
            &["tool", "ralph", "ralph", "get", "--"],
            HOOK_STDIN,
        ),
    );
    assert_eq!(get["active"], serde_json::json!(true));

    let clear = parse_strict_json(
        "ralph clear",
        &run_sah_in(
            temp.path(),
            &["tool", "ralph", "ralph", "clear", "--"],
            HOOK_STDIN,
        ),
    );
    assert_eq!(clear["cleared"], serde_json::json!(true));
}

/// With no instruction stored, `check` still has to answer in JSON — this is
/// the state the hook meets on most stops.
#[test]
fn ralph_check_emits_json_when_no_instruction_is_active() {
    let temp = tempfile::TempDir::new().expect("temp dir");

    let stdout = run_sah_in(
        temp.path(),
        &["tool", "ralph", "ralph", "check", "--"],
        HOOK_STDIN,
    );

    let parsed = parse_strict_json("ralph check", &stdout);
    assert!(
        parsed.get("decision").is_none(),
        "allow is expressed by omitting `decision`, got: {parsed}"
    );
}

/// Only the machine-read tools switch to JSON. A person running a `sah tool`
/// command still gets YAML, so `kanban` output must not turn into braces.
#[test]
fn non_ralph_tool_output_stays_yaml() {
    let temp = tempfile::TempDir::new().expect("temp dir");

    let stdout = run_sah_in(temp.path(), &["tool", "kanban", "board", "get", "--"], "");

    let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&stdout)
        .unwrap_or_else(|e| panic!("kanban output is not YAML: {e}\n{stdout:?}"));
    assert!(
        parsed.get("columns").is_some(),
        "expected a YAML mapping with columns, got {stdout:?}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_err(),
        "kanban output must stay YAML, but it parsed as JSON: {stdout:?}"
    );
}
