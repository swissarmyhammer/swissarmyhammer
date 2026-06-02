//! UpdateBoard command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_config::model::{ModelExecutorType, ModelManager};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// The stable model id for the synthesized Claude Code entry.
///
/// `claude-code` is always a valid model id for a board — it bypasses the
/// `find_agent_by_name` lookup because the kanban-app synthesizes its entry
/// directly from [`ModelConfig::claude_code`] rather than reading a YAML file.
const CLAUDE_CODE_MODEL_ID: &str = "claude-code";

/// Validate that `model_id` names a chat-capable agent that can back the
/// board's AI panel.
///
/// `claude-code` is always accepted. Any other id must resolve via
/// [`ModelManager::find_agent_by_name`] and parse to a `ModelConfig` whose
/// executor is `ClaudeCode` or `LlamaAgent` — embedding executors
/// (`LlamaEmbedding`, `AneEmbedding`) cannot drive a chat agent.
///
/// This mirrors `resolve_model_config` in `apps/kanban-app/src/ai/models.rs`,
/// which is the runtime-side validator used when the agent is actually started.
/// Persisting an id here that the runtime would later reject would surface as
/// a confusing failure at agent-startup time, so the two checks must agree.
///
/// # Errors
///
/// Returns [`KanbanError::InvalidValue`] when the id is unknown, the agent
/// file is malformed, or the resolved config is not a runnable chat agent.
fn validate_model_id(model_id: &str) -> Result<()> {
    if model_id == CLAUDE_CODE_MODEL_ID {
        return Ok(());
    }

    let info = ModelManager::find_agent_by_name(model_id).map_err(|e| {
        KanbanError::invalid_value("model", format!("unknown model `{model_id}`: {e}"))
    })?;
    let config = swissarmyhammer_config::model::parse_model_config(&info.content).map_err(|e| {
        KanbanError::invalid_value(
            "model",
            format!("model `{model_id}` has an invalid configuration: {e}"),
        )
    })?;

    match config.executor_type() {
        ModelExecutorType::ClaudeCode | ModelExecutorType::LlamaAgent => Ok(()),
        other => Err(KanbanError::invalid_value(
            "model",
            format!("model `{model_id}` uses executor {other:?}, which cannot back a chat agent"),
        )),
    }
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
    /// New AI-panel model id (e.g. `claude-code`, `qwen`).
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

    /// Set the new AI-panel model id.
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
            .with_model("qwen")
            .execute(&ctx)
            .await
            .into_result()
            .expect("setting a kanban-tagged model must succeed");

        let yaml = std::fs::read_to_string(temp.path().join(".kanban/boards/board.yaml"))
            .expect("board.yaml must exist after UpdateBoard");
        assert!(
            yaml.contains("model: qwen"),
            "board.yaml must contain `model: qwen`, got:\n{yaml}"
        );
    }

    /// Setting a model and then `GetBoard`ing must round-trip the chosen id.
    #[tokio::test]
    async fn test_update_board_model_round_trips_via_get_board() {
        use crate::board::GetBoard;

        let (_temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("qwen")
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
            board["model"], "qwen",
            "GetBoard must report the model id that was just set"
        );
    }

    /// An unknown model id must be rejected — no agent file resolves it.
    #[tokio::test]
    async fn test_update_board_rejects_unknown_model() {
        let (_temp, ctx) = setup().await;

        let result = UpdateBoard::new()
            .with_model("bogus-xyz")
            .execute(&ctx)
            .await;

        match result {
            ExecutionResult::Failed { error } => {
                let msg = error.to_string();
                assert!(
                    msg.contains("bogus-xyz"),
                    "error must name the unknown model id, got: {msg}"
                );
            }
            other => panic!("expected Failed for unknown model id, got {other:?}"),
        }
    }

    /// Embedding executors can't back a chat agent and must be rejected even
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

    /// `claude-code` is always valid — the synthesized entry never goes
    /// through `find_agent_by_name`.
    #[tokio::test]
    async fn test_update_board_accepts_claude_code() {
        use crate::board::GetBoard;

        let (_temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("claude-code")
            .execute(&ctx)
            .await
            .into_result()
            .expect("claude-code must always be accepted");

        let board = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(board["model"], "claude-code");
    }

    /// `qwen` is the kanban-tagged local llama chat model — it must round-trip.
    #[tokio::test]
    async fn test_update_board_accepts_qwen() {
        use crate::board::GetBoard;

        let (_temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("qwen")
            .execute(&ctx)
            .await
            .into_result()
            .expect("the kanban-tagged `qwen` model must be accepted");

        let board = GetBoard::default()
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(board["model"], "qwen");
    }

    /// Updating only `name` after a model is set must NOT clear the model.
    /// The entity is read-modify-written, so untouched fields survive.
    #[tokio::test]
    async fn test_update_board_model_preserved_when_only_name_changes() {
        use crate::board::GetBoard;

        let (_temp, ctx) = setup().await;

        UpdateBoard::new()
            .with_model("qwen")
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
            board["model"], "qwen",
            "a name-only update must not clobber an existing model"
        );
    }
}
