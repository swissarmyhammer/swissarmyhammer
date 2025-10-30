Create an abort file to signal workflow termination.

## Examples

```json
{
  "reason": "User cancelled the destructive operation"
}
```

## Returns

Returns confirmation with the abort reason written to `.swissarmyhammer/.abort`.
