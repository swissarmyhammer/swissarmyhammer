//! The short-id input forms and the `short_id` output.
//!
//! These tests hold the bare, the caret and the lowercase forms of a short id,
//! the full ULID that stays as it is, the clean not-found error, and the
//! `depends_on` references that dispatch resolves to a full ULID.

use super::*;

// ------------------------------------------------------------------
// Short-id input coercion + output (`short_id` field)
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_get_task_by_bare_short_id() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Short fetch").await;
    let short = crate::types::short_id(&id);

    let ops = parse_input(json!({"op": "get task", "id": short})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"].as_str().unwrap(), id);
    assert_eq!(result["title"], "Short fetch");
}

#[tokio::test]
async fn dispatch_get_task_by_caret_short_id() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Caret fetch").await;
    let caret = format!("^{}", crate::types::short_id(&id));

    let ops = parse_input(json!({"op": "get task", "id": caret})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"].as_str().unwrap(), id);
}

#[tokio::test]
async fn dispatch_get_task_by_short_id_is_case_insensitive() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Upper fetch").await;
    let upper = crate::types::short_id(&id).to_uppercase();

    let ops = parse_input(json!({"op": "get task", "id": upper})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"].as_str().unwrap(), id);
}

#[tokio::test]
async fn dispatch_get_task_by_full_ulid_still_works() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Full fetch").await;

    let ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"].as_str().unwrap(), id);
}

