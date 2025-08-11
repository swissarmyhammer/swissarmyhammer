Update a memo's content by its ID. The title remains unchanged.

## Parameters

- `id` (required): ULID identifier of the memo to update
- `content` (required): New markdown content for the memo

## Examples

Update memo content:
```json
{
  "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "content": "# Updated Content\n\nNew information..."
}
```

## Returns

Returns confirmation of the update operation with the memo's updated metadata.