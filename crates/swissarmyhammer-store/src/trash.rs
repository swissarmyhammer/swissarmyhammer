//! Soft-delete support via a `.trash/` subdirectory.
//!
//! Instead of permanently deleting files, they are moved to a `.trash/`
//! directory within the store root. This allows undo of delete operations
//! by restoring files from trash.

use std::io;
use std::path::Path;

use crate::id::{StoredItemId, UndoEntryId};

/// Move an item's file from the live directory to the `.trash/` subdirectory.
///
/// The trash filename includes the undo entry ID so that multiple versions
/// of the same item can coexist in trash (e.g., create → undo → create → undo).
/// Creates the `.trash/` directory if it does not already exist.
pub fn trash_file(
    root: &Path,
    item_id: &StoredItemId,
    ext: &str,
    entry_id: &UndoEntryId,
) -> io::Result<()> {
    let trash_dir = root.join(".trash");
    std::fs::create_dir_all(&trash_dir)?;

    let src = root.join(format!("{}.{}", item_id.as_str(), ext));
    let dst = trash_dir.join(format!("{}.{}.{}", item_id.as_str(), entry_id, ext));
    std::fs::rename(src, dst)?;
    Ok(())
}

/// Restore an item's file from `.trash/` back to the live directory.
///
/// The `entry_id` identifies which trashed version to restore.
/// Returns an error if the trashed file does not exist.
pub fn restore_file(
    root: &Path,
    item_id: &StoredItemId,
    ext: &str,
    entry_id: &UndoEntryId,
) -> io::Result<()> {
    let trash_dir = root.join(".trash");
    let src = trash_dir.join(format!("{}.{}.{}", item_id.as_str(), entry_id, ext));
    let dst = root.join(format!("{}.{}", item_id.as_str(), ext));
    std::fs::rename(src, dst)?;
    Ok(())
}

/// Check whether an item's file exists in the `.trash/` directory for a given entry.
pub fn is_trashed(root: &Path, item_id: &StoredItemId, ext: &str, entry_id: &UndoEntryId) -> bool {
    let trash_path = root
        .join(".trash")
        .join(format!("{}.{}.{}", item_id.as_str(), entry_id, ext));
    trash_path.exists()
}

/// Move an item's file from the live directory to the `.archive/` subdirectory.
///
/// Works identically to [`trash_file`] but uses `.archive/` instead of `.trash/`.
/// The archive filename includes the undo entry ID so that multiple versions
/// of the same item can coexist in the archive.
pub fn archive_file(
    root: &Path,
    item_id: &StoredItemId,
    ext: &str,
    entry_id: &UndoEntryId,
) -> io::Result<()> {
    let archive_dir = root.join(".archive");
    std::fs::create_dir_all(&archive_dir)?;

    let src = root.join(format!("{}.{}", item_id.as_str(), ext));
    let dst = archive_dir.join(format!("{}.{}.{}", item_id.as_str(), entry_id, ext));
    std::fs::rename(src, dst)?;
    Ok(())
}

/// Restore an item's file from `.archive/` back to the live directory.
///
/// The `entry_id` identifies which archived version to restore.
/// Returns an error if the archived file does not exist.
pub fn restore_archived_file(
    root: &Path,
    item_id: &StoredItemId,
    ext: &str,
    entry_id: &UndoEntryId,
) -> io::Result<()> {
    let archive_dir = root.join(".archive");
    let src = archive_dir.join(format!("{}.{}.{}", item_id.as_str(), entry_id, ext));
    let dst = root.join(format!("{}.{}", item_id.as_str(), ext));
    std::fs::rename(src, dst)?;
    Ok(())
}

/// Check whether an item's file exists in the `.archive/` directory for a given entry.
pub fn is_archived(root: &Path, item_id: &StoredItemId, ext: &str, entry_id: &UndoEntryId) -> bool {
    let archive_path =
        root.join(".archive")
            .join(format!("{}.{}.{}", item_id.as_str(), entry_id, ext));
    archive_path.exists()
}

