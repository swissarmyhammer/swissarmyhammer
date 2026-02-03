//! Operation types: Verb, Noun, and Operation

use super::ids::{ActorId, LogEntryId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

/// The canonical verbs for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    Init,
    Get,
    List,
    Add,
    Update,
    Move,
    Delete,
    Next,
    Tag,
    Untag,
    Complete,
    Assign,
}

impl Verb {
    /// Get the canonical string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Get => "get",
            Self::List => "list",
            Self::Add => "add",
            Self::Update => "update",
            Self::Move => "move",
            Self::Delete => "delete",
            Self::Next => "next",
            Self::Tag => "tag",
            Self::Untag => "untag",
            Self::Complete => "complete",
            Self::Assign => "assign",
        }
    }

    /// Parse from string, recognizing aliases
    pub fn from_alias(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "init" | "create" | "new" => Some(Self::Init),
            "get" | "show" | "read" | "fetch" => Some(Self::Get),
            "list" | "ls" | "find" | "search" | "query" => Some(Self::List),
            "add" | "insert" => Some(Self::Add),
            "update" | "edit" | "modify" | "set" | "patch" => Some(Self::Update),
            "move" | "mv" => Some(Self::Move),
            "delete" | "remove" | "rm" | "del" => Some(Self::Delete),
            "next" => Some(Self::Next),
            "tag" | "label" => Some(Self::Tag),
            "untag" | "unlabel" => Some(Self::Untag),
            "complete" | "done" | "finish" | "close" => Some(Self::Complete),
            "assign" => Some(Self::Assign),
            _ => None,
        }
    }
}

impl fmt::Display for Verb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The canonical nouns for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Noun {
    Board,
    Task,
    Tasks,
    Column,
    Columns,
    Swimlane,
    Swimlanes,
    Actor,
    Actors,
    Tag,
    Tags,
    Comment,
    Comments,
    Subtask,
    Activity,
}

impl Noun {
    /// Get the canonical string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Board => "board",
            Self::Task => "task",
            Self::Tasks => "tasks",
            Self::Column => "column",
            Self::Columns => "columns",
            Self::Swimlane => "swimlane",
            Self::Swimlanes => "swimlanes",
            Self::Actor => "actor",
            Self::Actors => "actors",
            Self::Tag => "tag",
            Self::Tags => "tags",
            Self::Comment => "comment",
            Self::Comments => "comments",
            Self::Subtask => "subtask",
            Self::Activity => "activity",
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "board" => Some(Self::Board),
            "task" => Some(Self::Task),
            "tasks" => Some(Self::Tasks),
            "column" => Some(Self::Column),
            "columns" => Some(Self::Columns),
            "swimlane" => Some(Self::Swimlane),
            "swimlanes" => Some(Self::Swimlanes),
            "actor" => Some(Self::Actor),
            "actors" => Some(Self::Actors),
            "tag" => Some(Self::Tag),
            "tags" => Some(Self::Tags),
            "comment" => Some(Self::Comment),
            "comments" => Some(Self::Comments),
            "subtask" => Some(Self::Subtask),
            "activity" => Some(Self::Activity),
            _ => None,
        }
    }
}

impl fmt::Display for Noun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single canonical operation ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Unique identifier for this operation instance
    pub id: LogEntryId,

    /// The canonical verb
    pub verb: Verb,

    /// The canonical noun
    pub noun: Noun,

    /// Normalized parameters (all aliases resolved, snake_case keys)
    pub params: Map<String, Value>,

    /// Who initiated this operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<ActorId>,

    /// Optional note/reasoning (useful for agent operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Operation {
    /// Create a new operation
    pub fn new(verb: Verb, noun: Noun, params: Map<String, Value>) -> Self {
        Self {
            id: LogEntryId::new(),
            verb,
            noun,
            params,
            actor: None,
            note: None,
        }
    }

    /// Set the actor
    pub fn with_actor(mut self, actor: ActorId) -> Self {
        self.actor = Some(actor);
        self
    }

    /// Set a note
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Returns the canonical op string (e.g., "add task")
    pub fn op_string(&self) -> String {
        format!("{} {}", self.verb.as_str(), self.noun.as_str())
    }

    /// Check if this operation is a mutation (vs read-only)
    pub fn is_mutation(&self) -> bool {
        !matches!(self.verb, Verb::Get | Verb::List | Verb::Next)
    }

    /// Get a parameter value
    pub fn get_param(&self, key: &str) -> Option<&Value> {
        self.params.get(key)
    }

    /// Get a string parameter
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.params.get(key).and_then(|v| v.as_str())
    }

    /// Get a required string parameter
    pub fn require_string(&self, key: &str) -> Result<&str, String> {
        self.get_string(key)
            .ok_or_else(|| format!("missing required field: {}", key))
    }
}

