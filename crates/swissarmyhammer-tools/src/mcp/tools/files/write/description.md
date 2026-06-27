Write content to files with atomic operations, creating new files or overwriting existing ones.

## Read-before-write freshness guard

To prevent clobbering changes you have not seen, overwriting an **existing**
file is guarded by a freshness token:

- **New / nonexistent file** — unguarded. Write freely; no token is required.
- **Existing file** — supply `expected_hash`: the whole-file hash from your most
  recent `read files` of that path (the bare hex after the `#hash:` line). The
  write proceeds only when `expected_hash` matches the file's current on-disk
  content.
  - If `expected_hash` is **absent or stale** (the file changed since you read
    it), the file is **not** overwritten. Instead the tool returns the current
    file content as a SUCCESS — led by a `#hash:<hex>` token and hashline-tagged,
    exactly as `read files` would return it — so you can re-base your edit and
    retry with the fresh `expected_hash`.

This mirrors the read-before-edit mandate: read first, then write with the token
you saw. Only UTF-8 text files participate; a non-UTF-8 (binary) existing file is
rejected with an error rather than silently overwritten.

## Examples

Create a new file (no token needed):

```json
{
  "file_path": "/workspace/src/new_module.rs",
  "content": "//! New module\\n\\npub fn hello() {\\n    println!(\\\"Hello, world!\\\");\\n}"
}
```

Overwrite an existing file, presenting the token from your prior read:

```json
{
  "file_path": "/workspace/src/existing.rs",
  "content": "//! Updated module\\n",
  "expected_hash": "d41d8cd98f00b204e9800998ecf8427e"
}
```

## Returns

On a successful write, returns confirmation (`OK`).

When the freshness guard declines to overwrite an existing file (missing or stale
`expected_hash`), returns the current file content — a leading `#hash:<hex>`
token followed by the hashline-tagged content — so you can re-base and retry.
