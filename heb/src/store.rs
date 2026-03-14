//! SQLite WAL persistence for HEB events.
//!
//! Every publisher writes independently: open, write, close.
//! This is the most reliable path — events survive ZMQ failures and leader transitions.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::error::{HebError, Result};
use crate::header::EventHeader;

/// Initialize the events table if it doesn't exist.
pub fn init_schema(db_path: &Path) -> Result<()> {
    let conn = open_connection(db_path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (
            seq         INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp   TEXT    NOT NULL,
            session_id  TEXT    NOT NULL,
            cwd         TEXT    NOT NULL,
            category    TEXT    NOT NULL,
            event_type  TEXT    NOT NULL,
            source      TEXT    NOT NULL,
            header_json TEXT    NOT NULL,
            body        BLOB
        );
        CREATE INDEX IF NOT EXISTS idx_events_session ON events (session_id, seq);
        CREATE INDEX IF NOT EXISTS idx_events_category ON events (category, seq);
        CREATE INDEX IF NOT EXISTS idx_events_cwd ON events (cwd, seq);",
    )
    .map_err(HebError::Database)?;
    Ok(())
}

/// Log an event to SQLite. Opens, writes, closes the connection.
///
/// Returns the assigned sequence number.
pub fn log_event(db_path: &Path, header: &EventHeader, body: &[u8]) -> Result<u64> {
    let conn = open_connection(db_path)?;

    conn.execute(
        "INSERT INTO events (timestamp, session_id, cwd, category, event_type, source, header_json, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
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

    let seq = conn.last_insert_rowid() as u64;
    Ok(seq)
}

/// Replay events from SQLite, optionally filtered by category, since a given seq.
pub fn replay(
    db_path: &Path,
    since_seq: u64,
    category: Option<&str>,
) -> Result<Vec<(EventHeader, Vec<u8>)>> {
    let conn = open_connection(db_path)?;

    let sql = match category {
        Some(_) => "SELECT header_json, body FROM events WHERE seq > ?1 AND category = ?2 ORDER BY seq",
        None => "SELECT header_json, body FROM events WHERE seq > ?1 ORDER BY seq",
    };

    let mut stmt = conn.prepare(sql).map_err(HebError::Database)?;

    let extract = |row: &rusqlite::Row| -> rusqlite::Result<(String, Vec<u8>)> {
        Ok((row.get(0)?, row.get(1)?))
    };

    let raw_rows: Vec<(String, Vec<u8>)> = if let Some(cat) = category {
        stmt.query_map(params![since_seq as i64, cat], extract)
    } else {
        stmt.query_map(params![since_seq as i64], extract)
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
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
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
        let seq = log_event(&db_path, &header, body).unwrap();
        assert_eq!(seq, 1);

        let seq2 = log_event(&db_path, &header, b"second").unwrap();
        assert_eq!(seq2, 2);
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

        let events = replay(&db_path, 0, None).unwrap();
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

        let events = replay(&db_path, 0, Some("hook")).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0.category, EventCategory::Hook);
    }

    #[test]
    fn test_replay_since_seq() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("events.db");
        init_schema(&db_path).unwrap();

        let h = make_header(EventCategory::Hook, "test");
        log_event(&db_path, &h, b"1").unwrap();
        log_event(&db_path, &h, b"2").unwrap();
        log_event(&db_path, &h, b"3").unwrap();

        let events = replay(&db_path, 2, None).unwrap();
        assert_eq!(events.len(), 1);
    }
}
