# Fix mcp.log Truncation on Subprocess Spawn

## Problem

The `.swissarmyhammer/mcp.log` file is being truncated (all contents deleted) every time a subprocess is spawned, such as when ClaudeCode agent is invoked for rule checking. This causes loss of valuable debugging information and makes it difficult to track operations across multiple agent invocations.

## Root Cause

**File**: `swissarmyhammer-tools/src/mcp/unified_server.rs:152`

```rust
match std::fs::File::create(&log_file_path) {
    Ok(file) => {
        // ... setup logging
    }
}
```

### Issue with `File::create()`

`std::fs::File::create()` has these semantics:
- Creates a new file if it doesn't exist
- **TRUNCATES (deletes all content) if the file already exists**

### When Truncation Occurs

1. **Per-Process Guard**: There's a `Once` guard (`MCP_LOGGING_INIT`) at line 103 that ensures logging is configured only once **per process**

2. **Subprocess Spawning**: When ClaudeCode executor spawns a subprocess:
   - File: `swissarmyhammer-agent-executor/src/claude/executor.rs:77`
   - Each subprocess is a **new process** with fresh memory
   - The `Once` guard hasn't run in this new process yet
   - `configure_mcp_logging()` executes
   - `File::create()` truncates `mcp.log`
   - All previous log contents are lost

3. **Frequency**: Log truncation happens:
   - Every time a ClaudeCode subprocess is spawned for rule checking
   - Every time any agent executor creates a subprocess
   - On main process startup (first time only, due to `Once` guard)

## Impact

- **Lost Debugging Information**: Cannot trace operations across multiple agent invocations
- **Difficult Troubleshooting**: Previous errors and context disappear when new subprocess starts
- **Workflow Debugging**: Review workflow with rules checking creates many subprocesses, making logs nearly useless
- **Production Monitoring**: Cannot maintain continuous logs for production debugging

## Proposed Solution

Replace `File::create()` with `OpenOptions` configured for append mode.

### Code Change

**File**: `swissarmyhammer-tools/src/mcp/unified_server.rs:152`

**Current (WRONG)**:
```rust
match std::fs::File::create(&log_file_path) {
    Ok(file) => {
        let shared_file = Arc::new(Mutex::new(file));
        // ...
    }
}
```

**Fixed (CORRECT)**:
```rust
match std::fs::OpenOptions::new()
    .create(true)      // Create file if it doesn't exist
    .append(true)      // Append to file if it does exist (preserves content)
    .open(&log_file_path) 
{
    Ok(file) => {
        let shared_file = Arc::new(Mutex::new(file));
        // ...
    }
}
```

### Benefits of This Fix

1. **Preserves Logs**: Previous log entries remain when new subprocess starts
2. **Full Trace**: Can see complete execution history across all processes
3. **Chronological Order**: Logs from multiple processes append in order
4. **No Breaking Changes**: Same API, just different file opening semantics

## Implementation Notes

### Imports Needed

Add at the top of the file:
```rust
use std::fs::OpenOptions;
```

### File Locking Considerations

The current code already uses `Arc<Mutex<File>>` for thread-safe writing within a process. Multiple processes writing to the same file with append mode is safe on UNIX systems because:
- Append mode uses atomic writes at the OS level
- Each write operation is independent
- No coordination needed between processes

### Testing Strategy

1. **Unit Test**: Verify file is created in append mode
2. **Integration Test**: 
   - Write to log
   - Spawn subprocess that also logs
   - Verify both sets of logs are present
3. **Manual Test**:
   - Run `sah serve`
   - Trigger rule checking (spawns ClaudeCode subprocess)
   - Verify mcp.log contains logs from both main process and subprocess

## Testing Requirements

### Unit Test

```rust
#[test]
fn test_mcp_log_append_mode() {
    // Create a temp directory
    let temp_dir = tempfile::TempDir::new().unwrap();
    let log_path = temp_dir.path().join("mcp.log");
    
    // Write some initial content
    std::fs::write(&log_path, "Initial log entry\n").unwrap();
    
    // Simulate what configure_mcp_logging does
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();
    
    use std::io::Write;
    writeln!(&file, "Second log entry").unwrap();
    
    // Verify both entries are present
    let contents = std::fs::read_to_string(&log_path).unwrap();
    assert!(contents.contains("Initial log entry"));
    assert!(contents.contains("Second log entry"));
}
```

### Integration Test

Create a test that:
1. Starts MCP server
2. Invokes a rule check (which spawns subprocess)
3. Verifies mcp.log contains logs from both processes

## Alternative Solutions Considered

### 1. Single Log File Per Process
**Approach**: Use PID in filename: `mcp-{pid}.log`
**Rejected**: 
- Would create many log files
- Harder to trace execution across processes
- Need cleanup mechanism for old files

### 2. Log Rotation
**Approach**: Rotate logs when they get too large
**Rejected**: 
- Doesn't solve the truncation problem
- Adds complexity
- Can be added later if needed

### 3. Shared Logger Process
**Approach**: Central logging process that all subprocesses connect to
**Rejected**: 
- Significant architectural change
- Adds complexity and failure modes
- Overkill for the problem

## Related Issues

This fix will significantly improve:
- Debugging the review workflow (which spawns many subprocesses)
- Troubleshooting rule checking failures
- Understanding agent behavior in complex scenarios

## Success Criteria

After this fix:
1. `mcp.log` accumulates logs from all processes
2. No log truncation when subprocesses spawn
3. Logs remain in chronological order
4. No performance degradation
5. All existing tests still pass

## Risk Assessment

**Risk Level**: Low

**Mitigations**:
- Very small, localized change (one line)
- Append mode is standard practice for log files
- No breaking changes to API or behavior
- Easy to verify correctness through testing

## Files to Modify

1. `swissarmyhammer-tools/src/mcp/unified_server.rs` - Change File::create to OpenOptions with append mode

## Files to Create

1. Test for append mode behavior (can be added to existing test file)
