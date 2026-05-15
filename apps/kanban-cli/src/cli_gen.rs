//! CLI generation from JSON schema.
//!
//! Builds a clap `Command` tree from the kanban schema's `op` enum values
//! and `properties`. Extracts typed arguments from clap matches back into JSON.

use clap::{Arg, ArgAction, Command};
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// String interning (clap requires 'static lifetimes)
// ---------------------------------------------------------------------------

static STRING_CACHE: Lazy<Mutex<HashSet<&'static str>>> = Lazy::new(|| Mutex::new(HashSet::new()));

fn intern(s: String) -> &'static str {
    let mut cache = STRING_CACHE.lock().unwrap();
    if let Some(&cached) = cache.get(s.as_str()) {
        return cached;
    }
    let leaked: &'static str = Box::leak(s.into_boxed_str());
    cache.insert(leaked);
    leaked
}

// ---------------------------------------------------------------------------
// Schema helpers
// ---------------------------------------------------------------------------

fn schema_has_type(schema: &Value, type_name: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(t)) => t.as_str() == type_name,
        Some(Value::Array(types)) => types.iter().any(|t| t.as_str() == Some(type_name)),
        _ => false,
    }
}

fn primary_type(schema: &Value) -> Option<&str> {
    match schema.get("type") {
        Some(Value::String(t)) => Some(t.as_str()),
        Some(Value::Array(types)) => types
            .iter()
            .find_map(|t| t.as_str().filter(|s| *s != "null")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Build clap commands from schema
// ---------------------------------------------------------------------------

/// Build the noun -> verb command tree from the schema.
///
/// Uses `x-operation-schemas` to scope each verb's args to only
/// the parameters that operation actually accepts.
pub fn build_commands_from_schema(schema: &Value) -> Vec<Command> {
    // Build per-operation arg maps from x-operation-schemas
    // Key: op string (e.g. "init board"), Value: (description, args)
    let mut op_args: HashMap<String, (String, Vec<ArgMeta>)> = HashMap::new();

    if let Some(op_schemas) = schema.get("x-operation-schemas").and_then(|v| v.as_array()) {
        for op_schema in op_schemas {
            if let Some(title) = op_schema.get("title").and_then(|t| t.as_str()) {
                let desc = op_schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = precompute_args(op_schema);
                op_args.insert(title.to_string(), (desc, args));
            }
        }
    }

    // Fall back to global schema args if no x-operation-schemas
    let fallback_args = if op_args.is_empty() {
        precompute_args(schema)
    } else {
        Vec::new()
    };

    // Parse op enum to get verb-noun pairs
    let op_values = match schema
        .get("properties")
        .and_then(|p| p.get("op"))
        .and_then(|op| op.get("enum"))
        .and_then(|e| e.as_array())
    {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    // Group by noun, carrying the op string for arg lookup
    let mut noun_groups: HashMap<String, Vec<(String, String)>> = HashMap::new(); // noun -> [(verb, op_string)]
    for op_val in op_values {
        if let Some(op_str) = op_val.as_str() {
            let parts: Vec<&str> = op_str.splitn(2, ' ').collect();
            if parts.len() == 2 {
                noun_groups
                    .entry(parts[1].to_string())
                    .or_default()
                    .push((parts[0].to_string(), op_str.to_string()));
            }
        }
    }

    let mut commands: Vec<Command> = noun_groups
        .into_iter()
        .map(|(noun, verbs)| build_noun_command(&noun, &verbs, &op_args, &fallback_args))
        .collect();
    commands.sort_by(|a, b| a.get_name().cmp(b.get_name()));
    commands
}

fn build_noun_command(
    noun: &str,
    verbs: &[(String, String)],
    op_args: &HashMap<String, (String, Vec<ArgMeta>)>,
    fallback_args: &[ArgMeta],
) -> Command {
    let mut cmd =
        Command::new(intern(noun.to_string())).about(intern(format!("{} operations", noun)));

    let mut verb_cmds: Vec<Command> = verbs
        .iter()
        .map(|(verb, op_str)| build_verb_command(verb, op_str, op_args, fallback_args))
        .collect();
    verb_cmds.sort_by(|a, b| a.get_name().cmp(b.get_name()));

    for verb_cmd in verb_cmds {
        cmd = cmd.subcommand(verb_cmd);
    }
    cmd
}

fn build_verb_command(
    verb: &str,
    op_str: &str,
    op_args: &HashMap<String, (String, Vec<ArgMeta>)>,
    fallback_args: &[ArgMeta],
) -> Command {
    // Use per-operation args if available, otherwise fall back to global
    let (about, args) = if let Some((desc, per_op_args)) = op_args.get(op_str) {
        (desc.as_str(), per_op_args.as_slice())
    } else {
        ("", fallback_args)
    };

    let about_str = if about.is_empty() {
        format!("{} operation", verb)
    } else {
        about.to_string()
    };

    let mut cmd = Command::new(intern(verb.to_string())).about(intern(about_str));

    for arg in args {
        if arg.name == "op" {
            continue;
        }
        cmd = cmd.arg(build_clap_arg(arg));
    }
    cmd
}

// ---------------------------------------------------------------------------
// Arg metadata extraction from JSON schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ArgMeta {
    name: String,
    help: Option<String>,
    is_required: bool,
    arg_type: ArgMetaType,
    default_value: Option<String>,
    possible_values: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
enum ArgMetaType {
    String,
    Integer,
    Float,
    Boolean,
    NullableBoolean,
    Array,
}

fn precompute_args(schema: &Value) -> Vec<ArgMeta> {
    let properties = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(props) => props,
        None => return Vec::new(),
    };

    let required: HashSet<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    properties
        .iter()
        .map(|(name, prop_schema)| {
            // In CLI context, arrays default to empty so never require them.
            // Also skip requiring "op" — it's set by the noun-verb path.
            let is_required = required.contains(name)
                && !matches!(primary_type(prop_schema), Some("array"))
                && name != "op";
            let arg_type = if schema_has_type(prop_schema, "boolean")
                && schema_has_type(prop_schema, "null")
            {
                ArgMetaType::NullableBoolean
            } else {
                match primary_type(prop_schema) {
                    Some("boolean") => ArgMetaType::Boolean,
                    Some("integer") => ArgMetaType::Integer,
                    Some("number") => ArgMetaType::Float,
                    Some("array") => ArgMetaType::Array,
                    _ => ArgMetaType::String,
                }
            };

            ArgMeta {
                name: name.clone(),
                help: prop_schema
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                is_required,
                arg_type,
                default_value: prop_schema
                    .get("default")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                possible_values: prop_schema
                    .get("enum")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    }),
            }
        })
        .collect()
}

fn build_clap_arg(meta: &ArgMeta) -> Arg {
    let name_static = intern(meta.name.clone());
    let mut arg = Arg::new(name_static).long(name_static);

    if meta.is_required {
        arg = arg.required(true);
    }
    if let Some(ref help) = meta.help {
        arg = arg.help(intern(help.clone()));
    }
    if let Some(ref default) = meta.default_value {
        arg = arg.default_value(intern(default.clone()));
    }
    if let Some(ref values) = meta.possible_values {
        let strs: Vec<&'static str> = values.iter().map(|s| intern(s.clone())).collect();
        arg = arg.value_parser(clap::builder::PossibleValuesParser::new(strs));
    }

    arg = match meta.arg_type {
        ArgMetaType::Boolean => arg.action(ArgAction::SetTrue),
        ArgMetaType::NullableBoolean => arg
            .value_parser(clap::builder::PossibleValuesParser::new(["true", "false"]))
            .value_name("BOOL"),
        ArgMetaType::Integer => {
            let mut a = arg.value_parser(clap::value_parser!(i64));
            if !meta.is_required {
                a = a.value_name("NUMBER");
            }
            a
        }
        ArgMetaType::Float => {
            let mut a = arg.value_parser(clap::value_parser!(f64));
            if !meta.is_required {
                a = a.value_name("NUMBER");
            }
            a
        }
        ArgMetaType::Array => {
            let mut a = arg.action(ArgAction::Append);
            if !meta.is_required {
                a = a.value_name("VALUE");
            }
            a
        }
        ArgMetaType::String => {
            if !meta.is_required {
                arg.value_name("TEXT")
            } else {
                arg
            }
        }
    };

    arg
}

// ---------------------------------------------------------------------------
// Extract arguments from clap matches
// ---------------------------------------------------------------------------

/// Extract noun-verb arguments from matched clap args.
///
/// Navigates the noun -> verb subcommand tree and builds a JSON object
/// with `"op": "verb noun"` plus all matched arguments.
pub fn extract_noun_verb_arguments(
    matches: &clap::ArgMatches,
    schema: &Value,
) -> Result<serde_json::Map<String, Value>, String> {
    match matches.subcommand() {
        Some((noun, noun_matches)) => match noun_matches.subcommand() {
            Some((verb, verb_matches)) => {
                let op_string = format!("{} {}", verb, noun);
                build_arguments_from_matches(verb_matches, &op_string, schema)
            }
            None => Err(format!("No verb specified for '{}'", noun)),
        },
        None => Err("No noun specified".to_string()),
    }
}

fn build_arguments_from_matches(
    matches: &clap::ArgMatches,
    op_string: &str,
    schema: &Value,
) -> Result<serde_json::Map<String, Value>, String> {
    let mut arguments = serde_json::Map::new();

    if !op_string.is_empty() {
        arguments.insert("op".to_string(), Value::String(op_string.to_string()));
    }

    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in properties {
            if prop_name == "op" {
                continue;
            }
            if let Some(value) = extract_value_from_matches(matches, prop_name, prop_schema) {
                arguments.insert(prop_name.clone(), value);
            }
        }
    }

    Ok(arguments)
}

