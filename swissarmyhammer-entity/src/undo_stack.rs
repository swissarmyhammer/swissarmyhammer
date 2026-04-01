//! Persistent undo/redo stack backed by a YAML file on disk.
//!
//! The `UndoStack` tracks changelog entry IDs (or transaction IDs) with a
//! pointer-based design: entries before the pointer have been done, entries
//! at or after the pointer are available for redo. When a new entry is pushed,
//! the redo tail is discarded.
//!
//! The stack is serialized to `{root}/undo_stack.yaml` after every mutation
//! so that `cat .kanban/undo_stack.yaml` always shows the current state.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// A single entry on the undo stack.
///
/// Stores the ID used to invoke undo/redo (a ChangeEntryId or TransactionId)
/// and a human-readable label describing the operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UndoEntry {
    /// The changelog entry ID or transaction ID.
    pub id: String,
    /// Human-readable label, e.g. "create task 01ABC".
    pub label: String,
}

/// A bounded, pointer-based undo/redo stack persisted as YAML.
///
/// `pointer` always points one past the last executed entry:
/// - `entries[0..pointer)` have been done (and not undone)
/// - `entries[pointer..len)` are available for redo
///
/// When a new entry is pushed, the redo side is discarded.
/// When the stack exceeds `max_size`, the oldest entries are trimmed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoStack {
    /// Ordered list of undo entries.
    pub entries: Vec<UndoEntry>,
    /// Index one past the last executed entry.
    pub pointer: usize,
    /// Maximum number of entries to retain.
    #[serde(default = "default_max_size")]
    pub max_size: usize,
}

