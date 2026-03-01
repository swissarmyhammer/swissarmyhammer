//! Task types: Task, Attachment, Comment

use super::ids::{ActorId, AttachmentId, CommentId, TaskId};
use super::position::Position;
use serde::{Deserialize, Serialize};

/// A task/card on the kanban board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(skip)]
    pub id: TaskId,
    pub title: String,
    #[serde(default)]
    pub description: String,

    /// Legacy tags field — accepted on read for backward compat, never written.
    /// Tags are now computed from `#tag` patterns in the description.
    #[serde(default, skip_serializing)]
    _legacy_tags: Vec<String>,

    /// Position = column + swimlane + ordinal
    pub position: Position,

    /// Dependencies - creates a DAG
    #[serde(default)]
    pub depends_on: Vec<TaskId>,

    /// Actors assigned to this task
    #[serde(default)]
    pub assignees: Vec<ActorId>,

    /// Comments/discussion thread
    #[serde(default)]
    pub comments: Vec<Comment>,

    /// Legacy subtasks field - ignored on read, never written.
    /// Kept for backward compatibility with existing task JSON files.
    #[serde(default, skip_serializing, rename = "subtasks")]
    _legacy_subtasks: Vec<serde_json::Value>,

    /// Attachments
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

impl Task {
    /// Create a new task with the given title and position
    pub fn new(title: impl Into<String>, position: Position) -> Self {
        Self {
            id: TaskId::new(),
            title: title.into(),
            description: String::new(),
            _legacy_tags: Vec::new(),
            position,
            depends_on: Vec::new(),
            assignees: Vec::new(),
            comments: Vec::new(),
            _legacy_subtasks: Vec::new(),
            attachments: Vec::new(),
        }
    }

    /// Reconstruct a task from parsed frontmatter parts (used by markdown reader)
    pub fn from_parts(
        title: String,
        description: String,
        position: Position,
        depends_on: Vec<TaskId>,
        assignees: Vec<ActorId>,
        comments: Vec<Comment>,
        attachments: Vec<Attachment>,
    ) -> Self {
        Self {
            id: TaskId::new(),
            title,
            description,
            _legacy_tags: Vec::new(),
            position,
            depends_on,
            assignees,
            comments,
            _legacy_subtasks: Vec::new(),
            attachments,
        }
    }

    /// Compute tag names from `#tag` patterns in the description.
    pub fn tags(&self) -> Vec<String> {
        crate::tag_parser::parse_tags(&self.description)
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set dependencies
    pub fn with_depends_on(mut self, deps: Vec<TaskId>) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set assignees
    pub fn with_assignees(mut self, assignees: Vec<ActorId>) -> Self {
        self.assignees = assignees;
        self
    }

    /// Check if this task has legacy subtask data that needs migration.
    pub fn has_legacy_subtasks(&self) -> bool {
        !self._legacy_subtasks.is_empty()
    }

    /// Migrate legacy subtasks into markdown checklist lines in the description.
    /// Returns true if migration occurred.
    pub fn migrate_legacy_subtasks(&mut self) -> bool {
        if self._legacy_subtasks.is_empty() {
            return false;
        }

        let mut checklist_lines = Vec::new();
        for subtask in &self._legacy_subtasks {
            let title = subtask
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("untitled");
            let completed = subtask
                .get("completed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if completed {
                checklist_lines.push(format!("- [x] {}", title));
            } else {
                checklist_lines.push(format!("- [ ] {}", title));
            }
        }

        if !checklist_lines.is_empty() {
            if !self.description.is_empty() && !self.description.ends_with('\n') {
                self.description.push('\n');
            }
            if !self.description.is_empty() {
                self.description.push('\n');
            }
            self.description.push_str(&checklist_lines.join("\n"));
        }

        self._legacy_subtasks.clear();
        true
    }

    /// Calculate progress as fraction of completed markdown checklist items.
    ///
    /// Parses `- [ ]` (incomplete) and `- [x]`/`- [X]` (complete) from the description.
    /// Returns 0.0 if no checklist items are found.
    pub fn progress(&self) -> f64 {
        let (total, completed) = Self::parse_checklist_counts(&self.description);
        if total == 0 {
            return 0.0;
        }
        completed as f64 / total as f64
    }

    /// Parse markdown checklist items from text, returning (total, completed) counts.
    pub fn parse_checklist_counts(text: &str) -> (usize, usize) {
        let mut total = 0usize;
        let mut completed = 0usize;
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("- [ ] ") || trimmed == "- [ ]" {
                total += 1;
            } else if trimmed.starts_with("- [x] ")
                || trimmed.starts_with("- [X] ")
                || trimmed == "- [x]"
                || trimmed == "- [X]"
            {
                total += 1;
                completed += 1;
            }
        }
        (total, completed)
    }

