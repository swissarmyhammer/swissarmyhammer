# cel_get

Evaluate a CEL (Common Expression Language) expression in the process-global CEL context and return the result.

## Description

This tool evaluates a CEL expression using the current global CEL context, which includes all variables previously stored via `cel_set`. The result is returned but not stored.

The CEL context is:
- **Process-global**: Shared across all MCP server instances in the same process
- **Thread-safe**: Protected by RwLock for concurrent access
- **In-memory only**: State is lost when the process terminates

## Parameters

- `expression` (string, required): CEL expression to evaluate

## Returns

Returns a JSON object containing:
- `result`: The computed value from the expression (type depends on expression result)

If the expression fails to compile or execute, returns an error string.

## Examples

### Basic arithmetic
```json
{
  "expression": "10 * 2"
}
```
Returns: `{"result": 20}`

### Referencing stored variables
```json
{
  "expression": "x + 5"
}
```
Returns: `{"result": 15}` (assuming x was set to 10)

### List operations
```json
{
  "expression": "[1, 2, 3].size()"
}
```
Returns: `{"result": 3}`

### Map/object access
```json
{
  "expression": "config.retries"
}
```
Returns: `{"result": 3}` (assuming config was set with retries field)

### Boolean operations
```json
{
  "expression": "x > 5 && y < 10"
}
```
Returns: `{"result": true}` (depending on x and y values)

### String operations
```json
{
  "expression": "'hello'.size()"
}
```
Returns: `{"result": 5}`

### Complex expressions
```json
{
  "expression": "items.filter(x, x > 5).map(x, x * 2)"
}
```
Returns: `{"result": [12, 14, 16]}` (if items is [4, 5, 6, 7, 8])

## Error Handling

Errors are returned as string values in the result:

```json
{
  "expression": "undefined_variable"
}
```
Returns: `{"result": "CEL execution error: undeclared reference to 'undefined_variable'..."}`

## Use Cases

- Query computed values without modifying state
- Test expressions before storing them
- Access properties of stored complex objects
- Perform calculations using multiple stored variables
- Check conditions using boolean expressions
