//! Perspective changelog for independent undo/redo.
//!
//! Stores an append-only JSONL log of perspective mutations at
//! `.kanban/perspectives.jsonl`. Each line is a self-contained
//! [`PerspectiveChangeEntry`] recording the operation, full previous/current
//! snapshots, and a timestamp. This log is separate from entity changelogs
//! so that perspective undo/redo does not interfere with entity history.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use ulid::Ulid;

use crate::types::Perspective;

/// The type of mutation applied to a perspective.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerspectiveChangeOp {
    Create,
    Update,
    Delete,
}

/// A single changelog entry recording a perspective mutation.
///
/// For `Create`, only `current` is set (previous is `None`).
/// For `Update`, both `previous` and `current` are set.
/// For `Delete`, only `previous` is set (current is `None`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerspectiveChangeEntry {
    /// Unique entry identifier (ULID).
    pub id: String,
    /// When the change occurred.
    pub timestamp: DateTime<Utc>,
    /// What kind of mutation.
    pub op: PerspectiveChangeOp,
    /// The perspective ID affected.
    pub perspective_id: String,
    /// Snapshot of the perspective before the change (None for creates).
    pub previous: Option<Value>,
    /// Snapshot of the perspective after the change (None for deletes).
    pub current: Option<Value>,
}

/// Append-only JSONL changelog for perspective mutations.
///
/// Each method appends a single JSON line to the backing file. The file is
/// created on first write if it does not exist.
#[derive(Debug)]
pub struct PerspectiveChangelog {
    path: PathBuf,
}

impl PerspectiveChangelog {
    /// Create a new changelog handle pointing at the given JSONL file path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Log a perspective creation.
    ///
    /// Stores a snapshot of the newly created perspective; previous is `None`.
    pub async fn log_create(&self, perspective: &Perspective) -> std::io::Result<()> {
        let entry = PerspectiveChangeEntry {
            id: Ulid::new().to_string(),
            timestamp: Utc::now(),
            op: PerspectiveChangeOp::Create,
            perspective_id: perspective.id.clone(),
            previous: None,
            current: Some(serde_json::to_value(perspective).expect("Perspective serializes")),
        };
        self.append(&entry).await
    }

    /// Log a perspective update.
    ///
    /// Stores full snapshots of both the previous and current state so the
    /// change can be reversed without consulting external state.
    pub async fn log_update(
        &self,
        id: &str,
        previous: &Perspective,
        current: &Perspective,
    ) -> std::io::Result<()> {
        let entry = PerspectiveChangeEntry {
            id: Ulid::new().to_string(),
            timestamp: Utc::now(),
            op: PerspectiveChangeOp::Update,
            perspective_id: id.to_string(),
            previous: Some(serde_json::to_value(previous).expect("Perspective serializes")),
            current: Some(serde_json::to_value(current).expect("Perspective serializes")),
        };
        self.append(&entry).await
    }

    /// Log a perspective deletion.
    ///
    /// Stores the full snapshot of the deleted perspective; current is `None`.
    pub async fn log_delete(&self, perspective: &Perspective) -> std::io::Result<()> {
        let entry = PerspectiveChangeEntry {
            id: Ulid::new().to_string(),
            timestamp: Utc::now(),
            op: PerspectiveChangeOp::Delete,
            perspective_id: perspective.id.clone(),
            previous: Some(serde_json::to_value(perspective).expect("Perspective serializes")),
            current: None,
        };
        self.append(&entry).await
    }

