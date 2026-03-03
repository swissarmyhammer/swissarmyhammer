//! Task types (legacy typed structs, being replaced by Entity)

use super::ids::{ActorId, CommentId, TaskId};
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
}

impl Task {
    /// Create a new task with the given title and position
    pub fn new(title: impl Into<String>, position: Position) -> Self {
        Self {
            id: TaskId::new(),
            title: title.into(),
            description: String::new(),
            position,
            depends_on: Vec::new(),
            assignees: Vec::new(),
            comments: Vec::new(),
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
    ) -> Self {
        Self {
            id: TaskId::new(),
            title,
            description,
            position,
            depends_on,
            assignees,
            comments,
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

    /// Calculate progress as fraction of completed markdown checklist items.
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
}

/// A comment on a task - part of the discussion thread
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comment {
    pub id: CommentId,
    pub body: String,
    pub author: ActorId,
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
        let task = Task::new("Test", test_position()).with_description("Fix the #bug in #login");
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

        let (total, completed) = Task::parse_checklist_counts("  - [ ] indented\n  - [x] done");
        assert_eq!((total, completed), (2, 1));

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
}
