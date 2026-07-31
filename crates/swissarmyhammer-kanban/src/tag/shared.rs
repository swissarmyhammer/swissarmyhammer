//! Helpers shared by the tag operations.

use crate::error::Result;
use serde_json::json;
use swissarmyhammer_entity::EntityContext;

/// Rewrite the tag markers in every task body with one edit function.
///
/// A task's tags are not a stored field — they are `#slug` markers inside the
/// task body — so renaming or deleting a tag must walk every task and rewrite
/// its body. This is the ONE walker for that job: `delete tag` and
/// `update tag` differ only in the [`crate::tag_parser`] call they pass in, so
/// they share this writer and cannot drift apart.
///
/// # Untouched bodies
///
/// **A task the edit does not change comes out byte-identical.** The walk visits
/// every task on the board, so this is the whole safety property: `delete tag`
/// and `update tag` name one tag, and no card that does not carry that tag may
/// be rewritten — not its prose, not its trailing whitespace, not its line
/// endings.
///
/// It holds because `edit_fn` returns untouched text byte for byte. Both
/// [`crate::tag_parser`] writers guarantee that, and state it (see
/// [`crate::tag_parser::remove_tag`]); `remove_tag` once broke it by trimming
/// trailing whitespace off every line of every body it was handed, marker or
/// not. `edit_rewrites_only_the_tasks_carrying_the_marker` holds the contract
/// here, against the stored markdown of a bystander whose body carries trailing
/// whitespace.
///
/// The `new_body != body` guard skips the [`EntityContext::write`] CALL for a
/// task the edit did not change. It is an efficiency guard, not a correctness
/// one: the store handle behind the entity layer already returns early when the
/// serialized text is identical, before the changelog append, the atomic write,
/// and the pending change event, so dropping the guard would still leave every
/// bystander's `.md`, `.jsonl`, mtime, and event stream alone.
///
/// It is not a pure no-op, though — the entity cache invalidates its memoized
/// computes on every `write` call, hash-unchanged or not, so an unguarded walk
/// would evict the compute cache for every task on the board. That is a cost, not
/// a correctness difference, and no test holds it.
///
/// # Parameters
///
/// - `ectx` — the entity context the tasks are read from and written back to.
/// - `edit_fn` — the body rewrite. It takes the current body and returns the
///   new one, normally [`crate::tag_parser::remove_tag`] or
///   [`crate::tag_parser::rename_tag`] bound to the slug being edited.
///
/// # Errors
///
/// Returns the entity-layer error when listing the tasks or writing one back
/// fails. A failure part way through leaves the earlier writes applied — the
/// entity layer offers no transaction across tasks.
pub(crate) async fn apply_tag_edit_to_all_tasks(
    ectx: &EntityContext,
    edit_fn: impl Fn(&str) -> String,
) -> Result<()> {
    for mut task in ectx.list("task").await? {
        let body = task.get_str("body").unwrap_or("").to_string();
        let new_body = edit_fn(&body);
        if new_body != body {
            task.set("body", json!(new_body));
            ectx.write(&task).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::tag::{AddTag, DeleteTag};
    use crate::task::AddTask;
    use std::path::Path;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// The stored markdown for one task, read straight off disk.
    fn stored_markdown(kanban_dir: &Path, task_id: &str) -> String {
        let path = kanban_dir.join("tasks").join(format!("{task_id}.md"));
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("task file missing at {}: {e}", path.display()))
    }

    /// The walk visits EVERY task, so a bystander — a task carrying no marker
    /// for the edited tag — must come out byte-identical while the task that
    /// does carry the marker is rewritten.
    ///
    /// The bystander's body carries trailing whitespace on purpose, and the
    /// assertion compares the whole stored `.md`. That is what makes the fixture
    /// bite: an `edit_fn` that normalizes the WHOLE body rather than only the
    /// lines it edited rewrites this card even though the deleted tag never
    /// appears in it. `remove_tag` did exactly that until card `^tnr56gg`.
    ///
    /// That is all this fixture holds. Its bystander contains no `#` at all, so
    /// it says nothing about where a marker's boundary falls; the boundary rule
    /// is held by the `tag_parser` tests
    /// (`test_remove_tag_next_to_punctuation`,
    /// `test_remove_tag_skips_heading_lines`), and the byte-for-byte identity of
    /// untouched text by `test_writers_leave_text_without_the_slug_byte_identical`.
    #[tokio::test]
    async fn edit_rewrites_only_the_tasks_carrying_the_marker() {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir.clone());
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let tag = AddTag::new("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tag_id = tag["id"].as_str().unwrap().to_string();

        let tagged = AddTask::new("Tagged")
            .with_description("Login broken #bug please fix")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let tagged_id = tagged["id"].as_str().unwrap().to_string();

        let untouched = AddTask::new("Untouched")
            .with_description("No marker here   \nsecond line")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let untouched_id = untouched["id"].as_str().unwrap().to_string();

        let tagged_before = stored_markdown(&kanban_dir, &tagged_id);
        let untouched_before = stored_markdown(&kanban_dir, &untouched_id);

        DeleteTag::new(tag_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(
            stored_markdown(&kanban_dir, &untouched_id),
            untouched_before,
            "a task carrying no marker for the deleted tag must survive byte-identical"
        );
        let tagged_after = stored_markdown(&kanban_dir, &tagged_id);
        assert_ne!(
            tagged_after, tagged_before,
            "the task that did carry the marker must have been rewritten"
        );
        assert!(
            !tagged_after.contains("#bug") && tagged_after.contains("Login broken"),
            "the marker must go and the prose must stay: {tagged_after}"
        );
    }
}
