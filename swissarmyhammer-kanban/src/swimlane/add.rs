//! AddSwimlane command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Swimlane, SwimlaneId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Add a new swimlane to the board
#[operation(verb = "add", noun = "swimlane", description = "Add a new swimlane to the board")]
#[derive(Debug, Deserialize)]
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
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        // Check for duplicate ID
        if board.find_swimlane(&self.id).is_some() {
            return Err(KanbanError::duplicate_id("swimlane", self.id.to_string()));
        }

        // Determine order
        let order = self.order.unwrap_or_else(|| {
            board
                .swimlanes
                .iter()
                .map(|s| s.order)
                .max()
                .map(|o| o + 1)
                .unwrap_or(0)
        });

        let swimlane = Swimlane {
            id: self.id.clone(),
            name: self.name.clone(),
            order,
        };

        board.swimlanes.push(swimlane.clone());
        ctx.write_board(&board).await?;

        Ok(serde_json::to_value(&swimlane)?)
    }
}
