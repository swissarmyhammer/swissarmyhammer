//! InitBoard command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::Board;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Initialize a new kanban board
#[operation(
    verb = "init",
    noun = "board",
    description = "Initialize a new kanban board"
)]
#[derive(Debug, Deserialize)]
pub struct InitBoard {
    /// The board name
    pub name: String,
    /// Optional board description
    pub description: Option<String>,
}

impl InitBoard {
    /// Create a new InitBoard command
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for InitBoard {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        // Check if already initialized
        if ctx.is_initialized() {
            return Err(KanbanError::AlreadyExists {
                path: ctx.root().to_path_buf(),
            });
        }

        // Create directory structure
        ctx.create_directories().await?;

        // Build board with default columns
        let mut board = Board::new(&self.name);
        if let Some(desc) = &self.description {
            board = board.with_description(desc);
        }

        // Write board file
        ctx.write_board(&board).await?;

        Ok(serde_json::to_value(&board)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_init_board() {
        let (_temp, ctx) = setup().await;

        let cmd = InitBoard::new("Test Board").with_description("A test board");
        let result = cmd.execute(&ctx).await.unwrap();

        assert_eq!(result["name"], "Test Board");
        assert_eq!(result["description"], "A test board");
        assert!(result["columns"].is_array());
        assert_eq!(result["columns"].as_array().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn test_init_board_already_exists() {
        let (_temp, ctx) = setup().await;

        // First init should succeed
        let cmd = InitBoard::new("Test");
        cmd.execute(&ctx).await.unwrap();

        // Second init should fail
        let result = cmd.execute(&ctx).await;
        assert!(matches!(result, Err(KanbanError::AlreadyExists { .. })));
    }

    #[test]
    fn test_operation_metadata() {
        use swissarmyhammer_operations::Operation;

        // Create an instance to test Operation trait methods
        let op = InitBoard::new("test");

        // Verify the Operation trait is correctly implemented via macro
        assert_eq!(op.verb(), "init");
        assert_eq!(op.noun(), "board");
        assert_eq!(op.description(), "Initialize a new kanban board");

        // Verify parameters
        let params = op.parameters();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "name");
        assert!(params[0].required);
        assert_eq!(params[1].name, "description");
        assert!(!params[1].required);
    }
}
