//! UpdateSwimlane command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::SwimlaneId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Update a swimlane
#[operation(verb = "update", noun = "swimlane", description = "Update a swimlane's name or order")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateSwimlane {
    /// The swimlane ID to update
    pub id: SwimlaneId,
    /// New swimlane name
    pub name: Option<String>,
    /// New position in swimlane order
    pub order: Option<usize>,
}

impl UpdateSwimlane {
    pub fn new(id: impl Into<SwimlaneId>) -> Self {
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
impl Execute<KanbanContext, KanbanError> for UpdateSwimlane {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut board = ctx.read_board().await?;

            let swimlane = board
                .swimlanes
                .iter_mut()
                .find(|s| s.id == self.id)
                .ok_or_else(|| KanbanError::SwimlaneNotFound {
                    id: self.id.to_string(),
                })?;

            if let Some(name) = &self.name {
                swimlane.name = name.clone();
            }
            if let Some(order) = self.order {
                swimlane.order = order;
            }

            let result = serde_json::to_value(&*swimlane)?;
            ctx.write_board(&board).await?;

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
