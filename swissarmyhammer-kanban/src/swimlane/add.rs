//! AddSwimlane command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{Swimlane, SwimlaneId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a new swimlane to the board
#[operation(
    verb = "add",
    noun = "swimlane",
    description = "Add a new swimlane to the board"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddSwimlane {
    /// The swimlane ID (slug)
    pub id: SwimlaneId,
    /// The swimlane display name
    pub name: String,
    /// Optional position in swimlane order
    pub order: Option<usize>,
}

impl AddSwimlane {
    /// Create a new AddSwimlane command
    pub fn new(id: impl Into<SwimlaneId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            order: None,
        }
    }

    /// Set the order (position in swimlane list)
    pub fn with_order(mut self, order: usize) -> Self {
        self.order = Some(order);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddSwimlane {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            // Check for duplicate ID
            if ctx.swimlane_exists(&self.id).await {
                return Err(KanbanError::duplicate_id("swimlane", self.id.to_string()));
            }

            // Determine order
            let order = if let Some(order) = self.order {
                order
            } else {
                let swimlanes = ctx.read_all_swimlanes().await?;
                swimlanes
                    .iter()
                    .map(|s| s.order)
                    .max()
                    .map(|o| o + 1)
                    .unwrap_or(0)
            };

            let swimlane = Swimlane {
                id: self.id.clone(),
                name: self.name.clone(),
                order,
            };

            ctx.write_swimlane(&swimlane).await?;

            Ok(serde_json::to_value(&swimlane)?)
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