fn default_max_size() -> usize {
    100
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    /// Create a new empty UndoStack with the default max size (100).
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            pointer: 0,
            max_size: default_max_size(),
        }
    }

    /// Create a new empty UndoStack with a custom max size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            pointer: 0,
            max_size,
        }
    }

    /// Whether there is at least one entry that can be undone.
    pub fn can_undo(&self) -> bool {
        self.pointer > 0
    }

    /// Whether there is at least one entry that can be redone.
    pub fn can_redo(&self) -> bool {
        self.pointer < self.entries.len()
    }

    /// Return the ID of the entry that would be undone next, if any.
    pub fn undo_target(&self) -> Option<&str> {
        if self.can_undo() {
            Some(&self.entries[self.pointer - 1].id)
        } else {
            None
        }
    }

    /// Return the ID of the entry that would be redone next, if any.
    pub fn redo_target(&self) -> Option<&str> {
        if self.can_redo() {
            Some(&self.entries[self.pointer].id)
        } else {
            None
        }
    }

    /// Push a new entry onto the stack.
    ///
    /// Discards any entries on the redo side (at or after the pointer),
    /// appends the new entry, and trims the oldest entries if the stack
    /// exceeds `max_size`.
    ///
    /// **Transaction dedup**: if `id` matches the top entry's ID (i.e. the
    /// entry at `pointer - 1`), the push is skipped. This prevents multiple
    /// writes within the same transaction from creating duplicate stack entries.
    pub fn push(&mut self, id: impl Into<String>, label: impl Into<String>) {
        let id = id.into();

        // Transaction dedup: skip if same ID is already at top of done side
        if self.pointer > 0 && self.entries[self.pointer - 1].id == id {
            return;
        }

        // Discard redo side
        self.entries.truncate(self.pointer);

        self.entries.push(UndoEntry {
            id,
            label: label.into(),
        });
        self.pointer += 1;

        // Trim oldest entries if over capacity
        if self.entries.len() > self.max_size {
            let excess = self.entries.len() - self.max_size;
            self.entries.drain(0..excess);
            self.pointer -= excess;
        }
    }

    /// Record that an undo was performed — move the pointer back by one.
    ///
    /// This should be called after `EntityContext::undo()` succeeds.
    /// Does nothing if there is nothing to undo.
    pub fn record_undo(&mut self) {
        if self.can_undo() {
            self.pointer -= 1;
        }
    }

    /// Record that a redo was performed — move the pointer forward by one.
    ///
    /// This should be called after `EntityContext::redo()` succeeds.
    /// Does nothing if there is nothing to redo.
    pub fn record_redo(&mut self) {
        if self.can_redo() {
            self.pointer += 1;
        }
    }

    /// Clear all entries and reset the pointer.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pointer = 0;
    }

    /// Load an UndoStack from a YAML file.
    ///
    /// Returns a default (empty) stack if the file does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = std::fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            return Ok(Self::new());
        }
        let mut stack: Self = serde_yaml_ng::from_str(&contents)?;
        // Clamp pointer to valid range (defensive against corrupted YAML)
        stack.pointer = stack.pointer.min(stack.entries.len());
        // Trim if over capacity
        if stack.entries.len() > stack.max_size {
            let excess = stack.entries.len() - stack.max_size;
            stack.entries.drain(0..excess);
            stack.pointer = stack.pointer.saturating_sub(excess);
        }
        Ok(stack)
    }

    /// Save the UndoStack to a YAML file.
    ///
    /// Creates parent directories if needed. Writes directly (not atomic);
    /// the file is small and non-critical, so a partial write during a crash
    /// just means the stack resets to empty on next load.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml_ng::to_string(self)?;
        std::fs::write(path, yaml)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_stack_is_empty() {
        let stack = UndoStack::new();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_target(), None);
        assert_eq!(stack.redo_target(), None);
    }

    #[test]
    fn push_and_undo_redo() {
        let mut stack = UndoStack::new();
        stack.push("id1", "create task t1");
        stack.push("id2", "update task t1");

        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_target(), Some("id2"));

        stack.record_undo();
        assert!(stack.can_undo());
        assert!(stack.can_redo());
        assert_eq!(stack.undo_target(), Some("id1"));
        assert_eq!(stack.redo_target(), Some("id2"));

        stack.record_redo();
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_target(), Some("id2"));
    }

    #[test]
    fn push_discards_redo_side() {
        let mut stack = UndoStack::new();
        stack.push("id1", "op1");
        stack.push("id2", "op2");
        stack.record_undo(); // pointer at 1, redo has id2

        stack.push("id3", "op3"); // should discard id2
        assert!(!stack.can_redo());
        assert_eq!(stack.entries.len(), 2); // id1, id3
        assert_eq!(stack.entries[1].id, "id3");
    }

    #[test]
    fn transaction_dedup() {
        let mut stack = UndoStack::new();
        stack.push("tx1", "create task t1");
        stack.push("tx1", "update task t1 (same tx)");

        assert_eq!(stack.entries.len(), 1);
        assert_eq!(stack.pointer, 1);
    }

    #[test]
    fn max_size_trims_oldest() {
        let mut stack = UndoStack::with_max_size(3);
        stack.push("id1", "op1");
        stack.push("id2", "op2");
        stack.push("id3", "op3");
        stack.push("id4", "op4");

        assert_eq!(stack.entries.len(), 3);
        assert_eq!(stack.entries[0].id, "id2");
        assert_eq!(stack.pointer, 3);
    }

    #[test]
    fn round_trip_yaml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("undo_stack.yaml");

        let mut stack = UndoStack::new();
        stack.push("id1", "create task t1");
        stack.push("id2", "update task t1");
        stack.record_undo();
        stack.save(&path).unwrap();

        let loaded = UndoStack::load(&path).unwrap();
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.pointer, 1);
        assert_eq!(loaded.entries[0].id, "id1");
        assert_eq!(loaded.entries[1].id, "id2");
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.yaml");
        let stack = UndoStack::load(&path).unwrap();
        assert_eq!(stack.entries.len(), 0);
        assert_eq!(stack.pointer, 0);
    }

    #[test]
    fn record_undo_noop_when_empty() {
        let mut stack = UndoStack::new();
        stack.record_undo(); // should not panic
        assert_eq!(stack.pointer, 0);
    }

    #[test]
    fn record_redo_noop_when_nothing_to_redo() {
        let mut stack = UndoStack::new();
        stack.push("id1", "op1");
        stack.record_redo(); // should not panic, already at end
        assert_eq!(stack.pointer, 1);
    }

    #[test]
    fn clear_resets_undo_and_redo() {
        let mut stack = UndoStack::new();
        stack.push("id1", "op1");
        stack.push("id2", "op2");
        stack.push("id3", "op3");
        stack.record_undo(); // create redo state

        assert!(stack.can_undo());
        assert!(stack.can_redo());

        stack.clear();

        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_target(), None);
        assert_eq!(stack.redo_target(), None);
        assert_eq!(stack.pointer, 0);
        assert!(stack.entries.is_empty());
    }

    #[test]
    fn with_max_size_sets_capacity() {
        let stack = UndoStack::with_max_size(5);
        assert_eq!(stack.max_size, 5);
        assert!(stack.entries.is_empty());
        assert_eq!(stack.pointer, 0);
    }

    #[test]
    fn undo_target_walks_back_through_entries() {
        let mut stack = UndoStack::new();
        stack.push("a", "op-a");
        stack.push("b", "op-b");
        stack.push("c", "op-c");

        // Pointer at 3, undo target is the most recent entry
        assert_eq!(stack.undo_target(), Some("c"));
        assert_eq!(stack.redo_target(), None);

        stack.record_undo(); // pointer 2
        assert_eq!(stack.undo_target(), Some("b"));
        assert_eq!(stack.redo_target(), Some("c"));

        stack.record_undo(); // pointer 1
        assert_eq!(stack.undo_target(), Some("a"));
        assert_eq!(stack.redo_target(), Some("b"));

        stack.record_undo(); // pointer 0
        assert_eq!(stack.undo_target(), None);
        assert_eq!(stack.redo_target(), Some("a"));
    }

    #[test]
    fn redo_target_walks_forward_through_entries() {
        let mut stack = UndoStack::new();
        stack.push("a", "op-a");
        stack.push("b", "op-b");
        // Undo both
        stack.record_undo();
        stack.record_undo();
        assert_eq!(stack.pointer, 0);

        assert_eq!(stack.redo_target(), Some("a"));
        stack.record_redo(); // pointer 1
        assert_eq!(stack.redo_target(), Some("b"));
        stack.record_redo(); // pointer 2
        assert_eq!(stack.redo_target(), None);
    }

    #[test]
    fn push_trims_multiple_excess_entries() {
        let mut stack = UndoStack::with_max_size(2);
        // Push 2 to fill
        stack.push("id1", "op1");
        stack.push("id2", "op2");
        assert_eq!(stack.entries.len(), 2);

        // Push another, should trim oldest
        stack.push("id3", "op3");
        assert_eq!(stack.entries.len(), 2);
        assert_eq!(stack.entries[0].id, "id2");
        assert_eq!(stack.entries[1].id, "id3");
        assert_eq!(stack.pointer, 2);
    }

    #[test]
    fn push_trims_adjusts_pointer_correctly() {
        // With max_size=1, every push trims old entries
        let mut stack = UndoStack::with_max_size(1);
        stack.push("a", "op-a");
        assert_eq!(stack.pointer, 1);
        assert_eq!(stack.entries.len(), 1);

        stack.push("b", "op-b");
        assert_eq!(stack.pointer, 1);
        assert_eq!(stack.entries.len(), 1);
        assert_eq!(stack.entries[0].id, "b");
    }

    #[test]
    fn clear_on_empty_stack_is_noop() {
        let mut stack = UndoStack::new();
        stack.clear();
        assert_eq!(stack.pointer, 0);
        assert!(stack.entries.is_empty());
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir
            .path()
            .join("nested")
            .join("deep")
            .join("undo_stack.yaml");

        let mut stack = UndoStack::new();
        stack.push("id1", "op1");
        stack.save(&path).unwrap();

        assert!(path.exists());
        let loaded = UndoStack::load(&path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].id, "id1");
    }

    #[test]
    fn load_empty_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("undo_stack.yaml");
        std::fs::write(&path, "").unwrap();

        let stack = UndoStack::load(&path).unwrap();
        assert!(stack.entries.is_empty());
        assert_eq!(stack.pointer, 0);
    }

    #[test]
    fn load_whitespace_only_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("undo_stack.yaml");
        std::fs::write(&path, "   \n\n  ").unwrap();

        let stack = UndoStack::load(&path).unwrap();
        assert!(stack.entries.is_empty());
        assert_eq!(stack.pointer, 0);
    }

    #[test]
    fn load_clamps_out_of_range_pointer() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("undo_stack.yaml");
        // Write YAML with pointer beyond entries length
        let yaml = "entries:\n  - id: x\n    label: op-x\npointer: 999\nmax_size: 100\n";
        std::fs::write(&path, yaml).unwrap();

        let stack = UndoStack::load(&path).unwrap();
        assert_eq!(stack.entries.len(), 1);
        // Pointer should be clamped to entries.len()
        assert_eq!(stack.pointer, 1);
    }

    #[test]
    fn load_trims_entries_exceeding_max_size() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("undo_stack.yaml");
        // Write YAML with 3 entries but max_size of 2
        let yaml = "\
entries:
  - id: a
    label: op-a
  - id: b
    label: op-b
  - id: c
    label: op-c
