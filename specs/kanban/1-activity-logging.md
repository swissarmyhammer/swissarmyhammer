# Activity Logging Implementation

## Status: Not Implemented

## Problem

The spec describes dual logging (global activity + per-task logs), and `KanbanContext` has `append_activity()` and `append_task_log()` methods, but **no operations actually call them**.

This means:
- No audit trail of changes
- Cannot derive timestamps (created_at, updated_at, completed_at)
- Cannot see who made changes
- Cannot replay history
- Activity log is always empty

## Solution: OperationProcessor Handles Execution + Logging

### Architecture

```
┌─────────────┐
│  MCP Tool   │  Thin adapter - parses input, delegates to processor
└──────┬──────┘
       │
       ▼
┌──────────────────────┐
│ OperationProcessor   │  Executes operations and handles logging
│  (kanban crate)      │  - Calls operation.execute()
└──────┬───────────────┘  - Extracts log entry
       │                  - Writes to global + per-task logs
       ▼
┌─────────────┐
│ Operations  │  Individual commands (AddTask, MoveTask, etc.)
│             │  Return ExecutionResult (Logged/Unlogged/Failed)
└─────────────┘
```

**MCP tool is just an adapter** - it doesn't know about logging, just delegates.

**Processor handles the orchestration** - testable, reusable from CLI/API/MCP.

### OperationProcessor Trait

Define a generic trait in the operations crate:

```rust
// In swissarmyhammer-operations/src/processor.rs

#[async_trait]
pub trait OperationProcessor<C, E>
where
    C: Send + Sync,
    E: Send + Sync + std::fmt::Display,
{
    /// Execute an operation and handle any logging
    ///
    /// This is the main entry point - it:
    /// 1. Calls operation.execute(ctx)
    /// 2. Extracts the log entry (if any)
    /// 3. Writes logs to appropriate locations
    /// 4. Returns the final result
    async fn process<T>(
        &self,
        operation: &T,
        ctx: &C,
    ) -> Result<Value, E>
    where
        T: Execute<C, E> + Send + Sync;

    /// Write a log entry to persistent storage
    ///
    /// Implementations decide where logs go (files, DB, etc.)
    async fn write_log(
        &self,
        ctx: &C,
        log_entry: &LogEntry,
        affected_resources: &[String],
    ) -> Result<(), E>;
}
```

### KanbanOperationProcessor

Implement the processor in the kanban crate:

```rust
// In swissarmyhammer-kanban/src/processor.rs

use crate::{KanbanContext, KanbanError, Result};
use swissarmyhammer_operations::{Execute, ExecutionResult, LogEntry, OperationProcessor};
use async_trait::async_trait;
use serde_json::Value;

/// Kanban-specific operation processor
///
/// Handles execution and logging for all kanban operations.
/// - Executes operations via Execute trait
/// - Writes logs to global activity log
/// - Writes logs to per-task logs for affected tasks
pub struct KanbanOperationProcessor {
    /// Optional actor performing operations (for log attribution)
    pub actor: Option<String>,
}

impl KanbanOperationProcessor {
    pub fn new() -> Self {
        Self { actor: None }
    }

    pub fn with_actor(actor: impl Into<String>) -> Self {
        Self {
            actor: Some(actor.into()),
        }
    }
}

#[async_trait]
impl OperationProcessor<KanbanContext, KanbanError> for KanbanOperationProcessor {
    async fn process<T>(
        &self,
        operation: &T,
        ctx: &KanbanContext,
    ) -> Result<Value>
    where
        T: Execute<KanbanContext, KanbanError> + Send + Sync,
    {
        // Execute the operation
        let exec_result = operation.execute(ctx).await;

        // Split into result and log entry
        let (result, mut log_entry) = exec_result.split();

        // Write log if present
        if let Some(ref mut entry) = log_entry {
            // Add actor attribution
            if let Some(ref actor) = self.actor {
                entry.actor = Some(actor.clone());
            }

            // Write logs
            if let Ok(ref value) = result {
                let affected = operation.affected_resource_ids(value);
                self.write_log(ctx, entry, &affected).await?;
            } else {
                // Still log errors
                self.write_log(ctx, entry, &[]).await?;
            }
        }

        result
    }

    async fn write_log(
        &self,
        ctx: &KanbanContext,
        log_entry: &LogEntry,
        affected_resources: &[String],
    ) -> Result<()> {
        use crate::types::TaskId;

        // Global activity log (all operations)
        ctx.append_activity(log_entry).await?;

        // Per-task logs (for operations that affect specific tasks)
        for resource_id in affected_resources {
            let task_id = TaskId::from_string(resource_id);
            ctx.append_task_log(&task_id, log_entry).await?;
        }

        Ok(())
    }
}

impl Default for KanbanOperationProcessor {
    fn default() -> Self {
        Self::new()
    }
}
```

