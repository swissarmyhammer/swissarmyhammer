# js

Evaluate JavaScript expressions and manage variables in a process-global context.

## Operations

### set expression
Evaluate a JavaScript expression and store the result as a named variable.

**Parameters:**
- `name` (or `key`): Variable name to store the result
- `expression` (or `value`): JavaScript expression to evaluate

After storing the named result, all new/modified global variables created by the script
are automatically captured into the tracked context.

### get expression
Retrieve a variable's value by evaluating it as a JavaScript expression.

**Parameters:**
- `name` (or `key`): Variable name or expression to evaluate

## Context

- **Process-global**: Variables persist across calls within the same process
- **Thread-safe**: Safe for concurrent access from multiple tools
- **In-memory only**: State is lost when the process terminates

## Environment Variables

Environment variables are available as:
- `env.HOME`, `env.PATH`, etc.
- `process.env.HOME`, `process.env.PATH`, etc.

## Examples

### Basic arithmetic
```json
{"op": "set expression", "name": "x", "expression": "10 + 5"}
```
Returns: `15`

### Using existing variables
```json
{"op": "set expression", "name": "counter", "expression": "counter + 1"}
```

### Creating objects
```json
{"op": "set expression", "name": "config", "expression": "({name: 'test', retries: 3})"}
```

### Boolean flags
```json
{"op": "set expression", "name": "is_ready", "expression": "true"}
```

### Retrieve a value
```json
{"op": "get expression", "name": "x"}
```
