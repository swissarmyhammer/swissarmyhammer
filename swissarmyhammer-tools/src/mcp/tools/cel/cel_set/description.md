# cel_set

Evaluate a CEL (Common Expression Language) expression and store the result as a named variable in the process-global CEL context.

## Description

This tool evaluates a CEL expression in the current global context and stores the result as a named variable that can be referenced in future `cel_set` and `cel_get` operations.

The CEL context is:
- **Process-global**: Shared across all MCP server instances in the same process
- **Thread-safe**: Protected by RwLock for concurrent access
- **In-memory only**: State is lost when the process terminates

## Parameters

- `name` (string, required): Name of the variable to store the result
- `expression` (string, required): CEL expression to evaluate

## Returns

Returns the computed value from the expression directly as a JSON value (string, number, boolean, array, or object).

If the expression fails to compile or execute, returns an error string.

## Examples

### Basic arithmetic
```json
{
  "name": "x",
  "expression": "10 + 5"
}
```
Returns: `15`

### Using existing variables
```json
{
  "name": "counter",
  "expression": "counter + 1"
}
```
Returns: `2` (assuming counter was 1)

### Creating complex structures
```json
{
  "name": "config",
  "expression": "{name: 'test', retries: 3, enabled: true}"
}
```
Returns: `{"name": "test", "retries": 3, "enabled": true}`

### List operations
```json
{
  "name": "items",
  "expression": "[1, 2, 3]"
}
```
Returns: `[1, 2, 3]`

### String operations
```json
{
  "name": "greeting",
  "expression": "'Hello' + ' ' + 'World'"
}
```
Returns: `"Hello World"`

## Error Handling

Errors are returned as string values:

```json
{
  "name": "bad",
  "expression": "undefined_variable"
}
```
Returns: `"CEL execution error: ..."`
