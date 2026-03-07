//! View changelog with whole-view snapshots and undo/redo support.
//!
//! Each change entry stores the complete `previous` and `current` ViewDef
//! as JSON. Undo replays `previous`, redo replays `current`.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::context::ViewsContext;
use crate::error::{Result, ViewsError};
use crate::types::ViewDef;

/// The operation type for a changelog entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ViewChangeOp {
    Create,
    Update,
    Delete,
}

/// A single changelog entry with whole-view snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChangeEntry {
    pub id: String,
    pub timestamp: String,
    pub op: ViewChangeOp,
    pub view_id: String,
    /// The previous state of the view (None for create).
    pub previous: Option<serde_json::Value>,
    /// The current state of the view (None for delete).
    pub current: Option<serde_json::Value>,
}

impl ViewChangeEntry {
    /// Create a new changelog entry.
    pub fn new(
        op: ViewChangeOp,
        view_id: String,
        previous: Option<&ViewDef>,
        current: Option<&ViewDef>,
    ) -> Result<Self> {
        Ok(Self {
            id: ulid::Ulid::new().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            op,
            view_id,
            previous: previous.map(serde_json::to_value).transpose()?,
            current: current.map(serde_json::to_value).transpose()?,
        })
    }
}

/// Append a changelog entry to the views changelog file.
pub async fn append_changelog(changelog_path: &Path, entry: &ViewChangeEntry) -> Result<()> {
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');

    if let Some(parent) = changelog_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(changelog_path)
        .await?;

    file.write_all(line.as_bytes()).await?;
    file.flush().await?;
    Ok(())
}

/// Read all changelog entries from the views changelog file.
pub async fn read_changelog(changelog_path: &Path) -> Result<Vec<ViewChangeEntry>> {
    if !changelog_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(changelog_path).await?;
    let entries: Vec<ViewChangeEntry> = content
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Ok(entries)
}

/// Undo a changelog entry by replaying the `previous` snapshot.
///
/// Returns the ULID of the new (compensating) changelog entry.
pub async fn undo_entry(
    changelog_path: &Path,
    entry_id: &str,
    ctx: &mut ViewsContext,
) -> Result<String> {
    let entries = read_changelog(changelog_path).await?;
    let entry = entries
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| ViewsError::ChangelogEntryNotFound {
            id: entry_id.to_string(),
        })?
        .clone();

    match entry.op {
        ViewChangeOp::Create => {
            // Undo create = delete (capture snapshot first so redo can restore)
            let snapshot = ctx.get_by_id(&entry.view_id).cloned();
            ctx.delete_view(&entry.view_id).await?;
            let undo_entry =
                ViewChangeEntry::new(ViewChangeOp::Delete, entry.view_id, snapshot.as_ref(), None)?;
            let id = undo_entry.id.clone();
            append_changelog(changelog_path, &undo_entry).await?;
            Ok(id)
        }
        ViewChangeOp::Update => {
            // Undo update = write previous
            let previous: ViewDef = entry
                .previous
                .as_ref()
                .ok_or(ViewsError::NothingToUndo)
                .and_then(|v| serde_json::from_value(v.clone()).map_err(ViewsError::Json))?;
            let current_def = ctx.get_by_id(&entry.view_id).cloned();
            ctx.write_view(&previous).await?;
            let undo_entry = ViewChangeEntry::new(
                ViewChangeOp::Update,
                entry.view_id,
                current_def.as_ref(),
                Some(&previous),
            )?;
            let id = undo_entry.id.clone();
            append_changelog(changelog_path, &undo_entry).await?;
            Ok(id)
        }
        ViewChangeOp::Delete => {
            // Undo delete = write previous back
            let previous: ViewDef = entry
                .previous
                .as_ref()
                .ok_or(ViewsError::NothingToUndo)
                .and_then(|v| serde_json::from_value(v.clone()).map_err(ViewsError::Json))?;
            ctx.write_view(&previous).await?;
            let undo_entry =
                ViewChangeEntry::new(ViewChangeOp::Create, entry.view_id, None, Some(&previous))?;
            let id = undo_entry.id.clone();
            append_changelog(changelog_path, &undo_entry).await?;
            Ok(id)
        }
    }
}

/// Changelog manager that pairs with a ViewsContext.
#[derive(Debug)]
pub struct ViewsChangelog {
    path: PathBuf,
}

