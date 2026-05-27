//! Snapshot-style assertions on the generated wire `inputSchema`.
//!
//! Pins the contract that `generate_mcp_schema` produces for the six
//! `command` operations: the `op` enum lists every verb+noun pair, the
//! per-operation schemas in `x-operation-schemas` cover the union of
//! parameters, and operations sharing the noun `command` group together
//! in `x-operation-groups`.

use swissarmyhammer_command_service::operations;
use swissarmyhammer_operations::{generate_mcp_schema, SchemaConfig};

/// The schema's top-level shape: object with a flat properties map and
/// the documented extension fields.
#[test]
fn schema_has_top_level_object_shape() {
    let schema = generate_mcp_schema(operations(), SchemaConfig::new("command tool"));

    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], true);
    assert!(schema["properties"].is_object());
    assert!(schema["x-operation-schemas"].is_array());
    assert!(schema["x-operation-groups"].is_object());
}

/// The `op` field's enum lists every verb+noun pair, in any order.
#[test]
fn schema_op_enum_lists_every_verb() {
    let schema = generate_mcp_schema(operations(), SchemaConfig::new("command tool"));
    let enum_values: Vec<String> = schema["properties"]["op"]["enum"]
        .as_array()
        .expect("op.enum must be an array")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    let expected = [
        "register command",
        "unregister command",
        "execute command",
        "available command",
        "list command",
        "schema command",
    ];
    for op in expected {
        assert!(
            enum_values.contains(&op.to_string()),
            "op enum missing {op:?}: {enum_values:?}"
        );
    }
    assert_eq!(
        enum_values.len(),
        expected.len(),
        "unexpected ops in enum: {enum_values:?}"
    );
}

/// Each operation has a per-operation schema in `x-operation-schemas` whose
/// `op` const matches the operation's verb+noun.
#[test]
fn schema_per_operation_entries_cover_every_verb() {
    let schema = generate_mcp_schema(operations(), SchemaConfig::new("command tool"));
    let per_op = schema["x-operation-schemas"].as_array().unwrap();

    let titles: Vec<String> = per_op
        .iter()
        .map(|s| s["title"].as_str().unwrap().to_string())
        .collect();

    for op in [
        "register command",
        "unregister command",
        "execute command",
        "available command",
        "list command",
        "schema command",
    ] {
        assert!(
            titles.contains(&op.to_string()),
            "x-operation-schemas missing {op:?}: {titles:?}"
        );
    }
}

/// The shared properties map covers the union of every operation's
/// parameters. This guards against silently dropping a param when one of
/// the operations gets edited — including the `register`-only registration
/// fields, so dropping any of them fails this snapshot in addition to
/// `meta_tree.rs::meta_tree_register_exposes_full_registration_payload`.
#[test]
fn schema_properties_union_covers_known_params() {
    let schema = generate_mcp_schema(operations(), SchemaConfig::new("command tool"));
    let properties = schema["properties"].as_object().unwrap();

    // op is always present
    assert!(properties.contains_key("op"));

    // Parameters that each operation contributes:
    //   register: id, name, execute (required), plus the YAML-equivalent
    //             optional fields (menu_name, description, category,
    //             scope, keys, menu, context_menu, context_menu_group,
    //             context_menu_order, tab_button, view_kinds, undoable,
    //             visible, params, available)
    //   unregister/schema: id
    //   execute/available: id, ctx, (execute also: force)
    //   list: scope, category, id_prefix
    let expected = [
        // shared across multiple verbs
        "id",
        "ctx",
        "force",
        "category",
        "id_prefix",
        // register-only required fields
        "name",
        "execute",
        // register-only optional fields the YAML supports — `scope` is
        // shared with `list`, but every other entry is unique to register
        "menu_name",
        "description",
        "scope",
        "keys",
        "menu",
        "context_menu",
        "context_menu_group",
        "context_menu_order",
        "tab_button",
        "view_kinds",
        "undoable",
        "visible",
        "params",
        "available",
    ];
    for field in expected {
        assert!(
            properties.contains_key(field),
            "expected union to contain param {field:?}: keys = {:?}",
            properties.keys().collect::<Vec<_>>()
        );
    }
}

/// Operations sharing the noun `command` group together under one
/// `x-operation-groups.command` entry.
#[test]
fn schema_groups_operations_under_command_noun() {
    let schema = generate_mcp_schema(operations(), SchemaConfig::new("command tool"));
    let groups = schema["x-operation-groups"].as_object().unwrap();

    let command_group = groups
        .get("command")
        .expect("expected an x-operation-groups.command entry")
        .as_array()
        .expect("expected the command group to be an array");

    assert_eq!(
        command_group.len(),
        6,
        "command group should hold one entry per verb: {command_group:?}"
    );
}
