//! KanbanContext - I/O primitives for kanban storage
//!
//! The context provides access to storage and utilities. No business logic methods,
//! just data access primitives. Commands do all the work.

use crate::defaults::kanban_defaults;
use crate::error::{KanbanError, Result};
use crate::types::{
    Actor, ActorId, Attachment, Board, Column, ColumnId, Comment, LogEntry, Position, Swimlane,
    SwimlaneId, Tag, TagId, Task, TaskId,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use swissarmyhammer_fields::FieldsContext;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Context passed to every command - provides access, not logic
pub struct KanbanContext {
    /// Path to the .kanban directory
    root: PathBuf,
    /// Field registry (populated via `open()`, None when created via `new()`)
    fields: Option<FieldsContext>,
}

impl KanbanContext {
    /// Create a new context for the given .kanban directory.
    ///
    /// This is a lightweight synchronous constructor. The field registry is
    /// not initialized — use `open()` for a fully-initialized context.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            fields: None,
        }
    }

    /// Create a fully-initialized context with field registry.
    ///
    /// Opens (or creates) the `fields/` directory under root and seeds
    /// built-in kanban field definitions and entity templates.
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let fields = FieldsContext::open(root.join("fields"))
            .with_defaults(kanban_defaults())
            .build()
            .await
            .map_err(|e| KanbanError::FieldsError(e.to_string()))?;
        Ok(Self {
            root,
            fields: Some(fields),
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
        self.fields.as_ref()
    }

    // =========================================================================
    // Path helpers
    // =========================================================================

    /// Get the root .kanban directory
    pub fn root(&self) -> &Path {
        &self.root
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

    /// Path to swimlanes directory
    pub fn swimlanes_dir(&self) -> PathBuf {
        self.root.join("swimlanes")
    }

    /// Path to a swimlane's YAML file
    pub fn swimlane_path(&self, id: &SwimlaneId) -> PathBuf {
        self.root.join("swimlanes").join(format!("{}.yaml", id))
    }

    /// Path to a swimlane's log file
    pub fn swimlane_log_path(&self, id: &SwimlaneId) -> PathBuf {
        self.root.join("swimlanes").join(format!("{}.jsonl", id))
    }

    /// Path to the activity directory
    pub fn activity_dir(&self) -> PathBuf {
        self.root.join("activity")
    }

    /// Path to the current activity log
    pub fn activity_path(&self) -> PathBuf {
        self.root.join("activity").join("current.jsonl")
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
        self.board_path().exists() || self.root.join("board.json").exists()
    }

    /// Check if all required directories exist
    pub fn directories_exist(&self) -> bool {
        self.root.exists()
            && self.tasks_dir().exists()
            && self.actors_dir().exists()
            && self.tags_dir().exists()
            && self.columns_dir().exists()
            && self.swimlanes_dir().exists()
            && self.activity_dir().exists()
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
        fs::create_dir_all(self.swimlanes_dir()).await?;
        fs::create_dir_all(self.activity_dir()).await?;
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
    // Board I/O
    // =========================================================================

    /// Read the board file, auto-migrating legacy format if needed.
    ///
    /// Tries board.yaml first, falls back to legacy board.json.
    /// If the board contains embedded columns/swimlanes (old format),
    /// they are extracted to individual files and board is rewritten.
    pub async fn read_board(&self) -> Result<Board> {
        let yaml_path = self.board_path(); // board.yaml
        let path = if yaml_path.exists() {
            yaml_path
        } else {
            let json_path = self.root.join("board.json");
            if !json_path.exists() {
                return Err(KanbanError::NotInitialized {
                    path: self.root.clone(),
                });
            }
            json_path
        };

        let content = fs::read_to_string(&path).await?;
        // serde_yaml can parse both YAML and JSON
        let board: Board = serde_yaml::from_str(&content)?;

        // Migrate legacy embedded columns/swimlanes to individual files
        if !board.columns.is_empty() || !board.swimlanes.is_empty() {
            self.ensure_directories().await?;

            for column in &board.columns {
                if !self.column_exists(&column.id).await {
                    self.write_column(column).await?;
                }
            }
            for swimlane in &board.swimlanes {
                if !self.swimlane_exists(&swimlane.id).await {
                    self.write_swimlane(swimlane).await?;
                }
            }

            // Rewrite as board.yaml without embedded columns/swimlanes
            let slim_board = Board::new(&board.name);
            let slim_board = if let Some(ref desc) = board.description {
                slim_board.with_description(desc)
            } else {
                slim_board
            };
            self.write_board(&slim_board).await?;

            // Remove legacy board.json if we migrated from it
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let _ = fs::remove_file(&path).await;
            }

            return Ok(slim_board);
        }

        // Auto-migrate legacy .json to .yaml
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            self.write_board(&board).await?;
            let _ = fs::remove_file(&path).await;
        }

        Ok(board)
    }

    /// Write the board file as YAML (atomic write via temp file)
    pub async fn write_board(&self, board: &Board) -> Result<()> {
        let path = self.board_path();
        let content = serde_yaml::to_string(board)?;
        atomic_write(&path, content.as_bytes()).await
    }

    /// Get the first column (lowest order) from file-based storage
    pub async fn first_column(&self) -> Result<Option<Column>> {
        let columns = self.read_all_columns().await?;
        Ok(columns.into_iter().min_by_key(|c| c.order))
    }

    /// Get the terminal/done column (highest order) from file-based storage
    pub async fn terminal_column(&self) -> Result<Option<Column>> {
        let columns = self.read_all_columns().await?;
        Ok(columns.into_iter().max_by_key(|c| c.order))
    }

    /// Find a column by ID from file-based storage
    pub async fn find_column(&self, id: &ColumnId) -> Result<Option<Column>> {
        match self.read_column(id).await {
            Ok(col) => Ok(Some(col)),
            Err(KanbanError::ColumnNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Find a swimlane by ID from file-based storage
    pub async fn find_swimlane(&self, id: &SwimlaneId) -> Result<Option<Swimlane>> {
        match self.read_swimlane(id).await {
            Ok(sl) => Ok(Some(sl)),
            Err(KanbanError::SwimlaneNotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // =========================================================================
    // Task I/O
    // =========================================================================

    /// Read a task file, auto-migrating legacy formats.
    ///
    /// Tries `.md` (YAML frontmatter + markdown body) first, falls back to `.json`.
    pub async fn read_task(&self, id: &TaskId) -> Result<Task> {
        let md_path = self.task_path(id); // .md
        let path = if md_path.exists() {
            md_path
        } else {
            // Fall back to legacy .json
            let json_path = self.root.join("tasks").join(format!("{}.json", id));
            if !json_path.exists() {
                return Err(KanbanError::TaskNotFound { id: id.to_string() });
            }
            json_path
        };

        let content = fs::read_to_string(&path).await?;

        let mut task = if path.extension().and_then(|s| s.to_str()) == Some("md") {
            parse_task_markdown(&content)?
        } else {
            // Legacy JSON
            serde_json::from_str(&content)?
        };

        task.id = id.clone();

        // Migrate legacy subtasks to markdown checklists in description
        if task.migrate_legacy_subtasks() {
            self.write_task(&task).await?;
        }

        // Auto-migrate legacy .json to .md
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            self.write_task(&task).await?;
            // Remove old .json file after successful .md write
            let _ = fs::remove_file(&path).await;
        }

        Ok(task)
    }

    /// Write a task file as YAML frontmatter + markdown body
    pub async fn write_task(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let meta = TaskMeta::from_task(task);
        let frontmatter = serde_yaml::to_string(&meta)?;
        let content = format!("---\n{}---\n{}", frontmatter, task.description);
        atomic_write(&path, content.as_bytes()).await
    }

    /// Delete a task file and its log (handles both .md and legacy .json)
    pub async fn delete_task_file(&self, id: &TaskId) -> Result<()> {
        let md_path = self.task_path(id); // .md
        let json_path = self.root.join("tasks").join(format!("{}.json", id));
        let log_path = self.task_log_path(id);

        if md_path.exists() {
            fs::remove_file(&md_path).await?;
        }
        if json_path.exists() {
            fs::remove_file(&json_path).await?;
        }
        if log_path.exists() {
            fs::remove_file(&log_path).await?;
        }

        Ok(())
    }

    /// List all task IDs by reading the tasks directory (accepts .md and legacy .json)
    pub async fn list_task_ids(&self) -> Result<Vec<TaskId>> {
        let tasks_dir = self.tasks_dir();
        if !tasks_dir.exists() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&tasks_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("md") || ext == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if seen.insert(stem.to_string()) {
                        ids.push(TaskId::from_string(stem));
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Read all tasks
    pub async fn read_all_tasks(&self) -> Result<Vec<Task>> {
        let ids = self.list_task_ids().await?;
        let mut tasks = Vec::with_capacity(ids.len());

        for id in ids {
            tasks.push(self.read_task(&id).await?);
        }

        Ok(tasks)
    }

    // =========================================================================
    // Actor I/O
    // =========================================================================

    /// Read an actor file (YAML, with JSON fallback)
    pub async fn read_actor(&self, id: &ActorId) -> Result<Actor> {
        let yaml_path = self.actor_path(id); // .yaml
        let path = if yaml_path.exists() {
            yaml_path
        } else {
            let json_path = self.root.join("actors").join(format!("{}.json", id));
            if !json_path.exists() {
                return Err(KanbanError::ActorNotFound { id: id.to_string() });
            }
            json_path
        };

        let content = fs::read_to_string(&path).await?;
        let mut actor: Actor = serde_yaml::from_str(&content)?;
        actor.set_id(id.clone());

        // Auto-migrate legacy .json to .yaml
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            self.write_actor(&actor).await?;
            let _ = fs::remove_file(&path).await;
        }

        Ok(actor)
    }

    /// Write an actor file as YAML (atomic write via temp file)
    pub async fn write_actor(&self, actor: &Actor) -> Result<()> {
        let path = self.actor_path(actor.id());
        let content = serde_yaml::to_string(actor)?;
        atomic_write(&path, content.as_bytes()).await
    }

    /// Delete an actor file and its log (handles both .yaml and legacy .json)
    pub async fn delete_actor_file(&self, id: &ActorId) -> Result<()> {
        let yaml_path = self.actor_path(id);
        let json_path = self.root.join("actors").join(format!("{}.json", id));
        let log_path = self.actor_log_path(id);

        if yaml_path.exists() {
            fs::remove_file(&yaml_path).await?;
        }
        if json_path.exists() {
            fs::remove_file(&json_path).await?;
        }
        if log_path.exists() {
            fs::remove_file(&log_path).await?;
        }

        Ok(())
    }

    /// List all actor IDs by reading the actors directory (accepts .yaml and legacy .json)
    pub async fn list_actor_ids(&self) -> Result<Vec<ActorId>> {
        let actors_dir = self.actors_dir();
        if !actors_dir.exists() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&actors_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("yaml") || ext == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if seen.insert(stem.to_string()) {
                        ids.push(ActorId::from_string(stem));
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Read all actors
    pub async fn read_all_actors(&self) -> Result<Vec<Actor>> {
        let ids = self.list_actor_ids().await?;
        let mut actors = Vec::with_capacity(ids.len());

        for id in ids {
            actors.push(self.read_actor(&id).await?);
        }

        Ok(actors)
    }

    /// Check if an actor exists (checks .yaml and legacy .json)
    pub async fn actor_exists(&self, id: &ActorId) -> bool {
        self.actor_path(id).exists()
            || self.root.join("actors").join(format!("{}.json", id)).exists()
    }

    // =========================================================================
    // Tag I/O
    // =========================================================================

    /// Read a tag file by ULID (YAML, with JSON fallback)
    pub async fn read_tag(&self, id: &TagId) -> Result<Tag> {
        let yaml_path = self.tag_path(id); // .yaml
        let path = if yaml_path.exists() {
            yaml_path
        } else {
            let json_path = self.root.join("tags").join(format!("{}.json", id));
            if !json_path.exists() {
                return Err(KanbanError::TagNotFound { id: id.to_string() });
            }
            json_path
        };

        let content = fs::read_to_string(&path).await?;
        let mut tag: Tag = serde_yaml::from_str(&content)?;
        tag.id = id.clone();

        // Auto-migrate legacy .json to .yaml
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            self.write_tag(&tag).await?;
            let _ = fs::remove_file(&path).await;
        }

        Ok(tag)
    }

    /// Write a tag file as YAML (atomic write via temp file)
    pub async fn write_tag(&self, tag: &Tag) -> Result<()> {
        let path = self.tag_path(&tag.id);
        let content = serde_yaml::to_string(tag)?;
        atomic_write(&path, content.as_bytes()).await
    }

    /// Delete a tag file and its log (handles both .yaml and legacy .json)
    pub async fn delete_tag_file(&self, id: &TagId) -> Result<()> {
        let yaml_path = self.tag_path(id);
        let json_path = self.root.join("tags").join(format!("{}.json", id));
        let log_path = self.tag_log_path(id);

        if yaml_path.exists() {
            fs::remove_file(&yaml_path).await?;
        }
        if json_path.exists() {
            fs::remove_file(&json_path).await?;
        }
        if log_path.exists() {
            fs::remove_file(&log_path).await?;
        }

        Ok(())
    }

    /// List all tag IDs by reading the tags directory (accepts .yaml and legacy .json)
    pub async fn list_tag_ids(&self) -> Result<Vec<TagId>> {
        let tags_dir = self.tags_dir();
        if !tags_dir.exists() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&tags_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("yaml") || ext == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if seen.insert(stem.to_string()) {
                        ids.push(TagId::from_string(stem));
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Read all tags
    pub async fn read_all_tags(&self) -> Result<Vec<Tag>> {
        let ids = self.list_tag_ids().await?;
        let mut tags = Vec::with_capacity(ids.len());

        for id in ids {
            tags.push(self.read_tag(&id).await?);
        }

        Ok(tags)
    }

    /// Check if a tag exists by ULID (checks .yaml and legacy .json)
    pub async fn tag_exists(&self, id: &TagId) -> bool {
        self.tag_path(id).exists()
            || self.root.join("tags").join(format!("{}.json", id)).exists()
    }

    /// Find a tag by its human-readable name (slug).
    ///
    /// Scans all tag files. Returns None if no tag has that name.
    pub async fn find_tag_by_name(&self, name: &str) -> Result<Option<Tag>> {
        let tags = self.read_all_tags().await?;
        Ok(tags.into_iter().find(|t| t.name == name))
    }

    /// Check if a tag with the given name exists.
    pub async fn tag_name_exists(&self, name: &str) -> Result<bool> {
        Ok(self.find_tag_by_name(name).await?.is_some())
    }

    // =========================================================================
    // Column I/O
    // =========================================================================

    /// Read a column file (YAML, with JSON fallback)
    pub async fn read_column(&self, id: &ColumnId) -> Result<Column> {
        let yaml_path = self.column_path(id); // .yaml
        let path = if yaml_path.exists() {
            yaml_path
        } else {
            let json_path = self.root.join("columns").join(format!("{}.json", id));
            if !json_path.exists() {
                return Err(KanbanError::ColumnNotFound { id: id.to_string() });
            }
            json_path
        };

        let content = fs::read_to_string(&path).await?;
        let mut column: Column = serde_yaml::from_str(&content)?;
        column.id = id.clone();

        // Auto-migrate legacy .json to .yaml
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            self.write_column(&column).await?;
            let _ = fs::remove_file(&path).await;
        }

        Ok(column)
    }

    /// Write a column file as YAML (atomic write via temp file)
    pub async fn write_column(&self, column: &Column) -> Result<()> {
        let path = self.column_path(&column.id);
        let content = serde_yaml::to_string(column)?;
        atomic_write(&path, content.as_bytes()).await
    }

    /// Delete a column file and its log (handles both .yaml and legacy .json)
    pub async fn delete_column_file(&self, id: &ColumnId) -> Result<()> {
        let yaml_path = self.column_path(id);
        let json_path = self.root.join("columns").join(format!("{}.json", id));
        let log_path = self.column_log_path(id);

        if yaml_path.exists() {
            fs::remove_file(&yaml_path).await?;
        }
        if json_path.exists() {
            fs::remove_file(&json_path).await?;
        }
        if log_path.exists() {
            fs::remove_file(&log_path).await?;
        }

        Ok(())
    }

    /// List all column IDs by reading the columns directory (accepts .yaml and legacy .json)
    pub async fn list_column_ids(&self) -> Result<Vec<ColumnId>> {
        let columns_dir = self.columns_dir();
        if !columns_dir.exists() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&columns_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("yaml") || ext == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if seen.insert(stem.to_string()) {
                        ids.push(ColumnId::from_string(stem));
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Read all columns
    pub async fn read_all_columns(&self) -> Result<Vec<Column>> {
        let ids = self.list_column_ids().await?;
        let mut columns = Vec::with_capacity(ids.len());

        for id in ids {
            columns.push(self.read_column(&id).await?);
        }

        Ok(columns)
    }

    /// Check if a column exists (checks .yaml and legacy .json)
    pub async fn column_exists(&self, id: &ColumnId) -> bool {
        self.column_path(id).exists()
            || self.root.join("columns").join(format!("{}.json", id)).exists()
    }

    // =========================================================================
    // Swimlane I/O
    // =========================================================================

    /// Read a swimlane file (YAML, with JSON fallback)
    pub async fn read_swimlane(&self, id: &SwimlaneId) -> Result<Swimlane> {
        let yaml_path = self.swimlane_path(id); // .yaml
        let path = if yaml_path.exists() {
            yaml_path
        } else {
            let json_path = self.root.join("swimlanes").join(format!("{}.json", id));
            if !json_path.exists() {
                return Err(KanbanError::SwimlaneNotFound { id: id.to_string() });
            }
            json_path
        };

        let content = fs::read_to_string(&path).await?;
        let mut swimlane: Swimlane = serde_yaml::from_str(&content)?;
        swimlane.id = id.clone();

        // Auto-migrate legacy .json to .yaml
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            self.write_swimlane(&swimlane).await?;
            let _ = fs::remove_file(&path).await;
        }

        Ok(swimlane)
    }

    /// Write a swimlane file as YAML (atomic write via temp file)
    pub async fn write_swimlane(&self, swimlane: &Swimlane) -> Result<()> {
        let path = self.swimlane_path(&swimlane.id);
        let content = serde_yaml::to_string(swimlane)?;
        atomic_write(&path, content.as_bytes()).await
    }

    /// Delete a swimlane file and its log (handles both .yaml and legacy .json)
    pub async fn delete_swimlane_file(&self, id: &SwimlaneId) -> Result<()> {
        let yaml_path = self.swimlane_path(id);
        let json_path = self.root.join("swimlanes").join(format!("{}.json", id));
        let log_path = self.swimlane_log_path(id);

        if yaml_path.exists() {
            fs::remove_file(&yaml_path).await?;
        }
        if json_path.exists() {
            fs::remove_file(&json_path).await?;
        }
        if log_path.exists() {
            fs::remove_file(&log_path).await?;
        }

        Ok(())
    }

    /// List all swimlane IDs by reading the swimlanes directory (accepts .yaml and legacy .json)
    pub async fn list_swimlane_ids(&self) -> Result<Vec<SwimlaneId>> {
        let swimlanes_dir = self.swimlanes_dir();
        if !swimlanes_dir.exists() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&swimlanes_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("yaml") || ext == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if seen.insert(stem.to_string()) {
                        ids.push(SwimlaneId::from_string(stem));
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Read all swimlanes
    pub async fn read_all_swimlanes(&self) -> Result<Vec<Swimlane>> {
        let ids = self.list_swimlane_ids().await?;
        let mut swimlanes = Vec::with_capacity(ids.len());

        for id in ids {
            swimlanes.push(self.read_swimlane(&id).await?);
        }

        Ok(swimlanes)
    }

    /// Check if a swimlane exists (checks .yaml and legacy .json)
    pub async fn swimlane_exists(&self, id: &SwimlaneId) -> bool {
        self.swimlane_path(id).exists()
            || self
                .root
                .join("swimlanes")
                .join(format!("{}.json", id))
                .exists()
    }

    // =========================================================================
    // Activity logging
    // =========================================================================

    /// Append a log entry to the global activity log
    pub async fn append_activity(&self, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.activity_path(), entry).await
    }

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

    /// Append a log entry to a swimlane's log
    pub async fn append_swimlane_log(&self, id: &SwimlaneId, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.swimlane_log_path(id), entry).await
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

    /// Read activity log entries (from current.jsonl)
    pub async fn read_activity(&self, limit: Option<usize>) -> Result<Vec<LogEntry>> {
        let path = self.activity_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path).await?;
        let mut entries: Vec<LogEntry> = content
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        // Reverse to get newest first
        entries.reverse();

        if let Some(limit) = limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    // =========================================================================
    // Locking
    // =========================================================================

    // =========================================================================
    // Storage migration
    // =========================================================================

    /// Migrate all entity files from legacy JSON format to YAML/Markdown.
    ///
    /// This triggers a read+write cycle on every entity, which auto-converts:
    /// - `board.json` → `board.yaml`
    /// - `tasks/*.json` → `tasks/*.md` (YAML frontmatter + markdown body)
    /// - `tags/*.json` → `tags/*.yaml`
    /// - `columns/*.json` → `columns/*.yaml`
    /// - `swimlanes/*.json` → `swimlanes/*.yaml`
    /// - `actors/*.json` → `actors/*.yaml`
    ///
    /// Returns counts of migrated entities. Safe to run multiple times (idempotent).
    pub async fn migrate_storage(&self) -> Result<MigrationStats> {
        let mut stats = MigrationStats::default();

        // Board
        if self.root.join("board.json").exists() {
            self.read_board().await?;
            stats.board = true;
        }

        // Tasks
        let task_ids = self.list_task_ids().await?;
        for id in &task_ids {
            let json_path = self.root.join("tasks").join(format!("{}.json", id));
            if json_path.exists() {
                self.read_task(id).await?;
                stats.tasks += 1;
            }
        }

        // Tags
        let tag_ids = self.list_tag_ids().await?;
        for id in &tag_ids {
            let json_path = self.root.join("tags").join(format!("{}.json", id));
            if json_path.exists() {
                self.read_tag(id).await?;
                stats.tags += 1;
            }
        }

        // Columns
        let col_ids = self.list_column_ids().await?;
        for id in &col_ids {
            let json_path = self.root.join("columns").join(format!("{}.json", id));
            if json_path.exists() {
                self.read_column(id).await?;
                stats.columns += 1;
            }
        }

        // Swimlanes
        let sl_ids = self.list_swimlane_ids().await?;
        for id in &sl_ids {
            let json_path = self.root.join("swimlanes").join(format!("{}.json", id));
            if json_path.exists() {
                self.read_swimlane(id).await?;
                stats.swimlanes += 1;
            }
        }

        // Actors
        let actor_ids = self.list_actor_ids().await?;
        for id in &actor_ids {
            let json_path = self.root.join("actors").join(format!("{}.json", id));
            if json_path.exists() {
                self.read_actor(id).await?;
                stats.actors += 1;
            }
        }

        Ok(stats)
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
            Ok(()) => Ok(KanbanLock {
                file,
                path: lock_path,
            }),
            Err(_) => Err(KanbanError::LockBusy),
        }
    }
}

/// Statistics from a storage migration run
#[derive(Debug, Default)]
pub struct MigrationStats {
    /// Whether the board was migrated
    pub board: bool,
    /// Number of tasks migrated
    pub tasks: usize,
    /// Number of tags migrated
    pub tags: usize,
    /// Number of columns migrated
    pub columns: usize,
    /// Number of swimlanes migrated
    pub swimlanes: usize,
    /// Number of actors migrated
    pub actors: usize,
}

impl MigrationStats {
    /// Total number of entities migrated
    pub fn total(&self) -> usize {
        (if self.board { 1 } else { 0 })
            + self.tasks
            + self.tags
            + self.columns
            + self.swimlanes
            + self.actors
    }
}

impl std::fmt::Display for MigrationStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Migrated {} entities (board: {}, tasks: {}, tags: {}, columns: {}, swimlanes: {}, actors: {})",
            self.total(),
            self.board,
            self.tasks,
            self.tags,
            self.columns,
            self.swimlanes,
            self.actors,
        )
    }
}

/// RAII lock guard - releases on drop
pub struct KanbanLock {
    file: std::fs::File,
    #[allow(dead_code)]
    path: PathBuf,
}

impl Drop for KanbanLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Helper for YAML frontmatter serialization (everything except description and id)
#[derive(Serialize, Deserialize)]
struct TaskMeta {
    pub title: String,
    #[serde(default, skip_serializing)]
    _legacy_tags: Vec<String>,
    pub position: Position,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<ActorId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
    #[serde(default, skip_serializing, rename = "subtasks")]
    _legacy_subtasks: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
}

impl TaskMeta {
    fn from_task(task: &Task) -> Self {
        Self {
            title: task.title.clone(),
            _legacy_tags: Vec::new(),
            position: task.position.clone(),
            depends_on: task.depends_on.clone(),
            assignees: task.assignees.clone(),
            comments: task.comments.clone(),
            _legacy_subtasks: Vec::new(),
            attachments: task.attachments.clone(),
        }
    }
}

/// Parse a task from YAML frontmatter + markdown body format
fn parse_task_markdown(content: &str) -> Result<Task> {
    // Split on "---" delimiters
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    // parts[0] = "" (before first ---), parts[1] = frontmatter, parts[2] = body
    if parts.len() < 3 {
        return Err(KanbanError::parse(
            "invalid task markdown: missing frontmatter delimiters",
        ));
    }
    let frontmatter = parts[1].trim();
    let body = parts[2].strip_prefix('\n').unwrap_or(parts[2]);

    let meta: TaskMeta = serde_yaml::from_str(frontmatter)?;
    Ok(Task::from_parts(
        meta.title,
        body.to_string(),
        meta.position,
        meta.depends_on,
        meta.assignees,
        meta.comments,
        meta.attachments,
    ))
}

/// Atomic write via temp file and rename
async fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Write to temp file in same directory
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).await?;

    // Rename (atomic on same filesystem)
    fs::rename(&temp_path, path).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Column, ColumnId, Swimlane, SwimlaneId};
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

    #[tokio::test]
    async fn test_board_io() {
        let (_temp, ctx) = setup().await;

        let board = Board::new("Test Board");
        ctx.write_board(&board).await.unwrap();

        let loaded = ctx.read_board().await.unwrap();
        assert_eq!(loaded.name, "Test Board");
    }

    #[tokio::test]
    async fn test_task_io() {
        use crate::types::{ColumnId, Ordinal, Position};

        let (_temp, ctx) = setup().await;

        // Initialize board first
        let board = Board::new("Test");
        ctx.write_board(&board).await.unwrap();

        let task = Task::new(
            "Test Task",
            Position::new(ColumnId::from_string("todo"), None, Ordinal::first()),
        );
        let task_id = task.id.clone();

        ctx.write_task(&task).await.unwrap();

        let loaded = ctx.read_task(&task_id).await.unwrap();
        assert_eq!(loaded.title, "Test Task");

        // List tasks
        let ids = ctx.list_task_ids().await.unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], task_id);

        // Delete
        ctx.delete_task_file(&task_id).await.unwrap();
        let ids = ctx.list_task_ids().await.unwrap();
        assert!(ids.is_empty());
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
        assert!(ctx.activity_dir().exists());
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
    async fn test_swimlane_paths() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");

        assert_eq!(ctx.swimlanes_dir(), root.join("swimlanes"));
        assert_eq!(
            ctx.swimlane_path(&SwimlaneId::from_string("backend")),
            root.join("swimlanes").join("backend.yaml")
        );
        assert_eq!(
            ctx.swimlane_log_path(&SwimlaneId::from_string("backend")),
            root.join("swimlanes").join("backend.jsonl")
        );
    }

    #[tokio::test]
    async fn test_column_io() {
        let (_temp, ctx) = setup().await;

        let column = Column {
            id: ColumnId::from_string("todo"),
            name: "To Do".into(),
            order: 0,
        };

        // Write
        ctx.write_column(&column).await.unwrap();

        // Read
        let loaded = ctx.read_column(&ColumnId::from_string("todo")).await.unwrap();
        assert_eq!(loaded.name, "To Do");
        assert_eq!(loaded.order, 0);

        // List
        let ids = ctx.list_column_ids().await.unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].as_str(), "todo");

        // Read all
        let all = ctx.read_all_columns().await.unwrap();
        assert_eq!(all.len(), 1);

        // Delete
        ctx.delete_column_file(&ColumnId::from_string("todo")).await.unwrap();
        let ids = ctx.list_column_ids().await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_swimlane_io() {
        let (_temp, ctx) = setup().await;

        let swimlane = Swimlane {
            id: SwimlaneId::from_string("backend"),
            name: "Backend".into(),
            order: 0,
        };

        // Write
        ctx.write_swimlane(&swimlane).await.unwrap();

        // Read
        let loaded = ctx.read_swimlane(&SwimlaneId::from_string("backend")).await.unwrap();
        assert_eq!(loaded.name, "Backend");

        // List
        let ids = ctx.list_swimlane_ids().await.unwrap();
        assert_eq!(ids.len(), 1);

        // Read all
        let all = ctx.read_all_swimlanes().await.unwrap();
        assert_eq!(all.len(), 1);

        // Delete
        ctx.delete_swimlane_file(&SwimlaneId::from_string("backend")).await.unwrap();
        let ids = ctx.list_swimlane_ids().await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_column_not_found() {
        let (_temp, ctx) = setup().await;

        let result = ctx.read_column(&ColumnId::from_string("nonexistent")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_swimlane_not_found() {
        let (_temp, ctx) = setup().await;

        let result = ctx.read_swimlane(&SwimlaneId::from_string("nonexistent")).await;
        assert!(result.is_err());
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

        // fields/ directory should exist
        assert!(kanban_dir.join("fields").exists());
        assert!(kanban_dir.join("fields/definitions").exists());
        assert!(kanban_dir.join("fields/entities").exists());

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

        // Should have all 16 built-in fields
        assert_eq!(fields.all_fields().len(), 16);

        // Should have all 5 entity templates
        assert_eq!(fields.all_entities().len(), 5);

        // Check a specific field
        let status = fields.get_field_by_name("status").unwrap();
        assert_eq!(status.name, "status");
    }

    #[tokio::test]
    async fn test_open_preserves_customizations() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();

        // First open — seeds defaults
        {
            let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
            let fields = ctx.fields().unwrap();
            assert_eq!(fields.all_fields().len(), 16);
        }

        // Manually add a custom field to definitions/
        let custom_yaml = r#"id: 0000000000000000000000ZZZZ
name: sprint
type:
  kind: text
  single_line: true
"#;
        tokio::fs::write(
            kanban_dir.join("fields/definitions/sprint.yaml"),
            custom_yaml,
        )
        .await
        .unwrap();

        // Re-open — should have 16 built-in + 1 custom = 17
        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        let fields = ctx.fields().unwrap();
        assert_eq!(fields.all_fields().len(), 17);

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
        assert!(fields.get_field_by_name("status").is_some());
        assert!(fields.get_field_by_name("nonexistent").is_none());

        // Should be able to get entity templates
        assert!(fields.get_entity("task").is_some());
        assert!(fields.get_entity("tag").is_some());
        assert!(fields.get_entity("nonexistent").is_none());

        // Entity fields should resolve to field definitions
        let task_fields = fields.fields_for_entity("task");
        assert!(task_fields.len() >= 7); // title, status, priority, tags, assignees, due, depends_on, body
    }
}
