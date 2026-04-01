//! Core field and entity types for the fields registry.
//!
//! All types serialize to/from YAML via serde. Field definitions describe
//! named, typed attributes. Entity definitions are templates listing which
//! fields belong to a given entity type.

use serde::{Deserialize, Serialize};

use crate::id_types::{EntityTypeName, FieldDefId, FieldName};

/// Serde helper: skip serializing a bool field when it is `false`.
fn is_false(b: &bool) -> bool {
    !b
}

/// A single option in a select or multi-select field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectOption {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub order: i32,
}

/// Default max attachment size: 100 MB (GitHub's limit).
fn default_max_bytes() -> u64 {
    104_857_600
}

/// The type of a field -- determines what shape the value takes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FieldType {
    Text {
        #[serde(default)]
        single_line: bool,
    },
    Markdown {
        #[serde(default)]
        single_line: bool,
    },
    Date,
    Number {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
    },
    Color,
    Select {
        options: Vec<SelectOption>,
    },
    MultiSelect {
        options: Vec<SelectOption>,
    },
    /// Stores entity IDs (ULIDs) pointing to another entity type.
    Reference {
        entity: EntityTypeName,
        #[serde(default)]
        multiple: bool,
    },
    /// File attachment field -- stores references to uploaded files.
    Attachment {
        /// Max file size in bytes. Defaults to GitHub's 100 MB limit.
        #[serde(default = "default_max_bytes")]
        max_bytes: u64,
        /// Whether this field holds multiple attachments.
        #[serde(default)]
        multiple: bool,
    },
    /// Read-only derived value -- no stored triple.
    ///
    /// `depends_on` declares which entity types this aggregate depends on.
    /// When an entity of a listed type changes, the owning entity's computed
    /// field is recomputed and an `entity-field-changed` event is emitted.
    Computed {
        derive: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        depends_on: Vec<String>,
        /// Optional target entity type (e.g. "tag" for parse-body-tags).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity: Option<EntityTypeName>,
        /// When true, commit display names (slugs) instead of entity IDs.
        #[serde(default, skip_serializing_if = "is_false")]
        commit_display_names: bool,
    },
}

// Editor and Display are plain strings — any value is accepted.
// The frontend resolves display/editor names to components via registries.
// No Rust enum needed: adding a new display type is a frontend-only change.

/// How a field sorts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum SortKind {
    Alphanumeric,
    Lexical,
    OptionOrder,
    Datetime,
    Numeric,
}

/// A field definition -- the complete schema for a single named attribute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldDef {
    pub id: FieldDefId,
    pub name: FieldName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub type_: FieldType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Lucide icon name for display in the inspector (e.g. "file-text", "users", "tag").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Inspector layout section: "header", "body", "footer", or "hidden".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate: Option<String>,
}

impl FieldDef {
    /// Infer editor from field type if not explicitly set.
    pub fn effective_editor(&self) -> String {
        if let Some(ref e) = self.editor {
            return e.clone();
        }
        match &self.type_ {
            FieldType::Text { .. } => "markdown",
            FieldType::Markdown { .. } => "markdown",
            FieldType::Date => "date",
            FieldType::Number { .. } => "number",
            FieldType::Color => "color-palette",
            FieldType::Select { .. } => "select",
            FieldType::MultiSelect { .. } => "multi-select",
            FieldType::Reference { multiple: true, .. } => "multi-select",
            FieldType::Reference {
                multiple: false, ..
            } => "select",
            FieldType::Attachment { .. } => "attachment",
            FieldType::Computed { .. } => "none",
        }
        .to_string()
    }

    /// Infer display from field type if not explicitly set.
    pub fn effective_display(&self) -> String {
        if let Some(ref d) = self.display {
            return d.clone();
        }
        match &self.type_ {
            FieldType::Text { .. } => "text",
            FieldType::Markdown { .. } => "markdown",
            FieldType::Date => "date",
            FieldType::Number { .. } => "number",
            FieldType::Color => "color-swatch",
            FieldType::Select { .. } => "badge",
            FieldType::MultiSelect { .. } => "badge-list",
            FieldType::Reference { multiple: true, .. } => "badge-list",
            FieldType::Reference {
                multiple: false, ..
            } => "badge",
            FieldType::Attachment { multiple: true, .. } => "attachment-list",
            FieldType::Attachment {
                multiple: false, ..
            } => "attachment",
            FieldType::Computed { .. } => "text",
        }
        .to_string()
    }

