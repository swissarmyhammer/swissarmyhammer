//! Core field and entity types for the fields registry.
//!
//! All types serialize to/from YAML via serde. Field definitions describe
//! named, typed attributes. Entity definitions are templates listing which
//! fields belong to a given entity type.

use serde::{Deserialize, Serialize};
use ulid::Ulid;

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

/// The type of a field — determines what shape the value takes.
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
        entity: String,
        #[serde(default)]
        multiple: bool,
    },
    /// Read-only derived value — no stored triple.
    Computed {
        derive: String,
    },
}

/// How a field value is edited.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Editor {
    Markdown,
    Select,
    MultiSelect,
    Date,
    ColorPalette,
    Number,
    None,
}

/// How a field value is displayed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Display {
    Markdown,
    Badge,
    BadgeList,
    Avatar,
    Date,
    ColorSwatch,
    Number,
    Text,
}

/// How a field sorts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SortKind {
    Alphanumeric,
    OptionOrder,
    Datetime,
    Numeric,
}

/// A field definition — the complete schema for a single named attribute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldDef {
    pub id: Ulid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub type_: FieldType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<Editor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<Display>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate: Option<String>,
}

impl FieldDef {
    /// Infer editor from field type if not explicitly set.
    pub fn effective_editor(&self) -> Editor {
        if let Some(ref e) = self.editor {
            return e.clone();
        }
        match &self.type_ {
            FieldType::Text { .. } => Editor::Markdown,
            FieldType::Markdown { .. } => Editor::Markdown,
            FieldType::Date => Editor::Date,
            FieldType::Number { .. } => Editor::Number,
            FieldType::Color => Editor::ColorPalette,
            FieldType::Select { .. } => Editor::Select,
            FieldType::MultiSelect { .. } => Editor::MultiSelect,
            FieldType::Reference { multiple: true, .. } => Editor::MultiSelect,
            FieldType::Reference {
                multiple: false, ..
            } => Editor::Select,
            FieldType::Computed { .. } => Editor::None,
        }
    }

    /// Infer display from field type if not explicitly set.
    pub fn effective_display(&self) -> Display {
        if let Some(ref d) = self.display {
            return d.clone();
        }
        match &self.type_ {
            FieldType::Text { .. } => Display::Text,
            FieldType::Markdown { .. } => Display::Markdown,
            FieldType::Date => Display::Date,
            FieldType::Number { .. } => Display::Number,
            FieldType::Color => Display::ColorSwatch,
            FieldType::Select { .. } => Display::Badge,
            FieldType::MultiSelect { .. } => Display::BadgeList,
            FieldType::Reference { multiple: true, .. } => Display::BadgeList,
            FieldType::Reference {
                multiple: false, ..
            } => Display::Badge,
            FieldType::Computed { .. } => Display::Text,
        }
    }
}

