//! Task types (legacy typed structs, being replaced by Entity)

use super::ids::{ActorId, TaskId};
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
        }
    }

    /// Reconstruct a task from parsed frontmatter parts (used by markdown reader)
    pub fn from_parts(
        title: String,
        description: String,
        position: Position,
        depends_on: Vec<TaskId>,
        assignees: Vec<ActorId>,
    ) -> Self {
        Self {
            id: TaskId::new(),
            title,
            description,
            position,
            depends_on,
            assignees,
        }
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