    /// Check if all dependencies are complete (in the given terminal column)
    pub fn is_ready(&self, all_tasks: &[Task], terminal_column_id: &str) -> bool {
        self.depends_on.iter().all(|dep_id| {
            all_tasks
                .iter()
                .find(|t| &t.id == dep_id)
                .map(|t| t.position.column.as_str() == terminal_column_id)
                .unwrap_or(true) // Missing dependency is treated as complete
        })
    }

    /// Get tasks that this task is blocked by (incomplete dependencies)
    pub fn blocked_by(&self, all_tasks: &[Task], terminal_column_id: &str) -> Vec<TaskId> {
        self.depends_on
            .iter()
            .filter(|dep_id| {
                all_tasks
                    .iter()
                    .find(|t| &t.id == *dep_id)
                    .map(|t| t.position.column.as_str() != terminal_column_id)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Get tasks that depend on this task
    pub fn blocks(&self, all_tasks: &[Task]) -> Vec<TaskId> {
        all_tasks
            .iter()
            .filter(|t| t.depends_on.contains(&self.id))
            .map(|t| t.id.clone())
            .collect()
    }

    /// Find a comment by ID
    pub fn find_comment(&self, id: &CommentId) -> Option<&Comment> {
        self.comments.iter().find(|c| &c.id == id)
    }

    /// Find a comment by ID (mutable)
    pub fn find_comment_mut(&mut self, id: &CommentId) -> Option<&mut Comment> {
        self.comments.iter_mut().find(|c| &c.id == id)
    }

    /// Find an attachment by ID
    pub fn find_attachment(&self, id: &AttachmentId) -> Option<&Attachment> {
        self.attachments.iter().find(|a| &a.id == id)
    }

    /// Find an attachment by ID (mutable)
    pub fn find_attachment_mut(&mut self, id: &AttachmentId) -> Option<&mut Attachment> {
        self.attachments.iter_mut().find(|a| &a.id == id)
    }
}

/// A comment on a task - part of the discussion thread
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comment {
    pub id: CommentId,
    pub body: String,
    pub author: ActorId,
    // Timestamps are derived from the per-task operation log
}

impl Comment {
    /// Create a new comment
    pub fn new(body: impl Into<String>, author: ActorId) -> Self {
        Self {
            id: CommentId::new(),
            body: body.into(),
            author,
        }
    }
}

/// An attachment on a task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub id: AttachmentId,
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    // created_at is derived from the per-task operation log
}

impl Attachment {
    /// Create a new attachment
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: AttachmentId::new(),
            name: name.into(),
            path: path.into(),
            mime_type: None,
            size: None,
        }
    }

    /// Set the MIME type
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the file size
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ids::ColumnId;
    use crate::types::position::Ordinal;

    fn test_position() -> Position {
        Position::new(ColumnId::from_string("todo"), None, Ordinal::first())
    }

    #[test]
    fn test_task_creation() {
        let task = Task::new("Test task", test_position());
        assert_eq!(task.title, "Test task");
        assert!(task.description.is_empty());
        assert!(task.tags().is_empty());
    }

