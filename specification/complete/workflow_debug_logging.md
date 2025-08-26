# Workflow Debug Logging Specification

## Overview

Enhance the SwissArmyHammer workflow execution system to capture all tracing output to a persistent NDJSON log file when the global `--debug` flag is enabled. This will provide developers with complete debug logs for workflow troubleshooting and analysis.

## Current State

When running `sah --debug flow run <workflow>`, the system:
- Saves workflow run metadata to `.swissarmyhammer/workflow-runs/runs/WorkflowRunId(<the_id>)/run.json`
- Outputs detailed tracing logs to the console (Level::DEBUG when --debug flag is used)
- Uses existing tracing infrastructure with tracing::info!, tracing::debug!, tracing::warn!, tracing::error! calls
- Stores run state in memory during execution

### Existing Infrastructure to Leverage

**DO NOT RE-IMPLEMENT** these components that already exist:

1. **CLI Debug Flag**: Global `--debug` flag already exists (swissarmyhammer-cli/src/cli.rs:82-84)
2. **Logging System**: Sophisticated tracing setup in main.rs:92-119 with registry-based layer composition
3. **File Writing**: Thread-safe `FileWriterGuard` with immediate flush/sync (swissarmyhammer-cli/src/logging.rs:34-69)
4. **File Management**: Directory creation and path utilities in `FileSystemWorkflowRunStorage` (swissarmyhammer/src/workflow/storage.rs:565-574)
5. **Workflow Storage**: Existing `.swissarmyhammer/workflow-runs/runs/WorkflowRunId(<id>)/` structure
6. **MCP Logging**: Already logs to `.swissarmyhammer/mcp.log` (completed in issue 000178)
7. **Tracing Infrastructure**: Full tracing-subscriber setup with EnvFilter and fmt layers

## Proposed Enhancement

When the global `--debug` flag is passed, capture all tracing output that would normally go to the terminal and save it to a persistent NDJSON log file alongside the `run.json` file that gets created for each workflow run.

### New Behavior

When `sah --debug flow run <workflow>` is executed:

1. **Create debug log file**: Save an NDJSON file at `.swissarmyhammer/workflow-runs/runs/WorkflowRunId(<the_id>)/run_logs.ndjson`
2. **Capture all tracing output**: Duplicate all tracing logs that appear on the terminal to the log file
3. **Use existing log format**: Convert existing tracing output to structured NDJSON format
4. **Maintain existing functionality**: Terminal logging continues to work as before

### Log Entry Format

Each line in `run_logs.ndjson` should be a JSON object representing a tracing event:

```json
{
  "timestamp": "2025-01-22T10:30:45.123Z",
  "level": "DEBUG|INFO|WARN|ERROR|TRACE",
  "target": "swissarmyhammer_cli::flow",
  "message": "🚀 Starting workflow: plan",
  "fields": {
    "workflow_name": "plan",
    "run_id": "01HXAMPLE123456789"
  },
  "span": {
    "name": "execute_workflow",
    "level": "INFO"
  }
}
```

### Events to Capture

Capture all existing tracing output including:

1. **Workflow execution logs** (from flow.rs:262 and similar)
   - "🚀 Starting workflow"
   - "✅ Workflow completed successfully"
   - "❌ Workflow failed"
   - State transition logs

2. **Action execution logs** (from workflow actions)
   - Action start/completion messages
   - Execution results and errors

3. **Debug information** (existing tracing::debug! calls)
   - File system events
   - Internal state changes
   - Variable updates

4. **Error and warning logs**
   - Exception handling
   - Recovery attempts
   - Warning conditions

## Implementation Requirements

### Integration with Existing CLI

- Use the existing global `--debug` flag (already defined in cli.rs:82-84)
- No changes needed to FlowSubcommand::Run structure
- Leverage existing logging initialization in main.rs:54-55

### File Management

- Create debug log file only when global `--debug` flag is present
- Use NDJSON format (one JSON object per line)
- Store alongside existing `run.json` in the same directory structure
- Use existing FileSystemWorkflowRunStorage path structure