/// Check if a verb+noun combination is valid
pub fn is_valid_operation(verb: Verb, noun: Noun) -> bool {
    matches!(
        (verb, noun),
        // Board operations
        (Verb::Init, Noun::Board) | (Verb::Get, Noun::Board) | (Verb::Update, Noun::Board) |
        // Column operations
        (Verb::Get, Noun::Column) | (Verb::Add, Noun::Column) | (Verb::Update, Noun::Column) |
        (Verb::Delete, Noun::Column) | (Verb::List, Noun::Columns) |
        // Swimlane operations
        (Verb::Get, Noun::Swimlane) | (Verb::Add, Noun::Swimlane) | (Verb::Update, Noun::Swimlane) |
        (Verb::Delete, Noun::Swimlane) | (Verb::List, Noun::Swimlanes) |
        // Actor operations
        (Verb::Get, Noun::Actor) | (Verb::Add, Noun::Actor) | (Verb::Update, Noun::Actor) |
        (Verb::Delete, Noun::Actor) | (Verb::List, Noun::Actors) |
        // Task operations
        (Verb::Get, Noun::Task) | (Verb::Add, Noun::Task) | (Verb::Update, Noun::Task) |
        (Verb::Move, Noun::Task) | (Verb::Delete, Noun::Task) | (Verb::Next, Noun::Task) |
        (Verb::Tag, Noun::Task) | (Verb::Untag, Noun::Task) | (Verb::Complete, Noun::Task) |
        (Verb::Assign, Noun::Task) |
        // Tasks listing
        (Verb::List, Noun::Tasks) |
        // Tag operations (board-level)
        (Verb::Get, Noun::Tag) | (Verb::Add, Noun::Tag) | (Verb::Update, Noun::Tag) |
        (Verb::Delete, Noun::Tag) | (Verb::List, Noun::Tags) |
        // Comment operations
        (Verb::Get, Noun::Comment) | (Verb::Add, Noun::Comment) | (Verb::Update, Noun::Comment) |
        (Verb::Delete, Noun::Comment) | (Verb::List, Noun::Comments) |
        // Subtask operations
        (Verb::Add, Noun::Subtask) | (Verb::Update, Noun::Subtask) |
        (Verb::Complete, Noun::Subtask) | (Verb::Delete, Noun::Subtask) |
        // Activity
        (Verb::List, Noun::Activity)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_aliases() {
        assert_eq!(Verb::from_alias("add"), Some(Verb::Add));
        assert_eq!(Verb::from_alias("create"), Some(Verb::Init));
        assert_eq!(Verb::from_alias("ls"), Some(Verb::List));
        assert_eq!(Verb::from_alias("rm"), Some(Verb::Delete));
        assert_eq!(Verb::from_alias("mv"), Some(Verb::Move));
    }

    #[test]
    fn test_noun_parsing() {
        assert_eq!(Noun::parse("board"), Some(Noun::Board));
        assert_eq!(Noun::parse("TASK"), Some(Noun::Task));
        assert_eq!(Noun::parse("tasks"), Some(Noun::Tasks));
    }

    #[test]
    fn test_operation_string() {
        let op = Operation::new(Verb::Add, Noun::Task, Map::new());
        assert_eq!(op.op_string(), "add task");
    }

    #[test]
    fn test_valid_operations() {
        assert!(is_valid_operation(Verb::Init, Noun::Board));
        assert!(is_valid_operation(Verb::Add, Noun::Task));
        assert!(is_valid_operation(Verb::Move, Noun::Task));
        assert!(is_valid_operation(Verb::List, Noun::Tasks));

        // Invalid combinations
        assert!(!is_valid_operation(Verb::Move, Noun::Board));
        assert!(!is_valid_operation(Verb::Init, Noun::Task));
    }

    #[test]
    fn test_mutation_check() {
        let add = Operation::new(Verb::Add, Noun::Task, Map::new());
        assert!(add.is_mutation());

        let get = Operation::new(Verb::Get, Noun::Task, Map::new());
        assert!(!get.is_mutation());

        let list = Operation::new(Verb::List, Noun::Tasks, Map::new());
        assert!(!list.is_mutation());
    }
}
