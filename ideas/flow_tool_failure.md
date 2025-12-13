# Flow Tool Failure: result.content Missing

## Problem

The `test` and `review` workflows fail when executed via the MCP flow tool with:
```
Workflow execution failed: Expression evaluation failed: CEL execution failed:
Unable to execute expression 'result.content.contains("YES")' (No such key: content)
```

## Symptoms

### Works
- `cargo run -- test` - CLI execution works perfectly
- `cargo run -- flow test` - CLI calling flow tool works
- `cargo run -- hello-world` - Workflows without prompt actions work
- `cargo run -- test_direct_result` - Custom test workflow with prompts works

### Fails
- MCP flow tool call from Claude Code: `flow(flow_name="test")` - FAILS
- MCP flow tool call from Claude Code: `flow(flow_name="review")` - FAILS

## Key Evidence

1. **CLI Success Log**:
   ```
   2025-12-02T18:21:57.374876Z  INFO swissarmyhammer_workflow::actions: Found NO in result.content
   ```
   CLI can access `result.content` successfully.

2. **MCP Failure**:
   ```
   MCP error -32603: Workflow 'test' execution failed: Expression evaluation failed:
   CEL execution failed: Unable to execute expression 'result.content.contains("YES")'
   (No such key: content)
   ```

3. **Unit Test Confirms Expected Behavior**:
   ```rust
   // Test in swissarmyhammer-workflow/src/executor/result_cel_test.rs
   let agent_response = json!({
       "content": "YES",
       "metadata": null,
       "response_type": "Success"
   });
   context.insert("result".to_string(), agent_response);

   // This works - accessing result.content
   let expression = "result.content.contains(\"YES\")";
   assert!(result.is_ok());
   ```

## Code Paths

### PromptAction Execution (swissarmyhammer-workflow/src/actions.rs:672-692)

```rust
// Store result in context if variable name specified
if let Some(var_name) = &self.result_variable {
    // Convert AgentResponse to JSON Value for context storage
    let response_value = serde_json::to_value(&response).unwrap_or_default();
    context.insert(var_name.clone(), response_value);
}

// Always store in special last_action_result key
context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(true));
// Store the response content as a string for backward compatibility
context.insert(
    CLAUDE_RESPONSE_KEY.to_string(),
    Value::String(response.content.clone()),
);

// Convert AgentResponse back to Value for the Action trait compatibility
let response_value = serde_json::to_value(&response)
    .unwrap_or_else(|_| Value::String(response.content.clone()));

Ok(response_value)
```

**Expected**: `response_value` should be `{"content": "...", "metadata": null, "response_type": "Success"}`

### Context Storage (swissarmyhammer-workflow/src/executor/core.rs:653-685)

```rust
fn set_action_result_vars(
    &mut self,
    run: &mut WorkflowRun,
    success: bool,
    result_value: Value,
) {
    run.context.insert("success".to_string(), Value::Bool(success));
    run.context.insert("failure".to_string(), Value::Bool(!success));
    // ...
    run.context.insert("result".to_string(), result_value);  // <-- Stores AgentResponse as JSON
    run.context.insert(LAST_ACTION_RESULT_KEY.to_string(), Value::Bool(success));
}
```

### CEL Expression Evaluation (swissarmyhammer-workflow/src/executor/validation.rs:503-536)

```rust
// Add all context variables (including 'result' as an object if it exists)
for (key, value) in context {
    // Debug log for result variable
    if key == RESULT_VARIABLE_NAME {
        tracing::debug!(
            "Adding 'result' variable to CEL context: {:?}",
            value
        );
    }

    Self::add_json_variable_to_cel_context_static(&mut cel_context, key, value).map_err(
        |e| {
            ExecutorError::ExpressionError(format!(
                "CEL context error: Failed to add variable '{key}' ({e})"
            ))
        },
    )?;
}

// Add 'result' as text fallback only if not already added as an object
// This maintains backward compatibility for expressions that expect result as a string
if !context.contains_key(RESULT_VARIABLE_NAME) {
    tracing::debug!("'result' not in context, adding text fallback");
    let result_text = Self::extract_result_text_static(context);
    cel_context
        .add_variable(RESULT_VARIABLE_NAME, result_text)
        // ...
} else {
    tracing::debug!("'result' found in context, using object version");
}
```

**Fix Applied**: Changed order to add context variables BEFORE adding text fallback, so the object version (with `content` field) takes precedence.

### JSON to CEL Conversion (swissarmyhammer-workflow/src/executor/validation.rs:774-780)

