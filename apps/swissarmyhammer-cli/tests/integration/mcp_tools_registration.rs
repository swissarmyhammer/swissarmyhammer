//! Test to verify MCP tools are properly registered and available
//!
//! This test validates that the MCP server actually registers tools and they are accessible

use swissarmyhammer_operations::WIRE_DROPPED_KEYS;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use swissarmyhammer_tools::mcp::tool_registry::{
    create_fully_registered_tool_registry, register_file_tools, register_kanban_tools,
    register_shell_tools, register_web_tools,
};

/// Test that verifies all expected MCP tools are registered
#[tokio::test]
async fn test_mcp_tools_are_registered() {
    let mut registry = ToolRegistry::new();

    // This mirrors exactly what McpServer does in its constructor
    register_file_tools(&mut registry);
    register_shell_tools(&mut registry);
    register_kanban_tools(&mut registry);
    register_web_tools(&mut registry);

    let tool_count = registry.len();
    println!("📊 Registered {} MCP tools", tool_count);

    // We should have a significant number of tools. The threshold of 4 is based on the
    // minimum set of core tools across all categories (files, shell, kanban, web).
    // This acts as a smoke test to catch missing tool registrations.
    assert!(
        tool_count >= 4,
        "Expected at least 4 tools, got {}. This suggests tools are not being registered properly.",
        tool_count
    );

    // Check for specific tools we know should be there (internal names, not MCP prefixed)
    let expected_tools = ["files", "shell", "kanban", "web"];

    let mut missing_tools = Vec::new();
    let mut found_tools = Vec::new();

    for &expected_tool in &expected_tools {
        if let Some(tool) = registry.get_tool(expected_tool) {
            found_tools.push(expected_tool);
            println!(
                "✓ Found tool: {} - {}",
                expected_tool,
                tool.description()
                    .lines()
                    .next()
                    .unwrap_or("No description")
            );
        } else {
            missing_tools.push(expected_tool);
        }
    }

    if !missing_tools.is_empty() {
        let all_tool_names = registry.list_tool_names();
        println!("✗ Missing expected tools: {:?}", missing_tools);
        println!("📋 All registered tools: {:?}", all_tool_names);
        panic!("Expected tools are missing from registry");
    }

    println!("✓ All expected tools are registered");
    println!(
        "📊 Found {} out of {} expected core tools",
        found_tools.len(),
        expected_tools.len()
    );

    // Test that tools can be listed for MCP
    let mcp_tools = registry.list_tools();
    assert!(!mcp_tools.is_empty(), "MCP tools list should not be empty");

    println!(
        "✓ MCP tools list generation works ({} tools)",
        mcp_tools.len()
    );

    // Validate tool structure
    for (i, tool) in mcp_tools.iter().take(5).enumerate() {
        assert!(
            !tool.name.is_empty(),
            "Tool {} should have non-empty name",
            i
        );
        assert!(
            tool.description.is_some(),
            "Tool {} should have description",
            i
        );
        assert!(
            !tool.input_schema.is_empty(),
            "Tool {} should have input schema",
            i
        );
    }

    println!("✓ Tool structure validation passed");

    println!("🎉 SUCCESS: MCP tools are properly registered and available");
    println!("   This disproves the issue that 'sah serve does not actually appear to serve any MCP tools'");
    println!("   The tools ARE registered and would be served by the MCP server.");
}

/// Test CLI category mapping works
#[tokio::test]
async fn test_cli_categories_are_available() {
    let mut registry = ToolRegistry::new();

    // Register all tools
    register_file_tools(&mut registry);
    register_shell_tools(&mut registry);
    register_kanban_tools(&mut registry);
    register_web_tools(&mut registry);

    let categories = registry.get_cli_categories();
    println!("📋 CLI Categories: {:?}", categories);

    // These categories should be available (excluding hidden tools like JS and notify)
    let expected_categories = ["files", "kanban", "shell", "web"];

    for &expected_cat in &expected_categories {
        assert!(
            categories.contains(&expected_cat.to_string()),
            "Expected CLI category '{}' not found. Available: {:?}",
            expected_cat,
            categories
        );
    }

    println!("✓ All expected CLI categories are available");

    // This proves the dynamic CLI generation works
    for category in &categories {
        let tools_in_category = registry.get_tools_for_category(category);
        assert!(
            !tools_in_category.is_empty(),
            "Category '{}' should have at least one tool",
            category
        );

        println!(
            "📂 Category '{}': {} tools",
            category,
            tools_in_category.len()
        );
    }

    println!("🎯 VALIDATION: CLI categories and tool mapping work correctly");
    println!("   This explains why 'sah --help' shows tool categories - they come from MCP tools!");
}

