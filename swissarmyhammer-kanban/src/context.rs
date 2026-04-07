//! KanbanContext - I/O primitives for kanban storage
//!
//! The context provides access to storage and utilities. No business logic methods,
//! just data access primitives. Commands do all the work.

use crate::defaults::{
    builtin_actor_entities, builtin_entity_definitions, builtin_field_definitions,
    builtin_view_definitions, kanban_compute_engine, KanbanLookup,
};
use crate::error::{KanbanError, Result};
use crate::types::{ActorId, ColumnId, LogEntry, TagId, TaskId};
use fs2::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use swissarmyhammer_entity::changelog::ChangeEntry;
use swissarmyhammer_entity::{Entity, EntityContext};
use swissarmyhammer_fields::{load_yaml_dir, DeriveRegistry, FieldsContext, ValidationEngine};
use swissarmyhammer_perspectives::PerspectiveContext;
use swissarmyhammer_views::{ViewsChangelog, ViewsContext};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OnceCell, RwLock};

/// Context passed to every command - provides access, not logic
pub struct KanbanContext {
    /// Path to the .kanban directory
    root: PathBuf,
    /// Human-readable context name derived from the parent of `.kanban`.
    /// Computed once in the constructor from `root.parent().file_name()`.
    context_name: String,
    /// Field registry (populated via `open()`, None when created via `new()`)
    fields: Option<Arc<FieldsContext>>,
    /// Entity I/O coordinator — lazy-initialized on first access, wrapped in Arc
    /// so it can be shared as a CommandContext extension independently of KanbanContext.
    entities: OnceCell<Arc<EntityContext>>,
    /// View registry (populated via `open()`, None when created via `new()`)
    views: Option<RwLock<ViewsContext>>,
    /// View changelog (populated via `open()`, None when created via `new()`)
    views_changelog: Option<ViewsChangelog>,
    /// Perspective registry — lazy-initialized on first access.
    perspectives: OnceCell<RwLock<PerspectiveContext>>,
    /// Derive handlers for computed field read/write
    derive_registry: Arc<DeriveRegistry>,
}

impl KanbanContext {
    /// Derive the context name from a `.kanban` root path.
    ///
    /// Returns the file-stem of the parent directory (e.g.
    /// `/home/user/my-project/.kanban` → `"my-project"`).
    /// Returns an empty string if the parent cannot be determined.
    fn derive_context_name(root: &Path) -> String {
        root.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string()
    }

