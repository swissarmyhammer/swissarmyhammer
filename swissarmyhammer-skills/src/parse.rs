//! Input parsing for skill operations
//!
//! Routes JSON input `{verb, noun, ...}` or `{op: "verb noun", ...}` to the correct operation.
//! Follows the same forgiving input pattern as swissarmyhammer-kanban.

use crate::error::SkillError;
use crate::operations::{ListSkills, SearchSkill, UseSkill};
use serde_json::{Map, Value};

/// A parsed skill operation ready for execution
pub enum SkillOperation {
    List(ListSkills),
    Use(UseSkill),
    Search(SearchSkill),
}

/// Parse input JSON into a skill operation
pub fn parse_input(input: Value) -> std::result::Result<SkillOperation, SkillError> {
    let obj = match input {
        Value::Object(obj) => obj,
        _ => {
            return Err(SkillError::Parse {
                message: "input must be a JSON object".to_string(),
            })
        }
    };

    // Extract verb and noun
    let (verb, _noun) = extract_verb_noun(&obj)?;

    match verb.as_str() {
        "list" | "ls" | "show" | "available" => Ok(SkillOperation::List(ListSkills::new())),
        "use" | "get" | "load" | "activate" | "invoke" => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SkillError::Parse {
                    message: "missing required field: name".to_string(),
                })?;
            Ok(SkillOperation::Use(UseSkill::new(name)))
        }
        "search" | "find" | "lookup" => {
            let query = obj
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SkillError::Parse {
                    message: "missing required field: query".to_string(),
                })?;
            Ok(SkillOperation::Search(SearchSkill::new(query)))
        }
        _ => Err(SkillError::Parse {
            message: format!("unsupported skill operation: {}", verb),
        }),
    }
}

/// Extract verb and noun from input using multiple strategies
fn extract_verb_noun(obj: &Map<String, Value>) -> std::result::Result<(String, String), SkillError> {
    // Strategy 1: "op" field with "verb noun" string
    if let Some(op_str) = obj.get("op").and_then(|v| v.as_str()) {
        let parts: Vec<&str> = op_str.split_whitespace().collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
        if parts.len() == 1 {
            // Single word — treat as verb with default noun "skill"
            return Ok((parts[0].to_string(), "skill".to_string()));
        }
    }

    // Strategy 2: Separate verb/noun fields
    if let Some(verb) = obj.get("verb").and_then(|v| v.as_str()) {
        let noun = obj
            .get("noun")
            .and_then(|v| v.as_str())
            .unwrap_or("skill");
        return Ok((verb.to_string(), noun.to_string()));
    }

    // Strategy 3: Shorthand — presence of "name" implies "use skill"
    if obj.contains_key("name") && !obj.contains_key("op") && !obj.contains_key("verb") {
        return Ok(("use".to_string(), "skill".to_string()));
    }

    // Strategy 4: Shorthand — presence of "query" implies "search skill"
    if obj.contains_key("query") && !obj.contains_key("op") && !obj.contains_key("verb") {
        return Ok(("search".to_string(), "skill".to_string()));
    }

    // Strategy 5: Empty object or no verb → "list skill"
    if obj.is_empty()
        || (!obj.contains_key("name")
            && !obj.contains_key("query")
            && !obj.contains_key("op")
            && !obj.contains_key("verb"))
    {
        return Ok(("list".to_string(), "skill".to_string()));
    }

    Err(SkillError::Parse {
        message: "cannot determine skill operation from input".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_list_op() {
        let input = json!({"op": "list skill"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::List(_)));
    }

    #[test]
    fn test_parse_use_op() {
        let input = json!({"op": "use skill", "name": "plan"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Use(_)));
    }

    #[test]
    fn test_parse_get_backward_compat() {
        let input = json!({"op": "get skill", "name": "plan"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Use(_)));
    }

    #[test]
    fn test_parse_search_op() {
        let input = json!({"op": "search skill", "query": "commit"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Search(_)));
    }

    #[test]
    fn test_parse_find_alias() {
        let input = json!({"op": "find skill", "query": "test"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Search(_)));
    }

    #[test]
    fn test_parse_verb_noun() {
        let input = json!({"verb": "list", "noun": "skill"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::List(_)));
    }

    #[test]
    fn test_parse_shorthand_use() {
        let input = json!({"name": "plan"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Use(_)));
    }

    #[test]
    fn test_parse_shorthand_search() {
        let input = json!({"query": "commit"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Search(_)));
    }

    #[test]
    fn test_parse_empty_is_list() {
        let input = json!({});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::List(_)));
    }

    #[test]
    fn test_parse_load_alias() {
        let input = json!({"op": "load skill", "name": "plan"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Use(_)));
    }

    #[test]
    fn test_parse_activate_alias() {
        let input = json!({"op": "activate skill", "name": "plan"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, SkillOperation::Use(_)));
    }
}
