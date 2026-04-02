//! ListActivity command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List activity log entries
#[operation(
    verb = "list",
    noun = "activity",
    description = "List activity log entries (most recent first)"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListActivity {
    /// Maximum number of entries to return
    pub limit: Option<usize>,
}

impl ListActivity {
    /// Create a new ListActivity command with no limit.
    pub fn new() -> Self {
        Self { limit: None }
    }

    /// Set the maximum number of entries to return.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListActivity {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let entries = ctx.read_activity(self.limit).await?;

            Ok(serde_json::json!({
                "entries": entries,
                "count": entries.len()
            }))
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
    use crate::{KanbanOperationProcessor, OperationProcessor};
    use tempfile::TempDir;

    /// Set up a temporary board with an initialized context.
    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);
        let processor = KanbanOperationProcessor::new();
        processor
            .process(&InitBoard::new("Test Board"), &ctx)
            .await
            .unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn list_activity_returns_entries_newest_first() {
        let (_temp, ctx) = setup().await;
        let processor = KanbanOperationProcessor::new();

        // Add two tasks so we have multiple log entries beyond init board
        processor
            .process(&AddTask::new("Task One"), &ctx)
            .await
            .unwrap();
        processor
            .process(&AddTask::new("Task Two"), &ctx)
            .await
            .unwrap();

        // Execute ListActivity via the processor (still unlogged, but result is accessible)
        let cmd = ListActivity::new();
        let result = cmd.execute(&ctx).await;

        let value = match result {
            ExecutionResult::Unlogged { value } => value,
            _ => panic!("Expected Unlogged result from ListActivity"),
        };

        // Should have 3 entries (init board + 2 add tasks); newest first
        let count = value["count"].as_u64().unwrap();
        assert_eq!(count, 3, "count field should reflect 3 logged operations");

        let entries = value["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0]["op"].as_str().unwrap(), "add task");
        assert_eq!(entries[2]["op"].as_str().unwrap(), "init board");
    }

    #[tokio::test]
    async fn list_activity_respects_limit() {
        let (_temp, ctx) = setup().await;
        let processor = KanbanOperationProcessor::new();

        // Create several additional operations
        for i in 0..5u32 {
            processor
                .process(&AddTask::new(format!("Task {i}")), &ctx)
                .await
                .unwrap();
        }

        // Execute with a limit of 2
        let cmd = ListActivity::new().with_limit(2);
        let value = match cmd.execute(&ctx).await {
            ExecutionResult::Unlogged { value } => value,
            _ => panic!("Expected Unlogged result from ListActivity"),
        };

        let entries = value["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2, "limit of 2 should return only 2 entries");

        let count = value["count"].as_u64().unwrap();
        assert_eq!(count, 2, "count field should match the truncated length");
    }

    #[tokio::test]
    async fn list_activity_on_empty_log_returns_empty() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);
        // Ensure directories exist without logging any operations
        ctx.ensure_directories().await.unwrap();

        let cmd = ListActivity::new();
        let value = match cmd.execute(&ctx).await {
            ExecutionResult::Unlogged { value } => value,
            _ => panic!("Expected Unlogged result from ListActivity"),
        };

        assert_eq!(value["count"].as_u64().unwrap(), 0);
        assert!(value["entries"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_activity_is_unlogged() {
        // ListActivity should produce an Unlogged result, not appending to the log itself.
        let (_temp, ctx) = setup().await;

        let before = ctx.read_activity(None).await.unwrap();
        let before_count = before.len();

        let cmd = ListActivity::new();
        let result = cmd.execute(&ctx).await;

        // The result itself must be Unlogged
        assert!(
            matches!(result, ExecutionResult::Unlogged { .. }),
            "ListActivity must return an Unlogged result"
        );

        // The activity log must not have grown
        let after = ctx.read_activity(None).await.unwrap();
        assert_eq!(
            after.len(),
            before_count,
            "ListActivity must not append to the activity log"
        );
    }

    #[tokio::test]
    async fn list_activity_builder_methods() {
        let cmd = ListActivity::new();
        assert!(cmd.limit.is_none(), "default limit should be None");

        let cmd = cmd.with_limit(10);
        assert_eq!(cmd.limit, Some(10), "with_limit should set the limit");
    }
}
