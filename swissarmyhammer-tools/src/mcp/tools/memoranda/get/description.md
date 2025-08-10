Retrieve a memo by its unique ID. Returns the memo content with metadata.

## Parameters

- `id` (required): ULID identifier of the memo to retrieve

## Examples

Get a memo by its ULID:
```json
{
  "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV"
}
```

## Returns

Returns the memo content with metadata including title, content, creation timestamp, and unique identifier.