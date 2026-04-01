//! PerspectiveContext -- file-backed CRUD for perspective definitions.
//!
//! Manages perspectives stored as YAML files in a `perspectives/` directory,
//! with in-memory indexes for fast lookup by ID and name.
//!
//! ```text
//! .kanban/perspectives/
//!   01JPERSP000000000000000000.yaml
//!   01JPERSP000000000000000001.yaml
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::debug;

use crate::error::{PerspectiveError, Result};
use crate::types::Perspective;

/// Context for perspective definitions -- file-backed CRUD with in-memory indexes.
///
/// Each perspective is stored as `{id}.yaml` in the root directory. On `open()`,
/// all YAML files are loaded into memory. Mutations (`write`, `delete`) persist
/// to disk immediately and update the in-memory indexes.
#[derive(Debug)]
pub struct PerspectiveContext {
    root: PathBuf,
    perspectives: Vec<Perspective>,
    id_index: HashMap<String, usize>,
    name_index: HashMap<String, usize>,
}

impl PerspectiveContext {
    /// Open a perspectives directory, loading all YAML files into memory.
    ///
    /// Creates the directory if it does not exist. Invalid YAML files are
    /// logged and skipped.
    pub async fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let root = dir.into();
        fs::create_dir_all(&root).await?;

        let mut ctx = Self {
            root,
            perspectives: Vec::new(),
            id_index: HashMap::new(),
            name_index: HashMap::new(),
        };

        ctx.load_all().await?;

        debug!(
            perspectives = ctx.perspectives.len(),
            "perspective context opened"
        );
        Ok(ctx)
    }

    /// Write (create or update) a perspective. Persists to YAML immediately.
    ///
    /// If a perspective with the same ID already exists, it is replaced.
    /// The in-memory indexes are updated to reflect name changes.
    ///
    /// Returns `PerspectiveError::DuplicateName` if a *different* perspective already
    /// uses the same name (names must be unique across perspectives).
    pub async fn write(&mut self, perspective: &Perspective) -> Result<()> {
        // Enforce name uniqueness *before* touching disk
        if let Some(&idx) = self.id_index.get(&perspective.id) {
            // Existing perspective being updated -- only check if the name changed
            let old_name = &self.perspectives[idx].name;
            if *old_name != perspective.name {
                if let Some(&other_idx) = self.name_index.get(&perspective.name) {
                    if self.perspectives[other_idx].id != perspective.id {
                        return Err(PerspectiveError::duplicate_name(
                            "perspective",
                            &perspective.name,
                        ));
                    }
                }
            }
        } else {
            // Brand-new perspective -- reject if name already taken
            if self.name_index.contains_key(&perspective.name) {
                return Err(PerspectiveError::duplicate_name(
                    "perspective",
                    &perspective.name,
                ));
            }
        }

        let yaml = serde_yaml_ng::to_string(perspective)?;
        let path = self.perspective_path(&perspective.id);
        atomic_write(&path, yaml.as_bytes()).await?;

        // Update in-memory state (name uniqueness already validated above)
        if let Some(&idx) = self.id_index.get(&perspective.id) {
            let old_name = self.perspectives[idx].name.clone();
            if old_name != perspective.name {
                self.name_index.remove(&old_name);
            }
            self.perspectives[idx] = perspective.clone();
            self.name_index.insert(perspective.name.clone(), idx);
        } else {
            let idx = self.perspectives.len();
            self.perspectives.push(perspective.clone());
            self.id_index.insert(perspective.id.clone(), idx);
            self.name_index.insert(perspective.name.clone(), idx);
        }

        Ok(())
    }

    /// Look up a perspective by its ULID.
    pub fn get_by_id(&self, id: &str) -> Option<&Perspective> {
        self.id_index.get(id).map(|&i| &self.perspectives[i])
    }

    /// Look up a perspective by its human-readable name.
    pub fn get_by_name(&self, name: &str) -> Option<&Perspective> {
        self.name_index.get(name).map(|&i| &self.perspectives[i])
    }

    /// All loaded perspectives.
    pub fn all(&self) -> &[Perspective] {
        &self.perspectives
    }

    /// Delete a perspective by ID. Removes the YAML file and returns the
    /// deleted perspective for changelog recording.
    ///
    /// Returns `PerspectiveError::NotFound` if no perspective with that ID exists.
    pub async fn delete(&mut self, id: &str) -> Result<Perspective> {
        let idx = self
            .id_index
            .get(id)
            .copied()
            .ok_or_else(|| PerspectiveError::NotFound {
                resource: "perspective".to_string(),
                id: id.to_string(),
            })?;

        let path = self.perspective_path(id);
        match fs::remove_file(&path).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(PerspectiveError::Io(e));
            }
        }

        let deleted = self.perspectives.swap_remove(idx);
        self.id_index.remove(&deleted.id);
        self.name_index.remove(&deleted.name);

        // Fix the index of the element that was swapped into `idx`
        if idx < self.perspectives.len() {
            let moved = &self.perspectives[idx];
            self.id_index.insert(moved.id.clone(), idx);
            self.name_index.insert(moved.name.clone(), idx);
        }

        Ok(deleted)
    }

    /// The root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // --- Internal ---

    /// Path to a perspective's YAML file.
    fn perspective_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.yaml"))
    }

    /// Load all YAML files from the root directory into memory.
    async fn load_all(&mut self) -> Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        let mut entries = fs::read_dir(&self.root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let content = fs::read_to_string(&path).await?;
            match serde_yaml_ng::from_str::<Perspective>(&content) {
                Ok(p) => {
                    let idx = self.perspectives.len();
                    self.id_index.insert(p.id.clone(), idx);
                    self.name_index.insert(p.name.clone(), idx);
                    self.perspectives.push(p);
                }
                Err(e) => {
                    tracing::warn!(?path, %e, "skipping invalid perspective file");
                }
            }
        }
        Ok(())
    }
}

