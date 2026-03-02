//! Built-in field definitions and entity templates for kanban.
//!
//! `kanban_defaults()` provides the full set of field definitions and entity
//! templates that `swissarmyhammer-kanban` needs. These are passed to
//! `FieldsContext::open().with_defaults()` to seed the schema on first open
//! and add new built-in fields on upgrade.
//!
//! `KanbanLookup` implements `EntityLookup` for kanban entity stores,
//! enabling reference field validation to prune dangling IDs.

use std::path::PathBuf;

use async_trait::async_trait;
use swissarmyhammer_fields::{
    Display, Editor, EntityDef, EntityLookup, FieldDef, FieldDefaults, FieldType, SelectOption,
    SortKind,
};
use ulid::Ulid;

use crate::context::KanbanContext;

/// Deterministic ULID from a 26-char Crockford Base32 string.
fn ulid(s: &str) -> Ulid {
    Ulid::from_string(s).expect("invalid built-in ULID")
}

/// All built-in kanban field definitions and entity templates.
pub fn kanban_defaults() -> FieldDefaults {
    FieldDefaults::new()
        // =====================================================================
        // Task fields
        // =====================================================================
        .field(FieldDef {
            id: ulid("00000000000000000000000001"),
            name: "title".into(),
            description: Some("Task title".into()),
            type_: FieldType::Markdown { single_line: true },
            default: None,
            editor: Some(Editor::Markdown),
            display: Some(Display::Markdown),
            sort: Some(SortKind::Alphanumeric),
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("00000000000000000000000002"),
            name: "status".into(),
            description: Some("Current workflow state".into()),
            type_: FieldType::Select {
                options: vec![
                    SelectOption {
                        value: "Backlog".into(),
                        label: None,
                        color: Some("gray".into()),
                        icon: None,
                        order: 0,
                    },
                    SelectOption {
                        value: "Todo".into(),
                        label: None,
                        color: Some("blue".into()),
                        icon: None,
                        order: 1,
                    },
                    SelectOption {
                        value: "In Progress".into(),
                        label: None,
                        color: Some("yellow".into()),
                        icon: None,
                        order: 2,
                    },
                    SelectOption {
                        value: "In Review".into(),
                        label: None,
                        color: Some("purple".into()),
                        icon: None,
                        order: 3,
                    },
                    SelectOption {
                        value: "Done".into(),
                        label: None,
                        color: Some("green".into()),
                        icon: None,
                        order: 4,
                    },
                ],
            },
            default: Some("Backlog".into()),
            editor: Some(Editor::Select),
            display: Some(Display::Badge),
            sort: Some(SortKind::OptionOrder),
            filter: Some("exact".into()),
            group: Some("value".into()),
            validate: None,
        })
        .field(FieldDef {
            id: ulid("00000000000000000000000003"),
            name: "priority".into(),
            description: Some("Task priority level".into()),
            type_: FieldType::Select {
                options: vec![
                    SelectOption {
                        value: "P0".into(),
                        label: Some("Critical".into()),
                        color: Some("red".into()),
                        icon: None,
                        order: 0,
                    },
                    SelectOption {
                        value: "P1".into(),
                        label: Some("High".into()),
                        color: Some("orange".into()),
                        icon: None,
                        order: 1,
                    },
                    SelectOption {
                        value: "P2".into(),
                        label: Some("Medium".into()),
                        color: Some("yellow".into()),
                        icon: None,
                        order: 2,
                    },
                    SelectOption {
                        value: "P3".into(),
                        label: Some("Low".into()),
                        color: Some("blue".into()),
                        icon: None,
                        order: 3,
                    },
                ],
            },
            default: None,
            editor: Some(Editor::Select),
            display: Some(Display::Badge),
            sort: Some(SortKind::OptionOrder),
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("00000000000000000000000004"),
            name: "tags".into(),
            description: Some("Tags derived from #tag patterns in body".into()),
            type_: FieldType::Computed {
                derive: "parse-body-tags".into(),
            },
            default: None,
            editor: Some(Editor::None),
            display: Some(Display::BadgeList),
            sort: None,
            filter: Some("substring".into()),
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("00000000000000000000000005"),
            name: "assignees".into(),
            description: Some("Assigned actors".into()),
            type_: FieldType::Reference {
                entity: "actor".into(),
                multiple: true,
            },
            default: None,
            editor: Some(Editor::MultiSelect),
            display: Some(Display::Avatar),
            sort: None,
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("00000000000000000000000006"),
            name: "due".into(),
            description: Some("Due date".into()),
            type_: FieldType::Date,
            default: None,
            editor: Some(Editor::Date),
            display: Some(Display::Date),
            sort: Some(SortKind::Datetime),
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("00000000000000000000000007"),
            name: "depends_on".into(),
            description: Some("Task dependencies".into()),
            type_: FieldType::Reference {
                entity: "task".into(),
                multiple: true,
            },
            default: None,
            editor: Some(Editor::MultiSelect),
            display: Some(Display::BadgeList),
            sort: None,
            filter: Some("substring".into()),
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("00000000000000000000000008"),
            name: "body".into(),
            description: Some("Task body content".into()),
            type_: FieldType::Markdown { single_line: false },
            default: None,
            editor: Some(Editor::Markdown),
            display: Some(Display::Markdown),
            sort: None,
            filter: None,
            group: None,
            validate: None,
        })
        // =====================================================================
        // Tag fields
        // =====================================================================
        .field(FieldDef {
            id: ulid("00000000000000000000000009"),
            name: "tag_name".into(),
            description: Some("Tag identifier".into()),
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: Some(Editor::Markdown),
            display: Some(Display::Text),
            sort: Some(SortKind::Alphanumeric),
            filter: None,
            group: None,
            validate: Some(
                r#"const { value } = ctx;
let v = value.trim().replace(/ +/g, "_").replace(/\0/g, "");
if (v.length === 0) throw new Error("tag_name cannot be empty");
return v;"#
                    .into(),
            ),
        })
        .field(FieldDef {
            id: ulid("0000000000000000000000000A"),
            name: "color".into(),
            description: Some("Display color".into()),
            type_: FieldType::Color,
            default: None,
            editor: Some(Editor::ColorPalette),
            display: Some(Display::ColorSwatch),
            sort: None,
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("0000000000000000000000000B"),
            name: "description".into(),
            description: Some("Short text description".into()),
            type_: FieldType::Markdown { single_line: true },
            default: None,
            editor: Some(Editor::Markdown),
            display: Some(Display::Markdown),
            sort: None,
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("0000000000000000000000000C"),
            name: "usage".into(),
            description: Some("Number of entities using this tag".into()),
            type_: FieldType::Computed {
                derive: "tag-usage-count".into(),
            },
            default: None,
            editor: Some(Editor::None),
            display: Some(Display::Number),
            sort: Some(SortKind::Numeric),
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("0000000000000000000000000D"),
            name: "last_used".into(),
            description: Some("When this tag was last used".into()),
            type_: FieldType::Computed {
                derive: "tag-last-used".into(),
            },
            default: None,
            editor: Some(Editor::None),
            display: Some(Display::Date),
            sort: Some(SortKind::Datetime),
            filter: None,
            group: None,
            validate: None,
        })
        // =====================================================================
        // Shared fields (used by actor, column, swimlane)
        // =====================================================================
        .field(FieldDef {
            id: ulid("0000000000000000000000000E"),
            name: "name".into(),
            description: Some("Display name".into()),
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: Some(Editor::Markdown),
            display: Some(Display::Text),
            sort: Some(SortKind::Alphanumeric),
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("0000000000000000000000000F"),
            name: "order".into(),
            description: Some("Sort position".into()),
            type_: FieldType::Number {
                min: Some(0.0),
                max: None,
            },
            default: None,
            editor: Some(Editor::Number),
            display: Some(Display::Number),
            sort: Some(SortKind::Numeric),
            filter: None,
            group: None,
            validate: None,
        })
        .field(FieldDef {
            id: ulid("0000000000000000000000000G"),
            name: "actor_type".into(),
            description: Some("Whether this actor is human or agent".into()),
            type_: FieldType::Select {
                options: vec![
                    SelectOption {
                        value: "human".into(),
                        label: None,
                        color: None,
                        icon: None,
                        order: 0,
                    },
                    SelectOption {
                        value: "agent".into(),
                        label: None,
                        color: None,
                        icon: None,
                        order: 1,
                    },
                ],
            },
            default: None,
            editor: Some(Editor::Select),
            display: Some(Display::Badge),
            sort: None,
            filter: None,
            group: None,
            validate: None,
        })
        // =====================================================================
        // Entity templates
        // =====================================================================
        .entity(EntityDef {
            name: "task".into(),
            body_field: Some("body".into()),
            fields: vec![
                "title".into(),
                "status".into(),
                "priority".into(),
                "tags".into(),
                "assignees".into(),
                "due".into(),
                "depends_on".into(),
                "body".into(),
            ],
        })
        .entity(EntityDef {
            name: "tag".into(),
            body_field: None,
            fields: vec![
                "tag_name".into(),
                "color".into(),
                "description".into(),
                "usage".into(),
                "last_used".into(),
            ],
        })
        .entity(EntityDef {
            name: "actor".into(),
            body_field: None,
            fields: vec!["name".into(), "actor_type".into()],
        })
        .entity(EntityDef {
            name: "column".into(),
            body_field: None,
            fields: vec!["name".into(), "order".into()],
        })
        .entity(EntityDef {
            name: "swimlane".into(),
            body_field: None,
            fields: vec!["name".into(), "order".into()],
        })
}

