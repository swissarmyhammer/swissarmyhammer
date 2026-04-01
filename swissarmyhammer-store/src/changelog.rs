//! Append-only changelog stored as a JSONL file.
//!
//! Each line in the file is a JSON-serialized `ChangelogEntry`. This format
//! is append-friendly, human-readable with `jq`, and resilient to partial
//! writes (only the last line can be corrupt).

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::id::{StoredItemId, UndoEntryId};

/// The type of change recorded in a changelog entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeOp {
    /// A new item was created.
    Create,
    /// An existing item was updated.
    Update,
    /// An item was deleted.
    Delete,
}

/// A single entry in the changelog, recording one mutation to the store.
///
/// Stores unified diffs (forward and reverse patches) rather than full
/// before/after text snapshots, significantly reducing storage overhead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangelogEntry {
    /// Unique identifier for this changelog entry.
    pub id: UndoEntryId,
    /// When the change was recorded.
    pub timestamp: DateTime<Utc>,
    /// The type of change.
    pub op: ChangeOp,
    /// The item's serialized ID.
    pub item_id: StoredItemId,
    /// Unified diff that transforms old content into new content.
    pub forward_patch: String,
    /// Unified diff that transforms new content back into old content.
    pub reverse_patch: String,
    /// Optional transaction ID for grouping related changes.
    pub transaction_id: Option<String>,
}

/// Handle for an append-only JSONL changelog file.
///
/// The changelog file is created on first append. Reading a nonexistent
/// file returns an empty vector.
#[derive(Debug)]
pub struct Changelog {
    path: PathBuf,
}

impl Changelog {
    /// Create a new Changelog handle pointing to the given JSONL file.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Append a single entry as one JSON line to the changelog.
    ///
    /// Uses `write_all` for atomicity of each line write.
    pub async fn append(&self, entry: &ChangelogEntry) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        let mut line = serde_json::to_string(entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        line.push('\n');
        file.write_all(line.as_bytes()).await?;
        Ok(())
    }

