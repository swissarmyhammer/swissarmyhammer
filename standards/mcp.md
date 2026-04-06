# MCP Tool Standards

Standards and conventions for building MCP tools in SwissArmyHammer. Every new tool MUST follow these patterns. Derived from the shell and code_context reference implementations.

## File Structure

```
swissarmyhammer-tools/src/mcp/tools/{tool_name}/
├── mod.rs              # McpTool impl + register_{tool}_tools()
├── description.md      # Tool description (see "Tool Description" section)
├── state.rs            # State management (if needed)
├── doctor.rs           # Custom health checks (if needed, e.g. code_context)
└── execute/            # Operation modules (for complex tools)
    └── mod.rs
```

## Required Traits

Every tool must implement three traits:

### McpTool

```rust
#[async_trait]
impl McpTool for MyTool {
    // --- Required ---
    fn name(&self) -> &'static str { "my_tool" }
    fn description(&self) -> &'static str { include_str!("description.md") }
    fn schema(&self) -> Value { generate_mcp_schema(&MY_OPERATIONS, config) }
    async fn execute(&self, arguments: Map<String, Value>, context: &ToolContext) -> Result<CallToolResult, McpError>;

    // --- Required for CLI discovery ---
    fn cli_category(&self) -> Option<&'static str> { Some("my_tool") }
    fn cli_name(&self) -> &str { "my_tool" }

    // --- Required for operation-based tools ---
    fn operations(&self) -> &'static [&'static dyn Operation] { &MY_OPERATIONS }

    // --- Set explicitly when needed ---
    fn is_agent_tool(&self) -> bool { false }       // true = filtered when Claude Code detected
    fn hidden_from_cli(&self) -> bool { false }     // true = not in `sah tool` CLI
}
```

**`cli_category()` and `cli_name()`**: Always set these explicitly. The trait has defaults that extract from the tool name, but explicit values are clearer and prevent surprises when names change. If your tool name IS the category (e.g., "kanban", "shell", "ralph"), both return the same value.

