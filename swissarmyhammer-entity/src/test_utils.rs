//! Shared test utilities for the entity crate.
//!
//! Provides helpers used by both unit tests (in `context.rs`) and
//! integration tests (in `tests/undo_redo.rs`). This module is
//! `#[doc(hidden)]` — not part of the public API.

use std::sync::Arc;

use swissarmyhammer_fields::FieldsContext;
use tempfile::TempDir;

/// Build a FieldsContext with tag and task entity types for testing.
///
/// Tag: plain YAML entity with tag_name and color fields.
/// Task: frontmatter+body entity with title and body fields.
pub fn test_fields_context() -> Arc<FieldsContext> {
    let defs = vec![
        (
            "tag_name",
            "id: 00000000000000000000000TAG\nname: tag_name\ntype:\n  kind: text\n  single_line: true\n",
        ),
        (
            "color",
            "id: 00000000000000000000000COL\nname: color\ntype:\n  kind: color\n",
        ),
        (
            "title",
            "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
        ),
        (
            "body",
            "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
        ),
    ];
    let entities = vec![
        ("tag", "name: tag\nfields:\n  - tag_name\n  - color\n"),
        (
            "task",
            "name: task\nbody_field: body\nfields:\n  - title\n  - body\n",
        ),
    ];

    let dir = TempDir::new().unwrap();
    Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
}
