# question_summary

Retrieve all persisted question/answer pairs as a YAML summary for agent context.

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
