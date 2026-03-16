Ask the user a question via MCP elicitation and persist the answer.

Sends an elicitation request to the MCP client UI. Blocks until the user responds. Saves the Q&A pair to `.sah/questions/` for future reference.

Requires MCP protocol 2025-06-18+ with elicitation support.

## Examples

```json
{"question": "What is your preferred deployment target?"}
```

Returns: `{"answer": "staging", "saved_to": ".sah/questions/20250605_133045_question.yaml"}`
