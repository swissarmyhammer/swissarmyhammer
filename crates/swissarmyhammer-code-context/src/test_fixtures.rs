//! Shared test helpers for the code-context crate.
//!
//! Provides canonical versions of the helper functions that were previously
//! duplicated across every test module. Import with `use crate::test_fixtures::*;`
//! from any `#[cfg(test)] mod tests` block.

use rusqlite::Connection;

use crate::db::{configure_connection, create_schema};
use crate::layered_context::SharedLspSession;
use crate::lsp_communication::LspJsonRpcClient;

// ---------------------------------------------------------------------------
// Shared LSP-session test helpers
//
// Canonical versions of the LSP mock helpers that were previously copy-pasted
// across the layered_context and ops test modules. Spawning a mock server and
// wrapping it in a session is one setup contract, so it lives here once.
// ---------------------------------------------------------------------------

/// Build a session over an absent client (`None`).
///
/// Models the daemon-not-running state: `has_live_lsp()` is false and every
/// live request degrades to `Ok(None)`.
pub fn none_session() -> SharedLspSession {
    SharedLspSession::new(std::sync::Arc::new(std::sync::Mutex::new(None)), "rust")
}

/// A spawned mock LSP child that is killed and reaped when it goes out of scope.
///
/// The mock server (see [`spawn_mock_lsp`]) blocks on `stdin.readline()` until it
/// has read exactly as many messages as it was scripted with. If the code under
/// test sends fewer messages than the script expects — which happens whenever the
/// live-LSP wire protocol changes (e.g. dropping a per-request `didClose`) — the
/// child parks on that read forever. A test that then *blocks* on the child (the
/// old `child.wait()` pattern) deadlocks, and because libtest waits for every
/// spawned test thread to report completion, one parked test hangs the entire
/// `cargo test` run indefinitely.
///
/// This guard removes that whole failure class: tests never wait on the child.
/// On drop it sends `SIGKILL` (which cannot block) and then reaps the zombie, so
/// a message-count mismatch surfaces as a normal test assertion instead of a hang,
/// and no mock process is ever leaked.
pub struct MockLsp(std::process::Child);

