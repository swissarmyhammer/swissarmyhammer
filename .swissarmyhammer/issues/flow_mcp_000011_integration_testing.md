# Step 11: Integration Testing

Refer to ideas/flow_mcp.md

## Objective

Create comprehensive integration tests for the complete flow MCP tool system.

## Context

With all components implemented, we need end-to-end integration tests to verify the system works correctly across MCP protocol, CLI shortcuts, notifications, and parameter handling.

## Tasks

### 1. MCP Protocol Integration Tests

Create `swissarmyhammer-tools/tests/flow_mcp_integration_test.rs`:

```rust
#[tokio::test]
async fn test_flow_tool_appears_in_list() {
    // Test flow tool is registered and appears in tools list
    let registry = create_test_registry();
    let tools = registry.list_tools();
    assert!(tools.iter().any(|t| t.name == "flow"));
}

#[tokio::test]
async fn test_flow_discovery_via_mcp() {
    // Test calling flow tool with flow_name="list"
    let result = call_mcp_tool(
        "flow",
        json!({
            "flow_name": "list",
            "format": "json",
            "verbose": true
        })
    ).await;
    
    assert!(result.is_ok());
    let response: WorkflowListResponse = serde_json::from_str(&result.unwrap())?;
    assert!(!response.workflows.is_empty());
}

#[tokio::test]
async fn test_flow_execution_via_mcp() {
    // Test executing a workflow via MCP
    let result = call_mcp_tool(
        "flow",
        json!({
            "flow_name": "test_workflow",
            "parameters": {
                "param1": "value1"
            },
            "dry_run": true
        })
    ).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_flow_missing_required_parameter() {
    // Test error when required parameter missing
    let result = call_mcp_tool(
        "flow",
        json!({
            "flow_name": "plan",
            "parameters": {}
        })
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("plan_filename"));
}
```

### 2. CLI Integration Tests

Create `swissarmyhammer-cli/tests/flow_cli_integration_test.rs`:

```rust
#[tokio::test]
async fn test_flow_command_no_run_subcommand() {
    // Test flow takes workflow name directly (NO "run" subcommand)
    let output = run_cli(&["flow", "plan", "test.md"]).await;
    assert!(output.status.success());
}

#[tokio::test]
async fn test_flow_list_special_case() {
    // Test flow list works
    let output = run_cli(&["flow", "list"]).await;
    assert!(output.status.success());
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("workflows"));
}

#[tokio::test]
async fn test_shortcut_execution() {
    // Test executing workflow via shortcut
    let output = run_cli(&["plan", "test.md"]).await;
    assert!(output.status.success());
}

#[tokio::test]
async fn test_shortcut_vs_full_form() {
    // Test shortcut and full form produce same result
    let shortcut = run_cli(&["plan", "test.md"]).await;
    let full_form = run_cli(&["flow", "plan", "test.md"]).await;
    
    assert_eq!(shortcut.status, full_form.status);
}

#[tokio::test]
async fn test_shortcut_name_conflict() {
    // Test workflow with conflicting name gets underscore
    let help = run_cli(&["--help"]).await;
    let help_text = String::from_utf8_lossy(&help.stdout);
    
    // If there's a workflow named "list", it should appear as "_list"
    if help_text.contains("_list") {
        let output = run_cli(&["_list"]).await;
        assert!(output.status.success());
    }
}

#[tokio::test]
async fn test_positional_args() {
    // Test positional args work
    let output = run_cli(&["flow", "plan", "spec.md"]).await;
    assert!(output.status.success());
}

#[tokio::test]
async fn test_optional_params() {
    // Test --param works
    let output = run_cli(&[
        "flow", "test_workflow",
        "--param", "key1=value1",
        "--param", "key2=value2"
    ]).await;
    assert!(output.status.success());
}
```

### 3. Notification Integration Tests

Create `swissarmyhammer-tools/tests/flow_notification_integration_test.rs`:

```rust
#[tokio::test]
async fn test_notifications_during_execution() {
    // Test notifications are sent during workflow execution
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = NotificationSender::new(tx);
    
    let context = ToolContext::with_notifications(
        PathBuf::from("/tmp"),
        sender,
    );
    
    // Execute workflow
    let _result = execute_workflow_via_tool(&context).await;
    
    // Collect notifications
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }
    
    // Verify notification sequence
    assert!(has_flow_start(&notifications));
    assert!(has_state_transitions(&notifications));
    assert!(has_flow_complete(&notifications));
}

#[tokio::test]
async fn test_error_notification() {
    // Test error notification sent on failure
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = NotificationSender::new(tx);
    
    let context = ToolContext::with_notifications(
        PathBuf::from("/tmp"),
        sender,
    );
    
    // Execute workflow that will fail
    let _result = execute_failing_workflow_via_tool(&context).await;
    
    // Verify error notification
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }
    
    assert!(has_flow_error(&notifications));
}
```

### 4. Parameter Migration Tests

Create `swissarmyhammer-cli/tests/parameter_migration_test.rs`:

```rust
#[tokio::test]
async fn test_var_to_param_migration() {
    // Test --var still works but shows warning
    let output = run_cli(&[
        "flow", "test_workflow",
        "--var", "key=value"
    ]).await;
    
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--var is deprecated"));
}

#[tokio::test]
async fn test_param_preferred() {
    // Test --param works without warning
    let output = run_cli(&[
        "flow", "test_workflow",
        "--param", "key=value"
    ]).await;
    
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("deprecated"));
}
```

### 5. End-to-End Workflow Test

```rust
#[tokio::test]
async fn test_complete_workflow_e2e() {
    // Complete end-to-end test:
    // 1. Discover workflows via MCP
    // 2. Execute workflow via CLI
    // 3. Verify notifications sent
    // 4. Verify workflow completes successfully
}
```

### 6. Test Helper Utilities

Create test helper functions:

```rust
async fn run_cli(args: &[&str]) -> std::process::Output {
    // Helper to run CLI and capture output
}

async fn call_mcp_tool(
    tool_name: &str,
    args: serde_json::Value,
) -> Result<String> {
    // Helper to call MCP tool and get response
}

fn has_flow_start(notifications: &[FlowNotification]) -> bool {
    // Helper to check for flow start notification
}

// ... other helper functions
```

## Files to Create

- `swissarmyhammer-tools/tests/flow_mcp_integration_test.rs`
- `swissarmyhammer-cli/tests/flow_cli_integration_test.rs`
- `swissarmyhammer-tools/tests/flow_notification_integration_test.rs`
- `swissarmyhammer-cli/tests/parameter_migration_test.rs`
- `swissarmyhammer-cli/tests/test_helpers.rs`

## Acceptance Criteria

- [ ] MCP protocol tests pass (discovery and execution)
- [ ] CLI tests confirm NO "run" subcommand exists
- [ ] `sah flow [workflow]` works (not `sah flow run [workflow]`)
- [ ] CLI shortcut tests pass
- [ ] Notification tests pass
- [ ] Parameter migration tests pass
- [ ] End-to-end test passes
- [ ] Name conflict resolution tested
- [ ] Error handling tested
- [ ] All tests pass consistently
- [ ] No flaky tests

## Estimated Changes

~400 lines of test code
