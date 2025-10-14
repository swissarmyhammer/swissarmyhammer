Retrieve a specific todo item or the next incomplete item from a todo list.

## Parameters

- `item` (required): Either a specific ULID or "next" to get the first incomplete item

## Examples

```json
{
  "item": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

```json
{
  "item": "next"
}
```

## Returns

Returns the todo item as structured data and YAML with all fields.
