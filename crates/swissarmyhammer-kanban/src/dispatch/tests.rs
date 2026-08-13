//! Tests for [operation dispatch](super).
//!
//! The tests are in one module for each subject. The review engine puts a
//! whole file into one agent prompt. It does not review a file that is larger
//! than the per-file prompt cap. Thus a test tree of this size must be more
//! than one file.
//!
//! - [`actors_tags`] — the actor and the board-level tag operations.
//! - [`basics`] — the board bootstrap, the task rows that dispatch gives back,
//!   the missing-field error, and the actor that the processor holds.
//! - [`board_columns`] — the board and the column operations.
//! - [`collections`] — the `attachments` and `assignees` parameters.
//! - [`comments`] — the comment operations.
//! - [`dates`] — the `due` and `scheduled` fields.
//! - [`perspectives`] — the perspective operations.
//! - [`short_ids`] — the short-id input forms and the `short_id` output.
//! - [`tags`] — the `tags` parameter on add task and on update task.
//! - [`tasks`] — the task operations and their optional parameters.
//!
//! This module holds what those ten share: the imports, the `setup` fixture,
//! and the helpers that read a task back.

mod actors_tags;
mod basics;
mod board_columns;
mod collections;
mod comments;
mod dates;
mod perspectives;
mod short_ids;
mod tags;
mod tasks;

use super::*;
use crate::parse::parse_input;
use crate::types::Ordinal;
use serde_json::json;
use tempfile::TempDir;

async fn setup() -> (TempDir, KanbanContext) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(kanban_dir);
    // Init a board first
    let ops = parse_input(json!({"op": "init board", "name": "Test"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();
    (temp, ctx)
}

/// Fetch the full task via `get task` — mutation responses are thin
/// acks / slim projections, so effect assertions go through the stored
/// state.
async fn get_task(ctx: &KanbanContext, id: &str) -> serde_json::Value {
    let ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
    execute_operation(ctx, &ops[0]).await.unwrap()
}

/// Add a single task and return its full ULID.
async fn add_one_task(ctx: &KanbanContext, title: &str) -> String {
    let ops = parse_input(json!({"op": "add task", "title": title})).unwrap();
    let r = execute_operation(ctx, &ops[0]).await.unwrap();
    r["id"].as_str().unwrap().to_string()
}

/// The stored tag set for a task, sorted — `get task` derives it from the
/// body, so this asserts effect, never response echo.
async fn stored_tags(ctx: &KanbanContext, id: &str) -> Vec<String> {
    get_task(ctx, id).await["tags"]
        .as_array()
        .expect("tags should be an array")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// Create a tag entity and return its full ULID.
async fn add_one_tag(ctx: &KanbanContext, name: &str) -> String {
    let ops = parse_input(json!({"op": "add tag", "name": name})).unwrap();
    let r = execute_operation(ctx, &ops[0]).await.unwrap();
    r["id"].as_str().unwrap().to_string()
}
