Send notification messages from LLM to user through the logging system.

## Parameters

- `message` (required): The message to notify the user about
- `level` (optional): The notification level - "info", "warn", or "error" (default: "info")
- `context` (optional): Structured JSON data for the notification

## Examples

Basic notification:
```json
{
  "message": "Processing large codebase - this may take a few minutes"
}
```

Notification with level and context:
```json
{
  "message": "Found potential security vulnerability",
  "level": "warn",
  "context": {
    "stage": "analysis",
    "affected_files": ["src/auth.rs"]
  }
}
```

## Returns

Returns confirmation message that notification was sent.