impl ViewsChangelog {
    pub fn new(changelog_path: impl Into<PathBuf>) -> Self {
        Self {
            path: changelog_path.into(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Log a create operation.
    pub async fn log_create(&self, def: &ViewDef) -> Result<String> {
        let entry = ViewChangeEntry::new(ViewChangeOp::Create, def.id.clone(), None, Some(def))?;
        let id = entry.id.clone();
        append_changelog(&self.path, &entry).await?;
        Ok(id)
    }

    /// Log an update operation.
    pub async fn log_update(&self, previous: &ViewDef, current: &ViewDef) -> Result<String> {
        let entry = ViewChangeEntry::new(
            ViewChangeOp::Update,
            current.id.clone(),
            Some(previous),
            Some(current),
        )?;
        let id = entry.id.clone();
        append_changelog(&self.path, &entry).await?;
        Ok(id)
    }

    /// Log a delete operation.
    pub async fn log_delete(&self, def: &ViewDef) -> Result<String> {
        let entry = ViewChangeEntry::new(ViewChangeOp::Delete, def.id.clone(), Some(def), None)?;
        let id = entry.id.clone();
        append_changelog(&self.path, &entry).await?;
        Ok(id)
    }

    /// Read all entries.
    pub async fn read_all(&self) -> Result<Vec<ViewChangeEntry>> {
        read_changelog(&self.path).await
    }

    /// Undo a specific entry by ID.
    pub async fn undo(&self, entry_id: &str, ctx: &mut ViewsContext) -> Result<String> {
        undo_entry(&self.path, entry_id, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ViewDef, ViewKind};
    use tempfile::TempDir;

    fn make_view(id: &str, name: &str) -> ViewDef {
        ViewDef {
            id: id.into(),
            name: name.into(),
            icon: None,
            kind: ViewKind::Board,
            entity_type: None,
            card_fields: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[tokio::test]
    async fn changelog_append_and_read() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("views.jsonl");

        let view = make_view("01A", "Board");
        let entry =
            ViewChangeEntry::new(ViewChangeOp::Create, "01A".into(), None, Some(&view)).unwrap();

        append_changelog(&path, &entry).await.unwrap();
        let entries = read_changelog(&path).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].op, ViewChangeOp::Create);
        assert_eq!(entries[0].view_id, "01A");
        assert!(entries[0].previous.is_none());
        assert!(entries[0].current.is_some());
    }

    #[tokio::test]
    async fn changelog_multiple_entries() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("views.jsonl");

        let view1 = make_view("01A", "Board");
        let view2 = make_view("01A", "Board Updated");

        let e1 =
            ViewChangeEntry::new(ViewChangeOp::Create, "01A".into(), None, Some(&view1)).unwrap();
        let e2 = ViewChangeEntry::new(
            ViewChangeOp::Update,
            "01A".into(),
            Some(&view1),
            Some(&view2),
        )
        .unwrap();

        append_changelog(&path, &e1).await.unwrap();
        append_changelog(&path, &e2).await.unwrap();

        let entries = read_changelog(&path).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].op, ViewChangeOp::Create);
        assert_eq!(entries[1].op, ViewChangeOp::Update);
    }

    #[tokio::test]
    async fn changelog_nonexistent_returns_empty() {
        let entries = read_changelog(Path::new("/nonexistent/views.jsonl"))
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn views_changelog_log_and_read() {
        let tmp = TempDir::new().unwrap();
        let changelog = ViewsChangelog::new(tmp.path().join("views.jsonl"));

        let view = make_view("01A", "Board");
        let create_id = changelog.log_create(&view).await.unwrap();
        assert!(!create_id.is_empty());

        let view2 = make_view("01A", "Board V2");
        let update_id = changelog.log_update(&view, &view2).await.unwrap();
        assert!(!update_id.is_empty());

        let delete_id = changelog.log_delete(&view2).await.unwrap();
        assert!(!delete_id.is_empty());

        let entries = changelog.read_all().await.unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[tokio::test]
    async fn undo_create_deletes_view() {
        let tmp = TempDir::new().unwrap();
        let views_root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&views_root).build().await.unwrap();
        let changelog = ViewsChangelog::new(tmp.path().join("views.jsonl"));

        let view = make_view("01A", "Board");
        ctx.write_view(&view).await.unwrap();
        let create_id = changelog.log_create(&view).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);

        changelog.undo(&create_id, &mut ctx).await.unwrap();

        assert_eq!(ctx.all_views().len(), 0);

        // The compensating Delete entry should have `previous` set so redo works
        let entries = changelog.read_all().await.unwrap();
        let compensating = entries.last().unwrap();
        assert_eq!(compensating.op, ViewChangeOp::Delete);
        assert!(
            compensating.previous.is_some(),
            "compensating Delete must capture previous for redo"
        );

        // Undo the compensating Delete (i.e. redo the original create)
        changelog.undo(&compensating.id, &mut ctx).await.unwrap();
        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01A").unwrap().name, "Board");
    }

    #[tokio::test]
    async fn undo_update_reverts_view() {
        let tmp = TempDir::new().unwrap();
        let views_root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&views_root).build().await.unwrap();
        let changelog = ViewsChangelog::new(tmp.path().join("views.jsonl"));

        let v1 = make_view("01A", "Board");
        ctx.write_view(&v1).await.unwrap();
        changelog.log_create(&v1).await.unwrap();

        let v2 = make_view("01A", "Board Updated");
        ctx.write_view(&v2).await.unwrap();
        let update_id = changelog.log_update(&v1, &v2).await.unwrap();

        assert_eq!(ctx.get_by_id("01A").unwrap().name, "Board Updated");

        changelog.undo(&update_id, &mut ctx).await.unwrap();

        assert_eq!(ctx.get_by_id("01A").unwrap().name, "Board");
    }

    #[tokio::test]
    async fn undo_delete_restores_view() {
        let tmp = TempDir::new().unwrap();
        let views_root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&views_root).build().await.unwrap();
        let changelog = ViewsChangelog::new(tmp.path().join("views.jsonl"));

        let view = make_view("01A", "Board");
        ctx.write_view(&view).await.unwrap();
        changelog.log_create(&view).await.unwrap();

        let delete_id = changelog.log_delete(&view).await.unwrap();
        ctx.delete_view("01A").await.unwrap();

        assert_eq!(ctx.all_views().len(), 0);

        changelog.undo(&delete_id, &mut ctx).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01A").unwrap().name, "Board");
    }
}
