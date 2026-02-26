Question operations for interactive user elicitation and Q&A history.

## Operations

- **ask question**: Ask the user a question via MCP elicitation and persist the answer
- **summarize questions**: Retrieve all persisted question/answer pairs as a YAML summary

## Examples

```json
{"op": "ask question", "question": "What is your preferred deployment target?"}
```

```json
{"op": "summarize questions"}
```

```json
{"op": "summarize questions", "limit": 5}
```
