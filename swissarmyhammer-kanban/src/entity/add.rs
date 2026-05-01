//! AddEntity command — generic, schema-driven entity creation.
//!
//! Creates a new entity of any type using field-level `default` values
//! declared in the field definition YAML. Task-shaped entities (those with
//! `position_column` / `position_ordinal` fields) get automatic column and
//! ordinal resolution, so `entity.add:task` drops into the lowest-order
//! column at the end of its ordinal list with no caller-supplied context.
//!
//! This is the backend that powers the dynamic `entity.add:{type}` command
//! surfaced from the active view scope by
//! `crate::scope_commands::emit_dynamic_commands`. Adding a new entity type
//! YAML plus a grid view YAML automatically gets a working "New {Type}"
//! action — no Rust code changes are required. The `entity.add:{type}`
//! moniker is rewritten to the canonical `entity.add` command in
//! `kanban-app/src/commands.rs::dispatch_command_internal` before being
//! routed into [`AddEntity`].
//!
//! Position-resolution logic (column and ordinal) is shared with
//! [`crate::task::AddTask`] via [`crate::entity::position`] so a fix in one
//! place propagates to both creation paths.

use crate::context::KanbanContext;
use crate::entity::position::{self, POSITION_COLUMN_FIELD, POSITION_ORDINAL_FIELD};
use crate::error::{KanbanError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Create a new entity of any type using field-default values.
///
/// For each field in the entity's schema, the field's declared `default`
/// (from the field definition YAML) is used as the initial value. Explicit
/// overrides supplied via `overrides` take precedence.
///
/// Entities with a `position_column` field get automatic placement:
/// - `column` override is used as-is, or
/// - if absent, the lowest-order column is picked.
///
/// Entities with a `position_ordinal` field get automatic appending:
/// - `ordinal` override is used as-is, or
/// - if absent, an ordinal strictly after the last entity in the resolved
///   column is generated (or `Ordinal::first()` when the column is empty).
#[operation(
    verb = "add",
    noun = "entity",
    description = "Create a new entity of any type with field-default values"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddEntity {
    /// The entity type (e.g. "task", "tag", "project")
    pub entity_type: String,
    /// Explicit field overrides that take precedence over schema defaults.
    ///
    /// Recognised position-related keys when the entity has `position_column`:
    /// - `column` — target column id; resolves to lowest-order column if absent.
    /// - `ordinal` — explicit fractional-index string; resolves to append-at-end
    ///   if absent.
    ///
    /// Any other key is written as a field value on the entity (after
    /// schema validation that the field actually belongs to the entity).
    #[serde(default)]
    pub overrides: HashMap<String, Value>,
}

impl AddEntity {
    /// Create a new AddEntity command for the given entity type with no overrides.
    pub fn new(entity_type: impl Into<String>) -> Self {
        Self {
            entity_type: entity_type.into(),
            overrides: HashMap::new(),
        }
    }

    /// Replace the full override map.
    pub fn with_overrides(mut self, overrides: HashMap<String, Value>) -> Self {
        self.overrides = overrides;
        self
    }

    /// Set a single override, overwriting any prior value for the same key.
    pub fn with_override(mut self, key: impl Into<String>, value: Value) -> Self {
        self.overrides.insert(key.into(), value);
        self
    }
}

/// Override-bag keys reserved for positional semantics.
///
/// These keys are consumed by [`position::resolve_column`] and
/// [`position::resolve_ordinal`] to drive kanban-column placement — they
/// are never written directly onto the entity as field values, even when
/// the entity schema happens to declare a field with the same name.
///
/// Both the dispatcher-convention names (`column`, `ordinal`) and the raw
/// field names ([`POSITION_COLUMN_FIELD`], [`POSITION_ORDINAL_FIELD`]) are
/// reserved. The dispatcher contract guarantees only the short names will
/// flow through, but reserving the field names too prevents a hostile or
/// buggy caller from bypassing [`apply_position`] by writing the resolved
/// columns directly via the override bag.
///
/// This reservation is a deliberate trade-off: the generic dispatch arg
/// bag is flat (no distinct "positional args" namespace), so the special
/// keys must be picked by name. If a future entity type ever needs to
/// declare a field literally named `column` or `ordinal`, this list is
/// where that collision would need to be resolved (e.g. by migrating to
/// sentinel keys like `_column` / `_ordinal`).
const RESERVED_POSITION_OVERRIDE_KEYS: &[&str] = &[
    "column",
    "ordinal",
    POSITION_COLUMN_FIELD,
    POSITION_ORDINAL_FIELD,
];

impl AddEntity {
    /// Apply schema field defaults to an entity in place.
    fn apply_defaults(
        entity: &mut Entity,
        entity_def: &swissarmyhammer_fields::EntityDef,
        fields_ctx: &swissarmyhammer_fields::FieldsContext,
    ) {
        for field_name in &entity_def.fields {
            let Some(field_def) = fields_ctx.get_field_by_name(field_name.as_str()) else {
                continue;
            };
            if let Some(default_value) = field_def.default.as_ref() {
                entity.set(field_name.as_str(), default_value.clone());
            }
        }
    }

    /// Apply `column` / `ordinal` placement when the entity type opts in.
    async fn apply_position(
        &self,
        entity: &mut Entity,
        entity_def: &swissarmyhammer_fields::EntityDef,
        ectx: &swissarmyhammer_entity::EntityContext,
    ) -> Result<()> {
        let has_position_column = entity_def
            .fields
            .iter()
            .any(|f| f.as_str() == POSITION_COLUMN_FIELD);
        if !has_position_column {
            return Ok(());
        }
        let explicit_column = self.overrides.get("column").and_then(|v| v.as_str());
        let column =
            position::resolve_column(ectx, explicit_column, self.entity_type.as_str()).await?;
        entity.set(POSITION_COLUMN_FIELD, json!(column));

        let has_position_ordinal = entity_def
            .fields
            .iter()
            .any(|f| f.as_str() == POSITION_ORDINAL_FIELD);
        if has_position_ordinal {
            let explicit_ordinal = self.overrides.get("ordinal").and_then(|v| v.as_str());
            let ordinal = position::resolve_ordinal(
                ectx,
                self.entity_type.as_str(),
                &column,
                explicit_ordinal,
            )
            .await?;
            entity.set(POSITION_ORDINAL_FIELD, json!(ordinal));
        }
        Ok(())
    }

    /// Apply explicit field overrides, ignoring reserved keys and unknown fields.
    ///
    /// Unknown or positional-only keys are silently ignored: the dispatch layer
    /// passes through a generic arg bag and a stray `column` arg on an entity
    /// without placement should not surface as a hard error.
    fn apply_overrides(&self, entity: &mut Entity, entity_def: &swissarmyhammer_fields::EntityDef) {
        for (key, value) in &self.overrides {
            if RESERVED_POSITION_OVERRIDE_KEYS.contains(&key.as_str()) {
                continue;
            }
            if !entity_def.fields.iter().any(|f| f.as_str() == key) {
                continue;
            }
            entity.set(key, value.clone());
        }
    }

    /// Build and persist the new entity, returning its JSON representation.
    async fn build_and_write(&self, ctx: &KanbanContext) -> Result<Value> {
        let ectx = ctx.entity_context().await?;
        let fields_ctx = ectx.fields();

        let entity_def = fields_ctx
            .get_entity(&self.entity_type)
            .ok_or_else(|| {
                KanbanError::parse(format!("unknown entity type: '{}'", self.entity_type))
            })?
            .clone();

        let id = ulid::Ulid::new().to_string();
        let mut entity = Entity::new(self.entity_type.as_str(), id.as_str());

        Self::apply_defaults(&mut entity, &entity_def, fields_ctx);
        self.apply_position(&mut entity, &entity_def, &ectx).await?;
        self.apply_overrides(&mut entity, &entity_def);

        ectx.write(&entity).await?;
        Ok(entity.to_json())
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddEntity {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = self.build_and_write(ctx).await;

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

    fn affected_resource_ids(&self, result: &Value) -> Vec<String> {
        result
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
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

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn add_tag_with_defaults_sets_tag_name() {
        // `tag_name.yaml` declares default: "new-tag". A generic
        // entity.add:tag should therefore create a tag with that value.
        let (_temp, ctx) = setup().await;

        let result = AddEntity::new("tag")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["tag_name"], "new-tag");
        assert!(!result["id"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn add_task_defaults_to_lowest_order_column() {
        // The default board has todo (order 0) / doing / done. Adding a
        // task with no column override must land it in "todo".
        let (_temp, ctx) = setup().await;

        let result = AddEntity::new("task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(
            result["position_column"], "todo",
            "task must be placed in lowest-order column when no override given"
        );
        // Schema default "Untitled" must be applied to the title field.
        assert_eq!(result["title"], "Untitled");
    }

    #[tokio::test]
    async fn add_task_with_explicit_column_override_honored() {
        // When the caller provides `column` in overrides, that column wins
        // over the lowest-order auto-resolution.
        let (_temp, ctx) = setup().await;

        let result = AddEntity::new("task")
            .with_override("column", json!("doing"))
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["position_column"], "doing");
    }

    #[tokio::test]
    async fn add_task_appends_ordinal_after_existing_tasks() {
        let (_temp, ctx) = setup().await;

        let first = AddEntity::new("task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let second = AddEntity::new("task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ord1 = first["position_ordinal"].as_str().unwrap();
        let ord2 = second["position_ordinal"].as_str().unwrap();
        assert!(
            ord2 > ord1,
            "second task ordinal must sort after first: {ord1:?} vs {ord2:?}"
        );
    }

    #[tokio::test]
    async fn add_tag_ignores_column_override_silently() {
        // Tags don't have position_column, so a stray `column` arg flowing
        // through from the generic dispatcher must not poison the entity
        // or produce an error.
        let (_temp, ctx) = setup().await;

        let result = AddEntity::new("tag")
            .with_override("column", json!("todo"))
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert!(result.get("position_column").is_none() || result["position_column"].is_null());
    }

    #[tokio::test]
    async fn add_unknown_entity_type_errors() {
        let (_temp, ctx) = setup().await;
        let result = AddEntity::new("nonsense").execute(&ctx).await.into_result();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn add_entity_with_explicit_field_override() {
        // An explicit field value in overrides must win over the schema default.
        let (_temp, ctx) = setup().await;

        let result = AddEntity::new("tag")
            .with_override("tag_name", json!("custom-tag"))
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["tag_name"], "custom-tag");
    }

    #[tokio::test]
    async fn add_project_uses_name_default() {
        // `name.yaml` declares default: "New item" — projects use `name`,
        // so the creation should populate that field.
        let (_temp, ctx) = setup().await;

        let result = AddEntity::new("project")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "New item");
    }
}
