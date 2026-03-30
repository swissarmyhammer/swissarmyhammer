//! CopyTag operation — snapshot a tag's fields for the clipboard.

use crate::clipboard;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Copy a tag's fields to clipboard-ready JSON.
///
/// Returns the serialized clipboard payload as a JSON string in the result.
/// The Command layer is responsible for writing this to the system clipboard.
#[operation(
    verb = "copy",
    noun = "tag",
    description = "Copy a tag to the clipboard"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct CopyTag {
    /// The tag entity ID to copy.
    pub id: String,
}

impl CopyTag {
    /// Create a new CopyTag operation.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for CopyTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let entity = ectx
                .read("tag", &self.id)
                .await
                .map_err(KanbanError::from_entity_error)?;

            let fields = serde_json::to_value(&entity.fields)?;
            let clipboard_json = clipboard::serialize_to_clipboard("tag", &self.id, "copy", fields);

            Ok(serde_json::json!({
                "copied": true,
                "id": self.id,
                "entity_type": "tag",
                "clipboard_json": clipboard_json,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::clipboard;
    use crate::tag::AddTag;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let ctx = KanbanContext::new(temp.path().join(".kanban"));
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_copy_tag_returns_clipboard_json() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = add_result["id"].as_str().unwrap();

        let result = CopyTag::new(tag_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["copied"], true);
        assert_eq!(result["entity_type"], "tag");

        let clip_json = result["clipboard_json"].as_str().unwrap();
        let payload = clipboard::deserialize_from_clipboard(clip_json)
            .expect("should deserialize clipboard payload");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "tag");
        assert_eq!(payload.swissarmyhammer_clipboard.entity_id, tag_id);
    }

    #[tokio::test]
    async fn test_copy_nonexistent_tag_fails() {
        let (_temp, ctx) = setup().await;
        let result = CopyTag::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();
        assert!(result.is_err());
    }
}
