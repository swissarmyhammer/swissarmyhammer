# Fix rules_check Layering Violation - Bypass of todo_create Interface

## Problem

The `rules_check` MCP tool bypasses the `todo_create` tool interface and directly accesses `TodoStorage` implementation when `create_todo: true` parameter is used.

**Location**: `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs:197-207`

```rust
async fn create_todo_for_violation(violation: &RuleViolation) -> Result<TodoId, McpError> {
    // ... format task and context ...

    // WRONG: Direct storage access bypasses tool interface
    let storage = TodoStorage::new_default().map_err(...)?;
    let (todo_item, _gc_count) = storage
        .create_todo_item(task, Some(context))
        .await
        .map_err(...)?;

    Ok(todo_item.id)
}
```

## Why This Is Wrong

### 1. Layering Violation
- MCP tools should call other MCP tools through their public interface
- Bypassing the tool interface couples to implementation details
- Violates architectural principle: tools compose through interfaces, not implementations

### 2. Duplicates Logic
- `todo_create` tool already has parameter validation
- `todo_create` tool already has error handling
- `todo_create` tool already sends progress notifications
- rules_check reimplements todo creation logic

### 3. Inconsistent Behavior
- If `todo_create` tool behavior changes, rules_check won't reflect it
- Two different code paths for creating todos
- Different error messages and behaviors possible

### 4. Testing Implications
- Tests for `todo_create` don't cover usage from rules_check
- Have to test todo creation in rules_check separately
- Can't mock or intercept tool calls for testing

### 5. Tight Coupling
- rules_check depends on TodoStorage implementation
- Changes to storage layer affect rules_check
- Can't swap storage implementations

## Proposed Solution

Add tool-calling capability to ToolContext so tools can call other tools through their MCP interface.

### Step 1: Add Tool-Calling to ToolContext

```rust
impl ToolContext {
    /// Call another MCP tool from within a tool
    pub async fn call_tool(&self, name: &str, params: Value) -> Result<Value, McpError> {
        self.tool_registry.execute_tool(name, params, self).await
    }
}
```

### Step 2: Update create_todo_for_violation

```rust
async fn create_todo_for_violation(
    context: &ToolContext,  // Add context parameter
    violation: &RuleViolation
) -> Result<TodoId, McpError> {
    let task = format!("Fix {} violation in {}", ...);
    let context_str = format!("## Rule Violation\n...");

    // Call todo_create tool through interface
    let response = context.call_tool("todo_create", json!({
        "task": task,
        "context": context_str
    })).await?;

    // Parse the response to get todo_id
    let todo_id = response["todo_item"]["id"]
        .as_str()
        .ok_or(McpError::internal_error("No todo_id in response", None))?;

    Ok(TodoId::from_string(todo_id.to_string())?)
}
```

### Step 3: Update Caller

Update line 578 in execute() method:
```rust
// Before:
match create_todo_for_violation(violation).await {

// After:
match create_todo_for_violation(context, violation).await {
```

## Benefits

1. **Single code path** for todo creation across all tools
2. **Consistent behavior** - all todos created the same way
3. **Better testing** - test tool composition, not storage details
4. **Loose coupling** - tools depend on interfaces, not implementations
5. **Progress notifications** - todo_create's notifications automatically included
6. **Easier changes** - update todo_create once, affects all callers

## Acceptance Criteria

- ✅ ToolContext has `call_tool()` method
- ✅ `create_todo_for_violation()` calls `todo_create` tool instead of TodoStorage
- ✅ All existing tests pass
- ✅ New test verifies tool-to-tool calling works
- ✅ No direct TodoStorage usage in rules_check tool
- ✅ Build succeeds with no warnings
- ✅ Clippy passes

## Estimated Changes

~50-100 lines (add call_tool to ToolContext, update create_todo_for_violation, add tests)

## Notes

This establishes the architectural pattern that tools compose through MCP interfaces rather than bypassing to implementation layers. This pattern will be useful for other tool compositions going forward.
