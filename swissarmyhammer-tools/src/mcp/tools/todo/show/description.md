# Todo Show Tool

Retrieve a specific todo item or the next incomplete item from a todo list.

## Purpose
Display todo items for review and work planning. Supports both specific item lookup by ULID and sequential workflow using the "next" pattern.

## Parameters

- `item` (required): Either a specific ULID or "next" to get the first incomplete item

## Behavior

- If `item` is a ULID: Returns the specific todo item as structured data and YAML
- If `item` is "next": Returns the first incomplete todo item (FIFO order)
- Enforces single-item focus to avoid context pollution
- Returns complete item with all fields
- Returns both JSON structure and YAML representation for easy reading

## Examples

Get a specific todo item by ID:
```json
{
  "item": "01K1KQM85501ECE8XJGNZKNJQW"
}
```

Get the next incomplete item:
```json
{
  "item": "next"
}
```
