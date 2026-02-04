//! Test to verify MCP tools are properly registered and available
//!
//! This test validates that the MCP server actually registers tools and they are accessible

use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use swissarmyhammer_tools::mcp::tool_registry::{
    register_cel_tools, register_file_tools, register_kanban_tools, register_shell_tools,
    register_web_fetch_tools, register_web_search_tools,
};

/// Test that verifies all expected MCP tools are registered
#[tokio::test]
async fn test_mcp_tools_are_registered() {
    let mut registry = ToolRegistry::new();

    // This mirrors exactly what McpServer does in its constructor
    register_cel_tools(&mut registry);
    register_file_tools(&mut registry).await;
    register_shell_tools(&mut registry);
    register_kanban_tools(&mut registry);
    register_web_fetch_tools(&mut registry);
    register_web_search_tools(&mut registry);

    let tool_count = registry.len();
    println!("📊 Registered {} MCP tools", tool_count);

    // We should have a significant number of tools. The threshold of 10 is based on the
    // minimum set of core tools across all categories (files, shell, kanban, web, etc.).
    // This acts as a smoke test to catch missing tool registrations.
    assert!(
        tool_count >= 10,
        "Expected at least 10 tools, got {}. This suggests tools are not being registered properly.",
        tool_count
    );

    // Check for specific tools we know should be there (internal names, not MCP prefixed)
    let expected_tools = [
        "cel_set",
        "cel_get",
        "files_read",
        "files_write",
        "files_edit",
        "files_glob",
        "files_grep",
        "shell_execute",
        "kanban",
        "web_fetch",
        "web_search",
    ];

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
    register_cel_tools(&mut registry);
    register_file_tools(&mut registry).await;
    register_shell_tools(&mut registry);
    register_kanban_tools(&mut registry);
    register_web_fetch_tools(&mut registry);
    register_web_search_tools(&mut registry);

    let categories = registry.get_cli_categories();
    println!("📋 CLI Categories: {:?}", categories);

    // These categories should be available (excluding hidden tools like CEL and notify)
    let expected_categories = ["file", "kanban", "shell", "web-search"];

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
    register_cel_tools(&mut registry);
    register_file_tools(&mut registry).await;
    register_shell_tools(&mut registry);
    register_kanban_tools(&mut registry);
    register_web_fetch_tools(&mut registry);
    register_web_search_tools(&mut registry);

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

/// Test that verifies kanban tool schema has all 50 operations
#[tokio::test]
async fn test_kanban_schema_has_all_operations() {
    let mut registry = ToolRegistry::new();
    register_kanban_tools(&mut registry);

    let tools = registry.list_tools();
    let kanban_tool = tools
        .iter()
        .find(|t| t.name == "kanban")
        .expect("kanban tool should be registered");

    // Check op enum count
    let op_enum = &kanban_tool.input_schema["properties"]["op"]["enum"];
    let op_count = op_enum
        .as_array()
        .expect("op enum should be array")
        .len();

    assert_eq!(
        op_count, 50,
        "Expected 50 operations in op enum, got {}",
        op_count
    );

    // Check x-operation-schemas count
    let op_schemas = &kanban_tool.input_schema["x-operation-schemas"];
    let op_schemas_count = op_schemas
        .as_array()
        .expect("x-operation-schemas should be array")
        .len();

    assert_eq!(
        op_schemas_count, 50,
        "Expected 50 operation schemas, got {}",
        op_schemas_count
    );

    // Verify some expected operations are present
    let op_list = op_enum.as_array().unwrap();
    let expected_ops = [
        "init board",
        "add task",
        "assign task",
        "complete task",
        "add subtask",
        "add attachment",
        "list activity",
    ];

    for expected_op in &expected_ops {
        assert!(
            op_list
                .iter()
                .any(|v| v.as_str() == Some(expected_op)),
            "Expected operation '{}' not found in schema",
            expected_op
        );
    }

    println!("✅ Kanban schema has all 50 operations");
    println!(
        "   Including: add subtask, add attachment (newly added operations)"
    );
}
