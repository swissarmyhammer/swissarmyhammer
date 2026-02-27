//! InitBoard command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::Board;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Initialize a new kanban board
#[operation(
    verb = "init",
    noun = "board",
    description = "Initialize a new kanban board"
)]
#[derive(Debug, Deserialize, Serialize)]
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
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            // Check if already initialized
            if ctx.is_initialized() {
                return Err(KanbanError::AlreadyExists {
                    path: ctx.root().to_path_buf(),
                });
            }

            // Create directory structure
            ctx.create_directories().await?;

            // Build board metadata (slim - no columns/swimlanes)
            let mut board = Board::new(&self.name);
            if let Some(desc) = &self.description {
                board = board.with_description(desc);
            }

            // Write board file
            ctx.write_board(&board).await?;

            // Write default columns as individual files
            for column in Board::default_columns() {
                ctx.write_column(&column).await?;
            }

            // Return board with columns in response (for API compatibility)
            let mut result = serde_json::to_value(&board)?;
            result["columns"] = serde_json::to_value(Board::default_columns())?;
            result["swimlanes"] = serde_json::json!([]);
            Ok(result)
        }
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(value) => ExecutionResult::Logged {
                value: value.clone(),
                log_entry: LogEntry::new(self.op_string(), input, value, None, duration_ms),
            },
            Err(error) => {
                let error_msg = error.to_string();
                ExecutionResult::Failed {
                    error,
                    log_entry: Some(LogEntry::new(
                        self.op_string(),
                        input,
                        serde_json::json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
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
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["name"], "Test Board");
        assert_eq!(result["description"], "A test board");
        assert!(result["columns"].is_array());
        assert_eq!(result["columns"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_init_board_already_exists() {
        let (_temp, ctx) = setup().await;

        // First init should succeed
        let cmd = InitBoard::new("Test");
        cmd.execute(&ctx).await.into_result().unwrap();

        // Second init should fail
        let result = cmd.execute(&ctx).await.into_result();
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