    /// Create a new context for the given .kanban directory.
    ///
    /// This is a lightweight synchronous constructor. The field registry is
    /// not initialized — use `open()` for a fully-initialized context.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let context_name = Self::derive_context_name(&root);
        Self {
            root,
            context_name,
            fields: None,
            entities: OnceCell::new(),
            views: None,
            views_changelog: None,
            perspectives: OnceCell::new(),
            derive_registry: Arc::new(crate::derive_handlers::kanban_derive_registry()),
        }
    }

    /// Create a fully-initialized context with field registry.
    ///
    /// Loads builtin field/entity YAML definitions (embedded at compile time),
    /// then merges with any local overrides from `.kanban/definitions/` and
    /// `.kanban/entities/`.
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();

        // Ensure definitions/ and entities/ exist for local overrides
        fs::create_dir_all(root.join("definitions")).await?;
        fs::create_dir_all(root.join("entities")).await?;

        let (fields, entities) = Self::build_entity_context(&root)?;
        let cell = OnceCell::new();
        cell.set(Arc::new(entities)).ok();

        // Build views context: seed builtins to disk (if not present), then load all
        let views_root = root.join("views");
        fs::create_dir_all(&views_root).await?;
        Self::seed_builtin_views(&views_root).await?;
        Self::seed_builtin_actors(&root).await?;
        let views = Self::build_views_context(&views_root)?;
        let views_changelog = ViewsChangelog::new(root.join("views.jsonl"));

        // Build perspectives context
        let perspectives_dir = root.join("perspectives");
        fs::create_dir_all(&perspectives_dir).await?;
        let perspectives = PerspectiveContext::open(&perspectives_dir).await?;

        let persp_cell = OnceCell::new();
        persp_cell.set(RwLock::new(perspectives)).ok();

        let context_name = Self::derive_context_name(&root);
        Ok(Self {
            root,
            context_name,
            fields: Some(fields),
            entities: cell,
            views: Some(RwLock::new(views)),
            views_changelog: Some(views_changelog),
            perspectives: persp_cell,
            derive_registry: Arc::new(crate::derive_handlers::kanban_derive_registry()),
        })
    }

    /// Create a context by finding the .kanban directory from a starting path
    pub fn find(start: impl AsRef<Path>) -> Result<Self> {
        let mut current = start.as_ref().to_path_buf();

        loop {
            let kanban_dir = current.join(".kanban");
            if kanban_dir.is_dir() {
                return Ok(Self::new(kanban_dir));
            }

            if !current.pop() {
                return Err(KanbanError::NotInitialized {
                    path: start.as_ref().to_path_buf(),
                });
            }
        }
    }

    /// Access the field registry, if initialized.
    pub fn fields(&self) -> Option<&FieldsContext> {
        self.fields.as_deref()
    }

    /// Access the derive handler registry.
    pub fn derive_registry(&self) -> &DeriveRegistry {
        &self.derive_registry
    }

    /// Access the view registry lock, if initialized.
    pub fn views(&self) -> Option<&RwLock<ViewsContext>> {
        self.views.as_ref()
    }

    /// Access the view changelog, if initialized.
    pub fn views_changelog(&self) -> Option<&ViewsChangelog> {
        self.views_changelog.as_ref()
    }

    /// Access the perspective registry lock, lazy-initializing on first call.
    pub async fn perspective_context(&self) -> Result<&RwLock<PerspectiveContext>> {
        self.perspectives
            .get_or_try_init(|| async {
                let dir = self.perspectives_dir();
                let ctx = PerspectiveContext::open(dir).await?;
                Ok::<RwLock<PerspectiveContext>, KanbanError>(RwLock::new(ctx))
            })
            .await
    }

    // =========================================================================
    // Path helpers
    // =========================================================================

    /// Get the root .kanban directory
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Human-readable context name (path stem above `.kanban`).
    ///
    /// For example, if root is `/home/user/my-project/.kanban`, returns `"my-project"`.
    /// Returns an empty string if the parent directory cannot be determined.
    pub fn name(&self) -> &str {
        &self.context_name
    }

    /// Path to board.yaml
    pub fn board_path(&self) -> PathBuf {
        self.root.join("board.yaml")
    }

    /// Path to tasks directory
    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join("tasks")
    }

    /// Path to a task's markdown file (YAML frontmatter + markdown body)
    pub fn task_path(&self, id: &TaskId) -> PathBuf {
        self.root.join("tasks").join(format!("{}.md", id))
    }

    /// Path to a task's log file
    pub fn task_log_path(&self, id: &TaskId) -> PathBuf {
        self.root.join("tasks").join(format!("{}.jsonl", id))
    }

    /// Path to actors directory
    pub fn actors_dir(&self) -> PathBuf {
        self.root.join("actors")
    }

    /// Path to an actor's YAML file
    pub fn actor_path(&self, id: &ActorId) -> PathBuf {
        self.root.join("actors").join(format!("{}.yaml", id))
    }

    /// Path to tags directory
    pub fn tags_dir(&self) -> PathBuf {
        self.root.join("tags")
    }

    /// Path to a tag's YAML file
    pub fn tag_path(&self, id: &TagId) -> PathBuf {
        self.root.join("tags").join(format!("{}.yaml", id))
    }

    /// Path to columns directory
    pub fn columns_dir(&self) -> PathBuf {
        self.root.join("columns")
    }

    /// Path to a column's YAML file
    pub fn column_path(&self, id: &ColumnId) -> PathBuf {
        self.root.join("columns").join(format!("{}.yaml", id))
    }

    /// Path to a column's log file
    pub fn column_log_path(&self, id: &ColumnId) -> PathBuf {
        self.root.join("columns").join(format!("{}.jsonl", id))
    }

    /// Path to the perspectives directory
    pub fn perspectives_dir(&self) -> PathBuf {
        self.root.join("perspectives")
    }

    /// Path to the lock file
    pub fn lock_path(&self) -> PathBuf {
        self.root.join(".lock")
    }

    // =========================================================================
    // Directory initialization
    // =========================================================================

    /// Check if the board is initialized (checks board.yaml or legacy board.json)
    pub fn is_initialized(&self) -> bool {
        // Check new entity location first, then legacy
        self.root.join("boards").join("board.yaml").exists()
            || self.board_path().exists()
            || self.root.join("board.json").exists()
    }

    /// Check if all required directories exist
    pub fn directories_exist(&self) -> bool {
        self.root.exists()
            && self.tasks_dir().exists()
            && self.actors_dir().exists()
            && self.tags_dir().exists()
            && self.columns_dir().exists()
            && self.perspectives_dir().exists()
    }

    /// Create the directory structure for a new board
    ///
    /// This is idempotent - safe to call multiple times.
    /// Creates the root .kanban directory and all subdirectories.
    pub async fn create_directories(&self) -> Result<()> {
        // Ensure root .kanban directory exists first
        fs::create_dir_all(&self.root).await?;

        // Create all subdirectories
        fs::create_dir_all(self.tasks_dir()).await?;
        fs::create_dir_all(self.actors_dir()).await?;
        fs::create_dir_all(self.tags_dir()).await?;
        fs::create_dir_all(self.columns_dir()).await?;
        fs::create_dir_all(self.perspectives_dir()).await?;
        Ok(())
    }

    /// Ensure directories exist, creating them if needed
    ///
    /// This should be called at the start of operations that need directories.
    /// It's idempotent and fast when directories already exist.
    pub async fn ensure_directories(&self) -> Result<()> {
        if !self.directories_exist() {
            self.create_directories().await?;
        }
        Ok(())
    }

    // =========================================================================
    /// Append a log entry to a task's log
    pub async fn append_task_log(&self, task_id: &TaskId, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.task_log_path(task_id), entry).await
    }

    /// Path to a tag's log file
    pub fn tag_log_path(&self, id: &TagId) -> PathBuf {
        self.root.join("tags").join(format!("{}.jsonl", id))
    }

    /// Path to an actor's log file
    pub fn actor_log_path(&self, id: &ActorId) -> PathBuf {
        self.root.join("actors").join(format!("{}.jsonl", id))
    }

    /// Path to the board log file
    pub fn board_log_path(&self) -> PathBuf {
        self.root.join("board.jsonl")
    }

    /// Append a log entry to a tag's log
    pub async fn append_tag_log(&self, id: &TagId, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.tag_log_path(id), entry).await
    }

    /// Append a log entry to an actor's log
    pub async fn append_actor_log(&self, id: &ActorId, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.actor_log_path(id), entry).await
    }

    /// Append a log entry to a column's log
    pub async fn append_column_log(&self, id: &ColumnId, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.column_log_path(id), entry).await
    }

    /// Append a log entry to the board log
    pub async fn append_board_log(&self, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.board_log_path(), entry).await
    }

    /// Append a log entry to a JSONL file
    async fn append_log(&self, path: &Path, entry: &LogEntry) -> Result<()> {
        let mut line = serde_json::to_string(entry)?;
        line.push('\n');

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    // =========================================================================
    // Generic Entity I/O (delegates to EntityContext)
    // =========================================================================

    /// Get the EntityContext for generic entity operations.
    ///
    /// Lazy-initialized on first access from builtin + local field definitions.
    /// Access the entity context, lazy-initializing on first call.
    ///
    /// Returns `Arc<EntityContext>` so it can be set as a direct extension on
    /// `CommandContext` without going through `KanbanContext`.
    pub async fn entity_context(&self) -> Result<Arc<EntityContext>> {
        let ectx = self
            .entities
            .get_or_try_init(|| async {
                let (_fields, entities) = Self::build_entity_context(&self.root)?;
                Ok::<Arc<EntityContext>, KanbanError>(Arc::new(entities))
            })
            .await?;
        Ok(Arc::clone(ectx))
    }

    /// Build a FieldsContext + EntityContext from builtin and local field definitions.
    ///
    /// Loads builtin YAML definitions (embedded at compile time), then merges
    /// with any local overrides from `.kanban/definitions/` and `.kanban/entities/`.
    /// Does NOT create directories — callers that need dirs should ensure them beforehand.
    fn build_entity_context(root: &Path) -> Result<(Arc<FieldsContext>, EntityContext)> {
        let builtin_defs = builtin_field_definitions();
        let builtin_entities = builtin_entity_definitions();

        // Load local overrides (returns empty vec if dirs don't exist)
        let local_defs = load_yaml_dir(&root.join("definitions"));
        let local_entities = load_yaml_dir(&root.join("entities"));

        let mut all_defs: Vec<(&str, &str)> = builtin_defs.clone();
        let local_def_refs: Vec<(&str, &str)> = local_defs
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        all_defs.extend(local_def_refs);

        let mut all_entities: Vec<(&str, &str)> = builtin_entities.clone();
        let local_entity_refs: Vec<(&str, &str)> = local_entities
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        all_entities.extend(local_entity_refs);

        let fields = Arc::new(
            FieldsContext::from_yaml_sources(root, &all_defs, &all_entities)
                .map_err(|e| KanbanError::FieldsError(e.to_string()))?,
        );

        // Build engines — KanbanLookup uses a bare EntityContext (no engines)
        // to avoid circular dependency.
        let lookup = KanbanLookup::new(root, Arc::clone(&fields));
        let compute = Arc::new(kanban_compute_engine());
        let validation = Arc::new(ValidationEngine::new().with_lookup(lookup));
        let entities = EntityContext::new(root, Arc::clone(&fields))
            .with_compute(compute)
            .with_validation(validation);
        Ok((fields, entities))
    }

    /// Seed builtin view definitions to disk (write only if not already present).
    async fn seed_builtin_views(views_root: &Path) -> Result<()> {
        for (name, yaml) in builtin_view_definitions() {
            // Parse to get the ID for the filename
            let def: swissarmyhammer_views::ViewDef = match serde_yaml_ng::from_str(yaml) {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!(name = %name, %e, "skipping invalid builtin view");
                    continue;
                }
            };
            let path = views_root.join(format!("{}.yaml", def.id));
            if !path.exists() {
                fs::write(&path, yaml).await?;
            }
        }
        Ok(())
    }

    /// Seed builtin actor entities to disk (write only if not already present).
    async fn seed_builtin_actors(root: &Path) -> Result<()> {
        let actors_dir = root.join("actors");
        fs::create_dir_all(&actors_dir).await?;
        for (id, yaml) in builtin_actor_entities() {
            let path = actors_dir.join(format!("{}.yaml", id));
            if !path.exists() {
                fs::write(&path, yaml).await?;
            }
        }
        Ok(())
    }

    /// Build a ViewsContext from builtin + local view definitions.
    fn build_views_context(views_root: &Path) -> Result<ViewsContext> {
        let builtin_views = builtin_view_definitions();
        let local_views = swissarmyhammer_views::load_yaml_dir(views_root);

        let mut all_views: Vec<(&str, &str)> = builtin_views;
        let local_refs: Vec<(&str, &str)> = local_views
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        all_views.extend(local_refs);

        ViewsContext::from_yaml_sources(views_root, &all_views)
            .map_err(|e| KanbanError::ViewsError(e.to_string()))
    }

    /// Read a single entity by type and ID.
    pub async fn read_entity_generic(&self, entity_type: &str, id: &str) -> Result<Entity> {
        Ok(self.entity_context().await?.read(entity_type, id).await?)
    }

    /// Write an entity with automatic changelog.
    ///
    /// Returns `Ok(Some(ulid))` when changes were logged, `Ok(None)` when no changes.
    pub async fn write_entity_generic(&self, entity: &Entity) -> Result<Option<String>> {
        Ok(self
            .entity_context()
            .await?
            .write(entity)
            .await?
            .map(|id| id.to_string()))
    }

    /// Delete an entity by type and ID.
    ///
    /// Returns `Ok(Some(ulid))` when a delete entry was logged, `Ok(None)` otherwise.
    pub async fn delete_entity_generic(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<Option<String>> {
        Ok(self
            .entity_context()
            .await?
            .delete(entity_type, id)
            .await?
            .map(|id| id.to_string()))
    }

    /// List all entities of a given type.
    pub async fn list_entities_generic(&self, entity_type: &str) -> Result<Vec<Entity>> {
        Ok(self.entity_context().await?.list(entity_type).await?)
    }

    /// Read the changelog for an entity.
    pub async fn read_entity_changelog(
        &self,
        entity_type: &str,
        id: &str,
    ) -> Result<Vec<ChangeEntry>> {
        Ok(self
            .entity_context()
            .await?
            .read_changelog(entity_type, id)
            .await?)
    }

    // =========================================================================
    // Locking
    // =========================================================================

    /// Try to acquire an exclusive lock (non-blocking)
    pub async fn lock(&self) -> Result<KanbanLock> {
        let lock_path = self.lock_path();

        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&lock_path)?;

        // Non-blocking lock attempt
        match file.try_lock_exclusive() {
            Ok(()) => Ok(KanbanLock { file }),
            Err(_) => Err(KanbanError::LockBusy),
        }
    }
}

