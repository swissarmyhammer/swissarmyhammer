//! The `attachments` and `assignees` parameters.
//!
//! These two parameters are siblings of `tags`. These tests hold each shape
//! that they accept, the full list that dispatch refuses when one reference is
//! unknown, and the empty array that clears the list.

use super::*;

// -----------------------------------------------------------------------
// Sibling collection params audited alongside `tags`
// -----------------------------------------------------------------------

/// `attachments` is declared on `UpdateTask` (so the schema advertises it)
/// but dispatch never read it — the same silent-drop defect as `tags`.
#[tokio::test]
async fn dispatch_update_task_attachments_persists() {
    let (temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Attach me").await;

    // The entity layer verifies each attachment source exists, so point at
    // real files.
    let one = temp.path().join("one.png");
    let two = temp.path().join("two.png");
    std::fs::write(&one, b"one").unwrap();
    std::fs::write(&two, b"two").unwrap();
    let paths = vec![
        one.to_string_lossy().to_string(),
        two.to_string_lossy().to_string(),
    ];

    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "attachments": paths,
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    // The entity layer copies each source into `.attachments/` and reads the
    // field back as attachment metadata objects.
    let ectx = ctx.entity_context().await.unwrap();
    let stored = ectx.read("task", &id).await.unwrap();
    let attached = stored
        .get("attachments")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let names: Vec<&str> = attached.iter().filter_map(|a| a["name"].as_str()).collect();
    assert_eq!(
        names,
        vec!["one.png", "two.png"],
        "`attachments` on update must persist, not be dropped"
    );
}

/// Register an actor so a `reference` assignee survives the entity write.
async fn add_one_actor(ctx: &KanbanContext, id: &str) {
    let ops =
        parse_input(json!({"op": "add actor", "id": id, "name": id, "type": "human"})).unwrap();
    execute_operation(ctx, &ops[0]).await.unwrap();
}

/// The stored assignee list for a task.
async fn stored_assignees(ctx: &KanbanContext, id: &str) -> Vec<String> {
    get_task(ctx, id).await["assignees"]
        .as_array()
        .expect("assignees should be an array")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// `get task` hands attachments back as enriched metadata objects. A client
/// that reads a task, edits a field, and sends the object back must not be
/// rejected for a shape this API itself produced.
#[tokio::test]
async fn dispatch_update_task_attachments_accepts_enriched_objects() {
    let (temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Round trip").await;

    let one = temp.path().join("one.png");
    std::fs::write(&one, b"one").unwrap();
    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "attachments": [one.to_string_lossy()],
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    // Feed the enriched form straight back.
    let enriched = get_task(&ctx, &id).await["attachments"].clone();
    assert!(
        enriched[0].is_object(),
        "expected enriched attachment objects, got: {enriched}"
    );
    let ops = parse_input(json!({"op": "update task", "id": id, "attachments": enriched})).unwrap();
    execute_operation(&ctx, &ops[0])
        .await
        .expect("the enriched attachment shape must round-trip");

    let after = get_task(&ctx, &id).await["attachments"].clone();
    let names: Vec<&str> = after
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();
    assert_eq!(names, vec!["one.png"], "the attachment must survive");
}

/// The singular `tag` alias must be as forgiving as `tags`, including its
/// loudness: a shape it cannot read is an error, not a quiet skip.
#[tokio::test]
async fn dispatch_update_task_singular_tag_accepts_an_array_and_rejects_junk() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Alias shapes").await;

    let ops =
        parse_input(json!({"op": "update task", "id": id, "tag": ["bug", "kanban"]})).unwrap();
    execute_operation(&ctx, &ops[0])
        .await
        .expect("the singular alias must take an array too");
    assert_eq!(stored_tags(&ctx, &id).await, vec!["bug", "kanban"]);

    let ops = parse_input(json!({"op": "update task", "id": id, "tag": 42})).unwrap();
    assert!(
        execute_operation(&ctx, &ops[0]).await.is_err(),
        "a malformed singular `tag` must error, not vanish"
    );
}

/// An attachment object the entity layer cannot resolve must be rejected at
/// the door. Passing it through wipes the attachment list and still reports
/// success — the silent drop this whole card exists to kill.
#[tokio::test]
async fn dispatch_update_task_attachments_rejects_unresolvable_objects() {
    let (temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Attachment junk").await;

    let one = temp.path().join("one.png");
    std::fs::write(&one, b"one").unwrap();
    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "attachments": [one.to_string_lossy()],
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    // An object with neither `id` nor `name` resolves to nothing.
    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "attachments": [{"path": "/tmp/one.png"}],
    }))
    .unwrap();
    assert!(
        execute_operation(&ctx, &ops[0]).await.is_err(),
        "an unresolvable attachment object must error"
    );

    let after = get_task(&ctx, &id).await["attachments"].clone();
    let names: Vec<&str> = after
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();
    assert_eq!(
        names,
        vec!["one.png"],
        "the rejected update must leave the attachments alone"
    );
}