/// An entity definition — a template declaring which fields belong to an entity type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_field: Option<String>,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_type_text_yaml_round_trip() {
        let ft = FieldType::Text { single_line: true };
        let yaml = serde_yaml::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml::from_str(&yaml).unwrap();
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
        let yaml = serde_yaml::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn field_type_reference_yaml_round_trip() {
        let ft = FieldType::Reference {
            entity: "task".into(),
            multiple: true,
        };
        let yaml = serde_yaml::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn field_type_computed_yaml_round_trip() {
        let ft = FieldType::Computed {
            derive: "parse-body-tags".into(),
        };
        let yaml = serde_yaml::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn field_type_number_yaml_round_trip() {
        let ft = FieldType::Number {
            min: Some(0.0),
            max: Some(100.0),
        };
        let yaml = serde_yaml::to_string(&ft).unwrap();
        let parsed: FieldType = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(ft, parsed);
    }

    #[test]
    fn editor_display_sort_yaml_round_trip() {
        let editor = Editor::ColorPalette;
        let yaml = serde_yaml::to_string(&editor).unwrap();
        let parsed: Editor = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(editor, parsed);

        let display = Display::BadgeList;
        let yaml = serde_yaml::to_string(&display).unwrap();
        let parsed: Display = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(display, parsed);

        let sort = SortKind::OptionOrder;
        let yaml = serde_yaml::to_string(&sort).unwrap();
        let parsed: SortKind = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(sort, parsed);
    }

    #[test]
    fn field_def_yaml_round_trip() {
        let field = FieldDef {
            id: Ulid::new(),
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
            default: Some("Backlog".into()),
            editor: Some(Editor::Select),
            display: Some(Display::Badge),
            sort: Some(SortKind::OptionOrder),
            filter: Some("exact".into()),
            group: Some("value".into()),
            validate: None,
        };
        let yaml = serde_yaml::to_string(&field).unwrap();
        let parsed: FieldDef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(field, parsed);
    }

    #[test]
    fn field_def_type_renames_to_type_in_yaml() {
        let field = FieldDef {
            id: Ulid::new(),
            name: "title".into(),
            description: None,
            type_: FieldType::Markdown { single_line: true },
            default: None,
            editor: Some(Editor::Markdown),
            display: Some(Display::Markdown),
            sort: Some(SortKind::Alphanumeric),
            filter: None,
            group: None,
            validate: None,
        };
        let yaml = serde_yaml::to_string(&field).unwrap();
        assert!(yaml.contains("type:"));
        assert!(!yaml.contains("type_:"));
    }

    #[test]
    fn entity_def_yaml_round_trip() {
        let entity = EntityDef {
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
        };
        let yaml = serde_yaml::to_string(&entity).unwrap();
        let parsed: EntityDef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(entity, parsed);
    }

    #[test]
    fn entity_def_without_body_field() {
        let entity = EntityDef {
            name: "tag".into(),
            body_field: None,
            fields: vec!["tag_name".into(), "color".into(), "description".into()],
        };
        let yaml = serde_yaml::to_string(&entity).unwrap();
        assert!(!yaml.contains("body_field"));
        let parsed: EntityDef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(entity, parsed);
    }

    #[test]
    fn effective_editor_inferred() {
        let field = FieldDef {
            id: Ulid::new(),
            name: "test".into(),
            description: None,
            type_: FieldType::Date,
            default: None,
            editor: None,
            display: None,
            sort: None,
            filter: None,
            group: None,
            validate: None,
        };
        assert_eq!(field.effective_editor(), Editor::Date);
        assert_eq!(field.effective_display(), Display::Date);
    }

    #[test]
    fn effective_editor_explicit_overrides() {
        let field = FieldDef {
            id: Ulid::new(),
            name: "test".into(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: Some(Editor::None),
            display: Some(Display::Badge),
            sort: None,
            filter: None,
            group: None,
            validate: None,
        };
        assert_eq!(field.effective_editor(), Editor::None);
        assert_eq!(field.effective_display(), Display::Badge);
    }

    #[test]
    fn computed_field_infers_no_editor() {
        let field = FieldDef {
            id: Ulid::new(),
            name: "tags".into(),
            description: None,
            type_: FieldType::Computed {
                derive: "parse-body-tags".into(),
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            filter: None,
            group: None,
            validate: None,
        };
        assert_eq!(field.effective_editor(), Editor::None);
        assert_eq!(field.effective_display(), Display::Text);
    }

    #[test]
    fn reference_field_infers_editor_display() {
        let single = FieldDef {
            id: Ulid::new(),
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
            filter: None,
            group: None,
            validate: None,
        };
        assert_eq!(single.effective_editor(), Editor::Select);
        assert_eq!(single.effective_display(), Display::Badge);

        let multi = FieldDef {
            id: Ulid::new(),
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
            filter: None,
            group: None,
            validate: None,
        };
        assert_eq!(multi.effective_editor(), Editor::MultiSelect);
        assert_eq!(multi.effective_display(), Display::BadgeList);
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
filter: exact
group: value
"#;
        let field: FieldDef = serde_yaml::from_str(yaml_input).unwrap();
        assert_eq!(field.name, "status");
        assert_eq!(field.default, Some("Backlog".into()));
        assert_eq!(field.editor, Some(Editor::Select));
        assert_eq!(field.display, Some(Display::Badge));
        assert_eq!(field.sort, Some(SortKind::OptionOrder));

        if let FieldType::Select { ref options } = field.type_ {
            assert_eq!(options.len(), 5);
            assert_eq!(options[0].value, "Backlog");
            assert_eq!(options[4].value, "Done");
        } else {
            panic!("expected Select type");
        }

        // Round-trip
        let yaml_out = serde_yaml::to_string(&field).unwrap();
        let reparsed: FieldDef = serde_yaml::from_str(&yaml_out).unwrap();
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
filter: substring
"#;
        let field: FieldDef = serde_yaml::from_str(yaml_input).unwrap();
        assert_eq!(field.name, "tags");
        assert_eq!(field.editor, Some(Editor::None));
        assert_eq!(field.display, Some(Display::BadgeList));
        if let FieldType::Computed { ref derive } = field.type_ {
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
        let field: FieldDef = serde_yaml::from_str(yaml_input).unwrap();
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
        let field: FieldDef = serde_yaml::from_str(yaml_input).unwrap();
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
filter: substring
"#;
        let field: FieldDef = serde_yaml::from_str(yaml_input).unwrap();
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
        let entity: EntityDef = serde_yaml::from_str(yaml_input).unwrap();
        assert_eq!(entity.name, "task");
        assert_eq!(entity.body_field, Some("body".into()));
        assert_eq!(entity.fields.len(), 8);
        assert!(entity.fields.contains(&"assignees".to_string()));
        assert!(entity.fields.contains(&"depends_on".to_string()));
    }
}