### MCP Tool Uses Processor

The MCP tool becomes a thin adapter:

```rust
// In swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs

use swissarmyhammer_kanban::KanbanOperationProcessor;

#[async_trait]
impl McpTool for KanbanTool {
    async fn execute(
        &self,
        params: serde_json::Map<String, Value>,
        ctx: &ToolContext,
    ) -> CallToolResult {
        let kanban_ctx = self.get_context(ctx)?;

        // Parse operation from MCP params
        let op = parse_input(&params)
            .map_err(|e| McpError::invalid_params(e, None))?;

        // Create processor with actor from operation context
        let processor = match &op.actor {
            Some(actor) => KanbanOperationProcessor::with_actor(actor.to_string()),
            None => KanbanOperationProcessor::new(),
        };

        // Dispatch to the appropriate command
        let result = match (op.verb, op.noun) {
            (Verb::Add, Noun::Task) => {
                let cmd = AddTask::new(op.require_string("title")?);
                processor.process(&cmd, &kanban_ctx).await
            }
            (Verb::Get, Noun::Task) => {
                let cmd = GetTask::new(op.require_string("id")?);
                processor.process(&cmd, &kanban_ctx).await
            }
            // ... all other operations
        };

        // Convert to MCP result
        match result {
            Ok(value) => CallToolResult::success(value),
            Err(e) => CallToolResult::error(e.to_string()),
        }
    }
}
```

### ExecutionResult Enum

```rust
// In swissarmyhammer-operations/src/execution_result.rs

use serde_json::Value;
use crate::LogEntry;

pub enum ExecutionResult<T, E> {
    /// Operation succeeded and should be logged
    Logged {
        value: T,
        log_entry: LogEntry,
    },
    /// Operation succeeded but no logging needed (read-only)
    Unlogged {
        value: T,
    },
    /// Operation failed
    Failed {
        error: E,
        log_entry: Option<LogEntry>,  // Some ops log failures
    },
}

impl<T, E> ExecutionResult<T, E> {
    /// Extract the result (Ok or Err)
    pub fn into_result(self) -> Result<T, E> {
        match self {
            Self::Logged { value, .. } => Ok(value),
            Self::Unlogged { value } => Ok(value),
            Self::Failed { error, .. } => Err(error),
        }
    }

    /// Get the value and log entry separately
    pub fn split(self) -> (Result<T, E>, Option<LogEntry>) {
        match self {
            Self::Logged { value, log_entry } => (Ok(value), Some(log_entry)),
            Self::Unlogged { value } => (Ok(value), None),
            Self::Failed { error, log_entry } => (Err(error), log_entry),
        }
    }

    /// Check if this should be logged
    pub fn should_log(&self) -> bool {
        matches!(self, Self::Logged { .. } | Self::Failed { log_entry: Some(_), .. })
    }
}
```

### LogEntry Type

```rust
// In swissarmyhammer-operations/src/log.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A log entry recording an operation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique ID for this log entry (ULID format)
    pub id: String,

    /// When the operation occurred
    pub timestamp: DateTime<Utc>,

    /// Canonical op string (e.g., "add task", "move task")
    pub op: String,

    /// The normalized input parameters (as JSON)
    pub input: Value,

    /// The result value or error (as JSON)
    pub output: Value,

    /// Who performed the operation (optional)
    /// Format: "user_id" or "agent_name[session_id]"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,

    /// How long the operation took (milliseconds)
    pub duration_ms: u64,
}

impl LogEntry {
    pub fn new(
        op: impl Into<String>,
        input: Value,
        output: Value,
        actor: Option<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            timestamp: Utc::now(),
            op: op.into(),
            input,
            output,
            actor,
            duration_ms,
        }
    }

    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }
}
```

### Updated Execute Trait

