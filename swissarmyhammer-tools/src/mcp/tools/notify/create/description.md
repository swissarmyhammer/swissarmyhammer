# LLM Notification Tool

Send notification messages from LLM to user through the logging system. Enables LLMs to communicate important information, status updates, and contextual feedback during workflow execution.

## Description

The `notify_create` tool provides a direct communication channel for LLMs to send messages to users through the system's tracing infrastructure. This tool is essential for providing real-time feedback, status updates, and transparency during workflow execution, code analysis, and other long-running operations.

The tool integrates seamlessly with the tracing system using the `llm_notify` target, ensuring messages are properly categorized and can be filtered separately from other system logs. This enables users to focus specifically on LLM communications when needed.

## Parameters

- `message` (required): The message to notify the user about
  - Type: string
  - Must not be empty (minimum length: 1)
  - Supports Unicode characters and emojis
  - No maximum length restriction

- `level` (optional): The notification level (default: "info")
  - Type: string
  - Valid values: `"info"`, `"warn"`, `"error"`
  - Invalid values automatically default to `"info"`
  - Determines the logging level and visual presentation

- `context` (optional): Structured JSON data for the notification
  - Type: object
  - Can contain any valid JSON structure
  - Useful for providing machine-readable metadata
  - Logged as structured data for filtering and analysis

## Usage Examples

### Basic Information Notification
```json
{
  "message": "Processing large codebase - this may take a few minutes"
}
```

### Warning with Simple Context
```json
{
  "message": "Found potential security vulnerability in authentication logic",
  "level": "warn"
}
```

### Error Notification with Structured Context
```json
{
  "message": "Database migration detected - recommend backing up data before proceeding", 
  "level": "error",
  "context": {
    "stage": "analysis",
    "safety": "critical",
    "affected_files": ["migrations/001_initial.sql", "config/database.yml"]
  }
}
```

### Progress Notification with Metrics
```json
{
  "message": "Completed analysis of codebase",
  "level": "info", 
  "context": {
    "files_processed": 47,
    "issues_found": 3,
    "warnings": 12,
    "time_elapsed": "2.5s",
    "next_phase": "refactoring"
  }
}
```

### Complex Analysis Results
```json
{
  "message": "Code quality analysis complete with recommendations",
  "level": "info",
  "context": {
    "metrics": {
      "cyclomatic_complexity": 8.5,
      "test_coverage": 85.2,
      "technical_debt_hours": 4.2
    },
    "recommendations": [
      "Extract common validation logic",
      "Add integration tests for payment flow",
      "Reduce function complexity in UserService"
    ]
  }
}
```

## Notification Levels

### Info Level (Default)
- General status updates and progress information
- Successful completion messages
- Discovery notifications and insights
- Non-critical observations and recommendations

### Warn Level
- Potential issues that require attention
- Deprecated usage or patterns detected  
- Performance concerns or suboptimal approaches
- Safety recommendations and best practices

### Error Level  
- Critical issues requiring immediate attention
- Failures or blocking conditions
- Data safety concerns
- Prerequisites not met or validation failures

## Behavior and Implementation

### Logging Integration
- Messages are sent through the tracing system with target `"llm_notify"`
- The notification level maps directly to tracing levels (info, warn, error)
- Context data is included as structured logging fields for filtering
- Messages appear immediately in CLI output for real-time user feedback

### Validation and Error Handling
- Empty messages are rejected with a validation error
- Invalid notification levels automatically default to "info"
- Malformed context objects are preserved as-is (JSON validation handled by system)
- Tool execution continues even if logging fails (non-blocking)

### Rate Limiting
- Built-in rate limiting prevents notification spam
- Rate limits are configurable and apply per execution context
- Exceeded rate limits result in tool execution errors with clear messaging

## Response Format

Successful notifications return a confirmation message:

```json
{
  "content": [{
    "text": "Notification sent: {message}",
    "type": "text"
  }],
  "is_error": false
}
```

