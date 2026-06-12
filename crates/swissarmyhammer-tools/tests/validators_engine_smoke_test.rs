//! Smoke test proving `swissarmyhammer-tools` can depend on the pluggable review
//! engine crate (`swissarmyhammer-validators`) and drive its hook-free engine API.
//!
//! This is the dependency-direction proof for the local-review system: the MCP
//! tools layer pulls in the engine and calls the standalone, hook-free
//! `match_rules(file_path)` surface with no hook or ACP-hook arguments.

use swissarmyhammer_validators::match_rules;

/// `match_rules("foo.rs")` resolves the builtin source-code validators.
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
    let matched = match_rules("foo.rs").expect("loading and matching rules should succeed");

    let names: Vec<&str> = matched.iter().map(|rs| rs.name()).collect();
    assert!(
        names.contains(&"duplication"),
        "a Rust source file must match the builtin source-code validators; got: {names:?}"
    );
}
