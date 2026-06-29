Read file contents from the local filesystem with partial reading support.

Paths can be absolute or relative to the working directory. Use `offset` and `limit` for partial reads of large files.

## Examples

```json
{"path": "/workspace/src/main.rs"}
{"path": "logs/application.log", "offset": 1000, "limit": 100}
{"path": "/workspace/src/main.rs", "format": "plain"}
```

## Output format

The first line of a successful read is always a freshness-token metadata line:

```text
#hash:<hex>
```

`<hex>` is a whole-file content hash over the **full** on-disk bytes (independent
of any `offset`/`limit`). The per-line `N:HH` anchors are how `edit files`
detects staleness and refuses to clobber a line the model has not seen in its
current state; a full-file `write files` is unguarded and always clobbers.

Everything after that first line is the file content (subject to
`offset`/`limit`), rendered according to `format`:

- `hashline` (default) — each text line is prefixed with a `N:HH|` anchor, where
  `N` is the **absolute** 1-based line number (stable across `offset`/`limit`
  windows) and `HH` is a short content hash. `edit files` resolves these anchors
  back to lines, tolerating small drift and rejecting stale edits.
- `plain` — untagged content, exactly as it appears on disk (subject to
  `offset`/`limit`).

Only UTF-8 text is read. Non-UTF-8 (binary) files are rejected with an error
rather than decoded, so tagged output is always text.

## Returns

Returns the freshness-token line followed by the UTF-8 file content
(hashline-tagged text by default, untagged with `format: "plain"`).