/// Write to a temp file then rename for atomic persistence.
async fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent dir"))?;
    let tmp = dir.join(format!(".tmp_{}", ulid::Ulid::new()));
    fs::write(&tmp, data).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PerspectiveFieldEntry, SortDirection, SortEntry};
    use tempfile::TempDir;

    /// Helper to create a test perspective with sensible defaults.
    fn make_perspective(id: &str, name: &str) -> Perspective {
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

    /// Helper to create a rich perspective with all optional fields populated.
    fn make_rich_perspective(id: &str, name: &str) -> Perspective {
        Perspective {
            id: id.to_string(),
            name: name.to_string(),
            view: "grid".to_string(),
            fields: vec![PerspectiveFieldEntry {
                field: "01JMTASK0000000000TITLE00".to_string(),
                caption: Some("Title".to_string()),
                width: Some(200),
                editor: None,
                display: None,
                sort_comparator: None,
            }],
            filter: Some("(e) => e.Status !== \"Done\"".to_string()),
            group: Some("(e) => e.Assignee".to_string()),
            sort: vec![SortEntry {
                field: "01JMTASK0000000000PRIORTY".to_string(),
                direction: SortDirection::Asc,
            }],
        }
    }

    #[tokio::test]
    async fn write_and_read_by_id() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Sprint View");
        ctx.write(&p).await.unwrap();

        let found = ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
        assert_eq!(found.name, "Sprint View");
        assert_eq!(found.view, "board");
    }

    #[tokio::test]
    async fn write_and_read_by_name() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_rich_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "My Grid");
        ctx.write(&p).await.unwrap();

        let found = ctx.get_by_name("My Grid").unwrap();
        assert_eq!(found.id, "01BBBBBBBBBBBBBBBBBBBBBBBB");
        assert_eq!(found.fields.len(), 1);
        assert_eq!(found.filter.as_deref(), Some("(e) => e.Status !== \"Done\""));
    }

    #[tokio::test]
    async fn list_all_perspectives() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        assert!(ctx.all().is_empty());

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "View A"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "View B"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "View C"))
            .await
            .unwrap();

        assert_eq!(ctx.all().len(), 3);
    }

    #[tokio::test]
    async fn delete_perspective() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write(&p).await.unwrap();
        assert_eq!(ctx.all().len(), 1);

        let deleted = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert_eq!(deleted.name, "Doomed");
        assert!(ctx.all().is_empty());
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none());
        assert!(ctx.get_by_name("Doomed").is_none());

        // File should be gone
        let file = tmp
            .path()
            .join("perspectives")
            .join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml");
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn delete_tolerates_already_removed_file() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Ghost");
        ctx.write(&p).await.unwrap();

        // Remove the file externally before calling delete
        let file = tmp
            .path()
            .join("perspectives")
            .join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml");
        fs::remove_file(&file).await.unwrap();
        assert!(!file.exists());

        // delete should still succeed (NotFound is tolerated)
        let deleted = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert_eq!(deleted.name, "Ghost");
        assert!(ctx.all().is_empty());
    }

    #[tokio::test]
    async fn delete_nonexistent_errors() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let err = ctx.delete("01ZZZZZZZZZZZZZZZZZZZZZZZZ").await;
        assert!(err.is_err());
        let msg = format!("{}", err.unwrap_err());
        assert!(msg.contains("perspective"));
        assert!(msg.contains("not found"));
    }

    #[tokio::test]
    async fn persistence_survives_reopen() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Write two perspectives
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
                .await
                .unwrap();
            ctx.write(&make_rich_perspective(
                "01BBBBBBBBBBBBBBBBBBBBBBBB",
                "Beta",
            ))
            .await
            .unwrap();
            assert_eq!(ctx.all().len(), 2);
        }

        // Reopen and verify everything is still there
        {
            let ctx = PerspectiveContext::open(&dir).await.unwrap();
            assert_eq!(ctx.all().len(), 2);

            let alpha = ctx.get_by_name("Alpha").unwrap();
            assert_eq!(alpha.id, "01AAAAAAAAAAAAAAAAAAAAAAAA");

            let beta = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").unwrap();
            assert_eq!(beta.name, "Beta");
            assert_eq!(beta.fields.len(), 1);
            assert!(beta.filter.is_some());
        }
    }

    #[tokio::test]
    async fn write_updates_existing() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let mut p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Original");
        ctx.write(&p).await.unwrap();

        // Update name
        p.name = "Renamed".to_string();
        ctx.write(&p).await.unwrap();

        assert_eq!(ctx.all().len(), 1);
        assert!(ctx.get_by_name("Original").is_none());
        assert!(ctx.get_by_name("Renamed").is_some());
        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap().name,
            "Renamed"
        );
    }

    #[tokio::test]
    async fn duplicate_name_rejected_on_add() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Sprint View"))
            .await
            .unwrap();

        // Second perspective with a different ID but same name should fail
        let err = ctx
            .write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Sprint View"))
            .await;
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("Sprint View"));
        assert!(msg.contains("already exists"));

        // Only one perspective should be in the store
        assert_eq!(ctx.all().len(), 1);
        // Name index should still point to the original
        assert_eq!(
            ctx.get_by_name("Sprint View").unwrap().id,
            "01AAAAAAAAAAAAAAAAAAAAAAAA"
        );
    }

    #[tokio::test]
    async fn duplicate_name_rejected_on_rename() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
            .await
            .unwrap();

        // Renaming Beta to Alpha should fail
        let mut renamed = make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Alpha");
        renamed.view = "grid".to_string();
        let err = ctx.write(&renamed).await;
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("Alpha"));
        assert!(msg.contains("already exists"));

        // Beta should still have its original name
        assert_eq!(
            ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").unwrap().name,
            "Beta"
        );
    }

    #[tokio::test]
    async fn rename_to_own_name_succeeds() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Stable");
        ctx.write(&p).await.unwrap();

        // Writing the same perspective with the same name should succeed (no-op rename)
        ctx.write(&p).await.unwrap();
        assert_eq!(ctx.all().len(), 1);
    }

    /// Deleting the first element (not the last) exercises the swap_remove index
    /// fixup at lines 156-159. After deletion the remaining perspectives must
    /// still be findable by both ID and name.
    #[tokio::test]
    async fn delete_first_element_fixes_swap_remove_indexes() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        // Insert three perspectives -- internal order is [A, B, C]
        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "Gamma"))
            .await
            .unwrap();
        assert_eq!(ctx.all().len(), 3);

        // Delete the FIRST perspective -- swap_remove swaps last element into idx 0
        let deleted = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert_eq!(deleted.name, "Alpha");
        assert_eq!(ctx.all().len(), 2);

        // The deleted perspective must be gone from both indexes
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none());
        assert!(ctx.get_by_name("Alpha").is_none());

        // Beta and Gamma must still be reachable by both ID and name
        let beta_by_id = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB");
        assert!(beta_by_id.is_some(), "Beta must be findable by ID");
        assert_eq!(beta_by_id.unwrap().name, "Beta");

        let beta_by_name = ctx.get_by_name("Beta");
        assert!(beta_by_name.is_some(), "Beta must be findable by name");
        assert_eq!(beta_by_name.unwrap().id, "01BBBBBBBBBBBBBBBBBBBBBBBB");

        let gamma_by_id = ctx.get_by_id("01CCCCCCCCCCCCCCCCCCCCCCCC");
        assert!(gamma_by_id.is_some(), "Gamma must be findable by ID");
        assert_eq!(gamma_by_id.unwrap().name, "Gamma");

        let gamma_by_name = ctx.get_by_name("Gamma");
        assert!(gamma_by_name.is_some(), "Gamma must be findable by name");
        assert_eq!(gamma_by_name.unwrap().id, "01CCCCCCCCCCCCCCCCCCCCCCCC");
    }

    /// Deleting the middle element also exercises swap_remove index fixup.
    /// The last element is swapped into the middle slot.
    #[tokio::test]
    async fn delete_middle_element_fixes_swap_remove_indexes() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "Gamma"))
            .await
            .unwrap();

        // Delete the middle element
        let deleted = ctx.delete("01BBBBBBBBBBBBBBBBBBBBBBBB").await.unwrap();
        assert_eq!(deleted.name, "Beta");
        assert_eq!(ctx.all().len(), 2);

        assert!(ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").is_none());
        assert!(ctx.get_by_name("Beta").is_none());

        // Alpha and Gamma must still be reachable
        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap().name,
            "Alpha"
        );
        assert_eq!(
            ctx.get_by_name("Alpha").unwrap().id,
            "01AAAAAAAAAAAAAAAAAAAAAAAA"
        );
        assert_eq!(
            ctx.get_by_id("01CCCCCCCCCCCCCCCCCCCCCCCC").unwrap().name,
            "Gamma"
        );
        assert_eq!(
            ctx.get_by_name("Gamma").unwrap().id,
            "01CCCCCCCCCCCCCCCCCCCCCCCC"
        );
    }

    /// When remove_file hits a non-NotFound IO error, delete must propagate it
    /// as `PerspectiveError::Io`. We induce this by replacing the YAML file with
    /// a directory -- `remove_file` on a directory yields an OS error.
    #[tokio::test]
    async fn delete_propagates_non_not_found_io_error() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write(&p).await.unwrap();

        // Replace the YAML file with a directory so remove_file fails with a
        // non-NotFound error (IsADirectory / PermissionDenied depending on OS).
        let file_path = tmp
            .path()
            .join("perspectives")
            .join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml");
        fs::remove_file(&file_path).await.unwrap();
        fs::create_dir(&file_path).await.unwrap();

        let err = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await;
        assert!(err.is_err(), "delete should fail with IO error");
        let msg = err.unwrap_err().to_string();
        assert!(
            msg.contains("IO error"),
            "expected IO error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn load_all_ignores_non_yaml_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Bootstrap with a valid perspective
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Valid"))
                .await
                .unwrap();
        }

        // Drop non-.yaml files into the directory
        fs::write(dir.join("notes.txt"), b"some text").await.unwrap();
        fs::write(dir.join("config.json"), b"{}").await.unwrap();
        fs::write(dir.join("noext"), b"data").await.unwrap();

        // Reopen -- non-yaml files must be silently ignored
        let ctx = PerspectiveContext::open(&dir).await.unwrap();
        assert_eq!(ctx.all().len(), 1);
        assert_eq!(ctx.get_by_name("Valid").unwrap().id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[tokio::test]
    async fn load_all_skips_malformed_yaml() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Bootstrap with a valid perspective
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Good"))
                .await
                .unwrap();
        }

        // Write a malformed YAML file that will fail deserialization
        fs::write(dir.join("01BADBADBADBADBADBADBADBAD.yaml"), b"not: [valid: yaml: {{")
            .await
            .unwrap();

        // Reopen -- malformed file must be skipped, valid file still loads
        let ctx = PerspectiveContext::open(&dir).await.unwrap();
        assert_eq!(ctx.all().len(), 1);
        assert_eq!(ctx.get_by_name("Good").unwrap().id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[tokio::test]
    async fn open_fresh_directory_creates_it_and_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("brand_new_perspectives");
        assert!(!dir.exists(), "directory must not exist before open");

        let ctx = PerspectiveContext::open(&dir).await.unwrap();

        assert!(dir.exists(), "open() must create the directory");
        assert!(dir.is_dir(), "created path must be a directory");
        assert!(ctx.all().is_empty(), "fresh context must have no perspectives");
        assert_eq!(ctx.root(), dir);
    }

    #[tokio::test]
    async fn open_loads_preexisting_yaml_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        fs::create_dir_all(&dir).await.unwrap();

        // Write YAML files directly (not via PerspectiveContext) to simulate pre-existing data
        let p1 = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Pre A");
        let p2 = make_rich_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Pre B");
        let p3 = make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "Pre C");

        for p in [&p1, &p2, &p3] {
            let yaml = serde_yaml_ng::to_string(p).unwrap();
            fs::write(dir.join(format!("{}.yaml", p.id)), yaml.as_bytes())
                .await
                .unwrap();
        }

        let ctx = PerspectiveContext::open(&dir).await.unwrap();

        assert_eq!(ctx.all().len(), 3, "all three pre-existing perspectives must load");
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_some());
        assert!(ctx.get_by_name("Pre B").is_some());
        // Verify rich fields survived the round-trip
        let b = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").unwrap();
        assert_eq!(b.view, "grid");
        assert_eq!(b.fields.len(), 1);
        assert!(b.filter.is_some());
        assert_eq!(b.sort.len(), 1);
    }

    #[tokio::test]
    async fn open_write_close_reopen_round_trip() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("round_trip_perspectives");
        assert!(!dir.exists());

        // Phase 1: open fresh, write, drop (close)
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            assert!(dir.exists(), "directory created on first open");
            assert!(ctx.all().is_empty());

            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
                .await
                .unwrap();
            ctx.write(&make_rich_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
                .await
                .unwrap();
            assert_eq!(ctx.all().len(), 2);
        }
        // ctx dropped here -- simulates close

        // Phase 2: reopen and verify persistence
        {
            let ctx = PerspectiveContext::open(&dir).await.unwrap();
            assert_eq!(ctx.all().len(), 2, "both perspectives must survive reopen");

            let alpha = ctx.get_by_name("Alpha").unwrap();
            assert_eq!(alpha.id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
            assert_eq!(alpha.view, "board");

            let beta = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").unwrap();
            assert_eq!(beta.name, "Beta");
            assert_eq!(beta.view, "grid");
            assert_eq!(beta.fields.len(), 1);
            assert!(beta.filter.is_some());
            assert!(beta.group.is_some());
            assert_eq!(beta.sort.len(), 1);
        }

        // Verify YAML files exist on disk
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());
        assert!(dir.join("01BBBBBBBBBBBBBBBBBBBBBBBB.yaml").exists());
    }

    #[tokio::test]
    async fn load_all_returns_ok_when_root_missing() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Create and immediately remove the directory so load_all hits the early return
        fs::create_dir_all(&dir).await.unwrap();
        fs::remove_dir(&dir).await.unwrap();
        assert!(!dir.exists());

        // Build the context manually to bypass open()'s create_dir_all
        let mut ctx = PerspectiveContext {
            root: dir,
            perspectives: Vec::new(),
            id_index: HashMap::new(),
            name_index: HashMap::new(),
        };

        // load_all should return Ok with zero perspectives
        ctx.load_all().await.unwrap();
        assert!(ctx.all().is_empty());
    }
}