Error responses include detailed validation information:

```json
{
  "content": [{
    "text": "Notification validation failed: message cannot be empty",
    "type": "text"
  }],
  "is_error": true
}
```

## Use Cases and Examples

### Code Analysis and Discovery
```json
// Security Analysis
{
  "message": "Potential SQL injection vulnerability detected in user input handling",
  "level": "warn",
  "context": {
    "file": "src/controllers/UserController.php",
    "line": 145,
    "severity": "medium",
    "cwe": "CWE-89"
  }
}

// Architecture Insights
{
  "message": "Found existing implementation of requested feature in legacy module",
  "context": {
    "module": "legacy/payment-processor",
    "similarity": 85,
    "reuse_potential": "high"
  }
}

// Pattern Recognition
{
  "message": "Identified common validation pattern that could be extracted to shared utility",
  "context": {
    "occurrences": 7,
    "files": ["UserService.js", "OrderService.js", "PaymentService.js"],
    "estimated_reduction": "150 lines"
  }
}
```

### Workflow Status and Progress
```json
// Long-running Operations
{
  "message": "Starting comprehensive code analysis - estimated time: 3-5 minutes",
  "context": {
    "files_to_analyze": 247,
    "languages": ["TypeScript", "Python", "Rust"],
    "analysis_types": ["security", "performance", "maintainability"]
  }
}

// Phase Transitions
{
  "message": "Analysis phase complete - moving to refactoring recommendations",
  "context": {
    "phase_completed": "analysis",
    "next_phase": "refactoring",
    "progress": "40%",
    "time_elapsed": "2m 15s"
  }
}

// Completion Status
{
  "message": "All optimizations applied successfully",
  "context": {
    "optimizations": 12,
    "files_modified": 8,
    "performance_improvement": "23%",
    "test_status": "passing"
  }
}
```

### Decision Communication and Transparency
```json
// Automated Decisions
{
  "message": "Automatically selected main branch as merge target based on git history",
  "context": {
    "decision_factor": "recent_activity",
    "branch_candidates": ["main", "develop", "feature/auth"],
    "confidence": "high"
  }
}

// Conflict Resolution
{
  "message": "Multiple merge conflicts detected - manual intervention recommended",
  "level": "warn",
  "context": {
    "conflicts": 5,
    "affected_files": ["config.json", "package.json", "src/main.ts"],
    "resolution_strategies": ["manual_review", "automatic_theirs", "automatic_ours"]
  }
}

// Safety Measures
{
  "message": "Breaking complex refactoring into smaller steps for safety",
  "context": {
    "original_complexity": "high",
    "safety_approach": "incremental",
    "planned_steps": 4,
    "rollback_strategy": "git_commits"
  }
}
```

### Warning and Safety Notifications  
```json
// Deprecation Warnings
{
  "message": "Deprecated API usage detected - update recommended before next major version",
  "level": "warn",
  "context": {
    "api": "legacy-auth-service",
    "deprecated_since": "v2.1.0",
    "removal_planned": "v3.0.0",
    "migration_guide": "https://docs.example.com/migration/auth"
  }
}

// Performance Concerns
{
  "message": "Large file detected - consider streaming approach for better memory efficiency",
  "level": "warn",
  "context": {
    "file_size": "45.2MB",
    "file_type": "CSV",
    "current_approach": "load_all",
    "recommended_approach": "streaming"
  }
}

// Data Safety
{
  "message": "Database schema changes detected - backup recommended before proceeding",
  "level": "error",
  "context": {
    "schema_changes": ["alter_table", "drop_column"],
    "affected_tables": ["users", "orders"],
    "data_risk": "high",
    "backup_command": "pg_dump production > backup_$(date).sql"
  }
}
```

## Integration Patterns

### Prompt Template Usage

Notifications can be integrated into prompt templates using conditional logic:

