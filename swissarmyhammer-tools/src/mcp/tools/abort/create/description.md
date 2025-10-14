Create an abort file to signal workflow termination.

## Parameters

- `reason` (required): String containing the abort reason/message

## Examples

```json
{
  "reason": "User cancelled the destructive operation"
}
```

## Returns

Returns confirmation with the abort reason written to `.swissarmyhammer/.abort`.