/// RAII lock guard - releases on drop
pub struct KanbanLock {
    file: std::fs::File,
}

impl Drop for KanbanLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ActorId, ColumnId, TagId, TaskId};
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();
        let ctx = KanbanContext::new(kanban_dir);
        ctx.create_directories().await.unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_paths() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");

        assert_eq!(ctx.root(), root);
        assert_eq!(ctx.board_path(), root.join("board.yaml"));
        assert_eq!(ctx.tasks_dir(), root.join("tasks"));
    }

    #[test]
    fn context_name_returns_parent_dir_stem() {
        let ctx = KanbanContext::new("/home/user/my-project/.kanban");
        assert_eq!(ctx.name(), "my-project");
    }

    #[test]
    fn context_name_empty_for_root_kanban() {
        let ctx = KanbanContext::new("/.kanban");
        assert_eq!(ctx.name(), "");
    }

    #[tokio::test]
    async fn test_task_io() {
        let (_temp, ctx) = setup_with_fields().await;

        // Write a task via entity context
        let mut task = swissarmyhammer_entity::Entity::new("task", "01TESTTASK");
        task.set("title", serde_json::json!("Test Task"));
        task.set("position_column", serde_json::json!("todo"));
        task.set("position_ordinal", serde_json::json!("a0"));

        let ectx = ctx.entity_context().await.unwrap();
        ectx.write(&task).await.unwrap();

        // Read back
        let loaded = ectx.read("task", "01TESTTASK").await.unwrap();
        assert_eq!(loaded.get_str("title"), Some("Test Task"));

        // List tasks
        let tasks = ectx.list("task").await.unwrap();
        assert_eq!(tasks.len(), 1);

        // Delete
        ectx.delete("task", "01TESTTASK").await.unwrap();
        let tasks = ectx.list("task").await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn test_locking() {
        let (_temp, ctx) = setup().await;

        // First lock should succeed
        let lock1 = ctx.lock().await.unwrap();

        // Second lock should fail (busy)
        let result = ctx.lock().await;
        assert!(matches!(result, Err(KanbanError::LockBusy)));

        // After dropping, should be able to lock again
        drop(lock1);
        let _lock2 = ctx.lock().await.unwrap();
    }

    #[tokio::test]
    async fn test_create_directories_creates_root() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");

        // DO NOT manually create .kanban directory - that's what we're testing
        let ctx = KanbanContext::new(&kanban_dir);

        // Root should not exist yet
        assert!(!ctx.root().exists());

        // create_directories should create root AND subdirectories
        ctx.create_directories().await.unwrap();

        // Verify root exists
        assert!(ctx.root().exists());
        assert!(ctx.root().is_dir());

        // Verify all subdirectories exist
        assert!(ctx.tasks_dir().exists());
        assert!(ctx.actors_dir().exists());
        assert!(ctx.tags_dir().exists());
        assert!(ctx.perspectives_dir().exists());
    }

    #[tokio::test]
    async fn test_directories_exist_checks_all_dirs() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);

        // Initially nothing exists
        assert!(!ctx.directories_exist());

        // Create only root
        std::fs::create_dir_all(ctx.root()).unwrap();
        assert!(!ctx.directories_exist(), "Should require all subdirs");

        // Create all directories
        ctx.create_directories().await.unwrap();
        assert!(ctx.directories_exist());

        // Remove one subdirectory
        std::fs::remove_dir_all(ctx.actors_dir()).unwrap();
        assert!(!ctx.directories_exist(), "Should detect missing actors dir");
    }

    #[tokio::test]
    async fn test_ensure_directories_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);

        // Call multiple times
        ctx.ensure_directories().await.unwrap();
        ctx.ensure_directories().await.unwrap();
        ctx.ensure_directories().await.unwrap();

        // Should work without errors
        assert!(ctx.directories_exist());
    }

    #[tokio::test]
    async fn test_column_paths() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");

        assert_eq!(ctx.columns_dir(), root.join("columns"));
        assert_eq!(
            ctx.column_path(&ColumnId::from_string("todo")),
            root.join("columns").join("todo.yaml")
        );
        assert_eq!(
            ctx.column_log_path(&ColumnId::from_string("todo")),
            root.join("columns").join("todo.jsonl")
        );
    }

    #[tokio::test]
    async fn test_ensure_directories_recreates_missing() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);

        // Create directories
        ctx.ensure_directories().await.unwrap();
        assert!(ctx.directories_exist());

        // Delete actors directory
        std::fs::remove_dir_all(ctx.actors_dir()).unwrap();
        assert!(!ctx.directories_exist());

        // ensure_directories should recreate it
        ctx.ensure_directories().await.unwrap();
        assert!(ctx.directories_exist());
        assert!(ctx.actors_dir().exists());
    }

    #[tokio::test]
    async fn test_open_creates_fields_directory() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();

        // definitions/ and entities/ should exist as top-level siblings
        assert!(kanban_dir.join("definitions").exists());
        assert!(kanban_dir.join("entities").exists());

        // fields() should return Some
        assert!(ctx.fields().is_some());
    }

    #[tokio::test]
    async fn test_open_seeds_defaults() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        let fields = ctx.fields().unwrap();

        // Should have all 23 built-in fields
        assert_eq!(fields.all_fields().len(), 23);

        // Should have all 7 entity templates
        assert_eq!(fields.all_entities().len(), 7);

        // Check a specific field
        let title = fields.get_field_by_name("title").unwrap();
        assert_eq!(title.name, "title");
    }

    #[tokio::test]
    async fn test_open_preserves_customizations() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let fields_defs_dir = kanban_dir.join("definitions");
        std::fs::create_dir_all(&fields_defs_dir).unwrap();

        // Manually add a custom field to definitions/
        let custom_yaml = r#"id: 0000000000000000000000ZZZZ