pointer: 3
max_size: 2
";
        std::fs::write(&path, yaml).unwrap();

        let stack = UndoStack::load(&path).unwrap();
        assert_eq!(stack.entries.len(), 2);
        // Oldest entry (a) should be trimmed
        assert_eq!(stack.entries[0].id, "b");
        assert_eq!(stack.entries[1].id, "c");
        // Pointer was 3, excess is 1, so pointer = 3-1 = 2
        assert_eq!(stack.pointer, 2);
    }

    #[test]
    fn save_then_load_preserves_all_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("undo_stack.yaml");

        let mut stack = UndoStack::with_max_size(50);
        stack.push("tx-001", "create task A");
        stack.push("tx-002", "update task A");
        stack.push("tx-003", "delete task B");
        stack.record_undo(); // pointer moves back to 2

        stack.save(&path).unwrap();
        let loaded = UndoStack::load(&path).unwrap();

        assert_eq!(loaded.entries.len(), 3);
        assert_eq!(loaded.pointer, 2);
        assert_eq!(loaded.max_size, 50);
        assert_eq!(loaded.entries[0].id, "tx-001");
        assert_eq!(loaded.entries[0].label, "create task A");
        assert_eq!(loaded.entries[1].id, "tx-002");
        assert_eq!(loaded.entries[1].label, "update task A");
        assert_eq!(loaded.entries[2].id, "tx-003");
        assert_eq!(loaded.entries[2].label, "delete task B");
        assert!(loaded.can_undo());
        assert!(loaded.can_redo());
        assert_eq!(loaded.undo_target(), Some("tx-002"));
        assert_eq!(loaded.redo_target(), Some("tx-003"));
    }
}