/// Test that all tool schemas are compatible with Claude API
///
/// The Claude API does NOT support `oneOf`, `allOf`, or `anyOf` at the top level
/// of tool input schemas. This test ensures no tool accidentally uses these
/// unsupported constructs.
#[tokio::test]
async fn test_tool_schemas_are_claude_api_compatible() {
    let mut registry = ToolRegistry::new();

    // Register all tools
    register_file_tools(&mut registry);
    register_shell_tools(&mut registry);
    register_kanban_tools(&mut registry);
    register_web_tools(&mut registry);

    let unsupported_constructs = ["oneOf", "allOf", "anyOf"];
    let mut violations = Vec::new();

    for tool in registry.list_tools() {
        // input_schema is already Arc<Map<String, Value>>
        for construct in &unsupported_constructs {
            if tool.input_schema.contains_key(*construct) {
                violations.push(format!(
                    "Tool '{}' has unsupported '{}' at top level of input_schema",
                    tool.name, construct
                ));
            }
        }
    }

    if !violations.is_empty() {
        for violation in &violations {
            eprintln!("❌ SCHEMA VIOLATION: {}", violation);
        }
        panic!(
            "Claude API compatibility check failed!\n\
            The Claude API does NOT support oneOf/allOf/anyOf at the top level of tool schemas.\n\
            Found {} violation(s). Use runtime validation instead of schema-level oneOf/allOf/anyOf.\n\
            See: https://docs.anthropic.com/en/docs/build-with-claude/tool-use",
            violations.len()
        );
    }

    println!(
        "✅ All {} tool schemas are Claude API compatible",
        registry.len()
    );
    println!("   No oneOf/allOf/anyOf constructs found at top level");
}

/// The number of kanban operations.
///
/// Single source of truth for this test: the `KANBAN_OPERATIONS` table in
/// `crates/swissarmyhammer-kanban/src/schema.rs` (the wire op enum and the full
/// `x-operation-schemas` array both derive from it). Bump this when an op is
/// added or removed. The assertions below also cross-check the wire schema's op
/// enum against the full schema's `x-operation-schemas`, so a count change that
/// touches only one of the two surfaces fails the test rather than silently
/// disagreeing.
const EXPECTED_KANBAN_OP_COUNT: usize = 54;

/// Test that verifies kanban tool schema has all expected operations
#[tokio::test]
async fn test_kanban_schema_has_all_operations() {
    let mut registry = ToolRegistry::new();
    register_kanban_tools(&mut registry);

    let tools = registry.list_tools();
    let kanban_tool = tools
        .iter()
        .find(|t| t.name == "kanban")
        .expect("kanban tool should be registered");

    // The wire schema (advertised via `list_tools()`) keeps the op enum...
    let op_enum = &kanban_tool.input_schema["properties"]["op"]["enum"];
    let op_count = op_enum.as_array().expect("op enum should be array").len();

    assert_eq!(
        op_count, EXPECTED_KANBAN_OP_COUNT,
        "Expected {EXPECTED_KANBAN_OP_COUNT} operations in op enum, got {op_count}"
    );

    // ...but the wire schema OMITS the heavy `x-operation-schemas` key.
    assert!(
        kanban_tool
            .input_schema
            .get("x-operation-schemas")
            .is_none(),
        "wire schema must omit x-operation-schemas"
    );

    // The per-op `x-operation-schemas` array lives on the FULL schema, which the
    // in-process CLI command tree consumes via `McpTool::schema_full()`. It has
    // one entry per op, and must agree with the wire op enum.
    let full_schema = registry
        .get_tool("kanban")
        .expect("kanban tool should be registered")
        .schema_full();
    let op_schemas = &full_schema["x-operation-schemas"];
    let op_schemas_count = op_schemas
        .as_array()
        .expect("x-operation-schemas should be array")
        .len();

    assert_eq!(
        op_schemas_count, op_count,
        "full schema x-operation-schemas count ({op_schemas_count}) must match wire op enum count ({op_count})"
    );

    // Verify some expected operations are present
    let op_list = op_enum.as_array().unwrap();
    let expected_ops = [
        "init board",
        "add task",
        "assign task",
        "complete task",
        "archive task",
        "unarchive task",
        "list archived",
    ];

    for expected_op in &expected_ops {
        assert!(
            op_list.iter().any(|v| v.as_str() == Some(expected_op)),
            "Expected operation '{}' not found in schema",
            expected_op
        );
    }

    println!("✅ Kanban schema has all {EXPECTED_KANBAN_OP_COUNT} operations");
}

