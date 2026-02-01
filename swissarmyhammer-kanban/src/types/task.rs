//! Task types: Task, Subtask, Attachment, Comment

use super::ids::{ActorId, AttachmentId, CommentId, SubtaskId, TagId, TaskId};
use super::position::Position;
use serde::{Deserialize, Serialize};

/// A task/card on the kanban board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<TagId>,

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

    /// Subtasks/checklist items
    #[serde(default)]
    pub subtasks: Vec<Subtask>,

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
            tags: Vec::new(),
            position,
            depends_on: Vec::new(),
            assignees: Vec::new(),
            comments: Vec::new(),
            subtasks: Vec::new(),
            attachments: Vec::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<TagId>) -> Self {
        self.tags = tags;
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

    /// Calculate progress as fraction of completed subtasks
    pub fn progress(&self) -> f64 {
        if self.subtasks.is_empty() {
            return 0.0;
        }
        let completed = self.subtasks.iter().filter(|s| s.completed).count();
        completed as f64 / self.subtasks.len() as f64
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

    /// Find a subtask by ID
    pub fn find_subtask(&self, id: &SubtaskId) -> Option<&Subtask> {
        self.subtasks.iter().find(|s| &s.id == id)
    }

    /// Find a subtask by ID (mutable)
    pub fn find_subtask_mut(&mut self, id: &SubtaskId) -> Option<&mut Subtask> {
        self.subtasks.iter_mut().find(|s| &s.id == id)
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

/// A subtask/checklist item within a task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subtask {
    pub id: SubtaskId,
    pub title: String,
    #[serde(default)]
    pub completed: bool,
    // completed_at is derived from the per-task operation log
}

impl Subtask {
    /// Create a new subtask
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: SubtaskId::new(),
            title: title.into(),
            completed: false,
        }
    }

    /// Create a completed subtask
    pub fn completed(title: impl Into<String>) -> Self {
        Self {
            id: SubtaskId::new(),
            title: title.into(),
            completed: true,
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
        assert!(task.tags.is_empty());
    }

    #[test]
    fn test_task_progress() {
        let mut task = Task::new("Test", test_position());
        assert_eq!(task.progress(), 0.0);

        task.subtasks.push(Subtask::new("Sub 1"));
        task.subtasks.push(Subtask::completed("Sub 2"));
        assert_eq!(task.progress(), 0.5);
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
}
