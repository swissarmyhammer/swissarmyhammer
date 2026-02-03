//! Forgiving input parsing for kanban operations
//!
//! The parser accepts multiple input formats and normalizes them to canonical Operations.

use crate::error::{KanbanError, Result};
use crate::types::{Noun, Operation, Verb};
use serde_json::{Map, Value};

/// Parse input JSON into one or more Operations
pub fn parse_input(input: Value) -> Result<Vec<Operation>> {
    match input {
        Value::Array(arr) => {
            // Batch operations
            arr.into_iter().map(parse_single).collect()
        }
        Value::Object(obj) => {
            // Single operation
            Ok(vec![parse_single(Value::Object(obj))?])
        }
        _ => Err(KanbanError::parse("input must be an object or array")),
    }
}

/// Parse a single operation from JSON
fn parse_single(input: Value) -> Result<Operation> {
    let obj = match input {
        Value::Object(obj) => obj,
        _ => return Err(KanbanError::parse("operation must be an object")),
    };

    // Try to extract verb and noun
    let (verb, noun, mut params) = extract_operation(&obj)?;

    // Extract actor if present (before normalizing params)
    let actor = obj
        .get("actor")
        .and_then(|v| v.as_str())
        .map(crate::types::ActorId::from_string);

    // Normalize parameters (resolve aliases, snake_case keys)
    normalize_params(&mut params);

    let mut operation = Operation::new(verb, noun, params);
    if let Some(actor) = actor {
        operation = operation.with_actor(actor);
    }

    Ok(operation)
}

/// Extract verb and noun from the input object
fn extract_operation(obj: &Map<String, Value>) -> Result<(Verb, Noun, Map<String, Value>)> {
    // Strategy 1: Explicit "op" field with "verb noun" string
    if let Some(op_value) = obj.get("op").or_else(|| obj.get("operation")).or_else(|| obj.get("action")) {
        if let Some(op_str) = op_value.as_str() {
            if let Some((verb, noun)) = parse_op_string(op_str) {
                let params = filter_op_keys(obj);
                return Ok((verb, noun, params));
            }
        }
    }

    // Strategy 2: Separate verb/noun fields
    let verb_value = obj.get("verb").or_else(|| obj.get("action"));
    let noun_value = obj.get("noun").or_else(|| obj.get("target"));

    if let (Some(v), Some(n)) = (verb_value, noun_value) {
        if let (Some(verb_str), Some(noun_str)) = (v.as_str(), n.as_str()) {
            if let (Some(verb), Some(noun)) = (Verb::from_alias(verb_str), Noun::parse(noun_str)) {
                let params = filter_verb_noun_keys(obj);
                return Ok((verb, noun, params));
            }
        }
    }

    // Strategy 3: Shorthand keys like { "add": "task", ... }
    for (key, value) in obj {
        if let Some(verb) = Verb::from_alias(key) {
            if let Some(noun_str) = value.as_str() {
                if let Some(noun) = Noun::parse(noun_str) {
                    let params = filter_shorthand_keys(obj, key);
                    return Ok((verb, noun, params));
                }
            }
        }
    }

    // Strategy 4: Infer from data
    if let Some((verb, noun)) = infer_operation(obj) {
        let params = obj.clone();
        return Ok((verb, noun, params));
    }

    Err(KanbanError::parse("cannot determine operation from input"))
}

/// Parse an "op" string like "add task" into (Verb, Noun)
fn parse_op_string(s: &str) -> Option<(Verb, Noun)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let verb = Verb::from_alias(parts[0])?;
    let noun = Noun::parse(parts[1])?;
    Some((verb, noun))
}

/// Infer operation from the data present
fn infer_operation(obj: &Map<String, Value>) -> Option<(Verb, Noun)> {
    let has_id = obj.contains_key("id") || obj.contains_key("task_id") || obj.contains_key("taskId");
    let has_title = obj.contains_key("title");
    let has_column = obj.contains_key("column") || obj.contains_key("position");

    // Has title but no id → add task
    if has_title && !has_id {
        return Some((Verb::Add, Noun::Task));
    }

    // Has id + column but no other updates → move task
    if has_id && has_column {
        let update_keys = ["title", "description", "tags", "assignees", "depends_on", "subtasks"];
        if !update_keys.iter().any(|k| obj.contains_key(*k)) {
            return Some((Verb::Move, Noun::Task));
        }
    }

    // Has id + other fields → update task
    if has_id {
        let has_updates = obj.keys().any(|k| {
            !matches!(k.as_str(), "id" | "task_id" | "taskId")
        });
        if has_updates {
            return Some((Verb::Update, Noun::Task));
        }
        // Just id → get task
        return Some((Verb::Get, Noun::Task));
    }

    // Empty or just path → get board
    if obj.is_empty() || (obj.len() == 1 && obj.contains_key("path")) {
        return Some((Verb::Get, Noun::Board));
    }

    None
}

