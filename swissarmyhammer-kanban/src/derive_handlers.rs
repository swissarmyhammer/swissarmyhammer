//! Kanban-specific derive handlers for computed fields.
//!
//! Each handler implements the `DeriveHandler` trait from `swissarmyhammer-fields`.

use std::collections::BTreeSet;
use std::collections::HashMap;

use serde_json::Value;
use swissarmyhammer_fields::derive::{DeriveError, DeriveHandler};
use swissarmyhammer_fields::types::EntityDef;

use crate::tag_parser;

/// Handler for `parse-body-tags` — computes tags from `#tag` patterns in the body field.
///
/// **compute**: Reads the `body_field` from entity fields, parses `#tag` patterns,
/// returns a JSON array of tag slugs.
///
/// **apply**: Receives a desired array of tag slugs. Diffs against current tags
/// parsed from the body, then calls `append_tag`/`remove_tag` on the body text
/// for each difference. Writes the updated body back to fields.
pub struct ParseBodyTags;

impl DeriveHandler for ParseBodyTags {
    fn compute(&self, fields: &HashMap<String, Value>, schema: &EntityDef) -> Value {
        let body_field = schema.body_field.as_deref().unwrap_or("body");
        let body = fields
            .get(body_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tags = tag_parser::parse_tags(body);
        Value::Array(tags.into_iter().map(Value::String).collect())
    }

    fn apply(
        &self,
        fields: &mut HashMap<String, Value>,
        schema: &EntityDef,
        desired: &Value,
    ) -> Result<(), DeriveError> {
        let desired_tags: BTreeSet<String> = match desired.as_array() {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(tag_parser::normalize_slug)
                .filter(|s| !s.is_empty())
                .collect(),
            None => {
                return Err(DeriveError::InvalidValue(
                    "expected array of tag strings".into(),
                ));
            }
        };

        let body_field = schema.body_field.as_deref().unwrap_or("body");
        let body = fields
            .get(body_field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let current_tags: BTreeSet<String> = tag_parser::parse_tags(&body).into_iter().collect();

        let mut new_body = body;

        // Remove tags not in desired set
        for tag in current_tags.difference(&desired_tags) {
            new_body = tag_parser::remove_tag(&new_body, tag);
        }

        // Add tags not in current set
        for tag in desired_tags.difference(&current_tags) {
            new_body = tag_parser::append_tag(&new_body, tag);
        }

        fields.insert(body_field.to_string(), Value::String(new_body));

        Ok(())
    }
}

/// Build a `DeriveRegistry` with all kanban-specific handlers registered.
pub fn kanban_derive_registry() -> swissarmyhammer_fields::DeriveRegistry {
    let mut registry = swissarmyhammer_fields::DeriveRegistry::new();
    registry.register("parse-body-tags", Box::new(ParseBodyTags));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_schema() -> EntityDef {
        EntityDef {
            name: "task".into(),
            icon: None,
            body_field: Some("body".into()),
            fields: vec!["title".into(), "body".into(), "tags".into()],
            sections: vec![],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            mention_slug_field: None,
            search_display_field: None,
            commands: vec![],
        }
    }

    #[test]
    fn compute_returns_tags_from_body() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            Value::String("Fix the #bug in #login".to_string()),
        );

        let result = handler.compute(&fields, &schema);
        let tags: Vec<&str> = result
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(tags, vec!["bug", "login"]);
    }

    #[test]
    fn compute_returns_empty_for_no_body() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let fields = HashMap::new();

        let result = handler.compute(&fields, &schema);
        assert_eq!(result, Value::Array(vec![]));
    }

    #[test]
    fn apply_adds_new_tag_to_body() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            Value::String("Fix the #bug".to_string()),
        );

        handler
            .apply(&mut fields, &schema, &serde_json::json!(["bug", "feature"]))
            .unwrap();

        let body = fields["body"].as_str().unwrap();
        let tags = tag_parser::parse_tags(body);
        assert!(tags.contains(&"bug".to_string()));
        assert!(tags.contains(&"feature".to_string()));
    }

    #[test]
    fn apply_removes_tag_from_body() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            Value::String("Fix #bug and #wontfix issue".to_string()),
        );

        handler
            .apply(&mut fields, &schema, &serde_json::json!(["bug"]))
            .unwrap();

        let body = fields["body"].as_str().unwrap();
        let tags = tag_parser::parse_tags(body);
        assert_eq!(tags, vec!["bug"]);
        assert!(!body.contains("#wontfix"));
    }

    #[test]
    fn apply_sets_all_tags_from_empty_body() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();
        // No body field at all
        handler
            .apply(
                &mut fields,
                &schema,
                &serde_json::json!(["new-tag", "another"]),
            )
            .unwrap();

        let body = fields["body"].as_str().unwrap();
        let tags = tag_parser::parse_tags(body);
        assert!(tags.contains(&"new-tag".to_string()));
        assert!(tags.contains(&"another".to_string()));
    }

    #[test]
    fn apply_clears_all_tags() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            Value::String("Has #bug and #feature".to_string()),
        );

        handler
            .apply(&mut fields, &schema, &serde_json::json!([]))
            .unwrap();

        let body = fields["body"].as_str().unwrap();
        let tags = tag_parser::parse_tags(body);
        assert!(tags.is_empty());
    }

    #[test]
    fn apply_normalizes_slugs() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();

        handler
            .apply(&mut fields, &schema, &serde_json::json!(["Bug Fix"]))
            .unwrap();

        let body = fields["body"].as_str().unwrap();
        let tags = tag_parser::parse_tags(body);
        assert_eq!(tags, vec!["Bug_Fix"]);
    }

    #[test]
    fn apply_rejects_non_array() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();

        let result = handler.apply(&mut fields, &schema, &serde_json::json!("not-an-array"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected array"));
    }

    #[test]
    fn apply_no_op_when_tags_unchanged() {
        let handler = ParseBodyTags;
        let schema = task_schema();
        let mut fields = HashMap::new();
        let original = "Fix the #bug in code".to_string();
        fields.insert("body".to_string(), Value::String(original.clone()));

        handler
            .apply(&mut fields, &schema, &serde_json::json!(["bug"]))
            .unwrap();

        // Body should be unchanged (or at least have same tags)
        let body = fields["body"].as_str().unwrap();
        let tags = tag_parser::parse_tags(body);
        assert_eq!(tags, vec!["bug"]);
    }

    #[test]
    fn registry_has_parse_body_tags() {
        let registry = kanban_derive_registry();
        assert!(registry.has("parse-body-tags"));
        assert!(registry.get("parse-body-tags").unwrap().writable());
    }
}
