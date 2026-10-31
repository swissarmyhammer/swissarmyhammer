//! Enable and disable behaviour and deterministic tool listing order.

use super::fixtures::{MockTool, ReplacementMockTool};
use crate::mcp::tool_registry::*;

// --- disabled_tools tests ---

#[test]
fn test_default_all_enabled() {
    // Fresh registry — no tools should be disabled
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });
    assert!(registry.is_tool_enabled("git"));
    assert!(registry.disabled_tools().is_empty());
    assert_eq!(registry.list_tools().len(), 1);
}

#[test]
fn test_disabled_tool_excluded_from_list_tools() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });

    registry.set_tool_enabled("git", false);

    assert!(!registry.is_tool_enabled("git"));
    assert!(registry.list_tools().is_empty());
    assert!(registry.list_tool_names().is_empty());
    assert_eq!(registry.iter_tools().count(), 0);
}

/// `tools.yaml` disabling must also remove the tool from the generated
/// `sah tool ...` command tree, not just from lookup.
///
/// The CLI builds its subcommands from `get_cli_categories` and
/// `get_tools_for_category`. When those ignored `disabled_tools`, a
/// disabled tool stayed in `sah tool --help` while `get_tool` refused it,
/// so the CLI advertised a command whose only possible outcome was
/// "Tool 'x' not found" — with `x` listed among the available tools.
#[test]
fn test_disabled_tool_excluded_from_cli_command_tree() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });

    // Enabled: it is a CLI command.
    assert!(registry.get_cli_categories().contains(&"git".to_string()));
    assert_eq!(registry.get_tools_for_category("git").len(), 1);
    assert_eq!(registry.get_cli_tools().len(), 1);

    registry.set_tool_enabled("git", false);

    // Disabled: the command must be gone, matching `get_tool`.
    assert!(
        registry.get_cli_categories().is_empty(),
        "a disabled tool must not contribute a `sah tool` category"
    );
    assert!(
        registry.get_tools_for_category("git").is_empty(),
        "a disabled tool must not appear under its category"
    );
    assert!(
        registry.get_cli_tools().is_empty(),
        "a disabled tool must not be listed as a CLI tool"
    );
    assert!(
        registry.get_tool("git").is_none(),
        "guards the premise: lookup already refuses a disabled tool"
    );
}

#[test]
fn test_disabled_tool_blocked_from_get_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });

    assert!(registry.get_tool("git").is_some());
    registry.set_tool_enabled("git", false);
    assert!(registry.get_tool("git").is_none());

    // Re-enabling should restore access
    registry.set_tool_enabled("git", true);
    assert!(registry.get_tool("git").is_some());
}

#[test]
fn test_set_all_disabled_then_enable_one() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });
    registry.register(MockTool {
        name: "kanban",
        description: "kanban tool",
    });

    // Disable everything
    registry.set_all_enabled(false);
    assert!(registry.list_tools().is_empty());

    // Re-enable only git
    registry.set_tool_enabled("git", true);

    let names = registry.list_tool_names();
    assert!(names.contains(&"git".to_string()));
    assert!(!names.contains(&"kanban".to_string()));
    assert_eq!(registry.list_tools().len(), 1);
}

#[test]
fn test_list_tools_always_load_meta() {
    // Every tool returned by list_tools() must have _meta with
    // "anthropic/alwaysLoad": true so Claude Code loads them eagerly.
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "alpha",
        description: "First tool",
    });
    registry.register(MockTool {
        name: "beta",
        description: "Second tool",
    });

    let tools = registry.list_tools();
    assert!(!tools.is_empty(), "expected at least one tool");

    for tool in &tools {
        let meta = tool
            .meta
            .as_ref()
            .unwrap_or_else(|| panic!("tool '{}' is missing _meta", tool.name));
        let always_load = meta
            .0
            .get("anthropic/alwaysLoad")
            .unwrap_or_else(|| panic!("tool '{}' _meta missing 'anthropic/alwaysLoad'", tool.name));
        assert_eq!(
            always_load,
            &serde_json::Value::Bool(true),
            "tool '{}' has wrong value for 'anthropic/alwaysLoad'",
            tool.name
        );
    }
}