/// Filter out op/operation/action/actor/note keys
fn filter_op_keys(obj: &Map<String, Value>) -> Map<String, Value> {
    obj.iter()
        .filter(|(k, _)| !matches!(k.as_str(), "op" | "operation" | "action" | "actor" | "note"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Filter out verb/noun/action/target/actor/note keys
fn filter_verb_noun_keys(obj: &Map<String, Value>) -> Map<String, Value> {
    obj.iter()
        .filter(|(k, _)| !matches!(k.as_str(), "verb" | "noun" | "action" | "target" | "actor" | "note"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Filter out the shorthand key and metadata keys (actor, note)
fn filter_shorthand_keys(obj: &Map<String, Value>, shorthand_key: &str) -> Map<String, Value> {
    obj.iter()
        .filter(|(k, _)| !matches!(k.as_str(), k if k == shorthand_key || k == "actor" || k == "note"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Normalize parameter keys (aliases, snake_case)
fn normalize_params(params: &mut Map<String, Value>) {
    // Key aliases
    let aliases: &[(&[&str], &str)] = &[
        (&["taskId", "task_id"], "id"),
        (&["desc", "body", "content"], "description"),
        (&["col"], "column"),
        (&["lane"], "swimlane"),
    ];

    for (from_keys, to_key) in aliases {
        for from_key in *from_keys {
            if let Some(value) = params.remove(*from_key) {
                if !params.contains_key(*to_key) {
                    params.insert(to_key.to_string(), value);
                }
            }
        }
    }

    // Convert camelCase to snake_case (simplified)
    let keys_to_convert: Vec<String> = params.keys().cloned().collect();
    for key in keys_to_convert {
        let snake = to_snake_case(&key);
        if snake != key {
            if let Some(value) = params.remove(&key) {
                if !params.contains_key(&snake) {
                    params.insert(snake, value);
                }
            }
        }
    }
}

/// Simple camelCase to snake_case conversion
fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_explicit_op() {
        let input = json!({ "op": "add task", "title": "Test" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].verb, Verb::Add);
        assert_eq!(ops[0].noun, Noun::Task);
        assert_eq!(ops[0].params.get("title").unwrap(), "Test");
    }

    #[test]
    fn test_parse_verb_noun_fields() {
        let input = json!({ "verb": "add", "noun": "task", "title": "Test" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].verb, Verb::Add);
        assert_eq!(ops[0].noun, Noun::Task);
    }

    #[test]
    fn test_parse_shorthand() {
        let input = json!({ "add": "task", "title": "Test" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].verb, Verb::Add);
        assert_eq!(ops[0].noun, Noun::Task);
    }

    #[test]
    fn test_parse_verb_aliases() {
        let input = json!({ "op": "create task", "title": "Test" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].verb, Verb::Init); // 'create' -> Init for board context, but works as Add for task

        let input2 = json!({ "op": "rm task", "id": "abc" });
        let ops2 = parse_input(input2).unwrap();
        assert_eq!(ops2[0].verb, Verb::Delete);
    }

    #[test]
    fn test_infer_add_task() {
        let input = json!({ "title": "New task" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].verb, Verb::Add);
        assert_eq!(ops[0].noun, Noun::Task);
    }

    #[test]
    fn test_infer_move_task() {
        let input = json!({ "id": "abc", "column": "done" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].verb, Verb::Move);
        assert_eq!(ops[0].noun, Noun::Task);
    }

    #[test]
    fn test_infer_get_board() {
        let input = json!({});
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].verb, Verb::Get);
        assert_eq!(ops[0].noun, Noun::Board);
    }

    #[test]
    fn test_batch_operations() {
        let input = json!([
            { "op": "add task", "title": "Task 1" },
            { "op": "add task", "title": "Task 2" }
        ]);
        let ops = parse_input(input).unwrap();
        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn test_normalize_aliases() {
        let input = json!({ "op": "get task", "taskId": "abc" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].params.get("id").unwrap(), "abc");
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(to_snake_case("dependsOn"), "depends_on");
        assert_eq!(to_snake_case("taskId"), "task_id");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn test_parse_with_actor() {
        let input = json!({ "op": "add task", "title": "Test", "actor": "user123" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].actor, Some(crate::types::ActorId::from_string("user123")));
        assert_eq!(ops[0].params.get("title").unwrap(), "Test");
    }

    #[test]
    fn test_parse_without_actor() {
        let input = json!({ "op": "add task", "title": "Test" });
        let ops = parse_input(input).unwrap();
        assert_eq!(ops[0].actor, None);
    }
}
