//! UpdateTag command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TagId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Update a tag
#[operation(verb = "update", noun = "tag", description = "Update a tag's name, color, or description")]
#[derive(Debug, Deserialize)]
pub struct UpdateTag {
    /// The tag ID to update
    pub id: TagId,
    /// New tag name
    pub name: Option<String>,
    /// New color (6-character hex without #)
    pub color: Option<String>,
    /// New description
    pub description: Option<String>,
}

impl UpdateTag {
    pub fn new(id: impl Into<TagId>) -> Self {
        Self {
            id: id.into(),
            name: None,
            color: None,
            description: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateTag {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        let tag = board
            .tags
            .iter_mut()
            .find(|t| &t.id == &self.id)
            .ok_or_else(|| KanbanError::TagNotFound {
                id: self.id.to_string(),
            })?;

        if let Some(name) = &self.name {
            tag.name = name.clone();
        }
        if let Some(color) = &self.color {
            tag.color = color.clone();
        }
        if let Some(description) = &self.description {
            tag.description = Some(description.clone());
        }

        let result = serde_json::to_value(&*tag)?;
        ctx.write_board(&board).await?;

        Ok(result)
    }
}