    /// Infer sort kind from field type if not explicitly set.
    pub fn effective_sort(&self) -> SortKind {
        if let Some(ref s) = self.sort {
            return s.clone();
        }
        match &self.type_ {
            FieldType::Date => SortKind::Datetime,
            FieldType::Number { .. } => SortKind::Numeric,
            FieldType::Select { .. } | FieldType::MultiSelect { .. } => SortKind::OptionOrder,
            _ => SortKind::Lexical,
        }
    }
}

/// Keybindings for an entity command, per keymap mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityCommandKeys {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cua: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emacs: Option<String>,
}

/// A command declared in an entity definition.
///
/// Commands are metadata only -- the frontend attaches `execute` implementations
/// at mount time by matching on `id`. The `name` field is a template string
/// that may reference `{{entity.type}}` or `{{entity.<field>}}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityCommand {
    pub id: String,
    pub name: String,
    /// Whether this command appears in context menus. Defaults to false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub context_menu: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<EntityCommandKeys>,
}

/// An entity definition -- a template declaring which fields belong to an entity type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityDef {
    pub name: EntityTypeName,
    /// Lucide icon name for this entity type (e.g. "check-square", "tag", "user").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_field: Option<FieldName>,
    #[serde(default)]
    pub fields: Vec<FieldName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate: Option<String>,
    /// Single-character prefix for mentions in markdown (e.g. "#" for tags, "@" for actors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_prefix: Option<String>,
    /// Which field to display in mentions (e.g. "tag_name", "name").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention_display_field: Option<FieldName>,
    /// Which field to display in search results (e.g. "title", "name").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_display_field: Option<FieldName>,
    /// Commands that can be invoked on instances of this entity type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<EntityCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_type_text_yaml_round_trip() {
        let ft = FieldType::Text { single_line: true };
        let yaml = serde_yaml_ng::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn field_type_select_yaml_round_trip() {
        let ft = FieldType::Select {
            options: vec![
                SelectOption {
                    value: "Backlog".into(),
                    label: None,
                    color: Some("gray".into()),
                    icon: None,
                    order: 0,
                },
                SelectOption {
                    value: "Done".into(),
                    label: None,
                    color: Some("green".into()),
                    icon: None,
                    order: 4,
                },
            ],
        };
        let yaml = serde_yaml_ng::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn field_type_reference_yaml_round_trip() {
        let ft = FieldType::Reference {
            entity: "task".into(),
            multiple: true,
        };
        let yaml = serde_yaml_ng::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn field_type_computed_yaml_round_trip() {
        let ft = FieldType::Computed {
            derive: "parse-body-tags".into(),
            depends_on: vec![],
            entity: None,
            commit_display_names: false,
        };
        let yaml = serde_yaml_ng::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn field_type_number_yaml_round_trip() {
        let ft = FieldType::Number {
            min: Some(0.0),
            max: Some(100.0),
        };
        let yaml = serde_yaml_ng::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn sort_kind_yaml_round_trip() {
        let sort = SortKind::OptionOrder;
        let yaml = serde_yaml_ng::to_string(&sort).unwrap();
        let parsed: SortKind = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(sort, parsed);
    }

    #[test]
    fn unknown_display_type_parses_fine() {
        let yaml_input = r#"
id: 00000000000000000000000001
name: test
type:
  kind: text
  single_line: true
display: some-new-type
"#;
        let field: FieldDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(field.display.as_deref(), Some("some-new-type"));
    }

    #[test]
    fn unknown_editor_type_parses_fine() {
        let yaml_input = r#"
id: 00000000000000000000000001
name: test
type:
  kind: text
  single_line: true
editor: custom-widget
"#;
        let field: FieldDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(field.editor.as_deref(), Some("custom-widget"));
    }

    #[test]
    fn field_def_yaml_round_trip() {
        let field = FieldDef {
            id: FieldDefId::new(),
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
                        value: "Done".into(),
                        label: None,
                        color: Some("green".into()),
                        icon: None,
                        order: 4,
                    },
                ],
            },
            default: Some(serde_json::json!("Backlog")),
            editor: Some("select".into()),
            display: Some("badge".into()),
            sort: Some(SortKind::OptionOrder),
            width: Some(120),
            icon: None,
            section: None,
            validate: None,
        };
        let yaml = serde_yaml_ng::to_string(&field).unwrap();
        let parsed: FieldDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(field, parsed);
    }

    #[test]
    fn field_def_type_renames_to_type_in_yaml() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "title".into(),
            description: None,
            type_: FieldType::Markdown { single_line: true },
            default: None,
            editor: Some("markdown".into()),
            display: Some("markdown".into()),
            sort: Some(SortKind::Alphanumeric),
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        let yaml = serde_yaml_ng::to_string(&field).unwrap();
        assert!(yaml.contains("type:"));
        assert!(!yaml.contains("type_:"));
    }

    #[test]
    fn entity_def_yaml_round_trip() {
        let entity = EntityDef {
            name: "task".into(),
            icon: None,
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
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            search_display_field: None,
            commands: vec![],
        };
        let yaml = serde_yaml_ng::to_string(&entity).unwrap();
        let parsed: EntityDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(entity, parsed);
    }

    #[test]
    fn entity_def_without_body_field() {
        let entity = EntityDef {
            name: "tag".into(),
            icon: None,
            body_field: None,
            fields: vec!["tag_name".into(), "color".into(), "description".into()],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            search_display_field: None,
            commands: vec![],
        };
        let yaml = serde_yaml_ng::to_string(&entity).unwrap();
        assert!(!yaml.contains("body_field"));
        let parsed: EntityDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(entity, parsed);
    }

    #[test]
    fn effective_editor_inferred() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "test".into(),
            description: None,
            type_: FieldType::Date,
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_editor(), "date");
        assert_eq!(field.effective_display(), "date");
    }

    #[test]
    fn effective_editor_explicit_overrides() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "test".into(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: Some("none".into()),
            display: Some("badge".into()),
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_editor(), "none");
        assert_eq!(field.effective_display(), "badge");
    }

    #[test]
    fn computed_field_infers_no_editor() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "tags".into(),
            description: None,
            type_: FieldType::Computed {
                derive: "parse-body-tags".into(),
                depends_on: vec![],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_editor(), "none");
        assert_eq!(field.effective_display(), "text");
    }

    #[test]
    fn reference_field_infers_editor_display() {
        let single = FieldDef {
            id: FieldDefId::new(),
            name: "assignee".into(),
            description: None,
            type_: FieldType::Reference {
                entity: "actor".into(),
                multiple: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(single.effective_editor(), "select");
        assert_eq!(single.effective_display(), "badge");

        let multi = FieldDef {
            id: FieldDefId::new(),
            name: "assignees".into(),
            description: None,
            type_: FieldType::Reference {
                entity: "actor".into(),
                multiple: true,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(multi.effective_editor(), "multi-select");
        assert_eq!(multi.effective_display(), "badge-list");
    }

    // NOTE: text/markdown/color/select/multi_select editor+display inference
    // tests are below, using the field_with_type() helper for conciseness.

    #[test]
    fn number_field_infers_editor_display() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "weight".into(),
            description: None,
            type_: FieldType::Number {
                min: None,
                max: Some(100.0),
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_editor(), "number");
        assert_eq!(field.effective_display(), "number");
    }

    #[test]
    fn built_in_status_field_serializes_correctly() {
        let yaml_input = r#"
id: 00000000000000000000000001
name: status
description: "Current workflow state"
type:
  kind: select
  options:
    - value: Backlog
      color: gray
      order: 0
    - value: Todo
      color: blue
      order: 1
    - value: In Progress
      color: yellow
      order: 2
    - value: In Review
      color: purple
      order: 3
    - value: Done
      color: green
      order: 4
default: Backlog
editor: select
display: badge
sort: option-order
"#;
        let field: FieldDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(field.name, "status");
        assert_eq!(field.default, Some(serde_json::json!("Backlog")));
        assert_eq!(field.editor.as_deref(), Some("select"));
        assert_eq!(field.display.as_deref(), Some("badge"));
        assert_eq!(field.sort, Some(SortKind::OptionOrder));

        if let FieldType::Select { ref options } = field.type_ {
            assert_eq!(options.len(), 5);
            assert_eq!(options[0].value, "Backlog");
            assert_eq!(options[4].value, "Done");
        } else {
            panic!("expected Select type");
        }

        // Round-trip
        let yaml_out = serde_yaml_ng::to_string(&field).unwrap();
        let reparsed: FieldDef = serde_yaml_ng::from_str(&yaml_out).unwrap();
        assert_eq!(field, reparsed);
    }

    #[test]
    fn built_in_tags_computed_field() {
        let yaml_input = r#"
id: 00000000000000000000000002
name: tags
type:
  kind: computed
  derive: parse-body-tags
editor: none
display: badge-list
"#;
        let field: FieldDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(field.name, "tags");
        assert_eq!(field.editor.as_deref(), Some("none"));
        assert_eq!(field.display.as_deref(), Some("badge-list"));
        if let FieldType::Computed { ref derive, .. } = field.type_ {
            assert_eq!(derive, "parse-body-tags");
        } else {
            panic!("expected Computed type");
        }
    }

    #[test]
    fn built_in_assignees_reference_field() {
        let yaml_input = r#"
id: 00000000000000000000000003
name: assignees
type:
  kind: reference
  entity: actor
  multiple: true
editor: multi-select
display: avatar
"#;
        let field: FieldDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(field.name, "assignees");
        if let FieldType::Reference {
            ref entity,
            multiple,
        } = field.type_
        {
            assert_eq!(entity, "actor");
            assert!(multiple);
        } else {
            panic!("expected Reference type");
        }
    }

    #[test]
    fn built_in_tag_name_with_validate() {
        let yaml_input = r#"
id: 00000000000000000000000004
name: tag_name
type:
  kind: text
  single_line: true
editor: markdown
display: text
sort: alphanumeric
validate: |
  const { value } = ctx;
  let v = value.trim().replace(/ +/g, "_").replace(/\0/g, "");
  if (v.length === 0) throw new Error("tag_name cannot be empty");
  return v;
"#;
        let field: FieldDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(field.name, "tag_name");
        assert!(field.validate.is_some());
        assert!(field
            .validate
            .as_ref()
            .unwrap()
            .contains("tag_name cannot be empty"));
    }

    #[test]
    fn built_in_depends_on_reference_field() {
        let yaml_input = r#"
id: 00000000000000000000000005
name: depends_on
type:
  kind: reference
  entity: task
  multiple: true
editor: multi-select
display: badge-list
"#;
        let field: FieldDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(field.name, "depends_on");
        if let FieldType::Reference {
            ref entity,
            multiple,
        } = field.type_
        {
            assert_eq!(entity, "task");
            assert!(multiple);
        } else {
            panic!("expected Reference type");
        }
    }

    #[test]
    fn task_entity_def_from_yaml() {
        let yaml_input = r#"
name: task
body_field: body
fields:
  - title
  - status
  - priority
  - tags
  - assignees
  - due
  - depends_on
  - body
"#;
        let entity: EntityDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(entity.name, "task");
        assert_eq!(entity.body_field, Some("body".into()));
        assert_eq!(entity.fields.len(), 8);
        assert!(entity.fields.contains(&FieldName::from("assignees")));
        assert!(entity.fields.contains(&FieldName::from("depends_on")));
    }

    #[test]
    fn sort_kind_lexical_yaml_round_trip() {
        let sort = SortKind::Lexical;
        let yaml = serde_yaml_ng::to_string(&sort).unwrap();
        assert!(yaml.contains("lexical"));
        let parsed: SortKind = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(sort, parsed);
    }

    #[test]
    fn effective_sort_returns_lexical_when_none() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "test".into(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_sort(), SortKind::Lexical);
    }

    #[test]
    fn effective_sort_returns_explicit_when_some() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "test".into(),
            description: None,
            type_: FieldType::Date,
            default: None,
            editor: None,
            display: None,
            sort: Some(SortKind::Datetime),
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_sort(), SortKind::Datetime);
    }

    #[test]
    fn effective_sort_date_defaults_to_datetime() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "due".into(),
            description: None,
            type_: FieldType::Date,
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_sort(), SortKind::Datetime);
    }

    #[test]
    fn effective_sort_number_defaults_to_numeric() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "priority".into(),
            description: None,
            type_: FieldType::Number {
                min: None,
                max: None,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_sort(), SortKind::Numeric);
    }

    #[test]
    fn effective_sort_select_defaults_to_option_order() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "status".into(),
            description: None,
            type_: FieldType::Select {
                options: vec![SelectOption {
                    value: "A".into(),
                    label: None,
                    color: None,
                    icon: None,
                    order: 0,
                }],
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_sort(), SortKind::OptionOrder);

        // Also verify MultiSelect infers the same.
        let multi = FieldDef {
            id: FieldDefId::new(),
            name: "tags".into(),
            description: None,
            type_: FieldType::MultiSelect {
                options: vec![SelectOption {
                    value: "X".into(),
                    label: None,
                    color: None,
                    icon: None,
                    order: 0,
                }],
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(multi.effective_sort(), SortKind::OptionOrder);
    }

    #[test]
    fn effective_sort_text_defaults_to_lexical() {
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "title".into(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_sort(), SortKind::Lexical);
    }

    #[test]
    fn effective_sort_explicit_overrides_inference() {
        // Date would normally infer Datetime, but explicit Lexical overrides it.
        let field = FieldDef {
            id: FieldDefId::new(),
            name: "created".into(),
            description: None,
            type_: FieldType::Date,
            default: None,
            editor: None,
            display: None,
            sort: Some(SortKind::Lexical),
            width: None,
            icon: None,
            section: None,
            validate: None,
        };
        assert_eq!(field.effective_sort(), SortKind::Lexical);
    }

    #[test]
    fn entity_def_with_commands_yaml_round_trip() {
        let entity = EntityDef {
            name: "task".into(),
            icon: None,
            body_field: Some("body".into()),
            fields: vec!["title".into(), "tags".into()],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            search_display_field: None,
            commands: vec![
                EntityCommand {
                    id: "entity.inspect".into(),
                    name: "Inspect {{entity.type}}".into(),
                    context_menu: true,
                    keys: None,
                },
                EntityCommand {
                    id: "entity.archive".into(),
                    name: "Archive {{entity.type}}".into(),
                    context_menu: true,
                    keys: Some(EntityCommandKeys {
                        vim: Some("da".into()),
                        cua: None,
                        emacs: None,
                    }),
                },
            ],
        };
        let yaml = serde_yaml_ng::to_string(&entity).unwrap();
        let parsed: EntityDef = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(entity, parsed);
        // verify commands survive round-trip
        assert_eq!(parsed.commands.len(), 2);
        assert_eq!(parsed.commands[0].id, "entity.inspect");
        assert!(parsed.commands[0].context_menu);
        assert_eq!(parsed.commands[1].id, "entity.archive");
        assert!(parsed.commands[1].keys.is_some());
    }

    #[test]
    fn task_entity_def_from_yaml_with_commands() {
        let yaml_input = r#"
name: task
body_field: body
commands:
  - id: entity.inspect
    name: "Inspect {{entity.type}}"
    context_menu: true
  - id: entity.archive
    name: "Archive {{entity.type}}"
    context_menu: true
fields:
  - title
  - status
  - priority
  - tags
  - assignees
  - due
  - depends_on
  - body
"#;
        let entity: EntityDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(entity.name, "task");
        assert_eq!(entity.body_field, Some("body".into()));
        assert_eq!(entity.fields.len(), 8);
        assert_eq!(entity.commands.len(), 2);
        assert_eq!(entity.commands[0].id, "entity.inspect");
        assert!(entity.commands[0].context_menu);
        assert_eq!(entity.commands[1].id, "entity.archive");
        assert!(entity.commands[1].context_menu);
    }

    #[test]
    fn entity_def_without_commands_still_deserializes() {
        // Backwards compat: existing YAML without a commands field should
        // deserialize fine and produce an empty commands vec.
        let yaml_input = r#"
name: tag
fields:
  - tag_name
  - color
"#;
        let entity: EntityDef = serde_yaml_ng::from_str(yaml_input).unwrap();
        assert_eq!(entity.name, "tag");
        assert!(entity.commands.is_empty());
    }

    // ── helpers for effective_* tests ─────────────────────────────────

    /// Build a minimal FieldDef with the given type and no explicit overrides.
    fn make_field(type_: FieldType) -> FieldDef {
        FieldDef {
            id: FieldDefId::new(),
            name: "test".into(),
            description: None,
            type_,
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        }
    }

    // ── is_false (serde helper) ───────────────────────────────────────

    #[test]
    fn is_false_returns_true_for_false_input() {
        assert!(is_false(&false));
    }

    #[test]
    fn is_false_returns_false_for_true_input() {
        assert!(!is_false(&true));
    }

    // ── effective_editor: explicit override ───────────────────────────

    #[test]
    fn effective_editor_uses_explicit_when_set() {
        let mut f = make_field(FieldType::Text { single_line: false });
        f.editor = Some("custom-editor".into());
        assert_eq!(f.effective_editor(), "custom-editor");
    }

    // ── effective_editor: Computed type (not covered by existing tests)

    #[test]
    fn effective_editor_computed_defaults_to_none() {
        let f = make_field(FieldType::Computed {
            derive: "count".into(),
            depends_on: vec![],
            entity: None,
            commit_display_names: false,
        });
        assert_eq!(f.effective_editor(), "none");
    }

    // ── effective_display: explicit override ──────────────────────────

    #[test]
    fn effective_display_uses_explicit_when_set() {
        let mut f = make_field(FieldType::Date);
        f.display = Some("custom-display".into());
        assert_eq!(f.effective_display(), "custom-display");
    }

    // ── effective_display: Computed type (not covered by existing tests)

    #[test]
    fn effective_display_computed_defaults_to_text() {
        let f = make_field(FieldType::Computed {
            derive: "count".into(),
            depends_on: vec![],
            entity: None,
            commit_display_names: false,
        });
        assert_eq!(f.effective_display(), "text");
    }

    // ── effective_sort: types not covered by existing tests ───────────

    #[test]
    fn effective_sort_markdown_defaults_to_lexical() {
        let f = make_field(FieldType::Markdown { single_line: false });
        assert_eq!(f.effective_sort(), SortKind::Lexical);
    }

    #[test]
    fn effective_sort_color_defaults_to_lexical() {
        let f = make_field(FieldType::Color);
        assert_eq!(f.effective_sort(), SortKind::Lexical);
    }

    #[test]
    fn effective_sort_reference_defaults_to_lexical() {
        let f = make_field(FieldType::Reference {
            entity: "task".into(),
            multiple: false,
        });
        assert_eq!(f.effective_sort(), SortKind::Lexical);
    }

    #[test]
    fn effective_sort_computed_defaults_to_lexical() {
        let f = make_field(FieldType::Computed {
            derive: "count".into(),
            depends_on: vec![],
            entity: None,
            commit_display_names: false,
        });
        assert_eq!(f.effective_sort(), SortKind::Lexical);
    }

    #[test]
    fn field_type_attachment_yaml_round_trip() {
        let ft = FieldType::Attachment {
            max_bytes: 104_857_600,
            multiple: true,
        };
        let yaml = serde_yaml_ng::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);

        // Also round-trip with multiple=false
        let ft_single = FieldType::Attachment {
            max_bytes: 52_428_800,
            multiple: false,
        };
        let yaml2 = serde_yaml_ng::to_string(&ft_single).unwrap();
        let parsed2: FieldType = serde_yaml_ng::from_str(&yaml2).unwrap();
        assert_eq!(ft_single, parsed2);
    }

    #[test]
    fn attachment_max_bytes_defaults_to_100mb() {
        let yaml_input = "kind: attachment\n";
        let parsed: FieldType = serde_yaml_ng::from_str(yaml_input).unwrap();
        if let FieldType::Attachment {
            max_bytes,
            multiple,
        } = parsed
        {
            assert_eq!(max_bytes, 104_857_600);
            assert!(!multiple);
        } else {
            panic!("expected Attachment type");
        }
    }

    #[test]
    fn attachment_field_infers_editor_display() {
        let single = make_field(FieldType::Attachment {
            max_bytes: 104_857_600,
            multiple: false,
        });
        assert_eq!(single.effective_editor(), "attachment");
        assert_eq!(single.effective_display(), "attachment");
        assert_eq!(single.effective_sort(), SortKind::Lexical);

        let multi = make_field(FieldType::Attachment {
            max_bytes: 104_857_600,
            multiple: true,
        });
        assert_eq!(multi.effective_editor(), "attachment");
        assert_eq!(multi.effective_display(), "attachment-list");
        assert_eq!(multi.effective_sort(), SortKind::Lexical);
    }
}
