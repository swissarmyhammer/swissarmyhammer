//! `StoreHandle` wraps any [`TrackedStore`] to provide write, delete,
//! undo, redo, changelog, and change detection.
//!
//! This is the main workhorse of the crate. It maintains an in-memory cache
//! of last-known file contents, an append-only changelog, and delegates
//! serialization to the underlying store.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::RwLock;

use crate::changelog::{ChangeOp, Changelog, ChangelogEntry};
use crate::diff;
use crate::error::{Result, StoreError};
use crate::event::ChangeEvent;
use crate::id::{StoredItemId, UndoEntryId};
use crate::store::TrackedStore;
use crate::trash;

/// A handle wrapping a [`TrackedStore`] with changelog, cache, and undo support.
///
/// All mutations go through this handle, which records changes to per-item
/// changelogs, maintains an in-memory cache for idempotency detection, and
/// supports undo/redo via patch reversal. Each item gets its own `.jsonl`
/// changelog file alongside its data file.
pub struct StoreHandle<S: TrackedStore> {
    pub(crate) store: Arc<S>,
    cache: RwLock<HashMap<String, String>>,
}

impl<S: TrackedStore> StoreHandle<S> {
    /// Create a new `StoreHandle` wrapping the given store.
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Return the path to the per-item changelog for the given item ID.
    ///
    /// The changelog lives at `{root}/{item_id}.jsonl` alongside the data
    /// file at `{root}/{item_id}.{ext}`.
    fn changelog_for(&self, item_id: &StoredItemId) -> Changelog {
        let path = self
            .store
            .root()
            .join(format!("{}.jsonl", item_id.as_str()));
        Changelog::new(path)
    }

