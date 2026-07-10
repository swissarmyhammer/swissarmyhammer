//! UpdateEntityField command

use crate::auto_color;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag::tag_name_exists_entity;
use crate::tag_parser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_entity::EntityContext;
use swissarmyhammer_fields::types::EntityDef;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Parse `#tag` patterns from an entity's body field and auto-create tag entities
/// for any that don't already exist.
async fn auto_create_tags(
    ectx: &EntityContext,
    entity: &Entity,
    entity_def: &EntityDef,
) -> std::result::Result<(), KanbanError> {
    let body_field = entity_def.body_field.as_deref().unwrap_or("body");
    let body = entity.get_str(body_field).unwrap_or("");
    let tags = tag_parser::parse_tags(body);
    for tag_name in &tags {
        if !tag_name_exists_entity(ectx, tag_name).await {
            let color = auto_color::auto_color(tag_name).to_string();
            let tag_id = ulid::Ulid::new().to_string();
            let mut tag_entity = Entity::new("tag", tag_id.as_str());
            tag_entity.set("tag_name", json!(tag_name));
            tag_entity.set("color", json!(color));
            ectx.write(&tag_entity).await?;
        }
    }
    Ok(())
}

/// Update a single field on any entity.
///
/// Generic command that works with any entity type (task, tag, actor, etc.).
/// Validates the field name against the entity's schema, reads the entity,
/// sets (or removes if null) the field, and writes it back.
#[operation(
    verb = "update",
    noun = "entity field",
    description = "Update a single field on any entity"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateEntityField {
    /// The entity type (e.g. "task", "tag", "actor", "column")
    pub entity_type: String,
    /// The entity ID
    pub id: String,
    /// The field name to update
    pub field_name: String,
    /// The new value (null to remove the field)
    pub value: Value,
}

impl UpdateEntityField {
    /// Create a new UpdateEntityField command.
    pub fn new(
        entity_type: impl Into<String>,
        id: impl Into<String>,
        field_name: impl Into<String>,
        value: Value,
    ) -> Self {
        Self {
            entity_type: entity_type.into(),
            id: id.into(),
            field_name: field_name.into(),
            value,
        }
    }
}

impl UpdateEntityField {
    /// Verify `field_name` is declared on the entity's schema.
    ///
    /// Returns [`KanbanError::InvalidValue`] for a field the entity type does
    /// not define, so an unknown field fails before any read/write.
    fn validate_field(&self, entity_def: &EntityDef) -> std::result::Result<(), KanbanError> {
        if entity_def
            .fields
            .iter()
            .any(|f| f.as_str() == self.field_name)
        {
            return Ok(());
        }
        Err(KanbanError::InvalidValue {
            field: self.field_name.clone(),
            message: format!(
                "field '{}' is not defined for entity type '{}'",
                self.field_name, self.entity_type
            ),
        })
    }

    /// Update a writable computed field by routing through its `DeriveHandler`.
    ///
    /// The handler rewrites the entity's source fields (e.g. editing `tags`
    /// rewrites `#tag` mentions in `body`); the entity is then written and any
    /// newly-mentioned tags are auto-created. Errors if no handler is registered
    /// for `derive` or the handler is read-only.
    async fn handle_computed_field(
        &self,
        ctx: &KanbanContext,
        ectx: &EntityContext,
        entity_def: &EntityDef,
        derive: &str,
    ) -> std::result::Result<Value, KanbanError> {
        let handler =
            ctx.derive_registry()
                .get(derive)
                .ok_or_else(|| KanbanError::InvalidValue {
                    field: self.field_name.clone(),
                    message: format!("no derive handler registered for '{}'", derive),
                })?;
        if !handler.writable() {
            return Err(KanbanError::InvalidValue {
                field: self.field_name.clone(),
                message: "computed field is read-only".into(),
            });
        }

        let mut entity = ectx
            .read(&self.entity_type, &self.id)
            .await
            .map_err(KanbanError::from_entity_error)?;

        handler
            .apply(&mut entity.fields, entity_def, &self.value)
            .map_err(|e| KanbanError::InvalidValue {
                field: self.field_name.clone(),
                message: e.to_string(),
            })?;

        ectx.write(&entity).await?;

        // Auto-create tag entities for any new tags in the body
        auto_create_tags(ectx, &entity, entity_def).await?;

        Ok(entity.to_json())
    }

