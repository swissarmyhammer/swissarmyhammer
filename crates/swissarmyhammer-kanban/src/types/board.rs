//! Board-level defaults.

use swissarmyhammer_entity::Entity;

/// Get the default columns as entities ready to write.
///
/// Single source of truth for the default column set
/// (todo/doing/review/done). `review` sits between `doing` and `done` so the
/// review gate of the implement→review→done pipeline has a home, and `done`
/// stays the highest-order (terminal) column — the invariant that completion
/// and progress logic rely on.
///
/// Used by both `InitBoard` and auto-initialization in the processor.
pub fn default_column_entities() -> Vec<Entity> {
    let columns = [
        ("todo", "To Do", 0),
        ("doing", "Doing", 1),
        ("review", "Review", 2),
        ("done", "Done", 3),
    ];
    columns
        .into_iter()
        .map(|(id, name, order)| {
            let mut entity = Entity::new("column", id);
            entity.set("name", serde_json::json!(name));
            entity.set("order", serde_json::json!(order));
            entity
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_column_entities() {
        let cols = default_column_entities();
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0].id, "todo");
        assert_eq!(cols[0].get_str("name"), Some("To Do"));
        assert_eq!(cols[2].id, "review");
        assert_eq!(cols[2].get_str("name"), Some("Review"));
        // `done` stays the highest-order (terminal) column.
        assert_eq!(cols[3].id, "done");
        assert_eq!(cols[3].get_i64("order"), Some(3));
    }
}
