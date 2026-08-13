//! Computed fields, and the values the compute engine gets.
//!
//! The changelog and the file-created time are injected only when a
//! computed field depends on them, and are stripped again after the
//! derivation. `list_where` filters on the derived result.

use super::*;

#[tokio::test]
async fn enrich_attachment_fields_without_compute_engine() {
    // Test that attachment enrichment happens even without a compute engine
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);
    // No .with_compute() — but attachment enrichment should still work

    let source = dir.path().join("photo.png");
    tokio::fs::write(&source, b"png data").await.unwrap();

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    let read = ctx.read("item", "01TEST").await.unwrap();
    let meta = read.fields.get("avatar").unwrap();
    assert!(
        meta.is_object(),
        "attachment should be enriched without compute engine"
    );
    assert_eq!(meta["name"], "photo.png");
}

#[tokio::test]
async fn list_with_compute_engine_enriches_entities() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let compute = Arc::new(swissarmyhammer_fields::ComputeEngine::new());
    let ctx = EntityContext::new(dir.path(), fields.clone()).with_compute(compute);

    let mut t1 = Entity::new("tag", "t1");
    t1.set("tag_name", json!("One"));
    let mut t2 = Entity::new("tag", "t2");
    t2.set("tag_name", json!("Two"));
    ctx.write(&t1).await.unwrap();
    ctx.write(&t2).await.unwrap();

    let tags = ctx.list("tag").await.unwrap();
    assert_eq!(tags.len(), 2);
}

#[tokio::test]
async fn read_with_compute_engine_derives_fields() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let compute = Arc::new(swissarmyhammer_fields::ComputeEngine::new());
    let ctx = EntityContext::new(dir.path(), fields.clone()).with_compute(compute);

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    let loaded = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Bug"));
}

// -----------------------------------------------------------------------
// list_where tests (from kanban branch)
// -----------------------------------------------------------------------

#[tokio::test]
async fn list_where_filters_by_field() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut t1 = Entity::new("tag", "bug");
    t1.set("tag_name", json!("Bug"));
    t1.set("color", json!("#ff0000"));
    let mut t2 = Entity::new("tag", "feature");
    t2.set("tag_name", json!("Feature"));
    t2.set("color", json!("#00ff00"));

    ctx.write(&t1).await.unwrap();
    ctx.write(&t2).await.unwrap();

    let result = ctx
        .list_where(
            "tag",
            |entities| crate::filter::EntityFilterContext::new(entities),
            |entity, _ctx| entity.get_str("tag_name") == Some("Bug"),
        )
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id.as_ref(), "bug");
}

#[tokio::test]
async fn list_where_with_context_extra() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut t1 = Entity::new("tag", "bug");
    t1.set("tag_name", json!("Bug"));
    let mut t2 = Entity::new("tag", "feature");
    t2.set("tag_name", json!("Feature"));

    ctx.write(&t1).await.unwrap();
    ctx.write(&t2).await.unwrap();

    // Inject a set of allowed tag names via extras
    let allowed: std::collections::HashSet<String> =
        ["Feature"].iter().map(|s| s.to_string()).collect();

    let result = ctx
        .list_where(
            "tag",
            |entities| {
                let mut fctx = crate::filter::EntityFilterContext::new(entities);
                fctx.insert(allowed.clone());
                fctx
            },
            |entity, fctx| {
                let allowed = fctx.get::<std::collections::HashSet<String>>().unwrap();
                entity
                    .get_str("tag_name")
                    .is_some_and(|name| allowed.contains(name))
            },
        )
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id.as_ref(), "feature");
}

// -----------------------------------------------------------------------
// _changelog injection tests
// -----------------------------------------------------------------------

/// Build a FieldsContext whose "task" entity includes a computed field
/// that depends on `_changelog`.
fn fields_context_with_changelog_computed() -> Arc<FieldsContext> {
    let defs = vec![
        (
            "title",
            "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
        ),
        (
            "body",
            "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
        ),
        (
            "change_count",
            "id: 00000000000000000000000CHG\nname: change_count\ntype:\n  kind: computed\n  derive: count-changelog\n  depends_on:\n    - _changelog\n",
        ),
    ];
    let entities = vec![(
        "task",
        "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - change_count\n",
    )];
    let dir = TempDir::new().unwrap();
    Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
}

/// Build a FieldsContext whose "task" entity has a computed field
/// that does NOT depend on `_changelog`.
fn fields_context_with_plain_computed() -> Arc<FieldsContext> {
    let defs = vec![
        (
            "title",
            "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
        ),
        (
            "body",
            "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
        ),
        (
            "upper_title",
            "id: 00000000000000000000000UPR\nname: upper_title\ntype:\n  kind: computed\n  derive: upper-title\n",
        ),
    ];
    let entities = vec![(
        "task",
        "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - upper_title\n",
    )];
    let dir = TempDir::new().unwrap();
    Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
}