/// A read-edit-write client that adds one new file path to the enriched
/// list it just read must not be rejected for mixing the two shapes.
#[tokio::test]
async fn dispatch_update_task_attachments_accepts_a_mixed_list() {
    let (temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Mixed shapes").await;

    let one = temp.path().join("one.png");
    let two = temp.path().join("two.png");
    std::fs::write(&one, b"one").unwrap();
    std::fs::write(&two, b"two").unwrap();

    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "attachments": [one.to_string_lossy()],
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let mut mixed = get_task(&ctx, &id).await["attachments"]
        .as_array()
        .unwrap()
        .clone();
    mixed.push(json!(two.to_string_lossy()));

    let ops = parse_input(json!({"op": "update task", "id": id, "attachments": mixed})).unwrap();
    execute_operation(&ctx, &ops[0])
        .await
        .expect("an enriched object plus a new path must round-trip");

    let after = get_task(&ctx, &id).await["attachments"].clone();
    let names: Vec<&str> = after
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();
    assert_eq!(names, vec!["one.png", "two.png"]);
}

/// A name and that tag's ULID in one list name one tag. That is not an
/// error — the tag applies once.
#[tokio::test]
async fn dispatch_update_task_tags_tolerates_duplicate_refs() {
    let (_temp, ctx) = setup().await;
    let bug_id = add_one_tag(&ctx, "bug").await;
    let id = add_one_task(&ctx, "Duplicate refs").await;

    let ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "tags": ["bug", bug_id, "bug"],
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0])
        .await
        .expect("refs naming one tag must not error");

    assert_eq!(stored_tags(&ctx, &id).await, vec!["bug"]);
}

/// The singular `tag` is the key an agent learns from `tag task`. On
/// add/update it must apply, not vanish.
#[tokio::test]
async fn dispatch_add_task_singular_tag_applies() {
    let (_temp, ctx) = setup().await;

    let ops =
        parse_input(json!({"op": "add task", "title": "Singular key", "tag": "bug"})).unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();

    let id = created["id"].as_str().unwrap();
    assert_eq!(stored_tags(&ctx, id).await, vec!["bug"]);
}

