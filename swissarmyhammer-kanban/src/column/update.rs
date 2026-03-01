//! UpdateColumn command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ColumnId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update a column
#[operation(
    verb = "update",
    noun = "column",
    description = "Update a column's name or order"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateColumn {
    /// The column ID to update
    pub id: ColumnId,
    /// New column name
    pub name: Option<String>,
    /// New position in column order
    pub order: Option<usize>,
}

impl UpdateColumn {
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self {
            id: id.into(),
            name: None,
            order: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_order(mut self, order: usize) -> Self {
        self.order = Some(order);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateColumn {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut column = ctx.read_column(&self.id).await?;

            if let Some(name) = &self.name {
                column.name = name.clone();
            }
            if let Some(order) = self.order {
                column.order = order;
            }

            ctx.write_column(&column).await?;
            let mut result = serde_json::to_value(&column)?;
            result["id"] = serde_json::json!(&column.id);
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