    /// Read all entries from the changelog file.
    ///
    /// Skips blank lines and logs a warning for lines that fail to parse
    /// (resilient to partial writes or corruption).
    pub async fn read_all(&self) -> io::Result<Vec<ChangelogEntry>> {
        match tokio::fs::read_to_string(&self.path).await {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
            Ok(contents) => {
                let mut entries = Vec::new();
                for line in contents.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<ChangelogEntry>(line) {
                        Ok(entry) => entries.push(entry),
                        Err(e) => {
                            tracing::warn!(line = line, error = %e, "skipping corrupt changelog line");
                        }
                    }
                }
                Ok(entries)
            }
        }
    }

    /// Find a single entry by its ID, reading from the end of the file.
    ///
    /// Since undo targets are typically recent entries, scanning in reverse
    /// provides O(1) performance for the common case. Returns `None` if the
    /// file does not exist or the entry is not found.
    pub async fn find_entry(&self, id: &UndoEntryId) -> io::Result<Option<ChangelogEntry>> {
        let contents = match tokio::fs::read_to_string(&self.path).await {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
            Ok(c) => c,
        };
        // Read lines in reverse order so recent entries are found first
        for line in contents.lines().rev() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<ChangelogEntry>(line) {
                if entry.id == *id {
                    return Ok(Some(entry));
                }
            }
        }
        Ok(None)
    }

    /// Compact the changelog, keeping only entries whose IDs are in the
    /// referenced set.
    ///
    /// Rewrites the JSONL file in place, discarding unreferenced entries.
    /// This prevents unbounded growth when entries are no longer needed
    /// (e.g., trimmed from the undo stack).
    pub async fn compact(&self, referenced_ids: &HashSet<UndoEntryId>) -> io::Result<()> {
        let entries = self.read_all().await?;
        let kept: Vec<_> = entries
            .into_iter()
            .filter(|e| referenced_ids.contains(&e.id))
            .collect();

        // Rewrite the file with only referenced entries
        let mut buf = String::new();
        for entry in &kept {
            let line = serde_json::to_string(entry)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            buf.push_str(&line);
            buf.push('\n');
        }
        tokio::fs::write(&self.path, buf.as_bytes()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;

    fn make_entry(item_id: &str, op: ChangeOp) -> ChangelogEntry {
        let (forward_patch, reverse_patch) = crate::diff::create_patches("old", "new");
        ChangelogEntry {
            id: UndoEntryId::new(),
            timestamp: Utc::now(),
            op,
            item_id: StoredItemId::from(item_id),
            forward_patch,
            reverse_patch,
            transaction_id: None,
        }
    }

    #[tokio::test]
    async fn append_and_read_all_round_trip() {
        let dir = TempDir::new().unwrap();
        let changelog = Changelog::new(dir.path().join("changelog.jsonl"));

        let e1 = make_entry("task-1", ChangeOp::Create);
        let e2 = make_entry("task-2", ChangeOp::Update);
        changelog.append(&e1).await.unwrap();
        changelog.append(&e2).await.unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].item_id, StoredItemId::from("task-1"));
        assert_eq!(entries[1].item_id, StoredItemId::from("task-2"));
    }

    #[tokio::test]
    async fn read_all_nonexistent_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let changelog = Changelog::new(dir.path().join("missing.jsonl"));
        let entries = changelog.read_all().await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn read_all_skips_blank_lines() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("changelog.jsonl");
        let changelog = Changelog::new(path.clone());

        let entry = make_entry("task-1", ChangeOp::Create);
        changelog.append(&entry).await.unwrap();

        // Manually inject a blank line
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file).unwrap();
        writeln!(file, "   ").unwrap();

        let entry2 = make_entry("task-2", ChangeOp::Update);
        changelog.append(&entry2).await.unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn read_all_preserves_order() {
        let dir = TempDir::new().unwrap();
        let changelog = Changelog::new(dir.path().join("changelog.jsonl"));

        let ids: Vec<StoredItemId> = (0..5)
            .map(|i| StoredItemId::from(format!("item-{}", i)))
            .collect();
        for id in &ids {
            changelog
                .append(&make_entry(id.as_str(), ChangeOp::Create))
                .await
                .unwrap();
        }

        let entries = changelog.read_all().await.unwrap();
        let read_ids: Vec<&StoredItemId> = entries.iter().map(|e| &e.item_id).collect();
        let expected: Vec<&StoredItemId> = ids.iter().collect();
        assert_eq!(read_ids, expected);
    }

    #[tokio::test]
    async fn find_entry_by_id() {
        let dir = TempDir::new().unwrap();
        let changelog = Changelog::new(dir.path().join("changelog.jsonl"));

        let e1 = make_entry("task-1", ChangeOp::Create);
        let e2 = make_entry("task-2", ChangeOp::Update);
        let target_id = e2.id;
        changelog.append(&e1).await.unwrap();
        changelog.append(&e2).await.unwrap();

        let found = changelog.find_entry(&target_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().item_id, StoredItemId::from("task-2"));
    }

    #[tokio::test]
    async fn find_entry_not_found() {
        let dir = TempDir::new().unwrap();
        let changelog = Changelog::new(dir.path().join("changelog.jsonl"));

        let entry = make_entry("task-1", ChangeOp::Create);
        changelog.append(&entry).await.unwrap();

        let missing_id = UndoEntryId::new();
        let found = changelog.find_entry(&missing_id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn compact_keeps_only_referenced_entries() {
        let dir = TempDir::new().unwrap();
        let changelog = Changelog::new(dir.path().join("changelog.jsonl"));

        let e1 = make_entry("task-1", ChangeOp::Create);
        let e2 = make_entry("task-2", ChangeOp::Update);
        let e3 = make_entry("task-3", ChangeOp::Delete);
        let keep_id = e2.id;
        changelog.append(&e1).await.unwrap();
        changelog.append(&e2).await.unwrap();
        changelog.append(&e3).await.unwrap();

        let mut referenced = HashSet::new();
        referenced.insert(keep_id);
        changelog.compact(&referenced).await.unwrap();

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, keep_id);
        assert_eq!(entries[0].item_id, StoredItemId::from("task-2"));
    }
}
