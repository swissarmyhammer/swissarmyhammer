//! The `tags` parameter on add task and on update task.
//!
//! These tests hold each shape that `tags` accepts, the tag entities that
//! dispatch creates, the errors for a reference that it cannot resolve, and
//! the markers that it writes into the description.

use super::*;

// -----------------------------------------------------------------------
// `tags` on add task / update task
// -----------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_task_tags_array_applies() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Tagged at birth",
        "tags": ["bug", "kanban"],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();

    let id = created["id"].as_str().unwrap();
    assert_eq!(stored_tags(&ctx, id).await, vec!["bug", "kanban"]);
}

#[tokio::test]
async fn dispatch_update_task_tags_array_replaces_the_set() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Retag me",
        "description": "body carries #stale",
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(stored_tags(&ctx, &id).await, vec!["stale"]);

    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "tags": ["bug", "init", "mirdan"],
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(
        stored_tags(&ctx, &id).await,
        vec!["bug", "init", "mirdan"],
        "`tags` on update replaces the whole set"
    );
}

/// The equivalence contract: one `add task {tags:[a,b,c]}` and one
/// `add task` followed by three `tag task` calls must land on the same
/// stored tag set. This is what makes the plural form a real alias for
/// the singular op instead of a second, drifting implementation.
#[tokio::test]
async fn dispatch_add_task_tags_equivalent_to_three_tag_task_calls() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Plural",
        "tags": ["bug", "init", "mirdan"],
    }))
    .unwrap();
    let plural = execute_operation(&ctx, &ops[0]).await.unwrap();
    let plural_id = plural["id"].as_str().unwrap().to_string();

    let singular_id = add_one_task(&ctx, "Singular").await;
    for tag in ["bug", "init", "mirdan"] {
        let ops = parse_input(json!({"op": "tag task", "id": singular_id, "tag": tag})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();
    }

    assert_eq!(
        stored_tags(&ctx, &plural_id).await,
        stored_tags(&ctx, &singular_id).await,
        "a tags array must equal one `tag task` per tag"
    );
}

#[tokio::test]
async fn dispatch_add_task_tags_single_string_applies() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add task", "title": "Scalar tag", "tags": "bug"})).unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();

    let id = created["id"].as_str().unwrap();
    assert_eq!(stored_tags(&ctx, id).await, vec!["bug"]);
}

#[tokio::test]
async fn dispatch_update_task_tags_stringified_array_applies() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Stringified").await;

    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "tags": "[\"bug\",\"kanban\"]",
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(stored_tags(&ctx, &id).await, vec!["bug", "kanban"]);
}

/// A tag ref given as the tag entity's full ULID resolves to that tag's
/// name — the exact form that was silently dropped.
#[tokio::test]
async fn dispatch_add_task_tags_full_ulid_resolves_to_tag_name() {
    let (_temp, ctx) = setup().await;
    let bug_id = add_one_tag(&ctx, "bug").await;
    let kanban_id = add_one_tag(&ctx, "kanban").await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "By ulid",
        "tags": [bug_id, kanban_id],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();

    let id = created["id"].as_str().unwrap();
    assert_eq!(stored_tags(&ctx, id).await, vec!["bug", "kanban"]);
}

/// Short id and `^<short>` both resolve, mirroring every other id-taking
/// param on the board.
#[tokio::test]
async fn dispatch_update_task_tags_short_id_and_caret_resolve() {
    let (_temp, ctx) = setup().await;
    let bug_id = add_one_tag(&ctx, "bug").await;
    let kanban_id = add_one_tag(&ctx, "kanban").await;
    let id = add_one_task(&ctx, "By short id").await;

    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "tags": [
            crate::types::short_id(&bug_id),
            format!("^{}", crate::types::short_id(&kanban_id)),
        ],
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(stored_tags(&ctx, &id).await, vec!["bug", "kanban"]);
}

/// An unresolvable tag id ref is an error and creates nothing — the same
/// rule `depends_on` already states.
#[tokio::test]
async fn dispatch_add_task_tags_unresolvable_ulid_errors_and_creates_nothing() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Doomed",
        "tags": ["01KJZEPKJ35S76KF7E9HS5742J"],
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;

    assert!(
        result.is_err(),
        "an unresolvable tag ref must error, not silently drop"
    );

    let ops = parse_input(json!({"op": "list tasks"})).unwrap();
    let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(
        listed["tasks"].as_array().unwrap().len(),
        0,
        "the failed add must not leave a task behind"
    );
}

#[tokio::test]
async fn dispatch_update_task_tags_unresolvable_ulid_errors_without_changing_tags() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Keep my tags",
        "tags": ["keep"],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "tags": ["01KJZEPKJ35S76KF7E9HS5742J"],
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;

    assert!(result.is_err(), "an unresolvable tag ref must error");
    assert_eq!(
        stored_tags(&ctx, &id).await,
        vec!["keep"],
        "a rejected update must leave the tag set untouched"
    );
}

#[tokio::test]
async fn dispatch_update_task_tags_empty_array_clears_the_set() {
    let (_temp, ctx) = setup().await;

    // Seed through the singular op so the pre-state holds regardless of
    // whether the plural form works — the clear is what's under test.
    let id = add_one_task(&ctx, "Clear me").await;
    for tag in ["bug", "kanban"] {
        let ops = parse_input(json!({"op": "tag task", "id": id, "tag": tag})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();
    }
    assert_eq!(stored_tags(&ctx, &id).await, vec!["bug", "kanban"]);

    let ops = parse_input(json!({"op": "update task", "id": id, "tags": []})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert!(
        stored_tags(&ctx, &id).await.is_empty(),
        "an explicit empty tags array replaces the set with nothing"
    );
}

/// A malformed `tags` value (neither string nor array) errors instead of
/// being dropped — on update a silent drop would look like "no change".
#[tokio::test]
async fn dispatch_update_task_tags_malformed_scalar_errors() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Malformed tags").await;

    let ops = parse_input(json!({"op": "update task", "id": id, "tags": 42})).unwrap();
    assert!(
        execute_operation(&ctx, &ops[0]).await.is_err(),
        "a non-string, non-array tags value must error"
    );
}

/// Auto-created tag entities must exist after a plural apply, exactly as
/// `tag task` guarantees — otherwise `list tags` and the UI disagree with
/// the task's own tag list.
#[tokio::test]
async fn dispatch_add_task_tags_auto_creates_tag_entities() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Auto create",
        "tags": ["brand-new"],
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "list tags"})).unwrap();
    let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
    let names: Vec<&str> = listed["tags"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        names.contains(&"brand-new"),
        "plural tags must auto-create the Tag entity, got: {names:?}"
    );
}
