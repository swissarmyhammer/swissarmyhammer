//! Perspective data types and YAML serialization.
//!
//! A perspective is a named, ordered list of fields with per-field overrides,
//! plus optional filter and group functions (stored as opaque strings).
//! Perspectives reference fields by ULID and can override display properties
//! from the base field definition.

use serde::{Deserialize, Serialize};

/// Sort direction for a sort entry.
///
/// Serializes as lowercase "asc" / "desc" to match the YAML spec format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    Desc,
}

/// A single sort entry specifying which field to sort by and in which direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SortEntry {
    /// Field ULID to sort by.
    pub field: String,
    /// Sort direction (asc or desc).
    pub direction: SortDirection,
}

/// A field entry within a perspective, referencing a field by ULID with
/// optional display overrides.
///
/// All override fields are optional -- when absent, the base field definition's
/// values are used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PerspectiveFieldEntry {
    /// Field ULID -- survives field renames.
    pub field: String,
    /// Override column header (default: field name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Override column width in pixels (default: field definition width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Override editor type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// Override display type (e.g. "text", "badge").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// Override sort comparator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_comparator: Option<String>,
}

/// A perspective -- a named, saved view configuration.
///
/// Stores an ordered list of fields (column order), optional filter/group
/// functions as opaque JS strings, and sort entries. The backend stores
/// filter/group strings verbatim; it never evaluates them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Perspective {
    /// Unique identifier (ULID).
    pub id: String,
    /// Human-readable name (e.g. "Active Sprint").
    pub name: String,
    /// View type (e.g. "board", "grid").
    pub view: String,
    /// Ordered list of field entries (defines column order).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<PerspectiveFieldEntry>,
    /// Opaque filter function string (JS expression). Stored, not evaluated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Group-by field name. Stored as a plain field name string, consumed by the UI as a TanStack Table grouping column ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Sort entries, applied in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<SortEntry>,
}

impl SortEntry {
    /// Create a new sort entry for the given field and direction.
    pub fn new(field: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            field: field.into(),
            direction,
        }
    }
}

impl PerspectiveFieldEntry {
    /// Create a new field entry with only the required field ULID.
    ///
    /// All override fields default to `None`.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            caption: None,
            width: None,
            editor: None,
            display: None,
            sort_comparator: None,
        }
    }

    /// Set the caption override.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Set the width override.
    pub fn with_width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the editor override.
    pub fn with_editor(mut self, editor: impl Into<String>) -> Self {
        self.editor = Some(editor.into());
        self
    }

    /// Set the display override.
    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = Some(display.into());
        self
    }

    /// Set the sort comparator override.
    pub fn with_sort_comparator(mut self, sort_comparator: impl Into<String>) -> Self {
        self.sort_comparator = Some(sort_comparator.into());
        self
    }
}

