//! ListActivity command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List activity log entries
#[operation(verb = "list", noun = "activity", description = "List activity log entries (most recent first)")]
#[derive(Debug, Default, Deserialize)]
pub struct ListActivity {
    /// Maximum number of entries to return
    pub limit: Option<usize>,
}

impl ListActivity {
    pub fn new() -> Self {
        Self { limit: None }
    }

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
