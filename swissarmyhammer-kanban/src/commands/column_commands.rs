//! Column command implementations: reorder.

use super::run_op;
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Column id + order pair used by ColumnReorderCmd.
#[derive(Deserialize)]
struct ColumnOrder {
    id: String,
    order: usize,
}

/// Reorder columns by updating each column's order field.
///
/// Always available. Required arg: `columns` (JSON array of `{id, order}` objects).
///
/// Note: updates are applied per-column, not atomically. A mid-loop failure
/// leaves earlier columns with updated orders. This is acceptable because
/// column order is cosmetic (no data loss) and the user can simply re-drag.
pub struct ColumnReorderCmd;

#[async_trait]
impl Command for ColumnReorderCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let columns_val = ctx
            .arg("columns")
            .ok_or_else(|| CommandError::MissingArg("columns".into()))?;

        let columns: Vec<ColumnOrder> = serde_json::from_value(columns_val.clone())
            .map_err(|e| CommandError::ExecutionFailed(format!("invalid 'columns' arg: {}", e)))?;

        let mut operation_ids: Vec<String> = Vec::new();

        for col in &columns {
            let op = crate::column::UpdateColumn::new(col.id.clone()).with_order(col.order);
            let result = run_op(&op, &kanban).await?;
            if let Some(op_id) = result.get("operation_id").and_then(|v| v.as_str()) {
                operation_ids.push(op_id.to_string());
            }
        }

        Ok(json!({ "updated": columns.len(), "operation_ids": operation_ids }))
    }
}
