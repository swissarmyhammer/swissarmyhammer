//! Fanout file watcher that broadcasts file change events to multiple handlers.
//!
//! The [`FanoutWatcher`] accepts registrations of [`WatcherHandler`] trait objects
//! and fans out every [`FileEvent`] to all handlers. It also marks affected files
//! dirty in the database so they are re-indexed.

use std::path::PathBuf;

use rusqlite::Connection;

/// Event types for file changes in the workspace.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// A new file was created.
    Created(PathBuf),
    /// An existing file was modified.
    Modified(PathBuf),
    /// A file was deleted.
    Deleted(PathBuf),
}

impl FileEvent {
    /// Returns the path associated with this event.
    pub fn path(&self) -> &PathBuf {
        match self {
            FileEvent::Created(p) | FileEvent::Modified(p) | FileEvent::Deleted(p) => p,
        }
    }
}

/// Trait for handlers that react to file changes.
///
/// Implementations must be `Send + Sync` so they can be stored
/// inside the [`FanoutWatcher`] and invoked from any thread.
pub trait WatcherHandler: Send + Sync {
    /// Called when a file event occurs.
    fn on_file_event(&self, event: &FileEvent);
}

/// Broadcasts file events to multiple handlers and marks files dirty in the DB.
///
/// On each [`notify`](Self::notify) call the watcher:
/// 1. Iterates all registered handlers and calls [`WatcherHandler::on_file_event`].
/// 2. Updates the `indexed_files` table:
///    - For `Deleted` events: deletes the row (CASCADE cleans up chunks/symbols/edges).
///    - For `Created`/`Modified` events: sets `ts_indexed = 0, lsp_indexed = 0`.
pub struct FanoutWatcher {
    handlers: Vec<Box<dyn WatcherHandler>>,
}

impl FanoutWatcher {
    /// Creates a new empty watcher with no handlers.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Registers a handler that will receive all future events.
    pub fn register_handler(&mut self, handler: Box<dyn WatcherHandler>) {
        self.handlers.push(handler);
    }

    /// Broadcasts `event` to all handlers and updates the database.
    ///
    /// - `Created` / `Modified`: sets `ts_indexed = 0, lsp_indexed = 0` on the file row.
    /// - `Deleted`: deletes the file row (CASCADE removes chunks, symbols, edges).
    ///
    /// Returns the number of database rows affected.
    pub fn notify(&self, conn: &Connection, event: &FileEvent) -> Result<usize, rusqlite::Error> {
        // Fan out to all handlers
        for handler in &self.handlers {
            handler.on_file_event(event);
        }

        // Update the database
        let path_str = event.path().to_string_lossy();
        match event {
            FileEvent::Deleted(_) => conn.execute(
                "DELETE FROM indexed_files WHERE file_path = ?1",
                [&*path_str],
            ),
            FileEvent::Created(_) | FileEvent::Modified(_) => conn.execute(
                "UPDATE indexed_files SET ts_indexed = 0, lsp_indexed = 0 WHERE file_path = ?1",
                [&*path_str],
            ),
        }
    }
}

impl Default for FanoutWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::sync::{Arc, Mutex};

    /// A mock handler that records all received events.
    struct RecordingHandler {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl WatcherHandler for RecordingHandler {
        fn on_file_event(&self, event: &FileEvent) {
            let desc = match event {
                FileEvent::Created(p) => format!("created:{}", p.display()),
                FileEvent::Modified(p) => format!("modified:{}", p.display()),
                FileEvent::Deleted(p) => format!("deleted:{}", p.display()),
            };
            self.events.lock().unwrap().push(desc);
        }
    }

    fn open_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::configure_connection(&conn).unwrap();
        db::create_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn test_fanout_broadcasts_to_two_handlers() {
        let events_a: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let events_b: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let mut watcher = FanoutWatcher::new();
        watcher.register_handler(Box::new(RecordingHandler {
            events: Arc::clone(&events_a),
        }));
        watcher.register_handler(Box::new(RecordingHandler {
            events: Arc::clone(&events_b),
        }));

        let conn = open_memory_db();
        let event = FileEvent::Modified(PathBuf::from("src/main.rs"));
        watcher.notify(&conn, &event).unwrap();

        let a = events_a.lock().unwrap();
        let b = events_b.lock().unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);
        assert_eq!(a[0], "modified:src/main.rs");
        assert_eq!(b[0], "modified:src/main.rs");
    }

    #[test]
    fn test_dirty_marking_on_file_event() {
        let conn = open_memory_db();

        // Insert a file with ts_indexed = 1
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES ('src/main.rs', X'00112233', 1024, 1000, 1, 1)",
            [],
        )
        .unwrap();

        let watcher = FanoutWatcher::new();
        let event = FileEvent::Modified(PathBuf::from("src/main.rs"));
        let rows = watcher.notify(&conn, &event).unwrap();
        assert_eq!(rows, 1);

        // Verify flags are cleared
        let (ts, lsp): (i64, i64) = conn
            .query_row(
                "SELECT ts_indexed, lsp_indexed FROM indexed_files WHERE file_path = 'src/main.rs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(ts, 0);
        assert_eq!(lsp, 0);
    }

    #[test]
    fn test_deleted_event_removes_row() {
        let conn = open_memory_db();

        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES ('src/main.rs', X'00112233', 1024, 1000)",
            [],
        )
        .unwrap();

        let watcher = FanoutWatcher::new();
        let event = FileEvent::Deleted(PathBuf::from("src/main.rs"));
        let rows = watcher.notify(&conn, &event).unwrap();
        assert_eq!(rows, 1);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE file_path = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_cascade_propagation_via_watcher() {
        let conn = open_memory_db();

        // Seed a file + chunk + symbol + edge
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES ('src/main.rs', X'00112233', 1024, 1000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES ('src/main.rs', 0, 100, 1, 10, 'fn main() {}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:src/main.rs:main', 'main', 12, 'src/main.rs', 1, 0, 10, 1)",
            [],
        )
        .unwrap();

        // Second file for edge target
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES ('src/lib.rs', X'44556677', 512, 1000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:src/lib.rs:init', 'init', 12, 'src/lib.rs', 1, 0, 5, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, from_ranges)
             VALUES ('lsp:src/main.rs:main', 'lsp:src/lib.rs:init', 'src/main.rs', 'src/lib.rs', '[]')",
            [],
        )
        .unwrap();

        // Delete via watcher
        let watcher = FanoutWatcher::new();
        watcher
            .notify(&conn, &FileEvent::Deleted(PathBuf::from("src/main.rs")))
            .unwrap();

        // Chunks gone
        let chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(chunks, 0);

        // Symbols for main.rs gone
        let syms: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(syms, 0);

        // Edges gone (CASCADE from caller symbol deletion)
        let edges: i64 = conn
            .query_row("SELECT COUNT(*) FROM lsp_call_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edges, 0);

        // lib.rs symbol still exists
        let lib_syms: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'src/lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lib_syms, 1);
    }
}
