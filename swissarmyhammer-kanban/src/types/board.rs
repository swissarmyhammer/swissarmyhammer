//! Board-level types: Board, Column, Swimlane, Actor, Tag

use super::ids::{ActorId, ColumnId, SwimlaneId, TagId};
use serde::{Deserialize, Serialize};

/// The kanban board - defines structure but not task membership
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub columns: Vec<Column>,
    #[serde(default)]
    pub swimlanes: Vec<Swimlane>,
    #[serde(default)]
    pub actors: Vec<Actor>,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

impl Board {
    /// Create a new board with the given name and default columns
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            columns: Self::default_columns(),
            swimlanes: Vec::new(),
            actors: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Create a new board with custom settings
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Get the default columns for a new board
    pub fn default_columns() -> Vec<Column> {
        vec![
            Column {
                id: ColumnId::from_string("todo"),
                name: "To Do".into(),
                order: 0,
            },
            Column {
                id: ColumnId::from_string("doing"),
                name: "Doing".into(),
                order: 1,
            },
            Column {
                id: ColumnId::from_string("done"),
                name: "Done".into(),
                order: 2,
            },
        ]
    }

    /// Get the first column (lowest order)
    pub fn first_column(&self) -> Option<&Column> {
        self.columns.iter().min_by_key(|c| c.order)
    }

    /// Get the terminal/done column (highest order)
    pub fn terminal_column(&self) -> Option<&Column> {
        self.columns.iter().max_by_key(|c| c.order)
    }

    /// Find a column by ID
    pub fn find_column(&self, id: &ColumnId) -> Option<&Column> {
        self.columns.iter().find(|c| &c.id == id)
    }

    /// Find a swimlane by ID
    pub fn find_swimlane(&self, id: &SwimlaneId) -> Option<&Swimlane> {
        self.swimlanes.iter().find(|s| &s.id == id)
    }

    /// Find an actor by ID
    pub fn find_actor(&self, id: &ActorId) -> Option<&Actor> {
        self.actors.iter().find(|a| a.id() == id)
    }

    /// Find a tag by ID
    pub fn find_tag(&self, id: &TagId) -> Option<&Tag> {
        self.tags.iter().find(|t| &t.id == id)
    }
}

/// A column defines a workflow stage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Column {
    pub id: ColumnId,
    pub name: String,
    pub order: usize,
}

/// A swimlane provides horizontal grouping orthogonal to columns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Swimlane {
    pub id: SwimlaneId,
    pub name: String,
    pub order: usize,
}

/// An actor is a person or agent that can be assigned to tasks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Actor {
    Human { id: ActorId, name: String },
    Agent { id: ActorId, name: String },
}

impl Actor {
    /// Create a new human actor
    pub fn human(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Human {
            id: ActorId::from_string(id),
            name: name.into(),
        }
    }

    /// Create a new agent actor
    pub fn agent(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Agent {
            id: ActorId::from_string(id),
            name: name.into(),
        }
    }

    /// Get the actor's ID
    pub fn id(&self) -> &ActorId {
        match self {
            Self::Human { id, .. } => id,
            Self::Agent { id, .. } => id,
        }
    }

    /// Get the actor's name
    pub fn name(&self) -> &str {
        match self {
            Self::Human { name, .. } => name,
            Self::Agent { name, .. } => name,
        }
    }

    /// Check if this is a human actor
    pub fn is_human(&self) -> bool {
        matches!(self, Self::Human { .. })
    }

    /// Check if this is an agent actor
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }
}

/// A tag categorizes tasks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 6-character hex color code without #
    pub color: String,
}

impl Tag {
    /// Create a new tag
    pub fn new(id: impl Into<String>, name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            id: TagId::from_string(id),
            name: name.into(),
            description: None,
            color: color.into(),
        }
    }

    /// Add a description to the tag
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_creation() {
        let board = Board::new("Test Board");
        assert_eq!(board.name, "Test Board");
        assert_eq!(board.columns.len(), 3);
        assert!(board.description.is_none());
    }

    #[test]
    fn test_board_with_description() {
        let board = Board::new("Test").with_description("A test board");
        assert_eq!(board.description, Some("A test board".into()));
    }

    #[test]
    fn test_first_and_terminal_columns() {
        let board = Board::new("Test");
        let first = board.first_column().unwrap();
        let terminal = board.terminal_column().unwrap();

        assert_eq!(first.id.as_str(), "todo");
        assert_eq!(terminal.id.as_str(), "done");
    }

    #[test]
    fn test_actor_types() {
        let human = Actor::human("alice", "Alice Smith");
        assert!(human.is_human());
        assert!(!human.is_agent());
        assert_eq!(human.name(), "Alice Smith");

        let agent = Actor::agent("claude", "Claude");
        assert!(agent.is_agent());
        assert!(!agent.is_human());
    }

    #[test]
    fn test_board_serialization() {
        let board = Board::new("Test");
        let json = serde_json::to_string_pretty(&board).unwrap();
        let parsed: Board = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, board.name);
        assert_eq!(parsed.columns.len(), board.columns.len());
    }
}
