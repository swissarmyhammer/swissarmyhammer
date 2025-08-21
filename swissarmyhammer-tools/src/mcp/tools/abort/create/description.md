Create an abort file to signal workflow termination. Creates `.swissarmyhammer/.abort` file with the abort reason for file-based abort detection.

## Parameters

- `reason` (required): String containing the abort reason/message

## Examples

Create an abort file with a reason:
```json
{
  "reason": "User cancelled the destructive operation"
}
```

Create an abort file with a detailed message:
```json
{
  "reason": "Invalid configuration detected: missing required database connection"
}
```

## Behavior

- Creates `.swissarmyhammer/.abort` file containing the reason text
- Ensures the `.swissarmyhammer/` directory exists
- File-based approach ensures abort state persists across process boundaries
- Tool returns success to allow proper error propagation in calling context

## Returns

Returns confirmation message with the abort reason that was written to the file.
