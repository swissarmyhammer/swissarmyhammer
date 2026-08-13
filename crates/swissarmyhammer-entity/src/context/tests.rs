//! Tests for the [entity context](super).
//!
//! The tests are split by subject, one module for each. The review engine
//! renders a whole file into one agent prompt, and a file over the per-file
//! prompt cap is not reviewed at all, so a test tree this size has to be
//! several files rather than one.
//!
//! - [`basics`] — the entity directory and path, the plain and body-carrying
//!   round trip, listing, delete to trash, and the trash-layout migration.
//! - [`changelog`] — the guard that the entity layer writes one changelog
//!   record, not two.
//! - [`attachments`] — an attachment field through write, read, update and
//!   delete.
//! - [`enrichment`] — `enrich_attachment_fields` and `resolve_attachment_value`
//!   on their own.
//! - [`archive`] — archive, unarchive, and reading an archived entity.
//! - [`error_paths`] — what each operation returns for an unknown entity type,
//!   a missing entity, and an empty store.
//! - [`computed`] — computed fields, the changelog and file-created values the
//!   compute engine gets, and `list_where`.
//! - [`cache`] — the entity cache, and the events a delete or archive emits
//!   through undo and redo.
//!
//! This module carries what those eight share: the imports, the
//! store-backed context fixture, and the attachment fields context.

mod archive;
mod attachments;
mod basics;
mod cache;
mod changelog;
mod computed;
mod enrichment;
mod error_paths;
use super::*;
use crate::changelog::FieldChange;
use crate::test_utils::test_fields_context;
use serde_json::json;
use tempfile::TempDir;

/// Wire up an `EntityContext` with a `StoreHandle` registered for the
/// `tag` entity type. This is the production-realistic shape: every
/// write/delete/archive/unarchive routes through the store layer.
async fn ctx_with_tag_store(dir: &TempDir) -> EntityContext {
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let entity_dir = dir.path().join("tags");
    std::fs::create_dir_all(&entity_dir).unwrap();
    let entity_def = fields.get_entity("tag").unwrap();
    let field_defs: Vec<_> = fields
        .fields_for_entity("tag")
        .into_iter()
        .cloned()
        .collect();
    let store = crate::store::EntityTypeStore::new(
        &entity_dir,
        "tag",
        Arc::new(entity_def.clone()),
        Arc::new(field_defs),
    );
    let handle = Arc::new(swissarmyhammer_store::StoreHandle::new(Arc::new(store)));
    ctx.register_store("tag", handle).await;
    ctx
}

/// Build a FieldsContext with an entity type that has attachment fields.
fn attachment_fields_context() -> Arc<FieldsContext> {
    let defs = vec![
        (
            "title",
            "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
        ),
        (
            "avatar",
            "id: 00000000000000000000000AVT\nname: avatar\ntype:\n  kind: attachment\n  max_bytes: 1048576\n  multiple: false\n",
        ),
        (
            "files",
            "id: 00000000000000000000000FLS\nname: files\ntype:\n  kind: attachment\n  max_bytes: 1048576\n  multiple: true\n",
        ),
    ];
    let entities = vec![(
        "item",
        "name: item\nfields:\n  - title\n  - avatar\n  - files\n",
    )];

    let dir = TempDir::new().unwrap();
    Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
}
