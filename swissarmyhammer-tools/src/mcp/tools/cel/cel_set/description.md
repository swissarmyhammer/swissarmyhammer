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

Returns a JSON object containing:
- `result`: The computed value from the expression (type depends on expression result)

If the expression fails to compile or execute, returns an error string.

## Examples

### Basic arithmetic
```json
{
  "name": "x",
  "expression": "10 + 5"
}
```
Returns: `{"result": 15}`

### Using existing variables
```json
{
  "name": "counter",
  "expression": "counter + 1"
}
```
Returns: `{"result": 2}` (assuming counter was 1)

### Creating complex structures
```json
{
  "name": "config",
  "expression": "{name: 'test', retries: 3, enabled: true}"
}
```
Returns: `{"result": {"name": "test", "retries": 3, "enabled": true}}`

### List operations
```json
{
  "name": "items",
  "expression": "[1, 2, 3]"
}
```
Returns: `{"result": [1, 2, 3]}`

### String operations
```json
{
  "name": "greeting",
  "expression": "'Hello' + ' ' + 'World'"
}
```
Returns: `{"result": "Hello World"}`

## Error Handling

Errors are returned as string values in the result:

```json
{
  "name": "bad",
  "expression": "undefined_variable"
}
```
Returns: `{"result": "CEL execution error: ..."}`
