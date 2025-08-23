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
