# question_summary

Retrieve all persisted question/answer pairs as a YAML summary for agent context.

## Description

This tool reads all question/answer YAML files from `.swissarmyhammer/questions/` and merges them into a single YAML string. This allows agents to inject the full Q&A history into their context to understand previous user decisions and preferences.

## Parameters

### limit (optional)
- **Type**: integer
- **Description**: Maximum number of Q&A pairs to include (default: all)
- **Behavior**: When specified, returns the most recent N entries

## Response

Returns a JSON object containing:
- `summary`: YAML string with all Q&A entries
- `count`: Total number of Q&A pairs included

## Summary Format

The `summary` field contains YAML in this format:

```yaml
# Question/Answer History
# Generated: 2025-06-05T14:00:00.000Z
# Total Q&A Pairs: 3

entries:
  - timestamp: "2025-06-05T13:30:45.123Z"
    question: "What is your preferred approach?"
    answer: "Option A"

  - timestamp: "2025-06-05T13:45:12.456Z"
    question: "Select deployment target"
    answer: "staging"

  - timestamp: "2025-06-05T14:00:00.789Z"
    question: "Which database should we use?"
    answer: "PostgreSQL"
```

## Sorting and Ordering

- Entries are sorted by timestamp (oldest first)
- This maintains chronological order of decisions
- If `limit` is specified, the most recent N entries are returned (still sorted oldest to newest among those N)

## Error Handling

- **Empty Directory**: Returns empty entry list with count 0 (not an error)
- **Corrupted Files**: Individual file parsing errors are logged but don't fail the entire operation
- **Missing Directory**: Returns empty entry list with count 0 (not an error)

## Examples

### Get All Questions

```json
{}
```

Response:
```json
{
  "summary": "# Question/Answer History\n# Generated: 2025-06-05T14:00:00.000Z\n# Total Q&A Pairs: 5\n\nentries:\n  - timestamp: \"2025-06-05T13:30:45.123Z\"\n    question: \"What is your preferred approach?\"\n    answer: \"Option A\"\n  ...",
  "count": 5
}
```

### Get Recent 10 Questions

```json
{
  "limit": 10
}
```

Response:
```json
{
  "summary": "# Question/Answer History\n# Generated: 2025-06-05T14:00:00.000Z\n# Total Q&A Pairs: 10\n\nentries:\n  - timestamp: \"2025-06-05T13:50:12.456Z\"\n    question: \"Select deployment target\"\n    answer: \"staging\"\n  ...",
  "count": 10
}
```

## Use Cases

- **Multi-Session Context**: Access decisions made in previous sessions
- **Agent Memory**: Let agents remember user preferences across interactions
- **Decision Review**: Understand what choices have been made
- **Context Injection**: Inject Q&A history into agent prompts

## See Also

- `question_ask`: Ask a user a question via elicitation and persist the answer
