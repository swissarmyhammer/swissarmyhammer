Retrieve a specific todo item or the next incomplete item from a todo list.

## Parameters

- `item` (required): Either a specific ULID or "next" to get the first incomplete item

## Examples

Get a specific todo item by ULID:
```json
{
  "item": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

Get the next incomplete todo item:
```json
{
  "item": "next"
}
```

## Returns

Returns the todo item as structured data and YAML with all fields.
