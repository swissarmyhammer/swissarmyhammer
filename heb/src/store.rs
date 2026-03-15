//! SQLite WAL persistence for HEB events.
//!
//! Every publisher writes independently: open, write, close.
//! This is the most reliable path — events survive ZMQ failures and leader transitions.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::error::{HebError, Result};
use crate::header::EventHeader;

/// Initialize the events table if it doesn't exist.
///
/// This is idempotent and also called automatically by `log_event()` and `replay()`.
/// Callers may call it explicitly to fail fast on database errors at startup.
pub fn init_schema(db_path: &Path) -> Result<()> {
    let _ = open_connection(db_path)?;
    Ok(())
}

/// Log an event to SQLite. Opens, writes, closes the connection.
///
/// The event's ULID (from `header.id`) is used as the primary key.
/// Returns the ULID.
pub fn log_event(db_path: &Path, header: &EventHeader, body: &[u8]) -> Result<String> {
    let conn = open_connection(db_path)?;

    conn.execute(
        "INSERT INTO events (id, timestamp, session_id, cwd, category, event_type, source, header_json, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            header.id,
            header.timestamp.to_rfc3339(),
            header.session_id,
            header.cwd.display().to_string(),
            header.category.as_str(),
            header.event_type,
            header.source,
            serde_json::to_string(header).map_err(HebError::Serialization)?,
            body,
        ],
    )
    .map_err(HebError::Database)?;

    Ok(header.id.clone())
}

/// Replay events from SQLite, optionally filtered by category, since a given ULID.
///
/// Pass an empty string to replay from the beginning.
pub fn replay(
    db_path: &Path,
    since_id: &str,
    category: Option<&str>,
) -> Result<Vec<(EventHeader, Vec<u8>)>> {
    let conn = open_connection(db_path)?;

    let sql = match category {
        Some(_) => "SELECT header_json, body FROM events WHERE id > ?1 AND category = ?2 ORDER BY id",
        None => "SELECT header_json, body FROM events WHERE id > ?1 ORDER BY id",
    };

    let mut stmt = conn.prepare(sql).map_err(HebError::Database)?;

    let extract = |row: &rusqlite::Row| -> rusqlite::Result<(String, Vec<u8>)> {
        Ok((row.get(0)?, row.get(1)?))
    };

    let raw_rows: Vec<(String, Vec<u8>)> = if let Some(cat) = category {
        stmt.query_map(params![since_id, cat], extract)
    } else {
        stmt.query_map(params![since_id], extract)
    }
    .map_err(HebError::Database)?
    .collect::<std::result::Result<Vec<_>, _>>()
    .map_err(HebError::Database)?;

    raw_rows
        .into_iter()
        .map(|(header_json, body)| {
            let header: EventHeader =
                serde_json::from_str(&header_json).map_err(HebError::Serialization)?;
            Ok((header, body))
        })
        .collect()
}

fn open_connection(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(HebError::Io)?;
    }
    let conn = Connection::open(db_path).map_err(HebError::Database)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        CREATE TABLE IF NOT EXISTS events (
            id          TEXT PRIMARY KEY,
            timestamp   TEXT    NOT NULL,
            session_id  TEXT    NOT NULL,
            cwd         TEXT    NOT NULL,
            category    TEXT    NOT NULL,
            event_type  TEXT    NOT NULL,
            source      TEXT    NOT NULL,
            header_json TEXT    NOT NULL,
            body        BLOB
        );
        CREATE INDEX IF NOT EXISTS idx_events_session ON events (session_id, id);
        CREATE INDEX IF NOT EXISTS idx_events_category ON events (category, id);
        CREATE INDEX IF NOT EXISTS idx_events_cwd ON events (cwd, id);",
    )
    .map_err(HebError::Database)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::EventCategory;
    use tempfile::TempDir;

    fn make_header(category: EventCategory, event_type: &str) -> EventHeader {
        EventHeader::new("test-session", "/workspace", category, event_type, "test")
    }

    #[test]
    fn test_init_and_log() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("events.db");

        init_schema(&db_path).unwrap();

        let header = make_header(EventCategory::Hook, "pre_tool_use");
        let body = b"hello world";
        let id = log_event(&db_path, &header, body).unwrap();
        assert_eq!(id, header.id);

        let header2 = make_header(EventCategory::Hook, "post_tool_use");
        let id2 = log_event(&db_path, &header2, b"second").unwrap();
        assert_eq!(id2, header2.id);

        // ULIDs are lexicographically ordered
        assert!(id < id2, "second ULID should sort after first");
    }

    #[test]
    fn test_replay_all() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("events.db");
        init_schema(&db_path).unwrap();

        let h1 = make_header(EventCategory::Hook, "pre_tool_use");
        let h2 = make_header(EventCategory::Session, "start");
        log_event(&db_path, &h1, b"body1").unwrap();
        log_event(&db_path, &h2, b"body2").unwrap();

        let events = replay(&db_path, "", None).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0.event_type, "pre_tool_use");
        assert_eq!(events[1].0.event_type, "start");
    }

    #[test]
    fn test_replay_filtered() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("events.db");
        init_schema(&db_path).unwrap();

        let h1 = make_header(EventCategory::Hook, "pre_tool_use");
        let h2 = make_header(EventCategory::Session, "start");
        log_event(&db_path, &h1, b"body1").unwrap();
        log_event(&db_path, &h2, b"body2").unwrap();

        let events = replay(&db_path, "", Some("hook")).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0.category, EventCategory::Hook);
    }

    #[test]
    fn test_replay_since_id() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("events.db");
        init_schema(&db_path).unwrap();

        let h1 = make_header(EventCategory::Hook, "test");
        let h2 = make_header(EventCategory::Hook, "test");
        let h3 = make_header(EventCategory::Hook, "test");
        log_event(&db_path, &h1, b"1").unwrap();
        let id2 = log_event(&db_path, &h2, b"2").unwrap();
        log_event(&db_path, &h3, b"3").unwrap();

        let events = replay(&db_path, &id2, None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, b"3");
    }

    #[test]
    fn test_header_json_contains_correct_id() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("events.db");
        init_schema(&db_path).unwrap();

        let header = make_header(EventCategory::Hook, "test");
        let expected_id = header.id.clone();
        log_event(&db_path, &header, b"body").unwrap();

        let events = replay(&db_path, "", None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0.id, expected_id, "replayed header should have the correct ULID");
    }
}