name: sprint
type:
  kind: text
  single_line: true
"#;
        tokio::fs::write(fields_defs_dir.join("sprint.yaml"), custom_yaml)
            .await
            .unwrap();

        // Open — should have 23 built-in + 1 custom = 24
        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        let fields = ctx.fields().unwrap();
        assert_eq!(fields.all_fields().len(), 24);

        // Custom field should be present
        let sprint = fields.get_field_by_name("sprint").unwrap();
        assert_eq!(sprint.name, "sprint");
    }

    #[tokio::test]
    async fn test_new_has_no_fields() {
        let (_, ctx) = setup().await;
        assert!(ctx.fields().is_none());
    }

    #[tokio::test]
    async fn test_fields_accessor() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        let fields = ctx.fields().unwrap();

        // Should be able to look up fields by name
        assert!(fields.get_field_by_name("title").is_some());
        assert!(fields.get_field_by_name("body").is_some());
        assert!(fields.get_field_by_name("nonexistent").is_none());

        // Should be able to get entity templates
        assert!(fields.get_entity("task").is_some());
        assert!(fields.get_entity("tag").is_some());
        assert!(fields.get_entity("nonexistent").is_none());

        // Entity fields should resolve to field definitions
        let task_fields = fields.fields_for_entity("task");
        assert_eq!(task_fields.len(), 12); // title, tags, project, progress, assignees, depends_on, body, position_column, position_ordinal, attachments, virtual_tags, filter_tags
    }

    // =========================================================================
    // Generic Entity I/O tests (integration with EntityContext)
    // =========================================================================

    async fn setup_with_fields() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();
        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        ctx.create_directories().await.unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_entity_context_available_after_open() {
        let (_temp, ctx) = setup_with_fields().await;
        assert!(ctx.entity_context().await.is_ok());
    }

    #[tokio::test]
    async fn test_entity_context_lazy_init() {
        let (_temp, ctx) = setup().await;
        ctx.create_directories().await.unwrap();
        // entity_context() lazy-initializes even without explicit open()
        assert!(ctx.entity_context().await.is_ok());
    }

    #[tokio::test]
    async fn test_generic_entity_round_trip_plain_yaml() {
        let (_temp, ctx) = setup_with_fields().await;

        let mut tag = swissarmyhammer_entity::Entity::new("tag", "bug");
        tag.set("tag_name", serde_json::json!("Bug"));
        tag.set("color", serde_json::json!("#ff0000"));

        ctx.write_entity_generic(&tag).await.unwrap();

        let loaded = ctx.read_entity_generic("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug"));
        assert_eq!(loaded.get_str("color"), Some("#ff0000"));
    }

    #[tokio::test]
    async fn test_generic_entity_round_trip_with_body() {
        let (_temp, ctx) = setup_with_fields().await;

        let mut task = swissarmyhammer_entity::Entity::new("task", "01ABC");
        task.set("title", serde_json::json!("Fix the bug"));
        task.set(
            "body",
            serde_json::json!("This needs fixing.\n\n- [ ] Step 1\n- [ ] Step 2"),
        );

        ctx.write_entity_generic(&task).await.unwrap();

        let loaded = ctx.read_entity_generic("task", "01ABC").await.unwrap();
        assert_eq!(loaded.get_str("title"), Some("Fix the bug"));
        assert!(loaded.get_str("body").unwrap().contains("Step 1"));
    }

    #[tokio::test]
    async fn test_generic_entity_list_and_delete() {
        let (_temp, ctx) = setup_with_fields().await;

        let mut t1 = swissarmyhammer_entity::Entity::new("tag", "bug");
        t1.set("tag_name", serde_json::json!("Bug"));
        let mut t2 = swissarmyhammer_entity::Entity::new("tag", "feature");
        t2.set("tag_name", serde_json::json!("Feature"));

        ctx.write_entity_generic(&t1).await.unwrap();
        ctx.write_entity_generic(&t2).await.unwrap();
        assert_eq!(ctx.list_entities_generic("tag").await.unwrap().len(), 2);

        ctx.delete_entity_generic("tag", "bug").await.unwrap();
        assert_eq!(ctx.list_entities_generic("tag").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_entity_error_for_unknown_type() {
        let (_temp, ctx) = setup_with_fields().await;
        assert!(ctx.read_entity_generic("unicorn", "xyz").await.is_err());
    }

    // =========================================================================
    // Views integration tests
    // =========================================================================

    #[tokio::test]
    async fn test_open_creates_views_directory() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        assert!(kanban_dir.join("views").exists());
        assert!(ctx.views().is_some());
    }

    #[tokio::test]
    async fn test_open_seeds_builtin_views() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        let views = ctx.views().unwrap().read().await;

        // Should have at least the board view
        assert!(!views.all_views().is_empty());
        assert!(views.get_by_name("Board").is_some());
    }

    #[tokio::test]
    async fn test_views_accessor() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        let views = ctx.views().unwrap().read().await;

        let board = views.get_by_name("Board").unwrap();
        assert_eq!(board.kind, swissarmyhammer_views::ViewKind::Board);
        assert!(board.entity_type.as_deref() == Some("task"));
    }

    #[tokio::test]
    async fn test_new_has_no_views() {
        let (_, ctx) = setup().await;
        assert!(ctx.views().is_none());
    }

    #[tokio::test]
    async fn test_views_changelog_initialized() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        assert!(ctx.views_changelog().is_some());
    }

    #[tokio::test]
    async fn test_builtin_views_seeded_to_disk() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let _ctx = KanbanContext::open(&kanban_dir).await.unwrap();

        // Check that the board view YAML file was written to disk
        let views_dir = kanban_dir.join("views");
        let board_file = views_dir.join("01JMVIEW0000000000BOARD0.yaml");
        assert!(board_file.exists(), "board view should be seeded to disk");
    }

    // =========================================================================
    // find() tests
    // =========================================================================

    #[test]
    fn find_discovers_kanban_dir_in_current() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        let ctx = KanbanContext::find(temp.path()).unwrap();
        assert_eq!(ctx.root(), kanban_dir);
    }

    #[test]
    fn find_walks_up_to_parent_directory() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Start search from a nested subdirectory
        let nested = temp.path().join("src").join("lib").join("deep");
        std::fs::create_dir_all(&nested).unwrap();

        let ctx = KanbanContext::find(&nested).unwrap();
        assert_eq!(ctx.root(), kanban_dir);
    }

    #[test]
    fn find_returns_error_when_no_kanban_dir() {
        let temp = TempDir::new().unwrap();
        // No .kanban directory created
        let result = KanbanContext::find(temp.path());
        assert!(matches!(result, Err(KanbanError::NotInitialized { .. })));
    }

    #[test]
    fn find_uses_closest_kanban_dir() {
        let temp = TempDir::new().unwrap();
        // Create .kanban at root
        let root_kanban = temp.path().join(".kanban");
        std::fs::create_dir_all(&root_kanban).unwrap();

        // Create .kanban in a subdirectory (closer to search start)
        let sub_kanban = temp.path().join("sub").join(".kanban");
        std::fs::create_dir_all(&sub_kanban).unwrap();

        let ctx = KanbanContext::find(temp.path().join("sub")).unwrap();
        assert_eq!(ctx.root(), sub_kanban);
    }

    // =========================================================================

    #[tokio::test]
    async fn test_append_task_log() {
        let (_temp, ctx) = setup().await;

        let task_id = TaskId::new();
        let entry = LogEntry::new(
            "update task",
            serde_json::json!({"id": task_id.to_string()}),
            serde_json::json!({}),
            Some("alice".into()),
            5,
        );
        ctx.append_task_log(&task_id, &entry).await.unwrap();

        // Verify the file was created at the expected path
        let log_path = ctx.task_log_path(&task_id);
        assert!(log_path.exists());

        // Read back and verify content
        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        let parsed: LogEntry = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.op, "update task");
        assert_eq!(parsed.actor, Some("alice".into()));
    }

    #[tokio::test]
    async fn test_append_tag_log() {
        let (_temp, ctx) = setup().await;

        let tag_id = TagId::new();
        let entry = LogEntry::new(
            "add tag",
            serde_json::json!({}),
            serde_json::json!({}),
            None,
            1,
        );
        ctx.append_tag_log(&tag_id, &entry).await.unwrap();

        let log_path = ctx.tag_log_path(&tag_id);
        assert!(log_path.exists());
    }

    #[tokio::test]
    async fn test_append_actor_log() {
        let (_temp, ctx) = setup().await;

        let actor_id = ActorId::from_string("bob");
        let entry = LogEntry::new(
            "add actor",
            serde_json::json!({}),
            serde_json::json!({}),
            None,
            1,
        );
        ctx.append_actor_log(&actor_id, &entry).await.unwrap();

        let log_path = ctx.actor_log_path(&actor_id);
        assert!(log_path.exists());
    }

    #[tokio::test]
    async fn test_append_column_log() {
        let (_temp, ctx) = setup().await;

        let col_id = ColumnId::from_string("doing");
        let entry = LogEntry::new(
            "add column",
            serde_json::json!({}),
            serde_json::json!({}),
            None,
            1,
        );
        ctx.append_column_log(&col_id, &entry).await.unwrap();

        let log_path = ctx.column_log_path(&col_id);
        assert!(log_path.exists());
    }

    #[tokio::test]
    async fn test_append_board_log() {
        let (_temp, ctx) = setup().await;

        let entry = LogEntry::new(
            "update board",
            serde_json::json!({}),
            serde_json::json!({}),
            None,
            1,
        );
        ctx.append_board_log(&entry).await.unwrap();

        let log_path = ctx.board_log_path();
        assert!(log_path.exists());
    }

    // =========================================================================
    // seed_builtin_views tests
    // =========================================================================

    #[tokio::test]
    async fn test_seed_builtin_views_writes_yaml_files() {
        let temp = TempDir::new().unwrap();
        let views_root = temp.path().join("views");
        std::fs::create_dir_all(&views_root).unwrap();

        KanbanContext::seed_builtin_views(&views_root)
            .await
            .unwrap();

        // At least one view file should have been written
        let entries: Vec<_> = std::fs::read_dir(&views_root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "yaml"))
            .collect();
        assert!(
            !entries.is_empty(),
            "seed_builtin_views should write at least one YAML file"
        );
    }

    #[tokio::test]
    async fn test_seed_builtin_views_does_not_overwrite_existing() {
        let temp = TempDir::new().unwrap();
        let views_root = temp.path().join("views");
        std::fs::create_dir_all(&views_root).unwrap();

        // Seed once
        KanbanContext::seed_builtin_views(&views_root)
            .await
            .unwrap();

        // Overwrite a view file with custom content
        let board_path = views_root.join("01JMVIEW0000000000BOARD0.yaml");
        assert!(board_path.exists());
        std::fs::write(&board_path, "custom: content\n").unwrap();

        // Seed again - should not overwrite
        KanbanContext::seed_builtin_views(&views_root)
            .await
            .unwrap();

        let content = std::fs::read_to_string(&board_path).unwrap();
        assert_eq!(content, "custom: content\n");
    }

    #[tokio::test]
    async fn test_builtin_views_not_overwritten() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let views_dir = kanban_dir.join("views");
        std::fs::create_dir_all(&views_dir).unwrap();

        // Write a customized board view
        let custom_yaml = r#"id: 01JMVIEW0000000000BOARD0
