//! Kanban operation processor

use crate::types::default_column_entities;
use crate::{KanbanContext, KanbanError, Result};
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_operations::{Execute, LogEntry, OperationProcessor};

/// Kanban-specific operation processor
///
/// Handles execution and actor attribution for all kanban operations.
/// Per-entity logging is handled by EntityContext/StoreHandle.
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

        // TODO: Add store-level transaction support so compound operations
        // (e.g. tag rename that touches multiple tasks) can be undone as a
        // single unit. For now, each write/delete is an independent undo entry.

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

        // Split into result and log entry
        let (result, mut log_entry) = exec_result.split();

        // Add actor attribution to log entry (per-entity logging is handled
        // by EntityContext; there is no global activity log).
        if let Some(ref mut entry) = log_entry {
            if let Some(ref actor) = self.actor {
                entry.actor = Some(actor.clone());
            }
        }

        result
    }

    async fn write_log(
        &self,
        _ctx: &KanbanContext,
        _log_entry: &LogEntry,
        _affected_resources: &[String],
    ) -> Result<()> {
        // Per-entity logging is handled by EntityContext/StoreHandle;
        // there is no global activity log, so this is intentionally a no-op.
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
    async fn test_processor_executes_operations() {
        let (_temp, ctx) = setup().await;
        let processor = KanbanOperationProcessor::new();

        let cmd = AddTask::new("Test task");
        let result = processor.process(&cmd, &ctx).await.unwrap();
        assert!(result["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_processor_with_actor() {
        let (_temp, ctx) = setup().await;
        let processor = KanbanOperationProcessor::with_actor("assistant[session123]");

        let cmd = AddTask::new("Test task");
        let result = processor.process(&cmd, &ctx).await.unwrap();
        assert!(result["id"].as_str().is_some());
    }
}
