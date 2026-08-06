//! Smoke test proving `swissarmyhammer-tools` can depend on the pluggable review
//! engine crate (`swissarmyhammer-validators`) and drive its hook-free engine API.
//!
//! This is the dependency-direction proof for the local-review system: the MCP
//! tools layer pulls in the engine and calls the standalone, hook-free
//! `match_rules(file_path, workspace_root)` surface with no hook or ACP-hook
//! arguments.

use std::path::Path;

use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_validators::match_rules;
use tempfile::TempDir;

/// `match_rules("foo.rs", …)` resolves the builtin source-code validators.
///
/// `foo.rs` is a Rust source file, which the builtin `duplication` validator
/// selects via its `@file_groups/source_code` match criteria. The focused
/// review-time validators are file-triggered (no tool match), so they resolve
/// from a file path alone — the hook-free path this engine API exists to
/// express. Calling the engine from the tools crate must surface that match,
/// confirming both the dependency edge and the standalone file-path matching
/// surface.
#[test]
fn match_rules_selects_source_code_ruleset_for_rust_file() {
    let matched = match_rules("foo.rs", None).expect("loading and matching rules should succeed");

    let names: Vec<&str> = matched.iter().map(|rs| rs.name()).collect();
    assert!(
        names.contains(&"duplication"),
        "a Rust source file must match the builtin source-code validators; got: {names:?}"
    );
}

/// The name of the `project_types: [rust]` keyed RuleSet the workspace test
/// seeds.
const PROJECT_TYPE_RULESET_NAME: &str = "rust-workspace-rules";

/// Write a RuleSet under `base/<name>/` whose match criteria pin it to a file
/// glob AND to the `rust` workspace project type — the shape every tool rule
/// uses.
fn write_project_type_scoped_ruleset(base: &Path, name: &str) {
    let dir = base.join(name);
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    std::fs::write(
        dir.join("VALIDATOR.md"),
        format!(
            "---\nname: {name}\ndescription: {name} ruleset\nmatch:\n  files:\n    - \"**/*.rs\"\n  project_types:\n    - rust\n---\n\n# {name}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("rules/check.md"),
        "---\nname: check\ndescription: Check\n---\n\nCheck the code.\n",
    )
    .unwrap();
}

/// The hook surface pairs a `project_types`-keyed RuleSet with a file only when
/// it is given the workspace root whose detected types the key names.
///
/// A `None` root keeps the fail-closed behavior: no root resolves no project
/// types, so the keyed set does not match.
#[test]
#[serial_test::serial(cwd)]
fn match_rules_selects_a_project_types_keyed_ruleset_in_a_matching_workspace() {
    let _home = IsolatedTestEnvironment::new().expect("isolated env");
    let project = TempDir::new().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    std::fs::write(
        project.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    write_project_type_scoped_ruleset(
        &project.path().join(".validators"),
        PROJECT_TYPE_RULESET_NAME,
    );
    let _cwd = CurrentDirGuard::new(project.path()).expect("chdir");

    let matched_names = |root: Option<&Path>| -> Vec<String> {
        match_rules("src/lib.rs", root)
            .expect("loading and matching rules should succeed")
            .iter()
            .map(|rs| rs.name().to_string())
            .collect()
    };

    let in_workspace = matched_names(Some(project.path()));
    assert!(
        in_workspace.contains(&PROJECT_TYPE_RULESET_NAME.to_string()),
        "the rust workspace root must select the `project_types: [rust]` keyed set; got: {in_workspace:?}"
    );

    let without_root = matched_names(None);
    assert!(
        !without_root.contains(&PROJECT_TYPE_RULESET_NAME.to_string()),
        "no workspace root must fail closed on a `project_types`-keyed set; got: {without_root:?}"
    );
}