### Logging Integration

- Hook into existing tracing subscriber setup in main.rs
- Add a secondary writer that outputs to the NDJSON file
- Maintain existing console output behavior
- Use existing tracing filter levels (Level::DEBUG when --debug is enabled)

### Performance Considerations

- Minimal overhead when --debug flag is not used
- Asynchronous file writing where possible
- Leverage existing dashmap caching in FileSystemWorkflowRunStorage
- Avoid blocking workflow execution for log writes

### Error Handling

- Debug logging failures must not affect workflow execution
- Log debug logging errors to existing tracing system
- Graceful degradation if log file cannot be created

## Usage Examples

### Basic Debug Logging
```bash
sah --debug flow run plan
# Creates: .swissarmyhammer/workflow-runs/runs/WorkflowRunId(01HXAMPLE)/run_logs.ndjson
```

### Debug with Workflow Parameters
```bash
sah --debug flow run plan --var plan_filename="./spec.md"
# Debug logs include variable assignments and template rendering
```

### Debug in Test Mode
```bash
sah --debug flow run plan --test
# Debug logs include test mode execution details
```

## File Structure

After implementation, the workflow run directory structure will be:

```
.swissarmyhammer/workflow-runs/runs/WorkflowRunId(01HXAMPLE123456789)/
├── run.json          # Existing workflow run metadata
└── run_logs.ndjson   # New debug logs (only if --debug used)
```

## Benefits

1. **Complete Debug History**: All tracing output captured for post-execution analysis
2. **Existing Tooling Compatibility**: Leverages current tracing infrastructure
3. **Zero Learning Curve**: Uses familiar --debug flag and existing log messages
4. **Structured Data**: NDJSON format enables easy parsing and analysis
5. **Performance Optimized**: Minimal impact when debug logging is disabled

## Technical Implementation

### Required Changes (Minimal - Leverage Existing Infrastructure)

This should be a **2-3 issue implementation** using existing components:

1. **Add NDJSON Layer to main.rs** (5-10 lines):
   - Add conditional layer to existing registry() setup in main.rs:92-119
   - Use existing FileWriterGuard for thread-safe file writing
   - Only activate when `cli.debug && matches!(cli.command, Commands::Flow{..})`

2. **Pass WorkflowRunId to Logging Context**:
   - Modify flow.rs to add run_id to tracing span fields
   - Use existing WorkflowRun.id from current execution
   - No global state needed - use tracing span context

3. **Basic Integration Testing**:
   - Verify NDJSON file creation alongside existing run.json
   - Test with existing flow commands (run, resume, test)

### File Path Resolution

- **USE EXISTING**: `FileSystemWorkflowRunStorage.run_dir()` method (storage.rs:572-574)
- **USE EXISTING**: Directory creation patterns from workflow storage
- **CREATE**: `run_logs.ndjson` using existing file path structure

### Implementation Simplicity

**DO NOT**:
- Create new debug_logging.rs module 
- Implement custom file management or cleanup
- Add complex configuration structs
- Create global workflow run context
- Modify FlowSubcommand structure
- Add file rotation or size limits

**DO**:
- Add one NDJSON layer to existing tracing registry
- Reuse FileWriterGuard for consistent file operations
- Use existing WorkflowRunId from current execution scope
- Leverage existing directory and error handling patterns

## Success Criteria

1. All tracing output visible in terminal is also captured to NDJSON file when `--debug` flag is used
2. Debug log files are created in correct location using existing storage structure  
3. Log entries are valid NDJSON with timestamp, level, target, message, and fields
4. Zero performance impact when --debug flag is not used
5. Existing workflow execution behavior is unchanged
6. Error handling prevents logging issues from affecting workflow execution
7. Works with all flow subcommands (run, resume, test modes)

## Future Considerations

- Log analysis tools for NDJSON format
- Integration with log aggregation systems
- Configurable log retention policies
- Performance metrics extraction from logs