#[tokio::test]
async fn dispatch_update_task_singular_tag_replaces_the_set() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Singular update").await;
    let ops = parse_input(json!({"op": "tag task", "id": id, "tag": "stale"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "update task", "id": id, "tag": "fresh"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(stored_tags(&ctx, &id).await, vec!["fresh"]);
}

/// A tag marker sitting next to punctuation is a tag, so `tags` must be
/// able to replace and clear it.
#[tokio::test]
async fn dispatch_update_task_tags_replaces_markers_next_to_punctuation() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Punctuated",
        "description": "Fix #bug, then ship #login.",
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(stored_tags(&ctx, &id).await, vec!["bug", "login"]);

    let ops = parse_input(json!({"op": "update task", "id": id, "tags": []})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert!(
        stored_tags(&ctx, &id).await.is_empty(),
        "punctuated markers must clear too"
    );
}

/// A description ending in a code fence swallows an inline marker. The tag
/// must still land where `get task` can read it.
#[tokio::test]
async fn dispatch_add_task_tags_apply_to_a_description_ending_in_a_code_fence() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Fenced",
        "description": "Repro:\n```\ncargo test\n```",
        "tags": ["bug"],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();

    let id = created["id"].as_str().unwrap();
    assert_eq!(stored_tags(&ctx, id).await, vec!["bug"]);
}

#[tokio::test]
async fn dispatch_update_task_assignees_single_string_persists() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "zara").await;
    let id = add_one_task(&ctx, "Scalar assignee").await;

    let ops = parse_input(json!({"op": "update task", "id": id, "assignees": "zara"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(
        stored_assignees(&ctx, &id).await,
        vec!["zara"],
        "a scalar `assignees` must apply, not be dropped"
    );
}

#[tokio::test]
async fn dispatch_add_task_assignees_stringified_array_persists() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;
    add_one_actor(&ctx, "bob").await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Stringified assignees",
        "assignees": "[\"alice\",\"bob\"]",
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    assert_eq!(stored_assignees(&ctx, &id).await, vec!["alice", "bob"]);
}

#[tokio::test]
async fn dispatch_update_task_assignees_empty_array_clears() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Unassign via update",
        "assignees": ["alice"],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(
        get_task(&ctx, &id).await["assignees"],
        json!(["alice"]),
        "pre-state: the task starts with one assignee"
    );

    let ops = parse_input(json!({"op": "update task", "id": id, "assignees": []})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert!(
        stored_assignees(&ctx, &id).await.is_empty(),
        "an explicit empty assignees array must clear the list"
    );
}

#[tokio::test]
async fn dispatch_update_task_assignees_malformed_scalar_errors() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Malformed assignees").await;

    let ops = parse_input(json!({"op": "update task", "id": id, "assignees": 42})).unwrap();
    assert!(
        execute_operation(&ctx, &ops[0]).await.is_err(),
        "a non-string, non-array assignees value must error"
    );
}

/// An `assignees` ref naming no actor fails the update, and the stored
/// list is left exactly as it was.
#[tokio::test]
async fn dispatch_update_task_unknown_assignee_errors_and_keeps_list() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;
    let ops = parse_input(json!({
        "op": "add task",
        "title": "Keep alice",
        "assignees": ["alice"],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    let ops =
        parse_input(json!({"op": "update task", "id": id, "assignees": ["nosuchactor"]})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "nosuchactor"),
        "expected ActorNotFound, got: {result:?}"
    );
    assert_eq!(
        stored_assignees(&ctx, &id).await,
        vec!["alice"],
        "a rejected update must not disturb the stored assignees"
    );
}

/// One unknown ref rejects the whole `assignees` list. A partial apply
/// would drop the named actor while acking success.
#[tokio::test]
async fn dispatch_update_task_mixed_assignees_rejects_whole_list() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;
    let id = add_one_task(&ctx, "Mixed assignees").await;

    let ops =
        parse_input(json!({"op": "update task", "id": id, "assignees": ["alice", "nosuchactor"]}))
            .unwrap();
    assert!(
        execute_operation(&ctx, &ops[0]).await.is_err(),
        "an unknown ref anywhere in the list must fail the update"
    );
    assert!(
        stored_assignees(&ctx, &id).await.is_empty(),
        "a rejected update must apply none of the list"
    );
}

/// `add task` with an unknown assignee creates no task. A card that is
/// created and then rejected is worse than either outcome.
#[tokio::test]
async fn dispatch_add_task_unknown_assignee_creates_nothing() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Ghost assignee",
        "assignees": ["nosuchactor"],
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "nosuchactor"),
        "expected ActorNotFound, got: {result:?}"
    );

    let ops = parse_input(json!({"op": "list tasks"})).unwrap();
    let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(
        listed["count"], 0,
        "the rejected create must leave no partial task"
    );
}

