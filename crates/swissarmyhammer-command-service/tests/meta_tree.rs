//! Snapshot-style assertions on the `io.swissarmyhammer/operations` `_meta`
//! tree the discovery generator produces for the six `command` operations.

use swissarmyhammer_command_service::operations;
use swissarmyhammer_operations::generate_operations_meta;

/// The meta tree groups every verb under the noun `command`.
#[test]
fn meta_tree_groups_under_command_noun() {
    let meta = generate_operations_meta(operations());

    let command = meta
        .as_object()
        .expect("meta tree should be an object")
        .get("command")
        .expect("meta tree should hold a `command` noun")
        .as_object()
        .expect("meta.command should be an object");

    for verb in [
        "register",
        "unregister",
        "execute",
        "available",
        "list",
        "schema",
    ] {
        assert!(
            command.contains_key(verb),
            "meta.command.{verb:?} missing: keys = {:?}",
            command.keys().collect::<Vec<_>>()
        );
    }

    assert_eq!(
        command.len(),
        6,
        "expected exactly 6 verbs under meta.command"
    );
}

/// Each verb leaf carries `op`, `description`, and `parameters`.
#[test]
fn meta_tree_leaf_shape() {
    let meta = generate_operations_meta(operations());
    let register = &meta["command"]["register"];

    assert_eq!(register["op"], "register command");
    assert!(
        register["description"].is_string(),
        "register leaf should carry a description"
    );
    assert!(
        register["parameters"].is_object(),
        "register leaf should carry a parameters object"
    );
}

/// `unregister`/`schema`/`execute`/`available` all expose `id` as a
/// required string param.
#[test]
fn meta_tree_id_param_is_required_where_expected() {
    let meta = generate_operations_meta(operations());

    for verb in ["unregister", "schema", "execute", "available"] {
        let id = &meta["command"][verb]["parameters"]["id"];
        assert_eq!(id["type"], "string", "{verb}.id type");
        assert_eq!(id["required"], true, "{verb}.id required flag");
    }
}

/// `list` exposes its three filters as optional strings.
#[test]
fn meta_tree_list_filters_are_optional() {
    let meta = generate_operations_meta(operations());
    let params = &meta["command"]["list"]["parameters"];

    for field in ["scope", "category", "id_prefix"] {
        let entry = &params[field];
        assert_eq!(entry["type"], "string", "list.{field} type");
        assert_eq!(entry["required"], false, "list.{field} required flag");
    }
}

/// `execute` exposes `force` as an optional boolean param.
#[test]
fn meta_tree_execute_force_is_optional_boolean() {
    let meta = generate_operations_meta(operations());
    let force = &meta["command"]["execute"]["parameters"]["force"];

    assert_eq!(force["type"], "boolean");
    assert_eq!(force["required"], false);
}

/// `register` exposes every YAML-equivalent registration field so the
/// discovery `_meta` covers the union built-in plugins write today.
#[test]
fn meta_tree_register_exposes_full_registration_payload() {
    let meta = generate_operations_meta(operations());
    let params = &meta["command"]["register"]["parameters"];

    // Required fields (no Option<>).
    assert_eq!(params["id"]["required"], true);
    assert_eq!(params["name"]["required"], true);
    assert_eq!(params["execute"]["required"], true);

    // Optional fields the YAML supports — each must appear in the meta
    // tree (rather than being silently dropped).
    for field in [
        "menu_name",
        "description",
        "category",
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
    ] {
        let entry = &params[field];
        assert!(entry.is_object(), "register.{field} missing from meta tree");
        assert_eq!(
            entry["required"], false,
            "register.{field} should be optional"
        );
    }
}