```rust
// In swissarmyhammer-operations/src/operation.rs

#[async_trait]
pub trait Execute<C, E>: Operation
where
    C: Send + Sync,
{
    /// Execute the operation and return result + logging intent
    ///
    /// Returns ExecutionResult which indicates:
    /// - Logged: Mutation operations that should be audited
    /// - Unlogged: Read-only operations with no side effects
    /// - Failed: Errors (optionally logged)
    async fn execute(&self, ctx: &C) -> ExecutionResult<Value, E>;

    /// Extract affected resource IDs for targeted logging
    ///
    /// Used for per-resource logs (e.g., per-task logs in kanban).
    /// Default returns empty (most operations don't affect specific resources).
    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        Vec::new()
    }
}
```

## Why This Architecture Is Better

1. **Separation of concerns**:
   - MCP tool: Parse MCP protocol, delegate to processor
   - Processor: Execute operations, handle logging
   - Operations: Domain logic only

2. **Testable**:
   - Test operations in isolation (return ExecutionResult)
   - Test processor with mock operations
   - Test MCP tool with mock processor
   - No need for MCP infrastructure to test logging

3. **Reusable**:
   - CLI can use the same processor
   - API can use the same processor
   - Any interface can use the processor

4. **Compiler-enforced**:
   - All 40+ operations break when Execute trait changes
   - Forces update to every operation
   - No operations can be missed

## Testing Strategy

### Test Operations Independently

```rust
// In swissarmyhammer-kanban/src/task/add.rs

#[tokio::test]
async fn test_add_task_returns_logged_result() {
    let ctx = setup_context().await;

    let cmd = AddTask::new("Test");
    let result = cmd.execute(&ctx).await;

    match result {
        ExecutionResult::Logged { value, log_entry } => {
            assert_eq!(log_entry.op, "add task");
            assert_eq!(log_entry.input["title"], "Test");
            assert_eq!(value["title"], "Test");
        }
        _ => panic!("Expected Logged result"),
    }
}
```

### Test Processor Independently

```rust
// In swissarmyhammer-kanban/src/processor.rs (tests)

#[tokio::test]
async fn test_processor_writes_activity_log() {
    let ctx = setup_context().await;
    let processor = KanbanOperationProcessor::new();

    let cmd = AddTask::new("Test");
    processor.process(&cmd, &ctx).await.unwrap();

    // Verify log was written
    let entries = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].op, "add task");
}

#[tokio::test]
async fn test_processor_writes_per_task_log() {
    let ctx = setup_context().await;
    let processor = KanbanOperationProcessor::new();

    // Add task
    let cmd = AddTask::new("Test");
    let result = processor.process(&cmd, &ctx).await.unwrap();
    let task_id = result["id"].as_str().unwrap();

    // Update task
    let cmd = UpdateTask::new(task_id).with_title("Updated");
    processor.process(&cmd, &ctx).await.unwrap();

    // Check per-task log
    let task_log_path = ctx.task_log_path(&TaskId::from_string(task_id));
    let content = std::fs::read_to_string(task_log_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 2);  // add + update
}

#[tokio::test]
async fn test_processor_with_actor() {
    let ctx = setup_context().await;
    let processor = KanbanOperationProcessor::with_actor("assistant");

    let cmd = AddTask::new("Test");
    processor.process(&cmd, &ctx).await.unwrap();

    // Verify actor is in log
    let entries = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries[0].actor, Some("assistant".to_string()));
}

#[tokio::test]
async fn test_processor_unlogged_operations() {
    let ctx = setup_context().await;
    let processor = KanbanOperationProcessor::new();

    // Add a task (logged)
    let add_cmd = AddTask::new("Test");
    let result = processor.process(&add_cmd, &ctx).await.unwrap();
    let task_id = result["id"].as_str().unwrap();

    // Get task (unlogged)
    let get_cmd = GetTask::new(task_id);
    processor.process(&get_cmd, &ctx).await.unwrap();

    // Activity log should only have add, not get
    let entries = ctx.read_activity(None).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].op, "add task");
}
```

### Test MCP Tool as Thin Adapter

```rust
// In swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs (tests)

#[tokio::test]
async fn test_mcp_tool_delegates_to_processor() {
    let temp = TempDir::new().unwrap();
    let context = create_test_context().await
        .with_working_dir(temp.path().to_path_buf());
    let tool = KanbanTool::new();

    init_test_board(&tool, &context).await;

    // Execute via MCP
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!("add task"));
    args.insert("title".to_string(), json!("Test"));

    let result = tool.execute(args, &context).await.unwrap();
    let task_id = extract_task_id(&result);

    // Verify the task was created
    assert!(task_id.len() > 0);

    // Verify the log was written (processor did this, not MCP tool)
    let kanban_ctx = KanbanContext::find(temp.path()).unwrap();
    let entries = kanban_ctx.read_activity(None).await.unwrap();
    assert_eq!(entries.len(), 1);
}
```