    /// Read an item by ID from disk.
    pub async fn get(&self, id: &S::ItemId) -> Result<S::Item> {
        let id_str = id.to_string();
        let path = self.item_path(&id_str);
        let text = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => StoreError::NotFound(id_str.clone()),
                _ => StoreError::Io(e),
            })?;
        self.store.deserialize(id, &text)
    }

    /// Write an item to disk. Returns `None` if the text is unchanged (idempotent).
    ///
    /// On change, appends a changelog entry with before/after text and
    /// updates the in-memory cache. Holds the cache write lock for the
    /// entire operation to prevent TOCTOU races.
    pub async fn write(&self, item: &S::Item) -> Result<Option<UndoEntryId>> {
        let id = self.store.item_id(item);
        let id_str = id.to_string();
        let stored_id = StoredItemId::from(id_str.clone());
        let new_text = self.store.serialize(item)?;

        // Hold the cache write lock across the entire operation
        let mut cache = self.cache.write().await;

        // Read old text from cache or disk
        let old_text = if let Some(text) = cache.get(&id_str) {
            Some(text.clone())
        } else {
            self.read_text_from_disk(&id_str).await?
        };

        // Idempotent: no change
        if old_text.as_deref() == Some(new_text.as_str()) {
            return Ok(None);
        }

        let op = if old_text.is_some() {
            ChangeOp::Update
        } else {
            ChangeOp::Create
        };

        let (forward_patch, reverse_patch) =
            diff::create_patches(old_text.as_deref().unwrap_or(""), &new_text);

        let entry_id = UndoEntryId::new();
        let entry = ChangelogEntry {
            id: entry_id,
            timestamp: Utc::now(),
            op,
            item_id: stored_id.clone(),
            forward_patch,
            reverse_patch,
            transaction_id: None,
        };

        self.changelog_for(&stored_id)
            .append(&entry)
            .await
            .map_err(StoreError::Io)?;

        // Atomic write: temp file then rename
        self.atomic_write(&id_str, &new_text).await?;

        // Update cache (lock already held)
        cache.insert(id_str, new_text);

        Ok(Some(entry_id))
    }

    /// Delete an item by moving it to trash.
    ///
    /// Records the deletion in the changelog so it can be undone.
    /// Holds the cache write lock for the entire operation to prevent
    /// TOCTOU races.
    pub async fn delete(&self, id: &S::ItemId) -> Result<UndoEntryId> {
        let id_str = id.to_string();
        let stored_id = StoredItemId::from(id_str.clone());

        // Hold the cache write lock across the entire operation
        let mut cache = self.cache.write().await;

        let text = if let Some(text) = cache.get(&id_str) {
            text.clone()
        } else {
            self.read_text_from_disk(&id_str)
                .await?
                .ok_or_else(|| StoreError::NotFound(id_str.clone()))?
        };

        let (forward_patch, reverse_patch) = diff::create_patches(&text, "");

        let entry_id = UndoEntryId::new();
        let entry = ChangelogEntry {
            id: entry_id,
            timestamp: Utc::now(),
            op: ChangeOp::Delete,
            item_id: stored_id.clone(),
            forward_patch,
            reverse_patch,
            transaction_id: None,
        };

        self.changelog_for(&stored_id)
            .append(&entry)
            .await
            .map_err(StoreError::Io)?;

        // Trash the data file
        trash::trash_file(
            self.store.root(),
            &stored_id,
            self.store.extension(),
            &entry_id,
        )
        .map_err(StoreError::Io)?;

        // Trash the per-item changelog alongside the data file
        trash::trash_file(self.store.root(), &stored_id, "jsonl", &entry_id)
            .map_err(StoreError::Io)?;

        // Update cache (lock already held)
        cache.remove(&id_str);

        Ok(entry_id)
    }

    /// Undo an operation identified by its changelog entry ID.
    ///
    /// The `item_id` identifies which per-item changelog contains the entry.
    ///
    /// - Create: trashes the created file and its changelog
    /// - Update: reverts to the before text (with 3-way merge if concurrently edited)
    /// - Delete: restores the file and its changelog from trash
    pub async fn undo(&self, entry_id: &UndoEntryId, item_id: &StoredItemId) -> Result<S::Item> {
        // If the changelog is in trash (from a prior delete), restore it first
        // so we can read the entry. Ignore errors -- the file may not be trashed.
        let _ = trash::restore_file(self.store.root(), item_id, "jsonl", entry_id);

        let entry = self
            .changelog_for(item_id)
            .find_entry(entry_id)
            .await
            .map_err(StoreError::Io)?
            .ok_or_else(|| StoreError::EntryNotFound(entry_id.to_string()))?;

        match entry.op {
            ChangeOp::Create => {
                // Undo create: trash both the data file and its changelog.
                // Reconstruct the created content by applying forward_patch to ""
                // so we can return the item that was undone.
                let created_text = diff::apply_patch("", &entry.forward_patch)?;

                trash::trash_file(
                    self.store.root(),
                    &entry.item_id,
                    self.store.extension(),
                    &entry.id,
                )
                .map_err(StoreError::Io)?;
                trash::trash_file(self.store.root(), &entry.item_id, "jsonl", &entry.id)
                    .map_err(StoreError::Io)?;
                self.cache.write().await.remove(entry.item_id.as_str());

                let id = entry
                    .item_id
                    .as_str()
                    .parse::<S::ItemId>()
                    .map_err(|_| StoreError::Deserialize(entry.item_id.to_string()))?;
                self.store.deserialize(&id, &created_text)
            }
            ChangeOp::Update => {
                // Read current file directly from disk to detect external edits
                let current = self
                    .read_text_from_disk(entry.item_id.as_str())
                    .await?
                    .ok_or_else(|| StoreError::NotFound(entry.item_id.to_string()))?;

                // Apply reverse_patch to current content. If the file hasn't been
                // modified since the entry was created, this applies cleanly. If
                // there were concurrent edits, the patch will fail with a conflict.
                let target = diff::apply_patch(&current, &entry.reverse_patch).map_err(|_| {
                    StoreError::MergeConflict(
                        "reverse patch failed to apply — file was concurrently modified".into(),
                    )
                })?;

                self.atomic_write(entry.item_id.as_str(), &target).await?;
                self.cache
                    .write()
                    .await
                    .insert(entry.item_id.to_string(), target.clone());

                let id = entry
                    .item_id
                    .as_str()
                    .parse::<S::ItemId>()
                    .map_err(|_| StoreError::Deserialize(entry.item_id.to_string()))?;
                self.store.deserialize(&id, &target)
            }
            ChangeOp::Delete => {
                // Undo delete: restore the data file from trash.
                // The changelog was already restored at the top of undo().
                trash::restore_file(
                    self.store.root(),
                    &entry.item_id,
                    self.store.extension(),
                    &entry.id,
                )
                .map_err(StoreError::Io)?;

                // Reconstruct the deleted content by applying reverse_patch to ""
                let restored_text = diff::apply_patch("", &entry.reverse_patch)?;
                self.cache
                    .write()
                    .await
                    .insert(entry.item_id.to_string(), restored_text.clone());

                let id = entry
                    .item_id
                    .as_str()
                    .parse::<S::ItemId>()
                    .map_err(|_| StoreError::Deserialize(entry.item_id.to_string()))?;
                self.store.deserialize(&id, &restored_text)
            }
        }
    }

    /// Redo an operation (inverse of undo).
    ///
    /// The `item_id` identifies which per-item changelog contains the entry.
    ///
    /// - Create: restores the file and its changelog from trash
    /// - Update: re-applies the forward change
    /// - Delete: trashes the file and its changelog again
    pub async fn redo(&self, entry_id: &UndoEntryId, item_id: &StoredItemId) -> Result<S::Item> {
        // If the changelog is in trash (from a prior undo of a create), restore
        // it first so we can read the entry. Ignore errors -- the file may not
        // be trashed.
        let _ = trash::restore_file(self.store.root(), item_id, "jsonl", entry_id);

        let entry = self
            .changelog_for(item_id)
            .find_entry(entry_id)
            .await
            .map_err(StoreError::Io)?
            .ok_or_else(|| StoreError::EntryNotFound(entry_id.to_string()))?;

        match entry.op {
            ChangeOp::Create => {
                // Redo create: restore the data file from trash.
                // The changelog was already restored at the top of redo().
                trash::restore_file(
                    self.store.root(),
                    &entry.item_id,
                    self.store.extension(),
                    &entry.id,
                )
                .map_err(StoreError::Io)?;

                // Reconstruct the created content by applying forward_patch to ""
                let created_text = diff::apply_patch("", &entry.forward_patch)?;
                self.cache
                    .write()
                    .await
                    .insert(entry.item_id.to_string(), created_text.clone());

                let id = entry
                    .item_id
                    .as_str()
                    .parse::<S::ItemId>()
                    .map_err(|_| StoreError::Deserialize(entry.item_id.to_string()))?;
                self.store.deserialize(&id, &created_text)
            }
            ChangeOp::Update => {
                // Read current file directly from disk to detect external edits
                let current = self
                    .read_text_from_disk(entry.item_id.as_str())
                    .await?
                    .ok_or_else(|| StoreError::NotFound(entry.item_id.to_string()))?;

                // Apply forward_patch to current content. If the file hasn't been
                // modified since the undo, this applies cleanly. If there were
                // concurrent edits, the patch will fail with a conflict.
                let target = diff::apply_patch(&current, &entry.forward_patch).map_err(|_| {
                    StoreError::MergeConflict(
                        "forward patch failed to apply — file was concurrently modified".into(),
                    )
                })?;

                self.atomic_write(entry.item_id.as_str(), &target).await?;
                self.cache
                    .write()
                    .await
                    .insert(entry.item_id.to_string(), target.clone());

                let id = entry
                    .item_id
                    .as_str()
                    .parse::<S::ItemId>()
                    .map_err(|_| StoreError::Deserialize(entry.item_id.to_string()))?;
                self.store.deserialize(&id, &target)
            }
            ChangeOp::Delete => {
                // Redo delete: trash both the data file and its changelog again.
                // Reconstruct the deleted content by applying reverse_patch to ""
                // so we can return the item that was re-deleted.
                let deleted_text = diff::apply_patch("", &entry.reverse_patch)?;

                trash::trash_file(
                    self.store.root(),
                    &entry.item_id,
                    self.store.extension(),
                    &entry.id,
                )
                .map_err(StoreError::Io)?;
                trash::trash_file(self.store.root(), &entry.item_id, "jsonl", &entry.id)
                    .map_err(StoreError::Io)?;
                self.cache.write().await.remove(entry.item_id.as_str());

                let id = entry
                    .item_id
                    .as_str()
                    .parse::<S::ItemId>()
                    .map_err(|_| StoreError::Deserialize(entry.item_id.to_string()))?;
                self.store.deserialize(&id, &deleted_text)
            }
        }
    }

    /// Check whether this store's per-item changelog contains the given entry ID.
    ///
    /// The `item_id` identifies which per-item changelog to search. Also checks
    /// if the changelog is in trash (which happens after a delete or undo-create).
    pub async fn has_entry(&self, id: &UndoEntryId, item_id: &StoredItemId) -> bool {
        // First check the live changelog
        let found = self
            .changelog_for(item_id)
            .find_entry(id)
            .await
            .map(|opt| opt.is_some())
            .unwrap_or(false);
        if found {
            return true;
        }
        // Also check if the changelog is in trash (e.g. after delete)
        trash::is_trashed(self.store.root(), item_id, "jsonl", id)
    }

    /// Scan the store directory and detect changes since the last flush.
    ///
    /// Compares current files against the in-memory cache to produce
    /// create/change/remove events. Updates the cache afterwards.
    pub async fn flush_changes(&self) -> Vec<ChangeEvent> {
        let ext = self.store.extension();
        let root = self.store.root();
        let mut events = Vec::new();
        let mut cache = self.cache.write().await;

        // Scan directory for current files using async I/O
        let mut current_files: HashMap<String, String> = HashMap::new();
        if let Ok(mut entries) = tokio::fs::read_dir(root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_ext) = path.extension() {
                        if file_ext == ext {
                            if let Some(stem) = path.file_stem() {
                                let name = stem.to_string_lossy().to_string();
                                // Skip dot-prefixed files (e.g. .trash, .tmp_*)
                                if name.starts_with('.') {
                                    continue;
                                }
                                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                                    current_files.insert(name, content);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Detect new and changed files
        for (id, content) in &current_files {
            match cache.get(id) {
                None => {
                    events.push(ChangeEvent {
                        event_name: "item-created".to_string(),
                        payload: serde_json::json!({ "id": id }),
                    });
                }
                Some(cached) if cached != content => {
                    events.push(ChangeEvent {
                        event_name: "item-changed".to_string(),
                        payload: serde_json::json!({ "id": id }),
                    });
                }
                _ => {}
            }
        }

        // Detect removed files
        for id in cache.keys() {
            if !current_files.contains_key(id) {
                events.push(ChangeEvent {
                    event_name: "item-removed".to_string(),
                    payload: serde_json::json!({ "id": id }),
                });
            }
        }

        // Replace cache with current state
        *cache = current_files;

        events
    }

    /// Construct the file path for an item by its string ID.
    fn item_path(&self, id: &str) -> PathBuf {
        self.store
            .root()
            .join(format!("{}.{}", id, self.store.extension()))
    }

    /// Read text directly from disk, bypassing the cache.
    ///
    /// Used when the caller already holds the cache lock.
    async fn read_text_from_disk(&self, id: &str) -> Result<Option<String>> {
        let path = self.item_path(id);
        match tokio::fs::read_to_string(&path).await {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    /// Write content atomically: write to a temp file, then rename.
    async fn atomic_write(&self, id: &str, content: &str) -> Result<()> {
        let root = self.store.root();
        let tmp_name = format!(".tmp_{}", ulid::Ulid::new());
        let tmp_path = root.join(&tmp_name);
        let final_path = self.item_path(id);

        // Ensure root directory exists
        tokio::fs::create_dir_all(root).await?;

        tokio::fs::write(&tmp_path, content).await?;
        tokio::fs::rename(&tmp_path, &final_path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A simple mock store for testing. Items are plain strings where the
    /// first line is the ID.
    struct MockStore {
        root: PathBuf,
    }

    impl TrackedStore for MockStore {
        type Item = String;
        type ItemId = String;

        fn root(&self) -> &Path {
            &self.root
        }

        fn item_id(&self, item: &String) -> String {
            item.lines().next().unwrap_or("unknown").to_string()
        }

        fn serialize(&self, item: &String) -> Result<String> {
            Ok(item.clone())
        }

        fn deserialize(&self, _id: &String, text: &str) -> Result<String> {
            Ok(text.to_string())
        }

        fn extension(&self) -> &str {
            "txt"
        }
    }

    fn setup() -> (tempfile::TempDir, StoreHandle<MockStore>) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Arc::new(MockStore {
            root: dir.path().to_path_buf(),
        });
        let handle = StoreHandle::new(store);
        (dir, handle)
    }

    #[tokio::test]
    async fn write_creates_file_and_changelog() {
        let (_dir, handle) = setup();
        let item = "item1\ncontent here".to_string();

        let result = handle.write(&item).await.unwrap();
        assert!(result.is_some());

        // File should exist
        let path = _dir.path().join("item1.txt");
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), item);

        // Per-item changelog should have one entry
        let item1_id = StoredItemId::from("item1");
        let entries = handle.changelog_for(&item1_id).read_all().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].op, ChangeOp::Create);
        assert_eq!(entries[0].item_id, item1_id);
    }

    #[tokio::test]
    async fn write_same_content_is_idempotent() {
        let (_dir, handle) = setup();
        let item = "item1\ncontent".to_string();

        let first = handle.write(&item).await.unwrap();
        assert!(first.is_some());

        let second = handle.write(&item).await.unwrap();
        assert!(second.is_none());

        // Only one changelog entry
        let entries = handle
            .changelog_for(&StoredItemId::from("item1"))
            .read_all()
            .await
            .unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn get_reads_written_item() {
        let (_dir, handle) = setup();
        let item = "item1\nsome data".to_string();
        handle.write(&item).await.unwrap();

        let retrieved = handle.get(&"item1".to_string()).await.unwrap();
        assert_eq!(retrieved, item);
    }

    #[tokio::test]
    async fn delete_moves_to_trash() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        handle.write(&item).await.unwrap();

        let entry_id = handle.delete(&"item1".to_string()).await.unwrap();
        let item1_id = StoredItemId::from("item1");
        assert!(!_dir.path().join("item1.txt").exists());
        assert!(trash::is_trashed(_dir.path(), &item1_id, "txt", &entry_id));
        // Per-item changelog is also trashed alongside the data file
        assert!(trash::is_trashed(
            _dir.path(),
            &item1_id,
            "jsonl",
            &entry_id
        ));
    }

    #[tokio::test]
    async fn undo_create_trashes_file() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        let entry_id = handle.write(&item).await.unwrap().unwrap();

        let item1_id = StoredItemId::from("item1");
        handle.undo(&entry_id, &item1_id).await.unwrap();
        assert!(!_dir.path().join("item1.txt").exists());
        assert!(trash::is_trashed(_dir.path(), &item1_id, "txt", &entry_id));
        assert!(trash::is_trashed(
            _dir.path(),
            &item1_id,
            "jsonl",
            &entry_id
        ));
    }

    #[tokio::test]
    async fn undo_update_reverts_to_before() {
        let (_dir, handle) = setup();
        let v1 = "item1\nversion1".to_string();
        let v2 = "item1\nversion2".to_string();

        handle.write(&v1).await.unwrap();
        let update_id = handle.write(&v2).await.unwrap().unwrap();

        handle
            .undo(&update_id, &StoredItemId::from("item1"))
            .await
            .unwrap();

        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, v1);
    }

    #[tokio::test]
    async fn undo_delete_restores_from_trash() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        handle.write(&item).await.unwrap();
        let delete_id = handle.delete(&"item1".to_string()).await.unwrap();

        handle
            .undo(&delete_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        assert!(_dir.path().join("item1.txt").exists());
        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, "item1\ndata");
    }

    #[tokio::test]
    async fn redo_after_undo_restores_change() {
        let (_dir, handle) = setup();
        let v1 = "item1\nversion1".to_string();
        let v2 = "item1\nversion2".to_string();

        handle.write(&v1).await.unwrap();
        let update_id = handle.write(&v2).await.unwrap().unwrap();

        // Undo
        handle
            .undo(&update_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, v1);

        // Redo
        handle
            .redo(&update_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, v2);
    }

    #[tokio::test]
    async fn flush_changes_detects_external_create() {
        let (_dir, handle) = setup();

        // Write a file externally (not through the handle)
        std::fs::write(_dir.path().join("external.txt"), "external content").unwrap();

        let events = handle.flush_changes().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "item-created");
    }

    #[tokio::test]
    async fn flush_changes_detects_external_change() {
        let (_dir, handle) = setup();
        let item = "item1\noriginal".to_string();
        handle.write(&item).await.unwrap();

        // Modify externally
        std::fs::write(_dir.path().join("item1.txt"), "item1\nmodified").unwrap();

        let events = handle.flush_changes().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "item-changed");
    }

    #[tokio::test]
    async fn flush_changes_detects_external_remove() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        handle.write(&item).await.unwrap();

        // Remove externally
        std::fs::remove_file(_dir.path().join("item1.txt")).unwrap();

        let events = handle.flush_changes().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "item-removed");
    }

    #[tokio::test]
    async fn has_entry_returns_true_for_known_entry() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        let entry_id = handle.write(&item).await.unwrap().unwrap();

        assert!(
            handle
                .has_entry(&entry_id, &StoredItemId::from("item1"))
                .await
        );
    }

    #[tokio::test]
    async fn has_entry_returns_false_for_unknown_entry() {
        let (_dir, handle) = setup();
        let unknown = UndoEntryId::new();
        assert!(
            !handle
                .has_entry(&unknown, &StoredItemId::from("item1"))
                .await
        );
    }

    #[tokio::test]
    async fn write_update_creates_update_changelog() {
        let (_dir, handle) = setup();
        let v1 = "item1\nv1".to_string();
        let v2 = "item1\nv2".to_string();

        handle.write(&v1).await.unwrap();
        handle.write(&v2).await.unwrap();

        let entries = handle
            .changelog_for(&StoredItemId::from("item1"))
            .read_all()
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].op, ChangeOp::Create);
        assert_eq!(entries[1].op, ChangeOp::Update);
    }

    #[tokio::test]
    async fn redo_create_restores_from_trash() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        let create_id = handle.write(&item).await.unwrap().unwrap();

        handle
            .undo(&create_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        assert!(!_dir.path().join("item1.txt").exists());

        handle
            .redo(&create_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        assert!(_dir.path().join("item1.txt").exists());
        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, item);
    }

    #[tokio::test]
    async fn redo_delete_trashes_file_again() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        handle.write(&item).await.unwrap();
        let delete_id = handle.delete(&"item1".to_string()).await.unwrap();

        // Undo delete (restore)
        handle
            .undo(&delete_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        assert!(_dir.path().join("item1.txt").exists());

        // Redo delete (trash again)
        handle
            .redo(&delete_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        assert!(!_dir.path().join("item1.txt").exists());
        let item1_id = StoredItemId::from("item1");
        assert!(trash::is_trashed(_dir.path(), &item1_id, "txt", &delete_id));
        assert!(trash::is_trashed(
            _dir.path(),
            &item1_id,
            "jsonl",
            &delete_id
        ));
    }

    #[tokio::test]
    async fn undo_update_with_non_overlapping_concurrent_edit_succeeds() {
        // Write v1, write v2, externally modify to v3, undo v1->v2.
        // Non-overlapping changes apply cleanly because the reverse patch
        // only touches the region that was changed by the update.
        let (_dir, handle) = setup();

        let v1 = "item1\nline2\nline3\nline4\nline5\nline6\nline7\n".to_string();
        let v2 = "item1\nCHANGED\nline3\nline4\nline5\nline6\nline7\n".to_string();
        let v3 = "item1\nCHANGED\nline3\nline4\nline5\nline6\nEXTERNAL\n".to_string();

        handle.write(&v1).await.unwrap();
        let update_id = handle.write(&v2).await.unwrap().unwrap();

        // External edit: change last line (non-overlapping with v1->v2 change)
        std::fs::write(_dir.path().join("item1.txt"), &v3).unwrap();

        // Undo applies reverse patch, reverting line2 while keeping external edit
        let result = handle
            .undo(&update_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        assert!(result.contains("line2"), "should revert to original line2");
        assert!(
            result.contains("EXTERNAL"),
            "should keep external line7 change"
        );
    }

    #[tokio::test]
    async fn undo_update_with_conflicting_edit_returns_error() {
        let (_dir, handle) = setup();

        let v1 = "item1\nline2\nline3\n".to_string();
        let v2 = "item1\nCHANGED\nline3\n".to_string();
        // Conflict: external edit changes the same line that v1->v2 changed
        let v3 = "item1\nCONFLICT\nline3\n".to_string();

        handle.write(&v1).await.unwrap();
        let update_id = handle.write(&v2).await.unwrap().unwrap();

        // External edit on same line
        std::fs::write(_dir.path().join("item1.txt"), &v3).unwrap();

        let result = handle.undo(&update_id, &StoredItemId::from("item1")).await;
        assert!(
            result.is_err(),
            "conflicting concurrent edit should produce MergeConflict"
        );
        match result {
            Err(StoreError::MergeConflict(_)) => {} // expected
            other => panic!("expected MergeConflict, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn redo_update_with_non_overlapping_concurrent_edit_succeeds() {
        // Write v1, write v2, undo, externally modify, redo.
        // Non-overlapping changes apply cleanly because the forward patch
        // only touches the region that was changed by the update.
        let (_dir, handle) = setup();

        let v1 = "item1\nline2\nline3\nline4\nline5\nline6\nline7\n".to_string();
        let v2 = "item1\nCHANGED\nline3\nline4\nline5\nline6\nline7\n".to_string();

        handle.write(&v1).await.unwrap();
        let update_id = handle.write(&v2).await.unwrap().unwrap();

        // Undo to get back to v1
        handle
            .undo(&update_id, &StoredItemId::from("item1"))
            .await
            .unwrap();

        // External edit: change last line (non-overlapping with v1->v2 change)
        let v1_modified = "item1\nline2\nline3\nline4\nline5\nline6\nEXTERNAL\n".to_string();
        std::fs::write(_dir.path().join("item1.txt"), &v1_modified).unwrap();

        // Redo applies forward patch, re-applying line2 change while keeping external edit
        let result = handle
            .redo(&update_id, &StoredItemId::from("item1"))
            .await
            .unwrap();
        assert!(result.contains("CHANGED"), "should re-apply line2 change");
        assert!(
            result.contains("EXTERNAL"),
            "should keep external line7 change"
        );
    }
}