#[test]
fn test_list_tools_deterministic_sorted_order() {
    // The rendered tools/list must be byte-identical across sibling
    // sessions so the KV prefix-reuse donor's LCP stays full. The registry
    // is HashMap-backed, so the emission point must sort by tool name to be
    // deterministic. Register in deliberately non-sorted insertion order;
    // assert the emitted order is sorted by name and stable across calls.
    //
    // Asserts exact ordered equality (not set-equality) so it fails under
    // HashMap iteration order / hash randomization.
    let names_in = [
        "kanban",
        "git",
        "web",
        "code_context",
        "review",
        "ralph",
        "skill",
        "agent",
    ];
    let mut registry = ToolRegistry::new();
    for name in names_in {
        registry.register(MockTool {
            name,
            description: "tool",
        });
    }

    let mut expected = names_in.to_vec();
    expected.sort_unstable();

    let first: Vec<String> = registry
        .list_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    let second: Vec<String> = registry
        .list_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();

    // Two successive calls return an identical ordered sequence.
    assert_eq!(
        first, second,
        "successive list_tools() calls returned different orders"
    );
    // And the order is the expected stable ordering (sorted by name).
    assert_eq!(
        first, expected,
        "list_tools() order is not sorted by tool name"
    );

    // list_tools_filtered's other entry point (host composition) must be
    // sorted too — share the same emission guarantee.
    let for_host: Vec<String> = registry
        .list_tools_for_host(crate::mcp::host::Host::Claude)
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    assert_eq!(
        for_host, expected,
        "list_tools_for_host() order is not sorted by tool name"
    );
}

/// A `Replacement`-category tool supersedes a native host tool, so the primary
/// per-client serve advertises it to Claude (which gets `Shared` +
/// `Replacement`) but not to unknown clients (which get `Shared` only).
#[test]
fn replacement_served_to_claude_not_unknown() {
    use crate::mcp::host::Host;

    let mut registry = ToolRegistry::new();
    registry.register(ReplacementMockTool);

    let names_for = |host: Host| -> Vec<String> {
        registry
            .list_tools_for_host(host)
            .iter()
            .map(|t| t.name.to_string())
            .collect()
    };

    assert!(
        names_for(Host::Claude)
            .iter()
            .any(|n| n == "replacement_mock"),
        "Claude must be served the `Replacement` tool"
    );
    assert!(
        !names_for(Host::Other)
            .iter()
            .any(|n| n == "replacement_mock"),
        "unknown clients must not be served a `Replacement` tool via the primary serve"
    );
}

// --- set_all_enabled(true) clears all ---

#[test]
fn test_set_all_enabled_true_clears_disabled() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "tool_a",
        description: "A",
    });
    registry.register(MockTool {
        name: "tool_b",
        description: "B",
    });

    registry.set_all_enabled(false);
    assert_eq!(registry.list_tool_names().len(), 0);

    registry.set_all_enabled(true);
    assert_eq!(registry.list_tool_names().len(), 2);
    assert!(registry.disabled_tools().is_empty());
}

// --- iter_tools respects disabled ---

#[test]
fn test_iter_tools_respects_disabled() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "keep_me",
        description: "kept",
    });
    registry.register(MockTool {
        name: "hide_me",
        description: "hidden",
    });

    registry.set_tool_enabled("hide_me", false);
    let names: Vec<&str> = registry
        .iter_tools()
        .map(<dyn McpTool as McpTool>::name)
        .collect();
    assert!(names.contains(&"keep_me"));
    assert!(!names.contains(&"hide_me"));
}

// --- CLI lookup respects disabled ---

#[test]
fn test_disabled_tool_not_returned_by_cli_name_lookup() {
    let mut registry = ToolRegistry::new();
    registry.register(MockTool {
        name: "git",
        description: "git tool",
    });

    assert!(registry
        .get_tool_by_cli_name(CliCategory("git"), CliName("git"))
        .is_some());

    registry.set_tool_enabled("git", false);

    assert!(
        registry
            .get_tool_by_cli_name(CliCategory("git"), CliName("git"))
            .is_none(),
        "a disabled tool must not be found by its CLI name, matching `get_tool`"
    );
}