    /// Read all changelog entries in order.
    ///
    /// Returns an empty vec if the file does not exist or is empty.
    /// Skips any lines that fail to parse (defensive against corruption).
    pub async fn read_all(&self) -> std::io::Result<Vec<PerspectiveChangeEntry>> {
        match fs::File::open(&self.path).await {
            Ok(file) => {
                let reader = BufReader::new(file);
                let mut lines = reader.lines();
                let mut entries = Vec::new();

                while let Some(line) = lines.next_line().await? {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(entry) = serde_json::from_str::<PerspectiveChangeEntry>(trimmed) {
                        entries.push(entry);
                    }
                }

                Ok(entries)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Append a single entry as one JSON line.
    async fn append(&self, entry: &PerspectiveChangeEntry) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        let mut json = serde_json::to_string(entry).expect("PerspectiveChangeEntry serializes");
        json.push('\n');
        file.write_all(json.as_bytes()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Perspective;

    /// Helper to build a minimal test perspective.
    fn test_perspective(id: &str, name: &str) -> Perspective {
        Perspective {
            id: id.to_string(),
            name: name.to_string(),
            view: "board".to_string(),
            fields: vec![],
            filter: None,
            group: None,
            sort: vec![],
        }
    }

    #[tokio::test]
    async fn changelog_log_create() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perspectives.jsonl");
        let changelog = PerspectiveChangelog::new(path);

        let p = test_perspective("01AAA", "Sprint View");
        changelog.log_create(&p).await.unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 1);

        let e = &entries[0];
        assert_eq!(e.op, PerspectiveChangeOp::Create);
        assert_eq!(e.perspective_id, "01AAA");
        assert!(e.previous.is_none(), "create should have no previous");
        assert!(e.current.is_some(), "create should have current");

        // Current snapshot should contain the perspective name
        let current = e.current.as_ref().unwrap();
        assert_eq!(current["name"], "Sprint View");
    }

    #[tokio::test]
    async fn changelog_log_update_has_both_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perspectives.jsonl");
        let changelog = PerspectiveChangelog::new(path);

        let prev = test_perspective("01BBB", "Old Name");
        let curr = test_perspective("01BBB", "New Name");
        changelog.log_update("01BBB", &prev, &curr).await.unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 1);

        let e = &entries[0];
        assert_eq!(e.op, PerspectiveChangeOp::Update);
        assert_eq!(e.perspective_id, "01BBB");
        assert!(e.previous.is_some(), "update should have previous");
        assert!(e.current.is_some(), "update should have current");

        let previous = e.previous.as_ref().unwrap();
        let current = e.current.as_ref().unwrap();
        assert_eq!(previous["name"], "Old Name");
        assert_eq!(current["name"], "New Name");
    }

    #[tokio::test]
    async fn changelog_log_delete_has_previous() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perspectives.jsonl");
        let changelog = PerspectiveChangelog::new(path);

