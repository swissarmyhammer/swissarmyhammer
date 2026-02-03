//! UpdateBoard command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Update board metadata
#[operation(verb = "update", noun = "board", description = "Update board name or description")]
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct UpdateBoard {
    /// New board name
    pub name: Option<String>,
    /// New board description
    pub description: Option<String>,
}

impl UpdateBoard {
    /// Create a new UpdateBoard command
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
        }
    }

    /// Set the new name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the new description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateBoard {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut board = ctx.read_board().await?;

            if let Some(name) = &self.name {
                board.name = name.clone();
            }
            if let Some(desc) = &self.description {
                board.description = Some(desc.clone());
            }

            ctx.write_board(&board).await?;
            Ok(serde_json::to_value(&board)?)
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
    use crate::board::InitBoard;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Original").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_update_board_name() {
        let (_temp, ctx) = setup().await;

        let cmd = UpdateBoard::new().with_name("Updated Name");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["name"], "Updated Name");
    }

    #[tokio::test]
    async fn test_update_board_description() {
        let (_temp, ctx) = setup().await;

        let cmd = UpdateBoard::new().with_description("New description");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["description"], "New description");
    }
}