impl std::ops::Deref for MockLsp {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for MockLsp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for MockLsp {
    fn drop(&mut self) {
        // kill() is non-blocking; the following wait() only reaps the
        // already-terminating process, so neither call can deadlock the way a
        // bare wait() on a parked mock would.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Spawn a Python mock LSP server driven by scripted `responses`.
///
/// The child reads JSON-RPC messages from stdin and replies with the canned
/// `responses` in order. Each entry is either `null` (read a notification, send
/// no reply) or a JSON-RPC response object (read a request, reply with it).
///
/// Returns a [`MockLsp`] guard that kills and reaps the child on drop. Tests must
/// never block on the child themselves — see [`MockLsp`] for why a bare
/// `child.wait()` can hang the whole suite.
pub fn spawn_mock_lsp(responses: &[serde_json::Value]) -> MockLsp {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir for mock LSP");
    let response_file = temp_dir.path().join("mock_responses.json");
    std::fs::write(&response_file, serde_json::to_string(responses).unwrap())
        .expect("failed to write mock responses file");

    let script = "\
        import sys, json, os\n\
        def read_msg():\n\
        \tcl = None\n\
        \twhile True:\n\
        \t\tline = sys.stdin.readline()\n\
        \t\tif not line: return None\n\
        \t\tline = line.strip()\n\
        \t\tif not line: break\n\
        \t\tif line.startswith('Content-Length:'):\n\
        \t\t\tcl = int(line.split(':', 1)[1].strip())\n\
        \tif cl is None: return None\n\
        \tbody = sys.stdin.read(cl)\n\
        \treturn json.loads(body)\n\
        def send_msg(obj):\n\
        \ts = json.dumps(obj)\n\
        \tsys.stdout.write(f'Content-Length: {len(s)}\\r\\n\\r\\n{s}')\n\
        \tsys.stdout.flush()\n\
        with open(os.environ['MOCK_RESPONSE_FILE']) as f:\n\
        \tresponses = json.load(f)\n\
        for resp in responses:\n\
        \tread_msg()\n\
        \tif resp is not None:\n\
        \t\tsend_msg(resp)\n";

    // Keep the tempdir alive for the lifetime of the child process.
    std::mem::forget(temp_dir);

    let child = std::process::Command::new("python3")
        .arg("-c")
        .arg(script)
        .env("MOCK_RESPONSE_FILE", &response_file)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn mock LSP python3 process");
    MockLsp(child)
}

/// Wrap a mock LSP child process in a [`SharedLspSession`].
///
/// The session keeps documents open across requests, so the live-path tests
/// drive it the same way the production ops do: `open` once, then issue
/// requests with no per-request `didClose`.
pub fn mock_lsp_session(child: &mut std::process::Child) -> SharedLspSession {
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let client = LspJsonRpcClient::new(stdin, stdout);
    SharedLspSession::new(
        std::sync::Arc::new(std::sync::Mutex::new(Some(client))),
        "rust",
    )
}

/// Build a session over the mock child plus a temp directory holding a
/// `test.rs` source file to diagnose.
///
/// Returns the [`tempfile::TempDir`] so the caller can keep it alive and build
/// a path to the source file.
pub fn mock_session_and_file(
    child: &mut std::process::Child,
) -> (SharedLspSession, tempfile::TempDir) {
    use std::io::Write;
    let session = mock_lsp_session(child);
    let dir = tempfile::tempdir().expect("temp dir");
    let mut f = std::fs::File::create(dir.path().join("test.rs")).unwrap();
    writeln!(f, "fn main() {{}}").unwrap();
    (session, dir)
}

/// Create an in-memory test database with the full schema applied.
pub fn test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();
    conn
}

/// Insert a row into `indexed_files`.
///
/// The simple form sets `ts_indexed` and `lsp_indexed` to 0 (unindexed).
/// Pass explicit values when tests need to control indexing state.
pub fn insert_file(conn: &Connection, path: &str, ts_indexed: i32, lsp_indexed: i32) {
    conn.execute(
        "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
         VALUES (?1, X'DEADBEEF', 1024, 1000, ?2, ?3)",
        rusqlite::params![path, ts_indexed, lsp_indexed],
    )
    .unwrap();
}

/// Insert a row into `indexed_files` with default indexing flags (both 0).
pub fn insert_file_simple(conn: &Connection, path: &str) {
    insert_file(conn, path, 0, 0);
}

/// Insert an LSP symbol into `lsp_symbols`.
#[allow(clippy::too_many_arguments)]
pub fn insert_lsp_symbol(
    conn: &Connection,
    id: &str,
    name: &str,
    kind: i32,
    file_path: &str,
    start_line: i32,
    start_char: i32,
    end_line: i32,
    end_char: i32,
    detail: Option<&str>,
) {
    conn.execute(
        "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![id, name, kind, file_path, start_line, start_char, end_line, end_char, detail],
    )
    .unwrap();
}

/// Insert a tree-sitter chunk into `ts_chunks`.
pub fn insert_ts_chunk(
    conn: &Connection,
    file_path: &str,
    start_line: i32,
    end_line: i32,
    text: &str,
    symbol_path: Option<&str>,
) {
    conn.execute(
        "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text, symbol_path)
         VALUES (?1, 0, ?5, ?2, ?3, ?4, ?6)",
        rusqlite::params![file_path, start_line, end_line, text, text.len() as i64, symbol_path],
    )
    .unwrap();
}

/// Insert a call edge into `lsp_call_edges`.
pub fn insert_call_edge(
    conn: &Connection,
    caller_id: &str,
    callee_id: &str,
    caller_file: &str,
    callee_file: &str,
    source: &str,
    from_ranges: &str,
) {
    conn.execute(
        "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, source, from_ranges)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![caller_id, callee_id, caller_file, callee_file, source, from_ranges],
    )
    .unwrap();
}
