//! One changelog record for each change, never two.

use super::*;

// -----------------------------------------------------------------------
// Single-changelog regression guards.
//
// After card 01KQ5FJ0VXEQZVKHZBN49Q5GFS, the entity layer no longer
// dual-writes a legacy entity-format `ChangeEntry` alongside the
// store-format `ChangelogEntry`. The per-entity `.jsonl` file must
// contain only store-format records (no `"changes":[` arrays) when
// a `StoreHandle` is registered.
// -----------------------------------------------------------------------

/// Count `.jsonl` lines that look like the entity-layer dual-writer's
/// `ChangeEntry` shape (`"changes":[…]`). Store-format `ChangelogEntry`
/// records never contain that substring, so this is a precise gate
/// against accidental re-introduction of the dual-write.
fn count_entity_format_lines(path: &Path) -> usize {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .filter(|l| l.contains("\"changes\":["))
        .count()
}

#[tokio::test]
async fn write_does_not_append_to_entity_changelog() {
    let dir = TempDir::new().unwrap();
    let ctx = ctx_with_tag_store(&dir).await;

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    // A store-format record was written by the StoreHandle, but the
    // entity layer must not also append a `"changes":[…]` line.
    let log_path = dir.path().join("tags").join("bug.jsonl");
    assert!(log_path.exists(), "store handle must write a changelog");
    assert_eq!(
        count_entity_format_lines(&log_path),
        0,
        "EntityContext::write must not append entity-format ChangeEntry lines"
    );
}

#[tokio::test]
async fn delete_does_not_append_to_entity_changelog() {
    let dir = TempDir::new().unwrap();
    let ctx = ctx_with_tag_store(&dir).await;

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();
    ctx.delete("tag", "bug").await.unwrap();

    // After delete, the live `.jsonl` is gone. The store layer
    // trashes the changelog file under `{type}s/.trash/` with a
    // versioned filename (`bug.{entry_id}.jsonl`). The trashed copy
    // must not contain any legacy entity-format lines.
    let live_log = dir.path().join("tags").join("bug.jsonl");
    assert!(
        !live_log.exists(),
        "live changelog should be gone after delete"
    );

    let trash_dir = dir.path().join("tags").join(".trash");
    let trashed_logs: Vec<_> = std::fs::read_dir(&trash_dir)
        .expect("trash dir must exist after delete")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .map(|e| e.path())
        .collect();
    assert!(
        !trashed_logs.is_empty(),
        "at least one trashed changelog must exist after delete"
    );
    for log in &trashed_logs {
        assert_eq!(
            count_entity_format_lines(log),
            0,
            "EntityContext::delete must not append entity-format ChangeEntry lines (in {log:?})"
        );
    }
}

#[tokio::test]
async fn changelog_path_correct() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let p = ctx.changelog_path("tag", "bug").unwrap();
    assert_eq!(p, dir.path().join("tags").join("bug.jsonl"));
}