```rust
Value::Object(obj) => {
    let mut cel_map = std::collections::HashMap::new();
    for (k, v) in obj {
        cel_map.insert(k.clone(), Self::json_to_cel_value(v)?);
    }
    Ok(cel_interpreter::Value::Map(cel_map.into()))
}
```

This should convert `{"content": "YES", ...}` to a CEL Map with accessible fields.

## Workflow Definitions

### Test Workflow (builtin/workflows/test.md)

```yaml
## States

stateDiagram-v2
    [*] --> start
    start --> are_tests_passing
    are_tests_passing --> loop
    loop --> done: result.content.contains("YES")  # <-- FAILS HERE
    loop --> test: default
    test --> are_tests_passing
    done --> [*]

## Actions

- start: log "Making tests pass"
- are_tests_passing: execute prompt "are_tests_passing"  # <-- Returns AgentResponse
- test: execute prompt "test"
- done: log "All tests passing!"
```

### Review Workflow (builtin/workflows/review.md)

```yaml
## States

stateDiagram-v2
    [*] --> start
    start --> are_rules_passing
    are_rules_passing --> loop
    loop --> done: result.content.contains("YES")  # <-- FAILS HERE
    loop --> fix: default
    fix --> test
    test --> are_rules_passing
    done --> [*]

## Actions

- start: log "Running code review with rules"
- are_rules_passing: execute prompt "are_rules_passing"  # <-- Returns AgentResponse
- fix: run workflow "do"
- test: run workflow "test"
- done: log "All rules passing!"
```

## Differences Between Paths

### CLI Path
1. User runs: `cargo run -- test`
2. CLI creates CliToolContext with its own MCP server (port assigned dynamically)
3. CLI calls flow tool via `cli_tool_context.execute_tool("flow", ...)`
4. Calls `server.execute_tool()` on CLI's MCP server
5. FlowTool executes workflow with CLI's MCP server context
6. PromptAction serializes AgentResponse to JSON with `content` field
7. CEL expression finds `result.content` ✓

### MCP Tool Path (Claude Code)
1. Claude calls: `flow(flow_name="test")`
2. Calls flow tool on "sah" MCP server (the server Claude is connected to)
3. FlowTool executes workflow with sah MCP server context
4. PromptAction serializes AgentResponse to JSON
5. CEL expression CANNOT find `result.content` ✗

## Investigation Questions

### Are both MCP servers configured identically?
- Both use `McpServer::new_with_work_dir()`
- Both register the same tools
- Both should have the same behavior

### Is there an agent configuration difference?
- CLI MCP server: created with `None` agent override (unless --agent flag)
- sah MCP server: created with agent override from command line

### Is the AgentResponse serialization different?
- AgentResponse struct in swissarmyhammer-agent-executor/src/response.rs:
  ```rust
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
  pub struct AgentResponse {
      pub content: String,
      pub metadata: Option<serde_json::Value>,
      pub response_type: AgentResponseType,
  }
  ```
- Serialization should always produce `{"content": "...", "metadata": null, "response_type": "Success"}`

### Is there a timing issue?
- No - the error is deterministic and immediate

### Is the context being cleared between states?
- No - context persists across state transitions
- The test shows `result` exists but doesn't have `.content` key

## Hypothesis

The most likely issue is that when AgentResponse is returned from PromptAction and stored via `set_action_result_vars()`, something in the conversion chain is stripping the object structure and converting it to a string.

Possible culprits:
1. `serde_json::to_value(&response)` is failing and falling back to string
2. The `unwrap_or_else` fallback is being hit
3. Context storage is somehow flattening the object
4. There's a different code path for MCP tool execution that I haven't traced

## Debug Steps Needed

1. Add tracing in `PromptAction::execute_once_internal` to log the exact `response_value` being returned
2. Add tracing in `set_action_result_vars` to log the exact `result_value` being stored
3. Add tracing in CEL evaluation to log the exact structure of `result` from context
4. Compare logs between CLI execution and MCP execution

## Workaround

Change workflow transitions from:
```
loop --> done: result.content.contains("YES")
```

To use "Store As" and explicit variable:
```yaml
- are_tests_passing: execute prompt "are_tests_passing"
  Store As: check_result
```

Then:
```
loop --> done: check_result.content.contains("YES")
```

This bypasses the `result` variable entirely.

## Test Results

- ✓ All 3007 unit/integration tests pass
- ✓ CLI workflows work (`cargo run -- test`)
- ✗ MCP flow tool workflows fail (calling flow tool via MCP)
- ✓ Simple workflows without prompts work via both paths
- ✓ CEL unit tests confirm object with content field works
