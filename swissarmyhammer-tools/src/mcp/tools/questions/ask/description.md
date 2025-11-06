# question_ask

Ask the user a question via MCP elicitation and persist the answer.

## Description

This tool sends an elicitation request to the MCP client, prompting the user for input through the client's UI. The tool blocks until the user responds, then saves the question/answer pair to a YAML file in `.swissarmyhammer/questions/` for future reference.

## Requirements

- **MCP Protocol Version**: 2025-06-18 or later
- **Client Capability**: Elicitation support required
- **Blocking Behavior**: Tool execution blocks indefinitely until user responds

## Parameters

### question (required)
- **Type**: string
- **Description**: The question to ask the user

## Response

Returns a JSON object containing:
- `answer`: The user's response (string)
- `saved_to`: Path to the YAML file where the Q&A was saved

## Persistence Format

Each question/answer is saved to a separate YAML file:
- **Directory**: `.swissarmyhammer/questions/`
- **Filename Pattern**: `YYYYMMDD_HHMMSS_question.yaml`
- **Content**:
  ```yaml
  # Saved at 2025-06-05 13:30:45 UTC
  timestamp: "2025-06-05T13:30:45.123Z"
  question: "What is your preferred approach?"
  answer: "Option A"
  ```

## Error Handling

- **No Elicitation Support**: Returns error if client doesn't support elicitation or if peer is not available
- **User Cancellation**: Returns error if user cancels the elicitation request
- **File System Errors**: Returns error if unable to save Q&A to file

## Examples

### Basic Question

```json
{
  "question": "What is your preferred deployment target?"
}
```

Response:
```json
{
  "answer": "staging",
  "saved_to": ".swissarmyhammer/questions/20250605_133045_question.yaml"
}
```

### Configuration Choice

```json
{
  "question": "Which database should we use for this project?"
}
```

Response:
```json
{
  "answer": "PostgreSQL",
  "saved_to": ".swissarmyhammer/questions/20250605_140012_question.yaml"
}
```

## Use Cases

- **Interactive Workflows**: Gather user input during agent execution
- **Configuration Decisions**: Ask users to choose between options
- **Context Collection**: Build up a history of user preferences for future reference
- **Multi-Session Context**: Answers are persisted and can be retrieved later with `question_summary`

## See Also

- `question_summary`: Retrieve all persisted question/answer pairs as YAML
