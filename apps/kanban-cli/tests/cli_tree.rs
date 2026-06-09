//! Golden command-tree tests for the schema-driven `kanban` noun/verb surface.
//!
//! These tests assert that the clap command tree kanban-cli builds — the
//! schema-driven noun → verb → args portion produced by the shared
//! [`swissarmyhammer_operations::cli_gen`] generator fed the kanban FULL schema
//! (`generate_kanban_mcp_schema_full`) — keeps its expected nouns, verbs, and
//! per-op-scoped required flags.
//!
//! They are built from the exact same data source `src/main.rs::build_cli`
//! uses (the full schema → shared generator), so a regression in either the
//! schema or the generator surfaces here as a failing golden assertion rather
//! than a silently broken CLI.

use clap::Command;
use serde_json::Value;
use swissarmyhammer_operations::cli_gen::build_commands_from_schema;

/// Build the kanban FULL schema exactly as `src/main.rs` does.
fn kanban_full_schema() -> Value {
    let operations = swissarmyhammer_kanban::schema::kanban_operations();
    swissarmyhammer_kanban::schema::generate_kanban_mcp_schema_full(operations)
}

/// Assemble the schema-driven noun commands under a `kanban` root, mirroring
/// the schema-driven portion of `src/main.rs::build_cli`.
fn schema_driven_root() -> Command {
    let schema = kanban_full_schema();
    let mut cmd = Command::new("kanban");
    for subcmd in build_commands_from_schema(&schema) {
        cmd = cmd.subcommand(subcmd);
    }
    cmd
}

/// Look up a subcommand named `name` under `cmd`, panicking if absent.
///
/// `role` names what is being looked up (`"noun"` or `"verb"`) so the panic
/// message reads naturally for either level of the noun → verb tree.
fn sub<'a>(cmd: &'a Command, name: &str, role: &str) -> &'a Command {
    cmd.get_subcommands()
        .find(|c| c.get_name() == name)
        .unwrap_or_else(|| panic!("missing `{name}` {role} under `{}`", cmd.get_name()))
}

/// Collect the argument ids declared on a verb command.
fn arg_ids(verb_cmd: &Command) -> Vec<&str> {
    verb_cmd
        .get_arguments()
        .map(|a| a.get_id().as_str())
        .collect()
}

#[test]
fn command_tree_has_expected_nouns() {
    let root = schema_driven_root();
    let names: Vec<&str> = root.get_subcommands().map(|c| c.get_name()).collect();

    for expected in ["board", "task", "column", "tag", "actor", "project"] {
        assert!(
            names.contains(&expected),
            "schema-driven command tree missing `{expected}` noun; got {names:?}",
        );
    }
}

#[test]
fn board_noun_carries_its_verbs() {
    let root = schema_driven_root();
    let board = sub(&root, "board", "noun");
    let verbs: Vec<&str> = board.get_subcommands().map(|c| c.get_name()).collect();

    for expected in ["init", "get", "update"] {
        assert!(
            verbs.contains(&expected),
            "board noun missing `{expected}` verb; got {verbs:?}",
        );
    }
}

#[test]
fn board_init_args_are_scoped_to_that_op() {
    let root = schema_driven_root();
    let init = sub(sub(&root, "board", "noun"), "init", "verb");
    let args = arg_ids(init);

    assert!(
        args.contains(&"name"),
        "board init missing --name; got {args:?}"
    );
    // Per-op scoping: task-specific args must NOT leak onto `board init`.
    assert!(
        !args.contains(&"title"),
        "board init should not carry --title (global-union leak); got {args:?}",
    );
    assert!(
        !args.contains(&"assignees"),
        "board init should not carry --assignees (global-union leak); got {args:?}",
    );
}

#[test]
fn task_move_requires_its_scoped_fields() {
    let root = schema_driven_root();
    let mv = sub(sub(&root, "task", "noun"), "move", "verb");

    let id = mv
        .get_arguments()
        .find(|a| a.get_id().as_str() == "id")
        .expect("task move missing --id");
    assert!(id.is_required_set(), "task move --id should be required");

    let column = mv
        .get_arguments()
        .find(|a| a.get_id().as_str() == "column")
        .expect("task move missing --column");
    assert!(
        column.is_required_set(),
        "task move --column should be required",
    );
}

#[test]
fn task_move_rejects_global_union_args() {
    let root = schema_driven_root();
    let mv = sub(sub(&root, "task", "noun"), "move", "verb");
    let args = arg_ids(mv);

    // `title` belongs to `task add`, not `task move`. Per-op scoping must keep
    // it off `task move` so the generated CLI does not accept the global union.
    assert!(
        !args.contains(&"title"),
        "task move should not accept --title (global-union leak); got {args:?}",
    );

    // Parsing `task move ... --title` must fail because the flag is undefined.
    let result = schema_driven_root().try_get_matches_from([
        "kanban", "task", "move", "--id", "01ABC", "--column", "doing", "--title", "nope",
    ]);
    assert!(
        result.is_err(),
        "task move should reject the undefined --title flag",
    );
}