        let p = test_perspective("01CCC", "Doomed");
        changelog.log_delete(&p).await.unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 1);

        let e = &entries[0];
        assert_eq!(e.op, PerspectiveChangeOp::Delete);
        assert_eq!(e.perspective_id, "01CCC");
        assert!(e.previous.is_some(), "delete should have previous");
        assert!(e.current.is_none(), "delete should have no current");

        let previous = e.previous.as_ref().unwrap();
        assert_eq!(previous["name"], "Doomed");
    }

    #[tokio::test]
    async fn changelog_read_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perspectives.jsonl");
        let changelog = PerspectiveChangelog::new(path);

        let p1 = test_perspective("01DDD", "First");
        let p2 = test_perspective("01EEE", "Second");
        let p2_updated = test_perspective("01EEE", "Second Updated");

        changelog.log_create(&p1).await.unwrap();
        changelog.log_create(&p2).await.unwrap();
        changelog.log_update("01EEE", &p2, &p2_updated).await.unwrap();
        changelog.log_delete(&p1).await.unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 4);

        // Verify ordering matches append order
        assert_eq!(entries[0].op, PerspectiveChangeOp::Create);
        assert_eq!(entries[0].perspective_id, "01DDD");
        assert_eq!(entries[1].op, PerspectiveChangeOp::Create);
        assert_eq!(entries[1].perspective_id, "01EEE");
        assert_eq!(entries[2].op, PerspectiveChangeOp::Update);
        assert_eq!(entries[2].perspective_id, "01EEE");
        assert_eq!(entries[3].op, PerspectiveChangeOp::Delete);
        assert_eq!(entries[3].perspective_id, "01DDD");
    }

    #[tokio::test]
    async fn changelog_empty_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perspectives.jsonl");

        // File does not exist
        let changelog = PerspectiveChangelog::new(path.clone());
        let entries = changelog.read_all().await.unwrap();
        assert!(entries.is_empty(), "nonexistent file should yield empty vec");

        // Create an empty file
        std::fs::write(&path, "").unwrap();
        let entries = changelog.read_all().await.unwrap();
        assert!(entries.is_empty(), "empty file should yield empty vec");
    }

    /// Verify that read_all skips blank and whitespace-only lines interspersed
    /// in the JSONL file, only returning valid entries.
    #[tokio::test]
    async fn read_all_skips_blank_and_whitespace_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perspectives.jsonl");
        let changelog = PerspectiveChangelog::new(path.clone());

        // Write one real entry via the normal API so we get valid JSON.
        let p = test_perspective("01FFF", "Kept");
        changelog.log_create(&p).await.unwrap();

        // Now read back the file, inject blank/whitespace lines, and rewrite.
        let original = std::fs::read_to_string(&path).unwrap();
        let padded = format!(
            "\n   \n\t\n{}\n\n  \n",
            original.trim_end()
        );
        std::fs::write(&path, padded).unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 1, "should skip blank/whitespace lines");
        assert_eq!(entries[0].perspective_id, "01FFF");
    }

    /// Verify that read_all propagates non-NotFound IO errors instead of
    /// swallowing them (e.g. trying to open a directory as a file).
    #[tokio::test]
    async fn read_all_propagates_non_not_found_io_error() {
        let dir = tempfile::tempdir().unwrap();
        // Point the changelog at a directory, not a file.
        let changelog = PerspectiveChangelog::new(dir.path().to_path_buf());

        let result = changelog.read_all().await;
        assert!(result.is_err(), "opening a directory should be an IO error");
        let err = result.unwrap_err();
        assert_ne!(
            err.kind(),
            std::io::ErrorKind::NotFound,
            "error should NOT be NotFound (that branch returns Ok)"
        );
    }

    /// Round-trip test: append entries via the changelog API, then read the raw
    /// JSONL file content and verify the on-disk format line by line.
    #[tokio::test]
    async fn round_trip_jsonl_format_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("perspectives.jsonl");
        let changelog = PerspectiveChangelog::new(path.clone());

        let p1 = test_perspective("01GGG", "Alpha");
        let p2 = test_perspective("01HHH", "Beta");
        changelog.log_create(&p1).await.unwrap();
        changelog.log_delete(&p2).await.unwrap();

        // Read raw file content and split into non-empty lines.
        let raw = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 2, "should have exactly 2 JSONL lines");

        // Each line must be valid JSON that deserializes to PerspectiveChangeEntry.
        let e1: PerspectiveChangeEntry = serde_json::from_str(lines[0])
            .expect("first line should be valid PerspectiveChangeEntry JSON");
        let e2: PerspectiveChangeEntry = serde_json::from_str(lines[1])
            .expect("second line should be valid PerspectiveChangeEntry JSON");

        // Verify structural fields round-trip correctly.
        assert_eq!(e1.op, PerspectiveChangeOp::Create);
        assert_eq!(e1.perspective_id, "01GGG");
        assert!(e1.previous.is_none());
        assert_eq!(e1.current.as_ref().unwrap()["name"], "Alpha");

        assert_eq!(e2.op, PerspectiveChangeOp::Delete);
        assert_eq!(e2.perspective_id, "01HHH");
        assert_eq!(e2.previous.as_ref().unwrap()["name"], "Beta");
        assert!(e2.current.is_none());

        // Verify each raw line ends with a newline in the file (JSONL convention).
        assert!(
            raw.ends_with('\n'),
            "JSONL file should end with a trailing newline"
        );
    }
}