/// Entity lookup backed by kanban file storage.
///
/// Reads entities from the `.kanban/` directory structure. Each entity type
/// dispatches to the appropriate subdirectory (tasks/, tags/, actors/, etc.).
pub struct KanbanLookup {
    root: PathBuf,
}

impl KanbanLookup {
    /// Create a lookup for a kanban root directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Create a lookup from a KanbanContext.
    pub fn from_context(ctx: &KanbanContext) -> Self {
        Self {
            root: ctx.root().to_path_buf(),
        }
    }
}

#[async_trait]
impl EntityLookup for KanbanLookup {
    async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value> {
        let ctx = KanbanContext::new(&self.root);
        match entity_type {
            "task" => {
                let task_id = crate::types::TaskId::from_string(id);
                ctx.read_task(&task_id).await.ok().map(|t| {
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "tag" => {
                let tag_id = crate::types::TagId::from_string(id);
                ctx.read_tag(&tag_id).await.ok().map(|t| {
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "actor" => {
                let actor_id = crate::types::ActorId::from_string(id);
                ctx.read_actor(&actor_id).await.ok().map(|a| {
                    let mut v = serde_json::to_value(&a).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "column" => {
                let col_id = crate::types::ColumnId::from_string(id);
                ctx.read_column(&col_id).await.ok().map(|c| {
                    let mut v = serde_json::to_value(&c).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "swimlane" => {
                let sl_id = crate::types::SwimlaneId::from_string(id);
                ctx.read_swimlane(&sl_id).await.ok().map(|s| {
                    let mut v = serde_json::to_value(&s).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            _ => None,
        }
    }

    async fn list(&self, entity_type: &str) -> Vec<serde_json::Value> {
        let ctx = KanbanContext::new(&self.root);
        match entity_type {
            "task" => ctx
                .read_all_tasks()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|t| {
                    let id = t.id.to_string();
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "tag" => ctx
                .read_all_tags()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|t| {
                    let id = t.id.to_string();
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "actor" => ctx
                .read_all_actors()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|a| {
                    let id = a.id().to_string();
                    let mut v = serde_json::to_value(&a).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "column" => ctx
                .read_all_columns()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|c| {
                    let id = c.id.to_string();
                    let mut v = serde_json::to_value(&c).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "swimlane" => ctx
                .read_all_swimlanes()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|s| {
                    let id = s.id.to_string();
                    let mut v = serde_json::to_value(&s).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kanban_defaults_has_all_fields() {
        let defaults = kanban_defaults();
        let fields = defaults.fields();
        let entities = defaults.entities();

        // 8 task + 5 tag + 3 shared = 16 fields
        assert_eq!(fields.len(), 16);

        // 5 entity templates
        assert_eq!(entities.len(), 5);
    }

    #[test]
    fn kanban_defaults_field_names() {
        let defaults = kanban_defaults();
        let names: Vec<&str> = defaults.fields().iter().map(|f| f.name.as_str()).collect();

        // Task fields
        assert!(names.contains(&"title"));
        assert!(names.contains(&"status"));
        assert!(names.contains(&"priority"));
        assert!(names.contains(&"tags"));
        assert!(names.contains(&"assignees"));
        assert!(names.contains(&"due"));
        assert!(names.contains(&"depends_on"));
        assert!(names.contains(&"body"));

        // Tag fields
        assert!(names.contains(&"tag_name"));
        assert!(names.contains(&"color"));
        assert!(names.contains(&"description"));
        assert!(names.contains(&"usage"));
        assert!(names.contains(&"last_used"));

        // Shared fields
        assert!(names.contains(&"name"));
        assert!(names.contains(&"order"));
        assert!(names.contains(&"actor_type"));
    }

    #[test]
    fn kanban_defaults_entity_names() {
        let defaults = kanban_defaults();
        let names: Vec<&str> = defaults.entities().iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"task"));
        assert!(names.contains(&"tag"));
        assert!(names.contains(&"actor"));
        assert!(names.contains(&"column"));
        assert!(names.contains(&"swimlane"));
    }

    #[test]
    fn task_entity_has_body_field() {
        let defaults = kanban_defaults();
        let task = defaults
            .entities()
            .iter()
            .find(|e| e.name == "task")
            .unwrap();
        assert_eq!(task.body_field, Some("body".into()));
    }

    #[test]
    fn tag_entity_has_no_body_field() {
        let defaults = kanban_defaults();
        let tag = defaults
            .entities()
            .iter()
            .find(|e| e.name == "tag")
            .unwrap();
        assert_eq!(tag.body_field, None);
    }

    #[test]
    fn all_ulids_are_unique() {
        let defaults = kanban_defaults();
        let ids: Vec<Ulid> = defaults.fields().iter().map(|f| f.id).collect();
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len(), "duplicate ULIDs in defaults");
    }
}