#[tokio::test]
async fn dispatch_move_task_by_short_id() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Short move").await;
    let short = crate::types::short_id(&id);

    let ops = parse_input(json!({"op": "move task", "id": short, "column": "doing"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    // The ack carries the resolved full ULID; the stored position is
    // asserted via `get task`.
    assert_eq!(result["id"].as_str().unwrap(), id);
    assert_eq!(get_task(&ctx, &id).await["position"]["column"], "doing");
}

#[tokio::test]
async fn dispatch_complete_task_by_short_id() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "Short complete").await;
    let short = crate::types::short_id(&id);

    let ops = parse_input(json!({"op": "complete task", "id": short})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"].as_str().unwrap(), id);
}

#[tokio::test]
async fn dispatch_get_task_output_includes_short_id() {
    let (_temp, ctx) = setup().await;
    let id = add_one_task(&ctx, "With short id").await;

    let ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(
        result["short_id"].as_str().unwrap(),
        crate::types::short_id(&id)
    );
}

#[tokio::test]
async fn dispatch_unknown_short_id_returns_clean_not_found() {
    let (_temp, ctx) = setup().await;
    add_one_task(&ctx, "Real task").await;

    // `zzzzzzz` matches no task — must be a clean error, not a panic.
    let ops = parse_input(json!({"op": "get task", "id": "zzzzzzz"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(result.is_err(), "unknown short id must return an error");
}

#[tokio::test]
async fn dispatch_ambiguous_prefix_returns_not_found() {
    let (_temp, ctx) = setup().await;
    // Two tasks both exist; an empty-ish ambiguous prefix that matches more
    // than one task resolves to an error rather than picking one.
    let id1 = add_one_task(&ctx, "Amb one").await;
    let _id2 = add_one_task(&ctx, "Amb two").await;

    // Both ULIDs share a long leading run (minted within the same ms burst);
    // the first two chars `01` are a prefix of every ULID → ambiguous.
    let shared_prefix = &id1[..2];
    let ops = parse_input(json!({"op": "get task", "id": shared_prefix})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        result.is_err(),
        "an ambiguous prefix must return a not-found error, not a match"
    );
}

#[tokio::test]
async fn dispatch_add_task_depends_on_short_id_persists_full_ulid() {
    let (_temp, ctx) = setup().await;
    let dep_id = add_one_task(&ctx, "Dependency").await;
    let dep_short = crate::types::short_id(&dep_id);

    // Create a task whose depends_on is given as a short id.
    let ops = parse_input(json!({
        "op": "add task",
        "title": "Dependent",
        "depends_on": [dep_short],
    }))
    .unwrap();
    let created = execute_operation(&ctx, &ops[0]).await.unwrap();

    // The returned depends_on must carry the full canonical ULID.
    let deps = created["depends_on"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str().unwrap(), dep_id);
}

#[tokio::test]
async fn resolve_task_ref_short_circuits_canonical_full_ulid() {
    // A canonical full 26-char ULID is returned directly by the resolver
    // without consulting the board — proven here by resolving one that is
    // NOT on the board: the old board-scan path returned TaskNotFound, the
    // short-circuit returns the ULID unchanged (existence is then enforced
    // by the underlying command, not the resolver).
    let (_temp, ctx) = setup().await;
    let absent = "01KT6SAXCBZFE6S0DEPZDJSQAA";
    let resolved = resolve_task_ref(&ctx, absent).await.unwrap();
    assert_eq!(resolved, absent);
}

#[tokio::test]
async fn resolve_task_ref_short_circuit_normalizes_case_and_caret() {
    // The short-circuit must yield the canonical uppercase ULID even when
    // the caller passes a lowercase form or a `^`-sigil-prefixed full ULID,
    // matching the casing the board scan would have returned.
    let (_temp, ctx) = setup().await;
    let canonical = "01KT6SAXCBZFE6S0DEPZDJSQAA";
    assert_eq!(
        resolve_task_ref(&ctx, &canonical.to_lowercase())
            .await
            .unwrap(),
        canonical
    );
    assert_eq!(
        resolve_task_ref(&ctx, &format!("^{canonical}"))
            .await
            .unwrap(),
        canonical
    );
}

#[tokio::test]
async fn dispatch_update_task_depends_on_short_id_persists_full_ulid() {
    let (_temp, ctx) = setup().await;
    let dep_id = add_one_task(&ctx, "Dep target").await;
    let task_id = add_one_task(&ctx, "Will depend").await;
    let dep_short = crate::types::short_id(&dep_id);

    let ops = parse_input(json!({
        "op": "update task",
        "id": crate::types::short_id(&task_id),
        "depends_on": [dep_short],
    }))
    .unwrap();
    let updated = execute_operation(&ctx, &ops[0]).await.unwrap();

    // The ack carries the resolved full ULID; the persisted dependency
    // is asserted via `get task`.
    assert_eq!(updated["id"].as_str().unwrap(), task_id);
    let task = get_task(&ctx, &task_id).await;
    let deps = task["depends_on"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str().unwrap(), dep_id);
}

#[tokio::test]
async fn dispatch_update_task_depends_on_single_string_persists() {
    // A bare id string (not wrapped in an array) must persist — the
    // forgiving shape real clients frequently serialize, previously
    // silently dropped by the `.as_array()` gate.
    let (_temp, ctx) = setup().await;
    let dep_id = add_one_task(&ctx, "Dep target").await;
    let task_id = add_one_task(&ctx, "Will depend").await;

    let ops = parse_input(json!({
        "op": "update task",
        "id": task_id,
        "depends_on": dep_id,
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let task = get_task(&ctx, &task_id).await;
    let deps = task["depends_on"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str().unwrap(), dep_id);
}

#[tokio::test]
async fn dispatch_update_task_depends_on_stringified_array_persists() {
    // A stringified JSON array (`"[\"01K…\"]"`) must parse and persist.
    let (_temp, ctx) = setup().await;
    let dep_id = add_one_task(&ctx, "Dep target").await;
    let task_id = add_one_task(&ctx, "Will depend").await;
    let stringified = serde_json::to_string(&vec![dep_id.clone()]).unwrap();

    let ops = parse_input(json!({
        "op": "update task",
        "id": task_id,
        "depends_on": stringified,
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let task = get_task(&ctx, &task_id).await;
    let deps = task["depends_on"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str().unwrap(), dep_id);
}

#[tokio::test]
async fn dispatch_update_task_depends_on_caret_single_string_persists_full_ulid() {
    // A `^`-prefixed single string must resolve to the canonical ULID.
    let (_temp, ctx) = setup().await;
    let dep_id = add_one_task(&ctx, "Dep target").await;
    let task_id = add_one_task(&ctx, "Will depend").await;
    let caret_short = format!("^{}", crate::types::short_id(&dep_id));

    let ops = parse_input(json!({
        "op": "update task",
        "id": task_id,
        "depends_on": caret_short,
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let task = get_task(&ctx, &task_id).await;
    let deps = task["depends_on"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str().unwrap(), dep_id);
}

#[tokio::test]
async fn dispatch_update_task_depends_on_unresolvable_ref_errors() {
    // An unresolvable ref must error, not silently drop to an empty list.
    let (_temp, ctx) = setup().await;
    let task_id = add_one_task(&ctx, "Will depend").await;

    let ops = parse_input(json!({
        "op": "update task",
        "id": task_id,
        "depends_on": "nosuch7",
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        result.is_err(),
        "an unresolvable depends_on ref must error, not silently drop"
    );
}

#[tokio::test]
async fn dispatch_update_task_depends_on_malformed_scalar_errors_without_clearing() {
    // A non-string, non-array value (e.g. a number) is malformed. It must
    // error — never silently clear existing deps, which is exactly the
    // silent-drop anti-pattern this fix exists to kill.
    let (_temp, ctx) = setup().await;
    let dep_id = add_one_task(&ctx, "Dep target").await;
    let task_id = add_one_task(&ctx, "Will depend").await;

    // Seed a real dependency first.
    let ops = parse_input(json!({
        "op": "update task",
        "id": task_id,
        "depends_on": dep_id,
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    // A malformed scalar must error, not wipe the seeded dependency.
    let ops = parse_input(json!({
        "op": "update task",
        "id": task_id,
        "depends_on": 42,
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        result.is_err(),
        "a malformed (non-string, non-array) depends_on must error"
    );

    // The pre-existing dependency must survive the rejected update.
    let task = get_task(&ctx, &task_id).await;
    let deps = task["depends_on"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str().unwrap(), dep_id);
}