```liquid
{% if complexity_score > 7 %}
  {{ notify_create({
    message: "Complex refactoring detected - proceeding with extra caution",
    level: "warn",
    context: {
      complexity: complexity_score,
      safety_measures: ["incremental_changes", "comprehensive_testing"]
    }
  }) }}
{% endif %}

{% assign files_count = files | size %}
{% if files_count > 50 %}
  {{ notify_create({
    message: "Large codebase analysis starting - estimated time: " | append: estimated_time,
    context: {
      files_count: files_count,
      estimated_duration: estimated_time
    }
  }) }}
{% endif %}
```

### Workflow Integration

LLMs can use notifications throughout workflow execution:

```
1. Initialize analysis environment
2. {{ notify_create({ message: "Environment ready - beginning code scan" }) }}
3. Scan codebase for patterns
4. {{ notify_create({ 
     message: "Found " + issues.length + " potential improvements",
     context: { issues: issues, priority_high: priority_high_count }
   }) }}
5. Prioritize changes by impact
6. {{ notify_create({ 
     message: "Starting with highest-impact changes first",
     context: { approach: "impact_prioritized", first_change: first_change }
   }) }}
```

### CLI Integration

When running through the CLI, notifications appear in the terminal output stream:

```bash
$ sah workflow run code-analysis
[INFO  llm_notify] Starting comprehensive analysis - 247 files detected
[WARN  llm_notify] Deprecated API usage found in 3 files - migration recommended  
[INFO  llm_notify] Analysis complete - 12 improvements identified
[INFO  llm_notify] All changes applied successfully
```

## Performance Characteristics

- **Low Latency**: Notifications are sent immediately through the tracing system
- **Non-blocking**: Logging failures do not interrupt tool execution
- **Memory Efficient**: Messages are streamed rather than buffered
- **Rate Limited**: Prevents notification spam while allowing burst communication
- **Structured Logging**: Context data enables efficient filtering and analysis

## Error Handling Scenarios

### Validation Errors
```json
// Empty message
{
  "message": "",
  "level": "info"
}
// Returns: "Notification validation failed: message cannot be empty"

// Invalid message type
{
  "message": 123,
  "level": "info"  
}
// Returns: JSON parsing error with type mismatch details
```

### Rate Limiting
```json
// Too many notifications in short time
// Returns: "Rate limit exceeded for notification: maximum 10 notifications per minute"
```

### System Integration
- **Tracing System Failures**: Tool continues execution, logs warning
- **Invalid Context Data**: Preserves original data, continues execution
- **Network Issues**: N/A (local logging only)
- **Permission Issues**: Handled by underlying tracing system

## Security Considerations

- **Data Sanitization**: Messages and context are logged as-provided
- **Sensitive Information**: Users responsible for avoiding secrets in notifications
- **Rate Limiting**: Prevents abuse and system resource exhaustion
- **Structured Logging**: Context data should not contain authentication tokens

## Technical Integration

### MCP Tool Registry
The tool is automatically registered with the MCP tool registry and available for:
- Direct CLI invocation through MCP protocol
- Integration with workflow systems
- Usage in prompt templates
- Programmatic access through tool context

### Logging Configuration
The tool respects system logging configuration:
- Log levels can be filtered at the tracing subscriber level  
- Output formatting controlled by logging backend
- File vs console output determined by environment configuration
- Structured data preserved for analysis tools

## Future Enhancements

The notification system is designed for extensibility and may support additional features:

- **Rich Formatting**: Support for markdown and structured text formatting
- **User Preferences**: Configurable notification levels and filtering
- **Persistence**: Optional notification history and replay capabilities
- **Integration Hooks**: Webhook or external system integration for notifications
- **Visual Indicators**: Enhanced CLI presentation with colors and symbols

This tool provides essential transparency and communication capabilities that enhance the user experience during LLM-driven development workflows, code analysis, and automated task execution.