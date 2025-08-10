Delete a memo by its unique ID. This action cannot be undone.

## Parameters

- `id` (required): ULID identifier of the memo to delete

## Examples

Delete a memo by its ULID:
```json
{
  "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV"
}
```

## Returns

Returns confirmation of the deletion operation. This action is permanent and cannot be undone.