name: My Custom Board
icon: star
kind: board
entity_type: task
card_fields:
  - title
"#;
        std::fs::write(views_dir.join("01JMVIEW0000000000BOARD0.yaml"), custom_yaml).unwrap();

        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        let views = ctx.views().unwrap().read().await;

        // The custom name should be preserved (local override wins)
        let board = views.get_by_id("01JMVIEW0000000000BOARD0").unwrap();
        assert_eq!(board.name, "My Custom Board");
    }

    // =========================================================================
    // Activity logging tests
    // =========================================================================

    /// Helper to create a LogEntry for testing
    fn make_log_entry(op: &str) -> LogEntry {
        LogEntry::new(
            op,
            serde_json::json!({"test": true}),
            serde_json::json!({"ok": true}),
            None,
            10,
        )
    }

    #[tokio::test]
    async fn test_append_task_log_writes_jsonl() {
        let (_temp, ctx) = setup().await;
        let task_id = TaskId::from_string("01TESTTASK00000000000000");
        let entry = make_log_entry("add task");

        ctx.append_task_log(&task_id, &entry).await.unwrap();

        let path = ctx.task_log_path(&task_id);
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: LogEntry = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.op, "add task");
    }

    #[tokio::test]
    async fn test_append_tag_log_writes_jsonl() {
        let (_temp, ctx) = setup().await;
        let tag_id = TagId::from_string("01TESTTAG000000000000000");
        let entry = make_log_entry("add tag");

        ctx.append_tag_log(&tag_id, &entry).await.unwrap();

        let path = ctx.tag_log_path(&tag_id);
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: LogEntry = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.op, "add tag");
    }

    #[tokio::test]
    async fn test_append_actor_log_writes_jsonl() {
        let (_temp, ctx) = setup().await;
        let actor_id = ActorId::from_string("test-actor");
        let entry = make_log_entry("add actor");

        ctx.append_actor_log(&actor_id, &entry).await.unwrap();

        let path = ctx.actor_log_path(&actor_id);
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: LogEntry = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.op, "add actor");
    }

    #[tokio::test]
    async fn test_append_column_log_writes_jsonl() {
        let (_temp, ctx) = setup().await;
        let col_id = ColumnId::from_string("todo");
        let entry = make_log_entry("add column");

        ctx.append_column_log(&col_id, &entry).await.unwrap();

        let path = ctx.column_log_path(&col_id);
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: LogEntry = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.op, "add column");
    }

    #[tokio::test]
    async fn test_append_board_log_writes_jsonl() {
        let (_temp, ctx) = setup().await;
        let entry = make_log_entry("update board");

        ctx.append_board_log(&entry).await.unwrap();

        let path = ctx.board_log_path();
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: LogEntry = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.op, "update board");
    }

    #[tokio::test]
    async fn test_append_log_appends_multiple_entries() {
        let (_temp, ctx) = setup().await;
        let task_id = TaskId::from_string("01TESTTASK00000000000000");

        let e1 = make_log_entry("add task");
        let e2 = make_log_entry("update task");
        ctx.append_task_log(&task_id, &e1).await.unwrap();
        ctx.append_task_log(&task_id, &e2).await.unwrap();

        let path = ctx.task_log_path(&task_id);
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 2);

        let p1: LogEntry = serde_json::from_str(lines[0]).unwrap();
        let p2: LogEntry = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(p1.op, "add task");
        assert_eq!(p2.op, "update task");
    }

    // =========================================================================
    // Path helper tests for tag, actor, board log paths
    // =========================================================================

    #[tokio::test]
    async fn test_tag_log_path() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");
        let tag_id = TagId::from_string("01TESTTAG000000000000000");
        assert_eq!(
            ctx.tag_log_path(&tag_id),
            root.join("tags").join("01TESTTAG000000000000000.jsonl")
        );
    }

    #[tokio::test]
    async fn test_actor_log_path() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");
        let actor_id = ActorId::from_string("alice");
        assert_eq!(
            ctx.actor_log_path(&actor_id),
            root.join("actors").join("alice.jsonl")
        );
    }

    #[tokio::test]
    async fn test_board_log_path() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");
        assert_eq!(ctx.board_log_path(), root.join("board.jsonl"));
    }

    // =========================================================================
    // Entity path helper tests
    // =========================================================================

    #[tokio::test]
    async fn test_task_path() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");
        let task_id = TaskId::from_string("01TESTTASK00000000000000");
        assert_eq!(
            ctx.task_path(&task_id),
            root.join("tasks").join("01TESTTASK00000000000000.md")
        );
    }

    #[tokio::test]
    async fn test_actor_path() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");
        let actor_id = ActorId::from_string("bob");
        assert_eq!(
            ctx.actor_path(&actor_id),
            root.join("actors").join("bob.yaml")
        );
    }

    #[tokio::test]
    async fn test_tag_path() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");
        let tag_id = TagId::from_string("01TESTTAG000000000000000");
        assert_eq!(
            ctx.tag_path(&tag_id),
            root.join("tags").join("01TESTTAG000000000000000.yaml")
        );
    }

    // =========================================================================
    // find() tests
    // =========================================================================

    #[test]
    fn test_find_discovers_kanban_dir() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // find() from the temp root should locate .kanban
        let ctx = KanbanContext::find(temp.path()).unwrap();
        assert_eq!(ctx.root(), kanban_dir);
    }

    #[test]
    fn test_find_discovers_from_subdirectory() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // Create a nested subdirectory
        let sub = temp.path().join("src").join("lib");
        std::fs::create_dir_all(&sub).unwrap();

        // find() from the nested dir should walk up and find .kanban
        let ctx = KanbanContext::find(&sub).unwrap();
        assert_eq!(ctx.root(), kanban_dir);
    }

    #[test]
    fn test_find_returns_error_when_not_initialized() {
        let temp = TempDir::new().unwrap();
        // No .kanban directory created
        let result = KanbanContext::find(temp.path());
        assert!(matches!(result, Err(KanbanError::NotInitialized { .. })));
    }
}