impl Perspective {
    /// Create a new perspective with the required fields.
    ///
    /// Optional fields (fields list, filter, group, sort) default to empty/None.
    pub fn new(id: impl Into<String>, name: impl Into<String>, view: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            view: view.into(),
            fields: Vec::new(),
            filter: None,
            group: None,
            sort: Vec::new(),
        }
    }

    /// Set the fields list.
    pub fn with_fields(mut self, fields: Vec<PerspectiveFieldEntry>) -> Self {
        self.fields = fields;
        self
    }

    /// Set the filter expression.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Set the group-by field.
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Set the sort entries.
    pub fn with_sort(mut self, sort: Vec<SortEntry>) -> Self {
        self.sort = sort;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perspective_yaml_round_trip() {
        let perspective = Perspective {
            id: "01JPERSP000000000000000000".to_string(),
            name: "Active Sprint".to_string(),
            view: "board".to_string(),
            fields: vec![
                PerspectiveFieldEntry {
                    field: "01JMTASK0000000000TITLE00".to_string(),
                    caption: None,
                    width: None,
                    editor: None,
                    display: None,
                    sort_comparator: None,
                },
                PerspectiveFieldEntry {
                    field: "01JMTASK0000000000STATUS0".to_string(),
                    caption: None,
                    width: Some(150),
                    editor: None,
                    display: None,
                    sort_comparator: None,
                },
                PerspectiveFieldEntry {
                    field: "01JMTASK0000000000PRIORTY".to_string(),
                    caption: Some("P".to_string()),
                    width: Some(60),
                    editor: None,
                    display: None,
                    sort_comparator: None,
                },
                PerspectiveFieldEntry {
                    field: "01HQ3USERFIELD00000SPRINT".to_string(),
                    caption: None,
                    width: None,
                    editor: None,
                    display: Some("text".to_string()),
                    sort_comparator: None,
                },
            ],
            filter: Some(
                "(entity) => entity.Status !== \"Done\" && entity.Sprint === \"Sprint 23\""
                    .to_string(),
            ),
            group: Some("(entity) => entity.Status".to_string()),
            sort: vec![
                SortEntry {
                    field: "01JMTASK0000000000PRIORTY".to_string(),
                    direction: SortDirection::Asc,
                },
                SortEntry {
                    field: "01JMTASK0000000000DUEDAT0".to_string(),
                    direction: SortDirection::Asc,
                },
            ],
        };

        let yaml = serde_yaml_ng::to_string(&perspective).unwrap();
        let parsed: Perspective = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(perspective, parsed);

        // Verify key field names appear in the YAML output
        assert!(yaml.contains("name: Active Sprint"));
        assert!(yaml.contains("view: board"));
        assert!(yaml.contains("direction: asc"));
    }

    #[test]
    fn perspective_minimal_round_trip() {
        let perspective = Perspective {
            id: "01JPERSP000000000000000001".to_string(),
            name: "Default".to_string(),
            view: "grid".to_string(),
            fields: vec![],
            filter: None,
            group: None,
            sort: vec![],
        };

        let yaml = serde_yaml_ng::to_string(&perspective).unwrap();
        let parsed: Perspective = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(perspective, parsed);

        // Minimal perspective should not contain optional sections
        assert!(!yaml.contains("fields:"));
        assert!(!yaml.contains("filter:"));
        assert!(!yaml.contains("group:"));
        assert!(!yaml.contains("sort:"));
    }

    #[test]
    fn field_entry_all_overrides() {
        let entry = PerspectiveFieldEntry {
            field: "01JMTASK0000000000PRIORTY".to_string(),
            caption: Some("Priority".to_string()),
            width: Some(80),
            editor: Some("dropdown".to_string()),
            display: Some("badge".to_string()),
            sort_comparator: Some("numeric".to_string()),
        };

        let yaml = serde_yaml_ng::to_string(&entry).unwrap();
        let parsed: PerspectiveFieldEntry = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(entry, parsed);

        // All overrides should be present
        assert!(yaml.contains("caption: Priority"));
        assert!(yaml.contains("width: 80"));
        assert!(yaml.contains("editor: dropdown"));
        assert!(yaml.contains("display: badge"));
        assert!(yaml.contains("sort_comparator: numeric"));
    }

    #[test]
    fn field_entry_minimal() {
        let entry = PerspectiveFieldEntry {
            field: "01JMTASK0000000000TITLE00".to_string(),
            caption: None,
            width: None,
            editor: None,
            display: None,
            sort_comparator: None,
        };

        let yaml = serde_yaml_ng::to_string(&entry).unwrap();
        let parsed: PerspectiveFieldEntry = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(entry, parsed);

        // Minimal entry should only have the field ULID
        assert!(yaml.contains("field: 01JMTASK0000000000TITLE00"));
        assert!(!yaml.contains("caption:"));
        assert!(!yaml.contains("width:"));
        assert!(!yaml.contains("editor:"));
        assert!(!yaml.contains("display:"));
        assert!(!yaml.contains("sort_comparator:"));
    }

    #[test]
    fn sort_direction_serde() {
        // Asc serializes as "asc"
        let asc_yaml = serde_yaml_ng::to_string(&SortDirection::Asc).unwrap();
        assert!(asc_yaml.trim() == "asc", "got: {}", asc_yaml.trim());

        let parsed: SortDirection = serde_yaml_ng::from_str("asc").unwrap();
        assert_eq!(parsed, SortDirection::Asc);

        // Desc serializes as "desc"
        let desc_yaml = serde_yaml_ng::to_string(&SortDirection::Desc).unwrap();
        assert!(desc_yaml.trim() == "desc", "got: {}", desc_yaml.trim());

        let parsed: SortDirection = serde_yaml_ng::from_str("desc").unwrap();
        assert_eq!(parsed, SortDirection::Desc);
    }

    #[test]
    fn filter_group_as_strings() {
        let perspective = Perspective {
            id: "01JPERSP000000000000000002".to_string(),
            name: "Filtered".to_string(),
            view: "list".to_string(),
            fields: vec![],
            filter: Some("(entity) => entity.Status !== \"Done\"".to_string()),
            group: Some("(entity) => entity.Assignee".to_string()),
            sort: vec![],
        };

        let yaml = serde_yaml_ng::to_string(&perspective).unwrap();
        let parsed: Perspective = serde_yaml_ng::from_str(&yaml).unwrap();

        // JS function strings round-trip intact
        assert_eq!(parsed.filter, perspective.filter);
        assert_eq!(parsed.group, perspective.group);
    }
}