## Implementation

### 1. Operations Return ExecutionResult

```rust
// Read operation
#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match ctx.read_task(&self.id).await {
            Ok(task) => {
                let value = serde_json::to_value(&task).unwrap();
                ExecutionResult::Unlogged { value }
            }
            Err(e) => ExecutionResult::Failed {
                error: e,
                log_entry: None,
            },
        }
    }
}

// Mutation operation
#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        // Do the work
        let board = ctx.read_board().await.unwrap();
        let position = /* ... */;
        let task = Task::new(&self.title, position);

        if let Err(e) = ctx.write_task(&task).await {
            let duration_ms = start.elapsed().as_millis() as u64;
            return ExecutionResult::Failed {
                error: e,
                log_entry: Some(LogEntry::new(
                    self.op_string(),
                    input,
                    serde_json::json!({"error": e.to_string()}),
                    None,
                    duration_ms,
                )),
            };
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let value = serde_json::to_value(&task).unwrap();

        ExecutionResult::Logged {
            value: value.clone(),
            log_entry: LogEntry::new(
                self.op_string(),
                input,
                value,
                None,
                duration_ms,
            ),
        }
    }

    fn affected_resource_ids(&self, result: &Value) -> Vec<String> {
        result.get("id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
    }
}
```

### 2. KanbanOperationProcessor Orchestrates

```rust
// Usage in kanban crate
let processor = KanbanOperationProcessor::with_actor("assistant");

let cmd = AddTask::new("Implement feature");
let result = processor.process(&cmd, &ctx).await?;

// Logs are already written!
```

### 3. MCP Tool Delegates

```rust
async fn dispatch_operation(
    op: KanbanOperation,
    ctx: &KanbanContext,
) -> Result<Value, KanbanError> {
    // Create processor with actor
    let processor = match op.actor {
        Some(actor) => KanbanOperationProcessor::with_actor(actor.to_string()),
        None => KanbanOperationProcessor::new(),
    };

    // Dispatch to command and let processor handle execution + logging
    match (op.verb, op.noun) {
        (Verb::Add, Noun::Task) => {
            let cmd = AddTask::new(op.require_string("title")?);
            processor.process(&cmd, ctx).await
        }
        (Verb::Get, Noun::Task) => {
            let cmd = GetTask::new(op.require_string("id")?);
            processor.process(&cmd, ctx).await
        }
        // ... all operations delegate to processor
    }
}
```

## File Structure

### Operations Crate

**File:** `swissarmyhammer-operations/src/execution_result.rs` (new)
```rust
pub enum ExecutionResult<T, E> {
    Logged { value: T, log_entry: LogEntry },
    Unlogged { value: T },
    Failed { error: E, log_entry: Option<LogEntry> },
}
```

**File:** `swissarmyhammer-operations/src/log.rs` (new)
```rust
pub struct LogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub op: String,
    pub input: Value,
    pub output: Value,
    pub actor: Option<String>,
    pub duration_ms: u64,
}
```

**File:** `swissarmyhammer-operations/src/processor.rs` (new)
```rust
#[async_trait]
pub trait OperationProcessor<C, E> {
    async fn process<T>(&self, operation: &T, ctx: &C) -> Result<Value, E>
    where
        T: Execute<C, E> + Send + Sync;

    async fn write_log(
        &self,
        ctx: &C,
        log_entry: &LogEntry,
        affected_resources: &[String],
    ) -> Result<(), E>;
}
```

**File:** `swissarmyhammer-operations/src/operation.rs`
```rust
#[async_trait]
pub trait Execute<C, E>: Operation {
    async fn execute(&self, ctx: &C) -> ExecutionResult<Value, E>;

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        Vec::new()
    }
}
```

**File:** `swissarmyhammer-operations/src/lib.rs`
```rust
mod execution_result;
mod log;
mod operation;
mod processor;

pub use execution_result::ExecutionResult;
pub use log::LogEntry;
pub use operation::{Execute, Operation};
pub use processor::OperationProcessor;
```

### Kanban Crate

