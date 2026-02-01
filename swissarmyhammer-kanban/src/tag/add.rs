//! AddTag command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Tag, TagId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Add a new tag to the board
#[operation(verb = "add", noun = "tag", description = "Add a new tag to the board")]
#[derive(Debug, Deserialize)]
pub struct AddTag {
    /// The tag ID (slug)
    pub id: TagId,
    /// The tag display name
    pub name: String,
    /// 6-character hex color code (without #)
    pub color: String,
    /// Optional description
    pub description: Option<String>,
}

impl AddTag {
    pub fn new(id: impl Into<TagId>, name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            color: color.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddTag {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        // Check for duplicate ID
        if board.tags.iter().any(|t| &t.id == &self.id) {
            return Err(KanbanError::duplicate_id("tag", self.id.to_string()));
        }

        let mut tag = Tag::new(self.id.0.clone(), &self.name, &self.color);
        if let Some(desc) = &self.description {
            tag = tag.with_description(desc);
        }

        board.tags.push(tag.clone());
        ctx.write_board(&board).await?;

        Ok(serde_json::to_value(&tag)?)
    }
}
