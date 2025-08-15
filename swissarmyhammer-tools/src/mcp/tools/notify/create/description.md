Send a notification message from LLM to user through the logging system. Enables LLMs to communicate important information, status updates, and contextual feedback during workflow execution.

## Parameters

- `message` (required): The message to notify the user about
- `level` (optional): The notification level - "info", "warn", or "error" (default: "info") 
- `context` (optional): Optional structured JSON data for the notification

## Examples

Send a basic info notification:
```json
{
  "message": "Processing large codebase - this may take a few minutes"
}
```

Send a warning notification:
```json
{
  "message": "Found potential security vulnerability in authentication logic",
  "level": "warn"
}
```

Send an error notification with context:
```json
{
  "message": "Database migration detected - recommend backing up data before proceeding", 
  "level": "error",
  "context": {
    "stage": "analysis",
    "safety": "critical"
  }
}
```

Send notification with structured context:
```json
{
  "message": "Completed analysis of codebase",
  "level": "info", 
  "context": {
    "files_processed": 47,
    "issues_found": 3,
    "time_elapsed": "2.5s"
  }
}
```

## Behavior

- Messages are logged through the tracing system with target "llm_notify"
- The notification level determines the log level used (info, warn, error)
- Invalid levels default to "info" level
- Context data is included as structured logging data
- Messages appear immediately in CLI output for real-time feedback
- Empty messages are rejected with validation error

## Returns

Returns confirmation message indicating the notification was sent successfully.

## Use Cases

### Status Updates
```
"Starting code analysis - scanning 150 files"
"Phase 1 complete - moving to refactoring stage"
```

### Discovery Notifications  
```
"Found existing implementation of requested feature in legacy module"
"Identified common pattern that could be extracted to shared utility"
```

### Decision Communication
```
"Automatically selected main branch as merge target based on git history"
"Multiple merge conflicts detected - manual intervention may be required"
```

### Warning Messages
```
"Deprecated API usage detected - recommend updating to newer version"
"Large file detected (>10MB) - consider using streaming approach"
```

### Workflow Transparency
```
"Breaking complex refactoring into smaller steps for safety"
"Prioritizing changes by impact - starting with highest-impact first"
```

## Integration

This tool provides a crucial communication channel between LLMs and users, enabling:

- **Real-time Feedback**: Users get immediate insight into LLM decision-making
- **Improved Transparency**: Clear communication of what the LLM is doing and why  
- **Better Debugging**: Contextual information helps diagnose issues
- **Enhanced UX**: Users stay informed during long-running operations
- **Workflow Visibility**: Clear understanding of workflow progress and state