//! Input parsing for agent operations
//!
//! Routes JSON input to the correct operation using the same forgiving pattern as skills.

use crate::error::AgentError;
use crate::operations::{ListAgents, SearchAgent, UseAgent};
use serde_json::{Map, Value};

/// A parsed agent operation ready for execution
pub enum AgentOperation {
    List(ListAgents),
    Use(UseAgent),
    Search(SearchAgent),
}

/// Parse input JSON into an agent operation
pub fn parse_input(input: Value) -> std::result::Result<AgentOperation, AgentError> {
    let obj = match input {
        Value::Object(obj) => obj,
        _ => {
            return Err(AgentError::Parse {
                message: "input must be a JSON object".to_string(),
            })
        }
    };

    let (verb, _noun) = extract_verb_noun(&obj)?;

    match verb.as_str() {
        "list" | "ls" | "show" | "available" => Ok(AgentOperation::List(ListAgents::new())),
        "use" | "get" | "load" | "activate" | "invoke" => {
            let name =
                obj.get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::Parse {
                        message: "missing required field: name".to_string(),
                    })?;
            Ok(AgentOperation::Use(UseAgent::new(name)))
        }
        "search" | "find" | "lookup" => {
            let query =
                obj.get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::Parse {
                        message: "missing required field: query".to_string(),
                    })?;
            Ok(AgentOperation::Search(SearchAgent::new(query)))
        }
        _ => Err(AgentError::Parse {
            message: format!("unsupported agent operation: {}", verb),
        }),
    }
}

/// Extract verb and noun from input using multiple strategies
fn extract_verb_noun(
    obj: &Map<String, Value>,
) -> std::result::Result<(String, String), AgentError> {
    // Strategy 1: "op" field with "verb noun" string
    if let Some(op_str) = obj.get("op").and_then(|v| v.as_str()) {
        let parts: Vec<&str> = op_str.split_whitespace().collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
        if parts.len() == 1 {
            return Ok((parts[0].to_string(), "agent".to_string()));
        }
    }

    // Strategy 2: Separate verb/noun fields
    if let Some(verb) = obj.get("verb").and_then(|v| v.as_str()) {
        let noun = obj.get("noun").and_then(|v| v.as_str()).unwrap_or("agent");
        return Ok((verb.to_string(), noun.to_string()));
    }

    // Strategy 3: Shorthand — presence of "name" implies "use agent"
    if obj.contains_key("name") && !obj.contains_key("op") && !obj.contains_key("verb") {
        return Ok(("use".to_string(), "agent".to_string()));
    }

    // Strategy 4: Shorthand — presence of "query" implies "search agent"
    if obj.contains_key("query") && !obj.contains_key("op") && !obj.contains_key("verb") {
        return Ok(("search".to_string(), "agent".to_string()));
    }

    // Strategy 5: Empty object or no verb → "list agent"
    if obj.is_empty()
        || (!obj.contains_key("name")
            && !obj.contains_key("query")
            && !obj.contains_key("op")
            && !obj.contains_key("verb"))
    {
        return Ok(("list".to_string(), "agent".to_string()));
    }

    Err(AgentError::Parse {
        message: "cannot determine agent operation from input".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_list_op() {
        let input = json!({"op": "list agent"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::List(_)));
    }

    #[test]
    fn test_parse_use_op() {
        let input = json!({"op": "use agent", "name": "test"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::Use(_)));
    }

    #[test]
    fn test_parse_search_op() {
        let input = json!({"op": "search agent", "query": "test"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::Search(_)));
    }

    #[test]
    fn test_parse_shorthand_use() {
        let input = json!({"name": "test"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::Use(_)));
    }

    #[test]
    fn test_parse_shorthand_search() {
        let input = json!({"query": "commit"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::Search(_)));
    }

    #[test]
    fn test_parse_empty_is_list() {
        let input = json!({});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::List(_)));
    }

    #[test]
    fn test_parse_non_object_fails() {
        let input = json!("not an object");
        assert!(parse_input(input).is_err());
    }

    #[test]
    fn test_parse_array_fails() {
        let input = json!([1, 2, 3]);
        assert!(parse_input(input).is_err());
    }

    #[test]
    fn test_parse_list_aliases() {
        for verb in &["ls", "show", "available"] {
            let input = json!({"op": verb});
            let op = parse_input(input).unwrap();
            assert!(
                matches!(op, AgentOperation::List(_)),
                "verb '{}' should map to List",
                verb
            );
        }
    }

    #[test]
    fn test_parse_use_aliases() {
        for verb in &["get", "load", "activate", "invoke"] {
            let input = json!({"op": verb, "name": "default"});
            let op = parse_input(input).unwrap();
            assert!(
                matches!(op, AgentOperation::Use(_)),
                "verb '{}' should map to Use",
                verb
            );
        }
    }

    #[test]
    fn test_parse_search_aliases() {
        for verb in &["find", "lookup"] {
            let input = json!({"op": verb, "query": "test"});
            let op = parse_input(input).unwrap();
            assert!(
                matches!(op, AgentOperation::Search(_)),
                "verb '{}' should map to Search",
                verb
            );
        }
    }

    #[test]
    fn test_parse_verb_noun_fields() {
        let input = json!({"verb": "list", "noun": "agent"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::List(_)));
    }

    #[test]
    fn test_parse_verb_field_without_noun() {
        let input = json!({"verb": "list"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::List(_)));
    }

    #[test]
    fn test_parse_use_missing_name_fails() {
        let input = json!({"op": "use agent"});
        assert!(parse_input(input).is_err());
    }

    #[test]
    fn test_parse_search_missing_query_fails() {
        let input = json!({"op": "search agent"});
        assert!(parse_input(input).is_err());
    }

    #[test]
    fn test_parse_unsupported_verb_fails() {
        let input = json!({"op": "delete agent"});
        assert!(parse_input(input).is_err());
    }

    #[test]
    fn test_parse_op_verb_only() {
        // op field with just a single word (no noun)
        let input = json!({"op": "list"});
        let op = parse_input(input).unwrap();
        assert!(matches!(op, AgentOperation::List(_)));
    }
}
