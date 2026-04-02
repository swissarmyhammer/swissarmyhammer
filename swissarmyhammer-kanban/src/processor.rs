//! Kanban operation processor

use crate::types::default_column_entities;
use crate::{KanbanContext, KanbanError, Result};
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_entity::EntityContext;
use swissarmyhammer_operations::{Execute, LogEntry, OperationProcessor};

/// Kanban-specific operation processor
///
/// Handles execution and logging for all kanban operations.
/// - Executes operations via Execute trait
/// - Writes logs to global activity log
/// - Writes logs to per-task logs for affected tasks
pub struct KanbanOperationProcessor {
    /// Optional actor performing operations (for log attribution)
    pub actor: Option<String>,
}

impl KanbanOperationProcessor {
    /// Create a new processor without actor attribution
    pub fn new() -> Self {
        Self { actor: None }
    }

    /// Create a new processor with actor attribution
    pub fn with_actor(actor: impl Into<String>) -> Self {
        Self {
            actor: Some(actor.into()),
        }
    }
}

#[async_trait]
impl OperationProcessor<KanbanContext, KanbanError> for KanbanOperationProcessor {
    async fn process<T>(&self, operation: &T, ctx: &KanbanContext) -> Result<Value>
    where
        T: Execute<KanbanContext, KanbanError> + Send + Sync,
    {
        // Ensure directory structure exists (idempotent, fast when dirs exist)
        ctx.ensure_directories().await?;

        // Auto-initialize board if not present (idempotent)
        // Skip auto-init if the operation is InitBoard itself
        if !ctx.is_initialized() && operation.op_string() != "init board" {
            let ectx = ctx.entity_context().await?;
            let mut board_entity = swissarmyhammer_entity::Entity::new("board", "board");
            board_entity.set("name", serde_json::json!("Untitled Board"));
            ectx.write(&board_entity).await?;
            for entity in &default_column_entities() {
                ectx.write(entity).await?;
            }
        }

        // Generate a transaction ID and set it on the EntityContext.
        // All write/delete calls during operation execution get this stamped,
        // enabling compound undo/redo as a single unit.
        let tx_id = EntityContext::generate_transaction_id();
        let ectx = ctx.entity_context().await?;
        ectx.set_transaction(tx_id.clone()).await;

        // Log every operation flowing through the processor so we can trace
        // activity from any entry point (Tauri, MCP, CLI, tests).
        let op_name = operation.op_string();
        tracing::info!(
            op = %op_name,
            actor = ?self.actor,
            "[op] {}", op_name,
        );

        // Execute the operation
        let exec_result = operation.execute(ctx).await;

        // Clear the transaction after execution
        ectx.clear_transaction().await;

        // Split into result and log entry
        let (result, mut log_entry) = exec_result.split();

        // Write log if present
        if let Some(ref mut entry) = log_entry {
            // Add actor attribution
            if let Some(ref actor) = self.actor {
                entry.actor = Some(actor.clone());
            }

            // Write logs
            if let Ok(ref value) = result {
                let affected = operation.affected_resource_ids(value);
                self.write_log(ctx, entry, &affected).await?;
            } else {
                // Still log errors
                self.write_log(ctx, entry, &[]).await?;
            }
        }

        // Add the operation_id (transaction ULID) to the result JSON
        // so callers can use it for undo/redo.
        match result {
            Ok(mut value) => {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("operation_id".to_string(), Value::String(tx_id.to_string()));
                }
                Ok(value)
            }
            Err(e) => Err(e),
        }
    }

    async fn write_log(
        &self,
        ctx: &KanbanContext,
        log_entry: &LogEntry,
        _affected_resources: &[String],
    ) -> Result<()> {
        // Global activity log only — per-entity logging is handled by EntityContext
        ctx.append_activity(log_entry).await?;

        Ok(())
    }
}

impl Default for KanbanOperationProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
    use crate::types::TaskId;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        // Initialize board first
        let processor = KanbanOperationProcessor::new();
        processor
            .process(&InitBoard::new("Test Board"), &ctx)
            .await
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_processor_writes_activity_log() {
        let (_temp, ctx) = setup().await;
        let processor = KanbanOperationProcessor::new();

        let cmd = AddTask::new("Test task");
        processor.process(&cmd, &ctx).await.unwrap();

        // Verify log was written
        let entries = ctx.read_activity(None).await.unwrap();
        assert_eq!(entries.len(), 2); // InitBoard + AddTask
        assert_eq!(entries[0].op, "add task"); // Newest entry
    }

    #[tokio::test]
    async fn test_processor_writes_entity_changelog() {
        let (_temp, ctx) = setup().await;
        let processor = KanbanOperationProcessor::new();

        // Add task
        let cmd = AddTask::new("Test task");
        let result = processor.process(&cmd, &ctx).await.unwrap();
        let task_id = result["id"].as_str().unwrap();

        // Check entity changelog (written by EntityContext::write)
        let task_log_path = ctx.task_log_path(&TaskId::from_string(task_id));
        let content = std::fs::read_to_string(task_log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // Only entity changelog entry (processor no longer writes per-entity logs)
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_processor_default() {
        // Ensure the Default impl is covered — it delegates to new()
        let processor = KanbanOperationProcessor::default();
        assert!(processor.actor.is_none());
    }

    #[tokio::test]
    async fn test_processor_with_actor() {
        let (_temp, ctx) = setup().await;
        let processor = KanbanOperationProcessor::with_actor("assistant[session123]");

        let cmd = AddTask::new("Test task");
        processor.process(&cmd, &ctx).await.unwrap();

        // Verify actor is in log
        let entries = ctx.read_activity(None).await.unwrap();
        let add_task_entry = &entries[0]; // Newest entry (AddTask)
        assert_eq!(
            add_task_entry.actor,
            Some("assistant[session123]".to_string())
        );
    }
}