    /// Update a comment-log field by merging the incoming (possibly stale) UI
    /// array against the stored log.
    ///
    /// Server-assigned ids and authors, explicit tombstone deletes, and
    /// concurrent appends are all preserved — all comment/actor logic stays in
    /// this crate via [`crate::comment::normalize_comment_log`].
    async fn handle_comment_log(
        &self,
        ctx: &KanbanContext,
        ectx: &EntityContext,
    ) -> std::result::Result<Value, KanbanError> {
        let mut entity = ectx
            .read(&self.entity_type, &self.id)
            .await
            .map_err(KanbanError::from_entity_error)?;

        let old = entity.get(&self.field_name).cloned().unwrap_or(Value::Null);
        let normalized = crate::comment::normalize_comment_log(ctx, &old, &self.value).await?;
        entity.set(&self.field_name, normalized);

        ectx.write(&entity).await?;

        Ok(entity.to_json())
    }

    /// Update a plain (non-computed, non-comment-log) field with a direct
    /// read-set-write.
    ///
    /// A null value removes the field; otherwise it is set. Tag entities are
    /// auto-created when a body field is updated directly.
    async fn handle_normal_field(
        &self,
        ectx: &EntityContext,
        entity_def: &EntityDef,
    ) -> std::result::Result<Value, KanbanError> {
        let mut entity = ectx
            .read(&self.entity_type, &self.id)
            .await
            .map_err(KanbanError::from_entity_error)?;

        if self.value.is_null() {
            entity.remove(&self.field_name);
        } else {
            entity.set(&self.field_name, self.value.clone());
        }

        ectx.write(&entity).await?;

        // Auto-create tag entities when a body field is updated directly
        auto_create_tags(ectx, &entity, entity_def).await?;

        Ok(entity.to_json())
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateEntityField {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: std::result::Result<Value, KanbanError> = async {
            let ectx = ctx.entity_context().await?;

            // Validate field_name against the entity's schema
            let entity_def = ectx
                .entity_def(&self.entity_type)
                .map_err(KanbanError::from_entity_error)?;
            self.validate_field(entity_def)?;

            // Route computed and comment-log fields through their handlers;
            // everything else is a plain field update.
            let field_def = ectx.fields().get_field_by_name(&self.field_name);
            if let Some(field_def) = field_def {
                if let swissarmyhammer_fields::FieldType::Computed { ref derive, .. } =
                    field_def.type_
                {
                    return self
                        .handle_computed_field(ctx, &ectx, entity_def, derive)
                        .await;
                }

                if matches!(
                    field_def.type_,
                    swissarmyhammer_fields::FieldType::CommentLog {}
                ) {
                    return self.handle_comment_log(ctx, &ectx).await;
                }
            }

            self.handle_normal_field(&ectx, entity_def).await
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
    use crate::task::AddTask;
    use crate::test_support::setup;
    use swissarmyhammer_operations::Execute;

    #[tokio::test]
    async fn test_update_entity_field_set_value() {
        let (_temp, ctx) = setup().await;

        // Create a task first
        let task_result = AddTask::new("Original title")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Update the title field
        let cmd = UpdateEntityField::new("task", &task_id, "title", serde_json::json!("New title"));
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["title"], "New title");
        assert_eq!(result["id"], task_id);
    }

    #[tokio::test]
    async fn test_update_entity_field_remove_value() {
        let (_temp, ctx) = setup().await;

        // Create a task with a description
        let task_result = AddTask::new("Test task")
            .with_description("Some description")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Remove the body field by setting it to null
        let cmd = UpdateEntityField::new("task", &task_id, "body", Value::Null);
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        // The body field should be absent from the result
        assert!(result.get("body").is_none() || result["body"].is_null());
    }

    #[tokio::test]
    async fn test_update_entity_field_invalid_field() {
        let (_temp, ctx) = setup().await;

        // Create a task
        let task_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Try to update a field that doesn't exist on tasks
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "nonexistent_field",
            serde_json::json!("value"),
        );
        let result = cmd.execute(&ctx).await.into_result();

        assert!(result.is_err(), "Should fail for undefined field");
    }

    #[tokio::test]
    async fn test_update_body_auto_creates_tag_entities() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Tag test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Update the body to include a #hashtag
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "body",
            serde_json::json!("Fix the #autotest issue"),
        );
        cmd.execute(&ctx).await.into_result().unwrap();