/// Find the undo entry ID for an archived item by scanning the `.archive/` directory.
///
/// Looks for a file matching `{item_id}.{entry_id}.{ext}` and extracts the
/// entry_id portion. Returns the most recently archived version if multiple exist.
/// Returns `None` if the item is not found in the archive.
pub fn find_archived_entry_id(
    root: &Path,
    item_id: &StoredItemId,
    ext: &str,
) -> Option<UndoEntryId> {
    let archive_dir = root.join(".archive");
    let prefix = format!("{}.", item_id.as_str());
    let suffix = format!(".{}", ext);

    let entries = std::fs::read_dir(&archive_dir).ok()?;
    let mut best: Option<UndoEntryId> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(&suffix) {
            // Extract the entry_id portion between prefix and suffix
            let mid = &name[prefix.len()..name.len() - suffix.len()];
            if let Ok(eid) = mid.parse::<UndoEntryId>() {
                // Keep the largest (most recent) entry ID
                best = Some(match best {
                    Some(prev) if prev > eid => prev,
                    _ => eid,
                });
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn trash_file_moves_to_trash_dir() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        trash_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();

        assert!(!file_path.exists());
        assert!(is_trashed(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));
    }

    #[test]
    fn restore_file_moves_back() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        trash_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(!file_path.exists());

        restore_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "content");
    }

    #[test]
    fn is_trashed_checks_correctly() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        assert!(!is_trashed(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));

        trash_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(is_trashed(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));

        restore_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(!is_trashed(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));
    }

    #[test]
    fn trash_creates_trash_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        assert!(!root.join(".trash").exists());
        trash_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(root.join(".trash").exists());
    }

    #[test]
    fn restore_nonexistent_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let entry_id = UndoEntryId::new();
        let result = restore_file(root, &StoredItemId::from("missing"), "txt", &entry_id);
        assert!(result.is_err());
    }

    #[test]
    fn archive_file_moves_to_archive_dir() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        archive_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();

        assert!(!file_path.exists());
        assert!(is_archived(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));
    }

    #[test]
    fn restore_archived_file_moves_back() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        archive_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(!file_path.exists());

        restore_archived_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "content");
    }

    #[test]
    fn is_archived_checks_correctly() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        assert!(!is_archived(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));

        archive_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(is_archived(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));

        restore_archived_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(!is_archived(
            root,
            &StoredItemId::from("item1"),
            "txt",
            &entry_id
        ));
    }

    #[test]
    fn archive_creates_archive_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file_path = root.join("item1.txt");
        std::fs::write(&file_path, "content").unwrap();

        let entry_id = UndoEntryId::new();
        assert!(!root.join(".archive").exists());
        archive_file(root, &StoredItemId::from("item1"), "txt", &entry_id).unwrap();
        assert!(root.join(".archive").exists());
    }

    #[test]
    fn restore_nonexistent_archived_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let entry_id = UndoEntryId::new();
        let result = restore_archived_file(root, &StoredItemId::from("missing"), "txt", &entry_id);
        assert!(result.is_err());
    }

    #[test]
    fn multiple_versions_coexist_in_trash() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create and trash first version
        std::fs::write(root.join("item1.txt"), "v1").unwrap();
        let entry1 = UndoEntryId::new();
        let item_id = StoredItemId::from("item1");
        trash_file(root, &item_id, "txt", &entry1).unwrap();

        // Create and trash second version
        std::fs::write(root.join("item1.txt"), "v2").unwrap();
        let entry2 = UndoEntryId::new();
        trash_file(root, &item_id, "txt", &entry2).unwrap();

        // Both should exist in trash
        assert!(is_trashed(root, &item_id, "txt", &entry1));
        assert!(is_trashed(root, &item_id, "txt", &entry2));

        // Restoring one doesn't affect the other
        restore_file(root, &item_id, "txt", &entry2).unwrap();
        assert!(is_trashed(root, &item_id, "txt", &entry1));
        assert!(!is_trashed(root, &item_id, "txt", &entry2));
        assert_eq!(
            std::fs::read_to_string(root.join("item1.txt")).unwrap(),
            "v2"
        );
    }
}