/// Build a ComputeEngine with a "count-changelog" derivation that reads
/// the `_changelog` array and returns its length.
fn compute_engine_with_changelog_counter() -> Arc<swissarmyhammer_fields::ComputeEngine> {
    let mut engine = swissarmyhammer_fields::ComputeEngine::new();
    engine.register(
        "count-changelog",
        Box::new(|fields| {
            let count = fields
                .get("_changelog")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            Box::pin(async move { json!(count) })
        }),
    );
    Arc::new(engine)
}

/// Build a ComputeEngine with an "upper-title" derivation that does
/// not need `_changelog`.
fn compute_engine_with_upper_title() -> Arc<swissarmyhammer_fields::ComputeEngine> {
    let mut engine = swissarmyhammer_fields::ComputeEngine::new();
    engine.register(
        "upper-title",
        Box::new(|fields| {
            let title = fields
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_uppercase();
            Box::pin(async move { serde_json::Value::String(title) })
        }),
    );
    Arc::new(engine)
}

/// Append a legacy entity-format `ChangeEntry` line to the given
/// `.jsonl` path. Used by tests to seed on-disk fixtures that the
/// projecting reader will translate back into field-level diffs.
async fn write_legacy_changelog_line(path: &Path, entry: &ChangeEntry) {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    let mut line = serde_json::to_string(entry).unwrap();
    line.push('\n');
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut file, line.as_bytes())
        .await
        .unwrap();
    // tokio::fs::File buffers writes and does NOT flush on drop, so without
    // an explicit flush a concurrent read can race ahead of the buffered
    // bytes and lose this line under load.
    tokio::io::AsyncWriteExt::flush(&mut file).await.unwrap();
}

#[tokio::test]
async fn changelog_injected_for_changelog_dependent_computed_field() {
    let dir = TempDir::new().unwrap();
    let fields = fields_context_with_changelog_computed();
    let compute = compute_engine_with_changelog_counter();
    let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

    // Write a task (legacy fallback doesn't write changelog entries)
    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Hello"));
    ctx.write(&task).await.unwrap();

    // Manually append changelog entries so read_changelog finds them
    let log_path = ctx.changelog_path("task", "01ABC").unwrap();
    let entry1 = ChangeEntry::new(
        "task",
        "01ABC",
        "create",
        vec![(
            "title".into(),
            FieldChange::Set {
                value: json!("Hello"),
            },
        )],
    );
    let entry2 = ChangeEntry::new(
        "task",
        "01ABC",
        "update",
        vec![(
            "title".into(),
            FieldChange::Changed {
                old_value: json!("Hello"),
                new_value: json!("Updated"),
            },
        )],
    );
    write_legacy_changelog_line(&log_path, &entry1).await;
    write_legacy_changelog_line(&log_path, &entry2).await;

    // Read the entity — derivation should see 2 changelog entries
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    let count = loaded.fields.get("change_count").unwrap().as_u64().unwrap();
    assert_eq!(count, 2, "expected 2 changelog entries, got {}", count);
}

#[tokio::test]
async fn changelog_not_injected_for_non_changelog_computed_field() {
    let dir = TempDir::new().unwrap();
    let fields = fields_context_with_plain_computed();
    let compute = compute_engine_with_upper_title();
    let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("hello"));
    ctx.write(&task).await.unwrap();

    let loaded = ctx.read("task", "01ABC").await.unwrap();
    // The derivation ran successfully without _changelog
    assert_eq!(loaded.get_str("upper_title"), Some("HELLO"));
    // _changelog was never injected, so it should not appear
    assert!(
        !loaded.fields.contains_key("_changelog"),
        "_changelog should not be present in entity fields"
    );
}

#[tokio::test]
async fn changelog_stripped_after_derivation() {
    let dir = TempDir::new().unwrap();
    let fields = fields_context_with_changelog_computed();
    let compute = compute_engine_with_changelog_counter();
    let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Test"));
    ctx.write(&task).await.unwrap();

    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert!(
        !loaded.fields.contains_key("_changelog"),
        "_changelog must be stripped from entity fields after derivation"
    );
    // But the computed field was still derived
    assert!(loaded.fields.contains_key("change_count"));
}

/// Build a FieldsContext whose "task" entity includes a computed field
/// that depends on `_file_created`.
fn fields_context_with_file_created_computed() -> Arc<FieldsContext> {
    let defs = vec![
        (
            "title",
            "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
        ),
        (
            "body",
            "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
        ),
        (
            "file_ts",
            "id: 00000000000000000000000FTS\nname: file_ts\ntype:\n  kind: computed\n  derive: capture-file-created\n  depends_on:\n    - _file_created\n",
        ),
    ];
    let entities = vec![(
        "task",
        "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - file_ts\n",
    )];
    let dir = TempDir::new().unwrap();
    Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
}

/// Build a ComputeEngine with a "capture-file-created" derivation that
/// returns the injected `_file_created` value verbatim.
fn compute_engine_with_file_created_capture() -> Arc<swissarmyhammer_fields::ComputeEngine> {
    let mut engine = swissarmyhammer_fields::ComputeEngine::new();
    engine.register(
        "capture-file-created",
        Box::new(|fields| {
            let v = fields
                .get("_file_created")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Box::pin(async move { v })
        }),
    );
    Arc::new(engine)
}

