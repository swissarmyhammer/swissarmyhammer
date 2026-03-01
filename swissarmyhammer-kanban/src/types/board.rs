//! Board-level types: Board, Column, Swimlane, Actor, Tag

use super::ids::{ActorId, ColumnId, SwimlaneId, TagId};
use serde::{Deserialize, Serialize};

/// The kanban board - just metadata (name + description).
/// Columns and swimlanes are stored as individual files for git-friendly merging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Legacy: columns stored inline. Used only during migration from old format.
    #[serde(default, skip_serializing)]
    pub columns: Vec<Column>,
    /// Legacy: swimlanes stored inline. Used only during migration from old format.
    #[serde(default, skip_serializing)]
    pub swimlanes: Vec<Swimlane>,
}

impl Board {
    /// Create a new board with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            columns: Vec::new(),
            swimlanes: Vec::new(),
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
}

/// A column defines a workflow stage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Column {
    #[serde(skip)]
    pub id: ColumnId,
    pub name: String,
    pub order: usize,
}

/// A swimlane provides horizontal grouping orthogonal to columns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Swimlane {
    #[serde(skip)]
    pub id: SwimlaneId,
    pub name: String,
    pub order: usize,
}

/// An actor is a person or agent that can be assigned to tasks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Actor {
    Human {
        #[serde(skip)]
        id: ActorId,
        name: String,
    },
    Agent {
        #[serde(skip)]
        id: ActorId,
        name: String,
    },
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

    /// Set the actor's ID (used after deserialization to restore from filename)
    pub fn set_id(&mut self, new_id: ActorId) {
        match self {
            Self::Human { id, .. } => *id = new_id,
            Self::Agent { id, .. } => *id = new_id,
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

/// A tag categorizes tasks.
///
/// Tags have a ULID-based `id` for stable identity and a human-readable
/// `name` (slug) that appears as `#name` in task descriptions.
/// Color defaults to a deterministic auto-color based on the name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    #[serde(skip)]
    pub id: TagId,
    /// Human-readable slug (e.g. "bug", "high-priority").
    /// This is what appears as `#name` in task descriptions.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 6-character hex color code without #
    pub color: String,
}

impl Tag {
    /// Create a new tag with a ULID and auto-color based on name.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let color = crate::auto_color::auto_color(&name).to_string();
        Self {
            id: TagId::new(),
            name,
            description: None,
            color,
        }
    }

    /// Create a new tag with an explicit color.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
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
        assert!(board.description.is_none());
    }

    #[test]
    fn test_board_with_description() {
        let board = Board::new("Test").with_description("A test board");
        assert_eq!(board.description, Some("A test board".into()));
    }

    #[test]
    fn test_default_columns() {
        let cols = Board::default_columns();
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0].id.as_str(), "todo");
        assert_eq!(cols[2].id.as_str(), "done");
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
        // New format: columns are NOT serialized into board.json
        assert!(parsed.columns.is_empty());
    }

    #[test]
    fn test_board_reads_legacy_columns() {
        // Test that old board.json with embedded columns can still be read (for migration)
        let json_with_old_fields = r#"{
            "name": "Test Board",
            "columns": [
                {"id": "todo", "name": "To Do", "order": 0}
            ],
            "swimlanes": [],
            "actors": [
                {"type": "human", "id": "alice", "name": "Alice"}
            ]
        }"#;

        let result: Result<Board, _> = serde_json::from_str(json_with_old_fields);
        assert!(
            result.is_ok(),
            "Should deserialize old format, got: {:?}",
            result
        );

        let board = result.unwrap();
        assert_eq!(board.name, "Test Board");
        // Legacy columns are readable for migration purposes
        assert_eq!(board.columns.len(), 1);
    }

    #[test]
    fn test_tag_creation() {
        let tag = Tag::new("bug");
        assert_eq!(tag.name, "bug");
        assert!(!tag.id.as_str().is_empty());
        // ULID should be 26 chars
        assert_eq!(tag.id.as_str().len(), 26);

        // Serialized output should contain name
        let serialized = serde_json::to_string(&tag).unwrap();
        assert!(serialized.contains("\"name\""));
        assert!(serialized.contains("\"bug\""));
    }
}