**`is_agent_tool()`**: Set to `true` ONLY if your tool replicates capabilities the host agent already has natively (e.g., `files` duplicates Claude Code's Read/Write/Glob/Grep). Most tools should be `false` — they provide new capabilities the agent doesn't have.

### Doctorable

Health checks for `sah doctor`. Use the macro for tools with no external dependencies:

```rust
impl_empty_doctorable!(MyTool);
```

Or implement custom checks:

```rust
impl Doctorable for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn category(&self) -> &str { "tools" }
    fn is_applicable(&self) -> bool { true }
    fn run_health_checks(&self) -> Vec<HealthCheck> {
        // Check dependencies, directories, connections
    }
}
```

### Initializable

Lifecycle hooks. Use the macro for tools with no setup:

```rust
impl_empty_initializable!(MyTool);
```

Or implement for tools that need directory creation, etc:

```rust
impl Initializable for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn category(&self) -> &str { "tools" }
    fn init(&mut self) -> Result<()> {
        // Create directories, initialize state
        Ok(())
    }
}
```

## Operations (verb/noun pattern)

Tools use the `swissarmyhammer_operations::Operation` trait to define their operations. This is the primary pattern — all new tools should use it.

### Defining Operations

Each operation is a struct implementing `Operation`:

```rust
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

#[derive(Debug, Default)]
pub struct AddWidget;

static ADD_WIDGET_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("name")
        .description("Widget name")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("color")
        .description("Optional hex color (without #)")
        .param_type(ParamType::String),
];

impl Operation for AddWidget {
    fn verb(&self) -> &'static str { "add" }
    fn noun(&self) -> &'static str { "widget" }
    fn description(&self) -> &'static str { "Create a new widget" }
    fn parameters(&self) -> &'static [ParamMeta] { ADD_WIDGET_PARAMS }
}
```

### Collecting Operations

Operations are collected in a static vec for schema generation:

```rust
use once_cell::sync::Lazy;

static ADD_WIDGET_OP: Lazy<AddWidget> = Lazy::new(AddWidget::default);
static GET_WIDGET_OP: Lazy<GetWidget> = Lazy::new(GetWidget::default);
static LIST_WIDGETS_OP: Lazy<ListWidgets> = Lazy::new(ListWidgets::default);

static MY_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| vec![
    &*ADD_WIDGET_OP,
    &*GET_WIDGET_OP,
    &*LIST_WIDGETS_OP,
]);
```

### Exposing Operations from `operations()`

The `operations()` method must return `&'static [&'static dyn Operation]`. Since `OPERATIONS` is a `Lazy<Vec<...>>`, both shell and code_context use an unsafe transmute to extend the lifetime. This is safe because the `Lazy` is truly static, but the pattern must include a safety comment:

```rust
fn operations(&self) -> &'static [&'static dyn Operation] {
    let ops = MY_OPERATIONS.as_slice();
    // SAFETY: MY_OPERATIONS is a static Lazy initialized once and never dropped.
    // The references inside are also to static Lazy values with 'static lifetime.
    unsafe { std::mem::transmute(ops) }
}
```

This is the established pattern across all operation-based tools. Do not try to avoid it — the trait signature requires `'static` and `Lazy<Vec>` doesn't directly provide it.

### Standard Verbs

Use these verbs consistently across all tools:

| Verb | Meaning | Example |
|------|---------|---------|
| `add` | Create new entity | `add task`, `add widget` |
| `get` | Retrieve single entity by ID | `get task`, `get widget` |
| `list` | Retrieve multiple entities with filters | `list tasks`, `list widgets` |
| `update` | Modify existing entity | `update task`, `update widget` |
| `delete` | Remove entity | `delete task`, `delete widget` |
| `move` | Change position/column | `move task` |
| `complete` | Mark as done | `complete task` |
| `assign` | Add actor to entity | `assign task` |
| `search` | Semantic/fuzzy search | `search history` |
| `grep` | Pattern match search | `grep history` |
| `execute` | Run a command or expression | `execute command` |
| `set` | Set a value or state | `set ralph` |
| `clear` | Remove/reset state | `clear ralph`, `clear status` |
| `check` | Verify or inspect state | `check ralph` |
| `build` | Trigger a build/index process | `build status` |
| `find` | Discover entities by criteria | `find duplicates` |
| `query` | Structured query | `query ast` |
| `detect` | Auto-discover | `detect projects` |
| `kill` | Terminate a process | `kill process` |

**Extending verbs**: If none of the standard verbs fit, you may introduce a new verb. Prefer single-word verbs that clearly describe the action. Avoid domain-specific nouns as verbs (e.g., prefer `get status` with a filter parameter over `lsp status`).

### Nouns

- Singular for single-entity operations: `add task`, `get widget`
- The op string is always `"{verb} {noun}"` lowercase

## Schema Generation

Use `generate_mcp_schema()` — never build schemas by hand:

```rust
use swissarmyhammer_operations::{generate_mcp_schema, SchemaConfig};

fn schema(&self) -> Value {
    let config = SchemaConfig::new("My tool description for the LLM agent");
    generate_mcp_schema(&MY_OPERATIONS, config)
}
```

The generated schema:
- Flat `properties` — all params from all operations merged
- `op` enum field listing all operation strings
- `x-operation-schemas` — per-operation required fields and docs
- `x-operation-groups` — operations grouped by noun
- `additionalProperties: true` — forgiving input
- NO `oneOf`/`anyOf`/`allOf` (Claude API restriction)

### Supported Parameter Types

| ParamType | JSON Schema | Rust Type |
|-----------|-------------|-----------|
| `String` | `"string"` | `String` |
| `Integer` | `"integer"` | `i64`, `u32`, etc. |
| `Number` | `"number"` | `f64` |
| `Boolean` | `"boolean"` | `bool` |
| `Array` | `"array"` | `Vec<T>` |

**Not supported**: `object`, `null` — will fail CLI validation.

## Operation Dispatch

Match on the `op` field in `execute()`:

```rust
async fn execute(&self, arguments: Map<String, Value>, context: &ToolContext) -> Result<CallToolResult, McpError> {
    let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");
    let mut args = arguments.clone();
    args.remove("op");

    match op_str {
        "add widget" => { /* ... */ }
        "get widget" => { /* ... */ }
        "list widgets" => { /* ... */ }
        "" => {
            // Optional: infer operation from present keys
        }
        other => {
            return Err(McpError::invalid_params(
                format!("Unknown operation '{}'. Valid: add widget, get widget, list widgets", other),
                None,
            ));
        }
    }
}
```

### Default operation (optional)

If your tool has one primary operation, you may treat `""` (missing op) as that operation. The shell tool does this — missing `op` defaults to `"execute command"`:

```rust
"execute command" | "" => {
    // Default: execute a command
}
```

This makes the tool more forgiving for simple use cases where the agent omits the op field.

### Operation inference from keys (optional)

For maximum forgiveness, you can infer the operation from which keys are present when `op` is missing. The kanban tool does this — if `title` is present but `id` is not, it infers `"add task"`. Only do this if the inference is unambiguous.

## Response Helpers

Use `BaseToolImpl` for consistent responses:

```rust
// Success — text content
Ok(BaseToolImpl::create_success_response("Widget created successfully"))

// Success — JSON content (serialize to string first)
let json = serde_json::to_string_pretty(&result)?;
Ok(BaseToolImpl::create_success_response(json))

// Error — returned to the agent
Err(McpError::invalid_params("Widget name is required", None))
Err(McpError::internal_error("Database write failed", None))

// Error — with details
Ok(BaseToolImpl::create_error_response("Operation failed", Some(details)))
```

## Execution Results (Audit Trail)

The `ExecutionResult` enum exists for tools that need audit logging of mutations. The kanban tool uses this extensively. Shell and code_context do NOT use it — they return `CallToolResult` directly.

**Use `ExecutionResult` when**: your tool has a persistent store and you want per-entity audit logs (like kanban's per-task JSONL logs).

**Skip it when**: your tool is stateless, session-scoped, or doesn't need audit trails.

```rust
use swissarmyhammer_operations::ExecutionResult;

// Mutations — logged with audit trail
ExecutionResult::Logged { value: result, log_entry }

// Read-only queries — no logging
ExecutionResult::Unlogged { value: result }

// Failures — optional logging
ExecutionResult::Failed { error, log_entry: Some(entry) }
```

## Tool Description

Every tool needs a `description.md`. This is what the LLM agent sees.

### Loading descriptions

Two patterns exist in the codebase:

**Pattern A — compile-time inclusion** (preferred for static descriptions):
```rust
fn description(&self) -> &'static str { include_str!("description.md") }
```

**Pattern B — registry lookup** (used by shell, when description key differs from tool name):
```rust
fn description(&self) -> &'static str {
    get_tool_description("shell", "execute")
        .expect("Tool description should be available")
}
```

Use Pattern A unless you have a reason to use the registry.

### Description format

```markdown
# {tool_name}

{One-line summary of what this tool does.}

## Overview

{Brief description of the tool's purpose and when to use it.}

## Operations

The tool accepts `op` as a "verb noun" string (e.g., "add widget").

### Widget Operations

- `add widget` - Create a new widget
  - Required: `name`
  - Optional: `color`

- `get widget` - Get widget by ID
  - Required: `id`

- `list widgets` - List all widgets
  - Optional: `color` (filter)

## Examples

### Create a widget

```json
{
  "op": "add widget",
  "name": "My Widget",
  "color": "ff0000"
}
```
```

## Registration

### Per-tool registration function

```rust
// In tools/{tool_name}/mod.rs
pub fn register_my_tools(registry: &mut ToolRegistry) {
    registry.register(MyTool::new());
}
```

### Central registration

Add to `register_all_tools()` in `server.rs`:

```rust
register_my_tools(&mut tool_registry);
```

### Re-exports

Add to `swissarmyhammer-tools/src/mcp/mod.rs`:

```rust
pub use tools::my_tool::register_my_tools;
```

And to `swissarmyhammer-tools/src/lib.rs` if needed for external crate access.

## State Management

Choose based on your tool's needs:

| Pattern | When to use | Example tools |
|---------|-------------|---------------|
| Stateless (`#[derive(Default)]`) | Pure computation, no persistence | js, web |
| File-based (working dir) | Per-project persistent state | kanban (`.kanban/`), ralph (`.sah/ralph/`) |
| Shared libraries (`Arc<RwLock<T>>`) | Dynamic registries | agent, skill |
| In-process (`Arc<Mutex<T>>`) | Session-scoped ephemeral state | shell (command history) |
| Context access (`ToolContext`) | Shared infrastructure | git (via `context.git_ops`) |

### File-based state convention

- Store in `.sah/{tool_name}/` or `.{tool_name}/` in the working directory
- Create directory in `Initializable::init()`
- Check directory in `Doctorable::run_health_checks()`

## Error Handling

```rust
// Parameter validation
McpError::invalid_params("message", None)

// Internal failures
McpError::internal_error("message", None)

// Invalid request structure
McpError::invalid_request("message", None)
```

Always return descriptive error messages — the LLM agent reads them to recover.

## Testing

### Unit tests (in mod.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name() {
        let tool = MyTool::new();
        assert_eq!(tool.name(), "my_tool");
    }

    #[test]
    fn test_schema_has_operations() {
        let tool = MyTool::new();
        let schema = tool.schema();
        assert!(schema["properties"]["op"]["enum"].is_array());
    }

    #[test]
    fn test_operations_have_correct_verbs() {
        let tool = MyTool::new();
        let ops = tool.operations();
        let op_strings: Vec<String> = ops.iter().map(|o| o.op_string()).collect();
        assert!(op_strings.contains(&"add widget".to_string()));
    }
}
```

### Integration tests

Test the full `execute()` path with a real `ToolContext`:

```rust
#[tokio::test]
async fn test_add_widget_execution() {
    let tool = MyTool::new();
    let context = test_utils::create_test_context().await;
    let mut args = Map::new();
    args.insert("op".to_string(), json!("add widget"));
    args.insert("name".to_string(), json!("Test Widget"));

    let result = tool.execute(args, &context).await.unwrap();
    assert!(!result.is_error.unwrap_or(false));
}
```

## CLI Integration

Tools are automatically discovered by the CLI via `ToolRegistry`. The dynamic CLI builder reads:

- `cli_category()` — groups tool under `sah tool {category}`
- `operations()` — generates noun/verb subcommands: `sah tool {category} {noun} {verb}`
- `schema()` — generates CLI flags from parameter metadata

**Operation-based CLI structure:**
```
sah tool kanban task add --title "..."
sah tool kanban task list --column todo
sah tool ralph ralph set --instruction "..."
sah tool ralph ralph check --
```

**The `--` convention** (stdin piping): When `--` appears with no trailing args, the CLI executor reads stdin as JSON or YAML and merges fields into the tool's arguments. CLI flags override stdin on conflict. This enables piping data into any tool.

## Advanced Patterns

These patterns appear in the reference tools (shell, code_context) and should be used where applicable.

### Working directory and git root discovery

Many tools need the project root. Use `context.working_dir` from `ToolContext`. For git-aware tools, find the git root before operating:

```rust
let work_dir = context.working_dir.as_deref().unwrap_or(Path::new("."));
// code_context finds git root:
let git_root = find_git_root(work_dir).unwrap_or(work_dir.to_path_buf());
```

### Graceful degradation

When a tool depends on a background process (indexing, LSP, etc.), don't fail — return partial results with a status message. The code_context tool checks indexing readiness and appends a notice:

```rust
if let Some(progress_msg) = check_readiness(&workspace) {
    // Append progress notice to result, don't error
    result.push_str(&format!("\n\n{}", progress_msg));
}
```

### Background workers

For tools that need async processing (embedding computation, indexing), spawn a background worker at construction time. The shell tool does this for semantic search embeddings:

```rust
impl ShellState {
    pub fn new() -> Self {
        let (chunk_tx, chunk_rx) = mpsc::channel(100);
        let worker_handle = tokio::spawn(embedding_worker(chunk_rx, db.clone()));
        Self { chunk_tx, worker_handle, .. }
    }
}
```

Clean up in `Drop` or `Initializable::stop()`.

### Dynamic descriptions

If your tool's description needs to include runtime-discovered content (available agents, skills, etc.), build the description dynamically:

```rust
fn description(&self) -> &'static str {
    // For static parts, use include_str!
    // For dynamic parts, build at registration time and leak:
    let desc = format!("{}\n{}", STATIC_PREFIX, dynamic_content);
    Box::leak(desc.into_boxed_str())
}
```

The agent and skill tools do this to append `<available_agents>` / `<available_skills>` XML sections.

### Process management

For tools that spawn child processes (shell), implement:
- Timeout with graceful SIGTERM → forced SIGKILL escalation
- Process tracking by ID for `kill` operations
- Proper cleanup on drop (AsyncProcessGuard pattern)

### Output size limiting

For tools that may produce large output (shell, grep), implement output buffering with size limits:
- Binary detection (don't return binary content to the agent)
- Line-count limiting (`max_lines` parameter)
- Truncation markers so the agent knows output was cut

## Checklist for New Tools

- [ ] Create `tools/{name}/mod.rs` with `McpTool` impl
- [ ] Create `tools/{name}/description.md`
- [ ] Implement `Doctorable` (macro or custom)
- [ ] Implement `Initializable` (macro or custom)
- [ ] Define operations with `Operation` trait, `ParamMeta`, and static `Lazy` instances
- [ ] Generate schema via `generate_mcp_schema()`
- [ ] Implement `execute()` with `match op_str` dispatch
- [ ] Add `register_{name}_tools()` function
- [ ] Call from `register_all_tools()` in `server.rs`
- [ ] Add `pub mod {name};` to `tools/mod.rs`
- [ ] Add re-exports to `mcp/mod.rs` and `lib.rs`
- [ ] Set `cli_category()` and `cli_name()` explicitly
- [ ] Set `is_agent_tool()` if tool replicates native agent capabilities
- [ ] Write unit tests for name, schema, operations
- [ ] Write integration tests for execute path