#[tokio::test]
async fn apply_compute_injects_file_created_when_field_depends_on_it() {
    let dir = TempDir::new().unwrap();
    let fields = fields_context_with_file_created_computed();
    let compute = compute_engine_with_file_created_capture();
    let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

    let mut task = Entity::new("task", "01FILE");
    task.set("title", json!("File ts test"));
    let before = std::time::SystemTime::now();
    ctx.write(&task).await.unwrap();

    let loaded = ctx.read("task", "01FILE").await.unwrap();
    let ts_str = loaded
        .fields
        .get("file_ts")
        .and_then(|v| v.as_str())
        .expect("file_ts should resolve to an RFC 3339 string");

    // Parse the timestamp and verify it falls within ±5 seconds of the
    // write window.
    let parsed = chrono::DateTime::parse_from_rfc3339(ts_str)
        .unwrap_or_else(|e| panic!("file_ts {ts_str:?} should parse as RFC 3339: {e}"));
    let ts_system: std::time::SystemTime = parsed.into();

    let lower = before
        .checked_sub(std::time::Duration::from_secs(5))
        .unwrap();
    let upper = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    assert!(
        ts_system >= lower && ts_system <= upper,
        "file_ts {ts_str} should be within ±5s of write time",
    );
}

#[tokio::test]
async fn apply_compute_strips_file_created_after_derivation() {
    let dir = TempDir::new().unwrap();
    let fields = fields_context_with_file_created_computed();
    let compute = compute_engine_with_file_created_capture();
    let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

    let mut task = Entity::new("task", "01STRIP");
    task.set("title", json!("Strip test"));
    ctx.write(&task).await.unwrap();

    let loaded = ctx.read("task", "01STRIP").await.unwrap();
    assert!(
        !loaded.fields.contains_key("_file_created"),
        "_file_created must be stripped from entity fields after derivation"
    );
    // Capture field was still populated
    assert!(loaded.fields.contains_key("file_ts"));
}

/// Exercises the "entity file missing" branch of the injector
/// (`tokio::fs::metadata(&path).await` fails) to lock in the no-panic /
/// Null-return contract. `read()` cannot reach this branch because it fails
/// earlier when the file is absent; call `apply_compute_with_query`
/// directly with a hand-built entity whose id has no corresponding file.
#[tokio::test]
async fn apply_compute_file_created_null_when_md_missing() {
    let dir = TempDir::new().unwrap();
    let fields = fields_context_with_file_created_computed();
    let compute = compute_engine_with_file_created_capture();
    let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

    let mut entity = Entity::new("task", "01PHANTOM");
    entity.set("title", json!("Phantom"));

    let query_fn = ctx.build_entity_query_fn();
    ctx.apply_compute_with_query("task", &mut entity, &query_fn)
        .await
        .expect("apply_compute_with_query must not error on missing file");

    let captured = entity
        .fields
        .get("file_ts")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    assert!(
        captured.is_null(),
        "file_ts should be Null when the entity file is missing, got {captured:?}"
    );
    assert!(
        !entity.fields.contains_key("_file_created"),
        "_file_created must still be stripped after a Null injection"
    );
}

#[tokio::test]
async fn changelog_empty_array_when_no_changelog_file_exists() {
    let dir = TempDir::new().unwrap();
    let fields = fields_context_with_changelog_computed();
    let compute = compute_engine_with_changelog_counter();
    let ctx = EntityContext::new(dir.path(), fields).with_compute(compute);

    // Write an entity but do NOT write any changelog entries.
    // The JSONL file simply does not exist.
    let mut task = Entity::new("task", "01XYZ");
    task.set("title", json!("Brand new"));
    ctx.write(&task).await.unwrap();

    let loaded = ctx.read("task", "01XYZ").await.unwrap();
    // With no changelog file, _changelog should be injected as []
    // and the derivation should see count == 0.
    let count = loaded.fields.get("change_count").unwrap().as_u64().unwrap();
    assert_eq!(
        count, 0,
        "expected 0 changelog entries for an entity with no changelog file"
    );
}

#[tokio::test]
async fn list_where_predicate_accesses_all_entities() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut t1 = Entity::new("tag", "a");
    t1.set("tag_name", json!("Alpha"));
    let mut t2 = Entity::new("tag", "b");
    t2.set("tag_name", json!("Beta"));
    let mut t3 = Entity::new("tag", "c");
    t3.set("tag_name", json!("Charlie"));

    ctx.write(&t1).await.unwrap();
    ctx.write(&t2).await.unwrap();
    ctx.write(&t3).await.unwrap();

    // Keep only entities when total count > 2 (cross-entity logic)
    let result = ctx
        .list_where(
            "tag",
            |entities| crate::filter::EntityFilterContext::new(entities),
            |_entity, fctx| fctx.entities.len() > 2,
        )
        .await
        .unwrap();

    // All 3 pass because entities.len() == 3 > 2
    assert_eq!(result.len(), 3);
}
