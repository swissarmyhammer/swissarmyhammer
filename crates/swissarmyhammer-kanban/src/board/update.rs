//! UpdateBoard command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_config::model::ModelManager;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Validate that `model_id` can back the board's chat agent.
///
/// Claude Code is the only chat executor, so the board's model is the value of
/// the Claude CLI `--model` switch — `haiku`, `sonnet`, `opus`, or a full model
/// id. The Claude CLI owns that vocabulary, so any non-blank value is accepted
/// except the name of a model in the SwissArmyHammer model library: every model
/// YAML there declares an embedding executor, which cannot drive a chat agent.
///
/// # Errors
///
/// Returns [`KanbanError::InvalidValue`] when the id is blank, or when it names
/// an embedding model.
fn validate_model_id(model_id: &str) -> Result<()> {
    if model_id.trim().is_empty() {
        return Err(KanbanError::invalid_value(
            "model",
            "model must be a Claude CLI --model switch (e.g. `haiku`), not blank",
        ));
    }

    if ModelManager::find_agent_by_name(model_id).is_ok() {
        return Err(KanbanError::invalid_value(
            "model",
            format!("model `{model_id}` names an embedding model, which cannot back a chat agent"),
        ));
    }

    Ok(())
}

/// Update board metadata
#[operation(
    verb = "update",
    noun = "board",
    description = "Update board name or description"
)]
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct UpdateBoard {
    /// New board name
    pub name: Option<String>,
    /// New board description
    pub description: Option<String>,
    /// New chat-agent model id — the Claude CLI `--model` switch (e.g. `haiku`).
    ///
    /// `None` leaves the existing `model` field on the board entity untouched;
    /// `Some(id)` writes the id after validating it via [`validate_model_id`].
    pub model: Option<String>,
}

impl UpdateBoard {
    /// Create a new UpdateBoard command
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            model: None,
        }
    }

    /// Set the new name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the new description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the new chat-agent model id.
    ///
    /// The id is validated at `execute` time, not here, so the builder stays
    /// infallible and matches the shape of `with_name` / `with_description`.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateBoard {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity =
                ectx.read("board", "board")
                    .await
                    .map_err(|_| KanbanError::NotInitialized {
                        path: ctx.root().to_path_buf(),
                    })?;

            if let Some(name) = &self.name {
                entity.set("name", json!(name));
            }
            if let Some(desc) = &self.description {
                entity.set("description", json!(desc));
            }
            if let Some(model) = &self.model {
                validate_model_id(model)?;
                entity.set("model", json!(model));
            }

            ectx.write(&entity).await?;

            Ok(json!({
                "name": entity.get_str("name").unwrap_or(""),
                "description": entity.get_str("description"),
                "model": entity.get_str("model"),
            }))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Original")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_update_board_name() {
        let (_temp, ctx) = setup().await;

        let cmd = UpdateBoard::new().with_name("Updated Name");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["name"], "Updated Name");
    }

    #[tokio::test]
    async fn test_update_board_description() {
        let (_temp, ctx) = setup().await;

        let cmd = UpdateBoard::new().with_description("New description");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["description"], "New description");
    }

    /// Setting a valid model id must persist `model: <id>` to the raw
    /// `.kanban/boards/board.yaml` on disk — the storage contract this whole
    /// task hinges on.
    #[tokio::test]
    async fn test_update_board_model_persists_to_yaml() {
        let (temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("haiku")
            .execute(&ctx)
            .await
            .into_result()
            .expect("setting a Claude CLI --model switch must succeed");

        let yaml = std::fs::read_to_string(temp.path().join(".kanban/boards/board.yaml"))
            .expect("board.yaml must exist after UpdateBoard");
        assert!(
            yaml.contains("model: haiku"),
            "board.yaml must contain `model: haiku`, got:\n{yaml}"
        );
    }

    /// Setting a model and then `GetBoard`ing must round-trip the chosen id.
    #[tokio::test]
    async fn test_update_board_model_round_trips_via_get_board() {
        use crate::board::GetBoard;

        let (_temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("haiku")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let board = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(
            board["model"], "haiku",
            "GetBoard must report the model id that was just set"
        );
    }

    /// A blank model id must be rejected — it would spawn `claude --model ""`.
    #[tokio::test]
    async fn test_update_board_rejects_blank_model() {
        let (_temp, ctx) = setup().await;

        let result = UpdateBoard::new().with_model("   ").execute(&ctx).await;

        match result {
            ExecutionResult::Failed { error } => {
                let msg = error.to_string();
                assert!(
                    msg.contains("--model"),
                    "error must name the setting to fix, got: {msg}"
                );
            }
            other => panic!("expected Failed for a blank model id, got {other:?}"),
        }
    }

    /// Embedding models can't back a chat agent and must be rejected even
    /// though `find_agent_by_name` finds them.
    #[tokio::test]
    async fn test_update_board_rejects_embedding_model() {
        let (_temp, ctx) = setup().await;

        let result = UpdateBoard::new()
            .with_model("qwen-embedding")
            .execute(&ctx)
            .await;

        match result {
            ExecutionResult::Failed { error } => {
                let msg = error.to_string();
                assert!(
                    msg.contains("qwen-embedding"),
                    "error must name the rejected model id, got: {msg}"
                );
            }
            other => panic!("expected Failed for embedding model, got {other:?}"),
        }
    }

    /// A full Claude model id is a valid `--model` switch and must round-trip.
    #[tokio::test]
    async fn test_update_board_accepts_full_claude_model_id() {
        use crate::board::GetBoard;

        let (_temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("claude-opus-4-20250514")
            .execute(&ctx)
            .await
            .into_result()
            .expect("a full Claude model id must be accepted");

        let board = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(board["model"], "claude-opus-4-20250514");
    }

    /// Updating only `name` after a model is set must NOT clear the model.
    /// The entity is read-modify-written, so untouched fields survive.
    #[tokio::test]
    async fn test_update_board_model_preserved_when_only_name_changes() {
        use crate::board::GetBoard;

        let (_temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("haiku")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        UpdateBoard::new()
            .with_name("Renamed")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let board = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(board["name"], "Renamed");
        assert_eq!(
            board["model"], "haiku",
            "a name-only update must not clobber an existing model"
        );
    }
}