/// The `actor` fallback is attribution, not a requested assignment, so an
/// unregistered caller still creates the task. The id is skipped rather
/// than echoed, keeping the ack equal to the stored list.
#[tokio::test]
async fn dispatch_add_task_unregistered_actor_fallback_is_skipped() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Ghost caller",
        "actor": "ghostactor",
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    assert_eq!(
        created["assignees"],
        json!([]),
        "the ack must not echo an assignee the write would prune"
    );
    assert!(
        stored_assignees(&ctx, &id).await.is_empty(),
        "an unregistered fallback actor must not be stored"
    );
}

/// A registered `actor` still auto-assigns, and the assignment reaches
/// storage rather than only the ack.
#[tokio::test]
async fn dispatch_add_task_registered_actor_fallback_persists() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "agent").await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Known caller",
        "actor": "agent",
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    assert_eq!(stored_assignees(&ctx, &id).await, vec!["agent"]);
}

/// The singular `assignee` alias is the same caller input as `assignees`,
/// so an unknown actor is rejected there too.
#[tokio::test]
async fn dispatch_add_task_unknown_singular_assignee_creates_nothing() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Ghost alias",
        "assignee": "nosuchactor",
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "nosuchactor"),
        "expected ActorNotFound, got: {result:?}"
    );

    let ops = parse_input(json!({"op": "list tasks"})).unwrap();
    let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(
        listed["count"], 0,
        "the rejected create must leave no partial task"
    );
}

/// The singular `assignee` key accepts an array, the same shape the
/// plural key accepts. A dropped array reads as "no assignee asked for"
/// and `add task` falls back to the operation actor.
#[tokio::test]
async fn dispatch_add_task_singular_assignee_array_shape_persists() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Array under singular key",
        "assignee": ["alice"],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    assert_eq!(stored_assignees(&ctx, &id).await, vec!["alice"]);
}

/// The singular `assignee` key accepts an array on `update task` too.
/// A dropped array leaves the stored list untouched behind an `ok` ack.
#[tokio::test]
async fn dispatch_update_task_singular_assignee_array_shape_persists() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;
    let id = add_one_task(&ctx, "Array under singular key").await;

    let ops = parse_input(json!({"op": "update task", "id": id, "assignee": ["alice"]})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(stored_assignees(&ctx, &id).await, vec!["alice"]);
}

/// The singular `assignee` key accepts a stringified array, the shape a
/// client sends when the transport gives it no array type hint.
#[tokio::test]
async fn dispatch_add_task_singular_assignee_stringified_array_persists() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;
    add_one_actor(&ctx, "bob").await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Stringified array under singular key",
        "assignee": "[\"alice\",\"bob\"]",
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    assert_eq!(stored_assignees(&ctx, &id).await, vec!["alice", "bob"]);
}

/// Shape tolerance on the singular key does not bypass actor validation:
/// an unknown id inside an array still rejects the create.
#[tokio::test]
async fn dispatch_add_task_unknown_singular_assignee_array_creates_nothing() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Ghost in an array",
        "assignee": ["nosuchactor"],
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "nosuchactor"),
        "expected ActorNotFound, got: {result:?}"
    );

    let ops = parse_input(json!({"op": "list tasks"})).unwrap();
    let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(
        listed["count"], 0,
        "the rejected create must leave no partial task"
    );
}

/// An unknown id inside a stringified array under the singular key
/// rejects the update and leaves the stored list exactly as it was.
#[tokio::test]
async fn dispatch_update_task_unknown_singular_assignee_stringified_array_errors() {
    let (_temp, ctx) = setup().await;
    add_one_actor(&ctx, "alice").await;
    let ops = parse_input(json!({
        "op": "add task",
        "title": "Keep alice through the alias",
        "assignees": ["alice"],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "update task", "id": id, "assignee": "[\"nosuchactor\"]"}))
        .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "nosuchactor"),
        "expected ActorNotFound, got: {result:?}"
    );
    assert_eq!(
        stored_assignees(&ctx, &id).await,
        vec!["alice"],
        "a rejected update must not disturb the stored assignees"
    );
}
