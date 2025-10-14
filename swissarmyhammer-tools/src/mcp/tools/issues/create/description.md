Create a new issue stored as a markdown file in ./issues directory.

## Parameters

- `content` (required): Markdown content of the issue
- `name` (optional): Name of the issue (used in filename). When omitted, a ULID is auto-generated

## Examples

```json
{
  "name": "feature_name",
  "content": "# Implement new feature\\n\\nDetails..."
}
```

## Returns

Returns the created issue name and confirmation message.