fn extract_value_from_matches(
    matches: &clap::ArgMatches,
    name: &str,
    schema: &Value,
) -> Option<Value> {
    // Skip args not defined in this command (per-operation scoping)
    if !matches.try_contains_id(name).unwrap_or(false) {
        return None;
    }

    let type_str = primary_type(schema);

    match type_str {
        Some("boolean") => matches.get_flag(name).then_some(Value::Bool(true)),
        Some("integer") => matches
            .get_one::<i64>(name)
            .map(|v| Value::Number((*v).into())),
        Some("number") => matches
            .get_one::<f64>(name)
            .and_then(|v| serde_json::Number::from_f64(*v))
            .map(Value::Number),
        Some("array") => {
            let values: Vec<String> = matches
                .get_many::<String>(name)
                .map(|v| v.cloned().collect())
                .unwrap_or_default();
            if values.is_empty() {
                None
            } else {
                Some(Value::Array(
                    values.into_iter().map(Value::String).collect(),
                ))
            }
        }
        _ => matches
            .get_one::<String>(name)
            .map(|v| Value::String(v.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the real kanban schema for testing.
    fn kanban_schema() -> Value {
        let ops = swissarmyhammer_kanban::schema::kanban_operations();
        swissarmyhammer_kanban::schema::generate_kanban_mcp_schema(ops)
    }

    #[test]
    fn build_commands_produces_noun_verb_tree() {
        let schema = kanban_schema();
        let commands = build_commands_from_schema(&schema);

        let names: Vec<&str> = commands.iter().map(|c| c.get_name()).collect();
        // Must include core nouns
        assert!(names.contains(&"board"), "missing board noun");
        assert!(names.contains(&"task"), "missing task noun");
        assert!(names.contains(&"column"), "missing column noun");
        assert!(names.contains(&"tag"), "missing tag noun");
    }

    #[test]
    fn board_noun_has_expected_verbs() {
        let schema = kanban_schema();
        let commands = build_commands_from_schema(&schema);

        let board = commands.iter().find(|c| c.get_name() == "board").unwrap();
        let verb_names: Vec<&str> = board.get_subcommands().map(|c| c.get_name()).collect();
        assert!(verb_names.contains(&"init"), "board missing init verb");
        assert!(verb_names.contains(&"get"), "board missing get verb");
        assert!(verb_names.contains(&"update"), "board missing update verb");
    }

    #[test]
    fn board_init_has_scoped_args() {
        let schema = kanban_schema();
        let commands = build_commands_from_schema(&schema);

        let board = commands.iter().find(|c| c.get_name() == "board").unwrap();
        let init = board
            .get_subcommands()
            .find(|c| c.get_name() == "init")
            .unwrap();
        let arg_names: Vec<&str> = init.get_arguments().map(|a| a.get_id().as_str()).collect();

        assert!(arg_names.contains(&"name"), "init missing --name");
        // Should NOT have task-specific args
        assert!(
            !arg_names.contains(&"title"),
            "init should not have --title"
        );
        assert!(
            !arg_names.contains(&"assignee"),
            "init should not have --assignee"
        );
    }

    #[test]
    fn extract_noun_verb_roundtrip() {
        let schema = kanban_schema();
        let commands = build_commands_from_schema(&schema);

        // Build a clap command matching main.rs structure
        let mut cmd = Command::new("kanban");
        for subcmd in commands {
            cmd = cmd.subcommand(subcmd);
        }

        let matches = cmd
            .try_get_matches_from(["kanban", "board", "init", "--name", "Test Board"])
            .unwrap();

        let args = extract_noun_verb_arguments(&matches, &schema).unwrap();
        assert_eq!(args.get("op").unwrap(), "init board");
        assert_eq!(args.get("name").unwrap(), "Test Board");
    }

    #[test]
    fn extract_task_add_with_optional_args() {
        let schema = kanban_schema();
        let commands = build_commands_from_schema(&schema);

        let mut cmd = Command::new("kanban");
        for subcmd in commands {
            cmd = cmd.subcommand(subcmd);
        }

        let matches = cmd
            .try_get_matches_from([
                "kanban",
                "task",
                "add",
                "--title",
                "Fix bug",
                "--description",
                "Something broke",
            ])
            .unwrap();

        let args = extract_noun_verb_arguments(&matches, &schema).unwrap();
        assert_eq!(args.get("op").unwrap(), "add task");
        assert_eq!(args.get("title").unwrap(), "Fix bug");
        assert_eq!(args.get("description").unwrap(), "Something broke");
        // assignees not provided — should be absent
        assert!(args.get("assignees").is_none());
    }

    #[test]
    fn extract_missing_verb_returns_error() {
        let schema = kanban_schema();
        let commands = build_commands_from_schema(&schema);

        let mut cmd = Command::new("kanban")
            .subcommand_required(false)
            .allow_external_subcommands(true);
        for subcmd in commands {
            cmd = cmd.subcommand(subcmd);
        }

        // "board" noun without a verb — clap treats it as external subcommand
        let matches = cmd.try_get_matches_from(["kanban", "board"]).unwrap();

        let result = extract_noun_verb_arguments(&matches, &schema);
        assert!(result.is_err());
    }
}
