//! YAML-persisted undo/redo stack.
//!
//! Tracks an ordered list of undo entries with a pointer separating
//! the "done" side from the "redo" side. The stack is serialized to
//! a human-readable YAML file on disk so it survives app restarts.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A single entry in the undo stack.
///
/// Each entry records the ULID of a changelog transaction, a human-readable
/// label, and an ISO-8601 timestamp.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UndoEntry {
    /// Transaction ULID that can be undone/redone.
    pub id: String,
    /// Human-readable description of what the transaction did.
    pub label: String,
    /// ISO-8601 timestamp of when the transaction occurred.
    pub timestamp: String,
}

/// YAML-persisted undo/redo stack.
///
/// `entries[0..pointer]` are undoable (most recent at `pointer - 1`).
/// `entries[pointer..]` are redoable (next redo at `pointer`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UndoStack {
    /// Maximum number of entries to keep. Oldest entries are trimmed on push.
    pub max_size: usize,
    /// Index one past the last executed entry.
    pub pointer: usize,
    /// Ordered list of undo entries.
    pub entries: Vec<UndoEntry>,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self {
            max_size: 100,
            pointer: 0,
            entries: Vec::new(),
        }
    }
}

impl UndoStack {
    /// Create an empty undo stack with the given capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            ..Default::default()
        }
    }

    /// Push a new entry onto the stack.
    ///
    /// Discards any entries on the redo side (at and after `pointer`),
    /// appends the new entry, and trims the oldest entries if the stack
    /// exceeds `max_size`.
    pub fn push(&mut self, entry: UndoEntry) {
        // Discard redo side
        self.entries.truncate(self.pointer);
        // Append new entry
        self.entries.push(entry);
        self.pointer = self.entries.len();
        // Trim oldest if over capacity
        if self.entries.len() > self.max_size {
            let excess = self.entries.len() - self.max_size;
            self.entries.drain(..excess);
            self.pointer = self.entries.len();
        }
    }

    /// Returns `true` if there is at least one entry that can be undone.
    pub fn can_undo(&self) -> bool {
        self.pointer > 0
    }

    /// Returns `true` if there is at least one entry that can be redone.
    pub fn can_redo(&self) -> bool {
        self.pointer < self.entries.len()
    }

    /// Returns the entry that would be undone next, without modifying the pointer.
    ///
    /// This is `entries[pointer - 1]` when `can_undo()` is true.
    pub fn undo_target(&self) -> Option<&UndoEntry> {
        if self.can_undo() {
            Some(&self.entries[self.pointer - 1])
        } else {
            None
        }
    }

    /// Returns the entry that would be redone next, without modifying the pointer.
    ///
    /// This is `entries[pointer]` when `can_redo()` is true.
    pub fn redo_target(&self) -> Option<&UndoEntry> {
        if self.can_redo() {
            Some(&self.entries[self.pointer])
        } else {
            None
        }
    }

    /// Load an undo stack from a YAML file on disk.
    ///
    /// Returns `Default::default()` if the file does not exist or cannot be parsed.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                serde_yaml_ng::from_str(&contents).unwrap_or_default()
            }
            Err(_) => Self::default(),
        }
    }

    /// Write the undo stack to a YAML file on disk.
    ///
    /// Returns an `io::Error` if the write fails.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let yaml = serde_yaml_ng::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper to create a test entry with a given id.
    fn entry(id: &str, label: &str) -> UndoEntry {
        UndoEntry {
            id: id.to_string(),
            label: label.to_string(),
            timestamp: "2026-03-28T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn empty_stack_cannot_undo_or_redo() {
        let stack = UndoStack::default();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert!(stack.undo_target().is_none());
        assert!(stack.redo_target().is_none());
    }

    #[test]
    fn push_and_undo_target() {
        let mut stack = UndoStack::default();
        stack.push(entry("A", "Create task"));
        stack.push(entry("B", "Update task"));

        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_target().unwrap().id, "B");
        assert_eq!(stack.pointer, 2);
    }

    #[test]
    fn redo_target_after_simulated_undo() {
        let mut stack = UndoStack::default();
        stack.push(entry("A", "Create task"));
        stack.push(entry("B", "Update task"));

        // Simulate undo by moving pointer back
        stack.pointer -= 1;

        assert!(stack.can_undo());
        assert!(stack.can_redo());
        assert_eq!(stack.undo_target().unwrap().id, "A");
        assert_eq!(stack.redo_target().unwrap().id, "B");
    }

    #[test]
    fn push_discards_redo_side() {
        let mut stack = UndoStack::default();
        stack.push(entry("A", "Create"));
        stack.push(entry("B", "Update"));
        stack.push(entry("C", "Delete"));

        // Simulate two undos
        stack.pointer = 1;
        assert_eq!(stack.redo_target().unwrap().id, "B");

        // Push a new entry — B and C should be discarded
        stack.push(entry("D", "New action"));

        assert_eq!(stack.entries.len(), 2);
        assert_eq!(stack.entries[0].id, "A");
        assert_eq!(stack.entries[1].id, "D");
        assert_eq!(stack.pointer, 2);
        assert!(!stack.can_redo());
    }

    #[test]
    fn capacity_trimming() {
        let mut stack = UndoStack::new(3);

        stack.push(entry("A", "1"));
        stack.push(entry("B", "2"));
        stack.push(entry("C", "3"));
        assert_eq!(stack.entries.len(), 3);

        // Pushing a 4th should trim the oldest
        stack.push(entry("D", "4"));
        assert_eq!(stack.entries.len(), 3);
        assert_eq!(stack.entries[0].id, "B");
        assert_eq!(stack.entries[2].id, "D");
        assert_eq!(stack.pointer, 3);
    }

    #[test]
    fn yaml_round_trip() {
        let mut stack = UndoStack::new(50);
        stack.push(entry("01ABC", "Update task title"));
        stack.push(entry("01DEF", "Move task to Done"));
        // Simulate one undo
        stack.pointer = 1;

        let yaml = serde_yaml_ng::to_string(&stack).unwrap();
        let deserialized: UndoStack = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(stack, deserialized);
        assert_eq!(deserialized.pointer, 1);
        assert_eq!(deserialized.entries.len(), 2);
        assert_eq!(deserialized.max_size, 50);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.yaml");

        let stack = UndoStack::load(&path);
        assert_eq!(stack, UndoStack::default());
    }

    #[test]
    fn save_and_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("undo_stack.yaml");

        let mut stack = UndoStack::new(100);
        stack.push(entry("01ABC", "Update task title"));
        stack.push(entry("01DEF", "Move task to Done"));

        stack.save(&path).unwrap();
        let loaded = UndoStack::load(&path);

        assert_eq!(stack, loaded);
    }

    #[test]
    fn load_corrupt_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("undo_stack.yaml");
        std::fs::write(&path, "not: valid: undo: stack: [[[").unwrap();

        let stack = UndoStack::load(&path);
        assert_eq!(stack, UndoStack::default());
    }
}
