//! Board-level defaults.

use swissarmyhammer_entity::Entity;

/// Get the default columns as entities ready to write.
///
/// Single source of truth for the default column set (todo/doing/done).
/// Used by both `InitBoard` and auto-initialization in the processor.
pub fn default_column_entities() -> Vec<Entity> {
    let columns = [
        ("todo", "To Do", 0),
        ("doing", "Doing", 1),
        ("done", "Done", 2),
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
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0].id, "todo");
        assert_eq!(cols[0].get_str("name"), Some("To Do"));
        assert_eq!(cols[2].id, "done");
    }
}
