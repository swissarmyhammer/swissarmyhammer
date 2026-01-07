# cel_get

Retrieve the value of a stored variable from the process-global CEL context.

## Description

This tool retrieves a variable that was previously stored via `cel_set` by looking up its name in the global CEL context. The variable's value is evaluated as a CEL expression and returned.

The CEL context is:
- **Process-global**: Shared across all MCP server instances in the same process
- **Thread-safe**: Protected by RwLock for concurrent access
- **In-memory only**: State is lost when the process terminates

## Parameters

- `name` (string, required): Name of the variable to retrieve
  - **Alias**: `key` - Can be used interchangeably with `name`

## Returns

Returns the value of the stored variable directly as a JSON value (string, number, boolean, array, or object).

If the variable is not found or evaluation fails, returns an error string.

## Examples

### Retrieve a simple value
After setting with `cel_set`:
```json
{
  "name": "x",
  "expression": "10"
}
```

Retrieve with `cel_get`:
```json
{
  "name": "x"
}
```
Returns: `10`

### Using key alias
```json
{
  "key": "x"
}
```
Returns: `10`

### Retrieve a computed value
After setting:
```json
{
  "name": "total",
  "expression": "price * quantity"
}
```

Retrieve:
```json
{
  "name": "total"
}
```
Returns: `100` (if price=10 and quantity=10)

### Retrieve a boolean
After setting:
```json
{
  "name": "is_ready",
  "expression": "true"
}
```

Retrieve:
```json
{
  "name": "is_ready"
}
```
Returns: `true`

### Retrieve a list
After setting:
```json
{
  "name": "items",
  "expression": "[1, 2, 3]"
}
```

Retrieve:
```json
{
  "name": "items"
}
```
Returns: `[1, 2, 3]`

### Retrieve a map/object
After setting:
```json
{
  "name": "config",
  "expression": "{'retries': 3, 'timeout': 30}"
}
```

Retrieve:
```json
{
  "name": "config"
}
```
Returns: `{"retries": 3, "timeout": 30}`

## Error Handling

If the variable doesn't exist:

```json
{
  "name": "nonexistent"
}
```
Returns: `"CEL execution error: undeclared reference to 'nonexistent'..."`

## Use Cases

- Retrieve values stored by `cel_set`
- Access computed values for use in workflows
- Check the current value of flags or configuration
- Retrieve intermediate calculation results
- Read state set by previous workflow steps