/// The full FULL-only schema keys every operation tool's `schema_full()` must
/// carry (and which the slim wire `schema()` must never leak).
///
/// These are the in-process CLI-generation keys the noun/verb command tree is
/// built from. They are a subset of [`WIRE_DROPPED_KEYS`] — the ones whose
/// *presence* on the full surface we positively assert.
const FULL_ONLY_KEYS: [&str; 3] = [
    "x-operation-schemas",
    "x-operation-groups",
    "x-op-signatures",
];

/// Workspace-wide guard for the wire/full schema split.
///
/// Builds the real, fully-registered tool registry (the same single source of
/// truth the MCP server uses) and, for every tool that exposes operations,
/// mechanically enforces the contract:
///
/// - `schema()` (the WIRE surface advertised over MCP `tools/list`) omits ALL of
///   [`swissarmyhammer_operations::WIRE_DROPPED_KEYS`] — the heavy CLI-facing keys
///   must never ship over the wire on every prompt.
/// - `schema_full()` (the in-process CLI surface) carries
///   `x-operation-schemas` + `x-operation-groups` + `x-op-signatures`.
///
/// This fails the moment a new operation tool forgets to override
/// `schema_full()` (so `schema()` would leak the full keys) — the gap that let
/// `review` ship the heavy schema over the wire. The dropped-key list is
/// imported from the operations crate rather than re-listed here, so adding a
/// key keeps this guard in lockstep automatically.
#[tokio::test]
async fn test_operation_tools_split_wire_and_full_schemas() {
    let registry = create_fully_registered_tool_registry().await;

    let mut checked = Vec::new();
    for tool in registry.iter_tools() {
        // Only operation-based tools participate in the wire/full split.
        if tool.operations().is_empty() {
            continue;
        }
        let name = swissarmyhammer_tools::mcp::tool_registry::McpTool::name(tool);
        checked.push(name);

        // WIRE schema must drop every heavy CLI-facing key.
        let wire = tool.schema();
        let wire_obj = wire
            .as_object()
            .unwrap_or_else(|| panic!("`{name}` wire schema must be a JSON object"));
        for key in WIRE_DROPPED_KEYS {
            assert!(
                !wire_obj.contains_key(key),
                "`{name}` wire schema (schema()) must omit full-only key {key:?}; \
                 it likely returns the FULL schema — override schema_full() and have \
                 schema() return generate_mcp_schema_wire(...)",
            );
        }

        // FULL schema must carry the CLI-generation keys.
        let full = tool.schema_full();
        for key in FULL_ONLY_KEYS {
            assert!(
                full.get(key).is_some(),
                "`{name}` full schema (schema_full()) must contain {key:?}",
            );
        }
        assert!(
            full["x-operation-schemas"].is_array(),
            "`{name}` full schema x-operation-schemas must be an array",
        );
        assert!(
            full["x-operation-groups"].is_object(),
            "`{name}` full schema x-operation-groups must be an object",
        );
        assert!(
            full["x-op-signatures"].is_object(),
            "`{name}` full schema x-op-signatures must be an object",
        );
    }

    assert!(
        !checked.is_empty(),
        "expected at least one operation-based tool in the registry",
    );
    assert!(
        checked.contains(&"review"),
        "the `review` tool must be covered by the wire/full guard; got {checked:?}",
    );
    println!("✅ wire/full split holds for operation tools: {checked:?}");
}