    #[test]
    fn test_task_tags_computed_from_description() {
        let task = Task::new("Test", test_position())
            .with_description("Fix the #bug in #login");
        let tags = task.tags();
        assert_eq!(tags.len(), 2);
        assert!(tags.iter().any(|t| t == "bug"));
        assert!(tags.iter().any(|t| t == "login"));
    }

    #[test]
    fn test_task_progress_from_markdown() {
        let task = Task::new("Test", test_position());
        assert_eq!(task.progress(), 0.0);

        let task = Task::new("Test", test_position())
            .with_description("## Checklist\n- [ ] Sub 1\n- [x] Sub 2");
        assert_eq!(task.progress(), 0.5);
    }

    #[test]
    fn test_parse_checklist_counts() {
        let (total, completed) = Task::parse_checklist_counts("");
        assert_eq!((total, completed), (0, 0));

        let (total, completed) =
            Task::parse_checklist_counts("- [ ] one\n- [x] two\n- [X] three\n- [ ] four");
        assert_eq!((total, completed), (4, 2));

        // Indented checklists
        let (total, completed) = Task::parse_checklist_counts("  - [ ] indented\n  - [x] done");
        assert_eq!((total, completed), (2, 1));

        // Non-checklist lines ignored
        let (total, completed) =
            Task::parse_checklist_counts("plain text\n- regular bullet\n- [ ] real item");
        assert_eq!((total, completed), (1, 0));
    }

    #[test]
    fn test_task_dependencies() {
        let pos = test_position();
        let done_pos = Position::new(ColumnId::from_string("done"), None, Ordinal::first());

        let task1 = Task::new("Task 1", done_pos);
        let mut task2 = Task::new("Task 2", pos);
        task2.depends_on.push(task1.id.clone());

        let all_tasks = vec![task1.clone(), task2.clone()];

        assert!(task2.is_ready(&all_tasks, "done"));
        assert!(task2.blocked_by(&all_tasks, "done").is_empty());
    }

    #[test]
    fn test_comment() {
        let comment = Comment::new("Test comment", ActorId::from_string("alice"));
        assert_eq!(comment.body, "Test comment");
        assert_eq!(comment.author.as_str(), "alice");
    }

    #[test]
    fn test_task_serialization() {
        let task = Task::new("Test", test_position()).with_description("Description");
        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, task.title);
        assert_eq!(parsed.description, task.description);
    }

    #[test]
    fn test_legacy_subtask_migration() {
        // Simulate reading a task JSON with old-style subtasks and tags
        let json = r#"{
            "id": "test",
            "title": "Test",
            "description": "Some work",
            "tags": ["bug"],
            "position": {"column": "todo", "ordinal": "a0"},
            "depends_on": [],
            "assignees": [],
            "comments": [],
            "subtasks": [
                {"id": "s1", "title": "Write tests", "completed": false},
                {"id": "s2", "title": "Implement feature", "completed": true}
            ],
            "attachments": []
        }"#;

        let mut task: Task = serde_json::from_str(json).unwrap();
        assert!(task.has_legacy_subtasks());
        assert!(task.migrate_legacy_subtasks());
        assert!(!task.has_legacy_subtasks());

        // Description should now contain checklist
        assert!(task.description.contains("- [ ] Write tests"));
        assert!(task.description.contains("- [x] Implement feature"));
        assert!(task.description.starts_with("Some work"));

        // Progress should work from the migrated checklist
        assert_eq!(task.progress(), 0.5);

        // Serialized output should NOT contain subtasks
        let serialized = serde_json::to_string_pretty(&task).unwrap();
        assert!(!serialized.contains("\"subtasks\""));
    }

    #[test]
    fn test_no_migration_without_subtasks() {
        let mut task = Task::new("Test", test_position());
        assert!(!task.has_legacy_subtasks());
        assert!(!task.migrate_legacy_subtasks());
    }
}
