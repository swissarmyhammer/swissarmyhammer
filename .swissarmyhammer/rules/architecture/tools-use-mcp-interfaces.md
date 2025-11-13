---
severity: error
tags: ["architecture", "layering", "coupling"]
---

Check {{ language }} code in MCP tools for direct usage of other tools' storage implementations instead of calling through MCP interfaces.

**This rule only applies to files in**: `swissarmyhammer-tools/src/mcp/tools/`

Look for:

**Violations (calling implementations directly):**
- Importing storage crates: `use swissarmyhammer_todo::TodoStorage`, `use swissarmyhammer_issues::IssueStorage`, `use swissarmyhammer_memoranda::MemoStorage`
- Direct instantiation: `TodoStorage::new_default()`, `IssueStorage::new()`, `MemoStorage::new()`
- Calling storage methods directly from tool code
- Bypassing the MCP tool interface to access implementation layers

**Correct patterns (calling through interfaces):**
- Using `context.call_tool()` to invoke another MCP tool
- Calling tool registry methods
- Using shared utilities that don't bypass tool interfaces
- Tools in the same domain can share implementation (e.g., rules/check and rules/validate both using RuleChecker)

**Exceptions allowed:**
- Tools can use their OWN domain's implementation (rules tools can use RuleChecker, RuleStorage, etc.)
- Shared utilities in swissarmyhammer-common
- Test files (tools can test storage directly in tests)

**Examples of violations:**

```rust
// BAD: rules_check calling TodoStorage directly
async fn create_todo_for_violation(violation: &RuleViolation) -> Result<TodoId> {
    let storage = TodoStorage::new_default()?;  // VIOLATION
    let todo = storage.create_todo_item(task, context).await?;
    Ok(todo.id)
}
```

**Examples of correct usage:**

```rust
// GOOD: rules_check calling todo_create tool
async fn create_todo_for_violation(
    context: &ToolContext,
    violation: &RuleViolation
) -> Result<TodoId> {
    let response = context.call_tool("todo_create", json!({
        "task": task,
        "context": context_str
    })).await?;

    let todo_id = response["todo_item"]["id"].as_str()?;
    Ok(TodoId::from_string(todo_id.to_string())?)
}
```

## Why This Matters

- **Loose coupling**: Tools depend on interfaces, not implementations
- **Single code path**: Changes to a tool affect all callers consistently
- **Testability**: Can mock/intercept tool calls
- **Composability**: Tools compose like microservices through APIs
- **Consistency**: All tool calls have same behavior (validation, notifications, error handling)

Report violations with:
- Which tool is violating the rule
- What storage implementation it's calling directly
- Which MCP tool it should call instead
- File path and line number