        // The tag entity should now exist
        let ectx = ctx.entity_context().await.unwrap();
        let tags = ectx.list("tag").await.unwrap();
        let found = tags
            .iter()
            .any(|t| t.get_str("tag_name") == Some("autotest"));
        assert!(found, "Tag entity 'autotest' should have been auto-created");
    }

    #[tokio::test]
    async fn test_update_body_does_not_duplicate_existing_tags() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Tag test")
            .with_description("Already has #existing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Tag 'existing' was auto-created by AddTask. Update body with same tag.
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "body",
            serde_json::json!("Still has #existing tag"),
        );
        cmd.execute(&ctx).await.into_result().unwrap();

        // Should still be exactly one tag entity named 'existing'
        let ectx = ctx.entity_context().await.unwrap();
        let tags = ectx.list("tag").await.unwrap();
        let count = tags
            .iter()
            .filter(|t| t.get_str("tag_name") == Some("existing"))
            .count();
        assert_eq!(count, 1, "Should not duplicate existing tag");
    }

    #[tokio::test]
    async fn test_update_computed_tags_via_derive_handler() {
        let (_temp, ctx) = setup().await;

        // Create a task with a tag in the body
        let task_result = AddTask::new("Derive test")
            .with_description("Has #original tag")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Update the computed "tags" field — should route through ParseBodyTags handler
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "tags",
            serde_json::json!(["original", "added"]),
        );
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        // The body should now contain both tags
        let body = result["body"].as_str().unwrap();
        let tags = crate::tag_parser::parse_tags(body);
        assert!(
            tags.contains(&"original".to_string()),
            "original tag preserved"
        );
        assert!(
            tags.contains(&"added".to_string()),
            "new tag added via derive handler"
        );

        // Tag entity for "added" should have been auto-created
        let ectx = ctx.entity_context().await.unwrap();
        let tag_entities = ectx.list("tag").await.unwrap();
        assert!(
            tag_entities
                .iter()
                .any(|t| t.get_str("tag_name") == Some("added")),
            "Tag entity 'added' should have been auto-created"
        );

        // The user-visible computed `tags` field — read through the real read
        // path that runs the ComputeEngine — must reflect the body, including
        // both the preserved and the newly-added tag. The original test only
        // asserted on the raw body string, so the read-path filter bug (tags
        // not showing in the field) was invisible to it.
        let read_back = ectx.read("task", &task_id).await.unwrap();
        let field_tags = crate::task_helpers::task_tags(&read_back);
        assert!(
            field_tags.contains(&"original".to_string()),
            "computed tags field should include the preserved tag, got {field_tags:?}"
        );
        assert!(
            field_tags.contains(&"added".to_string()),
            "computed tags field should include the added tag, got {field_tags:?}"
        );
    }

    /// Direction B (body → field) through the real read path: a `#tag` typed
    /// into the body surfaces in the computed `tags` field even when NO `tag`
    /// entity exists for it. Writing the task entity directly bypasses
    /// `auto_create_tags`, isolating the read-path derivation. Under the old
    /// `known.contains(...)` existence filter this yielded an empty `tags`
    /// field — the user-reported "tags not showing" bug.
    #[tokio::test]
    async fn test_body_tag_surfaces_in_field_without_existing_tag_entity() {
        let (_temp, ctx) = setup().await;
        let ectx = ctx.entity_context().await.unwrap();

        // Write a task whose body mentions #unregistered, with no tag entities
        // present at all (direct write skips auto-create).
        let mut task = Entity::new("task", "01TAGSYNCREAD");
        task.set("title", json!("Sync test"));
        task.set("body", json!("Investigate the #unregistered issue"));
        ectx.write(&task).await.unwrap();
        assert!(
            ectx.list("tag").await.unwrap().is_empty(),
            "precondition: no tag entities exist to validate against"
        );

        // Reading through the real path runs the ComputeEngine; the body tag
        // must appear in the computed field regardless of tag-entity existence.
        let read_back = ectx.read("task", "01TAGSYNCREAD").await.unwrap();
        let field_tags = crate::task_helpers::task_tags(&read_back);
        assert_eq!(
            field_tags,
            vec!["unregistered".to_string()],
            "body is the source of truth: the #unregistered tag must surface in the field"
        );
    }

    /// Real production write path: the app's `entity.update_field` command
    /// routes through `EntityContext::update_field` (see the
    /// `swissarmyhammer-entity-mcp` `handle_update_field`), NOT the kanban
    /// `UpdateEntityField` op. Editing the computed `tags` field must run the
    /// `parse-body-tags` derive handler's `apply` — rewriting `#tag` mentions
    /// in the body — instead of blindly storing the value (which the read-path
    /// compute would discard, the user-reported "nothing saves" bug). Covers
    /// BOTH directions through the kanban-wired context that the app shares.
    #[tokio::test]
    async fn test_entity_context_update_field_routes_tags_through_body() {
        let (_temp, ctx) = setup().await;
        let ectx = ctx.entity_context().await.unwrap();

        let task_result = AddTask::new("Routing test")
            .with_description("Initial work")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Direction A (field → body): set the computed `tags` field via the
        // shared write path. The derive handler must append the tags to body.
        let entry_id = ectx
            .update_field(
                "task",
                &task_id,
                "tags",
                serde_json::json!(["alpha", "beta"]),
            )
            .await
            .unwrap();
        assert!(
            entry_id.is_some(),
            "setting the computed tag field must edit the body and record a change \
             (entry_id None means the no-op bug is still present)"
        );

        let read1 = ectx.read("task", &task_id).await.unwrap();
        let body1 = read1.get_str("body").unwrap_or("").to_string();
        let parsed1 = crate::tag_parser::parse_tags(&body1);
        assert!(
            parsed1.contains(&"alpha".to_string()) && parsed1.contains(&"beta".to_string()),
            "body must gain #alpha and #beta, got {body1:?}"
        );
        let mut tags1 = crate::task_helpers::task_tags(&read1);
        tags1.sort();
        assert_eq!(tags1, vec!["alpha".to_string(), "beta".to_string()]);

        // Removing a tag via the field strips it from the body.
        ectx.update_field("task", &task_id, "tags", serde_json::json!(["alpha"]))
            .await
            .unwrap();
        let read2 = ectx.read("task", &task_id).await.unwrap();
        assert_eq!(
            crate::task_helpers::task_tags(&read2),
            vec!["alpha".to_string()]
        );
        assert!(!read2.get_str("body").unwrap_or("").contains("#beta"));

        // Direction B (body → field): editing the body via the same path
        // surfaces a brand-new tag in the computed field — no tag entity needed.
        ectx.update_field(
            "task",
            &task_id,
            "body",
            serde_json::json!("Reworked #gamma now"),
        )
        .await
        .unwrap();
        let read3 = ectx.read("task", &task_id).await.unwrap();
        assert_eq!(
            crate::task_helpers::task_tags(&read3),
            vec!["gamma".to_string()]
        );
    }

    /// Field-change EVENT coverage for the UI re-render path — the bug the
    /// user hit: the tag was saved to the body but the card never re-rendered
    /// (no `tags` field-change event fired). Editing the computed `tags` field
    /// rewrites the body via the derive handler, so the emitted EntityChanged
    /// event MUST include a `tags` field-change (recomputed from the new body),
    /// or the frontend's field-level re-render never happens and the tag
    /// "doesn't save" from the user's view. Fails without the compute-aware
    /// diff in EntityCache::write (raw diff only sees `body` change).
    #[tokio::test]
    async fn test_update_tags_field_emits_tags_field_change_event() {
        use swissarmyhammer_entity::EntityEvent;

        let (_temp, ctx) = setup().await;
        let ectx = ctx.entity_context().await.unwrap();
        let cache = ctx
            .entity_cache()
            .expect("entity_cache initialized by entity_context()");
        let mut rx = cache.subscribe();

        let task_result = AddTask::new("Event test")
            .with_description("Some work")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // Drain the AddTask events so we only inspect the tag update below.
        while rx.try_recv().is_ok() {}

        ectx.update_field("task", &task_id, "tags", serde_json::json!(["bug"]))
            .await
            .unwrap();

        // The EntityChanged event for the task must carry a `tags` field-change
        // with the recomputed value.
        let mut saw_tags = None;
        while let Ok(evt) = rx.try_recv() {
            if let EntityEvent::EntityChanged { id, changes, .. } = evt {
                if id == task_id {
                    for ch in &changes {
                        if ch.field == "tags" {
                            saw_tags = Some(ch.value.clone());
                        }
                    }
                }
            }
        }
        assert_eq!(
            saw_tags,
            Some(serde_json::json!(["bug"])),
            "editing the tags field must emit a `tags` field-change event so the UI re-renders the field"
        );
    }

    #[tokio::test]
    async fn test_update_computed_field_read_only_returns_error() {
        // This test verifies that the derive handler routing checks writable().
        // Since ParseBodyTags is writable, we test the invalid-field path instead:
        // a computed field with no registered handler returns an error.
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Error test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // "progress" is a computed field with derive: parse-body-progress
        // but no DeriveHandler is registered for it in kanban_derive_registry
        let cmd = UpdateEntityField::new("task", &task_id, "progress", serde_json::json!(0.5));
        let result = cmd.execute(&ctx).await.into_result();
        assert!(
            result.is_err(),
            "Should fail when no derive handler registered"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no derive handler"),
            "Error should mention missing handler: {}",
            err
        );
    }

    /// The full UI field-set flow for the comment log: a fresh commit
    /// normalizes a new member, a stale commit must not drop a concurrent
    /// agent append, and an explicit tombstone deletes.
    #[tokio::test]
    async fn test_update_comments_field_normalizes_and_merges() {
        let (_temp, ctx) = setup().await;
        let os_user = swissarmyhammer_common::slug(&whoami::username());

        let task_result = AddTask::new("Comment log task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        // 1. Commit a new member with no id — server assigns id/actor/timestamp.
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "comments",
            serde_json::json!([{"text": "hi"}]),
        );
        cmd.execute(&ctx).await.into_result().unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", &task_id).await.unwrap();
        let comments = task.get("comments").unwrap().as_array().unwrap().clone();
        assert_eq!(comments.len(), 1);
        let first_id = comments[0]["id"].as_str().unwrap().to_string();
        assert_eq!(first_id.len(), 26, "member must get a ULID id");
        assert_eq!(comments[0]["actor"], os_user.as_str(), "OS-user fallback");
        chrono::DateTime::parse_from_rfc3339(comments[0]["timestamp"].as_str().unwrap())
            .expect("timestamp must be RFC3339");

        // 2. Agent appends concurrently; the UI then commits a stale array
        //    (text edit of the first member, agent comment absent).
        crate::comment::AddComment::new(task_id.as_str(), "agent progress")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "comments",
            serde_json::json!([{"id": first_id, "text": "hi (edited)"}]),
        );
        cmd.execute(&ctx).await.into_result().unwrap();

        let task = ectx.read("task", &task_id).await.unwrap();
        let comments = task.get("comments").unwrap().as_array().unwrap().clone();
        assert_eq!(comments.len(), 2, "agent comment must survive stale commit");
        assert_eq!(comments[0]["id"], first_id.as_str());
        assert_eq!(comments[0]["text"], "hi (edited)");
        assert_eq!(comments[1]["text"], "agent progress");

        // 3. Explicit tombstone deletes the first member only.
        let agent_id = comments[1]["id"].as_str().unwrap().to_string();
        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "comments",
            serde_json::json!([
                {"id": first_id, "deleted": true},
                {"id": agent_id, "text": "agent progress"},
            ]),
        );
        cmd.execute(&ctx).await.into_result().unwrap();

        let task = ectx.read("task", &task_id).await.unwrap();
        let comments = task.get("comments").unwrap().as_array().unwrap().clone();
        assert_eq!(comments.len(), 1, "tombstoned member must be gone");
        assert_eq!(comments[0]["id"], agent_id.as_str());
        assert_eq!(comments[0]["text"], "agent progress");
    }

    /// A new member naming an unknown explicit actor fails the whole
    /// field-set (reuses resolve_comment_author validation).
    #[tokio::test]
    async fn test_update_comments_field_unknown_actor_errors() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Actor validation")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap().to_string();

        let cmd = UpdateEntityField::new(
            "task",
            &task_id,
            "comments",
            serde_json::json!([{"text": "hi", "actor": "ghost"}]),
        );
        let result = cmd.execute(&ctx).await.into_result();
        assert!(
            matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "ghost"),
            "expected ActorNotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_update_entity_field_entity_not_found() {
        let (_temp, ctx) = setup().await;

        // Try to update a task that doesn't exist
        let cmd = UpdateEntityField::new(
            "task",
            "nonexistent_id",
            "title",
            serde_json::json!("value"),
        );
        let result = cmd.execute(&ctx).await.into_result();

        assert!(result.is_err(), "Should fail for nonexistent entity");
    }
}