**File:** `swissarmyhammer-kanban/src/processor.rs` (new)
```rust
pub struct KanbanOperationProcessor {
    pub actor: Option<String>,
}

impl OperationProcessor<KanbanContext, KanbanError> for KanbanOperationProcessor {
    async fn process<T>(&self, operation: &T, ctx: &KanbanContext) -> Result<Value>
    where
        T: Execute<KanbanContext, KanbanError> + Send + Sync
    {
        // Execute operation
        let exec_result = operation.execute(ctx).await;
        let (result, log_entry) = exec_result.split();

        // Write logs
        if let Some(mut entry) = log_entry {
            if let Some(ref actor) = self.actor {
                entry.actor = Some(actor.clone());
            }

            if let Ok(ref value) = result {
                let affected = operation.affected_resource_ids(value);
                self.write_log(ctx, &entry, &affected).await?;
            } else {
                self.write_log(ctx, &entry, &[]).await?;
            }
        }

        result
    }

    async fn write_log(
        &self,
        ctx: &KanbanContext,
        log_entry: &LogEntry,
        affected_resources: &[String],
    ) -> Result<()> {
        // Global activity log
        ctx.append_activity(log_entry).await?;

        // Per-task logs
        for resource_id in affected_resources {
            let task_id = TaskId::from_string(resource_id);
            ctx.append_task_log(&task_id, log_entry).await?;
        }

        Ok(())
    }
}
```

**File:** `swissarmyhammer-kanban/src/lib.rs`
```rust
mod processor;
pub use processor::KanbanOperationProcessor;
```

**File:** All 40+ operation files
- Update execute() to return `ExecutionResult`
- Override `affected_resource_ids()` for task operations

**File:** `swissarmyhammer-kanban/src/types/log.rs`
- Remove LogEntry (moved to operations crate)
- Re-export: `pub use swissarmyhammer_operations::LogEntry;`

### MCP Tool

**File:** `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`
- Import `KanbanOperationProcessor`
- Create processor per request (with actor from op context)
- Delegate all execution to `processor.process(&cmd, &ctx)`
- Remove any logging logic

## Migration Checklist

### Phase 1: Update operations crate (BREAKING)

- [ ] Add `execution_result.rs`
- [ ] Add `log.rs`
- [ ] Add `processor.rs` with trait
- [ ] Update `Execute` trait return type
- [ ] Export all new types

**Result**: All 40 operations fail to compile ✅

### Phase 2: Add processor to kanban crate

- [ ] Create `swissarmyhammer-kanban/src/processor.rs`
- [ ] Implement `OperationProcessor` for `KanbanOperationProcessor`
- [ ] Add tests for processor

### Phase 3: Update all 40 operations

For each operation file:
- [ ] Change `execute() -> Result<Value>` to `execute() -> ExecutionResult<Value, E>`
- [ ] Return `Logged`, `Unlogged`, or `Failed`
- [ ] Override `affected_resource_ids()` for task operations
- [ ] Update tests to handle `ExecutionResult`

### Phase 4: Update MCP tool

- [ ] Import `KanbanOperationProcessor`
- [ ] Replace execution logic with `processor.process(&cmd, &ctx)`
- [ ] Remove logging code from MCP layer
- [ ] Update tests

## Benefits

1. **Clean architecture** - MCP tool is a thin adapter
2. **Testable** - Can test processor without MCP infrastructure
3. **Reusable** - CLI/API can use same processor
4. **Compiler-enforced** - All 40 operations break until updated
5. **Proper separation** - Logging logic lives with domain logic

## Operations That Return Logged (24)

- InitBoard, UpdateBoard
- AddColumn, UpdateColumn, DeleteColumn
- AddSwimlane, UpdateSwimlane, DeleteSwimlane
- AddActor, UpdateActor, DeleteActor
- AddTask, UpdateTask, MoveTask, DeleteTask, CompleteTask, AssignTask
- AddTag, UpdateTag, DeleteTag, TagTask, UntagTask
- AddComment, UpdateComment, DeleteComment

## Operations That Return Unlogged (16)

- GetBoard
- GetColumn, ListColumns
- GetSwimlane, ListSwimlanes
- GetActor, ListActors
- GetTask, ListTasks, NextTask
- GetTag, ListTags
- GetComment, ListComments
- ListActivity

## Summary

**Key Architecture**:
- **Operations** return `ExecutionResult` (Logged/Unlogged/Failed)
- **OperationProcessor** trait defines execution + logging contract
- **KanbanOperationProcessor** implements processor for kanban domain
- **MCP tool** is a thin adapter that delegates to processor

**Compiler enforces** updating all 40+ operations.

**Clean separation** allows testing execution and logging independently of MCP.
